# Review models, targets and panels

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the reviewer routing, the review target and the panel do not establish. The index says which area holds which question.

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

  - **A pull request review relays too, and its slot carries no loss record.** Under a `pr_review`
    claim the relayed slot is marked in its job directory and found by `estigia review prompt` and
    `admit` when no issue's job matches (`a_relayed_panel_waits_for_its_answers_and_admits_them_against_the_claim`).
    A relayed slot that passes its deadline unanswered is abandoned and records nothing in
    `relay_losses`, so the selection order does not change for that run; the generated plugin was not
    driven against this road.

  - **A recorded relay loss changes the order for that run, and records one abandoned job.** When a
    relayed job is collected without an answer (`capture-runner-lost`), `collect_with` records the loss
    against the run in the local store's `relay_losses` table, and [the selection rule](what-it-writes.md#the-opencode-review-relay-plugin)
    then takes the CLI-first order for that run, asking an installed `claude` or `codex` first. It
    proves one job ended without an answer, not that the host cannot relay: a foreground relay that
    crashed records the same loss, and a lost CLI job records none. The marker is per run, not per
    host, so a later session on the same machine starts from the relay again.

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

- **The CLI road and the relay establish different things, and `review admit` serves only the
  relay.** A job launched for a sealed `claude` or `codex` is answered from that process's own
  `result.json` by `collect_review_input`; `review admit` refuses it `capture-not-relayed`, so the
  publishing run's shell cannot hand in an answer, an empty-findings one included, for a reviewer
  still working. What the CLI road establishes is unchanged: a process boundary and a tool-less
  grant, not an identity, over a diff rebuilt from the receipt. What the relay establishes is
  narrower still: a host context was handed the exact prompt and returned an answer naming the
  job's secret, which `review prompt` writes into the prompt it materialises — derived from a
  per-store key and the job id, created on first use in `store_meta` and never written into the
  job directory — so an answer composed without reading that prompt is refused
  `capture-secret-missing` or `capture-secret-mismatch`. `estigia review prompt` and
  `estigia review admit` are refused `review-relay-is-the-reviewers` to a run that holds a
  complete published receipt, so the publisher cannot ask for the secret of its own slots; the
  reviewer's own context holds no receipt and is not refused. It still proves nothing about which
  model answered, whether the host routed to a distinct context, or whether the reviewer read the
  diff. **The local store is readable from the same run's shell.** The secret lives in
  `~/.estigia/state.db`, which no refusal protects — only the stand-down record is denied — so a
  run that opens the store and computes the digest for a job id obtains what `review admit`
  requires. The mitigation bounds the one-line road the issue describes; it is not a signature, and
  nothing measures whether a run opened the store.

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
  **A result is the runner's only if its lock has been released and it carries the launch's token.**
  `job::job_status_with` reads the lock first, so a `result.json` written beside a running job answers
  `Running` rather than an answer, and `check_job::status` is the same function. A launch mints a
  32-byte token in `job_tokens`, hands it to its runner in `ESTIGIA_JOB_TOKEN`, and the collector
  compares the token the result carries with the store's, refusing `capture-result-unbound` a result
  that does not match; a job no launch of this binary recorded a token for — a rendering job, or one an
  older binary started — is read as before. Two launches of one job id in one second compute the same
  directory from `job_id`, and they now share one token: `mint_job_token` records the first and answers
  it to every later launch of that id, so whichever runner takes the lock last writes a value the store
  still holds (`a_second_launch_of_one_job_reads_back_the_token_the_first_minted`,
  `both_launches_of_one_job_are_admitted_by_the_collector`). What that does not give is two answers: the
  launches still share one directory and one `result.json`, so a launch whose runner never writes leaves
  the earlier attempt's cleared directory behind rather than a result of its own. A job directory written again — a job id that recurs, as a
  resumed claim's does — is cleared first: `write_job` deletes the previous attempt's `result.json`,
  `started`, `usage.json` and `relayed` marker, so the new launch cannot read the dead attempt's answer
  as its own, and `check_job::write_job` does the same for a checks job. **Not proven:** the token is
  stored in the clear, so a
  reader who can open `state.db` with `sqlite3` can read it and forge a result; the relay road
  (`review admit`) hands the answer over directly rather than through a result file, and a job directory
  whose `lock` was never created is read from its result alone; and nothing here authenticates the
  model that wrote the answer.
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
  `capture-job-other-run`. `result.json` is removed after admission; the directory itself only by
  the daily bound in `maintain` — past thirty days by its own modification time, and beyond five
  hundred finished directories per repository, newest first. A directory the table holds no row for
  (a check job, a pull-review panel job) is removed by age only and never by the count, and a
  directory whose row names another path is not touched. **Not proven:** the age is the directory's
  mtime, so anything that rewrites a file in it makes it young again; `review_jobs` rows are never
  deleted, so the table grows with every launch; and the `running` sweep is the lock probe, not
  proof the runner is gone.

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
  `a_quota_failure_on_a_lens_records_nothing_and_leaves_release_ci_refused` in `tests/pipe/claims_and_delivery.rs`
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
  `review-blind` and `review-blind-sealed` stay installed and inert. `estigia judge-workspaces` scopes its
  directories to a publication's receipt identity, so a Judgment Day over an unpublished target chooses
  them by hand. `tests/no_judges_in_ordinary_review.rs` holds the rest of `skill/` and the rendered
  contract to "ordinary review acquires no judge" by reading sentences and fails closed: a sentence
  naming a judge and an acquiring verb (run, spawn, use, require, need, add, dispatch, acquire, launch,
  convene, license) fails unless its exact text is in `ALLOWED` with a reason, and a second test fails
  when a listed sentence no longer occurs. Passive voice is caught. What remains: a verb outside the
  list (start, invoke, call, get, have), a judge and verb split across two sentences, a text naming the
  definitions (`review-blind`) with no judge word, the Judgment Day policy file and judge definitions
  (not scanned), and an `ALLOWED` reason, which is a reviewer's claim.

