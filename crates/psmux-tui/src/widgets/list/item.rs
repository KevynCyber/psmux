//! Ported from `ratatui-widgets` 0.3.2 `src/list/item.rs` (ZDEP-038),
//! reduced to `ListItem::new(Line)` -- the only constructor this repo's
//! inventory found in use (tests/monitor always builds items from a
//! `Line`, never a bare string).

use crate::text::Line;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ListItem<'a> {
    pub(super) content: Line<'a>,
}

impl<'a> ListItem<'a> {
    pub fn new(content: Line<'a>) -> Self {
        Self { content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-038
    #[test]
    fn new_wraps_the_line() {
        let line = Line::from("hi");
        let item = ListItem::new(line.clone());
        assert_eq!(item.content, line);
    }
}
