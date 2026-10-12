use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    FORMAT, JobResult, JobStatus, OnThread, RESULT, Spawner, await_started, job_status, run,
    write_job,
};
use crate::outcome::Refusal;
use crate::reviewer::{Adapter, Invocation, Receipt, ReviewerCode};

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
