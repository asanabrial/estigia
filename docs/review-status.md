# `review_status`: what a review does next

Issue #710, slice S2b. gentle-ai v4 answers STATUS with exactly one next transition and the agent runs it
without deciding; without one the orchestrator chooses the review route, which is what v4 removed.
`review_status` (MCP) / `review-status --issue <n>` (transport) is that answer. It is read-only, needs
no claim, and decides nothing the tools do not already decide: each state below is read from the same
functions the tools judge with ([`claim.rs`](../src/transport/claim.rs),
[`claim/status.rs`](../src/transport/claim/status.rs)). `publish_review` and `republish_review` take
`--with-review`, which returns the first `review_status` (with its forecast) beside the publication it
just made, so the read that always follows a publication is not a second round trip.

## What it reads

- **The local store, only.** The issue's timeline comes from `Ledger::read_timeline_at`: no import, no
  sync, no store created. It never reads an issue's comments or labels from GitHub.
- **GitHub's own objects, only.** `gh pr view` for the newest publication's pull request (state, draft,
  head, base, `mergeable`, `mergeStateStatus`) and, only when the next step would record an approval,
  the publication lane's check runs. When the head is the receipt's and only the base differs, one local
  `git merge-base --is-ancestor <reviewed base> <current base>` in the checkout says whether the base
  advanced; a base object the clone does not hold makes the answer `unavailable`, never a clean advance.
- **The repository is not read from GitHub either.** It is the operator-named repository when one is
  configured, else the `origin` remote when that is a `github.com` remote. Anything else is a failed
  read, and so is a store whose rows sit under another repository than the one derived (the issue is
  then `review-status-issue-not-in-store`). A fork whose `gh` default is not `origin` needs the
  repository named.

## The answer

`{ok, issue, receipt, next, forecast}`. `receipt` is the newest publication's `epoch`, `pr`, `head`,
`base` and `digest`, or `null` before any publication. `next.kind` is one of:

- `execute`: run `next.operation` with `next.arguments`, adding your own `run_id`, an `operation_id`
  where the tool takes one, and the tool's own non-receipt arguments: `branch`, `base`, `pr_title` and
  `pr_body_file` for `publish_review`; `escalated_to` and `reason` for `escalate_review`.
- `collect`: obtain every `next.inputs[]` entry with its `capture` tool, then ask again. Every input
  sharing a `capture` may be obtained in one call: `launch_review_input` and `collect_review_input`
  take `names` as well as `name`, so a `high` review launches its four lenses in one call and collects
  them in the next. A batch reports a slot a running job already holds (`capture-already-running`, a
  launch) or one still running (`capture-running`, a collect) rather than refusing the whole call.
  `next.arguments` is the receipt they record against. Each input is read against the job table
  and the job's lock, never the network, and names the one call that clears it:
  - no job holds the slot on this receipt: `capture: "launch_review_input"`, which starts the
    reviewer itself;
  - the job finished, its runner died, or it is still alive a minute past its `deadline_at`:
    `capture: "collect_review_input"` and `job`, because collecting records the answer, abandons the
    job or ends it, and the slot is offered again;
  - the job is running: `capture: null` and `running: {job, deadline_at}`. No command is named,
    because `collect_review_input` would be refused `capture-running`. Ask `status` again.

  When every input is running, `next.ask_after` is the earliest `deadline_at` (Unix seconds): the
  latest sensible time to ask again, since a reviewer usually answers sooner and asking earlier costs
  only a read. `kind` stays `collect`. A job of another epoch, or one already admitted, abandoned or
  stale, holds nothing. A name `status` does not offer, or one that is none of a lens, `refutation-batch` and
`review-validator`, is refused `capture-input-not-offered` by `launch_review_input` before any
reviewer starts, so `status` is what decides what may be captured. A hand-recorded lens report, refutation or
validator result is refused `recorded-by-capture-only`, so a slot
`status` offers is cleared by `launch_review_input` and `collect_review_input` and by nothing else. With neither `claude` nor
`codex` on `PATH` the collect refuses `reviewer-cli-missing`; the review then moves by
`handoff_review` to a run that has one.
- `stop`: `next.reason_code`, with `next.continuation` saying who acts. Nothing is executed.

`forecast.horizon` is `terminal` for a `stop` and `partial` otherwise.

## Every state, and its answer

