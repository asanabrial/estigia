# Blind judges

Ordinary review here is the lenses `review_tier` licenses — none at `low`, one at `medium`, four at
`high` — plus the refuter when a standing finding owes one, at most the `Corrections per lineage` corrections and each one's targeted
validator. It acquires no judge at any tier: `review_tier` answers no judge count, and the computed
verdict `release_ci` requires reads lens reports and the finding ledger alone. This document states
ordinary review's settlement — what a finding is, what the ledger computes, refutation, the one
correction and its validator — because that is what `skill/SKILL.md` developer step 5 routes to.
Judgment Day is the separate, opt-in method in which judges exist, and it loads only when the operator
explicitly asks for it — see [judgment day](judgment-day.md).

Review is receipt-driven: the lenses read the frozen digest, and a review verdict is bound to a
digest over the complete intended target. `Review protocol` was the only row behind this and is now
retired, so this holds regardless and an installed row loads and changes nothing. The mandatory
frozen review transaction and the computed verdict `release_ci` requires hold regardless.

## Ordinary review

Lenses only. This section and every one after it is what a lens's findings are, how the ledger settles
them and what a correction costs; none of them convenes a judge.

### Lens acquisition

`review_tier` answers a lens count beside its tier, and `lens_roles` names what it licenses. The
verification table: `low` is structural readback only and acquires none; `medium` acquires one lens, the `focus`'s —
`risk`, `resilience`, `readability` or `reliability`, default `reliability`, with `resilience` filled by
`review-readback`; `high` acquires four lenses, `review-risk`, `review-readback`,
`review-readability` and `review-reliability`. `review_tier`'s `raise` lifts a tier to `high` for a risk the
diff cannot show and never lowers one: `authorization`, `migration`, `input`, `concurrency`, `irreversibility` or
`contract`, with a reason, the binary (`RISK_ITEMS`) being the authority on the list. `publish_review` records the raise and the gate reads it. A tier that cannot be read is `high`, never
permission to acquire less. `review-lens` is in no tier's allocation: only the delta-lens republish
below launches it. A lens is not a judge, and no tier acquires a judge. The one round that runs fewer lenses than
the tier's own is the republish licensed by a `warning` or a `suggestion` the repair-loop section
below defines, and a `severe` finding that round's lens records, whatever its evidence class, buys
the repair the tier's own lenses rather than a confirmation it has none to give.

Every acquired lens runs concurrently, each sealed to the identical diff and criteria and holding no
`Read` — it cannot fetch anything the launch did not hand it, so the binary, which launches it,
carries every byte a lens reviews. An `inferential` lens finding is answered by the refuter below and by nothing else.

