// Covers: ZDEP-001
// Requirement: the shipped psmux binaries must build with zero third-party
// (registry-sourced) crate dependencies once slice S9 lands; both Cargo.lock
// files (workspace root and tests/monitor) must contain no
// `source = "registry+` lines. Ignored until S9 flips the project so it does
// not block earlier slices' builds, but stays present from S0 so the ratchet
// target is visible in the test tree from day one.

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

#[test]
#[ignore = "enforced from slice S9"]
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

    let monitor_lock = Path::new(manifest_dir).join("tests/monitor/Cargo.lock");
    let monitor_count = count_registry_lines(&monitor_lock);
    assert_eq!(
        monitor_count, 0,
        "{} has {} registry+ dependency line(s); expected 0",
        monitor_lock.display(),
        monitor_count
    );
}
