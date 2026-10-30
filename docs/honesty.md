# What this instrument does not measure

An axis list that names what is not covered is the difference between "zero defects found" and
"zero defects found where we looked".

Read this before adding a claim elsewhere in the repository. It records known limits, their current
measurements and the tests that hold them. Unmeasured cases are not clearance.

`tests/honesty.rs` crosses the *countable* claims here against the code: the number of agents, the
number of tools, the number of doctor checks, the settings rows. A count that drifts fails the
suite. Everything else here is prose held by review.

## How this file is organised

[`docs/README.md`](README.md) maps the documentation set. Entries retain their original order;
the section links below provide navigation rather than a subject index.

- [Test evidence, and the run id shape](#test-evidence-and-the-run-id-shape)
- [Comments, instructions and the build](#comments-instructions-and-the-build)
- [Models, routing and reviewer definitions](#models-routing-and-reviewer-definitions)
- [Node, CI and the actions that run the suite](#node-ci-and-the-actions-that-run-the-suite)
- [The OpenCode plugin, and what it knows about a call](#the-opencode-plugin-and-what-it-knows-about-a-call)
- [The suite, its fixtures and the retired port](#the-suite-its-fixtures-and-the-retired-port)
- [Review targets, panels and the lane that runs them](#review-targets-panels-and-the-lane-that-runs-them)
- [Findings, grants and locations](#findings-grants-and-locations)
- [The control surface and what it records](#the-control-surface-and-what-it-records)
- [What the gate writes, and who else reads](#what-the-gate-writes-and-who-else-reads)
- [Roles and the definitions they need](#roles-and-the-definitions-they-need)
- [Skips, deletions and the bounds of a round](#skips-deletions-and-the-bounds-of-a-round)
- [The correction budget and the publication lane](#the-correction-budget-and-the-publication-lane)
- [Setup, configuration and uninstall](#setup-configuration-and-uninstall)
- [The gate's reach across the agents](#the-gates-reach-across-the-agents)
- [Delivery: the receipt, the base and the remote](#delivery-the-receipt-the-base-and-the-remote)
- [The seams, and the gates that never see a call](#the-seams-and-the-gates-that-never-see-a-call)
- [The push guard, the MCP server and the state a run keeps](#the-push-guard-the-mcp-server-and-the-state-a-run-keeps)
- [What the gate covers](#what-the-gate-covers)
- [doctor, and what it checks](#doctor-and-what-it-checks)
- [Identity and the tracker](#identity-and-the-tracker)
- [The gate's boundary](#the-gates-boundary)
- [The populations and the prose they cross](#the-populations-and-the-prose-they-cross)
- [Ownership](#ownership)
- [Reachability, registration and the rounds around them](#reachability-registration-and-the-rounds-around-them)
- [What the gate narrows, and what it records](#what-the-gate-narrows-and-what-it-records)
- [What a measurement reconstructs](#what-a-measurement-reconstructs)
- [Refusals, and the way out they name](#refusals-and-the-way-out-they-name)
- [Test counts and mutation claims in a receipt](#test-counts-and-mutation-claims-in-a-receipt)
- [Citations, and the coordinates that rot](#citations-and-the-coordinates-that-rot)
- [Generated files and reproduction](#generated-files-and-reproduction)
- [Which hook commands are Estigia's](#which-hook-commands-are-estigias)
- [The Pi system section and the prompt cache](#the-pi-system-section-and-the-prompt-cache)
- [What `release_ci` says about CI](#what-release_ci-says-about-ci)
- [What `deliver` does not see](#what-deliver-does-not-see)
- [What `estigia changes` does not do](#what-estigia-changes-does-not-do)
- [The session registry](#the-session-registry)
- [The session mailbox](#the-session-mailbox)
- [The npm installer and the registries](#the-npm-installer-and-the-registries)
- [The release builds; CI tests](#the-release-builds-ci-tests)
- [The waits audit](#the-waits-audit)

The last run, **The review tier, the lens and the finding** has its own
document: [`honesty-review-tier.md`](honesty-review-tier.md). It stays last in the order and nothing
follows it. Within it, "the entry above" and "the entry below" reach the neighbouring entry, and a
pointer that leaves it names the document it points at.

## Test evidence, and the run id shape

- **A `setup` test has answered red once and green every other time over identical bytes, and the
  mechanism is unknown.**
  `setup::tests::setup_accepts_an_in_flight_body_no_ledger_entry_will_ever_cover` failed in one full
  `cargo test` on Windows (2026-09-07), the second `setup(...)` call refusing a body it had itself
  written; two later Windows runs failed once each in the same reviewer-definition fixture, the one
  captured error being an access-denied temp-file write. Measured on darwin over twenty consecutive
  full runs of identical bytes: 20/20 green. The rate is **not established** — twenty runs come back
  all-green about 7% of the time at a true rate of 1 in 8, and the loop says nothing about Windows.
  The assertion names the reviewer the loop was on and carries the refusal. The sandbox's OS temporary
  tree (an indexer, scanner or cleanup racing a fresh directory) is the hypothesis, offered as one.

- **The run-id shape cannot tell a dashed runtime from a suffix that carried one.** A dashed suffix
  such as `codex-01k4-9f2a` is refused and a non-string `run_id` is malformed in every tool. An id whose
  last-dash suffix *is* the minted shape is admitted: `session::run_id_shape("codex-01k4-9f2a1234")`
  answers `Named { runtime: "codex-01k4" }`, as it would if `codex-01k4` were a real runtime word.
  Nothing holds the runtime words an operator's hosts use, so no predicate over the text can separate
  the two. Labels minted from text the old checks admitted are permanent and left alone.

- **Only Codex's and Cursor's hooks derive their own runtime word.** `session::AGENT_RUNTIMES` maps `codex`
  to `codex` and `cursor` to `cursor`; an agent not listed derives `claude-…`. A Cursor run registered before
  that mapping keeps its `claude-…` run id and its `claude` row, and `resume` still tells it from a Claude Code
  run only by the shape of its session id. Codex registers no `SessionStart` hook from
  Estigia, so nothing tells a Codex run its id; whether the run can read its own session id is not
  measured.

- **A decision append that waits past five seconds on the store is dropped, counted, and not
  retried.** `every_decision_survives_the_ledger_passing_its_cap_while_runs_are_writing` failed on
  Linux arm64 under load (24 of 100 runs lost decisions with eight writers and thirty busy loops;
  every captured error was `database is locked` after 5.3 s to 10.6 s, the `APPEND_WAIT` bound
  expiring). The product behaves as `record` documents — best effort, a failed append counted for
  `doctor` — so the test, which assumed the bound never expires, now waits ten minutes. Open: on a
  machine starved that far a real decision is lost the same way and the `doctor` count is the only
  trace; no bound was raised, because a longer wait puts the stall on the gate's critical path. The
  same message on Windows (`could not create its version table: database is locked`) is
  **unexplained**: 4,800 raw opens and 300 rounds each of 32 and 8 concurrent `Store::open` gave none
  on macOS. Every open-path error ends `(sqlite <extended code>, after <n> ms)`.

- **A staged body path that survives a crash is named by process id and counter, so a recycled process
  id can land on a leftover.** `paths::scratch_file` adds a per-process counter so two threads never
  share a path; the crash-recycle case is not measured.

- **Load flakes with no cause found.**
  `transport::claim::tests::two_runs_claiming_one_issue_at_once_leave_exactly_one_winner_decided_by_the_store`
  (`read-failed`, expected `lost-claim-race`) did not fail in 200 runs alone under thirty busy loops;
  the store's five second busy wait is the suspect and is unmeasured. The stand-in reviewer in the
  launch tests waits for a release file bounded at thirty seconds, so a launch that wrongly joins its
  runner fails slowly, not instantly.

## Comments, instructions and the build

- **The instruction-surface ceiling counts bytes, not whether the prose agrees with the gate.**
  `tests/instruction_ceiling.rs` refuses a `skill/*.md` file above its registered ceiling, and any
  unexcepted line above 2,000 bytes. The registry is an upper bound: a stale count below the file is
  not a refusal. It cannot see conditional routing, so it does not bound the always-loaded set an
  activation pays, and it does not prove a sentence agrees with `src/harness/`.

- **The shipped skill does not carry its own provenance; the why of a rule is here.** Open
  measurements appear below under scratch isolation, review payloads and the publication lane.
  Whether `merge_group` satisfies required contexts remains unmeasured. No mechanical check detects a
  launch brief overriding the reviewer's classification rule, or measures compliance. The tier omits
  the external source's six-path review-authority self-reference: this repository's authority is not
  separable by filename.

- **Nothing measures comment prose in `src/`, and that is the decision.** `AGENTS.md` item 2 — code
  carries no comments by default — is held by review alone. A comment-counting test was deleted
  because its only cure was raising the number, and the ruling widened: **no test at any level may
  depend on the text of a comment.** The cost is that prose can accumulate or say something false and
  no run will say so. A test reading Rust source strips with `src/source_text.rs`'s `code_of`; nothing
  enforces that, and an earlier sweep that missed `std::fs::read_to_string` was rejected twice.

- **`build.rs` strips comments once, at the read.** It extracts `TRANSPORT_VOCABULARY` from each
  `src/transport/*.rs` after `source_text::code_of`, before `shipped_part` cuts the test module and
  before `extract_reasons`, `extract_constructed` and `assert_reasons_covered` walk it. The floor is
  the count `refusal-baseline.txt` records (`population 178` over `reasons.len()`). Regenerating the
  vocabulary with and without the change gave the same 123 entries.

- **`build.rs`'s envelope guard is held to the extractor's window by shape, not by a case.**
  `the_envelope_guard_and_the_extractor_read_one_window` in `tests/guards.rs` checks that
  `assert_reasons_covered` and `extract_constructed` both take their window from `envelope_window`.
  Nothing under `tests/` compiles the build script's functions, so no test drives a `"reason":` the
  walk cannot read; a second derivation of the window is what the shape test can fail on.

- **Nothing stops a comment from spelling a unicode escape instead of writing the character.** The
  test that checked this was deleted with the comment-text ruling; `cargo doc` publishes a `///` line
  verbatim, so a spelled-out escape there reaches the public documentation.

- **Nothing holds a transport port's clock to saying where it must come from.** The requirement that a
  `pub now:` clock come "never from the run being judged" exists only in doc comments in
  `src/transport/branch.rs` and `src/transport/claim.rs`; the test that read it was deleted. Nothing in
  production constructs these structs yet.

- **`AGENTS.md`'s numeric claims are not crossed.** `tests/honesty.rs` crosses the counts in `README.md`
  and `docs/`, nothing in `AGENTS.md`, which every agent reads at session start. The `tracker_rig`
  number was dropped from its sentence rather than re-numbered, and its `node` count (three tests) is
  false: `src/setup/tests.rs` has twelve test functions that panic when `node` is absent. Extending the
  guard is a change of its own, since `documented()` is `README.md` plus `docs/` and the node count has
  no exported constant.

- **No check can hold the `cargo build --bins --examples` instruction.** A stale `target/debug/estigia`
  answers exactly as a fresh one. `cargo test --test pipe` rebuilds the binary itself (an integration
  test depends on the bin target), so the pipe tests cannot run against an earlier commit; what is not
  held is a reader who probes `target/debug/estigia` directly.

- **The old-reference guard sees only numbers above a ceiling.** `tests/issue_references.rs` fails on a
  `#N` in the shipped text when `N` exceeds `PUBLIC_ISSUES_THROUGH`, raised by hand. An old reference at
  or below the ceiling passes, and the guard does not read `tests/` or the comments of `src/`.

## Models, routing and reviewer definitions

- **Model catalogs do not measure runtime availability or capability.** The Claude Code and Codex
  lists are reviewed snapshots; they can become stale between releases. OpenCode's list is whatever
  the configured host reports through `opencode models` at that moment, without a refresh, and a
  successful line is still not proof that a later run accepts or uses the ID. Estigia does not
  validate catalog membership, execute models, or inspect or filter their tool-call capability.
  Only Claude Code receives host-routable reviewer definitions: one `review-blind` definition
  installed in every mode, whose tool grant `Evidence standard` decides, and the sealed lenses and
  the refuter beside it. The two
  delegated roles `developer` and `analyst` are installed for OpenCode as well as Claude Code,
  each in its host's dialect, installed only when `Model routing` names it and retracted otherwise.
  Models reach both roles; effort reaches only their Claude Code copies. `review-blind` retains literal
  `model: inherit`, pinned against an earlier release: `judge` model and effort reach Judgment Day only
  through the launch. Other adapters have no role-definition surface. No test measures host compliance.

  `applies()` uses `AgentAdapter::can_gate_tools`: `Held` where it gates, otherwise `Asked` for
  `Delivery`, `Review`, `Transitions` and `Boundaries`, `Held` for the rest. `App::applies` separately
  decides what the shared all-installed-agents write reaches. Neither governs the CLI setter
  (`src/cli/mod.rs`). The retired `model/effort` grammar also refuses a genuine model ID ending in one
  of the five effort words (`sonnet/high`); other provider-qualified IDs remain models.

  Tolerant reads drop removed routing keys, preserve valid neighbours and report on stdout; the next
  `config set` or `config edit`, on any row, rewrites the survivors. A retired `Delegated workers` row
  folds names into routing as `inherit`. Installed-contract reads also fold it; unknown rows still
  refuse. **Limits:** a caller checking only the successful exit code misses the report; the fold and
  parse-error channels each name only their first problem. Operator receipt of the notes is unmeasured.

  Definition ownership is checked in preflight and immediately before writing, without a lock. A file
  arriving after the second read can be overwritten, for delegated roles and reviewers alike.
  `the_second_ownership_look_refuses_what_arrives_after_the_first` exercises the second branch through
  `pending`, not a real concurrent interleaving; no test drives that race.
  OpenCode and every
  other host keep the reviewer values as routing declarations. `orchestrate`, `apply`, `judge`, and a visible
  route or installed definition are likewise not proof that a host executes them.
  Claude's generated matcher now wakes for current `Agent` and legacy `Task` launches of any reserved
  reviewer target. Estigia walks from the launch cwd through the first `.git` repository root,
  parses candidate YAML, recursively refuses project-scoped shadows, and requires the normalized
  canonical user definition to be the only user-scoped definition with that identity; setup performs
  the same recursive user-tree uniqueness preflight. A running reviewer uses the embedded policy. This is Claude-only and
  proves what Estigia read before launch, not that Claude launched the context, kept reviewers concurrent
  or blind, withheld sibling output, or received an honest verdict. A refused launch is no evidence and
  never reduces or serializes the configured review; the fallback is separate-session or durable handoff.
  OpenCode process-tree cleanup is likewise not containment proof: the controller bounds how long the
  TUI waits, while process-group and Job Object cleanup remain best-effort OS operations.
- **The OpenCode review relay is a transport, not a definition and not a measurement.** With the plugin
  installed, or when no reviewer CLI is installed ([the selection rule](what-it-writes.md)),
  `launch_review_input` records the relay job and answers `relay: true`; `estigia-review-relay.js`
  hooks the host's sub-agent tool, asks the binary for the prompt the job holds, and hands the raw
  output back for admission. It proves nothing about whether the host routed to a distinct context,
  whether the sub-agent carried the template's tool grant, or whether a model answered. The relay ships
  no host-routable definition, so a host with no agent to route to launches nothing and the review
  stays owed. A result for a receipt the review has moved past is refused `capture-result-stale`.

  - **Coverage uses `node` stand-ins; live foreground transport was measured once.** The tests drive
    the generated plugin with a stand-in whose command objects honour the `<` redirect and have no
    `.stdin()`; none runs it under Bun, which would make `bun` a second required interpreter in CI. The
    redirect form and a 200,000-byte answer were read to EOF under Bun 1.3.1 and OpenCode 2.0.28's
    event shapes were read from its binary. Four native OpenCode foreground reviewers were admitted
    privately with zero findings against one exact receipt. This measures foreground transport, not
    approval, acknowledgment handling or background support. A host that changes `result.content` or the
    `(input, output)` shape leaves the plugin inert or refusing, never admitting a wrong answer.
  - **A relayed reviewer runs as the built-in `general` sub-agent, with its tool grant.** The answer
    names `general` and the marker prompt `<!-- estigia:review name=<role> -->`, because setup installs
    no reviewer agent and a reserved name is refused `Unknown agent`. `general` carries OpenCode's write
    tools where the CLI's reviewer has none, so the seal is the prompt, a weaker one than the CLI's
    tool-less reviewer; nothing reads what the sub-agent did with the grant. Inherited repository
    instructions can require acknowledgments that conflict with JSON-only output, so the binary
    supplies a job-bound result channel ([what it writes](what-it-writes.md#the-opencode-review-relay-plugin)).
    `an_opencode_reviewer_can_acknowledge_repository_instructions_before_its_result` drives the generated
    plugin under Node through real ledger admission; removing the appended channel instruction makes it
    fail with `reviewer-result-malformed`. No model's compliance is measured. The native completion
    shape was read from OpenCode 2.0.32 (external source): structured `output.output` carries final
    text and `content` wraps it for display. `a_relayed_answer_is_admitted_against_the_job_receipt`
    passes a synthetic native completion through the plugin and admission, and the driver checks
    recursively collected string values for leaked answer text. Foreground review is supported;
    background launches are refused, and no correlated private background channel exists.
  - **ZCode, Cursor and DeepSeek Harness cannot relay.** `src/setup/capabilities.rs` records no
    agent-definitions surface for any of the three, and only Cursor has a tool-call gate, which fires on
    tool calls rather than a sub-agent spawn. Delivery from any of them needs a reviewer CLI; without
    one the call is refused `reviewer-unavailable`, naming the CLI. The verdict is against the surfaces
    this build has read.
  - **`handoff_review` remains for a host that can neither start a CLI nor relay.** It records the exact
    receipt for another run; it runs no reviewer and produces no verdict.

- **Where no reviewer definition file is installed, the grant is still computed from the embedded
  template.** Six of the eight adapters have no agent-definitions root. `hook.rs`'s prelaunch check
  calls `render_reviewer_agent` on the embedded `RESERVED_REVIEWERS` template and the running
  `Evidence` value, never reading a file back. What is unmeasured is whether the host enforces the grant,
  and for an adapter with no reachable gate whether the gate ever runs.

- **The grant is a declaration whose enforcement is unmeasured on every adapter, including Claude
  Code.** The external source (`gentle-ai` `v4.0.0`, `ff77164d`) likewise establishes no host enforcement:
  four of seventeen runtimes carry file-based tool lists, Cursor has only `readonly`, and Kimi's
  `exclude_tools` is neither parsed nor asserted in its Go. Pi loader behaviour is unverified. A missing
  definitions root means a missing declaration, not loss of a demonstrated control.

- **The profile carries the two authorisations that can ask and no judge row.** Ordinary review has no
  judges: it is the lenses `review_tier` licenses, derived from `RiskLevel::lenses` and stated in
  `skill/policies/blind-judges.md`; the two judges survive only as Judgment Day. The `Authorisation
  posture` row discriminates the two authorisations that can ask. The reviewed presets are screen-only:
  choosing one expands into the `Model routing` cell, and `setup` installs a delegated role only when
  that cell names it. `analyst` is in all six presets at each preset's `judge` tier, crossed by
  `every_preset_runs_the_analyst_at_its_judge_tier`. Not measured: that any host obeys the rendered
  `model:` line.

- **An existing installation keeps its rows; a row the table is missing takes the new default, and
  nothing says so.** A table whose `Transition authorisation` row is *missing* takes `auto` on the next
  read, and `setup` rewrites the block whole, so the value is then written down; neither direction is
  reported. Measured by `an_installed_table_keeps_its_rows_and_a_missing_row_takes_the_new_default_on_setup`
  and, for reading alone, `an_existing_table_keeps_its_rows_and_a_missing_row_takes_the_new_default`.
  The CLI's own `install_planned` layering is not measured.

- **`launch_review_input` and `collect_review_input` start and record lens inputs, the refutation batch
  and the validator; the validator reads a diff the binary rebuilds, not the checkout
  `inspect_candidate` reads.** `collect_review_input` is the one caller of `reviewer::settle`. The
  recorder answers `Recording` (`Recorded`, `AnswerRejected` or `Refused`); only an `AnswerRejected`
  with an unstarted write becomes `reviewer-result-malformed`, never a refusal's code or class. The
  fourteen codes of `ANSWER_SCOPED` (`src/transport/claim/capture.rs`) are the checks about the
  answer; `invalid-finding-lens` is not one, and every other failure is `Refused`. A unit test poses
  all fourteen behind a valid finding and holds the posed set equal to the declared one.

  **Pre-checking is not a transaction.** A failed write leaves earlier findings and reports world
  `committed`. Relaunching an abandoned job starts a new reviewer; repeated `(id, class)` pairs replay,
  others add. Changing an id's class can therefore record that id twice across answers, unmeasured.
  Partial writes are crossed by
  `a_write_that_fails_after_another_landed_is_reported_as_committed`,
  `a_lens_report_that_fails_after_a_finding_landed_is_reported_as_committed`,
  `a_read_that_fails_after_a_write_is_reported_as_a_write` and
  `a_config_defect_after_a_write_is_reported_as_committed`. The pre-check misses
  `operation-id-kind-conflict`, which can surface after earlier writes. `findings_recorded` counts
  read-back findings; `findings` exposes metadata, with text behind `review_read`.

  **A repair is described to the lens.** For a publication with lineage the criteria carry the issue
  body, the parent epoch, the rule that a new severe finding owes `origin` (`introduced` or `exposed`),
  and the parent's finding ids, under a heading naming them as the parent's. Measured by
  `a_lens_over_a_repair_is_told_so_and_a_severe_answer_with_an_origin_is_recorded`,
  `a_repair_whose_parent_recorded_nothing_is_told_so` and
  `a_lens_finding_that_cites_a_recorded_parent_is_recorded`. Whether a reviewer obeys is not measured.

  **What the reviewer is handed.** The diff is rebuilt from the receipt and must hash to the recorded
  `payload-digest`: none refuses `payload-digest-unavailable`, a mismatch `reviewer-payload-mismatch`,
  both before the reviewer runs. The criteria are the issue body read from GitHub when the capture
  runs, so a later edit is what the reviewer reads. The model is the `reviewer` target of `Model
  routing` in the layered table, never an `estigia.<agent>.md` file; an unparsable entry is dropped. The
  reviewer is the first `claude`, then `codex`, on `PATH`; a launch refusal reaches the caller under the
  reviewer's own code.

  **The refuter's distinctness rests on a process boundary.** A tool-free process gets the receipt,
  rebuilt diff, issue body and offered finding ids. Its single `record_refutation` is credited to
  `review-refuter`, so `require_distinct_reviewer` passes by construction, not identity verification.
  A hand call crediting publisher/requester refuses `reviewer-not-distinct`
  (`the_capture_credits_the_role_label_while_a_hand_call_naming_the_publisher_is_still_refused`,
  `a_hand_call_naming_the_requester_of_a_handoff_is_still_refused`), and a hand `record_refutation` is
  refused `recorded-by-capture-only`. Missing, repeated or extra ids and the five unit-tested
  `REFUTATION_ANSWER_SCOPED` codes reject the whole answer as `reviewer-result-malformed`.
  `refutation-batch-incomplete` instead means the population moved since collect and is `Refused`.
  The batch is one write. Both offering and recording use `inferential_population`, including severe
  inferential findings settled as follow-ups, held by
  `a_batch_offered_for_a_follow_up_settled_finding_too_is_captured_and_the_review_moves_on`.
  A model can refute a true finding; neither reading nor correctness is measured, and one batch per
  receipt makes that answer final (`refutation-batch-already-recorded`).
  `launch_and_collect_review_input_run_the_offered_refuter_and_record_the_batch_over_the_real_mcp_server`
  drives a stand-in `claude` on Unix, not a real CLI.

  **The validator sees a diff, a list and criteria, not the checkout.** A fresh tool-free process gets
  the receipt, `parent.head..head` from recorded lineage, parent's severe findings and issue body;
  `record_validator_result` records its answer. With `delta-parent=integrated`, capture recomputes the
  merged-tree-to-head diff, while `computed_verdict` trusts the flag. A no-longer-computable merge leaves
  validation owed but refused `validator-delta-carries-upstream`. Recorded `delta-range-reason` escalates
  rewritten history, unclean base advances and conflicted merges (although the budget measures them).
  **Unmeasured:** pre-field corrections read as honest; a rejected parent with no severe findings is
  handed as `none`; and a validator without `Read` may not disclose unresolvable outside-diff citations.
  Its prompt digest is computed from the handed bytes, so `reviewer-payload-mismatch` cannot fire and
  an absent published digest does not block it. Commit ids and isolated diff settings pin the bytes.

  Launch freezes the attempt at recorded inconclusive results plus one and derives its operation id.
  An escalated-receipt store error retries once with that id, but the suite cannot reproduce the retry;
  `a_second_recording_of_the_same_validator_answer_replays_the_recorded_result` cannot distinguish
  re-settlement from early return. `validator_shape` rejects malformed answers before writes.
  `fail` records rejection below the limit, escalation at it; `unavailable` records escalation with
  `validator_inconclusive`. The three recorder refusals (`validator-result-already-recorded`,
  `validator-attempts-exhausted`, `validator-transaction-not-a-correction`) pass through.
  `invalid-validator-result` conflates bad shape and a non-next attempt: another recorder racing between
  reads would hit the latter, unmeasured because it needs two runs holding one claim. A `pass` remains
  a model's reading, not a tree inspection.

  **The lens focus** a medium change is reviewed under is the receipt's own policy focus, named on the
  report only when not the default; a recovered-lineage and a default-focus fixture measure it.

  The tool is driven over the real MCP server with a real checkout, a stand-in `gh` and a stand-in
  `claude` by `launch_and_collect_review_input_run_the_offered_lens_and_record_it_over_the_real_mcp_server`
  (Unix): a launch returning before a two-second stand-in answers, an early collect refused
  `capture-running`, the recording collect, and a refused second launch. The missing-CLI refusal is a
  unit test with a selector standing in for `select_adapter`, because a pipe test cannot ensure no real
  `claude` sits later on `PATH`.

  `inspect_candidate` takes the parent from the recorded lineage, not the pull request's history, so a
  publication without lineage, or a recovered successor, refuses `inspect-parent-missing`; that the
  parent is on the same pull request is not checked. It reads the given checkout and refuses
  `reviewer-inspect-transaction-unresolved` rather than fetching absent commits. Driven over the real
  MCP server by `inspect_candidate_reads_a_correction_between_two_publications_over_the_real_mcp_server`;
  the other refusal codes are unit tests only.

- **A reviewer launched through a Windows `.cmd` or `.bat` shim is launched by Rust's standard library,
  and that path has never run on a Windows host here.** `src/reviewer/run.rs` spawns with plain
  `Command::new(program).args(args)`. At the pinned toolchain (1.98.1) the standard library sends a
  `.bat`/`.cmd` through `cmd.exe /e:ON /v:OFF /d /c` with its own quoting and returns `InvalidInput` for
  an argument it cannot pass (`reviewer-launch-failed`); a missing shim is `reviewer-cli-missing`.
  Its documentation says the set of characters it cannot escape may change between releases, so a
  toolchain bump can move this path unnoticed. **Not measured:** that npm's shim forwards each argument
  through `%*`, that the Job Object contains `cmd.exe` and the `node` it starts, and that an argument
  survives the round trip. The TUI's catalog probe refuses a script launcher because its tree cannot be
  bounded ([`configuration.md`](configuration.md)) while the reviewer path does not ask and attaches a
  kill-on-close Job Object after the spawn; the divergence stands because reviewer cleanup rests on that
  Job Object and refusing would stop reviewers on common npm layouts. A Windows run showing the Job
  Object kills `cmd.exe` and its `node` child would settle it; none exists while the CI lane is off.

- **A reviewer's answer is read for what its text alone settles, not for what the trees or the ledger
  settle.** For a finding `parse` makes every check `record_review_finding` makes from the answer's text
  (id uniqueness, class, vocabularies, severe declarations, a location read by
  `parse_finding_location`, non-filler `evidence` and `impact`, no malformed citation path). The
  recorder can still refuse an admitted answer on checks needing the trees or ledger: a citation's path
  or line in base or head, a location inside the diff, a recorded `parent`, a repair's `origin`, a
  refuter covering every candidate; such a refusal means the answer is unusable, not recorded. The
  recorder has no per-finding text ceiling (`FINDING_TEXT_CEILING` bounds only a later read). The
  prompt delimits the diff with its byte count and digest rather than an escape, so a diff containing
  the closing marker is told apart only if the reviewer reads those, which nothing enforces.

- **A binary-launched reviewer is isolated by an allow-list on Claude Code and a deny-list on Codex.**
  `claude` runs with `--tools ""`, `--strict-mcp-config`, no session and no settings sources, so a tool
  added later stays off. `codex exec` has no equivalent: it runs `--sandbox read-only` with `--disable`
  for each stable tool-granting feature on codex-cli 0.159.2 (`shell_tool`, `unified_exec`,
  `unified_exec_tty`, `write_stdin_approval`, `sleep_tool`, `code_mode_host`, `view_image`,
  `image_generation`, `apps`, `plugins`, `plugin_sharing`, `remote_plugin`,
  `skill_mcp_dependency_install`, `skill_search`, `tool_suggest`, `browser_use`,
  `browser_use_external`, `browser_use_full_cdp_access`, `computer_use`, `in_app_browser`,
  `in_app_local_automation`, `hooks`, `multi_agent`) and `shell_environment_policy.inherit=none`. **A
  tool-granting feature a later Codex adds is not denied until added to that list**, a read-only sandbox
  alone still lets a shell read files and print them, and nothing measures the list against the
  installed Codex. Both children get a cleared environment plus a fixed allow-list matched without
  regard to case: the transport's `PATH`, `PATHEXT`, `COMSPEC`, `SYSTEMDRIVE`, `SYSTEMROOT`, `WINDIR`,
  `TEMP`, `TMP`, `TMPDIR` plus `HOME`, the platform's profile names, locale, `CLAUDE_CONFIG_DIR`,
  `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `CODEX_HOME`, `OPENAI_API_KEY` and
  `OPENAI_BASE_URL`. `GH_TOKEN` does not reach them; `AWS_*`, `GOOGLE_*` and the Bedrock and Vertex
  switches are not on it, so a reviewer routed through either refuses as unavailable. Claude runs with
  `--output-format json` because `claude -p` is reported to exit 0 with an API error as its text. One
  hand run of each real CLI (2026-10-06; claude 2.1.292 answered success on exit 0, codex-cli 0.159.2
  started and failed only on authentication) is the whole evidence that they accept these arguments;
  nothing re-runs it and the tests drive a stand-in. Classifying a failure as unavailable is a heuristic
  over the last 2 KiB of error text after the echoed prompt is removed, so it can misname a cause; every
  such code blocks.

- **A reviewer runs detached from the call that started it, and a collect records its answer.**
  `launch_review_input` writes a job directory under `~/.estigia/jobs/`, records the job against its
  slot and re-executes `estigia __review-job <dir>` detached; the runner holds an exclusive lock on
  `<dir>/lock` for its life, runs the adapter `select_adapter` returns and leaves `result.json`
  atomically (provider usage in `usage.json`). `collect_review_input` is collected only by the
  launching run; a receipt that moved discards the job as stale; admission moves the job `running` to
  `admitting` in one `BEGIN IMMEDIATE` transaction and ends `admitted`. Operation ids are the same on
  every attempt and the validator's attempt is frozen at launch, so a job a crash left `admitting` is
  admitted again without a second result. Liveness is the lock and the file, never the process id.
  **Not proven:** a `lost` job says the runner is gone, not why (nothing reads `runner.err`); a runner
  killed after the answer and before the rename reads as lost; a runner alive a minute past its
  deadline has its group killed by the collect that finds it, but on Unix only the runner's group —
  the reviewer CLI is in a group of its own, bounded by the runner's deadline — and on Windows
  `taskkill /T /F` was never run. A dead run's job holds its slot until it finishes or is found lost;
  while the runner is alive a launch from another run is refused `capture-already-running`. The unit
  tests run the runner on a thread; the real detached spawn is crossed by `tests/pipe/review_job.rs`
  and the two `launch_and_collect_review_input_*` pipe tests (Unix), and nothing exercises the Windows
  flags or a breakaway refusal (`review-job-detach-refused`). The manifest and result carry a format
  version either side refuses. A lock probe landing between the runner creating and locking its lock
  file makes the runner refuse to start (read from `take_lock`, never reproduced), and a job whose lock
  file does not exist yet reads as lost. `review_status` does not know which run asked, so a finished job
  another run launched is named for `collect_review_input`, which that run refuses
  `capture-job-other-run`. A job directory is never removed except `result.json` after admission, so
  `~/.estigia/jobs/` grows.

## Node, CI and the actions that run the suite

- **Every direct action in both workflows that declares a Node runtime is on `node24`; the workflow
  guard reads lines, not YAML, and has known blind spots.** `dtolnay/rust-toolchain` and the composite
  `actions/attest-build-provenance@v3` declare none (shell steps and two pinned `node24` children,
  read at the commits `v3` names). The guard holds a per-action floor table (`RUNTIME_FLOORS`) and a
  completeness check (`ACTIONS_WITHOUT_A_NODE_RUNTIME_FLOOR`) refusing any versioned action in either
  workflow missing from both lists, so an unread action is a red test. Each floor was measured by
  putting the old major back and seeing the guard redden.

  **What the guard cannot do.**
  - It cannot bound a composite's *children*: `@v3`'s pins were read by hand, so a future `@v4` that
    repinned a `node20` child would not be caught. Closing it means parsing another repository's YAML.
  - It does not reach a `uses:` that names no `@ref` (a local action, an unpinned `docker://image:tag`,
    a spelling the line reader does not split). A digest-pinned `docker://` or commit-pinned
    `foo/bar@<sha>` is parsed and an unlisted one reddens; what stays unbound is a commit pin's major.
    `dtolnay/rust-toolchain@stable` is caught because it is hand-listed.
  - The run of legal spellings found so far (flow-style steps, folded values, quoting, key case,
    indentation, `with:` above `uses:`) were each closed by reading each line's key and value, and
    **a `steps:` that yields no step is refused, whatever the reason**, as is a `jobs:` whose value is
    a mapping, so an unreadable spelling is a red with a message rather than a file nobody read. A
    flow-style step is refused rather than skipped. A setting counts only if it is under the step that
    reads it: `cache-on-failure` is read from under `with:` with comments stripped, keys and values
    unquoted and case-folded; `save-if` is read as the action's `save.ts` reads it (saves only when the
    lowercased input is exactly `true`), leaving a `${{ … }}` alone.
  - Not run: `cache-on-failure: yes` is refused, and whether GitHub hands the action `yes` or `true`
    was never read; `lookup-only:`'s effect on saving is not measured and the guard does not read it.

  | Passes but should not | Fails but should not |
  |---|---|
  | a checkout pinned to a **commit** — no version string to floor | a correct step the line reader cannot take apart (flow-style steps, a `steps:` yielding none) is refused, not skipped |
  | a caching step under `if: false`, wherever a cache is allowed | |
  | a `uses:` folded onto the next line, **in a third workflow** (in `ci.yml` or `release.yml` the checkout floor turns it red) | |

  This is a list of what has been run, not a census; its count is left off because a count is a claim
  to have finished looking. The version floor reads every workflow file; the checkout-exists floor is
  scoped to the two lanes measured to have a checkout. Keeping the release lane uncached is this
  repository's own judgement, not something the issue ruled.

  The episode behind it cost nine consecutive red `ci` runs on `main` (`31753883982` through
  `31760121174`); the issue's own first run returns 404, so nine is a floor.

- **The CI toolchain is pinned, and the cost is that this repository stops seeing the newest stable.**
  `rust-toolchain.toml` names the channel and the `rustfmt`/`clippy` components, and `ci.yml` runs no
  toolchain action, so rustup resolves that one file for CI and a fresh clone alike. Floating on
  `@stable` with `-D warnings` once reddened the lane on a line no diff touched (run `32419475997`,
  `rust-1.98.0`). The crate is tested only against the pinned version, so a lint a later stable adds is
  invisible until somebody bumps the file. A red lane whose run executed carries `failing_paths` (the
  source paths its failed-step log names, read from `gh run view <id> --log-failed`, not from
  annotations, which put every failure at `.github`), `changed_paths` and `outside_diff`;
  `outside_diff` is absent when either half is unknown.

- **On Windows two Pi assertions about a signalled process are not made.** A process that kills itself
  on Windows exits 1 and Node reports no signal, so the Pi session-context test expects `exit 1` there
  and the Pi loop test's `no_exit_code` case is not run on Windows: a Pi loop that retried a signalled
  `estigia goal` on Windows would pass.

- **On Windows a POSIX absolute path in a shell line is not read as absolute, so the issue gate asks
  where it would not on Unix.** `Path::new("/tmp/y").is_absolute()` is false there, and under Git Bash
  `/tmp` and `/c/…` are mounts the gate cannot place, so such a redirect or heredoc target is read as
  inside the checkout and held under `ask`: the safe direction, at the cost of extra prompts. Tests
  naming an outside target use `test_env::outside_the_checkout`. A POSIX `Worktree location` is refused.

- **The interpreter inventory is prose, and nothing derives it from the suite.** `AGENTS.md` states
  which tests require `node`, `tar`, `bash` and `python3`, and [`testing.md`](testing.md) carries the
  catalogue. No test counts spawns; `no_workflow_needs_only_interpreters_it_sets_up` in
  `tests/release.rs` asks only that one named test exists and that both workflows install a runtime.
  The names are crossed: `every_test_the_catalogue_names_is_still_a_function` fails when no function of
  a catalogued name remains, but not the role — a test that lost its `#[test]` or changed which
  interpreter it needs stays green.

- **The Windows `check` job sets the wall time of every CI run, and its cost is processes, not
  compilation.** Measured on `check (windows-latest)` by the issue that cut its fixture cost: the lib
  suite took 845 s before that issue's fixture work and about 600 s after it (576 s and 631 s on two
  runs of the same code, with run-to-run noise near ±25 %), against about 150 s on `ubuntu-latest`; a
  cached build takes about 80 s of it. Per test, CPU-bound modules run 2–2.5× slower there and modules
  that spawn `git` 15–24× slower, at roughly 100 ms a process. `cargo test --lib` spawned 24 648 `git`
  processes before that work and 17 463 after (counted with a logging `git` on `PATH`); most of what
  remains is the code under test's own Git. The tier scan now sizes and reads its blobs with one
  `cat-file --batch-check` and one `cat-file --batch` each (`blob_sizes`, `blob_contents`, through
  `transport::run_with_stdin`), which took `cargo test --lib transport::claim` from 10 617 `git`
  processes to 9 615 on macOS; its Windows timing is not measured. A scan in the suite holds one or
  two blobs, so the saving there is one or two processes a scan, not the dozens a real review of a
  wide change saves. Disabling Defender real-time scanning was tried and does nothing: it is already
  off on the runner (`RealTimeProtectionEnabled` printed `False`, run 40435200479). Not measured: the
  `tests/pipe.rs` share (about 230 s there against 90–160 s elsewhere), which spawns the binary rather
  than `git`.

## The OpenCode plugin, and what it knows about a call

- **What the OpenCode plugin knows about where a call runs, and what it does not.** The gate decides
  which run a call belongs to by its directory and the identity the host hands it. Both host
  generations carry a session id, forwarded as `--session-id`, from which the gate derives the run id
  through `session::run_id`, the identity the Claude hook mints; a call with no session id and no
  exported run id keeps the identity-less road. OpenCode 1's directory is
  `worktree ?? directory ?? process.cwd()`; OpenCode 2's is `location.directory` (measured inside a git
  worktree: the worktree, with `location.project.canonical` the originating checkout). OpenCode 2's
  shell tool is `shell` (`{command}`, no `workdir`) where OpenCode 1's is `bash`, and `GATED` carries
  both; its writes name the target as `path`, an alias `path_argument` already reads.
  `narrowed_by_the_call` reads the shell road from `harness::SHELL_TOOLS`, not the literal `bash`.

  For a **Bash** call `narrowed_by_the_call` reads `workdir`; five write-shaped tools (`edit`, `write`,
  `multiedit`, `notebookedit`, `update`) carry a target path (`file_path`, `path`, `notebook_path`),
  read through `path_argument` and clamped like `workdir`: a relative target is resolved against the
  launch directory, **placed** (spelling collapsed, resolved as far as the filesystem exists, the rest
  kept lexically, so a not-yet-created module still narrows to `<worktree>/new-mod/src`), and a landing
  outside the launch checkout and every checkout a pointer already records is discarded.

  **OpenCode's `patch` is not among them, and this gap stays open.** `patch` puts its target in a patch
  body, like Codex's `apply_patch`; `path_argument` reads only fields, so it answers `None` and the call
  is adjudicated at the project root through `checkout_root_covering`'s fallback to `launched`: two
  concurrent isolated runs writing through `patch` still refuse each other. The classifier in
  `harness::mod` records the same fallback. Nothing drives `patch` with a real patch body:
  `the_plugin_hands_the_gate_the_directory_every_write_lands_in` and
  `a_write_tool_call_narrows_by_the_directory_its_target_lands_in` hand it a synthetic `file_path`.

  **The clamped directory is evidence of which checkout the call belongs to, never `context.repo_dir`
  by itself.** Role-definition lookup and a repository's configuration read `context.repo_dir` with no
  upward search. The clamped candidate is matched against every known holding, and when exactly one
  covers it *and that holder's recorded root is not a strict ancestor of the launch directory*, that
  root becomes `context.repo_dir` (`checkout_root_covering`, read by `launched_context`). A strict
  ancestor is dropped because a root climbing above `launched` reaches a directory the call was never
  adjudicated against; a root equal to or disjoint from the launch directory survives. Held by
  `checkout_root_covering_never_climbs_above_the_launch_directory`. Without a more specific covering
  holding the candidate falls back to the launch directory. This mirrors `context_the_payload_names`
  for the host's own `cwd`.

  **What `cwd` may and may not do.** `cwd` is the host's key (adapters write it in the hook envelope,
  not as a model-composed argument). A value naming a checkout a live claim covers is taken as given; a
  value naming a directory no live claim covers is discarded and the call answered as if it carried no
  cwd. The name is placed before anyone is asked who covers it, because `holders_of` uses
  `coverage_depth`, which keeps an unresolvable spelling (`wt-a/../../nope` still starts with `wt-a`).
  Uncrossed: a `cwd` naming another run's existing worktree is still believed, so an adapter growing a
  model-composable passthrough cwd would let a write be attributed to the named holder. OpenCode's
  shell-tool schema has no `cwd` member (read from the installed binary, not held here).

  `workdir` is a tool *argument*, composed by a model on every runtime here, so it may only **narrow to
  somewhere the gate was already going to look**: a relative value resolves against the launch
  directory, is placed, and a value landing outside the launch directory and every recorded checkout is
  discarded. Placing is the comparison itself: comparing with `covers`, written for existing
  directories, left `wt-a/../../nope` starting with the launch directory and attributed to the holder of
  a component it climbed through, so a run holding one worktree could borrow another's authority.

  Where two runs hold isolated worktrees inside one base checkout, a write through a tool naming neither
  a directory nor a target path is covered by both at equal depth and refused
  `several-runs-hold-this-checkout`. A call carrying the host's session id never reaches this road, and
  neither does `deliver`'s merge decision: it loads the pointer of the run id it was given and asks the
  same session gate the hook does, so another live holder of the shared checkout neither blocks it nor
  lends it authority. A run id with no claim, an unreadable pointer, a receipt for another pull request,
  a moved head and a missing CI release each still refuse. A reviewed caller whose exact receipt has
  a recorded CI release is allowed beside another live holder, and the other holder's pointer is left
  byte-identical; the release is seeded into the test store rather than produced by `release_ci`.

  **The `workdir` spelling is not crossed against the host, and neither is the resolution base.**
  `workdir` was read from OpenCode's shell-tool schema in the installed binary, but nothing here holds a
  copy and every test supplies the key itself, so a host rename would leave the suite green and the fix
  inert. A relative `workdir` is resolved here against the plugin's `worktree ?? directory`, while
  OpenCode resolves it against its tool context's `directory`; started in a subdirectory these differ,
  the shell runs in `<subdir>/x` and the gate adjudicates `<root>/x`. Both lie inside the project, so
  nothing escapes, but *which* holder answers can be wrong. Closing it needs the plugin to forward its
  resolution base, a second owner for the rule, so it is recorded rather than done.

## The suite, its fixtures and the retired port

- **Three debug-only overrides keep the suite off production waits and double work, and a `--release`
  build compiles all three out.** `ESTIGIA_UNSAFE_SKIP_FSYNC` (`src/paths.rs`'s `fsync_skipped`) turns
  `sync_durably`/`sync_parent` into no-ops and opens the local store with `synchronous = OFF` instead
  of `FULL` (`durability` in `src/transport/store/mod.rs`), so a fresh store's nineteen migration commits
  flush nothing; in-process tests arm it through
  `set_fsync_skip_for_tests`. `ESTIGIA_TEST_READBACK_NO_WAIT` (`readback_wait_is_disabled` in
  `src/transport/claim.rs`) makes `publish_with` skip the 2+4+…+18 = 90 s read-back sleeps.
  `ESTIGIA_TEST_SKIP_PREVIEW` (`preview_skipped` in `src/setup/mod.rs`) skips setup's dry-run preview;
  a fixture exercising it arms `with_preview`. Each is read under `cfg(debug_assertions)`.
  `with_real_fsync` drives `the_real_fsync_paths_run_when_the_debug_skip_is_off` and
  `the_store_opens_in_wal_with_foreign_keys_and_a_recorded_schema`, which still pins `FULL`;
  `the_debug_only_overrides_are_compiled_out_of_release` asserts all three env reads sit under
  `cfg(debug_assertions)` and not `cfg(test)` alone; two lib tests pin `PUBLISH_READBACK_ATTEMPTS` and
  `PUBLISH_READBACK_RETRY_BASE_SECS` to the production constants. Not measured: a full publish that
  waits the real ninety seconds under load. `cargo test --release` runs with real fsyncs and the real
  preview, and the fsync-skip assertion is debug-only.

- **Tests execute the generated plugin and *require* `node`; the count is not crossed.**
  `the_plugin_hands_the_gate_the_directory_the_call_runs_in` covers both OpenCode generations with
  stand-in shell/Bun objects; `the_plugin_hands_the_gate_the_directory_every_write_lands_in` walks
  every write-shaped `GATED` tool. `the_loop_plugin_continues_while_estigia_says_there_is_work` drives
  continuation, stops, `--max` and `/flow-status` under a stubbed OpenCode 2 context.
  `the_loop_plugin_waits_on_held_or_pending_work_and_asks_again` and `cli::loop_stop::decide` hold
  bounded unread-answer retries; `the_plugin_declares_the_default_definition_opencode_two_loads`
  holds the loader shape. These tests fail without Node; none measures a live host's behaviour.

  **Loop coverage is instance-local.** One store and synthetic events establish location filtering and
  a goal re-read, not host fan-out or identical `event.location.directory`/`ctx.location.directory`
  spellings. No compare-and-set protects the re-read/enqueue gap. **Parked prompts are not drained:**
  OpenCode 2.0.16 exposes no `session.inbox.cancel`, so one queued prompt can arrive after stop.
  `session.synthetic` with `resume: false` scheduling nothing is protocol evidence only.
  `last`/watch records cannot distinguish an uncalled plugin, missing `session.execution.succeeded`
  events or disconnection losses from no turn ending. Resubscription uses a stubbed stream; the
  2026-09-30 session's lack of reaction remains unexplained.

  **Children are an in-memory map.** Restart loses earlier children and permits immediate continuation;
  an unseen child end costs one continuation per timer interval. OpenCode 2.0.26's `session.created`
  `parentID`, four execution-event shapes and inability to list children were read from its binary.
  A parent deciding just after a child ends waits for the timer or its next turn end.

  **Event names and rendering are read, not seen.** The event name comes from `@opencode/schema`;
  OpenCode 2.0.20 displaying synthetic `description` rather than `text`, with newlines, comes from
  binary renderers. Neither prompt delivery nor work is measured: repeated idle prompts look like
  progress. `claimable` counts unassigned `review`/`ready` items, not claims taken. `dischargeable`
  trusts recorded clauses; malformed/unreadable ones (`unparsed`/`unread`) can hide a discharged item.
  `--max` counts closed `status:done` items mentioning the run only when `delivery_close` proves the
  merge parents; squash/rebase deliveries are missed, so the cap can overrun. Install details are in
  [`what-it-writes.md`](what-it-writes.md).

  **The wait sees counts, not liveness.** `held` includes any unclaimable open `in-progress`/`review`
  item: stale holders, this run's requested review or earlier claim, and person-parked items. A holder
  never finishing can wait forever; `--max` counts completions only. Pending clauses can also wait
  forever; unread ones count pending, malformed ones become `unconditioned` for a person. `open`
  precedes the 200-item cut while `count` follows it, over-counting review `held`; blocked counts see
  at most 200. Eviction/reload loses the process timer until location boot and storage scan, leaving
  `next` in the past meanwhile. Re-arm tests use a fake clock and non-paging `storage.scan`.

  **The generic `/goal` proves nothing about the work, and this is not Estigia evidence.** The `Goal
  loop plugin` row writes `goal-loop.js`, whose `/goal` completes when a model answers `MET` about the
  transcript: never read from the tracker, never reaching the gate, `verify-claim` or `estigia goal
  complete`, and holding no Estigia path (asserted from its source). Not measured: that
  `ctx.session.generate` is on a real host's plugin context (read from the 2.0.20 binary and the 2.0.16
  pick list), its return shape (`{ text }` and `{ data: { text } }` are both read), that its context is
  the whole transcript, and that a judgement request does not perturb the session. A self-paced `/loop`
  has no cap unless `--max` is given.

  **The Codex loop was measured against a mock model, not a real one.** On codex-cli 0.159.2 with the
  row `on`, `hooks/list` showed the gate, `SessionStart` and `Stop` trusted; against a stand-in model API
  and a stand-in `gh` whose ready queue held two issues, then one, then none, `Stop` blocked twice with
  the preamble naming `Run id codex-…` and `$flow dev`, and allowed the third time. Not measured: any
  real model's response, a long loop (one growing turn with no cap of Codex's own, ended only by an
  interrupt), whether Codex compacts such a turn, and how a real Codex surfaces an allowed stop's
  `systemMessage`.

  **A `$flow dev` inside a continuation does not load the skill.** With the skill under
  `CODEX_HOME/skills/flow`, a first prompt of `$flow dev` added the skill body once and `/flow dev` and
  `flow dev` added nothing; across two continuations the count stayed one, since Codex reads mentions
  from the input a turn starts with. A loop whose operator did not open with `$flow` is left with the
  text, the `flow` entry in the skill list and a model that may or may not read `SKILL.md`; that case was
  not run against a real model.

  **What the Codex loop does not port.** `--wait` (a `Stop` hook answers and exits, so a held or pending
  queue ends the turn with a message), `--max` (nothing counts delivered issues; the preamble omits the
  cap), per-session operator text, and `/flow-status` (there is no stored goal to print). Each `Stop`
  runs the four queue reads `estigia goal` runs, and a second set when the first was not confirmed,
  inside a 180 s timeout, so a slow tracker adds that latency to every turn end and one slower than the
  timeout ends the loop. `owned` recognises exactly the three commands setup writes, while
  `strip_hooks` on uninstall drops any handler whose command merely contains an Estigia event name, so a
  foreign Codex hook spelled that way would lose its entry and keep its trust table.

  **An older `node` test skips.** `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer` spawns
  `node --input-type=module --check` and, when `node` is absent, prints a line and returns: a pass that
  measured nothing, the same shape as the skip filed against the push-guard fixtures.

- **`estigia goal complete` re-reads the receipt and the lane; it proves nothing about the work.** It
  reads, at call time, that the run's receipt is the newest complete publication on its pull request,
  that the verdict recomputed from the finding ledger is `approved` and equal to the recorded marker (a
  `low` receipt does not need that), and that the head's lane is not red and finished — or,
  independently, that the tracker calls the issue closed. The stored verdict carries no lane state. It
  cannot see the work: findings are reviewers' declarations never re-checked, the receipt proves which
  bytes were published not that they were right, a closed issue is the tracker's word, and the merge
  and the claim's honesty are outside it. Any other shape answers `complete: false`; an unreadable
  tracker answer or lane is `unread`/`unreadable`, never `complete`. The verb writes nothing.

- **The transport is ported, and the Python is not shipped.** The retired script, suite and
  `tests/transport/oracle.json` corpus are gone. Their installation-path-dependent keys could not be
  extended without the reference implementation. Agreement with that implementation is no longer
  asserted; only named unit tests hold regressions, including most defects the crossing found.
  **No check in this suite reaches a live tracker.**

- **The scripted-`gh` fixtures no longer share the process environment, and one trap replaces
  another.** The fake `gh` and its answers travel through `crate::transport::with_process_override`, a
  thread-local the process-spawning seam reads, and `scripted_gh` caches its directory per test binary.
  Not measured: a fixture whose own spawned threads call the transport does not inherit the override and
  reaches the real `gh`, with nothing failing loudly. Two such fixtures name
  `with_scripted_gh_across_threads`, which keeps the process-wide lock; a third written the wrong way
  would fail on a machine with no `gh` and could pass against a live tracker on one that has it. Only
  `bounded::run` honours the override; `bounded::run_process`, which the model listing calls, spawns the
  program named.

- **Load flakes still without a cause.** `cli::tests::a_repository_that_tightens_is_not_broken_for_doctor`
  once failed with the real `gh auth status` broken the first of two calls (timeout or network, not
  distinguished); the test now answers it from a scripted `gh`. The review-runner race in
  `transport::claim::tests::capture::` (`await_started` reporting a finished runner dead) is re-read
  after the thread finishes and held by a test that makes the finish happen between the two looks;
  `a_count_read_that_fails_after_the_findings_landed_reports_the_write` panicked in the same `launch` and
  is read as this race, not reproduced by name. `with_fake_clis` points fakes at the null device, with
  the caller's choice and `ESTIGIA_FAKE_EXIT_WITHOUT_READING` as the two opt-outs.

## Review targets, panels and the lane that runs them

- **Estigia can prove what it launched, not who answered.** `publish_review` freezes a clean draft
  receipt over epoch, PR, head, base and manifest digest. `launch_review_input` and
  `collect_review_input` use embedded instructions and fixed adapter arguments; `reviewer::select_adapter`
  chooses the first installed `claude`, then `codex`, with `--model` only when `Model routing` names
  `reviewer`. Lens and refuter payloads must match the published digest. The validator instead reads
  `parent.head..head`, covered by no published digest. Only Claude receives `--no-session-persistence`.

  Nothing authenticates the answering model or account: remapping, fallback and ignored `--model` are
  indistinguishable. The timeline records role, receipt and launching run, not adapter, arguments or
  template. Two labels can be one model on one account, and the launcher may be the author. Distinctness
  rests on a process boundary, not a signed answer or recorded requested/reported model.
  `a_quota_failure_on_a_lens_records_nothing_and_leaves_release_ci_refused` in `tests/pipe/part_09.rs`
  holds refusal of an absent answer (`reviewer-unavailable`, `reviewer-exited-nonzero` or
  `reviewer-cli-missing`); hand reports, refutations, validator results and lens findings refuse
  `recorded-by-capture-only`. No test establishes independence of a present answer.

  A distinct run's computed verdict of any outcome resolves an exact-receipt handoff; only `approved`
  permits delivery. `release_ci` recomputes the newest receipt's verdict, compares its marker, and checks
  a current draft PR and clean target. The four distinctness checks are listed in [`parity.md`](parity.md).
  Refutation compares the supplied `refuter` label with publisher/requester ids, not the recording run:
  the claim holder records on the refuter's behalf. An honest label is therefore still required.

  **An abandoned pull request's handoff keeps excluding its publisher from later pull requests.**
  `review_eligibility` reads a timeline with no PR state. Abandonment cannot discharge a handoff; a
  distinct judgement must. Held by `an_abandoned_pull_requests_handoff_still_excludes_its_publisher`.

  Closed-issue `verify_claim` requires the registered publication, closing PR and merge parents
  (reviewed base, then head) to agree; other closes are `issue-not-open`, unreadable closers a failed
  read. Its message distinguishes the holder's reopen-and-transition continuation from another close.
  None of these identity checks proves reviewers read the bytes or gave honest findings. Judgment Day
  and OpenCode Task launches remain instructions: context existence, lens count, concurrency,
  independence, blindness, same-finding identity and composition are not observed. Collaborators outside
  Estigia can forge markers. The enforced floor is one computed exact-receipt verdict, with per-finding
  evidence credited separately; there is no per-reviewer verdict or multi-verdict adjudication.

- **Judgment Day's rules are stated, not enforced.** Ordinary review licenses no judges.
  `skill/policies/judgment-day.md` states the contract: explicit request only, never alongside a lens
  review, no refuter, no receipt or gate effect, findings reported to the operator and filed as issues,
  only a severe finding every judge reports fixed, the operator asked before round one, at most two fix
  rounds and two scoped re-judgments, `APPROVED`/`ESCALATED` as the only verdicts and one final
  `JUDGMENT:` line. Nothing counts those rounds (`repair_rounds` reads publications, and a Judgment Day
  has none), checks that a fix touched only the confirmed IDs, or reads the final line. The fix role is
  the `developer` definition or the orchestrator's own writer, so a correction is an ordinary claimed
  write. A judge's finding recorded with `record_review_finding` enters the ledger like a lens's and can
  reject the computed verdict. Nothing refuses a run that convenes Judgment Day over a target it also
  reviews with lenses, or launches `review-refuter` beside it, or observes that the operator asked.
  `review-blind` and `review-blind-sealed` stay installed and inert. `judge_workspaces` scopes its
  directories to a publication's receipt identity, so a Judgment Day over an unpublished target chooses
  them by hand. `tests/no_judges_in_ordinary_review.rs` holds the rest of `skill/` and the rendered
  contract to "ordinary review acquires no judge" by reading sentences and fails closed: a sentence
  naming a judge and an acquiring verb (run, spawn, use, require, need, add, dispatch, acquire, launch,
  convene, license) fails unless its exact text is in `ALLOWED` with a reason, and a second test fails
  when a listed sentence no longer occurs. Passive voice is caught. What remains: a verb outside the
  list (start, invoke, call, get, have), a judge and verb split across two sentences, a text naming the
  definitions (`review-blind`) with no judge word, the Judgment Day policy file and judge definitions
  (not scanned), and an `ALLOWED` reason, which is a reviewer's claim.

- **`judge_workspaces` hands out directories and proves nothing about where a judge wrote.** It reads
  the run's `Worktree location`, mints one **bare working directory** per judge outside the claimed
  checkout, reads each back, and refuses rather than returning a short list. The directories are not git
  checkouts, so a `reading` judge has nothing to build. Estigia cannot see whether a judge stayed in its
  directory, whether the orchestrator handed it over, or whether two judges shared a scratch path its
  caller never passed through the operation; only the case where no judge was offered a distinct
  directory is removed.

- **The review-target payload is bounded, and handing it is still an instruction.** `publish_review`
  materialises per-path unified hunks against the receipt's recorded base, once per epoch, and binds
  `payload-digest` on the published marker. The answer carries `payload_bytes` and `payload_digest`
  always and the text only when `--inline-payload` asks. Overflow of `REVIEW_PAYLOAD_CEILING` (262,144
  bytes) refuses `review-payload-too-large` before any remote write; the payload is never truncated.
  Estigia cannot prove the orchestrator asked for those bytes or handed them to a judge; the judgment-day
  policy asks for the payload first, which is prose nothing enforces. One real publication of 145,756
  bytes was refused by the host's own result cap at 151,889 characters, not by this ceiling
  (**reconstructed, not measured**: the cap counts the whole escaped JSON answer; the 6,133-character
  difference was never measured). Estigia implements no answer-size cap.
  **`payload_digest` is checkable only by somebody who can rebuild the bytes, and the recipe is in no
  document a caller reads.** It is `git diff --no-color --no-ext-diff --no-textconv --no-renames
  --unified=3 <base> <head>` under `pinned_diff_args`' five `-c` settings, run inside the fresh, empty
  `GIT_DIR` `isolated_diff_env` builds (sharing only the repository's object store), the child's
  environment replaced rather than layered: `ISOLATED_ENV_ESSENTIAL` plus `LC_ALL=C` survive, and
  `isolated_diff_env` supplies eight of its own: `GIT_DIR`, `GIT_OBJECT_DIRECTORY`,
  `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_COUNT`,
  `GIT_ATTR_NOSYSTEM` and `LANG`. The two `NOSYSTEM` ones are load-bearing. `diff.algorithm`,
  `diff.interHunkContext` and `diff.indentHeuristic` are unpinned but the isolated config carries none,
  so a rebuilder gets git's compiled default; a different git version is the residual
  `payload-rebuild-mismatch` names. A person following the recipe chooses the ends by hand and can diff
  the wrong pair; the operation takes object ids from the marker. A **Rust** consumer can call
  `estigia::transport::target::review_payload` (`pub`; `pinned_diff_args` and
  `review_payload_bounded` are `pub(crate)`). Nothing crosses this sentence against the code.

  `review_payload_membership` reads a candidate reviewer-facing text from a path, recomputes the payload
  as `publish_review` did, and reconciles the digest against the marker's `payload-digest` **before**
  asking about the candidate. A disagreement (a different git version, since both ends are object ids from
  the marker) answers `payload-rebuild-mismatch` with no line reported; a marker with no recorded digest,
  or one resolved only through an inherited lineage parent, answers `payload-digest-unavailable`. Only
  then does it test candidate lines for membership, never completeness. **It does not check order,
  multiplicity or file boundaries**: a `-` line shown after the `+` that replaced it, one real line
  repeated, or a line moved under another file's `diff --git` header still answers `ok: true`.
  `claim::tests::review_payload_membership_accepts_reordered_duplicated_and_cross_spliced_real_lines`
  pins all three. `ok: true` buys "every line was published somewhere", not "this text is a faithful
  rendering of the change".

- **A sealed reviewer holds a copy, and nobody holding it can check it.** A sealed lens, or Judgment
  Day's sealed judge, has one tool, a web search (`src/setup/mod.rs` renders it), so it cannot run the
  recipe; an unsealed judge gets a shell only where `Evidence standard` is `measuring`. What a sealed
  reviewer reviews is the copy the orchestrating context re-emitted into a launch prompt, which Estigia
  never sees: an abridgement, re-wrap or dropped hunk yields a review of something adjacent, in the
  register of a verdict, with every identifier correct. This is a third limit, besides whether the
  orchestrator handed the bytes on at all and whether a reviewer stayed inside what it was handed. It
  has happened: `skill/policies/judgment-day.md` records a sealed judge detecting an abridged payload,
  and in one panel the orchestrator re-emitted a row's count `30_174` as `39_121`, a well-formed line
  the lens held no tool to compare; the launch was withdrawn only because the orchestrator re-read it.
  **What bounds it:** an oversize payload is refused rather than truncated; a finding naming any receipt
  but this one is refused; and a structured location is admitted against the change
  (`finding-location-outside-the-change` for a severe finding disposed `introduced` or `worsened`, plus a
  path check for a lens). So an answer is bound to the candidate's identity, never to its bytes. The
  project this design is ported from computes no digest over the patch bytes it delivers either (read at
  `a537c63e`); there a program composes the bytes and here a model does.

- **A Judgment Day judge's starting context is enumerable only for what Estigia writes.** The reserved
  `review-blind` definition carries the review contract — the finding classes, the rule that decides
  between them, and no tracker operations — and not the tracker binding. Measured at this HEAD
  (LF-normalised UTF-8 bytes; `tests/honesty.rs` re-derives every figure and refuses a drift):

  | Surface | Bytes | How measured |
  |---|---:|---|
  | `skill/bindings/github.md` | 18,397 | file on disk |
  | `skill/policies/blind-judges.md` | 42,675 | file on disk |
  | `skill/policies/judgment-day.md` | 15,969 | file on disk; loaded only when the operator explicitly asks for Judgment Day |
  | Installed `skill/SKILL.md` | *rendered* | `skill::rendered` over `Config::default()` — not a file size; the shipped block is rewritten |
  | `skill/bindings/github/delivery.md` | 42,450 | file on disk; a **publishing** run loads this companion per `SKILL.md`'s delivery row |

  A run that does not reach publication loads the binding and the installed contract, plus the policy
  its path uses: `blind-judges.md` for ordinary review, `judgment-day.md` only when the operator
  explicitly asks for Judgment Day. A publishing run loads those plus `delivery.md`. The rendered
  `review-blind` definition (reading grant, unsealed) is measured through
  `setup::render_reviewer_agent` and crossed by the same test; it is the judge body, not the
  starting context total. On every adapter but Claude Code no definition is installed at all, so
  there the rule reaches a judge only if the orchestrator pastes it. Claude's agent frontmatter has
  no field that suppresses CLAUDE.md or skills. OpenCode `Task` has none either. Host assembly is a
  named limit, not a cleared gap.

- **A lens report records that the binary admitted a lens's answer, not who gave it.**
  `record_lens_report` stores that a role finished against a receipt, written by the run holding the
  `review` claim, because a sealed lens holds no tools. The computed verdict refuses `approved` without
  a report for every role the tier licenses, and a hand `record_lens_report` is refused
  `recorded-by-capture-only`. A `low` receipt licenses no lens and releases on the structural readback
  alone; at `medium` the focus is read from the reports themselves, so a single wrong report is
  indistinguishable from a right one until a second disagrees.

- **A `review-verdict` marker releases nothing, and the one thing it can still do is refuse.**
  `release_ci`/`goal_completion` require the computed verdict. A distinct reviewer's old rejection
  still refuses every tier as `legacy-verdict-rejected` (or `rejected-on-insufficient-evidence`), even
  after its finding is refuted; only a new receipt clears that objection. `repair_rounds` treats old
  rejected parents as corrections and accepted parents as approvals; accepted parents also trigger
  `approved-republish-without-cause`. Distinct old verdicts still resolve their handoffs.

  Computed markers decide acceptance when both kinds exist, but an aggregate rejection still rejects
  a computed-approved receipt. An old marker credited to a forbidden run can make republish count a
  correction even where release accepts. Computed approval is final against `escalate_review`; only
  a distinct allowed run's approval supersedes escalation. These are attribution checks, not protection
  from forged/imported comments or direct store edits.

  `record_review_verdict` refuses all calls `review-verdict-retired`, including nonnumeric issue/PR
  strings, naming arguments as ignored/receipt. `the_retired_verdict_tool_leaves_the_pointer_alone`
  checks registration because refusal precedes pointer effects. Deterministic lens findings need no
  second reading; only the refutation batch can drop inferential findings. No test authenticates the
  person behind a stored marker.

- **`Publication lane: off` skips the check-run read and says so.** With the row off,
  `record_computed_verdict` does not call the check-runs API for an approval, `publication_lane`
  answers `not-consulted`, and the marker carries `classification: not-consulted`. A repository whose
  Actions cannot run is answered mechanically (`disabled` for `enabled: false` or every workflow
  `disabled_manually`; `never-ran` for empty-steps runs; an unreadable `actions/permissions` is a failed
  read), so `off` is for skipping a lane that *could* run, including red or `cancelled`, and a read this
  token cannot make. The `disabled` read proves only those two cases: any other workflow state leaves
  `absent`, a workflow outside `.github/workflows/` is dropped before the predicate, and the read is asked
  only where the head has no check runs. No fixture poses a non-active state other than
  `disabled_manually`, so that limit describes the predicate, not a measured outcome.

- **A proof that says nothing is refused, and the list of what counts as nothing is a floor.** One
  predicate, `says_nothing`, decides a finding's `id`-excluded `evidence` and `impact` and the
  `--never-ran-evidence-file`, matching the trimmed lowercased value against `SAYS_NOTHING_WORDS`;
  `claim::tests::every_says_nothing_word_earns_its_place` iterates the list. Filler is unbounded, so the
  list is `TooTight` on purpose and its `Population` says so; `tests/guards.rs` fingerprints it and
  reopens the claim when it changes. It errs the other way too: `none`, `same` and `unchanged` are
  refused as an `impact`, and a `suggestion` with no impact needs a round trip
  (`cosmetic only; no user-visible impact`). It deliberately does not refuse on length, so `nothing was
  executed` survives. A finding's `id` keeps the plain blank check, but it is the key
  `record_refutation` groups by, so two findings recorded as `id: "-"` collapse into one identity: a
  recorded gap. The checks run before the tracker read, so a replay of a marker written before the
  predicate existed, carrying filler, is refused rather than read back.

- **The tier's cross-check cannot fire through the real path, and that is measured.**
  `review_tier_answer` derives the level from the diff through `classify_risk` and from the published
  reasons through `risk_level_from_reasons` and refuses when they disagree. Both turn on the same event
  (every candidate reason carries a signal; the fallback's single reason mirrors the remaining arms), so
  two independent derivations agree. In the tree: `explained_matches_level` extracted so a test can hand
  it a pair the real path does not produce, a fourteen-shape enumeration asserting agreement (with a
  count assertion so the list cannot shrink silently, and it cannot speak for a shape outside the
  fourteen), and a test pinning the pair it must accept. **Not in the tree:** a sweep of 1,008 single-stat
  and 96 two-stat combinations (written, run, deleted), and the guard's own wiring: every new test calls
  `explained_matches_level` or the derivations directly and none calls `review_tier_answer`, so deleting
  the single line that invokes it leaves those tests green (the end-to-end `review_tier` tests do
  redden). `passive_and_configuration` is the one shared helper production, `classify` and `agree` call.
  The `Population` declares `FailClosed`. `only_passive` already implies `touches_configuration` is
  false, so that term of the `Low` arm excludes nothing; not a defect.

- **Seven reserved reviewers are launched by the binary and installed by nothing, and the gate's refusal
  of a hand launch is by name, not by finding a file.** `launch_review_input` launches every sealed lens,
  the refuter and the validator from templates embedded with `include_str!`, with `--tools ""`, so setup
  installs only `review-blind` and `review-blind-sealed` (Judgment Day's judges, opt-in) and retracts an
  earlier build's `review-lens`, `review-risk`, `review-readability`, `review-reliability`,
  `review-readback`, `review-refuter` and `review-validator` under the reviewer's own rule,
  `reviewer_is_ours` (a current or historical rendering, or the recorded digest), not `RETIRED_AGENTS`,
  which would delete an operator-edited copy. An owned, unedited copy is deleted and forgotten; an edited
  one is kept, reported `Kept` and relinquished; a path with no record is never touched. The same
  function runs under `uninstall`. Tests: `setup_retracts_*`, `setup_keeps_*`, `setup_leaves_*`,
  `uninstall_*binary_launched*` in `src/setup/tests.rs`. The names stay in `RESERVED_REVIEWERS`, and the
  gate refuses a `Task` launch of one as `reviewer-launched-by-the-binary` after the project walk,
  whatever sits in the user tree; the refusal names `launch_review_input` and no setup command. Nothing
  constrains OpenCode, whose lens `Task` contexts are not reserved names. The `reviewer` routing key's
  effort half and the `judge` key outside Judgment Day are declarations nothing in the binary reads
  ([`configuration.md`](configuration.md)).

- **`review_tier` allocates the source project's `0`/`1`/`4` lenses, and what differs is named.** `high`
  runs `review-risk`, `review-readback`, `review-readability` and `review-reliability`; `medium` runs the
  one lens its `focus` names, `reliability` by default. Resilience is filled by `review-readback`, a
  question re-derived here. The readability override for large pure documentation is **not implemented**
  (the source's `DominantLens` is declared no longer derived), so large pure documentation is `low` with
  no lens. `review-lens` is still launched for the delta-lens repair round and sits in no tier. The tier
  reads boundaries, not size: the changed-path count at zero, the per-path authored line count at zero,
  and `SCAN_BYTE_BUDGET` in bytes, over which reads `high`. Held by
  `the_tiers_license_zero_one_and_four_lenses` and
  `the_lens_count_stated_beside_review_tier_is_the_one_the_tier_derives`. The tier's boundary claim in the
  `src/transport/tier.rs` module doc is not crossed by any test, since `source_text::code_of` strips
  comments; the falsifiable absolute was removed.

## Findings, grants and locations

- **A structured finding is a legible claim, not a checked one.** `record_review_finding` refuses a
  class outside `severe`/`warning`/`suggestion`, a finding missing its identity, evidence or impact or
  supplying filler for either of the last two, a severe finding whose `evidence_class` or
  `causal_disposition` is absent or outside its vocabulary, a receipt that is not the current one, and —
  across a repair — a parent identity the parent receipt never recorded or a new severe finding stating
  no origin. The computed verdict reads the ledger those gates keep honest.

  Every one is a check on **shape and reference**. Nothing here can check that a `severe` finding is
  severe, that a `suggestion` is not a defect in disguise, that the evidence reproduces, that the impact
  is real, or that a repair `introduced` what its reviewer says. A reviewer that classifies every
  preference as `severe` clears the rule as a careful one does. The two severe declarations are checked
  for presence and vocabulary alone, so a severe finding declaring `insufficient` and `unknown` has
  satisfied the gate while saying neither (though the guard refuses a `rejected` verdict whose severe
  finding declares `insufficient`).

  Narrower limits:

  - **A finding identity is a spelling, not a comparison.** Two markers with the same `id` name one
    finding; two lenses describing one defect differently record two. Nothing computes agreement from
    identities. The policy asks reviewers to derive the identity from the affected behaviour and
    location; nothing measures whether they did.
  - **The fix-delta digest names a repair; it does not scope a review.** A publication over an earlier
    one records the whole parent receipt and a digest covering both ends, derived from the timeline.
    The targeted validator (payload built in `src/transport/claim/capture.rs` from `delta_parent` and
    `review_payload_declared_at`) and every tier lens on a correction whose recorded range is honest
    (`delta-range` present, no `delta-range-reason`, not a recovered successor, same pull request) get
    `parent-head..head` with that payload's digest; a lens on a dishonest range, a marker written before
    `delta-range` existed, or a first publication gets the whole `base..head` (`review_payload`), as does
    the refuter. Each fallback and each of the four conditions `lens_reads_delta` joins is held by a
    capture test that fails when the condition is removed. When the parent head cannot be resolved the
    capture refuses `read-failed` and launches no lens. A lens handed the delta is not prevented from
    reading more, and a lens finding is checked against `base..head`, not the delta. `publish_with`
    shares `repair_range` with `review_tier_for_repair` to record whether the range was honest
    (`delta-range`) or why not (`delta-range-reason`: `history-rewritten`, `empty-repair-range`,
    `base-advanced`, `merged-range`). Whether a reviewer reads that field is unmeasured: the criterion is
    met by the *record*, not an enforcement.
  - **A lineage is silently absent when the parent publication came from another account.**
    `latest_publication` keeps only comments `viewerDidAuthor` vouches for. `republish_review` refuses
    (`published-transaction-missing`); `publish_review` does not, so a continuation after a cross-account
    reclaim publishes as a **first** publication and neither lineage rule applies to its findings:
    permissive rather than a wrong delivery.
  - **`introduced` and `exposed` do not cover a severe defect that was always present, always
    reachable, and simply missed.** Against a repair publication such a reviewer has no honest origin
    word: `--parent` names nothing recorded, both origin words are false, a warning means `rejected` is
    refused, and recording nothing leaves a real defect unable to block. Measured on epoch
    `3ac4a576aa80ec96bc42ce4283e94f93` by one of five judges (no quorum). Widening the vocabulary is its
    own issue.

  **Judge isolation is unproved.** Judges run only as Judgment Day. The role gate is fed the *embedded*
  reviewer definition rendered with the effective `Evidence standard`, and
  `the_shipped_blind_reviewer_gets_the_grant_its_standard_decides` pins both standards: `Write`, `Edit`,
  `Agent` and `Task` denied under either, `Bash` following the row. Under `reading` two judges cannot
  write over each other; under `measuring` they can by design and the isolation rule answers it. A panel
  this harness cannot place in the role is invisible both to the reserved-role prelaunch check and to
  that gate. Nothing here records how many such judges ran,
  whether two of them shared a working directory, or what they wrote into it. The isolation rule in
  `skill/policies/judgment-day.md` is prose an orchestrator follows or does not, on the same footing as
  panel size and blindness above.

- **A non-severe finding is informational, and nothing about delivery waits on it.** The
  `disposition-missing` gate is retired, following `gentle-ai` v3.7.0
  (`internal/reviewtransaction/advisory_disposition.go`, read at `e2cb2bf2`): a `WARNING` or
  `SUGGESTION` "requires no action of any kind". `record_disposition` remains but is optional and may
  answer a subset; an unrecorded finding is delivered as informational by class alone. The recorder
  checks **shape**: the id names a finding this exact receipt recorded, no identity whose every recording
  is `severe` is named (one recorded at two classes is answered for its non-severe recording), a `filed`
  entry carries an issue number, and a reason is neither empty nor filler. Nothing checks that an
  `acted-on` repair happened, that a `filed` number names a real issue, or that a `declined` reason is
  real. The answer is the **author's own** and requires no distinct context, so the record is auditable,
  not independent. The pre-change disposition ratio (18 closed issues: 30 `severe`, 232 `warning`, 69
  publications, 14 rejected verdicts) was never re-derived; the "share of publications spent on changes
  no reviewer rejected" figure is owed to whatever re-measures repair rounds under the informational rule.

- **A sealed reviewer's grant is enforced by this gate, and what the gate reads is never the file on
  disk.** `reserved_reviewer` answers which of nine reserved names carries which seal. Both CLI and
  hook render the embedded `reserved_reviewer_row` template and seal.
  `a_tampered_sealed_definition_does_not_widen_what_its_doors_allow` replaces disk tools with
  `Read, Grep, Glob, Bash` and still requires both doors to refuse `Read`; removing the sealed name
  reddens it. Nine reserved names comprise two installed judges and seven binary-launched reviewers;
  seven rows are sealed. Actual launch composition remains unobserved.

  Claude Code's 2026-09-02 probe found `WebSearch` launched with exactly one tool and no filesystem
  reach; this is the sealed rendering. It still reaches the public internet without waking the gate.
  Other hosts were not measured. `none` and `TodoWrite` were refused as "unrecognized";
  `ReportFindings` and `ListAgents` as "not available to subagents". Both `[]` and `ToolSearch`
  launched with zero tools, but `tools_line` reads `[]` as no policy (all permitted), and
  `ToolSearch`'s purpose is fetching schemas.
  **Unmeasured:** host enforcement of grants; names sent by the two matcher-less hosts; project-shadow
  refusal outside Claude; and whether a judge stays inside its supplied evidence. `Read`, `Grep` and
  `Glob` wake none of the matchers this repository writes (`src/setup/render.rs`), so the gate cannot
  enforce their absence even though embedded rendering is tested.

- **`reviewer_is_static` keeps the plain cross product `Evidence::all()` × every seal
  `RESERVED_REVIEWERS` carries; no reachability predicate narrows it, and that is a decision.** Byte
  acceptance follows the renderer, not the settings table. Setup's path-based
  `validate_reviewer_definition` runs two passes, one per installed row, and can nevertheless refuse
  a safer sealed body at the unsealed path.
  Measured: an owned `~/.claude/agents/review-blind.md` containing the sealed rendering passes the
  unsealed pass, then refuses `reviewer-definition-unowned` in the sealed-name pass before writes.
  Setup and sync stay blocked until `move <path> aside before setup`; static-byte acceptance does not
  establish path ownership.

- **A location checks consistency with the diff, not causation.** `record_review_finding` refuses a
  severe finding whose `causal_disposition` is `introduced` or `worsened` and carries no `location`. A
  value that does not parse as `path:line` or `path:start-end` with a repository-relative path is
  refused under any class or disposition. Hunk membership is checked only where a location is owed
  (`severe` with `introduced` or `worsened`) and proves only that the diff touched that line; the check
  refuses the **impossible** and is silent on everything merely wrong. `warning` and `suggestion` never
  owe a location, but a malformed one is still refused. `pre-existing` and `base-only` are unverified by
  construction: a `pre-existing` finding may name a line the change never touched, because refusing it
  would reward withholding the coordinate.

  **The predicate is containment, not overlap:** a range settles only when one hunk's range contains the
  whole claimed range (`range.start() <= claimed.start() && claimed.end() <= range.end()`), and the
  refusal names the first unchanged line inside the span. Measured over the tracker's 885 recorded
  `review-finding` markers, 14 were in the gated class and 5 carried a span; 2 would fail containment
  (both on PR 209, one unchanged line each).

  **Containment assumes separated new-side hunks.** `changed_new_side_lines` uses `--unified=0`,
  `--no-renames` and `--inter-hunk-context=0`. Four modify/delete/keep fixture shapes in one repository
  produced `2`/`4` or `2`, never adjacent `2`/`3`; this probes, not proves, the precondition. Split ranges
  over-refuse; fused ranges wrongly admit unchanged lines. The invocation pins inter-hunk context,
  removes `GIT_DIFF_OPTS` in `super::run`, and disables external diff/textconv. Measured hazards:
  `GIT_DIFF_OPTS=-u3` widened a one-line hunk to seven; `diff.interHunkContext=5` fused edits at 3/7
  across unchanged line 5. These fixtures do not exhaust Git's possible hunk shapes.

  **A path the diff touched but that carries no new-side hunk settles by having been touched at all** (a
  pure deletion, deleted file, binary or mode-only change): an empty range vector settles by the exact
  `diff --git a/<path> b/<path>` header having appeared; no header settles nothing.

  The pathspec is `:(literal)<path>`, not `<path>` after `--`, because pathspec magic like `:!x` selects
  every other path. `:(literal)` still prefix-matches directories (measured on git 2.55), so only hunks
  under a header naming **exactly** `path` are kept. `parse_finding_location` refuses any path containing
  a colon, which also rejects `path:line:col`.

  **Renames admit broad locations.** One-path filtering precedes rename detection (Git 2.55): old path
  is deletion-only, admitting any line; new path is an addition, admitting every file line.
  `--no-renames` pins that treatment rather than proving a cited line caused the defect.

  **The diff is read locally**, from `context.repo_dir`. A checkout missing the objects for `base` or
  `head` meets a failed read naming both revisions and the path (`super::run`'s detail is truncated to
  the first three arguments, so `changed_new_side_lines` adds them back). No test drives the missing-
  objects shape, only a directory holding no repository.

  **The header pin covers all four of git's prefix variables.** `pinned_diff_args` pins
  `diff.mnemonicPrefix=false`, `diff.noprefix=false`, `core.quotePath=false`, `diff.srcPrefix=a/` and
  `diff.dstPrefix=b/`: a repository with only `diff.srcPrefix` set rendered `diff --git x/<path>
  b/<path>`, `touched` stayed `false` and a correct location was refused
  `finding-location-outside-the-change`. `--default-prefix` is not used because it needs git ≥ 2.41 and an
  older git would silently ignore the unrecognised `-c` keys.

- **A verdict's basis is a declaration, not a check.** `basis` is optional on the retired verdict
  operation, stored as sent, never re-run. A rejection rests on a recorded `severe` finding, not on
  `basis`. An accepted verdict with zero findings and no `basis` is well-formed: a `reading` judge has no
  commands to name, and Estigia cannot tell that from a `measuring` judge that skipped stating one. What
  changed is that "measured and said so" versus "said nothing" is a field on the timeline.

- **`record_spend` is a declaration, not a measurement.** Estigia cannot see a subagent's token count,
  wall-clock seconds, or how many contexts produced a figure. `input`, `tokens`, `seconds` and `contexts`
  are supplied numbers stored and read back; nothing confirms them, a run that under-reports is
  undetectable, and `conditions` is free text trusted the way `basis` is. The orchestrating run's own
  consumption is the sharpest gap, since a context cannot read its own count; the hand-measured cost table
  says its task-flow layer is not instrumented and its total is low by an unmeasured amount. `input` and
  `tokens` stay separate fields because dividing one by the other would look like a measurement while
  resting on the same unverified declaration twice.

- **The crossing test proves presence, not consistency.** The requirement has one home,
  `skill::VERDICT_BASIS_REQUIREMENT`. Three of seven surfaces carrying it are rendered from those bytes
  (both reviewer-definition arms and the OpenCode review block); four are hand-authored (the policy, the
  MCP tool schema, the contract's developer step, the operations companion's `Recording a verdict`
  section). `the_verdict_basis_requirement_reaches_every_generated_launch_text` crosses all seven; two of
  the four are checked against the region that must carry it. The count of carriers checked that way is
  prose, here and in `CHANGELOG.md`, and nothing crosses it. `str::contains` does not catch a surface that
  keeps the requirement and contradicts it elsewhere (which has produced real defects, including two
  inside crossed carriers) nor one that discusses `basis` without being a carrier.
  `the_stale_prose_rule_and_its_carve_out_reach_both_carriers` asserts within sections (`Review contract`
  in the definition, `What a finding is` in the policy); the `skill/SKILL.md` paraphrase is deliberately
  uncrossed.

- **A frozen-body ledger cannot be a definitive fix.** `REVIEWER_TEMPLATE_BEFORE_*` in `src/setup/mod.rs`
  requires a human to freeze each superseded body before release; omission leaves nothing a test can
  detect. `write_reviewer_definition` also records the exact path's digest through
  `written_outside`/`note_written_outside` in `src/skill/record.rs`, falling back to the ledger when
  absent. Neither rescues a body stranded without either record (`reviewer-definition-changed`),
  and skipped setups leave no digest for intermediate published renderings.
  `historical_reviewer_renderings` spans every frozen template and reserved `(name, seal)` pair;
  digest acceptance does not make bytes current for `reviewer_is_static_for_seal`.
  Single-writer ownership is unenforced: one Claude skill root's record is asked; another adapter
  sharing its directory under another root would be invisible. One machine's 2026-09-05 `review-lens`
  measurement confirmed exit 1→0 and all seven digests bootstrapped despite `changed 0 file(s)`;
  it establishes neither retroactive rescue nor multi-writer safety.

## The control surface and what it records

- **Estigia cannot make a person decide or start a turn.** For a human adjudication that is not a
  routine wait on the ordinary review, CI, identity, and delivery-permission gates it can preserve
  the built branch, PR, receipt, checks, and evidence, and record `blocked` with the exact decision
  or exit condition and discharger before ownership is released. That durable record is not a
  scheduler, notification guarantee, or decision; only the named person can discharge it. A routine
  wait on those gates stays in `review` with its exact blocker, even when a named person must supply
  the permission. The Decision Gates table in `skill/SKILL.md` owns those destinations.

  **Routing and single-sourcing are only partly held.**
  `every_named_disposition_uses_a_transport_state` crosses the table against `STATES`, proving reachable
  destinations, not correct routing: swapping two leaves the suite green.
  `the_decision_gate_is_not_restated_as_a_second_table` catches arrow/backtick restatements in four
  companions and a `repository-delivery.md` table headed `| Destination |`. Renaming that header while
  retaining the pointer sentence restores the duplicate with the suite green. Arrow-free prose passes;
  `SKILL.md`'s own steps restate routing outside the parser's section. Other spellings rely on review.
- **One review-protocol readback is held by no test, and one is held only by a string search.**
  Disabling `handoff_review`'s post-release readback leaves the suite green. Its retry receipt recheck is
  held only by `the_compound_handoff_records_before_release_and_checks_review_afterwards`: deletion
  fails, ignoring the answer does not. A 49-mutation sweep found about twenty further removable checks
  in replay validation, operation ids, marker fields, idempotency and visibility readbacks; this is not
  an exhaustive inventory. Requester exclusion, first-write receipt checks, distinctness, live claims,
  CI release, escaping and the four markers' field shapes did redden tests. Exact-whitespace guards in
  `claim.rs` can falsely attribute line-ending changes to a caught mutation.
- **Four stops around `ensure_draft` and the closing-keyword scan are unheld.** No test holds the
  `[world-action]` guidance's `Refs #<n>`. `Answer::already_wrote`, which selects `translate`'s exit arm,
  lies outside the `exit-code` fingerprint; its `StatusRequired` axis has only the fingerprint tripwire.
  The shared range scan reads different checkouts: isolated publication versus repository-root
  `assess_autoclose`. `keyword_sources` exposes two shapes under one key, with no parser today.
  A body saying `close #464` closed #464 on merge; publication now refuses every reference, but the
  standalone check still returns before scanning when the delivered issue has no closing PR, missing
  a body that closes only another issue. These limits have no behavioural test named here.
- **The follow-up's single-sourcing is named, not observed.** `mandatory_follow_up`
  (`src/transport/commands.rs`) and the branch-link follow-up (`src/transport/claim.rs`) both interpolate
  `closing::FOLLOW_UP`, and each call site's test compares the emitted text to the constant's value rather
  than its source: re-inlining a byte-identical copy at the `check_closing_keywords` site leaves both green
  (measured). What is held is the transition: deleting `transition --to done` from `FOLLOW_UP` reddens
  `follow_up_names_both_the_transition_and_the_release`.
- **Nothing enumerates the refusals that write before they refuse, and nothing can.**
  `Answer::already_wrote` selects `translate`'s `Committed` arm only for `"world": "committed"`.
  Reachability after a write depends on interleaving, not a source walk. Repaired sites have tests
  driving their refusal through `translate`; this is not a complete inventory. Still answering
  `NotStarted` over a timeline marker, with no proof of unreachability: both `claim-operation-expired`
  readings, `claim-metadata-mismatch`, `reclaim-metadata-mismatch`, `unassign-metadata-mismatch`,
  `claim-operation-conflict`, `reclaim-operation-conflict`, `unassign-operation-conflict`,
  `operation-id-kind-conflict`, both `resumed_release` stops and post-write
  `held-by-other-holder-disappeared`. On another repository's #318, two reported non-writes left two
  control markers no verb could reach.

- **The control surface reaches further than a checkout, and the reach has a price.** A write outside every
  checkout the claim covers is answered `outside-the-claim` without asking the tracker: right for a scratch
  note and for a `Boundary` write (which stays gated), wrong for a `Routine` write to a file that governs
  the harness, so each adapter's instruction file, `~/.claude/settings.local.json` and `~/.claude/agents`
  are `Boundary`. The instruction files are derived from the adapter table;
  `every_control_file_an_adapter_has_is_one_the_gate_measures` resolves the real path per adapter on all
  three platforms, under two `XDG_CONFIG_HOME` layouts, on both roads, for the bare root as well as a file
  inside it. It catches a stale fragment **only where nothing else already matches the path**: staling each
  of the eleven in turn, it reddens for one (cursor) and stays green for ten (`claude.md`, `agents.md`,
  `gemini.md`, `qwen.md` and `crush.md` match as whole segments at any depth; continue, cline and windsurf
  are covered by their rules directories). Nothing is unguarded:
  `the_spelled_instruction_files_and_the_adapter_table_agree` reddens for all eleven.

  **Gating a file in a directory the host reads whole is defeated by a neighbour.** Continue applies any
  rule with no frontmatter and Cline loads its rules directory for every task, so a sibling of Estigia's file
  saying *Estigia is retired* changes what an agent is told; the rules directories are gated.
  `windsurf/memories` and `.cursor/rules` are in the population on a weaker basis (`paths_in` does not say
  those hosts read a directory whole, and Estigia writes nothing into `.cursor/rules`), since whether a host
  reads a directory whole is a fact about somebody else's product. The Continue and Cline rules directories
  are literals in `CONTROL_SURFACE`
  (`the_rules_directories_of_continue_and_cline_stay_gated_without_an_adapter_deriving_them`; Windsurf's by
  `a_retired_hosts_instruction_directory_is_still_a_boundary`), so a future adapter's rules directory is
  gated only if somebody adds the literal.

  **The cost of a `Boundary`.** A `Boundary` never rides the renewal window and never stands aside outside the
  claim, so it is a live `gh issue view` every time: 30 invocations through the real hook counted **30
  tracker calls out of 30** against **0 out of 30** for a `Routine`, and with no network it is a refusal.
  Three measurements of the read disagree (means 0.93 s, 0.66 s, 1.74 s; range 0.58 to 3.20 s), so the honest
  statement is **a round trip to GitHub per write to a watched path, between half a second and three seconds**.

  **Anchoring and over-gating.** A fragment ending in `/` names a directory, so a name that merely *ends*
  alike (`.estigiaignore`, `skills/flow.md`, `.claude/agentsmith.md`) stays `Routine`; the left side is
  anchored for dot-fragments and directory fragments. A fragment naming a **file** without a leading dot cannot
  be anchored (`cli/hosts.yml` must match mid-segment: the Windows path is `%APPDATA%\GitHub CLI\hosts.yml`),
  so vendored copies answer `Boundary` (`vendor/myopencode/agents.md`, `vendor/mygemini/gemini.md`,
  `~/.codeium/notwindsurf/memories/global_rules.md`) and two-component file fragments match anywhere
  (`.claude/settingsmap.ts`, `docs/gemini/settings.json`; deliberate, since `.claude/settings` is trimmed to
  reach `settings.local.json`). An exact collision in somebody's own tree and every per-project entry
  (`<repo>/.claude/agents/*`, `.cursor/rules/*`, `.cline/rules/*`, `.continue/rules/*`, `CLAUDE.md`,
  `AGENTS.md`, `.claude/plugins|commands/**`, `.codex/prompts/**`, `.cursor/commands/**`, `.gemini|qwen/extensions/**`,
  `.continue/config*`, `.codeium/windsurf/**`, `.windsurf/rules/**`, `.opencode/agent/**`) costs a tracker read
  per write; this repository's own `CLAUDE.md` and `AGENTS.md` trip it. **Over-gating is held unevenly**: the
  `Routine`-direction fixtures list paths by name and nothing derives one from the adapter table, so widening
  a derived fragment to its bare directory is caught only where a hand-written row sits under it. Eight
  widenings redden (`.agents/agents`, `.claude/claude`, `.codex/agents`, `opencode/agents.md`,
  `.cursor/estigia-workflow-authority.md`, `.continue/rules/estigia.md`, `.cline/rules/estigia.md`,
  `windsurf/memories/global_rules.md`; Cline's is not re-measured) and five are silent (`.qwen/qwen.md`,
  `gemini/gemini.md`, `crush/crush.md`, `.zcode/agents`, `.dsh/agents`). The under-gating direction is crossed
  against `resolve_paths` for every adapter, the retired six included.

  **The shell road's punctuation fold is fixture-bounded.** `surface_of` supplies separators for fragment
  matching. Nine repaired spellings and eighteen folded characters are pinned, not the space of shell
  spellings; no generated-command property test exists. Tests include
  `punctuation_does_not_hide_a_control_surface`,
  `a_drive_relative_spelling_is_measured_like_a_relative_one`,
  `a_relative_operand_is_measured_like_an_absolute_one`,
  `a_later_operand_does_not_collapse_an_earlier_surface`; the other direction by
  `folding_punctuation_does_not_gate_an_ordinary_line` and `an_option_prefix_does_not_gate_an_ordinary_line`.
  **Open:** `${P-path}`, `${P:+path}` and `${P:?path}` answer `Routine` (once `$`, `{`, `}` and `:` fold away
  `P-path` is a file named `P-path`; laddering from a mid-segment `-` would also gate `x-.claude/agents`);
  substitutions, a surface split across a `cd` and a glob that empties a directory without naming it are
  invisible; a command that merely *mentions* a control surface while writing elsewhere is `Boundary`.

  **The containing directories are `Routine` while their contents are `Boundary`:** `~/.claude`, `~/.codex`,
  `~/.cursor`, `~/.qwen`, `~/.agents`, `~/.zcode`, `~/.dsh`, `~/.gemini`, `~/.cline`, `~/.continue`,
  `~/.codeium`, `~/.codeium/windsurf`, `~/.config/crush`, `~/.config/gh`. A single recursive delete of
  `~/.claude` takes the settings file the gate is registered in, the directive, the agent definitions and
  the installed contract, and being outside every checkout is answered by `outside-the-claim` before the
  tracker is asked. A fixture holds it for `~/.claude`, `~/.agents` and `~/.codex`. It is an ordinary path a
  shell writes, not the hard-link limit; the shape that would close it already works next door
  (`~/.config/opencode` and `~/.estigia` are `Boundary` because those entries name the directory).

  **Two readings of the same text, deliberately.** The write road hands `is_control_surface` a `file_path`; the
  shell and payload roads hand `text_names_a_control_surface` a command line or patch body, and only the second
  folds punctuation. On the write road a decorated relative spelling (quotes inside the field, a leading
  space, tab, `~`, `$`, `%`, `{`, backtick or `=`) is `Routine`, which is not a hole: each either fails
  outright or resolves to a different directory, and only the bare spelling reaches the file and is gated.

  **Open, neither closed.** `<repo>/.opencode/plugins` has no entry (`opencode/plugins/` is anchored,
  `.opencode` is not it; Estigia reads nothing from a project-local plugin directory). `curl "-o<path>"`
  with the value attached **and quoted** is not seen as a write at all, upstream of `surface_of` in
  `shell::writes_a_file` (`wget "-O<path>"` is a `Boundary`). `harness::roles::definition_for` reads its five
  definition roots from a hand-spelled list: `every_definition_root_is_a_boundary_on_both_roads` crosses all
  five, but adding a sixth reddens nothing, and a definition not found is `Ok(None)`, which
  `declared_policy` reads as *every tool allowed*. `gh`'s hosts file decides which account every tracker
  call acts as and is named in `CONTROL_SURFACE` at its default location only: `GH_CONFIG_DIR` and
  `XDG_CONFIG_HOME` move it and those spellings answer `Routine` (measured at `/home/me/xdg/gh/hosts.yml`);
  `config.yml` stays `Routine` on purpose (default host and protocol, not the credential).

  **The contract refusal comes before every answer except two.** A control-surface write is refused when no
  contract is installed, even outside the claim
  (`an_unreadable_control_surface_refuses_even_a_write_outside_the_claim`), and the renewal window's `Allow`
  sits below it (`an_unreadable_control_surface_permits_no_write_inside_the_renewal_window`,
  `an_unreadable_control_surface_refuses_a_write_outside_the_claim_inside_the_window`). One stand-aside still
  answers above it: `another-checkout` fires on the directory the hook was invoked in rather than the write
  target, so a run claimed in one checkout whose hook fires in an unrelated one stands aside without the
  contract being read (unlike `nothing-sworn` and `no-tracker`, it is reached by a run that does hold an
  issue; `a_claim_in_another_checkout_does_not_gate_this_one` is green with no contract on disk).

  **Limits of the stand-aside.** A **hard link** outside the checkout to a file inside it cannot be crossed
  (`mklink /H`, `ln`, a Linux `mount --bind`), the classification is taken before the write so a link created
  in between is not seen, and a write tool that creates missing parents can make a directory inside the
  checkout on the way to a target that is honestly outside. A landing that is not on a drive cannot be
  compared to a checkout that is, so `placed` declines it and the caller reads that as *inside*: the feature
  is **off** on UNC ground (a claim over `C:\...\repo` gates a write to `\\192.168.1.10\pikaflix\...`, and a
  run whose checkout is on a share stands aside for nothing). The guard has no measurement on a machine that
  does not serve its drives as shares, the newer fixture passes when `mklink /D` is refused for want of
  Developer Mode, and a landing with no drive-prefix component would be refused instead of compared as
  relative (no test). **The POSIX half of `placed` is measured by nobody who reads it**: the `..` handling is
  where the platforms disagree and the arm is a `cfg!(unix)` expression that never runs on a Windows desk
  (`a_write_that_lands_inside_the_claim_is_gated_however_it_is_spelled` once wrote through
  `<root>/decoy/../repo/src/main.rs` and went red on both POSIX lanes only after the last verdict was
  recorded); the publication lane now starts CI against the pushed head and `record_computed_verdict` refuses
  an approval for a head whose lane is red or running, so a POSIX-only failure is found before the verdicts
  are spent, but nobody reading the change runs the POSIX arm.

## What the gate writes, and who else reads

- **The gate named what Estigia writes, and the host reads further than that.** The instruction
  *file* each adapter's `setup` authors is derived from the adapter table; the rules *directories*
  are literals in `CONTROL_SURFACE`. So a twelfth adapter's directive is gated with no edit to any
  list and its rules directory is not: with an `acme` adapter whose instructions resolve to
  `~/.acme/rules/estigia.md`, the neighbour `zz-override.md` answered `Routine` with the whole suite
  green. `.cursor/rules/` was always a literal, because Estigia writes no file into it.
  `agents_root` is a `match` on an enum rather than on `adapter.slug`, since a `&str` match can
  never be exhaustive.

  The surface is decided by one principle: *a path is on the control surface when a host loads it
  without a person choosing it at the moment of use*. Under it the skills tree around Estigia's own
  installed contract (derived per adapter from the field that decides where the installer writes),
  plugin payloads, commands and prompts, the Gemini and Qwen extension roots, Continue's assistant
  configuration, Windsurf's MCP file and workflows, and the per-project twins (a project's own root
  `CLAUDE.md`, `AGENTS.md`, `.clinerules`, `.windsurf/rules`) are all `Boundary`. A slash command is
  invoked explicitly, but its name and description are put in front of the agent whether or not it
  ever is, so the principle puts it on the surface.

  **What it costs.** `CLAUDE.md` and `AGENTS.md` are `Boundary` in this repository (a claimed run pays one live
  tracker read; an unclaimed one is `Outside` and pays nothing). The write road is not the whole count:
  `apply_patch` and `patch` put the path in a patch body, so `classify_with` reads the whole payload and a
  patch to `README.md` whose body contains *see AGENTS.md for the rules* is `Boundary`; and a **shell** line
  pays it whenever the command is already one `writes_a_file` recognises, including a trailing comment:

  ```
  Routine   git commit -m "update AGENTS.md"          Routine   echo see CLAUDE.md
  Routine   grep -rn agents.md src                    Routine   echo hi > /repo/notes.txt
  Boundary  echo 'see AGENTS.md' > /repo/notes.txt
  Boundary  rm -rf /tmp/scratch # see AGENTS.md
  Boundary  sed -i 's/x/y/' /repo/notes.txt # per AGENTS.md
  ```

  The rule is *whether the line writes at all*: a command that is not a write never reaches the matcher, and
  one that is pays for every control-surface name anywhere in its text. No count in this document covers this
  road.

  **What is open.**

  - A fragment holding no separator is a whole segment on the **left** only, so
    `<repo>/AGENTS.md.bak` and `<repo>/CLAUDE.md.orig` are `Boundary`, as `.claude/settings` reaches
    `.claude/settingsmap.ts`. Deliberate: the right side is where `settings.local.json` and
    `CLAUDE.local.md` live. `.continue/config` is trimmed to reach `config.yaml`, `config.json` and
    `config.ts`, so `~/.continue/configuration-notes.md` is `Boundary` too. Further instances follow
    the same shapes (`/repo/.cursorrules-old`, `/repo/.windsurfrules.bak`, `/repo/qwen.mdx`,
    `/repo/vendor/gemini/extensions/pack/manifest.json`). One shape is road-dependent, and the suffix
    ladder is its cause: `/repo/-agents.md`, `/repo/-claude.md`, `/repo/-qwen.md` and
    `/repo/--agents.md` are `Routine` to `Write` and `Boundary` to `rm`. The cost is one tracker read
    and nothing is under-gated. The fixture holds representatives, not the family.
  - `<repo>/.opencode/plugins` is `Routine`; Estigia reads nothing from a project-local plugin
    directory, which is the stated reason and not a measurement. The over-gating direction is not
    derived from the adapter table (the measurement is in the previous section).
  - A host that reads a directory whole and is declared not to is invisible here. The declaration
    moved from a distant list onto the adapter; it did not become checkable.
  - **Flipping the Evidence standard row does not re-render the installed definition, and nothing reports the
    drift.** `config set` writes the contract table and reads it back but never writes an agent's files
    (`docs/configuration.md` states it as a safety property). Measured: `estigia config set "Evidence
    standard" measuring` answers `Evidence standard is now measuring` with exit 0 while
    `~/.claude/agents/review-blind.md` still reads `tools: Read, Grep, Glob`, and `doctor` answers `ok
    canonical — and every configured agent reads the rows it decides by`; only `setup` or `sync` re-renders.
    A grant the row no longer asks for is refused at the gate, which enforces from the embedded definition
    rendered with the effective row; with the row widened and the file not re-rendered the gate **allows**
    and only the host's stale `tools:` line refuses, if it enforces one. `reviewer_is_static` accepts any body
    `Evidence::all()` × `RESERVED_REVIEWERS` renders, and `reviewer_is_static_for_seal`, which a launch reads,
    cannot see the drift either.
  - **Only one of the two derived families is compiler-forced.** The measurement is in the roles
    entry's *Still open* list, under the bullet beginning *"Only one of the two derived families"*.
  - **Plugin-provided sub-agent definitions are the same shape as `.opencode/agent/`.**
    `~/.claude/plugins/**/agents/*.md` and `.opencode/agent/` (singular) are `Boundary`, but
    `roles::definition_for` searches neither, so such a definition is `Ok(None)` on read, which
    `declared_policy` reads as *every tool allowed*. They are gated against being written, not read
    when enforcing; a sixth definition spelling is known and not searched.
  - **Other wildcards of the `agents_root` shape survive.** Adding an `InstructionFile` variant
    fails to compile in several places, and six `_` arms on that enum in shipping code then hand it a
    default nobody chose. This is a reading by `grep`, not a measurement; nothing derives the list:

    | where | arm | reachable by the next adapter |
    |---|---|---|
    | `setup/mod.rs` `hooks` | `_ => return None` | only behind `gate_spec` |
    | `setup/mod.rs` `plugin` | `_ => None` | yes |
    | `setup/mod.rs` `apply`'s plugin body | `_ => plugin::source(…)` | only behind `plugin` |
    | `setup/mod.rs` `model_profiles` | `_ => &[]` | yes, and advisory |
    | `setup/mod.rs` `model_catalog` | `_ => …None` | yes, and advisory |
    | `setup/render.rs` `gate_gap` | `_ => "no published reference…"` | only the next one, and the text is no longer benign |

    `mcp_config` is compiler-forced, so a new adapter's author must choose an MCP file; `paths_in`
    explains why — an MCP entry written where the agent does not read it reports success and
    configures nothing. `gate_spec`'s and `mcp_format`'s wildcards are spelled out in
    `src/setup/render.rs`, which is what stopped ZCode and DeepSeek Harness getting *no gate* and the
    flat `mcpServers.estigia` default silently.

    Two defaults of the same kind are not `_` arms and are invisible to that grep: `supports_mcp` and
    `can_gate_tools`, both `matches!`-shaped, so a new variant falls to `true` for the first and
    `false` for the second. A test catches each rather than the compiler —
    `every_agent_that_supports_mcp_resolves_a_config_path` and
    `the_gate_gap_and_the_gate_never_disagree`. One more `_` arm sits in test code
    (`src/setup/tests.rs`).

## Roles and the definitions they need

- **`Evidence standard` gives the reviewer a shell, and a shell is not under the gate.** The reserved
  reviewer's grant is derived from `Evidence standard` (`reading`, the default, or `measuring`). Under
  `measuring` the role gets a shell and nothing else: `Write`, `Edit`, `Agent` and `Task` stay refused
  (a test walks `Evidence::all()` asserting that shape), which withholds four tools and not the
  capability, since a shell writes and launches. A judge that measures writes inside the directory its
  launch hands it, and **when that directory is outside the claimed checkout** the stand-aside answers
  before the tracker is asked. That condition is the orchestrator's to meet and nothing enforces it: a
  delegated context's tool events carry the launching session's id, so a judge handed a directory *inside*
  the claimed checkout is measured against the parent run's live claim and **allowed**. A five-judge panel
  sharing one scratch directory once had a judge's script overwritten by another's and executed in a
  third's checkout, so two verdicts stopped being independent. The surviving statement of what a shell does
  not buy is in `skill/policies/judgment-day.md`. The gate reads the row from `GateContext` and **an
  unreadable contract answers `reading`**, because a fault must not hand out a capability.

  `reviewer_is_static` means *one of the spellings Estigia can produce*, enumerated over `Evidence::all()` ×
  `crate::harness::roles::RESERVED_REVIEWERS`; the compiler holds the `Evidence` axis closed, and the seal
  axis (a plain slice) is held by `every_seal_has_a_reserved_name`. **No acceptance test enumerates the
  sealed rows**: the walk over `Evidence::all()` is one block inside
  `the_default_rendering_is_the_bytes_already_installed`, two sealed bodies are pinned one at a time
  (`a_sealed_rendering_is_static`, `a_sealed_lens_rendering_is_static`), and `review-risk`,
  `review-readability`, `review-reliability` and `review-readback` sit where no acceptance assertion reaches
  them; `seal_has_exactly_two_values` pins only `Seal::all().len()`. The crossing that binds the *installed*
  bytes, `the_reserved_reviewer_grant_is_the_same_through_both_doors`, walks a hand-written pair of rows, so
  a third standard reaches it only if somebody adds it.

  **The workflow front door is reachable the same way.** `estigia claim` and `estigia transition` are CLI
  entrances onto the operations the tool server runs, and the role gate bars tool *names*, not shell
  commands, so under `measuring` the shell that reaches the target reaches those. What holds a judge off
  them is the sentence *"Still do NOT change tracker state"* in its own definition: an instruction, not a
  gate. A judge holds no claim, so a write against the repository still refuses; what is unguarded is the
  tracker call in its own name. At the grant "no delegated context gains orchestration authority" is true;
  at the shell it is prose.

  **`setup` and `sync` reach further than a redirected `HOME`.** A measuring judge's scratch-home sync
  once rewrote two real-profile files through `%APPDATA%`. `Environment::resolve` now accepts inherited
  `APPDATA`/`XDG_CONFIG_HOME` only under the supplied home (`paths::covers`, through nearest existing
  ancestors), otherwise substituting a home-relative fallback with a note, not a refusal.
  **The gate differs:** `harness::roles::definition_for` reads `XDG_CONFIG_HOME` raw and searches both
  the default and relocated OpenCode roots; nothing tests that divergence.

  Three enumerated cases lose a legitimate location: relocated `%APPDATA%`, `XDG_CONFIG_HOME` outside
  `HOME`, and `HOME` differing from `%USERPROFILE%` with unchanged `%APPDATA%`. Setup can report success
  after registering where the host never reads. The note is the only warning; the process cannot tell
  intentional relocation from incomplete home isolation. This is measured on one machine, not an
  exhaustive population (`what-it-writes.md`). The cost remains for OpenCode and uninstall of retired
  hosts. **Uninstall uses the same substitution:** real `%APPDATA%\gemini\settings.json` `BeforeTool`/MCP entries
  and `GEMINI.md` can remain live, with no route back except the note.

  **The test-only environment lock excludes writers, not all readers.** `src/test_env.rs`'s
  `ENVIRONMENT` also excludes readers opting into `with_environment_read_locked`, but unlocked reads of
  `HOME`, `USERPROFILE`, `APPDATA` and `XDG_CONFIG_HOME` remain, including `harness::roles::tests` calls to
  `definition_for`. Thus unsafe `set_var`'s precondition is not met. A picker race failed 3/15
  `cargo test --lib home -- --test-threads=8` runs and is fixed for that test only. The hand-maintained
  `// SAFETY:` notes name known readers, not a complete inventory; no test detects a new unlocked reader.

  **One row, one file, and a machine can hold two answers.** `Evidence standard` is `Scope::Everywhere`, so
  it is set per repository, while the artifact it decides is a single user-level `review-blind.md` rendered
  from whichever checkout last ran `setup` or `sync`. On a machine holding one `reading` and one `measuring`
  repository, the drift `docs/configuration.md` frames as lasting until the next `setup` or `sync` is
  permanent for whichever was not last written. The gate answers per repository and narrows on an unreadable
  contract, so this costs a capability rather than granting one.

  **Still open.**

  - No derived directory family is compiler-forced: `skills_root_fragment` matches `SkillsRoot`, whose
    variants a new adapter reuses. The `_` arms on the instruction enum are in the previous section.
  - **The roles a run's delegated contexts ran as are recorded, within the run and durably.** New
    `agent_type` values reach the session pointer and append-only `roles` table independently of claim
    decisions, including `Outside`; `estigia roles` reads by run/issue. Model/effort are declarations.
    This is neither a count nor proof of isolation or a panel: absent host metadata leaves no record.
    Role-gate refusals are unrecorded; claim-gate refusals are recorded. The default reviewer's
    `Read, Grep, Glob` never wake the matcher. Built-binary measurement: `review-blind` `Bash` under
    `reading` denies without a pointer; under `measuring` records `roles: ["review-blind"]`.
    A new role on a denied call can also persist a verification watermark, letting the next routine
    write ride renewal, and create `<runtime>-unknown` for a blank session id, not removed by session
    end. `session::store` drops stale revisions, so concurrent tool writes can lose pointer roles;
    the append-only log avoids that race. No test establishes complete host role reporting.
  - **`Run.inline_writes` (`src/harness/session.rs`) is only a within-run record.** It adds a target
    (path or shell fragment/verb/tool) on an allowed call with missing, not blank, `agent_type`, only
    after a role was seen. Hosts omitting metadata, runs never delegating, earlier inline writes and
    role-refused calls produce nothing, not a floor. `Outside` writes are excluded; unplaceable and
    relative paths count inside. No model identity is established and no inline write is refused:
    the gate cannot judge "non-trivial"; a changed-file proxy over-fired on this history.

  - **A write to `.git/estigia/estigia.local.md` is a boundary whichever row it names.**
    `crate::config::rows` reads regions: changing a newline, fence or preserved tail can change guarded
    rows without naming them. `is_repository_configuration` therefore guards the whole file on the
    write-tool road; shell writes and `chmod`/`chattr`/`chown` use the control-surface boundary checks.
    `REPOSITORY_SHELL` also asks these checks (`git commit -m wip && printf x >>
    .git/estigia/estigia.local.md` is `Boundary`); `IRREVERSIBLE` still precedes it.
    **Over-gating:** even an ordinary row such as the retired fixture string `Repair ceiling` costs a
    tracker read. `estigia config set --repo` is unaffected; allowed scopes are in
    [configuration.md](configuration.md), *Which root the gate decides in*.
    **Under-gating:** `rm -rf .git/estigia` and `mv .git/estigia .git/estigia.bak` remain `Routine`;
    neither the configuration-file spelling nor a control-surface fragment names the bare directory.

    **Mentioning a surface also pays.** Whole-line matching makes `git commit -m update CLAUDE.md`
    and a message naming `.claude/settings.json` require a live read. `CONTROL_SURFACE` has twelve
    separator-free entries, seven instruction files (`claude.md`, `claude.local.md`, `agents.md`,
    `agents.local.md`, `gemini.md`, `qwen.md`, `crush.md`). This is the fail-closed asymmetry of
    `WRITES_A_FILE_POPULATION`; the frequency of such ordinary commits is unmeasured. Debug timings:
    `git stash` 4.6→29 µs, a long `-`/`~` message 39→470 µs, against roughly 75 ms end to end for
    `estigia gate Bash`. The suffix ladder is unbounded; an 832-byte `rm` operand cost 1.70 ms.

  - Estigia still cannot prove a Judgment Day ran, how many contexts it had, or that any two of them were
    given separate directories: this narrows what a judge **can** do and proves nothing about what a judge
    **did**.
  - **A reviewer's directory has no remover, and Estigia is not one.** Each Judgment Day round mints one
    directory per judge, but Estigia does not spawn the judge and `release` puts down only the checkout
    `start_branch` created, so they accumulate until an operator sweeps them by hand, after the judge has
    returned its findings and only once `git status --porcelain` is empty and every commit is on the remote.
    They live under the configured `Worktree location`, which makes them look like registered worktrees when
    Git considers them no such thing (a separate root is an open question). `estigia prune` lists a detached
    checkout no branch owns but cannot tell a judge's directory from any other orphan: a name to check, not a
    disposal.
  - **What a delivery's remote deletion cannot see.** `release` deletes every remote head the run recorded
    starting, read back through `ls-remote`, but the record is only what `start_branch` wrote: a branch pushed
    by hand, or under an identity the issue's timeline never named, is not in the set, and GitHub's
    `delete_branch_on_merge` covers the pull request's own head only. `estigia prune` catches it later, and
    only when the operator confirms.
  - **What the prune sweep cannot see.** It attributes a head or checkout to an issue by the `issue-<n>-…`
    convention in a branch name or by a run pointer, so a head naming no issue and carrying no pointer is
    listed only when merged into the base, while a checkout is always listed, kept when nothing says it is
    finished. The remote-gone orphan route recognises a clean checkout whose commits remain on any refreshed
    remote ref even when the branch is neither merged nor closed
    ([what-it-writes.md](what-it-writes.md#explicit-orphan-pruning);
    `a_remote_gone_checkout_reachable_from_another_remote_is_removed_only_on_confirmation`). The legacy route's
    "a live claim holds it" is a readable run pointer whose issue the tracker still reports open, not a re-read
    of the claim (a pointer left behind reads as live, a claim the pointer never recorded as unheld, and
    `held_worktree` stops protecting a readable one when its issue reads closed). The orphan route keeps every
    matching pointer and every checkout named by a registry session not explicitly ended, refuses unreadable
    ownership, and rereads both at confirmation with the exact commit inspected as retirement's expected HEAD.
    These are observations, not a lock: a claim with no pointer and no session directory cannot be attributed,
    and another process can acquire ownership, recreate the upstream or change Git state between the last read
    and `git worktree remove`.
  - **The sweep's remote deletion bypasses the push guard, and that is a bypass.** `estigia prune --confirm`
    runs `git push --no-verify --delete` for each head, because the guard refuses a push no live claim
    authorises and the sweep holds none. A deletion is not work entering the flow and `--confirm` is the
    authorisation, passed by the operator or by an agent (finished cleanup is the run's to execute), but
    `--no-verify` is the rail the guard's own script names, so anything that can run `estigia prune` can
    delete a remote head without a claim. The listing before the deletion, and the rule that a live claim's
    head or a checkout with unpublished commits is never marked, bound it.
  - `Evidence standard` is one row for one shipped reviewer definition, not per role: the delegated roles
    carry fixed grants, and a repository wanting a different grant for one has no way to say so.

## Skips, deletions and the bounds of a round

- **A skip spelled as a pass: the remaining skip sites, and the routes the rig guard does not hold.**
  `tracker_rig`, `repository()` in `src/harness/guard/tests.rs`, `repository_with_commit` in
  `src/test_env.rs`, `git_repo()` in `src/harness/doctor.rs` and `release_repo()` in `src/cli/tests.rs`
  return the directory (or rig) and panic, naming the failing command, when `git` is absent or any call
  fails, so a machine without git fails those tests loudly instead of reporting them `ok`. **The remaining
  skip sites:** seven in the `pipe` suite itself, across five test functions, at the read-only-file
  capability and at git usability, left because raising or `#[ignore]` is a decision (`#[ignore]` is
  reported as *ignored*, a `return;` after an `eprintln!` as *passed*).
  `a_row_that_is_broken_comes_out_of_the_report_broken` prints its skip and **does not return**, dropping
  two assertions and reporting `ok`. The read-only capability is a genuine machine property and the git
  sites are not, so one policy cannot close both. A legitimate skip is one the machine could not have run:
  symlink privilege, `mklink /D`, administrative shares, drive letters (the ten skip sites in
  `src/harness/tests.rs`) and build-state properties (no commit, a dirty tree, a shallow clone, no scratch
  directory). `git` is not that: a machine without git cannot measure the push guard **at all**.

  **The guard that holds the rig catches an accident, not an author.** Reviewers walked past
  `the_tracker_rig_cannot_answer_that_it_did_not_run` twenty-three times; eleven routes are held and twelve
  are not (counted from the table). Each attempt was measured with the fixture removed, except those about
  *where* the fixture is looked for (measured with it present); `106 passed` is the defect reproduced whole
  with every gate green.

  | Route | Outcome | Held now |
  |---|---|---|
  | `type TrackerRigMaybe = Option<TrackerRig>` in the signature | 106 passed | yes |
  | a caller split over two lines | 106 passed | yes |
  | `if … { return; }` before the call | 106 passed | yes |
  | the fixture located from `CARGO_MANIFEST_DIR` again | whole suite green | yes |
  | `return Default::default();` | 106 passed | yes |
  | `std::process::exit(0)` in the rig | **no result line at all**, exit 0 | yes |
  | a decoy comment *naming* the rig | unchanged alone; pairs with an exit | yes |
  | a `//` comment carrying the **whole signature line** | whole suite green | yes |
  | a `/* */` block putting the signature at column zero | 106 passed | yes |
  | `process::exit` in a caller, or in a helper below the rig | no result line, exit 0 | yes |
  | `use std::process::exit as leave;` | no result line, exit 0 | yes |
  | `use std::process as sys;` then `sys::exit(0)` | no result line, exit 0 | **no** |
  | `use std::process::{exit as leave};` | no result line, exit 0 | **no** |
  | a decoy `current_exe()` call, the real lookup in a helper | whole suite green | **no** |
  | a macro expanding to `return`, defined before the first `#[test]` | 106 passed | **no** |
  | `'rig: { … break 'rig; }` around each body | 106 passed | **no** |
  | `#[ignore]` on all sixteen | 90 passed, 16 ignored | **no** |
  | `#[should_panic]` on all sixteen | 106 passed — the rig's own panic is swallowed | **no** |
  | `#[cfg(feature = "rig-tests")]` on all sixteen | 90 passed, **0 ignored** — no trace at all | **no** |
  | a block decoy plus the real definition written `pub fn` | whole suite green | **no** |
  | a `}` at column zero inside a string, truncating the body | whole suite green | **no** |
  | a second `tests/*.rs` with its own `Option` rig | its own suite green | **no** |
  | a body skip — `if built { … } else { eprintln!(…) }`, no return | 106 passed | **no** |

  The filter is `trim_start().starts_with("fn tracker_rig(")`, so a real definition written `pub fn` is
  invisible to it and a decoy becomes the unique match (measured: the suite and the guard stay green), and the
  body ends at the first line that trims to `}`, which a multi-line string literal can supply from column
  zero; both are one more needle away from being caught and not chased, because recognising a Rust item is
  parsing it. The file-wide `process::exit` scan catches `use std::process::exit as leave;` because that line
  contains the substring, and not the two other spellings; a macro, a labelled `break` and a body skip evade
  any substring. **What actually holds the accidental path is the compiler**: `let Some(rig) = tracker_rig()`
  does not compile against a non-`Option` (`error[E0308]`), so nobody reintroduces this by copying a
  neighbour. The `return` scan reads whole lines, string literals included (a version that cut strings out
  was defeated by `let _sep = '"'; if !built { return; }`), and its two false positives (a `return` in a
  string literal, and one inside a **closure**) are declared in the failure message.

  One more control-surface path of the hosts file's shape, not closed: `<checkout>/.git/config` answers
  `Routine` while `<checkout>/.git/hooks/pre-push` answers `Boundary`, and `core.hooksPath` in that file
  decides whether the push guard runs at all. Normally harmless, since `covered()` yields both `repo_dir`
  and the worktree; with a pointer carrying only a worktree (`repo_dir = <project>/worktrees/wt-a`) writes
  to `<project>/src/x.rs` **and** `<project>/.git/config` both stand aside. `start_branch` now records the
  claim's checkout beside the worktree, so Estigia's own tools no longer *produce* such a pointer; it stands
  for a run whose `repo_dir` genuinely is a worktree or a hand-written record. Whether the fix is naming the
  file or widening what a worktree run covers is open.

- **A deleted comment is missing evidence, never satisfied evidence.** The verdict requirement does not
  appear only once a handoff exists (else deleting the handoff comment would lower the bar from *a computed
  verdict approved these bytes* to *nothing*); delivery asks for the same computed `approved` on every
  route. The one exception is the `low` tier, which licenses no lens and needs no marker: a deleted finding
  reads as absent and the receipt recomputes as `approved`, and a deleted lens report reads as a lens that
  never ran at `medium` and `high`. A deletion can still **restore** something, but only with a `gh` call
  outside Estigia (no operation edits or deletes a comment): deleting a handoff comment lifts the requester
  exclusion it carries, returning the publishing run to the queue for work no verdict covers (liveness, not
  integrity); deleting a `rejected` `review-verdict` marker lifts the `legacy-verdict-rejected` refusal over
  those bytes, the one deletion that clears a release (the ledger recomputes `approved` over the same
  receipt, so a lost objection rather than an unreviewed merge).
- **The draft/ready CI barrier is cooperative, not adversarial.** Compatible repositories start PR CI on
  `ready_for_review`, not topic push/open/synchronize/reopen, and carry a `workflow_dispatch`
  **publication lane** that `publish_review` starts once per publication epoch against the head it pushed
  (not once per push; dispatching does not mark the PR ready). GitHub has no atomic conditional-ready
  operation. Collaborators or repository workflows outside Estigia can mark ready, push, trigger other CI or
  forge comments; Estigia neither parses arbitrary consumer YAML with weak substring checks nor claims
  malicious-collaborator authenticity.

- **Corrections per lineage are enforced where a correction is published, and counted from markers anyone
  who can write a comment can also delete.** The `Corrections per lineage` row (default 3) counts an
  original candidate and its repairs on one pull request, and the publish call refuses the correction past
  the limit (`correction-already-spent`) before it pushes. Nothing is stored: `repair_rounds` derives the
  count from the `parent` receipts on the timeline, so a deleted `published` or verdict comment lowers it
  and a forged one raises it. It does not read: **a correction nobody published** (an attempt that never
  reached `publish_review` spends nothing, and a republish over a parent no panel has judged is not a
  correction, so a run can republish as often as it likes before a verdict lands, bounded by the size
  budget and scope admission); **a new pull request** (the lineage boundary: a chained slice or replacement
  begins afresh, and nothing tells a new change from the same one republished under a new branch name); **a
  forced base advance** (the integration is the lineage's correction when it is the first, refused when the
  parent already was one; how often the base moves is not measured); **a republish over an approved
  receipt** (zeroes the count, spends no correction, still bounded by budget and scope and refusable
  `correction-outside-reviewed-scope`; a warning- or suggestion-cleared one is narrowed to one lens and no
  judge by `review_tier_for_repair`); and **the escalation that follows** (`escalate_review` opens when
  `repair_rounds` answers `correction_spent`, whether or not the publication was refused, and nothing reads
  whether the person escalating had cause). Nothing refuses a panel launched over a spent lineage or
  requires `repair_rounds` to be called, and Estigia cannot prove a panel ran, so it cannot prove a
  correction was reviewed; delivery still requires an approved computed verdict. Judges are never told the
  count. `Repair ceiling` is retired and an installed row is reported `repair-ceiling-retired`.

- **A validator result records an answer the binary was handed, not proof the validator read the trees.**
  A hand `record_validator_result` is refused `recorded-by-capture-only`. It stores that a read-only
  validator answered `pass` or `fail` for `original_criteria` and `correction_regression` over one
  correction, written by the run holding the `review` claim because a sealed validator holds no tools;
  Estigia cannot see which diff or findings it was handed, whether it inspected the frozen trees, or
  whether `unavailable` was honest. A conclusive result is recorded once, a pass never overrides an open
  severe finding, a failure once the lineage has spent its `Corrections per lineage` or a third
  `unavailable` writes the `escalated` receipt `release_ci` refuses, and a failure with a correction left
  computes `rejected`. Limits: the validator is required **in addition to** the tier's lenses
  (`docs/review-cost.md`); nothing sets the attempt number except the caller (a skipped number is refused,
  but a run that never records `unavailable` is never escalated by it); `budget_exceeded` is not a cause;
  `escalated-to` is the word `maintainer`, not a person; and a receipt's standing does not fall back to an
  empty publisher when the timeline cannot be read, while a pull request's first publication records no
  parent.

- **A recovery is a person's command, and what the gate can prove about that is narrow.** `estigia
  recover` is a CLI command with no MCP tool; the gate denies an agent's shell line for it
  (`recover-is-for-a-person`, any host), decided by `harness::person_only` before any claim, stand-down or
  role road, and the named login must hold the `admin` or `maintain` role on GitHub's
  `collaborators/{login}/permission` read (`role_name`). **What keeps an agent out is the gate's shell
  match, not the login check**: the login is not authenticated, so anyone with shell access and a gh token
  can name another maintainer. The Bash rule matches the words `estigia recover` wherever they stand, on the
  line and a dequoted copy, so a commit message that merely names both is refused with no stand-down escape,
  and an alias or wrapper that never writes them as two words is not matched. `!` in Claude Code and the
  person's own terminal never pass through the agent's tools. The record is local, so a lost store loses the
  recovery, and a publication carrying a recovered-from field with no readable record is refused
  `recovery-record-missing`. The inherited tier is a floor taken when the recovery is recorded; `review_tier`
  and `review_tier_for_repair` read only base and head and do not apply it, so a successor's advertised lens
  count can be smaller than its verdict will demand. A half-finished review that never escalated is not
  covered. The marker is a timeline entry like every other, so a comment synced under the same identity can
  carry it. A correction on a new pull request over a rejected parent on another is a new change and owes
  no validator.

- **150,000 is an inherited comparison target, not a number derived from the measured distribution.**
  `Review budget` defaults to 150,000 tokens **per context** (a lens or a judge), not per panel or issue,
  and accepts `none`. The figure was inherited from an external project; at its `v4.0.0` that project's only
  `150000` is the parent-context length at which a long session is advised to delegate, a different quantity
  from a per-judge spend. The distribution supplies a check, not a derivation: twenty-eight judges across
  five issues, from harness completion reports, put the minimum at 96,073, the median at 127,025 and the
  maximum at 182,136 tokens, and 150,000 sits above that median while 18% (5 of 28) still ran over it.
  `record_spend` records a panel total beside a `contexts` count (a convention; validation refuses only a
  zero). The row is **advisory**: nothing refuses a panel whose recorded tokens exceed it, and
  `record_spend` is a declaration, not a measurement (an under-report is undetectable). **None of the
  twenty-eight figures include the orchestrating run itself**; the delivery step records one `record_spend
  --phase orchestrator` before `transition --to done` (`docs/review-cost.md`), which closes the *channel*
  and not the *trust* gap. `skill/policies/judgment-day.md` and `skill/bindings/github/operations.md` name
  the row, and nothing in the always-routed contract reads or reports the figure, so naming it there would
  describe an action no run takes.

## The correction budget and the publication lane

- **The correction budget is a genesis tree pair, held and never recomputed, and it has narrower
  guarantees than the machine it ports.** `publish_review`/`republish_review` measure a repair between two
  frozen Git tree object ids (`git rev-parse <rev>^{tree}`, `git diff --numstat` between two trees), never
  a payload and never a mutable ref. The parent is **selected, not validated**: `publish_with` picks the
  latest publication on *this pull request* rather than the issue's newest, and `repair_rounds` scopes the
  same way and shares whatever this leaves open. Five things this port does not prove:
  - **A first publication is never budgeted.** Nothing freezes it against, so `budget-candidate-tree` is
    written alone and a candidate of any size publishes.
  - **A base advance does not reset the budget, and what replaced the reset is measured against a tree
    nobody authored.** When the base advanced past the parent's (`base_advanced_past`) and the branch
    integrated it, the round is measured from `git merge-tree --write-tree <parent head> <new base>` to
    this head's tree; the genesis, budget and cumulative carry over (`budget-parent=integrated`), and the
    conflicted paths and the advance's rename destinations are admitted beside the genesis paths.
    `budget-refrozen=base-advanced` remains for the one shape that re-freezes deliberately: the fork point
    is not in the parent's history and the base was replaced rather than advanced. Open: **conflict
    resolution is charged as authored lines** (a conflict-heavy integration can consume the budget on lines
    the author did not choose to write); **only conflicted paths and rename destinations are admitted**
    (an edit to a cleanly merged path is a new change, refused for scope, reporting `genesis_paths` and
    `integrated_paths` separately); **`git merge-tree --write-tree` needs git 2.38** and a failure fails
    the publication as a failed read with nothing pushed and no fallback (the fixture pins an unresolvable
    ref, not an old git, which this repository's CI does not run); and an unmerged advance never reaches
    the check, because `publication-base-not-ancestor` refuses it first, naming merge or rebase outside a
    correction and only the merge during one (a rebase rewrites the parent head and the validator refuses
    `validator-delta-carries-upstream`), at the cost of two reads (pull request listing and timeline).
  - **The forecast is not measured, only the actual.** The source project estimates a correction's size
    before it runs; every figure here is measured afterwards from two trees that exist.
  - **There is no compare-and-set and no lock**, because an issue timeline is append-only. Two publications
    racing one lineage can read the same parent marker and both post; the second marker still carries a
    correct measured total, but the single-round rule a lock would make load-bearing is advisory.
  - **The companion-test extension table is this port's own addition, and it admits more than its prose
    discloses.** The source has no row for Rust, because Rust tests are separated by build configuration
    rather than by a name, and names the hazard: admitted shapes "compile into the shipped artifact", so a
    correction can ship unreviewed production code inside an approved candidate.
    `correction_companion_directory("tests", ".")` is true and this repository's genesis directory is `.`
    on nearly every candidate (`CHANGELOG.md` moves with most documented changes), so for any candidate
    touching a root-level file the entire top-level `tests/` tree is admissible on every repair. The mirror
    image: `correction_test_file_name("src/transport/target/tests.rs")` is false, so this repository's
    dominant unit-test location is admissible only when the candidate already touched that file. What
    bounds it: admitted lines are charged against the budget and every republish is re-reviewed in full.

- **Bounding the frozen manifest bounds the comment it lands in, by measuring the comment.** The publish
  call refuses `genesis-manifest-too-large` when `budget-genesis-paths`, percent-encoded, would clear
  `GENESIS_MANIFEST_CEILING` (32,768 bytes). `topology`, `topology_reason` and `run-id` are caller-supplied
  with no length limit of their own (the MCP argument definitions declare a type and a description, never a
  maximum), so `publication-comment-too-large` fires from `publish_with_sleep` after trimming and before
  the fetch when the caller's marker fields plus `topology_line` would exceed `COMMENT_BODY_CEILING`
  (65,536) minus `PUBLICATION_FIXED_OVERHEAD` (8,192, an upper bound on the prose template and every
  generated field other than the manifest's and the lane's, derived by enumerating the fields `published`
  writes) minus `GENESIS_MANIFEST_CEILING` minus the lane's measured width (`lane_comment_bytes`). Every
  bound has to be reserved or they do not compose, and the reservations are conservative (the manifest
  ceiling is charged even on a first publication) because a comment GitHub rejects leaves the branch pushed
  with no receipt. Nothing is pushed on refusal
  (`an_oversize_topology_is_refused_before_any_remote_write`, `tests/pipe/part_10.rs`). Measured in the
  percent-encoded quantity: the largest manifest across the 109 merge commits then in this history encoded
  to 1,452 characters over 64 paths, a second larger repository to 13,095 over 195 (4% and 40% of the
  ceiling); the encoded-to-raw ratio on real path sets is 1.03 to 1.10, while bytes outside the marker's
  safe set would sit at the 3x bound, which is why the check measures.

- **The budget's own tree diff runs isolated from every `.gitattributes`.** A local `.git/info/attributes`
  carrying `* -diff` marks every path binary, which `count_changed_lines` skips, and this repository's
  committed `.gitattributes` already marks `*.png`, `*.jpg`, `*.gif` and `*.ico` `binary`, so one more line
  would silence the budget. `diff_stats`'s `DiffIsolation::Isolated` reaches the child through
  `run_with_isolated_env`, which **replaces** the environment (a fresh `GIT_DIR` with no worktree, index or
  attribute source, and no `HOME` or `XDG_CONFIG_HOME`) rather than layering onto the caller's as
  `run_with_env` does, so committed and untracked attributes and the caller's global or system git config
  and default global attributes file are invisible (measured: `* -diff` in the default global attributes
  file made a two-line change count `-\t-` under layered isolation). Binary detection still runs on content.
  `review_tier`'s own `diff_stats` call stays ambient, and since a `low` answer lets `release_ci` release on
  the structural readback, that read moves a clearance and is out of this entry's scope.
  `transport::tests::essential_env_drops_home_and_keeps_path_case_insensitively` tests the filter and
  `tier::tests::a_default_global_attributes_file_does_not_reach_the_isolated_diff` the composed path (its rule
  is narrowed to one unused name because `* -diff` poisoned concurrent tests). **Deleting
  `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_COUNT`, `GIT_ATTR_NOSYSTEM`
  and `LANG` from `tier.rs` leaves the whole suite green.** `GIT_CONFIG_NOSYSTEM` and `GIT_ATTR_NOSYSTEM`
  alone stand between the isolated diff and a real system-wide install (proving it would mean writing to a
  shared, privileged path); the other four are redundant by construction (the replacement already drops what
  they intercepted, and `LC_ALL=C` shadows `LANG=C`). The source's `--text` flag belongs to an `Inspect` this
  port does not implement; the function `diff_stats` mirrors carries no `--text`, so a binary-content path
  is charged zero lines by the source too.

- **The review payload a judge is handed runs isolated the same way, and that moves a digest wherever a
  repository's own config sets a diff knob.** `review_payload_bounded` (`src/transport/target.rs`) builds
  the unified diff every judge reads through `isolated_diff_env` and `run_with_isolated_env`. Without it an
  attribute source marked every path binary and the payload shrank to one `Binary files differ` line per
  path, small enough that `REVIEW_PAYLOAD_CEILING` stopped binding. Isolation makes the payload independent
  of ambient diff configuration too, a behaviour change: `diff.algorithm`, `diff.interHunkContext` and
  `diff.indentHeuristic` are not attribute sources and none is pinned by `pinned_diff_args`, so for a
  repository whose own config sets one the bytes move once and an earlier `payload-digest` will not
  reproduce (measured: two one-line edits ten lines apart give 23 lines under an ambient
  `diff.interHunkContext=10` and 20 isolated;
  `a_local_diff_inter_hunk_context_moves_the_ambient_payload_but_not_the_isolated_one`). **No digest already
  on this timeline moved**, a fact about one machine on the day measured (no `diff.*` key locally or
  globally, the system `diff.astextplain.textconv` never invoked under `--no-textconv`,
  `core.attributesFile` and `core.bigFileThreshold` unset, no untracked `.git/info/attributes` or default
  global attributes file, and the committed `binary` marking renders the line content sniffing already
  produces); a fork or a contributor's machine could set something none of that measured.

- **Reading the lane before launching a panel is instruction, not enforcement.** `publication_lane`
  answers green, red, never-ran, running, absent or disabled (an unreadable listing or permissions answer is
  a failed read), and Step 5 routes on all of them. Nothing refuses a panel launched over a red lane,
  because a sub-agent launch is not a write the gate sees. `record_computed_verdict` still refuses an
  approval on a lane whose run genuinely ran and failed or is unfinished; the one case it does not refuse is
  a lane whose run never executed a step, gated on the caller's own evidence rather than on green.

- **Estigia starts the publication lane and cannot prove a repository has one.** The refusal in
  `record_computed_verdict` is real, but everything upstream is best-effort. `publish_review` discovers
  every active workflow under `.github/workflows/` (or takes `Publication lanes`) and dispatches each once
  per publication epoch. It **does not parse consumer YAML** for `workflow_dispatch`: a candidate without
  the trigger is told apart only by `HTTP 422` and dropped from the started set, not refused. Estigia sees
  which workflows exist and accepted the dispatch, never which a repository considers load-bearing, so a
  release lane or nightly is excluded only when named on `Publication lanes exclude`. A repository with no
  dispatchable candidate gets no check runs, and **a head with no check runs proceeds**, deliberately:
  refusing would break every consumer that has not adopted the lane. If the workflows listing cannot be
  read, `ci.yml` is tried, which is not a claim that every lane is called that. Eight narrower limits:
  - **The dispatch names a branch, not a commit.** A push over the branch between Estigia's push and its
    dispatch starts a lane on someone else's bytes; the published head then has no check runs and clears.
  - **`HTTP 404` is read as *no lane*, not *no permission*.** GitHub answers `404` for both, so a token
    that cannot see the workflow loses the protection silently; only `401` and `403` refuse
    (`publication-lane-forbidden`).
  - **Only `accepted` is gated.** A rejection over a red head records. `neutral` and `skipped` clear; every
    other conclusion, including none, refuses.
  - **More than one page of check runs (or of jobs, in `never_ran_signal`) is a failed read.** Both are
    asked at `per_page=100` and refused when `total_count` outruns what arrived, so a head with more check
    runs cannot record an accepted verdict until the read is widened (the jobs read answers `None`, which
    `classify_reds` reads as `Ran`). Fail-closed and untested at that size.
  - **The read side is fail-closed on a permission error, where the dispatch side is lenient.** A token
    that can push, comment and dispatch but lacks `Checks: read` publishes normally and can never record an
    approved computed verdict: `read-failed`, permanently. Its **detail** names the read, but the shared
    envelope still ends *retry the read*, advice no retry can take. Nothing detects the shape.
  - **A dispatched run is not visible the instant the dispatch returns**, so a verdict recorded inside that
    window sees an empty listing, and an empty listing proceeds.
  - **The listing is every check run on the head, not the lane's.** A coverage bot or scanner sitting
    `queued` refuses an accepted verdict naming a lane the reviewer cannot start, watch or clear.

  A dispatched run's check runs **do not** satisfy branch protection's required contexts on a pull request
  head. Measured on this repository (2026-08-27, head `ead3ca84`): `gh workflow run ci.yml --ref <branch>`
  completed success with three check runs carrying the required context names, yet `gh pr checks 110`
  answered `no checks reported`, `statusCheckRollup` was `[]` and `mergeStateStatus` stayed `BLOCKED`;
  toggling draft to ready produced a run with `event: pull_request` and the rollup went from 0 to 3. The
  lane is the cheapest disqualifying signal **in the Actions tab**, where `record_computed_verdict` reads
  it, not a claim about mergeability. The draft barrier still closes the merge hole: `release_ci` verifies a
  draft before marking ready, and GitHub refuses to merge a draft.

- **A marker field that is present but unreadable is refused by name rather than read as an absent one.**
  `ReviewTransaction::from_marker` and `CorrectionState::from_marker` answer absent, present-and-valid or
  present-but-unreadable (`published-transaction-unparseable`, `correction-state-unparseable`, naming the
  `epoch` and first offending `field`), before anything remote moves, and every reader of the lineage
  refuses rather than proceeding over a dropped publication. **The absent case is the designed migration
  path**: a marker with none of the budget fields reads `Ok(None)`; only one carrying *some* and failing to
  read them all is refused. Reachability is the weak part: markers are written and read by Estigia, `comment`
  escapes control-shaped prose and `all_publications` filters to self-authored, unedited comments, so **no
  malformed marker has been observed**. The two ways in are a partially written marker (the unbounded
  `budget-genesis-paths` failure lands after the push) and version skew between a swapped binary and an MCP
  server still holding the old executable. How often a base advance occurs during a review round is not
  answered (an audit counted 7 of 270 publications whose base was not an ancestor of the head: a floor for
  movement, not a measure).

## Setup, configuration and uninstall

- **The superseded-reviewer ledger is crossed against the templates beside it, and not against a freeze that
  never happened.** `REVIEWER_TEMPLATES_SUPERSEDED` (`src/setup/mod.rs`) lists every previous
  `review-blind.md` and lens body so an install carrying an older definition upgrades instead of being
  refused as foreign. `every_frozen_reviewer_template_is_a_member_of_the_superseded_ledger` requires every
  `include_str!` const to be a ledger member and vice versa, and every `.md` file in `src/setup/` to be
  declared. **It cannot see a freeze that never happened:** an author who edits the current template and
  updates the `replacen` chain `the_default_rendering_is_the_bytes_already_installed` walks, but never adds the
  previous body as an `include_str!` const, satisfies the sibling guard, leaves the ledger one body short, and
  no guard holds the chain against the list.
- **`status` names why the tools half is off, and leaves two of the four states together.** `tools off`
  covers no verified registration site, a site known and not written yet, a file that cannot be read or
  parsed, and an entry under a key this build does not look under. `AgentAdapter::tools_gap()` names the first
  and an adapter that can register but has not prints `run: estigia setup <agent>`; **the last two still print
  that same command**, because `exposes_tools` answers `false` and `tools_command` returns `None` for an
  unreadable file, an unparseable one and an unrecognised key alike.
- **The two readers of the configuration table differ in one place.** The binding looks a setting up by
  **prefix** (`cfg` returns the first key that `startswith` the label) and this crate compares whole words,
  trying the exact match first (`a_row_the_binding_acts_on_is_a_row_this_reader_finds`). `Context::live` reads
  *this crate's own rendering* of the layered configuration, so a label this crate forgives (a doubled space,
  backticks) reaches the transport as the value read out of it
  (`a_label_this_crate_forgives_is_one_the_transport_reads_too`). What is left is `get` answering with the
  **first** row whose label the question is a prefix of, so `| Worktree location (mine) |` still answers
  `worktree location` (written on `Context::get`).
- **An override row is matched by its label, and a label no setting resolves is reported rather than dropped
  in silence.** `estigia.local.md` is the operator's own file and `setup` never regenerates it, so a row outlives
  the setting it named; `Setting::from_label` answers `None` for a deleted setting and a typo alike. `doctor`'s
  `contract` row names the file and those labels (`skill::unknown_override_rows`, through `Contract::unknown`),
  never the values (permissions and machine paths). It is a report, not a refusal: the row is dropped from the
  effective table and no command fails; a file that cannot be opened is refused by name by `local_overrides`,
  since the values underneath would run on settings nobody chose. It cannot tell a typo from a deleted setting.
- **An unmarked document is read whole, and a prose table in it is offered as configuration.** Nothing comes
  of that only because no prose row's first cell spells a setting (the reader's own comment calls it *luck,
  not a property*). `no_prose_table_in_the_contract_spells_a_setting` holds the shipped contract to it: with
  its closing marker removed, 8 configuration rows become 25, the extra 17 being prose
  (`Situation | Action`, `Load | When`). `a_document_with_no_usable_block_is_read_the_same_by_both` puts six
  shapes of unmarked and half-written document to both readers, with a floor failing if the transport reads
  nothing anywhere.
- **An uninstall takes an empty `hooks` wrapper it may not have written.** When Estigia's entries leave a
  settings file the now-empty `hooks` object goes with them. An operator who wrote `"hooks": {}` themselves
  loses it (measured: `estigia setup cursor --uninstall` on a machine where nothing was ever installed rewrites
  that file and reports `update`). Telling the two apart needs a record of what this crate created, which
  exists for the instruction file (`created_outside`) and not for this;
  `an_uninstall_leaves_no_empty_husk_of_its_own_making` asserts the decidable half.
- **Fifty-three refusals the transport can give are named in no prose an agent reads.**
  `every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer` reads `TRANSPORT_VOCABULARY`
  (the vocabulary `build.rs` generates from all three spellings a refusal can take), one entry per code, and
  `UNDOCUMENTED_REFUSALS` in `tests/honesty.rs` is the exact set no prose names. The check is a ratchet: a
  **new** undocumented refusal is red on the review that adds it, an entry that stops being undocumented must
  be taken off, and the list can only shrink. Each is a real gap (`lost-claim-race` among them): the
  instructions live only in the `action` field of the envelope, and the binding's source is not something an
  agent reads. Closing it means writing them into `skill/references/`. The separate build-time measurement of
  refusals that name no way out is under *Refusals, and the way out they name*.
- **A read-back that agrees after waiting may agree for the wrong reason, and nothing here can tell.** Ten
  spaced attempts cover about ninety seconds because the pull request object lags the branch ref (45 and 120
  seconds on the two publications measured), and the refusal says which world the last read stood in (the ref
  carrying the pushed head means wait, a moved head means investigate, an unreadable ref means it cannot
  tell). It cannot see a foreign push landing inside the window: a run that pushes, waits through lag and
  reads agreement may be reading a pull request that settled onto a head it did not push, and the receipt
  binds that head either way.
- **An exit-2 refusal with no `action` is still sent to `estigia config list`, and nothing checks that the
  command clears it.** `translate`'s code-2 arm carries the transport's own `action` for every reason the
  table does not answer; the fallback repeats the envelope's scalar-valued fields beside the ones the refusal
  is built from (`an_envelope_field_that_is_not_scalar_is_dropped_and_a_scalar_is_repeated`) and runs for
  **every** code whose envelope carries no `detail`. What is left is the last resort: a
  `Failure::ConfigDefect` or code-2 `Failure::Stop` without an `action` or a `malformed_argument` entry
  (`invalid-finding-class`, `missing-argument`, `incomplete-review-transaction` and their neighbours, none of
  which the configuration has any part in) reaches the caller as `run: estigia config list`, a dead end that
  only a per-reason decision can replace.
- **Skill directories are verified for four entries** (`claude-code`, `codex`, `deepseek-harness` and the
  neutral `~/.agents` root); the other adapters (OpenCode, Pi, Cursor, ZCode) have an unverified skill
  directory, so their skill lands in the shared root and the directive names the path, which `estigia setup`
  says on the line where it matters.
- **`estigia update` reports local state only and verifies nothing remote.** It reports local recorded
  provenance and release high-water, makes no network request, and reports the latest public release as not
  verifiable without claiming currency from absence. The JSON is not authenticated against a malicious
  same-user writer; official installers publish candidate-derived local records before replacement, but
  publication and copy are not atomic and concurrent installers stay unserialized. There is no remote update,
  self-upgrade, rollback, or mutating public upgrade command.
- **The seam tests check the payload against itself**, not against an agent that read it: every markdown file
  the payload links to is shipped, and every shipped runtime companion is reachable from another shipped file,
  which cannot prove an agent opened or obeyed any document.

## The gate's reach across the agents

- **`Decision mode: auto` is enforced on Claude Code alone.** The gate refuses `AskUserQuestion`, the one
  question tool its matcher is woken for, and records the recommended answer from the payload. Codex,
  OpenCode and Pi register no question tool with the gate, so on them `auto` is the contract's sentence
  and nothing more, and on every agent a question asked in prose rather than through the tool is not seen.
  "Recommended" is the option the agent itself labelled `(Recommended)`, or its first option: the gate reads
  which answer the agent ranked first and does not judge it. Measured by
  `src/harness/decision_mode/tests.rs`; turning the refusal off reddens two of its six tests.

- **The harness gates named populations, not every write.** The editing tools each agent ships, the git
  spellings that change the tree, the irreversible steps, and the shell constructs that visibly write a
  file: a redirect, a `sed -i` (a `curl -o` only when some output is not `/dev/null`, `/dev/stdout` or
  `/dev/stderr`), an interpreter handed code, the utilities that copy, move or delete (also
  under fifteen prefixes that run them: `sudo`, `flock`, `timeout`, …), and any command with a **long write
  flag** (`--write`, `--in-place`, `--fix`). The command a segment runs is resolved first
  (`shell::candidate_commands`: `if`, `then`, `else`, `elif`, `do`, `while`, `until`, `!`, `{` and
  `NAME=value` are skipped; behind a `RUNS_ANOTHER_COMMAND` wrapper every later word is a head of its own).
  **Not matched:** a write that shows nothing (an `eval`, a wrapper *script*, a merge reached by an alias or
  `gh api`), a formatter whose default is to rewrite (`cargo fmt`, `black .`, `gofmt -w`; reading short flags
  everywhere would report a search as a write), the keywords `case` and `select`, and a quoted command word
  except as below.

  **`git pull` is an `IRREVERSIBLE` boundary and deliberately not a `DELIVERS` one.** It runs a merge
  internally (a run once reached the identical merge with `git pull origin main --no-rebase --no-edit` after
  the merge spelling was refused), but pulling is an integration step and `stale_verdict` would otherwise
  refuse the contract's own `integrate before publishing` default. Classifying by *direction* was rejected
  (`origin/main` into a feature branch is an integration, `origin/<feature>` into `main` a delivery, and the
  arguments do not say which). `git pull --rebase` is intercepted too.

  **Provably data spans are blanked; the whole-text scan is the floor.** The fragment and disarm lists, the
  unclaimed-commit matcher and `shell::writes_a_file` read a line whose data spans are blanked, so a command
  name written as data (the `split` inside a single-quoted `gh issue view --jq '…'` program, `estigia
  stand-down` quoted in a `cat > file <<EOF` body) is not adjudicated as a command. Eligibility is a
  **grammar checked byte by byte** in `data_view`, an allowlist (the denylist it replaced lost seven review
  rounds, each to one more spelling bash reads differently): a bare first word from a bounded list of
  read-only commands, a restricted byte set outside quotes, quotes opened at a word start and plain
  here-documents. `rg`, `sort`, `printf` and `test` are absent because they run what they are handed
  (`rg --pre`, `sort --compress-program`, array subscripts), and `gh repo`/`gist` because `clone`, `fork` and
  `create` forward words after `--` to `git`. **Not closed:** an unquoted argument, so `echo git push` still
  reads as a boundary; what a user's shell profile or an alias changes about the allowlisted names is
  unproved. **A differential test holds the property against bash itself**:
  `nothing_bash_executes_is_blanked_as_data` runs the measured inputs and 4000 generated cases under `bash
  --noprofile --norc -c` and fails when a marker command bash executed (`touch ran_N_`) is not still in
  `data_view`'s line. It cannot pass vacuously (a timeout fails, every measured input must run its marker on a
  bash new enough, a floor of 4500 executed markers with 4790 measured), requires `bash` and fails without
  it; the versions needed are `printf -v` subscripts 4.0, `test -v` 4.2 and `${ cmd; }` 5.3, the local bash
  3.2 rejects all three so unit tests carry those, and the version numbers are unmeasured here. **A quoted
  command word is read as the word it spells** (`shell::unquote_plain_words`: `pu'sh'`, `"gi"t`, `$'push'`);
  not read are a backslash-escaped quote, a word spelled through a variable, `$(…)`, an alias or script, and
  non-plain content (`git "pu\sh"`).

  **What exact-literal matching leaves** (traced by reading on 2026-09-07, not run): *not matched at all* are a
  backslash line continuation, a `.exe` suffix and a full path carrying a global option, `git update-ref
  refs/tags/…`, `gh release upload`/`delete`/`delete-asset`, `git send-pack`, `gh repo sync`, `git branch
  <name>`, `git checkout HEAD -- <path>`, `git checkout -f` and `git filter-repo`; *matched but never read as
  a delivery* are `gh api` against the merge, release and tag-ref endpoints and the pull spelling.

  **The gate also measures its own disarmament.** `estigia stand-down`, `guard --uninstall`, `uninstall` and
  writes to the files those decisions are read from (its state, the contract, each agent's settings entry)
  are on the disarm list with the sibling subcommands (`estigia config edit`/`forget` beside `set`); a
  crossing over the parser's own tree holds every leaf below a listed verb to that list or to `DISARM_READS`
  (`config list`, `config repos`), so a third writing subcommand fails a test, while a new **top-level** verb
  is a review decision. `without_leading_options` moves a global option out from between the binary and the
  verb (`estigia --json config edit` is refused) and the binary is recognised by basename.

  **Declaring a stand-down is a person's act, and that covers the record as well as the command.**
  `harness::person_only` is decided first by `gate`, `gate_observing`, the push guard and the role road, so
  no stand-down, claim or unreadable definition lifts it. The gate refuses an agent's shell line running
  `estigia stand-down` (`stand-down-is-for-a-person`, any host, sub-agent or orchestrator), a file-tool,
  patch-body or shell write naming the stand-down record under `~/.estigia` (also via the control directory
  and the bare name), deleting it, and a line carrying `--lift` (it cannot be told from a declaration that
  only quotes the flag). The person lifts with `estigia stand-down --lift --reason <why>` in their own
  shell, or `!` in Claude Code. **Open by declaration:** a spelling that never writes `estigia stand-down`
  as two words (an alias, wrapper script, `cargo run -- stand-down`, a renamed binary), a write the shell
  reader does not recognise (an interpreter's `open(...)`), a glob or directory target that never spells the
  file name, a directory change in a separate call or through a variable or `popd`, a name the shell expands
  (`stand-$'down'.json`, `${x:-stand-down.json}`) and a name split across a line continuation. The match is
  broad on purpose: a commit message, `rg`, `git grep` or `git log --grep` naming `estigia` then `stand-down`
  and a patch body naming the record are refused with no lift; `cat` and the `Read` tool stay allowed.

  Twenty `Population` consts in the source (`src/harness/population.rs` holds the type) name each legitimate population and what falls outside it, and that number is checked here.
- **An issue is refused from a shell line, and the template is checked in one place.** `create` refuses a
  body whose first `##` heading outside a code fence is not `## Description for dumb humans` followed by a
  `> [!NOTE]` alert, or that lacks a required heading or has them out of order, before it calls `gh`. The
  required headings are the `##` lines of the fenced block in `skill/assets/analyst-issue-template.md`
  (extracted by `build.rs`, so the file is the only list); the `###` sub-sections are not enforced, and
  the check is structural, not about the prose beneath. `gh issue create`/`new`, `gh api` posting to an
  `/issues` path and a GraphQL `createIssue` are refused `issue-creation-outside-flow` whoever holds the
  checkout, naming `create` and its `repo` argument. Matching is on substrings of the command line, so a
  line that merely mentions the command is refused, `gh` after `sudo` is matched, and an alias or script
  that never spells `gh` still files an issue nothing checks. The read exemption knows only `-X GET` and
  `--method GET` anywhere on the line, so a post beside a `-X GET` read (`gh api -X GET a; gh api
  repos/o/r/issues -f t=x`) is unchecked. An issue filed in the web interface or by a token outside this
  harness is unchecked too.
- **A delivery is refused in a state that holds no verdict.** `git merge`, `git tag`, `gh pr merge` and the
  two `gh release` steps need the issue in `review` or `done`; `git push` and `gh pr create` are exempt
  (publishing is how a run reaches review). Operator-declared boundaries are never read as deliveries,
  since Estigia cannot know whether a `make deploy` delivers or rehearses. Two `git merge` shapes are
  preparation, not delivery; the integration merge has its own entry below. The narrower: an exact literal
  `git merge --ff-only <target>` (also `--` before the target) may run from `in-progress` after the tracker
  agrees when local Git proves all of it: on a branch, upstream a canonical `refs/remotes/*` ref, clean
  worktree, target that upstream's exact name or a full commit object ID, and ancestry from `HEAD` through
  the target to the upstream. Every proof Git process strips inherited `GIT_*`, and the exception is denied
  when any was present at entry or when `BASH_ENV`, `ENV` or `BASH_FUNC_git%%` is (compared without case).
  It stays a `Boundary` (a live tracker read, a ledger entry), skips only `out-of-phase`, only when the
  tracker answered exactly `in-progress`, after the verdict-to-bytes check. `analysis`, `ready` and
  `blocked` keep the refusal. `exact_integration_merge` spawns no process, so it refuses wrappers,
  compounds, quoting, expansion, alternate git directories and extra flags on the text alone; a detached
  branch is refused by `is_integration_in_the_runs_own_checkout`'s `symbolic-ref`; a dirty tree is refused
  nowhere on that path, deliberately, since `--continue` runs against a mid-merge tree
  (`a_conflicting_integration_is_resolved_without_leaving_in_progress`). **The proof is local and can be
  stale** (it never fetches), **and the proof and the merge are not atomic**: another process can move a
  ref, change the worktree or replace what `PATH` resolves between any two reads, and Estigia takes no
  lock. The ledger's allow line names `git merge` and the live claim, not which exemption admitted it.
- **That narrowing is not configurable, and the asymmetry is the reason.** Every other axis here can be
  switched; this one only tightens, and a setting that could loosen a guard rail turns it into a
  preference, as with the operator's boundary list, which adds and never removes.
- **Nothing here has driven a real agent.** The hook protocol is exercised by feeding the binary the
  payloads an agent sends and reading what it prints. That proves Estigia answers correctly; it does not
  prove Claude Code asks.
- **The tool-call gate reaches every agent Estigia knows but two.** Five agents, in four shapes, each
  taken from that agent's published reference or its source: Claude Code and Codex (`PreToolUse` →
  `permissionDecision`), Cursor (`preToolUse` → `{"permission":"deny",…}`), OpenCode (a plugin that blocks
  by throwing) and Pi (an extension that blocks by returning `{ block: true, reason }`). The two it does not
  reach are ZCode and DeepSeek Harness, and the bullets on them say why. The eighth entry is the
  agent-neutral skills root, which is not an agent and has no tool loop to gate. The gate also still
  answers for six retired hosts — Gemini CLI, Continue, Cline, Crush, Windsurf and Qwen — because a machine
  that installed one before it was retired still carries its hook and dialect; `setup` installs into none
  of them and `uninstall` is the only path that touches them.
- **Three gates fire on every tool, and only Estigia's own lists narrow them; one of the three is
  still installable.** Cursor registers no matcher, so its host wakes the hook for whatever the model
  called and the tool name arrives in the payload; the retired Windsurf does the same, and the
  retired **Cline** arrives a different way: its gate is a script Estigia wrote whole, and that script
  pipes every `PreToolUse` payload straight through — no matcher, and no named list of its own. For
  every other agent something is crossed against the classifier: a settings matcher for Claude Code
  and Codex, and for OpenCode and Pi the `GATED` array their plugin and extension carry. For these three there is nothing
  to cross: this build has not been shown which names those hosts send, so a tool of theirs that is
  missing from `WRITE_TOOLS` or `SHELL_TOOLS` reaches the gate, classifies as `Untouched`, and is
  answered "not something this run's oath covers" — a sentence that asserts coverage nobody checked.
  Closing it takes a payload captured from a real session of each, which is the same evidence the
  rest of this table is built on and the reason this line is here instead. Measured, so the size of
  it is not a guess: `git push --force` reaches the classifier as a boundary under `Bash` and as
  `Untouched` under three other plausible spellings of a shell tool, and an edit to `src/main.rs` the
  same way.

  The guard that enforces this entry walks every agent whose tool calls Estigia gates by any
  mechanism, retired ones included, because their hooks are still on machines that installed them; a
  plugin-written gate that carries no list of its own has to be named here.
- **`ensure-states` cannot tell "already there" from "gh could not run".** `gh label create --force` exits
  non-zero for both, and the command reads that as idempotence. Measured against a `gh` that refuses every
  call: each answers `ok: true` with all six labels listed as `ensured`, and none exists. Closing it means
  reading what `gh` printed, and this crate has never been shown those exact words (the reason `icacls` is
  named beside `neutralising-shell` and not added to it). A closed git operation window is the one case
  where the answer says so (`deferred: true`, `window: closed`, nothing created).
- **The counts in the sentence about gated agents are crossed against the adapter table and the dialect
  list.** The check accepts the phrase "every agent Estigia knows" and used to stop there, while the
  sentence went on to say "six of them, in three dialects" when ten were gated in five: one end written by
  hand and the other in code, with only *part* of the prose crossing over.
- **A sub-agent's declared tool list is enforced only for the tools the gate wakes for, and only where an
  agent name arrives.** The role gate runs in `PreToolUse` and in `estigia gate`, but the second door is
  reached in this tree only by OpenCode's plugin, which sends no agent name, and whose documented limit is
  that `tool.execute.before` never sees a subagent's call, so it closes nothing today. Where the question
  *is* asked it sees exactly what the matcher wakes the hook for
  (`Edit|Write|MultiEdit|NotebookEdit|Update|Bash|Agent|Task|AskUserQuestion` for Claude Code; `Agent` and
  `Task` take the reserved-reviewer prelaunch path first). Measured against a published `builder`
  sub-agent declaring `Read, Write, Edit, Glob, Grep, Bash`: three of the six are tools the gate can judge
  and `Read`, `Glob` and `Grep` are never seen. It can refuse a tool **in the matcher and not in the
  list** (`MultiEdit`, `NotebookEdit`, `Update` for that builder), and not `WebFetch` or `WebSearch`,
  which an operator narrowing a sub-agent usually means. Widening the matcher would close it and is the
  one thing the matcher exists to avoid: waking this process for every `Read` and `Grep` is a cost paid
  thousands of times to answer "not mine".
- **The verdict's binding is checked at delivery only for what this run published through Estigia's own
  tools, with an earlier exact-receipt check at CI release.** `publish_review` records a fresh epoch over
  PR/head/base/digest while the PR is confirmed draft. `release_ci` re-verifies the live `review` claim, the
  newest receipt on the receipt's pull request across runs, the verdict recomputed over the live ledger, a
  current draft PR and a coherent clean target, then marks ready and reads back every outcome. The gate asks
  for one approved computed verdict; it does not enforce the blind policies' lens count, concurrency,
  independence, blindness or same-finding identity, and cannot tell one context from two or five. GitHub has
  no atomic conditional-ready operation, so an out-of-band ready or push can bypass this cooperative order.

  The local delivery gate persists the complete epoch/PR/head/base/digest receipt and checks the published
  head at the boundary that spends it, against different things per boundary. A `git merge`, `git tag` or
  `gh release` from a run whose recorded review head is not the invoking checkout's own head is refused
  `verdict-bound-to-other-bytes` (the run's own merge commit is the #58 exception); the invoking checkout is
  the covered directory selected for this call, not inferred from the pointer, whose worktree can be absent
  after a handoff. A `gh pr merge` is refused against the PR head the tracker reports (one `gh pr view
  <number>` read per attempt), never the invoking checkout's `HEAD`, which under Claude Code is structurally
  the launch checkout. A linked sibling sharing the pointer checkout's Git common directory can reach that
  verification: the classifier retains one positive numeric PR only from one literal `gh pr merge <number>
  ...`, candidates are filtered by complete `receipt.pr == command PR`, exactly one holder must remain, and
  only then are the tracker's PR head and that holder's claim compared. Refusals name the cause:
  `delivery-pr-repository-option` (`--repo`, `-R`), `delivery-pr-unidentified` (quoting, escaping, expansion,
  a redirect), `delivery-pr-mismatch`, `complete-review-transaction-missing` (a pointer with only a legacy
  `reviewed_head`) and `complete-review-transaction-not-selected` (no unique sibling holder); ambiguous
  candidates and unreadable pointers select no holder, and a sessionless call in an unrelated clone remains
  outside.

  Publish and republish first invalidate prior complete and legacy local authority and retain output only
  when all five receipt fields are complete (also when a refusal says a write committed or cannot exclude
  one); verdict and CI-release effects restore a supplied receipt into a reviewer's pointer after a handoff
  removed the publisher's. `gh pr merge` is also refused `merge-without-ci-release` unless the store holds a
  trusted `ci-release` marker for exactly the pointer's receipt (written by this identity, never edited, so
  an imported comment carrying one counts the same; an unreadable store refuses). A `claim` restores a
  receipt when its answer carries the issue's newest confirmed, approved, unescalated publication whose pull
  request is open at the receipt's head with that base or one advanced cleanly past it, at the price of one
  `gh pr view` read (a failed read restores nothing); it does not read CI state, and a push between the
  claim's read and the merge is caught only by `--match-head-commit`. `git push` and `gh pr create` are not
  gated on it (the answer to a moved head is *re-publish*). Not seen: a run that bypasses the tools and
  changes GitHub directly. For `git merge`, `git tag` and `gh release` both halves of the comparison run
  through the same unsteered git invocation, so an exported `GIT_DIR` cannot make an unrelated clone look
  like the reviewed checkout.
- **No live Pi turn has run any Estigia extension; each is measured under `node` against a stand-in binary
  and a fake `pi`, and the rest is read from the installed Pi 1.1.0 package.** The stand-ins assert the
  arguments handed to `estigia gate`, `goal`, `review prompt|model|admit`, `session start|end` and `doctor`,
  the working directory, the three outcomes and the `{ block: true, reason }`, the relay's refusal codes and
  failed-completion paths, the loop's continuation, waits and stops, the status text, the refusal card and
  the subagent runner's RPC handling (the stand-in `pi` speaks only the records the runner reads and loads
  the real generated gate). `loadExtensions` was run once by hand against the generated gate and relay files
  (not in the suite). **Not observed:** that Pi **acts** on a returned `block` and awaits an async handler; a
  model calling the review tool or a real `streamSimple` returning an `AssistantMessage` (shape read from the
  `.d.ts`); a provider honouring the `AbortSignal`; a model turn started by the loop's continuation, and
  `agent_settled` in print and RPC modes (which have no UI); the footer line, the card on screen,
  `appendEntry` inside a streaming `tool_call` handler, the completion list, prompt-template expansion,
  `ctx.ui.custom` with `overlay: true`, `registerShortcut("ctrl+alt+a")` being free; that Pi accepts a section
  added in place by a `before_agent_start` handler, awaits an async `session_shutdown` handler and reports
  `fork` for both `/fork` and `/clone`; RPC `steer` accepted by a real child; the subagent runner's stall
  watchdog (none: a child that never settles runs until stopped; only the thirty-second first-answer deadline
  is enforced) and Windows (it kills the child, not its group). The built-in MCP client the `mcp.json` entry
  depends on exists from Pi 0.99.0 (an older Pi ignores the file while `status` reports the tools
  registered), and `exposure: "direct"` is read from `docs/mcp.md`. Pi 1.1.0 has no sub-agent concept, so
  "child session" is a heuristic (a `parentSession` in the session header). The tool is `estigia_subagent`,
  not `subagent`, so the issue's wording is not met literally.
- **The Pi loop forgets its goal when Pi restarts, and its `/goal` judge was read from Pi's declarations, not
  run.** The goal is held in the extension's memory (the run's claim and `stop-after` live in the store), a
  timer firing after the session was replaced stops the loop without notifying, and the objective is sent as
  a custom message Pi does not expand like `/skill:flow`. The judge asks `ctx.modelRegistry.streamSimple`
  and no test reaches a provider: not measured are request-time authentication, whether a model given 200
  output tokens answers on its first line (a reasoning model that spends them on thinking returns no text,
  which stops the goal as unreadable, never as met), and what the judge sees (a plain-text transcript cut at
  4,000 characters a message, 2,000 per tool output, 20,000 in all, keeping the newest; every third turn and
  the cap turn). Another extension registering `goal` or `loop` is not detected.
- **`deferred` exposure was measured with one model and one run per prompt, and setup reads the Pi version
  from `pi --version`.** On Pi 1.2.0 with `opencode-go/deepseek-v4.1-flash` a first request cost 3,255 input
  tokens with `deferred` against 13,868 with `direct` and 2,986 with the server off, and the model reached
  `mcp__estigia__*` tools through `tool_search` for an explicit prompt (8,720 tokens in all) and an implicit
  one (67,063): single runs, 2 to 15 turns, no other model, and a real flow under `deferred` is the open
  number. The version gate compares the first `x.y.z` in `pi --version` with 1.2.0; a `pi` off the `PATH` of
  the process running `setup`, or printing no version, gets `direct`, and a Pi upgraded afterwards keeps
  `direct` until the next setup (the `pi-config` warning names it). Pi 1.2.0 was not installed on the build
  machine.
- **Pi parent and child messages were run live twice, by hand** (2026-10-15, Pi 1.1.0 as both): a foreground
  `estigia_subagent` showed the parent cannot reply while it waits, and a `background: true` run woke the
  parent on the child's query, returned the reply as the child's tool result and delivered the child's final
  answer as a follow-up within 15 seconds. Not run live: the immediate refusal of a foreground query,
  steering through `estigia_subagent_message`, a query that times out after the background fix, a child under
  an agent definition restricting `--tools`, `deliverAs: "followUp"` waking an idle session, and ordering
  between two children. A notification sent while the parent is gone is lost (at-most-once), and the
  120-second query timeout is a constant. **A reply can still be reported as answered after its child gave
  up**: a reply that crosses the child's withdraw frame in flight, or a withdraw that never arrives (the
  parent's own expiry `WITHDRAW_GRACE_MS`, five seconds after the timeout, is all that closes it), makes
  `estigia_subagent_reply` answer `answered` though the child never receives the text; no test covers either
  window, a failed withdraw send, or a withdraw for an unknown request.
- **A Pi child is linked by its session header, with the pid as the fallback and an environment variable
  beneath that; the live check ran without a claim and under `-p` only.** The header link is held by
  `the_pi_extension_links_a_child_by_its_parent_header_whatever_its_pid`. The parent's id is the exported
  root, else the `id` in its session file, else the part of its file name after the first `_`; none is
  authenticated and `--parent-session-id` is an assertion the binary cannot check, like `--run-id`. Live
  (2026-10-14, isolated Pi home): a child `pi` started by Pi's `subagent` example was another process and its
  `write` and `bash` reached the gate with `--parent-session-id` equal to the parent's session; the write was
  refused `issue-required` for want of a claim, so what a child is *allowed* under a held claim was not
  observed. A forked or new session also carries a `parentSession` but is read as a root by its start reason;
  a replacement inside a separate child process is still linked by the pid; an extension passing its own
  `env` without the two variables is unlinked; a parent that names no run gets no fallback to the child's own
  (the child receives the parent's answer, `nothing-sworn` included). The `pi-subagents` row of `doctor`
  reads names and source text, not behaviour: a runner is recognised when `subagent` is in its package source
  or extension name and called gating only when every `.ts`/`.js`/`.mjs`/`.cjs` file within three levels was
  read inside a 1 MiB budget and a `spawn(`, `execFile(` or `spawnSync(` visibly takes `pi` as a
  string-literal first argument; anything else is answered unknown, and an npm or git package is unread.
- **Pi's reviewer model is only as precise as the `reviewer` routing row, and Pi's agent definitions follow an
  example extension's convention.** A `provider/id` Pi lists runs on that model; a bare alias such as `opus`
  is a valid CLI model and unresolvable for Pi (`reviewer-unavailable`); on a machine with the `claude` CLI
  Pi's relay is never reached; no reasoning effort is set; and neither the relay nor `admit` bounds the
  answer's size as a CLI reviewer's `MAX_OUTPUT` does. The agent definitions (`name`, `description`,
  `tools`, `model` frontmatter, `~/.pi/agent/agents/`, the `--tools`/`--model` pass-through) come from the
  shipped `examples/extensions/subagent` in Pi 1.1.0 and no child `pi` was launched against one. `tools`
  restricts only built-in tools (MCP tools stay unless an entry starts with `mcp__`), so what stops the
  analyst writing is the gate measuring the child against the parent's claim, and
  `harness::roles::definition_for` does not search this root.
- **Pi's `powershell` is read as POSIX shell, other extensions' tools are gated only by input shape, and
  Pi's own settings are not covered.** The extension sends any tool whose input has a string `command` to the
  gate as `bash` and a string `path` with `content` or an `edits` array as `write`/`edit`; a PowerShell
  cmdlet that writes (`Set-Content`, `Out-File`, `Remove-Item`) goes through, a tool naming its command or
  path differently (`cmd`, `file_path`, `script`) is not recognised, and a later `tool_call` handler can
  mutate `event.input` after this one answered. **Pi's settings can unload the gate and the tool server**:
  the `extensions` array of `settings.json` takes an exact exclusion `-path`, a glob `!pattern` and
  `-builtin:mcp`, a project's settings apply over the user's, and a project `mcp.json` entry with no
  `command`, `url` or `type` overrides `enabled`, `exposure` and `toolExposure` of the user's server, so one
  write to `~/.pi/agent/settings.json`, `.pi/settings.json` or `.pi/mcp.json` unloads them without touching
  a file Estigia owns. All of those, and `.pi/extensions/`, are on the control surface (a write to a
  *different* repository's `.pi/settings.json` too, since the matcher cannot tell home from project). The
  `pi-config` row of `doctor` reads them for an exclusion of `estigia-workflow-authority.ts`,
  `estigia-review-relay.ts` or `-builtin:mcp`, and the project `mcp.json` for `enabled: false` or an
  `exposure` other than `direct`/`deferred`; it is broken, rewrites nothing and names no command. Not
  measured: whether a project `-path` excludes a *user-level loose* extension at load time, Pi's glob
  dialect (the row lets `*` cross `/` and knows `?`), other resource arrays and `packages`, that the row's
  checkout is the project Pi opens, and project trust.
- **Pi's agent directory follows `PI_CODING_AGENT_DIR`, but only as the process that runs `setup`, `status`,
  `doctor` or the gate sees it.** One resolver reads the variable (a leading `~` is the home; anything else
  not absolute is dropped with a note, falling back to `~/.pi/agent`) and every Pi path comes from it;
  unlike `DSH_HOME` the directory is followed wherever it points. The gate treats the named directory's
  `extensions/`, `mcp.json`, `settings*`, `AGENTS*` and `agents/` as control surface, but the static
  `harness::CONTROL_SURFACE` cannot spell a directory the environment chooses, so a Pi started with a
  different variable is not covered. An install made under `~/.pi/agent` and then run under a variable
  naming another is not reported (`doctor` says only that Pi is not configured), Pi's expansion beyond the
  leading `~` is not reproduced, and nothing was run against a live Pi under the variable. Pi documents
  that `AGENTS.override.md` replaces `AGENTS.md` in the same directory, so the `pi-config` row is broken
  when one is in the agent directory and does not look beside a project's own `AGENTS.md`.
- **A gate plugin newer than its binary blocks every gated call.** Both plugins send `--input -` and the
  input on standard input; an older binary reads `-` as the JSON, refuses with exit `1`, and the plugin
  blocks the call. `setup` writes the plugins together with the installed binary, so the skew arises only
  when a plugin outlives a downgrade; re-running `setup` clears it.

## Delivery: the receipt, the base and the remote

- **What the `gh pr view` read measures, and what it does not.** It measures the PR's `headRefOid` at
  the moment `stale_verdict` asks, one read per merge attempt, and refuses a mismatch against the
  receipt with the PR number, the reviewed head and the head found. It does not measure the base:
  `release_ci` re-verified `baseRefOid` at ready time, and the only later reads of it are
  `review_status` and the claim's restoration of a receipt, which accept a clean advance (next entry),
  so a base that moves *between* that ready check and this merge attempt is not seen at this boundary.
  It does not measure anything after the read: a push landing on the PR between the read and the merge
  is refused by GitHub itself only when the merge call carries `--match-head-commit <head-sha>`, which
  `skill/bindings/github/delivery.md` prescribes and this gate does not verify was used. It does not
  resolve which repository `gh` would ask: an unrelated clone is refused on `same_repository` (whether
  the covered checkout and this one share a Git common directory), never on what `gh pr view` would
  have answered for it.
- **A base that advanced cleanly keeps the receipt live, and a semantic conflict between two
  independently green changes is not tested before the merge.** Issue #44, operator decision
  2026-10-13. A receipt stays live for the merge, in `review_status` and in the claim's restoration of
  it (`current_review_transaction`), when the head is the reviewed head, the pull request is ready and
  its receipt approved, the current base descends from the reviewed base (`git merge-base
  --is-ancestor`), and GitHub reports `mergeable: MERGEABLE` with a `mergeStateStatus` that is not
  `DIRTY`. `UNKNOWN` in either field is answered `mergeability-unknown`, never clean. **The accepted
  risk:** CI went green on the reviewed head over the *old* base and nothing asks it again over the
  new one, so two changes that merge without a textual conflict yet break each other are not caught
  before the merge; only the CI that runs on `main` afterwards sees it. `release_ci` stays strict
  about the base: a draft whose base moved is answered `pull-request-moved`. `mergeable` is GitHub's
  own and lazily computed, so a clean read can precede a conflict that lands in between. When the base
  object is not in the local clone the ancestry read fails and `review_status` answers `unavailable`
  rather than guessing; the claim restores nothing.
- **`recorded_receipt` does not require the caller to be the run that published, and this proves only
  that the tracker recorded the publication — never that the caller reviewed anything.** A local run
  pointer is deleted at `SessionEnd` (`session::forget`) while the tracker's comment marking the
  publication is not, so a run that resumes an issue, holds the live `review` claim, and supplies the
  byte-exact latest receipt is not refused on the publisher field. `require_recorded_receipt` compares
  only `latest.receipt != *supplied`; the publishing run id is read back into the refusal's
  `recorded.run_id` but is never grounds to refuse. What still stands between an arbitrary caller and
  CI is what `release_ci` re-verifies: a live review claim on this exact issue, a computed `approved`
  verdict over this receipt's ledger, the current draft PR identity, and a re-derived coherent clean
  target. **"Distinct" excludes the publisher and every handoff requester, not the caller**, so the
  run that recorded the computed verdict can be the run that spends it; that was already the design
  (`PointerEffect::Renew` writes the restored receipt into the resuming run's pointer). Estigia still
  cannot prove who reviewed anything.
- **A delivery can still merge onto a base the reviewed receipt did not name, and the remedy is a
  GitHub setting that Estigia neither sets nor reads back.** `release_ci` re-verifies every byte of
  the reviewed head, and nothing asks whether `main` was still the receipt's base when the merge
  happened. With `required_status_checks.strict: true`, GitHub requires the topic branch to be up to
  date with the base, and an update is a push, which invalidates the receipt, so a moved base forces a
  fresh publish/review round. The forced round: the `compatible` classification still integrates, but
  the integrating republish is repair-shaped (the republish gate reads whether the base **advanced**
  past the parent receipt's), so a merge that keeps the parent head is publishable over an approved
  parent, spends no correction, and the panel re-judges the parent receipt's findings over the whole
  change. `repair_range` answers `whole-change` with `whole_change_reason: "base-advanced"` whenever
  the base moved, merged or rebased (`"merged-range"` when a side branch was merged back), so the
  delta lens never reviews base content, and the budget measures the round against the tree git
  merges the parent head and the new base into. The exemption is direction-aware: it needs the
  parent's base to be a proper ancestor of this publication's, so a base reset backwards is not an
  integration. `unknown` is never compatible. What this does **not** guarantee:
  - **Estigia does not read the setting back.** Nothing in the gate queries branch protection at
    delivery time; the guarantee holds only while a collaborator leaves `strict: true` on. Read live
    on 2026-09-27, `main`'s protection answered `required_status_checks: null`, so the setting was
    not on, and the forced-integration shape was the contract's intended route rather than the
    platform's requirement.
  - **The delivered SHA's first parent is guaranteed by GitHub's strict check, not by Estigia.** The
    gate's merge-identity checks (`stale_verdict`, `pr_merge_target`) compare the reviewed head, never
    the base; the base's constancy is the repository setting's promise, taken on trust.
  - **A strict setting does not make concurrent delivery converge.** A 2026-08-20 diagnosis measured
    `main` moving seven times under one branch while the delivery floor (integrate ~4 min, CI 14 min,
    merge: over 18 minutes at zero review time) exceeded the delivery interval (15 to 25 minutes);
    each base move costs an integrate-and-republish round that invalidates the accepted verdict. An
    integration over a moved base spends no correction, so the correction limit does not bound a
    base-movement storm either; what ends the wait is the base stopping. `ci.yml` carries
    `merge_group:` so a merge queue can run the suite after ready, but enabling the queue and whether
    its run satisfies required contexts is live-repository configuration this crate neither sets nor
    proves.
- **A `published` marker does not prove the remote still holds that head, and an unconfirmed one does
  not prove the pull request ever reported it.** After a successful push, `publish_review` reads the
  PR back before recording a confirmed receipt. When that readback disagrees, the push has already
  landed, and the recovery records an **unconfirmed** publication (`unconfirmed=true`) naming the head
  pushed: `republish_review` leases against it, and `require_recorded_receipt` refuses it as
  `publication-unconfirmed` so review and CI cannot bind to bytes nobody confirmed. Still unproved:
  that the remote is *still* at that head when the lease runs (git decides), that the PR view ever
  agreed, or that the unconfirmed marker was written if the comment post itself failed (the refusal
  then says so and names no executable recovery). A confirmed marker means only that the PR agreed
  *at publication time*.
- **A `republish_review` that refuses has already written to the pull request, and names which
  writes.** Its refusals arrive after the reused pull request has been edited — the renewal stands
  immediately before the push, and the lease is evaluated by git at the push itself — so a remote
  somebody else moved leaves the branch untouched and the pull request altered. **Every** refusal site
  downstream of a write carries that fact (the draft readback, the failed edit, the renewal and the
  push); the report names each write separately and only when it happened, because `ensure_draft`
  un-readies only a pull request that was *ready*, and an operator undoing a conversion nobody
  performed runs `gh pr ready`, which exposes the branch to CI. Estigia reports the write; it does not
  undo it.
- **The review transaction carries a declared execution topology, and Estigia sees the declaration, not the
  delegation.** `publish_review`/`republish_review` take `--topology` and `--topology-reason`, write them to
  the `published` marker (recorded, not bound) and answer them through `review_transaction`; a blank value or
  a reason with no topology is refused and both are trimmed once. The string is what a run wrote about itself:
  `--topology developer:3` having launched nothing is indistinguishable on the timeline from a real
  delegation. There is no refusal, because the proxy that stood in for one (a bare `inline` over two changed
  files) could not judge the row: this repository's last twenty merges touched 3 to 26 files (median 11).
  `Run.roles` holds half of an *observed* topology elsewhere, unconnected. Saying nothing is free, and so is
  `inline`: nothing tells "judged the table and departed with reasons" from "never read the table", and
  making the field required would refuse every caller that predates it. The `published` marker also does not
  record that a publication was a republish (`republished` is in the answer only); that costs no reader and
  was declined on scope.
- **A republish cannot lease against a receipt published from a different GitHub account.**
  `latest_publication` keeps only comments the authenticated identity authored, because a marker this
  identity did not write is one anybody could have forged. A run that reclaims an abandoned issue
  published from another account finds no receipt, is refused `published-transaction-missing`, and has
  no route inside Estigia (`publish_review` cannot fast-forward a rewritten branch either). It fails
  closed and leaves that reclaim nothing to do but step outside the tools. The lease compares one
  commit id to another, so a rebase that dropped a hunk leases as cleanly as one that did not; every
  republish creates a new epoch and invalidates prior evidence, and the answer to *is this still the
  change that was approved* is a fresh verdict against the new receipt, never the lease.

- **The store-only timeline read trusts the store and is given its repository.**
  `Ledger::read_timeline_at` reads no GitHub, so it cannot learn what the store has not yet synced: a
  stand-down or adjudication commented since this machine last touched the issue is not in what it
  answers. It does not discover which repository the checkout belongs to; the caller names it, and
  `review_status` does so from the checkout. The `expected_epoch` check on `publish_review` and
  `republish_review` reads the timeline the publication already reads, compares the newest
  publication on the branch's pull request and nothing else, and a branch with no open pull request
  compares against `none`.

- **`review_status` is a read, not a lock, and it names the repository from the checkout.** The
  answers are in [`review-status.md`](review-status.md). It answers against the receipt it read and
  every command it names carries that receipt, so a publication landing afterwards is refused by the
  command (`published-transaction-mismatch`, `expected-epoch-moved`) and never executed. That protects
  a reader that executes the answer unchanged; it does nothing for one that decides from the answer
  and drops the binding. The store it reads is as fresh as this machine's last sync, so a finding
  another run posted since is not in the answer until a tool that syncs has run. The repository comes
  from the operator-named repository, else the `origin` remote when that is a `github.com` remote,
  never from GitHub: where `gh` resolves a different repository (a fork with an `upstream` default)
  the answer is `review-status-issue-not-in-store`, unavailable, never a wrong state. A pull request
  merged and a new slice not yet published under the same issue answers `delivered` for the issue's
  newest publication only. **A stale approval with no cause has no command of its own.** When the
  ledger or tier moved after an approval and no finding was recorded against it (a tier that moved, a
  late lens report that escalates the lens focus), neither a publication
  (`approved-republish-without-cause`) nor a re-record (`computed-verdict-already-recorded`) is
  admitted, so `review_status` answers a `verdict-stale` stop that names a person, who either records
  the finding the verdict did not weigh with `record_review_finding` or closes the pull request and
  publishes afresh. Whether such an approval should be re-reviewable without a finding is open. A base
  advance is not read, so a stale approval whose base advanced still answers the stop.
- **`base-integration-not-required` proves a textual fact, not a semantic one.** The refusal needs the
  parent head to merge cleanly with the current base, the paths the base touched to be disjoint from
  the delivery's, and `required_status_checks.strict` to read `false` or the branch to be unprotected.
  None of that says the change still means what it did on the advanced base; `integrate_reason` is the
  run's recorded judgement that it does not, and nothing reads it back to decide. The protection is
  read through the branch-protection endpoint only: a repository that requires an up-to-date branch
  through a ruleset can answer `Branch not protected` there, which is read as non-strict, and the
  integration it requires is refused until `integrate_reason` is given. A protection read that fails
  any other way, or answers something unreadable, is not a refusal; the answer carries
  `integration_unread` and the integration goes through. The same holds for a failed ancestry or
  `git merge-tree` read. The check runs only over a parent receipt on the same pull request that is
  not the root of a recovered successor lineage. `integrate_reason` is recorded on the marker and,
  unlike `delta-range-reason`, is not echoed by `review_transaction`.

## The seams, and the gates that never see a call

- **This section is checked against the code, and the seams along the path.** `tests/honesty.rs` crosses the
  *countable* claims (how many agents are gated, how many things `doctor` looks at, which mechanisms the code
  uses); the claims about *kind* are judgements and are not checked. The gates, the flag lists the MCP tools
  build for the transport and the values a configuration refusal tells an operator to write are each crossed
  against the classifier, the transport's own `argparse` and the parser itself.
- **A registered gate is not an enforcing gate.** Two guards cross-check every matcher and the
  OpenCode plugin's own list against the names the classifier recognises, because a registered gate
  that finds a tool name it does not know stands aside while `status` reports `gate on`. Entries naming
  an event this build refuses (`PreToolUse` where the host's documentation prints another spelling:
  a non-blocking error) or an executable that is no longer there (`cargo clean` deletes it; so does
  moving the checkout) are caught from the other end: `estigia status` reads the registered command
  back and says `gate REGISTERED BUT DEAD` rather than `gate on`, and `doctor` reports the file and
  the fault.
- **A worktree Claude Code makes for itself is not seen.** `--worktree`, `isolation: "worktree"` and
  background sessions create one through the runtime rather than through a shell, so no `Bash` call
  carries `git worktree add` and `PreToolUse` never fires. The event that does fire is
  `WorktreeCreate`, and it is not a permission gate: it *replaces* git's behaviour, expects the
  worktree path on standard output, and fails creation when the hook fails or prints nothing.
  Registering there to deny would make Estigia responsible for creating every worktree. So the shell
  spelling is gated and the native path is not — and a write inside that worktree is still measured,
  because the claim covers the directory rather than the way it was made.
- **Claude Code subagents are covered; that is why the line below is OpenCode's and not the
  design's.** A subagent's tool calls fire the same `PreToolUse` hooks as the main thread, carrying
  `agent_id` and `agent_type`, checked in the runtime's own hook reference.
- **An extension point is not automatically a place to deny.** Three were checked and refused for the
  same reason. Claude Code's `WorktreeCreate` *replaces* git's behaviour and fails creation when the
  hook says nothing. Goose's classification API is an HTTP service that labels text as prompt
  injection. OpenHands' `SecurityAnalyzerBase` is an in-process Python class returning a **risk
  level**, and what happens next is the user's confirmation setting. Estigia's refusal means *this run
  holds no claim covering this write*; rendering that as "high risk", or as an injection score, is a
  different sentence, and one the receiving tool is entitled to overrule. A gate is a place that asks
  permission and obeys the answer; anything else is a suggestion with a hook's costs.
- **Kiro is not an adapter, and one unknown keeps it out.** Only `kiro-cli` (from the Amazon Q
  Developer CLI lineage) is gateable, not the IDE, whose hooks live in a workspace's `.kiro/hooks/`
  while `setup` writes user-level files. Its `PreToolUse` is the only blocking event, it rejects with
  **exit code 2 and the reason on standard error** (any other code lets the action proceed, which is
  Estigia's own rule about a broken hook), and its payload `{hook_event_name, cwd, session_id,
  tool_name, tool_input}` is Estigia's shape exactly. Its steering file at `~/.kiro/steering/` is
  global and always loaded. What is missing is where the hook is registered: Kiro's reference
  documents the events and exit codes and **not the configuration path**. The only evidence is a
  third-party integration naming `~/.kiro/agents/<name>.json` and the key `preToolUse` (lower case,
  where the reference writes `PreToolUse`), and whether `~/.kiro/hooks/` is read at all is
  unchecked. Both unknowns fail silently — a key in the wrong case parses and never fires, and `<name>`
  is the operator's *default agent*, whose file Estigia cannot name — and adding it ungated would buy
  a contract nobody enforces. Settling it needs a real `kiro-cli` installation. If `~/.kiro/hooks/` is
  read, this is a table entry and a `GateSpec`, and nothing else.
- **The OpenCode plugin does not see subagents.** Measured on OpenCode 1: `tool.execute.before` is
  reported not to intercept tool calls made by subagents spawned through the task tool. OpenCode 2's
  `execute.before` has not been measured for the same hole. A gate with a hole is still a gate; a gate
  whose hole nobody mentions is a lie, so it is written into the plugin file itself.
- **The OpenCode plugin costs a process per gated call.** It shells out to the binary for `edit`,
  `write`, `patch`, `multiedit`, `notebookedit`, `update`, `bash` and `shell` — named rather than `*`,
  because waking a process for every read would be a cost paid thousands of times to answer "not
  mine". The list is crossed against the classifier by `the_plugin_gates_the_tools_the_classifier_judges`;
  prose naming the same list is crossed by nothing.
- **The push guard is a guard rail, not a lock.** `git push --no-verify` bypasses it, so does a push
  from another checkout of the same repository, and so does anything that writes refs without
  invoking hooks. A guard rail that claims to be a lock is worse than one that does not.
  - **The operation schedule is observed through the same openings.** An enabled `Git operation
    window` denies commits, ordinary pushes, and branch creation at every Estigia entry point — the
    shell classifier, the pre-push guard, and `publish_review` — and nothing else enforces it.
    `start_branch` makes the local branch and the worktree and queues the link to the issue while the
    window is closed. `git push --no-verify` skips the guard, direct object and ref writes never reach
    the classifier, hosting APIs never reach any hook, and another clone holds no copy of the row: the
    repository override lives in one clone's git directory. A quiet period is observed through
    instrumented entry points, never proven across the repository. An enabled value that does not
    parse, and a clock that cannot be read, deny the protected operations rather than falling back to
    unrestricted, and ordinary file edits, tests, builds, and worktree operations on an existing branch
    stay available either way. The weekday and the minute are the ones the machine's local clock
    reports, so a changed `TZ` or system zone moves the window with it.

## The push guard, the MCP server and the state a run keeps

- **Nothing adjudicates what a push is aimed at.** git hands a `pre-push` hook the refs it is about
  to write, on standard input. They are read and written into the ledger, so the record names *which*
  push was decided on, but no decision is given them **to classify the push itself**: `guard::decide`
  takes the checkout *and* the refs, and what it adjudicates is the three words `git push`, whichever
  branch the refs name. A run holding a live claim may `git push origin branch-a:main`, and the guard
  allows it as the same three unaimed words, because `git push` is deliberately absent from the
  delivery population — publishing a review target is how a run reaches review, and pushing after one
  is how it fixes what the review found. That reasoning assumes a push is a step **towards** delivery.
  A push aimed at the base branch **is** the delivery, and it never runs `git merge`, so no verdict is
  ever asked for. For the adapters that get the contract and no tool gate (ZCode and DeepSeek Harness)
  this hook is the only enforcement there is. Measured by
  `what_a_push_is_aimed_at_is_recorded_and_not_adjudicated`.

  **Under `Workflow: assisted` the destination is checked, and everywhere else it still is not.** An assisted
  run's push is admitted only when every line git names has the run's own branch as both its local and remote
  ref (`harness::assisted`, `a_push_is_admitted_only_for_the_runs_own_branch_and_only_when_its_refs_are_known`).
  It is not closed for other runs because closing it wrongly is worse than the gap: the check needs the base
  branch and `Integration mode` (under `trunk` the base branch is where work is meant to land), and a
  boundary that refuses a legitimate push is how a harness gets uninstalled.

  What *is* read from the refs is **who** several holders' refusal belongs to, not **what** the push is: a git
  hook has no session, so `holders_of` is all the push road has, and `adjudicate_action` narrows the holder
  list by the pushed refs through `git worktree list`, read fresh and stripped of inherited `GIT_*`.
  `git push origin branch-a` and `branch-a:main` both write the local ref `refs/heads/branch-a`, so both
  narrow to branch-a's holder and reach the same unaimed decision; `HEAD:main` writes the literal `HEAD` and
  stays refused under several holders. **The rule is not a union over whatever resolves**: every ref has to
  resolve to one worktree owned by one holder, and all refs to the same holder; it fails closed on an empty
  ref list or a local ref that is not `refs/heads/<branch>` (a tag, `HEAD`, a raw sha, `(delete)`), an
  unreadable listing, a branch carried by zero or by two or more registered worktrees (`git worktree add
  --force`), a worktree owned by zero or by two or more of `holders`, and two refs on different holders.
  `harness::guard::tests` drives each road. Two edges stay outside: the run-id-less **tool** road (the
  OpenCode plugin's `tool.execute.before` when its host handed it no session id and no run id was exported,
  and `estigia gate <tool>` with no identity) has no refs to narrow on, and `guard::decide` is the one
  production caller (`a_pre_push_hook_is_decided_on_the_holder_the_pushed_branch_names`, `tests/pipe/gate_hook.rs`,
  drives that wiring with a real `pre-push` stdin).
- **The push guard refuses to replace somebody else's hook.** A `pre-push` is a test runner or a
  secret scanner as often as it is nothing, and Estigia will not take away a check somebody relies on
  to add one they did not ask for. It says so and names the way out.
- **The MCP server has never met a real client.** Every response is checked against the JSON-RPC
  envelope rules a client's parser enforces before any content matters, and the server is driven
  through a full session in tests. That proves the envelopes are legal and the answers are right; it
  does not prove a particular client is happy with them, and the protocol subset is a subset.
- **The cost avoided by hand-writing this server instead of using `rmcp` is asserted, not
  benchmarked.** Leteo uses `rmcp`; this crate does not, because the same binary answers a `PreToolUse`
  hook on every edit, and a `tokio` runtime spun up per process was judged too expensive to pay that
  often. Synchronous stdio JSON-RPC over a fixed, small tool list avoids spinning one up at all, but
  nothing here has measured what either approach costs per invocation.
- **The renewal window is a cadence, and a cadence is a gap.** A routine write inside two minutes of
  the last answer is not re-verified, and neither is a Claude Code `Stop`: it keeps a successful
  answer for the window and records its own, because reading the tracker at every turn end spent the
  GitHub GraphQL budget of a whole account. A claim lost inside the window keeps the turn going once
  more per window. Only a successful read is kept; one that failed or answered unknown is read again.
  The Codex `Stop` path and `estigia goal` read the tracker on every call and are not covered. A claim
  lost during those two minutes is not caught until the window closes or a boundary arrives. **Claim
  validity is the whole of what the window excepts**: the contract check sits above it, a boundary
  never consults it, and an unreadable control surface permits no write inside it.
- **A rate limit is named, never waited out, and its reset is only as readable as `gh` makes it.** A
  read that fails with `gh` saying a primary or secondary rate limit was exceeded answers
  `rate-limited` (exit `3`, still a refusal and never clearance) instead of `read-failed`, and the
  detail carries a reset instant only when `gh`'s own stderr printed one. Nothing asks GitHub for the
  reset (`rate_limit` is itself a call), nothing retries, and a rate-limited *write* is still
  `write-failed`, because it may have landed. The stderr is matched on the phrases `gh` prints today; a
  rewording falls back to `read-failed`, the safe direction.
- **A JSON file Estigia edits keeps its shape and loses its shaping.** The indentation, a byte-order mark, the
  line endings and whether the file ended with a newline are read off the original and given back, and key
  order is preserved (`serde_json` with `preserve_order`): every operator file of every adapter, written the
  ordinary Windows way, comes back **byte for byte** through an install and an uninstall
  (`every_agents_own_files_come_back_byte_for_byte`), and the readers parse through `render::parse_settings`
  so a BOM file still reads as gated and tooled (`a_bom_settings_file_still_reads_as_gated_and_tooled_after_setup`).
  The formatting inside the document is not kept: a hand-written one-line `"allow": ["Read", "Grep"]` comes
  back over four, so a diff shows the operator's arrays reflowed around the entry added. Markdown is spliced
  textually.
- **`skills/` is left behind, even empty.** It is the agent's namespace, not a file Estigia created.
  What comes out is the tree that was written.

## What the gate covers

- **The gate covers two directories: the checkout the claim was made in, and the isolated one
  `start_branch` created or a resumed run adopted.** A run that makes its own worktree by hand (one git
  has never registered, or whose ownership marker names no issue) gets that write gated but the directory
  is never recorded, so subsequent writes there read as `Outside`. Use the tool, or resume through a
  checkout the transport already knows about.

  `start_branch` records the claim's checkout as well as the worktree, filling and never overwriting. A
  `claim` whose tracker write lands and whose readback fails returns before the pointer effect runs, so no
  checkout is recorded and the worktree would become the run's **only** covered directory; the server is
  then outside its own run and every later call is refused `run-id-names-another-checkout`
  (`one_server_survives_the_isolation_it_created` drives one real server through both calls and reads the
  pointer **between** them). The dispatch guard measures `repo_dir`, not `covered().count()`, so a record
  that never named its checkout stands aside like a run that has claimed nothing (standing aside is not
  clearance), and a renewal answered `ok` completes such a record with no tracker **write** and no
  invented authority (`a_renewal_completes_a_record_the_tracker_has_just_agreed_with`,
  `a_stranded_run_recovers_from_the_checkout_it_is_running_in`).

  **The same renewal discovers a checkout this run never built.** `claim`, `reclaim` and every renewal read
  every checkout `git worktree list` has registered, find the one whose ownership marker names this issue,
  and fill `worktree` from it (`a_renewal_records_the_checkout_a_resumed_run_adopted`,
  `a_reclaim_records_the_checkout_it_takes_over`,
  `a_write_inside_an_adopted_checkout_is_gated_once_a_renewal_records_it`). It records nothing for **more
  than one checkout naming the same issue**, nor for **a read that fails**; the scan's `?` stops at the
  first unreadable marker, which disables discovery for every issue and run this clone holds until that
  marker reads again, without turning a successful renewal into a refusal. It cannot see a checkout git
  never registered or whose marker was never written in the shape `read_ownership` reads.

  **The renewal's fill accepts the caller's working directory on no directory evidence.** `verify_claim`'s
  tracker read binds no directory, and run ids are public, so a call under run A's id from directory B,
  while `A.repo_dir` is unset, writes B into A's record permanently (A's own server is then refused in its
  real checkout), and `discover_adopted_worktree` plants a `worktree` the same way from B's clone. Not
  closed: the shape needed is the one the isolation fix stops producing, not filling on renewal is the
  stranding itself, and deleting the pointer is the only correction. **The same rule is spelled two
  ways**: the dispatch guard asks `repo_dir.is_some()`, the write gate in `src/harness/mod.rs` asks
  `covered().count() > 0`, so a worktree-only record makes writes in the base checkout read as `Outside`
  and go ungated; unifying them is a change to the write gate that belongs to its own issue.
- **A branch differing from a registered one only by case or Unicode normalization is refused, on a
  filesystem that distinguishes them too.** `worktree_path` composes `<branch>~<run-id>`, so `Fix-1-a` and
  `fix-1-a` under one run ID are two strings and one directory on default APFS and NTFS, and the registry
  (keyed by `normalise_path`, which folds neither) answered `worktree-path-occupied` with
  `occupied_by_branch: null`, the orphan diagnosis whose action invited removing the first run's live
  checkout. `may_occupy` compares the **spellings** (separators unified, NFC-normalized, lowercased,
  normalized again) and refuses a collision `case-insensitive-branch-collision`, naming both branches. The
  fold ignores the filesystem on purpose (the conservative direction). The case half is
  `str::to_lowercase`, not full Unicode case folding, so a pair the volume merges but `to_lowercase` does
  not (`Σ`/`ς`, `µ`/`μ`, `ſ`/`s`) is not refused. Held by
  `two_branches_differing_only_by_case_are_refused_rather_than_called_an_orphan`,
  `two_branches_differing_only_by_normalization_are_refused_rather_than_called_an_orphan` and
  `a_case_only_difference_is_the_collision_and_a_separator_one_is_not`.
- **The closed-window refusal covers the transport's own spawns, and the gate reads an agent's line by the same
  matcher.** While the git operation window is closed, `window::tripwire` refuses a `gh` write or `git push`
  that `transport::run` is asked to start (`outbox-bypass`), and a `gh` line an agent runs itself is
  classified through `window::is_outbound_write` as an `Action::Remote` and denied
  `git-operation-window-denied` (`gh pr merge`/`create`, `gh release create`/`edit`, `git merge`, `git tag`,
  `gh issue create`/`develop`, a mutating `gh api`, `gh cache delete`, `gh ssh-key`/`gpg-key` and `gh repo
  deploy-key`/`autolink` writes: `the_gh_writes_outside_the_old_verb_table_are_remote_and_denied_while_closed`).
  The matcher is a table of the write verbs it knows, so a `gh` subcommand a later release adds reads as a
  non-write until named; it does not read an alias, a script, a shell variable or substitution that spells the
  call, or a `gh` word that is not first in its command, and a `gh api graphql` whose query comes from
  `--input` or an `@file` field is treated as a write while a mutation behind a shell variable passes. While
  closed, `create`, `comment`, `transition`, `claim`, `claim --takeover`, `unassign` and `start_branch` queue
  their GitHub writes, a publication is refused with the number of writes waiting, and `release_ci` is
  refused (`release_ci_denies_outside_the_period_before_reading_anything_else`).
- **The window's scope is the repository a write is aimed at, and the gate's reading of an agent's own
  shell line still belongs to the checkout it runs in.** The transport resolves the pin `Context::repo` to
  a checkout whose `origin` names it (the caller's own, else one among `known_repositories`) and reads that
  checkout's rows; a repository with no such checkout is closed rather than inheriting the caller's
  schedule. The gate classifies a hand-written `gh` line through
  `harness::operation_window::check_protected`, which reads `GateContext.operation_window` built from the
  **caller's** checkout, so a shell `gh issue create -R <destination>` is judged by the caller's schedule;
  only the transport's own `create` and the flush moved. A destination whose checkout is not registered, or
  whose `origin` is not a GitHub remote, reads as no rows and closes.
- **The store holds a write queue and one permitted rewrite of an entry.** Schema 9 adds `outbox` and
  `provisional_issues`, and the `entries_no_update` trigger lets exactly one update through, moving an
  entry's `issue` from a provisional number to the real number `provisional_issues` records
  (`an_entry_may_move_from_a_provisional_issue_to_its_recorded_real_number_and_nowhere_else`). Provisional
  numbers start at `PROVISIONAL_BASE` from one counter in `store_meta`, so two machines can allocate the
  same number and file the same issue twice. `Store::rekey` moves a provisional issue's rows in one
  transaction and `outbox::rekey_files` follows with the run pointers and the ownership record; a branch or
  worktree directory named with the provisional number keeps that name. A provisional issue opens its
  ledger without any GitHub call, and an unallocated or unrecorded number is refused.
- **The transition record is the store's own, and it is not a log of everything that moved a state.** Schema 16
  adds `transitions` (`at`, `run_id`, `from_state`, `to_state` per issue), append-only like `entries`, and
  `Ledger::set_state` writes the row in the same transaction as the state, so a history that cannot be written
  leaves the state unchanged and the transition fails. Every state change a command makes goes through
  `set_state` (`transition`, `transition --to` on `claim`, the merged-release close, the begin-review step, and the
  flush's reconverge), so none bypasses it; setting the state an issue already has records nothing, which
  makes the flush's re-projection invisible. What it cannot see: a transition names no caller, so `run_id` is
  whoever holds the issue at `at` and is empty when nobody does; the state an import reads from the labels is a starting point, not a transition, and states set before
  schema 16 have no row, and
  the first change after it records `from_state` as the state the store held; a label edited on GitHub by hand
  never reaches the store, and another machine's changes are another machine's store. Nothing reads the
  record yet.
- **The flush sends the queue in one line, and a stalled write holds everything behind it.**
  `outbox::flush::run` refuses while the window is closed, takes a per-repository lease in `store_meta`
  (a pid and an expiry; a dead or expired holder is taken over), rekeys pointers and ownership files, and
  sends unsent writes oldest first, marking each `sending` before and `sent` only after its own readback (a
  comment by its `<!-- estigia-outbox:{op_id} -->` marker, a create by `gh issue view` on the number printed,
  a link by the Development sidebar, a reconverge by `transition`'s readbacks). A write whose outcome cannot
  be read stays `sending` and stops the flush with `outbox-pending`; a failure that learned nothing on a first
  attempt goes back to `queued`, a write that has been `sending` never does. The line is global: one ambiguous
  write stalls every write behind it, other issues' included. A `sending` create is resolved by searching for
  exactly its title, and that search is the index GitHub updates after the issue exists, so a flush seconds
  after an ambiguous create can find nothing and file a second issue; two issues with the title stop the
  flush. The search adopts only an issue the authenticated user (`--author @me`) created within five minutes
  before the row was enqueued whose number the store does not hold (the bound is the enqueue time, not the
  first attempt, so a same-user same-title issue filed after enqueue is still adopted, and an open-window
  `pending-create` mark has no row time and is bound by author and store alone). A reconverge rebuilds labels,
  board and owner from the store, overwriting changes made on GitHub meanwhile; the comment readback lists
  the thread without a `since` filter (a thread of a thousand comments or more is refused as unread). Writes
  give up after `WRITE_SECONDS`, inside `LEASE_SECONDS`, and a flusher renews the lease before it sends and
  before it settles a row. **Open:** the renewal and the `gh` call are not one step, so a flusher that loses
  the lease in that instant can send once more, and a write that times out has an unknown outcome resolved by
  the next flush's readback, which for a create is the lagging search.
- **The queue is sent before anything else writes, and a detached flush is the machine's, not the
  session's.** With the window open and a write waiting, `comment`, `create`, `transition`, `claim`,
  `unassign`, `start-branch`, `publish-review`, `republish-review`, `release-ci`, `draft-pr` and
  `ensure-states` run the flush first and are refused `outbox-pending` when it stops; other operations do
  not. The list is written in `outbox::flush`, not derived from what each operation spawns, so a new
  operation that writes to GitHub is unflushed until added. With nothing waiting the check opens the store
  read-only and asks GitHub nothing. `estigia flush --detach` reports the pid it started, not whether the
  flush then succeeded; the lease keeps it from running beside another.
- **"Offline" means the window is closed, not that the network is down.** Reads still go to GitHub while
  closed. `list_state` answers from the local store and lists issues created while closed as `provisional`;
  another machine, a person and the board see none of it until the flush, so labels, board column, assignee
  and comments disagree with the store for as long as writes are queued, and a second machine can file the
  same issue under the same provisional number. A task whose work is a commit cannot be marked done while
  closed (the window denies the commit); one closed by an `evidence` note can. `SessionStart` and `Stop` count
  unsent writes of every repository from a read-only open and say so (`Stop` as a `systemMessage`, never
  blocking), and with the window open they start `estigia flush` detached and report that it was started, not
  that it succeeded.
- **One row is read by nobody.** `Delivery preset` stores nothing of its own and only writes the rows it
  stands for. `State names` is read by `assist`, `ready_pr` and `review_stages`; `Corrections after ready`
  and `Ready by` by `draft_pr` and `ready_pr`; `External reviewers` by `review_stages`; `Merge by` by
  `review_stages` and `deliver`, which hands back at `authorisation` when it is not `agent`; all through
  `Context::layered`. `READ_BY_NOBODY` in `tests/honesty.rs` lists `Delivery preset` and fails when it is
  read and not taken off the list.
  - **`review_stages` reads, and decides nothing.** Its answer is a reading of the pull request at the
    moment of the call; nothing is recorded, so a run that does not ask again acts on a stale one. A check
    stage's comments are its author's **unresolved review threads** (a comment outside a thread, or a
    summary review, is not read, and a thread resolved without addressing it counts as addressed). For
    `any` and `org/team` a review from any account other than the author, a `[bot]` or a check stage's
    author satisfies the stage; team membership is not read. An approval completes a review stage only
    for the current head; GitHub does not expose the historical base that reviewer read. Exact-head
    polling, full thread conversations, guarded reply/resolution and their non-atomic mutation limit are
    in [assisted review](assisted-review.md). A list with a next page (more than a hundred threads,
    comments per thread, reviews, requests or check runs) refuses the reading. A blocking check not
    reported on the head holds the stages behind it and says it was never reported, since a check not yet
    started and a stage name matching no check look the same; an advisory check never holds them. Held
    against a fake `gh` by `src/transport/stages/tests.rs`.
- **No other setting is read by nobody.**
  An operator sets thirty-one rows and `config list` reports all of them. Measured: `context.get` is
  called for the labels in `READ_BY_THE_TRANSPORT` — `worktree location`, `Review budget` (for the review forecast line) and `Git operation window`. The gate reads
  `Irreversible commands`, and the payload's prose names the rest of the acted-on rows. The four rows that
  had exactly one answer (`Delivery route`, `Review delegation`, `Merge strategy`, `Review protocol`) were
  retired and hard-coded, and the guard fails on any row the gate, the transport and the prose all leave
  unread (`every_setting_is_read_by_the_gate_the_transport_or_the_prose`). `Delivery authorisation` and
  `Merge by` are read in code (`deliver` hands back `authorisation` unless they are `auto` and `agent`), and
  a merge made by hand through the shell is held only by the prose. `Model routing` is closed by a narrower
  claim than its name suggests: `setup` reads it for the `model:` and `effort:` lines of the definitions it
  writes, and the managed block of the **installed** `SKILL.md` carries a generated sentence telling the
  agent to honour the row (`selected_documents` builds it and `configuration_body` splices it, so it is in
  no file in this repository). Five of the eight supported adapters share the neutral skill root
  (`adapters_on_the_neutral_root()` computes it; `docs/what-it-writes.md` is held to the same number), so
  they share one installed contract and one rendered sentence, which names Claude Code as the one adapter
  that receives the definitions `setup` installs. Naming a role in `Model routing` installs its definition
  and removing the entry retracts it. Nothing here starts a model; what changed is that something reads
  the row.

## doctor, and what it checks

- **`doctor`'s rows are forced through the binary, except two, and the assembly is held row by row.**
  `tests/pipe/status_doctor.rs`'s `FORCED` list requires every new row to be forced or exempted. The two
  exemptions limit what function and report-assembly tests establish:
  - `stale-run-pointer` needs a `gh` that answers `issue-not-open` for one specific issue number;
    `harness::doctor`'s own module drives `stale_run_pointers` through a scripted `gh`
    (`a_readable_stale_pointer_is_reported_and_names_the_release_command`,
    `a_readable_live_pointer_is_reported_fine`, `a_tracker_read_that_fails_is_reported_as_unread_not_stale`).
  - `gate-version`'s unrelated-history pipe fixture is `skipped`, not `Broken`.
    `gate_version_reports_fine_and_genuinely_stale_against_real_commits` tests real ancestry but skips on
    dirty builds and shallow clones missing the build commit, including CI's checkout.
    `an_older_binary_than_the_checkout_is_reported_broken_with_a_reinstall` always holds `gate_age_verdict`
    with `Some(false)`, not the call's ancestry direction; swapped arguments can escape in those conditions.

  **Missing build provenance leaves the suite green.** Removing `ESTIGIA_COMMIT` emission makes
  `VERSION` say `0.1.0 (unknown build)` and stale checks skip. Corruption is caught on a clean build;
  absence is not. Page assembly is held by `what_status_prints_is_what_somebody_needs_to_act_on`,
  `what_the_companion_verb_prints_is_crossed` and `somebody_elses_hook_is_not_this_agents_gate`, not by
  a requirement that the build carry its commit.
- **Estigia cannot check who reviewed.** Attribution is not authenticated identity; see
  [Estigia can prove what it launched, not who answered](#review-targets-panels-and-the-lane-that-runs-them)
  for the measurement and the limits of distinctness checks.
- **Refusals about a tracker or host with no transport: only the tracker's can fire.**
  `Tracker::transport()` returns `None` for read-only assisted Linear; `RepositoryHost` is GitHub-only,
  leaving host-bound MCP branches unreachable. `every_reader_of_a_transportless_tracker_asks_the_same_function`
  holds only a floor of four production callers, not the absence of a second hand-rolled decision.
  Its doc comment's one-variant claim is stale; comments are not checked.
- **`doctor` is held at the function and at the composition, and not at the call site.**
  `the_tracker_in_force_is_the_one_this_checkout_names_not_the_contracts` and
  `the_stale_run_pointer_check_asks_gh_about_the_checkout_s_own_repository` cross that the tracker `doctor`
  reports on is the checkout's own, not the contract's. Neither reaches `cli::doctor()` itself: replacing
  its call with a contract-only read leaves the suite green, since the function reads
  `std::env::current_dir()`, takes no parameters, and is reachable only through a subprocess. A defect
  purely in that call site's arguments is caught by nothing.

- **`doctor` checks twenty things, not everything.** Skill, installed skill assets, `gh`
  authentication, a git remote, this repository's push guard, the contract each configured agent
  reads, whether the root the gate decides in carries the rows those agents read, whether the gate
  each of them registers would actually run, whether the tool server each
  of them registers would actually start, whether the operator has the gate standing down right
  now, whether every run pointer on the machine can still say what it holds, whether a pointer that
  *does* parse still names an issue the tracker holds open, whether branches and checkouts no
  delivery owns have accumulated, whether any call has reached that
  gate and gone undecided, whether the binary answering this command is the checkout's own
  build, whether the local workflow store opens, whether Pi's own settings exclude Estigia's
  extensions or override its tool server, whether the Pi subagent runners installed give their children
  the gate, whether a tracked lockfile is sent to every reviewer in full because
  `estigia.generated.toml` does not declare it (an advisory row that is absent when there is nothing to name), and which permission mode each of
  Claude Code, Codex, OpenCode and Pi is configured with (see the entry below). It does not check the tracker's labels, or whether the
  repository it found is the one the issues live in. The skill comparison is against this binary's embedded
  copy, so a stale binary reports its own age as current; an install whose operator table no build
  recognises reports unreadable rather than current.

- **A host's automatic mode keeping Estigia's gate is pinned by tests and measured live on three hosts of
  four.** On 2026-10-14, an unclaimed `hello.txt` write with `Issue requirement: ask` was refused
  `issue-required` by OpenCode 2.0.26 `--auto` and Claude Code 2.1.296 `bypassPermissions`; neither made
  the file. Pi 1.1.0 has no permission prompts. Codex 0.159.2 was unauthenticated (`401 Unauthorized`),
  so `--full-auto`/`never` behaviour is unmeasured. The live `ask` result does not distinguish an
  unanswered question from a refusal.

  Suite fixtures hold identical answers with/without `bypassPermissions`, `auto`, `dontAsk`,
  `acceptEdits` and `approval_policy: never`: `required` denies, `ask` asks Claude and denies Codex.
  Both OpenCode plugin generations throw on exits `1`/`2` and register no permission hook. These
  tests cannot hold a future host's ordering of permission checks and gate calls.
  Doctor reads files only: Claude's project `settings.local.json`, `settings.json`, then user
  `permissions.defaultMode`; OpenCode's `permission`; Codex's top-level `approval_policy`/`sandbox_mode`.
  CLI flags, profiles, managed policy and environment overrides are invisible. `no permission mode is
  set` means no file value, not the runtime default; malformed files are unread, other hosts have no row.

- **Installed-asset staleness is a recorded digest comparison, and a mismatch is Broken with `sync`.**
  Every install records an aggregate digest of the embedded skill assets (sorted paths, NUL-delimited,
  metadata-free) and `doctor` compares it against the running build's. No record at all is skipped, not
  cleared. At the reviewer-definition door bytes that moved since the install wrote them name an edit,
  while a record that predates digests says unknown rather than picking. Neither answers whether the
  running binary itself is current.
- **The `gate-version` row compares what is installed against the checkout, not what any host actually
  invokes.** The running binary's embedded commit is compared with the checkout's `HEAD`: a **no** does not
  establish that this build is earlier, only that the checkout's head is not in the history this build
  held. It does not discover which path another agent's hooks invoke (that is the `gate` row's *not this
  build* warning). It says *skipped* rather than passing when the build carried no commit (a published
  crate, or built without git; a `release.yml` archive still has one), came from a dirty tree, the
  directory has no head, or `git merge-base --is-ancestor` could not answer (exit neither `0` nor `1`,
  most often because the commit is not an object this repository holds: two unrelated histories or a
  shallow clone). The session-start note (`build_note`) is silent in exactly one of those: a clean build
  with a commit, a readable head, and a repository that does not hold the commit, which is every project
  that is not Estigia's own (a shallow clone of Estigia lacking the commit is silent too; `doctor` still
  says *skipped*).
- **`status --json`'s copy of the identity is asserted nowhere.** `estigia --version`, the MCP
  handshake's `serverInfo.version` and the TUI banner are compared to `estigia::build_info::VERSION`
  (`version_reports_the_build_identity_not_only_the_crate_version` in `tests/pipe/cli_process.rs`);
  `status --json`'s `"estigia"` field (`src/cli/mod.rs`) has no equivalent, and reverting it to bare
  `env!("CARGO_PKG_VERSION")` leaves the whole suite green.
- **The embedded identity's `dirty` flag can go stale between builds, and only that half.** `build.rs`
  watches itself, shipped transport Rust, per-worktree `HEAD` and `packed-refs`, disabling Cargo's
  default rerun. Other tracked content edits can rebuild with cached `ESTIGIA_DIRTY=0` and report
  `gate-version` current. `watch_git_identity` holds the commit half only by hand measurement; deleting
  its call leaves the suite green. `--untracked-files=no` also calls a build clean while an untracked
  transport `.rs` contributes to `TRANSPORT_VOCABULARY`, untested. Finally `capture_identity`'s
  `git_query` conflates failed and empty output: successful rev-parse plus failed status embeds clean.
  No test closes these three dirty-state gaps.
- **The Windows CLI's 8 MiB stack is headroom, not a measured budget.** A debug `estigia` overflowed
  MSVC's default 1 MiB main stack on every command once `session start` gained `--session-file`
  (the Windows lane: `thread 'main' has overflowed its stack` across `tests/lifecycle.rs`), so
  `cli::run` runs the command on a thread with Unix's default stack on Windows. No test measures how
  deep a command goes, so the margin left is unknown; the Windows lane running the binary is the only
  detector.

- **Codex's trust for Estigia's gate is written from a re-implementation of its hash, and `doctor` cannot
  ask Codex whether it would run the hook.** codex-cli 0.159 skipped Estigia's hook without a message
  because no `[hooks.state."<key>"] trusted_hash` named it while `doctor` said `1 live`; setup now writes
  the entry and `doctor` reads it back. What that leaves unproven:
  - **The hash is Estigia's port of `hook_hash` and `version_for_toml` in `codex-rs`
    (`hooks/src/engine/discovery.rs`, `config/src/fingerprint.rs`), read at `rust-v0.159.2`.** Six vectors
    taken from what `codex app-server`'s `hooks/list` reported for the same `hooks.json` (different
    events, a matcher Codex ignores for `Stop`, timeout `0`, absent timeout, `async`, `statusMessage`, a
    command with non-ASCII, quotes and a backslash) and a real-binary measurement (trusted by the table
    `setup` wrote, gate invoked) hold it, and it is **not** held against any other Codex version. A release
    that adds a field to the hashed identity makes the hash drift silently: Codex reports the hook modified
    and skips it while `doctor`, comparing against Estigia's own hash, says `live`. No test fails on that,
    `estigia setup codex` does not detect it, and the vectors would have to be re-taken.
  - **`doctor` assumes a Codex that enforces trust**; on an older one a hand-edited `config.toml` without
    the table is reported broken for a gate that runs. The boundary was read from the source:
    `trusted_hash` first appears in `config.schema.json` at `rust-v0.129.0` and is absent at
    `0.128.0`/`0.127.0`/`0.125.0`/`0.123.0`; `ConfigToml` carries `#[schemars(deny_unknown_fields)]` and no
    `#[serde(deny_unknown_fields)]`, so the loader ignores the extra table. No pre-0.159 binary was run,
    and `--strict-config` is not covered.
  - **The key is built from the `hooks.json` path Estigia resolves** (`$HOME/.codex/hooks.json`) and
    `setup` does not read `CODEX_HOME`; if Codex resolves its home elsewhere or through a symlink it
    normalises, the hook is untrusted there and `doctor` still says `live` (measured only with `CODEX_HOME`
    set to the directory setup wrote). Enterprise-managed, project-level and plugin hooks are not looked at.
    Nothing here asks Codex to run a write it should refuse (the invocation measurement used a stand-in gate
    that always denies), and `status` still says `gate on` from the registration alone.
  - **`config.toml` is edited through a TOML parser (`toml_edit`)**, so every spelling of the key names the
    same entry and comments, order, spacing, CRLF and a BOM survive, and before writing `setup` re-parses
    the whole file and reads the trust back, refusing a file that does not parse or carry it. The
    `[mcp_servers.estigia]` table is still edited by the line (a comment inside it is dropped with it when a
    key follows), covered by the parse-before-write guard, and the re-read is Estigia's own, not Codex's.

- **A stalled `gh` is a broken row within ten seconds, and the screen that waits on it is not measured.**
  `src/bounded.rs` runs the program and one that does not answer within `PROGRAM_TIMEOUT` is `Broken`
  (`a_program_that_stalls_is_broken_within_its_limit` through `program_within`, not `examine`: the check
  starts `gh` from `PATH`, not through the transport's scripted `gh`). While the checks or a model catalog
  load, `App::press` ignores every key but `q` and Ctrl-C
  (`keys_pressed_while_the_checks_run_change_neither_the_page_nor_the_report`;
  `a_checks_worker_that_panics_reaches_the_screen_as_a_report_not_a_stuck_wait`). `run` itself is read, not
  driven: that the channel is read by the loop, that `poll` ticks while it waits, and that quitting during
  the wait restores the terminal. Quitting abandons the worker rather than cancelling it.

## Identity and the tracker

- **The identity and the verdict reach different surfaces, and a refused call carries neither.**
  `answer_text` in `src/harness/mcp/mod.rs` adds `build` from `src/build_info.rs` only to successful JSON
  object answers without `compact: true`. Refusals reveal no build identity. The `gate_version` verdict
  reaches doctor and `hook::session_start_response`, not tool answers; the session note is a one-time
  reading of local `GateContext::repo_dir` `HEAD`, never the remote default branch. Later commits, pulls
  or branch switches leave it stale. Any feature commit absent from a clean main-built binary reports
  `Broken`; dirty or provenance-free builds report `Skipped` without an action. `Broken` requires both
  reinstall and session restart, so its note names no ineffective in-session command or `Resolution`.
  These are code readings; no test here proves the note stays current during a session.
- **The compact `task` answer is chosen by skill prose, not by adapter code.** It carries `version`
  (`COMPACT_TASK_VERSION`) and the changed row; a retried acknowledgment answers the same row and a
  reopen keeps the closing commit (`a_lost_compact_acknowledgment_retried_answers_the_same_row_in_the_same_place`).
  No adapter sends `compact` on `task` by itself, and two processes writing one checklist at once are not
  tested.
- **A compact receipt is expanded from this run's pointer, not from the store.** `expanded_receipt` in
  `src/harness/mcp/mod.rs` fills `pr`, `head`, `base` and `digest` for a call that names only `epoch`
  when the calling run's pointer holds that epoch's review transaction for the issue
  (`a_receipt_named_by_its_epoch_is_filled_from_the_pointer_holding_it`). Another run, or a pointer that
  lost its transaction, gets the old missing-argument answer. The compact view carries `compact_version`;
  `collect`, `stop` and an unpublished answer keep what they cannot rebuild, and a compacted `execute`
  expands back to its arguments (`a_compacted_execute_answer_expands_back_to_the_arguments_it_dropped`).
  The CLI has no compact `review_status` view.

- **A phantom holder is reconciled against the tracker only where the ambiguity is real.**
  `guard::adjudicate_action` asks the tracker about each named holder before refusing
  `several-runs-hold-this-checkout`, and drops one only on an explicit `issue-not-open`; every other answer,
  including a failed read, keeps it counted. The decision is per holder: one answering `issue-not-open` and
  another's read failing leaves `holders.len()` exactly **1**, the `1 =>` arm calls `gate`, and whatever
  `gate` refuses first is what the operator is told, not the read that failed.

  A flake of that shape reached CI twice on `ubuntu-latest` alone (runs 32738461410 and 33482324896) and was
  never explained: the guard tests' fixture had installed no contract, so `control-surface-not-installed`
  surfaced; `guard/tests.rs`'s `context` now writes it
  (`an_unanswered_tracker_read_names_the_read_and_not_the_contract`), but why one of two identical reads fails
  on Linux and on neither other platform is unidentified and the lane has been disabled since 2026-09-01.

  **A single holder is deliberately not reconciled.** An unrecognised close still refuses
  `issue-not-open`; dropping that holder would instead yield `Outside(NothingSworn)` unless another
  early denial applies. Changing the reconciliation threshold to one reddens
  `a_single_stale_holder_still_refuses_through_its_own_issue_not_open`. A recognised delivery close
  (`src/transport/claim/close.rs`) answers `closed-by-own-delivery` and remains `Live`.

  **Doctor has three pointer buckets:** `issue-not-open` (release named), own-delivery `stranded`, and
  `unread`. `the_fine_sentence_claims_only_the_buckets_the_row_empties` holds the empty-bucket `Fine`
  claim; `own_delivery_close_reads_the_transports_own_prefix_and_nothing_else` holds the prefix predicate,
  not end-to-end stranded delivery. Two blind spots remain: a lapsed stranded acquisition becomes
  `not-current-live-holder` and escapes every bucket; a deleted/moved/non-repository checkout makes
  `same_git_repository` false and excludes its pointer entirely.

  **Two dead pointers reconcile to zero and stand aside; one still refuses.** With no unreadable
  holding, zero means `Outside(NothingSworn)`. A live holder survives beside any number of dead ones.

  Multiple healthy runs also cover their shared base checkout. Release is offered only for
  `not-current-live-holder` (missing acquisition, expired horizon or lost race); other survivors get no
  command because checkout isolation is MCP-only. `plan_release` searches live and stale ownership;
  absence clears only the pointer, presence writes under the operator's identity after repository
  agreement. Multiple holders cost one tracker read each plus the survivor's gate read unless renewed
  (typically two to three). This precedes `standdown::over`; a hung read cannot be escaped by stand-down,
  untested. Push narrowing happens earlier: one matching branch skips reconciliation and its
  `ClosedIssue` filter, held by
  `narrowing_before_the_tracker_can_deny_what_reconciliation_would_have_allowed`.

  **The sibling holder list is driven only at its floor.** `holders_for_action` builds one for a `gh pr
  merge` with no ordinary holder here, filtered to siblings whose own complete receipt names the pull request;
  `sibling_selection_binds_pr_before_head_and_attributes_no_ambiguous_holder` gives it two receipt-matched
  siblings with an unreachable tracker (both unknown, denial follows). It has never been crossed against a
  *successful* tracker read, and no fixture takes a sibling list past two or drops one as `ClosedIssue`.
  `run_with_path_locked` clears `GH_REPO`, `GH_HOST` and `GH_TOKEN`
  (`an_operators_gh_repo_never_reaches_a_spawned_gh`), a before/after on the mechanism; other variables a real
  `gh` reads are not audited. The harness's own `GH_REPO` pin is set or removed by `apply_gh_repo` on every call
  (`a_named_gh_repo_pin_does_not_survive_a_call_that_names_none`) but is still a process global, so concurrent
  tests racing on `set_var` remain under the Rust-2024 precondition the `test_env` entry names.

- **The delivery sequence asks for its own release, and nothing adjudicates that it happened.**
  Developer step 6 is prose. `transition --to done` restarts renewal for the held issue, and
  `release_ci` restores receipt/pointer fields and verifies; neither forgets the run. A recognised
  delivery close leaves a `Live` pointer reconciliation cannot drop. Measured: a later claim makes two
  holders and its first write refuses `several-runs-hold-this-checkout`. Only doctor's stranded bucket
  names the leftover; delivery still reports success when release is skipped.
  `releasing_a_delivered_issue_puts_its_checkout_and_branch_down` and
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down` in `tests/pipe/part_09.rs` read back
  directory, registry and branch removal. Production cleanup remains best-effort/self-reported, and
  no guard test covers a single recognised-delivery holder (the single-stale test covers other closes).

  **Naming the release in step 6 costs the `dev:<runtime>` label on every delivery that follows it.**
  `release`'s `unassign` ends through `converge_ownership_projection` (`src/transport/claim.rs`): with no
  holder left, every `dev:<runtime>` label is emitted as `--remove-label` from an inline
  `BTreeSet::difference`. This is not measured: `src/transport/claim/tests.rs` exercises
  `converge_projection`, a helper with zero production callers, and disabling the production emission leaves
  the whole suite green. A query grouped by `dev:<runtime>` no longer shows which runtime delivered a closed
  issue; only the run-id in the close evidence survives.

## The gate's boundary

- **A redirect to `/dev/null`, `/dev/stdout`, `/dev/stderr` or `/dev/fd/<n>` is not read as a write.**
  `shell::redirects_into_a_file` matches the target word exactly, quotes allowed; a word that only begins
  with one of them (`/dev/nullx`, `/dev/null/x`) or a target it cannot parse stays a write. Not seen: a
  redirect through a variable or substitution (`> $sink`), or a `/dev/null` that is a regular file. Held by
  `a_redirect_to_a_stream_is_not_a_write_and_any_other_target_is`.
- **The redirect reader models the quote rules that hide a `>`, and fails closed where it cannot.** It ends
  a double-quoted region only at an unescaped `"`, reads `$'...'` with backslash escapes, treats an unquoted
  `#` at a word start as a comment (a `>` inside it is still a redirect: `echo x # > y` is a write), and
  skips a here-document body as data; a line that ends inside a quote after hiding a `>` is a write. Not
  modelled: a body `here_document` refuses (an unquoted delimiter with `$`, a backtick or a backslash in the
  body) is scanned as code, and `$"..."`, `${...}` and `$(...)` nesting are not tracked
  (`a_redirect_after_a_quote_the_scan_could_misread_is_still_a_write`,
  `a_quoted_arrow_stays_data_whichever_quote_holds_it`).

- **An unclaimed run is refused a commit or a push only where the push guard is installed, and only for the
  spellings the command line names.** `unclaimed-commit` is raised when no claim holds the checkout and
  `guard::state` answers `Installed` or `Chained` (running `estigia guard` is the opt-in), on any branch,
  since any unclaimed commit or push skips the flow. A guard reading `Unreadable`, `Inert`, `Foreign` or
  `Absent` stands aside (denying would lock every commit because of a hook file; `doctor` reports the first
  two as broken), as does a repository whose tracker cannot adjudicate an unsworn write, whose skill has no
  contract file, or a checkout with no remote for a GitHub tracker that names no repository
  (`unclaimed::applies`). `--abort`/`--quit`/`--skip` recovery and `git pull` are not refused;
  an alias, script, editor or `gh api` writing a commit passes; `git push --no-verify` skips the hook half; a
  **claimed** run is not held to its worktree (its claim covers the checkout it was made in too). Held by
  `a_commit_without_a_claim_is_refused_where_the_guard_is_installed`,
  `a_push_without_a_claim_is_refused_on_any_branch_where_the_guard_is_installed` and
  `the_pre_push_hook_refuses_an_unclaimed_push_where_the_guard_is_installed`; nothing drives the two shell
  entry points (`estigia gate` with no session, the dialect hook with an empty session id) end to end.
- **`Unclaimed write policy` holds the writes the hook sees, and cannot hold the rest.** With the row
  `required` or `ask`, a run holding no claim is refused (`issue-required`) or asked before a routine write
  the gate can place inside a checkout whose tracker can name an issue there and that has an installed
  contract (a file edit or write, or a shell line carrying a `REPOSITORY_SHELL` fragment such as
  `git commit`), and before one it **cannot place** when the call was launched in a repository or from a
  directory whose repository could not be established. Such an unplaceable write is allowed under `ask`
  only from outside every repository, and the checkout it may have dirtied is named at the next session
  start (at most eight paths, then a count; it cannot prove they came from an allowed write rather than
  the operator's own work). What it does not hold:
  - **A GitHub tracker that names no repository infers one from the remote, so zero remotes is a tracker
    that cannot name an issue.** A write landing only in such repositories is not held (no ask, no deny) and
    the SessionStart text says there is nothing to claim. The read is `git remote` in the repository each
    target lands in, not the launch directory, so a no-remote checkout writing into a nested checkout that
    has one is still held. A failed read or a `.git` entry git will not use is not zero remotes. A
    non-GitHub remote still counts, because the URL is not read. Open: a refusal that survives there (an
    unplaceable write under `required`, a nested target the tracker can name) still names `claim`/`create`,
    which cannot run from the no-remote checkout. Held by `a_checkout_with_no_remote_is_not_held_for_an_issue`,
    `adding_a_remote_holds_the_same_write_again`,
    `a_named_repository_holds_the_write_where_the_checkout_has_no_remote`,
    `a_write_into_a_nested_checkout_that_has_a_remote_is_still_held`,
    `a_failed_remote_read_is_not_zero_remotes` and `a_guarded_checkout_with_no_remote_refuses_no_commit`.
  - **`ask` is an ask on Claude Code and on Pi's terminal UI only, and its answer cannot be read back.** Pi
    is asked through `estigia gate --runtime pi --can-ask` (exit `4`; the extension passes the flag only
    when `ctx.mode === "tui"` and shows `ctx.ui.confirm`), **unverified against a live Pi** (the `confirm`
    call is read from Pi 1.1.0's `docs/extensions.md` and `types.d.ts`); `rpc`, `json` and print modes are
    never asked. Every other agent is refused with the same code (an `ask` a host ignores is an allow), and
    the gate never learns whether the person approved, so it **asks on every such write**.
  - **A write the hook does not intercept is not held** (a tool no matcher wakes for, a script or formatter
    whose shell line names no git fragment, another clone), and **only a routine write and a `git push` are
    held**: a push is held as a commit is (`an_unclaimed_push_is_held_as_an_unclaimed_commit_is`), while an
    unclaimed run writing `.git/estigia/estigia.local.md` through a shell line is not stopped by this row and
    can set it to `off`.
  - **A write that lands outside every repository the gate can place is not a repository write.** The gate
    asks git once per such write (`rev-parse --show-toplevel`) and, where git names no working tree, walks
    the ancestors for a `.git` entry. **No entry anywhere** is outside every repository; **an entry git will
    not use, or an ancestor this process cannot read**, is no answer and the write is held. The walk is not
    git's, so a work tree named only through `GIT_DIR`, a ceiling directory, or a bare or mirror layout is
    read as a repository only where an ancestor has a `.git` entry; it costs one subprocess per such write,
    plus one `git remote` per landing repository for a GitHub tracker that names none.
  - **A write tool whose paths live in a patch body is placed by the body's own paths** (for `patch` and
    `apply_patch` only: `*** Add File:`/`Update File:`/`Delete File:`/`Move to:` lines; a write to **any**
    repository among them is held; no other write tool's body is read as paths, which a `pre_write_code` call
    with a `*** Add File:` line in its `content` regression-tests). A path that cannot be placed is read as
    inside.
  - **A shell write is placed inside only by its literal redirect targets or by naming a repository
    operation.** A redirect to a literal absolute path outside every repository is not held when the line is
    one plain printing command (`echo`, `printf`, `cat`, `ls`, `grep`, `rg`, `head`, `tail`, `wc`, `date`,
    `pwd`) with no quote, `$`, backtick, backslash, `<`, `|`, `;`, `&`, parenthesis, line break or `..`.
    **Everything else a shell line can write is unplaceable** (a `cd` before a redirect, a variable, `&&`, a
    second writer, inline interpreter code): refused under `required`; under `ask` put to the person when the
    call was launched in a repository or from an unestablished directory, and allowed only from outside
    every repository; a symlink outside the checkout pointing in is not resolved. **Exactly one shape is added**
    (`shell::quoted_heredoc_target`): `cat > TARGET <<'DELIM'`, `cat >> TARGET <<'DELIM'` or `cat <<'DELIM' >
    TARGET`, a literal absolute `TARGET` without `..`, any body (**not read**; how an issue body is drafted in
    the scratchpad) and a closing `DELIM` line. An "any shape" relaxation was abandoned after five review
    rounds each found a case the reader modelled differently from bash (operator decision 2026-10-02).
  - **A write inside an isolated checkout that a claim names is exempt, whichever session made it** (a
    delegated writer, such as an OpenCode child session deriving its own run, holds no claim of its own): the
    exemption applies when a pointer holding an issue has a `worktree` covering both the call's directory and
    the write's landing and differing from its `repo_dir`. It is path-based, so any session with that working
    directory is exempt, claimed or not, for as long as the pointer holds an issue (no expiry); an unreadable
    pointer grants nothing. Measured at the library level, not through `estigia gate` with a child
    `--session-id` or a live OpenCode host. The stand-down covers an `ask` as it covers a refusal.
- **`Claim workflow` `assisted` takes an issue through a local claim and publishes it as a draft pull
  request.** Every tool in `TRACKER_WRITERS` is refused `workflow-assisted-writes-no-tracker`, `estigia goal`
  answers no claimable work, and `assist` (offered only there) reads the issue and makes the isolated
  checkout without writing the tracker. The claim is a local run pointer that `harness::assisted` reads
  against the ownership marker inside the worktree and the branch it has checked out, so a pointer whose
  marker names another run, whose branch was switched or whose worktree is gone is refused at the next
  boundary or when the renewal window ends; the tracker's `verify-claim` is never asked. A write is admitted
  only inside its worktree on its branch. A write tool's absolute path is placed by that path, whatever
  directory the session runs in: inside the run's repository it is judged against the worktree, in another
  repository it is that checkout's, and anywhere else (a session scratchpad) it is outside the claim, and a
  refusal names that path. A shell write names no path, so it is placed by the session's directory, and
  `cd <worktree> && sed -i …` from the primary checkout is refused `assisted-outside-the-worktree` naming
  where the call runs (`harness::assisted`,
  `a_shell_write_from_the_base_checkout_is_still_placed_by_where_it_runs`). A write tool whose paths live in its body and a shell command
  whose destination cannot be placed (`patch -p1 < x.diff`) are refused `assisted-write-unplaced`; delivery
  and `gh pr create` from a shell are refused with `draft_pr` named. `estigia release --run-id <run>` puts an
  assisted run down whether its issue is a number or a key: it forgets the pointer, writes no tracker and keeps
  the worktree, because nothing local reads whether a person delivered the branch
  (`releasing_an_assisted_run_forgets_its_pointer_whatever_names_the_issue`); `already-holding` from an
  assisted holder and the refused `release` tool name that command. What it does not hold:
  - **`git push origin HEAD` is refused**: the hook reads `HEAD` as the local ref
    (`git_hands_the_hook_head_and_not_the_branch_for_a_push_of_head`). A push is admitted only when every
    line has the run's branch as both refs; `branch:main`, a tag and a deletion are refused
    `assisted-push-unconfined`; a shell push names no refs to the `PreToolUse` gate.
  - **The renewal window rides a local check, not a tracker answer.** Only `harness::assisted` stamps an
    assisted pointer, after the marker and `git symbolic-ref HEAD` agreed, so a marker or branch changed
    inside the window is seen at the next boundary
    (`no_effect_but_the_assisted_decision_leaves_a_stamp_on_an_assisted_pointer`).
  - **The marker and the branch are bytes the agent can write**, an unsworn session in the worktree is not
    stopped for a routine write (`issue_requirement` reads the worktree as claimed, whichever session writes
    there; a `git push` from it is held by `Unclaimed write policy` wherever it runs), and an unclaimed shell
    write to the repository's own `estigia.local.md` can set `assisted`.
  - **`assist` adopts an existing branch only through linkage the tracker records.** A branch origin already
    has is taken only when it heads exactly one open pull request that the host links to the issue: for a
    number, a closing reference on the pull request (`closingIssuesReferences`) or the branch in the issue's
    Development sidebar; for a Linear key, the pull request's URL among the issue's Linear attachments.
    Otherwise it is refused `assist-branch-not-adoptable`, and a failed read of the list, the sidebar or the
    attachments stops before any repository write. The free-text mention (`Refs #12`, a key in the title or
    branch name) was dropped as an admission path: anyone able to open a pull request could write it, and
    assisted pull requests never carry a closing keyword, which is why the sidebar link counts. What stays
    open: the author of the pull request or of the link is not checked, so whoever can link a branch to the
    issue can make it adoptable, and still only for a branch the operator names to `assist`; a branch
    published without being linked (a plain `git push` and `draft_pr`) cannot be taken again until it is.
    The sidebar link names a branch, not a pull request, and a closing reference is matched on the issue number alone, not on the
    repository it names. Measured by the `assist_*existing_branch*`, `assist_adopts_*`, `assist_refuses_*`
    and `only_a_closing_reference_to_the_number_closes_the_issue` tests in `src/transport/branch/tests.rs`, and
    by `a_key_is_adopted_only_when_linear_attaches_the_pull_request` in `src/transport/linear/tests.rs`.
  - **`draft_pr` publishes a draft and guarantees little else.** It pushes the pointer's branch
    fast-forward, opens a draft and reads it back as open, a draft and at the pushed head; it writes nothing
    to the tracker and adds no marker, receipt or `Refs` line. The team's own CI and reviews are theirs; a
    closing keyword is refused in the title, body and branch commit messages and not in a later host edit or
    commit; the branch is not checked to be the one named for the issue. An open ready pull request is
    handled as `Corrections after ready` says (`refuse`: refused; `redraft`: asked back to draft; `push-ready`:
    pushed to as it stands, so CI and reviewers see the new head at once), a draft that exists is reused with
    title and body unchanged (`"title_and_body_applied": false`), and a failed read-back answers in the
    `committed` world. `ready_pr` marks ready what was pushed, not what was reviewed (no receipt binds a
    review to bytes, so `Self review` is an instruction). Held against a fake `gh` by
    `src/transport/draft/tests.rs` and the `draft_pr` tests in `src/harness/mcp/tests.rs`.
  - **Coordination is left to the team**: nothing reads another person's claim, assignee or label, `assist`
    refuses a second run on one branch on this machine only, and Estigia cannot tell that the person, not the
    agent, named the issue.
  - **An unreadable contract is not treated alike by every tool.** `GateContext.workflow` is `None`; `assist`
    is refused and an assisted pointer is `assisted-claim-not-in-force`, but tracker writers are not refused
    (that would stop every tracker write in every repository whenever one unrelated row was malformed), so a
    broken contract in a repository whose row says `assisted` leaves tracker writes open until it reads again
    (`an_unreadable_contract_names_no_workflow_at_all`,
    `an_assisted_only_tool_is_refused_unless_the_row_says_assisted`).
  - A test classifies every tool in the server's table as tracker writer, assisted-only, local writer or
    read, so a new tool fails until placed; a writer a person runs by hand through `gh` is not a tool call.
- **The screen's door and event loop are unmeasured.** `the_screens_install_proceeds_on_a_source_build`
  drives `guided_install`, which always consents a source build (a person confirming the screen is that
  consent), and nothing asserts that `dispatch` does not put a lifecycle preflight back in front of `guided`;
  `every_production_caller_of_install_planned_is_preceded_by_the_lifecycle_preflight` is a text guard that a
  `#[cfg(test)]` call, macro, function pointer or `use` alias escapes. The screen writes three things the
  lifecycle gate covers none of (the guard key's `pre-push` hook, the language key's remembered tongue, the
  folder picker's directory: `docs/lifecycle.md`), and `run` needs a terminal, so that it passes
  `app.repository` to `install_guard` and `report_of_checks`, keeps a guard refusal apart from an install
  refusal and calls `Reads::refresh` before each key are readings. `wrapped_rows` in `src/tui.rs` emulates
  ratatui's word wrap (`Paragraph::line_count` sits behind `unstable-rendered-line-info`), checked only
  against Help in both tongues and one synthetic long row.
- **The legacy-checkout stop cannot name the legacy path of a template that named no placeholder.** A
  `Worktree location` missing `<branch>` or `<run-id>` gains it in memory, and `start_branch` stops with
  `legacy-worktree-registered` when the pre-migration checkout is still registered, computed from the raw
  template. For a bare directory naming **neither**, that raw path is one no build created (they appended
  `~<run-id>`), so a run resuming such a branch is stopped by `branch-checked-out-elsewhere` instead, before
  any remote state is written and with no data lost; the recovery is the same `git worktree remove`, written
  in the binding. Measured across twenty-four template shapes by the blind reviewer of receipt
  `6b192e0ee94f11004b621c06c8e8e5dd`.
- **Four of the twenty are about the past.** `LOOKS_BACK` excludes `silence`, `stale-run-pointer`,
  `prune` and `gate-version` from readiness: undecided past calls, another run's pointer, orphaned
  branches and a stale diagnostic binary do not prove a fresh claim impossible. The store row is not
  exempt: an unreadable authority prevents claiming now. This classification is a policy boundary,
  not proof that the historical problems were repaired.
- **The claim and review families decide from the local workflow store and write only to it; GitHub carries
  labels, the assignee and human prose.** First touch imports through `gh issue view`, refusing
  `import-diverged` for differing holder/latest publication. Open-held renewal costs two REST calls,
  held by `a_gated_write_on_an_open_held_issue_makes_no_graphql_call_and_two_rest_calls`; closed-issue
  delivery proof uses the stored receipt, held by
  `an_own_delivery_close_is_proved_from_the_store_when_github_carries_no_marker`. GitHub projection
  follows local commit; the stand-in rejects posted `issue-flow:` bodies. Closing-reference,
  projection-convergence, claim assignee/label and handoff label reads still use GraphQL. Limits:
  - **After the first touch only a person's stand-down or adjudication is admitted from GitHub**, from
    comments **whose markers are all `standdown` or `adjudication`, written by the authenticated user and
    never edited** (`after_the_first_touch_only_a_stop_is_admitted_from_github_comments`). An operation-id
    claim needs `adjudication`; comment stand-down cannot carry its scope. Later edits, labels and
    assignees are not reimported. GitHub `created_at` and local marker clocks can tie; insertion order
    may differ across stores. Sync fails at 10,000 comments. REST trust requires authenticated-login
    equality and flags any differing created/updated timestamps, stricter than GraphQL.
  - **A lost claim race rolls back; a lost reclaim race still commits** (`reclaim` adjudicates after its
    transaction). A `dev:` label that cannot be made after the store accepted the claim is `write-failed` and
    the retry runs the projection again. The state compared is the store's, set at import from the single
    `status:` label (two or none import as no state: `unexpected-state` until a `transition`). The
    differential check compares the holder and the latest publication, and nothing else.
  - **An older binary on the same issue cannot see these markers**: it reads GitHub only, may claim an issue
    this binary holds, and its merge gate may clear a head this binary has not reviewed. Nothing detects the
    mix; the cure is reinstalling every agent profile before an issue is in flight on both. A person
    reading the issue page sees no claim or receipt comment, and there is no single chained lifecycle test.
  - **`create`'s store admission is recoverable only by title.** If admission fails after filing, a repeat
    searches `gh issue list --search "\"<title>\" in:title"`, adopting a single match, refusing two and
    filing anew on none, so a search index that has not caught up (a lag nobody measured) would let the retry
    file a duplicate.
  - **The queues decide from the same store.** `list-state`, `list-identities` and `completed_count` read the
    open issues from one REST list cut to the oldest 200 **open** issues across every state (a state whose
    items are all newer than that cut is not seen), decide state, holder and dedup key from the store (an
    open issue the store never admitted is `unadmitted`, reported by `estigia goal` as `null` when the read
    failed, never `0`), and read the review queue's per-candidate timeline without the sync a ledger open
    pays. `estigia store import` admits untouched open issues oldest first.
  - **The dedup identity, the `blocked-until` clauses and the task checklist live in the store and nowhere
    else**, so a lost `state.db` loses all three. `blocked_until` requires no claim; a `task` is recorded, not
    verified (nothing checks its commit exists or its `evidence` note names anything real); `transition --to
    done` answers nothing to verify when the issue has no confirmed publication. FLOW's routing rules in
    `skill/SKILL.md` are instructions, enforced only by `task`'s `done` rule, and their effect on cost is not
    claimed (`docs/review-cost.md`).
- **The store is one copy, and its protection is local and partial.** Losing `~/.estigia/state.db`
  loses store-only identities, clauses, tasks, in-flight claims and review history. Reimport cannot
  recover what GitHub never held, even while its projection still names the run. On the write path,
  `VACUUM INTO` takes a snapshot when none exists or the newest is at least 24 hours old, keeping
  three in `~/.estigia/snapshots/`. Failures are counted, not fatal, and restoration is manual.
  Doctor reports age, not a verified restore. Append-only triggers do not resist an OS user
  able to drop them. `store-unwritable` rejects detected network mounts on macOS/Linux, not Windows.
  `foreign-projection` refuses unexplained GitHub owners across stores; reclaim overrides it.
  The best-effort `gh` counter counts spawns, not cost, misses other threads/undispatched operations,
  and silently drops counts on open/insert failure or a store busy beyond 200 ms. No test here proves
  restoration, Windows mount detection or counter completeness.
- **A run that swore nothing is never gated, including when Estigia is broken.** Failing closed on everybody
  would be a lock rather than authority and would teach people to remove the hook: a missing transport denies
  a sworn run's write and lets an unsworn one through. **An unreadable run record stops a write**: a pointer
  that exists and will not read says a run existed and not what it swore. `session::load` treats only a
  `NotFound` read as absence, and only while the pointers' directory is one (Windows answers a path whose
  parent is not a directory with `ERROR_PATH_NOT_FOUND`, so a state root that is a file would otherwise read
  as *every run swore nothing*); other failures load as unreadable and name the path and error. The push
  guard denies `run-pointers-unreadable` when nothing readable covers the checkout and some holding is
  unreadable, and a readable holder that covers it makes the unknown one change nothing.
- **A contract row the gate cannot read costs that row, and no longer the rows beside it.** The gate reads
  the installed contract with a fallback to defaults rather than refusing, because a file that will not parse
  is a problem `doctor` reports, not a reason to stop an agent mid-edit; the fallback takes the renewal
  window to zero and keeps every row that parses
  (`one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it`; salvaging can only narrow, since
  `Config::default` is the loosest configuration). A *boundary* row that will not parse still costs the
  boundaries it named (`an_unreadable_contract_costs_a_declared_boundary_its_boundary_status`), the one place
  where *configuration may only tighten* runs backwards.
- **Every script this crate writes reads the binary's answer instead of inheriting it, and a script that
  cannot get one lets the call through with a word.** Only `1` (a refusal) and `2` (an outcome it could not
  read back) are decisions; anything else (a missing binary at `127`, a panic at `101`) means it did not
  answer and the write or push goes through (*"a hook that breaks does not deny"*). The OpenCode plugin holds
  the three codes (`the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer`) and a result with no numeric
  exit code is let through with a word (`the_plugin_says_so_when_the_gate_returns_no_exit_code`). The cost:
  on a machine whose binary has gone, every gated write goes out unmeasured, saying so on standard error;
  failing closed would be a harness people uninstall the first time it is wrong at a bad moment.
- **Nothing here resists a local actor running as you.** The gate reads a run pointer, a contract and a hook
  command, all files in your home with your permissions; anything able to write them can rewrite what Estigia
  believes, and a replaced binary answers whatever it likes. No tamper-resistance is claimed: the threats are
  accidental (a truncated write, a crash mid-rename, two processes racing on one pointer). **A stale writer
  is dropped, not merged**: a pointer on disk written more times than the one in hand is news the caller has
  not seen and its store is skipped (the fail-closed direction), without reconciling two concurrent edits or
  reporting the dropped one.
- **The boundary list is compiled, and can only be added to.** Estigia knows git and GitHub, not that a
  repository ships `npm publish` or `terraform apply`, so `Irreversible commands` lets an operator name
  those as boundaries (no renewal window, a fresh read every time). It **only adds**: no value removes a
  built-in, because a setting that can make the gate looser is not a guard rail. Nobody can express a rule
  the classifier has no shape for; matching is substring, on a normalised command line whose
  provably-data spans are blanked only when the whole line is one read-only simple command.
- **Decisions are written down; the reasoning behind them is not.** Every allow and deny is appended to the
  local store's `decisions` table with the run, the tool, **what the decision was about**, the verdict and the
  refusal code. `Outside` is not recorded (a record per tool call of every session that never swore would bury
  the ones that matter) and the write is best-effort (a refusal that could not be written down is still a
  refusal). **A payload Estigia cannot parse does not stop the call, and says so**: the hook stands aside
  (a schema this build does not know could wrap `Read` as easily as `Write`) and writes `payload-unreadable`
  to the ledger. Records older than thirty days are deleted at most once every twenty-four hours, by
  `SessionStart` only; a record that could not be appended is counted (itself best-effort) and reported by
  `doctor`; the JSONL history this replaced was imported then deleted (a torn legacy line is skipped, so
  whether every record survived is not measured). `status` lists what this machine's runs recorded holding;
  whether those claims are live is the tracker's answer.

## The populations and the prose they cross

- **Every population declaration is found by walking `src/`, resolved to the item its `subject` names,
  and fingerprinted together with it.** A declaration is a `pub const … : Population` (the type in
  `src/harness/population.rs`, each const beside the code it describes), read by `tests/guards.rs`
  with `syn`. The sources are read off the directory rather than listed, and a `subject` naming no real
  item in the same file, or more than one, is refused loudly. Changing the rule or the code beneath it
  reopens the claim: `tests/guards.rs` fails and names the new fingerprint, and somebody has to read the
  declaration again before recording it. That proves the pair has been read since it last moved, not
  that any declaration is true, and not that every guard needing one has one.

  **A list is a population; the matcher is a decision.** The declaration on `CONTROL_SURFACE`
  fingerprints the list, so changes to `is_anchored_fragment` once moved which paths the gate watched
  with this registry green. `CONTROL_SURFACE_CORPUS` is the matcher's subject: a fixed corpus of paths
  with the answer both roads must give each one, asserted by
  `the_control_surface_matcher_answers_its_corpus`. The corpus is representative rather than exhaustive.

  **Where the walk reaches, and what catches the rest.** `collect_found` reads top-level items,
  associated consts inside `impl` blocks, and the bodies of inline `mod`s, but does **not** walk
  statements, so a `const … : Population` inside a function body is invisible to it.
  `no_population_declaration_hides_from_the_walk` counts the type ascription in each file's
  comment-stripped text and fails, naming the file, when the text writes more declarations than the walk
  found. **Two spellings still escape both ends**: the ascription both counters match is one constant,
  `harness::population::DECLARATION_ASCRIPTION`, a contiguous substring, so `: population::Population =`
  does not contain it, and a declaration behind a type alias (`type Pop = Population;`) is invisible to
  the counters *and* to `is_population_type`. Neither is a plausible accident and neither is closed.
- **The seam guards read two shapes, not every shape.** They resolve markdown links and `<skill>/`
  rooted command lines. A payload that names a file some third way would slip past both.
- **The directive's rules are crossed against the gate by name, not by reading.** The text every agent
  carries in context states three rules, and each is pinned to a named test that holds the gate to it:
  a rule deleted from the prose, or a test renamed out from under one, fails the build. What no test
  can check is whether the sentence still *means* what the test enforces; that reading is a person's
  job.
- **The transport and Estigia read the operator table the same way, and the crossing that held it is
  deleted.** Both read `SKILL.md` between the same markers (`estigia:config:*`, with the superseded
  `issue-flow:` pair still read), normalising a hand-written cell (backticked token, `**bold**`, an
  explanation after an em dash or in parentheses). Nothing now crosses the two readers over the same
  bytes; `a_row_the_binding_acts_on_is_a_row_this_reader_finds` holds one side.

## Ownership

- **The runtime is still folded into a forced takeover's evidence digest, and the retry that would read
  it cannot be reached.** `forced_reclaim_hash` in `src/transport/ownership.rs` retains runtime for wire
  compatibility: removing it would invalidate existing evidence and change historical ownership.
  The fold recomputes from the marker's own runtime, so it binds to itself. Caller-runtime comparison
  in `resumed_reclaim` requires `Takeover::Write`, but an accepted marker first answers `already-yours`.
  Measured: a `runtime=claude` forced marker under a Codex-derived id, retried with the same operation,
  horizon and target, answered `already-yours`, not `reclaim-metadata-mismatch`. Reaching the latter
  requires a discarded marker still targeting the current `from-op`; no writer produces it. No test
  demonstrates that hypothetical retry.
- **The gate's wait on the transport is bounded at fifteen minutes, and what comes back is `Unknown`.**
  `Command::output` returns when the child does, and the transport bounds only *some* of its own `gh`
  calls, so a call without a timeout held the gate open as long as the socket stayed open (a `cargo test`
  against a transport that sleeps was still running ten minutes later). The bound is far past anything a
  working transport reaches (its own longest documented wait is two minutes), and the answer is never
  "nothing happened", because the command may have posted a comment or opened a pull request before it
  stopped. For the same reason the **next interpreter is not tried**.
- **`setup --all` keeps each agent's own configuration, and one residual is read, not run.** The
  configuration is read from the skill root it is about to be written to. A root with no contract
  inherits **per row**: an `Everywhere`- or `Machine`-scoped setting every installed contract agrees on
  is folded onto the portable defaults, and a row they disagree on (or every row, when nothing else is
  installed) stays at the default, and the run says which rows those are. `Agent`-scoped rows are never
  inherited from a neighbour. The population is every *installed* adapter unioned with the run's own
  targets, and the value inherited is `skill::contract_config`, never the layered value, so a
  machine-local `estigia.local.md` override cannot be promoted into a **different** adapter's versioned
  and shared contract. **The residual:** `doctor`'s `canonical` row compares two **layered** views
  (`installed_config` against each adapter's `installed_config_for`) on every row whose `scope()` is not
  `Agent`, so on a machine whose `Everywhere`- or `Machine`-scoped answers live in a per-root
  `estigia.local.md` rather than in the contracts, a newly registered adapter's contract will not match
  and `canonical` reports it `BROKEN`. Not a regression (that machine was already `BROKEN` before any
  adapter was added, and `estigia config set` without `--agent` writes every installed **contract**
  directly, which cleared the reported case); closing it belongs to `canonical`'s comparison or to
  `config set`.

## Reachability, registration and the rounds around them

- **The integration exemption proves *a* branch is checked out where the merge runs, not that it is the
  issue's own branch.** `is_integration_in_the_runs_own_checkout` checks `run.worktree` alone, an
  unredirected environment and any `refs/heads/*` symbolic ref. `start_branch` returns a branch but the
  MCP pointer effect retains only worktree; the branch remains in `ownership_path`'s private git file.
  `checkout`/`switch` are not observed. Git prevents checking out a branch held by another worktree,
  not trunk after the base moves away, `develop`, `release/1.x` or another feature branch. No guard
  distinguishes these from the issue's branch. A direct transport call also bypasses pointer effects.

  `integration_environment_does_not_redirect_the_repository` refuses `GIT_DIR`, `GIT_WORK_TREE`,
  `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_INDEX_FILE`,
  `GIT_NAMESPACE`, `GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM`, `GIT_CONFIG`,
  `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_COUNT`,
  `GIT_CONFIG_PARAMETERS`, `GIT_SHALLOW_FILE`, `GIT_REPLACE_REF_BASE`, `BASH_ENV`, `ENV` and
  `BASH_FUNC_git%%` by exact case-insensitive name, plus `GIT_CONFIG_KEY_*`/`GIT_CONFIG_VALUE_*`: a list,
  not an exhaustive proof. `GIT_EDITOR`, `GIT_PAGER` and `GIT_ASKPASS` are permitted. The config family
  can redirect worktrees or hooks. `is_safe_local_fast_forward` shares the list. The eventual merge
  runs after this process exits; repository/ref/worktree/PATH races remain
  (`local_fast_forward_proof_fails_closed_on_repository_state`,
  `an_exact_local_fast_forward_is_allowed_without_widening_other_merges`).

  **An integrating merge spends no verdict, and it is read in the directory it names.** The shapes are
  exactly `git merge [--no-edit|--no-ff|--ff|--quiet|--continue|--abort] [<ref>]`, alone, behind one leading
  `cd <dir> &&`, or as `git -C <dir> merge ...` with `<dir>` a plain path (optionally in one pair of quotes,
  not starting with `-` or `~`, or a drive-lettered `X:/` path; a relative one is taken from the session
  directory). It is allowed when the directory is inside `run.worktree`, the environment does not redirect
  Git and `HEAD` there is on a branch. The same line in another checkout, `--ff-only`, a second command, a
  redirect or an expansion, `git tag` and `gh pr merge` stay deliveries. A line that changes directory any
  other way (`cd $VAR`, a second `cd`, a subshell, `pushd`, `--git-dir`) is refused
  `review-checkout-unreadable` (class `unavailable`) once a review is recorded, for every git command in
  `DELIVERS`. **The run's own merge commit (#58)**: the prescribed `git tag -a "<tag>" -F "<notes-file>"
  "<merge-sha>"` and the `git merge --ff-only origin/main` that brings the base checkout to the delivered
  merge act on a commit that is not the reviewed head, so that commit is admitted when it is a two-parent
  merge whose second parent is the receipt's head and whose first parent descends from its base
  (`ReviewTransaction::delivered_by_merge`, shared with `match_merge`); a tag line is read only when plain
  words (`tag_target`) and a tag that names a commit is judged on that commit. Not covered: `git -C <dir>
  merge --ff-only ...`, `cd <dir> && git merge --ff-only ...`, a merge whose base the checkout cannot
  resolve (refused), and a squash or rebase merge (no second parent).

  **The near-miss diagnostic stops at a JSON string.** `integration-shape-not-exact` names the accepted
  `git -C <worktree> merge --no-edit origin/<base>` in `in-progress`; it never admits. But
  `shell_command_argument` sets `literal` only for one normalised command key holding a JSON string.
  An argv-array `git merge origin/main` still refuses `out-of-phase`. This is fail-closed; adapters
  sending arrays have not been measured.

- **Nothing counts the rounds a change takes, and nothing bounds one that is not converging.** The
  two-round limit is gone: `skill/policies/blind-judges.md` carries no count and no path reaches `blocked`
  because one ran out. The judgement triggers (the put-down row of `skill/references/decision-gates.md`: a
  known mistake repeated, three hypotheses without narrowing, more editing than measuring) are prose a run
  applies to itself. A claim's horizon plus the renewal window bounds a run that has stopped reporting, not
  one reporting diligently on its ninth publication. What stops an unconverging change is a person noticing.

- **ZCode registers its instruction file and its MCP server; three things remain unverified.**
  ZCode CLI 0.16.5 logged `mcp.server.connected` for `mcp.servers` in `~/.zcode/cli/config.json`, with no
  fallback file present. The shared skill and `~/.zcode/AGENTS.md` follow its configuration reference.
  Remaining limits:
  - The stated fallback loads `~/.agents/mcp.json` only when the scope has no servers. Adding Estigia
    could hide every fallback server while setup/status report success; this was not run.
  - `is_now_empty`/`discard` can delete the whole config after `strip_zcode_mcp` leaves only empty/null
    values, e.g. `{"hooks": {}}`. A nonempty `hooks.enabled` preserves it; unchanged files are untouched.
  - `~/.zcode/skills` is documented but unverified from shipped source, so the neutral root is used
    and that tree remains ungated. Hooks require global `hooks.enabled`; denial and payload are
    unverified, so no tool gate is installed (`gate_gap`).
  `every_control_file_an_adapter_has_is_one_the_gate_measures` crosses `.zcode/agents` and the MCP
  path; `a_host_read_root_carries_the_same_authority_as_the_directive` crosses the neutral fallback.

- **DeepSeek Harness registers, with its skill root and instruction target verified.** The adapter installs
  the skill into `~/.dsh/skills` (the harness's `user-dsh` root, from its `skill-filesystem` provider) and
  fences the directive into `~/.dsh/AGENTS.md`, both from the harness's own shipping source. `$DSH_HOME` is
  read and contained as `XDG_CONFIG_HOME` is (an absolute value under the call's home is honoured, one
  outside is dropped for `~/.dsh` with a per-adapter note). **No MCP registration** (where an operator
  registers a server has not been crossed against a running client, so `paths_in` answers `None`), **no
  tool-call gate** (whether a plugin can **deny** a call like OpenCode's `tool.execute.before`, and under what
  payload, has not been crossed against a shipped harness, so it is contract-only and nothing checks the
  agent honoured the directive) and **no role/sub-agent definitions** (`agents_root` stays `None`, yet the
  tree is gated by a **prefix**: the extension-trimmed fragment `.dsh/agents` reaches `~/.dsh/agents/**`
  only because it is right-unanchored, and narrowing it to `.dsh/agents.md` would drop the tree to
  `Routine`; the `~/.dsh/agents/reviewer.md` row in
  `a_host_read_root_carries_the_same_authority_as_the_directive` pins that). The fragments `.dsh/agents` and
  `.dsh/skills/` are crossed by `every_control_file_an_adapter_has_is_one_the_gate_measures`.

- **A workspace install covers the runtimes whose project-level files could be read from the runtime's own
  reference, and refuses the rest.** `setup --scope=workspace` writes each adapted runtime's files into a
  checkout (destinations: [`what-it-writes.md`](what-it-writes.md)); ZCode and DeepSeek Harness have no
  verified project-level destination, so a workspace install refuses them by name and a workspace
  *uninstall* of them (or a retired adapter) is a no-op
  (`a_workspace_install_needs_a_workspace_and_a_verified_adapter`,
  `a_workspace_refusal_names_the_command_that_clears_it`,
  `a_workspace_install_writes_under_the_workspace_and_nowhere_else`,
  `a_workspace_uninstall_takes_the_skill_back_from_one_shared_root`). **Not covered:** that each runtime reads
  every path in the table was read from documentation, not crossed against a host; Codex's project
  `hooks.json` is reported by an open upstream issue as not loaded where `.git` is a file (a git worktree);
  Cursor's project `hooks.json` has a stricter schema than the user-level file (a required top-level
  `version`), read from a community issue, so whether the workspace hook loads is untested; both
  project-level gates load only in a project the runtime trusts, which nothing reports. In a workspace the
  runtimes that read `AGENTS.md` share one instruction file and one skill root
  (`the_runtimes_that_share_an_instruction_file_share_its_skill_root`), so the shared directive and skill go
  with the **last** adapter the record names, and the block names whichever adapter wrote it last, so the
  per-agent override file it points at may belong to a runtime no longer installed (nothing asserts that
  file). A record is found by the digest of the checkout's canonical path, so a moved or renamed checkout is
  one no record names, and a fresh clone carrying an install in its files carries no record. The record is
  read, changed and written without a lock (two runs against one checkout can lose one adapter's name) and
  is written before the adapter's files (a failed `setup` leaves the adapter named). `is_configured` answers
  "not installed" for every adapter when the record cannot be read, the one reader that turns this unknown
  into an absence: the TUI uninstall reads `read_configured`, which keeps `Unknown` apart
  (`uninstall-not-read-back`), but the CLI `uninstall` output, `sync` and the status listings still turn an
  unreadable file into "not configured", and in the workspace scope `read_configured` takes an unreadable
  record as absent. `workspace_installed_adapters` answers `None` for a record it cannot read, an install
  that would merge into one refuses, and a record whose checkout is `NotFound` reads as gone. `setup --all
  --scope=workspace` was observed once to stop with a refusal naming the two adapters it cannot place after
  the five writable ones were done; no test drives that CLI path.

- **A global install is found through a record of its roots, and only installs made after the record
  existed are in it.** `forget_state` keeps the store while any recorded root, under
  `<home>/.estigia/roots`, still holds an adapter, and when a record cannot be read. An install by a build
  that did not write the record, under a `CLAUDE_CONFIG_DIR` other than the uninstall's, is invisible until
  `setup` runs again under that root. The record holds the Claude, XDG and `APPDATA` roots, not `CODEX_HOME`
  or any other root `Environment::resolve` does not read. The command line names the record on its own
  `create`/`remove` lines after a real run; `--dry-run` names neither. Held by
  `uninstalling_one_claude_root_keeps_the_store_the_other_root_reads`.

## What the gate narrows, and what it records

- **`observed` is attached on more roads than it is rendered on, and the register naming what is still
  missing has twice claimed more than the code does.** A gate refusal carries a structured
  `Refusal.observed` account of what the gate parsed: the command fragment matched and the checkout the
  call was adjudicated against. `gate_observing`/`decide_observing` receive the door's own reading and
  `prefixed` attaches it to every `Deny` they produce; `gate`/`decide` receive nothing (`decide` calls
  `decide_observing` with `&Observed::default()`), and `prefixed` emits `observed: None` rather than an
  empty object for an empty reading. `guard::adjudicate_action`'s single-holder arm is on this second
  road. `cli::show_gate`'s session-less branch passes the `Decision` through `report_gate`, whose `Deny`
  arm attaches the door's reading to any refusal that carries none, closing the gap for that caller
  under `--json` for every refusal `adjudicate_action` can produce there
  (`complete_review_receipt_not_selected`, `run-pointers-unreadable`,
  `several-runs-hold-this-checkout`, all via `Refusal::not_started`, which never sets `observed`).
  **Attached is not rendered.** `report` writes `Display`, which never mentions `observed`, unless `json` is
  `true`, and `src/setup/plugin.rs` invokes `${ESTIGIA} gate ${tool} --input ${payload}` without `--json`, so
  the one shipped agent that drives the session-less branch never sees the account. What remains open:
  - **The push road carries nothing.** `cli::git_hook`'s `PrePush` arm calls the same
    `adjudicate_action` through `guard::decide` but answers its `Decision::Deny` straight to `report`
    and never reaches `report_gate`, so every refusal it can answer with carries nothing, and nothing
    can top it up. Closing it needs two more readings: `session::holdings` drops the path it read once
    the run is built, and `guard::holders_of` discards every coverage depth but the winner's.
  - **`show_gate`'s own early failures carry nothing**: `gate-input-not-json` fires before `parsed`
    exists, and `working-directory-unknown` and `home-not-resolvable` can propagate out of
    `context_the_payload_names` before the `CheckoutObserved` half is built; each reaches `report` as a
    bare `Refusal::not_started`.
  - **The MCP road builds no account and adjudicates a checkout anyway**:
    `run-id-names-another-checkout` in `src/harness/mcp/mod.rs` compares `context.repo_dir` against
    `run.covered()` and answers through `Refusal::not_started` directly.
  - **The receipt-comparison half**: nothing reports what a delivery refusal actually compared a stated
    PR or head against (`delivery_pr_mismatch`, `stale_verdict` carry their own prose).
  - **No hook dialect renders it, and a role-refused `PreToolUse` never builds it.** The `PreToolUse`
    arm in `src/harness/hook.rs` builds the full account only for a call that clears
    `authorize_review_blind_launch`, the reviewer's own `roles::gate`, `roles::definition_for`'s `Err`
    arm and the named-agent `roles::gate` check; for a call that does reach the build, `Dialect::deny`
    emits only `deny_reason` prose for all five `Dialect` variants (`claude-code`, `gemini-cli`,
    `cursor`, `exit-code`, `cline`). Left open deliberately: a hook's answer belongs to the *host's*
    protocol, and a sibling structured key inside somebody else's schema is a decision with its own
    tradeoffs.

- **The clamp `narrowed_by_the_call` applies trusts one thing beyond `launched`: a pointer a claim
  already wrote.** `Worktree location` on a real install is a **sibling** of the repository it was
  minted from, never a child, so a call naming that worktree could never resolve through a sibling-blind
  clamp, and a run that had claimed, published and released CI could not deliver (`estigia gate`
  adjudicated `gh pr merge` against the launch checkout on `main` and answered
  `verdict-bound-to-other-bytes`). The invariant this repository runs on: several tasks per agent and
  several agents are fine, but a worktree holds exactly one task, so the worktree is the unit of task
  identity and `repo_dir` is not (several runs sharing one launch checkout is the *designed* state).
  `held_by_a_pointer` honours a directory when it lies inside `launched` **or** is a checkout a pointer
  records, drawn from `worktree` alone, so neither road reads `repo_dir`. A directory no pointer reaches
  is not admitted, and `checkout_root_covering` never climbs above `launched`
  (`checkout_root_covering_never_climbs_above_the_launch_directory`).

  With a run id (`--run-id`, a session id with `--runtime`, or ambient `ESTIGIA_RUN_ID`),
  `held_by_a_pointer` is restricted to that run's own `worktree`, so a call naming run B's worktree gains
  nothing and is decided against `launched` under the run id it carried. Without one, `named` goes through
  `harness::guard::holders_of`, restricted to a single holder whose own `worktree` covers it (never one whose
  `repo_dir` merely is an ancestor). What a run-id-less call cannot do is tell one caller from another, so it
  cannot refuse a call that *reaches into* another task's worktree: that flow violation is not enforced on
  this road, every call with no identity takes it, and `stale_verdict` reads `context.repo_dir`, so a
  delivery boundary the call takes is adjudicated against that other task's verdict binding. An operator who
  exports `ESTIGIA_RUN_ID` for a calling road that does not set it closes this without a code change; the one
  shipped adapter that invokes `estigia gate`, the OpenCode plugin, forwards `--session-id <session>
  --runtime opencode` and `gate_identity` derives the run id through `session::run_id`. The forwarded identity
  cannot establish that the claim was sworn under it: a claim sworn under a hand-minted id
  (`opencode-f2ae8c`) is not found by the derivation, the gate loads a fresh run and answers `outside`
  (permitted, no ledger line), so every action from such a run, delivery boundaries included, is permitted
  rather than judged. The derivation's pinning is prose, not enforcement.

  **Three readers of the resolved `repo_dir`, not one.** `launched_context` sets the field from
  `gate_context(&looking_at)` after the narrowing, so a model-written `workdir` or write target that a
  pointer records reaches all three:

  | Reader | What it decides from `repo_dir` | Where |
  |---|---|---|
  | Installed configuration | `tracker`, `boundaries`, `evidence`, `integration`, `window` | `gate_context` in `src/cli/mod.rs` — `skill::installed_config_in_keeping_what_parses(&skill_root, &repo_dir)` |
  | Role definition | which agent allowlist applies to the tool | the gate path in `src/cli/mod.rs` — `harness::roles::definition_for(&context.repo_dir, …)` |
  | Verdict binding | whether a delivery is bound to the bytes under review | `stale_verdict` in `src/harness/mod.rs` |

  **Cross-repository resolution is admitted when a pointer records the worktree.** A worktree path under
  `Worktree location` can belong to another clone on the same machine; if a live pointer names it,
  `held_by_a_pointer` honours it as it honours a sibling worktree, and nothing asks whether `repo_dir`'s
  git identity matches `launched`'s. That is the invariant as written for task identity, and it means
  the other repository's `.estigia/` rows and role definitions decide the call. No test drives a second
  repository's pointer through the identity-less road. **A corrupt pointer can remove a holder from
  consideration on that road**: `holders_of` walks every pointer through `session::holdings`, whose
  `pointer_at` answers `None` for a pointer that will not parse (unlike `session::load`, which loads it
  as `unreadable: true`), so a directory two holders cover at equal depth can answer down to the one whose
  pointer still parses. No test drives it. This is the `estigia gate` CLI door only; the `PreToolUse`
  hook road builds its `GateContext` from the payload's own `cwd` and reaches neither
  `narrowed_by_the_call` nor `checkout_root_covering`.

## What a measurement reconstructs

- **The review-cost measurement does not close what it was launched to answer.** No counterfactual
  quantifies the rework, claim races or unreviewed delivery the panel avoids. The two 2026-08-30 trials
  in [`review-cost.md`](review-cost.md) have these limits:
  - The first changed its definition mid-run; eight judges lacked the severity vocabulary. It ran the
    external rubric inside this host's `analyst`, not that project's binary, with two judges per arm,
    no variance or retained artifacts, on an already-merged PR with no receipt. The 13.7% brief effect
    and +18.2% summed judge time cannot be separated from sampling.
  - The second ran the external binary but admitted none of three answers (two discarded after
    181/265 seconds). It compares inputs, not findings, mixing bytes and agent tokens at an assumed
    four bytes/token. Its observed `high`/four-lens tier falsely matched a Rust `#!` attribute;
    corrected `medium`/one-lens was read, not run. External revisions were `a537c63e` and `4db804e5`;
    no artifact remains, and its relative external citations cannot be checked from this repository.
  No judge-versus-judge or whole-delivery figure exists, despite the orchestrator spend channel.
  `collect_review_input --wait-seconds` has only a before measurement: one session, 16 calls, one
  `capture-running`. Zero subsequent polls is by construction, not an after measurement. No suite
  test supplies the missing live cost evidence.

- **A preserved usage artifact is not a measurement, and no total folds it in yet.** The runner writes
  the reviewer CLI's own counters to `usage.json` in a job's directory, beside `result.json` and never
  inside it, and `job::read_usage` reads them back; the admitted answer's bytes are identical whether or
  not the CLI reported usage. Only the Claude adapter's `--output-format json` result carries a `usage`
  object, so a Codex review and a relayed review preserve nothing. `scripts/measure-transcripts.py` reads
  Claude Code's own `.jsonl` transcripts and does not fold this in, and nothing in production calls
  `read_usage`, so what a native review cost stays unmeasured. The artifact outlives the result it
  belongs to (`discard_result` deletes `result.json` and leaves `usage.json`), so a reader checks
  `job_status` first. **A missing artifact is unknown, not zero**: `read_usage` answers `None` both when
  the CLI reported no usage and when the counters could not be stored (a failed write goes to
  `runner.err`), and the two are not distinguished; an artifact that is not JSON, names no provider or
  carries a non-whole counter is an `Err`, never a zero. No consumer exists yet to read `None` as
  unknown.

## Refusals, and the way out they name

- **A refusal that names no way out is measured at build time, and the 67 the guard reports are frozen
  as debt rather than fixed.** `build.rs`'s `assert_every_refusal_names_a_way_out` reads the same walk
  over `src/transport` that fills `TRANSPORT_VOCABULARY` and holds three rules that were held by review
  alone: *do not name a command unless running it clears the block* (`AGENTS.md`), *an unknown result is
  not clearance*, and the contract's *never reinterpret an unknown result as clearance*. The population
  is every distinct reason the walk extracts, because a reason is the operator-visible identity of a
  refusal: **224 at this head**. A reason is satisfied when its envelope carries an `action`
  (`reasons_naming_a_continuation`; **154** do), when `BY_DESIGN` declares a shape from
  `NO_CONTINUATION_SHAPES` (`operator-knowledge`, `world-action`, `human-authority`, which is
  `src/outcome.rs`'s own `NoCommandReason` vocabulary, crossed by
  `tests/guards.rs::the_refusal_guard_vocabulary_is_the_harness_own`; **3** are declared:
  `blank-argument`, `invalid-horizon`, `invalid-operation-id`, all `operator-knowledge`), or when
  `refusal-baseline.txt` freezes it (**67** do: the operation-conflict and metadata-mismatch families,
  every `*-readback-failed`, and the generic argument refusals `malformed-argument`,
  `missing-argument` and `unknown-operation` among them). A `BY_DESIGN` declaration for a reason whose
  envelope already carries an action fails the build. `refusal-baseline.txt` may only shrink: an entry
  that matches no unsatisfied refusal fails the build naming it, so *the refusal was fixed* and *the
  refusal moved into a shape the walk cannot see* are told apart, and the walk's floor is the count that
  file records (`population 224`), so a walk that finds fewer fails naming the count.

  **What the guard cannot do.** It reads the operator-facing text for the presence of an `action`; it
  cannot tell a correct continuation from one that parses, runs and does not help, and only a person
  hitting it can see that. It cannot tell a genuine `BY_DESIGN` from a fixable refusal someone declared
  to silence the guard, and it does not reach a refusal whose reason is built by a shape the extractors
  do not read (`assert_reasons_covered` closes that class, not this). It was measured by mutation: a
  refusal added with no action and no declaration fails naming it, a baseline line matching no refusal
  fails naming the line, and removing a refusal from the source fails the floor.

  Three earlier refusals broke the rule and were found by a person hitting them
  (`several-runs-hold-this-checkout` naming runs that no longer exist, a phantom pointer's refusal naming an
  `estigia release` that did not discharge it, `reviewer-definition-changed` naming a world-action for a body
  Estigia itself wrote).

- **The per-runtime capability table is canonical; it does not measure that any host obeys a capability
  it is declared to carry.** `src/setup/capabilities.rs` holds `canonical(InstructionFile) ->
  Capabilities`, a total `match` with no `_` arm, and every entry of `AGENTS` declares its own
  `Capabilities`. A declaration that disagrees with the table fails the build (a `const` assertion runs
  `capabilities_agree` over the registry: `error[E0080]`), the same predicate crosses the table against
  `gate_spec`'s events, matcher and dialect and `agent_definitions_root`, `AgentAdapter::checked`
  refuses a mismatched adapter at construction
  (`an_adapter_whose_declaration_disagrees_with_the_table_fails_at_construction`), and
  `capabilities_for_slug` is fail-closed with `Capabilities::NOTHING`
  (`a_runtime_absent_from_the_table_carries_nothing`). What it does **not** measure is whether a host
  refuses a tool call, reads a hook, or discovers a skill at the root the table names; every column is a
  claim derived from what each host ships, so a column can be wrong about a host with the build green.
  What it buys is that a disagreement **inside the crate** is a compile error or a construction refusal.
  One disagreement is recorded and not fixed: ZCode's row carries `gate: Gate::None` while it carries
  the contract and the MCP server, the gap the ZCode entry states. This narrows the reporting gap (one
  word covering four states in what a person reads) and does not close it.

- **The merge-commit topology rule admits a forward-integrated delivery, and no longer proves the first
  parent was exactly the reviewed base.** `skill/SKILL.md` step 6 and `skill/bindings/github.md`'s
  `verify_claim` exception require the delivered SHA's second parent to be exactly the reviewed head and
  the reviewed base to be an **ancestor** of its first parent; `delivery_close` computes it with `git
  merge-base --is-ancestor <receipt.base> <first parent>` in `context.repo_dir`, reusing
  `transport::target::is_ancestor` (exit 0 is an ancestor, exit 1 is not, any other exit is a `Read`
  failure, because an unknown answer is not clearance). A delivery GitHub merged after `main` moved is
  recognised even though its first parent is a descendant, and a first parent unrelated to the reviewed
  base is still refused: `a_forward_integrated_delivery_is_recognised_as_this_runs_own_close` and
  `a_delivery_whose_first_parent_is_unrelated_is_not_this_runs_own_close` in
  `transport::claim::close::tests` measure both against a real repository. Estigia never guaranteed the
  first parent *equals* the base; the repository's strict check was that guarantee.

- **A Linear source reads an issue, writes nothing but a mapped state, and its transport is the `linear`
  CLI.** `Tracker linear <team-key>` is admitted only under `Workflow: assisted`, answered in a repository's
  own file (`config set` and `config edit` refuse it without `--repo`, `config set --repo` where `Claim
  workflow` is not assisted, and `config set Workflow autonomous --repo` where `Tracker` is already Linear;
  `doctor` and run start refuse it). `assist` reads the issue's title and description with one constant
  GraphQL query (`a_read_spawns_linear_api_with_the_query_and_sends_no_mutation`); `STATES_QUERY` and
  `MOVE_MUTATION` are sent only through `move_issue`, which `follow_state` calls only for a state the
  repository's `State names` row maps and only for a Linear key
  (`follow_state_moves_only_a_mapped_state_of_a_linear_issue`). A move is read back, reported under
  `linear_state` and never retried, so a run that ignores a failed one leaves Linear behind the work. Every
  operation is `linear api --variables-json <variables> <operation>` through the crate's one process boundary
  with a 30-second deadline; the CLI holds the credential, so Estigia never reads or passes the key. What it
  does **not** do: it writes Linear only to move a mapped state, and does not prove a credential can be
  scoped read-only. It depends on the CLI's behaviour measured against version 1.10.0 (`api`,
  `--variables-json`, exit status 1 on a failed request, the text `No API key configured`), so a changed
  message would be reported `linear-unreachable`; the workspace is the CLI's choice, so a repository's
  `.linear.toml` is not guaranteed to be the one read; only the two reads ran against the live API, the move
  only against the stand-in; a key the credential cannot see and a key that names nothing both answer
  `linear-issue-unknown`; authentication and "not found" errors in a GraphQL `errors` array are **matched
  heuristically**; `assist` refuses a key whose team is not the configured `<team-key>`
  (`linear-team-mismatch`). Nothing crosses the Linear client against a second implementation: the stand-in
  `linear` is the `fake_process` example.

  **The unclaimed-write refusal and `Unclaimed write policy` hold under `Tracker linear` with
  `Workflow: assisted`, as they do under GitHub.** Both ask `Tracker::adjudicates_unclaimed_writes`
  (true for a tracker a run can hold a claim against, which an assisted run does against Linear
  locally); Linear with workflow unset or autonomous stands down
  (`a_read_only_tracker_without_an_assisted_workflow_refuses_nothing`; `doctor` flags that
  configuration). Under `assisted` both refusals and the SessionStart text name `assist`, not `claim`,
  `create` or `start_branch`, which are refused there
  (`an_assisted_run_is_told_to_assist_not_to_claim`,
  `an_assisted_session_start_names_assist_and_not_the_refused_tools`). The tests build the gate context
  with `Tracker::Linear` directly, so the repository-file layer that produces it is read by its own
  tests, not crossed here. `estigia guard` and the pre-push hook say a read-only tracker without
  `assisted` adjudicates nothing.

## Test counts and mutation claims in a receipt

- **A review transaction's evidence prose and a pull request body are not files in this repository, and
  nothing reads either: a test count or a mutation claim written into one is unchecked prose.** The
  operator decision of 2026-10-04 chose this record over rewriting a merged body. The measurement is one
  pull request's evidence section, which says *"Six new tests, each demonstrated red under mutation in
  both directions"* against a diff that added **nine** test functions to `src/cli/tests.rs`; its
  mutation paragraph is aggregate and records no per-test result. Measured here: widening
  `held_by_a_pointer` (`src/cli/mod.rs`) back to `Run::covered()` reddens
  `without_a_run_id_a_directory_only_a_holders_repo_dir_covers_stays_refused` and
  `a_call_carrying_an_identity_cannot_reach_a_sibling_run_s_worktree_nested_inside_the_base` and leaves
  the other seven green, and the third unlisted test declares its own non-mutating status in its doc
  comment, correctly. This does not put a count back into that pull request, and no reader of
  pull-request bodies exists; a receipt's test count and mutation claims stay prose held by review.
  Recording the set of test names a receipt claims and diffing it against the derived set is a separate
  enhancement.

## Citations, and the coordinates that rot

**`docs/` cites code by file and item, never by line.** A `path:line` pin silently pointed at unrelated
code after a base integration shifted the lines (two such pins were found wrong by two blind judges
while the suite was green on three platforms), because `every_link_in_this_repositorys_documentation_resolves`
crosses markdown link targets and knows nothing about a backticked citation.
`every_repository_citation_in_the_documentation_holds_up` (`tests/honesty.rs`) is the crossing: it
fails on a `path:line` citation it resolves to a repository source file, on a carried line (`:NNNN`),
on a `path:line` whose repository-prefixed path is not there, and on an item one of the two explicit
forms binds to a file that does not hold it. A bare path with no coordinate is not checked against the
tree, because in prose it may be illustrative, historical or external, and it is the coordinate that
rots; a handful of bare paths here name a file the tree does not hold (the comment-ceiling test, the
retired Python transport and its oracle, the retired SDD protocol), and the history is kept while the
coordinates are not.

**What the crossing cannot see.** It reads `README.md` and `docs/**/*.md` outside fenced blocks, so a
citation inside a code block is unchecked. It binds an item to a file only in the two forms it
recognises (`item` then `path` in parentheses, and `path`'s `item`), and an item the cited file merely
mentions satisfies the binding. It collects an item only from the first token after one of nine
definition keywords, so a short name, an associated item, or one not first on its line is not checked.
It reads a span as a `path:line` only when the path before the colon carries an extension or names a
repository prefix or a known root file, so an extensionless file outside that list is read as a bare
path and skipped; it resolves a suffix-form path only among `src/` and `tests/` `.rs` files; and it
cannot tell a path into another project from a repository one, so a citation into the comparison project
is skipped, and a missing path that does not start with a repository prefix is skipped with it. A
citation added in a form the test does not parse is prose held by review alone.

## Generated files and reproduction

- **A declared generated file is reviewed by reproduction at approval time, not by reading.** The
  payload shows it as a manifest line and `review_computed_verdict` regenerates it before recording
  `approved`; the digest still binds the real head blob ([`generated-files.md`](generated-files.md) has
  the format). What this measures is that the head blob equals what the base's declared command writes,
  when the verdict is recorded. What it does not measure:
  - **The head is trusted only through that comparison.** No judge reads a manifest path's bytes, so a
    command that regenerates something harmful passes; the declaration is trusted because the base it is
    read from was reviewed.
  - **Drift.** The gate runs when the verdict is recorded, not at merge. A registry that moved between
    the two makes an honest file mismatch (refused) or a stale one match (approved) at that moment only.
  - **The network and the host.** The command inherits the caller's environment and `PATH` and may reach
    the network, so the answer depends on the machine that recorded the verdict. `unavailable` refuses;
    it is never a match. An honest file can mismatch (a lockfile resolved against a registry that moved):
    `mismatch` means the bytes differ, not that the change is wrong.
  - **The command is repository code, run by this binary.** A declaration in the head is ignored, and
    nothing sandboxes it. A verdict read runs it only with `reproduce: true`; the approval gate runs it
    unconditionally, under the claim.
  - **A reviewer can still comment on a manifest path.** Finding locations are checked against the real
    diff, so a finding there is allowed; the prompt only asks reviewers not to.
  - **Attribute conversions are not applied to the comparison**, and the tests drive `sh` with `printf`,
    not `npm`: no real lockfile generator was run.

- **The tier and the correction budget do not see generated churn.** `review_tier` leaves a manifest
  path out of `changed_lines` and of the content scan, and the correction budget leaves it out of the
  original count and of every repair's charge, so a regenerated lockfile neither licenses lenses nor
  spends the 200-line allowance. Both read `generated::manifested_paths`, and the tier answer names the
  paths in `generated_paths`. What it does not measure:
  - **Trust moves to reproduction.** A hot-path name, a shebang or a dangerous sink inside a manifest
    path no longer raises the tier; only the approval gate's byte comparison stands between that content
    and a merge.
  - **A generated-only change is `medium`.** With nothing measured the tier keeps its empty-diff answer;
    it is not read as `low` because the reproduction that would justify that runs only at approval.
  - **A correction can change a generated file without limit.** A repair that edits a manifest path by
    hand is bounded only by the reproduction mismatch it would cause, not by the budget.
  - **Anchors still see the real diff**: a finding on a manifest path is accepted, because a reviewer
    may open the file through `inspect`.

- **A timed child is drained, given no stdin, contained and reaped.** `spawn_with_deadline` reads both
  pipes on threads while it polls, gives the child a null stdin, runs it under
  `reviewer::process_tree`, and on a deadline kills the tree and waits. What it does not measure: **a
  grandchild is cut off after two seconds** (once the child has exited, the pipes get `PIPE_GRACE` to
  reach end of file; output a lingering descendant writes later is not seen); **no pipe-buffer size was
  measured** (the 1 MiB test exceeds any common buffer); and **the zombie is not asserted** (no test
  inspects the process table).

- **A declared-checks receipt binds the tracked bytes and the commands, and nothing else.**
  `run_checks` keys its receipt on the clean-target digest `publish_review` binds and a SHA-256 of the
  declared commands ([`generated-files.md`](generated-files.md#declared-checks)). What it does not
  measure:
  - **Inputs outside the digest are not in the key.** A gitignored file, an installed toolchain, an
    environment variable or the network can change a check's result without changing either digest. A
    receipt that failed is never rerun for the same digest; a new publication is the way to ask again.
  - **The checks run on the published head, not on the merge result CI tests.**
  - **Host MCP timeouts are not measured.** The wait is capped at 50 seconds and each call returns inside
    it, on the reading that some hosts give up near 60 seconds; no host was run to confirm.
  - **A timed-out job records nothing**, so a check that always overruns 30 minutes never yields a
    receipt, and the runner's cause of death is not read back (`checks-runner-lost`).
  - **The detaching launch is not crossed for this job.** The `__check-job` pipe test, Unix only, runs the
    subcommand directly; the launch that detaches it is the one `__review-job` uses, crossed by
    `tests/pipe/review_job.rs` for that subcommand alone, and nothing exercises the Windows flags.

## Which hook commands are Estigia's

A settings-file hook is recognised by its shape, `<program> hook <event>` (`runs_estigia_hook` in
`src/setup/render.rs`), not by a substring, so `slack-webhook stop` is not taken for the gate. What this
measures is the one regression test that writes such a command and runs `setup` then `uninstall` over it.
What it does not measure:

- **The program is not checked.** Renamed binaries and shims are accepted on purpose, so another tool
  whose command line is literally `<program> hook stop` is still read as Estigia's.
- **Only the first word is the program.** A wrapper in front of it (`env X=1 estigia hook stop`,
  `sh -c "…"`) is not recognised, so setup leaves it and writes its own entry beside it.

## The Pi system section and the prompt cache

The `<estigia>` section Pi carries (`estigia session start`) states the queued GitHub writes without their
count, so a queue that only grows leaves the text byte-identical (one unit test serves the text with one
and with five queued writes and compares the bytes). What it does not measure:

- **A real state change still changes the section.** A claim, a release or a changed isolated checkout
  rewrites the text, and on a model without mid-conversation system messages that rewrites the leading
  prompt and costs the history cache once.
- **Read from source, not from a run.** That Pi 1.1.0 records a changed section as a later system message,
  and that pi-ai folds it into the leading prompt for most providers, comes from Pi's source; no live
  session was run against a provider's cache.
- **The uncommitted-work paragraph is the section's largest and most volatile part.** Measured on
  2026-10-22 with `estigia session start --session-id probe-a --runtime pi --wake poll` under a scratch
  `HOME`: 1,228 bytes, of which **945** are the paragraph naming five checkouts no run holds and the files
  each carries, 141 the "no contract is installed" sentence, 126 the run-id line, 10 the heading and 6 the
  blank lines. The figures are bytes, not cache hits, and no session was run with a live claim, so the
  claim line's own length is unmeasured. [`prompt-cache.md`](prompt-cache.md) carries the table.

## What `release_ci` says about CI

`release_ci` returns `lane`, the reading `publication_lane` gives for the released head, and `next`:
`merge` for `not-consulted`, `disabled` and `never-ran`, `ci_status` for every other reading (unit tests
script each lane reading and a failed read). What it does not measure:

- **`absent` goes to `ci_status`.** The read follows the ready mark within moments, so a head whose check
  runs have not registered yet is indistinguishable from a repository with Actions on and no workflow;
  merging on it could skip a CI that is about to start. The second case still waits on `gh pr checks`.
- **An unreadable lane goes to `ci_status`.** Not knowing is not a licence to merge; the cost is the old wait.
- **No end-to-end release against a lane that is off.** The payload is asserted through the pipe only for
  an unreadable lane; the `disabled` and `never-ran` branches are asserted on the function that builds the
  answer.

## What `deliver` does not see

`deliver` runs the steps from the computed verdict to the close and hands back at the first that needs a
decision (`src/harness/mcp/deliver/tests.rs`: a scripted set of tool answers and a scripted `gh`, over a
real `git` repository with a bare origin; the happy path, each hand-back, a resume that repeats no write,
and a merge whose answer was lost). What it does not measure:

- **The live tools and the gate are not driven end to end.** One pipe test drives the real MCP server up
  to the receipt hand-back; nothing runs the whole tail against the real store and gate.
- **The required set is the pull request's whole rollup.** The host's required names and the repository's
  own lanes are not resolved: a failing check nobody requires hands back `ci-red`, and a required check
  that never reported is not noticed while the rest are green.
- **An empty rollup waits ten minutes from the `ci-release` marker.** A repository whose pull-request run
  starts later reads `ci-absent`; one with Actions on and no workflow answers `pending` for the whole
  window. The window is a constant, not a measurement. On a `not-consulted` lane an empty rollup passes
  at once.
- **`wait_seconds` bounds what `deliver` waits for, not what it calls.** A hung `gh` or `git` call, or a
  slow `release_ci` clean-target read, is not cut off.
- **A host that ends an MCP call before `wait_seconds` is not measured.** The ceiling is 900 s, the one
  `collect_review_input` has. Claude Code held a `collect_review_input` call for 111 s
  ([`waits.md`](waits.md)), and no other host's timeout was measured. Nothing tells a caller what its own
  host allows, and the default stays 20 s.
- **Past the first 50 s the pull request is read every 20 s, not every 2 s.** A check that finishes in
  that window is seen up to 20 s late. The read count per call is bounded by the cadence, not measured
  against GitHub's rate limit.
- **A merge queue or an approval rule hands back.** `merge-unconfirmed` and `merge-refused` are not told
  apart from other causes, and the queue's configured method is not proven to keep a merge commit; a
  merge whose read-back never answers reads `merge-unknown` (it may have landed); an unreadable base or
  manifest reads `base-unreadable` or `version-unreadable`, never as advanced or unchanged.
- **Only two manifests are read for a version**: `[package] version` in a changed `Cargo.toml` and
  `"version"` in a changed `package.json`. A `pyproject.toml`, a workspace-inherited version whose root
  file did not change, and any other manifest move without a hand-back.
- **The lens roles in the evidence follow the tier and focus recorded on the receipt**, not each lens's
  report. **The summary line has two languages** (`English`, `Español`); any other `Summary language` is
  written in English. **The GitHub issue is closed after the release**: a failure of that last close
  leaves the issue `done` and released but open, reported as `delivery_incomplete`. A slice whose pull
  request does not close the issue is not released: `deliver` posts its evidence, moves the issue back to
  `in-progress` and answers `continues: true`
  (`a_slice_whose_pull_request_does_not_close_the_issue_continues_it`). It records no spend, since
  `release` is what records it, and it reads the closing references from GitHub, so a branch that
  names the issue with a closing keyword still closes it. A `deliver` started
  in the checkout its release removes runs every later read from the main working tree
  (`a_delivery_run_from_the_checkout_its_release_removes_still_reads_the_close`); when git cannot name
  that tree, the read still fails closed.

## What `estigia changes` does not do

`estigia changes` is the one read of uncommitted work per checkout, and `estigia prune` lists every
checkout it keeps (unit tests build real repositories with a dirty orphan checkout, a held one, a
directory git cannot read and a repository with no commits, and one pipe test drives the binary). What it
does not measure:

- **Two surfaces render the listing, and neither has run live.** The Pi `/estigia-changes` command and
  widget (`ctx.ui.setWidget`) are driven by node against a stand-in binary whose `setWidget` is a
  recorder; the Claude Code `/estigia-changes` slash command is tested as a file `setup` writes and
  removes, not as a command Claude Code ran. Not built: the OpenCode TUI plugin, the Claude Code
  status-line segment, and a Codex notice of its own. OpenCode, Claude Code and Codex see the
  session-start notice and nothing else, and none of it has run in a live session.
- **The session-start notice is the binary's text, not a per-runtime surface.** It counts uncommitted
  files with one `git status` per linked checkout under a three-second budget and names what it did not
  reach; the base checkout is never named, and a checkout a pointer names but git does not register is
  not read.
- **The `done`/`release` notice is tested on its function, not through a tracker**, and reads the
  pointer's checkout, so a run whose pointer recorded none gets no notice. It is a notice only
  (`lifecycle.md` documents the decision not to refuse).
- **No session-end notice.** `SessionEnd` answers `{}` and no runtime is known to show that output.
- **`orphan` is a readable run pointer, not the claim**; the issue's state is not read.
- **Line counts are Git's, and partial.** An untracked file is counted only when it is a text file of at
  most one mebibyte; a binary file, or one counted against a checkout with no commits, has none. An
  unmerged path is marked `M`. **Fifteen seconds is the bound on each Git call**, chosen rather than
  measured.

## The feed

`estigia feed` ([`feed.md`](feed.md)) projects the local store and records nothing (eleven unit tests seed a store with
claims, transitions, a marker entry in a second repository and a message, and drive each filter, the paging, the
refusals and the absent store). The `feed` MCP tool calls the same function; two tests hold it to the command's rows
for one seeded transition and to the unknown-repository refusal, not to every filter. What it does not measure:

- **It sees only what the store recorded.** A merge is read live from the tracker and is not an event; a consent
  prompt, a turn that ended and a gate decision older than the 30-day retention are not in the store. A state change
  made before the `transitions` table existed has no event. Every answer says so in `cannot_see`.
- **Quiet is not proof of a quiet run.** Runtimes with no gate (`zcode`, `deepseek-harness`, `agents`) or a
  pre-tool-only gate (`cursor`) produce no lifecycle events; the feed lists none for them and cannot tell that from
  a run that did nothing. A Cursor run registered before `AGENT_RUNTIMES` listed `cursor` is filtered as
  `claude`; one registered since is filtered as `cursor`.
- **The window is bounded, the scan is not indexed.** `entries` and `transitions` carry no index on time, so each
  call evaluates the time of every row before applying `--since`; not measured against a large store. An `entries`
  row whose `created_at` SQLite cannot read as a time (the compact `20260726T005435Z` form) is not listed.
- **The snapshot is a filtered re-read.** A row written in the same second as `taken_at` can join a later page,
  as with `estigia sessions`; the cursor is a digest, not a MAC. A `runtime` is the one the run id names, so a
  message is filed under its sender's runtime.
- **There is no link and no session start or end event.** Sessions are in `estigia sessions`; the feed lists
  transitions, markers and messages. A message's recipient may be a run that never registered.
- **Previews are tested for the three bodies seeded, not for every marker field.** A preview is the kind and the
  issue; nothing of a marker's other fields is shown without `--bodies`.

## Where

`estigia where` ([`where.md`](where.md)) is tested against a two-repository store and a stand-in tracker (eleven unit
tests for the answer, six for the resolver, three for `tasks` and the identity parser); the live `gh` reader is not
exercised by any of them. What it does not measure:

- **The holder is the registry's, not the timeline's.** A run on a runtime with no gate leaves no pointer, so an
  issue it holds shows no holder; the store's claim markers are not consulted for it.
- **Linear is almost wholly unavailable.** `read_issue` returns title, description and state type, so a key
  answers `unavailable` in every field and reaches no tracker.
- **The repository of a pointer is read from its checkout's `origin`.** A checkout with no GitHub `origin`
  contributes no candidate; a bare number held only there falls back to the caller's checkout.
- **The tracker reads are two `gh` calls with no retry.** A rate limit or a missing login leaves `issue` and
  `pull_request` `unavailable` with the reason, and `bound_to_live_head` `null`.
- **Message previews carry bodies.** Unlike `feed`, which omits them by default, the last messages are shown with
  200 characters, because the question is what the sessions said.
- **A bare-number reference to `tasks` that no repository holds** answers `unavailable` only when the store or
  the repository is missing; a repository the store knows with no tasks recorded is a real, empty checklist.

## The session registry

`estigia sessions` lists the rows `estigia session start` and `estigia session end` write to the `sessions`
table, joined with the run pointers (five pipe tests drive the binary through a started, a crashed, an
unregistered and an ended session, a bounded walk with neither omission nor duplicate, each refusal a page
answers and an empty registry offering no cursor; four store unit tests). What it does not measure:

- **`last_seen` is refreshed by a session start and by a verified claim, not by every gated write**, so a
  session holding no claim past the six-hour bound reads `stale` while alive (the bound errs toward showing a
  live session as stale). **Only sessions whose host calls `estigia session start` or fires a registered
  `SessionStart` hook are registered**: Pi, Claude Code and Codex do, OpenCode's gate plugin registers a
  session the first time a tool call completes, and the rest appear as `unregistered` only while their
  pointer names an issue. There is no pid and no host query.
- **A pointer is read through `holdings`, which finds a file only under the name its run id gives itself**: a
  run id with an uppercase letter is filed under a digested name, so its pointer and issue are not shown
  (minted ids from the hosts measured are lowercase; not fixed).
- **A bounded snapshot is a filtered re-read, not one consistent read of the machine.** Each page keeps rows
  whose `started_at <= taken_at`; the ordering key is immutable, so a traversal has neither omissions nor
  duplicates under the tested mutations, but `status` and `last_seen` are re-derived per page. Membership is
  frozen only to the second (a session registering in the same second as `taken_at` can join a later page:
  `a_session_registered_in_the_same_second_as_taken_at_can_join_the_walk`). The cursor's digest detects
  corruption and an edit that does not recompute it; it is **not a MAC**, and possessing a cursor is not
  authorization. Where `store_identity` cannot be read (the Windows arm when its handle read fails, any other
  platform) every such store shares `unknown`, so the cursor's store check cannot tell two apart.
- **A `status`-filtered traversal is not a complete set** (the value is read afresh per page, so a row that
  ends between pages can leave the walk; `a_status_filtered_traversal_reads_the_status_afresh_and_can_omit_a_changed_row`
  asserts the omission), and **a bounded page covers the registry rows only** (a pointer-only run has no
  immutable start; `status=unregistered` is refused `sessions-unregistered-not-bounded`). A bounded answer
  has no fixed byte ceiling: it is bounded by `limit` (default 50, at most 500) and the row shape; one real
  call on 2026-10-18 returned 433 sessions as 141,698 UTF-8 bytes (a response size, not a token estimate).
- **The derived "task k of n" is matched by issue number alone.** A session row names no repository
  identity, so `fields: status` reads the checklists of the whole store and drops an issue number that two
  repositories both have a checklist for rather than guess which one the run holds; the row then shows nothing.
  A `status_text` is what the run wrote, redacted and capped but not checked, and is never evidence.
- **The pane is one environment variable, read once.** `HERDR_PANE_ID` was measured on 2026-10-22 inside herdr
  0.9.3 and 0.9.5 (the OpenCode processes in `w0:p1` and `wE:pA` carried it, matching `herdr pane list`). Not
  measured: a host exporting the same name for something else, a Windows pane, or a pane id that outlives its
  pane. The row records what the registering process inherited, a locator rather than a way in. **No service
  URL is recorded for any runtime**: the OpenCode service seen on 2026-10-21 (`127.0.0.1:49374`) answered
  `GET /session` with HTML and `GET /api/session` with 401, so a recorded URL would not have been a way in.

### `estigia resume`

Unit tests cover the command per runtime, each refusal, liveness from a row with no process, from another machine, with a dead pid and with a live one, the `--exec` refusals, the command and directory `--exec` prepares and the exit code it returns through an injected launcher, a stale OpenCode run with its worktree, an ended run whose
claim is live and one whose claim lapsed, and a worktree with a dirty file and an unpushed commit. What it does not
measure:

- **No resume command was run against a live session.** On 2026-10-27 only the installed help was read:
  `claude --resume [value]` (Claude Code 2.1.311), `codex resume [SESSION_ID]` (codex-cli 0.159.2) and
  `opencode --session, -s` (OpenCode v2.1.4) exist. That the same session id and context come back was not
  measured for any of them. Pi and Cursor have no command.
- **The shape check is a heuristic.** A UUID for `claude` and `codex` and a `ses_` prefix for `opencode` are what
  the sessions seen so far looked like; a Cursor session whose id is a UUID is registered as `claude` and gets a
  `claude --resume` command that will not find it.
- **The claim is read from the store's entries by a substring of the run id**, which imported entries need
  because their `run_id` column is empty. A prefix collision would only add a row the reducer then drops.
- **The push state is the commit count against the branch's upstream**, not `branch_identity_verdict`, and it is
  read from the last fetch; it never asks the remote.
- **A pid can be reused.** The probe asks whether a process with the recorded pid exists on this machine; a
  session that ended and whose pid an unrelated process took reads as `live`, and `--exec` refuses it. The safe
  direction, and not fixed: the process start time is not recorded.
- **The recorded process is the nearest ancestor that is not a shell, found by `ps`.** From the parent of the
  registering hook or command, `ps -o ppid=,comm= -p <pid>` is walked up at most eight levels, skipping `sh`,
  `bash`, `zsh`, `dash`, `fish`, `ksh` (with or without a leading `-`) and `estigia`; the first other ancestor is
  recorded. A `ps` that fails or cannot be parsed, a walk that reaches pid 1, and eight levels of wrappers all
  record no token, so liveness is `unknown` and `--exec` is refused. The limits: an agent launched through a
  wrapper that is not a shell (`env`, `node` running a script, `npx`, a supervisor) is recorded as that wrapper, and
  the row then reads `gone` or `live` for the wrapper's life rather than the agent's; a wrapper that outlives the
  agent reads `live`, which refuses safely. **None of this was measured against real hosts**: the walk is tested
  on fixture chains, and which ancestors Claude Code, Codex, OpenCode and Pi put between themselves and their
  hooks was not read.
- **Windows records no process**, so every row there is `unknown` and `--exec` is always refused. A row
  registered before the process columns existed is the same.
- **`--exec` was never run against a real agent.** Its launcher is injected in the tests, and the native
  commands remain unverified against the installed CLIs, as the first bullet says.
- **The stored binary is the one seen at registration.** The registry records the program of the agent process
  (`/proc/<pid>/exe` on Linux, `ps -o comm=` elsewhere, so a full path on macOS and possibly a bare name on other
  Unixes) and `resume` uses it only when it is an absolute path whose file name is exactly the runtime's command.
  A Node-hosted agent that `ps` names `node`, a binary since moved or upgraded, and a row registered before
  migration 20 all fall back to the bare name. Which program each host's `ps` entry names was not read on real
  hosts.
- **Pi's session-file path comes from `ctx.sessionManager.getSessionFile()`**, which the installed Pi's type
  declarations carry; the Node driver supplies a fake of it, and no live Pi session was resumed with the path.
  Only a root session files its path: a child process registers under its root's id, and its own file would be
  wrong for that row. A Pi row registered before migration 20, or by an extension older than this one, has no file
  and `resume` refuses it.
- **OpenCode still never sends `session end`, and no event was found that could.** The strings of OpenCode 2.1.4
  list `session.created`, `session.execution.*`, `session.idle`, `session.archived`, `session.deleted` and the
  like, but no event of a session being closed; `deleted` and `archived` are actions on a stored session, not the end
  of an open one. The plugin's dispose hook ends the instance, and nothing shows it fires when the process exits. No
  hook was shipped on a guess, so OpenCode rows only ever go `stale`; a live OpenCode session ending was not
  observed.
- **`resume` is CLI only, and `SessionStart`'s "holds no issue" text does not yet point at `claim`.**

## The session mailbox

`estigia sessions send` writes a `messages` row, `estigia session deliver` takes and stamps the pending rows,
and `before_agent_start` (Pi) and `PreToolUse` (Claude Code, Codex) call it (store unit tests, pipe tests through
send, list, deliver, refusal and an ended recipient, one pipe test through the Claude Code and Codex hook, one
unit test that a refusal keeps the mail waiting, Node tests on the generated Pi extension). What it does not
measure:

- **No runtime was run live for delivery.** Whether Claude Code puts `hookSpecificOutput.additionalContext` of
  a `PreToolUse` answer in front of the model, and whether the Codex build in use reads that field from
  `PreToolUse` at all, are taken from documentation; a Codex session that ignores the field receives nothing
  and the row is nonetheless stamped delivered, because the binary cannot tell. The Codex terminal UI and
  daemon were not driven. Pi's section lasts one turn (whether this Pi keeps it out of the stored
  conversation was not measured). Beyond `PreToolUse` only `Stop` delivers (Claude Code's waiter, Codex's loop
  `Stop`), so a session that makes no tool call and ends no turn is not reached; `UserPromptSubmit` and
  `PostToolUse` are not registered.
- **OpenCode was driven under `node` against stubs** (OpenCode 1's `tool.execute.after` appending to a string
  result, OpenCode 2's `execute.after` steering through a stub `ctx.session.prompt`, and the
  `ctx.session.synthetic` fallback). Unverified: that OpenCode 1 puts `output.output` in front of the model,
  that OpenCode 2's `ctx.session.prompt` accepts `delivery: "steer"` mid-turn and that every OpenCode 2 build
  has `execute.after`. A result whose `output` is not a string is left alone with its mail pending. **A known
  loss window**: when `ctx.session.prompt` or `synthetic` exists and throws after the rows were taken, the
  rows are stamped delivered that the host refused and the sender sees them delivered; no small fix exists,
  since take and stamp are one statement (a delivery is always stamped before the host acknowledges it, and a
  killed hook process loses the message as delivered; delivery never records "read", which is the recipient's
  own act, below). OpenCode costs one extra
  process per tool call.
- **Waking an idle session was measured live once, by hand, per host, and not under load.** Claude Code 2.1.300
  woke an idle interactive session within 15 seconds when an `asyncRewake` `Stop` hook exited 2 (a new waiter
  was running afterwards, none after the session was killed;
  [`sessions.md`](sessions.md#waking-an-idle-claude-code-session)); the OpenCode 2 wake
  ([`sessions.md`](sessions.md#waking-an-idle-opencode-session)) was driven against a stand-in host and run once
  on OpenCode 2.0.28 with `--standalone`. Not measured live: a Claude Code session idle longer than 60 s,
  mail arriving mid-turn, two waiters starting at the same instant (the unit tests drive threads; each new
  waiter overwrites a token in `store_meta` and a superseded one exits at its next poll), OpenCode refusing
  a prompt, and a session woken while the background service rather than a private server runs the plugin.
  OpenCode 1 is not woken, a session that never finished a turn is not polled, an already idle Codex session
  is not woken (Codex's `Stop` is synchronous and has an entry only with the `Loop plugin` row `on`), and Codex
  `turn/steer` is not built. Mail a waiter took is stamped delivered before the host acknowledges the exit.
- **The MCP `send_message` door was driven live on one host only** (Claude Code 2.1.300: `claude -p` refused
  with no row written, the interactive dialog's *Allow once* recorded a row that was then delivered; *Allow for
  this session* and *Deny*, Codex and OpenCode were not run live). A client that declares the capability and
  never renders the question waits out the 15 minutes and the send is refused. The grant is per server
  process and keyed by the sender's run id, which the caller names, so an agent naming another run's id
  borrows its grant only if that id was granted in the same process.
- **`read` and `acked` are what the recipient said, not what a host observed.** Nothing watches a model read
  its context: `inbox --read` and `--ack` are explicit acts, so a recipient that never calls them leaves
  every row at `delivered` however carefully it was handled, and one that acks without acting makes the sender's
  view say so. `inbox` itself is covered by store, unit, MCP and pipe tests against stand-in sessions and was
  driven on no live host. A thread is the id of its first message and is not scoped to the parties: any run
  that knows a message id can join its thread. `expires_in` is recorded and shown and decides nothing, so an
  expired message is still delivered. Types, threads and acks are text and rows in a local file; no gate,
  permission or claim reads them.
- **`ask` and `wait` wake only what can be woken, and were driven on no live host.** A question reaches a
  session at its next hook point or idle wake. Claude Code (`Stop` waiter), Pi and OpenCode 2 (polling)
  record a wake; Codex and OpenCode 1 record none, so an `ask` to them returns `not_woken` after delivering
  the question and does not wait, and a model on Codex can answer only on a turn it is already running. The
  heartbeat is an MCP `notifications/progress` sent only to a call that carried a `progressToken`; a host
  that sends none and kills silent calls will still kill a long `wait`, and no host was run to measure
  which does. The ask and its answer are separate writes: an answer admitted a moment before the asker's
  timeout is recorded can sit in the mailbox as mail for a question already reported `timed_out`, never as
  the answer to it. A session that dies without ending its row is not seen as `ended` and the ask times out.
  The Pi parent-child channel was not moved onto the mailbox: it shares the rules (one outcome, refused
  late answers) and nothing else.
- **Broadcast and the completion contract were driven on no live host.** A broadcast reaches the members the
  resolver lists at the moment of the call; one that registers a second later is not in it, and a member that
  ends between the question and the write still gets a row it will never take. `idle` means a live session
  whose pointer names no issue, which is not the same as a session between turns: Estigia does not observe
  that. The one consent covers the group as named in the question, so a person approving ten sessions has
  approved ten sessions, not a rule. Accepting a handoff is read from a successful `claim` by a run of the
  recipient runtime in the directory the handoff recorded, so a recipient that starts from a worktree, or that
  works the issue without claiming it, never accepts and owes nothing; a store that cannot be written at
  that moment leaves the handoff unaccepted without saying so. The duty is closed by the store, and the mail
  that tells the sender is a second write: a mailbox that refuses it leaves the outcome standing and
  reports `mail_failure` to the recipient, and the sender still sees the outcome through `inbox` with
  `sent`. An abandoned duty is reported when the registry records the session's end; a session that dies
  without ending its row is never reported, and one that resumes after its end finds the duty abandoned and a
  late `done` refused. The deadline is a time the sender reads as `overdue`, not a timer: nothing wakes anyone
  when it passes, and it can be set only through the MCP `handoff` tool, not `estigia handoff`. The
  hook that accepts on `claim` has a unit test of the acceptance and none that drives a real claim.
- **A message is text from another session.** Delivery says so in its first lines; nothing stops a model from
  acting on it. **The `flow dev` conflict rule reads the words of the message**: `send_message` and
  `estigia sessions send` refuse text carrying `flow dev <n>` while the recipient's pointer names another
  issue, so a sentence quoting it as something *not* to run is refused too and a differently spelled
  instruction (`/flow`, tabs, a Linear key) is not detected. A handoff is never consumed, expired or
  read-stamped: every later session of that runtime in that repository is told again, and whether anybody
  took it is visible only from the tracker, through `claim`.
- **The handoff's label read-back is against a stand-in `gh`** (the fake models the label edit in a file; no
  handoff was run against GitHub), and the row is written through `store::beside(state_root)` while the label
  goes through `store::store_path()`: the same file in production, two scratch stores under `cargo test`.
  The counts in `estigia sessions` belong to listed sessions (a message whose sender has neither a registry
  row nor a run pointer is counted and shown against nobody).

## The needs-person record

`estigia needs-person` lists rows written at three store writes (`escalate_review`, a `blocked` transition
without a clause, a gate `ask`), measured by one unit test each and a store test for the dedup. What it does
not establish:

- **A notification is best effort and no guarantee.** With `Notifications` at `os` the binary hands the text to
  `osascript` (macOS), `notify-send` (Linux, when on `PATH`) or a PowerShell toast (Windows) and records
  `handed to the OS` on exit 0. That is not "seen": a focus mode, a missing notification daemon or a
  session with no display all exit 0 and show nothing. The argv of all three is measured against a
  stand-in, not a desktop: no real notification was shown while writing it. The default is `off`, and
  the record is written either way. The quiet hours read this machine's clock and, when it cannot say, hold the
  notification back; the hourly cap counts `handed to the OS` rows in the store, so a notification that
  failed frees its slot.
- **Three moments of the six named are not written yet.** `handoff_review`, a verified `done` and the delivery
  halt write nothing: the first has no behavioural fixture to measure it against, and the halt is a pure read
  that must not write.
- **`blocked` is judged at the transition.** A clause recorded afterwards does not retract the row, and an
  unreadable clause read records nothing.
- **A failed write is reported where the answer has room for it.** Recording never fails the decision it
  describes. `escalate_review` and a transition say `needs_person_unrecorded`; a gate `ask` leaves a decision
  with the verdict `unrecorded`, which is itself lost if the same store cannot be written. A host that hides
  the gate's decisions hides this too.
- **An `ask` from a sub-agent is recorded and notified.** The record path does not see `agent_type`, and in
  Claude Code a sub-agent's `ask` does reach the person, so skipping it would hide a real wait.
- **An `ask` repeats after thirty minutes.** The key carries a thirty-minute bucket: a wait that outlives it is
  recorded again, and one that straddles a bucket edge can be recorded twice in a minute.
- **`ask` is only as real as the gate's `ask`.** A `Delivery authorisation` of `ask` may not reach the gate at
  all, and hosts other than Claude Code convert `ask` into a deny before it is recorded.
- **No runtime was run live.** The tests drive the functions, not a session of any host.

## The npm installer and the registries

`npm/bin/estigia.js` installs a release archive the way the scripts do, and `publish-registries.yml`
publishes crates.io, npm and the MCP registry from a tag (four Node tests in `tests/npm.rs` drive the
installer against archives served from `127.0.0.1`, plus static tests holding the target table, the three
version fields, the workflow's `v*` input guard and the pinned `mcp-publisher`). What it does not
measure:

- **The Node tests run on Unix only.** The stand-in binary is a shell script, so the Windows branches
  (the `System32` `tar.exe` that reads the zip, the `%LOCALAPPDATA%` directory, replacing an `.exe`
  another process holds open) have never run.
- **No registry was published to.** Trusted publishing, the idempotence probes and `mcp-publisher` run
  only on a tag; `server.json` was not validated by the registry.
- **The tag-against-version steps in the `crates-io` and `npm` jobs are asserted by nothing.** Removing
  them leaves every test green; `cargo publish` and `npm publish` would publish whatever version the
  checked-out manifests name.
- **The installed-version check reads `--version` text.** A binary at the install path that answers the
  package's version is run without its bytes being checked against the release.

## The release builds; CI tests

`release.yml` runs no `cargo test` (#135): the suite runs in `ci.yml`, whose `check` matrix covers every
runner a release archive is built on (`every_runner_a_release_builds_on_is_one_ci_tests_on`), so a flake
fails a pull request instead of a tag. Measured before the change, on the `v0.1.0` and `v0.1.1` tags: about
50 minutes per release, and one release (`v0.1.1`, run 39043362083) failed on a bash timeout in a test that
passed on the same target the run before. What the change gives up:

- **The release profile is tested nowhere.** `ci.yml` runs the debug profile; `lto = "thin"`,
  `codegen-units = 1` and `strip` are compiled by the release build and executed by nobody before an
  archive ships.
- **`x86_64-apple-darwin` is built and never run.** It is cross-compiled on the arm64 `macos-latest`
  image; no runner executes an Intel macOS binary, and Rosetta was not tried.
- **The tagged tree is assumed to be the tested tree.** A tag is cut on the merge of a pull request CI
  passed; nothing checks that the base did not advance between the green run and the merge beyond the
  delivery rules a merge already follows.

## The waits audit

[`waits.md`](waits.md) counts the calls this repository's deliveries made because a clock passed: 247
of 17,062 tool calls in 45 Claude Code transcripts, carrying 72.95 M of 5,119.2 M context tokens. What
the count cannot show:

- **One machine, one host, one pace.** The transcripts are Claude Code's, from one person's profile, over
  four weeks. OpenCode, Pi and Codex sessions, and every subagent's own transcript, are not read. A
  slower lane or a different habit of checking CI would give other figures.
- **The classification is read from command text.** A loop is matched by `until`/`while` and `sleep`, and
  a pending CI read by the words `pending`, `in_progress` or `queued` in its output. A read that also
  diagnosed a failure is counted as a wait, so the pending-read row is an upper bound. A wait written
  in another shape is not counted, so the total is not a ceiling.
- **Context is not saving, and not billing.** The figure is the context of the requests that carried
  the calls, nearly all of it cache read. A replaced call still costs the request that reads its
  notification, and that net was not computed.
- **Why a run polled is not in the transcript.** The count shows that a run polled a file a background
  job would have announced. It cannot show whether the run expected no notification, or followed an
  instruction to wait in the foreground.
- **The 111 s is the longest MCP call this host was seen to hold, not its limit.** No other host was
  run, so raising `deliver`'s ceiling to 900 s leaves the shorter-host case unmeasured
  ([*What `deliver` does not see*](#what-deliver-does-not-see)).

## Re-running a red lane

`review_status` offers, once per epoch, `gh run rerun <run-id> --failed` for a red lane
([`review-status.md`](review-status.md)). What the offer cannot prove:

- **The offer is recorded, not the re-run.** The store row (`store_meta`, keyed by repository, issue and
  epoch) is written when the offer is made, because Estigia does not see the agent run `gh`. An agent that
  asks twice before re-running is told the head needs a correction, and a re-run that was never started
  reads the same as one that came back red.
- **"Does not reproduce locally" is the agent's word.** Nothing runs the suite on the failing job's
  platform. A failure that exists only on a platform the agent cannot run (Windows, say) can pass
  the condition and still be a defect in the bytes.
- **"Outside the change's paths" is read from the failed log.** The offer is withheld when the log names a
  path in the diff. A log that names no source path, or a path the change does not touch while a changed
  file caused it, is offered a re-run. A green re-run after a failure in a test the change touched is not
  proof the change is sound.
- **One re-run per epoch, whatever the cause.** A second flake on the same epoch is a correction and a new
  epoch, paid again.
