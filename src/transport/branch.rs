//! `start-branch` — reserve an isolated checkout **locally**, and stop there.
//!
//! The order is the point. Every local reservation that can fail — the path, the
//! branch, the ownership marker, the worktree itself — runs before the first
//! remote mutation, so a run that loses the race, or finds a stranger's
//! directory, or cannot prove it owns its own, leaves **nothing** behind on
//! GitHub. The order this replaces created a server-side branch and a sidebar
//! link first, and could then stop on a local check — leaving remote state
//! advertising work no checkout was ever made for.
//!
//! The **branch reservation** is what makes that ordering worth anything with
//! two runs on one machine, and it had no port at all: [`reserve_branch`] is
//! new. Its absence was invisible to every crossing there is, because a
//! crossing runs one process at a time and an uncontended lock leaves nothing
//! in the envelope — so the oracle reported agreement about a mechanism one
//! side did not have. It is measured here rather than there for the same
//! reason: what it prevents needs two runs, and the differential poses one.
//!
//! **The link is ported now**, and it was the half whose absence lost real
//! behaviour: [`develop_link`] runs `gh issue develop` — one call that creates
//! the remote ref *and* the issue's Development sidebar link — and then
//! [`is_branch_linked`] re-reads the sidebar rather than believing an exit code,
//! because a nonzero exit covers three states and in two of them the link is
//! there. Until it existed, this side stopped after the local reservation and
//! went on to `git fetch`, so the branch existed and the issue showed nothing,
//! and the two sides disagreed about every `start-branch` that got that far.
//! `the_port_still_does_not_link_a_branch_to_its_issue` holds the pair together
//! in both directions: it fails if the link is ported and the honesty contract
//! still names the gap, and if the entry goes while the port still cannot link.
//!
//! The ownership marker is kept where the binding keeps it now, and it was not:
//! `ownership_path` asks git for the checkout's **private admin directory**
//! and writes `issue-flow-owner.json` inside it, for the reasons the binding
//! gives — a marker in the working tree is committed by accident, shows in
//! every `git status` so people delete it, and outlives `git worktree remove`
//! as a stale claim. This side wrote `.estigia/worktree.json` in the tree, so
//! neither could see the other's, and a checkout one had reserved read as
//! *unmarked* to the other — which is the state whose refusal tells an agent
//! nobody owns this directory and to remove it.

use super::{Context, Failure, How, run, worktree};

/// A branch head, or a real absence — never a failed read read as an absence.
///
/// **Not** `git rev-parse --verify --quiet`, which cannot answer this question:
/// it exits `1` for "no such ref" *and* for a corrupt object store, an
/// unreadable `.git`, a broken packed-refs file — every way the question can go
/// unanswered. A run that reads "absent" from a failed read creates the branch
/// again from the base, and a resumed branch silently restarts from zero.
///
/// `for-each-ref` separates the two: a successful exit with no output is a real
/// absence, with one object id a real presence, and anything else is a read that
/// did not answer.
pub fn ref_object(at: &std::path::Path, reference: &str) -> Result<Option<String>, Failure> {
    // `--end-of-options` closes option parsing, so a ref pattern beginning with
    // `-` is a pattern and not a flag. Nothing upstream validates branch names.
    let answer = run(
        &[
            "git",
            "for-each-ref",
            "--format=%(refname)%09%(objectname)",
            "--end-of-options",
            reference,
        ],
        Some(at),
        How::tolerated(),
    )?;
    if answer.status != 0 {
        return Err(Failure::Read(format!(
            "git for-each-ref {reference} failed ({}): {}",
            answer.status,
            answer.stderr.trim()
        )));
    }
    let matched = exact_refs(&answer.stdout, reference)?;
    match matched.as_slice() {
        [] => Ok(None),
        [only] if is_object_id(only) => Ok(Some(only.clone())),
        _ => Err(Failure::Read(format!(
            "git for-each-ref {reference} answered ambiguously: {matched:?}"
        ))),
    }
}

/// The object ids `for-each-ref` reported for **exactly** this refname.
///
/// The argument is a *pattern*, not a path, so the refname is matched again
/// here. A `for-each-ref` pattern matches a ref completely **or from the
/// beginning up to a slash**, so `refs/heads/foo` also matches
/// `refs/heads/foo/bar` — and asking about a branch `foo` that does not exist
/// would otherwise return the child's object id. That is not merely unhelpful:
/// `foo` would be reported as present at a commit belonging to another branch.
/// Children are ignored rather than mistaken for the parent.
fn exact_refs(output: &str, reference: &str) -> Result<Vec<String>, Failure> {
    let mut matched = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((name, oid)) = line.split_once('\t') else {
            return Err(Failure::Read(format!(
                "git for-each-ref {reference} answered in an unexpected format: {line:?}"
            )));
        };
        if name == reference {
            matched.push(oid.trim().to_owned());
        }
    }
    Ok(matched)
}

/// Whether this is a full object id in either hash.
fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

/// One spelling for a path, so two readings of the same directory compare equal.
///
/// Necessary on Windows, where git reports `H:/REPO/…` and a resolved path is
/// `H:\REPO\…`; a raw comparison silently says "different directory" and turns a
/// resume into a refusal. Case is folded **only** where the filesystem folds it:
/// on POSIX two directories differing only by case are two directories, and
/// lowercasing them into one key would report the wrong worktree as the owner of
/// a path — the single fact the resume-versus-refuse decision rests on.
pub fn normalise_path(path: &std::path::Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    spelling(&resolved)
}

