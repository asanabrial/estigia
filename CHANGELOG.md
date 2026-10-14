# Changelog

One `##` heading per version, newest first, and a few one-line bullets under it. A pull request does
not edit this file: the agent cutting a version writes its entry in the release pull request, from
the pull requests merged since the previous tag. `changelog-notes` reads that entry for the tag and
the Release, and refuses a missing or empty one.

## 0.1.0 — unreleased

The first version. Estigia is a harness: it does not ask an agent to follow the workflow, it holds
the tools.

- A claim on an issue, adjudicated in the local store (`~/.estigia/state.db`); every repository write is measured against it.
- A gate under Claude Code, Codex, OpenCode and Pi that refuses writes, pushes and merges the claim does not justify.
- Review verdicts and CI runs bound to an exact head and base; any push invalidates them.
- Receipt-driven blind review: tiered lenses, refuter and validator launched and captured by the binary.
- Delivery checks: merge only reviewed, green, at the reviewed head, on its base or a cleanly advanced one.
- GitHub tracker binding, with workflow states as labels; Linear read-only for assisted work.
- Assisted mode: work on a named issue with no tracker writes, ending in a draft pull request.
- A git operation window that queues GitHub writes while it is closed.
- Sessions, a session registry and a mailbox delivered at hook points.
- Loop and goal-loop plugins that keep a run going until the tracker says it is done.
- Pi subagents with a live card, a full view and history.
- `estigia setup` and `uninstall` for each supported agent, and `doctor`, `changes` and `prune` to inspect and clean up.
- Linux, macOS and Windows builds.

- **A branch differing from a registered one only by case is refused, not misread as an orphan.**
  `worktree_path` composes `<branch>~<run-id>` and lowercases the run ID by construction,
  but only separator-checks the branch — so `Fix-1-a` and `fix-1-a` under one run ID compose two
  strings that are one directory on a case-insensitive filesystem (default APFS on macOS, NTFS on
  Windows). The second run's path then exists as the first run's checkout, while the registry key
  (hex of the bytes on Unix) did not fold case, so `may_occupy` answered `worktree-path-occupied`
  with `occupied_by_branch: null` — the orphan diagnosis, whose action invited removing the first
  run's live checkout. `may_occupy` now refuses a composed path whose spelling case-folds onto a
  registered worktree's as `case-insensitive-branch-collision`, naming both branches and saying not
  to delete. The fold compares spellings and not the filesystem, so it refuses on a case-sensitive
  volume too — the conservative direction. The composed `<branch>~<run-id>` shape and `SCOPE_JOIN`
  are unchanged.

- **A branch differing from a registered one only by Unicode normalization is refused too.** The collision check folded case only, so `café` as U+00E9 and `café` as `e`+U+0301 composed
  two byte sequences that default APFS treats as one directory (it is normalization-insensitive), and
  the registry key (hex of the bytes) missed for the same reason it misses for case. The same orphan
  diagnosis — `worktree-path-occupied` with `occupied_by_branch: null` — was reachable through an
  ordinary accented branch name. `may_occupy`'s spelling fold now normalizes, folds case and normalizes
  again, so the pair is refused `case-insensitive-branch-collision`, and its action names both folds.
  The second pass composes a capital base that lowercasing first makes composable (`J`+U+030C against `ǰ`).

- **A closing keyword that names another issue is refused, not only one naming the issue under
  delivery.** `keywords_naming` (`src/transport/closing.rs`) reported only references to
  the issue it was handed, and `publish_review`'s pre-push scan and `check-closing-keywords` both used
  it, so a body saying `close #464, #469, #473 and #474` published clean and GitHub closed #464 on
  merge — skipping #464's `done` transition — with nothing objecting. The scan is now one walk
  reporting every reference; the pre-push refusal takes any of them, `assess_autoclose` carries the
  ones naming another issue as `other_issue_keywords`, and both the post-push arm and
  `check-closing-keywords` refuse them. The delivered issue's own check is unchanged. A test drives a
  body and a commit message naming another issue through `publish_review` before the push, and
  `docs/honesty.md` records the case `assess_autoclose` still cannot see.

- **A global option between `estigia` and its verb no longer hides a disarm.** `without_leading_options` (`src/harness/mod.rs`) moved options out of the way only after seeing
  `git` or `gh`, so `estigia --json config edit` — the same handler and the same write as the listed
  `estigia config edit`, because `--json` is a clap global arg — contained no listed fragment and
  classified `Untouched`, outside the gate. The walk now recognises an `estigia` word by its
  basename, as `as_estigia` already did for the match itself, so a global option between the binary
  and the verb cannot break the fragment; a path prefix and a `.exe` suffix are folded the same way.
  A test drives the flag-first spelling for `config edit`, `config set`, `stand-down`, `uninstall`
  and `guard --uninstall`, and holds `estigia --json status` and `estigia --json config list` as
  reads.

- **A reclaim the fold discards while it leaves no holder names its condition, and the
  reclaim-discard vocabulary's counts agree.** `won_the_reclaim` (`src/transport/claim.rs`) consulted `ownership::discarded_reclaim` only on
  the foreign-holder branch, so a fresh reclaim the fold discarded while it displaced nobody — its
  target scrolled out of the fetched window, the cause `target-missing` itself names — answered only
  that the write's fate was unknown, and the caller could not learn which check dropped it. The
  no-holder branch now answers `reclaim-discarded` with the same `condition`, `world: committed` and
  action, and omits `winner` because the timeline shows no holder to name; a timeline carrying no
  reclaim of this operation at all is still the write failure it was. `reclaim_is_valid`'s doc counts
  the fold's six conditions (`target-run-mismatch` was the missing one), the `ReclaimDiscard` doc
  separates the nine a refusal carries from the fold-internal `legacy-takeover`, `evidence-unproven`'s
  doc names the unnamed-epoch input beside unbound evidence, and `skill/bindings/github.md`'s
  `reclaim` row names the reader-level conditions a fresh write cannot carry and what it answers
  instead. A unit test drives a discarded reclaim with no live holder through `won_the_reclaim`, a
  second pins the malformed-before-reserved precedence, and an end-to-end `reclaim` over the MCP door
  answers `reclaim-discarded` with `target-missing`. The entry below is scoped to a takeover
  rather than a forced one and its mutation claim now matches the mutations run.

- **The last two git fixtures stop spelling a skip as a pass.** `git_repo()`
  (`src/harness/doctor.rs`) and `release_repo()` (`src/cli/tests.rs`) answered
  `Option<tempfile::TempDir>` over a single `git init --quiet`, and every call site that reached
  them opened with a `let … else { eprintln!("SKIPPED: …"); return; }` — so a machine where git
  is absent or refuses took every test built on them to green having measured nothing. Both now
  return the directory and panic, naming the failing command, the repair `repository()` and
  `repository_with_commit()` already got. Measured with a failing `git` first on `PATH`:
  `gate_version_reports_a_row_against_a_real_checkout` reported `ok` before and panics after, and
  `forgetting_a_pointer_the_tracker_says_holds_no_epoch_removes_it` the same. The third fixture the
  issue named, `two_holders_with_worktrees`, had already stopped answering one in its own
  follow-up. `tests/guards.rs`'s skip-as-a-pass guard no longer names any fixture as still answering
  one, and `docs/honesty.md`'s entry records the closure. No behaviour changed outside the fixtures.

- **The relocated-root check is anchored to a path boundary and agrees across both roads.** `matches_relocated_claude_config` (`src/harness/mod.rs`) matched the resolved `$CLAUDE_CONFIG_DIR`
  with `path.find(root)`, a **substring**: a path that merely carried the root's text mid-segment —
  the root's own text under an unrelated prefix — answered `Boundary` though it did not land under
  the root. And the shell road hands the matcher the view `text_names_path` builds, where every
  whitespace run and every `NOT_IN_A_PATH_SEGMENT` byte is folded to `/`; a root whose own name
  carries one therefore stopped matching on the shell road while the write road still answered
  `Boundary`, so `rm` walked round a boundary a `Write` could not. The match now requires the root to
  begin at the path's own start or after a separator, and asks the shell view's folded spelling of
  the root beside the root itself, so the two roads agree.
  `a_path_that_merely_carries_the_relocated_root_is_not_a_boundary` reddens when the boundary
  requirement is dropped, and `a_relocated_claude_config_dir_with_a_space_is_a_boundary_on_both_roads`
  reddens when the folded spelling is dropped. Unset, every classification is byte-identical to before.

- **A relocated root spelled with an escaped space is a boundary on the shell road.** `matches_relocated_claude_config`
  (`src/harness/mod.rs`) asked the shell road's folded spelling of the resolved `$CLAUDE_CONFIG_DIR`
  beside the root itself, which closed the quoted and bare spellings of a root whose name carries a
  space. The **escaped** spelling — `/tmp/estigia\ relocated/settings.json` — leaves the folded byte
  and the token boundary side by side, so the shell view carries `//` where the folded root carries one
  `/`; the folded spelling missed it and the shell road answered `Routine` while a `Write` to the same
  path answered `Boundary`. The folded spelling is now asked with one separator and with two, so the
  quoted, bare, single-quoted and escaped spellings all classify as `Boundary`.
  `a_relocated_claude_config_dir_with_a_space_is_a_boundary_on_both_roads` now crosses all four and
  reddens when the doubled spelling is dropped.

- **A Codex uninstall refuses an unparseable `config.toml` before removing anything.** `uninstall_from` (`src/setup/mod.rs`) removed the skill, directive, `hooks.json` and the gate and
  loop plugins, and only then reached the MCP step, where `codex_trust::without_trust` parses
  `~/.codex/config.toml` to take Estigia's trust out. A config that is not TOML failed there, leaving
  a half-uninstalled agent whose retry failed the same way. The uninstall now parses the config —
  only when there is trust to remove — before the first removal, so a refusal writes nothing;
  `codex_uninstall_refuses_a_config_it_cannot_parse_and_removes_nothing` seeds an unparseable config
  and holds the config, the hooks file, the directive and the skill untouched. No exit code, outcome,
  replay or resolution changed.

- **The OpenCode review section names the disabled lane as a launch state.** `opencode_review_section()` (`src/setup/mod.rs`) restated the panel-launch condition as "green,
  never-ran, absent, or has no lane at all" and omitted `disabled`, while
  `skill/references/runtime-notes.md` and `skill/SKILL.md` both name it. An OpenCode host with Actions
  off read no branch that matched its lane and could withhold or stall the panel where the contract
  says to launch. The rendered list now names `disabled` beside the other four and still excludes red
  and unreadable. `the_opencode_section_names_every_launch_lane_state_the_contract_names` crosses the
  rendered section against `runtime-notes.md`'s own list rather than typing the states a second time,
  comparing the two lists for equality so neither can add or drop a state the other does not name; it
  reddens when `disabled` is removed from either enumeration. `skill/SKILL.md` names the same states
  in its own words and is not crossed by that test.

- **The exit-code fallback says which codes it reaches, and the sentences beside it match the code.**
  `translate` computes the fallback
  message before it matches `answer.code`, so the field dump replaces the exit-code sentence for
  **every** code whose envelope carries no `detail`, not only `2`; `Failure::envelope` always writes
  one for `Read`, `Timeout` and `Write`, so `3`/`4`/`5` are unreached, but a code-1 `Stop` payload
  with a field beside `ok`, `reason`, `action` and `world` is not. That reach is now in `translate`'s
  own doc and in `docs/honesty.md`'s entry on it. The entry's claim that the message repeats a field
  beside the reason is narrowed to a **scalar-valued** field beside the ones the refusal is built
  from, and a new test,
  `an_envelope_field_that_is_not_scalar_is_dropped_and_a_scalar_is_repeated`, holds both directions:
  a string, number, boolean or array of scalars is repeated, and a null or an object is dropped.
  The remaining prose items are corrected in the same pass: `skill/bindings/github.md`'s exit-2 row
  now says the malformed-argument table takes precedence over the JSON's `action`; `envelope_detail`'s
  `///` no longer claims "every scalar field" (it joins arrays) and no longer names the incident
  `docs/honesty.md` already holds; `malformed_argument`'s `///` says "the reasons" over its three
  rows rather than "the two"; the unwrapped line in the honesty entry is rewrapped; and `AFFIRMED`'s
  doc states that its dated notes are the record the table exists to hold. That last item was filed
  as history, but it is a question about the table's convention, so it is answered in the table
  rather than by deleting one line. `tests/instruction_ceiling.rs` re-registers
  `skill/bindings/github.md` at 109,222 bytes. No exit code, outcome, replay or resolution changed.

- **A health row stops denying the population it cannot see, its token has one spelling, and its
  test discriminates.** `own_delivery_close`
  (`src/harness/doctor.rs`) matched a `closed-by-own-delivery` literal that `src/transport/claim.rs`
  wrote as a second, uncrossed copy; both now read one const, `OWN_DELIVERY_CLOSE_PREFIX`, and
  `OWN_DELIVERY_CLOSE_SPELLING_POPULATION` reopens `tests/guards.rs` when the token moves. The
  `Fine` sentence said no pointer "names one closed behind this run's own delivery", false of an
  **aged** stranded pointer — `reduce_ownership` (`src/transport/ownership.rs:1104-1119`) drops an
  expired acquisition to `holder: None`, so `verify_claim` answers `not-current-live-holder` and the
  row reported `Fine` over it; the sentence now says "still holds", and the row reports how many
  pointers it read as still open and held. That count makes
  `a_genuinely_passing_pointer_is_reported_fine` fail when the fixture's label state is changed to
  `"review"` — where asserting the `Health` variant alone passed either way — and
  `own_delivery_close`'s predicate is now exercised directly, including bodies whose `checked[0]` is
  absent, non-array or non-string. `stale_run_pointers`' doc names both arms that feed `unread`.
  `docs/honesty.md` counts nineteen declared populations.

- **The ten branches that answer a tracker with no executable transport are named, not left
  unchecked.** `Tracker` has one variant and `Tracker::transport()` answers `Some` for
  it, so every `is_none()`/`is_some()`/`?` arm that reads its absence is unreachable —
  `cli::doctor`'s sentence, `cli::inert_tracker`, `harness::doctor::stale_run_pointers`'s `Skipped`
  row, `harness::issue_requirement::in_force`, `harness::mcp::run_tool`'s tracker arm,
  `harness::decide_observing`'s `NoTracker` aside, `harness::tracker_answer_for_pointer`,
  `harness::queue_read`, `harness::completed_work` and `harness::unclaimed::applies`. They are kept
  as the seam a second tracker variant would use, and
  `TRACKER_TRANSPORT_ABSENT_POPULATION` in `src/config/mod.rs` binds to `Tracker`'s own variants, so
  adding one reopens the claim and sends a reader to each site. Deleting the declaration reddens
  `tests/guards.rs`, and so does adding a second variant; a call site added before one lands is the
  escape the population names. `docs/honesty.md` counts eighteen declared populations.

- **The MCP handshake and the TUI banner answer this build's identity, not the crate version every
  build shares.** Four surfaces answer "which build is this", and two of them — the MCP
  `serverInfo.version` (`src/harness/mcp/mod.rs`) and the TUI banner (`src/tui.rs`) — read
  `env!("CARGO_PKG_VERSION")`, the bare `0.1.0` every build this crate has produced answers with.
  Both now read `crate::build_info::VERSION`, the `0.1.0 (<commit>[, dirty])` the `--version` and
  `status` surfaces already use, and each is asserted: reverting either reddens
  `the_handshake_answers_with_this_server_s_identity` or
  `the_landing_screen_is_a_mark_and_a_menu_with_no_frame_around_it`. MCP 2025-06-18's `Implementation`
  shape types `version` as a free-form string, so the build identity is protocol-conformant and no
  second field is needed. `docs/honesty.md` narrows the unasserted-identity gap to `status --json`.

- **A command line carrying two or more irreversible fragments of one tier is refused, not ranked.**
  `classify_observing` records one fragment per line, chosen by `delivery_tier`, so when
  every fragment was the same tier the key tied and resolved by lowest index — `git merge origin/main
  && git tag v1` recorded the merge and never adjudicated the tag, and `git push && gh pr create`
  recorded the push and reached no delivery check. A line whose built-in `IRREVERSIBLE` fragments put
  two or more in one tier is now refused with
  `several-irreversible-fragments-on-one-line`, which names the fragments rather than any one of them;
  a line carrying exactly one fragment answers as it did. Operator-declared fragments are not counted,
  for the reason `DELIVERS_POPULATION` excludes them from `DELIVERS`. `docs/honesty-review-tier.md`
  narrows the residue this closes.

- **The merge-commit topology rule now asks what a delivering run can guarantee.** `skill/SKILL.md` step 6 required the delivered SHA's exact parents to be the reviewed base and head,
  but a delivering run checks the base and GitHub merges non-atomically, so a concurrent merge can move
  the first parent while the reviewed bytes arrive intact. The rule now requires the second parent to
  be exactly the reviewed head and the reviewed base to be an **ancestor** of the first parent, and
  `verify_claim`'s closed-issue exception applies the same predicate: `match_own_close` takes the
  ancestry, `own_delivery_close` reads it with `git merge-base --is-ancestor`, and a read that does not
  answer is a `Read` failure rather than "not my delivery". A forward-integrated delivery is now
  recognised; a diverged or rewritten history is still refused. `docs/honesty.md` states what the
  narrowed rule no longer proves.

- **The inline-write record is called a target, and a blank `agent_type` no longer turns a
  delegate's write into this run's own.** `Run.inline_writes` holds `Action::subject()` — a path for a write tool, but the matched fragment
  (`git commit`), the verb (`chmod`) or the tool name for a shell write — while the `SessionStart`
  message, `session.rs`'s doc, `docs/honesty.md` and `SKILL.md` all called the elements files. All
  four now say **target**, and the field's doc names what the element can be. Separately, a
  present-but-blank `agent_type` was filtered to `None` and then read as this run's own context
  whenever `roles` was non-empty, so a delegate sending one produced a claim that this run wrote its
  write; only a genuinely absent field is attributed now. `SKILL.md` no longer promises the readback
  across `SessionEnd`, `Action::subject`'s own doc and `note()`'s comment stop calling the element a
  file, and the three drifted `hook.rs`/`cli/mod.rs` citations in `docs/honesty.md` are re-derived.

- **The receipt rule lives in one function again, and two non-handoff callers stop claiming a handoff.**
  The publisher comparison that had been the only thing distinguishing
  `recorded_receipt` from `require_latest_receipt` was removed, leaving both building the same
  `published-transaction-mismatch` envelope word for word. `require_latest_receipt` is now the only
  construction site and takes the blocked operation's action tail; `recorded_receipt` is a thin `pub`
  wrapper passing `cannot release CI`, so `release_ci`'s missing-receipt answer is unchanged. The two
  callers that were not handoffs no longer answer `cannot be handed off`: `record_review_finding` now
  says `cannot be recorded as a finding`, and the verdict/refutation path through
  `require_distinct_reviewer` says `cannot be credited with a verdict or a refutation`. The reasons and
  the six-field `recorded` object (including `run_id`) are unchanged.

- **`doctor`'s reinstall command is the checkout's own and names its shell, and the source scan reads
  or names every `Resolution::run` argument.** `reinstall_command` printed
  `cargo install --path . --force` (or, on Windows, a bare `Stop-Process …; cargo install …`), but
  `repo_dir` is the process's current directory and cargo does not search upward for a manifest — so
  from `<checkout>/src` the row could report `Broken` and the command it named would fail there, and
  under `cmd.exe` or `sh` the `Stop-Process` token was not a program at all. It now installs from the
  checkout root (`git rev-parse --show-toplevel`, falling back to `repo_dir`) and, on Windows, runs
  through `powershell -NoProfile -Command`, so it is correct for the shell it is printed into. The
  source scan that checks suggested commands read the next `"` in the file rather than the argument,
  so `Resolution::run(reinstall_command(…))` was never checked and `Resolution::run(instead)` or
  `format!("{command} …")` were read as whatever string happened to follow; it now reads a string
  literal, a raw string or a `format!`'s first literal at the call site and records anything else as
  unread, which the guard names rather than skips. Fixing it surfaced two dead roads: `estigia tui
  --allow-source-build`, a flag `tui` does not have, and the `require_terminal`/`lifecycle_preflight`
  helpers, which now take the `Resolution` their callers build so the command is visible where it is
  written. `tests/build_install.rs`'s order check matched a bare `doctor` anywhere in the log — which
  this worktree's own path contains — and now matches the logged step.

- **A boundary with a reason is now delivered on the surface the agent touches.** `NOT_EXPOSED`
  wrote why each of the contract's unexposed operations has no tool, but nothing in the runtime read
  it — only two tests did. `tools/list` answered from `TOOLS` alone, so `close` was never advertised
  and the client dropped the name with `No such tool available`, indistinguishable from a typo; the
  run then closed the issue with raw `gh`. The three names are now advertised from the same
  `NOT_EXPOSED` list, each with no arguments and the reason as its description, and a call to one
  refuses `operation-has-no-tool` with that reason **and** the `bindings/` section that carries the
  operation (`*Closing*` and `*Version tags*` in `bindings/github/delivery.md`, `*last_activity*` in
  `bindings/github.md`). The acting tools are unchanged — `TOOLS` still holds thirty-two, the README
  count still keys on it, and the seam ratchet still fails an operation neither exposed nor declared.
  Four new tests hold it: every reason reaches `tools/list` and every call, a name that only refuses
  refuses whatever it is given, and the advertised count is the acting tools plus the refusing names.

- **The role a delegated context ran as now outlives the session that observed it.** The
  gate wrote each distinct `agent_type` into the run pointer, which `SessionEnd` removes, so no run
  could be audited for it afterwards. A second append-only history, the store's `roles` table —
  separate from the decision history, which is a history of adjudications and not a place for an
  observation — records the role at the moment the gate sees it and independent of the decision, so a
  call answered `Outside` is recorded too. `estigia roles` reads it back, filtered by `--run-id` and
  `--issue`. It is not a count and not an isolation proof, and `docs/honesty.md` states which panel
  limits it does and does not touch.
- **A project's own instruction files are watched, so a write to one costs a live tracker read.** The
  control surface gained the per-project twins of the files it already watched and the host
  configuration beside them: a checkout's own root `CLAUDE.md`, `AGENTS.md` and their siblings, its
  commands and prompts directories (`.claude/commands/**`, `.codex/prompts/**`,
  `.cursor/commands/**`), its extension roots (`.gemini/extensions/**`, `.qwen/extensions/**`),
  Continue's config stem and assistants, Windsurf's MCP file and workflows, `.opencode/agent/`,
  `.cursor/cli-config.json` and `.claude/plugins/**` are `Boundary` inside the checkout, where each
  was `Routine` before. The population's second clause covers them — a write to one changes what an
  agent is told this harness may enforce — and a `Boundary` is consulted before the renewal window,
  so a write to one pays a live `gh issue view`: measured by classifying every path `git ls-files`
  reports, **two** of this repository's own tracked files, `CLAUDE.md` and `AGENTS.md`, are now
  `Boundary`. A consumer's own root instruction files join that count, and that read per write is the
  cost this entry moves onto them.
