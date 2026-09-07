//! Ported from `ratatui-widgets` 0.3.2 `src/barchart.rs` +
//! `src/barchart/*.rs` (ZDEP-039), reduced to this repo's one call site: a
//! single ungrouped, vertical `BarChart` built via `.data(group)` /
//! `.bar_width` / `.bar_gap` / `.value_style` / `.style` / `.block`.
//! Not ported: `grouped`/`horizontal`/`.max`/`.bar_style`/`.label_style`/
//! `group_gap`/`symbols::bar` eighth-cell resolution -- every bar here
//! renders at whole-row resolution (round to the nearest full row) rather
//! than upstream's 1/8-cell `NINE_LEVELS` glyphs, a visual-fidelity gap
//! flagged as a ZDEP-039 risk for S8c if finer resolution is ever needed.

mod bar;
mod bar_group;

pub use bar::Bar;
pub use bar_group::BarGroup;

use crate::buffer::Buffer;
use crate::layout::{Alignment, Position, Rect};
use crate::style::{Color, Style};
use crate::text::{Line, Span};
use crate::widgets::render_text::{fill_style, render_line};
use crate::widgets::{Block, Widget};

const FULL_BLOCK: &str = "\u{2588}";

#[derive(Debug, Clone, PartialEq)]
pub struct BarChart<'a> {
    block: Option<Block<'a>>,
    bar_width: u16,
    bar_gap: u16,
    value_style: Style,
    style: Style,
    data: Vec<BarGroup<'a>>,
}

impl Default for BarChart<'_> {
    fn default() -> Self {
        Self { block: None, bar_width: 1, bar_gap: 1, value_style: Style::default(), style: Style::default(), data: Vec::new() }
    }
}

impl<'a> BarChart<'a> {
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn data(mut self, data: BarGroup<'a>) -> Self {
        if !data.bars.is_empty() {
            self.data.push(data);
        }
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn bar_width(mut self, width: u16) -> Self {
        self.bar_width = width;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn bar_gap(mut self, gap: u16) -> Self {
        self.bar_gap = gap;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn value_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.value_style = style.into();
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

impl<'a> Widget for &BarChart<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        fill_style(area, buf, self.style);
        if let Some(block) = &self.block {
            block.render(area, buf);
        }
        let inner = self.block.as_ref().map_or(area, |b| b.inner(area));
        if inner.width == 0 || inner.height == 0 || self.bar_width == 0 {
            return;
        }

        let bars: Vec<&Bar<'a>> = self.data.iter().flat_map(|g| g.bars.iter()).collect();
        if bars.is_empty() {
            return;
        }
        let max = bars.iter().map(|b| b.value).max().unwrap_or(0).max(1);
        let has_label = bars.iter().any(|b| b.label.is_some());
        let label_rows = u16::from(has_label);
        let bar_area_height = inner.height.saturating_sub(label_rows);
        if bar_area_height == 0 {
            return;
        }

        let right = inner.x + inner.width;
        let mut x = inner.x;
        for bar in bars {
            if x >= right {
                break;
            }
            let width = self.bar_width.min(right - x);
            if width == 0 {
                break;
            }

            let filled_rows = ((bar.value as f64 / max as f64) * f64::from(bar_area_height)).round() as u16;
            let filled_rows = filled_rows.min(bar_area_height);
            let bar_top = inner.y + (bar_area_height - filled_rows);

            for row in 0..filled_rows {
                let y = bar_top + row;
                for col in 0..width {
                    let cell = &mut buf[Position::new(x + col, y)];
                    cell.set_symbol(FULL_BLOCK);
                    cell.fg = bar.style.fg.unwrap_or(Color::Reset);
                    cell.bg = bar.style.bg.unwrap_or(Color::Reset);
                }
            }

            if filled_rows > 0 {
                let text = bar.text_value.clone().unwrap_or_else(|| bar.value.to_string());
                let value_line = Line::from(Span::styled(text, self.value_style));
                render_line(&value_line, Rect::new(x, bar_top, width, 1), buf, 0, Alignment::Center, 0);
            }
            if has_label {
                let label_row = inner.y + bar_area_height;
                if let Some(label) = &bar.label {
                    render_line(label, Rect::new(x, label_row, width, 1), buf, 0, Alignment::Center, 0);
                }
            }

            x += width + self.bar_gap;
        }
    }
}

impl<'a> Widget for BarChart<'a> {
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

    // Covers: ZDEP-039
    #[test]
    fn taller_value_fills_more_rows() {
        let bars = [Bar::default().value(10), Bar::default().value(2)];
        let mut b = buf(4, 4);
        let chart = BarChart::default().data(BarGroup::default().bars(&bars)).bar_width(1).bar_gap(1);
        chart.render(b.area, &mut b);
        // Row 0 is the value-text row for the full-height bar 0; check row 1
        // (below the text) to see the actual fill height difference.
        assert_eq!(b[(0, 1)].symbol(), FULL_BLOCK);
        assert_ne!(b[(2, 1)].symbol(), FULL_BLOCK);
    }

    // Covers: ZDEP-039
    #[test]
    fn bar_gap_spaces_bars_apart() {
        let bars = [Bar::default().value(1), Bar::default().value(1)];
        let mut b = buf(5, 2);
        let chart = BarChart::default().data(BarGroup::default().bars(&bars)).bar_width(1).bar_gap(2);
        chart.render(b.area, &mut b);
        assert_eq!(b[(0, 1)].symbol(), FULL_BLOCK);
        assert_eq!(b[(1, 1)].symbol(), " ");
        assert_eq!(b[(2, 1)].symbol(), " ");
        assert_eq!(b[(3, 1)].symbol(), FULL_BLOCK);
    }

    // Covers: ZDEP-039
    #[test]
    fn value_text_rendered_on_top_row_of_bar() {
        let bars = [Bar::default().value(1).text_value("9")];
        let mut b = buf(3, 1);
        let chart = BarChart::default().data(BarGroup::default().bars(&bars)).bar_width(1);
        chart.render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), "9");
    }

    // Covers: ZDEP-039
    #[test]
    fn label_rendered_below_bars_when_any_bar_has_one() {
        let bars = [Bar::default().value(1).label(Line::from("A"))];
        let mut b = buf(3, 2);
        let chart = BarChart::default().data(BarGroup::default().bars(&bars)).bar_width(1);
        chart.render(b.area, &mut b);
        assert_eq!(b[(0, 1)].symbol(), "A");
    }

    // Covers: ZDEP-039
    #[test]
    fn empty_data_is_a_noop() {
        let mut b = buf(3, 2);
        BarChart::default().render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), " ");
    }
}
