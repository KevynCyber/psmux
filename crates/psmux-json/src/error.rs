use std::fmt;

/// A parse or conversion error. Carries a plain message only -- callers that
/// need machine-readable error kinds are out of scope for this crate (ZDEP-014
/// only requires `Display` + `std::error::Error`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    msg: String,
}

impl Error {
    /// Constructs an error with a plain message. Public so downstream
    /// `FromJson` impls (e.g. rejecting an unknown enum tag) can build their
    /// own errors without a `From` bridge.
    pub fn new(msg: impl Into<String>) -> Self {
        Error { msg: msg.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for Error {}
