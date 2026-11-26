---
name: flow
description: "Trigger: flow triage [conditions], flow dev [domain-rules] [issue-number], bare flow (the combined analyst-developer, routing to which issue from tracker state), flow review <pr> [<pr> …] (review pull requests this run did not build, in order), flow fix <pr> [<pr> …] (address the review comments on pull requests, in order), and the long forms analyst and developer. Also claim issue, move task states. Triage files at most one evidenced issue; without conditions, audit the repository critically."
license: GPL-2.0
metadata:
  author: asanabrial
  version: "1.17.0"
---

# Issue Flow — Analyst / Dev over a shared issue tracker

## Activation Contract

Use for any invocation the description above names, for analyst or developer work, claims, and
workflow-state transitions. `flow review` reviews each named pull request this run did not build, and
`flow fix` addresses the review comments on each; neither is a role, and both take their sequence from
the matching rows of [decision gates](references/decision-gates.md) under either `Claim workflow`.
`triage` and `dev` are short names for `analyst` and `developer`. No role word is the combined
analyst-developer, which loads both rule books and does both jobs; a role word names one role, `flow dev
<n>` also names that issue, and neither is inferred from anything else. Under `assisted`, `flow <issue>`
takes the assisted row of [decision gates](references/decision-gates.md) before any developer step and
with no claim.
Analyst conditions and dev domains are optional: a bounded target is the scope; otherwise the analyst audits
the repository and the dev implements the selected issue. Read repository instructions first and load only
the tracker binding the configuration selects; open every reference from the resolved skill directory, never
the install alias ([runtime notes](references/runtime-notes.md)).

## Hard Rules

