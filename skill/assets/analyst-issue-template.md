# Analyst issue body template

Fill this contract without changing its heading order, fixed first heading, GitHub NOTE syntax, or attribution.
The implementer has this issue and the repository, and nothing else. Cite only what you opened at the commit under `## Evidence`, and say plainly what you did not check.

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
The design, closed. A decision only a person can take makes the issue `blocked`, not `ready`.

### Decisions taken
Each decision as a closed statement, and each rejected alternative with its reason. Do not reopen them.

### Order
Only where parts really depend on each other or the work splits into pull requests; the claim holder still owns its `task` checklist.

### Pitfalls
What looks right and is not: a check that passes without measuring, a fixture that must be built first, an existing test that pins the old behaviour, code that looks dead and is not.

### Size
Expected changed lines against `Change size`, and where the work would split. A typical body stays under 4,000 bytes; over 6,000 only when it splits into pull requests or carries real risk.

## Where to start
For each file: the symbol, whether it changes or is read only, and what changes; the nearest existing test to copy, with its fixture.

### If this does not match
Stop when a symbol is missing or the code differs: comment what you found and move the issue to `blocked`. Do not improvise a design.

## Acceptance criteria
Checkable statements. "Faster" is not one; "p95 under 200 ms measured by X" is. The first names the new test, its file and rig, its assertion, and why it fails today. Name how each invariant is checked, and the hand-check that turning the fix off reddens the test.

### Rules that apply
Repository or domain rules this change must keep, and the documents it must update, named by file and section, not restated.

## How to verify
The exact commands from the repository's rules, with any build the run needs first.

## Out of scope
What a dev should explicitly not do here. This prevents scope creep more
reliably than any amount of prose in the sections above.

## Evidence
Commands run, files read, numbers measured, and the commit they were read at. State what you did not verify, and warn when that commit is not the base branch. Enough that a reviewer can re-check the analysis without repeating it.

— Analysed by <run-id> on <date>
```