/// One spelling for a path **without resolving it**.
///
/// Separators and case only. [`canonical_worktree_path`] compares a resolved
/// path against a half-resolved one, and resolving again here would collapse
/// exactly the difference it is looking for: the linked leaf and its target
/// would come back identical, and the aliased directory would be accepted.
fn spelling(path: &std::path::Path) -> String {
    // The verbatim prefix off first. `fs::canonicalize` answers with `\\?\C:\…`
    // on Windows and `Path.resolve()` does not, so every existing path spelled
    // one way here came back `//?/c:/…` and the other way `c:/…` — the two
    // sides never agreeing about **any** directory on this crate's own
    // platform. Measured on eleven of thirteen.
    //
    // Nothing compares one side's spelling against the other's today: each
    // normalises both halves of its own comparison, so the prefix cancels and
    // there is no live fault. It is removed because the oracle exists to catch
    // this class before it crosses, and because `setup::executable_path`
    // already strips it with the same helper for the same reason.
    let path = crate::paths::remove_windows_verbatim_prefix(path.to_path_buf());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        // Unix paths are bytes. Lossy UTF-8 would collapse two distinct linked
        // leaves onto the same U+FFFD spelling and let an alias pass as its own
        // target. Hex is only an internal comparison key; Git still receives
        // the original OsStr through `run_os`.
        //
        // The tail of the function rather than a `return`: on unix everything
        // below is compiled out, so this block *is* the last expression, and
        // clippy says so — on the two platforms a Windows-only lint run never
        // builds. `-D warnings` turned that into a red CI on a green desk.
        path.as_os_str()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    #[cfg(not(unix))]
    let text = path
        .display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_owned();
    #[cfg(not(unix))]
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Resolves the ancestors of a worktree path and refuses an aliased **leaf**.
///
/// Two separate jobs, because the two kinds of link are not the same problem.
///
/// An **ancestor** that is a symlink or a junction is ordinary — a worktree root
/// parked on another volume. It redirects every run's directory identically, so
/// it cannot fold two run IDs together, and refusing it would break working
/// setups for nothing. It is resolved instead, so the registry lookup, `git
/// worktree add` and the ownership marker all speak of one spelling. Windows'
/// `\\?\` prefix is removed from the returned path after resolution because Git
/// for Windows rejects its slash-normalised `//?/` form; the helper is an
/// identity operation on Linux and macOS.
///
/// The **leaf** being a link is the collapse. `run-b`'s directory pointing at
/// `run-a`'s is two distinct valid run IDs resolving to one real directory, and
/// no amount of resolution fixes it: whichever run arrives second is standing in
/// the first one's checkout. The ownership marker would catch it a moment later,
/// but "your own template aliases somebody else's directory" is a configuration
/// defect and deserves to be reported as one, at the point where nothing has
/// been created yet.
///
/// Isolating the leaf from its ancestors is the whole trick: resolve the parent,
/// re-attach the name, and compare against resolving the path as a whole. They
/// differ exactly when the last component redirects on its own account.
pub fn canonical_worktree_path(path: &std::path::Path) -> Result<std::path::PathBuf, Failure> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Ok(path.to_path_buf());
    };
    let canonical = std::fs::canonicalize(parent)
        .unwrap_or_else(|_| parent.to_path_buf())
        .join(name);
    let whole = std::fs::canonicalize(path).unwrap_or_else(|_| canonical.clone());
    if spelling(&whole) != spelling(&canonical) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "aliased-worktree-path",
            "path": path.display().to_string(),
            "resolves_to": whole.display().to_string(),
            "action": "this run's worktree directory is itself a symlink or junction pointing \
                       somewhere else, so two distinct run IDs can land in one real directory. \
                       Remove the link, or point the worktree template at a real directory tree",
        })));
    }
    Ok(crate::paths::remove_windows_verbatim_prefix(canonical))
}

/// The checkouts git has registered, by normalised path.
///
/// `-z` because a worktree path may contain a newline, and the line-oriented
/// form would then report one checkout as two — the second of which owns no
/// branch and would read as an unowned directory this run may take.
pub fn registered_worktrees(
    context: &Context,
) -> Result<std::collections::BTreeMap<String, Option<String>>, Failure> {
    let listing = run(
        &["git", "worktree", "list", "--porcelain", "-z"],
        Some(&context.repo_dir),
        How::tolerated(),
    )?;
    if listing.status != 0 {
        return Err(Failure::Read(format!(
            "git worktree list failed ({}): {}",
            listing.status,
            listing.stderr.trim()
        )));
    }
    // The original reads this call binary and decodes with `surrogateescape`,
    // saying why: *"a path byte that is not valid UTF-8 must survive as itself
    // rather than collapse into U+FFFD, because two undecodable paths would
    // otherwise normalise to one key and the registry would name the wrong
    // owner."* The port decodes with replacement, which does collapse them —
    // and it turns out **not** to name a wrong owner, because
    // `parse_worktree_listing` already refuses a repeated path. Measured rather
    // than assumed: refusing every replaced listing here was tried first and is
    // a regression, since a *single* undecodable worktree works correctly today
    // — the registry key and the live path are mangled the same way, so they
    // still match.
    //
    // What is left is the wording. "Repeated the path" describes `git` as
    // having said the same thing twice, when it said two different things this
    // build could not tell apart, and it sends a reader to look for a duplicate
    // that is not there.
    parse_worktree_listing(&listing.stdout).map_err(|failure| match failure {
        Failure::Read(detail)
            if listing.stdout_replaced && detail.contains("repeated the path") =>
        {
            Failure::Read(format!(
                "{detail} — the listing holds a path that is not UTF-8, so two checkouts \
                 this build cannot tell apart were read as one"
            ))
        }
        other => other,
    })
}

/// Reads `git worktree list --porcelain -z` into path → branch, or refuses it.
///
/// Records are separated by an **empty** NUL-terminated field; within a record
/// each attribute is its own field. A record with no `branch` line is a detached
/// checkout, which owns the directory just as firmly as a branch does — so the
/// path is present with `None` rather than absent.
///
/// **Refuses rather than interprets**, and the reason is the transport's, in its
/// own words: *this is the ONLY local authority that separates a resume from
/// writing into somebody else's checkout, and the defect it replaces was
/// precisely that an unanswered read looked identical to "no worktrees exist". A
/// parser that guesses at an incoherent record hands out that same false
/// clearance one level lower down.*
///
/// This side was that parser. It could not fail: empty output became an empty
/// registry, a stream stopping mid-record dropped the record, and an attribute
/// it did not know was skipped. Measured through `start-branch` with a world
/// that answered nothing for the listing — the transport refused the read and
/// this side went on to take the directory, because as far as it could tell
/// nobody held any.
///
/// Pure by construction, like the transport's: it takes the stream and touches
/// neither git nor the filesystem, so every rejection below is exercisable
/// without a repository.
/// Public for the crossing: the transport's half is pure by construction so
/// that every rejection is exercisable without a repository, and a rule this
/// side could only be shown through `start-branch` would be a rule crossed at
/// one shape.
pub fn parse_worktree_listing(
    output: &str,
) -> Result<std::collections::BTreeMap<String, Option<String>>, Failure> {
    let refuse = |why: String| Failure::Read(why);
    if output.is_empty() {
        return Err(refuse(
            "git worktree registry was empty; git lists at least the main worktree".to_owned(),
        ));
    }
    if !output.ends_with('\0') {
        return Err(refuse("git worktree registry ended mid-field".to_owned()));
    }

    let mut found = std::collections::BTreeMap::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut current: Option<WorktreeRecord> = None;

    let fields: Vec<&str> = output.split('\0').collect();
    for field in &fields[..fields.len() - 1] {
        if field.is_empty() {
            let Some(record) = current.take() else {
                return Err(refuse(
                    "git worktree registry terminated a record that never began".to_owned(),
                ));
            };
            let path = record.path.clone();
            let branch = record.finish()?;
            found.insert(normalise_path(std::path::Path::new(&path)), branch);
            continue;
        }

        let (label, value) = field.split_once(' ').unwrap_or((field, ""));
        if label == "worktree" {
            if current.is_some() {
                return Err(refuse(
                    "git worktree registry began a record before terminating the previous one"
                        .to_owned(),
                ));
            }
            if value.is_empty() {
                return Err(refuse(
                    "git worktree registry declared a worktree with no path".to_owned(),
                ));
            }
            if !seen.insert(value.to_owned()) {
                return Err(refuse(format!(
                    "git worktree registry repeated the path {value:?}"
                )));
            }
            current = Some(WorktreeRecord {
                path: value.to_owned(),
                ..WorktreeRecord::default()
            });
            continue;
        }

        let Some(record) = current.as_mut() else {
            return Err(refuse(format!(
                "git worktree registry field {label:?} preceded its worktree path"
            )));
        };
        match label {
            "HEAD" => {
                if record.head.is_some() {
                    return Err(refuse("git worktree record declared HEAD twice".to_owned()));
                }
                // Through `is_object_id`, which is already crossed against the
                // transport's own pattern rather than spelled a second time here.
                if !is_object_id(value) {
                    return Err(refuse(format!(
                        "git worktree record carried a malformed HEAD {value:?}"
                    )));
                }
                record.head = Some(value.to_owned());
            }
            "branch" => {
                if record.branch.is_some() {
                    return Err(refuse(
                        "git worktree record declared branch twice".to_owned(),
                    ));
                }
                let Some(name) = value
                    .strip_prefix("refs/heads/")
                    .filter(|name| !name.is_empty())
                else {
                    return Err(refuse(format!(
                        "git worktree record carried a non-branch ref {value:?}"
                    )));
                };
                record.branch = Some(name.to_owned());
            }
            "detached" | "bare" => {
                if !value.is_empty() {
                    return Err(refuse(format!(
                        "git worktree record gave the boolean {label:?} a value"
                    )));
                }
                let flag = if label == "detached" {
                    &mut record.detached
                } else {
                    &mut record.bare
                };
                if *flag {
                    return Err(refuse(format!(
                        "git worktree record declared {label:?} twice"
                    )));
                }
                *flag = true;
            }
            "locked" | "prunable" => {
                let slot = if label == "locked" {
                    &mut record.locked
                } else {
                    &mut record.prunable
                };
                if slot.is_some() {
                    return Err(refuse(format!(
                        "git worktree record declared {label:?} twice"
                    )));
                }
                *slot = Some(value.to_owned());
            }
            _ => {
                return Err(refuse(format!(
                    "unknown git worktree registry field {label:?}"
                )));
            }
        }
    }

    if current.is_some() {
        return Err(refuse(
            "git worktree registry stopped inside an unterminated record".to_owned(),
        ));
    }
    Ok(found)
}

