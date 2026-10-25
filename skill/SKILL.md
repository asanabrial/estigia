---
name: flow
description: "Trigger: flow triage [conditions], flow dev [domain-rules] [issue-number], bare flow (the combined analyst-developer, routing to which issue from tracker state), and the long forms analyst and developer. Also claim issue, move task states. Triage files at most one evidenced issue; without conditions, audit the repository critically."
license: GPL-2.0
metadata:
  author: asanabrial
  version: "1.17.0"
---

# Issue Flow — Analyst / Dev over a shared issue tracker

## Activation Contract

Use for `flow triage [conditions]`, `flow dev [domain-rules] [issue-number]`, bare `flow`, analyst or developer work, claims, and workflow-state transitions.
`triage` and `dev` are the roles' short names; `analyst` and `developer` remain accepted and mean the same thing. Naming no role word is the combined analyst-developer, which loads both rule books and does both jobs; a role word, when written, still names one role — `flow dev`/`flow developer` and `flow triage`/`flow analyst` each narrow to that role, and `flow dev <n>` also to that issue — and is never inferred from anything but the words above and the routing row below. `flow <issue-number>` in a repository whose `Claim workflow` is `assisted` is the assisted row in Decision Gates, and takes no role.
Analyst conditions and dev domains are optional: a bounded target is the scope; otherwise the analyst audits the repository and the dev implements the selected issue.
Read repository instructions first and load only the tracker binding selected by operator configuration before tracker work.
Resolve this skill's immutable filesystem directory once per activation and load every local reference from that resolved directory; never reopen companions through the stable installation alias during a concurrent upgrade.
Load the other references only as the References table below says.

## Hard Rules

- Issue Flow owns roles, states, claims, and transitions; routed domains own value, priority, evidence, and done criteria. Neither side invents the other's policy.
- An analyst, including an adversarial reviewer, is read-only for the repository and every external system except the configured tracker operations required below: it creates no branch/worktree/commit/PR and runs no migration, sync, database write, or delivery action. One exception, and it is bounded: a Judgment Day judge on a repository whose `Evidence standard` is `measuring` builds, tests and mutates inside the isolated directory its prompt hands it and nowhere else. That directory MUST be outside the claimed checkout, which is what leaves the repository read-only to it — and it is a rule the orchestrator obeys, not one the gate enforces: a delegated context is measured against its parent's claim, so a Judgment Day judge handed a directory inside that checkout would be allowed rather than refused. It files at most one evidenced issue, save for the filing duty [blind judges](policies/blind-judges.md) places on the run that records the computed verdict — that run files one issue for every `severe` finding it records `pre-existing`, `base-only` or `unknown`, however many that is. Review MUST use a context that did not write the change.
- Mint one `<runtime>-<session-prefix>` run-id and reuse it for every write. Keep bounded runtime attribution in `analyst:<runtime>`/`dev:<runtime>` labels and unbounded run-id attribution in text; analyst labels persist, while dev labels are removed by release, including the release that ends delivery; the run-id in the close evidence is what survives that removal.
- Resume this run's held issue before consulting queues. Use the selected binding's claim, adjudicated in the local store; the earliest live claim wins. **The gate performs the pre-write verification**: a repository write it observes is re-verified against the claim whenever the renewal window has passed, and at every expensive or irreversible boundary, so a separate `verify_claim` before the first write is not required. An unreadable control surface permits no write, and an explicit `verify_claim` remains available when a run wants the answer itself. The one exception is the assisted path in Decision Gates: where the repository's `Claim workflow` is `assisted` no claim exists to verify, and the local claim `assist` records stands in for it.
- Keep exactly one of `analysis`, `ready`, `in-progress`, `review`, `blocked`, or `done`. The binding's workflow state is authoritative; every configured projection is updated and read back, and `done` is explicit before tracker closure.
- A claimed task reaches `done` or a state that names its exact exit condition and discharger; it never enters an ownerless, conditionless queue.
- The selected binding MUST map `ensure_states`, `create`, `list_state`, `claim`, `reclaim`, `verify_claim`, `heartbeat`, `transition`, `comment`, `unassign`, `publish_review`, `republish_review`, `handoff_review`, `review_finding`, `review_computed_verdict`, `review_disposition`, and `release_ci`. Run executable reversible operations instead of reconstructing them; bindings MUST declare unsupported capabilities and fail closed.
- Issue acceptance criteria, routed domain rules, and repository rules define done. Never invent missing gates or reinterpret the issue silently.
- Aim for no more than the `Change size` row of the operator table below, in changed lines per pull request, but treat size as reviewability guidance rather than a gate. A larger coherent change is valid when splitting would reduce safety or review quality; the developer records why before review and the independent reviewer assesses that rationale without relaxing any quality gate. **The candidate is the publication**: a commit is not a review candidate, so no unit of work is committed on its own as a review boundary, and no review is due before one — a verdict binds the epoch, PR, head, base and digest `publish_review` mints, so there is nothing to record one against earlier, and reviewing a unit on its own would mean publishing it on its own. The lever is this row; [repository delivery](references/repository-delivery.md) states the same rule where review-unit sizing lives.

