//! `draft-pr` — publish an assisted run's branch as a draft pull request.
//!
//! Where [`super::claim::publish_review`] binds a receipt to the issue's
//! timeline, this writes nothing to the tracker at all: no marker, no comment, no
//! `Refs` line, no ledger. What it does hold is the one line the assisted
//! workflow cannot hand to the person — a closing keyword would close the issue
//! on merge in Estigia's name.
//!
//! The pull request is read back for the pushed head and for being a draft; the
//! team's own CI and reviews are theirs, and nothing here waits for either.

use super::claim::{
    PUBLISH_READBACK_ATTEMPTS, Push, open_prs, pr_number, publish_schedule, push_to_origin,
    readback_delay_secs, stage,
};
use super::{Context, Failure, How, run};

/// One call: the branch and checkout the run pointer names, and the person's words.
#[derive(Debug, Clone, Copy)]
pub struct Draft<'a> {
    /// The branch `assist` recorded for this run.
    pub branch: &'a str,
    /// The checkout `assist` made, which must have `branch` checked out.
    pub worktree: &'a std::path::Path,
    /// The pull request title, as given.
    pub title: &'a str,
    /// Path to the pull request body, read once and written as given.
    pub body_file: &'a std::path::Path,
}

/// `draft-pr` — see the module.
pub fn draft_pull_request(
    context: &Context,
    what: &Draft<'_>,
) -> Result<serde_json::Value, Failure> {
    draft_with_sleep(context, what, &mut |delay| std::thread::sleep(delay))
}

fn stop(reason: &str, action: String) -> Failure {
    Failure::Stop(serde_json::json!({ "ok": false, "reason": reason, "action": action }))
}

fn draft_with_sleep(
    context: &Context,
    what: &Draft<'_>,
    sleep: &mut dyn FnMut(std::time::Duration),
) -> Result<serde_json::Value, Failure> {
    let Draft {
        branch,
        worktree,
        title,
        body_file,
    } = *what;

    let checked_out = run(
        &["git", "symbolic-ref", "HEAD"],
        Some(worktree),
        How::tolerated(),
    )?;
    if checked_out.status != 0 || checked_out.stdout.trim() != format!("refs/heads/{branch}") {
        return Err(stop(
            "draft-pr-branch-not-checked-out",
            format!(
                "{} does not have `{branch}` checked out, and this run's local claim names that \
                 branch; nothing was pushed and no pull request was opened \u{2014} check it out \
                 there, or work in the checkout `assist` made",
                worktree.display()
            ),
        ));
    }
    publish_schedule(context)?;

    let body = std::fs::read_to_string(body_file)
        .map_err(|error| Failure::Read(format!("the PR body could not be read: {error}")))?;
    let base = default_branch(context)?;
    run(
        &["git", "fetch", "origin", "--", &base],
        Some(worktree),
        How::read(),
    )?;
    let mut keywords = super::closing::keywords_any(title);
    keywords.extend(super::closing::keywords_any(&body));
    keywords.extend(super::closing::any_keywords_in_commits(
        worktree, &base, branch,
    )?);
    if !keywords.is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "closing-keyword-live",
            "keyword_sources": keywords,
            "action": "a merge would close the issue in Estigia's name \u{2014} remove the keyword \
                       from the title, the body or the commit messages and run this again; \
                       nothing has been pushed and no pull request was opened",
        })));
    }

    let head = run(&["git", "rev-parse", "HEAD"], Some(worktree), How::read())?
        .stdout
        .trim()
        .to_owned();
    let existing = open_prs(context, branch)?;
    if existing.len() > 1 {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "ambiguous-open-prs",
            "prs": existing,
            "action": "more than one open pull request for this branch \u{2014} resolve by hand; \
                       nothing has been pushed",
        })));
    }
    let reused = existing.first();
    if let Some(open) = reused
        && open.get("isDraft").and_then(serde_json::Value::as_bool) != Some(true)
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "draft-pr-exists-ready",
            "pr": pr_number(open),
            "action": "this branch already has an open pull request that is ready for review, and \
                       this operation never converts or edits one \u{2014} nothing has been \
                       pushed; the person decides what happens to it",
        })));
    }

    push_to_origin(worktree, branch, Push::FastForward)?;
    let committed = |failure: Failure| match failure {
        Failure::Read(detail) | Failure::RateLimited(detail) | Failure::Write(detail) => {
            Failure::Write(format!(
                "{detail} \u{2014} the branch `{branch}` is already pushed to origin, so this is not \
             a call that changed nothing"
            ))
        }
        other => other,
    };

    let number = match reused {
        Some(open) => pr_number(open),
        None => {
            let staged = stage(&body, &staged_name(branch)).map_err(committed)?;
            let answer = run(
                &[
                    "gh",
                    "pr",
                    "create",
                    "--base",
                    &base,
                    "--head",
                    branch,
                    "--title",
                    title,
                    "--draft",
                    "--body-file",
                    &staged,
                ],
                Some(&context.repo_dir),
                How::write(),
            );
            let _ = std::fs::remove_file(&staged);
            let answer = answer.map_err(committed)?;
            number_in(&answer.stdout).ok_or_else(|| {
                unconfirmed(
                    branch,
                    &head,
                    None,
                    &[serde_json::json!({ "created": answer.stdout.trim() })],
                )
            })?
        }
    };

    let mut seen = Vec::new();
    for upcoming in 1..=PUBLISH_READBACK_ATTEMPTS {
        if upcoming > 1 {
            sleep(std::time::Duration::from_secs(readback_delay_secs(
                upcoming,
            )));
        }
        if let Some(value) = super::gh_json(
            &[
                "pr",
                "view",
                &number.to_string(),
                "--json",
                "number,url,headRefOid,headRefName,state,isDraft",
            ],
            Some(&context.repo_dir),
        )
        .map_err(|_| unconfirmed(branch, &head, Some(number), &seen))?
        {
            if agrees(&value, branch, &head) {
                return Ok(serde_json::json!({
                    "ok": true,
                    "pr": number,
                    "url": value.get("url"),
                    "branch": branch,
                    "base": base,
                    "head": head,
                    "draft": true,
                    "created": reused.is_none(),
                    "title_and_body_applied": reused.is_none(),
                    "tracker_written": false,
                }));
            }
            seen.push(value);
        }
    }
    Err(unconfirmed(branch, &head, Some(number), &seen))
}

