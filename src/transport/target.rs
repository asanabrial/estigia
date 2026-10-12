//! `expected-target` — the complete intended review target, without disturbing it.
//!
//! **Strictly read-only**: `ls-tree`, `status` and `hash-object` *without* `-w`,
//! so nothing reaches the object store, no ref moves, and the index and worktree
//! are untouched. That matters because this runs before review, on a tree
//! somebody is still working in.
//!
//! That promise is `expected-target`'s and not the whole module's:
//! `review_payload_bounded` builds its diff in an isolated `GIT_DIR`, which
//! creates a directory and four files under the repository's own admin
//! directory and removes them on drop. Still no object, no ref, no index and no
//! worktree — but not literally write-free, and an unwritable `$GIT_DIR` fails
//! the payload build rather than diffing.

use super::{
    Context, Failure, How, commands, isolated_diff_env, manifest, pinned_diff_args, run,
    run_with_isolated_env,
};

/// Emits the target: the exact base through `HEAD`, plus the uncommitted
/// worktree, as a manifest with one digest.
///
/// `supplied` is a caller's own idea of the review target, and disagreeing with
/// it is a stop: an approval that covers less than the branch delivers looks
/// exactly like one that covers all of it. The comparison runs against the
/// internal manifest built below, never against the rendered answer, so it
/// fires the same way whether or not `brief` asked for that manifest back.
///
/// `brief` omits the `manifest` array from the answer, keeping `ok`, `base`,
/// `head`, `digest`, `paths` and `uncommitted`. The digest is computed over
/// the manifest before either shape is rendered, so it does not move between
/// them — see `CHANGELOG.md` for why the array is worth skipping.
pub fn expected_target(
    context: &Context,
    base: &str,
    worktree: Option<&std::path::Path>,
    supplied: Option<&std::path::Path>,
    brief: bool,
) -> Result<serde_json::Value, Failure> {
    let at = worktree.unwrap_or(&context.repo_dir);
    let base_oid = object(at, &format!("{base}^{{commit}}"))?;
    let head = object(at, "HEAD^{commit}")?;

    // Every path below is repository-root relative, because that is what
    // `ls-tree --full-tree` and `status --porcelain` both emit — regardless of
    // which directory the command ran from. Joining them onto the worktree
    // silently mislocates every file when that is a **subdirectory**: the path
    // then reads as absent and the target quietly shrinks while reporting
    // success.
    let top = run(
        &["git", "rev-parse", "--show-toplevel"],
        Some(at),
        How::read(),
    )?
    .stdout
    .trim()
    .to_owned();

    let listing = run(
        &["git", "ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        Some(at),
        How::read(),
    )?
    .stdout;
    // A listing that stops mid-entry is a short read, not a short tree.
    if !listing.is_empty() && !listing.ends_with('\0') {
        return Err(Failure::Read("git ls-tree ended mid-entry".to_owned()));
    }
    let entries: Vec<&str> = listing
        .split('\0')
        .filter(|line| !line.is_empty())
        .collect();
    let committed = manifest::tree_manifest(&entries).map_err(commands::manifest_trouble)?;

    // `--no-optional-locks`, because a plain `git status` refreshes and
    // **rewrites** `.git/index`, taking `index.lock` to do it. This command's
    // whole promise is that it can run against a tree somebody is working in,
    // and taking their index lock breaks that for a cache update nobody asked
    // for.
    let reported = run(
        &[
            "git",
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ],
        Some(at),
        How::read(),
    )?
    .stdout;
    let fields: Vec<&str> = reported
        .split('\0')
        .filter(|field| !field.is_empty())
        .collect();
    let status = commands::read_status(&fields)?;

    let target = manifest::overlay_worktree(&committed, &status, |path| {
        blob_of(at, &top, &committed, path)
    })
    .map_err(commands::manifest_trouble)?;

    let uncommitted: std::collections::BTreeSet<&String> =
        status.iter().map(|(_, path)| path).collect();
    let digest = manifest::manifest_digest(&target);
    let mut answer = serde_json::json!({
        "ok": true,
        "base": base_oid,
        "head": head,
        "digest": digest,
        "paths": target.len(),
        "uncommitted": uncommitted,
    });
    if !brief {
        // The manifest itself, and not only the digest over it: a digest says
        // two targets differ, the manifest says *which paths*. In `Manifest`
        // order, which is sorted, matching what the transport emits.
        answer["manifest"] = serde_json::json!(
            target
                .iter()
                .map(|(path, (mode, blob))| serde_json::json!({
                    "path": path,
                    "mode": mode,
                    "blob": blob,
                }))
                .collect::<Vec<_>>()
        );
    }

    if let Some(path) = supplied {
        let claimed = supplied_manifest(path)?;
        let Difference {
            unreviewed,
            not_delivered,
            differing,
        } = difference(&target, &claimed);
        if !unreviewed.is_empty() || !not_delivered.is_empty() || !differing.is_empty() {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "review-target-mismatch",
                "base": base_oid,
                "head": head,
                "expected_digest": digest,
                "supplied_digest": manifest::manifest_digest(&claimed),
                "unreviewed": unreviewed,
                "not_delivered": not_delivered,
                "differing": differing,
                "action": "the target the reviewer was given is not the target this branch \
                           delivers. `unreviewed` paths would ship without authority. Rebuild \
                           the review over the complete target; do not proceed on the \
                           difference being small",
            })));
        }
        answer["native_start"] = serde_json::Value::String("matches".to_owned());
    }
    Ok(answer)
}