/// One record of the porcelain registry, while it is being read.
#[derive(Default)]
struct WorktreeRecord {
    path: String,
    head: Option<String>,
    branch: Option<String>,
    detached: bool,
    bare: bool,
    locked: Option<String>,
    prunable: Option<String>,
}

impl WorktreeRecord {
    /// Rejects a record whose fields contradict each other or leave its state
    /// unstated, and answers the branch it is on.
    ///
    /// Split out for the transport's reason: the contradictions are named once,
    /// in one place, rather than accumulating inside the field loop where the
    /// next one added would be easy to misplace.
    fn finish(self) -> Result<Option<String>, Failure> {
        let path = &self.path;
        if self.bare {
            if self.head.is_some() || self.branch.is_some() || self.detached {
                return Err(Failure::Read(format!(
                    "git worktree record for {path:?} is bare and checked out at once"
                )));
            }
            return Ok(None);
        }
        if self.branch.is_some() && self.detached {
            return Err(Failure::Read(format!(
                "git worktree record for {path:?} is on a branch and detached at once"
            )));
        }
        if self.branch.is_none() && !self.detached {
            return Err(Failure::Read(format!(
                "git worktree record for {path:?} declared no branch, detached or bare state"
            )));
        }
        if self.head.is_none() {
            return Err(Failure::Read(format!(
                "git worktree record for {path:?} is checked out with no HEAD"
            )));
        }
        let _ = (&self.locked, &self.prunable);
        Ok(self.branch)
    }
}

/// Whether this run may take the directory its template resolved to.
///
/// Four outcomes, and telling them apart is the whole reason the ownership
/// marker exists. An **absent** path is fresh. A path this run already owns is
/// a **resume** — re-running the command must not destroy the checkout it made
/// last time. And there are **two** ways it can be somebody else's, not one.
///
/// This answered with a single refusal, `worktree-not-owned-by-this-run`, which
/// is a code the transport never emits and `SKILL.md` has never named. The
/// transport distinguishes three, and all three are in the prose, because the
/// agent is told to do something different for each: a directory holding
/// another branch, a directory holding this branch that nobody has claimed, and
/// a directory another run owns. Collapsing them handed an agent an unknown
/// refusal carrying the advice for the wrong one — in particular it withheld
/// the recovery, which is the only case that has one.
///
/// The `issue` this used to compare is gone with them. The transport decides on
/// the run-id, and the branch check now in front of it covers what comparing
/// issues was standing in for: a run's branch is derived from its issue, so the
/// same run on another issue arrives here on another branch and is refused as
/// `worktree-path-occupied` — a refusal the agent has instructions for.
pub fn may_occupy(
    path: &std::path::Path,
    registered: &std::collections::BTreeMap<String, Option<String>>,
    marker: Option<&serde_json::Value>,
    run_id: &str,
    branch: &str,
) -> Result<bool, Failure> {
    // Existence alone, as the transport has it. This also asked the registry,
    // and the two questions come apart exactly where they are most often
    // apart: a directory removed by hand stays registered until somebody runs
    // `git worktree prune`. The transport creates a fresh checkout there. This
    // fell through to the ownership test instead, found no marker in a
    // directory that is not there, and refused — permanently, and with a
    // recovery nobody had written down.
    if !path.exists() {
        return Ok(false);
    }

    // The registry answers the first half of "is this mine": whose branch is
    // checked out here. Nothing asked it before, so a checkout registered to
    // some other branch was resumed into as long as the marker matched, and
    // the answer was recorded in the refusal payload without being used.
    let occupied_by = registered.get(&normalise_path(path)).and_then(Clone::clone);
    if occupied_by.as_deref() != Some(branch) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "worktree-path-occupied",
            "path": path.display().to_string(),
            "occupied_by_branch": occupied_by,
            "action": "this directory is not a registered worktree for your branch — it is \
                       another checkout or an orphan from a dead run. Do NOT write into it; \
                       verify the holder, or remove the orphan first",
        })));
    }

    // And a marker written by the run that made the checkout answers the
    // second. Absent is a stop of its own, not a lesser form of the next one:
    // it is the only one of the three with a recovery, and the recovery is
    // written down rather than guessed.
    let Some(owner) = marker else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "worktree-ownership-unproven",
            "path": path.display().to_string(),
            "branch": branch,
            "action": "this checkout carries your branch but no run has ever recorded owning \
                       it, so it cannot be told apart from a directory another run is using. \
                       Follow *Recovering a worktree or a branch lock* in bindings/github.md: \
                       preserve any work, prove no process is using it, remove it with \
                       `git worktree remove`, then re-run this command",
        })));
    };

    if owner.get("run_id").and_then(serde_json::Value::as_str) != Some(run_id) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "worktree-owned-by-another-run",
            "path": path.display().to_string(),
            "owner": owner,
            "action": "another run owns this checkout. Do NOT write into it and do NOT delete \
                       it — it may hold unpushed work. If your template is not run-scoped, \
                       make it so; if it is, then the two paths resolve to one real directory \
                       and a symlink or junction above them is aliasing your run onto theirs",
        })));
    }
    Ok(true)
}

#[cfg(test)]
mod tests;

/// The file a worktree's private admin directory holds, under the transport's
/// own name.
const OWNERSHIP_FILE: &str = "issue-flow-owner.json";

