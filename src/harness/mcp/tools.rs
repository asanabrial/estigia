//! The workflow operations, as a table.
//!
//! One list, walked by the schema generator, the dispatcher, and the seam
//! test that checks it against the contract — not a `match` and a
//! hand-written schema beside it, which would be two places that drift
//! silently. Every tool maps to an operation `SKILL.md` requires the binding
//! to provide; `every_required_operation_is_exposed_or_declared` holds the
//! two sides together.

use serde_json::{Value, json};

/// What a tool needs from the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argument {
    /// The JSON property name, and the flag it becomes.
    pub name: &'static str,
    /// What it is, for the agent reading the schema.
    pub description: &'static str,
    /// `"integer"`, `"string"` or `"boolean"`.
    ///
    /// `"boolean"` means the flag carries no value — the transport declares
    /// it `action="store_true"` — so the two must agree, which
    /// `a_boolean_argument_is_one_the_transport_takes_without_a_value` checks.
    pub json_type: &'static str,
    /// The only values this argument accepts, when the transport constrains it.
    ///
    /// [`crate::config::STATES`] rather than a list written here, because a
    /// value written twice is a value that will disagree with itself.
    pub choices: Option<&'static [&'static str]>,
    /// The smallest value this argument takes, when it has one.
    ///
    /// The value is rendered straight into the transport's argv, where a
    /// negative one arrives as a **flag** — refused here rather than left to
    /// the process boundary to discover.
    pub least: Option<i64>,
    /// Whether the call is refused without it.
    pub required: bool,
    /// The transport flag this becomes, when it is not `--<name>`.
    pub flag: Option<&'static str>,
}

impl Argument {
    const fn required(
        name: &'static str,
        json_type: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            json_type,
            required: true,
            flag: None,
            choices: None,
            least: None,
        }
    }

    const fn optional(
        name: &'static str,
        json_type: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            json_type,
            required: false,
            flag: None,
            choices: None,
            least: None,
        }
    }

    const fn counting_from(mut self, least: i64) -> Self {
        self.least = Some(least);
        self
    }

    const fn of(mut self, choices: &'static [&'static str]) -> Self {
        self.choices = Some(choices);
        self
    }

    /// The command-line flag this argument becomes.
    pub fn as_flag(&self) -> String {
        match self.flag {
            Some(flag) => flag.to_owned(),
            None => format!("--{}", self.name.replace('_', "-")),
        }
    }
}

/// How the run pointer changes when a tool succeeds.
///
/// The pointer is not authority — see [`super::super::session`] — but it has
/// to follow reality, and the moment reality changes is a successful write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEffect {
    /// Nothing changes.
    None,
    /// This run now holds the issue named in `issue`, in the state it asked for.
    Swear,
    /// This run believes the issue is now in the state named in `to`.
    Moved,
    /// This run holds nothing.
    Forget,
    /// The tracker answered just now, so the renewal window restarts.
    Renew,
    /// A review target was published, bound to the head the answer names.
    Published,
    /// This run was given an isolated checkout, and the work happens there.
    ///
    /// The path comes from the transport's **answer**, not from the
    /// arguments: `worktree_root` is a template the transport expands per
    /// run and per branch.
    Isolated,
    /// This run took an issue the team coordinates: the pointer records the
    /// issue, the branch and the isolated checkout, and nothing on the tracker.
    Assisted,
}

/// One workflow operation, exposed as an MCP tool.
#[derive(Debug, Clone, Copy)]
pub struct Tool {
    /// The MCP tool name, as the agent calls it.
    pub name: &'static str,
    /// The name the binding's operations table gives this row.
    ///
    /// `every_tool_names_a_row_the_binding_actually_has` checks it against
    /// the table, so a tool cannot claim an operation under a name the
    /// binding never used.
    pub contract_name: &'static str,
    /// What it does, for the agent choosing between them.
    pub description: &'static str,
    /// The transport subcommand.
    pub operation: &'static str,
    /// Its arguments, in the order they are passed.
    pub arguments: &'static [Argument],
    /// What a success does to the run pointer.
    pub effect: PointerEffect,
    /// Whether this tool writes to the tracker.
    ///
    /// Exposed in the schema as `readOnlyHint`, so an agent can tell a safe
    /// retry apart from one without reading this file.
    pub writes: bool,
}

/// Which configuration role a tool's executable binding comes from.
///
/// Tracking ops need the tracker; publication and receipt-bound review ops
/// need the repository host. A tracker without a transport must not refuse
/// a host-bound tool, and the reverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportBind {
    /// Issues, claims, comments, states — the tracker.
    Tracker,
    /// Pull requests, tags, CI lane, receipt-bound review writes — the host.
    RepositoryHost,
}

impl Tool {
    /// Which role must supply a transport for this tool.
    ///
    /// Named rather than a field so a tool added later defaults to the
    /// tracker — the common case — and publication tools opt in here.
    pub fn binds(self) -> TransportBind {
        // Publication half of the host only. Receipt markers and handoff write
        // the issue timeline, which is the tracker's ledger — binding those to
        // the host left markers where claim/review_findings would not look when
        // the two pins differ.
        match self.name {
            "publish_review" | "republish_review" | "release_ci" | "draft_pr"
            | "judge_workspaces" => TransportBind::RepositoryHost,
            _ => TransportBind::Tracker,
        }
    }
}

const RUN_ID: Argument = Argument::required(
    "run_id",
    "string",
    "This run's id, as SessionStart reported it.",
);
const ISSUE: Argument = Argument::required("issue", "integer", "Issue number.").counting_from(1);
const FILING_REPO: Argument = Argument::optional(
    "repo",
    "string",
    "Repository to file in, `owner/name`; defaults to this checkout's. Starts unclaimed.",
);
const RECEIPT_EPOCH: Argument = Argument::required("epoch", "string", "Publication epoch.");
const RECEIPT_PR: Argument =
    Argument::required("pr", "integer", "Pull request number.").counting_from(1);