- **A branch whose separators the worktree path flattened could share a directory with another.**
  The isolated checkout's path folded every `/` and `\` in the branch to `-`, so
  `fix/1-a`, `fix-1-a`, `fix//1-a` and `fix\1-a` — four branches taking four different branch locks
  — all composed one path, and the second run found a registered worktree carrying what looked like
  its own branch at what looked like its own directory. The branch is now validated rather than
  flattened, exactly as the run ID already was: a branch carrying `/` or `\` is refused with
  `separator-in-branch-component` (since retired: the separator is encoded instead), and a branch with no separator is injective and is accepted
  unchanged by this rule.
  The repository name and the issue number stay flattened, for the reasons `worktree_path`'s own doc
  records.

- **`release` finishes putting its own checkout down on a retry, from outside it, and says which half
  stayed.** Three defects in the checkout retirement, plus the reviewer
  directories it left unowned. A release that found its own unassign marker already on the issue —
  the retried release — answered without a `checkout` key and never attempted retirement, so a first
  release that posted its marker and then failed before cleanup left a delivered checkout with no
  record and no report; both paths now return through one `release_answer`, and
  `tests/pipe.rs`'s `a_retried_release_puts_its_checkout_down_too` drives it end to end. Both
  destructive commands now run from a directory outside the target — the common git directory's
  parent — because the workflow tells a run to work inside its own checkout, where `repo_dir` is the
  directory being removed and `git branch -D` otherwise spawns from a path that no longer exists;
  `src/transport/branch/tests.rs`'s `a_release_launched_from_inside_the_checkout_still_puts_it_down`
  is the fixture whose process cwd is that checkout. And the answer now distinguishes "worktree
  removed, branch kept" from "kept" — the `removed` conjunction could not, and the printed line
  called a removed directory kept — through a new `outcome` word; kept answers carry the branch whose
  work survived, and the `branch` field's doc is true again. Three `docs/honesty.md` sentences were
  corrected to what the suite asserts: the unrecognised-close test now reads `checkout.detail` back,
  the "nothing outside the answer crosses" sentence is scoped to production because the two pipe
  fixtures cross it, and the guard-population sentence no longer quotes a doc comment that does not
  exist. On the reviewer half: a judge's directory has no remover, and that is now stated in
  `docs/honesty.md` and `skill/references/repository-delivery.md` — the orchestrating run creates
  it, Estigia does not spawn the judge, `release` never reaches it, and an operator sweeps it by hand
  under the same unpublished-work check `release` applies to its own checkout. Whether those
  directories belong under the configured `Worktree location` at all is recorded as unsettled.

- **A membership answer means every line was published somewhere, not that the text is faithful.**
  `review_payload_membership` builds one set over the whole payload, so a
  candidate assembled entirely from real lines can be reordered, duplicated or cross-spliced between
  files and still answer `ok: true` — the passing answer a caller reads as "this payload is faithful"
  is only "every line was published somewhere". The limit is now stated in `docs/honesty.md` with a
  constructed example, in the tool description, and pinned by
  `claim::tests::review_payload_membership_accepts_reordered_duplicated_and_cross_spliced_real_lines`,
  which builds all three shapes from the fixture's own payload bytes and asserts each answer. Nothing
  about what `publish_review` records, or the payload itself, changed.

- **A never-ran lane could be cleared with the word `n/a` as its evidence.** The `--never-ran-evidence-file` that stands in for a lane that did not run was refused only
  when it was empty or whitespace-only, so `n/a` cleared the one place a verdict moves past unread
  CI. The check now runs `says_nothing`, the same predicate that decides a verdict's `basis` and a
  finding's `evidence` and `impact`, and the word list moved out of the function into
  `SAYS_NOTHING_WORDS` so the test iterates the one list instead of retyping it; the `Population`'s
  subject moved to the list so `tests/guards.rs` still fingerprints the words, and adding or
  removing one reopens the claim. `says_nothing` is `pub(crate)` now, having no caller outside the
  transport. The honest short answer — `nothing was executed` — still clears the lane. The
  carve-outs that remain are recorded rather than closed: a finding's `id` still takes filler, and
  because the checks run before the tracker read, a replay of a marker written before the predicate
  existed, carrying filler, is refused rather than read back. See
  [`docs/honesty.md`](docs/honesty.md).

- **SDD is gone: the protocol, its five phase agents and the `Planning` setting.** The
  planning protocol `skill/protocols/sdd.md`, the five `sdd-*.md` phase definitions and the
  `Planning` row are deleted, along with the `PlanningPhase` model-routing kind and the phase keys
  `explore`, `propose`, `spec`, `design` and `tasks`. A stored `Planning: sdd*` row still reads as
  `direct` and reports `planning-protocol-retired` once — the earlier promise, kept as the setting
  itself goes — and an owned `sdd-*.md` is still retracted through the retired-names list in
  `src/skill.rs`. A stored `Model routing` cell naming a phase key is dropped by the tolerant read
  and named in a report, and the strict read refuses it. `docs/configuration.md`,
  `docs/honesty.md`, `docs/parity.md`, `docs/what-it-writes.md`, `docs/lifecycle.md` and
  `README.md` move with it.
- **The configuration screen no longer offers a planning protocol.** The `Planning` row
  and the planning-phase model targets are removed from the screen and its help text.
  `Model routing` still projects `orchestrate`, `apply` and the delegated roles. A later change removes
  the setting, the protocol and the `PlanningPhase` type the screen used to offer, and with them the
  retired phase routing keys.
- **A repository can require an issue before a run changes it.** A plain-text request with
  no `flow` edited the primary checkout with no issue filed. The new `Issue requirement` row, answered
  only by the repository's own file, takes `ask` (the default), `off` and `required`. A run holding no
  claim is refused a routine write with `issue-required` under `required`, and asked under `ask` where
  the host can — Claude Code — and refused elsewhere. `off`, a checkout with no installed contract,
  reads, writes outside the checkout and claimed runs are unchanged. See
  [`docs/honesty.md`](docs/honesty.md) for what the hook cannot see and for the approval it cannot
  read back.
- **The OpenCode loop plugin is `flow-loop.js`, with commands `/flow-loop` and `/flow-status`.** The
  commands and the plugin id were named `estigia-loop` and `estigia-status`, which read as the tool and
  not as the workflow they drive. `estigia setup` writes `flow-loop.js` and removes an Estigia-owned
  `estigia-loop.js` left by an earlier build, and `uninstall` removes both; a file of that name Estigia
  did not write is left alone.
- **An unclaimed run can no longer commit or push where the push guard is installed.** A
  run holding no claim could `git commit` on the default branch and push it with nothing refusing
  either. With the guard installed, the agent gate refuses every commit-creating command and every
  `git push` while no claim holds the checkout (`unclaimed-commit`), and the `pre-push` hook refuses
  the same push; the way out is `claim`, then `start_branch`. A checkout without the guard is
  unchanged. See [`docs/honesty.md`](docs/honesty.md) for what it does not hold.
- **Both OpenCode loop plugins show their messages.** OpenCode 2.0.20 renders a
  synthetic message's `description` and never its `text`, and the Estigia loop and the generic goal
  plugin sent a constant one, so `/estigia-status`, `/goal`, stops and refusals showed only "Estigia
  loop" or "Goal loop". Each message now goes out in `description` as well as `text`, prefixed only
  when it does not already name the plugin. Read from the binary, not observed on a live host; see
  [`docs/honesty.md`](docs/honesty.md).
- **The Estigia loop runs on Codex, behind the same `Loop plugin` row.** With the row
  `on` for Codex, `estigia setup codex` registers a `Stop` and a `SessionStart` hook beside the gate,
  each with its trust entry, and takes both back when the row is `off`. `SessionStart` announces
  `Run id codex-…`. `Stop` blocks with the claim reason while the run holds a live claim, and
  otherwise reads what `estigia goal` reads: with claimable or dischargeable work it continues with
  the OpenCode preamble and the objective `$flow dev`, and otherwise it allows the stop with a
  `systemMessage` saying what the queue waits on or what could not be read. The Codex path ignores
  `stop_hook_active`, which Codex never resets within a turn, so the tracker read is the only
  terminator; Claude Code's `Stop` is unchanged. The objective rule and the preamble live in
  `src/harness/loop_text.rs`, and the OpenCode plugin is rendered from them. Codex invokes a skill by
  mention, so its objective is `$flow dev`, not `/flow dev`. `--wait`, `--max`, per-session text and a
  status command are not ported; `estigia goal --session-id <id> --runtime codex` is the status.
  A hook payload whose `transcript_path` is `null`, which Codex's schema allows, was read as
  unreadable and stood aside; it now parses.

- **`/estigia-loop` text is prefixed with `/flow`, not `/flow dev`.** Free text such as
  `only Alicante and Murcia` was sent as `/flow dev only Alicante and Murcia`, which skipped the
  routing and made the text a dev domain rule. It is now `/flow only Alicante and Murcia`, which routes
  from the tracker, so an empty queue triages the text first. A bare `/estigia-loop` is still
  `/flow dev`. `skill/SKILL.md` says that text after a bare `flow` is the analyst's conditions on
  triage and the dev's domain rules on dev.

- **`/estigia-loop` runs `/flow dev` by default.** With no text, or with only `--max` and
  `--wait`, the command returned without saying anything. It now stores and enqueues `/flow dev`.
  Text that begins with the word `/flow` or `flow` is used as given, and any other text is prefixed
  (with `/flow`). The stored
  goal, every re-enqueue and `/estigia-status` carry the prefixed text; `stop` is unchanged.

- **`setup codex` trusts the gate it registers, and `doctor` says `live` only while Codex would run
  it.** codex-cli 0.159 runs a user hook only when `config.toml` holds
  `[hooks.state."<key>"] trusted_hash` equal to the hook's current hash, and skips any other without
  a word, so the gate `setup codex` registered decided nothing while `doctor` reported `1 live`.
  Setup now writes that entry for each Estigia-owned Codex hook, with Codex's own key and hash
  derivation; `uninstall` takes back exactly those entries. `doctor` reports a Codex gate that is
  untrusted, whose hash has drifted, or that is switched off with `enabled = false` as broken, and
  names `estigia setup codex` only where running it clears the row (a `config.toml` that defines the trust
  in a shape setup refuses is reported as such, and an unreadable one as unreadable). `docs/what-it-writes.md` says what
  is written and `docs/honesty.md` what is not proven. Re-running `setup` also no longer moves
  Estigia's entry to the end of a hooks array, which had shifted the position, and so the trust, of
  every hook after it.

- **The Estigia loop plugin's commands are `/estigia-loop` and `/estigia-status`, and a generic
  `/goal` and `/loop` plugin is a separate row.** The plugin registered `loop` and `goal`,
  which shadowed the commands those names mean to a person and to other tools. It now registers
  `estigia-loop` and `estigia-status`, and `/estigia-status <text>` refuses visibly. A new `Goal loop
  plugin` row (default `off`, OpenCode only) writes `goal-loop.js`, which offers `/goal <condition>`
  with a turn cap and `/loop [interval] <prompt>` and holds no Estigia path and runs no Estigia
  command. Its evaluator is `ctx.session.generate`; an answer that cannot be read stops the goal, and
  `docs/honesty.md` says the judgement is a model's, never Estigia evidence.

- **`setup` folds the retired `Delegated workers` row instead of refusing it.** The
  installed-contract read went through the strict chain, which refuses a contract still carrying the
  row an earlier restructure deleted — so `estigia setup` and `setup --all`, the commands an upgrade
  runs, exited 1 without writing, and every later run refused the same row identically. The read now
  folds the retired row the way `config set` already did — each name it held becomes an `inherit` in
  `Model routing` — and carries the row it folded so `setup` prints it as a note. A row a person
  mistyped is still refused, because a fold and a parse refusal are separate channels; `status`,
  `config list` and `doctor` read the same fold, so a contract `sync` can rewrite no longer reads as
  *not understood*. `docs/honesty.md` states what the fold does not measure.

- **The loop's `--max` counts completed issues, not turns.** `--max <n>` stopped the
  loop after `n` turns, so a session that drained its queue slowly hit the cap early and one that
  delivered several issues could exceed it. The plugin now stops when the run's **completed issues**
  reach `n`: it asks `estigia goal --completed`, whose count is enumerated from the tracker — the
  closed, `status:done` issues that mention the run, each proved by `own_delivery_close`, the merge
  proof `verify-claim` uses — never inferred by the plugin and never kept locally. An answer whose
  completion count Estigia cannot report stops the loop rather than reading as zero. A squash or
  rebase delivery is a floor for that proof, so the cap can run past it; `docs/honesty.md` says so.
  The plugin's `node` test gains the completion-cap and unreadable-count exits.
- **The OpenCode loop stops when Estigia cannot confirm the run's claim.** The loop
  decided on `live` and `claimable` alone, so a turn whose `estigia goal` answer read
  `standing: "unread"` — the tracker could not be read for this run's claim — continued on claimable
  work. An unknown result is not clearance: the loop now continues on a non-live answer only when the
  standing is a definite `stopped` or `nothing-sworn`, and stops on `unread` or `unreadable` whatever
  the queue says. The plugin's `node` test gains that exit.
- **A run can record progress or blocked activity, and ask whether its goal is complete.** The second half of `loop-goal-adjudication`. The timeline's activity markers gain `progress` and
  `blocked`, both of which renew a claim exactly as `heartbeat` does, and the one renewal writer now
  records the kind it is asked for, refusing any word but `heartbeat`, `progress` or `blocked` —
  `branch` and `published` are activity too but each has a writer of its own. `estigia goal progress
  <note>` and `estigia goal blocked <reason>` are the CLI roads to them, with the identity flags
  global so they may be given before or after the subcommand, and the `heartbeat` MCP tool gains an
  optional `kind`. `estigia goal complete` is read-only and adjudicates completion against delivery
  evidence: the tracker says the issue is closed, or the run's latest publication carries a distinct
  accepted verdict and a lane the gate would clear — not red, and finished. It writes nothing and exits 0 for a read answer, and
  it re-reads the receipt and the lane at call time — `docs/honesty.md` says what it cannot prove
  about the work, the merge, or the honesty of the run's own claim.
- **The `gate-version` fixture picks its assertion branch from the binary it asserts on.** `tests/pipe.rs` chose the branch with `estigia::build_info::dirty()`, a constant compiled
  into the test target's own copy of the library, while asserting on what the spawned binary printed
  — the only way the row could be checked against a shape the binary was not in. It now reads the
  binary's own identity from `estigia --version`, and the three *cannot tell* branches are unchanged.
  The reported disagreement did not reproduce when re-measured — one `cargo test` links one library
  rlib — and `docs/honesty.md` records that as a fact distinct from the stale-`dirty` flag.
- **The OpenCode loop plugin goes on to the next task instead of stopping with the first.** The plugin continued a session only while `estigia goal` said the run still held its issue,
  so a completed task ended the loop. It now continues while the run holds a live claim **or**
  another issue is claimable by it: `estigia goal` gained a `claimable` count — the unassigned issues
  in the `review` and `ready` states, summed, read through the transport's own `list-state` and only
  when the run is not live — and the plugin's exits are that reading and the `--max` cap. A queue
  that could not be read answers `null`, never zero, so the loop stops on the fault rather than
  reading it as no work. `docs/what-it-writes.md`, `docs/configuration.md` and `docs/honesty.md`
  state the new condition and what the plugin still cannot prove.
- **A fresh installation ships the balanced posture, in one word.** The portable table
  shipped `ask` for all three authorisations, so an installation that wanted Estigia to work
  unattended moved five rows by hand and had to know which five. The new `Profile` row names the
  whole posture — `balanced` makes `Delivery authorisation`, `Review delegation` and `Transition
  authorisation` `auto`, and `cautious` makes all three `ask` — and is derived over those rows
  rather than stored, so an explicit authorisation written after it wins. `Config::default()` is the
  balanced posture now. `Blind judges` has been removed and the panel is fixed at two judges
  (one sealed, one unsealed) at `medium`/`high` and one at `low`, so the judge half of the issue has
  no row to carry. On the model half, where `Model routing` is unset and the adapter has a reviewed
  `balanced` preset, `setup` renders that preset's phases into the definitions instead of `inherit`;
  under `cautious`, and for every adapter with no reviewed preset, the definitions stay `inherit`.
  A reviewed preset never installs a delegated role — only the operator's own `Model routing` names
  one — and `analyst` now joins all six presets at each preset's own `judge` tier. `review-blind`
  stays the literal `model: inherit` its byte-comparison and reserved-role uniqueness depend on.
  An installation that carries its own table keeps every row it wrote; a table missing a row takes
  the new value on the next read.
- **The OpenCode loop plugin is an operator setting, not a setup flag.** The optional
  loop plugin was reachable only by a `--loop` flag on `estigia setup`, which the TUI never showed
  and `scripts/build-install.sh`'s non-interactive `setup --all` could not reach. It is now the
  `Loop plugin` row — `on`/`off`, default `off`, agent-scoped — edited in the TUI and honoured by
  `setup`, which reads the effective setting instead of a flag. The row is offered for OpenCode only
  — the one adapter whose `has_loop_plugin()` is true — on the setup screen's per-agent step; a
  `config edit` whole-table view shows every row of the table, as it does for every setting.
  `uninstall` still removes the plugin
  by reading its path, and a foreign `estigia-loop.js` is neither replaced nor removed. The flag,
  its `conflicts_with = "all"`, and the `loop-plugin-unsupported` refusal are gone, so nothing
  names a command that no longer exists.
- **OpenCode gets an optional loop plugin, and Estigia adjudicates its completion.** When the `Loop plugin` row is `on`, `estigia setup opencode` writes `estigia-loop.js` beside the
  gate plugin, marked `estigia:loop`; the default `off` writes nothing new. The plugin continues a
  session while `estigia goal` — a new read-only verb that answers
  whether this session's run still holds a live claim, reusing the gate's one identity rule and
  naming `estigia goal` in its own refusals — says the run is live. Completion is never a value the
  agent supplies: the plugin reads no `<promise>`, takes no done input, and its only exits are that
  verb's answer and an optional `--max <n>` cap; an answer that could not be read stops the loop
  rather than continuing it. `status`'s OpenCode continuation word becomes `able: loop plugin, when
  installed`. A `node`-driven test proves the three exits, `docs/what-it-writes.md` states the
  optional install and how uninstall takes it back, and `docs/honesty.md` states what the loop does
  not prove — that the host fired the event, that the enqueued prompt was delivered, and that the
  agent did any work between turns.
- **`Model routing` is two sections, and a role's presence in it is what installs it.** The flat `key=model` row mixed four kinds of target — orchestration, planning phases, roles Estigia
  installs, and names another orchestrator's — and hid the effort behind a slash inside the model
  value. The row now presents a phases section and a roles section, and the `Delegated workers` row is
  deleted: naming a role in the roles section installs its definition and removing the entry retracts
  it. Model and effort are two separate lists in one cell — models first, then, after a `;`, the
  efforts — so an effort is its own decision and a target may run on the inherited model at a named
  effort. `Role` is `developer`, `analyst`, `reviewer` and `judge`, with `Role::installs()` true for
  the first two, so `analyst` routes from Estigia's own vocabulary and `ORCHESTRATED_ROLES` — with the
  five names Estigia never spawns, `strategist`, `builder`, `refactorer`, `validator` and `auditor` —
  is gone. A routing key this build no longer accepts is dropped by the tolerant read with a report
  naming it and, where a rename removed it, its replacement; a retired `Delegated workers` row folds
  its names into the routing as `inherit` and is reported the same way, so no configured value
  disappears in silence. Twenty-two typed settings. Reading the table produces a valid
  configuration or a refusal that names what may be written instead. `docs/configuration.md`,
  `README.md`, `docs/honesty.md`, `skill/SKILL.md`, `skill/references/runtime-notes.md`,
  `docs/what-it-writes.md` and the interactive surface move with it.

- **The direct route's review granularity is written down: the candidate is the publication.** `SKILL.md`'s `Change size` rule and `skill/references/repository-delivery.md`'s review-unit
  section now state that a commit is not a review candidate and that no review is due before a
  publication — a verdict binds the epoch, PR, head, base and digest `publish_review` mints, so
  there is nothing to record one against earlier, and reviewing a unit on its own would mean
  publishing it on its own, a delivery of several pull requests this contract does not take. No new
  read is added: `review_tier`, `review_tier_for_repair` and `repair_rounds` answer the questions
  that exist. This settles the last difference `docs/parity.md` listed.

- **`Review protocol` defaults to `receipt-driven`.** The default was `standard`, the
  weaker of the two protocols the row admits, while `skill/protocols/rdd.md` argues that review is
  not switchable off and that a verdict binds the bytes it was given rather than the commit SHA that
  happened to carry them — a mode change, a symlink, or an uncommitted edit all ship without moving
  the SHA. gentle-ai, the project this crate measures itself against, ships receipt-driven on by
  default. A fresh table now starts there and the default install carries `protocols/rdd.md`; an
  installation whose table already names `standard` keeps it, because the default moves for a table
  Estigia writes, not for one it reads back. `docs/configuration.md` names the default on the row.

- **The delegated worker `implementer` is renamed `developer`.** The worker definition
  (`skill/agents/developer.md`), the `Delegated workers` value, the `Model routing` key, the
  `record_spend` phase vocabulary (`SPEND_PHASES`, now `review|developer|analyst|orchestrator`) and
  the example topology label all take the role's own word. `implementer` is refused rather than kept
  as a second spelling: the row is a closed vocabulary, and the `invalid-spend-phase` refusal names
  the four words it takes. A timeline whose records predate the rename keeps the old phase word —
  `SpendRecord::from_marker` still reads a marker under the word it was written with — and
  `issue_totals` still sums it; nothing rewrites a marker. `docs/honesty.md` and `docs/review-cost.md`
  keep the past-tense uses that name what a past run recorded.
- **A non-severe finding is informational: `release_ci` no longer owes a disposition for a `warning`
  or a `suggestion`.** Three shipped surfaces disagreed about the same class: step 5 of `skill/SKILL.md` gave
  every `warning` and `suggestion` one `record_disposition` answer — `acted-on`, `filed` or
  `declined` — while `skill/policies/blind-judges.md` and the `elective-repair-refused` row of
  `skill/bindings/github.md` still said a non-blocking finding "owes its own tracker issue" and was
  "delivered and filed", and `release_ci` refused `disposition-missing` until every one was answered.
  `gentle-ai` v3.7.0 (`internal/reviewtransaction/advisory_disposition.go`, read at `e2cb2bf2`) states
  the direction now followed: a `WARNING` or `SUGGESTION` is `informational`, "requires no action of
  any kind", and is "never a reason to re-run review on this candidate" — recorded 2026-08-16 in
  `92778f7d`. `release_ci` no longer reads a disposition; `record_disposition` remains for a run that
  chooses to record what it acted on or filed, is now optional and may answer a subset, and refuses
  nothing for a finding left out. `skill/SKILL.md` step 5, `skill/policies/blind-judges.md`,
  `skill/bindings/github.md`, the tool descriptions and `docs/honesty.md` state the one rule, and the
  documents move with their registered byte ceilings in `tests/instruction_ceiling.rs`. The one gate
  this removes is named where it stood: `disposition-missing` is retired, and a `severe` finding whose
  `causal_disposition` reads `pre-existing` or `base-only` still owes its own issue, because no gate
  blocks on one.
- **A delivering run puts its own checkout down, and the merged branch goes with the merge.** `release` — the one step the delivery sequence runs after the close — now removes the isolated
  worktree and local branch `start_branch` created, and only on proof: the tracker must recognise the
  issue's close as this run's own delivery (`own_delivery_close`: the run's `published` marker, the
  closing pull request, and a merge whose parents are the reviewed base and head), the checkout's
  head must be exactly the delivered head, and `git status --porcelain` must be empty. Anything short
  of that is kept, and the release answer names the check it failed. The gate is not widened to reach
  this: a closed issue still refuses the run's own destructive commands, and the checkout is
  Estigia's own artifact, put down by the operation that owns it. The remote half belongs to the
  host: `delete_branch_on_merge` is on for this repository, and the merged head branches that had
  accumulated were swept — 165 remote, plus 27 local worktrees with their branches, returning about
  64.7 GB to the worktree volume. `tests/pipe.rs` drives both directions end to end.
- **A base movement classified `compatible` is spent as a repair-shaped republish rather than a full
  panel, and a round the base forced is not charged to `Repair ceiling`.** Three surfaces
  contradicted each other about the same event. The transport classified later base movement and
  answered `"integrate": false` for a compatible movement — disjoint paths, conflict-free merge —
  while the routed contract spent every classification as a full re-freeze and re-review, and the
  delivery assumes `required_status_checks.strict: true`, under which GitHub requires the branch to
  be up to date before merging, so the update is forced and `integrate: false` is not advice a run
  here can take. The dispatch question
  settled the issue's proposed alternative first: `workflow_dispatch`'s `ref` must be a branch or tag
  name, so a merged tree that is not one would have to be pushed — the integration the route existed
  to avoid — and the fallback became the live shape. What changed: the elective-repair gate now reads
  the two recorded base tips rather than the push shape, so a merge of the moved base is publishable
  over an accepted parent exactly as a rebase was; `repair_rounds`'s `author_elected` excludes a
  publication whose base differs from its parent receipt's, because the platform elected that round
  and not the author; and `classify_base_movement`, `skill/protocols/rdd.md`,
  `skill/references/repository-delivery.md` and the bindings now state one policy — `compatible`
  integrates into a delta read of `parent-head..head` with the parent receipt's findings carried to
  the panel, every other value keeps integration plus whole-change re-review, and `unknown` is still
  never compatible. A moved head still invalidates through `stale_verdict` with no exception added,
  and this change neither sets nor reads back `strict`. The price record in `docs/honesty.md` is
  updated with what the decision changes and what it leaves unbounded — the ceiling no longer
  bounds a base-movement storm, and the honest bound there is the base itself — and it records the
  live measurement that the setting is off today.
- **A run id is held to the one shape `session::run_id` mints, and `create`'s runtime to the same
  component, so no label is minted from text this crate could not have written.** `codex-01k4-9f2a` — what the contract's own instruction
  produces on a host whose session ids carry a dash — passed the alphabet check because the last-dash
  split reads the well-formed runtime `codex-01k4`, and minted the permanent repository-level label
  `dev:codex-01k4`, which `gh issue list --label dev:codex` never finds. Every run-id refusal read
  `Value::as_str`, so a JSON number answered `None` to all four at once and `flags_for` rendered it
  into `--run-id`, where `verify_claim`, `heartbeat` and `transition` reach the tracker with no
  runtime at all. `create`'s `runtime` was a required free-form string no check reached before
  `analyst:<runtime>` was created. `session::run_id_shape` now answers the whole question once —
  named, nameless, or which part is not the minted shape — `run_tool` reads it from the argument
  (a non-string is a malformed call) and derives the projected runtime from the validated answer,
  and `create`'s declared runtime goes through `session::runtime_component`, the same function the
  derived one uses. One crossing test runs every tool that takes a run id against the predicate's
  verdict over the class, not over the spellings. **Decisions:** a dashed suffix is refused, not
  derived — nothing can tell a dashed runtime from a suffix that carried one — and a suffix after
  the last dash is admitted only when it is the minted shape: eight to sixteen ASCII alphanumerics
  ending in eight lowercase hex digits, with the part before the dash a runtime component. The
  residual this leaves is recorded with its measurement in `docs/honesty.md`; `flags_for` keeps
  rendering a number as text for every other `string`
  argument, because the two values the gate reads rather than renders are the two it now reads as
  arguments; labels already minted are left alone, since this repairs what is written next rather
  than migrating the residue.
- **`build.rs`'s envelope carve-out ends where its extractor stops reading, and two sentences that had become false are cut or corrected.** The guard that clears a non-literal `"reason":` in `Failure::envelope` ended its window at the next
  column-zero `fn` while `extract_constructed` stopped at the function's first four-space closing
  brace — in `src/transport/mod.rs` those two ends sat seventy-one lines apart — so a `"reason":`
  written between them cleared the guard and reached no vocabulary, which is the collapse the guard
  exists to refuse. Both now read one window, derived once in `envelope_window`; measured by writing
  such a `"reason":` into `mod.rs`, where the build succeeds at this branch's merge base and aborts at
  this head. `AGENTS.md`'s sentence claiming that `build.rs` reads the transport's sources without
  stripping them — false now that it strips once at the read — is cut, and `docs/honesty.md`'s account
  of the eleven reasons that joined its undocumented list at that point now decomposes them as the eight
  `operation_marker`'s two `format!`-built expressions spelled and the three of
  `branch_identity_verdict`'s four incoherent verdicts that `checkout_identity` passed through a
  `&'static str`. `tests/guards.rs` holds the guard and the extractor to the one window, and
  `three_heads_have_to_tell_one_story` already pinned the four verdicts the corrected sentence
  counts.
- **`estigia config edit` and `estigia config forget` are gated as the harness-disarming writes
  they are, and the crossing that should have caught them now reaches subcommands.** Between them they cover what the listed `estigia config
  set` writes — `edit` the installed contract, `forget` the repository's own file — and neither
  matched any population, so the gate answered `Untouched`: `Outside(NotWatched)`, no gate at all
  rather than a weaker one.
  Measured through `estigia gate` before the change, `estigia config edit` answered `no boundary
  matched this command`; after it, both carry their own fragment and the claim is verified. The
  alias crossing walked top-level verbs only, so the next writing subcommand would have gone
  unchecked the same way: it now walks the parser's own tree, holds every leaf below a verb the
  disarm list names to that list or to the new `DISARM_READS` declaration (`config list` and
  `config repos`, the two that only read), and fails on a leaf that is neither — a third writing
  subcommand is a decision rather than an omission. A test drives the exact case over one tree
  state: `config set` and `config edit` both reach the tracker and are denied by its answer.
  `docs/honesty.md` records what the widened crossing still does not reach: a new top-level verb.
- **`publication_lane` answers `disabled` when the repository cannot run Actions, and delivery
  states what it records instead.** A head with no check runs used to answer `absent`
  whether no lane was configured or Actions was switched off, so a run delivering where CI could
  never run had to decide from the repository's own prose — measured on another repository, where one run
  held the merge for a day while five PRs merged with zero CI. The advisory answer now reads
  `repos/<owner>/<name>/actions/permissions` and the workflows' `state` when the check-run listing
  is empty: `disabled` when `enabled: false` or every workflow under `.github/workflows/` is
  `disabled_manually`, `absent` when nothing proves it, and an Actions surface that does not
  answer is a failed read rather than `absent`. `delivery.md` names the three cases where CI is
  not a delivery requirement, what the merge rests on in each, and the read the run posts on the
  issue before merging. The gate is unchanged: `record_review_verdict` still clears an empty
  check-run listing, and a run that executed and failed or was `cancelled` still refuses
  `publication-lane-red`.
- **BSD `script`'s trailing command is read by the gate.** util-linux's `script -c '…'` quotes its command, so the earlier fix read it through the code
  flag; darwin's `script` has no `-c`: `man script` gives
  `script [-aeFkqr] [-t time] [file [command ...]]`, and the trailing words are the command.
  Measured on 2026-09-27: `script -q /dev/null echo BSD_SCRIPT_RAN_A_COMMAND` printed the
  command's output, while `script /dev/null rm -rf src` classified `Untouched` — a deletion
  with no tracker read and no boundary, on darwin. `script`
  now sits in `RUNS_ANOTHER_COMMAND` as well as `INLINE_CODE`, so the wrapper reading takes any
  later word that names a write. The class is exact: a read naming no write stays `None`, and a
  read whose typescript file names a write is `Some`, at the one-tracker-read cost the module
  always picks; the quoted spelling and bare `script` keep their answers, and a test reddens when
  the new entry is removed. `docs/honesty.md` records what this leaves: a typescript file named
  after a write reads as one.
- **Five more refusals reached after their own write stop claiming nothing was written, and a retried
  operation is told the write had landed rather than which call made it.** `claim-operation-no-longer-current` is reached when the claim marker
  was read visible and then left the timeline before adjudication read again, and the projection
  loop's four stops — `ownership-changed-projections-repaired`, `holder-runtime-missing`,
  `ownership projection write did not read back exactly` and
  `ownership-kept-changing-before-projection` — all run after the caller's control marker has been
  posted and read back. All five now carry `"world": "committed"`, so `translate` answers *the write
  landed; what failed came after it* with `StatusRequired` instead of *nothing was written*.
  `resumed_reclaim`'s `reclaim-operation-no-longer-current` declares the same way, so a retried claim
  and a retried reclaim answer one retry case alike, and `Answer::already_wrote`'s contract and the
  sentence it delivers now say *this operation*: a retry resuming an earlier call's marker is right
  to report `Committed`, because what the flag reports is that the write had landed, not which call
  made it. Each repair is driven red first and mutated back to `NotStarted` on its own
  test. `docs/honesty.md` records the decision on the standing gap — no test or guard can enumerate
  the stops reachable after a write, because reachability is a property of the interleaving — and
  names the sites this sweep found and left silent rather than implying the class is closed.
- **A takeover the ownership fold discards says which condition dropped it, instead of reporting a
  race it never ran.** A takeover the ownership fold refused — a malformed marker, forced
  evidence that did not bind to that takeover, a comment the tracker did not attribute to this run or
  that was edited, an operation id first reserved at another marker, an epoch the takeover cannot
  displace, a live holder taken without `--force` — answered `lost-reclaim-race` naming the old
  holder, which reads *somebody got there first* and was true of none of them. It is now
  `reclaim-discarded`, carrying a `condition` field naming which check dropped it and, when a holder
  remains, the run that still holds the issue — the no-holder shape, a target that scrolled out of
  the fetched window while the takeover displaced nobody, omits `winner` because there is nobody to
  name — with `world: committed` because the comment is on the issue; and
  `lost-reclaim-race` is left meaning only that this run's reclaim counted and somebody else still
  holds. The `from-op` lookup searches every live epoch rather than the winner alone, so a live epoch
  that is not the holder is refused by name rather than read as one that does not exist — the
  accept/refuse outcome is unchanged, because a takeover may displace only the holder or, when
  nothing is live, a stale acquisition. The fold decides six of the ten conditions and the reader the
  other four before an event exists. One test per condition drives `holding` and the refusal path
  over rendered markers: replacing a condition's name at its return site reddens that site's test,
  and the malformed arm's precedence over `reserved-elsewhere` is pinned by a marker that is both, so
  reordering the two checks reddens that test alone. The tenth — a legacy takeover, which no caller
  can address by operation id — is tested at the fold.
- **The OpenCode plugin hands the gate the session's identity, so a shared base stops refusing
  parallel work.** Both host generations hand the plugin a session id — OpenCode 1 in
  the `tool.execute.before` hook's input, OpenCode 2 in the `execute.before` event — and the
  rendered module forwarded neither it nor a run id, so every call was adjudicated by which oath
  covered the directory it ran in. A session rooted at the shared base is covered by every live run
  at equal depth — measured on 2026-09-25, two isolated runs holding the same checkout refused
  `~/.zshrc` with `several-runs-hold-this-checkout` — including writes naming no checkout at all,
  because a target outside every pointer-recorded checkout falls back to the launch directory.
  `estigia gate` now takes `--session-id` with `--runtime` and derives the run id through
  `session::run_id`, the identity the Claude hook already mints; both hooks forward the session id
  they were handed, and a call the host gave none keeps the checkout road, with `ESTIGIA_RUN_ID`
  as the fallback when neither names one and a forwarded session id superseding it. A claim sworn
  under a hand-minted id is not found by the derivation and its calls are answered `outside`, which permits —
  `docs/honesty.md` records that trade, and the contract's Runtime identity derivation pins the shape
  an agent must mint.
- **The gate refuses a round nobody elected, and the Stop hook stops paying for a poll.** `publish_review` and `republish_review` over a parent on the same
  pull request whose latest verdict was `accepted` are refused `elective-repair-refused` before
  anything is pushed, unless a `review_finding` has been recorded against that exact parent receipt
  since the verdict — the author's declaration of a defect the panel did not weigh — or the base moved
  and a leased republish follows a rebase onto it. That is the shape `repair_rounds` has counted since
  it was written and the contract forbade in prose alone; measured over the baseline's 62 issues, 40
  of 192 publications followed an accepted verdict, most of them repairing warnings, and issues that
  reached three or more rounds took a median 116 minutes against 34–55 for one or two. And `Stop` no
  longer forces a waiting turn: a run whose only remaining work is a background launch is let go.
  What counts as a launch is the host's own record that it started — a `toolUseResult` carrying a
  non-empty `backgroundTaskId`, or one whose status is `async_launched`, the flagless shape Claude
  Code 2.1.x writes for a background subagent. The text of a result is never read, so output that
  quotes the notice cannot become a launch. It has reported when a later record carries a
  `<task-notification>` naming its `<tool-use-id>`. A call the host refused reports neither and is
  not a launch, so a run whose launch never ran keeps the block; an absent or unreadable transcript
  keeps it too, and a report this build does not recognise leaves its launch unanswered and ends the
  turn. `docs/honesty.md` records what neither reader proves.
  The remaining acceptance items — the tool answers' size with byte counts, the implementer brief's
  anchors, the round-trip measurement, the lens route for a post-acceptance repair, the orchestrator
  spend record and the re-measurement against the baseline — stay outstanding.
- **The `blocked` qualifier says what it means, and the pointer names a step that exists.** Three contract sentences were only correct read beside their neighbour, as a blind reviewer found.
  The decision-gate row, `repository-delivery.md`'s handoff paragraph and `safety-incidents.md`'s I09
  row scoped `blocked` to adjudication "outside" the ordinary gates, while the flagship input — two
  blind judges contradicting each other about the same bytes — arises *at* the review gate; read as
  set exclusion it fell through to "built work awaiting delivery evidence -> `review`", the hiding
  place the rule exists to close. All three now say what the phrase meant: a human adjudication that
  is not a routine wait on those gates. `repository-delivery.md` also sent the reader to "the
  `blocked` decision-wait procedure in `SKILL.md`", a heading that does not exist; it now names
  Developer step 7. `every_named_disposition_uses_a_transport_state` crosses the new qualifier and
  `tests/instruction_ceiling.rs` carries the new sizes. The issue's third defect — "evidence" and
  "permission" in one row — is not repaired in place: the row that carried it is gone from `skill/`
  rather than corrected, and the held-restatement paragraph under `docs/honesty.md`'s
  `## The control surface and what it records` says which copies a test holds.
- **Four actions stop declaring Node 20, and the guard reads every action.** Raising
  `actions/checkout` to `v7` left four `node20` actions printing GitHub's deprecation warning on
  every run — **all four in `release.yml`**, which runs on a tag and cannot be retried by pushing a
  fix, and one of them also in `ci.yml`. `setup-node@v4 → v5`, `upload-artifact@v4 → v6`,
  `download-artifact@v4 → v7`, `action-gh-release@v2 → v3` and `attest-build-provenance@v2 → v3`
  (whose two pinned children were also `node20`) are now on `node24`, with every input each receives
  read from the new major's own `action.yml` rather than assumed. The guard that only checked the
  checkout is now a per-action floor table over every versioned action in both workflows, with a
  completeness check that refuses an action nobody has read. Measured by putting each old major
  back — all four direct actions and a `rust-cache` downgrade, every one of which reddens it — and by
  inventing an unlisted action, which the completeness check refuses; the composite's own major is
  floored too, so a revert to the `node20`-pinning `@v2` reddens, though the commits it pins remain
  unbound and `docs/honesty.md` says so.
- **Two more fixtures stop spelling a skip as a pass.** `repository()` in
  `src/harness/guard/tests.rs` and `repository_with_commit` in `src/test_env.rs` both returned
  `Option<tempfile::TempDir>` and answered `None` when `git` was unusable, so their **twenty-three**
  call sites reported `ok` to every ordinary run having installed nothing — the push guard's fixtures
  most of all, on the one gate that sits below the agent layer. Both now return the directory and
  panic, naming the command that clears it, when git is absent or a git call fails: the same repair
  `tracker_rig` got, and the opposite policy this crate held for one condition. `repository_with_commit`
  also stops tolerating *any* failure — a signing machine or a rejecting hook used to take the whole
  of the acceptance evidence to green having measured nothing. `docs/honesty.md` records what
  closed and what `tests/pipe.rs`'s seven read-only and git skip sites still leave open.
- **An out-of-scope finding goes to an analyst, not to the tracker.** A developer run that
  tripped over a defect its issue did not cover filed it itself, mid-task, with the context that made
  it look important and an interest in its being real; the contract had no rule for the case and no
  bound like triage's *at most one evidenced issue*. The Developer execution steps now say the
  finding is handed to an analyst run — a separate context holding its own claim — which applies the
  analyst contract already shipped; the delegated `analyst` definition, where installed, carries the
  reading and files nothing, holding no tracker tool. The developer files nothing, does not wait,
  and names the handoff and its outcome in its close evidence.
  `openspec/14-out-of-scope-findings/` records the proposal, criteria, design and tasks;
  `docs/honesty.md` records that nothing observes the handoff. No gate, tool or setting changed:
  `analyst` already ships behind the `Delegated workers` row.

- **The gate now reads `CLAUDE_CONFIG_DIR` too.** `setup`, `sync`, `doctor` and
  `config` resolve Claude Code's roots under `$CLAUDE_CONFIG_DIR` when it is set, but the gate itself
  did not: `definition_for` never searched `$CLAUDE_CONFIG_DIR/agents`, so a sub-agent definition
  written exactly where `setup` puts it answered `Ok(None)` — every tool allowed, the loosening that
  asymmetry exists to refuse. `authorize_review_blind_launch` proved the canonical
  `review-blind`/`review-blind-sealed` reviewer against `~/.claude/agents` alone, so a session
  running under a relocated configuration was refused `reviewer-canonical-unavailable` against the
  *other* profile's tree or an empty one — measured live on 2026-09-18 against a real two-profile
  machine. And the control-surface fragments `.claude/settings` and `.claude/agents/` never matched a
  relocated root at all, because they carry the `.claude` directory name inside the fragment and
  `$CLAUDE_CONFIG_DIR` names an arbitrary directory.

  `definition_for` now also searches `$CLAUDE_CONFIG_DIR/agents`, beside the home root rather than
  instead of it. `authorize_review_blind_launch` proves the canonical reviewer against
  `$CLAUDE_CONFIG_DIR/agents` alone when the variable is set, and never falls back to
  `~/.claude/agents` — that tree is the other profile's, and letting it stand in would let a stale or
  foreign file there satisfy or refuse a launch it has no standing over. `matches_relocated_claude_config`
  closes the control-surface gap with a resolved-root check — it reads the variable's actual value
  and asks whether a target lands under it, on one of the names `settings.json`, `agents`, `skills`,
  `.claude.json`, `CLAUDE.md` or `CLAUDE.local.md` — rather than a spelling that would have to name
  every possible relocated directory to be safe. Unset, every classification and every root is
  byte-identical to before. `docs/honesty.md`'s entry on `CLAUDE_CONFIG_DIR` carries the full account.

- **A run that inherits an issue is told which checkout holds its branch.** `start_branch` resolves the worktree path from the run id, so a successor never lands on the
  path its predecessor made — `may_occupy` is asked about a directory that does not exist,
  answers *fresh*, and `git worktree add` is then asked for a branch another worktree already
  holds. Git refuses with `fatal: '<branch>' is already used by worktree at '<path>'` and exit
  128: a sentence and a status, carrying no reason a caller can branch on and no way out. The
  run stands down, correctly, because there is nothing it can be told to do. Measured on
  another repository's #318, where a session whose tool channel died left a checkout behind and every
  later run collided with it and moved to another issue, for a working day, with nothing on the
  timeline saying why. `branch_pinned_elsewhere` now answers before git is asked, naming the
  branch, the checkout that holds it, the path that was requested, and the recovery in
  `bindings/github/recovery.md`. What it does **not** do is adopt the predecessor's checkout:
  the worktree is the unit of task identity here, and resuming another run's directory would
  decide that for the operator rather than telling them.

