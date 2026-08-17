# Review contract adjudication

Move the review contract from prose the run may ignore to a verdict the authority can refuse — as far
as it can actually be taken, and no further.

## Why

`skill/SKILL.md` states it as a Hard Rule:

> Review MUST use a context that did not write the change.

Nothing checks it. `docs/honesty.md` already says so, in its own words:

> **Estigia cannot prove reviewers or blind judge panels ran.** `publish_review` mechanically freezes a
> coherent clean draft receipt over epoch, PR, head, base and manifest digest [...] It does not prove
> an independent context existed or establishes panel size, concurrency, independence, blindness,
> same-finding identity or quorum. It cannot prove one, two or five judges read those bytes or that
> their verdicts were honest. `single`, `two blind` and `five blind` remain operator-selected
> review contracts, not observations the harness can make.

That entry is honest and it is also an admission: the most-repeated rule in the contract is the one
with no machine behind it. This crate exists because *a claim is adjudicated, not asserted*, and the
review contract is currently asserted.

**The sister case, measured elsewhere on 2026-08-13.** A repository whose `AGENTS.md` demands "las
cuatro (4R)" at its highest risk tier never names the four. The expansion lives only in prose inside
historical changelog entries, and the same file insists — three lines above — that the bar must not
be routed to an external source. A run reading only the canonical file cannot know the lenses, so it
declared its own: *correctness, regression + ops, security, design and docs accuracy*. None of the
four. The gate fired and decided nothing repeatable: two runs over the same change review different
things, and both report the contract satisfied.

That is not that repository's bug to fix alone. A review contract that names a count without naming
its members is a contract no authority can adjudicate, and Estigia is the authority.

## What is provable, and what is not

The value of this change is in keeping the three tiers apart. Collapsing them is how a gate that
decides nothing gets built and believed.

**Tier 1 — refusable in this design.** Estigia already mints run ids, binds every write to one through
`verify_claim`, and `publish_review` freezes the reviewed bytes against the publishing run. So these
are mechanical:

- A verdict recorded by the run that published the work. Same run id, both sides, already known here.
- A verdict against a digest that is no longer current. `A verdict is bound to exact bytes; every
  push invalidates it` is already the rule; a verdict arriving after a push is a stale one.
- A verdict naming **no lens**, or naming a lens outside the set the repository declared. This is the
  4R defect, closed at the authority rather than in each repository's prose.
- Fewer verdicts than the configured contract requires — `single` versus `two blind` is already an
  operator-selected axis, and counting is not a judgement.

Only the first two exist today. The shipped operation records one aggregate exact-receipt verdict; it
carries no lens, per-judge outcome, panel count, finding identity or sealing order, and therefore does
not enforce panel size or quorum.

**Tier 2 — bindable, not provable.** Two judges being blind *to each other* cannot be observed. What
can be recorded is ordering: each verdict sealed against the frozen digest before the next becomes
readable. That makes a verdict written after reading the other's distinguishable from one that was
not — by sequence, not by honesty.

**Tier 3 — not provable at all, and it stays in the honesty contract.** That an independent context
existed. That anyone read the bytes. That a verdict is honest. Nothing proposed here changes those,
and the contract entry above must survive this change with those three clauses intact. If it comes
out whole, this change overclaimed.

## What is being changed

- Per-judge records, each bound to the frozen digest, the reviewing run id, and the lenses that judge
  claims to have applied. They extend today's one aggregate marker; that marker is not a panel
  transcript and cannot compare judges or their lenses.
- A **declared lens set**, read from the repository the way the settings table is read now. A count
  with no members is refused at configuration time rather than at review time.
- Refusals for the Tier 1 list, each naming what it prevents.

## What is NOT being changed

- No claim that reviewers ran, were independent, or were honest. See Tier 3.
- The review contract axis stays operator-selected. This adjudicates the contract chosen; it does not
  choose one.
- No parsing of arbitrary consumer CI YAML — the honesty contract already refuses that, and a lens
  set is not a place to start.
- No reinterpretation of today's single aggregate marker as a panel transcript. Until structured
  verdicts ship, panel count, same-finding identity and quorum remain prose and honesty boundaries.

## Open questions

1. Where does the lens set live? The settings table is the obvious home, and the sister case argues
   the set must be **owned by one file** rather than referenced — that is the defect it hit.
2. Does a verdict become a first-class tracker artifact, or a marker on the timeline beside
   `heartbeat`, `branch` and `published`? The second is cheaper and already has a reader.
3. Does the same-run refusal need an escape? A solo operator reviewing their own work in a second
   context is the case `two blind` exists for, and refusing it outright may push people off the gate
   — which `sdd.md` already warns is how a method becomes ceremony.

## Status

Draft, no issue filed. Third of the notes taken on 2026-08-13, with
[`loop-goal-adjudication`](../loop-goal-adjudication/proposal.md) and
[`companion-installation`](../companion-installation/proposal.md). Of the three, this is the one that
touches a Hard Rule.
