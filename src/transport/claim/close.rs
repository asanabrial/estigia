//! Closed-issue renewal.

use super::{ReviewTransaction, ownership};
use crate::transport::{Context, Failure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnClose {
    pub pr: u64,
    pub epoch: String,
    /// The receipt head the merge's second parent proved delivered.
    pub head: String,
    pub delivering_sha: String,
}
pub(crate) fn match_own_close(
    receipt: &ReviewTransaction,
    closing_prs: &[u64],
    delivering_sha: &str,
    second_parent: &str,
    base_is_ancestor_of_first_parent: bool,
    pin: Option<u64>,
) -> Option<OwnClose> {
    if pin.is_some_and(|pr| pr != receipt.pr) {
        return None;
    }
    if !closing_prs.contains(&receipt.pr) {
        return None;
    }
    match_merge(
        receipt,
        delivering_sha,
        second_parent,
        base_is_ancestor_of_first_parent,
    )
}

fn match_merge(
    receipt: &ReviewTransaction,
    delivering_sha: &str,
    second_parent: &str,
    base_is_ancestor_of_first_parent: bool,
) -> Option<OwnClose> {
    if delivering_sha.len() != 40
        || second_parent != receipt.head
        || !base_is_ancestor_of_first_parent
    {
        return None;
    }
    Some(OwnClose {
        pr: receipt.pr,
        epoch: receipt.epoch.clone(),
        head: receipt.head.clone(),
        delivering_sha: delivering_sha.to_owned(),
    })
}

/// What `transition --to done` reads before it moves anything: the merge of
/// the latest confirmed publication's pull request, held to the same proof as
/// [`own_delivery_close`] without its closing-reference condition — a pull
/// request that only references the issue is delivered all the same.
///
/// `None` when the issue has no confirmed publication, so a `done` with no
/// receipt is not this check's to refuse. Otherwise the merge and its parents,
/// or a `delivery-not-verified` stop carrying them.
pub(crate) fn verified_delivery(
    context: &Context,
    comments: &[ownership::Comment],
) -> Result<Option<serde_json::Value>, Failure> {
    let Some(publication) = super::all_publications(comments)?
        .into_iter()
        .rfind(|p| p.confirmed)
    else {
        return Ok(None);
    };
    let receipt = publication.receipt;
    let not_verified = |cause: &str, detail: String, extra: serde_json::Value| {
        let mut body = serde_json::json!({
            "ok": false,
            "reason": "delivery-not-verified",
            "cause": cause,
            "pr": receipt.pr,
            "reviewed_head": receipt.head,
            "reviewed_base": receipt.base,
            "detail": detail,
            "action": "merge the reviewed head with a merge commit (not a squash or a rebase) \
                       and repeat the transition; `done` records a delivery, so it is not \
                       written for work whose merge does not join what was reviewed",
        });
        if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
            body.extend(extra.clone());
        }
        Failure::Stop(body)
    };
    let Some((sha, first, second)) = closing_merge(context, receipt.pr)? else {
        return Err(not_verified(
            "not-merged",
            format!("pull request #{} has no merge commit", receipt.pr),
            serde_json::json!({}),
        ));
    };
    let base_is_ancestor_of_first_parent =
        super::super::target::is_ancestor(&context.repo_dir, &receipt.base, &first)?;
    let merge = serde_json::json!({
        "pr": receipt.pr,
        "merge_commit": sha,
        "parents": [first, second],
    });
    if match_merge(&receipt, &sha, &second, base_is_ancestor_of_first_parent).is_none() {
        return Err(not_verified(
            merge_cause(&receipt, &sha, &second, base_is_ancestor_of_first_parent),
            format!(
                "the merge {sha} of pull request #{} does not join the reviewed head {}",
                receipt.pr, receipt.head
            ),
            merge,
        ));
    }
    Ok(Some(merge))
}

