use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    FORMAT, JobResult, JobStatus, OnThread, RESULT, Spawner, USAGE, await_started, job_status,
    read_usage, run, write_job,
};
use crate::outcome::Refusal;
use crate::reviewer::{Adapter, Invocation, Receipt, Review, ReviewerCode, Usage};

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
        prompt: b"the prompt \xff bytes".to_vec(),
        model: Some("a-model".to_owned()),
    }
}

struct Echo;

impl Adapter for Echo {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        let mut answer = format!("{}|{:?}|", invocation.role, invocation.model).into_bytes();
        answer.extend(&invocation.prompt);
        Ok(answer)
    }
}

struct Refuses;

impl Adapter for Refuses {
    fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
        Err(ReviewerCode::Unavailable.refusal("out of credit"))
    }
}

struct Reports(Option<Usage>);

impl Adapter for Reports {
    fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
        Ok(b"the answer".to_vec())
    }

    fn review_with_usage(&self, _: &Invocation) -> Result<Review, Refusal> {
        Ok(Review {
            answer: b"the answer".to_vec(),
            usage: self.0.clone(),
        })
    }
}

fn usage() -> Usage {
    Usage {
        provider: "claude".to_owned(),
        counters: [
            ("input_tokens".to_owned(), 100_u64),
            ("cache_read_input_tokens".to_owned(), 250),
            ("output_tokens".to_owned(), 20),
        ]
        .into_iter()
        .collect(),
    }
}

fn echo() -> Result<Box<dyn Adapter>, Refusal> {
    Ok(Box::new(Echo))
}

fn refuses() -> Result<Box<dyn Adapter>, Refusal> {
    Ok(Box::new(Refuses))
}

fn prepared() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("a directory");
    let dir = root.path().join("job");
    write_job(&dir, &invocation()).expect("the job is written");
    (root, dir)
}

fn finished(dir: &std::path::Path) -> JobResult {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match job_status(dir).expect("a readable status") {
            JobStatus::Finished(result) => return result,
            other if Instant::now() > until => panic!("never finished: {other:?}"),
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn settled(dir: &std::path::Path) -> JobStatus {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match job_status(dir).expect("a readable status") {
            JobStatus::Running if Instant::now() <= until => {
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
}

#[test]
fn the_runner_leaves_the_answer_the_adapter_gave_byte_for_byte() {
    let (_root, dir) = prepared();
    OnThread(echo).start(&dir).expect("the runner starts");
    let JobResult::Answer(bytes) = finished(&dir) else {
        panic!("a refusal where an answer was expected");
    };
    let mut wanted = b"review-risk|Some(\"a-model\")|".to_vec();
    wanted.extend(b"the prompt \xff bytes");
    assert_eq!(bytes, wanted);
}

#[test]
fn the_runner_leaves_the_refusal_the_adapter_gave() {
    let (_root, dir) = prepared();
    OnThread(refuses).start(&dir).expect("the runner starts");
    let JobResult::Refused(refusal) = finished(&dir) else {
        panic!("an answer where a refusal was expected");
    };
    assert_eq!(refusal.code, "reviewer-unavailable");
    assert_eq!(refusal.class, "unavailable");
    assert!(refusal.message.contains("out of credit"));
    assert_eq!(refusal.resolution["kind"], "no_command");
}

#[test]
fn an_adapter_that_cannot_be_selected_is_a_recorded_refusal() {
    let (_root, dir) = prepared();
    let missing = || Err(ReviewerCode::CliMissing.refusal("no reviewer"));
    OnThread(missing).start(&dir).expect("the runner starts");
    let JobResult::Refused(refusal) = finished(&dir) else {
        panic!("an answer from nothing");
    };
    assert_eq!(refusal.code, "reviewer-cli-missing");
}

#[test]
fn a_manifest_in_another_format_is_a_recorded_refusal_and_never_run() {
    let (_root, dir) = prepared();
    std::fs::write(dir.join("manifest.json"), r#"{"format": 99}"#).expect("rewritten");
    OnThread(echo).start(&dir).expect("the runner starts");
    let JobResult::Refused(refusal) = finished(&dir) else {
        panic!("a manifest from the future was run");
    };
    assert_eq!(refusal.code, "reviewer-launch-failed");
    assert!(refusal.message.contains("99") || refusal.message.contains("format"));
}

#[test]
fn a_result_in_another_format_is_not_read() {
    let (_root, dir) = prepared();
    std::fs::write(
        dir.join("result.json"),
        r#"{"format": 99, "answer_hex": "00"}"#,
    )
    .expect("written");
    assert!(job_status(&dir).is_err());
    assert_eq!(FORMAT, 1);
}

#[test]
fn a_slow_reviewer_is_running_after_the_start_returns_and_finished_when_it_answers() {
    struct Waits(std::sync::Arc<std::sync::Mutex<mpsc::Receiver<()>>>);
    impl Adapter for Waits {
        fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
            let _ = self.0.lock().expect("the gate").recv();
            Ok(b"late".to_vec())
        }
    }
    let (release, gate) = mpsc::channel();
    let gate = std::sync::Arc::new(std::sync::Mutex::new(gate));
    let (_root, dir) = prepared();
    let select = move || -> Result<Box<dyn Adapter>, Refusal> { Ok(Box::new(Waits(gate.clone()))) };
    OnThread(select).start(&dir).expect("the runner starts");

    assert_eq!(job_status(&dir).expect("status"), JobStatus::Running);
    release.send(()).expect("released");
    assert_eq!(finished(&dir), JobResult::Answer(b"late".to_vec()));
}

#[test]
fn a_free_lock_with_no_result_is_lost() {
    let (_root, dir) = prepared();
    assert_eq!(job_status(&dir).expect("status"), JobStatus::Lost);
    std::fs::write(dir.join("lock"), b"").expect("a lock file nobody holds");
    assert_eq!(job_status(&dir).expect("status"), JobStatus::Lost);
}

#[test]
fn a_second_runner_on_a_held_job_refuses_and_writes_nothing() {
    let (_root, dir) = prepared();
    let held = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("lock"))
        .expect("opens");
    held.try_lock().expect("held");
    assert_eq!(job_status(&dir).expect("status"), JobStatus::Running);
    assert!(run(&dir, &echo).is_err());
    assert!(!dir.join("result.json").exists());
}

#[test]
fn the_runner_never_touches_the_ledger() {
    let (root, dir) = prepared();
    let path = root.path().join("state.db");
    crate::transport::store::Store::open(&path).expect("opens");
    let connection = rusqlite::Connection::open(&path).expect("opens");
    let count = || -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .expect("counts")
    };
    let before = count();
    OnThread(echo).start(&dir).expect("starts");
    finished(&dir);
    assert_eq!(count(), before);
}

#[test]
fn a_runner_that_finished_between_the_two_looks_is_started_not_stopped_before_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let result = dir.path().join(RESULT);
    let looked = await_started(dir.path(), || {
        std::fs::write(&result, "").unwrap();
        Some("finished".to_owned())
    });
    assert!(looked.is_ok(), "{looked:?}");
}

#[test]
fn a_runner_that_stopped_having_written_nothing_is_stopped_before_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let looked = await_started(dir.path(), || Some("finished".to_owned()));
    assert!(looked.is_err());
}

