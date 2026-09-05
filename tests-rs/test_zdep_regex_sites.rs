// Covers: ZDEP-018
// Requirement: the three `regex::` call sites in `src/format.rs`
// (`Substitute`, `Match` with `regex: true`, `SearchContent` incl. its
// `regex::escape` path) switch to `psmux_regex` with unchanged control
// flow: `Substitute` replaces the first match only and leaves the value
// unchanged on an invalid pattern; `Match` returns `"0"` on an invalid
// pattern; `SearchContent` returns `""` on an invalid pattern; all three
// prepend `(?i)` when case-insensitive. The `regex` manifest line is
// removed; `psmux-regex` is a workspace member and root path dependency.

use crate::format::expand_format_for_window;
use crate::types::AppState;

fn mock_app() -> AppState {
    let mut app = AppState::new("test_session".to_string());
    app.window_base_index = 0;
    app
}

// ---------------------------------------------------------------------
// Substitute (#{s/pattern/replacement/flags:target})
// ---------------------------------------------------------------------

#[test]
fn substitute_replaces_first_occurrence_only_with_two_or_more_matches() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/o/X/:foo boo}", &app, 0);
    assert_eq!(val, "fXo boo");
}

#[test]
fn substitute_dollar_group_ref_numeric() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/(o+)/[$1]/:foo}", &app, 0);
    assert_eq!(val, "f[oo]");
}

#[test]
fn substitute_dollar_group_ref_braced() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/(o+)/[${1}]/:foo}", &app, 0);
    assert_eq!(val, "f[oo]");
}

#[test]
fn substitute_dollar_dollar_is_literal_dollar() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/o/$$/:foo}", &app, 0);
    assert_eq!(val, "f$o");
}

#[test]
fn substitute_dollar_followed_by_non_group_stays_literal() {
    let app = mock_app();
    assert_eq!(expand_format_for_window("#{s/o/$-1/:foo}", &app, 0), "f$-1o");
    assert_eq!(expand_format_for_window("#{s/o/$ 1/:foo}", &app, 0), "f$ 1o");
    assert_eq!(expand_format_for_window("#{s/o/x$/:foo}", &app, 0), "fx$o");
}

#[test]
fn substitute_dollar_zero_is_whole_match() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/o+/[$0]/:foo}", &app, 0);
    assert_eq!(val, "f[oo]");
}

#[test]
fn substitute_dollar_nonexistent_group_is_empty() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/(o)/[$2]/:foo}", &app, 0);
    assert_eq!(val, "f[]o");
}

#[test]
fn substitute_case_insensitive_flag() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/FOO/bar/i:xfoox}", &app, 0);
    assert_eq!(val, "xbarx");
}

#[test]
fn substitute_invalid_pattern_leaves_value_unchanged() {
    let app = mock_app();
    let val = expand_format_for_window("#{s/a{,5}/X/:a{,5}text}", &app, 0);
    assert_eq!(val, "a{,5}text");
}

#[test]
fn substitute_catastrophic_pattern_does_not_hang() {
    // (a*)*b on 10,000 a's: the Pike-VM-backed psmux_regex must not
    // backtrack; the crate::regex equivalent hung under naive backtracking.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let app = mock_app();
        let value = "a".repeat(10_000);
        let expr = format!("#{{s/(a*)*b/X/:{}}}", value);
        let val = expand_format_for_window(&expr, &app, 0);
        let _ = tx.send(val);
    });
    let result = rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("substitute with (a*)*b must finish within 5s (no catastrophic backtracking)");
    assert_eq!(result, "a".repeat(10_000), "no match: value must be unchanged");
}

// ---------------------------------------------------------------------
// Match with regex: true (#{m/r...:pattern,subject})
// ---------------------------------------------------------------------

#[test]
fn match_regex_true_matching() {
    let app = mock_app();
    assert_eq!(expand_format_for_window("#{m/r:^pwsh$,pwsh}", &app, 0), "1");
}

#[test]
fn match_regex_true_nonmatching() {
    let app = mock_app();
    assert_eq!(expand_format_for_window("#{m/r:^pwsh$,bash}", &app, 0), "0");
}

#[test]
fn match_regex_case_insensitive_flag() {
    let app = mock_app();
    assert_eq!(expand_format_for_window("#{m/ri:^PWSH$,pwsh}", &app, 0), "1");
}

#[test]
fn match_regex_invalid_pattern_returns_zero() {
    let app = mock_app();
    assert_eq!(expand_format_for_window("#{m/r:(a,text}", &app, 0), "0");
}

// ---------------------------------------------------------------------
// SearchContent (#{C/r...:pattern}) — escape-path coverage
// ---------------------------------------------------------------------
//
// Constructing a pane with real screen content (a live vt100 parser +
// AppState::windows entry) is out of scope for a format.rs unit test:
// AppState::new() starts with an empty `windows` Vec and the pane/window
// construction machinery (Node::Leaf with a running `Pane` holding a
// `vt100::Parser`) is exercised only via higher-level session/server
// integration tests elsewhere in this repo, not from tests-rs unit tests.
// With no window at win_idx, SearchContent's window/pane lookup fails
// before any regex is built and it always returns "" regardless of the
// regex engine underneath — so that path cannot discriminate `regex` from
// `psmux_regex` here and is not asserted as a behavior lock in this file.
// The escape step feeding SearchContent's non-regex mode is asserted
// directly against `psmux_regex::escape`, which is what SearchContent's
// non-regex branch calls before compiling the pattern.

#[test]
fn search_content_escape_neutralizes_metacharacters() {
    assert_eq!(psmux_regex::escape("a.b(c)[d]$"), "a\\.b\\(c\\)\\[d\\]\\$");
}

#[test]
fn search_content_escape_leaves_non_metacharacters_unchanged() {
    assert_eq!(psmux_regex::escape("a/b_c:d"), "a/b_c:d");
}

#[test]
fn search_content_with_no_window_returns_empty_regardless_of_mode() {
    let app = mock_app();
    assert_eq!(expand_format_for_window("#{C/r:^foo$}", &app, 0), "");
    assert_eq!(expand_format_for_window("#{C:a.b(c)}", &app, 0), "");
}

// ---------------------------------------------------------------------
// Manifest / source-token assertions
// ---------------------------------------------------------------------

#[test]
fn root_manifest_has_no_regex_dependency_line_and_lists_psmux_regex() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("read root Cargo.toml");
    assert!(
        !manifest.lines().any(|l| l.trim_start().starts_with("regex =")),
        "root Cargo.toml must not depend on the `regex` crate anymore"
    );
    assert!(
        manifest.contains("psmux-regex"),
        "root Cargo.toml must list psmux-regex as a workspace member and dependency:\n{}",
        manifest
    );
    let members_section = manifest
        .split("[workspace]")
        .nth(1)
        .and_then(|s| s.split("members").nth(1))
        .and_then(|s| s.split(']').next())
        .unwrap_or("");
    assert!(
        members_section.contains("crates/psmux-regex"),
        "[workspace] members must include crates/psmux-regex"
    );
}

#[test]
fn format_rs_has_no_regex_crate_token_only_psmux_regex() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/format.rs"))
        .expect("read src/format.rs");
    assert!(
        !src.contains("regex::"),
        "src/format.rs must no longer reference the `regex` crate directly"
    );
    assert!(
        src.contains("psmux_regex::"),
        "src/format.rs must call into psmux_regex"
    );
}
