/// Diagnostic: verify what crate::term's VtBackend emits for CROSSED_OUT,
/// HIDDEN modifiers and named/indexed colors (ZDEP-027 native backend).
/// Run with: cargo run --example vt_sgr_diag
use std::io::Write;

fn main() {
    let mut out: Vec<u8> = Vec::new();

    // SGR 9 = CrossedOut, 29 = NotCrossedOut (see term/backend.rs write_modifier_diff)
    out.write_all(b"\x1b[9m").unwrap();
    out.write_all(b"STRIKE").unwrap();
    out.write_all(b"\x1b[29m").unwrap();
    out.write_all(b" ").unwrap();

    // SGR 8 = Hidden, 28 = NoHidden
    out.write_all(b"\x1b[8m").unwrap();
    out.write_all(b"HIDDEN").unwrap();
    out.write_all(b"\x1b[28m").unwrap();
    out.write_all(b" ").unwrap();

    // Named color Red vs Indexed(1): both encode as the same 256-indexed
    // sequence in VtBackend's color_params (issue #425 "bold is bright").
    out.write_all(b"\x1b[38;5;1m").unwrap();
    out.write_all(b"RED").unwrap();
    out.write_all(b"\x1b[39m").unwrap();
    out.write_all(b" ").unwrap();

    out.write_all(b"\x1b[38;5;1m").unwrap();
    out.write_all(b"IDX1").unwrap();
    out.write_all(b"\x1b[39m").unwrap();

    println!("=== Raw bytes ({}) ===", out.len());
    let mut i = 0;
    while i < out.len() {
        if out[i] == 0x1b {
            let start = i;
            i += 1;
            while i < out.len() && !out[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i < out.len() {
                i += 1;
            }
            let seq = &out[start..i];
            let seq_str = String::from_utf8_lossy(seq);
            println!("  ESC: {:?}", seq_str);
        } else if out[i].is_ascii_graphic() || out[i] == b' ' {
            let start = i;
            while i < out.len() && (out[i].is_ascii_graphic() || out[i] == b' ') {
                i += 1;
            }
            println!("  TXT: {:?}", String::from_utf8_lossy(&out[start..i]));
        } else {
            i += 1;
        }
    }

    println!("\n=== ratatui Color -> VtBackend SGR mapping ===");
    use psmux_tui::style::Color;
    println!("  Color::Red       = {:?} -> 38;5;1", Color::Red);
    println!("  Color::Indexed(1) = {:?} -> 38;5;1", Color::Indexed(1));
    println!("  Color::LightRed  = {:?} -> 38;5;9", Color::LightRed);
    println!("  Color::Indexed(9) = {:?} -> 38;5;9", Color::Indexed(9));
}