- **`estigia judge-workspaces` hands out directories and proves nothing about where a judge wrote.** It reads
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
  A receipt whose derived tier licenses zero lenses, published without `--inline-payload`, is the
  exception: no lens reads the text, so the payload is materialised without the ceiling and the
  answer stays digest and size only. The refusal
  reports what was already elided and names the split and verbatim-move routes with the call for each.
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

  Two substitutions run over that text before any digest: a block of removed lines byte-identical to a
  block of added lines in another path becomes one `# estigia-moved` line on each side (source and
  destination line ranges and the SHA-256 of the elided bytes), and a whole-file deletion becomes one
  `# estigia-deleted` line (path, range, SHA-256); a hunk that lost lines is recounted. A block is a
  whole run of `-` or `+` lines, or at least `BLOCK_LINES` (8) consecutive lines inside one, so a file
  split into several and a moved block beside new lines of its own are both found; the search is
  greedy in payload order, longest block first and the earliest destination on a tie, and it looks at
  no more than 64 destinations for a block's first line, so a block that starts on a very common line
  is found from a later line or not at all. Each substitution is used only where the line is smaller
  than the bytes it replaces, so an ordinary small deletion or move stays a hunk, and in an edited
  move the edited lines and any remainder under 8 lines stay hunks on both sides. Measured on
  `7f8bb440..0b594879`, four merged slices that split one document into twelve, the payload went from
  836 KB to 44 KB; whole-run matching alone had saved nothing there.

  A second pass, over the lines the first left, also pairs a block of at least 8 lines **moved with one
  indentation shift**: every non-blank source line equals one whitespace prefix (spaces or tabs, never
  one for the other) followed by its destination line, or the reverse, and every blank line is
  byte-identical. Its manifest line adds `indent-removed` or `indent-added` carrying that exact prefix,
  and its `sha256` and `bytes` are of the **source** bytes, so the destination is the source with the
  prefix taken off (or put on) each non-blank line. This is the one place a manifest line stands for
  bytes the judge would read differently: a multi-line string literal whose continuation lines moved
  with the block changed value, and the manifest says only that the block moved with this indentation,
  never that nothing changed. A continuation line that did not move with the block breaks it, and what
  remains is shown. Measured on `bb84ef73..622dbb29`, which moved one test module into its own file, the
  payload went from 243 KB to 23 KB; on `99eaa441..622dbb29`, three such slices, from 2,071 KB to 46 KB.
  **The payload is
  complete in description, not in content**: a judge never reads the moved bytes, and never reads a
  deleted file at all; a person who holds the repository can recover either from the base or head blob
  by the line ranges and digest the manifest names, and nothing in the review transaction makes the
  judge do so. `review_payload_membership` applies the same substitutions, and the same zero-lens
  exemption to the ceiling, deriving the tier from the receipt's ends.

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
  | `skill/bindings/github.md` | 7,998 | file on disk |
  | `skill/policies/blind-judges.md` | 43,406 | file on disk |
  | `skill/policies/judgment-day.md` | 16,022 | file on disk; loaded only when the operator explicitly asks for Judgment Day |
  | Installed `skill/SKILL.md` | *rendered* | `skill::rendered` over `Config::default()` — not a file size; the shipped block is rewritten |
  | `skill/bindings/github/delivery.md` | 42,980 | file on disk; a **publishing** run loads this companion per `SKILL.md`'s delivery row |

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
  executed` survives. A finding's `id` keeps the plain blank check, and it is still the key
  `record_refutation` groups by, but a second recording under one id no longer disappears: every
  recording of an id is weighed, every one of them is handed to the refuter, and the verdict keeps the
  strongest answer they give. What is left is the other direction, which cannot hide a defect: two
  lenses may spell one defect as two ids, and the receipt then carries two findings where one would
  do. The checks run before the tracker read, so a replay of a marker written before the predicate
  existed, carrying filler, is refused rather than read back.

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

- **A pull request review's inline comments are confirmed by count and path, not by line or text, and the
  create-review call's atomicity was not measured.** `pr_review` `submit` posts a finding whose whole
  range sits inside one hunk of `held.merge_base..held.head` as a `comments` entry of the one
  create-review call, and `pull-review-submission-unconfirmed` is answered when `…/reviews/{id}/comments`
  lists a different number of comments or a different multiset of paths than were sent
  (`a_review_whose_comments_did_not_land_is_unconfirmed`). Not compared: each comment's line, side and
  text. A retry that finds this account's review of the commit recomputes the expected comments from
  the same diff and holds the found review to them; a review sent with no comment is not asked for its
  comments, so one carrying comments nobody sent reads back as submitted. The anchoring is decided
  locally because GitHub is read to refuse the whole review for one comment outside the diff, and that
  refusal, the claim that the call lands whole or not at all, the REST field names for a range
  (`line`, `start_line`, `side`, `start_side`) and the per-comment size limit were not run against
  GitHub; `COMMENT_CEILING` is a guess at the limit, and a longer finding stays in the body. A diff that
  cannot be read for a finding's path leaves that finding in the body rather than refusing, so an
  unreadable diff degrades to the body-only review without saying so. `site_in_change` keeps a range
  that spans two hunks, or one only partly inside a hunk, in the body, which loses the inline anchor
  the author could have had.

- **Under a `Review voice` the text posted is not the text the panel answered, and only its shape is
  checked.** With the row naming a voice, `pr_review` `submit` posts each inline comment and the
  review body, and `review_thread` posts its reply, as a detached tool-less job reworded them
  (`src/transport/voice.rs`), not as the lenses or the author wrote them. What is checked about the
  answer, by `a_named_voice_rewrites_each_comment_and_the_body_and_moves_nothing_else` and
  `an_answer_that_is_not_one_text_per_input_posts_nothing`: it is one JSON object with a `texts` list and
  no other field, the list holds exactly as many texts as were given, and each is non-blank and at most
  60,000 bytes, which is the ceiling `submit` already keeps a comment under. What is not: that a
  reworded text still says what the written one did, that an identifier, path, code span, number
  or quotation survived byte for byte, that no finding id, class or label came back, and that the language
  and length are the voice's. Those are in the job's fixed instructions and nowhere compared, so a text
  that drops a caveat, softens a finding or invents a line reference is posted as long as it is the right
  count. The texts handed to the job come from findings a reviewer wrote after reading the diff under
  review, and the job is told they are data; it has no tools, so what a hostile diff could do through it
  is change the words posted under this account, not run anything. Nothing reads the voice file but
  that job, and nothing reviews the file: it is the operator's.
  Also not measured: what a rewording job costs, which no review budget counts; and whether
  the CLI that runs it honours the tool-less flags on every build, which is the claim
  [the isolation entry](#review-models-targets-and-panels) already leaves unmeasured for the panel.
  A rendering is kept by a digest of the voice file's bytes and the text, so a retry posts the
  words already used, but an edited voice file is a new key: a retry after an edit that finds no review of
  the commit posts a different wording than the first attempt did. `review_thread` closes that for a
  reply: it keeps the text it is about to post under a key that holds the repository, the thread and the
  input body, and no voice bytes, so a retry after an edit posts, or recognises, the same reply.
  The voice is read from the repository's own file alone, tolerating a row for another setting that
  does not parse; an unreadable file, or a `Review voice` row that does not parse, stops `submit` and
  `review_thread` as `review-voice-row-unreadable`, even when no voice is named, since whether one was
  named is then unknown. A file that does not exist reads as no voice. The voice path must be a regular
  file, and at most the cap plus one byte of it is read.
  The inline-comment read-back above still compares count and path, never text, so it does not see the
  rewording either.
  **The order of a rendering's answer is not checked, only its count and shape.** An answer that swaps
  two texts puts one comment's words on another comment's line, and `submit` posts it: the sites
  (path, line, side) are fixed before the voice is asked, the text placed on each is whatever the answer
  holds at that position. The job is told to keep the order; nothing compares it, and one job per text,
  which would, was rejected for what it costs.
