// Covers: ZDEP-017
// Requirement: `psmux_regex::Regex::new(&str) -> Result<Regex, Error>` and
// `psmux_regex::escape(&str) -> String` reproduce regex 1.13.1 on the
// documented supported subset (docs/features/zero-deps.md ## ZDEP-017):
// leftmost-first alternation; greedy/lazy `* + ? {m} {m,} {m,n}` with
// stacking; captures record the last loop iteration; empty alternatives and
// `()` are valid; `^`/`$` are text-only anchors; `.` excludes `\n`; `[^a]`
// includes `\n`; empty pattern matches empty at 0; groups `()` `(?:)`
// `(?P<n>)` `(?<n>)` with ASCII names (duplicate name is Err); classes with
// range/`]`/`-` edge placements and ASCII-only POSIX `[[:name:]]`/
// `[[:^name:]]`; `\d \w \s \D \W \S \b \B` plus the named single-char
// escapes and the escaped-literal set; `(?i)` ONLY as a leading prefix
// (repeated prefix ok) with fold-set semantics; everything else the oracle
// accepts but the engine doesn't (`\p{..}`, `(?s)(?m)(?x)(?u)(?R)(?-i)
// (?i:..)`, mid-pattern `(?i)`, `\A \z \< \> \u{..}`, class set operators,
// nested classes, bogus/collating/equivalence POSIX forms, non-ASCII group
// names, `{m,n}` with n > 1000, nesting depth > 64) is `Err`, recorded as
// `unsupported`; every oracle `Err` pattern is also `Err` for us. `Regex`
// exposes `is_match(&str) -> bool`, `find(&str) -> Option<(usize, usize)>`,
// `captures(&str) -> Option<Captures>`, `replace(&str, &str) -> String`
// (first match only, `$$`/`$name`/`${name}`/`$0`/nonexistent-or-unmatched
// group -> empty/literal-`$` rules); `Captures::get(i) -> Option<(usize,
// usize)>`, `Captures::len() -> usize` (group count incl. group 0),
// `Captures::name(&str) -> Option<(usize, usize)>`. The oracle fixture
// `crates/psmux-regex/tests/fixtures/regex_1.13.1.txt` (rows
// `<kind>|<name>|<tab-separated fields>`) replays with 0 mismatches; the
// replay parser rejects a row with the wrong field count for its kind or an
// unknown kind.

use psmux_regex::Regex;

const FIXTURE: &str = include_str!("fixtures/regex_1.13.1.txt");

#[derive(Debug, Clone)]
enum Row {
    Match { name: String, pattern: String, input: String, spans: String },
    Err { name: String, pattern: String },
    Unsupported { name: String, pattern: String },
    Replace { name: String, pattern: String, input: String, replacement: String, output: String },
    Escape { name: String, input: String, output: String },
}

/// Unescapes a fixture field per the header contract: `\\` `\t` `\n` `\r`
/// and `\x{HH}` (uppercase hex byte/scalar value).
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('x') => {
                assert_eq!(chars.next(), Some('{'), "malformed \\x escape in field {s:?}");
                let mut hex = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(h) => hex.push(h),
                        None => panic!("unterminated \\x{{..}} escape in field {s:?}"),
                    }
                }
                let code = u32::from_str_radix(&hex, 16)
                    .unwrap_or_else(|e| panic!("bad hex in \\x{{{hex}}}: {e}"));
                out.push(char::from_u32(code).unwrap_or_else(|| panic!("invalid scalar {code:#x}")));
            }
            Some(other) => panic!("unknown escape \\{other} in field {s:?}"),
            None => panic!("trailing backslash in field {s:?}"),
        }
    }
    out
}

/// Strict row parser: panics on an unknown `kind` or a field count that
/// doesn't match the kind's documented shape.
fn parse_row(line: &str) -> Row {
    let mut top = line.splitn(3, '|');
    let kind = top.next().unwrap_or_default();
    let name = top.next().unwrap_or_default().to_string();
    let rest = top.next().unwrap_or_default();
    let fields: Vec<String> = rest.split('\t').map(unescape).collect();
    match kind {
        "match" => {
            assert_eq!(fields.len(), 3, "match row {name} must have 3 fields, got {}", fields.len());
            Row::Match { name, pattern: fields[0].clone(), input: fields[1].clone(), spans: fields[2].clone() }
        }
        "err" => {
            assert_eq!(fields.len(), 1, "err row {name} must have 1 field, got {}", fields.len());
            Row::Err { name, pattern: fields[0].clone() }
        }
        "unsupported" => {
            assert_eq!(fields.len(), 1, "unsupported row {name} must have 1 field, got {}", fields.len());
            Row::Unsupported { name, pattern: fields[0].clone() }
        }
        "replace" => {
            assert_eq!(fields.len(), 4, "replace row {name} must have 4 fields, got {}", fields.len());
            Row::Replace {
                name,
                pattern: fields[0].clone(),
                input: fields[1].clone(),
                replacement: fields[2].clone(),
                output: fields[3].clone(),
            }
        }
        "escape" => {
            assert_eq!(fields.len(), 2, "escape row {name} must have 2 fields, got {}", fields.len());
            Row::Escape { name, input: fields[0].clone(), output: fields[1].clone() }
        }
        other => panic!("unknown fixture row kind {other:?} for row {name}"),
    }
}

