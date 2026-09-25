// Covers: LAG-008
// Requirement: the first frame pushed to a newly attached client must wake
// its writer at once. If the frame slot opens before the frame waker is
// registered, a push landing in that window fills the slot with no
// FrameReady, and every later push only overwrites (no wake), so the frame
// waits out the writer's 5ms fallback poll. The persistent-connection setup
// therefore creates the WriterHandle (which registers the waker) before
// register_frame_channel and before spawning the writer thread.
//
// Nested under test_pr267_backpressure_proof.rs so it shares BP_TEST_LOCK:
// push_frame broadcasts to every registered slot process-wide.

use crate::types::{push_frame, register_frame_channel, register_frame_waker, shutdown_client_stream, WriterMsg};
use std::sync::mpsc;

#[test]
fn waker_registered_before_slot_opens_is_woken_by_first_push() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let client_id = u64::MAX - 7201;
    shutdown_client_stream(client_id);

    let (tx, rx) = mpsc::channel::<WriterMsg>();
    register_frame_waker(client_id, tx);
    let slot = register_frame_channel(client_id);

    push_frame("first");
    let woke = rx.try_iter().filter(|m| matches!(m, WriterMsg::FrameReady)).count();
    let taken = slot.lock().expect("frame slot lock").take();
    shutdown_client_stream(client_id);

    assert_eq!(woke, 1, "waker-then-slot order leaves no unsignalled window");
    assert_eq!(taken.as_deref(), Some("first"));
}

#[test]
fn persistent_setup_registers_waker_before_slot_and_writer_spawn() {
    let src = include_str!("../src/server/connection.rs");
    let start = src.find(r#"line.trim() == "PERSISTENT""#).expect("PERSISTENT setup block");
    let end = start + src[start..].find(r#"line.trim() == "CONTROL""#).expect("CONTROL block follows PERSISTENT");
    let block = &src[start..end];

    let handle = block.find("WriterHandle::new(").expect("PERSISTENT setup must create a WriterHandle");
    let slot = block.find("register_frame_channel(client_id)").expect("PERSISTENT setup opens a frame slot");
    let spawn = block.find("std::thread::spawn(").expect("PERSISTENT setup spawns the writer thread");

    assert_eq!(block.matches("WriterHandle::new(").count(), 1, "exactly one WriterHandle per persistent connection");
    assert!(handle < slot, "WriterHandle (frame waker) must be registered before the frame slot opens");
    assert!(handle < spawn, "WriterHandle (frame waker) must be registered before the writer thread spawns");
}
