//! The one typed road from a computed verdict to GitHub.
//!
//! The head is read again, the review is sent carrying the reviewed `commit_id`,
//! and it is reported only once read back. GitHub has no conditional write here,
//! so a push between the read and the write cannot be prevented: the read-back and
//! a last read of the head say what happened, and `docs/assisted-review.md` says
//! what that does not prove.

use std::collections::HashMap;
use std::ops::RangeInclusive;

use super::panel::{self, Verdict};
use super::{held_by, remove_worktree, stop};
use crate::reviewer::result::Finding;
use crate::transport::claim::{
    FindingLocation, changed_new_side_lines, parse_finding_location, stage,
};
use crate::transport::store::{PullReview, Store, store_path};
use crate::transport::voice::{self, Renderer};
use crate::transport::{Context, Failure, How, gh_json, run};

const BODY_CEILING: usize = 60_000;
const COMMENT_CEILING: usize = 60_000;

struct Live {
    state: String,
    head: String,
    base_ref: String,
}

fn read_live(context: &Context, pr: u64) -> Result<Live, Failure> {
    let number = pr.to_string();
    let data = gh_json(
        &[
            "pr",
            "view",
            &number,
            "--json",
            "state,headRefOid,baseRefName",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh pr view {pr} returned nothing")))?;
    let text = |key: &str| data[key].as_str().unwrap_or_default().to_owned();
    let live = Live {
        state: text("state"),
        head: text("headRefOid"),
        base_ref: text("baseRefName"),
    };
    if live.state.is_empty() || live.head.is_empty() || live.base_ref.is_empty() {
        return Err(Failure::Read(format!(
            "gh pr view {pr} did not name a state, a head and a base branch"
        )));
    }
    Ok(live)
}

// Nothing was sent and this claim cannot produce a verdict any more, so it is put
// down here: the answer is a success at the tool level because the pointer goes
// with it, and `submitted: false` is what a caller reads.
fn refuse(
    context: &Context,
    store: &Store,
    held: &PullReview,
    reason: &str,
    detail: String,
) -> Result<serde_json::Value, Failure> {
    let checkout = remove_worktree(context, &held.worktree);
    store.drop_pull_review(&held.run_id)?;
    Ok(serde_json::json!({
        "ok": true,
        "submitted": false,
        "reason": reason,
        "pr": held.pr,
        "head": held.head,
        "detail": detail,
        "checkout": checkout,
    }))
}

fn line_of(finding: &Finding) -> String {
    match finding.location.as_deref() {
        Some(location) => format!("- `{location}`: {} {}\n", finding.evidence, finding.impact),
        None => format!("- {} {}\n", finding.evidence, finding.impact),
    }
}

struct Comment {
    site: FindingLocation,
    text: String,
}

impl Comment {
    fn json(&self) -> serde_json::Value {
        let mut comment = serde_json::json!({
            "path": self.site.path,
            "line": self.site.end,
            "side": "RIGHT",
            "body": self.text,
        });
        if self.site.start != self.site.end {
            comment["start_line"] = self.site.start.into();
            comment["start_side"] = "RIGHT".into();
        }
        comment
    }
}

fn site_in_change(
    location: Option<&str>,
    changed: &mut dyn FnMut(&str) -> Vec<RangeInclusive<u32>>,
) -> Option<FindingLocation> {
    let site = parse_finding_location(location?)?;
    changed(&site.path)
        .iter()
        .any(|range| range.contains(&site.start) && range.contains(&site.end))
        .then_some(site)
}

// A comment is what the diff can carry: a whole range inside one hunk of the right
// side. Anything else, and anything the host would refuse for length, stays in the
// body, because one refused comment refuses the whole review.
fn place<'a>(
    context: &Context,
    held: &PullReview,
    findings: &'a [Finding],
    open: &[String],
) -> (Vec<Comment>, Vec<&'a Finding>) {
    let mut seen: HashMap<String, Vec<RangeInclusive<u32>>> = HashMap::new();
    let mut changed = |path: &str| {
        seen.entry(path.to_owned())
            .or_insert_with(|| {
                changed_new_side_lines(context, &held.merge_base, &held.head, path)
                    .map(|(_, ranges)| ranges)
                    .unwrap_or_default()
            })
            .clone()
    };
    let mut comments = Vec::new();
    let mut body = Vec::new();
    let shown = findings
        .iter()
        .filter(|finding| open.contains(&finding.id) || finding.class != "severe");
    for finding in shown {
        let text = format!("{} {}", finding.evidence, finding.impact);
        match site_in_change(finding.location.as_deref(), &mut changed) {
            Some(site) if text.len() <= COMMENT_CEILING => comments.push(Comment { site, text }),
            _ => body.push(finding),
        }
    }
    (comments, body)
}

fn body_of(findings: &[&Finding], inline: usize) -> String {
    let mut body = String::new();
    let mut left = 0;
    if inline > 0 {
        body.push_str(&format!("{inline} posted inline on the diff.\n\n"));
    }
    for finding in findings {
        let line = line_of(finding);
        if body.len() + line.len() > BODY_CEILING {
            left += 1;
        } else {
            body.push_str(&line);
        }
    }
    if left > 0 {
        body.push_str(&format!("\n{left} more did not fit here.\n"));
    }
    body
}

fn voiced(
    context: &Context,
    renderer: &Renderer,
    body: String,
    comments: &[Comment],
) -> Result<(String, Vec<Comment>), Failure> {
    let mut texts: Vec<String> = comments
        .iter()
        .map(|comment| comment.text.clone())
        .collect();
    if !body.is_empty() {
        texts.push(body.clone());
    }
    let mut rendered = voice::render(context, renderer, &texts)?;
    let body = if body.is_empty() {
        body
    } else {
        rendered.pop().unwrap_or_default()
    };
    let comments = comments
        .iter()
        .zip(rendered)
        .map(|(comment, text)| Comment {
            site: FindingLocation {
                path: comment.site.path.clone(),
                start: comment.site.start,
                end: comment.site.end,
            },
            text,
        })
        .collect();
    Ok((body, comments))
}

fn reviews_path(pr: u64) -> String {
    format!("repos/{{owner}}/{{repo}}/pulls/{pr}/reviews")
}

// A write whose answer was lost leaves its review on GitHub, and a second one
// would be a second verdict, so a retry takes this account's review of this commit.
fn already_sent(
    context: &Context,
    held: &PullReview,
) -> Result<Option<serde_json::Value>, Failure> {
    let reviews = gh_json(
        &["api", "--paginate", "--slurp", &reviews_path(held.pr)],
        Some(&context.repo_dir),
    )?
    .unwrap_or_default();
    let viewer = crate::transport::store::viewer_login(context)?;
    Ok(reviews
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|page| page.as_array().into_iter().flatten())
        .find(|review| {
            review["commit_id"].as_str() == Some(held.head.as_str())
                && review["user"]["login"].as_str() == Some(viewer.as_str())
        })
        .cloned())
}

