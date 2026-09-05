//! Zero-dependency regex engine reproducing regex 1.13.1 on the documented
//! subset (docs/features/zero-deps.md ZDEP-017/ZDEP-018): a Pike VM over a
//! parsed-and-compiled program, giving leftmost-first semantics with no
//! backtracking.

mod ast;
mod class;
mod compile;
mod error;
mod exec;
mod parse;
mod replace;

pub use error::Error;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Regex {
    program: Rc<compile::Program>,
    names: Rc<HashMap<String, usize>>,
    num_groups: usize,
}

impl std::fmt::Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Regex")
    }
}

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, Error> {
        let parsed = parse::parse(pattern)?;
        let group_count = parsed.group_count;
        let names = parsed.names.clone();
        let program = compile::compile(&parsed)?;
        Ok(Regex { program: Rc::new(program), names: Rc::new(names), num_groups: group_count })
    }

    pub fn is_match(&self, text: &str) -> bool {
        exec::run(&self.program, text).is_some()
    }

    pub fn find(&self, text: &str) -> Option<(usize, usize)> {
        self.captures(text)?.get(0)
    }

    pub fn captures(&self, text: &str) -> Option<Captures> {
        let saves = exec::run(&self.program, text)?;
        Some(Captures { saves, names: Rc::clone(&self.names), num_groups: self.num_groups })
    }

    /// Replaces the first match only; the value is left unchanged when
    /// there is no match.
    pub fn replace(&self, text: &str, replacement: &str) -> String {
        match self.captures(text) {
            None => text.to_string(),
            Some(caps) => {
                let (s, e) = caps.get(0).expect("group 0 always set on a match");
                let mut out = String::with_capacity(text.len());
                out.push_str(&text[..s]);
                out.push_str(&replace::expand_replacement(&caps, text, &self.names, replacement));
                out.push_str(&text[e..]);
                out
            }
        }
    }
}

pub struct Captures {
    saves: Vec<Option<usize>>,
    names: Rc<HashMap<String, usize>>,
    num_groups: usize,
}

impl Captures {
    pub fn get(&self, i: usize) -> Option<(usize, usize)> {
        if i >= self.num_groups {
            return None;
        }
        match (self.saves.get(2 * i).copied().flatten(), self.saves.get(2 * i + 1).copied().flatten()) {
            (Some(s), Some(e)) => Some((s, e)),
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        self.num_groups
    }

    pub fn is_empty(&self) -> bool {
        self.num_groups == 0
    }

    pub fn name(&self, name: &str) -> Option<(usize, usize)> {
        let idx = *self.names.get(name)?;
        self.get(idx)
    }
}

/// Escapes exactly `\ . + * ? ( ) | [ ] { } ^ $ # & - ~`; every other
/// character (including space, `/`, `_`, `:`, and non-ASCII) survives
/// unchanged.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_meta(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn is_meta(c: char) -> bool {
    matches!(
        c,
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '#' | '&' | '-' | '~'
    )
}
