//! Internal helpers shared by every ported widget (ZDEP-033): writing a
//! `Line`'s spans into one buffer row (clipped/aligned/scrolled) and
//! painting a `Style` over a whole `Rect`. Not part of upstream's own
//! module layout -- upstream spreads this logic across `Buffer::set_line`/
//! `set_stringn`/`set_style` and `reflow`'s `LineTruncator`, none of which
//! this crate's reduced `Buffer` (S8a, ZDEP-031) carries. Kept
//! `pub(crate)` since it is plumbing, not part of the ported public API.

use crate::buffer::Buffer;
use crate::layout::{Alignment, Position, Rect};
use crate::style::Style;
use crate::text::Line;

/// Display width of `line`'s spans, summed per span via `psmux_unicode`
/// (same convention as the rest of this repo's width handling: a
/// zero-width-joiner cluster that happens to straddle a span boundary is
/// not collapsed -- no call site in the reduced surface produces that).
pub(crate) fn line_width(line: &Line<'_>) -> u16 {
    line.spans.iter().map(|s| psmux_unicode::str_width(&s.content) as u16).sum()
}

/// Writes `line` into row `row` of `area` (0-indexed from `area.y`),
/// left/center-aligned per `alignment`, skipping the first `x_scroll`
/// display columns of the line's own content. Cells beyond the line's
/// content are left untouched (callers that need a filled background
/// paint it first with `fill_style`). A no-op if `row` is outside `area`.
pub(crate) fn render_line(
    line: &Line<'_>,
    area: Rect,
    buf: &mut Buffer,
    row: u16,
    alignment: Alignment,
    x_scroll: u16,
) {
    if row >= area.height || area.width == 0 {
        return;
    }
    let y = area.y + row;
    let visible_width = line_width(line).saturating_sub(x_scroll);
    let start_x = match alignment {
        Alignment::Left => 0,
        Alignment::Center => (area.width / 2).saturating_sub(visible_width / 2),
    };

    let mut skip = x_scroll as usize;
    let mut x = start_x;
    for span in &line.spans {
        for ch in span.content.chars() {
            let w = psmux_unicode::char_width(ch).unwrap_or(1);
            if w == 0 {
                continue;
            }
            if skip > 0 {
                skip = skip.saturating_sub(w);
                continue;
            }
            if x >= area.width {
                return;
            }
            let pos = Position::new(area.x + x, y);
            buf[pos].set_char(ch).set_style(line.style).set_style(span.style);
            x += w as u16;
        }
    }
}

/// Paints `style` over every cell in `area` without touching the symbol
/// (mirrors upstream `Buffer::set_style`, not ported onto this crate's
/// reduced `Buffer`).
pub(crate) fn fill_style<S: Into<Style>>(area: Rect, buf: &mut Buffer, style: S) {
    let style = style.into();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            buf[Position::new(x, y)].set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;
    use crate::text::Span;

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    // Covers: ZDEP-033
    #[test]
    fn render_line_left_aligned_writes_symbols() {
        let mut b = buf(10, 1);
        let line = Line::from("hi");
        render_line(&line, b.area, &mut b, 0, Alignment::Left, 0);
        assert_eq!(b[(0, 0)].symbol(), "h");
        assert_eq!(b[(1, 0)].symbol(), "i");
        assert_eq!(b[(2, 0)].symbol(), " ");
    }

    // Covers: ZDEP-033
    #[test]
    fn render_line_center_aligned_offsets() {
        let mut b = buf(6, 1);
        let line = Line::from("ab");
        render_line(&line, b.area, &mut b, 0, Alignment::Center, 0);
        assert_eq!(b[(2, 0)].symbol(), "a");
        assert_eq!(b[(3, 0)].symbol(), "b");
    }

    // Covers: ZDEP-033
    #[test]
    fn render_line_truncates_to_area_width() {
        let mut b = buf(3, 1);
        let line = Line::from("hello");
        render_line(&line, b.area, &mut b, 0, Alignment::Left, 0);
        assert_eq!(b[(0, 0)].symbol(), "h");
        assert_eq!(b[(2, 0)].symbol(), "l");
    }

    // Covers: ZDEP-033
    #[test]
    fn render_line_x_scroll_skips_leading_columns() {
        let mut b = buf(5, 1);
        let line = Line::from("hello");
        render_line(&line, b.area, &mut b, 0, Alignment::Left, 2);
        assert_eq!(b[(0, 0)].symbol(), "l");
        assert_eq!(b[(1, 0)].symbol(), "l");
        assert_eq!(b[(2, 0)].symbol(), "o");
    }

    // Covers: ZDEP-033
    #[test]
    fn render_line_out_of_row_is_noop() {
        let mut b = buf(5, 1);
        let line = Line::from("hi");
        render_line(&line, b.area, &mut b, 5, Alignment::Left, 0);
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    // Covers: ZDEP-033
    #[test]
    fn render_line_preserves_span_style_over_line_style() {
        let mut b = buf(5, 1);
        let line = Line { style: Style::default().fg(Color::Red), spans: vec![Span::styled("x", Color::Blue)], ..Default::default() };
        render_line(&line, b.area, &mut b, 0, Alignment::Left, 0);
        assert_eq!(b[(0, 0)].fg, Color::Blue);
    }

    // Covers: ZDEP-033
    #[test]
    fn fill_style_paints_whole_area_leaving_symbols_alone() {
        let mut b = buf(2, 2);
        b[(0, 0)].set_symbol("x");
        fill_style(b.area, &mut b, Style::default().bg(Color::Green));
        assert_eq!(b[(0, 0)].symbol(), "x");
        assert_eq!(b[(0, 0)].bg, Color::Green);
        assert_eq!(b[(1, 1)].bg, Color::Green);
    }
}
