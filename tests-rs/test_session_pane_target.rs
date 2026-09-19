// Requirement: `-t 'session:%paneid'` must resolve the same way tmux
// resolves it -- the pane id names a PANE, not a window, even though it sits
// in the target's window slot (e.g. `display-message -t 'sess:%4' -p
// '#{pane_id}'`). Today `cli_validate_window_pane_target` in src/main.rs
// splits the post-colon remainder on the LAST '.', so a dot-less remainder
// like "%4" is never recognized as a pane id and is instead validated as a
// window name, producing "psmux: can't find window: %4" (rc=1) even though
// the pane exists. `-t 'sess:.%4'` (with the disambiguating dot) already
// works, proving the split-on-'.' path itself is fine -- only the no-dot
// path is wrong.
//
// This locks in the pure parsing step the fix needs: splitting the
// post-colon remainder into (window_part, pane_part) must treat a bare
// "%<id>" (or bare numeric index, or relative-pane token) with NO dot as a
// pane-only remainder (window_part = None), not as a window name. Assumed
// signature for the implementer, to be added in src/main.rs and used by
// cli_validate_window_pane_target:
//
//   fn split_target_window_pane(rest: &str) -> (Option<&str>, Option<&str>)
//
// where `rest` is the substring after the target's first ':' (e.g. "%4",
// "win.%4", "win", "win.5"). Returns (window component, pane component).

use crate::split_target_window_pane;

#[test]
fn bare_pane_id_with_no_dot_is_a_pane_not_a_window() {
    // "sess:%4" -> rest is "%4": no window, pane id %4.
    assert_eq!(split_target_window_pane("%4"), (None, Some("%4")));
}

#[test]
fn bare_numeric_index_with_no_dot_is_a_pane_not_a_window() {
    // tmux also accepts "sess:4" meaning pane index 4 of the active window
    // when there is no dot.
    assert_eq!(split_target_window_pane("4"), (None, Some("4")));
}

#[test]
fn dotted_window_and_pane_id_still_split_on_last_dot() {
    // "sess:win.%4" -> window "win", pane "%4" (unchanged behavior).
    assert_eq!(split_target_window_pane("win.%4"), (Some("win"), Some("%4")));
}

#[test]
fn window_name_alone_has_no_pane_component() {
    assert_eq!(split_target_window_pane("win"), (Some("win"), None));
}

#[test]
fn window_name_containing_dots_is_not_misread_as_a_pane_split() {
    // A window literally named "a.b" (no leading '%', not all-digit, not a
    // relative-pane token) must not be chopped at the dot.
    assert_eq!(split_target_window_pane("a.b"), (Some("a.b"), None));
}
