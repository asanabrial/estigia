use super::*;
use crate::transport::voice::stand_in::{self, Answers};

fn mixed_script(rig: &Rig, listed: &[&str]) -> Vec<serde_json::Value> {
    vec![
        view(rig, &rig.head, "OPEN", "main"),
        comments_on(41, listed),
        no_reviews(),
        posted(41),
        read_back(41, &rig.head, "CHANGES_REQUESTED"),
    ]
}

fn voiced_review(rig: &Rig, name: &str) {
    stand_in::write_voice(name, "Write like a colleague, briefly.");
    stand_in::name_voice(&rig.context.repo_dir, name);
}

fn requests(rig: &Rig) -> Vec<serde_json::Value> {
    log(rig)
        .lines()
        .filter(|line| line.contains("--method POST"))
        .map(|line| {
            let (_, rest) = line.split_once("--input ").expect("sent with --input");
            let path = rest.split('\t').next().unwrap_or_default().trim();
            let document = std::fs::read_to_string(path).expect("the staged request");
            serde_json::from_str(&document).expect("a JSON request")
        })
        .collect()
}

const PLAIN_COMMENT: &str = "change.txt:1 does the wrong thing the wrong thing happens";

#[test]
fn a_named_voice_rewrites_each_comment_and_the_body_and_moves_nothing_else() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    voiced_review(&rig, "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let answer = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect("the submission");
    assert_eq!(answer["submitted"], true, "{answer}");
    let sent = request(&rig);
    assert_eq!(sent["commit_id"], rig.head.as_str());
    assert_eq!(sent["event"], "REQUEST_CHANGES");
    assert_eq!(
        sent["comments"],
        serde_json::json!([{
            "path": "change.txt",
            "line": 1,
            "side": "RIGHT",
            "body": format!("voiced: {PLAIN_COMMENT}"),
        }])
    );
    let body = sent["body"].as_str().expect("a body");
    assert!(body.starts_with("voiced: "), "{body}");
    assert!(body.contains("readme.txt:1"), "{body}");
    assert!(body.contains("change.txt:1-2"), "{body}");
    assert!(body.contains("1 posted inline"), "{body}");
    assert_eq!(probe.launched(), 1, "one job renders every text");
    let prompt = probe.prompt();
    assert!(
        prompt.contains("Write like a colleague, briefly."),
        "{prompt}"
    );
    assert!(prompt.contains(PLAIN_COMMENT), "{prompt}");
    for withheld in [rig.head.as_str(), "REQUEST_CHANGES", "\"side\"", "severe"] {
        assert!(!prompt.contains(withheld), "the job was handed {withheld}");
    }
    gone(&rig, &worktree);
}

#[test]
fn with_no_voice_the_request_is_the_one_sent_without_the_feature() {
    for row in [None, Some("none")] {
        let rig = rig();
        let worktree = reviewed(&rig, &mixed_answer());
        if let Some(row) = row {
            stand_in::name_voice(&rig.context.repo_dir, row);
        }
        let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
        gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
            submit_with(&rig.context, RUN, &renderer)
        })
        .expect("the submission");
        let sent = request(&rig);
        assert_eq!(
            sent["comments"],
            serde_json::json!([{
                "path": "change.txt",
                "line": 1,
                "side": "RIGHT",
                "body": PLAIN_COMMENT,
            }]),
            "{row:?}"
        );
        let body = sent["body"].as_str().expect("a body");
        assert!(
            body.starts_with("1 posted inline on the diff.\n\n"),
            "{body}"
        );
        assert_eq!(probe.launched(), 0, "{row:?}");
        gone(&rig, &worktree);
    }
}

#[test]
fn a_named_voice_whose_file_is_missing_stops_the_submission_naming_the_file() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    stand_in::name_voice(&rig.context.repo_dir, "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect_err("no voice file");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "review-voice-file-missing");
    assert!(
        envelope["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("house.md")),
        "{envelope}"
    );
    assert!(!log(&rig).contains("--method POST"), "something was posted");
    assert_eq!(probe.launched(), 0);
    assert!(worktree.is_dir(), "the claim was put down");
}

#[test]
fn a_voice_file_over_the_cap_is_refused_naming_its_size() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    stand_in::write_voice("house", &"x".repeat(16 * 1024 + 1));
    stand_in::name_voice(&rig.context.repo_dir, "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let failure = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect_err("a voice over the cap");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "review-voice-file-too-large");
    let detail = envelope["detail"].as_str().expect("a detail");
    assert!(
        detail.contains("house.md") && detail.contains("16385"),
        "{detail}"
    );
    assert!(!log(&rig).contains("--method POST"));
    assert_eq!(probe.launched(), 0);
    assert!(worktree.is_dir());
}

#[test]
fn a_voice_file_at_the_cap_is_read() {
    let rig = rig();
    reviewed(&rig, &mixed_answer());
    stand_in::write_voice("house", &"x".repeat(16 * 1024));
    stand_in::name_voice(&rig.context.repo_dir, "house");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect("the submission");
    assert_eq!(probe.launched(), 1);
}

#[test]
fn an_answer_that_is_not_one_text_per_input_posts_nothing() {
    for answers in [Answers::OneShort, Answers::NotJson, Answers::Blank] {
        let rig = rig();
        let worktree = reviewed(&rig, &mixed_answer());
        voiced_review(&rig, "house");
        let (renderer, _) = stand_in::renderer(answers);
        let failure = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
            submit_with(&rig.context, RUN, &renderer)
        })
        .expect_err("a refused rendering");
        assert_eq!(
            failure.envelope()["reason"],
            "review-voice-answer-malformed"
        );
        assert!(!log(&rig).contains("--method POST"));
        assert!(worktree.is_dir(), "the claim was put down");
    }
}