/// Where a worktree records which run owns it.
///
/// Inside the worktree's **private git admin directory**, not the working tree.
/// The binding says why and the reasons are not stylistic: a marker in the tree
/// is committed by accident, shows as an untracked file in every `git status`
/// — so people delete it — and **outlives `git worktree remove`** as a stale
/// claim on a directory nobody owns any more.
///
/// This side wrote `.estigia/worktree.json` in the working tree instead, and the
/// cost was not the file's habits: neither side could see the other's, so a
/// checkout one of them had reserved read as *unmarked* to the other — and
/// unmarked is the state whose refusal tells an agent no run owns this
/// directory and to remove it. Two runs, one machine, and the wrong one is told
/// to delete the tree with the work in it.
fn ownership_path(worktree: &std::path::Path) -> Result<std::path::PathBuf, Failure> {
    let answer = run(
        &["git", "rev-parse", "--path-format=absolute", "--git-dir"],
        Some(worktree),
        How::read(),
    )?;
    let text = answer.stdout.trim();
    if text.is_empty() {
        return Err(Failure::Read(format!(
            "git did not report an admin directory for {}",
            worktree.display()
        )));
    }
    Ok(std::path::PathBuf::from(text).join(OWNERSHIP_FILE))
}

/// The run that owns this checkout, nothing, or a read that failed.
///
/// **Three answers, and the third is the point.** `None` is a fact: no run has
/// ever marked this directory. A marker that is there and cannot be read means
/// the fact is *unknown*, and the binding says in its own words why that may
/// not be spelled like the first — *unknown may not be spelled like a
/// permissive fact*.
///
/// This was `.ok().and_then(|text| from_str(&text).ok())`, which spells all
/// three the same. What the caller then said was `worktree-ownership-unproven`:
/// *no run has ever recorded owning it*, with instructions to remove the
/// checkout. A corrupted marker therefore produced a false statement of fact
/// and the destructive recovery for it, in a directory that may hold another
/// run's unpushed work.
fn read_ownership(path: &std::path::Path) -> Result<Option<serde_json::Value>, Failure> {
    match path.try_exists() {
        Ok(false) => return Ok(None),
        Ok(true) => {}
        Err(error) => {
            return Err(Failure::Read(format!(
                "whether the worktree at {} exists could not be read: {error}",
                path.display()
            )));
        }
    }
    let file = ownership_path(path)?;
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Failure::Read(format!(
                "the ownership marker at {} could not be read: {error}",
                file.display()
            )));
        }
    };
    let record: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        Failure::Read(format!(
            "the ownership marker at {} could not be read: {error}",
            file.display()
        ))
    })?;
    // A record naming no run names no owner, which the binding refuses for the
    // same reason: the comparison below would answer "not yours" about a
    // checkout whose holder was never established.
    if !record
        .get("run_id")
        .is_some_and(serde_json::Value::is_string)
    {
        return Err(Failure::Read(format!(
            "the ownership marker at {} names no run",
            file.display()
        )));
    }
    Ok(Some(record))
}

/// A branch reserved for as long as this value lives.
///
/// The acquirer removes it, and a run that **adopted** its own leftover lock is
/// an acquirer — which is why adoption returns one of these rather than merely
/// proceeding. A run that stepped past its own lock without adopting would
/// leave nobody entitled to remove it, and a different run would be blocked by
/// it forever: a leak dressed as caution.
#[derive(Debug)]
pub struct Reservation {
    lock: std::path::PathBuf,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        // Best effort, as the binding has it. A lock that cannot be removed is
        // reported by the next run that meets it, which is the run that can do
        // something about it.
        let _ = std::fs::remove_file(&self.lock);
    }
}

/// Where the lock for one branch lives, in the clone's **shared** admin
/// directory.
///
/// `--git-dir` inside a linked worktree answers that worktree's private
/// directory, which is exactly the wrong scope for a lock meant to serialize
/// different worktrees of one clone.
///
/// Named by digest rather than by the branch: a branch name may contain `/`,
/// and two may differ only in case — one turns the filename into a nested path
/// and the other into a collision. The branch is recorded inside the file, so
/// nothing is lost by not spelling it in the name.
pub fn branch_lock_path(context: &Context, branch: &str) -> Result<std::path::PathBuf, Failure> {
    let answered = run(
        &[
            "git",
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ],
        Some(&context.repo_dir),
        How::read(),
    )?;
    let common = answered.stdout.trim();
    if common.is_empty() {
        return Err(Failure::Read(
            "git did not report a common git directory".to_owned(),
        ));
    }
    Ok(lock_under(common, branch))
}

/// The same, fed the admin directory — which is what makes it measurable.
///
/// Separated for one reason beyond testing: the separator. Git answers
/// `--path-format=absolute` with forward slashes on Windows, the binding builds
/// on that with `pathlib` — which normalises every separator — and `Path::join`
/// does not. The same file therefore came back `C:\repo\.git\issue-flow\…`
/// there and `C:/repo/.git\issue-flow\…` here: one file, two spellings, and the
/// second is the one an operator is handed in a refusal and then types.
fn lock_under(common: &str, branch: &str) -> std::path::PathBuf {
    let common = if cfg!(windows) {
        common.replace('/', std::path::MAIN_SEPARATOR_STR)
    } else {
        common.to_owned()
    };
    let digest = crate::transport::ownership::sha256_hex(branch.as_bytes());
    std::path::Path::new(&common)
        .join("issue-flow")
        .join("branch-locks")
        .join(format!("{}.json", &digest[..32]))
}

/// Reserves one branch within one clone, or says who already has it.
///
/// `create_new` is the whole mechanism: one atomic filesystem operation on both
/// platforms, so exactly one of two concurrent runs creates the file and the
/// other is told it exists. There is no window to lose because there is no
/// separate read — which is precisely what git's own "already used by worktree
/// at" check does not have, and why it loses the race it exists to win. The
/// binding's note records it losing that race on Git 2.55.0.windows.3: two
/// successful concurrent worktrees for one branch.
///
/// **A held lock is never broken on elapsed time.** A timeout is a guess about
/// a process nobody looked at, and the case it guesses wrong is the expensive
/// one: a slow-but-live run has its checkout taken while it is writing into it.
/// The one exception needs no proof about anybody else — a lock naming *this*
/// run is this run's own retry.
pub fn reserve_branch(
    lock: &std::path::Path,
    branch: &str,
    run_id: &str,
    issue: u64,
    now: &str,
) -> Result<Reservation, Failure> {
    if let Some(parent) = lock.parent() {
        std::fs::create_dir_all(parent).map_err(|why| {
            Failure::Write(format!("could not create {}: {why}", parent.display()))
        })?;
    }
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(lock)
    {
        Ok(mut handle) => {
            use std::io::Write as _;
            let record = serde_json::json!({
                "run_id": run_id,
                "issue": issue,
                "branch": branch,
                // Machine identity and PID are deliberately absent: neither is
                // authenticated here, so the recovery contract relies on the
                // named run's durable tracker activity instead.
                "acquired_at": now,
            });
            let body = serde_json::to_string_pretty(&record).unwrap_or_default();
            handle.write_all(body.as_bytes()).map_err(|why| {
                Failure::Write(format!(
                    "could not write the branch lock {}: {why}",
                    lock.display()
                ))
            })?;
            Ok(Reservation {
                lock: lock.to_path_buf(),
            })
        }
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_lock_record(lock)?;
            if existing.get("run_id").and_then(serde_json::Value::as_str) == Some(run_id) {
                return Ok(Reservation {
                    lock: lock.to_path_buf(),
                });
            }
            Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "branch-locked-by-another-run",
                "branch": branch,
                "lock": lock.display().to_string(),
                "held_by": existing,
                "action": "another start-branch holds this branch. Wait for it, or follow \
                           *Recovering a worktree or a branch lock* in bindings/github.md \u{2014} \
                           which requires PROVING the holder's process is stopped before the lock \
                           is removed. Never delete it on an assumption about elapsed time",
            })))
        }
        Err(why) => Err(Failure::Write(format!(
            "could not create the branch lock {}: {why}",
            lock.display()
        ))),
    }
}