/// Derives a target that can be bound to a published commit.
///
/// `expected_target` deliberately includes uncommitted bytes so a reviewer can
/// diagnose an incomplete review. Publication has a narrower contract: a PR
/// head can name only committed bytes, so a dirty checkout is refused rather
/// than given a digest the remote head cannot reproduce. Equal clean status and
/// HEAD observations before and after derivation prevent a manifest assembled
/// across two checkout states from becoming evidence.
pub fn clean_target(
    context: &Context,
    base: &str,
    worktree: Option<&std::path::Path>,
) -> Result<serde_json::Value, Failure> {
    let at = worktree.unwrap_or(&context.repo_dir);
    let before_head = object(at, "HEAD^{commit}")?;
    let before_status = clean_status(at)?;
    let target = expected_target(context, base, worktree, None, false)?;
    let after_status = clean_status(at)?;
    let after_head = object(at, "HEAD^{commit}")?;
    coherent_clean_snapshot(
        &before_head,
        &before_status,
        &target,
        &after_head,
        &after_status,
    )?;
    Ok(target)
}

/// Byte ceiling on the review-target payload. Overflow is a refusal, never a truncation.
pub const REVIEW_PAYLOAD_CEILING: usize = 256 * 1024;

/// Per-path unified hunks of one publication, derived from the receipt's base and head.
///
/// A path the base declares in `estigia.generated.toml` and the change adds or
/// modifies as a regular file is a `diff --git` header and one manifest line
/// after the other paths' hunks; `docs/generated-files.md` has the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPayload {
    /// The unified diff `publish_review` hands every judge of this epoch.
    pub text: String,
    /// SHA-256 of [`Self::text`], bound on the published marker as `payload-digest`.
    pub digest: String,
}

/// Materialises the review-target payload against the named ends.
///
/// Overflow of [`REVIEW_PAYLOAD_CEILING`] is a refusal, never a truncation.
pub fn review_payload(
    at: &std::path::Path,
    base: &str,
    head: &str,
) -> Result<ReviewPayload, Failure> {
    review_payload_bounded(at, base, head, REVIEW_PAYLOAD_CEILING)
}

pub(crate) fn review_payload_bounded(
    at: &std::path::Path,
    base: &str,
    head: &str,
    ceiling: usize,
) -> Result<ReviewPayload, Failure> {
    payload_declared_at(at, base, head, base, ceiling)
}

