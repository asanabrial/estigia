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
> through the entire state machine and executed a mechanical step **zero times in a whole session**.

Prose cannot fix a run that does not execute prose. So the steps that are **mechanical and
verifiable** — label swaps, the project projection and its read-back, claim adjudication, the renewal,
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

**Every refusal also carries a `class`**, one word that says what to do next and that the exit code
alone cannot: `unavailable` (the control surface could not be read) and `unknown` (a write may have
landed) mean read again and never treat the answer as a state; `stale` (a receipt, head or authority
moved) means re-read before deciding; `refused` (the rule says no) means stop, because repeating the
command repeats the answer. Each refusal names its class.

**What is deliberately NOT scripted**: `merge`, `publish_version`, `close`, and the interpretation of
`review_status` / `ci_status`. Those write irreversibly to the remote or require a verdict, and a
defect in a script must not be able to merge, tag or close anything. The agent performs them itself,
by the prose in `github/delivery.md`, after verifying SHAs.

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

`SKILL.md` warns that "parsing prose for the same answer is fragile — any rewording breaks it", and
adjudication used to depend on exactly that. So every control marker these operations record
carries a machine-readable trailer. `claim`, `reclaim`, `unassign`, `heartbeat` and `start-branch`
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

The store keeps this text as a row and the rows are append-only. A comment an older binary posted
is read through the import under the same rules (and, for a stand-down or adjudication aimed at a run,
the sync), and if it is edited it becomes inert
because GitHub no longer exposes its original bytes; corrections append a fresh operation. Reading falls back to the prose forms
(`Claimed by <run-id>`) for comments written before markers existed, and that fallback is
deliberately narrow: a control
message must both **name the run-id AND instruct**. A heartbeat that mentions your run-id in passing
("waiting on `<run-id>`'s measurement phase") instructs you to do nothing, and classifying it as a
stand-down would have you abandon work nobody asked you to drop.

Only `viewerDidAuthor=true` comments are control input. Reclaims must match the stale prior holder
unless marked `forced=true`; horizonless acquisitions expire four hours after trusted activity.

**Worked example — a real claim race, 2026-07-22**, from the issue's comment trail, which is how claims
were recorded then. Today the store orders both claims in one transaction and refuses the loser
at once; the lesson about renewal stands:

| Time (UTC) | Event |
|---|---|
| 14:40:03 | `claude-code-cb8d3f2c` writes its claim comment |
| 14:40:08 | `claude-code-d7d8a22e` writes its claim comment, 5 s later, before the collision is visible |
| 14:40:41 | adjudication comment: the earliest claim wins, `claude-code-d7d8a22e` is told to stand down |
| 14:49–15:24 | `claude-code-d7d8a22e` posts heartbeats 1, 2 and 3 and keeps building — none of its writes re-read the timeline |
| 15:24:21 | the winner delivers |
| 15:28:52 | `claude-code-d7d8a22e` finally stands down, retracting a measurement taken on work it no longer held |

Under write-only heartbeats the stand-down sat unread for 48 minutes. `heartbeat` makes that
timeline impossible: `heartbeat` runs the renewal **before** it records, and refuses to record when the renewal
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

Every operation's full guarantee beyond the command moved to
[`github/operations-guarantees.md`](github/operations-guarantees.md), which quotes each row's own cell.

Every ownership write uses a lowercase 32-hex `--operation-id`, reused unchanged on retry. Review handoff and verdict IDs are derived from their immutable inputs, so an unchanged retry names the same event. `reclaim` and `unassign` first return the exact target without writing; repeat with `--target-operation` to authorize only that epoch. Historical legacy ownership remains readable, but broad legacy controls cannot mutate a modern epoch. `review-handoff`, `review-verdict` and `review-finding` are neither release nor activity markers: the compound handoff releases through its separate exact `unassign`, and neither event extends a claim.