/// A held lock, read. An unreadable lock is a failed read, not an absent holder.
///
/// Deliberately **not** the same three answers as [`read_ownership`], and the
/// binding draws the line in the same place: a lock whose text is not JSON
/// still proves somebody created the file, so it comes back as a holder nobody
/// can name rather than as a read failure. What must never happen is the file
/// existing and this answering *nobody holds it*.
fn read_lock_record(lock: &std::path::Path) -> Result<serde_json::Value, Failure> {
    let text = std::fs::read_to_string(lock).map_err(|why| {
        Failure::Read(format!(
            "a branch lock exists at {} but could not be read: {why}",
            lock.display()
        ))
    })?;
    let unnameable = |raw: Option<&str>| {
        let mut record = serde_json::json!({"run_id": null, "unreadable": true});
        if let Some(raw) = raw {
            // Characters, as the binding slices it. Bytes would cut a
            // multi-byte character in half, and the two would disagree about a
            // lock written in anything but ASCII.
            record["raw"] = serde_json::Value::String(raw.chars().take(400).collect());
        }
        record
    };
    let Ok(record) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(unnameable(Some(&text)));
    };
    Ok(if record.is_object() {
        record
    } else {
        unnameable(None)
    })
}

/// Everything one `start-branch` needs.
#[derive(Debug, Clone)]
pub struct Start<'a> {
    /// The issue this checkout is for.
    pub issue: u64,
    /// The branch to reserve.
    pub branch: &'a str,
    /// The base it starts from.
    pub base: &'a str,
    /// The run making the reservation.
    pub run_id: &'a str,
    /// A worktree template overriding the configured one.
    pub worktree_root: Option<&'a str>,
    /// The repository name, for `<repo>`.
    pub repo_name: &'a str,
    /// The state this run believes the issue is in.
    pub expect_state: &'a str,
    /// The moment the timeline is judged against.
    ///
    /// **Read from the machine, never from the run being judged.** The binding
    /// takes it from `utc_now_stamp()` for a reason that is not tidiness: this
    /// value decides whether a claim is still live, so a run that supplies it
    /// decides whether its own claim has expired. The port accepts it as an
    /// argument because a test has to be able to stand at a chosen moment; that
    /// is the only caller allowed to choose one.
    pub now: &'a str,
}

/// `start-branch` — reserve the checkout locally, then link the branch.
pub fn start_branch(
    context: &Context,
    what: &Start<'_>,
    configured: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    // The claim first, as the binding does it: *a claim binds only what the
    // tracker can see, so renew it before the first thing it cannot.*
    // Reserving a checkout and linking a branch is an expensive,
    // half-irreversible boundary, and the contract's first rule is that a claim
    // is adjudicated at every one of them. Without this the worktree and the
    // branch were the first write **nobody checked** — the phrase the
    // `start_branch` tool's own description uses for what it prevents.
    //
    // It sat after the local refusals for a while, on the argument that
    // refusing a missing worktree template should not cost a round trip. That
    // is a better order and it is not this function's to choose: a port that
    // answers a different refusal than the binding for the same call is a port
    // that has stopped being the same thing. The two disagreed on exactly this
    // input — `no-worktree-location` here, `read-failed` there — and nothing
    // saw it, because `start-branch` was one of the two subcommands the
    // differential oracle never ran. That oracle is deleted; what runs this path
    // now is the unit test beside it, not a crossing.
    crate::transport::claim::verify_claim(
        context,
        what.issue,
        what.run_id,
        what.expect_state,
        what.now,
        None,
    )?;

    let template = what.worktree_root.or(configured).unwrap_or_default();
    if template.is_empty() || template.eq_ignore_ascii_case("unset") {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "no-worktree-location",
            // Named for the file the transport actually reads. Both sides said
            // `operator.local.md` while both reads had moved to
            // `estigia.local.md`, so the refusal sent an operator to write a
            // row neither reader would pick up.
            "action": "set the `Worktree location` row in estigia.local.md, or pass \
                       --worktree-root",
        })));
    }
    validate_worktree_override(what.worktree_root)?;

    // The path is resolved next. Everything here can refuse, and refusing
    // before the first remote mutation is the whole ordering guarantee.
    let (scoped, migrated) = worktree::scoped_template(template);
    let resolved = worktree::worktree_path(
        &scoped,
        what.repo_name,
        what.branch,
        what.run_id,
        what.issue,
    )?;
    let path = canonical_worktree_path(&resolved)?;

    // Everything from here to the end of this function is serialized for this
    // branch within this clone, exactly where the binding opens the same scope.
    // Held in a binding rather than dropped immediately: a reservation taken and
    // released on the same line reserves nothing, and the compiler would not
    // say so.
    let _reservation = reserve_branch(
        &branch_lock_path(context, what.branch)?,
        what.branch,
        what.run_id,
        what.issue,
        what.now,
    )?;

    let registered = registered_worktrees(context)?;
    let marker = read_ownership(&path)?;
    let resuming = may_occupy(
        &path,
        &registered,
        marker.as_ref(),
        what.run_id,
        what.branch,
    )?;

    // A template that gained a scope, whose legacy path still has a checkout, is
    // not this command's to clean up: it may hold unpushed work. Either scope
    // can be the one that was added — see `worktree::scoped_template`.
    if migrated {
        let legacy = worktree::worktree_path(
            template,
            what.repo_name,
            what.branch,
            what.run_id,
            what.issue,
        )?;
        if let Some(refusal) = legacy_worktree_block(&legacy, &path, &registered) {
            return Err(refusal);
        }
    }

    let preflight = branch_preflight(&context.repo_dir, what.branch, what.base)?;
    let base_sha = preflight.base_sha;
    let base_tree = preflight.base_tree;
    let fresh = preflight.fresh;
    let base_advance = preflight.base_advance;

    if !resuming {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|why| {
                Failure::Write(format!("could not create {}: {why}", parent.display()))
            })?;
        }
        crate::transport::run_os(
            &[
                std::ffi::OsString::from("git"),
                std::ffi::OsString::from("worktree"),
                std::ffi::OsString::from("add"),
                std::ffi::OsString::from("--"),
                path.as_os_str().to_owned(),
                std::ffi::OsString::from(what.branch),
            ],
            Some(&context.repo_dir),
            How::write(),
        )?;
    }
    let ownership = write_ownership(&path, what)?;

    // ---- the first remote mutation, and not one line earlier ---------------
    //
    // Everything above can refuse, and refusing before this is the whole
    // ordering guarantee: a run that loses the race or cannot prove it owns its
    // own directory leaves nothing behind on GitHub.
    //
    // This call is what the port did not have. Without it the branch existed
    // locally and the issue's Development sidebar stayed empty, so the two sides
    // disagreed about every `start-branch` that got this far — the binding's
    // next read found a published head and this one found none.
    let develop = develop_link(context, what.issue, what.branch, what.base)?;

    let linked = develop
        .get("linked")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let published = refresh_remote_branch(&context.repo_dir, what.branch, linked)?;
    let (head, coherent, verdict) =
        checkout_identity(&path, fresh, published.as_deref(), Some(&base_sha))?;
    if !coherent {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": verdict,
            "branch": what.branch,
            "local_head": head,
            "remote_head": published,
            "recorded_base": base_sha,
            "worktree": path.display().to_string(),
            "action": "the local head, the published head and the recorded base do not tell one \
                       story — most likely the base moved while the branch was being created, or \
                       somebody else pushed to it. Nothing was reported as delivered. The branch \
                       DOES now exist remotely, so this is not a clean slate: re-run this command \
                       to resume it as it now stands, or reconcile the heads by hand first",
        })));
    }

    Ok(serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "branch": what.branch,
        "worktree": path.display().to_string(),
        // Under the transport's own two names. The caller reads whether the
        // sidebar carries the link, and *how* it came to — `created` and
        // `already-linked` are the same end state reached from opposite sides of
        // a resume, and a caller that could not tell them apart would report a
        // resume as a fresh start.
        "natively_linked": linked,
        "link_outcome": develop.get("outcome").cloned().unwrap_or_default(),
        "template_migrated": migrated,
        "resumed_existing_worktree": resuming,
        "ownership": ownership,
        "head": head,
        "base_sha": base_sha,
        "base_tree": base_tree,
        // What happened to the caller's own base branch. A hold reason is
        // reported rather than swallowed: the worktree is correct either way,
        // and an operator who sees a stale `main` deserves to know it was a
        // decision and which one.
        "base_advance": base_advance,
        "identity": verdict,
        "reminder": "gitignored files (.env, credentials, local settings) are NOT in a fresh \
                     worktree",
    }))
}

