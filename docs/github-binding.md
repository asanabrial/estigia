# Why the GitHub binding is shaped the way it is

The incidents and arguments behind several rules in
[`skill/bindings/github.md`](../skill/bindings/github.md), moved here so a run loads the
rules without them. Every rule a run acts on stays in the binding; where a section below describes how
the store reads a legacy marker, it says why the binding's rule exists rather than stating a second
rule.

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
still has slices left. `claim` with `branch` and `base` takes `no_link: true` (`--no-link` on the command line), which
skips the develop call and so the Development-sidebar link. The branch is still reserved locally and
the answer says so honestly: `natively_linked: false` and `link_outcome: skipped`, with no `link_queued`
even while the schedule is closed, because nothing is deferred. The branch reaches the remote with the
first push, as a hand-created one does. With no link and no closing keyword GitHub reports no closing
reference, so the publication's autoclose verdict reads `will_autoclose: false` and the merge leaves the issue
open; the last slice starts linked as before and closes it as described above. Nothing downstream
assumed the link: `verify_claim`'s own-delivery close and the post-merge continuation are reached only
when GitHub closed the issue. The opt-out does not remove a link that already exists, which is why a
branch started earlier without it still takes the reopen path in `skill/bindings/github/delivery.md`
*Closing*. Which slice is last is the operator's to know: nothing in the transport records a chain.

A merged call refuses the arguments it would not use. `claim` with `takeover` refuses
`transition_to`, `branch`, `base`, `worktree_root` and `no_link`: a takeover displaces a holder and
starts no checkout, so none of them has a step to reach. Without `takeover`, `no_link` and
`worktree_root` are refused when no `branch` names the checkout they belong to. `release` refuses
`evidence_file` and `spend` when it names no `target_operation` and the run is not the issue's live
holder: a close binds one exact epoch, and a discovery call writes nothing for those arguments to
close.

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

## When SSH to GitHub never connects

Every `git` network call this crate makes names `origin`, so a repository whose `origin` is SSH depends on SSH
alone, while `gh` and HTTPS may be working with a credential the machine already holds. `run_with_env_and_mode`
therefore repeats a `git push`, `fetch`, `ls-remote` or `pull` once over HTTPS when its first attempt failed with
git's `Could not read from remote repository` beside a message that the connection never opened (`NEVER_OPENED` in
`src/transport/ssh_fallback.rs`). Git prints that line only for a failed initial contact, so nothing was sent and
repeating a write is safe, and it translates the line, so these four verbs run under `LC_ALL=C`. A failure after contact, such as a rejected push, runs once. So does a refused host key:
it may be the one failure somebody needs to see.

