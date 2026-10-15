# Analyst issue body template

Fill this contract without changing its heading order, fixed first heading, GitHub NOTE syntax, or attribution.

## Which language each part is written in

Two rows of the operator configuration, and they are separate on purpose. The summary at the top
exists to be read by somebody who will never read past it — often not the person implementing the
thing — and the body under it is for whoever does. A repository whose team reads Spanish and whose
code, commits and logs are in English wants exactly that pair, and a single row cannot express it.

| Part of the issue | Row that governs it |
|---|---|
| The `## Description for dumb humans` sentence | `Summary language` |
| Every section from `## Problem` down, and the title | `Issue body language` |

**Headings are never translated.** They are the contract's own vocabulary: `## Problem` is
`## Problem` in every installation. What the rows govern is the prose
written under them.

```markdown
## Description for dumb humans

> [!NOTE]
> One or two sentences, plain language, no jargon, no file paths, no metrics — what this issue is
> about, for someone who will never read past this line. Written in the language the operator
> configuration's `Summary language` row names (see the table at the end of that file). The header
> itself is the fixed, literal title above — always in English, always that exact phrase, never
> reworded per issue. `> [!NOTE]` is GitHub's alert syntax: it renders as a bordered, coloured
> callout, hard to miss in an issue full of technical prose.

This is the first thing on the issue, before `## Problem`.

Everything from here down is written in the language the operator configuration's
`Issue body language` row names — which is not necessarily the one above it.

## Problem
What is wrong or missing, and how you know. Evidence, not assertion: measured numbers, logs,
reproduction steps. Cite a file with the symbol inside it, never a line number.

### Today
What happens now, and the command or test that shows it.

### Expected
What should happen instead.

## Why it matters
Impact if left alone. If you cannot state one, say so plainly — a documented
"low impact, filed for completeness" is honest and lets the dev deprioritise.

## Proposed approach
The design. Alternatives you considered and why you rejected them.
Ordered work is not here: it comes later, from the run that holds the issue.
If unsure, name the decision and its discharger, not an open question. Work that waits on a
person's decision is filed `blocked`, not `ready`.

### Pitfalls
What looks right and is not: a check that passes without measuring, a fixture that must be built
first.

### Size
Expected changed lines against `Change size`, and where the work would split.

## Where to start
Each file to read or change, with the symbol inside it and one line on why; the nearest existing
test to copy. Locations, not ordered steps: ordered work stays the claim holder's `task` checklist.

## Acceptance criteria
Checkable statements. "Faster" is not a criterion; "p95 under 200 ms measured
by X" is. The first criterion is the test that goes red against today's code. Include what must NOT
change (invariants, byte-identical outputs).

### Rules that apply
Repository or domain rules this change must keep, named by file and section, not restated.

## How to verify
The exact commands, including any build or tool they need first.

## Out of scope
What a dev should explicitly not do here. This prevents scope creep more
reliably than any amount of prose in the sections above.

## Evidence
Commands run, files read, numbers measured, and the commit they were read at. Enough that a
reviewer can re-check the analysis without repeating it.

— Analysed by <run-id> on <date>
```
