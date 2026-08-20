# Blind judges

`single` requires one independent review context. `two blind` uses the two-context rules below;
`five blind` uses the additional quorum section.

**This document is routed by `two blind` and `five blind` only.** A run configured `single` is never
handed it, so everything the `single` table below states is also stated in the contract itself, and
the table here is a restatement rather than the only copy. Issue #18 owns that reachability gap; what
matters for a reader of *this* file is that finding the rule here does not mean a `single` run saw
it.

No mode changes the mandatory frozen publication receipt or the one aggregate verdict Estigia
requires before CI release.

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

A judge launched under the reserved `review-blind` role is **read-only** against that target: the
role's gate refuses every write, edit, shell and delegation tool, so such a judge cannot mutate the
target even if its prompt asks it to. A judge in that role that edits is no longer judging. Panels
launched under any other subagent type are governed by the isolation rule two sections below, not by
this sentence.

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

### Panels run outside the reserved role

A verdict is sometimes only worth what it measured. In a repository whose evidence standard is
mutation — where confirming a finding means building the change, reddening a test, or turning the fix
off to see the suite go red — the judge that measures cannot be the reserved reviewer, because that
role is gated read-only and refuses the tools a measurement needs. Such a panel is launched under
another subagent type, which places it outside the prelaunch check and outside the role gate above.
It is not thereby ungoverned: this section is the rule that governs it.

**Two judges run outside the reserved role are never pointed at one working directory.** Each judge
run outside that role is given a directory nothing else writes for the duration of its review — not a
sibling judge, not the orchestrator, not another run. Two measuring judges in one directory are
concurrent writers: each reads the other's edits, rebuilt artefacts and test output as though they
were the target's, so neither verdict is a reading of the bytes that were published and neither can
be reproduced. A panel that cannot be given one such directory per judge runs fewer judges; it does
not share one.

A judge that finds the target already dirty **stops and reports it rather than restoring it**. Whatever
made it dirty was somebody's measurement in progress, and restoring is a write like any other: it
destroys that measurement, races whoever else is repairing the same directory, and leaves the head that
gets delivered as whatever survived. Reporting a dirty target costs one verdict. Quietly repairing it
costs the evidence that anything was wrong.

Where those directories come from is not decided here. [Repository delivery](../references/repository-delivery.md)
already owns run isolation — checkout paths, what makes a checkout this run's own, and per-checkout
caches — and this rule adds a reason to obey that document rather than a second copy of it.

Isolation buys reproducibility, not blindness. A judge's reading tools are unrestricted over whatever
the directory it is handed contains, in the reserved role and outside it alike, so blindness under
any shared target rests on this prose and on what the orchestrator gives each judge, and nothing
measures it. Nor does this rule reach a published head a judge mutated: the review target is
re-derived from a coherent clean snapshot and a dirty path is refused before publication or release,
so that outcome is a blocked release rather than a bad delivery — a refusal that already existed and
is not produced by anything written here.

## What a finding is

A verdict is one bit and a review is not. Every observation worth carrying is recorded as its own
finding with `review_finding`, bound to the exact publication receipt, before the aggregate verdict is
recorded. Each one carries an identity, concrete evidence, stated material impact, and exactly one
class:

| Class | What it means | What it costs |
|---|---|---|
| `severe` | A reproducible correctness, security, reliability or contract defect with concrete user or delivery impact | It can block, and only it can |
| `warning` | A bounded risk with no demonstrated incorrectness | Recorded beside the verdict; never withdraws an acceptance |
| `suggestion` | Optional improvement, cosmetic wording, style, preference | The same |

**Cosmetic, style and preference observations are `suggestion`** unless they identify a concrete
defect, in which case say which one and classify it as that.

**The precision gate.** If concrete evidence and material impact cannot both be stated, omit the
observation or keep it as a non-blocking note. A classification with no evidence cannot be re-run and
one with no stated impact cannot be weighed, and either is a preference wearing the word `severe`.
`record_review_finding` refuses a finding missing any of the three, and refuses one that names any
receipt but the publication under review.

**The identity is chosen from the affected behaviour and location**, never from the run or the
wording. That is what lets two judges reporting the same defect count as agreeing rather than as two
defects, and what lets a repair name what it answers.

### Single

| What the reviewer found | The verdict |
|---|---|
| At least one `severe` finding | `rejected` |
| Only warnings and suggestions | `accepted`, with those notes preserved against the receipt |
| Nothing | `accepted` |

**One direction of that table is mechanically enforced**, and it is worth being exact about which.
`record_review_verdict` refuses `rejected` unless that reviewer has already recorded a `severe`
finding against the exact receipt — the reviewer's own findings, not the panel's pool, because two
contexts each holding one suspicion is not one confirmed defect. **Nothing refuses `accepted` over a
severe finding.** Row 1 of the table is a rule for reviewers; rows 2 and 3 are what the transport
holds them to. A reviewer that accepts over its own severe finding is making an error the harness
cannot see.

**Who the verdict credits, when a panel produced it.** The rule reads *that reviewer's* findings, and
a panel records one aggregate verdict — so the two have to be reconciled somewhere, and it is here.
Record each judge's findings under **that judge's** identity, which is what makes the same-`id`
agreement count meaningful. Then credit the aggregate verdict to a judge whose severe finding the
outcome rests on. Re-recording one judge's finding under a panel name would satisfy the rule and
inflate the agreement count at the same time, which is the one thing this arrangement must not do.

Operational failures are not review findings and keep their own fail-closed refusals: a missing
reviewer, an unreadable target, a stale receipt or a target mismatch is a hold, and recording one as a
cosmetic acceptance is the mislabelling this table exists to stop.

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

