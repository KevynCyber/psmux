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
