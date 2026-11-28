# Binding: GitHub Issues

How this workflow's operations are performed against GitHub, and which of its assumptions GitHub
satisfies natively. `SKILL.md` describes WHAT happens; this file says HOW. Read it when the operator
configuration names `github` as the tracker.

## The mechanical half runs as a tool

**Every reversible operation below is executed by Estigia's tools, not by an agent composing `gh`
calls.** What needs judgement stays in the prose. Each operation is a tool or CLI command called by
name, and a `--flag` here is the argument of the same name. The tools read the same operator
configuration this prose does, so no third source of truth exists: `estigia config list` prints what
they resolved.

**Exit codes carry *a failed read is not a failed answer*:**

| Exit | What happened, and what you do |
|---|---|
| `0` | completed and read back — continue |
| `1` | **STOP**: a check answered stop — follow the JSON's `action`; do not retry |
| `2` | usage or configuration error; nothing was attempted — fix the invocation named and retry |
| `3` | the **READ** failed — write nothing, retry the read; never clearance |
| `4` | internal error — the state is **UNKNOWN**; re-read the issue first |
| `5` | a **WRITE** failed — **re-read** what landed, then decide; never retry blindly |

Exit `2` is not a lesser `1`: no authority changed hands. Exit `3` is never clearance, and exit `5`
may have landed before the failure surfaced. **A refusal's `class` says what to do**: `unavailable`
and `unknown` mean read again and never treat the answer as a state; `stale` means re-read before
deciding; `refused` means stop, because repeating the command repeats the answer.

Not scripted, and performed by the agent after verifying SHAs (`github/delivery.md`): `ci_status`,
`publish_version`, `close`, `merge`. A defect in a script must not be able to tag or close anything.

Where a sandboxed runtime cannot read the keyring, use `GH_TOKEN`; verify with `gh issue list`
before relying on the workflow.

## State names

The workflow's states are `analysis`, `ready`, `in-progress`, `review`, `blocked` and `done`. Here
each one is stored as the label **`status:<name>`** — the prefix is this binding's convention, not the
workflow's.

GitHub's own closed state is independent of labels, but `done` still rides as a sixth label set with
it: closed alone leaves a finished issue wearing `status:review`. GitHub has no single-valued state,
no server-side claim, no native priority and no trusted last activity, so exactly one `status:*` is a
discipline, claims are adjudicated in the store, priority is the label `<scale>:<value>` the domain
names, and only the store's timeline renews a claim.

## Claims

The assignee cannot adjudicate a shared account, so verification reads the **timeline in the local
store**, where `claim` records and adjudicates in one transaction: the earliest live claim wins. An
acquisition expires at its horizon or four hours after trusted activity, whichever is later.

## Operations

`<n>` is the issue number. `SCRIPT` is *the Estigia tool for this operation*: the tool's schema
spells its arguments the same way. Every operation's full guarantee is in
[`github/operations-guarantees.md`](github/operations-guarantees.md).

Every ownership write uses a lowercase 32-hex `--operation-id`, reused on retry. `reclaim` and
`unassign` first return the exact target; repeat with `--target-operation` to authorize that epoch. Broad legacy controls cannot mutate a modern epoch.

