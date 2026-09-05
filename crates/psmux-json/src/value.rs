use std::fmt;
use std::ops::Index;

use crate::number::Number;

/// A parsed JSON value. `Object` keeps insertion order (a `Vec` of pairs,
/// last-write-wins on duplicate keys) rather than sorting keys, matching
/// what ZDEP-014 requires for byte-stable round trips.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

static NULL: Value = Value::Null;

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, Value)>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Looks up a key on an object; `None` for a missing key or a non-object
    /// value (matches serde_json's `Value::get`, unlike `Index` which never
    /// fails and returns a shared `Null` instead).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

impl<'a> Index<&'a str> for Value {
    type Output = Value;
    fn index(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&NULL)
    }
}

impl Index<usize> for Value {
    type Output = Value;
    fn index(&self, idx: usize) -> &Value {
        match self {
            Value::Array(arr) => arr.get(idx).unwrap_or(&NULL),
            _ => &NULL,
        }
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Value::String(s) if s == *other)
    }
}

impl PartialEq<bool> for Value {
    fn eq(&self, other: &bool) -> bool {
        matches!(self, Value::Bool(b) if b == other)
    }
}

impl PartialEq<i64> for Value {
    fn eq(&self, other: &i64) -> bool {
        self.as_i64() == Some(*other)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_value(self, f)
    }
}

fn write_value(v: &Value, out: &mut impl fmt::Write) -> fmt::Result {
    match v {
        Value::Null => out.write_str("null"),
        Value::Bool(true) => out.write_str("true"),
        Value::Bool(false) => out.write_str("false"),
        Value::Number(n) => out.write_str(&n.to_display_string()),
        Value::String(s) => write_json_string(s, out),
        Value::Array(arr) => {
            out.write_char('[')?;
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    out.write_char(',')?;
                }
                write_value(item, out)?;
            }
            out.write_char(']')
        }
        Value::Object(pairs) => {
            out.write_char('{')?;
            for (i, (k, val)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.write_char(',')?;
                }
                write_json_string(k, out)?;
                out.write_char(':')?;
                write_value(val, out)?;
            }
            out.write_char('}')
        }
    }
}

/// String escaping matching serde_json's writer: `"`, `\`, and controls below
/// U+0020 escape (short form for `\b \f \n \r \t`, `\u00XX` otherwise); `/`,
/// U+007F, and non-ASCII bytes are written raw.
fn write_json_string(s: &str, out: &mut impl fmt::Write) -> fmt::Result {
    out.write_char('"')?;
    for c in s.chars() {
        match c {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\u{8}' => out.write_str("\\b")?,
            '\u{c}' => out.write_str("\\f")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32)?,
            c => out.write_char(c)?,
        }
    }
    out.write_char('"')
}