The orchestrator calls `launch_review_input` for each lens slot `review_status` offers, as a `collect`
input, for a lens a receipt still owes; the binary starts the reviewer process itself, from a template embedded in the binary — the first installed `claude` or `codex` CLI, with
no tools (an OpenCode run with its relay plugin installed relays instead, launching the built-in sub-agent and marker prompt the `relay: true` answer's `launch` names) — and records the findings and then the lens report, or records nothing when any finding is
outside the change. On Claude Code there is no hand launch: setup installs no lens definition and the
launch gate refuses a `Task` launch of a lens name.

`--lens` names one of the five shipped roles or nothing; a name outside that list is refused
rather than minted from caller text. Every lens finding owes a `location` whose path `base..head`
actually changed, checked on the path alone and never the line — a lens may anchor any line of a
changed file — and a finding naming a path the diff never touched is refused. **That is the floor,
not the whole rule for every lens finding:** one that is also `severe` with `causal_disposition`
`introduced` or `worsened` owes the same containment any severe causal finding owes, checked
against the line too — not a lens-specific exception, the one rule applied to both.

## What agreement buys

Under Judgment Day, two judges agreeing on a finding is that finding's corroboration and nothing
more: nothing is recorded, no ledger counts it and no verdict reads it. Neither judge can veto the
other. Preserve dissent, warnings, suggestions and which judge reported each finding, and report them
to the operator. A finding worth separate work is filed as an issue; it is never recorded against a
receipt.

In ordinary review nobody agrees with anybody. A severe finding a lens records is settled by the
ledger at every tier, in this order: a missing `evidence_class` or `causal_disposition`,
`insufficient` evidence or an `unknown` cause escalates; a `pre-existing` or `base-only` cause is a
follow-up that blocks nothing; `deterministic` is open and rejects; `inferential` follows its refuter
(`refuted` drops it, `corroborated` or `inconclusive` keep it, no result escalates). Estigia counts no
agreement: preserve dissent, warnings, suggestions and which lens recorded each finding. Before
releasing ownership, record the exact decision or exit condition and its discharger, plus the branch,
PR, frozen receipt, every finding and lens report recorded, and other evidence already built.
Warnings and suggestions are recorded and not acted on automatically.

## What a finding is

This section and the ones after it settle ordinary review's findings. A Judgment Day judge words its
observations in the same three classes and records none of them.

A verdict is one bit and a review is not. Every observation worth carrying is recorded as its own
finding with `review_finding`, bound to the exact review transaction, before the computed verdict is
recorded. Each one carries an identity, concrete evidence, stated material impact, and exactly one
class:

| Class | What it means | What it costs |
|---|---|---|
| `severe` | A reproducible correctness, security, reliability or contract defect with concrete impact | It can block, and only it can |
| `warning` | A bounded risk with no demonstrated incorrectness | Recorded beside the verdict; never withdraws an approval |
| `suggestion` | Optional improvement, cosmetic wording, style, preference | The same |

**Cosmetic, style and preference observations are `suggestion`** unless they name a concrete
defect, in which case say which one and give it that defect's class.

Incorrect or stale comments and documentation are observations about the words beside the
behaviour, not about the behaviour itself, and they are **never `severe`** — not under a demonstrated
cost, not under a reader who might be misled, not under any condition. A demonstrated mismatch is a
`warning`, editorial drift alone is a `suggestion`, and no fourth word appears between the three.
They stay defects `AGENTS.md` requires repaired in the same change; what they never do is force a
republication and a fresh panel.

**Text a running agent obeys is not documentation about behaviour — it is the behaviour.** Where a
repository ships instructions its own agents execute, a contradiction between two of those
instructions is a contract defect and the never-`severe` rule above does not soften it. The carve-out
is narrow: it is gated on a contradiction between two instructions a repository's own agents execute,
not on staleness, and it does not reach a stale comment or a stale document the never-`severe` rule
above already covers.

**A launch prompt does not get to restate this.** A reviewer definition tells the reviewer to prefer the
definition's rule over any classification a prompt supplies, and to record the conflict. A brief
that means to carry this rule points at the installed definition — the reviewer's own
file under `skill/agents/` — rather than paraphrasing it, because a paraphrase can only narrow
the rule it restates. Nothing mechanical detects a brief that overrides it — a sub-agent's prompt
is not a write the gate decides on — so the reviewer is the only place this can be held.

**The precision gate.** If concrete evidence and material impact cannot both be stated, omit the
observation or keep it as a non-blocking note. A classification with no evidence cannot be re-run and
one with no stated impact cannot be weighed, and either is a preference wearing the word `severe`.
`record_review_finding` refuses a finding missing any of `id`, `evidence` or `impact` — or naming
`evidence` or `impact` as filler that says nothing (`n/a`, `TODO`, `placeholder`, and their kin),
refused the same as an absent one — and refuses
one that names any receipt but the publication under review. Every `path:line` or `path:start-end`
citation `evidence` or `impact`'s prose contains is also resolved against the receipt's `base` and
`head`, and refused as `finding-citation-unresolvable` under one of four causes — `malformed-path`
(the path is absolute, escapes the repository, or carries a colon or a `..`), `path-not-in-either-tree`,
`path-is-not-a-file`, or `line-past-end-of-file` — the free text is checked, not merely stored. The
lookup reads both trees from the recording checkout, so a run that has not fetched the receipt's
`base` and `head` fails with a git read error rather than recording, under any class. It resolves
only tokens that look like a path citation; the six shapes it skips, including a raw trailing slash
and an unquoted extensionless root-level path such as `Makefile:12`, are enumerated in the
`finding-citation-unresolvable` refusal.

**The precision gate and the causality rule are also in the shipped `review-blind` definition, in its
own words, and the duplication is deliberate; the citation resolution and the severe finding's
vocabulary are here alone.** This document is routed to whoever runs a review; the definition is what a reviewer
starts with. Estigia installs that definition on Claude Code alone — `agent_definitions_root` is
`Some` for Claude Code and OpenCode now, but the loop that writes reviewers is gated on
`ClaudeCode` — so on the other hosts the machine receives no definition, so this document is what
an orchestrator has. Keeping one copy would have left the rule reachable on one adapter or on none.
The definition is the authority where they overlap and disagree, and a disagreement is a defect in this file.

**A severe finding declares its evidence and its cause.** `severe` is the one class that can block,
so it owes two words the others do not: an `evidence_class` (`deterministic`, `inferential`,
`insufficient`) saying how the claim was established, and a `causal_disposition`
(`introduced`, `worsened`, `pre-existing`, `base-only`, `unknown`) saying who owns the defect on
this timeline. Whether those words are true is the reviewer's claim like every other field here;
that they are present and in vocabulary is refused by the recorder rather than left to this prose,
because a rejection rests on this class alone and a severe finding missing either how it was
established or whose defect it is arrives unweighable by exactly the rule above. A `pre-existing`
or `base-only` answer is a follow-up, recorded and not acted on — what blocks this change is a
defect *this* change answers, and it is filed as its own issue under **The fix is read again**
below; an `unknown` answer escalates the receipt to a person, and is filed too, where only these
three classes are filed by right; a warning or a suggestion is informational there instead.
Warnings and suggestions state neither; asking them to would price the cheap observation out again,
which is the reason these words sit on one class only.

`evidence_class` also decides which severe findings the refutation pass owes an answer for, stated
in full under **Refutation** below: `inferential` is the population that pass exists for, and
`deterministic` skips it entirely.

A severe finding whose `causal_disposition` is `introduced` or `worsened` owes a third word:
`location`, `path:line` or `path:start-end` with the path repository-relative — the one claim
`base..head` can actually settle. A malformed value is refused under any class or disposition;
falling inside a changed hunk of that path is checked only here, and a path this change touched
only by deletion still settles, since no such hunk adds a new-side line to fall inside.

**The identity is chosen from the affected behaviour and location**, never from the run or the
wording. That is what lets two reviewers reporting the same defect count as one defect rather than as
separate defects, and what lets a repair name what it answers.

### What the ledger computes

The verdict is computed from the receipt's findings, never chosen by a reviewer:

| What the ledger holds | The computed verdict |
|---|---|
| A `severe` finding the ledger cannot settle — evidence class or cause missing, `insufficient` or `unknown`, an `inferential` one no refuter has answered | `escalated`, to a person |
| A `severe` finding that stands — `deterministic`, or `inferential` and not refuted — and was introduced or worsened here | `rejected` |
| Only warnings, suggestions, follow-ups and refuted findings | `approved`, once every lens the tier licenses has reported |

Each severe finding, read by its first severe recording as the refutation batch reads it, is settled by
the first row it matches: a missing `evidence_class` or `causal_disposition`, `insufficient` evidence or
an `unknown` cause is **escalated**; a `pre-existing` or `base-only` cause is a **follow-up** and blocks
nothing; a `deterministic` finding is **open**; an `inferential` one follows its refuter — `refuted` drops
it, `corroborated` and `inconclusive` keep it open, and no refuter result escalates it. A finding that is
not severe never blocks. `escalated` outranks `rejected`, and `rejected` outranks `approved`; a `rejected` verdict never rests
on a severe finding that declares `insufficient` evidence. The computation does not see
what no finding records. A lens that finds a severe defect records it as a `severe` finding with its
evidence class and cause, and that is the whole of its say: there is nothing to accept or reject, and
no tool records an aggregate verdict. **Nothing refuses a lens that records a warning for
what is a severe defect.** The class is the reviewer's claim, and a reviewer that understates it is
making an error the harness cannot see.

Record each lens's findings under **that lens's** own name. Re-recording one lens's finding under a
panel name would misstate who found it.

Operational failures are not review findings and keep their own fail-closed refusals: a missing
reviewer, an unreadable target, a stale receipt or a target mismatch is a hold, and recording one as a
cosmetic suggestion is the mislabelling this table exists to stop.

### Refutation

A rejection can rest on a mistaken severe finding, and a full repair round is an expensive way to
find that out. Before that round starts, every standing severe finding whose `evidence_class` is
`inferential` goes to a distinct `review-refuter` context — sealed, holding no `Read`, adding no
findings of its own and writing nothing itself — in **one pass per receipt**, whether the batch holds
two candidates or twenty: never one launch per candidate. It receives the exact review transaction,
the diff, and the candidates themselves, and returns exactly one of `corroborated`, `refuted` or
`inconclusive` for each.

The recording run does not launch the refuter and does not record the batch. `review_status` offers
the batch as a `collect` input, `refutation-batch`, whose `capture` is `launch_review_input`, and that
tool launches the refuter itself — the first installed `claude` or `codex` CLI (the relay on an OpenCode run with its plugin installed), with no tools, handed
the exact receipt, the diff and the listed candidates — and records its answer in one write credited to
`review-refuter`, or records nothing when the answer names a finding that was not offered or leaves
one out. Its distinctness is that of a fresh, isolated context, not of a separate authoring session.
There is no hand path: setup installs no `review-refuter` definition, the launch gate refuses a `Task`
launch of it, and a refutation is recorded only by capture: no tool records one by hand.

A batch never holds a `deterministic` severe finding — that class already qualifies a rejection with
no confirmation owed, so naming one in the batch is refused, and so is omitting a standing
`inferential` finding, naming one twice, or naming an id this receipt recorded no severe finding under.

The computed verdict reads the batch. A `refuted` finding drops out of the ledger and no longer
counts toward `rejected`. `corroborated` and `inconclusive` change nothing: the finding stays open
exactly as it would if no refutation batch had ever touched it. An `inferential` severe finding no
batch has answered escalates the receipt, which is what the batch exists to prevent.

This is the only confirmation an `inferential` finding gets in ordinary review: no judge confirms a
lens finding. The batch adjudicates findings a lens has already produced and a rejection already rests
on, and it decides whether an `inferential` severe finding blocks.

### Recovering an escalated review

A receipt that stands escalated while its lineage still has a correction left may be corrected like a
rejected one. One whose lineage has no correction left, or that `escalate_review` handed to
a person is final until a maintainer says otherwise, and saying so is a
**person's command** that no agent runs: the agent tells the person the exact command and waits. The
command and the maintainer check are in *Recovering an escalated review* in `bindings/github/operations.md`.
It mints a **successor lineage**: the next publication on the pull request is
a fresh candidate with a new correction allowance and its own budget, reviewed again — recovering is
not a release, and `release_ci` still needs an approved computed verdict on the successor's receipt.
The successor keeps the frozen policy of the lineage it replaces: its tier is at least the one that
lineage was reviewed at, and its lens focus is the same one. A correction published on a new pull
request over a rejected parent on another is a new change, not a correction, so it owes no validator
and no recovery.

### The targeted validator

A correction — a publication over a parent that stands rejected or escalated — is read by one more
role than any other publication: `review-validator`, sealed and read-only, holding no `Read`, adding no
finding and writing nothing, handed the exact correction receipt, the correction's diff over its
parent and the parent receipt's severe findings. It answers two questions. `original_criteria`: did the correction fix the frozen findings? `correction_regression`:
did it break anything on the corrected paths? `review_status` offers it as soon as the correction's
receipt exists and its result is still owed, provided its diff over the parent is the correction's
own, in the same `collect` that names any of the correction's lens reports still missing — the
validator reads neither their output nor the panel's, so it does not wait for them. When the recorded
`delta-range-reason` says that diff carries the base's advance or a merge (`"base-advanced"`,
`"merged-range"`) and the publication recorded `delta-parent=integrated` — the base advanced, the head
contains it, and `git merge-tree` merges the parent head and that base without a conflict — the
correction's own diff is the one from that merged tree to the head, and the validator is offered over
it. When the history was rewritten (`"history-rewritten"`), or the merge does not integrate cleanly,
`review_read` with `view: verdict` answers `validator-delta-carries-upstream` and `review_status`
offers no validator. `launch_review_input` launches it, as the `review-validator` input of that
`collect`: the first installed `claude` or `codex` CLI, with no tools, handed the exact receipt, the
correction's diff over its parent (`parent.head..head`, or the merged tree to the head when the
correction integrated a moved base; not the whole change) and the parent receipt's severe
findings — and
records its answer, or records nothing when the answer is not a well-formed inspection. The attempt is
the binary's: the inconclusive results already recorded plus one. On Claude Code the validator has no
hand launch: setup installs no `review-validator` definition and the launch gate refuses a `Task`
launch of it. It is **additional** to the tier's own lenses, not instead of them: a
smaller panel for a correction is a decision this slice does not take. Those lenses, and the refuter that judges their findings, are handed the
correction's diff over its parent too (`parent.head..head`) whenever the publication recorded an
honest `delta-range`, and the whole change when it recorded a `delta-range-reason`.