const RECEIPT_HEAD: Argument = Argument::required("head", "string", "Published head SHA.");
const RECEIPT_BASE: Argument = Argument::required("base", "string", "Published base SHA.");
const RECEIPT_DIGEST: Argument = Argument::required("digest", "string", "Target manifest digest.");
// `assist` reads a Linear key (`ENG-123`) as readily as a GitHub number, so its
// issue argument is a string; `dispatch` parses whichever it is.
const ISSUE_ANY: Argument = Argument::required(
    "issue",
    "string",
    "The issue: a number for GitHub, or a Linear key such as `ENG-123`.",
);
// From the transport that adjudicates it, not spelled again here — the same
// reason `choices` above points at `STATES` instead of a literal list.
const FINDING_CLASSES: &[&str] = crate::transport::claim::FINDING_CLASSES;
const REVIEW_FOCUSES: &[&str] = crate::transport::tier::REVIEW_FOCUSES;
const FINDING_ORIGINS: &[&str] = crate::transport::claim::FINDING_ORIGINS;
const FINDING_EVIDENCE_CLASSES: &[&str] = crate::transport::claim::FINDING_EVIDENCE_CLASSES;
const FINDING_CAUSAL_DISPOSITIONS: &[&str] = crate::transport::claim::FINDING_CAUSAL_DISPOSITIONS;
const SPEND_PHASES: &[&str] = crate::transport::claim::SPEND_PHASES;
// Shared between `publish_review` and `republish_review`, so one wording
// cannot drift from the other.
const TOPOLOGY: Argument = Argument::optional(
    "topology",
    "string",
    "How this delivery was executed: `inline`, `analyst`, `developer:<n>` or free text, per \
     SKILL.md's Direct Work Delegation table. Declared, not observed; read back through \
     `review_read` (`view: transaction`).",
);
const TOPOLOGY_REASON: Argument = Argument::optional(
    "topology_reason",
    "string",
    "Why this topology was declared. Recorded beside `topology`; refused alone.",
);
const EXPECTED_EPOCH: Argument = Argument::optional(
    "expected_epoch",
    "string",
    "The epoch of the newest publication on this branch's pull request as your answer read it, \
     or `none`. Refused `expected-epoch-moved` before any push when it differs. Omit to ask \
     nothing.",
);
// Shared between `publish_review` and `republish_review`, so one wording
// cannot drift from the other.
const INLINE_PAYLOAD: Argument = Argument::optional(
    "inline_payload",
    "boolean",
    "Carry the payload's full diff text in the answer. Defaults to false; `payload_bytes` and \
     `payload_digest` are always present, and the text can make the answer too large for the \
     MCP surface to return. Ask only when you launch the reviewer yourself; \
     `launch_review_input` needs nothing handed to it. Ask at publication time: publishing \
     again mints a new epoch.",
);

// Shared between `publish_review` and `republish_review`, so one wording
// cannot drift from the other.
const WITH_REVIEW: Argument = Argument::optional(
    "with_review",
    "boolean",
    "Add the review tier for `base..head`, its forecast and the first `review_status` read to \
     this answer. Defaults to false; a read that fails is reported under \
     `review_reads_unavailable` rather than refusing a push that already landed.",
);

