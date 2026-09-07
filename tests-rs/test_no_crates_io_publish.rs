// Covers: ZDEP-051
// Requirement: psmux is closed off from accidental crates.io publication now
// that slice S9 flipped the project to zero-third-party-deps -- every
// workspace member manifest must carry `publish = false` in its [package]
// section (derived from the root Cargo.toml's [workspace] members list, so a
// NEW member added later without `publish = false` fails this test), the
// release workflow must never invoke `cargo publish` or reference
// `CARGO_REGISTRY_TOKEN`, and no doc/readme/workflow file may tell users to
// run `cargo install psmux` (the correct forms are `cargo install --git
// https://github.com/psmux/psmux` for psmux itself, and `cargo install
// pstop`/`psnet`/`tmuxpanel`/`omp-manager` for the separate live satellite
// crates, both of which must remain untouched).

use std::fs;
use std::path::{Path, PathBuf};

/// Parse the `[workspace] members = [...]` array out of the root Cargo.toml
/// using plain string handling (no toml crate available in this
/// zero-third-party-dep project). Returns the quoted entries, e.g.
/// `["." , "crates/vt100-psmux", ...]`.
fn parse_workspace_members(root_manifest: &str) -> Vec<String> {
    let start = root_manifest
        .find("members = [")
        .unwrap_or_else(|| panic!("Cargo.toml has no `members = [` line in [workspace]"));
    let after = &root_manifest[start + "members = [".len()..];
    let end = after
        .find(']')
        .unwrap_or_else(|| panic!("Cargo.toml `members = [` array is never closed with `]`"));
    let list = &after[..end];

    list.split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                return None;
            }
            let entry = entry.trim_matches('"');
            Some(entry.to_string())
        })
        .collect()
}

/// Map a workspace member entry ("." or "crates/<name>") to its manifest
/// path relative to the workspace root.
fn member_manifest_path(manifest_dir: &Path, member: &str) -> PathBuf {
    if member == "." {
        manifest_dir.join("Cargo.toml")
    } else {
        manifest_dir.join(member).join("Cargo.toml")
    }
}

/// Return true if `manifest` contains a `publish = false` line inside its
/// `[package]` section (before the next `[section]` header).
fn package_section_has_publish_false(manifest: &str) -> bool {
    let mut in_package_section = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package_section = trimmed == "[package]";
            continue;
        }
        if in_package_section && trimmed == "publish = false" {
            return true;
        }
    }
    false
}

#[test]
fn every_workspace_member_manifest_has_publish_false() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_manifest_path = manifest_dir.join("Cargo.toml");
    let root_manifest = fs::read_to_string(&root_manifest_path).unwrap_or_else(|e| {
        panic!("failed to read {}: {e}", root_manifest_path.display())
    });

    let members = parse_workspace_members(&root_manifest);
    assert!(
        !members.is_empty(),
        "parsed zero workspace members out of {}; parser or manifest is broken",
        root_manifest_path.display()
    );

    for member in &members {
        let path = member_manifest_path(manifest_dir, member);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        assert!(
            package_section_has_publish_false(&contents),
            "{} is missing `publish = false` in its [package] section -- every \
             workspace member must opt out of crates.io publication",
            path.display()
        );
    }
}

#[test]
fn release_workflow_never_publishes_to_crates_io() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow_path = manifest_dir.join(".github/workflows/release.yml");
    let contents = fs::read_to_string(&workflow_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", workflow_path.display()));

    assert!(
        !contents.contains("cargo publish"),
        "{} still contains a `cargo publish` invocation; psmux must not be \
         published to crates.io",
        workflow_path.display()
    );
    assert!(
        !contents.contains("CARGO_REGISTRY_TOKEN"),
        "{} still references CARGO_REGISTRY_TOKEN; psmux must not be \
         published to crates.io",
        workflow_path.display()
    );
}

/// Recursively collect every file under `dir`, skipping nothing -- callers
/// decide what to do with non-UTF8 content.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn no_docs_advertise_cargo_install_psmux() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let banned = "cargo install psmux";

    let mut candidates: Vec<PathBuf> = Vec::new();

    let readme = manifest_dir.join("README.md");
    if readme.is_file() {
        candidates.push(readme);
    }

    for sub in ["docs", ".github"] {
        collect_files(&manifest_dir.join(sub), &mut candidates);
    }

    let mut offenders: Vec<String> = Vec::new();
    for path in &candidates {
        let contents = match fs::read_to_string(path) {
            Ok(c) => c,
            // Skip non-UTF8 files rather than failing on them.
            Err(_) => continue,
        };
        if contents.contains(banned) {
            offenders.push(path.display().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "found literal `{}` in: {} -- use `cargo install --git \
         https://github.com/psmux/psmux` instead (satellite crates pstop/psnet/\
         tmuxpanel/omp-manager are separate and correctly untouched)",
        banned,
        offenders.join(", ")
    );
}
