//! Pike VM: a Thompson-NFA simulation with per-thread capture slots and
//! strict priority ordering, giving leftmost-first (Perl) semantics with
//! no backtracking (`(a*)*b` on 10,000 `a`s is linear, not exponential).
//! Unanchored search is implemented by injecting a new lowest-priority
//! "start here" thread at every position until a match is found.

use crate::ast::{ClassSpec, Shorthand};
use crate::compile::{Assert, Inst, Program};
use std::rc::Rc;

type Saves = Rc<Vec<Option<usize>>>;

#[derive(Clone)]
struct Thread {
    pc: usize,
    saves: Saves,
}

struct ThreadList {
    threads: Vec<Thread>,
    seen: Vec<u32>,
    gen: u32,
}

impl ThreadList {
    fn new(n_insts: usize) -> Self {
        ThreadList { threads: Vec::new(), seen: vec![0; n_insts], gen: 1 }
    }
}

/// Depth-first epsilon closure from `pc`, in priority order, using an
/// explicit stack (never native recursion) so pattern shape can never
/// overflow the stack. `pos`/`cur`/`prev` describe the *target* step this
/// closure is being built for (used to evaluate `Assert`s encountered
/// along the way).
#[allow(clippy::too_many_arguments)] // one step's worth of position context; a wrapper struct wouldn't shrink this
fn add_thread(
    list: &mut ThreadList,
    pc: usize,
    saves: Saves,
    pos: usize,
    cur: Option<char>,
    prev: Option<char>,
    prog: &Program,
    text_len: usize,
) {
    let mut stack = vec![(pc, saves)];
    while let Some((pc, saves)) = stack.pop() {
        if list.seen[pc] == list.gen {
            continue;
        }
        list.seen[pc] = list.gen;
        match &prog.insts[pc] {
            Inst::Jmp(t) => stack.push((*t, saves)),
            Inst::Split(a, b) => {
                stack.push((*b, Rc::clone(&saves)));
                stack.push((*a, saves));
            }
            Inst::Save(slot) => {
                let mut saves = saves;
                let s = Rc::make_mut(&mut saves);
                s[*slot] = Some(pos);
                stack.push((pc + 1, saves));
            }
            Inst::Assert(kind) => {
                if assert_holds(*kind, prev, cur, pos, text_len) {
                    stack.push((pc + 1, saves));
                }
            }
            Inst::Char(..) | Inst::Any | Inst::Class(..) | Inst::Shorthand(..) | Inst::Match => {
                list.threads.push(Thread { pc, saves });
            }
        }
    }
}

fn assert_holds(kind: Assert, prev: Option<char>, cur: Option<char>, pos: usize, text_len: usize) -> bool {
    match kind {
        Assert::StartText => pos == 0,
        Assert::EndText => pos == text_len,
        Assert::WordBoundary(want) => {
            let before = prev.is_some_and(is_word_char);
            let after = cur.is_some_and(is_word_char);
            (before != after) == want
        }
    }
}

use crate::ast::is_word_char;

