// Covers: ZDEP-004
// Requirement: `src/which.rs` replaces the `which` crate with a native
// PATH/PATHEXT walk, matching the `which` crate's Windows semantics: a name
// containing a path separator is checked directly (as-is when it already has
// an extension, then with each PATHEXT extension appended); otherwise each
// PATH entry is scanned in order and, within one entry, a name that already
// carries an extension is tried as-is before the PATHEXT extensions are
// appended in PATHEXT order; extension comparison is case-insensitive;
// empty PATH entries are skipped; an empty PATHEXT falls back to
// .COM;.EXE;.BAT;.CMD; the current directory is not searched.

use crate::which::{which, which_in};

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static UNIQ: AtomicU64 = AtomicU64::new(0);

/// A scratch directory under the OS temp dir, cleaned up on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let n = UNIQ.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "psmux_zdep_which_{}_{}_{}",
            std::process::id(),
            tag,
            n
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }

    fn touch(&self, name: &str) -> PathBuf {
        let p = self.path.join(name);
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

/// which("cmd") must resolve to the real system cmd.exe on PATH.
#[test]
fn which_finds_cmd_on_real_path() {
    let found = which("cmd").expect("cmd.exe must be found via real PATH/PATHEXT");
    let s = found.to_string_lossy().to_lowercase();
    assert!(s.ends_with("cmd.exe"), "which(\"cmd\") must resolve to cmd.exe, got {:?}", found);
}

/// which("pwsh").or(which("powershell")) must resolve to a real shell.
#[test]
fn which_finds_a_powershell_variant_on_real_path() {
    let found = which("pwsh").or_else(|| which("powershell"));
    assert!(found.is_some(), "neither pwsh nor powershell found on real PATH");
}

/// PATH order beats PATHEXT order: dir A (tool.cmd only) precedes dir B
/// (tool.exe) on PATH, so which_in must return A's tool.cmd even though
/// .EXE sorts before .CMD in PATHEXT.
#[test]
fn path_order_beats_pathext_order_across_dirs() {
    let a = TempDir::new("a");
    let b = TempDir::new("b");
    let cmd_in_a = a.touch("tool.cmd");
    let _exe_in_b = b.touch("tool.exe");

    let path = format!("{};{}", a.path.display(), b.path.display());
    let found = which_in("tool", &path, ".COM;.EXE;.BAT;.CMD").expect("tool must resolve");
    assert_eq!(canon(&found), canon(&cmd_in_a), "PATH order must win over PATHEXT order");
}

/// Within a single directory, PATHEXT order decides: tool.exe beats
/// tool.cmd when both exist in the same dir.
#[test]
fn pathext_order_decides_within_one_dir() {
    let dir = TempDir::new("onedir");
    let _cmd = dir.touch("tool.cmd");
    let exe = dir.touch("tool.exe");

    let found = which_in("tool", &dir.path.display().to_string(), ".COM;.EXE;.BAT;.CMD")
        .expect("tool must resolve");
    assert_eq!(canon(&found), canon(&exe), "PATHEXT order must pick .exe before .cmd");
}

/// An extension-less basename resolves against the appended-extension file.
#[test]
fn extensionless_basename_resolves_via_pathext() {
    let dir = TempDir::new("py3");
    let exe = dir.touch("python3.exe");

    let found = which_in("python3", &dir.path.display().to_string(), ".COM;.EXE;.BAT;.CMD")
        .expect("python3 must resolve via PATHEXT");
    assert_eq!(canon(&found), canon(&exe));
}

/// A name that already carries an extension resolves as-is.
#[test]
fn name_with_explicit_extension_resolves_as_is() {
    let dir = TempDir::new("py3exe");
    let exe = dir.touch("python3.exe");

    let found = which_in("python3.exe", &dir.path.display().to_string(), ".COM;.EXE;.BAT;.CMD")
        .expect("python3.exe must resolve as-is");
    assert_eq!(canon(&found), canon(&exe));
}

/// A file that exists but with a non-PATHEXT extension must NOT resolve
/// when asked for by its extension-less basename.
#[test]
fn non_pathext_extension_file_is_not_found() {
    let dir = TempDir::new("notes");
    dir.touch("notes.txt");

    let found = which_in("notes", &dir.path.display().to_string(), ".COM;.EXE;.BAT;.CMD");
    assert!(found.is_none(), "notes.txt must not satisfy which_in(\"notes\", ...)");
}

/// Empty PATH entries (leading/trailing/double semicolons) are skipped
/// without error.
#[test]
fn empty_path_entries_are_skipped() {
    let dir = TempDir::new("emptyentries");
    let exe = dir.touch("tool.exe");

    let path = format!(";;{};", dir.path.display());
    let found = which_in("tool", &path, ".COM;.EXE;.BAT;.CMD").expect("tool must still resolve");
    assert_eq!(canon(&found), canon(&exe));
}

/// An empty PATHEXT falls back to .COM;.EXE;.BAT;.CMD.
#[test]
fn empty_pathext_falls_back_to_default_list() {
    let dir = TempDir::new("emptypathext");
    let exe = dir.touch("tool.exe");

    let found = which_in("tool", &dir.path.display().to_string(), "").expect("tool must resolve with default PATHEXT");
    assert_eq!(canon(&found), canon(&exe));
}

/// Extension comparison in PATHEXT is case-insensitive: a lower-case ".exe"
/// entry still finds an upper-case TOOL.EXE.
#[test]
fn pathext_comparison_is_case_insensitive() {
    let dir = TempDir::new("caseinsensitive");
    let exe_path = dir.path.join("TOOL.EXE");
    fs::write(&exe_path, b"stub").expect("write stub file");

    let found = which_in("tool", &dir.path.display().to_string(), ".com;.exe;.bat;.cmd")
        .expect("lower-case PATHEXT entry must still match upper-case file extension");
    assert_eq!(canon(&found), canon(&exe_path));
}

/// A name containing a path separator resolves directly, ignoring PATH
/// entirely -- both with and without an explicit extension.
#[test]
fn name_with_separator_resolves_directly_ignoring_path() {
    let dir = TempDir::new("directpath");
    let exe = dir.touch("tool.exe");

    let full_with_ext = exe.display().to_string();
    let found = which_in(&full_with_ext, "C:\\does\\not\\exist", ".COM;.EXE;.BAT;.CMD")
        .expect("explicit path with extension must resolve directly");
    assert_eq!(canon(&found), canon(&exe));

    let full_without_ext = dir.path.join("tool").display().to_string();
    let found2 = which_in(&full_without_ext, "C:\\does\\not\\exist", ".COM;.EXE;.BAT;.CMD")
        .expect("explicit path without extension must resolve via PATHEXT");
    assert_eq!(canon(&found2), canon(&exe));
}

/// A missing name yields None.
#[test]
fn missing_name_yields_none() {
    let dir = TempDir::new("missing");
    let found = which_in("does-not-exist-anywhere", &dir.path.display().to_string(), ".COM;.EXE;.BAT;.CMD");
    assert!(found.is_none());
}

/// The returned path is the joined dir/file path.
#[test]
fn returned_path_is_the_joined_dir_and_file() {
    let dir = TempDir::new("joined");
    let exe = dir.touch("tool.exe");

    let found = which_in("tool", &dir.path.display().to_string(), ".COM;.EXE;.BAT;.CMD")
        .expect("tool must resolve");
    assert_eq!(canon(&found), canon(&exe), "returned path must be dir joined with the resolved file name");
}
