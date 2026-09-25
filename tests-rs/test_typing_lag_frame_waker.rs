// Covers: LAG-002
// Requirement: a freshly pushed frame wakes the client's connection writer
// immediately instead of waiting out the writer's timed poll (a ~15.6ms
// Windows tick per keystroke echo). push_frame sends exactly one
// WriterMsg::FrameReady to the client's registered waker when its frame
// slot goes empty -> full; overwriting an undrained frame sends nothing, so
// a burst of frames never floods the writer channel.
//
// Nested under test_pr267_backpressure_proof.rs so it shares BP_TEST_LOCK:
// push_frame broadcasts to every registered slot process-wide.

use crate::types::{
    push_frame, register_frame_channel, register_frame_waker, shutdown_client_stream, WriterMsg,
};
use std::sync::mpsc;

fn frame_ready_count(rx: &mpsc::Receiver<WriterMsg>) -> usize {
    rx.try_iter().filter(|m| matches!(m, WriterMsg::FrameReady)).count()
}

#[test]
fn push_into_empty_slot_sends_exactly_one_frame_ready() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let client_id = u64::MAX - 7001;
    shutdown_client_stream(client_id);

    let slot = register_frame_channel(client_id);
    let (tx, rx) = mpsc::channel::<WriterMsg>();
    register_frame_waker(client_id, tx);

    push_frame("frame-1");
    let woke = frame_ready_count(&rx);
    let taken = slot.lock().expect("frame slot lock").take();
    shutdown_client_stream(client_id);

    assert_eq!(woke, 1, "empty -> full slot transition must send exactly one FrameReady");
    assert_eq!(taken.as_deref(), Some("frame-1"));
}

#[test]
fn push_over_undrained_frame_sends_no_extra_frame_ready() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let client_id = u64::MAX - 7002;
    shutdown_client_stream(client_id);

    let slot = register_frame_channel(client_id);
    let (tx, rx) = mpsc::channel::<WriterMsg>();
    register_frame_waker(client_id, tx);

    push_frame("frame-1");
    push_frame("frame-2");
    push_frame("frame-3");
    let woke = frame_ready_count(&rx);
    let taken = slot.lock().expect("frame slot lock").take();
    shutdown_client_stream(client_id);

    assert_eq!(woke, 1, "overwriting an undrained frame must not send another FrameReady");
    assert_eq!(taken.as_deref(), Some("frame-3"), "slot still holds only the newest frame");
}

#[test]
fn push_after_writer_drains_slot_sends_frame_ready_again() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let client_id = u64::MAX - 7003;
    shutdown_client_stream(client_id);

    let slot = register_frame_channel(client_id);
    let (tx, rx) = mpsc::channel::<WriterMsg>();
    register_frame_waker(client_id, tx);

    push_frame("frame-1");
    let first_wake = frame_ready_count(&rx);
    let _ = slot.lock().expect("frame slot lock").take();
    push_frame("frame-2");
    let second_wake = frame_ready_count(&rx);
    let taken = slot.lock().expect("frame slot lock").take();
    shutdown_client_stream(client_id);

    assert_eq!(first_wake, 1);
    assert_eq!(second_wake, 1, "a drained slot refilling is a new empty -> full transition");
    assert_eq!(taken.as_deref(), Some("frame-2"));
}

#[test]
fn push_wakes_only_the_client_whose_slot_filled() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let full_id = u64::MAX - 7004;
    let empty_id = u64::MAX - 7005;
    shutdown_client_stream(full_id);
    shutdown_client_stream(empty_id);

    // full_id already holds an undrained frame; empty_id's slot is empty.
    let _full_slot = register_frame_channel(full_id);
    push_frame("pre-existing");
    let (full_tx, full_rx) = mpsc::channel::<WriterMsg>();
    register_frame_waker(full_id, full_tx);
    let _empty_slot = register_frame_channel(empty_id);
    let (empty_tx, empty_rx) = mpsc::channel::<WriterMsg>();
    register_frame_waker(empty_id, empty_tx);

    push_frame("next");
    let full_woke = frame_ready_count(&full_rx);
    let empty_woke = frame_ready_count(&empty_rx);
    shutdown_client_stream(full_id);
    shutdown_client_stream(empty_id);

    assert_eq!(full_woke, 0, "slot was already full: overwrite, no wake");
    assert_eq!(empty_woke, 1, "slot was empty: exactly one wake");
}
