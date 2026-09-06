// Covers: ZDEP-025
// Requirement: crossterm's event types (KeyModifiers, KeyEventState, KeyCode,
// KeyEventKind, KeyEvent, MouseButton, MouseEventKind, MouseEvent, Event) and
// its Command layer (EnterAlternateScreen, LeaveAlternateScreen,
// EnableBlinking, DisableBlinking, EnableMouseCapture, DisableMouseCapture,
// EnableBracketedPaste, DisableBracketedPaste, Print<T>, Hide, Show, MoveTo,
// the `Command` trait, and the `execute!`/`queue!` macros) are replaced by an
// in-tree, zero-third-party-dependency `crate::term` module that is
// shape-identical to crossterm 0.29 for every construct psmux actually uses.
// The existing SSH/pipe VT byte parser in `crate::ssh_input` (VtParser) is
// re-anchored on these new types without being rewritten, so its published
// byte -> Event behavior (SGR mouse, focus in/out, bracketed paste, arrows,
// F-keys, ctrl/alt chars, XTWINOPS resize) must not drift.

use crate::term::cursor::{DisableBlinking, EnableBlinking, Hide, MoveTo, Show};
use crate::term::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use crate::term::style::Print;
use crate::term::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crate::term::Command;

// ═══════════════════════════════════════════════════════════════════════
// (a) KeyModifiers / KeyEventState bitflag semantics
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn key_modifiers_bit_constants() {
    assert_eq!(KeyModifiers::NONE.bits(), 0);
    assert_eq!(KeyModifiers::SHIFT.bits(), 1);
    assert_eq!(KeyModifiers::CONTROL.bits(), 2);
    assert_eq!(KeyModifiers::ALT.bits(), 4);
    assert_eq!(KeyModifiers::SUPER.bits(), 8);
    assert_eq!(KeyModifiers::HYPER.bits(), 16);
    assert_eq!(KeyModifiers::META.bits(), 32);
}

#[test]
fn key_modifiers_empty_and_all() {
    assert!(KeyModifiers::empty().is_empty());
    assert_eq!(KeyModifiers::empty().bits(), 0);
    let all = KeyModifiers::all();
    assert_eq!(all.bits(), 1 | 2 | 4 | 8 | 16 | 32);
    assert!(all.contains(KeyModifiers::SHIFT));
    assert!(all.contains(KeyModifiers::CONTROL));
    assert!(all.contains(KeyModifiers::ALT));
    assert!(all.contains(KeyModifiers::SUPER));
    assert!(all.contains(KeyModifiers::HYPER));
    assert!(all.contains(KeyModifiers::META));
}

#[test]
fn key_modifiers_from_bits_truncate_drops_unknown_bits() {
    // 0xFF has bits above META (32) set; from_bits_truncate must drop them.
    let m = KeyModifiers::from_bits_truncate(0xFF);
    assert_eq!(m.bits(), 1 | 2 | 4 | 8 | 16 | 32);
}

