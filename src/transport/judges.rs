//! `judge-workspaces` — one bare working directory per judge, and stop there.
//!
//! `skill/policies/judgment-day.md` states the panel's isolation rule and used
//! to end it by saying where those directories come from was not decided. This
//! is the operation that decides it: one directory per judge, distinct from its
//! siblings, outside the checkout the run's claim covers, each created and read
//! back before it is reported. Requesting more than can be made refuses and
//! removes what it made — it never answers a short list, which is how a caller
//! composes one shared directory by hand.
//!
//! The directories are **bare working directories**, not git checkouts of the
//! reviewed head. Under `Evidence standard: reading` a judge needs neither; the
//! other standard needs a buildable tree only where the repository configures
//! one, which this operation does not do.
//!
//! It answers; it does not prove. Nothing here can see whether a judge stayed in
//! the directory it was handed, or whether a judge was launched at all.

use std::path::{Path, PathBuf};

use super::{Context, Failure, worktree};

/// The most judges one call will mint directories for.
///
/// A caller-supplied count with no ceiling is an unbounded filesystem write:
/// the operation makes a directory per judge, so a panel of `u64::MAX` is a loop
/// the filesystem never ends. Far above any panel this contract convenes.
pub const MAX_JUDGES: u64 = 64;

/// Everything one `judge-workspaces` needs.
#[derive(Debug, Clone)]
pub struct Panel<'a> {
    /// The issue the panel reviews for, as its tracker spells it.
    pub issue: crate::identity::Identity,
    /// The run convening the panel.
    pub run_id: &'a str,
    /// The publication epoch the judges are reviewing.
    pub epoch: &'a str,
    /// The pull request the publication reached.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target manifest digest.
    pub digest: &'a str,
    /// How many judges the panel has, and so how many directories it needs.
    pub count: u64,
    /// A worktree template overriding the configured one.
    pub worktree_root: Option<&'a str>,
}

/// The directory the panel's judge workspaces are minted under.
///
/// Resolved the way [`super::branch::start_branch`] resolves the claimed
/// checkout — from `Worktree location`, with the same `<repo>`, `<branch>` and
/// `<run-id>` scopes — so the panel's directories land beside the claimed
/// checkout rather than inside it, and two runs never compose one path. The
/// receipt's head scopes the name, so a republish mints fresh directories
/// rather than reusing a panel's.
pub fn panel_root(
    template: &str,
    repo_name: &str,
    run_id: &str,
    issue: &crate::identity::Identity,
    head: &str,
) -> Result<PathBuf, Failure> {
    let (scoped, _) = worktree::scoped_template(template);
    let short: String = head.chars().take(12).collect();
    let label = format!("judges-{short}");
    worktree::worktree_path(&scoped, repo_name, &label, run_id, issue)
}

/// Whether a write at `target` lands inside the checkout the claim covers.
///
/// The claim's own predicate, so the operation refuses exactly what the gate
/// would measure against the claim. A path this process cannot place is read as
/// inside: the asymmetry runs one way, and loosening a gate is the direction
/// that costs.
fn inside_the_claimed_checkout(checkout: &Path, target: &Path) -> bool {
    match crate::paths::placed(target) {
        Some(placed) => crate::paths::covers(checkout, &placed),
        None => true,
    }
}

fn repo_name(context: &Context) -> Result<String, Failure> {
    match context.repo.as_deref() {
        Some(named) => Ok(named.rsplit('/').next().unwrap_or(named).to_owned()),
        None => Ok(super::closing::repo_identity(context)?.1),
    }
}

fn inside_checkout_refusal(path: &Path, checkout: &Path) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "judge-workspaces-inside-the-claimed-checkout",
        "path": path.display().to_string(),
        "checkout": checkout.display().to_string(),
        "action": "the resolved panel directory is inside the checkout this run's claim covers, \
                   so a judge given it would be measured against that claim and allowed to write \
                   into the delivery target. Point `Worktree location` outside the checkout, as \
                   judgment-day.md requires, and re-run",
    }))
}

fn uncreatable_refusal(requested: u64, creatable: u64, root: &Path, detail: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "judge-workspaces-uncreatable",
        "requested": requested,
        "creatable": creatable,
        "root": root.display().to_string(),
        "detail": detail,
        "action": "a panel of N judges needs one directory each and this run will not share one; \
                   nothing was kept. Clear what is blocking the directory and re-run, or use a \
                   separate session or durable handoff",
    }))
}