The record is local and posts nothing. Both criteria `pass` is what the correction's computed verdict
waits for — until then `review-validator` is a missing role and the verdict is `incomplete` — and it
never overrides the ledger: an open severe finding still computes `rejected`, so the verdict reads
`approved` only when the validator and the ledger agree. Either criterion `fail` is conclusive. While the lineage has a correction left the receipt stands
rejected and the next correction is offered, the verdict naming the failure; at the limit it escalates the
receipt, naming `original_criteria_failed` or, when only the second failed,
`correction_regression_failed`, and the local `escalated` receipt it writes is the one `release_ci`
refuses. A candidate the validator could not inspect is `unavailable`, carries a reason and neither
criterion, and spends nothing: it is retried, and the third such attempt escalates with cause
`validator_inconclusive`. Never record an inspection that failed as a failed check — a failed check
escalates a correction on something nobody observed. `budget_exceeded` is not a cause here, because an
over-budget correction is refused before anything is pushed and so has no receipt to escalate.

### Severity floor

Only a `severe` finding the ledger leaves open enters the fix-and-re-review loop below. `warning` and
`suggestion` findings are recorded once against the receipt, are never re-reviewed, and never block.

A lens decides its own severity and nothing here overrides it — a lens may record `severe`. Every
lens finding owes a `location` regardless — stated once already under **Lens acquisition** above
and not restated as an exception here, because `causal_disposition` never lifts it: a lens finding
declaring `unknown` still owes a `location` whose path `base..head` changed, and a write omitting
one is refused `lens-finding-location-required` exactly as any other lens finding's would be.
What `causal_disposition` decides, past that floor, is how the computed verdict settles the finding,
in the source's own order. A severe finding missing its `evidence_class` or `causal_disposition`, or
declaring `insufficient` evidence or an `unknown` cause, is **escalated**: it goes to a person, as the
source's terminal escalation sends it. A `pre-existing` or `base-only` one is a follow-up — recorded,
filed as its own issue and blocking nothing. A `deterministic` one is open and rejects the receipt, no
confirmation owed, because the lens demonstrated the behaviour directly rather than inferring
it. An `inferential` one follows its refuter: `refuted` drops it, `corroborated` and `inconclusive`
keep it open, and with no refutation result it escalates.
Separately, an `introduced`/`worsened` claim this repository cannot prove is not downgraded into
`unknown` the way the source does; it is refused outright at the write itself —
`finding-location-required` or `finding-location-outside-the-change` — so a claim that started as
`introduced`/`worsened` and could not be proven never reaches this routing at all, the same way a
claim the reviewer declares `unknown` from the start does reach it and is escalated rather than
refused.

