# Judgment Day

Judgment Day is a two-judge adversarial method. Estigia installs its two judges and states the rules below; it allocates nothing for it.

1. **Explicit request only.** Convene it when the operator asks for Judgment Day. A tier, a risky
   change or an ordinary review that found nothing is never a reason to.
2. **It replaces ordinary review for that target; never both.** No lens, refuter, correction or
   validator runs over a target Judgment Day judges, and no judge runs beside a lens review. That
   forbids running both as the adversarial method; it does not forbid ordinary negotiated review the
   operator explicitly asks for on the same target. Run that as its own step, after or before: it is
   the review release needs, and neither outcome authorizes delivery, which stays under ordinary
   repository policy.
3. **It never launches `review-refuter`.** Two judges agreeing is its corroboration, and a finding
   only one judge reports stays that judge's claim.
4. **It issues no receipt and carries no delivery authority.** It satisfies no commit, push, PR or
   release gate, and `release_ci` neither reads nor needs it. Its findings are reported to the
   operator and, where worth separate work, filed as issues through the analyst route. They are never
   recorded with `record_review_finding`, never enter a receipt's ledger and never reach a computed
   verdict. Estigia does not enforce this: nothing refuses a judge's finding recorded against a
   receipt.
5. **Fix rule.** Every judge returns before anything is merged; a single judge's result is never a
   judgment. The orchestrator merges the results into a frozen ledger, held in its own report
   because Estigia stores none. Only a severe finding that every judge reports independently is fixed.
   A finding one judge alone reports is a suspect and is never fixed automatically. Only a defect the
   target introduced or worsened is the target's to fix: a confirmed `pre-existing` or `base-only`
   severe finding is reported and filed, never fixed here, and one whose cause is `unknown` is reported
   as unknown for the operator to settle and is not fixed. Warning and suggestion rows are
   info: they are reported, never fixed on this path and never re-judged. The orchestrator passes the
   same resolved project skill paths to every judge and to the fix actor.
6. **Ask before fixing.** The orchestrator asks the operator before the round-one correction and
   fixes nothing until the operator answers. Estigia installs no counterpart to v4's fix agent; the role is played by the
   `developer` definition when the orchestrator delegates the correction, and by the orchestrator's
   own writer when it does not. Either one is scoped to the confirmed severe IDs and touches nothing
   else: it never reviews, never adds a finding, never refactors unrelated code and never launches
   another actor. Judgment Day grants no claim, so the correction is a repository write like any
   other and needs the run's own live claim. The actor reports each work unit with its focused test
   result and rollback boundary, and returns control to the orchestrator, which alone launches the
   re-judgment. If ordinary review already approved the head, the fix cannot be republished on that
   pull request: `approved-republish-without-cause` refuses it and rule 4 forbids recording a cause.
   The fix lands as a new pull request, or before ordinary review approves.
7. **Bounded rounds.** At most two fix rounds and two scoped re-judgments. A re-judgment is the
   judges again over a frozen ledger plus the fix delta and nothing else, and it may report a defect
   the fix caused, with its proof. A re-judgment that fails before round two lets the orchestrator
   launch the second and final fix round. An exhausted budget is never reset or extended: there is
   no third round.
8. **Terminal verdicts.** The only ones are `APPROVED` and `ESCALATED`. `APPROVED` means no confirmed
   severe finding is open. `ESCALATED` hands the target to a person. A suspect is reported beside
   either verdict and decides neither. Before it returns either, the orchestrator runs an independent
   final verification of the target as it stands, in a context that wrote none of it; an escalation
   under rule 9 is returned after that verification.
9. **Escalation and scope.** Judges that contradict each other escalate to a person; the orchestrator
   never breaks the tie. A target that is unclear gets one scope question, and then the orchestrator
   stops and launches nothing. Anything still open after the second round escalates: the orchestrator runs rule 8's final verification, returns `ESCALATED` and stops.
10. **Output contract.** Report the target identity, the round, the confirmed, suspect,
    contradiction and info counts, the correction work units, the re-judgment result (approved,
    escalated or not run), the skill resolution and exactly one final line, `JUDGMENT: APPROVED` or `JUDGMENT: ESCALATED`. v4's artifact references are dropped, because
    Estigia keeps no Judgment Day store.

