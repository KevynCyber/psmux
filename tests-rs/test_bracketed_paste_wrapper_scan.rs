// Covers: R245-002
// Requirement: the client's raw input accumulator must recognize a
// bracketed-paste wrapper (ESC[200~ ... ESC[201~) in the buffered char
// stream so a real paste is delivered as one atomic chunk instead of falling
// through to the existing arrival-timing heuristic, which misclassifies
// large/slow pastes as individual keystrokes. `scan_bracketed_paste_wrapper`
// is the pure scan step: given the accumulated buffer, it reports whether no
// opener has arrived yet (None -- the caller keeps using the timing
// heuristic unchanged), an opener arrived but the closer hasn't (Incomplete
// -- keep buffering), or both markers are present (Complete{payload, rest}
// -- payload is the text between the markers and rest is whatever trailed
// the closer, to be reprocessed normally).
#[cfg(windows)]
use super::*;

#[cfg(windows)]
#[test]
fn complete_wrapper_in_one_burst() {
    let buf = "\x1b[200~hello\x1b[201~";
    match scan_bracketed_paste_wrapper(buf) {
        BracketedPasteWrapper::Complete { payload, rest } => {
            assert_eq!(payload, "hello");
            assert_eq!(rest, "");
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

#[cfg(windows)]
#[test]
fn wrapper_split_across_two_reads() {
    // First read only has the opener plus partial payload: no closer yet.
    let first = "\x1b[200~hel";
    match scan_bracketed_paste_wrapper(first) {
        BracketedPasteWrapper::Incomplete => {}
        other => panic!("expected Incomplete, got {:?}", other),
    }

    // Second read is the accumulated buffer now including the closer.
    let second = "\x1b[200~hello\x1b[201~";
    match scan_bracketed_paste_wrapper(second) {
        BracketedPasteWrapper::Complete { payload, rest } => {
            assert_eq!(payload, "hello");
            assert_eq!(rest, "");
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

#[cfg(windows)]
#[test]
fn payload_with_embedded_newlines() {
    let buf = "\x1b[200~line one\nline two\r\nline three\x1b[201~";
    match scan_bracketed_paste_wrapper(buf) {
        BracketedPasteWrapper::Complete { payload, rest } => {
            assert_eq!(payload, "line one\nline two\r\nline three");
            assert_eq!(rest, "");
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

#[cfg(windows)]
#[test]
fn payload_containing_bare_esc_byte_not_confused_with_terminator() {
    // A bare ESC not followed by "[201~" is just pasted content, not the
    // close marker, and must not truncate the payload early.
    let buf = "\x1b[200~before\x1bxafter\x1b[201~";
    match scan_bracketed_paste_wrapper(buf) {
        BracketedPasteWrapper::Complete { payload, rest } => {
            assert_eq!(payload, "before\x1bxafter");
            assert_eq!(rest, "");
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

#[cfg(windows)]
#[test]
fn empty_payload() {
    let buf = "\x1b[200~\x1b[201~";
    match scan_bracketed_paste_wrapper(buf) {
        BracketedPasteWrapper::Complete { payload, rest } => {
            assert_eq!(payload, "");
            assert_eq!(rest, "");
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

#[cfg(windows)]
#[test]
fn unterminated_opener_stays_incomplete_never_none_or_panic() {
    let buf = "\x1b[200~some pasted text with no terminator yet";
    match scan_bracketed_paste_wrapper(buf) {
        BracketedPasteWrapper::Incomplete => {}
        other => panic!("expected Incomplete, got {:?}", other),
    }
}

#[cfg(windows)]
#[test]
fn plain_typed_burst_with_no_opener_returns_none() {
    let buf = "just a normal fast keystroke burst";
    match scan_bracketed_paste_wrapper(buf) {
        BracketedPasteWrapper::None => {}
        other => panic!("expected None, got {:?}", other),
    }
}

// Covers: R245-002
// Requirement: a user who fast-types (not pastes) while the arrival-timing
// heuristic has speculatively entered stage 2 must never lose keystrokes.
// `manage_paste_pend`'s stage-2 timeout branch (client_paste.rs ~line 224)
// flushes the accumulated buffer as `send-paste` once the 300ms deadline
// passes with no Ctrl+V Release, and arms `paste_suppress_until` for 200ms
// to block the Ctrl+V clipboard-read fallback that would otherwise
// double-deliver the same text. `src/client.rs`'s Char handler (~3608-3613)
// currently reuses that SAME deadline to gate ordinary char intake, so any
// key typed in the 200ms after a stage-2 flush is silently dropped
// ("misse^" instead of "missed"). Char accumulation must stay open across
// that window; only the clipboard-read fallback may be gated by it.
//
// `manage_paste_pend` gains a 9th param, `clipboard_fallback_suppress_until`,
// dedicated to gating the Ctrl+V clipboard-read fallback; the stage-2
// timeout, Ctrl+V-confirmed, and wrapper-Complete arms now arm THAT field
// instead of `paste_suppress_until`. This test recomputes the exact
// suppression predicate `src/client.rs:3608-3613` uses today
// (`paste_suppress_until.map_or(false, |t| Instant::now() < t)`) against the
// state `manage_paste_pend` leaves behind in `paste_suppress_until`. Today
// that predicate is `true` right after the stage-2 flush, which is the bug:
// it proves the single shared deadline blocks chars too. Once client.rs
// stops keying char intake off `paste_suppress_until` and consults the new
// clipboard-fallback-only field instead (client_paste.rs:149,182,248 and
// client.rs:4315,4334), `paste_suppress_until` itself must stay unarmed by
// the stage-2 timeout path, so this predicate must read `false`.
#[cfg(windows)]
#[test]
fn stage2_timeout_flush_must_not_leave_char_intake_blocked() {
    let mut paste_pend = "abc".to_string();
    // Already 301ms into stage 2 -> past the 300ms timeout deadline.
    let mut paste_pend_start = Some(Instant::now() - Duration::from_millis(301));
    let mut paste_stage2 = true;
    // No growth since last check -> takes the "buffer stopped growing,
    // flush as send-paste" arm, not the growth-extends-window arm.
    let mut paste_stage2_last_len = paste_pend.len();
    let mut paste_confirmed = false;
    let mut paste_suppress_until: Option<Instant> = None;
    let mut clipboard_fallback_suppress_until: Option<Instant> = None;
    let mut cmd_batch: Vec<String> = Vec::new();

    manage_paste_pend(
        false, // paste_detection_enabled: skip the bracketed-wrapper scan,
        // isolating the arrival-timing stage-2 timeout path under test.
        &mut paste_pend,
        &mut paste_pend_start,
        &mut paste_stage2,
        &mut paste_stage2_last_len,
        &mut paste_confirmed,
        &mut paste_suppress_until,
        &mut clipboard_fallback_suppress_until,
        &mut cmd_batch,
    );

    assert!(
        cmd_batch.iter().any(|c| c.starts_with("send-paste ")),
        "expected stage2 timeout to flush accumulated chars as send-paste, got {:?}",
        cmd_batch
    );

    // The bug: client.rs's Char handler treats an armed paste_suppress_until
    // as "drop this char". A typed key landing right after this flush must
    // still be accepted.
    let would_drop_next_typed_char = paste_suppress_until.map_or(false, |t| Instant::now() < t);
    assert!(
        !would_drop_next_typed_char,
        "stage2 timeout flush left paste_suppress_until armed in a way that \
         (per client.rs:3608-3613's identical predicate) would drop the next \
         typed char; client.rs needs a clipboard-fallback-only deadline \
         separate from the field char intake consults"
    );
}

// Covers: R245-002
// Requirement: the fix for the char-drop bug above must not simply delete
// the stage-2 timeout's suppression arming -- the Ctrl+V clipboard-read
// fallback (client.rs ~4320) still needs to be blocked immediately after a
// stage-2 flush, or Ctrl+V Release arriving late for a paste already sent
// via stage2 would read the clipboard again and double-deliver it. This
// locks the clipboard-fallback side of the contract so a fix that widens
// char intake cannot regress it by removing the deadline outright. The
// stage-2 timeout arm sets this deadline on the NEW
// `clipboard_fallback_suppress_until` param (9th arg to `manage_paste_pend`,
// added right after `paste_suppress_until`), the field client.rs's Ctrl+V
// clipboard-read fallback consults once the fix lands; the OTHER test in
// this file pins that `paste_suppress_until` itself stays unarmed.
#[cfg(windows)]
#[test]
fn stage2_timeout_flush_still_blocks_clipboard_fallback_immediately_after() {
    let mut paste_pend = "abc".to_string();
    let mut paste_pend_start = Some(Instant::now() - Duration::from_millis(301));
    let mut paste_stage2 = true;
    let mut paste_stage2_last_len = paste_pend.len();
    let mut paste_confirmed = false;
    let mut paste_suppress_until: Option<Instant> = None;
    let mut clipboard_fallback_suppress_until: Option<Instant> = None;
    let mut cmd_batch: Vec<String> = Vec::new();

    manage_paste_pend(
        false,
        &mut paste_pend,
        &mut paste_pend_start,
        &mut paste_stage2,
        &mut paste_stage2_last_len,
        &mut paste_confirmed,
        &mut paste_suppress_until,
        &mut clipboard_fallback_suppress_until,
        &mut cmd_batch,
    );

    // Mirrors client.rs:4319-4320's clipboard-fallback suppression check,
    // now against the dedicated clipboard-fallback deadline field.
    let clipboard_fallback_suppressed =
        clipboard_fallback_suppress_until.map_or(false, |t| Instant::now() < t);
    assert!(
        clipboard_fallback_suppressed,
        "stage2 timeout flush must still block the Ctrl+V clipboard-read \
         fallback immediately afterward, else a late Ctrl+V Release for a \
         paste already sent via stage2 double-delivers it"
    );
}
