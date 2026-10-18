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

Seen live (2026-07-26): the PR body's first line read
`Refs #N — a plain reference, deliberately NOT a closing keyword`, no commit message on the branch
carried a keyword either, and `closingIssuesReferences` still returned `[N]`. GitHub converts the
Development-sidebar link into a closing reference the moment a PR opens from that branch, and
empties `linkedBranches` in the same move. A check that blamed the prose sent the run to edit text
that never contained the offence.

**This binding accepts the branch-link close deliberately.** The native link is the most durable
join between an issue and its code, and `gh issue develop` is recommended here precisely because it
creates that link. Treating its consequence as a violation would make the recommended path
permanently un-shippable, and a gate that fires on every correct delivery is a gate that gets
ignored — at which point it also stops catching the keyword, which is the avoidable case that
actually matters.

**A slice that is not the last of its chain opts out of the link.** The accepted close is wrong for the
first slices of a chain delivered as several pull requests: the first merge would close an issue that
still has slices left. `start_branch` takes `no_link: true` (`--no-link` on the command line), which
skips the develop call and so the Development-sidebar link. The branch is still reserved locally and
the answer says so honestly: `natively_linked: false` and `link_outcome: skipped`, with no `link_queued`
even while the schedule is closed, because nothing is deferred. The branch reaches the remote with the
first push, as a hand-created one does. With no link and no closing keyword GitHub reports no closing
reference, so `check_closing_keywords` answers `will_autoclose: false` and the merge leaves the issue
open; the last slice starts linked as before and closes it as described above. Nothing downstream
assumed the link: `verify_claim`'s own-delivery close and the post-merge continuation are reached only
when GitHub closed the issue. The opt-out does not remove a link that already exists, which is why a
branch started earlier without it still takes the reopen path in `skill/bindings/github/delivery.md`
*Closing*. Which slice is last is the operator's to know: nothing in the transport records a chain.

## Why `done` verifies the merge

`done` means merged and verified, and the verification used to be the operator remembering to read the merge
commit's parents. `transition --to done` now reads them: with a confirmed review receipt on the issue it takes the
receipt's pull request and requires a merge commit whose second parent is the reviewed head and whose first parent
descends from the reviewed base — the proof `verify_claim` already uses for a closed issue (`delivery_close`),
without its condition that GitHub counts the pull request as a closer. That condition is left out on purpose: a pull
request that only says `Refs #<n>` is delivered all the same, and refusing it would make the recommended wording
un-shippable.

A refusal is `delivery-not-verified`, with a `cause` (`not-merged`, `second-parent-is-not-the-reviewed-head`,
`reviewed-base-is-not-behind-the-first-parent`, and `merge-commit-unreadable` for a merge sha that is not 40 hex) and the merge commit and parents it read. A squash or rebase merge is
refused earlier, as `delivery-topology-mismatch`. An issue with no confirmed receipt moves as before, and so does every
state other than `done`. The check reads GitHub, so it also runs when the tracker is closed to writes: a merge it
cannot read refuses the `done` rather than recording it unverified. The same check serves the CLI and the
`transition` tool, because both reach one transport operation.
