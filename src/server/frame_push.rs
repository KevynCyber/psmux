//! Rate-limited frame pushes that never drop one-shot events (LAG-006, LAG-009).
//!
//! The bell and OSC 52 clipboard are one-shot: taking them from AppState
//! consumes them. A dump-state reply built inside the push interval serves
//! only the requesting client, so the events it took are held here and
//! spliced into the next push that reaches every client.

use crate::types::AppState;
use std::time::{Duration, Instant};

#[derive(Default)]
struct OneShots {
    bell: bool,
    clipboard: Option<String>,
}

impl OneShots {
    fn take_from(app: &mut AppState) -> Self {
        OneShots {
            bell: std::mem::take(&mut app.bell_forward),
            clipboard: app.clipboard_osc52.take(),
        }
    }

    fn is_empty(&self) -> bool {
        !self.bell && self.clipboard.is_none()
    }

    /// Fold `newer` in; a newer clipboard copy supersedes an older one.
    fn merge(&mut self, newer: OneShots) {
        self.bell |= newer.bell;
        if newer.clipboard.is_some() {
            self.clipboard = newer.clipboard;
        }
    }

    fn splice(&self, frame: &str) -> String {
        if self.is_empty() || !frame.ends_with('}') {
            return frame.to_string();
        }
        let mut out = String::with_capacity(frame.len() + 64);
        out.push_str(&frame[..frame.len() - 1]);
        if let Some(text) = &self.clipboard {
            out.push_str(",\"clipboard_osc52\":\"");
            out.push_str(&crate::util::base64_encode(text));
            out.push('"');
        }
        if self.bell {
            out.push_str(",\"bell\":true");
        }
        out.push('}');
        out
    }
}

pub(crate) struct FramePusher {
    last_push: Option<Instant>,
    held: OneShots,
}

impl FramePusher {
    pub(crate) fn new() -> Self {
        FramePusher { last_push: None, held: OneShots::default() }
    }

    /// Time until a push is allowed; ZERO when one may go out now.
    pub(crate) fn wait(&self, now: Instant) -> Duration {
        crate::wake::frame_push_wait(self.last_push, now)
    }

    /// Dump-state reply for the requesting client. Pushed to every client when
    /// outside the push interval; otherwise its events are held for push().
    pub(crate) fn reply(&mut self, app: &mut AppState, frame: &str, now: Instant) -> String {
        let fresh = OneShots::take_from(app);
        let reply = fresh.splice(frame);
        if self.wait(now).is_zero() {
            if self.held.is_empty() {
                crate::types::push_frame(&reply);
            } else {
                // Held events have not reached the other clients yet.
                let mut all = std::mem::take(&mut self.held);
                all.merge(fresh);
                crate::types::push_frame(&all.splice(frame));
            }
            self.last_push = Some(now);
        } else {
            self.held.merge(fresh);
        }
        reply
    }

    /// Push `frame` to every client with all held and pending one-shot events.
    pub(crate) fn push(&mut self, app: &mut AppState, frame: &str, now: Instant) {
        let mut all = std::mem::take(&mut self.held);
        all.merge(OneShots::take_from(app));
        crate::types::push_frame(&all.splice(frame));
        self.last_push = Some(now);
    }
}
