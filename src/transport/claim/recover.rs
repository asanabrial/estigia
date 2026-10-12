//! Recovering an escalated review: a named maintainer's role, read from GitHub, a
//! local record, and the policy a successor lineage inherits.

use super::{
    Ledger, ReviewTransaction, Standing, all_publications, receipt_standing,
    require_recorded_receipt, timeline,
};
use crate::transport::tier::{ReviewFocus, RiskLevel};
use crate::transport::{Context, Failure, ownership};

const MAINTAINER_ROLES: &[&str] = &["admin", "maintain"];

/// What a successor lineage keeps of the lineage it replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InheritedPolicy {
    pub(super) level: RiskLevel,
    pub(super) focus: ReviewFocus,
}

struct Recovery {
    receipt: ReviewTransaction,
    actor: String,
    permission: String,
    policy: InheritedPolicy,
}

impl Recovery {
    fn from_marker(marker: &crate::transport::markers::Marker) -> Option<Self> {
        Some(Self {
            receipt: ReviewTransaction::from_marker(marker)?,
            actor: marker.get("actor")?.clone(),
            permission: marker.get("permission")?.clone(),
            policy: InheritedPolicy {
                level: RiskLevel::parse(marker.get("level")?)?,
                focus: ReviewFocus::parse(Some(marker.get("focus")?)).ok()?,
            },
        })
    }
}

fn recoveries(comments: &[ownership::Comment]) -> Vec<Recovery> {
    comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| crate::transport::markers::parse(&comment.body))
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("review-recovery"))
        .filter_map(|marker| Recovery::from_marker(&marker))
        .collect()
}

/// Whether a maintainer recovered exactly this receipt.
pub(super) fn recovered(comments: &[ownership::Comment], receipt: &ReviewTransaction) -> bool {
    recoveries(comments)
        .iter()
        .any(|recovery| recovery.receipt == *receipt)
}

/// The policy a receipt inherits from the lineage a recovery replaced, found by
/// walking its parents on its own pull request back to the successor's root. A
/// lineage that began by a first publication, or over an approved parent,
/// inherits nothing.
pub(super) fn inherited_policy(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Option<InheritedPolicy>, Failure> {
    let publications = all_publications(comments)?;
    let recorded = recoveries(comments);
    let mut current = receipt.clone();
    for _ in 0..=publications.len() {
        let Some(found) = publications.iter().find(|found| found.receipt == current) else {
            return Ok(None);
        };
        let Some(lineage) = found.lineage.as_ref().filter(|l| l.parent.pr == current.pr) else {
            return Ok(None);
        };
        if lineage.recovered {
            return recorded
                .iter()
                .find(|recovery| recovery.receipt == lineage.parent)
                .map(|recovery| Some(recovery.policy))
                .ok_or_else(|| {
                    Failure::Stop(serde_json::json!({
                        "ok": false,
                        "reason": "recovery-record-missing",
                        "parent": lineage.parent.epoch,
                        "action": "this publication descends from a receipt a maintainer \
                                   recovered, and no readable local recovery record matches \
                                   it, so the tier and lens focus it must keep are unknown; \
                                   restore the local store, or publish the work as a new \
                                   change on a new pull request",
                    }))
                });
        }
        if receipt_standing(comments, &lineage.parent)?.standing == Standing::Approved {
            return Ok(None);
        }
        current = lineage.parent.clone();
    }
    Ok(None)
}

/// What a recovery asks.
pub struct RecoverReview<'a> {
    /// The issue under review.
    pub issue: u64,
    /// The escalated publication's epoch, from which the exact receipt is read; the one receipt
    /// of the issue that stands escalated and is not yet recovered when absent.
    pub epoch: Option<&'a str>,
    /// The GitHub login of the maintainer authorising the recovery; the authenticated `gh` user
    /// when absent.
    pub authorized_by: Option<&'a str>,
    /// Why the review is being recovered.
    pub reason: &'a str,
    /// The repository that hosts the pull request, `owner/name`: the Repository host row's, not the
    /// tracker's, because the role is the one the code's own repository grants.
    pub host_repo: &'a str,
    /// The moment the timeline is read.
    pub now: &'a str,
}

