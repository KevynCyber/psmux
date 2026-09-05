#![cfg(windows)]
// Covers: ZDEP-023, ZDEP-024
// Requirement: crates/portable-pty-psmux folds into crate::pty (ConPTY-only,
// zero third-party crates): CreatePseudoConsole/ResizePseudoConsole/
// ClosePseudoConsole are resolved via extern "system" decls plus
// GetModuleHandleW/LoadLibraryW + GetProcAddress, OnceLock replaces
// lazy_static+shared_library, an in-tree HANDLE wrapper replaces
// filedescriptor, and std::io::Error replaces anyhow so
// crate::pty::Error = std::io::Error everywhere the old crate returned
// anyhow::Error. The Windows-only cmdbuilder quoting (append_quoted) and the
// HKLM/HKCU registry environment merge (registry_environment) are ported
// verbatim from crates/portable-pty-psmux/src/cmdbuilder.rs. The unix/serial
// halves of the old crate, and its 12 now-unused manifest dependencies, are
// deleted; crates/portable-pty-psmux is removed as a workspace member and
// as a release-publish step.

use crate::pty::{
    append_quoted, conpty_base_flags, probe_conpty, registry_environment, CommandBuilder, Child,
    ChildKiller, Error, MasterPty, PtySize, PtySystem, SlavePty, native_pty_system,
    PSEUDOCONSOLE_PASSTHROUGH_MODE, PSEUDOCONSOLE_RESIZE_QUIRK, PSEUDOCONSOLE_WIN32_INPUT_MODE,
};
use crate::pty::passthrough_supported;
use std::io::ErrorKind;
use std::os::windows::ffi::OsStringExt;

/// passthrough_supported is a pure predicate: an explicit "1" or
/// case-insensitive "true" env override always disables passthrough
/// regardless of build number; otherwise the build-number threshold
/// (Windows 11 22H2 = 22621) governs.
#[test]
fn passthrough_supported_predicate_table() {
    let cases: &[(u32, Option<&str>, bool)] = &[
        (22621, None, true),
        (22620, None, false),
        (26100, None, true),
        (22621, Some("1"), false),
        (22621, Some("TRUE"), false),
        (22621, Some("0"), true),
        (0, None, false),
    ];
    for &(build, env, expected) in cases {
        assert_eq!(
            passthrough_supported(build, env),
            expected,
            "build={build} env={env:?} expected={expected}"
        );
    }
}

/// The base ConPTY flag set is RESIZE_QUIRK | WIN32_INPUT_MODE == 0x6.
/// INHERIT_CURSOR (0x1) must never be part of it (it makes conhost block
/// startup waiting for an unanswered ESC[6n reply). PASSTHROUGH_MODE (0x8)
/// is OR'ed on separately and must not be part of the base flags either.
#[test]
fn conpty_base_flags_value_and_constants() {
    assert_eq!(PSEUDOCONSOLE_RESIZE_QUIRK, 0x2);
    assert_eq!(PSEUDOCONSOLE_WIN32_INPUT_MODE, 0x4);
    assert_eq!(PSEUDOCONSOLE_PASSTHROUGH_MODE, 0x8);
    assert_eq!(conpty_base_flags(), 0x6);
    assert_eq!(
        conpty_base_flags() & 0x1,
        0,
        "INHERIT_CURSOR must not be part of the base flags"
    );
    assert_eq!(
        conpty_base_flags() & PSEUDOCONSOLE_PASSTHROUGH_MODE,
        0,
        "PASSTHROUGH_MODE must not be part of the base flags"
    );
}

/// crate::pty::Error is std::io::Error (A7): a plain io::Error coerces into
/// it and its ::kind() is preserved.
#[test]
fn error_type_is_io_error() {
    let _coerces: fn(std::io::Error) -> Error = |e| e;
    let e: Error = std::io::Error::other("x");
    assert_eq!(e.kind(), ErrorKind::Other);
}

/// probe_conpty resolves the three ConPTY entry points from the named
/// module. kernel32.dll always has them.
#[test]
fn probe_conpty_kernel32_ok() {
    assert!(probe_conpty("kernel32.dll").is_ok());
}

/// A module that exists but lacks the ConPTY exports must Err with
/// ErrorKind::Unsupported and a message naming ConPTY -- never panic.
#[test]
fn probe_conpty_missing_symbols_errors_unsupported() {
    let err = probe_conpty("advapi32.dll").expect_err("advapi32.dll has no ConPTY exports");
    assert_eq!(err.kind(), ErrorKind::Unsupported);
    assert!(
        err.to_string().contains("ConPTY"),
        "error message must mention ConPTY, got: {}",
        err
    );
}

/// append_quoted is the ArgvQuote port (portable-pty-psmux cmdbuilder.rs
/// :726-769). Decode the emitted UTF-16 back to a String to check the exact
/// quoting shape for each case.
#[test]
fn append_quoted_matches_argv_quote_table() {
    let cases: &[(&str, &str)] = &[
        ("abc", "abc"),
        ("a b", "\"a b\""),
        ("a\"b", "\"a\\\"b\""),
        ("a\\", "a\\"),
        ("a b\\", "\"a b\\\\\""),
        ("", "\"\""),
        ("a\\\"b", "\"a\\\\\\\"b\""),
        ("a\tb", "\"a\tb\""),
    ];
    for &(input, expected) in cases {
        let mut buf: Vec<u16> = vec![];
        append_quoted(std::ffi::OsStr::new(input), &mut buf);
        let decoded = String::from_utf16(&buf).expect("valid utf16 output");
        assert_eq!(decoded, expected, "input={input:?}");
    }
}

