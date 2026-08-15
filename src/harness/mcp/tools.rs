//! The workflow operations, as a table.
//!
//! One list, walked by the schema generator, the dispatcher, and the seam test
//! that checks it against the contract. The alternative — a `match` in the
//! dispatcher and a hand-written schema beside it — is two places that drift,
//! and the drift is silent because a tool with a wrong schema is simply never
//! called correctly.
//!
//! Every tool here maps to an operation `SKILL.md` requires the binding to
//! provide. `every_required_operation_is_exposed_or_declared` holds the two
//! sides together.

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
    /// `"boolean"` is not a third way of spelling a value: it means the flag
    /// carries none, and the transport declares it `action="store_true"`. The
    /// two have to agree, and
    /// `a_boolean_argument_is_one_the_transport_takes_without_a_value` is what
    /// says so — the flag names were crossed against that parser and the
    /// **shapes** were not, so `--fix true` went out to a flag that takes
    /// nothing and argparse rejected the call.
    pub json_type: &'static str,
    /// The only values this argument accepts, when the transport constrains it.
    ///
    /// Six flags in that parser carry `choices=STATES` and the schema published
    /// none of them: an agent was told `to` is a string, given one sentence of
    /// prose, and left to guess between `in-progress`, `in progress` and
    /// `doing`. A wrong guess is argparse refusing the whole call — the same
    /// cost the `boolean` note above records, on the tool that moves work
    /// through the workflow.
    ///
    /// [`crate::config::STATES`] rather than a list written here: that one is
    /// already crossed against the binding's own sentence, and a second copy is
    /// the thing this crate keeps finding disagreeing with itself.
    pub choices: Option<&'static [&'static str]>,
    /// The smallest value this argument takes, when it has one.
    ///
    /// Both integers here are counts of something that starts at one: an issue
    /// number and a page limit. Neither was bounded, and the value is rendered
    /// straight into the transport's argv — so `issue: -5` arrived at `gh` as a
    /// **flag**:
    ///
    /// ```text
    /// gh issue view failed (1): unknown shorthand flag: '5' in -5
    /// ```
    ///
    /// Which is the failure this file's own notes describe twice already, in
    /// those words: *the agent read the failure as a configuration defect*, and
    /// *the agent that believed the schema gets its error from two processes
    /// away*. A wrong argument reported as a broken transport is the wrong
    /// cause, and the one an agent retries against.
    ///
    /// Published as `minimum` as well as enforced, for the reason [`choices`]
    /// gives: enforcing a rule the schema does not carry makes the schema a
    /// description.
    ///
    /// [`choices`]: Argument::choices
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

    /// The same, refusing anything below `least`.
    const fn counting_from(mut self, least: i64) -> Self {
        self.least = Some(least);
        self
    }

    /// The same argument, constrained to the values the transport accepts.
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
/// The pointer is not authority — see [`super::super::session`] — but it has to
/// follow reality, and the moment reality changes is a successful write.
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
    ///
    /// Renews like the rest, and keeps the bytes. `publish-review`'s own answer
    /// ends with *bind review and CI to these SHAs*, and until this was written
    /// down nothing on this side kept them — so the boundary that delivers on
    /// that verdict had no way to notice the head had moved under it.
    Published,
    /// This run was given an isolated checkout, and the work happens there.
    ///
    /// The path comes from the transport's **answer**, not from the arguments:
    /// `worktree_root` is a template the transport expands per run and per
    /// branch, so the directory an agent asked for is not the directory it got.
    Isolated,
}

/// One workflow operation, exposed as an MCP tool.
#[derive(Debug, Clone, Copy)]
pub struct Tool {
    /// The MCP tool name, as the agent calls it.
    pub name: &'static str,
    /// The name the binding's operations table gives this row.
    ///
    /// Not ours to choose: `every_tool_names_a_row_the_binding_actually_has`
    /// checks it against the table, so a tool cannot claim to implement an
    /// operation under a name the binding never used. Some rows are prose
    /// (`branch + worktree`) rather than the backticked identifiers the
    /// contract's MUST-map line lists, and that difference is upstream's.
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
    /// Read-only tools are safe to retry and safe to call speculatively. The
    /// distinction is exposed in the schema so an agent can tell them apart
    /// without reading this file.
    pub writes: bool,
}

