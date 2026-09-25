// Bare "%<id>" w/o dot is a pane in the window slot (tmux parity); digit-only stays a window index; dotted forms split on the last '.' as before.
pub(crate) fn split_target_window_pane(rest: &str) -> (Option<&str>, Option<&str>) {
    if rest.starts_with('%') && !rest.contains('.') {
        return (None, Some(rest));
    }
    match rest.rfind('.').map(|d| (&rest[..d], &rest[d + 1..])) {
        Some((w, p))
            if !p.is_empty()
                && (p.starts_with('%')
                    || p.chars().all(|c| c.is_ascii_digit())
                    || crate::is_relative_pane(p)) =>
        {
            (Some(w), Some(p))
        }
        _ => (Some(rest), None),
    }
}
