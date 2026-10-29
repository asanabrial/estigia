//! Refusing a base integration the platform does not require.

use std::collections::BTreeSet;

use super::{LatestPublication, Standing, integrated_parent_tree};
use crate::transport::{Context, Failure, How};

pub(super) enum Integration {
    Allowed,
    Unread(String),
    NotRequired(serde_json::Value),
}

enum Protection {
    Strict,
    NotStrict,
}

pub(super) struct Inputs<'a> {
    pub(super) context: &'a Context,
    pub(super) at: &'a std::path::Path,
    pub(super) base: &'a str,
    pub(super) parent: &'a LatestPublication,
    pub(super) standing: Standing,
    pub(super) payload_base: &'a str,
    pub(super) payload_head: &'a str,
    pub(super) issue: u64,
}

pub(super) fn assess(inputs: &Inputs<'_>) -> Integration {
    let Inputs {
        context,
        at,
        parent,
        payload_base,
        payload_head,
        ..
    } = *inputs;
    let receipt = &parent.receipt;
    match crate::transport::target::is_ancestor(at, payload_base, &receipt.head) {
        Ok(false) => {}
        Ok(true) => return Integration::Allowed,
        Err(failure) => return unread("ancestry", &failure),
    }
    match crate::transport::target::is_ancestor(at, payload_base, payload_head) {
        Ok(true) => {}
        Ok(false) => return Integration::Allowed,
        Err(failure) => return unread("ancestry", &failure),
    }
    match disjoint_and_clean(at, &receipt.head, &receipt.base, payload_base) {
        Ok(true) => {}
        Ok(false) => return Integration::Allowed,
        Err(failure) => return unread("merge-tree", &failure),
    }
    match protection(context, inputs.base) {
        Ok(Protection::NotStrict) => {}
        Ok(Protection::Strict) => return Integration::Allowed,
        Err(detail) => return Integration::Unread(format!("branch-protection: {detail}")),
    }
    Integration::NotRequired(refusal(inputs))
}

fn unread(read: &str, failure: &Failure) -> Integration {
    Integration::Unread(format!("{read}: {}", failure.detail()))
}

pub(super) fn disjoint_and_clean(
    at: &std::path::Path,
    parent_head: &str,
    parent_base: &str,
    payload_base: &str,
) -> Result<bool, Failure> {
    let (_, conflicted) = integrated_parent_tree(at, parent_head, payload_base)?;
    if !conflicted.is_empty() {
        return Ok(false);
    }
    let from = crate::transport::tier::tree_at(at, parent_base)?;
    let base_paths: BTreeSet<String> = crate::transport::tier::tree_pair_changed_paths_at(
        at,
        &from,
        &crate::transport::tier::tree_at(at, payload_base)?,
    )?
    .into_iter()
    .collect();
    let delivery_paths = crate::transport::tier::tree_pair_changed_paths_at(
        at,
        &from,
        &crate::transport::tier::tree_at(at, parent_head)?,
    )?;
    Ok(!delivery_paths.iter().any(|path| base_paths.contains(path)))
}

fn protection(context: &Context, base: &str) -> Result<Protection, String> {
    let (owner, name) =
        crate::transport::closing::repo_identity(context).map_err(|failure| failure.detail())?;
    let path = format!("repos/{owner}/{name}/branches/{base}/protection");
    let output = crate::transport::run(
        &["gh", "api", &path],
        Some(&context.repo_dir),
        How::tolerated(),
    )
    .map_err(|failure| failure.detail())?;
    read_protection(output.status, &output.stdout, &output.stderr)
}

fn read_protection(status: i32, stdout: &str, stderr: &str) -> Result<Protection, String> {
    if status != 0 {
        if stdout.contains("Branch not protected") || stderr.contains("Branch not protected") {
            return Ok(Protection::NotStrict);
        }
        return Err(format!(
            "gh api exited {status}: {}",
            stderr.trim().lines().next().unwrap_or_default()
        ));
    }
    let text = stdout.trim();
    if text.is_empty() {
        return Err("the response was empty".to_owned());
    }
    let data: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("the response was not JSON: {error}"))?;
    if !data.is_object() {
        return Err("the response was not an object".to_owned());
    }
    match data.get("required_status_checks") {
        None | Some(serde_json::Value::Null) => Ok(Protection::NotStrict),
        Some(checks) => match checks.get("strict").and_then(serde_json::Value::as_bool) {
            Some(true) => Ok(Protection::Strict),
            Some(false) => Ok(Protection::NotStrict),
            None => Err("required_status_checks carries no `strict` boolean".to_owned()),
        },
    }
}

fn refusal(inputs: &Inputs<'_>) -> serde_json::Value {
    let receipt = &inputs.parent.receipt;
    let leave = if inputs.standing == Standing::Approved {
        "Leave the branch at the head the panel approved and run `deliver`, which merges it on the \
         advanced base"
    } else {
        "Publish the repair without merging the base in"
    };
    serde_json::json!({
        "ok": false,
        "reason": "base-integration-not-required",
        "issue": inputs.issue,
        "pr": receipt.pr,
        "epoch": receipt.epoch,
        "head": receipt.head,
        "base": inputs.payload_base,
        "action": format!(
            "this publication carries base commits its parent publication did not, but the base's \
             advance touches none of the delivery's paths, git merges it with the parent head \
             without a conflict, and {} does not require a branch to be up to date before it \
             merges. Integrating it would make the panel read the whole change again for \
             nothing. {leave}; or, when the integration is needed anyway, publish again with \
             `integrate_reason` saying why. Nothing has been pushed and no pull request was opened",
            inputs.base
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_strict(read: Result<Protection, String>) -> Option<bool> {
        match read {
            Ok(Protection::Strict) => Some(true),
            Ok(Protection::NotStrict) => Some(false),
            Err(_) => None,
        }
    }

    #[test]
    fn a_protection_read_distinguishes_strict_from_not_and_from_unreadable() {
        let strict = r#"{"required_status_checks":{"strict":true}}"#;
        let loose = r#"{"required_status_checks":{"strict":false}}"#;
        let none = r#"{"enforce_admins":{}}"#;
        assert_eq!(is_strict(read_protection(0, strict, "")), Some(true));
        assert_eq!(is_strict(read_protection(0, loose, "")), Some(false));
        assert_eq!(is_strict(read_protection(0, none, "")), Some(false));
        assert_eq!(
            is_strict(read_protection(
                1,
                "{\"message\":\"Branch not protected\"}",
                "gh: Branch not protected (HTTP 404)"
            )),
            Some(false)
        );
        for (status, stdout, stderr) in [
            (1, "", "gh: Not Found (HTTP 404)"),
            (1, "", "gh: Resource not accessible (HTTP 403)"),
            (0, "", ""),
            (0, "not json", ""),
            (0, "[]", ""),
            (0, r#"{"required_status_checks":{}}"#, ""),
        ] {
            assert_eq!(
                is_strict(read_protection(status, stdout, stderr)),
                None,
                "{status} {stdout:?} {stderr:?}"
            );
        }
    }
}
