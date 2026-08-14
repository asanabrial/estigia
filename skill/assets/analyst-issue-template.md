# Analyst issue body template

Fill this contract without changing its heading order, fixed first heading, GitHub NOTE syntax, attribution, or machine marker.

## Which language each part is written in

Two rows of the operator configuration, and they are separate on purpose. The summary at the top
exists to be read by somebody who will never read past it — often not the person implementing the
thing — and the body under it is for whoever does. A repository whose team reads Spanish and whose
code, commits and logs are in English wants exactly that pair, and a single row cannot express it.

| Part of the issue | Row that governs it |
|---|---|
| The `## Description for dumb humans` sentence | `Summary language` |
| Every section from `## Problem` down, and the title | `Issue body language` |

**Headings are never translated.** They are the contract's own vocabulary and the machine marker's
neighbours: `## Problem` is `## Problem` in every installation. What the rows govern is the prose
written under them.

```markdown
## Description for dumb humans

> [!NOTE]
> One or two sentences, plain language, no jargon, no file paths, no metrics — what this issue is
> about, for someone who will never read past this line. Written in the language the operator
> configuration's `Summary language` row names (see the table at the end of that file). The header
> itself is the fixed, literal title above — always in English, always that exact phrase, never
> reworded per issue. `> [!NOTE]` is GitHub's native alert syntax — it renders as a bordered,
> coloured callout, which is the point: this has to be visually impossible to miss on an issue
> otherwise full of technical prose.

This is the first thing on the issue, before `## Problem`.

Everything from here down is written in the language the operator configuration's
`Issue body language` row names — which is not necessarily the one above it.

## Problem
What is wrong or missing, and how you know. Evidence, not assertion:
file:line references, measured numbers, logs, reproduction steps.

## Why it matters
Impact if left alone. If you cannot state one, say so plainly — a documented
"low impact, filed for completeness" is honest and lets the dev deprioritise.

## Proposed approach
The design. Alternatives you considered and why you rejected them.
If you are NOT confident, say which part is uncertain and what would settle it.

## Acceptance criteria
Checkable statements. "Faster" is not a criterion; "p95 under 200 ms measured
by X" is. Include what must NOT change (invariants, byte-identical outputs).

## Out of scope
What a dev should explicitly not do here. This prevents scope creep more
reliably than any amount of prose in the sections above.

## Evidence
Commands run, files read, numbers measured. Enough that a reviewer can
re-check the analysis without repeating it.

— Analysed by <run-id> on <date>

<!-- issue-flow: analysis run-id=<run-id> -->
```
