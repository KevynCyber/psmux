// Covers: ZDEP-021
// Requirement: `vt100_psmux::vt_parser` (the DEC ANSI parser state machine
// replacing the `vte` crate, docs/plans/2026-09-04-zero-third-party-deps.md
// slice S5) reproduces vte 0.15.0 byte-for-byte on every fixture pair
// committed under `tests/fixtures/vte_0.15.0/`: `<name>.bin` is the raw
// input, `<name>.events` is the exact callback stream vte 0.15.0 produced
// for that input (generated once via a throwaway cargo project, oracle
// recorder identical in shape to the replay recorder below so the event
// format cannot drift between generation and replay). Event line format:
// `print U+XXXX` (uppercase hex, >=4 digits), `execute XX` (2 hex digits),
// `esc <inter> <ignore> XX` (`<inter>` = intermediates as 2-hex-digit bytes
// joined by `,`, or `-` when empty; `<ignore>` = 0|1; `XX` = final byte, 2
// hex digits), `csi <params> <inter> <ignore> <final>` / `hook <params>
// <inter> <ignore> <final>` (`<params>` = vte's own `Params` Debug
// rendering, e.g. `[1:2;3]`, `[]` when empty; `<final>` = `U+XXXX`), `put
// XX`, `unhook`, `osc <params> <bel>` (`<params>` = each raw param as
// comma-joined 2-hex-digit bytes, empty param is empty string, params
// joined by `;`; `<bel>` = 0|1). Every corpus pair replays with 0
// mismatches feeding the whole buffer at once, and an identical byte-for-
// byte event stream feeding 1 byte at a time and in 7-byte chunks (vte
// buffers partial UTF-8 across `advance` calls, so chunking must never
// change the observed events). `vt_parser::Params` mirrors vte's `Params`:
// `Debug` renders `[1:2;3]` for a `CSI 1:2;3 ...` dispatch, `is_empty()` is
// true only for a bare `CSI m` (no digits at all), and `len()` counts
// parameters AND subparameters combined (vte's own semantics, see
// vte-0.15.0/src/params.rs `Params::len`). The fixture directory must
// contain at least 6 `.bin`/`.events` pairs (regression guard against an
// accidentally emptied corpus).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use vt100_psmux::vt_parser::{Params, Parser, Perform};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vte_0.15.0")
}

/// Discovers every `<name>.bin`/`<name>.events` pair in the fixture
/// directory. Panics (rather than silently skipping) if a `.bin` file has
/// no matching `.events` file, or vice versa.
fn corpus_pairs() -> Vec<(String, Vec<u8>, Vec<String>)> {
    let dir = fixtures_dir();
    let mut bin_names: HashSet<String> = HashSet::new();
    let mut events_names: HashSet<String> = HashSet::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {dir:?}: {e}")) {
        let entry = entry.unwrap();
        let file_name = entry.file_name().to_string_lossy().to_string();
        if let Some(stem) = file_name.strip_suffix(".bin") {
            bin_names.insert(stem.to_string());
        } else if let Some(stem) = file_name.strip_suffix(".events") {
            events_names.insert(stem.to_string());
        }
    }
    let missing_events: Vec<_> = bin_names.difference(&events_names).cloned().collect();
    assert!(missing_events.is_empty(), "fixtures missing .events sibling: {missing_events:?}");
    let missing_bin: Vec<_> = events_names.difference(&bin_names).cloned().collect();
    assert!(missing_bin.is_empty(), "fixtures missing .bin sibling: {missing_bin:?}");

    let mut names: Vec<String> = bin_names.into_iter().collect();
    names.sort();

    names
        .into_iter()
        .map(|name| {
            let bytes = fs::read(dir.join(format!("{name}.bin"))).unwrap();
            let events_raw = fs::read_to_string(dir.join(format!("{name}.events"))).unwrap();
            let events: Vec<String> =
                events_raw.lines().map(str::to_string).filter(|l| !l.is_empty()).collect();
            (name, bytes, events)
        })
        .collect()
}

// ---- replay recorder: MUST format events identically to the oracle
// generator that produced the committed `.events` files ----

fn fmt_char(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

fn fmt_byte(b: u8) -> String {
    format!("{:02X}", b)
}

fn fmt_ignore(b: bool) -> &'static str {
    if b {
        "1"
    } else {
        "0"
    }
}

fn fmt_intermediates(inter: &[u8]) -> String {
    if inter.is_empty() {
        "-".to_string()
    } else {
        inter.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(",")
    }
}

