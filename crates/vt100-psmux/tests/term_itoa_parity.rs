// Covers: ZDEP-022
// Requirement: docs/plans/2026-09-04-zero-third-party-deps.md slice S5
// removes `itoa` from vt100-psmux; the screen parser's formatted-output
// paths (`Screen::contents_formatted`) must still render decimal integers
// (cursor rows/columns, 256-color and truecolor SGR parameters) exactly
// like `itoa::Buffer::format` did -- i.e. plain ASCII decimal, no leading
// zeros, no separators. This is a parity check on `vt100_psmux::Parser`
// (the existing, unchanged screen parser), not on the new `vt_parser`
// module. It also pins the manifest shape S5 leaves behind: no
// `[dependencies.vte]`, `[dependencies.itoa]`, or `[dev-dependencies` line
// (the six unused dev-dependencies nix/quickcheck/rand/serde/serde_json/
// terminal_size are removed in this slice too), while `[dependencies.
// psmux-unicode]` remains.

const CARGO_TOML: &str = include_str!("../Cargo.toml");

fn formatted(rows: u16, cols: u16, input: &[u8]) -> Vec<u8> {
    let mut parser = vt100_psmux::Parser::new(rows, cols, 0);
    parser.process(input);
    parser.screen().contents_formatted()
}

#[test]
fn cursor_position_renders_plain_decimal_after_cup() {
    // CSI 12;34 H is 1-indexed; row 11 col 33 (0-indexed) is where the
    // cursor ends up, and the formatted output's trailing cursor-move must
    // reproduce that as plain decimal text, not itoa-internal padding.
    let out = formatted(24, 80, b"\x1b[12;34H");
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.ends_with("\x1b[12;34H"),
        "expected formatted output to end with cursor move to 12;34, got {out_str:?}"
    );
}

#[test]
fn indexed_256_color_renders_as_38_5_n() {
    let out = formatted(24, 80, b"\x1b[38;5;208mX");
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("\x1b[38;5;208m"),
        "expected 256-color SGR \\x1b[38;5;208m in formatted output, got {out_str:?}"
    );
}

#[test]
fn truecolor_renders_as_38_2_r_g_b() {
    let out = formatted(24, 80, b"\x1b[38;2;12;200;255mX");
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("\x1b[38;2;12;200;255m"),
        "expected truecolor SGR \\x1b[38;2;12;200;255m in formatted output, got {out_str:?}"
    );
}

#[test]
fn three_digit_row_and_col_render_without_padding_on_a_200x300_screen() {
    // 200x300 screen, cursor moved to the last row/col (1-indexed 200;300).
    let out = formatted(200, 300, b"\x1b[200;300H");
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.ends_with("\x1b[200;300H"),
        "expected formatted output to end with cursor move to 200;300, got {out_str:?}"
    );
    // No leading zeros anywhere itoa would have produced 3-digit output.
    assert!(!out_str.contains("\x1b[0200;"), "unexpected zero-padded row");
}

#[test]
fn manifest_has_no_vte_itoa_or_dev_dependencies_lines_and_keeps_psmux_unicode() {
    for line in CARGO_TOML.lines() {
        let trimmed = line.trim();
        assert!(
            !trimmed.starts_with("[dependencies.vte]"),
            "Cargo.toml still has a [dependencies.vte] line"
        );
        assert!(
            !trimmed.starts_with("[dependencies.itoa]"),
            "Cargo.toml still has a [dependencies.itoa] line"
        );
        assert!(
            !trimmed.starts_with("[dev-dependencies"),
            "Cargo.toml still has a [dev-dependencies...] line: {trimmed}"
        );
    }
    assert!(
        CARGO_TOML.contains("[dependencies.psmux-unicode]"),
        "Cargo.toml must still depend on psmux-unicode"
    );
}
