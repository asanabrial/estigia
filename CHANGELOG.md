# Changelog

Notable changes, newest first. The format is the one `changelog_notes` reads:
a version opens its heading, and the entry under it is what the tag and the
Release carry. That command fails closed on a missing or empty entry, because a
tag is immutable and notes invented at tag time are permanent.

## 0.1.0 — unreleased

The first version. Estigia is a **harness**: it does not ask an agent to follow
the workflow, it holds the tools.

### The harness

- `estigia mcp` serves the workflow operations as **18 MCP tools** over stdio —
  claim, verify_claim, heartbeat, transition, comment, reclaim, release,
  start_branch, publish_review, and the read-only checks. Hand-written
  JSON-RPC rather than `rmcp`: the same binary answers a `PreToolUse` hook on
  every edit, so an async runtime per process is a cost paid thousands of times
  to move a few lines of JSON across a pipe.
- Review publication now forms a cooperative draft/ready CI barrier. `publish_review` drafts and
  confirms reused PRs before push, creates new PRs draft, and records a fresh epoch over
  PR/head/base/clean-target digest, invalidating old evidence even for identical bytes.
  `release_ci` re-verifies the live review claim, latest full receipt, current draft PR and
  coherent re-derived clean target before marking the PR ready and confirming every ambiguous outcome
  by readback. GitHub cannot condition ready atomically; out-of-band collaborators and repository
  workflows can bypass the order, so no malicious-collaborator authenticity is claimed.
  Topic pushes and PR open/synchronize events no longer start CI; default-branch pushes
  remain, and PR CI starts on `ready_for_review`.
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
  run that dies leaves no phantom state.
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

### Setup

- Added `scripts/build-install.ps1` for a complete Windows source reinstall. It installs Rust 1.97 when
  needed, compiles and force-installs the locked release build, runs all-agent setup with explicit
  source-build consent, and verifies the result with lifecycle status, installation status, and
  doctor.
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
  registration for eleven adapters. `setup --all` continues after one adapter
  refuses, reports any actions already proved for that adapter, and still
  configures the unaffected adapters. One unconfirmed adapter keeps the batch
  `Unknown` even when later adapters complete.
- **Uninstall is the exact inverse.** It removes what Estigia wrote and nothing
  else, and a file that held nothing but Estigia's block does not survive as an
  empty husk. JSON keeps the operator's key order; markdown comes back byte for
  byte.
- Eighteen typed settings. Reading the table produces a valid configuration or a
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
  delegated roles and external sub-agent names; inactive phases and workflow
  states stay persisted but hidden. Each row opens advisory models for the concrete host:
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

- Every refusal carries a stable code, what happened to the world, whether a
  replay is safe, and a resolution — either a runnable invocation checked
  against the real parser, or a reason from a closed vocabulary saying why no
  command can honestly exist.
- The transport's **69 refusal reasons** are derived from the transport itself
  at build time, so the two cannot drift.

### Guards

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
  it — eight adapters share the neutral root, so a row answered there is the row
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

### Removed

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