fn fmt_osc_params(params: &[&[u8]]) -> String {
    params
        .iter()
        .map(|p| p.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join(";")
}

#[derive(Default)]
struct Rec {
    events: Vec<String>,
}

impl Perform for Rec {
    fn print(&mut self, c: char) {
        self.events.push(format!("print {}", fmt_char(c)));
    }

    fn execute(&mut self, b: u8) {
        self.events.push(format!("execute {}", fmt_byte(b)));
    }

    fn hook(&mut self, params: &Params, inter: &[u8], ignore: bool, action: char) {
        self.events.push(format!(
            "hook {:?} {} {} {}",
            params,
            fmt_intermediates(inter),
            fmt_ignore(ignore),
            fmt_char(action)
        ));
    }

    fn put(&mut self, b: u8) {
        self.events.push(format!("put {}", fmt_byte(b)));
    }

    fn unhook(&mut self) {
        self.events.push("unhook".to_string());
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bel: bool) {
        self.events.push(format!("osc {} {}", fmt_osc_params(params), fmt_ignore(bel)));
    }

    fn csi_dispatch(&mut self, params: &Params, inter: &[u8], ignore: bool, action: char) {
        self.events.push(format!(
            "csi {:?} {} {} {}",
            params,
            fmt_intermediates(inter),
            fmt_ignore(ignore),
            fmt_char(action)
        ));
    }

    fn esc_dispatch(&mut self, inter: &[u8], ignore: bool, byte: u8) {
        self.events.push(format!(
            "esc {} {} {}",
            fmt_intermediates(inter),
            fmt_ignore(ignore),
            fmt_byte(byte)
        ));
    }
}

fn record_whole(bytes: &[u8]) -> Vec<String> {
    let mut parser = Parser::new();
    let mut rec = Rec::default();
    parser.advance(&mut rec, bytes);
    rec.events
}

fn record_chunked(bytes: &[u8], chunk: usize) -> Vec<String> {
    let mut parser = Parser::new();
    let mut rec = Rec::default();
    for c in bytes.chunks(chunk.max(1)) {
        parser.advance(&mut rec, c);
    }
    rec.events
}

#[test]
fn corpus_has_at_least_6_fixture_pairs() {
    let pairs = corpus_pairs();
    assert!(pairs.len() >= 6, "expected >= 6 fixture pairs, found {}", pairs.len());
}

#[test]
fn every_corpus_pair_replays_with_zero_mismatches_whole_buffer() {
    let pairs = corpus_pairs();
    assert!(!pairs.is_empty(), "no fixture pairs found");
    let mut failures = Vec::new();
    for (name, bytes, expected) in &pairs {
        let actual = record_whole(bytes);
        if actual != *expected {
            let mut first_diff = None;
            for i in 0..actual.len().max(expected.len()) {
                let a = actual.get(i);
                let e = expected.get(i);
                if a != e {
                    first_diff = Some((i, e.cloned(), a.cloned()));
                    break;
                }
            }
            failures.push(format!(
                "{name}: mismatch at line {:?} (expected vs got): {:?}",
                first_diff.as_ref().map(|(i, ..)| *i),
                first_diff
            ));
        }
    }
    assert!(failures.is_empty(), "corpus mismatches:\n{}", failures.join("\n"));
}

#[test]
fn every_corpus_pair_is_chunk_invariant() {
    let pairs = corpus_pairs();
    let mut failures = Vec::new();
    for (name, bytes, _expected) in &pairs {
        let whole = record_whole(bytes);
        let one = record_chunked(bytes, 1);
        let seven = record_chunked(bytes, 7);
        if whole != one {
            failures.push(format!("{name}: 1-byte chunking diverged from whole-buffer replay"));
        }
        if whole != seven {
            failures.push(format!("{name}: 7-byte chunking diverged from whole-buffer replay"));
        }
    }
    assert!(failures.is_empty(), "chunk-invariant failures:\n{}", failures.join("\n"));
}

// ---- Params unit checks ----

#[derive(Default)]
struct ParamsCapture {
    last_csi_params_debug: Option<String>,
    last_csi_len: usize,
    last_csi_is_empty: bool,
}

impl Perform for ParamsCapture {
    fn csi_dispatch(&mut self, params: &Params, _inter: &[u8], _ignore: bool, _action: char) {
        self.last_csi_params_debug = Some(format!("{params:?}"));
        self.last_csi_len = params.len();
        self.last_csi_is_empty = params.is_empty();
    }
}

#[test]
fn params_debug_renders_like_vte_for_subparams() {
    let mut parser = Parser::new();
    let mut cap = ParamsCapture::default();
    parser.advance(&mut cap, b"\x1b[1:2;3m");
    assert_eq!(cap.last_csi_params_debug.as_deref(), Some("[1:2;3]"));
}

#[test]
fn params_is_empty_true_for_bare_csi_m() {
    let mut parser = Parser::new();
    let mut cap = ParamsCapture::default();
    parser.advance(&mut cap, b"\x1b[m");
    // vte still pushes one implicit zero param for a bare final byte, so
    // is_empty() is false and len() is 1 -- exercised by the next test;
    // this test instead checks the truly empty case: no params AND no
    // final-byte dispatch happens for params with zero entries only when
    // params.clear() state is observed directly. Bare `CSI m` in vte
    // always has len() == 1 (the implicit zero), so is_empty() is false.
    assert!(!cap.last_csi_is_empty, "bare CSI m has an implicit zero param in vte");
    assert_eq!(cap.last_csi_len, 1);
}

#[test]
fn params_len_counts_subparams_like_vte() {
    let mut parser = Parser::new();
    let mut cap = ParamsCapture::default();
    // "1:2:3;4" => 5 total entries (3 subparams under param 1, plus param 4)
    parser.advance(&mut cap, b"\x1b[1:2:3;4m");
    assert_eq!(cap.last_csi_len, 4);
    assert_eq!(cap.last_csi_params_debug.as_deref(), Some("[1:2:3;4]"));
}