fn agrees(seen: &serde_json::Value, branch: &str, head: &str) -> bool {
    let text = |key: &str| seen.get(key).and_then(serde_json::Value::as_str);
    text("headRefOid") == Some(head)
        && text("headRefName") == Some(branch)
        && text("state") == Some("OPEN")
        && seen.get("isDraft").and_then(serde_json::Value::as_bool) == Some(true)
}

fn unconfirmed(
    branch: &str,
    head: &str,
    number: Option<u64>,
    observed: &[serde_json::Value],
) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "draft-pr-readback-failed",
        "world": "committed",
        "pr": number,
        "expected_head": head,
        "observed": observed.last(),
        "action": format!(
            "the push landed (`{branch}` at `{head}`) and a pull request may exist, but it was not \
             read back as an open draft at that head \u{2014} do not report one; read the \
             branch's pull requests, then run this again, which reuses an open draft rather than \
             making a second"
        ),
    }))
}

fn default_branch(context: &Context) -> Result<String, Failure> {
    super::gh_json(
        &["repo", "view", "--json", "defaultBranchRef"],
        Some(&context.repo_dir),
    )?
    .and_then(|value| {
        value
            .get("defaultBranchRef")?
            .get("name")?
            .as_str()
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    })
    .ok_or_else(|| {
        Failure::Read(
            "the repository's default branch could not be read, so the pull request has no base \
             to open against"
                .to_owned(),
        )
    })
}

fn number_in(created: &str) -> Option<u64> {
    created
        .lines()
        .rev()
        .find_map(|line| line.trim().rsplit_once("/pull/")?.1.parse().ok())
}

fn staged_name(branch: &str) -> String {
    static STAGED_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let call = STAGED_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("draft-pr-{}-{}", slug(branch), call)
}

fn slug(branch: &str) -> String {
    branch
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests;
