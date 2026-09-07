# zero-deps feature area (continued from zero-deps-2.md)

`docs/features/zero-deps-2.md` is at the markdown-quality-check length cap.
Same plan and scope (`docs/plans/2026-09-04-zero-third-party-deps.md`).

## ZDEP-040..044 Terminal/Frame/TestBackend layer folded into
crates/psmux-tui (S8c, additive half; the flip is NOT done -- see below)

`crates/psmux-tui` gains the last CORE-layer surface needed to render, per
the 2026-09-06 project-wide inventory (S8c plan entry):
- ZDEP-040: `Layout::default()` + `.direction(...)`/`.constraints(...)`
  builder setters (`layout/split.rs`) -- every real `.split(...)` call site
  in this repo chains off `Layout::default()`, not `Layout::new(direction,
  constraints)`, which S8a's port did not yet cover.
- ZDEP-041: `impl From<Size> for Rect` (`layout/rect.rs`), needed by
  `Terminal::resize`/`autoresize` and by `client.rs`'s own `sz.into()` call
  site.
- ZDEP-042: `Buffer::{resize, reset, diff}` (`buffer/buffer.rs`). `diff` is
  a plain cell-by-cell comparison, NOT upstream's `BufferDiff` (no
  multi-width-glyph trailing-cell or VS16 special-casing): this crate's
  `Cell` (ZDEP-031) carries no width field to drive that logic, and no
  widget in the reduced surface blanks a wide glyph's trailing column itself
  (`render_text.rs`, ZDEP-033), so a width-aware diff has nothing extra to
  catch here.
- ZDEP-043: `backend::TestBackend` (`new`, `.buffer()`, plus
  `cursor_visible()`/`cursor_position()` test accessors), reduced from
  upstream: no `scrollback` buffer, no `with_lines`/`assert_buffer*` helpers
  (call sites build their own `Buffer` and `assert_eq!` directly);
  `clear_region` collapses all five `ClearType` variants to a full clear
  since the fullscreen-only `Terminal` (ZDEP-044) only ever requests `All`.
- ZDEP-044: `terminal::{Terminal, Frame, CompletedFrame}` (new
  `terminal/mod.rs` + `terminal/frame.rs`), FULLSCREEN-ONLY: no `Viewport`/
  `TerminalOptions`/`with_options`/inline mode/`try_draw`/`insert_before` --
  nothing in the inventory calls any of those. `Terminal::clear`'s cursor
  save/restore dance is simplified to the fullscreen case (just
  `ClearType::All` + resetting the back buffer). A `prelude` module
  (`src/prelude.rs`) mirrors `ratatui::prelude::*`'s re-export surface
  (restricted to what this crate has) so every glob-import call site listed
  in the S8 plan entry is a one-line `ratatui::` -> `psmux_tui::` swap.

**This is the ADDITIVE half only -- the flip (removing `ratatui` from both
manifests) is NOT done.** `ratatui` stays in the root and `tests/monitor`
manifests. Blocker: flipping the ~130 real call sites requires
`ratatui::` -> `psmux_tui::` import-path edits inside `src/`, `examples/`
(software-engineer-owned, doable), but ALSO inside ~56 `tests-rs/*.rs` files
and all of `tests/monitor/src/*.rs` (both gated to test-engineer by
`agent-write-gate`; verified live via two denied probe `Edit` calls to
`tests-rs/test_zoom_bleed.rs` and `tests/monitor/src/ui.rs`). Root crate
version bumped 3.5.0 -> 3.6.0 (A10: still a behaviour-preserving, additive
slice; 4.0.0 stays reserved for S9's actual removal).

**Risk 1 (Layout::split's `Vec<Rect>` vs upstream's `Rects` newtype)
RE-VERIFIED, no call site affected:** every `.split(...)` result across
`src/`, `tests-rs/`, and `tests/monitor/src/ui.rs` is indexed
(`chunks[0]`/`rows[1]`) or destructured via an `if`/tuple binding, never via
upstream's `let [a, b] = layout.areas(...)` array-pattern convenience.
`client.rs:6162`'s `sz.into()` needed the new `From<Size> for Rect`
(ZDEP-041) above.

**Risk 2 (direct-algorithm solver vs kasuari's real cassowary solve)
DIFFERENTIALLY CHECKED against real `ratatui` 0.30.2/`ratatui-core` 0.1.2,
CONFIRMED DIVERGENT for one real call site:** a temporary standalone probe
crate (outside this repo, discarded after the check per plan A9's
revertible-slice intent) ran both `Layout::split` implementations side by
side for every constraint set found in `src/` + `tests/monitor/src/ui.rs`.
`Length`/`Min`-only combinations, and `Percentage`-only combinations, match
exactly (client.rs's status-bar split, all of `tests/monitor/src/ui.rs`'s
splits). `rendering.rs:576-583`'s `centered_rect` (`[Percentage(50),
Length(h), Percentage(50)]`) DIFFERS: e.g. total height 20, `h=5` --
`ratatui` yields row heights `[5, 5, 10]` (asymmetric between the two equal
`Percentage(50)`s), this crate's direct algorithm yields `[10, 5, 5]`. Both
give the popup's `Length` segment (index 1, the only one `centered_rect`
actually uses for height) the same height, but its Y-OFFSET differs (5 vs
10) -- flipping this call site as currently ported would visibly
mis-position every popup. Root cause: kasuari's real solver distributes an
over-constrained `Percentage`+`Length` mix non-uniformly (not simple
proportional remainder, not simple first-or-last-absorbs -- several
plausible tie-break rules were tested against the probe output and none
matched consistently), consistent with plan assumption A2's own framing
(full solver explicitly not ported). **Must-fix-before-flip for
`rendering.rs:576-583`:** rewrite `centered_rect` to compute the middle
rect's Y-offset directly by arithmetic (`(r.height - clamped_h) / 2`)
instead of relying on `Layout::split`'s `Percentage` handling for an
asymmetric case the reduced solver cannot reproduce.

**Risk 3 (`Text::from(&str)` not splitting on `\n`, from S8a) RE-VERIFIED,
no call site affected:** every `Paragraph::new(...)` call across `src/`,
`examples/`, `tests-rs/`, and `tests/monitor/src/ui.rs` passes either a
`Text::from(Vec<Line>)`, a `Line::from(...)`, a `Span::styled(...)`, or a
single-line `&str`/`String`/`format!(...)` -- none pass a multi-line string
literal directly, confirming `text/text.rs`'s own ZDEP-035 doc note.

Acceptance: `cargo test -p psmux-tui` (119 tests: 98 prior + 21 new, same
one-`mod tests`-per-module convention, `// Covers: ZDEP-04N`); `cargo check
-p psmux-tui`/`--workspace`, default features, `--no-default-features`,
`--features underline-color`, all 0 errors; `cargo check` in the separate
`tests/monitor` workspace unaffected (still resolves the real `ratatui`
crates); `grep -rn ratatui` on both manifests still finds the existing
dependency lines (the flip has not happened).
Tests: inline `#[cfg(test)]` modules under `crates/psmux-tui/src/{layout,
buffer,backend,terminal}/`.
