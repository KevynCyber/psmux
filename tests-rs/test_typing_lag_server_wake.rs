// Covers: LAG-001
// Requirement: pane output wakes the server loop immediately instead of
// waiting out its timed recv (a ~15.6ms Windows tick per keystroke echo).
// Marking PTY data ready sends exactly one CtrlReq::Wake to the registered
// server wake sender on the false -> true transition; repeated marks while
// the flag is still set send nothing, and once the server loop clears the
// flag the next mark sends one Wake again.
//
// The invariant "one Wake per false -> true transition" holds even when a
// pane parser thread elsewhere in the test binary marks the flag between
// our steps, so these counts are not order-dependent. WAKE_TEST_LOCK
// serializes the tests in this module, which are the only in-process
// writers of the global wake sender and the only clearers of the flag.

use crate::types::{mark_pty_data_ready, register_server_waker, CtrlReq, PTY_DATA_READY};
use std::sync::atomic::Ordering;
use std::sync::mpsc;

static WAKE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn wake_count(rx: &mpsc::Receiver<CtrlReq>) -> usize {
    rx.try_iter().filter(|m| matches!(m, CtrlReq::Wake)).count()
}

#[test]
fn first_mark_sends_exactly_one_wake() {
    let _guard = WAKE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (tx, rx) = mpsc::channel::<CtrlReq>();
    register_server_waker(tx);
    PTY_DATA_READY.store(false, Ordering::Release);

    mark_pty_data_ready();

    assert_eq!(wake_count(&rx), 1, "false -> true must send exactly one Wake");
    assert!(PTY_DATA_READY.load(Ordering::Acquire), "mark must leave the flag set");
}

#[test]
fn repeated_marks_before_clear_send_no_extra_wake() {
    let _guard = WAKE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (tx, rx) = mpsc::channel::<CtrlReq>();
    register_server_waker(tx);
    PTY_DATA_READY.store(false, Ordering::Release);

    mark_pty_data_ready();
    mark_pty_data_ready();
    mark_pty_data_ready();

    assert_eq!(wake_count(&rx), 1, "marks while already set must not send another Wake");
}

#[test]
fn mark_after_server_clears_flag_sends_wake_again() {
    let _guard = WAKE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (tx, rx) = mpsc::channel::<CtrlReq>();
    register_server_waker(tx);
    PTY_DATA_READY.store(false, Ordering::Release);

    mark_pty_data_ready();
    let first = wake_count(&rx);
    // Server loop consumes the flag (same swap it performs each iteration).
    let consumed = PTY_DATA_READY.swap(false, Ordering::AcqRel);
    mark_pty_data_ready();
    let second = wake_count(&rx);

    assert!(consumed, "flag was set by the first mark");
    assert_eq!(first, 1);
    assert_eq!(second, 1, "after the server clears the flag, the next mark wakes it again");
}

#[test]
fn mark_with_disconnected_waker_still_sets_flag() {
    let _guard = WAKE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (tx, rx) = mpsc::channel::<CtrlReq>();
    register_server_waker(tx);
    drop(rx);
    PTY_DATA_READY.store(false, Ordering::Release);

    // Server gone (receiver dropped): must not panic, flag still published.
    mark_pty_data_ready();

    assert!(PTY_DATA_READY.load(Ordering::Acquire));
}
