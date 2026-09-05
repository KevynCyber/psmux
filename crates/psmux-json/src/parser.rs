use crate::error::Error;
use crate::number::Number;
use crate::value::Value;

/// Matches serde_json 1.0.151's default recursion limit: 127 nested
/// arrays/objects parse, 128 is rejected (docs/features/zero-deps.md
/// ZDEP-014, measured against the oracle fixture).
const MAX_DEPTH: u32 = 127;

/// Parses a complete JSON document per RFC 8259: no trailing commas,
/// comments, leading zeros, `NaN`/`Infinity`, single quotes, or unescaped
/// control characters in strings; nesting past `MAX_DEPTH` is rejected
/// before recursing further, so pathological input cannot overflow the
/// stack.
pub fn parse(input: &str) -> Result<Value, Error> {
    let mut p = Parser { bytes: input.as_bytes(), pos: 0 };
    p.skip_ws();
    let value = p.parse_value(1)?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(Error::new("trailing characters after JSON value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.bump();
        }
    }

    /// `depth` is the nesting depth this value would have if it opens a new
    /// container (1 for the top-level value).
    fn parse_value(&mut self, depth: u32) -> Result<Value, Error> {
        self.skip_ws();
        match self.peek() {
            None => Err(Error::new("unexpected end of input")),
            Some(b'{') | Some(b'[') if depth > MAX_DEPTH => {
                Err(Error::new("recursion limit exceeded"))
            }
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b't') => self.parse_literal("true", Value::Bool(true)),
            Some(b'f') => self.parse_literal("false", Value::Bool(false)),
            Some(b'n') => self.parse_literal("null", Value::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(Error::new(format!("unexpected character {:?}", c as char))),
        }
    }

    fn parse_literal(&mut self, lit: &str, value: Value) -> Result<Value, Error> {
        if self.bytes[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            Ok(value)
        } else {
            Err(Error::new(format!("expected literal {lit:?}")))
        }
    }

    fn parse_object(&mut self, depth: u32) -> Result<Value, Error> {
        self.bump(); // '{'
        let mut pairs: Vec<(String, Value)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(Error::new("expected string key"));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(Error::new("expected ':' after object key"));
            }
            self.bump();
            let value = self.parse_value(depth + 1)?;
            match pairs.iter_mut().find(|(k, _)| *k == key) {
                Some(existing) => existing.1 = value,
                None => pairs.push((key, value)),
            }
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        return Err(Error::new("trailing comma in object"));
                    }
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Value::Object(pairs));
                }
                _ => return Err(Error::new("expected ',' or '}' in object")),
            }
        }
    }

    fn parse_array(&mut self, depth: u32) -> Result<Value, Error> {
        self.bump(); // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Value::Array(items));
        }
        loop {
            let value = self.parse_value(depth + 1)?;
            items.push(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        return Err(Error::new("trailing comma in array"));
                    }
                }
                Some(b']') => {
                    self.bump();
                    return Ok(Value::Array(items));
                }
                _ => return Err(Error::new("expected ',' or ']' in array")),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, Error> {
        self.bump(); // opening '"'
        let mut s = String::new();
        loop {
            match self.peek() {
                None => return Err(Error::new("unterminated string")),
                Some(b'"') => {
                    self.bump();
                    return Ok(s);
                }
                Some(b'\\') => {
                    self.bump();
                    self.parse_escape(&mut s)?;
                }
                Some(b) if b < 0x20 => {
                    return Err(Error::new("unescaped control character in string"));
                }
                Some(_) => {
                    let len = utf8_char_len(self.bytes[self.pos]);
                    let end = (self.pos + len).min(self.bytes.len());
                    let text = std::str::from_utf8(&self.bytes[self.pos..end])
                        .map_err(|_| Error::new("invalid utf-8 in string"))?;
                    s.push_str(text);
                    self.pos = end;
                }
            }
        }
    }

    fn parse_escape(&mut self, s: &mut String) -> Result<(), Error> {
        match self.peek() {
            Some(b'"') => { s.push('"'); self.bump(); }
            Some(b'\\') => { s.push('\\'); self.bump(); }
            Some(b'/') => { s.push('/'); self.bump(); }
            Some(b'b') => { s.push('\u{8}'); self.bump(); }
            Some(b'f') => { s.push('\u{c}'); self.bump(); }
            Some(b'n') => { s.push('\n'); self.bump(); }
            Some(b'r') => { s.push('\r'); self.bump(); }
            Some(b't') => { s.push('\t'); self.bump(); }
            Some(b'u') => {
                self.bump();
                let cp1 = self.parse_hex4()?;
                if (0xD800..=0xDBFF).contains(&cp1) {
                    if self.peek() != Some(b'\\') {
                        return Err(Error::new("lone high surrogate"));
                    }
                    self.bump();
                    if self.peek() != Some(b'u') {
                        return Err(Error::new("expected \\u after high surrogate"));
                    }
                    self.bump();
                    let cp2 = self.parse_hex4()?;
                    if !(0xDC00..=0xDFFF).contains(&cp2) {
                        return Err(Error::new("expected low surrogate"));
                    }
                    let combined = 0x10000 + ((cp1 - 0xD800) << 10) + (cp2 - 0xDC00);
                    let ch = char::from_u32(combined)
                        .ok_or_else(|| Error::new("invalid surrogate pair"))?;
                    s.push(ch);
                } else if (0xDC00..=0xDFFF).contains(&cp1) {
                    return Err(Error::new("lone low surrogate"));
                } else {
                    let ch = char::from_u32(cp1).ok_or_else(|| Error::new("invalid \\u escape"))?;
                    s.push(ch);
                }
            }
            _ => return Err(Error::new("invalid escape sequence")),
        }
        Ok(())
    }

    fn parse_hex4(&mut self) -> Result<u32, Error> {
        let mut v = 0u32;
        for _ in 0..4 {
            let c = self.peek().ok_or_else(|| Error::new("unexpected end of \\u escape"))?;
            let d = (c as char)
                .to_digit(16)
                .ok_or_else(|| Error::new("invalid hex digit in \\u escape"))?;
            v = v * 16 + d;
            self.bump();
        }
        Ok(v)
    }

    fn parse_number(&mut self) -> Result<Value, Error> {
        let start = self.pos;
        let mut is_float = false;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek() {
            Some(b'0') => self.bump(),
            Some(c) if c.is_ascii_digit() => {
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.bump();
                }
            }
            _ => return Err(Error::new("invalid number")),
        }
        if self.peek() == Some(b'.') {
            is_float = true;
            self.bump();
            if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                return Err(Error::new("expected digit after decimal point"));
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                return Err(Error::new("expected digit in exponent"));
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.bump();
            }
        }
        // Safe: every byte consumed above is ASCII.
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
        if is_float {
            let f: f64 = text.parse().map_err(|_| Error::new("invalid float literal"))?;
            if !f.is_finite() {
                return Err(Error::new("number out of range"));
            }
            Ok(Value::Number(Number::Float(f)))
        } else {
            parse_integer(text)
        }
    }
}

