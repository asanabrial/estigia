# Spec-driven planning inside the state contract

Read this when `Planning` is `sdd`, with or without `lite`/`openspec`. It replaces nothing in the Hard Rules, the
Decision Gates, or the state contract: it says what a run **produces** between claiming an issue and
closing it, and where those artifacts live.

## What this is not

This is not a workflow, and it does not replace the state contract in `SKILL.md`. That contract says how a *task* moves —
roles, states, claims, renewal — and it is the ground everything here stands on. This says how a
*change is planned* once a task is held, which is a different question with a different answer, and
the two are chosen separately because they compose.

It is not a second workflow beside the tracker's either. The workflow states belong to the transport,
which accepts exactly `analysis`, `ready`, `in-progress`, `review`, `blocked` and `done` and refuses
anything else. A phase list that tried to replace them would be discovered on the first transition,
halfway through a delivery. So the phases sit **inside** the states:

| Phase | State it happens in | What it leaves behind |
|---|---|---|
| explore | `analysis` | the bounded question, and what was ruled out |
| propose | `analysis` | the change being proposed, and the alternative not taken |
| spec | `analysis` → `ready` | the acceptance criteria, written before any code |
| design | `ready` | the shape of the change, and the decisions inside it |
| tasks | `ready` | the ordered work, each item independently checkable |
| apply | `in-progress` | the change itself, in the isolated checkout |
| verify | `review` | evidence against the criteria written in `spec` |
| archive | `done` | the record, joined to the delivering SHA |

An issue arriving already specified skips the phases that produced that specification. Re-deriving a
spec that exists is ceremony, and ceremony is what makes people stop following a method.

## When to engage it at all

**Ambiguity, and nothing else.** Engage the planning phases when the work carries real uncertainty
about *what* to build — when a written proposal, spec or design would materially reduce it.

Not size, and **not risk**. A change can be large, dangerous and completely understood; running eight
phases over it produces artifacts nobody needed and teaches people that the method is ceremony. Risk
belongs to the other axis: it strengthens verification and review, and it never forces planning.
`Review protocol` and `Blind judges` are where a risky change is answered.

An issue that arrives already specified skips the phases that produced that specification.
Re-deriving a spec that exists is the same ceremony wearing a different hat.

## Selecting `sdd` does not mean running the phases

It means they are **available**, and the rule above is what engages them — per change, on ambiguity,
by the run holding the issue. There is no second setting for this and there should not be: an option
that turned the decision on would be an option to turn it off, and what it would be turned off
*into* is the ceremony this whole section exists to prevent.

That is worth stating outright because the opposite is the natural reading. "Planning: sdd" sounds
like a promise of five artifacts on every issue, and a team that reads it that way produces four
documents nobody opens for a one-line fix, concludes the method is theatre, and stops. The setting
says which protocol is in force. This document says when it acts.

**Record the call, with its reason, before the work it authorises.** One line naming what was
uncertain — or naming that nothing was — on the issue, beside the acceptance criteria. Estigia
adjudicates rather than taking a run's word for anything, and an unrecorded "I judged this one
simple" is indistinguishable from skipping the protocol.

Two failures this invites, named so they can be recognised:

- **Deciding after the fact.** A run that builds first and records its reasoning afterwards has
  written a description, not a decision, and it will always conclude the work was clear.
- **Reading the rule as "skip unless forced".** It is ambiguity, not effort. A change nobody has
  specified and everybody pictures differently is exactly the case this exists for, and exactly the
  case where planning feels most like delay.

## The rule that does not bend

**The criteria are written before the code, and they are not rewritten around it.** This is X08 in
the incident ledger: *"an issue without acceptance criteria forces the implementer to invent the bar
it later claims to have met, and a partial improvement can then masquerade as completion."*

`spec` is where that bar is set. `verify` measures against what `spec` recorded, not against what
`apply` happened to produce. A spec that changes after `apply` starts is a new spec, and it is
recorded as one — with the reason — rather than edited to match the code.

## Where the artifacts live

Two answers, and the operator picks one in the configuration table.

**`sdd` — on the issue.** Each phase records its artifact through the binding's `comment` operation,
under a heading naming the phase. This is the default because the operator already chose the tracker
as the single source of truth: the spec sits with the claim, the timeline and the delivery record,
and a run on another machine reads it without a checkout.

**`sdd openspec` — under `openspec/` in the repository.** The layout follows the OpenSpec
convention:

```text
openspec/
  config.yaml        schema: spec-driven, context, strict_tdd, rules, testing
  <change-id>/
    proposal.md      what is being changed, and what is not
    spec.md          the acceptance criteria
    design.md        the shape, and the decisions inside it
    tasks.md         the ordered work
```

Choosing this is choosing a second place where the answer to *"what were we building"* lives. That
is a real cost and it buys something real: a spec that is reviewed, diffed and versioned with the
code it describes. What it does not buy is authority. **When `openspec/` and the issue disagree, the
issue wins** — it is the surface the claim is adjudicated on, and the one another run can read.
Record the divergence rather than resolving it silently.

`openspec/config.yaml` is convention, not a validated schema: nothing parses it into a contract, so
treat a field you do not recognise as somebody's note and leave it alone.

## What Estigia enforces here, and what it does not

**Enforced, mechanically:** the claim, the renewal before repository writes and at every irreversible
boundary, one run holding one task, and the state the tracker says the issue is in. Those are the
same under every methodology, because they come from the claim rather than from the method.

**Not enforced:** that a phase ran, that its artifact is any good, or that `verify` was honest. A
gate can see a write; it cannot see whether a specification was written before the code or after it.
That is a judgement, it stays a judgement, and a tool claiming otherwise would be the false comfort
this whole contract is written against.
