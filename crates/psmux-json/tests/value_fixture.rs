// Covers: ZDEP-014
// Requirement: `psmux_json::parse(&str) -> Result<Value, Error>` and
// `Value::to_string()` reproduce serde_json 1.0.151 byte-for-byte for every
// value-ok/value-str/value-err/float row of the committed oracle fixture
// `tests-rs/fixtures/serde_json_1.0.151.txt` (RFC 8259 grammar: no trailing
// commas, comments, leading zeros, NaN/Infinity, single quotes, or
// unescaped control characters; the four whitespace bytes; every
// `\`-escape incl. `\uXXXX` surrogate pairs; nesting deeper than the
// serde_json default recursion limit is Err). `Value` implements
// `Index<&str>`/`Index<usize>` (missing key/index -> shared `Null`),
// `PartialEq<&str>`, `PartialEq<bool>`, `PartialEq<i64>`, and accessors
// `as_str`, `as_bool`, `as_i64`, `as_u64`, `as_f64`, `as_array`,
// `as_object`, `get(&str)`. `Error` implements `Display` and
// `std::error::Error`. A 10 000-deep `[` input must return `Err` without
// overflowing the stack.

use psmux_json::Value;

const FIXTURE: &str = include_str!("../../../tests-rs/fixtures/serde_json_1.0.151.txt");

/// Row payload unescaping mirrors the generator's escaping documented in the
/// fixture header: backslash -> \\, pipe -> \p, newline -> \n.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('p') => out.push('|'),
                Some('n') => out.push('\n'),
                Some(other) => { out.push('\\'); out.push(other); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct Row {
    kind: String,
    label: String,
    payload: String,
}

fn parse_fixture() -> Vec<Row> {
    let mut rows = Vec::new();
    for line in FIXTURE.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, '|');
        let kind = parts.next().unwrap_or_default().to_string();
        let label = parts.next().unwrap_or_default().to_string();
        let payload = unescape(parts.next().unwrap_or_default());
        rows.push(Row { kind, label, payload });
    }
    rows
}

/// (a) Every value-ok row parses without error, and the paired value-str row
/// (immediately following, same label) matches `Value::to_string()` exactly.
#[test]
fn value_ok_rows_parse_and_round_trip_to_the_recorded_string() {
    let rows = parse_fixture();
    let mut checked = 0;
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind == "value-ok" {
            let label = rows[i].label.clone();
            let input = rows[i].payload.clone();
            let parsed = psmux_json::parse(&input)
                .unwrap_or_else(|e| panic!("value-ok {label} must parse: {input:?}: {e}"));
            // find the matching value-str row by label (may not be adjacent
            // for the 1e400 special-case row, but is for all others).
            let want = rows.iter()
                .find(|r| r.kind == "value-str" && r.label == label)
                .unwrap_or_else(|| panic!("no value-str row for label {label}"));
            assert_eq!(
                parsed.to_string(), want.payload,
                "round-trip mismatch for {label}"
            );
            checked += 1;
        }
        i += 1;
    }
    assert!(checked >= 25, "expected many value-ok rows, found {checked}");
}

/// (b) Every value-err row is rejected by the parser.
#[test]
fn value_err_rows_are_rejected() {
    let rows = parse_fixture();
    let mut checked = 0;
    for row in &rows {
        if row.kind == "value-err" {
            let result = psmux_json::parse(&row.payload);
            assert!(
                result.is_err(),
                "value-err {} should fail to parse: {:?} -> {:?}",
                row.label, row.payload, result
            );
            checked += 1;
        }
    }
    assert!(checked >= 15, "expected many value-err rows, found {checked}");
}

/// (c) depth-<n>-ok / depth-<n>-err rows reproduce the exact recursion
/// boundary serde_json enforces.
#[test]
fn depth_boundary_rows_match_serde_json() {
    let rows = parse_fixture();
    let mut saw_ok = false;
    let mut saw_err = false;
    for row in &rows {
        if row.kind.starts_with("depth-") {
            let result = psmux_json::parse(&row.payload);
            if row.kind.ends_with("-ok") {
                assert!(result.is_ok(), "{} should parse: {}", row.kind, row.payload);
                saw_ok = true;
            } else {
                assert!(result.is_err(), "{} should fail to parse", row.kind);
                saw_err = true;
            }
        }
    }
    assert!(saw_ok && saw_err, "fixture must carry both sides of the depth boundary");
}

/// (d) A 10,000-deep `[` input returns Err and does not overflow the stack
/// (built directly in the test, independent of the fixture).
#[test]
fn extremely_deep_nesting_returns_err_without_stack_overflow() {
    let input: String = "[".repeat(10_000);
    let result = psmux_json::parse(&input);
    assert!(result.is_err(), "10,000-deep nesting must be Err, not a stack overflow");
}

/// (e) float rows: the writer's `to_string()` output for representative
/// f64 values matches what the fixture recorded from serde_json (numbers
/// arrive into `Value` only via parsing in this crate's public API, so we
/// round-trip parse(serialized) -> to_string() to exercise the writer path).
#[test]
fn float_rows_writer_output_matches_fixture() {
    let rows = parse_fixture();
    let mut checked = 0;
    for row in &rows {
        if row.kind == "float" {
            // Reparsing the fixture's own recorded float text must produce
            // the same text back out (writer round-trips its own numbers).
            let parsed = psmux_json::parse(&row.payload)
                .unwrap_or_else(|e| panic!("float row {} must parse: {}", row.label, e));
            assert_eq!(parsed.to_string(), row.payload, "float row {} writer mismatch", row.label);
            checked += 1;
        }
    }
    assert!(checked >= 6, "expected float rows, found {checked}");
}

// ---- accessor / trait surface named in ZDEP-014 ----

#[test]
fn index_by_str_and_usize_yield_null_for_missing() {
    let v = psmux_json::parse(r#"{"a":1}"#).unwrap();
    assert_eq!(v["missing"], Value::Null);
    let arr = psmux_json::parse(r#"[1,2]"#).unwrap();
    assert_eq!(arr[99], Value::Null);
}

#[test]
fn partial_eq_str_bool_i64() {
    let v = psmux_json::parse(r#"{"s":"x","b":true,"n":5}"#).unwrap();
    assert_eq!(v["s"], "x");
    assert_eq!(v["b"], true);
    assert_eq!(v["n"], 5i64);
}

#[test]
fn accessors_cover_every_named_kind() {
    let v = psmux_json::parse(r#"{"s":"x","b":true,"i":5,"u":5,"f":1.5,"a":[1],"o":{"k":1}}"#).unwrap();
    assert_eq!(v["s"].as_str(), Some("x"));
    assert_eq!(v["b"].as_bool(), Some(true));
    assert_eq!(v["i"].as_i64(), Some(5));
    assert_eq!(v["u"].as_u64(), Some(5));
    assert_eq!(v["f"].as_f64(), Some(1.5));
    assert!(v["a"].as_array().is_some());
    assert!(v["o"].as_object().is_some());
    assert_eq!(v.get("s").and_then(|x| x.as_str()), Some("x"));
    assert!(v.get("missing").is_none() || v.get("missing") == Some(&Value::Null));
}

#[test]
fn error_implements_display_and_std_error() {
    let err = psmux_json::parse("{").unwrap_err();
    let _: &dyn std::error::Error = &err;
    let text = format!("{}", err);
    assert!(!text.is_empty(), "Display impl must produce a non-empty message");
}
