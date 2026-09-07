// Covers: ZDEP-001, ZDEP-048
// Requirement: the shipped psmux binaries must build with zero third-party
// (registry-sourced) crate dependencies now that slice S9 has flipped the
// project; both Cargo.lock files (workspace root and tests/monitor) must
// contain no `source = "registry+` lines, and no `[[package]]` entry in
// either lockfile may carry ANY `source = ` line at all -- a pure
// path/workspace dependency has no `source` key, so any `source = ` line
// (registry+, git+, or otherwise) marks a third-party dependency that
// slipped past the registry+-only check.

use std::fs;
use std::path::Path;

fn count_registry_lines(path: &Path) -> usize {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    contents
        .lines()
        .filter(|line| line.contains("source = \"registry+"))
        .count()
}

/// Scan a Cargo.lock's text for `[[package]]` blocks and return the
/// `name = "..."` of every block that also contains a `source = ` line.
/// A pure path/workspace dependency has no `source` key at all, so any
/// `source = ` line inside a package block marks a third-party dependency.
fn packages_with_source_line(contents: &str) -> Vec<String> {
    let mut offenders = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_has_source = false;

    fn flush(offenders: &mut Vec<String>, name: &Option<String>, has_source: bool) {
        if has_source {
            if let Some(n) = name {
                offenders.push(n.clone());
            }
        }
    }

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            flush(&mut offenders, &current_name, current_has_source);
            current_name = None;
            current_has_source = false;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name = \"") {
            if current_name.is_none() {
                if let Some(end) = rest.find('"') {
                    current_name = Some(rest[..end].to_string());
                }
            }
            continue;
        }
        if trimmed.starts_with("source = ") {
            current_has_source = true;
        }
    }
    flush(&mut offenders, &current_name, current_has_source);

    offenders
}

fn assert_no_source_lines(path: &Path) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let offenders = packages_with_source_line(&contents);
    assert!(
        offenders.is_empty(),
        "{} has {} package(s) with a `source = ` line (expected none, all deps \
         must be path/workspace-sourced): {}",
        path.display(),
        offenders.len(),
        offenders.join(", ")
    );
}

#[test]
fn zero_third_party_deps_in_both_lockfiles() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let root_lock = Path::new(manifest_dir).join("Cargo.lock");
    let root_count = count_registry_lines(&root_lock);
    assert_eq!(
        root_count, 0,
        "{} has {} registry+ dependency line(s); expected 0",
        root_lock.display(),
        root_count
    );
    assert_no_source_lines(&root_lock);

    let monitor_lock = Path::new(manifest_dir).join("tests/monitor/Cargo.lock");
    let monitor_count = count_registry_lines(&monitor_lock);
    assert_eq!(
        monitor_count, 0,
        "{} has {} registry+ dependency line(s); expected 0",
        monitor_lock.display(),
        monitor_count
    );
    assert_no_source_lines(&monitor_lock);
}
