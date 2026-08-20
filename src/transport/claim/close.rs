//! Closed-issue renewal.

use super::{LatestPublication, ReviewReceipt, ownership};
use crate::transport::{Context, Failure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnClose {
    pub pr: u64,
    pub epoch: String,
    pub delivering_sha: String,
}
pub(crate) fn match_own_close(
    receipt: &ReviewReceipt,
    closing_prs: &[u64],
    delivering_sha: &str,
    first_parent: &str,
    second_parent: &str,
    pin: Option<u64>,
) -> Option<OwnClose> {
    if pin.is_some_and(|pr| pr != receipt.pr) {
        return None;
    }
    if !closing_prs.contains(&receipt.pr) {
        return None;
    }
    if delivering_sha.len() != 40 || first_parent != receipt.base || second_parent != receipt.head {
        return None;
    }
    Some(OwnClose {
        pr: receipt.pr,
        epoch: receipt.epoch.clone(),
        delivering_sha: delivering_sha.to_owned(),
    })
}
pub(crate) fn own_delivery_close(
    context: &Context,
    issue: u64,
    run_id: &str,
    comments: &[ownership::Comment],
    pin: Option<u64>,
) -> Result<Option<OwnClose>, Failure> {
    let closing = super::super::closing::closing_refs(context, issue)?;
    let closing_prs: Vec<u64> = closing
        .iter()
        .filter_map(|r| r.get("number").and_then(serde_json::Value::as_u64))
        .collect();
    let Some(publication) = comments
        .iter()
        .filter(|c| c.viewer_did_author && !c.includes_created_edit)
        .flat_map(|c| super::super::markers::parse(&c.body))
        .filter(|m| m.get("kind").map(String::as_str) == Some("published"))
        .filter_map(|m| {
            let p = LatestPublication {
                publisher: m.get("run-id")?.clone(),
                receipt: ReviewReceipt::from_marker(&m)?,
                lineage: super::PublicationLineage::from_marker(&m),
            };
            (p.publisher == run_id).then_some(p)
        })
        .next_back()
    else {
        return Ok(None);
    };
    let receipt = &publication.receipt;
    if pin.is_some_and(|pr| pr != receipt.pr) || !closing_prs.contains(&receipt.pr) {
        return Ok(None);
    }
    let Some((sha, first, second)) = closing_merge(context, receipt.pr)? else {
        return Ok(None);
    };
    Ok(match_own_close(
        receipt,
        &closing_prs,
        &sha,
        &first,
        &second,
        pin,
    ))
}
fn closing_merge(context: &Context, pr: u64) -> Result<Option<(String, String, String)>, Failure> {
    let number = pr.to_string();
    let data = crate::transport::gh_json(
        &["pr", "view", &number, "--json", "mergeCommit"],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh pr view {pr} returned nothing")))?;
    let Some(sha) = data
        .get("mergeCommit")
        .and_then(|c| c.get("oid"))
        .and_then(serde_json::Value::as_str)
        .filter(|v| v.len() == 40 && v.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return Ok(None);
    };
    let (owner, name) = crate::transport::closing::repo_identity(context)?;
    let path = format!("repos/{owner}/{name}/commits/{sha}");
    let commit = crate::transport::gh_json(&["api", &path], Some(&context.repo_dir))?
        .ok_or_else(|| Failure::Read(format!("gh api {path} returned nothing")))?;
    let parents: Vec<String> = commit
        .get("parents")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Failure::Read(format!("gh api {path} omitted parents")))?
        .iter()
        .filter_map(|p| {
            p.get("sha")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    if parents.len() != 2 {
        return Ok(None);
    }
    Ok(Some((
        sha.to_ascii_lowercase(),
        parents[0].to_ascii_lowercase(),
        parents[1].to_ascii_lowercase(),
    )))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> ReviewReceipt {
        ReviewReceipt {
            epoch: "a".repeat(32),
            pr: 42,
            head: "b".repeat(40),
            base: "c".repeat(40),
            digest: "d".repeat(64),
        }
    }

    fn sha() -> String {
        "e".repeat(40)
    }

    #[test]
    fn a_matching_own_merge_permits_the_follow_up() {
        let r = receipt();
        let found = match_own_close(&r, &[42], &sha(), &r.base, &r.head, None)
            .expect("own delivery should permit");
        assert_eq!(found.pr, 42);
        assert_eq!(found.epoch, r.epoch);
        assert_eq!(found.delivering_sha, sha());
    }

    #[test]
    fn a_different_run_pin_or_missing_closer_is_not_ours() {
        let r = receipt();
        assert!(match_own_close(&r, &[42], &sha(), &r.base, &r.head, Some(7)).is_none());
        assert!(match_own_close(&r, &[7], &sha(), &r.base, &r.head, None).is_none());
    }

    #[test]
    fn a_merge_whose_parents_are_not_the_receipt_is_not_ours() {
        let r = receipt();
        assert!(match_own_close(&r, &[42], &sha(), &r.head, &r.base, None).is_none());
        assert!(match_own_close(&r, &[42], "short", &r.base, &r.head, None).is_none());
    }

    #[test]
    fn transition_does_not_refuse_a_closed_issue() {
        let source = include_str!("../commands.rs");
        let start = source
            .find("pub fn transition(")
            .expect("transition exists");
        let body = &source[start..start + 2500];
        assert!(
            !body.contains("issue-not-open") && !body.contains("state != \"OPEN\""),
            "transition grew a closed-issue stand-down, so it no longer agrees with verify_claim"
        );
    }
}