/// Records, inside the checkout, which run and issue it belongs to.
///
/// This is what makes a resume distinguishable from standing in somebody else's
/// directory — see [`may_occupy`].
fn write_ownership(path: &std::path::Path, what: &Start<'_>) -> Result<serde_json::Value, Failure> {
    // Five fields, under the transport's own names. This wrote three, so a
    // marker written here and read there was missing the two that say *which
    // directory* and *since when* — and a record that cannot name its own
    // worktree is a record nobody can check against the path they are standing
    // in.
    let record = serde_json::json!({
        "run_id": what.run_id,
        "issue": what.issue,
        "branch": what.branch,
        "worktree": path.display().to_string(),
        "claimed_at": crate::harness::session::stamp_of(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
        ),
    });
    let file = ownership_path(path)?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|why| {
            Failure::Write(format!("could not create {}: {why}", parent.display()))
        })?;
    }
    let body = match serde_json::to_string_pretty(&record) {
        Ok(text) => format!("{text}\n"),
        Err(why) => {
            return Err(Failure::Write(format!(
                "the ownership record did not serialise: {why}"
            )));
        }
    };
    crate::paths::replace_atomically(&file, &body)
        .map_err(|why| Failure::Write(format!("could not write {}: {why}", file.display())))?;
    Ok(record)
}

/// Resolves one reference in the checkout that owns it.
fn object_at(path: &std::path::Path, reference: &str) -> Result<String, Failure> {
    let oid = run(
        &["git", "rev-parse", "--verify", reference],
        Some(path),
        How::read(),
    )?
    .stdout
    .trim()
    .to_owned();
    if !is_object_id(&oid) {
        return Err(Failure::Read(format!(
            "git did not resolve {reference:?} to an object id: {oid:?}"
        )));
    }
    Ok(oid)
}

/// The branch identity from the isolated checkout, never the caller's checkout.
fn checkout_head(path: &std::path::Path) -> Result<String, Failure> {
    object_at(path, "HEAD")
}

/// Reads and judges branch identity from the checkout being reserved.
fn checkout_identity(
    path: &std::path::Path,
    fresh: bool,
    published: Option<&str>,
    base: Option<&str>,
) -> Result<(String, bool, &'static str), Failure> {
    // The main checkout may be ahead of or behind this worktree. Its HEAD is
    // unrelated local work and must never decide whether this branch coheres.
    let head = checkout_head(path)?;
    let reachable = match published {
        None => None,
        Some(remote) => {
            let answer = run(
                &["git", "merge-base", "--is-ancestor", remote, &head],
                Some(path),
                How::tolerated(),
            )?;
            match answer.status {
                0 => Some(true),
                1 => Some(false),
                status => {
                    return Err(Failure::Read(format!(
                        "git merge-base could not establish ancestry ({status}): {}",
                        answer.stderr.trim()
                    )));
                }
            }
        }
    };
    let (coherent, verdict) =
        worktree::branch_identity_verdict(fresh, Some(&head), published, base, reachable);
    Ok((head, coherent, verdict))
}

/// Refreshes every remote branch before deciding whether this is a fresh start.
fn branch_preflight(
    repo: &std::path::Path,
    branch: &str,
    base: &str,
) -> Result<BranchPreflight, Failure> {
    // Plain `git fetch origin` still obeys a clone's narrow refspec. Probe and
    // fetch both exact refs explicitly so a single-branch clone cannot hide a
    // remote-only issue branch or leave the recorded base stale.
    let base_sha = refresh_remote_branch(repo, base, true)?
        .ok_or_else(|| Failure::Read(format!("origin has no branch named {base:?}")))?;
    let base_tree = object_at(repo, &format!("{base_sha}^{{tree}}"))?;
    // Convenience, deliberately after the base is proved current and before the
    // branch is cut, so a hold here cannot leave the new branch anywhere else.
    let base_advance = advance_local_base(repo, base, &base_sha);
    let local_head = ref_object(repo, &format!("refs/heads/{branch}"))?;
    let remote_head = refresh_remote_branch(repo, branch, false)?;
    if let Some(start) =
        worktree::branch_start_point(local_head.is_some(), remote_head.is_some(), branch, base)
    {
        run(
            &["git", "branch", "--", branch, &start],
            Some(repo),
            How::write(),
        )?;
    }
    Ok(BranchPreflight {
        base_sha,
        base_tree,
        fresh: local_head.is_none() && remote_head.is_none(),
        base_advance,
    })
}

struct BranchPreflight {
    base_sha: String,
    base_tree: String,
    fresh: bool,
    base_advance: &'static str,
}