fn merge_cause(
    receipt: &ReviewTransaction,
    delivering_sha: &str,
    second_parent: &str,
    base_is_ancestor_of_first_parent: bool,
) -> &'static str {
    if delivering_sha.len() != 40 {
        "merge-commit-unreadable"
    } else if second_parent != receipt.head {
        "second-parent-is-not-the-reviewed-head"
    } else if !base_is_ancestor_of_first_parent {
        "reviewed-base-is-not-behind-the-first-parent"
    } else {
        "merge-commit-unreadable"
    }
}
pub(crate) fn own_delivery_close(
    context: &Context,
    issue: u64,
    run_id: &str,
    comments: &[ownership::Comment],
    pin: Option<u64>,
) -> Result<Option<OwnClose>, Failure> {
    let Some((receipt, closing_prs)) =
        own_publication_and_closers(context, issue, run_id, comments)?
    else {
        return Ok(None);
    };
    let receipt = &receipt;
    if pin.is_some_and(|pr| pr != receipt.pr) || !closing_prs.contains(&receipt.pr) {
        return Ok(None);
    }
    let Some((sha, first, second)) = closing_merge(context, receipt.pr)? else {
        return Ok(None);
    };
    let base_is_ancestor_of_first_parent =
        super::super::target::is_ancestor(&context.repo_dir, &receipt.base, &first)?;
    Ok(match_own_close(
        receipt,
        &closing_prs,
        &sha,
        &second,
        base_is_ancestor_of_first_parent,
        pin,
    ))
}

/// The run's own confirmed publication and the issue's closing pull requests,
/// when it has published at all — the shared head of [`own_delivery_close`] and
/// [`closed_by_own_publication`].
fn own_publication_and_closers(
    context: &Context,
    issue: u64,
    run_id: &str,
    comments: &[ownership::Comment],
) -> Result<Option<(ReviewTransaction, Vec<u64>)>, Failure> {
    let has_own = comments.iter().any(|c| {
        c.viewer_did_author
            && !c.includes_created_edit
            && super::super::markers::parse(&c.body).iter().any(|m| {
                m.get("kind").map(String::as_str) == Some("published")
                    && m.get("run-id").map(String::as_str) == Some(run_id)
            })
    });
    if !has_own {
        return Ok(None);
    }
    let closing = super::super::closing::closing_refs(context, issue)?;
    let closing_prs: Vec<u64> = closing
        .iter()
        .filter_map(|r| r.get("number").and_then(serde_json::Value::as_u64))
        .collect();
    let Some(publication) = super::all_publications(comments)?
        .into_iter()
        .rfind(|p| p.publisher == run_id && p.confirmed)
    else {
        return Ok(None);
    };
    Ok(Some((publication.receipt, closing_prs)))
}

