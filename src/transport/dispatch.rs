//! One operation name and its flags, answered **here** rather than by spawning
//! an interpreter.
//!
//! This is the piece that ends the chain. Every tool an agent calls arrived as
//! `agent → MCP tool (Rust) → python github.py <operation> → gh`, so the port
//! existed beside the transport and the transport was still what ran. What this
//! module does is what the transport's own `main()` does: read an operation name
//! and a flag list, and call the function that answers it.
//!
//! The flags are the transport's, not a new vocabulary. `harness::mcp` builds
//! them from each tool's typed arguments and they were already being handed to a
//! command line, so what changes is who reads them — and a flag spelling that
//! drifted from the transport's would be a tool that silently stopped passing
//! something, which is why the names below are the binding's own.
//!
//! **Unknown is refused, never ignored.** An operation this does not know is a
//! caller asking for something that will not happen, and answering `ok` to it —
//! or falling back to the interpreter — is the shape this crate exists to
//! refuse: a result nobody read reported as a result.

use std::path::Path;

use super::{Context, Failure};

/// The flags of one call, by name.
///
/// Built once so a missing value is answered the same way everywhere. Repeated
/// flags keep the **last**, which is what `argparse` does and therefore what a
/// caller that passes one twice already expects.
struct Flags<'a> {
    named: std::collections::BTreeMap<&'a str, &'a str>,
    present: std::collections::BTreeSet<&'a str>,
}

impl<'a> Flags<'a> {
    fn read(flags: &'a [String]) -> Self {
        let mut named = std::collections::BTreeMap::new();
        let mut present = std::collections::BTreeSet::new();
        let mut at = 0;
        while at < flags.len() {
            let Some(name) = flags[at].strip_prefix("--") else {
                at += 1;
                continue;
            };
            present.insert(name);
            // A flag whose next word is another flag is a switch, and one at the
            // end of the list is a switch too. Reading the following word blindly
            // would make `--force --json` set `force` to the string `--json` and
            // swallow the flag after it.
            match flags.get(at + 1) {
                Some(value) if !value.starts_with("--") => {
                    named.insert(name, value.as_str());
                    at += 2;
                }
                _ => at += 1,
            }
        }
        Self { named, present }
    }

    fn get(&self, name: &str) -> Option<&'a str> {
        self.named.get(name).copied()
    }

    fn on(&self, name: &str) -> bool {
        self.present.contains(name)
    }

    /// A flag the operation cannot proceed without.
    ///
    /// `ConfigDefect` and not `Read`: a missing argument is the caller's own
    /// defect, and the transport spells it the same way — exit `2`, the code
    /// that means *the operator's configuration is wrong, or an argument was
    /// malformed*.
    fn need(&self, name: &str, operation: &str) -> Result<&'a str, Failure> {
        let value = self.get(name).ok_or_else(|| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "missing-argument",
                "operation": operation,
                "argument": format!("--{name}"),
            }))
        })?;
        // **There is not the same as says something**, and only the first was
        // being asked. Measured across the fifteen flags this refuses without:
        // an empty `--run-id`, `--to`, `--state`, `--expect-state` or
        // `--branch` went straight past the argument layer and reached `gh`.
        //
        // Each of those is carried into the world as a name: `--to ""` writes
        // `status:` with nothing after the colon, `--run-id ""` claims an issue
        // for nobody, `--branch ""` pushes a ref with no name. It is the same
        // defect the empty `--runtime` had, and it had five more homes.
        //
        // Refused here rather than at each reader, because the two questions
        // are one question — a required argument is one that arrived **and**
        // answered — and none of the fifteen has a meaningful empty value.
        if value.trim().is_empty() {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "blank-argument",
                "operation": operation,
                "argument": format!("--{name}"),
                "value": value,
                // Naming the flag, because the sentence is what an agent reads
                // and the envelope's fields are not. This said *"it is there and
                // says nothing"* with the flag in the field beside it and not in
                // the words — the fact in hand at the point of decision, left
                // out of the answer.
                "detail": format!("--{name} is there and says nothing"),
            })));
        }
        Ok(value)
    }

    /// A flag that has to be a number.
    fn number(&self, name: &str, operation: &str) -> Result<u64, Failure> {
        let text = self.need(name, operation)?;
        text.trim().parse::<u64>().map_err(|_| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "malformed-argument",
                "operation": operation,
                "argument": format!("--{name}"),
                "value": text,
            }))
        })
    }

    fn path(&self, name: &str) -> Option<&'a Path> {
        self.get(name).map(Path::new)
    }
}

