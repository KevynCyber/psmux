# Plan: psmux with zero third-party crates (native Rust replacements)

GOAL: Every Cargo.lock in the project (root workspace + tests/monitor) contains ZERO `source = "registry+` entries; psmux builds with `--locked` and
passes CI on x86_64/i686/aarch64-pc-windows-msvc; user-visible behaviour unchanged.
CRITERIA: `grep -c registry+ Cargo.lock tests/monitor/Cargo.lock` == 0 AND all committed fixture tests green AND monitor --snapshot golden
identical.
SIGNALS: CI green on 3 test targets; monitor --snapshot golden; fixture tests.

## Goal

Ship a supply-chain-free psmux binary (only rustc/cargo/std trusted) with no feature regression. Decision this drives: whether psmux can ship
without third-party crates at all.

## Recon facts (from plan-v0 + plan-v1 corrections)

- Windows-only: release.yml targets x86_64/i686/aarch64-pc-windows-msvc; 0 `cfg(unix)`/`cfg(not(windows))` blocks in src/. README claims Windows
  only.
- Windows-target normal deps: 93 crates (97 incl. build). 246 packages in lock.
- Direct deps (root): ratatui, crossterm, portable-pty (path fork), which, chrono, vt100 (path fork), unicode-width, serde(derive), serde_json,
  regex, glob, anyhow, base64, windows-sys. dev: parking_lot.