- **A refusal that wrote before it refused says so.** `adjudicate`'s `lost-claim-race`
  and `won_the_reclaim`'s `lost-reclaim-race` are reached only after this run's own control marker is
  on the issue, and their payloads carried no world, so every caller that lost a race was told the
  world was untouched while its marker sat on the timeline — invisible to it. Both now carry
  `"world": "committed"`, which `translate` reads as *the write landed; what failed came after it*
  with `StatusRequired`, and the process exit code follows `exit_code_for` rather than the code
  alone: `1` no longer means nothing was written, so `ExitCode::Refused` and the README's exit-code
  table defer to the refusal. `docs/honesty.md` records that nothing enumerates the class, and what
  the mis-reporting cost on another repository's #318.

- **`Publication lane` can be switched off.** `on` (default) keeps
  accepted verdicts gated on the head's check runs; `off` skips the check-run
  read, answers `classification: not-consulted` on the verdict, and makes
  `publication_lane` answer the same. A disabled workflow's `HTTP 422` stays
  `Absent`. Cancellation is still not never-ran clearance when the gate is on.

- **A sealed reviewer's copy is now crossed against what was published, rather than trusted on say-so.**
  Nothing anywhere read a candidate reviewer-facing text against the payload
  `publish_review` actually bound. Measured against a delivered publication: four of six
  sampled lines this run had handed a sealed judge did not exist in that receipt's payload — multi-line
  enum declarations collapsed onto one line and annotated with prose that appears nowhere in the
  repository, in the same brief that told the judge no retyping had happened. `review_payload_membership`
  reads a candidate from a path, never inline, recomputes the payload from the resolved receipt's
  `base`/`head` with the same call `publish_review` made, and reconciles the recomputed digest against
  the marker's own recorded `payload-digest` **before** testing anything about the candidate. A mismatch
  answers `payload-rebuild-mismatch` and reports no line — a disagreement cannot tell an altered
  candidate from an unfaithful rebuild, and naming a line there would be reporting the nearest named
  state instead of the unknown one. Only once the digests agree is the candidate tested
  for **membership**, never completeness: every non-empty line must be a line of the payload, so
  withholding whole files to fit a sealed reviewer's limit stays legitimate. The first line that is not
  answers `payload-line-not-found`, naming its position; a passing answer carries `omitted`, so a brief
  can state its own subset honestly.

- **The delta a re-review reads absorbed the base after a rebase.** Four shipped
  surfaces named `parent-head..head` as the range a repair's re-review should read, with no
  condition attached. Once a branch is rebased onto an advanced base that range also carries every
  commit the base gained, and nothing in it marks which half belongs to somebody else. Measured on
  a real repair: 1,205 insertions where the repair was 562. Taking the merge-base instead
  — the repair anybody reaches for first — answers 2,991, so it is worse than the defect.
  `review_tier_for_repair` already decided this correctly and nothing else could ask it; the
  decision now has one home, `repair_range`, and a publication calls it and records the answer as
  `delta-range` or, when no honest narrow range exists, `delta-range-reason` naming
  `history-rewritten` or `empty-repair-range`. The lineage reads both back, so a marker written
  before they existed still parses. Deleting the ancestry branch answers `low` where the same
  input is `medium`, which is the under-rating the check exists to prevent.

- **A guard that decides was never shown able to say no.** `review_tier` derives its
  level twice, from the diff and from the reasons it publishes, and refuses the answer when the
  two disagree. That refusal shipped with the feature, and its message occurred once in the whole
  tree — at the production site — so nothing had ever made it fire. It still cannot through the
  real path: both derivations turn on the same event, which two contexts derived independently and
  a fourteen-shape enumeration now pins, so an edit that desyncs the two classifiers on one of
  those shapes reddens in the suite. The enumeration and production now share one
  `passive_and_configuration`, so an edit to how `review_tier_answer` composes the inputs is caught
  by the tests that drive it end to end rather than by the enumeration. The check is extracted so a
  test can hand it a pair the real path does not produce, and deleting its `if explained != level`
  block reddens that test. Its `Population` declares `FailClosed` and states the boundary as it is:
  no reaching input is known to exist.

- **A verdict could rest on the word `n/a`.** `record_review_verdict` refused a
  `basis` that was empty or whitespace-only and accepted `n/a`, `TODO`, `pass` and `placeholder`,
  recording them as the grounds a verdict rests on; a finding's `evidence` and `impact` were
  required to be present and never examined. One predicate now decides all three, matching the
  trimmed and lowercased value against eighteen words. Two of the eighteen removals were
  independently re-run and both reddened; the other sixteen are the implementer's report. The
  match is exact equality over eighteen distinct strings, so removing any one leaves that word
  alone uncaught. The list is declared `TooTight` in its own `Population` because filler is
  unbounded and a remembered set has recurred here before, so `tests/guards.rs` reopens the claim
  when it changes. **No length floor**: the contract says measuring nothing is a legitimate basis
  when stated plainly, so `nothing was executed` stays accepted — asserted against the predicate
  and carried through the real MCP call end to end. A finding's `id` keeps the plain blank check,
  being an identifier rather than a declaration.

- **A reviewer was required to state a field its own instructions never named.** `record_review_finding` refuses a `severe` finding that is new to a repair round and names no
  parent unless it carries `origin` — `introduced` if the repair created the defect, `exposed` if the
  repair made an existing one reachable — and not one of the seven reviewer definitions that record
  findings mentioned it. `causal_disposition`'s five values were taught and `origin`'s two were not;
  the word itself appeared zero times in any of the six templates. The orchestrator filled it in,
  which put the publishing run's causal attribution inside the reviewer's own record on the one
  question where the publishing run is the interested party. Both judge seals render from one
  template, so a single sentence reaches both; the same sentence went into the five sealed lenses,
  because a lens may record `severe` and the refusal reads the finding's class and lineage without
  distinguishing which reviewer produced it. `review-refuter` is excluded deliberately — it answers
  through `record_refutation`, which carries no causal field. A test renders all seven and crosses
  them against `FINDING_CLASSES`, `FINDING_ORIGINS`, `FINDING_EVIDENCE_CLASSES` and
  `FINDING_CAUSAL_DISPOSITIONS` read from the `pub const`s rather than retyped — against each
  definition's own bytes, with the number of definitions walked pinned. (The rendered-body half of
  the vocabulary crossing was removed because it could never fail where the template
  half passed.) It reddens when the sentence is removed and when a value spelled
  unlike anything already in the document is added to a vocabulary; both were run. It is a presence
  check and not a structural one, and `docs/honesty.md` states the three things that follow —
  including one measured rather than reasoned: removing the `severe` row from the judge's own
  classification table leaves it green, because that word stands at eight other points in the same
  template. Six previous bodies are frozen into the superseded-template ledger,
  without which `setup` would have refused every existing install as changed. Nothing about what a
  value means, or when the transport requires one, moved: this is the definition catching up to the
  transport.

- **`mandatory_follow_up` named `transition --to done` and stopped there, one step short of the
  contract's own close-and-release — until now.** `check_closing_keywords`'s branch-link
  answer (`src/transport/commands.rs`) and `publish_review`'s branch-linked receipt
  (`src/transport/claim.rs`) each restated the post-merge follow-up in its own prose, and both were
  silent about `release`, which `skill/SKILL.md`'s step 6 names as last and not optional. A field
  called `mandatory_follow_up` is read as the complete follow-up, and a `done` not followed by the
  release leaves a pointer the push guard then refuses as
  `several-runs-hold-this-checkout`. Both sites now emit one `FOLLOW_UP` constant
  (`src/transport/closing.rs`), which names the release and says what it does: it puts the issue
  down on the tracker and drops the local pointer this run wrote. One constant, where there were
  three separate texts of one rule — the third being `closing.rs`'s own module doc, which now names
  the constant instead of restating it.

- **A second spelling of a gated verb reached the same commit unwatched — until now.** `IRREVERSIBLE` matched seven exact literals and `git pull` was not among them, though
  `git pull --no-rebase` performs the merge internally. It classified `Action::Untouched` — the gate
  standing aside entirely — so a run refused on one spelling reached the identical commit by writing
  the other, which is what happened once and was reported rather than done quietly. The
  fragment now joins `IRREVERSIBLE` and **deliberately not `DELIVERS`**: it reaches a boundary, so the
  claim is verified and the line cannot ride the renewal window, but `stale_verdict` and
  `out_of_phase` do not read it, because pulling is an integration step rather than a landing one —
  `DELIVERS` holds the spellings that end a delivery — and refusing it would refuse the contract's
  own *integrate before publishing* default. **The substitution still
  completes**, and `docs/honesty.md` says so rather than implying the route is closed; what answers
  that route is the session-cwd attribution defect that produced the original refusal.
  Classifying by direction instead was rejected because it widens: merging `origin/main` into a
  feature branch is an integration but merging `origin/<feature>` into `main` is a delivery, and the
  distinction is not readable from the arguments. Because exact-literal matching guarantees something
  is always unmatched, the six matchers were read and what they leave is enumerated in
  `docs/honesty.md` in two halves — never matched, and matched but never read as a delivery — with
  two further families filed separately rather than declared.

- **A tool answer named no build, and a stale gate could not be told apart from a current one — until
  now.** `gate_age_verdict` (`src/harness/doctor.rs`) already decided whether this
  binary is current against the checkout it runs in, but only `doctor`'s own row surfaced it, and
  nothing runs `doctor` before a gate call. The MCP server's `answer_text` (`src/harness/mcp/mod.rs`) now inserts a
  `build` object — the commit and dirty flag from `src/build_info.rs` — into a successful tool
  answer whose body is a JSON object, both for `mcp__estigia__*` and for `estigia <tool>`, which
  share one `run_tool`. `session_start_response` (`src/harness/hook.rs`) now also renders
  `gate_version`'s verdict into the message an agent reads at `SessionStart` whenever it is not
  `Fine` — a build whose currency could not be shown and one that could not be compared at all both speak. The
  arm that could not establish currency names how many commits the checkout holds that the build
  does not, when that count can be read, through a new `git rev-list --count` helper beside
  `git_is_ancestor`, and says the count could not be read when it cannot. `docs/honesty.md` names what this leaves open.

- **A mistaken blocking finding cost a whole repair round — until now.** A
  `review-refuter` reserved role, sealed and holding no `Read`, adjudicates a panel's own standing
  severe findings before a rejection rests on them. `review_refutation` records the whole batch in
  **one** call — `corroborated`, `refuted` or `inconclusive` per finding, never one task per
  candidate — against one exact receipt, refusing a batch that names a `deterministic` finding
  (that class already qualifies a rejection with no confirmation owed), names an id this receipt
  recorded no severe finding under, names one twice, omits a standing `inferential` one, or is
  credited to the publisher or a requester of this same review. `review_verdict` then refuses
  `rejected` as `rejection-rests-only-on-refuted-findings` when every one of that reviewer's own
  severe findings has been recorded `refuted`; `corroborated` and `inconclusive` leave the rejection
  exactly as permitted as an unrefuted finding does. The floor excludes `deterministic` alone,
  because that is the one class the batch may not adjudicate, and it reads the class per finding
  because refutations are keyed by finding id and two reviewers may share one. `repair_rounds` is
  untouched, nothing was added in the permitting direction, and the confirming pass an `inferential`
  **lens** finding already gets is a separate question this change deliberately leaves open, and
  `docs/honesty.md` records it as open.

- **A second delivery on one line travelled on the first one's label — until now.** A
  command line was classified by the first `IRREVERSIBLE` fragment present *in list order*, and both
  delivery checks then read that one recorded fragment — `stale_verdict` and `out_of_phase` each
  return early unless `DELIVERS` contains it. `git push` is on `IRREVERSIBLE` and deliberately not on
  `DELIVERS`, so `git push && gh pr merge <n>` recorded `git push` and the whole delivery apparatus
  was skipped: no receipt, no phase check, no pull-request identity. The gate answered allow from
  `in-progress` and the shell merged the pull request. `git merge origin/main && gh pr merge <n>`
  from `review` did the same through `git merge`, which compares only the receipt head against
  `HEAD`. The fragment recorded is now the one the strictest check reads: `gh pr merge` first, since
  it is the only fragment carrying a pull-request identity, then the rest of `DELIVERS`, then
  everything else. **A line carrying exactly one fragment is unchanged, in every tier**, and
  `git push` stays out of `DELIVERS` — a push is not a delivery, and widening that list would refuse
  every ordinary topic push. `docs/honesty.md` carries what this leaves open and one thing the
  repair measured that no document said before.

- **A compound shell line answered without the control-surface question being asked — until now.** `REPOSITORY_SHELL`'s arm returned on the first of its own fragments found in a command, before the
  control-surface and repository-configuration questions the two arms below it ask — so
  `git stash; rm ~/.claude/settings.json` rode the renewal window the same way a bare `git stash`
  does, because `REPOSITORY_SHELL` matched `git stash` first and returned before
  `~/.claude/settings.json` was ever read; `&&` and a bare newline in place of `;` answered the same
  way. The arm now asks the same question `shell::writes_a_file`'s arm already asks —
  `surface_of` or `text_names_the_repository_configuration` — before it answers, reporting `Boundary`
  when either is true and `Routine` otherwise.
  `docs/honesty.md` and `docs/configuration.md` carry what this closes and what it does not: a
  directory-shaped removal (`rm -rf .git/estigia`, `mv .git/estigia .git/estigia.bak`) still answers
  `Routine`, unchanged, because neither line names `estigia/{LOCAL_FILE}` or a `CONTROL_SURFACE`
  fragment reaching the bare directory.

- **A frozen manifest too large to record failed after the push — until now.** `publish_with`
  freezes the original candidate's own changed-path manifest onto the published marker as
  `budget-genesis-paths`, `\u{1f}`-joined, and nothing bounded it — the marker lands in a GitHub
  issue comment, which caps at 65,536 characters, and the marker was assembled after `push_to_origin`
  had already pushed the branch and opened the pull request as a draft, so a manifest too large to
  record failed with the remote already moved and no receipt for it. `GENESIS_MANIFEST_CEILING`
  (32,768 bytes, `claim.rs`) is checked immediately after `correction_budget_check` returns and
  before anything remote is touched, and it bounds the **percent-encoded** length the published
  marker would actually carry, not the raw joined bytes: `markers::render`'s safe set is alnum plus
  `-._~:/+@`, so a `\u{1f}` separator, and any other byte outside that set, costs three characters
  once encoded rather than one — a ceiling on the raw join would have permitted up to three times
  what it claimed to bound. Overflow refuses `genesis-manifest-too-large`, naming the encoded length,
  the ceiling and the path count, and never truncates. `docs/honesty.md` says what this still leaves
  open: `topology` and `topology_reason` are caller free text with no length limit of their own, so
  bounding the manifest does not bound the comment.

- **A base that moved after the rebase inverted into the payload — until now.** `publish_with` resolved `--base <branch>` to the branch's current tip and materialised a two-dot
  `base..head` diff before checking that `base` was still an ancestor of `head`. When the base tip
  moved after a branch was rebased onto it and before the call, that diff carried everything the
  base gained in the interval inverted into what read as this change's own deletions — measured
  against every one of this repository's own 270 publications, 6 of which were corrupted exactly
  this way, worst case a receipt claiming 1,338 deletions for a change that deleted 26. `publish_with`
  (`claim.rs`) now runs `git merge-base --is-ancestor <base> <head>` between `clean_target` and
  `review_payload` — before the payload is materialised, and before `review-payload-too-large` could
  measure a corrupted one — and refuses `publication-base-not-ancestor` naming `base`, `head` and the
  `merge_base` between them when it is not. An unreadable answer from `git` is a failed read, never
  treated as "not an ancestor." `docs/honesty.md` says what the audit did and did not settle.

- **A finding is written and no operation reads it back — until now.** `record_review_finding` wrote an observation against a review transaction and nothing answered
  it, which blocked porting a refuter — a reviewer whose whole job is to judge findings it was
  handed — because the transport held no evidence one was ever shown any. `review_findings`
  (`claim.rs`, dispatched as `review-findings`) is read-only, takes no `run_id` and no claim, the
  same standing `repair_rounds`, `spend_summary` and `publication_lane` already have. `--epoch`
  resolves to a **full receipt read off the timeline**, never to caller-supplied bytes — a
  publication this issue's timeline actually recorded, or, failing that, the latest publication's
  own recorded lineage parent, so a parent whose own `published` comment was deleted is still
  answerable — and omitting it answers the latest publication. Findings are matched against that
  resolved receipt, never by a finding's own `epoch` field, which is whatever the finding says it
  is. Structured fields only by default, plus `blocking` (from `is_severe()`) and
  `evidence_bytes`/`impact_bytes`; `--with-text` adds each finding's `evidence` and `impact`
  verbatim, refused rather than truncated past `FINDING_TEXT_CEILING` (32,768 bytes, summed over
  the findings being answered) — the same never-truncate shape `REVIEW_PAYLOAD_CEILING` already
  holds for a review payload — naming `--id` as the way to read one finding at a time instead. An
  issue with no complete publication at all refuses `published-transaction-missing`; a well-formed
  `--epoch` naming neither a recorded publication nor the latest one's own parent refuses
  `findings-transaction-unknown`, a malformed one `invalid-operation-id` before the timeline is read at
  all; an `--id` naming no finding on the resolved receipt refuses `finding-id-unknown` — both list
  what the timeline actually holds. An empty ledger answers `ok: true` with `findings: []`, and an
  unreadable timeline is a failed read, never the same answer.
- **The tier scales the acquired judge panel.** `medium` and `high` still acquire two
  judges, one sealed and one unsealed; `low` now acquires one, the unsealed judge, rather than the
  same two — a `low` change is prose the tier has already proven passive, judged against code the
  diff does not carry, and a sealed judge holds only the diff and a web search, so sealing buys
  nothing there either. `RiskLevel::judges` derives the count from the tier alone, the same way
  `RiskLevel::lenses` already did, and it is **not** the mechanism that was deleted: that
  `RiskLevel::judges` read an operator's `Blind judges` row and could not tell a written row from a
  rendered default; this one reads no configuration at all, because there is no such row left in the
  model. `low` never answers `0`: the source project this binding ports `review_tier` from closes a
  passive review with no reviewer at all, and this repository cannot follow it there —
  `release_ci` requires one distinct accepted `review_verdict` and refuses to credit the publishing
  run, so a `0` would leave nobody able to close the review. Every carrier that stated the old
  absolute — `skill/SKILL.md`, `skill/policies/blind-judges.md`, `skill/references/runtime-notes.md`,
  `skill/bindings/github.md`, `README.md`, `docs/configuration.md`, and the `review_tier` MCP tool
  description — now states the tier's own count instead, crossed against `RiskLevel::judges` by
  `tests/honesty.rs`, which also sweeps every file under `skill/` for the superseded absolute rather
  than trusting the enumerated list alone. `opencode_review_section()` — the panel-launch text an
  OpenCode host reads in place of the reserved `review-blind` type — states the same tier-scaled
  count for its own `Task` launches, and the completeness sweep now calls it directly and checks
  what it renders, since it is a `src/` surface the `skill/` file walk cannot reach.

  **A repair round is sized by its own delta, not by the whole
  change.** `review_tier` used to answer `base..head` on a repair exactly as it did on a first
  publication, so correcting a sentence was sized as the code change the correction sat on top of —
  measured as this repository's single largest review cost (8 judges, 3 rejections, zero code
  defects, on prose alone). The first design, `review_tier_over_trees`, read two frozen tree object
  ids instead, believing that excluded base movement; the panel reviewing it proved
  it did not — a candidate tree is the tree *of a commit*, and `git diff tree(A) tree(B)` is the
  same diff as `git diff A B`, so that pair *was* `parent-head..head` under a different name, the
  exact range an earlier measurement had already shown folding `main`'s own advance into the repair's
  apparent size (1,205 insertions where the repair was 562, on a real repair). Freezing
  the id never froze the range. The replacement, `review_tier_for_repair`, takes commit-ish
  arguments and an ancestry guard: when the parent publication's own head is still an ancestor of
  this one and the narrower range touches at least one path, the answer reads
  `parent-head..head` and carries `range: "repair"`. Otherwise it falls back to the whole
  `base..head` with `range: "whole-change"` and a `whole_change_reason` — `"history-rewritten"`
  when the base moved and the branch was rebased onto it, which branch protection here forces
  whenever `main` advances during review, or `"empty-repair-range"` when the narrower range
  touched nothing at all. The second review round on it rejected the first shape
  of this guard too: `git merge-base --is-ancestor A A` exits `0`, so an identical-bytes republish
  (`parent-head == head`, a real input the contract itself names) made the ancestry check alone
  wave through an empty diff and derive a tier from nothing — a probe answered `low` over the
  reflexive range where the whole change was `high`. And the `base_advanced` boolean the first
  shape answered with was false in two further histories an amend and a merged-in base advance
  each produce; a boolean claiming an untested base movement could not be patched into honesty, so
  it is gone, replaced by the range actually measured and, only on the whole-change path, why. In
  the narrow case the answer's `"base"` field now carries `parent-head`, documented as such. The
  claim that this guard mirrored `correction_budget_check` is also gone: that function tests
  different operands and disagrees with this guard on the merge-from-base case, so it never
  mirrored it. So the saving does not apply to a rebased or an empty repair round, and both are
  ordinary here — a real bound on this issue's value, named rather than softened.
  `skill/SKILL.md` developer step 5 now states which range to read and when — it named none
  before, which is the whole reason this went unnoticed — and `skill/bindings/github.md` gains a
  `review_tier_for_repair` row beside `review_tier`'s own. `docs/honesty.md` names what is still
  not closed: nothing verifies that an orchestrator reads the narrower range on a repair rather
  than the whole change, the same way nothing verifies the judge count itself ran.

  **The completeness sweep is now a mechanism, not a list.** The four-phrase sweep above missed all
  three prose severes the same panel found, because each restated the deleted "panel is never
  reduced" absolute in different words. It is replaced by a sweep over every `skill/**.md` and the
  rendered `opencode_review_section()` for an absolute word — `always`, `no reduced panel`, `both
  judges`, `two contexts` — within 120 bytes of a panel noun, catching a new absolute by default;
  only a specific, enumerated occurrence with a stated reason for still being true is let through.

- **A row that decides nothing here now says so.** `Applies::Inert` had no producer —
  `AgentAdapter::applies` derived every row from `can_gate_tools()` alone, so `Delegated workers`
  read `Held` even on adapters with no host-routable agent-definitions surface, where the row's
  entire effect is an installed agent definition. `AgentAdapter::agent_definitions_root`
  now names that surface — one path segment list for Claude Code, `None` for the other twelve,
  matched over the instruction-file enum so an added adapter has to answer `None` on purpose —
  and `paths_in`'s own `agents_root` match is retired in favour of the same method, joined onto
  the resolved home. `applies()` checks it **before** the gate check: most adapters that can gate tool
  calls still have no agent-definitions root, so the gate check alone would have swallowed the new
  arm on exactly the adapters it exists for, and a test now pins that ordering: moving the check
  after the gate reddens it. `Model routing` and `Planning` are unchanged — the first is
  applied by hand regardless of whether a host reads it back, and the second selects contract prose
  every adapter receives. `Evidence standard` stays `Held` on every adapter, rootless or not: it is
  `Scope::Everywhere`, and the gate renders and enforces the reserved reviewer's grant from the
  embedded template regardless of any installed file, so an earlier version of this fix that answered
  it `Inert` there too was wrong — `Setting::reaches_only_an_installed_agent_definition` now names the
  one row whose whole effect is a file, written as an exhaustive match so a row added later cannot
  fall through it silently. `docs/what-it-writes.md` and `docs/honesty.md` carry the per-adapter table
  and the narrower claim this leaves standing about the grant's own enforcement. A second finding
  followed against the same producer: on the setup wizard's shared "every agent" stop, `App::applies`
  still asked `self.adapter()` — whichever adapter the cursor had last visited before the walk reached
  that stop — while `set()` writes the answer to every ticked adapter under `self.uniform`. A row inert
  only for that one last-visited adapter refused an answer the write still reached everywhere else.
  A repair for that finding answered `Held` outright once `self.uniform` was true, which fixed the
  refusal but asserted an enforcement no adapter's own `applies()` grants: `Review` is `Applies::Asked`
  on all thirteen adapters unconditionally, and the shared stop was reporting it plainly `Held`,
  dropping the caption naming the gap that answer exists to keep. `App::applies` now asks every
  adapter the write reaches (the same set `model_destinations` writes to) and answers their unanimous
  result — `Inert` when every one answers `Inert`, the shared `Asked(caption)` when every one answers
  that same caption, `Held` when every one answers `Held` — and falls back to `Held` only when they
  disagree, since the shared contract is read by every one of them and none may call a row worthless
  for the whole stop. That fallback does overstate for a member of a mixed set that only answers
  `Asked`; `docs/honesty.md` names the imprecision rather than hiding it.

- **The correction budget ports as the source's own machine: two frozen Git tree object ids, never a
  payload and never a mutable ref.** `publish_review`/`republish_review` now freeze
  `budget-genesis-tree`, `budget-genesis-paths`, `budget-original-lines` and `budget` on a repair's
  first round, from the original candidate's own merge base rather than its recorded (and possibly
  advanced) base; every later repair only grows `budget-cumulative`, read back from the parent
  marker rather than recomputed. Admission runs before arithmetic: a repair may only touch the
  frozen genesis paths or add a companion test (`_test.rs`, or anything under a `tests`-shaped
  directory — this repository's own adaptation, since the source project's own table has no Rust
  row), refused by name as `correction-outside-reviewed-scope` otherwise; growing the cumulative
  total past the frozen budget refuses `correction-budget-exceeded`. When the base has moved past
  what the parent publication's own history contained — merged forward or not — the lineage
  re-freezes from this publication's own ends instead of charging the advance, and
  `budget-refrozen` records that it happened. A first publication is never budgeted. See
  `docs/honesty.md` for what this port does not prove. **Two fixes from the first blind panel's own
  reading:** the budget is now scoped to the pull request, not the issue — a fresh pull request
  opened after a refused one is never budgeted against the abandoned candidate's frozen genesis,
  the same exclusion `repair_rounds` already applies to the round ceiling; and the tree-to-tree
  measurement now runs in an isolated `GIT_DIR`, immune to `.gitattributes` wherever it is
  declared — committed, uncommitted in `.git/info/attributes`, or in ambient global/system
  config — since a `-diff` attribute marks a path binary and `count_changed_lines` skips binary,
  which silently defeated the budget entirely and looked like it was working. Ports
  `isolatedImmutableTreeGit` (`frozen_candidate_context.go:319-387`). **Two more from a second
  panel, the same two identities found independently and split on class, resolved by measurement
  rather than by vote:** the isolation above still layered its environment onto the caller's own,
  so a caller's own *default* global attributes file — resolved from `HOME`/`XDG_CONFIG_HOME`,
  invisible to every variable the isolation named — still silenced the budget; the measurement now
  reaches git with the environment replaced instead, closing that file along with
  `GIT_ATTR_SOURCE`, `GIT_CONFIG_PARAMETERS` and the caller's own `LC_ALL` as a consequence of the
  replacement rather than by naming each. And the pull-request scoping above validated the issue's
  single newest publication rather than selecting the newest one on this pull request, so a second
  pull request publishing on the same issue skipped this pull request's next repair around the
  budget entirely; it now selects the newest publication on this pull request from the whole
  timeline instead of checking whether the issue's newest happens to be one.

- **The `tasks` phase's output has an owner, in prose.** `skill/protocols/sdd.md`'s phase
  table already named `tasks` as leaving behind "the ordered work, each item independently
  checkable", but nothing said where that work must land once produced, or who answers for it
  reaching the issue when no phase produced one at all — measured against this checkout, zero of 131
  open issues (`gh issue list --state open --limit 500 --json number,body`) carry a `## Tasks`
  heading in their body. A new section states both: the run holding the issue owes the ordered
  work's arrival on the issue under every `Planning` value, including `direct`, where no `tasks`
  phase exists to produce it; an implementer brief cites that recorded decomposition rather than
  restating it; and a decision the executor cannot make is recorded as a decision with a named
  discharger, never as an open question left inside a step. Nothing enforces either half —
  `docs/honesty.md` says so, and no new test asserts prose exists by string match, since none of the
  crossings in `tests/honesty.rs` reads either file today. `skill/assets/analyst-issue-template.md`'s
  `## Proposed approach` line that told an unconfident analyst to leave "which part is uncertain" for
  a later reader to notice — the open-question shape the new rule refuses — is rewritten to ask for
  a named discharger instead; the template itself does not gain a task-list section, and says why in
  one clause: that artifact is produced later, by whoever holds the issue, once the design is
  understood, not by the analyst filing it and not as a section every issue would carry.

- **A filed finding names its receipt.** `blind-judges.md` said a warning or a suggestion
  is filed as its own tracker issue, but only in the branch where the publication carried no severe
  finding, and said nothing about a `severe` finding recorded `pre-existing` or `base-only` — the one
  class a reviewer thought serious enough to block on. Both are now one rule, unconditional on the
  publication's own severe finding, filed by the run that records the verdict before it releases the
  claim. `create` learns two optional arguments, `--epoch` and `--findings` (one or more
  `+`-separated finding identities), and `--epoch` alone changes the filed issue's marker from
  `analysis` to `review` — byte-identical to today's marker when absent, proved by test.
  `--findings` without `--epoch` is refused `finding-without-transaction`, documented in
  `skill/bindings/github.md` beside its neighbours. Neither the marker vocabulary nor claim
  adjudication reads the new `review` kind — it is in neither `RELEASE_KINDS` nor `ACTIVITY_KINDS`,
  and a dedicated test proves a body carrying one leaves the adjudicated holder unchanged. Filing
  remains asked for, not adjudicated: Estigia cannot see whether a caller ever calls `create`, nor
  that a named epoch or finding really exists. See `docs/honesty.md` for what that costs, measured
  against this repository's own open issues — including three filings by three runs
  across five days over two overlapping counts, with nothing but a 2026-09-06 prose comment linking
  them.

- **The lens is launched.** Five sealed lens definitions shipped, `RiskLevel::lenses`
  answered `0`/`1`/`5`, and nothing acquired one — `skill/SKILL.md` developer step 5 now does, beside
  the two judges and scaled by `review_tier`'s own count, in two waves: every acquired lens runs
  concurrently first, sealed and holding no `Read`; then the two judges run with the merged list of
  lens findings in both their briefs identically. `record_review_finding` learns `--lens`, validated
  against the reserved-reviewer registry rather than minted from caller text; every lens finding owes
  a `location` whose path `base..head` changed, checked on the path alone and never the line except
  for one that is also `severe` with `introduced`/`worsened`, which owes the same line containment
  any severe causal finding owes. A lens
  decides its own severity, ported from the source project's Go rather than forced to `warning` as an
  earlier draft of this change proposed; `evidence_class` routes whether it blocks —
  `deterministic` on its own, `inferential` through the judges' existing confirming pass,
  `insufficient` never. Two divergences from the source project are kept and named rather than
  smoothed over: this build ships a fifth lens with no counterpart in theirs, and lenses run beside
  the two judges here rather than replacing them the way their own Judgment Day mode replaces 4R.
  See `docs/honesty.md` for what remains unproved.

- **`review_tier` stops answering a judge count, and `Blind judges` stops being a setting.**
  The source project this crate ports `review_tier` from counts lenses in Go
  (`SelectReviewLenses`, `risk.go:120-154`) and states judge counts in prose
  (`internal/assets/skills/judgment-day/SKILL.md`) — no Go of theirs counts judges at all. Wiring
  their `0`/`1`/`4` lens dial to this binding's judge count, as an entry below did, put a number
  in code where they keep it in prose; `RiskLevel::judges` and the `"judges"` key in `review_tier`'s
  answer are deleted rather than replaced with a constant, because a hardcoded `2` in Rust is still
  counting judges in code. `Config::judges`, the `Judges` enum and the `Blind judges` row are gone
  from every settings surface — the table, the TUI, `doctor`, the MCP tool description,
  `skill/bindings/github.md`'s `review_tier` row — and `skill/SKILL.md` developer step 5 and
  `skill/policies/blind-judges.md` now state the panel in prose instead: two judges, always, one
  sealed and one unsealed, over one immutable target, with no dial, no mode and no `review_tier`
  reduction. A judge's own severe finding blocks only when both confirm it with the same
  identity — a lens's own routes differently, stated in full in `blind-judges.md` — the
  unanimity rule that `two blind` already had is what survives; `five blind`'s 3-of-5 quorum and
  `single`'s tier-reduced panel are both gone with the setting that chose between them. `verdict.basis`
  no longer records a configured panel or a number of judges acquired.

- **`review_tier` answers a lens count alongside its judge count.** The first change:
  `RiskLevel::lenses` reads `0` for `low`, `1` for `medium` and `high` alike — a lens count is
  questions whose subject the change contains, not opinions that scale with risk, so risk buys more
  judges and never more copies of the one lens question a `medium` or `high` change has a subject
  for. Unlike `judges`, `lenses` takes no `Context` and answers a plain `u8`: it never reads the
  `Blind judges` panel row, and cannot fail the way an unreadable row can. `review_tier`'s JSON
  answer, the MCP tool description and `skill/bindings/github.md`'s row all state the mapping, and
  `tests/honesty.rs` crosses both prose surfaces against `RiskLevel::lenses()` directly. Nothing
  launched a lens with it at the time: panel composition, `blind-judges.md`'s routing and
  `record_review_finding` were all unchanged, and acquiring what this answers was separate work,
  described below. See `docs/honesty.md` for why a lens count is not a risk dial. **Superseded above,
  in this same unreleased entry:** the `judges` half that was deleted was exactly the mistake this
  bullet described without naming — a lens dial wired to a judge count that the source project never
  derives from code at all.

