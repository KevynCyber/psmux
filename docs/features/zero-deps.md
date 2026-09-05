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
East Asian Wide/Fullwidth and emoji-presentation characters, `Some(3)` for
U+17D8 KHMER SIGN BEYYAL (the single width-3 code point in unicode-width
0.2.2), `Some(1)` otherwise (including unassigned and private-use code
points). The table is
`crates/psmux-unicode/src/tables.rs`, emitted by
`scripts/gen_unicode_width.py` from the committed oracle fixture
`tests-rs/fixtures/unicode_width_0.2.2.txt` (run-length rows
`<start-hex>..<end-hex>|<N|0|1|2|3>` covering U+0000..U+10FFFF minus
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
root and `crates/vt100-psmux` manifests; `crates/psmux-unicode` is added to
`[workspace] members` and as a path dependency of both, and the crate-tree
golden gains `psmux-unicode v0.1.0`. `unicode-width v0.2.2` remains in
`Cargo.lock` and the golden only as a transitive dependency of
`ratatui-core` until S8 (plan ledger item 8: tree absence is an S9 check).
Tests: `crates/psmux-unicode/tests/str_width_fixture.rs`,
`crates/vt100-psmux/tests/width_thai_441.rs`,
`crates/vt100-psmux/tests/issue533_vs16_width.rs`

## ZDEP-014 serde_json Value, parser and writer replaced by crates/psmux-json

`crates/psmux-json` (workspace member, zero dependencies) provides
`Value` (`Null`, `Bool`, `Number`, `String`, `Array(Vec<Value>)`,
`Object(Vec<(String, Value)>)` in insertion order),
`psmux_json::parse(&str) -> Result<Value, Error>` and
`Value::to_string()` (compact, no whitespace, serde_json byte-for-byte
for every fixture row). Parser: RFC 8259 grammar only (no trailing commas,
comments, leading zeros, `NaN`/`Infinity`, single quotes, or unescaped
control characters); the four JSON whitespace bytes; every `\`-escape
including `\uXXXX` with UTF-16 surrogate pairs combined into one scalar,
a lone or mismatched surrogate is `Err`; the nesting limit is whatever
the fixture records (serde_json 1.0.151 with its default recursion limit
of 128 accepts 127 nested arrays/objects and rejects 128), enforced with
bounded recursion (a 10 000-deep `[` input must return `Err`, not
overflow the stack); trailing non-whitespace after
the value is `Err`; duplicate object keys keep the last; input size is
the caller's responsibility (plan S3). Writer: strings escape `"`, `\`,
and control characters below U+0020 as `\b \f \n \r \t` or `\u00XX`,
leave U+007F and non-ASCII unescaped; integers print in decimal; floats
print however the fixture records for the rows present (no psmux call
site serialises a float). `Value` implements `Index<&str>` and
`Index<usize>` (a missing key or index yields a shared `Null`),
`PartialEq<&str>`, `PartialEq<bool>`, `PartialEq<i64>` and accessors
`as_str`, `as_bool`, `as_i64`, `as_u64`, `as_f64`, `as_array`,
`as_object`, `get(&str)`, so `tests-rs/test_issue451_status_styles.rs`
compiles with `serde_json::Value` renamed to `psmux_json::Value`.
`Error` implements `Display` and `std::error::Error`.
Acceptance: every row of the committed oracle fixture
`tests-rs/fixtures/serde_json_1.0.151.txt` (rows `<kind>|<name>|<payload>`,
generated once from serde_json 1.0.151; kinds cover parse-ok round trips,
parse-err inputs, writer escapes, numbers, depth boundary) replays with
0 mismatches; a seeded random-bytes and random-mutated-JSON smoke test
(>= 100 000 inputs) never panics.
Tests: `crates/psmux-json/tests/value_fixture.rs`,
`crates/psmux-json/tests/no_panic.rs`

## ZDEP-015 serde derives replaced by manual psmux_json ToJson/FromJson impls

`psmux_json::ToJson` (`fn to_json(&self) -> Value`) and
`psmux_json::FromJson` (`fn from_json(&Value) -> Result<Self, Error>`)
with impls for `bool`, `u8`, `u16`, `u32`, `i32`, `i64`, `u64`, `usize`,
`f64`, `String`, `Option<T>` (`Null` <-> `None`), and `Vec<T>`, plus
`psmux_json::to_string<T: ToJson>(&T) -> String` and
`psmux_json::from_str<T: FromJson>(&str) -> Result<T, Error>`. Every
serde-derived type in `src/` gets a hand-written impl that reproduces the
derive semantics of its attributes exactly as inventoried: field order is
declaration order; a field without `#[serde(default)]` is required and
its absence is `Err`, except an `Option<T>` field which is `None` when
absent; `#[serde(default)]` substitutes `Default::default()`;
`#[serde(default = "f")]` calls `f`; unknown keys are ignored;
`#[serde(tag = "type")]` with `#[serde(rename = "split")]`/`"leaf"` on
`LayoutJson` and `LayoutSimple` reads and writes `"type"` as the first
key and rejects an unknown or missing tag; `CellRunJson.link` with
`skip_serializing_if = "Option::is_none"` is absent from the output when
`None` and present as a string when `Some` (field-absence asserted). Type
mismatches (`1.5` or `"1"` for a `u16`, `256` for a `u8`, `null` for a
required `String`) are `Err` like serde. Covered types: `CellJson`,
`CellRunJson`, `RowRunsJson`, `LayoutJson` (both variants) in
`src/layout.rs`; `WinInfo`, `PaneInfo`, `WinTree`, `LayoutSimple` in
`src/util.rs`; `FloatJson`, `WinStatus`, `BindingEntry`,
`ServerMenuItem`, `CustomizeOption`, `DumpState` (all fields incl. the
`default = "..."` functions) in `src/client.rs`; the local `Partial` in
`tests-rs/test_client.rs`. All 26 `serde_json::` call sites in `src/`
and `tests-rs/` switch to `psmux_json` with unchanged control flow
(`.ok()` fallbacks stay `.ok()`; `LayoutJson`/`LayoutSimple` parsing at
`src/preview.rs` and the `DumpState` parses in `src/client.rs` inherit the
depth limit from ZDEP-014, plan ledger item 20). `serde` and `serde_json`
lines are removed from the root manifest and `psmux-json` is added to
`[workspace] members` and as a root path dependency; the crate-tree golden
gains `psmux-json v0.1.0` and loses `serde`, `serde_core`, `serde_derive`,
`serde_json`, `zmij` (serde_json 1.0.151's float writer); `ryu`, `itoa`
and `memchr` stay, pulled by other normal dependencies. `serde`/`serde_json` remain in `Cargo.lock` via
`crates/vt100-psmux` dev-dependencies and `crates/portable-pty-psmux`'s
optional `serde_support` feature until S5/S6 (plan ledger item 8).
Acceptance: for every type at its fixture sample value the new writer
output equals the serde_json bytes and the serde_json bytes parse back
to an equal value (one round-trip test per serde attribute occurrence,
124 in the inventory, grouped per type/attribute kind); the field-absence
test for `link` passes; `cargo test --locked --bin psmux` control-mode
and layout tests pass unchanged.
Tests: `tests-rs/test_zdep_json_types.rs`,
`tests-rs/test_pane_wants_mouse_selection.rs`,
`tests-rs/test_issue451_status_styles.rs`, `tests-rs/test_client.rs`

## ZDEP-016 tests/monitor serde replaced by a psmux-json path dependency

`tests/monitor/Cargo.toml` replaces its `serde` and `serde_json` lines
with `psmux-json = { path = "../../crates/psmux-json" }`; `ResultRecord`
in `tests/monitor/src/parse.rs` gets a manual `FromJson` impl that reads
the PascalCase keys `Name`, `Status`, `Passed`, `Failed`, `Duration`
(f64, integer or fraction literal accepted), `ExitCode` (`null` or absent
-> `None`), errors on a missing required key, and ignores unknown keys;
the per-line parse in `parse.rs` keeps skipping malformed lines.
Acceptance: `cargo build --locked --release` in `tests/monitor` succeeds
with no serde in its lock; `psmux-test-monitor.exe --snapshot` output is
byte-identical to `tests-rs/fixtures/monitor_snapshot.txt` after CRLF
normalisation; a monitor unit test parses a fixture line with every key,
one with `ExitCode: null`, one with `ExitCode` absent, and one malformed
line.
Tests: `tests/monitor/src/parse.rs` (unit tests), monitor snapshot golden

## ZDEP-017 psmux-regex crate replaces the regex crate

`crates/psmux-regex` (no dependencies) exposes `Regex::new(&str) ->
Result<Regex, Error>`, `is_match(&str) -> bool`, `find(&str) ->
Option<(usize, usize)>`, `captures(&str) -> Option<Captures>` (byte-offset
spans, `Captures::get(i) -> Option<(usize, usize)>`, `name(&str)`),
`replace(&str, &str) -> String` (first match only) and `escape(&str) ->
String`. Semantics reproduce regex 1.13.1 on the supported subset:
leftmost-first (Perl) alternation, greedy and lazy `* + ? {m} {m,} {m,n}`
with stacking allowed, captures record the last iteration, empty
alternatives (`a|`, `(|a)+`) and `()` are valid; `^`/`$` are text-only
anchors, `.` excludes `\n`, `[^a]` includes `\n`, empty pattern matches
empty at 0; groups `()` `(?:)` `(?P<n>)` `(?<n>)` with ASCII names
`[A-Za-z_][A-Za-z0-9_]*` (duplicate name is Err); classes with ranges,
`]` first literal, `-` first/last literal, escapes, and ASCII-only POSIX
`[[:name:]]`/`[[:^name:]]`; `\d \w \s \D \W \S \b \B \n \t \r \f \v \a \xHH
\x{HHHH}` and the escaped literals `\` + any of `. + * ? ( ) | [ ] { } ^ $
# & - ~ / : ' " , = ! @ % _ ; \` backtick and space; `(?i)` ONLY as a
leading prefix (repeated prefix ok). Everything else the oracle accepts
but the engine does not (`\p{..}`, `(?s)` `(?m)` `(?x)` `(?u)` `(?R)`
`(?-i)` `(?i:..)`, mid-pattern `(?i)`, `\A \z \< \> \u{..}`, class set
operators `&& -- ~~`, nested classes, `[[:bogus:]]` `[[.a.]]` `[[=a=]]`,
non-ASCII group names) is Err and the fixture records them as
`unsupported`. Every oracle Err row is Err. Limits: nesting depth > 64
(groups + classes) is Err, `{m,n}` with n > 1000 is Err, a compiled
program > 100 000 instructions is Err. Compile-time native recursion is
bounded independently of pattern length: stacked quantifiers on one atom
(e.g. `a{1,1}{1,1}...`) and long `|` alternation chains must return from
`Regex::new` (Ok or Err) without overflowing a 256 KiB stack; a security
review on 2026-09-05 found both paths overflowed the default stack at
~5 000 stacked quantifiers / ~50 000 branches. Unicode simplification
(documented divergence from the oracle, fixture rows stay inside it):
`\d` is ASCII `0-9` only; `\w` is `char::is_alphanumeric() || '_'`; `\s` is
`char::is_whitespace()`; `\b` derives from `\w`. `(?i)`: pattern char x
matches text char c iff fold-set(x) intersects fold-set(c) where
fold-set(x) = {x, lower(x), upper(x), lower(upper(x))} using the std
single-char mappings (multi-char results ignored); class membership under
`(?i)` tests every element of fold-set(c), then negates for `[^..]`
(reproduces U+00DF~U+1E9E, k~U+212A, s~U+017F, U+03C3~U+03C2,
U+00E9~U+00C9, `(?i)[a-z]` on U+212A). Matcher is a Pike VM (no
backtracking): `(a*)*b` on 10 000 `a`s returns no match in under 1 s in
debug, a 1 000-deep `(` pattern is Err without stack overflow.
Replacement syntax: `$$` -> `$`, `$name` takes the longest
`[0-9A-Za-z_]+` run (`$1a` is group "1a" -> empty, `$01` is group 1),
`${name}`, `$0` whole match, nonexistent or unmatched group -> empty,
`$` followed by anything else or end -> literal `$`; no match -> text
unchanged. `escape` escapes exactly `\ . + * ? ( ) | [ ] { } ^ $ # & - ~`.
Acceptance: every row of the committed oracle fixture
`crates/psmux-regex/tests/fixtures/regex_1.13.1.txt` (generated once from
regex 1.13.1; rows `<kind>|<name>|<tab-separated fields>`, field escapes
`\\ \t \n \r \x{HH}`; kinds `match` (pattern, input, spans as
`0:0-4;1:-`), `err`, `unsupported`, `replace` (pattern, input,
replacement, output), `escape` (input, output); >= 100 match rows; the
replay parser rejects a row with the wrong field count) replays with 0
mismatches; the two DoS tests above pass.
Tests: `crates/psmux-regex/tests/regex_fixture.rs`,
`crates/psmux-regex/tests/no_backtrack.rs`

## ZDEP-018 format.rs regex call sites switched to psmux-regex

The three `regex::` call sites in `src/format.rs` (`Substitute`, `Match`
with `regex: true`, `SearchContent` incl. its `regex::escape` path)
switch to `psmux_regex` with unchanged control flow: `Substitute`
replaces the first match only and leaves the value unchanged on an
invalid pattern; `Match` returns `"0"` on an invalid pattern;
`SearchContent` returns `""` on an invalid pattern; all three prepend
`(?i)` when case-insensitive. The `regex` line is removed from the root
manifest, `psmux-regex` is added to `[workspace] members` and as a root
path dependency; the crate-tree golden gains `psmux-regex v0.1.0` and
loses `regex`, `regex-automata`, `regex-syntax`, `aho-corasick` (`memchr`
stays, pulled by `vte`).
Acceptance: `tests-rs/test_zdep_regex_sites.rs` (wired via
`src/tests_zdep_wiring.rs`) covers Substitute first-occurrence-only with
2+ matches, `$1`, `${1}`, `$$` and literal-`$` replacements, `(?i)`
Substitute and Match, an invalid pattern per site, and the escape path in
SearchContent; `tests-rs/test_format.rs` and
`tests-rs/test_issue476_bindkey_quoting.rs` pass unchanged; `cargo tree`
golden matches.
Tests: `tests-rs/test_zdep_regex_sites.rs`

## ZDEP-019 Crate-tree golden enforced by a test

`tests-rs/test_zdep_crate_tree.rs` (wired via `src/tests_zdep_wiring.rs`)
runs `cargo tree -e normal --target x86_64-pc-windows-msvc --prefix none
--locked` from the root manifest directory using the `CARGO` environment
variable, normalises the output (cut each line at its first ` (` annotation,
drop `\r`, trim, drop empty lines, sort, dedupe) and asserts it equals
`tests-rs/fixtures/crate_tree_x86_64.txt` normalised the same way. On a
mismatch the failure message lists the lines present only in the live tree
and the lines present only in the golden. ZDEP-003 describes the golden;
nothing enforced it before this test.
Tests: `tests-rs/test_zdep_crate_tree.rs`

## ZDEP-020 find_matching_brace balances bare braces inside #{...}

`find_matching_brace` in `src/format.rs` decremented depth on every bare
`}` but only incremented on `#{`, so a `${1}` group reference in an `s/`
replacement or a regex bounded repeat such as `o{,1}` inside `#{...}`
closed the expression early and leaked the tail (`X/:session_name}`) as
literal text. It now also increments depth on a bare `{` (tmux-style
balancing); `split_at_depth0` is unchanged.
Acceptance (session_name `modvar`): `#{s/(mod)var/${1}X/:session_name}`
-> `modX`; `#{s/o{,1}d/_/:session_name}` -> `m_var`;
`#{?#{==:#{session_name},modvar},yes,no}` -> `yes`; a literal after the
expression survives; a simple bare expression is unchanged.
Tests: `tests-rs/test_zdep_brace_match.rs` (wired via
`src/tests_zdep_wiring.rs`)
