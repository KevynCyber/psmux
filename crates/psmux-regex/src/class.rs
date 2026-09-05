//! Bracket-expression (`[...]`) parsing.
//!
//! Dash handling follows the documented regex-syntax rule: a literal `-`
//! read directly at the top of the item loop (i.e. not already consumed as
//! part of a `char '-' char` range) is *always* a plain member, never a
//! range's left endpoint. A range is only formed when a just-read atom is
//! immediately followed by an unescaped `-` that is not itself immediately
//! before the closing `]`.

use crate::ast::{Ast, ClassItem, ClassSpec, Posix};
use crate::error::Error;
use crate::parse::{EscOutcome, Parser};

pub(crate) fn parse_class(p: &mut Parser) -> Result<Ast, Error> {
    p.pos += 1; // consume '['
    if p.depth >= crate::parse::MAX_DEPTH {
        return Err(Error::new("nesting depth exceeds 64"));
    }
    p.depth += 1;
    let negate = if p.peek() == Some('^') {
        p.pos += 1;
        true
    } else {
        false
    };
    let mut items = Vec::new();
    // First-char-is-']' is a literal member, not the close bracket.
    if p.peek() == Some(']') {
        p.pos += 1;
        items.push(ClassItem::Range(']', ']'));
    }
    loop {
        match p.peek() {
            None => return Err(Error::new("unterminated character class")),
            Some(']') => {
                p.pos += 1;
                break;
            }
            Some('[') => {
                if let Some(item) = try_parse_posix(p)? {
                    items.push(item);
                } else {
                    return Err(Error::new("nested/POSIX-only bracket forms are unsupported"));
                }
            }
            Some('-') => {
                p.pos += 1;
                items.push(ClassItem::Range('-', '-'));
            }
            Some('&') if p.chars.get(p.pos + 1) == Some(&'&') => {
                return Err(Error::new("class set intersection (&&) is unsupported"));
            }
            Some('~') if p.chars.get(p.pos + 1) == Some(&'~') => {
                return Err(Error::new("class set symmetric difference (~~) is unsupported"));
            }
            Some('\\') => {
                p.pos += 1;
                match p.parse_escape(true)? {
                    EscOutcome::Shorthand(k) => {
                        reject_dash_after_shorthand(p)?;
                        items.push(ClassItem::Shorthand(k));
                    }
                    EscOutcome::Char(c) => items.push(read_range_or_single(p, c)?),
                    EscOutcome::Boundary(_) => unreachable!("parse_escape(true) never yields Boundary"),
                }
            }
            Some(c) => {
                p.pos += 1;
                items.push(read_range_or_single(p, c)?);
            }
        }
    }
    p.depth -= 1;
    Ok(Ast::Class(ClassSpec { items, negate }))
}

/// After reading atom `c`, checks for a `-end` range suffix (skipped when
/// the dash is absent or sits immediately before the closing `]`).
fn read_range_or_single(p: &mut Parser, c: char) -> Result<ClassItem, Error> {
    if p.peek() == Some('-') && !dash_is_last(p) {
        p.pos += 1; // consume '-'
        let end = read_single_char_atom(p)?;
        if end < c {
            return Err(Error::new("descending class range"));
        }
        return Ok(ClassItem::Range(c, end));
    }
    Ok(ClassItem::Range(c, c))
}

fn reject_dash_after_shorthand(p: &Parser) -> Result<(), Error> {
    if p.peek() == Some('-') && !dash_is_last(p) {
        return Err(Error::new("a shorthand class cannot start a range"));
    }
    Ok(())
}

/// True when the `-` at the current position is immediately followed by
/// the class-closing `]` (i.e. it must be read as a literal, not an
/// operator).
fn dash_is_last(p: &Parser) -> bool {
    p.chars.get(p.pos + 1) == Some(&']')
}

/// Reads a single-char range endpoint: a plain literal or a single-char
/// escape. A shorthand/POSIX atom here is an error (can't end a range).
fn read_single_char_atom(p: &mut Parser) -> Result<char, Error> {
    match p.peek() {
        Some('\\') => {
            p.pos += 1;
            match p.parse_escape(true)? {
                EscOutcome::Char(c) => Ok(c),
                EscOutcome::Shorthand(_) => Err(Error::new("a shorthand class cannot end a range")),
                EscOutcome::Boundary(_) => unreachable!(),
            }
        }
        Some(c) => {
            p.pos += 1;
            Ok(c)
        }
        None => Err(Error::new("unterminated character class")),
    }
}

/// Parses `[:name:]` / `[:^name:]` when the current position is `[` and
/// the next char is `:`. Returns `Ok(None)` when it's some other bracket
/// form nested in the class (unsupported/nested), which the caller turns
/// into an `Err`.
fn try_parse_posix(p: &mut Parser) -> Result<Option<ClassItem>, Error> {
    if p.chars.get(p.pos + 1) != Some(&':') {
        return Ok(None);
    }
    p.pos += 2; // consume "[:"
    let negate = if p.peek() == Some('^') {
        p.pos += 1;
        true
    } else {
        false
    };
    let start = p.pos;
    while matches!(p.peek(), Some(c) if c.is_ascii_alphabetic()) {
        p.pos += 1;
    }
    let name: String = p.chars[start..p.pos].iter().collect();
    if p.peek() != Some(':') || p.chars.get(p.pos + 1) != Some(&']') {
        return Err(Error::new("unterminated [:name:]"));
    }
    p.pos += 2; // consume ":]"
    let kind = Posix::from_name(&name).ok_or_else(|| Error::new(format!("unknown POSIX class {name}")))?;
    Ok(Some(ClassItem::Posix(kind, negate)))
}
