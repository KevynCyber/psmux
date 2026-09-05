//! Zero-dependency JSON: `Value`, a parser, and a compact writer replacing
//! `serde_json::Value`/`parse`/`to_string` (ZDEP-014), plus `ToJson`/
//! `FromJson` replacing serde derives for hand-written impls (ZDEP-015).
//! See `docs/features/zero-deps.md` ZDEP-014..016 for the full contract.

mod error;
mod number;
mod parser;
mod traits;
mod value;

pub use error::Error;
pub use number::Number;
pub use parser::parse;
pub use traits::{from_str, to_string, FromJson, ToJson};
pub use value::Value;
