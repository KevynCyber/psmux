// ZDEP-004: native PATH/PATHEXT walk replacing the `which` crate.
// Semantics mirror the `which` crate's Windows behaviour: a name containing
// a path separator is checked directly; otherwise each PATH entry is
// scanned in order, and within one entry, a name with an explicit extension
// is tried as-is before the PATHEXT extensions are appended in order.

use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Resolve `name` against the process's real `PATH`/`PATHEXT`.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = env::var("PATH").unwrap_or_default();
    let pathext = env::var("PATHEXT").unwrap_or_default();
    which_in(name, &path, &pathext)
}

/// Pure PATH/PATHEXT resolver, taking `path`/`pathext` as explicit strings.
pub fn which_in(name: &str, path: &str, pathext: &str) -> Option<PathBuf> {
    let pathext = if pathext.trim().is_empty() {
        DEFAULT_PATHEXT
    } else {
        pathext
    };
    let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();

    if name.contains('/') || name.contains('\\') {
        return resolve_in_dir(Path::new(name), &exts);
    }

    for dir in path.split(';') {
        if dir.is_empty() {
            continue;
        }
        if let Some(found) = resolve_in_dir(&Path::new(dir).join(name), &exts) {
            return Some(found);
        }
    }
    None
}

/// Try `candidate` as-is (if it already has an extension) before trying
/// each PATHEXT extension appended, in order.
fn resolve_in_dir(candidate: &Path, exts: &[&str]) -> Option<PathBuf> {
    let has_ext = candidate
        .extension()
        .map(|e| exts.iter().any(|ext| ext.trim_start_matches('.').eq_ignore_ascii_case(e.to_str().unwrap_or(""))))
        .unwrap_or(false);

    if has_ext && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    let base = candidate.as_os_str().to_string_lossy().into_owned();
    for ext in exts {
        let with_ext = PathBuf::from(format!("{base}{ext}"));
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }

    // Fall back to the bare candidate (covers names that already carry a
    // non-PATHEXT extension or no extension at all but exist verbatim).
    if candidate.is_file() {
        return Some(candidate.to_path_buf());
    }
    None
}
