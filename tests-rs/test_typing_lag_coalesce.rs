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

// Covers: LAG-005
// Requirement: a multi-chunk ConPTY redraw that opens with a small chunk
// (cursor hide, cursor move, a split escape sequence) must still wait for
// the rest of the frame, or the server snapshots a half-applied frame (torn
// line / cursor flicker). should_coalesce_batch(bytes) is true for a batch
// that ends mid-escape-sequence or leaves the cursor hidden, and false for a
// small complete echo so LAG-004 keystroke latency is kept.
mod batch {
    use crate::pane::should_coalesce_batch;

    #[test]
    fn plain_keystroke_echo_does_not_coalesce() {
        assert!(!should_coalesce_batch(b"a"));
    }

    #[test]
    fn echo_ending_with_cursor_shown_does_not_coalesce() {
        assert!(!should_coalesce_batch(b"a\x1b[?25h"));
        assert!(!should_coalesce_batch(b"a\x1b[?25l\x1b[1;5H\x1b[?25h"));
    }

    #[test]
    fn empty_batch_does_not_coalesce() {
        assert!(!should_coalesce_batch(b""));
    }

    #[test]
    fn lone_cursor_hide_chunk_coalesces() {
        assert!(should_coalesce_batch(b"\x1b[?25l"));
    }

    #[test]
    fn cursor_hidden_then_moved_without_show_coalesces() {
        assert!(should_coalesce_batch(b"\x1b[?25l\x1b[1;5H"));
        assert!(should_coalesce_batch(b"\x1b[?25l\x1b[1;5Habc"));
    }

    #[test]
    fn cursor_shown_then_hidden_again_coalesces() {
        assert!(should_coalesce_batch(b"\x1b[?25h\x1b[?25l"));
    }

    #[test]
    fn batch_ending_on_lone_esc_coalesces() {
        assert!(should_coalesce_batch(b"a\x1b"));
    }

    #[test]
    fn batch_ending_inside_csi_coalesces() {
        assert!(should_coalesce_batch(b"a\x1b["));
        assert!(should_coalesce_batch(b"a\x1b[1;5"));
        assert!(should_coalesce_batch(b"a\x1b[?25"));
    }

    #[test]
    fn batch_ending_inside_osc_coalesces() {
        assert!(should_coalesce_batch(b"\x1b]0;title"));
    }

    #[test]
    fn terminated_osc_does_not_coalesce() {
        assert!(!should_coalesce_batch(b"\x1b]0;title\x07"));
        assert!(!should_coalesce_batch(b"\x1b]0;title\x1b\\"));
    }

    #[test]
    fn large_complete_batch_still_coalesces() {
        assert!(should_coalesce_batch(&[b'a'; 256]));
        assert!(!should_coalesce_batch(&[b'a'; 255]));
    }
}
