# `estigia where`

The one answer to «what has happened to this issue»: `estigia where <ref>` and the `where` MCP tool return
repository, state, holder, pull request, review, checklist, clauses, messages and queued writes in one
read. It is read-only: it opens `~/.estigia/state.db` for reading, creates no store that is absent, needs no
claim and sits behind no gate. Gaps are in [`honesty.md`](honesty.md), "Where".

## The reference

| Spelled | Means |
|---|---|
| `owner/name#N` | That GitHub issue, used as given. Read as a GitHub reference, never as a tracker key. |
| `N` or `#N` | An issue number whose repository is found by the resolver below. |
| `ENG-123` | A Linear key. Every field answers `unavailable`: the Linear binding reads an issue's title, description and state type only. |

## Where a bare number lives

One resolver (`transport::scope`) serves `where` and `tasks`. It gathers every repository the store holds an issue
numbered `N` in, plus the repository of every live run pointer holding `N` (taken from the pointer's checkout
`origin`). Exactly one is used (`scope.basis: candidate`). None falls back to the caller's checkout
(`scope.basis: checkout`), so the answer says the repository was a guess. Several are refused `ambiguous-issue`
with the candidates and nothing is read.

`tasks` keeps what it always meant: when the checkout's repository is one of the candidates it is kept. `where`
does not, because a question about an issue should not answer for the wrong one of two. `send_message` addresses
a *run* by what the live registry shows, which never depended on the checkout; a number held by two live
runs is refused `message-recipient-ambiguous`.

## What each field reads

Every field is either its data or `{"unavailable": "<why>"}`; none is filled from a nearby state. An issue the
store holds with no state is `unavailable`, not a default.

| Field | Source |
|---|---|
| `issue` | the tracker: open or closed, URL (`gh api`) |
| `workflow`, `transitions` | `issues.state`; the last five rows of the `transitions` history (the same rows as [`feed.md`](feed.md)) |
| `holders` | the session registry: runs whose pointer holds the issue in this repository, their status (`live`, `stale`, `ended`), last seen, worktree and the branch checked out there |
| `pull_request` | the tracker: number from the newest review receipt, URL, state and live head and base |
| `review` | the newest receipt on the local timeline: verdict, `escalated`, epoch, PR, **head and base it is bound to**, digest, and `bound_to_live_head` (`null` when the live head could not be read) |
| `tasks`, `blocked_until` | the checklist progress and open tasks; the recorded clauses |
| `messages` | the last five messages to or from each holder run (preview of 200 characters) |
| `outbox` | writes queued and not landed (`queued` or `sending`); a sent write is not listed |

`taken_at` stamps the answer: the holder can change between reads.

## The MCP tool

`where` takes `reference` and answers the same object as `estigia where --json`. It is read-only
(`writes: false`, no pointer effect) and answered from the local store and the tracker, so it is on every
runtime with a tool server; the CLI-only runtimes use the command. Refusals: `ambiguous-issue`,
`where-reference-malformed` ([`refusal-classes.md`](refusal-classes.md)).