## The fix is read again

A finding the ledger leaves open is fixed, and the fix produces an immutable delta that the lenses read again —
the same freeze, the same blindness, the same rules.

The loop is bounded, and two different kinds of bound stop it.

**A lineage gets a bounded number of corrections.** A lineage is an original candidate and its repairs on one pull
request, and The `Corrections per lineage` row sets the bound (default 3): after the original review, that many correction attempts. The count is derived, not stored. `review_status`'s `corrections`
reads it off the
`parent` receipts `published` markers already carry: a publication over a parent that stands
`rejected` or `escalated` — a computed marker, a retired `rejected` aggregate verdict, or a local
`escalated` receipt a person recorded — is a **correction**; a publication over a parent nobody has
judged is neither a correction nor a round, because nothing was corrected; a first publication, and a
publication over an `approved` parent, begin a **new lineage** and zero the count. Only the pull
request the issue's latest publication names is counted, since an issue outlives its changes and a
publication records whichever one preceded it on the issue.

The first correction past the limit is refused, `correction-already-spent`, by the publish call itself, before
anything is pushed, so no panel is ever launched over a round the lineage cannot have. A spent
correction is also what opens `escalate_review`: the change stays in `review` and a person decides,
carrying what every round measured — a routine wait on the review gate, so not the non-routine
adjudication `blocked` is for. A change that needs more corrections than the limit belongs on a new pull
request, which starts its own lineage. Reviewers are never told the count. An approved computed verdict
on the exact latest receipt is still required; spending the correction manufactures none. `Repair ceiling`, which counted rounds and
refused none, is retired, and an installed row is reported rather than followed.