/// Runs one transport operation in this process.
///
/// `now` is passed in rather than read here: the timeline decisions this feeds
/// are compared against stamps the caller already holds, and a function that
/// asks the clock twice can answer two questions about two different moments.
pub fn dispatch(
    context: &Context,
    operation: &str,
    flags: &[String],
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let f = Flags::read(flags);
    let issue = || f.number("issue", operation);
    let run_id = || f.need("run-id", operation);

    match operation {
        "config" => super::commands::config(context),
        "ensure-states" => super::commands::ensure_states(context),
        "list-boards" => super::board::list_boards(context, f.get("owner")),
        "list-state" => super::commands::list_state(
            context,
            f.need("state", operation)?,
            run_id()?,
            // 200, which is the transport's own default. It was 30 here, and a
            // tool that leaves `--limit` off — its schema marks it optional —
            // got a list cut to a seventh of what it asked for, reported as the
            // state. A partial read offered as the state is the failure this
            // crate refuses everywhere else.
            f.get("limit")
                .and_then(|limit| limit.trim().parse::<u32>().ok())
                .unwrap_or(200),
        ),
        "comment" => super::commands::comment(
            context,
            issue()?,
            Path::new(f.need("body-file", operation)?),
            f.get("run-id"),
            f.get("kind"),
        ),
        "create" => super::commands::create(
            context,
            f.need("identity", operation)?,
            f.need("title", operation)?,
            Path::new(f.need("body-file", operation)?),
            // Defaulted, not required. The tool marks `state` optional because
            // the transport defaults it to `ready`; required here, a `create`
            // that left it off was refused for an argument nobody has to send.
            f.get("state").unwrap_or("ready"),
            f.get("priority"),
            f.get("domain"),
            f.get("runtime"),
            f.get("run-id"),
            !f.on("no-cache"),
        ),
        "transition" => super::commands::transition(
            context,
            issue()?,
            f.need("to", operation)?,
            f.get("from"),
            !f.on("no-cache"),
        ),
        "audit-board" => super::commands::audit_board(context, f.on("fix"), !f.on("no-cache")),
        "changelog-notes" => super::commands::changelog_notes(
            context,
            Path::new(f.need("file", operation)?),
            f.need("version", operation)?,
            f.on("include-heading"),
            f.path("out"),
        ),
        "check-closing-keywords" => super::commands::check_closing_keywords(
            context,
            issue()?,
            f.get("base"),
            f.get("branch"),
        ),
        "base-movement" => super::commands::base_movement(
            context,
            f.need("base", operation)?,
            f.need("recorded-base", operation)?,
            f.path("worktree"),
        ),
        "expected-target" => super::target::expected_target(
            context,
            f.need("base", operation)?,
            f.path("worktree"),
            // `--native-start`: a manifest a reviewer was given, and any
            // disagreement with it fails closed. Read under a name nobody
            // sends, it arrived as `None` — so the one argument whose whole
            // purpose is to be compared against was never compared.
            f.path("native-start"),
        ),
        "verify-claim" => super::claim::verify_claim(
            context,
            issue()?,
            run_id()?,
            f.need("expect-state", operation)?,
            now,
            f.get("allow-closed-by-pr")
                .and_then(|number| number.trim().parse::<u64>().ok()),
        ),
        "heartbeat" => super::claim::heartbeat(
            context,
            issue()?,
            run_id()?,
            f.need("expect-state", operation)?,
            Path::new(f.need("body-file", operation)?),
            now,
        ),
        "claim" => super::claim::claim(
            context,
            &super::claim::Acquisition {
                issue: issue()?,
                run_id: run_id()?,
                runtime: f.need("runtime", operation)?,
                horizon: f.need("horizon", operation)?,
                operation_id: f.need("operation-id", operation)?,
                now,
            },
        ),
        "reclaim" => super::claim::reclaim(
            context,
            &super::claim::Reclaim {
                issue: issue()?,
                run_id: run_id()?,
                runtime: f.need("runtime", operation)?,
                horizon: f.need("horizon", operation)?,
                operation_id: f.need("operation-id", operation)?,
                target_operation: f.get("target-operation"),
                force: f.on("force"),
                reason_file: f.path("reason-file"),
                now,
            },
        ),
        "unassign" => super::claim::unassign(
            context,
            &super::claim::Departure {
                issue: issue()?,
                run_id: run_id()?,
                runtime: f.need("runtime", operation)?,
                operation_id: f.need("operation-id", operation)?,
                target_operation: f.get("target-operation"),
                held_by_other: f.on("held-by-other"),
                now,
            },
        ),
        "handoff-review" => super::claim::handoff_review(
            context,
            &super::claim::HandoffReview {
                issue: issue()?,
                run_id: run_id()?,
                runtime: f.need("runtime", operation)?,
                operation_id: f.need("operation-id", operation)?,
                target_operation: f.need("target-operation", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                blocker: f.need("blocker", operation)?,
                discharger: f.need("discharger", operation)?,
                now,
            },
        ),
        "review-verdict" => super::claim::record_review_verdict(
            context,
            &super::claim::VerdictReview {
                issue: issue()?,
                run_id: run_id()?,
                reviewer: f.need("reviewer", operation)?,
                operation_id: f.need("operation-id", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                outcome: f.need("outcome", operation)?,
                now,
            },
        ),
        "publish-review" => super::claim::publish_review(
            context,
            &super::claim::Publication {
                issue: issue()?,
                run_id: run_id()?,
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                base: f.need("base", operation)?,
                branch: f.need("branch", operation)?,
                // `--pr-title`, which is what the transport takes and what the
                // tool sends. Read as `title`, the value arrived and nothing
                // asked for it: the call was dispatched, the flag was parsed
                // into a map nobody consulted, and `publish-review` refused
                // for a missing argument the caller had supplied.
                title: f.need("pr-title", operation)?,
                pr_body_file: f.path("pr-body-file"),
                // Declared by the tool, taken by the original, and read by
                // nobody: the flag was parsed and the push ran in the base
                // checkout whatever the caller sent.
                worktree: f.path("worktree"),
                now,
            },
        ),
        // The same arguments as `publish-review`, read the same way, because the
        // two differ in the push and in nothing else. Spelled out rather than
        // shared with the arm above: the flag guard reads each arm's own body,
        // so an arm that borrowed its neighbour's would be an operation whose
        // arguments nothing checks.
        "republish-review" => super::claim::republish_review(
            context,
            &super::claim::Publication {
                issue: issue()?,
                run_id: run_id()?,
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                base: f.need("base", operation)?,
                branch: f.need("branch", operation)?,
                title: f.need("pr-title", operation)?,
                pr_body_file: f.path("pr-body-file"),
                worktree: f.path("worktree"),
                now,
            },
        ),
        "release-ci" => super::claim::release_ci(
            context,
            &super::claim::CiRelease {
                issue: issue()?,
                run_id: run_id()?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                worktree: f.path("worktree"),
                now,
            },
        ),
        "start-branch" => super::branch::start_branch(
            context,
            &super::branch::Start {
                issue: issue()?,
                branch: f.need("branch", operation)?,
                base: f.need("base", operation)?,
                run_id: run_id()?,
                worktree_root: f.get("worktree-root"),
                // Derived from the checkout when the caller does not name it,
                // which is what the original does: `_, repo_name =
                // repo_identity(cwd)`. Ported as a **required flag**, and
                // nothing supplies it — not the MCP tool, whose argument list
                // has no `repo_name` in any spelling, and not the binding,
                // which never mentions one. So every `start_branch` an agent
                // could make refused with `missing-argument --repo-name`, and
                // the crossing passed because the test hands it over.
                //
                // Measured through the server: `start_branch` with exactly the
                // arguments its own schema declares came back
                // `transport-refused`, on the one tool whose whole job is
                // making the isolated checkout.
                repo_name: &match f.get("repo-name") {
                    Some(named) => named.to_owned(),
                    None => super::closing::repo_identity(context)?.1,
                },
                // The transport defaults this to `in-progress` and the tool
                // marks it optional, so requiring it refused a call nobody had
                // to make differently.
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                now,
            },
            // The operator's own `Worktree location`, which the flag overrides
            // and does not replace. This passed the flag for both, so the
            // configured row was never consulted: with no flag the template was
            // empty and every checkout landed wherever an empty template puts
            // it, whatever the operator had set.
            context.get("worktree location"),
        ),
        _ => Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "unknown-operation",
            "operation": operation,
        }))),
    }
}

#[cfg(test)]
mod tests;
