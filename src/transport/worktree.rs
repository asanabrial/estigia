//! Where a run's checkout lands — and why two runs can never land in one place.
//!
//! Path uniqueness is the whole job. A run ID names a directory; if two distinct
//! IDs can fold into one name, two runs share a checkout and each one's commits
//! land in the other's tree. So every transformation here is checked for
//! injectivity, and anything that could fold is **refused** rather than applied.

use super::Failure;

/// Names Windows resolves to a device rather than a file, in any case.
const DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// What joins a scope a template did not name to the template that lacks it.
///
/// Git rejects `~` in a ref name and the run-ID alphabet rejects it too, so it
/// appears in **neither** value it joins. A template can therefore gain both a
/// branch and a run ID — two joins, three parts — and every part still ends
/// where the next begins. Nothing here splits the composed name back apart;
/// what the property buys is that two different pairs cannot spell one
/// directory, which is the collision the join exists to prevent.
pub const SCOPE_JOIN: char = '~';

/// Refuses a path component that does not name a plain directory on every
/// supported platform.
///
/// The device check is on the part **before the first dot**, because Windows
/// resolves `con.txt`, `nul.log` and `com1.anything` to the device just as it
/// resolves the bare name. A run ID can never reach that branch — dots are not
/// in its alphabet — but a flattened branch or repository name can, and this is
/// shared by all of them.
pub fn assert_safe_component(value: &str, kind: &str) -> Result<(), Failure> {
    let head = value.split_once('.').map_or(value, |(head, _)| head);
    if DEVICE_NAMES.contains(&head.to_lowercase().as_str()) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "reserved-device-component",
            "component": value,
            "kind": kind,
            "action": format!(
                "`{value}` is a reserved Windows device name in any case, so it names a \
                 device rather than a directory — choose another value"
            ),
        })));
    }
    Ok(())
}

/// The run ID as a path component.
///
/// Lowercase `a-z0-9` groups joined by single `-`, and **nothing else**. It is
/// not flattened like the other components: flattening is a transformation, and
/// any transformation that could fold two distinct IDs into one is refused
/// rather than applied. The alphabet is lowercase by construction, because the
/// identity that separates two checkouts has to survive a filesystem that folds
/// case.
pub fn run_component(text: &str) -> Result<String, Failure> {
    let well_formed = !text.is_empty()
        && text.split('-').all(|group| {
            !group.is_empty()
                && group
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if !well_formed {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "unsafe-run-id",
            "run_id": text,
            "action": "a run ID must be lowercase `a-z0-9` groups joined by single `-`; it \
                       names a directory that must not collide with another run's, and any \
                       transformation that could fold two distinct IDs into one is refused \
                       rather than applied",
        })));
    }
    assert_safe_component(text, "run-id")?;
    Ok(text.to_owned())
}

/// A template that carries both `<branch>` and `<run-id>`, and whether either
/// had to be added.
///
/// **Two dimensions, and a template missing either one collides.** Without
/// `<run-id>` every run of a branch gets the same directory. Without
/// `<branch>` every branch of a run gets the same directory — which is the whole
/// of issue #27: a run working a queue meets the checkout it made for its
/// previous issue, and the `worktree-path-occupied` it is handed is correct for
/// what it can see. What is wrong is that two branches of one run were ever
/// asked to share a path.
///
/// Only the first was migrated, and the asymmetry stayed invisible because the
/// operator who met it had since configured a template naming `<branch>` — so a
/// later run measured four checkouts, no collision, and read that as a fix. That
/// is the trap this function is now shaped against: the documented accepted
/// value for `Worktree location` is *"an absolute directory"*, naming no
/// placeholder at all, and the skill ships the row `unset`. A plain directory is
/// the ordinary thing to write, and it is the one shape that carries **both**
/// collisions at once.
///
/// Neither is a stale default to reject: both are persisted operator policy,
/// already in use, and a run that stops dead on one has lost work to protect
/// against losing work. So both are migrated **in memory**, and the operator's
/// own policy file is never rewritten.
///
/// A sibling rather than a nested child, both times, so the legacy directory —
/// if it exists — is neither a parent of nor a child of the new one. Colliding
/// inside a directory somebody else owns is the defect itself, and making the
/// new path a descendant of the old one would reproduce it. The branch dimension
/// needs that more sharply than the run one: a run-scoped legacy path **is** a
/// checkout this run owns, so nesting the new one under it would put a worktree
/// inside a worktree.
///
/// The join is [`SCOPE_JOIN`], and it is not cosmetic: joining with `-` makes
/// the **composed** name ambiguous even though each half is unambiguous. Branch
/// `fix/6` with run `a-b` and branch `fix/6-a` with run `b` both spell
/// `fix-6-a-b`. Those are two different branches, so they take two different
/// branch locks, and nothing downstream would catch them sharing one directory.
pub fn scoped_template(template: &str) -> (String, bool) {
    let mut scoped = template.trim_end_matches(['/', '\\']).to_owned();
    let mut migrated = false;
    // The branch before the run, so a template naming neither ends
    // `…~<branch>~<run-id>`: the run is then the outermost scope, and every
    // directory one run owns shares a suffix rather than a prefix nobody can
    // read off the end.
    for placeholder in ["<branch>", "<run-id>"] {
        if !scoped.contains(placeholder) {
            scoped = format!("{scoped}{SCOPE_JOIN}{placeholder}");
            migrated = true;
        }
    }
    (scoped, migrated)
}

