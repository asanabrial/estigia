use super::*;
use crate::transport::voice::stand_in::{self, Answers};

const REPLY: &str = "Fixed the concern.";
const VOICED: &str = "voiced: Fixed the concern.";

fn posted_replies(log: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .split("--- ")
        .filter(|entry| entry.contains("addPullRequestReviewThreadReply"))
        .filter_map(|entry| entry.split_once('\n'))
        .map(|(_, body)| body.trim_end_matches('\n').to_owned())
        .collect()
}

fn readings(rig: &ThreadRig, reply: &str) -> String {
    rig.script(&[
        rig.pr(false, false),
        rig.pr_replied(Some(reply), false),
        rig.pr_replied(Some(reply), false),
        rig.pr_replied(Some(reply), true),
    ])
}

fn respond_voiced(
    rig: &ThreadRig,
    script: &str,
    renderer: &crate::transport::voice::Renderer,
) -> (Result<serde_json::Value, Failure>, Vec<String>) {
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("bodies");
    let counts = rig.root.path().join("counts");
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&counts);
    let answer = crate::test_env::with_counting_body_log(bin.path(), script, &counts, &log, || {
        thread::respond_with(&rig.context, &rig.response(Some(&rig.fix), true), renderer)
    });
    (answer, posted_replies(&log))
}

#[test]
fn a_named_voice_posts_the_rendered_reply_and_the_checks_read_what_they_read_today() {
    let rig = ThreadRig::new();
    stand_in::write_voice("house", "Sound like a colleague.");
    stand_in::name_voice(rig.repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let (answer, posted) = respond_voiced(&rig, &readings(&rig, VOICED), &renderer);
    let answer = answer.expect("the reply");
    assert_eq!(posted, [VOICED]);
    assert_eq!(answer["thread_id"], "thread-1");
    assert_eq!(answer["head"], rig.fix.as_str());
    assert_eq!(answer["fix_commit"], rig.fix.as_str());
    assert_eq!(answer["reply_id"], "reply-1");
    assert_eq!(answer["resolution_confirmed"], true);
    assert_eq!(probe.launched(), 1);
    assert!(probe.prompt().contains("Sound like a colleague."));
}

#[test]
fn with_no_voice_the_reply_goes_as_written_and_no_job_runs() {
    for row in [None, Some("none")] {
        let rig = ThreadRig::new();
        if let Some(row) = row {
            stand_in::name_voice(rig.repo.path(), row);
        }
        let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
        let (answer, posted) = respond_voiced(&rig, &readings(&rig, REPLY), &renderer);
        answer.expect("the reply");
        assert_eq!(posted, [REPLY], "{row:?}");
        assert_eq!(probe.launched(), 0, "{row:?}");
    }
}

#[test]
fn a_named_voice_that_cannot_be_applied_posts_nothing() {
    for (file, answers, reason) in [
        (false, Answers::Prefixed, "review-voice-file-missing"),
        (true, Answers::OneShort, "review-voice-answer-malformed"),
        (true, Answers::NotJson, "review-voice-answer-malformed"),
    ] {
        let rig = ThreadRig::new();
        stand_in::name_voice(rig.repo.path(), "house");
        if file {
            stand_in::write_voice("house", "Sound like a colleague.");
        }
        let (renderer, _) = stand_in::renderer(answers);
        let (answer, posted) = respond_voiced(&rig, &readings(&rig, REPLY), &renderer);
        assert_eq!(answer.expect_err(reason).envelope()["reason"], reason);
        assert!(posted.is_empty(), "{reason}: {posted:?}");
    }
}

#[test]
fn a_retried_reply_under_a_voice_is_one_reply_in_the_same_words() {
    let rig = ThreadRig::new();
    stand_in::write_voice("house", "Sound like a colleague.");
    stand_in::name_voice(rig.repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Counting);
    let first_words = "voiced #1: Fixed the concern.";
    let (first, posted) = respond_voiced(&rig, &readings(&rig, first_words), &renderer);
    first.expect("the first attempt");
    assert_eq!(posted, [first_words]);

    let already = rig.script(&[rig.pr_replied(Some(first_words), true)]);
    let (second, again) = respond_voiced(&rig, &already, &renderer);
    let second = second.expect("the retry");
    assert!(again.is_empty(), "a second reply was posted: {again:?}");
    assert_eq!(second["reply_id"], "reply-1");
    assert_eq!(probe.launched(), 1, "the retry rendered again");
}

#[test]
fn a_reply_posted_but_not_read_back_is_not_posted_again_after_the_voice_is_edited() {
    let rig = ThreadRig::new();
    stand_in::write_voice("house", "Sound like a colleague.");
    stand_in::name_voice(rig.repo.path(), "house");
    let (renderer, probe) = stand_in::renderer(Answers::Counting);
    let first_words = "voiced #1: Fixed the concern.";
    let unread = rig.script(&[rig.pr(false, false)]);
    let (first, posted) = respond_voiced(&rig, &unread, &renderer);
    let first = first.expect_err("the reply did not read back");
    assert_eq!(
        first.envelope()["reason"],
        "review-thread-write-unconfirmed"
    );
    assert_eq!(posted, [first_words]);

    stand_in::write_voice("house", "Now write formally.");
    let already = rig.script(&[rig.pr_replied(Some(first_words), true)]);
    let (second, again) = respond_voiced(&rig, &already, &renderer);
    let second = second.expect("the retry");
    assert!(again.is_empty(), "a second reply was posted: {again:?}");
    assert_eq!(second["reply_id"], "reply-1");
    assert_eq!(probe.launched(), 1, "the retry rendered again");
}