## Direct Work Delegation

Use the smallest useful topology and keep one writer. Delegate only when the expected work crosses
one of these boundaries:

| Work | Stay inline | Delegate |
|---|---|---|
| One batch of reads or lookups | At most 3 calls and about 10k tokens stays inline, even when it prepares a write | More than about 5 sequential lookups delegates one explorer with a ~2k-token handoff |
| Broad research or context compression | - | Delegate broad research or context compression |
| Writing | A small understood change stays inline whatever its file count | Delegate one writer only for a named reason: independent units that can proceed in parallel, or the parent's context limit |

Past about 150k tokens of parent context, hand the next unit to a fresh worker. Pass a worker the spec verbatim, by reference, never paraphrased. A worker's check of its own work gets one correction batch for all of its blockers, a recheck limited to those blockers, and then it stops and asks.

The topology actually used is recorded at publication, not merely required here: `publish_review`/`republish_review` take `--topology` and `--topology-reason` and record them on the marker, and nothing refuses a declaration. See the binding contract for the exact mechanics and what it cannot prove.

The writing row's reasons are **stated, not adjudicated**: nothing refuses a write that ignores them. The gate only names, at `SessionStart`, a target this run's own context wrote after a delegated one had run this session — a floor, not a count.

Tests, builds, installs, and native review actions may each use a fresh per-action worker without changing the route. Child workers do not gain orchestration authority.

Two definitions ship for exactly this: `developer`, which writes and runs the suite inside the
checkout its launch names, and `analyst`, which is repository-read-only. Handing a bounded piece of
work to a fresh context happens under every run. Each is installed only where `Model routing` names
its role, so no definition is there to launch until the routing names it, and the brief is composed
in the prompt until then; naming the role installs it, and the model beside it says only what it
runs on. The row's other value, `by task`, names no role and installs nothing: the shape of the work
decides the model and the effort, and the launcher records what it chose. A delegated context is
measured against **this** run's claim, so what the definition tells
it not to do is a rule the launch keeps rather than one the gate enforces on it separately.

Crossing a threshold selects delegated work only. Size, file count, and risk do not select a protocol.

## Decision Gates

