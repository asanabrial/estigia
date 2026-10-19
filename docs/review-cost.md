# What a review costs, and what actually spends it

The question was whether Estigia's blind-review panel is expensive because its judges are slow
readers, or for some other reason. This measures it: two panels, four judges, one review target,
launched concurrently on 2026-08-30 by `claude-ac6d2184b9c7fca3`, design fixed before either panel
ran. Two measurements are recorded here: this one, which compares two briefs inside this
repository, and a second below, which compares this repository's panel against the other
project's own binary.

Ordinary review no longer acquires judges: it licenses lenses only, and Judgment Day is
the opt-in. The judge and panel figures below are measured history of the shape that preceded it,
and they stay as measured; what a lens costs is measured in the sections on lenses. A review's
forecast counts lenses alone.

It is one document in [the reference set](README.md); the rows this measures are in
[`configuration.md`](configuration.md), and [`honesty.md`](honesty.md) says what the recording cannot
count.

**What was under review**, identical for every judge. This is not a review transaction in this
repository's own sense — `skill/references/repository-delivery.md`'s *Publish, review, and verify*
section defines that as epoch,
PR, head, base, target digest and payload digest, and no epoch was reachable here: the PR was
already merged before this measurement ran.

| | |
|---|---|
| Pull request | "Let the gate see the worktree a call names" |
| Base | `883864e8261fa93eadd94390e4f38a768fd3ee9a` |
| Head | `39ffe43ca68d6ab3e27f6137f0069e5b73e69355` |
| Size | 5 files, 796 insertions, 54 deletions |

Chosen because it is the change the question already argues from: four publications and eight blind
judges on three rejections with no code defect ever found, and a diff that mixes code and prose,
which is the axis the grading question turns on.

## The result that inverts the premise

| | tokens | tool uses | panel latency (concurrent) | summed judge time | findings (summed) | severe (identities) |
|---|---|---|---|---|---|---|
| Arm E — `review-blind`, the definition `estigia setup` installs | 291,408 | 69 | 558.1 s | 1078.7 s | 17 | 1 |
| Arm C — the comparison rubric, pasted into an `analyst` carrier | 268,162 | 72 | 491.0 s | 912.8 s | 4 | 0 |
| **E / C** | **+8.7%** | −4% | **+13.7%** | +18.2% | 4.25x | 1 vs 0 |

Per judge: E1 133,651 tok / 520.6 s; E2 157,757 tok / 558.1 s; C1 127,021 tok / 491.0 s; C2
141,141 tok / 421.8 s. These are what the agent harness's own completion report returned for each
launched judge at the moment it finished — token count, tool-use count and duration in
milliseconds — not a reconstruction from a retained transcript; no artifact of the measurement is
kept in this repository (see `docs/honesty.md`).

**The two panels came within nine percent on tokens and fourteen on latency, against a 4.25x
difference in what they reported.** What that does not show is a judge reading more slowly under
one brief than the other.