fn send(
    context: &Context,
    held: &PullReview,
    event: &str,
    body: &str,
    comments: &[Comment],
) -> Result<serde_json::Value, Failure> {
    let mut request = serde_json::json!({
        "commit_id": held.head,
        "event": event,
        "body": body,
    });
    if !comments.is_empty() {
        request["comments"] = comments.iter().map(Comment::json).collect();
    }
    let staged = stage(&request.to_string(), &format!("pull-review-{}", held.pr))?;
    let path = reviews_path(held.pr);
    let output = run(
        &["gh", "api", "--method", "POST", &path, "--input", &staged],
        Some(&context.repo_dir),
        How::write(),
    )?;
    serde_json::from_str(output.stdout.trim())
        .map_err(|error| Failure::Write(format!("the review's answer could not be read: {error}")))
}

fn unconfirmed(held: &PullReview, detail: String) -> Failure {
    stop(
        "pull-review-submission-unconfirmed",
        format!(
            "the review of pull request #{} may have landed and was not confirmed: {detail}",
            held.pr
        ),
        "call `pr_review` `submit` again: it looks for this account's review of this commit \
         before sending another. Do not report it as submitted until one reads back",
    )
}

fn read_back(
    context: &Context,
    held: &PullReview,
    event: &str,
    sent: &serde_json::Value,
    comments: &[Comment],
) -> Result<serde_json::Value, Failure> {
    let id = sent["id"]
        .as_u64()
        .ok_or_else(|| unconfirmed(held, "the answer named no review id".to_owned()))?;
    let path = format!("{}/{id}", reviews_path(held.pr));
    let read = gh_json(&["api", &path], Some(&context.repo_dir))
        .map_err(|failure| unconfirmed(held, failure.detail()))?
        .ok_or_else(|| unconfirmed(held, "the read-back returned nothing".to_owned()))?;
    let wanted_state = match event {
        "APPROVE" => "APPROVED",
        _ => "CHANGES_REQUESTED",
    };
    if read["commit_id"].as_str() != Some(held.head.as_str())
        || read["state"].as_str() != Some(wanted_state)
    {
        return Err(unconfirmed(
            held,
            format!(
                "it reads commit {} in state {}, and {} in state {wanted_state} was sent",
                read["commit_id"], read["state"], held.head
            ),
        ));
    }
    confirm_comments(context, held, id, comments)?;
    Ok(read)
}