| Situation | Action |
|---|---|
| The repository's `Claim workflow` is `assisted` (a refusal that names the `Claim workflow` row says which mode applies: a tracker write refused under `assisted`, or `assist` refused outside it) | `flow <issue>` is the person naming the issue, and this path is exempt from the Hard Rules on `claim` and `verify_claim`. Call `assist` with that issue and a branch named for it, and work only in the worktree it returns. Claim, transition, comment, heartbeat and record nothing: Estigia writes no tracker here. Run the repository's own checks, publish with `draft_pr` (a title and a body file, no closing keyword), report the draft URL it read back, and stop where the *Delivery* rows say ([runtime notes](references/runtime-notes.md)). Never merge. |
| This checkout has no tracker, or is not inside a Git repository | Say so once and work without a claim for work that lands here. A repository with no git remote, none the configured binding can name an issue in, or a directory outside every repository has nothing to claim against — and demanding one leaves no way forward at all: the claim needs a tracker, the tracker needs a remote, and inventing either is a write nobody asked for. The gate agrees and already says so: a run holding no issue whose write lands outside every repository the gate can place is answered `outside`, so requiring more here would be the contract demanding what the harness does not enforce. A write that lands in a repository is still held, however far the call was launched from: a patch, an absolute path or a shell write into a checkout somebody holds is refused until claimed. Never create a repository, a remote, or a tracker project to satisfy this row. |
| The tracker could not be read, or the repository could not be placed | Not the row above. *There is no tracker*, or *this is not a repository*, is an answer; *the tracker did not answer*, or a directory whose repository the gate could not establish, is not one — treating the second as the first turns every outage into permission. Report the failed read and retry it. |
| Invoked with no role word | The combined analyst-developer: load both rule books, each bar governing its own half and neither relaxed — the analyst's evidence bar for every finding it files, the dev's delivery bar for every change it delivers — and route to which issue, not which role, from the tracker in order, saying which branch answered: this run's held issue -> resume at its recorded state; else an unassigned `review`, then an unassigned `ready`, then an `analysis` item to specify, and triage only when every queue is empty — work in flight first. `flow dev` and `flow triage` (or `developer` and `analyst`) narrow to one role. Never route from a label, assignee, prose, or observed worktree: claim rules refuse them. Text after bare `flow` is triage conditions or dev domain rules. |
| A routing read fails | Nothing was answered, and an empty queue is not what an unreadable one returned. Do not fall through to triage: triage writes to the tracker, so a failed read would become a filed issue justified by a queue nobody saw. Report the failed read and retry it. |
| Analyst has conditions or a bounded diff/PR/issue | Analyse only that scope under applicable repository and domain rules. |
| Analyst has no conditions or domain | Use the autonomous `general` contract in [domain composition](references/domain-composition.md); file only the strongest deduplicated finding or nothing. |
| A domain route is recorded | Analyst loads the left rule book; dev loads the right; the combined analyst-developer loads both, each bar governing its own half and neither relaxed. State explicitly when implementation routing is absent. |
| Runtime mechanics or independent-context acquisition vary | Follow [runtime notes](references/runtime-notes.md); runtime limits change the mechanism, never the invariant. |
| State must be chosen | Invalid specification -> `analysis`; implementable and unassigned -> `ready`; claimed build -> `in-progress`; built work awaiting delivery evidence or ordinary delivery permission -> `review`; an unbuilt external wait, or a human adjudication that is not a routine wait on the ordinary review, CI, identity and delivery-permission gates -> `blocked` with the exact condition or decision and its discharger; merged and verified -> `done`; an issue whose answer required no change also reaches `done`, with no publication, and [repository delivery](references/repository-delivery.md) owns what that close records. |
| A renewal answers "stop" | A control message naming this run, or an item closed or no longer in the expected state. Stop repository work, acknowledge once where the binding asks, release only this runtime's `dev:<runtime>` projection, and change no workflow state — it belongs to whoever moved it. |
| A renewal's read fails | Nothing was answered. Write nothing — no heartbeat, evaluation, review, or delivery — and retry the read. Never a stand-down and never clearance. |
| A holder looks abandoned | Reclaimable from **any** state once attributed activity passes the declared horizon plus the binding's renewal window, or its legacy window when no horizon was declared. Use the binding's reclaim operation with exact target discovery; never infer authority from a label, assignee, prose, or an observed worktree. Record who was displaced and the timestamped evidence, retain their work, and transition to the state that evidence supports rather than the state the label shows. |
| Work must be put down | Repeated a known mistake, three hypotheses without narrowing, or more editing than measuring. Persist diagnosis, ruled-out hypotheses, and branch/worktree location on the issue, then route by the row above. |

## Execution Steps

Before either role's first tracker write in an unfamiliar project, run the selected binding's `ensure_states`; then use that binding's executable operations and readbacks rather than reconstructing them.

### Analyst

