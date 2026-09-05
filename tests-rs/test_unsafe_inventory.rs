// Covers: ZDEP-002
// Requirement: track every `unsafe` usage in the codebase as an explicit,
// reviewed ratchet. A file's unsafe-token count must never grow silently; a
// drop must also be recorded (the allowlist is a two-way ratchet, not just a
// ceiling). Any new file containing `unsafe` must be added to the allowlist
// before it is accepted.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// Edit deliberately; drops must be recorded too (ratchet both ways).
const ALLOWLIST: &[(&str, usize)] = &[
    ("crates/portable-pty-psmux/src/cmdbuilder.rs", 7),
    ("crates/portable-pty-psmux/src/lib.rs", 5),
    ("crates/portable-pty-psmux/src/unix.rs", 15),
    ("crates/portable-pty-psmux/src/win/conpty.rs", 2),
    ("crates/portable-pty-psmux/src/win/mod.rs", 8),
    ("crates/portable-pty-psmux/src/win/procthreadattr.rs", 5),
    ("crates/portable-pty-psmux/src/win/psuedocon.rs", 16),
    ("src/clipboard.rs", 7),
    ("src/main.rs", 8),
    ("src/paths.rs", 1),
    ("src/platform.rs", 59),
    ("src/server/connection.rs", 1),
    ("src/server/mod.rs", 2),
    ("src/session.rs", 1),
    ("src/ssh_input.rs", 26),
    ("src/timefmt.rs", 3),
    ("src/tree.rs", 1),
    ("src/types.rs", 1),
];

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Counts occurrences of the standalone token `unsafe` (ASCII word-boundary
/// on both sides) so identifiers like `unsafe_op` are excluded but
/// `unsafe {` / `unsafe fn` are counted.
fn count_unsafe_tokens(contents: &str) -> usize {
    let bytes = contents.as_bytes();
    let needle = b"unsafe";
    let mut count = 0;
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            let after_idx = i + needle.len();
            let after_ok = after_idx == bytes.len() || !is_ident_char(bytes[after_idx]);
            if before_ok && after_ok {
                count += 1;
                i = after_idx;
                continue;
            }
        }
        i += 1;
    }
    count
}

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().map(|n| n == "target").unwrap_or(false) {
                continue;
            }
            walk_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[test]
fn unsafe_token_counts_match_allowlist() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut roots = vec![manifest_dir.join("src")];
    if let Ok(entries) = fs::read_dir(manifest_dir.join("crates")) {
        for entry in entries.flatten() {
            let p = entry.path().join("src");
            if p.is_dir() {
                roots.push(p);
            }
        }
    }
    roots.push(manifest_dir.join("tests/monitor/src"));

    let mut files = Vec::new();
    for root in &roots {
        walk_rs_files(root, &mut files);
    }

    let mut actual: BTreeMap<String, usize> = BTreeMap::new();
    for file in &files {
        let rel = file
            .strip_prefix(manifest_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let contents = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file.display()));
        let count = count_unsafe_tokens(&contents);
        if count > 0 {
            actual.insert(rel, count);
        }
    }

    let expected: BTreeMap<&str, usize> = ALLOWLIST.iter().copied().collect();

    let mut drift = Vec::new();

    for (path, &actual_count) in &actual {
        match expected.get(path.as_str()) {
            None => drift.push(format!(
                "{}: new file with {} unsafe token(s), not in ALLOWLIST",
                path, actual_count
            )),
            Some(&expected_count) => {
                if actual_count > expected_count {
                    drift.push(format!(
                        "{}: grew from {} to {} unsafe token(s); update ALLOWLIST",
                        path, expected_count, actual_count
                    ));
                } else if actual_count < expected_count {
                    drift.push(format!(
                        "{}: shrank from {} to {} unsafe token(s); record the drop in ALLOWLIST",
                        path, expected_count, actual_count
                    ));
                }
            }
        }
    }

    for (&path, &expected_count) in &expected {
        if !actual.contains_key(path) {
            drift.push(format!(
                "{}: listed with {} unsafe token(s) but file is missing or now has 0",
                path, expected_count
            ));
        }
    }

    assert!(
        drift.is_empty(),
        "unsafe inventory drift ({} issue(s)):\n{}",
        drift.len(),
        drift.join("\n")
    );
}
