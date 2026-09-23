//! Windows paste-detection helpers for the client input loop: the pure
//! bracketed-paste wrapper scan, the ESC-routing guard it needs, and the
//! pending-buffer flush paths that share the per-char keystroke mapping.

use std::time::{Duration, Instant};

use crate::debug_log::{input_log, input_log_enabled};
use crate::term::event::KeyModifiers;
use crate::util::base64_encode;

/// Whether a mouse-drag selection is worth copying to the system clipboard.
///
/// A drag across blank prompt rows yields newline/whitespace-only text; if
/// that gets copied, it silently clobbers whatever the user had on the
/// clipboard, and the next paste sends N blank lines into the pane instead
/// (PSMUX-PASTE-WS-001).
pub(crate) fn selection_worth_copying(text: &str) -> bool {
    !text.trim().is_empty()
}

pub(crate) const BRACKETED_PASTE_OPEN: &str = "\x1b[200~";
const BRACKETED_PASTE_CLOSE: &str = "\x1b[201~";

/// Result of scanning the raw input accumulator for a bracketed-paste wrapper.
#[derive(Debug)]
pub(crate) enum BracketedPasteWrapper {
    /// No opener yet - the caller keeps using the arrival-timing heuristic.
    None,
    /// Opener seen, closer still in flight - keep buffering.
    Incomplete,
    /// Both markers present: `payload` is the pasted text, `rest` is whatever
    /// trailed the closer and must be reprocessed normally.
    Complete { payload: String, rest: String },
}

/// Pure scan step of bracketed-paste detection.  The markers are proof of a
/// paste, where the arrival-timing heuristic can only guess and misclassifies
/// large or slow pastes as individual keystrokes.
pub(crate) fn scan_bracketed_paste_wrapper(buf: &str) -> BracketedPasteWrapper {
    let open_at = match buf.find(BRACKETED_PASTE_OPEN) {
        Some(i) => i,
        None => return BracketedPasteWrapper::None,
    };
    let body = &buf[open_at + BRACKETED_PASTE_OPEN.len()..];
    // Match the closer as a whole string: a bare ESC in the payload is pasted
    // content, not a terminator, and must not truncate the payload early.
    match body.find(BRACKETED_PASTE_CLOSE) {
        Some(close_at) => BracketedPasteWrapper::Complete {
            payload: body[..close_at].to_string(),
            rest: body[close_at + BRACKETED_PASTE_CLOSE.len()..].to_string(),
        },
        None => BracketedPasteWrapper::Incomplete,
    }
}

/// A bracketed-paste opener starts with a literal ESC, so a bare ESC has to be
/// able to enter the accumulator for the scan above to ever see a wrapper.
/// Guarded so ordinary ESC handling is unaffected: only an unmodified ESC, only
/// while paste detection is on, and only when it could open a wrapper (nothing
/// pending) or continue one already pending.  A buffered ESC that turns out not
/// to belong to a wrapper is flushed as `send-key esc`, not as text.
pub(crate) fn should_buffer_esc_for_paste(
    modifiers: KeyModifiers,
    paste_detection_enabled: bool,
    paste_pend: &str,
) -> bool {
    if !paste_detection_enabled || !modifiers.is_empty() {
        return false;
    }
    paste_pend.is_empty()
        || matches!(
            scan_bracketed_paste_wrapper(paste_pend),
            BracketedPasteWrapper::Incomplete
        )
}

/// Emit one pending char as the keystroke command the pane expects.
fn push_char_as_key(c: char, cmd_batch: &mut Vec<String>) {
    match c {
        '\n' => cmd_batch.push("send-key enter\n".into()),
        '\t' => cmd_batch.push("send-key tab\n".into()),
        ' ' => cmd_batch.push("send-key space\n".into()),
        // A buffered ESC that never became a paste opener is still an ESC.
        '\x1b' => cmd_batch.push("send-key esc\n".into()),
        _ => {
            let escaped = match c {
                '"' => "\\\"".to_string(),
                '\\' => "\\\\".to_string(),
                _ => c.to_string(),
            };
            cmd_batch.push(format!("send-text \"{}\"\n", escaped));
        }
    }
}

/// Flush the pending buffer as individual keystrokes and reset it.
pub(crate) fn flush_pend_as_keystrokes(
    paste_pend: &mut String,
    paste_pend_start: &mut Option<Instant>,
    cmd_batch: &mut Vec<String>,
) {
    for c in paste_pend.chars() {
        push_char_as_key(c, cmd_batch);
    }
    paste_pend.clear();
    *paste_pend_start = None;
}

