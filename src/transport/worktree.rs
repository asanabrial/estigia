//! Where a run's checkout lands — and why two runs can never land in one place.
//!
//! Path uniqueness is the whole job. A run ID names a directory; if two distinct
//! IDs can fold into one name, two runs share a checkout and each one's commits
//! land in the other's tree. So every transformation here is checked for
//! injectivity, and anything that could fold is **refused** rather than applied —
//! except a branch, whose separators are escaped instead.

use super::Failure;

/// Names Windows resolves to a device rather than a file, in any case.
const DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// What joins a scope a template did not name to the template that lacks it.
///
/// Git rejects `~` in a ref name and the run-ID alphabet rejects it too, so it
/// appears in **neither** value it joins: every part of the composed name still
/// ends where the next begins, and two different pairs cannot spell one
/// directory.
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
/// `<run-id>` every run of a branch gets the same directory; without `<branch>`
/// every branch of a run gets the same directory. Neither is a stale default to
/// reject — both are persisted operator policy — so both are migrated **in
/// memory**, and the operator's own policy file is never rewritten. The migrated
/// path is a **sibling**, not a nested child, so a legacy directory is neither
/// parent nor child of the new one.
///
/// The join is [`SCOPE_JOIN`], which is not cosmetic: joining with `-` makes the
/// **composed** name ambiguous even though each half is unambiguous — branch
/// `fix-6` with run `a-b` and branch `fix-6-a` with run `b` both spell
/// `fix-6-a-b`, and nothing downstream would catch them sharing one directory.
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

/// A branch as a single path component, injectively.
///
/// `+` is the escape: `+` becomes `+p`, `/` becomes `+s` and `\` becomes `+b`,
/// and every other character is unchanged. Because the escape is itself
/// escaped, no two branches spell one component — `fix/1-a`, `fix-1-a`,
/// `fix//1-a`, `fix\1-a`, `fix+1-a` and `fix+s1-a` are six components — and a
/// branch with none of the three characters keeps the component it always had.
/// `+` is safe in paths and in file URLs, which `%` is not for several build
/// tools.
pub fn branch_component(branch: &str) -> String {
    let mut encoded = String::with_capacity(branch.len());
    for c in branch.chars() {
        match c {
            '+' => encoded.push_str("+p"),
            '/' => encoded.push_str("+s"),
            '\\' => encoded.push_str("+b"),
            other => encoded.push(other),
        }
    }
    encoded
}

/// Resolves the configured worktree template.
///
/// The repository name and the issue number are flattened; the branch is
/// encoded by [`branch_component`] instead, because folding a branch's
/// separators is not injective and a branch is what two runs of one root
/// routinely differ by. The issue is a `u64`, whose decimal spelling cannot
/// carry a separator, so the fold is the identity on it. The repository name is
/// `owner/repo` as the tracker spells it, so refusing its separator would refuse
/// every ordinary call; the residual collision — two clones with a folded
/// repository name pointed at one root — is the operator's own configuration.
/// `..` segments are rejected outright for the reason the fold exists to serve.
///
/// Placeholders are substituted in **one pass** over the template, so text a value
/// carries is never read as a placeholder: branch `a<run-id>` and `aR` under run
/// `R` stay two paths. A component that `trim()` would change is refused rather
/// than trimmed, because git allows U+00A0 and U+2003 in a ref name and trimming
/// would hand `\u{a0}fix` the component of `fix`.
pub fn worktree_path(
    template: &str,
    repo: &str,
    branch: &str,
    run_id: &str,
    issue: &crate::identity::Identity,
) -> Result<std::path::PathBuf, Failure> {
    let flatten = |value: &str, kind: &str| -> Result<String, Failure> {
        let cleaned = if kind == "branch" {
            branch_component(value)
        } else {
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
            cleaned
        };
        // Before the whitespace check below: the two sides do not trim the same set —
        // `str.strip()` treats U+001C–U+001F as whitespace and `str::trim()`
        // does not — so a component led by one named two different directories.
        // Refused rather than reconciled, which makes the two agree by
        // construction.
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
        // `.` beside `..`, and rejected rather than folded: `pathlib` folds it
        // away while `PathBuf` keeps it, so the two sides would otherwise name
        // two different directories for the same branch.
        if cleaned.is_empty()
            || cleaned.trim() != cleaned
            || cleaned == "."
            || cleaned.contains("..")
        {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "unsafe-worktree-component",
                "component": value,
                "kind": kind,
                "action": "a worktree path component may not be empty, `.`, contain `..`, \
                           carry a control character, or begin or end with whitespace",
            })));
        }
        assert_safe_component(&cleaned, kind)?;
        Ok(cleaned)
    };

    let repo = flatten(repo, "repo")?;
    let branch = flatten(branch, "branch")?;
    let run = run_component(run_id)?;
    let issue = flatten(&issue.to_string(), "issue")?;

    let mut resolved = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('<') {
        resolved.push_str(&rest[..open]);
        let tail = &rest[open..];
        let named = [
            ("<repo>", &repo),
            ("<branch>", &branch),
            ("<run-id>", &run),
            ("<issue>", &issue),
        ]
        .into_iter()
        .find(|(token, _)| tail.starts_with(token));
        match named {
            Some((token, value)) => {
                resolved.push_str(value);
                rest = &tail[token.len()..];
            }
            None => {
                resolved.push('<');
                rest = &tail[1..];
            }
        }
    }
    resolved.push_str(rest);
    Ok(std::path::PathBuf::from(resolved))
}

/// Where a fallback `git branch` should start — or `None` when it must **not**
/// run at all. **Never move an existing ref**: `--force` on an existing branch
/// rewinds it to the base, discarding every commit on it.
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
/// Pure, and fed answers rather than asked to obtain them: every refusal below
/// is a state a concurrent writer, or the operator's own editor, can produce
/// between two git calls.
///
/// **It may never cost work, and it may never fail the run** — advancing the
/// base is a convenience, so every state that cannot be *proved* safe holds
/// instead of proceeding, and a hold is reported rather than raised:
///
/// 1. **Diverged.** Moving the ref discards commits the remote has never seen.
/// 2. **Ancestry unknown.** An unreadable answer must not be spent as a safe
///    one.
/// 3. **Checked out and dirty.** Moving the ref under a working tree that has
///    changes leaves the index describing a commit the files do not match.
/// 4. **Checked out in another worktree.** `git update-ref` does **not** refuse
///    this — measured, it exits 0 — and the tree this run cannot see is not the
///    one whose status it can read, so clean or dirty is not consulted.
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
/// Pure, and fed answers rather than asked to obtain them: the failure it
/// guards is a **race**, which cannot be reproduced by calling the real thing
/// in a test.
///
/// A **fresh** native creation is the strict case: `gh issue develop` branches
/// from the base as the *server* sees it, and if the base moved in between,
/// reporting the recorded base as this branch's base is a lie that outlives the
/// command. So fresh requires local == remote == base, exactly.
///
/// A **resume** is looser in one direction only. Local ahead of remote is
/// ordinary unpushed work. Remote ahead of local means somebody else pushed to
/// this branch, and continuing would build on a head this checkout has never
/// seen; `reachable` is `None` when it could not be established, which is not a
/// pass.
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