/// Resolves the configured worktree template.
///
/// Every substituted value except the run ID is flattened. The branch is the one
/// that carries a `/` in normal use, but the reason — a separator in a
/// substituted value silently *restructures* the path instead of naming a
/// directory — applies identically to the repository name and the issue number.
/// `..` segments are rejected outright for the same reason.
pub fn worktree_path(
    template: &str,
    repo: &str,
    branch: &str,
    run_id: &str,
    issue: u64,
) -> Result<std::path::PathBuf, Failure> {
    let flatten = |value: &str, kind: &str| -> Result<String, Failure> {
        let mut cleaned = String::with_capacity(value.len());
        let mut separator = false;
        for c in value.chars() {
            if c == '/' || c == '\\' {
                if !separator {
                    cleaned.push('-');
                }
                separator = true;
            } else {
                cleaned.push(c);
                separator = false;
            }
        }
        // Before the trim, not after: the two sides do not trim the same set.
        // `str.strip()` treats U+001C–U+001F — the file, group, record and unit
        // separators — as whitespace and `str::trim()` does not, so a component
        // led by one reached the device check stripped on one side and whole on
        // the other — `con` there, and the whole thing here —
        // and any component carrying one of them named two different
        // directories. Refused here rather than reconciled, because a control
        // character in a path component is not a thing anyone meant to ask for,
        // and refusing makes the two agree by construction — the same move the
        // `.` case below took, for the same reason.
        if cleaned.chars().any(char::is_control) {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "unsafe-worktree-component",
                "component": value,
                "kind": kind,
                "action": "a worktree path component may not be empty, `.`, contain `..`, \
                           or carry a control character",
            })));
        }
        let cleaned = cleaned.trim().to_owned();
        // `.` beside `..`, and for a reason the other one does not have: a
        // single dot names nothing, and the two sides disagreed about it.
        // `pathlib` folds it away while `PathBuf` keeps it, so a branch called
        // `.` put the checkout at `<root>/acme/./<run>` here and
        // `<root>/acme/<run>` there — the transport creating one directory
        // while the gate records another. Measured on the pair.
        //
        // Rejected rather than folded, because the two sides then agree by
        // construction instead of by hoping a filesystem forgives the
        // difference.
        if cleaned.is_empty() || cleaned == "." || cleaned.contains("..") {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "unsafe-worktree-component",
                "component": value,
                "kind": kind,
                "action": "a worktree path component may not be empty, `.`, contain `..`, \
                           or carry a control character",
            })));
        }
        assert_safe_component(&cleaned, kind)?;
        Ok(cleaned)
    };

    let resolved = template
        .replace("<repo>", &flatten(repo, "repo")?)
        .replace("<branch>", &flatten(branch, "branch")?)
        .replace("<run-id>", &run_component(run_id)?)
        .replace("<issue>", &flatten(&issue.to_string(), "issue")?);
    Ok(std::path::PathBuf::from(resolved))
}

/// Where a fallback `git branch` should start — or `None` when it must **not**
/// run at all.
///
/// This decides whether existing work survives a resume, so it is a pure
/// function fed booleans rather than a line inside the git sequence: the defect
/// it exists to prevent shipped precisely because the sequence had no test.
///
/// **Never move an existing ref.** The original was `git branch --force <branch>
/// origin/<base>`, and `--force` on a branch that already exists *rewinds* it to
/// the base, discarding every commit on it. The `--force` was there to make the
/// resume case work — which is exactly the case where the branch has work to
/// lose, so it destroyed what it was meant to accommodate.
///
/// Precedence is ordered by how much work each source can lose:
/// 1. a **local** branch is authoritative and is left alone — it may hold
///    commits never pushed;
/// 2. otherwise a **remote** branch of the same name is the published work, so
///    branch from *that*, not from the base — starting a resumed branch at the
///    base silently restarts it from zero;
/// 3. only when neither exists is this genuinely a new branch off the base.
pub fn branch_start_point(
    exists_local: bool,
    exists_remote: bool,
    branch: &str,
    base: &str,
) -> Option<String> {
    if exists_local {
        return None;
    }
    Some(if exists_remote {
        format!("origin/{branch}")
    } else {
        format!("origin/{base}")
    })
}

/// What `start-branch` may do to the caller's local base branch.
///
/// Two distinct permissions rather than one boolean, because the safe command
/// is not the same in both cases: moving a ref nothing has checked out touches
/// no file, while moving one that *is* checked out has to carry the index and
/// the working tree with it. Collapsing them would mean picking one command for
/// two situations, and the wrong pick in either direction corrupts a checkout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseAdvance {
    /// Nothing to move: no local base branch, or it is already at the remote.
    Current,
    /// Move the ref alone. No working tree is involved, so no file changes.
    UpdateRef,
    /// The base is checked out and clean, so git moves ref, index and files.
    FastForwardCheckout,
    /// Refused. The string is the reason, and it is reported, never guessed at.
    Hold(&'static str),
}

