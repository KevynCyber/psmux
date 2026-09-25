// Covers: LAG-007
// Requirement: the server requests the 1ms system timer only while clients
// are attached, not for its whole lifetime (a detached server otherwise
// keeps the whole machine at a raised timer rate, costing power).
// TimerResolutionGuard::acquire() is ref-counted RAII: the first live guard
// requests 1ms, dropping the last releases it. timer_resolution_holders()
// exposes the live count. Each WriterHandle (one per attached persistent
// client connection) holds a guard for exactly its own lifetime.
//
// Counts are asserted relative to a baseline taken under TIMER_TEST_LOCK,
// which serializes the only in-process creators of guards in this binary.

use crate::sched_priority::{timer_resolution_holders, TimerResolutionGuard};
use crate::types::WriterMsg;
use crate::wake::WriterHandle;
use std::sync::mpsc;

static TIMER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn acquire_holds_resolution_and_drop_releases_it() {
    let _lock = TIMER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = timer_resolution_holders();

    let guard = TimerResolutionGuard::acquire();
    let held = timer_resolution_holders();
    drop(guard);
    let released = timer_resolution_holders();

    assert_eq!(held, base + 1, "a live guard must hold the 1ms timer");
    assert_eq!(released, base, "dropping the guard must release it");
}

#[test]
fn nested_guards_release_only_when_the_last_one_drops() {
    let _lock = TIMER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = timer_resolution_holders();

    let first = TimerResolutionGuard::acquire();
    let second = TimerResolutionGuard::acquire();
    let both = timer_resolution_holders();
    drop(first);
    let one_left = timer_resolution_holders();
    drop(second);
    let none_left = timer_resolution_holders();

    assert_eq!(both, base + 2);
    assert_eq!(one_left, base + 1, "an earlier drop must not release a still-held timer");
    assert_eq!(none_left, base);
}

#[test]
fn guard_dropped_on_another_thread_still_releases() {
    let _lock = TIMER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = timer_resolution_holders();

    let guard = TimerResolutionGuard::acquire();
    std::thread::spawn(move || drop(guard)).join().expect("drop thread");

    assert_eq!(timer_resolution_holders(), base, "connection threads drop guards off the acquiring thread");
}

#[test]
fn writer_handle_holds_resolution_for_its_lifetime() {
    let _lock = TIMER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = timer_resolution_holders();
    let client_id = u64::MAX - 7101;
    let (tx, _rx) = mpsc::channel::<WriterMsg>();

    let handle = WriterHandle::new(client_id, tx);
    let attached = timer_resolution_holders();
    drop(handle);
    let detached = timer_resolution_holders();

    assert_eq!(attached, base + 1, "an attached client connection must hold the 1ms timer");
    assert_eq!(detached, base, "detaching the last client must release the 1ms timer");
}