pub(crate) fn review_payload_declared_at(
    at: &std::path::Path,
    base: &str,
    head: &str,
    authority: &str,
) -> Result<ReviewPayload, Failure> {
    payload_declared_at(at, base, head, authority, REVIEW_PAYLOAD_CEILING)
}

fn payload_declared_at(
    at: &std::path::Path,
    base: &str,
    head: &str,
    authority: &str,
    ceiling: usize,
) -> Result<ReviewPayload, Failure> {
    if !is_object_id(base) || !is_object_id(head) {
        return Err(Failure::Read(
            "review payload ends are not object ids".to_owned(),
        ));
    }
    // Isolated unconditionally, not only when a review found an attribute
    // source that collapsed it: a committed `.gitattributes`, an untracked
    // `.git/info/attributes`, `core.attributesFile` and the caller's own
    // default global attributes file all reach a plain `git diff` and can
    // mark every path binary, which silences the hunks this text is for and
    // shrinks it under `REVIEW_PAYLOAD_CEILING` along the way. `docs/honesty.md`
    // has the measurement.
    let (_guard, envs) = isolated_diff_env(at)?;
    let envs: Vec<(&str, &str)> = envs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let entries = super::generated::manifest_entries(at, authority, base, head, &envs)?;
    let excluded: Vec<String> = entries
        .iter()
        .map(|entry| format!(":(top,exclude,literal){}", entry.path))
        .collect();
    let mut rest = vec!["--no-renames", "--unified=3", base, head];
    if !excluded.is_empty() {
        rest.push("--");
        rest.extend(excluded.iter().map(String::as_str));
    }
    let output =
        run_with_isolated_env(&pinned_diff_args(&rest), Some(at), How::tolerated(), &envs)?;
    if output.status != 0 && output.status != 1 {
        return Err(Failure::Read(format!(
            "git diff failed ({status}): {detail}",
            status = output.status,
            detail = output.stderr.trim()
        )));
    }
    if output.stdout_replaced {
        return Err(Failure::Read(
            "git diff produced bytes that are not UTF-8".to_owned(),
        ));
    }
    let mut text = output.stdout;
    for entry in &entries {
        text.push_str(&entry.block());
    }
    let size = text.len();
    if size > ceiling {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "review-payload-too-large",
            "bytes": size,
            "ceiling": ceiling,
            "action": "the change is too large to review as one unit. Split it; do not truncate \
                       the payload and do not hand a judge a partial view",
        })));
    }
    Ok(ReviewPayload {
        digest: super::ownership::sha256_hex(text.as_bytes()),
        text,
    })
}

/// How many paths a unified diff touches.
///
/// One `diff --git a/<path> b/<path>` header per path — [`review_payload_bounded`]
/// runs `--no-renames`, so a rename never collapses two paths into one header —
/// and every path git can report gets one: an add, a delete, a mode-only change
/// and a binary file all open a header with no hunk beneath it, so counting
/// hunks would undercount them. A content line inside the diff body is prefixed
/// `+`, `-` or a space and so never opens at column zero, which is what makes
/// `starts_with` safe against a file whose own content is itself a diff.
pub(crate) fn changed_file_count(diff: &str) -> usize {
    diff.lines()
        .filter(|line| line.starts_with("diff --git "))
        .count()
}

pub(crate) fn merge_base(at: &std::path::Path, one: &str, two: &str) -> Result<String, Failure> {
    let oid = run(&["git", "merge-base", one, two], Some(at), How::read())?
        .stdout
        .trim()
        .to_owned();
    if !is_object_id(&oid) {
        return Err(Failure::Read(format!(
            "git did not resolve a merge base of {one:?} and {two:?}: {oid:?}"
        )));
    }
    Ok(oid)
}

