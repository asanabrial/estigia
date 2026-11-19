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
- [What the gate covers](honesty-gate.md#what-the-gate-covers)
- [doctor, and what it checks](#doctor-and-what-it-checks)
- [Identity and the tracker](#identity-and-the-tracker)
- [The gate's boundary](honesty-gate-boundary.md#the-gates-boundary)
- [The populations and the prose they cross](honesty-gate.md#the-populations-and-the-prose-they-cross)
- [Ownership](#ownership)
- [Reachability, registration and the rounds around them](honesty-gate.md#reachability-registration-and-the-rounds-around-them)
- [What the gate narrows, and what it records](honesty-gate.md#what-the-gate-narrows-and-what-it-records)
- [What a measurement reconstructs](#what-a-measurement-reconstructs)
- [Refusals, and the way out they name](honesty-gate.md#refusals-and-the-way-out-they-name)
- [Test counts and mutation claims in a receipt](honesty-testing.md#test-counts-and-mutation-claims-in-a-receipt)
- [Citations, and the coordinates that rot](honesty-testing.md#citations-and-the-coordinates-that-rot)
- [Generated files and reproduction](honesty-testing.md#generated-files-and-reproduction)
- [Which hook commands are Estigia's](honesty-gate.md#which-hook-commands-are-estigias)
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