const RUN_ID: Argument = Argument::required(
    "run_id",
    "string",
    "This run's id, exactly as SessionStart reported it.",
);
const ISSUE: Argument =
    Argument::required("issue", "integer", "The issue number.").counting_from(1);
const REVIEW_OUTCOMES: &[&str] = &["accepted", "rejected"];

/// Every operation Estigia exposes.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "claim",
        contract_name: "claim",
        // This used to promise the gate measures *all* of a run's repository
        // writes, and this reader acts on what it is told. The gate sees one
        // made through a tool its matcher covers, in an agent whose calls
        // Estigia can gate — and eight of the eleven adapters get the contract
        // and no gate. What is unconditional is the claim itself, and what the
        // push guard does with it.
        description: "Swear to an issue. Adjudicated by the tracker timeline — the earliest live \
                      claim wins. From this point a repository write the gate sees is measured \
                      against this claim, and a push from this checkout is refused unless the \
                      claim justifies it.",
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
                "The workflow state to verify against afterwards. Defaults to in-progress.",
            )
            .of(crate::config::STATES),
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
        ],
        effect: PointerEffect::Renew,
        writes: false,
    },
    Tool {
        name: "heartbeat",
        contract_name: "heartbeat",
        description: "Renew the claim and post progress. Renewal first, post second: it refuses \
                      to post when the renewal says stop.",
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
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "transition",
        contract_name: "transition",
        description: "Move the workflow state. Mirrors the board first, swaps the label in one \
                      call, then reads both back and repairs a board that disagrees.",
        operation: "transition",
        arguments: &[
            ISSUE,
            Argument::required("to", "string", "The state to move to.").of(crate::config::STATES),
            // Not passed to the transport — `transition` does not take it —
            // and **required anyway**. Without it the run pointer cannot follow
            // the move, so the run goes on believing the old state and every
            // later `verify_claim --expect-state` measures against a state the
            // issue has left: every write after a transition is then refused
            // with `unexpected-state`, and nothing says why. See `POINTER_ONLY`.
            Argument::required(
                "run_id",
                "string",
                "This run's id, so the state it believes the issue is in stays current.",
            ),
            Argument::optional(
                "from",
                "string",
                "The state being left. Omitting it removes whatever stale state labels are found.",
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
        name: "reclaim",
        contract_name: "reclaim",
        description: "Displace an abandoned holder. Target discovery is read-only and answers \
                      the epoch to name; repeat with `target_operation` to take it over. The \
                      write binds target, evidence and privilege, then proves the projections.",
        operation: "reclaim",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("horizon", "string", "When this run expects to report."),
            Argument::optional(
                "target_operation",
                // A **string**, as `release` one entry below already says. An
                // operation epoch is the holder's 32 hex characters, or
                // `legacy-<id>` for a claim made before there were any — never
                // a number. Declared an integer here, the schema told an agent
                // to send one, and a number can never equal the epoch: the
                // second call of every `reclaim` answered
                // `target-operation-mismatch` for as long as the agent believed
                // what this server published.
                "string",
                "The holder's operation epoch, exactly as the discovery call reported it.",
            ),
            // The forced takeover, which the binding documents as
            // `[--force --reason-file <f>]` and this tool published half of.
            //
            // `force` was here and `reason_file` was not, and the transport
            // refuses a forced reclaim without one: `force-reason-required`,
            // raised before anything is read. So the only path an agent has
            // could ask for the takeover and could not satisfy the answer — a
            // documented operation, unreachable, with a refusal it cannot
            // discharge. That is `start_branch --repo-name` again, and nothing
            // crossed the binding's table against these arguments until
            // `every_flag_the_binding_documents_is_one_a_tool_can_send`.
            Argument::optional(
                "force",
                "boolean",
                "Take a claim that is still live. Requires `reason_file`.",
            ),
            Argument::optional(
                "reason_file",
                "string",
                "Path to a file stating why a live claim is being taken. Required with `force`, \
                 and its text is bound to the takeover as evidence.",
            ),
        ],
        effect: PointerEffect::Swear,
        writes: true,
    },
    Tool {
        name: "release",
        contract_name: "unassign",
        description: "Put the issue down. Discovery is read-only and answers the epoch to name; \
                      repeat with `target_operation` to release it. Releases only this runtime's \
                      projection; retries cannot release a later acquisition.",
        operation: "unassign",
        arguments: &[
            ISSUE,
            RUN_ID,
            // Without this the tool could only ever make the *discovery* call:
            // the transport answers `write_performed: false` and says to repeat
            // naming the epoch, and there was no way to name it. `reclaim` next
            // to it has had the argument all along.
            Argument::optional(
                "target_operation",
                "string",
                "The epoch to release, as the discovery call reported it.",
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
            Argument::required("epoch", "string", "The publication epoch."),
            Argument::required("pr", "integer", "The pull request number.").counting_from(1),
            Argument::required("head", "string", "The full published head SHA."),
            Argument::required("base", "string", "The full published base SHA."),
            Argument::required("digest", "string", "The complete-target manifest digest."),
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
        name: "record_review_verdict",
        contract_name: "review_verdict",
        description: "Record an immutable verdict for the latest complete publication receipt. \
                      Requires a live review claim and refuses to credit the publishing or \
                      requesting run; either outcome resolves a handoff, but only `accepted` \
                      releases delivery.",
        operation: "review-verdict",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required(
                "reviewer",
                "string",
                "The context credited with the review. After a handoff this is the recording run \
                 itself; a run that acquired a reviewer without releasing the claim names that \
                 reviewer instead. Never the run that published.",
            ),
            Argument::required("epoch", "string", "The publication epoch."),
            Argument::required("pr", "integer", "The pull request number.").counting_from(1),
            Argument::required("head", "string", "The full published head SHA."),
            Argument::required("base", "string", "The full published base SHA."),
            Argument::required("digest", "string", "The complete-target manifest digest."),
            Argument::required("outcome", "string", "The review verdict.").of(REVIEW_OUTCOMES),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    Tool {
        name: "list_state",
        contract_name: "list_state",
        description: "The requester-aware unassigned queue for one state, partitioned by domain. \
                      It excludes unresolved review handoffs from their publishing/requesting run, \
                      fails closed on candidate timeline reads, and never ranks across partitions.",
        operation: "list-state",
        arguments: &[
            Argument::required("state", "string", "The workflow state.").of(crate::config::STATES),
            RUN_ID,
            // Published because it is a **ceiling**, not a preference. The
            // transport passes `--limit` to `gh issue list` and defaults it to
            // 200, and the answer carries `count: len(data)` — the number
            // returned, not the number there are. With no property here, an
            // agent asking what is in `ready` on a busy project read two hundred
            // issues as the whole queue and had no way to ask for more. A
            // partial answer read as the state is the failure this crate is
            // named for, arriving through a flag nobody published.
            Argument::optional(
                "limit",
                "integer",
                concat!(
                    "How many issues to read at most. Defaults to 200, which is a ceiling ",
                    "and not a total: an answer holding exactly that many may be a longer ",
                    "queue read to its limit, and the answer's `at_limit` says when it is."
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
        name: "create",
        contract_name: "create",
        description: "File a finding. Creates every label before attaching it, then mirrors the \
                      initial board column.",
        operation: "create",
        arguments: &[
            Argument::required("identity", "string", "The issue identity marker."),
            Argument::required("title", "string", "The issue title."),
            Argument::required("body_file", "string", "Path to the filled template."),
            Argument::required("priority", "string", "As scale:value."),
            Argument::required("domain", "string", "The routed domain."),
            Argument::required("runtime", "string", "The runtime filing it."),
            RUN_ID,
            Argument::optional(
                "state",
                "string",
                "The state to file it in. Defaults to ready; blocked waits on a condition.",
            )
            .of(crate::config::CREATED_STATES),
        ],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "publish_review",
        contract_name: "publish_review",
        description: "Push and publish the shared review target. Head and base are read back \
                      until the remote agrees, never taken from the write path.",
        operation: "publish-review",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("branch", "string", "The branch to publish."),
            Argument::required("base", "string", "The base branch."),
            Argument::required("pr_title", "string", "The pull request title."),
            Argument::required("pr_body_file", "string", "Path to the pull request body."),
            Argument::optional("worktree", "string", "The isolated checkout."),
            // Taken by the original and read by this operation's arm, and
            // declared by no tool — so it was always `in-progress`, and a run
            // already in `review` could not publish at all.
            //
            // That is the loop this crate's second rule requires: *a verdict is
            // bound to exact bytes; every push invalidates it — re-publish and
            // ask again*. The re-publish happens **after** a review, when the
            // contract says to *"keep claimed review work in `review`"*, and it
            // came back `expected exactly [status:in-progress], found
            // ["status:review"]` with no way for the caller to say otherwise:
            // `publish_review does not take 'expect_state'`.
            Argument::optional(
                "expect_state",
                "string",
                concat!(
                    "The state this run believes the issue is in. Defaults to in-progress, ",
                    "so omitting it chooses that check rather than skipping one — and a ",
                    "second publish, after a review sent work back, is made from review."
                ),
            )
            .of(crate::config::STATES),
        ],
        effect: PointerEffect::Published,
        writes: true,
    },
    Tool {
        name: "republish_review",
        contract_name: "republish_review",
        description: "Republish a branch whose history was rewritten \u{2014} rebased onto a moved \
                      base, or amended. Leases against the head the last publication recorded and \
                      refuses when the remote moved since. Use publish_review for every ordinary \
                      publication; this one destroys remote history.",
        operation: "republish-review",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("branch", "string", "The branch to republish."),
            Argument::required("base", "string", "The base branch."),
            Argument::required("pr_title", "string", "The pull request title."),
            Argument::required("pr_body_file", "string", "Path to the pull request body."),
            Argument::optional("worktree", "string", "The isolated checkout."),
            Argument::optional(
                "expect_state",
                "string",
                concat!(
                    "The state this run believes the issue is in. Defaults to in-progress; a ",
                    "republish after a review sent work back is made from review."
                ),
            )
            .of(crate::config::STATES),
        ],
        effect: PointerEffect::Published,
        writes: true,
    },
    Tool {
        name: "release_ci",
        contract_name: "release_ci",
        description: "Release one exact reviewed draft target to CI. Re-verifies the live review claim, globally latest publication receipt, current draft PR identity, and coherent clean target before marking ready, then confirms every outcome by readback.",
        operation: "release-ci",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("epoch", "string", "The publication epoch."),
            Argument::required("pr", "integer", "The pull request number.").counting_from(1),
            Argument::required("head", "string", "The exact published head SHA."),
            Argument::required("base", "string", "The exact published base SHA."),
            Argument::required("digest", "string", "The expected-target manifest digest."),
            Argument::optional("worktree", "string", "The isolated checkout."),
        ],
        effect: PointerEffect::Renew,
        writes: true,
    },
    // The isolated checkout. The contract asks a run to *"verify the claim
    // immediately before the first branch/worktree/file write, and implement in
    // the isolated checkout"* — and this operation does both in one call. Not
    // exposing it left the agent composing `git worktree add` by hand, which is
    // an unverified first write, and precisely what the binding forbids:
    // "Run executable reversible operations instead of reconstructing them."
    Tool {
        name: "start_branch",
        contract_name: "branch + worktree",
        description: "Create the isolated checkout for this issue. Verifies the claim first, so \
                      the branch and the worktree are the first write the claim covers rather \
                      than the first write nobody checked.",
        operation: "start-branch",
        arguments: &[
            ISSUE,
            RUN_ID,
            Argument::required("branch", "string", "The branch to create."),
            Argument::required("base", "string", "The base branch to start from."),
            Argument::optional(
                "expect_state",
                "string",
                concat!(
                    "The state this run believes the issue is in. Defaults to in-progress, ",
                    "so omitting it chooses that check rather than skipping one."
                ),
            )
            .of(crate::config::STATES),
            Argument::optional(
                "worktree_root",
                "string",
                "Where the checkout is made. Defaults to the configured location.",
            ),
        ],
        effect: PointerEffect::Isolated,
        writes: true,
    },
    Tool {
        name: "check_closing_keywords",
        contract_name: "check closing keywords",
        description: "Scan the branch and its commit messages for keywords that would close the \
                      issue on merge. Read-only, and worth running before delivery: an \
                      auto-close skips the `done` transition the contract makes mandatory.",
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
        name: "audit_board",
        contract_name: "board audit",
        description: "Compare every issue's workflow label against its board column and report \
                      the drift. Pass fix to repair what it finds.",
        operation: "audit-board",
        arguments: &[Argument::optional(
            "fix",
            "boolean",
            "Repair the drift this pass finds rather than only reporting it.",
        )],
        effect: PointerEffect::None,
        writes: true,
    },
    Tool {
        name: "changelog_notes",
        contract_name: "changelog-notes",
        description: "Extract a version's changelog entry for its tag and Release. Reads the \
                      changelog, and writes the notes to `out` when one is given. Fails closed \
                      on a missing or empty entry — a tag is immutable, so notes invented at \
                      tag time are permanent.",
        operation: "changelog-notes",
        arguments: &[
            Argument::required("version", "string", "As 1.2.0 or v1.2.0."),
            Argument::required("file", "string", "Path to the changelog."),
            Argument::optional("out", "string", "Write the notes here, for git tag -F."),
        ],
        effect: PointerEffect::None,
        // `out` writes a file at a path the caller chooses, and `writes` is what
        // becomes `readOnlyHint` in the schema — the annotation a client reads
        // to decide whether it may run a tool without asking anybody. This one
        // said read-only, the binding's table said "read-only" on the same row
        // that documents `--out <f>`, and nothing here is on the gate's path:
        // an MCP tool is Estigia's own, so `classify_with` never sees it. The
        // client's prompt was the only thing between an agent and a file
        // anywhere the process can reach, and the hint invited it to skip.
        //
        // True whether or not `out` is given, because the annotation is per
        // tool and cannot vary by argument. Over-strict on the read-only call
        // beats advertising the writing one as safe.
        writes: true,
    },
    Tool {
        name: "expected_target",
        contract_name: "expected_target",
        description: "The complete delivery target as a path/mode/blob manifest with one digest. \
                      Read-only, and safe against a tree somebody is still working in.",
        operation: "expected-target",
        arguments: &[
            Argument::required("base", "string", "The base SHA."),
            Argument::optional("worktree", "string", "The isolated checkout."),
            Argument::optional("native_start", "string", "A reviewer's target to compare."),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
    Tool {
        name: "base_movement",
        contract_name: "base_movement",
        description: "Classify later base movement as none, compatible, overlapping, conflicting \
                      or unknown. Unknown is never compatible.",
        operation: "base-movement",
        arguments: &[
            Argument::required("base", "string", "The base branch."),
            Argument::required(
                "recorded_base",
                "string",
                "The base SHA the review was bound to.",
            ),
            Argument::optional("worktree", "string", "The isolated checkout."),
        ],
        effect: PointerEffect::None,
        writes: false,
    },
];

/// Operations the contract requires that Estigia deliberately does not expose.
///
/// Frozen, and it may only shrink. Each entry says why no tool can exist —
/// which is the ratchet applied to a tool list: a missing tool with no reason
/// is a gap, and a missing tool with a reason is a boundary.
pub const NOT_EXPOSED: &[(&str, &str)] = &[
    (
        "label",
        "the GitHub binding does not map it either; exposing a tool for an operation no binding \
         provides would be inventing a capability",
    ),
    (
        "last_activity",
        "the binding maps it to a raw `gh issue view` rather than the transport, so there is no \
         executable operation to wrap",
    ),
    (
        "publish_version",
        "declared `(agent, not scripted)` — tag and Release publication is not a transport \
         operation",
    ),
    (
        "close",
        "declared `(agent, not scripted)` — closing follows the contract's own Closing section",
    ),
];

/// Arguments that are Estigia's bookkeeping, not transport flags.
///
/// Passing one through would make the transport reject the whole call for an
/// unknown option. They exist because the run pointer has to follow what became
/// true, and the transport has no reason to care whose run asked.
pub const POINTER_ONLY: &[(&str, &str)] = &[("claim", "state"), ("transition", "run_id")];

/// Whether an argument is bookkeeping rather than a flag.
pub fn is_pointer_only(tool: &str, argument: &str) -> bool {
    POINTER_ONLY
        .iter()
        .any(|(name, field)| *name == tool && *field == argument)
}

/// Transport operations Estigia deliberately does not run.
///
/// Frozen, and it may only shrink. Paired with `NOT_EXPOSED`, which answers the
/// same question from the contract's side: that one says which *required
/// operations* have no tool, this says which *executable operations* have none.
pub const NOT_RUN: &[(&str, &str)] = &[
    (
        "config",
        "the operator configuration is Estigia's own — `estigia config list` reads the same table, and a second reader of it would be a second answer to what the settings are",
    ),
    (
        "list-boards",
        "asked by the setup screen so a board can be chosen instead of typed, and by nobody else — a run does not pick which board its issues are mirrored onto, it uses the one the operator configured",
    ),
];

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
                // Published, not merely enforced. An agent that cannot see
                // the vocabulary guesses at it, and every wrong guess costs
                // a whole call.
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