*Confirm* means both judges recorded a `severe` finding **with the same identity** against the same
receipt. Two severe findings with different identities are two findings, each with one confirmation;
under this mode neither authorises an automatic repair. Estigia records each judge's findings and
counts nothing: quorum is the orchestrator's to apply, and the section at the end of this document
says so rather than implying otherwise.

### Five blind

Launch five independent reviewer contexts concurrently over the identical immutable target and criteria.
A 3-of-5 quorum independently confirming the same severe finding blocks acceptance and
authorizes automatic repair. One or two confirmations remain suspicions. Ambiguous finding identities
do not aggregate. Preserve dissent, warnings, suggestions and which judges confirmed each finding.
After either blind mode, record one aggregate exact-receipt verdict, not one marker per judge.

## The fix is bounded

A confirmed finding is fixed, and the fix produces an immutable delta that the judges read again —
the same freeze, the same blindness, the same rules.

### What a repair carries

Republishing mints a new epoch and invalidates every prior verdict, which is what stops reviewed bytes
being delivered as unreviewed ones. What it must not also do is throw away what the last round
settled.

A publication over an earlier one records, **derived from the timeline and never supplied by the run
being reviewed**, the whole parent receipt — epoch, PR, head, base and target digest — and a delta
digest covering both ends. A run that could name its own parent could name the epoch whose findings
were mildest. `parent-head..head` is the delta; Estigia records the two ends and does not compute the
diff, scope the review to it, or check that a judge read only it.

The receipt rather than the epoch, and that is a repair rather than a flourish. An epoch is not a
function of the bytes it names, and a finding's epoch field is whatever the finding says it is — so a
parent ledger matched on the epoch alone could be written into after the fact, by recording a finding
that named the parent epoch and carried the repair's own bytes. Matched on the receipt, it cannot.

A lineage reaches **one publication back**, and that bound is worth knowing before a third round
needs it: `--parent` is matched against the ledger of the receipt this publication descends from,
not against every ancestor. A severe finding raised in round one and not re-recorded in round two
cannot be cited in round three — the reviewer is told which identities the parent receipt holds
and records it as new, with an origin. Each round that still stands on a finding re-records it and
the chain holds; a round that lets one lapse ends it.

The parent's findings stay on the timeline. Preserving settled work is not an operation — nothing
rewrites those markers — so what a re-review owes is the *reference*:

| The finding | What it must say |
|---|---|
| Reassesses one the parent ledger holds | `--parent <that identity>`, which must exist against the parent **receipt** |
| Is `severe` and new to this repair | `--origin introduced` if the repair created it, `--origin exposed` if the repair made an existing defect reachable |
| Is a warning or a suggestion new to this repair | Nothing. Pricing the cheap observation is the defect this whole mechanism repairs |

The **origin** of a new **severe** finding is refused when absent, which is what keeps a full-target resweep that rediscovers
or rephrases settled work from arriving indistinguishable from a defect the repair caused. The
parent reference cannot be refused when absent — a finding that silently declines to name what it
reassesses is indistinguishable from a new one, and nothing here can tell them apart. What Estigia adjudicates
is that the reference exists and the origin is one of the two words. Whether it is *true* is the
reviewer's claim, like every other field here.

Delivery identity is untouched by any of this. `release_ci` still re-derives the coherent clean target
and matches the complete current head, base and digest. A lineage says what a repair descends from; it
never becomes the thing that is delivered.

**At most two fix rounds in one lineage.** A third round is not a fix; it is a change that has not
converged, and it goes to `blocked` for a named person's decision. Before releasing ownership, record
that decision or exact exit condition and discharger, the branch, PR, latest receipt, both rounds'
verdicts, and what the rounds learned. This is the same threshold the handoff procedure uses: hand off
after a known mistake repeats, after three hypotheses fail without narrowing, or when editing exceeds
measuring.

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the draft barrier, one aggregate exact-receipt verdict, the exact
publication receipt, Claude's reserved-role prelaunch checks, the **publication lane refusal** —
`record_review_verdict` will not bank an accepted verdict for a head whose dispatched CI lane is red or
still running — the **severity rule**, which refuses `rejected` without that reviewer's own severe
finding against that receipt, the **receipt currency**, which refuses a finding naming any receipt but
the one under review, and the **lineage references**, which refuse a parent identity the parent
receipt never recorded and a new severe finding against a repair that states no origin. Nothing
proves panel execution.

That is worth stating first and plainly. Estigia gates repository writes against an adjudicated
claim. It cannot prove panel size, concurrency, independence, blindness, same-finding identity or
quorum. It cannot see whether a context read the target, whether a verdict was honest, or whether two
judges running outside the reserved role were given separate working directories. There is no
hook that fires when a reviewer forms an opinion.

The findings are the same kind of evidence and carry the same limit. Estigia adjudicates that a class
is one of three words, that evidence and impact were stated, that a receipt is the current one and that
a named parent exists. It cannot check that a `severe` finding is severe, that a `suggestion` is not a
defect in disguise, that the evidence reproduces, that the impact is real, or that a repair `introduced`
what its reviewer says it introduced. A structured claim is still a claim; what changed is that it is
now legible enough to be argued with.

`publish_review` keeps ordinary compatible CI behind a draft PR, records epoch/PR/head/base/digest, and
starts one publication-lane run against the head it pushed. That last one is why the refusal above is
mechanical rather than a rule somebody has to remember: the signal is collected at publication and the
gate sits on the single writer of a verdict marker, not in the prompt that launches the judges. It is
not a proof that a lane exists — a repository without a dispatchable one has no check runs on its
head, and that proceeds.
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
