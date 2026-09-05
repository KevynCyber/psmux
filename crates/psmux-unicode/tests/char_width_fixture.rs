// Covers: ZDEP-012
// Requirement: `psmux_unicode::char_width(char) -> Option<usize>` reproduces
// unicode-width 0.2.2's `UnicodeWidthChar::width` exactly: `None` for the Cc
// general category (U+0000..U+001F, U+007F..U+009F), `Some(0)` for
// zero-width characters (Mn/Me/Cf, ZWJ, ZWNJ, Hangul jamo V/T, ...),
// `Some(2)` for East Asian Wide/Fullwidth and emoji-presentation characters,
// `Some(3)` for U+17D8 KHMER SIGN BEYYAL (the single width-3 code point in
// unicode-width 0.2.2), `Some(1)` otherwise (including unassigned and
// private-use code points). The table backing this is generated from the
// committed oracle fixture `tests-rs/fixtures/unicode_width_0.2.2.txt`
// (run-length rows `<start-hex>..<end-hex>|<N|0|1|2|3>` covering every code
// point U+0000..U+10FFFF except surrogates), and must replay with 0
// mismatches over every code point in that range.

const FIXTURE: &str = include_str!("../../../tests-rs/fixtures/unicode_width_0.2.2.txt");

struct Row {
    start: u32,
    end: u32,
    code: char,
}

fn parse_fixture() -> Vec<Row> {
    let mut rows = Vec::new();
    for line in FIXTURE.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (range, code) = line.split_once('|').unwrap_or_else(|| panic!("malformed row: {line:?}"));
        let (start_hex, end_hex) = range.split_once("..").unwrap_or_else(|| panic!("malformed range: {line:?}"));
        let start = u32::from_str_radix(start_hex, 16).unwrap_or_else(|_| panic!("bad start hex: {line:?}"));
        let end = u32::from_str_radix(end_hex, 16).unwrap_or_else(|_| panic!("bad end hex: {line:?}"));
        let code = code.chars().next().unwrap_or_else(|| panic!("empty width code: {line:?}"));
        assert!(
            matches!(code, 'N' | '0' | '1' | '2' | '3'),
            "row {line:?} has width code outside {{N,0,1,2,3}}: {code:?}"
        );
        rows.push(Row { start, end, code });
    }
    rows
}

fn width_from_code(code: char) -> Option<usize> {
    match code {
        'N' => None,
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        other => panic!("unexpected width code {other:?}"),
    }
}

/// (a) Fixture rows must be contiguous and ascending, and together must
/// cover exactly U+0000..=U+10FFFF minus the surrogate range D800..=DFFF,
/// with no gaps and no overlaps.
#[test]
fn fixture_rows_are_contiguous_ascending_and_cover_the_full_range() {
    let rows = parse_fixture();
    assert!(!rows.is_empty(), "fixture must not be empty");

    let mut expected_next: u32 = 0;
    for (i, row) in rows.iter().enumerate() {
        assert!(row.start <= row.end, "row {i} has start > end: {:04X}..{:04X}", row.start, row.end);
        if row.start == 0xE000 && expected_next == 0xD800 {
            // surrogate range D800..=DFFF is skipped entirely
            expected_next = 0xE000;
        }
        assert_eq!(
            row.start, expected_next,
            "row {i} starts at {:04X}, expected {:04X} (gap or overlap)",
            row.start, expected_next
        );
        expected_next = row.end + 1;
    }
    assert_eq!(
        expected_next, 0x110000,
        "fixture must cover up to and including U+10FFFF, ended at {:04X}",
        expected_next - 1
    );
}

/// (b) Fixture replay: every code point U+0000..=U+10FFFF (minus surrogates)
/// must match `psmux_unicode::char_width` exactly, with zero mismatches.
#[test]
fn char_width_matches_fixture_with_zero_mismatches() {
    let rows = parse_fixture();
    let mut mismatches: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for row in &rows {
        let expected = width_from_code(row.code);
        for cp in row.start..=row.end {
            if (0xD800..=0xDFFF).contains(&cp) {
                continue;
            }
            let c = char::from_u32(cp).expect("valid scalar value in fixture range");
            checked += 1;
            let actual = psmux_unicode::char_width(c);
            if actual != expected {
                if mismatches.len() < 20 {
                    mismatches.push(format!(
                        "U+{cp:04X} expected={expected:?} actual={actual:?}"
                    ));
                }
            }
        }
    }

    assert!(checked > 1_000_000, "expected to check the full Unicode range, only checked {checked}");
    assert!(
        mismatches.is_empty(),
        "mismatches (first 20 shown):\n{}",
        mismatches.join("\n")
    );
}

/// (c) Explicit boundary/semantic assertions named in the ZDEP-012 spec.
#[test]
fn explicit_semantic_boundaries() {
    assert_eq!(psmux_unicode::char_width('\u{0000}'), None, "U+0000 must be None");
    assert_eq!(psmux_unicode::char_width('\u{001F}'), None, "U+001F must be None");
    assert_eq!(psmux_unicode::char_width('\u{007F}'), None, "U+007F (DEL) must be None");
    assert_eq!(psmux_unicode::char_width('\u{0080}'), None, "U+0080 (C1) must be None");
    assert_eq!(psmux_unicode::char_width('\u{009F}'), None, "U+009F (C1) must be None");
    assert_eq!(psmux_unicode::char_width('\u{0301}'), Some(0), "U+0301 combining acute must be Some(0)");
    assert_eq!(psmux_unicode::char_width('\u{200D}'), Some(0), "U+200D ZWJ must be Some(0)");
    assert_eq!(psmux_unicode::char_width('\u{200B}'), Some(0), "U+200B ZWSP must be Some(0)");
    assert_eq!(psmux_unicode::char_width('a'), Some(1), "'a' must be Some(1)");
    assert_eq!(psmux_unicode::char_width('\u{4E2D}'), Some(2), "U+4E2D CJK ideograph must be Some(2)");
    assert_eq!(psmux_unicode::char_width('\u{1F600}'), Some(2), "U+1F600 emoji must be Some(2)");
    assert_eq!(psmux_unicode::char_width('\u{E000}'), Some(1), "U+E000 (private use) must be Some(1)");
    // U+0378 is unassigned in the Greek block; unicode-width 0.2.2 assigns it
    // Some(1), matching the fixture row 0370..0482|1 (verified against the
    // committed fixture at authoring time).
    assert_eq!(psmux_unicode::char_width('\u{0378}'), Some(1), "unassigned U+0378 must be Some(1)");
    // The single width-3 exception in unicode-width 0.2.2.
    assert_eq!(psmux_unicode::char_width('\u{17D8}'), Some(3), "U+17D8 KHMER SIGN BEYYAL must be Some(3)");
}
