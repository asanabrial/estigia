//! Which class of answer each refusal code is.
//!
//! One table, four lists, and a code sits in exactly one of them. The rules for
//! placing a code, and every ambiguous placement, are in
//! `docs/refusal-classes.md`; the crossing tests below fail when a code the
//! transport can answer with, or one the harness gate raises for a review, has
//! no place here.

use crate::outcome::RefusalClass;

const UNAVAILABLE: &[&str] = &[
    "agent-definition-unreadable",
    "ancestry-unknown",
    "assisted-issue-unreadable",
    "board-readback-failed",
    "capture-runner-lost",
    "capture-running",
    "clock-unreadable",
    "config-local-unreadable",
    "correction-state-unparseable",
    "draft-pr-readback-failed",
    "draft-readback-failed",
    "executable-not-resolvable",
    "flush-detach-failed",
    "gate-input-not-json",
    "generated-reproduction-unavailable",
    "home-not-resolvable",
    "home-unknown",
    "install-record-unreadable",
    "label-readback-failed",
    "lifecycle-state-unreadable",
    "linear-unreachable",
    "mcp-stream-failed",
    "message-holder-unreadable",
    "message-question-failed",
    "message-source-unreadable",
    "message-store-unavailable",
    "no-terminal-for-the-screen",
    "outbox-pending",
    "payload-digest-unavailable",
    "prune-read-failed",
    "publication-lane-never-ran",
    "publication-lane-unfinished",
    "publication-unconfirmed",
    "published-transaction-unparseable",
    "push-hook-unreadable",
    "rate-limited",
    "read-failed",
    "ready-readback-failed",
    "recover-login-unresolved",
    "recover-permission-unreadable",
    "repository-configuration-unreadable",
    "review-checkout-unreadable",
    "review-computed-verdict-readback-failed",
    "review-disposition-readback-failed",
    "review-finding-readback-failed",
    "review-head-unreadable",
    "review-lens-report-readback-failed",
    "review-refutation-readback-failed",
    "review-status-issue-not-in-store",
    "review-status-store-unreadable",
    "reviewer-canonical-unavailable",
    "reviewer-deadline",
    "reviewer-inspect-unreadable",
    "reviewer-no-answer",
    "reviewer-project-unprovable",
    "reviewer-unavailable",
    "run-pointer-unreadable",
    "run-pointers-unreadable",
    "sessions-store-unreadable",
    "setting-not-read-back",
    "spend-readback-failed",
    "store-import-read-failed",
    "terminal-unusable",
    "working-directory-unknown",
];
const STALE: &[&str] = &[
    "already-owned-by-different-operation",
    "assisted-claim-not-in-force",
    "assisted-claim-stale",
    "base-moved-during-creation",
    "capture-result-stale",
    "claim-metadata-mismatch",
    "claim-operation-conflict",
    "claim-operation-expired",
    "claim-operation-no-longer-current",
    "computed-verdict-stale",
    "computed-verdict-tier-mismatch",
    "expected-epoch-moved",
    "fresh-branch-diverged",
    "held-by-other-holder-disappeared",
    "held-by-other-without-other-holder",
    "import-diverged",
    "lost-claim-race",
    "lost-reclaim-race",
    "not-current-live-holder",
    "operation-id-kind-conflict",
    "ownership-changed-projections-repaired",
    "ownership-kept-changing-before-projection",
    "payload-rebuild-mismatch",
    "publication-base-not-ancestor",
    "publication-readback-disagrees",
    "published-transaction-mismatch",
    "reclaim-discarded",
    "reclaim-metadata-mismatch",
    "reclaim-operation-conflict",
    "reclaim-operation-no-longer-current",
    "release-pr-mismatch",
    "release-target-mismatch",
    "remote-not-reachable-from-local",
    "review-computed-verdict-metadata-mismatch",
    "review-computed-verdict-operation-conflict",
    "review-disposition-metadata-mismatch",
    "review-disposition-operation-conflict",
    "review-finding-metadata-mismatch",
    "review-finding-operation-conflict",
    "review-handoff-metadata-mismatch",
    "review-handoff-operation-conflict",
    "review-lens-report-metadata-mismatch",
    "review-lens-report-operation-conflict",
    "review-refutation-metadata-mismatch",
    "review-refutation-operation-conflict",
    "review-target-mismatch",
    "review-target-moved-during-read",
    "reviewer-definition-changed",
    "sessions-cursor-expired",
    "sessions-cursor-store-replaced",
    "spend-metadata-mismatch",
    "spend-operation-conflict",
    "stale-foreign-requires-reclaim",
    "stale-self-requires-claim",
    "target-operation-mismatch",
    "unassign-metadata-mismatch",
    "unassign-operation-conflict",
    "unexpected-state",
    "verdict-bound-to-other-bytes",
];
const UNKNOWN: &[&str] = &[
    "ambiguous-write",
    "push-guard-not-read-back",
    "push-guard-removal-not-read-back",
    "repository-readback-missing",
    "setup-write-failed",
    "uninstall-not-read-back",
    "write-failed",
];
const REFUSED: &[&str] = &[
    "agent-configuration-unwritable",
    "agent-file-not-editable",
    "agent-not-named",
    "agent-retired",
    "agent-unknown",
    "aliased-worktree-path",
    "already-holding",
    "already-yours",
    "ambiguous-changelog-entry",
    "ambiguous-open-prs",
    "approved-republish-without-cause",
    "assist-issue-does-not-match-the-tracker",
    "assisted-does-not-deliver",
    "assisted-outside-the-worktree",
    "assisted-publishes-through-draft-pr",
    "assisted-push-unconfined",
    "assisted-worktree-not-started",
    "assisted-write-unplaced",
    "autonomous-needs-github-tracker",
    "blank-argument",
    "blank-disposition-reason",
    "blank-escalate-reason",
    "blank-focus-reason",
    "blank-never-ran-evidence",
    "blank-recover-reason",
    "blank-spend-conditions",
    "blank-topology",
    "blank-topology-reason",
    "body-violates-template",
    "branch-checked-out-elsewhere",
    "branch-locked-by-another-run",
    "capture-already-running",
    "capture-input-not-offered",
    "capture-job-missing",
    "capture-job-other-run",
    "case-insensitive-branch-collision",
    "changelog-not-found",
    "closing-keyword-live",
    "comment-body-invalid",
    "comment-marker-incomplete",
    "companion-unknown",
    "companion-with-a-workspace",
    "complete-review-transaction-missing",
    "complete-review-transaction-not-selected",
    "computed-verdict-already-recorded",
    "computed-verdict-escalated",
    "computed-verdict-incomplete",
    "computed-verdict-rejected",
    "computed-verdict-unrecorded",
    "config-value-unrecognised",
    "config-value-untableable",
    "contract-is-shared",
    "contract-needs-an-agent",
    "contract-not-writable",
    "control-message",
    "control-surface-not-installed",
    "correction-already-spent",
    "correction-budget-exceeded",
    "correction-outside-reviewed-scope",
    "delegated-definition-unowned",
    "delivery-not-verified",
    "delivery-pr-mismatch",
    "delivery-pr-repository-option",
    "delivery-pr-unidentified",
    "delivery-route-retired",
    "delivery-topology-mismatch",
    "dirty-review-target",
    "disposition-batch-duplicate-finding",
    "disposition-filed-issue-missing",
    "disposition-includes-severe-finding",
    "disposition-transaction-unknown",
    "disposition-unexpected-filed-issue",
    "disposition-unknown-finding",
    "draft-pr-branch-not-checked-out",
    "draft-pr-exists-ready",
    "draft-pr-run-not-assisted",
    "empty-changelog-entry",
    "environment-not-ready",
    "escalate-review-already-approved",
    "escalate-review-correction-unspent",
    "executable-path-not-quotable",
    "finding-citation-unresolvable",
    "finding-id-unknown",
    "finding-location-malformed",
    "finding-location-outside-the-change",
    "finding-location-required",
    "finding-parent-unknown",
    "finding-parent-without-lineage",
    "finding-text-too-large",
    "finding-without-transaction",
    "findings-transaction-unknown",
    "focus-reason-without-focus",
    "force-reason-invalid",
    "force-reason-required",
    "force-required-for-reason",
    "foreign-projection",
    "gate-identity-incomplete",
    "gate-identity-over-specified",
    "generated-declaration-malformed",
    "generated-reproduction-mismatch",
    "genesis-manifest-too-large",
    "git-operation-window-denied",
    "goal-activity-body-unwritable",
    "goal-identity-missing",
    "harness-not-installed",
    "heartbeat-body-invalid",
    "holder-not-stale",
    "holder-runtime-missing",
    "hook-event-unknown",
    "incomplete-review-finding",
    "incomplete-review-transaction",
    "inspect-parent-missing",
    "installer-downgrade-blocked",
    "integration-out-of-phase",
    "integration-shape-not-exact",
    "invalid-blocked-clause",
    "invalid-disposition-batch",
    "invalid-disposition-word",
    "invalid-escalate-target",
    "invalid-finding-causal-disposition",
    "invalid-finding-class",
    "invalid-finding-evidence-class",
    "invalid-finding-lens",
    "invalid-finding-origin",
    "invalid-horizon",
    "invalid-lens-focus",
    "invalid-lens-role",
    "invalid-marker-attribute",
    "invalid-operation-id",
    "invalid-recover-login",
    "invalid-refutation-batch",
    "invalid-refutation-outcome",
    "invalid-risk-raise",
    "invalid-spend-phase",
    "invalid-task",
    "invalid-unassign-target",
    "invalid-validator-result",
    "issue-creation-outside-flow",
    "issue-has-no-number",
    "issue-not-a-number",
    "issue-not-blocked",
    "issue-not-open",
    "issue-required",
    "judge-workspaces-count-too-large",
    "judge-workspaces-inside-the-claimed-checkout",
    "judge-workspaces-uncreatable",
    "language-malformed",
    "legacy-verdict-rejected",
    "legacy-worktree-registered",
    "lens-finding-location-outside-the-diff",
    "lens-finding-location-required",
    "lifecycle-release-publication-failed",
    "linear-credential-malformed",
    "linear-credential-missing",
    "linear-credential-rejected",
    "linear-is-per-repository",
    "linear-issue-unknown",
    "linear-needs-assisted",
    "linear-team-mismatch",
    "loop-not-writable",
    "malformed-argument",
    "merge-strategy-retired",
    "message-empty",
    "message-issue-has-no-holder",
    "message-needs-a-person",
    "message-not-allowed",
    "message-recipient-unknown",
    "message-too-long",
    "missing-argument",
    "missing-pr-body",
    "never-ran-evidence-invalid",
    "new-blocker-without-origin",
    "no-changelog-entry",
    "no-worktree-location",
    "non-positive-spend-value",
    "nothing-configured",
    "nothing-held",
    "nothing-to-reclaim",
    "nothing-to-unassign",
    "operation-has-no-tool",
    "out-of-phase",
    "outbox-bypass",
    "payload-line-not-found",
    "payload-membership-transaction-unknown",
    "planning-protocol-retired",
    "publication-comment-too-large",
    "publication-epoch-not-found",
    "publication-lane-forbidden",
    "publication-lane-red",
    "published-transaction-missing",
    "push-guard-left-alone",
    "push-hook-belongs-to-somebody-else",
    "push-hook-carries-your-lines",
    "push-hook-not-writable",
    "qualifying-review-verdict-missing",
    "read-only-tracker-admits-only-assisted",
    "recorded-by-capture-only",
    "recorded-downgrade-blocked",
    "recover-epoch-ambiguous",
    "recover-epoch-unknown",
    "recover-is-for-a-person",
    "recover-no-escalated-receipt",
    "recover-not-a-maintainer",
    "recover-receipt-not-escalated",
    "recovery-record-missing",
    "refutation-batch-already-recorded",
    "refutation-batch-duplicate-finding",
    "refutation-batch-incomplete",
    "refutation-includes-deterministic-finding",
    "refutation-unknown-finding",
    "rejected-on-insufficient-evidence",
    "release-repository-mismatch",
    "renewal-kind-unsupported",
    "repair-ceiling-retired",
    "repair-range-disagrees-with-recorded-lineage",
    "repo-ref-malformed",
    "repository-cannot-loosen",
    "repository-configuration-unwritable",
    "repository-host-has-no-transport",
    "repository-rows-unsaved",
    "reserved-comment-kind",
    "reserved-device-component",
    "review-delegation-retired",
    "review-handoff-requester-excluded",
    "review-payload-too-large",
    "review-protocol-retired",
    "review-receipt-escalated",
    "review-verdict-retired",
    "reviewer-cli-missing",
    "reviewer-definition-unowned",
    "reviewer-error-result",
    "reviewer-exited-nonzero",
    "reviewer-input-too-large",
    "reviewer-inspect-absent-side",
    "reviewer-inspect-outside-correction",
    "reviewer-inspect-transaction-unresolved",
    "reviewer-launched-by-the-binary",
    "reviewer-launch-failed",
    "not-a-repository",
    "reviewer-not-distinct",
    "reviewer-output-too-large",
    "reviewer-payload-mismatch",
    "reviewer-project-shadow",
    "reviewer-result-malformed",
    "risk-raise-lowers",
    "risk-raise-reason-missing",
    "risk-raise-reason-without-raise",
    "run-id-names-another-checkout",
    "run-id-names-no-run",
    "run-pointer-not-writable",
    "runtime-names-no-runtime",
    "screen-language-unwritable",
    "setting-needs-an-agent",
    "setting-not-for-this-agent",
    "setting-not-per-agent",
    "setting-not-the-repositorys",
    "setting-only-the-repositorys",
    "setting-shadowed-by-agent-file",
    "setting-shadowed-by-local-file",
    "setting-unknown",
    "setup-prevalidation-failed",
    "several-irreversible-fragments-on-one-line",
    "several-runs-hold-this-checkout",
    "sessions-cursor-malformed",
    "sessions-cursor-mismatch",
    "sessions-cursor-tampered",
    "sessions-cursor-too-large",
    "sessions-limit-too-large",
    "sessions-unregistered-not-bounded",
    "skill-not-installed",
    "skill-root-unknown",
    "source-build-not-allowed",
    "spend-transaction-unknown",
    "stand-down-covers-no-time",
    "stand-down-is-for-a-person",
    "stand-down-needs-a-reason",
    "stand-down-not-lifted",
    "stand-down-too-long",
    "stand-down-unserialisable",
    "stand-down-unwritable",
    "stop-after-not-writable",
    "store-unwritable",
    "task-done-without-commit",
    "task-incomplete",
    "tool-arguments-invalid",
    "tool-outside-declared-role",
    "topology-reason-without-topology",
    "tracker-has-no-transport",
    "transport-refused",
    "unclaimed-commit",
    "unflagged-on-trunk",
    "unknown-operation",
    "unsafe-run-id",
    "unsafe-worktree-component",
    "validator-attempts-exhausted",
    "validator-delta-carries-upstream",
    "validator-transaction-not-a-correction",
    "validator-result-already-recorded",
    "verdict-bound-to-other-repository",
    "workflow-assisted-writes-no-tracker",
    "workflow-not-assisted",
    "workspace-without-a-path",
    "workspace-without-scope",
    "worktree-location-not-absolute",
    "worktree-owned-by-another-run",
    "worktree-ownership-unproven",
    "worktree-path-occupied",
];

