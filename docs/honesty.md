# What this instrument does not measure

An axis list that names what is not covered is the difference between "zero defects found" and
"zero defects found where we looked".

This is the document to read before adding a claim anywhere else in the repository: it carries what
this crate knows it does not check, and a new document that contradicts it is a document that will
be believed. It lived inside [`README.md`](../README.md) until it was 55% of that file — a reference
list a reader had to scroll past to reach the layout, which is how the most important section became
the least read.

`tests/honesty.rs` crosses the *countable* claims here against the code: the number of agents, the
number of tools, the number of doctor checks, the settings rows. A count that drifts fails the
suite. Everything else here is prose held by review.

## How this file is organised

[`docs/README.md`](README.md) is the map of the set this document belongs to. The entries are in the
order they were written, not grouped by subject: each one records a gap
a change found, with the measurement that found it. The headings mark boundaries in that order
and name the run of entries each one opens — an entry that would sit elsewhere under a subject
index stays where it was written — so this list is how to reach an entry without reading the
file.

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

- **Only Codex's hook derives its own runtime word.** `session::AGENT_RUNTIMES` maps `codex` to
  `codex`; every other agent that registers a `PreToolUse` hook (Cursor) derives `claude-…`, so it
  disagrees with `estigia goal --runtime <its own word>`. Codex registers no `SessionStart` hook from
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

- **The shipped skill does not carry its own provenance; the why of a rule is here.**
  - *Digest, not SHA* (`skill/protocols/rdd.md`). A SHA says what was committed, not what was in the
    tree: a mode change, a symlink swap or an uncommitted change ships without moving it. `target`
    treats mode as identity and hashes a symlink as its link text (mode `120000`).
  - *Branch collision.* `git worktree add` refusing a second run on one branch is a read then a write
    with no lock; the review probe produced two concurrent worktrees for one branch on Git
    2.55.0.windows.3. `start-branch` reserves locally before the first GitHub mutation.
  - *Scratch isolation.* On a five-judge panel sharing one scratch directory, one judge's script was
    overwritten and run in another's checkout, so verdicts were no longer independent.
    `skill/policies/judgment-day.md` and `skill/references/repository-delivery.md` state the rule.
  - *Delivery.* The publication lane's check runs do not populate the pull request merge box; only a
    `pull_request` event does. Whether a `merge_group` run satisfies required contexts is a
    live-repository measurement, not asserted.
  - *Closing keywords.* `Fixes #<n>` is the muscle-memory opening of a PR body, so the skill supplies
    the exact `Refs #<n>` wording.
  - *Review target halves.* Seen live (another repository's #70): the approved lineage covered 12
    paths while the pull request delivered 15; the diagnostic target is derived once and compared with
    what a reviewer is given.
  - *Launch-prompt restatement.* Nothing mechanical detects a brief that overrides the reviewer
    definition's classification rule; what a judge does with one is unmeasured.
  - *Approved republish.* An undeclared republish over an approved receipt is refused
    `approved-republish-without-cause` before it costs anything (about 550,000 tokens a round, measured
    on one issue) and is not counted as a round.
  - *Review tier.* The tier does not port the source project's six-path review-authority
    self-reference, because this repository's review authority is not separable by filename.

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
  each in its host's own dialect, each written only where `Model routing` names its role and retracted
  when the entry goes away — the exception opened while the refusal to share a dialect
  stands for the reviewers. A model is rendered into the two roles, and an `effort:` line, where
  one is named, into their Claude Code copies alone — **not** into `review-blind`, whose
  `model: inherit` is literal and whose bytes are pinned against an earlier release, so a model or
  effort named for `judge` reaches no file and is carried to Judgment Day's judges by the launch instead. An
  `effort:` line reaches a delegated role on Claude Code; OpenCode has no effort field, so an
  effort named for a role reaches no OpenCode file. That a host obeys either field is not measured
  here, and every adapter but Claude Code and OpenCode has nowhere to put a role at all. `Delegated workers` no longer exists: its effect moved into `Model routing`, so no row's
  whole effect is an installed definition, the `Applies::Inert` answer that named the rootless
  adapters is gone with the row that needed it, and `Setting::reaches_only_an_installed_agent_definition`
  is deleted with the exhaustive match it fed. `applies()` now decides on `AgentAdapter::can_gate_tools`
  alone — `Held` where the adapter gates, `Asked` for `Delivery`, `Review`, `Transitions` and
  `Boundaries` where it does not, and `Held` otherwise — and the screen's own `App::applies`, on the
  all-installed-agents page `everyone` marks, is a second decision over whichever adapters one shared write
  reaches. This decision is TUI-only: the CLI setter never calls `applies()` (`src/cli/mod.rs`). A model
  ID whose own last segment is one of the five effort words is refused, not read as a model ID: the
  slash grammar is retired, so `sonnet/high` is reported as the old `model/effort` spelling and the
  entries beside it survive, the same drop-with-report path a removed key takes. A model ID whose
  last segment is not one of those five words, such as a provider-qualified `anthropic/claude-opus-4`,
  is still one model.
  A stored routing cell holding a removed key is dropped by the next write — `config set` or
  `config edit` — whichever row it names, and reported. Taking the six workflow-state keys and the
  five retired phase keys out means a cell like
  `orchestrate=opus, in-progress=haiku` no longer parses. The tolerant read keeps every entry it can, drops
  the retired key alone, and prints a report naming it and, where a rename removed it, the key that
  replaced it; a `config set` therefore reports the dropped key and rewrites
  `Model routing` to the entries that survived — `orchestrate=opus` here — rather than to `unset`. A
  retired `Delegated workers` row folds each name it holds into the routing as `inherit` and is
  reported the same way. What is not measured: the report reaches stdout and a caller reading only the
  exit code still sees a successful write. The mechanism predates this change — the repository-scoped
  writer deliberately chose the opposite rule — and what this change adds is a report where there was
  silence.
  The installed-contract read folds that row rather than refusing it, so a contract still carrying it
  is read. It was the one state `setup` could not clear: the strict read refused the row, `setup`
  writes nothing when it refuses, and so every later run refused it identically. `status` called such a
  contract *not understood* rather than *out of date*, because `sync` was refused by the same read.
  The fold and a row
  this build cannot parse are kept in separate channels, so a mistyped row is still refused and a fold
  beside one is not laundered into a writable table. What is not measured: that the fold's note
  reaches an operator who reads only the exit code, and that a contract carrying more than one bad row
  names only the first — both channels are first-only.
  A delegated definition's ownership is looked at twice and locked neither time: `setup` decides it
  in the preflight, before anything is written, and asks again immediately before the bytes go down.
  Two observations are not a lock — a file appearing between the second read and the write is still
  overwritten, and nothing in this process can hold that path in the meantime. The same is already
  true of the reviewer's definition one pass over, and its comment says so. What **is** crossed is
  the branch: `the_second_ownership_look_refuses_what_arrives_after_the_first` reaches it through
  `pending`, which the write pass reads and the preflight does not. What is **not** crossed is a real
  interleaving, which cannot be driven from a test in one process. Two blind judges asked for that
  test and graded its absence a warning; this is the honest half of it rather than a fixture shaped
  to look like coverage.
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
  Code.** "A real second line of defence" is not established, here or in the project this crate ports
  from (`gentle-ai` `v4.0.0`, `ff77164d`), which carries the file-based tool-list mechanism on four of
  seventeen runtimes and states nowhere that a host enforces a declared list. Cursor carries only a
  coarse `readonly` boolean; Kimi's `exclude_tools` is not parsed or asserted in that project's Go. The
  claim that a malformed `tools:` line breaks Pi's loader is unverified and removed. An adapter with no
  agent-definitions root is missing a declaration, not failing to hold a control no adapter is shown to
  hold.

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

  **Atomic is narrower than it sounds.** Every finding is checked before any is written, but the writes
  are not one transaction: a write failing after an earlier one landed leaves those findings and
  reports world `committed`. A relaunch after an abandoned job starts a new reviewer; a finding the new
  answer repeats under the same `id` and `class` is read back under the same operation id, every other
  is added. A later answer giving an `id` a different `class` mints a different operation id, so across
  answers the ledger can hold one `id` twice, which is not measured. Crossed by
  `a_write_that_fails_after_another_landed_is_reported_as_committed`,
  `a_lens_report_that_fails_after_a_finding_landed_is_reported_as_committed`,
  `a_read_that_fails_after_a_write_is_reported_as_a_write` and
  `a_config_defect_after_a_write_is_reported_as_committed`. The pre-check does not read
  `operation-id-kind-conflict`, so that surfaces as a failed write after earlier writes.
  `findings_recorded` counts what the capture reads back for that lens and receipt; `findings` lists
  them by `id`, `class`, `blocking`, `lens` and `location` without text, which stays behind `review_read`.

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

  **The refuter's distinctness is a context's, not a session's.** For a `refutation-batch` input the
  binary hands a fresh tool-free reviewer the receipt, the rebuilt diff, the issue body and exactly
  the offered finding ids, and records the answer in one `record_refutation` call credited to
  `review-refuter`, a role label never a run id, so `require_distinct_reviewer` passes by construction.
  The claim rests on the launch being a new process handed only that input, and nothing checks it. A
  hand call crediting the publisher or a requester is refused `reviewer-not-distinct`
  (`the_capture_credits_the_role_label_while_a_hand_call_naming_the_publisher_is_still_refused`,
  `a_hand_call_naming_the_requester_of_a_handoff_is_still_refused`), and a hand `record_refutation` is
  refused `recorded-by-capture-only`. The answer must name every offered finding once and nothing else,
  or the whole answer is `reviewer-result-malformed` and nothing is written. The five codes of
  `REFUTATION_ANSWER_SCOPED` reject it the same way, each posed in a unit test;
  `refutation-batch-incomplete` is deliberately not one (from the recorder it means a finding was
  recorded since the `collect`, so it is `Refused`). The batch is one write: no partial case.
  The ids `review_status` offers and the ids `record_refutation` requires both come from
  `inferential_population` (every standing inferential severe finding, including one settled as a
  follow-up), measured by `a_batch_offered_for_a_follow_up_settled_finding_too_is_captured_and_the_review_moves_on`.
  Not measured: that the refuter read the candidates, and that `refuted` is right — a launched model
  can drop a true finding. A receipt holds one batch, so a wrong one stands
  (`refutation-batch-already-recorded`). `launch_and_collect_review_input_run_the_offered_refuter_and_record_the_batch_over_the_real_mcp_server`
  (Unix) drives it with a stand-in `claude`; no test runs a real reviewer CLI.

  **The validator sees a diff, a list and a criteria text.** For a `review-validator` input the binary
  hands a fresh tool-free reviewer the receipt, the correction's diff over its parent
  (`review_payload` over `parent.head..head`, the parent from the recorded lineage), the parent's
  severe findings and the issue body, and records the answer in one `record_validator_result` call.
  When the publication recorded `delta-parent=integrated` the diff runs from the merged tree to the
  head; the capture recomputes the merge, but `computed_verdict` reads the flag alone, so a flag whose
  merge no longer computes leaves the validator owed and refused `validator-delta-carries-upstream`. A
  rewritten history, or a base advance that did not integrate cleanly, is escalated by
  `computed_verdict` from the recorded `delta-range-reason` instead of validated; a conflicted merge is
  escalated although the budget measures it, since the author's resolution is not a diff a validator
  can attribute. **Not measured:** a correction published before `delta-range-reason` existed reads
  like an honest one. The prompt's receipt carries the digest of the very bytes handed over, so
  `reviewer-payload-mismatch` cannot fire for a validator and a marker without `payload-digest` does
  not block it; the two content-addressed commit ids and the isolated diff settings pin the diff. A
  validator is handed no `Read`, so a citation outside the correction can be judged only from the
  diff, and nothing checks that it says when one does not resolve. A parent standing rejected with no
  severe finding is launched with `none`; that state needs a hand-written verdict and was not measured.
  The attempt is frozen at launch (the inconclusive results already recorded plus one) and the
  operation id minted from it. A store error writing the `escalated` receipt is retried once against
  the same operation id; the retry is not reproducible in the suite, and
  `a_second_recording_of_the_same_validator_answer_replays_the_recorded_result` cannot tell a reuse
  branch that re-settles from one that returns early. The answer's shape is checked with
  `validator_shape` first (`reviewer-result-malformed`, nothing written). A conclusive `fail` and an
  `unavailable` inspection are `Recorded`: the first escalates at the limit and rejects below it, the
  second escalates with `validator_inconclusive`. `validator-result-already-recorded`,
  `validator-attempts-exhausted` and `validator-transaction-not-a-correction` pass through. **Open:**
  `invalid-validator-result` is one code for a bad shape and for an attempt that is not the next one; a
  result recorded by another process between the capture's read and the recorder's would raise the
  second, and that needs two runs holding one claim and is not measured. A validator's `pass` is a
  model's reading of a diff.

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
  `several-runs-hold-this-checkout`. A call carrying the host's session id never reaches this road.

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
  of `FULL` (`durability` in `src/transport/store/mod.rs`), so a fresh store's fifteen migration commits
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

- **Tests execute the generated plugin and *require* `node`; the count is not crossed.** Three tests
  drive it. `the_plugin_hands_the_gate_the_directory_the_call_runs_in` covers both host generations
  (OpenCode 1's `tool.execute.before` with a stand-in shell, OpenCode 2's `execute.before` with a
  stand-in `Bun.$`); `the_plugin_hands_the_gate_the_directory_every_write_lands_in` does it once per
  write-shaped tool in `GATED`, so a tool added without evidence of where it lands is a failure; and
  `the_loop_plugin_continues_while_estigia_says_there_is_work` drives the optional loop plugin through a
  stubbed OpenCode 2 context: a turn ending with a live goal, claimable work or a discharged condition
  enqueues the prompt again, one with none of them or with an unreportable count does not, `--max`
  stops a run, every stopping exit keeps the goal with its reason, and `/flow-status` reports it. A
  turn whose claim or `estigia goal` answer could not be read is retried up to a bounded number of
  attempts on the plugin's wait timer and stops only once they are spent
  (`the_loop_plugin_waits_on_held_or_pending_work_and_asks_again`, `cli::loop_stop::decide`).
  `the_plugin_declares_the_default_definition_opencode_two_loads` holds the loader shape. They fail,
  naming what is missing, when `node` is absent. Source-text assertions let the working-directory
  defect stand, which is why these execute the plugin.

  **What the loop plugin's tests do not establish.** They drive stubbed instances against one store and
  one event, so they prove an instance acts only on its own location's events and re-reads the goal
  before the enqueue; not a real host delivering one event to N instances, nor that
  `event.location.directory` and `ctx.location.directory` are spelled identically (a mismatch would make
  the loop never react). Storage has no compare-and-set, so a stop written between the re-read and the
  enqueue still lets one prompt through. **Parked prompts are not drained:** `ctx.session` does not
  expose `session.inbox.cancel` (OpenCode 2.0.16), so a queued loop prompt is delivered once after a
  stop. That `session.synthetic` with `resume: false` schedules nothing is read from the protocol
  description. `last` and the per-location watch record say only what an instance that was called
  observed: a never-called plugin, a host that never fires `session.execution.succeeded`, or an event
  during a stream disconnection (volatile) leaves no record, so "no turn end observed" is not "no turn
  ended". Resubscription is driven by a stubbed stream; why the 2026-09-30 session produced no reaction
  is not established.

  **A child session.** Running children are a map in the plugin's memory, so a child started before a
  restart is unknown and its parent continues immediately; a child whose end is never observed holds its
  parent only until the wait timer fires, then costs one continuation per interval. That OpenCode 2.0.26
  emits `session.created` with `parentID` and four `session.execution.*` events with only the session
  id, and that `ctx.session` cannot list children, is read from the installed binary. A parent whose
  turn ends between the child's end event and its own decision waits for the timer or the next turn end.

  **Event names and rendering are read, not seen.** `session.execution.succeeded` is read from the
  installed `@opencode/schema` types; that `ctx.session.prompt` delivered the prompt or that the agent
  did any work is not measured. The loop's only evidence is the tracker's answer when it asks, so a
  session repeating a prompt and writing nothing looks like progress. That OpenCode 2.0.20 shows a
  synthetic message's `description`, not its `text`, and keeps newlines, was read from the binary's
  renderers, not a live TUI. A continuation on `claimable` (`list-state`'s count of unassigned
  `review`/`ready` issues) does not prove the agent claims one. `dischargeable` counts blocked items
  whose *recorded* clause holds, so it proves what the marker says, not that the marker is the real
  condition; a malformed clause or an unreadable issue is reported as not holding (`unparsed`/`unread`),
  so a loop can stop with a discharged condition nobody counted. The `--max` count is enumerated from
  closed `status:done` issues mentioning the run, each proved by `delivery_close` (the merge-parent
  proof `verify-claim` uses), so a squash or rebase delivery is a **floor**: not counted, and the cap can
  run past it. [`what-it-writes.md`](what-it-writes.md) states the install half.

  **The wait sees counts, not liveness.** With `held + pending > 0` the loop waits on a timer. `held` is
  every open `in-progress` or `review` issue the run could not claim, not "held by a live run": a stale
  unreclaimed claim, a review item this run requested, this run's own earlier `in-progress` issue and an
  item a person parked there all count. A holder that never finishes keeps the loop waiting until
  `/flow-loop stop` or a pending `estigia goal stop-after`; `--max` bounds it only by completed issues.
  `pending` is as good as the clause: a clause naming an issue that never reaches its state keeps the
  item pending, an unreadable one counts as pending, and a malformed one is `unconditioned` and left for a
  person. `open` is counted before the 200-item cut and `count` after, so a review queue past the
  ceiling over-counts `held`; each blocked-queue count reads at most the 200 items `list-state`
  returns. The timer lives in the plugin's process: an evicted location or reloaded plugin re-arms only
  when its location boots again and the storage scan lands, so a location nothing boots keeps a goal
  saying it waits with a `next` in the past. The re-arm is tested against a stubbed `storage.scan` that
  never pages; tests use the driver's fake clock.

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

- **The transport is ported, and the Python is not shipped.** The `gh`/`git` operations are answered
  in-process; the script is in `RETIRED`, so a `sync` removes it and its directory, and nothing on
  `doctor`'s list asks for an interpreter. `skill/scripts/github.py`, its suite and the
  `tests/transport/oracle.json` corpus of recorded answers are deleted: a crossing's key fingerprinted
  the whole question, including the installed skill directory's name, so the corpus froze the
  installation's shape and could not be extended once the reference implementation was gone. **The cost:**
  the port's agreement with the retired implementation is no longer asserted anywhere. Every defect the
  crossings found is still fixed and most are held by a unit test beside the fix, but a regression in
  the port is one nothing here will notice unless a unit test names it. **Nothing here has been tested
  against a live tracker**: no check in this suite reaches the network.

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

- **Estigia can prove what it launched, not who answered.** `publish_review` freezes a coherent clean
  draft receipt over epoch, PR, head, base and manifest digest, and the binary launches each lens, the
  refuter and the validator through `launch_review_input` and `collect_review_input`. From the code, not
  the agent: the adapter is the first installed `claude` or `codex` CLI (`reviewer::select_adapter`);
  the arguments are the fixed set in `src/reviewer/adapters.rs`, with no tools, plus `--model` only when
  `Model routing` names one for `reviewer`; for a lens and the refuter the diff hashes to the receipt's
  payload digest or the launch is refused `reviewer-payload-mismatch`; the instructions are the template
  embedded in the binary.

  **What it does not establish.** Which model or account answered: a CLI that ignores or remaps
  `--model`, falls back, or runs on another account is indistinguishable, and nothing is signed. The
  validator's diff is `parent.head..head`, which no published digest covers, so its check cannot fail;
  only the `claude` arguments carry `--no-session-persistence`, so "no session persisted" holds for
  `claude` alone. The timeline records the role label, receipt and launching run, not the adapter,
  arguments or template. A reviewer that is out of quota, not logged in or missing is refused
  `reviewer-unavailable` (an authentication refusal names the `HOME` and config-dir value the child got),
  `reviewer-exited-nonzero` or `reviewer-cli-missing` and records nothing, so `release_ci` keeps refusing
  the receipt: a hand `record_lens_report`, `record_refutation` or `record_validator_result`, or a hand
  `record_review_finding` naming a `lens`, is refused `recorded-by-capture-only`
  (`a_quota_failure_on_a_lens_records_nothing_and_leaves_release_ci_refused` in `tests/pipe/part_09.rs`).
  That refuses an absent answer; it is not evidence a present one was independent.

  **Reviewer identity is still a label.** The role label is Estigia's name for the slot, not a property
  of whoever answered: two lenses answered by one model on one account carry two labels. The launching
  run is the claim holder, who may have written the change, so distinctness from the author rests on the
  process boundary of one launch. Closing it needs the adapter, requested and reported model recorded on
  the lens report and an answer signed by something other than the process that asked; neither exists.

  `handoff_review` records the exact receipt before releasing one ownership epoch, and
  `review_computed_verdict` records the verdict computed from the receipt's ledger. A computed verdict of
  any outcome recorded by a run that is neither publisher nor requester resolves the handoff, read per
  pull request against its latest receipt; only `approved` permits delivery. `release_ci` recomputes the
  verdict over the newest receipt on the receipt's own pull request, checks it against the marker, and
  checks the current draft PR and a re-derived clean target before marking ready. A Judgment Day judge is
  not launched by the binary: Estigia cannot observe that the host launched it, kept it blind or kept two
  judges concurrent.

  The distinctness rule survives only where a handoff is resolved, where an approval supersedes an
  escalation, where a retired rejected verdict refuses, and where a refutation is recorded (the four
  [`parity.md`](parity.md) names). The refutation rule compares the caller-supplied `refuter` label
  against the recorded publisher and requester run ids, never the recording `run_id`. **Decision:** left
  as a label comparison, because the recording run is the claim holder writing on a distinct refuter's
  behalf, so a `run_id` comparison would refuse the publisher's holder for a refutation it did not
  adjudicate; it stops an author only while the label is honest. Estigia asks for lens reports and
  findings because an unrecorded review is one nobody can audit, not because it can check them.

  **An abandoned pull request's handoff keeps excluding its publisher from later pull requests.**
  `review_eligibility` reduces the timeline alone, which carries no pull-request state. The exclusion is
  kept: the handoff's discharge is a distinct run's recorded judgement, not time, and a publisher that
  could escape by abandoning a pull request would be choosing its own reviewer. Held by
  `an_abandoned_pull_requests_handoff_still_excludes_its_publisher`.

  None of it proves an independent context existed or establishes lens count, concurrency,
  independence, blindness, same-finding identity or composition — nothing checks that the run
  launched the lenses the tier licenses. A closed-issue `verify_claim` accepts the follow-up only when the review's registered publication, the closing PR, and the merge commit's parents (reviewed base, then head) all agree; any other close is still `issue-not-open`, and an unreadable closer listing is a failed read. The refusal tells the two apart by its message: when the closer is still the review's registered publication and the run still holds the issue, it names the reopen-and-transition continuation instead of calling the close somebody else's. It cannot prove the lenses
  read those bytes or that their findings were honest. An OpenCode run launching Task lenses is that
  same instruction, not a proof those Tasks ran or were blind. A marker can still be forged by a collaborator
  acting outside Estigia. The composition is the tier's lens count rather than operator-selected — the
  `single`/`two blind`/`five blind` dial and the judges are gone — but derived is still not observed: what a
  run actually launched remains not an observation the harness can make. The enforced floor remains
  one computed exact-receipt verdict: the transport has no per-reviewer **verdict** marker — `review_verdict`
  is retired and refuses — and does not implement multi-verdict adjudication. It does have a per-finding
  marker, credited to a named reviewer and bound to the exact receipt — so the *evidence* the lenses
  produced is recorded separately, and the *verdict* it reduces to is computed from it, not chosen.
  The entry below says what that record does and does not carry.

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
  | `skill/policies/blind-judges.md` | 42,284 | file on disk |
  | `skill/policies/judgment-day.md` | 15,969 | file on disk; loaded only when the operator explicitly asks for Judgment Day |
  | Installed `skill/SKILL.md` | *rendered* | `skill::rendered` over `Config::default()` — not a file size; the shipped block is rewritten |
  | `skill/bindings/github/delivery.md` | 42,206 | file on disk; a **publishing** run loads this companion per `SKILL.md`'s delivery row |

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
  `release_ci` and `goal_completion` read the computed verdict alone. An `accepted` marker releases
  nothing. A `rejected` one from a distinct reviewer refuses release on its exact receipt at **every**
  tier as `legacy-verdict-rejected`, or `rejected-on-insufficient-evidence` when the rejecting
  reviewer's severe finding declares `insufficient`, because the marker binds those bytes. The cost: a
  receipt a reviewer rejected cannot be cleared by recomputation even when the finding was later
  refuted; a republish is a new receipt. `repair_rounds` reads a rejected marker as a receipt that
  stands rejected (a republish over it is a correction) and an `accepted` parent as approved; the
  `approved-republish-without-cause` gate treats an accepted parent as one a finding must answer; a
  distinct verdict still resolves the handoff it was recorded against; a computed marker of any outcome
  from a run that is neither publisher nor requester resolves a handoff, `rejected`/`escalated` stand a
  receipt rejected, an `approved` parent is the acceptance a new candidate is measured from, and where a
  receipt carries both kinds the computed marker decides acceptance. Estigia cannot tell any of this from
  a forged marker: they are comments. **Decision:** the both-judges-confirm rule is retired. A lens's
  `deterministic` finding rejects the receipt without a second reading, and only a refutation batch can
  drop an `inferential` one. `record_review_verdict` refuses every call as `review-verdict-retired`; its
  arguments are untyped strings, so a zero or non-numeric `issue` or `pr` is answered with that code and
  every supplied argument is named (`ignored` or `receipt`). A refused call returns before any pointer
  effect, so `the_retired_verdict_tool_leaves_the_pointer_alone` reads the registration itself. A
  timeline mixing a computed approval with an aggregate rejection stands rejected for every reader, and
  the republish gate no longer refuses what release refuses (ending a deadlock), but an old marker
  written by a forbidden run can make a receipt stand rejected, and so a republish a correction, for a
  receipt release still accepts. An `approved` receipt is recorded by `record_computed_verdict`, so an
  approval is final against `escalate_review` and a distinct run's approval supersedes an escalation; a
  receipt credited to a run the review forbade supersedes nothing. A receipt is a store record only a
  run holding the review claim writes through the tools; nothing stops somebody with the store.

- **`Merge strategy` is retired.** The row once took `squash` and `rebase` while `closing_merge`
  recognised only a two-parent merge commit. The variants are gone, `closing_merge` hard-codes the merge
  commit, an installed row loads and changes nothing, `config set` refuses a new write with
  `merge-strategy-retired`, and a delivered SHA that is not a two-parent merge refuses
  `delivery-topology-mismatch`.

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
  disk.** `render_reviewer_agent` takes a `Seal` orthogonal to `Evidence standard`, and
  `reserved_reviewer` (`src/harness/roles.rs`) answers which of nine reserved names carries which
  seal — two of them installed, the judges, and seven launched by `launch_review_input`;
  `reserved_reviewer_row` answers the same name's seal and its own template together. Both doors
  (`src/cli/mod.rs` and `src/harness/hook.rs`) render with the row's answer, so a sealed judge is gated
  by the embedded template rather than whatever `tools:` line its installed file holds.
  `a_tampered_sealed_definition_does_not_widen_what_its_doors_allow` installs the sealed definition,
  rewrites its grant on disk to `Read, Grep, Glob, Bash`, and requires both doors to keep refusing
  `Read`; removing the sealed name from `reserved_reviewer` turns only that test red. `Seal::Sealed` is
  constructed seven times in `RESERVED_REVIEWERS` (`review-blind-sealed`, five sealed lenses, the
  refuter `review-refuter`), and `setup`'s install loop only destructures each row and hands it to the
  renderer, so a shipped `review-blind-sealed` rendering **is** sealed. Which definition a panel actually
  launched is the composition entry above.
  **The sealed arm's rendering was measured on Claude Code on 2026-09-02, and it changed as a
  result.** Seven one-shot sub-agent definitions were launched, each with only its own `tools:` line:
  `none` and `TodoWrite` were refused at launch (*"unrecognized"*), `ReportFindings` and `ListAgents`
  as *"not available to subagents"*; `[]` and `ToolSearch` launched with zero tools but `[]` is read by
  `tools_line` (`src/harness/roles.rs`) as `None`, "no policy at all, every tool permitted", which would
  invert "sealed", and `ToolSearch`'s job is fetching other tools' schemas; `WebSearch` launched holding
  exactly one tool and no filesystem reach. **It is what the sealed arm renders** (`src/setup/mod.rs`):
  sealed is not inert, since that one tool reaches the public internet, which `src/harness/roles.rs`
  names among the tools this gate's matcher never sees. Nothing here measured any host but Claude Code.
  **Four things are unmeasured.** Whether the host itself refuses a call the grant does not name. Under
  every adapter whose matcher this repository writes, `Read`, `Grep` and `Glob` do not wake this gate
  (`src/setup/render.rs` names `Bash` and not those three); two adapters are written with no matcher and
  what those hosts send is not established. The prelaunch check refusing a project-local file
  re-claiming `review-blind` (`skill/policies/judgment-day.md`) is Claude-only. And a sealed judge acting
  only on what its launch handed it is a paragraph `render_reviewer_agent` writes, not a proof.

- **`reviewer_is_static` keeps the plain cross product `Evidence::all()` × every seal
  `RESERVED_REVIEWERS` carries; no reachability predicate narrows it, and that is a decision.** A
  predicate would couple this acceptance check to the settings table rather than the renderer; the
  sealed arm names one tool, `WebSearch`, that cannot become a repository read, so admitting it cannot
  widen what a judge may do past that bounded reach; and refusing the bytes would refuse a strictly
  safer definition an operator hand-wrote for bytes this binary's renderer produces. This holds only at
  the byte-acceptance point. `setup`'s own `validate_reviewer_definition` (`src/setup/mod.rs`) is a
  separate, path-based check: a loop of two passes, one per installed `RESERVED_REVIEWERS` row, that
  refuses a definition declaring `review-blind-sealed` for reclaiming that reserved name outside
  Estigia's canonical path. Measured: with a sealed rendering over an owned
  `~/.claude/agents/review-blind.md`, the `review-blind` pass accepts and the `review-blind-sealed` pass
  refuses `reviewer-definition-unowned`, so `setup` aborts before writing. **The cost:** an operator who
  puts a strictly safer definition at the unsealed path is locked out of `setup` and `sync` until they
  `move <path> aside before setup`, which the refusal names.

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

  **The equivalence between "every line of the span is changed" and "one hunk contains the span" holds
  only because `changed_new_side_lines` runs `git diff --unified=0 --no-renames
  --inter-hunk-context=0`**, so new-side ranges are ascending and separated by at least one unchanged
  line. That precondition is probed, not proved: four fixture shapes over one throwaway repository
  (modify 2/delete 3/keep 4/modify 5 gives `2` `4`; modify 2/keep 3/delete 4/modify 5 gives `2` `4`;
  delete 2-3/modify 4 gives `2`; modify 2/keep 3/modify 4 gives `2` `4`), never `2` `3`, because changes
  contiguous in the old file are one hunk. Four shapes is not a proof. Ranges split where this probe
  found them whole make the predicate too strict (the safe direction); ranges **fused** make it too
  permissive. Four pins prevent the second: `--inter-hunk-context=0` on this invocation's list rather
  than `pinned_diff_args` (that helper is shared with the human-read review-target payload),
  `GIT_DIFF_OPTS` removed from the child in `super::run` (git applies it after argv, so no flag can win;
  measured `-u3` widening `@@ -5 +5 @@` to `@@ -2,7 +2,7 @@`), `--no-ext-diff` and `--no-textconv`
  (a textconv driver makes the `@@` numbers stop referring to source lines). With
  `diff.interHunkContext=5` a head changing lines 3 and 7 renders one hunk, so a location on line 5
  would settle without the flag.

  **A path the diff touched but that carries no new-side hunk settles by having been touched at all** (a
  pure deletion, deleted file, binary or mode-only change): an empty range vector settles by the exact
  `diff --git a/<path> b/<path>` header having appeared; no header settles nothing.

  The pathspec is `:(literal)<path>`, not `<path>` after `--`, because pathspec magic like `:!x` selects
  every other path. `:(literal)` still prefix-matches directories (measured on git 2.55), so only hunks
  under a header naming **exactly** `path` are kept. `parse_finding_location` refuses any path containing
  a colon, which also rejects `path:line:col`.

  **Renames.** With one path in the pathspec, filtering runs before rename detection, so a file renamed
  and edited shows as a deletion of the old path and an addition of the new (measured on git 2.55; the
  cause is pathspec ordering, not `diff.renames`). A location naming the **old** path is accepted for
  any line (the deletion-only carve-out) and one naming the **new** path for any line in the file. Both
  are what asking `git diff` for one un-renamed path gives back. `--no-renames` makes this hold by flag.

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

- **Evidence standard: reading carries the basis paragraph.** Both arms of `render_reviewer_agent`
  append `VERDICT_BASIS_REQUIREMENT`. An existing `reading` install is upgraded by `setup` because its
  previous bytes are on the historical ledger or the record holds the digest of what this build wrote at
  that path; launch refuses `reviewer-canonical-unavailable` until that setup. A `reading` judge has no
  commands to name, only the fact it ran none, which the requirement accepts as a basis.

- **A frozen-body ledger cannot be a definitive fix.** `setup` identifies an installed reviewer as
  ours-older by the ledger of every body this or an earlier binary could render for `review-blind`
  (`REVIEWER_TEMPLATE_BEFORE_*` in `src/setup/mod.rs`); a digest cannot identify an install that predates
  it. The ledger needs a human to freeze each previous body **before** it ships (an author who never freezes one leaves nothing a test can see); `--allow-source-build`
  installs unpublished bytes, and the same refusal recurred four times on one machine. So `setup` also
  records a digest of what `write_reviewer_definition` last wrote at the reviewer's exact path
  (`written_outside` / `note_written_outside` in `src/skill/record.rs`); an absent digest falls through to
  the ledger. The ledger answers *is this one of the bodies Estigia ever shipped*, the digest *did anyone
  touch this since Estigia last wrote it*. Not self-healing: a published, superseded body still needs
  freezing, and a machine skipping from one published template to a later without running `setup` has no
  digest for the middle state. `historical_reviewer_renderings` renders every frozen template across every
  `(name, seal)` pair `RESERVED_REVIEWERS` carries (previously only `review-blind` unsealed, so
  `review-blind-sealed.md` from an earlier binary matched nothing).
  `reviewer_is_static_for_seal` (what gates a *running* judge launch, `src/harness/roles.rs`) is
  unchanged on purpose: a body accepted for its digest alone is not a current rendering. The single-writer
  condition is unenforced: only `InstructionFile::ClaudeCode` gets an `agents_root` today (matched
  exhaustively in `src/setup/mod.rs`), so one skill root's record is the only one asked; a second adapter
  sharing that directory under a different skill root would be invisible to it. Measured on one real
  machine, one adapter and one name (`review-lens`, on 2026-09-05): the unfixed build exited 1 with
  `reviewer-definition-changed` and the fixed build exited 0, and the first fixed-build `setup` reported
  `changed 0 file(s)` yet recorded all seven digests (the `Change::Unchanged` path bootstraps). A body
  already stranded with no recorded digest and in no ledger is not rescued retroactively and still meets
  `reviewer-definition-changed` once.

## The control surface and what it records

- **Estigia cannot make a person decide or start a turn.** For a human adjudication that is not a
  routine wait on the ordinary review, CI, identity, and delivery-permission gates it can preserve
  the built branch, PR, receipt, checks, and evidence, and record `blocked` with the exact decision
  or exit condition and discharger before ownership is released. That durable record is not a
  scheduler, notification guarantee, or decision; only the named person can discharge it. A routine
  wait on those gates stays in `review` with its exact blocker, even when a named person must supply
  the permission. The Decision Gates table in `skill/SKILL.md` owns those destinations.

  **One shape of restatement is held mechanically and the rest is held by review, and four judges
  measured where the line falls.** Four copies were removed by hand — the incident ledger's I09
  protection column, the hard-wrapped handoff bullet beside it in the same file, a five-row table in
  `repository-delivery.md`, and the routing tail of `SKILL.md`'s own step 6. A guard was written to
  keep back the first of those and no other — the table is the *surviving* guard's, and an earlier
  version of this paragraph credited one guard with both. That guard counted how many workflow states
  a **source line** named, and both judges of its receipt defeated it three ways, and the three are
  worth keeping apart because a later reader will want to know which test each one belongs to. It **did** catch I09's protection column, which is the copy it was written for. It
  did **not** catch the hard-wrapped handoff bullet in the same file: that restores byte for byte and
  passes, because no single line of it reaches the threshold. It could not see a routing table at all,
  which names one state per row by construction — what a renamed header evades is the *surviving*
  guard, not this one, and conflating the two was a mistake in an earlier version of this paragraph.
  And joining two hard-wrapped lines of ordinary prose in this very file reddens it without a word
  changing: the pair that does it enumerates the state *vocabulary*, four of those names backticked,
  and enumerating the names is not restating the routing. The substring effect is real and separate —
  `already` contains `ready` and `reviewer` contains `review`, which inflates the count on many lines
  of this file — but it is not what defeated the guard here, and an earlier version of this sentence
  said it was. Measured at both revisions the guard existed at: exactly one adjacent pair crosses the
  threshold in each, it is the same vocabulary enumeration both times, and deleting every `already`
  and `reviewer` from it leaves it over the threshold still. That
  last one is why patching was the wrong move: a guard that reddens on correct prose is not narrow, it
  is aimed at the wrong object. The guard was withdrawn rather than patched: it measured a source line while the rule
  it was written for is about a **statement**, and the distance between those two is where all three
  defeats live.

  So what is left, exactly. **Two tests hold something**, and an earlier version of this paragraph
  named only the first, which a judge measured as understating the guard shipped beside it:

  - `every_named_disposition_uses_a_transport_state` crosses the Decision Gates table against
    `STATES` and proves every state is *reachable* — **not** that any particular condition routes to
    the right one. Swapping two destinations in that table leaves the suite green, measured.
  - `the_decision_gate_is_not_restated_as_a_second_table` holds two spellings: an arrow followed by a
    backtick — the assertion is on `-> \``, so it is *stricter* than the Decision Gates table's own
    notation and catches an arrow into anything backticked at all — in any of the four companion
    documents, and a table in
    `repository-delivery.md` whose header cell reads `| Destination |`. Restoring the five-row table
    this change deleted, byte for byte, reddens it. **Renaming that header and keeping the pointer
    sentence restores the whole table with the suite green** — measured by a judge, and the hole is
    named here because the guard cannot name it itself.

  Everything else is held by review: a restatement in prose that uses no arrow, in any of those files,
  passes. That is not a hypothetical — the handoff bullet this change removed restores byte for byte
  and nothing objects.

  And `SKILL.md`'s own steps restate the routing outside the section the parse is bounded to;
  `rg -n 'analysis|ready|in-progress|review|blocked|done' skill/SKILL.md` finds them, and the number of
  them is deliberately not written here, because a count in this paragraph is the class of claim this
  document keeps having to correct. Step 7 gained a pointer to the table and kept its restatement:
  step 6's routing tail became a pointer in this same change while step 7's restatement stayed, so
  what left the contract is step 6's copy and what remains is step 7's.
- **One review-protocol readback is held by no test, and one is held only by a string search.** The
  review protocol's guards were mutated one at a time — each disabled, the whole suite run, the tree
  restored from a byte copy. Held by a test that goes red: all three enforcement points of the
  requester exclusion (`claim`, `reclaim`, the review queue), the queue's fail-closed candidate read,
  the verdict's distinctness rule on *both* its halves, its live-claim requirement, the receipt's
  exactness on each write path **before** it writes, the CI-release gate, the comment escaping, the
  read-side requester filter, and every field-shape validator on the four markers — receipt widths,
  the handoff's authority, target, timestamps and its blocker/discharger, the verdict's two
  identities and its outcome vocabulary, and the finding's class, origin, identity, evidence and
  impact shape.

  That sentence used to say "on the write paths" without the qualifier, and it was false: neutering
  both of `handoff_review`'s receipt checks left the suite green, and a handoff recorded against a
  superseded receipt excludes nobody while its ownership epoch has already gone — the publishing run
  eligible again for the item it may not review, which is this repository's own livelock with an
  audit trail. It is held now. The *second* copy, the one guarding a retry, is a different matter and
  is named below.

  **Many of the smaller ones are not, and this entry does not claim to have found them all.**
  `handoff_review`'s post-release readback, which proves the released ownership epoch is no longer
  authoritative, can be disabled with the suite entirely green. Its retry-path receipt recheck is
  caught only by `the_compound_handoff_records_before_release_and_checks_review_afterwards`, which
  searches the source text for the call: that catches deletion, and would not catch the call being
  made and its answer ignored. Beyond those, an adversarial sweep of forty-nine mutations found a
  further score of defensive checks — replay-path repeats of rules whose first-write copy *is* held,
  operation-id and marker-field validators, idempotency and visibility readbacks — that can each be
  removed with the suite green. They are one class: belt to a brace that a test does hold, which is
  why the load-bearing list above is the one this file stands behind.

  Two earlier versions of this entry were wrong, in both directions, and how they were wrong is the
  useful part. The first named five checks as unmeasured on somebody else's report rather than a
  measurement run here — one of the five was in fact held, and there were more than five. The second
  said only two were unheld, which understated it by roughly a factor of ten. A count nobody counted
  is exactly what this document exists to refuse, and it refused itself twice before this sentence
  was written.

  One caution for whoever mutates next: this crate has tests that anchor on exact whitespace in
  `claim.rs`, so a mutation harness that rewrites line endings — Python's text mode on Windows will,
  silently — produces failures that look like a guard being caught and are not. Two of the
  measurements behind this entry had to be discarded and rerun for that reason.
- **`ensure_draft`'s two doors are closed.** This entry named them and it is kept as a record of what
  the gap was, narrowed to what is now true. It runs `gh pr ready --undo`, a remote write, and both
  its `draft-readback-failed` stop and a failed `view_pr` read used to answer *nothing was written*
  afterwards — the same lie the post-push refusals had already been fixed for, sitting before the push
  rather than after it, which is how both fell outside the bar the issue set. `un_readied` carries the
  world on both now, and both are driven end to end: the read door by a readback that fails, the stop
  door by a pull request that comes back still ready.
  What the second door is worth is more than a report. Wrapping the `draft-readback-failed` condition
  in `if false &&` used to leave the whole suite green, so **nothing stopped a still-ready reused pull
  request exposing the new head to CI** — the barrier that refusal exists to hold. That mutation is
  red now, and so is routing its refusal through the shared wording: doing so replaced the action
  naming the hazard with one inviting an operator to `gh pr ready`, which is the exposure itself.

  Three more, from the same reviews and left with their measurements. The `[world-action]` guidance
  naming `Refs #<n>` is held by no test — stripping it from either refusal leaves the suite green —
  though the issue lists it under *unchanged*. `Answer::already_wrote` decides which exit-code arm
  `translate` takes and sits outside the `exit-code` population fingerprint, and that arm's
  `StatusRequired` axis is held by the fingerprint alone: a tripwire that says *go and read this*
  rather than a test of what it does. The pull-request body used to be read three times — by the scan,
  by `edit_pr`, and by `pr create` — so a body edited between the scan and the write was published
  unscanned, and the `Closes #<n>` it could gain auto-closes the issue on merge. It is one read now,
  hoisted above the pull-request listing and carried down as bytes, which is the *one place rather
  than three* this entry called the fix.

  Two smaller things found in the same review and left: the commit-range scan is one function now, but
  its two callers still read from different checkouts — `publish_review` from the isolated one where
  the commits are, `assess_autoclose` from the repository root — and unifying that changes which tree
  a standalone `check-closing-keywords` inspects, which is a decision rather than a tidy-up. And
  `keyword_sources` carries two shapes under one key, a list of strings from the precondition and a
  list of `{where, text}` from `assess_autoclose`. Nothing parses it today, which is the only reason
  that is a note and not a defect.

  The closing-keyword scan was handed the delivered issue and reported only references to it, so a
  pull request body saying `close #464` closed #464 on merge with neither the pre-push scan nor
  `check-closing-keywords` objecting — the incident recorded earlier. The scan now reports every
  reference, the pre-push refusal takes any of them, `assess_autoclose` carries the ones naming
  another issue as `other_issue_keywords`, and both the post-push arm and `check-closing-keywords`
  refuse them. What remains unread: `assess_autoclose` returns before it scans when the delivered
  issue has no closing pull request, so a body closing another issue from a pull request that does
  not close the delivered one is refused before the push and not by the standalone check.

- **The follow-up's single-sourcing is named, not observed.** `src/transport/commands.rs`'s
  `mandatory_follow_up` and `src/transport/claim.rs`'s branch-link follow-up both interpolate
  `closing::FOLLOW_UP`, and each call site's test compares the emitted text to the constant's value
  rather than to its source. Re-inlining a byte-identical copy at the `check_closing_keywords` site,
  verified applied with no remaining reference to the constant, leaves both tests green — measured by
  making that edit, not reasoned from the shape. What the tests hold is that the text is right; which
  text it came from is not observed. The clause the test does hold is the transition: deleting
  `transition --to done` from `FOLLOW_UP` reddens
  `follow_up_names_both_the_transition_and_the_release` — measured by making that edit.

- **Nothing enumerates the refusals that write before they refuse, and nothing can.** `Answer::already_wrote`
  decides which arm `translate` takes, and a refusal earns `Committed` only by carrying
  `"world": "committed"` in its own payload. The enumeration question was decided rather than
  left implied: no test and no guard enumerates the stops reachable *after* a write, because
  reachability is a property of the **interleaving** — a marker is posted, a later read comes back
  without it — and not of the source text. The same function holds stops on both sides of its own
  write — `claim` checks the horizon and the handoff exclusion before it posts and adjudicates after,
  and the projection loop stops without having written whenever an earlier pass did — so a walk of the
  source cannot place a stop relative to a write, and a hand-kept list of the sites would be a second
  copy of the control flow that drifts silently. What holds each repaired site instead is a test that
  drives the decision to the refusal and reads it through `translate`; that is what `lost-claim-race` and
  `lost-reclaim-race` have, and what the five — `claim-operation-no-longer-current`,
  `ownership-changed-projections-repaired`, `holder-runtime-missing` from the projection loop,
  `ownership projection write did not read back exactly` and
  `ownership-kept-changing-before-projection` — gained, together with
  `reclaim-operation-no-longer-current`, so the two verbs answer a retry alike. The class has now
  been closed four times — the post-push refusals, then `ensure_draft`'s two doors, then the two lost
  races, then the five — and each sweep repaired the instances it could see. The fourth sweep's
  reading also found sites it left silent, named here so the fifth sweep does not have to find them
  again: `claim-operation-expired` on both its readings, `claim-metadata-mismatch`,
  `reclaim-metadata-mismatch` and `unassign-metadata-mismatch`, the operation-conflict family —
  `claim-operation-conflict`, `reclaim-operation-conflict`, `unassign-operation-conflict` and
  `operation-id-kind-conflict` — `resumed_release`'s two stops and `unassign`'s post-write
  `held-by-other-holder-disappeared` all answer `NotStarted` over a marker that is on the timeline.
  None of them was shown unreachable, and
  none is covered by an enumeration for the reason above. What it costs was measured rather than
  supposed: on another repository's #318 a run was told twice that nothing had been written, and left two
  control markers on an issue that no verb reaches.

- **The control surface reaches further than it did, and what it costs is here too.** A
  write whose path lies outside every checkout the claim covers is answered `outside-the-claim`
  without asking the tracker — right for a scratch note, right for a `Boundary` write because those
  stay gated, and wrong for a `Routine` write to a file that governs the harness. The instruction file
  each adapter's `setup` writes its workflow-authority directive into was exactly that, and so were
  `~/.claude/settings.local.json` and `~/.claude/agents`.

  Counted on one platform: ten of the eleven adapters' instruction files answered `Routine` at the
  base commit, OpenCode's being already covered by the `.config/opencode/` entry. Counting spellings
  rather than adapters adds gemini-cli's `%APPDATA%` path, and the two `~/.claude` entries make
  thirteen. The basis is stated because three drafts of this sentence carried a bare number, each
  measured wrong by a different reviewer, and the fourth said twelve over a list of thirteen.

  The instruction files are derived from the adapter table rather than spelled, the way the skill tree
  is derived from `skill::DIRECTORY` — a hand-spelled copy agrees with the installer only until
  somebody renames one, which this crate has already paid for once. The crossing that keeps them
  honest is `every_control_file_an_adapter_has_is_one_the_gate_measures`: it resolves the real path
  per adapter, on all three platforms, under two `XDG_CONFIG_HOME` layouts, on both roads, and asks
  about the bare root as well as a file inside it. Every one of those four dimensions was added because a reviewer found a hole the crossing
  could not see, and the holes are the entry below. Only the first of them was then found by the
  crossing itself, once it had the dimension — the rest were measured by people, which is the
  honest attribution and not the flattering one.

  And the crossing catches a stale fragment **only where nothing else already matches the path**.
  Measured by staling each of the eleven in turn — last component replaced, directory kept — and
  running the crossing on its own. At the base it reddened for eight and stayed green for three:
  continue, cline and windsurf, whose paths their rules directories cover independently. **At this
  head it reddens for one and stays green for ten**, and the one is cursor.

  That is a **cost of the per-project entries**, not a mistake in them, and naming it is the only
  reason this paragraph exists. `claude.md`, `agents.md`, `gemini.md`, `qwen.md` and `crush.md` are
  matched as whole segments at any depth, and every adapter's **home** instruction file is named
  exactly one of those — `~/.claude/CLAUDE.md`, `~/.agents/AGENTS.md`, `~/.codex/AGENTS.md`,
  `<xdg>/opencode/AGENTS.md`, `~/.gemini/GEMINI.md`, `~/.qwen/QWEN.md`, `<xdg>/crush/CRUSH.md`. So
  seven fragments the crossing used to hold are now covered incidentally by a literal that has
  nothing to do with them, on top of the three their rules directories already covered. Cursor is the one left
  because `estigia-workflow-authority.md` is neither a per-project root file name nor inside a rules
  directory this crate derives. Three other fragments are not root file names either —
  `.continue/rules/estigia.md`, `.cline/rules/estigia.md` and `windsurf/memories/global_rules.md` —
  and they were already covered by their own derived directories, which is the three the base
  measurement named.

  Nothing is thereby unguarded, and that was measured rather than assumed:
  `the_spelled_instruction_files_and_the_adapter_table_agree` was run against the same eleven
  stalings and **reddens for all eleven**. What narrowed is the crossing's own diagnostic value, and
  this document is the only place that would say so.
  OpenCode's *was* caught at the base, because the crossing walks the relocated `XDG_CONFIG_HOME`
  layout where nothing else matched `<moved>/opencode/AGENTS.md`. Four drafts of this sentence have
  been wrong now — it was deleted with the version of the fix it described, restored saying four,
  inverted to read as though eight was the small number, and then re-asserted later as a
  re-measurement that had in fact re-run only three of the eleven and reasoned about the rest. A
  blind reviewer of that head ran all eleven and found one.

  **What the missing dimensions were hiding.**
  `%APPDATA%\gemini\settings.json` — where this harness registers Gemini's own deny hook — answered
  `Routine`, because only the POSIX spelling with its leading dot was listed. `opencode`'s plugin,
  that adapter's only deny mechanism, and `crush`'s settings answered `Routine` whenever
  `XDG_CONFIG_HOME` was moved. Four entries carried a trailing slash, which split the two roads:
  `surface_of` appends a separator, so `rm <dir>` was `Boundary` while a write to the bare directory
  was `Routine`. And the crossing asked only about writes, so a path containing a space — the shape
  that produced the `cli/hosts.yml` entry — would have arrived unnoticed. All closed here, each with
  a dimension of the crossing that now holds it.

  **Gating a file in a directory the host reads whole is defeated by a neighbour.** `paths_in`'s own
  comments say so for two of them: Continue applies any rule with no frontmatter, and Cline loads its
  rules directory for every task. Estigia's filename was gated and the directory was not, so
  `~/.cline/rules/zz-override.md` answered `Routine` — and a sibling saying *Estigia is retired*
  changes what an agent is told this harness may enforce without touching a `Boundary` path.

  Two more directories are named alongside them on a weaker basis, and it is worth being exact about
  which: `windsurf/memories` and `.cursor/rules` are locations those hosts read, but `paths_in` says
  nothing about them reading a directory whole — its Windsurf comment is about a six-thousand
  character cap on one file — and Estigia writes nothing into `.cursor/rules` at all. They are in the
  population because the restated rule covers them, not because this crate has verified the
  read-whole behaviour. A first draft of this paragraph claimed all four were documented; a reviewer
  read `paths_in` and found two.

  Three of those four were hand-spelled entries and were derived for a while. The
  derivation was later removed: `windsurf/memories/` is a literal in `CONTROL_SURFACE` beside `.continue/rules/` and
  `.cline/rules/`, so every path that answered `Boundary` still does and an install an earlier build
  wrote stays gated exactly as it was.
  `the_rules_directories_of_continue_and_cline_stay_gated_without_an_adapter_deriving_them` goes red
  when either of those two literals is removed, and
  `a_retired_hosts_instruction_directory_is_still_a_boundary` goes red when Windsurf's is — measured:
  removing it also reddens `a_sibling_in_a_rules_directory_is_a_boundary_too` and
  `the_control_surface_matcher_answers_its_corpus`. **What removing the derivation loses:** a future
  adapter's rules directory is no longer a compile-time question. Nothing forces its author to say
  whether the host reads a directory whole, and such a directory is gated only if somebody adds the
  literal. The weaker basis above is unchanged: whether a host reads a directory whole is a fact about
  somebody else's product, and neither the derivation nor the literals ever checked it.

  **The cost, which the population's own declaration understated.** That declaration says a false
  positive costs one tracker read "and that is the direction this chooses on purpose", illustrated by
  a one-character typo on a path already listed. A `Boundary` never rides the renewal window and never
  stands aside outside the claim, so it is a live `gh issue view` every time — measured, not inferred:
  a reviewer drove 30 invocations of each class through the real hook against a `gh` with a real
  process boundary and counted **30 tracker calls out of 30** for a `Boundary` against **0 out of 30**
  for a `Routine`. With no network it is a refusal rather than a delay.

  **What that read costs is the number to distrust, and three independent measurements disagree.**
  0.61–1.22 s (mean 0.93, n=8); 0.58–0.92 s (mean 0.66, n=8); and 0.85–3.20 s (mean 1.74, n=10) —
  the last one roughly double the first two and 2.6× their ceiling, taken against the same tracker on
  a different machine and a different network. Against 0.05–0.42 s for a `Routine` write inside the
  renewal window, likewise machine-dependent. So the honest statement is a **round trip to GitHub per
  write to a watched path, between half a second and three seconds**, and any tighter figure here is a
  sample of one environment rather than a property of the change. The earlier drafts of this paragraph
  quoted the first measurement alone as though it bounded the cost, which is what a reader would have
  believed.
  Two attempts at the `XDG_CONFIG_HOME` fix were wrong in opposite directions before this one, and
  both were found by reviewers rather than by a test. Dropping the `.config/` prefix left `opencode`
  as a bare directory name matched anywhere: `node_modules/opencode/**`, `packages/opencode/**` and a
  checkout *named* `opencode` answered `Boundary` on every file in them. Replacing the directory
  entry with three tails then **loosened** it — everything under `~/.config/opencode/` that Estigia
  does not write went `Boundary` to `Routine`, which is an existing entry's sensitivity changing and
  was recorded here as an over-gating fix. Not a countable set: it is whatever OpenCode itself keeps
  there, and three drafts of this sentence said *nine* without anybody being able to enumerate them.
  A reviewer measured `auth.json`, `config.json`, `themes/`, `command/`, `instructions/`, `tui.json`
  and `mcp.json` among them, and the bare directory. The directory entry is back *beside* the tails, which
  cover the relocated root it cannot.

  What that leaves, measured rather than claimed: `node_modules/opencode/index.js` and
  `packages/opencode/src/main.ts` are `Routine` again, and `node_modules/opencode/agents/**`,
  `node_modules/opencode/plugins/**` and `node_modules/opencode/opencode.json` are not — a vendored
  copy of that agent with those exact subdirectories pays a tracker read per write. An earlier
  sentence here said six shapes had come back, then three; the number was never the point and
  neither figure was measured. This is the list.

  What over-gating remains, measured rather than argued. A fragment ending in `/` names a directory
  and the matcher honours that, so a name that merely *ends* alike — `.estigiaignore`,
  `skills/flow.md`, `.claude/agentsmith.md`, `.cursor/rulesets.md` — stays `Routine`. A fixture holds
  that direction, which nothing did before: every other guard here asserts something *is* `Boundary`,
  so over-gating was invisible to all of them.

  The left side is anchored for two kinds of fragment: those beginning with a **dot**, and those
  naming a **directory**. A dot-directory is always a whole segment, so `my.claude/agents` is not
  `.claude/agents`. Every real target of `opencode/agents/`, `opencode/plugins/`, `windsurf/memories/`
  and `skills/issue-flow/` has that first segment whole too, so those anchor without losing anything —
  measured, `~/.config/opencode/agents` stays `Boundary` on both roads.

  It took three attempts, and each one is why the fixture lists what it lists. The first anchored
  `ends_with` and left `contains` alone, so a bare `my.claude/agents` came back `Routine` while a file
  under it stayed `Boundary`, and three documents said the case was closed. The second anchored the
  dot fragments only — which left the road split alive on precisely the directory entries this change
  added: `surface_of` gives every token a trailing `/`, so `/repo/.opencode/agents`, `/repo/.opencode/plugins`,
  `/repo/xyzopencode/agents` and `/repo/notwindsurf/memories` each answered `Routine` to `Write` and
  `Boundary` to `rm`. The suite was green over all of them, because nothing asserted the `Routine`
  direction for a non-dot fragment. All four are in the fixture now, on both roads.

  Anchoring cost coverage on the shell road three separate times, and the shape of that mistake is
  worth more than the fix. `surface_of` builds a view of the command and matches the fragments against
  it. Anchoring means a fragment now needs a separator in front of it, and each attempt supplied one
  in a narrower place than the shell can put punctuation:

  1. **Split on whitespace, append a separator.** A relative operand then has a *space* in front of
     it and is never at position 0, because the verb is. Removing the state directory by its bare
     relative name, or truncating the file the gate is registered in, went `Boundary` at the base to
     `Routine`.
  2. **Wrap each token on both sides.** That reached exactly the operands whose first character is
     the fragment's, and no others. A quoted operand, a redirect written with no space after it, and
     an operand joined to a long flag all put a character between the token boundary and the
     fragment, so all three stayed `Routine` — again reaching the run pointer and the settings file,
     by the shortest line there is. The drive-relative Windows spelling `C:` did the same thing and
     reached the **write** road as well, which none of the others did.
  3. **Fold every character a path segment cannot contain into a separator**, and wrap the line. That
     reached the quotes, the redirects and the long flags at once — and not the fourth family, because
     the character in the way there is an ordinary **letter**: a short option carrying its value
     attached, `-o.estigia`, `-C.estigia`, `-oskills/flow/SKILL.md`. Brace expansion was the same
     miss one character wide.
  4. **Fold the braces too, and offer every split point of a token beginning with `-`.** The braces
     could join the folded set because no fragment carries one. `-` could not: `hooks/pre-push`,
     `.estigia/stand-down.json` and cursor's derived fragment all carry one, and folding it cuts them
     in half and loses them outright. A shell gives no way to know where the option letters end and
     the value begins, so every split point is offered instead. That is what is in the code.

  5. **Normalise each token on its own and join afterwards.** Folding whitespace made the whole line
     read as one path, and the parent-segment collapse then reached **across an operand boundary**:
     `mv .estigia ..` became `mv/.estigia/../`, collapsed to `mv/`, and the surface being moved was
     gone. At the base the tokens were joined with a space, so `/../` never formed there. 66 measured,
     all of the shape *surface operand followed by an operand beginning `..`* — `mv .estigia ..`,
     `cp -r .estigia ../snapshot`, `mv ~/.claude/settings.json ..`. These are the ordinary way to move
     a config file aside, and this was the worst of the six because of that.
  6. **Read the option prefix off the folded token, and let `~` start one too.** The suffix rule
     filtered on the *raw* token beginning with `-`, so a leading quote defeated it while the fold
     still left the option letters in the way: `wget "-O.claude/settings.json"` and
     `7z x a.7z '-o.estigia/run.json'`, 38 measured. Quoting `-o<dir>` is the documented 7-Zip habit
     for the reason the rule exists — the directory may hold a space. `%~dp0` expands *with* a
     trailing separator, so `%~dp0.estigia\run.json` is the correct batch idiom and puts a digit where
     the anchoring wants one; `%` joined the folded set and `~` joined `-` as a prefix marker. The
     caret joined the set on the same round: under `cmd.exe` `^` escapes the next character, so
     `del ^.estigia\run.json` names the state directory. It was the one character of 660 a reviewer
     swept where the base's answer was right for a reason rather than by accident — `!`, `#`, `@`, `[`
     and the rest name different files, and head stopped gating those on purpose.
  7. **Ask each rung of the ladder on its own.** The suffixes were appended into one view, separated
     by the same character that separates path segments, so adjacent rungs concatenated into a path
     that was never in the command. The ladder for `~/.claude` is `.claude/claude/laude/…`, and
     `.claude/claude` is ClaudeCode's derived instruction fragment — so a recursive delete of the home
     config directory answered `Boundary` for a path nobody wrote, and so did `~/backup.claude`,
     `~/notes.agents` and `-obackup.claude`. `.agents/agents` has the same `A/A[1..]` shape and did the
     same. Introduced by the sixth, found by a reviewer, and it had already made the
     containing-directories paragraph below false about the very directories that paragraph declares
     open.
  8. **Bound the ladder to the marker's own segment, and read a patch body the same way.** Two halves,
     both found by one reviewer. Unbounded, the ladder voided the left anchoring for every token
     beginning with `~` — which is how a home path is written — because the rungs of
     `~/my.claude/agents` include `.claude/agents`. Eighteen of the thirty-one rows of the anchoring
     fixture answered the opposite when respelled with `~`, against three paragraphs here that name
     those paths as `Routine`, and every row of that fixture was spelled absolute. An option prefix and
     a shell expansion both end before the first separator, so cutting past one is not reading a
     prefix, it is deleting path segments. And the payload road — the fallback `classify_with` takes
     for Codex's `apply_patch` and OpenCode's `patch`, where the path sits in a patch body rather than
     a field — handed the whole blob to `is_control_surface` directly. That was right under a bare
     `contains` and wrong once the fragments were anchored: thirteen relative spellings, the run
     pointer and the stand-down record among them, stopped matching on that road while `Write` still
     gated the same file. Both roads read free text the same way now.
  9. **Read the ladder off the same string the matcher reads.** The view is built from
     `normalise(&fold(token))` and the ladder's entry test read `fold(token)` alone — `normalise`
     folds `:` and `\` and `fold` does not. So in `${P:-.claude/settings.json}` the `-` starts a
     segment in the string that is matched and did not in the string the ladder inspected: no rung,
     and **fourteen of the nineteen** relative surfaces answered `Routine` where the base answered
     `Boundary`. The basis, because a number here is worth nothing without one: one relative spelling
     per fragment in the **base's own** fragment set. The five that were never lost are the two file
     fragments carrying no leading dot, `hooks/pre-push` and `cli/hosts.yml`, which the anchoring
     never touched so the ladder was never needed to reach them, and `.config/opencode/opencode.json`,
     `.gemini/settings.json` and `.config/crush/crush.json`, each reached by an unanchored sibling this
     branch added. This sentence has now been wrong twice: first *thirteen*, with no basis and
     unreproducible, then *every relative surface*, which is checkable and false — it would have told a
     reader the push guard stopped being gated, and it never did. A reviewer measured both.
     `TARGET=${1:-.claude/settings.json}` then `rm -f "$TARGET"` is the ordinary script idiom for a
     defaulted path. A reviewer found it by sweeping **two**-character contexts; the 660-character
     sweep recorded above is one character wide and could not reach `:-`. The same fix reaches `:~`,
     `\-` and `\~`, which that reviewer's exhaustive two-character sweep names as the only other
     non-lookalike contexts that were open.

     **What stays open, why it is a limit rather than an oversight, and what it costs.** `${P-path}`,
     `${P:+path}` and `${P:?path}` answer `Routine`. Not *still* — they answered `Boundary` at the base
     commit, so this is a **loss against the base** on the same fourteen of nineteen relative surfaces
     counted on the same basis as above, and unlike every
     other spelling recorded in this section those three do address the file. Named that way because a
     reviewer pointed out that the earlier wording read like a gap that had always been there.
     Once `$`, `{`, `}` and `:` are folded away, `P-path` is
     indistinguishable from a file named `P-path`, and `+`/`?` sit where a segment's first character
     would. Laddering from a `-` in the *middle* of a segment would reach them and would also gate
     `x-.claude/agents` and every `foo-bar` tail like it — trading a hole for over-gating on ordinary
     names. Measured and left, rather than closed by widening.

  `7z` is why the fourth is not optional: its extract-to spelling is `-oDIR`, a space there is a
  syntax error, and it is in `WRITES_A_FILE` deliberately — so the only correct way to write "extract
  into the state directory" was the one spelling that was not gated.

  Each attempt was measured against the base by a reviewer classifying raw command strings through
  the built binary, and each time the whole suite was green over the loss — because every fixture in
  this crate spelled its commands the way the last fix had taught it to. That is the same blind spot
  **nine** times: a narrowing nothing asserts, found by a reader who spells the command differently
  than the fixture does. It is recorded at this length because the count is the finding, and the count
  is the honest summary of this work: anchoring a matcher against a shell is not one change. Each
  spelling that reaches a path is its own claim, the suite went green over every loss until somebody
  wrote the spelling down, and **five of the nine were introduced by the fix for the one before it**.

  The eighth paragraph of this section predicted a ninth and a reviewer found one in the next round,
  which is the closest thing here to a measurement of the process rather than the code. A tenth is
  likelier than not. The fixtures hold the nine spellings that were found, plus the eighteen folded
  characters and the ordinary-line direction. They do not hold *the space of spellings*, because nothing in this crate
  enumerates it. A property test over generated command lines, crossed against a second reading of the
  same path, is the shape that would — and there is none. `punctuation_does_not_hide_a_control_surface`,
  `a_drive_relative_spelling_is_measured_like_a_relative_one` and
  `a_relative_operand_is_measured_like_an_absolute_one` hold the direction now, and
  `folding_punctuation_does_not_gate_an_ordinary_line` holds the other one, because reading
  punctuation as separators makes the view more permissive and a false `Boundary` costs a live
  tracker read on every build.

  What this leaves is stated rather than measured away. A command that *mentions* a control surface
  while writing somewhere else is `Boundary`, which is the asymmetry `surface_of` already ran on and
  pays one extra tracker read. Substitutions are still invisible — a path assembled from a variable or
  a subshell names nothing any of this can match, at the base commit as much as here. So is a surface
  split across a `cd`, and so is a glob that removes a directory's contents without naming the
  directory; a reviewer measured all three at both commits, and they are the same at both.

  The option-prefix rule is generous in one direction only — it can gate a token that merely ends like
  a surface, never miss one that is a surface. That cost was priced rather than assumed: across 50
  ordinary developer command lines, including the write-heavy ones whose flags take attached paths,
  it gates **none**. `an_option_prefix_does_not_gate_an_ordinary_line` holds twelve of them. A reviewer
  measured 149 lines the same way, 30 of them deliberately punctuation-heavy, and found the folding
  costing **nothing** on any of them: folding only creates separators, and a separator only helps a
  fragment that is already in the line.

  What the *list* costs on an ordinary path is separate and is real. `mkdir -p
  test/fixtures/windsurf/memories` — a project directory that happens to be spelled exactly like a
  fragment — is `Boundary` here and was `Routine` at the base. That is not the lookalike class named
  above, which is about names that merely resemble one; it is an exact collision inside somebody's own
  tree, and it costs a tracker read per write. The same holds for a project's own `.claude/agents` or
  rules directory, which is intended. A reviewer found this one by classifying paths a developer would
  plausibly create rather than paths this crate already talks about.

  **The two halves of the wrap are not alike, and the paragraph that used to stand here got it
  backwards one commit before it was read.** It said the trailing half was not load-bearing. That was
  true when it was written and false as soon as the option-prefix rule landed, because the suffixes
  that rule offers are appended *after* the wrap: without the trailing separator the first suffix runs
  straight into the folded line and the last segment of a command stops being a whole one. **Two**
  fixtures redden without it now — `a_relative_operand_is_measured_like_an_absolute_one` and
  `a_later_operand_does_not_collapse_an_earlier_surface` — and it is the separator pushed **between
  tokens** that carries it, not a wrap at the end of the line: dropping the final separator alone, or
  the one closing each ladder rung, leaves the whole suite green. This paragraph said *eight*, which
  was measured before the ladder rungs were split out and never measured again; a reviewer counted. Nobody re-measured the sentence when the code under it moved, which
  is the failure this document exists to catch, committed inside this document.

  The **leading** half is the inert one, and it is inert structurally rather than by luck: `anchored`
  tests `starts_with` as well as `contains`, so a fragment at position 0 is reached either way, and
  dropping the wrap can only *add* matches. Measured over every fragment the gate consults, five
  spellings and five write verbs — 975 command lines, **zero answers changed**. It stays for symmetry
  and nothing holds it, which a reader should know rather than infer a guard that is not there.

  Each of the eighteen folded characters is now held individually, and that took two attempts as well.
  A reviewer mutated the set character by character and found ten of the then-fourteen could be
  dropped with the whole suite green — the set had been justified four characters at a time by
  measurement and the rest by the sentence *"the rest terminate a word the same way"*, which the
  missing brace had already disproved. The first fixture written for it **iterated the constant**,
  which reads like coverage and is none: dropping a character from the set also drops it from the
  test, and eleven mutations still survived. The characters are spelled in the fixture now and crossed
  against the constant, so all eighteen die.

  A fragment naming a **file** without a leading dot still cannot be anchored, because `cli/hosts.yml`
  exists to match **mid-segment**: the Windows path is `%APPDATA%\GitHub CLI\hosts.yml`, and
  `github cli` is one segment holding a space. The cost is that a vendored copy of somebody else's
  agent answers `Boundary`, and will keep doing so: `~/.codeium/notwindsurf/memories/global_rules.md`
  through windsurf's derived fragment, and `vendor/myopencode/agents.md`, `vendor/mygemini/gemini.md`
  and `vendor/mycrush/crush.md` through theirs. A draft of the fixture asserted the windsurf one away
  and went red, which is why it is written here rather than believed.

  The other three are here because two reviewers measured this paragraph and found it naming
  `vendor/myopencode/agents/a.md`, which answers `Routine` — `opencode/agents/` is a **directory**
  fragment and is therefore anchored, so a lookalike cannot reach it. The derived fragment that
  demonstrates the limit is `opencode/agents.md`; the written path was off by a separator, and nothing
  in the suite held it, because this file asserted the ends-alike direction and never this one. It is
  held now, on both roads — which is the point of the paragraph: a declared limit nothing measures is
  a limit that drifts from the code while the document keeps saying it.

  What still over-matches is the fragments that name a file: `.claude/settings` reaches
  `.claude/settingsmap.ts`, and `crates/crush/crush.json`, `docs/gemini/settings.json` and
  `tests/fixtures/gemini/settings.json` answer `Boundary` because two-component fragments match
  anywhere. Deliberate — `.claude/settings` is trimmed on purpose to reach `settings.local.json`.

  And a per-project cost that is new and worth naming plainly: `<repo>/.claude/agents/*`,
  `<repo>/.cursor/rules/*`, `<repo>/.cline/rules/*`, `<repo>/.continue/rules/*` and
  `<repo>/.claude/settings.local.json` are `Boundary` now. A project that keeps its own agent
  definitions or rules pays a live tracker read on every write to them. This crate ships agent
  definitions under `skill/agents/`, which is not one of those paths — and at the head this paragraph
  was written for, no file tracked in this repository tripped any fragment.

  That last clause stopped being true, and the change is deliberate rather than
  incidental: `<repo>/CLAUDE.md` and `<repo>/AGENTS.md` are `Boundary` now, which is **two** of this
  repository's own tracked files, measured by classifying every path `git ls-files` reports.
  `<repo>/.claude/plugins/**`, `<repo>/.claude/commands/**`, `<repo>/.codex/prompts/**`,
  `<repo>/.cursor/commands/**`, `<repo>/.cursor/cli-config.json`, `<repo>/.gemini/extensions/**`,
  `<repo>/.qwen/extensions/**`, `<repo>/.continue/config*`, `<repo>/.continue/assistants/**`,
  `<repo>/.codeium/windsurf/**`, `<repo>/.windsurf/rules/**` and `<repo>/.opencode/agent/**` join the
  list above in the same change and are not in it, because that list predates it. A run
  holding a claim that edits this repository's own instructions pays one tracker read for the
  privilege, which is the honest price of the rule the sentence above states.

  **Two things this change leaves open, both measured by reviewers of it.**

  Over-gating is held only for hand-spelled shapes, and it is held **unevenly**. The fixtures that
  watch the `Routine` direction list paths by name, and nothing derives an over-gating check from the
  adapter table, so whether widening a *derived* fragment is caught depends on whether some hand-
  written row happens to sit under it. Measured, one adapter at a time, each widened to its bare
  directory and the full suite re-run:

  - **Eight** redden: `.agents/agents` → `.agents/`, `.claude/claude` → `.claude/`, `.codex/agents` →
    `.codex/`, `opencode/agents.md` → `opencode/`, `.cursor/estigia-workflow-authority.md` →
    `.cursor/`, `.continue/rules/estigia.md` → `.continue/`, `.cline/rules/estigia.md` → `.cline/`
    and `windsurf/memories/global_rules.md` → `windsurf/`. Cline's was caught by
    a truncation test that was deleted with the derivation, rather than by an over-gating
    fixture — coverage arriving from the under-gating side. It is not re-measured here, and no
    claim is made that the widening is still caught.
  - **Five** are silent, each widened on its own with the full suite re-run: `.qwen/qwen.md` →
    `.qwen/`, `gemini/gemini.md` → `gemini/`, `crush/crush.md` → `crush/`, `.zcode/agents` →
    `.zcode/` and `.dsh/agents` → `.dsh/`. Nothing in this crate names a path under those five
    directories in the `Routine` direction.

    A draft of this bullet said **one**, and named crush and gemini in the bullet above
    as reddening *through* `the_declared_over_gating_is_the_shape_the_document_names`. Blind reviewers
    re-ran the widening and it is not so. A second draft then explained the silence by saying the
    widening makes those fixture rows answer `Routine`, and reviewers of *that* head measured it
    false in both halves: `/repo/vendor/mycrush/crush.md` and `/repo/vendor/mygemini/gemini.md` stay
    `Boundary` under the widening — held by the `crush.md` and `gemini.md` entries this same change
    added, which match a whole segment at any depth — and a row that *did* flip would turn that
    fixture **red**, not leave it green, because the fixture asserts `Boundary`. Measured on windsurf,
    where the row does flip: `~/.codeium/notwindsurf/memories/global_rules.md` goes `Routine` and
    **three** tests reddened —
    `a_directory_entry_does_not_gate_a_name_that_only_starts_the_same`,
    `the_declared_over_gating_is_the_shape_the_document_names` and a truncation test that was
    deleted — any one of which put windsurf in the eight; the first two remain.

    So there is no special mechanism here, and the two sentences that claimed one are gone. Crush and
    gemini are silent for the ordinary reason the bullet above states for all three: nothing in this
    crate names a path under those directories in the `Routine` direction.

    One consequence is worth keeping, because it is a coverage fact rather than an explanation: those
    two fixture rows are now **double-covered**, by the unanchored file fragment and by the
    per-project entry. Deleting `crush.md` alone leaves that fixture green — the entry is still held,
    by `a_projects_own_always_loaded_rules_are_a_boundary`, but the row whose comment describes the
    unanchored shape no longer isolates it.

    This list has now said *one*, then *four*, then two, then one, then three. The four was true
    until `the_suffix_ladder_does_not_synthesise_a_path` landed and began catching `.agents/` and
    `.codex/`, and nobody re-ran it. Every re-count but the first came from a reviewer rather than
    from this repository, which is the honest attribution and the argument for deriving this
    direction rather than counting it again.

  The under-gating direction is crossed against `resolve_paths` for every adapter, the retired six included. This one is
  crossed against nothing, and the coverage it has is incidental rather than derived.

  And the **containing** directories are `Routine` on both roads while their contents are now
  `Boundary`: `~/.claude`, `~/.codex`, `~/.cursor`, `~/.qwen`, `~/.agents`, `~/.zcode`, `~/.dsh`,
  `~/.gemini`, `~/.cline`, `~/.continue`, `~/.codeium`, `~/.codeium/windsurf`, `~/.config/crush`,
  `~/.config/gh`. `~/.claude/skills` was in that list and is not any more: the skills
  tree is derived per adapter, so `~/.claude/skills`, `~/.agents/skills`, `~/.codex/skills` and
  `~/.dsh/skills` are `Boundary` themselves. So a single recursive delete of `~/.claude` — taking
  the settings file the gate is registered in, the workflow-authority directive, the agent
  definitions and the installed contract — answers `Routine`, and being outside every checkout it is
  answered by `outside-the-claim` before the
  tracker is asked. Identical at the base commit, so not a regression; recorded because *this* change
  is what makes it matter, and re-measured because for one head it was **false** — the suffix ladder
  concatenated `.claude/claude` out of its own rungs and answered `Boundary` for `~/.claude`, so this
  paragraph declared open a gap that was not open, in the spelling it writes here and in no other. A
  reviewer measured it; a fixture holds the sentence for `~/.claude`, `~/.agents` and `~/.codex`, by
  moving files inside them to `Boundary` while the directory around them stays open. It names no
  other entry in the list above, so the two added with ZCode and DeepSeek Harness are held by
  reading. It is not the declared hard-link limit — it is an ordinary path a
  shell writes. The shape that would close it already works next door: `~/.config/opencode` and
  `~/.estigia` are `Boundary` because those entries name the directory.

  **There are two readings of the same text, and the difference is deliberate.** The write road hands
  `is_control_surface` a `file_path`, which is a path; the shell and payload roads hand
  `text_names_a_control_surface` a command line or a patch body, which is prose with a path in it.
  Only the second folds punctuation, because only the second can have punctuation *next to* a path
  that is not part of it. A reviewer measured what that leaves and then measured whether it matters:
  on the write road a decorated relative spelling — `".claude/settings.json"` with the quotes inside
  the field, or with a leading space, tab, `~`, `$`, `%`, `{`, backtick or `=` — is `Routine` at this
  head and was `Boundary` at the base. A draft of this paragraph used `.claude/CLAUDE.md` as the
  worked example, which cannot be true of it: that file was not on the control surface at the base at
  all — it is the file the population was widened for, and the fragment that reaches it arrived in this
  branch's first commit. A reviewer caught the example contradicting the sentence around it. It is not a hole, and that was checked rather than assumed: writing
  each of those spellings on this machine either fails outright or resolves to a different,
  non-existent directory. Only the bare spelling reaches the file, and the bare spelling is gated. The
  same reviewer confirmed it end to end — the quoted `file_path` costs zero tracker reads while the
  bare one, the quoted absolute, the shell road and a patch body naming it each cost exactly one.

  What is true and worth a reader's attention is that `punctuation_does_not_hide_a_control_surface`
  drives only the shell road. The write road has no punctuation fixture, because there is no
  punctuation behaviour there to hold — which is a fine reason and an easy one to forget.

  Two more `Routine` at both commits, each found by a different reviewer and neither closed here.
  `<repo>/.opencode/plugins` is not reached: `.opencode/agents/` was added for the repository-local
  definition root, and the plugin directory beside it has no entry, because `opencode/plugins/` is
  anchored and `.opencode` is not it. Estigia reads nothing from a project-local plugin directory, so
  it is not the same class as the definition root — but it is the same *shape* as the hole this change
  closed one directory over, and it is named here so the next reader does not have to find it twice.
  And `curl "-o<path>"` with the value attached **and quoted** is not seen as a write at all: the gap
  is upstream of `surface_of`, in `shell::writes_a_file`'s reading of the `curl -o` idiom, which this
  change does not touch — `src/harness/shell.rs` is byte-identical between the two commits. `wget
  "-O<path>"` is a `Boundary` in the same spelling, which is what makes the asymmetry visible.

  Still `Routine` at the head this paragraph was written for, and **since closed**: a project's
  own `AGENTS.md` or `CLAUDE.md` at its root, `.cursorrules`, `.clinerules`, `.windsurfrules`,
  `.github/copilot-instructions.md` and `.mcp.json`.
  They carry the same always-loaded authority, and they are **inside** a checkout, so they stay
  measured against the claim rather than standing aside — which is why they were a smaller thing than
  what this issue closed, not the same thing. `harness::roles::definition_for` reads its **five**
  definition roots from a hand-spelled list that nothing crosses against the gate. All five answer
  `Boundary` today and a sixth would not — but that sentence said *four*, and said it while one of
  them was `Routine` on both roads. `<repo>/.opencode/agents` is searched beside `<repo>/.claude/agents`
  in a single `vec!`, and the two were gated by different fragments: `.claude/agents/` reached the
  first, `opencode/agents/` is anchored on the left so `.opencode` is not it, and nothing reached the
  second. A definition that is not found is `Ok(None)`, which `declared_policy` reads as *every tool
  allowed*, so the file that writes an agent its own allowlist was the one riding the renewal window.
  A reviewer measured it; the fixture in this repository had gone further and asserted the hole,
  listing that root among the ordinary paths a directory entry must not gate. `.opencode/agents/` is
  a `CONTROL_SURFACE` entry now, `every_definition_root_is_a_boundary_on_both_roads` crosses all five,
  and one of them — `~/.claude/agents` — is additionally `paths.agents_root`, reached by the walk.

  What is still not crossed is the **list itself**: adding a sixth root to `definition_for` reddens
  nothing. The fixture spells the five by hand, exactly as the roots are spelled by hand.

  `gh`'s hosts file would have belonged on that list and is not on it. It decides which account
  every tracker call acts as, so it is named in `CONTROL_SURFACE` instead — which is what the issue
  asked for: *if one can reach tracker state, it is that path that needs naming rather than the whole
  class*. Both spellings, and through the shell as well as the write tool, which the first attempt
  got wrong by using a fragment with a space in it. (An earlier draft of this paragraph said the file
  "was on that list until this change", which was a claim about a previous shipped state that never
  existed — the whole entry is new. A reviewer checked it against `2477de0` and it was not there.)

  The hole predates this: the instruction files and the two `~/.claude` paths were never `Boundary`,
  so they were once measured against the claim and allowed under a valid one. What was newly lost is the refusal when the claim
  is *not* valid — which is what was closed for them, and this paragraph is kept because the
  ordering it goes on to describe is what makes any of it hold. It is no longer lost when no contract is installed: the answer is
  given after the `control-surface-not-installed` refusal, and
  `an_unreadable_control_surface_refuses_even_a_write_outside_the_claim` is what stops that drifting
  back. One published head had it the other way round and both reviewers of that head raised the
  cost, so it moved rather than staying here as a declared limit.

  It was lost again **inside the renewal window** for as long as the window's `Allow` sat above the
  contract refusal — for its duration a routine write was permitted with no `SKILL.md` on disk at
  all, on both roads, and the guard above passed only because its fixture was outside the window. An
  earlier draft of this paragraph claimed the rule applied *without an exception* while that was an
  absolute the code did not hold; a reviewer measured the exception by adding one `mark_verified()`
  line to that fixture. It was closed by moving `control-surface-not-installed` above the
  window rather than below it, which costs one `is_file()` on a path `decide` was already joining.
  Each of the two fixtures now has a twin differing only in that `mark_verified()` line —
  `an_unreadable_control_surface_permits_no_write_inside_the_renewal_window` and
  `an_unreadable_control_surface_refuses_a_write_outside_the_claim_inside_the_window` — and both go
  red with `Allow("issue #N was verified inside the renewal window")` if the order is put back.

  One stand-aside still answers above that refusal, and it is not the window. `another-checkout`
  fires on the directory the hook was invoked in rather than on the write target, so a run claimed in
  one checkout whose hook fires in an unrelated one stands aside without the contract being read.
  Unlike `nothing-sworn` and `no-tracker`, which are above the check because a run holding no issue
  and a project with no transport are outside the remit entirely, this one is reached by a run that
  *does* hold an issue with a tracker configured. Measured rather than argued:
  `a_claim_in_another_checkout_does_not_gate_this_one` uses the default fixture context — no contract
  on disk — with a sworn run, and is green both before and after that change. Keeping it above the check
  was that change's own requirement, so the honest sentence is that every answer above the contract refusal is
  a deny or a scope stand-aside and the renewal window is no longer among them; the unqualified
  absolute is not one the code holds.

  What was still lost when this entry first said so is closed, and the entry above says how. Three
  drafts of this paragraph described the state before the fix that shipped in the same commit — the
  last of them naming two `CONTROL_SURFACE` entries that the same diff had already replaced. Four
  reviewers found it, one after another, which is the measure of how easily a paragraph written
  before the code survives the code.

  And one boundary this cannot cross at all: a **hard link** outside the checkout pointing at a
  file inside it. Measured — `mklink /H <outside>/alias.rs <repo>/src/kept.rs`, classified
  outside, and writing through the alias rewrote the file in the checkout; `ln` does the same on
  POSIX. A hard link has no path to resolve to, so no amount of placing finds it.
  `is_control_surface` declares that same limit for its own matcher, and `writes_outside_the_claim`'s
  own doc comment now carries it too. A Linux `mount --bind` is the same shape and equally out of
  reach. Two more that path resolution cannot answer: the classification is taken before the write,
  so a link created in between is not seen, and a write tool that creates missing parents can make a
  directory inside the checkout on the way to a target that is honestly outside it.

  `gh`'s hosts file is named at its default location only. `GH_CONFIG_DIR` and `XDG_CONFIG_HOME` move
  it, and those spellings answer `Routine` — measured at `/home/me/xdg/gh/hosts.yml` and at a
  `gh-config` directory under a Windows profile. Narrowing the entry from the directory to the file
  also left `config.yml` `Routine`, which is deliberate: it holds the default host and protocol, not
  the credential that decides which account acts.

  Three limits of the stand-aside itself, none closed here.
  A landing that is not on a drive cannot be compared to a checkout that is, so `placed` declines it
  and the caller reads that as *inside*. That is what shuts the administrative-share hole, and it
  also means the whole feature is **off** on UNC ground: measured, a claim over `C:\...\repo` gates
  a write to `\\192.168.1.10\pikaflix\notes\scratch.md` rather than standing aside, and a run whose
  checkout is itself `\\192.168.1.10\pikaflix\repo` stands aside for nothing at all. An operator
  whose profile is redirected to a share, or whose checkout lives on one, gets the field failure
  this issue exists to end. Closing it means deciding what a share is the same *place* as, which is
  the question that made declining the honest answer in the first place.

  The second limit is not over-gating but measurement: the guard has none on a machine that does
  not serve its drives as shares. Widening its allow-list to accept UNC reddens both share fixtures
  here; on a runner without `\\localhost\C$` it would redden nothing and they would report pass
  having asserted nothing. A machine-independent companion was written, measured passing against
  that same widened allow-list, and deleted rather than kept — the guard only fires on a landing
  `canonicalize` actually produced, and producing one takes a real share. There is a second silent
  road on the newer of the two fixtures: it also passes when `mklink /D` is refused for want of
  Developer Mode or elevation. Measured by sabotaging the flag with the guard deleted — it printed
  its skip line and reported pass, while the older fixture still went red, so a machine that will
  not link loses the landing-versus-spelling measurement and not the guard's existence.

  The third is latent and could not be measured at all. The guard requires the landing to carry a
  drive prefix rather than rejecting the prefixes it knows, so a landing with no prefix component
  would be refused instead of compared as though it were relative. Nothing `canonicalize` produced
  on this machine has that shape and two reviewers went looking, so the branch is written for its
  failure direction and is held by no test.

  Separately from those three, and about the review rather than the change: **the POSIX half of
  this is measured by nobody who reads it.** The `..` handling in `placed` is where the two
  platforms genuinely disagree — Windows collapses the segment in the spelling, POSIX resolves the
  link first — and the arm that says so is a `cfg!(unix)` *expression*, so it is compiled on this
  desk and never runs here. The CI lanes are the only place it runs.

  **Narrowed later, and only for repositories that answer.** When this was written CI first
  ran when `release_ci` marked the pull request ready, which is after every review has been
  obtained; `publish_review` now starts a publication lane against the head it pushes and
  `record_computed_verdict` refuses to bank an approval for a head whose lane is red or still
  running. So on **this** repository a POSIX-only failure is found before the verdicts are spent
  rather than after. What is unchanged is everything below about the review itself — nobody reading
  the change runs the POSIX arm, nothing in `skill/SKILL.md` or `skill/references/` asks for a
  platform, and the lane answers a question no reviewer asked. The gap is smaller; the reason it
  existed is not closed.

  What that costs, measured on this change:
  `a_write_that_lands_inside_the_claim_is_gated_however_it_is_spelled` wrote through
  `<root>/decoy/../repo/src/main.rs` without creating `decoy`. Windows does not need it to exist;
  POSIX answers `ENOENT`. Seven published heads of this branch carried it, every one of them
  reviewed and none of the reviews able to run it, and it went red on both POSIX lanes the first
  time CI saw the branch — after the last verdict was recorded and the target released.

  The protocol did not lack the means, it lacked the question. Two reviewers of the fix reached the
  same failure from this same desk in minutes, by extracting `placed` into one file and running it
  under the WSL `rustc` that is already installed; one reproduced the `ENOENT` byte for byte and
  also on a symlinked root, which is the shape macOS gives every temporary directory. Nothing in
  `skill/SKILL.md` or `skill/references/` mentions a platform at all, so nothing asks for it. An
  earlier draft of this entry said the arm *cannot be compiled* here and that nothing in the
  protocol *could have* caught it. Both were measured false by the reviewers of the very head that
  claimed them — the first with a deliberate type error inside the arm, which failed the build on
  Windows.

  And the lanes measure less than "the lanes" suggests: `cargo test` is fail-fast across targets, so
  when the lib target failed on Linux and macOS the integration targets never ran there at all —
  `tests/pipe.rs`, which is where this change's end-to-end evidence lives, never executed on POSIX on
  this branch. That half was a diagnosis comment and is closed separately: the workflow
  passes `--no-fail-fast`, so a red run reports every target rather than the first.

  What is **not** true, and was written here for one commit: that those sixteen tests had never run
  on any runner. They had. A bare `cargo test` builds examples — cargo says so, and
  `cargo test --all-features --no-run` in a cold tree at this base leaves a runnable
  `fake_process.exe`. The measurement that produced the wrong claim was taken with
  `cargo test --test pipe`, a **filtered** invocation that builds no other target and appears nowhere
  in the workflow. A reviewer caught it before it shipped. The exposure is real and narrower: a
  filtered run in a cold tree, which is exactly how every mutation measurement in this document is
  taken. The skip is gone from the rig's type; `cargo clippy --all-targets` remains no help
  there, leaving a `.d` and a zero-byte `.rmeta`.

## What the gate writes, and who else reads

- **The gate named what Estigia writes, and the host reads further than that.** It is
  the layer immediately outside the one above. The class was closed for the instruction file each
  adapter's `setup` authors and for four rules directories; a reviewer of that change walked one step
  further out and found the same shape in three places it did not reach.

  **The mechanism, which is the half with teeth.** The instruction *file* is derived from the adapter
  table; the four rules *directories* were hand-spelled. So a twelfth adapter's directive was gated
  with no edit to any list and its rules directory was not — measured with an `acme` adapter whose
  instructions resolved to `~/.acme/rules/estigia.md`, whose neighbour `zz-override.md` answered
  `Routine` with the whole suite green. Three of the four were derived for a while, by truncating the fragment the adapter
  already declares, behind a `match` with **no wildcard arm**. All three have since been turned
  into literals in `CONTROL_SURFACE` and the derivation is gone, so the twelfth-adapter hole measured
  above is open again by construction; it is the cost named under the weaker basis earlier in this
  document. `.cursor/rules/` was always a literal, for a reason of the same kind — Estigia writes no
  file into it.
  `agents_root` had the same defect in the more usual spelling — `match adapter.slug` with `_ => None`,
  and a `&str` match can never be exhaustive — so it moved onto the same enum.

  **The two decisions, taken as sets.** The remaining items were enumerations rather than mechanisms,
  and the issue asked for them to be decided together rather than one path at a time. They were, on
  one stated principle: *a path is on the control surface when a host loads it without a person
  choosing it at the moment of use*. Under it, the skills tree around Estigia's own installed contract
  (derived per adapter from the same field that decides where the installer writes), plugin payloads,
  commands and prompts, the Gemini and Qwen extension roots, Continue's assistant configuration and
  Windsurf's MCP file and workflows all join — and so do the per-project twins, including a project's
  own root `CLAUDE.md`, `AGENTS.md`, `.clinerules` and `.windsurf/rules`, which the previous line
  excluded by nothing better than which home fragment happened to carry a dotted prefix.

  The commands and prompts directories are the one row where the principle is doing visible work
  rather than confirming an obvious answer. A slash command is invoked explicitly, which is the
  weaker authority the issue itself noted; its name and description are put in front of the agent
  whether or not it ever is, which is the always-loaded half. Deciding that per directory is the
  reasoning that left the class open in the first place, so the principle decides it.

  **What it costs, measured.** Every path `git ls-files` reports was classified: **two** of this
  repository's own tracked files are `Boundary` now, `CLAUDE.md` and `AGENTS.md`. A run holding a
  claim that edits this repository's instructions pays one live tracker read for it. A run holding no
  claim is `Outside` and pays nothing.

  That count is the **write** road only, and it understates the roads that read prose. Two of them
  do. `apply_patch` and `patch` put the path in a patch body, so `classify_with` reads the whole
  payload: a patch to `README.md` whose body contains *see AGENTS.md for the rules* is `Boundary`,
  and the same patch without the mention is `Routine`. And a **shell** line pays it too, whenever the
  command is already one `writes_a_file` recognises. Measured, each against its own control:

  ```
  Routine   git commit -m "update AGENTS.md"          Routine   echo see CLAUDE.md
  Routine   grep -rn agents.md src                    Routine   echo hi > /repo/notes.txt
  Boundary  echo 'see AGENTS.md' > /repo/notes.txt
  Boundary  rm -rf /tmp/scratch # see AGENTS.md
  Boundary  sed -i 's/x/y/' /repo/notes.txt # per AGENTS.md
  ```

  So the rule is not *which agent* but *whether the line writes at all*: a command that is not a write
  never reaches the matcher, and one that is pays for every control-surface name anywhere in its text,
  including a trailing comment. Two reviewers measured the writing half and one measured the
  non-writing half and read the scope as accurate; both are right about their own examples, and this
  is the statement that covers both. No count in this document covers this road.

  **What is still open, and none of it is closed by this entry.**

  - The matcher grew a third anchoring rule — a fragment holding no separator is a whole segment,
    without which `agents.md` reaches `myagents.md` — and it anchors only the **left** side. So
    `<repo>/AGENTS.md.bak` and `<repo>/CLAUDE.md.orig` are `Boundary`, as `.claude/settings` already
    reached `.claude/settingsmap.ts`. Deliberate and the same trade: the right side is where
    `settings.local.json` and `CLAUDE.local.md` live, and trimming to reach them is why those entries
    are prefixes at all.
  - `.continue/config` is trimmed to reach `config.yaml`, `config.json` and `config.ts` under one
    stem, so `~/.continue/configuration-notes.md` is `Boundary` too.
  - `<repo>/.opencode/plugins` is still `Routine`, unchanged and named in the entry above. Estigia
    reads nothing from a project-local plugin directory, which is the stated reason and not a
    measurement.
  - The over-gating direction is still not derived from the adapter table. How many adapters are
    silent when their fragment is widened is measured in the paragraph above and stated **only**
    there — this bullet used to repeat the number and the two disagreed after the count was corrected
    in one place. One fact, one place.
  - A host that reads a directory whole and is declared not to is invisible here, in exactly the way
    the four hand-spelled entries were invisible for the adapters nobody thought of. The declaration
    moved from a distant list onto the adapter; it did not become checkable.
  - **Flipping the row does not re-render the installed definition, and nothing reports the drift.**
    `config set` writes the contract table and reads it back; it never writes an agent's files, which
    is true of every row and stated as a safety property in `docs/configuration.md`. For this row it is
    also a hazard: measured, `estigia config set "Evidence standard" measuring` answers `Evidence
    standard is now measuring` with exit 0 while `~/.claude/agents/review-blind.md` still reads
    `tools: Read, Grep, Glob`, and `doctor` answers `ok canonical — and every configured agent reads
    the rows it decides by`. Only `setup` or `sync` re-renders. **One** direction of the mismatch fails
    closed at the gate, which enforces from the embedded definition rendered with the effective row
    rather than from the file: a host offering a grant the row no longer asks for is refused there.
    The other direction is not the gate’s doing at all — with the row widened and the file not
    yet re-rendered, the gate **allows**, and what refuses is the host’s stale `tools:` line, if
    that host enforces one, which the opening entry of this document says Estigia cannot prove. Either way the judge is told one thing by its own
    definition and answered another by the hook. An earlier version of this sentence said both
    directions fail closed at the gate, and contradicted itself one clause later; the repair for that
    left this fragment behind, which a reviewer then found. Nothing reports it, and `reviewer_is_static`
    accepts any body `Evidence::all()` × `RESERVED_REVIEWERS` renders by design — more than the
    reading/measuring pair alone, since seven of the eight rows are sealed and each of those collapses
    to one body, `render_reviewer_agent`'s sealed arm never reading `evidence` at all
    (`src/setup/mod.rs`). Six of the seven carry their own template; `review-blind-sealed`
    shares the judge's (`src/harness/roles.rs`). `reviewer_is_static_for_seal`, which is what
    a launch reads, cannot see the drift either.
  - **Only one of the two derived families is compiler-forced**, named here only because this row now
    decides a grant beside them. The measurement is **further down and in a different entry** — the
    roles entry's *Still open* list, under the bullet beginning *"Only one of the two derived
    families"* — and is not repeated here. Two earlier versions of this pointer were wrong in two
    different ways: the first sent a reader to *the entry above*, where the material is not, and the
    second fixed the direction and still said *this same entry*, which it is not either. Both were
    found by reviewers.
  - **Plugin-provided sub-agent definitions are the same shape as `.opencode/agent/`.**
    `~/.claude/plugins/**/agents/*.md` is a `Boundary` now and `roles::definition_for` does not search
    it, so such a definition is `Ok(None)` on read, which `declared_policy` reads as *every tool
    allowed*. Gated against being written, not read when enforcing.
  - The over-gating this change adds is declared **by shape, not by instance**: left-anchored only, a
    whole segment at any depth, and `.clinerules` / `.continue/config` as prefixes. Instances a
    reviewer measured beyond the fixture's rows — `/repo/.cursorrules-old`, `/repo/.windsurfrules.bak`,
    `/repo/.github/copilot-instructions.md.bak`, `/repo/target/doc/qwen.md`, `/repo/qwen.mdx`,
    `/repo/vendor/gemini/extensions/pack/manifest.json` — each follow one of those shapes. One shape
    is road-dependent, and the suffix ladder is its cause: `/repo/-agents.md`, `/repo/-claude.md`,
    `/repo/-qwen.md` and `/repo/--agents.md` are `Routine` to `Write` and `Boundary` to `rm`, because
    the shell road splits a token at a `-` or `~` that starts a segment and offers the rest as a rung,
    while the write road has no ladder. The cost is one tracker read and nothing is under-gated. The
    fixture holds representatives, not the family.
  - **This change narrowed the crossing's own diagnostic value**, and the paragraph on staling the
    eleven fragments carries the measurement: from eight of eleven caught to one, because the
    per-project entries also match every adapter's home instruction file. Nothing was unguarded at
    that head — `the_spelled_instruction_files_and_the_adapter_table_agree` reddened for all eleven
    of the fragments the table then held — but a reviewer relying on the crossing to catch a stale
    fragment now has one adapter's worth of signal.
  - `.opencode/agent/` — the singular spelling — is a `Boundary` now, and
    `roles::definition_for` still searches only `.opencode/agents` beside `.claude/agents`. So a
    sub-agent defined in the singular directory is gated against being written and is still
    `Ok(None)` when read, which `declared_policy` reads as *every tool allowed*. This change closed
    the gate half of a one-letter asymmetry and left the enforcement half; the sentence above about
    five definition roots is not falsified, but a sixth spelling is known now and is not searched.
  - **Other wildcards of the `agents_root` shape survive.** Adding another `InstructionFile`
    variant fails to compile in **seven** places, and **six** `_` arms on that enum in shipping
    code then hand it a default nobody chose. Read by hand with `grep`, and that is the limit of
    the claim — nothing derives this list, so it is a reading and not a measurement, and an
    earlier draft that said these were *named rather than implied* named five of the eight it then
    held:

    | where | arm | reachable by the next adapter |
    |---|---|---|
    | `setup/mod.rs` `hooks` | `_ => return None` | only behind `gate_spec` |
    | `setup/mod.rs` `plugin` | `_ => None` | yes |
    | `setup/mod.rs` `apply`'s plugin body | `_ => plugin::source(…)` | only behind `plugin` |
    | `setup/mod.rs` `model_profiles` | `_ => &[]` | yes, and advisory |
    | `setup/mod.rs` `model_catalog` | `_ => …None` | yes, and advisory |
    | `setup/render.rs` `gate_gap` | `_ => "no published reference…"` | only the next one, and the text is no longer benign |

    `mcp_config` is one of the seven the compiler now forces, so a new adapter's author is made to
    choose an MCP file. `paths_in`'s own comment beside `mcp_config` is the argument for forcing that
    choice at all — *an MCP entry written where the agent does not read it reports success and
    configures nothing; a wrong path is worse than a missing one, because it fails while claiming to
    have worked* — and until this change the **dialect** written into that file, once an author
    answered `Some`, still came from a wildcard: nothing crossed it, and no test asserted the format
    for an adapter that did not already have one.

    `gate_spec`'s `_ => None` was the eighth entry here and is **closed**: it is spelled out at
    `src/setup/render.rs` since ZCode and DeepSeek Harness joined the table, because that
    wildcard is what handed both of them *no gate* with nothing written down — and `gate_gap`'s
    wildcard beside it then told an operator that no reference had been read for ZCode, where the
    bullet on ZCode below says one was. That is what a silent default costs on this enum, measured
    rather than anticipated, and it is why the `gate_gap` row above no longer calls its text benign.

    `mcp_format`'s `_ => McpFormat::McpServers` is **closed** too, by this change: it is spelled out
    at `src/setup/render.rs`, because that wildcard is the one three reviewers found
    independently, and a wrong dialect in the right file fails the same way `mcp_config`'s own
    comment says a wrong path does — it reports success and configures nothing. Left open, it would
    have handed ZCode the same flat `mcpServers.estigia` default every plain adapter gets, where
    ZCode's own CLI reads a two-level `mcp.servers.estigia` — the shape `render_mcp` and `strip_mcp`
    now write and remove explicitly, crossed against a running ZCode CLI 0.16.5 rather than read on
    a page (`docs/honesty.md`, the ZCode adapter entry below).

    None of the six that remain is closed here.

    Two defaults of the same kind are invisible to that grep because they are not `_` arms:
    `supports_mcp` and `can_gate_tools`, both `matches!`-shaped on the same enum, so a new variant
    falls to `true` for the first and `false` for the second. Each is caught by a test rather than by
    the compiler — `every_agent_that_supports_mcp_resolves_a_config_path` and
    `the_gate_gap_and_the_gate_never_disagree` — so neither is a silent default in behaviour, only in
    the reading. The same grep also finds one `_` arm in test code (`src/setup/tests.rs`), which hands
    a default to a test and not to the harness; the six above are shipping code.

## Roles and the definitions they need

- **Estigia defined roles its own roles could not do the work of.** Six shipped definitions and not one of
  them able to run anything — eight ship now, and the two added later are measured further
  down rather than folded into this count: `review-blind` carried a fixed `tools: Read, Grep, Glob`, and the five planning
  phases carried `{{TOOLS}}`, substituted to `Read, Grep, Glob`, `Read, Grep, Glob, Write, Edit` or
  `Read, Grep, Glob, WebFetch, WebSearch` — three branches, two of them strict supersets of the third, and
  an earlier draft of this sentence named only the second. **No branch produced a shell.** In a repository whose stated evidence standard is mutation — this one's is, and
  `CLAUDE.md` item 1 makes it load-bearing: a fix starts with a test that goes red — a reviewer that
  cannot run the suite cannot check either. So every panel that mattered was launched under a generic type instead, and the
  read-only guarantee `policies/judgment-day.md` states for a judge governed **none** of them.

  Measured before this closed: three publication epochs, twelve independent judge
  launches, all generic. The role gate governed zero. The reserved-role prelaunch check governed zero.
  Every capability bound on those contexts came from prose in a launch prompt written by hand that
  turn — the failure mode already closed for the reviewer's *brief* and left open for its
  *capabilities*.

  **What closes it.** `Evidence standard` — `reading` or `measuring`, defaulting to `reading` — and
  the reviewer's grant derived from it rather than fixed. Under `measuring` the role gets a shell and
  nothing else: `Write`, `Edit`, `Agent` and `Task` stay refused, and a test walks `Evidence::all()`
  asserting exactly that shape for every standard. That withholds four tools and not the
  capability — a shell writes, and it launches — which is why the paragraph below says the
  isolation precondition is the only thing making a measuring judge safe.
  Three shipped sentences drew the opposite conclusion from the same grant and two blind judges
  caught all three independently; the surviving statement of what a shell does not buy is in
  `skill/policies/judgment-day.md`. The others state the grant and name that rule; two of them do
  restate the consequence in the safe direction, `CHANGELOG.md` saying so in the same breath, and
  a judge measured that this sentence claimed otherwise.

  Two things had to give, and both were deliberate:

  - `reviewer_is_static` meant *one* spelling and now means *one of the spellings Estigia can
    produce*, enumerated over `Evidence::all()` × `crate::harness::roles::RESERVED_REVIEWERS`
    (a later round moved the seal axis off `Seal::all()` and onto that hand-written
    const, so setup and the launch gate share one list of reserved names instead of two). The
    compiler still holds the `Evidence` axis closed the way `Evidence::all` states on itself; it no
    longer holds the `seal` axis closed at this function, because `RESERVED_REVIEWERS` is a plain
    slice and a third `Seal` variant compiles without adding a row to it. A test holds that axis
    instead — `every_seal_has_a_reserved_name` (`src/harness/roles/tests.rs`) asserts every
    `Seal::all()` value has an entry in `RESERVED_REVIEWERS` and reddens on the variant that has
    none — and a separate test holds the **acceptance**. Arriving is not being returned: the arm
    can be widened with the list left short, and `Evidence::all` says so on itself. So what keeps
    the acceptance side a proof rather than a guess is the pair — and only on the `Evidence` axis.
    The acceptance walk over `Evidence::all()` with the seal fixed unsealed is one block
    inside `the_default_rendering_is_the_bytes_already_installed` (`src/setup/tests.rs`), not a
    test of its own; two sealed bodies are pinned one at a time, by the whole test
    `a_sealed_rendering_is_static` (`src/setup/tests.rs`) for `review-blind-sealed` and
    `a_sealed_lens_rendering_is_static` (`src/harness/roles/tests.rs`) for `review-lens`.
    **No acceptance test enumerates the sealed rows**, so the four remaining sealed bodies —
    `review-risk`, `review-readability`, `review-reliability` and `review-readback` — sit where no
    acceptance assertion reaches them, and a further seal value, once given a reserved name, would
    join them. What reddens on the count alone is
    `seal_has_exactly_two_values` (`src/config/tests.rs`), which pins `Seal::all().len()`
    at two and nothing more. The
    crossing that binds the *installed* bytes —
    `the_reserved_reviewer_grant_is_the_same_through_both_doors` — walks a hand-written pair of
    rows rather than `Evidence::all()`, so a third standard would arrive at the rendering and at
    `reviewer_is_static` and reach that test only if somebody added it there. A judge measured that
    this was disclosed for the first and not for the second.
  - The gate had to read the row. It carries it in `GateContext` beside `window`, and narrows the same
    way for the same sentence: **an unreadable contract answers `reading`**, because a fault must not
    hand out a capability.

  **What it does not buy, and this is the part worth a reader's attention.** A judge that measures
  writes inside the directory its launch hands it, and **when that directory is outside the claimed
  checkout** the stand-aside answers before the tracker is asked. That condition is the
  orchestrator's to meet and nothing here enforces it: a delegated context's tool events carry the
  launching session's id, so a judge handed a directory *inside* the claimed checkout is measured
  against the parent run's live claim and **allowed**. The isolation rule is what asks for the
  separate directory, and it is a rule an orchestrator follows rather than a gate that refuses. **Giving the role a shell does not put its
  mutations under the gate.** What makes them safe is the isolation precondition, and that is why the
  isolation rule moved in the same change: it bound the checkout and said nothing about anywhere else
  a judge writes. Measured on a five-judge panel that shared one scratch directory — one judge's
  script overwritten by another's and executed inside a third judge's checkout, a fourth reading the
  implementing run's planning notes — two verdicts stopped being independent readings and one had to
  be discounted. Nothing in the delivery target was touched and no checkout rule was broken, which is
  precisely why the rule needed the second half.

  **And the workflow front door is reachable the same way.** `estigia claim` and
  `estigia transition` are CLI entrances onto the operations the tool server runs, and the role
  gate bars tool *names*, not shell commands — so under `measuring` the same shell that reaches
  the target reaches those. What holds a judge off them is the sentence *"Still do NOT change tracker state"* in its own rendered
  definition: an instruction, not a gate. The claim gate answers separately and a judge holds no
  claim, so a write against the repository still refuses; what is unguarded is the tracker call
  a judge could make in its own name. Named here rather than implied, because the last acceptance
  criterion says no delegated context gains orchestration authority, and at the grant
  that is true while at the shell it is prose.

  **And `setup` and `sync` reach further than a redirected `HOME`.** Named here because a judge
  measured it while reviewing this very change, not because it was reasoned about. Under `measuring`
  a judge ran `estigia sync --allow-source-build` with `HOME` and `USERPROFILE` pointed at its own
  scratch directory — the isolation every launch brief here asks for — and the run walked every
  adapter. The `gemini-cli` root resolves under `%APPDATA%`, which neither variable governs, so two
  files in the operator's real profile were rewritten: the skill path repointed at the sandbox, and
  the MCP command and `BeforeTool` hook repointed at the judge's own `target/debug/estigia.exe` —
  a binary inside a checkout that gets deleted. The judge disclosed it and restored both files, and
  the restoration was verified independently before this was written.

  So what a shell reaches is not only the tracker front door above but `setup` and `sync`, which
  rewrite host agent definitions — the reviewer's own `tools:` line among them — at machine-scoped
  paths no launch directory encloses. Redirecting the home is what every judge brief on this
  repository calls isolation, and for this command it was not sufficient. Fixed: `Environment::resolve` now takes an inherited `APPDATA` or `XDG_CONFIG_HOME` only when it
  resolves under the same home `HOME`/`USERPROFILE` supplied on that call, through
  `paths::covers` rather than a hand-rolled prefix check, so it case-folds on Windows the way
  `%APPDATA%` and `%USERPROFILE%` routinely differ. A value that fails that check is dropped in favour
  of the fallback the isolated home already computes, and the drop is reported as a note in the
  per-adapter output — never a refusal, so an operator running an ordinary `estigia sync` on a
  machine nothing is wrong with sees nothing new.

  **The gate does not apply this rule, and is not meant to.** `harness::roles::definition_for` reads
  `XDG_CONFIG_HOME` raw, so on a machine where the installer substitutes, installer and gate name
  different OpenCode definition roots. Nothing is loosened by that: the gate pushes
  `~/.config/opencode/agents` *and* the relocated root and enforces a definition found in either, so
  whichever one the installer chose is one it already looks in. It is written here because the
  divergence is real and nothing tests for it — a later change that made the gate prefer a single
  root would break the installer's guarantee from a file that never mentions it.

  **That report now reaches the one path where it mattered most, and what remains there is not
  silence.** A note is attached to an adapter whose own resolved paths sit under the dropped
  variable's fallback (`src/setup/mod.rs`'s `paths_in`), and a note is printed for an adapter the run
  reports on. `sync` with no adapter named reports the adapters `is_present` finds — resolving
  through the substituted root — so an install that really sits under the real `%APPDATA%` still
  stops being found and is still not operated on. That decision is out of scope here, and the silence
  is not: the substitution notes are now read independently of any target, printed beside the run for
  the ones no target already reports, and carried by the `no agent has Estigia installed` refusal
  when the substitution is why the list came back empty. `setup --all` prints them too, and the
  refusal names it, so an operator is not left without a next move. Two blind reviewers of epoch
  `314aa1f385cf80c00bae6e47bf97ad8d` reached the gap independently by reading; it is that defect's own
  third consequence — the write is silent — in the mirror direction, and has been repaired.

  What this design accepts, and it is three cells rather than two: a machine whose `%APPDATA%` is
  genuinely relocated by folder redirection; a machine whose `XDG_CONFIG_HOME` legitimately sits
  outside `HOME`; and a machine whose `HOME` is exported to some absolute path other than
  `%USERPROFILE%` while `%APPDATA%` was never moved to follow it. The check compares against the
  **resolved** home, `paths::home_dir()`, which prefers `HOME` over `USERPROFILE` with no
  `cfg(windows)` gate, not literally against `%USERPROFILE%`. All three now get the
  home-relative path instead of the one the machine actually configured.

  **What that costs them is the same thing the fourth cell costs, and it is worth saying in the same
  words.** Those roots do not only carry instruction files: `%APPDATA%` carries Gemini CLI's
  `settings.json`, which holds the MCP server command *and* the `BeforeTool` hook, and
  `XDG_CONFIG_HOME` carries Crush's `crush.json` and OpenCode's plugin. So on those three machines
  `setup` writes the tool-call gate registration where the host does not read it and reports success
  — *a wrong path is worse than a missing one, because it fails while claiming to have worked*, which
  is `paths_in`'s own standard, quoted from the comment that put it there. The operator gets the
  substitution note, and the step from that note to *my gate is now inert* is one they have to take
  themselves. That is the accepted cost stated at full width; the alternative was refusing the run,
  and why that was rejected is above. Gemini CLI and Crush, named here, have since been retired and
  `setup` no longer writes their files; the cost now stands for OpenCode, and for `uninstall` on a
  machine that still carries the other two.

  **Three is what was enumerated, not what was proved to be all, and a fourth arrived by a different
  mechanism.** `paths::covers` canonicalised both sides, falling back to the literal spelling for a
  path that does not exist, so a `HOME` reached through a symlink resolved through it while an
  `XDG_CONFIG_HOME` naming a directory nothing had created did not — a value plainly under the home
  could be read as outside it and dropped, writing OpenCode's and Crush's files where those hosts do
  not read them, and where the two spellings coincided the note named the same path on both sides of
  *points to X outside the resolved home; using X instead*. It is closed rather than only named:
  `coverage_depth` resolves each side through its nearest existing ancestor and re-appends the tail,
  so a candidate under a symlinked home is compared in one vocabulary whether or not anything has
  created it. Found by reading, by a blind reviewer of epoch `314aa1f385cf80c00bae6e47bf97ad8d`;
  since repaired.

  Two measurements, made rather than assumed, and the second is the one worth having. On the
  operator's own machine `%APPDATA%` is `%USERPROFILE%\AppData\Roaming` and `estigia` resolves its
  home to `%USERPROFILE%`, so the condition does not fire there. And **Git Bash is not an instance
  of the third cell**, which is the first example anybody reaches for and the reason it is written
  down: the shell sets `HOME=/c/Users/<user>`, but MSYS translates it for a native child, so a
  process launched from that shell reads `HOME` as `C:\Users\<user>` — the profile, identical to
  `%USERPROFILE%`, and the two agree. Measured by reading the environment a native binary actually
  receives from that shell, not the shell's own. Nothing wider than that one machine is
  measured — an absent report and a report nobody looked for read the same to this code, and there
  is no telemetry behind either. A refusal was the other option and was rejected: a process reading
  `APPDATA` cannot tell a deliberately relocated `AppData` apart from a redirected `HOME` that simply
  forgot to move it — both answer the same question, "the variable points outside the home," and
  only one of them is the operator's own machine working as configured. `docs/what-it-writes.md`
  states that choice in the prose under its per-adapter table, which is where a reader looking for
  the reason will find it; the table itself names roots and governing variables and nothing else.

  **And uninstall reads the same substitution, with no route back to the real path.** `uninstall`
  resolves through the identical `resolve_paths`, so on that same machine it checks the substituted,
  home-relative root — never the real `%APPDATA%\gemini\settings.json` an earlier `setup` wrote
  into. The `BeforeTool` hook and the MCP command entry in that real file, and the real `GEMINI.md`
  beside it, are therefore never the files it looked at: they stay live and are not removed by that
  build. The substitution note printed above the per-file rows is the only thing that says so.

  **Which sentence `describe` then prints is deliberately not narrated here.** Two rounds of this
  entry tried and both were wrong — the first about the command, the second about the shared-root
  bookkeeping that decides it, in more detail and no more correctly. What an operator needs from this
  paragraph is that the real file survives unremoved, and that claim stands on `resolve_paths` alone.
  The printed line depends on `skill_shared_with`, on `Change::Shared` reaching `changed_files`, and
  on the order `describe_all` fills `moved` — three mechanisms that must be read together, and a
  wrong account of them is worse here than none. Changing `uninstall`'s behaviour is a repair rather
  than documentation, and is out of scope for the round that found it.

  **The test-only environment lock claims writer exclusivity, not reader exclusivity, and that gap
  is not closed.** `src/test_env.rs`'s `ENVIRONMENT` mutex serialises the *writers* in that module,
  and the readers that opt in through `with_environment_read_locked` — the helper added when a
  measured flake was traced to a test reading the home twice with a redirect landing between the
  reads. It says nothing about the readers that do not opt in, and those are what this entry is
  about. Unlocked readers of `HOME`, `USERPROFILE`, `APPDATA` and `XDG_CONFIG_HOME` exist
  elsewhere in the crate; the ones a hand check found are named in the `// SAFETY:` notes themselves,
  and that enumeration is not claimed to be complete — one family, `harness::roles::definition_for`'s
  unlocked call sites in `harness::roles::tests`, asserts something the value helps decide rather
  than something invariant to it. So `set_var`'s documented precondition, that no other thread reads
  the variable concurrently, is **narrowed and not met**; Rust 2024 makes `set_var` `unsafe` for
  exactly that precondition.

  The race is real and was measured: `cargo test --lib home -- --test-threads=8` at `b1f7bc2`'s
  parent gave **three failures in fifteen runs** — a picker test comparing a `TempDir` against the
  real home because it read `home_dir()` twice, unlocked, with a concurrent redirect landing between
  the reads. That one was closed, by wrapping its read pair in `with_environment_read_locked`. Green
  tallies were taken afterwards and are deliberately not recorded here: a scheduling window is not
  closed by any number of green runs, so they would be provenance to maintain around numbers that
  establish nothing.

  What stays open is the general case: meeting `set_var`'s precondition process-wide would mean
  taking a test-only lock around every ordinary `home_dir()` call in shipping code, which is not
  worth it. The named counterexample is narrower than that — `definition_for`'s unlocked call sites
  are in `harness::roles::tests`, not in shipping code, so the cost sentence above is not what keeps
  them unlocked. Nobody has opted them in, and this entry does not claim they cannot be. Nothing in
  this crate checks for a new unlocked reader, or for a fixture that gives an existing one something
  to find; the `// SAFETY:` notes are that check, done by hand.

  **One row, one file, and a machine can hold two answers.** `Evidence standard` is
  `Scope::Everywhere`, so it is set per repository, while the artifact it decides is a single
  user-level `review-blind.md` rendered from whichever checkout last ran `setup` or `sync`. On a
  machine holding one `reading` repository and one `measuring` repository, the drift
  `docs/configuration.md` frames as lasting only until `setup` or `sync` next writes the file is
  permanent for
  whichever repository was not last written, and no re-render fixes both. The gate answers per
  repository and narrows on an unreadable contract, so this costs a capability rather than
  granting one — a measuring judge offered the reading grant, which fails closed.

  **A count corrected in the documents and left in the source comments.** This change derives the
  neutral-root adapter count and corrects the two documents that carry it: `CHANGELOG.md` and
  `docs/what-it-writes.md`, whose sentence the count guard binds. `README.md` states no *neutral-root* adapter
  count and is not among them — it counts the adapters themselves — an earlier version of this entry said it was, which is the shape
  this document exists to refuse. Source comments still say *eight*, in `src/skill/record.rs`,
  `src/cli/mod.rs` and `src/setup/mod.rs` among others; the set is the search below, not this
  list. Nothing reads them and no shipped document repeats them, so no behaviour is
  wrong; what is wrong is that a reader of the code is told a number the code disagrees with. All
  pre-existing at `fe4ff4b1` and filed as a single item, which carries the search rather than a list —
  its first version listed fourteen sites by hand where the search answers 31.

  **Still open, and named rather than implied.**

  - No derived directory family is compiler-forced. `skills_root_fragment` matches `SkillsRoot`,
    whose variants a new adapter reuses; the other, `instruction_directory_fragment`, matched
    `InstructionFile` and has since been removed.
  - The seven `_` arms on the instruction enum are untouched, and the entry above owns them.
  - **The roles a run’s delegated contexts ran as are recorded, within the run and durably.** The
    gate writes each distinct `agent_type` it sees into the run pointer, and `SessionStart` reads it
    back to **that same run** on a resume. That is an **observation** rather than a declaration —
    what fired on a tool call, not what a launch prompt claimed — which is the half worth having.

    A second, append-only history in the same store makes that observation outlive the session. The
    gate appends each distinct `agent_type` it sees to the store's `roles` table — one JSON object
    carrying `at`, `run_id`, `agent_type`, `repo_dir` and, when the run holds one, `issue` — **at the
    moment it sees the role and independent of the decision**, so a call answered `Outside` (which is
    what a delegated context’s reads produce) is recorded where `note` drops it. `estigia roles` reads
    it back, filtered by `--run-id` and `--issue`; a run that did not produce an entry can read it
    after the session that did has ended. That is the audit the fifth acceptance criterion asked for
    and the pointer alone could not give.

    **A launch that named a model or an effort records them on that same entry.** The
    gate reads the `model` and `effort` Claude Code's `Agent`/`Task` input carried and appends them,
    when present, to the role entry it writes for the launch target, so the store's `roles` table —
    and the line `estigia roles` prints — carries what the launcher declared. **It is a declaration,
    not a check**, the same standing as the retired verdict's `self_attested`: Estigia reads what the
    launch named and cannot observe the model the host actually ran or the effort it actually
    applied. A field is absent when the launcher declared nothing, never present and blank, and a
    launch that declared nothing writes no entry of its own — the role is then recorded when its
    context's own call is seen, exactly as before. The declaration is read from Claude Code's launch
    payload, so a host that sends no such payload records none.

    **Why a second table and not the decision history.** `decisions` is a history of adjudications
    and a role observation is not one; widening it would change what every reader of that history
    understands. Its writer, `note`, also returns early on `Decision::Outside` and records the
    *adapter* slug rather than `agent_type`. So each table answers one question, and the reason is
    stated on `session::record_role` as well as here.

    Two judges drove the gap through the built binary before the durable log existed, and it is
    stated here in their terms rather than softened:

    ```
    same session    -> "Delegated contexts this run has already gated ran as: `review-blind`"
    a different run -> no such line at all
    session-end     -> the pointer file is gone
    ```

    The first line is still the pointer’s reach; the second and third are what the durable log
    changes. `run_id` is derived from `session_id`, so the pointer is per-session and `SessionEnd`
    still removes it — the log is not, and is not removed by it.

    **What the durable record proves, and what it does not close.** It proves that the gate observed
    a named `agent_type` on a tool call by a run, and that the observation survives the run. It does
    **not** close any of the panel limits this document already refuses to claim. It is **not a
    count**: five contexts under one role and one under it write the same bytes, so it says nothing
    about panel size. It is **not an isolation proof**: it does not show that any two contexts were
    given separate directories, nor that they ran concurrently or independently. It does **not prove
    a panel ran**: a host that never sends `agent_type` leaves it empty exactly as the pointer does,
    so an empty log is not evidence that nothing was delegated.

    This entry claimed the opposite for one publication: *"`SessionStart` reads them back to the next
    run"*. It was corrected to say the record lived only as long as the run, and that correction is
    now itself narrowed: the pointer half still dies with the run, and the durable half does not.
    Both sentences are recorded here rather than quietly, because a false sentence in the register of
    what is not measured is the one kind this document cannot carry.

    Five narrower limits on the observation. The first four apply to the durable log too; the fifth
    is the pointer’s alone. The first three are ways the set can be empty
    while contexts ran, the fourth is a way it can be non-empty and still not answer what was
    asked, and the fifth is a write that did not happen before. A host that does not send `agent_type` leaves it empty, and Estigia cannot tell that apart
    from nothing having been delegated. A context refused at the role gate before any call is not
    recorded, which is deliberate — a refused launch contributes no judge — but it means the record carries
    nothing for a launch the role gate refused. A call the *claim* gate denied is recorded, since
    the store fires on a new role or an allow — so this is one gate's refusals, not every gate's.

    **And under the default row the reserved reviewer is never recorded at all**, which is the case
    that governs every installation nobody has changed. Its whole grant is `Read, Grep, Glob`, and
    none of those three is in the matcher the hook is woken for —
    `Edit|Write|MultiEdit|NotebookEdit|Update|Bash|Agent|Task|AskUserQuestion` — so no call it is permitted to make
    ever reaches the record, and every call that would reach it is outside the grant and refused
    before it. A judge measured both directions through the built binary: under `reading`, a
    `review-blind` context calling `Bash` is denied and no pointer is written at all; under
    `measuring`, the same call records `roles: ["review-blind"]`. So the criterion asking which role
    a delegated context ran as is delivered for a measuring judge and for a delegated role that
    writes, and is **inert for the reserved reviewer on a default install** — the role this change is
    named after. Recorded rather than fixed: making it non-empty would mean waking the hook for
    tools no gate has anything to say about. And the field names a role, never a *count*:
    five contexts under one role and one context under it write the same byte, so nothing here bears
    on panel size, which the entry below already states this crate cannot see. And the store now
    fires on a **new role** as well as on an allow, so a call the claim gate denied writes a
    pointer where only an allowed one did — including under the `<runtime>-unknown` name a blank
    `session_id` produces, which `SessionEnd` deliberately does not remove. Such a pointer holds
    no issue, so it cannot loosen the gate, and the allow path could already create it. The wider
    half is a real run pointer: `gate` takes `&mut run` and calls `mark_verified` once the tracker
    answers, before the refusals that can follow it, so a **denied** call carrying a role not seen
    before now persists that watermark where it used to be discarded. What is persisted is true —
    the tracker did answer — and the next routine write can ride the window without asking again.
    Recorded because it is a change to the gate's own state that no other sentence names.

    One more, found by a judge rather than by design, and it is the pointer’s alone: the role write
    goes through `session::store`,
    which drops a stale write whole when another writer moved the revision first. `session::update`
    exists for exactly the case of a writer carrying information only it has, and this is not using
    it — so a role can be lost to a race with a tool write. Nothing another writer set is lost, which
    is the fail-closed direction. The durable log is append-only and has no such race, so a role the
    pointer loses this way is still recorded there.

  - **A second field is added beside `roles`, from the same signal and inheriting the same
    limits, and it is still only a record.** `Run.inline_writes` (`src/harness/session.rs`) gains a
    target whenever `agent_type` is absent on a call the gate allowed, once `roles` already held an
    entry this session — the same absence `session_start_response` reads as this run's own context
    rather than an unreported one. The element is what `Action::subject` names, so it is a path for
    a write tool and the matched fragment, the verb or the tool for a shell write, and the surfaces
    that name it say **target**, never file. Absent is a missing field and not a present-but-blank
    one, because a
    blank `agent_type` from inside a sub-agent would otherwise flip the silence into a claim that
    this run's own context wrote a delegate's write. Everything named above for the within-run half
    of `roles` applies to it unchanged: within-run only, gone at `SessionEnd`, empty on a host that
    never sends `agent_type`, and silent for a call the role gate refused before it ran.

    **One limit is its own: a run that never delegates reports nothing, not a floor.** `roles` starts
    empty, so the earliest inline write of any session — including a session that never delegates at
    all — is indistinguishable, at that moment, from a host that does not report `agent_type`. It is
    recorded as neither this run's own nor a delegate's, and stays that way for the whole session if
    no delegated call ever follows. Counting it would mean deciding, before a session ends, that a
    context which has not yet delegated never will — a claim about what a run *will* do that nothing
    else in this crate makes. So a session that delegates at least once gets a floor on the writes
    after its first delegated call; a session that never delegates gets silence, which reads
    identically to a session with nothing to report. The issue's own motivating case still lands,
    because the run it measured had already delegated before it wrote inline — but a run that skips
    delegation entirely, the harder case the rule exists for, is exactly the one this record cannot
    see.

    **A write outside the claimed checkout is not counted.** The record takes only a call the gate
    answered `Allow`, and a routine write to an absolute path no covered checkout reaches (a scratchpad
    file such as a PR body) is answered `Outside` at every age of the claim's verification, inside the
    renewal window as well as after it. It used to be `Allow` inside the window and so was recorded
    there only. A write that cannot be placed, and every relative path, is still read as inside.

    **A gate cannot judge "non-trivial", and this adds none.** Nothing here refuses an inline write,
    whatever `SKILL.md`'s Direct Work Delegation table says crossed it — the topology entry
    below measured that a mechanical proxy (changed-file count) over-fired on this repository's own
    history, and the refusal built on it is gone; a second gate scoring the same judgement would fare no
    better. `SessionStart` names
    the targets; nothing stops the next one from writing more.

    **And it says nothing about which model wrote them.** The record is which *context* wrote and
    what its definition declared — never which model actually answered, which Estigia renders into a
    definition and cannot observe running. A claim narrower than that is a different vocabulary, not
    this field's.

  - **A write to `.git/estigia/estigia.local.md` on the write-tool road is a boundary whichever row
    it names, and a shell write to it gets the same answer a shell write to any other
    control-surface file gets — not answered from the renewal window, and not, after six cuts spent
    trying, answered by reading the write's own payload either. `REPOSITORY_SHELL` used to answer
    before either question and shadow this file and the control surface alike; that shadow is closed — the list at the end of this entry names how.** `Evidence standard` is
    `Scope::Everywhere`, so it can be set per repository, and a repository row lives in
    `.git/estigia/estigia.local.md` — a path `CONTROL_SURFACE` does not carry and
    `is_control_surface` does not derive. This used to be measured by
    asking the gate with a live claim held: that file answered *"verified inside the renewal
    window"*, exactly as `src/lib.rs` does, whichever row a write named — so a run holding this
    checkout could widen the grant its own judges receive on an answer that had nothing to do with
    the write itself.

    **The first cut** scanned `input.to_string()` — the tool call's whole JSON payload,
    lowercased — for each setting's label as a byte-exact substring, and answered `Routine`
    only when the payload named at least one label and none of them guarded. Measured through the
    built binary before it shipped: an `Edit` reading `old_string: "| github |"`,
    `new_string: "| github evil/repo |"` changes `Tracker`'s claim adjudication without ever
    writing the word `Tracker` in either string, and with `Model routing` sitting on the sibling line
    the payload still named a label — just not the one it changed — and answered `Routine`.
    Rejected before it shipped.

    **The second cut** required every `|`-carrying fragment of the payload to itself name a
    label, splitting on a real newline and on the literal two-character `\n` escape rather than
    trusting the payload whole. Two blind judges rejected it independently, reproducing four
    findings through the built binary with a live claim held between them:

    1. The split was vacuous. `input.to_string()` is compact JSON, which can never contain a raw
       newline, so an `old_string` carrying two real rows serialised back to one line and the
       "every fragment names a label" rule collapsed back into "the payload mentions a label
       somewhere" — the exact defect it replaced.
    2. The gate and the parser spelled a label differently. A byte-exact lowercase substring
       match does not know what `Setting::from_label`'s own `normalize_label` already does —
       collapse internal whitespace and strip backticks — so `Evidence  standard` (one extra
       space) parsed for `config set` and was invisible to the gate's scan.
    3. The early return shadowed the control surface. A shell branch answered the
       repository-configuration question and `return`ed before `surface_of` was asked, so one
       line naming both files answered by the narrower question: `rm ~/.claude/settings.json`
       downgraded from `Boundary` to `Routine` the moment a repository-configuration mention was
       appended to the same command.
    4. The path matcher used `ends_with` where every other fragment in this module uses an
       anchored `contains`. `text_names_path`'s view appends a separator after **every** token,
       so the fragment was only ever seen when it was the command's **last** token — a trailing
       `2>/dev/null` or a second command hid it completely.

    **The third cut** asked the real parser instead of a second, weaker implementation of the
    same question. `Setting::named_in_row` split a line's cells and resolved cell 0 through
    `Setting::from_label` — the same normalising and aliasing every other reader of the table
    goes through. `harness::payload_text_fields` read the write tool's own text fields —
    `content`, `old_string`, `new_string`, and whichever ones the eleven gated tools use, walked
    recursively — instead of the tool call's serialisation. `crate::config::document_lines` split
    them by the parser's own rule. The control surface was asked first and unconditionally on
    both roads, so a path also on `CONTROL_SURFACE` could not be shadowed. `is_anchored_fragment`
    replaced `ends_with` with the anchored `contains` rule every other fragment in this module
    already used, over a fragment widened to recognise a submodule's and a `--separate-git-dir`
    checkout's shape too. All four findings above were closed, and both judges confirmed it on
    the parent ledger.

    It still rejected, by the same two judges independently, on one new finding: accountability
    was asked **once across the whole payload** rather than per field. A single `names_a_row`
    flag was set the moment *any* field anywhere resolved an unguarded row, and a pipe-less
    fragment was skipped rather than counted against it — so one resolvable unguarded row
    anywhere licensed every other field in the call, and a pipe-less fragment is exactly what an
    `Edit` or a `MultiEdit` uses to rewrite the *inside* of a value cell, where every guarded
    capability actually lives. Reproduced three ways through the built binary with a live claim
    held: an `Edit` reading `old_string: "measuring"`, `new_string: "reading\n| Repair ceiling |
    none |"` lowered `Evidence standard` with no `|` beside the word `Evidence` in either string;
    the same shape through `MultiEdit`, one entry making that exact change and a second,
    unrelated entry editing `Repair ceiling` beside it, bought the run's own judges a shell the
    same way; and a `MultiEdit` repointing `Tracker` — `github` to `github attacker/repo`, again
    with no `|` in either string — beside the same decoy config edit moved the surface a claim is
    adjudicated on. `MultiEdit` is in `WRITE_TOOLS`, is named in `payload_text_fields`'s own doc
    comment, and had no test anywhere in `src/harness/tests.rs` — both judges grepped and found
    the same absence.

    **The fourth cut** asked the question per field rather than per payload:
    `repository_configuration_sensitivity` required every text field `payload_text_fields` finds
    to itself contribute at least one `|`-carrying line that resolves to an unguarded row, with
    no line anywhere resolving to a guarded one — a field that ended without one resolvable line
    was exactly as unaccountable as a line that read as a row and resolved to nothing, and
    neither licensed another field in the same call. A field that was present and empty was not
    exempted either: it produced no lines, so it produced no contribution. Coverage for
    `MultiEdit` — the tool the third cut's absence was named against — was added at the
    classifier and reproduced all three built-binary measurements above. All confirmed fixed.

    It still rejected, by the same two judges reaching the **same** finding independently: the
    rule decided on rows a payload *named*, and never on rows a write *removed*. Removal is the
    loosening direction — a comment in `cli::gate_context` already states the cost in its own
    words, for the case of a broken contract: *"the operator's one-way doors stop being one-way
    doors, and a window they narrowed goes back to its widest."* Reproduced two ways through the
    built binary with a live claim held: `Write` with `content: "| Repair ceiling | 2 |"`
    answered `Routine`, because `Write` replaces the file whole and every guarded row it does not
    carry is simply gone — `Tracker`, `Integration mode`, `Renewal window`, `Evidence standard` and
    `Irreversible commands` all revert to the layer beneath, with nothing in the payload naming
    that they had been removed at all; and `Edit` reading `old_string: "| Repair ceiling | 2 |
    3 |"`, `new_string: "<!-- estigia:config:start -->\n| Repair ceiling | 2 | 3 |"`
    needed no truncation — the injected marker becomes the block start for `fence::locate`, so
    `config::rows` reads a region beginning at the Repair ceiling row, and the marker line itself carries
    no `|` and rode uncounted past the accountability check the same way a pipe-less line always
    had. Two things made this sharper than an edge case: the fourth cut's own test asserted the
    first payload's answer as correct — `a_repository_configuration_write_is_classified_by_row`
    read `write("| Repair ceiling | 2 |\n") == Routine` — and `Write` with `content: ""` was
    already a `Boundary`, so an empty replacement was caught while a one-row replacement erasing
    the same guarded rows was not.

    **The fifth cut** made two changes, and the first is structural rather than a reading of the
    payload at all. `ANCHORED_REPOSITORY_CONFIGURATION_WRITES` names the write tools whose write
    can only **remove** text it names — `Edit`, `MultiEdit`, Gemini CLI's `replace` — and every
    other write tool in `WRITE_TOOLS`, `Write` foremost among them, is a `Boundary` unconditionally
    for this path before the per-row question is asked at all: a replacing write erases whatever
    it does not carry, and no reading of its payload can say which guarded rows those were.
    Per-row classification now applies only to the anchored tools, which is sound because such a
    tool can only remove text it names, and naming a guarded row's text to remove it is already a
    boundary through the ordinary question. Second, the pipe-less `continue` is gone:
    `repository_configuration_sensitivity` now holds **every non-empty line** to account, not only
    ones containing `|` — a fence marker, a comment, a stray heading is unaccountable exactly the
    way a row-shaped line that resolves to nothing already was, because the `|` test was always a
    proxy for *is this line a row* and `Setting::named_in_row` is the real answer. Only a
    genuinely empty line, or one that is only whitespace, is skipped without being held to
    account.

    Measured directly, both ways, through the built binary with a live claim held: a `Write`
    naming `Tracker` answers `allow` with the tracker's own reason naming the issue held — a
    fresh read, not the `verified inside the renewal window` a routine write gets — and the
    identical write naming only `Repair ceiling` still answers `verified inside the renewal
    window` the way it always did. `Repair ceiling` stayed `Routine` on purpose under this cut,
    since it reaches no gate decision — the seventh cut later gave that concession up; see
    "What ships now" below.

    It shipped, and it was rejected in turn — it was reopened against the running fifth cut on
    exactly the question `named_in_row` never asks. That function answers for a line's **label**,
    cell zero of a split on `|`, and never reads any cell after it — but the document this file's
    own block is read out of is not parsed cell by cell at all: `fence::locate` finds the managed
    block by a raw substring search over the whole text, with no line anchoring whatsoever. A
    marker appended **inside** an already-accountable row's own line, past the value cell,
    resolves that row exactly as it always did and still becomes the block's start or end the
    moment the file is actually read. The rows above the marker fell outside the region
    `config::rows` then read. Measured through the built binary at
    head `5b90a23`, appending the opening marker after an already-accountable three-cell row, the
    closing marker after a two-cell row, and both together after a two-cell row: all three
    answered `Routine`, the label still resolving and nothing asking whether the line carried
    anything past it. A fourth spelling — the opening marker alone, appended after a two-cell row —
    was reported answering `boundary` through the same command; reproduced directly against
    `classify` in this checkout it did not: all four spellings, including that one, answered
    `Routine` before this cut, identically to one another. The discrepancy in how that one spelling
    was originally reported is not explained here and does not change what needed closing — every
    one of the four is `Boundary` now regardless.

    **The sixth cut, closing it.** A line is asked whether it carries any spelling of
    `CONFIG_FENCE`'s own markers — `begin`, `end`, and every superseded pair — **before**
    `named_in_row` is asked at all, and on the whole line rather than on the one cell that parser
    reads. Reused rather than respelled: `Setting::parse_into` already refuses a `config set`
    value carrying one of these same markers, for the identical reason — it "closed the block in
    the middle of its own table" — so `config set` and the gate now read the same enumeration
    instead of two lists that agree only until one of them is renamed. A line carrying a marker
    anywhere in it is unaccountable exactly the way an unresolvable line already was, whether or
    not its label would otherwise have resolved to an unguarded row.

    **It shipped, and it was rejected too — the sixth and last rejection of a per-row reading of
    this file, and the round this file opens with.** Two judges, working blind of each other,
    found two different escapes in the same round, in the same bytes, against the running sixth
    cut:

    - Deleting the **line break** between two rows. `old_string: "| Change size | 800 | 800 |\n"`,
      `new_string: "| Change size | 800 | 800 |"` removes nothing the checker can name — read
      alone, both strings are one ordinary, unguarded row, `Change size`, and neither carries a
      guarded label or a marker spelling. Applied to the real document, though, deleting that one
      newline merges this row's line with whatever followed it into one line, and
      `crate::config::rows` — which turns a region into rows by splitting on exactly the
      characters this checker was reading lines with — sees a single merged row where the row that
      used to sit below it has vanished into a trailing cell. Measured against `classify` in this
      checkout, before this fix: `Routine`.
    - Assembling a fence marker **at the junction** between a `new_string` and the preserved
      tail — the part of the original document `old_string` never consumed, and that a per-field
      reader has no way to see at all because it was never part of the payload. `old_string`
      and `new_string` both end mid-marker, on a line that otherwise reads as an ordinary
      `Repair ceiling` row, so neither field contains a complete marker spelling and both resolve
      cleanly; the same class was also produced with two separate `MultiEdit` entries, each on its
      own an unremarkable `Repair ceiling` row carrying half a marker in its own trailing cell.
      Measured against `classify` in this checkout, before this fix: `Routine`, in both spellings.

    Both are the same class, and it is not a payload-reading defect this fix could patch a
    seventh time: `crate::config::rows` reads a **region**, and an anchored write can move that
    region's boundary — a cell boundary, a line break, a marker assembled across two edits — using
    text that never names a single row. A per-payload reading cannot have a property of the
    *document* that results from applying that payload; it can only ever have a property of the
    payload. The fifth cut's own justification for splitting the write tools — *"none of the three
    anchored tools can remove a row without naming the text that named it"* — is false, and both
    escapes above falsify it in one working payload apiece.

    **What ships now.** Every write to `.git/estigia/estigia.local.md` on the write-tool road is a
    `Boundary`, whichever row it names — the per-row question `classify_with` asked through six cuts
    is not asked at all any more on either road; `is_repository_configuration` answering true is the
    whole test. A shell write to this file was never offered a per-row reading either — a shell line
    has no named field a row can be read out of, only text that was typed — so `printf … >>
    .git/estigia/estigia.local.md` already got the same answer a shell write to any other
    control-surface file gets. The two roads that used to disagree on that construct — an anchored
    write could ride the window for `Repair ceiling`, a recognised shell write never could — now
    agree wherever the line reaches the file's own question, because neither reads a row any more.

    **The neutralising verbs read it too now.** `chmod`, `chattr` and `chown` asked `surface_of` by
    itself — the control-surface question, which this file does not answer, since it is deliberately
    not on that list — so `chmod 000 .git/estigia/estigia.local.md` fell through to `Untouched` while
    the same line on a control surface was a `Boundary`. They now ask `names_a_boundary`, the same
    control-surface-or-repository-configuration question every other shell arm asks, so the parity
    holds for a neutralising line as it does for a write.

    **What that parity used to not say.** `REPOSITORY_SHELL` used to answer before either
    question, so a line it recognised was decided without this file's question ever being asked — and
    `shell::writes_a_file` was no help in spotting that, because it recognised the redirect in
    `git commit -m wip && printf x >> .git/estigia/estigia.local.md` too, and that line still answered
    `Routine`. The parity was exact and that was the point: this file sat under the same shadow every
    control-surface file already sat under. The list below names it, and how it closed.

    The cost is real, and stated here rather than argued away: `Repair ceiling` — retired as a
    setting, and kept in these tests only as the string of a row the gate never reads —
    a row
    nothing in the gate reads (`Setting::decides_what_the_gate_enforces` answers `false` for it,
    unchanged by this fix), now costs one tracker read like every other row in the file, on every
    write. Acceptance criterion 3 — an ordinary `Repair ceiling` write does not become a boundary —
    is not met as a strict reading of it would demand, and this file says so rather than
    qualifying the criterion until it appears to be. `estigia config set --repo` writes this same
    file directly, without going through a gated tool, and this fix does not touch it either way.

    Left exactly where it was, and not narrowed by gating the whole file: whether a row may be
    repository-scoped at all is a separate decision — `docs/configuration.md`'s *Which root the
    gate decides in* section is where that question is asked and answered today, row by row. The
    drift between what `config set --repo` accepts on a row and what `setup` writes for the same
    row also stays open where it already was. A directory-shaped removal is not named by either
    surface: `classify_with("Bash", …)` answers `Routine` for both `rm -rf .git/estigia` and
    `mv .git/estigia .git/estigia.bak`, measured directly against the classifier in this checkout.
    Neither line spells `estigia/{LOCAL_FILE}`, so `text_names_the_repository_configuration` never
    matches, and `CONTROL_SURFACE` carries no fragment reaching the bare `.git/estigia` directory
    either — `.estigia/` is a different path. This fix does not close it; it is named here rather
    than folded in, since removing a directory is a different question from which row a write
    inside the file names. The fix below does not touch it either: no `REPOSITORY_SHELL`
    fragment is a substring of either line, so that arm never runs for either, with or without the
    fix — measured the same way, still `Routine` after it.

    `REPOSITORY_SHELL` also used to reach further than the directory removal above: `classify_with`
    matched it **before** `NEUTRALISES_A_FILE`, before `shell::writes_a_file`, and before either the
    control-surface or the repository-configuration question was ever asked, so a shell line naming
    one of its fragments answered `Routine` unless the same line also matched one of the boundary
    lists asked *before* it — `DISARMS_THE_HARNESS`, `mutates_through_the_api`, or `IRREVERSIBLE` and
    the operator's declared boundaries. This was tracked separately rather than folded into this
    one, and it is now closed by that work, not by this fix: `REPOSITORY_SHELL`'s own arm stays
    exactly where it was in the match order — still before `NEUTRALISES_A_FILE` and before
    `shell::writes_a_file` — but it now asks the same control-surface and repository-configuration
    question `shell::writes_a_file`'s arm already asked, before it answers rather than instead of answering.

    `git commit -m wip && git push` is unaffected by that fix, measured against the
    classifier at both ends: `Action::Boundary { command: "git push", .. }`, because `git push`
    is on `IRREVERSIBLE`, and `IRREVERSIBLE` is still asked before `REPOSITORY_SHELL` either way. What
    the precedence used to shadow, measured the same way: `git commit -m wip && printf x >>
    .git/estigia/estigia.local.md` answered a `Routine` sensitivity before that fix and a `Boundary` one
    now. `git commit` is still what `REPOSITORY_SHELL` matches and the whole line is still decided in
    that arm — the arm now reads `.git/estigia/estigia.local.md` beside it before it answers, rather
    than returning before either question is asked.

    The cost is not a small constant, and it is not carried by every command — only by the ones on
    `REPOSITORY_SHELL`, which are `git` spellings and `gh issue develop`. Measured
    in-process, `classify_with` called in a loop, debug build, against builds at both ends: `git stash`
    ~4.6µs a call before that fix and ~29µs after — the new call to `surface_of` and
    `text_names_the_repository_configuration`, paid even when neither matches. A longer line paid
    more, because `text_names_path`'s suffix ladder re-runs `normalise` and the whole fragment list
    once per rung, one rung per byte offset of every `-`- or `~`-prefixed word: a realistic long
    `git commit -m "…"` message carrying a handful of such words cost ~39µs before that fix and
    ~470µs after, over 5,000 iterations each way. Both numbers are small next to end-to-end gate
    latency: `estigia gate Bash --input '{"command":"git stash"}'`, timed end to end rather than in
    process, same debug build and machine, costs ~75ms wall clock per invocation — so that fix ships without a
    bound on the ladder — but the ladder's own cost was never bounded either, and a line built to
    carry many more `-`/`~`-prefixed words than an ordinary commit message will was not measured here.

    **The cost is not new to the crate, and one measurement settles that.** On a different line from
    the one above — 832 bytes, punctuation-heavy, 2,000 iterations rather than 5,000, so the figures
    here are not comparable to that paragraph's and are not meant to be — debug, both ends built: `git commit -m "…"` cost
    117µs before that fix and 1.70ms after — but an `rm` carrying the same operand already cost
    **1.70ms**, unchanged by that fix, because `rm` reaches the write-reader arm and has always paid
    `text_names_path` in full. An `echo` of the same length, which reaches no path reader at all,
    cost 246µs. So that fix does not add a cost class: it removes `git` lines' exemption from the
    one the crate already had, and the exemption was the same early return that let a control
    surface travel on a repository-shell answer. A line built to burn the ladder could already be
    spelled `rm`.

    **The population that newly pays is wider than a write, and no other document says so.** The
    predicate is asked of the whole command line, with no write-shaped precondition — the arm it
    copies from reaches its question only after `shell::writes_a_file` has identified a write
    target. So a `REPOSITORY_SHELL` command that merely *names* a control surface, writing nothing,
    now costs a live tracker read. Measured against binaries built at both ends:
    `git commit -m note about .claude/settings.json` answered from the renewal window before that
    fix and answers `is held by …` after it. **`CONTROL_SURFACE` carries twelve entries with no
    separator, seven of them instruction files — `claude.md`, `claude.local.md`, `agents.md`,
    `agents.local.md`, `gemini.md`, `qwen.md`, `crush.md` — so `git commit -m update CLAUDE.md` is in
    that population too**, and this repository's own commit messages name those files routinely.
    Measured: it answers `is held by …` at this head.

    Two things bound it. The direction is fail-closed — a `Boundary` is stricter than a `Routine`,
    so nothing that was refused is now allowed — and it is the same false-positive asymmetry
    `WRITES_A_FILE_POPULATION` already declares for its own list. What is **not** bounded is the
    count: nobody has measured how many ordinary commits in a working day name one of those seven
    filenames, and the cost of each is a round trip to GitHub rather than the µs above. Both blind
    judges found this independently and neither called it severe.

  - Estigia still cannot prove a Judgment Day ran, how many contexts it had, or that any two of
    them were given separate directories. This narrows what a judge **can** do and proves nothing
    about what a judge **did** — the same boundary `policies/judgment-day.md` draws for everything
    else about Judgment Day.
  - **A reviewer's directory has no remover, and Estigia is not one.** The isolation rule asks every
    judge for a directory of its own and each Judgment Day round mints one per judge — one clone, one build
    cache — but nothing removes them. Estigia does not spawn the judge, and `release` puts down only
    the checkout `start_branch` created for the delivering run's own branch; it never reaches a
    judge's directory. The orchestrating run is the only context that creates one and the only one
    that could remove it, and no step of the delivery path requires it to. They therefore accumulate
    until an operator sweeps them by hand, and the sweep is the operator's: remove a directory only
    after the judge that wrote it has returned its findings, and only once the same
    check `release` applies to its own checkout holds — `git status --porcelain` is empty and every
    commit it holds is on the remote. A directory that fails that check is left alone and named.
    Where they live is unsettled: they are placed under the configured `Worktree location` today,
    which is what makes them look like registered worktrees when Git considers them no such thing,
    and a separate root is an open question rather than a decision. `estigia prune` now
    lists a detached checkout no branch owns, so the sweep can find one, but it cannot tell a
    judge's directory from any other orphan: a live judge's panel is listed like the rest and removed
    only when the operator confirms, which is why the listing is a name to check rather than a
    disposal.
  - **What a delivery's remote deletion cannot see.** `release` deletes every remote head the run
    recorded starting, read back through `ls-remote`, but the record is only what `start_branch`
    wrote: a branch a run pushed by hand, or pushed under an identity the issue's timeline never
    named, is not in the set. GitHub's `delete_branch_on_merge` covers the pull request's own head
    and nothing else, so the two together still leave a hand-pushed head behind. `estigia prune` is
    what catches it later, and only when the operator confirms.
  - **What the prune sweep cannot see.** It attributes a head or checkout to an issue by the
    `issue-<n>-…` convention in a branch name or by a run pointer, so a head that names no issue
    and carries no pointer is listed only when it is merged into the base, while a checkout is always
    listed, kept when nothing says it is finished. The additional remote-gone orphan route now
    recognises a clean checkout whose commits remain on any refreshed remote ref, even when the
    branch is neither merged nor closed. The behavioral regression
    `a_remote_gone_checkout_reachable_from_another_remote_is_removed_only_on_confirmation` in
    `src/harness/prune/tests.rs` failed on the delivered base and failed again when only the new
    eligibility result was disabled; the corrected prune module passed 18 tests. Its fixture deletes
    the branch from a second checkout, leaving a stale tracking ref to be pruned, and retains the
    commits on an archive branch rather than on main. The retained-state fixture covers dirty,
    unique, pointer-held, session-held, remote-present and unreadable cases. See
    [the route's rules](what-it-writes.md#explicit-orphan-pruning).

    The legacy route's "a live claim
    holds it" answer is a readable run pointer whose issue the tracker still reports open, not a
    re-read of the claim itself: a pointer a run left behind reads as live, and a claim the pointer
    never recorded reads as unheld. A false "held" keeps a branch; a false "unheld" is not safe just
    because the operator confirms deletion. In particular, `held_worktree` skips unreadable pointers
    and stops protecting a readable pointer when its issue reads closed. This extension does not
    alter that independent legacy route. The new route instead keeps every matching pointer and
    every checkout named by a registry session not explicitly ended, refusing unreadable ownership.
    The three correction regressions first failed together: a renamed upstream still present was
    selected, a session registered after discovery was removed, and a clean unique commit injected
    after proof was substituted as the expected revision and removed. The corrected route resolves
    the configured remote and full upstream ref even after its tracking ref disappears. Tests cover
    both non-origin and differently named upstreams while present and after deletion, and retain
    missing, local-only, unknown and ambiguous tracking identities. Confirmation now rereads both
    run pointers and sessions, including immediately after the proof hook used by the behavioral
    fixtures, and uses the exact commit id inspected for reachability as retirement's expected HEAD.
    A newly registered holder, corrupted ownership read, changed branch or changed revision is kept.

    These are observations, not a lock: a claim with no pointer and no session directory cannot be
    attributed. Another process can still acquire ownership after the last ownership read, recreate
    the upstream after the last remote query, or change Git state after retirement's branch/HEAD/status
    checks and before `git worktree remove`. The
    correction removes snapshot reuse and revision substitution from the new route; it does not
    make that multi-command operation atomic, and neither route proves absence of every live process.
  - **The sweep's remote deletion bypasses the push guard, and that is a bypass.** `estigia prune
    --confirm` runs `git push --no-verify --delete` for each head, because the guard refuses a push
    no live claim authorises and the sweep runs from a checkout that holds none — a guarded
    repository could otherwise never be swept. A deletion is not work entering the flow, and `--confirm` is the authorisation — passed
    by the operator or, since the contract makes finished cleanup the run's to execute, by an agent — but `--no-verify` is the same rail the guard's own
    script names, so anything that can run `estigia prune` can delete a remote head without a claim.
    The listing before the deletion, and the rule that a live claim's head or a checkout with
    unpublished commits is never marked, are what bound it.
  - `Evidence standard` is one row for one shipped reviewer definition. It is not per role: the
    delegated roles carry fixed grants, and a repository wanting a different grant for one has no
    way to say so.

## Skips, deletions and the bounds of a round

- **A skip spelled as a pass, in the places the `tracker_rig` repair did not reach.** That repair took the skip out of
  `tracker_rig`'s *type*, so no caller of it can be handed a value meaning *did not run*. **It was taken out of the other two fixtures named here, and later out of the last two that
  still answered an `Option` — `git_repo()` in `src/harness/doctor.rs` and `release_repo()` in
  `src/cli/tests.rs` — and this entry records what that closed and what it did not.** The
  paragraphs below are kept as measured, with the disposition of each stated in place, because a
  reader deserves the history that produced the shape rather than only the shape.

  `repository()` in `src/harness/guard/tests.rs` was the exact twin —
  `-> Option<tempfile::TempDir>`, with **fourteen** call sites: twelve early returns and two that skip
  the body instead. Measured with `git` off `PATH` and a POSIX shell still present: the guard module
  answered `28 passed; 1 failed`, with six `SKIPPED:` lines that are only visible under
  `--nocapture`. A normal run showed six plain `ok`. (A first draft of this said thirteen and seven.
  Thirteen counted the definition line; the seventh skip was `no POSIX shell here`, which appeared
  only because the stripped `PATH` used to take git away had removed bash as well — and that one is a
  legitimate skip by the test below, not one of these.) **Closed:** `repository()` now
  returns the directory and panics, naming the command that clears it, when git is absent or a git
  call fails — the same repair `tracker_rig` got. Its subject is the **push guard** — the one gate no
  agent can route around, because it sits under git rather than under an agent, and the one `estigia
  doctor` currently reports as not installed on the operator's machine. **A machine without git now
  fails these tests loudly instead of reporting them `ok`; what is still unmeasured is that no
  machine here runs them without git, so this paragraph's `28 passed; 1 failed` is history rather
  than a current measurement.**

  Seven more skip sites sit in the `pipe` suite itself, across five test functions, at the
  read-only-file capability and at git usability. **Left as they are**, because whether
  they should raise or be `#[ignore]` is a decision rather than a mechanical edit:
  `#[ignore]` is reported as *ignored* and a `return;` after an `eprintln!` is reported as *passed*.
  One of the seven is worse than the others and is counted here for the first time:
  `a_row_that_is_broken_comes_out_of_the_report_broken` prints its skip and **does not return** — it
  drops two assertions and carries on to report `ok`, which is the same failure in a smaller dose and
  invisible even to a reader looking for early returns. **This is the part of the defect
  that survives it**: the read-only-file capability is a genuine machine property and the git sites
  are not, so a single policy cannot close both, and it stays an open decision with its skip sites
  counted here rather than silently.

  The third is newer than both: `repository_with_commit` in `src/test_env.rs`, added since —
  `-> Option<tempfile::TempDir>` again, with **nine** call sites, **seven early returns and two that
  skip the body instead** — five early returns and the two block sites in
  `src/harness/mcp/tests.rs`, one early return in `src/transport/branch/tests.rs`, and one inside
  `two_holders_with_worktrees` in `src/harness/guard/tests.rs` that the base's enumeration did not
  name. The two block-skipping sites were both inside
  `discovery_records_nothing_it_cannot_prove_and_refuses_nothing_on_its_own_account`, whose third
  block needs no `git` and is not guarded at all, so on a machine where the fixture failed that test
  did not skip: it ran one of the three refusals it is named for and reported `ok`. That is the
  smaller-dose failure described one paragraph up, and the entry below says the same
  test "drives all three refusals … in one place" — true only where the fixture succeeds. **Closed:** the fixture returns the directory and panics when a git call fails, so the test now
  runs all three refusals on every machine or fails loudly, and the "true only where the fixture
  succeeds" caveat no longer holds.

  Its tolerance was also wider than `repository()`'s: it covered *any* failure of its four `git` calls
  — `init`, two `config` calls, and an empty commit — and of `tempfile::tempdir()` besides, not only
  a missing `git`. A machine with `commit.gpgsign` set globally, or a `core.hooksPath` hook that
  rejects the commit, took every test built on it to green having measured nothing. **Closed:** failure of any call is now a panic naming the failing command, so a signing machine or a
  rejecting hook fails the fixture rather than passing the tests built on it. That is not an
  idle set of tests: they carry the whole acceptance evidence for adoption on the
  `Swear` and `Renew` effects (`reclaim` and `verify_claim` are the tools driven; `claim` shares
  `Swear` and is covered through it rather than by a test of its own), the two-adopted-checkouts
  refusal, the three discovery refusals, the write-gating test, and the push test, all named where
  the entry for that issue lists what measures it, below.

  The last two are `git_repo()` in `src/harness/doctor.rs` and `release_repo()` in
  `src/cli/tests.rs`, both `-> Option<tempfile::TempDir>` over a single `git init --quiet`, with
  every call site that reached them a `return;` after an `eprintln!`. The count is not restated: it
  measured skip blocks that no longer exist, and the call sites have moved since, so any number
  written here would be one no test can cross. **Closed:** both return the directory
  and panic, naming the failing command, when `git init --quiet` cannot spawn or exits non-zero.
  Measured with a
  failing `git` first on `PATH`: `gate_version_reports_a_row_against_a_real_checkout` reported `ok`
  before and panics at `src/harness/doctor.rs` after, and
  `forgetting_a_pointer_the_tracker_says_holds_no_epoch_removes_it` the same at `src/cli/tests.rs`.
  `two_holders_with_worktrees` in `src/harness/guard/tests.rs`, the third the issue named, had
  already stopped answering one in its own follow-up, so there was nothing left to close there.

  The line between this and a legitimate skip is whether the machine could have run it. Symlink
  privilege, `mklink /D`, administrative shares and drive letters cannot be conjured, and the ten
  skip sites in `src/harness/tests.rs` that answer to those are correct. (Sites, not functions, on
  both sides of that comparison — a reviewer found the two numbers here were counted in different
  units, which is how a count stops being checkable.) `git` is not that: this is a git workflow harness, and a
  machine without git cannot measure the push guard **at all**, so `ok` claimed something never
  checked. That was already decided for the tracker rig, where git absence is now a hard
  failure. **The two `Option` fixtures this paragraph was about have been closed** — `git_repo()`
  and `release_repo()` now panic, naming the failing `git` command, measured with a failing `git`
  first on `PATH`. **The inline skips the same repair had not reached have been closed too**: the
  `git init` that `full_reports_the_home_it_was_given_and_softens_the_push_guard_there` wrote by
  hand, the git-failure skips in the doctor rows that each printed `SKIPPED:` and returned, and the
  inline `git` setup `the_narrowing_resolves_to_the_worktree_the_call_is_adjudicated_in` stood down
  on, now use `git_repo()` or panic, because a machine where `git` is unusable cannot measure the
  rows built on it either. The skips that answer a build-state property rather than a git failure —
  no commit, a dirty tree, a shallow clone, no scratch directory — remain, and legitimately: the
  machine could not have run those. So what still holds two opposite policies for one condition is
  the seven `pipe`-suite skip sites above, which remain the open decision.

  **The guard that holds the rig catches an accident. It does not catch an author, and this is the
  measurement of how far short it falls.** Reviewers walked past
  `the_tracker_rig_cannot_answer_that_it_did_not_run` twenty-three times — eleven of those routes are
  held now and twelve are not, which is the last column below. That split is counted from the table
  rather than remembered: every version of this paragraph that carried a number from memory carried
  a wrong one. Each attempt was measured with the
  fixture removed — except those about *where* the fixture is looked for, which are measured with it
  present, since their point is that the rig looks in the wrong place rather than that it is missing.
  One row is unchanged applied alone and only bites paired with another, and its outcome column says
  so; a reviewer found that reading `no result line` there and measuring something else.
  Where the outcome is `106 passed`, that is the defect reproduced whole with every gate in
  this repository green.

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

  Two of those were fixed because the guard was not reading what it claimed to read: the body
  extraction bound to the first text in the file matching `fn tracker_rig()`, which a two-line comment
  could claim, and the `process::exit` refusal was scoped to the rig's body when its sentence meant
  the suite. Both fixes are **narrower than the first draft of this table said**, and a reviewer
  measured the difference. The decoy took three rounds to close and each round narrowed it rather
  than ending it: from *any text matching `fn tracker_rig()`* to *the whole signature line* to — now —
  the definition itself. The last step is what a check should have done first: there must be exactly
  one line the *filter* reads as a definition, and the body runs from that line rather than from the
  first substring that matches it. A reviewer had put the signature inside a `/* */` block at column
  zero, where a comment does not begin with `//`, and taken the signature check itself. All three
  comment shapes are red now — **against a definition the filter recognises**, which is the load in
  that sentence. The filter is `trim_start().starts_with("fn tracker_rig(")`, so a real definition
  written `pub fn` is invisible to it and a decoy becomes the unique match: measured, with
  `pub struct TrackerRig` to keep `-D warnings` quiet, the whole suite is green and so is the guard,
  while the fixture is looked for under the manifest again. The body's end is the first line that
  trims to `}`, which a multi-line string literal can supply from column zero — also measured, also
  green, also `cargo fmt` clean. Both restore a route this table calls held, and both are one more
  needle away from being caught; that is the reason neither is chased. Recognising a Rust item is
  parsing it. And the file-wide
  `process::exit` scan catches `use std::process::exit as leave;`, because that line contains the
  substring, while `use std::process as sys;` and `use std::process::{exit as leave};` do not. Rows of
  this table said the opposite of each, every time in the direction that flattered the guard, which is
  the direction to be suspicious of. The rest are declared rather than chased. A seventh substring would not close them: a
  macro puts the refused word in a region the scan never visits, a labelled `break` is a return under
  another name, and a body skip never leaves the function at all. Telling any of them from correct
  code needs to know which statements assert, which is a semantic question and not a textual one.

  **What actually holds the accidental path is the compiler.** `let Some(rig) = tracker_rig()` does
  not compile against a non-`Option` — measured, `error[E0308]: mismatched types` — so nobody
  reintroduces this by copying a neighbour, which is how all sixteen callers came to have it. The
  guard adds four things a single careless edit could do and the compiler would not see: reverting the
  signature, moving where the fixture is looked for, ending the process instead of failing, and
  writing `return` in a test that reaches the rig. It holds those against a decoy definition too —
  which it did not until a reviewer took the signature check with a block comment — but only while the
  real definition is spelled the way its filter reads. An earlier draft of this paragraph said three and
  left out the `process::exit` refusal — which is the check that actually catches four of the rows
  above, so the omission understated the guard in the paragraph somebody reads to learn what it
  holds. That is its honest scope. An earlier version
  of this entry said six routes were closed and two things remained unseen; both numbers were wrong
  when they were written, and the sentence claiming completeness is the reason they were worth
  writing down.

  The `return` scan reads whole lines, string literals included, and that is a decision made twice.
  A version of it cut strings out first, to spare an assertion message with the word in it. A reviewer
  measured what that cost: the stripper toggled on any `"`, a char literal among them, so
  `let _sep = '"'; if !built { return; }` hid a plain `return;` and put 106 tests back to passing on
  nothing — a false negative on the one spelling this guard exists to catch, introduced by the change
  meant to make it kinder. It also guarded a hazard that cannot arise where it runs: this reads
  `tests/pipe.rs`, and the two shipped messages saying *"did not return"* are in `src/tui/models.rs`,
  which it never opens.

  So the scan is back to whole lines, and its two false positives are declared rather than engineered
  around. A `return` in a string literal reddens it, and so does one inside a **closure**, which
  cannot skip the test at all. Both measured. The failure message names the offending line, says both
  cases are known, and says what to do — move the line into a helper outside the test, or rephrase the
  message — because an earlier version sent the reader to an entry about closures while they were
  holding a string.

  One more control-surface path of the hosts file's shape, found by a reviewer of this change and
  not closed here:
  `<checkout>/.git/config`, which answers `Routine` while `<checkout>/.git/hooks/pre-push` answers
  `Boundary`. `core.hooksPath` in that file decides whether the push guard runs at all. In the
  ordinary shape it is harmless because `covered()` yields both `repo_dir` and the worktree, so the
  base checkout's `.git` is inside the claim — but measured with a pointer carrying only a worktree
  (`repo_dir = <project>/worktrees/wt-a`), writes to `<project>/src/x.rs` **and** to
  `<project>/.git/config` both stand aside. Whether the fix is naming the file or widening what a
  worktree run covers is the question, and it is not this issue's.

  Narrower since `start_branch` began recording the claim's checkout beside the worktree, which is
  the entry further down about the gate covering two directories: Estigia's own tools no longer
  *produce* a pointer carrying only a worktree. The residual is unchanged for a pointer that carries
  one anyway — a run whose `repo_dir` genuinely is a worktree, or a hand-written record — so the
  entry is narrowed rather than removed, and the question it ends on is still open.

- **A deleted comment is missing evidence, never satisfied evidence.** The verdict requirement does
  not appear only once a handoff exists — if it did, deleting the handoff comment would lower the
  bar from *a computed verdict approved these bytes* to *nothing*, and an erased record would read
  as clearance. Delivery asks for the same computed `approved` on every route, so no deletion can
  manufacture evidence that was never recorded. The one exception is the `low` tier, which licenses
  no lens and needs no marker: a deleted finding there reads as absent and the receipt recomputes as
  `approved`, and a deleted lens report reads as a lens that never ran at `medium` and `high`.

  A deletion can still **restore** something, and that is a narrower claim than "a deletion can only
  refuse". Both cases are reachable only with a `gh` call outside Estigia, which exposes no
  operation that edits or deletes a comment. Deleting a handoff comment lifts the requester
  exclusion it carries, returning the publishing run to the queue for work no verdict covers —
  liveness, not integrity. And deleting a `review-verdict` marker that was `rejected` lifts
  the refusal `legacy-verdict-rejected` held over those bytes, which is the one deletion that clears
  a release: the ledger recomputes `approved` over the same receipt. The bytes it clears were still
  recomputed clean, so this is a lost objection rather than an unreviewed merge — but it is a
  deletion that clears, and saying otherwise would be the sentence, not the code, doing the work.
- **The draft/ready CI barrier is cooperative, not adversarial.** Compatible repositories start PR
  CI on `ready_for_review`, not topic push/open/synchronize/reopen. Beside it they carry a
  `workflow_dispatch` **publication lane**, which `publish_review` starts once per publication epoch
  against the head it pushed — not once per push, and dispatching does not mark the PR ready. GitHub
  has no atomic conditional-ready operation. Collaborators or repository workflows acting outside
  Estigia can mark ready, push, trigger other CI or forge comments; Estigia neither parses arbitrary
  consumer YAML with weak substring checks nor claims malicious-collaborator authenticity.

- **Corrections per lineage are enforced where a correction is published, and counted from markers
  anyone who can write a comment can also delete.** The limit is the `Corrections
  per lineage` row (default 3). A lineage is an original candidate
  and its repairs on one pull request, and the publish call refuses the correction past the limit —
  `correction-already-spent` — before it pushes. Nothing is stored for it: `repair_rounds` derives the
  count from the `parent` receipts on the timeline, so a deleted `published` or verdict comment
  lowers it, and a forged one raises it. What the count does not read:
  - **A correction nobody published.** An attempt that never reached `publish_review` spends nothing,
    and a republish over a parent no panel has judged is not a correction at all, so a run can
    republish a correction as often as it likes before a verdict lands, bounded only by the size
    budget and the scope admission.
  - **A new pull request.** The pull request is the lineage's boundary, so a change moved to a new
    pull request begins with its own corrections. The refusal names that route, because a change that
    needs more corrections than the limit is, by the rule, a new change; nothing here can tell a new change
    from the same one republished under a new branch name.
  - **A base advance forced during a correction.** The integration is a publication over a rejected
    parent, so it is the lineage's correction when it is the first, and is refused when the parent
    already was one; a moved base exempts nothing here. How often the base moves during a
    correction is not measured; it now costs the budget only the repair's own lines, over a
    parent tree that already carries the advance.
  - **A republish over an approved receipt.** It zeroes the count and spends no correction, and it
    stays inside its pull request's budget and scope, so it can still be refused
    `correction-outside-reviewed-scope`. Within that bound a warning- or suggestion-cleared one is
    still narrowed to one lens and no judge by `review_tier_for_repair`, at any size on a non-hot path.
  - **The escalation that follows.** `escalate_review` opens when `repair_rounds` answers
    `correction_spent`, whether or not the publication past the limit was ever refused; a person escalating is
    the exit, and nothing reads whether they had cause.
  Nothing refuses a panel launched over a spent lineage, and nothing requires the call to
  `repair_rounds` be made at all, because neither a launch nor a skipped read is a write the gate
  sees. The refusal is at publication, which is a write. Estigia cannot prove a panel ran, so it cannot
  prove a correction was reviewed. Delivery still requires an approved computed verdict; an escalation
  does not manufacture one. Judges are never told the count. `Repair ceiling` — an advisory number
  default 3 with no refusal behind it — is retired as a setting, and an installed row is reported as
  `repair-ceiling-retired` rather than followed.

- **A validator result records an answer the binary was handed, not proof the validator read the
  trees.** A hand `record_validator_result` is refused `recorded-by-capture-only`, so a result is a launched context's answer, described under the
  validator launch above. `record_validator_result` stores that a read-only validator
  answered `pass` or `fail` for `original_criteria` and `correction_regression` over one correction,
  written by the run holding the `review` claim on the validator's behalf because a sealed validator
  holds no tools to write it. Estigia cannot see which diff or findings it was handed, whether it
  inspected the frozen trees, or whether `unavailable` was honest, and an answer
  admitted from a launched validator is believed as far as the launch is. What it does hold: a conclusive result
  is recorded once, a pass never overrides an open severe finding in the ledger, a failure once the lineage has spent
  its `Corrections per lineage` or a third `unavailable` writes the `escalated` receipt `release_ci`
  refuses, and a failure with a correction left computes `rejected` instead, so the next correction is offered. Four limits. The validator
  is required **in addition to** the tier's lenses, not instead of them, so a correction costs more
  than before rather than less, which is the opposite of v4's saving and is recorded in
  `docs/review-cost.md`. Nothing sets the attempt number except the caller: `attempt` is checked
  against the count already recorded, so a skipped number is refused, but a run that never records an
  `unavailable` attempt is never escalated by it. `budget_exceeded` is not a cause, because an
  over-budget correction is refused before it publishes. And `escalated-to` on the receipt a validator
  writes is the word `maintainer`, not a person, because v4's escalation names none and a person is
  `escalate_review`'s to name. Two fail-closed repairs came with it: a receipt's standing no longer
  falls back to an empty publisher when the timeline cannot be read to the end or holds no publication
  for it — an approval cannot supersede an escalation whose publisher is unknown — and the parent a
  publication records is the newest on its own pull request and only that: a publication on another
  one in between no longer resets the count, and a pull request's first publication records no parent
  at all, where it used to record the issue's newest publication from another pull request. The pull request is the lineage boundary, so a chained slice or a replacement pull request
  begins with its own corrections.

- **A recovery is a person's command, and what the gate can prove about that is narrow.** v4 binds a six-line authorisation text to the predecessor lineage, revision and target identity, and checks only that
  binding. Estigia drops the text: `estigia recover` is a CLI command with no MCP tool, the gate denies
  an agent's shell line for it (`recover-is-for-a-person`) on every host rather than asking, decided by `harness::person_only` before any claim, stand-down or
  role road, and the
  named login must hold the `admin` or `maintain` role on GitHub's own `collaborators/{login}/permission`
  read (`role_name`; the `permission` field alone never says `maintain`). **What keeps an agent out is
  the gate's shell match, not the login check**: the named login is not authenticated, and anyone who can
  run the command can name any maintainer. What that proves: the login
  named holds the role, and no agent *tool* can mint the record. What it does not: that the person at the
  shell is that maintainer — anyone with shell access and a gh token can name another maintainer's login,
  and the check is the login's standing, not their presence. The gate's Bash rule matches the words
  `estigia recover` wherever they stand, on the line and on a dequoted copy with quotes and
  backslashes removed, so a commit message that merely names both is refused too, and an alias or a
  wrapper script that never writes `estigia recover` as two words is not matched, the standing
  every shell rule here has (the entry on the gate's own disarmament, `estigia stand-down` among its
  spellings, says so for those). `!` in Claude Code, and the person's own terminal, never pass through the
  agent's tools, which is the point. Further limits. The record is local, so a lost store loses the
  recovery and a publication that carries a recovered-from field with no readable record is refused
  `recovery-record-missing` rather than read as inheriting nothing. The successor is found from that
  field, so a publication that is not the first over the recovered receipt is a correction like any
  other. The inherited tier is a floor taken when the recovery is recorded, not re-derived. `review_tier`
  and `review_tier_for_repair` read only base and head and do not apply the floor, so a successor's
  advertised lens count can be smaller than the one its computed verdict will demand. And a half-finished review
  that never escalated is not covered: nothing abandons it, a republish supersedes it as it always did.
  A commit message that names both words is refused with no stand-down escape, because a stand-down
  never lifts the rule: the person commits it themselves, in their own shell.
  The recovery marker is a timeline entry like every other marker, so a comment synced from GitHub under
  the same identity can carry it; the store does not tell a synced entry from one it appended, and that
  is the standing every marker here has.
  A correction published on a new pull request over a rejected parent on another is a new change, not
  a correction, and owes no validator.

- **150,000 is an inherited comparison target, not a number derived from the measured
  distribution.** `Review budget` defaults to 150,000 tokens **per context** (a lens or a Judgment Day judge) — not per panel and not
  per issue, because a review is a lens count times this figure and an issue is the round count
  times a review, and neither is a quantity one number bounds usefully — and accepts `none`. The
  number itself is the target this issue inherited from **gentle-ai**, the project it measures
  itself against, as it stood when the issue was written. At `v4.0.0` that project has no
  counterpart: its only `150000` is `ApproxParentContextTokens`
  (`internal/agents/capabilitymanifest/manifest.go:231`), the parent-context length at which a long
  session is advised to delegate, which is a different quantity from a per-judge spend. No percentile, rounding rule or arithmetic step in this repository turns any
  measured figure into it. What the distribution
  supplies is a check, not a derivation: twenty-eight judges measured across five issues, read from
  harness completion reports, put the minimum at 96,073, the median at 127,025 and the maximum at
  182,136 tokens, and 150,000 sits above that median with headroom while 18% (5 of 28) still ran
  over it — a useful rate to sit at, not the arithmetic that produced the figure. This campaign's own
  `record_spend` calls recorded a panel total beside a `contexts` count, not these per-judge figures
  — a convention (`src/transport/claim.rs`), since the validation itself refuses only a
  zero (`src/transport/claim.rs`) and a caller may record one judge with `contexts: 1`. The
  two remain different measurements. **Advisory**:
  nothing here refuses a panel whose recorded tokens exceed it, and nothing compares one figure to
  the row automatically
  — a reader does that by eye. `record_spend` is a declaration, not a measurement — this file's own
  entry on it, above, states the same standing for the number itself: an under-report is
  undetectable, and there is no second source to cross a judge's reported `tokens` against. **What
  none of those twenty-eight figures include is the orchestrating run itself.** Each one is a
  subagent's own completion report, read back by the run that launched it; that launching run's own
  consumption — briefing every judge, reading every report, recording every
  finding — has no such channel to measure itself from inside its own context, and is not in this
  default or in the distribution behind it. **Narrowed: an orchestrator record is now
  possible, not merely absent.** The delivery step records one `record_spend --phase orchestrator`
  before `transition --to done`, self-reported on the same terms as every phase already is — see
  `docs/review-cost.md`'s "The orchestrator's own phase is recorded, not measured". That closes the
  *channel*, not the *trust* gap this paragraph names: the orchestrating run still has no way to
  measure its own consumption from inside its own context, so what it reports is exactly as
  unverifiable as what the twenty-eight judge figures above already are, and a delivery before that
  landed still has no such record at all. **This row's own prose used to be conditionally
  routed, and that gap is closed.** `skill/policies/judgment-day.md` names `Review budget`, and so
  does `skill/bindings/github/operations.md`, but the document is unconditionally routed now — the
  `single`/`two blind`/`five blind` dial that used to leave the default operator with no policy
  document at all is gone, along with the row that chose between them, so every operator reaches
  the row's own description. Unlike the correction state, which developer step 5 routes on
  through `repair_rounds` and `escalate_review`, nothing in the always-routed contract reads or
  reports this figure, so
  naming it there would describe an action no run takes rather than narrow one that exists.

## The correction budget and the publication lane

- **The correction budget is a genesis tree pair, held and never recomputed — and it has narrower
  guarantees than the machine it ports.** `publish_review`/`republish_review` measure a repair
  between two frozen Git tree object ids (`git rev-parse <rev>^{tree}`, `git diff --numstat` between
  two trees), never a payload and never a mutable ref — this is what closed five rounds of severe
  findings against payload-diff and commit-range designs, all one family: a repair refusing correct
  work, or admitting one it exists to refuse, because the input it measured could move under it.

  **The parent this measures against is selected, not validated.** `publish_with` picks the latest
  publication on *this pull request* — `all_publications(comments).iter().rev().find(|p|
  p.receipt.pr == this_pr)` — rather than reading the issue's own newest publication and checking
  whether it happens to belong here. A panel found the earlier shape genuinely, twice, split on
  class: `latest_publication` answers the issue's newest parent from any pull request, so a second
  pull request publishing on the same issue after this one's own original made `this_pr ==
  parent.receipt.pr` false and took the unbudgeted-first-publication path on this pull request's
  very next repair, of any size — a refusal converted into a pass, and the round after that one
  bootstrapped a fresh genesis from the escaped head with `cumulative` reset to zero. Selecting
  fixes it at the one call site that reads the timeline; `correction_budget_check` itself no longer
  re-validates the pair, because a `parent` this function is handed is never one belonging to a
  different pull request by construction. `repair_rounds` scopes to the pull
  request the same way, for the correction count, and shares whatever this leaves open.

  Five things this port does not prove:
  - **A first publication is never budgeted.** There is nothing yet to freeze it against, so the
    check writes `budget-candidate-tree` alone and returns. A candidate of any size — one line or
    two thousand — publishes.
  - **A base advance no longer resets the budget, and what replaced the reset is
    measured against a tree nobody authored.** When this publication's base advanced past the
    parent's (`base_advanced_past`) and the branch integrated it, by merge or by rebase, the round
    is measured from `git merge-tree --write-tree <parent head> <new base>` to this head's tree,
    not from the parent head's own tree. The frozen genesis, the budget and the cumulative carry
    over unchanged, the marker records `budget-parent=integrated`, and the paths the merge conflicted
    on and the advance's rename destinations are admitted beside the genesis paths, because the
    author's conflict resolution and a moved destination are theirs to adjust while the advance's
    other paths are not. Before this, the one legal continuation of a
    `publication-base-not-ancestor` refusal, an integration, started a new genesis with the
    cumulative at zero, so the lineage answered *you have spent nothing* in exactly the case where
    most had changed; it also cost an unmeasured round, which a version of this entry once
    mis-described as a ratchet that only tightened. `budget-refrozen=base-advanced` remains for the
    one shape that still re-freezes, and the re-freeze is deliberate rather than an unresolved
    gap: the branch's fork point is not in the parent's history and the base did not advance past
    the parent's, so the base was replaced by another line of history rather than advanced — there
    is no advance of the parent's own base to measure the lineage against, and charging the
    unrelated base's whole content as an integration would charge the author for a base they never
    had. A fresh genesis measures this branch's own content instead. A first publication, and one
    over an approved parent, are still a fresh lineage.
    What this leaves open:
    - **Conflict resolution is the author's, and is charged.** A conflicted merge still writes a
      tree, with the markers in the files, so the author's resolution is measured as authored
      lines. That is intended and it is not discounted: a conflict-heavy integration can consume
      the budget on lines the author did not choose to write.
    - **Only the paths the merge conflicted on and the advance's rename destinations are admitted
      beside the genesis paths.** The advance's other paths are not: the base's own change to a
      path that merged cleanly is already in this round's candidate tree, and an author's edit
      there is a new change rather than a resolution, so it is refused for scope. An admitted
      path's edit is charged by its lines and read whole; the panel reads the whole change, so it
      is reviewed, not unread. The refusal reports the reviewed set as `genesis_paths` and the
      admitted integrated paths separately as `integrated_paths`.
    - **`git merge-tree --write-tree` needs git 2.38.** A git that lacks it, or a tree it cannot
      write, fails the publication as a failed read naming the cause, with nothing pushed. It
      never falls back to the old reset. The fixture pins the failure on a ref git cannot
      resolve, not on an old git, which this repository's CI does not run.
    - An unmerged advance still never reaches this check: `publication-base-not-ancestor`
    refuses it first. Outside a correction it names two recoveries, merging the
      base's tip or rebasing onto it and publishing again, and either lands here. During a
      correction, when a receipt of the issue stands on the branch's open pull request, it names
      only the merge and says why: a rebase rewrites the parent head and the validator refuses
      `validator-delta-carries-upstream`. That refusal was corrected twice
      over which operation each shape needs, and an enumeration run for it found no other refusal
      in the transport that names a recovery landing on a second refusal. The correction reading
      costs the refusal two reads, the pull request listing and the timeline, and a failure of
      either fails the publication as a failed read rather than falling back to the rebase advice.
  - **The forecast is not measured at all, only the actual.** The source project's own compact
    review estimates a correction's likely size before it runs and compares the estimate to the
    actual after. Nothing here estimates; every figure this port writes is measured after the fact,
    from two trees that already exist.
  - **There is no compare-and-set and no lock**, because an issue timeline is append-only with no
    conditional write (the source's own exclusive purchase: two spans in `compact_store.go`). Two publications racing the same lineage can both read the same parent marker,
    both compute the same cumulative, and both post — the second publication's own marker still
    carries the correct, freshly measured total once it lands, but the single-round rule that a
    lock would make load-bearing is advisory here: nothing refuses the second
    publication, and the lineage's corrections are counted from markers.
  - **The companion-test extension table is this port's own addition, not a translation, and it
    admits more than its own prose discloses.** The source project's own table has no row for Rust
    at all — reasoned in its own comments (`correction_scope.go:66-73`) as a toolchain whose tests
    are separated by build configuration rather than by a name a path can be checked against, and
    naming the hazard in the same breath: the admitted shapes "compile into the shipped artifact",
    so admitting them lets a correction ship unreviewed production code inside an approved
    candidate. That hazard was left undisclosed here until now. Two further gaps neither judge who
    read this file found stated: `correction_companion_directory("tests", ".")` is true, and this
    repository's own genesis directory is `.` on nearly every candidate — `CHANGELOG.md` is under
    `.` and moves with almost every documented change — so for **any** candidate touching a
    root-level file, the entire top-level `tests/` tree is admissible on every repair, not only a
    `_test.rs`-suffixed sibling. The mirror-image gap sits beside it:
    `correction_test_file_name("src/transport/target/tests.rs")` is false, so this repository's own
    dominant unit-test location is admissible only when the reviewed candidate already touched that
    exact file. **What bounds this, and is worth stating plainly:** an admitted path's lines are
    still charged against the frozen budget, and Estigia re-reviews every republish in full — unlike
    the source, where an admitted correction path is not re-inspected by any lens at all.

- **Bounding the frozen manifest does not bound the comment it lands in — and the
  channel left open is now closed.** The publish call refuses `genesis-manifest-too-large` when `budget-genesis-paths`
  alone, percent-encoded, would clear `GENESIS_MANIFEST_CEILING` (32,768 bytes) — but `topology` and
  `topology_reason` are caller free text with no length limit of their own (`claim.rs`; the MCP
  argument definitions at `src/harness/mcp/tools.rs` declare a type and a description, never a
  maximum), and they land on the same `published` marker the manifest does, and again in the prose the
  answer's own `publication_note` writes around it. A caller could still grow the comment past
  GitHub's 65,536 character cap through either field while the manifest itself sat comfortably under
  its own ceiling; that closed one channel, not the comment as a whole. **The comment is measured
  and the field that would not fit is refused, before any remote write.** `publication-comment-too-large`
  fires from `publish_with_sleep` after the values are trimmed and before the fetch, when the caller's
  own marker fields plus `topology_line` would exceed
  `COMMENT_BODY_CEILING` (65,536) minus `PUBLICATION_FIXED_OVERHEAD` (8,192: an upper bound on the
  prose template and every generated marker field **other than the manifest's and the lane's**, at
  its widest, **derived by enumerating the fields `published` writes**) minus `GENESIS_MANIFEST_CEILING`
  (32,768) minus the lane's own measured width
  (`lane_comment_bytes`: three renderings of the candidate workflow names plus `LANE_SENTENCE_CEILING`,
  which also bounds the clamped `gh` detail an absent or unknown lane carries).
  Every bound has to be reserved or they do not compose — a manifest at its limit, a lane as wide as
  the repository's workflows, and the caller's own text at this one must all fit together. Three
  panels found the three ways this went wrong: the manifest was not reserved; `PUBLICATION_FIXED_OVERHEAD`
  was a 1,454-character sample; and `run-id` was treated as bounded by the run-pointer filename. The
  constant is now an enumerated bound with a pinning test, the lane is measured, and `run-id` is
  measured with the topology. Nothing is pushed and no pull request is touched, and
  `an_oversize_topology_is_refused_before_any_remote_write` (`tests/pipe/part_10.rs`) reads the fake `gh` call
  log to assert no write was attempted. **The enumeration of caller-controlled marker fields behind
  that ceiling:** `run-id`, `topology` and `topology_reason` are all caller-supplied and none is
  bounded by a constant; every other marker field is generated by the transport.

  Measured, not estimated, and measured in the quantity the ceiling actually compares — the
  percent-encoded length, not the raw join. Across all 109 merge commits in this repository's own
  history the largest candidate manifest encodes to 1,452 characters across 64 paths (`02e32994`,
  1,326 raw), and a second, larger real repository on this machine — not named here, and no figure
  here was taken from a shallow clone — reaches 13,095 encoded across 195 paths (12,707 raw). Both
  are under the ceiling: 4% and 40% of it. The two magnitudes matter because they can diverge by up
  to three times, and on real path sets they do not: the measured encoded-to-raw ratio is 1.03 to
  1.10, because a repository path is almost entirely bytes the marker's safe set already passes and
  only the `\u{1f}` separators inflate. A path set built from bytes outside that set would sit at the
  3x bound instead, which is why the check measures rather than assumes.

  What that margin is **not** is a guaranteed remainder — but with the caller's own
  fields measured and both the manifest's ceiling and the lane's measured width reserved, the comment is
  bounded: the fixed 8,192 characters plus a manifest of at most 32,768 plus the lane plus the
  caller's text at the remainder sum to no more than the cap. The reservations are conservative — the
  manifest ceiling is charged even on a first publication, which writes no manifest at all — and that
  is the direction to err, because a comment GitHub rejects leaves the branch pushed with no receipt.

- **The budget's own tree diff runs isolated from every `.gitattributes`, not only the untracked
  one a review found.** A blind panel found that a local, untracked `.git/info/attributes` carrying
  `* -diff` marks every path binary, which `count_changed_lines` skips — a gate that decided nothing
  while looking like it worked. Measuring the committed alternative mattered before deciding how far
  the fix should reach: **this repository's own `.gitattributes` is committed, 1,068 bytes, and
  already marks four patterns `binary`** — `*.png`, `*.jpg`, `*.gif`, `*.ico` — for an unrelated
  reason: the same file's own `* text=auto eol=lf` line, whose comment records the line-ending
  defect it fixes as measured 2026-08-13, keeps line endings consistent across checkouts and has
  nothing to do with review measurement.
  That file is not exotic and not adversarial; it is maintained. One more line in it — a
  `*.md -diff`, or a vendored directory marked wholesale — would have silenced the budget with
  nobody touching review measurement at all. **A second panel found that isolation still had a gap:
  it layered its environment onto the caller's own rather than replacing it, so a caller's *default*
  global attributes file — resolved from `HOME`/`XDG_CONFIG_HOME`, not from any config value
  `GIT_CONFIG_GLOBAL` could redirect — still reached the count.** Measured directly: with `* -diff`
  in that file, an otherwise two-line change counted `-\t-` under the layered isolation and its true
  additions and deletions once the environment was replaced instead. `diff_stats`'s
  `DiffIsolation::Isolated` now reaches the child through `run_with_isolated_env`, which replaces the
  environment the way `sanitizedGitEnvironmentForRun` does under `isolateConfig`
  (`snapshot.go:1874-1922`) rather than layering the way `run_with_env` does everywhere else in this
  crate — ports the isolation `isolatedImmutableTreeGit` (`frozen_candidate_context.go:319-387`)
  builds together with the environment replacement the source applies around it, rather than the
  first alone. A fresh `GIT_DIR` with no worktree, no index and no attribute source of its own, and
  no `HOME` or `XDG_CONFIG_HOME` left for a default one to resolve through, so a committed
  `.gitattributes`, an untracked `.git/info/attributes`, the caller's own global or system git
  config, and the caller's own default global attributes file are all equally invisible to the
  budget's own measurement now. `GIT_ATTR_SOURCE`, `GIT_CONFIG_PARAMETERS` and the caller's own
  `LC_ALL` are gone the same way, as a consequence of replacing the environment rather than by
  naming each one — the candidate's own binary detection still runs on content, since git sniffs
  `NUL` bytes independently of any attribute. `review_tier`'s own `diff_stats` call stays ambient: now a `low` answer lets
  `release_ci` release on the structural readback, so that read now moves a clearance, and it is
  out of this entry's scope.

  **Two tests, two levels, after one shape of the first was measured to poison its own suite.**
  `essential_env` — the filter `EnvMode::Replace` applies — is unit-tested directly over a synthetic
  environment (`transport::tests::essential_env_drops_home_and_keeps_path_case_insensitively`), with
  no process environment touched at all. The composed path —
  `tier::tests::a_default_global_attributes_file_does_not_reach_the_isolated_diff` — does redirect
  this test binary's real `HOME`/`XDG_CONFIG_HOME` and does write a real default global attributes
  file, because that is the only way to prove `diff_stats` actually reaches `git` through
  `run_with_isolated_env` rather than merely calling a filter nothing else calls. Its first shape
  used `* -diff`, matching every path, and poisoned every other concurrently running test's own
  ambient `git diff` for the width of that redirect — measured directly, the same two unrelated
  tests failing in each of three full-suite runs with that rule in place, and four consecutive clean
  runs once the rule was narrowed to the one committed path this test's own fixture uses by an
  otherwise-unused name (`estigia-default-global-attributes-sentinel.txt -diff`), which no other
  test's own diff can ever match.

  **Still true after this repair: deleting `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_SYSTEM`,
  `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_COUNT`, `GIT_ATTR_NOSYSTEM` and `LANG` from `tier.rs`, keeping
  only `GIT_DIR`/`GIT_OBJECT_DIRECTORY` and the environment replacement above, leaves the whole
  suite green — measured on this repair's own head, not carried over.** Two of the six protect a
  path this fix does not touch at all: git's own compiled-in system config and system attributes
  file are not derived from `HOME` or any other environment variable, so `GIT_CONFIG_NOSYSTEM` and
  `GIT_ATTR_NOSYSTEM` are the only things standing between the isolated diff and a real system-wide
  install's own `gitconfig`/`gitattributes` — and proving that reddens without them would mean
  writing to that real, shared, often privileged path, which this suite does not do and should not
  start doing. The other four are not merely untested now; replacing the layered environment with a
  cleared one made them **redundant by construction**, not only by absence of a test: `GIT_CONFIG_GLOBAL`
  and `GIT_CONFIG_COUNT` only ever intercepted a value carried on the ambient environment the
  replacement already drops before either is applied, and `LANG=C` is shadowed by the `LC_ALL=C` the
  replacement sets in the same breath, `LC_ALL` taking precedence over `LANG` in every locale POSIX
  defines. No input reddens their removal any more, which is a different claim from "untested" and
  is the reason this repair adds no test for those four rather than a contrived one.

  **Considered and left as it is: the source's `--text` flag on its count operations.** A
  `frozen_candidate_context.go:253` call forces `--text` so a path git classifies binary on content
  is still charged its line count. That flag belongs to `Inspect`, which renders a candidate to a
  provider adapter this port does not implement. The function `diff_stats` actually mirrors is
  `DiffStats` (`snapshot.go:461-471`), reached from `ChangedLines` (`risk.go:884-890`) and measuring
  both the risk tier and the correction budget: its own `git diff --numstat` carries `--no-renames`,
  `--no-ext-diff`, `--no-textconv` and `--ignore-submodules=none` and **no `--text`**, and
  `snapshot.go:492-493` then maps a `-`/`-` pair to `Binary`, which `CountChangedLines` skips. So a
  binary-content path is charged zero lines by the source's own correction budget, exactly as it is
  here.

- **The review payload a judge is handed runs isolated the same way, and that moves a digest
  wherever a repository's own config sets a diff knob.** `review_payload_bounded`
  (`src/transport/target.rs`) built the unified diff every judge and every crossing operation reads
  with a plain `git diff`, layered over the caller's ambient environment the way `run_with_env`
  layers everywhere else in this crate. The same collapse the entry above measures for `diff_stats`
  reaches it: a committed `.gitattributes`, an untracked `.git/info/attributes`, `core.attributesFile`
  or the caller's own default global attributes file marks every path binary, and the payload
  shrinks to one `Binary files a/x and b/x differ` line per path — small enough that
  `REVIEW_PAYLOAD_CEILING` stops binding at exactly the size it exists to refuse. It now runs
  through `isolated_diff_env` and `run_with_isolated_env`. The first moves to `src/transport/mod.rs`
  so both callers share one copy rather than two; the second already lived there.

  **Isolating the payload makes it independent of ambient diff configuration, not merely of
  attribute sources — and that is a real behaviour change, not only a closed gap.** `diff.algorithm`,
  `diff.interHunkContext` and `diff.indentHeuristic` are not attribute sources; they reach a plain
  `git diff` the same way an attribute file does, and none of the three is pinned by
  `pinned_diff_args`. For a repository whose own config sets one of them, the isolated payload's
  bytes move once, and a `payload-digest` recorded before this change will not reproduce against a
  payload rebuilt after it. Measured: a two-commit fixture with two one-line edits ten lines apart
  answers a 23-line payload under an ambient `diff.interHunkContext=10` and a 20-line payload once
  isolated — the knob merges two hunks into one, absorbing four unchanged lines and one hunk
  header. `tests::a_local_diff_inter_hunk_context_moves_the_ambient_payload_but_not_the_isolated_one`
  pins it: it asserts the ambient diff moves under the knob before asserting the isolated one does
  not, so the test cannot pass on a knob that was never live.

  **No digest already on this timeline moves.** The class this change closes reaches both the three
  `diff.*` config layers and the attribute sources measured for the entry above
  (`docs/honesty.md`), and both need checking, not only the first. Measured directly
  against the three config layers: no `diff.*` key is set in `git config --local`, none in
  `--global`, and the only `--system` entry is `diff.astextplain.textconv`, a content driver that a
  `git diff` never invokes unless an attribute assigns it to a path and that the pinned
  `--no-textconv` flag disables regardless — the system `gitattributes` this machine ships
  (`C:\Program Files\Git\etc\gitattributes`) assigns only that same `diff=astextplain`, disabled the
  same way. Measured against the attribute source the config layers alone would miss: this
  repository's own committed `.gitattributes` marks `*.png`, `*.jpg`, `*.gif` and `*.ico` `binary`,
  which is inside the class isolation now closes. That marking does not move a digest either —
  `binary` renders the same `Binary files a/<path> and b/<path> differ` line the payload's own
  content sniffing already produces for those four extensions, so isolating the attribute away
  changes nothing about what the payload contains for them. Four sources the `diff.*` scan cannot
  see were read separately, because either would mean a past payload was recorded already collapsed:
  `core.attributesFile` and `core.bigFileThreshold` are unset in every scope, and neither an
  untracked `.git/info/attributes` nor a default global attributes file exists in the publishing
  checkout, the base checkout or this user's `.config/git/`. That is a fact about this machine and
  this repository on the day it was measured, not a general guarantee — a fork, a contributor's own
  machine, or this repository's own config or `.gitattributes` a year from now could set something
  none of the above measured and move a payload's bytes the next time it is rebuilt.

  **Six sites across four files described the previous behaviour when this landed, and they
  were closed.** Isolating the payload falsified every sentence about the rebuild
  `review_payload_membership` performs, since it reaches `review_payload_bounded` through
  `super::target::review_payload` — in `CHANGELOG.md`, twice in `skill/bindings/github.md`, in the
  MCP tool description, and twice in `src/transport/claim.rs`, the last of those being the `action` a
  caller reads on a refusal, which named a recovery isolation had turned into a dead end. Correcting
  them here was refused by this repository's own correction budget: `admit_correction_scope` answered
  `correction-outside-reviewed-scope`, because those paths lie outside this candidate's frozen genesis
  manifest. That refusal was right — they are a new change, not a repair — and widening a gate to
  make a change pass is refused by `AGENTS.md`, so the work was split and delivered on its own
  lineage. No line numbers are cited above: they moved when the correction landed, and a coordinate
  in a closed disclosure is a coordinate nobody re-derives.

- **Reading the lane before launching a panel is instruction, not enforcement.** `publication_lane`
  answers green, red, never-ran, running, absent or disabled, and an unreadable listing or
  permissions answer is a failed read. Step 5 of the contract routes on all of them. Nothing
  refuses a panel launched over a red lane, because a sub-agent launch is not a write the gate
  sees. `record_computed_verdict` still refuses an
  approval on a lane whose run genuinely ran and failed, or one still unfinished — the one case it does not refuse is a lane whose run never executed a step at all, gated on
  the caller's own evidence rather than on green. This adds an earlier read; it removes no later
  refusal but that one.

- **Estigia starts the publication lane and cannot prove a repository has one.** The refusal in
  `record_computed_verdict` is real — an approval is not banked for a head whose check runs
  are red or unfinished — but everything upstream of it is best-effort, and the shape of what it
  does not cover is worth being exact about.

  It discovers every active workflow under `.github/workflows/` (or takes an explicit
  `Publication lanes` list) and dispatches each once per publication epoch, recording the started
  set and its count on the receipt. It still **does not parse consumer YAML** for
  `workflow_dispatch`: a candidate without the trigger is told apart only when `gh workflow run`
  answers `HTTP 422`, and is then dropped from the started set rather than refused. What remains
  unprovable is that the dispatched set is the set that *matters* — Estigia can see which workflows
  exist and which accepted the dispatch, never which ones a repository considers load-bearing. A
  release lane, a nightly, or any other `workflow_dispatch` workflow that must not run per
  publication is excluded only when the operator names it on `Publication lanes exclude`; discovery
  alone will start it. A repository with no dispatchable candidate gets no check runs on its head —
  and **a head with no check runs proceeds**, deliberately. That is the old ordering and the old
  exposure: the reviewers spend their rounds first and CI answers afterwards, exactly as before.
  Refusing there instead would have broken every consumer that has not adopted the lane in order to
  protect the ones that have, so absence is a pass and this entry is where that is admitted rather
  than a gate that looks tighter than it is. When the workflows listing itself cannot be read, the
  historical single name `ci.yml` is tried as a fallback so a list outage is not a silent absence;
  that fallback is not a claim that every repository's lane is called that.

  Eight narrower limits, none of them hypothetical:

  - **The dispatch names a branch, not a commit.** `workflow_dispatch` takes a ref. Somebody pushing
    over the branch between Estigia's push and its dispatch starts a lane on their bytes; the
    published head then has no check runs and clears, which is the old ordering rather than a false
    green.
  - **`HTTP 404` is read as *no lane*, not as *no permission*.** GitHub answers `404` for both, so a
    token that cannot see the workflow loses the protection silently instead of being refused. Only
    `401` and `403` refuse (`publication-lane-forbidden`). The alternative — refusing on the
    ambiguous code — stops repositories that have no lane at all from publishing.
  - **Only `accepted` is gated.** A rejection over a red head is the route out, not the waste this
    issue measured, so it records. And `neutral` and `skipped` conclusions clear, because a matrix
    leg that skips on purpose must not refuse a verdict; every other conclusion, including none at
    all, refuses.
  - **More than one page of check runs is a failed read, not a verdict.** The listing is asked for at
    `per_page=100` and refused when `total_count` outruns what arrived. A repository with more check
    runs than that on one head cannot record an accepted verdict at all until the read is **widened**
    — a larger page, or following the pages until every check run has arrived — fail-closed, and
    untested against a real repository of that size.
  - **The never-ran jobs read carries the same ceiling.** `never_ran_signal` asks
    `actions/runs/{id}/jobs` at `per_page=100`, matching the check-runs read beside it. A workflow
    run with more jobs than one page answers `None` from `jobs_report_never_ran`'s own partial-page
    check, which `classify_reds` reads as `Ran` — the ordinary red refusal, fail-closed, and
    untested against a real run of that size.
  - **The read side is fail-closed on a permission error, where the dispatch side is lenient.** The
    two halves disagree on purpose and the cost lands on one token shape. `publish_review` reads
    `HTTP 404` from `gh workflow run` as *no lane* and proceeds; `record_computed_verdict` maps **any**
    failure of `gh api .../check-runs` to a refusal, because an unknown result is not clearance. So a
    token that can push, comment and dispatch but lacks `Checks: read` publishes normally and can
    then never record an approved computed verdict — the refusal is `read-failed`, permanently, and no retry
    reaches it. Its **detail** says so, naming the read that failed rather than a lane that answered;
    the shared `read-failed` envelope it travels in still ends *retry the read*, which for this token
    shape is advice no retry can take, and that generic sentence is the one every failed read in the
    crate gets. That detail is the whole of the mitigation: nothing detects the shape, and the two
    halves are not going to be made to agree without either refusing repositories that have no lane
    or clearing heads nobody read.
  - **A dispatched run is not visible the instant the dispatch returns.** `gh workflow run` answers
    when GitHub has accepted the request, not when the check run exists on the head. A verdict
    recorded inside that window sees an empty listing, and an empty listing proceeds. Narrow in
    practice — a review takes minutes and the check run appears in seconds — but it is a second way
    "absence proceeds" fires, and unlike the branch-overwrite race above it needs no second actor.
  - **The listing is every check run on the head, not the lane's.** `commits/{sha}/check-runs`
    returns whatever anything attached to those bytes: a coverage bot, a security scanner, a
    repository app unrelated to the lane set Estigia dispatched. One of those sitting `queued` refuses an accepted verdict
    naming a lane the reviewer cannot start, watch or clear, and there is no way past it but to wait
    or to remove the app. Fail-closed, and arguably the right answer for a head nobody has finished
    checking, but it is a wider net than "the lane Estigia dispatched" and this is where that is
    admitted.

  A dispatched run's check runs **do not** satisfy branch protection's required contexts on a pull
  request head. Measured on this repository, 2026-08-27, both runs against head
  `ead3ca84`: `gh workflow run ci.yml --ref <branch>` (Actions run `33027439851`) completed success
  with three check runs on the commit under `app_id 15368` carrying the three context names then
  required; with those green, `gh pr checks 110` still answered `no checks reported`,
  `statusCheckRollup` was `[]`, the commit GraphQL `statusCheckRollup` was `null`, and
  `mergeStateStatus` stayed `BLOCKED`. Toggling draft → ready produced a run with
  `event: pull_request`, and the rollup went 0 → 3 immediately, every entry pointing at that run.
  Lag and name mismatch were both ruled out by reading. The publication lane remains the cheapest
  disqualifying signal **in the Actions tab**, where `record_computed_verdict` reads it — not a claim
  about the pull request's mergeability. The draft barrier still closes the merge hole:
  `release_ci` verifies a draft before marking ready, and GitHub refuses to merge a draft.

  And it **cannot protect its own delivery.** `workflow_dispatch` has to exist on the default branch
  before `gh workflow run` will accept it against a topic ref, so the publication that carries this
  change is dispatched against a `main` that does not yet have the trigger and is refused `HTTP 422`
  — reported as `publication_lane: absent`, not as a failure. The first delivery this protects is
  the one after it.

- **A marker field that is present but unreadable is refused by name rather than read as an absent
  one — and the two readers that used to collapse the two are the ones that changed.**
  `ReviewTransaction::from_marker` and `CorrectionState::from_marker` both returned `Option`, so a
  `published` marker whose fields were present but malformed was indistinguishable from one that
  carried no such fields. `all_publications` consumed them through `filter_map`, so a corrupt
  publication vanished from the lineage — it did not appear to `latest_publication`, `repair_rounds`,
  the pull-request-scoped parent selection, or the correction budget — and a half-written `budget-*`
  field reset the lineage's cumulative to zero in silence, the fail-open the correction-budget entry
  above already measures from the re-freeze side. Both readers now answer three outcomes
  (`Result<Option<T>, Failure>`): absent, present-and-valid, and present-but-unreadable, the third
  refused as `published-transaction-unparseable` or `correction-state-unparseable`, naming the marker's
  `epoch` and the first offending `field`, before anything remote moves. `all_publications` and
  `latest_publication` are fallible in turn, so every reader of the lineage — `review_transaction`,
  `review_findings`, `review_payload_membership`, `repair_rounds`, the correction budget,
  `release_ci`'s verdict lookup and `verify_claim`'s own-delivery close — refuses rather than
  proceeding over a publication it silently dropped.

  **The absent case is the designed migration path and is untouched.** Every `published` marker
  written before the budget fields existed carries none of them, and `bootstrap_correction_state`
  freezes a fresh lineage from such a parent; a marker carrying *none* of the fields still reads
  `Ok(None)`. Only a marker carrying *some* of them and failing to read them all is refused.

  **The choice between refusing and dropping-with-refusal-on-the-operation was made against the
  timeline, not assumed.** The uncertain half of the issue was whether an older `published` marker
  might legitimately fail `is_complete()` and so turn a readable timeline unreadable. Measured
  against this repository's own timeline: the **oldest** surviving `published` marker is
  `234883ed2fa264d0be514a36fd074342` on one issue, written 2026-08-23T21:19:26Z, and it is complete —
  32-hex epoch, positive PR, 40-hex head and base, 64-hex digest. Of **649** `published` markers on
  the timeline, **none** is malformed. Refusing is therefore safe against every marker this
  repository has ever written, and the drop-with-refusal alternative was not taken.

  **What remains unmeasured.** Reachability is the weak part and is stated rather than implied: these
  markers are written and read by Estigia, `comment` escapes control-shaped prose so a marker cannot
  be forged from an agent's text, and `all_publications` already filters to self-authored, unedited
  comments — **no instance of a malformed marker has been observed**. The two ways in are both things
  this project already does: a partially written marker (the unbounded `budget-genesis-paths` failure
  lands after the push) and version skew between a swapped binary and the MCP server
  still holding the old executable. How often a base advance occurs during a review round is also not
  answered here; an audit counted 7 of 270 publications whose base was not an ancestor of the
  head, and that is a floor for movement, not a measure of it.

## Setup, configuration and uninstall

- **The superseded-reviewer ledger is crossed against the templates beside it, and not against a
  freeze that never happened.** `REVIEWER_TEMPLATES_SUPERSEDED` (`src/setup/mod.rs`) is
  the hand-written list of every previous `review-blind.md` and lens body, and `reviewer_is_ours`
  consults it so an install still carrying an older definition upgrades instead of being refused as
  foreign. Nothing crossed it: commenting out an entry left the suite green while every operator
  carrying that body met a refusal.
  `every_frozen_reviewer_template_is_a_member_of_the_superseded_ledger` (`src/setup/tests.rs`) now
  reads `src/setup/mod.rs` through `source_text::code_of` and requires every `include_str!` const to
  be a ledger member and vice versa, and every `.md` file in `src/setup/` to be declared. **It cannot
  see a freeze that never happened.** An author who edits the current template and updates the
  `replacen` chain `the_default_rendering_is_the_bytes_already_installed` walks, but never adds the
  previous body as a new `include_str!` const, adds nothing for this guard to cross: the sibling
  guard is satisfied by the chain alone, the ledger stays one body short, and neither guard holds the
  chain against the list. The two hold one half each — this one the list against its declarations,
  the sibling the chain against the current rendering.
- **`status` names why the tools half is off, and leaves two of the four states together.** `tools off` was one word for four states — no verified registration site, a site known and
  not written yet, a file that cannot be read or parsed, and an entry under a key this build does not
  look under — and the only prose on the row answered the *gate* half. `AgentAdapter::tools_gap()`
  now names the first and `status` prints it under the harness line, while an adapter that can
  register but has not prints `run: estigia setup <agent>`. **The last two still print that same
  command.** `exposes_tools` (`src/setup/mod.rs`) answers `false` for an unreadable file, an
  unparseable one and an entry under a key this build does not look under alike, and the row reads
  that one bit. `tools_command` reads the same file and returns `None` for the same three, so
  neither tells them apart, and the sentence printed beside the command says no registration is
  there for states where the entry may exist or the file may not be readable.
- **The two readers of the configuration table are not identical, and one half is now closed.**
  Both read the same block of the same file, and they resolve a row differently. The binding looks a
  setting up by **prefix** — `cfg` returns the first key that `startswith` the label — and this crate
  compared whole words, so `| Repair ceiling (mine) | 2 |` was a row the transport acted on and
  Estigia reported as unset — `Repair ceiling` was read by the transport then and is a retired
  setting now, and the example stays because the two readers still resolve any other label this way.
  Measured on the installed pair: the transport read the value `2` there, `Repair ceiling  3` here. That half is closed —
  `a_row_the_binding_acts_on_is_a_row_this_reader_finds` holds it — by trying the prefix **after** the
  exact match, so an exact spelling never loses to a looser one.
  **The other half is closed too, and by neither of the two ways this entry said it would take.**
  It said closing it meant resurrecting the silent drop, or reporting the row as one nobody reads —
  and reporting it is what `doctor` did, on a **BROKEN** row, for several rounds. The third way is
  that the transport stopped reading the operator's cells at all: `Context::live` reads *this
  crate's own rendering* of the layered configuration, so `| Repair  ceiling |` with a doubled space
  and `` | `Repair ceiling` | `` with backticks both reach it as the value this crate read out of
  them. The `doctor` row went with the gap, and
  `a_label_this_crate_forgives_is_one_the_transport_reads_too` stands where it was.

  What is left is not a difference between two readers: `get` answers with the **first** row whose
  label the question is a prefix of, so `| Worktree location (mine) |` still answers `worktree location`.
  That is written on `Context::get` and is the same surprise on both sides.
- **An override row is matched by its label, and a label no setting resolves is reported rather
  than dropped in silence.** `estigia.local.md` is the operator's own file and `setup` never
  regenerates it, so a row outlives the setting it named. Every reader resolves a row through
  `Setting::from_label`, which answers `None` for a label this build does not know — a deleted
  setting and a typo alike — and the row falls out of a `filter_map`. Nothing refused, warned
  about or reported it, so the row read as configured and did nothing. `doctor`'s `contract` row
  now names the file and the labels in it that resolve to no setting
  (`skill::unknown_override_rows`, read into the row through `Contract::unknown`). **It is a
  report, not a refusal**: the row is still dropped from the effective table, every other row in
  the file still applies, and no command fails. Only the labels are read — the values are
  permissions and machine paths and never reach the report. A file that cannot be opened is a
  different case and unchanged: `local_overrides` refuses it by name, because falling back to the
  values underneath would run on settings nobody chose. What this still cannot do is tell a typo
  from a deleted setting; both are named, and the operator is the one who knows which they meant.
- **An uninstall takes an empty `hooks` wrapper it may not have written.** When Estigia's entries
  leave a settings file, the now-empty `hooks` object goes with them, on the reading that Estigia put
  it there — which is right whenever it did. An operator who wrote `"hooks": {}` themselves loses it,
  and it is measured rather than argued: `estigia setup cursor --uninstall`, on a machine where
  nothing was ever installed, rewrites that file and reports `update`. Telling the two apart needs a
  record of what this crate created; one exists for the instruction file (`created_outside`) and not
  for this. `an_uninstall_leaves_no_empty_husk_of_its_own_making` asserts the half that is decidable
  and says in its own body why it stops there.
- **The oracle covered 125 of the transport's 131 reachable functions when it was retired.**
  That number is now a fact about two deleted files — the reference implementation and the recording
  of it — and it is kept because it is the high-water mark of what was ever crossed. Six functions
  were never crossed and never will be: `read_ownership` and `read_lock_record`,
  which needed a real checkout on disk rather than a scripted `git`; `reclaim_landed`, the read-back
  inside `cmd_reclaim` that every reclaim crossing stopped before; and three legacy command bodies
  kept for older markers. The port's behaviour in those six places has never been crossed against a
  second implementation and cannot be now.

  **The numerator was measured with a tracer that loses lines.** Four traced runs of the same suite
  disagreed — 124 and 125, and one of them dropped `cmd_config`, which every run of `config`
  executes — because the crossings ran in parallel and each recorded into one file. The number above
  is the union of those runs, which is the honest reading of a counter that can miss and cannot
  invent.

  **What replaced it can fail, and that was measured.** Corrupting one recorded answer turns its
  crossing red; the corpus is load-bearing rather than decorative. What it cannot do is answer a
  question it was never asked: a fixture that changes fails with *no recorded answer for this
  crossing* rather than quietly agreeing, and re-blessing one from the port itself is possible,
  named in that message, and costs the case its independence.

  The refusal for a case with no recorded answer used to name an `ESTIGIA_ORACLE_BLESS` that was
  **never implemented** — a message promising a way out that does not exist, which is worse than
  promising none. It now says the only thing that can extend the corpus: bring the reference back,
  re-record, delete it again.

  **The question was wider than the answer, which made ordinary editing expensive.** A crossing's
  fingerprint took every file of a fixture's payload, so rewording a sentence in
  `bindings/github.md` invalidated a recorded answer about the *configuration block* — and the only
  remedy on offer was the blessing above. It now takes the three files the transport opens out of a
  payload, at any depth: the contract and the two spellings of the operator's own overrides. The
  corpus was re-recorded against the reference, which had to be brought back to do it and was
  deleted again after — and narrowed once more when a sentence added to the contract's step 6
  invalidated a crossing about the table: of a payload document it now takes the **configuration
  block**, which is the part `Context::live` parses.
- **Fifty-nine refusals the transport can give are named in no prose an agent reads.**
  `every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer` accepted a reason
  spelled anywhere in `github.py` as documented — and the binding's source is not something an agent
  reads. Deleting that file did not create these; it stopped concealing them. `lost-claim-race` and
  fifty-eight more reach a run as a word to branch on, with the instructions that go with them living
  only in the `action` field of the envelope itself.
  Two of these — `holder-not-stale` and `operation-id-kind-conflict` — were closed by naming
  them in `skill/bindings/github.md`'s `reclaim` and `claim` rows. `reclaim-discarded` was then added
  to the `reclaim` row with its instructions, and the same sentence names
  `lost-reclaim-race`, so that refusal left this list as the new one arrived documented. The figure
  this entry carried before that change counted one more than the list held, so the correction moved
  it by two rather than one: the over-count retired and `lost-reclaim-race` removed. Then
  `board-empty-inconclusive` came off the list as the audit-board operation left the port, moving the
  figure once more.

  The list was then re-derived against `TRANSPORT_VOCABULARY` — the vocabulary `build.rs` generates
  from all three spellings a refusal can take — where this check had re-implemented only the first.
  Seven refusals the narrower walk could not see joined it: `read-failed`, `write-failed` and
  `ambiguous-write` from `Failure::envelope`'s `match self`, and `already-yours`, `nothing-to-reclaim`,
  `not-current-live-holder` and `unexpected-state` from `stop` calls whose reason was not a
  `"reason":` literal. Each is a real gap, not an exemption: the transport can answer with it and no
  prose an agent reads names it.

  Eleven then joined the list: eight of them were spelled by `operation_marker`'s two
  `format!`-built expressions, and three were verdicts of `branch_identity_verdict`
  (`src/transport/worktree.rs`) that `checkout_identity` passed through a plain `&'static str`. That
  function returns four incoherent verdicts across three return sites — one site carrying two — and
  the fourth, `remote-not-reachable-from-local`, was already documented in
  `skill/bindings/github/delivery.md`. They were as unreadable to this walk as they were to
  `build.rs`'s: a `"reason": <expression>` the check's own literal-only extractor could not read was a
  reason it silently skipped, the identical blindness `TRANSPORT_VOCABULARY`'s extractor had. Giving
  each kind's two reasons a literal per match arm (see `OperationKind` in `src/transport/claim.rs`)
  fixed both walks at once: the harness gap that was filed, and this one, which had been
  concealing eleven reasons this walk had never been able to see rather than eleven the crate had
  newly grown.

  The number counts **refusals**, which is what `UNDOCUMENTED_REFUSALS` holds. The check now reads
  `TRANSPORT_VOCABULARY` itself, so it collects one entry per code and the two counts are one: the
  list is the exact set of vocabulary codes no prose names, and either a code the prose does not name
  or an entry that is no longer missing is red.

  The list is frozen in `UNDOCUMENTED_REFUSALS` and the check is a ratchet: a **new** refusal with no
  prose is red on the review that adds one, an entry that stops being undocumented has to be taken
  off, and the list can only get shorter *for a refusal the check could already see*. Closing it means
  writing the instructions into `skill/references/`, and that is not written yet.
- **A read-back that agrees after waiting may agree for the wrong reason, and nothing here can tell.**
  Ten spaced attempts now cover about ninety seconds because the pull request object lags the branch
  ref — 45 and 120 seconds on the two publications measured — and the refusal says which
  world the last read stood in: the ref carrying the pushed head means wait, a moved head means
  investigate, an unreadable ref means it cannot tell. What the distinction cannot do is see a foreign
  push land inside the waiting window. A run that pushes, waits through lag, and reads agreement may
  be reading a pull request that settled onto a head it did not push, and the receipt binds that head
  either way; the branch-ref check narrows the world to the last read, not to the whole window.
- **An exit-2 refusal with no `action` is still sent to `estigia config list`, and nothing checks
  that the command clears it.** `translate`'s code-2 arm carries the transport's own `action` for
  every reason the table does not answer, and the fallback message repeats the envelope's scalar
  fields; that closed a gap where a `finding-citation-unresolvable` reached a session as the
  exit code and a configuration command and nothing else, and the session spent five calls guessing
  at a `cause` the envelope had carried all along. The fallback is not the code-2 arm's alone: it
  runs for **every** code whose envelope carries no `detail`, and `Failure::envelope` writes one for
  `Read`, `Timeout` and `Write`, so `3`, `4` and `5` keep the exit-code sentence while a code-1
  `Stop` payload with a field beside `ok`, `reason`, `action` and `world` lists those fields where
  it used to name only the exit code. What is left is the arm's last resort. Counted by walking
  every `Failure::ConfigDefect(serde_json::json!({` block under `src/transport/`, tests excluded,
  plus the two `Failure::Stop` reasons `Failure::code` sends to `2`: 41 sites, 16 with an `action`,
  3 answered by `malformed_argument`'s table, and **22 that reach the caller as `run: estigia config
  list`** — `invalid-finding-class`, `missing-argument`, `incomplete-review-transaction` and their
  neighbours, none of which the configuration has any part in. Where the envelope carries a
  scalar-valued field beside the ones the refusal is built from, the message repeats it; a null or
  an object value is dropped, and
  `an_envelope_field_that_is_not_scalar_is_dropped_and_a_scalar_is_repeated` holds both directions.
  The command is still a dead end, and only a per-reason decision about what does clear each one can
  replace it.
- **The operator's local file used to reach the gate and not the transport, and it now reaches both.**
  Kept here because how it was closed is the part worth reading. `load_config` opened
  `SKILL_DIR / "operator.local.md"` and nothing else, while Estigia had renamed that file to
  `estigia.local.md` and said so in the contract it writes; and `_parse_config_block` answered `{}`
  for any document it could not find a marker pair in, while `settings::rows` falls back to the whole
  document. Both are closed in the vendored copy, and **neither is carried upstream yet**.

  Which side moved was the decision. Teaching the gate to read nothing would have agreed just as
  well and would have **loosened** it — no declared boundaries, the widest renewal window, every
  narrowing the operator typed silently dropped. Moving the transport can only let it see rows the
  gate is already enforcing, so that is the direction both fixes went.

  Closing the second half found a third thing nobody had measured: a document whose closing marker
  sits **above** its opening one — which a truncated rewrite produces — satisfied
  `start in text and end in text`, so the transport believed it had found a block and read the empty
  tail of the file. There is no fallback from a block you think you have. The gate answered `linear`
  on that document and the transport answered nothing.
  `a_document_with_no_usable_block_is_read_the_same_by_both` now puts six shapes of unmarked and
  half-written document to both readers, with a floor that fails if the transport reads nothing
  anywhere — because agreement about nothing is what the defect looked like.

  What stands is the risk the fallback inherits: an unmarked document is read whole, so a prose table
  in it is offered as configuration, and nothing comes of that only because no prose row's first cell
  spells a setting. The reader's own comment calls that *luck, not a property*, and
  `no_prose_table_in_the_contract_spells_a_setting` holds the shipped contract to it — measured with
  its closing marker removed, 8 configuration rows become 25, the extra 17 being prose
  (`Situation | Action`, `Load | When`), and the resulting configuration is still identical.
- **Skill directories are verified for four entries.** `claude-code`, `codex`, `deepseek-harness` and
  the neutral `~/.agents` root are known. The other three supported adapters have a verified *instruction* file
  and an unverified skill directory, so their skill lands in the shared root and the directive names
  the path. `estigia setup` says so on the line where it matters.
- **No public Release exists, and `update` does not query a remote in this slice.** Five build targets
  have to be produced before `install.sh` has anything to download. `estigia update` can report only
  local recorded provenance and release high-water; it explicitly reports the latest public release
  as unavailable and does not claim currency from absence. The JSON is not authenticated against a
  malicious same-user writer. Official release installers publish candidate-derived local records
  before replacement, but publication and copy are not atomic and concurrent installers remain
  unserialized. There is no remote update, self-upgrade, rollback, or mutating public upgrade command.
- **The seam tests check the payload against itself**, not against an agent that read it. They
  measure both directions: every markdown file the payload links to is shipped, and every shipped
  runtime companion is reachable from another shipped file through a link or its canonical routed
  identity. These prose-to-prose checks cannot prove that an agent opened or obeyed any document.
- **The retired SDD spellings still load from a table, and neither the setting nor the screen
  remains.** `Planning: sdd*` is retired: the retirement reading recognises the retired
  spellings and reports `planning-protocol-retired` once without storing a planning value; `setup`
  drops the row when it rewrites the block, and an owned `sdd-*.md` is retracted by the retired-names
  list in [`src/skill.rs`](../src/skill.rs). The
  `Planning` row and the planning phase model targets are removed from the interactive surface, and
  the setting, the protocol, the phase assets and the `PlanningPhase` routing
  kind are gone, while keeping that retirement reading in `src/config/settings.rs`. What is **not** measured:
  doctor reads through the folding read, so it sees `direct` and does not surface the report the
  writer prints. The retirement list names `sdd-apply`, `sdd-verify` and `sdd-archive`, which no
  build here installed, because retraction is by ownership and the list is the retired protocol's
  phase vocabulary, not its executor set.

## The gate's reach across the agents

- **`Decision mode: auto` is enforced on Claude Code alone.** The gate refuses `AskUserQuestion`,
  the one question tool its matcher is woken for, and records the recommended answer from the
  payload. Codex, OpenCode and Pi register no question tool with the gate, so on them `auto` is the
  contract's sentence and nothing more — and on every agent a question asked in prose rather than
  through the tool is not seen at all. "Recommended" is the option the agent itself labelled
  `(Recommended)`, or its first option: the gate reads which answer the agent ranked first and does
  not judge it. Measured by `src/harness/decision_mode/tests.rs`; turning the refusal off reddens
  two of its six tests.

- **The harness gates named populations, not every write.** The editing tools each agent ships, the
  git spellings that change the tree, the irreversible steps, and — since 2026-08-01 — the shell
  constructs that visibly write a file: a redirect, a `sed -i`, an interpreter handed code, and the
  utilities that copy, move or delete — including under a prefix that runs them, since `sudo rm -rf src`
  hides nothing and was missed for no better reason than its first word. That list of prefixes had
  been filled to thirteen and not to its own boundary: measured on 2026-08-06, `flock … rm -rf src`,
  `taskset`, `parallel`, `runuser`, `chroot`, `strace`, `ltrace`, `watch`, `systemd-run`,
  `eatmydata`, `unbuffer`, `xvfb-run`, `proxychains`, `torify` and `setarch` each classified as
  nothing at all while `timeout 5 rm` beside them classified as a write — the same shape and the
  same visibility. All fifteen are matched now. What is still not matched is a
  write that shows nothing: an `eval`, a wrapper *script*, a merge reached by an alias or `gh api`.
  The command a segment runs is resolved before any check reads it (`shell::candidate_commands`):
  the reserved words `if`, `then`, `else`, `elif`, `do`, `while`, `until`, `!` and `{`, and any
  `NAME=value` assignment, are skipped, so `if true; then rm -rf src; fi`, `{ rm src/x; }`,
  `! rm x` and `LC_ALL=C rm -rf src` are read as the `rm` they run. Behind a
  `RUNS_ANOTHER_COMMAND` wrapper every later word is read as a head of its own, because which
  wrapper options take an argument is not known, so the interpreter, `-i`, `curl -o`, `dd of=` and
  `find -delete` checks apply to the wrapped command: `timeout 60 bash -c '…'`,
  `sudo sed -i …` and `xargs sh -c '…'` are writes. Keywords not listed (`case`, `select`) are not
  resolved, and a quoted command word is out of scope.
  Since 2026-08-06 a **long write flag** counts whatever the command is — `--write`, `--in-place`,
  `--fix` — because the boundary is what is visible on the line and those are as visible as a
  redirect; they had been read only for the few commands that take `-i`, so `prettier --write src`
  and `eslint --fix .` rewrote every file in a tree and classified as nothing at all. What is
  **still** not matched is a formatter whose default is to rewrite: `cargo fmt`, `black .` and
  `gofmt -w` show no long flag, and the last shows a short one that means *whole word* to `grep`.
  Reading short flags everywhere would report a search as a write, so that gap is left open and
  written down rather than closed by guessing.

  Since 2026-09-07 **`git pull` is matched**, as an `IRREVERSIBLE` boundary and deliberately not as a
  `DELIVERS` one. It runs a merge internally, so a run refused on the first spelling reached the
  identical commit with the second — measured once, where `git pull origin main --no-rebase
  --no-edit` produced merge `3e09d3d` after the merge spelling was refused, and the substitution was
  reported rather than performed quietly. Before the change it classified `Untouched`, which is no
  gate at all; it is now a boundary, so the claim is verified, and it stays out of `DELIVERS` because
  pulling is an integration step rather than a landing one — `DELIVERS` holds the spellings that end
  a delivery — and `stale_verdict` would otherwise refuse the contract's own `integrate before
  publishing` default. Classifying by *direction* instead was
  considered and rejected: merging `origin/main` into a feature branch is an integration but merging
  `origin/<feature>` into `main` is a delivery, so the distinction is not readable from the
  arguments, and a rule that read it from the checkout would demote a real delivery to a routine
  write. The same entry also intercepts `git pull --rebase`, which had been missing
  `REPOSITORY_SHELL`'s rebase fragment: a boundary is stricter than that arm, which is
  conditional on the surface the line names.
  **The merge spelling now integrates where the pull spelling always did.** The two used to differ
  for no reason the contract cares about: `stale_verdict` and `out_of_phase` return early for any
  fragment `DELIVERS` does not hold, so a run refused `verdict-bound-to-other-bytes` on `git merge`
  could integrate with `git pull --no-rebase`. An exact integration merge in the run's own
  checkout (the shapes are in the entry on `stale_verdict` below) spends no verdict either, so the
  asymmetry no longer pushes anybody to the pull spelling. The pull stays out of `DELIVERS` and is
  still read as neither a delivery nor an integration by `stale_verdict`.
  Since 2026-10-04 the fragment and disarm lists, the unclaimed-commit matcher and
  `shell::writes_a_file` read a line whose **provably data** spans are blanked, so a command name
  written as data is no longer adjudicated as a command. Both measured refusals were this shape
  (2026-08-24): a read-only `gh issue view --jq '… | split(…) …'` refused for the `split` inside its
  single-quoted program — that one was `writes_a_file` reading the write verb — and a `cat > file
  <<EOF` whose prose quoted `estigia stand-down` refused for the word in its body.
  **The whole-text scan is the floor, and only a proven span is suppressed.** Eligibility is a
  **grammar the scanner checks byte by byte** — an allowlist, because the denylist it replaced lost
  seven review rounds, each to one more spelling bash reads differently (round 7: `a=/cat sh -c '…'`,
  `</bin/cat sh -c '…'` and `sh${IFS}-c${IFS}'$1'${IFS}x/cat '…'` were read as `cat` lines through a
  basename of the first word). A line is eligible only when all of this holds, and anything else is
  returned whole. The grammar reads the **original-case** line — the classifier and the
  unclaimed-commit matcher lowercase the view it returns, not the text it reads — so a here-document
  terminator is compared byte for byte as bash does, and a head with an upper-case letter (`Cat`,
  `gh Pr`) is not a bare word and makes the line ineligible. Handing it a lowercased copy let `eof`
  close an unquoted here-document that bash left open, and the substitution line after it was read as
  a quoted body and blanked (round 9, measured: `cat <<EOF <<'X'`, `eof`, `$(git push origin main)`,
  `EOF`, `X` classified `Routine` while bash ran the substitution). **The view is built only for a
  single-string command** (round 10, measured): a payload naming several command keys is joined text
  that bash never parses as one script, and a quote opened in one value closed in another and blanked
  a boundary the other carried (`{"command": "echo '", "commandLine": "git push #'"}` and a
  `cat <<'EOF'` / `git push …\nEOF` pair classified `Routine` while the single key is a boundary). A
  joined multi-key text and an argv array are read unblanked, as before the view existed;
  `a_quote_opened_in_one_command_key_does_not_blank_another` holds it:
  - the first word is, **bare**, one of a bounded list of commands that read their arguments and
    cannot run them (`gh`, `cat`, `grep`, `head`, `tail`, `wc`, `jq`, `ls`, `uniq`, `cut`, `tr`,
    `echo`, `date`, `pwd`, `true`, `false`, `basename`, `dirname`, `realpath`, `stat`, `file`,
    `which`, `type`) — `[a-z]+` between blanks, so no path, assignment, redirect, quote, escape or
    expansion can make a word that only looks like the name; for `gh` the second word is bare in the
    list below;
  - outside quotes every byte is an ASCII alphanumeric, a space, a tab or one of `-_./:,=+@%>`, so
    `$`, backtick, backslash, `|;&()<{}*?[]~!#` (`<` except as a here-document operator, below), a
    carriage return and any non-ASCII byte are never read — nothing is a separator, expansion, glob, comment or escape; `>` stays because a redirect
    writes a file and runs nothing, and `shell::writes_a_file` reads the view, in which `>` and its
    unquoted target are never blanked;
  - a quote opens at the start of a word and is followed by a blank or the end of the line, so no word
    is glued from quoted and unquoted parts, and never as a redirect target, so `> '.git/hooks/x'`
    keeps the file it writes visible; `'…'` is data and `"…"` is data when it holds no `$`,
    backtick, backslash or `!`;
  - a here-document is `<<` or `<<-` after a blank, a delimiter of `[A-Za-z0-9_]+` — bare, in single
    or double quotes, or after one backslash — that ends the word, so `<<<`, `<<END-X`, `<<E'OF'`,
    `<<EOF>f` and a delimiter holding a backslash are not read;
  - a body is data under a quoted delimiter and under a bare one holds no `$`, backtick or
    backslash, the terminator is the exact delimiter (leading tabs stripped only for `<<-`), and
    nothing may follow the last terminator, because the line after it is a new command.
  `rg` and `sort` are deliberately absent from the list: `rg --pre` and `sort --compress-program`
  run the program they are handed. So are `printf` and `test`: both are shell builtins that
  evaluate an array subscript, so `printf -v 'a[$(cmd)]'` and `test -v 'a[$(cmd)]'` run `cmd` out of
  a quoted word (found by the round-6 panel).
  **The allowlist was audited head by head in round 7**, and what each was cleared on is:
  `cat`, `head`, `tail`, `wc`, `ls`, `uniq`, `cut`, `tr`, `basename`, `dirname`, `realpath` — external
  programs that read their operands and have no option that runs one; `grep` — `-f` reads patterns,
  no GNU or BSD option runs a program (`rg --pre` is the one that does); `jq` — `env`, `input` and
  `$__loc__` read, and nothing in the language runs a process; `echo` — `-e` interprets escapes and
  evaluates nothing; `true`, `false`, `pwd` — ignore their operands; `date` — `-f` and `-r` read,
  `-s` sets the clock and runs nothing; `stat` and `file` — `-m`, `-f` and `--printf` read or format;
  `which` and `type` — look a name up and do not evaluate it. Removed: `printf` and `test` (above),
  and the `gh` subcommands `repo` and `gist`, whose `clone`, `fork` and `create` forward the words
  after `--` to `git`, where `--upload-pack` and `--config core.sshCommand` run a program; a quoted
  `'--'` forms the same word, so no spelling of the separator can be told apart from data.
  What stays unproved is only what a user's own shell profile or an alias changes about these names.
  `gh` is admitted by an **allowlist of its second word** (`api`, `issue`, `pr`, `run`,
  `release`, `search`, `status`, `label`, `project`, `workflow`), read as it stands in the
  blanked line. An earlier round denied the words `codespace`, `ssh`, `exec`, `alias` and
  `extension` and a panel beat it with spellings the shell forms into them — `gh code\space
  s\sh`, `gh co"de"space`, `$'codespace'` — and a user-defined `gh` alias runs a shell command the
  line never shows. A formed word never equals an allowlisted one, so nothing has to recognise the
  spelling, and `gh browse`, `gh auth` and every alias fall out with the rest.
  **A differential test holds the property against bash itself.**
  `nothing_bash_executes_is_blanked_as_data` runs the inputs earlier rounds measured and 4000
  generated cases (a fixed seed; about six seconds) under `bash --noprofile --norc -c`, and fails
  when a marker command bash executed is not still in `data_view`'s line. A marker is `touch ran_N_`
  in a per-case directory, detected by the file's existence: it holds only bytes inside the grammar,
  where the earlier `echo ran N >&2` carried `&`, so any line with one outside quotes or a body was
  rejected at that byte and the positions after `;`, `#`, `|` and a bare newline were never really
  exercised (the recorded `differential-marker-contains-ampersand` warning). The generator includes head
  words with assignment prefixes, leading redirects, paths, `${IFS}` joins and glued quotes, and
  here-document body lines that are the lower- or upper-case copy of the delimiter. It cannot
  pass vacuously: a timeout fails it, every measured input must run its marker on a bash new enough
  to run it (each carries the version it needs), a floor of executed markers (4500; 4790 measured on the local bash) must be met, and the failure
  names the bash it ran against. It found the double-quoted backslash-newline hole and, once the
  generator learned head words, the round-7 class. It requires `bash`, found on the inherited `PATH`
  and then `/usr/bin` and `/bin`, and fails without it. The versions: `printf -v` subscripts 4.0, `test -v` 4.2, `${ cmd; }` 5.3. The local bash 3.2 was
  measured to reject all three, so it cannot see them — the unit tests carry those, and the version
  numbers are unmeasured here; a newer bash proves them.
  The text of a `#` comment is never blanked, so a verb in a comment is still read.
  Every other line — a pipeline, a separator, an interpreter (`sh`, `bash`, `dash`, `python`, `php`,
  `lua`), a wrapper, an assignment — is returned unchanged, so the scan that caught it before still
  catches it. A
  corpus of the executor spellings two blind panels found — `echo 'git push' | sh`, `printf '…' |
  bash`, `sh <<EOF`, `bash <<'EOF'`, `bash <<< '…'`, `bash -lc '…'`, `dash -c '…'`, `php -r`,
  `lua -e`, `eval '…'`, `ssh host '…'`, `"estigia" stand-down`, `git config "core.hooksPath"
  /dev/null` — asserts each is still a boundary; it was red against the masking approach before this
  one, which is why the allowlist replaced it. What is **not** closed: an unquoted argument, so
  `echo git push` still reads as a boundary — the suppression is drawn at quoted data and
  here-document bodies, not at prose a person typed without quotes.
  The choice is **subtract only proven data, never subtract a whole class of quotes**: an earlier
  attempt blanked every quoted region an interpreter was not provably handed, and a panel found its
  boundaries were themselves holes — the quote in the `echo` segment of `echo 'git push' | sh`, a
  here-document body handed to `sh`, a combined flag (`bash -lc`) the code-flag table did not list.
  The same change drops `git tag --list=<pattern>` from the listing exemption: `--list` takes no
  value (`error: option 'list' takes no value`, measured 2026-10-04), so that spelling stays a
  boundary and the `irreversible-shell` declaration names every listing it exempts. A quoted `-l` or
  `--list` still lists because `git` is not in the allowlist, so `git tag '--list'` is read whole and
  the flag reader sees the flag; no separate quoted-flag view is needed.
  **A quoted command word is read as the word it spells.** Bash removes quotes before it
  runs a word, so `git 'push' origin main`, `gh pr 'merge' 7` and `estigia config 'set' …` run the
  unquoted command, and used to classify as nothing while `gh pr 'merge'` was worse: the data view
  blanked the verb itself. `shell::unquote_plain_words` now runs once, before every matcher and
  before `data_view`: a quoted run holding only `[A-Za-z0-9_\-./:,@%+]` loses its quotes, wherever it
  sits in a word (`pu'sh'`, `"gi"t`, `$'push'`), including inside `$(…)`, backticks and double
  quotes, where the text still runs. A run holding a blank, an expansion, a backslash or a glob byte
  keeps its quotes and stays data, a here-document delimiter keeps its quotes, and a quote that never
  closes leaves the whole line as it was. `read_only_spans` also refuses a `gh` line whose
  subcommand is followed by a quoted word, so a verb is never blanked. Not read: a backslash-escaped
  quote (`` `git \"push\"` `` inside double quotes), a word spelled through a variable, `$(…)`, an
  alias or a script, and a quoted word whose content is not plain (`git "pu\sh"`).
  **What exact-literal matching leaves is enumerated rather than implied**, because it guarantees
  something always is. Traced by reading the six matchers on 2026-09-07 and not by running them.
  *Not matched at all:* a backslash line continuation survives, because the
  shell strips continuations and the literal text does not match (a quoted verb is read now); a `.exe` suffix and a full path
  carrying a global option survive, because `.exe` is stripped only for `estigia` and
  `without_leading_options` compares the word `git` exactly; `git update-ref refs/tags/…` creates a
  tag without the tag verb; `gh release upload`, `delete` and `delete-asset` are in no list at all;
  `git send-pack` and `gh repo sync` write remote refs without the push verb; and
  `git branch <name>`, `git checkout HEAD -- <path>`, `git checkout -f` and `git filter-repo` reach
  what `REPOSITORY_SHELL` names by other spellings.
  *Matched, but never read as a delivery:* `gh api` against the merge, release and tag-ref
  endpoints, and the pull spelling this entry adds — both reach a boundary, and `stale_verdict` and
  `out_of_phase` read neither. Two further families are filed here rather than declared. The first
  is two `estigia config` subcommands that write what the disarm list watches, filed as unwatched; both are matched now, and the disarm paragraph below this one says what the crossing
  holds. The other family, `su -c`/`script -c`, is covered: both names joined
  `INLINE_CODE`, so the code flag reads their quoted command. **BSD `script` runs the words after
  the optional typescript file instead, and that spelling is covered**: `script` joined
  `RUNS_ANOTHER_COMMAND` too, whose reading takes any later word that names a write, so
  `script /dev/null rm -rf src` — measured running the deletion on darwin, where `-c` is an
  illegal option — is a write now. What that buys costs one tracker read in the direction the
  module always picks: a typescript file named after a write reads as `… under script`.
  **`rg --pre COMMAND` is the same shape spelled as a flag, and it is covered**: `rg`
  alone searches, so it is not on `RUNS_ANOTHER_COMMAND`, but `--pre`/`--pre=` names a program the
  line runs over every file it searches. Such a line is a write, and it is placed **inside**
  however it redirects — `rg --pre /bin/rm x > /tmp/out` is held, not passed by its redirect to
  `/tmp/out` — because the program's target is not on the line to be bounded. `rg pattern src` and
  `rg pattern src > /tmp/out` keep their previous reading. The flag spelling is matched by
  `shell::runs_a_program_after_pre`, read by `writes_a_file` and by the placement in
  `issue_requirement::shell_write`; `--pre-glob` takes a glob, not a program, and is not one.
  Since 2026-08-04 the gate also measures **its own disarmament**: `estigia stand-down`,
  `guard --uninstall` and `uninstall` classified as nothing at all, so the one thing the whole model
  rests on — that an agent cannot switch the gate off — was reachable through the tool it uses most.
  It also measures writes to the files those decisions are read from — its own state, the
  contract, and the entry in each agent's settings — because closing the road through the shell
  left the shorter one open. Since 2026-09-27 the list holds the sibling subcommands too:
  between them `estigia config edit` and `estigia config forget` cover the same contract and
  repository rows as the listed `estigia config set` — `edit` the installed contract, `forget` the
  repository's own file — and neither matched any population, so the gate answered
  `Untouched` — `Outside(NotWatched)`, no gate at all rather than a weaker one. The crossing that
  reads the parser's own tree now holds every leaf below a verb the list names to that list or to
  `DISARM_READS`, which declares the two subcommands that only read (`config list`, `config repos`),
  so a third writing subcommand fails a test instead of arriving unclassified. Since 2026-10-05 the
  leading-option normalisation reaches Estigia's own verbs too: `without_leading_options` moves a
  global option out from between the binary and the verb, so `estigia --json config edit` carries
  the listed `estigia config edit` fragment and is refused where the plain spelling is — before that
  the flag broke the contiguous substring and the line answered `Untouched`. The walk recognises the
  binary by its basename, so a path prefix and a `.exe` suffix are folded here the same way
  `as_estigia` folds them for the match itself. Since 2026-10-06 declaring a stand-down is **a
  person's act, not an agent's**, and since 2026-10-07 that covers the record as well as the command.
  `estigia stand-down` stays on the disarm list, but the classifier reads it first
  (`harness::person_only`) and the gate refuses an agent's shell line that runs it with
  `stand-down-is-for-a-person`, on every host and for a sub-agent as for the orchestrator. A write
  that names `~/.estigia/stand-down.json` (also with `"`, `'` or `\` inside the name) is refused with the same code: a file tool (`Write`,
  `Edit`, `MultiEdit`, `NotebookEdit`, an `apply_patch` or `patch` body) and a shell line that
  writes a file and names the record (a redirect, `tee`, `cp`, `mv`, `rm`). The name is anchored
  at both ends, so a backup such as `stand-down.json.bak` is a different file; and a shell line
  that names the control directory and writes the record under its bare name is the record too, so
  `cd ~/.estigia && echo x > stand-down.json` is refused even though the two halves never join
  into one path. Deleting the record is
  refused too, because a line that deletes it cannot be told from one that also writes it, so
  the person lifts with
  `estigia stand-down --lift --reason <why>` in their own shell. An agent's lift is refused as well,
  because the gate cannot tell a lift from a declaration that only quotes the flag (`--rea\son=,
  #--lift` declares): a line carrying `--lift` anywhere is refused with the same code and a message
  whose way out is the lift command; `doctor` names the same lift as an operator-knowledge
  resolution carrying the command for the person to run, never as a runnable one, because an agent
  is refused it. The rule is
  decided in one function from the classified action, called first by `gate`, `gate_observing`,
  the push guard, and the role road in the hook and in `estigia gate`, so no stand-down, claim,
  refusal on the same line, or unreadable sub-agent definition lifts or replaces it, and each
  refusal is in the ledger, on the hook road always and on `estigia gate` when the call carries a
  run id. A stand-down a person declares in their own shell, or with `!` in
  Claude Code, still lifts every other refusal and `ask`; it never lifts this one. What stays
  open, by declaration: a spelling that never writes `estigia stand-down` as two words (an alias,
  a wrapper script, `cargo run -- stand-down`, a renamed binary), a write to the record the shell
  reader does not recognise as a write (an interpreter's `open(...)`), a glob (`stand-*.json`) or a
  directory target (`cp x ~/.estigia/`) that never spells the file name, a directory change in a
  separate call, one whose target is held in a variable, or one the matcher does not read (`popd`,
  which returns to the stack top) while a later line writes the bare name, and any name the shell
  expands before the reader sees it (`stand-$'down'.json`, `${x:-stand-down.json}`), and a name split
  across a line continuation, which the shell joins. The match
  is broad on purpose, and its over-reach is a refusal with no lift: a commit message, `rg`,
  `git grep` or `git log --grep` that names `estigia` and then `stand-down` is refused, and so is
  a patch body that merely names the record, and a line that reads the record with `cat` beside any
  other write. A plain `cat` of the record, and the `Read` tool, stay allowed. `estigia guard --uninstall`,
  `uninstall`, `config set|edit|forget`, `setup`, `install` and `tui` remain refusals a person's
  stand-down can lift and an agent can no longer declare one; they stay ordinary disarm-list gates
  until somebody decides they are person-only too. What that crossing
  still does not reach is a new **top-level** verb: it walks the subcommand tree of a verb the list
  already names, and a verb the list does not name is a review decision rather than a failing test.
  Twenty `Population` consts in the source (`src/harness/population.rs`
  holds the type) name each legitimate population and what falls
  outside it, and that number is checked here.
- **An issue is refused from a shell line, and the template is checked in one place.** `create`
  refuses a body whose first `##` heading outside a code fence is not `## Description for dumb
  humans` followed by a `> [!NOTE]` alert (prose before that heading is accepted), or that lacks a required heading, or has them out of order, before it calls `gh` at all. The
  required headings are the `##` lines of the fenced block in `skill/assets/analyst-issue-template.md`,
  extracted by `build.rs`, so the file is the only list. Headings inside a code fence in the body
  do not count, and headings beyond the required ones are allowed. `gh issue create` and its alias
  `gh issue new`, `gh api` posting to an `/issues` path, and a GraphQL `createIssue` mutation are
  refused with `issue-creation-outside-flow` whoever holds the checkout, naming the `create` tool and its `repo` argument, which files in another repository.
  The matching is on substrings of the command line, with options allowed between `gh`, `issue`
  and `create`, so a line that merely mentions the command is refused too, `gh` after a wrapper
  such as `sudo` is matched, and an alias or a script that never spells `gh` on the line still
  files an issue nothing checks. A `gh api` call is read from its own words, per command of the line (a separator inside quotes does not split it, a substitution inside double
  quotes does, and a line whose quotes do not balance is split on every separator), so
  an unrelated `-f` on another command does not make it a post and an explicit `GET` with fields
  is a read in every spelling. The read exemption that remains knows only `-X GET` and
  `--method GET` and matches them anywhere on the line, so a post beside a `-X GET` read
  (`gh api -X GET a; gh api repos/o/r/issues -f t=x`) still files an issue unchecked. The check is structural: it reads headings
  and the alert, not the prose beneath them. The template's `###` sub-sections (`### Today`,
  `### Expected`, `### Pitfalls`, `### Size`, `### Rules that apply`) are prose guidance and are
  not enforced: a body that omits them is accepted, whereas `## Where to start` and `## How to
  verify` are `##` headings and so required. It is held only by `create`, so an issue filed in
  the web interface or by a token outside this harness is unchecked as well.
- **A delivery is refused in a state that holds no verdict.** Holding an issue and being allowed to
  land it were one question, and only the first was ever put: a run in `in-progress` that ran
  `gh pr merge` was told yes, because the tracker confirmed the claim and nobody asked what the
  claim was *for*. `git merge`, `git tag`, `gh pr merge` and the two `gh release` steps now need the
  issue to be in `review` or `done`. `git push` and `gh pr create` are deliberately exempt —
  publishing is how a run reaches review, and gating it on review is a deadlock. Boundaries the
  operator declared are never read as deliveries: Estigia cannot know whether a `make deploy`
  delivers or rehearses, and guessing would refuse a step it never understood.

  Two `git merge` shapes are preparation rather than delivery, not one — the second, an integration in
  the run's own checkout, is its own entry further down this file and is not repeated here. This entry
  is the first and narrower of the two: an exact literal `git merge --ff-only
  <target>` (also accepting `--` before the target) may run from `in-progress` after the tracker
  agrees when local Git proves all of it. The checkout is on a branch, its upstream is a canonical
  `refs/remotes/*` ref, the worktree is clean, the target is that upstream's exact short or full
  canonical name, or a full object ID of the length this repository's object format requires, and
  ancestry runs from `HEAD` through the target to the upstream. An object ID must resolve to itself
  as a commit; an annotated tag's ID does not qualify merely because it peels to one. Every proof Git
  process removes every inherited `GIT_*` variable, and the exception is denied when any such
  variable was present at entry. It is also denied when `BASH_ENV`, `ENV`, or Bash's exported `git`
  function name (`BASH_FUNC_git%%`) is present, compared without case so Windows environment names
  cannot evade the check. Those variables can make the shell execute a different `git` from the
  direct process the proof resolved; `PATH` remains eligible because both resolve through the same
  inherited path when no shell-only steering is present. The `GIT_*` check prevents the named
  repository, object-store, index, shallow-file and configuration steering, while the three shell
  checks prevent those specific initialization and inherited-function paths from substituting a
  different command. It remains a `Boundary`, so it still pays the live tracker read and is recorded
  in the decision ledger. The
  verdict-to-bytes check runs first; a stale verdict is refused before this exception can skip only
  the `out-of-phase` check, and only when the tracker answered exactly `in-progress`. `analysis`,
  `ready` and `blocked` keep the ordinary refusal. Wrappers, compound commands, duplicate command
  aliases, quoting, expansion, alternate git directories, extra flags or targets, detached branches,
  dirty trees and any unreadable Git answer all retain the existing refusal here, under this
  exemption's own local-Git proofs. That does not carry over whole to the second exemption, the
  integration merge covered further down this file: its literal reader, `exact_integration_merge`,
  spawns no process, so it independently refuses wrappers, compound commands, duplicate command
  aliases, quoting, expansion, alternate git directories, and extra flags or targets on the command
  text alone. A detached branch is refused there too, but not by that reader — by
  `is_integration_in_the_runs_own_checkout`'s separate `symbolic-ref` check. An unreadable Git
  answer is not something a reader that spawns no process can produce or see. A dirty tree is
  refused nowhere on the integration path, and that absence is deliberate: `--continue` only ever
  runs against a mid-merge tree, which a cleanliness check would refuse along with everything else,
  leaving the exemption with no case left to grant.
  `a_conflicting_integration_is_resolved_without_leaving_in_progress` in `tests/pipe/part_04.rs` stages
  exactly that tree — a resolved conflict, mid-merge — and gates `git merge --continue` expecting it
  allowed. What stands in the cleanliness check's place is git's own refusal to run a merge that
  would overwrite local changes it was not asked to discard.

  **The proof is local and can therefore be stale.** It never fetches and cannot establish that the
  remote-tracking ref still matches the server. It proves only that the merge is safe relative to
  the tracking ref already present in this checkout; a later fetch may reveal commits it did not
  know about. Avoiding network access keeps the gate from changing repository state or turning every
  local update into another remote boundary, and this limitation is recorded rather than hidden.

  **The proof and the merge are not one atomic operation.** Attachment, upstream, cleanliness,
  object resolution and ancestry are separate local Git processes, and the shell resolves and runs
  the merge only after the gate returns. Another process can move a ref, change the worktree, or
  replace what `PATH` resolves between any two of those reads, or between the final read and
  execution. `--ff-only` still prevents a merge commit, but Estigia takes no repository or process
  resolution lock and cannot claim the executed command saw the exact state or executable it proved.
  The decision ledger has the same pre-existing resolution limit: its allow line names the
  subject as `git merge` and the live claim, not whether this local proof or an ordinary reviewed
  delivery admitted it. Changing that shared allow vocabulary would reach every gate consumer, so
  this narrow exception records the residual ambiguity here instead.
- **That narrowing is not configurable, and the asymmetry is the reason.** Every other axis here can
  be switched; this one only tightens, and a setting that could loosen a guard rail turns it into a
  preference. It is the same rule as the operator's boundary list, which adds and never removes.
- **Closing the shell hole changed a declaration that had been wrong.** `repository-shell` claimed
  "every spelling of a repository write reachable from a shell" while matching only spellings that
  name git. A redirect writes the same tree and was never in reach of that list. The claim is now
  split in two, because the two halves have two different boundaries.
- **Nothing here has driven a real agent.** The hook protocol is exercised by feeding the binary the
  payloads an agent sends and reading what it prints. That proves Estigia answers correctly; it does
  not prove Claude Code asks.
- **The tool-call gate reaches every agent Estigia knows but two.** Five agents, in four shapes,
  each taken from that agent's published reference or its source: Claude Code and Codex
  (`PreToolUse` → `permissionDecision`), Cursor (`preToolUse` → `{"permission":"deny",…}`),
  OpenCode (a plugin that blocks by throwing) and Pi (an extension that blocks by returning
  `{ block: true, reason }`). The two it does not reach are ZCode and DeepSeek
  Harness, and the bullets on them below say why. The eighth entry is the agent-neutral skills root,
  which is not an agent and has no tool loop to gate.
  The gate also still answers for six retired hosts — Gemini CLI, Continue, Cline, Crush, Windsurf
  and Qwen — because a machine that installed one before it was retired still carries its hook
  and dialect; `setup` installs into none of them and `uninstall` is the only path that touches them.
- **No live Pi turn has run the gate; the rest is read from the installed Pi 1.1.0 package.** The Pi
  extension is measured under `node` against a stand-in binary: the arguments it hands
  `estigia gate`, the working directory, the three outcomes and the `{ block: true, reason }` it
  returns are asserted. Against `@earendil-works/pi-coding-agent` 1.1.0 itself, the `.d.ts` and
  tool sources in its `dist` give `ToolCallEventResult`, `ExtensionFactory`, `ctx.cwd`,
  `ctx.sessionManager.getSessionId()` and the `bash`, `edit` and `write` argument names, and Pi's own
  `loadExtensions` (its jiti loader) was run once by hand on the generated file: it loaded with no
  error, registered one `tool_call` handler, and that handler, driven with a `write` event and the real
  binary, returned `{ block: true, reason }`. That check is not in the suite, which does not depend on
  Pi being installed. What no check here has done is run a model turn, so that Pi **acts** on the
  returned `block` while a model calls a tool, and awaits an async handler there, is read from the
  types and not observed. The built-in MCP client the `mcp.json` entry depends on exists from Pi
  0.99.0; an older Pi ignores the file while `status` reports the tools registered. The entry's
  `exposure: "direct"` is read from Pi's `docs/mcp.md` ("Control tool exposure"), not observed: no live
  Pi has connected to the server and declared its tools to a model. An older Pi that does not know the
  field is not known to ignore it rather than reject the entry.
- **`deferred` exposure was measured with one model and one run per prompt, and setup reads the Pi version from
  `pi --version`.** On Pi 1.2.0 with `opencode-go/deepseek-v4.1-flash`, a first request cost 3,255 input tokens with
  `deferred` against 13,868 with `direct` and 2,986 with the server off, and the model reached
  `mcp__estigia__*` tools through `tool_search` for an explicit prompt (8,720 input tokens in all) and an
  implicit one (67,063). Those are single runs, turn counts varied between 2 and 15, and no other model was
  tried. What is not measured: a real flow (claim, task, transition, release) under `deferred`, where each
  `tool_search` result stays declared on the branch and the cost after several loads is the open number. The
  version gate is a comparison against 1.2.0 of the first `x.y.z` in `pi --version`'s output; a pre-release
  suffix is dropped, so `1.2.0-beta.1` reads as 1.2.0. A `pi` that is not on the `PATH` of the process
  running `setup`, or prints no version, gets `direct`, and the Pi that actually runs may be another one. The
  entry is written when `setup` runs, so a Pi upgraded afterwards keeps `direct` until the next setup, which
  is what the `pi-config` warning names. Pi 1.2.0 itself was not installed on the machine this was built on
  (1.1.0 was); the 1.2.0 behaviour is read from the issue's measurements, not re-observed here.
- **No live Pi turn has run the review relay, and no model completion has been requested through it.**
  The relay is measured under `node` against a stand-in binary and a stubbed `ctx.modelRegistry`: the
  arguments it hands `estigia review prompt`, `model` and `admit`, the prompt it submits verbatim as the
  one user message, the model it chooses, the raw answer it pipes into `admit`, and the refusal codes it
  keeps apart (`authority_unavailable`, `capture-result-stale`, `transport-refused`, and the
  `reviewer-*` codes for a completion that failed) are asserted. So are the paths a failed completion
  takes: a `streamSimple` that is still running when the deadline passes (aborted by the relay's
  timeout) and one that returns after it, each `reviewer-deadline` with nothing admitted, driven with a
  100 ms deadline in place of the default; a result whose `stopReason` is `aborted`; and a registry with no
  `streamSimple`. The argv without `--session-id` and `--runtime` for a blank or throwing `getSessionId`
  and the refusal of an `inherit` row with no `ctx.model` are asserted too, and the `estigia review model`
  text a relay reads (the bare model, or nothing) by a pipe test against a seeded relay job. Against `@earendil-works/pi-coding-agent`
  1.1.0, Pi's own `loadExtensions` was run once by hand on the generated file in a temporary home: it
  loaded with no error and registered `estigia_review_lens`, and the tool's plain JSON-schema
  `parameters` passed Pi's own `validateToolArguments` for a listed name and failed it for an unlisted
  or missing one. That check is not in the suite. Not observed: a model calling the tool, a real
  `streamSimple` call returning an `AssistantMessage` — its shape, `stopReason` and `content` are read
  from the `.d.ts` files of the installed `ModelRegistry` and `pi-ai` — a provider honouring the
  extension's `AbortSignal`, and an extension-registered provider answering through the registry. A
  completion is never requested by a check here, because it would spend the operator's credentials.
- **No live Pi turn has run the loop extension; the events and calls it uses are read from the installed
  Pi 1.1.0 package.** The extension is measured under `node` against a stand-in binary and a fake `pi`:
  the arguments handed to `estigia goal`, the continuation it sends, the waits, the retries, the stops
  and the refusals to act (a changed session id, a child session, an aborted or busy turn) are asserted.
  From Pi's `dist`: `agent_settled` exists in 1.1.0 and is emitted from `AgentSession._emitAgentSettled`
  after retries, compaction and queued work, and `pi.sendMessage` with `triggerTurn` called while that
  event is being emitted is deferred until the emission ends; `registerCommand`'s handler takes
  `(args, ctx)`; `ctx.isIdle()` and `ctx.ui.notify` exist; a custom message reaches the model as a user
  message. Not observed: a model turn actually started by that continuation, Pi's behaviour on a
  `sendMessage` after the session was replaced, and `agent_settled` firing in print and RPC modes beyond
  the source's `emit`. Pi 1.1.0 has no sub-agent concept, so "child session" is a heuristic —
  only a `parentSession` in the session header marks one, and a child started without that header is not
  recognised as a child. A loop only
  exists where `/flow-loop` was typed, which is what keeps a child from running one in practice.
- **The Pi subagent runner has been driven against a stand-in `pi`, never a live one, and it is the first
  slice of the issue.** Reproduced first: each of the four runner tests fails when the piece it names
  is removed. Spawning the child without the parent's environment, zeroing the tokens, the cost or the
  elapsed time on the card, dropping the refusal lines, making the stop a no-op, skipping the check that
  the child loaded the gate, and skipping the check that the parent session was exported each turn one of
  them red. The stand-in `pi` speaks only the RPC records the runner reads (`get_commands`, `get_state`,
  `prompt`, then `tool_execution_*`, `message_update`, `message_end`, `agent_settled`) and loads the real
  generated gate extension, with a logging stand-in for the binary, so what is measured is the argument
  list the child's gate call carries. Not measured: a real Pi child, Pi's own RPC event shapes beyond the
  documented ones, whether Pi's `setWidget` accepts the component the runner returns, a UI at all (the
  tests pass a `ctx.ui` that records), a child that is slow to answer `get_commands`, the stall
  watchdog (there is none: a child that never settles runs until stopped, and only the thirty-second
  deadline for its first answer is enforced), and Windows (the runner kills the child, not its group,
  and the tests are Unix-only because the stand-in is an executable script). The card truncates by code
  units, not columns, so a wide character can overflow a row. The check that the child
  loaded the gate starts the process and sends it one command before the prompt; it is not a refusal to
  launch. A child that lists `estigia-doctor` from another extension passes it. The tool is named
  `estigia_subagent`, not `subagent`, so it cannot replace another runner's tool of that name, which means the
  issue's wording is not met literally.
- **Pi parent and child messages were run live twice, both on one host and by hand.** On 2026-10-15, with Pi
  1.1.0 as both parent and child:
  - The first run used a foreground `estigia_subagent`. The IPC channel and the child tool worked, and the run
    showed that the parent could not reply while it waited on that child.
  - The second run used `background: true`. The child's query woke the parent, the parent's
    `estigia_subagent_reply` came back as the child's tool result, and the child's final answer reached the
    parent as a follow-up, all within 15 seconds.

  Not run live:
  - the immediate refusal of a foreground query;
  - steering through `estigia_subagent_message`;
  - a query that times out after the background fix;
  - a child launched under an agent definition that restricts `--tools`.
- **A reply can still be reported as answered after its child gave up.** When a child's query times out, the
  child sends a withdraw frame and the parent forgets the query. Two windows remain. One is a reply that crosses
  the withdraw in flight. The other is a withdraw that never arrives, in which case the parent's own expiry
  (`WITHDRAW_GRACE_MS`, five seconds after the timeout) is all that closes it. In either window
  `estigia_subagent_reply` answers `answered`, although the child never receives the text. No test covers
  either window, a failed withdraw send, or a withdraw for an unknown request.
- **Pi parent and child messages were first driven against a stand-in `pi` that is a Node script.** Reproduced first, by mutation: removing the running check on the steer, the
  `triggerTurn` option, the deletion of an answered request, the query timeout, the condition that registers
  the child-side tool, the answer text in the child's result and the steer send each turn exactly the
  test that names them red. Not measured: that a real `pi` is a Node program and so inherits the IPC
  channel, that it passes an extension tool's thrown error back to the model as a tool error, that
  `--tools` in an agent definition leaves an extension tool available, that `sendMessage` with
  `deliverAs: "followUp"` really queues at a turn boundary and wakes an idle session (the stand-in records
  the call and its options only), and any ordering between two children. At-most-once: a notification sent
  while the parent is gone is lost without the child being told after the send call returns. The query
  timeout, 120 seconds, is a constant, not a setting.
- **The Pi subagent view, steering and history have been driven against recorded stand-ins for Pi's UI and
  session API, never a live Pi.** Reproduced first: each of the four view tests and the three `doctor`
  and setup tests fails when its piece is removed. Dropping the transcript, the `steer` send, the stop
  confirmation, the `appendEntry` call, the read-back of history on `session_start`, the
  `doctor` naming of Estigia's own runner, the `pi-config` exclusion of `estigia-agents.ts`
  and the setup note each turn one red; a `sendMessage` added beside `appendEntry` turns the history test red,
  which is the whole of "never sent to the model" as measured: the test records every method the runner
  calls on the host and allows only registering and `appendEntry`. Not measured: that `ctx.ui.custom`
  with `overlay: true` opens an overlay in a real Pi, that the component's `render` and `handleInput` match
  what Pi's TUI calls (the key bytes for the arrows are the usual ANSI ones, not read from Pi), that
  `registerShortcut("ctrl+alt+a")` is free in a real session, that `getEntries()` returns entries as
  `{ type: "custom", customType, data }` and that Pi keeps them out of the model's context (the
  runner's part, calling only `appendEntry`, is measured; Pi's is not), that RPC `steer` is accepted by a
  real child (the stand-in only records it, and a steer the child refuses is not shown), and any
  rendering of the overlay on a screen. The transcript is only what the runner saw in events, capped at
  two hundred lines of two thousand characters, so a child's thinking and long tool output are cut. A
  resumed session lists earlier children but cannot stop or steer them. The view's row ids restart at
  `a1` each process, so a restored entry can share an id with a live one. `doctor` and setup find
  another runner by the word `subagent` in its name, as before, so a runner called otherwise is not
  named; the row stays fine, not a warning, when Estigia's own runner sits beside a gated one. The setup
  note is advisory. A step the issue names that is still not met literally: the tool is `estigia_subagent`,
  not `subagent`.
- **A Pi child is linked by its session header, with the pid as the fallback; the live check ran without
  a claim and under `-p` only.** Reproduced first: an in-process child (the parent's pid, a header
  naming its parent) was decided as an unrelated session and overwrote the exported root, so a later
  separate-process child inherited the wrong parent; `the_pi_extension_links_a_child_by_its_parent_header_whatever_its_pid`
  fails against that code. The id of the parent is the exported root, else the `id` in the parent's
  session file, else the part of its file name after the first `_`; none of the three is
  authenticated, as with the flag. The live check, 2026-10-14, in an isolated Pi home: a child `pi`
  started by Pi's `subagent` example was another process (parent pid 43006, child pid 55301), and every
  `write` and `bash` from it reached the gate with `--parent-session-id` equal to the parent's session.
  The write was refused `issue-required` because no claim was held, so what a child is *allowed* under a
  held claim was not observed there; and it ran under `-p` only, never in the terminal UI or RPC mode.
  A forked or new session also carries a `parentSession`, but Pi starts it with the reason `new`,
  `fork`, `resume` or `reload`, and the extension reads a session so started as a root whatever its
  header says; a child that began at `startup` is linked by the header. Only the start reason
  separates the two, and a replacement session started inside a separate child process is still
  linked by the pid. The `pi-subagents` row of `doctor` reads names and source text, not behaviour: a
  runner is recognised only when `subagent` is in its package source or extension name, its children
  are judged by whether its source mentions `--no-extensions` or an in-process session constructor,
  and it is called gating only when every `.ts`/`.js`/`.mjs`/`.cjs` file within three directory
  levels was read inside a 1 MiB budget and a `spawn(`, `execFile(` or `spawnSync(` call visibly
  takes `pi` (or a path ending in `/pi`) as a string literal first argument; a command held in a
  variable, a file skipped for size or depth, or a spawn of anything else is answered unknown. An
  npm or git package is unread because the row does not know where Pi unpacks it (only an absolute local
  path and the extension directories are read), and whether a runner replaces the environment its
  child inherits is not examined. The next entry is the earlier measurement.
- **A Pi subagent is linked to its parent by an environment variable and an unauthenticated flag; no
  live Pi child has run it.** Reproduced first: a child `--session-id CHILD --runtime pi`
  writing in a claimed worktree is answered `outside` / `nothing-sworn`, because its derived run holds no
  issue (`a_pi_child_is_decided_as_its_parent_run_when_it_names_the_parent_session` pins that row, then
  the same call with `--parent-session-id` refused exactly as the parent's own write is). What is
  measured is the extension under `node`, including a real second `node` process spawned with the default
  environment seeing the exported root. What is read, not run: that Pi's subagent example spawns `pi`
  without an `env` option, so Node hands the child `process.env` as it is at spawn time (read from
  `examples/extensions/subagent/index.ts` in 1.1.0), and that every extension shares one process, so
  the mutation made by this extension is visible to the one that spawns. A subagent extension that
  passes its own `env` without the two variables is unlinked and unmeasured as before. The flag is an
  assertion the binary cannot check, like `--run-id`: whoever can run `estigia gate` can name any
  session. A parent that names no run gets no fallback to the child's own, so the child then receives
  the parent's answer, `nothing-sworn` included. The child's own session id is not recorded: the role
  attribution `agent_type` drives takes a real agent definition, and a made-up label would be refused as
  an unknown one. A root session that switches (`/new`, `/resume`) re-exports its new id, so a child
  spawned afterwards names the new session; one spawned before keeps the old one.
- **No live Pi has drawn the claim status or the refusal card; both are read from the installed Pi
  1.1.0 package.** Measured under `node` against a stand-in binary and a fake `pi`: the
  arguments of the `goal` call, that a held claim, a stopped one, none, a signal, an exit code, unparsable
  output and a call past the bound each produce the status text or the clear the code says, that a
  child asks about the root session, that nothing is shown without `ctx.hasUI`, and that a refusal
  appends one entry carrying the reason byte for byte and sends no message. Read, not run:
  `ExtensionUIContext.setStatus`, `ExtensionAPI.appendEntry` and `registerEntryRenderer` in
  `dist/core/extensions/types.d.ts`; `convertToLlm` in `dist/core/messages.js` turning a `custom`
  message into a user message regardless of `display` (the reason the card is an entry);
  `CustomEntryComponent` and `addCustomEntryToChat` in `dist/modes/interactive` drawing a renderer's
  component on `entry_appended`; `Component` as `render(width)` plus `invalidate()`. Not observed: the
  footer showing the line, the card on screen, how it looks in a narrow terminal or under a theme, and
  `appendEntry` called from inside a `tool_call` handler during a streaming turn. The issue text names
  `registerMessageRenderer`; that API draws `sendMessage` messages, which reach the model, so it was
  not used. Print and RPC modes have no UI: no status and no card there, and the model still gets the
  reason as the block result. The status costs one `estigia goal` process per refresh, and `goal`
  reads the tracker to say whether a claim is live, so a slow tracker shows the claim as unknown after
  five seconds.
- **No live Pi has run `/estigia-doctor` or expanded the prompt templates; both are read from the
  installed Pi 1.1.0 package.** Measured under `node` against a stand-in binary and a fake
  `pi`: the one `doctor` call and its working directory, that the report arrives as one entry carrying
  the binary's text unchanged and sends no message, that a nonzero exit with output still shows it,
  that a missing binary, a signal, a call past the bound and a silent exit each say `did not answer`
  rather than show a report, and that no UI means standard output. Setup is measured for the two
  templates' bytes, ownership, no-op rerun, removal and the relocated directory. Read, not run:
  `registerCommand` in `dist/core/extensions/types.d.ts` and `docs/extensions.md`, and
  `docs/prompt-templates.md` for the directory, the frontmatter keys and `$ARGUMENTS`. Not observed: the
  command in the completion list, the entry drawn on screen (`appendEntry` called from a command handler
  rather than an event), `doctor` run from Pi's working directory taking less than its bound, Pi
  expanding the templates, and whether the MCP tool is listed to the model under the bare name `claim`
  or one a Pi MCP adapter prefixes, which the templates assume. `ctx.ui.notify` is a single notification
  and was not used for the report. The templates' marker is sent to the model with the body.
- **The Pi loop forgets its goal when Pi restarts, and `/flow` is not expanded.** The goal is held in the
  extension's memory (OpenCode keeps it in its plugin storage), so a restart or a switched session starts
  with no loop and `/flow-status` says nothing is stored; the run's claim and `stop-after` request live in
  the store and are unaffected. The objective is sent as a custom message in the shape OpenCode's slash
  text has (`/flow dev`); Pi names skills `/skill:flow`, and a custom message is not expanded, so whether
  the model treats the text as a request to load the `flow` skill is not measured. A timer that fires
  after the session was replaced finds the session id changed and stops the loop without notifying,
  because the stale context is not touched.
- **No live Pi has received the session context, and a replaced session's pointer outlives it.** Reproduced first: the generated extension never ran `estigia session`, so a Pi turn began
  without the run id, the held issue or the `Issue requirement` note a Claude Code session is told. Now
  measured under `node` against a stand-in binary: the arguments of `session start` with the reason and
  the previous session id read from the first line of `previousSessionFile`, the `sections.estigia`
  assignment beside a section it must keep and no returned `systemPrompt`, a non-zero exit, a signal
  and a call past the bound each giving the `session context unknown` line, one `session end` for two
  `session_shutdown` events, and a child asking about the root and never ending it; and against the
  binary, the fork paragraph for `fork` and `new` only, and the pointer kept for every replacing reason
  and forgotten for `quit`. Read, not run: `BeforeAgentStartEvent.systemPromptOptions` and
  `NormalizedBuildSystemPromptOptions.sections` in `dist/core/extensions/types.d.ts` and
  `dist/core/system-prompt.d.ts` (a section renders as `<name>` around its content), `SessionStartEvent`
  and `SessionShutdownEvent` with their `reason` values, and the session header's `id` in
  `docs/session-format.md`. Not observed: that Pi accepts a section added in place by a handler and sends
  it to the model, that it awaits an async `session_shutdown` handler before exiting, that `/fork` and
  `/clone` both report `fork`, and the section after a compaction. A previous session file that cannot
  be read gives no `--previous-session-id`, so the fork paragraph is absent rather than guessed. The
  pointers of sessions that were replaced and never resumed are not forgotten by this change; they are
  the ordinary stale pointers the daily prune in `session::maintain` already ages out.

- **The Pi `/goal` judge was read from Pi's declarations, not run.** The extension asks
  `ctx.modelRegistry.streamSimple(ctx.model, { systemPrompt, messages }, { signal, maxTokens })` and reads
  the text blocks of `await stream.result()`; the signatures, the `AssistantMessage` content shape and the
  `stopReason` values were read from the installed Pi 1.1.0's `.d.ts` files and `docs/extensions.md`, and
  the tests drive a stand-in registry shaped like them. No test reaches a provider, so three things are
  not measured: that request-time authentication succeeds for a model the session picked, that a model
  given 200 output tokens answers on its first line (a reasoning model that spends them on thinking
  returns no text, which stops the goal as unreadable, never as met), and what the judge is shown. The
  transcript is the extension's own plain-text rendering of user, custom, assistant text and tool-result
  messages from `buildSessionProjection`, not `convertToLlm`'s: images, thinking, tool-call arguments,
  compaction and branch summaries are not rendered, a user or assistant message is cut at 4,000 characters,
  tool output at 2,000 and the whole at 20,000 keeping the newest, so after a compaction, or a long
  turn, the judge sees less than the model does. The judge runs on every third turn and on the cap
  turn, so a condition that holds after turn 1 is noticed at turn 3, and the turns it was not asked on
  were spent: no completion marker in the session is read to judge earlier (issue #3). The goal and loop live in the
  extension's memory and `session_shutdown` clears them; that the event fires for `/new`, `/resume` and
  `/fork` as well as quitting is read from the documentation's statement about session replacement, not
  observed. Another extension that registers `goal` or `loop` is not detected, and which registration Pi
  keeps was not read.
- **Pi's reviewer model is only as precise as the `reviewer` routing row.** The row is one value for every
  host, and a CLI reviewer reads it as a flag. A `provider/id` that Pi lists runs on that model; a bare
  alias such as `opus` is a valid CLI model and an unresolvable one for Pi, and is refused
  `reviewer-unavailable` rather than guessed at. On a machine with the `claude` CLI installed Pi's relay
  is never reached, so a `provider/id` row there is passed to that CLI instead. Pi reasoning effort is
  not set: the completion sends no `reasoning` option, so the model's own default applies whatever effort
  the row names. Neither the relay nor `admit` bounds the answer's size the way a CLI reviewer's `MAX_OUTPUT` does; only the completion's own limits apply.
- **Pi's `powershell` is read as POSIX shell, other extensions' tools are gated only by input shape,
  and Pi's own settings are not covered.** The extension sends any tool whose input has a string
  `command` to the gate as `bash`, and a string `path` with a string `content` or an `edits` array as
  `write` or `edit`. That includes Pi's `powershell` and gentle-shell's `bash_background` and `monitor`.
  The shell classifier reads POSIX shell, so a PowerShell cmdlet that writes (`Set-Content`,
  `Out-File`, `Remove-Item`, `Add-Content`) is classified as no write and goes through, while the
  `git`/`gh` command lines, redirects and nested interpreters it does recognise are caught: a Windows
  Pi that writes through cmdlets writes ungated, and this was chosen over not gating `powershell`
  at all. A tool that names its command or path differently (`cmd`, `file_path`, `script`) is not
  recognised, and one that hands over a write in a shape other than those is not gated; the
  extension does not guess at it. The refusal text the binary prints names `bash` or `write`, not the
  tool's real name; the card names the real one. None of this has run in a live Pi: it is measured
  against a stand-in binary under `node`. A later `tool_call` handler can mutate `event.input` after this one has answered; Pi documents that
  mutation as allowed. Pi's own control files are covered in the entry below.
- **Pi's settings can unload the gate and the tool server, and the gate measures the write and
  `doctor` reads the result, not the loader.** Read in Pi 1.1.0's documentation and
  settings screen: the `extensions` array of `settings.json` takes an exact exclusion `-path`, a glob
  exclusion `!pattern` and the built-in `-builtin:mcp`; a project's settings apply over the user's;
  and a project `mcp.json` entry with no `command`, `url` or `type` overrides only `enabled`,
  `exposure` and `toolExposure` of the user's server of that name. One ordinary write to
  `~/.pi/agent/settings.json`, `.pi/settings.json` or `.pi/mcp.json` therefore unloads the gate or the
  tool server from the next session without touching a file Estigia owns. All of those, and the
  project's `.pi/extensions/`, are on the control surface now, so a write or a shell command naming one
  is a boundary. The matcher does not tell a home path from a project one: each is a fragment read
  anywhere in the path, so `.pi/settings.json` matches the project file and `.pi/agent/settings.json`
  the user's, and a write to a *different* repository's `.pi/settings.json` is a boundary as well. The
  `pi-config` row of `doctor` reads the user and project `settings.json` for an `extensions` entry that
  excludes `estigia-workflow-authority.ts` or `estigia-review-relay.ts`, by exact name or by a glob that
  matches either, or `-builtin:mcp`, and the project `mcp.json` for `enabled: false` or an `exposure`
  other than `direct` or `deferred` on the `estigia` server; it is broken, never rewrites a file, and names no
  command, because `estigia setup pi` does not touch Pi's settings. What is not measured: whether a
  project `-path` excludes a *user-level loose* extension at load time (the write path was read, not
  the loader); Pi's glob dialect (the row's own matcher lets `*` cross `/` and knows `?`, nothing
  else); other resource arrays and `packages`, which are not read; a settings or `mcp.json` that does
  not parse, which the row treats as Pi would, as absent; the row's checkout is the directory `doctor`
  runs in, not necessarily the project Pi opens; and that a project file takes effect only once the
  project is trusted, which the row does not consult. The agent directory is the one the entry below describes.
- **Pi's agent directory follows `PI_CODING_AGENT_DIR`, but only as the process that runs `setup`,
  `status`, `doctor` or the gate sees it.** One resolver reads the variable (a leading `~`
  is the home; anything else not absolute is dropped with a note, falling back to `~/.pi/agent`) and
  every Pi path comes from it: the directive, the server entry, the extensions, the agent definitions,
  and the directory the `pi-config` row reads. The directory is followed wherever it points, unlike
  `DSH_HOME`, which is contained under the home. The gate treats the named directory's `extensions/`,
  `mcp.json`, `settings*`, `AGENTS*` and `agents/` as control surface, but the static
  `harness::CONTROL_SURFACE` cannot spell a directory the environment chooses, so that coverage is the
  gate process's own variable: a Pi started with a different one is not covered. Not measured: an install
  made under `~/.pi/agent` and then run under a variable that names another is not reported at all (Pi,
  with that variable, does not read it either, and `doctor` says only that Pi is not configured); Pi's own
  expansion of the variable beyond the leading `~` is not reproduced; and nothing was run against a
  live Pi under the variable. Pi documents that `AGENTS.override.md` replaces `AGENTS.md` in the same
  directory (read in the installed Pi's `docs/configuration.md` and `loadContextFileFromDir`), so a file
  by that name leaves the fenced block unread; the `pi-config` row is broken when one is in the agent
  directory, and does not look for one beside a project's own `AGENTS.md`.
- **Pi's agent definitions follow an example extension's convention, and none has run in Pi.** Pi's
  core has no subagents. The `name`, `description`, `tools` and `model` frontmatter, the
  `~/.pi/agent/agents/` directory and the `--tools`/`--model` pass-through are read from the shipped
  `examples/extensions/subagent` (`agents.ts` and `index.ts`) in Pi 1.1.0; another subagent extension
  (gentle-shell's) may read a different directory or dialect, and no child `pi` was launched against
  an installed definition. `model` is the `Model routing` value verbatim, so an alias Pi's `--model`
  cannot resolve fails the child rather than being mapped; `inherit` writes no line. No effort is
  written, because the example reads none. `tools` restricts only built-in tools (`Glob` becomes `find`
  and `ls`): Pi's `--tools` keeps MCP tools unless an entry starts with `mcp__`, so the analyst is
  read-only for built-ins and still reaches Estigia's own MCP operations and any codemode script;
  what stops it writing is the gate measuring the child against the parent's claim, not the
  definition. `harness::roles::definition_for` does not search this root, since Pi children carry no
  role in the gate call; the directory is control surface through the Pi instruction fragment. A
  `PI_CODING_AGENT_DIR` move strands them with the rest, as above.
- **A gate plugin newer than its binary blocks every gated call.** Both plugins send `--input -` and
  the input on standard input. An older binary reads `-` as the JSON, refuses with exit `1`, and the
  plugin blocks the call with that refusal. `setup` writes the plugins together with the installed binary,
  so the skew arises only when a plugin outlives a downgrade; re-running `setup` clears it.

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
- **`ensure-states` cannot tell "already there" from "gh could not run".** `gh label create
  --force` exits non-zero for both, and the command reads that status as idempotence — which is
  right for the first and wrong for the second. Measured on both sides against a `gh` that refuses
  every call: each answers `ok: true` with all six labels listed as `ensured`, and not one of them
  exists. The workflow then proceeds to label an issue with names the tracker has never heard of.
  It is not a divergence, so the differential oracle could not have failed on it even while it
  existed. Closing it means reading
  what `gh` printed rather than only its exit code, and this crate has never been shown those exact
  words — the same reason `icacls` is named beside `neutralising-shell` and not added to it.
  A closed git operation window is the one case where the answer says so: it carries
  `deferred: true` and `window: closed` beside the same `ensured` list, and
  nothing was created — so a reader has to tell that answer from a `gh` that could not run, and the
  `deferred` field is the only thing that does it.
- **Both of those numbers used to drift unread.** The check accepted the phrase "every agent Estigia
  knows" and then stopped, so the sentence went on to say "six of them, in three dialects" while ten
  were gated in five. The counts in that sentence are now crossed against the adapter table and the
  dialect list, which is the same defect this project keeps finding: one end written by hand, the
  other in code, and only *part* of the prose crossing over.
- **A sub-agent's declared tool list is enforced only for the tools the gate wakes for, and only
  where an agent name arrives.** The role gate runs in `PreToolUse` and, since it was added there
  too, in `estigia gate` — but that second door is reached in this tree only by OpenCode's plugin,
  which sends the tool's own arguments and no agent name, and whose own documented limit is that
  `tool.execute.before` never sees a call made by a subagent at all. So the second door closes
  nothing today: it is open for a caller that names a sub-agent, and there is not one yet. Where the
  question *is* asked, it sees exactly what the matcher wakes the hook for —
  `Edit|Write|MultiEdit|NotebookEdit|Update|Bash|Agent|Task|AskUserQuestion` for Claude Code. `Agent` and `Task` first
  take the dedicated reserved-reviewer prelaunch path rather than repository classification. A tool outside that set is
  never offered to it. Measured against the list this crate itself cites as the case it exists for,
  a published `builder` sub-agent declaring `Read, Write, Edit, Glob, Grep, Bash`: three of the six are tools the
  gate can judge, and `Read`, `Glob` and `Grep` are never seen. What it can therefore refuse is a
  tool **in the matcher and not in the list** — `MultiEdit`, `NotebookEdit`, `Update` for that
  builder — and not `WebFetch` or `WebSearch`, which are among those an operator narrowing a
  sub-agent usually means. The module says it *makes the author's policy true*; it makes the part
  of it that overlaps the gate true. Widening the matcher would close it and is the one thing the
  matcher exists to avoid — waking this process for every `Read` and every `Grep` is a cost paid
  thousands of times to answer "not mine" — so this is stated rather than fixed.
- **The verdict's binding is checked at delivery only for what this run published through Estigia's
  own tools, with an earlier exact-receipt check at CI release.** `publish_review` records a fresh
  epoch over PR/head/base/digest while the PR is confirmed draft. `release_ci` re-verifies the live
  `review` claim, the newest receipt on the receipt's own pull request across runs, the verdict recomputed over the live ledger and its marker,
  current draft PR and coherent clean target, then marks ready and reads back every outcome. The
  gate asks for one approved computed verdict; it does not enforce the blind policies' lens count,
  concurrency, independence, blindness or same-finding identity, and it cannot tell one
  context from two or five. An identical-byte republish still invalidates old evidence because it creates a new
  epoch. GitHub has no atomic conditional-ready operation, so an out-of-band ready or push can bypass
  this cooperative order.
  The local delivery gate persists the complete epoch/PR/head/base/digest receipt and still checks
  the published head at the boundary that spends it, but not always against the
  same thing: a `git merge`, `git tag` or `gh release` from a run whose recorded review head is not
  the invoking checkout's own head is refused as `verdict-bound-to-other-bytes`, naming the path
  inspected and the head found there (the run's own merge commit is the exception the entry on
  #58 below describes). The invoking checkout is the covered directory the gate already
  selected and verified for this call; it is not inferred again from the run pointer, whose worktree
  can be absent after a handoff. A `gh pr merge` is instead refused against the PR head the tracker
  reports — one `gh pr view <number>` read per merge attempt, naming the PR number, the reviewed head
  and the head found there — never against the invoking checkout's own `HEAD`. That checkout is,
  under Claude Code, structurally the launch checkout, since the Bash tool resets it between calls;
  comparing against it could never pass even when the PR's remote head was exactly the one reviewed,
  which is the gap that was closed. For a delivery `gh pr merge` boundary only, a linked sibling
  sharing the pointer checkout's Git common directory can reach that verification. The classifier
  first retains one positive numeric PR only from one literal `gh pr merge <number> ...`; malformed,
  omitted, URL, branch, long, leading-zero, duplicate, compound and shell-evaluated targets remain
  irreversible boundaries but cannot select sibling evidence. So does any merge that names a
  repository — `--repo`, `--repo=` or a short option bundle containing `R` — even for the run's own
  receipt: the line is refused as `delivery-pr-repository-option`, which names the option and says to
  run the merge from the checkout without it, because the number is there and the gate reads one
  spelling only. A command the byte filter disqualifies — quoting, escaping, expansion, a
  redirect — is refused as `delivery-pr-unidentified` with the disqualifying byte named in the
  resolution, so a `2>&1` redirect reads as what it is rather than as a claim that the command names
  no PR. Candidate pointers are then filtered by complete `receipt.pr == command PR`, exactly one
  holder must remain, and only then is the PR head the tracker reports compared with that holder's
  receipt and its live tracker claim verified. Equal PR heads never select between PR lineages — the
  receipt's own PR field already did. A legacy `reviewed_head` pointer remains readable after upgrade
  but cannot qualify PR-targeted sibling delivery because it carries no PR, epoch, base or digest.
  Ambiguous candidates and unreadable pointers select no holder, including for ledger attribution; the
  recorded holder comes from the same adjudication as the decision rather than a later directory
  scan. An unrelated or unreadable checkout named with that run can answer six ways: a checkout
  that is not a checkout of the reviewed repository, or a PR head the tracker reports that has moved
  from the reviewed head, is `verdict-bound-to-other-bytes`; a command whose identity the byte
  filter disqualified is `delivery-pr-unidentified`; a merge naming a repository is
  `delivery-pr-repository-option`; a merge whose PR differs from the receipt's is
  `delivery-pr-mismatch`; and a pointer carrying only a legacy head is
  `complete-review-transaction-missing`; a linked sibling that selects no unique holder is
  `complete-review-transaction-not-selected`. A sessionless call in an unrelated clone has no local holder
  to adjudicate and remains outside. Successful publish and republish effects first invalidate prior
  complete and legacy local authority, then retain output only when all five receipt fields are
  complete. Their refusals also invalidate that authority when the transport says a write committed
  or cannot exclude one; a conclusive pre-write or read-only refusal preserves it. Verdict and
  CI-release effects restore a supplied complete receipt atomically into a reviewer's pointer after a
  handoff removed the publisher's pointer. `gh pr merge` is also refused `merge-without-ci-release`
  unless the local store holds a trusted `ci-release` marker for exactly the pointer's receipt,
  which `release_ci` writes only over the basis it admits; the read is the issue's facts and then
  the store read-only, and an unreadable one refuses. A marker is trusted as every other is: written
  by this identity and never edited, so an imported comment carrying one counts the same.
  A `claim` restores one too: its answer carries the issue's newest publication under
  `review_transaction` when that publication is confirmed, stands approved and unescalated, and its
  pull request is open at the receipt's head with a base that is the receipt's or has advanced cleanly
  past it (see *A base that advanced cleanly keeps the receipt live* below), so a release-then-claim
  sequence keeps the merge the gate admits. That is one `gh pr view` read, made only when the issue
  holds a publication, plus one local ancestry read when only the base differs, and a failed read
  restores nothing rather than failing the claim. It does not read CI state, and a
  push between the claim's read and the merge is caught only by `--match-head-commit`.
  `git push` and `gh pr create` are deliberately not gated on it, because pushing after a review is
  how a run fixes what the review found and the answer to a moved head is *re-publish*, not *stop*.
  What it does not see: a run that bypasses the tools and changes GitHub directly. The gate's own question to the tracker is still
  `verify-claim --issue --run-id --expect-state`, carrying neither head nor base; the head comparison
  is a separate question, and only for `git merge`, `git tag` and `gh release` is it local, against
  that invoking checkout. For those three, both halves of the comparison — whether the invoking
  directory shares the covered clone, and what HEAD it is at — run through the same unsteered git
  invocation the fast-forward proof already used, so an exported `GIT_DIR` cannot make an unrelated
  clone look like the reviewed checkout. `gh pr merge` still takes the first half from that same
  unsteered invocation — an exported `GIT_DIR` cannot make an unrelated clone pass its
  `same-repository` check either — but takes the second from the tracker read described above rather
  than from local git.

## Delivery: the receipt, the base and the remote

- **What the `gh pr view` read measures, and what it does not.** It measures the PR's
  `headRefOid` at the moment `stale_verdict` asks, one read per merge attempt, and refuses a mismatch
  against the receipt with the PR number, the reviewed head and the head found. It does not measure
  the base: `release_ci` already re-verified `baseRefOid` at ready time, and the only later reads of
  it are `review_status` and the claim's restoration of a receipt, which accept a clean advance (the
  entry after this one) — the delivered SHA's first parent is GitHub's strict setting's promise,
  as the branch-protection entry below says — so a base that moves *between* that ready check and
  this merge attempt is not seen at this boundary. It does not measure anything after the read: the
  read is a snapshot, and a push landing on the PR between this read and the merge itself is not seen
  by this check. A push in that window is refused by GitHub itself only when the merge call carries
  `--match-head-commit <head-sha>` naming the reviewed head, which `skill/bindings/github/delivery.md`
  prescribes for the `merge` step and which this gate does not verify was used. It does not resolve which
  repository `gh` would ask: an unrelated clone is refused on `same_repository` — whether the
  covered checkout and this one share a Git common directory — never on what `gh pr view` itself
  would have answered for it, so a checkout `gh` could still reach is refused before that read ever
  runs if it is not a checkout of the claimed repository.
- **A base that advanced cleanly keeps the receipt live, and a semantic conflict between two
  independently green changes is not tested before the merge.** Issue #44, operator decision
  2026-10-13. A receipt used to be live only while the pull request's `baseRefOid` was the reviewed
  base, so every merge to `main` cost each other approved pull request a review and a CI run. It now
  stays live for the merge, in `review_status` and in the claim's restoration of it
  (`current_review_transaction`), when the head is the reviewed head, the pull request is ready and
  its receipt approved, the current base descends from the reviewed base (`git merge-base
  --is-ancestor`), and GitHub reports `mergeable: MERGEABLE` with a `mergeStateStatus` that is not
  `DIRTY`. `UNKNOWN` in either field is answered `mergeability-unknown`, never clean. **The accepted
  risk:** CI went green on the reviewed head over the *old* base, and nothing asks it again over the
  new one, so two changes that merge without a textual conflict yet break each other are not caught
  before the merge; only the CI that runs on `main` afterwards sees it. The gate still compares the
  head alone (the entry above), so it never refused a moved base; what changed is that the receipt
  is no longer withdrawn. `release_ci` stays strict about the base: it publishes the target to CI,
  and a draft whose base moved is answered `pull-request-moved`, as is every approved-but-not-ready,
  rejected or unjudged receipt. `mergeable` is GitHub's own and is lazily computed, so a clean read
  can still precede a conflict that lands in between; `--match-head-commit` guards the head only.
  When the base object is not in the local clone the ancestry read fails and `review_status` answers
  `unavailable` rather than guessing; the claim restores nothing.
- **`recorded_receipt` no longer requires the caller to be the run that published, and this proves
  only that the tracker recorded the publication — never that the caller reviewed anything.** A
  local run pointer is deleted at `SessionEnd` (`session::forget`, `src/harness/hook.rs`) while
  the tracker's own comment marking the publication is not, so a run that resumes an issue, holds
  the live `review` claim, and supplies the byte-exact latest receipt used to be refused
  `published-transaction-mismatch` on the publisher field alone even when every other field matched.
  `recorded_receipt` (which delegates to `require_recorded_receipt`, where the one comparison lives) now
  compares only `latest.receipt != *supplied`;
  the run id that published is read back into the refusal's `recorded.run_id` for a caller to see,
  but is never itself grounds to refuse. What still stands between an arbitrary caller and CI is
  everything else `release_ci` already re-verifies: a live review claim on this exact issue, a
  computed `approved` verdict recomputed over this exact receipt's ledger, the current draft PR
  identity, and a re-derived coherent clean target — the same four checks the paragraph above this one names.
  **What "distinct" excludes is the publisher and every handoff requester, not the caller**,
  so after this change the run that recorded the computed verdict can be the run that spends it. That was already the design — `src/harness/mcp/mod.rs`
  writes the supplied receipt into the pointer for `release_ci`, for
  "a reviewer claiming after a handoff" — and the publisher comparison contradicted it. Removing it
  closes that contradiction rather than opening a hole, and Estigia still cannot prove who reviewed
  anything, which the entry on panels already records.
  Restoring the receipt this way answers one question only: *did the tracker record this exact
  publication?* It does not and cannot answer *did the
  resuming run review any of it* — that is what the computed verdict is for, and this change
  reads it rather than substituting for it. `PointerEffect::Renew` then writes the restored receipt
  back into the resuming run's own local pointer, in the `PointerEffect::Renew` arm of
  `src/harness/mcp/mod.rs`; no code path here or in that arm hand-edits a pointer file, and none
  should be needed for this restore to work.
- **A delivery can still merge onto a base the reviewed receipt did not name, and this repository
  now answers it with a GitHub setting that Estigia neither sets nor reads back.** It was measured
  the gap: `release_ci` re-verifies every byte of the reviewed head, and nothing asked whether `main`
  was still the receipt's base when the merge happened — GitHub's own protection reported
  `required_status_checks.strict: false`, so a stale branch merged cleanly onto whatever `main` had
  become. The chosen remedy is that one setting: with `strict: true`, GitHub requires the topic branch
  to be up to date with the base before merging, and an update is a push, which invalidates the
  receipt — so a moved base forces a fresh publish/review round rather than a silent merge. **The forced round is defined as follows.** The `compatible` classification still integrates, but the
  integrating republish is repair-shaped: the republish gate reads whether the base **advanced** past
  the parent receipt's rather than the push shape, so a merge that keeps the parent head is publishable
  over an approved parent, the marker records the parent receipt and the delta, the panel re-judges
  the parent receipt's findings over the whole change, since a moved base is read whole, and
  a republish over an
  approved parent is now a new candidate that spends no correction — the platform elected it, not the
  author. A merged advance keeps `parent-head` an ancestor and so used to read as the narrow range;
  `repair_range` now answers `whole-change` with `whole_change_reason: "base-advanced"`
  whenever the base moved, merged or rebased — and `"merged-range"` when the base did not move but
  a side branch was merged back — so the delta lens never reviews base content, and
  the budget now measures the round against the tree git merges the parent head and the new
  base into. The exemption is also direction-aware: it read inequality of the two base
  tips before, so a base reset backwards counted as an integration, and now it needs the parent's
  base to be a proper ancestor of this publication's. Every other classification still integrates and re-reviews the whole
  change, `unknown` is still never compatible, and this change touches neither `strict` nor the
  gate's head comparison. What that
  does **not** guarantee is written here because it is the boundary of the fix:
  - **Estigia does not read the setting back.** Nothing in the gate queries branch protection at
    delivery time; the guarantee holds only while a collaborator leaves `strict: true` on. A setting
    removed between publication and merge is a delivery with the same exposure as before, and no
    local check would say so. Read live on 2026-09-27: `main`'s protection answered
    `required_status_checks: null`, so the setting is not on at this moment — the exposure this
    bullet names. The forced-integration shape is, until it is restored, the
    contract's intended route rather than the platform's requirement.
  - **The delivered SHA's first parent is guaranteed by GitHub's strict check, not by Estigia.** The
    gate's merge-identity checks (`stale_verdict`, `pr_merge_target`) still compare the reviewed
    head — for `gh pr merge`, the PR head the tracker reports; for the other three deliveries, the
    invoking checkout's own head — never the base; the base's constancy is the repository setting's
    promise, taken on trust.
  - **A strict setting does not make concurrent delivery converge.** The 2026-08-20 diagnosis
    measured `main` moving seven times under one branch while the full delivery floor — integrate
    (~4 min) + `ready_for_review` CI (14 min) + merge, over 18 minutes even at zero review time —
    already exceeded the delivery interval (15–25 minutes); each base move now costs an
    integrate-and-republish round that invalidates the accepted verdict. The window the contract
    needs — publish, review, CI, merge with the base unchanged — is still longer than the observed
    interval, so a run delivering beside another may wait many rounds. An integration
    over a moved base is admitted over an approved parent — and it is a new candidate that
    spends no correction, repair-shaped on a `compatible` movement — so the correction limit does not
    bound a base-movement storm either; the honest bound there is the base itself, and what ends the
    wait is
    the base stopping rather than a person being escalated to. That is the honest price of
    the guarantee, stated rather than hidden. `ci.yml` carries `merge_group:` so a merge
    queue can run the suite after ready without each branch re-integrating by hand first; enabling
    the queue and whether its run satisfies required contexts is a live-repository configuration
    and measurement, not something this crate sets or proves. Read live on 2026-09-12: this
    repository's `main` answered `Required status checks not enabled` and GraphQL `mergeQueue` was
    `null`, so that measurement could not be taken from a checkout alone.
- **A `published` marker does not prove the remote still holds that head, and an unconfirmed one
  does not prove the pull request ever reported it.** After a successful push,
  `publish_review` reads the PR back before recording a confirmed receipt. When that readback
  disagrees, the push has already landed and the timeline would otherwise name nothing the lease
  can target — leaving only a hand force-push, which is the step `republish_review` exists to keep
  adjudicated. The recovery records an **unconfirmed** publication (`unconfirmed=true`) naming the
  head that was pushed: `republish_review` leases against it; `recorded_receipt` /
  `require_recorded_receipt` refuse it as `publication-unconfirmed` so review and CI cannot bind to
  bytes nobody confirmed. What that still does not prove: that the remote is *still* at that head
  when the lease runs (git decides), that the PR view ever agreed (it is what failed), or that the
  unconfirmed marker was written if the comment post itself failed — the refusal then says so and
  names no executable recovery. A confirmed marker still only means the PR agreed *at publication
  time*; nothing re-checks the remote before a later verdict.
- **A `republish_review` that refuses has already written to the pull request, and names which
  writes.** Its refusals arrive after the reused pull request has been edited — the renewal stands
  immediately before the push, and the lease is evaluated by git at the push itself — so a remote
  somebody else moved leaves the branch untouched, which is what the operation is for, and leaves the
  pull request altered. **Every** refusal site downstream of a write carries that fact, so none answers
  *nothing was written* — the draft readback, the failed edit, the renewal and the push. Counted rather
  than named, that sentence said *three* and a later change made it four without moving it; the number
  is the part that had no reason to be there.
  Each had reached the agent through the same `stop()` and `?` every other path uses, whose envelopes
  carry no `world`, so *the absence of a claim about the world was read as a claim that the world was
  untouched*.
  **Which** writes is the part that took four rounds. The report was built from one boolean and named
  two writes: an edit, and a conversion back to draft. `ensure_draft` un-readies only a pull request
  that was *ready*, and at republish time the reused one is normally already draft — `publish_review`
  drafts it and only `release_ci` makes it ready — so the common path claimed a conversion nobody
  performed. An operator putting that back runs `gh pr ready`, which exposes the branch to CI: the
  exact outcome `ensure_draft` exists to prevent. The report now names each write separately and only
  when it happened, which is the same rule this document applies to everything else — say what was
  measured, not what usually accompanies it. What is still on the reader is the decision: Estigia
  reports the write, it does not undo it.
- **The timeline does not record that a publication was a republish.** The answer `republish_review`
  returns carries `republished` and the head it leased against, but the `published` marker it writes to
  the issue is byte-identical to an ordinary publication's. So an incident review reading the timeline
  — which is the only record that outlives the call — sees a new epoch and cannot tell whether the
  bytes arrived by fast-forward or over a force-push.
  **Corrected: this was never a consumer cost.** The entry used to say
  naming it in the marker would change what every consumer of that marker parses — stated as a general
  claim about the marker rather than about `republished` specifically. It was wrong: `markers::parse`
  (`markers.rs`) inserts every key with no validation on read, and every `published` reader
  takes named keys only, so a new key breaks none. No count is given, because a count here would be a
  second copy of something nothing crosses: an earlier edit of this entry said `eight`, and the
  readings that checked it disagreed on what counts as a reader. The
  `topology`/`topology-reason` entry below adds two keys on that basis, at zero
  cost to every existing reader. What was actually declined here is narrower and still stands:
  `republished` is not recorded, and the reason is scope — this repository's own rule against
  widening one issue's fix into a second one's, not a technical wall. A future change is free to add
  it the same way.
- **The review transaction carries a declared execution topology, and Estigia sees the declaration,
  not the delegation.** `publish_review`/`republish_review` take `--topology` and
  `--topology-reason`, write them to the `published` marker on the `payload-digest` precedent —
  recorded, not bound, since Estigia cannot observe how an agent wrote code — and answer them
  through `review_transaction`, not through the publish answer. Four things this still cannot prove, named rather than smoothed over:
  - **The string is what a run wrote about itself, not what happened.** `verify_claim` sees a claim;
    `publish_review` sees a topology field. Neither watches which contexts a run launched. A run that
    types `--topology developer:3` having launched nothing is indistinguishable on the timeline from
    one that actually delegated — the same standing a lens report has for a lens nobody launched.
  - **There is no refusal, because the proxy that stood in for one could not judge the row.** An
    earlier version refused a bare `inline` over two changed files. The contract's writing row now
    delegates a writer for a named reason — independent units, or the parent's context limit — and a
    file count says neither. Measured against this repository's own last twenty merges
    (`git log --format=%H --merges -20 main`, changed files per merge against its first parent), every
    one touched at least 3 files, the range running 3–26 and the median 11, so the refusal would have
    asked for a sentence on every delivery that declared itself `inline`. It was removed rather than
    raised: a higher number is still a file count. The declaration is recorded as typed and the gate
    gains no authority from it.
  - **Estigia already holds half of an *observed* topology, and this field is not it.** `Run.roles`
    (`src/harness/session.rs`) records which roles this run's delegated contexts actually ran
    as — observed from tool calls, not declared in a launch prompt — and this document's own earlier
    entry on it (`docs/honesty.md`) is its own measured limit: per-session, dies with the
    run, names a role and never a count, and under the default `Evidence standard: reading` the
    reserved reviewer is never recorded at all because its grant holds no tool the hook matcher wakes
    for. A reader who finds a *declared* topology field on the review transaction should know the
    crate holds half of an *observed* one elsewhere, unconnected to it — wiring the two together is a
    separate issue, not this one.
  - **Saying nothing is free, and so is saying `inline`.** A publication that passes no `--topology`
    carries no topology key, and one that passes `inline` carries exactly that string. A report was
    filed because *"nobody can tell 'judged the table and departed with reasons' apart from 'never
    read the table'"*, and nothing here closes that: the field is a record a reader can weigh against
    the diff, not a check. Making it required would refuse every caller that predates it.

  The three ways a declaration could differ across the note, the marker and the
  answer were closed: a blank `--topology` or `--topology-reason` is refused rather than recorded, a reason with
  no topology is refused as incomplete, and both values are trimmed once before the
  marker and the answer see them — so the three surfaces agree by construction. Case is preserved as
  typed: with nothing reading the field back, folding it would lose information for nothing.
- **A republish cannot lease against a receipt published from a different GitHub account.**
  `latest_publication` keeps only comments the authenticated identity authored, because a marker this
  identity did not write is one anybody could have forged. The consequence is narrow and worth naming
  rather than leaving to be discovered: a run that reclaims an abandoned issue published from another
  account finds no receipt, is refused `published-transaction-missing`, and then has no route inside
  Estigia at all — `publish_review` cannot fast-forward a rewritten branch either. It fails closed,
  which is the right direction, and it leaves that reclaim with nothing to do but step outside the
  tools, which is the thing this operation exists to stop.
  What the lease does **not** prove is that the rewritten history still contains the reviewed change.
  It compares one commit id to another; a rebase that dropped a hunk leases exactly as cleanly as one
  that did not. Every republish creates a new epoch and invalidates the prior review evidence for
  that reason, and the answer to *is this still the change that was approved* is a fresh verdict
  against the new receipt, never the lease.

- **The store-only timeline read trusts the store and is given its repository.** `Ledger::read_timeline_at`
  (S2a) reads no GitHub, so it cannot learn what the store has not yet synced: a stand-down or
  adjudication commented since this machine last touched the issue is not in what it answers. It also
  does not discover which repository the checkout belongs to — the caller names it, and
  `review_status` does so from the checkout (next entry). The `expected_epoch` check on `publish_review` and `republish_review` reads the timeline
  the publication already reads, so it is as fresh as that read; it compares the newest publication on
  the branch's pull request and nothing else, and a branch with no open pull request compares against
  `none`.

- **`review_status` is a read, not a lock, and it names the repository from the checkout.** The answers are in [`review-status.md`](review-status.md). It answers against the receipt it
  read and every command it names carries that receipt, so a publication landing afterwards is refused
  by the command (`published-transaction-mismatch`, `expected-epoch-moved`) and never executed. That
  protects a reader that executes the answer unchanged; it does nothing for one that decides from the
  answer and drops the binding. The store it reads is as fresh as this machine's last sync (see the
  entry above), so a finding another run posted since is not in the answer until a tool that syncs has
  run. The repository comes from the operator-named repository, else the `origin` remote when that is
  a `github.com` remote, never from GitHub: where `gh` resolves a different repository (a fork with an
  `upstream` default) the store holds the issue under that other name and the answer is
  `review-status-issue-not-in-store`, unavailable, never a wrong state. A pull request merged and a new
  slice not yet published under the same issue answers `delivered` for the issue's newest publication
  only. An approval is read through `release_basis` and a computable approval through the publication
  lane; a refusal outside the cases below is passed through as theirs rather than named a second way.
  **A stale approval with no cause has no command of its own.** When the ledger or tier moved after an
  approval and no finding was recorded against it (a tier that moved, a late lens report that escalates
  the lens focus), neither a publication (`approved-republish-without-cause`) nor a re-record
  (`computed-verdict-already-recorded`) is admitted without a cause, so `review_status` answers a
  `verdict-stale` stop that names a person, who either records the finding the verdict did not weigh
  with `record_review_finding` — which admits the publication — or closes the pull request and
  publishes the change afresh. Whether such an approval should be re-reviewable without a finding is
  open. A base advance is not read, so a stale approval whose base advanced still answers the stop.

## The seams, and the gates that never see a call

- **This section is checked against the code.** `tests/honesty.rs` crosses the *countable* claims
  here — how many agents are gated, how many things `doctor` looks at, which mechanisms the code
  still uses — against the thing they describe, and fails when they drift. The claims about *kind*
  are judgements and are not checked, because a test that pretended to would be the false comfort
  the section exists to prevent.
- **The seams are checked along the path, not at its ends.** Three places had each end verified and
  the middle assumed: the gates, the flag lists the MCP tools build for the transport, and the
  values a configuration refusal tells an operator to write. Each is now crossed against the thing
  it has to agree with — the classifier, the transport's own `argparse`, and the parser itself.
  In a refusal, a backticked value is a literal a person types, and every one of them is checked to
  parse.
- **A registered gate is not an enforcing gate.** Five of them registered, fired, reached the
  classifier, found a tool name it did not know and stood aside — for four rounds, while `status`
  correctly reported `gate on`. Two guards now cross-check every matcher and the OpenCode plugin's
  own list against the names the classifier recognises. Two further instances turned up later, from
  the other end of the same path: an entry naming an event this build refuses (`PreToolUse`, the
  spelling the host's own documentation prints — a non-blocking error, so the tool call went through
  ungated with one line in a transcript), and an entry naming an executable that is no longer there
  (`cargo clean` deletes it; so does moving the checkout). Both were live on the machine this was
  written on. `estigia status` now reads the registered command back and says `gate REGISTERED BUT
  DEAD` rather than `gate on`, and `doctor` reports the file and the fault.
- **That count went 1 → 2 → 4 → 5 → 6**, once per round of actually checking. Every stop along the
  way was a verification route not yet exhausted rather than an agent that could not be gated —
  twice the answer was in a source repository the published documentation did not mention.
- **A worktree Claude Code makes for itself is not seen.** `--worktree`, `isolation: "worktree"` and
  background sessions create one through the runtime rather than through a shell, so no `Bash` call
  carries `git worktree add` and `PreToolUse` never fires. The event that does fire is
  `WorktreeCreate`, and it is not a permission gate: it *replaces* git's behaviour, expects the
  worktree path on standard output, and fails creation when the hook fails or prints nothing.
  Registering there to deny would make Estigia responsible for creating every worktree, and would
  break the feature for anyone whose run holds no issue. So the shell spelling is gated and the
  native path is not — and a write inside that worktree is still measured, because the claim covers
  the directory rather than the way it was made.

- **Claude Code subagents are covered; that is why the line below is OpenCode's and not the
  design's.** A subagent's tool calls fire the same `PreToolUse` hooks as the main thread, carrying
  `agent_id` and `agent_type`. Checked in the runtime's own hook reference rather than assumed from
  the parent behaviour, because the sibling runtime answers differently.

- **An extension point is not automatically a place to deny.** Three were checked and refused for
  the same reason, and it is worth stating once rather than three times. Claude Code's
  `WorktreeCreate` *replaces* git's behaviour and fails creation when the hook says nothing, so
  registering there to deny would make Estigia responsible for making every worktree. Goose's
  classification API is an HTTP service that labels text as prompt injection. OpenHands'
  `SecurityAnalyzerBase` is an in-process Python class returning a **risk level**, and what happens
  next is the user's confirmation setting. Estigia's refusal means *this run holds no claim covering
  this write* — rendering that as "high risk", or as an injection score, is a different sentence, and
  one the receiving tool is entitled to overrule. A gate is a place that asks permission and obeys
  the answer; anything else is a suggestion with a hook's costs.

- **Kiro's terminal agent can deny, and one unknown keeps it out.** Not the IDE — that was the wrong
  target: the IDE's hooks live in a workspace's `.kiro/hooks/` and `setup` writes user-level files.
  `kiro-cli`, from the Amazon Q Developer CLI lineage, is the gateable one. Its `PreToolUse` is the
  only blocking event, it rejects with **exit code 2 and the reason on standard error**, and any
  other code lets the action proceed — which is Estigia's own rule about a broken hook, and its
  existing exit-code dialect. The payload is `{hook_event_name, cwd, session_id, tool_name,
  tool_input}`: Estigia's shape exactly, with nothing to rename.

  What is missing is where the hook is registered. Kiro's own reference documents the events and the
  exit codes and **not the configuration path**; the only evidence for one is a third-party
  integration naming `~/.kiro/agents/<name>.json` and the key `preToolUse` — lower case, where the
  reference writes `PreToolUse`. Two unknowns, and both fail silently: a key in the wrong case is a
  hook that parses and never fires, and `<name>` is the operator's *default agent*, whose file
  Estigia cannot name. Settling it needs a real `kiro-cli` installation, not another documentation
  page. Everything else is already built.

- **An extension point is not automatically a place to deny.** Three were checked and refused for
  the same reason, and it is worth stating once rather than three times. Claude Code's
  `WorktreeCreate` *replaces* git's behaviour and fails creation when the hook says nothing, so
  registering there to deny would make Estigia responsible for making every worktree. Goose's
  classification API is an HTTP service that labels text as prompt injection. OpenHands'
  `SecurityAnalyzerBase` is an in-process Python class returning a **risk level**, and what happens
  next is the user's confirmation setting. Estigia's refusal means *this run holds no claim covering
  this write* — rendering that as "high risk", or as an injection score, is a different sentence, and
  one the receiving tool is entitled to overrule. A gate is a place that asks permission and obeys
  the answer; anything else is a suggestion with a hook's costs.

- **Kiro can deny and is not here yet, on purpose.** Its `PreToolUse` blocks on exit 2 and hands
  standard error to the agent — the same shape the retired Windsurf adapter used — and its steering file at
  `~/.kiro/steering/` is global and always loaded. What is missing is where its *hooks* live for
  every workspace: only `.kiro/hooks/` inside a workspace is documented, and `setup` writes
  user-level files. Adding it ungated would buy a contract nobody enforces, which is the state
  Estigia exists to leave. The one thing to check is whether `~/.kiro/hooks/` is read; if it is, this
  is a table entry and a `GateSpec`, and nothing else.

- **The OpenCode plugin does not see subagents.** Measured on OpenCode 1: `tool.execute.before` is
  reported not to intercept tool calls made by subagents spawned through the task tool. OpenCode 2's
  `execute.before` has not been measured for the same hole. A gate with a hole is still
  a gate; a gate whose hole nobody mentions is a lie, so it is written into the plugin file itself.
- **The OpenCode plugin costs a process per gated call.** It shells out to the binary for `edit`,
  `write`, `patch`, `multiedit`, `notebookedit`, `update`, `bash` and `shell` — named rather than `*`,
  because waking a process for every read would be a cost paid thousands of times to answer "not
  mine".
  `notebookedit` and `update` were missing from this sentence for as long as it has existed, which is
  the shape of drift this file is most exposed to: the list is crossed against the classifier by
  `the_plugin_gates_the_tools_the_classifier_judges`, and prose naming the same list is crossed by
  nothing.
- **The push guard is a guard rail, not a lock.** `git push --no-verify` bypasses it, so does a push
  from another checkout of the same repository, and so does anything that writes refs without
  invoking hooks. A guard rail that claims to be a lock is worse than one that does not.
  - **The operation schedule is observed through the same openings.** An enabled `Git operation
    window` denies commits, ordinary pushes, and branch creation at every Estigia entry point —
    the shell classifier, the pre-push guard, and `publish_review` — and nothing
    else enforces it. `start_branch` is no longer one of them: while the window is closed it makes
    the local branch and the worktree and queues the link to the issue. `git push --no-verify` skips the guard, direct object and ref writes never
    reach the classifier, hosting APIs never reach any hook, and another clone holds no copy of
    the row at all: the repository override lives in one clone's git directory. A quiet period is
    therefore observed through instrumented entry points, never proven across the repository. An
    enabled value that does not parse, and a clock that cannot be read, deny the protected
    operations rather than falling back to unrestricted — and ordinary file edits, tests, builds,
    and worktree operations on an existing branch stay available either way. The weekday and the
    minute are the ones the machine's local clock reports, so a changed `TZ` or system zone moves
    the window with it.

## The push guard, the MCP server and the state a run keeps

- **Nothing adjudicates what a push is aimed at.** git hands a `pre-push` hook the refs it is about
  to write, on standard input, for free. They are read and written into the ledger, so the record
  names *which* push was decided on rather than only that one was — and no decision is given them
  **to classify the push itself**: `guard::decide` now takes the checkout *and* the refs, and what
  it adjudicates is still the three words `git push`, the same whichever branch the refs name. So a
  run holding a live claim may `git push origin branch-a:main` — the local ref, `refs/heads/branch-a`,
  is the run's own branch, so narrowing (below) selects that run's holder same as any other push of
  it — and the guard allows it as the same three unaimed words `git push`: the byte-binding check
  never fires, because `git push` is deliberately absent from the delivery population — *publishing a
  review target is how a run reaches review, and pushing after one is how it fixes what the review
  found*. That reasoning assumes a push is a step **towards** delivery. A push aimed at the base
  branch **is** the delivery, and it never runs `git merge`, so no verdict is ever asked for. For the
  adapters that get the contract and no tool gate (ZCode and DeepSeek Harness), this hook is the only enforcement there is.
  Measured by `what_a_push_is_aimed_at_is_recorded_and_not_adjudicated`.

  **Under `Workflow: assisted` the destination is checked, and everywhere else it still is not.** An
  assisted run's push is admitted only when every line git names has the run's own branch as both
  its local and its remote ref (`harness::assisted`, measured by
  `a_push_is_admitted_only_for_the_runs_own_branch_and_only_when_its_refs_are_known`). That is the
  one place a push's aim decides anything; the paragraph above describes every other run, and still
  holds for them.

  Narrowing (below) changes what this gap used to catch incidentally. Before it existed, any local
  ref other than one resolving to exactly one holder left every several-holder checkout refused
  `several-runs-hold-this-checkout` regardless of what the push was aimed at, so a base-branch push
  from a several-holder checkout was blocked by the ambiguity refusal, not by anything reading the
  destination. Narrowing lifts that incidental block for a push whose local ref resolves to one
  holder's own branch — measured against real git in a throwaway repository with a bare remote and a
  stdin-dumping `pre-push` hook: `git push origin branch-a` and `git push origin branch-a:main` both
  write the local ref `refs/heads/branch-a`, so both narrow to branch-a's holder and reach the same
  unaimed decision. `git push origin HEAD:main` writes the literal `HEAD` as its local ref, which is
  not `refs/heads/<branch>`, so it still fails narrowing closed and stays refused under several
  holders.

  It is not closed here because closing it wrongly is worse than the gap. The check needs the base
  branch and the `Integration mode` setting — under `trunk` the base branch is exactly where work is
  meant to land — and a boundary that refuses a legitimate push is how a harness gets uninstalled,
  which gates nothing at all.

  What *is* now read from those refs is narrower, and closes a different gap: **who** several
  holders' refusal belongs to, not **what** the push is. A git hook has no session to mint a run id
  from, so `holders_of` is all the push road has — and when two or more runs held one checkout it
  refused `several-runs-hold-this-checkout` even where the push named a branch exactly one of those
  runs' isolated worktrees was checked out on.
  `adjudicate_action` now takes the local refs as a fifth argument and, before the tracker
  reconciliation, narrows the holder list — through `git worktree list`, read fresh rather than
  trusted from any pointer, and stripped of every inherited `GIT_*` variable the same way every other
  proof git command in this crate is, because a `pre-push` hook already runs with `GIT_DIR` set.

  **The rule is not a union over whatever happens to resolve.** Every ref has to resolve, to one
  worktree, owned by one holder, and every ref has to agree on that same holder; anything else
  returns the holder list unchanged, for the refusal that follows to name.
  `narrow_to_pushed_branch`'s own doc comment in `guard.rs` points here rather than repeating the
  enumeration. It fails closed on:
  - an empty ref list, or a local ref that is not `refs/heads/<branch>` — a tag, `HEAD`, a raw sha, or
    the literal `(delete)` git sends for a branch deletion, none of which can be attributed to a
    worktree. Measured, not assumed: `git push origin v1` (a tag) writes `refs/tags/v1`,
    `git push origin HEAD:main` writes `HEAD`, `git push origin --delete topic` writes `(delete)`,
    and `git push origin <sha>:refs/heads/x` writes the bare sha;
  - a listing that cannot be read or parsed;
  - a branch carried by zero of git's own registered worktrees, or by two or more of them —
    `git worktree add --force` or `git checkout --ignore-other-worktrees` can put one branch in two
    worktrees at once;
  - a registered worktree owned by zero of `holders`, or by two or more of them — two of this
    machine's own pointers can name one worktree even though `parse_worktree_listing` refuses a
    *repeated path* from git's own registry; `skill/references/repository-delivery.md` names this
    directly: *"registration is not ownership."*
  - two refs landing on different holders.

  Only when every ref lands on the one and same holder does narrowing return that holder alone —
  never two candidates, and never a guess between them. An earlier version of this rule did both:
  it dropped whichever ref did not resolve rather than failing the whole narrowing closed, and it
  took the *first* registry entry carrying a branch rather than requiring exactly one, so a
  `git push origin main <holder's own branch>` on a machine with three live holders was refused for
  `main` alone and **allowed** for the same line plus the holder's branch — a base-branch write
  smuggled through on the back of a ref that happened to resolve. Both are the roads enumerated
  above now; `harness::guard::tests` drives each one, including the two-ref shapes nothing in the
  checkout exercised before.

  Two edges stay outside all of this. The run-id-less **tool** road — the OpenCode plugin's
  `tool.execute.before` when its host handed it no session id and no run id was exported around the
  plugin's process, and `estigia gate <tool>` invoked with no identity at all — asks the same
  `adjudicate_action` with no refs to narrow on, because a tool call carries no destination ref to
  read; that gap is separate, and this change does not touch it. And `guard::decide` is the one
  production caller: `cli::mod::git_hook`'s `PrePush` arm calls it directly rather than building its
  own copy of the `Action::Boundary` literal and calling `adjudicate_action`, and
  `tests/pipe/gate_hook.rs`'s `a_pre_push_hook_is_decided_on_the_holder_the_pushed_branch_names` drives that
  exact wiring with a real `pre-push` stdin — the first test in the checkout to do so with more than
  one holder in play.
- **The push guard refuses to replace somebody else's hook.** A `pre-push` is a test runner or a
  secret scanner as often as it is nothing, and Estigia will not take away a check somebody relies
  on to add one they did not ask for. It says so and names the way out.
- **The MCP server has never met a real client.** Every response is checked against the JSON-RPC
  envelope rules a client's parser enforces before any content matters, and the server is driven
  through a full session in tests. That proves the envelopes are legal and the answers are right; it
  does not prove a particular client is happy with them, and the protocol subset is a subset.
- **The cost avoided by hand-writing this server instead of using `rmcp` is asserted, not
  benchmarked.** Leteo uses `rmcp`; this crate does not, because the same binary answers a
  `PreToolUse` hook on every edit, and a `tokio` runtime spun up per process was judged too
  expensive to pay that often. Synchronous stdio JSON-RPC over a fixed, small tool list avoids
  spinning one up at all — but nothing here has measured what either approach costs per invocation;
  the judgement rests on the shape of the workload, not on a number.
- **The renewal window is a cadence, and a cadence is a gap.** A routine write inside two minutes of
  the last answer is not re-verified, and neither is a Claude Code `Stop`: it used to read the tracker
  at every turn end, which with the claim read-back's own reads spent the GitHub GraphQL budget of a
  whole account, so it now keeps a successful answer for the window and records its own.
  A claim lost inside it keeps the turn going once more per window. Only a successful read is kept;
  one that failed or answered unknown is read again. The Codex `Stop` path and `estigia goal` read the
  tracker on every call and are not covered by this. A claim lost during those two minutes is a claim the gate does
  not catch until the window closes or a boundary arrives.

  **Claim validity is the whole of what it excepts.** The window used to except the control surface
  as well — its `Allow` answered above `control-surface-not-installed`, so for two minutes a routine
  write went through with no `SKILL.md` on disk at all. The contract check now sits above the
  window, and what the window caches is now only what it ever measured: that the tracker answered
  about this issue recently. A boundary never consults it at all, and an unreadable control surface
  permits no write inside it either.
- **A rate limit is named, never waited out, and its reset is only as readable as `gh` makes it.** A
  read that fails with `gh` saying a primary or secondary rate limit was exceeded answers
  `rate-limited` (exit `3`, still a refusal and never clearance) instead of `read-failed`, and the
  detail carries a reset instant only when `gh`'s own stderr printed one; otherwise it says the reset
  time is unknown. Nothing asks GitHub for the reset (`rate_limit` is itself a call), nothing retries
  — a retry spends the budget being waited for — and a rate-limited *write* is still `write-failed`,
  because it may have landed. The stderr is matched on the phrases `gh` prints today; a rewording
  falls back to `read-failed`, which is the safe direction.
- **A JSON file Estigia edits keeps its shape and loses its shaping.** `settings.json` and
  `.claude.json` have to be parsed to be edited safely, and four things are read off the original
  and given back: the indentation, a byte-order mark, the line endings, and whether the file ended
  with a newline. Key order is preserved too — `serde_json` is built with `preserve_order` for
  exactly this. Measured rather than argued: every operator file of every adapter, written the
  ordinary Windows way, comes back **byte for byte** through an install and an uninstall, held by
  `every_agents_own_files_come_back_byte_for_byte`. The readers (`is_gated`, `exposes_tools`,
  `tools_start_the_server`, `wiring::narrowed`, `wiring::wires`) parse through the same
  `render::parse_settings`, so a file whose mark Estigia kept still reads as gated and tooled, held by
  `a_bom_settings_file_still_reads_as_gated_and_tooled_after_setup`.
  What is *not* kept is the formatting inside the document. A hand-written
  `"allow": ["Read", "Grep"]` on one line comes back over four, and a blank line between two keys
  is gone — the value is re-serialised, and only the four properties above are read off what was
  there. So a diff on a file Estigia edited shows the entry it added, and shows the operator's own
  arrays reflowed around it. Markdown is spliced textually and does not go through any of this.
- **`skills/` is left behind, even empty.** It is the agent's namespace, not a file Estigia created.
  What comes out is the tree that was written.

## What the gate covers

- **The gate covers two directories: the checkout the claim was made in, and the isolated one
  `start_branch` created or a resumed run adopted.** A run that makes its own worktree by hand — one
  git has never registered, or whose ownership marker names no issue — gets that write gated but the
  directory is never recorded, so subsequent writes there read as `Outside`. Use the tool, or resume
  through a checkout the transport already knows about.

  *Two* is now what the isolation step writes, rather than what it assumes it will find. It used to
  record only the worktree, which is correct whenever the claim already recorded a checkout and
  wrong in the one shape nobody has to do anything strange to reach: a `claim` whose tracker write
  lands and whose readback fails returns before the pointer effect runs, so no checkout is recorded
  at all. From there the dispatch guard's own precondition — *a run that has claimed nothing has
  nothing to be outside of* — lets `start_branch` through, and the worktree it writes becomes the
  run's **only** covered directory. The server that made the call is then outside its own run, every
  later call is refused `run-id-names-another-checkout`, and repeating it cannot help: an MCP
  server's directory does not change when a child command uses another one. The only way out
  observed in the field was restarting the agent from a path the workflow had just created, which
  loses the live context and can mint a different runtime identity — the thing claim attribution
  exists to prevent.

  So `start_branch` fills the claim's checkout in as well as the worktree, and fills rather than
  overwrites: a run whose claim named checkout A keeps A when a server standing in B calls under its
  id, because refusing B for that run is exactly what the guard is for. It is not coverage
  manufactured from a client's path — `start_branch` verifies the claim against the tracker from
  that same directory before it creates anything. Measured by
  `one_server_survives_the_isolation_it_created`, which drives one real server through both calls
  down one pipe, because no per-call `GateContext` can pose *the same server asking twice* — and
  which reads the pointer **between** the two, because the renewal below fills the same field by its
  own route. Asserting only at the end measured the pair and not the halves: with the check at the
  end, removing the isolation's line left that test green while the unit test beside it reddened.
  Two independent reviewers of this change found that, separately, by deleting the line and running
  the suite.

  **And a run already in that state gets itself back**, which prevention does not do for the runs
  that are in it. Two things were needed. The refusal above measured `covered().count()`, which is
  the right question asked of the wrong field: the worktree is the *additional* directory a claim
  covers, never the one that says where it was sworn. So a record holding only a worktree could
  ground a refusal — and an incomplete record is not a narrower claim, it is an unknown one. It
  measures `repo_dir` now, so a record that never named its checkout stands aside exactly as a run
  that has claimed nothing does. Nothing is widened for a record that *does* name one: a foreign run
  id, an unrecorded worktree and a directory outside the coverage are refused as before, and
  standing aside is not clearance — the call still goes to the tracker, which is the only thing that
  adjudicates.

  Then the way back is a call the contract already requires. A renewal answered `ok` is the tracker
  saying, at that moment, that this run is the live holder of this issue in this state — the same
  fact `Swear` writes, from the same authority — so the pointer is completed from it rather than
  left broken until something writes to the tracker again. It takes no tracker **write**, which is
  what makes it reachable during exactly the outage that causes the damage: the state is produced by
  a `claim` whose write lands and whose readback fails, and during that outage there is no write to
  be had. Filled and never overwritten, and unable to invent authority, because a run that does not
  hold the issue is refused before the pointer is touched. Measured by
  `a_renewal_completes_a_record_the_tracker_has_just_agreed_with` and, as a process,
  `a_stranded_run_recovers_from_the_checkout_it_is_running_in` — which asserts both halves, since a
  run readmitted on an empty record is a run whose writes are still measured against nothing.

  **The same renewal also discovers a checkout this run never built at all.**
  Everything above repairs a run stranded mid-`claim`, from a worktree that run *created* itself.
  A run that never calls `start_branch` — because a previous session's checkout, or the one its own
  `reclaim` just took over, already exists — used to leave `worktree` `None` for the whole of its
  life: the same hole this entry opens with, reached from the other side. The run swore to a
  checkout it does not edit, and the one it does edit was never on the pointer at all, so `covered`
  answered `Outside` for every write the run actually made. `claim`, `reclaim` and every renewal now
  read every checkout `git worktree list` has registered for this repository, find the one whose
  ownership marker names this issue, and fill `worktree` from it — the same fill-never-overwrite
  discipline as everything else here.

  Two shapes it answers by recording nothing rather than guessing, because a repair is not an
  authority: **more than one registered checkout naming the same issue** is not resolved by
  preferring the newest or this run's own id, and neither is **a read that fails**. The scan's `?`
  stops at the first one, so a failed listing and an unreadable marker do not read the same: the
  listing is one read for the whole clone, but a marker is read once per registered checkout, and an
  unreadable one anywhere among them — not only in the checkout it belongs to — aborts the scan for
  every checkout still unread, disabling discovery for every issue and every run this clone holds
  until that one marker reads again. Both leave `worktree` exactly as it was, and neither turns
  an otherwise-successful renewal into a refusal — discovery rides on a call that already succeeded
  and is never itself a gate. What it cannot see at all: a checkout git never registered, or one whose
  ownership marker was never written in the shape `read_ownership` reads — the same "use the tool"
  the entry above already says, now stated for the read side rather than the write side. Measured by
  `a_renewal_records_the_checkout_a_resumed_run_adopted`, `a_reclaim_records_the_checkout_it_takes_over`,
  `two_adopted_checkouts_are_not_conflated_into_one_holder`, and
  `discovery_records_nothing_it_cannot_prove_and_refuses_nothing_on_its_own_account`, the last of
  which drives all three refusals — the unreadable marker, the ambiguous pair, and the failed listing
  — in one place, on a machine where its fixture succeeds; two of its three blocks are skipped rather
  than returned from where it does not, which the fixture entry above states. The central claim —
  that a write inside the discovered checkout is gated once the
  fill records it, rather than passing through ungated as `Outside` — is measured by
  `a_write_inside_an_adopted_checkout_is_gated_once_a_renewal_records_it` and its push twin
  `a_push_from_the_adopted_checkout_stops_reading_as_unsworn_once_recorded`.

  **One thing here is pinned on one platform only, and the suite does not say so.** The discovered
  path is canonicalised before it is recorded, so this field's two writers spell one directory one
  way — `start_branch`'s and discovery's. Removing that canonicalisation reddens the byte assertions
  in the two recording tests **on Windows and nowhere else**, because Windows is the only platform
  where the two spellings differ at all: `git worktree list --porcelain` reports forward slashes
  there, while `canonical_worktree_path` returns native ones. On Linux and macOS that helper is
  `canonicalize(parent).join(name)` followed by a prefix strip its own doc calls an identity there,
  so for an already-resolved parent it returns its input byte for byte and the mutation is invisible.
  The lane runs `windows-latest`, so a regression is caught by the machine — but the local run a
  contributor types on Linux is not what catches it.

  **What the renewal's fill accepts on no directory evidence.** `start_branch` at least performs its
  tracker read from the directory it then records; `verify_claim` performs the same read and neither
  read binds a directory at all, so the renewal takes the caller's working directory as coverage
  because it is the only one on offer. Run ids are public — every claim comment carries one — so a
  call under run A's id arriving from directory B, while `A.repo_dir` is unset, writes B into A's
  record permanently, and A's own server is then refused in its real checkout. That is this defect
  in mirror image, and it is not closed: what bounds it is that the shape it needs is the one the
  isolation fix stops producing, and that the alternative — not filling on renewal — is the stranding
  itself. Deleting the pointer is the only correction. Both reviewers raised it and neither blocked
  on it.

  Discovery reaches the same call from the same unbound directory: `discover_adopted_worktree` reads
  `git worktree list` against `context.repo_dir`, which is this same caller-supplied directory B, not
  a directory anything has verified is A's. The identical shape can plant a `worktree` as permanently
  as it plants a `repo_dir` — whichever checkout B's clone happens to register for A's issue becomes
  A's record, read from a repository A's own server never touched. Bounded the same way and by the
  same argument, and closed the same way: deleting the pointer.

  **And the same rule is now spelled two ways.** The dispatch guard asks `repo_dir.is_some()`; the
  write gate in `src/harness/mod.rs` still asks `covered().count() > 0` about the same question. The
  semantic argued for above — *an incomplete record is unknown rather than narrow* — applies verbatim
  to the second, where the direction of failure is the opposite: a worktree-only record makes writes
  in the base checkout read as `Outside` and go ungated rather than refused. Not a regression, and
  not made worse here, but this repository's own rule is that a fact written twice is a fact that
  will disagree with itself, and these two now do. Unifying them is a change to the write gate and
  belongs to its own issue.

  The claim retry that this entry used to leave open is closed: a live marker for this same run is
  adopted rather than refused, so the pointer can be completed without a second comment. A live
  marker with no adoptable operation id is still `already-owned-by-different-operation`.
- **Two checkouts are told apart by resolving them, and by case when they will not resolve.**
  `canonicalize` answers with the real spelling on disk, so a live directory spelled two ways is one
  directory whatever the operator typed. A path this process cannot resolve has only its spelling
  left, and there the answer is folded the way Windows folds — the whole way, not only ASCII. This
  entry used to record the ASCII half as a residual, on the strength of a code comment claiming the
  transport folded that way too. It did not: `branch::spelling` calls `to_lowercase()` and the
  retired binding's `fold_case` called `.lower()`, and **both of those ran** — the harness was Rust
  and the transport that executed was Python, so two live components folding differently answered
  *is this the same checkout* differently. One of the two is gone and the rule that survived is the
  wider one, which is the direction that keeps a checkout from being mistaken for somebody else's.
  Measured by
  `a_checkout_this_process_cannot_resolve_is_still_one_checkout`, whose corpus differs by an
  accented pair rather than an ASCII one, because an ASCII pair cannot tell the two rules apart.
- **A branch differing from a registered one only by case or Unicode normalization is refused, on a
  filesystem that distinguishes them too.** `worktree_path` composes `<branch>~<run-id>`,
  and the run ID is lowercase by construction while the branch is only separator-encoded — so
  `Fix-1-a` and `fix-1-a` with one run ID compose `/w/estigia~Fix-1-a~run-x` and
  `/w/estigia~fix-1-a~run-x`, two strings and one directory on the filesystems this crate's operators
  run (default APFS on macOS, NTFS on Windows). On such a volume the second run's path exists — it is
  the first run's checkout — but the registry is keyed by `normalise_path`, whose Unix spelling is hex
  of the bytes and so folds neither case nor normalization, so the lookup missed and `may_occupy`
  answered `worktree-path-occupied` with `occupied_by_branch: null`. That is the orphan diagnosis,
  and its action invited removing the first run's live checkout.
  `may_occupy` now compares the **spellings** of the composed path and every registered worktree —
  separators unified, then NFC-normalized and case-folded — and refuses a collision as
  `case-insensitive-branch-collision` with an action that names both branches and says not to
  delete. The fold is deliberately independent of the filesystem: on a volume that distinguishes the
  two spellings they are two directories and they are refused anyway, the conservative direction,
  because accepting them there would leave the hazard standing on the volumes that fold. The composed
  `<branch>~<run-id>` shape and `SCOPE_JOIN` are unchanged. The fold was extended to
  normalization: `é` as U+00E9 and `e` plus U+0301 are different byte sequences but one directory on
  default APFS, which is normalization-insensitive, so the same orphan diagnosis was reachable through
  an ordinary accented branch name. The fold normalizes, lowercases and normalizes again: the first
  pass cannot compose a capital base with no precomposed accented form, so `J` plus U+030C stays two
  code points, and only after lowercasing changes the base to `j` does the second pass compose it to
  `ǰ` — the one code point the composed spelling already was. The case half is still
  `str::to_lowercase`, not full Unicode case folding, so a pair the volume's case folding merges but
  `to_lowercase` does not — `Σ`/`ς`, `µ`/`μ`, `ſ`/`s` — is not refused; that limit is separate and this
  change does not touch it. Measured by
  `two_branches_differing_only_by_case_are_refused_rather_than_called_an_orphan` and
  `two_branches_differing_only_by_normalization_are_refused_rather_than_called_an_orphan`, which
  compose the pair through `worktree_path` and read the reason back, and by
  `a_case_only_difference_is_the_collision_and_a_separator_one_is_not`, which holds the Windows
  separator case apart from it and the case-plus-normalization pair with it.
- **`worktree_path` substitutes placeholders in one pass and refuses a component `trim()` would change.**
  The template was resolved by four chained `replace` calls, so placeholder text inside
  an earlier value was substituted again by a later one — branch `a<run-id>` and `aR` under run `R`
  composed one path. Separately, the component was passed through `trim()`, which strips Unicode
  whitespace git allows in a ref name (U+00A0, U+2003), so `\u{a0}fix` and `fix` composed one path.
  The template is now scanned once for `<repo>`, `<branch>`, `<run-id>` and `<issue>` and substituted
  text is never rescanned; an unknown or unclosed `<…` is left as written. A component with leading
  or trailing whitespace of any kind is refused as `unsafe-worktree-component`, which also means an
  ASCII-padded branch or repository name that used to be trimmed silently is now refused. Measured by
  `placeholder_text_inside_a_branch_is_never_substituted_again` and
  `a_component_that_trim_would_change_is_refused_and_not_given_its_twin`.
- **The closed-window refusal covers the transport's own spawns, and the gate reads an agent's line by the same matcher.**
  While the git operation window is closed, `window::tripwire` refuses a `gh` write or a
  `git push` that `transport::run` is asked to start, with `outbox-bypass`. A `gh` line an agent
  runs itself never passes through it; the gate classifies the line instead, through
  `window::is_outbound_write` over each command of the line, as an `Action::Remote` and denies it
  with `git-operation-window-denied` while the window is closed, and stands aside from it while the
  window is open, as it did before. `gh pr merge`, `gh pr create`, `gh release create` and `edit`,
  `git merge`, `git tag`, `gh issue create` and `develop`, and a mutating `gh api` were already
  classified and are now denied with it, and so are `gh cache delete`, `gh ssh-key` and `gh gpg-key`
  `add` and `delete`, and `gh repo deploy-key` and `gh repo autolink` `add`, `create` and `delete`
  (`the_gh_writes_outside_the_old_verb_table_are_remote_and_denied_while_closed`). The matcher is
  a table of the write verbs it knows, so a `gh` subcommand a later release adds reads as a
  non-write until the table names it; `release` and `project` are the two read-listed exceptions.
  What the matcher does not read goes through: an alias, a
  script, a shell variable or a command substitution that spells the call, and a `gh` word that is
  not the first of its command. A `gh api` call is judged from its own words, per command and without regard to
  case, so an attached `-XPUT`, an explicit `-X GET` with fields and a `-F` belonging to another
  command are all read as they run. A `gh api graphql -f query=…` that only reads stays a
  read, but one whose query comes from `--input` or an `@file` field is treated as a write in
  `api_writes`, because its content cannot be read from the words of the call; a mutation hidden
  behind a shell variable still passes. While it is closed, `create`, `comment`, `transition`, `claim`, `claim --takeover`,
  `unassign` and `start_branch` queue their GitHub writes instead of sending them, and a publication
  is refused with `git-operation-window-denied` and the number of writes waiting. Measured by
  `a_transition_while_closed_moves_the_store_and_queues_the_projection`,
  `a_comment_while_closed_is_queued_with_its_escaped_body`,
  `a_create_while_closed_is_held_under_a_provisional_number`,
  `a_claim_while_closed_is_recorded_and_its_projection_queued`,
  `a_takeover_while_closed_queues_its_projection`,
  `a_release_while_closed_queues_its_projection_and_its_evidence`,
  `a_branch_started_while_closed_is_made_locally_and_its_link_queued`,
  `a_branch_that_exists_locally_and_remotely_is_not_linked_while_closed`,
  `an_issue_created_while_closed_can_be_worked_from_claim_to_release`,
  `while_closed_a_gh_write_is_refused_before_any_process_starts`,
  `while_closed_a_git_push_is_refused`,
  `every_write_to_github_classifies_as_a_protected_operation`,
  `reads_from_github_stay_unprotected`, `a_remote_write_outside_the_period_is_denied` and
  `a_remote_write_inside_the_period_or_with_no_schedule_is_not_watched`. `release_ci` is refused with
  `git-operation-window-denied` while the window is closed, like a publication, because marking a
  pull request ready is a write the queue does not hold; measured by
  `release_ci_denies_outside_the_period_before_reading_anything_else`.
- **The window's scope is the repository a write is aimed at, and the gate's reading of an agent's
  own shell line still belongs to the checkout it runs in.** The transport resolves the pin
  `Context::repo` — `create`'s `repo` argument, or the tracker's or host's named repository — to a
  checkout whose `origin` names it: the caller's own first, otherwise one recorded among
  `known_repositories`, and reads that checkout's rows; a repository with no such checkout is closed
  rather than inheriting the caller's schedule. Measured by
  `a_create_aimed_at_another_repository_is_judged_by_that_repositorys_window`,
  `a_flush_readback_reports_the_window_of_the_repository_it_is_aimed_at`,
  `a_closed_caller_filing_into_an_open_repository_is_not_deferred` and
  `a_repository_with_no_readable_rows_closes_rather_than_inheriting_the_caller`. What stays open:
  the gate classifies a hand-written `gh` line through `harness::operation_window::check_protected`,
  which reads `GateContext.operation_window` — built by `cli` from the **caller's** checkout — so a
  shell `gh issue create -R <destination>` is still denied or allowed by the caller's schedule, and
  only the transport's own `create` and the flush moved. The destination lookup also reads each
  recorded checkout's remote through git, so a destination whose checkout is not registered, or
  whose `origin` is not a GitHub remote, reads as no rows and closes.
- **The store holds a write queue and one permitted rewrite of an entry.**
  Schema 9 adds `outbox` and `provisional_issues`, and the `entries_no_update` trigger now lets
  exactly one update through: moving an entry's `issue` from a provisional number to the real
  number `provisional_issues` records for it, every other column unchanged. Measured by
  `an_entry_may_move_from_a_provisional_issue_to_its_recorded_real_number_and_nowhere_else`.
  Provisional numbers start at `PROVISIONAL_BASE` and are allocated from one counter in `store_meta`,
  so two machines can allocate the same number and file the same issue twice.
  `Store::rekey` moves the rows of a provisional issue to its real number in one transaction, and
  `outbox::rekey_files` follows with the run pointers and the ownership record of the worktree each
  names; a branch or a worktree directory named with the provisional number keeps that name, and an
  ownership record in a worktree no pointer names is not found. Measured by
  `rekeying_moves_every_row_of_the_issue_to_the_real_number` and
  `the_ownership_file_of_the_pointed_worktree_follows_the_issue`.
  A provisional issue opens its ledger without any call to GitHub, which has no such issue, and the
  ledger reads as open; a provisional number the store never allocated, or allocated and never
  recorded, is refused. Measured by `a_provisional_issue_opens_without_asking_github_anything`.
- **The flush sends the queue in one line, and a stalled write holds everything behind it.**
  `outbox::flush::run` refuses while the window is closed, takes a per-repository lease in `store_meta`
  (`outbox-lease host/owner/name`, a pid and an expiry; a dead or expired holder is taken over; a flush of another repository does not hold it, and a write runs the flush only when its own repository has unsent writes, measured by `a_flush_for_another_repository_does_not_refuse_this_ones_write` and `a_lease_of_one_repository_does_not_keep_another_repository`), rekeys the pointers
  and ownership files, and then sends unsent writes oldest first. A write is marked `sending` before it
  is sent and `sent` only after its own readback: a comment is read back by the
  `<!-- estigia-outbox:{op_id} -->` marker it carries, a create by `gh issue view` on the number
  `gh issue create` printed, a link by the issue's Development sidebar, and a reconverge by the
  readbacks `transition` already makes. A write whose outcome cannot be read stays `sending` and stops
  the flush with `outbox-pending`; the next flush reads it first and sends it only when the readback
  says it did not land. A failure that learned nothing on a first attempt puts the write back as
  `queued` with the reason; once a write has been `sending` it is never put back. A link that
  `gh issue develop` cannot make, and whose sidebar read says the branch is not linked (a branch of
  that name already exists), is settled as `sent` with the `gh` reason in its detail and listed under
  `not_linked` in the flush report, the same verdict the open-window `start-branch` gives
  (`natively_linked: false`); a link whose outcome cannot be read still stops the flush. The line is global:
  one ambiguous write stalls every write behind it, including writes for other issues. A `sending`
  create is resolved by searching for exactly its title, and that search is the index GitHub updates
  after the issue exists, so a flush run seconds after an ambiguous create can find nothing and file
  a second issue; two issues with the title stop the flush instead of being guessed between. The
  search adopts only an issue the authenticated user (`--author @me`) created no more than five
  minutes before the row was enqueued and whose number the store does not already hold; an older
  issue, one the store holds, or one the search lists without a `createdAt` is refused as a candidate,
  so a same-title issue that is not this attempt's is neither adopted nor allowed to stall the row at
  `rekey`. A search that answers nothing, or anything other than a JSON list, is a failed read that
  keeps the row `sending`, not "no such issue". The bound is the enqueue time and not the first
  attempt, an issue another run of the same user filed under the same title after the row was
  enqueued is still adopted, and the `create` of an open window that left only a `pending-create`
  mark has no row time and so is bound by author and store alone. A
  reconverge rebuilds labels, board and owner from the store, so it overwrites an assignee or label
  someone changed on GitHub meanwhile. A comment keeps the marker in the text GitHub shows, and its
  timestamp there is the flush time. The readback of a comment lists the thread without a `since`
  filter, so a local clock ahead of GitHub's cannot hide a posted comment
  (`the_comment_readback_does_not_filter_by_the_local_clock`); the cost is that a thread of a
  thousand comments or more is refused as unread. Every `gh issue comment` and `gh issue create` the
  flush makes gives up after `WRITE_SECONDS`, well inside `LEASE_SECONDS`
  (`a_write_the_flush_makes_gives_up_before_the_lease_runs_out`), and a flusher renews the lease
  before it sends and before it settles a row, stopping without touching the row when another live
  process holds it (`a_flusher_that_lost_its_lease_does_not_mark_the_row_sent`,
  `a_flusher_that_lost_its_lease_sends_nothing`). What stays open: the renewal and the `gh` call are
  not one step, so a flusher that loses the lease in the instant after the check can still send once
  more, and a write that times out has an unknown outcome and is resolved by the next flush's
  readback, which for a create is the search whose index lag is described above. Measured by `the_queue_is_sent_in_the_order_it_was_made`,
  `a_created_issue_takes_its_number_before_the_next_write_on_it`,
  `an_ambiguous_create_stops_the_queue_with_the_rest_still_queued`,
  `a_create_left_sending_is_adopted_by_its_title_and_not_filed_twice`,
  `a_create_left_sending_does_not_adopt_an_issue_older_than_the_row`,
  `a_create_left_sending_does_not_adopt_an_issue_without_a_creation_time`,
  `a_create_left_sending_does_not_adopt_an_issue_the_store_already_holds`,
  `a_title_search_that_answers_a_non_list_does_not_file_the_create_again`,
  `an_empty_title_search_answer_does_not_file_the_create_again`,
  `a_comment_left_sending_that_already_landed_is_not_posted_again`,
  `a_reconverge_that_cannot_be_read_back_stops_the_queue`,
  `a_write_already_sent_once_never_goes_back_to_the_queue`,
  `a_link_gh_cannot_make_is_settled_with_the_reason_and_does_not_hold_the_queue`,
  `a_link_left_sending_that_gh_cannot_make_is_settled_and_never_requeued`,
  `a_flush_while_closed_sends_nothing_and_keeps_the_queue` and
  `a_lease_is_kept_by_a_live_holder_and_taken_from_a_dead_or_expired_one`.
- **The queue is sent before anything else writes, and a detached flush is the machine's, not the session's.**
  With the window open and a write waiting, `comment`, `create`, `transition`, `claim`, `unassign`,
  `start-branch`, `publish-review`, `republish-review`, `release-ci`, `draft-pr` and `ensure-states` run
  the flush first and are refused `outbox-pending` when it stops; the other operations do not, so a
  stalled queue does not block a read or a local-store write. The list of those operations is written
  down in `outbox::flush` and is not derived from what each operation spawns, so a new operation that
  writes to GitHub is unflushed until it is added. When nothing is waiting the check opens the store
  read-only and asks GitHub nothing. `estigia flush --detach` starts the same flush with no terminal in
  its own process group and reports the pid it started, not whether the flush then succeeded; the
  lease keeps it from running beside another. Measured by
  `a_write_while_open_sends_what_the_closed_window_queued_first`,
  `a_write_while_open_is_refused_when_the_queue_does_not_drain`,
  `an_open_write_with_an_empty_queue_asks_nothing_extra`,
  `a_publication_refused_while_closed_says_what_is_queued_and_what_runs_it` and
  `flush_detach_returns_at_once_with_the_process_it_started`.
- **"Offline" means the window is closed, not that the network is down, and what a closed window shows is the machine's own view.**
  Reads still go to GitHub while the window is closed, so `list_state` fails exactly as it does with
  the window open when GitHub cannot be reached; nothing here queues a read. `list_state` answers
  the state from the local store, as it always did, so a transition made while closed is already in
  it; it also lists the issues created while closed and not yet filed, marked `provisional`, with the
  title and labels the queued create carries, and `list_state --identities` lists their identity so a
  finding is not filed twice. Another machine, a person, and the board see none of it until the flush:
  the labels, the board column, the assignee and the comments on GitHub disagree with the store for
  as long as their writes are queued, and a second machine can file the same issue and take the same
  provisional number. A task whose work is a commit cannot be marked done while closed, because
  the commit that records it is denied by the window; a task closed by an `evidence` note instead
  is not. `start_branch` makes the local branch and worktree while closed,
  which it refused before, so a branch can exist locally that GitHub has not been told of. Measured by
  `an_issue_created_while_closed_is_listed_as_provisional_in_the_state_the_store_gives_it`,
  `a_listing_with_no_provisional_issue_carries_no_trace_of_one` and
  `the_identity_of_an_issue_created_while_closed_is_listed_so_it_is_not_filed_twice`.
- **The session hooks read the queue count and nothing else, so what they say can be out of date.**
  `SessionStart` and `Stop` open the local store read-only and count the unsent writes; they ask
  GitHub nothing, and they count every repository's writes, not only the one the session is in. With
  the window closed and a write waiting they say how many are queued and when the window permits a
  flush; `Stop` says it as a `systemMessage` and never blocks the stop for it. With the window open
  they start `estigia flush` detached and say so, and report that the flush was started, not that
  it succeeded: a session that ends at once leaves it running, and its failure is read from
  `estigia flush --status` the next time. The text of a session start with an empty queue is
  unchanged. Measured by `an_empty_queue_leaves_the_session_start_text_as_it_was`,
  `a_closed_window_says_what_is_queued_and_starts_nothing`,
  `an_open_window_with_a_queue_starts_the_flush_and_says_so`,
  `the_stop_hook_names_the_queue_and_never_blocks_for_it` and
  `the_stop_hook_with_an_empty_queue_answers_as_before`; a queued claim's owner and runtime label
  are projected from the store, not from the payload, measured by
  `a_queued_claim_projects_the_assignee_and_runtime_label_from_the_store`.
- **One row is read by nobody.** `Delivery preset` stores nothing of its own and only writes the
  rows it stands for, so nothing reads it at run time. `State names` is read by `assist`, `ready_pr`
  and `review_stages`, which move a Linear issue to the state it maps. `Corrections after ready` and `Ready by` are read by `draft_pr` and
  `ready_pr`, `External reviewers` by `review_stages`, and `Merge by` by `review_stages` and by `deliver`, which hands
  back at `authorisation` when it is not `agent`, all through
  `Context::layered`. `READ_BY_NOBODY` in `tests/honesty.rs` lists `Delivery preset` and fails when it
  is read and not taken off the list.
  - **`review_stages` reads, and decides nothing.** Its answer is a reading of the pull request at the
    moment of the call; nothing is recorded, so a run that does not ask again acts on a stale one.
    A check stage's comments are its author's **unresolved review threads**: a comment the author
    leaves outside a thread, or a summary review, is not read, and a thread somebody resolves without
    addressing it counts as addressed. For `any` and `org/team` a review from any account other than
    the author, a `[bot]` or a check stage's author satisfies the stage — team membership is not read.
    An approval completes a review stage only for the current head; GitHub does not expose the
    historical base that reviewer read. Exact-head polling, full thread conversations, guarded
    reply/resolution and their non-atomic mutation limit are in [assisted review](assisted-review.md).
    A list GitHub answers with a next page — more than a hundred threads, comments per thread,
    reviews, requests or check runs — refuses the reading rather than deciding from the first page.
    A blocking check that is not reported on the head holds the stages behind it
    and says it was never reported, because a check that has not started yet and a stage name that
    matches no check look the same from here; an advisory check never holds them. Measured by
    `src/transport/stages/tests.rs` against a fake `gh`, including that no review is requested while
    the cubic check is pending.
- **No other setting is read by nobody.**
  An operator sets thirty rows and `config list` reports all of them. Measured: `context.get` is
  called for the labels in `READ_BY_THE_TRANSPORT` — `worktree location`, `Review budget` (for the review forecast line) and `Git operation window`. The gate reads
  `Irreversible commands`, and the payload's prose names the rest of the acted-on rows. That once
  left `Delivery authorisation`, `Transition authorisation`, `Delivery route`, `Merge strategy`,
  `Review delegation` and `Model routing` read by nothing at all. `setup::Applies::Asked` said of
  three of them *"the contract asks, and the agent may still honour it, but nothing checks"*, and
  for three the contract did not ask either. The four rows that had exactly one
  answer — `Delivery route`, `Review delegation`, `Merge strategy` and `Review protocol` — were retired, and
  hard-coded what each meant, so they are no longer settings to read, and the list the guard
  declares is empty.

  The two authorisations are closed: `SKILL.md` now says, in the steps where a state moves and where
  a change is delivered, that doing it unasked needs the row's permission. `Delivery authorisation` is
  also read in code now: `deliver` hands back `authorisation` unless it is `auto`, and so does `Merge by`
  unless it is `agent`, both through `Context::layered`. A merge made by hand through the shell is still
  held only by the prose, so the row stops no run that never calls `deliver`. `Model routing` is closed
  too, and by a narrower claim than the row's name suggests: `setup` reads it for the
  `model:` and `effort:` lines of the definitions it writes, and the managed block of the **installed**
  `SKILL.md` now carries a sentence telling the agent to honour the row. It is generated rather than
  payload text — `selected_documents` builds it and `configuration_body` splices it between the
  markers — so it is in no file in this repository, which is why an earlier draft of this entry sent a
  reader to `skill/SKILL.md` to look for it and a later one sent them to the directive file, where the
  configuration block is never written at all. Five of the eight supported adapters share the neutral skill
  root — `adapters_on_the_neutral_root()` computes it, and `docs/what-it-writes.md` is held to the
  same number by a test — so they share one installed contract and one rendered sentence rather than
  each having their own, and the sentence is written for that: it names the host it means, Claude Code
  being the one adapter that receives the definitions `setup` installs, and leaves the routing value
  in the per-agent row, which that adapter's own file may move, rather than restating it.
  Comments in the
  source still say eight; that population carries the search rather than a count. It does
  **does** decide whether those two definitions exist — naming the role in `Model routing` is what
  installs it, and removing the entry retracts it, which is the change made when the
  `Delegated workers` row was deleted. Nothing here starts a model; what changed is that something
  reads the row. `Merge strategy` left
  the unread list when `closing_merge` read it. It was then retired, so
  delivery's merge commit is hard-coded and the row is gone: the list in
  `every_setting_is_read_by_the_gate_the_transport_or_the_prose` is empty, and the guard now fails on
  any row the gate, the transport and the prose all leave unread.

## doctor, and what it checks

- **`doctor`'s rows were measured as builders and not as report — closed, with the measurement.**
  Each row's own function was tested. The **assembly** was not: downgrading every `Broken` of one
  row family on its way out of `doctor::full` left the whole suite green for ten of the eleven. A row
  could therefore be built correctly and dropped, filtered or softened on the way into the report,
  and every unit test on its builder would still pass — which was not hypothetical, because `full`
  once failed to pass the home directory to `state_root`, and the push-guard row vanished entirely
  under a tracker with no transport.

  All of them can break now — `gate-version` joined them, after
  `stale-run-pointer`, and `transport` is gone rather than kept as the one row that never could:
  with Linear and Trello removed, the only tracker left always has an executable, so the row
  answered `ok` on every configuration and nothing else. All but two are forced through the
  binary, on states any machine can be put into: no skill; an install whose asset digest came
  from another build; a contract taken from under a registered agent; two installed roots whose
  machine-wide rows disagree; a gate and a tool server whose settings name a binary that is not
  there; a checkout with
  no remote; a `pre-push` hook that is not text; an unreadable stand-down; an unreadable run pointer;
  a ledger line saying a call went through ungated; and, for the one row that is about the machine
  rather than the installation, a search path with no `gh` on it.

  The other two are exemptions: named in `tests/pipe/status_doctor.rs`'s own `FORCED` list without being forced
  *broken* there, each with its own reason said inline rather than left as a silent gap.

  `stale-run-pointer` is one: `a_row_that_is_broken_comes_out_of_the_
  report_broken` names it in that list without forcing it broken. Forcing it needs a `gh` that
  answers `issue-not-open` for one specific issue number rather than the blanket absence the `gh`
  row's own forcing step already uses — a different fixture shape than every other row on this list
  needs, and one `harness::doctor`'s own module already builds through a scripted `gh`:
  `a_readable_stale_pointer_is_reported_and_names_the_release_command`,
  `a_readable_live_pointer_is_reported_fine` and `a_tracker_read_that_fails_is_reported_as_unread_
  not_stale` drive `stale_run_pointers` directly, with `stale-run-pointer` reported `Broken` (a
  stale pointer, and separately an unread one) and `Fine`. `harness::guard::tests` scripts `gh` for
  its own reconciliation tests but constructs no `doctor::Check` and never touches this row.

  `gate-version` is the other. The same fixture forces it against a checkout that shares no history
  with the binary under test — an unrelated repository built with a fresh `git init` — but that state
  is not the row's *broken*: `git merge-base --is-ancestor` cannot answer either *yes* or *no* about
  two histories with no object in common, so the row reports `skipped`, *could not be established*,
  the same *cannot tell* answer this row gives for a dirty build. Shares-no-history is not the same
  fact as older, and reading it as one is the inference this row was rewritten to stop making.
  Genuinely `Broken`, on a real ancestor pair, is `harness::doctor`'s own
  `gate_version_reports_fine_and_genuinely_stale_against_real_commits` — not forced through this
  binary either, the same shape as `stale-run-pointer` above. That name is the assembled,
  end-to-end version, and it may execute nowhere on a given run: it skips on a dirty build — every
  local dev loop — and again at its own `cat-file -e` guard when a local clone of a shallow
  checkout does not hold the binary's own commit, which is what CI's checkout gives it. What holds
  the `Broken` decision unconditionally is a separate, pure test on fabricated inputs —
  `an_older_binary_than_the_checkout_is_reported_broken_with_a_reinstall`, against
  `gate_age_verdict` directly. That holds what the arm *says* when it is reached, and not that it
  is reached: it is fed `Some(false)`, so it cannot see which end of the ancestry the call site
  asks for. Under the two conditions above — a dirty build, or a clone without the commit — the
  assembly is what goes unheld, and the assembly is where a swapped argument would report a
  current gate broken and a stale one current. Naming the two revisions rather than passing a
  slice is what stands in for a test there, and it is a weaker thing than a test.

  **A build that stops carrying its commit degrades to *cannot tell* everywhere, and nothing
  fails.** A `build.rs` regression that stopped emitting `ESTIGIA_COMMIT` — the
  whole of the identity this issue exists to add — would leave the suite green: `VERSION` becomes
  `0.1.0 (unknown build)` and the `--version` test still passes, the end-to-end stale test
  early-returns, and the pipe fixture takes its no-commit arm and asserts that degradation as a
  success. The failure mode is honest — every row answers `skipped`, never a false `ok` — and a
  *corrupted* commit is still caught on a clean build by the `Fine` assertion against the manifest
  directory. What is unheld is the absence.

  A row added later fails the same test until somebody forces it or says why they cannot.

  The same lens was then turned on the other two assembled pages, and found two more of the same
  shape. Blanking the **skill root** printed under a configured agent in `estigia status` left the
  suite green — the line that answers *which directory*, for somebody who has just been told their
  skill is out of date. And the whole of `estigia setup --companion <slug>` was crossed by nothing:
  its one `say!` could be replaced with a discard, and the unpublished companion could be answered
  with `cargo install leteo`, a command that would 404, without a single test objecting. Both are
  now read back out of the binary by `what_status_prints_is_what_somebody_needs_to_act_on` and
  `what_the_companion_verb_prints_is_crossed`.

  A row can also be wrong in the direction that reads as working. The `gate` row and `status`'s
  `gate on` both come from one question — *is Estigia's hook registered for this agent* — and
  `is_estigia_hook`, which answers half of it, could be made to say **yes to every entry** with the
  suite still green: the only files anything crossed it against held Estigia's own hook or held
  nothing at all. Between those sits an ordinary machine, an operator with their own `PreToolUse`
  hook and no Estigia entry beside it, and there a yes prints `gate on` over no gate. That machine
  is now built and read back by `somebody_elses_hook_is_not_this_agents_gate`.
- **Estigia cannot check who reviewed.** The contract requires that a review *"MUST NOT be
  performed by the context that wrote the change"*. The harness reads who recorded a computed
  verdict, and will not let the publishing run or a run that asked for review resolve its own
  handoff or supersede its own escalation. Where the reviewing run recorded its own verdict that is timeline attribution; where the
  claim holder recorded somebody else's, it is a name that run supplied. The judgement still belongs
  to the agent; Estigia records its outcome and receipt but does not observe who or what produced
  it.
- **Refusals about a tracker or host with no transport can no longer fire on GitHub alone, and are
  kept anyway.** Removing Linear and Trello left `Tracker` with one variant, and
  `RepositoryHost` is GitHub-only the same way, so both `transport()` methods answer
  `Some("estigia")` for every value that can exist. Production sites still branch on absence —
  tracker sites in `src/cli/mod.rs`, `src/harness/doctor.rs`, `src/harness/mod.rs`, and the
  per-operation match in `src/harness/mcp/mod.rs` (tracker-bound tools vs host-bound tools after
  the split) — and every one of those branches is unreachable while only GitHub is implemented. The
  tests that could only construct the missing-transport scenario were deleted rather than weakened;
  the guards were not. What the split *did* change is the shape of the MCP refusal: it is no longer a
  single top-of-`run_tool` check on the tracker for every tool; publication and receipt-bound
  review tools ask the host instead, so a future tracker without a transport can still run
  `publish_review` against a GitHub host.

  The removal of Linear and Trello also deleted `doctor`'s `transport` row, for the same arithmetic,
  so the two decisions need the distinction that separates them stated rather than assumed: **a refusal that cannot fire
  is inert and harmless, while a reported row that cannot vary is noise an operator reads every
  time.** The `Option` also still carries a real statement — that a tracker might have no
  executable — which is true of the design and would be true again the day a second binding ships
  with one. What is given up by keeping them is that nothing distinguishes a guard held for that
  reason from one nobody noticed had died, and this paragraph is the only thing recording which
  these are.

- **`doctor` is held at the function and at the composition, and not at the call site.** Two tests
  now cross that the tracker `doctor` reports on is the checkout's own and not the contract's:
  `the_tracker_in_force_is_the_one_this_checkout_names_not_the_contracts` gives the contract a named
  repository so both layers are distinguishable, and
  `the_stale_run_pointer_check_asks_gh_about_the_checkout_s_own_repository` composes
  `tracker_in_force` into `stale_run_pointers` and reads back which repository `gh` was asked about.
  Mutating `tracker_in_force` to answer from the contract alone reddens both.

  **Neither reaches `cli::doctor()` itself, and that gap is measured rather than supposed.** Replacing
  the call at `src/cli/mod.rs` with a contract-only read — leaving `tracker_in_force` correct and
  breaking only what the caller hands it — leaves the whole suite green: 1,251 lib tests and nine
  further suites, 0 failed. That function reads `std::env::current_dir()` directly and takes no
  parameters, so it is reachable only through a subprocess, and this repository's own convention sends
  `stale-run-pointer` the other way: `tests/pipe/status_doctor.rs`'s `FORCED` list deliberately exempts it and the
  row is measured in `harness::doctor`'s own module with a scripted `gh` instead.

  So a defect purely in that call site's arguments — the wrong root, the wrong directory, the two
  swapped — is caught by nothing. The original test that would have caught it drove the binary, and it
  was deleted in this change because its other assertions could no longer be posed. That is the
  trade this round made, and it is written here rather than left in a report.

- **A guard's copy detector lost half its grip in the same change, and stayed inert and unnamed for
  one round.** `tests/honesty.rs`'s `every_reader_of_a_transportless_tracker_asks_the_same_function`
  had two halves: a floor (at least four production call sites ask `Tracker::transport()`) and a text
  search for `["linear", "trello"]` written out as a literal pair — the shape of a second
  implementation of "is this tracker operable" that would drift from `transport()` silently. Removing
  Linear and Trello took both names out of every `.rs` file under `src/` — measured directly,
  grepping for them returns nothing — so the second half became permanently unfalsifiable: no edit
  anyone could make would ever fail it again. The doc comment above it was left stale too, still
  naming "the doctor's row" as one of four callers after this same change deleted that row.

  Both are fixed now: the doc comment names the six current call sites by file and line instead of a
  soft count, and the copy detector is deleted rather than re-aimed. Re-aiming it at "some future
  second tracker decided by a hand-rolled match instead of `transport()`" was considered and rejected
  — there is no real violation of that shape to prove such a detector against today, and a guard that
  cannot be shown red is not measured, only asserted. What this gives up: a tracker added later with
  no transport could again be decided by a second, drifting implementation, and nothing in this suite
  would catch it until the guard is rewritten against that tracker's actual shape.

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
  repository it found is the one the issues live in.

  The skill comparison is against this binary's embedded copy, so a stale binary reports its own
  age as current — it cannot detect that a newer build would write something else. An install whose
  operator table no build recognises reports unreadable rather than current, and the remedy names
  the row rather than `sync`. Same mechanism as the installed judge definition, which is
  linked rather than restated.

- **A host's automatic mode keeping Estigia's gate is pinned by tests and measured live on three hosts of four.**
  The rule: an automatic mode approves the host's own prompts, and Estigia's decisions are never
  among them. Checked on 2026-10-14 in a scratch repository with no claim held and `Issue
  requirement` at `ask`, asking the model to write `hello.txt`: OpenCode v2.0.26 under `opencode run
  --auto` and Claude Code 2.1.296 under `claude -p --permission-mode bypassPermissions` both refused
  `issue-required` and created no file, and Pi 1.1.0 has no permission prompts at all, so its gate
  was unchanged and a subagent child's writes were refused too. **Codex 0.159.2 was not run:** the
  local install was not signed in (`401 Unauthorized`), so nothing here says what Codex does under
  `--full-auto` or the `never` approval policy, and the Codex check stays open until a signed-in run
  records it. What the suite pins instead is the part Estigia owns: with the mode's fields in the
  hook payload (`permission_mode` of `bypassPermissions`, `auto`, `dontAsk`, `acceptEdits`;
  `approval_policy` of `never`), the answer is byte-identical to the answer without them, a
  deny where the requirement is `required` and, where it is `ask`, an `ask` on Claude Code and a deny
  on Codex; and the generated OpenCode plugin throws on exits `1` and `2` with automatic-mode fields on
  the hook input, in both generations of its loader, and registers no permission hook. What no test
  can pin is the host's own ordering: a host release that put its permission layer before hooks or
  plugins would turn a refusal into an approval, and only a live run against that release would see
  it. Under `ask`, Claude Code's hook answers `ask`, and the live run saw the file not created; it
  did not separate a host that put the question to nobody from one that refused it. The `permission` row
  of `doctor` reads the setting from the host's settings files only (`permissions.defaultMode` for
  Claude Code, from the project's `settings.local.json`, then its `settings.json`, then the user's; `permission` in `opencode.json`; `approval_policy` and
  `sandbox_mode` at the top level of `~/.codex/config.toml`), so a mode chosen on a command line
  (`--auto`, `--permission-mode`, `--full-auto`), a Codex profile, a managed policy file or an
  environment variable is invisible to it, and `no permission mode is set` means the files say
  nothing, not that the host runs its default. A file that does not parse is reported as unread, not as
  a mode. Other hosts get no row.

- **Installed-asset staleness is a recorded digest comparison, and a mismatch is Broken with `sync`.**
  Every install records an aggregate digest of the embedded skill assets — sorted
  paths, NUL-delimited, metadata-free, so two builds from identical sources agree — and `doctor`
  compares it against the running build's. A mismatch is a stale install and names `sync`; no
  record at all is skipped, not cleared, because absence says nothing about the files. The same
  decision at the reviewer-definition door reads the other way round: bytes that moved since the
  install wrote them name an edit, while a record that predates digests cannot tell an older
  build from one and says unknown rather than picking. What neither answers is whether the
  running binary itself is current — the comparison is against its own embedded copy either way.
- **The `gate-version` row compares what is installed against the checkout, not what any host
  actually invokes.** The comparison is the running binary's embedded commit against the
  checkout's `HEAD`: it establishes whether the checkout's `HEAD` is a point *this* binary — the
  one answering the command — already contained; a **no** does not establish that this build is
  earlier, only that the checkout's head is not a fact of the history this build already held. It
  does not discover which path another agent's hooks invoke; a host wired to a different executable
  is the `gate` row's *not this build* warning, a separate question the count above keeps separate.
  It cannot tell when the build carried
  no commit (a published crate, or a build made without git on the path — a released archive built
  by `release.yml` still has one, since that workflow checks out the tag before building), came from
  a dirty tree, the directory has no head, or `git merge-base --is-ancestor` itself could not answer
  (exit code neither `0` nor `1` — most often the binary's commit is not an object this checkout's
  repository holds at all, e.g. two unrelated histories or a shallow clone) — and in every one of
  those four cases it says *skipped* rather than passing, because an unknown answer is not
  clearance.
  `doctor` keeps every one of those rows. The session-start note (`build_note` in the hook) is
  silent in exactly one of them: a clean build with a commit, a readable head, and a repository
  that does not hold the commit (`git cat-file -e` fails), which is every project that is not
  Estigia's own and where nothing can be done about it. A dirty build, a build with no commit and a
  directory with no head are still named at session start, and so is a stale build in a checkout
  that holds its commit. A shallow clone of Estigia that lacks the commit is silent too, since it
  looks the same as a foreign repository; `doctor` still says *skipped* there.
- **`status --json`'s copy of the identity is asserted nowhere.** `estigia --version` has
  `version_reports_the_build_identity_not_only_the_crate_version` in `tests/pipe/cli_process.rs` comparing the
  printed line to `estigia::build_info::VERSION` directly, and the same assertion was added
  for the MCP handshake's `serverInfo.version` (`src/harness/mcp/mod.rs`) and the TUI banner
  (`src/tui.rs`). `status --json`'s `"estigia"` field (`src/cli/mod.rs`) has no equivalent:
  reverting it to bare `env!("CARGO_PKG_VERSION")` leaves the whole suite green.
- **The embedded identity's `dirty` flag can go stale between builds, and only that half.** The
  first cut of `build.rs` watched `build.rs` itself and every `.rs` file under `src/transport`
  except any named `tests.rs` — `rust_sources` skips those, so `src/transport/claim/tests.rs` was
  never among them — and emitting any `rerun-if-changed` at all disables Cargo's default "rerun
  the build script if anything in the package changed" — so an edit anywhere else recompiled the
  crate while Cargo replayed the previous run's cached `ESTIGIA_COMMIT` and `ESTIGIA_DIRTY`.
  Measured: editing `src/harness/doctor.rs` (outside `src/transport`) and rebuilding left
  `--version` reporting the commit and dirtiness of whatever tree the build script had last
  actually run under, not the one on disk. It now additionally watches the git files whose change
  means the *commit* half is stale — the per-worktree `HEAD`, resolved through `git rev-parse
  --git-dir`/`--git-common-dir` so a worktree's indirection and a branch's ref both work, plus
  `packed-refs` for a ref a `git gc` has moved. That closes the commit half: committing, or
  switching branches, and rebuilding now reports the new commit. Nothing but that hand measurement
  holds the claim, and it is the only thing that does: deleting the `watch_git_identity(&root);`
  call at `build.rs` leaves the suite green, and the identity would simply go stale on the next
  unrelated rebuild — silently, in the direction that reads as working. It does not close the
  `dirty` half, and cannot without giving up the point of not emitting a `rerun-if-changed` for
  every file — editing an already-tracked file's *content*, without
  committing or touching any ref, changes `git status --porcelain`'s answer without touching anything
  this build script watches. Measured the same way: after the fix, editing `doctor.rs` again and
  rebuilding still reports the tree clean, because nothing told Cargo to rerun the script. A wrong
  *clean* is the failure mode this row was filed to prevent for *older*, and it is still reachable
  here for *dirty* — an operator who committed cleanly, then edited without rebuilding through a
  fresh `cargo build` invocation that itself changes a watched file, sees a stale `clean` on the next
  unrelated rebuild. There is no cheap fix: watching every tracked file for content changes is
  exactly the default Cargo behaviour this build script turned off for the transport-vocabulary
  extraction's sake, and restoring it here would restore the unrelated-edit rebuild this same change
  was written to close.

  The dirty read itself narrowed too, on purpose: `git status --porcelain` counts an untracked file
  the same as an edit to a tracked one, so a machine that always has one lying around — a stray
  build artifact outside `target/`, an editor swap file — reported `dirty` on every build, and the
  row this feeds was permanently `skipped` there, which is the machine the issue was filed about.
  `capture_identity` now reads `--untracked-files=no`, because the question is whether the
  *committed* tree still matches the commit it names, and an untracked file does not change that —
  a narrower claim than "compiled from": `build.rs`'s own `rust_sources` walks the filesystem under
  `src/transport` rather than git, so an untracked `.rs` file placed there *is* folded into the
  generated `TRANSPORT_VOCABULARY` while this flag still reports the tree clean. `--untracked-
  files=no` is held the same way as the commit-half closure above: by the hand measurement in this
  entry, and nothing else — no test in this repository builds an untracked file and asserts that it
  does not flip `dirty`.

  A fourth route to reporting an unread state as clean, not disclosed until now:
  `capture_identity`'s own `dirty` (`build.rs`) is
  `git_match(&["status", "--porcelain", "--untracked-files=no"]).is_some()`, and `git_query`
  (`build.rs`) returns `None` for both "the command failed" and "the reply was empty" — so a
  build where `git rev-parse` succeeds and `git status` fails embeds `ESTIGIA_DIRTY=0`, and the row
  reports `ok … current` for a tree whose status was never actually read. Left as found rather than
  fixed this round: distinguishing a failed `status` from a clean one needs `dirty` to carry three
  states, not two, which is a change to what `build_info::dirty()` returns, not a change local to
  `build.rs`.

  A fifth reading of this row, filed and measured apart, is **not** the same fact: it was
  two artifacts of one `cargo test` reading *different* values, not one value going stale. The
  `gate-version` fixture in `tests/pipe.rs` chose its assertion branch from
  `estigia::build_info::dirty()` — a constant compiled into the test target's copy of the library —
  while asserting on what the *binary* printed; a run once saw the test target read `dirty` while the
  binary's row did not, which is the failure message naming the dirty branch. Re-measured, the
  disagreement did **not** reproduce: in one `cargo test` the integration test and the binary link the
  same library rlib, so `dirty()` is a single compiled value and the two targets cannot read it apart.
  What was true, and what the fix removes, is the shape that could produce the symptom at all — a
  branch chosen from a different artifact than the row is about. The fixture now reads the binary's own
  identity from `estigia --version`, whose `VERSION` always carries a parenthetical marker (`, dirty`,
  or `unknown build`), so whichever shape the binary is in, its own report picks the branch, and all
  three *cannot tell* branches are still asserted on their own words. This does not prove the original
  observation wrong, only that it is not reproducible on this toolchain and that the test no longer
  rests on the two artifacts agreeing.

  What the row answers during that stale-`dirty` window is not silent either way: it still prints
  `ok       gate-version this build (C) is current against the checkout`, the same line it prints
  when the tracked tree genuinely matches, for a binary whose source is in fact an edited tree
  neither commit `C` nor the checkout's `HEAD` names. The issue this row was filed against is
  "never a silent pass" — this is a pass, spoken plainly, for a state the row cannot see.

- **Codex's trust for Estigia's gate is written from a re-implementation of its hash, and `doctor`
  cannot ask Codex whether it would run the hook.** codex-cli 0.159 skipped Estigia's
  hook without a message because no `[hooks.state."<key>"] trusted_hash` named it, while `doctor`
  said `1 live`: the row read the registration and never whether the agent would honour it. Setup now
  writes the entry, and `doctor` reads it back. What that leaves unproven:

  - **The hash is Estigia's port of `hook_hash` and `version_for_toml` in `codex-rs`
    (`hooks/src/engine/discovery.rs`, `config/src/fingerprint.rs`), read at `rust-v0.159.2`.** It is
    held by six vectors taken from what `codex app-server`'s `hooks/list` reported for the same
    `hooks.json` — different events, a matcher that Codex ignores for `Stop`, a timeout of `0`,
    an absent timeout, `async`, `statusMessage`, and a command with a non-ASCII character, quotes and a
    backslash — and by a measurement with the real binary: trusted by the table `setup` wrote, and the
    gate invoked. It is **not** held against any other Codex version. A release that adds a field to
    the hashed identity, changes the canonical form or the key, makes Estigia's hash drift silently:
    Codex then reports the hook as modified and skips it, and `doctor` — which compares against
    Estigia's own hash, not Codex's — still says `live`. A drift like that fails no test here.
    `estigia setup codex` does not detect it either, and the vectors would have to be re-taken.
  - **`doctor` assumes a Codex that enforces trust.** On an older one the hook runs whether or not the
    table is there, so a hand-edited `config.toml` without it is reported broken for a gate that runs.
    After `setup` the table is there either way.
  - **The version boundary was read from the source, not measured.** `trusted_hash` first appears in
    Codex's `config.schema.json` at `rust-v0.129.0` and is absent at `rust-v0.128.0`, `0.127.0`,
    `0.125.0` and `0.123.0`. The schema of `0.105.0`, `0.113.0`, `0.115.0`, `0.118.0`, `0.121.0` and
    `0.123.0` has no `hooks` key at all, and `0.125.0` to `0.127.0` have `hooks` as the events table
    with no `state`. `ConfigToml` at `0.105.0`, `0.113.0`, `0.125.0`, `0.127.0`, `0.128.0`, `0.129.0` and
    `0.159.2` carries `#[schemars(deny_unknown_fields)]` and no `#[serde(deny_unknown_fields)]`; the
    generated schema says `additionalProperties: false`, which is the editor's schema and not the
    loader, so the extra table is ignored there. No pre-0.159 binary was run. A user who starts Codex
    with `--strict-config` is not covered by that reading.
  - **The key is built from the `hooks.json` path Estigia resolves** (`$HOME/.codex/hooks.json`), and
    `setup` does not read `CODEX_HOME`. If Codex resolves its home to another path, or to a symlinked
    one that it normalises, the key differs from Codex's, the hook is untrusted there, and `doctor`
    still says `live`. Measured only with `CODEX_HOME` set to the same directory setup wrote.
  - **Enterprise-managed, project-level and plugin hooks are not looked at**; only the user layer
    `setup` writes is.
  - **Nothing here asks Codex to run a write it should refuse.** The invocation measurement used a
    stand-in gate that always denies, to see whether Codex calls the hook and honours a refusal. The
    decisions Estigia's own gate takes for a Codex write are covered by the tests on the gate, not by
    this measurement, and the gate stands aside outside a repository with a claim.
  - **`status` still says `gate on` from the registration alone.** Only `doctor` reads trust.

- **`config.toml` is read and edited through a TOML parser, and setup refuses a file it cannot read
  back.** The line reader that wrote `[hooks.state."<key>"]` misread valid inputs two
  blind panels found: a `[hooks]` header carrying a URL with `=` in its comment, a `]` inside a
  header comment, a single-quoted `'state'` key, and a line starting with `[` inside a `"""…"""`
  value. It missed the spellings `[hooks."state"."<key>"]` and `[ hooks.state."<key>" ]`, so a table
  already written that way was duplicated; it read a foreign header carrying `=` as part of Estigia's
  own section, taking that table's trust with it; and it refused `["hooks.state"]`, which is a
  different key. `hooks.state` is now edited with `toml_edit`, so every spelling names the same entry
  and comments, key order, spacing, CRLF and a leading BOM survive. Before writing, `setup` re-parses
  the whole file it is about to write and reads the trust back, refusing a file that does not parse
  or does not carry the trust the entry earned. What that leaves:

  - **The `[mcp_servers.estigia]` table is still edited by the line.** The parse-before-write guard
    covers that edit — a line-based removal that leaves the file unparseable is refused before the
    write — but the edit itself is not done through the parser. A comment inside the table is dropped with it when a key follows; a comment after the last key is kept.
  - **The re-read is Estigia's own.** Setup proves the file parses and that Estigia reads the trust
    it meant to write; it still does not ask Codex whether Codex would honour it, which is the
    gap above.

- **A stalled `gh` is a broken row within ten seconds, and the screen that waits on it is not
  measured.** `doctor`'s `gh` row ran `gh auth status` with no time limit, and the
  `tui` Doctor page ran the checks on the thread that reads keys, so in raw mode (where Ctrl-C is a
  key and not a signal) a stalled `gh` froze the screen with no way out, and the keys pressed during
  the wait were handed to the page afterwards. `src/bounded.rs` now runs the program (the runner the
  advisory model catalog already used, moved rather than copied), and a program that does not answer
  within `PROGRAM_TIMEOUT` is `Broken` with the row's own resolution.
  `a_program_that_stalls_is_broken_within_its_limit` measures that through `program_within` with a
  stand-in that sleeps past a short limit. It does not go through `examine`: the check starts `gh`
  from `PATH`, not through the transport's scripted `gh`, so no fixture here stands in for it there.

  `App` carries the busy state (`Busy`): while the checks or a model catalog load run, `press` ignores
  every key but `q` and Ctrl-C, and `keys_pressed_while_the_checks_run_change_neither_the_page_nor_the_report`
  and its two siblings measure that. `a_checks_worker_that_panics_reaches_the_screen_as_a_report_not_a_stuck_wait`
  drives `spawn_worker` and `deliver` with a closure that panics, so a worker that dies without
  answering is measured clearing the busy state; a worker's normal answer is not. What stays
  unmeasured is `run` itself, as before: that the channel is read by the loop, that `poll` ticks while it waits, and that
  quitting during the wait restores the terminal are read from the code, not driven. Quitting
  abandons the worker rather than cancelling it, so a stalled `gh` or catalog probe is left to its
  own bound, and unsaved answers still make `q` or Ctrl-C ask once first.

## Identity and the tracker

- **The identity and the verdict reach different surfaces, and a refused call carries neither.**
  `answer_text` (`src/harness/mcp/mod.rs`) inserts a `build` object — `commit()` and `dirty()` from
  `src/build_info.rs`, read directly and not through `gate_age_verdict` — into a successful tool
  answer whose body is a JSON object, unless the call passed `compact: true`; a body of any other shape is left as it was. `gate_version`'s
  `Health`, which is what `gate_age_verdict` decides, has two callers: `doctor`'s `gate-version`
  row, as it always has, and now `hook::session_start_response`, which renders it into
  a sentence at `SessionStart` when it is not `Fine`. It reaches no tool answer, so no tool answer
  says whether the build that produced it is current. None of this
  touches `refusal_from_answer`'s
  `Refusal` path nor `crate::transport::Failure::envelope`: a call that is refused reports no commit
  and no dirty flag, the same as before this round. A caller who only ever sees refusals for a given
  tool still cannot tell which build answered.

  The `SessionStart` note is computed once, from one `gate_version(&context.repo_dir)` call at the
  moment the hook fires. Nothing re-reads it for the rest of the session: a checkout that commits,
  pulls, or checks out a different branch after that message was printed leaves the agent holding a
  verdict that described an earlier state of the repository, until the next `SessionStart`.

  The note also speaks on every session it cannot compare, and carries nothing to act on when it
  does. `build_info::dirty()` is true for any build from an uncommitted tree, and a crate installed
  from a released tarball carries no embedded commit at all; both reach a `Skipped` arm, so both see
  the note at every `SessionStart`, and nothing a reader can do removes it. `Health::Skipped` holds
  no `Resolution`, and `render_build_note` discards the one `Health::Broken` holds, so the reinstall
  command `doctor`'s own row offers reaches neither arm. Both seats of the panel on this change
  reached the first of those independently.

  **The `Broken` arm is also not rare.** `render_build_note` is silent only for `Fine`, and `Fine`
  needs the checkout's head to be reachable from the build's commit. A clean build, installed from
  `main`, on any branch that carries a commit the installed binary lacks therefore takes `Broken`,
  not `Skipped`: every feature-branch session after an install from `main` sees the note. The
  `Skipped` populations above are not the only ones that hear it every session.

  **The note names no command, by decision.** `CLAUDE.md` says a message names no command unless
  running it clears the block, and a reinstall does not clear this one in-session: the `Broken` detail
  says the tools pick up a reinstalled binary only once the session restarts. That rule wins over the
  ratchet in `src/harness/doctor.rs` that wants a `Resolution` on every failing check: the `Broken`
  note says plainly that the block clears only after reinstalling and restarting the session, the
  `Skipped` note names nothing because nothing a reader does clears it, and no `Resolution` field is
  added to either.

  **Three mutations were run against the tests of this note, and each reddened a test.** Deleting
  `{distance}; ` from the `Health::broken` format string reddened three in `harness::doctor`
  (`a_broken_verdict_names_the_distance_when_it_is_known`,
  `a_broken_verdict_says_the_distance_could_not_be_read_rather_than_current` and
  `a_broken_verdict_over_a_real_git_commits_ahead_names_the_distance`). Making `render_build_note`'s
  `Skipped` arm return an empty string reddened three in `harness::hook`
  (`a_nameless_session_start_carries_the_build_note_too`,
  `a_session_start_carries_the_build_note_when_the_checkout_cannot_be_compared` and
  `a_skipped_build_health_says_cannot_tell_never_current`). Replacing `if let Value::Object(fields)`
  in `answer_text` with an unconditional wrap reddened exactly one,
  `a_non_object_body_is_left_alone_rather_than_corrupted`. Each count is of the library tests under
  `harness::` only; none was run against the integration targets.

  The comparison itself — which only the verdict makes, since the tool answer reads two constants and
  compares nothing — is against the **local checkout's `HEAD`**, the directory `GateContext::repo_dir`
  names, and never against a remote's default branch. Production code does read
  remote and upstream refs: `@{upstream}` in `src/harness/mod.rs`, `symbolic-ref --quiet HEAD` there
  and in `src/transport/branch.rs`, and `ls-remote --heads origin` for a caller-supplied refname. None
  of those resolves a repository default branch — an upstream is the current branch's own tracking
  ref, and `ls-remote` is told which name to look up — so there is no such notion here for either
  surface to compare against, and what a fork or a stale local clone's `HEAD` last pointed at is the
  only fact either one reads.

- **A phantom holder is reconciled against the tracker only where the ambiguity is real.**
  `guard::adjudicate_action` used to refuse `several-runs-hold-this-checkout` for two or more run
  pointers naming one checkout without ever asking whether either was still true, naming *runs that
  no longer exist* and a resolution its reader could not perform. It now asks the tracker about each
  named holder before building that refusal, and drops one only on an explicit `issue-not-open` —
  every other answer, including a failed read, keeps it counted, because an unknown result is not
  clearance in this direction either.

  **Reconciliation decides each holder on its own answer, and a mixed pair is what reached CI.** The
  rule above is per-holder, not a pair of whole-run cases: one holder answering `issue-not-open` and
  another's read failing are decided separately. The first drops and the second is kept,
  `holders.len()` is exactly **1**, and the `1 =>` arm calls `gate`. Whatever `gate` refuses first is
  then what the operator is told, and it is not the read that failed. Measured twice on this
  repository's own lane, both times on `ubuntu-latest` alone and both times the only failure in a run
  that otherwise passed: run
  32738461410 on 2026-08-24 and run 33482324896 on 2026-09-01, each reporting
  `control-surface-not-installed` against a `SKILL.md` under a temporary directory, with a resolution
  of `estigia setup --all`. The guard tests' own fixture had never installed a contract, so the first
  thing `gate` reached was its absence. The refusal named a file the failure had nothing to do with,
  and sent its reader to install something that was not missing anywhere but in the fixture.

  `guard/tests.rs`'s `context` now writes the contract itself rather than leaving the tests that
  reach the tracker to install it themselves, so the same asymmetry answers `read-failed`,
  `gh issue view 12 returned nothing`, with the resolution the transport already carried: *the
  tracker could not be read; write nothing and retry the read*. Pinned by
  `an_unanswered_tracker_read_names_the_read_and_not_the_contract`, which reports the
  `control-surface-not-installed` refusal verbatim when the fixture's write is removed.

  **What the fixture change does not close, and cannot be closed from here.** Why one of two
  identical reads fails on Linux and on neither of the other two platforms is unidentified. Three
  readings survive the code
  and are indistinguishable from the failure output — a transient spawn failure against the freshly
  copied `fake_process` binary that `scripted_gh` writes per call at 74 call sites, a `canonicalize`
  that succeeds for one holder's coverage depth and falls back for the other, and a pointer file
  transiently unreadable, which the `1 =>` arm never consults. Separating them needs instrumentation
  recording how many `gh issue view` calls the failing run made, and that instrumentation would have
  to run on a platform this repository's maintainers do not reproduce on. No further sighting is
  possible for now: this lane has been disabled since 2026-09-01. The legibility is
  fixed and measured; the flake itself is not, and the entry says so rather than implying the
  fixture change ended it.

  What reconciliation deliberately does not close: a **single** holder whose issue is closed by a
  close this harness cannot recognise as the review's registered delivery still refuses every write in the
  checkout it
  names, with the same `issue-not-open` reason it always has. Reconciliation is guarded to two or
  more holders on purpose — not because `GateContext` cannot tell an own-delivery close from a
  stranger's, but because folding a single holder into this loop would not repeat that refusal, it
  would **erase** it. A close this harness *does* recognise as the review's registered delivery — the closing
  PR the newest confirmed publication names, and a merge whose second parent is the reviewed head and whose first parent
  descends from the reviewed base
  (`src/transport/claim/close.rs`) — makes `verify_claim` answer `closed-by-own-delivery`
  rather than `issue-not-open`, so `holder_standing` counts that pointer `Live` and there is nothing
  to drop. A close it does not recognise this way already refuses through the single-holder arm's
  own call into `gate`. Reconciling that same holder anyway would drop it, bringing
  `holders.len()` to zero, which falls to the `0 =>` arm below — and that arm answers
  `Outside(NothingSworn)` only when nothing else there denies first: an unfinished sibling
  review still denies `complete_review_receipt_not_selected`, and any unreadable pointer
  machine-wide still denies `run-pointers-unreadable` (`guard.rs`'s `0 =>` arm). Only landing on
  `Outside(NothingSworn)` replaces `gate`'s refusal rather than repeating it — the other two arms
  still deny — and that is the gate-widening hazard the guard exists to keep closed. Measured: changing
  `guard.rs`'s `holders.len() >= 2` to `>= 1` reddens
  `a_single_stale_holder_still_refuses_through_its_own_issue_not_open`, which then reports "a single
  stale holder no longer refuses on its own answer: Outside(NothingSworn)". `doctor`'s new
  `stale-run-pointer` row reports the stranger case: it names the pointer and the release command. A
  pointer this harness recognises as an own-delivery close is not that residue, and it has
  a bucket of its own — `stale_run_pointers` (`src/harness/doctor.rs`) now reads `Answer::standing()`
  (`src/harness/tracker.rs`), which tells a code-0 pass apart from a code that never landed a read
  where `stopped_reason` alone could not, and tests a passing answer's own `checked[0]` for the
  token `OWN_DELIVERY_CLOSE_PREFIX` (`src/transport/claim.rs`) to route it to a `stranded` population,
  reported by name with the
  same `estigia release --run-id` command the stale population gets. Before that change this
  population fell into the row's unread arm instead, whose message asserts the pointer "could not be
  checked against the tracker" and sends the operator to find out why `gh issue view` failed — both
  wrong for a call that succeeded. That was a defect in the row's own message, and it is fixed for
  this population now. That token has **one spelling**: `verify_claim` builds
  `checked[0]` from `OWN_DELIVERY_CLOSE_PREFIX` and `Answer::is_own_delivery_close` reads it back from the
  same const, where before the transport wrote the literal and the harness matched a second copy of
  it with nothing crossing them. `OWN_DELIVERY_CLOSE_SPELLING_POPULATION` binds to the const, so
  editing the token reopens `tests/guards.rs`; a second spelling introduced elsewhere is the escape
  that population names rather than hides. **The predicate that routes the population is exercised
  by a test; the end-to-end arm is not.** The test `own_delivery_close_reads_the_transports_own_prefix_and_nothing_else` calls the predicate
  with a fabricated `Answer` — including bodies whose `checked[0]` is absent, non-array or
  non-string — so the reader itself is measured. Driving the whole `stranded` arm still needs a
  fixture that actually produces `delivery_close` — a closing PR the receipt names and a merge
  whose second parent is the reviewed head and whose first parent descends from the reviewed base —
  and, as the next entry's own disclosure says, no such fixture exists
  anywhere in this crate's suite, so the `stranded` arm is classified code with no test behind it,
  same as the population it was carved out of.

  **Two more findings landed on the same row.** The `Fine` sentence used to say no
  pointer "names one closed behind this run's own delivery", which is false of an **aged** stranded
  pointer: `verify_claim` orders its checks open-or-own-delivery, then label state, then live
  holdership, and `reduce_ownership` (`src/transport/ownership.rs`) moves an acquisition
  whose deadline has passed into `stale` with `holder: None`, so such a pointer answers
  `not-current-live-holder` — a code-1 stop the row treats as neither stale nor stranded nor unread
  and so reports `Fine` over. The sentence was narrowed and the row says how many pointers it
  actually read as still open and held (`N of them read as still open and held by this run`). That
  count is what makes `a_genuinely_passing_pointer_is_reported_fine` fail when the fixture's label
  state is changed to `"review"`: the pointer then answers `unexpected-state` and the count is zero,
  where asserting the `Health` variant alone passed either way.

  **The narrowed sentence still denied two states.** Its first clause, "names an
  issue the tracker reports closed", is false of an aged stranded pointer, which reaches `Fine` with
  its issue closed. Its second, "still holds one closed behind the registered delivery", is false of
  a label-mismatched own-delivery close: that one stops at `unexpected-state` before holdership is
  examined, so it reaches `Fine` with a live holder while `checked[0]` still carries the prefix. The
  sentence now names exactly the row's three buckets — answered with `issue-not-open`, answered with
  `Passed` and an own-delivery close, or carried an answer the row could not read — which is true of
  every state that reaches `Fine`, because `Fine` is returned only when all three are empty.
  `the_fine_sentence_claims_only_the_buckets_the_row_empties` pins the bucket names and rejects both
  over-claims, since neither state is drivable through the scripted-`gh` fixtures.
  `stale_run_pointers`' own doc now names both arms that feed `unread` — `Standing::Unread`, and the
  outer `None` from `tracker_answer_for_pointer` that is no answer at all.

  The row has a blind spot of its own, and it is plausibly the commonest dead-pointer shape:
  `same_git_repository` fails closed to `false`, so a pointer whose recorded checkout no longer
  resolves as a git repository at all — deleted, moved, the removed-worktree phantom this row
  exists for — compares `false` against every repository, this one included, and is silently
  excluded from the row entirely: not stale, not unread, not named. `run_pointers` still reports it
  if the pointer itself will not parse; a pointer that parses fine and names a checkout `git
  rev-parse` can no longer find is invisible to both rows.

  Also deliberate, and the sharper edge: **two dead pointers with nothing live covering the same
  checkout reconcile all the way to zero, and the gate stands aside, while one dead pointer still
  refuses.** A live holder covering that checkout survives reconciliation regardless of how many
  dead pointers sit alongside it, so this is the all-dead case specifically, not "two or more stale
  pointers" on its own. `holders.len() == 0` after reconciliation answers `Outside(NothingSworn)` —
  the ordinary shape of a checkout nobody claims — when `session::unreadable_holdings` is also empty;
  an unreadable pointer anywhere on the machine denies `run-pointers-unreadable` instead
  (`guard.rs`'s `0 =>` arm), and an unfinished sibling review denies through its own arm ahead of
  either. So a machine carrying two phantom pointers for one checkout, and nothing else unreadable,
  is *more* permissive than one carrying a single live-looking one. That is a consequence of the
  guard stated above, not a second gap: a lone
  `issue-not-open` pointer refuses through `gate`'s own single-holder path rather than through this
  reconciliation, so `N == 1` was never touched, and nothing here decided that two phantoms are safer
  than one legitimate refusal — it decided that a checkout two closed claims both left behind is a
  checkout nobody currently holds, which is true.

  Two or more holders that reconciliation cannot separate — because the tracker genuinely says both
  are still live — are the common case rather than the rare one: `start_branch` gives each run its
  own isolated worktree without narrowing what the pointer's claim covers, so two healthy runs both
  still cover the shared base checkout. What tells them apart is the tracker's own answer for each
  survivor, not the pointer: `not-current-live-holder` is the only standing `estigia release
  --run-id <run-id>` is offered for, and it is **three** different timelines, not one — once a
  state label already matches (a disagreement answers `unexpected-state` first, and classifies
  live), `verify_claim` answers it whenever `holding().holder` is not this run, which covers no
  acquisition for this run at all; an acquisition of this run's own that has gone stale, past its
  horizon and still on the timeline (`ownership::holding` moves a lapsed one into `stale` rather
  than dropping it); and a live acquisition of this run's own that lost a claim race to a different,
  earlier holder. A pointer's `worktree` field was tried first and was wrong in both directions,
  measured against this crate's own `mcp::run_tool`: `PointerEffect::Swear`, what `claim` runs,
  writes `issue`, `state` and `repo_dir`, and writes `worktree` only when an adopted checkout is
  already there to discover — a fresh `claim` ahead of its own `start_branch` still writes none.
  `PointerEffect::Isolated`, what `start_branch` runs *after* the claim, is what fills it for that
  ordinary case — so a live run between the two still names no worktree and was offered for release,
  and a session that died right after `start_branch` names one nobody works in any more and was read
  as live. When nothing among the survivors is
  `not-current-live-holder` — two or more claims the tracker itself calls live — the resolution
  names no command: there is no CLI verb for isolating a checkout the way `start_branch` does (that
  tool is MCP-only), so nothing exists that would actually discharge the block for a reader holding
  one of the live claims, and naming one anyway would be exactly the dead end this repository's own
  rule forbids.

  `estigia release` had to be made to actually clear a phantom pointer's refusal, and what it does
  differs by which of the three timelines this is: `plan_release` searches `ownership.live` chained
  with `ownership.stale` for this run's own acquisition. Not found — no acquisition ever existed —
  answers `nothing-to-unassign`, and only the local pointer goes. Found — a stale acquisition, or a
  live one that lost the race — the acquisition is really there on the timeline, only lapsed or
  losing, and `release` ends it: a genuine write to the tracker under the operator's own identity,
  not the no-op "clears it harmlessly" first read as. Both shapes are recognised, and only once that
  pointer's own recorded repository is the one `release` was run against, so a successful read of an
  unrelated project's timeline forgets nothing; every other refusal, including a failed read, leaves
  the pointer exactly where it was.

  **The cost this buys**: once two or more pointers cover one checkout, every gated action pays one
  tracker round trip per named holder before the filter runs, plus the survivor's own `gate` call once
  it has — two holders reduced to one costs at most three tracker reads, not one. The survivor's own
  `gate` call is the one that can skip its read: `Sensitivity::Routine` inside the renewal window
  (`harness::gate`, `how == Sensitivity::Routine && run.within_window(context.window)`) answers
  `Allow` from the cached verification without dispatching at all, so the true floor is the two reads
  reconciliation itself spent and the ceiling is three. The spawn happens above
  `standdown::over`, not below it, so a `gh` that hangs is not a door `estigia stand-down` can open
  past — the reconciliation blocks before the stand-down is even consulted. Unmeasured beyond that: no
  fixture drives this cost.

  One action pays less than this whenever it can. `git push` narrows on the pushed refs when
  `holders.len() >= 2` (`src/harness/guard.rs`), which is *before* the reconciliation loop that spends
  the tracker round trips above (see "Nothing adjudicates what a push is aimed at" above), so a push
  whose local ref names exactly one holder's own branch reduces to one holder for free — no tracker
  round trip is spent reconciling the holder list at all, only the survivor's own boundary `gate`
  call. The two-or-three-read cost above is what a push that does *not* narrow — no refs, an
  unresolved branch, or any of the other fail-closed roads — still pays, the same as every other
  gated action with two or more pointers on one checkout. Narrowing before reconciliation also means
  the `ClosedIssue` filter that reconciliation applies never runs on the narrowed survivor — see
  `narrowing_before_the_tracker_can_deny_what_reconciliation_would_have_allowed`.

  The *other* holder list this same reconciliation reads over is driven, not left untouched:
  `holders_for_action` builds one for a `gh pr merge` with no ordinary holder in this checkout,
  filtered to siblings whose own complete review receipt names the pull request being merged — a
  `map`/`filter`/`collect` this reconciliation runs exactly as it does for an ordinary holder list —
  and `guard::tests::sibling_selection_binds_pr_before_head_and_attributes_no_ambiguous_holder` gives
  it two receipt-matched siblings naming the same pull request to reconcile between, scripted with
  `unreachable_tracker_answer()` — a `gh` that exits 1 — so both answer as unknown, not as live, and
  denial follows the same fail-closed rule an unauthenticated `gh` in this environment would actually
  reach: an unknown result keeps a holder counted rather than dropping it (`guard.rs`'s own comment on
  the `_` arm is explicit that this is not the same thing as "live"). This sibling path has never been
  crossed against a *successful* tracker read. Undriven beyond that two-holder floor: no fixture
  takes a sibling list past two, or reconciles a sibling list where one is dropped as `ClosedIssue`
  while another survives —
  every ordinary-holder test that shape has (`a_stale_holder_is_dropped_…`,
  `a_stale_holder_dropped_to_one_live_holder_lets_gate_allow`) has no sibling-list counterpart.
  `linked_siblings` is a different list entirely: it feeds `linked`, consulted only in the `0 =>` arm
  to tell "no ordinary holder and no sibling either" apart from "a sibling exists but has not finished
  reviewing," and reconciliation never touches it.

  **Reconciliation made `tests/pipe.rs` non-hermetic once, and that is now measured rather than
  believed fixed.** `run_with_path`, the fixture every `run`/`run_in` call in that file goes through,
  spawns the real binary and inherited the developer's whole environment beyond a handful of named
  variables — which cost nothing while a several-holder refusal never asked the tracker at all.
  Reconciliation is what first made it ask, and `a_per_call_working_directory_selects_the_holder_
  that_owns_it` is the one test in that file that reaches reconciliation through `run_in` rather than
  through a scripted `tracker_command`, so it is the one an inherited `GH_REPO` — the variable this
  crate's own transport uses to steer `gh`'s repository resolution — could point at a real repository
  instead of the fixture's, deciding the test's outcome by whatever issues happened to exist there.
  `run_with_path_locked` now clears `GH_REPO`, `GH_HOST` and `GH_TOKEN` before spawning.

  What this closes and what it does not: `an_operators_gh_repo_never_reaches_a_spawned_gh`
  (`tests/pipe/cli_process.rs`) puts `GH_REPO` on the test process, spawns `estigia doctor` through
  `run_with_path_locked` with a scripted `gh` on `PATH` — `doctor`'s own unconditional `gh auth status`
  call is the trigger, not reconciliation, so this needs no two-holder fixture — and reads back what
  that `gh` actually saw through a file `examples/fake_process.rs` writes on request. That is a live
  before/after on the mechanism itself: disabling `run_with_path_locked`'s `env_remove` loop with
  `if false { … }` turned this test red, reporting the operator's own `GH_REPO` back out of the
  child. What it is not is a reproduction of the *original* defect end to end — no test here points a
  real `GH_REPO` at a real repository and lets `a_per_call_working_directory_selects_the_holder_
  that_owns_it` make an actual network call to it, and none needs to now that the removal is
  mutation-tested directly against a scripted `gh` rather than inferred from the suite passing
  unchanged. Not audited for every other variable a real `gh` process might read to steer itself —
  `GH_REPO`, `GH_HOST` and `GH_TOKEN` are the three this crate's own transport and `gh`'s own manual
  name as authority- and repository-steering; others may exist and are unmeasured, the same way the
  round-trip cost above is unmeasured rather than assumed zero.

- **`GH_REPO` on the harness process was set and never cleared.** Two production
  sites — `gate` and `tracker_answer_for_pointer` — wrote `GH_REPO` only when `named_repo` was
  `Some`, so a later call that named none inherited the previous pin. MCP `run_tool` already set or
  cleared it after the split; it now goes through the same `apply_gh_repo` helper. Under `cargo test` every
  path shares one process; the SAFETY notes that said "one invocation per process" are true in
  production and false in the suite. `apply_gh_repo` always sets or removes, and
  `a_named_gh_repo_pin_does_not_survive_a_call_that_names_none` holds the clear. What still costs:
  the pin is still a process global rather than a per-spawn `Command::env`, so concurrent tests that
  race on `set_var` remain under the same Rust-2024 precondition the `test_env` entry already names;
  and any other process-global a test fixture mutates without a restore is a separate leak this
  change did not inventory.
- **The delivery sequence now asks for its own release, and nothing adjudicates that it happened.**
  `skill/SKILL.md`'s Developer step 6 names merge, tag, renew, transition to `done`, record evidence,
  close *and release* — but that sentence is prose an agent reads, not a gate. No code observes that a
  run which reached `done` still holds a pointer. `transition`'s `PointerEffect::Moved` sets
  `run.state` and calls `run.mark_verified()` — only when the call names the issue the pointer holds,
  so a `transition` of any other issue leaves the held issue's expected state and renewal window
  alone; `release_ci`'s `PointerEffect::Renew` fills `run.issue`
  and `run.repo_dir` when they are absent, fills `run.worktree` from a discovered checkout when the
  pointer does not already name one, sets `review_receipt` and clears `reviewed_head` when the call
  carries a complete receipt, and calls `run.mark_verified()` too — it fills `run.state` only for
  a call that names one, which `release_ci` is not, since neither `state` nor `expect_state` is among
  its arguments (`run_tool` in `src/harness/mcp/mod.rs`, the `release_ci` tool definition in
  `src/harness/mcp/tools.rs`). Neither
  effect forgets the run, and because `Moved` calls `mark_verified()`, a `transition --to done` that
  is not followed by release restarts the renewal window: the stranded pointer looks *more* live
  afterwards, not less.
  **It also reads as live to everything that would otherwise clear it, and that is the half worth
  knowing.** A delivery that followed the rest of step 6 leaves behind a close this harness recognises
  as the registered delivery — `delivery_close` matches the newest confirmed publication's pull
  request against the closing PR, and a merge whose second parent is exactly the receipt's head and
  whose first parent descends from the receipt's base
  (`src/transport/claim/close.rs`); the run that published the receipt is not part of the match, so a
  successor holding the live claim renews against a predecessor's delivery, and `verify_claim` answers
  `closed-by-own-delivery` rather than `issue-not-open` (`src/transport/claim.rs`). `Answer::stopped_reason()` is
  `None` for anything but a code-1 answer (`src/harness/tracker.rs`), so `holder_standing`
  classifies that pointer `Live` (`src/harness/mod.rs`). That read was rewritten to go
  through `Answer::standing()`, and the classification it produces is unchanged: a pass reaches the
  same `Live` arm it always did. The reconciliation cannot drop it at any holder count, and the
  single-holder `gate` call does not refuse it, so nothing in the gate
  itself will call that pointer stale — `doctor`'s `stale-run-pointer` row is the one place that now
  does: it reports this population in its own `stranded` bucket, named separately from `stale` and
  from `unread`, rather than folding it into the unknown arm the way it used to.
  **The close proof itself has tests from 2026-09-27; the population it names does not.** `release`
  now removes the delivering run's checkout on exactly `delivery_close`'s proof, and
  `tests/pipe/part_09.rs` drives both directions end to end:
  `releasing_a_delivered_issue_puts_its_checkout_and_branch_down` asserts a recognised close removes
  the worktree and the local branch, and
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down` asserts an unrecognised one keeps
  the checkout and names the reason, reading `checkout.detail` back rather than only the kept flag.
  That removal is best-effort and self-reported on the shipped path: a checkout `git worktree remove`
  refuses is kept, and nothing outside the answer crosses what it says against the filesystem. The
  two `tests/pipe/part_09.rs` fixtures are the exception — each reads the directory, the worktree registry
  and the local branch back after the call — so the sentence is about production, not the suite.
  What remains unexercised is the guard's own population: `src/harness/guard/tests.rs`'s
  `a_single_stale_holder_still_refuses_through_its_own_issue_not_open` covers only the stranger
  population, so a single holder whose issue closed behind a delivery this harness recognises as the
  review's registered delivery — the population `verify_claim` answers `closed-by-own-delivery` for
  instead of `issue-not-open` — is not
  exercised by any guard test. That guard module discloses the gap; this entry, whose job that is,
  did not until now.
  What it does instead is hold the checkout. The next run to claim there makes two holders, neither of
  them droppable, and `several-runs-hold-this-checkout` refuses that run's first write — which is what
  was measured. The `issue-not-open` path belongs to a different population: a pointer whose
  close this harness cannot recognise as a delivery of its own. So a delivery that follows every other
  step and skips release still reports success, and what it leaves is not a stale record somebody
  sweeps up later but a live-looking holder that blocks the next run. `tests/pipe/part_09.rs`'s
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down` drives `claim`, `transition --to
  done`, and `release` end to end against an issue the fixture closes in between, and proves the step
  the prose now names actually succeeds — that `release` puts a closed issue's claim down cleanly, not
  that any delivery calls it.

  **Naming the release in step 6 also costs the `dev:<runtime>` label on every delivery that follows
  it.** `release`'s `unassign` ends through `converge_ownership_projection`
  (`src/transport/claim.rs`): with no holder left, `wanted_runtimes` is empty and every
  `dev:<runtime>` label already on the issue is emitted as `--remove-label`, read from an inline
  `BTreeSet::difference` inside `converge_ownership_projection` at `src/transport/claim.rs`.
  This is not measured: `src/transport/claim/tests.rs` exercises `converge_projection`
  (`src/transport/claim.rs`), a helper with zero production callers anywhere in the crate —
  every call site is in that same test module — and disabling the production emission at
  `claim.rs` leaves the whole suite green. Before this
  step named release, delivery ended at close, so the label survived until a separate `release` ran,
  if one ever did; now an ordinary delivery removes it as its own last write. A query
  grouped by `dev:<runtime>` no longer shows which runtime delivered a closed issue — only the
  unbounded run-id in the close evidence survives that removal, and reading it back means opening
  the issue rather than filtering a label.
## The gate's boundary

- **A redirect to `/dev/null`, `/dev/stdout`, `/dev/stderr` or `/dev/fd/<n>` is not read as a write.**
  `shell::redirects_into_a_file` reads the target word after `>`, `>>` or `&>` and matches it
  exactly, quotes allowed, so `grep x src 2>/dev/null` stops asking. A word that only begins with one
  of them (`/dev/nullx`, `/dev/null/x`) or a target it cannot parse stays a write. What it does not
  see: a redirect through a variable or substitution (`> $sink`), or a `/dev/null` that is a regular
  file on a machine where somebody replaced it. Measured by
  `a_redirect_to_a_stream_is_not_a_write_and_any_other_target_is`, which goes red with the exemption
  removed.
- **The redirect reader models the quote rules that hide a `>`, and fails closed where it cannot.**
  `shell::redirects_into_a_file` ends a double-quoted region only at an unescaped `"`, reads `$'...'`
  with backslash escapes, treats an unquoted `#` at a word start as a comment to the end of the line
  (a `>` inside it is still read as a redirect, so `echo x # > y` stays a write), and skips a
  here-document body found by `heredoc_word` and `here_document` as data. A line that ends inside a
  quote after hiding a `>` is a write. What it does not model: a body `here_document` refuses (an
  unquoted delimiter with `$`, a backtick or a backslash in the body) is scanned as code, and
  `$"..."`, `${...}` and `$(...)` nesting are not tracked. Measured by
  `a_redirect_after_a_quote_the_scan_could_misread_is_still_a_write` and
  `a_quoted_arrow_stays_data_whichever_quote_holds_it`.
- **The redirect reader models the quote rules that hide a `>`, and fails closed where it cannot.**
  `shell::redirects_into_a_file` ends a double-quoted region only at an unescaped `"`, reads `$'...'`
  with backslash escapes, treats an unquoted `#` at a word start as a comment to the end of the line
  (a `>` inside it is still read as a redirect, so `echo x # > y` stays a write), and skips a
  here-document body found by `heredoc_word` and `here_document` as data. A line that ends inside a
  quote after hiding a `>` is a write. What it does not model: a body `here_document` refuses (an
  unquoted delimiter with `$`, a backtick or a backslash in the body) is scanned as code, and
  `$"..."`, `${...}` and `$(...)` nesting are not tracked. Measured by
  `a_redirect_after_a_quote_the_scan_could_misread_is_still_a_write` and
  `a_quoted_arrow_stays_data_whichever_quote_holds_it`.

- **An unclaimed run is refused a commit or a push only where the push guard is installed, and only
  for the spellings the command line names.** `unclaimed-commit` is raised when no claim holds the
  checkout and `guard::state` answers `Installed` or `Chained`: the operator running `estigia guard`
  is the opt-in, so a repository with no guard, or with no tracker, has no commit refusal from this
  check; whether an unclaimed run may write there at all is `Unclaimed write policy`, below, which asks by
  default and is `off` for the old behaviour. It is
  deliberately **not** scoped to the default branch: a branch needs a worktree to be worked on, and
  `start_branch` is what gives one, so any unclaimed commit or push skips the flow whichever branch
  it lands on. No default branch is resolved, which is also why no answer about one can be mistaken
  for clearance. What it does not hold:
  - The guard reading `Unreadable`, `Inert`, `Foreign` or `Absent` stands aside, not denies. An
    unknown is not clearance, but here denying would lock every commit in a repository because of a
    hook file; `doctor` reports `Unreadable` and `Inert` as broken, which is where that is said.
  - A repository whose tracker cannot adjudicate an unsworn write (no transport, and not a
    read-only tracker under `Workflow: assisted`), or whose installed skill has no contract file,
    stands aside too: a claim cannot be made there, so refusing would lock a door nobody can open.
    The SessionStart text and the refusal read one predicate, `unclaimed::applies`.
  - Recovering a half-finished operation is not refused: a commit-creating command followed by
    `--abort`, `--quit` or `--skip` (`git rebase --abort`, `git am --skip`) needs no claim.
  - `git pull` is not refused, though it can create a merge commit; it is the everyday way to bring a
    checkout up to date and is not told apart from a fast-forward without running it.
  - The commit spellings are matched on the command line as every other fragment is, so an alias, a
    script, an editor or `gh api` writing a commit passes, and `git push --no-verify` skips the
    hook half. The gate half still sees `git push` on the command line.
  - A **claimed** run is unchanged and is not held to its worktree: its claim covers the checkout it
    was made in as well as the isolated one, so a commit in the primary checkout is verified against
    the claim and allowed. The base checkout stays read-only by convention, not by this gate.
  - Measured by `a_commit_without_a_claim_is_refused_where_the_guard_is_installed`,
    `a_push_without_a_claim_is_refused_on_any_branch_where_the_guard_is_installed`,
    `the_pre_push_hook_refuses_an_unclaimed_push_where_the_guard_is_installed` and
    `a_session_with_no_issue_is_refused_a_commit_where_the_guard_is_installed`, each of which goes
    red with its half of the check disabled. Nothing drives the two shell entry points
    (`estigia gate` with no session, and the dialect hook with an empty session id) end to end; they
    call `adjudicate_command` with the line, and that call is covered only through the library.
- **`Unclaimed write policy` holds the writes the hook sees, and cannot hold the rest.** With the row
  `required` or `ask`, a run holding no claim is refused (`issue-required`) or asked before a routine
  write the gate can place inside a checkout that has a tracker and an installed contract — a file
  edit or write, or a shell line carrying one of the `REPOSITORY_SHELL` fragments such as `git commit`.
  Under `ask` a write the gate **cannot place** inside or outside the checkout is allowed
  rather than asked for, and the checkout it may have dirtied is named at the next session start.
  What it does not hold:
  - **`ask` is delivered as an ask on Claude Code and on Pi's terminal UI only, and its answer cannot
    be read back.** The hook answers `permissionDecision: ask` when the agent that registered it is
    `claude-code`. Pi is asked through `estigia gate --runtime pi --can-ask`, which exits `4` with the
    question; the extension passes the flag only when `ctx.mode === "tui"` and shows `ctx.ui.confirm`.
    That path is **unverified against a live Pi**: the exit-code and extension halves are
    driven under `node` against a stand-in binary, and the `confirm` call, its title and its
    resolution to `true` are read from Pi 1.1.0's `docs/extensions.md` and `dist/core/extensions/types.d.ts`,
    not observed on screen. `rpc`, `json` and print modes are never asked. Every
    other agent is refused with the same code instead: Codex and the other agents that share Claude's
    dialect are not known to act on the word, an `ask` a host ignores is an allow, and Cursor has
    no verified ask in the shape Estigia writes (nor did the retired Gemini CLI, Windsurf and Cline). The OpenCode plugin
    and `estigia gate` without `--can-ask` answer through an exit code that has no third value, so they refuse. The gate
    never learns whether the person approved, so it **asks on every such write** and caches nothing:
    approving one edit does not approve the next. Not measured against a live Claude Code host; the
    document the hook prints is, and the field name is taken from that host's published hook reference.
  - **A write the hook does not intercept is not held.** A tool no matcher wakes for, an editor or
    script that writes through a shell line naming none of the git fragments, a formatter run over
    the tree, or another clone, passes. The row is as wide as the classifier and no wider.
  - **Only a routine write and a `git push` are held.** A push is held as a commit is, so publishing
    is never less guarded than recording (`an_unclaimed_push_is_held_as_an_unclaimed_commit_is`).
    Every other boundary — a merge, a tag, a write to a control-surface
    file — is answered by the checks that already hold it, so an unclaimed run writing
    `.git/estigia/estigia.local.md` through a shell line is not stopped by this row, and can set it
    to `off`. `unclaimed-commit` stays what it was, and answers first where the push guard is installed.
  - **A write that lands outside every repository the gate can place is not a repository write.**
    A failed `git rev-parse` used to fall back to the launch directory itself, which made a
    directory that is not a repository read as one — a personal file write under `~/.config` was
    refused `issue-required` and told to claim an issue. The gate now asks git once per such write
    (`rev-parse --show-toplevel`, through the proof subprocess) and, where git names no working
    tree, walks the directory's ancestors for a `.git` entry. **No entry anywhere** is the
    directory established as outside every repository, and a write landing there is not reached by
    this row. **An entry git will not use, or an ancestor this process cannot read**, is not an
    answer: every such write is held, exactly as under `required`, and is never relabelled *no
    tracker* or *no repository*. The launch directory being outside git grants nothing on its own —
    a target is placed by its own ancestors' `.git` entry too, so a write from outside git into a
    repository, a claimed worktree or a sibling checkout stays refused, and only the message
    changes, no longer calling an unestablished directory "this repository". What is not closed:
    the walk is not git's answer, so a work tree named only through `GIT_DIR`, a ceiling directory,
    or a bare or mirror layout is read as a repository only where a `.git` entry sits in an
    ancestor; the reading is one `git` subprocess per unclaimed routine write this row reaches, and
    nothing caches across writes.
  - **A write tool whose paths live in a patch body is placed by the body's own paths.** For
    `patch` and `apply_patch` — the two write tools whose body *is* their target list, and only
    where no field named a path — `classify_observing` reads every string the payload carries, the
    reading the control-surface rule already makes, and takes `*** Add File:`, `*** Update File:`,
    `*** Delete File:` and `*** Move to:` as naming a target, one per line. `patch` with `patchText`
    is the payload the reproduction failed on. Each named path is placed, and a write to **any**
    repository among them is held, so a mixed-target patch is refused whole. A body shape this
    reader does not know names nothing, and a `target` that is not a path places against the launch
    directory instead — the fail-closed direction — so a patch into a repository is never exempted
    for being under-read. **No other write tool's body is read as paths**: one that named no path in
    a field is a write whose destination was not read, and a marker in a content string must not
    displace the tool's own name and exempt it. That shape — a `pre_write_code` call whose `file`
    field goes unread while a `*** Add File:` line in its `content` supplies the target — is what a
    draft that read every write tool's body exempted, and the regression it drove is what holds the
    reader to the two tools. A path that cannot be placed is read as inside.
  - **A shell write is placed inside only by its literal redirect targets or by naming a repository
    operation.** Redirects to literal absolute paths the gate places outside every repository are not
    held: each `>`, `>>`
    or `N>` target (discarded streams ignored) must be a plain absolute word with no glob, `~`, `=`
    or brace, and none may lie inside the checkout. The line itself must be one plain command whose
    program only prints — `echo`, `printf`, `cat`, `ls`, `grep`, `rg`, `head`, `tail`, `wc`, `date`
    or `pwd` — and any quote, `$`, backtick, backslash, `<`, `|`, `;`, `&`, parenthesis, line break
    or `..` refuses it, so nothing on it is parsed for hidden commands. A line the gate classified as
    a repository operation (`git commit`, `git reset --hard`, a branch, `gh issue develop`) is placed
    inside. **Everything else a shell line can write is unplaceable**: a redirect on a line the gate
    cannot reduce to that one plain command — a `cd` before it (which decides where a relative target
    lands), a variable, a quote, `&&`, `;`, `|` — a line where `cp`, `mv`, `sed -i`, `tee`, a
    formatter or another writer is also present, and inline interpreter code (`bash -c`, `sh -c`,
    `python -c`, `eval`). Under `required` an unplaceable write is refused; under `ask` it is allowed,
    with a dirty checkout reported at the next session start rather than held. A symlink outside the checkout that points into it is not
    resolved: a redirect through one passes.
    **Exactly one shape is added to that rule**, recognised by `shell::quoted_heredoc_target`:
    a first line that is exactly `cat > TARGET <<'DELIM'`, `cat >> TARGET <<'DELIM'` or
    `cat <<'DELIM' > TARGET` (or `>>`), spaces or tabs between the tokens in any number, the delimiter
    quoted with `'` or `"` and made of `[A-Za-z0-9_]`, nothing else on the line; `TARGET` a literal
    absolute path of `[A-Za-z0-9_./+@%:,-]` with no `..`; then any body, then a line that is exactly
    `DELIM`, then nothing but one optional newline. The body is **not read**, so a quoted heredoc is
    how an issue body is drafted in the scratchpad. `TARGET` is then checked against the checkout as
    any redirect target is. Everything else is unplaceable — refused under `required` and allowed
    under `ask`: an unquoted delimiter, `<<-`, `<<<`, a target that is not a literal absolute path,
    anything after the closing delimiter or on the first line (`&&`, `;`, `|`), a head other than the
    bare word `cat` (`/bin/cat` too), a delimiter that never closes, and every arithmetic shift, comment
    or line continuation near a `<<`, which matches no shape and so is placed nowhere. A line whose
    heredoc body names a `REPOSITORY_SHELL` command (a `git commit`) is classified by that command —
    not as a redirect, and stays held — only where the head is not one of the read-only commands the
    suppression above trusts; under a read-only head (`cat`, `gh`) the body is data and the line is
    classified by its visible redirect instead. A body naming a boundary or a disarm is read where
    the head is an executor, or where the body is under an unquoted delimiter and the name sits in
    command substitution (`cat <<EOF` with a `$(git push)` line; a `$(`, a backslash or `${` in an
    unquoted body makes the whole line read, not only the span); a literal name in a body read by a
    read-only head, quoted or not, is not. An "any shape" relaxation, reading
    every redirect target whatever the line, was tried and abandoned after five review rounds each
    found a shell-grammar case the reader modelled differently from bash (operator decision
    2026-10-02). A file may also be written with the agent's file-writing tool to an absolute path
    outside the checkout, which this row does not hold. This decides this row only; the claimed-run
    checks and the boundaries classify the same line as before.
  - **The write `ask` allows is reported, not proven.** A run that holds no issue and whose
    `Unclaimed write policy` is `ask` is told the checkout's uncommitted paths at `SessionStart`, so the
    residue of a write the gate could not place — `bash -c 'echo x > src/lib.rs'` is the shape the
    operator raised — is named rather than silent. What it cannot do: prove those paths came from an
    allowed write rather than from the operator's own uncommitted work; see a `git status` that
    fails, which says nothing rather than reporting a clean tree; name more than eight paths at once
    (a count follows the eight); or undo anything. A publication's refusal of a dirty review target
    is the other boundary, and it refuses rather than reports.
  - **A write inside an isolated checkout that a claim names is exempt, whichever session made it.**
    A delegated writer (an OpenCode child session derives its own run) holds no claim of its own, and
    the claim, not the session, is what the requirement asks for. The exemption reads every run
    pointer holding an issue and applies when its recorded `worktree` covers both the directory the
    call runs in and the write's landing — the checkout enclosing an absolute target, or that same
    directory for a relative one — and its `worktree` differs from its `repo_dir`, so a pointer whose
    `worktree` equals its `repo_dir` grants nothing. Its boundary: it is
    path-based, so any session with that directory as its working directory is exempt, claimed or
    not, for as long as the pointer holds an issue — there is no expiry or supersession check; a
    write in the base checkout is still asked; an unreadable pointer is skipped, so it grants
    nothing. Measured at the library level, not by driving `estigia gate`
    with a child `--session-id`, and not against a live OpenCode host.
  - **The stand-down covers an `ask`** the same way it covers a refusal, and records the allowance.
  - **The exemption is the one `unclaimed::applies` uses.** A tracker that cannot adjudicate an
    unsworn write (`Tracker::adjudicates_unclaimed_writes`: a transport, or a read-only tracker under
    `Workflow: assisted`) or a missing contract file stands aside. Linear without `assisted` stands
    aside, which `a_read_only_tracker_without_an_assisted_workflow_refuses_nothing` measures.
  - Measured by the tests in `src/harness/issue_requirement/tests.rs`, which go red with `decision`
    returning `None`.
- **`Claim workflow` `assisted` takes an issue through a local claim and publishes it as a draft pull
  request.** Under `assisted` every tool in `TRACKER_WRITERS` is
  refused with `workflow-assisted-writes-no-tracker`, `estigia goal` answers no claimable or
  dischargeable work (its human line says the queues were not read, its JSON keeps the zeros a loop
  plugin stops on), and the `assist` tool, offered only there, reads the issue and makes the
  isolated checkout without writing the tracker: no claim, no label, no comment, no sidebar link.
  The claim it leaves is a local run pointer, which `harness::assisted` reads against the ownership
  marker inside the worktree and the branch it has checked out, so a pointer whose marker names
  another run, whose branch was switched or whose worktree is gone is refused at the next boundary
  or once the renewal window ends. The tracker's
  `verify-claim` is never asked about such a pointer, and `doctor` reports an assisted pointer
  separately, flagging one whose worktree is gone. A write under the pointer in this repository is
  admitted only inside its worktree, on its branch, after that local check; a write in another
  repository is outside this claim, not refused by it. A relative target is placed from the
  directory the call runs in, `..` included, and a write tool whose paths live in its body
  (`apply_patch`, `patch`) is refused with `assisted-write-unplaced`, as is a path named exactly
  like a write tool. Delivery is refused, and so is `gh pr create` from a shell, with the `draft_pr` tool named
  as the way out. The same `assisted-write-unplaced` refusal covers a shell command whose
  destination the gate cannot place, `patch -p1 < x.diff` run inside the worktree being the case
  measured: its target is classified as the name `patch`, and it is refused, not admitted.
  - **`git push origin HEAD` is refused.** Measured against real git with a recording `pre-push`
    hook (`git_hands_the_hook_head_and_not_the_branch_for_a_push_of_head`): the hook reads `HEAD` as
    the local ref and `refs/heads/<branch>` as the remote ref, so the local ref is not the run's
    branch. The refusal says to push the branch by name; `draft_pr` pushes it itself. A push line
    with no remote ref has its own refusal, and an empty ref list is the only case read as the hook
    not having spoken.
  - **The renewal window rides a local check, not a tracker answer.** Only `harness::assisted` stamps
    an assisted pointer, and only after the marker and `git symbolic-ref HEAD` agreed; a routine
    write may then ride the window, so a marker or branch changed inside it is seen at the next
    boundary and not before. Every effect that would stamp a pointer clears the stamp when the
    pointer is assisted, and a later `claim` resets `assisted` and `branch`. Measured by
    `no_effect_but_the_assisted_decision_leaves_a_stamp_on_an_assisted_pointer` and
    `a_later_swear_makes_the_pointer_a_tracker_claim_again`.
  - **An unsworn session in the worktree is not stopped by this.** `issue_requirement` reads an
    assisted pointer's worktree as claimed for a routine write, whichever session writes there, and
    `unclaimed::refusal` catches a commit only where the push guard is installed. A `git push` from
    that session is the exception: it is held by `Unclaimed write policy` wherever it runs.
  - **`assist` adopts an existing branch through its pull request, read as text.** A branch origin
    already has is taken only when it heads exactly one open pull request whose title, body or head
    branch names the issue (`#12` for a number, `ABC-102` for a key, case-insensitive, as a whole
    reference); otherwise it is refused `assist-branch-not-adoptable`, and a failed read stops before
    any repository write. The mention is the reference Linear's GitHub integration links by, but
    Linear's own attachment is not read, the pull request's author is not checked, and a branch this
    run published through `draft_pr` with a title and body that never named the issue cannot be
    taken again by `assist`. Measured by the `assist_*existing_branch*`, `assist_adopts_*` and
    `a_pull_request_names_an_issue_only_as_a_whole_reference` tests in `src/transport/branch/tests.rs`
    against a real bare remote and a scripted `gh`. One run holds one issue, so correcting several
    pull requests at once is one session per pull request, each adopting its own branch.
  - **The marker and the branch are bytes the agent can write.** A shell command inside the
    worktree can rewrite the marker or switch the branch back, and nothing places that write.
  - **`draft_pr` publishes a draft and guarantees little else.** It pushes
    the branch the assisted pointer names fast-forward, opens a draft with the title and body as
    given and reads it back as open, a draft and at the pushed head. It writes nothing to the
    tracker, and it adds no marker, receipt or `Refs` line, so nothing on the tracker ties the pull
    request to the run: the person reads the URL it reports. What it does not guarantee:
    - **The team's own CI and reviews are theirs.** Nothing waits for them, dispatches a lane or
      records a verdict, and a draft that was read back says nothing about whether it will pass.
    - **A closing keyword is refused where Estigia can read it** — the title, the body and the
      branch's commit messages — and not where it cannot: an edit made on the host after the read-back,
      or a keyword in a commit the person pushes later.
    - **The pointer is read, not the issue.** The branch comes from the run pointer, and the call
      checks that the checkout has it checked out; it does not check that the branch is the one named
      for the issue, because Estigia cannot tell that the person named it (see below).
    - **An open pull request that is ready for review is handled as `Corrections after ready` says**:
      refused and never converted under `refuse`, the `assisted` value; asked back to draft and read
      back before the push under `redraft`; pushed to as it stands under `push-ready`. Under
      `push-ready` the push reaches a pull request people may be reviewing, and its CI and its
      reviewers see the new head at once — which is the point of the value, and also why it is not
      the `assisted` default. A draft that already exists is reused as it stands: its title and body
      are not replaced by the new ones, and the result says so with `"title_and_body_applied": false`.
    - **A failed read-back is not a failed push.** Whether the read-back disagrees or `gh pr view`
      itself errors, it answers in the `committed` world: the push
      landed and a pull request may exist, so none is reported and the same call reuses an open draft.
    - **Measured** by the tests in `src/transport/draft/tests.rs` and the `draft_pr` tests in
      `src/harness/mcp/tests.rs`, against a fake `gh`, not a live host.
  - **`ready_pr` marks ready what was pushed, not what was reviewed.** It checks `Ready by` and that
    the pull request's head is the checkout's, and reads the result back. It cannot see whether the
    run reviewed its own change first: under assisted work no receipt binds a review to bytes, so
    `Self review` is an instruction the run follows, not a gate. Measured by the `ready_*` tests in
    `src/transport/draft/tests.rs`, against a fake `gh`.
  - **Coordination is left to the team.** Nothing reads another person's claim, an assignee or a
    label, and two people can still take the same issue; `assist` refuses a second run on one branch
    on this machine only.
  - **Estigia cannot tell that the person, not the agent, named the issue.** `assist` takes the
    number it is handed. The row is the person's decision; who picked the issue is not recorded
    anywhere that can be checked.
  - **An unclaimed shell write to the repository's own `estigia.local.md` can set `assisted`**,
    the same gap as `Unclaimed write policy: off`: the row is read from a file the agent can edit when it
    holds no claim, and the gate cannot place a shell write.
  - **The push guard is loosened.** "Work here goes through a claim" is met by a local claim. A push
    from the claimed worktree is admitted when every line git names on the pre-push hook's stdin has
    the run's own branch as both its local and its remote ref, so `branch:main`, a tag and a
    deletion are refused with `assisted-push-unconfined`; a push the agent types in a shell names no refs to the `PreToolUse` gate
    and is denied there.
  - **An unreadable contract is not treated alike by every tool.** `GateContext.workflow` is `None`
    when the contract could not be read, and the two assisted paths fail closed on it: `assist` is
    refused and an assisted pointer is `assisted-claim-not-in-force`. The tracker writers are not
    refused on `None`, because that would stop every tracker write in every repository whenever any
    one unrelated row was malformed, which is wider than what the gate does on an unreadable
    contract anywhere else (it narrows a grant, as `Evidence standard` does, and does not stop the
    work). So a broken contract in a repository whose row says `assisted` leaves the tracker writes
    open until the file is read again. Measured by `an_unreadable_contract_names_no_workflow_at_all`
    and `an_assisted_only_tool_is_refused_unless_the_row_says_assisted`.
  - What holds the refusals is a table: a test classifies every tool in the server's table as
    exactly one of tracker writer, assisted-only, local writer or read, so a tool added later fails
    until it is placed, but a writer a person runs by hand through `gh`, or through the transport
    directly, is not a tool call and is not refused, and a tool listed as a read is not shown to only
    read. Not declared as a `Population`: the classification is a closed list a test crosses, not a
    guard whose breadth is a judgement.
  - Confinement is path-based. A shell write names no path the gate can place, so it is judged by the
    directory it runs in alone, and a write through a symlink is placed through it only when the
    link exists.
- **The screen's door is still unmeasured.** `the_screens_install_proceeds_on_a_source_build`
  drives `guided_install`, which always consents a source build: a person confirming the screen is
  that consent, and there is no `--allow-source-build` on `estigia tui`. Nothing asserts that
  `dispatch` does not put a lifecycle preflight back in front of `guided`, so a door refusal would
  take the screen away again with the suite green. That lives behind `tui::setup`, which needs a
  terminal, and no test in this crate can hold one. What stands in is a guard over
  `install_planned`'s production call sites:
  `every_production_caller_of_install_planned_is_preceded_by_the_lifecycle_preflight`, declared as
  `install-planned-callers`, reads the stripped source text of every `src/` file outside a
  `*tests.rs` module and fails, naming the rule, when a call site is not preceded by a
  `lifecycle_preflight` call in its own function. It is a text guard, so a call written inside a
  `#[cfg(test)]` block in a production file, or reached through a macro, a function pointer or a
  `use` alias, escapes it: it catches the copying and the reverting, not deliberate circumvention.
  Direct `setup` and `sync` still refuse a source build without the flag,
  and `tests/lifecycle.rs` still drives those.
- **The screen writes three things before any install, and the lifecycle gate covers none of them.**
  The guard key writes a `pre-push` hook, the language key the remembered tongue under the home, and
  `n` in the folder picker a directory; none is an adapter asset, and no preflight stands in front of
  any of them. `docs/lifecycle.md` states this beside the install, and this is the register where a
  reader looks for what the gate does not cover.
- **The screen's event loop is still unmeasured, so the repository it acts on is held by prose.**
  `run` needs a terminal and no test here can hold one. `install_guard` and `report_of_checks` take
  the repository as an argument and are tested against a temporary one, and `App::press` answers
  Ctrl-C and the unsaved warning in tests; that `run` passes `app.repository` to them, keeps a guard
  refusal apart from an install refusal, and calls `make_folder` and `model_catalog_failed` is a reading, not an
  assertion. Turning those call sites back to the working directory leaves the suite green.
  The decision to read the checkouts and the folders at all is `Reads::refresh`, which is measured
  with a counting `World`, and so is what it forgets: closing the picker, and the `Reload` and
  `MakeFolder` actions through `Reads::performed`, each held by a test that goes red when that one
  forget is dropped. The screen records no checkout itself (`remember_repository` is the CLI's), so
  nothing else makes the list stale while it runs. That `run` calls `refresh` before each key and `performed` with
  every action it receives is a reading, not an assertion.
- **The scroll limit of the read-only pages emulates ratatui's word wrap rather than asking it.**
  `wrapped_rows` in `src/tui.rs` counts the rows a line takes by walking its words, because
  `Paragraph::line_count` sits behind ratatui's `unstable-rendered-line-info` feature. It is checked
  only against Help in both tongues and one synthetic long row; a divergence from the real wrap
  would put the scroll limit off by the difference, hiding the last rows or offering blank ones.
- **The legacy-checkout stop cannot name the legacy path of a template that named no placeholder.**
  A `Worktree location` missing `<branch>` or `<run-id>` gains it in memory, and `start_branch` stops
  with `legacy-worktree-registered` when the pre-migration checkout is still registered — computed
  from the raw template. For a template naming `<run-id>` and not `<branch>`, that raw path is
  exactly what an earlier build created, so the stop fires. For a bare directory naming **neither**,
  it is a path no build created: what they created was that directory with `~<run-id>` appended. So a run upgrading into this build and resuming such a branch is not stopped
  by *this* stop; it is stopped by `branch-checked-out-elsewhere`, which names the checkout still
  holding the branch, before any remote state is written and with no data lost. The gap this entry
  names is unchanged — the legacy stop still cannot compute that path — and what narrowed is the
  consequence: it is no longer git's own text that answers. The recovery is the same
  `git worktree remove`, and it is written in the binding rather than enforced here. Measured by the blind reviewer of receipt
  `6b192e0ee94f11004b621c06c8e8e5dd` across twenty-four template shapes.
- **Four of the twenty are about the past.** `silence`: a call the gate cannot decide on — a
  payload it cannot parse, or one that never arrived — is waved through, and that is the right
  answer: a schema this build does not know could be wrapping a read as easily as a write. What is
  wrong is doing it quietly. Both leave a ledger line, and `doctor` is what reads those lines back,
  because an operator opens the ledger after being stopped and this is the case where they never
  were. `stale-run-pointer` joined it in `LOOKS_BACK` for a narrower reason: filtered to this
  repository, everything it can name is a pointer some *other* run wrote, so a stale or unread
  answer for that pointer says nothing about whether a fresh claim by a different run could swear
  right now — `doctor`'s own `LOOKS_BACK` declaration states this in full. `prune` joined for the
  same reason: everything it names is a branch or a checkout a run that already happened left
  behind, and a lingering branch stops no fresh claim from swearing. `gate-version` joined for
  the sharpest version of the same reason: the whole point of the row is the machine whose gate is
  older than the code it reads, and that is the machine, by construction, that runs `doctor` **as
  the stale gate** — blocking readiness there would be the permanent false negative `silence` was
  filed against, returned in the shape that matters most. It reports *stale* and names the reinstall;
  it does not pretend the environment cannot swear, because it can.
  `store` is the fifth row that is **not** among them, on purpose: every claim, renewal and release
  now decides from the local workflow store, so a file that will not open is exactly why a run
  cannot swear, and the row blocks readiness like `gh` does.
- **The claim family decides from the local workflow store, and writes only to it.**
  What is posted is narrowed below. `claim`, `reclaim`, `unassign`, `heartbeat`, `start-branch`, `transition`
  and `verify-claim` — and so every gated write, which renews first — read ownership and workflow
  state from `~/.estigia/state.db`. An issue is imported on first touch, with one `gh issue view`
  read, and refused as `import-diverged` when the holder or the latest publication would differ.
  After that the open-or-closed question is one REST read, `gh api repos/{owner}/{repo}/issues/<n>`,
  which also says whose issue it is, beside the comments sync below: measured by the call counter, a renewal on an open, held issue
  was one GraphQL call before and is two REST calls and no GraphQL call now (the first touch adds
  the one `issue view` import). On a **closed** issue `verify-claim` proves the close was the review's registered
  delivery, whichever run published it, from the `published` receipt in the store, and asks GitHub only
  for the pull request that closed the issue
  and its merge commit. It read the issue's comments over GraphQL for that receipt until
  nothing posted the receipt there any more, so every delivery's own close was refused as
  `issue-not-open` once the review family stopped posting, which
  `an_own_delivery_close_is_proved_from_the_store_when_github_carries_no_marker` now holds. The assignee and
  `dev:`/`status:` labels are still read for the projection, and are never what decides.
  Slice a2 posted every marker to GitHub and appended the same entry to the store in the same
  operation. Slice d1a stopped that for `claim`, `reclaim`, `unassign`, `heartbeat`/`progress`/
  `blocked` and the `branch` marker of `start-branch` (the next entry says what that costs); the
  review markers (`published`, `review-handoff`, finding, refutation, disposition, spend, verdict)
  were still posted to GitHub until slice d1b, which also stopped that; a store that cannot record
  now fails the read, and nothing at GitHub changed.
  The store holds what Estigia itself wrote — `claim`, `reclaim`,
  `unassign`, `heartbeat`/`progress`/`blocked`, `branch`, and the review markers
  (`published`, `review-handoff`, finding, refutation, disposition, spend, verdict), because
  `claim` and `reclaim` read the publication and handoff to refuse a requester — plus the control
  instructions the sync finds on GitHub. The costs, each measured or read off the code:
  - **After the first touch only a person's stand-down or adjudication is admitted from GitHub, and
    nothing that hands the issue to someone.** Opening an issue's ledger reads
    `repos/{owner}/{repo}/issues/<n>/comments` since the newest comment already synced (a page of 100
    at a time) and appends the ones the store lacks **whose markers are all `standdown` or
    `adjudication`, written by the authenticated user and never edited**; a comment with any other
    marker (`claim`, `reclaim`, `unassign`, a review marker), a mixed comment, an edited or
    foreign-authored one and every prose comment are left on GitHub and decide nothing
    (`after_the_first_touch_only_a_stop_is_admitted_from_github_comments`). So a run on an older
    binary that writes a `claim` to GitHub only is not seen, and a comment cannot change an issue's
    state, receipts, findings, verdicts or spend, or give it to another run; it can only stop the
    run it names (`a_person_commenting_a_standdown_after_the_import_releases_the_run_it_names`,
    `a_reclaim_commented_after_the_import_decides_nothing`). A stand-down aimed at a claim that
    carries an operation id still does nothing, which is the ownership fold's rule and not this
    sync's: only a stand-down scoped to that operation releases it, and a comment cannot carry
    one, so a person stops such a run with an `adjudication`. A comment is known by its `node_id`, the id GraphQL gave the
    import. Edits made **after** a comment was synced are not re-read, and labels, assignees and the
    `status:` label are still only read at import, so a hand-moved label still does not reach the
    store.
  - **GitHub's order decides only for what GitHub carries.** A row with a GitHub counterpart
    carries GitHub's `created_at`: a synced stand-down or adjudication keeps it unclamped. A claim-family
    marker has no GitHub counterpart, so its stamp is the local clock of the run that wrote it, and
    the claim is decided in the transaction that wrote it with no resync afterwards. A stand-down
    or adjudication a person posted to GitHub arrives through the sync and is ordered by GitHub's clock. A local entry is stamped no earlier than the
    newest stamp on the issue, so the stamps are not monotonic in insertion order, and the reducer
    orders by stamp and then position as it always did. Rows that share a stamp are given positions
    in GitHub's order: a synced or matched row carries GitHub's numeric comment id (an imported row,
    its `issuecomment-<id>` URL when the payload has one), imported rows without an id sort first,
    and a row GitHub has not listed yet, which includes every claim-family marker, sorts last.
    Rows with distinct stamps keep the position they were recorded at. The limits: a tie between
    two rows GitHub has not listed yet falls to insertion order, which can differ between two
    stores; and a comment GitHub does not list yet (its list lags) is decided on the local stamp
    until the next sync. A sync that meets 10,000 or more comments since its watermark is a failed
    read.
  - **The REST trust derivation is stricter than GraphQL's and never looser.** REST carries no
    `viewerDidAuthor` or `includesCreatedEdit`. A synced comment is `viewer_did_author` only when
    its `user.login` equals the login `gh api user` answers (asked once per thread, and only when
    there is something new to classify; an unreadable login fails the read) and
    `includes_created_edit` whenever `updated_at` differs from `created_at`, which is true of every
    comment GraphQL would flag and of every comment edited later too. So a comment edited after
    posting is untrusted here where GraphQL could trust it, and a missing author is untrusted. The
    comparison is by login, so two tokens for one account are one author; GraphQL's
    `viewerDidAuthor` is the same fact.
  - **The state compared is the store's.** It is set at import from the single `status:` label (two
    of them, or none, import as no state) and moved by `transition`, which records it after the
    label edit lands. An issue with no state answers `unexpected-state` until a `transition`.
  - **A lost claim race rolls back; a lost reclaim race still commits.** The claim is one
    `BEGIN IMMEDIATE` transaction, so two runs on one store are serialised and the first committed
    holds, unless GitHub orders another run first (see "GitHub's order decides only for what
    GitHub carries" above). A fresh `claim` that writes its marker and then loses answers
    `lost-claim-race` with the world unchanged and **rolls its marker back**, so the store keeps
    only the winner's row and the loser makes no `dev:` label. `reclaim` adjudicates after its
    transaction has committed, so a `lost-reclaim-race` loser's row still stays.
  - **Reclaim validation used to resurrect scoped releases.** Discovery and preflight
    honored operation-scoped unassigns, but `ownership::reclaim_is_valid` reduced its prefix with
    an empty epoch-release map. With the released acquisitions' horizons still live, a forced
    takeover of the exact discovered holder committed and then answered `reclaim-discarded`,
    `target-not-holder`, naming that same holder as `winner`. Closed by deriving the prefix's
    scoped releases with `epoch_release_positions`, using only already accepted reclaims.
    `a_forced_reclaim_after_one_scoped_release_reads_back` and
    `a_forced_reclaim_after_two_scoped_releases_reads_back` in `src/transport/claim/tests.rs`
    drive discovery, the forced exact-target write and store read-back. Both failed with that
    diagnostic before the correction; replacing the corrected map with an empty one makes both
    fail again. `a_discarded_reclaim_releases_nothing_in_a_later_reclaims_prefix` checks that a
    refused takeover neither releases its target for a later validation nor becomes an
    authoritative activity/ownership event. These are deterministic scripted-tracker tests,
    not a recovery of any live issue.
  - **The claim's label follows the commit.** A `dev:` label that cannot be made after the store
    accepted the claim is `write-failed`, and the claim is held; the retry with the same operation
    id finds the stored claim, writes no second one, and runs the projection again.
  - **A reclaim whose target left the fetched window is unreachable here.** The store has no
    window, so `reclaim-discarded` with the condition `target-missing` can no longer be reached
    over the door. The pure `won_the_reclaim` still answers it, and its unit test stays.
  - **The differential check compares the holder** (comment ids included: an imported row keeps
    GitHub's comment id, so a `legacy-<id>` epoch is the same over both sources; ids are set aside
    only when GitHub reported a comment without one) **and the latest publication, and nothing
    else**: a store that drifts on findings, verdicts, spend or handoffs imports cleanly.
- **The claim family writes only the local store; GitHub carries labels and the assignee.**
  `claim`, `reclaim`, `unassign`, `heartbeat`/`progress`/`blocked` and the
  `branch` marker of `start-branch` record their marker in `~/.estigia/state.db` and post no
  comment, and no human sentence either (`Claimed by ..., expect to report by ...` is gone). What
  they still write to GitHub is the projection, after the local commit: the `dev:` label (made when
  the claim is recorded), the assignee and the `dev:` labels that follow the holder, and the develop
  link and branch of `start-branch`. The review family followed in slice d1b, the next entry. The
  `heartbeat` answer says `recorded`, not `posted`. The costs:
  - **An older binary on the same issue cannot see these markers.** A run on a binary before this
    slice reads GitHub only, so it sees no claim, no heartbeat and no release written after the
    change, and may claim an issue this binary holds. Its own markers still reach this binary through the
    sync; ours do not reach it. Nothing detects the mix.
  - **A person reading the issue page sees no claim comment.** The holder is the assignee and the
    `dev:` label, which are a projection and can lag the store or be edited by hand without the
    store noticing.
  - **The removed retry.** The store no longer holds an entry waiting for a post, so a crash between
    the commit and the label leaves a recorded claim whose label is made by the next projection,
    not by the retry of the claim itself.
- **The review family writes only the local store; GitHub carries human prose.**
  `publish_review` and `republish_review` (the `published` receipt and its
  unconfirmed form), `handoff_review`, the recorders of finding, lens report, refutation,
  disposition, validator result, spend and computed verdict, and the `review-receipt` marker of an
  approval or an escalation record their marker, and the prose beside it, in `~/.estigia/state.db`
  and post no comment. The posting machinery with no caller left is gone: the staging and `gh issue
  comment` step of the shared recorder, `Ledger::record` and the "GitHub took the comment" failure,
  so a store that cannot record is a failed read and nothing at GitHub changed. Two writes the
  claim-family slice left alone changed with it. The `comment` tool still posts a person's prose
  to the issue, but the body is the escaped prose alone: the `note`/`blocker`/`diagnosis` marker is
  no longer appended, and `--kind` and `--run-id` are validated as a pair and decide no text. `create`
  with `--epoch` no longer renders a `review` marker into the filed body; the marker is recorded in
  the store in the transaction that admits the issue, where nothing reads it today, so it is kept
  for the record and not for a reader. What GitHub holds now is issues and their prose, human
  comments, labels, open or closed, pull requests and branches.
  The sweep is a guard in the stand-in `gh` (`examples/fake_process.rs`): it exits `97` for any
  `--body-file` carrying an `issue-flow:` marker, whatever the subcommand, so every test that drives
  the stand-in sweeps the bodies of the operations it runs — publication, handoff, finding, lens
  report, refutation, disposition, spend, verdict and the `comment` and `create` tools. The costs,
  each read off the code or the tests rather than measured against a live repository:
  - **There is no single chained lifecycle test.** The sweep is the union of the per-operation
    pipe and unit tests under the guard, not one `create` → `claim` → `publish_review` → … →
    `release` run on one issue; a marker only a chained run would write is not seen. The guard is
    a stand-in, so a real `gh` posting a body some other way is not seen either.
  - **An older binary cannot see these markers.** A run on a binary before this slice reads GitHub
    only, so it sees no publication, no verdict and no spend written after the change; its merge
    gate reads no receipt and may clear a head this binary has not reviewed. Nothing detects the
    mix; the cure is reinstalling every agent profile before an issue is in flight on both.
  - **A reviewer or a person reading the pull request sees no receipt comment.** The review state is
    in the store and in `review_status`; the issue page shows labels, the assignee and the pull
    request, nothing of the lens reports or the verdict.
  - **`create`'s store admission is recoverable only by title.** `create` records a pending
    record in the store before it files, and clears it when the issue is admitted. If the
    admission fails after the issue was filed, the refusal names the issue (`#N`) and says a repeat
    adopts it; the repeat searches `gh issue list --search "\"<title>\" in:title"` (the title as one quoted phrase, any `"` in it replaced by a space) for an issue titled
    exactly the same and adopts a single match instead of filing again. It refuses when two issues
    carry the title, and it files a new issue when the search finds none, so a search index that
    has not yet caught up with the issue it just created (a lag nobody measured here) would let
    the retry file a duplicate. The store is also opened, and the repository resolved, before the
    issue is filed now, so an unreadable store refuses before anything is created.
  - **A retried `claim` makes its label again.** The first attempt can record the claim and fail
    on the `dev:` label; the retry under the same operation id writes no second marker and used to
    skip the label, contradicting the retry the documents describe. It now makes the label on every
    call that holds the claim, one more `gh label create --force` per retry.
- **Review evidence decides from the same store, and a few reads of the timeline still go to
  GitHub.** `review_transaction`, `repair_rounds`, `review_findings`,
  `review_payload_membership`, `spend_summary`, `goal_completion`, `release_ci`, the review
  recorders (finding, refutation, disposition, spend, verdict), `handoff_review`, the lineage reads
  of `publish_review` and `republish_review` and the review queue's per-candidate eligibility read
  take their timeline from the ledger, so a verdict, a release or a findings read on an issue the
  store already holds makes no `gh issue view` call over GraphQL: it makes the two REST calls every
  ledger open makes, the issue and the comments since the last sync, and a marker another run or
  a person posted on GitHub is seen through that sync. Each timeline read is its own ledger open,
  so an operation that reads the timeline several times pays those two calls each time, where it
  paid one GraphQL call; `record_spend` opens the ledger once, for the receipt check, the claim
  check, the write and the readback (two REST calls, where it was eight); a `release` that folds
  its close in opens it once and reuses it for every spend record's receipt check and write, the
  epoch check, the state read, the move to `done` and the release (two REST calls, where one spend
  record took twelve), and `claim` with `transition_to` opens it once for both (two, where it was
  four); `publish_review`
  opens it once at the top and for the entry it records after the push, and pays one more comments
  sync at the lineage read, before the pull request is touched, and one full open before a leased
  push, so a stand-down reaches the store ahead of each boundary it could stop; the capture
  family is the exception, because `launch_review_input` opens
  the ledger once and reuses it for the claim check, the timeline read and the slot write, and
  `collect_review_input` reuses the ledger it opened for the admission's policy and lens-precheck
  reads — the recorders the admission then calls still open their own, one per read they make. A
  store or REST failure is a failed read; anything after a GitHub write is
  `write-failed`. Still over GraphQL, on purpose or not yet moved: the closing-reference read that
  proves a registered delivery close (`verify-claim`, `completed_count`), the projection convergence
  reads of `claim`, `reclaim` and `unassign`, the pre-write `assignees,labels` read `claim` takes
  for the `foreign-projection` check, and the `labels` read `handoff_review` takes for
  its release plan and its final check. Because a marker posted by this run is recorded in the
  store before the next read, the visibility loops that followed each write now see it at once
  and no longer wait for GitHub to list it. The in-flight review identity is named
  `ReviewTransaction` in the code and `review_transaction` in the run pointer, which is still
  read under its old name `review_receipt`. The tool name, the refusal codes and the prose were
  renamed to that vocabulary; the old tool name `publication_receipt` is still
  accepted as an alias.
- **The queues, the dedup keys and the completion count decide from the same store, and a queue
  read no longer reads a candidate.** `list-state`, `list-identities` and
  `completed_count` read the open issues from one REST list
  (`repos/{owner}/{repo}/issues?state=open`, with no label filter) and then decide each issue's
  state, holder, identity and dedup key from `~/.estigia/state.db`: a `status:` label never decides,
  and an open issue the store has never admitted is reported under `unadmitted` rather than placed
  in a state a label claims. `list-state` reads each candidate's ownership fold, and for `review`
  its eligibility, from the store timeline, so a queue read makes no `gh issue view` call over
  GraphQL; `blocked` clauses come from the store, imported from the issue body's `blocked-until`
  markers on first touch and evaluated against the store's own states. `create` admits the issue it
  files to the store, so a freshly filed `ready` or `blocked` issue is seen by the next queue read
  without touching GitHub again. `completed_count` takes its candidates from the store's `done`
  issues and its evidence from the store timeline; only the close and merge proof still reads
  GitHub's own objects. The costs: the candidate list is the open issues read in full and cut to the
  oldest 200 **open** issues across every state, not the oldest 200 of the state asked for, so a
  state whose items are all newer than that cut is not seen until the store holds fewer open issues;
  and the review
  queue's per-candidate timeline is the store's own, read without the sync a ledger open pays, so a
  handoff another machine posted is seen only after some operation opens that issue and syncs it.
  `estigia store import` admits every open issue the store has never touched, oldest first, so a
  store that has never seen an open issue has one command that clears its `unadmitted` report; an
  issue the store already holds is counted and left alone, and one whose history diverges stops the
  import with a refusal naming it and writes nothing for it. `estigia goal` reports that count as
  `unadmitted` (`null` when the `ready` read failed, never `0`), and the loop does not treat those
  issues as work: with no live claim and nothing claimable or dischargeable it waits, or ends a
  Stop-hook turn without ending the loop, naming the count and saying to admit each with an explicit
  `transition`. No `status:` label is read to place them. A `blocked-until` clause an older body
  still carries is recorded only
  when that issue is imported, so one added to such a body after its first import is not seen; a
  clause recorded with `blocked_until` has no such gap.
- **FLOW's routing rules are instructions, and nothing measures whether a run followed them.**
  `skill/SKILL.md` *FLOW* says when a checklist is written, that a task acknowledgment asks for
  `compact: true`, that a row which is not a commit closes with `evidence`, and the two delivery
  spellings. The store enforces only the `evidence` rule — `task` refuses a `done` row with
  neither a commit nor evidence — and the gate refuses a wrong spelling after it is tried; whether
  work that spans one publication skipped the checklist, or a row named its kind, is the run's
  word. The effect on cost and time is not claimed here: `docs/review-cost.md` holds the baseline,
  and only the validation rerun against it can measure one.
- **The dedup identity, the `blocked-until` clauses and the task checklist are recorded in the store
  and nowhere else.** `create` records the identity and any clauses in the
  transaction that admits the issue and writes no `analysis` or `blocked-until` marker to the body;
  `blocked_until` adds or clears clauses and `task` records the checklist, and none of the three
  writes to GitHub. The costs: a lost or replaced `state.db` loses all three, because no copy on the
  tracker carries them any more, and an issue filed before this change keeps its markers in the body
  and is read through the import until the store holds it. `blocked_until` requires no claim, so any
  run that names itself may add or clear a clause on a blocked issue; the claim-gated `task` is the
  only one of the three that proves who wrote. A `task` is recorded, not verified: nothing checks
  that its commit exists, that its `evidence` note names anything real, or that its text is the
  acceptance the issue states, and a `done` row is the run's word that the work-unit commit exists
  — or, when it carries `evidence` instead, that no commit was required. The no-publication close
  is declared rather than measured: `transition --to done` refuses a `done` whose latest confirmed
  publication is unmerged, but answers nothing to verify when the issue has no confirmed
  publication at all, so a `done` with no publication is accepted whether the issue genuinely
  required no change or a run took the exception where a change did.
- **The store is one copy, and its protection is local and partial.** A lost `~/.estigia/state.db` is imported
  again on the next touch and loses only what GitHub never carried — which since slice c6 includes
  every identity, `blocked-until` clause and task a run recorded, since slice d1a every `claim`,
  `reclaim`, `unassign`, `heartbeat`/`progress`/`blocked` and `branch` marker, and since slice d1b
  every review marker (`published`, `review-handoff`, finding, lens report, refutation,
  disposition, validator result, spend, computed verdict, receipt) of an issue first touched
  afterwards (an issue whose timeline still carries them is read through the import). A lost file
  therefore releases the claim of every in-flight issue it held and loses the review history of
  every pull request in flight, and the issue page still names the run in its assignee and `dev:`
  label while the store does not. The write path snapshots the store with SQLite's `VACUUM INTO`
  into `~/.estigia/snapshots/` when the newest snapshot is a day old or older, keeping the newest
  three and counting a snapshot that could not be written rather than failing the operation, and
  `doctor`'s `store` row reports the newest snapshot's age; a snapshot is a point-in-time copy for
  a person to restore by hand, not one Estigia reads back, and the cadence means a day of writes is
  not in the newest copy. The append-only triggers
  refuse an `UPDATE` or `DELETE` through SQLite and are no defence against anyone who can drop
  them: trust moves from "this GitHub account wrote it unedited" to "this OS user's files". Opening
  the store for writing refuses `store-unwritable` when its directory sits on a filesystem this
  build can name as a network mount — the filesystem's own name on macOS, its type magic on Linux —
  before the connection is opened, so a network mount that accepts WAL and loses locking is not
  entered; a platform whose filesystem this build cannot name (Windows) is not detected and is not
  refused. A `claim` reads GitHub's projection and refuses `foreign-projection` when a `dev:*`
  label or assignee names an owner the store cannot explain, so two machines with separate stores
  no longer each hold one issue silently; `reclaim` is the explicit override.
- **A verify, heartbeat or transition still makes two REST reads of GitHub, and neither decides
  who holds the issue or in what state; a `claim` makes those two and one more.** The
  criterion is "makes no GitHub read to
  decide its outcome". Measured by the call counter on the stand-in `gh`
  (`a_gated_write_on_an_open_held_issue_makes_no_graphql_call_and_two_rest_calls`), a `verify-claim`
  on an open, held issue is two REST calls and no GraphQL call. The first reads whether the issue
  is open, which is GitHub's own object and is what stops work on a closed issue; it decides nothing
  about the holder or the state. The second reads the comments since the last sync, and after the
  first touch it admits only a person's stand-down or adjudication (see the sync entry above): an
  `adjudication` naming a run stops it as `control_after` reads it, a `standdown` releases it
  through the ownership fold, and no other comment, marker or prose, changes the store. After the first touch a verify or a heartbeat reads no issue's
  labels, assignee or body, and neither posts a comment
  (`after_the_first_touch_a_verify_and_a_heartbeat_read_no_issue_and_no_label_and_post_nothing`);
  a label GitHub answers with that disagrees with the store decides nothing
  (`a_transition_moves_the_state_the_store_decides_from`). A `claim` is the exception, and by
  design: it reads GitHub's own projection (`gh issue view --json assignees,labels`, over GraphQL)
  and refuses `foreign-projection` when the issue carries a `dev:` label or assignee the store
  cannot explain, so a claim does make a GitHub read that decides whether to refuse. The read is
  not made by `reclaim`, the explicit override. The count is therefore not zero: the
  workflow decision is the store's, and the two reads are GitHub's own open-or-closed object and
  the one channel a person has to stop a run. What closing the second read would take is a local
  command that writes a stand-down or an `adjudication` into the store, which does not exist, and
  until it does an older binary's claim, a `reclaim` comment and an old binary's findings are not
  seen by a newer binary at all.
- **`gh repo view` is asked once per thread for each checkout and named repository, and a `claim`
  reads the projection it refuses on once for the converge.** `repo_identity` keeps the owner and
  name it read (a failed read is not kept), so a `transition` with a board configured makes one
  `gh repo view` where it made two, and the claim's converge takes the `assignees,labels` payload
  the foreign-projection check read as its first read and does not create the `dev:` label the
  claim has just created. The converge's second, agreeing read and its post-edit proof reads are
  still made, so the projection is still read back from GitHub. A repository renamed or
  transferred while a process lives is still called by its old name until the thread ends.
- **The `gh` call counter is best-effort and counts spawns, not cost.** It tallies on the thread
  that runs `transport::dispatch`, at the one place the transport spawns a process, and writes
  one transaction per operation, with a row per kind. A `gh` spawned outside a dispatched
  operation, or from another thread, is not counted; a store that will not open, is busy for more
  than 200 milliseconds or fails to insert drops the tally without telling the operation, because
  the gate's answer must never wait on a counter. `gh issue`, `gh pr`, `gh project` and
  `gh api graphql` count as GraphQL, any other `gh api` as REST, and every other `gh` subcommand
  as `other`, classified from the argument list and not from what GitHub charged. One counted
  operation costs one `synchronous=FULL` commit. Unit tests count only into a path a test names,
  so the suite never writes the developer's own store; the claim family's own reads and writes
  go to a store private to the test thread for the same reason.
- **A run that swore nothing is never gated, including when Estigia is broken.** Failing closed on
  everybody would be a lock rather than authority, and would teach people to remove the hook. So a
  missing transport denies a sworn run's write and lets an unsworn one through — the oath is what
  brings a run inside, and nothing else does.

- **An unreadable run record stops a write; a push stops when no readable holder can cover the
  checkout.** The gate denies the write: a pointer that exists and will not read — bytes that will
  not parse, a directory at its path, a permission failure — says a run existed and not what it
  swore, and an unknown is not clearance. `session::load` treats only a `NotFound` read as absence,
  and only while the directory the pointers live in is one: Windows answers a path whose parent is
  not a directory with `ERROR_PATH_NOT_FOUND`, measured as os error 3 and mapped to `NotFound`, so a
  state root that is a file would otherwise read as *every run swore nothing* and stand every gate
  aside at once. A root that is not there at all stays absence, because that is what a machine looks
  like before anything was claimed. Every other read failure loads as unreadable and carries the
  pointer path and the underlying error into the refusal, so the message names the file an operator
  has to look at. The push guard
  cannot ask one unreadable pointer which checkout it holds, so it denies `run-pointers-unreadable`
  when nothing readable covers the checkout and some holding is unreadable; when a readable holder
  does cover it, the unknown one changes no outcome and the push proceeds on that holder's own
  terms.

- **A contract row the gate cannot read costs that row, and no longer costs the rows beside it.** The
  gate reads the installed contract with a fallback to defaults rather than refusing, because a file
  that will not parse is a problem `doctor` reports and not a reason to stop an agent mid-edit. What
  that fallback loosens is taken back two ways. The renewal window goes to zero, so nothing rides a
  cached answer. And the read keeps every row that parses: this entry used to say the rest "cannot
  be" taken back, on the grounds that the list which made a command a boundary was the thing that
  went missing — true only when the boundary row is *itself* the bad one. It usually was not.
  `Config::read` applied each setting with `?`, so a mistyped `Renewal window` three lines away threw
  a perfectly readable `| Irreversible commands | terraform apply |` out with it, and the gate
  classified `terraform apply` as a routine write. Measured by
  `one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it`, and salvaging can only narrow —
  `Config::default` is the loosest configuration there is, and every value a row can carry is refused
  unless it narrows that.

  What remains is the row itself: a *boundary* row that will not parse still costs the boundaries it
  named, and there is nothing to recover it from.
  `an_unreadable_contract_costs_a_declared_boundary_its_boundary_status` measures what an empty list
  buys the operator. That is the one place where *configuration may only tighten* runs backwards, and
  it is the cost of not refusing.

- **Every script this crate writes reads the binary's answer instead of inheriting it, and a script
  that cannot get one lets the call through with a word.** Only the codes Estigia defines are
  decisions: `1` is a refusal and `2` is an outcome it could not read back. Anything else — a missing
  binary at `127`, a panic at `101` — means it did not answer, and the write or the push goes
  through. The rule is written in the harness (*"a hook that breaks does not deny"*), and **two** of
  the four boundaries did not keep it. The push guard used `exec`, so deleting Estigia or moving it
  left a repository nobody could push from, with a fix nobody would guess. The OpenCode plugin
  caught every failure of its call and threw it as a refusal, so the same missing binary blocked
  **every write in the session** behind a message that was not a refusal —
  `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer` holds it to the three codes now, and
  checks that the file is still JavaScript while it is there. A result with no numeric exit code (a
  process ended by a signal, a missing field, an absent result) used to read as `0` and let the call
  through in silence; `the_plugin_says_so_when_the_gate_returns_no_exit_code` drives the plugin under
  `node` and holds each to the same word, and the goal-loop reader no longer reads it as success.

  What that costs is the other half, and it is the honest half: on a machine whose binary has gone,
  every gated write goes out unmeasured. It says so on standard error each time. Failing closed
  instead would be a harness people uninstall the first time it is wrong at a bad moment, and an
  uninstalled harness gates nothing at all.

- **Nothing here resists a local actor running as you.** The gate reads a run pointer, a contract and
  a hook command, all of them files in your home directory with your permissions. Anything able to
  write them can rewrite what Estigia believes, and a binary that can be replaced answers whatever it
  likes. No authenticity or tamper-resistance is claimed and none is implemented — the threats this
  is built against are accidental: a truncated write, a crash mid-rename, two processes racing on one
  pointer. Somebody who already has your account does not need to defeat Estigia.

- **A stale writer is dropped, not merged.** A pointer on disk written more times than the one in
  hand is news the caller has not seen, and its store is skipped rather than applied. That keeps the
  fresher answer, which is the direction that fails closed; it does not reconcile two concurrent
  edits, and the dropped one is not reported — storing is best effort by design, and turning a
  missed record into a denial would make the gate loud in exactly the situation it should be quiet.

- **The boundary list is compiled, and can only be added to.** Estigia knows git and GitHub; it
  cannot know that a repository ships with `npm publish` or `terraform apply`, so `Irreversible
  commands` lets an operator name those and the gate treats them as boundaries — no renewal window,
  a fresh read every time. It **only adds**: no value removes a built-in, because a setting that can
  make the gate looser is not a guard rail. What it still does not do is let anybody express a rule
  the classifier has no shape for; matching is substring, on a normalised command line whose
  provably-data spans are blanked only when the whole line is one read-only simple command, and
  nothing more.

- **Decisions are written down; the reasoning behind them is not.** Every allow and every deny is
  appended to the local store's `decisions` table with the run, the tool, **what the decision was
  about**, the verdict and the refusal code, so "why did it stop me?" has an answer after the terminal
  scrolls — and so does "what went through?". That second half was missing: a refusal is prefixed
  with its command and an allow was not, so `git tag v1.0`, `gh release create v1.0` and `git push
  --force origin main` under one claim left three identical records reading `tool=Bash verdict=allow
  detail=issue #8 is held by <run>`. The record named everything Estigia stopped and nothing it
  let through, which is the half that changed the world. `Outside` is not recorded — that
  is Estigia standing aside, and a record per tool call of every session that never swore would bury
  the ones that matter. It is written best-effort: a refusal that could not be written down is still
  a refusal.

- **A payload Estigia cannot parse does not stop the call — and now says so.** A schema this build
  does not know could be wrapping `Read` as easily as `Write`, so refusing it would refuse reads;
  the hook stands aside, which is the right answer and has always been the documented one. What was
  wrong was that it was silent. `Outside` is not recorded, so an agent that changed its payload
  shape would take the gate out of the loop for *every* call while looking installed — the exact
  shape of the five defects this project has already paid for. An unreadable body now writes
  `payload-unreadable` to the ledger. It is a trace, not a denial: nothing is refused, and the only
  claim made is that somebody can now find out.

- **The decision history is pruned by age, and the prune is a `SessionStart` job.** Every run on the
  machine appends to one store, so no rotation races another and no record is lost to a rename that
  clobbers the file another run just set aside. Records whose `at` is older than thirty days are
  deleted, at most once every twenty-four hours, and the only thing that runs the prune is
  `SessionStart` — a machine whose sessions never start keeps its old records rather than losing
  them. Two gaps stay, and neither is hidden: a record that could not be appended is counted in the
  store and reported by `doctor`, but that count is itself a best-effort write, and a store that will
  not open at all is reported as the `store` row rather than as a count. The JSONL history this
  replaced is imported in one transaction and then deleted: a failed import keeps its file, leaves
  the one-time marker unset and is counted, and two `SessionStart`s import once between them. A line
  that did not parse is skipped, so whether every record in a torn legacy file survived is not
  measured.

- **Held runs are pointers, not authority.** `status` lists what this machine's runs recorded
  holding, oldest answer first. Whether those claims are still live is the tracker's answer, and
  the list exists to say which issues to go and ask about.

## The populations and the prose they cross

- **Every population declaration is found by walking `src/`, resolved to the item its `subject`
  names, and fingerprinted together with it.** A declaration is now a `pub const … :
  Population` — the type in `src/harness/population.rs`, each const beside the code it describes —
  and not a doc comment, so `tests/guards.rs` reads it with
  `syn` rather than scraping a `#[doc]` attribute. The sources are read off the directory rather than
  listed, so a declaration in a file nobody remembered to add is still checked, and a `subject` naming
  no real item in the same file is refused loudly rather than silently binding to nothing. Changing the
  rule or the code beneath it reopens the claim: `tests/guards.rs` fails and names the new fingerprint,
  and somebody has to read the declaration again before recording it. That proves the pair has been
  read since it last moved — not that any declaration is true, and not that every guard needing one has
  one.

  **A list is a population; the matcher is a decision, and the control-surface one has a
  second subject.** The declaration on `CONTROL_SURFACE` fingerprints the list, so three changes to
  `is_anchored_fragment` moved which paths the gate watched with this registry green, two of them
  narrowings. `CONTROL_SURFACE_CORPUS` is now the matcher's subject: a fixed corpus of paths with the
  answer both roads must give each one, asserted by
  `the_control_surface_matcher_answers_its_corpus` in `src/harness/tests.rs`, so a matcher edit that
  moves an answer reddens that test and recording the new answer reopens the claim. A refactor that
  leaves every answer alone moves neither. The corpus is representative rather than exhaustive, which
  its own declaration states.

  **Where the walk reaches, and what catches the rest.** `collect_found` reads top-level items,
  associated consts inside `impl` blocks, and the bodies of inline `mod`s. It does **not** walk
  statements, so a `const … : Population` written inside a function body is invisible to it — the same
  blind spot the doc-comment mechanism had, still open after the migration, found by a blind judge and
  no longer shouted about by the guard that used to, which had been deleted along with the rest.
  `no_population_declaration_hides_from_the_walk` closes it from the other end: it counts the type
  ascription in each file's comment-stripped text and fails, naming the file, the moment the text
  writes more declarations than the walk found. So a declaration hidden in a function body is refused
  rather than quietly unfingerprinted — the check tells you to move it, and does not pretend to have
  read it where it is.

  **Two spellings still escape both ends.** The ascription both counters match is one constant,
  `harness::population::DECLARATION_ASCRIPTION`, and it is a contiguous substring. A declaration
  written `: population::Population =` does not contain it, and one written behind a type alias
  (`type Pop = Population;` then `: Pop =`) is invisible to the counters *and* to `is_population_type`,
  which resolves the last segment of the written path. Neither is a plausible accident and neither is
  closed. The subject side is guarded: `tests/guards.rs` refuses a `subject` that resolves to more than
  one item in its file, which the widened walk made possible by putting test modules into the same
  resolution pool.
- **The seam guards read two shapes, not every shape.** They resolve markdown links and `<skill>/`
  rooted command lines. A payload that names a file some third way would slip past both — the
  `scripts/github.py` defect did exactly that until the second guard was written.
- **The directive's rules are crossed against the gate by name, not by reading.** The text every
  agent carries in context states three rules, and each is pinned to a named test that holds the gate
  to it — a rule deleted from the prose, or a test renamed out from under one, fails the build. What
  no test can check is whether the sentence still *means* what the test enforces. That reading is a
  person's job, and a guard that implied otherwise would be the false comfort this section exists to
  prevent.

- **The transport could not read the contract Estigia installs, and nothing said so.** Estigia
  renamed the configuration block to `estigia:config:*` and treats the `issue-flow:` pair as
  superseded, so an installed contract carries only the new spelling. The transport knew only the
  old one — and a marker it cannot find is not an error there, it returns an empty table. So after
  **every real `estigia setup`** the transport read *no operator configuration at all*: not the
  tracker, not the merge strategy, not delivery authorisation. It ran on empty
  defaults while the harness enforced the operator's actual table. Nothing failed and nothing was
  logged; the two halves of one decision simply stopped agreeing. The transport now reads either
  spelling. What held it was a crossing that installed through `setup` and required both sides to
  read the same thirteen rows; that crossing is deleted with the rest of the corpus, and the fix
  itself is what remains.

- **Estigia and the transport read the operator table the same way, and that is now checked.** Both
  read `SKILL.md` between the same two markers, and nothing compared them. They did not agree: the
  transport normalises a hand-written cell — backticked token, `**bold**`, an explanation after an
  em dash or in parentheses — and Estigia refused any cell carrying decoration. Refusing was not
  where it ended, because the gate reads the configuration with `unwrap_or_default()`: it would have
  gone on enforcing the *defaults* while the transport honoured what the operator wrote. The shipped
  table teaches that style itself (``| Tracker | `github` | …``), so this was reachable by following
  the example. What crossed the two readers over the same bytes is deleted with the rest of the
  corpus; `a_row_the_binding_acts_on_is_a_row_this_reader_finds` is what still holds this side of
  it, and the other side no longer exists to be held.

## Ownership

- **A legacy ownership epoch hashed the word `None`.** When a comment carries no id, its epoch is a
  digest of `createdAt` and `body` — and the transport reads those with `.get()`, which answers the
  same for a missing key and a JSON `null`. Interpolating that into the hashed string wrote the four
  letters `None` into the identity, so a comment with no `createdAt` hashed as `"None\n<body>"` there
  and as `"\n<body>"` in the port, which takes `&str` and can only say nothing at all. Two epochs for
  one comment is a release that never finds its target, which is the one thing this identity exists
  to prevent — and nothing chose to hash `None`; it is what an f-string does with an absence. Found
  by sweeping the crossings for the shape below rather than by waiting for it: the corpus posed the
  empty string and never the absence.

- **A forced takeover's evidence hashed differently on the two sides when a field was absent.** The
  digest that binds a forced reclaim to its reason is built from a JSON list, and the transport
  builds it out of a dictionary — so a marker carrying no `runtime`, `horizon` or `from` puts `null`
  in that list. The port took those three as strings, so an absent one arrived as `""`. Measured on
  the pair: `bcb711ad…` there and `4e09f435…` here, for the same event. Whichever side did not write
  the marker then reads the evidence as unbound and refuses a takeover the other performed. The
  crossing had existed all along and could not see it — its corpus posed no absence, and it adapted
  each case with `unwrap_or_default()`, which is the difference itself written into the test.
  Absence is `Option` on this side now, the corpus poses all four shapes, and the empty string is
  posed beside them so a fix that folded the two together would fail instead of agreeing.

- **The runtime is still folded into a forced takeover's evidence digest, and the retry that would
  read it cannot be reached.** The runtime is attribution and the run id is
  identity, and four tuple comparisons were removed; `forced_reclaim_hash`
  ([`src/transport/ownership.rs`](../src/transport/ownership.rs)) was missed because its runtime
  argument is not spelled as a comparison — the neighbouring entry is that digest across the language
  boundary. On this side it decides nothing. The fold that says whether a forced reclaim stands
  recomputes the digest from the marker's **own** `runtime` attribute (`ownership.rs`), so
  a forced marker always binds to itself whatever runtime the retrying build derives from its run id;
  `authoritative_events` keeps it, and `reduce_ownership` makes its author the holder and closes the
  epoch before it (`ownership.rs`). The caller's runtime enters the digest in exactly one
  place — the retry recompute in `reclaim` (`claim.rs`), compared against the persisted hash in
  `resumed_reclaim` (`claim.rs`) — and that comparison is reached only after
  `plan_takeover` answers `Takeover::Write` (`claim.rs`), which an accepted marker
  prevents: its author holds, so `plan_takeover` answers `already-yours` (`claim.rs`)
  first. Measured, not supposed: a forced-reclaim marker written with `runtime=claude` under a run id
  deriving `codex`, retried with the same operation id, horizon and target, was answered
  `already-yours`, never `reclaim-metadata-mismatch`. The comparison is reachable only through a
  marker the fold discards for some other reason whose `from-op` still names the current holder —
  pose the same marker with no evidence body, and the retry does reach `reclaim-metadata-mismatch` —
  but no writer produces that marker, because `reclaim` writes `from` and `from-op` from the same
  target and always writes the evidence it hashes. It stays rather than being dropped because the
  digest is the wire format: a forced marker already on a timeline hashed its evidence with `runtime`
  in the list, and recomputing without it would read every one of them as `EvidenceUnproven` and
  change who held an issue retroactively. The population is every forced takeover written before that,
  which recorded `runtime=claude` under any run id. What it costs is the latent retry mismatch this
  entry names, and nothing reaches it.

- **The port stopped a `start-branch` the transport performs.** A worktree template missing a scope
  is migrated to one that has it, and both sides then refuse when the *pre-migration* checkout is
  still registered — it may hold unpushed work, so neither removes it. (At the time this was
  measured, only a branch-only template was migrated.) The transport asks
  `legacy.exists()` before it consults the registry and the port asked the registry alone, so they
  parted on the most ordinary state a worktree reaches: a directory removed with `rm -rf` instead of
  `git worktree remove`. Measured on a real repository, git goes on listing it — `prunable gitdir
  file points to non-existent location` — and neither registry reader filters that out. What the
  refusal protects is work in a directory, and its own way out says so; a registration whose
  directory is gone sends an operator to rescue a tree that is not there. The rule has a name now,
  `legacy_worktree_block`, so it can be measured without a tracker and a remote standing by.

- **The gate waited on the transport with no bound at all.** `Command::output` returns when the
  child does, and the transport bounds only *some* of its own `gh` calls — `DEVELOP_TIMEOUT_SECONDS`
  is the one it names — so a call left without a timeout held the gate open for as long as the
  socket stayed open. The measurement that found it never finished: a `cargo test` against a
  transport that sleeps was still running ten minutes later. A harness that hangs is worse than one
  that refuses, because the run has no verdict and no way to ask for one, and an operator whose
  harness hangs switches it off. The wait is now bounded at fifteen minutes — far past anything a
  working transport reaches, since its own longest documented wait is two — and what comes back is
  `Unknown`, never "nothing happened": the command may have posted a comment or opened a pull
  request before it stopped. For the same reason the **next interpreter is not tried**, which is the
  half a bound alone would not have fixed.

- **The two readers of the same prose folded different alphabets, and the population is now
  counted.** `re.IGNORECASE` matches `ſ` against `s`, and `ı` and `İ` against `i` — the last two are
  Unicode's Turkic-only mappings, applied unconditionally — while the port lowercases and compares
  ASCII-wise, which folds none of the three. Sweeping U+0020–U+30000 for characters the two rules
  disagree about returns exactly those, and they land in the words these matchers are made of: `you
  loſt` and `backıng off` were a stand-down the transport reads and the gate cannot see, which is
  what leaves a displaced run writing; `Claımed by <run>` was a held issue on one side and a free
  one on the other, which is rule one failing at the parse step. `### verſion 6.9.8` was an entry on
  one side and `no-changelog-entry` on the other, and where a file holds both spellings the
  transport refuses as ambiguous while the port picks one silently — the outcome that function's
  contract forbids, because the tag it feeds is immutable. `closing_hits` had met this same
  character and narrowed itself for it; the three readers beside it were left as they were, which is
  what a fault costs when nothing counts the population. All four are now narrowed on the transport
  side, crossed against their port, and
  `every_case_insensitive_pattern_of_the_transport_is_answered_for` asks the module — not the file —
  for the list, so a fifth cannot be added without an answer.

- **`setup --all` keeps each agent's own configuration.** It used to read the first one it found and
  install that everywhere, so two agents configured differently on purpose ended the run agreeing —
  the second rewritten to the first one's table, unasked and unmentioned. The configuration is now
  read from the skill root it is about to be written to. A root with no contract has no answer of
  its own and inherits **per row**: an `Everywhere`- or `Machine`-scoped setting every installed
  contract agrees on is folded onto the portable defaults, and a row they disagree on — or every row,
  when nothing else is installed at all — stays at the default, and the run says which rows those
  are. `Agent`-scoped rows (judges, delegated models, who plans) are never inherited from a
  neighbour, whole-table agreement or not, because they are meant to differ per agent — an operator
  who wanted otherwise still has to say so with `estigia config set`.

  A second, narrower defect survived this fix: `estigia setup <slug>` for one named
  agent built its shared population from that single adapter, so a fresh root's only neighbour was
  never read and the run wrote `Config::default()` with nothing said. The population read for
  inheritance is now every *installed* adapter unioned with the run's own targets, and
  whole-`Config` equality — which read two agents disagreeing on nothing but their `Judges` row as
  two entirely unrelated tables — no longer decides what a fresh root inherits; the per-row fold
  above does.

  A blind panel found a third defect in the same population before either of the above reached
  review: it was reading each root's `unlayered()` value, which prefers `estigia.local.md` — read
  with no scope filter, unlike the per-agent file — so an operator's own machine-local override
  could be folded into a **different** adapter's freshly written, versioned, and shared contract.
  This is the same promotion `contract_config`'s own doc comment (`src/skill.rs`) already rules
  out for an adapter's own write — *"the machine-local choice had become the team's"* — and the
  population now reads `skill::contract_config` instead, the same value `existing_config` was
  already using, so it cannot happen through this path either.

  A residual survives even this fix, and neither judge who found it ran anything to see it — it
  was found by **reading**, in review, and no run has reached the machine shape it describes.
  `doctor`'s `canonical` row (`src/harness/doctor.rs`) compares two **layered** views —
  `installed_config`, what the gate decides by, against each adapter's own `installed_config_for`,
  what it reads — faulting on every row whose `scope()` is not `Agent`. Inheritance now writes the
  **contract** value instead of that layered one, which is the repair the panel above required: a
  local override must not be promoted into a *different* adapter's versioned block. So on a
  machine whose `Everywhere`- or `Machine`-scoped answers live in a per-root `estigia.local.md`
  rather than in the contracts themselves, a newly registered adapter's contract still will not
  match `canonical`'s layered comparison, and `canonical` reports it `BROKEN`.

  Not a regression: before this fix, that same fresh root got `Config::default()`, which diverges
  from a layered override just the same — such a machine was already `BROKEN` on `canonical`, with
  or without a new adapter. Narrow, for the same reason: a machine answering shared rows through
  per-root overrides is `BROKEN` before any adapter is added, and `estigia config set` without
  `--agent` writes every installed **contract** directly — the mechanism that cleared this exact
  row on the machine that reported it, whose three-row `deepseek-harness` divergence was
  closed that way rather than by a per-root override. Closing this residual is not this fix's to
  make: it belongs to `canonical`'s own comparison, or to `config set`, neither of which this
  change touches.

## Reachability, registration and the rounds around them

- **Moving a comment into a document buys reachability, not a crossing, and proximity is the price.**
  [`AGENTS.md`](../AGENTS.md) item 2 takes the incidents, measurements and arguments out of `src/`
  and into `docs/`, in tranches. The first tranche has now moved: the first sweep took
  `src/harness/mcp/` from 22.5% of its non-blank lines to 8.0%, and the `rmcp` narrative
  one of those comments carried now lives here, above
  at "The cost avoided by hand-writing this server instead of using `rmcp`". What later tranches do
  to the rest of `src/` and `tests/` is not promised here. Two properties are worth naming before
  they change.

  **A document is not automatically checked.** This file's own opening says which of its claims are:
  the countable ones, crossed by `tests/honesty.rs`. A relocated `//` comment is prose held by
  review, exactly as it was in the source — what it gains is a reader who can find it without opening
  the function, and what the repository gains is one surface to check rather than two.

  A `///` gives up more than that. `src/lib.rs` denies `rustdoc::broken_intra_doc_links`, so a
  bracketed item name inside a doc comment is a claim `cargo doc` refuses to build once it stops
  being true — the comment there records nine that had accumulated, one of them naming a type no
  longer present. The same name in a document is ordinary text: the link test in `tests/honesty.rs`
  resolves file paths and knows nothing about crate items.

  **A document cannot sit beside the line.** A reader who opens a function sees the code and has to
  know a document exists, and **nothing here enforces that they look**. Neither does the suite tell a
  finding that was relocated from one that was made unreachable: a scanner that strips comments
  cannot see either, and a scanner that reads them objects only when what it read is what moved.

  **No list of them is kept, and that is deliberate.** An earlier draft of item 2 kept one and had
  already missed a reader; one class cannot be listed at all, since the offset pins in
  `src/transport/claim/tests.rs` read a comment without ever mentioning it. [`AGENTS.md`](../AGENTS.md)
  item 2 therefore asks for a test run instead of a list, and the cost of that belongs here: the run
  is complete where a list is not, and it is still not a review. A test holding a floor stays green
  while the population under it shrinks.

- **The integration exemption proves *a* branch is checked out where the merge runs, not that it is the
  issue's own branch.** `is_integration_in_the_runs_own_checkout` asks three things: does `run.worktree`
  **alone** — not `Run::covered()`, which chains `repo_dir` with `worktree` — cover the checkout; does the
  environment leave Git free to find that checkout's own repository rather than a redirected one; and does
  `git symbolic-ref` name any `refs/heads/*` ref. Reading `Run::covered()` instead, the way a reader who
  "simplifies" the first sentence would, extends the exemption to the base checkout as well — the exact
  case this proof exists to exclude, since a merge run there advances trunk with no verdict. `start_branch`'s own
  answer carries `"branch"` beside `"worktree"`, but `harness/mcp/mod.rs` reads only the second back
  into the run pointer — the branch itself lives nowhere the gate can reach: `ownership_path` in
  `src/transport/branch.rs` writes it into `<repo>/.git/worktrees/<name>/issue-flow-owner.json`, the
  checkout's private git admin directory, and that file is private to `transport::branch`.

  Worse, the gate never **observes** the branch changing at all. `git checkout <branch>` and `git switch
  <branch>` are deliberately outside `REPOSITORY_SHELL` — switching branches discards nothing, which is
  why the bare forms are left out on purpose (`src/harness/mod.rs`) — and nothing else in the
  harness watches `HEAD` move.

  Git narrows the window on its own, and this was measured rather than assumed: `git checkout main`
  inside a linked worktree answers `fatal: 'main' is already used by worktree at '<path>'` for as long as
  `main` is checked out anywhere else. But that refusal only covers a branch checked out **in another
  worktree** — it says nothing about one that is not checked out anywhere. So the opening this leaves is
  not "any checkout, any branch": it is either a run checking out trunk while the base checkout has moved
  off it — detached mid-delivery, or switched to something else by hand — or, with no precondition at all,
  a branch the base checkout has never had checked out to begin with, shared or not: `develop`,
  `release/1.x`, a second feature branch nobody else is working on.

  It is unclosed rather than uncloseable. The closing is one more field read at the same call site —
  `.get("branch")` beside the `.get("worktree")` already at `harness/mcp/mod.rs` — with the same
  reach limit `src/harness/session.rs` already records for `reviewed_head`: a run that shells out
  to the transport directly, rather than through Estigia's own `start_branch` tool, never reaches the
  writer that would record either one.

  `is_integration_in_the_runs_own_checkout`'s own environment check is guarded against the environment
  redirecting the repository, drawn from that one class rather than proved to cover it.
  `integration_environment_does_not_redirect_the_repository` refuses `GIT_DIR`, `GIT_WORK_TREE`,
  `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_INDEX_FILE`,
  `GIT_NAMESPACE`, `GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM`, `GIT_CONFIG`,
  `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_COUNT`,
  `GIT_CONFIG_PARAMETERS`, `GIT_SHALLOW_FILE`, `GIT_REPLACE_REF_BASE`, `BASH_ENV`, `ENV` and
  `BASH_FUNC_git%%` by exact name, case-insensitively, plus any name starting `GIT_CONFIG_KEY_` or
  `GIT_CONFIG_VALUE_` — the numbered pair a `GIT_CONFIG_COUNT` run of `git -c` values is spelled
  with, which carries no fixed suffix a list could name. This is a list, not a proof that the class
  is exhausted: a name absent from it is permitted rather than shown harmless. `GIT_EDITOR`,
  `GIT_PAGER` and `GIT_ASKPASS` are permitted, because an editor cannot change which branch a merge
  moves; the `GIT_CONFIG` family is refused because `core.worktree` redirects where the merge writes
  and `core.hooksPath` disables the hooks that run with it, and `proof_git_command` strips every
  `GIT_*` from the proof child, so this check is the only place that sees them at all. What this check
  cannot prove either way is that the merge it permits, which runs after this process exits, reaches
  the same repository this process just read; it can only prove the environment in front of it is not
  one of the names known to move that.

  The `--ff-only` exemption beside it, `is_safe_local_fast_forward`, uses this same list rather than a
  `GIT_` prefix. It used to refuse on every `GIT_*` prefix, so on a host that sets `GIT_EDITOR` —
  this checkout's own host does — the narrower command, `git merge --ff-only <target>`, was refused
  while the broader one, a merge-commit integration in the run's own checkout, was allowed: an
  inversion, recorded here until this change removed it. The two exemptions prove different things —
  ancestry against `@{upstream}` versus cwd-containment and `HEAD`-on-a-branch — but the variables
  that invalidate one invalidate the other, because both read the repository a redirect would move;
  and the proof child has every `GIT_*` stripped either way, so the guard's only job is the shell that
  runs after this process exits. `is_git_environment`'s prefix check survives only for
  `proof_git_command_with_environment`, which strips every `GIT_*` from the child.

  The tests no longer hide it. `local_fast_forward_proof_fails_closed_on_repository_state` drives
  `is_safe_local_fast_forward_in` with an environment holding each program-naming name (permitted) and
  each redirecting name, upper and lower case (refused); and
  `an_exact_local_fast_forward_is_allowed_without_widening_other_merges` puts `GIT_EDITOR=true` back
  into the child after `tracker_command`'s `GIT_*` strip and asserts the literal merge is still
  allowed, so the suite now fails on a host the exemption would refuse. The earlier scrub that this
  replaced is the one recorded earlier: the library test read the process environment and the
  `tests/pipe.rs` child inherited it, so both were made independent of the host by removing what the
  defect needed to be seen.

  **An integrating merge spends no verdict, and it is read in the directory it names.**
  `stale_verdict` used to refuse every `git merge` whose checkout head differed from the recorded
  receipt, so on a second review round the integration the contract prescribes was refused
  `verdict-bound-to-other-bytes` and the run fell back to a rebase, which rewrites history. It also
  compared the *session* directory's head, so `cd <worktree> && git merge origin/main` was judged
  against a checkout the command did not run in. Now:
  - **Shapes that count as an integration:** exactly `git merge [--no-edit|--no-ff|--ff|--quiet|--continue|--abort] [<ref>]`,
    spelled alone, behind one leading `cd <dir> &&`, or as `git -C <dir> merge ...`. `<dir>` is a
    plain path (letters, digits and `/._-~+@`, optionally in one pair of quotes, not starting with
    `-` or `~`, or a drive-lettered `X:/` path, which a Windows checkout is named by); a relative one is taken from the session directory. Everything else about the line
    must be literal, by the same reader `exact_integration_merge` applies to the in-progress case.
  - **When it is allowed:** the directory it names is inside `run.worktree`, the environment does not
    redirect Git, and `HEAD` there is on a branch (`is_integration_in_the_runs_own_checkout`, the
    same proof the in-progress exemption uses; there is one classification, not two). The next
    publication invalidates the verdict anyway.
  - **Still a delivery:** the same line in any other checkout, including the base checkout; a
    `git merge` with `--ff-only`, a second command, a redirect or an expansion; `git tag`;
    `gh pr merge`, whose adjudication is unchanged. `git tag` and an exact `git merge --ff-only`
    have one further road, below.
  - **Directory not readable:** a line that changes directory any way other than the leading `cd`
    or `-C` above (`cd $VAR`, a second `cd`, a subshell, `pushd`, `--git-dir`) is refused
    `review-checkout-unreadable` (class `unavailable`; it used to be documented here as
    `verdict-bound-to-other-bytes`, which the code never answered), says the gate cannot tell which
    checkout it runs in, and does not name the session directory. This applies only after a review is
    recorded, and since #58 to every git command in `DELIVERS`, `git tag` included: the directory
    change is read for each of them, where it had been read for `git merge` alone and a `git -C
    <dir> tag` was judged against the session's checkout.
  - **The run's own merge commit (#58).** The prescribed `git tag -a "<tag>" -F "<notes-file>"
    "<merge-sha>"` and the `git merge --ff-only origin/main` that brings the base checkout to the
    delivered merge both act on a commit that is not the reviewed head, so the head comparison
    refused them. The commit the line acts on — a tag's named commit, else the checkout's `HEAD`;
    the fast-forward's named target, in addition to `HEAD` — is now also admitted when it is a
    two-parent merge whose second parent is the receipt's head and whose first parent descends from
    the receipt's base. That is the topology `match_merge` applies to the merge readback, held
    in one place (`ReviewTransaction::delivered_by_merge`) that both read. A tag line is read only
    when it is plain words (`tag_target`); a quoted space, shell punctuation, a flag outside the few
    that create a tag or a third positional leaves the commit unread and the comparison with the
    checkout's `HEAD` as before. A tag that names a commit is judged on that commit, not on `HEAD`:
    where the checkout sat at the reviewed head, a tag of some other commit used to pass. Not
    covered: `git -C <dir> merge --ff-only ...` and `cd <dir> && git merge --ff-only ...` (not the
    bare four-word shape `exact_local_fast_forward_target` reads), a merge whose base the checkout
    cannot resolve (the ancestry question is then unanswered, which refuses), and a squash or
    rebase merge, which has no second parent.
  What this does not prove: it does not check that the branch is the issue's own, nor that it is not
  the base branch, only that the checkout is the run's own worktree; that is the first entry above and
  it stands. A run whose worktree is itself on the base branch is not told apart.

  **A near-miss integration names the shape.** A `git merge` in the run's own checkout whose
  shape is not the one `exact_integration_merge` proves — a pipe, a redirect, a second command, a
  second target, a quoted ref, or a directory the line moves to in a way the reader cannot name — is
  refused either way. It used to fall through to `out-of-phase` in `in-progress` when the checkout
  head matched the recorded receipt, or to `verdict-bound-to-other-bytes` when it did not, and
  neither said the shape was the problem: a run measured on 2026-10-06 read both as "the merge is
  forbidden" and rebased four times, paying a whole-change re-review each time. In `in-progress` it
  now refuses `integration-shape-not-exact`, whose message names the one accepted literal,
  `git -C <worktree> merge --no-edit origin/<base>`, and says nothing may ride beside it. The
  refusal is a refusal and never an allow: the exemption's own proof is still the only thing that
  admits a merge, and this arm changes only which reason the run reads. It fires only in
  `in-progress`, so `out_of_phase`'s own answer for `analysis`, `ready` and `blocked` is unchanged,
  and a merge in `review` or `done` is still a delivery. It does not close the gap the paragraph
  above states: it still does not check the branch the merge is aimed at.

  **The exemption's reach stops at a JSON string, and the deadlock is unchanged past that point.** `shell_command_argument` sets `literal` only when exactly one key
  normalises to a command and that key's value is a JSON string; an argv array donates a `text` for
  the ordinary classifier to read but no `literal`, and `is_exact_integration_merge` is `false`
  whenever `literal` is `None`. So for any shell tool whose adapter sends `command` as an argv array
  rather than a string, `git merge origin/main` run from `in-progress` is still refused
  `out-of-phase` — the exact deadlock this issue exists to close, left standing for that shape. This
  is correct and fail-closed, not a defect: the alternative would be trusting a literal the gate
  never actually read. Which agents' adapters send arrays and which send strings was not measured
  here and is not claimed; only the mechanism and its consequence are.

- **Nothing counts the rounds a change takes, and nothing bounds one that is not converging.** The
  two-round limit is gone: `skill/policies/blind-judges.md` carries no count, and no path reaches
  `blocked` because one ran out. What stands in its place is worth stating exactly, because it is less
  than it looks.

  **The judgement triggers are prose a run applies to itself.** The put-down row of `skill/references/decision-gates.md` — a known
  mistake repeated, three hypotheses without narrowing, more editing than measuring — is read by the
  run that would have to admit one of the three about its own work, and nothing here checks any of
  them. They were the better half of the rule that went, and they are unenforced.

  **The one mechanical bound answers a different question.** A claim carries a horizon, and past it
  plus the renewal window another run may reclaim. That bounds a run which has stopped reporting, not
  one reporting diligently on its ninth publication. A change can converge slowly, or not at all, for
  as long as its holder keeps renewing.

  So what stops an unconverging change is a person noticing. The count did bring one in — at the third
  round, whether or not the change was converging, which is why it went — and what replaces it is
  nothing. This entry exists so that is written down rather than assumed.

- **ZCode registers its instruction file and its MCP server; three things remain unverified.** The
  twelfth adapter installs the shared skill and fences the directive into `~/.zcode/AGENTS.md`, taken
  from ZCode's own shipped configuration reference: the user-scope instruction file, loaded into every
  session before any workspace `AGENTS.md`. **The MCP registration was the second thing withheld, and the reason for it was crossed against a running client rather than a page**: on the machine
  that filed it, `~/.zcode/cli/config.json` declared one stdio server, Leteo, under the nested
  `mcp.servers` key, and ZCode CLI 0.16.5's own log recorded `mcp.server.connected` for it — with no
  `~/.agents/mcp.json` and no project-scope `.zcode/` anywhere that connection could instead have come
  from. That is the key `render_mcp` and `strip_mcp` now read and write. What is still unread is the
  per-scope fallback to `~/.agents/mcp.json`, which fires only when that file holds none of its own;
  this adapter does not need it to register and has not verified it, so a machine relying on that
  fallback alone remains unmeasured ground.

  **If that unverified mechanism is as described, this adapter has a cost it does not name
  elsewhere.** `render_mcp` writes `~/.zcode/cli/config.json` unconditionally and creates it from
  nothing when the file is absent. An operator whose ZCode MCP servers all arrive through the
  fallback — which, on the stated mechanism, only happens when their scope file's own `mcp.servers`
  is empty — would have that file gain exactly one server, Estigia's, the moment `estigia setup
  zcode` runs; by the same stated mechanism every server they previously reached through
  `~/.agents/mcp.json` would then stop loading, while `setup` reports success and `status` reads
  `tools on` because `exposes_tools` reads back Estigia's own write. This is the fallback's
  consequence *if the fallback works as described*, not a measured one: the filing machine had no
  `~/.agents/mcp.json` at all (above), and a search of its `~/.zcode/cli/log/*.jsonl` for any mention
  of `.agents/mcp.json` returned nothing — consistent with the file's absence, and no more than that.

  **This registration also widened the population of an earlier fix, unrecorded until now.** Before it,
  `paths_in` answered `None` for ZCode's `mcp_config`, so `uninstall` never opened
  `~/.zcode/cli/config.json`; now it does. `is_now_empty` (`src/setup/mod.rs`) answers true
  for any document whose every top-level value is an empty container or null, and `discard`
  then removes the file. **That fix narrowed this at the MCP step: a file the uninstall
  changed nothing in is no longer written or discarded**, so an operator's `{"mcp": {}}` survives the
  uninstall byte for byte. What remains is the worked case, where the uninstall did change the file:
  `{"mcp": {"servers": {"estigia": …}}, "hooks": {}}` has `strip_zcode_mcp`
  remove `mcp` entirely, leaving `{"hooks": {}}`, which `is_now_empty` reads as true, so
  `estigia uninstall zcode` deletes the operator's whole ZCode CLI config rather than only the MCP
  section this adapter writes. What that costs is bounded by the same rule: a config carrying a real
  `hooks.enabled` has a non-empty top-level object, so `is_now_empty` answers false and the file
  survives. Only a file whose every remaining top-level value is an empty container or null is taken,
  which is why the worked case has to spell `"hooks": {}`. That population
  was general-CLI-configuration-file cases already; this entry just adds one, and the fix there left
  `is_now_empty` and `discard` alone.

  Two things that reference also names were deliberately **not** taken, each for the reason an
  unverified entry is worse than a missing one:

  - **No skill root of its own.** The reference scans `~/.zcode/skills` beside `~/.agents/skills`, but
    it names them on a page rather than in ZCode's own shipping source, which is the bar a root of its
    own is held to. ZCode sits in the neutral tier — the directive names the path — exactly as
    OpenCode, Cursor and the rest do. **And `~/.zcode/skills` is not gated.** A reviewer found it: a
    host-read skills root is loaded name-and-description-first, which is the authority
    `skills_root_fragment` exists to gate, but that fragment is derived from the `skills` field and
    ZCode's is the neutral one. Admitting the path to `CONTROL_SURFACE` as a literal was proposed and
    refused, because the declaration there carves the skills trees out on purpose and widening a rule
    to fit an entry is what its affirmation exists to catch. So a run holding a claim can drop a skill
    into `~/.zcode/skills` unmeasured, and the same write against Claude Code, Codex or the neutral
    root is a `Boundary`. What closes it is ZCode's skill root being read out of its own shipping
    source, which promotes the adapter and derives the fragment with it.
  - **No tool-call gate.** The reference documents a `PreToolUse` event among its seven, so there is
    clearly something here — but configuration-file hooks decide nothing until `hooks.enabled` is set
    true, a flag that governs the operator's other hooks in the same file, and neither the payload the
    host sends nor whether (and how) a hook can deny has been crossed against anything. Writing an
    entry under those conditions would be installed-looking and inert, which is the failure this whole
    crate exists to refuse. If ZCode's hook shape is ever verified, promotion follows the road
    OpenCode's plugin took. What an operator reads in `status` says the same thing rather than the
    generic line about no reference having been read: `gate_gap` carries its own arm.

  Almost everything the adapter participates in is derived or crossed:
  `adapters_on_the_neutral_root()` and the two README counts `tests/honesty.rs` keys on are crossed
  against the table; the gate fragment `.zcode/agents` is spelled by hand beside the path it shadows
  and crossed against it by `every_control_file_an_adapter_has_is_one_the_gate_measures`, and the
  spelled list in `src/harness/tests.rs` is crossed from both ends. The `~/.zcode/cli/config.json` entry
  is held by behaviour as well: `every_control_file_an_adapter_has_is_one_the_gate_measures` watches
  `paths.mcp_config`, which resolves to that file, so deleting the entry reddens it naming the path.
  The neutral `~/.agents/mcp.json` fallback resolves to no adapter's path, so it is held instead by
  `a_host_read_root_carries_the_same_authority_as_the_directive` in `src/harness/tests.rs`.

- **DeepSeek Harness registers, with its skill root and instruction target verified.** The thirteenth
  adapter when it was added (the seventh since six were retired) installs the skill into `~/.dsh/skills` — the harness's own skill root that the
  `skill-filesystem` provider scans as its `user-dsh` root — and fences the
  directive into `~/.dsh/AGENTS.md`, the harness's fixed user-global instruction file that every
  `agent-instructions` baseline starts from. Both come from the harness's own shipping source rather
  than a page, so this is the third adapter (after Claude Code and Codex) with a skill root of its
  own, discovered without the directive naming a path. `$DSH_HOME`, which the harness resolves both
  roots under, is read and contained the way `XDG_CONFIG_HOME` is: an absolute value under the same
  call's home is honoured, one outside it is dropped in favour of `~/.dsh` and reported as the
  per-adapter note. Three things were deliberately **not** taken:

  - **No MCP registration.** DeepSeek Harness runs its own MCP-client machinery, and where an operator
    registers a server for it has not been crossed against a running client. A server entry in the
    wrong place never starts under a `status` line saying it did — which is why `paths_in` answers
    `None` here, as it was for the since-retired Windsurf and Cline.
  - **No tool-call gate.** The harness's plugin API exposes an execution hook, but whether a plugin can
    **deny** a tool call the way OpenCode's `tool.execute.before` does — and under what payload shape —
    has not been crossed against a shipped harness. Writing a gate Estigia has not verified against
    this agent would be installed-looking and inert, the failure this crate exists to refuse. So the
    harness is contract-only: the directive asks, and nothing checks that the agent honoured it.
  - **No role/sub-agent definitions.** The harness's own agent definitions live under `~/.dsh/agents`,
    but the schema has not been verified, so `agents_root` stays `None` rather than a guess. The tree
    is gated anyway, and by a **prefix** rather than by a decision: the extension-trimmed fragment
    `.dsh/agents` reaches `~/.dsh/agents/**` only because it is right-unanchored, and narrowing it to
    the file `.dsh/agents.md` — which reads as a tightening — would drop the whole tree to `Routine`.
    What pins the prefix is the row for `~/.dsh/agents/reviewer.md` in
    `a_host_read_root_carries_the_same_authority_as_the_directive`: it sits under the directory the
    fragment names rather than on the file the narrowing would spell, so the narrowing reddens there.

  Everything the adapter participates in is derived or crossed: the two README counts
  `tests/honesty.rs` keys on are crossed against the table; the gate fragments `.dsh/agents` and
  `.dsh/skills/` are spelled by hand and crossed against the paths they shadow by
  `every_control_file_an_adapter_has_is_one_the_gate_measures`,
  and the spelled list in `src/harness/tests.rs` is crossed from both ends.

- **Nothing crosses the transport's documented refusals against the ones it constructs.**
  `src/harness/tracker.rs` used to hold `a_refusal_the_transport_documents_is_one_the_harness_can_tell_apart`,
  which scraped every `/// - **`reason`** — ...` bullet out of `src/transport`'s own doc comments and
  checked that each one survived `stable_code` unchanged — so a reason written in prose that the
  transport's code never actually constructs would have been caught reaching a caller as
  `transport-refused` regardless. It was removed: reading comment text to decide what a test
  measures is exactly what the ruling refuses, at every level, without exception for a taxonomy
  crossing. What replaced it, `the_generated_vocabulary_excludes_the_fallback`, measures one thing:
  that `transport-refused` is not itself an entry in `TRANSPORT_VOCABULARY`. It shipped with two
  further assertions, and neither survived. A round-trip through `stable_code` was cut before this
  test's first merge — a blind judge found it could not fail, since `stable_code` *is* a lookup into
  that same vocabulary. A floor on `TRANSPORT_VOCABULARY.len()` shipped and stood until a
  follow-up round made it dead: `include!` pulls this file's `TRANSPORT_VOCABULARY` in from exactly
  the generated file `build.rs`'s own floor already walked over, so the floor here can no longer
  observe a count `build.rs` has not already cleared — but only because `build.rs`'s own floor was
  raised, in this same round, to sit at or above the one removed here. That was not true of the two
  floors' history: at this branch's merge base `build.rs`'s floor was `40` and the assert removed here
  was `90`, so a generated count anywhere from 40 to 89 would have cleared the build and still been
  caught by this one. The floor was not dead by construction; this round is what made it so. A judge
  re-derived independently that no guard was lost at this head, and none was — the removal is right,
  the reasoning that used to justify it here was not. Both floors are gone rather than kept as
  decoration.

  The crossing that survives is a different one, and it is real: `the_vocabulary_covers_what_the_transport_actually_answers`
  and `every_reason_the_port_answers_with_is_a_code_and_not_a_catch_all` read the shipped transport
  source, take every literal `"reason": "…"` in it, and check each resolves to a vocabulary entry
  rather than to the catch-all. What is no longer compared against anything is a refusal *documented*
  in a doc comment and never actually returned by any code path, so that specific drift — prose
  promising a distinction the code does not make — can reopen silently. Nothing found one at the time
  this was written; nothing is watching for the next one either.

- **A workspace install covers the runtimes whose project-level files could be read from the
  runtime's own reference, and refuses the rest.** `setup --scope=workspace` writes each adapted
  runtime's files into a checkout instead of the machine's home; the destinations it writes, and the
  runtime's own page each was read from, are in
  [`what-it-writes.md`](what-it-writes.md). ZCode and DeepSeek Harness have no verified project-level
  destination, so a workspace install refuses them by name rather than writing a path they may not
  read — a path a runtime does not read reports success and configures nothing. A workspace
  *uninstall* of such an adapter, and of the retired ones, is a no-op instead: it never wrote there,
  so `uninstall --all --scope=workspace` names nothing it could not place. The write-side refusal of
  an unanswered workspace and of a destination-less adapter is checked by
  `a_workspace_install_needs_a_workspace_and_a_verified_adapter`; the three CLI refusals, for a
  workspace named under the global scope, for the workspace scope with no path and for a companion
  beside a workspace, and the verb each names, by `a_workspace_refusal_names_the_command_that_clears_it`,
  `the_setup_uninstall_arm_names_uninstall_in_its_way_out` and
  `the_companion_path_refuses_a_scope_that_names_nothing_too`; the writable adapters and their reversal
  by `a_workspace_install_writes_under_the_workspace_and_nowhere_else` and
  `a_workspace_uninstall_takes_the_skill_back_from_one_shared_root`. What is **not** covered: that
  each runtime actually reads every path in the table was read from published documentation, not
  crossed against a running host, so the table is a reading rather than a measurement — and one cell
  is weaker still: Codex's project `hooks.json`, read from a community configuration and Codex's own
  pages, is reported by an open upstream issue as not loaded when Codex runs where `.git` is
  a file, a git worktree, so a workspace install against one may write a gate Codex never reads.
  Nothing here crosses that against a host. Cursor's project `hooks.json` has a stricter schema than
  the user-level file the global scope writes — a required top-level `version` — read from a community
  issue and Cursor's own pages, and neither the generated body nor a running Cursor was read, so
  whether the workspace hook loads is untested. Both project-level gates also load only in a project
  the runtime trusts, unlike the user-level files, which is a difference from the global install that
  nothing reports. And in a workspace the runtimes that read `AGENTS.md`
  share one instruction file and one skill root, `.agents/skills` — the workspace root for every
  runtime that reads that file — so the shared directive and the shared skill go with the **last**
  adapter the record names, and an uninstall of one the record does not name is a no-op. A recorded
  adapter's own removal therefore takes the shared directive and skill only when it is the last of
  them —
  deliberate and stated in `what-it-writes.md` — but the block names whichever adapter wrote it last,
  so the per-agent override file it points at belongs to a runtime that may no longer be installed.
  The test that reads the shared block asserts the skill directory it names; the override file it
  names is what nothing asserts. The guards rest on one invariant — every runtime that shares
  `AGENTS.md` shares its skill root too — and `the_runtimes_that_share_an_instruction_file_share_its_skill_root`
  is what holds it: `shared_instructions_survive` keeps the directive on a sibling that reads the
  file, while the skill goes on a sibling that shares the root, so a runtime with one and not the
  other would leave a directive naming a directory that was taken. A record is found by the digest of
  the checkout's own canonical path, so a checkout that was moved or renamed is one no record names,
  and a fresh clone of one that carries an install in its files carries no record at all — the record
  is machine-side. Uninstalling either removes the adapter's own files and leaves the shared directive
  and skill, reporting only what it did. And `is_configured`, which reads the record, answers "not
  installed" for every adapter when the record cannot be read — the one reader that turns this unknown
  into an absence, though a gate or a plugin of the adapter's own still makes `is_present` true, so
  `sync` runs far enough to fail at the record write rather than refuse. The record is read, changed
  and written without a lock, so two runs against the same checkout at once can lose one adapter's
  name: the one dropped stops being reversible, and the one kept keeps the machine's state standing.
  A record is written before the adapter's files, so a `setup` that fails after that point — the
  adapter's own write refused, say — leaves the adapter named until it is re-run or uninstalled, and
  `is_configured` and `forget_state` believe the record in the meantime. The record that decides all of
  this is read through `workspace_installed_adapters`, which answers `None` — the unknown that keeps
  the shared surface — for a record it cannot read or parse, while `any_workspace_install` treats the
  same record as the unknown that keeps machine state, and an install that would merge into such a
  record refuses rather than overwriting the adapters it named — as a workspace uninstall now refuses
  before it removes anything, instead of half-reversing the checkout and failing at the record write.
  A record whose checkout is
  `NotFound` — removed, or on a volume that is not mounted — reads as gone rather than unknown, so a
  machine-wide `uninstall --all` can take the store while an install on a detached volume still reads
  it.
  The user-scope version of that collapse is closed where it was reported: `is_configured` still
  answers `false` for an instruction file it could not read, which `setup`'s `to_touch` planning
  wants, but the TUI uninstall now reads `read_configured`, which keeps `Unknown` apart from `Absent`.
  An uninstall whose read-back could not read the instruction file is a refusal that names the read
  failure (code `uninstall-not-read-back`, outcome unknown), never "read back as not installed", and
  the agent stays in `installed_agents`; the startup agent list keeps an `Unknown` agent listed rather
  than presenting it as not installed. What remains: every other caller of `is_configured` — the CLI
  `uninstall` output, `sync` and the status listings — still turns an unreadable file into "not
  configured", and in the workspace scope `read_configured` still takes an unreadable record as absent,
  as `is_configured` does.
  `setup --all --scope=workspace` was observed once to stop with a refusal naming the two adapters it
  cannot place, after the five writable ones were done; no test drives that CLI path, so the ordering
  and the count are a reading rather than a measurement.

- **A global install is found through a record of its roots, and only installs made after the record
  existed are in it.** `forget_state` keeps the store while any recorded root, under
  `<home>/.estigia/roots`, still holds an adapter, and keeps it when a record cannot be read. An
  install made by a build that did not write the record, under a `CLAUDE_CONFIG_DIR` other than the
  one the uninstall runs under, is invisible until `setup` is run again under that root. The record
  holds the Claude, XDG and `APPDATA` roots; `CODEX_HOME` and any other root `Environment::resolve`
  does not read are not in it. The command line names the record on its own `create` and `remove` lines after a
  real run, outside the per-adapter actions; a `--dry-run` names neither, because the plan of one
  adapter cannot know what an earlier adapter in the same run would have recorded or dropped.
  Measured by
  `uninstalling_one_claude_root_keeps_the_store_the_other_root_reads`.

## What the gate narrows, and what it records

- **`observed` is attached on more roads than it is rendered on, and the register naming what is
  still missing has twice claimed more than the code does.** A gate refusal was given a
  structured `Refusal.observed` account of what the gate parsed — the command fragment matched, the
  checkout the call was adjudicated against. Two panels rejected this entry, each on a sentence
  claiming coverage the code did not have; this version is written clause by clause against the
  code rather than by editing the sentences that were wrong.

  **Two internal roads, not one.** `gate_observing`/`decide_observing` receive the door's own
  reading and `prefixed` attaches it to every `Deny` they produce. `gate`/`decide` receive nothing —
  `decide` calls `decide_observing` with `&Observed::default()` — and since `prefixed` now emits
  `observed: None` rather than an empty object for an empty reading (the repair the previous panel
  asked for), a refusal built through this second road carries nothing at the point `prefixed`
  hands it back. `guard::adjudicate_action`'s single-holder arm (`src/harness/guard.rs`) is on
  this second road: it calls `gate`, not `gate_observing`.

  **Two callers share that single-holder arm, and only one of them can pick the account back up.**
  `cli::show_gate`'s session-less branch (`run_id: None`) calls `adjudicate_action` and then passes
  its `Decision` through `report_gate`, whose `Deny` arm attaches the door's own
  checkout-and-command reading to any refusal that does not already carry one — closing the gap for
  this caller, under `--json`, for every refusal `adjudicate_action` can produce on this branch: the
  single-holder arm's own (nothing, from `gate`/`decide`, topped up here), and the three
  `adjudicate_action` builds directly — `complete_review_receipt_not_selected`
  (`src/harness/mod.rs`, called from `guard.rs`), `run-pointers-unreadable` (`guard.rs`)
  and `several-runs-hold-this-checkout` (`guard.rs`), all three via `Refusal::not_started`,
  which never sets `observed` itself. `cli::git_hook`'s `PrePush` arm calls the *same*
  `adjudicate_action`, through `guard::decide` (`guard.rs`), a thin wrapper over it — but
  answers its own `Decision::Deny` straight to `report` and never reaches `report_gate`. So every
  one of those same refusals, reached from a push instead of from `estigia gate`, carries nothing,
  and nothing here can top it up: not one arm of the several, all of them, on that road.

  **Attached is not the same as rendered, and the one shipped caller that can reach the session-less
  road never asks for the rendering.** `report` (`src/cli/mod.rs`) writes `Display` — which never mentions
  `observed`, by the acceptance criterion it was built against — unless `json` is `true`.
  `src/setup/plugin.rs` invokes `${ESTIGIA} gate ${tool} --input ${payload}` with no `--json`,
  and reads `answer.stderr` as prose, says it when the status is neither allow nor refuse,
  and throws it as the refusal — the one shipped agent that drives
  `show_gate`'s session-less branch at all. So a refusal reaching it can be carrying a fully-built
  `observed` at the moment `report_gate` returns it, and the plugin never sees a trace: the account
  is attached and then discarded by serialisation, not by absence.

  What remains open, checked against the code rather than assumed. Every quantified claim below
  names the function that makes it true, rather than asserting it:
  - **The push road carries nothing, structured or otherwise, and cannot under the current
    wiring.** Not "a several-holder or unreadable-pointer refusal" — every refusal `PrePush` can
    answer with, because none of them passes through the one function that now attaches it.
  - **`show_gate`'s own two failure points ahead of `context_the_payload_names` carry nothing,
    because `observed` does not exist yet when they return.** `gate-input-not-json` (malformed
    `--input`) fires before `parsed` exists at all; `working-directory-unknown` and
    `home-not-resolvable` can both propagate out of `context_the_payload_names` itself (through
    `launch_directory` and `harness::session::state_root`) before its own return value — the
    `CheckoutObserved` half of `observed` — is built. Every one of the three reaches `report` as a
    bare `Refusal::not_started`, never `report_gate`.
  - **The MCP road builds no account and adjudicates a checkout anyway.**
    `src/harness/mcp/mod.rs`'s `run-id-names-another-checkout` refusal compares `context.repo_dir`
    against `run.covered()` — the same checkout question `decide_observing` asks — and answers
    through `Refusal::not_started` directly, never `prefixed`, never `report_gate`. A fourth
    uncovered road, not named here until a panel found it.
  - **The receipt-comparison half.** Nothing here reports what a delivery refusal actually compared
    a stated PR or head against; those refusals carry their own prose (`delivery_pr_mismatch`,
    `stale_verdict`) and none of it moved into `observed`.
  - **No hook dialect renders it, and a role-refused `PreToolUse` never builds it to begin with.**
    `src/harness/hook.rs`'s `PreToolUse` arm builds the full account — the same `classify_observing`
    and checkout resolution `estigia gate` uses — only for a call that clears every role check ahead
    of it: `authorize_review_blind_launch`, the `review-blind` reviewer's own `roles::gate`,
    `roles::definition_for`'s `Err` arm, and the named-agent `roles::gate` check. Any of those four
    denying returns through `dialect.deny` or `response_in` with nothing built. For a call that does
    reach the build, `Dialect::deny` still emits only `deny_reason` prose, for all five `Dialect`
    variants this crate has: `claude-code`, `gemini-cli`, `cursor`, `exit-code` (shared by Crush,
    Windsurf and every other host with no answer shape of its own), `cline`. Left open deliberately:
    the issue's own evidence is `estigia --json gate`'s answer, and a hook's answer belongs to the
    *host's* protocol — inventing a sibling structured key inside somebody else's schema is a
    decision with its own tradeoffs, not a quiet addition riding along inside this repair. It would
    need its own issue.
  Two more readings the push-road gap would need even with the wiring fixed, so closing it is more
  than routing a call through `report_gate`: `session::holdings` (`src/harness/session.rs`)
  reads each pointer file to build a `Run` and drops the path it read once the run is built, so
  which file was unreadable can be named in a message but not carried structurally;
  `guard::holders_of` computes a `coverage_depth` per holder to find the closest and
  discards every depth but the winner's, so how close each survivor's claim came is not a value
  anything downstream can still ask for.

- **The clamp `narrowed_by_the_call` applies now trusts one thing beyond `launched`: a pointer a
  claim already wrote.** Measured: `Worktree location` on a real install is
  `H:\REPO\worktree\<repo>\<branch>` — a **sibling** of the repository it was minted from, never a
  child of it — so a call naming that worktree could never resolve through the sibling-blind clamp
  the earlier entry above describes. A run that had claimed, published, obtained a verdict and
  released CI could not deliver: `estigia gate` adjudicated `gh pr merge` against the launch checkout
  on `main`, and answered `verdict-bound-to-other-bytes`. This fix admitted the worktree as evidence
  that the checkout was covered; it did not touch what `stale_verdict` compared a `gh pr merge`
  against, which stayed the invoking directory's own `HEAD` until a later change rebound that one
  delivery to the PR head the tracker reports (see the verdict-binding entry above) — so the launch
  checkout naming `main` stopped being what the comparison read at all. The invariant this repository runs on: an
  agent may work several tasks, and several agents may each work their own, whatever the runtime
  calling the gate — but a worktree holds exactly one task. The worktree is the unit of task
  identity, `repo_dir` is not, and several runs sharing one launch checkout is the *designed* steady
  state, never itself a conflict. `held_by_a_pointer` reads that invariant directly: a directory is
  now honoured when it lies inside `launched`, **or** when it is a checkout a pointer already
  records — drawn from `worktree` alone, whichever pointer is consulted, since `repo_dir` carries no
  task identity of its own. That holds for both roads, not only the identity-less one: a worktree
  nested inside the base checkout (`coverage_depth`'s own doc contemplates the layout, and this
  repository's own configuration permits it) would otherwise let one run's `repo_dir` reach another
  run's worktree, identity-carrying call or not, so neither arm reads `repo_dir`. Both arms read the
  call's own argument: the identity-less arm passes `named` straight into `holders_of`, where it is
  the key a holder is looked up by, and both arms require the holder's `worktree` to cover `named`.
  The lever the clamp exists to close is unchanged: a directory no
  pointer reaches is not admitted as evidence, exactly as before, and `checkout_root_covering`'s own
  filter still refuses to climb above `launched` — it drops a covered root only when it is a strict
  ancestor of `launched`, which is what
  `checkout_root_covering_never_climbs_above_the_launch_directory` still pins.

  Which pointer is consulted depends on whether the call carries a run id. With one — `--run-id`, a
  session id derived through `--session-id` with `--runtime`, or the ambient `ESTIGIA_RUN_ID` read at
  `src/cli/mod.rs` — `held_by_a_pointer` is restricted to that run's own
  `worktree`, so a call naming run B's worktree does not gain the directory as evidence: this arm
  answers `false`, `narrowed_by_the_call` returns `None`, and `launched_context` falls back to
  `launched`. The call is then decided against `launched` under the run id it carried — `show_gate`'s
  identified arm loads that one pointer and asks `harness::gate_observing`, which never consults
  `holders_of`, so no count of who holds `launched` is taken on this road. Nothing here measures
  a refusal on it; what this fix changes is only whether the named directory is admitted as
  evidence.

  Without a run id, `named` is asked for directly through `harness::guard::holders_of`, restricted
  to a single holder whose own `worktree` covers `named` — never a holder whose `repo_dir` merely
  happens to be an ancestor of it, which would admit any directory inside a single holder's base
  checkout, an unrelated repository's own root included, and adjudicate and ledger-record the call
  under that holder regardless of what `named` actually is. Because a worktree holds exactly one
  task, resolving to the single holder whose `worktree` covers `named` is the run that minted it.
  Resolving such a call to the run whose worktree it named is the invariant working correctly, not a
  fallback: the write is judged against that run's own live claim and recorded under that run's
  identity. What a run-id-less call genuinely cannot do is tell one caller's identity from
  another's, so it cannot refuse a call that *reaches into* another task's worktree — that flow
  violation is not enforced on this road. An operator who
  exports `ESTIGIA_RUN_ID` for a calling road that does not set it closes this without a code
  change; no shipped adapter currently does.

  **Both host generations carry an identity when the host sends one.** `estigia gate` is invoked by
  exactly one shipped adapter, the OpenCode plugin (`src/setup/wiring.rs`'s `plugin_wire`,
  `src/setup/plugin.rs`'s `source`); each generation forwards the session id its hook was handed as
  `--session-id <session> --runtime opencode`, and the gate derives the run id through
  `session::run_id` (`gate_identity`, `src/cli/mod.rs`) before `show_gate` is reached — so such a
  call takes the identity-restricted road, judged against the run it derives or against
  nobody, and a session rooted at the shared base is no longer refused for the runs that cover it.
  A call whose host handed no session id — and no exported run id — keeps the identity-less road.
  `ESTIGIA_RUN_ID` supplies the run id only when neither the flag nor a session id names one
  (`gate_identity`, `src/cli/mod.rs`), so an operator who exports it around the plugin's process
  closes that half without a code change, and a host that forwards a session id supersedes the
  variable rather than colliding with it. On the identity-less
  road the flow violation named above — one task reaching into another's live worktree by naming it —
  is not a road nothing exercises: every call with no identity at all takes it, a host that handed
  the hook no session id and left the variable unset among them. It is
  sharper than mis-attributed
  bookkeeping: `stale_verdict` (`src/harness/mod.rs`) reads
  `context.repo_dir` to decide whether a review is bound to the bytes it was produced on, and
  `context.repo_dir` is exactly the value that arm resolves, so a delivery boundary the call takes
  (`gh pr merge` among them) is adjudicated against that other task's own verdict binding, one the
  calling context never earned itself.

  What the forwarded identity cannot establish is that the claim was sworn under it. The derivation
  is the crate's own, and the contract's Runtime identity derivation pins the shape an agent must
  mint — but a claim already sworn under a hand-minted id (`opencode-f2ae8c`, `opencode-63195f18` on
  this machine) is not found by the derivation: the gate loads a fresh run, answers the call
  `outside`, which permits, and writes no ledger line — `Outside` is Estigia standing aside, not a
  decision to record. Every action from such a run is permitted rather than judged against its live
  claim, delivery boundaries (`gh pr merge` among them) exactly as routine writes, because nothing
  weighs the action once the run is not found — the trade this change makes so that a
  session is not refused for its neighbour's existence — and nothing in the gate detects the
  mismatch. The pinning is prose, not an enforcement, and no shipped adapter tells an agent its
  derived id the way `SessionStart` tells a Claude run.

   **Three readers of the resolved `repo_dir`, not one.** The same `context.repo_dir`
   this arm builds is also where configuration and role are selected, and the entry above named only
   the verdict binding. The three, each with the call that reads it:

   | Reader | What it decides from `repo_dir` | Where |
   |---|---|---|
   | Installed configuration | `tracker`, `boundaries`, `evidence`, `integration`, `window` | `gate_context` in `src/cli/mod.rs` — `skill::installed_config_in_keeping_what_parses(&skill_root, &repo_dir)` |
   | Role definition | which agent allowlist applies to the tool | the gate path in `src/cli/mod.rs` — `harness::roles::definition_for(&context.repo_dir, …)` |
   | Verdict binding | whether a delivery is bound to the bytes under review | `stale_verdict` in `src/harness/mod.rs` (already disclosed above) |

   `launched_context` sets that field from `gate_context(&looking_at)` after
   `narrowed_by_the_call` and `checkout_root_covering` resolve the candidate, so a model-written
   `workdir` or write target that a pointer already records reaches **all three**, not only the
   verdict reader. `checkout_root_covering`'s own comment once named configuration and role as the
   reason it resolves to a checkout root, then reduced the enforced filter to "never a strict
   ancestor of `launched`" without saying that a **sibling** root (another task's worktree, including
   one whose git directory is a different repository) is admitted on purpose under the one-task-per-
   worktree invariant and carries those two readers with it.

   **Cross-repository resolution is admitted when a pointer records the worktree, and is not a
   separate case this arm refuses.** A worktree path under `Worktree location` can belong to another
   clone on the same machine; if a live pointer names it, `held_by_a_pointer` honours it the same way
   it honours a sibling worktree of this repository. Nothing in this arm asks whether
   `repo_dir`'s git identity matches `launched`'s. That is the invariant working as written for task
   identity, not an accidental hole beside it — and it means the other repository's `.estigia/` rows
   and role definitions decide the call. The original request never named cross-repository resolution; the
   decision recorded here is that the code keeps it, and this entry is where a reader learns it
   rather than inferring it was considered harmless because it was silent. No test in this change
   drives a second repository's pointer through the identity-less road; the path is the same
   `held_by_a_pointer` / `holders_of` path the sibling-worktree fixtures already exercise, and the
   cross-repository reading is taken from that identity, not from a separate measured fixture.

   The identity-less road's own unreadable-pointer case is also narrower than the identity-carrying
   one's, in a way this entry had not yet named: `holders_of` walks every pointer through
   `session::holdings`, which calls `pointer_at` per file, and `pointer_at` answers `None` for a
   pointer that will not parse rather than surfacing that it exists — unlike `session::load`, given a
   run id, which loads such a pointer as `unreadable: true` rather than as absent. So a corrupt
   pointer does not merely fail to widen anything on the identity-less road; it can **remove a holder
   from consideration entirely**, and a directory two holders cover at equal depth can answer down to
   the one holder whose pointer still parses — `[holder]` rather than the two-or-more that would
   otherwise refuse. Nothing here corrupts a pointer to force that; it is `pointer_at`'s own existing
   fallback, and no test in this change drives it.

  This is the `estigia gate` CLI door only. The PreToolUse hook road (`src/cli/mod.rs`, the `hook`
  subcommand) builds its `GateContext` straight from the payload's own `cwd` and reaches neither
  `narrowed_by_the_call` nor `checkout_root_covering` — nothing here changes what that road accepts.

## What a measurement reconstructs

- **The review-cost measurement does not close what it was launched to answer.** The question was
  what makes a blind review expensive; the counterfactual it would have to be weighed against is
  still unmeasured — nobody has quantified what Estigia's panel *avoids*: rework a rejected verdict
  would have caught later, two runs racing one issue, a delivery of unreviewed bytes. The measurement
  reports a cost, not a saving, and this issue does not close on it.

  It also does not reproduce the conditions of the change it measured. The installed judge definition changed
  mid-run, on 2026-08-30, gaining the sentence naming the three severity classes and which one
  blocks; a later change replaced that sentence with the rule for choosing between them, so the
  cited lines have moved twice — the class names themselves are stated in several
  other places (`docs/review-cost.md` names them), but this is the first the judge's own definition
  carried. The change's eight judges ran under the earlier definition, which named neither `severe` nor which
  class blocks — the "three rejected verdicts, every one on a sentence" outcome the measurement is
  sampled from was produced by a judge never told which class blocks. A panel launched after the
  update measures the current shipped judge, not the one that produced those rejections, and
  `docs/review-cost.md` reports the later panel's numbers as that and nothing more.

  The two panels it compares are not identical carriers either: the comparison arm ran the other
  project's rubric pasted into Estigia's own `analyst` definition, so a difference between the arms
  is attributable to brief-plus-carrier, not to the brief alone — the cleaner design, one brief
  swapped inside a single fixed carrier, was considered and rejected because a comparison arm run as
  anything other than `review-blind` would no longer be measuring Estigia's own panel. And the
  comparison side of the comparison was never executed as its own project runs it: its rubric was
  read by this host and pasted into a launch prompt, not driven through its own binary, so what is
  measured is a reading of that rubric rather than that project's panel.

  No artifact of the measurement is retained in this repository — no saved transcript, no raw
  completion report, nothing a later reader can reopen. The per-judge figures in
  `docs/review-cost.md` — tokens, tool uses, duration — are exactly what the agent harness's own
  completion report returned for each launched judge at the moment it finished; nothing here
  re-derives them from a log this repository keeps.

  This is one trial: two judges per arm, no repetition, and no measurement of run-to-run
  variance. Nothing here separates a real 13.7% brief effect from a 13.7% sampling effect on a
  design this small. A claim that the two panels cost the same to run is not one a
  single-trial, two-judge-per-arm design can establish, whichever way the numbers land, and
  `docs/review-cost.md` reports the gap rather than making that claim. It quotes tokens and
  latency, 8.7% and 13.7%; summed judge time is +18.2%, the largest cost gap its own table reports.

  The measured target was also not a review transaction. `docs/review-cost.md` says so in its
  own opening section, and says this document carries the measurement's limits — so that fact
  belongs here too, not only there. `skill/references/repository-delivery.md`'s *Publish, review,
  and verify* section defines
  a review transaction as an epoch, PR, head, base, target digest and payload digest
  together; no epoch was reachable for the pull request because it was already merged before this
  measurement ran, so neither panel graded a receipt this repository's own gate would
  recognise as one.

  The comparison project's own coordinates cannot be verified from this repository. The six citations the two measurements above give for it —
  three to `internal/components/sdd/boundedreview.go` (a file `v4.0.0` no longer has),
  `internal/assets/claude/agents/jd-judge-a.md`,
  `internal/reviewtransaction/transaction.go:1621-1632`, and `judgment-day/SKILL.md` — are
  relative paths into its own checkout. Only `internal/reviewtransaction/transaction.go` still
  carries the span the run recorded; the other five name the file alone, so what they assert here is
  the file and not the line, and none is falsifiable by any reader confined to this one.

  **A second measurement, on 2026-08-30, drove the comparison project's own binary.** For itself it
  closes one of the limits above and narrows a second; both still stand over the first measurement,
  and `docs/review-cost.md` records the new one. Its target was a review transaction — epoch `2cbde7ffe09ff262d07cd885087520f7`, with head, base and target digest, though not the payload
  digest that definition also names. The comparison side was executed as its own project runs it
  rather than read from its source or pasted into a carrier, which closes the binary half of the limit
  at the paragraph beginning "The two panels it compares are not identical carriers"; the panel half
  of that same limit is not closed, because no reviewer result was ever admitted.

  What it did not obtain is that other half. **No reviewer result was ever admitted.** Three attempts
  were refused: one for a request carrying a token the contract had not rendered, which reached no
  reviewer; one because measuring a lens prompt records an emission that binds the lineage; and one
  because the reviewer returned a key the schema forbids. The last two discarded 181 and 265 seconds
  of reviewer execution. That measurement therefore compares what each system *sends* a reviewer and
  how many it selects, and compares nothing about what either system *finds* or how it grades it. The
  cross-system half of the grading question is open.

  Its two cost columns are bytes on one side and whole-agent tokens on the other, and
  `docs/review-cost.md` converts between them at four bytes per token — a ratio that measurement
  assumes and does not measure, stated there as an assumption. The comparison side is input only,
  never validated as an answer and never billed; the Estigia side includes the exploration a judge
  chooses to do. Its citations into the comparison project carry exactly the property the paragraph
  beginning "The comparison project's own coordinates" names: relative paths into a checkout no reader
  confined to this repository can falsify.

  **The tier that measurement observed is a false positive, and it says so.** The candidate was
  classified `high` and given four lenses on a `process_boundary` signal against `tests/honesty.rs`;
  the scan behind that signal greps for a leading `#!`, and that file's third line is a Rust
  crate-level inner attribute. The corrected tier — `medium`, one lens — is **derived from reading
  that project's source and was never observed**, so every figure in that section depending on the
  lens count is given both ways. Which line the scan actually matched is not in its recorded output.

  The version measured is recorded because the first pass did not record it: `a537c63e`, then
  re-confirmed at `4db804e5`, that project's `main` the same day and 3,003 commits later. Four things
  were compared across that range and all four were unchanged — two files byte-identical, and two
  constants — and the classifier answered identically, so the staleness cost nothing there. It was
  found by checking rather than by design, and a measurement that does not name its subject's revision
  cannot be re-run by a later reader.

  **No artifact of the second measurement is retained here either.** Its classifier output, consent
  prompt, per-lens byte counts and two durations were read from a binary's stdout against throwaway
  clones since deleted. The limit recorded above for the first measurement's per-judge figures applies
  to the second's figures in full.

  **No judge-versus-judge figure exists on either side, and none can be produced from
  the comparison project's source.** `docs/review-cost.md`'s closing section states why. Estigia's
  own per-judge figures in that document therefore measure its judges against nothing.

  **No figure in `docs/review-cost.md` measures a whole delivery.** Its per-judge figures come from
  two shapes of judge brief; its panel figures are panels. One delivery records what it
  could measure — its analyst, implementer and review layers — against its own review transaction
  through `record_spend`, and that delivery predates the orchestrator record, so it omitted the orchestrating run
  for the reason the `Review budget` entry above gives. Since then the orchestrating run has its own
  `record_spend --phase orchestrator` at delivery — `docs/review-cost.md`'s "What a tool answer
  costs" and its neighbouring section name the acceptance this closes — but that is one field
  existing, not a distribution: no whole-delivery figure spanning every phase of one issue,
  orchestrator included, has been recorded and reported here yet.

  **The wait a review now spends for its reviewers is measured before, not after.** `collect_review_input --wait-seconds` replaces polling a running lens with one bounded call;
  `docs/review-cost.md`'s "Waiting for a reviewer instead of polling" records the corpus's one
  session that reached the tool — 16 calls, 1 answered `capture-running` — and states the after as
  zero by construction rather than observed, because `launch_review_input` and `collect_review_input`
  are recent and no post-change session exists in the corpus.

- **A preserved usage artifact is not a measurement, and no total folds it in yet.** The runner now
  writes the reviewer CLI's own counters to `usage.json` in a job's directory, beside `result.json`
  and never inside it, and `job::read_usage` reads them back; the admitted answer's bytes and
  `result.json` are byte-identical whether or not the CLI reported usage. Three limits stand. Only
  the Claude adapter's `--output-format json` result carries a `usage` object, so a Codex review and
  a relayed review preserve nothing — no counter is invented for an adapter that does not report
  one. The totals `scripts/measure-transcripts.py` prints are read from Claude Code's own `.jsonl`
  transcripts and do not fold this in: nothing in the script reaches a job's artifact, so no reported
  total moved with this change. And nothing in production calls `read_usage` yet — the artifact is
  written, the reader exists and is tested, and no figure anywhere is derived from it, so what a
  native review actually cost stays unmeasured until something reads it. The artifact also outlives
  the result it belongs to: `discard_result` deletes `result.json` after admission and leaves
  `usage.json` in the job directory, so a reader must check `job_status` before trusting the counters
  of a job whose result is gone.

  **A missing artifact is unknown, not zero.** `read_usage` answers `None` both when the CLI reported
  no usage and when the counters could not be stored — a failed usage write is reported on the
  runner's standard error (`runner.err`) and never becomes the job's outcome, so a job can finish with
  its answer and no artifact for a reason other than "the CLI reported none". The two are not
  distinguished, and neither is a measurement. An artifact that exists but is not JSON, is another
  format, names no provider, holds no counters or carries a counter that is not a whole number is an
  `Err`: corrupt, and likewise never a zero. What is not measured here is whether a consumer folding
  counters in reads `None` as unknown rather than as zero — no such consumer exists yet.

## Refusals, and the way out they name

- **A refusal that names no way out is measured at build time, and the 74 the guard reports are
  frozen as debt rather than fixed.** `build.rs`'s
  `assert_every_refusal_names_a_way_out` reads the same walk over `src/transport` that fills
  `TRANSPORT_VOCABULARY` and holds three rules that were held by review alone — *do not name a
  command unless running it clears the block* (`AGENTS.md`), *an unknown result is not clearance*,
  and the contract's *never reinterpret an unknown result as clearance*. The population is every
  distinct reason the walk extracts, **177 at this head**, because a reason is the operator-visible
  identity of a refusal and the unit an operator can act on. A reason is satisfied when its envelope
  carries an `action` (`reasons_naming_a_continuation`), when `BY_DESIGN` declares a shape from
  `NO_CONTINUATION_SHAPES` — `operator-knowledge`, `world-action`, `human-authority`, which is
  `src/outcome.rs`'s own `NoCommandReason` vocabulary, crossed against it by
  `tests/guards.rs::the_refusal_guard_vocabulary_is_the_harness_own` — or when
  `refusal-baseline.txt` freezes it. A `BY_DESIGN` declaration for a reason whose envelope already
  carries an action fails the build, so the table cannot hide a working continuation.

  **What it found.** Of 177 reasons, **100 carry an `action`**, **3 are declared by-design**
  (`blank-argument`, `invalid-horizon`, `invalid-operation-id`, all `operator-knowledge` — the
  caller supplies a value, and no command clears it), and **74 name no runnable continuation and
  declare no reason why**. Those 74 are the finding this change files and does not fix: the
  operation-conflict and metadata-mismatch families, every `*-readback-failed`, and the generic
  argument refusals `malformed-argument`, `missing-argument` and `unknown-operation` among them.
  `refusal-baseline.txt` may only shrink: an entry that matches no unsatisfied refusal fails the
  build naming that entry, so *the refusal was fixed* and *the refusal moved into a shape the walk
  cannot see* are told apart. The walk's floor is the count that file records (`population 178`), so
  a walk that finds fewer fails naming the count — not a loose floor today's count happens to
  satisfy. The `>= 115` floor `build.rs` carried is gone with it, folded into the exact one.

  **What the guard cannot do.** It reads the operator-facing text for the presence of an `action`;
  it cannot tell a correct continuation from one that parses, runs and does not help. A refusal
  whose action names a command that does not clear the block passes here, and only a person hitting
  it can see that. It cannot tell a genuine `BY_DESIGN` from a fixable refusal someone declared to
  silence the guard, and it does not reach a refusal whose reason is built by a shape the extractors
  do not read — `assert_reasons_covered` closes that class, not this. Its own acceptance was
  measured by mutation: a refusal added with no action and no declaration fails naming
  `issue-228-probe`; a baseline line matching no refusal fails naming the line; and removing a
  refusal from the source fails the floor at `176 < 177`.

  **The rule is not hypothetical, and the three recorded instances were re-read.** A
  `several-runs-hold-this-checkout` refusal used to name *runs that no longer exist* and a
  resolution its reader could not perform, before `guard::adjudicate_action` asked the tracker about
  each holder. A phantom run pointer's refusal named an `estigia release` that did not discharge it,
  and deleting the pointer file was the only correction until `estigia release` was made to clear
  it. And `reviewer-definition-changed` named the installed file and a world-action to move it aside
  when the body was one Estigia itself had written, the real exit being a binary carrying the digest. Each was found by a person hitting it, and this is the first run that can report any of
  them.

- **The per-runtime capability table is canonical; it does not measure that any host obeys a
  capability it is declared to carry.** `src/setup/capabilities.rs` holds
  `canonical(InstructionFile) -> Capabilities`, a total `match` with no `_` arm, and every entry of
  `AGENTS` declares its own `Capabilities`. A declaration that disagrees with the table fails the
  build: a `const` assertion in that module runs `capabilities_agree` over the registry, so a row
  edited in one place and not the other is `error[E0080]` rather than a green suite. The same
  predicate crosses the table against the two `match`es that hold the detailed data — `gate_spec`'s
  events, matcher and dialect, and `agent_definitions_root` — so a column and its renderer cannot
  disagree at compile time. `AgentAdapter::checked` refuses a mismatched adapter at construction,
  which `an_adapter_whose_declaration_disagrees_with_the_table_fails_at_construction` constructs;
  and `capabilities_for_slug` is fail-closed, a runtime with no row carrying
  `Capabilities::NOTHING`, proven by `a_runtime_absent_from_the_table_carries_nothing`.

  What it does **not** measure: whether a host refuses a tool call, reads a hook, or discovers a
  skill at the root the table names. Every column is a claim derived from what each host ships,
  exactly as the prose it replaces was; the table makes the claim one place and checkable against
  the adapter, and no part of it runs a host. A column can be wrong about a host with the build
  green. What it buys is that a disagreement **inside the crate** — a row that says one thing while
  `gate_spec`, `paths_in` or a projection does another — is a compile error or a construction
  refusal rather than a state found at use.

  **Disagreements the reading surfaced.** One was fixed: `paths_in`'s `hooks` match carried an
  `InstructionFile::Cline` arm (Cline is retired since) that could never run, because it sits behind `adapter.gate_spec()`
  and Cline's `gate_spec` is `None`; the arm read as if Cline had a settings-file hook when its gate
  is the script `plugin` resolves, and it is removed. One is recorded and not fixed, because fixing
  it is a change of its own: ZCode's row carries `gate: Gate::None` while it carries the contract
  and the MCP server, which is the gap this document's ZCode entry already states, now stated in the
  table rather than only in prose. No other projection disagreed with the table. The `contract`
  column is `true` for every supported runtime — the directive is written for all of them — and
  `false` only in `Capabilities::NOTHING`.

  **This narrows the reporting gap and does not close it.** The gap is in the *reporting* — one word
  covering four states in what a person reads — and the table now holds, per runtime, which of the
  four states applies, so the row a report renders can be read off one place rather than inferred
  from the adapters. The reporting itself is untouched here. **One related part is already delivered**:
  agent definitions have a retirement list, `skill::RETIRED_AGENTS`, and this change does
  not touch it.

- **The merge-commit topology rule admits a forward-integrated delivery, and no longer proves the
  first parent was exactly the reviewed base.** `skill/SKILL.md` step 6 and
  `skill/bindings/github.md`'s `verify_claim` exception now require the delivered SHA's second parent
  to be exactly the reviewed head and the reviewed base to be an **ancestor** of its first parent;
  `skill/bindings/github/delivery.md` and `skill/references/repository-delivery.md` state the same
  predicate for the run's own post-merge readback and for `release`. `match_merge`
  (`src/transport/claim/close.rs`) is fed that ancestry rather than comparing the first parent
  directly, and `delivery_close` computes it with
  `git merge-base --is-ancestor <receipt.base> <first parent>` in `context.repo_dir`, reusing
  `transport::target::is_ancestor` — exit 0 is an ancestor, exit 1 is not, and any other exit is a
  `Read` failure rather than "not my delivery", because an unknown answer is not clearance.

  What the narrower rule **no longer proves** is that the first parent was exactly the reviewed base:
  a delivery GitHub merged after `main` moved is recognised even though its first parent is a
  descendant, so the rule admits a forward integration that the old exact-parents comparison refused.
  What it still proves is that the reviewed head is the second parent and that the reviewed base is in
  the first parent's history, so a first parent unrelated to the reviewed base — a genuinely diverged
  or rewritten history — is still refused. The two cases are measured end to end in
  `transport::claim::close::tests`, against a real repository whose base has one descendant commit and
  one unrelated root: `a_forward_integrated_delivery_is_recognised_as_this_runs_own_close` and
  `a_delivery_whose_first_parent_is_unrelated_is_not_this_runs_own_close`. Making the ancestry answer
  `true` unconditionally reddens the unrelated case; restoring the exact-parents comparison reddens
  the descendant case. The branch-protection entry above already records that Estigia never
  guaranteed the first parent *equals* the base — the repository's strict check was that guarantee,
  and this change does not touch it — so this narrows the close proof to the part it can actually
  read.

- **A Linear source reads an issue, writes nothing but a mapped state, and its transport is the
  `linear` CLI.**
  `Tracker linear <team-key>` is admitted only under `Workflow: assisted`, and the row
  that admits it is answered in a repository's own file: `config set` refuses the value without
  `--repo` (machine-wide), `config edit` refuses it, `config set --repo` refuses it where the
  repository's `Claim workflow` is not assisted, `config set Workflow autonomous --repo` refuses it where
  the repository's `Tracker` is already Linear, and `doctor` and run start refuse it. `assist` reads
  the issue's title and description with one GraphQL query (`src/transport/linear.rs`'s `QUERY`),
  which is a constant; `a_read_spawns_linear_api_with_the_query_and_sends_no_mutation` holds the
  exact argument list a read spawns, and that it is the only call. The source can send two more
  constants, `STATES_QUERY` and `MOVE_MUTATION`, and only through `move_issue`, which `follow_state`
  calls only for a state the repository's `State names` row maps and only for a Linear key —
  `follow_state_moves_only_a_mapped_state_of_a_linear_issue` holds both, and that a GitHub issue is
  never sent to Linear. A move is read back and reported under `linear_state` beside the tool's
  answer; it is never retried, and a run that ignores a failed one leaves Linear behind the work,
  which nothing here notices afterwards.
  Every operation is spawned as `linear api --variables-json <variables> <operation>` through the
  crate's one process boundary, with a 30-second deadline. The CLI holds the credential — the login
  `linear auth login` stored, or `LINEAR_API_KEY`, which it reads first — so Estigia never reads,
  stages or passes the key, and only the operation and its variables (an issue key, an issue id and
  a state id) travel on the argument list. A failed request is classified from Linear's `errors`
  JSON, which the CLI prints on stdout for some answers (an unknown issue) and on stderr for others
  (a validation error); a CLI with no credential answers `linear-credential-missing`, a program not
  on `PATH` `linear-cli-missing`, and a failure carrying no JSON `linear-unreachable` naming what the
  CLI printed. `assist` refuses an issue argument that is not an identity at all
  (`assist-issue-does-not-match-the-tracker`).

  What this does **not** do: it writes Linear only to move a mapped state — never a comment, label
  or assignment — and it does not prove a Linear credential can be scoped read-only; the operator's
  own credential decides that. It depends on the `linear` CLI's own behaviour, measured against
  version 1.10.0: the `api` subcommand, `--variables-json`, exit status 1 on a failed request, and
  the wording `No API key configured`, which is matched as text. A later CLI that changes any of
  these breaks the read, and a changed no-credential message would be reported as
  `linear-unreachable` rather than as a missing credential. The workspace is the CLI's choice — its
  default workspace, or `LINEAR_API_KEY`'s — and Estigia runs it from its own working directory, so a
  repository's `.linear.toml` is not guaranteed to be the one read. Only the two reads were run
  against the live API; the move was not, and is measured only against the stand-in. A key the
  credential cannot see and a key that names nothing both answer `linear-issue-unknown`, so the two
  are not told apart. Authentication reported inside a GraphQL `errors` array is **matched
  heuristically** — an entry whose `extensions.type` or message mentions "authentication", or whose
  message mentions an "api key" — and so is a "not found" error (`extensions.type` or message,
  checked after the authentication match), which answers `linear-issue-unknown`; any other `errors`
  with no `data` answers `linear-unreachable` naming the message, and an authentication or not-found
  error worded otherwise would be reported that way too. `assist` refuses a key whose team (the text
  before its last `-`, compared without regard to case) is not the configured `<team-key>`
  (`linear-team-mismatch`), and reads a key as upper case. And nothing here crosses the Linear
  client against a second implementation: the stand-in `linear` is the `fake_process` example, and
  it measures only the argument lists the source spawns and how it reads the answers scripted for
  them.

  **The unclaimed-write refusal and the Unclaimed write policy hold under a repository whose own file
  sets `Tracker linear` with `Workflow: assisted`, as they do under GitHub.** Both used to stand down there, because each asked
  `Tracker::transport()` and a read-only tracker has none, so an unsworn `git commit` or `git push`
  went unrefused and Unclaimed write policy was off. They now ask
  `Tracker::adjudicates_unclaimed_writes`, true for a tracker a run can hold a claim against, which an
  assisted run does hold against Linear locally and no other workflow does: Linear with workflow
  unset or autonomous stands down as before (`a_read_only_tracker_without_an_assisted_workflow_refuses_nothing`;
  `doctor` flags that configuration). Under `assisted` both refusals name `assist`, not `claim`,
  `create` or `start_branch`, which are refused there
  (`an_assisted_run_is_told_to_assist_not_to_claim`, and
  `an_assisted_run_under_github_is_told_to_assist_not_to_claim` for GitHub). The SessionStart text in
  `harness/hook.rs` branches on the workflow too, so under `assisted` it names `assist` and none of
  `claim`, `create` or `start_branch`
  (`an_assisted_session_start_names_assist_and_not_the_refused_tools`).
  `a_guarded_checkout_under_a_read_only_tracker_refuses_an_unsworn_commit`,
  `required_refuses_an_unclaimed_edit_under_a_read_only_tracker` and
  `ask_puts_an_unclaimed_edit_to_the_person_under_a_read_only_tracker` went red against the code
  before the change, and
  `a_keyed_assisted_run_under_a_read_only_tracker_is_still_admitted_in_its_worktree` measures that
  the assisted run is still admitted. What this does not measure: the tests build the gate context
  with `Tracker::Linear` directly, so the repository-file layer that produces it is read by its own
  tests, not crossed here. `estigia guard` and the pre-push hook say a read-only tracker without
  `assisted` adjudicates nothing, and the guard's caveat no longer promises the refusal it does not
  make (`a_read_only_tracker_without_an_assisted_workflow_is_said_to_adjudicate_nothing`).

## Test counts and mutation claims in a receipt

- **A review transaction's evidence prose and a pull request body are not files in this repository,
  and nothing reads either: a test count or a mutation claim written into one is unchecked prose.**
  The operator decision of 2026-10-04 chose this record over rewriting a merged body.
  The measurement is one pull request's evidence section, which says *"Six new tests, each demonstrated red
  under mutation **in both directions**"* and tables six rows, against a diff that added **nine** test
  functions to `src/cli/tests.rs`. The three it never tables are
  `without_a_run_id_a_sibling_directory_no_pointer_covers_stays_refused`,
  `without_a_run_id_a_directory_only_a_holders_repo_dir_covers_stays_refused` and
  `a_call_carrying_an_identity_cannot_reach_a_sibling_run_s_worktree_nested_inside_the_base`. Its
  mutation paragraph is aggregate — *"Four mutation runs were taken. Reverting the clamp reddens
  four of the six; opening it wide reddens the other two…"* — so it never records which test each
  run reddened, and the three outside its table carry no stated result either way.

  **The two whose silence costs the most are named, and their result is measured here rather than
  left implied.** `without_a_run_id_a_directory_only_a_holders_repo_dir_covers_stays_refused` and
  `a_call_carrying_an_identity_cannot_reach_a_sibling_run_s_worktree_nested_inside_the_base` pin the
  narrowest property the change added: `held_by_a_pointer` (`src/cli/mod.rs`) reads a holder's
  `worktree` and never its `repo_dir`, in the identity-carrying arm and the identity-less one alike.
  Widening that function back to `Run::covered()`, which chains `repo_dir` before `worktree`,
  reddens both of them and leaves the other seven of the nine green. So the receipt's silence about
  these two was not the absence of an experiment that would have caught them, only that no per-test
  result was written down. The third
  unlisted test, `without_a_run_id_a_sibling_directory_no_pointer_covers_stays_refused`, declares
  its own non-mutating status in its doc comment — *"it stays green under both the old widening and
  the repaired narrowing"* — and the declaration is correct: nothing in the change reaches it.

  **What this does not do.** It does not put a count back into that pull request: a merged pull-request body is
  no surface any delivery step carries, and the operator decision of 2026-10-04 ruled the rewrite out
  rather than open one. It does not remove the sentence that disagrees with its own diff — that
  sentence lives in a merged body, not in this repository, and no file here states a count for that change. And it does not build a reader for pull-request bodies: nothing in the suite opens
  one, so a receipt's test count and its mutation claims stay prose held by review alone, as the
  counts in this document were until `tests/honesty.rs` crossed them. Recording the set of test
  names a receipt claims, and diffing it against the set the change derives, is a separate
  enhancement and not this entry.

## Citations, and the coordinates that rot

**`docs/` cited code as `path:line`, and nothing crossed the coordinates — a base integration
shifted the lines and a pin silently pointed at unrelated code.** Two such pins were found
wrong by two blind judges on the same delivery, both landing on unrelated code — one on
`review_receipt`'s doc comment and its serde default where the `reviewed_head` reach limit was meant,
the other on prose inside a worktree-discovery guard where a `PointerEffect` arm was meant — while
the suite was green on three platforms. `every_link_in_this_repositorys_documentation_resolves`
crosses markdown link targets and knows nothing about a backticked citation, so a wrong markdown
target failed loudly and a wrong line pin failed nothing. The rule now is that a citation names the
file and the item inside it, never a line, and
`every_repository_citation_in_the_documentation_holds_up` (`tests/honesty.rs`) is the crossing: it
fails on a `path:line` citation it resolves to a repository source file, on a carried line (`:NNNN`), on a `path:line`
whose repository-prefixed path is not there, and on an item one of the two explicit forms binds to
a file that does not hold it. A bare path — one with no coordinate — is not checked against the
tree, because in prose it may be illustrative, historical or external, and it is the coordinate
that rots.

**What the crossing cannot see.** It reads `README.md` and `docs/**/*.md` outside fenced blocks, so a
citation inside a code block is unchecked; it binds an item to a file only in the two forms it
recognises — `item` then `path` in parentheses, and `path`'s `item` — so an item named in a looser
construction is not checked against its file; an item the cited file merely mentions satisfies the
binding, so a citation to a file that calls an item rather than defining it passes; it collects an item
only from the first token after one of nine definition keywords, so a short name, an associated item,
or one not first on its line is not checked at all; it reads a span as a `path:line` only when the
path before the colon carries an extension, or names a repository prefix or one of the root files
the crossing knows, so an extensionless file outside that list, cited with a line, is read as a bare
path and skipped; and it resolves a suffix-form path only among `src/` and `tests/` `.rs` files, so
a suffix naming a shipped file elsewhere is not resolved; and it cannot tell a path into another
project from a repository one, so a citation
into the comparison project is skipped rather than resolved, and a missing path that does not start
with a repository prefix is skipped with it. None of these is a hole the old form did not already
have — a line pin was crossed by nothing at all — but they are the shape of what is measured, and a
citation added in a form the test does not parse is prose held by review alone.

**A bare path is not checked against the tree, and the rewords that tried to are undone.** The
crossing resolves only a citation that carries a `path:line`; a path on its own may be illustrative,
historical, or a file in another project, so it is left to prose. Nine such paths name a file the
tree does not hold — the comment-ceiling test, the Python `github.py` at three sites along with its
oracle, the retired SDD protocol, and three illustrative paths the repository never had — and the
test written `tests/guards.rs::the_refusal_guard_vocabulary_is_the_harness_own`, a file and its test
in one span, stands as written too. A `path:line` to a missing repository file is still caught. The
history is kept; the coordinates are not.


## Generated files and reproduction

- **A declared generated file is reviewed by reproduction at approval time, not by reading.** The payload shows it as a manifest line and `review_computed_verdict` regenerates
  it before recording `approved`; the digest still binds the real head blob. Third slice: the tier and
  the correction budget no longer count a manifest path either (next entry).
  [`generated-files.md`](generated-files.md) has the format. What this measures is that the head blob
  equals what the base's declared command writes, when the verdict is recorded. What this does not
  measure:
  - **The head is trusted only through that comparison.** No judge reads a manifest path's bytes, so a
    command that regenerates something harmful passes. The declaration is trusted because the base it
    is read from was reviewed.
  - **Drift.** The gate runs when the verdict is recorded, not at merge. A registry that moved between
    the two makes an honest file mismatch (refused) or a stale one match (approved) at that moment only.
  - **The network.** The command inherits the caller's environment and may reach the network, so the
    answer depends on the machine that recorded the verdict. `unavailable` refuses; it is never a match.
  - **A reviewer can still comment on a manifest path.** Finding locations are checked against the
    real diff, so a finding there is allowed; the prompt only asks reviewers not to.
  What this does not measure:
  - **An honest file can mismatch.** A lockfile resolved against a registry that moved since the commit
    regenerates differently. `mismatch` means the bytes differ, not that the change is wrong.
  - **The command runs with the network and the host's tools, and only when asked.** It inherits the caller's environment
    and PATH, so a result depends on the machine that ran it. `unavailable` is the answer when it
    cannot run, never a match.
  - **The command is repository code, run by this binary.** It is trusted only because the base it is
    read from was reviewed; a declaration in the head is ignored for that reason. Nothing sandboxes it.
    A verdict read runs it only with `reproduce: true`; without that the paths read `not-run`. The
    approval gate runs it unconditionally, under the claim.
  - **Attribute conversions are not applied to the comparison.** The regenerated file is hashed
    without filters, against the head blob as stored.
  - **Tests drive `sh` with `printf`, not `npm`.** No real lockfile generator was run.

- **The tier and the correction budget do not see generated churn.**
  `review_tier` leaves a manifest path out of `changed_lines` and of the content scan, and the
  correction budget leaves it out of the original count and of every repair's charge, so a regenerated
  lockfile neither licenses lenses nor spends the 200-line allowance. Both read
  `generated::manifested_paths`, the manifest's own eligibility, and the tier answer names the paths in
  `generated_paths`. What this measures is that the figures agree with what a reviewer is shown. What
  it does not measure:
  - **Trust moves to reproduction.** A hot-path name, a shebang or a dangerous sink inside a manifest
    path no longer raises the tier. Only the approval gate's byte comparison stands between that
    content and a merge, and it is the base's declared command, not a reader, that vouches for it.
  - **A generated-only change is `medium`.** With nothing measured the tier keeps its empty-diff answer;
    it is not read as `low` because the reproduction that would justify that runs only at approval.
  - **A correction can change a generated file without limit.** Its churn is free, so a repair that
    edits a manifest path by hand is bounded only by the reproduction mismatch it would cause, not by
    the budget.
  - **Anchors still see the real diff.** A finding on a manifest path is accepted, because a reviewer
    may legitimately open the file through `inspect`; the exclusion is of counting, not of anchoring.

- **A timed child is drained, given no stdin, contained and reaped.** `spawn_with_deadline`
  reads both pipes on threads while it polls, gives the child a null stdin like the untimed
  `Command::output()`, runs it under `reviewer::process_tree`, and on a deadline kills the tree and
  waits. What this measures is that a child writing a megabyte, reading stdin, emitting non-UTF-8 bytes
  or leaving a grandchild on its pipe returns. What it does not measure:
  - **A grandchild is cut off after two seconds.** Once the child has exited, the pipes get
    `PIPE_GRACE` to reach end of file; past it the call returns what was read and terminates the tree.
    Output a lingering descendant writes later is not seen.
  - **No pipe-buffer size was measured.** The 1 MiB test exceeds any common buffer; the figure is not
    this machine's.
  - **The zombie is not asserted.** `wait` follows the kill, but no test inspects the process table.

- **A declared-checks receipt binds the tracked bytes and the commands, and nothing else.**
  `run_checks` keys its receipt on the clean-target digest `publish_review` binds and on a SHA-256 of
  the declared commands ([`generated-files.md`](generated-files.md#declared-checks)). What that does
  not measure:
  - **Inputs outside the digest are not in the key.** A gitignored file, an installed toolchain, an
    environment variable or the network can change a check's result without changing either digest, so
    the same receipt can stand for a run that would now fail, and a failed one stands for bytes that
    might now pass. A receipt that failed is never rerun for the same digest; a new publication is the
    way to ask again.
  - **The checks run on the published head, not on the merge result CI tests.** A base that moved
    since the publication is not in what ran.
  - **Host MCP timeouts are not measured.** The wait is capped at 50 seconds and each call returns
    inside it on this server's own clock, on the reading that some hosts give up on a call near 60
    seconds. No host was run to confirm where its limit is, and a host with a shorter one can still
    abandon a call that is waiting.
  - **A timed-out job records nothing.** It is not a verdict on the change, so the next call runs the
    checks afresh, and a check that always overruns 30 minutes never yields a receipt.
  - **The runner's cause of death is not read back.** A job that ended without a result is
    `checks-runner-lost` with whatever its stderr log held; nothing distinguishes a crash from a kill.
  - **The unit tests run the runner on a thread, and the detaching launch is not crossed for this job.**
    The `__check-job` pipe test, Unix only, runs the subcommand directly. The launch that detaches it
    is the one `__review-job` uses, crossed by `tests/pipe/review_job.rs` for that subcommand alone;
    nothing starts `__check-job` through it, and nothing exercises the Windows flags.

## Which hook commands are Estigia's

A settings-file hook is recognised by its shape, `<program> hook <event>` (`runs_estigia_hook` in
`src/setup/render.rs`), not by a substring, so `slack-webhook stop` is no longer taken for the gate,
removed by `setup` or `uninstall`, or reported as gated. What this measures is the one regression test
that writes such a command and runs `setup` then `uninstall` over it. What it does not measure:

- **The program is not checked.** Renamed binaries and shims are accepted on purpose, so another tool
  whose command line is literally `<program> hook stop` is still read as Estigia's.
- **Only the first word is the program.** A wrapper in front of it (`env X=1 estigia hook stop`,
  `sh -c "…"`) is not recognised, so setup leaves it and writes its own entry beside it.

## The Pi system section and the prompt cache

The `<estigia>` section Pi carries (`estigia session start`) states the queued GitHub writes without
their count, so a queue that only grows leaves the text byte-identical. What this measures is the one
unit test that serves the text with one and with five queued writes and compares the bytes. What it
does not measure:

- **A real state change still changes the section.** A claim, a release or a changed isolated checkout
  rewrites the text, and on a model without mid-conversation system messages that rewrites the leading
  prompt and costs the history cache once.
- **Read from source, not from a run.** That Pi 1.1.0 records a changed section as a later system
  message, and that pi-ai folds it into the leading prompt for most providers, comes from reading
  Pi's source; no live session was run against a provider's cache.
- **The uncommitted-work paragraph is the section's largest and most volatile part.** Measured on
  2026-10-22 with `estigia session start --session-id probe-a --runtime pi --wake poll` under a scratch
  `HOME`, against a binary built at the checkout's own head: 1,228 bytes, of which **945** are the
  paragraph naming five checkouts no run holds and the files each carries, 141 the "no contract is
  installed" sentence, 126 the run-id line, 10 the heading and 6 the blank lines between them. What
  this does not measure: any provider's cache — the figures are bytes, not hits — and no session was
  run with a live claim, so the claim line's own length is unmeasured.
  [`prompt-cache.md`](prompt-cache.md) carries the table and its conditional fifth part.

## What `release_ci` says about CI

`release_ci` returns `lane`, the reading `publication_lane` gives for the released head, and `next`: `merge` for
`not-consulted`, `disabled` and `never-ran`, `ci_status` for every other reading. What this measures is the unit
tests that script each lane reading and the one that scripts a failed read. What it does not measure:

- **`absent` goes to `ci_status`.** The read follows the ready mark within moments, so a head whose check runs have
  not registered yet is indistinguishable from a repository with Actions on and no workflow; merging on it could skip
  a CI that is about to start. A repository in the second case still waits on `gh pr checks`, which reports no checks.
- **An unreadable lane goes to `ci_status`.** Not knowing is not a licence to merge; the cost is the old wait.
- **No end-to-end release against a lane that is off.** The `release_ci` payload is asserted through the pipe only
  for an unreadable lane; the off, `disabled` and `never-ran` branches are asserted on the function that builds the
  answer, not on the whole operation.

## What `deliver` does not see

`deliver` runs the steps from the computed verdict to the close and hands back at the first that needs
a decision. What this measures is `src/harness/mcp/deliver/tests.rs`: the sequence against a scripted
set of tool answers and a scripted `gh`, over a real `git` repository with a bare origin, covering the
happy path, each hand-back, a resume that repeats no write, and a merge whose answer was lost. What it
does not:

- **The live tools and the gate are not driven end to end.** The tests script what `record_computed_verdict`,
  `release_ci`, `release` and the merge decision answer. One pipe test drives the real MCP server up to
  the receipt hand-back; nothing runs the whole tail against the real store and the real gate, so the
  seam between the two is asserted by reading it, not by a run.
- **The required set is the pull request's whole rollup.** The host's required names and the
  repository's own lanes are not resolved as *CI, merge and delivery* describes for a run by hand: a
  failing check nobody requires hands back `ci-red`, and a required check that never reported is not
  noticed while the rest are green.
- **An empty rollup waits ten minutes from the `ci-release` marker.** A repository whose pull-request
  run starts later reads `ci-absent`; one with Actions on and no workflow answers `pending` for the
  whole window before it does. The window is a constant, not a measurement of any host.
  On a `not-consulted` lane an empty rollup passes at once, so a repository whose lane is off but
  whose CI does run can merge before its first check appears, as `release_ci` already allows there.
- **The 50 s bound is on what `deliver` waits for, not on what it calls.** A `gh` or `git` call that
  hangs, or a slow `release_ci` clean-target read, is not cut off by `wait_seconds`, and a host that
  times a tool call out below that is not known to the server. No host's timeout was measured.
- **A merge queue or an approval rule hands back.** A merge command that answers and leaves the pull
  request unmerged reads `merge-unconfirmed`, one GitHub refuses reads `merge-refused`; neither is
  told apart from the other causes, and the queue's configured method is not proven to keep a merge
  commit before the attempt. A merge whose read-back never answers reads `merge-unknown`, not either
  of those: it may have landed, and the next call re-reads it. A merge test or a changed manifest
  that cannot be read reads `base-unreadable` or `version-unreadable`, never as advanced or unchanged.
- **Only two manifests are read for a version.** `[package] version` in a changed `Cargo.toml` and
  `"version"` in a changed `package.json`; a `pyproject.toml`, a workspace-inherited version whose
  root file did not change, and any other manifest move without a hand-back.
- **The lens roles in the evidence follow the tier and focus recorded on the receipt**, not each
  lens's report, so a narrow repair reviewed by the delta lens alone is named by its tier's roles.
- **The summary line has two languages.** `English` and `Español` have a sentence; any other
  `Summary language` is written in English.
- **The GitHub issue is closed after the release.** A failure of that last close leaves the issue
  `done` and released but open, and the answer says so as `delivery_incomplete`.

## What `estigia changes` does not do

`estigia changes` is the one read of uncommitted work per checkout, and `estigia prune` lists every
checkout it keeps. What this measures is the unit tests that build real repositories with a dirty
orphan checkout, a held one, a directory git cannot read and a repository with no commits, and one pipe
test that drives the binary. What it does not measure:

- **Two surfaces render the listing, and neither has run live.** The Pi `/estigia-changes` command and
  the Pi widget (`ctx.ui.setWidget`, a count of checkouts from `estigia changes --json`) are driven by
  node against a stand-in binary, never by a live Pi; the stand-in's `setWidget` is a recorder, so
  how Pi lays a widget out above the editor is not measured. The Claude Code `/estigia-changes` slash
  command is tested as a file `setup` writes and removes by the ownership rule, not as a command Claude
  Code ran: whether the agent shows the output unchanged, as the prompt asks, is its own behaviour and
  is not measured. Not built: the OpenCode TUI plugin (indicator and command) — `setup` writes nothing
  to `tui.json` today and no pattern for it exists to follow — the Claude Code status-line segment, whose
  setting is the person's and which `setup` does not write, and a Codex notice of its own. OpenCode,
  Claude Code and Codex see the session-start notice and nothing else, apart from the Claude Code
  command, and none of it has run in a live session.
- **The session-start notice is the binary's text, not a per-runtime surface.** It reaches Claude Code,
  Codex, OpenCode and Pi because they all inject `SessionStart` text; no live session of any of them has
  shown it. It counts uncommitted files with one `git status` per linked checkout under a three-second
  budget and names what it did not reach; the base checkout is never named, and a checkout a pointer
  names but git does not register is not read.
- **The `done`/`release` notice is tested on its function, not through a tracker.** The helper that
  decides whether to append it is unit-tested against a dirty repository; the wiring in `run_tool` is
  not driven end to end, because a `transition` or `release` needs a scripted tracker. It reads the
  pointer's checkout, so a run whose pointer recorded none gets no notice. It is a notice only; the
  decision not to refuse is documented in `lifecycle.md`.
- **No session-end notice.** `SessionEnd` answers `{}` and no runtime is known to show that output to the
  person, so a session that leaves its checkout dirty is told nothing at its end; only the next session's
  start reports it, as an orphan.
- **`orphan` is a readable run pointer, not the claim.** A pointer a run left behind reads as held,
  and a claim whose pointer never recorded a checkout reads as an orphan. The issue's state is not read.
- **Line counts are Git's, and partial.** An untracked file is counted only when it is a text file of at
  most one mebibyte; a binary file, or one counted against a checkout with no commits, has none. An
  unmerged path is marked `M`.
- **Fifteen seconds is the bound on each Git call**, chosen rather than measured.

## The session registry

`estigia sessions` lists the rows `estigia session start` and `estigia session end` write to the `sessions` table,
joined with the run pointers. What this measures is five pipe tests that drive the binary — a started, a crashed,
an unregistered and an ended session, with the crashed row aged in the store; a bounded walk of five registered
sessions whose pages have neither an omission nor a duplicate; each refusal a page answers; and a page answering
an empty registry while offering no cursor when nothing remains — and four store unit tests. What it does not
measure:

- **`last_seen` is refreshed by a session start and by a verified claim, not by every gated write.** A session
  that holds no claim and runs past the six-hour bound reads `stale` while alive. The bound is a constant, chosen
  to err toward showing a live session as stale rather than a dead one as live.
- **Only sessions whose host calls `estigia session start` or fires a `SessionStart` hook Estigia registered are
  registered.** Pi, Claude Code and Codex do; OpenCode's gate plugin registers a session the first time a tool call
  of it completes, so an OpenCode session that has made none is unknown to the registry. The rest appear as
  `unregistered`, and only while their pointer names an issue.
- **A pointer is read through `holdings`, which finds a file only under the name its run id gives itself.** A run id
  with an uppercase letter is filed under a digested name, so its pointer is not found and its issue is not shown.
  Minted ids from the hosts measured here are lowercase; the case was found while writing the test and is not fixed.
- **No pid, no host query.** Nothing asks a runtime whether a session is alive.
- **A bounded snapshot is a filtered re-read, not one consistent read of the machine.** Each
  page re-reads the store and keeps the rows whose `started_at <= taken_at`; the ordering key is immutable, so a
  full traversal has neither omissions nor duplicates under the tested mutations, but a row's `status` and
  `last_seen` are re-derived per page and can differ between pages. The cursor's digest detects corruption and an
  edit that does not recompute it; it is **not a MAC**, and possession of a cursor is not authorization. The
  CLI's `--limit` and `--cursor` are adapters of the same read, so every limit here holds for it as it does for
  `list_sessions`.
- **A snapshot's membership is frozen only to the second.** `started_at` has second granularity, so
  `started_at <= taken_at` admits a session that registers in the same wall-clock second as `taken_at` but after
  the page was taken, and a later page can show a row the first page never had. The ordering key is what is
  frozen outright; the behaviour is pinned as a baseline by
  `a_session_registered_in_the_same_second_as_taken_at_can_join_the_walk`.
- **An unreadable store answers the shared `unknown` identity.** Where the store's identity cannot be read — the
  Windows arm when its handle read fails, and any platform that is neither Unix nor Windows — `store_identity`
  answers `unknown`, which every such store shares, so the cursor's store check cannot tell two of them apart; a
  page taken against either refuses before it lists anything.
- **A `status`-filtered traversal is not a complete set, and no test makes it one.** The value a selector compares
  is read afresh from each page's own read, so a row that ends or heartbeats between pages can leave a
  `status`-filtered walk, and `matched` is that page's count rather than one held across the walk. The behaviour is
  pinned as a baseline by `a_status_filtered_traversal_reads_the_status_afresh_and_can_omit_a_changed_row`, which
  asserts the omission rather than a guarantee: a design that made the walk complete would redden it. Nothing in
  this design promises a complete set, and a caller that needs one reads the whole list and filters it.
- **A bounded page's snapshot is the registry rows only.** A pointer-only run has no immutable start, so it is not
  in the snapshot and a bounded traversal can neither omit nor duplicate it; `status=unregistered` is refused with
  `sessions-unregistered-not-bounded`, and the full list (`estigia sessions`, or `list_sessions` with no options)
  still shows those runs. The no-omissions property is therefore a property of the registry.
- **A bounded answer has no fixed byte ceiling.** What bounds it is the page size (`limit`, default 50 and at most
  500) and the row shape. The baseline measurement — a real `list_sessions` call on 2026-10-18 returning 433
  sessions whose compact JSON was 141,698 UTF-8 bytes — is a response-size measurement, not a billed-token
  estimate.
- **The pane is one environment variable, read once.** `HERDR_PANE_ID` was measured on 2026-10-22 inside herdr
  0.9.3 and 0.9.5: the OpenCode processes in `w0:p1` and `wE:pA` carried it, and the value matched the pane ids
  `herdr pane list` reported. One pipe test starts a session with the variable set and one without, and the
  second asserts the field is absent. Not measured: a host that exports the same name for something else, a
  Windows pane, or a pane id that is still the session's after the pane closes. The row records what the
  registering process inherited, not a live host query, and it is a locator rather than a way in.
- **No service URL is recorded for any runtime.** No adapter observes one. The OpenCode service seen on
  2026-10-21 (`127.0.0.1:49374`) answered `GET /session` with HTML and `GET /api/session` with 401 to an
  unauthenticated caller, so a recorded URL without the adapter's credential would not have been a way in.
  The `session_id` the row carries is the addressable fact.

## The session mailbox

`estigia sessions send` writes a `messages` row, `estigia session deliver` takes and stamps the pending rows, and
`before_agent_start` (Pi) and `PreToolUse` (Claude Code, Codex) call it. What this measures: store unit tests for the
send, the single-statement take and the two counts; pipe tests that drive the binary through send, list, deliver,
refusal and an ended recipient; one pipe test that drives the Claude Code and the Codex hook through the real
binary and checks the answer carries the mail once and decides no permission; one unit test that a refusal keeps the
mail waiting; and Node tests that drive the generated Pi extension. What it does not measure:

- **No runtime was run live.** The Claude Code and Codex hook answers were read from this crate's own output, not
  from a host. Whether Claude Code puts `hookSpecificOutput.additionalContext` of a `PreToolUse` answer in front of
  the model, and whether the Codex build in use reads that field from `PreToolUse` at all, are taken from the hosts'
  documentation and unverified. The Codex terminal UI and the Codex daemon were not driven: a Codex session that ignores the field
  receives nothing, and the row is nonetheless stamped delivered, because the binary cannot tell.
- **Pi's section lasts one turn.** The extension adds the text to that turn's prompt sections, which Pi rebuilds for
  the next turn, so a model that does not act on it in that turn does not see it again. Whether this Pi version keeps
  the section out of the stored conversation was not measured.
- **Beyond `PreToolUse`, only `Stop` delivers.** Claude Code's waiter and Codex's loop `Stop` take mail; a session that
  makes no tool call and ends no turn is not reached, on any runtime. `UserPromptSubmit` and `PostToolUse` are not
  registered by setup.
- **OpenCode was driven under `node` against stubs, not against a host.** Three Node tests drive the generated
  plugin: OpenCode 1's `tool.execute.after` appending to a string result, OpenCode 2's `execute.after` steering
  through a stub `ctx.session.prompt`, and the fallback to `ctx.session.synthetic` when that throws or is absent.
  Unverified: that OpenCode 1 puts `output.output` in front of the model, that OpenCode 2's `ctx.session.prompt`
  accepts `delivery: "steer"` mid-turn (the loop plugin uses the same call with `queue`), and that every OpenCode 2
  build has an `execute.after` hook. A result whose `output` is not a string is left alone and its mail stays pending,
  and on OpenCode 2 the mailbox is not read at all when the host offers neither `ctx.session.prompt` nor
  `ctx.session.synthetic`. A known loss window remains: when one of them exists and throws after the rows were taken,
  the rows are stamped delivered that the host refused, the text goes to standard error, and the sender sees them as
  delivered. No small fix exists, because the take and the stamp are one statement.
- **OpenCode costs one extra process per tool call.** The gate asks only for writes; the mailbox is read after every
  tool, and `estigia session deliver` answers from a read-only open of the store when nothing waits.
- **Claude Code, Pi and OpenCode 2 wake an idle session, each measured live once, by hand.** The OpenCode 2 wake
  ([`sessions.md`](sessions.md#waking-an-idle-opencode-session)) is driven under `node` against a stand-in host. It was
  also run once on OpenCode 2.0.28 with a private server (`--standalone`). Not measured live: a prompt the host refuses,
  and a session woken while the background service rather than a private server runs the plugin. OpenCode 1 is not woken, a session that never finished a turn is not polled, and a plugin loaded while a
  session is already idle learns of it only at that session's next terminal event. OpenCode's `generate` is not used, nothing in
  the plugin learns that a row was written, and Codex `turn/steer` (daemon threads) is not built. The Codex terminal
  UI and the Codex daemon stay undriven (above).
- **A delivery is stamped before the host acknowledges it.** A hook process killed between the stamp and the write to
  standard output loses the message as delivered. The window is one statement wide and was not provoked.
- **The Claude Code mail waiter was measured in one live session, not under load.** Claude Code 2.1.300 woke an idle
  interactive session within 15 seconds when an `asyncRewake` `Stop` hook exited 2, a new waiter was running
  afterwards, and none remained after the session was killed (see
  [`sessions.md`](sessions.md#waking-an-idle-claude-code-session)). Not measured live: a session idle longer than
  60 s before the mail came (the 3600 s `timeout` is the documented field, not a long idle run), and mail arriving
  while a turn is running (the `PreToolUse` delivery should take it first; if a waiter takes it instead, whether
  Claude Code delivers a rewake that arrives mid-turn was not seen). At most one waiter per run polls: each new waiter
  overwrites a token in `store_meta`, and a superseded one exits 0 at its next poll, within about a second, taking
  nothing. Two waiters starting at the same instant race on that write, and the loser retires; the unit tests drive
  threads, not two real hook processes. A waiter whose store cannot be read exits and leaves its rows pending, and
  mail the waiter took is stamped delivered before Claude Code acknowledges the exit, so a host that discards it loses
  it, as the other points do.
- **An already idle Codex session is not woken.** Codex's `Stop` hook is synchronous and continues the turn that is
  ending, so mail left for an idle Codex session waits for the end or the next `PreToolUse` of its next turn. Codex has
  a `Stop` entry only with the `Loop plugin` row `on`, so without it mail waits for `PreToolUse`. The blocking answer
  was exercised by unit tests on this crate's output, and the mail-plus-loop `reason` was not run against a live Codex.
- **"Read" is not recorded.** `delivered_at` is the observable event; no host reports that the model read the text.
- **The counts in `estigia sessions` belong to listed sessions.** A message whose sender has neither a registry row
  nor a run pointer is counted in the table and shown against nobody.
- **The MCP `send_message` door was driven live on one host only.** The unit tests play the client over an in-memory
  stream: capability absent, each refusing answer, allow once, allow for the session, a line arriving while the
  question is open, a closed input and a missed deadline. Live runs covered Claude Code 2.1.300 only: `claude -p`
  refused with no row written, and the interactive dialog's *Allow once* recorded a row that was then delivered.
  *Allow for this session* and *Deny* were not run live, and Codex and OpenCode were not run at all. A client that declares the capability and never renders the question waits out the 15 minutes and the send is
  refused. The grant is per server process and keyed by the sender's run id, which the caller names: an agent that
  names another run's id borrows its grant only if that run's id was granted in the same process.
- **The question shows the issue a run-id recipient holds from its run pointer, not from the store.** The pointer is
  not authority; only the resolution of an issue number reads the store.
- **A message is text from another session.** Delivery says so in its first lines; nothing stops a model from acting
  on it, and nothing here measures how the hosts' models treat it.
- **The `flow dev` conflict rule reads the words of the message.** `send_message` and `estigia sessions send`
  refuse a message whose text carries `flow dev <n>` while the recipient's pointer names another issue. The
  detection is textual, so a sentence quoting the instruction as something *not* to run is refused as well, and
  a differently spelled instruction (`/flow`, tabs inside it, a Linear key) is not detected at all. Both are
  unit tests over the tool and the CLI; no live model was sent a conflicting instruction to see what it would
  have done with it.
- **The handoff's label read-back is against a stand-in `gh`, and the two halves share one store only in
  production.** The store row, the `landed` and `deferred` answers, the projection and its two agreeing reads,
  the queue-then-flush path, the held-issue refusal, `SessionStart` and `list_sessions` are each driven by unit
  tests; the fake `gh` models the label edit in a file. No handoff was run against GitHub. The row is written by
  the tool through `store::beside(state_root)` and the label by the transport through `store::store_path()`,
  which are the same file in production and two scratch stores under `cargo test`; the explainability test seeds
  the transport's store directly, so the suite does not cross that boundary the way the binary does.
- **A handoff is never consumed, expired or read-stamped.** Every later session of that runtime in that
  repository is told about it again; whether anybody took it is visible only from the tracker, through `claim`.
  Nothing here measures a long-lived store's growth or a person's reaction to a stale line at `SessionStart`.

## The npm installer and the registries

`npm/bin/estigia.js` installs a release archive the way the scripts do, and `publish-registries.yml`
publishes crates.io, npm and the MCP registry from a tag. What this measures: four Node tests in `tests/npm.rs`
that drive the installer against archives served from `127.0.0.1` — a verified install that records, places and
runs, and a second run that fetches nothing; a mismatched sum, a missing sum and a refused record that place
nothing — and static tests that hold its target table, the three version fields, the workflow's `v*` input guard and the
pinned `mcp-publisher` to the files they describe. What it does not measure:

- **The Node tests run on Unix only.** The stand-in binary is a shell script, so the Windows branches — the
  `System32` `tar.exe` that reads the zip, the `%LOCALAPPDATA%` directory, replacing an `.exe` another
  process holds open — have never run.
- **No registry was published to.** Trusted publishing, the idempotence probes and `mcp-publisher` run only
  on a tag, and no tag has been cut; `server.json` was not validated by the registry.
- **The tag-against-version steps in the `crates-io` and `npm` jobs are asserted by nothing.** Removing them
  leaves every test green; `cargo publish` and `npm publish` would then publish whatever version the
  checked-out manifests name.
- **The installed-version check reads `--version` text.** A binary at the install path that answers the
  package's version is run without its bytes being checked against the release.

## The release builds; CI tests

Since #135 `release.yml` runs no `cargo test`: the suite runs in `ci.yml`, whose `check` matrix covers every runner
a release archive is built on (`every_runner_a_release_builds_on_is_one_ci_tests_on`), so a flake fails a pull
request instead of a tag. Measured before the change, on the `v0.1.0` and `v0.1.1` tags: about 50 minutes per
release, the x86_64 Windows job alone, and one release (`v0.1.1`, run 39043362083) failed on a bash timeout in a test
that passed on the same target the run before. What the change gives up:

- **The release profile is tested nowhere.** `ci.yml` runs the debug profile; `lto = "thin"`, `codegen-units = 1`
  and `strip` are compiled by the release build and executed by nobody before an archive ships.
- **`x86_64-apple-darwin` is built and never run.** It is cross-compiled on the arm64 `macos-latest` image; no
  runner executes an Intel macOS binary, and Rosetta on the arm64 image was not tried.
- **The tagged tree is assumed to be the tested tree.** A tag is cut on the merge of a pull request CI passed;
  nothing checks that the base did not advance between the green run and the merge beyond the delivery rules a
  merge already follows.
