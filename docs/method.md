# The method

**Status: draft.** This document states the method Estigia means an agent to work by, and it decides
no behaviour. The contract an agent reads today is [`skill/SKILL.md`](../skill/SKILL.md); where the
two disagree, the skill is what an agent follows and this document is the one that is wrong.

## The premise: the harness carries the workflow, so the agent need not

Estigia does not ask an agent to follow a workflow; it holds the tools. That is what lets the method
be light. A rule the harness refuses to let anyone break does not have to be remembered: the agent
meets it at the moment it applies, as a refusal whose `action` says what to do next, or as the
`next` step `review_status` answers. A rule written into always-loaded text costs context on every
turn, and is still only as good as the agent's attention.

So the method has two halves. **The flow** is the little an agent carries. **The harness** carries
everything else, and the agent learns each piece of it only when it crosses it.

## The flow

An agent working an issue carries six steps and three rules.

1. **Claim.** `claim` the issue, with `transition_to` when it moves. Nothing is written before it.
2. **Reproduce.** A test that fails against the code as it stands, for the reason the defect exists.
   Work with no deterministic test says so instead.
3. **Build** in the worktree the claim gave. Work of two or more meaningful steps records its tasks
   with `task` first; small work does not.
4. **Publish** with `publish_review`, asking for `with_review`.
5. **Follow `next`.** Do what `review_status` answers until it stops.
6. **Deliver and release.** Merge at the reviewed head when authorised, then `release` with the
   evidence.

The three rules:

- **A refusal is an instruction.** Do what its `action` says; do not retry the same call unchanged.
- **Report only what was read back.** Never the state a write should have produced.
- **An unreadable answer is not an answer.** A failed read is neither permission nor a stand-down:
  report it and retry.

## What the agent does not carry

Each of these is a rule the method depends on and the agent never has to hold in mind, because the
harness refuses a call that breaks it.

| Rule | Held by | How the agent meets it |
|---|---|---|
| A claim is the only authority to write | the tool gate and the `pre-push` hook | a refused write or push naming the claim |
| A verdict authorises exactly the bytes it was computed over | the receipt digest; `release_ci` | `release-target-mismatch`, then a republish |
| Delivery waits on a computed `approved` verdict | the `gh pr create` and `gh pr merge` boundaries; `release_ci` | `review_status` does not offer the next step |
| A lineage gets a bounded number of corrections | `publish_review` | `correction-already-spent`, then `escalate_review` |
| A moved head or base invalidates the verdict | `review_status`, comparing the pull request with the receipt | `pull-request-moved`, then `publish_review` |
| An escalated review continues only through a person | `estigia recover`, which an agent's shell line cannot run | the stop `review_status` reports |

[`protocols/rdd.md`](../skill/protocols/rdd.md), [`policies/blind-judges.md`](../skill/policies/blind-judges.md)
and the binding's references own the detail of each, and an agent loads one only when its condition
arises.

## The test for a sentence in the flow

A sentence belongs in the always-loaded flow only when both hold:

- **the harness cannot hold it** — no refusal, readback or `next` step enforces it; and
- **the agent needs it before anything could tell it** — by the time a refusal arrived, it would be
  too late or there would be no refusal at all.

Anything else goes to the harness, to a refusal's `action`, or to a reference loaded on its
condition. The step *Reproduce* passes the test: nothing refuses a fix without a failing test. The
claim's authority does not: the gate refuses the write, so the flow names `claim` as a step and
leaves the rule to the gate.

The size of what is always loaded is held by [`tests/instruction_ceiling.rs`](../tests/instruction_ceiling.rs);
a lighter flow is measured there, as a ceiling moved down.

## Where it came from

The sequence grew out of gentle-ai v4's ODD workflow and its receipt-driven review, which
[`parity.md`](parity.md) records as adopted with declared divergences. ODD's first four steps —
authorize, explore, resolve uncertainty, classify — are judgment an agent already exercises, and the
flow keeps only the classification that decides whether tasks are tracked. What Estigia adds is the
half the agent does not carry: the claim, the receipt and the gated delivery are where ODD's text
became Estigia's refusals.

## What this draft leaves open

- **The skill is not yet this flow.** `skill/SKILL.md` still carries the decision gates, both roles
  and the delivery rules as always-loaded text. Moving it to the flow above, sentence by sentence
  through the test, is the change this draft prepares.
- **The analyst role.** The flow covers working an issue; whether filing one is a second flow of the
  same size, or a reference loaded on request, is not decided.
- **The name.** The method is still referred to by the inherited names, ODD and RDD, in the skill,
  the tools and the retired settings. Renaming is a contract change — an installed
  `protocols/rdd.md` must be retracted the way the retired SDD files were.
- **[`parity.md`](parity.md)'s framing.** It still reads gentle-ai as the target Estigia measures
  against; once the method is settled, it answers where each part came from instead.
