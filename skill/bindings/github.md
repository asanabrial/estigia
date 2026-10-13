# Binding: GitHub Issues

How this workflow's operations are performed against GitHub, and which of its assumptions GitHub
satisfies natively rather than by convention. `SKILL.md` describes WHAT happens; this file is the
only place that says HOW.

Read this alongside `SKILL.md` when the operator configuration names `github` as the tracker.

## The mechanical half runs as a tool

**Every reversible operation below is executed by Estigia's tools, not by an agent composing
`gh` calls**: a run once moved labels correctly through the whole state machine and executed a
mechanical step zero times in a whole session (`SKILL.md`), and prose cannot fix a run that does not
execute prose. So the steps that are **mechanical and verifiable** — label swaps, the project
projection and its read-back, claim adjudication, the renewal, branch and worktree creation, draft PR
publication, CI release, the closing-keyword scan — are code. The steps that need **judgement** — what
is worth analysing, whether a blocker is discharged, whether a diff passes review — are not, and stay
in the prose where they belong.

Each operation is one of Estigia's MCP tools, called by name; a `--flag` written here is the tool's argument of the same name.

There is no interpreter and no script on the path: the operations answer in one place.

The tools read the SAME operator configuration this prose does — the block between the config
markers in `SKILL.md`, overridden by `estigia.local.md` — so it never becomes a third source of
truth. The `config` operation prints what it resolved; run it once if you are unsure what the
worktree row currently says.

**Exit codes carry the distinction this workflow calls *a failed read is not a failed answer* —
stated as a decision gate in `SKILL.md` and evidenced in `references/safety-incidents.md`:**

| Exit | Meaning | What you do |
|---|---|---|
| `0` | the operation completed **and its read-back verified** | continue |
| `1` | **STOP** — a check answered stop (lost race, stand-down, wrong state, closed issue) | follow the JSON's `action`; do not retry |
| `2` | usage or configuration error; nothing was attempted | fix the invocation the message names; where the JSON carries an `action`, follow it — unless the reason is one the malformed-argument table answers, which takes precedence |
| `3` | the **READ itself** failed — the control surface answered NOTHING | fail closed: write nothing, retry the read. Never clearance, never a stand-down |
| `4` | internal error — a defect in the tool | state is **UNKNOWN**; re-read the issue before doing anything |
| `5` | a **WRITE** failed | **re-read** to establish what landed, then decide. Do not retry blindly |

Exit `3` is never a stand-down and never clearance: treating a timeout as either halts every run or
lets a run write deaf. Exit `5` is not retried blindly, because a write may have landed before the
failure surfaced. Exit `2` is not a lesser `1`: no authority changed hands and nothing was attempted,
so fix the configuration and retry; reporting it as `1` makes a run abandon a claim it still holds.

**Every refusal also carries a `class`**, one word that says what to do next and that the exit code
alone cannot: `unavailable` (the control surface could not be read) and `unknown` (a write may have
landed) mean read again and never treat the answer as a state; `stale` (a receipt, head or authority
moved) means re-read before deciding; `refused` (the rule says no) means stop, because repeating the
command repeats the answer. Each refusal names its class.

**What is deliberately NOT scripted**: `merge`, `publish_version`, `close`, and the interpretation of
`review_status` / `ci_status`. Those write irreversibly to the remote or require a verdict, and a
defect in a script must not be able to merge, tag or close anything. The agent performs them itself,
by the prose in `github/delivery.md`, after verifying SHAs.

**Where `gh` cannot be installed**, the REST API with a token does everything below at the cost of
longer invocations; the installer checks for `gh` and tells you rather than fetching it.

## What GitHub provides

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

GitHub has a closed state of its own, independent of labels — but `done` is still carried as a
sixth label, and the two are set together. Relying on closed alone would leave the finished issue
wearing `status:review`, so every query for work awaiting verification would keep returning it.

## The claim hazard — why the assignee cannot adjudicate

The why: a shared account makes the assignee blind to a collision.

Verification here does not read the assignee. It reads the **timeline in the local store**, where
`claim` records its marker and adjudicates in the same transaction, which orders the writers of one
store; the earliest live claim wins. A claim an older binary or a person posted as a GitHub comment
is not read after the issue's first touch: only a person's stand-down or adjudication comes in
through the sync, nothing that hands the issue to someone, and it can only stop a run.

### Control markers

Parsing prose for the same answer is fragile — any rewording breaks it — so every control marker
these operations record carries a machine-readable trailer. `claim`, `reclaim`, `unassign`, `heartbeat` and `start-branch`
record theirs in the local store, as do the review operations; none is posted to GitHub as a comment. The
shape below is the one the store keeps and the one an older binary posted:

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