- **A third reserved reviewer role, `review-lens`, joins `review-blind` and `review-blind-sealed`.**
  A lens is sealed and carries one narrow question — does any sentence a change
  adds contradict any code the same change adds? — over the diff alone, rather than a judge's open
  scope. `RESERVED_REVIEWERS` now carries each row's own template beside its name and seal, and
  `skill/agents/review-lens.md` ships as its own asset. Nothing launched a lens at the time: panel
  composition and the finding gate were unchanged then, and this stage was the reserved name
  and the rendered definition alone — `review_tier` changed next, above. See `docs/honesty.md` for
  what sealing a lens costs, contrasted with what sealing a judge cost earlier.

- **Four more sealed lenses ship, and `high` answers all five.**
  The remaining changes, beside the `judges` deletion and the lens-count answer above.
  `review-risk`, `review-readability`, `review-reliability` and `review-readback` join `review-lens`
  as reserved, sealed reviewer definitions — seven reserved names in total, alongside `review-blind`
  and `review-blind-sealed` — each carrying its own template and its own one-question scope; none is
  launched by this change. `RiskLevel::High.lenses()` moves from `1` to `5`: every sealed lens this
  build ships, not five copies of `medium`'s one question, because risk evidence gives each of them a
  subject at once. **Supersedes, in this same unreleased entry:** the earlier bullet reads
  `1` for `medium` and `high` alike, which was true when it was written and stops being true here. The
  severity floor the source project states in prose — only a finding that
  survives adversarial verification enters the fix loop, everything else is reported once and never
  reopens it — is now a named rule in `skill/policies/blind-judges.md` rather than left implicit.

  **A migration note for an operator who already owns one of these four names.** `review-risk`,
  `review-readability`, `review-reliability` and `review-readback` are generic enough that a project or
  user tree may already define an agent under one of them; `validate_reviewer_definition` refuses the
  next `setup` when a non-canonical definition holds a reserved name, the same refusal an operator
  installing `review-lens` or `review-blind` for the first time could already hit. Rename the
  conflicting definition before upgrading.

- **A severe finding's location must contain the whole claimed span, not merely overlap the change.**
  `record_review_finding` checked a `severe`/`introduced`-or-`worsened` finding's
  `path:start-end` location by overlap — the claim settled if *any one* of its lines fell in *any
  one* changed hunk, so `changed.rs:1-10` against a hunk of `5..=5` was admitted with nine of its
  ten lines never touched by the change being blamed. The predicate is containment now: one hunk's
  range must contain the whole claimed range. Measured over every `review-finding` marker the
  tracker held before this changed — 885 recorded, 122 carrying a `location`, 14 in the gated class,
  5 of those carrying a span, 2 of the 5 failing containment on exactly one unchanged line each — the
  refusal now names the first unchanged line inside the claimed span, since a reviewer citing a
  mostly-correct range needs the one line to fix rather than being told the whole span is outside
  the change. A location accepted yesterday may be refused today; see `docs/honesty.md`
  for what the check does and does not still prove.

- **Fifteen refusals that collapsed to `transport-refused` now arrive under their own name.**
  `operation_marker`'s two `format!("{kind}-operation-conflict")`/
  `format!("{kind}-metadata-mismatch")` constructors, `start_branch` handing `checkout_identity`'s
  bare `verdict` variable to the envelope, and `claim.rs`'s `require_pr_matches` choosing its
  `"reason"` with an inline `if`/`else` all built a value `build.rs`'s extractor cannot see — a
  runtime string or a bare identifier, never a literal. Nine of `operation_marker`'s fourteen
  reasons, all four of `checkout_identity`'s failure verdicts and both of `require_pr_matches`'s
  outcomes — fifteen in total — reached a caller as the catch-all; the other five of
  `operation_marker`'s fourteen already had a literal spelling elsewhere in `claim.rs` and were
  already in the vocabulary. `operation_marker` now takes a closed `OperationKind` enum whose
  `operation_conflict`/`metadata_mismatch` methods match on `self` and answer a literal in every
  arm; the four failure verdicts `checkout_identity` can return and `require_pr_matches`'s two
  outcomes are each split into their own literal `serde_json::json!` block instead of one shared by
  a condition.
  `TRANSPORT_VOCABULARY` grew from 108 to 123 entries, measured by generating it at this branch's
  merge base and at this head and diffing the two — `claim-metadata-mismatch` is among the fifteen
  newly visible, the reason `skill/bindings/github.md:180` already tells an agent calling `claim`
  to branch on, which arrived as `transport-refused` until this change. `build.rs` also gained
  `assert_reasons_covered`, which fails the build itself when a future `"reason":` value is neither
  a literal nor one of the two recognised bound shapes, so the next one this class describes is
  caught here rather than found by accident, the way these four were.

- **`create` no longer prepends the identity to the title it was handed.** 172 issues
  filed on this tracker average 194 characters per title, 168 of them slug-prefixed and 108 doubled
  — `<slug>: <slug>: <sentence>` — because `src/transport/commands.rs`'s `create` composed
  `format!("{identity}: {title}")` before ever calling `gh issue create`, and nothing in the crate
  reads an identity back out of a title (`grep -rn "identity" src/` found the one writer and no
  reader), so an analyst reading the filed shape copied it into `--title` as well. `--title` now
  gets the caller's title unmodified; the identity moves into the `analysis` control marker
  `create` already appends to the body, beside `run-id`, where it stays the machine-readable dedup
  key `domain-composition.md` describes rather than decoration on a surface nothing reads. The
  `title` row in `domain-composition.md` now states the rule in full — a short name under 60
  characters, not a sentence, carrying no identity, run-id, file paths or measurements — and the
  `title`/`pr_title` argument descriptions in `src/harness/mcp/tools.rs` point at it instead of
  restating it, since `publish_review`/`republish_review` compose PR titles under the same rule.

- **The runtime is attribution, recorded in every marker and compared as an identity nowhere.**
  `claim`/`reclaim`, `unassign` and `handoff-review` pushed
  `session::DEFAULT_RUNTIME` (`claude`) as the `--runtime` flag over MCP whatever agent ran, so
  every marker these three write recorded `runtime=claude` regardless of the caller. A new
  `session::runtime_of` derives the true value from the run id itself, split on the *last* `-` so a
  dashed runtime like `claude-code` survives whole — and the MCP boundary refuses a run id whose
  derived runtime is not itself a well-formed run-id component before it can be pushed at all.
  Fixing only the flag left the comparisons standing: `claim`, its adopted-id retry, `reclaim` and
  `unassign` all
  compared a persisted `runtime` against the caller's own in `operation_marker`'s `expected`/
  `sought`, `ownership.rs`'s `unassign` control-mark reading refused a release whose runtime
  disagreed with the acquisition's **after** writing the release marker, `resumed_release` repeated
  the same comparison for a retry, and `plan_release`'s legacy arm fell back to it when no runtime
  was recorded at all. Every one of those already compares the run id **first**, and `runtime_of` is
  a total function of it — comparing the runtime a second time was comparing a projection of a value
  already compared, and every marker written before this head carries `runtime=claude` under
  whichever run actually wrote it. Once this build could carry a real runtime again, that stale
  comparison would have caught a renewal or a release crossing the build boundary as a mismatch
  where none exists, `unassign`'s specifically after its marker had already landed. Every one of
  those comparisons is gone; the runtime stays exactly where it was written, in the marker.
  `plan_release`'s legacy arm and its `unassign-metadata-mismatch` refusal are gone with it, since a
  legacy acquisition is found by run id like any other. The `dev:` label fallback it replaced is gone
  with it too. `a_legacy_acquisition_releases_regardless_of_the_labels`
  (`src/transport/claim/tests.rs:308-325`) now asserts release succeeds regardless of what `dev:`
  labels are present, iterating an empty projection first.

- **The sealed reviewer grant now names a tool a host can resolve.** `render_reviewer_agent`'s
  `Seal::Sealed` arm rendered `tools: none`, which seven one-shot Claude Code sub-agent launches
  probed on 2026-09-02 showed refuses at the launch boundary itself rather than merely denying every
  call — a sealed judge, as this crate rendered it, could not run on Claude Code at all. The arm now
  renders `tools: WebSearch`, the only one of seven spellings probed that both launches and denies a
  repository read; `[]` was rejected because an empty list reads as no policy at all at this gate's
  own parser, and `ToolSearch` because its job is fetching other tools' schemas. `WebSearch` reaches
  the public internet, so sealed now means the judge cannot read this repository, not that it holds
  nothing at all. Both gate doors now derive the seal from `reserved_reviewer`, and `setup` ships a
  sealed rendering — see `docs/honesty.md` for what was measured and what remains assumed.

  The sealed rendering is now reachable as a role, `review-blind-sealed`, installed and removed
  beside the unsealed `review-blind` from the same template — `render_reviewer_agent` now takes the
  reserved name explicitly, a public signature change, and substitutes it unconditionally into the
  frontmatter `name:` line rather than switching on `Seal::Sealed`, so the two never collide at one
  path. Both gate doors, the prelaunch proof and the candidate search that backs it now answer "is
  this name reserved, and with which seal" through one function, `harness::roles::reserved_reviewer`,
  rather than each hard-coding `Seal::Unsealed` or the literal `"review-blind"`;
  `authorize_review_blind_launch` takes the launched name and proves it against that name's own
  canonical path, so a project or user duplicate of either reserved name still refuses the launch
  exactly as before. A later change folded both reserved names and their seals into one list,
  `harness::roles::RESERVED_REVIEWERS`, read by setup's preflight, install and uninstall loops and
  by `reserved_reviewer` alike, so a third reserved name needs one row rather than four call sites
  kept in step by hand — the renderer's unconditional substitution above is what makes that row
  installable without widening `render_reviewer_agent`'s own match on `Seal` again.

- **`release_ci` accepts a byte-exact receipt from a run that did not publish it.** A
  run pointer is deleted at `SessionEnd` while the tracker's own publication comment is not, so a run
  that resumed an issue, held the live `review` claim and supplied the timeline's own latest receipt
  used to be refused `published-transaction-mismatch` on the publisher field alone, even when every other
  field matched. `recorded_receipt` now compares the receipt's five fields only; the recorded
  publisher is still reported in a refusal, but is no longer grounds for one on `release_ci` — what
  still stands between an arbitrary caller and CI is everything else `release_ci` already
  re-verifies: live claim, distinct accepted verdict, current draft PR, coherent clean target.
  `recorded_receipt` also loses its `run_id` parameter, a public signature change: obtaining the
  receipt no longer depends on who asks. See `docs/honesty.md` for what this restore does and does
  not prove.

- **The review transaction records the execution topology a run declares.** `publish_review`/`republish_review` take `--topology` and `--topology-reason`, write them to the
  `published` marker and echo them in the answer — recorded, not bound, since Estigia cannot observe
  how an agent wrote code, only what it declares. A `topology: inline` declared, trimmed and
  case-insensitively, at or past `SKILL.md`'s Direct Work Delegation writing threshold refuses unless
  `--topology-reason` accompanies it: Estigia counts changed files, it cannot judge non-trivial, so
  the refusal over-fires by design and a stated reason is accepted without argument. Both fields are
  caller-supplied free text and are escaped before they reach the receipt's prose, the same way
  `record_review_verdict`'s evidence already is. See `docs/honesty.md` for what a declared topology
  still cannot prove.

- **`review-blind`'s review contract is adapted from `gentle-ai`'s judge definition.** It
  restructures the body's flat prose into `## Rules` and `## Review contract`, taking sweep budget
  and the precision gate's shape from `internal/assets/claude/agents/jd-judge-a.md` (MIT, Copyright
  (c) 2025 Gentleman Programming) in `gentle-ai`, adapted to this repository's own tooling, evidence
  standard and panel mechanism. Three things an earlier version of this entry credited to that file
  are not from it: findings classification — the `severe`/`warning`/`suggestion` table with
  `evidence_class`, `causal_disposition` and `location` — was already in this repository's own body
  before the port; scoped re-judgment is inverted rather than copied, since the source narrows a
  repair round to the delta and this repository's rule says the opposite, that the delta does not
  narrow the sweep; and the severity-floor clauses actually taken from the source were deleted in
  the previous repair round, leaving this repository's own rule in their place. That same round also
  added a new classification carve-out to `skill/policies/blind-judges.md` — a contradiction between
  two instructions a repository's own agents execute is a contract defect the never-`severe` rule
  does not soften — which changes what a judge may class as `severe` and was left uncredited here
  until now. The carve-out is narrow: it is gated on a contradiction between two instructions a
  repository's own agents execute, not on staleness, and it does not reach a stale comment or a
  stale document the never-`severe` rule above already covers.

- **A review now acquires fewer judges when `review_tier` answers `low` or `medium`.** Developer step 5 in `skill/SKILL.md` reads `review_tier` for the same base and head before
  acquiring, and acquires the smaller of its `judges` and the configured panel, never fewer than
  one. A `low` tier's `0` still acquires one reviewer, because `release_ci` refuses
  `qualifying-review-verdict-missing` without a distinct accepted verdict and a panel of none can
  never record one. A failed read, or a `null` `judges`, acquires the configured panel unreduced,
  since an unread tier is not clearance to reduce. The verdict's `basis` records the tier, the
  answered `judges`, the configured panel and the number acquired. `review_tier` itself is
  unchanged — it was delivered unread, and against three real deliveries of this repository
  it answered `medium`/1 where the configured panel was `two blind`/2; that saving had never been
  taken.

- **`review_tier` derives a review tier from the change itself, so panel size can be routed on
  evidence instead of being fixed.** It ports `ClassifyRisk` from `gentle-ai`
  (`internal/reviewtransaction/risk.go`, MIT, Copyright (c) 2025 Gentleman Programming): three
  tiers, closed by construction, and the property that makes them worth having — **change size is
  read at a boundary, not as a magnitude**. A five-thousand-line documentation change and a
  five-line authorization edit are classified by what they touch, never by how much of it; `TestVolumeAloneNeverSelectsATier`'s
  own boundary sweep (`{1, 399, 400, 401, 1_000, 5_000, 50_000}` changed lines) is mirrored here and
  holds at every size. `high` fires on any of a closed signal set: a touch to a hot-path token
  (`auth`, `update`, `security`, `webhook`, `payments`, matched as whole tokens — `authority.rs`
  never matches `auth`), a path carrying adjacent `service`/`token` tokens, a shell-source extension
  (`.sh`, `.bash`, `.zsh`, or a `.yml`/`.yaml` under `.github/workflows/` — this repository's own
  publication lane included), or an execute-bit flip between the two modes. The last of those
  carries no eligibility gate, so a chmod inside `docs/` still escalates; the two name-derived
  producers before it are read behind the same gate the source project reads them behind —
  inspectable, no `docs`/`testdata`/`fixture`/`fixtures`/`__fixtures__` segment, no
  `readme`-prefixed basename — so a path merely shaped like one of them, under `docs/` or named
  `README...`, does not. `low` requires every authored byte proven passive from its own bytes and
  mode — never from extension alone: a `.md` file beginning `#!`, carrying an executable bit, or
  sitting beside a configuration file such as `Cargo.toml` is never `low`. `medium` is everything
  else, one consolidated review. Read-only, no claim required, decides and routes nothing itself:
  `judges` reads the configured `Blind judges` panel for a `high` tier — `single` 1, `two blind` 2,
  `five blind` 5 — through the same `Judges::parse` the row's own writer uses; `medium` is always
  `1` and `low` always `0`, regardless of the configured panel, because they are this port's
  one-lens and zero-reviewer tiers at every panel size. `null` answers only a `Context` built with
  no `Blind judges` row at all, which no live call produces — a repository with no readable
  contract still renders the default, `single`, so it reads `1` for a `high` tier there,
  indistinguishable from an operator who configured `single` themselves. Reads two trees with
  `git diff --numstat -z` and `--raw -z`, never the worktree, so the answer is reproducible from
  the same two object ids on any checkout that has them; an unreadable byte in `git diff`'s own
  output and a tier its own reasons do not explain are both read failures, not a tier — this
  operation answers `Err`, never `medium`, for either. An unrecognised mode and an over-budget
  content scan are different: both still answer a tier, just never `low` — an unrecognised mode is
  not proven passive, so it falls to `medium` unless another signal already reads `high`, and an
  over-budget content scan treats every deferred candidate as active for the same reason. And, for
  the UTF-8 case, a deliberate divergence from the source project's `utf8.Valid`: this transport hands back an
  already lossily-decoded `String`, so a length-preserving invalid subpart (one three-byte `U+FFFD`
  replacing a three-byte invalid sequence) is checked for directly rather than inferred from a
  byte-length comparison, which also means a document that legitimately contains `U+FFFD` is no
  longer provably passive either. **Not ported, and not
  because of a gap this side of it:** the source project's own six-path self-reference
  (`isNativeReviewAuthorityPath`) has no faithful counterpart here, measured rather than assumed.
  Substituting this repository's own gate files (`src/harness/mod.rs`, `src/harness/guard.rs`,
  `src/harness/roles.rs`, `src/harness/tracker.rs`, `src/transport/claim.rs`,
  `src/transport/ownership.rs`) answered `high` on all three of the real deliveries it was measured
  against, every one through `src/transport/claim.rs` — a 7,163-line module touched in 32 of this
  repository's last 40 commits, which made the classifier a constant function on this repository's
  own history. Dropping the busiest file would optimise the metric against the safety the rule
  exists for: `claim.rs` is where `record_review_verdict` and `release_ci` live. Also not ported: the source project's content scan for a
  shebang or a `subprocess`/`execute_process`/`exec` construct (`processBoundaryRiskReasons`) — it
  reads bytes through a `git grep` shape none of the other producers need. Both gaps narrow what
  reaches `high`, never what reaches `low`. One widening runs the other way and is named rather than
  closed: the source project skips a generated golden fixture (`testdata/golden/*.golden`) at seven
  call sites across its risk classification, not only the two a prior version of this entry named
  (`docs/honesty.md` cites all seven), and this port carries no such exclusion at any of them — this
  repository has no established convention for that shape of file, and building the exclusion for a
  convention nothing here uses would be scope this port does not need. A path shaped like the
  source project's golden fixture is `high` here and would not be there; it never turns a `high`
  answer into a `low` one. All three are named in `docs/honesty.md`. Nothing in `skill/SKILL.md`
  routes on this operation's answer yet; that is a separate decision.

- **The repair ceiling counts what actually spends the budget: rejections plus author-elected
  repairs, not rejections alone.** Two cases were measured in opposite directions: one pull request
  ran six repair rounds with zero rejected verdicts, because every round was elected by the author
  after a panel accepted, and the ceiling's counter never moved; another ran two rejections and zero
  elected rounds. Counting only rejections misses the first; counting only publications would have
  fired on the second at the round that succeeded. `repair_rounds` is a new read-only operation,
  no claim required, that derives the sum from the timeline. **Both terms are counted only for the
  pull request the issue's latest publication names** — the change under review — because an issue
  outlives its changes and a delivered one's rounds are not the next one's: `rejections` is every
  `review-verdict` marker against a receipt on that pull request whose outcome is `rejected`, and
  `author_elected` is every `published` marker on it carrying a `parent` receipt on the same pull
  request whose own latest recorded verdict was `accepted`. A parent whose latest verdict was
  `rejected` is not author-elected, `rejections` already counts it; a parent on a different pull
  request is the previous work on the issue rather than a round this author elected; and a first
  publication with no parent counts as neither. Scoping one term and not the other was published
  once and rejected by two judges — running the operation against a real timeline is what
  found it, after a sweep of the documents had missed it. `rounds` is the sum, `ceiling` mirrors the
  configured `Repair ceiling` row when it can be read and is `null` when it cannot, and `at_ceiling`
  is `false` whenever the ceiling is off or unreadable. Nothing here refuses a publication, a
  verdict or a delivery; the escalation the ceiling triggers stays the orchestrator's own read of
  the answer, exactly as before. `docs/honesty.md`'s entry recording that nothing enforces the
  ceiling is narrower now — a run can ask for the number — and still stands: nothing makes it obey.
  Run against a live timeline — the issue the first pull request published on — the
  operation answers `rounds 4, rejections 2, author_elected 2, ceiling "3", at_ceiling
  true`; those figures are scoped to the pull request the issue's latest publication names, not to
  the first one. A test replays the first pull request's own shape — every round author-elected, no
  rejection anywhere — and restoring the sum this replaces, rejections alone, turns
  it red at `rounds 0, at_ceiling false`.

- **`publish_review` and `republish_review` omit the payload by default; `inline_payload: true`
  asks for it.** The answer used to carry `payload.text` unconditionally, with no argument to
  suppress it. Measured on this repository, twice: one publication put
  `payload_bytes` at 145,756, and the MCP answer was refused at 151,889 characters — the publishing
  run could not read its own receipt back. Another publication (head
  `1a1bb08f1755b931401c0a59c13b6dbe83c8a422`) put it at 73,272, refused at 76,588 characters — about
  half the first case, so the ceiling is reached by an ordinary change, not only a large one; that
  run recovered `epoch`, `head`, `base` and `digest` from the spill file by hand. The default is now
  the omitted answer: `payload_bytes` and `payload_digest` are always present, and every other
  receipt field — `epoch`, `pr`, `head`, `base`, `digest`, `parent` and the rest of the lineage —
  carries exactly what it carried before. `next` differs between the two answers, and the
  omitted-payload one says the answer carries the payload's size and digest rather than its text and
  points at a checkout — it names no way to fetch the text afterwards, because publishing again to
  collect it would mint a new epoch and void every verdict recorded against the first. The payload is
  still never truncated, whether it is asked for or not. Asking for it inline does not make an
  arbitrarily large answer deliverable: what refused the read-back at that size was the host's
  own result cap and not `REVIEW_PAYLOAD_CEILING` — that payload was 145,756 bytes, under the
  262,144 the ceiling allows, which is why it materialised at all. The cap measures the whole JSON
  answer, which is the reconstruction — nobody measured the difference — offered for how those
  145,756 bytes became the 151,889 characters that were refused, once the diff's newlines and
  quotes were escaped and the receipt fields added.

- **A publication lane whose run never executed a step is told apart from one that ran and
  failed.** A billing or spending-limit failure on the account running Actions answers every check
  run identically to a genuine failure, so `record_review_verdict` refused both the same way and
  named a log — `gh run view --log-failed` — that a run with no steps has none of. Every distinct
  workflow run behind the head's red check runs is now checked for `steps: []`, not only the first
  GitHub lists, until one is found to have run; a `cancelled` conclusion is excluded from ever
  qualifying, since it is the one way the run being judged could reach this branch itself. A
  head found never-ran refuses `publication-lane-never-ran` naming the runs and GitHub's own
  annotation, and clears only when the caller supplies `--never-ran-evidence-file` — read back
  verbatim on the same footing as `basis`, never re-run. The advisory `publication_lane` answers
  `never-ran` through the identical classification, so a run following Step 5's lane guidance
  before ever reaching `record_review_verdict` sees the same state.

- **`skill/bindings/github.md` is split into a binding and four conditionally-routed companions.**
  At 95,498 bytes it was the largest single item an activation paying for tracker work loaded whole.
  Recovery (a lock, a
  migration, or a quiet failure), board sync, setup, and delivery mechanics moved verbatim to
  `skill/bindings/github/recovery.md`, `board.md`, `setup.md` and `delivery.md`; `SKILL.md`'s
  reference table routes each one by the same trigger a run already reads. `github.md` itself drops
  to 51,631 bytes, with the operations table, the claim hazard, control markers, state names and
  every refusal untouched in place. The auto-close section keeps the hazard a dev must know before
  pushing and the `check_closing_keywords` pointer; the mechanics behind it — scan timing, the
  keyword list, the incidents — moved to `delivery.md`, split at the paragraph that names the tool.
  No operation, refusal, marker, state name or capability mapping changed.

- **`review-blind` carries the severity decision rule, not only the three class names.** The
  definition named `severe`, `warning` and `suggestion` and which one blocks, and never said how to
  choose. The rule was routed to whoever runs the panel — `skill/policies/blind-judges.md` in full,
  `skill/SKILL.md` step 5 in part — and never to the judge, whose starting context the definition
  itself limits to that definition and the launch prompt. The measurement this closes:
  four publications, eight blind judges, three rejected verdicts, zero code defects found; every
  blocking finding was prose a launch prompt happened not to paste the rule for.

  `skill/policies/blind-judges.md` keeps the rule as well, and says why: the definition is installed
  on Claude Code alone, so on every other adapter that document is the only copy the machine gets and
  an orchestrator there pastes the rule into the launch prompt itself. Two copies is the smallest
  number that reaches a judge on one host and an orchestrator on the rest; the definition is the
  authority if they disagree.

- **`expected_target` can omit its manifest; `audit_board` caps `foreign`.** Both sized themselves by
  the repository rather than by the caller, and neither had a way to ask for less. `expected_target`
  takes an opt-in `brief` argument (`--brief` on the transport) that drops the `manifest` array:
  measured on this repository's 141 tracked files, the answer goes from 14,577 bytes to **216**. The
  digest is computed over the manifest before either shape is rendered, so it does not move between
  them, and `--native-start` compares the internal target rather than the rendered answer, so
  `review-target-mismatch` fires identically either way. Default is unchanged — a caller that asks
  for nothing gets today's full answer — and `skill/references/repository-delivery.md` says which
  call wants which shape, since an option nobody is told about saves nothing. `audit_board` caps
  `foreign` at 20 rather than cloning every card on a board shared with another repository;
  `foreign_count` carries the true total and `foreign_truncated` says whether the cap cut, so a
  capped array can never be read as the whole set. That cap is exercised against a fixture board:
  this repository runs `Project board: none`, so nothing here drives it against a real one.

  **What it cost to document, both halves of it.** Saying this grew `skill/bindings/github.md` by
  354 bytes and `skill/references/repository-delivery.md` by 641 — **995** together, measured against
  the base rather than estimated. The first is read before tracker work and the second is the routed
  fallback for repository work, so a run that reaches `expected_target` pays both: one `--brief` call
  repays about fourteen such runs, and about forty for a run that loads only the binding.
  `docs/honesty.md` declines to name an always-loaded set at all, because it depends on tracker,
  Blind judges and Planning, so neither figure is a per-session constant.

  `list_state` is deliberately unchanged. Lowering its 200-item default was the third reduction
  proposed, and at the time it was unsafe: the queue read passed no ordering argument, the endpoint
  answered newest-first, and the transport then sorted each partition oldest-first over whatever
  arrived — so a smaller ceiling dropped the oldest issues while still presenting an oldest-first
  list. The default still stays 200.

- **The queue read asks for the oldest issues, not the newest.** `list_state` read
  `gh api … issues?state=open&labels=status:<s>&per_page=100` and cut it to `--limit`, and the REST
  endpoint answers newest-first by default — so a cut answer held the newest N and the transport then
  presented it under an oldest-first heading, with `at_limit` saying the list was cut but nothing
  saying which end was removed. The request now carries `sort=created&direction=asc`, so a cut answer
  holds the oldest matching items and the oldest-first selection survives the cut. Ranking does not:
  the highest-priority item may still sit outside the window, and `at_limit` is the surface that says
  so. The default stays 200, and the partitioning, entry shape and `at_limit` computation are
  unchanged. Two tests hold it: one pins `sort=created&direction=asc` on the wire, and one poses three
  issues against `--limit 2` with both orders the endpoint can give, asserting the cut returns the
  oldest two. Dropping the ordering term reddens both.

- **A tool answer is rendered compact.** The MCP path indented every successful answer, and what
  reads it is a model paying by the byte. The two spellings carry identical data, so the indentation
  bought nothing: measured on this repository's own 141 tracked files, `expected_target` was 19,403
  bytes indented against 14,577 compact — **24.9%**. Files Estigia writes for a person to read — session
  pointers, the install record, branch records — stay indented, and so does the one command that
  prints a transport answer to a terminal, `estigia claim` without `--json`.

- **A gate refusal's `--json` answer carries what the gate actually parsed.** `Refusal.observed`
  names the shell fragment matched, the raw command line, which boundary list found it, and the
  byte that disqualified a `gh pr merge`; and, for the checkout a call was adjudicated against,
  which candidate (`payload-cwd`, `payload-workdir`, `write-target`, `launch-directory`) it came
  from and every reason a stated one was discarded. Reaches every denial `report_gate` answers —
  the ordinary claim road, a role refusal, and the session-less several-holder road — and the
  `allow`/`outside` documents besides. Absent, not `null`, when nothing was read. `estigia gate`'s
  own failures ahead of that door (a malformed `--input`, an unreadable launch directory or home)
  and the push guard and MCP roads still answer without it — see `docs/honesty.md`. No hook dialect
  renders it yet; that answer belongs to the host's own protocol.

- **`estigia tui` is the setup screen; `setup` is direct-only.** `tui` is its own verb and takes no
  flags. Bare `estigia setup` no longer infers a TTY. Confirming an install from the screen is the
  source-build consent; `setup` and `sync` still require `--allow-source-build`. `install` remains a
  visible alias of direct `setup`.

- **`estigia gate` admits a call naming a live worktree, sibling or not.** `Worktree location` is
  a sibling of the repository it was minted from, never a child of it, so the clamp that reads a
  call's `workdir` or write target could never resolve one — a run that had claimed, published,
  reviewed and released CI could not deliver, because `gh pr merge` was adjudicated against the
  launch checkout on `main` instead. A directory is now admitted when it lies inside the launch
  checkout, or when it is a checkout a pointer already records. A call carrying its own run id is
  restricted to that run's own `worktree`; a call with none resolves a named worktree to
  whichever run holds it, which is correct rather than a fallback — a worktree holds exactly one
  task — but cannot yet refuse a call reaching into a worktree that is not its own. At that change's
  landing the identity-restricted road was reached by nothing that shipped: the only shipped adapter
  that calls `estigia gate`, the OpenCode plugin, carried no run id and exported no `ESTIGIA_RUN_ID`,
  so on an unmodified install every call it made took the road that cannot tell one task's identity
  from another's. The plugin forwards the session id its host hands the hook, so a call
  carrying one takes the identity-restricted road from there on.

- **The instruction surface has a registered byte ceiling.** `tests/instruction_ceiling.rs`
  refuses a rise in any `skill/*.md` file above the measured count at landing, and refuses any
  unexcepted line above 2,000 bytes. `skill/SKILL.md` step 5 and four GitHub binding operation
  rows are named exceptions. It counts bytes, not whether the prose agrees with the gate, and
  it cannot see conditional routing.

- **A repository can declare a daily range for commits, pushes, and branch creation.** `Git operation window` is disabled by default and accepts one value such as `enabled 09:00-17:00`
  or `enabled 22:00-06:00 inverted` — a half-open `[start, end)` range in this machine's local time,
  a start later than the end crossing midnight, equal endpoints invalid. Outside the permitted period
  the gate denies `git commit`, every ordinary `git push`, plain and porcelain `git branch` creation,
  `git checkout -b/-B`, `git switch -c/-C`, and `git worktree add -b/-B`, and so do the pre-push guard
  (whether or not the checkout is claimed), `start_branch` (unless it proves it creates no branch),
  and `publish_review`. The schedule never grants: inside the period every other check still applies.
  An enabled value that does not parse, and an unreadable clock, deny the protected operations only.
  `docs/honesty.md` states what it cannot stop: `--no-verify`, direct ref writes, hosting APIs, and
  other clones.

- **The repair loop has a declared ceiling.** `Repair ceiling` defaults to `3`
  repair rounds — see the entry above for what is counted, which this entry
  originally stated as rejections alone — accepts `none` to switch off, and reaching it is an escalation with evidence
  rather than `blocked`. Judges are never told the ceiling or the round number.
  Delivery still requires an accepted verdict. Twenty-four typed settings. Reading the table
  produces a valid configuration or a refusal.

- **`Review budget` names the per-judge token figure a review is reported against.** Defaults to `150000` and accepts `none`, following `Repair ceiling`'s own shape — a number with an
  off value. 150,000 is an inherited comparison target, not a number derived from the distribution;
  what is measured is the rate against it — twenty-eight judges across five issues, read from
  harness completion reports, put the minimum at 96,073, the median at 127,025 and the maximum at
  182,136 tokens, with 18% running over 150,000. Advisory only: nothing here refuses a panel that
  runs over it, and this campaign's own `record_spend` calls recorded a panel total beside a
  `contexts` count, not these per-judge figures. `docs/honesty.md` names what the default cannot
  count — the orchestrating run's own spend, never in any of those twenty-eight figures.

- **The publication lane is readable before a panel launches.** `publication_lane` answers
  green, red, never-ran, running or absent for one head; an unreadable listing is a failed
  read. Step 5 routes on those six. Nothing refuses a panel launched over a red lane — a
  launch is not a write the gate sees. `record_review_verdict` still refuses an accepted
  verdict on a lane whose run genuinely ran and failed, or one still unfinished.

- **Setup upgrades an Estigia-authored reviewer from a previous rendering.** An owned
  `review-blind.md` whose bytes match a body this binary used to write is rewritten to the
  current rendering. An owned file that matches none is still `reviewer-definition-changed`.
  Launch still requires the current rendering (`reviewer-canonical-unavailable` until setup
  has rewritten the file).

- **The default `reading` reviewer carries the verdict-basis requirement.** Both evidence
  arms append `VERDICT_BASIS_REQUIREMENT`. An existing `reading` install is upgraded by
  `setup`; launch still refuses until that setup. A `reading` judge has no commands to
  name, only that it ran none.

