// Covers: ZDEP-020
// Requirement: find_matching_brace (src/format.rs) must balance ANY bare
// `{` inside a `#{...}` expression body against a bare `}`, the same way
// tmux balances braces, not just track nesting via literal `#{` opens.
// Today it decrements depth on every bare `}` while only incrementing on
// `#{`, so a bare `{` that appears inside the expression body (a `${1}`
// substitution group reference, or a regex bounded-repeat token like
// `{,1}`) is never counted and its matching `}` closes the expression
// early, truncating the rest of the format string.

use crate::format::expand_format;
use crate::types::{AppState, LayoutKind, Node, Window};

fn app() -> AppState {
    let mut a = AppState::new("modvar".to_string());
    a.window_base_index = 0;
    a.windows.push(Window {
        root: Node::Split { kind: LayoutKind::Horizontal, sizes: vec![], children: vec![] },
        active_path: vec![],
        name: "shell".to_string(),
        id: 0,
        area: psmux_tui::layout::Rect::new(0, 0, 120, 30),
        window_size: None,
        activity_flag: false,
        bell_flag: false,
        silence_flag: false,
        last_output_time: std::time::Instant::now(),
        last_seen_version: 0,
        manual_rename: false,
        layout_index: 0,
        pane_mru: vec![],
        zoom_saved: None,
        linked_from: None,
        floating: Vec::new(),
        floating_focus: None,
    });
    a
}

/// Reported reproduction: a `${1}` substitution group reference inside the
/// replacement text of a `s///` modifier must not truncate the expression
/// at the `}` that closes `${1}`.
#[test]
fn dollar_brace_group_ref_does_not_truncate_expression() {
    let out = expand_format("#{s/(mod)var/${1}X/:session_name}", &app());
    assert_eq!(
        out, "modX",
        "the `}}` closing `${{1}}` must not be mistaken for the expression's \
         own closing brace; got {:?}",
        out
    );
}

/// Reported reproduction: a regex bounded-repeat token `{,1}` inside the
/// pattern of a `s///` modifier must not truncate the expression at the
/// `}` that closes `{,1}`.
#[test]
fn regex_bounded_repeat_brace_does_not_truncate_expression() {
    let out = expand_format("#{s/o{0,1}d/_/:session_name}", &app());
    assert_eq!(
        out, "m_var",
        "the `}}` closing `{{0,1}}` must not be mistaken for the expression's \
         own closing brace; got {:?}",
        out
    );
}

/// Regression: nested `#{...}` expressions must still balance correctly.
#[test]
fn nested_expressions_still_balance() {
    let out = expand_format("#{?#{==:#{session_name},modvar},yes,no}", &app());
    assert_eq!(out, "yes", "nested #{{...}} expressions must still balance");
}

/// Regression: a literal after a brace-containing expression must survive.
#[test]
fn literal_after_expression_survives() {
    let out = expand_format("[#{s/(mod)var/${1}X/:session_name}]", &app());
    assert_eq!(
        out, "[modX]",
        "literal text following the expression must not be swallowed"
    );
}

/// Regression: a simple bare expression with no embedded braces.
#[test]
fn simple_bare_expression_still_works() {
    assert_eq!(expand_format("#{session_name}", &app()), "modvar");
}
