// Covers: ZDEP-007
// The `base64` crate is gone (ZDEP-007): psmux uses base64 in two places --
// cross_session_server.rs ships full VT screen state between servers as raw
// bytes, and util.rs has the hand-rolled OSC 52 / send-paste string codec.
// Both now go through byte-level codec functions in util.rs:
//   base64_encode_bytes(&[u8]) -> String
//   base64_decode_bytes(&str) -> Option<Vec<u8>>
// The existing string-variant functions (base64_encode/base64_decode) wrap
// them and keep their own parity tests below unchanged.
//
// tmux parity oracle (local tmux source checkout):
//   input.c input_osc_52_parse() decodes with b64_pton (compat/base64.c):
//     - invalid characters reject the whole payload (returns -1)
//     - whitespace is skipped anywhere in the payload
//     - both padded and unpadded input decode
//   input.c input_reply_clipboard() encodes replies with b64_ntop:
//     - standard alphabet, '=' padded

use crate::util::{base64_encode, base64_decode, base64_encode_bytes, base64_decode_bytes};

// RFC 4648 section 10 test vectors. Fixed by the spec, so any codec
// regression fails immediately.
const RFC4648: &[(&str, &str)] = &[
    ("", ""),
    ("f", "Zg=="),
    ("fo", "Zm8="),
    ("foo", "Zm9v"),
    ("foob", "Zm9vYg=="),
    ("fooba", "Zm9vYmE="),
    ("foobar", "Zm9vYmFy"),
];

#[test]
fn bytes_encode_matches_rfc4648_vectors() {
    for (plain, expected) in RFC4648 {
        assert_eq!(base64_encode_bytes(plain.as_bytes()), *expected,
            "byte encode of {:?} deviates from RFC 4648", plain);
    }
}

#[test]
fn bytes_decode_matches_rfc4648_vectors() {
    for (plain, encoded) in RFC4648 {
        let decoded = base64_decode_bytes(encoded).expect("valid vector must decode");
        assert_eq!(decoded, plain.as_bytes().to_vec(),
            "byte decode of {:?} deviates from RFC 4648", encoded);
    }
}

#[test]
fn bytes_roundtrip_all_byte_values() {
    // The cross-session screen payload is arbitrary binary VT data, so the
    // codec must round-trip every byte value, not just UTF-8 text.
    let all: Vec<u8> = (0u8..=255).collect();
    let enc = base64_encode_bytes(&all);
    let dec = base64_decode_bytes(&enc).expect("must decode own output");
    assert_eq!(dec, all, "binary round-trip is lossy");
}

#[test]
fn bytes_decode_rejects_invalid_input_like_tmux() {
    // tmux b64_pton returns -1 on invalid characters; the codec must too so
    // cross-session decode failures stay detectable.
    assert_eq!(base64_decode_bytes("not*valid"), None);
}

#[test]
fn bytes_decode_skips_embedded_whitespace() {
    // tmux compat/base64.c b64_pton: "Skip whitespace anywhere."
    let plain = "foobar";
    let clean = base64_encode_bytes(plain.as_bytes());
    let with_space = format!("{} {}", &clean[..4], &clean[4..]);
    let with_newline = format!("{}\n{}", &clean[..4], &clean[4..]);
    assert_eq!(base64_decode_bytes(&with_space).as_deref(), Some(plain.as_bytes()),
        "space inside payload must be skipped like tmux");
    assert_eq!(base64_decode_bytes(&with_newline).as_deref(), Some(plain.as_bytes()),
        "newline inside payload must be skipped like tmux");
    assert_eq!(base64_decode_bytes(&clean).as_deref(), base64_decode_bytes(&with_space).as_deref(),
        "decode with whitespace must equal decode without");
}

#[test]
fn bytes_decode_accepts_unpadded_input() {
    // tmux b64_pton accepts both padded and unpadded input.
    assert_eq!(base64_decode_bytes("Zm9v").as_deref(), Some(b"foo".as_slice()));
    assert_eq!(base64_decode_bytes("Zm8").as_deref(), Some(b"fo".as_slice()));
}

#[test]
fn handrolled_string_encode_matches_byte_encode() {
    // OSC 52 replies from psmux (util::base64_encode) must be byte-identical
    // to the byte-level encoder and tmux b64_ntop.
    for s in ["", "f", "fo", "foo", "hello world", "line1\nline2\ttab",
              "unicode: \u{00e9}\u{4e16}\u{754c}", "\x1b[31mred\x1b[0m"] {
        assert_eq!(base64_encode(s), base64_encode_bytes(s.as_bytes()),
            "hand-rolled string encode diverges from byte encode for {:?}", s);
    }
}

