//! Ported from `ratatui-widgets` 0.3.2 `src/list/state.rs` (ZDEP-038),
//! reduced to `default`/`.select`/`.offset` -- `with_offset`/`with_selected`/
//! `select_next`/`select_previous`/`select_first`/`select_last`/`selected`
//! are never called anywhere in this repo's inventory. `set_offset` is
//! `pub(crate)`, not upstream's own surface: it exists only so
//! `list::mod`'s render can write back the windowed offset, mirroring
//! upstream's own direct field write (`state.offset = first_visible_index`)
//! from inside the same crate.

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ListState {
    offset: usize,
    selected: Option<usize>,
}

impl ListState {
    pub const fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub(super) const fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub(super) const fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-038
    #[test]
    fn default_has_no_selection() {
        let state = ListState::default();
        assert_eq!(state.selected(), None);
        assert_eq!(state.offset(), 0);
    }

    // Covers: ZDEP-038
    #[test]
    fn select_none_resets_offset() {
        let mut state = ListState::default();
        state.set_offset(3);
        state.select(None);
        assert_eq!(state.offset(), 0);
    }

    // Covers: ZDEP-038
    #[test]
    fn select_some_sets_selection() {
        let mut state = ListState::default();
        state.select(Some(2));
        assert_eq!(state.selected(), Some(2));
    }
}