| Operation | Command |
|---|---|
| `flush` | `estigia flush`; `--status` lists the queue |
| `create` | `SCRIPT create` |
| `list_state` | `SCRIPT list-state` |
| `claim` | `SCRIPT claim` — see *Claim* in github/operations.md |
| `handoff` | `estigia handoff <issue> --to <runtime>` |
| `verify_claim` | `SCRIPT verify-claim` |
| `transition` | `SCRIPT transition` |
| `comment` | `SCRIPT comment` |
| `task` | `SCRIPT task` |
| `tasks` | `SCRIPT tasks` |
| `judge_workspaces` | `estigia judge-workspaces` |
| `publish_review` | `SCRIPT publish-review` — see *Publishing a review target* in github/operations.md |
| `republish_review` | `SCRIPT republish-review` |
| `handoff_review` | `SCRIPT handoff-review` |
| `review_finding` | `SCRIPT review-finding` — see *Recording a finding* in github/operations.md |
| `escalate_review` | `SCRIPT escalate-review` |
| `review_disposition` | `SCRIPT review-disposition` — see *Recording a disposition* in github/operations.md |
| `launch_review_input` | the `launch-review-input` operation is `collect_review_input` with `launch: true` |
| `collect_review_input` | `SCRIPT collect-review-input` |
| `review_computed_verdict` | the `review-computed-verdict` operation is `deliver` with `until: verdict`; it keeps its name |
| `record_spend` | passed as `spend` to `release` or `deliver` |
| `review_status` | `SCRIPT review-status` |
| `review_read` | `SCRIPT review-read` |
| `target` | the `target` view of `review_read` answers it |
| `release_ci` | the `release-ci` operation is `deliver` with `until: ci` |
| `deliver` | `deliver`, which calls the merge gate itself |
| `run_checks` | the `run-checks` operation is `collect_review_input` with `checks: true` |
| `review_tier` | the `review-tier` operation is `review_read` with `view: tier` — see *Deriving the review tier* in github/operations.md |
| `unassign` | `SCRIPT unassign` — see *Releasing an issue* in github/operations.md |
| assisted checkout | `SCRIPT assist` |
| pull request review | `pr_review`, whose `action` names one step of the review-only claim |
| draft pull request | `SCRIPT draft-pr` |
| edit pull request metadata | `SCRIPT edit-pr` |
| ready pull request | `SCRIPT ready-pr` |
| review stages | `SCRIPT review-stages` |
| review thread | `SCRIPT review-thread` |

The verdict is computed from the finding ledger and recorded by `review_computed_verdict` (`deliver`
with `until` `verdict`); `last_activity` is the issue's timeline in the local store.

### Operations no tool exposes

Each name below has no tool of its own; the call on the right answers it, and its kebab spelling
remains for the CLI (`start-branch`, `blocked-until`, `ensure-states`).

- `start_branch` → `claim` with `branch`, `base`, `worktree_root` and `no_link`
- `heartbeat` → `comment` with `expect_state`
- `blocked_until` → `transition` with `to` `blocked`
- `ensure_states` → `create`, or any label write
- `reclaim` → `claim` with `takeover` and `target_operation`

### Markdown bodies always go through a file

Every `gh` argument is passed as a list with no shell, and every markdown body through a temp file.
Where you compose a `gh` call by hand (`close`, `merge`), use `--body-file`: PowerShell corrupts an
inline body.

### Refusals

A refusal means the call could not be made, so nothing was written, and repeating it unchanged
repeats the answer. Per-code meaning and remedy are in [`github/refusals.md`](github/refusals.md).

## Auto-close, and why it does not end the work

GitHub can close an issue on merge without the workflow's `close` ever running: no transition to
`done`, labels frozen. Two causes produce the identical symptom; only one is a defect. A **closing
keyword** in the PR body or a commit message is forbidden — remove it, use `Refs #<n>`, re-check. A
**branch link** created by `gh issue develop` is expected under this binding, and no edit removes it.

**So `transition` to `done` AFTER the merge, REGARDLESS of whether GitHub already closed the issue:**
closing is not a label event, so skipping the transition because "it is already closed" leaves it
CLOSED wearing its old label. `publish-review` reports the cause: a keyword is a hard stop, a branch
link carries its mandated follow-up and does not block.

## `transition` — and why the read-back covers the projects

**Add and remove in the same invocation**: two calls leave a window with two states. Re-read, because
a two-state item poisons every query that touches either state. `transition` reads the label and
project halves together, projects again where they disagree, and exits `1` rather than reporting a
success that did not land.

GitHub is also the default **repository host**: pull requests, tags and the receipt-bound review half
bind to that row, separate from Tracker.