- **A judge's starting context is the review contract, not the tracker binding.** The reserved
  `review-blind` definition now carries finding classes and an explicit *no tracker operations*
  rule. Claude's launch surface has no field that suppresses host assembly; OpenCode `Task`
  neither. That limit is in `docs/honesty.md`.

- **`publish_review` hands a judge the change, not a checkout to search.** It materialises
  per-path unified hunks against the receipt's recorded base, once per epoch, binds their
  digest as `payload-digest`, and returns the payload in the answer. Overflow of
  `REVIEW_PAYLOAD_CEILING` (262,144 bytes) refuses `review-payload-too-large` before any
  remote write; the payload is never truncated. Isolation is unchanged: one directory per
  judge, none inside the claimed checkout. Estigia still cannot prove the orchestrator
  handed those bytes on.

- **`estigia --version`, `status --json` and a new `doctor` row (`gate-version`) report which
  build is actually running, not the crate version every build has answered with since the
  crate existed.** `build.rs` embeds the commit the binary was built from and whether the tree
  was dirty, watching the git refs whose change means the *commit* half of that identity is
  stale, in addition to `build.rs` itself and the transport's source files it already watched,
  so an unrelated edit and rebuild does not replay a cached *commit* from an earlier build. The
  *dirty* half is not closed by that watch — it depends on tracked file content, not which ref is
  checked out, and editing a tracked file without committing still reports the tree clean on the
  next unrelated rebuild (see `docs/honesty.md`). `doctor` compares the embedded commit against
  the checkout's `HEAD` and reports the gate broken, never a silent pass, when the checkout's
  head is not a point this build already contained: an older gate, or one on a different line
  of history. It reports *skipped* rather than passing when the comparison cannot be
  made at all (a dirty build, a published crate with no git to read at all, a checkout with no
  history, or an ancestry check git itself could not answer, such as a checkout sharing no
  history with wherever the binary was built). Reported, not blocking: a stale gate is deciding
  by older rules, not an unusable
  environment. The row names its own fix, platform-appropriate: on Windows,
  `Stop-Process -Name estigia -Force -ErrorAction SilentlyContinue; cargo install --path . --force`,
  because plain `cargo install --force` fails there while the running binary still holds the
  executable open; elsewhere, where an in-use executable can be replaced without stopping it first,
  plain `cargo install --path . --force`.

- **Every write to the repository's own configuration through a write tool is
  a boundary, whichever row it names, and a shell write to it gets the same
  answer a shell write to any other control-surface file gets.**
  `<repo>/.git/estigia/estigia.local.md` was not on `CONTROL_SURFACE` and
  `is_control_surface` did not derive it, so a write there classified
  `Routine` regardless of which row it touched — including `Evidence
  standard`, `Tracker`, `Integration`, `Renewal window` and `Irreversible
  commands`, the five rows the gate itself reads to decide what it enforces.
  A run holding a claim could widen its own reviewer's grant, or its own
  boundaries, on an answer that had nothing to do with the write.

  Six cuts tried to answer that by reading a write's own payload instead —
  which row a label named, which text field carried it, whether a line
  merely mentioned a row or resolved it through the same parser
  `estigia config set` uses — and a blind panel defeated every one of
  them, each on a working payload that named no guarded row at all. The
  last two escapes, found independently in the same round: deleting the
  line break between two rows so the parser reads the merged line as one
  and the row beneath it disappears without ever being named, and a fence
  marker assembled from two fragments, each an ordinary-looking row on its
  own, that only formed a complete marker once applied to the real
  document. Both falsify the same assumption every cut before them had
  leaned on — that an anchored write (`Edit`, `MultiEdit`, Gemini CLI's
  `replace`) cannot remove a row without naming the text that named it. A
  per-payload reading cannot have a property of the document that results
  from applying it, because `crate::config::rows` reads a *region*, and an
  anchored write can move a region's boundary with text that names no row.

  What ships instead is not a seventh reading: `is_repository_configuration`
  answering true is now the whole test, on the write-tool road, before any
  row is asked about at all. The shell road is unchanged — it never read a
  row out of this file either, since a redirect carries no named field to
  attribute one to — so the two roads that used to disagree over `Project
  board` now agree wherever a shell line reaches the file's own question.
  The control surface is still asked
  first and unconditionally on both roads, so a path that is also a
  control surface — `~/.estigia/estigia.local.md`, which matches both — is
  answered through that branch rather than this one. The cost, stated
  rather than argued away: `Project board`, the one row the gate itself
  never reads, now costs one tracker read on every write like the other
  five do. `estigia config set --repo` writes this file directly, outside
  any gated tool, and is unaffected. `docs/honesty.md` carries the full
  account of all six cuts and the payload each one died on.

- **A run whose own claim lapsed can take the issue back through `claim`,
  without releasing it first.** `claim` and `reclaim` both reuse the operation
  id stored on the run's pointer when the pointer still names the same issue —
  correct for a retry, where a dropped write must resume the one event it
  already wrote rather than mint a second. The same reuse also fired for a
  *re-claim after lapse*, where the stored id names a **dead** event, and the
  two are indistinguishable at that one line. Reusing it there answered
  `operation-id-kind-conflict` when the run's lapsed holding had come from a
  `reclaim` (the id names a `reclaim` marker, and `claim` will not resume an
  id reserved for a different kind) or a metadata mismatch when it had come
  from a `claim` whose horizon this call, sensibly, was asking to move —
  and `reclaim`'s own `stale-self-requires-claim` pointed at `claim`, which
  then refused too. One sequence did work — `release` the lapsed epoch, which
  drops the whole pointer and the stored id with it, then `claim` — and no
  message named it.

  `claim` now recognises a reused id that names a **stale** acquisition of its
  own run — whichever operation wrote it — ahead of the kind and metadata
  checks that used to answer first, and reports it by the one name
  `adjudicate` already had for the same fact found after a write:
  `claim-operation-expired`. The harness responds to that refusal by minting a
  fresh id and reissuing once — an `if`, not a loop — and only when the id
  that just failed was itself reused rather than freshly minted for this
  call. That second check is defence in depth rather than a case reachable
  today: a genuine lost race answers `lost-claim-race`, not
  `claim-operation-expired`, and an id minted for this call cannot already
  name a dead event on a timeline it has not written to yet. `reclaim` keeps
  `stale-self-requires-claim`: displacing yourself is not a takeover, and that
  refusal now says plainly to call `claim` instead, which works.

  One behaviour moved as a consequence: when this run's own acquisition is
  stale *and another run already holds the issue live*, the reissue still
  fires and writes a claim comment under the fresh id — where the earlier
  code refused before writing anything — and `adjudicate` then answers
  `lost-claim-race` rather than the wrong-kind or metadata-mismatch refusal
  the old path gave. Any fresh contender loses that same race and writes the
  same comment, and `lost-claim-race` names what that call itself answered.
  What it leaves on the timeline is not new: the reissue posts a live
  acquisition marker with a future horizon, and once the winner's own claim
  lapses, `reduce_ownership` hands the issue to the earliest remaining live
  event with no further call from the loser — the same mechanism any losing
  contender already leaves behind, now reached one more way.
- **The gate now covers the checkout a run *adopted*, not only one it built
  itself.** `Run.worktree` had a single writer, the effect `start_branch`
  runs, fed from that tool's own answer. A run that resumes a previous
  session's checkout — or takes one over through `reclaim` — never calls that
  tool, so the field stayed empty for the whole of its life: the run covered
  the base checkout it swore in, which is the one directory it does not edit,
  and every write it actually made read as `Outside` and went through ungated.
  Silent, and reached by never writing the checkout down rather than by any
  gate deciding wrongly.

  `claim`, `reclaim` and every renewal now discover it. Deterministically, and
  from what the transport already writes: the checkouts git has registered,
  and the ownership marker in each one's private admin directory naming the
  issue it belongs to. Never from the directory a call happened to run in —
  that would make coverage a property of the host rather than of what the run
  holds. Three things it declines to do, because a repair is not an authority:
  more than one registered checkout naming one issue records nothing rather
  than preferring the newest or this run's own id; a failed listing or an
  unreadable marker records nothing and turns no otherwise-successful call
  into a refusal; and a pointer that already names a checkout is left as it
  is. `repo_dir` still names the checkout the claim was sworn in — this adds a
  cover and removes none.

  The registry gained a `RegisteredWorktree { path, branch }` for it, and the
  path this discovery records is put through the same canonicalisation
  `start_branch` already applies to its own writes, so the field's two
  writers now agree on one spelling.
- **A phantom run pointer is reconciled against the tracker before it is refused by name.**
  Two or more run pointers naming one checkout refused
  `several-runs-hold-this-checkout` unconditionally, naming *runs that no longer
  exist* whenever one of them had since been closed out from under it — an issue
  delivered or closed by a third checkout leaves its pointer on disk with
  nothing to expire it. `guard::adjudicate_action` now asks the tracker about
  each named holder first, through the same call `gate` already makes for a
  renewal, and drops one only on an explicit `issue-not-open`; every other
  answer, including a failed read, keeps it counted. Guarded to two or more
  holders — a **single** pointer whose issue is closed by a close this harness
  cannot recognise as that run's own delivery still refuses through its own
  `issue-not-open`, unchanged. A close it does recognise this way answers
  `closed-by-own-delivery` instead, so the pointer reads `Live` and there is
  nothing to drop; folding that single pointer into this reconciliation
  instead would not repeat the `issue-not-open` refusal, it would erase it —
  dropping the lone holder falls to zero, which answers `Outside(NothingSworn)` —
  where the gate stands aside — only when nothing else there denies first: an
  unfinished sibling review still denies `complete_review_receipt_not_selected`,
  and any unreadable pointer machine-wide still denies `run-pointers-unreadable`.
  `N == 1` is left untouched for that reason.

  **The refusal now names a command that clears it — for the holder the tracker
  itself says it clears.** It listed "release the runs that do not" with no way
  to act on it. The first fix offered `estigia release` for a holder whose
  pointer named no worktree, which is wrong in both directions: `claim` writes
  `issue`, `state` and `repo_dir`, and writes `worktree` only when it adopts a
  checkout that already existed to discover — a fresh `claim` ahead of its own
  `start_branch` still writes none, and `start_branch` itself, *after* the
  claim, is what fills it for that ordinary case — so a live run between the
  two still has no worktree and the message offered to release its live
  claim; and a session that died right after `start_branch` leaves a pointer
  naming a worktree nobody works in any more, read as live because it named
  one. A pointer's own fields are not a liveness signal in either direction.
  The tracker's own answer is: reconciling now classifies every surviving
  holder as the issue closing under it (dropped, as before),
  `not-current-live-holder`, or live. Only a
  `not-current-live-holder` is ever named for release; two or more genuinely
  live holders is real ambiguity, and the resolution names no command rather
  than guess, because no CLI verb exists for isolating a checkout the way
  `start_branch` does.

  `not-current-live-holder` is three timelines, not one, and the command does
  not clear all three the same way. No acquisition for this run ever existed —
  `nothing-to-unassign` — and only the local pointer goes. A stale acquisition
  of this run's own, or a live one that lost a claim race, is found on the
  tracker's timeline and actually ended there under the operator's own
  identity, a real write and not a no-op: a stale claim is a claim that is
  really there, only lapsed, not a claim that "is not there". `release` now
  recognises `nothing-to-unassign` and forgets the local pointer instead of
  leaving it to answer the same refusal again, in both shapes, and only once
  the pointer's own recorded repository is the one `release` was actually run
  against — a successful read of the wrong project's timeline forgets
  nothing.

  **`doctor` reports the residue reconciliation leaves alone, asked about the
  right repository.** A new `stale-run-pointer` row names every readable
  covering pointer whose claim the tracker refuses as `issue-not-open`, not
  only one — a single such pointer still refuses the checkout it names, which
  is the case reconciliation deliberately leaves alone. A close this harness
  recognises as that run's own delivery is not in that population at all: it
  answers `closed-by-own-delivery`, so it is never named as residue with a
  release command — it lands in the row's unread arm instead, and is
  surfaced only when no covering pointer in this repository is stale; the
  row returns on the stale arm first, so an own-delivery pointer sharing a
  checkout with a genuinely stale one is never reported. Two or more
  covering the same checkout, with nothing live covering it besides, is the
  case reconciliation reconciles away instead, to `Outside(NothingSworn)`. Filtered to pointers
  covering this repository first: `session::holdings` is machine-wide, and
  asking every pointer using this checkout's own `repo_dir` let a live
  pointer for another project's issue collide with this one's numbering and
  be reported as this project's own stale claim. A tracker read that fails
  is reported as unread rather than
  folded into either verdict, and the row joins `silence` as one that reports
  on the past rather than blocking a fresh claim's readiness. `docs/honesty.md`
  states the gap reconciliation still does not close and why: `GateContext`
  carries no caller identity, so the gate cannot tell a run's own delivery
  from a dead stranger's pointer by the id alone.
- **A `pre-push` hook now narrows several holders to the one whose own branch is
  being pushed, before the tracker reconciliation above ever runs.**
  `guard::adjudicate_action` takes the local refs `git` hands the hook on
  stdin as a fifth argument; `narrow_to_pushed_branch` resolves each to a
  worktree through a freshly-read `git worktree list`, and returns the holder
  list unchanged unless every ref resolves to one worktree owned by one
  holder and every ref agrees on that same holder — never a union over
  whatever happens to resolve, and never a guess between two candidates. A
  push previously refused `several-runs-hold-this-checkout` at a checkout two
  or more runs hold is now decided on that one holder's own claim instead, at
  the boundary. This also changes what the ambiguity refusal used to catch
  incidentally: a push aimed at the shared base branch used to be blocked by
  that refusal whenever two or more runs held the checkout, and now is not,
  whenever the local ref names one holder's own branch (`docs/honesty.md`'s
  "Nothing adjudicates what a push is aimed at" says what still does not
  read the destination). `guard::tests::a_push_naming_one_holders_branch_is_decided_on_that_holders_claim`
  and its siblings drive every fail-closed road; `tests/pipe.rs`'s
  `a_pre_push_hook_is_decided_on_the_holder_the_pushed_branch_names` drives
  the same wiring end to end through a spawned `estigia hook pre-push`.