/// cmdline() must Err (not panic, not silently truncate) when an argument
/// contains an interior NUL, since NUL cannot be represented in a
/// NUL-terminated Windows command line.
#[test]
fn cmdline_errors_on_interior_nul_argument() {
    let mut cmd = CommandBuilder::new("cmd.exe");
    let bad_arg = std::ffi::OsString::from_wide(&['a' as u16, 0, 'b' as u16]);
    cmd.arg(&bad_arg);
    cmd.cmdline()
        .expect_err("an argument with an interior NUL must error, not panic or truncate");
}

/// registry_environment merges HKLM Session Manager\Environment then
/// HKCU\Environment (REG_EXPAND_SZ expanded, HKCU Path appended to HKLM
/// Path with ';'). On any real Windows machine this must surface a
/// Path-like key, and any expanded TEMP/TMP value must not retain an
/// unexpanded %...% token.
#[test]
fn registry_environment_has_path_and_expands_temp_tmp() {
    let entries = registry_environment();
    let has_path = entries
        .iter()
        .any(|(k, _)| k.to_string_lossy().eq_ignore_ascii_case("path"));
    assert!(
        has_path,
        "expected a Path-like key among registry_environment() entries, got keys: {:?}",
        entries.iter().map(|(k, _)| k).collect::<Vec<_>>()
    );
    for (k, v) in &entries {
        let key = k.to_string_lossy();
        if key.eq_ignore_ascii_case("temp") || key.eq_ignore_ascii_case("tmp") {
            let val = v.to_string_lossy();
            assert!(
                !val.contains('%'),
                "{} must be expanded, not retain a %...% token: {:?}",
                key,
                val
            );
        }
    }
}

/// Root manifest and workspace-member line no longer name the folded crate.
#[test]
fn root_manifest_no_longer_references_portable_pty_crate() {
    let manifest_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest = std::fs::read_to_string(manifest_path).expect("read root Cargo.toml");
    assert!(
        !manifest.lines().any(|l| l.contains("portable-pty")),
        "root Cargo.toml must not contain any line mentioning portable-pty"
    );
    let members_line = manifest
        .lines()
        .find(|l| l.contains("members = ["))
        .expect("workspace members line must exist");
    assert!(
        !members_line.contains("crates/portable-pty-psmux"),
        "workspace members must not list crates/portable-pty-psmux, got: {}",
        members_line
    );
}

/// The crate directory itself is deleted, not merely dropped from the
/// workspace.
#[test]
fn portable_pty_crate_directory_removed() {
    let dir_path = concat!(env!("CARGO_MANIFEST_DIR"), "/crates/portable-pty-psmux");
    assert!(
        !std::path::Path::new(dir_path).exists(),
        "crates/portable-pty-psmux must no longer exist"
    );
}

/// The release workflow's separate `cargo publish -p portable-pty-psmux`
/// step is removed along with the crate.
#[test]
fn release_workflow_has_no_portable_pty_publish_step() {
    let workflow_path = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/release.yml");
    let workflow = std::fs::read_to_string(workflow_path).expect("read release.yml");
    assert!(
        !workflow.contains("portable-pty-psmux"),
        "release.yml must not reference portable-pty-psmux"
    );
}

/// A live ConPTY round trip through the new crate::pty module: spawn a
/// child, read its echoed output through the pty, resize, and confirm
/// clean exit. This is a real ConPTY spawn (no mocks) -- existing
/// tests-rs suites already spawn real child processes through
/// portable_pty/ConPTY, so this is consistent with prior practice.
#[test]
fn live_conpty_round_trip_spawns_resizes_and_exits() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty must succeed");

    let mut cmd = CommandBuilder::new("cmd");
    cmd.args(["/c", "echo", "zdep-023-alive"]);
    let mut child = pair.slave.spawn_command(cmd).expect("spawn_command must succeed");

    let mut reader = pair
        .master
        .try_clone_reader()
        .expect("try_clone_reader must succeed");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = [0u8; 4096];
        let mut collected = String::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    collected.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if collected.contains("zdep-023-alive") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(collected);
    });

    let collected = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap_or_default();
    assert!(
        collected.contains("zdep-023-alive"),
        "expected pty output to contain the echoed token, got: {:?}",
        collected
    );

    pair.master
        .resize(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resize must succeed");
    let size = pair.master.get_size().expect("get_size must succeed");
    assert_eq!(size.rows, 30);
    assert_eq!(size.cols, 100);

    let status = child.wait().expect("wait must succeed");
    assert!(status.success(), "child must exit successfully");

    // Killing an already-exited child may succeed or return a benign
    // platform error (process already gone); either is acceptable, so we
    // do not assert on the outcome, only that it does not panic.
    let _ = child.kill();
}
