//! Direct-algorithm replacement for `ratatui-core` 0.1.2's `kasuari`
//! cassowary-style constraint solver (ZDEP-028): the plan explicitly scopes
//! in only `Layout` + `Direction` + `Constraint::{Length, Min,
//! Percentage}`, so a full solver port is not required -- see
//! `layout/mod.rs`. Not a line-for-line port of upstream `layout.rs`;
//! upstream's own inline tests assume the full solver (`Ratio`/`Fill`/
//! `Flex`) and don't carry over. The tests below are original, covering
//! this crate's reduced semantics.
//!
//! Algorithm: `Length`/`Percentage` get their exact size (percentage
//! truncates toward zero); any space left over after those is distributed
//! across `Min` segments (first-come gets the remainder unit when it
//! doesn't divide evenly), so `Min` behaves as "at least this much, growing
//! to fill". If the constraints overflow the available space, each
//! segment's length is clamped so the cumulative offset never exceeds the
//! area -- segments run out of room in order rather than panicking or
//! producing out-of-bounds rects.

use super::{Constraint, Direction, Rect};

pub struct Layout {
    direction: Direction,
    constraints: Vec<Constraint>,
}

impl Default for Layout {
    fn default() -> Self {
        Self { direction: Direction::default(), constraints: Vec::new() }
    }
}

impl Layout {
    pub fn new<I: IntoIterator<Item = Constraint>>(direction: Direction, constraints: I) -> Self {
        Self { direction, constraints: constraints.into_iter().collect() }
    }

    /// Builder-style setters mirroring upstream's `Layout::default().direction(...)`
    /// call-site shape (ZDEP-040): every `.split(...)` call site in this repo chains
    /// off `Layout::default()` rather than `Layout::new(direction, constraints)`.
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// See [`Layout::direction`].
    pub fn constraints<I: IntoIterator<Item = Constraint>>(mut self, constraints: I) -> Self {
        self.constraints = constraints.into_iter().collect();
        self
    }

    pub fn split(&self, area: Rect) -> Vec<Rect> {
        let total: u32 = match self.direction {
            Direction::Horizontal => area.width as u32,
            Direction::Vertical => area.height as u32,
        };

        let mut lengths: Vec<u32> = Vec::with_capacity(self.constraints.len());
        let mut is_min: Vec<bool> = Vec::with_capacity(self.constraints.len());
        for constraint in &self.constraints {
            match *constraint {
                Constraint::Length(n) => {
                    lengths.push(n as u32);
                    is_min.push(false);
                }
                Constraint::Percentage(p) => {
                    lengths.push(total * p as u32 / 100);
                    is_min.push(false);
                }
                Constraint::Min(n) => {
                    lengths.push(n as u32);
                    is_min.push(true);
                }
            }
        }

        let sum: u32 = lengths.iter().sum();
        if sum < total {
            let extra = total - sum;
            let min_count = is_min.iter().filter(|&&m| m).count() as u32;
            if min_count > 0 {
                let share = extra / min_count;
                let mut remainder = extra % min_count;
                for (len, min) in lengths.iter_mut().zip(is_min.iter()) {
                    if *min {
                        *len += share;
                        if remainder > 0 {
                            *len += 1;
                            remainder -= 1;
                        }
                    }
                }
            }
        }

        let mut rects = Vec::with_capacity(lengths.len());
        let mut offset: u32 = 0;
        for len in lengths {
            let start = offset.min(total) as u16;
            let end = (offset + len).min(total) as u16;
            let size = end - start;
            rects.push(match self.direction {
                Direction::Horizontal => {
                    Rect::new(area.x.saturating_add(start), area.y, size, area.height)
                }
                Direction::Vertical => {
                    Rect::new(area.x, area.y.saturating_add(start), area.width, size)
                }
            });
            offset += len;
        }
        rects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-040
    #[test]
    fn default_direction_and_constraints_builder_matches_new() {
        let area = Rect::new(0, 0, 30, 10);
        let built = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(10), Constraint::Length(20)])
            .split(area);
        let via_new = Layout::new(Direction::Horizontal, [
            Constraint::Length(10),
            Constraint::Length(20),
        ]).split(area);
        assert_eq!(built, via_new);
    }

    // Covers: ZDEP-040
    #[test]
    fn default_direction_is_vertical() {
        let area = Rect::new(0, 0, 4, 8);
        let vertical = Layout::default().constraints([Constraint::Min(0)]).split(area);
        let explicit = Layout::new(Direction::Vertical, [Constraint::Min(0)]).split(area);
        assert_eq!(vertical, explicit);
    }

    // Covers: ZDEP-040
    #[test]
    fn default_with_no_constraints_splits_into_nothing() {
        assert_eq!(Layout::default().split(Rect::new(0, 0, 1, 1)), Vec::<Rect>::new());
    }

    // Covers: ZDEP-028
    #[test]
    fn length_only_splits_horizontally() {
        let area = Rect::new(0, 0, 30, 10);
        let layout = Layout::new(Direction::Horizontal, [
            Constraint::Length(10),
            Constraint::Length(20),
        ]);
        assert_eq!(
            layout.split(area),
            vec![Rect::new(0, 0, 10, 10), Rect::new(10, 0, 20, 10)]
        );
    }

    // Covers: ZDEP-028
    #[test]
    fn percentage_splits_vertically() {
        let area = Rect::new(0, 0, 10, 20);
        let layout = Layout::new(Direction::Vertical, [
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]);
        assert_eq!(
            layout.split(area),
            vec![Rect::new(0, 0, 10, 10), Rect::new(0, 10, 10, 10)]
        );
    }

    // Covers: ZDEP-028
    #[test]
    fn min_absorbs_remaining_space() {
        let area = Rect::new(0, 0, 30, 10);
        let layout = Layout::new(Direction::Horizontal, [Constraint::Length(10), Constraint::Min(0)]);
        assert_eq!(
            layout.split(area),
            vec![Rect::new(0, 0, 10, 10), Rect::new(10, 0, 20, 10)]
        );
    }

    // Covers: ZDEP-028
    #[test]
    fn min_splits_remainder_giving_first_segment_the_extra_unit() {
        let area = Rect::new(0, 0, 11, 1);
        let layout = Layout::new(Direction::Horizontal, [Constraint::Min(0), Constraint::Min(0)]);
        assert_eq!(
            layout.split(area),
            vec![Rect::new(0, 0, 6, 1), Rect::new(6, 0, 5, 1)]
        );
    }

    // Covers: ZDEP-028
    #[test]
    fn combination_of_length_percentage_and_min() {
        let area = Rect::new(0, 0, 100, 1);
        let layout = Layout::new(Direction::Horizontal, [
            Constraint::Length(10),
            Constraint::Percentage(20),
            Constraint::Min(5),
        ]);
        assert_eq!(
            layout.split(area),
            vec![Rect::new(0, 0, 10, 1), Rect::new(10, 0, 20, 1), Rect::new(30, 0, 70, 1)]
        );
    }

    // Covers: ZDEP-028
    #[test]
    fn overflowing_constraints_clip_instead_of_panicking() {
        let area = Rect::new(0, 0, 10, 1);
        let layout = Layout::new(Direction::Horizontal, [Constraint::Length(8), Constraint::Length(8)]);
        assert_eq!(
            layout.split(area),
            vec![Rect::new(0, 0, 8, 1), Rect::new(8, 0, 2, 1)]
        );
    }
}
