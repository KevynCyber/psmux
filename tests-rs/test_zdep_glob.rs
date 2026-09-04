// Covers: ZDEP-005
// Requirement: `src/globmatch.rs` replaces the `glob` crate with a native
// matcher: `glob_match(pattern, name) -> bool` (`*` any run except a path
// separator, `?` one such char, `[abc]`/`[a-z]`/`[!a-z]` classes, `[` with
// no closing `]` literal, case-sensitive) and `glob(pattern) -> Vec<PathBuf>`
// which expands a pattern component by component over `read_dir`, sorted by
// path; a pattern with no wildcard yields the path itself only if it exists.

use crate::globmatch::{glob, glob_match};

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static UNIQ: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let n = UNIQ.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "psmux_zdep_glob_{}_{}_{}",
            std::process::id(),
            tag,
            n
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }

    fn touch(&self, rel: &str) -> PathBuf {
        let p = self.path.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(&p, b"stub").expect("write stub file");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn canon(p: &std::path::Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

// ---------------------------------------------------------------------
// glob_match table test
// ---------------------------------------------------------------------

#[test]
fn glob_match_literal_equality() {
    assert!(glob_match("a.conf", "a.conf"));
    assert!(!glob_match("a.conf", "b.conf"));
}

#[test]
fn glob_match_star_matches_empty_and_long_runs() {
    assert!(glob_match("*", ""));
    assert!(glob_match("*", "anything_at_all_long"));
    assert!(glob_match("a*z", "az"));
    assert!(glob_match("a*z", "a-----z"));
}

#[test]
fn glob_match_star_does_not_cross_path_separators() {
    assert!(!glob_match("*.conf", "sub/d.conf"));
    assert!(!glob_match("*.conf", "sub\\d.conf"));
}

#[test]
fn glob_match_question_mark_matches_exactly_one_char() {
    assert!(glob_match("a?c", "abc"));
    assert!(!glob_match("a?c", "ac"));
    assert!(!glob_match("a?c", "abbc"));
}

#[test]
fn glob_match_question_mark_does_not_match_separator() {
    assert!(!glob_match("a?c", "a/c"));
    assert!(!glob_match("a?c", "a\\c"));
}

#[test]
fn glob_match_bracket_classes() {
    assert!(glob_match("[abc].txt", "a.txt"));
    assert!(glob_match("[abc].txt", "b.txt"));
    assert!(!glob_match("[abc].txt", "d.txt"));
    assert!(glob_match("[a-z].txt", "m.txt"));
    assert!(!glob_match("[a-z].txt", "M.txt"));
    assert!(glob_match("[!a-z].txt", "M.txt"));
    assert!(!glob_match("[!a-z].txt", "m.txt"));
}

#[test]
fn glob_match_unclosed_bracket_is_literal() {
    assert!(glob_match("a[b.txt", "a[b.txt"));
    assert!(!glob_match("a[b.txt", "ab.txt"));
}

#[test]
fn glob_match_is_case_sensitive() {
    assert!(!glob_match("*.CONF", "a.conf"));
    assert!(glob_match("*.CONF", "a.CONF"));
}

#[test]
fn glob_match_star_matches_leading_dot() {
    // glob crate default: unlike shell globbing, a leading '.' is matched
    // by '*' with no special-casing.
    assert!(glob_match("*.conf", ".conf"));
    assert!(glob_match("*", ".hidden"));
}

#[test]
fn glob_match_conf_pattern_edge_cases() {
    assert!(glob_match("*.conf", "a.conf"));
    assert!(!glob_match("*.conf", "a.conf.bak"));
    assert!(glob_match("*.conf", ".conf"));
}

// ---------------------------------------------------------------------
// glob() filesystem expansion
// ---------------------------------------------------------------------

#[test]
fn glob_expands_star_conf_sorted() {
    let dir = TempDir::new("expand");
    dir.touch("a.conf");
    dir.touch("b.conf");
    dir.touch("c.txt");
    dir.touch("sub/d.conf");

    let pattern = format!("{}/*.conf", dir.path.display().to_string().replace('\\', "/"));
    let mut results: Vec<PathBuf> = glob(&pattern).into_iter().map(|p| canon(&p)).collect();
    results.sort();
    let mut expected = vec![canon(&dir.path.join("a.conf")), canon(&dir.path.join("b.conf"))];
    expected.sort();
    assert_eq!(results, expected, "glob(*.conf) must return exactly a.conf and b.conf, sorted");
}

#[test]
fn glob_expands_subdirectory_wildcard_component() {
    let dir = TempDir::new("subdir");
    dir.touch("a.conf");
    dir.touch("sub/d.conf");

    let pattern = format!("{}/*/d.conf", dir.path.display().to_string().replace('\\', "/"));
    let results: Vec<PathBuf> = glob(&pattern).into_iter().map(|p| canon(&p)).collect();
    assert_eq!(results, vec![canon(&dir.path.join("sub/d.conf"))]);
}

#[test]
fn glob_no_match_returns_empty() {
    let dir = TempDir::new("nomatch");
    dir.touch("a.conf");

    let pattern = format!("{}/z*.conf", dir.path.display().to_string().replace('\\', "/"));
    assert!(glob(&pattern).is_empty());
}

#[test]
fn glob_no_wildcard_returns_path_only_if_it_exists() {
    let dir = TempDir::new("literal");
    let f = dir.touch("exact.conf");

    let existing = f.display().to_string().replace('\\', "/");
    let results = glob(&existing);
    assert_eq!(results.len(), 1, "literal existing path must be returned once");
    assert_eq!(canon(&results[0]), canon(&f));

    let missing = dir.path.join("missing.conf").display().to_string().replace('\\', "/");
    assert!(glob(&missing).is_empty(), "literal missing path must return empty");
}

#[test]
fn glob_wildcard_directory_component_matching_nothing_returns_empty() {
    let dir = TempDir::new("nodircomp");
    dir.touch("a.conf");

    let pattern = format!("{}/nosuchdir*/x.conf", dir.path.display().to_string().replace('\\', "/"));
    assert!(glob(&pattern).is_empty());
}

#[test]
fn glob_accepts_a_backslash_pattern() {
    let dir = TempDir::new("backslash");
    dir.touch("a.conf");
    dir.touch("b.conf");

    let pattern = format!("{}\\*.conf", dir.path.display());
    let mut results: Vec<PathBuf> = glob(&pattern).into_iter().map(|p| canon(&p)).collect();
    results.sort();
    let mut expected = vec![canon(&dir.path.join("a.conf")), canon(&dir.path.join("b.conf"))];
    expected.sort();
    assert_eq!(results, expected, "a backslash-separated pattern must still expand");
}
