use super::*;

/// A GraphQL answer that carries errors is not a clean read.
///
/// Measured by mutation: making `carries_something` answer `false` for every
/// value left the whole suite green. GitHub returns `200 OK` with
/// `{"data": {…partial…}, "errors": [...]}` — that **is** the shape of a partial
/// failure — so with the check gone the reader walks straight into the partial
/// data and answers as though the read had succeeded.
///
/// What it answers is which pull requests close an issue on merge, which is a
/// delivery decision. Fewer references than exist reads as *nothing closes it*,
/// and that is the rule this whole crate is written around: an unknown result
/// is not clearance.
///
/// The other direction was already crossed and is the reason this function
/// exists — `"errors": []` is present and empty, and empty is not an error.
#[test]
fn an_answer_carrying_errors_is_not_read_as_a_clean_one() {
    // What the check is handed, in the four shapes GitHub sends.
    for carried in [
        serde_json::json!([{"message": "Something went wrong"}]),
        serde_json::json!({"message": "Something went wrong"}),
        serde_json::json!("Something went wrong"),
        serde_json::json!(true),
    ] {
        assert!(
            super::carries_something(&carried),
            "an answer carrying {carried} was read as carrying nothing"
        );
    }

    // And the floor, which is the case this function was written for: present
    // and empty is not an error, or every partial read would refuse.
    for empty in [
        serde_json::json!([]),
        serde_json::json!({}),
        serde_json::json!(""),
        serde_json::json!(null),
        serde_json::json!(false),
        serde_json::json!(0),
    ] {
        assert!(
            !super::carries_something(&empty),
            "{empty} is empty and was read as an error"
        );
    }
}

#[test]
fn a_closing_keyword_naming_this_issue_is_found_in_every_spelling() {
    for text in [
        "Closes #12",
        "closed:#12",
        "Fixes   :   #12",
        "fix #12",
        "Resolved #12",
        "this resolves https://github.com/acme/repo/issues/12 today",
    ] {
        assert_eq!(
            keywords_naming(text, 12).len(),
            1,
            "{text:?} names issue 12 and was not found"
        );
    }
}

#[test]
fn a_reference_that_is_not_a_closing_keyword_is_left_alone() {
    // The live case this exists for: issue #118's body opened with
    // `Refs #118 — deliberately NOT a closing keyword` while the reference was
    // live anyway. Reporting that as "fix the PR body" sends a run to edit prose
    // that never contained the keyword.
    for text in [
        "Refs #12",
        "See #12",
        "unfixed #12",
        "prefixes #12",
        "closes #13",
        "closes https://gitlab.com/acme/repo/issues/12",
        "closes",
        "closes #",
    ] {
        assert!(
            keywords_naming(text, 12).is_empty(),
            "{text:?} was read as a closing keyword for 12"
        );
    }
}

#[test]
fn the_number_must_be_the_whole_number() {
    // `#12` must not match `#120`, which a prefix comparison does.
    assert!(keywords_naming("closes #120", 12).is_empty());
    assert_eq!(keywords_naming("closes #120", 120).len(), 1);
}

#[test]
fn every_occurrence_is_reported_as_written() {
    // Reported in the author's own casing, because a person has to find it in
    // their own prose.
    let text = "Closes #12 and later FIXES #12 again";
    let found = keywords_naming(text, 12);
    assert_eq!(found, vec!["Closes #12".to_owned(), "FIXES #12".to_owned()]);
}

/// Every spelling GitHub closes on is one this scan sees.
///
/// The tool's own description says why it matters: *"an auto-close skips the
/// `done` transition the contract makes mandatory"*. So a spelling this does
/// not know is a merge that closes an issue while the check reports the branch
/// clear — and GitHub documents three, of which this read one and a half.
#[test]
fn the_three_spellings_github_documents_are_three_this_scan_reads() {
    for text in [
        "Fixes #42",
        "Closes: #42",
        "resolved   #42",
        "Fixes https://github.com/owner/repo/issues/42",
        // The two that went past.
        "Fixes GH-42",
        "fixes gh-42",
        "Fixes owner/repo#42",
        "Closes octo-org/octo-repo#42",
    ] {
        assert!(
            !super::keywords_naming(text, 42).is_empty(),
            "{text:?} closes issue 42 on merge and the scan reports the branch clear"
        );
    }

    // And what does not close it is still not reported. A scan that warned
    // about everything is one nobody reads.
    for text in [
        // A longer number that merely starts with the same digits.
        "Fixes #4242",
        // A word that merely ends in a keyword.
        "prefixes #42",
        // A keyword with no reference after it at all.
        "This fixes the parser",
        // Someone else's repository, named as an issue nobody here has.
        "Fixes owner/repo#7",
    ] {
        assert_eq!(
            super::keywords_naming(text, 42),
            Vec::<String>::new(),
            "{text:?} does not close issue 42 and was reported as though it did"
        );
    }

    // GitHub does not understand negation either, so neither does this: the
    // line really is there, and a person reading the warning can see it is.
    assert_eq!(
        super::keywords_naming("This does not fix #42", 42),
        vec!["fix #42".to_owned()]
    );
}

/// A `git log` that did not answer is not a branch with no keywords.
///
/// This scan was written twice, and the two copies disagreed about exactly
/// that: `publish_review`'s refused, `assess_autoclose`'s tolerated the failure
/// and carried on with an empty list. Tolerating it turns an unread source into
/// *no keyword found*, and a keyword nobody read is how an issue auto-closes
/// behind the workflow's back — the thing the refusal exists to prevent.
///
/// Unified into one function refusing on both sides, and the strictness is the
/// whole point of the unification, so it is measured rather than asserted: the
/// documentation says so in three places and nothing held it.
#[test]
fn an_unreadable_commit_range_is_not_a_range_without_keywords() {
    let nowhere = tempfile::tempdir().expect("a directory that is not a checkout");
    let failed = super::keywords_in_commits(nowhere.path(), "main", "some-branch", 42)
        .expect_err("a `git log` outside any repository was read as an answer");
    assert!(
        !matches!(failed, super::Failure::Stop(_)),
        "an unreadable range answered as a decision rather than a failed read"
    );
}
