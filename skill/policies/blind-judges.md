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
same `review-blind` definition for every judge; there are no numbered definitions.

**Every judge runs as the reserved `review-blind` role**, and what that role may do is the
`Evidence standard` row's answer rather than a constant.

Under `reading`, the role is read-only against the target: its gate refuses every write, edit, shell
and delegation tool, so such a judge cannot mutate the target even if its prompt asks it to.

Under `measuring`, the role gets a shell **and nothing else** — write, edit and delegation stay
refused, so a judge cannot hand the work on, and cannot reach the target through a tool call. It can
of course rewrite a file *through the shell*: what stops that being anybody else's problem is the
directory it is confined to, not the grant. The isolation rule below is doing that work, and it is the
only thing doing it. A repository whose findings are established by reading wants the first answer; one
whose evidence standard is mutation needs the second, because a judge that cannot run the suite
cannot check whether turning the fix off leaves it green.

Either way, a judge that edits anything but its own handed directory is no longer judging.

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

### One directory per judge, and every directory it writes

A verdict is sometimes only worth what it measured. In a repository whose evidence standard is
mutation — where confirming a finding means building the change, reddening a test, or turning the fix
off to see the suite go red — a judge that cannot run anything can only agree with the prose it was
handed. That is what `Evidence standard: measuring` is for, and this section is the rule that makes
it safe. It applies to every judge that may write at all, whatever launched it: a panel that this
harness cannot place in the reserved role is governed by this section too, not exempted by it.

**No two judges are ever pointed at one directory.** Each judge is given a directory nothing else
writes for the duration of its review — not a sibling judge, not the orchestrator, not another run.
Two measuring judges in one directory are concurrent writers: each reads the other's edits, rebuilt
artefacts and test output as though they were the target's, so neither verdict is a reading of the
bytes that were published and neither can be reproduced. A panel that cannot be given one such
directory per judge runs fewer judges; it does not share one.

**That covers every location a judge writes, not only its checkout.** Scripts, intermediate output,
notes and saved measurements go in a place of that judge's own. Measured live on a five-judge panel
that shared one scratch directory: one judge's script was overwritten by another's and then executed
inside a third judge's checkout, and a fourth read the implementing run's planning notes through a
file it had not written. Nothing in the delivery target was damaged and every judge re-verified its
own tree, but two verdicts were no longer independent readings and one had to be discounted. A rule
that binds the checkout and says nothing about the scratch is a rule with a hole the exact size of
the panel.

A judge that finds the target already dirty **stops and reports it rather than restoring it**. Whatever
made it dirty was somebody's measurement in progress, and restoring is a write like any other: it
destroys that measurement, races whoever else is repairing the same directory, and leaves the head that
gets delivered as whatever survived. Reporting a dirty target costs one verdict. Quietly repairing it
costs the evidence that anything was wrong.

Where those directories come from is not decided here. [Repository delivery](../references/repository-delivery.md)
already owns run isolation — checkout paths, what makes a checkout this run's own, and per-checkout
caches — and this rule adds a reason to obey that document rather than a second copy of it.

Isolation buys reproducibility, not blindness. A judge's reading tools are unrestricted over whatever
the directory it is handed contains, under either evidence standard, so blindness under
any shared target rests on this prose and on what the orchestrator gives each judge, and nothing
measures it. Nor does this rule reach a published head a judge mutated: the review target is
re-derived from a coherent clean snapshot and a dirty path is refused before publication or release,
so that outcome is a blocked release rather than a bad delivery — a refusal that already existed and
is not produced by anything written here.

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
publication receipt, Claude's reserved-role prelaunch checks, and the **publication lane refusal** —
`record_review_verdict` will not bank an accepted verdict for a head whose dispatched CI lane is red or
still running. Nothing proves panel execution.

That is worth stating first and plainly. Estigia gates repository writes against an adjudicated
claim. It cannot prove panel size, concurrency, independence, blindness, same-finding identity or
quorum. It cannot see whether a context read the target, whether a verdict was honest, or whether two
judges were given separate directories — for their checkouts or for anything else they wrote.
There is no hook that fires when a reviewer forms an opinion, and none that notices two of them
sharing a scratch path.

The `Evidence standard` row does not change that. It decides the grant Estigia writes into the
reserved definition and enforces from its own embedded copy, which is a real narrowing of what a
judge **can** do; it proves nothing about what a judge **did**, and an unreadable contract answers
the narrower value rather than the wider one because a fault must not hand out a capability.

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
