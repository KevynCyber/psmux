// Covers: ZDEP-017
// Requirement: `psmux_regex::Regex` is a Pike VM (no backtracking):
// `(a*)*b` against 10,000 `a`s (no trailing `b`) must return `is_match ==
// false` in under 1 second wall clock, not hang exponentially. A pattern
// of 1,000 nested `(` (well past the documented nesting-depth-64 limit)
// must return `Err` from `Regex::new`, and must do so without deep native
// recursion overflowing the stack -- proven by running the parse inside a
// thread with a deliberately small (256 KiB) stack. `(?i)` repeated as a
// leading prefix (`(?i)(?i)a`) is explicitly documented as allowed and
// must compile `Ok`.

use psmux_regex::Regex;
use std::time::{Duration, Instant};

#[test]
fn catastrophic_backtracking_shape_returns_no_match_quickly() {
    let pattern = "(a*)*b";
    let input = "a".repeat(10_000);
    let re = Regex::new(pattern).unwrap_or_else(|e| panic!("{pattern:?} must compile: {e}"));

    let start = Instant::now();
    let matched = re.is_match(&input);
    let elapsed = start.elapsed();

    assert!(!matched, "{pattern:?} must not match {} a's with no trailing b", input.len());
    assert!(
        elapsed < Duration::from_secs(1),
        "{pattern:?} on 10,000 a's took {elapsed:?}, expected < 1s (no backtracking)"
    );
}

#[test]
fn deeply_nested_open_parens_errs_without_stack_overflow() {
    // Run on a thread with a small, fixed stack: if the parser recursed
    // proportionally to nesting depth this would overflow and abort the
    // test process instead of returning a normal Err.
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let pattern = "(".repeat(1_000);
            Regex::new(&pattern).is_err()
        })
        .expect("failed to spawn small-stack thread");

    let is_err = handle.join().expect("small-stack thread must not crash/overflow");
    assert!(is_err, "1,000 nested '(' must be Err, not Ok");
}

#[test]
fn repeated_leading_case_insensitive_prefix_is_ok() {
    let result = Regex::new("(?i)(?i)a");
    assert!(result.is_ok(), "repeated leading (?i) prefix must be Ok, got {result:?}");
}
