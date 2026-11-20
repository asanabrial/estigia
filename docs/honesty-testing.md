# Tests, the suite and the build

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the suite, its fixtures, the CI lane and the build do not establish. The index says which area holds which question.

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
  `paths::staged_tests::two_threads_replacing_one_path_never_lose_their_staged_file` answered
  `NotFound` once in 1,000 contended replaces on `windows-latest` (CI run `41006783911`, both
  attempts) and in no other run: 0 failures in 40 runs alone and in 8 passes of the whole nextest suite
  on that runner. Which step answered is unknown; a `replace_bytes_atomically` error now names its
  step (`stage`, `permissions`, `rename`, `sync`, `verify`), so the next one says.

## Comments, instructions and the build

- **The instruction-surface ceiling counts bytes, not whether the prose agrees with the gate.**
  `tests/instruction_ceiling.rs` refuses a `skill/*.md` file above its registered ceiling, and any
  unexcepted line above 2,000 bytes. The registry is an upper bound: a stale count below the file is
  not a refusal. It cannot see conditional routing, so it does not bound the always-loaded set an
  activation pays, and it does not prove a sentence agrees with `src/harness/`.

- **The shipped skill does not carry its own provenance; the why of a rule is here.** Open
  measurements appear in [`honesty-review-rules.md`](honesty-review-rules.md) and
  [`honesty-review-panels.md`](honesty-review-panels.md) under scratch isolation, review payloads and
  the publication lane.
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

- **On Windows a POSIX absolute path in a shell line is not read as absolute, so the issue gate refuses
  where it would not on Unix.** `Path::new("/tmp/y").is_absolute()` is false there, and under Git Bash
  `/tmp` and `/c/…` are mounts the gate cannot place, so such a redirect or heredoc target is read as
  inside the checkout and refused under `on`: the safe direction, at the cost of extra refusals. Tests
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
  than `git`. A per-operation memo for content-addressed reads then took `transport::claim` from
  12 543 `git` processes to 10 841 on macOS, and the Windows `tests` step on that change still took
  1 106 s: what remains is separate process starts spread over many tests, not repeats inside one
  operation. So Windows now runs the suite in three nextest partitions behind one required context
  ([how the CI lane runs the suite](testing.md#how-the-ci-lane-runs-the-suite-and-the-deadline-on-every-test)),
  which divides the step's wall time and multiplies its runner minutes: each partition pays its own
  checkout, cache restore and fixture build.

- **One operation no longer asks `git` the same fixed question twice, and the removal is partial by
  construction.** The slowest Windows test,
  `an_answer_that_names_what_was_not_offered_or_omits_what_was_is_malformed_and_writes_nothing`
  (27.2 s there, 1.5 s on Linux), started 220 `git` processes on macOS, 12 of them fixture setup; at
  roughly 100 ms a process that is most of its Windows time. About 21 a capture came from the code
  under test asking the same question again: the repository lookup `isolated_diff_env` runs three
  times, `ls-tree` of the declaration file three times, `rev-parse --verify <oid>^{commit}` four times
  over two oids, and the same two-oid `diff` renderings twice. `transport::git_memo` now answers a
  repeated question from memory while a scope is open on the calling thread, and the scope is opened
  inside `review_status`, the capture's `prepare` and `collect_with`, `review_tier_declared`,
  `review_tier_for_repair_declared`, `payload_with` and `publish_with_sleep` — inside them, not only
  in the MCP dispatcher, because the suite calls them directly. Outside a scope nothing is
  remembered, which is why `estigia mcp`, a long-lived process whose worktree path can be recreated
  as a different repository, never carries an answer from one call to the next. Only a successful
  read is remembered, and only these: the repository lookup, and `ls-tree`, `rev-parse --verify
  <oid>^{commit}`, a two-revision `diff` and `cat-file blob <oid>:<path>` whose every revision is a
  full object id. `HEAD`, a ref name, `status`, a stdin-fed `cat-file --batch` and anything that
  writes are never remembered; `target.rs` reads `HEAD^{commit}` either side of a commit and that read
  stays live. The key drops the per-call `GIT_DIR` of an isolated diff directory, because
  `isolated_diff_env` mints a new one every call over the same shared object directory.
  Measured on macOS with the logging `git`: the test above 220 → 175 processes, `cargo test --lib
  transport::claim` 12 543 → 10 841 (the `diff` renderings 2 246 → 1 896, `rev-parse --verify`
  1 857 → 1 622, the repository lookup 1 381 → 740, `ls-tree --name-only` 1 158 → 727). What is left
  is the question asked by *different* operations: a capture is a launch, a collect and the status
  read after it, three scopes, so each fixed question is still asked once per operation, and the
  `cat-file --batch` pair is asked twice inside the collect because it takes stdin. The Windows time
  after the change is not measured here; the continuous-integration run of the pull request that
  made it is the measurement. The test
  `one_capture_asks_git_each_fixed_question_once` counts the spawns through a debug-only per-thread
  log in the transport's one process boundary and fails when a launch or a collect asks a fixed
  question twice.

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

## Test counts and mutation claims in a receipt

- **The assisted Linear queue is the CLI user's own list, and nothing makes taking from it atomic.**
  `query EstigiaQueue` offers only issues assigned to the `viewer` the `linear` CLI is logged in as, so a
  shared API key makes "assigned to me" mean that key's user. A live claim on this machine keeps a second
  local run off an issue, but nothing stops a person or another machine from starting the same issue in
  the same moment. Moving it to the `in-progress` name, where `State names` maps one, is visible and is
  not a lock. The estimate is the team's own number and is not normalised across estimate scales. More
  than one page of assigned issues is refused rather than read short. The tests are
  `the_queue_is_todo_then_backlog_by_priority_then_estimate_then_age` and
  `a_queue_with_another_page_is_a_failed_read_not_a_short_queue` in `src/transport/linear/tests.rs`, and
  `an_assisted_linear_loop_offers_the_head_of_its_queue_less_what_this_machine_holds` in
  `src/cli/tests.rs`.
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
  - **Which checkout ran them is not in the key.** A receipt names the target digest, not the directory
    that produced it: a worktree the pointer names is trusted, and one that reproduces another target
    records under that other digest, which the verdict gate then refuses as a missing receipt. Only the
    fallback to this repository's own checkout is refused before anything runs
    (`checks-worktree-unknown`).
  - **The detaching launch is not crossed for this job.** The `__check-job` pipe test, Unix only, runs the
    subcommand directly; the launch that detaches it is the one `__review-job` uses, crossed by
    `tests/pipe/review_job.rs` for that subcommand alone, and nothing exercises the Windows flags.

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
