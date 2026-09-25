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