fn confirm_comments(
    context: &Context,
    held: &PullReview,
    id: u64,
    comments: &[Comment],
) -> Result<(), Failure> {
    if comments.is_empty() {
        return Ok(());
    }
    let path = format!("{}/{id}/comments", reviews_path(held.pr));
    let listed = gh_json(
        &["api", "--paginate", "--slurp", &path],
        Some(&context.repo_dir),
    )
    .map_err(|failure| unconfirmed(held, failure.detail()))?
    .ok_or_else(|| unconfirmed(held, "the comments read back as nothing".to_owned()))?;
    let mut found: Vec<&str> = listed
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|page| page.as_array().into_iter().flatten())
        .map(|comment| comment["path"].as_str().unwrap_or_default())
        .collect();
    let mut wanted: Vec<&str> = comments
        .iter()
        .map(|comment| comment.site.path.as_str())
        .collect();
    found.sort_unstable();
    wanted.sort_unstable();
    if found != wanted {
        return Err(unconfirmed(
            held,
            format!(
                "the review reads back {} inline comments on {found:?}, and {} on {wanted:?} \
                 were sent",
                found.len(),
                wanted.len()
            ),
        ));
    }
    Ok(())
}

/// `pr-review-submit`: sends the computed verdict, bound to the reviewed head.
pub fn submit(context: &Context, run_id: &str) -> Result<serde_json::Value, Failure> {
    submit_with(context, run_id, &Renderer::detached())
}

pub(crate) fn submit_with(
    context: &Context,
    run_id: &str,
    renderer: &Renderer,
) -> Result<serde_json::Value, Failure> {
    let store = Store::open(&store_path()?)?;
    let held = held_by(&store, run_id)?;
    if panel::payload_exceeds_ceiling(context, &held)? {
        return refuse(
            context,
            &store,
            &held,
            "review-payload-too-large",
            "no review ran: the change is larger than a reviewer may be handed".to_owned(),
        );
    }
    let reading = panel::read(context, &store, run_id)?;
    let event = match &reading.verdict {
        Verdict::Incomplete { missing } => {
            return Err(stop(
                "pull-review-incomplete",
                format!("no verdict yet: {} has not answered", missing.join(", ")),
                "follow what `pr_review` `status` names, then submit",
            ));
        }
        Verdict::Escalated { reasons } => {
            return refuse(
                context,
                &store,
                &held,
                "pull-review-escalated",
                format!(
                    "the answers cannot settle a severe finding ({}), so neither event is right; \
                     report the findings to a person",
                    reasons.join("; ")
                ),
            );
        }
        Verdict::Approved => "APPROVE",
        Verdict::ChangesRequested { .. } => "REQUEST_CHANGES",
    };
    let open = match &reading.verdict {
        Verdict::ChangesRequested { open } => open.as_slice(),
        _ => &[],
    };
    let (comments, unanchored) = place(context, &held, &reading.findings, open);
    // Looked for before the pull request is judged: a review an earlier call may
    // have landed is answered for, whatever has happened to the pull request since.
    let found =
        already_sent(context, &held).map_err(|failure| unconfirmed(&held, failure.detail()))?;
    let sent = match found {
        Some(sent) => sent,
        None => {
            let live = read_live(context, held.pr)?;
            if live.state != "OPEN" {
                return refuse(
                    context,
                    &store,
                    &held,
                    "pull-review-not-open",
                    format!("pull request #{} is {} now", held.pr, live.state),
                );
            }
            if live.head != held.head {
                return refuse(
                    context,
                    &store,
                    &held,
                    "head-moved",
                    format!(
                        "the review read {} and the head is now {}; a new review is a new claim",
                        held.head, live.head
                    ),
                );
            }
            if live.base_ref != held.base_ref {
                return refuse(
                    context,
                    &store,
                    &held,
                    "pull-review-retargeted",
                    format!(
                        "the review was measured against {} and the pull request now targets {}",
                        held.base_ref, live.base_ref
                    ),
                );
            }
            let (body, voiced) = voiced(
                context,
                renderer,
                body_of(&unanchored, comments.len()),
                &comments,
            )?;
            send(context, &held, event, &body, &voiced)
                .map_err(|failure| unconfirmed(&held, failure.detail()))?
        }
    };
    let confirmed = read_back(context, &held, event, &sent, &comments)?;
    let head_after = read_live(context, held.pr).map(|after| after.head).ok();
    let checkout = remove_worktree(context, &held.worktree);
    store.drop_pull_review(run_id)?;
    Ok(serde_json::json!({
        "ok": true,
        "submitted": true,
        "pr": held.pr,
        "event": event,
        "review_id": confirmed["id"],
        "commit_id": confirmed["commit_id"],
        "state": confirmed["state"],
        "url": confirmed["html_url"],
        "head_moved_during_submission": head_after.as_deref().map(|head| head != held.head),
        "checkout": checkout,
    }))
}

#[cfg(test)]
mod tests;