What follows is only as much of the earlier two-judge panel as Judgment Day still needs: the
composition, the roles, the isolation rule and the measuring exception.

### The mechanism

**One immutable target, two judges, in parallel, blind.**

Fix the complete clean target first — a head, a diff or a commit range. Launch both judge
contexts concurrently over the identical immutable target and criteria. Each judge receives the same
target, scope and criteria — including the named exceptions
below — and is blind to the other's findings and reasoning.
A later opinion that has read an earlier one is not independent. Claude's orchestrator launches two
reserved roles from one template with a different `Seal`: one sealed, one unsealed. A sealed judge
holds one tool, a web
search; what that seal does and does not buy is stated below. An unsealed judge runs as
`review-blind`, whose grant is the `Evidence
standard` row's answer rather than a constant.

Under `reading`, review-blind is read-only against the target: its gate refuses every write, edit, shell
and delegation tool, so such a judge cannot mutate the target even if its prompt asks it to.

Under `measuring`, the role gets a shell **and nothing else** — `Write`, `Edit`, `Agent` and `Task`
stay refused. Through the shell it can of course write and launch anyway: what stops that being
anybody else's problem is the directory it is confined to, not the grant. The isolation rule below is
doing that work, and it is the only thing doing it.

### Reserved reviewer role

Every reserved role is operator-owned. For Claude's current `Agent` and legacy `Task` launch surfaces, any
project definition under recursive `.claude/agents`, from the launch cwd through the first `.git`
repository root, with that filename or YAML-parsed frontmatter name invalidates the panel. Unreadable or
duplicate candidates also invalidate it, and the canonical user file must be the only user-scoped
definition with that identity and match one of the renderings Estigia can produce of the embedded
reviewer. Setup performs the same recursive
user-tree uniqueness preflight before writing. A running reviewer is gated by the embedded policy, not
project-local bytes, so a tool that gate refuses under a reserved role cannot be added to a project
definition: the grant is Estigia's, the `Evidence standard` row is the one in-place change that widens
it — `measuring` gives the unsealed reviewer a shell, and no row grants any other tool — and past that
the way forward is a different sub-agent for the step or a separate session or durable handoff. A
refused or unprovable launch contributes no judge. If the collision or canonical
file cannot be restored, use a separate session or durable handoff; never silently reduce or serialize
the panel. Ordinary agent names remain project-first, and this prelaunch check is Claude-only.

### One directory per judge, and every directory it writes

A judge's findings are sometimes only worth what they measured. In a repository whose evidence standard is
mutation — where confirming a finding means building the change, reddening a test, or turning the fix
off to see the suite go red — a judge that cannot run anything can only agree with the prose it was
handed. That is what `Evidence standard: measuring` is for, and this section is the rule that makes
it safe. It applies to every judge that may write at all, whatever launched it: a panel that this
harness cannot place in the reserved role is governed by this section too, not exempted by it.

**No two judges are ever pointed at one directory.** Each judge is given a directory nothing else
writes for the duration of its review — not a sibling judge, not the orchestrator, not another run.
Two measuring judges in one directory are concurrent writers: each reads the other's edits, rebuilt
artefacts and test output as though they were the target's, so neither judge's findings are a reading of the
bytes that were published and neither can be reproduced. A panel that cannot be given one such
directory per judge does not share one; use a separate session or durable handoff rather than
reducing it.

**And none of those directories is inside the checkout the claim covers.** A delegated context's tool
events carry the launching run's session, so a judge given a directory inside that checkout is
measured against that run's live claim and allowed to write there — the gate answers *this run may
write here*, and it is right to. What keeps a measuring judge's mutations out of the delivery target
is this rule putting its directory somewhere else. Nothing downstream re-checks it, which is why it
is stated here as a precondition of the launch rather than left as a consequence of the grant.

**That covers every location a judge writes, not only its checkout.** Scripts, intermediate output,
notes and saved measurements go in a place of that judge's own.

A judge that finds the target already dirty **stops and reports it rather than restoring it**. Whatever
made it dirty was somebody's measurement in progress, and restoring is a write like any other: it
destroys that measurement, races whoever else is repairing the same directory, and leaves the head that
gets delivered as whatever survived. Reporting a dirty target costs one verdict. Quietly repairing it
costs the evidence that anything was wrong.