#[test]
fn key_modifiers_contains_and_intersects() {
    let m = KeyModifiers::SHIFT | KeyModifiers::CONTROL;
    assert!(m.contains(KeyModifiers::SHIFT));
    assert!(m.contains(KeyModifiers::CONTROL));
    assert!(!m.contains(KeyModifiers::ALT));
    assert!(m.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(!m.intersects(KeyModifiers::ALT | KeyModifiers::SUPER));
}

#[test]
fn key_modifiers_insert_remove_difference_union() {
    let mut m = KeyModifiers::empty();
    m.insert(KeyModifiers::SHIFT);
    m.insert(KeyModifiers::ALT);
    assert_eq!(m.bits(), 1 | 4);

    m.remove(KeyModifiers::SHIFT);
    assert_eq!(m.bits(), 4);
    assert!(!m.contains(KeyModifiers::SHIFT));

    let both = KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT;
    let diff = both.difference(KeyModifiers::CONTROL);
    assert_eq!(diff.bits(), 1 | 4);

    let u = KeyModifiers::SHIFT.union(KeyModifiers::ALT);
    assert_eq!(u.bits(), 1 | 4);
}

#[test]
fn key_modifiers_bitor_bitorassign_bitand_sub_not() {
    let a = KeyModifiers::SHIFT | KeyModifiers::CONTROL;
    assert_eq!(a.bits(), 1 | 2);

    let mut b = KeyModifiers::SHIFT;
    b |= KeyModifiers::ALT;
    assert_eq!(b.bits(), 1 | 4);

    let and = (KeyModifiers::SHIFT | KeyModifiers::CONTROL) & KeyModifiers::CONTROL;
    assert_eq!(and, KeyModifiers::CONTROL);

    let sub = (KeyModifiers::SHIFT | KeyModifiers::CONTROL) - KeyModifiers::SHIFT;
    assert_eq!(sub, KeyModifiers::CONTROL);

    let not = !KeyModifiers::empty();
    assert!(not.contains(KeyModifiers::SHIFT));
    assert!(not.contains(KeyModifiers::META));
}

#[test]
fn key_event_state_bit_constants_and_empty() {
    assert_eq!(KeyEventState::NONE.bits(), 0);
    assert_eq!(KeyEventState::KEYPAD.bits(), 1);
    assert_eq!(KeyEventState::CAPS_LOCK.bits(), 8);
    assert_eq!(KeyEventState::NUM_LOCK.bits(), 16);
    assert!(KeyEventState::empty().is_empty());
    assert_eq!(KeyEventState::empty(), KeyEventState::NONE);
}

// ═══════════════════════════════════════════════════════════════════════
// (b) Command ANSI strings, Print<T>, MoveTo, execute!/queue!
// ═══════════════════════════════════════════════════════════════════════

fn ansi_of(cmd: &impl Command) -> String {
    let mut s = String::new();
    cmd.write_ansi(&mut s).expect("write_ansi must not fail writing into a String");
    s
}

#[test]
fn command_ansi_strings_match_crossterm_shape() {
    assert_eq!(ansi_of(&EnterAlternateScreen), "\x1b[?1049h");
    assert_eq!(ansi_of(&LeaveAlternateScreen), "\x1b[?1049l");
    assert_eq!(ansi_of(&EnableBlinking), "\x1b[?12h");
    assert_eq!(ansi_of(&DisableBlinking), "\x1b[?12l");
    assert_eq!(
        ansi_of(&EnableMouseCapture),
        "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h"
    );
    assert_eq!(
        ansi_of(&DisableMouseCapture),
        "\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l"
    );
    assert_eq!(ansi_of(&EnableBracketedPaste), "\x1b[?2004h");
    assert_eq!(ansi_of(&DisableBracketedPaste), "\x1b[?2004l");
    assert_eq!(ansi_of(&Hide), "\x1b[?25l");
    assert_eq!(ansi_of(&Show), "\x1b[?25h");
}

#[test]
fn print_writes_display_value_verbatim() {
    assert_eq!(ansi_of(&Print("hello")), "hello");
    assert_eq!(ansi_of(&Print(42)), "42");
}

#[test]
fn move_to_emits_1_indexed_cup() {
    assert_eq!(ansi_of(&MoveTo(0, 0)), "\x1b[1;1H");
    assert_eq!(ansi_of(&MoveTo(5, 3)), "\x1b[4;6H");
    assert_eq!(ansi_of(&MoveTo(79, 23)), "\x1b[24;80H");
}

#[test]
fn execute_macro_writes_each_command_ansi_and_flushes_into_vec_u8() {
    // execute!/queue! are #[macro_export] macros, which live at crate root
    // (not under crate::term) even though the design note's path layout
    // says `crate::term::execute` -- see the RED report's macro-path note.
    let mut out: Vec<u8> = Vec::new();
    crate::execute!(out, MoveTo(1, 2), Print("hi"), Hide).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "\x1b[3;2Hhi\x1b[?25l"
    );
}

#[test]
fn queue_macro_writes_without_requiring_separate_flush_call() {
    let mut out: Vec<u8> = Vec::new();
    crate::queue!(out, Show, MoveTo(0, 0)).unwrap();
    use std::io::Write;
    out.flush().unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "\x1b[?25h\x1b[1;1H");
}

// ═══════════════════════════════════════════════════════════════════════
// (c) bytes -> Event table for pipe/SSH mode, exercised through
// crate::ssh_input's existing VtParser (re-anchored on crate::term::event).
// Requires VtParser, VtParser::new, and VtParser::feed to be `pub(crate)`
// (currently private) so this crate-root-wired test file can reach them --
// see the RED report's seam note.
// ═══════════════════════════════════════════════════════════════════════

fn parse(s: &str) -> Vec<Event> {
    let mut p = crate::ssh_input::VtParser::new();
    let mut events = Vec::new();
    for ch in s.chars() {
        p.feed(ch, &mut |evt| events.push(evt));
    }
    events
}

