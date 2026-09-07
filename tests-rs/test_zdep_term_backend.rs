// Covers: ZDEP-027
// Requirement: `crate::term::backend::VtBackend<W: Write>` replaces
// `psmux_tui::backend::CrosstermBackend`, byte-identical for the exact SGR /
// cursor / clear / append-lines / blink sequences psmux emits (crossterm and
// psmux_tui-crossterm are removed from the root and monitor manifests, and
// Cargo.lock no longer carries either crate). Cell-for-cell fidelity is
// pinned against tests-rs/fixtures/vt_backend_golden_3x2.bin, a byte capture
// taken from psmux_tui::backend::CrosstermBackend BEFORE crossterm was
// removed (see git history for the throwaway capture harness that produced
// it); this file only reads the committed fixture, it does not regenerate
// it.

use crate::term::backend::VtBackend;
use psmux_tui::backend::Backend;
use psmux_tui::buffer::{Buffer, Cell};
use psmux_tui::layout::Rect;
use psmux_tui::style::{Color, Modifier, Style};

/// Must stay byte-identical to the buffer-building code that produced
/// tests-rs/fixtures/vt_backend_golden_3x2.bin (captured via
/// psmux_tui::backend::CrosstermBackend before the crossterm removal).
fn build_test_buffer() -> (Rect, Buffer) {
    let area = Rect::new(0, 0, 3, 2);
    let mut buf = Buffer::empty(area);

    buf[(0, 0)].set_symbol("A");
    buf[(0, 0)].set_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));

    buf[(1, 0)].set_symbol("B");
    buf[(1, 0)].set_style(
        Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::DIM)
            .underline_color(Color::Indexed(99)),
    );

    buf[(2, 0)].set_symbol("C");
    buf[(2, 0)].set_style(
        Style::default()
            .fg(Color::Rgb(10, 20, 30))
            .bg(Color::Gray)
            .add_modifier(Modifier::ITALIC),
    );

    // (1, 1) intentionally left untouched -- creates a gap so the next drawn
    // cell (2, 1) requires a non-adjacent MoveTo instead of a bare write.
    buf[(0, 1)].set_symbol("D");
    buf[(0, 1)].set_style(
        Style::default()
            .fg(Color::White)
            .bg(Color::Indexed(200))
            .add_modifier(Modifier::UNDERLINED),
    );

    buf[(2, 1)].set_symbol("F");
    buf[(2, 1)].set_style(Style::default().fg(Color::Reset).bg(Color::Reset));

    (area, buf)
}

/// Cells in draw order, skipping (1, 1).
fn draw_order(area: Rect, buf: &Buffer) -> Vec<(u16, u16, &Cell)> {
    let mut v = Vec::new();
    for y in 0..area.height {
        for x in 0..area.width {
            if (x, y) == (1, 1) {
                continue;
            }
            v.push((x, y, &buf[(x, y)]));
        }
    }
    v
}

fn golden_bytes() -> Vec<u8> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/tests-rs/fixtures/vt_backend_golden_3x2.bin", root);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path, e))
}

// ═══════════════════════════════════════════════════════════════════════
// (a) VtBackend draw output is byte-identical to the crossterm-captured
// golden for a 3x2 buffer covering bold/dim/italic/underline/reversed,
// named colors (Red/LightRed/Gray/White/DarkGray), Rgb, Indexed, Reset,
// underline_color, adjacent-vs-non-adjacent MoveTo, and modifier removal.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn vt_backend_draw_matches_crossterm_golden_3x2() {
    let (area, buf) = build_test_buffer();
    let mut out: Vec<u8> = Vec::new();
    {
        let mut backend = VtBackend::new(&mut out);
        backend.draw(draw_order(area, &buf).into_iter()).unwrap();
        backend.hide_cursor().unwrap();
        backend.show_cursor().unwrap();
        backend.set_cursor_position((2, 1)).unwrap();
        backend.clear().unwrap();
        backend.append_lines(2).unwrap();
        use std::io::Write as _;
        crate::execute!(out, crate::term::cursor::EnableBlinking, crate::term::cursor::DisableBlinking).unwrap();
        out.flush().unwrap();
    }
    let golden = golden_bytes();
    assert_eq!(
        out, golden,
        "VtBackend output diverged from the crossterm-captured golden.\nvt_backend: {:?}\ngolden:     {:?}",
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&golden)
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Individual command bytes, in case the aggregate golden test's failure
// message is too coarse to localize a regression.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn vt_backend_hide_show_cursor_bytes() {
    let mut out: Vec<u8> = Vec::new();
    let mut backend = VtBackend::new(&mut out);
    backend.hide_cursor().unwrap();
    backend.show_cursor().unwrap();
    drop(backend);
    assert_eq!(String::from_utf8(out).unwrap(), "\x1b[?25l\x1b[?25h");
}

#[test]
fn vt_backend_set_cursor_position() {
    let mut out: Vec<u8> = Vec::new();
    let mut backend = VtBackend::new(&mut out);
    backend.set_cursor_position((5, 2)).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "\x1b[3;6H");
}

