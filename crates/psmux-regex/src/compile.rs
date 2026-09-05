//! Thompson-style compiler: `Ast` -> a flat `Vec<Inst>` program executed by
//! the Pike VM in `exec.rs`. `{m,n}` is expanded here by copying the body
//! AST (m required copies, then either a trailing `Star` for `{m,}` or
//! `n - m` trailing `Question`s for `{m,n}`), never at parse time.

use crate::ast::{Ast, ClassSpec, Shorthand};
use crate::error::Error;
use crate::parse::Parsed;

pub(crate) const MAX_INSTS: usize = 100_000;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Assert {
    StartText,
    EndText,
    WordBoundary(bool),
}

#[derive(Debug, Clone)]
pub(crate) enum Inst {
    Char(char, bool),
    Any,
    Class(ClassSpec, bool),
    Shorthand(Shorthand, bool),
    Split(usize, usize),
    Jmp(usize),
    Save(usize),
    Assert(Assert),
    Match,
}

pub(crate) struct Program {
    pub(crate) insts: Vec<Inst>,
    pub(crate) num_slots: usize,
}

struct Compiler {
    insts: Vec<Inst>,
}

impl Compiler {
    fn emit(&mut self, inst: Inst) -> Result<usize, Error> {
        self.insts.push(inst);
        if self.insts.len() > MAX_INSTS {
            return Err(Error::new("compiled program exceeds 100000 instructions"));
        }
        Ok(self.insts.len() - 1)
    }

    fn here(&self) -> usize {
        self.insts.len()
    }
}

pub(crate) fn compile(parsed: &Parsed) -> Result<Program, Error> {
    let mut c = Compiler { insts: Vec::new() };
    c.emit(Inst::Save(0))?;
    compile_ast(&mut c, &parsed.ast, parsed.ci)?;
    c.emit(Inst::Save(1))?;
    c.emit(Inst::Match)?;
    Ok(Program { insts: c.insts, num_slots: 2 * parsed.group_count })
}

fn compile_ast(c: &mut Compiler, ast: &Ast, ci: bool) -> Result<(), Error> {
    match ast {
        Ast::Empty => Ok(()),
        Ast::Char(ch) => {
            c.emit(Inst::Char(*ch, ci))?;
            Ok(())
        }
        Ast::AnyChar => {
            c.emit(Inst::Any)?;
            Ok(())
        }
        Ast::Class(spec) => {
            c.emit(Inst::Class(spec.clone(), ci))?;
            Ok(())
        }
        Ast::Shorthand(k) => {
            c.emit(Inst::Shorthand(*k, ci))?;
            Ok(())
        }
        Ast::StartText => {
            c.emit(Inst::Assert(Assert::StartText))?;
            Ok(())
        }
        Ast::EndText => {
            c.emit(Inst::Assert(Assert::EndText))?;
            Ok(())
        }
        Ast::WordBoundary(b) => {
            c.emit(Inst::Assert(Assert::WordBoundary(*b)))?;
            Ok(())
        }
        Ast::Concat(items) => {
            for item in items {
                compile_ast(c, item, ci)?;
            }
            Ok(())
        }
        Ast::Alt(branches) => compile_alt(c, branches, ci),
        Ast::Star(body, greedy) => compile_star(c, body, *greedy, ci),
        Ast::Plus(body, greedy) => compile_plus(c, body, *greedy, ci),
        Ast::Question(body, greedy) => compile_question(c, body, *greedy, ci),
        Ast::Repeat(body, m, n, greedy) => compile_repeat(c, body, *m, *n, *greedy, ci),
        Ast::Group(inner, idx) => {
            c.emit(Inst::Save(2 * idx))?;
            compile_ast(c, inner, ci)?;
            c.emit(Inst::Save(2 * idx + 1))?;
            Ok(())
        }
    }
}

fn compile_alt(c: &mut Compiler, branches: &[Ast], ci: bool) -> Result<(), Error> {
    // Iterative rather than recursing on branches[1..]: a chain of N
    // alternations previously recursed N deep (one native stack frame per
    // branch), which overflows a small stack well before N reaches the tens
    // of thousands. Each non-last branch still gets a Split guarding it from
    // the rest of the chain and a Jmp to the shared end, exactly as the
    // recursive version produced -- just built in a flat loop.
    if branches.len() == 1 {
        return compile_ast(c, &branches[0], ci);
    }
    let mut jmp_idxs = Vec::new();
    for (i, branch) in branches.iter().enumerate() {
        if i + 1 == branches.len() {
            compile_ast(c, branch, ci)?;
            break;
        }
        let split_idx = c.emit(Inst::Split(0, 0))?;
        let a_start = c.here();
        compile_ast(c, branch, ci)?;
        let jmp_idx = c.emit(Inst::Jmp(0))?;
        let b_start = c.here();
        c.insts[split_idx] = Inst::Split(a_start, b_start);
        jmp_idxs.push(jmp_idx);
    }
    let end = c.here();
    for jmp_idx in jmp_idxs {
        c.insts[jmp_idx] = Inst::Jmp(end);
    }
    Ok(())
}

fn compile_star(c: &mut Compiler, body: &Ast, greedy: bool, ci: bool) -> Result<(), Error> {
    // The loop-back uses a *second* Split instruction rather than a Jmp to
    // the entry Split: for a nullable body (e.g. `(a*)*`), the one-iteration
    // path must reach the shared exit target before the zero-iteration path
    // does, so it wins priority and its (better) captures are recorded. A
    // Jmp back to the same pc would be blocked by the per-step visited set
    // before it could ever reach the exit with those captures.
    let l1 = c.emit(Inst::Split(0, 0))?;
    let body_start = c.here();
    compile_ast(c, body, ci)?;
    let l2 = c.emit(Inst::Split(0, 0))?;
    let out = c.here();
    c.insts[l1] = if greedy { Inst::Split(body_start, out) } else { Inst::Split(out, body_start) };
    c.insts[l2] = if greedy { Inst::Split(body_start, out) } else { Inst::Split(out, body_start) };
    Ok(())
}

fn compile_plus(c: &mut Compiler, body: &Ast, greedy: bool, ci: bool) -> Result<(), Error> {
    let body_start = c.here();
    compile_ast(c, body, ci)?;
    let l2 = c.emit(Inst::Split(0, 0))?;
    let out = c.here();
    c.insts[l2] = if greedy { Inst::Split(body_start, out) } else { Inst::Split(out, body_start) };
    Ok(())
}

fn compile_question(c: &mut Compiler, body: &Ast, greedy: bool, ci: bool) -> Result<(), Error> {
    let l1 = c.emit(Inst::Split(0, 0))?;
    let body_start = c.here();
    compile_ast(c, body, ci)?;
    let out = c.here();
    c.insts[l1] = if greedy { Inst::Split(body_start, out) } else { Inst::Split(out, body_start) };
    Ok(())
}

fn compile_repeat(
    c: &mut Compiler,
    body: &Ast,
    m: usize,
    n: Option<usize>,
    greedy: bool,
    ci: bool,
) -> Result<(), Error> {
    for _ in 0..m {
        compile_ast(c, body, ci)?;
    }
    match n {
        None => compile_star(c, body, greedy, ci),
        Some(n) => {
            for _ in 0..(n - m) {
                compile_question(c, body, greedy, ci)?;
            }
            Ok(())
        }
    }
}
