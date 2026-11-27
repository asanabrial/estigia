#![cfg(unix)]

use super::*;

fn check(name: &str, script: &str) -> Check {
    Check {
        name: name.to_owned(),
        command: vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()],
        cwd: None,
    }
}

fn settled(dir: &Path) -> JobStatus<Outcome> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match status(dir).expect("status") {
            JobStatus::Running => {
                assert!(Instant::now() < until, "the job never let its lock go");
                std::thread::sleep(Duration::from_millis(5));
            }
            settled => return settled,
        }
    }
}

fn ran(tree: &Path, checks: &[Check], deadline: Duration) -> Outcome {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path().join("job");
    write_job(&dir, tree, &"d".repeat(64), &"c".repeat(64), checks).expect("the job is written");
    run_within(&dir, deadline).expect("the runner finishes");
    match settled(&dir) {
        JobStatus::Finished(outcome) => outcome,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_failing_check_does_not_stop_the_rest_and_the_job_does_not_pass() {
    let tree = tempfile::tempdir().expect("a tree");
    let outcome = ran(
        tree.path(),
        &[check("first", "exit 3"), check("second", "exit 0")],
        DEADLINE,
    );
    assert!(!outcome.timed_out);
    assert_eq!(
        outcome
            .runs
            .iter()
            .map(|run| (run.name.as_str(), run.exit))
            .collect::<Vec<_>>(),
        vec![("first", Some(3)), ("second", Some(0))]
    );
    assert!(!outcome.passed(2));
}

#[test]
fn every_check_exiting_zero_passes() {
    let tree = tempfile::tempdir().expect("a tree");
    let outcome = ran(tree.path(), &[check("only", "exit 0")], DEADLINE);
    assert!(outcome.passed(1));
}

#[test]
fn a_check_runs_in_its_cwd_inside_the_tree() {
    let tree = tempfile::tempdir().expect("a tree");
    std::fs::create_dir(tree.path().join("sub")).expect("sub");
    std::fs::write(tree.path().join("sub").join("only-here"), "").expect("a file");
    let outcome = ran(
        tree.path(),
        &[Check {
            name: "where".to_owned(),
            command: vec!["ls".to_owned()],
            cwd: Some("sub".to_owned()),
        }],
        DEADLINE,
    );
    assert_eq!(outcome.runs[0].exit, Some(0));
    assert!(outcome.runs[0].tail.contains("only-here"));
}

#[test]
fn a_check_sees_the_build_variables_and_nothing_else() {
    let tree = tempfile::tempdir().expect("a tree");
    let outcome = crate::test_env::with_process_env("ESTIGIA_LEAKED_TOKEN", "s3cret", || {
        ran(
            tree.path(),
            &[
                check("leak", "printf %s \"$ESTIGIA_LEAKED_TOKEN\""),
                check("path", "printf %s \"$PATH\""),
            ],
            DEADLINE,
        )
    });
    assert_eq!(
        outcome.runs[0].tail, "",
        "a check read a variable outside the allow-list, so it runs with the session's whole \
         environment"
    );
    assert!(
        !outcome.runs[1].tail.is_empty(),
        "a check lost `PATH`, so nothing it runs can be found"
    );
}

#[test]
fn a_command_is_an_argv_and_no_shell_expands_it() {
    let tree = tempfile::tempdir().expect("a tree");
    let outcome = ran(
        tree.path(),
        &[Check {
            name: "literal".to_owned(),
            command: vec![
                "printf".to_owned(),
                "%s".to_owned(),
                "$HOME; echo hi".to_owned(),
            ],
            cwd: None,
        }],
        DEADLINE,
    );
    assert_eq!(outcome.runs[0].tail, "$HOME; echo hi");
}

#[test]
fn the_tail_is_the_last_two_thousand_bytes_of_both_streams() {
    let tree = tempfile::tempdir().expect("a tree");
    let outcome = ran(
        tree.path(),
        &[check(
            "noisy",
            "i=0; while [ $i -lt 400 ]; do echo out-$i; echo err-$i >&2; i=$((i+1)); done; echo last-line",
        )],
        DEADLINE,
    );
    let tail = &outcome.runs[0].tail;
    assert!(tail.len() <= 2_000, "{}", tail.len());
    assert!(tail.trim_end().ends_with("last-line"), "{tail}");
    assert!(tail.contains("err-399"), "{tail}");
}

#[test]
fn a_program_that_cannot_start_is_a_failure_with_no_exit_code() {
    let tree = tempfile::tempdir().expect("a tree");
    let outcome = ran(
        tree.path(),
        &[Check {
            name: "gone".to_owned(),
            command: vec!["/nonexistent/estigia-program".to_owned()],
            cwd: None,
        }],
        DEADLINE,
    );
    assert_eq!(outcome.runs[0].exit, None);
    assert!(outcome.runs[0].tail.contains("could not start"));
    assert!(!outcome.passed(1));
}

#[test]
fn past_the_deadline_the_group_is_killed_and_the_job_times_out() {
    let tree = tempfile::tempdir().expect("a tree");
    let began = Instant::now();
    let outcome = ran(
        tree.path(),
        &[check("slow", "sleep 30"), check("never", "exit 0")],
        Duration::from_millis(300),
    );
    assert!(began.elapsed() < Duration::from_secs(15));
    assert!(outcome.timed_out);
    assert_eq!(outcome.runs.len(), 1);
    assert_eq!(outcome.runs[0].exit, None);
    assert!(!outcome.passed(2));
}

#[test]
fn a_manifest_the_runner_cannot_read_leaves_no_result() {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path().join("job");
    std::fs::create_dir_all(&dir).expect("dir");
    assert!(run_within(&dir, DEADLINE).is_err());
    let until = Instant::now() + Duration::from_secs(10);
    let found = loop {
        let found = status(&dir).expect("status");
        if found != JobStatus::Running || Instant::now() > until {
            break found;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(found, JobStatus::Lost);
}

#[test]
fn a_check_job_written_again_does_not_keep_the_previous_attempts_result() {
    let tree = tempfile::tempdir().expect("a tree");
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path().join("job");
    write_job(&dir, tree.path(), &"d".repeat(64), &"c".repeat(64), &[])
        .expect("the job is written");
    std::fs::write(dir.join(job::RESULT), "{\"checks\":[]}").expect("a result");
    std::fs::write(dir.join(job::STARTED), "").expect("a started marker");

    write_job(&dir, tree.path(), &"d".repeat(64), &"c".repeat(64), &[]).expect("written again");

    for left in [job::RESULT, job::STARTED] {
        assert!(
            !dir.join(left).exists(),
            "`{left}` from the previous attempt outlived the rewrite"
        );
    }
}
