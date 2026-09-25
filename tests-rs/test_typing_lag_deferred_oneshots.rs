// Covers: LAG-009
// Requirement: one-shot frame events (the audible bell and OSC 52 clipboard
// data) must reach every attached client even when the LAG-006 rate limit
// defers the frame push. A dump-state reply built inside the 4ms push window
// consumes the one-shot flags; the deferred push that goes out at the
// deadline must still carry them, or the other attached clients never ring
// the bell or receive the clipboard copy. Each event is delivered once: a
// later push must not repeat it.
//
// Seam: crate::server::frame_push::FramePusher owns the last push instant and
// any one-shot events held for a deferred push. reply() is the DumpState
// path, push() is the bottom-of-loop push path.
//
// Nested under test_pr267_backpressure_proof.rs so it shares BP_TEST_LOCK:
// push_frame broadcasts to every registered slot process-wide. No real time
// passes: every instant is derived from one Instant::now() base.

use crate::server::frame_push::FramePusher;
use crate::types::{register_frame_channel, shutdown_client_stream, AppState, FrameSlot};
use crate::wake::MIN_FRAME_PUSH_INTERVAL;
use std::time::{Duration, Instant};

const BELL: &str = r#""bell":true"#;

fn clip_field(text: &str) -> String {
    format!(r#""clipboard_osc52":"{}""#, crate::util::base64_encode(text))
}

fn take(slot: &FrameSlot) -> Option<String> {
    slot.lock().expect("frame slot lock").take()
}

struct TwoClients {
    ids: [u64; 2],
    slots: [FrameSlot; 2],
}

impl TwoClients {
    fn attach(base_id: u64) -> Self {
        let ids = [base_id, base_id - 1];
        for id in ids {
            shutdown_client_stream(id);
        }
        let slots = [register_frame_channel(ids[0]), register_frame_channel(ids[1])];
        TwoClients { ids, slots }
    }

    fn take_both(&self) -> [Option<String>; 2] {
        [take(&self.slots[0]), take(&self.slots[1])]
    }
}

impl Drop for TwoClients {
    fn drop(&mut self) {
        for id in self.ids {
            shutdown_client_stream(id);
        }
    }
}

/// Push one plain frame at `t0` so the next reply lands inside the window,
/// then empty both slots.
fn prime(pusher: &mut FramePusher, app: &mut AppState, clients: &TwoClients, t0: Instant) {
    pusher.push(app, r#"{"f":0}"#, t0);
    clients.take_both();
}

#[test]
fn bell_consumed_by_reply_inside_window_reaches_both_clients_on_deferred_push() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let clients = TwoClients::attach(u64::MAX - 9101);
    let mut app = AppState::new("lag009".to_string());
    let mut pusher = FramePusher::new();
    let t0 = Instant::now();
    prime(&mut pusher, &mut app, &clients, t0);

    app.bell_forward = true;
    let reply = pusher.reply(&mut app, r#"{"f":1}"#, t0 + Duration::from_millis(1));
    assert!(reply.contains(BELL), "requesting client's reply carries the bell: {reply}");
    assert!(!app.bell_forward, "the reply consumes the one-shot bell flag");
    assert_eq!(clients.take_both(), [None, None], "inside the 4ms window the push is deferred");

    pusher.push(&mut app, r#"{"f":2}"#, t0 + MIN_FRAME_PUSH_INTERVAL);
    let [a, b] = clients.take_both();
    let (a, b) = (a.expect("client A gets the deferred frame"), b.expect("client B gets the deferred frame"));
    assert!(a.contains(BELL), "deferred push lost the bell for client A: {a}");
    assert!(b.contains(BELL), "deferred push lost the bell for client B: {b}");
    assert!(a.contains(r#""f":2"#), "deferred push carries the new frame body: {a}");
}

#[test]
fn clipboard_consumed_by_reply_inside_window_reaches_both_clients_on_deferred_push() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let clients = TwoClients::attach(u64::MAX - 9111);
    let mut app = AppState::new("lag009".to_string());
    let mut pusher = FramePusher::new();
    let t0 = Instant::now();
    prime(&mut pusher, &mut app, &clients, t0);

    app.clipboard_osc52 = Some("copied text".to_string());
    let reply = pusher.reply(&mut app, r#"{"f":1}"#, t0 + Duration::from_millis(2));
    let want = clip_field("copied text");
    assert!(reply.contains(&want), "requesting client's reply carries the clipboard: {reply}");
    assert!(app.clipboard_osc52.is_none(), "the reply consumes the one-shot clipboard slot");
    assert_eq!(clients.take_both(), [None, None], "inside the 4ms window the push is deferred");

    pusher.push(&mut app, r#"{"f":2}"#, t0 + MIN_FRAME_PUSH_INTERVAL);
    let [a, b] = clients.take_both();
    let (a, b) = (a.expect("client A gets the deferred frame"), b.expect("client B gets the deferred frame"));
    assert!(a.contains(&want), "deferred push lost the clipboard for client A: {a}");
    assert!(b.contains(&want), "deferred push lost the clipboard for client B: {b}");
}

#[test]
fn held_events_are_delivered_once_not_repeated_by_later_pushes() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let clients = TwoClients::attach(u64::MAX - 9121);
    let mut app = AppState::new("lag009".to_string());
    let mut pusher = FramePusher::new();
    let t0 = Instant::now();
    prime(&mut pusher, &mut app, &clients, t0);

    app.bell_forward = true;
    app.clipboard_osc52 = Some("once".to_string());
    pusher.reply(&mut app, r#"{"f":1}"#, t0 + Duration::from_millis(1));
    pusher.push(&mut app, r#"{"f":2}"#, t0 + MIN_FRAME_PUSH_INTERVAL);
    clients.take_both();

    pusher.push(&mut app, r#"{"f":3}"#, t0 + MIN_FRAME_PUSH_INTERVAL * 2);
    let [a, _] = clients.take_both();
    let a = a.expect("client A gets the next frame");
    assert!(!a.contains(BELL), "bell must not repeat after delivery: {a}");
    assert!(!a.contains("clipboard_osc52"), "clipboard must not repeat after delivery: {a}");
}

#[test]
fn reply_outside_window_pushes_events_to_both_clients_immediately_and_once() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let clients = TwoClients::attach(u64::MAX - 9131);
    let mut app = AppState::new("lag009".to_string());
    let mut pusher = FramePusher::new();
    let t0 = Instant::now();

    app.bell_forward = true;
    let reply = pusher.reply(&mut app, r#"{"f":1}"#, t0);
    assert!(reply.contains(BELL));
    let [a, b] = clients.take_both();
    assert_eq!(a.as_deref(), Some(reply.as_str()), "first reply is pushed at once to client A");
    assert_eq!(b.as_deref(), Some(reply.as_str()), "first reply is pushed at once to client B");

    pusher.push(&mut app, r#"{"f":2}"#, t0 + MIN_FRAME_PUSH_INTERVAL);
    let [a, _] = clients.take_both();
    let a = a.expect("client A gets the next frame");
    assert!(!a.contains(BELL), "an event already pushed must not repeat: {a}");
}

#[test]
fn held_bell_and_fresh_clipboard_both_ride_the_deferred_push() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let clients = TwoClients::attach(u64::MAX - 9141);
    let mut app = AppState::new("lag009".to_string());
    let mut pusher = FramePusher::new();
    let t0 = Instant::now();
    prime(&mut pusher, &mut app, &clients, t0);

    app.bell_forward = true;
    pusher.reply(&mut app, r#"{"f":1}"#, t0 + Duration::from_millis(1));
    app.clipboard_osc52 = Some("later".to_string());
    pusher.push(&mut app, r#"{"f":2}"#, t0 + MIN_FRAME_PUSH_INTERVAL);
    let [a, b] = clients.take_both();
    for frame in [a.expect("client A frame"), b.expect("client B frame")] {
        assert!(frame.contains(BELL), "held bell missing: {frame}");
        assert!(frame.contains(&clip_field("later")), "fresh clipboard missing: {frame}");
    }
    assert!(app.clipboard_osc52.is_none() && !app.bell_forward);
}

#[test]
fn frames_without_events_pass_through_unchanged() {
    let _guard = super::BP_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let clients = TwoClients::attach(u64::MAX - 9151);
    let mut app = AppState::new("lag009".to_string());
    let mut pusher = FramePusher::new();
    let t0 = Instant::now();

    assert_eq!(pusher.reply(&mut app, r#"{"f":1}"#, t0), r#"{"f":1}"#);
    clients.take_both();
    pusher.push(&mut app, r#"{"f":2}"#, t0 + MIN_FRAME_PUSH_INTERVAL);
    let [a, b] = clients.take_both();
    assert_eq!(a.as_deref(), Some(r#"{"f":2}"#));
    assert_eq!(b.as_deref(), Some(r#"{"f":2}"#));
}

#[test]
fn server_loop_takes_one_shot_events_only_through_frame_pusher() {
    let src = include_str!("../src/server/mod.rs");
    assert_eq!(src.matches("clipboard_osc52.take()").count(), 0, "src/server/mod.rs must leave clipboard_osc52 to FramePusher");
    assert_eq!(src.matches("bell_forward = false").count(), 0, "src/server/mod.rs must leave bell_forward to FramePusher");
    assert!(src.matches("FramePusher").count() >= 1, "the server loop must own a FramePusher");
}