pub(crate) fn run(prog: &Program, text: &str) -> Option<Vec<Option<usize>>> {
    let char_positions: Vec<(usize, char)> = text.char_indices().collect();
    let n_steps = char_positions.len();
    let mut positions = Vec::with_capacity(n_steps + 1);
    let mut chars_opt = Vec::with_capacity(n_steps + 1);
    for &(b, c) in &char_positions {
        positions.push(b);
        chars_opt.push(Some(c));
    }
    positions.push(text.len());
    chars_opt.push(None);

    let n_insts = prog.insts.len();
    let mut clist = ThreadList::new(n_insts);
    let mut nlist = ThreadList::new(n_insts);
    let mut matched: Option<Vec<Option<usize>>> = None;

    for step in 0..=n_steps {
        let pos = positions[step];
        let cur = chars_opt[step];
        let prev = if step > 0 { chars_opt[step - 1] } else { None };

        if matched.is_none() {
            let saves = Rc::new(vec![None; prog.num_slots]);
            add_thread(&mut clist, 0, saves, pos, cur, prev, prog, text.len());
        }
        if clist.threads.is_empty() {
            // Once a match is recorded, no lower-priority start can beat it.
            // Before that, an empty step (e.g. every thread died on a
            // failed `^`/`$`/`\b` assert) must not stop the search: a later
            // position still gets a fresh start-thread injection.
            if matched.is_some() {
                break;
            }
            std::mem::swap(&mut clist, &mut nlist);
            nlist.threads.clear();
            nlist.gen += 1;
            continue;
        }

        let next_pos = positions.get(step + 1).copied().unwrap_or(text.len());
        let next_cur = chars_opt.get(step + 1).copied().flatten();

        let mut i = 0;
        while i < clist.threads.len() {
            let th = clist.threads[i].clone();
            match &prog.insts[th.pc] {
                Inst::Char(ch, ci) => {
                    if let Some(c) = cur {
                        if char_matches(*ch, *ci, c) {
                            add_thread(&mut nlist, th.pc + 1, th.saves, next_pos, next_cur, cur, prog, text.len());
                        }
                    }
                }
                Inst::Any => {
                    if let Some(c) = cur {
                        if c != '\n' {
                            add_thread(&mut nlist, th.pc + 1, th.saves, next_pos, next_cur, cur, prog, text.len());
                        }
                    }
                }
                Inst::Class(spec, ci) => {
                    if let Some(c) = cur {
                        if class_matches(spec, *ci, c) {
                            add_thread(&mut nlist, th.pc + 1, th.saves, next_pos, next_cur, cur, prog, text.len());
                        }
                    }
                }
                Inst::Shorthand(kind, ci) => {
                    if let Some(c) = cur {
                        if shorthand_matches(*kind, *ci, c) {
                            add_thread(&mut nlist, th.pc + 1, th.saves, next_pos, next_cur, cur, prog, text.len());
                        }
                    }
                }
                Inst::Match => {
                    matched = Some((*th.saves).clone());
                    break;
                }
                _ => unreachable!("epsilon instructions never land in a thread list"),
            }
            i += 1;
        }

        std::mem::swap(&mut clist, &mut nlist);
        nlist.threads.clear();
        nlist.gen += 1;
    }
    matched
}

fn char_matches(pattern: char, ci: bool, c: char) -> bool {
    if ci { fold_eq(pattern, c) } else { pattern == c }
}

fn shorthand_matches(kind: Shorthand, ci: bool, c: char) -> bool {
    if ci { fold_variants(c).into_iter().flatten().any(|fc| kind.matches(fc)) } else { kind.matches(c) }
}

fn class_matches(spec: &ClassSpec, ci: bool, c: char) -> bool {
    let raw = |ch: char| {
        spec.items.iter().any(|item| match item {
            crate::ast::ClassItem::Range(lo, hi) => ch >= *lo && ch <= *hi,
            crate::ast::ClassItem::Shorthand(k) => k.matches(ch),
            crate::ast::ClassItem::Posix(p, neg) => p.matches(ch) != *neg,
        })
    };
    let membership = if ci { fold_variants(c).into_iter().flatten().any(raw) } else { raw(c) };
    membership != spec.negate
}

/// fold-set(x) = {x, lower(x), upper(x), lower(upper(x))} using std's
/// single-char-result case mappings only (multi-char results ignored).
fn fold_variants(c: char) -> [Option<char>; 4] {
    let mut v = [Some(c), None, None, None];
    let lower = single_char_lower(c);
    v[1] = lower;
    if let Some(u) = single_char_upper(c) {
        v[2] = Some(u);
        v[3] = single_char_lower(u);
    }
    v
}

fn single_char_lower(c: char) -> Option<char> {
    let mut it = c.to_lowercase();
    let f = it.next()?;
    if it.next().is_none() { Some(f) } else { None }
}

fn single_char_upper(c: char) -> Option<char> {
    let mut it = c.to_uppercase();
    let f = it.next()?;
    if it.next().is_none() { Some(f) } else { None }
}

fn fold_eq(a: char, b: char) -> bool {
    let fa = fold_variants(a);
    let fb = fold_variants(b);
    fa.into_iter().flatten().any(|x| fb.into_iter().flatten().any(|y| x == y))
}