- **The delivery step in `skill/SKILL.md` now names release.** Developer step 6 ended at
  merge, tag, renew, transition to `done`, record evidence and close — never release —
  so a run that followed it exactly left its own pointer on disk naming an issue it had
  just closed. That pointer is not residue the reconciliation sweeps up: a close this
  harness recognises as the run's own delivery answers `closed-by-own-delivery`, so the
  pointer classifies `Live`, nothing calls it stale, and what it costs is the next run's
  first write in that checkout. The step now ends `close, and release`, and says plainly
  that `release` never checks whether the issue is open, so one a delivery just closed
  releases normally. This is prose an agent reads, not a gate — no code refuses a
  delivery that skips the step — so `tests/pipe.rs`'s
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down` drives `claim`, `transition --to
  done`, and `release` end to end against an issue the fixture closes in between, proving the step
  succeeds rather than asserting on its wording. `tests/honesty.rs`'s
  `the_delivery_step_still_names_the_release_it_spends` guards the prose itself, scoped to the
  numbered step 6 line and requiring it match exactly once, so the user-visible payload — the
  sentence that tells the agent to release — cannot be silently reverted once `pipe.rs` has proved
  the mechanism it describes actually works.
- **The decision-gate crossing holds a subset, not a count of arrows.**
  Rewriting `done` as `review` used to leave the suite green: the assertion was
  `destinations.len() >= STATES.len()`, which a duplicated arrow satisfies while
  a state is unreachable. It is a subset relation now, and that mutation reddens.

  **And two companion documents stopped re-issuing the routing.** The incident
  ledger's I09 restated it in full — wrong specifications to analysis, unbuilt
  external waits to blocked, ordinary built delivery waits to review — and
  `repository-delivery.md` carried a five-row table of the same thing. I09 states
  its lesson now and both leave the destinations to the table their own pointers
  name. The contract itself lost a copy too: `SKILL.md`'s step 6 had the same
  enumeration in its tail and now points at the table instead, while step 7 keeps
  a restatement whose whole subject is the distinction. Four copies in all.

  **One guard holds their return and a second was withdrawn.**
  `the_decision_gate_is_not_restated_as_a_second_table` reddens on the table this
  change deleted, restored byte for byte, and on an arrow of the table's own form
  in any of the four companion documents — and it is blind to that same table with
  its header cell renamed, which a judge measured.

  The withdrawn one counted how many states a *source line* named, and four blind
  judges measured it wrong rather than incomplete. It did catch I09's column, the
  copy it was written for. It missed the hard-wrapped handoff bullet in the same
  file, which restores byte for byte and passes on its line wrapping alone; it
  could not see a routing table at all, which names one state per row; and joining
  two lines of ordinary prose reddens it without a word changing, because the
  pair that does it enumerates the state vocabulary and enumerating the names is
  not restating the routing. That last one decided it: a guard that reddens on
  correct prose is aimed at the wrong object, and it measured a line while the
  rule it was written for is about a statement.
  `docs/honesty.md` records what each holds and what neither does.
- **The board mirror leaves a card that belongs to another repository alone, and says whose it is.**
  Matching on issue number alone moved another project's cards, measured once per
  write path: creating an issue here on 2026-08-18 moved the other project's
  card of the same number to `Ready`, and transitioning one a day later moved its
  same-numbered card out of `Review`. The card is picked by repository now, and `audit_board`
  reports foreign cards rather than repairing them.

  **A foreign card is left alone, and the transition it collided with still
  runs.** The first shape of this refused the whole call, which was wrong twice
  over: it aborted a `transition` whose label had every right to move — on a
  shared board a number collision is the normal case, not a misconfiguration —
  and in `create` it answered *nothing was written* over an issue `gh` had
  already filed, leaving a caller who retries to file a duplicate. The mirror
  runs before the label edit and is best-effort by construction, so what it does
  with somebody else's card is leave it and say so.

  **The read-back picks the same card the writer picked.** It matched on number
  while the writer matched on repository, so a correct transition passed or
  hard-failed depending on the order the API listed the cards in: the label
  landed, the read-back read another repository's column, and the call reported
  that nothing was written. That was a third spelling of one rule, contradicting
  the two that agreed.

  **And a card that does not say where it comes from is not this repository's.**
  `belongs.is_empty() || belongs == home` took a content node carrying no
  `repository` as ours and mirrored it, leaving the `an unnamed repository` arm
  unreachable. That spelling arrived mid-change, in a commit whose subject was
  not asking `gh repo view` when the board is off; the first commit had it
  right.

  **A repository whose identity cannot be read mirrors nothing**, and cannot be
  confused with a repository named "". The mirror takes an `Option`, so a failed
  `gh repo view` arrives as `None` and the answer says the question could not be
  asked rather than reporting a sentence about the card. The identity is read
  once, before anything writes: it was asked again through `?` below the label
  edit, where a failure reported *nothing was written* over an edit that had
  landed.

  **The audit says how many cards were this repository's to check**, beside how
  many the board returned: `compared` is the total less the foreign ones set
  aside. Reading only the total told an operator a pass had examined cards it
  never looked at.

- **The gate records which role each delegated context ran as, for the length of the run.** Claude
  Code sends `agent_type` on every tool event fired inside a sub-agent, so the run pointer now carries
  what the gate **saw** rather than what a launch prompt declared, and `SessionStart` reads it back on
  a resume.

  Recording it on the allow path alone recorded nothing for exactly the contexts this is about — a
  judge measuring in its own directory makes calls the gate answers `Outside`, not `Allow`, and the
  reads it makes never reach the gate at all — so the store is keyed on the role set having grown as
  well. `docs/honesty.md` carries what that still leaves empty.

  **It is not an audit trail.** The pointer is keyed on the session and `SessionEnd` removes it, so no
  later run reads it; that change’s criterion asking for one is split out separately. One publication
  claimed otherwise and two judges disproved it through the built binary; `docs/honesty.md` carries
  the measurement rather than a correction made quietly.

- **A delegated role can now do the work its own contract asks of it, and the judges that measure run
  as that role instead of outside it.** Estigia shipped six role definitions and **not one of them
  could run anything**: the reviewer carried a fixed `tools: Read, Grep, Glob`, and the planning
  phases' substitutions were `Read, Grep, Glob`, `Read, Grep, Glob, Write, Edit` and
  `Read, Grep, Glob, WebFetch, WebSearch` — two of them strict supersets of the third, and one of
  those two writes. So *read-only* was never the property all six shared; **no branch produced a
  shell** was. In a repository whose findings are established by running something — building the change, reddening a test, turning the
  fix off to see the suite go green — that reviewer cannot check anything, so every panel worth having
  was launched under a generic type, outside the role and outside every guarantee the contract hangs
  on it. Measured before this landed: twelve independent judge launches, and the
  read-only guarantee governed **none** of them.

  A new setting, `Evidence standard`, states the one fact only an operator knows — whether a verdict
  here is backed by reading or by running — and the reviewer's tool grant is derived from it instead
  of being a constant. `measuring` adds a shell **and nothing else**: `Write`, `Edit`, `Agent` and
  `Task` stay refused. That withholds four tools and not the capability — a shell writes, and it
  launches — so what confines a measuring judge is the isolation rule, not the grant. The default is
  `reading`, which is what every installation already had, so no upgrade widens anything by itself.

  Two consequences, both deliberate. `reviewer_is_static` used to mean one spelling and now means one
  of the spellings Estigia can produce, enumerated under an exhaustive match so a third standard
  cannot be added without arriving there. And the gate carries the row in `GateContext` beside the
  renewal window, narrowed the same way for the same reason: an unreadable contract answers `reading`,
  because a fault must not hand out a capability.

  The isolation rule moved with the capability rather than after it. It bound a run's checkout and
  said nothing about anywhere else a delegated context writes; it now covers every location. That was
  not a precaution — a five-judge panel sharing one scratch directory had one judge's script
  overwritten by another's and executed inside a third judge's checkout, while a fourth read the
  implementing run's planning notes. No checkout rule was broken and two verdicts still stopped being
  independent readings.

  `docs/honesty.md` carries what this does not buy, and the sharp edge is worth repeating here: a
  judge that measures writes inside the directory its launch hands it, and giving the role a shell
  does **not** put its mutations under the gate. What makes them safe is the isolation rule placing
  that directory outside the claimed checkout — a rule an orchestrator follows. A judge handed a
  directory inside it is measured against the launching run's claim and allowed.
- **A discharged human decision is not a verdict.** Returning built work from
  `blocked` to `review` still requires the configured panel against the exact
  latest receipt. Step 7 now routes an unresolved choice about what should be
  built to `analysis`, the same as a contradiction.

- **Delivery proof no longer inherits a steered Git environment.**
  `same_git_repository` and `head_of` used to shell out with this process's
  `GIT_DIR`, so an unrelated clone could spend another checkout's verdict.
  Both now use the unsteered invocation the fast-forward proof already built.
  A test with `GIT_DIR` and `GIT_COMMON_DIR` exported goes red against the
  previous callers.

- **`reclaim` takes the same optional `state` `claim` already publishes.**
  A takeover of a `review` issue used to stamp the pointer `in-progress`, a
  state nobody named, and the gate then refused every write with
  `unexpected-state`. The default remains `in-progress` and belongs to the
  caller. `Renew` still fills a missing state — for a call that names one —
  and never overwrites one.

- **A claim whose write lands and whose readback fails can be retried.**
  The run already holds the issue on the timeline; the retry used to mint a
  fresh operation id and refuse `already-owned-by-different-operation`. It now
  adopts the epoch the timeline names, writes no second comment, reports the
  horizon already on the marker, and lets the pointer record `issue`, `state`
  and `repo_dir`. A post-write view failure is a Write, not `read-failed`.

- **A cwd no live claim covers can no longer take a write out of the public gate.**
  payload_cwd still reads the key — the hook may name a checkout this process was not
  launched in — but a directory no holder covers is discarded and the call is answered the
  way the identical payload carrying no cwd is answered. Measured on 2026-08-16: cwd of
  C:\Windows, nested tool_input.cwd of .., and a write carrying cwd of
  C:\Windows were outside with exit zero. The doc no longer claims the model cannot
  compose the key.

- **`verify_claim` treats this run''s own branch-link auto-close as delivery, not as a stand-down.** After merge, the ordinary renewal answers `closed-by-own-delivery` with the delivering SHA and receipt epoch so `transition --to done` is reachable. A close that is not this run''s receipt is still `issue-not-open`. An unreadable closer listing remains a failed read.

- **OpenCode is told to launch Task judges, not to hand off because review-blind is a Claude type.**
  `skill/references/runtime-notes.md` said the reserved role does not cover OpenCode launches and
  that an absent capability uses the durable handoff. Read together, every OpenCode publication
  handed off. The reserved type is a launch mechanism; a host that can spawn a subagent already has
  the capability. The installed `estigia.opencode.md` now states the Task path. Handoff remains for
  a launch that failed. Estigia still cannot prove the panel ran.

- **A run pointer that is there and cannot be opened now stops the write and names the file.**
  `session::load` collapsed every `read_to_string` failure into a fresh unsworn run, so a pointer the
  filesystem refused — a directory at its path, a permission failure, a transient I/O error — read as
  *this run swore nothing*. The gate answers that reading `outside` **before** the tracker is asked,
  so the writes of a run that still held its claim stood aside from measurement while `verify_claim`
  went on answering `ok`: measured on 2026-08-15, when `claude-81d69d3e372497b6` held a claim
  through a reinstall that took its pointer, then published, released CI, merged and closed with the
  gate reading nothing. A disarmed gate looks exactly like a gate with nothing to do, which is why
  this direction is the one that had to change. Only `ErrorKind::NotFound` answers absence now; every
  other read failure loads as unreadable and carries the pointer path and the underlying error into
  the refusal, so `run-pointer-unreadable` — from the gate and from the MCP boundary, which build the
  same sentence — names the file an operator has to look at instead of only the run that lost it. The
  reason word is unchanged, so the documented refusal inventory is unchanged.

  The installer half is the question the issue recorded as **not measured**, and it is a measurement
  now rather than a reading of the call graph: `forget_state` is wired only into the take-out path, and
  `a_live_run_pointer_survives_a_plain_reinstall` sets up every adapter twice over one live pointer and
  asserts the bytes are the same afterwards. `uninstall --all` still takes the state with the last
  agent — the operator's recorded requirement, and `taking_estigia_out_takes_its_own_state_with_it_and_not_before`
  still holds it — so `docs/what-it-writes.md` now draws the line between live run state and what the
  person keeps, where it used to say `~/.estigia/` stays unconditionally.

  **`NotFound` alone does not prove absence, and Windows is where that stops being pedantic.** The
  acceptance criteria asked for a pointer whose parent is a file, expecting a read failure that is
  not `NotFound`; measured, Windows answers `ERROR_PATH_NOT_FOUND` — os error 3, mapped to
  `NotFound` — while Linux answers `NotADirectory` for the same shape. Reading the error kind alone
  would therefore have left the whole point of this change open on one platform in its widest form:
  a state root that is a file makes **every** pointer read as absence, so every run reads as unsworn
  and every gate stands aside at once. Absence is now confirmed against the root rather than taken
  from the kind, and a root that is simply not there is still absence — that is what a machine looks
  like before anything was ever claimed, and refusing it would refuse every first run.

  Two deterministic fixtures reach the arm on both platforms, because the incident itself does not
  reduce to a script: a directory at the pointer path is a read failure that is not `NotFound`, and it
  is posed at the loader, at the gate, and across every MCP tool that takes a run id. One older test
  moved with the fix rather than being kept green: the directory fixture in
  `a_pointer_that_could_not_be_written_says_so` was posing a write failure, and the same bytes are now
  refused one step earlier, at the read — the remaining write failures are environmental, no std-only
  fixture poses them on every platform, and the test says so rather than claiming the coverage.
- **A rejection now rests on a severe finding, and a repair records what it repairs.** Durable review
  evidence reduced every observation a reviewer could make to one bit. A preference about a word cost
  exactly what a reproducible correctness defect cost: a rejection, a republish, a new epoch and a full
  re-review of work that was already settled. This repository paid that on its own deliveries, and
  afterwards the tracker could not tell the two apart.

  `record_review_finding` is a new operation — the twenty-second — writing one immutable marker per
  finding, bound to the exact review transaction, carrying an identity, concrete evidence, stated
  material impact and one of `severe`, `warning` or `suggestion`. It refuses a finding missing any of
  the three, because a classification that cannot be re-run cannot be checked and one with no stated
  impact cannot be weighed. `record_review_verdict` then refuses `rejected` unless **that reviewer**
  has recorded a `severe` finding against that exact receipt — the reviewer's own findings, not the
  panel's pool, since two contexts each holding one suspicion is not one confirmed defect. An
  acceptance carrying warnings and suggestions is still an acceptance and still releases CI.

  Operational failures deliberately keep their existing fail-closed refusals. A missing reviewer, an
  unreadable target or a stale receipt is not a review finding, and recording an outage as a cosmetic
  acceptance is the mislabelling this rule exists to stop.

  The second half is lineage. A publication over an earlier one records the **whole parent receipt**
  and a delta digest covering both ends — **derived from the timeline, never supplied by the run being
  reviewed**, because a run that could name its own parent could name the epoch whose findings were
  mildest. The receipt rather than its epoch: an epoch is not a function of the bytes it names, and a
  finding's epoch field is whatever the finding says it is, so a parent ledger matched on the epoch
  alone could be written into after the fact by recording a finding that named the parent epoch and
  carried the repair's own bytes. The parent's findings stay where they are; nothing rewrites them. What a re-review owes
  is the reference: a finding that reassesses one names it, and the name must exist against the parent
  receipt — an epoch alone is claimable by a marker written after the repair; a `severe` finding new to the repair states whether the repair `introduced` the defect or
  `exposed` one already there. A warning or a suggestion new to a repair owes nothing, because pricing
  the cheap observation is the defect this whole change repairs.

  Every finding must also name the publication **under review**, not merely a well-formed receipt.
  That check was documented before it existed: the first version of this change shipped two sentences
  saying a stale receipt was refused while the operation only validated the receipt's shape. Four
  reviewers found it, two drove it, and the repair is the check rather than the retraction —
  `docs/honesty.md` records that history beside the limit.

  What is checked is shape and reference. Nothing here can tell whether a `severe` finding is severe
  or whether a repair introduced what its reviewer says it did, and the delta digest names a repair
  rather than scoping a review — `docs/honesty.md` carries both limits in those words.
- **A transition to the state an issue already holds no longer strips its state label, and a
  read-back that disagrees no longer says nothing was written.** `transition` appended the removal
  whichever state `--from` named, so `--from done --to done` built
  `gh issue edit N --add-label status:done --remove-label status:done` — a self-cancelling edit that
  `gh` settles by dropping the label. Measured on 2026-08-15: the timeline shows
  `unlabeled status:done`, nothing put it back, and the issue carried **no** state label at all —
  invisible to `list_state` in every partition, and disagreeing with every
  `verify_claim --expect-state`. The removal is now skipped when it names the label being added, so
  the call still ends with the issue in `--to` and there is no window where it is in nothing.

  The half with teeth is the report. `label-readback-failed` is a stop and it carried no `world`, so
  the envelope rendered *nothing was written* with *do not repeat this call* under it — over an edit
  that had already run, and against the one call that repairs what it left. It now carries
  `"world": "committed"`, the road `MutationOutcome::Committed` opened earlier in
  `publish_review`, so the sentence reads *the write landed; what failed came after it*; and its
  resolution names the repair — the same transition with `--from` omitted, which removes whatever
  stale state labels are found and is what actually restored the issue — instead of forbidding it.
  The reason word is unchanged, so the documented refusal inventory is unchanged.

  Both halves are separately measured, and the stand-in `gh` gained a **label store** to measure
  them at all: `nth` lets a scripted world change its mind on a schedule and cannot let a read answer
  with what the write before it did, so every fixture answered the same labels before and after
  `gh issue edit` and a transition that destroyed the label was indistinguishable from one that kept
  it. The board mirror is untouched and still runs first, which is asserted on the wire in the same
  call rather than inferred.

- **CI now answers before the reviewing, instead of after it.** `.github/workflows/ci.yml` gains a
  `workflow_dispatch` trigger, `publish_review` and `republish_review` start **one run of it per
  publication epoch** against the head they just pushed, and `record_review_verdict` refuses to bank
  an `accepted` verdict for a head whose lane concluded red or has not finished. Before this, the
  first cross-platform signal arrived when `release_ci` marked the pull request ready — after every
  verdict had been obtained — so a platform-only defect killed the receipt and every verdict bound
  to it. Measured once: sixteen reviewers across eight rounds discarded for a missing
  `create_dir_all` in a fixture that only POSIX could see.

  Both halves, because the first alone is a rule somebody has to remember, which is the failure
  this repository has concluded against. The refusal sits on `record_review_verdict` because it is the single
  writer of a verdict marker — `release_ci` reads what it wrote — so one enforcement point covers
  every route to one, including the handoff route. Check runs attach to a SHA by construction, so the
  evidence is read for the receipt's own head and `ReviewReceipt` is unchanged: no run id, and
  nothing new for a republish to invalidate.

  **The receipt comment on the timeline now names the lane this epoch got**, because the sentence it
  used to carry — *CI remains blocked while the PR is draft* — became false in the same call that
  writes it. That comment is what a blind reviewer reads, so one operation was answering that a lane
  had started and an accepted verdict waited on it while posting a note saying CI would not run at
  all. It now says which of `started`, `absent` and `unknown` this publication got, and says the
  ordinary pull-request-event lane still waits for the pull request to be marked ready — the draft
  barrier is unchanged, and only the publication lane is new.

  **A head with no check runs at all proceeds**, unchanged. A repository with no dispatchable
  `ci.yml` has none, Estigia does not parse consumer YAML to find out whether it has a lane, and
  refusing there would have broken every consumer in order to protect the ones that answer. Only a
  token that may not dispatch refuses, `publication-lane-forbidden`, and it refuses **after the push
  and before the receipt** — so the branch is on the remote, no epoch exists, and running the same
  call again after granting `actions: write` publishes cleanly. Red and unfinished lanes refuse
  `publication-lane-red` and `publication-lane-unfinished`, each naming the lane, the run and the one
  command that clears it. An unreadable listing is a failed read and never a green lane.

  One run per **epoch** is not the cardinality the draft barrier refused: topic push, open,
  synchronize and reopen still start nothing, and a dispatch does not mark the pull request ready.
  What this cannot do is protect its own delivery — `workflow_dispatch` has to exist on the default
  branch before it can be dispatched against a topic ref — and what it does not prove is in
  `docs/honesty.md`.

- **A run's second issue no longer collides with its own first checkout.** The worktree template was
  made to carry `<run-id>` and nothing made it carry `<branch>`, so a template without one gave every
  branch of a run the same directory: a run working a queue met the checkout it made for its previous
  issue and was refused `worktree-path-occupied` — correct for what the check could see, wrong that
  two branches of one run were asked to share a path at all. The workaround was a manual
  `git worktree remove`, which is the one action that refusal's own guidance says not to take lightly.
  Both scopes are now added in memory when absent, as siblings and never as nested children:
  `…/<branch>` becomes `…/<branch>~<run-id>`, `…/<run-id>` becomes `…/<run-id>~<branch>`, and a
  template naming neither becomes `…~<branch>~<run-id>`. `estigia.local.md` is still never rewritten
  and `template_migrated` still reports the substitution. The sibling rule matters more in the new
  dimension than the old one, because a run-scoped legacy path is a checkout that run owns and a
  nested child would put a worktree inside a worktree. `run_scoped_template` is now
  `scoped_template`, since it scopes two things.

  Worth stating because it is why this took two attempts: the collision was invisible on the machine
  that found it, whose operator had since configured a template naming `<branch>`. Four checkouts,
  one run, no collision — and a configuration difference read as a fix. The composition code had
  never changed. The tests now use the templates an operator actually writes, including the bare
  absolute directory the settings table documents and the skill ships `unset`.

  The `legacy-worktree-registered` stop, its payload key and the binding section that is its only
  written recovery all described one dimension and now describe both. One residual is recorded in
  `docs/honesty.md` rather than closed: the stop cannot name the legacy path of a template that named
  no placeholder at all, so on upgrade git refuses that resume instead of Estigia — before any remote
  write and with nothing lost. The delivery reference, which framed worktree uniqueness as per-run
  only, the framing that hid this defect, says both dimensions now. And a sentence in the operator's
  own recovery claimed a detached legacy checkout does not stop `start-branch`; it does, reporting
  `occupied_by_branch: null`, and there is now a test posing that case rather than a proofread.

- A row about **this machine** now reaches every installed contract, and cannot be given a different
  answer for one agent. `Setting::scope()` has three values and both write paths were written as
  though it had two: the plain `config set` asked `elsewhere()` only for `Scope::Everywhere`, so
  `Summary language` and `Issue body language` landed in the canonical contract and nowhere else;
  and `config set --agent <slug>` refused an `Everywhere` row while accepting a `Machine` one it
  could not hold, since a shared root's per-agent file is rendered through `render_some_agent_rows`
  and read through `Scope::Agent` — the row was dropped on the way out and the command exited on its
  own read-back, reporting `setting-shadowed-by-local-file` and blaming an operator file that need
  not exist in that root at all. Two agents on one machine could therefore answer a question that
  has one answer differently, with **no command that made them agree**, and most adapters share the
  neutral root, so the unreachable case was the common one. The plain form now
  propagates a machine row exactly as it propagates a repository one, the per-agent form refuses it
  and names the command that holds it, and `doctor`'s `canonical` row offers that command instead of
  the sentence saying nothing cleared it — a fork that existed only to describe the gap, removed
  with it. `config set --repo` refuses a machine row saying it is a fact about the machine rather
  than what one agent does, and points at the command that holds it instead of at `--agent`, which
  is the refusal one command later. The three `shadowed*` refusals no longer name a local override
  they did not find: one lookup answers `Option` for all of them, and where there is no
  `estigia.local.md` the refusal is `setting-not-read-back`, saying what was observed rather than
  naming a cause it cannot see. Only the first of the three is reached today — `local_override` tests
  `is_file()`, so an unreadable file still answers `Some`, and a root with no file at all round-trips
  what was written. The other two branches are held by the type rather than by a test.

- `Blind judges` now has canonical `five blind` beside `single` and `two blind`, with `single` still
  the default. Five independent reviewer contexts run concurrently over the identical immutable
  target and criteria; a severe finding blocks or authorizes automatic repair only when 3-of-5
  independently confirm the same finding. One or two confirmations remain suspicions, ambiguous
  identities do not aggregate, and dissent, warnings and suggestions survive. Every Claude Code setup
  installs one inert `review-blind` definition with `model: inherit`; it remains inert unless
  a launch names the active blind mode, exact receipt and criteria. The orchestrator supplies the
  effective `judge` model when it launches that definition twice or five times. Config writes never
  mutate it. Setup refuses an unowned or changed copy before writing other artifacts, and uninstall
  deletes only a textually unchanged owned copy after normalizing line endings and the final newline,
  preserving other changes and deletion retry evidence. Fresh and upgrade action manifests name the
  ownership-record mutation exactly once. The lifecycle
  digest now inventories exactly five SDD definitions and that reviewer in one of its two top-level
  asset collections. Transport still enforces one aggregate exact-receipt verdict only: Estigia cannot
  prove panel size, concurrency, independence, blindness, same-finding identity or quorum.
  `review-blind` is now reserved to the operator-owned Claude definition. Generated Claude hooks wake
  for current `Agent` and legacy `Task` launches, walk from the launch cwd through the first `.git`
  repository root, parse candidate frontmatter as YAML, recursively reject project shadows or
  unprovable candidates, require the canonical user text to be the unique user-scoped reserved
  definition, and enforce the embedded policy once running. Setup performs the same user-tree
  uniqueness preflight before writing, revalidates at the reviewer boundary, and publishes a fresh
  reviewer with a no-replace atomic create so a concurrent file is preserved rather than overwritten.
  Refused launches never count or degrade the panel; ordinary agents remain project-first and OpenCode
  is out of scope.
- `estigia mcp` serves the workflow operations as **21 MCP tools** over stdio —
  claim, verify_claim, heartbeat, transition, comment, reclaim, release,
  start_branch, publish_review, and the read-only checks. Hand-written
  JSON-RPC rather than `rmcp`: the same binary answers a `PreToolUse` hook on
  every edit, so an async runtime per process is a cost paid thousands of times
  to move a few lines of JSON across a pipe.
- The one force-push the delivery path needs is inside the harness. After a rebase onto a moved base,
  or an amended commit, the ordinary push is refused as a non-fast-forward, and the sequence a run
  actually performed was *leave Estigia, `git push --force-with-lease` by hand, come back and publish
  again* — measured on a live delivery where the base moved mid-task. The claim itself was checked
  even then: `git push --force` is a boundary, which always re-reads and has no renewal window. What
  the hand-run push lacked is the rest — no lease against the head a receipt recorded, so it
  overwrote whatever the remote held; no record tying the new bytes to that receipt; and no cover at
  all when typed into a terminal the gate does not watch. `republish_review` is that step,
  adjudicated: it reads the head the latest `published` marker
  recorded and pushes `--force-with-lease=<branch>:<that head>`, so a remote somebody else moved
  refuses the push instead of losing their commit. The renewal stands **immediately** before it,
  after the fetch, target derivation, keyword scan, pull-request listing and draft conversion that
  separate it from the first check — for the fast-forward push that gap costs a refused push, and for
  this one it costs history. An issue with no recorded publication is refused rather than forced over:
  the first publication is `publish_review`'s, whose non-fast-forward refusal is the check that would
  be skipped. It is a separate operation rather than a flag because the last thing this must keep is
  that `publish_review` never force-pushes implicitly, and two entry points hold that where one
  function with a boolean would only ask a reviewer to. **Every round of independent review it went through**, two
  blind contexts each, rejected it before this shape, and every finding is fixed in it. The count is
  deliberately not written: it drifted twice, and a number nothing crosses is a number that will. The ones worth
  naming: the refusals downstream of a write said *nothing was written*, because they reached the
  agent through the same `stop()` and `?` every other path uses and those envelopes carry no `world`
  — they now name **which** writes happened, one clause each, and stay silent when none did; the
  harness did not fill in the isolated checkout for this operation as it does for `publish_review`,
  so an agent omitting the optional argument had its target derived from the base checkout and
  learned about it *after* the force-push, in a message blaming a push nobody had made;
  `estigia config`'s list of the operations this transport performs had drifted from the dispatch by
  three, now crossed by a test rather than by a comment saying nothing catches it; and the guard that
  proves the ordinary publication adjudicates its claim is behavioural, because two attempts to hold
  it by counting a function body were both satisfied by a call the mutated route never made.
- The gate now adjudicates an OpenCode call against the directory it will actually run in. That
  plugin launches the gate from its project root, because OpenCode's plugin context carries a project
  and no session identity to mint a run id from — so with two runs each holding an isolated worktree
  inside one base checkout, the gate was handed the base. Both runs cover it at equal depth, and the
  call was refused *"2 runs on this machine hold this checkout"*: correct about the directory it was
  given, and the directory was the wrong one. Measured on 2026-08-16 with two live holders of this
  repository, a `git commit` and an `estigia config set` explicitly targeting one worktree were both
  refused, and the refusal advised releasing one of the runs — which is the concurrent isolation both
  were using. For a Bash call the arguments the plugin already forwards carry `workdir`, the
  execution directory, and nothing read it; `narrowed_by_the_call` reads it now, for Bash and no
  other tool, resolving a relative one against the directory the process was launched in.

  **It may only narrow to somewhere the gate was already going to look, and that distinction is the
  change.** `cwd` is written by an adapter's hook, which knows what it is gating, and is taken as
  given. `workdir` is a tool *argument*, so whatever composed the call wrote it — a model, on every
  runtime here. Read as freely as `cwd` it stops being
  evidence and becomes a lever: measured under review, with two live pointers and a `git push` under
  a claim, a `workdir` of `..`, of the parent checkout, or of `C:\Windows` resolved, was covered by no
  run, and was answered `outside` with exit **zero**. The command still ran where it was going to
  run; the gate simply stopped adjudicating it, and the same spelling reached `write` and `edit` too.
  A payload that can move the decision is a payload that can leave the gate, which is worse than the
  false ambiguity the key was read to fix — the widened gate that looks exactly like working
  correctly. So the resolved value is **placed** — its spelling collapsed the way the platform
  collapses it, then resolved as far as the filesystem exists — and one landing outside both the
  launch directory and every checkout a pointer already records is discarded in favour of the launch
  directory, which is where the decision sat before this key was read at all. Both blind reviews of
  the unclamped version rejected it on precisely that.

  Placing it is the clamp rather than a refinement of it, and the second round of review is why. A
  first attempt compared with `covers`, which is written for working directories *that exist* and
  falls back to the path as written when resolution fails; `..` was then never cancelled, so
  `wt-a/../../nope` still started with the launch directory and was attributed to the holder of the
  component it climbed **through** — measured as `allow`, exit zero, under a claim the call had
  nothing to do with. Strictly worse than the escape it replaced, which at least reached `outside`.
  `placed` already existed for exactly this, and its own doc names the failure; what it needed was to
  be the thing the gate asked.

  `holders_of` is untouched: its closest-worktree selection was always right when given the real
  directory, and the equal-depth ambiguity at a shared base is genuine and still refused. The alias
  is interpreted in one language rather than two — the plugin forwards and translates nothing — and
  the test that proves the plugin forwards it **executes** the plugin under `node` rather than
  reading its source. Both workflows install `node` now rather than trusting the runner image to
  carry it, on the reasoning the interpreter guard already stated for Python: a step needing a tool
  the job never sets up fails on a tag, which is the worst moment there is. `docs/honesty.md` records
  what per-call evidence exists, for which tools it does not, and the two things about it that are
  still uncrossed — the argument's spelling, and the base a relative one is resolved against.
- A claim governs a repository, not the machine. The gate classified writes by the checkout the hook
  was invoked in and never by the **path being written**, so a scratch note or an agent's own memory
  store, written from inside a claimed repository, was a repository write — and once the issue closed
  on merge it was refused with *issue is CLOSED*, at the exact moment a run had a delivery to write
  down. A write whose path lies outside every checkout the claim covers now stands aside as
  `outside-the-claim`, decided before the tracker is asked so the issue's state never reaches it. It
  is deliberately narrow: only an absolute path, only a run that covers something, and only a
  `Routine` write — the control surface sits outside the repository by nature, and watching it is the
  defence against an agent switching the gate off with the tool it uses most. `gh`'s hosts file joins
  that surface in the same change: it decides which account every tracker call acts as, so it is a
  boundary write in both spellings rather than something the stand-aside carries past. The answer is
  given after the contract check, not before it, so *an unreadable control surface permits no write*
  reaches it as it reaches everything else. That held only outside the renewal window until the contract check moved above it:
  the window's `Allow` answered above the contract refusal, so for its width a routine write went
  through with no `SKILL.md` on disk at all. The contract check now answers above the window, and the
  window still short-circuits below it when the contract is present — so the sentence reaches inside
  the window too, and the fast path it protects is unchanged. A path that **lands** anywhere other than a drive is
  declined rather than placed, and a declined path reads as inside. Windows serves every local drive
  as an administrative share, so `\\localhost\C$\repo\src\main.rs` against `C:\repo\...` is one file
  under two spellings, which stood the gate aside for a write that landed inside the claim. Judging
  the landing rather than the spelling is what also covers a drive letter mapped onto that share, and
  a directory link pointing at it: both arrive wearing a drive and resolve onto the share anyway. A
  share is the only non-drive landing anyone has produced — the device namespaces read as though they
  would be another, and `\\.\C:\Windows`, `\\?\Volume{...}\Windows` and
  `\\?\GLOBALROOT\Device\HarddiskVolume3\Windows` all resolve to `C:\Windows`, so they place normally
  and a write through one of them outside every checkout now stands aside where it used to be gated.
- The files carrying Estigia's own authority are watched. `setup` writes a workflow-authority
  directive — the sentence telling an agent this harness holds the authority at all — into each
  agent's instruction file, and the gate called ten of the eleven `Routine`; OpenCode's was already
  covered by the `.config/opencode/` entry. Those ten, plus gemini-cli's second spelling under
  `%APPDATA%` — it is the one adapter whose file moves with the platform — plus
  `~/.claude/settings.local.json`, the file an operator is told to put machine-local overrides in, and
  `~/.claude/agents`, where a definition carries a tool allowlist: **thirteen**. Each is outside every
  checkout by construction, which is what made it urgent — the stand-aside added for a closed issue's
  scratch note answers before the tracker, so they had moved from *measured against the claim* to *not
  gated at all*. They are `Boundary` now, on both roads.

  The instruction files are derived from the adapter table rather than spelled, because a hand-written
  copy agrees with the installer only until somebody renames one. The population test that crosses
  them against `resolve_paths` gained four dimensions on the way, each because a reviewer found a
  surface behind one: three platforms, two `XDG_CONFIG_HOME` layouts, both roads, and the bare root as
  well as a file inside it. What that uncovered is in the same change —
  `%APPDATA%\gemini\settings.json`, where the Gemini gate is registered on Windows; opencode's plugin
  and crush's settings, anchored to what the installer writes *beside* the directory entry rather than
  replacing it; and the rules **directories**, because gating a filename inside a directory the host
  reads whole is defeated by a neighbour. A fragment ending in `/` names a directory now, matched by
  what is under it or by itself, and not by a name that merely ends alike — nor by one that
  starts alike, because a fragment naming a directory is anchored on the left as a dot-fragment
  is. Anchoring only the dot ones kept the two roads disagreeing on the four entries this change
  added: `surface_of` gives every token a trailing separator, so a bare `.opencode/agents` was
  `Routine` to `Write` and `Boundary` to `rm`, with nothing red because no fixture asserted the
  `Routine` direction for a fragment without a dot. What stays unanchored is the fragments naming
  a **file** without one, and `cli/hosts.yml` is why: `%APPDATA%\GitHub CLI\hosts.yml` holds it
  mid-segment. `docs/honesty.md` measures what that leaves.

  Anchoring cost coverage on the shell road three times before it was right, and each loss was a
  spelling the base commit had gated with a bare `contains`. Splitting on whitespace and appending a
  separator left a relative operand with a space in front of it, never a separator, and never at
  position 0 because the verb is there. Wrapping each token reached only the operands whose first
  character *is* the fragment's — a quoted operand, a redirect written with no space, and an operand
  joined to a long flag each put a character in between, and the drive-relative Windows `C:` did the
  same while also reaching the **write** road. Every one of them named the run pointer or the file the
  gate is registered in, by the shortest line there is, and every one of them left the whole suite
  green, because the fixtures here spelled their commands plainly and their paths absolutely.

  What is in the code is one rule rather than a list of shell shapes: every character a path segment
  cannot contain is read as a separator, and the line is wrapped in one. Four fixtures hold it — three
  for the surfaces, one for the ordinary commands a run issues all day, since a more permissive view
  is a live tracker read on every build if it over-matches. Reviewers measured all three attempts;
  `docs/honesty.md` carries the sequence, and says what the folding still cannot see.

  `<repo>/.opencode/agents` joined the list for the same reason it was missed: `definition_for`
  searches it beside `<repo>/.claude/agents` in one `vec!`, `.claude/agents/` reached the first,
  and `opencode/agents/` is anchored so `.opencode` is not it. A definition that is not found is
  `Ok(None)`, which `declared_policy` reads as *every tool allowed* — so the file that writes an
  agent its own tool allowlist rode the renewal window, while two documents said every definition
  root was watched and a fixture listed that root among the ordinary paths. All five roots are
  crossed now, on both roads.

  `harness::roles::definition_for` also stopped spelling OpenCode's definition root by hand: it now
  searches the default root **and** the one `XDG_CONFIG_HOME` names, through the same rule `setup`
  uses. Searching only one meant a definition it did not find — which `declared_policy` reads as
  *every tool allowed*.

  The declared population was **restated** rather than stretched: the gate reads nothing from an
  instruction file, so it covers what an agent is *told* as well as what the binary *enforces*. The
  cost is recorded with it — a `Boundary` never rides the renewal window, so it is a live tracker read
  on every write — counted, 30 tracker calls out of 30 against 0 out of 30 for a routine one — and a
  refusal rather than a delay with no network. Three independent measurements of what that read costs
  disagree by a factor of two (0.61-1.22 s, 0.58-0.92 s, 0.85-3.20 s), so `docs/honesty.md` states it
  as a round trip between half a second and three seconds rather than quoting the first sample as a
  bound.
- A test can no longer report pass without running. Sixteen tests in `tests/pipe.rs` drive the real
  binary against a stand-in `gh`, and that stand-in is a cargo example; the rig answered `Option` and
  every caller opened with `let Some(rig) = … else { return; }`, so wherever the example was missing
  all sixteen returned early **reporting pass**. Measured at the base commit with the fixture moved
  aside: `cargo test --test pipe` answered *106 passed*. The skip is now gone from the type
  rather than from the callers — the rig raises, naming the command that fixes it, so there is no
  value it can return that means *did not run*, and no later caller can reintroduce the early return
  by copying its neighbours, which is how sixteen came to have it. The fixture is located from the
  running test binary rather than from a hardcoded `target/debug`, so it is found under whatever
  profile and target triple built it. Which invocations were exposed: a bare `cargo test` builds
  examples and always did, so CI was not blind; a **filtered** run — `--test`, `--lib`, any target
  selection — does not, and that is how every mutation measurement in this repository is taken.
- A refusal can now say that it already wrote. The outcome an agent is told was derived from the exit
  code alone, so every stop reported *nothing was written* — including `publish_review` refusing a
  closing keyword after it had pushed the branch and opened the pull request, which left both
  orphaned for a run that believed the message. `MutationOutcome::Committed` already carried the
  right words and the transport had no way to reach it; it does now, and a refusal that does not
  claim to have written behaves exactly as before. The same operation also scans for a closing
  keyword *before its first remote mutation* — before listing pull requests, before a reused one is
  drafted and its body replaced, and before the push — so that refusal leaves the remote exactly as
  it found it. Reads precede it, the claim renewal and the base fetch, and no write does. That scan is one function now rather than two that disagreed about what a `git log`
  which did not answer means; it refuses, on both sides, because a source nobody read is not a source
  with no keyword in it.
- Review handoff is now a durable, receipt-bound compound operation. It records and reads back the
  latest review transaction and exact ownership epoch before idempotently unassigning, keeps the
  issue in `review`, and excludes the publishing/requesting run from selection and reclaim until a
  distinct reviewer's immutable exact-receipt verdict. Rejection returns the work for repair. Timed
  requests record one deadline without scheduling or treating expiry as review.
- Delivery now requires one accepted `review_verdict` over the exact latest receipt, crediting a
  reviewer that is neither the publisher nor any run that asked for review. Both routes record it:
  after a handoff the reviewing run names itself, and a run that acquired a reviewer without
  releasing the claim names that reviewer, marked `self_attested`. The requirement is unconditional
  precisely so that no deletion can manufacture evidence that was never recorded; `docs/honesty.md`
  names the narrower cases where a deletion restores evidence a later marker disqualified.
- Review publication now forms a cooperative draft/ready CI barrier. `publish_review` drafts and
  confirms reused PRs before push, creates new PRs draft, and records a fresh epoch over
  PR/head/base/clean-target digest, invalidating old evidence even for identical bytes.
  `release_ci` re-verifies the live review claim, latest full receipt, current draft PR and
  coherent re-derived clean target before marking the PR ready and confirming every ambiguous outcome
  by readback. GitHub cannot condition ready atomically; out-of-band collaborators and repository
  workflows can bypass the order, so no malicious-collaborator authenticity is claimed.
  Topic pushes and PR open/synchronize events no longer start CI; default-branch pushes
  remain, PR CI starts on `ready_for_review`, and — since the publication lane above —
  `publish_review` dispatches one run of the workflow per publication epoch.
- `estigia hook <event>` gates repository writes. Once a run has claimed an
  issue, every write is measured against that claim and every irreversible
  boundary re-reads the tracker timeline. A run that has sworn nothing is not
  under Estigia's authority — the oath binds once sworn.
- The tool-call gate reaches **every agent Estigia knows** — Claude Code,
  Codex, Gemini CLI, Qwen, Cursor and OpenCode — in three dialects and three
  file shapes, each from that agent's published reference or its source. None of them spells a decision the same way, and a gate answered in
  the wrong dialect runs, decides correctly and is ignored. The OpenCode plugin
  declares its own known hole — it does not see subagent calls — in the file it
  installs. For the three agents with no gate, `estigia status` says why and
  what would close it — "gate off" alone leaves an operator unable to tell a
  declined gate from an unavailable one.
- `estigia guard` installs a `pre-push` hook: the one gate no agent can route
  around, because it sits under git rather than under an agent. It refuses a
  push that no live claim authorises, whoever typed it, and it refuses to
  replace a `pre-push` somebody else wrote.
- **The tracker's timeline is the only source of truth.** No local database of
  claims, so two runs on two machines still adjudicate against each other and a
  run that dies leaves no *authority* behind — whatever the timeline says still
  governs, whoever wrote it. The local run pointer a dead session leaves is a
  different question, never authority, and is what the reconciliation
  checks against the tracker rather than trusting on its own.
- `estigia doctor` checks, read-only, that everything a run needs before it
  swears actually works: the skill, the transport, a Python that runs, an
  authenticated `gh`, and a git remote. Every failure names a resolution.
- `start_branch` now **advances the caller's own base branch** to the tip it
  just fetched. Every new branch already started from `origin/<base>`, which the
  preflight refreshes and reads back — so worktrees were never stale. What was
  stale is `refs/heads/<base>` itself, leaving `git log main` in the primary
  checkout describing a base no worktree had been cut from for days. It refuses
  every state it cannot prove safe, and each refusal is a different way the
  obvious `git pull` here goes wrong: a **diverged** base would discard commits
  the remote has never seen, an **unestablished ancestry** is not clearance,
  moving a ref under a **dirty checked-out** tree leaves the index describing a
  commit the files do not match, and a base **checked out in another worktree**
  is the same corruption in the tree this run cannot see — `git update-ref` does
  not refuse that one, measured at exit 0, after which the untouched checkout
  reports `D <file>` for a file nobody deleted. The outcome — advanced, current, or the hold
  reason — is reported as `base_advance`, and it can never fail the run: the
  worktree's start point does not depend on it, so trading a real branch for a
  cosmetic ref update would be the wrong exchange.
- **A verdict may now say what it rests on.** `record_review_verdict` carried nine arguments and no
  field for the basis of the outcome. `basis` is a new optional argument, plumbed the way `evidence`
  and `impact` already are on a finding — a declaration Estigia stores and reads back, not a claim it
  re-runs. A supplied empty or whitespace-only string is refused rather than folded into "none",
  because the two callers said different things; an absent `basis` still parses from markers written
  before this existed and still qualifies delivery. The requirement itself is one constant,
  `skill::VERDICT_BASIS_REQUIREMENT`. At introduction the `reading` rendering omitted those bytes;
  both arms carry them now (see above).

  **The requirement's own words are now crossed on all six carriers that can state them:** the
  `measuring` reviewer definition, the OpenCode `## Review on this host` block, and
  `skill/policies/blind-judges.md`'s section, plus three more that a first pass missed — the MCP tool
  schema's `basis` argument description, `skill/SKILL.md`'s developer step 5, and
  `skill/bindings/github.md`'s `review_verdict` row, each of which had told a caller to *omit* `basis`
  when nothing was measured, the opposite of the requirement's own second sentence that measuring
  nothing is a legitimate basis when it says so plainly. The crossing test checks the requirement's
  exact words against all six; on two of those six it checks the one physical line that must carry
  it instead of the whole file.

  **A second, separate defect survived that crossing: guidance contradicting what the requirement
  means, on carriers that already stated its words.** `skill/policies/blind-judges.md`'s own guidance
  to a `reading` judge, `skill/bindings/github.md`'s `blank-review-basis` refusal row, and that
  refusal's own runtime `detail` string all told a caller or a judge that leaving `basis` blank was
  fine when nothing was measured; all three now ask for what was executed, or a plain statement that
  nothing was, with omitting the field entirely still the other accepted call. `docs/honesty.md`'s own
  entry on this field pre-dated the requirement and said there was "nothing for such a judge to name";
  it now says such a judge has no *commands* to name, only the fact that it ran none.
  `blank-review-basis` is new in this same release, so nothing is being corrected for an installed
  version there.

  **An existing `measuring` install needs one step on upgrade.** The `measuring` rendering moved, so
  the `review-blind.md` already on disk is no longer bytes this binary can produce: every blind-judge
  launch refuses with `reviewer-canonical-unavailable`, and `setup` refuses its whole preflight with
  `reviewer-definition-changed`. Move that path aside and let `setup` write it again — the launch
  refusal's own suggestion is one step short on this upgrade, so an operator meeting it first should
  go straight to the second refusal's recovery. The `reading` rendering is byte-identical, so an
  install on that standard upgrades with nothing to do.

- **`record_review_verdict`'s answer now carries the `basis` text itself, not only `basis_given`.**
  A replay of an already-recorded operation id answered `basis_given` alone, so a caller replaying with
  different wording than the persisted verdict had no way to see that its own text was discarded or
  which body actually stands — the binding's own row already promised "a replay answers with the basis
  already recorded rather than replacing it," and nothing in the answer let a caller check that. The
  replay branch now reports the persisted `basis`, and the first-recording branch reports the same field
  on its own verdict for the same shape on both paths. Caught by two blind judges; the replay branch's
  own test used one value for both the persisted marker and the replaying call's own `--basis`, so a
  branch that read the wrong one of the two could not fail it — two of its four cases now supply a
  different value on each side, in both directions of mismatch, and every case asserts the answer
  matches the persisted one.

- **A `basis` marker attribute that is whitespace only, built through `markers::render` or
  hand-typed with the literal escape `basis=%20%20%20`, used to read back as `basis_given: true`;
  it now reads back as `false`, the same as the writer already refuses it as.** The marker reader
  dropped only an exactly-empty `basis` attribute, `!value.is_empty()`, while the writer's own
  `blank-review-basis` refusal has always tested `trim().is_empty()`. The reader now applies the
  writer's own test. This changes what an existing marker
  carrying a percent-encoded whitespace-only `basis` reads back as; nothing changes for a marker
  with no attribute, an exactly-empty one, or a non-blank one. The rule a caller is actually held
  to, on both the write and the read side, is that `basis` must be neither empty nor
  whitespace-only.

- **`Flags::read` no longer infers a switch from the shape of the next word.** Before this, a flag
  not on a hand-maintained list was read as a switch whenever the word after it started with `--`,
  which holds for an actual switch (`--force --json` must not swallow `--json` as `force`'s own
  value) but not for a flag whose value is free text that itself happens to start with `--` — exactly
  what `basis` can send. A basis beginning with `--` was read as though `--basis` carried no value at
  all, `f.get("basis")` answered `None` for a caller that had supplied one, and the blank-basis
  refusal — which exists to tell absent and empty apart — never saw either. `SWITCHES` now
  names every flag this transport reads with `on` rather than `get`/`need`/`number`/`path`, and
  `a_switch_nobody_passed_is_off` checks that list against the file's own `.on("...")` calls; every
  other flag now takes the next word as its value unconditionally, whatever that word looks like.
  This changes argument parsing for every operation the transport reads, not only `--basis`.

- **`escape_control_input` now neuters any `<!--` that does not close within the caller's own
  fragment, not only one that opens this transport's marker vocabulary.** An opener that does not
  close is the hazard on its own terms: `markers::parse` scans forward from any `<!--` to the *first*
  `-->` it finds, wherever that is, so an unclosed opener in a caller's body borrows the next real
  marker's own close, reads everything in between — including that marker's opener — as one long
  comment, and lets `parse` resume past a verdict or claim nothing ever read. Every
  caller that escapes before posting or hashing runs it: `comment` escapes directly before appending
  its marker, `heartbeat` does the same, `protocol_body` runs it for the three review writers
  (handoff, finding, verdict) over the prose and the handoff's own `blocker`/`discharger`, and a
  forced `reclaim --reason-file` escapes its text before both hashing it into
  `forced_reclaim_hash` and posting it, verbatim, in the reclaim comment's own body — so an
  unclosed opener there is visible in the posted text too, not only in the digest.

- **The lapsed-self exception now reaches an acquisition a foreign `reclaim` pushed out of the live
  epoch.** `claim` already recognised a reused id naming a **stale** acquisition of
  its own run, but that check read `holding().stale` only, and `reduce_ownership` truncates the
  timeline at the last takeover. A run whose own claim had lapsed and was then reclaimed **and
  released** by another run therefore sat in neither `live` nor `stale`, and the reused id met the
  old lockout again — `operation-id-kind-conflict` when the dead marker was a `reclaim`,
  `claim-metadata-mismatch` when it was a `claim` — while `reclaim` answered
  `nothing-to-reclaim`. The dead-self check now reads the whole timeline and fires whenever the
  reused id names an acquisition of this run's own the timeline no longer holds live, so it answers
  `claim-operation-expired` and the harness reissues under a fresh id, the same way it already does
  within one epoch. The foreign takeover stays recorded: the reissue posts a `claim` and edits
  nothing, and `reclaim` is untouched.

- **A neutralising verb now reads the repository's own configuration, closing the last shell road
  that did not.** Every write to `.git/estigia/estigia.local.md` was gated, and the
  `REPOSITORY_SHELL` and `shell::writes_a_file` arms asked that file's question beside the
  control-surface one — but the `NEUTRALISES_A_FILE` arm asked `surface_of` alone, so
  `chmod 000 .git/estigia/estigia.local.md` answered `Untouched` while the same line on a control
  surface was a `Boundary`. The three verbs now ask `names_a_boundary` — the control surface, or the
  repository's own Estigia configuration — the predicate every other shell arm asks, so a neutralised
  repository configuration is a boundary the gate denies on. The population declaration and its
  affirmed fingerprint moved with the boundary sentence; the three verbs and the disposition are
  unchanged.

### Setup

- **Pi is an adapter.** `estigia setup pi` fences the directive into
  `~/.pi/agent/AGENTS.md`, registers the MCP server in `~/.pi/agent/mcp.json` and writes the owned
  gate extension `~/.pi/agent/extensions/estigia-workflow-authority.ts`, whose `tool_call` handler
  hands `edit`, `write` and `bash` to `estigia gate --runtime pi` and blocks on a refusal. Skills
  come from the neutral `~/.agents/skills`. What was read from Pi 1.1.0 rather than observed in a model turn is in `docs/honesty.md`.

- **Pi relays a sealed reviewer in process.** `estigia setup pi` also writes
  `~/.pi/agent/extensions/estigia-review-relay.ts`, which registers one tool, `estigia_review_lens`.
  Pi has no sub-agent tool to intercept, so the tool asks `estigia review prompt` for the prompt and
  the new `estigia review model` for the model the `reviewer` routing row named, runs one tool-free
  completion through `ctx.modelRegistry`, and pipes the raw answer into `estigia review admit`; the
  answer never reaches the model that called it. `inherit` or no row runs on the session model, and a
  row Pi's registry cannot resolve is refused `reviewer-unavailable` rather than answered by another
  model. `Capabilities::review_relay` is `true` for Pi, `review_relay_installed` replaces the
  OpenCode-only check `select_reviewer` made, and Pi's override file carries its own fenced review
  section. `docs/what-it-writes.md` describes the extension; `docs/honesty.md` says what was not
  observed live.

