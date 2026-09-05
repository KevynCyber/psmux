//! Small smoke tests for the `vt_parser` port (not vte's own unit tests --
//! the corpus/fixture tests in `tests/vt_parser_fixture.rs` cover behavioural
//! parity against real vte 0.15.0 output).

use super::{Params, Parser, Perform};

#[derive(Default)]
struct Dispatcher {
    dispatched: Vec<Sequence>,
}

#[derive(Debug, PartialEq, Eq)]
enum Sequence {
    Osc(Vec<Vec<u8>>, bool),
    Csi(Vec<Vec<u16>>, Vec<u8>, bool, char),
    Print(char),
}

impl Perform for Dispatcher {
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        let params = params.iter().map(|p| p.to_vec()).collect();
        self.dispatched.push(Sequence::Osc(params, bell_terminated));
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
        let params = params.iter().map(<[u16]>::to_vec).collect();
        let intermediates = intermediates.to_vec();
        self.dispatched.push(Sequence::Csi(params, intermediates, ignore, c));
    }

    fn print(&mut self, c: char) {
        self.dispatched.push(Sequence::Print(c));
    }
}

#[test]
fn csi_params_with_subparams() {
    const INPUT: &[u8] = b"\x1b[38:2:255:0:255;1m";
    let mut dispatcher = Dispatcher::default();
    let mut parser = Parser::new();

    parser.advance(&mut dispatcher, INPUT);

    assert_eq!(dispatcher.dispatched.len(), 1);
    match &dispatcher.dispatched[0] {
        Sequence::Csi(params, intermediates, ignore, action) => {
            assert_eq!(params, &[vec![38, 2, 255, 0, 255], vec![1]]);
            assert_eq!(intermediates, &Vec::<u8>::new());
            assert!(!ignore);
            assert_eq!(*action, 'm');
        },
        _ => panic!("expected csi sequence"),
    }
}

#[test]
fn osc_bell_terminated() {
    const INPUT: &[u8] = b"\x1b]11;ff/00/ff\x07";
    let mut dispatcher = Dispatcher::default();
    let mut parser = Parser::new();

    parser.advance(&mut dispatcher, INPUT);

    assert_eq!(dispatcher.dispatched.len(), 1);
    match &dispatcher.dispatched[0] {
        Sequence::Osc(_, true) => (),
        _ => panic!("expected osc with bell terminator"),
    }
}

#[test]
fn osc_st_terminated() {
    const INPUT: &[u8] = b"\x1b]11;ff/00/ff\x1b\\";
    let mut dispatcher = Dispatcher::default();
    let mut parser = Parser::new();

    parser.advance(&mut dispatcher, INPUT);

    // ST is ESC + '\', which also emits an Esc dispatch we don't model
    // here; this smoke test only checks the Osc entry (index 0).
    match &dispatcher.dispatched[0] {
        Sequence::Osc(_, false) => (),
        _ => panic!("expected osc with ST terminator"),
    }
}

#[test]
fn split_utf8_across_two_advance_calls() {
    const INPUT: &[u8] = b"\xF0\x9F\x9A\x80"; // U+1F680 ROCKET
    let mut dispatcher = Dispatcher::default();
    let mut parser = Parser::new();

    parser.advance(&mut dispatcher, &INPUT[..1]);
    parser.advance(&mut dispatcher, &INPUT[1..2]);
    parser.advance(&mut dispatcher, &INPUT[2..3]);
    parser.advance(&mut dispatcher, &INPUT[3..]);

    assert_eq!(dispatcher.dispatched.len(), 1);
    assert_eq!(dispatcher.dispatched[0], Sequence::Print('\u{1F680}'));
}

#[test]
fn invalid_utf8_emits_replacement_char() {
    const INPUT: &[u8] = b"a\xEF\xBCb";
    let mut dispatcher = Dispatcher::default();
    let mut parser = Parser::new();

    parser.advance(&mut dispatcher, INPUT);

    assert_eq!(dispatcher.dispatched.len(), 3);
    assert_eq!(dispatcher.dispatched[0], Sequence::Print('a'));
    assert_eq!(dispatcher.dispatched[1], Sequence::Print('\u{FFFD}'));
    assert_eq!(dispatcher.dispatched[2], Sequence::Print('b'));
}

#[test]
fn params_debug_format() {
    const INPUT: &[u8] = b"\x1b[1:2;3m";
    let mut dispatcher = Dispatcher::default();
    let mut parser = Parser::new();

    parser.advance(&mut dispatcher, INPUT);

    match &dispatcher.dispatched[0] {
        Sequence::Csi(params, ..) => assert_eq!(params, &[vec![1, 2], vec![3]]),
        _ => panic!("expected csi sequence"),
    }
}