#[test]
fn handrolled_string_decode_accepts_byte_encode_output() {
    for s in ["f", "foo", "clipboard text with spaces", "\x1b]0;title\x07"] {
        let enc = base64_encode_bytes(s.as_bytes());
        assert_eq!(base64_decode(&enc).as_deref(), Some(s),
            "hand-rolled string decode cannot read byte encode output for {:?}", s);
    }
}

#[test]
fn osc52_decode_padded_and_unpadded_parity() {
    // tmux b64_pton accepts both "Zm8=" and "Zm8"; psmux must too.
    assert_eq!(base64_decode("Zm8=").as_deref(), Some("fo"), "padded input must decode");
    assert_eq!(base64_decode("Zm8").as_deref(), Some("fo"), "unpadded input must decode");
    assert_eq!(base64_decode("Zm9vYg==").as_deref(), Some("foob"));
    assert_eq!(base64_decode("Zm9vYg").as_deref(), Some("foob"));
}

#[test]
fn osc52_decode_rejects_invalid_chars_parity() {
    // tmux rejects the entire payload on any invalid character. A terminal
    // app sending garbage in OSC 52 must not corrupt the clipboard.
    assert_eq!(base64_decode("Zm9*vYg"), None, "invalid '*' must reject payload");
    assert_eq!(base64_decode("Zm9\u{00e9}vYg"), None, "non-ascii must reject payload");
}

#[test]
fn osc52_decode_skips_whitespace_parity() {
    // tmux compat/base64.c b64_pton: "Skip whitespace anywhere." Some
    // OSC 52 producers wrap long payloads; tmux still decodes them.
    assert_eq!(base64_decode("Zm9v Ymfy".replace("Ymfy", "YmFy").as_str()).as_deref(),
        Some("foobar"), "space inside payload must be skipped like tmux");
    assert_eq!(base64_decode("Zm9v\nYmFy").as_deref(), Some("foobar"),
        "newline inside payload must be skipped like tmux");
    assert_eq!(base64_decode("Zm9v\r\n\tYmFy").as_deref(), Some("foobar"),
        "CRLF and tab inside payload must be skipped like tmux");
    assert_eq!(base64_decode(" Zm8= ").as_deref(), Some("fo"),
        "surrounding whitespace must be skipped like tmux");
}

#[test]
fn osc52_parser_stages_whitespace_payload_parity() {
    // The vt100 OSC 52 gate must admit ASCII whitespace inside the payload
    // (tmux b64_pton skips it anywhere); the pre-fix all-BASE64 gate dropped
    // the whole sequence before the server decoder ever saw it.
    let mut parser = vt100::Parser::new(24, 80, 500);
    parser.process(b"\x1b]52;c;aGVsbG8 gd29ybGQ=\x07");
    let staged = parser.screen_mut().take_clipboard();
    let (_, data) = staged.expect("whitespace payload must be staged like tmux");
    assert_eq!(data, b"aGVsbG8 gd29ybGQ=".to_vec());
    assert_eq!(base64_decode(std::str::from_utf8(&data).unwrap()).as_deref(),
        Some("hello world"), "server-side decode of staged payload");

    // And genuinely invalid characters must still reject the payload.
    let mut parser = vt100::Parser::new(24, 80, 500);
    parser.process(b"\x1b]52;c;Zm9*vYg\x07");
    assert!(parser.screen_mut().take_clipboard().is_none(),
        "invalid base64 must not be staged");
}

#[test]
fn cross_session_screen_state_roundtrip() {
    // Exercises the exact payload cross_session_server.rs ships: a colored
    // vt100 screen serialized with state_formatted(), byte-encoded, then
    // byte-decoded and replayed into a fresh parser on the "other server".
    let mut parser = vt100::Parser::new(24, 80, 500);
    parser.process(b"\x1b[2J\x1b[H\x1b[1;31mRED_MARKER\x1b[0m plain \x1b[44mBLUEBG\x1b[0m\r\nrow2 \x1b[3;20Hjump");
    let buf = parser.screen().state_formatted();
    let b64 = base64_encode_bytes(&buf);
    assert!(b64.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'='),
        "encoded screen payload must be pure base64 for the line protocol");
    let restored = base64_decode_bytes(&b64).expect("screen payload must decode");
    assert_eq!(restored, buf, "screen bytes corrupted by base64 round-trip");

    let mut parser2 = vt100::Parser::new(24, 80, 500);
    parser2.process(&restored);
    let contents = parser2.screen().contents();
    assert!(contents.contains("RED_MARKER"), "replayed screen lost text: {contents:?}");
    assert!(contents.contains("BLUEBG"), "replayed screen lost styled text");
    let cell = parser2.screen().cell(0, 0).expect("cell 0,0");
    assert_eq!(cell.fgcolor(), vt100::Color::Idx(1), "red fg lost in round-trip");
    assert!(cell.bold(), "bold attr lost in round-trip");
}
