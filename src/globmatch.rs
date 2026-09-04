// ZDEP-005: native glob matcher replacing the `glob` crate.
// `glob_match` supports `*`, `?`, and `[...]` classes; `*`/`?` never cross a
// path separator. `glob` expands a pattern component by component over
// `read_dir`, returning matches sorted by path.

use std::path::{Path, PathBuf};

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Match `name` (one path component, or a whole non-wildcard string) against
/// `pattern`. `*` matches any run of characters except a path separator,
/// `?` matches exactly one non-separator character, and `[...]` matches a
/// character class (`[!...]` negates; an unterminated `[` is literal).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    match_from(&p, 0, &n, 0)
}

fn match_from(p: &[char], mut pi: usize, n: &[char], mut ni: usize) -> bool {
    // Backtracking state for the most recent '*'.
    let mut star_pi: Option<usize> = None;
    let mut star_ni: usize = 0;

    loop {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    star_pi = Some(pi);
                    star_ni = ni;
                    pi += 1;
                    continue;
                }
                '?' => {
                    if ni < n.len() && !is_sep(n[ni]) {
                        pi += 1;
                        ni += 1;
                        continue;
                    }
                }
                '[' => {
                    if let Some((matched, next_pi)) = match_class(p, pi, n.get(ni).copied()) {
                        if matched && ni < n.len() {
                            pi = next_pi;
                            ni += 1;
                            continue;
                        }
                    } else if ni < n.len() && n[ni] == '[' {
                        // Unterminated '[': literal.
                        pi += 1;
                        ni += 1;
                        continue;
                    }
                }
                c => {
                    if ni < n.len() && n[ni] == c {
                        pi += 1;
                        ni += 1;
                        continue;
                    }
                }
            }
        } else if ni == n.len() {
            return true;
        }

        // Mismatch: backtrack to the last '*' if one exists and can still
        // consume a non-separator character.
        if let Some(sp) = star_pi {
            if star_ni < n.len() && !is_sep(n[star_ni]) {
                star_ni += 1;
                ni = star_ni;
                pi = sp + 1;
                continue;
            }
        }
        return false;
    }
}

/// Parse and test a `[...]` class starting at `p[pi] == '['`. Returns
/// `Some((matched, index_after_class))` when a valid class was found, or
/// `None` when there is no closing `]` (caller treats `[` as literal).
fn match_class(p: &[char], pi: usize, ch: Option<char>) -> Option<(bool, usize)> {
    let mut i = pi + 1;
    let negate = p.get(i) == Some(&'!');
    if negate {
        i += 1;
    }
    let start = i;
    // Find the closing ']', allowing a ']' as the first class member.
    if p.get(i) == Some(&']') {
        i += 1;
    }
    while p.get(i).is_some() && p[i] != ']' {
        i += 1;
    }
    if p.get(i) != Some(&']') {
        return None;
    }
    let class = &p[start..i];
    let matched = match ch {
        Some(c) => {
            let mut m = false;
            let mut k = 0;
            while k < class.len() {
                if k + 2 < class.len() && class[k + 1] == '-' {
                    if c >= class[k] && c <= class[k + 2] {
                        m = true;
                    }
                    k += 3;
                } else {
                    if c == class[k] {
                        m = true;
                    }
                    k += 1;
                }
            }
            if negate {
                !m
            } else {
                m
            }
        }
        None => false,
    };
    Some((matched, i + 1))
}

fn has_wildcard(s: &str) -> bool {
    s.contains(['*', '?', '['])
}

/// Expand `pattern` component by component over `read_dir`, returning
/// matches sorted by path. A pattern with no wildcard component yields the
/// path itself only if it exists.
pub fn glob(pattern: &str) -> Vec<PathBuf> {
    let normalized = pattern.replace('\\', "/");
    let components: Vec<&str> = normalized.split('/').collect();

    if !has_wildcard(&normalized) {
        let p = PathBuf::from(pattern);
        return if p.exists() { vec![p] } else { vec![] };
    }

    // Seed with the root: an absolute path keeps its leading separator (and
    // drive prefix on Windows), a relative path starts from ".".
    let mut current: Vec<PathBuf> = vec![PathBuf::new()];
    let mut start = 0;
    if let Some(first) = components.first() {
        if first.is_empty() {
            current = vec![PathBuf::from("/")];
            start = 1;
        } else if first.len() >= 2 && first.as_bytes()[1] == b':' {
            current = vec![PathBuf::from(format!("{first}/"))];
            start = 1;
        }
    }

    for (idx, comp) in components.iter().enumerate().skip(start) {
        if comp.is_empty() {
            continue;
        }
        let mut next: Vec<PathBuf> = Vec::new();
        let is_last = idx == components.len() - 1;
        if has_wildcard(comp) {
            for base in &current {
                let dir: PathBuf = if base.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    base.clone()
                };
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    if glob_match(comp, &name) {
                        next.push(join_component(base, &name));
                    }
                }
            }
        } else {
            for base in &current {
                let candidate = join_component(base, comp);
                if is_last || candidate.is_dir() {
                    next.push(candidate);
                }
            }
        }
        current = next;
        if current.is_empty() {
            break;
        }
    }

    current.retain(|p| p.exists());
    current.sort();
    current
}

fn join_component(base: &Path, comp: &str) -> PathBuf {
    if base.as_os_str().is_empty() {
        PathBuf::from(comp)
    } else {
        base.join(comp)
    }
}