const CLASSED: [(&[&str], RefusalClass); 4] = [
    (UNAVAILABLE, RefusalClass::Unavailable),
    (STALE, RefusalClass::Stale),
    (UNKNOWN, RefusalClass::Unknown),
    (REFUSED, RefusalClass::Refused),
];

/// The class a code is placed in, or `None` for a code the table does not name.
pub fn class_of(code: &str) -> Option<RefusalClass> {
    CLASSED
        .iter()
        .find(|(codes, _)| codes.contains(&code))
        .map(|(_, class)| *class)
}

/// Every code the table places, with its class.
pub fn classified() -> impl Iterator<Item = (&'static str, RefusalClass)> {
    CLASSED
        .iter()
        .flat_map(|(codes, class)| codes.iter().map(move |code| (*code, *class)))
}

/// The class a refusal built with `code` carries: the table's, and
/// `unavailable` for a code it does not name.
///
/// Unnamed is never `refused`, which would claim a rule said no that nobody
/// wrote down. The crossing tests name the codes they reach: the transport's
/// vocabulary and every literal code the crate's sources spell. A code a
/// refusal receives through a variable other than `CODE` is not crossed.
pub fn class_of_code(code: &str) -> RefusalClass {
    class_of(code).unwrap_or(RefusalClass::Unavailable)
}

