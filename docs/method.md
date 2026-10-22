# The method

**Status: draft.** This document states Estigia's own method — the principles a run works by, what
each one prevents, and what holds it — and it decides no behaviour. The contract an agent reads is
[`skill/SKILL.md`](../skill/SKILL.md); where the two disagree, the skill is what an agent follows
and this document is the one that is wrong.

The method grew out of gentle-ai v4's ODD workflow and its receipt-driven review, which
[`parity.md`](parity.md) records as adopted with declared divergences. Read from the other side,
those divergences are the method: the parts Estigia keeps where a literal port would have inverted
it. This document states them as principles first and places the inherited sequence beneath them.

## The principles

Each principle names what it prevents and who holds it. **Held by the harness** means a write that
breaks it is refused whatever the run intended. **Held by the run** means nothing refuses it, and
the gap is the run's discipline and the reviewer's attention — [`honesty.md`](honesty.md) is where
such a gap is measured.

### 1. A claim is the only authority to write

A run writes to a repository only under a live claim on the issue the work belongs to, and the local
store (`~/.estigia/state.db`) is the only record of who holds what. A label, an assignee, a tracker
comment or an observed worktree is a projection of a claim, never the claim.

- **Prevents:** two runs building the same issue, and a stale signal read as ownership.
- **Held by the harness:** the tool gate re-verifies the claim at the first write it observes and at
  every irreversible boundary; the `pre-push` hook refuses a push no live claim authorises, whoever
  typed it.

### 2. A verdict authorises exactly the bytes it was computed over

A review is computed over a frozen target — base through head plus the uncommitted worktree — whose
digest is the receipt. A push, a moved base or an edited file invalidates the verdict; reasoning that
the change "probably" still applies does not earn it back.
[`protocols/rdd.md`](../skill/protocols/rdd.md) owns the rule.

- **Prevents:** a review of one tree released as the approval of another.
- **Held by the harness:** `release_ci` compares the supplied receipt with the recorded one and with
  the digest re-derived from the clean target, refusing a difference as `release-target-mismatch`.
- **Held by the run:** the post-apply and pre-commit comparisons.

### 3. Delivery is gated, not advised

A pull request is marked ready and merged only over a computed `approved` verdict bound to its exact
head and base. No judge records a verdict that releases; the verdict is computed from the recorded
findings.

- **Prevents:** an approval that is informational, which is an approval nothing waits for.
- **Held by the harness:** the `pre-push` hook, the `gh pr create` and `gh pr merge` boundaries, and
  `release_ci`. A spent correction budget stops at a person's `estigia recover`, which an agent's
  shell line cannot run.

### 4. A defect is reproduced before it is fixed

A fix starts with a test that fails against the code as it stands, for the reason the defect exists.
If turning the fix off leaves the suite green, the fix is not tested, and that is a finding.

- **Prevents:** a fix whose test would pass without it, which measures nothing and is believed.
- **Held by the run.** The shipped skill states test-first as the default; the stronger form — the
  test fails *for the defect's reason*, and goes red again with the fix removed — is stated only in
  [`AGENTS.md`](../AGENTS.md) today, for this repository's own work.

### 5. Report only what was read back

A state is reported after it was read back, never inferred from the write that should have produced
it. An unreadable tracker, a failed read or an unplaceable repository is not an answer: it is
neither permission to proceed nor a reason to stand down, and it is reported and retried.

- **Prevents:** an outage read as clearance, and the nearest named state reported in place of the
  real one.
- **Held by the harness** where the transport performs the readback; **by the run** wherever it
  reports a state in prose.

### 6. What is not measured is said

A gap that was found and not closed is written down with the measurement that found it, and a claim
in a document is only as good as what holds it. A document describing the previous behaviour is
worse than none, so behaviour and its description change together.

- **Prevents:** a confident document that is wrong, which is believed longer than a missing one.
- **Held by the harness** for counts: `tests/honesty.rs` fails when a count a document claims drifts
  from the code. **Held by review** for everything else.

### 7. A rule lives in one place

When two places hold one rule, the fix is to remove a copy rather than to add a check that keeps
them aligned. The skill's references each own their subject once; the runtime directive points at
the skill rather than restating it.

- **Prevents:** a rule that disagrees with itself, and the round spent finding which copy is right.
- **Held by the run and by review.**

## The sequence

The work proceeds in seven steps, inherited from gentle-ai v4's ODD and stated as rules in
[`skill/SKILL.md`](../skill/SKILL.md). Each step is here for the principles it serves.

| Step | What it does | Serves |
|---|---|---|
| Authorize | read-only work stays read-only; anything else is claimed first | 1 |
| Explore | in proportion to the request | — |
| Resolve uncertainty | at most one product question and one challenge to a premise | — |
| Classify | two or more meaningful steps is substantial; fewer is small work | 7 |
| Track | a substantial change records its task checklist in the store, under the claim, before the first write | 1, 5 |
| Implement | one work-unit commit per task, test-first | 4 |
| Publish and review | freeze the target, compute the verdict over its receipt, release CI | 2, 3 |
| Close | deliver at the reviewed head, read the delivery back, record evidence and spend, release the claim | 3, 5, 6 |

Two rows differ from the inherited steps. **Publish and review** is its own step because the
publication, not a commit, is the review candidate. **Close** is concrete here where the inherited
step names only the word: closing is the delivery read back, the evidence recorded with the run-id,
and `release`, which puts down the checkout this run created.

## What this draft leaves open

- **The name.** The method is still referred to by the inherited names, ODD and RDD, in the skill,
  the tools and the settings that retired them. Renaming is a contract change — an installed
  `protocols/rdd.md` must be retracted the way the retired SDD files were — and is not made here.
- **Principle 4 in the skill.** The stronger form of test-first is not yet shipped to other
  people's agents.
- **[`parity.md`](parity.md)'s framing.** It still reads gentle-ai as the target Estigia measures
  against. Once this method is settled, that document answers where each part came from and what a
  later gentle-ai release changes, not whether Estigia is at parity.
