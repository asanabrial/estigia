# What this instrument does not measure

An axis list that names what is not covered is the difference between "zero defects found" and
"zero defects found where we looked".

Read this before adding a claim elsewhere in the repository. It records known limits, their current
measurements and the tests that hold them. Unmeasured cases are not clearance.

`tests/honesty.rs` crosses the *countable* claims here against the code: the number of agents, the
number of tools, the number of doctor checks, the settings rows. A count that drifts fails the
suite. Everything else here is prose held by review.

## How this file is organised

[`docs/README.md`](README.md) maps the documentation set. Entries retain their original order;
the section links below provide navigation rather than a subject index.

- [Test evidence, and the run id shape](honesty-testing.md#test-evidence-and-the-run-id-shape)
- [Comments, instructions and the build](honesty-testing.md#comments-instructions-and-the-build)
- [Models, routing and reviewer definitions](honesty-review-panels.md#models-routing-and-reviewer-definitions)
- [Node, CI and the actions that run the suite](honesty-testing.md#node-ci-and-the-actions-that-run-the-suite)
- [The OpenCode plugin, and what it knows about a call](#the-opencode-plugin-and-what-it-knows-about-a-call)
- [The suite, its fixtures and the retired port](honesty-testing.md#the-suite-its-fixtures-and-the-retired-port)
- [Review targets, panels and the lane that runs them](honesty-review-panels.md#review-targets-panels-and-the-lane-that-runs-them)
- [Findings, grants and locations](honesty-review-findings.md#findings-grants-and-locations)
- [The control surface and what it records](honesty-gate-surface.md#the-control-surface-and-what-it-records)
- [What the gate writes, and who else reads](honesty-gate-surface.md#what-the-gate-writes-and-who-else-reads)
- [Roles and the definitions they need](honesty-review-rules.md#roles-and-the-definitions-they-need)
- [Skips, deletions and the bounds of a round](honesty-review-rules.md#skips-deletions-and-the-bounds-of-a-round)
- [The correction budget and the publication lane](honesty-review-rules.md#the-correction-budget-and-the-publication-lane)
- [Setup, configuration and uninstall](#setup-configuration-and-uninstall)
- [The gate's reach across the agents](honesty-gate-reach.md#the-gates-reach-across-the-agents)
- [Delivery: the receipt, the base and the remote](#delivery-the-receipt-the-base-and-the-remote)
- [The seams, and the gates that never see a call](honesty-gate-surface.md#the-seams-and-the-gates-that-never-see-a-call)
- [The push guard, the MCP server and the state a run keeps](honesty-gate-surface.md#the-push-guard-the-mcp-server-and-the-state-a-run-keeps)
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
- [Test counts and mutation claims in a receipt](honesty-testing.md#test-counts-and-mutation-claims-in-a-receipt)
- [Citations, and the coordinates that rot](honesty-testing.md#citations-and-the-coordinates-that-rot)
- [Generated files and reproduction](honesty-testing.md#generated-files-and-reproduction)
- [Which hook commands are Estigia's](#which-hook-commands-are-estigias)
- [The Pi system section and the prompt cache](#the-pi-system-section-and-the-prompt-cache)
- [What `release_ci` says about CI](#what-release_ci-says-about-ci)
- [What `deliver` does not see](#what-deliver-does-not-see)
- [What `estigia changes` does not do](#what-estigia-changes-does-not-do)
- [The session registry](#the-session-registry)
- [The session mailbox](#the-session-mailbox)
- [The npm installer and the registries](honesty-testing.md#the-npm-installer-and-the-registries)
- [The release builds; CI tests](honesty-testing.md#the-release-builds-ci-tests)
- [The waits audit](#the-waits-audit)
- [Re-running a red lane](honesty-review-rules.md#re-running-a-red-lane)
- [The HTTPS repeat](#the-https-repeat)
- [The token-efficiency programme](#the-token-efficiency-programme)

The last run, **The review tier, the lens and the finding** has its own
document: [`honesty-review-tier.md`](honesty-review-tier.md). It stays last in the order and nothing
follows it. Within it, "the entry above" and "the entry below" reach the neighbouring entry, and a
pointer that leaves it names the document it points at.

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
- **`base-integration-not-required` proves a textual fact, not a semantic one.** The refusal needs the
  parent head to merge cleanly with the current base, the paths the base touched to be disjoint from
  the delivery's, and `required_status_checks.strict` to read `false` or the branch to be unprotected.
  None of that says the change still means what it did on the advanced base; `integrate_reason` is the
  run's recorded judgement that it does not, and nothing reads it back to decide. The protection is
  read through the branch-protection endpoint only: a repository that requires an up-to-date branch
  through a ruleset can answer `Branch not protected` there, which is read as non-strict, and the
  integration it requires is refused until `integrate_reason` is given. A protection read that fails
  any other way, or answers something unreadable, is not a refusal; the answer carries
  `integration_unread` and the integration goes through. The same holds for a failed ancestry or
  `git merge-tree` read. The check runs only over a parent receipt on the same pull request that is
  not the root of a recovered successor lineage. `integrate_reason` is recorded on the marker and,
  unlike `delta-range-reason`, is not echoed by `review_transaction`.

## What the gate covers

- **The gate covers two directories: the checkout the claim was made in, and the isolated one
  the claim created or a resumed run adopted.** A run that makes its own worktree by hand (one git
  has never registered, or whose ownership marker names no issue) gets that write gated but the directory
  is never recorded, so subsequent writes there read as `Outside`. Use the tool, or resume through a
  checkout the transport already knows about.

  `claim` with `branch` and `base` records the claim's checkout as well as the worktree, filling and never overwriting. A
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
  closed, `create`, `comment`, `transition`, `claim`, `claim --takeover` and `unassign` queue
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
  worktree directory named with the provisional number keeps that name. A claim whose flush issues the
  real number writes that number and the checkout `claim` started into the run pointer, read from the
  transport's answer rather than from the provisional argument the call was made with. A provisional
  issue opens its ledger without any GitHub call, and an unallocated or unrecorded number is refused.
  A provisional number is read from this machine's store alone: `list_state` with `issue`, and the gate's
  `provisional-issue-number` answer to `gh issue view|comment|edit|close|reopen` or a GraphQL `issue(number:)`
  naming one. The match is on the command line's words, so a number reached through a variable or a script
  goes to GitHub and reads as not found, and a number another machine allocated reads as never allocated
  here. `-R` naming another repository than the allocation's is left to GitHub. The tests are
  `provisional_read` in `src/harness/` and the offline cases in `src/transport/outbox/`.
- **The window's wake is told, not held, on two hosts.** `opens_at` is computed from this machine's
  local zone; under a test clock the offset is held fixed, and off Unix no daylight-saving change is
  read. In the refusal it is text, not a field. Claude Code's loop only tells the turn to schedule its
  own wake and lets the second stop for one opening through, so a turn that schedules nothing is not
  woken. Codex's `Stop` keeps no loop record, so it is told on every stop. Pi's and OpenCode's loops hold
  the timer themselves (`windowWait`), and lose it when the host process ends; they wait at the first
  closed-window stop even while another issue is claimable, where Claude Code's and Codex's `Stop` keep
  the turn going and name `park: true`. The tests are
  `next_opening` in `src/harness/operation_window/tests.rs`, the closed-window cases in
  `src/cli/tests.rs`, and the `window` scenarios of both loop drivers in `src/setup/tests.rs`.
- **A parked claim is the pointer's note, and nothing renews it.** `claim` with `park: true` keeps
  several claims under one run id, so the store's adjudication is untouched. Only the active one is
  covered and verified. Nothing checks that a parked issue's work is built, only that the window
  was closed when it was parked. A parked claim's horizon passes unrenewed, and once it has lapsed
  another run may `reclaim` it without `force`. A release puts down only the claim it names: the
  parked list outlives the active claim, a release naming an issue the run does not hold changes nothing, and the pointer goes once nothing is left on it. A pointer that still lists parked claims is kept when storing it fails; no test injects that failure. `SessionEnd` forgets the pointer and the parked list
  with it, as it does a single claim, so after that a parked issue is found again only from the
  store. The tests are `park_tests` in `src/harness/mcp/tests/`, plus the prune and rekey cases that
  name a parked claim.
- **The transition record is the store's own, and it is not a log of everything that moved a state.** Schema 16
  adds `transitions` (`at`, `run_id`, `from_state`, `to_state` per issue), append-only like `entries`, and
  `Ledger::set_state` writes the row in the same transaction as the state, so a history that cannot be written
  leaves the state unchanged and the transition fails. Every state change a command makes goes through
  `set_state` (`transition`, `transition --to` on `claim`, the merged-release close, the begin-review step, and the
  flush's reconverge), so none bypasses it; setting the state an issue already has records nothing, which
  makes the flush's re-projection invisible. What it cannot see: a transition names no caller, so `run_id` is
  whoever holds the issue at `at` and is empty when nobody does; the state an import reads from the labels is a starting point, not a transition, and states set before
  schema 16 have no row, and
  the first change after it records `from_state` as the state the store held; a label edited on GitHub by hand
  never reaches the store, and another machine's changes are another machine's store. Nothing reads the
  record yet.
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
  stands for. `State names` is read by `assist`, `draft_pr`, `ready_pr` and `review_stages`; `Corrections after ready`
  by `draft_pr`; `Ready by` by `draft_pr` and `ready_pr`; `Self review` by `draft_pr` and `ready_pr`; `External reviewers` by `review_stages`; `Merge by` by
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
  An operator sets thirty-two rows and `config list` reports all of them. Measured: `context.get` is
  called for the labels in `READ_BY_THE_TRANSPORT` — `worktree location` and `Review budget` (for the review forecast line). The gate reads
  `Irreversible commands`, and `Git operation window` is read from the repository's own file by the transport and the gate alike, never through `context.get`; and the payload's prose names the rest of the acted-on rows. The four rows that
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

- **`doctor` checks twenty-one things, not everything.** Skill, installed skill assets, `gh`
  authentication, a git remote, this repository's push guard, the contract each configured agent
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

## Identity and the tracker

- **The identity and the verdict reach different surfaces, and a refused call carries neither.**
  `answer_text` in `src/harness/mcp/mod.rs` adds `build` from `src/build_info.rs` only to successful JSON
  object answers without `compact: true`. Refusals reveal no build identity. The `gate_version` verdict
  reaches doctor and `hook::session_start_response`, not tool answers; the session note is a one-time
  reading of local `GateContext::repo_dir` `HEAD`, never the remote default branch. Later commits, pulls
  or branch switches leave it stale. Any feature commit absent from a clean main-built binary reports
  `Broken`; dirty or provenance-free builds report `Skipped` without an action. `Broken` requires both
  reinstall and session restart, so its note names no ineffective in-session command or `Resolution`.
  These are code readings; no test here proves the note stays current during a session.
- **The compact `task` answer is chosen by skill prose, not by adapter code.** It carries `version`
  (`COMPACT_TASK_VERSION`) and the changed row; a retried acknowledgment answers the same row and a
  reopen keeps the closing commit (`a_lost_compact_acknowledgment_retried_answers_the_same_row_in_the_same_place`).
  No adapter sends `compact` on `task` by itself. Two connections writing one checklist at once lose no
  row and keep each writer's order, and a row both revise keeps one writer's whole revision
  (`two_writers_on_one_checklist_lose_no_row_and_keep_each_writers_order`); that is two threads on one
  SQLite file, not two operating-system processes.
- **A compact receipt is expanded from this run's pointer, not from the store.** `expanded_receipt` in
  `src/harness/mcp/mod.rs` fills `pr`, `head`, `base` and `digest` for a call that names only `epoch`
  when the calling run's pointer holds that epoch's review transaction for the issue
  (`a_receipt_named_by_its_epoch_is_filled_from_the_pointer_holding_it`). Another run, or a pointer that
  lost its transaction, gets the old missing-argument answer. The compact view carries `compact_version`;
  `collect`, `stop` and an unpublished answer keep what they cannot rebuild, and a compacted `execute`
  expands back to its arguments (`a_compacted_execute_answer_expands_back_to_the_arguments_it_dropped`);
  an `expected_epoch` and a mixed collect's slots, jobs and deadlines pass through unchanged. There is no
  CLI `review_status` to compact: the view exists over MCP only.

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

  **A single holder is deliberately not reconciled.** An unrecognised close still refuses
  `issue-not-open`; dropping that holder would instead yield `Outside(NothingSworn)` unless another
  early denial applies. Changing the reconciliation threshold to one reddens
  `a_single_stale_holder_still_refuses_through_its_own_issue_not_open`. A recognised delivery close
  (`src/transport/claim/close.rs`) answers `closed-by-own-delivery` and remains `Live`.

  **Doctor has three pointer buckets:** `issue-not-open` (release named), own-delivery `stranded`, and
  `unread`. `the_fine_sentence_claims_only_the_buckets_the_row_empties` holds the empty-bucket `Fine`
  claim; `own_delivery_close_reads_the_transports_own_prefix_and_nothing_else` holds the prefix predicate,
  not end-to-end stranded delivery. Two blind spots remain: a lapsed stranded acquisition becomes
  `not-current-live-holder` and escapes every bucket; a deleted/moved/non-repository checkout makes
  `same_git_repository` false and excludes its pointer entirely.

  **Two dead pointers reconcile to zero and stand aside; one still refuses.** With no unreadable
  holding, zero means `Outside(NothingSworn)`. A live holder survives beside any number of dead ones.

  Multiple healthy runs also cover their shared base checkout. Release is offered only for
  `not-current-live-holder` (missing acquisition, expired horizon or lost race); other survivors get no
  command because checkout isolation is MCP-only. `plan_release` searches live and stale ownership;
  absence clears only the pointer, presence writes under the operator's identity after repository
  agreement. Multiple holders cost one tracker read each plus the survivor's gate read unless renewed
  (typically two to three). This precedes `standdown::over`; a hung read cannot be escaped by stand-down,
  untested. Push narrowing happens earlier: one matching branch skips reconciliation and its
  `ClosedIssue` filter, held by
  `narrowing_before_the_tracker_can_deny_what_reconciliation_would_have_allowed`.

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
  Developer step 6 is prose. `transition --to done` restarts renewal for the held issue, and
  `release_ci` restores receipt/pointer fields and verifies; neither forgets the run. A recognised
  delivery close leaves a `Live` pointer reconciliation cannot drop. Measured: a later claim makes two
  holders and its first write refuses `several-runs-hold-this-checkout`. Only doctor's stranded bucket
  names the leftover; delivery still reports success when release is skipped.
  `releasing_a_delivered_issue_puts_its_checkout_and_branch_down` and
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down` in `tests/pipe/part_09.rs` read back
  directory, registry and branch removal. Production cleanup remains best-effort/self-reported, and
  no guard test covers a single recognised-delivery holder (the single-stale test covers other closes).

  **Naming the release in step 6 costs the `dev:<runtime>` label on every delivery that follows it.**
  `release`'s `unassign` ends through `converge_ownership_projection` (`src/transport/claim.rs`): with no
  holder left, every `dev:<runtime>` label is emitted as `--remove-label` from an inline
  `BTreeSet::difference`. This is not measured: `src/transport/claim/tests.rs` exercises
  `converge_projection`, a helper with zero production callers, and disabling the production emission leaves
  the whole suite green. A query grouped by `dev:<runtime>` no longer shows which runtime delivered a closed
  issue; only the run-id in the close evidence survives.

## The gate's boundary

- **A redirect to `/dev/null`, `/dev/stdout`, `/dev/stderr` or `/dev/fd/<n>` is not read as a write.**
  `shell::redirects_into_a_file` matches the target word exactly, quotes allowed; a word that only begins
  with one of them (`/dev/nullx`, `/dev/null/x`) or a target it cannot parse stays a write. Not seen: a
  redirect through a variable or substitution (`> $sink`), or a `/dev/null` that is a regular file. Held by
  `a_redirect_to_a_stream_is_not_a_write_and_any_other_target_is`.
- **The redirect reader models the quote rules that hide a `>`, and fails closed where it cannot.** It ends
  a double-quoted region only at an unescaped `"`, reads `$'...'` with backslash escapes, treats an unquoted
  `#` at a word start as a comment (a `>` inside it is still a redirect: `echo x # > y` is a write), and
  skips a here-document body as data; a line that ends inside a quote after hiding a `>` is a write. Not
  modelled: a body `here_document` refuses (an unquoted delimiter with `$`, a backtick or a backslash in the
  body) is scanned as code, and `$"..."`, `${...}` and `$(...)` nesting are not tracked
  (`a_redirect_after_a_quote_the_scan_could_misread_is_still_a_write`,
  `a_quoted_arrow_stays_data_whichever_quote_holds_it`).

- **An unclaimed run is refused a commit or a push only where the push guard is installed, and only for the
  spellings the command line names.** `unclaimed-commit` is raised when no claim holds the checkout and
  `guard::state` answers `Installed` or `Chained` (running `estigia guard` is the opt-in), on any branch,
  since any unclaimed commit or push skips the flow. A guard reading `Unreadable`, `Inert`, `Foreign` or
  `Absent` stands aside (denying would lock every commit because of a hook file; `doctor` reports the first
  two as broken), as does a repository whose tracker cannot adjudicate an unsworn write, whose skill has no
  contract file, or a checkout with no remote for a GitHub tracker that names no repository
  (`unclaimed::applies`). `--abort`/`--quit`/`--skip` recovery and `git pull` are not refused;
  an alias, script, editor or `gh api` writing a commit passes; `git push --no-verify` skips the hook half; a
  **claimed** run is not held to its worktree (its claim covers the checkout it was made in too). Held by
  `a_commit_without_a_claim_is_refused_where_the_guard_is_installed`,
  `a_push_without_a_claim_is_refused_on_any_branch_where_the_guard_is_installed` and
  `the_pre_push_hook_refuses_an_unclaimed_push_where_the_guard_is_installed`; nothing drives the two shell
  entry points (`estigia gate` with no session, the dialect hook with an empty session id) end to end.
- **`Unclaimed write policy` holds the writes the hook sees, and cannot hold the rest.** With the row
  `required` or `watch`, a run holding no claim is refused (`issue-required`) or asked before a routine write
  the gate can place inside a checkout whose tracker can name an issue there and that has an installed
  contract (a file edit or write, or a shell line carrying a `REPOSITORY_SHELL` fragment such as
  `git commit`), and, under `required`, before one it **cannot place** when the call was launched in a repository or
  from a directory whose repository could not be established. Under `watch` such an unplaceable write
  **runs**: launched in a repository, the checkout's `git status` (untracked files included, with each
  path's time and size, so a second write to a modified file still counts) is recorded first and
  compared on the run's next call, which is put to the person naming the changed paths only when it
  differs; launched where the repository could not be established it is asked; launched outside every
  repository it is allowed, and the checkout it may have dirtied is named at the next session start (at
  most eight paths, then a count; it cannot prove they came from an allowed write rather than the
  operator's own work). **What `watch` cannot see:** the write has already landed when the person
  hears of it, and nothing is reverted; a change made by anyone else in the same checkout between the
  two calls is reported as the watched write's; ignored files and writes outside the checkout are not
  in `git status`; a call issued in parallel with the watched one can compare before it finished, so
  the change is reported on a later call, or not at all when a write the gate can place is asked in
  between, which drops the record; and on a host other than Claude Code the last call of a session is
  compared by nobody until the next session start names the dirty checkout; Claude Code's `PostToolUse`
  entry compares right after each `Bash` call and says what changed at once, a note the person sees but
  is not asked about until the next call. Held by
  `an_unplaceable_shell_write_runs_under_watch_and_the_next_call_asks_only_when_the_checkout_changed`,
  `a_write_placed_inside_the_checkout_is_still_asked_under_watch` and
  `the_shell_call_that_changed_a_watched_checkout_is_reported_as_soon_as_it_ran`. What it does not hold:
  - **A boundary is held as a commit is, unless the repository says `Unclaimed GitHub writes: allowed`, and
    `Merge by` is not read on this path.** An unclaimed `gh pr merge`, `git merge`, `git pull`, `git tag`,
    release, `gh api` mutation or declared irreversible command is asked under `watch` and refused under
    `required`, and so is `Action::Remote` (`gh pr review`, `gh pr comment`), through
    `issue_requirement::decision` from both `decide_observing` and `adjudicate_pushing`. It is **asked,
    never watched**: a boundary reaches other people and leaves no trace in the checkout to compare, so the
    `watch` that lets an unplaceable local write run does not apply. `allowed` is the operator's own
    declaration that a skill writing through `gh api`, `gh pr review` and `gh pr merge` unclaimed is not to
    be stopped on each call; it drops no question on a commit, a push, a branch or a worktree command, and a
    write carried beside a boundary stays held under both values (`harness::write_beside_a_boundary`, the
    classifier answering with a line's strongest command, which once let `gh pr merge 3; git worktree remove
    /x` run unasked). **What this costs:** a person's own `git merge`, `git pull` or `git tag` in a session
    that holds no claim is asked too; under `held` a skill that reviews and then merges meets one question
    per merge; and a pull request approved at the exact head does not clear it, because the skills that
    need this road do not take it. A command the fragment lists do not name is not found, and a GraphQL
    mutation behind a shell variable still passes. A call launched outside every repository is left alone,
    as a push is, and the git operation window's own refusals are unchanged. Held by
    `an_unclaimed_github_write_is_put_to_the_person_under_watch`,
    `an_unclaimed_github_write_is_refused_under_required`, `an_unclaimed_github_write_passes_when_the_policy_is_off`,
    `a_github_read_passes_under_every_value`, `an_unclaimed_github_write_is_held_through_the_gate_command_too`,
    `allowed_lets_the_github_writes_pass_under_watch_and_required` and `allowed_still_holds_what_the_policy_holds`.
  - **An assisted run cannot carry a GitHub write past the claim beside an admitted boundary.** `git pull;
    gh pr review 3 --approve` and a declared boundary followed by a GitHub write are refused
    `assisted-remote-write-unverified`, checked only after `assisted-does-not-deliver`,
    `assisted-publishes-through-draft-pr` and `assisted-api-mutation-unverified` have answered, so a merge
    still names the delivery. Held by `an_admitted_boundary_does_not_carry_a_remote_write_past_the_claim`
    and `the_specific_assisted_refusals_answer_before_the_remote_write`. The write is found by the same
    fragment lists as one alone.
  - **A GitHub tracker that names no repository infers one from the remote, so zero remotes is a tracker
    that cannot name an issue.** A write landing only in such repositories is not held (no ask, no deny) and
    the SessionStart text says there is nothing to claim. The read is `git remote` in the repository each
    target lands in, not the launch directory, so a no-remote checkout writing into a nested checkout that
    has one is still held. A failed read or a `.git` entry git will not use is not zero remotes. A
    non-GitHub remote still counts, because the URL is not read. Open: a refusal that survives there (an
    unplaceable write under `required`, a nested target the tracker can name) still names `claim`/`create`,
    which cannot run from the no-remote checkout. Held by `a_checkout_with_no_remote_is_not_held_for_an_issue`,
    `adding_a_remote_holds_the_same_write_again`,
    `a_named_repository_holds_the_write_where_the_checkout_has_no_remote`,
    `a_write_into_a_nested_checkout_that_has_a_remote_is_still_held`,
    `a_failed_remote_read_is_not_zero_remotes` and `a_guarded_checkout_with_no_remote_refuses_no_commit`.
  - **`watch` asks on Claude Code and on Pi's terminal UI only, and its answer cannot be read back.** Pi
    is asked through `estigia gate --runtime pi --can-ask` (exit `4`; the extension passes the flag only
    when `ctx.mode === "tui"` and shows `ctx.ui.confirm`), **unverified against a live Pi** (the `confirm`
    call is read from Pi 1.1.0's `docs/extensions.md` and `types.d.ts`); `rpc`, `json` and print modes are
    never asked. Every other agent is refused with the same code (an `ask` a host ignores is an allow), and
    the gate never learns whether the person approved, so it **asks on every such write**.
  - **A write the hook does not intercept is not held** (a tool no matcher wakes for, a script or formatter
    whose shell line names no git fragment, another clone), and **only a routine write, a boundary and a GitHub write are
    held**: a push is held as a commit is (`an_unclaimed_push_is_held_as_an_unclaimed_commit_is`), while an
    unclaimed run writing `.git/estigia/estigia.local.md` through a shell line is not stopped by this row and
    can set it to `off`.
  - **A write that lands outside every repository the gate can place is not a repository write.** The gate
    asks git once per such write (`rev-parse --show-toplevel`) and, where git names no working tree, walks
    the ancestors for a `.git` entry. **No entry anywhere** is outside every repository; **an entry git will
    not use, or an ancestor this process cannot read**, is no answer and the write is held. The walk is not
    git's, so a work tree named only through `GIT_DIR`, a ceiling directory, or a bare or mirror layout is
    read as a repository only where an ancestor has a `.git` entry; it costs one subprocess per such write,
    plus one `git remote` per landing repository for a GitHub tracker that names none.
  - **A write tool whose paths live in a patch body is placed by the body's own paths** (for `patch` and
    `apply_patch` only: `*** Add File:`/`Update File:`/`Delete File:`/`Move to:` lines; a write to **any**
    repository among them is held; no other write tool's body is read as paths, which a `pre_write_code` call
    with a `*** Add File:` line in its `content` regression-tests). A path that cannot be placed is read as
    inside.
  - **A shell write is placed inside only by its literal redirect targets or by naming a repository
    operation.** A redirect to a literal absolute path outside every repository is not held when the line is
    one plain printing command (`echo`, `printf`, `cat`, `ls`, `grep`, `rg`, `head`, `tail`, `wc`, `date`,
    `pwd`) with no quote, `$`, backtick, backslash, `<`, `|`, `;`, `&`, parenthesis, line break or `..`.
    **Everything else a shell line can write is unplaceable** (a `cd` before a redirect, a variable, `&&`, a
    second writer, inline interpreter code): refused under `required`; under `watch` run and compared against the
    checkout's recorded `git status` on the next call when launched in a repository, put to the person from
    an unestablished directory, and allowed from outside every repository; a symlink outside the checkout pointing in is not resolved. **Exactly one shape is added**
    (`shell::quoted_heredoc_target`): `cat > TARGET <<'DELIM'`, `cat >> TARGET <<'DELIM'` or `cat <<'DELIM' >
    TARGET`, a literal absolute `TARGET` without `..`, any body (**not read**; how an issue body is drafted in
    the scratchpad) and a closing `DELIM` line. An "any shape" relaxation was abandoned after five review
    rounds each found a case the reader modelled differently from bash (operator decision 2026-10-02).
  - **A named git write sent into another checkout is unplaceable, not inside** (`issue_requirement::git_directories`):
    when every git command on the line runs in a literal absolute directory — a leading `cd DIR &&` for the
    whole line, or `git -C DIR` on the command — and each lands outside the launch repository, the write is
    treated as one the gate cannot place, because the rest of the line (an interpreter heredoc beside a
    `git stash`) is not read. A second `cd`, `pushd`, `sh`/`bash`/`zsh`/`eval`, `GIT_DIR`, `--git-dir`,
    `--work-tree`, a git word inside a subshell or quotes, or a directory with a `$`, quote, glob or `~` keeps
    it inside. Not placed by this: a git command inside `"$(…)"`, which the classifier does not read as a
    write at all.
  - **A write inside an isolated checkout that a claim names is exempt, whichever session made it** (a
    delegated writer, such as an OpenCode child session deriving its own run, holds no claim of its own): the
    exemption applies when a pointer holding an issue has a `worktree` covering both the call's directory and
    the write's landing and differing from its `repo_dir`. It is path-based, so any session with that working
    directory is exempt, claimed or not, for as long as the pointer holds an issue (no expiry); an unreadable
    pointer grants nothing. Measured at the library level, not through `estigia gate` with a child
    `--session-id` or a live OpenCode host. The stand-down covers an `ask` as it covers a refusal.
- **`Claim workflow` `assisted` takes an issue through a local claim and publishes it as a draft pull
  request.** Every tool in `TRACKER_WRITERS` is refused `workflow-assisted-writes-no-tracker`, `estigia goal`
  answers no claimable work, and `assist` (offered only there) reads the issue and makes the isolated
  checkout without writing the tracker. The claim is a local run pointer that `harness::assisted` reads
  against the ownership marker inside the worktree and the branch it has checked out, so a pointer whose
  marker names another run, whose branch was switched or whose worktree is gone is refused at the next
  boundary or when the renewal window ends; the tracker's `verify-claim` is never asked. A write is admitted
  only inside its worktree on its branch. A write tool's absolute path is placed by that path, whatever
  directory the session runs in: inside the run's repository it is judged against the worktree, in another
  repository it is that checkout's, and anywhere else (a session scratchpad) it is outside the claim, and a
  refusal names that path. A shell write is placed by its first literal redirect destination when the
  line names one, as a write tool's path is: inside the worktree it is admitted, in the base checkout
  the refusal names that file, and outside every repository it is outside the claim
  (`a_redirect_inside_the_worktree_is_admitted_from_the_base_checkout`,
  `a_redirect_into_the_base_checkout_names_the_file_it_would_write`). A shell write with no placeable
  redirect names no path, so it is placed by the session's directory, and
  `cd <worktree> && sed -i …` from the primary checkout is refused `assisted-outside-the-worktree` naming
  where the call runs (`harness::assisted`,
  `a_shell_write_from_the_base_checkout_is_still_placed_by_where_it_runs`). A write tool whose paths live in its body and a shell command
  whose destination cannot be placed (`patch -p1 < x.diff`) are refused `assisted-write-unplaced`; delivery
  and `gh pr create` from a shell are refused with `draft_pr` named. `estigia release --run-id <run>` puts an
  assisted run down whether its issue is a number or a key: it forgets the pointer, writes no tracker and keeps
  the worktree, because nothing local reads whether a person delivered the branch
  (`releasing_an_assisted_run_forgets_its_pointer_whatever_names_the_issue`); `already-holding` from an
  assisted holder and the refused `release` tool name that command. What it does not hold:
  - **A shell GitHub write is refused `assisted-remote-write-unverified`**, whether alone or beside a write in
    the worktree (`an_assisted_run_reaches_a_pull_request_only_through_the_typed_tools`,
    `a_write_in_the_worktree_does_not_carry_a_remote_write_past_the_claim`). `gh pr edit` and `gh pr comment` have
    no typed tool, so in an assisted run they are a person's. Only a call deciding on this run's pointer is held: a
    call with no session id goes through `guard::adjudicate_command`, which still passes it.
  - **`git push origin HEAD` is refused**: the hook reads `HEAD` as the local ref
    (`git_hands_the_hook_head_and_not_the_branch_for_a_push_of_head`). A push is admitted only when every
    line has the run's branch as both refs; `branch:main`, a tag and a deletion are refused
    `assisted-push-unconfined`; a shell push names no refs to the `PreToolUse` gate.
  - **The renewal window rides a local check, not a tracker answer.** Only `harness::assisted` stamps an
    assisted pointer, after the marker and `git symbolic-ref HEAD` agreed, so a marker or branch changed
    inside the window is seen at the next boundary
    (`no_effect_but_the_assisted_decision_leaves_a_stamp_on_an_assisted_pointer`).
  - **The marker and the branch are bytes the agent can write**, an unsworn session in the worktree is not
    stopped for a routine write (`issue_requirement` reads the worktree as claimed, whichever session writes
    there; a `git push` from it is held by `Unclaimed write policy` wherever it runs), and an unclaimed shell
    write to the repository's own `estigia.local.md` can set `assisted`.
  - **One session can hold several assisted claims, one child run per worktree**
    ([`several-assisted-claims.md`](several-assisted-claims.md)). Measured on Claude Code 2.1.318 from the raw
    `PreToolUse` payload of real subagents, in a throwaway repository with a logging hook: a subagent's
    `session_id` is its parent's; it adds `agent_id`, distinct per subagent, and `agent_type`, its role; and
    `cwd` is the session's directory on every call, including the call after one whose command did `cd`. Only
    `isolation: "worktree"` changes it, to a fresh worktree under `.claude/worktrees/`. A `cwd` argument on the
    launch is accepted and ignored: the subagent's `pwd` printed the primary checkout. Parent and subagent
    calls reached one MCP server process, by its pid. So a shell call by a plainly launched subagent can only
    be placed by its own text, which is why one `git -C` is read. What this does not hold:
    - **A worker for A editing B by absolute path is decided under B.** The landing is attributed correctly, and
      the gate cannot see that the wrong worker wrote it. `agent_id` would tell the workers apart, and nothing
      ties it to a claim.
    - **A shell call physically inside one child's worktree is decided under that child** unless its line moves
      directory by a word the gate reads. `sed -i ../B/x` from inside A still lands in B under A's claim; a
      session in A always had this hole, and under fan-out it is a wrong-issue write.
    - **`git -C A <subcommand>` is judged under A and not read further**, so a subcommand that writes outside A
      by its own arguments is attributed to A.
    - **A child's pointer that cannot be read is nobody's child**, so a call landing there falls to the session's
      `Unclaimed write policy`. Under `off` that admits it.
    - **`git pull` is admitted inside a worktree whatever it names**, while `git merge` is admitted only for the
      run's base. The narrower reading is the second; nothing here proves what a pull brought in.
    - **The pointer lock is bounded**: a writer that cannot have it within two seconds falls back to the revision
      check, which can lose an update to another writer in that same moment
      (`concurrent_writers_do_not_lose_a_pointer_update` measures sixteen writers holding it).
  - **`assist` adopts an existing branch only through linkage the tracker records.** A branch origin already
    has is taken only when it heads exactly one open pull request that the host links to the issue: for a
    number, a closing reference on the pull request (`closingIssuesReferences`) or the branch in the issue's
    Development sidebar; for a Linear key, the pull request's URL among the issue's Linear attachments.
    Otherwise it is refused `assist-branch-not-adoptable`, and a failed read of the list, the sidebar or the
    attachments stops before any repository write. The free-text mention (`Refs #12`, a key in the title or
    branch name) was dropped as an admission path: anyone able to open a pull request could write it, and
    assisted pull requests never carry a closing keyword, which is why the sidebar link counts. What stays
    open: the author of the pull request or of the link is not checked, so whoever can link a branch to the
    issue can make it adoptable, and still only for a branch the operator names to `assist`; a branch
    published without being linked (a plain `git push` and `draft_pr`) cannot be taken again until it is.
    The sidebar link names a branch, not a pull request, and a closing reference is matched on the issue number alone, not on the
    repository it names. Measured by the `assist_*existing_branch*`, `assist_adopts_*`, `assist_refuses_*`
    and `only_a_closing_reference_to_the_number_closes_the_issue` tests in `src/transport/branch/tests.rs`, and
    by `a_key_is_adopted_only_when_linear_attaches_the_pull_request` in `src/transport/linear/tests.rs`.
  - **`draft_pr` publishes a draft and guarantees little else.** It pushes the pointer's branch
    fast-forward, opens a draft and reads it back as open, a draft and at the pushed head; it writes nothing
    to the tracker and adds no marker, receipt or `Refs` line. The team's own CI and reviews are theirs; a
    closing keyword is refused in the title, body and branch commit messages and not in a later host edit or
    commit; the branch is not checked to be the one named for the issue. An open ready pull request is
    handled as `Corrections after ready` says (`refuse`: refused; `redraft`: asked back to draft; `push-ready`:
    pushed to as it stands, so CI and reviewers see the new head at once), a draft that exists is reused with
    title and body unchanged (`"title_and_body_applied": false`), and a failed read-back answers in the
    `committed` world. It never creates a pull request open; under `Ready by: agent` with `Self review: off`
    it then marks the draft ready and reads that back, and a mark that fails after the push is
    `draft-pr-ready-not-read-back` in the `committed` world, never a call to publish again. Under
    `Self review: on`, `ready_pr` requires `reviewed_head` and refuses one that is not the published head
    (`ready-pr-review-not-this-head`). That is an **attestation, not proof**: no receipt binds a review to
    bytes, so nothing shows a review happened, in a context that did not write the change, or that it read
    the head it names; it stops a correction pushed after the review from opening unreviewed by accident,
    and a run that states a head it never reviewed passes. A recorded verdict replaces it once assisted
    runs have a review receipt. Held against a fake `gh` by `src/transport/draft/tests.rs` and the
    `draft_pr` tests in `src/harness/mcp/tests.rs`.
  - **`review_stages` answers `ci: none` on three conditions and a clock it cannot read exactly.** The
    rollup must be empty, the head must hold no `.github/workflows` file (one extra read, made once per
    head and only then) and no `check:` stage may be declared. GitHub exposes no time at which a head was
    pushed, so the 120 seconds are counted from the pull request's `updatedAt`: a comment or an edit
    delays `none`, never hastens it. A repository whose CI is external and reports late, with no workflow
    file, can read `none`; declaring that CI as a `check:` stage closes it. A bot with no check of its own
    cannot be waited for: only a declared `check:<name>@<login>` can be, so a repository with such a bot
    must declare it. Nothing makes a run call `review_stages` after `ready_pr`. The answers name it, and a
    `Stop` hook that holds the session open is not built. Held against a fake `gh` by
    `src/transport/stages/tests/no_ci.rs`.
  - **`draft_pr` counts corrections on the run pointer, so the count is only as durable as the pointer.**
    The first head a run lands is the original and each different head after it a correction, a pushed merge
    of the base included, because a merge can carry any edit and nothing here tells a clean one from another;
    the same head again is a retry and counts nothing. A new head past `Corrections per lineage` is
    `draft-pr-corrections-spent` before any host call. What it does not hold: `release` removes the pointer,
    so a run that releases and calls `assist` again starts at zero; a second run on the same branch from
    another machine has its own count; the first head of an adopted branch reads as an original although the
    branch already had a pull request; a failed call is counted from the checkout's remote-tracking ref, which
    a clone whose fetch refspec omits the branch does not update, and such a failure is not counted; and a
    pointer that will not write answers `corrections_not_recorded` instead of holding the next call. The way
    past it, raising the row with `estigia config set`, is refused to an agent by the gate's substring match,
    with the limits that matching has everywhere. Held by
    `draft_pr_counts_each_new_head_and_refuses_the_one_past_the_row` and its neighbours in
    `src/harness/mcp/tests.rs`.
  - **Coordination is left to the team**: nothing reads another person's claim, assignee or label, `assist`
    refuses a second run on one branch on this machine only, and Estigia cannot tell that the person, not the
    agent, named the issue.
  - **An unreadable contract is not treated alike by every tool.** `GateContext.workflow` is `None`; `assist`
    is refused and an assisted pointer is `assisted-claim-not-in-force`, but tracker writers are not refused
    (that would stop every tracker write in every repository whenever one unrelated row was malformed), so a
    broken contract in a repository whose row says `assisted` leaves tracker writes open until it reads again
    (`an_unreadable_contract_names_no_workflow_at_all`,
    `an_assisted_only_tool_is_refused_unless_the_row_says_assisted`). This is the contract chain — the
    versioned block, the operator's `estigia.local.md` and an agent's own file — and not the repository's own
    file: one that is there and cannot be read reads as the denying answers (`required` for `Unclaimed write
    policy`, `unreadable` for `Git operation window`), so the gate refuses what those rows could have refused,
    and the refusal that stops an unclaimed write names the file and the read error.
  - A test classifies every tool in the server's table as tracker writer, assisted-only, local writer or
    read, so a new tool fails until placed; a writer a person runs by hand through `gh` is not a tool call.
- **The screen's door and event loop are unmeasured.** `the_screens_install_proceeds_on_a_source_build`
  drives `guided_install`, which always consents a source build (a person confirming the screen is that
  consent), and nothing asserts that `dispatch` does not put a lifecycle preflight back in front of `guided`;
  `every_production_caller_of_install_planned_is_preceded_by_the_lifecycle_preflight` is a text guard that a
  `#[cfg(test)]` call, macro, function pointer or `use` alias escapes. The screen writes three things the
  lifecycle gate covers none of (the guard key's `pre-push` hook, the language key's remembered tongue, the
  folder picker's directory: `docs/lifecycle.md`), and `run` needs a terminal, so that it passes
  `app.repository` to `install_guard` and `report_of_checks`, keeps a guard refusal apart from an install
  refusal and calls `Reads::refresh` before each key are readings. `wrapped_rows` in `src/tui.rs` emulates
  ratatui's word wrap (`Paragraph::line_count` sits behind `unstable-rendered-line-info`), checked only
  against Help in both tongues and one synthetic long row.
- **The legacy-checkout stop cannot name the legacy path of a template that named no placeholder.** A
  `Worktree location` missing `<branch>` or `<run-id>` gains it in memory, and `claim` with `branch` and `base` stops with
  `legacy-worktree-registered` when the pre-migration checkout is still registered, computed from the raw
  template. For a bare directory naming **neither**, that raw path is one no build created (they appended
  `~<run-id>`), so a run resuming such a branch is stopped by `branch-checked-out-elsewhere` instead, before
  any remote state is written and with no data lost; the recovery is the same `git worktree remove`, written
  in the binding. Measured across twenty-four template shapes by the blind reviewer of receipt
  `6b192e0ee94f11004b621c06c8e8e5dd`.
- **Five of the twenty-one are about the past.** `LOOKS_BACK` excludes `silence`, `stale-run-pointer`,
  `prune`, `gate-version` and `store-records` from readiness: undecided past calls, another run's
  pointer, orphaned branches, a stale diagnostic binary and appends that already failed do not prove
  a fresh claim impossible. `store-records` reads counted append failures; a successful snapshot
  deletes the `snapshot` row, and a counted failure whose last occurrence is older than the thirty-day
  record retention is not broken and the daily pass deletes it. A row whose last occurrence the store
  did not record stays broken, because its age is unknown and an unknown is not clearance. The store
  row is not exempt: an unreadable authority prevents claiming now. This classification is a policy boundary,
  not proof that the historical problems were repaired.
- **The claim and review families decide from the local workflow store and write only to it; GitHub carries
  labels, the assignee and human prose.** First touch imports through `gh issue view`, refusing
  `import-diverged` for differing holder/latest publication. Open-held renewal costs two REST calls,
  held by `a_gated_write_on_an_open_held_issue_makes_no_graphql_call_and_two_rest_calls`; closed-issue
  delivery proof uses the stored receipt, held by
  `an_own_delivery_close_is_proved_from_the_store_when_github_carries_no_marker`. GitHub projection
  follows local commit; the stand-in rejects posted `issue-flow:` bodies. Closing-reference,
  projection-convergence, claim assignee/label and handoff label reads still use GraphQL. Limits:
  - **After the first touch only a person's stand-down or adjudication is admitted from GitHub**, from
    comments **whose markers are all `standdown` or `adjudication`, written by the authenticated user and
    never edited** (`after_the_first_touch_only_a_stop_is_admitted_from_github_comments`). An operation-id
    claim needs `adjudication`; comment stand-down cannot carry its scope. Later edits, labels and
    assignees are not reimported. GitHub `created_at` and local marker clocks can tie; insertion order
    may differ across stores. Sync fails at 10,000 comments. REST trust requires authenticated-login
    equality and flags any differing created/updated timestamps, stricter than GraphQL.
  - **A lost claim race rolls back; a lost reclaim race still commits** (`reclaim` adjudicates after its
    transaction). A `dev:` label that cannot be made after the store accepted the claim is `write-failed` and
    the retry runs the projection again. The state compared is the store's, set at import from the single
    `status:` label (two or none import as no state: `unexpected-state` until a `transition`). The
    differential check compares the holder and the latest publication, and nothing else.
  - **An older binary on the same issue cannot see these markers**: it reads GitHub only, may claim an issue
    this binary holds, and its merge gate may clear a head this binary has not reviewed. Nothing detects the
    mix; the cure is reinstalling every agent profile before an issue is in flight on both. A person
    reading the issue page sees no claim or receipt comment, and there is no single chained lifecycle test.
  - **`create`'s store admission is recoverable only by title.** If admission fails after filing, a repeat
    searches `gh issue list --search "\"<title>\" in:title"`, adopting a single match, refusing two and
    filing anew on none, so a search index that has not caught up (a lag nobody measured) would let the retry
    file a duplicate.
  - **The queues decide from the same store.** `list-state`, `list-identities` and `completed_count` read the
    open issues from one REST list cut to the oldest 200 **open** issues across every state (a state whose
    items are all newer than that cut is not seen), decide state, holder and dedup key from the store (an
    open issue the store never admitted is `unadmitted`, reported by `estigia goal` as `null` when the read
    failed, never `0`), and read the review queue's per-candidate timeline without the sync a ledger open
    pays. `estigia store import` admits untouched open issues oldest first.
  - **The dedup identity, the `blocked-until` clauses and the task checklist live in the store and nowhere
    else**, so a lost `state.db` loses all three. `transition` records a clause without requiring a claim; a `task` is recorded, not
    verified (nothing checks its commit exists or its `evidence` note names anything real); `transition --to
    done` answers nothing to verify when the issue has no confirmed publication. FLOW's routing rules in
    `skill/SKILL.md` are instructions, enforced only by `task`'s `done` rule, and their effect on cost is not
    claimed (`docs/review-cost.md`).
- **The store is one copy, and its protection is local and partial.** Losing `~/.estigia/state.db`
  loses store-only identities, clauses, tasks, in-flight claims and review history. Reimport cannot
  recover what GitHub never held, even while its projection still names the run. On the write path,
  `VACUUM INTO` takes a snapshot when none exists or the newest is at least 24 hours old, keeping
  three in `~/.estigia/snapshots/`. Failures are counted with when they last happened and the reason
  the last one gave, not fatal; a later successful snapshot clears the snapshot row. Restoration is manual.
  A store about to migrate is copied first, in its old schema, and the migration is refused when the
  copy fails ([lifecycle](lifecycle.md)); a write another process lands between that copy and the
  migration's lock is in neither. A store mid-migration looks like an older one, so openers racing a
  fresh store or an upgrade may each stage a copy; each is named for the schema read back from the
  copy, one already in the build's schema is dropped, and pruning keeps the newest per schema, which
  can leave copies of intermediate schemas nobody shipped
  (`many_openers_of_an_older_store_all_open_and_its_old_schema_is_kept`).
  Doctor reports age, not a verified restore. Append-only triggers do not resist an OS user
  able to drop them. `store-unwritable` rejects detected network mounts on macOS/Linux, not Windows.
  `foreign-projection` refuses unexplained GitHub owners across stores; reclaim overrides it.
  The best-effort `gh` counter counts spawns, not cost, misses other threads/undispatched operations,
  and silently drops counts on open/insert failure or a store busy beyond 200 ms. No test here proves
  restoration, Windows mount detection or counter completeness.
- **A run that swore nothing is never gated, including when Estigia is broken.** Failing closed on everybody
  would be a lock rather than authority and would teach people to remove the hook: a missing transport denies
  a sworn run's write and lets an unsworn one through. **An unreadable run record stops a write**: a pointer
  that exists and will not read says a run existed and not what it swore. `session::load` treats only a
  `NotFound` read as absence, and only while the pointers' directory is one (Windows answers a path whose
  parent is not a directory with `ERROR_PATH_NOT_FOUND`, so a state root that is a file would otherwise read
  as *every run swore nothing*); other failures load as unreadable and name the path and error. The push
  guard denies `run-pointers-unreadable` when nothing readable covers the checkout and some holding is
  unreadable, and a readable holder that covers it makes the unknown one change nothing.
- **A contract row the gate cannot read costs that row, and no longer the rows beside it.** The gate reads
  the installed contract with a fallback to defaults rather than refusing, because a file that will not parse
  is a problem `doctor` reports, not a reason to stop an agent mid-edit; the fallback takes the renewal
  window to zero and keeps every row that parses
  (`one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it`; salvaging can only narrow, since
  `Config::default` is the loosest configuration). A *boundary* row that will not parse still costs the
  boundaries it named (`an_unreadable_contract_costs_a_declared_boundary_its_boundary_status`), the one place
  where *configuration may only tighten* runs backwards.
- **Every script this crate writes reads the binary's answer instead of inheriting it, and a script that
  cannot get one lets the call through with a word.** Only `1` (a refusal) and `2` (an outcome it could not
  read back) are decisions; anything else (a missing binary at `127`, a panic at `101`) means it did not
  answer and the write or push goes through (*"a hook that breaks does not deny"*). The OpenCode plugin holds
  the three codes (`the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer`) and a result with no numeric
  exit code is let through with a word (`the_plugin_says_so_when_the_gate_returns_no_exit_code`). The cost:
  on a machine whose binary has gone, every gated write goes out unmeasured, saying so on standard error;
  failing closed would be a harness people uninstall the first time it is wrong at a bad moment.
- **Nothing here resists a local actor running as you.** The gate reads a run pointer, a contract and a hook
  command, all files in your home with your permissions; anything able to write them can rewrite what Estigia
  believes, and a replaced binary answers whatever it likes. No tamper-resistance is claimed: the threats are
  accidental (a truncated write, a crash mid-rename, two processes racing on one pointer). **A stale writer
  is dropped, not merged**: a pointer on disk written more times than the one in hand is news the caller has
  not seen and its store is skipped (the fail-closed direction), without reconciling two concurrent edits or
  reporting the dropped one.
- **The boundary list is compiled, and can only be added to.** Estigia knows git and GitHub, not that a
  repository ships `npm publish` or `terraform apply`, so `Irreversible commands` lets an operator name
  those as boundaries (no renewal window, a fresh read every time). It **only adds**: no value removes a
  built-in, because a setting that can make the gate looser is not a guard rail. Nobody can express a rule
  the classifier has no shape for; matching is substring, on a normalised command line whose
  provably-data spans are blanked only in read-only simple commands, alone or as the top-level commands of
  a compound line through which no data reaches execution.
- **Decisions are written down; the reasoning behind them is not.** Every allow and deny is appended to the
  local store's `decisions` table with the run, the tool, **what the decision was about**, the verdict, the
  refusal code and the repository it was made in — `github.com/owner/name` when the checkout's `origin`
  names one, otherwise the main checkout's path, and nothing when the directory is in no repository. A call
  identified only by `--run-id` — `estigia gate`, the question a person or an agent
  asks — answers and records nothing and touches no run pointer, because that road is how the gate is
  probed and it once wrote 75% of every recorded denial. `Outside` is not recorded (a record per tool call of every session that never swore would bury
  the ones that matter) and the write is best-effort (a refusal that could not be written down is still a
  refusal). **A payload Estigia cannot parse does not stop the call, and says so**: the hook stands aside
  (a schema this build does not know could wrap `Read` as easily as `Write`) and writes `payload-unreadable`
  to the ledger. Records older than thirty days are deleted at most once every twenty-four hours, by
  `SessionStart` only; each repository also keeps at most ten thousand decisions, newest first, and the
  undecided verdicts `doctor` reads are never removed by that cap, only by age. A record that could not be
  appended is counted (itself best-effort) and reported by
  `doctor`; the JSONL history this replaced was imported then deleted (a torn legacy line is skipped, so
  whether every record survived is not measured). `status` lists what this machine's runs recorded holding;
  whether those claims are live is the tracker's answer.

## The populations and the prose they cross

- **Every population declaration is found by walking `src/`, resolved to the item its `subject` names,
  and fingerprinted together with it.** A declaration is a `pub const … : Population` (the type in
  `src/harness/population.rs`, each const beside the code it describes), read by `tests/guards.rs`
  with `syn`. The sources are read off the directory rather than listed, and a `subject` naming no real
  item in the same file, or more than one, is refused loudly. Changing the rule or the code beneath it
  reopens the claim: `tests/guards.rs` fails and names the new fingerprint, and somebody has to read the
  declaration again before recording it. That proves the pair has been read since it last moved, not
  that any declaration is true, and not that every guard needing one has one.

  **A list is a population; the matcher is a decision.** The declaration on `CONTROL_SURFACE`
  fingerprints the list, so changes to `is_anchored_fragment` once moved which paths the gate watched
  with this registry green. `CONTROL_SURFACE_CORPUS` is the matcher's subject: a fixed corpus of paths
  with the answer both roads must give each one, asserted by
  `the_control_surface_matcher_answers_its_corpus`. The corpus is representative rather than exhaustive.

  **Where the walk reaches, and what catches the rest.** `collect_found` reads top-level items,
  associated consts inside `impl` blocks, and the bodies of inline `mod`s, but does **not** walk
  statements, so a `const … : Population` inside a function body is invisible to it.
  `no_population_declaration_hides_from_the_walk` counts the type ascription in each file's
  comment-stripped text and fails, naming the file, when the text writes more declarations than the walk
  found. **Two spellings still escape both ends**: the ascription both counters match is one constant,
  `harness::population::DECLARATION_ASCRIPTION`, a contiguous substring, so `: population::Population =`
  does not contain it, and a declaration behind a type alias (`type Pop = Population;`) is invisible to
  the counters *and* to `is_population_type`. Neither is a plausible accident and neither is closed.
- **The seam guards read two shapes, not every shape.** They resolve markdown links and `<skill>/`
  rooted command lines. A payload that names a file some third way would slip past both.
- **The directive's rules are crossed against the gate by name, not by reading.** The text every agent
  carries in context states three rules, and each is pinned to a named test that holds the gate to it:
  a rule deleted from the prose, or a test renamed out from under one, fails the build. What no test
  can check is whether the sentence still *means* what the test enforces; that reading is a person's
  job.
- **The transport and Estigia read the operator table the same way, and the crossing that held it is
  deleted.** Both read `SKILL.md` between the same markers (`estigia:config:*`, with the superseded
  `issue-flow:` pair still read), normalising a hand-written cell (backticked token, `**bold**`, an
  explanation after an em dash or in parentheses). Nothing now crosses the two readers over the same
  bytes; `a_row_the_binding_acts_on_is_a_row_this_reader_finds` holds one side.

## Ownership

- **The runtime is still folded into a forced takeover's evidence digest, and the retry that would read
  it cannot be reached.** `forced_reclaim_hash` in `src/transport/ownership.rs` retains runtime for wire
  compatibility: removing it would invalidate existing evidence and change historical ownership.
  The fold recomputes from the marker's own runtime, so it binds to itself. Caller-runtime comparison
  in `resumed_reclaim` requires `Takeover::Write`, but an accepted marker first answers `already-yours`.
  Measured: a `runtime=claude` forced marker under a Codex-derived id, retried with the same operation,
  horizon and target, answered `already-yours`, not `reclaim-metadata-mismatch`. Reaching the latter
  requires a discarded marker still targeting the current `from-op`; no writer produces it. No test
  demonstrates that hypothetical retry.
- **The gate's wait on the transport is bounded at fifteen minutes, and what comes back is `Unknown`.**
  `Command::output` returns when the child does, and the transport bounds only *some* of its own `gh`
  calls, so a call without a timeout held the gate open as long as the socket stayed open (a `cargo test`
  against a transport that sleeps was still running ten minutes later). The bound is far past anything a
  working transport reaches (its own longest documented wait is two minutes), and the answer is never
  "nothing happened", because the command may have posted a comment or opened a pull request before it
  stopped. For the same reason the **next interpreter is not tried**.
- **`setup --all` keeps each agent's own configuration, and one residual is read, not run.** The
  configuration is read from the skill root it is about to be written to. A root with no contract
  inherits **per row**: an `Everywhere`- or `Machine`-scoped setting every installed contract agrees on
  is folded onto the portable defaults, and a row they disagree on (or every row, when nothing else is
  installed) stays at the default, and the run says which rows those are. `Agent`-scoped rows are never
  inherited from a neighbour. The population is every *installed* adapter unioned with the run's own
  targets, and the value inherited is `skill::contract_config`, never the layered value, so a
  machine-local `estigia.local.md` override cannot be promoted into a **different** adapter's versioned
  and shared contract. **The residual:** `doctor`'s `canonical` row compares two **layered** views
  (`installed_config` against each adapter's `installed_config_for`) on every row whose `scope()` is not
  `Agent`, so on a machine whose `Everywhere`- or `Machine`-scoped answers live in a per-root
  `estigia.local.md` rather than in the contracts, a newly registered adapter's contract will not match
  and `canonical` reports it `BROKEN`. Not a regression (that machine was already `BROKEN` before any
  adapter was added, and `estigia config set` without `--agent` writes every installed **contract**
  directly, which cleared the reported case); closing it belongs to `canonical`'s comparison or to
  `config set`.

## Reachability, registration and the rounds around them

- **The integration exemption proves *a* branch is checked out where the merge runs, not that it is the
  issue's own branch.** `is_integration_in_the_runs_own_checkout` checks `run.worktree` alone, an
  unredirected environment and any `refs/heads/*` symbolic ref. `claim` returns a branch but the
  MCP pointer effect retains only worktree; the branch remains in `ownership_path`'s private git file.
  `checkout`/`switch` are not observed. Git prevents checking out a branch held by another worktree,
  not trunk after the base moves away, `develop`, `release/1.x` or another feature branch. No guard
  distinguishes these from the issue's branch. A direct transport call also bypasses pointer effects.

  `integration_environment_does_not_redirect_the_repository` refuses `GIT_DIR`, `GIT_WORK_TREE`,
  `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_INDEX_FILE`,
  `GIT_NAMESPACE`, `GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM`, `GIT_CONFIG`,
  `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_COUNT`,
  `GIT_CONFIG_PARAMETERS`, `GIT_SHALLOW_FILE`, `GIT_REPLACE_REF_BASE`, `BASH_ENV`, `ENV` and
  `BASH_FUNC_git%%` by exact case-insensitive name, plus `GIT_CONFIG_KEY_*`/`GIT_CONFIG_VALUE_*`: a list,
  not an exhaustive proof. `GIT_EDITOR`, `GIT_PAGER` and `GIT_ASKPASS` are permitted. The config family
  can redirect worktrees or hooks. `is_safe_local_fast_forward` shares the list. The eventual merge
  runs after this process exits; repository/ref/worktree/PATH races remain
  (`local_fast_forward_proof_fails_closed_on_repository_state`,
  `an_exact_local_fast_forward_is_allowed_without_widening_other_merges`).

  **An integrating merge spends no verdict, and it is read in the directory it names.** The shapes are
  exactly `git merge [--no-edit|--no-ff|--ff|--quiet|--continue|--abort] [<ref>]`, alone, behind one leading
  `cd <dir> &&`, or as `git -C <dir> merge ...` with `<dir>` a plain path (optionally in one pair of quotes,
  not starting with `-` or `~`, or a drive-lettered `X:/` path; a relative one is taken from the session
  directory). It is allowed when the directory is inside `run.worktree`, the environment does not redirect
  Git and `HEAD` there is on a branch. The same line in another checkout, `--ff-only`, a second command, a
  redirect or an expansion, `git tag` and `gh pr merge` stay deliveries. A line that changes directory any
  other way (`cd $VAR`, a second `cd`, a subshell, `pushd`, `--git-dir`) is refused
  `review-checkout-unreadable` (class `unavailable`) once a review is recorded, for every git command in
  `DELIVERS`. **The run's own merge commit (#58)**: the prescribed `git tag -a "<tag>" -F "<notes-file>"
  "<merge-sha>"` and the `git merge --ff-only origin/main` that brings the base checkout to the delivered
  merge act on a commit that is not the reviewed head, so that commit is admitted when it is a two-parent
  merge whose second parent is the receipt's head and whose first parent descends from its base
  (`ReviewTransaction::delivered_by_merge`, shared with `match_merge`); a tag line is read only when plain
  words (`tag_target`) and a tag that names a commit is judged on that commit. Not covered: `git -C <dir>
  merge --ff-only ...`, `cd <dir> && git merge --ff-only ...`, a merge whose base the checkout cannot
  resolve (refused), and a squash or rebase merge (no second parent).

  **The near-miss diagnostic stops at a JSON string.** `integration-shape-not-exact` names the accepted
  `git -C <worktree> merge --no-edit origin/<base>` in `in-progress`; it never admits. But
  `shell_command_argument` sets `literal` only for one normalised command key holding a JSON string.
  An argv-array `git merge origin/main` still refuses `out-of-phase`. This is fail-closed; adapters
  sending arrays have not been measured.

- **Nothing counts the rounds a change takes, and nothing bounds one that is not converging.** The
  two-round limit is gone: `skill/policies/blind-judges.md` carries no count and no path reaches `blocked`
  because one ran out. The judgement triggers (the put-down row of `skill/references/decision-gates.md`: a
  known mistake repeated, three hypotheses without narrowing, more editing than measuring) are prose a run
  applies to itself. A claim's horizon plus the renewal window bounds a run that has stopped reporting, not
  one reporting diligently on its ninth publication. What stops an unconverging change is a person noticing.

- **ZCode registers its instruction file and its MCP server; three things remain unverified.**
  ZCode CLI 0.16.5 logged `mcp.server.connected` for `mcp.servers` in `~/.zcode/cli/config.json`, with no
  fallback file present. The shared skill and `~/.zcode/AGENTS.md` follow its configuration reference.
  Remaining limits:
  - The stated fallback loads `~/.agents/mcp.json` only when the scope has no servers. Adding Estigia
    could hide every fallback server while setup/status report success; this was not run.
  - `is_now_empty`/`discard` can delete the whole config after `strip_zcode_mcp` leaves only empty/null
    values, e.g. `{"hooks": {}}`. A nonempty `hooks.enabled` preserves it; unchanged files are untouched.
  - `~/.zcode/skills` is documented but unverified from shipped source, so the neutral root is used
    and that tree remains ungated. Hooks require global `hooks.enabled`; denial and payload are
    unverified, so no tool gate is installed (`gate_gap`).
  `every_control_file_an_adapter_has_is_one_the_gate_measures` crosses `.zcode/agents` and the MCP
  path; `a_host_read_root_carries_the_same_authority_as_the_directive` crosses the neutral fallback.

- **DeepSeek Harness registers, with its skill root and instruction target verified.** The adapter installs
  the skill into `~/.dsh/skills` (the harness's `user-dsh` root, from its `skill-filesystem` provider) and
  fences the directive into `~/.dsh/AGENTS.md`, both from the harness's own shipping source. `$DSH_HOME` is
  read and contained as `XDG_CONFIG_HOME` is (an absolute value under the call's home is honoured, one
  outside is dropped for `~/.dsh` with a per-adapter note). **No MCP registration** (where an operator
  registers a server has not been crossed against a running client, so `paths_in` answers `None`), **no
  tool-call gate** (whether a plugin can **deny** a call like OpenCode's `tool.execute.before`, and under what
  payload, has not been crossed against a shipped harness, so it is contract-only and nothing checks the
  agent honoured the directive) and **no role/sub-agent definitions** (`agents_root` stays `None`, yet the
  tree is gated by a **prefix**: the extension-trimmed fragment `.dsh/agents` reaches `~/.dsh/agents/**`
  only because it is right-unanchored, and narrowing it to `.dsh/agents.md` would drop the tree to
  `Routine`; the `~/.dsh/agents/reviewer.md` row in
  `a_host_read_root_carries_the_same_authority_as_the_directive` pins that). The fragments `.dsh/agents` and
  `.dsh/skills/` are crossed by `every_control_file_an_adapter_has_is_one_the_gate_measures`.

- **A workspace install covers the runtimes whose project-level files could be read from the runtime's own
  reference, and refuses the rest.** `setup --scope=workspace` writes each adapted runtime's files into a
  checkout (destinations: [`what-it-writes.md`](what-it-writes.md)); ZCode and DeepSeek Harness have no
  verified project-level destination, so a workspace install refuses them by name and a workspace
  *uninstall* of them (or a retired adapter) is a no-op
  (`a_workspace_install_needs_a_workspace_and_a_verified_adapter`,
  `a_workspace_refusal_names_the_command_that_clears_it`,
  `a_workspace_install_writes_under_the_workspace_and_nowhere_else`,
  `a_workspace_uninstall_takes_the_skill_back_from_one_shared_root`). **Not covered:** that each runtime reads
  every path in the table was read from documentation, not crossed against a host; Codex's project
  `hooks.json` is reported by an open upstream issue as not loaded where `.git` is a file (a git worktree);
  Cursor's project `hooks.json` has a stricter schema than the user-level file (a required top-level
  `version`), read from a community issue, so whether the workspace hook loads is untested; both
  project-level gates load only in a project the runtime trusts, which nothing reports. In a workspace the
  runtimes that read `AGENTS.md` share one instruction file and one skill root
  (`the_runtimes_that_share_an_instruction_file_share_its_skill_root`), so the shared directive and skill go
  with the **last** adapter the record names, and the block names whichever adapter wrote it last, so the
  per-agent override file it points at may belong to a runtime no longer installed (nothing asserts that
  file). A record is found by the digest of the checkout's canonical path, so a moved or renamed checkout is
  one no record names, and a fresh clone carrying an install in its files carries no record. The record is
  read, changed and written without a lock (two runs against one checkout can lose one adapter's name) and
  is written before the adapter's files (a failed `setup` leaves the adapter named). `is_configured` answers
  "not installed" for every adapter when the record cannot be read, the one reader that turns this unknown
  into an absence: the TUI uninstall reads `read_configured`, which keeps `Unknown` apart
  (`uninstall-not-read-back`), but the CLI `uninstall` output, `sync` and the status listings still turn an
  unreadable file into "not configured", and in the workspace scope `read_configured` takes an unreadable
  record as absent. `workspace_installed_adapters` answers `None` for a record it cannot read, an install
  that would merge into one refuses, and a record whose checkout is `NotFound` reads as gone. `setup --all
  --scope=workspace` was observed once to stop with a refusal naming the two adapters it cannot place after
  the five writable ones were done; no test drives that CLI path.

- **A global install is found through a record of its roots, and only installs made after the record
  existed are in it.** `forget_state` keeps the store while any recorded root, under
  `<home>/.estigia/roots`, still holds an adapter, and when a record cannot be read. An install by a build
  that did not write the record, under a `CLAUDE_CONFIG_DIR` other than the uninstall's, is invisible until
  `setup` runs again under that root. The record holds the Claude, XDG and `APPDATA` roots, not `CODEX_HOME`
  or any other root `Environment::resolve` does not read. The command line names the record on its own
  `create`/`remove` lines after a real run; `--dry-run` names neither. Held by
  `uninstalling_one_claude_root_keeps_the_store_the_other_root_reads`.

## What the gate narrows, and what it records

- **`observed` is attached on more roads than it is rendered on, and the register naming what is still
  missing has twice claimed more than the code does.** A gate refusal carries a structured
  `Refusal.observed` account of what the gate parsed: the command fragment matched and the checkout the
  call was adjudicated against. `gate_observing`/`decide_observing` receive the door's own reading and
  `prefixed` attaches it to every `Deny` they produce; `gate`/`decide` receive nothing (`decide` calls
  `decide_observing` with `&Observed::default()`), and `prefixed` emits `observed: None` rather than an
  empty object for an empty reading. `guard::adjudicate_action`'s single-holder arm is on this second
  road. `cli::show_gate`'s session-less branch passes the `Decision` through `report_gate`, whose `Deny`
  arm attaches the door's reading to any refusal that carries none, closing the gap for that caller
  under `--json` for every refusal `adjudicate_action` can produce there
  (`complete_review_receipt_not_selected`, `run-pointers-unreadable`,
  `several-runs-hold-this-checkout`, all via `Refusal::not_started`, which never sets `observed`).
  **Attached is not rendered.** `report` writes `Display`, which never mentions `observed`, unless `json` is
  `true`, and `src/setup/plugin.rs` invokes `${ESTIGIA} gate ${tool} --input ${payload}` without `--json`, so
  the one shipped agent that drives the session-less branch never sees the account. What remains open:
  - **The push road carries nothing.** `cli::git_hook`'s `PrePush` arm calls the same
    `adjudicate_action` through `guard::decide` but answers its `Decision::Deny` straight to `report`
    and never reaches `report_gate`, so every refusal it can answer with carries nothing, and nothing
    can top it up. Closing it needs two more readings: `session::holdings` drops the path it read once
    the run is built, and `guard::holders_of` discards every coverage depth but the winner's.
  - **`show_gate`'s own early failures carry nothing**: `gate-input-not-json` fires before `parsed`
    exists, and `working-directory-unknown` and `home-not-resolvable` can propagate out of
    `context_the_payload_names` before the `CheckoutObserved` half is built; each reaches `report` as a
    bare `Refusal::not_started`.
  - **The MCP road builds no account and adjudicates a checkout anyway**:
    `run-id-names-another-checkout` in `src/harness/mcp/mod.rs` compares `context.repo_dir` against
    `run.covered()` and answers through `Refusal::not_started` directly.
  - **The receipt-comparison half**: nothing reports what a delivery refusal actually compared a stated
    PR or head against (`delivery_pr_mismatch`, `stale_verdict` carry their own prose).
  - **No hook dialect renders it, and a role-refused `PreToolUse` never builds it.** The `PreToolUse`
    arm in `src/harness/hook.rs` builds the full account only for a call that clears
    `authorize_review_blind_launch`, the reviewer's own `roles::gate`, `roles::definition_for`'s `Err`
    arm and the named-agent `roles::gate` check; for a call that does reach the build, `Dialect::deny`
    emits only `deny_reason` prose for all five `Dialect` variants (`claude-code`, `gemini-cli`,
    `cursor`, `exit-code`, `cline`). Left open deliberately: a hook's answer belongs to the *host's*
    protocol, and a sibling structured key inside somebody else's schema is a decision with its own
    tradeoffs.

- **The clamp `narrowed_by_the_call` applies trusts one thing beyond `launched`: a pointer a claim
  already wrote.** `Worktree location` on a real install is a **sibling** of the repository it was
  minted from, never a child, so a call naming that worktree could never resolve through a sibling-blind
  clamp, and a run that had claimed, published and released CI could not deliver (`estigia gate`
  adjudicated `gh pr merge` against the launch checkout on `main` and answered
  `verdict-bound-to-other-bytes`). The invariant this repository runs on: several tasks per agent and
  several agents are fine, but a worktree holds exactly one task, so the worktree is the unit of task
  identity and `repo_dir` is not (several runs sharing one launch checkout is the *designed* state).
  `held_by_a_pointer` honours a directory when it lies inside `launched` **or** is a checkout a pointer
  records, drawn from `worktree` alone, so neither road reads `repo_dir`. A directory no pointer reaches
  is not admitted, and `checkout_root_covering` never climbs above `launched`
  (`checkout_root_covering_never_climbs_above_the_launch_directory`).

  With a run id (`--run-id`, a session id with `--runtime`, or ambient `ESTIGIA_RUN_ID`),
  `held_by_a_pointer` is restricted to that run's own `worktree`, so a call naming run B's worktree gains
  nothing and is decided against `launched` under the run id it carried. Without one, `named` goes through
  `harness::guard::holders_of`, restricted to a single holder whose own `worktree` covers it (never one whose
  `repo_dir` merely is an ancestor). What a run-id-less call cannot do is tell one caller from another, so it
  cannot refuse a call that *reaches into* another task's worktree: that flow violation is not enforced on
  this road, every call with no identity takes it, and `stale_verdict` reads `context.repo_dir`, so a
  delivery boundary the call takes is adjudicated against that other task's verdict binding. An operator who
  exports `ESTIGIA_RUN_ID` for a calling road that does not set it closes this without a code change; the one
  shipped adapter that invokes `estigia gate`, the OpenCode plugin, forwards `--session-id <session>
  --runtime opencode` and `gate_identity` derives the run id through `session::run_id`. The forwarded identity
  cannot establish that the claim was sworn under it: a claim sworn under a hand-minted id
  (`opencode-f2ae8c`) is not found by the derivation, the gate loads a fresh run and answers `outside`
  (permitted, no ledger line), so every action from such a run, delivery boundaries included, is permitted
  rather than judged. The derivation's pinning is prose, not enforcement.

  **Three readers of the resolved `repo_dir`, not one.** `launched_context` sets the field from
  `gate_context(&looking_at)` after the narrowing, so a model-written `workdir` or write target that a
  pointer records reaches all three:

  | Reader | What it decides from `repo_dir` | Where |
  |---|---|---|
  | Installed configuration | `tracker`, `boundaries`, `evidence`, `integration`, `window` | `gate_context` in `src/cli/mod.rs` — `skill::installed_config_in_keeping_what_parses(&skill_root, &repo_dir)` |
  | Role definition | which agent allowlist applies to the tool | the gate path in `src/cli/mod.rs` — `harness::roles::definition_for(&context.repo_dir, …)` |
  | Verdict binding | whether a delivery is bound to the bytes under review | `stale_verdict` in `src/harness/mod.rs` |

  **Cross-repository resolution is admitted when a pointer records the worktree.** A worktree path under
  `Worktree location` can belong to another clone on the same machine; if a live pointer names it,
  `held_by_a_pointer` honours it as it honours a sibling worktree, and nothing asks whether `repo_dir`'s
  git identity matches `launched`'s. That is the invariant as written for task identity, and it means
  the other repository's `.estigia/` rows and role definitions decide the call. No test drives a second
  repository's pointer through the identity-less road. **A corrupt pointer can remove a holder from
  consideration on that road**: `holders_of` walks every pointer through `session::holdings`, whose
  `pointer_at` answers `None` for a pointer that will not parse (unlike `session::load`, which loads it
  as `unreadable: true`), so a directory two holders cover at equal depth can answer down to the one whose
  pointer still parses. No test drives it. This is the `estigia gate` CLI door only; the `PreToolUse`
  hook road builds its `GateContext` from the payload's own `cwd` and reaches neither
  `narrowed_by_the_call` nor `checkout_root_covering`.

## What a measurement reconstructs

- **The review-cost measurement does not close what it was launched to answer.** No counterfactual
  quantifies the rework, claim races or unreviewed delivery the panel avoids. The two 2026-08-30 trials
  in [`review-cost.md`](review-cost.md) have these limits:
  - The first changed its definition mid-run; eight judges lacked the severity vocabulary. It ran the
    external rubric inside this host's `analyst`, not that project's binary, with two judges per arm,
    no variance or retained artifacts, on an already-merged PR with no receipt. The 13.7% brief effect
    and +18.2% summed judge time cannot be separated from sampling.
  - The second ran the external binary but admitted none of three answers (two discarded after
    181/265 seconds). It compares inputs, not findings, mixing bytes and agent tokens at an assumed
    four bytes/token. Its observed `high`/four-lens tier falsely matched a Rust `#!` attribute;
    corrected `medium`/one-lens was read, not run. External revisions were `a537c63e` and `4db804e5`;
    no artifact remains, and its relative external citations cannot be checked from this repository.
  No judge-versus-judge or whole-delivery figure exists, despite the orchestrator spend channel.
  `collect_review_input --wait-seconds` has only a before measurement: one session, 16 calls, one
  `capture-running`. Zero subsequent polls is by construction, not an after measurement. No suite
  test supplies the missing live cost evidence.

- **A preserved usage artifact is not a measurement, and no total folds it in yet.** The runner writes
  the reviewer CLI's own counters to `usage.json` in a job's directory, beside `result.json` and never
  inside it, and `job::read_usage` reads them back; the admitted answer's bytes are identical whether or
  not the CLI reported usage. Only the Claude adapter's `--output-format json` result carries a `usage`
  object, so a Codex review and a relayed review preserve nothing. `scripts/measure-transcripts.py` reads
  Claude Code's own `.jsonl` transcripts and does not fold this in, and nothing in production calls
  `read_usage`, so what a native review cost stays unmeasured. The artifact outlives the result it
  belongs to (`discard_result` deletes `result.json` and leaves `usage.json`), so a reader checks
  `job_status` first. **A missing artifact is unknown, not zero**: `read_usage` answers `None` both when
  the CLI reported no usage and when the counters could not be stored (a failed write goes to
  `runner.err`), and the two are not distinguished; an artifact that is not JSON, names no provider or
  carries a non-whole counter is an `Err`, never a zero. No consumer exists yet to read `None` as
  unknown.

  **The failed-write diagnostic is emitted but unproved.** `report_usage_failure` writes to the
  runner's standard error, and no test added by this change reads that stream: the outcome a failed
  write leaves is asserted, the message is not, so a change that stopped emitting it would pass every
  test while the promise in `run`'s own comment stayed. The test that would capture `runner.err`
  drives the detached runner, and it lives outside the paths this change may touch.

## Refusals, and the way out they name

- **A refusal that names no way out is measured at build time, and the 67 the guard reports are frozen
  as debt rather than fixed.** `build.rs`'s `assert_every_refusal_names_a_way_out` reads the same walk
  over `src/transport` that fills `TRANSPORT_VOCABULARY` and holds three rules that were held by review
  alone: *do not name a command unless running it clears the block* (`AGENTS.md`), *an unknown result is
  not clearance*, and the contract's *never reinterpret an unknown result as clearance*. The population
  is every distinct reason the walk extracts, because a reason is the operator-visible identity of a
  refusal: **224 at this head**. A reason is satisfied when its envelope carries an `action`
  (`reasons_naming_a_continuation`; **154** do), when `BY_DESIGN` declares a shape from
  `NO_CONTINUATION_SHAPES` (`operator-knowledge`, `world-action`, `human-authority`, which is
  `src/outcome.rs`'s own `NoCommandReason` vocabulary, crossed by
  `tests/guards.rs::the_refusal_guard_vocabulary_is_the_harness_own`; **3** are declared:
  `blank-argument`, `invalid-horizon`, `invalid-operation-id`, all `operator-knowledge`), or when
  `refusal-baseline.txt` freezes it (**67** do: the operation-conflict and metadata-mismatch families,
  every `*-readback-failed`, and the generic argument refusals `malformed-argument`,
  `missing-argument` and `unknown-operation` among them). A `BY_DESIGN` declaration for a reason whose
  envelope already carries an action fails the build. `refusal-baseline.txt` may only shrink: an entry
  that matches no unsatisfied refusal fails the build naming it, so *the refusal was fixed* and *the
  refusal moved into a shape the walk cannot see* are told apart, and the walk's floor is the count that
  file records (`population 224`), so a walk that finds fewer fails naming the count.

  **What the guard cannot do.** It reads the operator-facing text for the presence of an `action`; it
  cannot tell a correct continuation from one that parses, runs and does not help, and only a person
  hitting it can see that. It cannot tell a genuine `BY_DESIGN` from a fixable refusal someone declared
  to silence the guard, and it does not reach a refusal whose reason is built by a shape the extractors
  do not read (`assert_reasons_covered` closes that class, not this). It was measured by mutation: a
  refusal added with no action and no declaration fails naming it, a baseline line matching no refusal
  fails naming the line, and removing a refusal from the source fails the floor.

  Three earlier refusals broke the rule and were found by a person hitting them
  (`several-runs-hold-this-checkout` naming runs that no longer exist, a phantom pointer's refusal naming an
  `estigia release` that did not discharge it, `reviewer-definition-changed` naming a world-action for a body
  Estigia itself wrote).

- **The per-runtime capability table is canonical; it does not measure that any host obeys a capability
  it is declared to carry.** `src/setup/capabilities.rs` holds `canonical(InstructionFile) ->
  Capabilities`, a total `match` with no `_` arm, and every entry of `AGENTS` declares its own
  `Capabilities`. A declaration that disagrees with the table fails the build (a `const` assertion runs
  `capabilities_agree` over the registry: `error[E0080]`), the same predicate crosses the table against
  `gate_spec`'s events, matcher and dialect and `agent_definitions_root`, `AgentAdapter::checked`
  refuses a mismatched adapter at construction
  (`an_adapter_whose_declaration_disagrees_with_the_table_fails_at_construction`), and
  `capabilities_for_slug` is fail-closed with `Capabilities::NOTHING`
  (`a_runtime_absent_from_the_table_carries_nothing`). What it does **not** measure is whether a host
  refuses a tool call, reads a hook, or discovers a skill at the root the table names; every column is a
  claim derived from what each host ships, so a column can be wrong about a host with the build green.
  What it buys is that a disagreement **inside the crate** is a compile error or a construction refusal.
  One disagreement is recorded and not fixed: ZCode's row carries `gate: Gate::None` while it carries
  the contract and the MCP server, the gap the ZCode entry states. This narrows the reporting gap (one
  word covering four states in what a person reads) and does not close it.

- **The merge-commit topology rule admits a forward-integrated delivery, and no longer proves the first
  parent was exactly the reviewed base.** `skill/SKILL.md` step 6 and `skill/bindings/github.md`'s
  `verify_claim` exception require the delivered SHA's second parent to be exactly the reviewed head and
  the reviewed base to be an **ancestor** of its first parent; `delivery_close` computes it with `git
  merge-base --is-ancestor <receipt.base> <first parent>` in `context.repo_dir`, reusing
  `transport::target::is_ancestor` (exit 0 is an ancestor, exit 1 is not, any other exit is a `Read`
  failure, because an unknown answer is not clearance). A delivery GitHub merged after `main` moved is
  recognised even though its first parent is a descendant, and a first parent unrelated to the reviewed
  base is still refused: `a_forward_integrated_delivery_is_recognised_as_this_runs_own_close` and
  `a_delivery_whose_first_parent_is_unrelated_is_not_this_runs_own_close` in
  `transport::claim::close::tests` measure both against a real repository. Estigia never guaranteed the
  first parent *equals* the base; the repository's strict check was that guarantee.

- **A Linear source reads an issue, writes nothing but a mapped state, and its transport is the `linear`
  CLI.** `Tracker linear <team-key>` is admitted only under `Workflow: assisted`, answered in a repository's
  own file (`config set` and `config edit` refuse it without `--repo`, `config set --repo` where `Claim
  workflow` is not assisted, and `config set Workflow autonomous --repo` where `Tracker` is already Linear;
  `doctor` and run start refuse it). `assist` reads the issue's title and description with one constant
  GraphQL query (`a_read_spawns_linear_api_with_the_query_and_sends_no_mutation`); `where` reads a key's
  state and URL with `STANDING_QUERY`, a finished issue included; `STATES_QUERY` and
  `MOVE_MUTATION` are sent only through `move_issue`, which `follow_state` calls only for a state the
  repository's `State names` row maps and only for a Linear key
  (`follow_state_moves_only_a_mapped_state_of_a_linear_issue`). A move is read back, reported under
  `linear_state` and never retried; a read-back that cannot be completed after Linear accepted the move
  answers `linear-move-not-read-back` with `world: unknown`, never the not-done `linear-unreachable`
  (`an_accepted_move_whose_read_back_fails_is_an_unknown_world`), so a run that ignores a failed one leaves Linear behind the work. Every
  operation is `linear api --variables-json <variables> <operation>` through the crate's one process boundary
  with a 30-second deadline; the CLI holds the credential, so Estigia never reads or passes the key. What it
  does **not** do: it writes Linear only to move a mapped state, and does not prove a credential can be
  scoped read-only. It depends on the CLI's behaviour measured against version 1.10.0 (`api`,
  `--variables-json`, exit status 1 on a failed request, the text `No API key configured`), so a changed
  message would be reported `linear-unreachable`; the workspace is the CLI's choice, so a repository's
  `.linear.toml` is not guaranteed to be the one read; only the two reads ran against the live API, the move
  only against the stand-in; a key the credential cannot see and a key that names nothing both answer
  `linear-issue-unknown`; authentication and "not found" errors in a GraphQL `errors` array are **matched
  heuristically**; `assist` refuses a key whose team is not the configured `<team-key>`
  (`linear-team-mismatch`). Nothing crosses the Linear client against a second implementation: the stand-in
  `linear` is the `fake_process` example.

  **The unclaimed-write refusal and `Unclaimed write policy` hold under `Tracker linear` with
  `Workflow: assisted`, as they do under GitHub.** Both ask `Tracker::adjudicates_unclaimed_writes`
  (true for a tracker a run can hold a claim against, which an assisted run does against Linear
  locally); Linear with workflow unset or autonomous stands down
  (`a_read_only_tracker_without_an_assisted_workflow_refuses_nothing`; `doctor` flags that
  configuration). Under `assisted` both refusals and the SessionStart text name `assist`, not `claim`,
  `create` or the checkout `claim` makes, which are refused there
  (`an_assisted_run_is_told_to_assist_not_to_claim`,
  `an_assisted_session_start_names_assist_and_not_the_refused_tools`). The tests build the gate context
  with `Tracker::Linear` directly, so the repository-file layer that produces it is read by its own
  tests, not crossed here. `estigia guard` and the pre-push hook say a read-only tracker without
  `assisted` adjudicates nothing.

## Which hook commands are Estigia's

A settings-file hook is recognised by its shape, `<program> hook <event>` (`runs_estigia_hook` in
`src/setup/render.rs`), not by a substring, so `slack-webhook stop` is not taken for the gate. What this
measures is the one regression test that writes such a command and runs `setup` then `uninstall` over it.
What it does not measure:

- **The program is not checked.** Renamed binaries and shims are accepted on purpose, so another tool
  whose command line is literally `<program> hook stop` is still read as Estigia's.
- **Only the first word is the program.** A wrapper in front of it (`env X=1 estigia hook stop`,
  `sh -c "…"`) is not recognised, so setup leaves it and writes its own entry beside it.

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

## What `release_ci` says about CI

`release_ci` returns `lane`, the reading `publication_lane` gives for the released head, and `next`:
`merge` for `not-consulted`, `disabled` and `never-ran`, `ci_status` for every other reading (unit tests
script each lane reading and a failed read). What it does not measure:

- **`absent` goes to `ci_status`.** The read follows the ready mark within moments, so a head whose check
  runs have not registered yet is indistinguishable from a repository with Actions on and no workflow;
  merging on it could skip a CI that is about to start. The second case still waits on `gh pr checks`.
- **An unreadable lane goes to `ci_status`.** Not knowing is not a licence to merge; the cost is the old wait.
- **No end-to-end release against a lane that is off.** The payload is asserted through the pipe only for
  an unreadable lane; the `disabled` and `never-ran` branches are asserted on the function that builds the
  answer.

## What `deliver` does not see

`deliver` runs the steps from the computed verdict to the close and hands back at the first that needs a
decision (`src/harness/mcp/deliver/tests.rs`: a scripted set of tool answers and a scripted `gh`, over a
real `git` repository with a bare origin; the happy path, each hand-back, a resume that repeats no write,
and a merge whose answer was lost). What it does not measure:

- **The live tools and the gate are not driven end to end.** One pipe test drives the real MCP server up
  to the receipt hand-back, and
  `a_release_that_stopped_part_way_is_not_read_as_delivered_beside_a_notice` drives `Live::tool` and a
  real store across the close alone, against a scripted `gh`; nothing runs the whole tail against the
  real store and gate.
- **The required set is the pull request's whole rollup.** The host's required names and the repository's
  own lanes are not resolved: a failing check nobody requires hands back `ci-red`, and a required check
  that never reported is not noticed while the rest are green.
- **An empty rollup waits ten minutes from the `ci-release` marker.** A repository whose pull-request run
  starts later reads `ci-absent`; one with Actions on and no workflow answers `pending` for the whole
  window. The window is a constant, not a measurement. On a `not-consulted` lane an empty rollup passes
  at once.
- **`wait_seconds` bounds what `deliver` waits for, not what it calls.** A hung `gh` or `git` call, or a
  slow `release_ci` clean-target read, is not cut off.
- **A host that ends an MCP call before `wait_seconds` is not measured.** The ceiling is 900 s, the one
  `collect_review_input` has. Claude Code held a `collect_review_input` call for 111 s
  ([`waits.md`](waits.md)), and no other host's timeout was measured. Nothing tells a caller what its own
  host allows, and the default stays 20 s.
- **Past the first 50 s the pull request is read every 20 s, not every 2 s.** A check that finishes in
  that window is seen up to 20 s late. The read count per call is bounded by the cadence, not measured
  against GitHub's rate limit.
- **A merge queue or an approval rule hands back.** `merge-unconfirmed` and `merge-refused` are not told
  apart from other causes, and the queue's configured method is not proven to keep a merge commit; a
  merge whose read-back never answers reads `merge-unknown` (it may have landed); an unreadable base or
  manifest reads `base-unreadable` or `version-unreadable`, never as advanced or unchanged.
- **Only two manifests are read for a version**: `[package] version` in a changed `Cargo.toml` and
  `"version"` in a changed `package.json`. A `pyproject.toml`, a workspace-inherited version whose root
  file did not change, and any other manifest move without a hand-back.
- **The lens roles in the evidence follow the tier and focus recorded on the receipt**, not each lens's
  report. **The summary line has two languages** (`English`, `Español`); any other `Summary language` is
  written in English. **The GitHub issue is closed after the release**: a failure of that last close
  leaves the issue `done` and released but open, reported as `delivery_incomplete`. A slice whose pull
  request does not close the issue is not released: `deliver` posts its evidence, moves the issue back to
  `in-progress` and answers `continues: true`
  (`a_slice_whose_pull_request_does_not_close_the_issue_continues_it`). It records no spend, since
  `release` is what records it, and it reads the closing references from GitHub, so a branch that
  names the issue with a closing keyword still closes it. A `deliver` started
  in the checkout its release removes runs every later read from the main working tree
  (`a_delivery_run_from_the_checkout_its_release_removes_still_reads_the_close`); when git cannot name
  that tree, the read still fails closed.

## What `estigia changes` does not do

`estigia changes` is the one read of uncommitted work per checkout, and `estigia prune` lists every
checkout it keeps (unit tests build real repositories with a dirty orphan checkout, a held one, a
directory git cannot read and a repository with no commits, and one pipe test drives the binary). What it
does not measure:

- **Two surfaces render the listing, and neither has run live.** The Pi `/estigia-changes` command and
  widget (`ctx.ui.setWidget`) are driven by node against a stand-in binary whose `setWidget` is a
  recorder; the Claude Code `/estigia-changes` slash command is tested as a file `setup` writes and
  removes, not as a command Claude Code ran. Not built: the OpenCode TUI plugin, the Claude Code
  status-line segment, and a Codex notice of its own. OpenCode, Claude Code and Codex see the
  session-start notice and nothing else, and none of it has run in a live session.
- **The session-start notice is the binary's text, not a per-runtime surface.** It counts uncommitted
  files with one `git status` per linked checkout under a three-second budget and names what it did not
  reach; the base checkout is never named, and a checkout a pointer names but git does not register is
  not read.
- **The `done`/`release` notice is tested on its function, not through a tracker**, and reads the
  pointer's checkout, so a run whose pointer recorded none gets no notice. It is a notice only
  (`lifecycle.md` documents the decision not to refuse).
- **No session-end notice.** `SessionEnd` answers `{}` and no runtime is known to show that output.
- **`orphan` is a readable run pointer, not the claim**; the issue's state is not read.
- **Line counts are Git's, and partial.** An untracked file is counted only when it is a text file of at
  most one mebibyte; a binary file, or one counted against a checkout with no commits, has none. An
  unmerged path is marked `M`. **Fifteen seconds is the bound on each Git call**, chosen rather than
  measured.

## The feed

`estigia feed` ([`feed.md`](feed.md)) projects the local store and records nothing (eleven unit tests seed a store with
claims, transitions, a marker entry in a second repository and a message, and drive each filter, the paging, the
refusals and the absent store). The `feed` MCP tool calls the same function; two tests hold it to the command's rows
for one seeded transition and to the unknown-repository refusal, not to every filter. What it does not measure:

- **It sees only what the store recorded.** A merge is read live from the tracker and is not an event; a consent
  prompt, a turn that ended and a gate decision older than the 30-day retention are not in the store. A state change
  made before the `transitions` table existed has no event. Every answer says so in `cannot_see`.
- **Quiet is not proof of a quiet run.** Runtimes with no gate (`zcode`, `deepseek-harness`, `agents`) or a
  pre-tool-only gate (`cursor`) produce no lifecycle events; the feed lists none for them and cannot tell that from
  a run that did nothing. A Cursor run registered before `AGENT_RUNTIMES` listed `cursor` is filtered as
  `claude`; one registered since is filtered as `cursor`.
- **The window is bounded, the scan is not indexed.** `entries` and `transitions` carry no index on time, so each
  call evaluates the time of every row before applying `--since`; not measured against a large store. An `entries`
  row whose `created_at` SQLite cannot read as a time (the compact `20260726T005435Z` form) is not listed.
- **The snapshot is a filtered re-read.** A row written in the same second as `taken_at` can join a later page,
  as with `estigia sessions`; the cursor is a digest, not a MAC. A `runtime` is the one the run id names, so a
  message is filed under its sender's runtime.
- **There is no link and no session start or end event.** Sessions are in `estigia sessions`; the feed lists
  transitions, markers and messages. A message's recipient may be a run that never registered.
- **Previews are tested for the three bodies seeded, not for every marker field.** A preview is the kind and the
  issue; nothing of a marker's other fields is shown without `--bodies`.

## Where

`estigia where` ([`where.md`](where.md)) is tested against a two-repository store and a stand-in tracker (fourteen unit
tests for the answer, six for the resolver, three for `tasks` and the identity parser); the live `gh` reader is not
exercised by any of them. What it does not measure:

- **The holder is the registry's, not the timeline's.** A run on a runtime with no gate leaves no pointer, so an
  issue it holds shows no holder; the store's claim markers are not consulted for it.
- **A Linear key is answered only in part.** `issue`, `workflow`, `holders` and `messages` are read; the
  transitions, tasks, clauses, queued writes, pull request and review stay `unavailable`, because the local store
  keys them by repository and number and a key has neither. `holders` matches the key alone, with no repository
  check. The Linear read is measured against the stand-in CLI
  (`a_finished_issue_is_read_as_closed_where_assist_would_refuse_it`), never the live API.
- **The repository of a pointer is read from its checkout's `origin`.** A checkout with no GitHub `origin`
  contributes no candidate; a bare number held only there falls back to the caller's checkout.
- **The tracker reads are two `gh` calls with no retry.** A rate limit or a missing login leaves `issue` and
  `pull_request` `unavailable` with the reason, and `bound_to_live_head` `null`.
- **Message previews carry bodies.** Unlike `feed`, which omits them by default, the last messages are shown with
  200 characters, because the question is what the sessions said.
- **A bare-number reference to `tasks` that no repository holds** answers `unavailable` only when the store or
  the repository is missing; a repository the store knows with no tasks recorded is a real, empty checklist.

## The session registry

`estigia sessions` lists the rows `estigia session start` and `estigia session end` write to the `sessions`
table, joined with the run pointers (five pipe tests drive the binary through a started, a crashed, an
unregistered and an ended session, a bounded walk with neither omission nor duplicate, each refusal a page
answers and an empty registry offering no cursor; four store unit tests). What it does not measure:

- **`last_seen` is refreshed by a session start and by a verified claim, not by every gated write**, so a
  session holding no claim past the six-hour bound reads `stale` while alive (the bound errs toward showing a
  live session as stale). **Only sessions whose host calls `estigia session start` or fires a registered
  `SessionStart` hook are registered**: Pi, Claude Code and Codex do, OpenCode's gate plugin registers a
  session the first time a tool call completes, and the rest appear as `unregistered` only while their
  pointer names an issue. There is no pid and no host query.
- **A pointer is read through `holdings`, which finds a file only under the name its run id gives itself**: a
  run id with an uppercase letter is filed under a digested name, so its pointer and issue are not shown
  (minted ids from the hosts measured are lowercase; not fixed).
- **A bounded snapshot is a filtered re-read, not one consistent read of the machine.** Each page keeps rows
  whose `started_at <= taken_at`; the ordering key is immutable, so a traversal has neither omissions nor
  duplicates under the tested mutations, but `status` and `last_seen` are re-derived per page. Membership is
  frozen only to the second (a session registering in the same second as `taken_at` can join a later page:
  `a_session_registered_in_the_same_second_as_taken_at_can_join_the_walk`). The cursor's digest detects
  corruption and an edit that does not recompute it; it is **not a MAC**, and possessing a cursor is not
  authorization. Where `store_identity` cannot be read (the Windows arm when its handle read fails, any other
  platform) every such store shares `unknown`, so the cursor's store check cannot tell two apart.
- **A `status`-filtered traversal is not a complete set** (the value is read afresh per page, so a row that
  ends between pages can leave the walk; `a_status_filtered_traversal_reads_the_status_afresh_and_can_omit_a_changed_row`
  asserts the omission), and **a bounded page covers the registry rows only** (a pointer-only run has no
  immutable start; `status=unregistered` is refused `sessions-unregistered-not-bounded`). A bounded answer
  has no fixed byte ceiling: it is bounded by `limit` (default 50, at most 500) and the row shape; one real
  call on 2026-10-18 returned 433 sessions as 141,698 UTF-8 bytes (a response size, not a token estimate).
- **The derived "task k of n" is matched by issue number alone.** A session row names no repository
  identity, so `fields: status` reads the checklists of the whole store and drops an issue number that two
  repositories both have a checklist for rather than guess which one the run holds; the row then shows nothing.
  A `status_text` is what the run wrote, redacted and capped but not checked, and is never evidence.
- **The pane is one environment variable, read once.** `HERDR_PANE_ID` was measured on 2026-10-22 inside herdr
  0.9.3 and 0.9.5 (the OpenCode processes in `w0:p1` and `wE:pA` carried it, matching `herdr pane list`). Not
  measured: a host exporting the same name for something else, a Windows pane, or a pane id that outlives its
  pane. The row records what the registering process inherited, a locator rather than a way in. **No service
  URL is recorded for any runtime**: the OpenCode service seen on 2026-10-21 (`127.0.0.1:49374`) answered
  `GET /session` with HTML and `GET /api/session` with 401, so a recorded URL would not have been a way in.

### `estigia resume`

Unit tests cover the command per runtime, each refusal, liveness from a row with no process, from another machine, with a dead pid and with a live one, the `--exec` refusals, the command and directory `--exec` prepares and the exit code it returns through an injected launcher, a stale OpenCode run with its worktree, an ended run whose
claim is live and one whose claim lapsed, and a worktree with a dirty file and an unpushed commit. What it does not
measure:

- **No resume command was run against a live session.** On 2026-10-27 only the installed help was read:
  `claude --resume [value]` (Claude Code 2.1.311), `codex resume [SESSION_ID]` (codex-cli 0.159.2) and
  `opencode --session, -s` (OpenCode v2.1.4) exist. That the same session id and context come back was not
  measured for any of them. Pi and Cursor have no command.
- **The shape check is a heuristic.** A UUID for `claude` and `codex` and a `ses_` prefix for `opencode` are what
  the sessions seen so far looked like; a Cursor session whose id is a UUID is registered as `claude` and gets a
  `claude --resume` command that will not find it.
- **The claim is read from the store's entries by a substring of the run id**, which imported entries need
  because their `run_id` column is empty. A prefix collision would only add a row the reducer then drops.
- **The push state is the commit count against the branch's upstream**, not `branch_identity_verdict`, and it is
  read from the last fetch; it never asks the remote.
- **A pid can be reused.** The probe asks whether a process with the recorded pid exists on this machine; a
  session that ended and whose pid an unrelated process took reads as `live`, and `--exec` refuses it. The safe
  direction, and not fixed: the process start time is not recorded.
- **The recorded process is the nearest ancestor that is not a shell, found by `ps`.** From the parent of the
  registering hook or command, `ps -o ppid=,comm= -p <pid>` is walked up at most eight levels, skipping `sh`,
  `bash`, `zsh`, `dash`, `fish`, `ksh` (with or without a leading `-`) and `estigia`; the first other ancestor is
  recorded. A `ps` that fails or cannot be parsed, a walk that reaches pid 1, and eight levels of wrappers all
  record no token, so liveness is `unknown` and `--exec` is refused. The limits: an agent launched through a
  wrapper that is not a shell (`env`, `node` running a script, `npx`, a supervisor) is recorded as that wrapper, and
  the row then reads `gone` or `live` for the wrapper's life rather than the agent's; a wrapper that outlives the
  agent reads `live`, which refuses safely. **None of this was measured against real hosts**: the walk is tested
  on fixture chains, and which ancestors Claude Code, Codex, OpenCode and Pi put between themselves and their
  hooks was not read.
- **Windows records no process**, so every row there is `unknown` and `--exec` is always refused. A row
  registered before the process columns existed is the same.
- **`--exec` was never run against a real agent.** Its launcher is injected in the tests, and the native
  commands remain unverified against the installed CLIs, as the first bullet says.
- **The stored binary is the one seen at registration.** The registry records the program of the agent process
  (`/proc/<pid>/exe` on Linux, `ps -o comm=` elsewhere, so a full path on macOS and possibly a bare name on other
  Unixes) and `resume` uses it only when it is an absolute path whose file name is exactly the runtime's command.
  A Node-hosted agent that `ps` names `node`, a binary since moved or upgraded, and a row registered before
  migration 20 all fall back to the bare name. Which program each host's `ps` entry names was not read on real
  hosts.
- **Pi's session-file path comes from `ctx.sessionManager.getSessionFile()`**, which the installed Pi's type
  declarations carry; the Node driver supplies a fake of it, and no live Pi session was resumed with the path.
  Only a root session files its path: a child process registers under its root's id, and its own file would be
  wrong for that row. A Pi row registered before migration 20, or by an extension older than this one, has no file
  and `resume` refuses it.
- **OpenCode still never sends `session end`, and no event was found that could.** The strings of OpenCode 2.1.4
  list `session.created`, `session.execution.*`, `session.idle`, `session.archived`, `session.deleted` and the
  like, but no event of a session being closed; `deleted` and `archived` are actions on a stored session, not the end
  of an open one. The plugin's dispose hook ends the instance, and nothing shows it fires when the process exits. No
  hook was shipped on a guess, so OpenCode rows only ever go `stale`; a live OpenCode session ending was not
  observed.
- **`resume` is CLI only, and `SessionStart`'s "holds no issue" text does not yet point at `claim`.**

## The session mailbox

`estigia sessions send` writes a `messages` row, `estigia session deliver` takes and stamps the pending rows,
and `before_agent_start` (Pi) and `PreToolUse` (Claude Code, Codex) call it (store unit tests, pipe tests through
send, list, deliver, refusal and an ended recipient, one pipe test through the Claude Code and Codex hook, one
unit test that a refusal keeps the mail waiting, Node tests on the generated Pi extension). What it does not
measure:

- **No runtime was run live for mail delivery.** One neighbouring fact was measured instead, on Claude Code
  2.1.319 (2026-11-07) against a probe hook answering `ask` with both a reason and `additionalContext`: when the
  person answered Yes, the model quoted the `additionalContext` and never the reason; when the person answered No,
  the model received only the rejection and no `additionalContext`. That is what the unclaimed-hold paragraph
  rides on ([configuration](configuration.md), `Unclaimed write policy`), and why it is repeated on every such
  `ask` rather than marked delivered. The same paragraph's sub-agent case was not measured: the hook answer
  carries `additionalContext` for a call with `agent_type` (a unit test), but no live Claude Code session with
  a sub-agent was driven to see whether the field reaches the sub-agent, its parent, or neither. Whether the same field beside an allow carries mail to the model, and whether the Codex build in use reads that field from
  `PreToolUse` at all, are taken from documentation; a Codex session that ignores the field receives nothing
  and the row is nonetheless stamped delivered, because the binary cannot tell. The Codex terminal UI and
  daemon were not driven. Pi's section lasts one turn (whether this Pi keeps it out of the stored
  conversation was not measured). Beyond `PreToolUse` only `Stop` delivers (Claude Code's waiter, Codex's loop
  `Stop`), so a session that makes no tool call and ends no turn is not reached; `UserPromptSubmit` and
  `PostToolUse` are not registered.
- **OpenCode was driven under `node` against stubs** (OpenCode 1's `tool.execute.after` appending to a string
  result, OpenCode 2's `execute.after` steering through a stub `ctx.session.prompt`, and the
  `ctx.session.synthetic` fallback). Unverified: that OpenCode 1 puts `output.output` in front of the model,
  that OpenCode 2's `ctx.session.prompt` accepts `delivery: "steer"` mid-turn and that every OpenCode 2 build
  has `execute.after`. A result whose `output` is not a string is left alone with its mail pending. **A known
  loss window**: when `ctx.session.prompt` or `synthetic` exists and throws after the rows were taken, the
  rows are stamped delivered that the host refused and the sender sees them delivered; no small fix exists,
  since take and stamp are one statement (a delivery is always stamped before the host acknowledges it, and a
  killed hook process loses the message as delivered; delivery never records "read", which is the recipient's
  own act, below). OpenCode costs one extra
  process per tool call.
- **The peek-then-stamp handover (Pi and OpenCode 2 idle wakes) is at-least-once, not exactly-once.** Store
  tests inject the failures: a poll that peeked and died before `--through` leaves every row pending for the
  next poll after a restart, so a host that accepted the mail before the crash is handed it again; a
  `--through` retried after its answer was lost stamps nothing twice and keeps the first stamp; two pollers
  stamping one peek at once deliver each of 40 rows to exactly one
  (`a_handover_that_dies_before_its_stamp_leaves_the_mail_for_the_next_poll_after_a_restart`,
  `a_stamp_whose_answer_was_lost_is_not_taken_twice_on_retry`,
  `two_pollers_stamping_one_peek_at_once_deliver_each_row_once`). No host gives a durable acknowledgment or
  a delivery key, so the duplicate window between host acceptance and the stamp stays.
- **Waking an idle session was measured live once, by hand, per host, and not under load.** Claude Code 2.1.300
  woke an idle interactive session within 15 seconds when an `asyncRewake` `Stop` hook exited 2 (a new waiter
  was running afterwards, none after the session was killed;
  [`sessions.md`](sessions.md#waking-an-idle-claude-code-session)); the OpenCode 2 wake
  ([`sessions.md`](sessions.md#waking-an-idle-opencode-session)) was driven against a stand-in host and run once
  on OpenCode 2.0.28 with `--standalone`. Not measured live: a Claude Code session idle longer than 60 s,
  mail arriving mid-turn, two waiters starting at the same instant (the unit tests drive threads; each new
  waiter overwrites a token in `store_meta` and a superseded one exits at its next poll), OpenCode refusing
  a prompt, and a session woken while the background service rather than a private server runs the plugin.
  OpenCode 1 is not woken, a session that never finished a turn is not polled, an already idle Codex session
  is not woken (Codex's `Stop` is synchronous and has an entry only with the `Loop plugin` row `on`), and Codex
  `turn/steer` is not built. Mail a waiter took is stamped delivered before the host acknowledges the exit.
- **The MCP `send_message` door was driven live on one host only** (Claude Code 2.1.300: `claude -p` refused
  with no row written, the interactive dialog's *Allow once* recorded a row that was then delivered; *Allow for
  this session* and *Deny*, Codex and OpenCode were not run live). A client that declares the capability and
  never renders the question waits out the 15 minutes and the send is refused. The grant is per server
  process and keyed by the sender's run id, which the caller names, so an agent naming another run's id
  borrows its grant only if that id was granted in the same process.
- **`read` and `acked` are what the recipient said, not what a host observed.** Nothing watches a model read
  its context: `inbox --read` and `--ack` are explicit acts, so a recipient that never calls them leaves
  every row at `delivered` however carefully it was handled, and one that acks without acting makes the sender's
  view say so. `inbox` itself is covered by store, unit, MCP and pipe tests against stand-in sessions and was
  driven on no live host. A thread is the id of its first message and is not scoped to the parties: any run
  that knows a message id can join its thread. `expires_in` is recorded and shown and decides nothing, so an
  expired message is still delivered. Types, threads and acks are text and rows in a local file; no gate,
  permission or claim reads them.
- **A question and a wait wake only what can be woken, and were driven on no live host.** A question reaches a
  session at its next hook point or idle wake. Claude Code (`Stop` waiter), Pi and OpenCode 2 (polling)
  record a wake; Codex and OpenCode 1 record none, so a question to them returns `not_woken` after delivering
  the question and does not wait, and a model on Codex can answer only on a turn it is already running. The
  heartbeat is an MCP `notifications/progress` sent only to a call that carried a `progressToken`; a host
  that sends none and kills silent calls will still kill a long wait, and no host was run to measure
  which does. The question and its answer are separate writes: an answer admitted a moment before the asker's
  timeout is recorded can sit in the mailbox as mail for a question already reported `timed_out`, never as
  the answer to it. A session that dies without ending its row is not seen as `ended` and the ask times out.
  The Pi parent-child channel was not moved onto the mailbox: it shares the rules (one outcome, refused
  late answers) and nothing else.
- **Broadcast and the completion contract were driven on no live host.** A broadcast reaches the members the
  resolver lists at the moment of the call; one that registers a second later is not in it, and a member that
  ends between the question and the write still gets a row it will never take. `idle` means a live session
  whose pointer names no issue, which is not the same as a session between turns: Estigia does not observe
  that. The one consent covers the group as named in the question, so a person approving ten sessions has
  approved ten sessions, not a rule. Accepting a handoff is read from a successful `claim` by a run of the
  recipient runtime in the directory the handoff recorded, so a recipient that starts from a worktree, or that
  works the issue without claiming it, never accepts and owes nothing; a store that cannot be written at
  that moment leaves the handoff unaccepted without saying so. The duty is closed by the store, and the mail
  that tells the sender is a second write: a mailbox that refuses it leaves the outcome standing and
  reports `mail_failure` to the recipient, and the sender still sees the outcome through `inbox` with
  `sent`. An abandoned duty is reported when the registry records the session's end; a session that dies
  without ending its row is never reported, and one that resumes after its end finds the duty abandoned and a
  late `done` refused. The deadline is a time the sender reads as `overdue`, not a timer: nothing wakes anyone
  when it passes, and only `estigia handoff --deadline-seconds` sets it. The
  hook that accepts on `claim` has a unit test of the acceptance and none that drives a real claim.
- **A message is text from another session.** Delivery says so in its first lines; nothing stops a model from
  acting on it. **The `flow dev` conflict rule reads the words of the message**: `send_message` and
  `estigia sessions send` refuse text carrying `flow dev <n>` while the recipient's pointer names another
  issue, so a sentence quoting it as something *not* to run is refused too and a differently spelled
  instruction (`/flow`, tabs, a Linear key) is not detected. A handoff is never consumed, expired or
  read-stamped: every later session of that runtime in that repository is told again, and whether anybody
  took it is visible only from the tracker, through `claim`.
- **The handoff's label read-back is against a stand-in `gh`** (the fake models the label edit in a file; no
  handoff was run against GitHub), and the row is written through `store::beside(state_root)` while the label
  goes through `store::store_path()`: the same file in production, two scratch stores under `cargo test`.
  The counts in `estigia sessions` belong to listed sessions (a message whose sender has neither a registry
  row nor a run pointer is counted and shown against nobody).

## The needs-person record

`estigia needs-person` lists rows written at three store writes (`escalate_review`, a `blocked` transition
without a clause, a gate `ask`), measured by one unit test each and a store test for the dedup. What it does
not establish:

- **A notification is best effort and no guarantee.** With `Notifications` at `os` the binary hands the text to
  `osascript` (macOS), `notify-send` (Linux, when on `PATH`) or a PowerShell toast (Windows) and records
  `handed to the OS` on exit 0. That is not "seen": a focus mode, a missing notification daemon or a
  session with no display all exit 0 and show nothing. The argv of all three is measured against a
  stand-in, not a desktop: no real notification was shown while writing it. The default is `off`, and
  the record is written either way. The quiet hours read this machine's clock and, when it cannot say, hold the
  notification back; the hourly cap counts `handed to the OS` rows in the store, so a notification that
  failed frees its slot.
- **Three moments of the six named are not written yet.** `handoff_review`, a verified `done` and the delivery
  halt write nothing: the first has no behavioural fixture to measure it against, and the halt is a pure read
  that must not write.
- **`blocked` is judged at the transition.** A clause recorded afterwards does not retract the row, and an
  unreadable clause read records nothing.
- **A failed write is reported where the answer has room for it.** Recording never fails the decision it
  describes. `escalate_review` and a transition say `needs_person_unrecorded`; a gate `ask` leaves a decision
  with the verdict `unrecorded`, which is itself lost if the same store cannot be written. A host that hides
  the gate's decisions hides this too.
- **An `ask` from a sub-agent is recorded and notified.** The record path does not see `agent_type`, and in
  Claude Code a sub-agent's `ask` does reach the person, so skipping it would hide a real wait.
- **An `ask` repeats after thirty minutes.** The key carries a thirty-minute bucket: a wait that outlives it is
  recorded again, and one that straddles a bucket edge can be recorded twice in a minute.
- **`ask` is only as real as the gate's `ask`.** A `Delivery authorisation` of `ask` may not reach the gate at
  all, and hosts other than Claude Code convert `ask` into a deny before it is recorded.
- **The repository key is read, not proved.** It comes from the shared git directory's `config` without
  spawning a process: a remote in a shape `RepoRef::from_github_remote` does not accept, or a config this
  reader cannot parse, falls back to the main checkout's path (a submodule's own root, inside one); a
  decision from a directory in no repository
  records no key, an ask records that working directory instead, and two clones of one repository with no
  GitHub remote are two keys. The path fallback is canonicalised, so symlinked spellings of one checkout
  agree.
- **No runtime was run live.** The tests drive the functions, not a session of any host.

## The waits audit

[`waits.md`](waits.md) counts the calls this repository's deliveries made because a clock passed: 247
of 17,062 tool calls in 45 Claude Code transcripts, carrying 72.95 M of 5,119.2 M context tokens. What
the count cannot show:

- **One machine, one host, one pace.** The transcripts are Claude Code's, from one person's profile, over
  four weeks. OpenCode, Pi and Codex sessions, and every subagent's own transcript, are not read. A
  slower lane or a different habit of checking CI would give other figures.
- **The classification is read from command text.** A loop is matched by `until`/`while` and `sleep`, and
  a pending CI read by the words `pending`, `in_progress` or `queued` in its output. A read that also
  diagnosed a failure is counted as a wait, so the pending-read row is an upper bound. A wait written
  in another shape is not counted, so the total is not a ceiling.
- **Context is not saving, and not billing.** The figure is the context of the requests that carried
  the calls, nearly all of it cache read. A replaced call still costs the request that reads its
  notification, and that net was not computed.
- **Why a run polled is not in the transcript.** The count shows that a run polled a file a background
  job would have announced. It cannot show whether the run expected no notification, or followed an
  instruction to wait in the foreground.
- **The 111 s is the longest MCP call this host was seen to hold, not its limit.** No other host was
  run, so raising `deliver`'s ceiling to 900 s leaves the shorter-host case unmeasured
  ([*What `deliver` does not see*](#what-deliver-does-not-see)).

## The HTTPS repeat

[`github-binding.md`](github-binding.md#when-ssh-to-github-never-connects) says what a refused SSH connection does
now. What it does not reach:

- **A `git` the agent runs in its own shell.** Estigia does not start it, so it stays on SSH alone.
- **Hosts other than `github.com`.** The rewrite matches nothing else, so the repeat fails as the first attempt did.
- **The identity.** The repeat authenticates as the active `gh` account, which may not be the owner of the SSH key.
  Nothing compares them.
- **Whether it happened.** A repeat that lands answers like a first attempt, and nothing reports that SSH failed.
  An intermittent refusal stays invisible until both roads fail.

The tests drive a stand-in `git` that scripts both attempts, plus one real `git` against a refusing `GIT_SSH_COMMAND`
for the message it prints. The repeat itself was run against GitHub once, by hand, and no test does.

## The token-efficiency programme

The programme asked for less redundant input and output across every supported runtime, with no lost
function. It closed with only its deterministic units delivered: the bounded session walk
([`sessions.md`](sessions.md)), the compact task acknowledgement and review view, the server-side
receipt expansion, the reviewer's own usage kept apart from its answer, the per-class transcript
accounting ([`review-cost.md`](review-cost.md)), the instruction budgets, the waits audit
([`waits.md`](waits.md)) and the mailbox crash cases ([*The session mailbox*](#the-session-mailbox)).
What it did not establish:

- **No saving is measured.** No paired whole-task run compares input, output, cost, time and quality
  before and after these units, on any runtime. A smaller answer is not shown to be a cheaper task: a
  compact form that costs an extra read, or a lost cache prefix, could cost more.
- **One host's telemetry.** Accounting reads Claude Code transcripts. Codex, Pi, OpenCode and the other
  registered adapters report no usage this crate reads, so their figures are unknown, not zero.
- **No attention-changing change shipped.** Deferred loading, filtering, compaction and checkpointing
  were left out rather than tried. Nothing here says whether they would preserve task quality.
- **No immutable-data reuse, no raw-artifact store.** Neither was built, so no security or lifecycle
  contract for one exists.
- **Mail stays at-least-once.** No supported host gives the durable acknowledgement that exactly-once
  delivery to the model would need.
