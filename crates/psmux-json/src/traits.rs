use crate::error::Error;
use crate::number::Number;
use crate::parser::parse;
use crate::value::Value;

/// Converts a Rust value into a `Value` tree (replaces `serde::Serialize`
/// for the hand-written impls added in ZDEP-015).
pub trait ToJson {
    fn to_json(&self) -> Value;
}

/// Converts a `Value` tree back into a Rust value (replaces
/// `serde::Deserialize`). Errors mirror serde's: a required field missing or
/// of the wrong type is `Err`.
pub trait FromJson: Sized {
    fn from_json(v: &Value) -> Result<Self, Error>;
}

/// Serializes `v` to a compact JSON string. Infallible: unlike
/// `serde_json::to_string`, `ToJson` impls cannot fail.
pub fn to_string<T: ToJson>(v: &T) -> String {
    v.to_json().to_string()
}

/// Parses `s` as JSON and converts it to `T`.
pub fn from_str<T: FromJson>(s: &str) -> Result<T, Error> {
    let value = parse(s)?;
    T::from_json(&value)
}

fn expect_number(v: &Value) -> Result<Number, Error> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err(Error::new("expected a JSON number")),
    }
}

macro_rules! impl_unsigned {
    ($t:ty) => {
        impl ToJson for $t {
            fn to_json(&self) -> Value {
                Value::Number(Number::PosInt(*self as u64))
            }
        }
        impl FromJson for $t {
            fn from_json(v: &Value) -> Result<Self, Error> {
                let n = expect_number(v)?;
                let u = n.as_u64().ok_or_else(|| Error::new("expected a non-negative integer"))?;
                <$t>::try_from(u).map_err(|_| Error::new(concat!("integer out of range for ", stringify!($t))))
            }
        }
    };
}

macro_rules! impl_signed {
    ($t:ty) => {
        impl ToJson for $t {
            fn to_json(&self) -> Value {
                if *self >= 0 {
                    Value::Number(Number::PosInt(*self as u64))
                } else {
                    Value::Number(Number::NegInt(*self as i64))
                }
            }
        }
        impl FromJson for $t {
            fn from_json(v: &Value) -> Result<Self, Error> {
                let n = expect_number(v)?;
                let i = n.as_i64().ok_or_else(|| Error::new("expected an integer"))?;
                <$t>::try_from(i).map_err(|_| Error::new(concat!("integer out of range for ", stringify!($t))))
            }
        }
    };
}

impl_unsigned!(u8);
impl_unsigned!(u16);
impl_unsigned!(u32);
impl_unsigned!(u64);
impl_unsigned!(usize);
impl_signed!(i32);
impl_signed!(i64);

impl ToJson for bool {
    fn to_json(&self) -> Value {
        Value::Bool(*self)
    }
}

impl FromJson for bool {
    fn from_json(v: &Value) -> Result<Self, Error> {
        match v {
            Value::Bool(b) => Ok(*b),
            _ => Err(Error::new("expected a JSON bool")),
        }
    }
}

impl ToJson for f64 {
    fn to_json(&self) -> Value {
        Value::Number(Number::Float(*self))
    }
}

impl FromJson for f64 {
    fn from_json(v: &Value) -> Result<Self, Error> {
        expect_number(v)?.as_f64().ok_or_else(|| Error::new("expected a JSON number"))
    }
}

impl ToJson for String {
    fn to_json(&self) -> Value {
        Value::String(self.clone())
    }
}

impl FromJson for String {
    fn from_json(v: &Value) -> Result<Self, Error> {
        match v {
            Value::String(s) => Ok(s.clone()),
            _ => Err(Error::new("expected a JSON string")),
        }
    }
}

impl<T: ToJson> ToJson for Option<T> {
    fn to_json(&self) -> Value {
        match self {
            Some(x) => x.to_json(),
            None => Value::Null,
        }
    }
}

impl<T: FromJson> FromJson for Option<T> {
    fn from_json(v: &Value) -> Result<Self, Error> {
        match v {
            Value::Null => Ok(None),
            other => T::from_json(other).map(Some),
        }
    }
}

impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> Value {
        Value::Array(self.iter().map(ToJson::to_json).collect())
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn from_json(v: &Value) -> Result<Self, Error> {
        match v {
            Value::Array(items) => items.iter().map(T::from_json).collect(),
            _ => Err(Error::new("expected a JSON array")),
        }
    }
}