1. Keep the repository and non-tracker external systems read-only, with the bounded `measuring` exception in the Hard Rules above. Drain `analysis` oldest first, then inspect `blocked`: move a discharged condition to the state its preserved evidence supports — built work cleared to continue delivery returns to `review`, while work requiring implementation returns to `ready`; date conditions that still hold, and name how long a person has owed any pending decision. A return to `review` does not record a verdict. The resuming run re-enters step 5, which re-derives the tier and re-acquires what it licenses against the exact latest receipt; a person's decision is not a verdict and must not be recorded as one.
2. Analyse only the bounded scope under the left-hand domain rule book. With no conditions or domain, use the autonomous `general` contract; verify evidence and duplicates — `list_state` with `identities` answers the `identity` each open issue recorded when it was filed, so a candidate is checked against the key rather than the title — then keep at most the strongest defensible finding or report that none survived.
3. Validate the fields, route, scale, and body contract for queued and new findings. For queued work, add missing handoff material with `comment` and transition only that same complete item; never create a replacement. For a new finding, fill the [analyst issue template](assets/analyst-issue-template.md) and `create` it with its identity, `analyst:<runtime>` and run-id attribution; `repo` (`owner/name`) files it in another repository, unclaimed. Use `ready`, or `blocked` with the exact missing condition and discharger; then STOP without implementing or assigning it. A condition a predicate can settle is also recorded where a reader counts it — one clause per `blocked_until` call naming `until_issue` and `until_state` (or in `create`'s `blocked_until`, spelled `<issue>=<state>` and joined by `+`), which holds when the local store puts that issue in that state and is kept only in the store, never in the issue body — so a queue is not read as empty while work waits one transition away. A condition no predicate settles, which is every human adjudication, records no clause and is never counted as discharged.

### Developer

#### FLOW

FLOW (Fast, Lean, Outcome-driven Workflow) keeps Estigia's two pillars: the claimed issue is the
record of the work, and the publication is the review candidate. Its capabilities merge or repeat,
and are not calls, states or agents:

1. **Understand.** Read-only work stays read-only. Explore in proportion; at most one product
   question and one premise challenge.
2. **Route.** Execute directly when the work is understood; plan briefly only for real uncertainty;
   delegate only for a reason the table above names.
3. **Execute.** Bounded changes, Conventional Commits, test-first wherever a deterministic test
   applies — the default, not a flag.
4. **Verify.** Deterministic evidence, then the review that *Renew before publishing*, in the
   numbered list below, acquires.
5. **Deliver.** That list's delivery and close. A body or commit says `Refs #<n>`, never a closing keyword; the merge
   is the bare `gh pr merge <pr> --merge --match-head-commit <head-sha>`, alone on its line, from
   the primary checkout.

A checklist exists only when the work spans publications or sessions, divides into units with their
own acceptance, or must survive an interruption. Record it with the `task` operation on the claimed
issue and `compact: true`, never as a `comment` and never as a file in the checkout: a stable id,
the kind and acceptance in its text, the route. An implementation row closes with its commit, any
other row with `evidence`; `tasks` is the only source of truth. A model change alters neither
authority nor independence.

1. Resume this run's held issue at its recorded state before consulting queues. Otherwise drain unassigned `review` before `ready`, rechecking the delivery blocker before claiming published work. The review queue is requester-aware (`list_state` excludes an unresolved handoff's publishing and requesting runs); a failed candidate-timeline read makes it unreadable rather than eligible. An item in `blocked` whose every recorded clause already holds is discharged before the queue reads as empty — the analyst's drain rule, applied here: move it to the state its preserved evidence supports, then select and claim it under this run's own id.
2. For `ready`, partition valid candidates by stable domain and declared scale, rank only within each partition, and choose the oldest partition head; a selected domain limits selection to its highest-ranked oldest item. Use oldest-first only when no candidate has a valid scale contract.
3. `claim` with the run-id and horizon, which the store adjudicates; the earliest live claim wins, and a loser releases and selects again. Pass `transition_to` to move the issue in the same call — `claim` then `transition` as one, which is what every ready item needs — or omit it to claim without moving. Keep claimed review work in `review` unless a code change requires `in-progress` and a fresh publish/review/CI cycle; for ready work, move once to `in-progress` with `dev:<runtime>` and no second claim event. Move a task between states on your own only when `Transition authorisation` authorises this run; otherwise ask first and say which move you are asking for.
4. Load the route's right-hand implementation rule book before planning; if absent, state that none applies and build to written acceptance criteria. Follow repository rules, use the repository-delivery fallback for missing isolation, integration, or review-unit sizing policy, and implement in the isolated checkout; the gate re-verifies the claim against the first branch/worktree/file write it observes.
5. **Renew before publishing.** Publish or reuse the shared PR as a confirmed draft, record its
   epoch, PR, head, base and clean-target digest, then transition to `review`; read the publication
   lane before acquiring reviewers, and use `republish_review` for a rewritten history —
   [repository delivery](references/repository-delivery.md) owns the lane reading, the cooperative
   CI ordering and the force-push rule. Ask for `start_review` on the publication to transition, read
   `review_status` and launch every slot it offers in one call, unless
   the lane is red; `with_review` brings the tier and status alone. Then follow `review_status`'s
   `next` until it stops: `launch_review_input` and `collect_review_input` for every slot it offers
   (`names` batches either; `record_verdict` on the collect records the verdict once
   `review_status` names it); `record_computed_verdict` for the receipt; `release_ci`, its own
   call. Call `escalate_review` when a spent correction leaves the decision
   to a person, and `handoff_review` when this run cannot obtain the required distinct context.
   Load [blind judges](policies/blind-judges.md) only on an escalated stop, a correction, or a disputed
   rule: it owns the settlement order, the refutation batch, the
   targeted validator, the one-correction rule, the classification of findings and the release
   rule, and states what reopens review and what is only a non-blocking finding;
   [runtime notes](references/runtime-notes.md) owns when a launch failed and a handoff is
   owed; [operations and guarantees](bindings/github/operations-guarantees.md) states `review_status`'s answers and the `unavailable` and `stale` classes.
   **Commit to the disposition before reading anything the panel returns**, because read afterwards
   it justifies whatever has already been decided.