pub(crate) fn is_ancestor(
    at: &std::path::Path,
    ancestor: &str,
    descendant: &str,
) -> Result<bool, Failure> {
    let output = run(
        &["git", "merge-base", "--is-ancestor", ancestor, descendant],
        Some(at),
        How::tolerated(),
    )?;
    match output.status {
        0 => Ok(true),
        1 => Ok(false),
        status => Err(Failure::Read(format!(
            "git merge-base --is-ancestor failed ({status}): {detail}",
            detail = output.stderr.trim()
        ))),
    }
}

fn clean_status(at: &std::path::Path) -> Result<String, Failure> {
    run(
        &[
            "git",
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ],
        Some(at),
        How::read(),
    )
    .map(|output| output.stdout)
}

/// Proves the target was derived between two identical clean observations.
pub fn coherent_clean_snapshot(
    before_head: &str,
    before_status: &str,
    target: &serde_json::Value,
    after_head: &str,
    after_status: &str,
) -> Result<(), Failure> {
    let target_head = target.get("head").and_then(serde_json::Value::as_str);
    let target_dirty = target
        .get("uncommitted")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|paths| !paths.is_empty());
    if !before_status.is_empty() || !after_status.is_empty() || target_dirty {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "dirty-review-target",
            "uncommitted": target.get("uncommitted"),
            "action": "commit or remove every worktree change before publishing or releasing CI; a PR head cannot name uncommitted bytes",
        })));
    }
    if before_head != after_head || target_head != Some(before_head) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "review-target-moved-during-read",
            "before_head": before_head,
            "target_head": target_head,
            "after_head": after_head,
            "action": "derive the target again after the checkout stops moving; no mixed snapshot was published",
        })));
    }
    Ok(())
}

/// The three ways two review targets can fail to be the same one.
#[derive(Debug, Default)]
pub struct Difference<'a> {
    /// In the delivery and not in what the reviewer was shown. The expensive
    /// one: these ship carrying an approval that never covered them.
    pub unreviewed: Vec<&'a String>,
    /// Shown to the reviewer and not delivered.
    pub not_delivered: Vec<&'a String>,
    /// In both, at different content or a different mode.
    pub differing: Vec<&'a String>,
}

/// How a caller's review target differs from the one just derived.
///
/// Pure and fed, so the decision is testable without a repository: the two
/// manifests come in and the three lists come out. Every list is sorted,
/// because `Manifest` is a `BTreeMap` and a refusal that names paths in a
/// different order on each run is one nobody can diff.
pub fn difference<'a>(
    target: &'a manifest::Manifest,
    claimed: &'a manifest::Manifest,
) -> Difference<'a> {
    Difference {
        unreviewed: target
            .keys()
            .filter(|path| !claimed.contains_key(*path))
            .collect(),
        not_delivered: claimed
            .keys()
            .filter(|path| !target.contains_key(*path))
            .collect(),
        differing: target
            .iter()
            .filter(|(path, ours)| claimed.get(*path).is_some_and(|theirs| theirs != *ours))
            .map(|(path, _)| path)
            .collect(),
    }
}

/// A caller's own review target, read from the file it was handed in.
///
/// Either a bare list of entries or an object with a `manifest` key, because
/// the transport accepts both. Anything else is a **read** failure and not a
/// stop: the file was unreadable, which is not the same as the two targets
/// disagreeing, and saying the second when the first happened is the one thing
/// this command must never do.
fn supplied_manifest(path: &std::path::Path) -> Result<manifest::Manifest, Failure> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        Failure::Read(format!(
            "the supplied native-start target could not be read: {error}"
        ))
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        Failure::Read(format!(
            "the supplied native-start target is not JSON: {error}"
        ))
    })?;
    let entries = match parsed.get("manifest") {
        Some(entries) => entries,
        None => &parsed,
    };
    let Some(entries) = entries.as_array() else {
        return Err(Failure::Read(
            "the supplied native-start target has no manifest list".to_owned(),
        ));
    };
    let mut claimed = manifest::Manifest::new();
    for entry in entries {
        let field = |name: &str| {
            entry
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        };
        let (Some(path), Some(mode), Some(blob)) = (field("path"), field("mode"), field("blob"))
        else {
            return Err(Failure::Read(format!(
                "a supplied manifest entry is malformed: {entry}"
            )));
        };
        claimed.insert(path, (mode, blob));
    }
    Ok(claimed)
}

