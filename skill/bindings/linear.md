# Binding: Linear

How this workflow reads an issue whose tracker is Linear, the one thing it may write there, and what
it deliberately never does.
`SKILL.md` describes WHAT happens; this file is the only place that says HOW. Read it when the
operator configuration names `linear` as the tracker.

A review-only claim on a pull request (`pr_review`) binds to the repository host and writes nothing here.

## Read, and a mapped state moved, only under assisted work

`Tracker linear <team-key>` names Linear as the tracker. Estigia **reads**, and writes nothing but an
issue's workflow state where the repository's `State names` row maps one (*Moving a mapped state*
below): no claim, no comment, no label, no assignment. That is why it is admitted only
under `Claim workflow: assisted` — autonomous work needs an atomic claim Linear cannot provide, so
`Tracker linear` with `Claim workflow autonomous` is refused by `config set`, by `doctor`, and at run
start, each naming this reason.

## The one operation

`flow <KEY>` (for example `flow ENG-123`) reads the issue with Linear's GraphQL API:

```graphql
query EstigiaIssue($id: String!) { issue(id: $id) { title description state { type } } }
```

`issue(id:)` accepts the shorthand identifier, so the key the operator typed resolves without a
second lookup. Title and description are what the run works from; the state's type is read only to
refuse an issue that is `completed` or `canceled` (`issue-not-open`). A key is
upper-cased, so its two spellings are one issue, and its team (the text before the last `-`)
must be the configured `<team-key>` (`linear-team-mismatch`). The query is a constant in
`src/transport/linear.rs`, beside the two operations *Moving a mapped state* sends and the read
`where` makes of a key's state and URL (`query EstigiaStanding`), and the loop's queue read below
(`query EstigiaQueue`); no other operation is composed.

The issue identity is a string on this path — the branch name carries the key, so Linear's own
GitHub integration links the draft pull request, which is that integration acting and not Estigia.

## The loop's queue

An assisted loop selects its next issue from `query EstigiaQueue`. That query reads the open issues of
the configured team that are assigned to the CLI's own user (`viewer`), in Todo (`unstarted`) or Backlog
(`backlog`). Nothing else is offered: not unassigned issues, and not anyone else's. Issues are worked in
this order:

- all of Todo before Backlog;
- within each, priority: Urgent, High, Medium, Low, then none;
- then the smaller `estimate`, with an unestimated issue after the estimated ones;
- then the oldest.

An issue a live claim on this machine holds is left out. The loop hands the head to the agent as
`/flow dev <KEY>`, which runs `assist` on it. A page that says more follow is a failed read, never a
short queue, and a failed read stops the loop.

## The credential

Every operation goes through the `linear` CLI (schpet/linear-cli) as `linear api <operation>
--variables-json <variables>`, the way GitHub goes through `gh`. The CLI holds the credential:
the login `linear auth login` stored, or `LINEAR_API_KEY`, which it reads first. Estigia never reads
or writes the key, and the workspace is the CLI's own choice. The credential should be read-only
where Linear allows it, unless `State names` maps a state, which needs write access to issues. A
missing CLI, a missing or rejected credential, an unknown key, a finished issue, or an unreachable
API stops the run before any repository write, and the refusal names which it was.

## Moving a mapped state

Only mapped destinations are written. With `State names` at `none` (the default), people move every state.

| Observation | Destination |
|---|---|
| `assist` creates the checkout | `in-progress` |
| `assist` resumes it | `in-progress` only from `triage`, `backlog` or `unstarted` in `EstigiaStanding`. Otherwise report the observed `state` and `state_type` |
| `ready_pr` confirms ready | `review` |
| `review_stages` reads an open PR | Blocking comments, or drift outside both mapped active states (including Todo and Backlog), take precedence and select `in-progress`. Otherwise a ready head with passed CI, completed blocking checks and zero open threads selects `review`, even while awaiting human review |
| `review_stages` reads a merge | `done` |

A `push-ready` correction keeps the current state. Every stage reading repeats the decision.

`EstigiaStates` reads the current and offered states. An issue already at the destination reports
`moved: false`. A move sends one `EstigiaMove` (`issueUpdate` with the target ID) and reads the
state back. The tool reports its own result alongside `linear_state`. A confirmed change returns
`moved: true` and `tracker_written: true`. An unreadable initial state during the correction cycle
or resumed assist returns `read_failed: true` without a mutation.

`linear-state-unknown` means the team lacks the mapped name. After a mutation, `linear-move-refused`
means Linear rejected it, `linear-move-unconfirmed` means its response or the read-back was unreadable,
and `linear-move-not-read-back` means another state was observed. All three leave `tracker_written: null`
and `world: unknown`. Read the issue before reporting its state.

## The refusals this source gives

Each stops the run before any repository write and names its reason:

- `linear-cli-missing` — the `linear` program is not on `PATH`.
- `linear-credential-missing` — the CLI holds no credential: no `linear auth login` and no
  `LINEAR_API_KEY`.
- `linear-credential-rejected` — Linear refused the credential (an authentication error in the
  answer's `errors`).
- `linear-team-mismatch` — the key's team is not the configured `<team-key>`.
- `issue-not-open` — the issue's state type is `completed` or `canceled`.
- `linear-issue-unknown` — Linear knows no issue with that key.
- `linear-unreachable` — the CLI failed without Linear's JSON (the API could not be reached, or it
  did not answer within 30 seconds), answered something that is not Linear's JSON, or Linear
  answered errors with no data.

And one defect rather than an answer: `issue-has-no-number`, when a Linear key reaches an operation
only a numbering tracker performs (every such operation is refused for Linear before it is reached).

## The operations this binding does not perform

Apart from a mapped state, Linear is not written, so every operation that would write the tracker is
**unsupported** rather than mapped: `create`, `list_state`, `claim`, `reclaim`, `verify_claim`,
`transition`, `comment`, `unassign`, `publish_review`,
`republish_review`, `handoff_review`, `review_finding`, `review_computed_verdict`, `review_disposition`, and `release_ci`. An assisted run performs none of
them — the team coordinates the work, and Estigia reads the issue and writes nothing but a mapped
state.

## The repository host stays GitHub

Linear holds the issues; GitHub remains the repository host, so the run's draft pull request is
opened there. `Tracker linear` is set in the repository's own file (`config set Tracker "linear
<team-key>" --repo`, the value quoted so it is one argument), so one machine can work a Linear-tracked repository and a GitHub-tracked one
side by side.
