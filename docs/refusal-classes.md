# Refusal classes

A refusal answers three questions already: what happened to the world (`outcome`), whether the same
command is safe to repeat (`replay`), and what to do (`resolution`). This document owns a fourth
that none of them answers: **what kind of answer this was**. Without it a caller reads an outage as
a verdict, or a moved receipt as a rule, and acts on the wrong one — as happened in a review ledger
that reported a failed read as a stale verdict.

`class` is one of four words. It is in `--json` as `"class"`, and it is one line of the text the
hook and the tool server show an agent (`Refusal::agent_text`, one function for both).

| Class | Means | The caller |
|---|---|---|
| `unavailable` | The control surface could not be read. Nothing was decided. | Read again later. The answer is not a state and never clearance. |
| `stale` | The surface was read, and what the run was acting on has moved: a receipt, a head, an authority. | Re-read the current state, then decide. Repeating the command unchanged repeats the answer. |
| `refused` | The surface was read and the rule says no. | Stop, or change what was asked. |
| `unknown` | A write may or may not have landed. | Look before acting. Never report it as any named state. |

`unknown` is the fourth because the rule forbids reporting an unknown outcome as a state: `write-failed`
and `ambiguous-write` are neither a refusal by a rule nor a stale authority. It matches
`outcome: unknown`, and the setup and TUI refusals that already carry that outcome carry this class.

## How a class is chosen

One table, [`src/harness/refusal_class.rs`](../src/harness/refusal_class.rs), places **every code
the crate raises** — the transport vocabulary, the gate, setup, the TUI, the configuration, the
reviewer. `Refusal::not_started` takes its class from it, so a site cannot default to a class by
omission; a struct literal takes it from the same table by its literal code. A code the table does
not name is `unavailable`, never `refused`: an unnamed refusal must not claim a rule said no. A test
(`every_code_the_crate_raises_has_a_class_and_the_class_agrees_with_the_outcome`) makes an unnamed
code a failure anyway, another that nothing named `unreadable`, `unavailable` or `unparseable` is
anything but `unavailable`.

For a refusal that came through the transport, the exit code decides first
(`harness::refusal_class::class_of_answer`):

- exit `1` reads the table, and a reason it does not name is `refused`;
- exit `2` (a configuration defect or a malformed argument) is `refused` — the rule, not the tracker,
  said no;
- exit `3` (`read-failed`, `rate-limited`) is `unavailable`;
- exit `5` (`write-failed`, `ambiguous-write`) is `unknown`;
- any other exit (`0`, `4`, a signal) is an answer the contract does not describe, and is `unknown`.

The exit code overrides the table so that a failed read can never be reported as `stale` whatever its
reason string says.

`unknown` and an unknown outcome are one fact: choosing `unknown` sets `outcome: unknown` and the
replay advice that goes with it (`Refusal::classed`), and a refusal whose outcome is unknown is
`unknown`. A refusal cannot print `Class: unknown` over "nothing was written".

## The rules, and every decision that was a judgement

**`unavailable`** — the answer could not be read or has not been given yet:

- `*-readback-failed` (the write landed, the read-back was unreadable; the outcome is already
  `committed`), `payload-digest-unavailable`, `ancestry-unknown`, `assisted-issue-unreadable`,
  `linear-unreachable`;
- `*-unparseable` (`correction-state-unparseable`, `published-transaction-unparseable`): the record is
  there and cannot be read;
- `publication-unconfirmed`, and `publication-lane-unfinished` / `publication-lane-never-ran`: CI has
  not answered, which is not the same as CI answering red (`publication-lane-red` is `refused`);
- `review-status-store-unreadable` and `review-status-issue-not-in-store`: the store-only timeline read
  that `status` stands on found no readable record, and an absent record is never an empty timeline;
- `outbox-pending`: the writes a closed git operation window queued have not all been sent, so the
  operation that asked for them to go first was not started; nothing was decided, and the answer names
  the write the queue stopped at;
- `generated-reproduction-unavailable`: the declared command did not start, timed out, exited non-zero or
  left no file, so the generated file was not reproduced; it is not a match, and `approved` is not
  recorded over it;
