use std::fmt;

/// Compile error: an unsupported or malformed pattern, or a documented
/// limit exceeded (nesting depth, repeat count, program size).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub(crate) String);

impl Error {
    pub(crate) fn new(msg: impl Into<String>) -> Self {
        Error(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}
