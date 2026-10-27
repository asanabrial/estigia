# `estigia feed`

What happened across sessions while you were away, answered from the local store and nothing else. The
command is read-only: it opens `~/.estigia/state.db` for reading, creates no store that is absent, installs
no hook and records no turn or token telemetry (`parity.md` puts that out of scope). What it cannot see is
in [`honesty.md`](honesty.md), "The feed".

## What it reads

One row per recorded event, from three tables, on one UTC scale. `entries.created_at` and
`transitions.at` are ISO strings and `messages.created_at` is epoch seconds; the feed turns all three
into epoch seconds (`at`) and prints the same instant as `time`.

| Source | `kind` | Carries |
|---|---|---|
| `transitions` | `transition` | `from`, `to`, and the run that held the issue when the state moved |
| `entries` with a marker kind | the marker's kind (`claim`, `reclaim`, `heartbeat`, `progress`, `blocked`, `published`, `review-finding`, `spend`, …) | repository, issue, run |
| `messages` | `message` | sender as `run_id`, recipient as `to` |

An entry that is not a marker (a person's comment) is not an event. A message carries no repository or
issue, so `--repo` and `--issue` leave messages out. `runtime` is read off the run id, which is what
`--runtime` filters on; a message's run is its sender.

## Filters

`--repo owner/name`, `--runtime`, `--run <run-id>`, `--issue N`, `--kind`, `--since`. `--since` is a
duration (`90m`, `6h`, `2d`) or a UTC time, and defaults to `24h`: the window is what keeps the call
bounded. A repository the store does not hold is refused `feed-repo-unknown`, never answered as an empty
page. A `--since` that is neither is refused `feed-since-malformed`. The repository is always the
`owner/name` the `repos` table holds, never one resolved from the caller's checkout.

## Previews and bodies

A row's preview is its kind, issue and, for a transition, both states. Entry and message bodies are
absent unless `--bodies` is given, and then only their first 200 characters; the store is not asked for
a body otherwise.

## Paging

Oldest first, ordered by `(at, source, id)`: the time and the row's own key, neither of which changes
after a row is written. A page is `--limit N` (default 50, at most 500). `next_cursor` continues the
walk; the cursor is bound to the selectors, the window's start, the snapshot time and the store file, and
is refused with the codes `sessions.md` documents: `sessions-cursor-malformed`, `-tampered`, `-too-large`,
`-mismatch`, `-expired` (15 minutes) and `-store-replaced`, and `sessions-limit-too-large` and
`sessions-selector-too-large` for the bounds. Their messages say "session cursor"; the rule is the same
one ([`sessions.md`](sessions.md), "Ordering and the snapshot").

## Output

Prose is one line per event, a footer saying how many of how many matched, and the list of what the feed
cannot see. `--json` carries `events`, `returned`, `matched`, `more`, `next_cursor`, `since`, `taken_at`,
`scope` and `cannot_see`.

## The MCP tool

`feed` answers the same page as `estigia feed --json`, from the same function, on every runtime with a
tool server; the CLI-only runtimes (deepseek-harness, the neutral `agents` root) use the command. The
tool takes the command's selectors as arguments, except that the repository is `repository` rather than
`repo`, a name the tool menu keeps for `create`. It is read-only and answered from the local store, never
from the tracker.