/// `owner/name` of the repository `context` resolves to when no repository is named: the
/// checkout's own, which is what hosts the pull request when the host row names none.
pub fn host_repo_of(context: &Context) -> Result<String, Failure> {
    crate::transport::closing::repo_identity(context).map(|(owner, name)| format!("{owner}/{name}"))
}

fn refuse(reason: &str, detail: serde_json::Value, action: &str) -> Failure {
    let mut envelope = serde_json::json!({ "ok": false, "reason": reason, "action": action });
    if let (Some(into), Some(from)) = (envelope.as_object_mut(), detail.as_object()) {
        into.extend(from.clone());
    }
    Failure::Stop(envelope)
}

/// The role GitHub says a login holds, and its coarse permission.
///
/// `permission` is only `admin`, `write`, `read` or `none`, so a `maintain` role
/// is read from `role_name`; where that is absent the permission stands alone.
pub(super) fn role_of(
    context: &Context,
    host_repo: &str,
    login: &str,
) -> Result<(String, String), Failure> {
    let unreadable = |detail: String| {
        refuse(
            "recover-permission-unreadable",
            serde_json::json!({ "login": login, "detail": detail }),
            "GitHub did not say what permission this login holds, and an unanswered question is \
             not a maintainer; check the login and the token's access, then ask again",
        )
    };
    if host_repo.split('/').filter(|part| !part.is_empty()).count() != 2 {
        return Err(unreadable(format!("{host_repo:?} is not owner/name")));
    }
    let path = format!("repos/{host_repo}/collaborators/{login}/permission");
    let data = crate::transport::gh_json(&["api", &path], Some(&context.repo_dir))
        .map_err(|failure| unreadable(failure.detail()))?
        .ok_or_else(|| unreadable("GitHub answered nothing".to_owned()))?;
    let word = |key: &str| {
        data.get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let permission = word("permission");
    match (word("role_name").or_else(|| permission.clone()), permission) {
        (Some(role), permission) => Ok((role, permission.unwrap_or_default())),
        _ => Err(unreadable(
            "the answer named no role or permission".to_owned(),
        )),
    }
}

/// The login `gh` is authenticated as.
fn authenticated_login(context: &Context) -> Result<String, Failure> {
    let unresolved = |detail: String| {
        refuse(
            "recover-login-unresolved",
            serde_json::json!({ "detail": detail }),
            "`gh` did not say which login it is authenticated as; name the maintainer's login \
             with `--authorized-by`",
        )
    };
    let data = crate::transport::gh_json(&["api", "user"], Some(&context.repo_dir))
        .map_err(|failure| unresolved(failure.detail()))?
        .ok_or_else(|| unresolved("GitHub answered nothing".to_owned()))?;
    data.get("login")
        .and_then(serde_json::Value::as_str)
        .filter(|login| !login.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| unresolved("the answer named no login".to_owned()))
}

/// The epoch of the one receipt that stands escalated and is not yet recovered: the newest
/// publication of each pull request, so a superseded one is never a candidate.
fn escalated_epoch(comments: &[ownership::Comment]) -> Result<String, Failure> {
    let publications = all_publications(comments)?;
    let mut candidates: Vec<String> = Vec::new();
    for (position, found) in publications.iter().enumerate() {
        let superseded = publications[position + 1..]
            .iter()
            .any(|later| later.receipt.pr == found.receipt.pr);
        if !superseded
            && receipt_standing(comments, &found.receipt)?.escalated
            && !recovered(comments, &found.receipt)
        {
            candidates.push(found.receipt.epoch.clone());
        }
    }
    match candidates.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(refuse(
            "recover-no-escalated-receipt",
            serde_json::json!({}),
            "no receipt on this issue stands escalated and unrecovered, so there is nothing to \
             recover; a rejected change is corrected, an approved one is released, and one that \
             was already recovered is published again",
        )),
        _ => Err(refuse(
            "recover-epoch-ambiguous",
            serde_json::json!({ "candidates": candidates }),
            "more than one receipt on this issue stands escalated; name the one to recover with \
             `--epoch`",
        )),
    }
}

