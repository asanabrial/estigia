# The parity target

Estigia is built by measuring itself against another project, **gentle-ai**. This document answers
the questions that measurement needs and no others: *which revision* it is read at, *which layer* is
compared, and *which parts* of that project Estigia deliberately does not carry.

It is not a roadmap and it decides no behaviour. Where it and the code disagree, the code is what
runs; where it and [`honesty.md`](honesty.md) disagree about what was measured, `honesty.md` is the
document to believe.

## The revision

The comparison is read at **`gentle-ai` `v4.0.0`**, commit `ff77164d4f56f1665b22fb6fac51c2ccbb769400`,
released 2026-10-01. Where a fact exists only on
`main`, the revision it was read at is named in the sentence that carries it.

The repository's own citations are mixed, and they are left as they stand rather than moved.

[`honesty.md`](honesty.md) reads `a537c63e` (2026-08-28) for the reserved-reviewer census and the
patch-digest comparison, and `e2cb2bf2`, on the `v3.7.0` line, for the disposition entry;
[`review-cost.md`](review-cost.md) records its figures against `a537c63e`, re-confirmed at
`4db804e5`. None of them is re-pointed here: a sentence moved to a revision nobody measured would
claim a reading nobody took, and how far apart those checkouts are is recorded where it was measured
rather than restated here.

What this document supplies is the reference the *current* conversation is against. A claim that
Estigia is or is not at parity is read at `v4.0.0` until a later tag replaces it here. A figure that names an older commit was
measured there and is not re-read at `v4.0.0` unless its own sentence says so.

## The layer that is in scope

Estigia is a **workflow authority** and nothing else. The comparison covers exactly that surface:

| In scope | What is compared |
|---|---|
| Claim, gate, verify | that a write is measured against a live claim, and that an unreadable tracker is neither permission nor a stand-down |
| The state contract | the workflow states, the transitions between them, and the local store as the only source of truth |
| Independent review | the tiers, the panel, the receipt a verdict binds to, and the delivery gate |
| Adapter registration | which runtimes Estigia sits in front of, and how |
| The installer lifecycle | what setup writes, what it records, and what uninstall takes back |

## ODD and RDD are adopted, with two pillars kept

Estigia adopts gentle-ai v4's ODD workflow and RDD review: small work stays
small, review is sized by risk, a lineage gets a bounded number of corrections, and Judgment Day is opt-in. The
differences that follow are each settled in the next section. Two of Estigia's own pillars are kept
where a literal port would invert them, and both are declared divergences with a cost.

- **Delivery stays gated.** In v4 an approval is informational: `review validate` answers
  `allowed: false` with `unmanaged`, and delivery is ordinary repository policy
  (`internal/cli/review_facade.go`, external). Estigia keeps the pre-push hook, the `gh pr create`
  and `gh pr merge` boundaries and `release_ci`, so a pull request is marked ready and merged only
  over a computed `approved` verdict bound to its exact head and base. The cost: every publication
  waits on its review before it can ship, a base that moves after the verdict forces a republish and
  a new review, and an escalated receipt waits on a person's `estigia recover` where v4 would let
  ordinary policy deliver it.
- **Authority stays with Estigia, not with a checkout file.** v4 keeps its review transaction under
  `<git-common-dir>/gentle-ai/review-transactions/v2/` and its ODD tasks in `odd/tasks/*.md`.
  Estigia keeps workflow state, receipts, findings, verdicts and tasks in the local store
  `~/.estigia/state.db`, never in the checkout and never on the issue timeline. The cost: the
  store is one copy per machine, snapshotted only onto that same machine under
  `~/.estigia/snapshots/`, so a second machine cannot see its
  claims, and a binary older than the store's schema refuses to read it, so every open session must
  restart after a reinstall.

## Components deliberately out of scope

gentle-ai is an **ecosystem configurator**; Estigia is one component of that ecosystem's concerns.
The rest is out of scope by the boundary [`README.md`](../README.md) states — Estigia is not memory
(Leteo is), not planning, and it does not run the working model (it launches only the reviewer CLI for a lens, the refuter or the validator, through `launch_review_input`), define tools or manage context — so each
piece has somewhere else to live:

