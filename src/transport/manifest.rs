//! The complete delivery target, as a manifest with one identity.
//!
//! A reviewer authorises a diff. If what they were shown is a **subset** of what
//! the pull request delivers, their approval is evidence for something that was
//! never reviewed — and nothing downstream can tell, because an approval looks
//! the same either way. Issue #70: the approved lineage covered 12 workspace
//! paths while the PR contained 15, and 35 of 65 final hunks carried no
//! authority at all.
//!
//! So the target is `base..HEAD` **plus** the uncommitted worktree, reported as
//! a path/mode/blob manifest with a single digest over it.

use std::collections::BTreeMap;

/// `{path: (mode, blob)}`.
pub type Manifest = BTreeMap<String, (String, String)>;

/// Why a tree listing could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// An entry that is not `mode blob oid<TAB>path`.
    Unreadable(String),
    /// The same path twice.
    Duplicate(String),
    /// No entries at all — an empty answer is not a delivery target.
    Empty,
}

/// `{path: (mode, blob)}` from `git ls-tree -r -z` output.
///
/// The **mode** is carried, not just the content: a file that becomes
/// executable, or a regular file replaced by a symlink, changes what is
/// delivered while every byte stays identical. A manifest of blobs alone would
/// call that no change at all.
pub fn tree_manifest(entries: &[&str]) -> Result<Manifest, Trouble> {
    let mut manifest = Manifest::new();
    for line in entries {
        if line.is_empty() {
            continue;
        }
        let Some((meta, path)) = line.split_once('\t') else {
            return Err(Trouble::Unreadable((*line).to_owned()));
        };
        let fields: Vec<&str> = meta.split_whitespace().collect();
        if path.is_empty() || fields.len() != 3 || fields[1] != "blob" {
            return Err(Trouble::Unreadable((*line).to_owned()));
        }
        if manifest
            .insert(
                path.to_owned(),
                (fields[0].to_owned(), fields[2].to_owned()),
            )
            .is_some()
        {
            return Err(Trouble::Duplicate(path.to_owned()));
        }
    }
    if manifest.is_empty() {
        return Err(Trouble::Empty);
    }
    Ok(manifest)
}

/// One identity for a delivery target, stable across machines and order.
///
/// A digest rather than the manifest itself, because it is what a caller can
/// compare in **one equality** without agreeing on JSON key order or line
/// endings. It covers path, mode and blob, so it changes for a content edit, a
/// mode flip, a rename and a deletion alike — including binary content, which no
/// textual diff summary represents faithfully.
pub fn manifest_digest(manifest: &Manifest) -> String {
    let payload: Vec<String> = manifest
        .iter()
        .map(|(path, (mode, blob))| format!("{mode} {blob} {path}"))
        .collect();
    super::ownership::sha256_hex(payload.join("\n").as_bytes())
}

/// Applies uncommitted worktree changes onto a committed manifest.
///
/// The review target is `base..HEAD` **plus** whatever is still uncommitted,
/// because that is what a reviewer is being asked to authorise. Reviewing only
/// the committed prefix, or only the dirty suffix, both authorise something
/// other than the delivery.
///
/// `status` is `(code, path)` from `git status --porcelain=v1 -z`, and
/// `blob_of` hashes a working-tree path and reports its mode — so this stays
/// pure and the filesystem stays with the caller that owns it.
///
/// A code this does not recognise is a **refusal**, never a skip. Silently
/// ignoring one would drop a changed path from the target and report success for
/// a review of less than what ships.
pub fn overlay_worktree(
    manifest: &Manifest,
    status: &[(String, String)],
    blob_of: impl Fn(&str) -> Option<(String, String)>,
) -> Result<Manifest, Trouble> {
    let mut overlaid = manifest.clone();
    for (code, path) in status {
        if path.is_empty() {
            return Err(Trouble::Unreadable("a change with no path".to_owned()));
        }
        let trimmed = code.trim();
        // A deletion in either column, and the two-letter forms git uses when
        // the index and the worktree disagree about it.
        let deleted = matches!(trimmed, "D" | "DD" | "AD")
            || (code.starts_with('D') && matches!(code.chars().nth(1), Some(' ' | 'D')));
        if deleted {
            overlaid.remove(path);
            continue;
        }
        let changed = code.contains('?') || trimmed.chars().any(|c| "MARCUT".contains(c));
        if changed {
            // `None` is a path that vanished between the status and the hash.
            match blob_of(path) {
                Some(entry) => overlaid.insert(path.clone(), entry),
                None => overlaid.remove(path),
            };
            continue;
        }
        return Err(Trouble::Unreadable(format!(
            "unclassified git status code {code:?} for {path:?}"
        )));
    }
    Ok(overlaid)
}

#[cfg(test)]
mod tests;