#[test]
fn a_finished_job_preserves_the_usage_its_adapter_reported() {
    let (_root, dir) = prepared();
    let reported = usage();
    let select = {
        let reported = reported.clone();
        move || -> Result<Box<dyn Adapter>, Refusal> {
            Ok(Box::new(Reports(Some(reported.clone()))))
        }
    };
    OnThread(select).start(&dir).expect("the runner starts");
    assert_eq!(finished(&dir), JobResult::Answer(b"the answer".to_vec()));
    assert!(dir.join(USAGE).is_file(), "no usage artifact was written");

    let artifact: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(USAGE)).expect("the usage artifact"))
            .expect("the usage artifact is JSON");
    let mut keys: Vec<&str> = artifact
        .as_object()
        .expect("the artifact is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["counters", "format", "provider"]);
    assert_eq!(artifact["provider"], "claude");
    assert_eq!(artifact["counters"].as_object().expect("counters").len(), 3);
    assert_eq!(read_usage(&dir).expect("readable"), Some(reported));
}

#[test]
fn a_job_whose_reviewer_reported_no_usage_writes_no_artifact() {
    let (_root, dir) = prepared();
    OnThread(echo).start(&dir).expect("the runner starts");
    let _ = finished(&dir);
    assert!(
        !dir.join(USAGE).exists(),
        "an empty usage artifact was written"
    );
    assert_eq!(read_usage(&dir).expect("readable"), None);
}

#[test]
fn the_reader_answers_nothing_for_a_directory_holding_no_artifact() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_usage(dir.path()).expect("readable"), None);
}

#[test]
fn discarding_the_result_leaves_the_usage_artifact_readable() {
    let (_root, dir) = prepared();
    let reported = usage();
    let select = {
        let reported = reported.clone();
        move || -> Result<Box<dyn Adapter>, Refusal> {
            Ok(Box::new(Reports(Some(reported.clone()))))
        }
    };
    OnThread(select).start(&dir).expect("the runner starts");
    let _ = finished(&dir);

    super::discard_result(&dir).expect("the result is discarded");
    assert_eq!(settled(&dir), JobStatus::Lost);
    assert_eq!(read_usage(&dir).expect("readable"), Some(reported));
}

#[test]
fn a_status_read_after_a_discard_waits_for_the_runner_to_let_go_of_its_lock() {
    let (_root, dir) = prepared();
    let held = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("lock"))
        .expect("opens");
    held.try_lock().expect("held");
    assert_eq!(job_status(&dir).expect("status"), JobStatus::Running);
    let runner = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        drop(held);
    });
    assert_eq!(settled(&dir), JobStatus::Lost);
    runner.join().expect("the runner returns");
}

