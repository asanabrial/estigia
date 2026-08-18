# Changelog

Notable changes, newest first. The format is the one `changelog_notes` reads:
a version opens its heading, and the entry under it is what the tag and the
Release carry. That command fails closed on a missing or empty entry, because a
tag is immutable and notes invented at tag time are permanent.

## 0.1.0 — unreleased

The first version. Estigia is a **harness**: it does not ask an agent to follow
the workflow, it holds the tools.

### The harness

- **CI now answers before the reviewing, instead of after it.** `.github/workflows/ci.yml` gains a
  `workflow_dispatch` trigger, `publish_review` and `republish_review` start **one run of it per
  publication epoch** against the head they just pushed, and `record_review_verdict` refuses to bank
  an `accepted` verdict for a head whose lane concluded red or has not finished. Before this, the
  first cross-platform signal arrived when `release_ci` marked the pull request ready — after every
  verdict had been obtained — so a platform-only defect killed the receipt and every verdict bound
  to it. Measured on issue #2: sixteen reviewers across eight rounds discarded for a missing
  `create_dir_all` in a fixture that only POSIX could see.

  Both halves, because the first alone is a rule somebody has to remember, which is the failure
  issue #28 concluded against. The refusal sits on `record_review_verdict` because it is the single
  writer of a verdict marker — `release_ci` reads what it wrote — so one enforcement point covers
  every route to one, including the handoff route. Check runs attach to a SHA by construction, so the
  evidence is read for the receipt's own head and `ReviewReceipt` is unchanged: no run id, and
  nothing new for a republish to invalidate.

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

- A row about **this machine** now reaches every installed contract, and cannot be given a different
  answer for one agent. `Setting::scope()` has three values and both write paths were written as
  though it had two: the plain `config set` asked `elsewhere()` only for `Scope::Everywhere`, so
  `Summary language` and `Issue body language` landed in the canonical contract and nowhere else;
  and `config set --agent <slug>` refused an `Everywhere` row while accepting a `Machine` one it
  could not hold, since a shared root's per-agent file is rendered through `render_some_agent_rows`
  and read through `Scope::Agent` — the row was dropped on the way out and the command exited on its
  own read-back, reporting `setting-shadowed-by-local-file` and blaming an operator file that need
  not exist in that root at all. Two agents on one machine could therefore answer a question that
  has one answer differently, with **no command that made them agree**, and nine of the eleven
  adapters share the neutral root, so the unreachable case was the common one. The plain form now
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
  installs one static read-only `review-blind` definition with `model: inherit`; it remains inert unless
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

  **It may only narrow, and that distinction is the change.** `cwd` is written by an adapter's hook,
  which knows what it is gating, and is taken as given. `workdir` is a tool *argument*, so whatever
  composed the call wrote it — a model, on every runtime here. Read as freely as `cwd` it stops being
  evidence and becomes a lever: measured under review, with two live pointers and a `git push` under
  a claim, a `workdir` of `..`, of the parent checkout, or of `C:\Windows` resolved, was covered by no
  run, and was answered `outside` with exit **zero**. The command still ran where it was going to
  run; the gate simply stopped adjudicating it, and the same spelling reached `write` and `edit` too.
  A payload that can move the decision is a payload that can leave the gate, which is worse than the
  false ambiguity the key was read to fix — the widened gate that looks exactly like working
  correctly. So the resolved value is **placed** — its spelling collapsed the way the platform
  collapses it, then resolved as far as the filesystem exists — and one landing outside the launch
  directory is discarded in favour of it, which is where the decision sat before this key was read at
  all. Both blind reviews of the unclamped version rejected it on precisely that.

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
  reaches it as it reaches everything else. That held only outside the renewal window until issue #29:
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
  latest publication receipt and exact ownership epoch before idempotently unassigning, keeps the
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
