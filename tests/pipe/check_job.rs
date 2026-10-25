#![cfg(unix)]

use std::process::{Command, Stdio};

fn manifest(dir: &std::path::Path, tree: &std::path::Path, scripts: &[(&str, &str)]) {
    let checks: Vec<serde_json::Value> = scripts
        .iter()
        .map(|(name, script)| {
            serde_json::json!({"name": name, "command": ["sh", "-c", script], "cwd": null})
        })
        .collect();
    std::fs::create_dir_all(dir).expect("the job directory");
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::json!({
            "format": 1,
            "worktree": tree.display().to_string(),
            "digest": "d".repeat(64),
            "commands": "c".repeat(64),
            "checks": checks,
        })
        .to_string(),
    )
    .expect("the manifest");
}

#[test]
fn the_hidden_check_job_runs_each_check_and_leaves_a_result_before_it_exits() {
    let root = tempfile::tempdir().expect("a directory");
    let tree = root.path().join("tree");
    std::fs::create_dir_all(&tree).expect("the tree");
    let dir = root.path().join("job");
    manifest(
        &dir,
        &tree,
        &[
            ("fails", "echo broken >&2; exit 4"),
            ("passes", "echo fine"),
        ],
    );

    let status = Command::new(env!("CARGO_BIN_EXE_estigia"))
        .arg("__check-job")
        .arg(&dir)
        .stdin(Stdio::null())
        .status()
        .expect("the runner starts");
    assert!(status.success(), "{status:?}");

    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("result.json")).expect("a result"))
            .expect("JSON");
    assert_eq!(result["timed_out"], false);
    assert_eq!(result["checks"][0]["name"], "fails");
    assert_eq!(result["checks"][0]["exit"], 4);
    assert!(
        result["checks"][0]["tail"]
            .as_str()
            .is_some_and(|tail| tail.contains("broken")),
        "{result}"
    );
    assert_eq!(result["checks"][1]["exit"], 0);
}

#[test]
fn the_hidden_check_job_is_not_a_visible_subcommand() {
    let help = Command::new(env!("CARGO_BIN_EXE_estigia"))
        .arg("--help")
        .output()
        .expect("help");
    assert!(!String::from_utf8_lossy(&help.stdout).contains("__check-job"));
}