- Issue Flow owns roles, states, claims, and transitions; routed domains own value, priority, evidence, and done criteria. Neither side invents the other's policy.
- An analyst, including an adversarial reviewer, is read-only: no branch, worktree, commit, PR, migration, sync, database write or delivery action, and no external system beyond the tracker operations required below. One bounded exception: a Judgment Day judge under `Evidence standard` `measuring` builds and mutates only inside the directory handed to it, outside the claimed checkout. It files at most one evidenced issue, save for the filing duty [blind judges](policies/blind-judges.md) places on the run that records the computed verdict — that run files one issue for every `severe` finding it records `pre-existing`, `base-only` or `unknown`, however many that is. Review MUST use a context that did not write the change.
- Mint one `<runtime>-<session-prefix>` run-id and reuse it for every write. Write no `analyst:<runtime>` or `dev:<runtime>` label: Estigia removes a leftover `dev:` label and adds none. Attribution stays in the local store: no GitHub text or label names the run-id or a tool, or hides a marker.
- Resume this run's held issue before consulting queues, and use the selected binding's claim: the store adjudicates, and the earliest live claim wins. **The gate performs the pre-write verification** — a repository write it observes is re-verified whenever the renewal window has passed and at every expensive or irreversible boundary — so no separate `verify_claim` is needed first. An unreadable control surface permits no write; under `assisted` ([decision gates](references/decision-gates.md)) the local `assist` record stands in.
- A closed `Git operation window` holds back only commits and remote writes: keep editing and testing, leave the work uncommitted, take the next issue with `claim` and `park: true`, and never seek `stand-down` or `disabled` to pass it ([delivery](bindings/github/delivery.md#the-repositorys-daily-schedule-for-pushes)). Tell the person to nudge it at `opens_at`.
- Editing only the title or body of an issue or a pull request, as one plain `gh issue edit` or `gh pr edit` with `--title` or `--body-file`, is no repository write: either role does it freely.
- Keep exactly one of `analysis`, `ready`, `in-progress`, `review`, `blocked`, or `done`. The binding's workflow state is authoritative; every projection is updated and read back, and `done` is explicit before tracker closure.
- A claimed task reaches `done` or a state that names its exact exit condition and discharger; it never enters an ownerless, conditionless queue.
- The selected binding MUST map `create`, `list_state`, `claim`, `reclaim`, `verify_claim`, `transition`, `comment`, `unassign`, `publish_review`, `republish_review`, `handoff_review`, `review_finding`, `review_computed_verdict`, `review_disposition`, and `release_ci`. Run those operations; declare what a binding cannot do and fail closed.
- Issue acceptance criteria, routed domain rules, and repository rules define done. Never invent missing gates or reinterpret the issue silently.
- Aim for no more than the `Change size` row below, in changed lines per pull request, but treat size as reviewability guidance: a larger coherent change is valid when splitting would reduce safety or review quality, recorded before review. **The candidate is the publication**, not a commit.

## Direct Work Delegation

Use the smallest useful topology and keep one writer. Delegate only when the expected work crosses
one of these boundaries:

| Work | Stay inline | Delegate |
|---|---|---|
| One batch of reads or lookups | At most 3 calls and about 10k tokens stays inline, even when it prepares a write | More than about 5 sequential lookups delegates one explorer with a ~2k-token handoff |
| Broad research or context compression | - | Delegate broad research or context compression |
| Writing | A small understood change stays inline whatever its file count | Delegate one writer only for a named reason: independent units that can proceed in parallel, or the parent's context limit |

A delegated context that must name a run is handed this run's id verbatim, never one composed for it. No turn ends waiting on a background delegation: a headless session takes no further turn, so wait for it in the foreground or do without its answer. Past about 150k tokens of parent context, hand the next unit to a fresh worker. Pass a worker the spec verbatim, by reference, never paraphrased. A worker's check of its own work gets one correction batch for all of its blockers, a recheck limited to those blockers, and then it stops and asks.

The writing row's reasons are **stated, not adjudicated**: nothing refuses a write that ignores them. The gate only names, at `SessionStart`, a target this run's own context wrote after a delegated one had run this session.

Tests, builds and native review actions may each use a fresh per-action worker; child workers gain no orchestration authority.

Crossing a threshold selects delegated work only. Size, file count, and risk do not select a protocol.

## Decision Gates

| Situation | Action |
|---|---|
| A pull request this run did not build, comments to address, an `assisted` repository, no tracker or not a Git repository, a failed tracker, routing or renewal read, a renewal answering "stop", an abandoned holder, or work to put down | Read the matching row in [decision gates](references/decision-gates.md). A failed read is never clearance. |
| Invoked with no role word | The combined analyst-developer: load both rule books (the analyst's is [analyst](references/analyst.md)) and route to which issue, not which role, in order: this run's held issue -> resume at its recorded state; else an unassigned `review`, then an unassigned `ready`, then an `analysis` item to specify, and triage only when every queue is empty. Never route from a label, assignee, prose, or observed worktree: claim rules refuse them. |
| A domain route is recorded | Analyst loads the left rule book; dev loads the right; the combined analyst-developer loads both, each bar governing its own half. State explicitly when implementation routing is absent. |
| Runtime mechanics or independent-context acquisition vary | Follow [runtime notes](references/runtime-notes.md); limits change the mechanism, never the invariant. |
| State must be chosen | Invalid specification -> `analysis`; implementable and unassigned -> `ready`; claimed build -> `in-progress`; built work awaiting delivery evidence or ordinary delivery permission -> `review`; an unbuilt external wait, or a human adjudication that is not a routine wait on the ordinary review, CI, identity and delivery-permission gates -> `blocked` with the exact condition or decision and its discharger; merged and verified -> `done`, and so does an issue whose answer required no change, with no publication. |

## Execution Steps

### Developer

#### FLOW

FLOW keeps the claimed issue as the record and the publication as the candidate.

1. **Understand.** Read-only work stays read-only. Explore in proportion; one product question,
   one premise challenge.
2. **Route.** Execute directly when the work is understood; plan briefly only for real uncertainty;
   delegate only for a reason the table above names.
3. **Execute.** Bounded changes, Conventional Commits, test-first wherever a deterministic test
   applies — the default, not a flag.
4. **Verify.** Deterministic evidence, then the review step 5 acquires.
5. **Deliver.** Step 6's delivery and close. A body or commit says `Refs #<n>`, never a closing
   keyword; the merge is the bare `gh pr merge <pr> --merge --match-head-commit <head-sha>`, alone on
   its line, from the primary checkout.

A checklist exists only when the work spans publications or sessions, divides into units with their own acceptance, or must survive an interruption. Record it with the `task` operation on the claimed issue and `compact: true`, never as a `comment` and never as a file in the checkout: a stable id, the acceptance in its text, the route. An implementation row closes with its commit, any other row with `evidence`; `tasks` is the only source of truth.

1. Resume the held issue first (Hard Rules); otherwise drain unassigned `review` before `ready`, rechecking the delivery blocker before claiming published work. The review queue is requester-aware; a failed candidate-timeline read is unreadable, never eligible. A `blocked` item whose every recorded clause already holds is discharged before the queue reads as empty: move it to the state its evidence supports, then claim it.
2. For `ready`, partition candidates by stable domain and scale, rank within each partition, and take the oldest head; a selected domain limits selection to its highest-ranked oldest item. Use oldest-first only when no candidate has a valid scale contract.
3. `claim` with the run-id and horizon; the store adjudicates and the earliest live claim wins. Pass `transition_to`, `branch` and `base` for a ready item; keep claimed review work in `review` unless a code change needs a fresh cycle. Move a task between states on your own only when `Transition authorisation` allows it; otherwise ask first.
4. Load the route's right-hand implementation rule book; if absent, say so and build to the acceptance criteria. Follow repository rules and the repository-delivery fallback, and implement in the isolated checkout.
5. **Renew before publishing.** Publish or reuse the shared PR as a confirmed draft, record epoch, PR, head, base and clean-target digest, then transition to `review`. Read the lane before acquiring reviewers. Where the base declares checks, `collect_review_input` with `checks` runs them, and no lens launches while the receipt is red or running. Follow `review_status`'s `next` until it stops — `collect_review_input` per slot, with `launch: true` to start a reviewer, `deliver` for the verdict — calling `escalate_review` when a spent correction leaves the decision to a person, `handoff_review` when this run cannot obtain the required distinct context. [Blind judges](policies/blind-judges.md) owns settlement. **Commit to the disposition before reading anything the panel returns.**
6. `deliver` runs step 5's tail through the close; follow its `next`. By hand: renew at each irreversible boundary; merge only when reviewed, green, at the reviewed head, on its base or a cleanly advanced one, and only when `Delivery authorisation` allows it — otherwise ask first, naming head and base. Verify the delivered SHA's topology as [github delivery](bindings/github/delivery.md) states, then close from `review` with one `release` carrying the evidence file and `spend`. Release is last and not optional; a run releasing its own issue names no `target_operation`. **A delivery that skips release is reported, not refused.**
7. Steps 5 and 6 apply the Decision Gates table and invent no third wait. Record the exact blocker, or the decision and its discharger, before release.
8. A defect, risk or improvement outside this issue's scope is handed to an analyst run and never written into this change. That run is this one's own analyst half, filing with `create` under the [analyst contract](references/analyst.md), including its bound of at most one evidenced issue or none, or a separate session that holds its own claim; a subagent of this run is not a separate run, so it reads and this run files. Hand over what is held: the reproduction, the file and line, the command and its output; or say plainly that it could not be reduced to a reproduction, rather than filing an impression. A delegated `analyst` worker definition, where the host installs one, can carry the reading, but it is repository-read-only and files nothing. Do not block delivery on the answer; record the handoff and its outcome in the close evidence when the answer has come back, or say the handoff is still open, so a later run can see that this one looked and what came of it.

## Output Contract

- Developer: return the issue and state, holder, branch/worktree, review URL, head/base/delivery identities, tests, review, CI, publications, any blocker or next owner; never reinterpret an unknown result as clearance.

## References

Load a reference when its condition holds; each owns its subject once.

| Load | When |
|---|---|
| [domain composition](references/domain-composition.md) | A finding is routed, a priority scale validated, or a queue partitioned. |
| [repository delivery](references/repository-delivery.md) | The repository leaves isolation, integration, review sizing, base movement or cleanup unsettled, or step 5 sends a run to its lane and force-push rules. |
| [analyst](references/analyst.md) | The role is analyst, or the run is the combined analyst-developer. |
| [decision gates](references/decision-gates.md) | A Decision Gates row above names it. |
| [safety incidents](references/safety-incidents.md) | Claim, reclaim, handoff or failure rationale, or an incident's evidence. |
| [github recovery](bindings/github/recovery.md) | A lock, a migration, or a quiet failure fired. |
| [github setup](bindings/github/setup.md) | `estigia setup` or `sync` is running. |
| [github delivery](bindings/github/delivery.md) | `deliver` answered `stopped`, step 6's topology check applies, or a version tag is due. |
| [github operations](bindings/github/operations.md) and its [guarantees](bindings/github/operations-guarantees.md) | An operation's full guarantee, or a refusal only it raises, is needed. |
| [github refusals](bindings/github/refusals.md) | A refusal is not self-explanatory. |
| [linear binding](bindings/linear.md) | The `Tracker` row names `linear`. |
| [runtime notes](references/runtime-notes.md) | Invocation, delegation, read-only enforcement, install mechanics, or runtime limits. |
| [blind judges](policies/blind-judges.md) | `review_status` stops as escalated or a correction is owed. |
| [judgment day](policies/judgment-day.md) | The operator asks for the adversarial panel. |
| [receipt-driven review](protocols/rdd.md) | The base moved under a review. |

<!-- issue-flow:config:start -->
---

## Operator configuration

`estigia.local.md` beside this file overrides same-named rows, holds permissions and machine paths,
and is ignored by Git: it may tighten only the two authorisations and MUST NOT be committed or
published. `estigia config list` prints every row in force.

| Setting | Value here | Skill default |
|---|---|---|
| Authorisation posture | balanced | balanced |
| Delivery authorisation | auto | auto |
| Worktree location | unset | unset |
| Tracker | `github` | `github` |
| Repository host | `github` | `github` |
| Summary language | English | English |

<!-- issue-flow:config:end -->

`Authorisation posture` sets both authorisations at once: `balanced` makes them `auto` and `cautious`
`ask`, and an explicit authorisation row after it wins.
