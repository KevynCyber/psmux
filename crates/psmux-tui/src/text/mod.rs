//! Ported from `ratatui-core` 0.1.2 `src/text/{span,line,text}.rs`
//! (ZDEP-030), reduced to the `From` conversions and constructors this
//! repo's inventory found in use: `Span::{raw, styled}`, and the specific
//! `Line`/`Text` `From` impls listed in `layout/mod.rs`'s sibling doc note.
//! Grapheme iteration, wrapping, and styled-graphemes helpers are not
//! ported (widgets are a later slice).

mod line;
mod span;
mod text;

pub use line::Line;
pub use span::Span;
pub use text::Text;