/// Whether the local base branch may be advanced to the fetched remote tip.
///
/// [`branch_preflight`](super::branch) already refreshes
/// `refs/remotes/origin/<base>` before a branch is created, so a **new** branch
/// has always started from the current base — that part was never the problem.
/// What stayed behind was `refs/heads/<base>` itself: the operator's own `main`
/// kept reading stale in the primary checkout even though every worktree cut
/// from it was current. This closes that gap, and only that one.
///
/// Pure, and fed answers rather than asked to obtain them, for the same reason
/// [`branch_start_point`] is: every refusal below is a state a concurrent
/// writer, or the operator's own editor, can produce between two git calls.
///
/// **It may never cost work, and it may never fail the run.** Advancing the
/// base is a convenience — the worktree's start point does not depend on it —
/// so every state that cannot be *proved* safe holds instead of proceeding, and
/// a hold is reported rather than raised. The three refusals each name a
/// different way the naive `git pull` here would have gone wrong:
///
/// 1. **Diverged.** The local base holds commits the remote has never seen.
///    Moving the ref discards them. This is the same lesson `--force` taught
///    [`branch_start_point`], at a different ref.
/// 2. **Ancestry unknown.** `git merge-base` could not answer. `An unknown
///    result is not clearance` (`openspec/config.yaml`) — an unreadable answer
///    must not be spent as a safe one.
/// 3. **Checked out and dirty.** Moving the ref under a working tree that has
///    changes leaves the index describing a commit the files do not match, and
///    every later `git status` in that checkout misreports what it holds.
/// 4. **Checked out in another worktree.** The same corruption, in the tree this
///    run cannot see — and linked checkouts are what this crate makes for a
///    living. `git update-ref` does **not** refuse this; measured, it exits 0,
///    and the untouched worktree then reports `D <file>` for a file nobody
///    deleted. Clean or dirty is not consulted here, because the tree at risk is
///    not the one whose status this run can read.
pub fn base_advance(
    local: Option<&str>,
    remote: &str,
    reachable: Option<bool>,
    checked_out: bool,
    dirty: bool,
    checked_out_elsewhere: bool,
) -> BaseAdvance {
    // No local base, or it is already there. Reported as Current and not as a
    // refusal: a refusal names a problem, and neither of these is one.
    let Some(local) = local else {
        return BaseAdvance::Current;
    };
    if local == remote {
        return BaseAdvance::Current;
    }
    match reachable {
        None => BaseAdvance::Hold("ancestry-unknown"),
        Some(false) => BaseAdvance::Hold("base-diverged"),
        // Before the `!checked_out` arm, which would otherwise read "no working
        // tree is involved" from the only tree this run can see.
        Some(true) if checked_out_elsewhere => BaseAdvance::Hold("base-checked-out-elsewhere"),
        Some(true) if !checked_out => BaseAdvance::UpdateRef,
        Some(true) if dirty => BaseAdvance::Hold("base-checked-out-dirty"),
        Some(true) => BaseAdvance::FastForwardCheckout,
    }
}

/// Whether the local head, the remote branch head and the recorded base tell
/// one coherent story.
///
/// Pure, and fed answers rather than asked to obtain them, for the same reason
/// [`branch_start_point`] is: the failure it guards is a **race**, and a race
/// cannot be reproduced by calling the real thing in a test. Every branch below
/// is a state some concurrent writer can actually produce.
///
/// A **fresh** native creation is the strict case. `gh issue develop` branches
/// from the base as the *server* sees it, which is not necessarily the base this
/// run fetched and recorded. If the base moved in between, the remote branch
/// starts at a commit the run never saw, and reporting the recorded base as this
/// branch's base is a lie that outlives the command — every later
/// "reviewed-base" claim inherits it. So fresh requires local == remote == base,
/// exactly.
///
/// A **resume** is looser in one direction only. Local ahead of remote is
/// ordinary unpushed work. Remote ahead of local is not: it means somebody else
/// pushed to this branch, and continuing would build on a head this checkout has
/// never seen. `reachable` carries the ancestry answer, and `None` — it could
/// not be established — is not a pass.
pub fn branch_identity_verdict(
    fresh: bool,
    local: Option<&str>,
    remote: Option<&str>,
    base: Option<&str>,
    reachable: Option<bool>,
) -> (bool, &'static str) {
    let Some(remote) = remote else {
        return (true, "local-only");
    };
    if fresh {
        if base.is_some() && local == Some(remote) && local == base {
            return (true, "fresh-at-base");
        }
        return (
            false,
            if local == base {
                "base-moved-during-creation"
            } else {
                "fresh-branch-diverged"
            },
        );
    }
    if local == Some(remote) {
        return (true, "resumed-in-sync");
    }
    match reachable {
        Some(true) => (true, "resumed-local-ahead"),
        Some(false) => (false, "remote-not-reachable-from-local"),
        None => (false, "ancestry-unknown"),
    }
}

#[cfg(test)]
mod tests;
