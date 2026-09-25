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
// heuristic has speculatively entered stage 2 must never lose keystrokes,
// and the buffered chars must still be delivered once the 300ms stage-2
// deadline passes with no Ctrl+V Release. `manage_paste_pend`'s stage-2
// timeout arm flushes the accumulated buffer as `send-paste`. The earlier
// char-drop bug (the flush armed the same `paste_suppress_until` deadline
// that client.rs's Char handler uses to drop chars) is now ruled out by the
// signature itself: `manage_paste_pend` no longer receives the char-intake
// deadline at all, so it cannot arm it. Only the clipboard-fallback-only
// deadline is passed in (pinned by the next test).
#[cfg(windows)]
#[test]
fn stage2_timeout_flush_emits_buffered_chars_as_send_paste() {
    let mut paste_pend = "abc".to_string();
    // Already 301ms into stage 2 -> past the 300ms timeout deadline.
    let mut paste_pend_start = Some(Instant::now() - Duration::from_millis(301));
    let mut paste_stage2 = true;
    // No growth since last check -> takes the "buffer stopped growing,
    // flush as send-paste" arm, not the growth-extends-window arm.
    let mut paste_stage2_last_len = paste_pend.len();
    let mut paste_confirmed = false;
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
        &mut clipboard_fallback_suppress_until,
        &mut cmd_batch,
    );

    let expected = format!("send-paste {}
", base64_encode("abc"));
    assert!(
        cmd_batch.iter().any(|c| c == &expected),
        "expected stage2 timeout to flush the buffered \"abc\" as {:?}, got {:?}",
        expected,
        cmd_batch
    );
    assert!(
        paste_pend.is_empty(),
        "stage2 timeout flush must drain the paste buffer, left {:?}",
        paste_pend
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
// stage-2 timeout arm sets this deadline on
// `clipboard_fallback_suppress_until`, the only suppression deadline
// `manage_paste_pend` receives and the field client.rs's Ctrl+V
// clipboard-read fallback consults.
#[cfg(windows)]
#[test]
fn stage2_timeout_flush_still_blocks_clipboard_fallback_immediately_after() {
    let mut paste_pend = "abc".to_string();
    let mut paste_pend_start = Some(Instant::now() - Duration::from_millis(301));
    let mut paste_stage2 = true;
    let mut paste_stage2_last_len = paste_pend.len();
    let mut paste_confirmed = false;
    let mut clipboard_fallback_suppress_until: Option<Instant> = None;
    let mut cmd_batch: Vec<String> = Vec::new();

    manage_paste_pend(
        false,
        &mut paste_pend,
        &mut paste_pend_start,
        &mut paste_stage2,
        &mut paste_stage2_last_len,
        &mut paste_confirmed,
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

// Covers: R254-015
// Requirement: a bare ESC (e.g. vim's Escape) followed by fast-typed
// ordinary characters (":wq") must never surface inside a `send-paste`
// payload. `route_esc_key`/`should_buffer_esc_for_paste` buffer a bare ESC
// into `paste_pend` whenever the buffer is empty, so a bracketed-paste
// opener (ESC [ 2 0 0 ~) can be recognized as it streams in. If the user
// then types ordinary chars fast enough to trip the 20ms/3-char heuristic,
// `paste_pend` becomes "\x1b:wq": `scan_bracketed_paste_wrapper` returns
// None (no full opener), stage 2 is entered, and today the stage-2 timeout
// arm in `manage_paste_pend` (client_paste.rs ~248) flushes the WHOLE
// buffer as one `send-paste`, whose base64 payload still contains the raw
// ESC byte -- `strip_prefix(BRACKETED_PASTE_OPEN)` only strips a full
// opener, not a lone leading ESC. The fix: once the buffer starts with ESC
// but is neither a prefix of nor equal to a real opener, the leading ESC
// must be flushed on its own as `send-key esc\n` (mirroring
// `push_char_as_key`'s bare-ESC handling), and only the typed text that
// follows may go out as `send-paste`.
#[cfg(windows)]
#[test]
fn bare_esc_then_fast_typed_chars_flushes_esc_as_key_not_inside_send_paste() {
    let mut paste_pend = "\x1b:wq".to_string();
    let mut paste_pend_start = Some(Instant::now() - Duration::from_millis(400));
    let mut paste_stage2 = true;
    let mut paste_stage2_last_len = paste_pend.len();
    let mut paste_confirmed = false;
    let mut clipboard_fallback_suppress_until: Option<Instant> = None;
    let mut cmd_batch: Vec<String> = Vec::new();

    manage_paste_pend(
        true, // paste_detection_enabled: exercise the real wrapper scan,
        // since it's the scan's None verdict on "\x1b:wq" that lets this
        // buffer reach the stage-2 timeout arm in the first place.
        &mut paste_pend,
        &mut paste_pend_start,
        &mut paste_stage2,
        &mut paste_stage2_last_len,
        &mut paste_confirmed,
        &mut clipboard_fallback_suppress_until,
        &mut cmd_batch,
    );

    let esc_key_count = cmd_batch.iter().filter(|c| c.as_str() == "send-key esc\n").count();
    assert_eq!(
        esc_key_count, 1,
        "expected the leading bare ESC to be flushed exactly once as \
         send-key esc, got {:?}",
        cmd_batch
    );

    for entry in cmd_batch.iter().filter(|c| c.starts_with("send-paste ")) {
        let encoded = entry
            .strip_prefix("send-paste ")
            .and_then(|s| s.strip_suffix('\n'))
            .expect("send-paste entry must be well-formed");
        let decoded = crate::util::base64_decode(encoded)
            .expect("send-paste payload must be valid base64 utf8");
        assert!(
            !decoded.contains('\x1b'),
            "send-paste payload must never contain the raw ESC byte unless \
             the buffer held a real bracketed-paste opener, got {:?} \
             (decoded: {:?})",
            entry,
            decoded
        );
    }

    let expected_text_paste = format!("send-paste {}\n", base64_encode(":wq"));
    assert!(
        cmd_batch.iter().any(|c| c == &expected_text_paste),
        "expected the typed \":wq\" to be flushed as its own send-paste \
         ({:?}), got {:?}",
        expected_text_paste,
        cmd_batch
    );
}

#[cfg(windows)]
#[path = "test_typing_lag_console_frame_event.rs"]
mod typing_lag_console_frame_event;
