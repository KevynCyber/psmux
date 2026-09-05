// Covers: ZDEP-019
// Requirement: the committed golden `tests-rs/fixtures/crate_tree_x86_64.txt`
// (ZDEP-003) is enforced against the live dependency tree. Running `cargo
// tree -e normal --target x86_64-pc-windows-msvc --prefix none --locked`
// from the root manifest directory, normalising both outputs (cut each line
// at its first ` (` annotation, drop `\r`, trim, drop empty lines, sort,
// dedupe) and comparing must yield no drift; a mismatch reports which lines
// are only in the live tree and which are only in the golden.

use std::process::Command;

fn normalize(s: &str) -> Vec<String> {
    let mut lines: Vec<String> = s
        .lines()
        .map(|l| l.replace('\r', ""))
        .map(|l| {
            let trimmed = l.trim();
            if let Some(idx) = trimmed.find(" (") {
                trimmed[..idx].trim().to_string()
            } else {
                trimmed.to_string()
            }
        })
        .filter(|l| !l.is_empty())
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

#[test]
fn normalize_strips_suffixes_crlf_blanks_and_dedupes() {
    let input = "\r\nfoo v1.0.0 (*)\r\nbar v2.0.0 (proc-macro)\r\n  \r\nfoo v1.0.0 (*)\r\n  baz v0.1.0  \r\nqux v3.0.0 (proc-macro) (*)\r\n";
    let expected = vec![
        "bar v2.0.0".to_string(),
        "baz v0.1.0".to_string(),
        "foo v1.0.0".to_string(),
        "qux v3.0.0".to_string(),
    ];
    assert_eq!(normalize(input), expected);
}

#[test]
fn crate_tree_matches_committed_golden() {
    let root = env!("CARGO_MANIFEST_DIR");
    let cargo_bin = option_env!("CARGO")
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CARGO").ok())
        .unwrap_or_else(|| "cargo".to_string());

    let output = Command::new(&cargo_bin)
        .args([
            "tree",
            "-e",
            "normal",
            "--target",
            "x86_64-pc-windows-msvc",
            "--prefix",
            "none",
            "--locked",
        ])
        .current_dir(root)
        .output()
        .expect("failed to spawn cargo tree");

    if !output.status.success() {
        panic!(
            "cargo tree failed with status {:?}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let live = String::from_utf8_lossy(&output.stdout);
    let golden_path = format!("{}/tests-rs/fixtures/crate_tree_x86_64.txt", root);
    let golden = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", golden_path, e));

    let live_lines = normalize(&live);
    let golden_lines = normalize(&golden);

    let only_live: Vec<&String> = live_lines
        .iter()
        .filter(|l| !golden_lines.contains(l))
        .collect();
    let only_golden: Vec<&String> = golden_lines
        .iter()
        .filter(|l| !live_lines.contains(l))
        .collect();

    assert!(
        only_live.is_empty() && only_golden.is_empty(),
        "crate tree drifted from tests-rs/fixtures/crate_tree_x86_64.txt\nonly in live tree: {:?}\nonly in golden: {:?}",
        only_live,
        only_golden
    );
}
