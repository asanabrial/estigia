# Binding: Linear (read-only)

How this workflow reads an issue whose tracker is Linear, and what it deliberately never does.
`SKILL.md` describes WHAT happens; this file is the only place that says HOW. Read it when the
operator configuration names `linear` as the tracker.

## Read-only, and only under assisted work

`Tracker linear <team-key>` names Linear as the tracker. Estigia **reads** and never writes: no
claim, no state transition, no comment, no label, no assignment. That is why it is admitted only
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
refuse an issue that is `completed` or `canceled` (`issue-not-open`), and is never moved. A key is
upper-cased, so its two spellings are one issue, and its team (the text before the last `-`)
must be the configured `<team-key>` (`linear-team-mismatch`). The query is a constant in
`src/transport/linear.rs`; the set of operations this source can send is one, and it is a query.

The issue identity is a string on this path — the branch name carries the key, so Linear's own
GitHub integration links the draft pull request, which is that integration acting and not Estigia.

## The credential

The API key is read from the environment variable `LINEAR_API_KEY`, never from a configuration file
somebody could commit. It should be read-only where Linear allows it. A missing or rejected
credential, an unknown key, a finished issue, or an unreachable API stops the run before any
repository write, and the refusal names which it was.

## What the states are not

Assisted work does not move Linear's workflow states (Backlog/Todo, In Progress, Ready for Review,
Ready for Release), and reads only the state's type, to refuse a finished issue. People move every
state; nothing here does.

## The refusals this source gives

Each stops the run before any repository write and names its reason:

- `linear-credential-missing` — `LINEAR_API_KEY` is unset.
- `linear-credential-rejected` — Linear refused the credential (401 or 403, or an authentication
  error in the answer's `errors`).
- `linear-team-mismatch` — the key's team is not the configured `<team-key>`.
- `issue-not-open` — the issue's state type is `completed` or `canceled`.
- `linear-credential-malformed` — the credential or the endpoint holds a quote, a backslash or a
  control character the curl configuration cannot carry.
- `linear-issue-unknown` — Linear knows no issue with that key.
- `linear-unreachable` — the API could not be reached, or answered something that is not Linear's
  JSON, or answered errors with no data.

And one defect rather than an answer: `issue-has-no-number`, when a Linear key reaches an operation
only a numbering tracker performs (every such operation is refused for Linear before it is reached).

## The operations this binding does not perform

Linear is read-only here, so every operation that would write the tracker is **unsupported** rather
than mapped: `ensure_states`, `create`, `list_state`, `claim`, `reclaim`, `verify_claim`,
`heartbeat`, `transition`, `comment`, `unassign`, `publish_review`,
`republish_review`, `handoff_review`, `review_finding`, `review_computed_verdict`, `review_disposition`, and `release_ci`. An assisted run performs none of
them — the team coordinates the work, and Estigia reads the issue and writes nothing.

## The repository host stays GitHub

Linear holds the issues; GitHub remains the repository host, so the run's draft pull request is
opened there. `Tracker linear` is set in the repository's own file (`config set Tracker "linear
<team-key>" --repo`, the value quoted so it is one argument), so one machine can work a Linear-tracked repository and a GitHub-tracked one
side by side.