/// Recovers an escalated review transaction, on the word of a named maintainer.
///
/// A person's operation: it runs from `estigia recover`, which the gate refuses an
/// agent's shell line for, and it is not an MCP tool. Refused unless the epoch
/// names the newest publication on its pull request, that receipt stands
/// escalated, and GitHub itself says the named login holds the `admin` or
/// `maintain` role. The record is local and posts nothing. It releases nothing:
/// the next publication on the pull request is a fresh candidate in a successor
/// lineage — a new correction, a new budget — that keeps this lineage's tier and
/// lens focus and is reviewed again.
pub fn recover_review(
    context: &Context,
    request: &RecoverReview<'_>,
) -> Result<serde_json::Value, Failure> {
    if request.reason.trim().is_empty() {
        return Err(refuse(
            "blank-recover-reason",
            serde_json::json!({}),
            "state why the review is being recovered",
        ));
    }
    let named_login = match request.authorized_by {
        Some(named) => named.to_owned(),
        None => authenticated_login(context)?,
    };
    let login = named_login.trim();
    if login.is_empty()
        || !login
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "invalid-recover-login",
            "action": "name the GitHub login that authorises the recovery: letters, digits, `-` and `_`",
        })));
    }
    let comments = timeline(context, request.issue, request.now)?;
    let publications = all_publications(&comments)?;
    let epoch = match request.epoch {
        Some(named) => named.to_owned(),
        None => escalated_epoch(&comments)?,
    };
    let Some(receipt) = publications
        .iter()
        .find(|found| found.receipt.epoch == epoch)
        .map(|found| found.receipt.clone())
    else {
        return Err(refuse(
            "recover-epoch-unknown",
            serde_json::json!({
                "recorded": publications.iter().map(|found| found.receipt.epoch.clone()).collect::<Vec<_>>(),
            }),
            "name the epoch of a publication this issue recorded",
        ));
    };
    require_recorded_receipt(&comments, &receipt, "cannot be recovered")?;
    let standing = receipt_standing(&comments, &receipt)?;
    if !standing.escalated {
        return Err(refuse(
            "recover-receipt-not-escalated",
            serde_json::json!({ "standing": standing.standing.word() }),
            "only a receipt that stands escalated is recovered; a rejected one is corrected, an \
             approved one is released, and an unjudged one is reviewed",
        ));
    }
    if let Some(existing) = recoveries(&comments)
        .iter()
        .find(|recovery| recovery.receipt == receipt)
    {
        return Ok(serde_json::json!({
            "ok": true,
            "issue": request.issue,
            "epoch": receipt.epoch,
            "authorized_by": existing.actor,
            "role": existing.permission,
            "level": existing.policy.level.as_str(),
            "focus": existing.policy.focus.word(),
            "reused_existing_recovery": true,
        }));
    }
    let (role, permission) = role_of(context, request.host_repo, login)?;
    if !MAINTAINER_ROLES.contains(&role.as_str()) && permission != "admin" {
        return Err(refuse(
            "recover-not-a-maintainer",
            serde_json::json!({ "login": login, "role": role, "permission": permission }),
            "GitHub says this login holds neither the `admin` nor the `maintain` role on the \
             repository, so it cannot authorise a recovery; a maintainer names their own login",
        ));
    }
    let policy = super::computed::policy_for(context, &comments, &receipt, None)?;
    let pr_field = receipt.pr.to_string();
    let marker = crate::transport::markers::render(
        "review-recovery",
        &[
            ("run-id", "person"),
            ("epoch", &receipt.epoch),
            ("pr", &pr_field),
            ("head", &receipt.head),
            ("base", &receipt.base),
            ("digest", &receipt.digest),
            ("actor", login),
            ("permission", &role),
            ("reason", request.reason.trim()),
            ("level", policy.level.as_str()),
            ("focus", policy.focus.word()),
        ],
    )
    .ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    Ledger::open(context, request.issue, request.now)?
        .append(&format!("{marker}\n"), request.now)?;
    Ok(serde_json::json!({
        "ok": true,
        "issue": request.issue,
        "epoch": receipt.epoch,
        "authorized_by": login,
        "role": role,
        "level": policy.level.as_str(),
        "focus": policy.focus.word(),
        "reused_existing_recovery": false,
    }))
}
