// Covers: ZDEP-017
// Requirement: `psmux_regex::Regex` is a Pike VM (no backtracking):
// `(a*)*b` against 10,000 `a`s (no trailing `b`) must return `is_match ==
// false` in under 1 second wall clock, not hang exponentially. A pattern
// of 1,000 nested `(` (well past the documented nesting-depth-64 limit)
// must return `Err` from `Regex::new`, and must do so without deep native
// recursion overflowing the stack -- proven by running the parse inside a
// thread with a deliberately small (256 KiB) stack. `(?i)` repeated as a
// leading prefix (`(?i)(?i)a`) is explicitly documented as allowed and
// must compile `Ok`. A pattern with 6,000 stacked `{1,1}` quantifiers on
// one atom, and a pattern with a 50,000-branch alternation chain, must
// each finish `Regex::new` (Ok or Err) without overflowing a 256 KiB
// stack and in under 5 seconds.

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
fn stacked_quantifiers_do_not_overflow_stack() {
    // compile recurses once per stacked quantifier on a single atom;
    // 6,000 stacked `{1,1}` quantifiers on the default 2 MiB stack is a
    // confirmed stack-overflow abort. Run on a small 256 KiB stack and
    // require the call to return (Ok or Err) rather than crash.
    let start = Instant::now();
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let pattern = "a".to_string() + &"{1,1}".repeat(6_000);
            let _ = Regex::new(&pattern);
        })
        .expect("failed to spawn small-stack thread");

    assert!(handle.join().is_ok(), "6,000 stacked quantifiers must not crash/overflow the stack");
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "stacked quantifiers took too long: {:?}",
        start.elapsed()
    );
}

#[test]
fn long_alternation_chain_does_not_overflow_stack() {
    // compile_alt recurses once per alternation branch; a 50,000-branch
    // chain on the default 2 MiB stack is a confirmed stack-overflow
    // abort. Run on a small 256 KiB stack and require the call to return
    // (Ok or Err) rather than crash.
    let start = Instant::now();
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let pattern = "a|".repeat(50_000) + "a";
            let _ = Regex::new(&pattern);
        })
        .expect("failed to spawn small-stack thread");

    assert!(handle.join().is_ok(), "50,000-branch alternation chain must not crash/overflow the stack");
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "long alternation chain took too long: {:?}",
        start.elapsed()
    );
}

#[test]
fn repeated_leading_case_insensitive_prefix_is_ok() {
    let result = Regex::new("(?i)(?i)a");
    assert!(result.is_ok(), "repeated leading (?i) prefix must be Ok, got {result:?}");
}
