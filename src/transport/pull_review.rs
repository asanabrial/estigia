//! A review-only claim on a pull request this run did not build.
//!
//! The claim is a `pull_reviews` row keyed on the pull request and a detached
//! worktree this module creates and removes. It never touches `Run::issue` or
//! the issue ledger, and nothing it records is evidence for a merge.
//! `docs/assisted-review.md` has the road.

use super::claim::repo_of;
use super::claim::{require_horizon, review_operation_id};
use super::store::{PullReview, Store, store_path, viewer_login};
use super::target::{is_object_id, merge_base};
use super::{Context, Failure, How, run};
use crate::identity::Identity;

/// What `pr_review_claim` was asked for.
#[derive(Debug, Clone, Copy)]
pub struct ClaimRequest<'a> {
    /// The pull request to review.
    pub pr: u64,
    /// The run taking the claim.
    pub run_id: &'a str,
    /// When the run expects to report; past it another run may take the claim.
    pub horizon: &'a str,
    /// Overrides the configured `Worktree location`.
    pub worktree_root: Option<&'a str>,
    /// The current stamp.
    pub now: &'a str,
}

struct Pull {
    head: String,
    head_ref: String,
    base_ref: String,
    base_tip: String,
}

fn stop(reason: &str, detail: String, action: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": reason,
        "detail": detail,
        "action": action,
    }))
}

fn head_moved(pr: u64, detail: String) -> Failure {
    stop(
        "head-moved",
        format!("pull request #{pr}: {detail}"),
        "nothing was reviewed against these bytes; `pr_review_release`, then a new \
         `pr_review_claim` reads the head as it stands",
    )
}

fn read_pull(context: &Context, pr: u64) -> Result<Pull, Failure> {
    let number = pr.to_string();
    let data = super::gh_json(
        &[
            "pr",
            "view",
            &number,
            "--json",
            "state,isCrossRepository,author,headRefOid,headRefName,baseRefName,baseRefOid",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh pr view {pr} returned nothing")))?;
    let text = |key: &str| {
        data.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let state = text("state");
    if state != "OPEN" {
        return Err(stop(
            "pull-review-not-open",
            format!("pull request #{pr} is {state:?}, not OPEN"),
            "stop; nothing was created, and there is nothing to review",
        ));
    }
    if data.get("isCrossRepository") != Some(&serde_json::Value::Bool(false)) {
        return Err(stop(
            "pull-review-fork-refused",
            format!("pull request #{pr} is not read as coming from this repository"),
            "a person reviews a fork's pull request; this claim takes branches of this \
             repository only",
        ));
    }
    let author = data
        .pointer("/author/login")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if author.is_empty() {
        return Err(Failure::Read(format!(
            "gh pr view {pr} named no author, so whose pull request it is cannot be told"
        )));
    }
    if author == viewer_login(context)? {
        return Err(stop(
            "pull-review-own-pull-request",
            format!("pull request #{pr} was opened by this account"),
            "someone else reviews it; GitHub refuses an author's own approval",
        ));
    }
    let pull = Pull {
        head: text("headRefOid"),
        head_ref: text("headRefName"),
        base_ref: text("baseRefName"),
        base_tip: text("baseRefOid"),
    };
    if !is_object_id(&pull.head)
        || !is_object_id(&pull.base_tip)
        || pull.base_ref.is_empty()
        || pull.head_ref.is_empty()
    {
        return Err(Failure::Read(format!(
            "gh pr view {pr} did not name a head, its branch, a base branch and a base commit"
        )));
    }
    Ok(pull)
}

fn fetch(context: &Context, pr: u64, pull: &Pull) -> Result<String, Failure> {
    let pull_ref = format!("refs/pull/{pr}/head");
    let tracking = |branch: &str| format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
    run(
        &[
            "git",
            "fetch",
            "--no-tags",
            "origin",
            &pull_ref,
            &tracking(&pull.head_ref),
            &tracking(&pull.base_ref),
        ],
        Some(&context.repo_dir),
        How::write(),
    )?;
    let commit = format!("{}^{{commit}}", pull.head);
    let held = run(
        &["git", "cat-file", "-e", &commit],
        Some(&context.repo_dir),
        How::tolerated(),
    )?;
    if held.status != 0 {
        return Err(head_moved(
            pr,
            format!(
                "{} was read as the head and the fetch did not bring it",
                pull.head
            ),
        ));
    }
    merge_base(&context.repo_dir, &pull.base_tip, &pull.head)
}

fn worktree_for(
    request: &ClaimRequest<'_>,
    repo_name: &str,
    configured: Option<&str>,
) -> Result<std::path::PathBuf, Failure> {
    let template = request.worktree_root.or(configured).unwrap_or_default();
    if template.is_empty() || template.eq_ignore_ascii_case("unset") {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "no-worktree-location",
            "action": super::branch::no_worktree_location_action(),
        })));
    }
    super::branch::validate_worktree_override(request.worktree_root)?;
    let (scoped, _) = super::worktree::scoped_template(template);
    let resolved = super::worktree::worktree_path(
        &scoped,
        repo_name,
        &format!("review-pr-{}", request.pr),
        request.run_id,
        &Identity::Number(request.pr),
    )?;
    super::branch::canonical_worktree_path(&resolved)
}

