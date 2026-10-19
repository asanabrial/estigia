#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use estigia::reviewer::job::{Detached, JobResult, JobStatus, Spawner, job_status, write_job};
use estigia::reviewer::{Invocation, Receipt};

fn invocation() -> Invocation {
    Invocation {
        role: "review-risk".to_owned(),
        receipt: Receipt {
            epoch: "e".repeat(32),
            pr: 7,
            head: "h".repeat(40),
            base: "b".repeat(40),
            digest: "d".repeat(64),
            payload_digest: "p".repeat(64),
        },
        prompt: b"review this".to_vec(),
        model: None,
    }
}

fn stand_in_claude(bin: &std::path::Path, release: &std::path::Path) {
    let said = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": "{\"findings\": []}",
    })
    .to_string();
    let program = bin.join("claude");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\n/bin/cat > /dev/null\nn=0\nwhile [ ! -e '{}' ] && [ $n -lt 300 ]; do /bin/sleep 0.1; n=$((n+1)); done\nprintf '%s' '{said}'\n",
            release.display()
        ),
    )
    .expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");
}

#[test]
fn a_detached_runner_returns_before_a_slow_reviewer_finishes_and_leaves_its_answer() {
    let root = tempfile::tempdir().expect("a directory");
    let home = root.path().join("home");
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let release = root.path().join("release");
    stand_in_claude(&bin, &release);
    let dir = root.path().join("job");
    write_job(&dir, &invocation()).expect("the job is written");

    let spawner = Detached::of(env!("CARGO_BIN_EXE_estigia"))
        .with_env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .with_env("HOME", &home);
    let pid = spawner.start(&dir).expect("the runner starts");

    assert!(pid > 0);
    assert_eq!(
        job_status(&dir).expect("status"),
        JobStatus::Running,
        "the launch waited for the reviewer, which was still held back"
    );
    std::fs::write(&release, "").expect("the reviewer is released");

    let until = Instant::now() + Duration::from_secs(20);
    let result = loop {
        match job_status(&dir).expect("status") {
            JobStatus::Finished(result) => break result,
            JobStatus::Lost => panic!(
                "the runner was lost: {}",
                std::fs::read_to_string(dir.join("runner.err")).unwrap_or_default()
            ),
            JobStatus::Running if Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(50));
            }
            JobStatus::Running => panic!("the runner never finished"),
        }
    };
    assert_eq!(result, JobResult::Answer(b"{\"findings\": []}".to_vec()));
    assert!(
        !home.join(".estigia").exists(),
        "the runner wrote under the home it was given"
    );
}

fn stand_in_claude_reporting_usage(bin: &std::path::Path) {
    let said = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": "{\"findings\": []}",
        "usage": { "input_tokens": 11, "output_tokens": 22 },
    })
    .to_string();
    let program = bin.join("claude");
    std::fs::write(
        &program,
        format!("#!/bin/sh\n/bin/cat > /dev/null\nprintf '%s' '{said}'\n"),
    )
    .expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");
}

#[test]
fn a_usage_write_that_fails_is_reported_and_leaves_the_answer_as_the_outcome() {
    let root = tempfile::tempdir().expect("a directory");
    let home = root.path().join("home");
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    stand_in_claude_reporting_usage(&bin);
    let dir = root.path().join("job");
    write_job(&dir, &invocation()).expect("the job is written");
    std::fs::create_dir(dir.join("usage.json"))
        .expect("a directory where the usage artifact would be written");

    let spawner = Detached::of(env!("CARGO_BIN_EXE_estigia"))
        .with_env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .with_env("HOME", &home);
    spawner.start(&dir).expect("the runner starts");

    let until = Instant::now() + Duration::from_secs(20);
    let result = loop {
        match job_status(&dir).expect("status") {
            JobStatus::Finished(result) => break result,
            JobStatus::Lost => panic!(
                "the runner was lost: {}",
                std::fs::read_to_string(dir.join("runner.err")).unwrap_or_default()
            ),
            JobStatus::Running if Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(50));
            }
            JobStatus::Running => panic!("the runner never finished"),
        }
    };
    assert_eq!(
        result,
        JobResult::Answer(b"{\"findings\": []}".to_vec()),
        "a failed usage write turned the review's answer into a refusal"
    );
    let reported = std::fs::read_to_string(dir.join("runner.err")).unwrap_or_default();
    assert!(
        reported.contains("usage.json") && reported.contains("could not be written"),
        "the failed usage write was not reported on the runner's standard error: {reported}"
    );
}

#[test]
fn a_runner_with_no_reviewer_on_the_path_leaves_a_refusal_not_silence() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path().join("job");
    write_job(&dir, &invocation()).expect("the job is written");
    Detached::of(env!("CARGO_BIN_EXE_estigia"))
        .with_env("PATH", root.path().display().to_string())
        .start(&dir)
        .expect("the runner starts");
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match job_status(&dir).expect("status") {
            JobStatus::Finished(JobResult::Refused(refusal)) => {
                assert_eq!(refusal.code, "reviewer-cli-missing");
                break;
            }
            JobStatus::Finished(other) => panic!("{other:?}"),
            _ if Instant::now() > until => panic!("no result"),
            _ => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

#[test]
fn a_runner_that_cannot_start_is_reported_by_the_launch() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path().join("job");
    std::fs::create_dir_all(&dir).expect("dir");
    let broken = root.path().join("not-estigia");
    std::fs::write(&broken, "#!/bin/sh\nexit 3\n").expect("script");
    std::fs::set_permissions(&broken, std::fs::Permissions::from_mode(0o755)).expect("exec");
    let refused = Detached::of(&broken).start(&dir);
    assert!(refused.is_err(), "{refused:?}");
}
