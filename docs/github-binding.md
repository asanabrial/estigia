# Why the GitHub binding is shaped the way it is

The incidents and arguments behind several rules in
[`skill/bindings/github.md`](../skill/bindings/github.md), moved here so a run loads the
rules without them. Every rule they explain stays in the binding; nothing here is a rule of
its own.

## The claim hazard

The obvious reading of GitHub is that assignees are a set, so two agents claiming leaves two
assignees and the collision is visible. **That is true only when the agents are different accounts,
and they usually are not.** Agents authenticate as one shared account, so `--add-assignee @me` twice
leaves exactly one assignee, and the re-read shows a clean issue assigned to you — while another run
is already building it.

The lesson generalises further than this one binding: **an identity field cannot adjudicate a race
between runs that share that identity.**

## Why markdown bodies always go through a file

Inline `--body`/`--comment` corrupts markdown on a PowerShell runtime. Seen live: three separate
comments, from two different runtimes, posted with the backtick stripped or eaten entirely —
`` `floor_starvation` `` arrived as `\loor_starvation\` (the backtick-plus-`f` was consumed as
PowerShell's form-feed escape, taking the `f` with it), `` `status:blocked` `` arrived as
`\status:blocked\`, and every intended line break arrived as the two literal characters `\n`.
Backtick is PowerShell's escape character; a double-quoted `--body "<text with `code spans`\nand
newlines>"` gets expanded by the shell BEFORE `gh` ever sees it, and no amount of care in the text
itself prevents that — the corruption happens one layer below where the text is composed.

## Auto-close, and why it does not end the work

Seen live (2026-07-26, issue #118 / PR #119): the PR body's first line read
`Refs #118 — a plain reference, deliberately NOT a closing keyword`, no commit message on the branch
carried a keyword either, and `closingIssuesReferences` still returned `[118]`. GitHub converts the
Development-sidebar link into a closing reference the moment a PR opens from that branch, and
empties `linkedBranches` in the same move. A check that blamed the prose sent the run to edit text
that never contained the offence.

**This binding accepts the branch-link close deliberately.** The native link is the most durable
join between an issue and its code, and `gh issue develop` is recommended here precisely because it
creates that link. Treating its consequence as a violation would make the recommended path
permanently un-shippable, and a gate that fires on every correct delivery is a gate that gets
ignored — at which point it also stops catching the keyword, which is the avoidable case that
actually matters.