A severe finding forces a republish, and a republish invalidates every verdict and restarts the
panel — a verdict could be `rejected` only over a `severe` finding (the verdict has since become
computed from the ledger, so the rule is the ledger's now), and the next round
pays the full panel again. Arm E produced one severe finding: E1 and E2 independently recorded a
severe finding with the same identity, E2's version carrying an extra limb E1 did not raise, and
under the `Blind judges: two blind` row this repository carried at the time, both judges confirming a
severe finding with the same identity is what blocks and buys a repair round. Arm C produced none and would have approved on the same
bytes. One round of Arm E costs 291,408 tokens; a rejection forces the next round to pay that
again, and this change paid a full panel four times over. The multiplier is the round count, and the
round count is set by grading, not by reading speed.

## What each panel found, and how each panel graded it

Both arms read the same bytes. Separating what a judge noticed from what class it gave the fact is
the whole of what follows.

### The finding that decides it

The change's third acceptance criterion: *"A call naming a directory covered by another run's
pointer is refused."* The test that asserts it passes a `--run-id`; at the time, the only shipped
caller passed none, so the criterion was satisfied only on a road no shipped adapter took. A later change gave the shipped adapter a derived identity from the host's session id, so it takes that
road when the host supplies one.

| Judge | Arm | What it did with that fact |
|---|---|---|
| E1 | E | **severe** — would block delivery |
| E2 | E | **severe** — would block delivery |
| C1 | C | did not raise it |
| C2 | C | raised it in its `evidence` array, explicitly declined to file it as a finding: disclosed in `docs/honesty.md` and the PR body, consistent with the operator's recorded invariant, therefore "an accepted trade rather than an undisclosed defect" |

Three of four judges reached the same reading of the same bytes. Under Estigia's rubric, both
judges confirming a severe finding with the same identity blocks, and the change is rejected.
Under the comparison rubric it is not a finding at all and the change is approved. Nothing about
the bytes explains the difference — the brief does. That claim has a bound: these are the same
bytes this repository's own two-blind panel accepted for real, merging the PR as `fb0fd04`, so
judge-to-judge variance inside one brief is at least as consistent with the split above as a
difference between briefs is — bounded in turn by the judge definition having changed between
that panel and this one, so the two readings are not of quite the same brief either.

### A second split, finer-grained

The PR body describes `held_by_a_pointer` as reading `repo_dir` and `worktree`; the shipped
function reads `worktree` only, and one of the change's own tests exists to refuse the `repo_dir`
reading. Three judges raised this and split into two classes: C1 **WARNING**; C2 **SUGGESTION**
and E1 **suggestion** agree. Same defect, two classes, inside one arm — worth keeping on its own,
because it means the comparison rubric does not eliminate grading variance either; it only changes
where the variance lands.

### Where the panels agreed

The stale "before this fix" doc comments on added tests — describing a base state that never
existed, since the behaviour they name belongs to an intermediate draft of the same PR — were
raised by E1 (**warning**) and C1 (**WARNING**). Identical grade under both briefs.

This matters for cause 1 below: Estigia's judges did not over-grade prose. Every prose finding in
Arm E came back `warning` or `suggestion` — the class `skill/policies/blind-judges.md`'s *What a finding is*
prescribes — from judges that never received that document (it is routed to the orchestrator, not
the judge; see "Where the rule lives" below). The over-grading was on the one finding that was
about *behaviour*, not on any of the prose.

The severity-choosing rule was withheld from the judge prompts by design, fixed before either
panel launched, not a gap noticed afterward.

## Why Arm E graded the behavioural finding severe, in the judge's own words

E1, justifying its class:

> "I raise it `severe` rather than `warning` only because the criterion is explicit, the surface is
> a delivery boundary, and this is the class that reaches the operator for a ruling."

It added: "A recommendation exists and does not require a code round." E2 landed in the same place
independently: "Whether criterion 3 is waived is an operator call, not mine — I am flagging it,
not deciding it."

Neither judge was misapplying a rubric it had. The definition as it then read gave it three words
and a consequence, and no criterion for choosing between them. Each judge used `severe` as
an escalation channel, because `severe` is the only class that blocks. That is a sharper
statement of the defect than its report carried: the missing choosing rule does not merely let a prose
finding block. It collapses two distinct needs — *this is a defect* and *a human must decide this*
— into one word, and the word costs a full repair round. Both judges wanted to escalate without
blocking, and the vocabulary had no way to say that.

## Where the rule lives, and where it is not routed

> [!NOTE]
> **This section measures the tree as it stood at the time of the measurement, before the choosing rule was added.**
> Every `skill/agents/review-blind.md` and `skill/policies/blind-judges.md` coordinate below is
> that tree's: the definition now carries the choosing rule,
> and the policy keeps it because the definition is installed on Claude Code alone. The finding
> this section reports is what produced the choosing rule; it is not a claim about the tree you are reading.

The three class names are stated in more than one place — `skill/bindings/github.md`'s own
`review_finding` row is among them; the linear and trello bindings carry no such definition.
Where they live is not the point; that no choosing criterion reaches the judge from any of them is.
The policy then gave a criterion for choosing between `warning` and
`suggestion` scoped to one kind of finding — a mismatch between prose and behaviour, not the
behavioural defect this measurement's severe finding turned on. Neither it nor the cosmetic criterion beside it reached the judge. That file is routed to the orchestrator —
`skill/SKILL.md` lists it in the References table the orchestrating run reads on activation —
and the judge never activates the skill: its own definition states that its starting context is
only this definition and the launch prompt — a sentence the choosing-rule change left standing — which does not
include the skill's reference table. The judge's own definition gave no criterion for choosing
between the three classes for any kind of finding.

The comparison project, as measured at `a537c63e` (at `v4.0.0` `internal/components/sdd/` is gone and
`jd-judge-a.md`'s severity floor ships as written, at line 45, with no render step to delete it),
states the equivalent rule inside its rendered judge prompt, which is the difference that matters: two spans of `internal/components/sdd/boundedreview.go` tell a
judge, in its own context, what does not rise to a finding at all and what cannot block. It also
carries the same defect one layer along — its own strongest statement of the rule
(`internal/assets/claude/agents/jd-judge-a.md:45`) is deleted at render
(`internal/components/sdd/boundedreview.go:182-198`), so it too ships a severity rule no judge
receives; it merely keeps a weaker one that survives. Neither project routed its best severity
rule to its judges. The comparison routes a lesser one; Estigia routed none — a sharper result
than "the other panel is faster", and a structural one, not an impression. That is what the choosing-rule change
changed, and only for the one adapter that installs the definition.

A different asymmetry turns out, on inspection, to be symmetric: the comparison project presents
four severity classes to every judge, but its own gate
(`internal/reviewtransaction/transaction.go:1621-1632`) reads only two — nothing anywhere
distinguishes its other two from each other. Estigia's gate does the same to its own three:
nothing distinguishes `warning` from `suggestion`, which carry identical cost in the policy's own
table — "Recorded beside the verdict; never withdraws an acceptance" against "The same"
(`skill/policies/blind-judges.md`, *What a finding is*). Both projects hand a judge more classes than their
gate distinguishes; that is a fact true of both, not an asymmetry between them.

A further asymmetry bears on how the two verdicts should be weighed against each other at all: the
comparison project's own panel "carries no delivery authority" (its `judgment-day/SKILL.md:24`),
where Estigia's verdict releases CI and gates delivery. The two panels are not staking the same
thing on a `severe` call, so this measurement compares brief-plus-carrier cost and grading, not
authority-for-authority.

## Which of the issue's four causes this measurement can and cannot distinguish

The issue named four candidate explanations. This measurement settles two, refutes one, and cannot
reach the fourth.

1. **No shared severity rubric — confirmed, and it is the dominant cause.** Established
   structurally (no criterion for choosing between the three classes reaches the judge for any
   kind of finding, general or scoped — see above) and now behaviourally: the same fact is
   severe under one brief and not a finding at all under the other. It has
   a rival worth naming: this repository's own two-blind panel accepted the PR's exact bytes
   for real, merging them as `fb0fd04`, so judge-to-judge variance within one brief is at least
   as consistent with that as a brief-to-brief difference is. That rival is itself bounded — the
   judge definition changed between the change's own panels and this measurement, so the panel that
   merged the PR and Arm E did not read under quite the same brief either.
2. **`reading` as the evidence standard — confirmed as stated, and it is symmetric.** All four
   judges ran read-only and all four disclosed it. For Arm E that disclosure was compelled
   rather than volunteered: `src/setup/mod.rs` appends `VERDICT_BASIS_REQUIREMENT`
   (`src/skill.rs`) to every rendered judge regardless of evidence standard, and
   `skill/policies/judgment-day.md`'s *What a judge's closing statement must report* asks a `reading` judge by name for "the plain
   statement that it executed nothing." Arm C's brief asked for what was inspected and not for
   what was executed, so those two disclosed without being asked. The standard cannot explain a
   *difference* between the arms, because both arms had it.
3. **The author — not measured, and out of this design's reach.** Arm E's rejection would have
   bought a repair round; this measurement stopped before the repair, so whether a fix in one file
   is left unrepaired in the next is neither reproduced nor refuted here.
4. **This repository's documentation rule making prose a defect — refuted as an explanation of the
   cost.** It predicts Estigia's judges rejecting on prose. They did not: every prose finding in
   Arm E was non-blocking. The severe finding was about behaviour at a delivery boundary. The
   rulebook asymmetry named in `AGENTS.md`'s documentation-currency rule (`AGENTS.md`) is real,
   but it is not what makes a review expensive here — the missing choosing rule is.

## What the first measurement cannot settle

`docs/honesty.md`'s review-cost entry carries the limits on this measurement.

## Conclusion

Estigia's judges weighted this review target correctly on the axis this repository is strict
about: every prose finding came back `warning` or `suggestion` — and after a later change, incorrect or
stale comments and documentation are **never `severe`**, so those grades match the rulebook even
from judges that never received it. What would force a repair round here was one behavioural
finding pushed to `severe` because `severe` is the only class that blocks. That is not a rubric to
loosen. The fix this measurement points at is the one already filed: a criterion for
choosing between `severe` and `warning`, and a way to escalate a judgment call to the operator
without spending a repair round to do it.

## A second measurement: the other system, not its rubric

The measurement above ran two arms, and **both were Estigia agents** — Arm E the installed
`review-blind` definition, Arm C that comparison rubric pasted into an `analyst` carrier. It compared
briefs. This one compares systems: C's own binary, built from source and run against a review
transaction this repository produced.

It also names an epoch, which the first measurement records it could not. It does not reproduce both
digests that definition asks for: the target digest is below, the payload digest is not.

| | |
|---|---|
| Epoch | `2cbde7ffe09ff262d07cd885087520f7` |
| PR | 167 |
| Head | `61e50ae3d490eac097b56d850c61a86e876841a0` |
| Base | `298936559023856ef100f9b229aab72001b1ec1d` |
| Target digest | `c6a9823d96e9df75875fc124f1e7744946c6a8ff479e3e231fe32053e898edec` |
| Size | 12 files, 2,199 changed lines by C's own count |
| Measured against | that project at `a537c63e`, re-confirmed at `4db804e5` |

Run on 2026-08-30 by `claude-9ac42328ad6a45a2`, against two independent throwaway clones so the
classification was reproduced rather than observed once.

**Which version.** The first pass ran against `a537c63e`, 3,003 commits behind that project's `main`,
in a checkout whose directory name said "latest". Every figure here was re-taken against `4db804e5`,
its `main` the same day, and the classifier answered identically. Four things were compared across
that range and all four unchanged: `internal/reviewtransaction/risk.go` and
`internal/reviewerprovider/contract.go`, byte-identical; and the constants
`MaxCompactCorrectionAttempts` and `MaxFrozenCandidateDiffBytes`. The first pass recorded no commit at
all, and that is why this one does.

### The tier this candidate got is a false positive

C answered `high` and selected four lenses. The signal it fired on does not describe the file it names.

The stated reason was *"this change gets a deeper review because it touches code that starts other
processes in `tests/honesty.rs`"*. The recorded `risk_reasons` are
`{"code":"process_boundary","signal":"shell_process","path":"tests/honesty.rs"}` — a **content scan**,
not the hot-path signal. That scan is a `git grep` for

```
(^#!)|((^|[^[:alnum:]_])(subprocess|execute_process|exec)([^[:alnum:]_]|$))
```

`tests/honesty.rs` is `#![allow(clippy::unwrap_used, clippy::expect_used)]`, a Rust crate-level
inner attribute whose first two bytes are a shebang's, and which the first alternative matches.
`hasHighSignal` returns true for any signal, so one match carries the whole candidate to the top tier.

What was checked in that file, at **this** repository's head rather than at the head C scanned:
`Command::new`, `.spawn(` and `process::` return nothing; `spawn` appears twice in
`tests/honesty.rs`, inside assertion strings; no `subprocess`, `execute_process` or standalone `exec` appears.
Which line C's own grep matched is not in its recorded output, and the clones are deleted.

Absent that signal the candidate carries no other and is not passive-only, so it classifies `medium`
and takes one lens. **That corrected figure is derived from reading C's source, not observed** — no run
produced it. Every figure below depending on the lens count is given both ways.

### What C decides before any model is called

C classifies risk and selects reviewers deterministically, in Go, and publishes the result in a
consent prompt. Against this candidate, twice, on two separate clones:

| | as answered | as derived |
|---|---|---|
| `risk_level` | `high` | `medium` |
| lenses | four | one |
| `correction_budget` | 200 lines | 200 lines |
| corrections permitted | 1 | 1 |

**Size did not choose the tier.** `ClassifyRisk` takes a signal or hot-path hit first, and its own
comment says *"Change volume, file count, model, provider, profile, and effort are intentionally not
classifier inputs"*; `CountChangedLines` runs anyway and *"its result deliberately does not reach the
decision"*. The 2,199 lines are reported and unused.

**A tier with no reviewer exists.** `SelectReviewLenses` answers `RiskLow` with `[]string{}` — zero
lenses, no model call — reached when every authored path is passive content and no configuration is
touched. Passive is computed from candidate bytes, not declared by the caller.

That tier would not have covered this change, and it would not cover this repository's `skill/`
either: `isOperationalMarkdownPath` walks every segment of the path, not the filename alone, and
excludes any segment tokenising to `skill`, `agent`, `prompt`, `instruction` or `workflow` — so the
leading `skill/` disqualifies the whole tree, on the stated grounds that such documents *"are consumed
as instructions rather than read as prose"*. It would cover `docs/`, `README.md` and `CHANGELOG.md`.

### What each side sends a reviewer

C materialises the whole candidate patch into the prompt and forbids the reviewer from reading
anything else — *"Do not read the working tree, the index, HEAD, or any other file, and do not run any
command."* The reviewer process is launched with no tools.

| lens | bytes |
|---|---:|
| `review-risk` | 158,056 |
| `review-resilience` | 158,070 |
| `review-readability` | 158,102 |
| `review-reliability` | 158,081 |
| **four-lens total, as answered** | **632,309** |
| **one lens, as derived** | **~158,000** |

The one-lens figure is rounded because which lens a `medium` tier selects was never observed; the four
measured prompts span 46 bytes. `review_tier` now names it: the `focus` argument, `reliability` by
default, as in the source project.

`review_tier` also takes a `raise`: one of six risk items (`authorization`, `migration`,
`input`, `concurrency`, `irreversibility`, `contract`) with a reason. A raise lifts
the tier to `high` and cannot lower it, so it can only buy the four lenses above where the diff signals
bought fewer; the answer keeps the derived tier as `computed_level` and echoes the item and reason.
`focus_reason` is echoed beside a `focus` the same way. `review_tier` itself
is read-only and stores nothing; `publish_review` and `republish_review` take the same declaration and record the
raise, its reason, the focus, its reason and the computed tier on the published marker, which `review_read` with
`view: transaction` answers. The tier the gate derives (`review_status`, the computed verdict, `release_ci`) takes the
larger of the diff's tier and `high` when a recorded raise names one of the six items, so a raise buys the four lenses
and never fewer; the lens counts per tier are unchanged. The limit is that a raise is a caller's word: it can add
review and nothing checks that the item applies. [`honesty-review-tier.md`](honesty-review-tier.md) says what else
it does not cover.

The orchestrator hands a judge the receipt's payload as primary evidence — Estigia materialises it
and answers `--inline-payload`; the handing is an instruction — and names the claims the change
makes about text the payload does not show, reserving the checkout for those; see
[judgment day](../skill/policies/judgment-day.md).

The figures below predate that shape — the handing, not the rubric. Three of the six judges on
the issue's three receipts reported, through the agent harness's own completion reports: **147,249**,
**178,100** and **171,667** tokens — each handed a checkout to search. Under the shape described
above the same harness reported **121,973** and **145,840**, and later **105,250** and **97,485**.
Which shape a figure came from is part of the figure, and the comparison below is drawn on the older
one.

**Those two columns are bytes and tokens and must be converted before they can be compared.** At four
bytes per token — a ratio this measurement assumes and does not measure — 632,309 bytes is roughly
158,000 tokens and 158,000 bytes roughly 40,000. So the four-lens panel C ran is about **one** Estigia
judge, and the one-lens panel derived above about **a quarter** of one. The issue was given six judges.

What that compares is the size of what each system hands a reviewer. It does not compare how fast
either reads it or how either grades what it finds; nothing here measured either.

### The ceiling

The measurement above concludes *"the multiplier is the round count"*. C bounds that number:
`MaxCompactCorrectionAttempts = 1`, and the one correction is capped at
`min(200, ceil(originalChangedLines / 2))` (`risk.go:227-232` at `4db804e5`; `risk.go:230-234` at
`v4.0.0`, where `MaxCorrectionChangedLines` and `MaxCompactCorrectionAttempts` are unchanged at 200 and 1) — 200 lines here. **Corrected: an
earlier version of this line read `originalChangedLines/2` with no round-up, which misreported C's
own code rather than its behaviour** — the Go rounds an odd count up, and Estigia's own port
(`src/transport/tier.rs`'s `correction_budget`) matches it. Exceeding either escalates to a successor
lineage requiring a maintainer's authorisation.

Estigia now bounds the same two things, and both are enforcement. The count: a lineage — an original
candidate and its repairs on one pull request — gets the corrections the `Corrections per lineage`
row allows (default 3, where C allows 1), and the publish call refuses the next one as
`correction-already-spent` before anything is pushed (`src/transport/claim.rs`'s `corrections_spent`
and `corrections_per_lineage`; `docs/honesty.md` records what the count does not read). Each
correction a lineage spends is another round the panel's cost multiplies by, but a round over an
honest correction range hands each lens, and the refuter that judges their findings, only `parent-head..head` (a lens also gets the parent's findings), not the
whole target; a round whose range is not honest (`delta-range-reason` recorded) hands the whole
target again. The setting that used
to bound rounds, `Repair ceiling`, was instruction rather than enforcement; it is retired, and the
new row is enforced by the same refusal. The size: the correction budget (`skill/bindings/github.md`, `src/transport/claim.rs`'s
`correction_budget_check`) refuses a repair publication whose cumulative changed lines exceed the
frozen budget, before anything is pushed. A republish over an approved receipt is a new candidate and
a new lineage, so it spends no correction and stays bounded by its pull request's budget and scope.
The correction is also read by a targeted validator, as in C, but in addition to the tier's own lenses
rather than instead of it, so the spend a correction saves in C is not saved here. C's other half, a
successor lineage requiring a maintainer's authorisation, exists here as `estigia recover`, a person's command: it releases
nothing, so a recovered change pays a full new review, and its tier floor means the successor never
reviews with fewer lenses than the lineage it replaces.

C also asks a human before each medium- or high-risk review, `blocking: true`, and grants nothing
forward: *"nothing is granted for later candidates, so each later medium- or high-risk candidate asks
again."*

### Three refusals

Two destroyed reviewer executions; the first reached no reviewer:

1. `review capture-result accepts either --repository-context or --cwd, not both` — the contract
   renders the exact tokens for each step and refuses a request that adds one. **No reviewer ran.**
2. `reviewer lens context emission already exists with different bytes` — measuring a lens's prompt
   with `review lens-context` records an emission and binds the lineage, so the later real run is
   refused. **181 seconds of reviewer execution were discarded.**
3. `decode reviewer result: json: unknown field "lens"` — the reviewer returned one key the schema
   does not allow, and `additionalProperties: false` discarded the whole result. **265 seconds of
   reviewer execution were discarded.** This is C's own reviewer breaching C's own contract.

The second and third are lineage behaviour, not visible in that project's source. Under
`MaxCompactCorrectionAttempts = 1`, a reviewer that emits a malformed object has spent one of the few
executions the design allows.

### What this measurement does not settle

`docs/honesty.md`'s review-cost entry carries the limits on this measurement, as it does on the first.
Three are worth stating here.

**Every citation above into the measured project is unfalsifiable from this repository.** The commit
hashes, the constants, the quoted Go comments and prompt strings, the regex, the per-lens byte figures
and the two durations are all relative to a checkout no reader confined here can open.

**No reviewer result was ever admitted**, across three attempts. So this compares what each system
sends a reviewer, how many it selects, and how many corrections it permits — and compares **nothing**
about what either finds or how it grades. Only the measurement above does that, between two Estigia
agents.

**No artifact of this measurement is retained here.** The classifier output, the consent prompt, the
lens byte counts and the two durations were read from a binary's stdout against clones since deleted.

Nothing here timed the two systems end to end.

## What the comparison is between

The operator ruled, 2026-08-31: compare layer against layer, and estimate whatever exists on one
side and not the other. Never compare the totals — Estigia carries a task-flow layer (claims,
workflow states, PR delivery) with no counterpart there, so a single per-issue
ratio between the two divides two different systems.

Citations into the comparison project's own checkout are unfalsifiable from this repository, the
limit `docs/honesty.md` already states of the figures above.

| layer | here (Estigia) | there (C) | comparable |
|---|---|---|---|
| lens | 0 / 1 / 4 across low/medium/high (`RiskLevel::lenses` in `src/transport/tier.rs`) | 0 / 1 / 4 across low/medium/high (`internal/reviewtransaction/risk.go:132-142`, cross-checked at `internal/cli/review_start_contract.go:394-409`) | yes — a role on both sides, at the same counts |
| refuter | one, sealed, holds no `Read`, writes nothing (`src/harness/roles.rs`) | one, part of a role vocabulary closed at three (`internal/reviewerprovider/contract.go:12-16`) | yes — a role on both sides |
| targeted validator | one, sealed, read-only, holds no `Read`, writes nothing (`src/harness/roles.rs`'s `RESERVED_VALIDATOR`); required of a correction beside the tier's lenses | one, part of the same closed vocabulary (`internal/reviewerprovider/contract.go:12-16`), the terminal event of a correction | yes — a role on both sides; here it adds to a correction's panel where there it replaces it |
| blind judges | none at any tier, since `RiskLevel::judges` was removed; they ran 1 / 2 / 2 across low/medium/high before it, and Judgment Day is the opt-in | installed as agent markdown, which a skill launches, with no reviewer role and nothing in its Go that runs one | no — nothing in its Go runs one |

The comparison project's reviewer role vocabulary is closed at three — lens, refuter,
targeted-validator (`internal/reviewerprovider/contract.go:12-16`) — so there is no role a judge
could be dispatched under. Its `jd-judge-a` and `jd-judge-b` are agent markdown it installs:
measured 2026-09-10 at `a537c63e`, `jd-judge` appears on 39 non-test lines of Go source across ten
files — its model tables and TUI pickers, its SDD asset writers, its uninstall list, and
`internal/components/sdd/boundedreview.go:189`, which rewrites a section of the markdown file. The
one counter that ever measured a judge is decoded so an old file still parses and then discarded:
`LegacyJudgeCount` is declared at `internal/reviewtransaction/transaction.go:96` (still there at
`v4.0.0`), and the assignment carries only `persistedCounters`, which has no field for it.

Neither of the two figures the campaign has been treating as a judge target is a judge figure.
~40,000 tokens per reviewer is this document's own four-bytes-per-token conversion of ~158,000
prompt bytes — what one **lens** is handed, under *What each side sends a reviewer* above. 181
seconds and 265 seconds are the two **lens** executions destroyed under *Three refusals* above; no
result was admitted from either. A judge-versus-judge figure does not exist on either side, and none
can be produced from the comparison project's source.

The blind judges are reported beside the layer comparison rather than folded into it, and they are no longer part of ordinary review at all. Before it, `RiskLevel::judges` licensed
1 / 2 / 2 across low/medium/high, a licence and not a release requirement since the verdict became computed
(`release_ci` reads the computed verdict and no judge's word), whereas the comparison project closed a
passive review with no reviewer at all. That layer was removed: ordinary review is the lenses,
the refuter and the validator on both sides, and Judgment Day, which replaces the lens review for its
target and "carries no delivery authority" in the comparison project, quoted under *Where the rule
lives, and where it is not routed* above, is the one place judges exist. A typical
`medium` review was measured at about 450,000 tokens with two judges and roughly a third of that without them; the
figure is an estimate and no run here reproduced it. A total that folds the judges in compares a
stack that ran this layer against one whose Go never does, and none is left to fold in.

`docs/honesty.md`'s review-cost entry records what this leaves unmeasured.

## What a tool answer costs

The second half: four tool answers — `publish_review`/`republish_review`,
`record_review_finding`, `record_review_verdict` (retired since; the rows below are
kept as measured) — carried fields nobody but a rare caller acts on,
on every ordinary call. This measures what that cost and what moved.

**Method.** Two ways, both used below and named per row. *Reconstructed* means the exact JSON shape
the pre-edit and post-edit source builds, assembled in a throwaway `python3 -c` script with
representative field values (realistic-length hex identities and prose) and measured with
`json.dumps(obj, separators=(',', ':'))` — byte-identical in output shape to `serde_json::to_string`'s
own compact, no-whitespace serialisation, so the count is exact for the structure and representative
for the text. *Measured* means the number came from a real `cargo test -- --nocapture` run of the
named test against the actual code in this change, reading the figure from a temporary `eprintln!`
placed beside the ceiling assertion and removed again — the shipped test asserts the ceiling only,
so the temporary print is what made the figure visible and the assertion is what keeps it honest. Neither method used a worktree or a scratch file outside this checkout: the
reconstruction is a `python3 -c` inline computation (no file written anywhere), and the measured
figures came from tests already in the tree.

| Scenario | Method | Before (bytes) | After (bytes) | What moved |
|---|---|---:|---:|---|
| `publish_review`, first publication, no inline payload, one lane started | Reconstructed | 1,309 | 616 | topology, lineage (absent on a first publication), lane detail, autoclose |
| `publish_review`, repair over a parent, `inline` topology with a reason, branch-link autoclose, two lanes started | Reconstructed | 2,089 | 617 | topology + reason, full parent receipt + delta + delta_range, lane note/workflows/count, autoclose verdict, mandatory_follow_up |
| `publish_review`, first publication, branch-link autoclose — measured on `a_branch_link_publication_keeps_the_follow_up_out_of_its_answer` | Measured | — (the pre-edit shape is not runnable from this tree; see the reconstructed rows above for the same fields) | 610 | same as row 1, plus autoclose + mandatory_follow_up |
| `record_review_finding`, first write, `suggestion`, every optional absent | Reconstructed | 248 | 144 | `parent`/`origin`/`evidence_class`/`causal_disposition`/`location`/`lens`, each omitted rather than `null` |
| `record_review_finding`, replay, `severe`, every optional present — measured on `a_replay_of_a_recorded_finding_answers_from_the_marker_even_when_the_diff_cannot_be_read` | Measured | — | 213 | nothing moved for this row (every field here has a value); the ceiling guards against regrowth |
| `record_review_verdict`, `accepted`, long basis + never-ran evidence | Reconstructed | 853 | 518 | `basis` and `never_ran_evidence` text, replaced by `basis_bytes`/`never_ran_evidence_bytes` |
| `record_review_verdict`, `accepted`, never-ran evidence and no basis — measured on the retired verdict's never-ran test | Measured | — | 593 | `never_ran_evidence` text |

The reconstructed and measured rows agree closely where they overlap — 616 reconstructed against 610
measured for the same first-publication shape — which is what makes the reconstructed method
trustworthy for the shapes this tree cannot currently run both sides of (the pre-edit code no longer
exists in this checkout to execute against).

**What did not move.** `publish_review`/`republish_review` keep `ok`, `issue`, `pr`, `url`,
`created`, `draft`, `epoch`, `head`, `base`, `digest`, `payload_digest`, `payload_bytes`,
`publication_lane` (the state word only) and `next`, plus `payload` when `inline_payload` was asked.
`record_review_finding` keeps every field that has a value. `record_review_verdict`, while it
existed, kept `basis_given`/`classification` and the outcome/identity fields and answered the two
free-text fields as byte counts.

**Where the removed detail lives now.** The new read tool `review_read` (`SCRIPT review-read --issue
<n> --view transaction [--epoch <id>]`) answers the topology, the repair lineage, the
publication lane's note/workflows/count and the autoclose verdict with its mandatory follow-up, for
the latest publication or a named epoch — see `skill/bindings/github.md`'s own row. No field a
caller acts on was removed; it was relocated to a call that costs nothing when nobody needs it.
Every one of these fields is also recorded on the `published` marker itself (the durable record in
the local store that a later run reads), which is what `review_read` reads
back from — it does not hold a second copy of the fact.

**Guarded against regrowth.** `publish_review`'s own answer stays under 900 bytes on the
branch-link fixture (measured 610), `record_review_finding`'s replay answer stays under 260 bytes on
the full-optional-field fixture (measured 213) — the retired verdict's 650-byte ceiling went with
its test — each ceiling carries measured headroom over the
real figure, not a production limit; a future field added to one of these answers is expected to
raise the ceiling deliberately, not slip under a stale ceiling with no test to catch it.

### What the tool menu itself costs

The consolidation, measured 2026-10-09. Every host that loads the schemas up front pays
this on every session, whether or not a tool is ever called, so the advertisement is the one answer
whose cost is paid by not acting. The figure is the compact `serde_json::to_string` of the `result`
object a real `estigia mcp` process answers `tools/list` with, newlines normalised to `\n`, so it is
the exact bytes a client receives for the table.

| | Advertised names | `tools/list` result (bytes) |
|---|---:|---:|
| Before — 48 acting tools plus 3 `NOT_EXPOSED` refusal schemas | 51 | 75,913 |
| After — the consolidated table, `NOT_EXPOSED` no longer advertised | 32 | 55,381 |
| Later — descriptions trimmed, from 56,133 at that head | 32 | 44,325 |
| Issue #5, `TOOLS` at 33 — shared argument text, no refusal-code lists, no `lens` argument; context unreadable | 33 | 42,643 |
| Issue #69, `review_tier` gains `raise`, `raise_reason` and `focus_reason` (ceiling set to the measured size) | 33 | 43,055 |
| Issue #69, `publish_review` and `republish_review` take the same four arguments (ceiling set to the measured size) | 33 | 44,277 |
| `send_message` and `list_sessions` join, answered from the local store (ceiling set to the measured size) | 35 | 45,266 |
| `list_sessions` gains bounded discovery: seven optional selectors and paging arguments (ceiling set to the measured size) | 35 | 46,642 |

Nineteen names left the surface and 20,532 bytes went with them. The four capture-only recorders, the
retired aggregate verdict and the three `NOT_EXPOSED` names were removed outright; `reclaim`,
`list_identities`, `review_tier_for_repair`, `expected_target`, `base_movement`, `review_findings`,
`review_transaction`, `computed_verdict`, `review_payload_membership`, `inspect_candidate`,
`spend_summary`, `publication_lane` and `repair_rounds` were absorbed by a surviving name, each
reached through a parameter or a `view` rather than a schema of its own. `NOT_EXPOSED` still declares
`reclaim` for the seam test, but it is a declaration and not a schema, so it is not advertised. The
measurement is reproducible: `estigia mcp` answering `tools/list` on the built binary, with the
`result` object serialised compactly.

The trim touched the descriptions only: names, argument names, types, enums, minimums and
`required` lists are unchanged. The table had drifted to 56,133 bytes by then because nothing bounded
it, and `TOOLS_LIST_CEILING` in `src/harness/mcp/tests.rs` now does, so growth past the registered
ceiling fails without a reviewed edit to that constant. What the trim removed was text that did not help an agent
call a tool: where a field moved to, and a measurement of the payload threshold. That measurement
stays here: answers carrying 73,272 and 145,756 payload bytes were refused by the MCP surface and
one carrying 36,288 was not, so the threshold sits inside the ordinary range, which is why
`inline_payload` defaults to false.

Issue #5 made the list depend on the process's `GateContext`, which is fixed for the process, so
`listChanged` stays false. Where `Workflow` is `assisted` the list omits every `TRACKER_WRITERS` tool,
which that workflow refuses; under any other readable workflow it omits `ASSISTED_ONLY`; a context that
cannot be read, or whose workflow is unknown, lists all 33. A hidden name still dispatches and refuses
with its own typed reason.

| `tools/list` result, issue #5 | Names | Bytes |
|---|---:|---:|
| Before — every context | 33 | 44,318 |
| After — context unreadable (every tool) | 33 | 42,643 |
| After — autonomous | 31 | 40,901 |
| After — assisted | 13 | 14,380 |

`TOOLS_LIST_CEILING` is the unreadable-context figure, the largest list a process can answer. Three
changes made the rest of the difference: the receipt arguments (`epoch`, `pr`, `head`, `base`,
`digest`), `run_id` and `issue` each carry one short description defined once; the
`collect_review_input` and `launch_review_input` descriptions no longer list refusal codes, since each
refusal names itself; and `record_review_finding` no longer advertises `lens`, which exists only to be
refused. A caller that still sends `lens` through the tool is told it does not take it; the transport
still refuses it `recorded-by-capture-only`.

`target` answers the brief form by default, so the manifest, about 100 bytes per tracked file, is
paid only by a call that passes `manifest`.

`republish_review` stays a tool of its own although its arguments equal `publish_review`'s. Its
separate name is the guard on a call that destroys remote history, and a flag on `publish_review`
would let an ordinary publication reach it by one wrong argument.

## Waiting for a reviewer instead of polling

`collect_review_input` used to answer `capture-running` while a launched reviewer was
still working, so a run waiting on a lens polled it — one full-context turn per poll. It now takes
an optional `wait_seconds` and blocks up to that many seconds, capped at 900 and never past the
point the job would be abandoned, for the job to finish before refusing.

**Method.** The same corpus read `scripts/measure-transcripts.py` makes: every
`mcp__estigia__collect_review_input` tool call in the Claude transcripts under
`~/.claude-personal/projects`, paired with the `tool_result` that answers it. Measured 2026-10-09.

| | sessions reaching the tool | `collect_review_input` calls | answered `capture-running` |
|---|---:|---:|---:|
| Before, corpus | 1 | 16 | 1 |
| After | — | — | 0 by construction |

The corpus under-counts this shape badly: `launch_review_input` and `collect_review_input` were
introduced on 2026-10-07, and one session in the whole corpus uses them. The one `capture-running`
it holds is a real poll that `wait_seconds` removes; it is not the forced `Stop` turns the corpus
holds, most of which are not review polls.

**The after is by construction, not observed.** With `wait_seconds` at or above the job's remaining
runtime the call returns the recorded answer and never answers `capture-running`, and a job past its
deadline is still killed and refused `reviewer-deadline` with nothing recorded. Named tests drive
both acceptance paths —
`a_bounded_wait_records_the_answer_once_the_reviewer_finishes` (the job finishes inside the wait and
is recorded) and `a_bounded_wait_that_elapses_is_refused_capture_running_and_writes_nothing` (still
running, nothing recorded) — and `docs/honesty.md` records that no post-change session exists in the
corpus to measure the after.

## The orchestrator's own phase is recorded, not measured

`review_read` with `view: spend` and `phase: orchestrator` answers whatever `record_spend` was asked to record for that
phase — the delivering run's own report of its context, tokens and time, exactly like every other
phase that view already answers. Nothing here re-derives it from a transcript or an agent
harness completion report; it carries exactly the limit `docs/honesty.md` already states —
*"`record_spend` is a declaration, not a measurement"* — because Estigia has no channel into the
harness that ran the orchestrator either, the same as it has none into any subagent's. The delivery
step now records one `record_spend` for `orchestrator` and one for each delegated phase against the
issue's review transaction, before the transition to `done` — `skill/SKILL.md`'s developer step 6
states this inside the delivery sentence; `tests/honesty.rs` pins that sentence's release clauses,
not this one, so the spend duty is held by prose alone.

## The reviewer prompt order buys no cache hits, and is not shipped

It was proposed to move the role template to the end of the prompt, so that the receipt, the
criteria and the diff — the same bytes for every lens — would lead it and could share a
prompt-cache prefix, where today `build` in `src/reviewer/prompt.rs` places the role's own template
first and every lens therefore prefixes the shared bytes differently. Measured 2026-10-09 against
a real review target; the reorder is **not shipped**, because the shared bytes are never
cache-read in any position.

**Method.** Four calls to `claude -p --model claude-sonnet-4-5 --output-format json`, reading the
`usage.cache_read_input_tokens` and `usage.cache_creation_input_tokens` its JSON reports. The
prompts are the bytes `build` produces for a real lens over that target (68,164 bytes) — sent as
they stand, then byte-identical again, then with the role's template moved to the end (68,170
bytes) — and, last, an unrelated two-word prompt that shares none of the reviewed bytes.

| call | prompt | bytes | cache read | cache created |
|---|---|---:|---:|---:|
| 1 | lens prompt, role first, cold | 68,164 | 0 | 70,980 |
| 2 | the same bytes again | 68,164 | 18,289 | 52,717 |
| 3 | the same prompt, role moved last | 68,170 | 18,289 | 52,770 |
| 4 | an unrelated two-word prompt | — | 18,289 | 32,665 |

**What the numbers say.** The read-back is **18,289** on three calls that share nothing but the
binary — the byte-identical repeat, the reordered prompt, and an unrelated two-word prompt — and
**0** on the one cold call. The last of those shares none of the reviewed bytes, so 18,289 is the
CLI's own invariant prefix and not any of them. The receipt, the criteria and the diff all travel in
the user message, and no call ever cache-reads it: the byte-identical second call would have read
that message had it been cacheable, and it read exactly the 18,289 the unrelated prompt read. The
role's template sits in that same uncached user message whether it leads or trails, so its position
cannot share what was never shared.

**What this does not settle.** One CLI, one model and one machine on one day, so the figure is a
property of that reviewer and not of every reviewer. The CLI `launch_review_input` starts discards
its own usage report, so the prompt above was driven directly through the CLI rather than through a
recorded launch. And the four lenses run concurrently, so a prefix the CLI did cache would not be
reused by a lens that started before the first returned — the saving would need them serialised,
which the issue puts out of scope.

## Re-measuring after a change

The baseline (2026-09-24, 15 transcripts, recorded in the issue body) measured orchestrator
context by phase, forced `Stop` turns and per-issue input tokens from Claude Code's own `.jsonl`
transcripts. `scripts/measure-transcripts.py` repeats that measurement: `python3
scripts/measure-transcripts.py --projects ~/.claude-personal/projects` (the default), reading every
transcript it can reach and reporting a JSON object and a readable table, per repository and
overall — session count, input tokens by context (orchestrator / developer / reviewers / analyst,
taken from the `agentType` each subagent's sibling `.meta.json` records), forced-`Stop` turns (a `system` record
whose `stop_hook_summary` names `estigia` among its hook errors, and the next assistant turn's
context — the same detection the baseline scripts in `~/estigia-baseline/` used, adapted rather
than reimplemented from first principles), per-issue input tokens where a transcript names an
issue, and each repository's most recent session time.

It also reports the four usage classes — `input`, `cache_read`, `cache_creation` and `output` —
separately, per context, from each record's own counters. The mapping is `CLAUDE_USAGE_KEYS` in the
script and is this CLI's; a provider whose cached counters already sit inside its input total needs
its own. A class no measured record carried is `null` and never `0`, and a record missing any of the
four is counted in `usage.records_missing_a_counter`, so the classes read as a floor rather than as
complete. They change nothing outside themselves: against the base script, on the same corpus, the
two agree byte for byte on `session_count`, `context_input_tokens`, the forced-`Stop` figures and
`per_issue_input_tokens` (measured 2026-10-18: 179 sessions, 90,563 usage records, none of them
missing a counter). `tests/usage_accounting.rs` pins the classes against a hand-written fixture,
including the unknown and the missing-counter cases the live corpus does not exercise, and pins the
readable table that names them.

**Run on this machine, against this checkout, after the change landed and delivery sessions closed
against it.** The condition the baseline's acceptance item 6 waited on is now met: the corpus under
`~/.claude-personal/projects` holds delivery sessions whose own transcripts name a build carrying
those slices. The recent estigia transcripts carry build commits
`a327302d5bf5c4bdb99524d55dfb5ede726bd768` (2026-10-01),
`23f86174685383b3d7fee080bccf8c7f9b2ad225` (2026-10-02),
`cdf3c243cf52e0b2469394943df271745a458875` (2026-10-04) and
`fc0ed0a93f32ca315f44174a3d9499c84cceb1af` (2026-10-04, the build installed on this machine), and
`git merge-base --is-ancestor 14f4390e4c1418ed57a97bc9502cb342f6b53de5 <commit>` answers true for
each. The corpus is no longer only older than the change, so the re-read below is the post-change one
the acceptance item asked for.

The script itself is exercised now — its own readable-table output as it stood on 2026-10-04, on
the corpus as it stood then, is below (`--projects ~/.claude-personal/projects`, the default; full
JSON is reproducible with the same command, no `--json` needed to see it — it prints after the
table). That capture predates the class lines above: a run now prints a `usage classes` block after
the per-context lines, and a `records missing a counter` line under it when any record lacked one.
Every other line is unchanged. The session counts name the repositories the original 15-transcript
baseline already drew from and several the corpus has since grown; this is the same live corpus,
re-read by the adapted script rather than a fresh one. The blocks for repositories outside this
project are omitted, and so is the `overall` block, which summed them.

```
== estigia ==
sessions: 28
  orchestrator        750924008 input tokens
  developer           163699882 input tokens
  analyst              82380724 input tokens
  general-purpose      80567277 input tokens
  review-blind         37705968 input tokens
  implementer          37271515 input tokens
  fork                 12216553 input tokens
  Explore               4105428 input tokens
  review-lens           2967927 input tokens
  review-blind-sealed      2708363 input tokens
forced Stop turns: 39 (median context 183231)
per-issue input tokens:
  issue 3: 25634554 input tokens
  issue 439: 37226722 input tokens
  issue 482: 14279325 input tokens
  issue 490: 1974546 input tokens
  issue 621: 15741024 input tokens
  issue 623: 10230620 input tokens
  issue 626: 17246095 input tokens
  issue 629: 64061143 input tokens
  issue 637: 111099846 input tokens
  issue 639: 78847376 input tokens
  issue 652: 21216361 input tokens
  issue 656: 5910025 input tokens
  issue 659: 10854166 input tokens
  issue 675: 135884073 input tokens
  issue 679: 70825585 input tokens
  issue 684: 10160932 input tokens
  issue 685: 47055986 input tokens
  issue 698: 23457733 input tokens
  issue 705: 26203780 input tokens
  issue 707: 18488193 input tokens
most recent session: 1791170379.327

== leteo ==
sessions: 3
  developer           110024272 input tokens
  orchestrator         98176158 input tokens
  general-purpose      44833932 input tokens
  review-blind         13626038 input tokens
  analyst               6188884 input tokens
  review-blind-sealed       311575 input tokens
  review-lens             221073 input tokens
  review-reliability        34008 input tokens
  review-readback           32149 input tokens
  review-readability        31639 input tokens
  review-risk             31178 input tokens
forced Stop turns: 5 (median context 225696)
per-issue input tokens:
  issue 124: 64555620 input tokens
  issue 125: 1533678 input tokens
  issue 128: 32086860 input tokens
most recent session: 1791172401.976

== worktree-estigia-issue-482-overhead-part-two-opencode-f2fa1b ==
sessions: 1
  orchestrator        133305448 input tokens
forced Stop turns: 0 (median context None)
most recent session: 1790259258.5040534

== worktree-estigia-issue-496-macos-build-install-opencode-f2fa1b ==
sessions: 2
  orchestrator          8265225 input tokens
forced Stop turns: 0 (median context None)
most recent session: 1790289692.8350403

```

**The corpus-wide comparison against the baseline is omitted.** The baseline's 15 transcripts were
drawn mostly from a repository outside this project, and the figures that compared against them —
session count, forced-`Stop` turns and their median context, the orchestrator's share of input
tokens — summed every repository in the corpus. They are left out for the same reason as that
repository's block. The rows above are this project's own; the worktree rows are its delivering
sessions, attributed under their own label because a worktree is a sibling directory under `~/repo`
and `repo_of` takes the text after the `-repo-` boundary.

## What the Judgment Day policy once quoted, and what the skill costs every activation

Measurements and history were taken out of `skill/` so that an agent does not pay for them on
each activation. The Judgment Day measurements below were in `skill/policies/judgment-day.md`
and are kept here as they were stated.

- **The payload against the checkout, one receipt, the brief held constant.** Withholding the
  checkout entirely cost 26.5% fewer tokens and 84% fewer tool calls, with no loss of finding
  count. Handing the checkout over unqualified was measured too, and the judge searches. Neither is
  the policy, because a third measurement ruled the first out: twice on this repository a real
  defect has lived in text the diff never showed. A change removing three paths from `skill::FILES`
  and adding them to no retirement list rested on two doc comments; `RETIRED` appears zero times in
  that publication's payload, and so does the sentence the judge quoted. A change making the payload
  opt-in left two shipped policy documents still instructing the orchestrator to hand judges a key
  the answer no longer carries, and both judges of that panel found it independently, outside the
  diff both times. On one publication under the named-exception shape a judge left the payload
  exactly once, to count the entries behind a number the change moved.
- **Sealed against unsealed, one observation and not a study.** On one change's final panel, the
  sealed judge detected that the payload it was handed had been abridged and correctly withheld
  every claim about the elided regions, while the unsealed judge re-derived fourteen citations and
  found a stale contract sentence outside the diff. The panel reached the out-of-payload defect; the
  sealed judge did not, and was not meant to. This is not a controlled comparison: the two rounds it
  rests on reviewed different changes of different sizes. Per judge, 173,453 and 177,059 tokens over
  685 and 755 seconds became 121,973 and 145,840 over 457 and 535, on briefs of the same shape. What
  is controlled is the sealing measurement above, and what that one costs is the two defects named.

### The always-loaded set, before and after

Bytes are LF-normalised UTF-8, as `tests/instruction_ceiling.rs` counts them; the ceiling for each
edited file is now its measured size, so the registry row is the "after" column.

| File | Before | After |
|---|---:|---:|
| `skill/SKILL.md` | 31,965 | 31,578 |
| `skill/bindings/github.md` | 28,141 | 28,128 |
| `skill/bindings/github/delivery.md` | 38,331 | 37,708 |
| `skill/bindings/github/operations.md` | 50,330 | 50,290 |
| `skill/bindings/github/refusals.md` | 52,252 | 52,232 |
| `skill/policies/blind-judges.md` | 42,419 | 41,760 |
| `skill/policies/judgment-day.md` | 18,400 | 16,041 |
| `skill/protocols/rdd.md` | 8,809 | 8,282 |
| `skill/references/repository-delivery.md` | 21,612 | 21,040 |
| `skill/references/runtime-notes.md` | 18,578 | 18,434 |
| `SKILL.md` plus `bindings/github.md`, loaded on every GitHub activation | 60,106 | 59,706 |
| All shipped `skill/**/*.md` | 430,776 | 425,432 |

The saving on the always-loaded pair is 400 bytes, about 100 tokens at the four-bytes-per-token ratio
this document assumes: most of what was removed lived in policies and companions that load only when
a review or a delivery reaches them. The old registry summed to 430,747, not the 430,776 the files
measured, because a ceiling is an upper bound and a stale one below its file is not a refusal.


### The always-loaded pair, a second trim

Issue #4. Same counting as above, measured from `bd8f161`. No rule changed: each removed instruction is
either stated by its owner elsewhere in the skill or history that moved to `references/safety-incidents.md`.

| File | Before | After |
|---|---:|---:|
| `skill/SKILL.md` | 31,564 | 29,995 |
| `skill/bindings/github.md` | 28,128 | 17,674 |
| `SKILL.md` plus `bindings/github.md`, loaded on every GitHub activation | 59,692 | 47,669 |
| `skill/bindings/github/operations-guarantees.md` | 44,396 | 34,237 |
| `skill/references/safety-incidents.md`, which received the history | 11,662 | 16,227 |
| All shipped `skill/**/*.md` | 425,666 | 407,474 |

The operations table no longer repeats the flags the MCP schema carries, `operations-guarantees.md` no
longer has a Command column, and the tool names the skill sends an agent to are the ones
`tools/list` advertises: `no_shipped_skill_file_names_an_operation_no_tool_performs` fails on a retired
name outside the one absorption map in `bindings/github.md`. That test keeps no list of retired names: it takes
every name in operation position from the text, in `_` and `-` spellings, fenced blocks included, and resolves it
against `TOOLS`, `NOT_EXPOSED`, `NOT_RUN` and the CLI subcommands. Operation position is a name after `SCRIPT`,
after "call", before `with` and a `--flag`, or in the first cell of a table headed `Operation` (backticked or
bare), except a row the agent performs by hand, a segment led by `with` or `its`, and the field
`last_activity`. A name left over is reported as absorbed when the map's left-hand side lists it. A second test
holds the map to the code: none of its left-hand names is a tool, and the call each answers with exists. A new
absorption needs only a line in the map.
