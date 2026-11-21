use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{VOICE_CAP, answer_through, read_capped, render, render_one};
use crate::transport::Context;
use crate::transport::voice::stand_in::{self, Answers};

fn context() -> (tempfile::TempDir, Context) {
    crate::transport::store::forget_scratch_store();
    let repo = crate::test_env::repository_with_commit();
    let context = Context {
        skill_dir: repo.path().join("skill"),
        repo_dir: repo.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    (repo, context)
}

fn texts(of: &[&str]) -> Vec<String> {
    of.iter().map(|text| (*text).to_owned()).collect()
}

#[test]
fn a_repository_naming_no_voice_gets_its_texts_back_untouched_and_no_job_runs() {
    let (_repo, context) = context();
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let given = texts(&["one", "two"]);
    assert_eq!(
        render(&context, &renderer, &given).expect("untouched"),
        given
    );
    assert_eq!(probe.launched(), 0);
}

#[test]
fn each_text_is_reworded_in_order_and_only_the_missing_ones_are_asked_for() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Counting);
    assert_eq!(
        render_one(&context, &renderer, "first").expect("a rendering"),
        "voiced #1: first"
    );
    let both = render(&context, &renderer, &texts(&["first", "second"])).expect("both");
    assert_eq!(both, ["voiced #1: first", "voiced #2: second"]);
    assert_eq!(probe.launched(), 2);
    let handed = probe.prompt();
    assert!(handed.contains("[\"second\"]"), "{handed}");
    assert!(!handed.contains("first"), "{handed}");
}

#[test]
fn the_job_is_handed_the_fixed_instructions_whatever_the_voice_says() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Answer in Spanish.");
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    render_one(&context, &renderer, "text").expect("a rendering");
    let prompt = probe.prompt();
    for fixed in [
        "Change wording only.",
        "byte for byte",
        "Add no finding id, class, label",
        "Answer strictly one text per input, in the order given, as JSON.",
    ] {
        assert!(prompt.contains(fixed), "{fixed}: {prompt}");
    }
    assert!(prompt.contains("Answer in Spanish."));
}

#[test]
fn a_job_that_has_not_answered_by_its_deadline_is_stopped_and_refused() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::name_voice(repo.path(), "house");
    let killed = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&killed);
    let renderer = stand_in::renderer_that_hangs().within(0, move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    let failure = render_one(&context, &renderer, "text").expect_err("no answer in time");
    assert_eq!(failure.envelope()["reason"], "review-voice-render-failed");
    assert_eq!(killed.load(Ordering::SeqCst), 1);
}

#[test]
fn nothing_to_reword_is_returned_without_the_voice_file_being_asked_for() {
    let (repo, context) = context();
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    assert_eq!(
        render(&context, &renderer, &[]).expect("nothing to do"),
        Vec::<String>::new()
    );
    assert_eq!(probe.launched(), 0);
}

fn unreadable_reason(path_made: impl Fn(&std::path::Path)) -> serde_json::Value {
    let (repo, context) = context();
    stand_in::name_voice(repo.path(), "house");
    path_made(&stand_in::voices_dir().join("house.md"));
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = render_one(&context, &renderer, "text").expect_err("not a voice file");
    assert_eq!(probe.launched(), 0);
    failure.envelope()["reason"].clone()
}

#[test]
fn a_voice_path_that_is_a_directory_is_unreadable() {
    let reason = unreadable_reason(|path| std::fs::create_dir_all(path).expect("a directory"));
    assert_eq!(reason, "review-voice-file-unreadable");
}

#[cfg(unix)]
#[test]
fn a_voice_path_that_is_a_fifo_is_unreadable_and_is_not_opened() {
    let reason = unreadable_reason(|path| {
        std::fs::create_dir_all(path.parent().expect("a directory")).expect("a directory");
        let name = std::ffi::CString::new(path.to_str().expect("a path")).expect("no NUL");
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    });
    assert_eq!(reason, "review-voice-file-unreadable");
}

#[test]
fn a_voice_that_grows_past_the_cap_after_it_was_measured_is_cut_at_the_cap_plus_one() {
    let read = read_capped(std::io::repeat(b'x')).expect("a bounded read");
    assert_eq!(read.len() as u64, VOICE_CAP + 1);
    let within = read_capped(&b"short"[..]).expect("a short read");
    assert_eq!(within, b"short");
}

fn jobs_left() -> usize {
    std::fs::read_dir(crate::transport::claim::jobs_dir().expect("a jobs directory"))
        .map(|entries| entries.count())
        .unwrap_or(0)
}

#[test]
fn a_job_whose_status_cannot_be_read_is_killed_and_its_directory_removed() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::name_voice(repo.path(), "house");
    let killed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&killed);
    let renderer = stand_in::renderer_whose_job_status_cannot_be_read(move |pid| {
        seen.lock().expect("the lock").push(pid);
    });
    let failure = render_one(&context, &renderer, "text").expect_err("an unreadable status");
    assert_eq!(failure.envelope()["reason"], "review-voice-render-failed");
    assert_eq!(*killed.lock().expect("the lock"), [stand_in::STARTED_PID]);
    assert_eq!(jobs_left(), 0);
}

#[test]
fn a_job_that_could_not_be_written_leaves_no_directory_and_starts_nothing() {
    let (repo, context) = context();
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = answer_through(
        &context,
        &renderer,
        "house",
        "voice",
        &["text"],
        |dir, _| {
            std::fs::create_dir_all(dir).expect("a directory");
            std::fs::write(dir.join("prompt"), "half").expect("a partial job");
            Err(crate::transport::Failure::Write("no space".to_owned()))
        },
    )
    .expect_err("an unwritten job");
    assert!(matches!(failure, crate::transport::Failure::Write(_)));
    assert_eq!(probe.launched(), 0);
    assert_eq!(jobs_left(), 0);
}

#[test]
fn a_row_the_repository_file_cannot_be_read_from_stops_rather_than_posting_plain() {
    let (repo, context) = context();
    stand_in::name_voice(repo.path(), "../outside");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = render_one(&context, &renderer, "text").expect_err("an unreadable row");
    assert_eq!(failure.envelope()["reason"], "review-voice-row-unreadable");
    assert_eq!(probe.launched(), 0);
}
