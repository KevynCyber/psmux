# zero-deps feature area

Plan: `docs/plans/2026-09-04-zero-third-party-deps.md`. Goal: zero `registry+`
entries in both Cargo.lock files with user-visible behaviour unchanged.
(Spec lives here because this repo gitignores `.claude/`.)

## ZDEP-001 Zero-third-party gate test

`tests-rs/test_zero_third_party_deps.rs` asserts that `Cargo.lock` and
`tests/monitor/Cargo.lock` contain zero lines with `source = "registry+`.
`#[ignore]` until slice S9 flips the project; enforced from S9 on.
Tests: `tests-rs/test_zero_third_party_deps.rs`

## ZDEP-002 Unsafe inventory ratchet

`tests-rs/test_unsafe_inventory.rs` counts `unsafe` tokens per `.rs` file under
`src/`, `crates/*/src/` and `tests/monitor/src/` and compares against an
allowlist table inside the test. Any file whose count grows, or any new file
with `unsafe`, fails the test; a drop must also be recorded (ratchet both
ways). Added in slice S0; every later slice edits the table deliberately.
Tests: `tests-rs/test_unsafe_inventory.rs`

## ZDEP-003 Committed goldens and CI test matrix

Fixtures under `tests-rs/fixtures/`: `monitor_snapshot.txt` (output of
`psmux-test-monitor --snapshot`, 120x40, no run dir) and
`crate_tree_x86_64.txt` (`cargo tree -e normal --target
x86_64-pc-windows-msvc --prefix none` sorted, deduplicated). CI diffs the
monitor snapshot against the golden after line-ending normalisation, runs
`cargo test --all-targets` on x86_64 and i686 (enforced) and on aarch64 via a
`continue-on-error` windows-11-arm job (informational), and passes `--locked`
to every cargo build/test invocation.
Tests: `.github/workflows/ci.yml` (monitor golden diff step)