**A republish over an `approved` receipt is a new candidate, not a correction.** It begins a new
lineage for counting: it spends no correction, but the correction budget and scope of its pull
request still bound it. It still needs a reason to exist, because an approval is
already the delivery the author asked for: the publish call refuses it
`approved-republish-without-cause` unless the timeline carries a finding recorded against that parent
receipt since the verdict — the author's declaration of a defect the panel did not weigh — or the base
**advanced** past the parent receipt's own recorded base, meaning that base is a proper ancestor of
the one this publication records. Direction is the point: a base that merely differs, reset backwards
or swapped for another line of history, integrates nothing the parent did not already carry, and
under `required_status_checks.strict: true` it is the advance that forces the integration, whether
the run merged it or rebased onto it. A base advance never resets the correction budget: the round
is measured against the tree git merges the parent head and the new base into, so only the author's own
lines, conflict resolution included, are charged, and the paths the merge conflicted on and the
advance's rename destinations are admitted beside the reviewed ones. Warnings and suggestions are delivered and informational; a
republish is not the route that repairs them.

The other kind of bound belongs to the binding rather than to this policy: it refuses a correction's
own publish call outright — before anything is pushed — when the correction or the lineage it belongs
to exceeds the size budget or leaves the reviewed paths. `skill/bindings/github.md` names which
refusals these are and what each one measures.

