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

const STOP_DROPPED: &[u8] = br#"{ "final_stop_dropped": 1.0 }"#;

#[test]
fn a_habits_file_beside_the_voice_is_applied_before_the_text_is_kept() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::write_habits("house", STOP_DROPPED);
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    assert_eq!(
        render_one(&context, &renderer, "Hola.").expect("a rendering"),
        "voiced: Hola"
    );
    assert!(!probe.prompt().contains("final_stop_dropped"));
}

#[test]
fn a_retry_reads_the_kept_text_and_applies_the_habits_nothing_more() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::write_habits("house", STOP_DROPPED);
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let first = render_one(&context, &renderer, "Hola. .").expect("a rendering");
    assert_eq!(first, "voiced: Hola. ");
    assert_eq!(
        render_one(&context, &renderer, "Hola. .").expect("a retry"),
        first
    );
    assert_eq!(probe.launched(), 1);
}

#[test]
fn editing_the_habits_file_is_another_key_and_asks_the_job_again() {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::write_habits("house", STOP_DROPPED);
    stand_in::name_voice(repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    render_one(&context, &renderer, "Hola.").expect("a rendering");
    stand_in::write_habits("house", br#"{ "final_stop_dropped": 0.0 }"#);
    assert_eq!(
        render_one(&context, &renderer, "Hola.").expect("another rendering"),
        "voiced: Hola."
    );
    assert_eq!(probe.launched(), 2);
}

fn habits_reason(write: impl Fn(&std::path::Path)) -> (serde_json::Value, usize) {
    let (repo, context) = context();
    stand_in::write_voice("house", "Plain and brief.");
    stand_in::name_voice(repo.path(), "house");
    let path = stand_in::habits_path("house");
    let _ = std::fs::remove_dir_all(&path);
    let _ = std::fs::remove_file(&path);
    write(&path);
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = render_one(&context, &renderer, "text").expect_err("invalid habits");
    (failure.envelope(), probe.launched())
}

fn written(bytes: &'static [u8]) -> impl Fn(&std::path::Path) {
    move |path| std::fs::write(path, bytes).expect("a habits file")
}

#[test]
fn every_invalid_habits_file_stops_the_post_and_launches_no_job() {
    let over = vec![b' '; 16 * 1024 + 1];
    let over: &'static [u8] = Box::leak(over.into_boxed_slice());
    type Making = Box<dyn Fn(&std::path::Path)>;
    let cases: [(&str, Making); 7] = [
        (
            "directory",
            Box::new(|path| std::fs::create_dir_all(path).expect("a directory")),
        ),
        ("cap", Box::new(written(over))),
        ("utf8", Box::new(written(&[0xff, 0xfe, 0x7b]))),
        ("json", Box::new(written(b"not json"))),
        ("field", Box::new(written(br#"{ "unknown": 1 }"#))),
        ("high", Box::new(written(br#"{ "lowercase_start": 1.5 }"#))),
        ("low", Box::new(written(br#"{ "lowercase_start": -0.1 }"#))),
    ];
    for (name, write) in cases {
        let (envelope, launched) = habits_reason(write);
        assert_eq!(envelope["reason"], "review-voice-habits-invalid", "{name}");
        assert!(
            envelope["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("house.habits.json")),
            "{name}: {envelope}"
        );
        assert_eq!(launched, 0, "{name}");
    }
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
