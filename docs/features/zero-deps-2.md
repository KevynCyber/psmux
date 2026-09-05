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
