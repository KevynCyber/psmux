//! Ported from `ratatui-widgets` 0.3.2 `src/block.rs` (ZDEP-034), reduced
//! to exactly this repo's inventory: `default` only (no `new`/`bordered` --
//! every call site is `Block::default()...`), `.borders`/`.border_type`/
//! `.border_style`/`.title`/`.style`/`.inner`. NOT ported: `.padding`
//! (`Padding` is never constructed anywhere), `.title_top`/`.title_bottom`/
//! `.title_alignment`/`.title_style`/`.title_position` (every call site
//! makes exactly one `.title(...)` call, so this port keeps a single
//! `Option<Line>` instead of upstream's `Vec<(Option<TitlePosition>,
//! Line)>` multi-title/merge machinery), `.shadow`/`.merge_borders`
//! (never called).
//!
//! Title is always rendered on the top border row (upstream's default
//! `TitlePosition::Top`), left-aligned starting just inside the left
//! border -- the one layout every call site in this repo produces.

use crate::buffer::Buffer;
use crate::layout::{Alignment, Position, Rect};
use crate::style::Style;
use crate::text::Line;
use crate::widgets::render_text::{fill_style, render_line};
use crate::widgets::{Borders, BorderType, Widget};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Block<'a> {
    title: Option<Line<'a>>,
    borders: Borders,
    border_style: Style,
    border_type: BorderType,
    style: Style,
}

impl<'a> Block<'a> {
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn borders(mut self, borders: Borders) -> Self {
        self.borders = borders;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn border_type(mut self, border_type: BorderType) -> Self {
        self.border_type = border_type;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn border_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.border_style = style.into();
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn title<T: Into<Line<'a>>>(mut self, title: T) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// The area remaining inside the block's borders (no padding: see the
    /// module doc note on why `Padding` is not ported).
    pub fn inner(&self, area: Rect) -> Rect {
        let mut inner = area;
        if self.borders.has_top() {
            inner.y = inner.y.saturating_add(1);
            inner.height = inner.height.saturating_sub(1);
        }
        if self.borders.has_bottom() {
            inner.height = inner.height.saturating_sub(1);
        }
        if self.borders.has_left() {
            inner.x = inner.x.saturating_add(1);
            inner.width = inner.width.saturating_sub(1);
        }
        if self.borders.has_right() {
            inner.width = inner.width.saturating_sub(1);
        }
        inner
    }

    fn render_borders(&self, area: Rect, buf: &mut Buffer) {
        let set = self.border_type.to_border_set();
        let top = self.borders.has_top();
        let bottom = self.borders.has_bottom() && area.height > 1;
        let left = self.borders.has_left();
        let right = self.borders.has_right() && area.width > 1;
        let right_x = area.x + area.width - 1;
        let bottom_y = area.y + area.height - 1;

        if top {
            for x in area.x..area.x + area.width {
                buf[Position::new(x, area.y)].set_symbol(set.horizontal).set_style(self.border_style);
            }
        }
        if bottom {
            for x in area.x..area.x + area.width {
                buf[Position::new(x, bottom_y)].set_symbol(set.horizontal).set_style(self.border_style);
            }
        }
        if left {
            for y in area.y..area.y + area.height {
                buf[Position::new(area.x, y)].set_symbol(set.vertical).set_style(self.border_style);
            }
        }
        if right {
            for y in area.y..area.y + area.height {
                buf[Position::new(right_x, y)].set_symbol(set.vertical).set_style(self.border_style);
            }
        }
        if top && left {
            buf[Position::new(area.x, area.y)].set_symbol(set.top_left).set_style(self.border_style);
        }
        if top && right {
            buf[Position::new(right_x, area.y)].set_symbol(set.top_right).set_style(self.border_style);
        }
        if bottom && left {
            buf[Position::new(area.x, bottom_y)].set_symbol(set.bottom_left).set_style(self.border_style);
        }
        if bottom && right {
            buf[Position::new(right_x, bottom_y)].set_symbol(set.bottom_right).set_style(self.border_style);
        }
    }

    fn render_title(&self, area: Rect, buf: &mut Buffer) {
        let Some(title) = &self.title else { return };
        let left = u16::from(self.borders.has_left());
        let right = u16::from(self.borders.has_right());
        if area.width <= left + right {
            return;
        }
        let title_area = Rect::new(area.x + left, area.y, area.width - left - right, 1);
        render_line(title, title_area, buf, 0, Alignment::Left, 0);
    }
}

impl<'a> Widget for &Block<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        fill_style(area, buf, self.style);
        self.render_borders(area, buf);
        self.render_title(area, buf);
    }
}

impl<'a> Widget for Block<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    // Covers: ZDEP-034
    #[test]
    fn default_has_no_borders_or_title() {
        let block = Block::default();
        assert_eq!(block.inner(Rect::new(0, 0, 5, 5)), Rect::new(0, 0, 5, 5));
    }

    // Covers: ZDEP-034
    #[test]
    fn inner_shrinks_by_all_four_borders() {
        let block = Block::default().borders(Borders::ALL);
        assert_eq!(block.inner(Rect::new(0, 0, 5, 5)), Rect::new(1, 1, 3, 3));
    }

    // Covers: ZDEP-034
    #[test]
    fn render_draws_plain_corners_and_edges() {
        let mut b = buf(4, 3);
        Block::default().borders(Borders::ALL).render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), "\u{250C}");
        assert_eq!(b[(3, 0)].symbol(), "\u{2510}");
        assert_eq!(b[(0, 2)].symbol(), "\u{2514}");
        assert_eq!(b[(3, 2)].symbol(), "\u{2518}");
        assert_eq!(b[(1, 0)].symbol(), "\u{2500}");
        assert_eq!(b[(0, 1)].symbol(), "\u{2502}");
    }

    // Covers: ZDEP-034
    #[test]
    fn render_rounded_uses_rounded_corners() {
        let mut b = buf(3, 3);
        Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), "\u{256D}");
    }

    // Covers: ZDEP-034
    #[test]
    fn render_title_sits_on_top_border_after_left_corner() {
        let mut b = buf(10, 3);
        Block::default().borders(Borders::ALL).title("Hi").render(b.area, &mut b);
        assert_eq!(b[(1, 0)].symbol(), "H");
        assert_eq!(b[(2, 0)].symbol(), "i");
    }

    // Covers: ZDEP-034
    #[test]
    fn border_style_paints_border_cells() {
        let mut b = buf(3, 3);
        Block::default().borders(Borders::ALL).border_style(Color::Red).render(b.area, &mut b);
        assert_eq!(b[(0, 0)].fg, Color::Red);
        assert_eq!(b[(1, 0)].fg, Color::Red);
    }

    // Covers: ZDEP-034
    #[test]
    fn style_paints_whole_area_background() {
        let mut b = buf(3, 3);
        Block::default().style(Style::default().bg(Color::Blue)).render(b.area, &mut b);
        assert_eq!(b[(1, 1)].bg, Color::Blue);
    }
}
