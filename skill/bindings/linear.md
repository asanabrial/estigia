# Binding: Linear

How this workflow reads an issue whose tracker is Linear, the one thing it may write there, and what
it deliberately never does.
`SKILL.md` describes WHAT happens; this file is the only place that says HOW. Read it when the
operator configuration names `linear` as the tracker.

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
`src/transport/linear.rs`, beside the two operations *Moving a mapped state* sends; no other
operation is composed.

The issue identity is a string on this path — the branch name carries the key, so Linear's own
GitHub integration links the draft pull request, which is that integration acting and not Estigia.

## The credential

Every operation goes through the `linear` CLI (schpet/linear-cli) as `linear api <operation>
--variables-json <variables>`, the way GitHub goes through `gh`. The CLI holds the credential:
the login `linear auth login` stored, or `LINEAR_API_KEY`, which it reads first. Estigia never reads
or writes the key, and the workspace is the CLI's own choice. The credential should be read-only
where Linear allows it, unless `State names` maps a state, which needs write access to issues. A
missing CLI, a missing or rejected credential, an unknown key, a finished issue, or an unreachable
API stops the run before any repository write, and the refusal names which it was.

## Moving a mapped state

With `State names` at `none` — the default — no state is moved and people move every one. Where it
maps a state, as `in-progress=In Progress, review=In Review, done=Done` does, the issue follows the run:
`assist` moves it to the `in-progress` name once the checkout exists, `ready_pr` to the `review` name
once the pull request reads back ready, and `review_stages` to the `done` name when it reads the
pull request merged. Each move reads the issue's state and its team's states
(`query EstigiaStates`), sends one `mutation EstigiaMove` (`issueUpdate` with the state's id) and reads
the state back. An issue already there is not moved again (`moved: false`).

The move is reported under `linear_state` beside the tool's own answer, never folded into it: the
checkout, the ready pull request or the merge happened whatever Linear answered. Its refusals are
`linear-state-unknown` (the team has no state of that name; nothing is sent), `linear-move-refused`
(Linear did not accept the mutation — a read-only credential is the common cause) and
`linear-move-unconfirmed` (the mutation was sent and no answer could be read) and
`linear-move-not-read-back` (it reads back as another state); the last three are an unknown world, so a
run reads the issue before claiming it moved. The credential needs write access to issues for this;
with `State names` at `none` a read-only one is enough.

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
**unsupported** rather than mapped: `ensure_states`, `create`, `list_state`, `claim`, `reclaim`, `verify_claim`,
`heartbeat`, `transition`, `comment`, `unassign`, `publish_review`,
`republish_review`, `handoff_review`, `review_finding`, `review_computed_verdict`, `review_disposition`, and `release_ci`. An assisted run performs none of
them — the team coordinates the work, and Estigia reads the issue and writes nothing but a mapped
state.

## The repository host stays GitHub

Linear holds the issues; GitHub remains the repository host, so the run's draft pull request is
opened there. `Tracker linear` is set in the repository's own file (`config set Tracker "linear
<team-key>" --repo`, the value quoted so it is one argument), so one machine can work a Linear-tracked repository and a GitHub-tracked one
side by side.
