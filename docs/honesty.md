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

  **Restating the decision gate is held mechanically in one spelling; the rest is held by review.** A
  guard that counted workflow states per source line was withdrawn: it measured a line while the rule is
  about a statement, and reddened on correct prose that enumerated the state vocabulary. Two tests hold
  something. `every_named_disposition_uses_a_transport_state` crosses the Decision Gates table against
  `STATES` and proves every state is *reachable*, **not** that any condition routes to the right one
  (swapping two destinations leaves the suite green). `the_decision_gate_is_not_restated_as_a_second_table`
  holds an arrow followed by a backtick in the four companion documents and a table in
  `repository-delivery.md` whose header cell reads `| Destination |`; **renaming that header and keeping the
  pointer sentence restores the whole table with the suite green**. A restatement in prose with no arrow
  passes, and `SKILL.md`'s own steps restate the routing outside the section the parse is bounded to
  (`rg -n 'analysis|ready|in-progress|review|blocked|done' skill/SKILL.md`; their number is deliberately
  not written here).
- **One review-protocol readback is held by no test, and one is held only by a string search.** The
  protocol's guards were mutated one at a time. Held by a test that goes red: all three enforcement points
  of the requester exclusion (`claim`, `reclaim`, the review queue), the queue's fail-closed candidate
  read, the verdict's distinctness rule on *both* halves, its live-claim requirement, the receipt's
  exactness on each write path **before** it writes, the CI-release gate, the comment escaping, the
  read-side requester filter, and every field-shape validator on the four markers. **Many smaller ones are
  not, and this entry does not claim to have found them all**: `handoff_review`'s post-release readback can
  be disabled with the suite green; its retry-path receipt recheck is caught only by
  `the_compound_handoff_records_before_release_and_checks_review_afterwards`, a source-text search that
  catches deletion and not the call being made with its answer ignored; and an adversarial sweep of
  forty-nine mutations found a further score of defensive checks (replay-path repeats of rules whose
  first-write copy *is* held, operation-id and marker-field validators, idempotency and visibility
  readbacks) removable with the suite green. A mutation harness that rewrites line endings produces failures
  that look like a guard being caught, because tests here anchor on exact whitespace in `claim.rs`.
- **Four stops around `ensure_draft` and the closing-keyword scan are unheld.** The `[world-action]`
  guidance naming `Refs #<n>` is held by no test. `Answer::already_wrote` decides which exit-code arm
  `translate` takes and sits outside the `exit-code` population fingerprint, so that arm's `StatusRequired`
  axis is held by the fingerprint alone, a tripwire that says *go and read this*. The commit-range scan is
  one function whose two callers read different checkouts (`publish_review` the isolated one,
  `assess_autoclose` the repository root); unifying them changes which tree a standalone
  `check-closing-keywords` inspects, a decision rather than a tidy-up. `keyword_sources` carries two shapes
  under one key; nothing parses it today. The scan reports every reference, not only to the delivered issue:
  a pull request body saying `close #464` closed #464 on merge with neither scan objecting, so the pre-push
  refusal takes any reference and `assess_autoclose` carries the others as `other_issue_keywords`. What
  remains unread: `assess_autoclose` returns before it scans when the delivered issue has no closing pull
  request, so a body closing another issue from a pull request that does not close the delivered one is
  refused before the push and not by the standalone check.
- **The follow-up's single-sourcing is named, not observed.** `mandatory_follow_up`
  (`src/transport/commands.rs`) and the branch-link follow-up (`src/transport/claim.rs`) both interpolate
  `closing::FOLLOW_UP`, and each call site's test compares the emitted text to the constant's value rather
  than its source: re-inlining a byte-identical copy at the `check_closing_keywords` site leaves both green
  (measured). What is held is the transition: deleting `transition --to done` from `FOLLOW_UP` reddens
  `follow_up_names_both_the_transition_and_the_release`.
