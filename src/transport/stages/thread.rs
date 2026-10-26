use std::path::Path;

use super::{fetch, nodes, text, verify_reading};
use crate::transport::{Context, Failure, How, claim, run};

pub(crate) struct Response<'a> {
    pub branch: &'a str,
    pub worktree: &'a Path,
    pub thread_id: &'a str,
    pub expected_head: &'a str,
    pub fix_commit: Option<&'a str>,
    pub body_file: &'a Path,
    pub resolve: bool,
}

const REPLY: &str = "mutation($thread:ID!,$body:String!){addPullRequestReviewThreadReply(input:{pullRequestReviewThreadId:$thread,body:$body}){comment{id}}}";
const RESOLVE: &str =
    "mutation($thread:ID!){resolveReviewThread(input:{threadId:$thread}){thread{id isResolved}}}";

pub(crate) fn respond(
    context: &Context,
    what: &Response<'_>,
) -> Result<serde_json::Value, Failure> {
    let body = std::fs::read_to_string(what.body_file)
        .map_err(|error| Failure::Read(format!("the thread reply could not be read: {error}")))?;
    if body.trim().is_empty()
        || !oid(what.expected_head)
        || what.fix_commit.is_some_and(|commit| !oid(commit))
        || (what.resolve && what.fix_commit.is_none())
    {
        return Err(stop(
            "review-thread-invalid",
            "supply a nonempty reply and a full expected head SHA; resolution also requires a full fix_commit SHA",
            false,
        ));
    }
    let prs = claim::open_prs(context, what.branch)?;
    let [pr] = prs.as_slice() else {
        return Err(stop(
            "review-thread-pr-unconfined",
            "exactly one open PR must name the claimed branch",
            false,
        ));
    };
    let number = pr
        .get("number")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Failure::Read("the open PR has no number".to_owned()))?;
    let pr = read(context, number, what)?;
    let thread = find(&pr, what.thread_id)?;
    if let Some(fix) = what.fix_commit {
        verify_fix(what, thread, fix)?;
    }
    let key = claim::review_operation_id(
        "thread-reply",
        &[
            what.thread_id,
            what.expected_head,
            what.fix_commit.unwrap_or_default(),
            &body,
        ],
    );
    let reply = format!(
        "{body}\n\n{}<!-- estigia-thread-reply:{key} -->",
        what.fix_commit
            .map(|fix| format!("Fix commit: `{fix}`.\n\n"))
            .unwrap_or_default()
    );
    let mut reply_id = confirmed_reply(thread, &reply).map(str::to_owned);
    if reply_id.is_none() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let staged = claim::stage(&reply, &format!("thread-{key}-{nonce}.md"))?;
        let mutation = mutate(context, REPLY, what.thread_id, Some(&staged));
        let _ = std::fs::remove_file(&staged);
        mutation.map_err(|failure| unknown("reply", false, failure))?;
        let seen =
            read(context, number, what).map_err(|failure| unknown("reply", false, failure))?;
        reply_id = confirmed_reply(find(&seen, what.thread_id)?, &reply).map(str::to_owned);
        if reply_id.is_none() {
            return Err(unknown(
                "reply",
                false,
                Failure::Read("the exact reply was not listed".to_owned()),
            ));
        }
    }
    if what.resolve {
        let seen =
            read(context, number, what).map_err(|failure| unknown("resolution", true, failure))?;
        let thread = find(&seen, what.thread_id)?;
        verify_fix(what, thread, what.fix_commit.unwrap_or_default())
            .map_err(|failure| unknown("resolution", true, failure))?;
        if thread
            .get("isResolved")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            mutate(context, RESOLVE, what.thread_id, None)
                .map_err(|failure| unknown("resolution", true, failure))?;
        }
        let seen =
            read(context, number, what).map_err(|failure| unknown("resolution", true, failure))?;
        if find(&seen, what.thread_id)?
            .get("isResolved")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Err(unknown(
                "resolution",
                true,
                Failure::Read("the thread did not read back resolved".to_owned()),
            ));
        }
    }
    Ok(serde_json::json!({
        "ok": true, "pr": number, "head": what.expected_head, "thread_id": what.thread_id,
        "reply_id": reply_id, "reply_confirmed": true, "resolution_confirmed": what.resolve,
        "fix_commit": what.fix_commit, "tracker_written": false,
    }))
}

