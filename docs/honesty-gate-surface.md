# The control surface and the seams

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the control surface records, who else reads it, and the calls no gate sees. The index says which area holds which question.

## The control surface and what it records

- **The three surfaces are asked of the landing as well as the spelling, and the landing is resolved
  once.** A write whose spelling is ordinary but whose path resolves through a link inside the checkout
  onto the person-only record, the control surface or the repository configuration is classified as the
  surface it lands on (`a_write_that_lands_on_a_persons_record_is_that_record`). Still open: a **hard
  link** to a surface is not resolved, a link created between the classification and the write is not
  seen, and a landing `placed` declines is read as ordinary, so the spelling decides it.
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
- **The follow-up's single-sourcing is named, not observed.** The branch-link follow-up
  (`src/transport/claim.rs`) and `review_status` (`src/transport/claim/status.rs`) both interpolate
  `closing::FOLLOW_UP`, and a test compares the emitted text to the constant's value rather than its
  source, so a byte-identical copy at either site would leave it green. What is held is the transition: deleting `transition --to done` from `FOLLOW_UP` reddens
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
    derived from the adapter table (the measurement is in *The control surface and what it records*, the first section of this file).
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
  - **Only one of the two derived families is compiler-forced.** The measurement is in the
    [roles entry](honesty-review-rules.md#roles-and-the-definitions-they-need)'s *Still open* list, under the bullet beginning *"Only one of the two derived families"*.
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
- **Claude Code subagents are covered; that is why the entry on OpenCode subagents below is OpenCode's and not the
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
    `claim` with `branch` and `base` makes the local branch and the worktree and queues the link to the issue while the
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
- **The MCP server decides with the settings it read when it started.** `estigia mcp` builds its gate
  context once and keeps it for the session, while the hook builds one per call. `deliver`'s merge
  decision reads the `Git operation window` again before asking the gate
  (`operation_window::current`;
  `the_live_merge_decision_reads_the_window_the_repository_answers_now`), and the transport reads it
  on every dispatch. Every other field of that context, the window `claim` with `park` consults among
  them, still answers what was configured when the server started. A server whose binary was
  replaced keeps running the old build, which no check names.
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