/// Route a bare ESC key press: into the paste accumulator when it could belong
/// to a bracketed-paste wrapper, otherwise straight out as `send-key esc`.
pub(crate) fn route_esc_key(
    modifiers: KeyModifiers,
    paste_detection_enabled: bool,
    paste_pend: &mut String,
    paste_pend_start: &mut Option<Instant>,
    cmd_batch: &mut Vec<String>,
) {
    if should_buffer_esc_for_paste(modifiers, paste_detection_enabled, paste_pend) {
        paste_pend.push('\x1b');
        if paste_pend_start.is_none() {
            *paste_pend_start = Some(Instant::now());
        }
    } else {
        cmd_batch.push("send-key esc\n".into());
    }
}

/// Flush or promote the pending paste buffer based on how long its chars have
/// been buffered, and on whether an explicit bracketed-paste wrapper arrived.
#[allow(clippy::too_many_arguments)]
pub(crate) fn manage_paste_pend(
    paste_detection_enabled: bool,
    paste_pend: &mut String,
    paste_pend_start: &mut Option<Instant>,
    paste_stage2: &mut bool,
    paste_stage2_last_len: &mut usize,
    paste_confirmed: &mut bool,
    // No longer read here -- kept as a parameter only so callers (client.rs's
    // Char-intake arm) keep exclusive ownership of the char-intake deadline;
    // this function now arms only the clipboard-fallback-only field below.
    _paste_suppress_until: &mut Option<Instant>,
    clipboard_fallback_suppress_until: &mut Option<Instant>,
    cmd_batch: &mut Vec<String>,
) {
    let start = match *paste_pend_start {
        Some(s) => s,
        None => return,
    };
    let elapsed = start.elapsed();

    // A buffered ESC that can no longer become a bracketed-paste opener
    // (neither a prefix of BRACKETED_PASTE_OPEN nor a full opener) must be
    // flushed on its own as send-key esc, mirroring push_char_as_key's
    // bare-ESC handling -- otherwise it rides along inside whatever
    // send-paste the rest of this function emits.
    if paste_pend.starts_with('\x1b')
        && !BRACKETED_PASTE_OPEN.starts_with(paste_pend.as_str())
        && !paste_pend.starts_with(BRACKETED_PASTE_OPEN)
    {
        if input_log_enabled() {
            input_log("paste", "buffered ESC can no longer open a paste wrapper, flushing as send-key esc");
        }
        cmd_batch.push("send-key esc\n".to_string());
        paste_pend.remove(0);
        if paste_pend.is_empty() {
            *paste_pend_start = None;
            *paste_stage2 = false;
            *paste_stage2_last_len = 0;
            *paste_confirmed = false;
            return;
        }
    }

    // An explicit wrapper takes precedence over the arrival-timing heuristic:
    // the markers are proof of a paste, where timing can only guess and
    // misclassifies large/slow pastes as individual keystrokes.
    if paste_detection_enabled {
        match scan_bracketed_paste_wrapper(paste_pend) {
            BracketedPasteWrapper::Complete { payload, rest } => {
                if input_log_enabled() {
                    input_log("paste", &format!("bracketed wrapper complete, {} chars as send-paste", payload.len()));
                }
                if !payload.is_empty() {
                    cmd_batch.push(format!("send-paste {}\n", base64_encode(&payload)));
                    // Suppress the clipboard-read fallback only -- char intake
                    // (paste_suppress_until) must stay open.
                    *clipboard_fallback_suppress_until = Some(Instant::now() + Duration::from_millis(200));
                }
                *paste_pend = rest;
                *paste_pend_start = if paste_pend.is_empty() { None } else { Some(Instant::now()) };
                *paste_stage2 = false;
                *paste_stage2_last_len = 0;
                *paste_confirmed = false;
                return;
            }
            BracketedPasteWrapper::Incomplete => {
                if !*paste_stage2 {
                    *paste_stage2 = true;
                    *paste_stage2_last_len = paste_pend.len();
                }
                // Never swallow an Incomplete: once past the stage-2 deadline
                // fall through to the timeout path below, which flushes it.
                if elapsed <= Duration::from_millis(300) {
                    return;
                }
            }
            BracketedPasteWrapper::None => {}
        }
    }

    if *paste_confirmed {
        // Ctrl+V Release already seen - send as paste now.
        if !paste_pend.is_empty() {
            if input_log_enabled() {
                input_log("paste", &format!("paste CONFIRMED (top), sending {} chars as send-paste: {:?}",
                    paste_pend.len(), &paste_pend.chars().take(200).collect::<String>()));
            }
            cmd_batch.push(format!("send-paste {}\n", base64_encode(paste_pend)));
            // Suppress clipboard-read fallback only -- char intake
            // (paste_suppress_until) must stay open.
            *clipboard_fallback_suppress_until = Some(Instant::now() + Duration::from_millis(200));
        }
        paste_pend.clear();
        *paste_pend_start = None;
        *paste_stage2 = false;
        *paste_confirmed = false;
    } else if !*paste_stage2 && elapsed > Duration::from_millis(20) {
        // 20ms window expired.
        let has_non_ascii = paste_pend.chars().any(|c| !c.is_ascii());
        if paste_pend.len() >= 3 && !has_non_ascii {
            // 3+ ASCII chars in 20ms -> likely paste, enter stage 2.
            // Non-ASCII chars (IME composition, CJK input) are excluded
            // because IME routinely generates 3+ chars in <20ms and would
            // trigger a false-positive 300ms delay (fixes #91).
            *paste_stage2 = true;
            *paste_stage2_last_len = paste_pend.len();
            if input_log_enabled() {
                input_log("paste", &format!("stage2: {} chars in 20ms, waiting for Ctrl+V Release", paste_pend.len()));
            }
        } else if paste_pend.len() >= 20 && has_non_ascii {
            // 20+ non-ASCII chars in 20ms - almost certainly a paste
            // containing Unicode content (em-dashes, CJK, etc.), not
            // IME composition (which rarely exceeds a few chars).
            *paste_stage2 = true;
            *paste_stage2_last_len = paste_pend.len();
            if input_log_enabled() {
                input_log("paste", &format!("stage2 (large non-ASCII): {} chars in 20ms", paste_pend.len()));
            }
        } else if paste_pend.len() >= 3 && has_non_ascii {
            // 3+ chars but contains non-ASCII (IME input) - flush
            // immediately as normal text to avoid the 300ms delay.
            if input_log_enabled() {
                input_log("paste", &format!("flush {} chars as normal (non-ASCII / IME detected)", paste_pend.len()));
            }
            flush_pend_as_keystrokes(paste_pend, paste_pend_start, cmd_batch);
        } else {
            // <3 chars -> normal typing, flush as send-text.
            if input_log_enabled() {
                input_log("paste", &format!("flush {} chars as normal (< 3 in 20ms)", paste_pend.len()));
            }
            flush_pend_as_keystrokes(paste_pend, paste_pend_start, cmd_batch);
        }
    } else if *paste_stage2 && elapsed > Duration::from_millis(300) {
        // Stage 2 timeout - no Ctrl+V Release arrived.
        // Growth detection: if the buffer grew since the last check, ConPTY
        // is still injecting characters (large paste).  Extend the window
        // instead of splitting the paste.
        if paste_pend.len() > *paste_stage2_last_len {
            *paste_stage2_last_len = paste_pend.len();
            *paste_pend_start = Some(Instant::now() - Duration::from_millis(280));
        } else {
            // Buffer stopped growing - send accumulated chars as send-paste
            // so the server wraps them in bracketed paste.  An unterminated
            // wrapper lands here too; drop its literal opener so the escape
            // sequence never reaches the pane.
            if input_log_enabled() {
                input_log("paste", &format!("stage2 timeout, sending {} chars as send-paste", paste_pend.len()));
            }
            let payload = paste_pend.strip_prefix(BRACKETED_PASTE_OPEN).unwrap_or(paste_pend);
            cmd_batch.push(format!("send-paste {}\n", base64_encode(payload)));
            paste_pend.clear();
            *paste_pend_start = None;
            *paste_stage2 = false;
            *paste_stage2_last_len = 0;
            // Suppress the clipboard-read fallback that fires when Ctrl+V
            // Release arrives later (the paste was already sent via stage2).
            // paste_suppress_until stays open so char intake keeps accepting
            // keys typed right after this flush.
            *clipboard_fallback_suppress_until = Some(Instant::now() + Duration::from_millis(200));
        }
    }
}