| Operation | Command |
|---|---|
| `ensure_states` | `SCRIPT ensure-states` |
| `flush` | `SCRIPT flush [--read-only]` |
| `create` | `SCRIPT create --identity <id> --title <t> --body-file <f> --priority <scale:value> --domain <name> --runtime <rt> --run-id <id> [--state ready\|blocked] [--blocked-until <n>=<s>[+<n>=<s>...]] [--epoch <publication> [--findings <id[+id...]>]]` |
| `list_state` | `SCRIPT list-state [--state <s>] --run-id <id> [--identities] [--limit <n>]` |
| `claim` | `SCRIPT claim --issue <n> --run-id <id> --runtime <rt> --horizon <UTC> --operation-id <32-hex> [--transition-to <s>] [--takeover [--target-operation <epoch>] [--force --reason-file <f>]]` — see *Claim* in github/operations.md |
| `reclaim` | `SCRIPT reclaim --issue <n> --run-id <id> --runtime <rt> --horizon <UTC> --operation-id <32-hex> [--state <s>] [--target-operation <epoch>] [--force --reason-file <f>]` |
| `verify_claim` | `SCRIPT verify-claim --issue <n> --run-id <id> --expect-state <s> [--allow-closed-by-pr <pr>]` |
| `transition` | `SCRIPT transition --issue <n> --to <s> [--from <s>]` |
| `comment` | `SCRIPT comment --issue <n> --body-file <f> [--run-id <id> --kind note\|blocker\|diagnosis]` |
| `blocked_until` | `SCRIPT blocked-until --issue <n> --run-id <id> (--until-issue <m> --until-state <s> \| --clear)` |
| `task` | `SCRIPT task --issue <n> --run-id <id> --id <task-id> [--text <acceptance>] [--route inline\|delegated] [--status open\|done] [--commit <hex>] [--expect-state <s>]` |
| `tasks` | `SCRIPT tasks --issue <n>` |
| `heartbeat` | `SCRIPT heartbeat --issue <n> --run-id <id> --expect-state <s> --body-file <f> [--kind heartbeat\|progress\|blocked]` |
| branch + worktree | `SCRIPT start-branch --issue <n> --branch <b> --base <base> --run-id <id>` |
| `judge_workspaces` | `SCRIPT judge-workspaces --issue <n> --run-id <id> --epoch <id> --pr <n> --head <sha> --base <sha> --digest <sha256> --count <n> [--worktree-root <p>]` |
| `publish_review` | `SCRIPT publish-review --issue <n> --branch <b> --base <base> --run-id <id> --pr-title <t> --pr-body-file <f> [--worktree <p>] [--inline-payload] [--with-review] [--topology <t>] [--topology-reason <text>] [--expected-epoch <epoch\|none>]` — see *Publishing a review target* in github/operations.md |
| `republish_review` | `SCRIPT republish-review --issue <n> --branch <b> --base <base> --run-id <id> --pr-title <t> --pr-body-file <f> [--worktree <p>] [--expect-state <s>] [--inline-payload] [--with-review] [--topology <t>] [--topology-reason <text>] [--expected-epoch <epoch\|none>]` |
| `handoff_review` | `SCRIPT handoff-review --issue <n> --run-id <id> --target-operation <epoch> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --blocker <text> --discharger <text>` |
| `review_finding` | `SCRIPT review-finding --issue <n> --run-id <id> --reviewer <id> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --id <lineage-key> --class severe\|warning\|suggestion --evidence <text> --impact <text> [--evidence-class deterministic\|inferential\|insufficient] [--causal-disposition introduced\|worsened\|pre-existing\|base-only\|unknown] [--location <path>:<line>\|<path>:<start>-<end>] [--lens <role>] [--parent <lineage-key>] [--origin introduced\|exposed]` — see *Recording a finding* in github/operations.md |
| `escalate_review` | `SCRIPT escalate-review --issue <n> --run-id <id> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --escalated-to <person> --reason <text>` — see *Escalating a review* in github/operations.md |
| `review_disposition` | `SCRIPT review-disposition --issue <n> --run-id <id> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --dispositions <json>` — see *Recording a disposition* in github/operations.md |
| `launch_review_input` | `SCRIPT launch-review-input --issue <n> --run-id <id> [--name <lens-role\|refutation-batch\|review-validator>] [--names <name,...>]` — see *Capturing a review input* in github/operations.md |
| `collect_review_input` | `SCRIPT collect-review-input --issue <n> --run-id <id> [--name <lens-role\|refutation-batch\|review-validator>] [--names <name,...>] [--wait-seconds <n>]` — see *Capturing a review input* in github/operations.md |
| `review_computed_verdict` | `SCRIPT review-computed-verdict --issue <n> --run-id <id> --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> [--worktree <p>] [--never-ran-evidence-file <f>]` — see *Recording a computed verdict* in github/operations.md |
| `record_spend` | `SCRIPT spend --issue <n> --run-id <id> --phase review\|developer\|analyst\|orchestrator --epoch <publication> --pr <n> --head <sha> --base <sha> --digest <sha256> --conditions <text> --input <n> --tokens <n> --seconds <n> --contexts <n> [--note <text>]` |
| `review_status` | `SCRIPT review-status --issue <n>` — answers `lane` and `corrections` beside `next` |
| `review_read` | `SCRIPT review-read --issue <n> --view <transaction\|findings\|verdict\|membership\|inspect\|spend> [--epoch <id>] [--id <finding-id>] [--with-text] [--candidate <path>] [--request <name-status\|numstat\|stat\|patch\|object>] [--path <p>] [--side <parent\|candidate>] [--pr <n>] [--head <sha>] [--base <sha>] [--digest <sha256>] [--phase <review\|developer\|analyst\|orchestrator>] [--worktree <p>]` |
| `target` | `SCRIPT target --base <sha> [--recorded-base <sha>] [--worktree <p>] [--native-start <f>] [--brief]` |
| `release_ci` | `SCRIPT release-ci --issue <n> --run-id <id> --epoch <id> --pr <n> --head <sha> --base <sha> --digest <sha256> [--worktree <p>]` — see *Recording a computed verdict* in github/operations.md |
| `review_tier` | `SCRIPT review-tier --base <sha> --head <sha> [--focus <risk\|resilience\|readability\|reliability>] [--worktree <p>]` — see *Deriving the review tier* in github/operations.md |
| `changelog-notes` | `SCRIPT changelog-notes --version <x.y.z> --file <changelog> [--out <f>]` |
| `check closing keywords` | `SCRIPT check-closing-keywords --issue <n>` |
| `unassign` | `SCRIPT unassign --issue <n> --runtime <rt> --run-id <id> --operation-id <32-hex> [--target-operation <epoch>] [--held-by-other] [--evidence-file <f>] [--spend <json>]` — see *Releasing an issue* in github/operations.md |
| `review_status` | *(computed from the finding ledger, recorded by `review_computed_verdict`)* |
| `ci_status` | *(agent, not scripted)* |
| `merge` | *(agent, not scripted)* |
| `publish_version` | *(agent, not scripted)* |
| `last_activity` | the issue's timeline in the local store |
| `close` | *(agent, not scripted)* |
| assisted checkout | `SCRIPT assist --issue <n> --branch <b> --base <base> --run-id <id>` |
| draft pull request | `SCRIPT draft-pr --title <t> --body-file <f> --branch <b> --worktree <p>` |

