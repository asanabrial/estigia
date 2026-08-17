# Binding: GitHub Issues

How this workflow's operations are performed against GitHub, and which of its assumptions GitHub
satisfies natively rather than by convention. `SKILL.md` describes WHAT happens; this file is the
only place that says HOW.

Read this alongside `SKILL.md` when the operator configuration names `github` as the tracker.

## The mechanical half runs as a tool

**Every reversible operation below is executed by Estigia's tools, not by an agent composing
`gh` calls.** That is not an optimisation — it is the fix for a specific, repeated failure:

> *"not a missing permission, not an unclear config, not a tracker limitation. The instruction was
> present and the run simply never executed it."* — `SKILL.md`, on a run that moved labels correctly
> through the entire state machine and mirrored the board **zero times in a whole session**.

Prose cannot fix a run that does not execute prose. So the steps that are **mechanical and
verifiable** — label swaps, the board mirror and its read-back, claim adjudication, the renewal,
branch and worktree creation, draft PR publication, CI release, the closing-keyword scan — are code. The steps that need
**judgement** — what is worth analysing, whether a blocker is discharged, whether a diff passes
review — are not, and stay in the prose where they belong.

Each operation is one of Estigia's MCP tools, called by name. Where this document writes a command
line — `SCRIPT <operation> --flag value` — the flags are the tool's arguments under the same names.

There is no interpreter and no script on the path. This used to name one, beside the contract, and
that shipped a second implementation of every decision below: one that the agent ran and one that
the gate read, alive at the same time and able to disagree. They did — about which comments name a claim, about when a
takeover's evidence is bound, about whether a branch is linked. The operations answer in one place
now.

The tools read the SAME operator configuration this prose does — the block between the config
markers in `SKILL.md`, overridden by `estigia.local.md` — so it never becomes a third source of
truth. The `config` operation prints what it resolved; run it once if you are unsure what the board
or worktree row currently says.

**Exit codes carry the distinction this workflow calls *a failed read is not a failed answer* —
stated as a decision gate in `SKILL.md` and evidenced in `references/safety-incidents.md`:**

| Exit | Meaning | What you do |
|---|---|---|
| `0` | the operation completed **and its read-back verified** | continue |
| `1` | **STOP** — a check answered stop (lost race, stand-down, wrong state, closed issue) | follow the JSON's `action`; do not retry |
| `2` | usage or configuration error; nothing was attempted | fix the invocation |
| `3` | the **READ itself** failed — the control surface answered NOTHING | fail closed: write nothing, retry the read. Never clearance, never a stand-down |
| `4` | internal error — a defect in the tool | state is **UNKNOWN**; re-read the issue before doing anything |
| `5` | a **WRITE** failed | **re-read** to establish what landed, then decide. Do not retry blindly |

Exit `3` is the one to read carefully. Treating a timeout as a stand-down lets a flaky network halt
every run; treating it as clearance lets a run write deaf, which is the defect the renewal exists to
close. The tools never collapse the two.

`5` exists because `3`'s advice is actively wrong for a write. "Nothing was learned, retry" is right
when a read failed; a write may have landed in the instant before the failure surfaced, so retrying
blindly duplicates it. `4` exists because without it an unhandled exception exits `1` with a
traceback and no JSON — indistinguishable from a deliberate STOP, with no `action` to follow.

`2` is not a lesser `1`, and the boundary between them is about AUTHORITY. Exit `1` says a check
read the control surface and it answered stop; the instruction that follows is "do not retry". A
malformed worktree template, a run ID that cannot name a directory, a missing `Worktree location`
answer none of that — no authority changed hands, nothing was attempted, and a retry after fixing
the configuration is exactly the right move. Reporting one as the other costs a run its work in the
direction that matters: told it lost a race, it stands down and abandons a claim it still holds.

**What is deliberately NOT scripted**: `merge`, `publish_version`, `close`, and the interpretation of
`review_status` / `ci_status`. Those write irreversibly to the remote or require a verdict, and a
defect in a script must not be able to merge, tag or close anything. The agent performs them itself,
by the prose further down, after verifying SHAs.

**Where `gh` cannot be installed** — a locked-down image, a sandbox with no package manager — the
REST API with a token does everything below at the cost of longer invocations. An analyst reaching
the API over HTTPS is exactly as capable as one running `gh`, which is the whole reason the analyst
role was defined as network-only. `gh` itself is a separate install
(`winget install GitHub.cli`, `brew install gh`, `apt install gh`); the installer deliberately makes
no network calls, so it checks and tells you rather than fetching it.

## What GitHub provides

The workflow asks a tracker for six things. What a binding must declare is not only which of them
exist, but what happens where they do not — a capability that is absent and undeclared is how a rule
silently stops applying.

| The workflow needs | GitHub | Consequence |
|---|---|---|
| A single-valued **state** per item | ✗ — labels are multi-valued | "exactly one `status:*`" is a **discipline, not a constraint**. Nothing stops two; every transition must remove the old label in the same call |
| A **claim** applied server-side | ⚠️ — assignees are a set, **but agents usually share one account** | then the set has one element however many runs claimed, and re-reading it can never show a collision — see *The claim hazard* |
| **Last activity**, timestamped by the server | ✓ — per-comment timestamps | only verified holder liveness markers renew a claim; unrelated `updatedAt` changes do not |
| A stable, short **identity** | ✓ — the issue number | usable verbatim in branch and worktree names |
| **Comments**, append-only and ordered | ✓ | heartbeats, blockers and hand-offs all land here |
| A native **priority** | ✗ | encoded as a label `<scale>:<value>`; the domain names the scale |

GitHub has a closed state of its own, independent of labels — but `done` is still carried as a
sixth label, and the two are set together. Relying on closed alone would leave the finished issue
wearing `status:review`, so every query for work awaiting verification would keep returning it, and
a board grouped by status would never move the card: closing an issue emits `issues.closed`, which
is not a label event and fires no mirror.

## The claim hazard — why the assignee cannot adjudicate

The obvious reading of GitHub is that assignees are a set, so two agents claiming leaves two
assignees and the collision is visible. **That is true only when the agents are different accounts,
and they usually are not.** Agents authenticate as one shared account, so `--add-assignee @me` twice
leaves exactly one assignee, and the re-read shows a clean issue assigned to you — while another run
is already building it.

So verification here does not read the assignee. It reads the **comment timeline**, which the server
orders and which no later writer can reorder. `claim` writes its comment first and then adjudicates
from that timeline; the earliest live claim wins.

This is the same mechanism the Linear binding needs, arrived at from the opposite direction — there
because the assignee is overwritten, here because it is shared. The lesson generalises further than
either: **an identity field cannot adjudicate a race between runs that share that identity.**

### Control markers

`SKILL.md` warns that "parsing prose for the same answer is fragile — any rewording breaks it", and
adjudication used to depend on exactly that. So every control comment these operations write
carries a machine-readable trailer alongside the sentence a human reads:

```
<!-- issue-flow: claim run-id=claude-code-60fabae1 horizon=2026-07-25T23:00Z -->
<!-- issue-flow: standdown run-id=claude-code-d7d8a22e -->
<!-- issue-flow: heartbeat run-id=claude-code-60fabae1 -->
<!-- issue-flow: reclaim run-id=claude-code-3f1a0b2c from=claude-code-60fabae1 -->
```

Two vocabularies read these, and they must agree about the same words. **Release** kinds
(`standdown`, `release`, `unassign`, `reclaim`) mean a run-id no longer holds the item, so its
earlier claim stops counting. **Control** kinds (`standdown`, `reclaim`, `adjudication`) mean a
message instructs a run-id to stop. Note that `reclaim` names the run it took over **from**, not the
run that wrote it — a marker's subject is not always its author.