Those directories come from `judge_workspaces`, which takes the run id, a publication's receipt identity
(it scopes the directories to a head and gives Judgment Day no authority) and
a judge count, and answers one **bare working directory** per judge — outside the checkout the
claim covers, distinct from each other and never nested in one another, each created and read back
before it is answered — refusing rather than returning a short list when it cannot make one for
every judge. A target nobody published has no receipt, so `judge_workspaces` cannot serve it: for
Judgment Day over it the orchestrator creates each judge's directory by hand, outside the checkout and
under this same rule. [Repository delivery](../references/repository-delivery.md) still owns run isolation
for the claimed checkout itself: checkout paths, what makes a checkout this run's own, and
per-checkout caches.

Isolation buys reproducibility, not blindness. A judge's reading tools are unrestricted over whatever
the directory it is handed contains, under either evidence standard, so blindness under
any shared target rests on this prose and on what the orchestrator gives each judge, and nothing
measures it. Nor does this rule reach a published head a judge mutated: the review target is
re-derived from a coherent clean snapshot and a dirty path is refused before publication or release,
so that outcome is a blocked release rather than a bad delivery — a refusal that already existed and
is not produced by anything written here.

The thing under review is the payload `publish_review` materialised for that epoch — per-path unified
hunks against the receipt's recorded base, bounded, refused rather than truncated on overflow. The
orchestrator asks for it with `inline_payload` and hands it to every judge; the default answer omits
the text, so a run that means to hand it on has to say so. Isolation still requires one directory per
judge, none inside the claimed checkout, covering every location a judge writes. Estigia cannot prove
any of that happened.

**The payload is the primary evidence and the checkout is the named exception.** **Reaching a defect that lives outside the payload is the panel's obligation, not each judge's.**
The unsealed judge is the one that can leave the payload to open a file; the sealed one cannot, and
this policy does not ask it to. A sealed reviewer holds one tool, a web search; it has no filesystem
read and no shell; and its definition tells it not to look the change up, which is asked for and not
enforced. On a published repository that instruction is the only thing between the seal and a copy
of the tree. What the arrangement buys is a verdict whose evidence is the payload rather than a
second reading of the tree it was cut from. It does not buy checkability: the receipt's digest binds
the file `publish_review` materialised, and a sealed judge holds the copy the orchestrator
re-emitted into its prompt, which nothing compares.

So the brief hands the payload first, and then **names the specific claims this change makes about
text it does not show** — a count, a registered size, a document asserted to describe the code — and
says the checkout is for those — to the unsealed judge, the only one that can open it. A brief that
tells a sealed judge the checkout is for the unshown claims invites it to report an access failure in
place of a verdict, which its own definition requires of it; hand it the claims and say plainly that
every figure it cannot see is a claim. **The brief asks the unsealed judge — the one that can act on it — to
say which file it opened and what question sent it there** whenever it leaves the payload, because
that is what makes the exception auditable rather than a licence — and the judge reads the brief,
never this document, so a duty written only here is one nothing carries.

`Review budget` names the figure a single context's tokens are reported against, a lens's or a judge's — `150000` by default,
`none` to declare nothing. It is advisory: nothing here refuses a panel that runs past it, and a
figure recorded with `record_spend` is recorded, not verified. Judges are never told the budget.

### What a judge's closing statement must report

A Judgment Day judge reports findings and no verdict, but the definitions still ask for a closing
statement to the orchestrator, and the standard it is held to has not moved. `Evidence standard` decides only whether
the reserved role's grant includes a shell — never what a judge holding that shell is asked to say
about using it.

A verdict states what it rests on: what was executed and its outcome, the mutation run behind each
severe finding and whether it failed for the reason the defect exists, and what was not checked.
Measuring nothing is a legitimate basis when it says so plainly rather than leaving the gap implied.

A `measuring` judge is the one with commands and outcomes to name. A `reading` judge ran nothing, and
the paragraph above still asks it for what it rests on: what it read stands in for what was executed,
alongside what it did not check and the plain statement that it executed nothing — the same standard
held for a `measuring` judge, not a lesser one; leaving any of that to be assumed is the gap that
paragraph refuses, whichever standard produced the statement. The statement is a declaration, on the
same footing as `evidence` and `impact` on a finding, and nothing records it: Estigia never re-runs a
command it names.