**A republish that a `warning` or a `suggestion` licensed, rather than a `severe` finding, buys the
delta lens alone.** The finding that licenses the republish is never one the panel itself
recorded — panel-recorded warnings and suggestions are delivered as informational and elect nothing on
their own. It is the author's own finding, recorded against the parent receipt since its verdict,
declaring a defect the panel did not weigh — any class recorded there satisfies the gate — and once
that declaration is not `severe`, the repair answering it is not the shape the full panel exists
for: `review_tier` with `parent_head`
takes that finding's own class as `clearing_finding`, and when it names `warning` or `suggestion`
over the repair's own honest narrow range whose level is not `high`, the answer carries `panel: "lens"`, one `review-lens`,
in place of `level`'s own allocation. That lens's report and findings are what the
computed verdict reads. A `severe` clearing finding still buys the full panel — it is what
blocks, and a fix for it is read the same way the original finding was — and so does a `high`
repair even when a `warning` or a `suggestion` cleared it, and so does a repair whose
own range is not honestly recoverable (`range: "whole-change"`, history rewritten, a base that moved
and was merged in, or an empty repair range): the delta lens substitutes for a range it can actually measure, never for one that
fell back to the whole change and never for a hot-path delta. A `severe` finding that round's lens records — `deterministic` or
`inferential` — buys the same full panel of lenses:
the repair returns to them, which is where the ledger settles it.

