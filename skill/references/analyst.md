# Analyst

The analyst half of the contract in [`SKILL.md`](../SKILL.md): its execution steps, the two
Decision Gates rows that scope its analysis, and what it returns. The Hard Rules in `SKILL.md`
bind it, the read-only rule among them, and this file adds to them rather than replacing them.

## Decision Gates

| Situation | Action |
|---|---|
| Analyst has conditions or a bounded diff/PR/issue | Analyse only that scope under applicable repository and domain rules. |
| Analyst has no conditions or domain | Use the autonomous `general` contract in [domain composition](domain-composition.md); file only the strongest deduplicated finding or nothing. |

## Execution Steps

1. Keep the repository and non-tracker external systems read-only, with the bounded `measuring` exception in the Hard Rules of `SKILL.md`. Drain `analysis` oldest first, then inspect `blocked`: move a discharged condition to the state its preserved evidence supports — built work cleared to continue delivery returns to `review`, while work requiring implementation returns to `ready`; date conditions that still hold, and name how long a person has owed any pending decision. A return to `review` does not record a verdict. The resuming run re-enters the Developer step 5 of `SKILL.md`, which re-derives the tier and re-acquires what it licenses against the exact latest receipt; a person's decision is not a verdict and must not be recorded as one.
2. Analyse only the bounded scope under the left-hand domain rule book. With no conditions or domain, use the autonomous `general` contract; verify evidence and duplicates — `list_state` with `identities` answers the `identity` each open issue recorded when it was filed, so a candidate is checked against the key rather than the title — then keep at most the strongest defensible finding or report that none survived.
3. Validate the fields, route, scale, and body contract for queued and new findings. For queued work, add missing handoff material with `comment` and transition only that same complete item; never create a replacement. For a new finding, fill the [analyst issue template](../assets/analyst-issue-template.md) and `create` it with its identity, `analyst:<runtime>` and run-id attribution; `repo` (`owner/name`) files it in another repository, unclaimed. Use `ready`, or `blocked` with the exact missing condition and discharger; then STOP without implementing or assigning it. A condition a predicate can settle is also recorded where a reader counts it — one clause per `blocked_until` call naming `until_issue` and `until_state` (or in `create`'s `blocked_until`, spelled `<issue>=<state>` and joined by `+`), which holds when the local store puts that issue in that state and is kept only in the store, never in the issue body — so a queue is not read as empty while work waits one transition away. A condition no predicate settles, which is every human adjudication, records no clause and is never counted as discharged.

## Output Contract

Return the created issue, state, identity, priority, domain route, and attribution, or state that no issue was filed and name the evidence checked; return no repository artifacts.