- **A shared-screen save reaches every installed contract, and a per-agent screen offers only the
  agent's own rows.** `config set` carried a row that is not per-agent — about the
  repository or about this machine — into every installed contract, while the guided screen behind
  `config edit` wrote only the target it was opened on: a machine-wide answer the screen saved
  stayed in one root and the others kept the old value. `cli::write_edited_table` now carries every
  changed non-agent row into every configured root, refusing on a root that does not read it back
  through the same `shadowed-elsewhere` shapes `config set` uses; `cli::elsewhere` takes the
  caller's `SetupOptions` instead of building a default one, so a sandboxed save cannot reach the
  real home. `config edit --agent <slug>` offers only that adapter's own rows, so the screen and
  `config set --agent` agree that a machine-wide or repository row is not one agent's to answer.
  `docs/honesty.md` no longer records the gap.

- **`status` says why the tools half is off, and separates an impossible registration from a missing
  one.** `tools off` was one word for four states — an adapter with no verified
  registration site, one whose site is known and simply not written yet, a file that cannot be read
  or parsed, and an entry under a key this build does not look under — and the only prose on the row
  answered the *gate* half, so the operator who asked why Estigia's MCP was absent in ZCode had to
  read `supports_mcp()` to find out. `AgentAdapter::tools_gap()` now returns the reason an adapter
  can never register a server (exhaustive over `InstructionFile`, no `_` arm, `None` exactly when
  `supports_mcp()`), `status` prints it under the harness line in the same style as `gate_gap`, and
  an adapter whose site is known but unregistered prints the command that fixes it
  (`run: estigia setup <agent>`) instead. `setup::tools_command` now reads OpenCode's array
  `command` (`[exe, "mcp"]`) rather than only a string, so a healthy OpenCode install does not read
  as an absent entry. The field is on `status --json`'s rows, and
  `the_tools_gap_and_the_tools_never_disagree` crosses the new method against `supports_mcp()` the
  way `the_gate_gap_and_the_gate_never_disagree` crosses the gate half. `README.md` and
  `docs/lifecycle.md` now say the tools half can be permanently unavailable.

- **A substitution is named wherever it moved a root, `paths::covers` compares both sides in one
  vocabulary, and `DSH_HOME` is read.** `sync` with no adapter named built its target list from `is_present`, which
  reads the *substituted* root, so the adapter the substitution moved was the one it could not find
  — and the note that would have explained the omission was attached only to an adapter that was a
  target, so it was lost with it; with no other adapter installed the run refused with `no agent has
  Estigia installed` and said nothing about the root it moved. The substitution notes are now read
  independently of any target (`setup::substitution_notes`), printed beside the run for the notes no
  target already reports, and carried by that refusal when the substitution is why the list came
  back empty — byte-identical when no substitution fired. `paths::covers` / `coverage_depth`
  canonicalised both sides, falling back to the literal spelling for a path that does not exist, so
  a candidate under a symlinked home that nothing had created was read as outside it and dropped, or
  a note named the same path on both sides of the substitution sentence; both sides now resolve
  through their nearest existing ancestor, re-appending the tail. And `DSH_HOME`, which DeepSeek
  Harness resolves both of its roots under, is now read and contained the way `XDG_CONFIG_HOME` is:
  an absolute value under the same call's home is honoured for `~/.dsh/skills` and `~/.dsh/AGENTS.md`,
  one outside it is dropped in favour of `~/.dsh` and reported as the per-adapter note.
  `docs/honesty.md` and `docs/what-it-writes.md` move with all three.