fn parse_bytes(bytes: &[u8]) -> Vec<Event> {
    let mut p = crate::ssh_input::VtParser::new();
    let mut events = Vec::new();
    for &b in bytes {
        p.feed(b as char, &mut |evt| events.push(evt));
    }
    events
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    })
}

#[test]
fn vt_parser_bytes_to_event_table() {
    // (input, expected single event)
    let cases: Vec<(&str, Event)> = vec![
        ("\x1b[A", key(KeyCode::Up, KeyModifiers::empty())),
        ("\x1b[B", key(KeyCode::Down, KeyModifiers::empty())),
        ("\x1b[C", key(KeyCode::Right, KeyModifiers::empty())),
        ("\x1b[D", key(KeyCode::Left, KeyModifiers::empty())),
        ("\x1b[H", key(KeyCode::Home, KeyModifiers::empty())),
        ("\x1b[F", key(KeyCode::End, KeyModifiers::empty())),
        ("\x1b[1;5A", key(KeyCode::Up, KeyModifiers::CONTROL)),
        ("\x1bOP", key(KeyCode::F(1), KeyModifiers::empty())),
        ("\x1bOQ", key(KeyCode::F(2), KeyModifiers::empty())),
        ("\x1b[15~", key(KeyCode::F(5), KeyModifiers::empty())),
        ("\x1b[17~", key(KeyCode::F(6), KeyModifiers::empty())),
        ("\x1b[3~", key(KeyCode::Delete, KeyModifiers::empty())),
        ("\x1b[2~", key(KeyCode::Insert, KeyModifiers::empty())),
        ("\x1b[Z", key(KeyCode::BackTab, KeyModifiers::SHIFT)),
        ("\x1b[I", Event::FocusGained),
        ("\x1b[O", Event::FocusLost),
        ("\x1b[8;24;80t", Event::Resize(80, 24)),
        ("\r", key(KeyCode::Enter, KeyModifiers::empty())),
        ("\t", key(KeyCode::Tab, KeyModifiers::empty())),
        ("\x7f", key(KeyCode::Backspace, KeyModifiers::empty())),
        ("\x01", key(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        ("\x1a", key(KeyCode::Char('z'), KeyModifiers::CONTROL)),
        ("\x1c", key(KeyCode::Char('\\'), KeyModifiers::CONTROL)),
        ("\0", key(KeyCode::Char(' '), KeyModifiers::CONTROL)),
        ("\x1ba", key(KeyCode::Char('a'), KeyModifiers::ALT)),
        (
            "\x1b[<0;10;20M",
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 9,
                row: 19,
                modifiers: KeyModifiers::empty(),
            }),
        ),
        (
            "\x1b[<0;10;20m",
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 9,
                row: 19,
                modifiers: KeyModifiers::empty(),
            }),
        ),
        (
            "\x1b[<64;5;5M",
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 4,
                row: 4,
                modifiers: KeyModifiers::empty(),
            }),
        ),
    ];

    for (input, expected) in &cases {
        let events = parse(input);
        assert_eq!(
            events.last().cloned(),
            Some(expected.clone()),
            "input {:?} bytes={:?} -> events {:?}, expected last event {:?}",
            input,
            input.as_bytes(),
            events,
            expected
        );
    }
    assert!(
        cases.len() >= 20,
        "fixture table must have >= 20 rows, has {}",
        cases.len()
    );
}

#[test]
fn vt_parser_x10_mouse_raw_bytes() {
    // \x1b[M followed by 3 raw bytes: button+32, col+33, row+33.
    // button=0 (left down), col=1 (byte 34-33), row=1 (byte 34-33).
    let bytes: &[u8] = &[0x1b, b'[', b'M', 32, 34, 34];
    let events = parse_bytes(bytes);
    assert_eq!(
        events,
        vec![Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::empty(),
        })]
    );
}

#[test]
fn vt_parser_bracketed_paste_roundtrip() {
    let events = parse("\x1b[200~hello world\x1b[201~");
    assert_eq!(events, vec![Event::Paste("hello world".to_string())]);
}

#[test]
fn key_event_new_and_new_with_kind() {
    let a = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(a.code, KeyCode::Char('x'));
    assert_eq!(a.modifiers, KeyModifiers::CONTROL);
    assert_eq!(a.kind, KeyEventKind::Press);
    assert_eq!(a.state, KeyEventState::NONE);

    let b = KeyEvent::new_with_kind(
        KeyCode::Char('x'),
        KeyModifiers::CONTROL,
        KeyEventKind::Release,
    );
    assert_eq!(b.kind, KeyEventKind::Release);
}
