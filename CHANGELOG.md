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
