//! `$`-replacement template expansion (first match only; caller slices the
//! surrounding text). Rules: `$$` -> `$`; `$name` takes the longest
//! `[0-9A-Za-z_]+` run (all-digits -> numeric group, else a named-group
//! lookup); `${name}` takes everything up to the next `}` verbatim as the
//! lookup key; a nonexistent/unmatched group is empty; `$` followed by
//! anything else (or end of template) is a literal `$`, and the following
//! character (if any) is then processed normally.

use crate::Captures;
use std::collections::HashMap;

pub(crate) fn expand_replacement(
    caps: &Captures,
    text: &str,
    names: &HashMap<String, usize>,
    template: &str,
) -> String {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        i += 1; // consume '$'
        if i >= chars.len() {
            out.push('$');
            break;
        }
        match chars[i] {
            '$' => {
                out.push('$');
                i += 1;
            }
            '{' => {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                if j >= chars.len() {
                    // Unterminated `${`: literal `$`, then reprocess from `{`.
                    out.push('$');
                } else {
                    let name: String = chars[start..j].iter().collect();
                    out.push_str(&resolve_group(caps, text, names, &name));
                    i = j + 1;
                    continue;
                }
            }
            c if c.is_ascii_alphanumeric() || c == '_' => {
                let start = i;
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let name: String = chars[start..j].iter().collect();
                out.push_str(&resolve_group(caps, text, names, &name));
                i = j;
                continue;
            }
            _ => {
                out.push('$');
                // Don't advance `i`: the offending char is reprocessed as
                // plain text on the next loop iteration.
            }
        }
    }
    out
}

fn resolve_group(caps: &Captures, text: &str, names: &HashMap<String, usize>, name: &str) -> String {
    let idx = if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
        name.parse::<usize>().ok()
    } else {
        names.get(name).copied()
    };
    match idx.and_then(|i| caps.get(i)) {
        Some((s, e)) => text[s..e].to_string(),
        None => String::new(),
    }
}