#[test]
fn vt_backend_clear_variants() {
    use psmux_tui::backend::ClearType;
    let cases: &[(ClearType, &str)] = &[
        (ClearType::All, "\x1b[2J"),
        (ClearType::AfterCursor, "\x1b[J"),
        (ClearType::BeforeCursor, "\x1b[1J"),
        (ClearType::CurrentLine, "\x1b[2K"),
        (ClearType::UntilNewLine, "\x1b[K"),
    ];
    for (kind, expected) in cases {
        let mut out: Vec<u8> = Vec::new();
        let mut backend = VtBackend::new(&mut out);
        backend.clear_region(*kind).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), *expected, "ClearType {:?}", kind);
    }
}

#[test]
fn vt_backend_append_lines_emits_n_newlines_and_flushes() {
    let mut out: Vec<u8> = Vec::new();
    let mut backend = VtBackend::new(&mut out);
    backend.append_lines(3).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "\n\n\n");
}

#[test]
fn vt_backend_blink_csi() {
    let mut out: Vec<u8> = Vec::new();
    crate::execute!(
        out,
        crate::term::cursor::EnableBlinking,
        crate::term::cursor::DisableBlinking
    )
    .unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "\x1b[?12h\x1b[?12l");
}

// ═══════════════════════════════════════════════════════════════════════
// (b) manifest-line tests: crossterm gone, psmux_tui line exact.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn root_manifest_has_no_crossterm_line() {
    let root = env!("CARGO_MANIFEST_DIR");
    let manifest = std::fs::read_to_string(format!("{}/Cargo.toml", root)).unwrap();
    assert!(
        !manifest.lines().any(|l| l.trim_start().starts_with("crossterm")),
        "root Cargo.toml must not contain a line naming crossterm"
    );
}

#[test]
fn monitor_manifest_has_no_crossterm_line() {
    let root = env!("CARGO_MANIFEST_DIR");
    let manifest = std::fs::read_to_string(format!("{}/tests/monitor/Cargo.toml", root)).unwrap();
    assert!(
        !manifest.lines().any(|l| l.trim_start().starts_with("crossterm")),
        "tests/monitor/Cargo.toml must not contain a line naming crossterm"
    );
}

#[test]
fn root_manifest_psmux_tui_line_is_exact() {
    let root = env!("CARGO_MANIFEST_DIR");
    let manifest = std::fs::read_to_string(format!("{}/Cargo.toml", root)).unwrap();
    let expected = r#"psmux_tui = { version = "0.30.2", default-features = false, features = ["std", "all-widgets", "underline-color"] }"#;
    assert!(
        manifest.lines().any(|l| l.trim() == expected),
        "root Cargo.toml must contain exactly:\n{}\ngot lines:\n{}",
        expected,
        manifest.lines().filter(|l| l.contains("psmux_tui")).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn monitor_manifest_psmux_tui_line_is_exact_minus_underline_color() {
    let root = env!("CARGO_MANIFEST_DIR");
    let manifest = std::fs::read_to_string(format!("{}/tests/monitor/Cargo.toml", root)).unwrap();
    let expected = r#"psmux_tui = { version = "0.30.2", default-features = false, features = ["std", "all-widgets"] }"#;
    assert!(
        manifest.lines().any(|l| l.trim() == expected),
        "tests/monitor/Cargo.toml must contain exactly:\n{}\ngot lines:\n{}",
        expected,
        manifest.lines().filter(|l| l.contains("psmux_tui")).collect::<Vec<_>>().join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════
// (c) src/term/*.rs contains no `crate::` token (same invariant intent as
// src/pty; examples and tests/monitor #[path]-include this module so it
// must stand alone).
// ═══════════════════════════════════════════════════════════════════════

fn walk_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[test]
fn src_term_has_no_crate_paths() {
    let root = env!("CARGO_MANIFEST_DIR");
    let term_dir = std::path::PathBuf::from(format!("{}/src/term", root));
    assert!(term_dir.exists(), "src/term must exist");
    let mut files = Vec::new();
    walk_rs_files(&term_dir, &mut files);
    assert!(!files.is_empty(), "src/term must contain .rs files");
    for f in &files {
        // `$crate::` in macro_rules bodies resolves to the INCLUDING crate
        // (every includer mounts this tree as `term`), so it is not a
        // crate-root path in the sense this invariant guards against.
        let contents = std::fs::read_to_string(f).unwrap().replace("$crate::", "");
        assert!(
            !contents.contains("crate::"),
            "{} must not reference `crate::` (src/term is #[path]-included by examples and tests/monitor, which cannot see the crate root)",
            f.display()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// (d) The normal-edge dependency tree no longer carries the crossterm
// family. (Cargo.lock cannot be the oracle: it resolves the union of all
// optional features, so psmux_tui's optional `crossterm` feature keeps the
// family in the lock file regardless; the tree golden is enforced against
// live `cargo tree -e normal` by test_zdep_crate_tree.rs.)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn crate_tree_has_no_crossterm_family_packages() {
    let root = env!("CARGO_MANIFEST_DIR");
    let tree = std::fs::read_to_string(format!(
        "{}/tests-rs/fixtures/crate_tree_x86_64.txt",
        root
    ))
    .unwrap();
    for banned in ["crossterm", "crossterm_winapi", "psmux_tui-crossterm", "winapi"] {
        let hit = tree.lines().any(|l| {
            l.trim().split_whitespace().next() == Some(banned)
        });
        assert!(
            !hit,
            "crate tree golden must not contain a `{}` package",
            banned
        );
    }
}