**A republish no finding blocked is one the author elected, and it costs a full cycle.** An author who
sees an approval and then republishes anyway to tidy what the panel merely noted spends a new epoch
and a new panel. The gate refuses the undeclared shape before it costs anything, as
`approved-republish-without-cause`: a finding recorded against the parent receipt since
the verdict, or an advanced base, is what clears it. The rule holds once the ledger computes
`approved`, not only once the verdict is recorded: an unjudged parent whose every licensed lens has
reported and which computes `approved` is refused the same way: the author records the verdict and
delivers, or records a finding since its newest lens report, and one whose licensed lens has not reported is refused
`republish-before-panel-reported`. It is no longer a counted round: nothing
charges the author's election to a ceiling, because there is none, and the new lineage it begins owns
its own corrections.

So a `warning` or a `suggestion` does not elect a round on its own. It is real and it is not free:
a repair round is a new epoch, which invalidates every verdict already recorded, and a new panel.
The ledger keeps every one of
them against the receipt they were found on, and a `record_disposition` answer says which were
delivered rather than repaired, and why.

Elect a round when a finding blocks, or when an author finds an unrecorded defect that would ship
something a reader would act on and be wrong — never for a finding a reviewer already recorded and
declined to block on. Not because the panel produced text.

**With no severe finding, the publication is delivered rather than repaired.** A `warning` or a
`suggestion` is informational: it never opens a correction, never blocks delivery, and requires no
action of any kind — nothing owes an issue or a disposition for being recorded, and it is never a
reason to re-run review on this candidate. A run may still record a `record_disposition` answer for one it acted on or filed, and the record is optional
and may answer a subset. Only a `severe` finding whose `causal_disposition` reads `pre-existing`,
`base-only` or `unknown` owes its own tracker issue by right of being recorded, whether or not this
same publication also carries a severe finding that blocks it: no gate ever blocks on a `pre-existing`
or `base-only` one and no disposition reaches any of them. `unknown` is filed too, though it
escalates the receipt to a person rather than blocking it. A `filed` issue records the finding's own `epoch` and `id`
in the store through `create`'s `--epoch` and `--findings` arguments, not in a sentence a later edit could drift out
of step with what it names. `causal_disposition` still sits on `severe` alone, for the reason given
above — a disposition asks a warning or a suggestion for neither word. Estigia cannot see whether
any of this happened: a caller supplies the provenance and the answer, and both are asked for here,
never adjudicated.

A republish over an approved receipt exists for a defect the author judges blocking that no reviewer recorded — not
for tidying what a reviewer explicitly declined to block on. A non-blocking finding is recorded once
against its receipt and is not re-reviewed.

### What a repair carries

Republishing mints a new epoch and invalidates every prior verdict, which is what stops reviewed bytes
being delivered as unreviewed ones. What it must not also do is throw away what the last round
settled.

A publication over an earlier one records, **derived from the timeline and never supplied by the run
being reviewed**, the whole parent receipt — epoch, PR, head, base and target digest — and a delta
digest covering both ends. A run that could name its own parent could name the epoch whose findings
were mildest. `parent-head..head` is the delta when it is honest — `parent-head` still an ancestor of
`head`, the base not having moved, and that range touching at least one path. A moved base is read
whole whichever way it was integrated: a merge keeps the parent head an ancestor but the range then
carries the base's advance, which is the base's and not the repair's. Estigia records the two ends and does not compute the diff, scope the review to it, or check
that a lens read only it — and now also records on the marker, answering back through
`review_read` with `view: transaction` rather than through the publish answer, whether that pair was honest at all:
`delta-range` carries `parent-head..head` verbatim when it is, `delta-range-reason` names
`"history-rewritten"`, `"base-advanced"`, `"merged-range"` or `"empty-repair-range"` when it is not.

The receipt rather than the epoch, and that is a repair rather than a flourish. An epoch is not a
function of the bytes it names, and a finding's epoch field is whatever the finding says it is — so a
parent ledger matched on the epoch alone could be written into after the fact, by recording a finding
that named the parent epoch and carried the repair's own bytes. Matched on the receipt, it cannot.