- **OpenCode receives the two delegated workers, in OpenCode's own dialect.** `Delegated
  workers` was `Applies::Inert` on OpenCode — `agent_definitions_root` answered `None`, so the TUI
  rendered it `(no effect)` and refused to edit it — and the absence was deliberate, but only
  about reusing Claude Code's renderer rather than having OpenCode's. OpenCode now has a root at
  `$XDG_CONFIG_HOME/opencode/agents`, falling back to `~/.config/opencode/agents`, resolved per
  adapter rather than from the Claude configuration directory the old code hard-coded — a root under
  `~/.claude` is the defect this closes. A renderer emits OpenCode frontmatter: the template's
  `description` verbatim, `mode: subagent`, `model:` only when `Model routing` names one (OpenCode
  inherits otherwise, and Claude's literal `inherit` is not a value it reads), and a `permission:`
  map derived from the same `tools:` line the Claude template carries — `edit` allowed iff that list
  names any write tool, `bash` iff it names `Bash`, fetch and delegate denied — with no `effort:`
  line, because OpenCode has no effort field and an effort named for a worker reaches no file there
  (`docs/honesty.md`). The planning phases and the reserved reviewers stay Claude-only, gated on the
  instruction enum rather than on a root merely existing; uninstall takes the delegated workers back
  for both hosts, and ownership still refuses a file Estigia did not author. `docs/what-it-writes.md`,
  `docs/honesty.md`, `README.md` and `skill/references/runtime-notes.md` move with it.

- **The installed contract's model-routing sentence stops naming the portable routing, and names
  Claude Code instead of "this host".** `Model routing` is `Scope::Agent`, so an adapter's
  own `estigia.<slug>.md` may move the row, while `selected_documents` renders its sentence once into
  a contract every adapter sharing a skill root reads — the sentence cannot follow the override. On a
  host whose effective routing was `unset` (inherit), the contract still ordered the agent to hand
  work to `implementer=sonnet`, a model the host had no provider for, and the run named a substitute.
  The sentence now defers to the row, says what an unnamed key obliges (launch with no model named,
  leaving the default in place), and replaces "where this host reads sub-agent definitions" with
  the host it means — Claude Code, the one adapter receiving the definitions `setup` installs. The
  generated block is held to naming the host it means and never "this host" by
  `a_named_model_routing_reaches_the_agent_that_has_to_honour_it`. Giving
  `estigia.<slug>.md` a prose section was considered and refused: it would leave one sentence
  needing a second place an agent reads guidance from, and the rule here is to remove a copy
  rather than add one.

- **The OpenCode gate loads on OpenCode 2.** `setup opencode` rendered an OpenCode 1
  plugin — a named function export returning a `tool.execute.before` hook — and OpenCode 2 refuses
  any module without a default definition carrying an `id` and a `setup`/`effect`, so after a host
  upgrade the file stayed on disk, `status`/`doctor` went on reporting the OpenCode gate as live
  (`plugin_wire` and `is_gated` read the marker alone), and every write went unadjudicated. The
  rendered module now carries both entrypoints: a default definition whose `setup` registers
  `ctx.tool.hook("execute.before", ...)` for OpenCode 2, whose `server(context)` delegates to the
  implementation OpenCode 1 already loaded — measured on 1.17.13, 1.18.20 and 1.18.31, all of which
  read the default object and ignore the named export — and the named export itself, left in place
  for releases older than those. `shell` joins `GATED` because OpenCode 2 names its shell tool
  `shell` where OpenCode 1 names it `bash`, and `narrowed_by_the_call` now decides the shell road by
  `harness::SHELL_TOOLS` rather than the literal `bash`, so a V2 shell call is read the way every
  other host's is; V2's write tools name their target as `path`, an alias `path_argument` already
  reads. The launch directory on V2 is `location.directory` — measured inside a git worktree, where
  it is the worktree and `project.canonical` is the checkout the worktree came from. Measured on
  2.0.16: the module loads with no `failed to load plugin` warning, a refusing gate blocks the call,
  and an allowing one lets it through.

- **Planning phases persist their own artifacts and return a result contract.** With
  the artifacts on the issue, `spec`, `design` and `tasks` render with the tracker's `comment`
  operation in their tool list (`mcp__estigia__comment`) instead of depending on the orchestrator
  to copy the artifact by hand; with the artifacts under `openspec/` they keep the `Write` they
  already had. Every phase definition now ends with the same result contract — `status`
  (`done`/`blocked`/`partial`), `artifacts`, `risks`, `next` — and `protocols/sdd.md` tells the
  orchestrator to route on `status` rather than reading prose. The contract shape follows
  gentle-ai's (`internal/assets/claude/agents/sdd-spec.md`, MIT, Copyright (c) 2025 Gentleman
  Programming), adapted: no memory-backend tools, and the grant stays short of `Write` where the
  artifact lives on the tracker.

- **`setup` no longer refuses a reviewer definition it wrote itself, and stops needing a hand-frozen
  body per template edit.** Ownership of `~/.claude/agents/review-blind.md` and its six
  sibling reserved reviewers was decided by byte-matching the installed file against a list of every
  body Estigia had ever shipped — `reviewer_is_static` for the current renderings, the
  `REVIEWER_TEMPLATE_BEFORE_*` ledger added for the older ones. That list has to be
  extended by hand *before* a template change ships, and `src/setup/tests.rs` records this
  repository's own decision not to extend it for an in-flight repair round, on the reasoning that
  nobody runs those bytes. `--allow-source-build` is the flag that makes that false: it installs
  whatever the working checkout holds, published or not, so an unpublished body does reach disk and
  the next template edit strands it with `reviewer-definition-changed` and a world-action telling the
  operator to move their own file aside. It recurred four times on one machine.
  `write_reviewer_definition` now records a digest of the bytes it actually leaves at that exact path
  — `record::note_written_outside`, sharing the map `note_written` already keeps for the skill root —
  and `reviewer_is_ours` accepts a file matching it. An absent digest is not read as a match: a
  record written before this existed falls straight through to the ledger unchanged, which is why the
  ledger stays rather than being retired. The digest arm lives in `reviewer_is_ours` itself, so the
  setup preflight and `uninstall`'s delete-or-keep choice cannot diverge — one door accepting a body
  the other keeps as foreign would orphan it at the reserved path for the next install to refuse as
  unowned. Separately, `historical_reviewer_renderings` rendered every frozen body under
  `Seal::Unsealed` and `review-blind` alone, so a `review-blind-sealed.md` from any earlier binary
  matched nothing and could never be upgraded; it now renders each frozen template across every
  `(name, seal)` pair `RESERVED_REVIEWERS` carries, a strict superset of what it produced before.
  `reviewer_is_static_for_seal`, which gates a *running* judge, is unchanged: a body accepted for its
  digest alone is not a current rendering and launch still requires one.

- **DeepSeek Harness joins the adapter table.** `estigia setup deepseek-harness` installs the skill
  into `~/.dsh/skills` — the harness's own `user-dsh` skill root, which its `skill-filesystem`
  provider scans as its `user-dsh` root — and fences the directive into `~/.dsh/AGENTS.md`, the
  harness's fixed user-global instruction file that every `agent-instructions` baseline starts from.
  Both are taken from the harness's own shipping source, so this is the third adapter (after Claude
  Code and Codex) that discovers the skill without the directive naming a path. Deliberately not
  written: no MCP registration (the harness runs its own MCP-client machinery and the registration
  site is unverified), and no tool-call gate (whether a harness plugin can deny a tool call, and
  under what payload, has not been crossed against a shipped harness). The adapter is contract-only;
  the decisions are recorded in `docs/honesty.md`.
- **ZCode joins the adapter table.** `estigia setup zcode` installs the shared skill and fences the
  directive into `~/.zcode/AGENTS.md`, ZCode's user-scope instruction file, taken from its own shipped
  configuration reference, and registers Estigia's MCP server into the nested `mcp.servers` key of
  `~/.zcode/cli/config.json` — crossed against a running ZCode CLI 0.16.5 and its own connection log
  rather than read on a page, which is why it is written here where DeepSeek Harness's own MCP site,
  above, still is not. What that reference also names and this change deliberately does not write: no
  skill root of its own (ZCode sits in the neutral tier until `~/.zcode/skills` is read out of ZCode's
  own shipping source), and no tool-call gate (its hooks decide nothing unless `hooks.enabled` is
  true, and neither the payload nor the deny semantics has been verified). The gate and skill-root
  decisions are recorded in `docs/honesty.md`; the adapter participates in every derived crossing the
  others do.
- **An inherited `APPDATA` or `XDG_CONFIG_HOME` that points outside the resolved home is no longer
  used.** No CLI flag ever sets `SetupOptions.home_dir`, so `estigia setup`/`estigia sync` always
  took the road where `home` came from `HOME`/`USERPROFILE` and these two roots came from the process
  environment independently of it — and on Windows `APPDATA` is always set. An isolated `HOME` moved
  the skill and every other root to the sandbox and left Gemini CLI's instruction file, hooks and MCP
  configuration resolving against the real machine; measured live, a reviewing run's redirected
  `estigia sync --allow-source-build` rewrote an operator's real `%APPDATA%\gemini\GEMINI.md` and
  `settings.json` from inside a checkout that then got deleted. `Environment::resolve` now takes an
  inherited value only when it resolves under that same call's `home`, through `paths::covers` rather
  than a hand-rolled prefix check, and falls back to the home-relative path — the same fallback it
  already used for an absent variable — when it does not. An explicit `SetupOptions.app_data` /
  `config_home` is unaffected; only what came from the process for free is held to this rule. The
  substitution is reported as a note in the per-adapter output rather than refused, because a process
  reading `APPDATA` cannot tell a relocated `AppData` apart from a redirected `HOME` that forgot to
  move it. It survives a failure raised after the resolution that produced it, so a run that stops
  before changing any files still says which root it substituted rather than reporting the stop
  alone. It does not reach a `sync` that names no adapter, which looks for installs under the
  substituted root and so can stop finding the very adapter the substitution moved; `docs/honesty.md`
  holds that gap and the canonicalisation asymmetry beside it. `docs/what-it-writes.md` now names every adapter's root and the variable that
  governs it.
- The setup screen opens on a source build, and the refusal arrives when an install is confirmed from
  it. `estigia tui` — a visible alias of `setup`, so the same verb — answered
  `source-build-not-allowed` and drew nothing, on the one command somebody who has just built the
  binary types before they know any agent slug. The way out its own resolution named was `estigia
  setup --interactive --allow-source-build`: the same screen, one flag later. The door therefore
  withheld nothing the flag would not grant, and took the read-only rows with it. The gate is the
  screen's install now, where a refusal is shown in the box and still leaves as the process's exit
  code. Non-interactive `setup` and `sync` keep refusing before they start, because they write
  immediately. There is one preflight and not three, so `recorded-downgrade-blocked` and
  `lifecycle-state-unreadable` moved with it: on the guided route those also arrive at the save
  rather than before the screen. They still refuse on every route. The screen was never read-only
  before an install either — its guard, language and folder keys write — and `docs/lifecycle.md` now
  says so rather than leaving the sentence to imply otherwise.
- Added `scripts/build-install.ps1` for a complete Windows source reinstall. It installs Rust 1.97 when
  needed, compiles and force-installs the locked release build, runs all-agent setup with explicit
  source-build consent, and verifies the result with lifecycle status, installation status, and
  doctor.
- Added `scripts/build-install.sh`, the same complete source reinstall for macOS and Linux. It shares
  no logic with the PowerShell script — only the outcome: require `rustup` and `cargo` before touching
  the build, install Rust 1.97 when needed, compile and force-install the locked release build through
  `cargo`, then run the same five steps against the installed binary — `--version`, `update`, `setup
  --all --allow-source-build`, `status`, `doctor`. It resolves the install root the way Cargo does — `CARGO_INSTALL_ROOT`, then `CARGO_HOME`, then
  `$HOME/.cargo` — and pins it with `cargo install --root`, so the path it checks is by construction
  the path Cargo wrote; a machine that sets Cargo's `install.root` should set `CARGO_INSTALL_ROOT`
  for it.
- Added the first secure binary lifecycle slice. `estigia update` reports read-only text or global
  `--json` inventory for bytes observed by reopening the `current_exe` pathname, installer-recorded or source/unrecorded
  provenance, relation to recorded machine high-water, and an honest unavailable public-release
  result without making a network request. Create-once records under `~/.estigia/lifecycle` bind the
  path-observation lookup key to the exact compiled package version and typed/count-framed embedded-asset
  digest; publishers derive those values, record reads use one inspected handle, reject final
  symlinks/reparse points, non-regular files and unknown fields, and malformed evidence fails closed.
  Publication uses unique create-new same-directory stages, sync, and create-once hard links.
  The corrected framing/field model is schema 3; schema-2 records fail closed as unsupported.
  These records are not authenticated against a malicious same-user writer, and preflight is not
  atomic with later setup/sync mutation. Source builds require
  per-command `--allow-source-build`, recorded downgrades remain blocked with it, and dry-run bypasses
  lifecycle inspection because it deploys no assets. The pathname digest is raceable inventory, not
  mapped-code or exact-executing-byte authentication.
- Official release installers now extract the verified candidate, invoke its hidden argument-free
  `__record-install` command, explicitly require success, and only then replace the destination.
  Candidate-derived orchestration reads high-water before publication, refuses typed downgrade,
  hashes its own pathname bytes, and publishes immutable provenance before release. Exact replay is
  idempotent; malformed/non-regular state and provenance conflicts fail closed without advancing
  release history. Attestation remains conditional. Publication then copy is not atomic, concurrent
  installers remain unserialized, and no public Release, remote update, self-upgrade, or rollback exists.

- `estigia setup` installs the skill, a short directive in the agent's
  always-loaded instruction file, the lifecycle hooks, and the MCP server
  registration for thirteen adapters. `setup --all` continues after one adapter
  refuses, reports any actions already proved for that adapter, and still
  configures the unaffected adapters. One unconfirmed adapter keeps the batch
  `Unknown` even when later adapters complete.
- **Uninstall is the exact inverse.** It removes what Estigia wrote and nothing
  else, and a file that held nothing but Estigia's block does not survive as an
  empty husk. JSON keeps the operator's key order; markdown comes back byte for
  byte.
- Twenty typed settings. Reading the table produces a valid configuration or a
  refusal that names what may be written instead.
- **The installed skill directory is `flow`.** It was `issue-flow`, upstream's
  name, kept so the two could be the same directory. Estigia now installs under
  its own, and the roles it answers to are `flow triage` and `flow dev` — the
  long forms `analyst` and `developer` still work. The gate's control surface
  derives that directory from `skill::DIRECTORY` instead of naming it a second
  time, which is what stopped the rename from leaving the contract writable on a
  routine answer.
- **Invoking `flow` with no role routes itself** from tracker state: this run's
  held issue resumes as dev, else an unassigned `review` then `ready`, else
  triage. A routing read that *fails* is not an empty queue and does not fall
  through to triage — triage writes, so a failed read would become a filed issue
  justified by a queue nobody saw.
- **`sdd` says the phases are *available*, not that they all run.** The screen
  said "five phases before code", flat, while `protocols/sdd.md` has always
  engaged them per change on ambiguity and skipped what an already specified
  issue answered. An operator reading the row was promised four artifacts on a
  one-line fix, which is the reading that makes a method feel like theatre and
  gets it switched off. The behaviour did not change; the sentences describing
  it did, in the screen and in the protocol. `auto` and `per-issue` are accepted
  as spellings of `sdd`, because somebody reaching for that word is asking for
  what `sdd` already does.
- **Direct work now has bounded delegation rules in the always-loaded
  contract.** `SKILL.md` owns the read and write thresholds, smallest-topology
  rule, single-writer invariant, and per-action worker exception. Crossing a
  threshold delegates direct work only: it never changes the configured
  `Planning` mode, creates SDD state or artifacts, or invokes an SDD phase. The
  harness does not claim to dispatch those workers; a scoped semantic test holds
  the contract instead.
- **Planning now owns an inline, agent-specific model section in the TUI.** The
  persisted `Model routing` row remains one `key=model` cell for config and CLI
  compatibility, but it is no longer a visible setting or two-stage target/raw
  picker. `Planning` is the last primary row. Beneath it, exact model rows are
  derived from `orchestrate`, the active planning phases, universal `apply`,
  delegated roles and external sub-agent names; inactive phases stay persisted
  but hidden, and the workflow states have since stopped being keys at all. Each row opens advisory models for the concrete host:
  curated lists for Claude Code and Codex, and a lazy `opencode models` read
  without `--refresh` for OpenCode. That read closes stdin and returns the TUI
  within 5.6 seconds through one deadline covering PATH resolution, validation,
  spawn, execution, cleanup and handoff; stdout above 1 MiB, outside strict UTF-8
  or without a valid ID is unavailable rather than parsed. Unix process groups
  and Windows Job Objects clean descendants after successful direct completion
  as well as failures, while Windows script launchers are refused. Other,
  neutral, uniform and shared answers invent no catalog; custom IDs and
  target-local `inherit` remain available. Dynamic suggestions and custom IDs may
  not contain the comma, pipe or line break that would end one `key=model` entry.
  One-target updates preserve the rest of each destination's cell. Assignment,
  inherit and target restore share one destination rule: the current agent in
  non-uniform mode, or only the currently selected agents in uniform mode.
  Restore returns each selected target to that destination's own installed
  assignment. Uniform display aggregates per target, and suppresses planning
  phases when selected agents disagree on Planning. All destinations validate
  before one atomic commit, model selection never changes `Planning`, and Enter
  and Space both choose while editor Space remains literal.
  Claude Code and Codex now add a synthetic profile row with independently
  reviewed `balanced`, `performance` and `economy` routes. A preset replaces the
  complete route; an exact mismatch is `custom`, and choosing `custom` preserves
  the target assignments edited below it. Profiles compile into the existing
  `key=model` cell and never change Planning. Dynamic OpenCode, shared/uniform
  views and adapters without stable catalogs invent no presets.
  Uniform `Planning` now uses the same selected-agent destination set rather
  than mutating every loaded table. `ConfigLayers` now preserves contract-only,
  per-agent, local and repository snapshots plus each override document's
  explicit row ownership. Agent documents parse and own only agent-scoped rows
  and repository documents only repository-scoped rows, so forbidden hand-edited
  rows are ignored before validation and cannot reject or pin lower-layer values;
  invalid owned rows still refuse. Each shared skill root renders one portable contract;
  selected agent rows land only in that adapter's override, while private roots
  keep them in their contract without a duplicate. Local, repository and
  unselected peer values are never promoted into shared `SKILL.md`; host phase
  definitions are instead materialized from the effective view and retracted
  with it. Repository saves preserve existing owned rows and union only rows
  changed in the session. Direct agent writers use one fail-closed document snapshot
  for ownership and output. Direct repository writers likewise treat only `NotFound`
  as creation; unreadable or invalid UTF-8 input remains byte-identical on both paths.
  Receipts carry every layer plus setting-level effective
  read-back, exact repository-owned settings and explicit lifecycle completion.
  One repository snapshot binds values and ownership across all loaded baselines;
  only owned rows advance, including after a later post-write failure. A proved
  repository document missing or unreadable at read-back is `Unknown`/status-required,
  with no row acknowledgement. Partial failures retain proved
  actions as `Committed`, or become `Unknown` when the attempted write itself
  cannot be confirmed, without marking an incomplete adapter installed; the TUI
  leaves unproven scopes dirty. The first `Unknown` refusal controls aggregate identity and remedy.
  `SetupFailure` names prevalidation, preview or mutation explicitly; any generic
  failure before mutation is `NotStarted` with an input/configuration remedy,
  while typed malformed JSON remains `NotEditable`. Failed real prevalidation
  carries no dry-run actions or wording. Preview failures remain `NotStarted`
  regardless of their planned actions. Planning-then-model and model-then-Planning still
  preserve both edits.
  Interactive dry-run uses the real batch's one pending action accumulator,
  including auxiliary files, so unique paths/change kinds/counts match while it
  writes and acknowledges nothing. Planned actions are never treated as mutation
  evidence; successful retry still replaces its refusal.
  Model rows, thumb and picker now share one focus-aware viewport; Tab resets
  both rows and thumb, picker opening restores row ownership, bottom targets stay
  attached at 80x24, and Space on the agent choice remains a toggle.
  Current and installed assignments share one bilingual renderer, and Help now
  has one table-backed translation whose English and Spanish pages both describe
  the inline selector and distinguish localized display labels from canonical
  persisted names, values, model IDs and printed CLI commands.

### Refusals

- **A near-miss integration names the shape, not a delivery reason that is false for it.** A `git merge` in the run's own checkout whose shape the integration exemption does not prove
  — a pipe, a redirect, a second command, a quoted ref — fell through to `out-of-phase` in
  `in-progress`, or to `verdict-bound-to-other-bytes` when the checkout head had moved, and neither
  said the shape was the problem; a run measured on 2026-10-06 read both as "the merge is forbidden"
  and rebased four times, paying a whole-change re-review each time. `in-progress` now refuses
  `integration-shape-not-exact`, naming the one accepted literal (`git -C <worktree> merge --no-edit
  origin/<base>`, nothing beside it). It is a refusal and never an allow, it fires only in
  `in-progress`, and `out_of_phase` is unchanged for `analysis`, `ready` and `blocked`.
  `publication-base-not-ancestor`'s `action` and the `skill/bindings/github.md` row now name the same
  literal and the `in-progress` requirement.
- **The caller's own fields are bounded by the comment they land in, before any remote write.**
  `run-id`, `topology` and `topology_reason` are caller-supplied and reach the published
  comment — `run-id` on the marker, the topology on the marker and in the prose — so any could grow
  it past its 65,536-character cap after the branch was pushed. `publication-comment-too-large` now
  refuses them before the fetch, against the cap less an enumerated fixed overhead, the
  genesis-manifest ceiling and the publication lane's own measured width.
- **The blank-topology-reason refusal names the topology it is refusing a reason for.** The `blank-topology-reason` action assumed `topology: inline` and mentioned "past the threshold",
  which is wrong for any other declared topology and for none. It now names the declared value, or
  says no topology was declared when there is none, and only an `inline` declaration is told to say
  why it needed no delegation — a delegated topology needs no reason at all.
- **The base-advance refusal names both recoveries, and the operation that clears each.** `publication-base-not-ancestor` told a run to "rebase ... and publish again", but a
  rebase rewrites the parent head and the ordinary push is then a non-fast-forward, so on any
  publication after the first the named recovery landed on a second refusal and `republish_review` —
  the leased push that clears it — went unnamed. The `action` and the `skill/bindings/github.md` row
  now name both recoveries with their consequences: a merge keeps the parent head, so a re-review
  reads only the delta; a rebase rewrites it and is published with `republish_review` when a
  publication already exists, or with `publish_review` on a first publication, where
  `republish_review` would refuse `published-transaction-missing`. No gate or condition changed. An
  enumeration found no other refusal naming a recovery that lands on a second refusal for this
  reason — the two others that name a rebase (`publication-unconfirmed`,
  `publication-readback-disagrees`) already name `republish_review`.
- **A `published` marker whose fields are present but unreadable is refused by name rather than read
  as an absent one.** `ReviewReceipt::from_marker` and `CorrectionState::from_marker`
  both answered `Option`, so a corrupt publication was dropped from the lineage by `all_publications`
  and a half-written correction budget reset the lineage's cumulative to zero in silence. Both readers
  now answer absent, present-and-valid, or present-but-unreadable, the third refused as
  `published-transaction-unparseable` or `correction-state-unparseable`, naming the marker's `epoch` and
  the first offending `field`, before anything remote moves. A marker carrying *none* of the budget
  fields is the pre-budget migration path and is still admitted. Measured against this repository's
  own timeline: the oldest surviving `published` marker (2026-08-23) is complete and none
  of the 649 recorded is malformed, so no legitimate marker is turned unreadable.
- **A `delivery-pr-unidentified` refusal says which byte disqualified the merge
  command, and the number must precede the flags.** `pr_merge_target` retains
  one positive numeric PR only from a literal `gh pr merge <number> ...`
  command; the byte filter is right and its reason is recorded, but the refusal
  claimed the command "does not name one positive numeric pull request" — a
  sentence that is false of `gh pr merge 136 --merge 2>&1`, which names PR 136
  perfectly well and was refused for the redirect. A run measuring that defect
  spent two hours reading the classifier, because the message pointed it at the
  one thing that was true of the command. The refusal now names the
  disqualifying byte, and the resolution states the shape that would clear it —
  the number in the fourth position, before every flag, spelled without leading
  zeros. `gh pr merge 007` no longer retains the identity `7`: the spelling is
  taken literally or left unidentified, never interpreted, because a retained
  number selects which review transaction may authorise the merge.
- **The other three refusals a delivery `gh pr merge` can answer with, and the pointer field behind
  them.** `delivery-pr-unidentified` was documented and its siblings were not, though an operator sees
  them in a refusal and searches for the code. Alongside it:
  `complete-review-transaction-missing` — the pointer carries only a legacy reviewed head, which names no
  PR, epoch, base or digest, so it cannot select a lineage;
  `delivery-pr-mismatch` — the merge targets one PR and this run's receipt is for another, each number
  named in the message; and
  `complete-review-transaction-not-selected` — the linked sibling checkout holds no unique complete
  receipt for the PR the command names, by ambiguity among readable pointers; an unreadable pointer
  is `run-pointers-unreadable`. The field
  they all read is the persisted complete review receipt (epoch, PR, head, base and digest); a
  `reviewed_head` pointer written before it remains readable after upgrade but cannot qualify a
  PR-targeted merge, which is what `complete-review-transaction-missing` says when it is all the pointer
  has.
- Every refusal carries a stable code, what happened to the world, whether a
  replay is safe, and a resolution — either a runnable invocation checked
  against the real parser, or a reason from a closed vocabulary saying why no
  command can honestly exist.
- The transport's **69 refusal reasons** are derived from the transport itself
  at build time, so the two cannot drift.

### Guards

- **The four `judge-workspaces` input refusals are held by tests.** `judge_workspaces`
  declares four refusals on caller input — an absent or `unset` `Worktree location`, a relative
  `--worktree-root`, a `--count` of zero and a count above `MAX_JUDGES` — and none was exercised, so
  deleting or inverting any guard left the suite green. Four tests now hold them, each asserting the
  exact `reason` and the fields that distinguish it: `malformed-argument` with `argument: --count` and
  `value: "0"`; `judge-workspaces-count-too-large` with `requested: 65` and `max: 64`;
  `no-worktree-location` for both the empty and the `unset` template; and
  `worktree-location-not-absolute` through `validate_worktree_override`. The two count refusals also
  assert nothing was created under the resolved panel root. Each reddens when its own guard is
  removed or inverted — demonstrated by running all four mutations, each failing the matching test.
  Production behaviour is unchanged: no refusal reason, wording or envelope moved.

- **An integration line's clearance is pinned by its consequence, and the criterion that explained
  the `DELIVERS` exclusion now matches the list it explains.** `docs/honesty.md` and `CHANGELOG.md` justified keeping `git pull` out of `DELIVERS` with
  "an integration lands nothing on a shared ref", which separates nothing: `git merge` is in
  `DELIVERS` and also lands nothing on a shared ref, while `git push` lands on one and is excluded.
  Both now use the wording `tests/guards.rs` already carried — an integration step rather than a
  landing one, matching `DELIVERS_POPULATION`'s own "spellings that end a delivery rather than open
  one". `README.md`'s boundary sentence now names all eight members of `IRREVERSIBLE` rather than five.
  And a new test pins the documented consequence rather than list membership: widening
  `stale_verdict`'s guard from `DELIVERS` to `IRREVERSIBLE` reddens
  `an_integration_clears_stale_verdict_rather_than_being_refused_as_a_delivery`, demonstrated by
  running that mutation. The classification itself is unchanged.

- **The envelope fallback's evidence delivery is held by acceptance tests.** `translate`
  repeated a refusal envelope's scalar fields in the fallback message (`envelope_detail`),
  but the four criteria that issue set were never tested — the last-resort string was written and
  unobserved, and no test drove a `Failure::Stop` payload carrying the `review-target-mismatch` lists.
  Two tests now hold it: `a_stop_that_names_evidence_and_no_detail_hands_the_lists_to_the_agent`
  drives the real payload shape through `translate` and asserts `unreviewed`, `not_delivered` and
  `differing` reach the refusal's own text while `code`, `outcome`, `replay` and `resolution` render
  unchanged, and `a_stop_with_nothing_scalar_falls_back_to_the_context_and_the_code` pins the last
  resort. The first reddens with `expected_target: the transport exited 1` when the fallback is
  reverted to its earlier form.

- **The isolated-payload crosses build their comparison side fenced, not from the machine.** `review_payload_bounded` builds its diff in an isolated `GIT_DIR` with the child's
  environment replaced, and the tests that cross that payload against a diff they build themselves
  built the comparison ambient. Before isolation both sides were ambient, so the cross was
  insensitive to the machine; after it, an attributes file marking every path `-diff` — through a
  global `core.attributesFile`, or through the default global attributes file that is resolved from
  `HOME` and `XDG_CONFIG_HOME` — collapsed the ambient side alone and reddened the cross while the
  operation behaved exactly as designed. The comparison sides now run through `fenced_git_env`, which
  nulls the system and global config layers, ignores the system attribute file and pins
  `core.attributesFile` to the null device; `isolated_diff_env` is refactored to build on the same
  fence, so it is defined once, and the equality is a property of the code rather than of the
  developer's own configuration. A new test,
  `the_ambient_fence_ignores_every_attribute_source_outside_the_repository`, drives both sources: the
  unfenced diff collapses and the fenced one stays byte-identical to the isolated payload.
  `review_payload_bounded`'s own behaviour is unchanged.

- **The document byte registry checks drift, not only growth.** `DOCUMENTS` in
  `tests/instruction_ceiling.rs` was checked in one direction. `documents_stay_under_their_registered_byte_ceilings`
  refused a file above its entry, but an entry left *above* its file passed silently and handed back
  that much growth with no review-visible edit — exactly the gap the registry exists to close, and the
  opposite half of what the neighbouring `LINE_EXCEPTIONS` check already did. A new test,
  `every_registered_byte_ceiling_is_the_measured_value`, asserts each entry equals its file's
  LF-normalised byte length and names the entry and both numbers on refusal; the one-sided check
  stays. Six entries had drifted above their files and were corrected to their measured values in the
  same change: `skill/agents/developer.md` (2,859→2,855), `skill/agents/sdd-spec.md` (2,130→2,128),
  `skill/bindings/github/delivery.md` (34,494→34,435), `skill/bindings/github/setup.md` (1,671→1,611),
  `skill/protocols/sdd.md` (8,697→8,692) and `skill/references/safety-incidents.md` (11,709→11,647).

- **Three checks in the reviewer-vocabulary crossing asserted less than the prose beside them
  claimed.** The crossing asserted each accepted vocabulary value against a reviewer's
  own template bytes and then again against its rendered body. `render_reviewer_agent` only
  substitutes `{{TOOLS}}`, `{{DISCIPLINE}}` and the name line, none of which any value is a substring
  of, so the rendered assertion could never fail where the template one passed — it is removed, and
  the crossing is the template-level check it always was; the fields `record_review_finding` reads
  are still asserted on the rendered body. The evidence grants were retyped by hand beside the
  `Evidence::all()` four other crossings derive from, so a third grant would have been rendered by
  `render_reviewer_agent` and left uncovered; they now come from `Evidence::all()`, walked once per
  grant for the unsealed row whose rendering reads the grant and once for the six sealed rows whose
  rendering ignores it. And the list's length was pinned at four, which catches a deleted entry but
  not one const substituted for another — the four are now asserted pairwise distinct as well.
  `docs/honesty-review-tier.md` is corrected clause by clause. Mutations run: substituting one const
  for another reddens naming both, and removing a field name from one template reddens naming the
  template.
- **The base-ancestry refusal's placement above the payload materialisation is held by a test.**
  A comment, the refusal table row and a `docs/honesty.md` paragraph called the order
  load-bearing, but moving the `is_ancestor` block below `review_payload` left the whole suite green —
  an oversize corrupted diff would be measured against `review-payload-too-large` before being
  refused. `the_base_ancestry_refusal_precedes_the_payload_materialisation`
  (`src/transport/target/tests.rs`) binds the order through `source_text::code_of`, so the comment
  above the call cannot move the boundary, and the mutation was run: moving the block below
  `review_payload` reddens it with the ordering named.
- **The control-surface registry now holds the matcher, not only the list.** The
  `control-surface` declaration fingerprints `CONTROL_SURFACE`, a list, so a change to the code that
  decides against it never reopened the claim. Three such changes rode through one pull request with
  the registry green — honouring a trailing `/` as a directory, anchoring fragments that begin with
  `.`, and anchoring those that end in `/` — and two of them were narrowings, the direction the
  registry exists to catch. What caught them was a blind reviewer measuring paths through the built
  binary. `CONTROL_SURFACE_CORPUS` is now a second declaration's subject: a fixed corpus of paths
  with the answer both roads must give each one, asserted by
  `the_control_surface_matcher_answers_its_corpus`. A matcher edit that moves an answer reddens that
  test; recording the new answer moves the fingerprint and reopens the claim; a refactor leaving every
  answer alone moves neither. The corpus is representative rather than exhaustive, and which seams it
  covers is in its own declaration. `docs/honesty.md`'s declaration entry carries the account, and
  `tests/guards.rs`'s module documentation no longer says the matcher is outside the registry.

- **The symbols `skill/` names are crossed against the code that owns them, for three of the five
  classes it enumerates.** `skill/` is instruction an agent installs and obeys, not
  documentation about the product, and nothing checked that the symbols it names exist. Three
  crossings now do, and the corpus behind each is **extracted from the Markdown on every run rather
  than enumerated in a list somebody must remember to extend** — the property whose absence cost
  four recurrences before it was undone. `crate::markdown::markdown_inline_code_spans`
  reads every inline code span, skipping fenced blocks entirely, so a prose mention never enters the
  corpus and a symbol inside a fence never does either.
  **Refusal codes** gain the direction nothing crossed before: the existing test asks whether every
  code the transport can answer with is named in `skill/`; the new one asks whether every code
  `skill/` names is one the crate can answer with, which is where a drift becomes an instruction
  branch no run can reach. Of the 184 lowercase-kebab spans the walk reads, 117 resolve as literals
  in `TRANSPORT_VOCABULARY` and one — `closed-by-own-delivery` — only as a `format!` prefix, so a
  literal-equality check would have reported a real code missing. Four other symbol families share
  that exact shape and are subtracted by extracting each from its own source rather than guessed
  away: workflow states from `config::STATES`, setting values from `Setting::answers()`, agent names
  from the shipped `skill/agents` listing, and operation verbs from the transport's own
  `transport::commands::SCRIPTED` table. What survives all of that is a small table carrying its
  reason as data — a fictional run-id, a regex fragment, a git subcommand named as an example — with
  a staleness assertion in both directions, so an entry that stops appearing has to come off, and an
  entry the transport now answers with is a real code the exemption was hiding.
  **Workflow states** are crossed both ways, and the reverse direction stops being a six-item literal
  retyped beside the sentence it claimed to check. **Configuration rows** are crossed against
  `SETTINGS`, which immediately caught a live defect: the shipped config block advertised
  `"Description for dumb humans" sentence language`, a deprecated alias, where the canonical label is
  `Summary language`.
  Two classes are deliberately not taken and `docs/honesty.md` says why. **Tool names** look like the
  easy one and are the trap — the family is spelled two ways: most of it is backticked `snake_case`,
  which the kebab filter never admits at all, and 84 occurrences of foreign host tool names Estigia
  does not own sit in the corpus beside them.
  **Command verbs** are a population of one distinct verb — `estigia setup`, two occurrences across
  two files — which is not a population. The check is also scoped to the transport's vocabulary: the
  roughly 28 codes `src/harness/**` answers with are not in `TRANSPORT_VOCABULARY`, none is named in
  `skill/` today, and `docs/honesty.md` records that a future one would be reported unknown rather
  than found.
  Beside it, `is_static_mdx_document` and `strip_markdown_code_spans` stop carrying their own
  Markdown walkers: both now delegate to one segment tokenizer, removing the copy rather than
  gaining a second.

- Neither workflow may check out with a deprecated action or throw away a red run's cache, and both
  halves are crossed rather than trusted. `actions/checkout@v4` targets Node 20, and it was one of
  five actions that did — a warning that becomes a workflow which will not start, on GitHub's
  schedule rather than on ours. **Only the checkout is raised here**; `setup-node@v4`,
  `upload-artifact@v4`, `download-artifact@v4` and `softprops/action-gh-release@v2` are still on
  Node 20, so the runs still print the warning and the release lane — which carries **all four**,
  three of them only there — still would not start. Every `uses:` line in both files is enumerated
  with its runtime in `docs/honesty.md`, along with two more `node20` actions reached transitively
  through `attest-build-provenance`, rather than the gap being closed under an issue that asked about
  the checkout. Both files move to `v7`, which runs on `node24`. That crosses three majors and each
  carries something: `v5` requires a runner of at least `v2.327.1`, `v6` moves persisted credentials
  out of `.git/config` into a file under `$RUNNER_TEMP`, and `v7` refuses fork checkouts under
  `pull_request_target` and `workflow_run`. The `ref` and `persist-credentials` **inputs** are
  unchanged, which is not the same as their behaviour being unchanged: `ci.yml` opts out of
  persistence and `release.yml` does not, so the release lane takes `v6`'s new location — it runs no
  authenticated git command and no container job, so nothing there reads it. The lanes run on hosted
  runners, and this repository triggers on neither event `v7` blocks. `Swatinem/rust-cache` discards a failing run's cache by default, so the fix pushed after a
  red build recompiled the dependency tree from cold on all three platforms — closing one set of
  platform failures paid it at least nine times, `ci` on `main` having failed nine consecutive times
  before it went green. The issue that raised this said six and named a first run that no longer
  exists; nine is what the run history still holds. The guard names a version floor rather than the
  word *deprecated*, because a file cannot be asked whether it is: `v2` to `v4` run on Node 12, 16
  and 20, `v1` is a runner plugin with no Node runtime at all, and everything from `v5` is `node24`.
  A future deprecation moves that number to somewhere a person has to look at it.
- The shipped payload passes **upstream's own 328 checks**.
- Population declarations are bound to their syntax node and fingerprinted:
  changing the rule or the code beneath it reopens the claim.
- Seam guards hold the contract, the bindings, the transport and the tool table
  against each other in both directions.

- **Model rows now say exactly what the active protocol can route.** Direct has
  no planning phases, full SDD exposes five and lite exposes spec/tasks. Hidden
  assignments remain readable and CLI-editable, so changing Planning never makes
  an existing table invalid. A uniform Planning disagreement exposes only fixed
  rows instead of presenting one selected agent's phases as shared truth.

- **A delegated worker ships for the work no planning phase describes, and a
  route now carries the effort it runs at.** `Planning: direct` runs no phase and
  so wrote no definition at all — which is exactly the arrangement where a
  developer hands a bounded piece of an understood change to a fresh context, and
  composed that brief in prose at every launch. Two definitions close it:
  `implementer` (`Read, Grep, Glob, Write, Edit, Bash`) and `analyst`
  (`Read, Grep, Glob`), two because the grants genuinely differ — this contract's
  analyst is repository-read-only and its implementer runs the suite.
  - **A new row is the switch, not `Planning` and not `Model routing`.**
    `Delegated workers` takes `none` (the default), `implementer`, `analyst`, or
    both, and only what it names is written — under any protocol, retracted when
    the name goes away. A definition carries a tool allowlist, which the
    `control-surface` population already treats as authority, and an upgrade may
    not hand out authority nobody asked for; `none` is what every contract
    written before this reads.
    - It is deliberately **not** the presence of a `Model routing` key. Two blind
      judges refused that shape and were right: `implementer` has been an
      accepted routing key since that row existed, offered as its first example,
      documented as inert, and set by all six shipped model presets — so the
      population holding it is every installation that ever chose a profile, and
      treating that as consent would have installed a `Write`/`Edit`/`Bash`
      definition into their home because they upgraded. A rule invented in one
      change cannot turn a value written under the opposite rule into an answer.
      `Model routing` still says what a named worker runs on.
    - **A definition Estigia did not author is refused, not replaced.** The write
      pass used to overwrite whatever was at the path and record no ownership, so
      the uninstall neither restored it nor reported it. It is decided in the
      preflight now, beside the reviewer's, so the ordinary case has nothing
      half-written behind it, and asked again immediately before the write, which
      is the narrower guarantee and the one that catches a file arriving between
      the two reads. The refusal names both ways out. This matters most for `analyst`, a name
      another harness's orchestrator answers to — which is the documented reason
      to name it at all.
  - **Effort travels with the model.** `design=opus/high`, one of `low`,
    `medium`, `high`, `xhigh`, `max`, in the same row rather than a second one,
    because sizing a context to a task is one decision. It is read from the right
    and only when the tail is one of those five words, so a provider-qualified ID
    such as `anthropic/claude-opus-4` stays one model. It renders as an `effort:`
    frontmatter line into the planning phase definitions and the two workers — not
    into `review-blind`, whose `model: inherit` is literal and byte-pinned; an
    unnamed effort writes **no** line, because the absent field is the
    host's own default and writing one down would freeze today's into every
    installation.
  - **The effort is chosen from the same list as the model.** A row that names a
    model offers `effort: low` through `effort: max`, and once one is set, an entry
    handing it back to the host; an inherited row offers none, because an effort is a property of
    the model the target runs on. Choosing a different model keeps the effort
    already named. One list, not a second stage — this row already retired an
    Advanced one, and a value reachable only by typing a slash into the custom
    field is a value an operator cannot discover. Two smaller faults went with it:
    a row showed `opus` where `opus/high` was stored, and a change that moved only
    the effort went unmarked, because both asked for the model half of the route.
  - Estigia still starts nothing. What changed is that `Model routing` is read:
    twice by `setup`, and by a sentence generated into the installed
    contract's managed block, telling an agent which keys are already routed for it and which it
    must act on itself.

- **Claude Code's SDD planning phases ship as sub-agent definitions, and their
  tool lists are enforced.** `setup` writes `sdd-explore`, `sdd-propose`,
  `sdd-spec`, `sdd-design` and `sdd-tasks` into `~/.claude/agents/`, the verified definition
  root `harness::roles::definition_for` searches. OpenCode and other hosts receive
  routing declarations only, because a definition written where the gate cannot
  verify its format and enforcement would overclaim what the configuration does.
  Other orchestration harnesses ship these by the handful; each
  declares a list and relies on the host to honour it. Estigia declares the same
  kind of list and then refuses the call that leaves it.
  - Only the phases the protocol runs: none under `direct`, spec and tasks under
    `sdd lite`.
  - `Model routing`'s phase key becomes the `model:` line, which is the first
    time that setting does anything — it was, in its own words, a declaration
    the agent reads and no dispatch this binary performs.
  - `Planning`'s `openspec` axis decides the tool list: with the artifacts on the
    issue no planning phase may write at all; under `openspec/` the three that
    leave an artifact behind get `Write` and `Edit`, and the two that only think
    do not.
  - The enforced half is the half the `PreToolUse` matcher wakes — `Write`,
    `Edit`, `MultiEdit`, `NotebookEdit`, `Update`, `Bash`. A `Read` or a
    `WebFetch` never reaches the gate, and each shipped definition says so in its
    own body rather than leaving an operator to assume otherwise.

- **A row an agent's own file answers is no longer reported as in force.**
  `config set` read its own write back from the shared table, and what a run
  actually reads is that table with the agent's `estigia.<slug>.md` laid over
  it — ten adapters share the neutral root, so a row answered there is the row
  that arrives. Measured on a real machine: the command answered *Planning is
  now direct*, all three shared tables said `direct`, and `config list` one
  command later said `sdd lite`. It now checks each adapter under that root and
  refuses with `setting-shadowed-by-agent-file`, naming who still reads
  something else and the command that clears it.
- **`config set --repo` and `config set --agent` stopped writing into the
  machine's own profile.** Both resolved the real home whatever they were
  handed: `writable_config`'s per-agent branch built its own `SetupOptions`, and
  `repository_set` called `paths::home_dir()` unconditionally. So the test suite
  recorded into the developer's `~/.estigia/repositories` — 896 entries when it
  was read, every one of them a temporary directory from a test run — and a test
  driving `config set --agent` through a sandbox wrote outside it. Measured
  after: a full `cargo test` now leaves that file's line count unchanged.

### Documentation

- **The GitHub binding loads a quarter of what it did.** `skill/bindings/github.md` fell
  from 113,675 bytes to 27,486: the per-code refusal table moved to
  `skill/bindings/github/refusals.md`, loaded when a refusal is not self-explanatory; the operations
  table's guarantee column moved to `skill/bindings/github/operations-guarantees.md`; and the
  incidents behind several rules moved to `docs/github-binding.md`. Every rule stays where it was — only the rationale and the
  detail the tools already carry moved out — and four operations-table rows that had carried an
  unescaped pipe inside a flag list now escape it, so they render as cells.
- **The reason three sentences give for `world: committed` no longer says the marker is present,
  which is false for one of the refusals they name.** The `adjudicate` doc (`src/transport/claim.rs`), `Answer::already_wrote`'s doc
  (`src/harness/tracker.rs`) and the `resumed_reclaim` retry test's doc comment said the post-write
  refusals carry `world: committed` because the marker is on the timeline;
  `claim-operation-no-longer-current` and `reclaim-operation-no-longer-current` fire exactly when the
  marker left it, and `CHANGELOG.md`'s own entry already described that path as "read visible
  and then left the timeline before adjudication read again". All three now say the write had already
  landed — read visible by the visibility loop or found by the retry's first read — so the flag
  reports the landing, not the marker's survival. No behaviour changed; no refusal changes its world
  value.

- **Three documents that still said a plugin host has no session identity to give are corrected.**
  The `gate` command's `--run-id` doc (`src/cli/args.rs`) and
  `holders_of`'s doc (`src/harness/guard.rs`) both described the optional identity as the road an
  agent with no session id takes; the OpenCode plugin already forwards the host's
  session id as `--session-id` with `--runtime`, so both now say the identity may be derived and only
  a call with none is found by asking which oath covers the checkout. `docs/review-cost.md` said in
  the present tense that the only shipped caller passed none, with a dead `src/setup/plugin.rs:97`
  citation; it now says the adapter passed none at the time and takes the derived-identity road when
  the host supplies one. No code or behaviour changed.

- **A count that was never measured, and the third copy the fix left behind.** `docs/honesty.md` said "four tests drive `review_tier` end to
  end"; the file holds 21 `review_tier` / `review_tier_for_repair` call sites across at least 17
  distinct `#[test]` functions, so the number was invented and understated by more than four times.
  The sentence now names no count, and the parallel `CHANGELOG.md` sentence — which already carried
  none — is corrected where it had become false. The same entry disclosed that production, `classify`
  and the `agree` helper each held their own copy of the `only_passive` / `touches_configuration`
  composition; those three copies are now one `passive_and_configuration` helper, which takes
  `active_content` so the callers that do not vary it pass the empty set explicitly. Deleting the
  helper's body reddens five tests — `a_large_ordinary_documentation_change_is_low_end_to_end` fails
  at `src/transport/tier/tests.rs:628` on `.expect("an answer")`, because the desynced level is
  refused by `explained_matches_level`. The classification is unchanged.

- **The containment entry names the ordering precondition, `--no-textconv`, the safe direction, and a
  fused range for what it is.** The
  paragraph said `first_unchanged_line`'s walk holds because consecutive new-side ranges are pairwise
  separated, but the walk also needs them **ascending** — which git's new-side order supplies and the
  entry now says. The four pinned things that keep a fused range from forming now include
  `--no-textconv` beside the `--no-ext-diff` already cited, because a `gitattributes` textconv driver
  makes `git diff` diff converted text and the `@@` new-side numbers stop naming source lines. The
  paragraph names which of its two failure directions is safe (**too strict**), and a fused range is no
  longer called "the overlap defect" — that named a predicate since removed, where the fused case is a
  defect of the ranges. The three stale code citations beside them are re-derived, and
  `repo_with_head_text`'s doc comment no longer claims every changed-line fixture below while
  `repo_with_one_deleted_line` now uses it instead of copying its setup. No behaviour changed.

- **Two `docs/honesty.md` enumerations that named the wrong call sites are gone.** The `build_info`-regression paragraph listed a guarded early return
  "outside `src/`" that exists only inside it, and the deferred three-state paragraph named
  `gate_age_verdict` and `status --json`'s field as readers of `build_info::dirty()` when the former
  takes a `bool` and the latter reads `VERSION`. Both conclusions stand without the lists, so both
  lists are deleted rather than maintained against the code.

- **Four claims beside the verdict-basis crossing test now agree with the code, or are gone.** The `CHANGELOG.md` crossing-test paragraph claimed
  the test checks the one physical line on "the two of those six where a whole-file `contains` would
  have let the requirement drift to some other paragraph of the same document" — but the policy
  document, `skill/SKILL.md` and the `review_verdict` row are three where a whole-file `contains` could
  drift, and only the latter two are line-selected; the relative clause is cut, and `docs/honesty.md`
  now says plainly that the number of line-selected carriers is prose nothing crosses. `src/setup/tests.rs`
  said "the same three assertions" over four clauses and that `tests/payload.rs` repeats clause
  assertions, where it crosses the whole constant — cut to what the test does. `docs/configuration.md`'s
  row-change paragraph now names the basis paragraph beside the tool grant, because `Evidence standard`
  moves both and the file on disk keeps the old one until `setup` or `sync`. And the `CHANGELOG.md`
  whitespace example is cut: a hand-typed `basis="   "` reads back `Some("\"")`, non-blank, so the
  sentence describing it as `Some("")` was wrong rather than merely stale. No behaviour,
  `VERDICT_BASIS_REQUIREMENT` byte, or crossing assertion changed.

- **The payload-membership row and doc comment state the limit the tool description already did.**
  `skill/bindings/github.md`'s `review_payload_membership` row called
  a passing answer a "genuine subset" and stopped at "every non-empty line must be a line somewhere in
  the payload", and the `///` on `review_payload_membership` said the same and no more. Both now carry
  the limit the MCP tool description states — membership checks presence only, never order,
  multiplicity or which file a line came from, so a candidate assembled entirely from real lines can
  still misrepresent the change and answer `ok: true` — so the three surfaces an agent reads agree.
  The binding's byte figure moved with the edit: its `tests/instruction_ceiling.rs` ceiling and the
  `docs/honesty.md` "Measured at this HEAD" cell both now read 106,632. The membership check itself,
  and what `publish_review` records, are unchanged.

- **The never-ran binding row and the id-collation sentence name the behaviour and the function they
  meant.** `skill/bindings/github.md`'s `blank-never-ran-evidence`
  row still said `--never-ran-evidence-file` is refused only when "empty or whitespace-only", and its
  `review_verdict` row said the reason fires "when blank", so an agent reading the contract believed a
  one-word file clears the lane — the behaviour that has since changed. The row now names filler (`n/a`, `TODO`,
  `placeholder`, and their kin) beside the empty and whitespace-only cases, matching the sibling
  `blank-review-basis` row, and the reason reads "blank or filler". `docs/honesty.md`'s `id`-collation
  sentence named `record_review_finding`, which contains no such grouping; the grouping is `by_id`
  inside `record_refutation` (`src/transport/claim.rs:5479`), and the sentence now names that
  function. The binding's byte figure moved with the edit: its `tests/instruction_ceiling.rs` ceiling
  and the `docs/honesty.md` "Measured at this HEAD" cell both now read 106,428.

- **The comparator section's qualifiers are narrowed to what its source supports.** Nine observations three panels left standing on `docs/review-cost.md`'s `## What the
  comparison is between` are answered in one change, preferring deletion to explanation. The task-flow
  layer no longer names *review transactions* among the members with no counterpart, since the
  comparison project carries a review-receipt schema; the blind-judges row now says what launches that
  project's judges (a skill), where before both cells together read as if the markdown were inert; "the
  parity number" becomes the layer comparison it meant; the refuter's seal is cited from
  `RESERVED_REVIEWERS` (`src/harness/roles.rs:204-206`) rather than the constant's name; the `jd-judge`
  count is stated as *non-test lines of Go source*; the per-judge figures' `shape` is defined at its
  introduction as the handing, not the rubric; the C2 row's `--run-id` citation points at the
  disclosure it meant; and the closed-role-vocabulary fact is left in `docs/review-cost.md` and this
  changelog, its `docs/honesty.md` copy removed. The review-cost entry's superlative
  regains its `cost` scope, the `before this fix` phrase loses the count that did not resolve, and the
  sentence enumerating the judge's own definition no longer claims to enumerate it. The comparator
  table, the layer decision and the figures are unchanged in substance.

- **Which runtimes Estigia adapts, and on what evidence, is written down.** `docs/what-it-writes.md` gains the admission rule — a runtime is adapted when the instruction file
  its host loads and where it discovers a skill are verified from what the runtime itself ships —
  together with what a listed adapter does and does not promise (a gate only where the runtime can
  refuse a tool call) and what an unadapted runtime gets (nothing, and no invented command). The
  rule gives its answer rather than the asking, and names the count drift it carries.
  `docs/parity.md` settles adapter coverage the same way, and `AGENTS.md`'s enumeration of `docs/`
  names the parity document.

- **The review-tier run of the honesty contract has its own document.** `docs/honesty.md`
  had reached 7,827 lines, and its last run — "The review tier, the lens and the finding", 1,214
  lines — now lives in [`docs/honesty-review-tier.md`](docs/honesty-review-tier.md). The entries keep
  the order they were written; the run stays last and nothing follows it. `docs/honesty.md` keeps its
  `##` table of contents, loses that one entry, and points at the new document, which
  `docs/README.md`'s map now names. No entry was rewritten or removed, and the six pointers that
  crossed the new boundary — a "new entry below", a "fix-delta-digest entry above", a further
  entry, a refusal test named above, the `build.rs` vocabulary pointer, and a neighbouring "above" —
  now name the document they point at; one already-stale `docs/honesty.md` coordinate is re-aimed at
  the entry it meant. `tests/honesty.rs`, `src/config/tests.rs` and `src/setup/tests.rs` read the two documents
  rather than one path; the rest of the ledger is unchanged, and extracting it further is left to a
  later change.

- **The parity target has a document and a revision.** `docs/parity.md` fixes which
  `gentle-ai` release Estigia is compared against (`v3.7.0`, 2026-09-23), names the layer that is in
  scope — claim and gate, the state contract, independent review, the planning binding, adapter
  registration, the installer lifecycle — and records each ecosystem component Estigia deliberately
  does not carry, with where it lives instead. `docs/honesty.md`'s `a537c63e` citations stay as the
  checkout those measurements were read from, which the new document states rather than silently
  re-pointing. The shared layer's remaining differences are an ordered list; model routing is left to the work that already holds it, and the SDD phase set
  is recorded as a decision rather than a defect.

- **The comparator behind the review-cost figures is written down.** `docs/review-cost.md`
  gets a closing section naming which of the two review stacks' four reviewer layers — lens, refuter,
  targeted validator, blind judges — exist on each side, citing each from its own checkout: the lens
  and the refuter exist on both, the targeted validator has no counterpart here, and nothing in the
  comparison project's Go runs a blind judge, its reviewer role vocabulary being closed at
  three and holding none. It records the operator's 2026-08-31 ruling to compare layer against layer
  rather than the two totals, and states why the blind judges are reported beside the parity number
  rather than folded into it: `release_ci` requires an accepted verdict the comparison project's
  passive review cannot produce. It also names what the two figures the campaign took for a judge
  target actually measure — a lens's prompt bytes, and two lens executions whose results were
  discarded. `docs/honesty.md` records that no judge-versus-judge figure exists on either side, and
  that no figure in `docs/review-cost.md` measures a whole delivery.

- **A lens now states the two words the routing already read.** Every shipped lens
  definition now asks a lens for a class and, on a `severe` finding, for `evidence_class` and
  `causal_disposition` — the two fields `record_review_finding`'s routing and
  `skill/policies/blind-judges.md`'s own prose already read off a finding, which no lens
  definition had ever asked a lens to produce. `skill/agents/review-blind.md` now carries the
  confirming pass a judge owes an `inferential` lens finding in its brief, and the crediting
  rule in `skill/policies/blind-judges.md` now names the case where a verdict rests on a lens's
  own finding rather than a judge's. One cross-reference that no longer resolved is gone. Six
  previous bodies — the judge's and all five lenses' — join the historical ledger, so an install
  still carrying one upgrades rather than being refused as matching no rendering of its definition.
  `docs/honesty.md` records what the change does not close: what reaches `record_review_finding` is
  whatever the orchestrator passes, and the five new prose copies of the stale-prose rule are
  crossed by nothing.

- **A third limit on sealed judges is recorded.** The payload a sealed judge reviews is
  a copy the orchestrator re-emitted into its launch prompt, which no digest binds and no grant that
  judge holds could check against the receipt. `skill/policies/blind-judges.md` no longer calls that
  evidence checkable against the receipt, `docs/review-cost.md` now names the handing an instruction
  rather than something Estigia itself performs, and `docs/honesty.md` records the gap beside the
  two limits already there and the ported project's own residual. The entry carries a second
  measured instance, produced by its own review: transcribing this change's payload into a sealed
  lens, the orchestrator mistyped one registered byte count, every receipt identifier in that brief
  stayed correct, and nothing but a re-read caught it.

- **`docs/review-cost.md` measures what a blind review costs, against a real review target.**
  Two panels launched concurrently over one pull request — Estigia's shipped `review-blind`
  brief and a comparison rubric — came within 8.7% on tokens and 13.7% on
  latency. One behavioural finding, graded `severe` under one brief and not filed at all under
  the other though both panels read the same bytes, is what would force a repair round: the
  missing choosing rule between `severe` and `warning`, not judge reading speed.
  `docs/honesty.md` records what the measurement does not close. A second measurement beside it
  runs the compared project's own binary against a real review transaction and reports what each
  system sends a reviewer, how many it selects and how many corrections it permits; no reviewer
  result was ever admitted, so it compares nothing about what either system finds.

### Removed

- **The `Project board` setting.** Whether the workflow state was
  mirrored onto a GitHub Project was decided by a `Project board` row holding `owner/number` or
  `none`, and the mirror targeted that one board. The row and `BoardRef` are gone; `create` and
  `transition` read the projects an issue belongs to — its own `projectItems` — and set the `Status`
  column matching the state by name or description in **every** project the issue is on,
  best-effort and before the label edit as before; a project with no matching column is a reported
  skip that never blocks the label. `OPTIONS_SETTINGS_WITHOUT_BOARD`, `Setting::applies_to`, the TUI
  row, `doctor`'s `inert_for_this_tracker` and `setup::needs_operator_answer` go with it, and
  `skill/bindings/github/board.md` is retired.

- **The `list-boards` operation.** The paged listing that fed the
  setup screen's board picker is gone, with the picker itself and the TUI state behind it. Nothing
  else read it, so `Project board` is typed directly until the row itself is removed.

- **The `audit-board` operation.** The paginated pass that compared
  every card's column against its `status:*` label — and `--fix` to repair it — is gone. The mirror
  in `create` and `transition` and its read-back are unchanged; what leaves is the standalone drift
  audit and its four answer lists (`drift`, `missing_column`, `foreign`, `unread_labels`), with the
  `board audit` row in `skill/bindings/github.md` and the drift section in
  `skill/bindings/github/board.md`. The priority-scale label check it also ran survives in
  `list-state`'s `unchecked`.

- **This repository's own `openspec/` tree is retired; the `Planning` axis it is written under
  stays.** The tree held this repository's own planning artifacts, and it is
  gone — the README, `AGENTS.md`, `docs/README.md` and the documentation-link walk no longer point
  at it. What did *not* change is the option itself: `Planning` still accepts `direct`, `sdd`,
  `sdd lite`, `sdd openspec` and `sdd lite openspec`, a repository that chooses `sdd openspec` still
  writes its artifacts under `openspec/`, and `skill/protocols/sdd.md` still states both homes. This
  checkout simply no longer keeps one of its own.

- **Every test that read the text of a comment.** A comment is a note to a reader, and six
  mechanisms had made it structural: a ratchet counting comment-bearing lines against a registered
  `CEILING`, whose only cure was to raise the number and whose sibling turned red on a *warranted*
  deletion; eleven `guard:population` declarations parsed out of `#[doc]` attributes; a refusal
  taxonomy scraped from `src/transport/` doc comments; two tests whose whole subject was comment
  prose — one requiring a sentence that lives in nine doc comments and nowhere else; and, in every
  direction, assertions searching Rust source as raw text, where a correct comment naming
  `tracker::invoke(` or `fs::write(` failed a test about the gate or was reported as writing a file
  in place. The declarations are `Population` consts now — the type in `harness::population`, each
  const beside the code it describes and naming the `subject` it is about, so the binding is a name
  that must resolve rather than adjacency. The taxonomy is crossed against the vocabulary `build.rs`
  extracts from the transport's own code. Everything that reads a Rust source now strips it first
  with the new `source_text::code_of`, which is the ceiling's own lexer inverted, and strips *before*
  slicing, since a boundary taken from raw text — `#[cfg(test)]` above all — is one a comment can
  move. Nothing enforces that rule; `AGENTS.md` states it as a shape a reviewer checks and
  `docs/honesty.md` records what is no longer measured, including the two panels that rejected this
  change for claiming otherwise.

- **The six workflow-state keys of `Model routing`, and the four lookups that
  served them.** `analysis=opus` parsed, round-tripped, and was reported by
  `config list` as set, while nothing anywhere consulted it: `for_state` was
  byte-identical to `for_phase`, `resolve_in` ranked it last of three, and no
  caller outside the configuration module's own tests ever asked any of them.
  Where the issue sits names no context to start, so there was never anything for
  the family to route to — and a row that parses and does nothing is worse than
  an absent row, because it reads as configured. The row now takes sixteen keys
  in three families and every family reaches a rendered definition. `for_state`,
  `resolve`, `resolve_in`, `get(Role)` and `for_phase` are gone with it; `route`,
  `for_target` and `effort_for` are what remain. A stored table naming a state is
  refused rather than read past, in the same whole-cell way every unknown key
  already was.

- **The differential suite and its corpus.** `tests/differential.rs` — a hundred
  and eleven crossings — and `tests/transport/oracle.json` — 228 answers recorded
  off the retired Python — are deleted. A crossing's key was a fingerprint of its
  whole question, and the question carried the installed layout, so the corpus
  froze the skill directory's name; renaming it asked one crossing something the
  corpus had never been asked, and extending a corpus means running a reference
  implementation this repository no longer has and does not want back. Two
  `tests/honesty.rs` guards that policed the corpus went with it. What this cost
  is written in `docs/honesty.md`: the
  port's agreement with a second implementation is no longer asserted anywhere.

### Known gaps

Written down rather than discovered — see *What this instrument does not
measure* in the README. The gate is installed for one agent, the MCP server has
never met a real client, nothing has been tested against a live tracker, and the
two-minute renewal window is a cadence and therefore a gap.

