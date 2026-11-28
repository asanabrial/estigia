# Delivery, identity and ownership

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what delivery, the tracker identity and ownership do not establish. The index says which area holds which question.

## Delivery: the receipt, the base and the remote

- **What the `gh pr view` read measures, and what it does not.** It measures the PR's `headRefOid`
  and `baseRefOid` at the moment `stale_verdict` asks, one read per merge attempt, and refuses a head
  mismatch against the receipt with the PR number, the reviewed head and the head found. The base is
  measured against the receipt's: the pull request's base is admitted when it is the receipt's base or
  descends from it — the same placement `review_status` allows — and a base that is neither refuses
  `verdict-bound-to-other-base`, because a retarget changes what the merge lands. When the ancestry
  cannot be read the merge refuses `review-base-unreadable` rather than guessing. It does not measure
  anything after the read: a push landing on the PR between the read and the merge is refused by
  GitHub itself only because the merge call carries `--match-head-commit <head-sha>`, and the gate now
  refuses a `gh pr merge` line that does not spell it. A base retargeted between the read and the
  merge is **not** caught by that flag, so the read and the merge are not atomic for the base. What
  that does not reach is the shell reader's own misses: a line the classifier does not read as one
  literal `gh pr merge` — a quoted, expanded or chained form, a `--repo` naming another repository —
  refuses under `delivery-pr-unidentified` or `delivery-pr-repository-option` rather than passing, and
  a line the reader never matches as a delivery is not watched by this check at all. It does not
  resolve which repository `gh` would ask: an unrelated clone is refused on `same_repository` (whether
  the covered checkout and this one share a Git common directory), never on what `gh pr view` would
  have answered for it.