| State | `kind` | `reason_code` | Names |
|---|---|---|---|
| No publication | `execute` | `no-publication` | `publish_review`, `expected_epoch: none` |
| Pull request closed, unmerged | `execute` | `pull-request-closed` | `publish_review`, `expected_epoch: none` |
| A licensed lens has not reported | `collect` | `lens-report-missing` | `launch_review_input` or `collect_review_input` per lens role, or neither while its job runs |
| Only the validator is owed | `collect` | `validator-owed` | one input, `review-validator`, annotated as above |
| Inferential severe finding, no refuter batch yet | `collect` | `refutation-missing` | one input, `refutation-batch`, annotated as above, listing every finding id that needs an entry |
| Verdict computable, not recorded | `execute` | `verdict-computable` | `record_computed_verdict` with the receipt |
| Approved and recorded | `execute` | `approved` | `release_ci` with the receipt |
| Rejected, correction unspent | `execute` | `correction-due` | `publish_review`, `expected_epoch: <this epoch>`; when the targeted validator failed the correction, the answer carries a plain `why` naming what it found |
| Rejected, correction spent | `execute` | `correction-spent` | `escalate_review` with the receipt; the answer carries a plain `why` |
| Escalated, recovered by a person (also over a moved pull request) | `execute` | `recovered` | `publish_review`, `expected_epoch: <this epoch>` |
| Escalated, unrecovered (also over a closed or moved pull request) | `stop` | `escalated` | the person's `estigia recover <issue> --reason "<why>"` command, never an agent's; the stop carries a plain `why` sentence beside the reason code |
| Approved, ledger or tier moved, and a finding was recorded after the verdict | `execute` | `verdict-stale` | `publish_review`, `expected_epoch: <this epoch>` |
| Approved, ledger or tier moved, and no finding was recorded after the verdict | `stop` | `verdict-stale` | a person decides: record the defect the verdict did not weigh with `record_review_finding` and publish again, or close the pull request and publish afresh |
| Approved by a legacy verdict with no computed marker, ledger not yet recorded | as the unjudged rows | | `record_computed_verdict`, or the lens and refutation inputs it needs |
| Approved, CI already released | `stop` | `released` | the run holding the receipt merges it with `gh pr merge <pr> --merge --match-head-commit <head>` (the receipt's own pull request and head) once CI is green on that head and `Delivery authorisation` authorises this run; otherwise it asks first, naming the reviewed head and base. The same answer when the base advanced past the reviewed base, the pull request is ready, its head is the reviewed head, the new base descends from the reviewed base and GitHub reports it `MERGEABLE` with a `mergeStateStatus` that is not `DIRTY`; the continuation then names the new base, and CI is not asked again on it |
| Approved, CI released, base advanced past the reviewed base, and GitHub still computing the mergeability | `stop` | `mergeability-unknown` | ask again once GitHub has answered; `UNKNOWN` (in `mergeable` or `mergeStateStatus`) is never read as clean |
| Merged | `stop` | `delivered` | the run holding the receipt completes the delivery with `closing::FOLLOW_UP`: `transition --to done`, then `release`, which is last and not optional |
| Pull request head or base is not the receipt's (a base that advanced cleanly over an approved, ready receipt is the two rows above, not this one) | `stop` | `pull-request-moved` | `publish_review` deliberately — a genuinely unjudged receipt and an approval already answered by a finding fall through to it — except a lineage whose corrections are all spent (`escalate_review`) or a fresh approval with no cause after its verdict (`record_review_finding`, then publish) |
| Computable approval over a lane that is red, never ran or is unfinished | `stop` | the lane's own `publication-lane-*` code | the lane's own action |

A recovered escalation is answered `recovered` before the pull request's head or base is compared,
so a recovered escalation over a moved pull request names its successor publication rather than
`pull-request-moved`; both name `publish_review`, and the successor is admitted by the gate. A stale
approval with no cause is answered the `verdict-stale` stop even when the pull request has moved, so
a moved pull request over one names the finding that admits the publication or the person, never the
`pull-request-moved` fall-through; a stale approval already answered by a finding has its cause and
takes the fall-through.

A receipt takes one refutation batch, so once one is recorded a later unrefuted finding is never
collected: the answer is `verdict-computable`, which records the escalation. An escalation the ledger cannot settle by a refuter (an insufficient-evidence finding, a lens-focus
disagreement) is `verdict-computable`: recording the verdict is what makes it a standing escalation.
A correction is named as `publish_review`; a history-rewriting one is `republish_review`, which
`publish_review` itself refuses with and names. `publish_review` over an approved receipt is admitted only when a finding was recorded after the
verdict or the base advanced, and `record_computed_verdict` refuses `computed-verdict-already-recorded`
over a changed ledger. So a tier that moved, or a late lens report that escalates the lens focus,
has no publication to name until a cause exists: the answer is the `verdict-stale` stop, whose
continuation says a person decides — record the finding the verdict did not weigh with
`record_review_finding`, which admits the publication, or close the pull request and publish the
change afresh. A receipt whose publication was never confirmed is refused
as `release_ci` would refuse it (`publication-unconfirmed`), not answered as a state. A configured
repository that is not `owner/name` is unavailable, never replaced by `origin`.

## Refusals: unavailable, stale, refused

Failure to read is never a state. The classes are [`refusal-classes.md`](refusal-classes.md)'s.

- **`unavailable`**: the store is unreadable or holds no such issue (`review-status-store-unreadable`,
  `review-status-issue-not-in-store`), the pull request read failed, the lane read failed, the
  repository could not be named, or the ancestry of an advanced base could not be read. Read again later; nothing was decided.
- **`stale`**: a command `status` named is refused because the receipt moved after the answer. Every
  `record_*` tool and `release_ci` re-check the exact receipt (`published-transaction-mismatch`), and a
  publication re-checks `expected_epoch` (`expected-epoch-moved`) before anything is pushed. The
  answer is therefore safe to act on late: it is refused, never executed.
- **`refused`**: the rule says no.

What this does not prove is in [`honesty.md`](honesty.md): the answer is a read, not a lock, and a
reader that executes it unchanged is protected by the commands' own checks, not by `status`.
