//! Signalled wakeups for the keystroke/echo path.
//!
//! Each hop used to notice new work only when its timed wait expired, and on
//! Windows every such wait rounds up to the ~15.6ms scheduler tick. These
//! helpers wake the next hop as soon as there is work. The timed waits stay in
//! place as fallbacks, so a missed signal costs latency, never a deadlock.

use crate::types::{CtrlReq, PTY_DATA_READY};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Minimum spacing between server frame pushes (LAG-006): caps streaming
/// output at ~250 full JSON frames per second instead of one per Wake.
pub const MIN_FRAME_PUSH_INTERVAL: Duration = Duration::from_millis(4);

/// How long to hold a dirty frame before pushing it. ZERO after an idle gap
/// so a keystroke echo is never delayed; otherwise the rest of the interval.
pub fn frame_push_wait(last_push: Option<Instant>, now: Instant) -> Duration {
    match last_push {
        None => Duration::ZERO,
        Some(last) => MIN_FRAME_PUSH_INTERVAL.saturating_sub(now.saturating_duration_since(last)),
    }
}

static SERVER_WAKER: Mutex<Option<mpsc::Sender<CtrlReq>>> = Mutex::new(None);

/// Register the server loop's control-channel sender as the PTY-data waker.
pub fn register_server_waker(tx: mpsc::Sender<CtrlReq>) {
    if let Ok(mut w) = SERVER_WAKER.lock() {
        *w = Some(tx);
    }
}

/// Publish PTY_DATA_READY and, on its false -> true transition, send one
/// CtrlReq::Wake so the server loop's recv returns now. While the flag is
/// still set the loop has not consumed the previous wake, so no more are sent.
pub fn mark_pty_data_ready() {
    if !PTY_DATA_READY.swap(true, Ordering::AcqRel) {
        if let Ok(w) = SERVER_WAKER.lock() {
            if let Some(tx) = w.as_ref() {
                let _ = tx.send(CtrlReq::Wake);
            }
        }
    }
}

/// Message to a persistent connection's writer thread.
pub enum WriterMsg {
    /// Oneshot receiver for a command response, written in arrival order.
    Resp(mpsc::Receiver<String>),
    /// The client's frame slot went empty -> full; take and write it now.
    FrameReady,
}

static FRAME_WAKERS: Mutex<Vec<(u64, mpsc::Sender<WriterMsg>)>> = Mutex::new(Vec::new());

/// Register the writer channel that push_frame wakes for `client_id`.
pub fn register_frame_waker(client_id: u64, tx: mpsc::Sender<WriterMsg>) {
    if let Ok(mut v) = FRAME_WAKERS.lock() {
        v.retain(|(cid, _)| *cid != client_id);
        v.push((client_id, tx));
    }
}

/// Drop the registry's sender for `client_id`. The registry must not outlive
/// the connection: its Sender clone would keep the writer from ever seeing
/// Disconnected, so teardown would stall.
pub fn remove_frame_waker(client_id: u64) {
    if let Ok(mut v) = FRAME_WAKERS.lock() {
        v.retain(|(cid, _)| *cid != client_id);
    }
}

/// Send FrameReady to each listed client's writer. Send errors mean the
/// writer already exited; its timed poll never needed the signal anyway.
pub fn wake_frame_writers(client_ids: &[u64]) {
    if client_ids.is_empty() {
        return;
    }
    if let Ok(v) = FRAME_WAKERS.lock() {
        for (cid, tx) in v.iter() {
            if client_ids.contains(cid) {
                let _ = tx.send(WriterMsg::FrameReady);
            }
        }
    }
}

/// Reader-side handle to a persistent connection's writer. Registers the
/// frame waker on creation and removes it on drop, so when the reader exits
/// every Sender is gone and the writer thread sees Disconnected.
pub struct WriterHandle {
    client_id: u64,
    tx: mpsc::Sender<WriterMsg>,
}

impl WriterHandle {
    pub fn new(client_id: u64, tx: mpsc::Sender<WriterMsg>) -> Self {
        register_frame_waker(client_id, tx.clone());
        WriterHandle { client_id, tx }
    }

    pub fn send_resp(&self, rrx: mpsc::Receiver<String>) {
        let _ = self.tx.send(WriterMsg::Resp(rrx));
    }
}

impl Drop for WriterHandle {
    fn drop(&mut self) {
        remove_frame_waker(self.client_id);
    }
}

/// Whether the parser thread should run its adaptive coalescing wait for a
/// staged batch of `len` bytes. A keystroke echo is far below 256 bytes and
/// is parsed at once: the wait's 1ms sleeps round up to a ~15.6ms Windows
/// tick. Larger multi-chunk frames still coalesce into one parser update.
pub fn should_coalesce(len: usize) -> bool {
    len >= 256
}

/// should_coalesce, plus: a small batch that ends mid-escape or leaves the
/// cursor hidden is the head of a multi-chunk redraw, so wait for the rest
/// instead of snapshotting a half-applied frame. Batches past the length
/// threshold return early, so the scan only walks fewer than 256 bytes.
pub fn should_coalesce_batch(bytes: &[u8]) -> bool {
    if should_coalesce(bytes.len()) {
        return true;
    }
    #[derive(Clone, Copy, PartialEq)]
    enum St { Ground, Esc, Csi(usize), Osc, OscEsc }
    let mut st = St::Ground;
    let mut hidden = false;
    for (i, &b) in bytes.iter().enumerate() {
        st = match st {
            St::Csi(start) if (0x40..=0x7e).contains(&b) => {
                if &bytes[start..i] == b"?25" && (b == b'l' || b == b'h') {
                    hidden = b == b'l';
                }
                St::Ground
            }
            St::Csi(start) if b != 0x1b => St::Csi(start),
            St::Osc if b == 0x07 => St::Ground,
            St::Osc if b != 0x1b => St::Osc,
            St::Osc => St::OscEsc,
            St::OscEsc if b == b'\\' => St::Ground,
            St::Esc | St::OscEsc if b == b'[' => St::Csi(i + 1),
            St::Esc | St::OscEsc if b == b']' => St::Osc,
            _ if b == 0x1b => St::Esc,
            _ => St::Ground,
        };
    }
    hidden || st != St::Ground
}
