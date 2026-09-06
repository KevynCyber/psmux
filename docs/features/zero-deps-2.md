# zero-deps feature area (continued)

Continuation of `docs/features/zero-deps.md` (that file is at the
markdown-quality-check length cap). Same plan and scope.

## ZDEP-021 vte replaced by vt_parser.rs

`crates/vt100-psmux` no longer depends on the `vte` crate.
`crates/vt100-psmux/src/vt_parser.rs` (with submodules `params.rs`,
`transitions.rs`, `actions.rs`) is an in-tree port of vte 0.15.0's std build
(MIT-licensed, source path cited in the module doc): the same Paul Williams
DEC ANSI state machine including DCS hook/put/unhook, OSC with BEL or ST
termination, `MAX_PARAMS` 32 (`ignore = true` beyond), `MAX_INTERMEDIATES`
2, `MAX_OSC_PARAMS` 16, u16 saturating params, `:` subparams, UTF-8 decoding
with U+FFFD replacement and partial sequences carried across `advance`
calls; `memchr` is replaced by a manual scan, vte's `MaybeUninit` OSC-slice
trick is replaced by a plain `Vec`. `Params`, `Perform`, `Parser` keep vte's
signatures so `parser.rs`/`perform.rs`/`screen.rs` changed only their
`vte::` paths to `crate::vt_parser::`. Removes `vte`, `arrayvec` and
`memchr` from the workspace tree.
Acceptance: six fixture pairs under
`crates/vt100-psmux/tests/fixtures/vte_0.15.0/` (`.bin` input, `.events`
callback log recorded from the real vte 0.15.0 crate: `omp_snippet`,
`pwsh_prompt`, `vim_altscreen`, `htop_like`, `edge_cases`, `utf8_split`)
replay with zero mismatches whole-buffer and are chunk-invariant (1- and
7-byte chunking); a deterministic 1,000,000-byte biased-random no-panic run
against both the raw parser and the screen parser; `Params` Debug renders
`[1:2:3;4]` and `len()` counts subparams.
Tests: `crates/vt100-psmux/tests/vt_parser_fixture.rs`,
`crates/vt100-psmux/tests/vt_parser_no_panic.rs`, unit tests in
`crates/vt100-psmux/src/vt_parser/tests.rs`

## ZDEP-022 itoa and unused dev-dependencies removed from vt100-psmux

`extend_itoa` in `crates/vt100-psmux/src/term.rs` now formats via std
`Display` (`i.to_string().as_bytes()`), byte-identical to the itoa output;
`[dependencies.itoa]` and the six unused `[dev-dependencies.*]` (nix,
quickcheck, rand, serde, serde_json, terminal_size -- referenced by no test
file) are removed from `crates/vt100-psmux/Cargo.toml`; `Cargo.lock` is
regenerated, dropping quickcheck, rand, rand_core, serde_json,
terminal_size, env_logger, env_filter, chacha20, cpufeatures, zmij from the
lock. `itoa` itself stays in the workspace tree because another crate still
uses it.
Acceptance: `contents_formatted()` cursor-position, 256-colour and
truecolour decimal rendering matches `format!("{}", n)` for boundary values
with no zero padding; the vt100-psmux manifest contains no vte, itoa or
dev-dependencies lines and retains psmux-unicode; the crate-tree golden
`tests-rs/fixtures/crate_tree_x86_64.txt` is regenerated minus arrayvec,
memchr, vte.
Tests: `crates/vt100-psmux/tests/term_itoa_parity.rs`,
`tests-rs/test_zdep_crate_tree.rs`

## ZDEP-023 portable-pty-psmux folded into src/pty (ConPTY-only, zero
third-party crates)

`crates/portable-pty-psmux` 0.9.7 (MIT) Windows half is ported into the root
crate as `src/pty/` with modules `mod.rs`, `child.rs` (WinChild/WinChildKiller
incl. the issue #446 poison-tolerant mutex fix), `cmdbuilder.rs` (Windows-only
CommandBuilder, `append_quoted` ArgvQuote), `conpty.rs`, `ffi.rs` (extern
"system" decls and structs, style mirrors src/win32.rs from ZDEP-008),
`handle.rs` (in-tree HANDLE wrapper: owns a Win32 HANDLE, closes on drop,
Read/Write via ReadFile/WriteFile, try_clone via DuplicateHandle -- replaces
`filedescriptor`), `procthreadattr.rs`, `psuedocon.rs`
(CreatePseudoConsole/ResizePseudoConsole/ClosePseudoConsole resolved from
kernel32.dll via GetModuleHandleW/LoadLibraryW + GetProcAddress in an
`OnceLock`, replacing `lazy_static` + `shared_library`; never sideloads a
foreign conpty.dll), `registry.rs` (hand-rolled HKLM/HKCU `Environment` key
read replacing `winreg`; malformed data yields fewer entries, never a panic).
`crate::pty::Error` is a direct alias of `std::io::Error` (replaces
`anyhow::Error`); nothing downcasts a pty trait object so the `downcast-rs`
supertraits are dropped, as are the unused async and signal APIs. Callers
(`src/pane.rs`, `src/proxy_pane.rs`, `src/popup.rs`, `src/platform.rs`,
`src/server/mod.rs` etc.) switch from `portable_pty::` to `crate::pty::`
paths. The unsafe allowlist (`tests-rs/test_unsafe_inventory.rs`) drops the
seven crate entries and adds src/pty child 6, conpty 2, ffi 2, handle 7,
procthreadattr 4, psuedocon 22, registry 7.
Acceptance: passthrough_supported is a pure predicate over (build number, env
override) with the 22621 threshold; conpty_base_flags() == 0x6
(RESIZE_QUIRK 0x2 | WIN32_INPUT_MODE 0x4), never INHERIT_CURSOR,
PASSTHROUGH_MODE 0x8 OR'ed separately; Error is io::Error; probe_conpty
succeeds against kernel32 and reports Unsupported when symbols are missing;
append_quoted matches the ArgvQuote table; a cmdline with an interior NUL
errors; registry_environment yields PATH and expands TEMP/TMP; a live ConPTY
round trip spawns, resizes and exits.
Tests: `tests-rs/test_zdep_pty_fold.rs`,
`tests-rs/test_zdep_proxy_pane_errors.rs`, `tests-rs/test_cmdbuilder.rs`,
`tests-rs/test_unsafe_inventory.rs`

## ZDEP-024 portable-pty-psmux crate removed from the workspace

`crates/portable-pty-psmux/` is deleted (its unix and serial halves had no
Windows consumer); it is removed from `[workspace] members` and the
`portable-pty` dependency line in `Cargo.toml`, from the
`cargo publish -p portable-pty-psmux` step in `.github/workflows/release.yml`,
and from the comment in `.github/dependabot.yml`. `Cargo.lock` is regenerated
and the crate-tree golden `tests-rs/fixtures/crate_tree_x86_64.txt` loses 15
lines: anyhow, downcast-rs, filedescriptor, lazy_static, libc, log, nix,
portable-pty-psmux, serial2, shared_library, shell-words, thiserror 1.x,
thiserror-impl 1.x, windows-sys, winreg. `winapi` stays in the tree via
crossterm_winapi.
Acceptance: the root manifest no longer references the crate; the crate
directory does not exist; the release workflow has no portable-pty publish
step; the crate-tree golden matches the live tree.
Tests: `tests-rs/test_zdep_pty_fold.rs` (manifest, directory and workflow
assertions), `tests-rs/test_zdep_crate_tree.rs`