- `review-head-unreadable` and `review-checkout-unreadable` (new): the head of the checkout, or the
  directory a command changes into, could not be read;
- everything the gate, setup or the configuration could not read — `*-unreadable`, `clock-unreadable`,
  `home-not-resolvable`, `gate-input-not-json`, `terminal-unusable` — and the reviewer that gave no
  answer in time (`reviewer-no-answer`, `reviewer-deadline`, `reviewer-unavailable`), and
  `reviewer-inspect-unreadable`: the checkout could not be read for an inspection, so the candidate is
  uninspected rather than failed. Also `capture-running` and `capture-runner-lost`: the reviewer's answer
  has not been given yet, or its process ended without giving one, which is not a verdict on the review.

**`stale`** — something the run held has moved:

- `capture-result-stale`: the receipt a reviewer was launched against is no longer the one
  `review_status` offers, so its answer is for bytes nobody is reviewing;
- every `*-operation-conflict`, `*-metadata-mismatch`, `*-operation-no-longer-current` and
  `claim-operation-expired`: another writer, or the passage of time, moved the operation. Also
  `already-owned-by-different-operation`, `operation-id-kind-conflict` and
  `target-operation-mismatch`, for the same reason;
- `computed-verdict-stale`, `computed-verdict-tier-mismatch`, `review-target-mismatch`,
  `review-target-moved-during-read`, `published-transaction-mismatch`, `release-target-mismatch`,
  `release-pr-mismatch`, `publication-base-not-ancestor`, `base-moved-during-creation`,
  `verdict-bound-to-other-bytes`, `expected-epoch-moved` (a publication whose `expected_epoch` is no
  longer the newest one): the bytes or the receipt the verdict was bound to are not the ones
  now there;
- `lost-claim-race`, `lost-reclaim-race`, `reclaim-discarded`, `not-current-live-holder`,
  `stale-self-requires-claim`, `stale-foreign-requires-reclaim`, `held-by-other-holder-disappeared`,
  `held-by-other-without-other-holder`, `ownership-changed-projections-repaired`,
  `ownership-kept-changing-before-projection`: the authority changed hands;
- `fresh-branch-diverged`, `import-diverged`, `remote-not-reachable-from-local`,
  `payload-rebuild-mismatch`, `publication-readback-disagrees`, `unexpected-state`: what was read is
  not what was expected.

**`unknown`** — `write-failed`, `ambiguous-write`, and `handoff-row-unwritten`: the label was
projected on the tracker and the local record of it could not be written, so the two halves of one
handoff disagree until somebody re-reads the issue.

**`refused`** — everything else, on the rule that the caller named something the ledger does not hold,
or broke a rule it can be told. That includes:

- rule violations: budgets (`correction-budget-exceeded`, `correction-already-spent`), things already
  recorded (`computed-verdict-already-recorded`, `refutation-batch-already-recorded`,
  `validator-result-already-recorded`), `rejected-on-insufficient-evidence`, `reviewer-not-distinct`,
  `reviewer-result-malformed` (the reviewer answered, outside its role's contract: it is not an unread
  surface, and nothing of the answer is read), `reviewer-payload-mismatch`, `reviewer-input-too-large`,
  `reviewer-inspect-transaction-unresolved`, `reviewer-inspect-outside-correction` and
  `reviewer-inspect-absent-side` (the caller named something the receipt or the correction does not
  hold), `capture-input-not-offered` (the caller named an input `review_status` does not offer, or
  one that is not a lens, so no reviewer ran),
  `capture-already-running` (a launch for a slot a running job holds), `capture-job-missing` (a
  collect for a slot no job holds) and `capture-job-other-run` (a collect by a run that did not launch
  the job: only the launching run collects it, and the rule, not a moved receipt, says no),
  `recorded-by-capture-only` (a hand call to `record_lens_report`, `record_refutation` or
  `record_validator_result`, or to `record_review_finding` naming a `lens`: only `collect_review_input`
  writes those, after the binary ran the reviewer),
  `issue-required`, `issue-creation-outside-flow` and `unclaimed-commit` (the gate read the claim and a
  rule said no: replaying cannot succeed until the claim exists, so these are not `unavailable`),
  `computed-verdict-rejected` and `-escalated`, every `invalid-*`, `blank-*`, `malformed-argument`,
  `missing-argument`;
