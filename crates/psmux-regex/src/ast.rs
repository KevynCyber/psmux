//! Parsed pattern tree, produced by `parse` and consumed by `compile`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shorthand {
    Digit,
    NotDigit,
    Word,
    NotWord,
    Space,
    NotSpace,
}

impl Shorthand {
    /// Base (non-case-folded) membership test for this shorthand class.
    pub(crate) fn matches(self, c: char) -> bool {
        match self {
            Shorthand::Digit => c.is_ascii_digit(),
            Shorthand::NotDigit => !c.is_ascii_digit(),
            Shorthand::Word => is_word_char(c),
            Shorthand::NotWord => !is_word_char(c),
            Shorthand::Space => c.is_whitespace(),
            Shorthand::NotSpace => !c.is_whitespace(),
        }
    }
}

/// `\w` definition shared by `\w`, `\b`/`\B`, and the ASCII `word` POSIX class.
pub(crate) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Posix {
    Alpha,
    Alnum,
    Digit,
    Upper,
    Lower,
    Space,
    Punct,
    Xdigit,
    Cntrl,
    Print,
    Graph,
    Blank,
    Ascii,
    Word,
}

impl Posix {
    pub(crate) fn from_name(name: &str) -> Option<Posix> {
        Some(match name {
            "alpha" => Posix::Alpha,
            "alnum" => Posix::Alnum,
            "digit" => Posix::Digit,
            "upper" => Posix::Upper,
            "lower" => Posix::Lower,
            "space" => Posix::Space,
            "punct" => Posix::Punct,
            "xdigit" => Posix::Xdigit,
            "cntrl" => Posix::Cntrl,
            "print" => Posix::Print,
            "graph" => Posix::Graph,
            "blank" => Posix::Blank,
            "ascii" => Posix::Ascii,
            "word" => Posix::Word,
            _ => return None,
        })
    }

    /// Base (non-case-folded) membership test; every class is ASCII-only
    /// per the documented simplification (non-ASCII input never matches).
    pub(crate) fn matches(self, c: char) -> bool {
        match self {
            Posix::Alpha => c.is_ascii_alphabetic(),
            Posix::Alnum => c.is_ascii_alphanumeric(),
            Posix::Digit => c.is_ascii_digit(),
            Posix::Upper => c.is_ascii_uppercase(),
            Posix::Lower => c.is_ascii_lowercase(),
            Posix::Space => matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C'),
            Posix::Punct => c.is_ascii_punctuation(),
            Posix::Xdigit => c.is_ascii_hexdigit(),
            Posix::Cntrl => c.is_ascii_control(),
            Posix::Print => c.is_ascii() && (c as u32) >= 0x20 && (c as u32) <= 0x7E,
            Posix::Graph => c.is_ascii_graphic(),
            Posix::Blank => matches!(c, ' ' | '\t'),
            Posix::Ascii => c.is_ascii(),
            Posix::Word => is_word_char(c) && c.is_ascii(),
        }
    }
}

/// One member of a bracket expression: a single-char range, a shorthand
/// class (`\d` etc.), or a POSIX `[:name:]`/`[:^name:]` item.
#[derive(Debug, Clone)]
pub(crate) enum ClassItem {
    Range(char, char),
    Shorthand(Shorthand),
    Posix(Posix, bool), // (kind, negated)
}

#[derive(Debug, Clone)]
pub(crate) struct ClassSpec {
    pub(crate) items: Vec<ClassItem>,
    pub(crate) negate: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Ast {
    Empty,
    Char(char),
    AnyChar,
    Class(ClassSpec),
    Shorthand(Shorthand),
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Star(Box<Ast>, bool),
    Plus(Box<Ast>, bool),
    Question(Box<Ast>, bool),
    Repeat(Box<Ast>, usize, Option<usize>, bool),
    Group(Box<Ast>, usize),
    StartText,
    EndText,
    WordBoundary(bool),
}
