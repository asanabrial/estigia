# Safety incidents and failure cases

This ledger owns the historical evidence behind Issue Flow's safety rules. IDs are stable: keep each
incident here exactly once, preserve its dates and issue links, and change the protection only when
the surviving executable or document owner changes.

## Omitted mechanics and stale projections

| ID | Failure mode and evidence | Protection or invariant | Surviving executable or document owner |
|---|---|---|---|
| I01 | Correct prose was repeatedly remembered incompletely: five claimed issues were never relabeled, and heartbeat loops wrote without reading. | Put reversible, mechanical operations and their readbacks in the selected binding executable; leave judgement in prose. | Estigia's own tools; each selected binding's operations table |
| I04 | A correctly assigned and transitioned issue received its `dev:<runtime>` marker only at close, so live-holder queries could not find it during the build. | Project the bounded runtime marker when work becomes held; do not add a second claim event. | `SKILL.md` dev step 3; binding claim/transition operations |
| I06 | Five runs died after claim and before transition, leaving assigned issues labeled `ready` after their horizons expired. | Treat claim and state as separate evidence, reclaim from any state, and transition to the state supported by retained work. | Claim recovery below; binding claim/reclaim/transition operations |
| I11 | On 2026-07-25, issues #61 and #62 traversed labels through delivery while the project cards they belonged to stayed on `Ready`; the projection ran zero times. | A transition includes every projection and its readback; both surfaces must agree before success. | Binding transition executable; `SKILL.md` state contract |
| X02 | A tracker that rejects unknown labels can fail only after the analyst has completed all judgement and attempts the first create. | Provision the state vocabulary and supplied marker labels before attaching them; make setup idempotent. | Binding ensure/create operations; `SKILL.md` Hard Rules and Execution Steps |

## Claim ownership and abandoned runs

| ID | Failure mode and evidence | Protection or invariant | Surviving executable or document owner |
|---|---|---|---|
| I02 | On 2026-07-26, a five-minute dev loop repeatedly re-entered selection while already holding one issue and nearly started additional work on every tick. | Resume self-held work before consulting any queue; one run holds one task. | `SKILL.md` dev step 1 |
| I03 | On 2026-07-24, issue #58 was claimed by two runs 1m46s apart; the loser did not renew before filesystem work and wrote a model, migration, and tests into the winner's checkout. | Verify the live claim immediately before branch, worktree, or file creation; stop if the store says another run won. | Binding verify-claim and start-branch operations; `SKILL.md` dev step 4 |
| I07 | On 2026-07-22, a run lost a claim race by five seconds, was told 33 seconds later, then posted three heartbeats and worked about 48 more minutes without rereading control messages. | Every heartbeat reads before it writes, and the same renewal precedes expensive or irreversible boundaries. | Renewal and reclaim below; binding heartbeat/verify-claim operations |
| I08 | Agent-team tasks have remained incomplete and blocked dependants after their session disappeared, requiring manual status repair. | Treat in-session task lists as ephemeral; persist progress on the issue and recover abandoned work from server-timestamped evidence. | Renewal and reclaim below; `references/runtime-notes.md` |

### Renewal and reclaim procedure

1. Claim with a declared horizon and persist progress through the binding's `heartbeat`
   operation, which records `progress` and `blocked` activity as readily as a heartbeat. Only an
   activity marker the holder authored renews ownership — `heartbeat`, `progress`, `blocked`,
   `branch` or `published`; a note, diagnosis, mention, unrelated update, or control target does
   not.
2. Every heartbeat runs `verify_claim` before writing. Run the same renewal before filesystem work,
   evaluation, review, merge, publication, and close. A failed read answered nothing: write nothing
   and retry the read. A successful stop answer means stop, acknowledge once when the binding asks,
   release only this runtime's projection, and leave state owned by whoever moved it.
3. A holder is reclaimable from any state when its attributed activity has passed its declared
   horizon plus the bounded renewal window, or the binding's legacy window when no horizon exists.
   Use the binding's reclaim operation and exact target discovery; never infer authority from labels,
   assignee, prose, or a locally observed worktree alone.
4. Record who was displaced and the server-timestamped evidence before repository work. Replace the
   dead runtime projection, retain every branch, diagnosis, ruled-out hypothesis, and partial result,
   then move to the state supported by that evidence rather than restarting from the displayed label.
   A live prior holder will receive the takeover through its next renewal and must stand down.
5. A run ending deliberately uses `unassign`, persists its state and evidence, and needs no reclaim.

## Repository isolation and immutable evidence