/// Advances the operator's own `refs/heads/<base>` to the tip just fetched.
///
/// **This can never fail the run, and it never returns `Err`.** Every worktree
/// already starts from `origin/<base>`, which [`branch_preflight`] refreshes a
/// few lines above, so a base that could not be advanced costs nothing but a
/// stale-looking `git log <base>` in the primary checkout. Raising here would
/// trade a real branch for a cosmetic one — so every failure, including an
/// unreadable answer, becomes a reported hold reason instead.
///
/// The decision itself is [`worktree::base_advance`], which is pure and tested
/// against the states this function can only observe one at a time.
fn advance_local_base(repo: &std::path::Path, base: &str, remote_sha: &str) -> &'static str {
    let Ok(local) = ref_object(repo, &format!("refs/heads/{base}")) else {
        return "local-base-unreadable";
    };
    // Three exit codes, three meanings: 0 is an ancestor, 1 is not, and anything
    // else did not answer. Collapsing the third into "not" would report a
    // divergence that was never established.
    let reachable = local.as_deref().and_then(|local| {
        run(
            &["git", "merge-base", "--is-ancestor", local, remote_sha],
            Some(repo),
            How::tolerated(),
        )
        .ok()
        .and_then(|answer| match answer.status {
            0 => Some(true),
            1 => Some(false),
            _ => None,
        })
    });
    let reference = format!("refs/heads/{base}");
    let checked_out = run(
        &["git", "symbolic-ref", "--quiet", "HEAD"],
        Some(repo),
        How::tolerated(),
    )
    .is_ok_and(|head| head.status == 0 && head.stdout.trim() == reference);
    // The tree this run cannot see. `symbolic-ref HEAD` answers for *this*
    // directory only, and a base checked out in a linked worktree is invisible
    // to it — while `git update-ref` moves the ref anyway, leaving that
    // checkout reporting deletions of files nobody removed. Unreadable counts
    // as present, because holding costs a cosmetic ref update and guessing
    // costs somebody else's working tree.
    let checked_out_elsewhere = !checked_out
        && !run(
            &["git", "worktree", "list", "--porcelain"],
            Some(repo),
            How::tolerated(),
        )
        .is_ok_and(|listing| {
            listing.status == 0
                && !listing
                    .stdout
                    .lines()
                    .any(|line| line.trim() == format!("branch {reference}"))
        });
    // An unreadable status counts as dirty. The conservative direction is the
    // one that holds, and this is a convenience with nothing to gain by guessing.
    let dirty = !run(
        &["git", "status", "--porcelain"],
        Some(repo),
        How::tolerated(),
    )
    .is_ok_and(|status| status.status == 0 && status.stdout.trim().is_empty());

    let moved = match worktree::base_advance(
        local.as_deref(),
        remote_sha,
        reachable,
        checked_out,
        dirty,
        checked_out_elsewhere,
    ) {
        worktree::BaseAdvance::Current => return "current",
        worktree::BaseAdvance::Hold(reason) => return reason,
        // No working tree points at this ref, so moving it changes no file.
        worktree::BaseAdvance::UpdateRef => run(
            &[
                "git",
                "update-ref",
                &format!("refs/heads/{base}"),
                remote_sha,
            ],
            Some(repo),
            How::tolerated(),
        ),
        // Checked out and clean: git has to carry index and files with the ref,
        // and `--ff-only` is what refuses rather than inventing a merge commit.
        worktree::BaseAdvance::FastForwardCheckout => run(
            &["git", "merge", "--ff-only", &format!("origin/{base}")],
            Some(repo),
            How::tolerated(),
        ),
    };
    if !moved.is_ok_and(|outcome| outcome.status == 0) {
        return "advance-failed";
    }
    // Read the ref back rather than trusting the exit code. Reporting "advanced"
    // off a status nobody re-read is the failure this repository refuses by name.
    match ref_object(repo, &format!("refs/heads/{base}")) {
        Ok(Some(now)) if now == remote_sha => "advanced",
        _ => "advance-unconfirmed",
    }
}

fn validate_worktree_override(override_root: Option<&str>) -> Result<(), Failure> {
    if override_root.is_some_and(|root| !std::path::Path::new(root).is_absolute()) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "worktree-location-not-absolute",
            "action": "pass an absolute directory in --worktree-root",
        })));
    }
    Ok(())
}

/// Reads one exact remote branch and refreshes its tracking ref regardless of refspec.
fn refresh_remote_branch(
    repo: &std::path::Path,
    branch: &str,
    required: bool,
) -> Result<Option<String>, Failure> {
    let reference = format!("refs/heads/{branch}");
    let answer = run(
        &["git", "ls-remote", "--heads", "origin", &reference],
        Some(repo),
        How::tolerated(),
    )?;
    if answer.status != 0 {
        return Err(Failure::Read(format!(
            "git ls-remote {reference} failed ({}): {}",
            answer.status,
            answer.stderr.trim()
        )));
    }
    let matched = exact_remote_refs(&answer.stdout, &reference)?;
    let oid = match matched.as_slice() {
        [] if !required => return Ok(None),
        [] => {
            return Err(Failure::Read(format!(
                "origin has no branch named {branch:?}"
            )));
        }
        [only] if is_object_id(only) => only.clone(),
        _ => {
            return Err(Failure::Read(format!(
                "git ls-remote {reference} answered ambiguously: {matched:?}"
            )));
        }
    };
    let tracking = format!("refs/remotes/origin/{branch}");
    let refspec = format!("+{reference}:{tracking}");
    run(
        &["git", "fetch", "origin", "--", &refspec],
        Some(repo),
        How::read(),
    )?;
    let refreshed = ref_object(repo, &tracking)?;
    if refreshed.as_deref() != Some(&oid) {
        return Err(Failure::Read(format!(
            "origin reported {reference} at {oid}, but {tracking} refreshed to {refreshed:?}"
        )));
    }
    Ok(refreshed)
}

/// The object ids `ls-remote` reported for exactly one remote refname.
fn exact_remote_refs(output: &str, reference: &str) -> Result<Vec<String>, Failure> {
    let mut matched = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((oid, name)) = line.split_once('\t') else {
            return Err(Failure::Read(format!(
                "git ls-remote {reference} answered in an unexpected format: {line:?}"
            )));
        };
        if name.trim() == reference {
            matched.push(oid.trim().to_owned());
        }
    }
    Ok(matched)
}

/// Every branch the issue's Development sidebar names.
const LINKED_BRANCHES_QUERY: &str = r#"
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      linkedBranches(first: 100) {
        nodes { ref { name target { oid } } }
        pageInfo { hasNextPage }
      }
    }
  }
}
"#;

