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

## ZDEP-004 which crate replaced by a PATH/PATHEXT walk

`src/which.rs` exposes `which(name: &str) -> Option<PathBuf>` (reads `PATH`
and `PATHEXT` from the environment) and the pure
`which_in(name: &str, path: &str, pathext: &str) -> Option<PathBuf>`.
Semantics match the `which` crate on Windows: a name containing a path
separator is checked directly (as-is when it already has an extension, then
with each `PATHEXT` extension appended); otherwise each `PATH` entry is
scanned in order and, within one entry, a name that already carries an
extension is tried as-is before the `PATHEXT` extensions are appended in
`PATHEXT` order; extension comparison is case-insensitive; empty `PATH`
entries are skipped; an empty `PATHEXT` falls back to `.COM;.EXE;.BAT;.CMD`;
the current directory is not searched. All `which::which` call sites in
`src/` and `tests-rs/` use it; the `which` manifest line is removed.
Tests: `tests-rs/test_zdep_which.rs`

## ZDEP-005 glob crate replaced by a native matcher

`src/globmatch.rs` exposes `glob_match(pattern: &str, name: &str) -> bool`
(`*` any run of characters except a path separator, `?` one such character,
`[abc]`/`[a-z]`/`[!a-z]` classes, `[` without a closing `]` is literal,
case-sensitive) and `glob(pattern: &str) -> Vec<PathBuf>` which expands a
pattern component by component over `read_dir`, returning matches sorted by
path; a pattern with no wildcard yields the path itself only when it exists.
`source-file` wildcard handling (`src/server/mod.rs`) uses it; the `glob`
manifest line is removed.
Tests: `tests-rs/test_zdep_glob.rs`

## ZDEP-006 anyhow removed from the root crate

`crates/portable-pty-psmux` re-exports its error type as
`portable_pty::Error`; `src/proxy_pane.rs` names that type in its
`MasterPty` impl and builds errors from `std::io::Error` (converted with
`into()`), so every error it returns downcasts to `std::io::Error`. The
root `anyhow` manifest line is removed (the sub-crate keeps its own until
its slice).
Tests: `tests-rs/test_zdep_proxy_pane_errors.rs`

## ZDEP-007 base64 crate replaced by the util codec

`src/util.rs` gains byte-level `base64_encode_bytes(&[u8]) -> String` and
`base64_decode_bytes(&str) -> Option<Vec<u8>>` (standard alphabet, `=`
padding, whitespace skipped, unpadded input accepted, invalid characters
reject the payload); the existing string variants wrap them. Cross-session
screen transfer (`src/cross_session_server.rs`) uses the byte variants; the
`base64` manifest line is removed and `tests-rs/test_deps_base64_parity.rs`
tests the util codec against the RFC 4648 vectors and an all-byte
round-trip instead of the crate.
Tests: `tests-rs/test_deps_base64_parity.rs`

## ZDEP-008 windows-sys replaced by extern "system" declarations

`src/win32.rs` (`#[cfg(windows)]`) declares, in `extern "system"` blocks
linked to `kernel32` and `user32`, exactly the items previously imported from
`windows-sys`: `GlobalAlloc`, `GlobalLock`, `GlobalUnlock`, `GlobalSize`,
`GlobalFree`, `GMEM_MOVEABLE` (= 2), `OpenClipboard`, `CloseClipboard`,
`EmptyClipboard`, `GetClipboardData`, `SetClipboardData`, `GetDriveTypeW`,
plus `DRIVE_REMOTE` (= 4). `src/clipboard.rs` and `src/server/mod.rs` use
them; the `windows-sys` manifest table is removed; the unsafe-inventory
allowlist counts are unchanged (edition 2021 extern blocks carry no `unsafe`
token).
Tests: `tests-rs/test_zdep_win32.rs`

## ZDEP-009 parking_lot dev-dependency replaced by std sync

`tests-rs/test_pane_writer_queue.rs` and
`tests-rs/test_pane_writer_transient_error.rs` use `std::sync::Mutex` and
`std::sync::Condvar::wait_while` with identical assertions; the
`parking_lot` dev-dependency line is removed.
Tests: `tests-rs/test_pane_writer_queue.rs`,
`tests-rs/test_pane_writer_transient_error.rs`