// The hooks are switched off because a repository whose `core.hooksPath` is
// relative would otherwise run a `post-checkout` taken from the reviewed bytes.
fn add_detached(context: &Context, worktree: &str, head: &str) -> Result<(), Failure> {
    run(
        &[
            "git",
            "-c",
            "core.hooksPath=/dev/null",
            "worktree",
            "add",
            "--detach",
            worktree,
            head,
        ],
        Some(&context.repo_dir),
        How::write(),
    )
    .map(|_| ())
}

fn answer(held: &PullReview, resumed: bool) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "pr": held.pr,
        "run_id": held.run_id,
        "epoch": held.epoch,
        "head": held.head,
        "base_ref": held.base_ref,
        "base_tip": held.base_tip,
        "merge_base": held.merge_base,
        "worktree": held.worktree,
        "tracker_written": false,
        "resumed_existing_worktree": resumed,
    })
}

/// `pr-review-claim`: adjudicates the pull request and makes its detached worktree.
///
/// Every read comes before any write, and a fork, a closed pull request or
/// this account's own stops before the first. A head other than the one this
/// run already holds is `head-moved`, because a new review is a new claim.
pub fn claim(
    context: &Context,
    request: &ClaimRequest<'_>,
    configured: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let horizon = require_horizon(request.horizon, Some(request.now))?;
    let seconds = |stamp: &str| {
        super::ownership::parse_stamp(stamp)
            .and_then(|seconds| u64::try_from(seconds).ok())
            .ok_or_else(|| Failure::Read(format!("{stamp} could not be read as a time")))
    };
    let now = seconds(request.now)?;
    let horizon = seconds(&horizon)?;
    let pull = read_pull(context, request.pr)?;
    let repo = repo_of(context)?;
    let worktree = worktree_for(request, &repo.name, configured)?;
    let worktree_text = worktree.display().to_string();
    let merge_base = fetch(context, request.pr, &pull)?;
    let wanted = PullReview {
        pr: request.pr,
        run_id: request.run_id.to_owned(),
        epoch: review_operation_id(
            "pull-review",
            &[
                request.run_id,
                &request.pr.to_string(),
                &pull.head,
                request.now,
            ],
        ),
        head: pull.head.clone(),
        base_ref: pull.base_ref.clone(),
        base_tip: pull.base_tip.clone(),
        merge_base,
        worktree: worktree_text.clone(),
        claimed_at: now,
        horizon,
    };
    let store = Store::open(&store_path()?)?;
    let held = store.claim_pull_review(&repo, &wanted, now)?;
    if held.run_id != request.run_id {
        return Err(stop(
            "pull-review-held",
            format!(
                "{} holds pull request #{} until {} (seconds since the epoch)",
                held.run_id, held.pr, held.horizon
            ),
            "leave this pull request to that run; its claim ends at its horizon",
        ));
    }
    if held.head != pull.head {
        return Err(head_moved(
            request.pr,
            format!(
                "this run's claim reviews {} and the head is now {}",
                held.head, pull.head
            ),
        ));
    }
    if std::path::Path::new(&held.worktree).is_dir() {
        return Ok(answer(&held, true));
    }
    if let Err(failure) = add_detached(context, &worktree_text, &held.head) {
        store.drop_pull_review(request.run_id)?;
        return Err(failure);
    }
    Ok(answer(&held, false))
}

/// Removes the detached worktree `review` names. A removal that failed is a
/// leftover `prune` names, reported rather than hidden.
pub(crate) fn remove_worktree(context: &Context, worktree: &str) -> serde_json::Value {
    if !std::path::Path::new(worktree).exists() {
        let _ = run(
            &["git", "worktree", "prune"],
            Some(&context.repo_dir),
            How::write(),
        );
        return serde_json::json!({ "worktree": worktree, "removed": true });
    }
    match run(
        &["git", "worktree", "remove", "--force", "--", worktree],
        Some(&context.repo_dir),
        How::write(),
    ) {
        Ok(_) => serde_json::json!({ "worktree": worktree, "removed": true }),
        Err(failure) => serde_json::json!({
            "worktree": worktree,
            "removed": false,
            "detail": failure.detail(),
        }),
    }
}

/// `pr-review-release`: puts the claim down and removes its worktree. Answers
/// from the store alone, so a pull request that is gone does not strand it.
pub fn release(context: &Context, run_id: &str) -> Result<serde_json::Value, Failure> {
    let store = Store::open(&store_path()?)?;
    let Some(held) = store.pull_review_of(run_id)? else {
        return Ok(serde_json::json!({
            "ok": true,
            "released": false,
            "detail": format!("{run_id} holds no review-only claim"),
        }));
    };
    let checkout = remove_worktree(context, &held.worktree);
    store.drop_pull_review(run_id)?;
    Ok(serde_json::json!({
        "ok": true,
        "released": true,
        "pr": held.pr,
        "checkout": checkout,
    }))
}

#[cfg(test)]
mod tests;