fn parse_fixture() -> Vec<Row> {
    FIXTURE
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(parse_row)
        .collect()
}

// ---- strict parser tests (inline bad rows, not the fixture) ----

#[test]
#[should_panic(expected = "must have 3 fields")]
fn strict_parser_rejects_match_row_with_wrong_field_count() {
    parse_row("match|bad_row|only\ttwo");
}

#[test]
#[should_panic(expected = "unknown fixture row kind")]
fn strict_parser_rejects_unknown_kind() {
    parse_row("bogus|bad_row|a\tb");
}

#[test]
#[should_panic(expected = "must have 1 field")]
fn strict_parser_rejects_err_row_with_extra_field() {
    parse_row("err|bad_row|a\tb");
}

// ---- fixture size sanity ----

#[test]
fn fixture_has_at_least_100_match_rows() {
    let rows = parse_fixture();
    let count = rows.iter().filter(|r| matches!(r, Row::Match { .. })).count();
    assert!(count >= 100, "expected >= 100 match rows, found {count}");
}

// ---- per-kind replay ----

#[test]
fn match_rows_reproduce_oracle_spans() {
    let rows = parse_fixture();
    let mut mismatches = Vec::new();
    for row in &rows {
        let Row::Match { name, pattern, input, spans } = row else { continue };
        let expected = if spans == "nomatch" {
            None
        } else {
            Some(spans.clone())
        };
        let re = match Regex::new(pattern) {
            Ok(re) => re,
            Err(e) => {
                mismatches.push(format!("{name}: pattern {pattern:?} failed to compile: {e}"));
                continue;
            }
        };
        let actual = match re.captures(input) {
            None => None,
            Some(caps) => {
                let mut parts = Vec::new();
                for i in 0..caps.len() {
                    match caps.get(i) {
                        Some((s, e)) => parts.push(format!("{i}:{s}-{e}")),
                        None => parts.push(format!("{i}:-")),
                    }
                }
                Some(parts.join(";"))
            }
        };
        if actual != expected {
            mismatches.push(format!(
                "{name}: pattern {pattern:?} on {input:?}: expected {expected:?}, got {actual:?}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "match row mismatches:\n{}", mismatches.join("\n"));
}

#[test]
fn err_rows_are_rejected() {
    let rows = parse_fixture();
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for row in &rows {
        let Row::Err { name, pattern } = row else { continue };
        checked += 1;
        if Regex::new(pattern).is_ok() {
            mismatches.push(format!("{name}: pattern {pattern:?} expected Err, compiled Ok"));
        }
    }
    assert!(checked >= 30, "expected >= 30 err rows, found {checked}");
    assert!(mismatches.is_empty(), "err row mismatches:\n{}", mismatches.join("\n"));
}

#[test]
fn unsupported_rows_are_rejected_by_our_engine() {
    let rows = parse_fixture();
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for row in &rows {
        let Row::Unsupported { name, pattern } = row else { continue };
        checked += 1;
        if Regex::new(pattern).is_ok() {
            mismatches.push(format!(
                "{name}: pattern {pattern:?} is oracle-Ok/us-unsupported, but our engine compiled it Ok"
            ));
        }
    }
    assert!(checked > 0, "expected unsupported rows in fixture");
    assert!(mismatches.is_empty(), "unsupported row mismatches:\n{}", mismatches.join("\n"));
}

#[test]
fn replace_rows_reproduce_oracle_output() {
    let rows = parse_fixture();
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for row in &rows {
        let Row::Replace { name, pattern, input, replacement, output } = row else { continue };
        checked += 1;
        let re = match Regex::new(pattern) {
            Ok(re) => re,
            Err(e) => {
                mismatches.push(format!("{name}: pattern {pattern:?} failed to compile: {e}"));
                continue;
            }
        };
        let actual = re.replace(input, replacement);
        if &actual != output {
            mismatches.push(format!(
                "{name}: pattern {pattern:?} on {input:?} with {replacement:?}: expected {output:?}, got {actual:?}"
            ));
        }
    }
    assert!(checked >= 25, "expected >= 25 replace rows, found {checked}");
    assert!(mismatches.is_empty(), "replace row mismatches:\n{}", mismatches.join("\n"));
}

#[test]
fn escape_rows_reproduce_oracle_output() {
    let rows = parse_fixture();
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for row in &rows {
        let Row::Escape { name, input, output } = row else { continue };
        checked += 1;
        let actual = psmux_regex::escape(input);
        if &actual != output {
            mismatches.push(format!("{name}: escape({input:?}) expected {output:?}, got {actual:?}"));
        }
    }
    assert!(checked >= 10, "expected >= 10 escape rows, found {checked}");
    assert!(mismatches.is_empty(), "escape row mismatches:\n{}", mismatches.join("\n"));
}