/// How long `gh issue develop` may take before its outcome is called unknown.
const DEVELOP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Is this branch in the issue's Development sidebar? Answers, or fails the read.
///
/// The read-back that turns an ambiguous `gh issue develop` into a fact.
///
/// It takes the branch rather than returning the set, and the asymmetry is the
/// point. Only **one** of the two answers needs the page to be complete. Finding
/// the branch present is conclusive whatever else the connection holds — a later
/// page cannot un-link it. Concluding it **absent** rests on having seen
/// everything, so a connection still advertising another page has not answered
/// the question; it has answered *here are some links*, which must never be read
/// as an absence. Refusing both ways would turn a definite yes into an
/// unretryable failed read, which is a fail-closed answer to a question that was
/// already answered.
pub fn is_branch_linked(context: &Context, issue: u64, branch: &str) -> Result<bool, Failure> {
    let (owner, name) = super::closing::repo_identity(context)?;
    let data = super::gh_json(
        &[
            "api",
            "graphql",
            "-f",
            &format!("query={LINKED_BRANCHES_QUERY}"),
            "-f",
            &format!("owner={owner}"),
            "-f",
            &format!("name={name}"),
            "-F",
            &format!("number={issue}"),
        ],
        Some(&context.repo_dir),
    )?;
    let unreadable =
        || Failure::Read("linked-branch GraphQL response is partial or malformed".to_owned());
    let Some(data) = data.filter(serde_json::Value::is_object) else {
        return Err(unreadable());
    };
    // `errors` beside `data` is a partial answer, and GraphQL sends both.
    if data.get("errors").is_some_and(|value| !value.is_null()) {
        return Err(unreadable());
    }
    let connection = data
        .get("data")
        .and_then(|data| data.get("repository"))
        .and_then(|repository| repository.get("issue"))
        .and_then(|issue| issue.get("linkedBranches"))
        .filter(|value| value.is_object());
    let Some(nodes) = connection
        .and_then(|connection| connection.get("nodes"))
        .and_then(serde_json::Value::as_array)
    else {
        return Err(Failure::Read(
            "linked-branch response omitted the branch connection".to_owned(),
        ));
    };
    // Present and a boolean, or the page said nothing about whether it is the
    // last one — and the absence below rests entirely on that.
    let Some(has_next) = connection
        .and_then(|connection| connection.get("pageInfo"))
        .filter(|value| value.is_object())
        .and_then(|page| page.get("hasNextPage"))
        .and_then(serde_json::Value::as_bool)
    else {
        return Err(Failure::Read(
            "linked-branch response carried no boolean hasNextPage".to_owned(),
        ));
    };
    let mut names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for node in nodes {
        let Some(found) = node
            .get("ref")
            .filter(|value| value.is_object())
            .and_then(|reference| reference.get("name"))
            .and_then(serde_json::Value::as_str)
        else {
            return Err(Failure::Read(
                "linked-branch response contains a malformed ref node".to_owned(),
            ));
        };
        names.insert(found);
    }
    if names.contains(branch) {
        return Ok(true);
    }
    if has_next {
        return Err(Failure::Read(
            "linked-branch connection advertises another page, and the branch was not on it; \
             the absence is unproven"
                .to_owned(),
        ));
    }
    Ok(false)
}

/// Link the branch natively, and never report an unread outcome as a known one.
///
/// `gh issue develop` is one command that replaces branch creation **and**
/// recording: it branches from the fresh base and links it in the issue's
/// Development sidebar. A branch nobody can find from the issue is work nobody
/// can follow — and until this existed the port stopped after the local
/// reservation and left that sidebar empty while the transport filled it.
///
/// What a **nonzero exit** means is the whole of it. It is not "not linked": the
/// command mutates two remotes at once, a ref and a link, and its failures are
/// not one thing — *a branch of that name already exists* (the ordinary resume,
/// where the link may well be present), a partial failure that created the ref
/// and not the link, and a connection that dropped after the server had already
/// committed both. Those three are indistinguishable from the exit code, and in
/// the last two "not linked" is false.
///
/// So a nonzero exit and a timeout are treated identically: **re-read the
/// sidebar**, and let the read decide. A read that fails is a failed read and
/// not a verdict — except after a timeout, where it is an ambiguous *write*,
/// because the ref may exist on the server right now.
pub fn develop_link(
    context: &Context,
    issue: u64,
    branch: &str,
    base: &str,
) -> Result<serde_json::Value, Failure> {
    let number = issue.to_string();
    let attempt = run(
        &[
            "gh", "issue", "develop", &number, "--name", branch, "--base", base,
        ],
        Some(&context.repo_dir),
        How {
            timeout: Some(DEVELOP_TIMEOUT),
            writes: true,
            ..How::tolerated()
        },
    );
    let (timed_out, detail) = match attempt {
        Ok(answer) if answer.status == 0 => {
            return Ok(serde_json::json!({"linked": true, "outcome": "created"}));
        }
        Ok(answer) => (
            false,
            answer.stderr.trim().chars().take(400).collect::<String>(),
        ),
        Err(Failure::Timeout(why)) => (true, why),
        Err(other) => return Err(other),
    };

    let linked = match is_branch_linked(context, issue, branch) {
        Ok(linked) => linked,
        // The wait expired **and** the state is unreadable. The one thing that
        // must not happen is reporting this as "nothing was created": the ref
        // may exist on the server right now.
        Err(Failure::Read(_)) if timed_out => {
            return Err(Failure::Write(format!(
                "gh issue develop for {branch} timed out and its outcome could not be re-read: \
                 {detail}"
            )));
        }
        Err(other) => return Err(other),
    };
    if linked {
        return Ok(serde_json::json!({
            "linked": true, "outcome": "already-linked", "detail": detail
        }));
    }
    if timed_out {
        return Err(Failure::Write(format!(
            "gh issue develop for {branch} timed out; the sidebar shows no link, but a ref may \
             still have been created — re-read `refs/remotes/origin/{branch}` before retrying"
        )));
    }
    Ok(serde_json::json!({"linked": false, "outcome": "not-linked", "detail": detail}))
}

/// The stop a template that gained a scope earns when its pre-migration checkout
/// is still there — and only when it is still **there**.
///
/// Either scope can be the one that was added: a template naming the branch and
/// not the run, or the run and not the branch. Both leave a directory the
/// previous build created, and neither is this command's to remove.
///
/// Registered *and* on disk, never registered alone. The transport asks
/// `legacy.exists()` before it looks the path up, and this asked only the
/// registry, so the two answered differently for the most ordinary state a
/// worktree reaches: a directory somebody deleted with `rm -rf` instead of
/// `git worktree remove`. Git goes on listing it — measured, it comes back
/// `prunable gitdir file points to non-existent location` — and neither
/// registry reader filters that out, so this side stopped a `start-branch` that
/// the transport performs.
///
/// The existence check is not a detail of the stop, it **is** the stop. What
/// this refusal protects is unpushed work in a directory, and its own way out
/// says so — *push or preserve that work*. A registration whose directory is
/// gone holds no work to preserve, and stopping for it sends an operator to
/// rescue a tree that is not there.
///
/// Named rather than left inline so it can be measured without a tracker, a
/// remote and a clock standing by.
fn legacy_worktree_block(
    legacy: &std::path::Path,
    run_scoped: &std::path::Path,
    registered: &std::collections::BTreeMap<String, Option<String>>,
) -> Option<Failure> {
    if !legacy.exists() {
        return None;
    }
    let owner = registered.get(&normalise_path(legacy))?;
    Some(Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "legacy-worktree-registered",
        "legacy_path": legacy.display().to_string(),
        "scoped_path": run_scoped.display().to_string(),
        "occupied_by_branch": owner,
        "action": "your worktree template does not name every scope a checkout needs \
                   (the branch, the run, or both), and a checkout from before the \
                   missing one was added is still registered at the legacy path. It may \
                   hold unpushed work, so nothing here removes it: push or preserve that \
                   work, `git worktree remove` it, then re-run this command",
    })))
}