The repeat uses `-c` options, not configuration: `insteadOf` rewrites `git@github.com:` and `ssh://git@github.com/`,
and `gh auth git-credential` answers for `https://github.com`. `origin`, its refs and the pre-push hook stay as they
are, and nothing is written to the repository. When both attempts fail the answer carries both messages and keeps
its kind; a repeat that outlives its deadline is an ambiguous write, as any other is. What this does not reach is in
[`honesty.md`](honesty-delivery.md#the-https-repeat).

## Why the transport is in this process, and why the server is hand-written

`src/transport/` is the transport, and it is the only one. It began as `skill/scripts/github.py`,
shipped inside the payload because `bindings/github.md` routed every reversible operation through
it — a payload without it was a binding whose first command did not exist. The binding now names
Estigia's own tools, the operations are answered in this process, and the script is deleted: no
`.py` file in the tree, no Python on any path, nothing in CI that installs one.

*No interpreter at all* is what this used to say, and it stopped being true when one test began
**executing** the OpenCode plugin this crate generates rather than reading its source. That plugin is
JavaScript; a gate written in a second language and never run is a copy of a rule nothing crosses. So
`node` is on the verification list in [`AGENTS.md`](../AGENTS.md), both workflows install it rather
than trusting the runner image to, and [`testing.md`](testing.md) says which tests need it.

The MCP server is hand-written rather than built on `rmcp`. The same binary answers a `PreToolUse`
hook on **every edit**, so a `tokio` runtime spun up per process is a cost paid thousands of times to
move a few lines of JSON across a pipe. Keeping the crate synchronous is what keeps the gate cheap.
What that costs is in `UNIMPLEMENTED`: no resources, no prompts, no sampling, no elicitation beyond
`send_message`'s one question, no notifications beyond `initialized`, and no more than one call
that changes the store or the tracker at a time; a read answers while another call waits. A name
the contract requires and no tool performs is declared in `NOT_EXPOSED` with the reason and the
section of `bindings/` that carries it; it is not advertised, so the surface never offers a call
nothing answers, and a seam test holds the two lists together: an operation that is neither exposed
nor declared fails the build. The operations the tools absorbed keep their capability under the
surviving name: `claim` performs `reclaim` through `takeover` (and a live holder's takeover needs the
person-only `estigia hand-over` confirmation), `list_state` answers identities through
`identities`, `review_status` carries the publication lane and the correction count, `review_read`
reads what six tools used to and, with `view: tier` and `parent_head`, narrows a repair's range and,
with `view: target`, classifies base movement; `collect_review_input` starts a reviewer with `launch`
and runs the declared checks with `checks`, and `deliver` records the verdict and releases CI
through `until`.

One crate. Five on day one would split boundaries before any of them had earned the split; a boundary
gets split out when it has earned the seat. The container image is the exception to "one binary on a
developer's machine": the [`Dockerfile`](../Dockerfile) builds one from the pinned toolchain, carrying
the server alone — `estigia mcp` over stdio, with no `git` and no `gh`, so a tool that reaches the
tracker refuses inside it — for a directory that inspects a server by running one in a container.

## What GitHub provides, and what it cannot

The workflow asks a tracker for six things. What a binding must declare is not only which of them
exist, but what happens where they do not — a capability that is absent and undeclared is how a rule
silently stops applying.

| The workflow needs | GitHub | Consequence |
|---|---|---|
| A single-valued **state** per item | ✗ — labels are multi-valued | "exactly one `status:*`" is a **discipline, not a constraint**. Nothing stops two; every transition must remove the old label in the same call |
| A **claim** applied server-side | ⚠️ — assignees are a set, **but agents usually share one account** | then the set has one element however many runs claimed, and re-reading it can never show a collision — see *The claim hazard* |
| **Last activity**, timestamped | ✗ — Estigia keeps its own | only a verified holder liveness record in the local store renews a claim; GitHub's `updatedAt` and comment times renew nothing |
| A stable, short **identity** | ✓ — the issue number | usable verbatim in branch and worktree names |
| **Comments**, append-only and ordered | ✓ | a person's prose lands here; Estigia's own records live in the local store and none is posted |
| A native **priority** | ✗ | encoded as a label `<scale>:<value>`; the domain names the scale |

GitHub has a closed state of its own, independent of labels — but `done` is still carried as a sixth
label, and the two are set together. Relying on closed alone would leave the finished issue wearing
`status:review`, so every query for work awaiting verification would keep returning it.

## Why the mechanical half is a tool

**Every reversible operation in the binding is executed by Estigia's tools, not by an agent composing
`gh` calls**: a run once moved labels correctly through the whole state machine and executed a
mechanical step zero times in a whole session, and prose cannot fix a run that does not execute
prose. So the steps that are **mechanical and verifiable** — label swaps, the project projection and
its read-back, claim adjudication, the renewal, branch and worktree creation, draft PR publication,
CI release, the closing-keyword scan — are code. The steps that need **judgement** — what is worth
analysing, whether a blocker is discharged, whether a diff passes review — are not, and stay in the
prose where they belong.

There is no interpreter and no script on the path: the operations answer in one place. The tools read
the same operator configuration the prose does, so it never becomes a third source of truth, and
`estigia config list` prints what they resolved.

Where `gh` cannot be installed, the REST API with a token does everything the binding describes at the
cost of longer invocations; the installer checks for `gh` and tells you rather than fetching it.

## Exit codes: why a failed read is not a failed answer

Exit `3` is never a stand-down and never clearance: treating a timeout as either halts every run or
lets a run write deaf. Exit `5` is not retried blindly, because a write may have landed before the
failure surfaced. Exit `2` is not a lesser `1`: no authority changed hands and nothing was attempted,
so a run fixes the configuration and retries; reporting it as `1` makes a run abandon a claim it still
holds.

## What the store keeps, and what an older binary posted

Parsing prose for the same answer is fragile — any rewording breaks it — so every control marker these
operations record carries a machine-readable trailer. `claim`, `reclaim`, `unassign`, the renewal half
of `comment` (transport `heartbeat`) and of `claim` (transport `start-branch`), and the review
operations record theirs in the local store; none is posted to GitHub as a comment. The shape below is
the one the store keeps and the one an older binary posted:

```
<!-- issue-flow: claim run-id=claude-code-60fabae1 horizon=2026-07-25T23:00Z -->
<!-- issue-flow: standdown run-id=claude-code-d7d8a22e -->
<!-- issue-flow: heartbeat run-id=claude-code-60fabae1 -->
<!-- issue-flow: reclaim run-id=claude-code-3f1a0b2c from=claude-code-60fabae1 -->
```

Two vocabularies read these, and they must agree about the same words. **Release** kinds
(`standdown`, `release`, `unassign`, `reclaim`) mean a run-id no longer holds the item, so its earlier
claim stops counting. **Control** kinds (`standdown`, `reclaim`, `adjudication`) mean a message
instructs a run-id to stop. `reclaim` names the run it took over **from**, not the run that wrote it —
a marker's subject is not always its author.

The store keeps this text as an append-only row; an older binary's comment is read through the import
under the same rules and is inert once edited. A control message must both **name the run-id AND
instruct**: a heartbeat that mentions a run-id in passing ("waiting on `<run-id>`'s measurement
phase") instructs you to do nothing, and reading it as a stand-down would have you abandon work nobody
asked you to drop. Only `viewerDidAuthor=true` comments are control input.

`comment` with `expect_state` runs the renewal **before** it records, and refuses to record when the
renewal says stop, so a loser's next renewal exits `1` instead of writing. The renewal does not prevent
losing the race; it caps the cost of having lost it at one renewal interval. The two halves have their
own kinds — `heartbeat`, `progress` and `blocked` for the renewal, `note`, `blocker` and `diagnosis`
otherwise — and a kind named against the other half is refused rather than guessed at.

## Credentials on a sandboxed runtime

`gh auth status` may report a token stored in a system keyring that a sandboxed runtime cannot read;
there, use `GH_TOKEN` instead. Verify with `gh issue list` before relying on the workflow — an analyst
that cannot file its issue has done the work and lost it.

`gh issue` needs no `project` scope. Project mirroring does — prove it once with
`gh project list --owner <owner>`, or just run any `transition`, which reports a missing scope as a
skip rather than failing.