#[test]
fn the_usage_artifact_never_changes_the_admitted_answer_bytes() {
    let (_root, plain) = prepared();
    let silent = || -> Result<Box<dyn Adapter>, Refusal> { Ok(Box::new(Reports(None))) };
    OnThread(silent).start(&plain).expect("the runner starts");
    let _ = finished(&plain);

    let (_root, reported) = prepared();
    let with_usage =
        || -> Result<Box<dyn Adapter>, Refusal> { Ok(Box::new(Reports(Some(usage())))) };
    OnThread(with_usage)
        .start(&reported)
        .expect("the runner starts");
    let _ = finished(&reported);

    assert_eq!(
        std::fs::read(plain.join(RESULT)).expect("the result without usage"),
        std::fs::read(reported.join(RESULT)).expect("the result with usage"),
        "the usage moved the admitted answer's bytes"
    );
}

#[test]
fn a_usage_write_that_fails_leaves_the_answer_as_the_outcome() {
    let (_root, dir) = prepared();
    std::fs::create_dir(dir.join(USAGE))
        .expect("a directory where the usage artifact would be written");

    let select = || -> Result<Box<dyn Adapter>, Refusal> { Ok(Box::new(Reports(Some(usage())))) };
    OnThread(select).start(&dir).expect("the runner starts");

    assert_eq!(
        finished(&dir),
        JobResult::Answer(b"the answer".to_vec()),
        "a failed usage write turned the review's answer into a refusal"
    );
    assert!(
        read_usage(&dir).is_err(),
        "a usage artifact was readable after a write that failed: {:?}",
        read_usage(&dir)
    );
    assert!(
        dir.join(USAGE).is_dir(),
        "the obstruction is gone, so the usage write did not fail"
    );
}

#[test]
fn the_writer_never_emits_the_empty_counter_map_the_reader_refuses() {
    let (_root, dir) = prepared();
    let empty = Usage {
        provider: "claude".to_owned(),
        counters: std::collections::BTreeMap::new(),
    };
    let select =
        move || -> Result<Box<dyn Adapter>, Refusal> { Ok(Box::new(Reports(Some(empty.clone())))) };
    OnThread(select).start(&dir).expect("the runner starts");

    assert_eq!(finished(&dir), JobResult::Answer(b"the answer".to_vec()));
    assert!(
        !dir.join(USAGE).exists(),
        "the writer emitted an empty counter map"
    );
    assert_eq!(read_usage(&dir).expect("readable"), None);
}

fn usage_artifact(dir: &std::path::Path, content: &str) {
    std::fs::write(dir.join(USAGE), content).expect("the usage artifact is written");
}

#[test]
fn the_reader_refuses_a_usage_artifact_that_is_not_json() {
    let dir = tempfile::tempdir().unwrap();
    usage_artifact(dir.path(), "not json at all");
    let error = read_usage(dir.path()).expect_err("a corrupt artifact is an error");
    assert!(error.detail().contains("not JSON"), "{error:?}");
}

#[test]
fn the_reader_refuses_a_usage_artifact_in_another_format() {
    let dir = tempfile::tempdir().unwrap();
    usage_artifact(
        dir.path(),
        r#"{"format": 99, "provider": "claude", "counters": {"input_tokens": 1}}"#,
    );
    let error = read_usage(dir.path()).expect_err("a foreign format is an error");
    assert!(error.detail().contains("format"), "{error:?}");
}

#[test]
fn the_reader_refuses_a_usage_artifact_that_names_no_provider() {
    let dir = tempfile::tempdir().unwrap();
    usage_artifact(
        dir.path(),
        r#"{"format": 1, "counters": {"input_tokens": 1}}"#,
    );
    let error = read_usage(dir.path()).expect_err("a providerless artifact is an error");
    assert!(error.detail().contains("provider"), "{error:?}");
}

#[test]
fn the_reader_refuses_a_usage_artifact_that_holds_no_counters_field() {
    let dir = tempfile::tempdir().unwrap();
    usage_artifact(dir.path(), r#"{"format": 1, "provider": "claude"}"#);
    let error = read_usage(dir.path()).expect_err("a counterless artifact is an error");
    assert!(error.detail().contains("counters"), "{error:?}");
}

#[test]
fn the_reader_refuses_a_usage_artifact_with_a_counter_that_is_not_a_whole_number() {
    let dir = tempfile::tempdir().unwrap();
    usage_artifact(
        dir.path(),
        r#"{"format": 1, "provider": "claude", "counters": {"input_tokens": "many"}}"#,
    );
    let error = read_usage(dir.path()).expect_err("a non-numeric counter is an error");
    assert!(error.detail().contains("whole number"), "{error:?}");
}

#[test]
fn the_reader_refuses_a_usage_artifact_whose_counters_are_empty() {
    let dir = tempfile::tempdir().unwrap();
    usage_artifact(
        dir.path(),
        r#"{"format": 1, "provider": "claude", "counters": {}}"#,
    );
    let error = read_usage(dir.path()).expect_err("an empty counter set is corrupt");
    assert!(error.detail().contains("no counters"), "{error:?}");
}