fn parse_integer(text: &str) -> Result<Value, Error> {
    if let Some(digits) = text.strip_prefix('-') {
        if digits == "0" {
            // Neither integer variant can hold a negative zero.
            return Ok(Value::Number(Number::Float(-0.0)));
        }
        return match digits.parse::<u64>() {
            Ok(v) if v <= i64::MAX as u64 + 1 => {
                let iv = if v == i64::MAX as u64 + 1 { i64::MIN } else { -(v as i64) };
                Ok(Value::Number(Number::NegInt(iv)))
            }
            // Beyond i64::MIN: not exercised by the oracle fixture; keep the
            // value as the closest f64 rather than failing outright.
            _ => Ok(Value::Number(Number::Float(text.parse().unwrap_or(f64::NEG_INFINITY)))),
        };
    }
    match text.parse::<u64>() {
        Ok(v) => Ok(Value::Number(Number::PosInt(v))),
        Err(_) => Ok(Value::Number(Number::Float(text.parse().unwrap_or(f64::INFINITY)))),
    }
}

/// UTF-8 sequence length from a leading byte. The parser only ever calls
/// this on bytes drawn from a valid `&str`, so the sequence is always
/// well-formed and fully present.
fn utf8_char_len(b: u8) -> usize {
    if b & 0x80 == 0 {
        1
    } else if b & 0xE0 == 0xC0 {
        2
    } else if b & 0xF0 == 0xE0 {
        3
    } else if b & 0xF8 == 0xF0 {
        4
    } else {
        1
    }
}
