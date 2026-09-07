//! Ported from `ratatui-widgets` 0.3.2 `src/barchart/bar_group.rs`
//! (ZDEP-039), reduced to `default`/`.bars(&[Bar])` -- `new`/`with_label`/
//! `.label` are never called anywhere in this repo's inventory (the one
//! call site is `BarGroup::default().bars(&bars)`, ungrouped/unlabeled).

use super::Bar;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BarGroup<'a> {
    pub(super) bars: Vec<Bar<'a>>,
}

impl<'a> BarGroup<'a> {
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn bars(mut self, bars: &[Bar<'a>]) -> Self {
        self.bars = bars.to_vec();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-039
    #[test]
    fn bars_copies_the_slice() {
        let bars = [Bar::default().value(1), Bar::default().value(2)];
        let group = BarGroup::default().bars(&bars);
        assert_eq!(group.bars.len(), 2);
        assert_eq!(group.bars[1].value, 2);
    }
}