- **Nothing enumerates the refusals that write before they refuse, and nothing can.**
  `Answer::already_wrote` decides which arm `translate` takes, and a refusal earns `Committed` only by
  carrying `"world": "committed"` in its payload. Whether a stop is reachable *after* a write is a property
  of the interleaving (a marker is posted, a later read comes back without it), not the source text, and one
  function holds stops on both sides of its own write, so a walk cannot place a stop relative to a write and
  a hand-kept list would be a second copy of the control flow. What holds each repaired site is a test that
  drives the decision to the refusal and reads it through `translate` (`lost-claim-race`,
  `lost-reclaim-race`, `claim-operation-no-longer-current`, `reclaim-operation-no-longer-current`,
  `ownership-changed-projections-repaired`, `holder-runtime-missing`, `ownership-kept-changing-before-projection`
  and `ownership projection write did not read back exactly`). Sites that still answer `NotStarted` over a
  marker on the timeline, none shown unreachable: `claim-operation-expired` (both readings),
  `claim-metadata-mismatch`, `reclaim-metadata-mismatch`, `unassign-metadata-mismatch`, the
  operation-conflict family (`claim-operation-conflict`, `reclaim-operation-conflict`,
  `unassign-operation-conflict`, `operation-id-kind-conflict`), `resumed_release`'s two stops and
  `unassign`'s post-write `held-by-other-holder-disappeared`. Measured cost: on another repository's #318 a
  run was told twice that nothing had been written, and left two control markers on an issue that no verb
  reaches.

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

  **The shell road folds punctuation, and the fold was reached by nine separate fixes.** `surface_of` builds a
  view of the command and matches fragments against it; anchoring needs a separator in front of a fragment,
  and each attempt to supply one missed a spelling the shell allows (an operand after the verb; quoted
  operands, redirects with no space and operands joined to a long flag; a short option with its value attached
  such as 7-Zip's `-oDIR`; brace expansion; `mv .estigia ..`, where joined tokens let the parent-segment
  collapse cross an operand boundary; `%~dp0` and the `cmd.exe` `^` escape; a suffix ladder that concatenated
  rungs into a path nobody wrote, then one unbounded by the marker's segment, then one read off a different
  string than the matcher's, so `${P:-.claude/settings.json}` was missed). **Five of the nine were introduced
  by the fix for the one before it**, and the suite was green over every loss because each fixture spelled its
  commands the way the last fix had taught it. A tenth is likelier than not: the fixtures hold the nine
  spellings, the eighteen folded characters (spelled in the fixture and crossed against the constant) and the
  ordinary-line direction, not the space of spellings, and no property test over generated command lines
  exists (`punctuation_does_not_hide_a_control_surface`,
  `a_drive_relative_spelling_is_measured_like_a_relative_one`,
  `a_relative_operand_is_measured_like_an_absolute_one`,
  `a_later_operand_does_not_collapse_an_earlier_surface`; the other direction by
  `folding_punctuation_does_not_gate_an_ordinary_line` and `an_option_prefix_does_not_gate_an_ordinary_line`).
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

  **`setup` and `sync` reach further than a redirected `HOME`.** A judge under `measuring` ran `estigia sync
  --allow-source-build` with `HOME` and `USERPROFILE` pointed at its scratch directory and the run rewrote
  two files in the operator's real profile: the `gemini-cli` root resolves under `%APPDATA%`, which neither
  variable governs, and the skill path, MCP command and `BeforeTool` hook were repointed at a binary in a
  checkout that gets deleted. `Environment::resolve` now takes an inherited `APPDATA` or `XDG_CONFIG_HOME`
  only when it resolves under the same home `HOME`/`USERPROFILE` supplied on that call (`paths::covers`,
  resolving each side through its nearest existing ancestor); a value that fails is dropped for the isolated
  home's fallback, with a note in the per-adapter output and never a refusal. **The gate does not apply this
  rule**: `harness::roles::definition_for` reads `XDG_CONFIG_HOME` raw and pushes `~/.config/opencode/agents`
  *and* the relocated root, which nothing tests. **The substitution costs three cells**: a genuinely
  relocated `%APPDATA%`, an `XDG_CONFIG_HOME` that legitimately sits outside `HOME`, and a `HOME` exported to
  a path other than `%USERPROFILE%` while `%APPDATA%` never followed it (compared against
  `paths::home_dir()`, which prefers `HOME`). All three get the home-relative path, so `setup` writes the gate
  registration where the host does not read it and reports success (a wrong path worse than a missing one;
  the step from the substitution note to *my gate is inert* is the operator's). A refusal was rejected
  because a process reading `APPDATA` cannot tell a relocated `AppData` from a redirected `HOME` that forgot
  to move it. Gemini CLI and Crush are retired, so the cost stands for OpenCode and for `uninstall` on a
  machine still carrying the others; three is what was enumerated, not proved to be all, and nothing wider than
  the operator's one machine is measured (`docs/what-it-writes.md` states the choice). **`uninstall` reads the
  same substitution, with no route back to the real path**: the `BeforeTool` hook and MCP entry in the real
  `%APPDATA%\gemini\settings.json` and the real `GEMINI.md` stay live, and the substitution note is the only
  thing that says so.

  **The test-only environment lock claims writer exclusivity, not reader exclusivity, and that gap is not
  closed.** `src/test_env.rs`'s `ENVIRONMENT` mutex serialises the *writers* and the readers that opt in
  through `with_environment_read_locked`; unlocked readers of `HOME`, `USERPROFILE`, `APPDATA` and
  `XDG_CONFIG_HOME` exist elsewhere (the ones a hand check found are named in the `// SAFETY:` notes, not
  claimed complete, among them `harness::roles::definition_for`'s call sites in `harness::roles::tests`), so
  `set_var`'s precondition is **narrowed and not met**, and Rust 2024 makes `set_var` `unsafe` for exactly
  that. The race was real (three failures in fifteen `cargo test --lib home -- --test-threads=8` runs, closed
  for that picker test), no number of green runs closes a scheduling window, and nothing checks for a new
  unlocked reader but the `// SAFETY:` notes, by hand.

  **One row, one file, and a machine can hold two answers.** `Evidence standard` is `Scope::Everywhere`, so
  it is set per repository, while the artifact it decides is a single user-level `review-blind.md` rendered
  from whichever checkout last ran `setup` or `sync`. On a machine holding one `reading` and one `measuring`
  repository, the drift `docs/configuration.md` frames as lasting until the next `setup` or `sync` is
  permanent for whichever was not last written. The gate answers per repository and narrows on an unreadable
  contract, so this costs a capability rather than granting one.

  **Still open.**

  - No derived directory family is compiler-forced: `skills_root_fragment` matches `SkillsRoot`, whose
    variants a new adapter reuses. The `_` arms on the instruction enum are in the previous section.
  - **The roles a run's delegated contexts ran as are recorded, within the run and durably.** The gate writes
    each distinct `agent_type` it sees into the run pointer (per-session, removed at `SessionEnd`, read back
    by `SessionStart` on a resume) and into an append-only `roles` table (`at`, `run_id`, `agent_type`,
    `repo_dir`, `issue`) **at the moment it sees the role and independent of the decision**, so a call answered
    `Outside` is recorded where `note` drops it; `estigia roles` reads it back by `--run-id` and `--issue`. A
    launch that named a `model` or `effort` records them: **a declaration, not a check**. It proves the gate
    observed a named `agent_type` on a tool call by a run, and the observation survives the run. It is **not a
    count**, **not an isolation proof**, and **does not prove a panel ran** (a host that never sends
    `agent_type` leaves it empty, which is not evidence that nothing was delegated). A context refused at the
    role gate before any call is not recorded; a call the *claim* gate denied is. **Under the default row the
    reserved reviewer is never recorded**: its grant is `Read, Grep, Glob`, none of which is in the matcher the
    hook is woken for (`Edit|Write|MultiEdit|NotebookEdit|Update|Bash|Agent|Task|AskUserQuestion`); measured
    through the built binary, under `reading` a `review-blind` context calling `Bash` is denied and no pointer
    is written, under `measuring` the same call records `roles: ["review-blind"]`. The store fires on a **new
    role** as well as an allow, so a denied call writes a pointer (also under the `<runtime>-unknown` name a
    blank `session_id` produces, which `SessionEnd` does not remove) and, because `gate` calls `mark_verified`
    once the tracker answers, persists that watermark, so the next routine write can ride the window without
    asking again. The pointer's role write goes through `session::store`, which drops a stale write whole when
    another writer moved the revision first (a role can be lost to a race with a tool write; the append-only
    log has no such race).
  - **`Run.inline_writes` (`src/harness/session.rs`) records writes by the run's own context and is still only
    a record.** A target is added when `agent_type` is absent on a call the gate allowed, once `roles` already
    held an entry this session (a path for a write tool; the matched fragment, verb or tool for a shell write;
    the surfaces that name it say **target**, never file). Absent means a missing field, not a blank one, since
    a blank `agent_type` from inside a sub-agent would claim this run's own context wrote a delegate's write.
    It is within-run only, empty on a host that never sends `agent_type`, silent for a call the role gate
    refused, and **a run that never delegates reports nothing, not a floor** (`roles` starts empty, so the
    earliest inline write is indistinguishable from a host that does not report `agent_type`). A write outside
    the claimed checkout is `Outside` and not counted; a write that cannot be placed, and every relative path,
    is read as inside. **A gate cannot judge "non-trivial"**: nothing refuses an inline write whatever
    `SKILL.md`'s Direct Work Delegation table says crossed it, because a changed-file-count proxy over-fired on
    this repository's own history. It says nothing about which model wrote them.

  - **A write to `.git/estigia/estigia.local.md` is a boundary whichever row it names.** `Evidence standard`
    is `Scope::Everywhere`, so a repository row lives in that file, a path `CONTROL_SURFACE` does not carry.
    A per-row reading of the write's payload was tried and rejected six times by blind judges, and the
    reason is the finding: `crate::config::rows` reads a **region**, and an anchored write can move that
    region's boundary (deleting a line break between two rows, assembling a fence marker at the junction of
    a `new_string` and the preserved tail or across two `MultiEdit` entries, a marker appended past a
    resolvable label, a `Write` that replaces the file whole and erases every guarded row it does not
    carry) using text that never names a row; a per-payload reading cannot have a property of the *document*
    that results. So `is_repository_configuration` answering true is the whole test on the write-tool road;
    a shell write gets the answer a shell write to any control-surface file gets, and `chmod`, `chattr` and
    `chown` ask `names_a_boundary` for the same parity. `REPOSITORY_SHELL` used to answer before either
    question and shadow this file and the control surface alike; its arm now asks both (`git commit -m wip
    && printf x >> .git/estigia/estigia.local.md` is `Boundary`), and `IRREVERSIBLE` is still asked before
    it. The cost is real: `Repair ceiling` (retired, kept in tests as the string of a row the gate never
    reads) costs one tracker read like every other row, so the criterion that an ordinary `Repair ceiling`
    write not become a boundary is not met. `estigia config set --repo` writes the file directly and is
    untouched; whether a row may be repository-scoped at all is `docs/configuration.md`'s *Which root the
    gate decides in*. A directory-shaped removal is named by neither surface: `rm -rf .git/estigia` and `mv
    .git/estigia .git/estigia.bak` answer `Routine` (neither spells `estigia/{LOCAL_FILE}`, no
    `CONTROL_SURFACE` fragment reaches the bare directory, no `REPOSITORY_SHELL` fragment is a substring).

    **The population that newly pays is wider than a write.** The `REPOSITORY_SHELL` predicate is asked of
    the whole command line, so a `git` command that merely *names* a control surface costs a live tracker
    read (`git commit -m note about .claude/settings.json` once answered from the renewal window and now
    answers `is held by …`). `CONTROL_SURFACE` carries twelve separator-free entries, seven of them
    instruction files (`claude.md`, `claude.local.md`, `agents.md`, `agents.local.md`, `gemini.md`,
    `qwen.md`, `crush.md`), so `git commit -m update CLAUDE.md` is in that population. The direction is
    fail-closed (the asymmetry `WRITES_A_FILE_POPULATION` declares); nobody has measured how many ordinary
    commits name one of those filenames, and each costs a round trip. In process it costs microseconds
    (`git stash` about 4.6 µs before and 29 µs after in a debug build; a long commit message with many
    `-`/`~` words about 39 µs and 470 µs) against about 75 ms for `estigia gate Bash` end to end; the suffix
    ladder is unbounded, though an `rm` with the same operand always paid it in full (1.70 ms on 832 bytes).

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
  file: a redirect, a `sed -i`, an interpreter handed code, the utilities that copy, move or delete (also
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
  An operator sets thirty rows and `config list` reports all of them. Measured: `context.get` is
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
  Each row's own function is tested, and the **assembly** into the report is crossed too (downgrading every
  `Broken` of one row family in `doctor::full` once left the suite green for ten of eleven rows). All rows
  can break, and all but two are forced through the binary in `tests/pipe/status_doctor.rs`'s `FORCED`
  list; a row added later fails until it is forced or says why not. The exemptions:
  - `stale-run-pointer` needs a `gh` that answers `issue-not-open` for one specific issue number;
    `harness::doctor`'s own module drives `stale_run_pointers` through a scripted `gh`
    (`a_readable_stale_pointer_is_reported_and_names_the_release_command`,
    `a_readable_live_pointer_is_reported_fine`, `a_tracker_read_that_fails_is_reported_as_unread_not_stale`).
  - `gate-version` is forced against a checkout sharing no history with the binary, but that state is not
    its *broken*: `git merge-base --is-ancestor` cannot answer, so the row reports `skipped`. Genuinely
    `Broken` on a real ancestor pair is `gate_version_reports_fine_and_genuinely_stale_against_real_commits`,
    which may execute nowhere on a given run (it skips on a dirty build and when a shallow clone lacks the
    binary's commit, which is what CI's checkout gives it). What holds the `Broken` decision
    unconditionally is `an_older_binary_than_the_checkout_is_reported_broken_with_a_reinstall` against
    `gate_age_verdict`, fed `Some(false)`, so it cannot see which end of the ancestry the call site asks
    for; in those two conditions a swapped argument would report a current gate broken and a stale one
    current.

  **A build that stops carrying its commit degrades to *cannot tell* everywhere, and nothing fails.** A
  `build.rs` regression that stopped emitting `ESTIGIA_COMMIT` leaves the suite green (`VERSION` becomes
  `0.1.0 (unknown build)`, the end-to-end stale test early-returns, the pipe fixture takes its no-commit
  arm). The failure is honest (every row answers `skipped`), and a *corrupted* commit is caught on a clean
  build; what is unheld is the absence. `what_status_prints_is_what_somebody_needs_to_act_on`,
  `what_the_companion_verb_prints_is_crossed` and `somebody_elses_hook_is_not_this_agents_gate` read the
  assembled pages and the ordinary machine (an operator's own `PreToolUse` hook beside no Estigia entry)
  where `is_estigia_hook` saying yes to every entry would print `gate on` over no gate.
- **Estigia cannot check who reviewed.** The contract requires that a review *"MUST NOT be performed by
  the context that wrote the change"*. The harness reads who recorded a computed verdict, and will not let
  the publishing run or a run that asked for review resolve its own handoff or supersede its own
  escalation. Where the reviewing run recorded its own verdict that is timeline attribution; where the claim
  holder recorded somebody else's, it is a name that run supplied. The judgement belongs to the agent.
- **Refusals about a tracker or host with no transport: only the tracker's can fire.** `Tracker` has two
  variants and `Tracker::transport()` answers `None` for Linear (read-only, admitted only under `Workflow:
  assisted`); `RepositoryHost` is GitHub-only, so the host-bound branches in `src/harness/mcp/mod.rs` are
  unreachable while only GitHub is implemented and are kept: a refusal that cannot fire is inert and
  harmless, while a reported row that cannot vary is noise, and nothing distinguishes a guard kept for that
  reason from one nobody noticed had died except this paragraph.
  `every_reader_of_a_transportless_tracker_asks_the_same_function` holds a floor alone (at least four
  production call sites ask `Tracker::transport()`); its second half, a text search for a literal
  tracker-name pair, was deleted because it became unfalsifiable when Linear and Trello were removed, so a
  hand-rolled second decision about a tracker with no transport is caught by nothing. The test's own doc
  comment still says `Tracker` is down to one variant, which is no longer so.
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
  four.** The rule: an automatic mode approves the host's own prompts, and Estigia's decisions are never
  among them. Checked on 2026-10-14 in a scratch repository with no claim held and `Issue requirement` at
  `ask`, asking the model to write `hello.txt`: OpenCode v2.0.26 under `opencode run --auto` and Claude Code
  2.1.296 under `claude -p --permission-mode bypassPermissions` both refused `issue-required` and created no
  file, and Pi 1.1.0 has no permission prompts at all. **Codex 0.159.2 was not run** (the local install was
  not signed in, `401 Unauthorized`), so nothing says what Codex does under `--full-auto` or the `never`
  approval policy. What the suite pins is the part Estigia owns: with the mode's fields in the hook payload
  (`permission_mode` of `bypassPermissions`, `auto`, `dontAsk`, `acceptEdits`; `approval_policy` of `never`)
  the answer is byte-identical to the answer without them (a deny under `required`; under `ask`, an `ask` on
  Claude Code and a deny on Codex), and the generated OpenCode plugin throws on exits `1` and `2` with
  automatic-mode fields, in both loader generations, and registers no permission hook. What no test can
  pin is the host's own ordering: a release that put its permission layer before hooks or plugins would turn
  a refusal into an approval. Under `ask` the live run saw the file not created but did not separate a host
  that put the question to nobody from one that refused it. The `permission` row of `doctor` reads the
  setting from the host's settings files only (`permissions.defaultMode` for Claude Code, from the
  project's `settings.local.json`, then `settings.json`, then the user's; `permission` in `opencode.json`;
  `approval_policy` and `sandbox_mode` at the top level of `~/.codex/config.toml`), so a mode chosen on a
  command line, a Codex profile, a managed policy file or an environment variable is invisible to it, and
  `no permission mode is set` means the files say nothing, not that the host runs its default. A file that
  does not parse is reported as unread. Other hosts get no row.

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
- **The embedded identity's `dirty` flag can go stale between builds, and only that half.** `build.rs` emits
  `rerun-if-changed` for itself, the `.rs` files under `src/transport` (except `tests.rs`) and the git files
  whose change means the *commit* half is stale (the per-worktree `HEAD` and `packed-refs`); emitting any
  disables Cargo's default rerun, so an edit elsewhere recompiles the crate while Cargo replays the cached
  `ESTIGIA_COMMIT` and `ESTIGIA_DIRTY`. That closes the commit half (deleting `watch_git_identity(&root);`
  leaves the suite green; only a hand measurement holds it) and not the `dirty` half: editing a tracked
  file's *content* without committing changes `git status --porcelain` without touching anything watched, so
  the next unrelated rebuild still reports the tree clean and the row prints `ok  gate-version this build (C)
  is current against the checkout`, a pass spoken plainly for a state it cannot see. The dirty read uses
  `--untracked-files=no`, a narrower claim than "compiled from" (`build.rs`'s `rust_sources` walks the
  filesystem under `src/transport`, so an untracked `.rs` file there is folded into `TRANSPORT_VOCABULARY`
  while the tree reads clean; no test holds it). `capture_identity`'s `git_query` returns `None` for both
  "failed" and "empty", so a build where `git rev-parse` succeeds and `git status` fails embeds
  `ESTIGIA_DIRTY=0`; distinguishing them needs `dirty` to carry three states.

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
  `answer_text` (`src/harness/mcp/mod.rs`) inserts a `build` object (`commit()` and `dirty()` from
  `src/build_info.rs`, read directly, not through `gate_age_verdict`) into a successful tool answer whose
  body is a JSON object, unless the call passed `compact: true`. `gate_version`'s `Health` has two callers,
  `doctor`'s `gate-version` row and `hook::session_start_response`, which renders it into a sentence at
  `SessionStart` when it is not `Fine`; it reaches no tool answer, and a refused call reports no commit and
  no dirty flag, so a caller who only sees refusals cannot tell which build answered. The note is computed
  once from one `gate_version(&context.repo_dir)` call when the hook fires, so a checkout that commits,
  pulls or switches branch afterwards leaves the agent holding a verdict about an earlier state. It speaks
  on every session it cannot compare and carries nothing to act on (a dirty build or a tarball install with
  no embedded commit reaches a `Skipped` arm), and **the `Broken` arm is not rare**: `Fine` needs the
  checkout's head reachable from the build's commit, so a clean build installed from `main` on any branch
  carrying a commit the binary lacks takes `Broken`, and every feature-branch session after an install from
  `main` hears it. **The note names no command, by decision**: `CLAUDE.md` says a message names no command
  unless running it clears the block, and a reinstall does not clear this one in-session (the tools pick up
  a reinstalled binary only after the session restarts), so the `Broken` note says plainly that the block
  clears only after reinstalling and restarting, the `Skipped` note names nothing, and no `Resolution` field
  is added to either (overriding the ratchet in `src/harness/doctor.rs` that wants one on every failing
  check). The comparison is against the **local checkout's `HEAD`** (`GateContext::repo_dir`), never a
  remote's default branch: production code reads `@{upstream}`, `symbolic-ref --quiet HEAD` and `ls-remote
  --heads origin` for a caller-supplied name, none of which resolves a repository default branch.

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

  **A single holder is not reconciled, on purpose.** A single holder whose issue is closed by a close this
  harness cannot recognise as the review's registered delivery still refuses every write in its checkout
  with `issue-not-open`. Folding it into the loop would **erase** that refusal rather than repeat it:
  dropping it gives zero holders, the `0 =>` arm answers `Outside(NothingSworn)` unless something there
  denies first (`complete_review_receipt_not_selected`, `run-pointers-unreadable`), and only
  `Outside(NothingSworn)` replaces `gate`'s refusal. Changing `holders.len() >= 2` to `>= 1` reddens
  `a_single_stale_holder_still_refuses_through_its_own_issue_not_open`. A close this harness *does*
  recognise (the closing PR the newest confirmed publication names, and a merge whose second parent is the
  reviewed head and whose first parent descends from the reviewed base: `src/transport/claim/close.rs`)
  makes `verify_claim` answer `closed-by-own-delivery`, so `holder_standing` counts the pointer `Live`.

  **`doctor`'s `stale-run-pointer` row has three buckets**: `issue-not-open` (named with the `estigia
  release --run-id` command), `Passed` with an own-delivery close (`stranded`, via `Answer::standing()` and
  the token `OWN_DELIVERY_CLOSE_PREFIX`, one spelling bound by `OWN_DELIVERY_CLOSE_SPELLING_POPULATION`) and
  an answer the row could not read (`unread`). `Fine` is returned only when all three are empty and says how
  many pointers it read as still open and held by this run
  (`the_fine_sentence_claims_only_the_buckets_the_row_empties`). An aged stranded pointer reaches `Fine`
  (`reduce_ownership` moves a lapsed acquisition into `stale` with `holder: None`, so `verify_claim` answers
  `not-current-live-holder`, which the row treats as neither stale nor stranded nor unread). The predicate
  that routes the population is exercised
  (`own_delivery_close_reads_the_transports_own_prefix_and_nothing_else`); the end-to-end `stranded` arm is
  not, since no fixture produces `delivery_close`. **A blind spot, plausibly the commonest dead-pointer
  shape:** `same_git_repository` fails closed to `false`, so a pointer whose recorded checkout no longer
  resolves as a git repository (deleted, moved, the removed-worktree phantom) is excluded from the row:
  not stale, not unread, not named.

  **Two dead pointers with nothing live covering the same checkout reconcile all the way to zero, and the
  gate stands aside, while one dead pointer still refuses.** A live holder survives reconciliation however
  many dead pointers sit beside it, so this is the all-dead case; `holders.len() == 0` answers
  `Outside(NothingSworn)` when `session::unreadable_holdings` is also empty, so a machine with two phantom
  pointers for one checkout is *more* permissive than one with a single live-looking one. That is a
  consequence of the guard: a checkout two closed claims left behind is one nobody currently holds.

  Two or more holders reconciliation cannot separate, because the tracker says both are live, are the common
  case (`start_branch` gives each run its own worktree without narrowing what the claim covers, so two
  healthy runs both cover the shared base checkout). `not-current-live-holder` is the only standing
  `estigia release --run-id <run-id>` is offered for, and it covers three timelines (no acquisition for this
  run; this run's own acquisition gone stale past its horizon; a live acquisition that lost a claim race). A
  pointer's `worktree` field was tried first and was wrong in both directions (`PointerEffect::Swear` writes
  none until `PointerEffect::Isolated` runs after `start_branch`). When nothing among the survivors is
  `not-current-live-holder` the resolution names no command, since no CLI verb isolates a checkout (that
  tool is MCP-only). `plan_release` searches `ownership.live` chained with `ownership.stale`: not found
  answers `nothing-to-unassign` and only the local pointer goes; found, `release` ends it with a genuine
  tracker write under the operator's identity, only once that pointer's recorded repository is the one
  `release` ran against. **The cost:** with two or more pointers on one checkout, every gated action pays one
  tracker round trip per named holder plus the survivor's own `gate` call unless it is a `Routine` write
  inside the renewal window (two to three reads), above `standdown::over`, so a hung `gh` is not a door
  `estigia stand-down` can open past; no fixture drives it. A `git push` narrows on the pushed refs before
  the loop (see "Nothing adjudicates what a push is aimed at"), so a push naming exactly one holder's own
  branch pays nothing and skips the `ClosedIssue` filter
  (`narrowing_before_the_tracker_can_deny_what_reconciliation_would_have_allowed`).

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
  `skill/SKILL.md`'s Developer step 6 names merge, tag, renew, transition to `done`, record evidence, close
  *and release*, but that is prose, and no code observes that a run which reached `done` still holds a
  pointer. `transition`'s `PointerEffect::Moved` calls `run.mark_verified()` only when the call names the
  issue the pointer holds, so a `transition --to done` not followed by release restarts the renewal window
  (the stranded pointer looks *more* live), and `release_ci`'s `PointerEffect::Renew` fills `run.issue`,
  `run.repo_dir` and `run.worktree` when absent, sets `review_receipt`, clears `reviewed_head` for a complete
  receipt and calls `mark_verified()` too; neither forgets the run. **A pointer left behind by a delivery
  that followed the rest of step 6 reads as live to everything that would otherwise clear it**: a recognised
  delivery close makes `verify_claim` answer `closed-by-own-delivery`, `Answer::stopped_reason()` is `None`
  for anything but a code-1 answer, `holder_standing` classifies it `Live`, reconciliation cannot drop it at
  any holder count, and the single-holder `gate` call does not refuse it; only `doctor`'s `stranded` bucket
  names it. The next run to claim there makes two holders, neither droppable, and
  `several-runs-hold-this-checkout` refuses that run's first write (measured). `release` removes the
  delivering run's checkout on exactly `delivery_close`'s proof
  (`releasing_a_delivered_issue_puts_its_checkout_and_branch_down` and
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down`, `tests/pipe/part_09.rs`, read the
  directory, registry and local branch back); the removal is best-effort and self-reported on the shipped
  path, and `a_single_stale_holder_still_refuses_through_its_own_issue_not_open` covers only the stranger
  population, so a single holder whose issue closed behind a recognised delivery is exercised by no guard
  test. A delivery that skips release still reports success.

  **Naming the release in step 6 costs the `dev:<runtime>` label on every delivery that follows it.**
  `release`'s `unassign` ends through `converge_ownership_projection` (`src/transport/claim.rs`): with no
  holder left, every `dev:<runtime>` label is emitted as `--remove-label` from an inline
  `BTreeSet::difference`. This is not measured: `src/transport/claim/tests.rs` exercises
  `converge_projection`, a helper with zero production callers, and disabling the production emission leaves
  the whole suite green. A query grouped by `dev:<runtime>` no longer shows which runtime delivered a closed
  issue; only the run-id in the close evidence survives.

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