A lineage reaches **one publication back**, and that bound is worth knowing before a later round
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

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the draft barrier, one computed exact-receipt verdict, the exact
review transaction, Claude's reserved-role prelaunch checks, the **publication lane refusal** —
`record_computed_verdict` will not bank an approval for a head whose dispatched CI lane is red or
still running — the **retirement of the aggregate verdict**, which is why no tool records an aggregate verdict under a reviewer's name, the **receipt currency**, which refuses a finding naming any receipt but
the one under review, the **declaration gate**, which refuses a severe finding that states neither its
evidence class nor its causal disposition and any finding whose declared word is outside those two
vocabularies, the **lineage references**, which refuse a parent identity the parent
receipt never recorded and a new severe finding against a repair that states no origin, and the
**refutation batch rules**, which refuse a refutation batch that omits a standing
`inferential` severe finding, names one twice, names a `deterministic` one, or is recorded by the
publisher or a requester of this same review. Nothing proves panel execution, and nothing proves a
refuter read what it was handed rather than guessing.

That is worth stating first and plainly. Estigia gates repository writes against an adjudicated
claim. It cannot prove panel size, concurrency, independence, blindness, same-finding identity or
quorum. It cannot see whether a context read the target, whether a verdict was honest, or whether
every Judgment Day judge was given a separate directory — for their checkouts or for anything else
they wrote.
There is no hook that fires when a reviewer forms an opinion, and none that notices two of them
sharing a scratch path.

The `Evidence standard` row does not change that. It decides the grant Estigia writes into the
reserved definition and enforces from its own embedded copy, which is a real narrowing of what a
judge **can** do, and the definition asks for what the closing
statement rests on — which is a request for a declaration, not a check on one. Neither proves what a judge
**did**, and an unreadable contract answers
the narrower value rather than the wider one because a fault must not hand out a capability.

The findings are the same kind of evidence and carry the same limit. Estigia adjudicates that a class
is one of three words, that evidence and impact were stated, that a receipt is the current one and that
a named parent exists. It cannot check that a `severe` finding is severe, that a `suggestion` is not a
defect in disguise, that the evidence reproduces, that the impact is real, or that a repair `introduced`
what its reviewer says it introduced. A structured claim is still a claim; what changed is that it is
now legible enough to be argued with.

`publish_review` keeps ordinary compatible CI behind a draft PR, records epoch/PR/head/base/digest, and
starts one publication-lane run against the head it pushed. That last one is why the refusal above is
mechanical rather than a rule somebody has to remember: the signal is collected at publication and the
gate sits on the single writer of a verdict marker, `record_computed_verdict`, not in the prompt that launches the lenses. It is
not a proof that a lane exists — a repository without a dispatchable one has no check runs on its
head, and that proceeds.
`release_ci` replays the newest receipt on its own pull request, current draft PR and coherent clean target before
marking ready. A republish invalidates earlier evidence even for identical bytes. These checks bind
the cooperative order to exact bytes; they do not prove who reviewed them. GitHub has no atomic
conditional-ready operation, and out-of-band collaborators or workflows can bypass the order or forge
evidence. There is no malicious-collaborator authenticity claim.

For Claude Code, setup installs two stable inert reviewer definitions. Judgment Day's two
judges are `review-blind` and `review-blind-sealed`, one template rendered twice, and no tier acquires
them. The seven other reserved names — the five sealed lenses `review-lens`,
`review-risk`, `review-readability`, `review-reliability` and `review-readback`, the sealed refuter
`review-refuter` and the sealed read-only validator `review-validator` — are installed by nothing:
`launch_review_input` launches each from a template embedded in the binary, as **Lens acquisition**,
**Refutation** and **The targeted validator** state, and setup retracts a copy an earlier build wrote.
Judgment Day's two judge definitions are fixed, not configured: one sealed, one unsealed, launched
only for Judgment Day.
`Evidence standard` decides the unsealed grant; the sealed one is fixed. The two judge definitions carry the paragraph asking for
what a closing statement rests on, applied when `setup` or `sync` next writes each file. For Judgment Day the orchestrator remains
responsible for passing the effective judge model, launching both judges concurrently, withholding
sibling output and preserving dissent. For ordinary review it remains responsible for calling `launch_review_input` for every
lens slot `review_status` offers. That is a real contract, not mechanical proof that it happened.

**What is still enforced around this:** the claim, the renewal before repository writes and at every
irreversible boundary, and a push no live claim authorises refused under git. Those hold whatever a
review decides, because they come from the claim rather than from the review.