/// `judge-workspaces` — mint one bare working directory per judge.
///
/// Everything that can refuse runs before the first directory is made, so a
/// panel this operation will not fill leaves no half-built tree behind. The
/// three properties the isolation rule states are then checked against the
/// directories the call actually read back, never against the paths it composed.
pub fn judge_workspaces(
    context: &Context,
    what: &Panel<'_>,
    configured: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let template = what.worktree_root.or(configured).unwrap_or_default();
    if template.is_empty() || template.eq_ignore_ascii_case("unset") {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "no-worktree-location",
            "action": super::branch::no_worktree_location_action(),
        })));
    }
    super::branch::validate_worktree_override(what.worktree_root)?;
    if what.count == 0 {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "malformed-argument",
            "argument": "--count",
            "value": "0",
            "detail": "a panel counts from 1",
        })));
    }
    if what.count > MAX_JUDGES {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "judge-workspaces-count-too-large",
            "requested": what.count,
            "max": MAX_JUDGES,
            "action": "convene the panel with at most 64 judges, or run more than one panel",
        })));
    }

    let repo = repo_name(context)?;
    let root = panel_root(template, &repo, what.run_id, &what.issue, what.head)?;
    if inside_the_claimed_checkout(&context.repo_dir, &root) {
        return Err(inside_checkout_refusal(&root, &context.repo_dir));
    }

    if let Err(error) = std::fs::create_dir_all(&root) {
        return Err(uncreatable_refusal(
            what.count,
            0,
            &root,
            &format!("could not create {}: {error}", root.display()),
        ));
    }

    let mut made: Vec<PathBuf> = Vec::new();
    let mut verified: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut detail = String::new();
    for judge in 1..=what.count {
        let target = root.join(format!("judge-{judge}"));
        if !target.is_dir() {
            if let Err(error) = std::fs::create_dir(&target) {
                if detail.is_empty() {
                    detail = format!("could not create {}: {error}", target.display());
                }
                continue;
            }
            made.push(target.clone());
        }
        match std::fs::canonicalize(&target) {
            Ok(resolved) if resolved.is_dir() => verified.push((target, resolved)),
            Ok(_) => {
                if detail.is_empty() {
                    detail = format!("{} was created and is not a directory", target.display());
                }
            }
            Err(error) => {
                if detail.is_empty() {
                    detail = format!(
                        "{} was created and could not be read back: {error}",
                        target.display()
                    );
                }
            }
        }
    }

    if verified.len() as u64 != what.count {
        discard(&made);
        return Err(uncreatable_refusal(
            what.count,
            verified.len() as u64,
            &root,
            &detail,
        ));
    }

    let distinct: std::collections::BTreeSet<&Path> = verified
        .iter()
        .map(|(_, resolved)| resolved.as_path())
        .collect();
    let nested = verified.iter().any(|(_, left)| {
        verified
            .iter()
            .any(|(_, right)| left != right && left.starts_with(right))
    });
    let inside = verified
        .iter()
        .find(|(_, resolved)| inside_the_claimed_checkout(&context.repo_dir, resolved));
    if distinct.len() as u64 != what.count || nested || inside.is_some() {
        discard(&made);
        return match inside {
            Some((target, _)) => Err(inside_checkout_refusal(target, &context.repo_dir)),
            None => Err(uncreatable_refusal(
                what.count,
                verified.len() as u64,
                &root,
                "the directories read back are not distinct and unnested",
            )),
        };
    }

    let directories: Vec<String> = verified
        .iter()
        .map(|(target, _)| target.display().to_string())
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "run_id": what.run_id,
        "epoch": what.epoch,
        "pr": what.pr,
        "head": what.head,
        "base": what.base,
        "digest": what.digest,
        "requested": what.count,
        "root": root.display().to_string(),
        "directories": directories,
        "bare": true,
    }))
}

/// Removes what this call made and leaves what it found, best effort.
///
/// Only the empty directories this call created are removed; a directory that
/// was already there is somebody else's and is never touched. A failure to
/// remove is left to the refusal, which already says nothing was kept.
fn discard(made: &[PathBuf]) {
    for directory in made {
        let _ = std::fs::remove_dir(directory);
    }
}

#[cfg(test)]
mod tests;
