// Covers: ZDEP-013
// Requirement: `psmux_unicode::str_width(&str) -> usize` reproduces
// unicode-width 0.2.2's `UnicodeWidthStr::width` for every row of the
// committed string fixture `tests-rs/fixtures/unicode_str_width_0.2.2.txt`
// (rows `<category>|<string with \u{XXXX} escapes>|<width>`). Declared
// categories: ascii, cjk, thai clusters (the width_thai_441 strings),
// combining marks, control characters (C0, DEL, C1, `\n`, `\t`, `\r`),
// U+FE0F VS16 after an emoji-presentation-capable base (the
// issue533_vs16_width strings) and after a non-emoji base, U+FE0E VS15
// after an emoji base, ZWJ emoji sequences, regional-indicator pairs,
// Hangul jamo L+V+T sequences, empty string. The exact widths the crate
// assigns are whatever the fixture records; the implementation is the sum
// of `char_width` plus the sequence rules needed for 0 fixture mismatches.

const FIXTURE: &str = include_str!("../../../tests-rs/fixtures/unicode_str_width_0.2.2.txt");

const DECLARED_CATEGORIES: &[&str] = &[
    "ascii",
    "cjk",
    "thai",
    "combining",
    "control",
    "vs16_emoji",
    "vs16_nonemoji",
    "vs15",
    "zwj",
    "regional_indicator",
    "hangul_jamo",
    "empty",
];

struct Row {
    category: String,
    text: String,
    width: usize,
    raw: String,
}

/// Unescape `\u{XXXX}` sequences in an otherwise-ASCII fixture string.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'u') {
            chars.next(); // consume 'u'
            let brace = chars.next();
            assert_eq!(brace, Some('{'), "malformed \\u escape in {s:?}");
            let mut hex = String::new();
            for h in chars.by_ref() {
                if h == '}' {
                    break;
                }
                hex.push(h);
            }
            let cp = u32::from_str_radix(&hex, 16).unwrap_or_else(|_| panic!("bad hex {hex:?} in {s:?}"));
            let ch = char::from_u32(cp).unwrap_or_else(|| panic!("bad code point {cp:04X} in {s:?}"));
            out.push(ch);
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_fixture() -> Vec<Row> {
    let mut rows = Vec::new();
    for line in FIXTURE.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        assert_eq!(parts.len(), 3, "malformed fixture row: {line:?}");
        let category = parts[0].to_string();
        assert!(
            DECLARED_CATEGORIES.contains(&category.as_str()),
            "row has undeclared category {category:?}: {line:?}"
        );
        let text = unescape(parts[1]);
        let width: usize = parts[2].parse().unwrap_or_else(|_| panic!("bad width in row: {line:?}"));
        rows.push(Row { category, text, width, raw: line.to_string() });
    }
    rows
}

/// (a) Fixture replay: every row must match `psmux_unicode::str_width`
/// exactly, with zero mismatches.
#[test]
fn str_width_matches_fixture_with_zero_mismatches() {
    let rows = parse_fixture();
    assert!(!rows.is_empty(), "fixture must not be empty");

    let mut mismatches: Vec<String> = Vec::new();
    for row in &rows {
        let actual = psmux_unicode::str_width(&row.text);
        if actual != row.width {
            mismatches.push(format!(
                "{} expected={} actual={}",
                row.raw, row.width, actual
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} mismatch(es) out of {} rows:\n{}",
        mismatches.len(),
        rows.len(),
        mismatches.join("\n")
    );
}

/// (b) Every declared category appears at least once in the fixture.
#[test]
fn every_declared_category_is_present() {
    let rows = parse_fixture();
    for cat in DECLARED_CATEGORIES {
        assert!(
            rows.iter().any(|r| r.category == *cat),
            "declared category {cat:?} has no fixture rows"
        );
    }
}

/// (c) Explicit boundary assertions.
#[test]
fn explicit_boundaries() {
    assert_eq!(psmux_unicode::str_width(""), 0, "empty string must be width 0");
    assert_eq!(psmux_unicode::str_width("abc"), 3, "ascii \"abc\" must be width 3");
}
