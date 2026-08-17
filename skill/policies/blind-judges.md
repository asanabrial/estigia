# Blind judges

`single` requires one independent review context. `two blind` uses the two-context rules below;
`five blind` uses the additional quorum section. Neither changes the mandatory frozen publication
receipt or the one aggregate verdict Estigia requires before CI release.

The setting is orthogonal to `Planning` and to `Review protocol`. Under `standard` review the judges
read the published review target; under `receipt-driven` they read the frozen digest; when `Planning`
is `sdd` they read the change against the criteria `spec` recorded. The mechanism below is the same
in every combination.

## The mechanism

**One immutable target, independent judges in parallel, blind.**

Build and publish the complete clean target as a draft first. Every judge receives the same epoch, PR,
head, base, digest, bytes, scope and criteria, and is blind to every sibling's verdict and reasoning.
A later opinion that has read an earlier one is not independent. Claude's orchestrator launches the
same static `review-blind` definition for every judge; there are no numbered definitions.

All judges are **read-only** against that target. A judge that edits is no longer judging.

### Reserved reviewer role

`review-blind` is operator-owned. For Claude's current `Agent` and legacy `Task` launch surfaces, any
project definition under recursive `.claude/agents`, from the launch cwd through the first `.git`
repository root, with that filename or YAML-parsed frontmatter name invalidates the panel. Unreadable or
duplicate candidates also invalidate it, and the canonical user file must be the only user-scoped
definition with that identity and match Estigia's normalized embedded reviewer. Setup performs the same
user-tree uniqueness preflight before writing. A running reviewer is gated by the embedded policy, not
project-local bytes. A refused or unprovable launch contributes no judge. If the collision or canonical
file cannot be restored, use a separate session or durable handoff; never silently reduce or serialize
the panel. Ordinary agent names remain project-first, and this prelaunch check is Claude-only.

## What agreement buys

| Outcome | What follows |
|---|---|
| Both judges confirm a severe finding | fix it |
| One judge sees it, the other does not | record it as a suspicion; **do not fix it automatically** |
| The judges contradict each other | **stop in `blocked`, and put the contradiction to a named person** |

There is no tie-break and no majority. A majority of two is a coin toss wearing a process, and the
case where two careful readers disagree about the same bytes is exactly the case worth a human minute.
Before releasing ownership, record the exact decision or exit condition and its discharger, plus the
branch, PR, frozen receipt, both verdicts, and other evidence already built.

A finding only one judge saw is not discarded; it remains a suspicion with which judge saw it.
Warnings and suggestions are recorded and not acted on automatically.

### Five blind

Launch five independent reviewer contexts concurrently over the identical immutable target and criteria.
A 3-of-5 quorum independently confirming the same severe finding blocks acceptance and
authorizes automatic repair. One or two confirmations remain suspicions. Ambiguous finding identities
do not aggregate. Preserve dissent, warnings, suggestions and which judges confirmed each finding.
After either blind mode, record one aggregate exact-receipt verdict, not one marker per judge.

## The fix is bounded

A confirmed finding is fixed, and the fix produces an immutable delta that the judges read again —
the same freeze, the same blindness, the same rules.

**At most two fix rounds in one lineage.** A third round is not a fix; it is a change that has not
converged, and it goes to `blocked` for a named person's decision. Before releasing ownership, record
that decision or exact exit condition and discharger, the branch, PR, latest receipt, both rounds'
verdicts, and what the rounds learned. This is the same threshold the handoff procedure uses: hand off
after a known mistake repeats, after three hypotheses fail without narrowing, or when editing exceeds
measuring.

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the draft barrier, one aggregate exact-receipt verdict, the exact
publication receipt, and Claude's reserved-role prelaunch checks. Nothing proves panel execution.

That is worth stating first and plainly. Estigia gates repository writes against an adjudicated
claim. It cannot prove panel size, concurrency, independence, blindness, same-finding identity or
quorum. It cannot see whether a context read the target or whether a verdict was honest. There is no
hook that fires when a reviewer forms an opinion.

`publish_review` keeps ordinary compatible CI behind a draft PR and records epoch/PR/head/base/digest.
`release_ci` replays the globally latest receipt, current draft PR and coherent clean target before
marking ready. A republish invalidates earlier evidence even for identical bytes. These checks bind
the cooperative order to exact bytes; they do not prove who reviewed them. GitHub has no atomic
conditional-ready operation, and out-of-band collaborators or workflows can bypass the order or forge
evidence. There is no malicious-collaborator authenticity claim.

For Claude Code, setup always installs one stable inert reviewer definition where the host finds it;
the setting does not create or rewrite that file. The orchestrator remains responsible for passing the
effective judge model, activating the right number of contexts concurrently, withholding sibling
output, applying finding identity and quorum, and preserving dissent. That is a real contract, not
mechanical proof that it happened.

**What is still enforced around this:** the claim, the renewal before repository writes and at every
irreversible boundary, and a push no live claim authorises refused under git. Those hold whatever the
judges decide, because they come from the claim rather than from the review.
