# Receipt-driven review inside the state contract

Read this before any review: review is receipt-driven, and this is not a configurable row. It adds to the Hard Rules, the Decision Gates and
the state contract rather than replacing them: the states still belong to the transport, and the claim
is still what gives a run authority.

What it changes is one thing, and it is a sharp one. **A review verdict authorises exactly the bytes
it was computed over, and nothing else.**

## The rule

Ordinary review names a head and a base and asks a reviewer to look. That is already strict — every
push invalidates it — but the *subject* is left implicit: whatever was at that head when the reviewer
happened to look, plus whatever was in the worktree and not committed.

Receipt-driven review makes the subject explicit and freezes it first:

| Step | State | What it does |
|---|---|---|
| freeze | `review` | `target` emits the complete intended target — the recorded base through `HEAD`, plus the uncommitted worktree — as a path/mode/blob manifest with **one digest** over it |
| classify | `review` | name the risk this change carries and which lenses the review must apply; a lens nobody named is a lens nobody applied |
| capture | `review` | each finding recorded against that receipt with its own identity, evidence, material impact and one of `severe`/`warning`/`suggestion`, then one computed exact-receipt verdict over epoch, PR, head, base and **that digest** |
| validate | every gate | re-derive the digest and compare; a difference stops the gate |

The digest is the receipt. It is not a file Estigia writes — it is what `target` returns,
and it belongs on the issue with the rest of the evidence.

## The gates

Five boundaries validate the receipt. Estigia holds two of them mechanically and the rest are the
run's discipline, and the difference is worth stating plainly because a gate nobody holds is a habit,
not a gate.

| Gate | Who holds it |
|---|---|
| post-apply | the run — re-derive after the last edit, before asking for review |
| pre-commit | the run |
| pre-push | **Estigia's `pre-push` git hook**, which refuses a push no live claim authorises, whoever typed it |
| pre-pr | **Estigia's tool gate** — `gh pr create` is an irreversible boundary and re-verifies the claim against the store |
| release | **Estigia's tool gate** — `gh release create` and `gh release edit` are irreversible boundaries |

At the boundaries Estigia holds, what it verifies is the *claim*, not the digest. It cannot compare a
receipt it never took. `release_ci` does compare it, twice: the supplied receipt must equal the
recorded one, and the digest re-derived from the clean target must equal it, or `release-target-mismatch`
refuses. Every other gate leaves that comparison to `target` and to the run.

## When the base moves

A receipt is bound to a base as well as a head. `target` with `recorded_base` classifies later movement as none,
compatible, overlapping, conflicting or unknown — and **unknown is not compatible**. The answer's
`integrate` field is the movement's own reading, never a verdict: it reports what the movement did,
not what the change still means.

Under `required_status_checks.strict: true` — the protection this repository's delivery assumes, and
which nothing reads back — GitHub requires the topic branch to be up to date with the base before
merging, so every movement but none forces an integrating republish: `"integrate": false` on a
compatible movement is not a route a run can take while that setting is on. If it is off, the
movement's own answer stands. What the
forced integration costs depends on the classification:

- `compatible` — disjoint paths and a conflict-free merge — is a **repair-shaped republish**: merge
  the base so the parent head survives, publish over the parent receipt, and hand the lenses the
  findings recorded against the parent receipt, for them to re-read. The range still carries the base's
  advance, so the tier is the whole change's own (`whole_change_reason: "base-advanced"`) and never the
  delta lens, and the correction budget is measured against the tree git merges the parent head and
  the new base into rather than reset. The republish follows the base rather than the author: over an approved parent it is a new
  candidate and spends no correction, and over a rejected one it is one of the lineage's corrections, which also owes the targeted validator's result. A receipt that stands escalated is recovered only by a person running `estigia recover` — the gate refuses an agent's line for it — and the successor lineage that follows is reviewed again under the same tier and lens focus. A rebase rewrites the parent head, so no honest delta survives it and the panel reads the
  whole change, the same as a merge does.
- every other value — overlapping, conflicting or unknown — integrates and re-reviews the whole
  change, because the movement touches what this delivery touches or could not be established.

A base that moved under a computed verdict invalidates it exactly as a push would; the forced
integration above is how the verdict is earned again, and reasoning about whether the change
"probably" still applies is not that.

Do not rebase as a routine step before review: it rewrites SHAs that reviews, CI and signatures
already name, and can force-push over refs another actor fetched (X07).
Merging: `repository-delivery.md`.

## Review is not switchable off here

Other tools that implement this pattern offer a disabled mode where nothing gates and delivery falls
back to ordinary repository policy. That mode does not exist under this contract: review is required
by the state contract. This run, or a separately started one, obtains the second context, and nobody is
asked for consent first: the run states the `forecast` line and proceeds. No consent and no off switch.

The invariant those tools state for their disabled mode is still worth keeping, because it is the
same one this whole contract is built on: *an unreadable switch is not a disabled switch*. A control
surface that cannot be read permits no write, and an unknown result is not clearance.

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the claim; the renewal before repository writes and at every irreversible
boundary; one run holding one task; the state the tracker reports; a push that no live claim
authorises, refused under git; one computed exact-receipt verdict bound to the latest publication,
which `release_ci` recomputes over the live ledger rather than trusting its marker; the retirement
of the aggregate verdict, so that no reviewer's word is one; and, since the finding ledger exists, that
every finding names the publication under review rather than merely a well-formed one, and that a repair's **severe** finding either names a parent
its parent receipt recorded or states whether the repair `introduced` or `exposed` the defect. A
warning or a suggestion new to a repair is asked for neither, which is deliberate and pinned:
requiring an origin of every class reddens the lineage test.

**Not enforced:** whether the lenses named were the right ones, whether a lens report means the
lens ran, or whether a finding's class and evidence class were honest. There is no per-judge
verdict and no panel transcript, and nothing here counts findings into a quorum.
Structured *finding* evidence, which used to be on that list, is not any more — there is one marker
per finding now, credited to a named reviewer and bound to the exact receipt. What is adjudicated is
shape and reference: that a class is one of three words, that evidence and impact were stated, and
that a receipt or a parent is the one it claims to be. Whether a `severe` finding is severe, whether
the evidence reproduces, and whether a repair introduced what its reviewer says it did are the
reviewer's claims. Estigia cannot see whether a reviewer actually read the receipt. A tool claiming
otherwise would be the false comfort this contract is written against.

The decision tables those classes are read by are in
[`policies/blind-judges.md`](../policies/blind-judges.md); this protocol decides what identifies the
target, not how many contexts read it.