/// The run's own confirmed publication, when it is one of the pull requests
/// that closed the issue — the weaker fact the `issue-not-open` refusal needs
/// where [`own_delivery_close`]'s merge proof does not match.
pub(crate) fn closed_by_own_publication(
    context: &Context,
    issue: u64,
    run_id: &str,
    comments: &[ownership::Comment],
) -> Result<Option<u64>, Failure> {
    let Some((receipt, closing_prs)) =
        own_publication_and_closers(context, issue, run_id, comments)?
    else {
        return Ok(None);
    };
    Ok(closing_prs.contains(&receipt.pr).then_some(receipt.pr))
}
fn closing_merge(context: &Context, pr: u64) -> Result<Option<(String, String, String)>, Failure> {
    let number = pr.to_string();
    let data = crate::transport::gh_json(
        &["pr", "view", &number, "--json", "mergeCommit"],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh pr view {pr} returned nothing")))?;
    let Some(sha) = data
        .get("mergeCommit")
        .and_then(|c| c.get("oid"))
        .and_then(serde_json::Value::as_str)
        .filter(|v| v.len() == 40 && v.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return Ok(None);
    };
    let (owner, name) = crate::transport::closing::repo_identity(context)?;
    let path = format!("repos/{owner}/{name}/commits/{sha}");
    let commit = crate::transport::gh_json(&["api", &path], Some(&context.repo_dir))?
        .ok_or_else(|| Failure::Read(format!("gh api {path} returned nothing")))?;
    let parents: Vec<String> = commit
        .get("parents")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Failure::Read(format!("gh api {path} omitted parents")))?
        .iter()
        .filter_map(|p| {
            p.get("sha")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    // Merge-commit topology: exactly two parents (the integration tip and the
    // reviewed head). Any other
    // shape is not this strategy — refuse naming it rather than returning the
    // generic "not my delivery" that stranded squash/rebase closes.
    if parents.len() != 2 {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "delivery-topology-mismatch",
            "strategy": "merge commit",
            "parents": parents.len(),
            "delivering_sha": sha,
            "action": "the delivered SHA is not a two-parent merge commit, the only topology \
                       delivery recognises; do not treat this as a failed claim read",
        })));
    }
    Ok(Some((
        sha.to_ascii_lowercase(),
        parents[0].to_ascii_lowercase(),
        parents[1].to_ascii_lowercase(),
    )))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> ReviewTransaction {
        ReviewTransaction {
            epoch: "a".repeat(32),
            pr: 42,
            head: "b".repeat(40),
            base: "c".repeat(40),
            digest: "d".repeat(64),
        }
    }

    fn sha() -> String {
        "e".repeat(40)
    }

    #[test]
    fn a_matching_own_merge_permits_the_follow_up() {
        let r = receipt();
        let found = match_own_close(&r, &[42], &sha(), &r.head, true, None)
            .expect("own delivery should permit");
        assert_eq!(found.pr, 42);
        assert_eq!(found.epoch, r.epoch);
        assert_eq!(found.head, r.head);
        assert_eq!(found.delivering_sha, sha());
    }

    #[test]
    fn a_different_run_pin_or_missing_closer_is_not_ours() {
        let r = receipt();
        assert!(match_own_close(&r, &[42], &sha(), &r.head, true, Some(7)).is_none());
        assert!(match_own_close(&r, &[7], &sha(), &r.head, true, None).is_none());
    }

    #[test]
    fn a_merge_whose_second_parent_is_not_the_receipt_head_is_not_ours() {
        let r = receipt();
        assert!(match_own_close(&r, &[42], &sha(), &r.base, true, None).is_none());
        assert!(match_own_close(&r, &[42], "short", &r.head, true, None).is_none());
    }

    #[test]
    fn a_delivery_whose_first_parent_descends_from_the_reviewed_base_is_recognised() {
        let r = receipt();
        let found = match_own_close(&r, &[42], &sha(), &r.head, true, None)
            .expect("a forward-integrated delivery is still this run's own");
        assert_eq!(found.delivering_sha, sha());
    }

    #[test]
    fn a_delivery_whose_first_parent_is_unrelated_to_the_reviewed_base_is_refused() {
        let r = receipt();
        assert!(match_own_close(&r, &[42], &sha(), &r.head, false, None).is_none());
    }

    /// A repository with `base`, a commit that descends from it, and a root
    /// commit that shares no history with either — the two shapes the delivered
    /// SHA's first parent can have against the reviewed base.
    fn repo_with_a_descendant_and_an_unrelated_commit()
    -> (tempfile::TempDir, String, String, String) {
        let root = tempfile::tempdir().expect("a repository");
        let at = root.path();
        let git = |args: &[&str]| {
            crate::transport::run(args, Some(at), crate::transport::How::write())
                .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
        };
        let oid = || {
            crate::transport::run(
                &["git", "rev-parse", "HEAD"],
                Some(at),
                crate::transport::How::read(),
            )
            .expect("HEAD")
            .stdout
            .trim()
            .to_owned()
        };
        git(&["git", "init", "--quiet", "-b", "main", "."]);
        git(&["git", "config", "user.email", "estigia@example.invalid"]);
        git(&["git", "config", "user.name", "Estigia test"]);
        std::fs::write(at.join("one.txt"), "one\n").expect("content");
        git(&["git", "add", "one.txt"]);
        git(&["git", "commit", "--quiet", "-m", "base"]);
        let base = oid();
        std::fs::write(at.join("two.txt"), "two\n").expect("content");
        git(&["git", "add", "two.txt"]);
        git(&["git", "commit", "--quiet", "-m", "descendant"]);
        let descendant = oid();
        git(&["git", "checkout", "--quiet", "--orphan", "unrelated"]);
        git(&[
            "git",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "unrelated",
        ]);
        let unrelated = oid();
        (root, base, descendant, unrelated)
    }

    fn own_close_for_first_parent(
        repo: &std::path::Path,
        base: &str,
        first_parent: &str,
    ) -> Option<OwnClose> {
        let run_id = "claude-abcd1234";
        let head = "b".repeat(40);
        let merge_sha = "e".repeat(40);
        let comments = vec![ownership::Comment {
            id: Some("IC_publication".to_owned()),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            body: crate::transport::markers::render(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &"a".repeat(32)),
                    ("pr", "42"),
                    ("head", &head),
                    ("base", base),
                    ("digest", &"d".repeat(64)),
                ],
            )
            .expect("a published marker"),
            viewer_did_author: true,
            includes_created_edit: false,
        }];
        let closing = serde_json::json!({
            "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                "nodes": [{ "number": 42, "state": "MERGED", "headRefName": "h", "baseRefName": "b" }],
                "pageInfo": { "hasNextPage": false, "endCursor": null },
            } } } },
        })
        .to_string();
        let parents = serde_json::json!({
            "parents": [{ "sha": first_parent }, { "sha": head }],
        })
        .to_string();
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[
            serde_json::json!({
                "matches": "repo view",
                "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "r" }).to_string(),
                "status": 0,
            }),
            serde_json::json!({ "matches": "api graphql", "stdout": closing, "status": 0 }),
            serde_json::json!({
                "matches": "pr view 42 --json mergeCommit",
                "stdout": serde_json::json!({ "mergeCommit": { "oid": merge_sha } }).to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": format!("api repos/o/r/commits/{merge_sha}"),
                "stdout": parents,
                "status": 0,
            }),
        ]);
        let context = Context {
            skill_dir: repo.to_path_buf(),
            repo_dir: repo.to_path_buf(),
            config: Vec::new(),
            repo: None,
        };
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            own_delivery_close(&context, 7, run_id, &comments, None)
                .expect("a readable tracker answers")
        })
    }

    #[test]
    fn a_forward_integrated_delivery_is_recognised_as_this_runs_own_close() {
        let (root, base, descendant, _unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        let found = own_close_for_first_parent(root.path(), &base, &descendant)
            .expect("a first parent that descends from the reviewed base is this run's delivery");
        assert_eq!(found.pr, 42);
        assert_eq!(found.head, "b".repeat(40));
    }

    #[test]
    fn a_delivery_whose_first_parent_is_unrelated_is_not_this_runs_own_close() {
        let (root, base, _descendant, unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        assert!(
            own_close_for_first_parent(root.path(), &base, &unrelated).is_none(),
            "a first parent sharing no history with the reviewed base was accepted as delivery"
        );
    }

    /// A renewal against an issue carrying this run's live claim and its
    /// `published` marker, closed or open. `closer` is the pull request the
    /// close is attributed to: `Some(42)` is the publication's own PR,
    /// `Some(99)` another PR, and `None` a close by hand with no closing
    /// reference at all.
    fn verify_claim_on_an_issue(
        repo: &std::path::Path,
        state: &str,
        base: &str,
        first_parent: &str,
        closer: Option<u64>,
    ) -> Result<serde_json::Value, Failure> {
        let run_id = "claude-abcd1234";
        let head = "b".repeat(40);
        let merge_sha = "e".repeat(40);
        let epoch = "a".repeat(32);
        let digest = "d".repeat(64);
        let claim_op = "c".repeat(32);
        let comment = |id: &str, at: &str, body: String| {
            serde_json::json!({
                "id": id,
                "createdAt": at,
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": body,
            })
        };
        let issue = serde_json::json!({
            "state": state,
            "labels": [{ "name": "status:in-progress" }],
            "comments": [
                comment(
                    "IC_claim",
                    "2026-01-01T00:00:00Z",
                    format!(
                        "Claimed by {run_id}.\n\n{}\n",
                        crate::transport::markers::render(
                            "claim",
                            &[
                                ("run-id", run_id),
                                ("runtime", "claude"),
                                ("horizon", "2099-01-01T00:00Z"),
                                ("op-id", &claim_op),
                            ],
                        )
                        .expect("a claim marker"),
                    ),
                ),
                comment(
                    "IC_publication",
                    "2026-01-01T01:00:00Z",
                    format!(
                        "published.\n\n{}\n",
                        crate::transport::markers::render(
                            "published",
                            &[
                                ("run-id", run_id),
                                ("epoch", &epoch),
                                ("pr", "42"),
                                ("head", &head),
                                ("base", base),
                                ("digest", &digest),
                            ],
                        )
                        .expect("a published marker"),
                    ),
                ),
            ],
        })
        .to_string();
        let nodes: Vec<serde_json::Value> = closer
            .map(|number| {
                vec![serde_json::json!({
                    "number": number,
                    "state": "MERGED",
                    "headRefName": "h",
                    "baseRefName": "b",
                })]
            })
            .unwrap_or_default();
        let closing = serde_json::json!({
            "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                "nodes": nodes,
                "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
            } } } },
        })
        .to_string();
        let parents = serde_json::json!({
            "parents": [{ "sha": first_parent }, { "sha": head }],
        })
        .to_string();
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[
            serde_json::json!({ "matches": "issue view", "stdout": issue, "status": 0 }),
            serde_json::json!({
                "matches": "repo view",
                "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "r" }).to_string(),
                "status": 0,
            }),
            serde_json::json!({ "matches": "api graphql", "stdout": closing, "status": 0 }),
            serde_json::json!({
                "matches": "pr view 42 --json mergeCommit",
                "stdout": serde_json::json!({ "mergeCommit": { "oid": merge_sha } }).to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": format!("api repos/o/r/commits/{merge_sha}"),
                "stdout": parents,
                "status": 0,
            }),
        ]);
        let context = Context {
            skill_dir: repo.to_path_buf(),
            repo_dir: repo.to_path_buf(),
            config: Vec::new(),
            repo: None,
        };
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::super::verify_claim(
                &context,
                7,
                run_id,
                "in-progress",
                "2026-01-01T02:00:00Z",
                None,
            )
        })
    }

    #[test]
    fn an_issue_closed_by_this_runs_own_publication_names_the_continuation() {
        let (root, base, _descendant, unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        // The merge's first parent shares no history with the reviewed base, so
        // `own_delivery_close` cannot prove the delivery — the shape that
        // stranded the run — while the closer is still this run's publication.
        let refused = verify_claim_on_an_issue(root.path(), "CLOSED", &base, &unrelated, Some(42))
            .expect_err("a close this run's own publication caused must still refuse the renewal");
        let Failure::Stop(body) = refused else {
            panic!("the own-delivery refusal was not a stop: {refused:?}");
        };
        assert_eq!(body["reason"], "issue-not-open", "{body}");
        let detail = body["detail"].as_str().expect("a detail");
        let action = body["action"].as_str().expect("an action");
        assert!(
            detail.contains("#7"),
            "the detail did not name the issue: {detail}"
        );
        assert!(
            action.contains("gh issue reopen 7"),
            "the refusal named no reopen that clears the block: {action}"
        );
        assert!(
            action.contains("transition"),
            "the refusal named no transition back into the state: {action}"
        );
        assert!(
            !detail.contains("not yours") && !action.contains("not yours"),
            "this run's own auto-close was refused as somebody else's: {detail} / {action}"
        );
    }

    #[test]
    fn an_issue_closed_by_a_stranger_keeps_the_not_yours_refusal() {
        let root = tempfile::tempdir().expect("a repository");
        // Another PR closed it, and a hand close left no closing reference at
        // all: neither is this run's publication, and neither may name a
        // command this run cannot use.
        for closer in [Some(99), None] {
            let refused = verify_claim_on_an_issue(
                root.path(),
                "CLOSED",
                &"c".repeat(40),
                &"e".repeat(40),
                closer,
            )
            .expect_err("a close this run did not cause must refuse the renewal");
            let Failure::Stop(body) = refused else {
                panic!("the stranger refusal was not a stop: {refused:?}");
            };
            assert_eq!(body["reason"], "issue-not-open", "{body}");
            let detail = body["detail"].as_str().expect("a detail");
            let action = body["action"].as_str().expect("an action");
            assert!(
                detail.contains("someone delivered or killed it"),
                "the stranger close no longer says who closed it: {detail}"
            );
            assert!(
                action.contains("it is not yours"),
                "the stranger close no longer keeps the not-yours advice: {action}"
            );
            assert!(
                !action.contains("gh issue reopen"),
                "a close this run did not cause named a reopen it cannot use: {action}"
            );
        }
    }

    #[test]
    fn reopening_and_transitioning_back_clears_the_refusal() {
        // What the refusal names is the state this asserts: once `gh issue
        // reopen` has run (the issue is OPEN) and `transition` has put the
        // label back, the same renewal passes — the commands clear the block
        // rather than naming a dead end.
        let root = tempfile::tempdir().expect("a repository");
        let answer = verify_claim_on_an_issue(
            root.path(),
            "OPEN",
            &"c".repeat(40),
            &"e".repeat(40),
            Some(42),
        )
        .expect("an open, labelled issue this run holds renews");
        assert_eq!(answer["ok"], true, "{answer}");
        assert_eq!(answer["checked"][0], "issue-open", "{answer}");
    }

    #[test]
    fn transition_does_not_refuse_a_closed_issue() {
        let source = crate::source_text::code_of(include_str!("../commands.rs"));
        let start = source
            .find("pub fn transition(")
            .expect("transition exists");
        let body = &source[start..start + 2500];
        assert!(
            !body.contains("issue-not-open") && !body.contains("state != \"OPEN\""),
            "transition grew a closed-issue stand-down, so it no longer agrees with verify_claim"
        );
    }

    struct DoneRun {
        answer: Result<serde_json::Value, Failure>,
        log: String,
    }

    fn done_on_an_issue(
        repo: &std::path::Path,
        base: &str,
        merge: Option<(&str, &str)>,
        published: bool,
    ) -> DoneRun {
        done_run(repo, base, merge, published, false, None)
    }

    fn done_run(
        repo: &std::path::Path,
        base: &str,
        merge: Option<(&str, &str)>,
        published: bool,
        closed: bool,
        unreadable: Option<&str>,
    ) -> DoneRun {
        let run_id = "claude-abcd1234";
        let head = "b".repeat(40);
        let merge_sha = "e".repeat(40);
        let mut comments = vec![serde_json::json!({
            "id": "IC_claim",
            "createdAt": "2026-01-01T00:00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!(
                "Claimed by {run_id}.\n\n{}\n",
                crate::transport::markers::render(
                    "claim",
                    &[
                        ("run-id", run_id),
                        ("runtime", "claude"),
                        ("horizon", "2099-01-01T00:00Z"),
                        ("op-id", &"c".repeat(32)),
                    ],
                )
                .expect("a claim marker"),
            ),
        })];
        if published {
            comments.push(serde_json::json!({
                "id": "IC_publication",
                "createdAt": "2026-01-01T01:00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!(
                    "published.\n\n{}\n",
                    crate::transport::markers::render(
                        "published",
                        &[
                            ("run-id", run_id),
                            ("epoch", &"a".repeat(32)),
                            ("pr", "42"),
                            ("head", &head),
                            ("base", base),
                            ("digest", &"d".repeat(64)),
                        ],
                    )
                    .expect("a published marker"),
                ),
            }));
        }
        let issue = serde_json::json!({
            "state": "OPEN",
            "labels": [{ "name": "status:review" }],
            "comments": comments,
        })
        .to_string();
        let merge_commit = match merge {
            Some(_) => serde_json::json!({ "mergeCommit": { "oid": merge_sha } }),
            None => serde_json::json!({ "mergeCommit": null }),
        };
        let (first, second) = merge.unwrap_or(("", ""));
        let parents = serde_json::json!({
            "parents": [{ "sha": first }, { "sha": second }],
        })
        .to_string();
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[
            serde_json::json!({
                "matches": "issue view 7 --json labels",
                "stdout": serde_json::json!({ "labels": [{ "name": "status:done" }] }).to_string(),
                "status": 0,
            }),
            serde_json::json!({ "matches": "issue view", "stdout": issue, "status": 0 }),
            serde_json::json!({
                "matches": "repo view",
                "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "r" }).to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": "pr view 42 --json mergeCommit",
                "stdout": merge_commit.to_string(),
                "stderr": "boom",
                "status": i32::from(unreadable == Some("pr")),
            }),
            serde_json::json!({
                "matches": format!("api repos/o/r/commits/{merge_sha}"),
                "stdout": parents,
                "stderr": "boom",
                "status": i32::from(unreadable == Some("commit")),
            }),
            serde_json::json!({ "matches": "issue edit", "status": 0 }),
        ]);
        let context = Context {
            skill_dir: repo.to_path_buf(),
            repo_dir: repo.to_path_buf(),
            config: if closed {
                vec![(
                    "Git operation window".to_owned(),
                    "enabled 09:00-17:00".to_owned(),
                    crate::transport::Source::Written,
                )]
            } else {
                Vec::new()
            },
            repo: closed.then(|| "fixture/repo".to_owned()),
        };
        let log_dir = tempfile::tempdir().expect("a log directory");
        let log = log_dir.path().join("gh.log");
        let answer = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            crate::harness::operation_window::with_now_override(Some(|| Some(20 * 60)), || {
                crate::transport::commands::transition(
                    &context,
                    7,
                    "done",
                    Some("review"),
                    "2026-01-01T02:00:00Z",
                )
            })
        });
        DoneRun {
            answer,
            log: std::fs::read_to_string(&log).unwrap_or_default(),
        }
    }

    fn refusal_reason(run: &DoneRun) -> serde_json::Value {
        let Err(Failure::Stop(body)) = &run.answer else {
            panic!("done was not refused as a stop: {:?}", run.answer);
        };
        body.clone()
    }

    #[test]
    fn done_is_refused_when_the_merge_does_not_join_the_reviewed_head() {
        let (root, base, descendant, _unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        let other = "f".repeat(40);
        let run = done_on_an_issue(root.path(), &base, Some((&descendant, &other)), true);
        let body = refusal_reason(&run);
        assert_eq!(body["reason"], "delivery-not-verified", "{body}");
        assert_eq!(body["merge_commit"], "e".repeat(40), "{body}");
        assert_eq!(
            body["parents"],
            serde_json::json!([descendant, other]),
            "{body}"
        );
        assert!(
            !run.log.contains("issue edit"),
            "the label moved: {}",
            run.log
        );
    }

    #[test]
    fn done_is_refused_when_the_pull_request_is_not_merged() {
        let (root, base, _descendant, _unrelated) =
            repo_with_a_descendant_and_an_unrelated_commit();
        let run = done_on_an_issue(root.path(), &base, None, true);
        let body = refusal_reason(&run);
        assert_eq!(body["reason"], "delivery-not-verified", "{body}");
        assert!(
            body["detail"].as_str().expect("a detail").contains("#42"),
            "{body}"
        );
        assert!(
            !run.log.contains("issue edit"),
            "the label moved: {}",
            run.log
        );
    }

    #[test]
    fn done_is_refused_when_the_reviewed_base_is_not_behind_the_first_parent() {
        let (root, base, _descendant, unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        let head = "b".repeat(40);
        let run = done_on_an_issue(root.path(), &base, Some((&unrelated, &head)), true);
        let body = refusal_reason(&run);
        assert_eq!(body["reason"], "delivery-not-verified", "{body}");
        assert!(
            !run.log.contains("issue edit"),
            "the label moved: {}",
            run.log
        );
    }

    #[test]
    fn done_is_accepted_and_reports_the_merge_when_it_joins_the_reviewed_head() {
        let (root, base, descendant, _unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        let head = "b".repeat(40);
        let run = done_on_an_issue(root.path(), &base, Some((&descendant, &head)), true);
        let answer = run
            .answer
            .expect("a delivery that joins the reviewed head is done");
        assert_eq!(answer["to"], "done", "{answer}");
        assert_eq!(
            answer["delivery"]["merge_commit"],
            "e".repeat(40),
            "{answer}"
        );
        assert_eq!(
            answer["delivery"]["parents"],
            serde_json::json!([descendant, head]),
            "{answer}"
        );
        assert_eq!(answer["delivery"]["pr"], 42, "{answer}");
    }

    #[test]
    fn done_with_no_receipt_reads_no_merge_and_behaves_as_before() {
        let (root, base, _descendant, _unrelated) =
            repo_with_a_descendant_and_an_unrelated_commit();
        let run = done_on_an_issue(root.path(), &base, None, false);
        let answer = run
            .answer
            .expect("a transition with no receipt is not checked");
        assert_eq!(answer["to"], "done", "{answer}");
        assert!(answer.get("delivery").is_none(), "{answer}");
        assert!(
            !run.log.contains("pr view"),
            "a merge was read: {}",
            run.log
        );
    }

    fn deferred_state_and_queue() -> (Option<String>, usize) {
        let repo = crate::transport::store::Repo {
            host: "github.com".to_owned(),
            owner: "fixture".to_owned(),
            name: "repo".to_owned(),
        };
        let store = crate::transport::store::Store::open(
            &crate::transport::store::store_path().expect("a store path"),
        )
        .expect("opens");
        (
            store.issue_state(&repo, 7).expect("reads"),
            store.outbox(&repo).expect("reads").len(),
        )
    }

    #[test]
    fn done_is_refused_while_the_window_is_closed_and_defers_nothing() {
        let (root, base, descendant, _unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        let other = "f".repeat(40);
        let run = done_run(
            root.path(),
            &base,
            Some((&descendant, &other)),
            true,
            true,
            None,
        );
        let body = refusal_reason(&run);
        assert_eq!(body["reason"], "delivery-not-verified", "{body}");
        let (state, queued) = deferred_state_and_queue();
        assert_eq!(state.as_deref(), Some("review"), "the refusal was recorded");
        assert_eq!(queued, 0, "the refusal queued a write");
    }

    #[test]
    fn done_deferred_while_the_window_is_closed_carries_the_delivery() {
        let (root, base, descendant, _unrelated) = repo_with_a_descendant_and_an_unrelated_commit();
        let head = "b".repeat(40);
        let run = done_run(
            root.path(),
            &base,
            Some((&descendant, &head)),
            true,
            true,
            None,
        );
        let answer = run.answer.expect("a verified delivery is deferred");
        assert_eq!(answer["projection"], "deferred", "{answer}");
        assert_eq!(answer["delivery"]["merge_commit"], "e".repeat(40));
        let (state, queued) = deferred_state_and_queue();
        assert_eq!(state.as_deref(), Some("done"));
        assert_eq!(queued, 1);
    }

    #[test]
    fn done_is_refused_naming_the_read_that_failed() {
        for (unreadable, named) in [("pr", "pr view"), ("commit", "commits/")] {
            let (root, base, descendant, _unrelated) =
                repo_with_a_descendant_and_an_unrelated_commit();
            let head = "b".repeat(40);
            let run = done_run(
                root.path(),
                &base,
                Some((&descendant, &head)),
                true,
                false,
                Some(unreadable),
            );
            let Err(Failure::Read(message)) = &run.answer else {
                panic!(
                    "{unreadable}: done was not refused as a read: {:?}",
                    run.answer
                );
            };
            assert!(message.contains(named), "{unreadable}: {message}");
            assert!(!run.log.contains("issue edit"), "{unreadable}: {}", run.log);
            assert_eq!(deferred_state_and_queue().0.as_deref(), Some("review"));
        }
    }

    #[test]
    fn a_malformed_merge_sha_is_named_as_unreadable_and_not_as_a_base_mismatch() {
        let receipt = receipt();
        let verdict = merge_cause(&receipt, &"e".repeat(39), &receipt.head, true);
        assert_eq!(verdict, "merge-commit-unreadable");
    }
}
