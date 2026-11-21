use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{render, render_one};
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
fn a_row_the_repository_file_cannot_be_read_from_stops_rather_than_posting_plain() {
    let (repo, context) = context();
    stand_in::name_voice(repo.path(), "../outside");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = render_one(&context, &renderer, "text").expect_err("an unreadable row");
    assert_eq!(failure.envelope()["reason"], "review-voice-row-unreadable");
    assert_eq!(probe.launched(), 0);
}