| Component | Where it stays |
|---|---|
| Persistent memory | [Leteo](https://github.com/asanabrial/leteo), a separate store |
| The curated skills library and the skill registry | not this crate |
| Context7 and other documentation MCP servers | not this crate |
| The teaching persona | not this crate |
| The permissions overlay | the host's own configuration, which Estigia does not overwrite |
| GGA, the pre-commit AI review | not this crate; Estigia's `pre-push` guard is a different mechanism at a different boundary |
| The theme | not this crate |
| Runtime telemetry and its dashboard | not this crate |
| The `gentle-pi` package harness | not this crate |

Naming these is not a gap list. A component here is not missing; it is *deferred to its owner*, and
a change that pulled one into Estigia would be a change of product rather than progress toward
parity.

## The shared layer's differences, and where each is settled

Nothing on the shared layer is open. Each difference the comparison found is settled by a rule or a
decision recorded where a reader looks for it:

- **The `Review protocol` default.** Settled: a fresh table starts at `receipt-driven`,
  the protocol the contract argues for and the one gentle-ai ships. It then became the only
  protocol: an installed `standard` still loads and reads as `receipt-driven`. The
  row was retired outright, so `standard` and every other value load under the hard-coded `receipt-driven` and an
  installed row is reported as `review-protocol-retired`. It also made the verdict computed, as v4's
  is: each severe
  finding is settled by v4's ordered table, a lens's completion is its own recorded event, and only
  a computed `approved` releases; no judge records a verdict that releases, so the aggregate
  verdict is retired (`review-verdict-retired`). Two declared divergences. A `low` receipt
  releases on the structural readback with no lens and no marker. And a retired `review-verdict`
  marker is history that still refuses when it was a distinct reviewer's rejection
  (`legacy-verdict-rejected`), which v4 never carried.
- **Judges.** Settled: ordinary review licenses none, matching v4 `ff77164`, whose
  `risk.go` names no judge; Judgment Day is the opt-in. It runs only on an explicit request,
  replaces the lens review for that target, launches no refuter, issues no receipt and gates
  nothing, as v4's `judgment-day/SKILL.md` states. `review-blind` and `review-blind-sealed` stay
  installed as its two judges. One divergence is declared rather than closed: v4 has no ledger a
  judge could write to, and Estigia's `record_review_finding` would accept a judge's finding,
  which the policy forbids and nothing enforces — [`honesty.md`](honesty.md) records it.
- **Reviewer distinctness.** Settled: v4 carries none, and Estigia keeps four
  rules as declared divergences, each because Estigia has a mechanism v4 does not. A handoff is
  resolved only by a recorder outside the publisher and requester set (v4 has no handoff). An
  approval supersedes an escalation only from such a recorder. A retired rejected
  `review-verdict` still refuses (`legacy-verdict-rejected`). And a refutation is refused when the
  caller-supplied `refuter` label names the publisher or a requester (`reviewer-not-distinct`): v4
  admits refuter results only against a Go-issued request hash with proof_refs, while Estigia's batch
  carries no proof. The check compares the label the caller supplies, never the recording `run_id`,
  so it stops an author only while that label is honest; `docs/honesty.md` records the limit and the
  choice to correct this sentence rather than bind the check to `run_id`. A fifth
  rule is kept the other way round: a targeted-validator result carries no reviewer identity and no
  proof, since the run holding the claim records it on a sealed role's behalf and nothing requires that
  run to differ from the publisher. That does not widen a gate, because a pass clears only the
  requirement the validator adds to a correction and never drops a finding, so an open severe finding
  still computes `rejected`. A finding's `reviewer` field is a label; nothing reads it to decide.
- **Adapter coverage.** Settled by the rule in
  [`what-it-writes.md`](what-it-writes.md#which-runtimes-estigia-adapts-and-on-what-evidence): a
  runtime is adapted when what it ships can be verified. The count was never the measure — the two
  projects overlap without either containing the other.
- **Windows batch-shim quoting.** Settled: v4's
  `internal/reviewerprovider/windows_batch_cmdline.go` exists because Go's `os/exec` cannot quote
  for a `.cmd` shim. Rust's standard library does it at the pinned toolchain, with `/v:OFF` and a
  `%` escape v4 lacks, so Estigia launches the shim with plain `Command` and ports nothing. What that
  leaves unmeasured is in [`honesty.md`](honesty.md).
- **Per-unit commits and candidate assessment.** Settled: the candidate is the
  publication. A commit is not a review candidate and no review is due before one, because a verdict
  binds the epoch, head, base and digest `publish_review` mints;
  [`repository-delivery.md`](../skill/references/repository-delivery.md) states the rule where
  review-unit sizing lives.
- **SDD removed, matching `gentle-ai` `v4.0.0`.** Settled: at
  `v4.0.0` gentle-ai ships no SDD — it removed SDD and OpenSpec in #4967 (`e219644b`),
  `internal/components/sdd/` no longer exists, and ODD is its only flow, where the `v3.7.0` line this
  entry once recorded against shipped ten phases. Estigia has removed it too: the protocol
  `skill/protocols/sdd.md`, the five phase definitions, the `Planning` setting, the `PlanningPhase`
  routing kind and the screen's planning targets are gone, a stored `Planning: sdd*` row still reads
  as `direct` and reports `planning-protocol-retired` once, and an owned `sdd-*.md` is retracted
  through a retired-names list. Neither side ships a phase set.
- **The ODD protocol.** Settled: `skill/SKILL.md` carries gentle-ai v4's seven ODD steps
  (`internal/components/agentguidance/routing.go:33-48`), the evidence budget
  (`internal/agents/capabilitymanifest/manifest.go:214-236`) and test-first as the default. Two
  divergences are declared. Tracking is recorded with the `task` operation under the claimed issue in the local store,
  not in a checkout file (`routing.go:116`); and review binds the publication, so a chained delivery reviews per pull
  request rather than per commit (`routing.go:119`).
- **The managed block per runtime.** Settled as a declared divergence. gentle-ai v4 renders a full
  orchestrator block per runtime and strips the receipt-driven-development content from runtimes that
  are not review-capable (`internal/components/agentguidance/orchestrator.go:246-266`). Estigia
  deliberately keeps a short pointer directive instead, identical for every runtime: the ODD and
  review rules live once, in the skill the directive names, because a copy in an always-loaded file
  would be a second statement of them to disagree with the first and would cost tokens every turn.
  Nothing is stripped per runtime, since the binary-launched review needs a reviewer CLI on the
  machine and not a particular runtime. What v4's split guards, relay wording reaching a runtime with
  no relay, is kept as `Capabilities::review_relay` (`true` for OpenCode, whose relay plugin is built, and for Pi, whose relay extension is built, and `false` for every other runtime) and a leak
  test over the rendered directive and the OpenCode and Pi review sections, using Estigia's own markers
  (`relay`, `review transport`, `--input`) rather than v4's, which name v4's host-specific wording.
  One block per instruction file and in-place replacement of an older one are pinned by tests.
- **Corrections per lineage.** Settled: v4
  allows one correction attempt after the original review (`MaxCompactCorrectionAttempts = 1`,
  `internal/reviewtransaction/compact.go:23`) and Estigia refuses the one past the `Corrections per
  lineage` row, as `correction-already-spent`, at the publish call. That row defaults to 3, an
  operator's choice (2026-10-11) and a departure from v4's limit of 1; set to `1` it is v4's limit.
  Five declared divergences besides it. The count is derived from the `parent` receipts in the store rather than held in a
  transaction record, so a deleted marker lowers it. The lineage's boundary is the pull request, so a
  change moved to a new pull request starts its own correction. A republish over an approved receipt
  is a new lineage, which v4 expresses as a new transaction. And the targeted validator
  (`internal/reviewtransaction/compact.go:2085-2200`) is a read-only role whose
  result is a local record that the correction's computed verdict waits for, in addition to the tier's
  lenses where v4's validator is the terminal event; a pass never overrides the ledger, a conclusive
  failure escalates with v4's two causes. The fifth: the third `unavailable` inspection escalates as
  `validator_inconclusive`, where v4 spends no correction on an inconclusive attempt and only refuses a
  fourth. `budget_exceeded` is not reachable here, since an over-budget correction is
  refused before it publishes. `escalate_review` also stays open once the corrections are spent, for a
  person. A sixth: when a correction merges a moved base, Estigia offers
  the targeted validator over the tree git merges the parent head and that base into, rather than
  escalating `validator-delta-carries-upstream`; only a rewritten history or a merge that conflicts
  still escalates that way. gentle-ai `main` at `8ae99114` (2026-10-08), read since v4.0.0
  (2026-10-01), has no counterpart: its escalation causes (`CompleteReview` to `StateEscalated` on
  unresolved findings), `MaxCompactCorrectionAttempts = 1` in `internal/reviewtransaction/compact.go`
  and the targeted validator (`validateCompactTargetedValidatorAttempts`) are unchanged, and nothing
  there handles a moved base or a merged range. Its additions since, an agent-raised escalation that
  only lifts the risk tier to `high` and a refuter that also accepts severe deterministic findings,
  do not touch this. Estigia also says why in a plain sentence, beside every reason code, where v4
  shows codes and typed refusals only.
  `Repair ceiling`, which had no counterpart in v4, is retired.
- **Recovering an escalated review.** Settled as a declared divergence. v4's `review recover` takes an
  exact six-line maintainer authorisation (two spans in
  `internal/reviewtransaction/compact_store.go`) bound to the predecessor lineage, revision and target identity and copies it onto the successor,
  checking the text binding and no repository permission. Estigia diverges, declared: it drops the
  text. Recovery is `estigia recover`, a person-only CLI command — no MCP tool, and the gate denies an
  agent's shell line for it on every host — plus a GitHub check that the named login holds the `admin` or
  `maintain` role, which v4 never asks. The record is local and posts nothing. The successor is not a
  stored lineage record but a publication whose marker carries `recovered-from`, so its tier floor and
  lens focus are found by walking parents. What a person at a shell could still do, name another
  maintainer's login, and what the gate's Bash rule does not match, are in `docs/honesty.md`. A
  second divergence: `estigia recover <issue> --reason "<why>"` is enough,
  `--epoch` defaulting to the issue's one escalated receipt and `--authorized-by` to the login `gh` is
  authenticated as, both overridable. gentle-ai `main` at `8ae99114` has not changed `review recover`'s
  arguments, defaults or messages since v4.0.0 (`RunReviewRecover` in
  `internal/cli/review_facade.go` only adds carry-over of agent escalation, lens selection and request
  context to the successor), so this is Estigia's own.
- **A moved base never resets the budget.** Settled by matching v4. v4 never re-freezes: a correction is a fix diff from the frozen candidate tree, charged
  against the frozen budget and genesis scope (`internal/reviewtransaction/compact.go:2053-2063`).
  Estigia used to start a new genesis with the cumulative at zero when the base advanced, and now
  matches: the round is measured against the tree `git merge-tree --write-tree` makes of the parent
  head and the new base, the cumulative is carried, and the marker records `budget-parent=integrated`.
  Two divergences, declared. The paths the merge conflicted on and the advance's rename destinations
  are admitted beside the genesis paths, which v4's frozen scope has no such notion for. And where
  v4 does not narrow a correction's review to lenses at all, Estigia still does for an unmoved base,
  and a moved base — or a side branch merged back — is read whole
  (`whole_change_reason: "base-advanced"` or `"merged-range"`).

Declined, with the reason:

- **Model routing.** That half of the comparison is already tracked elsewhere, and this document does not restate it.

## Where the shipped skill's ODD wording is anchored in gentle-ai

`skill/SKILL.md` states the delegation table and the seven ODD steps as rules and cites no source,
because an installed agent cannot open one. Each is anchored here, in gentle-ai source this
repository does not contain (external). The line ranges are copied as the skill carried them; the
revision they were read at was not recorded there, so they may have drifted from the revision named
at the top of this document, and the symbol or file is the citation that holds.

- **The delegation table's first row is v4's evidence budget:** `internal/agents/capabilitymanifest/manifest.go`,
  lines 214-236. File counts are no longer a delegation trigger in v4
  (`manifest.go`, lines 83-85), and the writing row's `2+ non-trivial files` is the one file-count
  boundary v4 keeps. Estigia no longer keeps it: its writing row delegates for a named reason only.
- **The ODD protocol's seven steps:** `internal/components/agentguidance/routing.go`, lines 33-48,
  adapted to Estigia's two kept pillars (the claimed issue is the record of the work, and the
  publication is the review candidate), as the section above settles.
- **Test-first as the single default policy:** `routing.go`, line 119. `internal/cli/sync.go`, lines
  226-228, retires v4's `--strict-tdd` flag, which is why the skill states test-first as a default
  and not as a switch.