### Operations no tool exposes

Each name below has no tool of its own; the call on the right answers it.

- `list_identities` → `list_state` with `identities`.
- `review_findings`, `review_transaction`, `review_payload_membership`, `inspect_candidate`, `computed_verdict`, `spend_summary` → `review_read` with `view` `findings`, `transaction`, `membership`, `inspect`, `verdict`, `spend`.
- `repair_rounds`, `publication_lane` → `review_status`, which answers `lane` and `corrections`.
- `review_tier_for_repair` → `review_tier` with `parent_head`.
- `expected_target`, `base_movement` → `target`, which classifies base movement when `recorded_base` is given.
- `review_refutation`, `review_validator_result` and the lens report → recorded only by `collect_review_input`, after the binary ran the reviewer; `review_verdict` is retired, and the verdict is computed by `record_computed_verdict`.

### Markdown bodies always go through a file

**The tools remove this class rather than warning about it**: every subprocess call passes an
argument list with no shell, and every markdown body goes through a temp file. That is also why the
operations are answered in **one** implementation rather than a PowerShell/bash pair — writing the
fix in the language that causes the bug, twice, in two implementations that would then drift, is not
a fix. It was a script beside this contract for a while, which was one implementation in the wrong
place: the agent ran it and the gate read something else.

Where you still compose a `gh` call by hand (`close`, `merge`), write the body to a file and use
`--body-file`, with no exception for "this one's short." The rule exists because PowerShell corrupts
an inline body.

### Refusals

A refusal from these operations says nothing about the tracker: the call could not be made, so
nothing was attempted and nothing was written, and repeating it unchanged repeats the answer. The
per-code meaning and remedy of every refusal this binding raises moved to
[`github/refusals.md`](github/refusals.md) — read it when a refusal you are reading is not
self-explanatory.

## Auto-close, and why it does not end the work

GitHub can close an issue on merge without the workflow's `close` ever running: no transition to
`done`, labels frozen wherever they were. Seen live: an issue closed by its delivery commit sat
CLOSED wearing `status:ready` until an audit caught it.

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
"the issue is already closed" and you have reproduced the exact incident above.

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

The label half has always been verified. **The project half is verified by the same read**, because
it is the half with no other feedback loop — a wrong label is caught by the very next `list_state`, a
wrong claim by the next `verify_claim`, but a column nobody looks at simply stays wrong forever, and
the run that skipped it sees nothing. `transition` reads both, and where they disagree it projects
again and reads; a projection that still will not land exits `1` rather than reporting success.

Matching a state to a column is by option **name OR description**, not description alone. Real
projects do not label that last column consistently: one observed project describes `Analysis`…`Blocked`
with their exact `status:*` labels and then describes `Done` as `closed`, because that column also
tracks the tracker's own closed flag. A verifier demanding `status:done` there would fail on the one
transition that matters most and send a run chasing a projection that had worked.

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