| ID | Failure mode and evidence | Protection or invariant | Surviving executable or document owner |
|---|---|---|---|
| I05 | On 2026-07-24, issue #58 runs derived the same worktree directory; files appeared in a checkout its owner had created clean minutes earlier. | Use one issue branch and an external checkout whose construction is unique per run — registration of a common branch is not isolation and is not ownership. Resume only on durable evidence that this run created the checkout; a foreign path, an orphan and an unproven claim all stop, and an unreadable claim is a failed read rather than a decision. | `references/repository-delivery.md`; binding start-branch operation |
| X06 | Fresh worktrees omit ignored credentials, environment files, and local settings, so a familiar tool can start normally and fail only when an absent value is needed. | Restore required ignored inputs and run the worktree's code while reusing the established environment when safe. | `references/repository-delivery.md`; selected binding's worktree guidance |
| X07 | Rebasing rewrites SHAs already named by reviews, CI, signatures, or deployments and can require a force-push over refs another actor fetched. | Integrate the current base without rewriting published evidence unless repository policy explicitly requires otherwise; reverify after integration. | `references/repository-delivery.md`; `SKILL.md` delivery steps |

## Runtime and routing mismatches

| ID | Failure mode and evidence | Protection or invariant | Surviving executable or document owner |
|---|---|---|---|
| X01 | Codex rejects the `/` skill sigil because it resolves only built-in commands there. | Use the runtime's documented invocation sigil without changing role or argument semantics. | `references/runtime-notes.md` |
| X03 | Familiar-looking values from different priority scales invite a fake global ordering, such as comparing `tier:2` with `priority:high`. | Validate each declared scale and rank only within its domain/scale partition. | `references/domain-composition.md` |
| X04 | Loading the analysis side of a routing arrow for implementation applies evidence and filing rules as though they defined done. | Analysts load the left rule book; devs load the right, and an absent implementation side is stated rather than guessed. | `references/domain-composition.md` |
| X05 | A shell can bypass a read-only tool allowlist, and teammate runtimes may not inherit a skill declaration even when they inherit tool restrictions. | Keep the prose write boundary authoritative and place required rule books explicitly in delegated prompts. | `references/runtime-notes.md` |
| X10 | A parallel A/B evaluation exhausted memory while another run measured on the same machine; the sequential rerun passed. | Announce heavy phases, inspect concurrent work, and rerun alone before treating a contention death as evidence about the change. | `references/runtime-notes.md` |

## Handoff, completion, and blockers

| ID | Failure mode and evidence | Protection or invariant | Surviving executable or document owner |
|---|---|---|---|
| I09 | Recoverable work was buried by choosing state from the obstacle rather than the evidence: external outages went to analysis, incoherent specifications to blocked, and built work could be hidden as unbuilt. | Choose the state from the evidence the work already carries, never from the obstacle in front of it. Where the wait is a human adjudication that is not a routine wait on an ordinary delivery gate, name its exact exit condition and its discharger, and keep the built evidence intact. Which state each case takes is the Decision Gates table's to say, and this row deliberately does not restate it. | Handoff procedure below; `SKILL.md` decision gates |
| I10 | During the session that produced the handoff threshold, a long-running agent repeated its already-documented `$?`-after-a-pipe mistake while debugging. | Hand off after repeating a known mistake, three non-narrowing hypotheses, or more editing than measuring; preserve the diagnosis on the issue. | Handoff procedure below |
| X08 | An issue without acceptance criteria forces the developer to invent the bar it later claims to have met, and a partial improvement can then masquerade as completion. | Write criteria before implementation or return the issue to analysis; never ship a partial fix as complete. | Handoff procedure below; `SKILL.md` dev steps; `references/domain-composition.md` |
| X09 | A blocked item has no natural holder or selection queue; vague conditions and unnamed human decisions therefore become abandoned work. | Name the exact exit condition and its discharger, surface overdue human decisions, and return discharged blockers to the state their preserved evidence supports. | `SKILL.md` Analyst steps and Decision Gates |
| I12 | In another repo's #92 / PR #167, the publishing run could not create an independent review context, retained ownership in `review`, and therefore remained both the only eligible worker and the one worker forbidden to produce the verdict. | Persist an exact-receipt review handoff, release the exact ownership epoch, exclude its publisher/requesters, and let a distinct run record the computed verdict before the publisher resumes; rejection permits repair, while only a computed `approved` releases delivery. | GitHub binding `handoff_review`, `review_computed_verdict`, requester-aware `list_state`; `SKILL.md` dev step 5 |

