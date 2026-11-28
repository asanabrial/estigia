# Setup, doctor and the host plugins

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what setup, doctor and the generated host integrations do not establish. The index says which area holds which question.

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
- **An older installation has no record of an operator's empty hooks wrapper.** An uninstall
  can remove that wrapper if setup populated it before provenance was recorded and removing
  Estigia's hooks empties it. New setups record an existing empty object before writing hooks.
  `cursor_uninstall_restores_preexisting_empty_hook_wrapper` checks restore after reinstall and
  verifies that previews preserve the live file. Cleanup of generated wrappers is checked by
  `an_uninstall_leaves_no_empty_husk_of_its_own_making`.
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
- **An unexplained configuration defect's refusal cannot name the row, and nothing checks that it
  could.** `translate`'s code-2 arm answers an unexplained `Failure::ConfigDefect` or code-2
  `Failure::Stop` — `invalid-finding-class`, `missing-argument`, `incomplete-review-transaction` and
  their neighbours, none of which the configuration has any part in — as `no_command` with
  `operator-knowledge`: the row is the operator's to decide, and no command the binary has was going
  to clear it. The transport named no setting, so the sentence says only that the row is the
  operator's; naming the right one needs a per-reason decision this fallback does not have.
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
  [Estigia can prove what it launched, not who answered](honesty-review-panels.md#review-targets-panels-and-the-lane-that-runs-them)
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

- **`doctor` checks twenty-two things, not everything.** Skill, installed skill assets, `gh`
  authentication, a git remote, this repository's own configuration rows (an unknown label, a row
  that does not parse, or a row that would loosen a machine answer), this repository's push guard,
  the contract each configured agent
  reads, whether the root the gate decides in carries the rows those agents read, whether the gate
  each of them registers would actually run, whether the tool server each
  of them registers would actually start, whether the operator has the gate standing down right
  now, whether every run pointer on the machine can still say what it holds, whether a pointer that
  *does* parse still names an issue the tracker holds open, whether branches and checkouts no
  delivery owns have accumulated, whether any call has reached that
  gate and gone undecided, whether the binary answering this command is the checkout's own
  build, whether the local workflow store opens, whether that store ever counted a record it could
  not write down, whether Pi's own settings exclude Estigia's
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
