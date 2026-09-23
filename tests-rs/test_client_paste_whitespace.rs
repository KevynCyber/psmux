// Covers: PSMUX-PASTE-WS-001
#[cfg(windows)]
#[test]
fn blank_row_drag_extracts_whitespace_only_text() {
    // A drag over blank prompt rows must still extract exactly the
    // whitespace present -- this is the raw extraction step, separate
    // from whether that text is worth copying to the clipboard.
    let layout = make_leaf(0, &["prompt>   ", "          ", "          ", "          "]);
    let text = extract_selection_text(&layout, 10, 4, (2, 1), (7, 3), false, None, "off", "");
    assert_eq!(text, "\n\n");
}

// Covers: PSMUX-PASTE-WS-001
#[cfg(windows)]
#[test]
fn whitespace_only_selection_is_not_worth_copying() {
    // Root cause: an accidental blank-row mouse drag copies newline-only
    // text to the clipboard, and the next paste drops N blank lines into
    // the terminal (e.g. Claude Code). A selection containing no visible
    // character must be rejected before it reaches the clipboard.
    assert!(!selection_worth_copying(""));
    assert!(!selection_worth_copying("\n\n\n"));
    assert!(!selection_worth_copying("   \n \t\n  "));
    assert!(!selection_worth_copying("\r\n\r\n"));
}

// Covers: PSMUX-PASTE-WS-001
#[cfg(windows)]
#[test]
fn selection_with_any_visible_char_is_still_copied() {
    // A selection with at least one non-whitespace character must still
    // be copied, even if surrounded by blank lines.
    assert!(selection_worth_copying("a"));
    assert!(selection_worth_copying("\n\n  x\n"));
    assert!(selection_worth_copying("prompt>\n\n"));
}

// Covers: PSMUX-PASTE-WS-001
#[cfg(windows)]
#[test]
fn blank_row_drag_is_not_copied_to_clipboard() {
    // End-to-end: extracting a blank-row drag and checking it against
    // selection_worth_copying must reject the copy; a drag that includes
    // the visible "prompt>" row must still be accepted.
    let layout = make_leaf(0, &["prompt>   ", "          ", "          ", "          "]);

    let blank_text = extract_selection_text(&layout, 10, 4, (0, 1), (9, 3), false, None, "off", "");
    assert!(!selection_worth_copying(&blank_text));

    let visible_text = extract_selection_text(&layout, 10, 4, (0, 0), (9, 2), false, None, "off", "");
    assert!(selection_worth_copying(&visible_text));
}
