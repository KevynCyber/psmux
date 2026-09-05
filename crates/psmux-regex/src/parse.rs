//! Recursive-descent parser for the documented regex 1.13.1 subset
//! (docs/features/zero-deps.md ZDEP-017). Depth (groups + classes) is
//! checked *before* recursing, so a pathologically deep pattern (e.g.
//! 1,000 nested `(`) returns `Err` after at most 65 stack frames instead
//! of recursing proportionally to the input.

use crate::ast::Ast;
use crate::error::Error;
use std::collections::HashMap;

pub(crate) const MAX_DEPTH: usize = 64;
pub(crate) const MAX_REPEAT: u64 = 1000;

pub(crate) struct Parsed {
    pub(crate) ast: Ast,
    pub(crate) group_count: usize, // includes group 0 (whole match)
    pub(crate) names: HashMap<String, usize>,
    pub(crate) ci: bool,
}

pub(crate) struct Parser {
    pub(crate) chars: Vec<char>,
    pub(crate) pos: usize,
    pub(crate) depth: usize,
    pub(crate) group_count: usize,
    pub(crate) names: HashMap<String, usize>,
}

pub(crate) fn parse(pattern: &str) -> Result<Parsed, Error> {
    let mut chars: Vec<char> = pattern.chars().collect();
    let mut pos = 0;
    let mut ci = false;
    loop {
        if chars[pos..].starts_with(&['(', '?', 'i', ')']) {
            ci = true;
            pos += 4;
        } else {
            break;
        }
    }
    chars.drain(0..pos);
    let mut p = Parser { chars, pos: 0, depth: 0, group_count: 1, names: HashMap::new() };
    let ast = p.parse_alternation()?;
    if p.pos != p.chars.len() {
        return Err(Error::new("unexpected trailing character (unmatched ')'?)"));
    }
    Ok(Parsed { ast, group_count: p.group_count, names: p.names, ci })
}

impl Parser {
    pub(crate) fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    pub(crate) fn parse_alternation(&mut self) -> Result<Ast, Error> {
        let mut branches = vec![self.parse_concat()?];
        while self.peek() == Some('|') {
            self.pos += 1;
            branches.push(self.parse_concat()?);
        }
        Ok(if branches.len() == 1 { branches.pop().unwrap() } else { Ast::Alt(branches) })
    }