- **`*-missing`, `*-not-found`, `*-unknown*`, `finding-citation-unresolvable`,
  `*-without-transaction`**: the caller cited a finding, receipt or entry the ledger does not hold. The
  ledger was read and it does not hold it, so the receipt did not move — it was never there;
- the gate's delivery codes, `delivery-pr-unidentified`, `delivery-pr-repository-option` (the line names a
  repository the gate does not read), `delivery-pr-mismatch`,
  `complete-review-transaction-missing`, `complete-review-transaction-not-selected`: the command or the
  checkout is not one a receipt can be selected for;
- `merge-without-ci-release`: the local store holds no `ci-release` for the exact receipt being
  merged, so no verdict was checked for those bytes;
- `inspect-parent-missing`: the publication named for an inspection was published with no recorded
  parent, or as a recovered successor, a fresh candidate, so there is no correction to look at;
- `transport-refused`, the catch-all for a reason the transport vocabulary does not name.

Judgement calls that could have gone the other way:

- `linear-cli-missing`, `linear-credential-missing` and `linear-credential-rejected` are `refused`:
  they are the operator's setup saying no, not the tracker being unreadable. `linear-unreachable` is
  `unavailable`.
- `worktree-ownership-unproven`, `branch-locked-by-another-run` and `worktree-owned-by-another-run`
  are `refused`: they are a rule about who may use a directory, answered after a successful read.
- `issue-not-open` is `refused`: the issue is not one to work, which is the rule and not a moved
  receipt.
- `delivery-not-verified` is `refused`: the merge was read and it does not join the reviewed head, so the rule
  says no. A merge that cannot be read is a `read-failed`, not this.
- `delivery-pr-mismatch` is `refused` although the receipt differs from the command: the command
  asked for the wrong pull request, and nothing the run held has moved.
- `verdict-bound-to-other-bytes` used to carry three causes. It now means only that the head moved
  (`stale`). A checkout of a different repository is `verdict-bound-to-other-repository`
  (`refused`: the command targets the wrong place, and nothing the run held moved), on `gh pr merge`
  and on every other delivery alike. A `cd` the gate cannot read, or a checkout git cannot say
  anything about (so it is neither known to be the same repository nor known to be another), is
  `review-checkout-unreadable` (`unavailable`).
- Not writing (`*-unwritable`, `*-not-writable`) is `refused`: the file system said no before
  anything was attempted, and repeating the command repeats the answer until it is fixed.
  `store-unwritable` is this class: the store's directory sits on a network filesystem whose
  locking and WAL cannot be trusted, so the connection is never opened.
- `foreign-projection` is `refused`: the issue's `dev:` label or assignee was read and names an
  owner the store cannot explain, so a rule — not a moved receipt — says no. The explicit `reclaim`
  is the override and does not run the check.
- `setting-not-read-back` is `unavailable` (the write landed and the read-back could not confirm it).
  The `*-not-read-back` and `repository-readback-missing` refusals whose outcome is already unknown
  are `unknown`.
- `sessions-send-takes-no-listing-flag` is `refused`: the flags were read and they name the listing
  door, which a `send` is not, so the rule says no and nothing is written. Nothing the run held moved,
  so it is not `stale`, and the answer is the rule rather than an unread surface.
- `assisted-claim-stale`, `assisted-claim-not-in-force` and `reviewer-definition-changed` are `stale`:
  what the run held moved. The five reasons the gate stands aside for (`not-watched`, `nothing-sworn`,
  `no-tracker`, `another-checkout`, `outside-the-claim`) are not refusals and carry no class.

A code that appears in the table cannot be moved without moving its tests: the class is part of the
contract an orchestrator reads.