/// Every operation Estigia exposes.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "claim",
        contract_name: "claim",
        description: "Swear to an issue. Adjudicated by the local store; the earliest live \
                      claim wins. From then on a repository write the gate sees is measured \
                      against it, and a push it does not justify is refused. With `takeover` it displaces an abandoned holder: discovery is read-only and \
                      answers the epoch to name; repeat with `target_operation` to take it over.",
        operation: "claim",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "horizon",
                "string",
                "When this run expects to report, as 2026-08-01T18:00Z.",
            ),
            Argument::optional(
                "state",
                "string",
                "The workflow state the run pointer records. Defaults to in-progress.",
            )
            .of(crate::config::STATES),
            Argument::optional(
                "transition_to",
                "string",
                "Move the issue to this state in the same call, after the claim is \
                 adjudicated. Omit to claim without moving.",
            )
            .of(crate::config::STATES),
            Argument::optional(
                "takeover",
                "boolean",
                "Displace an abandoned holder instead of swearing a new claim. Discovery \
                 is read-only and answers the epoch to name; repeat with `target_operation`.",
            ),
            Argument::optional(
                "target_operation",
                "string",
                "The holder's operation epoch, exactly as the takeover discovery call reported \
                 it. Read only with `takeover`.",
            ),
            Argument::optional(
                "force",
                "boolean",
                "Take a claim that is still live. Needs `reason_file`; with `takeover` only.",
            ),
            Argument::optional(
                "reason_file",
                "string",
                "Path to a file stating why a live claim is being taken. Required with `force`.",
            ),
        ],
        effect: PointerEffect::Swear,
        writes: true,
    },
    Tool {
        name: "verify_claim",
        contract_name: "verify_claim",
        description: "Ask the tracker whether this run is still the live holder. A failed check \
                      is a stop; a failed read is nothing — and nothing is never clearance.",
        operation: "verify-claim",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "expect_state",
                "string",
                "The state this run believes the issue is in.",
            )
            .of(crate::config::STATES),
            Argument::optional(
                "allow_closed_by_pr",
                "integer",
                "Optional pin of the PR that closed this issue after this run published it.",
            )
            .counting_from(1),
        ],
        effect: PointerEffect::Renew,
        writes: false,
    },
    Tool {
        name: "heartbeat",
        contract_name: "heartbeat",
        description: "Renew the claim and record activity of the named kind, default heartbeat. \
                      Renewal first, post second: it refuses to post when the renewal says stop.",
        operation: "heartbeat",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "expect_state",
                "string",
                "The state this run believes the issue is in.",
            )
            .of(crate::config::STATES),
            Argument::required("body_file", "string", "Path to the progress note."),
            Argument::optional(
                "kind",
                "string",
                "The activity kind to record: heartbeat, progress, or blocked. Defaults to \
                 heartbeat.",
            )
            .of(crate::transport::markers::RENEWAL_KINDS),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "transition",
        contract_name: "transition",
        description: "Move the workflow state. Projects it onto every project the issue belongs \
                      to, swaps the label, then reads both back and repairs a disagreement.",
        operation: "transition",
        arguments: &[
            ISSUE,
            Argument::required("to", "string", "The state to move to.").of(crate::config::STATES),
            // Not passed to the transport — `transition` does not take it —
            // and required anyway: without it the run pointer cannot follow
            // the move, and every later `verify_claim` measures against a
            // state the issue has already left. See `POINTER_ONLY`.
            RUN_ID,
            Argument::optional(
                "from",
                "string",
                "The state being left. Omitted, every stale state label is removed. Naming \
                 the state being entered removes nothing.",
            )
            .of(crate::config::STATES),
        ],
        effect: PointerEffect::Moved,
        writes: true,
    },
    Tool {
        name: "comment",
        contract_name: "comment",
        description: "Record evidence on the issue. Generic text can never become a control \
                      event: markers in it are escaped.",
        operation: "comment",
        arguments: &[
            ISSUE,
            Argument::required("body_file", "string", "Path to the comment body."),
            Argument::optional("run_id", "string", "Attribution. Pairs with kind."),
            Argument::optional("kind", "string", "note, blocker, or diagnosis.")
                .of(crate::config::COMMENT_KINDS),
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "blocked_until",
        contract_name: "blocked_until",
        description: "Record, or clear, the clauses that discharge a blocked issue. A clause \
                      holds when the local store puts `until_issue` in `until_state`. Local to \
                      the store. Needs no claim; the issue must be held in `blocked`.",
        operation: "blocked-until",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::optional(
                "until_issue",
                "integer",
                "The issue whose state discharges this one. Pairs with `until_state`.",
            )
            .counting_from(1),
            Argument::optional(
                "until_state",
                "string",
                "The state that issue must be in. Pairs with `until_issue`.",
            )
            .of(crate::config::STATES),
            Argument::optional(
                "clear",
                "boolean",
                "Remove every clause recorded for the issue instead of adding one.",
            ),
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "task",
        contract_name: "task",
        description: "Record or revise one row of the claimed issue's ODD task checklist in the \
                      local store: a stable id, its acceptance, the route used and, once done, \
                      the commit identity. Needs the live claim; nothing is written to the \
                      tracker. The answer carries the whole checklist.",
        operation: "task",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "id",
                "string",
                "The task's stable id, such as T1: letters, digits, `.`, `_` or `-`.",
            ),
            Argument::optional(
                "text",
                "string",
                "The task and its acceptance. Required the first time an id is recorded.",
            ),
            Argument::optional(
                "route",
                "string",
                "How the task is carried out. Required the first time an id is recorded.",
            )
            .of(&["inline", "delegated"]),
            Argument::optional(
                "status",
                "string",
                "`done` once the work-unit commit exists, `open` to reopen. Done needs `commit`.",
            )
            .of(&["open", "done"]),
            Argument::optional(
                "commit",
                "string",
                "The work-unit commit identity, lowercase hex.",
            ),
            Argument::optional(
                "expect_state",
                "string",
                "The state this run believes the issue is in. Defaults to in-progress.",
            )
            .of(crate::config::STATES),
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "tasks",
        contract_name: "tasks",
        description: "The ODD task checklist the local store holds for an issue, in the order \
                      the tasks were first recorded. Read-only, and needs no claim: a run \
                      resuming an issue reads it first.",
        operation: "tasks",
        arguments: &[ISSUE],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "release",
        contract_name: "unassign",
        description: "Put the issue down. Discovery is read-only and answers the epoch to name; \
                      repeat with `target_operation` to release it. Converges the `dev:` label, \
                      and puts down this run's isolated checkout when the close is this run's own \
                      delivery and nothing in it is unpublished; otherwise the checkout is kept \
                      and the answer says why.",
        operation: "unassign",
        arguments: &[
            ISSUE,
            RUN_ID,
            // Without this the tool could only ever make the *discovery* call.
            Argument::optional(
                "target_operation",
                "string",
                "The epoch to release, as the discovery call reported it.",
            ),
            Argument::optional(
                "evidence_file",
                "string",
                "Path to the close-evidence comment, posted as a note before the release. \
                 Omit to release without one.",
            ),
            Argument::optional(
                "spend",
                "string",
                "The spend figures to record before the release, as one JSON object with the \
                 fields `record_spend` takes, and the same checks. Omit to release without \
                 recording one.",
            ),
        ],
        effect: PointerEffect::Forget,
        writes: true,
    },
    Tool {
        name: "handoff_review",
        contract_name: "handoff_review",
        description: "Record one exact review request, read it back, then release only the current \
                      ownership epoch while keeping the issue in review. The same publishing or \
                      requesting run is excluded until a distinct exact-receipt verdict resolves it.",
        operation: "handoff-review",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "target_operation",
                "string",
                "The exact current ownership epoch this handoff may release.",
            ),
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::required(
                "blocker",
                "string",
                "The exact condition preventing this run from completing review.",
            ),
            Argument::required(
                "discharger",
                "string",
                "Who or what can discharge the blocker.",
            ),
        ],
        effect: PointerEffect::Forget,
        writes: true,
    },
    Tool {
        name: "record_review_finding",
        contract_name: "review_finding",
        description: "Record one immutable finding against an exact review transaction: its \
                      identity, evidence, impact and class. Only a `severe` finding can produce \
                      a rejection; `warning` and `suggestion` never withdraw the computed \
                      verdict. A severe finding also states `evidence_class` and \
                      `causal_disposition`.",
        operation: "review-finding",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "reviewer",
                "string",
                "The context credited with finding it; a label only.",
            ),
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::required(
                "id",
                "string",
                "The finding's identity within this lineage, derived from the affected \
                 behaviour and location, never from the run or the wording.",
            ),
            Argument::required(
                "class",
                "string",
                "`severe`: a reproducible correctness, security, reliability or contract \
                 defect with concrete impact. `warning`: a bounded risk with no demonstrated \
                 incorrectness. `suggestion`: optional improvement or preference.",
            )
            .of(FINDING_CLASSES),
            Argument::required(
                "evidence",
                "string",
                "What was observed, concretely enough for somebody else to re-run it.",
            ),
            Argument::required("impact", "string", "What it costs if left alone."),
            Argument::optional(
                "parent",
                "string",
                "The identity of the finding in the parent publication that this one \
                 reassesses. Refused when the parent receipt never recorded it.",
            ),
            Argument::optional(
                "origin",
                "string",
                "Why a repair answers for a new defect: `introduced` if it created it, \
                 `exposed` if it made an existing one reachable. Required of a severe finding \
                 naming no parent against a publication that has one.",
            )
            .of(FINDING_ORIGINS),
            Argument::optional(
                "evidence_class",
                "string",
                "How the claim was established: `deterministic` (demonstrated), \
                 `inferential` (follows from what was read) or `insufficient`. Required of a \
                 severe finding.",
            )
            .of(FINDING_EVIDENCE_CLASSES),
            Argument::optional(
                "causal_disposition",
                "string",
                "Who owns this defect on the timeline. Required of a severe finding; \
                 `pre-existing` and `base-only` are recorded but name nothing this change \
                 answers.",
            )
            .of(FINDING_CAUSAL_DISPOSITIONS),
            Argument::optional(
                "location",
                "string",
                "Where this defect lives: `path:line` or `path:start-end`, the path \
                 repository-relative. Required of a severe finding whose `causal_disposition` \
                 is `introduced` or `worsened`. A malformed value is always refused. Where owed, \
                 every line must fall inside a changed hunk; a path touched only by deletion, \
                 a binary edit or a mode change settles by being touched.",
            ),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "escalate_review",
        contract_name: "escalate_review",
        description: "Record that a review transaction's decision passed to a named person, the \
                      exit a spent correction forces. Requires a live review claim, a recorded \
                      receipt, and `correction_spent`; records a **local** `escalated` receipt \
                      naming the person and the reason, with no GitHub comment. It does not \
                      release CI.",
        operation: "escalate-review",
        arguments: &[
            ISSUE,
            RUN_ID,
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::required(
                "escalated_to",
                "string",
                "The person who takes the decision. Refused when empty.",
            ),
            Argument::required(
                "reason",
                "string",
                "Why the decision passed to them. Refused when blank.",
            ),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "record_disposition",
        contract_name: "review_disposition",
        description: "Record an optional disposition batch against an exact review transaction: \
                      `acted-on`, `filed` (with `filed_as`, the issue it became) or `declined`, \
                      each with a reason, for `warning` and `suggestion` findings the receipt \
                      recorded. Nothing owes this record. Refused: an id whose every recording is \
                      `severe` (the computed verdict adjudicates it; one also recorded as \
                      non-severe is answered for that non-severe recording), an id the receipt \
                      never recorded, a duplicate, a blank or filler reason, a `filed` entry \
                      naming no issue. A retry reads back the batch already recorded.",
        operation: "review-disposition",
        arguments: &[
            ISSUE,
            RUN_ID,
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::required(
                "dispositions",
                "string",
                "The whole batch, as a JSON array of `{\"id\": <finding id>, \"disposition\": \
                 <acted-on|filed|declined>, \"reason\": <text>}` objects, with `\"filed_as\": \
                 <issue number>` on every `filed` entry. An empty array is well formed.",
            ),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "launch_review_input",
        contract_name: "launch_review_input",
        description: "Start the reviewer for each slot `name` or `names` requests, each offered \
                      by a `review_status` `collect`, returning before any has answered. The \
                      diff is rebuilt from the `collect`'s receipt; the criteria are the issue's \
                      body. The reviewer is the first installed `claude` or `codex` CLI in a \
                      detached tool-less job under a 15 minute deadline, or the relay on an \
                      OpenCode run with its plugin. Answers `{job, deadline_at}` per slot, or \
                      `{launched: [...]}`, and records nothing. A batch reports a slot that \
                      cannot start. Requires a live review claim. `collect_review_input` \
                      records the answer.",
        operation: "launch-review-input",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::optional(
                "name",
                "string",
                "One slot to launch: the `name` of an input `review_status` offered in a \
                 `collect`. Give `names` instead to launch several.",
            ),
            Argument::optional(
                "names",
                "array",
                "The slots to launch in one call, as an array of `name` values, each with its \
                 own claim check. Give `name` or `names`, never both.",
            ),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "collect_review_input",
        contract_name: "collect_review_input",
        description: "Record what the reviewer `launch_review_input` started answered. Give \
                      `names` for several slots or `name` for one; only the run that launched a \
                      job collects it, and `review_status` must still be a `collect` offering \
                      each slot. A running job is refused unless `wait_seconds` is given; a batch \
                      reports a running slot rather than stopping. A lens's findings are checked \
                      as `record_review_finding` checks them before any is written, so one bad \
                      finding rejects the answer and writes nothing; then the lens report is \
                      recorded. A `refutation-batch` answer must name each listed finding once \
                      and nothing else. A `review-validator` `fail` or `unavailable` inspection \
                      is recorded, not rejected. A second call for a recorded job writes \
                      nothing. Requires a live review claim.",
        operation: "collect-review-input",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::optional(
                "name",
                "string",
                "One slot to collect: the `name` given to `launch_review_input`. Give `names` \
                 instead to collect several.",
            ),
            Argument::optional(
                "names",
                "array",
                "The slots to collect in one call, as an array of `name` values, each with its \
                 own claim check. Give `name` or `names`, never both.",
            ),
            Argument::optional(
                "wait_seconds",
                "integer",
                "Seconds to wait for a running job before refusing `capture-running`; omitted \
                 checks once. Capped at 900.",
            )
            .counting_from(1),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "record_computed_verdict",
        contract_name: "review_computed_verdict",
        description: "Compute the verdict an exact review transaction's finding ledger supports \
                      and record it once; the caller supplies the receipt, never the outcome. \
                      `approved` needs every licensed lens reported, no severe finding open or \
                      escalated, and the publication lane cleared; `rejected` rests on a severe \
                      finding that stands; `escalated` on one the ledger cannot settle. An \
                      `incomplete` computation is refused. Recording again reads back the marker.",
        operation: "review-computed-verdict",
        arguments: &[
            ISSUE,
            RUN_ID,
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::optional(
                "worktree",
                "string",
                "The checkout `base..head` is read from.",
            ),
            Argument::optional(
                "never_ran_evidence_file",
                "string",
                "Path to your own local verification that the publication lane's run never \
                 executed a step, read only for an `approved` computation over a lane answering \
                 `publication-lane-never-ran`. Stored, not verified.",
            ),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "record_spend",
        contract_name: "record_spend",
        description: "Record one measurement of what a phase cost against a review transaction: \
                      bytes handed in, tokens, seconds and agents. A round's cost stays \
                      recordable after a later republish supersedes it. Recorded, not verified.",
        operation: "spend",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "phase",
                "string",
                "Which phase spent this.",
            )
            .of(SPEND_PHASES),
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::required(
                "conditions",
                "string",
                "What this measurement varied, such as \"sealed\" or \"payload only\". Part of \
                 the record's identity: the same receipt and phase under different conditions \
                 are two measurements.",
            ),
            Argument::required(
                "input",
                "integer",
                "Bytes the phase was handed: brief plus payload.",
            )
            .counting_from(1),
            Argument::required("tokens", "integer", "Tokens the phase consumed.").counting_from(1),
            Argument::required("seconds", "integer", "Wall-clock seconds the phase ran.")
                .counting_from(1),
            Argument::required(
                "contexts",
                "integer",
                "How many agents produced this figure; a four-lens review is `contexts: 4`.",
            )
            .counting_from(1),
            Argument::optional("note", "string", "What produced it, when worth stating."),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "review_status",
        contract_name: "review_status",
        description: "The one next step of this issue's review, so the route is followed rather \
                      than chosen. `next.kind` is `execute` (run `next.operation` with \
                      `next.arguments`, adding your run id, an operation id and the tool's own \
                      non-receipt arguments), `collect` (obtain each of `next.inputs` with its \
                      `capture` operation, then ask again) or `stop` (`next.reason_code` and \
                      `next.continuation`). The answer also carries the `receipt` it was read \
                      against and a `forecast` horizon; every command it names carries that \
                      receipt, or `expected_epoch`, so a later publication makes it refuse as \
                      stale. An unreadable store, pull request or lane is an `unavailable` \
                      refusal, never a state. Read-only, no claim.",
        operation: "review-status",
        arguments: &[ISSUE],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "list_state",
        contract_name: "list_state",
        description: "The requester-aware unassigned queue for one state, partitioned by domain, \
                      or the dedup key each open issue recorded when `identities` is set. State \
                      and holder come from the local store, never a label or assignee; an open \
                      issue the store never admitted is under `unadmitted`. Excludes unresolved \
                      review handoffs from their publishing or requesting run.",
        operation: "list-state",
        arguments: &[
            Argument::optional("state", "string", "The workflow state, required unless \
                              `identities` is set.").of(crate::config::STATES),
            RUN_ID,
            Argument::optional(
                "identities",
                "boolean",
                "Answer each open issue's recorded dedup key instead of one state's queue.",
            ),
            Argument::optional(
                "limit",
                "integer",
                concat!(
                    "How many issues to read at most. Defaults to 200, a ceiling and not a ",
                    "total: the answer's `at_limit` says when the queue was cut. A cut keeps ",
                    "the oldest matching issues."
                ),
            )
            .counting_from(1),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "ensure_states",
        contract_name: "ensure_states",
        description: "Create the workflow's labels. Idempotent; run before the first write in an \
                      unfamiliar project.",
        operation: "ensure-states",
        arguments: &[],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "flush",
        contract_name: "flush",
        description: "Send the GitHub writes queued while the git operation window was closed, \
                      oldest first, stopping at the first one it cannot read back. Refuses while \
                      the window is closed. With `read_only`, lists the queue and sends nothing.",
        operation: "flush",
        arguments: &[Argument::optional(
            "read_only",
            "boolean",
            "List the queue and send nothing.",
        )],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "create",
        contract_name: "create",
        description: "File a finding. Creates every label before attaching it, then projects the \
                      initial state onto every project the issue belongs to.",
        operation: "create",
        arguments: &[
            Argument::required(
                "identity",
                "string",
                "The stable dedup key. Recorded in the local store, not the title or the body.",
            ),
            Argument::required(
                "title",
                "string",
                "The issue title — a short name, not a sentence; see domain-composition.md's \
                 title row.",
            ),
            Argument::required("body_file", "string", "Path to the filled template."),
            Argument::required("priority", "string", "As scale:value."),
            Argument::required("domain", "string", "The routed domain."),
            Argument::required(
                "runtime",
                "string",
                "The runtime filing it \u{2014} a label-safe runtime word: lowercase `a-z0-9` \
                 groups joined by single `-`.",
            ),
            RUN_ID,
            Argument::optional(
                "state",
                "string",
                "The state to file it in. Defaults to ready; blocked waits on a condition.",
            )
            .of(crate::config::CREATED_STATES),
            Argument::optional(
                "blocked_until",
                "string",
                "Clauses that discharge a blocked issue, `<issue>=<state>` joined by `+`. \
                 Recorded in the local store; only with state blocked.",
            ),
            Argument::optional(
                "epoch",
                "string",
                "The publication epoch the finding was recorded against. Renders the issue's \
                 marker as `review` instead of `analysis`, and is what lets a filed issue name \
                 the review that found it.",
            ),
            Argument::optional(
                "findings",
                "string",
                "One or more finding identities, `+`-separated, this issue files. Refused \
                 without `epoch` — a finding identity means nothing without the receipt it was \
                 recorded against.",
            ),
            FILING_REPO,
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "publish_review",
        contract_name: "publish_review",
        description: "Push and publish the shared review target. Head and base are read back \
                      until the remote agrees. The answer carries the identity fields, the \
                      digests, the lane's state word and `next`; `review_read` (`view: \
                      transaction`) answers the rest.",
        operation: "publish-review",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("branch", "string", "The branch to publish."),
            Argument::required("base", "string", "The base branch."),
            Argument::required(
                "pr_title",
                "string",
                "The pull request title — the same short-name rule as issue title; see \
                 domain-composition.md's title row.",
            ),
            Argument::required("pr_body_file", "string", "Path to the pull request body."),
            Argument::optional("worktree", "string", "The isolated checkout."),
            Argument::optional(
                "expect_state",
                "string",
                concat!(
                    "The state this run believes the issue is in. Defaults to in-progress; a ",
                    "publish after a review sent work back is made from review."
                ),
            )
            .of(crate::config::STATES),
            INLINE_PAYLOAD,
            TOPOLOGY,
            TOPOLOGY_REASON,
            EXPECTED_EPOCH,
            WITH_REVIEW,
        ],
        effect: PointerEffect::Published,
        writes: true,
    },
    Tool {
        name: "republish_review",
        contract_name: "republish_review",
        description: "Republish a branch whose history was rewritten, rebased or amended. Leases \
                      against the head the last publication recorded and refuses when the remote \
                      moved. Use publish_review for every ordinary publication; this one \
                      destroys remote history.",
        operation: "republish-review",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("branch", "string", "The branch to republish."),
            Argument::required("base", "string", "The base branch."),
            Argument::required(
                "pr_title",
                "string",
                "The pull request title — the same short-name rule as issue title; see \
                 domain-composition.md's title row.",
            ),
            Argument::required("pr_body_file", "string", "Path to the pull request body."),
            Argument::optional("worktree", "string", "The isolated checkout."),
            Argument::optional(
                "expect_state",
                "string",
                concat!(
                    "The state this run believes the issue is in. Defaults to in-progress; a ",
                    "republish after review sent work back is made from review."
                ),
            )
            .of(crate::config::STATES),
            INLINE_PAYLOAD,
            TOPOLOGY,
            TOPOLOGY_REASON,
            EXPECTED_EPOCH,
            WITH_REVIEW,
        ],
        effect: PointerEffect::Published,
        writes: true,
    },
    Tool {
        name: "release_ci",
        contract_name: "release_ci",
        description: "Release one exact reviewed draft target to CI. Re-verifies the live review claim, the newest review transaction, the draft PR identity and a coherent clean target before marking ready, then confirms by readback. A `warning` or `suggestion` never blocks it.",
        operation: "release-ci",
        arguments: &[
            ISSUE,
            RUN_ID,
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::optional("worktree", "string", "The isolated checkout."),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "start_branch",
        contract_name: "branch + worktree",
        description: "Create the isolated checkout for this issue, verifying the claim first.",
        operation: "start-branch",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("branch", "string", "The branch to create."),
            Argument::required("base", "string", "The base branch to start from."),
            Argument::optional(
                "expect_state",
                "string",
                "The state this run believes the issue is in. Defaults to in-progress.",
            )
            .of(crate::config::STATES),
            Argument::optional(
                "worktree_root",
                "string",
                "Where the checkout is made. A prefix, not a container: `<branch>~<run-id>` is \
                 appended to the path given, so the checkout lands as a sibling of it, keeping \
                 two checkouts from sharing one directory. Defaults to the configured \
                 location.",
            ),
        ],
        effect: PointerEffect::Isolated,
        writes: true,
    },
    Tool {
        name: "judge_workspaces",
        contract_name: "judge_workspaces",
        description: "Mint one bare working directory per Judgment Day judge for a panel over an \
                      exact review transaction, and refuse rather than return a short list. The \
                      directories are siblings of the claimed checkout, distinct, not nested and \
                      outside it, with no git checkout of the reviewed head. It proves nothing \
                      about where a judge wrote.",
        operation: "judge-workspaces",
        arguments: &[
            ISSUE,
            RUN_ID,
            RECEIPT_EPOCH,
            RECEIPT_PR,
            RECEIPT_HEAD,
            RECEIPT_BASE,
            RECEIPT_DIGEST,
            Argument::required(
                "count",
                "integer",
                "How many judges the Judgment Day panel has, and so how many directories it needs. At most 64.",
            )
            .counting_from(1),
            Argument::optional(
                "worktree_root",
                "string",
                "Where the panel root is made, as for `start_branch`. Defaults to the configured \
                 location.",
            ),
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "assist",
        contract_name: "assisted checkout",
        description: "Take an issue the team coordinates, where the `Workflow` row is `assisted`: \
                      creates the isolated checkout and records a local claim. Writes are \
                      admitted only inside that checkout, on that branch; delivery is refused. \
                      Writes nothing to the tracker, and refuses anywhere else.",
        operation: "assist",
        arguments: &[
            ISSUE_ANY,
            RUN_ID,
            Argument::required("branch", "string", "The branch to create."),
            Argument::required("base", "string", "The base branch to start from."),
            Argument::optional(
                "worktree_root",
                "string",
                "Where the checkout is made, as for `start_branch`. Defaults to the configured \
                 location.",
            ),
        ],
        effect: PointerEffect::Assisted,
        writes: true,
    },
    Tool {
        name: "draft_pr",
        contract_name: "draft pull request",
        description: "Publish an assisted run's branch, where the `Workflow` row is `assisted`, as \
                      a draft pull request: pushes fast-forward, opens a draft with the title and \
                      body as given, and reads it back. Refuses a closing keyword in the title, \
                      body or commits, and a branch whose open pull request is ready for review; \
                      an open draft is reused. Writes nothing to the tracker. The branch and \
                      checkout are the ones `assist` recorded.",
        operation: "draft-pr",
        arguments: &[
            RUN_ID,
            Argument::required("title", "string", "The pull request title."),
            Argument::required(
                "body_file",
                "string",
                "Path to the pull request body, written exactly as it is.",
            ),
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "check_closing_keywords",
        contract_name: "check closing keywords",
        description: "Scan the branch and its commit messages for keywords that would close the \
                      issue on merge. Read-only; run it before delivery.",
        operation: "check-closing-keywords",
        arguments: &[
            ISSUE,
            Argument::optional("base", "string", "The base ref, so commits can be scanned."),
            Argument::optional("branch", "string", "The branch to scan."),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "review_tier",
        contract_name: "review_tier",
        description: "The review tier for one base/head pair, derived from the change alone: \
                      `high` when a hot-path token, an adjacent `service`/`token` path pair, a \
                      shell-source extension or an execute-bit flip is touched; `low` when at \
                      least one path changed and every authored byte is proven passive prose \
                      and nothing configures anything; `medium` otherwise, including a range \
                      that changed no path (a `nothing_measured` reason, never `low`). It reads \
                      a boundary, not a size. `lenses` is a fixed allocation per tier: `medium` \
                      answers one lens, the `focus` argument's (default `reliability`); `high` \
                      answers four lenses, `review-risk`, `review-readback`, \
                      `review-readability` and `review-reliability`; `low` answers zero. \
                      `lens_roles` names the reviewers the count licenses; no judge count is \
                      carried. Reads the whole `base..head`; \
                      a repair round names `parent_head` to read its narrower range when that is \
                      recoverable. Read-only, no claim, decides nothing itself.",
        operation: "review-tier",
        arguments: &[
            Argument::required("base", "string", "The base commit-ish."),
            Argument::required("head", "string", "The head commit-ish."),
            Argument::optional(
                "parent_head",
                "string",
                "The parent publication's head commit-ish, for a repair round. When it is an \
                 ancestor of `head`, the base did not move and `parent_head..head` touches a \
                 path, that narrower range is read; otherwise the whole `base..head`, with the \
                 reason.",
            ),
            Argument::optional(
                "clearing_finding",
                "string",
                "The class of the review_finding that cleared the republish-over-an-approved-\
                 receipt gate on the parent receipt. Read only with `parent_head`.",
            )
            .of(FINDING_CLASSES),
            Argument::optional("worktree", "string", "The isolated checkout."),
            Argument::optional(
                "focus",
                "string",
                "The lens a `medium` change runs: risk, resilience, readability or reliability. \
                 Defaults to reliability; `high` runs all four regardless.",
            )
            .of(REVIEW_FOCUSES),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "review_read",
        contract_name: "review_read",
        description: "One read of a review transaction, chosen by `view`. `transaction` is the \
                      recorded publication for one epoch: topology, repair lineage, the lane's \
                      note and workflows, and the autoclose verdict. `findings` is every finding \
                      recorded against the receipt, `verdict` the verdict the ledger computes, \
                      `membership` whether a candidate file is a genuine subset of the payload, \
                      `inspect` what a correction changed over its parent, `spend` every \
                      measurement of one phase. Read-only, no claim.",
        operation: "review-read",
        arguments: &[
            ISSUE,
            Argument::required(
                "view",
                "string",
                "Which read: transaction, findings, verdict, membership, inspect or spend.",
            )
            .of(&[
                "transaction",
                "findings",
                "verdict",
                "membership",
                "inspect",
                "spend",
            ]),
            Argument::optional(
                "epoch",
                "string",
                "The publication epoch to answer. Omit to answer the latest publication. \
                 Required for `inspect`.",
            ),
            Argument::optional(
                "id",
                "string",
                "`findings` only: answer only the finding(s) carrying this identity.",
            ),
            Argument::optional(
                "with_text",
                "boolean",
                "`findings` only: include each finding's `evidence` and `impact` verbatim.",
            ),
            Argument::optional(
                "candidate",
                "string",
                "`membership` only: path to the candidate reviewer-facing text on disk.",
            ),
            Argument::optional(
                "request",
                "string",
                "`inspect` only: one of name-status, numstat, stat, patch or object.",
            ),
            Argument::optional(
                "path",
                "string",
                "`inspect` only: a path the correction changed, required for `object`.",
            ),
            Argument::optional(
                "side",
                "string",
                "`inspect` only: `parent` or `candidate`, required for `object`.",
            ),
            Argument::optional(
                "pr",
                "integer",
                "`verdict` only: the pull request number.",
            )
            .counting_from(1),
            Argument::optional(
                "head",
                "string",
                "`verdict` only: the full published head SHA.",
            ),
            Argument::optional(
                "base",
                "string",
                "`verdict` only: the full published base SHA.",
            ),
            Argument::optional(
                "digest",
                "string",
                "`verdict` only: the complete-target manifest digest.",
            ),
            Argument::optional(
                "reproduce",
                "boolean",
                "`verdict` only: run the generated-file commands the base declares in \
                 `estigia.generated.toml`. This executes repository code and may use the \
                 network. Omitted, `generated_files` lists the touched paths as `not-run`.",
            ),
            Argument::optional(
                "phase",
                "string",
                "`spend` only: which phase to answer for.",
            )
            .of(SPEND_PHASES),
            Argument::optional("worktree", "string", "The isolated checkout."),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "target",
        contract_name: "target",
        description: "The delivery target: base, head, digest, path count. Read-only. \
                      When `recorded_base` is given, answers how the base has moved instead: \
                      none, compatible, overlapping, conflicting or unknown.",
        operation: "target",
        arguments: &[
            Argument::required("base", "string", "The base SHA."),
            Argument::optional(
                "recorded_base",
                "string",
                "The base SHA the review was bound to. Given, classify base movement instead of \
                 answering the target.",
            ),
            Argument::optional("worktree", "string", "The isolated checkout."),
            Argument::optional("native_start", "string", "A reviewer's target to compare."),
            Argument::optional(
                "manifest",
                "boolean",
                "Add the path/mode/blob manifest: to save for native_start, or to read a \
                 difference path by path.",
            ),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
];

/// Operations the contract requires that Estigia deliberately does not expose
/// as a tool of their own.
///
/// A missing tool with no reason is a gap, and a missing tool with a reason is
/// a boundary. `reclaim` is here because `claim` performs it through
/// `takeover`, so the capability survives without a second name; a call to
/// `reclaim` refuses with this reason. The list is a declaration for the seam
/// test, not a second `tools/list`: it is not advertised, so a client cannot
/// see a name no call performs.
pub const NOT_EXPOSED: &[(&str, &str)] = &[(
    "reclaim",
    "exposed as `claim` with `takeover: true`, not as a tool of its own; the binding carries \
     the operation at *reclaim* in `bindings/github.md`",
)];

/// The reason a name is deliberately not exposed, when it is one.
pub fn unexposed_reason(name: &str) -> Option<&'static str> {
    NOT_EXPOSED
        .iter()
        .find(|(operation, _)| *operation == name)
        .map(|(_, reason)| *reason)
}

/// Arguments that are Estigia's bookkeeping, not transport flags.
///
/// Passing one through would make the transport reject the whole call for an
/// unknown option; they exist because the run pointer has to follow what
/// became true, and the transport has no reason to care whose run asked.
/// `create`'s `repo` is the other kind: this server reads it to choose the
/// repository the transport is pointed at, so the transport never sees it.
pub const POINTER_ONLY: &[(&str, &str)] = &[
    ("create", "repo"),
    ("claim", "state"),
    ("transition", "run_id"),
    ("draft_pr", "run_id"),
];

/// Whether an argument is bookkeeping rather than a flag.
pub fn is_pointer_only(tool: &str, argument: &str) -> bool {
    POINTER_ONLY
        .iter()
        .any(|(name, field)| *name == tool && *field == argument)
}

/// Transport operations Estigia deliberately does not run.
///
/// Paired with `NOT_EXPOSED`: that one names required operations with no tool,
/// this names executable operations Estigia never calls. A row is added here
/// when an operation moves off the tool surface rather than onto it — the tool
/// server no longer advertises it, and the transport still performs it for the
/// release flow that runs it by hand.
pub const NOT_RUN: &[(&str, &str)] = &[
    (
        "config",
        "the operator configuration is Estigia's own — `estigia config list` reads the same table, and a second reader of it would be a second answer to what the settings are",
    ),
    (
        "changelog-notes",
        "the release flow reads a version's changelog entry by hand before tagging, and the tool surface no longer carries a name for it",
    ),
];

/// The tools that write to the tracker or the repository host on a run's behalf.
///
/// Refused outright where `Workflow` is `assisted`. A tool that is in no list
/// here either reads or writes only a local file, and a test classifies every
/// entry of [`TOOLS`] so a new tool is not allowed to be unclassified.
pub const TRACKER_WRITERS: &[&str] = &[
    "claim",
    "release",
    "heartbeat",
    "transition",
    "comment",
    "create",
    "ensure_states",
    "flush",
    "handoff_review",
    "record_review_finding",
    "record_disposition",
    "launch_review_input",
    "collect_review_input",
    "record_computed_verdict",
    "escalate_review",
    "record_spend",
    "publish_review",
    "republish_review",
    "release_ci",
    "start_branch",
];

/// The tools that exist only where `Workflow` is `assisted`, and are refused
/// everywhere else.
pub const ASSISTED_ONLY: &[&str] = &["assist", "draft_pr"];

/// Whether a tool writes to the tracker or the repository host.
pub fn writes_the_tracker(name: &str) -> bool {
    TRACKER_WRITERS.contains(&name)
}

/// Whether a tool is offered only under `Workflow: assisted`.
pub fn is_assisted_only(name: &str) -> bool {
    ASSISTED_ONLY.contains(&name)
}

/// The tool a name refers to.
pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

impl Tool {
    /// The MCP schema an agent reads to call this tool.
    pub fn schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for argument in self.arguments {
            properties.insert(argument.name.to_owned(), {
                let mut property = json!({
                    "type": argument.json_type,
                    "description": argument.description,
                });
                // Published, not merely enforced: an agent that cannot see
                // the vocabulary guesses at it, and every wrong guess costs
                // a whole call.
                if argument.json_type == "array" {
                    property["items"] = json!({"type": "string"});
                }
                if let Some(choices) = argument.choices {
                    property["enum"] = json!(choices);
                }
                if let Some(least) = argument.least {
                    property["minimum"] = json!(least);
                }
                property
            });
            if argument.required {
                required.push(Value::String(argument.name.to_owned()));
            }
        }
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required,
            },
            "annotations": {
                "readOnlyHint": !self.writes,
            },
        })
    }
}
