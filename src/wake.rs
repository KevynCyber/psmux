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