- **A merge recomputes the verdict at the line, over the store as it stands then.** `clearance_holds`
  asks the same `release_basis` `release_ci` asked, so a finding, a correction or a tier change recorded
  after the release refuses the merge instead of being delivered. The recomputation also regenerates
  every declared generated file the receipt touches, so a generated mismatch refuses the merge
  ([generated files](generated-files.md#the-approval-gate)). It does not make the merge atomic
  with the read: a finding recorded between the recomputation and the `gh` call is merged. The
  recomputation reads the checkout named by the gate (not `release_ci`'s `worktree`), so a change whose
  base and head are not in that checkout refuses rather than clears.
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
- **A publication's base is the default branch, and what that still does not establish.** `publish_review`
  reads the repository's default branch (`gh repo view --json defaultBranchRef`) and refuses any other
  `base` with `publication-base-not-the-integration-branch`, so a stacked pull request is no longer
  opened silently. The configuration has no row that re-enables one; a repository that needs stacked
  pull requests cannot publish them through Estigia today. Three things are still outside this:
  - **The merge gate re-reads the base rather than remembering anything.** A retarget after the gate's
    `gh pr view` and before GitHub executes the merge is not caught, and `--match-head-commit` binds
    the head only. `Deliver::base` refuses a base that does not descend from the receipt's
    (`base-moved`), and `review_status` answers `pull-request-moved`, but both read at their own
    moment.
  - **An advanced base is admitted without asking CI again.** The ancestry test says the base moved
    forward, not that the change still behaves the same on top of it; nothing re-runs the reviewed
    head over the advanced base, and a semantic conflict between two independently green changes is
    caught only by the CI that runs on `main` afterwards. `release_ci` stays strict: a draft whose base
    moved is answered `pull-request-moved`.
  - **The default branch itself can move and is not a setting Estigia reads back.** The read is `gh`'s
    answer at publication time; a repository whose default branch changes afterwards is not compared
    against what a receipt recorded, because the receipt holds the base's commit, not its name.
- **A delivery can still merge onto a base the reviewed receipt did not name, and the remedy is a
  GitHub setting that Estigia neither sets nor reads back.** `release_ci` re-verifies every byte of
  the reviewed head, and the hand merge now compares the PR's base with the receipt's, so a retarget
  is refused at this boundary. What remains is the gap between the gate's read and GitHub's merge,
  above. With `required_status_checks.strict: true`, GitHub requires the topic branch to be up to
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
    base's constancy is the repository setting's promise, taken on trust; the gate's own base test
    refuses a divergent base but cannot see a concurrent first-parent rewrite.
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
  `a_delivery_that_follows_the_sequence_puts_its_own_claim_down` in `tests/pipe/claims_and_delivery.rs` read back
  directory, registry and branch removal. Production cleanup remains best-effort/self-reported, and
  no guard test covers a single recognised-delivery holder (the single-stale test covers other closes).

  **No label shows which runtime delivered a closed issue.** No `dev:<runtime>` or `analyst:<runtime>`
  label is written at all, and a leftover `dev:<runtime>` label is emitted as `--remove-label` by
  `converge_ownership_projection` (`src/transport/claim.rs`) from an inline `BTreeSet::difference`.
  `a_claim_removes_a_stale_runtime_label_and_adds_none` in `src/transport/claim/tests.rs` measures that
  emission for a claim, and the handoff tests in `src/transport/outbox/offline.rs` measure it for a
  handoff; `an_unassign_releases_the_epoch_in_the_store_and_its_discovery_writes_nothing` measures only that a release adds none. The run-id is not in the close note
  either: it survives only in the local store and in `deliver`'s `record`, which never leaves the machine.

- **A release closes a hand merge only on the road a holding run takes.** `open_hand_merge` in
  `src/transport/dispatch.rs` runs inside `merged_release`, which a release reaches with a
  `target_operation`; the tool fills one for a run that holds the issue, and a release naming none
  takes `claim::unassign` and closes nothing. Under a closed window it also closes nothing, since the
  close is not queued, and the issue stays open until a later release or `deliver`. A close that stops
  after its merge note landed answers `close_incomplete`; the repeat finds the issue still `OPEN` and
  posts the note a second time. `an_autonomous_release_after_a_hand_merge_closes_the_issue_and_puts_the_checkout_down`
  and `only_an_autonomous_repository_closes_a_hand_merged_delivery` measure the open-window road and
  the workflow predicate; `an_assisted_repository_never_closes_the_issue_a_release_finds_open` is
  answered by the tool's `workflow-assisted-writes-no-tracker` refusal before the transport is reached.

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
- **A person's hand-over confirmation is protected by the shell match and by the state directory.**
  `estigia hand-over` is refused to an agent's shell line by `harness::person_only`
  (`hand-over-is-for-a-person`), and a write tool whose target or resolved landing is under
  `~/.estigia` is refused `state-directory-is-for-the-binary` for every run, claimed or not, so a plain
  file tool no longer reaches the row. What it writes is still a `hand-over` row in the local store,
  and a writer that names no path the gate can place — `sqlite3`, a script, an interpreter handed code
  — reaches it with nothing refused. The confirmation keeps out an agent that stays inside its tools,
  which is the standing the claim markers themselves have; it is not evidence that a person ran
  anything. What the store does bind is exact: the marker names the run taking the issue and the epoch
  it displaces, so a confirmation for another run, or for an epoch that has moved, matches nothing
  (`plan_takeover`).
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
  (`a_slice_whose_pull_request_does_not_close_the_issue_continues_it`), and it records the spend it
  was given before it posts its evidence
  (`a_continuing_slice_records_the_spend_it_is_given_before_it_posts_evidence`). What the closing note
  posts is the summary line and the appendix and nothing else; the delivery's own account — receipt,
  tier and lens roles, verdict, findings by class, checks receipt, CI summary, merge and parents, spend
  and run id — is `Deliver::record`, answered to the caller and stored locally, never posted. It reads
  the closing references from GitHub, so a branch that names the issue with a closing keyword still
  closes it. A `deliver` started
  in the checkout its release removes runs every later read from the main working tree
  (`a_delivery_run_from_the_checkout_its_release_removes_still_reads_the_close`); when git cannot name
  that tree, the read still fails closed.

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