They are HTML comments, so they are invisible in rendered markdown. Ownership comments are
append-only protocol records: if edited, they become inert because GitHub no longer exposes their
original bytes; corrections append a fresh operation. Reading falls back to the prose forms
(`Claimed by <run-id>`) for comments written before markers existed, and that fallback is
deliberately narrow: a control
message must both **name the run-id AND instruct**. A heartbeat that mentions your run-id in passing
("waiting on `<run-id>`'s measurement phase") instructs you to do nothing, and classifying it as a
stand-down would have you abandon work nobody asked you to drop.

Only `viewerDidAuthor=true` comments are control input. Reclaims must match the stale prior holder
unless marked `forced=true`; horizonless acquisitions expire four hours after trusted activity.

**Worked example — a real claim race, 2026-07-22.** The timeline, from the issue's comment trail:

| Time (UTC) | Event |
|---|---|
| 14:40:03 | `claude-code-cb8d3f2c` writes its claim comment |
| 14:40:08 | `claude-code-d7d8a22e` writes its claim comment, 5 s later, before the collision is visible |
| 14:40:41 | adjudication comment: the earliest claim wins, `claude-code-d7d8a22e` is told to stand down |
| 14:49–15:24 | `claude-code-d7d8a22e` posts heartbeats 1, 2 and 3 and keeps building — none of its writes re-read the timeline |
| 15:24:21 | the winner delivers |
| 15:28:52 | `claude-code-d7d8a22e` finally stands down, retracting a measurement taken on work it no longer held |

Under write-only heartbeats the stand-down sat unread for 48 minutes. `heartbeat` makes that
timeline impossible: `heartbeat` runs the renewal **before** it posts, and refuses to post when the renewal
says stop — so the loser's next heartbeat at 14:49:22 exits `1` instead of writing. The renewal does
not prevent losing the race; it caps the cost of having lost it at one renewal interval.

## State names

The workflow's states are `analysis`, `ready`, `in-progress`, `review`, `blocked` and `done`. Here
each one is stored as the label **`status:<name>`** — the prefix is this binding's convention, not the
workflow's, and it exists so a glance at an issue separates state labels from attribution and domain
ones.

## Operations

`<n>` is the issue number throughout. `SCRIPT` abbreviates *the Estigia tool for this operation*:
each row's command names the operation and the flags are that tool's arguments, spelled the same.

Every ownership write uses a lowercase 32-hex `--operation-id`, reused unchanged on retry. Review handoff and verdict IDs are derived from their immutable inputs, so an unchanged retry names the same event. `reclaim` and `unassign` first return the exact target without writing; repeat with `--target-operation` to authorize only that epoch. Historical legacy ownership remains readable, but broad legacy controls cannot mutate a modern epoch. `review-handoff` and `review-verdict` are neither release nor activity markers: the compound handoff releases through its separate exact `unassign`, and neither event extends a claim.

