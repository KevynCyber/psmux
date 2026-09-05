//! Shared field-access helpers for the hand-written `FromJson` impls added by
//! ZDEP-015 (replacing `#[serde(default = "...")]`/required-field semantics).
//! Kept here rather than duplicated across layout.rs/util.rs/client.rs.

use psmux_json::{Error, FromJson, Value};

/// A required field: `Err` when absent (matches serde's default -- no
/// `#[serde(default)]` attribute).
pub(crate) fn req<T: FromJson>(v: &Value, key: &str) -> Result<T, Error> {
    match v.get(key) {
        Some(x) => T::from_json(x),
        None => Err(Error::new(format!("missing required field \"{key}\""))),
    }
}

/// An `Option<T>` field: absent or JSON `null` -> `None` (matches serde's
/// built-in `Option` handling, with or without `#[serde(default)]`).
pub(crate) fn opt<T: FromJson>(v: &Value, key: &str) -> Result<Option<T>, Error> {
    match v.get(key) {
        Some(x) => Option::<T>::from_json(x),
        None => Ok(None),
    }
}

/// A `#[serde(default)]` field: absent -> `T::default()`.
pub(crate) fn def<T: FromJson + Default>(v: &Value, key: &str) -> Result<T, Error> {
    match v.get(key) {
        Some(x) => T::from_json(x),
        None => Ok(T::default()),
    }
}

/// A `#[serde(default = "f")]` field: absent -> `f()`.
pub(crate) fn def_fn<T: FromJson>(v: &Value, key: &str, f: impl FnOnce() -> T) -> Result<T, Error> {
    match v.get(key) {
        Some(x) => T::from_json(x),
        None => Ok(f()),
    }
}
