// Covers: LAG-004
// Requirement: a small output batch (a keystroke echo) is parsed at once
// instead of sitting through the adaptive coalescing wait, whose 1ms sleeps
// round up to a ~15.6ms Windows tick. should_coalesce(len) is false for a
// staged batch under 256 bytes and true at 256 bytes and above, so large
// multi-chunk frames still coalesce into one atomic parser update.

use crate::pane::should_coalesce;

#[test]
fn single_keystroke_echo_does_not_coalesce() {
    assert!(!should_coalesce(1));
}

#[test]
fn typical_echo_with_cursor_sequences_does_not_coalesce() {
    // e.g. "a" plus a prompt redraw / cursor-position sequence.
    let echo = b"a\x1b[?25l\x1b[1;5H\x1b[?25h";
    assert!(!should_coalesce(echo.len()));
}

#[test]
fn batch_just_under_threshold_does_not_coalesce() {
    assert!(!should_coalesce(255));
}

#[test]
fn batch_at_threshold_coalesces() {
    assert!(should_coalesce(256));
}

#[test]
fn large_batch_coalesces() {
    assert!(should_coalesce(64 * 1024));
    assert!(should_coalesce(usize::MAX));
}