6. Renew at each irreversible boundary; merge only when reviewed, green, at the reviewed head, on its base or a cleanly advanced one. Deliver on your own only when `Delivery authorisation` authorises this run; otherwise ask first, with the reviewed head and base named. For a merge-commit route, require the delivered SHA's second parent to be exactly the reviewed head and the reviewed base to be an ancestor of its first parent; other configured strategies require their binding-declared equivalent topology and any delivered-SHA gate. For each changed version, publish and remotely verify its immutable annotated tag and required Release; renew again, put the close evidence (review/CI/tests/measurements/PR/delivering SHA/publications with the run-id) in a file and close with one `release` from `review`: it records the `spend` — one object, or an array with one record for the `orchestrator` phase and one for each delegated phase, against the issue's review transaction — then transitions to `done`, posts the evidence, and releases, in that order. Release is last and not optional: it puts down the isolated checkout and local branch this run created, and removes the pointer, a local projection of a claim that no longer exists. It also deletes every remote head the run recorded starting. `release` never reads whether the issue is open, so one this delivery already closed releases normally. **A delivery that skips it is reported, not refused** — [repository delivery](references/repository-delivery.md) owns the conditions and what `doctor` and `prune` name. A run releasing the issue it holds names no `target_operation`; see *Releasing an issue* in [github operations](bindings/github/operations.md). On handoff, preserve all evidence and choose the destination the Decision Gates table names.

7. The built-work handoff in steps 5 and 6 applies the Decision Gates table. It does not invent a third wait. Record the exact blocker, or the exact decision and its discharger, before releasing ownership.

8. A defect, risk or improvement outside this issue's scope is handed to an analyst run — a separate context holding its own claim — and never filed by this run. The runtime obtains that second context: a subagent, a teammate or a separate session all qualify, as [runtime notes](references/runtime-notes.md) states. Hand over what is held: the reproduction, the file and line, the command and its output; or say plainly that it could not be reduced to a reproduction, rather than filing an impression. That run applies the analyst contract this file states, including its bound of at most one evidenced issue or none, and it is the run that files. A delegated `analyst` worker definition, where the host installs one, can carry the reading, but it is repository-read-only and files nothing. Do not block delivery on the answer; record the handoff and its outcome in the close evidence when the answer has come back, or say the handoff is still open, so a later run can see that this one looked and what came of it.
   A combined run (bare `flow`) is its own analyst for step 8: it files its own findings under the analyst contract and hands off only what lies outside the issue it is writing.

## Output Contract

- Analyst: return the created issue, state, identity, priority, domain route, and attribution, or state that no issue was filed and name the evidence checked; return no repository artifacts.
- Developer: return the issue and exact state, holder, branch/worktree, review URL and immutable head/base/delivery identities, tests/review/CI/publications, and any blocker, handoff, rollback, or next owner. Surface claim, horizon, credential, capability, and resource limits from the focused references; never reinterpret an unknown result as clearance.

