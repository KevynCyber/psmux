//! Ported from `ratatui-widgets` 0.3.2 `src/gauge.rs` (ZDEP-037), reduced
//! to `default`/`.gauge_style`/`.ratio`/`.label`/`.block` -- `.percent`
//! and `.use_unicode` (eighth-cell resolution) are never called anywhere in
//! this repo's inventory, so this port always rounds the filled width to
//! whole cells (upstream's non-unicode path) and never emits the partial
//! block glyphs.

use crate::buffer::Buffer;
use crate::layout::{Alignment, Position, Rect};
use crate::style::{Color, Style};
use crate::text::{Line, Span};
use crate::widgets::render_text::{fill_style, render_line};
use crate::widgets::{Block, Widget};
use std::borrow::Cow;

const FULL_BLOCK: &str = "\u{2588}";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Gauge<'a> {
    block: Option<Block<'a>>,
    ratio: f64,
    label: Option<Span<'a>>,
    style: Style,
    gauge_style: Style,
}

impl<'a> Gauge<'a> {
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// # Panics
    /// Panics if `ratio` is not between 0 and 1 inclusively (matches
    /// upstream).
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn ratio(mut self, ratio: f64) -> Self {
        assert!((0.0..=1.0).contains(&ratio), "Ratio should be between 0 and 1 inclusively.");
        self.ratio = ratio;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn label<T: Into<Cow<'a, str>>>(mut self, label: T) -> Self {
        self.label = Some(Span::raw(label));
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn gauge_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.gauge_style = style.into();
        self
    }
}

impl<'a> Widget for &Gauge<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        fill_style(area, buf, self.style);
        if let Some(block) = &self.block {
            block.render(area, buf);
        }
        let inner = self.block.as_ref().map_or(area, |b| b.inner(area));
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        fill_style(inner, buf, self.gauge_style);

        let default_label = format!("{}%", (self.ratio * 100.0).round() as i64);
        let label = self.label.clone().unwrap_or_else(|| Span::raw(default_label));
        let label_width = psmux_unicode::str_width(&label.content) as u16;
        let clamped_label_width = inner.width.min(label_width);
        let label_col = inner.x + (inner.width.saturating_sub(clamped_label_width)) / 2;
        let label_row = inner.y + inner.height / 2;

        let filled_width = ((inner.width as f64) * self.ratio).round() as u16;
        let end_x = inner.x + filled_width.min(inner.width);

        for y in inner.y..inner.y + inner.height {
            for x in inner.x..end_x {
                let in_label = y == label_row && x >= label_col && x < label_col + clamped_label_width;
                let cell = &mut buf[Position::new(x, y)];
                if in_label {
                    cell.set_symbol(" ");
                    cell.fg = self.gauge_style.bg.unwrap_or(Color::Reset);
                    cell.bg = self.gauge_style.fg.unwrap_or(Color::Reset);
                } else {
                    cell.set_symbol(FULL_BLOCK);
                    cell.fg = self.gauge_style.fg.unwrap_or(Color::Reset);
                    cell.bg = self.gauge_style.bg.unwrap_or(Color::Reset);
                }
            }
        }

        let label_line = Line::from(label);
        let label_area = Rect::new(label_col, label_row, clamped_label_width, 1);
        render_line(&label_line, label_area, buf, 0, Alignment::Left, 0);
    }
}

impl<'a> Widget for Gauge<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    // Covers: ZDEP-037
    #[test]
    #[should_panic(expected = "Ratio should be between 0 and 1 inclusively.")]
    fn ratio_out_of_range_panics() {
        Gauge::default().ratio(1.5);
    }

    // Covers: ZDEP-037
    #[test]
    fn ratio_zero_fills_nothing() {
        let mut b = buf(10, 1);
        Gauge::default().ratio(0.0).gauge_style(Color::Green).render(b.area, &mut b);
        assert_ne!(b[(0, 0)].symbol(), FULL_BLOCK);
    }

    // Covers: ZDEP-037
    #[test]
    fn ratio_one_fills_the_whole_width() {
        let mut b = buf(10, 1);
        Gauge::default().ratio(1.0).gauge_style(Color::Green).label("").render(b.area, &mut b);
        assert_eq!(b[(9, 0)].symbol(), FULL_BLOCK);
        assert_eq!(b[(9, 0)].fg, Color::Green);
    }

    // Covers: ZDEP-037
    #[test]
    fn default_label_is_the_rounded_percentage() {
        let mut b = buf(10, 1);
        Gauge::default().ratio(0.5).render(b.area, &mut b);
        // "50%" is 3 cells wide, centered in a 10-wide inner area: col
        // offset = (10 - 3) / 2 = 3.
        assert_eq!(b[(3, 0)].symbol(), "5");
        assert_eq!(b[(4, 0)].symbol(), "0");
        assert_eq!(b[(5, 0)].symbol(), "%");
    }

    // Covers: ZDEP-037
    #[test]
    fn custom_label_overrides_percentage() {
        let mut b = buf(10, 1);
        Gauge::default().ratio(0.5).label("hi").render(b.area, &mut b);
        assert_eq!(b[(4, 0)].symbol(), "h");
        assert_eq!(b[(5, 0)].symbol(), "i");
    }
}
