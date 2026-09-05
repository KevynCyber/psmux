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

## ZDEP-010 chrono strftime replaced by src/timefmt.rs

`src/timefmt.rs` provides `LocalTime` (year, month, day, hour, minute,
second, millisecond, weekday) obtained from `GetLocalTime` (declared in
`src/win32.rs`, `#[cfg(windows)]`; unix fallback via `localtime_r` is out of
scope: psmux is Windows-only) and `strftime(&LocalTime, fmt) -> Option<String>`.
Supported specifiers are exactly the documented status-line set
(`docs/configuration.md`): `%H %I %M %S %p %R %d %b %Y %a`, plus `%m`, `%e`
(space-padded day), `%.3f` (milliseconds, 3 digits) and `%%`. English
weekday/month abbreviation tables are built in. Any other `%` sequence
(including a trailing `%`) returns `None`, mirroring chrono 0.4.45 where
`write!` of a `DelayedFormat` with an unknown specifier fails and the caller
keeps the unformatted string (`src/format.rs` `expand_format_for_window`,
`Modifier::ExpandTime`). `chrono::Local::now().format(..)` call sites in
`src/format.rs`, `src/debug_log.rs`, `src/window_ops.rs`, `src/platform.rs`,
`src/client.rs` use `timefmt`. Fixture
`tests-rs/fixtures/strftime_chrono_0.4.45.txt` is generated once from
chrono 0.4.45 (3 fixed `LocalTime` values x every supported specifier, the
fixed formats `%a %b %e %H:%M:%S %Y` and `%H:%M:%S%.3f`, and 3 unknown
specifiers) and committed; the acceptance test replays it with 0 mismatches.
Tests: `tests-rs/test_zdep_timefmt.rs`

## ZDEP-011 chrono clock and epoch conversions replaced by std time

`timefmt::local_from_epoch_secs(i64) -> Option<LocalTime>` converts a UTC
epoch-seconds value to local time via `FileTimeToLocalFileTime` +
`FileTimeToSystemTime` (`src/win32.rs`), returning `None` when the value is
outside the FILETIME range (before 1601-01-01 or past year 30827) or the
conversion fails; `Modifier::Time` in `src/format.rs` uses it and keeps the
raw value on `None`. `AppState::created_at` (`src/types.rs`) becomes a
`std::time::Instant` and the session-age computation in `src/server/mod.rs`
uses `elapsed().as_secs()`; the two `chrono::Utc::now().timestamp()` calls
in `src/server/connection.rs` use
`SystemTime::now().duration_since(UNIX_EPOCH)` seconds. The `chrono`
manifest line is removed and `cargo tree` no longer lists `chrono`,
`iana-time-zone`, `num-traits` (unless pulled by another crate), or
`windows-link` via chrono.
Tests: `tests-rs/test_zdep_timefmt.rs`, `tests-rs/test_zdep_time_std.rs`

## ZDEP-012 unicode-width char width replaced by crates/psmux-unicode

`crates/psmux-unicode` (workspace member, zero dependencies) provides
`char_width(char) -> Option<usize>` with exactly unicode-width 0.2.2's
`UnicodeWidthChar::width` semantics: `None` for the Cc general category
(U+0000..U+001F, U+007F..U+009F), `Some(0)` for zero-width characters
(Mn/Me/Cf, ZWJ, ZWNJ, U+1160..U+11FF Hangul jamo V/T, ...), `Some(2)` for
East Asian Wide/Fullwidth and emoji-presentation characters, `Some(1)`
otherwise (including unassigned and private-use code points). The table is
`crates/psmux-unicode/src/tables.rs`, emitted by
`scripts/gen_unicode_width.py` from the committed oracle fixture
`tests-rs/fixtures/unicode_width_0.2.2.txt` (run-length rows
`<start-hex>..<end-hex>|<N|0|1|2>` covering U+0000..U+10FFFF minus
surrogates, generated once from unicode-width 0.2.2). Deviation from the
plan's A6: the generator consumes the oracle fixture, not UCD files, so no
UCD inputs are checked in and the table equals the fixture by construction.
Acceptance: the fixture replays with 0 mismatches over every code point;
`scripts/test_gen_unicode_width.py` regenerates `tables.rs` byte-identical.
Tests: `crates/psmux-unicode/tests/char_width_fixture.rs`,
`scripts/test_gen_unicode_width.py`

## ZDEP-013 unicode-width str width replaced by psmux_unicode::str_width

`psmux_unicode::str_width(&str) -> usize` reproduces unicode-width 0.2.2's
`UnicodeWidthStr::width` for every row of the committed string fixture
`tests-rs/fixtures/unicode_str_width_0.2.2.txt` (rows
`<category>|<string with \u{XXXX} escapes>|<width>`, generated once from
unicode-width 0.2.2). Declared categories, each present in the fixture:
ascii, cjk, thai clusters (the `width_thai_441` strings), combining marks,
control characters (C0, DEL, C1, `\n`, `\t`, `\r`), U+FE0F VS16 after an
emoji-presentation-capable base (the `issue533_vs16_width` strings) and
after a non-emoji base, U+FE0E VS15 after an emoji base, ZWJ emoji
sequences, regional-indicator pairs, Hangul jamo L+V+T sequences, empty
string. The exact widths the crate assigns (notably for control characters
and VS16) are whatever the fixture records; the implementation is the
sum of `char_width` plus the sequence rules needed for 0 fixture mismatches.
Sequences outside the declared categories are an accepted risk (plan ledger
item 12). All call sites switch to `psmux_unicode` with fallbacks unchanged:
`UnicodeWidthChar::width(..).unwrap_or(0)` in `src/style.rs` (2 sites),
`unwrap_or(1)` in `src/client.rs`, `src/preview.rs`,
`crates/vt100-psmux/src/cell.rs`, `crates/vt100-psmux/src/screen.rs`;
`UnicodeWidthStr::width` in `src/client.rs`, `src/layout.rs`,
`src/rendering.rs`, `src/style.rs`, `crates/vt100-psmux/src/screen.rs`
(`wants_wide_promotion`), `examples/pipeline_diag.rs`,
`tests-rs/test_client.rs`. The `unicode-width` lines are removed from the
root and `crates/vt100-psmux` manifests, `Cargo.lock`, and the crate-tree
golden; `crates/psmux-unicode` is added to `[workspace] members` and as a
path dependency of both.
Tests: `crates/psmux-unicode/tests/str_width_fixture.rs`,
`crates/vt100-psmux/tests/width_thai_441.rs`,
`crates/vt100-psmux/tests/issue533_vs16_width.rs`
