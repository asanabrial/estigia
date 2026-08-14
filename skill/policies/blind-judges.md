# Two blind judges

Read this for both `Blind judges` modes. `single` requires one independent review context. `two blind`
requires the two contexts and agreement rules below. Neither mode changes the mandatory frozen
publication receipt.

The setting is orthogonal to `Planning` and to `Review protocol`. Under `standard` review the judges
read the published review target; under `receipt-driven` they read the frozen digest; when `Planning`
is `sdd` they read the change against the criteria `spec` recorded. The mechanism below is the same
in every combination.

## The mechanism

**One immutable target, two judges, in parallel, blind.**

Build and publish the complete clean target as a draft first. Both judges receive the same epoch, PR,
head, base, digest, bytes, scope and
the same criteria, and each is blind to the other's verdict and to the other's reasoning. Neither is
told what the other found, and neither runs after the other with the first result in hand — a second
opinion that has read the first is not a second opinion.

Both judges are **read-only** against that target. A judge that edits is no longer judging.

## What agreement buys

| Outcome | What follows |
|---|---|
| Both judges confirm a severe finding | fix it |
| One judge sees it, the other does not | record it as a suspicion; **do not fix it automatically** |
| The judges contradict each other | **stop, and put it to a person** |

There is no tie-break and no majority. A majority of two is a coin toss wearing a process, and the
case where two careful readers disagree about the same bytes is exactly the case worth a human
minute.

A finding only one judge saw is not discarded — it goes on the record as a suspicion, with which
judge saw it. Deleting it would make the second judge's work invisible whenever it disagreed, which
is the opposite of why there are two.

Warnings and suggestions are recorded and not acted on. They are information, and an automated fix
for a suggestion is a change nobody asked for.

## The fix is bounded

A confirmed finding is fixed, and the fix produces an immutable delta that the judges read again —
the same freeze, the same blindness, the same rules.

**At most two fix rounds in one lineage.** A third round is not a fix; it is a change that has not
converged, and it goes back to a person with what the rounds learned. This is the same threshold the
handoff procedure uses: hand off after a known mistake repeats, after three hypotheses fail without
narrowing, or when editing exceeds measuring.

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the draft barrier and exact publication receipt around the judges, but
nothing about whether the judges actually ran or were blind.

That is worth stating first and plainly. Estigia gates repository writes against an adjudicated
claim. It cannot see how many contexts read a change, whether they were blind, whether one waited for
the other, or whether a verdict was honest. There is no hook that fires when a reviewer forms an
opinion.

`publish_review` keeps ordinary compatible CI behind a draft PR and records epoch/PR/head/base/digest.
`release_ci` replays the globally latest receipt, current draft PR and coherent clean target before
marking ready. A republish invalidates earlier evidence even for identical bytes. These checks bind
the cooperative order to exact bytes; they do not prove who reviewed them. GitHub has no atomic
conditional-ready operation, and out-of-band collaborators or workflows can bypass the order or forge
evidence. There is no malicious-collaborator authenticity claim.

What the setting does is put this contract in front of the agent and record the choice where an
operator and a reviewer can both read it. That is a real thing — a rule nobody states is a rule
nobody keeps — and it is the whole of it.

**What is still enforced around this:** the claim, the renewal before repository writes and at every
irreversible boundary, and a push no live claim authorises refused under git. Those hold whatever the
judges decide, because they come from the claim rather than from the review.