- Fork portable-pty-psmux deps: anyhow, downcast-rs, filedescriptor, libc, log, nix, serial2, shell-words, lazy_static, shared_library, winapi,
  winreg. Windows path = win/conpty.rs(238) win/mod.rs(270) win/procthreadattr.rs(72) win/psuedocon.rs(346) + cmdbuilder.rs(787) + lib.rs(446). Uses
  CreatePseudoConsole/ResizePseudoConsole/ClosePseudoConsole with custom flags RESIZE_QUIRK=0x2, WIN32_INPUT_MODE=0x4, PASSTHROUGH_MODE=0x8
  (psuedocon.rs:31-33), PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE=0x00020016. In win/*.rs only winapi, filedescriptor, anyhow, shared_library, lazy_static
  are referenced. 4 #[test] by count, 0 test files.
- Fork vt100-psmux deps: vte 0.15 (Perform impl at perform.rs:33, methods print/execute/esc_dispatch/csi_dispatch/osc_dispatch; hook/put/unhook NOT
  implemented), itoa (term.rs:593 extend_itoa), unicode-width (cell.rs:1, screen.rs:2, screen.rs:1156). ~4.9k lines. 49 #[test].
- Root crate: 2893 #[test]; tests-rs/ has 11 integration tests; tests/ has ps1/py/cs/sh scripts (smoke, win10 ssh pipe mouse); tests/monitor is a
  SEPARATE workspace (own Cargo.lock), built with `cargo build --release` in CI and run headless with `--snapshot`.
- ratatui surface used: Rect(23) Position(19) Size, Style/Color/Modifier (Color::{named..., Reset, Indexed, Rgb}; Modifier BOLD DIM ITALIC
  UNDERLINED SLOW_BLINK REVERSED HIDDEN CROSSED_OUT), Text/Line/Span (150+), Paragraph(45) Block(29, BorderType Plain/Thick/Rounded/Double)
  Clear(42), f.render_widget (68 in client.rs), Terminal::new(backend) main.rs:4606, CrosstermBackend<PsmuxWriter> inside custom PsmuxBackend
  platform.rs:4555. NO Layout/Constraint solver, NO direct Buffer API.
- crossterm surface used: event::{poll, read} (ssh_input.rs:470,498), Event::{Key, Resize, Paste, Mouse}, KeyCode(81 sites) KeyModifiers(62)
  KeyEvent(19) KeyEventKind Press/Release/Repeat (8 sites client.rs, 2 ssh_input.rs), MouseEventKind/MouseButton, enable/disable_raw_mode
  (main.rs:4552/4657), Enter/LeaveAlternateScreen, Enable/DisableMouseCapture, Enable/DisableBracketedPaste (main.rs:4576), terminal::size
  (main.rs:4150), style::Print, execute!. No SetTitle/cursor commands.
- Small crates: regex 5 sites format.rs:833,873,892,896 (+regex::escape) with USER-SUPPLIED patterns (Substitute/Match/SearchContent format
  modifiers). serde derives on ~14 types (client.rs:24,1731-1784; layout.rs:113-135; util.rs:147-160); serde_json from_str x11, to_string x5, no
  Value. base64 STANDARD encode/decode x4 (cross_session_server.rs:91,223). chrono Local::now().format with "%H:%M:%S%.3f" and USER-SUPPLIED
  strftime strings (format.rs:249,762) + DateTime conversions format.rs:734. glob x1 (server/mod.rs:4239, user path). which x18 (hardcoded
  pwsh/powershell/cmd). anyhow x6 (proxy_pane.rs, not in pub API). unicode-width x26 in root.
- windows-sys: 4 items only (GlobalAlloc family / clipboard, GetDriveTypeW); src/ already hand-declares ~64 `extern "system"` fns (platform.rs 45),
  107 `unsafe` sites. build.rs uses only std.
- Local toolchain: no cargo-audit/deny/vet/geiger installed; CI installs cargo-audit and runs `cargo audit --deny warnings` on both locks.
- format.rs:827-830 prepends `(?i)` to user patterns for case-insensitive modifiers; error paths differ per modifier: Substitute returns the value
  unchanged, Match returns "0", SearchContent returns empty.
- crates/vt100-psmux/src/cell.rs:52 uses `c.width().unwrap_or(1)`: relies on unicode-width's None (control chars) vs Some(0) (combining)
  distinction.
- src/layout.rs serde types use `#[serde(tag = "type")]` internally-tagged enum, `rename`, and several `default` attributes.
- crates/portable-pty-psmux/src/win/psuedocon.rs:55-57 resolves ConPTY exports with `.expect()` (panics if missing); `supports_passthrough_mode()`
  gates the PASSTHROUGH flag on RtlGetVersion (Win10 vs 22H2+); no test covers it and CI runs only on current Windows Server.
- derive_more (and thus syn/quote/proc-macro2) is pulled by crossterm, not only ratatui. windows-sys is pulled transitively by crossterm/ratatui.
- tests-rs/test_pane_writer_queue.rs:67-76 uses parking_lot::Condvar::wait().
- ~half of cmdbuilder.rs is `#[cfg(unix)]`.
- CI also runs a posix-helper-tests job (ubuntu/macos) for shell installer / wrapper scripts; unaffected by Rust deps.
- CI runs `cargo test` only on x86_64; i686/aarch64 are build-only.
- cross-session peers feed bytes straight into the VT parser (proxy_pane.rs:259 `p.process(&snap)`); control-mode JSON from_str sites:
  client.rs:4835, preview.rs.

## Assumptions (A1-A14)

A1. Windows-only support matrix; non-Windows code in forks is deleted. The CI posix-helper job (shell scripts only) stays untouched.
A2. Regression oracle = 2893 root + 49 vt100 tests, tests-rs, CI smoke + win10 ssh pipe mouse ps1, monitor --snapshot golden, PLUS the committed
fixture files this plan adds. Full suite runs only in CI/disposable env.
A3. Trusted and out of scope: rustc, cargo, std, the pinned toolchain, GitHub Actions runners. Their advisories are tracked by pinning the toolchain
channel and reading Rust release notes (documented in README).
A4. Public behaviour unchanged: CLI, config, control-mode JSON field names AND field order (derive order) AND serde attribute semantics
(tag/rename/default); JSON whitespace may differ. Each regex call site keeps its current error path (no format.rs control-flow change).
A5. Regex grammar = tmux-style ERE (literals . [] [^] ^ $ * + ? {m,n} | () \d \w \s \b, group captures) PLUS the `(?i)` inline flag prefix format.rs
emits. No lookaround, no \p{}. Parser nesting depth capped at 64.
A6. unicode-width 0.2.2 semantics reproduced exactly, including the None (Cc/unassigned control) vs Some(0) (Mn/Me/Cf/ZW) split; generator consumes
EastAsianWidth.txt, DerivedGeneralCategory.txt and emoji-variation-sequences.txt. Oracle = committed fixture generated once from unicode-width 0.2.2
(range-run-length table of expected widths).
A7. ConPTY exports resolved at runtime via LoadLibraryW/GetProcAddress into a std::sync::OnceLock; missing exports become an io::Error surfaced as
"ConPTY unavailable" (today: panic). RtlGetVersion passthrough gate ported verbatim with a unit test on the version predicate.
A8. Shared zero-dep code lives in workspace path crates so both the bin-only root and the separate tests/monitor workspace can consume it:
crates/psmux-tui (ratatui replacement), crates/psmux-json, crates/psmux-unicode. tests/monitor adds path deps on them.
A9. Each slice lands on master separately, shippable, revertible by `git revert`; no long-lived branch.
A10. Versions: 3.4.x per behaviour-preserving slice; 4.0.0 at S9.
A11. parking_lot -> std Mutex + std Condvar::wait_while in tests.
A12. Proc-macro crates disappear only when BOTH crossterm and ratatui are gone (S7+S8); per-slice criteria count manifest lines, tree absence is
asserted at S9 only.
A13. Hand-written parsers fed untrusted bytes (VT, JSON, regex, base64, glob) get permanent in-repo tests: fixture corpora + a deterministic std-
only random-bytes no-panic smoke test (seeded xorshift PRNG, N=1M bytes).
A14. New unsafe FFI is gated: `#![deny(unsafe_op_in_unsafe_fn)]`, every extern decl carries a doc comment with the Win32 signature source, and a
ratchet test (tests-rs/test_unsafe_inventory.rs) pins unsafe-block counts per file to an allowlist that must be edited deliberately.

## Assumptions and design decisions

- A1 (Windows-only, delete non-Windows fork code): why -- release.yml only ever targeted Windows; dead code has no test coverage and blocks the
  zero-dep count.
- A2 (existing suite + new fixtures is the oracle): why -- no third-party fuzz/diff harness is allowed to survive past dev (ledger 9 REJECT of
  throwaway harnesses).
- A3 (rustc/cargo/std trusted): why -- "third-party" must have a boundary or the goal is unreachable; toolchain advisories tracked separately, not
  by cargo-audit (ledger 18 REJECT: cargo-audit never covered rustc anyway).
- A4 (byte-for-byte public behaviour, JSON field order preserved): why -- control-mode consumers may rely on derive order even though whitespace is
  not guaranteed.
- A5 (ERE + `(?i)` grammar, depth cap 64): why -- corrects plan-v0's oversight that format.rs already emits `(?i)` (ledger 1 ACCEPT); depth cap
  forecloses ReDoS via nesting (ledger 19 ACCEPT). Extended round 2: replacement-string grammar is regex-crate syntax ($N, ${N}, ${name}, $$ literal)
  since the replacement grammar was never inventoried before (ledger 31 ACCEPT).
- A6 (unicode-width None/Some(0) split via general-category data): why -- vt100 cell width test relies on the distinction; naive full-sweep
  differential was oversized so it is kept only as a committed run-length fixture, not a throwaway (ledger 12 PARTIAL).
- A7 (OnceLock + io::Error, not panic): why -- deliberate small behaviour improvement over today's `.expect()` panic; corrects plan-v0's false
  assumption that the loader already degrades gracefully (ledger 7 ACCEPT, both halves: predicate kept, loader errors instead of panics).
- A8 (path crates for shared code): why -- tests/monitor is a separate workspace and cannot path-dep a bin-only crate; adding lib.rs to root was
  rejected (R5) as it drags 50k lines of bin code into monitor's compile (ledger 6 ACCEPT).
- A9 (one slice per master commit, revertible): why -- keeps a wrong replacement recoverable without a long branch (R3 rejected: unverifiable big-
  bang).
- A10 (3.4.x then 4.0.0 at S9): why -- dependency-policy change is user-visible, warrants a major bump only once true.
- A11 (parking_lot -> std): why -- only a dev/test dependency, trivial swap, no design risk.
- A12 (proc-macro absence asserted only at S9): why -- derive_more/syn/quote/proc-macro2 are pulled by crossterm too, not just ratatui; asserting
  early would fail every intermediate slice (ledger 8 ACCEPT).
- A13 (fixture corpora + seeded no-panic smoke, no cargo-fuzz): why -- hand-written parsers replacing fuzz-hardened crates need a permanent, in-
  repo, zero-dep safety net; cargo-fuzz itself is third-party (ledger 16 ACCEPT).
- A14 (unsafe ratchet + doc-comment rule): why -- Miri cannot run Win32 and geiger is third-party, so growth in unsafe FFI surface is gated by a
  test instead (ledger 17 ACCEPT with limits).
- Ledger 11 (Pike VM oversized) REJECTED: patterns are user-supplied so a linear-time captures engine is the minimum both correct (Substitute needs
  captures) and ReDoS-safe; grammar is already capped by A5.
- Ledger 18 (cargo-audit removal loses rustc advisories) REJECTED as framed: cargo-audit only ever audited Cargo.lock crates, not rustc; mitigation
  folded into A3/S9 README note instead.
- Ledger 12 (full-range unicode sweep) PARTIAL: kept as a committed run-length fixture rather than dropped or left as a throwaway branch artifact.

## Slices

S0. Gate, goldens, ratchets.
    - tests-rs/test_zero_third_party_deps.rs (#[ignore] until S9) + the unsafe-inventory ratchet test (enforced from S0); both authored in S0.
    - tests-rs/fixtures/: monitor --snapshot golden (tests-rs/fixtures/monitor_snapshot.txt); `cargo tree -e normal --target x86_64-pc-windows-msvc`
    crate list.
    - CI: add `cargo test` on i686 (WoW64 on the x86_64 runner); pilot `cargo test --target i686-pc-windows-msvc` on the full suite (failures fixed
    in S0 or job scoped to tests-rs with a ledger note); add windows-11-arm test job as a separate `continue-on-error: true` job (Open Q3
    unconfirmed); add the test-monitor job diffing `--snapshot` output against the committed golden (normalising line endings); add `--locked` to
    the 5 invocations (3 build targets, test job build+test, monitor build).
    CRITERIA: both new tests run (gate test ignored, unsafe-inventory ratchet enforced); 2 fixtures committed; i686 counts as the second enforced
    test target, arm is informational; monitor golden diffed in CI; `--locked` on all 5 invocations.
    SIGNALS: CI matrix shows cargo test on 3 targets, i686 enforced green, arm informational.

S1a. Trivial std swaps (one commit each): which -> PATH/PATHEXT walk; glob -> `*` `?` `[..]` matcher over read_dir (server/mod.rs:4239 only); anyhow
-> io::Error (proxy_pane.rs); base64 STANDARD codec; windows-sys 4 items -> extern "system" decls; parking_lot -> std Mutex/Condvar (tests-rs).
which fixture adds an arbitrary extension-less basename plus a PATHEXT order case (pane.rs:1266 uses a variable basename).
    CRITERIA: RFC 4648 vectors pass; glob table passes; which finds pwsh/powershell/cmd and the extension-less/PATHEXT-order fixture case; proxy_pane
    tests pass; 6 manifest lines removed.
    SIGNALS: `cargo check` and targeted `cargo test` green.

S1b. chrono -> crates/psmux-time (or src/timefmt.rs): GetLocalTime + GetTimeZoneInformation FFI; strftime specifiers = the set documented in docs/
for status-line/format strings (grunt inventories docs/*.md + format.rs before coding) plus "%H:%M:%S%.3f"; unknown specifiers echoed exactly as
chrono does today (verify chrono's behaviour in a fixture). Adds epoch-seconds -> local conversion (FileTimeToLocalFileTime or
GetTimeZoneInformation math, None on out-of-range) replacing DateTime::from_timestamp(ts,0).into() (format.rs:734); English weekday/month name
tables for the fixed format "%a %b %e %H:%M:%S %Y" (format.rs:736,1234,1236,1781); fixture covers this exact format string.
    CRITERIA: fixture table generated from chrono 0.4.45 covers 100% of documented specifiers at 3 timestamps, 0 mismatches; format.rs tests pass;
    "%a %b %e %H:%M:%S %Y" fixture passes; out-of-range timestamp returns None; chrono manifest line removed.
    SIGNALS: none.

S2. unicode-width -> crates/psmux-unicode: scripts/gen_unicode_width.py (inputs checked in under scripts/unicode/) emits range tables for char width
Option<usize> and str width. Fixture: expected widths for U+0000..U+10FFFF as run-length ranges, generated once from unicode-width 0.2.2 and
committed (tests-rs/fixtures/unicode_width_0.2.2.txt).
    CRITERIA: fixture test 0 mismatches; vt100 tests width_thai_441 and issue533_vs16_width pass; cell.rs:52 semantics test (control -> None,
    combining -> Some(0)) passes; both call-site fallbacks (src/style.rs:254,483 unwrap_or(0); vt100 cell.rs:52 unwrap_or(1)) stay untouched --
    replacement returns Option<usize> exactly like unicode-width; unicode-width removed from root + vt100 manifests.
    SIGNALS: none.

S3. serde/serde_json -> crates/psmux-json: Value + parser (depth limit 128, RFC 8259 escapes incl. surrogate pairs, max input guarded by caller) +
writer + ToJson/FromJson traits. Manual impls for every derived type; a grunt inventories EVERY #[serde(...)] attribute (tag, rename, default, skip,
flatten) before coding; attribute inventory MUST include skip_serializing_if (layout.rs:126 uses skip_serializing_if = "Option::is_none"); each
attribute gets a round-trip test, field-absence tested for skip_serializing_if. tests/monitor takes a path dep. Fixture: JSON emitted by serde_json
for each type at a sample value, committed; new writer must match after whitespace normalisation, and must parse the old bytes back.
    CRITERIA: fixture tests pass; malformed/deep-nesting/invalid-escape cases pass; random-bytes no-panic smoke passes; tests-rs control-mode tests
    pass; 1 round-trip test per serde attribute occurrence incl. skip_serializing_if field-absence; serde + serde_json removed from root + monitor
    manifests.
    SIGNALS: monitor --snapshot golden identical.

S4. regex -> crates/psmux-regex: ERE+`(?i)` parser (depth cap 64, iterative compile) -> NFA -> Pike VM with capture slots; `escape()`; replacement-
string grammar per A5 ($N, ${N}, ${name}, $$ literal). format.rs call sites unchanged except the type path; each keeps its own error branch.
Substitute is first-occurrence-only (format.rs:834 currently uses Regex::replace, first match only -- must NOT become replace_all). Fixture:
>=100 (pattern, input, expected captures) triples generated once from regex 1.13 and committed, plus a fixture with 2+ matches asserting only the
first is replaced, plus $1 and literal-$ replacement-string cases.
    CRITERIA: fixture test (>=100 triples) passes; `(a*)*b` on 10k a-bytes completes < 10 ms; nested-group depth 1000 pattern -> Err with no stack
    overflow; first-occurrence-only fixture (2+ matches) passes; $1/${name}/$$ replacement-grammar fixtures pass; format.rs tests pass; regex
    removed.
    SIGNALS: none.

S5. vt100-psmux: drop vte + itoa. src/vt_parser.rs = full DEC ANSI state machine (Paul Williams) including DCS entry/param/passthrough/ignore and
OSC/SOS/PM/APC string states, feeding the existing Perform-shaped trait. Fixture corpus: recorded pwsh/vim/htop-like/ConPTY output files + expected
callback stream (generated once via vte 0.15, committed).
    CRITERIA: 49 vt100 tests pass; corpus 0 mismatches; 1M random-bytes no-panic; tests-rs vt100/osc/sgr tests pass; vte + itoa removed from vt100
    manifest.
    SIGNALS: none.

S6. portable-pty-psmux -> src/pty/ (ConPTY only). Extern decls replace winapi; OnceLock replaces lazy_static+shared_library (A7 error semantics);
own HANDLE wrapper replaces filedescriptor; io::Error replaces anyhow; delete unix.rs, serial.rs, cmdbuilder `#[cfg(unix)]` halves, and
nix/libc/serial2/shell-words/downcast-rs/log/winreg. Keep API names used by src/. Port the three flag constants, the supports_passthrough_mode
RtlGetVersion predicate (unit-tested with injected version numbers) and the Windows cmdbuilder quoting (with its 4 tests). Remove
crates/portable-pty-psmux from root [workspace] members (Cargo.toml:12-24).
    CRITERIA: cmdbuilder tests pass; version-predicate test passes; spawn/resize/kill succeeds under a `-L` namespace; 12 manifest lines removed
    from the pty crate (crate folded into src/); crates/portable-pty-psmux removed from [workspace] members.
    SIGNALS: CI smoke ps1 + win10 ssh pipe mouse ps1 green.

S7. crossterm -> src/term/: GetConsoleMode/SetConsoleMode raw mode with the same flag set crossterm applies (grunt quotes crossterm's Windows raw-
mode flags before coding); alt screen / mouse / bracketed paste as VT sequences; EnableBlinking/DisableBlinking (main.rs:49,4576,4667) emitted as
CSI ? 12 h / l; ReadConsoleInputW -> Event (Press/Release/Repeat, mouse, resize); VT/CSI input parser for pipe/SSH mode. Event types keep shape.
Crossterm cannot be removed alone: src/platform.rs:4553 PsmuxBackend wraps ratatui::backend::CrosstermBackend and ratatui's default `crossterm`
feature pulls ratatui-crossterm -- so S7 also implements ratatui::backend::Backend natively (VT output from the Cell iterator: cursor moves, SGR
from Style, clear/append_lines) and sets `ratatui = { default-features = false, features = ["std","all-widgets","underline-color"] }` in Cargo.toml.
Fixture: INPUT_RECORD -> Event and bytes -> Event tables (copied from crossterm's tests where semantics must match).
    CRITERIA: fixture tests pass; native Backend impl passes the same cell-for-cell fixture S8 uses; blink CSI sequence test passes; ratatui manifest
    line sets default-features = false with std/all-widgets/underline-color; crossterm removed.
    SIGNALS: win10 ssh pipe mouse ps1 and smoke ps1 green.

S8. ratatui -> crates/psmux-tui: Rect/Position/Size; Style/Color/Modifier; Span/Line/Text; Buffer + diff; Terminal (double buffer, diff, cursor
restore) + Terminal::draw emitting VT to PsmuxWriter; Frame::set_cursor_position/area/buffer_mut and Block::inner (client.rs:1210,5108,5161,920,5310);
Paragraph (wrap, align, scroll), Block (borders, 4 BorderTypes, titles), Clear. tests/monitor takes a path dep. Fixture: for each widget config used
in src/ (grunt enumerates), render into a ratatui Buffer in a test and dump cells+styles to a committed file; psmux-tui must reproduce cell-for-
cell.
    CRITERIA: widget fixtures 0 mismatches; Terminal double-buffer diff + cursor-restore tests pass; Frame API + Block::inner call sites covered;
    ratatui removed from root + monitor.
    SIGNALS: monitor --snapshot golden identical; CI green.

S9. Zero-dep flip: remove remaining manifest entries, `cargo update`, un-ignore the gate test, replace CI cargo-audit steps with the gate test, pin
toolchain channel in rust-toolchain.toml, README "Zero third-party dependencies" section (+ how toolchain advisories are tracked), version 4.0.0.
    CRITERIA: registry+ count 0 in both locks; `cargo tree` lists only workspace crates; gate test enforced and green.
    SIGNALS: CI green on 3 targets; binary size/startup within 10% (informational).

## Evaluation criteria

S0: 2 tests added and green (gate ignored); 2 fixtures committed; CI matrix shows cargo test on 3 targets.
S1a: 6 manifest lines removed (which glob anyhow base64 windows-sys parking_lot); >=1 regression test per replacement module.
S1b: chrono manifest line removed; fixture table covers 100% of documented specifiers; 0 mismatches.
S2: unicode-width removed from root + vt100 manifests; fixture 0 mismatches.
S3: serde + serde_json removed from root + monitor manifests; 1 round-trip test per serde attribute occurrence; monitor golden identical.
S4: regex removed; >=100 fixture triples pass; 2 DoS tests pass.
S5: vte + itoa removed from vt100 manifest; corpus 0 mismatches; 49 tests.
S6: 12 manifest lines removed from the pty crate (crate folded into src/); predicate test + 4 cmdbuilder tests + smoke/ssh ps1 green.
S7: crossterm removed; fixture tables pass; ssh pipe mouse ps1 green.
S8: ratatui removed from root + monitor; widget fixtures 0 mismatches.
S9: registry+ count 0 (both locks); gate test enforced and green; CI green.

## External signals

- GitHub Actions ci.yml + release.yml green per slice (3 test targets after S0). posix-helper-tests job unchanged.
- monitor --snapshot golden compared at S3, S8, S9.
- Binary size / startup vs 3.3.8 (informational).

## Blast radius / rollback

Wrong S5-S8 = garbled rendering, dropped keys, unspawnable panes for every user. Mitigation: one slice per release; `git revert` of the slice
restores the manifest line and the lock still pins the old crate until S9. After S9, reverting S8 re-resolves from crates.io (acceptable). New
unsafe surface is ratcheted (A14) so growth is explicit in review. S7 cannot be reverted as a bare manifest add: since S7 must also implement
ratatui::backend::Backend natively and flip ratatui to default-features = false (crossterm removal forces this, item 23), a revert of S7 restores
both the crossterm manifest line AND the prior default-features ratatui config together -- still a real manifest revert, just two lines not one.

## Risks accepted

- Hand-written parsers replace fuzz-hardened crates. Accepted with committed corpora + seeded random no-panic tests; no cargo-fuzz (third-party).
- No Miri/geiger for FFI (Miri cannot run Win32; geiger is third-party). Accepted with the unsafe ratchet + doc-comment rule.
- Old-Windows-10 ConPTY branch cannot run in CI; covered only by the predicate unit test and manual check.

## Ledger (round 1)

1. regex `(?i)` prefix unsupported (correctness CRIT, assumptions MAJOR) -> ACCEPT, corroborated: A5 + S4 add inline flag.
2. regex error semantics differ per call site (correctness CRIT) -> ACCEPT: call sites unchanged, engine returns Result.
3. unicode-width None vs Some(0) (correctness MAJOR) -> ACCEPT: A6/S2 use general-category data + fixture.
4. layout.rs tagged enum / renames (correctness MAJOR) -> ACCEPT: S3 attribute inventory + per-attribute test.
5. DCS state machine fidelity (correctness MINOR) -> ACCEPT: S5 full state machine + corpus.
6. monitor cannot path-dep a bin-only crate (failure CRIT) -> ACCEPT: A8 workspace path crates psmux-tui/json/unicode.
7. supports_passthrough_mode gate droppable (failure CRIT) + A7 false, loader panics (assumptions CRIT) -> ACCEPT both: S6 ports predicate with
test; loader returns io::Error (deliberate small behaviour improvement).
8. windows-sys still transitive at S1 (failure MAJOR) + derive_more from crossterm (assumptions CRIT) -> ACCEPT: A12; per-slice criteria count
manifest lines; tree absence only at S9.
9. throwaway differential harnesses discard the oracle (failure MAJOR, security SIMPLICITY, simplicity MAJOR x2) -> ACCEPT: all fixtures are
generated once and committed (S1b, S2, S3, S4, S5, S7, S8).
10. S8 A/B frame harness unbuildable (simplicity MAJOR) -> ACCEPT: replaced by widget-level ratatui Buffer dumps.
11. Pike VM oversized for 3 sites (simplicity MAJOR) -> REJECT: patterns are user-supplied; linear-time engine with captures is the minimum that is
both correct (Substitute needs captures) and ReDoS-safe. Grammar is already capped by A5.
12. Full-range unicode sweep overshoots (simplicity MAJOR) -> PARTIAL: keep the sweep but as a small committed run-length fixture, not a throwaway.
13. chrono hidden in S1 (simplicity MINOR) -> ACCEPT: S1b split out with a docs-driven specifier inventory.
14. cmdbuilder unix dead code (simplicity MINOR) -> ACCEPT: deleted in S6.
15. CI tests only x86_64 (failure MINOR) -> ACCEPT: S0 adds i686 + arm test jobs.
16. no permanent VT fuzz target (security CRIT) -> ACCEPT via A13 (seeded random no-panic + corpus); cargo-fuzz rejected (third-party).
17. unsafe growth ungated (security CRIT) -> ACCEPT with limits: A14 ratchet + deny(unsafe_op_in_unsafe_fn) + doc-comment rule.
18. cargo-audit removal loses rustc advisories (security MAJOR) -> REJECT as framed: cargo-audit only audits Cargo.lock crates, not rustc.
Mitigation folded into A3/S9 README note.
19. regex nesting-depth DoS (security MAJOR) -> ACCEPT: depth cap 64, iterative compile, test.
20. JSON depth limit unspecified (security MINOR) -> ACCEPT: 128, tests at the two untrusted from_str sites.
21. parking_lot Condvar (assumptions MAJOR) -> ACCEPT: A11.
22. posix-helper CI job (assumptions MINOR) -> ACCEPT: External signals note.

## Ledger (round 2)

23. S7 cannot remove crossterm alone: src/platform.rs:4553 PsmuxBackend wraps ratatui::backend::CrosstermBackend, and ratatui's default `crossterm`
feature pulls ratatui-crossterm (failure CRIT) -> ACCEPT: S7 now also implements ratatui::backend::Backend natively (VT output from the Cell
iterator: cursor moves, SGR from Style, clear/append_lines) and sets `ratatui = { default-features = false, features = ["std","all-widgets",
"underline-color"] }`. Rollback of S7 is then a real manifest revert.
24. windows-11-arm test runner unconfirmed (failure CRIT) -> ACCEPT: S0 adds the arm test job as a separate `continue-on-error: true` job until
Open Q3 is answered; S0 CRITERIA counts i686 as the second enforced target, arm as informational.
25. Root [workspace] members is an explicit list (Cargo.toml:12-24) (failure MAJOR) -> ACCEPT: each new path crate is added to `members` in the
slice that creates it (S2 psmux-unicode, S3 psmux-json, S8 psmux-tui); S6 removes crates/portable-pty-psmux from members.
26. monitor --snapshot golden has no diff in CI (failure MAJOR) -> ACCEPT: S0 commits tests-rs/fixtures/monitor_snapshot.txt and the test-monitor
job diffs `--snapshot` output against it (normalising line endings).
27. No test has ever run on i686 (failure MAJOR) -> ACCEPT: S0 pilots `cargo test --target i686-pc-windows-msvc` on the full suite in CI; failures
found are fixed in S0 or the job is scoped to tests-rs with a ledger note.
28. ratatui double-buffer diff lives in Terminal::draw not Backend (failure MAJOR) -> ACCEPT: S8 deliverable list adds Terminal (double buffer,
diff, cursor restore) explicitly.
29. `--locked` enumeration (failure MINOR) -> ACCEPT: S0 CRITERIA lists 5 invocations (3 build targets, test job build+test, monitor build).
30. Substitute uses Regex::replace (first match only) at format.rs:834 (correctness CRIT) -> ACCEPT: S4 CRITERIA adds first-occurrence-only
semantics and a fixture with 2+ matches.
31. Replacement-string grammar ($1, ${name}, $$) never inventoried (correctness CRIT) -> ACCEPT: A5 extended with regex-crate replacement syntax
($N, ${N}, ${name}, $$ literal); S4 fixtures include $1 and literal $ cases.
32. EnableBlinking/DisableBlinking used at main.rs:49,4576,4667 (correctness MAJOR) -> ACCEPT: S7 emits CSI ? 12 h / l.
33. layout.rs:126 uses skip_serializing_if = "Option::is_none" (correctness MAJOR) -> ACCEPT: S3 attribute inventory must include
skip_serializing_if; field-absence tested.
34. Frame::set_cursor_position/area/buffer_mut and Block::inner used (client.rs:1210,5108,5161,920,5310) (correctness MAJOR) -> ACCEPT: added to
S8 scope.
35. Fixed format "%a %b %e %H:%M:%S %Y" at format.rs:736,1234,1236,1781 plus DateTime::from_timestamp(ts,0).into() at format.rs:734 (correctness
MAJOR x2) -> ACCEPT: S1b scope adds epoch-seconds -> local conversion (FileTimeToLocalFileTime or GetTimeZoneInformation math, None on
out-of-range) and English weekday/month tables; fixture covers this string.
36. which::which called with a variable basename at pane.rs:1266 (correctness MINOR) -> ACCEPT: S1a which fixture adds an arbitrary
extension-less basename and PATHEXT order case.
37. unwrap_or(0) in src/style.rs:254,483 vs unwrap_or(1) in vt100 cell.rs:52 (correctness MINOR) -> ACCEPT: S2 CRITERIA notes both call-site
fallbacks stay untouched; replacement returns Option<usize> exactly like unicode-width.

## Open questions

1. Is a 4.0.0 major bump acceptable at S9?
2. Should the ConPTY missing-export case panic as today or return an error (plan chooses error)?
3. Is windows-11-arm CI runner available on this repo's plan?

## Process notes

Super-plan run 2026-09-04. 6 Haiku recon grunts; 5 Sonnet adversaries (correctness, failure modes, security, simplicity, assumption audit). Round 2
and the Sonnet fresh-eyes critic were SKIPPED because the orchestrator hit the context ceiling after arbitration. Recommended next step: `/super-
plan` round-2 on lenses failure-modes + correctness against this file before implementation via /tdd, slice by slice.

Round 2 run 2026-09-04 (fresh session): 2 Sonnet adversaries (failure-modes, correctness) produced items 23-37; all ACCEPT. Fresh-eyes critic still
not run.
