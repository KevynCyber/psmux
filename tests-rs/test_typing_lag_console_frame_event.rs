// Covers: LAG-003
// Requirement: the client input loop wakes as soon as a frame arrives from
// the server instead of sleeping out its input-poll timeout (a ~15.6ms
// Windows tick per keystroke echo). The console layer owns an auto-reset
// frame event; signal_frame_ready() sets it, and
// wait_input_or_frame(input_handle, timeout) returns Wakeup::FrameReady
// promptly even under a long timeout. Pending input wins when both are
// signalled, and one signal yields exactly one FrameReady.
//
// No timing is asserted: every "prompt" case signals BEFORE the call, and
// every "nothing pending" case uses a zero timeout. The fake input handle is
// a Win32 event the test owns (stdin under the test harness is not a
// console, so the real console handle cannot be used deterministically).

use crate::term::console::{signal_frame_ready, wait_input_or_frame, Wakeup};
use std::ffi::c_void;
use std::time::Duration;

#[link(name = "kernel32")]
extern "system" {
    fn CreateEventW(attrs: isize, manual_reset: i32, initial_state: i32, name: isize) -> isize;
    fn CloseHandle(handle: isize) -> i32;
}

static FRAME_EVENT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Generous upper bound for the "long timeout" cases; a correct impl
/// returns immediately because the event is already signalled.
const LONG: Duration = Duration::from_secs(30);

/// Test-owned manual-reset event standing in for the console input handle.
struct FakeInput(isize);
impl FakeInput {
    fn new(signalled: bool) -> Self {
        let h = unsafe { CreateEventW(0, 1, i32::from(signalled), 0) };
        assert!(h != 0, "CreateEventW failed");
        FakeInput(h)
    }
    fn handle(&self) -> *mut c_void {
        self.0 as *mut c_void
    }
}
impl Drop for FakeInput {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// Consume any frame signal left over from an earlier test.
fn drain_frame_event(idle: &FakeInput) {
    for _ in 0..4 {
        match wait_input_or_frame(idle.handle(), Duration::ZERO) {
            Ok(Wakeup::FrameReady) => continue,
            _ => return,
        }
    }
}

#[test]
fn signalled_frame_event_ends_a_long_wait_as_frame_ready() {
    let _guard = FRAME_EVENT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let idle = FakeInput::new(false);
    drain_frame_event(&idle);

    signal_frame_ready();
    let got = wait_input_or_frame(idle.handle(), LONG).expect("wait must not error");

    assert!(matches!(got, Wakeup::FrameReady), "a signalled frame event must end the wait as FrameReady");
}

#[test]
fn frame_event_auto_resets_after_one_frame_ready() {
    let _guard = FRAME_EVENT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let idle = FakeInput::new(false);
    drain_frame_event(&idle);

    signal_frame_ready();
    let first = wait_input_or_frame(idle.handle(), LONG).expect("first wait");
    let second = wait_input_or_frame(idle.handle(), Duration::ZERO).expect("second wait");

    assert!(matches!(first, Wakeup::FrameReady));
    assert!(matches!(second, Wakeup::Timeout), "one signal must yield exactly one FrameReady");
}

#[test]
fn nothing_pending_with_zero_timeout_is_timeout() {
    let _guard = FRAME_EVENT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let idle = FakeInput::new(false);
    drain_frame_event(&idle);

    let got = wait_input_or_frame(idle.handle(), Duration::ZERO).expect("wait must not error");

    assert!(matches!(got, Wakeup::Timeout));
}

#[test]
fn pending_input_wins_over_a_pending_frame() {
    let _guard = FRAME_EVENT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let idle = FakeInput::new(false);
    drain_frame_event(&idle);
    let ready_input = FakeInput::new(true);

    signal_frame_ready();
    let got = wait_input_or_frame(ready_input.handle(), LONG).expect("wait must not error");
    drain_frame_event(&idle);

    assert!(matches!(got, Wakeup::Input), "keystrokes must never be starved by frame wakeups");
}

#[test]
fn signalled_input_ends_a_long_wait_as_input() {
    let _guard = FRAME_EVENT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let idle = FakeInput::new(false);
    drain_frame_event(&idle);
    let ready_input = FakeInput::new(true);

    let got = wait_input_or_frame(ready_input.handle(), LONG).expect("wait must not error");

    assert!(matches!(got, Wakeup::Input));
}