fn oid(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn read(context: &Context, number: u64, what: &Response<'_>) -> Result<serde_json::Value, Failure> {
    let pr = fetch(context, number)?;
    verify_reading(&pr, what.branch, Some(what.expected_head))?;
    if text(&pr, &["state"]) != Some("OPEN") {
        return Err(stop(
            "review-thread-pr-unconfined",
            "the PR is no longer open",
            false,
        ));
    }
    find(&pr, what.thread_id)?;
    Ok(pr)
}

fn find<'a>(pr: &'a serde_json::Value, id: &str) -> Result<&'a serde_json::Value, Failure> {
    nodes(pr, &["reviewThreads"])
        .into_iter()
        .find(|thread| text(thread, &["id"]) == Some(id))
        .ok_or_else(|| {
            stop(
                "review-thread-pr-unconfined",
                "the thread is not in the claimed branch's PR",
                false,
            )
        })
}

fn confirmed_reply<'a>(thread: &'a serde_json::Value, body: &str) -> Option<&'a str> {
    nodes(thread, &["comments"])
        .into_iter()
        .find_map(|comment| {
            (text(comment, &["body"]) == Some(body)
                && comment
                    .get("viewerDidAuthor")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true))
            .then(|| text(comment, &["id"]))
            .flatten()
        })
}

fn verify_fix(what: &Response<'_>, thread: &serde_json::Value, fix: &str) -> Result<(), Failure> {
    let baseline = nodes(thread, &["comments"])
        .first()
        .and_then(|comment| {
            text(comment, &["originalCommit", "oid"]).or_else(|| text(comment, &["commit", "oid"]))
        })
        .filter(|commit| oid(commit))
        .ok_or_else(|| {
            stop(
                "review-thread-fix-unpublished",
                "the thread's original commit is unknown",
                false,
            )
        })?;
    let ancestor = |older, newer| {
        run(
            &[
                "git",
                "--no-replace-objects",
                "merge-base",
                "--is-ancestor",
                older,
                newer,
            ],
            Some(what.worktree),
            How::tolerated(),
        )
        .map(|out| out.status == 0)
    };
    if fix.eq_ignore_ascii_case(baseline)
        || !ancestor(baseline, fix)?
        || !ancestor(fix, what.expected_head)?
    {
        return Err(stop(
            "review-thread-fix-unpublished",
            "the fix must follow the thread's original commit and be an ancestor of the published head",
            false,
        ));
    }
    Ok(())
}

fn mutate(
    context: &Context,
    query: &str,
    thread: &str,
    body_file: Option<&str>,
) -> Result<(), Failure> {
    let query = format!("query={query}");
    let thread = format!("thread={thread}");
    let mut args = vec!["gh", "api", "graphql", "-f", &query, "-f", &thread];
    let body = body_file.map(|file| format!("body=@{file}"));
    if let Some(body) = body.as_deref() {
        args.extend(["-F", body]);
    }
    let out = run(&args, Some(&context.repo_dir), How::write())?;
    let value: serde_json::Value = serde_json::from_str(&out.stdout).map_err(|error| {
        Failure::Write(format!("the mutation's answer could not be read: {error}"))
    })?;
    if value
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
    {
        return Err(Failure::Write(format!(
            "the mutation reported errors: {value}"
        )));
    }
    Ok(())
}

fn stop(reason: &str, detail: &str, reply_confirmed: bool) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false, "reason": reason, "detail": detail, "reply_confirmed": reply_confirmed,
        "action": "read this PR's published head and threads; do not resolve an unverified fix",
    }))
}

fn unknown(operation: &str, reply_confirmed: bool, failure: Failure) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false, "reason": "review-thread-write-unconfirmed", "world": "unknown",
        "operation": operation, "reply_confirmed": reply_confirmed,
        "resolution_confirmed": false, "detail": failure.detail(),
        "action": "read the thread and published head before retrying; do not report an unconfirmed write as completed",
    }))
}
