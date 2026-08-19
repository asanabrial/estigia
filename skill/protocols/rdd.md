# Receipt-driven review inside the state contract

Read this when `Review protocol` is `receipt-driven`. It adds to the Hard Rules, the Decision Gates and
the state contract rather than replacing them, and it composes with whatever `Planning` is set to: the states still belong to the
transport, and the claim is still what gives a run authority.

What it changes is one thing, and it is a sharp one. **A review verdict authorises exactly the bytes
it was given, and nothing else.**

## The rule

Ordinary review names a head and a base and asks a reviewer to look. That is already strict — every
push invalidates it — but the *subject* is left implicit: whatever was at that head when the reviewer
happened to look, plus whatever was in the worktree and not committed.

Receipt-driven review makes the subject explicit and freezes it first:

| Step | State | What it does |
|---|---|---|
| freeze | `review` | `expected-target` emits the complete intended target — the recorded base through `HEAD`, plus the uncommitted worktree — as a path/mode/blob manifest with **one digest** over it |
| classify | `review` | name the risk this change carries and which lenses the review must apply; a lens nobody named is a lens nobody applied |
| capture | `review` | each finding recorded against that receipt with its own identity, evidence, material impact and one of `severe`/`warning`/`suggestion`, then one aggregate exact-receipt verdict over epoch, PR, head, base and **that digest** |
| validate | every gate | re-derive the digest and compare; a difference stops the gate |

The digest is the receipt. It is not a file Estigia writes — it is what `expected-target` returns,
and it belongs on the issue with the rest of the evidence.

## Why the digest and not the SHA

A commit SHA says what was committed. It does not say what was *in the tree* — a file made
executable, a file replaced by a symlink, or an uncommitted change sitting in the worktree all ship
without moving the SHA. `expected-target` treats mode as part of the identity and hashes a symlink as
its link text, which is what mode `120000` actually stores.

So: two runs can share a head SHA and deliver different bytes. The digest cannot be shared that way,
which is the entire reason for taking one.

## The gates

Five boundaries validate the receipt. Estigia holds two of them mechanically and the rest are the
run's discipline, and the difference is worth stating plainly because a gate nobody holds is a habit,
not a gate.

| Gate | Who holds it |
|---|---|
| post-apply | the run — re-derive after the last edit, before asking for review |
| pre-commit | the run |
| pre-push | **Estigia's `pre-push` git hook**, which refuses a push no live claim authorises, whoever typed it |
| pre-pr | **Estigia's tool gate** — `gh pr create` is an irreversible boundary and re-reads the timeline |
| release | **Estigia's tool gate** — `gh release create` and `gh release edit` are irreversible boundaries |

At the boundaries Estigia holds, what it verifies is the *claim*, not the digest. It cannot compare a
receipt it never took. Comparing the digest is `expected-target`'s job and the run's, at each gate.

## When the base moves

A receipt is bound to a base as well as a head. `base-movement` classifies later movement as none,
compatible, overlapping, conflicting or unknown — and **unknown is not compatible**. A base that
moved under a captured verdict invalidates it exactly as a push would; re-freeze and re-review rather
than reasoning about whether the change "probably" still applies.

Do not integrate the base as a routine step before review. Rebasing rewrites SHAs already named by
reviews, CI, signatures and deployments, and can require a force-push over refs another actor
fetched. That is X07 in the incident ledger.

## Review is not switchable off here

Other tools that implement this pattern offer a disabled mode where nothing gates and delivery falls
back to ordinary repository policy. That mode does not exist under this contract: review is required
by the state contract, and `Review delegation` decides only **who obtains the second context** — this
run, or a separately started one — never whether a second context is obtained at all.

The invariant those tools state for their disabled mode is still worth keeping, because it is the
same one this whole contract is built on: *an unreadable switch is not a disabled switch*. A control
surface that cannot be read permits no write, and an unknown result is not clearance.

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the claim; the renewal before repository writes and at every irreversible
boundary; one run holding one task; the state the tracker reports; a push that no live claim
authorises, refused under git; one aggregate exact-receipt verdict bound to the latest publication;
and, since the finding ledger exists, that a `rejected` verdict rests on a `severe` finding that
reviewer recorded against that same receipt, that every finding names the publication under review
rather than merely a well-formed one, and that a repair's finding either names a parent its parent
receipt recorded or states whether the repair `introduced` or `exposed` the defect.

**Not enforced:** per-lens or per-judge verdicts, whether the lenses named were the right ones, or
whether the aggregate verdict was honest. A per-judge verdict marker remains
future design, and so does the panel transcript it would build; the aggregate marker is neither, and
nothing here counts findings into a quorum.
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