/// Flush the paste-pending buffer as individual send-text / send-key commands.
/// Called when a non-bufferable key (Backspace, Delete, BackTab) interrupts a
/// potential paste burst, so we emit whatever we had as normal keystrokes.
pub(crate) fn flush_paste_pend_as_text(
    paste_pend: &mut String,
    paste_pend_start: &mut Option<Instant>,
    paste_stage2: &mut bool,
    cmd_batch: &mut Vec<String>,
) {
    if paste_pend.is_empty() {
        return;
    }
    // If we accumulated enough ASCII chars that stage2 was entered, this
    // is almost certainly pasted content - send as send-paste so the server
    // wraps it in bracketed paste sequences (fixes nvim autoindent).
    // Non-ASCII buffers (IME input) are always flushed as normal text to
    // avoid the 300ms delay (fixes #91).
    let has_non_ascii = paste_pend.chars().any(|c| !c.is_ascii());
    if (*paste_stage2 || paste_pend.len() >= 3) && !has_non_ascii {
        let encoded = base64_encode(paste_pend);
        cmd_batch.push(format!("send-paste {}\n", encoded));
    } else {
        for c in paste_pend.chars() {
            push_char_as_key(c, cmd_batch);
        }
    }
    paste_pend.clear();
    *paste_pend_start = None;
    *paste_stage2 = false;
}