#[test]
fn a_rendering_job_that_refuses_is_a_failed_rendering_and_posts_nothing() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    voiced_review(&rig, "house");
    let renderer = stand_in::renderer_that_refuses();
    let failure = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect_err("a refused job");
    assert_eq!(failure.envelope()["reason"], "review-voice-render-failed");
    assert!(!log(&rig).contains("--method POST"));
    assert!(worktree.is_dir());
}

#[test]
fn a_retried_submission_posts_the_words_rendered_the_first_time() {
    let rig = rig();
    reviewed(&rig, &mixed_answer());
    voiced_review(&rig, "house");
    let (renderer, probe) = stand_in::renderer(Answers::Counting);
    let lost = gh(&rig, &mixed_script(&rig, &[]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect_err("the comments did not read back");
    assert_eq!(
        lost.envelope()["reason"],
        "pull-review-submission-unconfirmed"
    );

    let retried = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect("the retry");
    assert_eq!(retried["submitted"], true, "{retried}");
    let [first, second] = requests(&rig)
        .try_into()
        .expect("two posts, one per attempt");
    assert_eq!(second["comments"], first["comments"]);
    assert_eq!(second["body"], first["body"]);
    assert!(
        second["comments"][0]["body"]
            .as_str()
            .is_some_and(|text| text.starts_with("voiced #1: ")),
        "{second}"
    );
    assert_eq!(probe.launched(), 1, "the retry rendered again");
}

#[test]
fn another_voice_file_is_another_rendering() {
    let rig = rig();
    reviewed(&rig, &mixed_answer());
    voiced_review(&rig, "house");
    let (renderer, probe) = stand_in::renderer(Answers::Counting);
    gh(&rig, &mixed_script(&rig, &[]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect_err("the comments did not read back");
    stand_in::write_voice("house", "Now write formally.");
    gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect("the retry");
    assert_eq!(probe.launched(), 2);
    let [first, second] = requests(&rig)
        .try_into()
        .expect("two posts, one per attempt");
    for (sent, launch) in [(first, 1), (second, 2)] {
        assert!(
            sent["comments"][0]["body"]
                .as_str()
                .is_some_and(|text| text.starts_with(&format!("voiced #{launch}: "))),
            "{sent}"
        );
    }
}

fn write_repository_rows(rig: &Rig, rows: &str) {
    let path = crate::skill::repository_config_path(&rig.context.repo_dir);
    std::fs::create_dir_all(path.parent().expect("a directory")).expect("a state directory");
    crate::paths::replace_atomically(
        &path,
        &format!("| Setting | Value here |\n|---|---|\n{rows}"),
    )
    .expect("a repository file");
}

#[test]
fn a_malformed_row_for_another_setting_does_not_stop_a_review_that_names_no_voice() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    write_repository_rows(&rig, "| Change size | many |\n");
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let answer = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect("the submission");
    assert_eq!(answer["submitted"], true, "{answer}");
    assert_eq!(request(&rig)["comments"][0]["body"], PLAIN_COMMENT);
    assert_eq!(probe.launched(), 0);
    gone(&rig, &worktree);
}

#[test]
fn a_malformed_voice_row_still_stops_the_submission_beside_a_good_one() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    write_repository_rows(
        &rig,
        "| Change size | 400 |\n| Review voice | ../outside |\n",
    );
    let (renderer, _) = stand_in::renderer(Answers::Prefixed);
    let failure = gh(&rig, &mixed_script(&rig, &["change.txt"]), || {
        submit_with(&rig.context, RUN, &renderer)
    })
    .expect_err("an unreadable voice row");
    assert_eq!(failure.envelope()["reason"], "review-voice-row-unreadable");
    assert!(!log(&rig).contains("--method POST"));
    assert!(worktree.is_dir());
}

#[test]
fn a_landed_review_is_answered_for_without_a_voice_being_asked() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    stand_in::name_voice(&rig.context.repo_dir, "house");
    let existing = serde_json::json!([[{
        "id": 41,
        "commit_id": rig.head,
        "user": { "login": "the-reviewer" },
        "body": "",
    }]]);
    let (renderer, probe) = stand_in::renderer(Answers::Prefixed);
    let answer = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            comments_on(41, &["change.txt"]),
            serde_json::json!({
                "matches": "--paginate --slurp",
                "stdout": existing.to_string(),
                "status": 0,
            }),
            read_back(41, &rig.head, "CHANGES_REQUESTED"),
        ],
        || submit_with(&rig.context, RUN, &renderer),
    )
    .expect("the retry");
    assert_eq!(answer["submitted"], true, "{answer}");
    assert!(!log(&rig).contains("--method POST"));
    assert_eq!(probe.launched(), 0);
    gone(&rig, &worktree);
}