## References

Load a reference when its condition holds; do not preload them, and do not copy their contents back
into this file. Each owns its subject once, so a rule that changes is changed in one place.

| Load | When |
|---|---|
| [domain composition](references/domain-composition.md) | A finding is handed over or routed, a priority scale must be validated, a queue is partitioned, or no domain applies and the autonomous `general` contract is needed. |
| [repository delivery](references/repository-delivery.md) | The repository's own rules do not fully define isolation, integration, review-unit sizing, review-target completeness, base-movement classification, or cleanup — the default fallback for repository work. `~/.agents/AGENTS.md`, when it exists, is the canonical operator override and wins over it. |
| [safety incidents](references/safety-incidents.md) | Claim, reclaim, handoff, or failure rationale is needed; a claimed issue turns out to have no acceptance criteria; or an incident's evidence must be cited. It is the ledger of *why*; the rules it justifies are already above. |
| [github recovery](bindings/github/recovery.md) | A lock, a migration, or a quiet failure fired. |
| [github setup](bindings/github/setup.md) | `estigia setup` or `sync` is running. |
| [github delivery](bindings/github/delivery.md) | The dev reaches publication, CI, merge, tags or closing. |
| [github operations](bindings/github/operations.md) | An operation's full guarantee is needed beyond the operations table's headline: claim and its two routes, publishing a review target, recording a finding, a verdict, a refutation or a disposition, or deriving a tier. |
| [github operations guarantees](bindings/github/operations-guarantees.md) | An operation's guarantee beyond its command is needed — what it does past the obvious, and the refusals only it raises. |
| [github refusals](bindings/github/refusals.md) | A github-binding operation refused and the answer's own `action` is not self-explanatory, so the code's meaning and remedy are needed. |
| [linear binding](bindings/linear.md) | The `Tracker` row names `linear`: the issue is read and only a `State names` state written, under assisted work |
| [runtime notes](references/runtime-notes.md) | Invocation sigils, delegation, read-only enforcement, independent-context acquisition, agent-team lifetime, or a runtime's honest limits are in question. |
| [analyst issue template](assets/analyst-issue-template.md) | Filling a new finding's body. It owns the heading order, the fixed first heading, alert syntax, and attribution. |
| [blind judges](policies/blind-judges.md) | `review_status` stops as escalated, a correction is owed, or a rule about how a lens's findings are settled or what reopens review is disputed. |
| [judgment day](policies/judgment-day.md) | The operator explicitly asks for the adversarial panel: the method's rules, its isolation precondition and what a judge's closing statement reports. |
| [receipt-driven review](protocols/rdd.md) | Every review. |

The selected tracker binding under `bindings/` owns exact commands, capability differences,
unsupported behaviour, and tracker-specific readbacks. Its executable owns mechanical operations and
their output contract; state the contract here and call the command rather than re-narrating it.

<!-- issue-flow:config:start -->
---

## Operator configuration

The table below contains portable defaults only. Before using it, look for `estigia.local.md`
beside this file. When present, read it and let its same-named rows override this table; its
additional instructions are local policy too. That file contains permissions and machine-specific
paths, is ignored by Git, and MUST NOT be committed or published.

A repository's own file (`$(git rev-parse --git-common-dir)/estigia/estigia.local.md`) may
tighten only `Delivery authorisation` and `Transition authorisation`; the stricter answer wins.

| Setting | Value here | Skill default |
|---|---|---|
| Authorisation posture | balanced | balanced |
| Delivery authorisation | auto | auto |
| Worktree location | unset | unset |
| Tracker | `github` | `github` |
| Repository host | `github` | `github` |
| Summary language | English | English |

<!-- issue-flow:config:end -->

> `Authorisation posture` sets the two authorisations that can ask, delivery and transitions, at once:
> `balanced` makes both `auto`, and `cautious` makes both `ask`. An explicit authorisation row written
> after it wins, and the model half is separate — `Model routing` stays `unset` unless you write it.

> The two markers delimit the default template used when the installer creates
> `estigia.local.md`. Configure the ignored local file, never this versioned block.