    fn parse_concat(&mut self) -> Result<Ast, Error> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            items.push(self.parse_repeat()?);
        }
        Ok(match items.len() {
            0 => Ast::Empty,
            1 => items.pop().unwrap(),
            _ => Ast::Concat(items),
        })
    }

    fn parse_repeat(&mut self) -> Result<Ast, Error> {
        let mut atom = self.parse_atom()?;
        loop {
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    let greedy = !self.eat_lazy_marker();
                    atom = Ast::Star(Box::new(atom), greedy);
                }
                Some('+') => {
                    self.pos += 1;
                    let greedy = !self.eat_lazy_marker();
                    atom = Ast::Plus(Box::new(atom), greedy);
                }
                Some('?') => {
                    self.pos += 1;
                    let greedy = !self.eat_lazy_marker();
                    atom = Ast::Question(Box::new(atom), greedy);
                }
                Some('{') => {
                    let (m, n) = self.parse_brace()?;
                    let greedy = !self.eat_lazy_marker();
                    atom = Ast::Repeat(Box::new(atom), m, n, greedy);
                }
                _ => break,
            }
        }
        Ok(atom)
    }

    fn eat_lazy_marker(&mut self) -> bool {
        if self.peek() == Some('?') {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Parses `{m}` / `{m,}` / `{m,n}`, current char must be `{`. Enforces
    /// n > 1000 -> Err before any expansion happens (expansion is compile's job).
    fn parse_brace(&mut self) -> Result<(usize, Option<usize>), Error> {
        self.pos += 1; // consume '{'
        let m = self.parse_count()?;
        let n = if self.peek() == Some(',') {
            self.pos += 1;
            if self.peek() == Some('}') {
                None
            } else {
                Some(self.parse_count()?)
            }
        } else {
            Some(m)
        };
        if self.peek() != Some('}') {
            return Err(Error::new("unterminated {m,n} quantifier"));
        }
        self.pos += 1;
        if let Some(nn) = n {
            if nn < m {
                return Err(Error::new("{m,n}: n < m"));
            }
            if nn as u64 > MAX_REPEAT {
                return Err(Error::new("{m,n}: n exceeds the 1000 limit"));
            }
        }
        Ok((m, n))
    }

    fn parse_count(&mut self) -> Result<usize, Error> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(Error::new("expected digits in {m,n} quantifier"));
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        let v: u64 = s.parse().map_err(|_| Error::new("{m,n} count overflow"))?;
        // Cap at MAX_REPEAT+1 (a sentinel that is still > MAX_REPEAT) so an
        // astronomically large literal count never becomes a huge `usize`
        // expansion request; parse_brace rejects anything over MAX_REPEAT.
        Ok(v.min(MAX_REPEAT + 1) as usize)
    }

    fn parse_atom(&mut self) -> Result<Ast, Error> {
        match self.peek() {
            None => Err(Error::new("unexpected end of pattern")),
            Some('*') | Some('+') | Some('?') | Some('{') => {
                Err(Error::new("quantifier with no preceding atom"))
            }
            Some('.') => {
                self.pos += 1;
                Ok(Ast::AnyChar)
            }
            Some('^') => {
                self.pos += 1;
                Ok(Ast::StartText)
            }
            Some('$') => {
                self.pos += 1;
                Ok(Ast::EndText)
            }
            Some('(') => self.parse_group(),
            Some('[') => crate::class::parse_class(self),
            Some(']') | Some('}') | Some('#') => {
                let c = self.bump().unwrap();
                Ok(Ast::Char(c))
            }
            Some('\\') => {
                self.pos += 1;
                self.parse_escape_atom()
            }
            Some(c) => {
                self.pos += 1;
                Ok(Ast::Char(c))
            }
        }
    }

    fn parse_escape_atom(&mut self) -> Result<Ast, Error> {
        match self.parse_escape(false)? {
            EscOutcome::Char(c) => Ok(Ast::Char(c)),
            EscOutcome::Shorthand(k) => Ok(Ast::Shorthand(k)),
            EscOutcome::Boundary(b) => Ok(Ast::WordBoundary(b)),
        }
    }

    /// Shared escape reader for both atom position and class-item position.
    /// `in_class` disables `\b`/`\B` (word-boundary assertions make no
    /// sense as a class member; the fixture documents `[\b]` as `Err`).
    pub(crate) fn parse_escape(&mut self, in_class: bool) -> Result<EscOutcome, Error> {
        let c = self.bump().ok_or_else(|| Error::new("trailing backslash"))?;
        use crate::ast::Shorthand::*;
        Ok(match c {
            'd' => EscOutcome::Shorthand(Digit),
            'D' => EscOutcome::Shorthand(NotDigit),
            'w' => EscOutcome::Shorthand(Word),
            'W' => EscOutcome::Shorthand(NotWord),
            's' => EscOutcome::Shorthand(Space),
            'S' => EscOutcome::Shorthand(NotSpace),
            'b' if !in_class => EscOutcome::Boundary(true),
            'B' if !in_class => EscOutcome::Boundary(false),
            'n' => EscOutcome::Char('\n'),
            't' => EscOutcome::Char('\t'),
            'r' => EscOutcome::Char('\r'),
            'f' => EscOutcome::Char('\x0C'),
            'v' => EscOutcome::Char('\x0B'),
            'a' => EscOutcome::Char('\x07'),
            'x' => EscOutcome::Char(self.parse_hex_escape()?),
            c if is_literal_escapable(c) => EscOutcome::Char(c),
            _ => return Err(Error::new(format!("unknown escape \\{c}"))),
        })
    }

    fn parse_hex_escape(&mut self) -> Result<char, Error> {
        if self.peek() == Some('{') {
            self.pos += 1;
            let start = self.pos;
            while matches!(self.peek(), Some(c) if c.is_ascii_hexdigit()) {
                self.pos += 1;
            }
            if self.pos == start || self.peek() != Some('}') {
                return Err(Error::new("malformed \\x{...} escape"));
            }
            let hex: String = self.chars[start..self.pos].iter().collect();
            self.pos += 1; // consume '}'
            let code = u32::from_str_radix(&hex, 16).map_err(|_| Error::new("bad hex digits"))?;
            char::from_u32(code).ok_or_else(|| Error::new("\\x{...} out of scalar range"))
        } else {
            let start = self.pos;
            while self.pos < start + 2 && matches!(self.peek(), Some(c) if c.is_ascii_hexdigit()) {
                self.pos += 1;
            }
            if self.pos != start + 2 {
                return Err(Error::new("\\xHH requires exactly two hex digits"));
            }
            let hex: String = self.chars[start..self.pos].iter().collect();
            let code = u32::from_str_radix(&hex, 16).map_err(|_| Error::new("bad hex digits"))?;
            char::from_u32(code).ok_or_else(|| Error::new("\\xHH out of scalar range"))
        }
    }

    fn parse_group(&mut self) -> Result<Ast, Error> {
        self.pos += 1; // consume '('
        if self.peek() == Some('?') {
            self.pos += 1;
            return match self.peek() {
                Some(':') => {
                    self.pos += 1;
                    self.parse_group_body_uncaptured()
                }
                Some('P') => {
                    self.pos += 1;
                    if self.bump() != Some('<') {
                        return Err(Error::new("expected '<' after (?P"));
                    }
                    self.parse_named_group()
                }
                Some('<') => {
                    self.pos += 1;
                    self.parse_named_group()
                }
                // Any other flag/group form ((?i) mid-pattern, (?s), (?=..),
                // etc.) is out of the supported subset.
                _ => Err(Error::new("unsupported (?...) group form")),
            };
        }
        if self.depth >= MAX_DEPTH {
            return Err(Error::new("nesting depth exceeds 64"));
        }
        self.depth += 1;
        let idx = self.group_count;
        self.group_count += 1;
        let inner = self.parse_alternation()?;
        self.depth -= 1;
        if self.bump() != Some(')') {
            return Err(Error::new("unterminated group"));
        }
        Ok(Ast::Group(Box::new(inner), idx))
    }

    fn parse_group_body_uncaptured(&mut self) -> Result<Ast, Error> {
        if self.depth >= MAX_DEPTH {
            return Err(Error::new("nesting depth exceeds 64"));
        }
        self.depth += 1;
        let inner = self.parse_alternation()?;
        self.depth -= 1;
        if self.bump() != Some(')') {
            return Err(Error::new("unterminated group"));
        }
        Ok(inner)
    }

    fn parse_named_group(&mut self) -> Result<Ast, Error> {
        let start = self.pos;
        if !matches!(self.peek(), Some(c) if c.is_ascii_alphabetic() || c == '_') {
            return Err(Error::new("invalid group name"));
        }
        self.pos += 1;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        if self.bump() != Some('>') {
            return Err(Error::new("unterminated group name"));
        }
        if self.names.contains_key(&name) {
            return Err(Error::new(format!("duplicate group name {name}")));
        }
        if self.depth >= MAX_DEPTH {
            return Err(Error::new("nesting depth exceeds 64"));
        }
        self.depth += 1;
        let idx = self.group_count;
        self.group_count += 1;
        let inner = self.parse_alternation()?;
        self.depth -= 1;
        if self.bump() != Some(')') {
            return Err(Error::new("unterminated group"));
        }
        self.names.insert(name, idx);
        Ok(Ast::Group(Box::new(inner), idx))
    }
}

pub(crate) enum EscOutcome {
    Char(char),
    Shorthand(crate::ast::Shorthand),
    Boundary(bool),
}

/// The literal set from ZDEP-017: `\` + any of
/// `. + * ? ( ) | [ ] { } ^ $ # & - ~ / : ' " , = ! @ % _ ; \`` (backtick)
/// or a space.
pub(crate) fn is_literal_escapable(c: char) -> bool {
    matches!(
        c,
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '#'
            | '&' | '-' | '~' | '/' | ':' | '\'' | '"' | ',' | '=' | '!' | '@' | '%' | '_' | ';'
            | '`' | ' '
    )
}