/// Resolves one reference, refusing anything that is not an object id.
///
/// A `rev-parse` that succeeds and prints a message, a ref name, or an empty
/// line is a read that did not answer — and a review bound to that text is
/// bound to nothing. Seven characters is git's own shortest abbreviation.
///
/// **Its own function so it can be measured.**
pub(super) fn is_object_id(oid: &str) -> bool {
    oid.len() >= 7 && oid.chars().all(|c| c.is_ascii_hexdigit())
}

fn object(at: &std::path::Path, reference: &str) -> Result<String, Failure> {
    let oid = run(
        &["git", "rev-parse", "--verify", reference],
        Some(at),
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

/// Hashes one working-tree path and reports its mode.
///
/// `Ok(None)` is reserved for a path that is really gone (`NotFound`), which
/// the overlay reads as a deletion. Any other failure is a refusal.
fn blob_of(
    at: &std::path::Path,
    top: &str,
    committed: &manifest::Manifest,
    path: &str,
) -> Result<Option<(String, String)>, manifest::Trouble> {
    let unhashable = || manifest::Trouble::Unhashable(path.to_owned());
    let entry = std::path::Path::new(top).join(path);
    let metadata = match std::fs::symlink_metadata(&entry) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unhashable()),
    };

    if metadata.file_type().is_symlink() {
        // Hash the **link text**, which is what git stores behind mode 120000.
        // `hash-object` on the path follows the link and records the blob of
        // whatever it points at — a different object, and one that changes when
        // the target changes rather than the link.
        let link = std::fs::read_link(&entry).map_err(|_| unhashable())?;
        let staged = crate::paths::scratch_file("link-probe");
        let written = crate::paths::replace_atomically(&staged, &link.display().to_string());
        let probe = written.is_ok().then(|| {
            run(
                &["git", "hash-object", "--", &staged.display().to_string()],
                Some(at),
                How::tolerated(),
            )
        });
        let _ = std::fs::remove_file(&staged);
        let probe = probe.ok_or_else(unhashable)?.map_err(|_| unhashable())?;
        if probe.status != 0 {
            return Err(unhashable());
        }
        return Ok(Some(("120000".to_owned(), probe.stdout.trim().to_owned())));
    }

    let probe = run(
        &["git", "hash-object", "--", &entry.display().to_string()],
        Some(at),
        How::tolerated(),
    )
    .map_err(|_| unhashable())?;
    if probe.status != 0 {
        return Err(unhashable());
    }
    Ok(Some((
        mode_of(at, committed, path, &metadata),
        probe.stdout.trim().to_owned(),
    )))
}

/// The mode to record for a working-tree file.
///
/// On Windows (or with `core.fileMode` off) there is no executable bit to
/// read, so the committed tree's own mode is kept rather than inferred from an
/// access check, which reports executable for every file and would make a
/// cross-platform digest disagree.
fn mode_of(
    at: &std::path::Path,
    committed: &manifest::Manifest,
    path: &str,
    metadata: &std::fs::Metadata,
) -> String {
    let honours = !cfg!(windows)
        && run(
            &["git", "config", "--get", "core.fileMode"],
            Some(at),
            How::tolerated(),
        )
        .map(|out| out.stdout.trim().to_lowercase() != "false")
        .unwrap_or(true);

    if honours {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 != 0 {
                return "100755".to_owned();
            }
            return "100644".to_owned();
        }
    }
    let _ = metadata;
    committed
        .get(path)
        .map_or("100644", |(mode, _)| mode.as_str())
        .to_owned()
}

#[cfg(test)]
mod tests;