| Operation | Command | What it guarantees beyond the obvious |
|---|---|---|
| `ensure_states` | `SCRIPT ensure-states` | idempotent; run it before your first write to an unfamiliar project |
| `create` | `SCRIPT create --identity <id> --title <t> --body-file <f> --priority <scale:value> --domain <name> --runtime <rt> --run-id <id> [--state ready\|blocked]` | creates every label **before** attaching it, then mirrors the initial board column — the case everyone forgets, because no `transition` ever follows a fresh issue to correct an empty `Status` |
| `list_state` | `SCRIPT list-state --state <s> --run-id <id>` | unassigned only, `--limit 200` (gh's own default is 30 and silently truncates the queue; 200 is **also a ceiling**, so an answer at the limit may be a longer queue). For `review`, it reads every candidate timeline, excludes unresolved handoffs from their publishing/requesting runs with receipt, blocker, discharger and deadline metadata, and fails the whole read when any candidate timeline is unreadable. Another run remains eligible. Results are partitioned by `domain:<name>` and never ranked across partitions |
| `claim` | `SCRIPT claim --issue <n> --run-id <id> --runtime <rt> --horizon <UTC> --operation-id <32-hex>` | retries may duplicate transport, never the semantic event; renewals use a fresh operation ID. A publishing/requesting run cannot acquire its unresolved review handoff; expiry never changes that answer |
| `reclaim` | `SCRIPT reclaim --issue <n> --run-id <id> --runtime <rt> --horizon <UTC> --operation-id <32-hex> [--target-operation <epoch>] [--force --reason-file <f>]` | target discovery is read-only; the write binds target, evidence, privilege and metadata, then proves exact projections. It refuses the publishing/requesting run of an unresolved review handoff on the same terms as `claim`, so a replacement that went stale cannot return the item to the one run forbidden to review it |
| `verify_claim` | `SCRIPT verify-claim --issue <n> --run-id <id> --expect-state <s> [--allow-closed-by-pr <pr>]` | proves the requested run is the reducer's current live winner and uses timeline position, not second-precision timestamps, as its control-message watermark. A closed-issue exception reads every page of closing PRs and accepts only the exact supplied PR; incomplete or malformed pagination is a failed read |
| `transition` | `SCRIPT transition --issue <n> --to <s> [--from <s>]` | mirrors the board **first**, swaps the label in **one** call, then reads **both** back and repairs a board that disagrees. Omitting `--from` removes whatever stale state labels it finds |
| `comment` | `SCRIPT comment --issue <n> --body-file <f> [--run-id <id> --kind note\|blocker\|diagnosis]` | file-based body, always; `--run-id` and `--kind` are a pair. Every generic comment gets a non-control marker, and quoted issue-flow markers plus claim-shaped legacy prose are escaped, so generic text cannot become a control event or fall through to the prose parser |
| `heartbeat` | `SCRIPT heartbeat --issue <n> --run-id <id> --expect-state <s> --body-file <f>` | renewal first, post second; **refuses to post** when the renewal says stop and escapes control-shaped text before appending its own heartbeat marker |
| branch + worktree | `SCRIPT start-branch --issue <n> --branch <b> --base <base> --run-id <id>` | renews first, then serializes on a branch-scoped lock and reserves the whole local checkout **before any GitHub mutation**, so a lost reservation leaves no remote state. Resumes only a checkout whose durable ownership marker names this run; a branch-only template gains a run-scoped sibling in memory without rewriting `estigia.local.md`. Resolved paths retain native POSIX spelling on Linux and macOS, while Windows' `\\?\` prefix is removed before invoking Git because Git for Windows rejects its slash-normalised `//?/` form. It fetches the named base before recording or branching from its remote-tracking ref. Ref existence is probed so absence and a failed read are different answers, a nonzero or timed-out `gh issue develop` is re-read rather than believed, and the isolated worktree head, published head and recorded remote base must agree before success is reported; the caller's main checkout may diverge without being mistaken for the new branch. It records the base COMMIT and the base TREE, so a later movement check compares against what the base contained rather than resolving it again. The local worktree registry is read NUL-delimited and fails closed: a failed, truncated or contradictory read is a failed read, never an empty registry |
| `publish_review` | `SCRIPT publish-review --issue <n> --branch <b> --base <base> --run-id <id> --pr-title <t> --pr-body-file <f> [--worktree <p>]` | requires a clean target. It **reuses** the single open PR or creates one and refuses on more than one. A reused ready PR is converted to draft and draft readback is confirmed **before push**; a new PR is created with `--draft` after its CI-silent topic push. Reused title/body are refreshed. Readback must confirm draft plus exact head/base. The published marker and answer bind a fresh epoch, PR, head, base, and complete path/mode/blob manifest digest; every republish creates a new epoch and invalidates prior evidence |
| `republish_review` | `SCRIPT republish-review --issue <n> --branch <b> --base <base> --run-id <id> --pr-title <t> --pr-body-file <f> [--worktree <p>] [--expect-state <s>]` | the same operation as `publish_review` in everything but the push, for a branch whose history was rewritten — rebased onto a moved base, or amended, where the ordinary push is refused as a non-fast-forward. It reads the **latest `published` marker on the timeline** and pushes `--force-with-lease=<branch>:<that head>`, so the remote is required to still be at the bytes a receipt bound; a remote that moved since refuses the push and nothing is destroyed. The live claim is verified a second time immediately before the push, after the fetch, target derivation, keyword scan, PR listing and draft conversion that separate it from the first. An issue with no recorded publication is refused `published-receipt-missing`: the first publication is `publish_review`'s. There is no plain `--force` and no way to reach this from `publish_review` |
| `handoff_review` | `SCRIPT handoff-review --issue <n> --run-id <id> --target-operation <epoch> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --blocker <text> --discharger <text>` | one ordered operation: verifies the live `review` claim; validates the globally latest full receipt and exact ownership epoch; records and reads back an immutable `review-handoff` carrying configured authority, internally generated request time and one deadline; invokes the existing idempotent exact-target `unassign`; then confirms the released epoch is no longer authoritative and state remains exactly `review`. Only after this success does the MCP pointer clear |
| `review_verdict` | `SCRIPT review-verdict --issue <n> --run-id <id> --reviewer <id> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --outcome accepted\|rejected` | requires a live claim in `review` by the recording run, the exact globally latest receipt, and a credited reviewer distinct from the publisher and every requester. No handoff is required: after one the reviewing run names itself, and a run that acquired a reviewer without releasing the claim names that reviewer instead — the answer marks the second `self_attested`, which records a declaration rather than checking it. The immutable marker is operation-idempotent. The latest qualifying outcome resolves any transfer so the publishing run may resume; only `accepted` qualifies delivery, and a later rejection withdraws that qualification. Prose, CI, elapsed deadlines, PR readiness, edited/untrusted markers, stale receipts and the publishing/requesting run resolve nothing |
| `release_ci` | `SCRIPT release-ci --issue <n> --run-id <id> --epoch <id> --pr <n> --head <sha> --base <sha> --digest <sha256> [--worktree <p>]` | verifies a live claim in `review`, requires the supplied receipt to be the globally latest complete publication and attributed to this run, requires a distinct accepted `review-verdict` for that same receipt, re-reads the same draft PR/head/base, and re-derives one coherent clean target before `gh pr ready`. Every ready outcome is read back: an exact ready target is success even when the command failed ambiguously; otherwise the read/write taxonomy is preserved |
| `expected_target` | `SCRIPT expected-target --base <sha> [--worktree <p>] [--native-start <f>]` | read-only. Emits the COMPLETE delivery target — the exact base through `HEAD` plus the uncommitted worktree — as a path/mode/blob manifest with one digest. With `--native-start` it compares a reviewer's target against it and **fails closed**, naming the paths that would ship unreviewed. Uses `ls-tree`, `--no-optional-locks status` and `hash-object` without `-w`, so no object is written and the index, worktree and refs are untouched — it is safe to run against a tree somebody is still working in |
| `base_movement` | `SCRIPT base-movement --base <branch> --recorded-base <sha> [--worktree <p>]` | read-only. Classifies later base movement as none, compatible, overlapping, conflicting or unknown, from exact refs, changed paths and `git merge-tree --write-tree` (git 2.38+; below that the answer is `unknown`), which touches no index, worktree or branch — it does fetch, and does write the merged tree into the object store, so it changes nothing about the delivery but is not literally write-free. Unknown is never compatible, and semantic impact is reported as the caller's judgement rather than decided |
| `changelog-notes` | `SCRIPT changelog-notes --version <x.y.z> --file <changelog> [--out <f>]` | reads the changelog; **`--out` writes that file**. Extracts the version's entry for its tag and Release, anchored on the version **opening** the heading. Fails closed on a missing or empty entry — a tag is immutable, so notes invented at tag time are permanent |
| `check closing keywords` | `SCRIPT check-closing-keywords --issue <n>` | run again before merging: the branch's commit messages can introduce one after the body is already clean. Historical merged closers and open PRs for another named branch/base are excluded from the current-delivery verdict |
| `unassign` | `SCRIPT unassign --issue <n> --runtime <rt> --run-id <id> --operation-id <32-hex> [--target-operation <epoch>] [--held-by-other]` | target discovery is read-only; retries cannot release a later acquisition |
| board audit | `SCRIPT audit-board [--fix]` | compares every card's column against its own `status:*` label. **Zero cards is reported as a failed read, not a clean board**, and a card whose labels did not all arrive lands in `unread_labels` rather than in `drift` |
| `review_status` | *(agent judgement, recorded by `review_verdict`)* | The reviewer or panel judges the change; the tool records only one aggregate outcome and exact publication identity. Use one independent context for `Blind judges: single`, the unanimity policy for `two blind`, or the same-finding 3-of-5 policy for `five blind`. Estigia cannot prove panel size, concurrency, independence, blindness, same-finding identity or quorum. Reject a verdict naming any earlier epoch |
| `ci_status` | *(agent, not scripted)* | see *CI, merge and delivery* below |
| `merge` | *(agent, not scripted)* | see *CI, merge and delivery* below |
| `publish_version` | *(agent, not scripted)* | see *Version tags* below |
| `last_activity` | `gh issue view <n> --json comments` | only trusted heartbeat/branch/published markers authored by the holder extend liveness beyond acquisition; mentions and control targets do not |
| `close` | *(agent, not scripted)* | see *Closing* below |

### Why markdown bodies always go through a file

Inline `--body`/`--comment` corrupts markdown on a PowerShell runtime. Seen live: three separate
comments, from two different runtimes, posted with the backtick stripped or eaten entirely —
`` `floor_starvation` `` arrived as `\loor_starvation\` (the backtick-plus-`f` was consumed as
PowerShell's form-feed escape, taking the `f` with it), `` `status:blocked` `` arrived as
`\status:blocked\`, and every intended line break arrived as the two literal characters `\n`.
Backtick is PowerShell's escape character; a double-quoted `--body "<text with `code spans`\nand
newlines>"` gets expanded by the shell BEFORE `gh` ever sees it, and no amount of care in the text
itself prevents that — the corruption happens one layer below where the text is composed.

**The tools remove this class rather than warning about it**: every subprocess call passes an
argument list with no shell, and every markdown body goes through a temp file. That is also why the
operations are answered in **one** implementation rather than a PowerShell/bash pair — writing the
fix in the language that causes the bug, twice, in two implementations that would then drift, is not
a fix. It was a script beside this contract for a while, which was one implementation in the wrong
place: the agent ran it and the gate read something else.

Where you still compose a `gh` call by hand (`close`, `merge`), write the body to a file and use
`--body-file`, with no exception for "this one's short."

### Three refusals about the call itself

These say nothing about the tracker. They say the call could not be made, so nothing was attempted
and nothing was written — retrying the same call unchanged will produce the same answer.

| `reason` | What it means | What to do |
|---|---|---|
| `unknown-operation` | The operation name is not one that exists. | Use a name from the operations table above. |
| `missing-argument` | A required argument was not passed. The refusal names it in `argument`. | Pass it and call again. |
| `malformed-argument` | An argument was passed and could not be read as what it has to be — an issue number that is not a number. The refusal carries the `value` it was given. | Correct it and call again. |
| `blank-argument` | A required argument arrived carrying nothing. It is neither missing nor malformed, and it is refused because every one of these is taken into the world as a name: an empty `--to` writes `status:` with nothing after the colon, an empty `--run-id` claims an issue for nobody. The refusal names the flag in `argument` and in its own sentence. | Send the value and call again. |
| `incomplete-review-receipt` / `invalid-review-verdict` | A review operation did not carry a 32-hex epoch, positive PR, full lowercase head/base SHAs, 64-hex digest, or accepted/rejected outcome. | Correct the invocation from the latest `publish_review` answer; do not shorten or reconstruct receipt fields. |
| `invalid-review-authority` | The installed `Review delegation` value cannot be converted into `auto`, `ask`, or a supported timed `ask`. | Correct that configuration row before recording a handoff. |
| `review-handoff-operation-conflict` / `review-verdict-operation-conflict` | One deterministic operation ID is already bound to different or incomplete immutable marker fields. | Do not edit protocol comments. Re-read the timeline; unchanged retries reuse the original event, while a genuinely different event needs different immutable inputs. |
| `dirty-review-target` | Publication or CI release found uncommitted paths, which no PR head can identify. | Commit or remove them, then publish a new epoch. |
| `review-target-moved-during-read` | HEAD changed while the clean manifest was being derived. | Wait for the checkout to stop moving and derive a new target. |
| `draft-readback-failed` | A reused PR did not read back as draft before push. | Do not push; establish why GitHub kept it ready. |
| `published-receipt-missing` / `published-receipt-mismatch` | The supplied epoch/PR/head/base/digest is incomplete or is not the latest recorded publication. | Use the latest complete receipt, or republish and review the new epoch. |
| `review-handoff-requester-excluded` / `reviewer-not-distinct` | The publishing/requesting run tried to acquire its own unresolved handoff, or to be credited with reviewing its own receipt. | Use another run, or credit the reviewer you actually acquired. Time, CI and prose never discharge it. |
| `qualifying-review-verdict-missing` | No accepted verdict crediting a distinct reviewer matches the latest full receipt. | Keep the PR draft and the issue in `review`; record the missing exact-receipt evidence. A deleted comment reaches here too — evidence that is gone is evidence that is missing. |
| `release-pr-mismatch` / `release-target-mismatch` | The current draft PR or freshly derived clean target differs from the receipt. | Do not release CI; republish and obtain fresh review evidence. |
| `ready-readback-failed` | The ready write was not confirmed with the same PR/head/base. | Re-read the PR before any further action; the write outcome is not established. |
| `worktree-location-not-absolute` | `--worktree-root` names a relative directory, so its destination depends on which checkout invoked Estigia and can place one worktree inside another. | Pass an absolute directory for isolated checkouts. |

They exist because the operations are answered **in process** now rather than by a command line: an
argument the caller left out used to be `argparse`'s refusal on the way in, and something has to say
it here. What must never happen is the alternative — an operation nobody implements answered with
`ok`, or handed somewhere unnamed and coming back looking answered.

## Auto-close, and why it does not end the work

GitHub can close an issue on merge without the workflow's `close` ever running: no transition to
`done`, no mirror, labels frozen wherever they were. The board then shows an open column for a
closed issue, and it was not any run's doing — it was the tracker's own automation acting outside
the workflow. Seen live: an issue closed by its delivery commit sat CLOSED wearing `status:ready`
until an audit caught it.

**Two different things cause it, and only one of them is a defect.** They produce the identical
symptom — a non-empty `closedByPullRequestsReferences` — so the symptom alone must never be read as
"somebody wrote a keyword":

| Cause | Status | Remedy |
|---|---|---|
| A **closing keyword** in the PR body or a commit message | forbidden — the text is yours | remove it, use `Refs #<n>`, re-check |
| A **branch link** created by `gh issue develop` | **expected under this binding** | none — no edit removes it |

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

**So the rule is not "prevent the auto-close". It is: `transition` to `done` after the merge
REGARDLESS of whether GitHub already closed the issue.** Closing an issue emits `issues.closed`,
which is not a label event: it moves neither the `status:*` label nor the board column. The
auto-close is the tracker's bookkeeping, never the workflow's `close`, and the state machine does
not know it happened. Run the transition afterwards and the end state is correct; skip it because
"the issue is already closed" and you have reproduced the exact incident above.

`check-closing-keywords` and `publish-review` report the cause, not just the symptom: a keyword is a
hard stop (exit `1`), a branch link is reported with the follow-up it mandates and does not block.

`publish-review` scans for a keyword **before its first remote mutation** — before it lists open pull
requests, before a reused one is drafted or its body refreshed, and before the push. Reads precede it —
the claim renewal and the base fetch — and no write does. The sources are
the commit messages `origin/<base>..<branch>` adds and the body it is about to write, both readable
locally, so that refusal leaves the remote exactly as it found it and says so.

*Before the push* is not far enough, and the difference is not academic: a reused pull request is
drafted and has its title and body replaced first, and a body carrying `Closes #<n>` is written
verbatim, so a refusal there has already published the hazard it exists to refuse.

The scan after the readback stays — a keyword can also arrive from the remote side of a pull request
this run did not write, and it settles a branch-derived link that needs the PR to exist. A refusal
from *there* carries `"world": "committed"`, which every surface reads as **the write landed; what
failed came after it** rather than as nothing having happened. Any refusal without that field means
what it always meant.

A `git log` that does not answer is not a branch with no keywords: both this scan and
`check-closing-keywords` refuse on it rather than continuing with an empty list.

**An issue that is not finished when its PR merges is a separate case**, because the auto-close does
not care whether the delivery was the whole issue or one slice of it. Marking a partly-delivered
parent `done` because GitHub closed it is the failure; the continuation model — a verified renewal
under `--allow-closed-by-pr`, an explicit reopen, and a transition back into the state that should
survive — is in *Closing* below. Nothing there is a shortcut around this section: it exists so a
retained issue can outlive its own merge without ever being reported as delivered.

**Write this, so the wording is not improvised per PR.** Knowing the rule is demonstrably not enough
— `Fixes #<n>` is the muscle-memory opening of a PR body, and a run reaches for it while composing
the caveat that forbids it. Seen live (2026-07-25, PR #97): the first line read, verbatim,
`Fixes #61 (link only — closing keywords are not used per project workflow; state is moved via the
tracker)`. The parenthetical states the rule and the sentence breaks it. The safe form:

```
Refs #<n> — a plain reference, deliberately NOT a closing keyword.
```

The forbidden set is not just `fixes`: GitHub honours **`close`, `closes`, `closed`, `fix`, `fixes`,
`fixed`, `resolve`, `resolves`, `resolved`**, case-insensitively, in a PR body or any commit message
on the branch. `Refs`, `Implements`, `Part of` and a bare `#<n>` all link without closing.

**Then verify it mechanically, because reading your own prose is how it slipped through.**
`publish-review` runs the check as part of publishing and refuses to finish while a keyword is live;
`check-closing-keywords` runs it standalone. Run it again before merging — the branch's commit
messages can introduce one after the body is already clean. Editing the body does remove the link,
though the value can lag a few seconds, so re-run rather than trusting the first response.

**One consequence to state rather than rediscover.** A board's native *Linked pull requests* column
is populated by these references, so under `gh issue develop` it fills itself and under a
hand-created branch it stays empty. Either way, never add a keyword to populate it — that trades a
cosmetic gap for the one cause this section actually forbids. The durable joins are the timeline
cross-reference (`#<n>` in the PR body), the branch linkage, and the delivering SHA in the close
comment.

## Branch, worktree and the linked issue

`gh issue develop <n> --name <branch> --base <base>` creates the branch **server-side, from the fresh
base, already linked** in the issue's Development sidebar — one command replacing branch creation AND
recording. A branch nobody can find from the issue is work nobody can follow.

`start-branch` runs it **last**, not first. It fetches, resumes an existing local or remote-only
branch without moving it — only a branch absent everywhere starts from the fetched base — creates
the worktree and records ownership, and only then links the issue. *Nothing is created remotely
until the local checkout is reserved*, below, is where that order and its reasons live; the summary
here exists so the sequence is not learned from this paragraph in reverse. It records branch, base
SHA, worktree path and holding run-id on the issue either way.

The worktree path comes from the `Worktree location` configuration row, with `<repo>`, `<branch>`,
`<issue>` and `<run-id>` substituted; `<repo>`, `<branch>` and `<issue>` are flattened, so a
`docs/113-…` branch does not create a stray `docs/` directory under the worktree root.

**`<run-id>` is the one value that is validated instead of flattened**, and the difference is the
whole isolation guarantee. Flattening is not injective: `a/b` and `a-b` are two distinct valid run
IDs that flatten onto ONE directory, and the run that arrives second finds a registered worktree
carrying its own branch at its own path — the exact shape of a legitimate resume. So a run ID must
already be a safe directory name and is refused otherwise: lowercase `a-z0-9` groups joined by
single `-`, nothing else. Each exclusion is a collapse rather than tidiness — separators restructure
the path, uppercase folds into lowercase on Windows and macOS so isolation would depend on the
filesystem, and Windows silently strips trailing dots and spaces so `run-a.` and `run-a` are one
directory to the OS and two keys to us. Reserved device names (`con`, `nul`, `com1`…) are refused in
**every** substituted component, because they name a device rather than a directory in any case and
with any extension.

**What the path must guarantee is that no two live runs share a directory.** Every template is now
made to carry that guarantee: one without `<run-id>` is migrated **in memory** to a run-scoped
sibling — `…/<branch>` becomes `…/<branch>~<run-id>` — and `estigia.local.md` is never rewritten. A
transport command that silently edits the operator's own policy file is a worse failure than the one
it fixes, and that file may be shared across machines where the migration is not wanted. The result
reports `template_migrated` so the substitution is visible rather than assumed.

A sibling and not a child (`…/<branch>/<run-id>`), so that a legacy checkout is neither a parent nor
a child of the new one — runs colliding inside a directory another run owns is the entire defect.

**The migration joins the two halves with `~`, and that is not cosmetic.** Joining with `-` makes
the composed name ambiguous even though each half is unambiguous on its own: branch `fix/6` with run
`a-b` and branch `fix/6-a` with run `b` both spell `fix-6-a-b`. Those are two *different* branches,
so they take two different branch locks, and nothing downstream would notice them sharing one
directory. Git rejects `~` in a ref name and the run-ID alphabet rejects it too, so it appears in
neither half and the split point is unique. If you write your own template, join `<branch>` and
`<run-id>` with a character that cannot occur in either — `~` is the one this binding uses.

Relying on git to catch the collision instead was tried and does not hold. `worktree add` refuses a
branch already checked out elsewhere (`fatal: '<branch>' is already used by worktree at …`), but that
is a read followed by a write with no lock between them: the review probe for issue #31 produced two
successful concurrent worktrees for one branch on Git 2.55.0.windows.3. Git loses the race its check
exists to win, so `start-branch` serializes on its own **branch-scoped lock** first (below). The
2026-07-24 incident — two runs deriving one directory, the loser writing its model, its migration and
its tests into the winner's checkout mid-build — is what a lost race costs.

**So `start-branch` asks whether the directory is YOURS, and requires proof.** "The registry says
this path holds this branch" is not proof: two runs of one branch satisfy it equally well, which is
how the second one checks out on top of the first. So the registry answers only half, and a durable
**ownership marker**, written by the run that created the checkout into that worktree's private git
admin directory, answers the other half. It lives there rather than in the working tree because a
marker that shows up in every `git status` is a marker people delete, and `git worktree remove`
clears it along with everything else about the checkout, so a clean removal cannot leave a stale
claim behind.

Six outcomes, and only two of them proceed:

| What is at the path | Outcome |
|---|---|
| nothing | fresh checkout |
| a registered worktree for this branch, marker names THIS run | `resumed_existing_worktree` |
| a registered worktree for this branch, marker names another run | stop — it may hold unpushed work |
| a registered worktree for this branch, **no marker** | stop — `worktree-ownership-unproven` |
| a registered worktree for this branch, marker present but unreadable | exit `3` — a failed READ |
| anything else (stranger's checkout, orphan directory) | stop — `worktree-path-occupied` |

The first row is decided before asking git for a private admin directory: a checkout that has not
been created has no directory in which `git rev-parse --git-dir` can run and no ownership marker to
read. Existing checkouts still fail closed on an unreadable marker as the rows below require.

The fifth row is not a pedantic distinction. A marker that exists and cannot be parsed says the
ownership question went UNANSWERED, and an unanswered read is not a permissive default any more
than a missing one is — it is the same fail-closed rule the registry read follows.

The fourth row is the one worth defending: an unproven claim is not a permissive default. A
registered checkout of your branch with no marker cannot be told apart from one a run is using right
now, and the recovery for it is documented below rather than guessed at.

A worktree directory that is itself a symlink or junction is refused as a configuration defect
(`aliased-worktree-path`), since two run IDs pointing into one real directory defeats every rule
above. An *ancestor* being a link — a worktree root parked on another volume, `/tmp` on macOS — is
ordinary and is resolved rather than refused: it redirects every run's directory identically, so it
cannot fold two runs together.

**Two junctions that fold two runs together are caught, but not here — and the distinction is worth
stating because three attempts to catch it here all shipped defects.** With a hand-written
`…/<run-id>/checkout` template, links at `run-a` and `run-b` both pointing at one directory collapse
two runs while every leaf resolves cleanly. That is not a property of one path: the other run's
junction does not appear in your spelling, so no inspection of it can answer the question. Each
heuristic that tried failed in a different direction — refusing a link that redirects every run
identically, refusing a path that merely mentions the run ID twice, and failing open whenever the
run ID happened to survive elsewhere in the path.

What answers it is **resolution followed by ownership**, and resolution happens twice on purpose.
`canonical_worktree_path` resolves the path before anything is created, so the checkout is made and
registered under one spelling; `normalise_path` resolves again when the registry is consulted, so
the lookup compares real directories rather than spellings. Either one alone is enough to make two
aliased spellings meet — established by mutation, not by argument: breaking either resolution on its
own leaves the collapse still refused, and only breaking BOTH degrades the answer, and then only to
`worktree-path-occupied`, which is still a refusal. The redundancy is the point; neither resolution
is load-bearing alone, which is exactly what makes the property hard to remove by accident.

Once the two runs name one directory, the ownership marker decides: it names the first run, so the
second is refused as `worktree-owned-by-another-run` — from evidence rather than inference, and a
more accurate message than any path heuristic could have produced. A single path refuses only what
a single path can prove: its own leaf being an alias.

That refusal is exact for the case it names — two runs on ONE branch, sharing a branch lock, the
second arriving after the first recorded ownership. Two runs on *different* branches take different
locks, and a template that aliases them onto one directory is an operator misconfiguration rather
than a race. The loser is still refused, as `worktree-path-occupied`, because the directory it
resolves onto holds a checkout of somebody else's branch. Git's own "already used by worktree"
guard cannot help there: it compares branches, and these are two different branches.

### Nothing is created remotely until the local checkout is reserved

The order of operations inside `start-branch` is a guarantee, not an implementation detail. The
branch lock, the path decision, the local branch, the worktree and the ownership marker all happen
**before** the first call that mutates GitHub. Reversed — which is how it worked until issue #31 —
a run created a server-side branch and a sidebar link, then stopped on the local check, leaving
remote state advertising work no checkout was ever made for.

Once local isolation is reserved, `gh issue develop` runs. A **nonzero exit from it is not an
answer.** It mutates two remote things at once, a ref and a link, and its failures are not one
thing: "a branch of that name already exists" (the ordinary resume, where the link may well be
present), a partial failure that created the ref and not the link, and a connection that dropped
after the server had committed both. The exit code cannot tell them apart, and in the last two
"not linked" is simply false. So a nonzero exit and a timeout are treated identically — **re-read
the Development sidebar** and let the read decide:

| Re-read says | Outcome |
|---|---|
| the branch is linked | success, `link_outcome: already-linked` |
| conclusively not linked, and the command exited nonzero | proceed without a native link |
| conclusively not linked, after a **timeout** | ambiguous write, exit `5` — a ref may exist right now |
| the read itself failed | exit `3`; after a timeout, exit `5` |

One asymmetry inside that table is deliberate: only the ABSENT answer needs a complete page. Finding
the branch in the sidebar is conclusive whatever else the connection holds, because a later page
cannot un-link it, while concluding it absent rests on having seen everything. So a truncated
connection that contains the branch is a yes, and a truncated connection that does not is a failed
read — refusing both would turn an answered question into an unretryable exit `3`.

Existence probes follow the same rule. `git rev-parse --verify --quiet` was used before and cannot
answer the question: it exits `1` for "no such ref" AND for a corrupt object store, an unreadable
`.git`, a broken packed-refs file — every way the question can go unanswered. `git for-each-ref`
separates them, because a successful exit with no output is a real absence and anything else is a
read that did not answer. A run that reads "absent" from a failed read creates the branch again from
the base, and a resumed branch silently restarts from zero.

**Its argument is a pattern, not a path, so the refname is matched again on the way out.** A
`for-each-ref` pattern matches a ref completely OR from the beginning up to a slash, so
`refs/heads/foo` also matches `refs/heads/foo/bar` — and asking about a branch `foo` that does not
exist would otherwise return the child's object id. That is not an unhelpful answer but a wrong one:
`foo` would be reported as present at a commit belonging to another branch. Only an exact refname
counts; children are ignored rather than mistaken for the parent.

Finally, **a successful native creation must tell one story.** The named base is fetched before its
remote-tracking ref is recorded or used as the local start point; otherwise a stale clone creates
locally from yesterday's base while GitHub creates remotely from today's. The local head in that story is read
from the isolated worktree, not from the caller's main checkout: the latter may legitimately be
ahead, behind, or carrying unrelated work and says nothing about the new branch. `gh issue develop` branches from the
base as the SERVER sees it, which is not necessarily the base this run fetched and recorded. If the
base moved in between, the branch starts at a commit the run never saw while the command reports the
recorded base as its own — and every later `Reviewed-Base:` claim inherits that. So a fresh creation
requires local head, published head and recorded base to be equal, and a resume requires the
published head to be reachable from the local head. Local ahead of remote is ordinary unpushed work.
A published head that is NOT reachable from the local one is refused as
`remote-not-reachable-from-local` — named for what was established rather than for the likeliest
cause, because it covers both somebody pushing ahead of you and somebody rebasing or force-pushing
the branch out from under you, and pointing the reader at commits to fast-forward that may not exist
is worse than saying less. Either way, continuing would build on a head this checkout has never seen.
An ancestry that could not be established is not a pass.

A coherence stop is the one refusal that comes *after* a remote write, necessarily: the disagreement
cannot be seen until the branch has been published. Nothing is reported as delivered, but the branch
does exist remotely, so the payload says so rather than implying a clean slate — re-running resumes
it as it now stands.

### Recovering a worktree or a branch lock

Every stop above names a directory or a lock and refuses to touch it. That is deliberate — the thing
being refused may hold work nobody has pushed — so the recovery is yours, in this order. It is the
same on Windows and POSIX except where noted.

1. **Look before deciding anything.** `git -C <path> status --short --branch` and
   `git -C <path> log --oneline @{u}.. ` show uncommitted and unpushed work. `git worktree list`
   shows what the clone thinks it owns; the lock file named in the stop payload shows which run
   claimed the branch and when.
2. **Preserve anything useful first.** Commit and push to the branch, or `git -C <path> stash create`
   and note the object, or simply copy the directory aside. Nothing below is reversible.
3. **Prove the holder is stopped — do not infer it from elapsed time.** The lock names the run,
   issue, branch and acquisition time, but deliberately carries no machine identity or PID it cannot
   authenticate. Use the run's tracker activity and the runtime/session named by `run_id`; if that
   evidence cannot establish that the holder stopped, do not remove the lock. **This is why nothing
   here expires on a timeout**: the case a timeout gets wrong is a slow-but-live run whose checkout
   is taken while it is writing into it, which is the corruption all of this exists to prevent.
4. **Remove the checkout cleanly.** `git worktree remove <path>` (add `--force` only when step 2 is
   done and you accept the loss), then `git worktree prune`. Deleting the directory by hand leaves a
   registry entry that keeps refusing the path; `prune` is what clears that.
5. **Remove a proven-stale lock** by deleting the file named in the stop payload. It is a plain JSON
   file under `<common-git-dir>/issue-flow/branch-locks/`. Only after step 3.
6. **Re-run `start-branch` unchanged.** It is idempotent: it re-reads everything and either creates
   the checkout or tells you what still refuses it. A lock naming your OWN run is your own retry and
   never blocks you — it is *adopted*, so the attempt that succeeds is also the one that releases
   it. A run that merely stepped past its own leftover lock would leave nobody entitled to remove
   it, and the next different run would be blocked by it forever.

Windows: `git worktree remove` fails while any process holds a handle in the directory — an editor,
a terminal, an antivirus scan. Close them rather than forcing. POSIX: a worktree whose path contains
newline bytes is legal and is handled by the NUL-delimited read below; quote paths in the shell.

### Migrating a branch-only worktree

A branch-only `Worktree location` gets the in-memory run-scoped sibling described above, and that is
usually the end of it. The one hard case is a checkout from before run-scoping that is **still
registered to a branch** at the legacy path: it may hold unpushed work, it is not run-scoped so it
cannot be proven to belong to anybody, and starting a sibling beside it would leave two live
checkouts of one branch. (Registered *to a branch* — a legacy checkout left detached carries no
branch in the registry and does not stop this.) `start-branch` stops with `legacy-worktree-registered` and neither removes nor writes into
it. Push or preserve its work, `git worktree remove` it, then re-run — steps 1, 2 and 4 above. An
unregistered leftover directory at the legacy path blocks nothing, because the sibling is a
different directory.

**"Not registered" and "the registry could not be read" are different answers, and only one of them
is a clearance.** That question is answered by `git worktree list`, so how it is read decides
whether the paragraph above means anything. The read is NUL-delimited (`--porcelain -z`) because a
worktree path may legally contain spaces and, on POSIX, newline bytes — splitting the stream on
lines turns one such path into two fragments, and the surviving fragment names a directory nobody
owns. Every record is then required to be coherent before any of it is believed: a nonzero exit, a
stream that stops mid-record, a repeated path, a repeated field, a field this parser does not know,
an attribute with no record, a checkout with no HEAD or no branch/detached/bare state, and any
bare-plus-checked-out or branch-plus-detached contradiction are all refused as exit `3`.

The failure that motivates it is the quiet one: the read used to ignore its exit code, so git being
unable to describe its own worktrees returned exactly the same answer as a repository with nothing
checked out anywhere — and that answer is the one that lets a run write into a directory another
run already owns.

**A fresh worktree does not have the files git never tracked.** Everything gitignored — environment
files, secrets, credentials, local settings — is simply absent, and the failure it produces is
confusing rather than obvious: the tool starts normally and then dies on a variable it has never had
trouble with, in a tree that looks identical to the one that works. `start_branch` says so in its
answer; copying them across is still yours to do.

## `transition` — and why the read-back covers the board

**Add and remove in the same invocation.** Two calls leave a window in which the issue carries two
states, and any run reading the board during that window sees an ambiguous item. Then re-read: a
two-state item poisons every query that touches either state, and it has happened in live use.

The label half has always been verified. **The board half is verified by the same read**, because it
is the half with no other feedback loop — a wrong label is caught by the very next `list_state`, a
wrong claim by the next `verify_claim`, but a column nobody looks at simply stays wrong forever, and
the run that skipped it sees nothing. `transition` reads both, and where they disagree it re-sets the
column with the ids it already resolved and reads again; a mirror that still will not land exits `1`
rather than reporting success.

Matching a state to a column is by option **name OR description**, not description alone. Real boards
do not label that last column consistently: one observed board describes `Analysis`…`Blocked` with
their exact `status:*` labels and then describes `Done` as `closed`, because that column also tracks
the tracker's own closed flag. A verifier demanding `status:done` there would fail on the one
transition that matters most and send a run chasing a mirror that had worked.

## Setup

`SCRIPT ensure-states` creates the six state labels. `gh` refuses to attach a label that does not
exist, and the error arrives at issue creation — the analyst's last step, after all the analysis is
done. That is why `create` creates every label first and attaches second: the domain names its own
priority scale and rule book, so no setup script can have created them in advance. Label creation is
idempotent, so it costs one call and removes the failure mode entirely.

Attribution labels are created on demand by `claim` and `create`, so the set stays exactly as wide as
the runtimes actually in use. Labels are what make attribution *queryable*:
`gh issue list --label "dev:codex"` answers "what is that runtime holding right now" in one call,
where parsing prose for the same answer breaks on any rewording.

**Adding `done` to a repository that already ran without it** leaves closed issues still wearing the
state they were in when someone closed them. They keep showing up in `review` queries and, on a
board, keep sitting in the column they never left. Repair once, one pass per state — deliberately
plainer than a single clever query, because this runs against real history and you want to be able
to read it before you trust it:

```bash
for s in analysis ready in-progress review blocked; do
  gh issue list --state closed --label "status:$s" --json number --jq '.[].number' |
  while read -r n; do
    gh issue edit "$n" --add-label "status:done" --remove-label "status:$s"
  done
done
```

An issue carrying two state labels is handled by the pass for each of them; an issue already on
`status:done` is matched by none of them and left alone.

## Credentials

`gh auth status` may report a token stored in a system keyring that a sandboxed runtime cannot read;
there, use `GH_TOKEN` instead. **Verify with `gh issue list` before relying on the workflow** — an
analyst that cannot file its issue has done the work and lost it.

`gh issue` needs no `project` scope. The board mirroring below does — prove it once with
`gh project list --owner <owner>`, or just run `SCRIPT config` and then any `transition`, which
reports a missing scope as a skip rather than failing.

---

## Keeping a board in sync

**A GitHub Project (v2) board's `Status` field does not follow your labels.** Nothing connects the
two: project items are references, so title, state, labels and assignees are always live, but a
custom field lives on the project item and no label touches it. Left alone, a board shows whatever
someone set by hand the day they set it.

**Whether to mirror at all is read from the configuration, not guessed.** The `Project board` row
names `owner/number` or `none`. `none` means no board anywhere and every board step becomes a no-op.

**The mirror is part of `transition` and it runs FIRST**, before the label edit, best-effort and
without pre-checking board membership — an issue not on the board is a quiet skip, not a
precondition. The fragile, easily-skipped half runs before anything can short-circuit it; the
reliable one-call label edit follows. This trades one failure for a rarer one: the label stays the
store queries read, so a run that dies between the board write and the label edit leaves the board
ahead of a still-stale label — far less likely than the board drift that board-last invited, which is
the incident this order exists to prevent.

### Drift a previous run left behind

**The mirror only fires on a `transition` you make — it does not repair drift from before you got
there.** Drift from your OWN transitions is now caught inside the operation by its read-back. What
remains uncovered is what an earlier run left, and there is no daemon reconciling the two by design
(see *Why a mirror is needed at all*), which means staleness is permanent unless somebody looks.

Seen live: five items sitting on `Ready` days after their labels had moved to `in-progress` or
`done`, because whatever run transitioned them either predates this section or hit the missing-scope
fallback — and nothing since then ever looked back.

`SCRIPT audit-board` is that look: one paginated pass comparing every card's column against its own
`status:*` label, `--fix` to repair what it finds. On 2026-07-25 that pass over 83 cards found
exactly the two the run itself had left stale, which is also how you learn the problem was yours and
not systemic. **A zero-card result is reported as a failed read, not a clean board** — a configured
board is never empty, and reporting an empty read as a pass would reproduce the exact failure this
whole file exists to remove.

The answer carries three lists and **an empty `drift` is not the whole verdict**. `drift` is a card
whose column disagrees with its label; `missing_column` is a card with a label and no column; and
`unread_labels` is a card the pass **did not finish reading** — the labels connection reported more
labels than it returned, and the workflow state is a label, so its `status:*` may be outside the
window. Those cards are compared against nothing and `--fix` leaves them alone, because repairing a
card whose state you have not read is writing a state you guessed. Treat a non-empty `unread_labels`
the way you treat a zero-card result: the pass ran and did not conclude about those cards. It is the
same rule as the paragraph above, one field along.

### Why a mirror is needed at all

**The constraints below were verified against the GitHub API in July 2026** — they are product
limitations, not laws, so re-check them before designing around one.

**Board columns cannot be labels.** In `BOARD_LAYOUT` the columns come from a **single-select**
field; `Labels` is multi-value, and a card cannot sit in two columns, so GitHub does not offer it.
The option is simply absent — you are not failing to find it. `Group by → Labels` exists only in
`TABLE_LAYOUT`.

**View configuration is UI-only.** The API exposes no view mutation — only `createProjectV2Field`,
`updateProjectV2Field` and `updateProjectV2ItemFieldValue`. No agent can set a layout or a grouping
for you; a human has to click it once.

So a board needs the built-in `Status` field, reshaped to mirror your states with
`updateProjectV2Field` — one option per state, each option's description naming the label it mirrors.
That reshaping is the one board step no tool performs, because it is the one a human clicks.

### Quiet failure modes these operations already handle

Worth knowing, because they are invisible when they happen and you will meet them on another board:

- For an **organisation-owned** project, `user(login:)` returns null rather than erroring, so a
  mirror written against it silently never fires. The board lookup tries `user` then
  `organization`.
- `items(first:100)` stops finding issues once the board passes a hundred items. Every board query
  paginates.
- Board field and option ids are resolved **once and cached for a day**. Per-transition discovery is
  the overhead that tempts a run to skip the mirror, so the cache exists to remove the temptation,
  not to save API calls. `--no-cache` re-resolves.

**If your agents cannot hold the `project` scope**, the same mirroring can run server-side in a
repository Action on `issues.labeled`/`unlabeled` — it needs a PAT stored as a secret, because the
automatic `GITHUB_TOKEN` cannot write Projects v2. One or the other, never both: two mirrors is two
things to debug when the board lags.

**Whatever you do, the labels stay authoritative.** They are what agents read and write; the board
is a view. Invert that and every agent needs the `project` scope and the workflow's transport has to
be rewritten — a board that is state costs a redesign, a board that is a view costs nothing.

**GitHub now renders agent activity of its own**, and it is a third view, not a second state. When a
coding agent is assigned to an issue, its session shows under the assignee with its own live status —
queued, working, waiting for review, completed. That reports what a runtime said about itself, not
what the state machine says, and the two legitimately disagree: a session can read as *completed*
while its issue is correctly still `status:in-progress`, because the run ended and the work did not.
Same rule as the board — read it, do not trust it, and never move a label to make it agree.

---

## CI, merge and delivery — the agent's half

CI interpretation and merge are not scripted. Releasing the exact reviewed draft to CI is mechanical
and is the `release_ci` operation above; deciding that the configured review evidence is sufficient
remains with the agent because Estigia cannot observe whether judges ran or were blind.

**Before `ci_status`, call `release_ci` with the latest reviewed receipt.** A compatible repository
starts PR CI on `ready_for_review` and not on topic push, open, synchronize, or reopen; Estigia's own
`.github/workflows/ci.yml` is the example. `publish_review` makes a reused PR draft and reads that back
before push, or creates a new PR with `--draft`; judges can therefore run before ordinary cooperative
CI. `release_ci` replays the live claim, globally latest receipt, draft PR and coherent target, then
marks ready and reads the result back.

GitHub cannot atomically say "mark ready only if head/base still equal these values". A collaborator
or repository workflow acting outside Estigia can mark ready, push, start another workflow, or forge
comments, and a repository may configure additional triggers Estigia does not parse. This is a
cooperative ordering contract, not a security boundary or malicious-collaborator authentication.
Estigia does not inspect arbitrary consumer YAML with weak substring checks and cannot prove judges
ran, were independent, or were blind.

**`ci_status`.** Capture `headRefOid` and `baseRefOid`; run `gh pr checks <pr> --watch --fail-fast`,
then separately read `gh pr checks <pr> --json name,workflow,state,bucket` (`--watch` and `--json` are
separate invocations). Build the expected set as the union of host-required names and every
applicable repository-required lane. For the host set, `gh pr checks <pr> --required --json name`
returning exactly `no required checks reported on the '<branch>' branch` means the empty set; every
other command error fails closed. When repository policy names workflow job ids rather than visible
checks, resolve each id through `jobs.<id>.name` in the workflow file at the captured head SHA before
comparing; never compare ids directly with `gh pr checks.name`. Each expected visible name must be
present exactly once with `bucket=pass`; `skipping`, `cancel`, a missing name or an unexpected
duplicate is not green. Re-read both SHAs afterwards; if either changed, discard review and CI.

**`merge`.** Require `reviewed head/base == CI head/base == current headRefOid/baseRefOid`, then run
`gh pr merge <pr> --merge --match-head-commit <head-sha>`. Never use `--admin` or `--delete-branch`:
deletion can fail after a successful API merge in a multi-worktree checkout and make the result look
retryable. If a merge queue owns delivery, first prove its configured method preserves merge commits
or stop. Poll `gh pr view <pr> --json state,mergedAt,mergeCommit` until merged and take
`.mergeCommit.oid` as the delivered SHA; fetch it and verify it has exactly two parents in order:
reviewed base, then reviewed head. A mismatch means the base raced or the host used another strategy:
leave the issue in `review` and invoke the repository's fix-forward/revert policy. If
exact-delivered-SHA CI is required, locate every required workflow with
`gh run list --commit <merge-sha> --workflow <workflow> --json databaseId,headSha,status,conclusion`,
wait with `gh run watch <id> --exit-status`, then verify
`gh run view <id> --json headSha,status,conclusion,jobs`: the head must equal `<merge-sha>`, the run
and every applicable required job must be `completed/success`, never `skipped`.

**Run `SCRIPT verify-claim` again immediately before merging, before publishing tags, and before
closing.** Merge, version publication and close are separate irreversible boundaries, and a long
quiet phase between them cannot be allowed to bypass the renewal.

### Version tags

**`publish_version`.** After every required delivered-SHA gate, renew the claim BEFORE the first
publication write, then detect each component whose declared version changed from `<old>` to `<new>`.
Enumerate `git ls-remote --tags origin` — never local tags — to derive the component's established
naming convention and previous component tag. Require exactly one convention; if history is empty,
require an unambiguous single-product/component classification before using `v<new>` or
`<component>/v<new>`.

**A tag carries what a human wrote about the version.** Before creating it, extract the version's
changelog entry:

```bash
SCRIPT changelog-notes --version <new> --file <component-changelog> --out <notes-file>
```

It **fails closed** when the version has no entry, or has a heading with nothing under it. That is
not an obstacle to route around: the entry is part of what "delivered" means, and a tag is immutable,
so notes improvised at tag time are permanent. Write the entry first.

The extraction anchors on the version OPENING the heading, which matters more than it sounds. Seen
live: an entry headed `### 2026-07-25 — (sin bump de versión) … (sigue en v6.9.8)` — whose entire
point is that 6.9.8 did *not* ship in it — was matched for 6.9.8 ahead of the genuine entry by a
looser pattern. That would have produced a tag whose notes describe a different change and disclaim
the version they are named after.

For `<tag>`, inspect both local and remote direct plus peeled refs. A valid annotated tag has a tag
object and a `refs/tags/<tag>^{}` target equal to `<merge-sha>`; a lightweight tag, another target or
ambiguous state blocks without rewriting anything. If the remote tag is absent, reuse a matching
annotated local tag or create it with `git tag -a "<tag>" -F "<notes-file>" "<merge-sha>"`, then
attempt `git push origin "refs/tags/<tag>"`. After ANY push result — success, rejection or timeout —
re-read the remote: a conclusive peeled target equal to `<merge-sha>` is idempotent success, another
target is conflict, and no conclusive read fails closed.

When GitHub Releases are required, first reverify the remote peeled target, then query
`gh release view "<tag>" --json tagName,isDraft,isPrerelease`. A matching non-draft Release with the
SemVer-derived prerelease flag is idempotent success; incompatible metadata blocks. Only a confirmed
not-found result permits `gh release create "<tag>" --verify-tag --notes-file "<notes-file>"` — the
same notes the tag carries (add `--prerelease` for a prerelease).

**Do not use `--generate-notes`.** It substitutes a list of commit subjects for the entry a human
wrote, which reads like documentation without being any — and it does so silently, so a component
whose changelog was never updated still gets a Release that looks complete. The `changelog-notes`
failure is the signal you want there. After ANY create result, re-read the Release JSON and the remote peeled
tag; both must match the expected metadata and `<merge-sha>`. Auth, network or API ambiguity fails
closed. Tag or Release failure leaves the issue in `review` even though the merge already exists.

A Git tag and a GitHub Release are different artifacts: the tag is mandatory; the Release is created
only when the repository already publishes them for that component. Never move, overwrite or
force-push an existing version tag — a tag that already points elsewhere is a delivery blocker, not
permission to rewrite release history.

### Closing

`close` moves the state to `done` **first** — `SCRIPT transition --issue <n> --to done` — then posts
the closing note with `gh issue comment <n> --body-file <file>`, then a bare `gh issue close <n>`.
`gh issue close` has NO file variant (`-c/--comment` is inline-only, confirmed against
`gh issue close --help`), so the note goes through the file-based comment path and the close carries
no body at all.

**Run the transition even when GitHub already closed the issue.** Under `gh issue develop` the merge
auto-closes it (see *Auto-close, and why it does not end the work*), and the temptation is to skip
the transition because the issue is already closed. That is precisely the failure: the auto-close
moved neither the label nor the board, so skipping leaves a CLOSED issue wearing `status:review` and
a card parked in the wrong column. `transition` is idempotent, so running it against an
already-closed issue costs one call. The final `gh issue close` is then a no-op and may report the
issue as already closed — that is success, not an error.

That is the **final-delivery** path. An intermediate slice of a retained issue must not borrow it:
record the workflow state that should survive before merging, renew with
`verify-claim --allow-closed-by-pr <pr>` after the linked PR closes the issue, reopen it with
`gh issue reopen <n>`, then transition from `review` into that recorded retained state and read it
back. Never transition an intermediate slice to `done`. A different closer, truncated connection,
or malformed pagination refuses the renewal, so none of those cases grants permission to reopen.

Two calls means a partial-failure case: if the comment lands but the close errors, the issue is left
open with its closing note already posted — retry only the bare `gh issue close <n>`, never re-post
the note.

Carry your run identity in the closing comment and state what was actually verified: review verdict,
CI run or checks, measured numbers, tests run, PR, **the delivering commit SHA**, and every version
tag or Release published. Never write just "done"; the state already says that, and the comment
exists to show what earned it.

**A shared GitHub account cannot manufacture independent approval.** The workflow's reviewer is a
fresh reasoning context, but GitHub sees the authenticated account, not that context. When the same
account authored the PR, record the reviewer verdict and evidence in the PR or issue with
`Reviewer-Run: <run-id>`, `Reviewed-Head: <full-head-sha>` and `Reviewed-Base: <full-base-sha>`; do
not claim a native `APPROVED` review that GitHub refused. Re-read both PR SHAs after the verdict —
matching prose without that race check is still stale evidence. This comment proves the workflow
review only; if repository protection requires a native approval, it does NOT substitute for one and
delivery stays blocked until a distinct identity supplies it.