### Handoff and incomplete-work procedure

- Choose from evidence, not frustration: the destination is the one the Decision Gates table in `SKILL.md` names. Never silently reinterpret the issue.
- Persist ruled-out hypotheses, confirmed evidence, methodological errors, branch/worktree location,
  and the next action on the issue. Preserve an out-of-scope discovery as a separate finding. Chat
  and in-session task state are not durable handoff stores.
- Hand off when a known mistake repeats, three hypotheses fail without narrowing, or editing exceeds
  measurement. A fresh run with a retained diagnosis is safer than a tired run continuing blindly.
- Acceptance criteria exist before implementation. If absent, state a narrow defensible contract on
  the issue or return it to `analysis`; never reconstruct criteria around completed bytes, and never
  ship a partial fix as complete.

## History moved out of the GitHub binding

`bindings/github.md` is loaded on every activation and carries the instructions; the history that
justified them is here.

- **The instruction was present and the run never executed it.** *"not a missing permission, not an
  unclear config, not a tracker limitation. The instruction was present and the run simply never
  executed it."* A run moved labels correctly through the entire state machine and executed a mechanical
  step zero times in a whole session. That is why the mechanical steps are tools.
- **No interpreter on the path.** A script once shipped beside the contract, which was a second
  implementation of every decision: one the agent ran and one the gate read, alive at the same time and
  able to disagree about which comments name a claim, when a takeover's evidence is bound, and whether a
  branch is linked. The operations answer in one place now.
- **Exit codes.** Exit `3` is the one to read carefully: treating a timeout as a stand-down lets a flaky
  network halt every run, and treating it as clearance lets a run write deaf, the defect the renewal
  exists to close. `5` exists because `3`'s advice ("nothing was learned, retry") is wrong for a write,
  which may have landed in the instant before the failure surfaced. `4` exists because an unhandled
  exception would otherwise exit `1` with a traceback and no JSON, indistinguishable from a deliberate
  STOP. `2` is about AUTHORITY: a malformed worktree template, a run ID that cannot name a directory or a
  missing `Worktree location` change no authority, so a retry after fixing the configuration is the right
  move, and reporting one as `1` costs a run its work in the direction that matters.
- **Reading old markers.** Reading once fell back to the prose forms (`Claimed by <run-id>`) for comments
  written before markers existed, deliberately narrow: a control message must both name the run-id and
  instruct. A comment an older binary posted is read through the import under the same rules and, if
  edited, becomes inert because GitHub no longer exposes its original bytes; corrections append a fresh
  operation.
- **A real claim race, 2026-07-22.** Claims were then recorded as comments. Today the store orders both
  claims in one transaction and refuses the loser at once; the lesson about renewal stands.

  | Time (UTC) | Event |
  |---|---|
  | 14:40:03 | `claude-code-cb8d3f2c` writes its claim comment |
  | 14:40:08 | `claude-code-d7d8a22e` writes its claim comment, 5 s later, before the collision is visible |
  | 14:40:41 | adjudication comment: the earliest claim wins, `claude-code-d7d8a22e` is told to stand down |
  | 14:49-15:24 | `claude-code-d7d8a22e` posts heartbeats 1, 2 and 3 and keeps building; none of its writes re-read the timeline |
  | 15:24:21 | the winner delivers |
  | 15:28:52 | `claude-code-d7d8a22e` finally stands down, retracting a measurement taken on work it no longer held |

  Under write-only heartbeats the stand-down sat unread for 48 minutes; `heartbeat` runs the renewal
  before it records, so the loser's next heartbeat at 14:49:22 would have exited `1` instead of writing.
- **Markdown bodies.** The tools remove the class of inline-body corruption rather than warning about it,
  and answer it in one implementation rather than a PowerShell/bash pair: writing the fix in the language
  that causes the bug, twice, would drift. It was once a script beside the contract, one implementation in
  the wrong place.
- **Project columns.** The label half of a transition was always verified; the project half is the one with
  no other feedback loop, and a column nobody looks at stays wrong forever. Real projects do not label the
  last column consistently: one described `Analysis` to `Blocked` with their exact `status:*` labels and
  then described `Done` as `closed`, because that column also tracks the tracker's own closed flag. A
  verifier demanding `status:done` there fails on the one transition that matters most.
- **Installing `gh`.** `gh` is a separate install (`winget install GitHub.cli`, `brew install gh`,
  `apt install gh`). An analyst reaching the API over HTTPS is as capable as one running `gh`, which is why
  the analyst role was defined as network-only.