The store keeps this text as an append-only row; an older binary's comment is read through the import
under the same rules and is inert once edited. A control message must both **name the run-id AND
instruct**: a heartbeat that mentions your run-id in passing ("waiting on `<run-id>`'s measurement
phase") instructs you to do nothing, and reading it as a stand-down would have you abandon work nobody
asked you to drop.

Only `viewerDidAuthor=true` comments are control input. Reclaims must match the stale prior holder
unless marked `forced=true`; horizonless acquisitions expire four hours after trusted activity.

`heartbeat` runs the renewal **before** it records, and refuses to record when the renewal says stop,
so a loser's next heartbeat exits `1` instead of writing. The renewal does not prevent losing the race;
it caps the cost of having lost it at one renewal interval.

## State names

The workflow's states are `analysis`, `ready`, `in-progress`, `review`, `blocked` and `done`. Here
each one is stored as the label **`status:<name>`** — the prefix is this binding's convention, not the
workflow's.

## Operations

`<n>` is the issue number throughout. `SCRIPT` abbreviates *the Estigia tool for this operation*:
each row names the operation, and the tool's schema carries its arguments, spelled the same.

Every operation's full guarantee beyond the row is in
[`github/operations-guarantees.md`](github/operations-guarantees.md).

Every ownership write uses a lowercase 32-hex `--operation-id`, reused unchanged on retry. Review handoff and verdict IDs are derived from their immutable inputs, so an unchanged retry names the same event. `reclaim` and `unassign` first return the exact target without writing; repeat with `--target-operation` to authorize only that epoch. Historical legacy ownership remains readable, but broad legacy controls cannot mutate a modern epoch. `review-handoff`, `review-verdict` and `review-finding` are neither release nor activity markers: the compound handoff releases through its separate exact `unassign`, and neither event extends a claim.

| Operation | Command |
|---|---|
| `ensure_states` | `SCRIPT ensure-states` |
| `flush` | `SCRIPT flush` |
| `create` | `SCRIPT create` |
| `list_state` | `SCRIPT list-state` |
| `claim` | `SCRIPT claim` — see *Claim* in github/operations.md |
| `verify_claim` | `SCRIPT verify-claim` |
| `transition` | `SCRIPT transition` |
| `comment` | `SCRIPT comment` |
| `blocked_until` | `SCRIPT blocked-until` |
| `task` | `SCRIPT task` |
| `tasks` | `SCRIPT tasks` |
| `heartbeat` | `SCRIPT heartbeat` |
| branch + worktree | `SCRIPT start-branch` |
| `judge_workspaces` | `SCRIPT judge-workspaces` |
| `publish_review` | `SCRIPT publish-review` — see *Publishing a review target* in github/operations.md |
| `republish_review` | `SCRIPT republish-review` |
| `handoff_review` | `SCRIPT handoff-review` |
| `review_finding` | `SCRIPT review-finding` — see *Recording a finding* in github/operations.md |
| `escalate_review` | `SCRIPT escalate-review` — see *Escalating a review* in github/operations.md |
| `review_disposition` | `SCRIPT review-disposition` — see *Recording a disposition* in github/operations.md |
| `launch_review_input` | `SCRIPT launch-review-input` — see *Capturing a review input* in github/operations.md |
| `collect_review_input` | `SCRIPT collect-review-input` — see *Capturing a review input* in github/operations.md |
| `review_computed_verdict` | `SCRIPT review-computed-verdict` — see *Recording a computed verdict* in github/operations.md |
| `record_spend` | `SCRIPT spend` |
| `review_status` | `SCRIPT review-status` — answers `lane` and `corrections` beside `next` |
| `review_read` | `SCRIPT review-read` |
| `target` | `SCRIPT target` |
| `release_ci` | `SCRIPT release-ci` — see *Recording a computed verdict* in github/operations.md |
| `review_tier` | `SCRIPT review-tier` — see *Deriving the review tier* in github/operations.md |
| `check closing keywords` | `SCRIPT check-closing-keywords` |
| `unassign` | `SCRIPT unassign` — see *Releasing an issue* in github/operations.md |
| assisted checkout | `SCRIPT assist` |
| draft pull request | `SCRIPT draft-pr` |
| `changelog-notes` | `SCRIPT changelog-notes --version <x.y.z> --file <changelog> [--out <f>]` |

Not scripted, performed by the agent after verifying SHAs (`github/delivery.md`): `ci_status`, `merge`,
`publish_version`, `close`. The review verdict is computed from the finding ledger and recorded by
`review_computed_verdict`; `last_activity` is the issue's timeline in the local store.

### Operations no tool exposes

Each name below has no tool of its own; the call on the right answers it.

- `reclaim` → `claim` with `takeover`, plus `target_operation`, and `force` with `reason_file`.
- `list_identities` → `list_state` with `identities`.
- `review_findings`, `review_transaction`, `review_payload_membership`, `inspect_candidate`, `computed_verdict`, `spend_summary` → `review_read` with `view` `findings`, `transaction`, `membership`, `inspect`, `verdict`, `spend`.
- `repair_rounds`, `publication_lane` → `review_status`, which answers `lane` and `corrections`.
- `review_tier_for_repair` → `review_tier` with `parent_head`.
- `expected_target`, `base_movement` → `target`, which classifies base movement when `recorded_base` is given.
- `review_refutation`, `review_validator_result` and the lens report → recorded only by `collect_review_input`, after the binary ran the reviewer; `review_verdict` is retired, and the verdict is computed by `record_computed_verdict`.

### Markdown bodies always go through a file

The tools pass every `gh` argument as a list with no shell and every markdown body through a temp file.
Where you still compose a `gh` call by hand (`close`, `merge`), write the body to a file and use
`--body-file`, with no exception for "this one's short": PowerShell corrupts an inline body.

### Refusals

A refusal from these operations says nothing about the tracker: the call could not be made, so
nothing was attempted and nothing was written, and repeating it unchanged repeats the answer. The
per-code meaning and remedy of every refusal this binding raises moved to
[`github/refusals.md`](github/refusals.md) — read it when a refusal you are reading is not
self-explanatory.

## Auto-close, and why it does not end the work

GitHub can close an issue on merge without the workflow's `close` ever running: no transition to
`done`, labels frozen wherever they were.

**Two different things cause it, and only one of them is a defect.** They produce the identical
symptom — a non-empty `closedByPullRequestsReferences` — so the symptom alone must never be read as
"somebody wrote a keyword":

| Cause | Status | Remedy |
|---|---|---|
| A **closing keyword** in the PR body or a commit message | forbidden — the text is yours | remove it, use `Refs #<n>`, re-check |
| A **branch link** created by `gh issue develop` | **expected under this binding** | none — no edit removes it |

**So the rule is not "prevent the auto-close". It is: `transition` to `done` after the merge
REGARDLESS of whether GitHub already closed the issue.** Closing an issue emits `issues.closed`,
which is not a label event: it moves no `status:*` label. The auto-close is the tracker's
bookkeeping, never the workflow's `close`, and the state machine does not know it happened. Run the transition afterwards and the end state is correct; skip it because
"the issue is already closed" and the issue stays CLOSED wearing its old `status:*` label.

`check-closing-keywords` reports the cause, not just the symptom: a keyword is a hard stop (exit
`1`), a branch link is reported with the follow-up it mandates and does not block. `publish-review`'s
own answer no longer carries that follow-up — it stays on the `published` marker and in
`review_read` with `view: transaction`.

Branch, worktree and linked-issue mechanics, and the rationale behind the closing-keyword scan above, moved to `github/delivery.md` — read it when the dev reaches publication, CI, merge, tags or closing.

Recovering a stuck worktree or branch lock, migrating an upgraded worktree template, and the quiet failures those operations already handle moved to `github/recovery.md` — read it when a lock, a migration, or a quiet failure fired.

## `transition` — and why the read-back covers the projects

**Add and remove in the same invocation.** Two calls leave a window in which the issue carries two
states, and any run reading the project during that window sees an ambiguous item. Then re-read: a
two-state item poisons every query that touches either state, and it has happened in live use.

The label half has always been verified; **the project half is verified by the same read**, because
it has no other feedback loop. `transition` reads both, and where they disagree it projects again and
reads; a projection that still will not land exits `1` rather than reporting success. Matching a state
to a column is by option **name OR description**, not description alone.

Creating the six state labels and repairing history from before `done` existed moved to `github/setup.md` — read it when `estigia setup` or `sync` is running.

## Credentials

`gh auth status` may report a token stored in a system keyring that a sandboxed runtime cannot read;
there, use `GH_TOKEN` instead. **Verify with `gh issue list` before relying on the workflow** — an
analyst that cannot file its issue has done the work and lost it.

`gh issue` needs no `project` scope. Project mirroring does — prove it once with
`gh project list --owner <owner>`, or just run any `transition`, which reports a missing scope as a
skip rather than failing.

CI interpretation, merge, version tags and closing moved to `github/delivery.md` — read it when the dev reaches publication, CI, merge, tags or closing.

## Repository host

GitHub is also the default **repository host**: pull requests, tags, and the receipt-bound review
half bind to that row, which is separate from Tracker so a future non-GitHub tracker can still
publish against GitHub.