/// The class of a refusal the transport answered.
///
/// Only an exit `1` stop reads the table, and a reason it does not name is the
/// rule saying no. Exit `3` is `unavailable`, `5` is `unknown`, and `2` is
/// `refused` whatever the reason says; any other exit is an answer the contract
/// does not describe and is `unknown`, never a rule.
pub fn class_of_answer(exit_code: i32, reason: &str) -> RefusalClass {
    match exit_code {
        1 => class_of(reason).unwrap_or(RefusalClass::Refused),
        2 => RefusalClass::Refused,
        3 => RefusalClass::Unavailable,
        _ => RefusalClass::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::tracker::TRANSPORT_VOCABULARY;

    const GATE_CODES: [&str; 9] = [
        "delivery-pr-unidentified",
        "delivery-pr-repository-option",
        "complete-review-transaction-missing",
        "delivery-pr-mismatch",
        "complete-review-transaction-not-selected",
        "verdict-bound-to-other-bytes",
        "review-head-unreadable",
        "review-checkout-unreadable",
        "verdict-bound-to-other-repository",
    ];

    #[test]
    fn every_code_the_transport_answers_with_has_a_class() {
        let unclassed: Vec<&&str> = TRANSPORT_VOCABULARY
            .iter()
            .filter(|code| class_of(code).is_none())
            .collect();
        assert!(
            unclassed.is_empty(),
            "unclassed transport codes: {unclassed:?}"
        );
    }

    #[test]
    fn a_refusal_the_gate_builds_for_a_rule_that_said_no_is_refused() {
        for code in [
            "issue-required",
            "issue-creation-outside-flow",
            "unclaimed-commit",
        ] {
            assert_eq!(class_of_code(code), RefusalClass::Refused, "`{code}`");
        }
    }

    #[test]
    fn every_gate_code_and_the_catch_all_has_a_class() {
        for code in GATE_CODES.iter().chain(&["transport-refused"]) {
            assert!(class_of(code).is_some(), "`{code}` has no class");
        }
    }

    #[test]
    fn a_code_sits_in_exactly_one_list() {
        let mut seen = std::collections::BTreeSet::new();
        for (code, _) in classified() {
            assert!(
                seen.insert(code),
                "`{code}` is in two lists or twice in one"
            );
        }
    }

    #[test]
    fn nothing_that_could_not_be_read_is_classed_as_a_rule_or_a_moved_receipt() {
        for (code, class) in classified() {
            if ["unreadable", "unavailable", "unparseable"]
                .iter()
                .any(|word| code.contains(word))
            {
                assert_eq!(class, RefusalClass::Unavailable, "`{code}`");
            }
        }
    }

    #[test]
    fn an_exit_the_contract_does_not_name_is_unknown_never_a_rule() {
        for exit in [0, 4, 6, 101, -1] {
            assert_eq!(
                class_of_answer(exit, "claim-operation-conflict"),
                RefusalClass::Unknown,
                "{exit}"
            );
        }
    }

    #[test]
    fn a_read_that_failed_is_never_a_state_and_an_unknown_write_is_never_named() {
        for reason in [
            "read-failed",
            "rate-limited",
            "claim-operation-conflict",
            "no-such",
        ] {
            assert_eq!(
                class_of_answer(3, reason),
                RefusalClass::Unavailable,
                "{reason}"
            );
        }
        for reason in [
            "write-failed",
            "ambiguous-write",
            "claim-operation-conflict",
        ] {
            assert_eq!(
                class_of_answer(5, reason),
                RefusalClass::Unknown,
                "{reason}"
            );
        }
    }

    #[test]
    fn the_unreadable_is_not_the_moved_and_neither_is_the_rule() {
        assert_eq!(
            class_of_answer(1, "published-transaction-unparseable"),
            RefusalClass::Unavailable
        );
        assert_eq!(
            class_of_answer(1, "published-transaction-mismatch"),
            RefusalClass::Stale
        );
        assert_eq!(
            class_of_answer(1, "reviewer-not-distinct"),
            RefusalClass::Refused
        );
        assert_eq!(
            class_of_answer(2, "malformed-argument"),
            RefusalClass::Refused
        );
        assert_eq!(
            class_of_answer(1, "a-reason-nobody-named"),
            RefusalClass::Refused
        );
    }
}
