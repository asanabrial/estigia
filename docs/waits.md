# What a delivery spends waiting

A run that waits for CI, a reviewer or a background job can wait in two ways. The runtime can hold the
wait: one call blocks, or a background job notifies the session when it ends. Or the model can hold it:
it calls again because a clock passed, and every call re-sends the whole conversation. This page counts
the second kind in this repository's own deliveries and says which of them an existing path could have
carried. It changes nothing. Moving a wait is a later unit of the token-efficiency programme, and [`honesty.md`](honesty.md#the-waits-audit)
says what this count cannot show.

## Population and method

The population is Claude Code's own transcripts for this repository and its worktrees on one machine,
read on 2026-10-27 up to 23:10 UTC. A transcript counts as a delivery when its root session called
`publish_review`, `deliver`, `release_ci` or `record_computed_verdict`. That gives **45 transcripts** with
first wait calls from 2026-09-29 to 2026-10-27, **17,062 tool calls**, **1,370 task notifications** and
**15,125 model requests**. Each request is counted once by message id. Their summed context is
**5,119.2 M tokens**: input plus cache read plus cache creation, nearly all of it cache read. Subagent
(sidechain) messages are excluded, as are other runtimes.

Every tool call was classified from its name and its input. Each was then paired with its result: the
wall-clock seconds between the call and its result, whether the answer still said it was not finished,
and the context of the request that carried the call. A request carrying several calls is counted in
the first category it matches.

| Kind | What matched |
|---|---|
| Foreground loop | a `Bash` call, not in the background, whose command is an `until`/`while` loop around `sleep` |
| `sleep` then read | a `Bash` call that starts with `sleep N` and is not such a loop |
| CI read | `gh pr checks` or `gh run view`/`gh run list` without `--watch` |
| CI watch | `gh run watch` or `gh pr checks --watch`; in the background it is a launch |
| Estigia waits | `deliver`, `collect_review_input`, `review_status` |
| Person | `AskUserQuestion` |
| Host scheduling | `Monitor`, `ScheduleWakeup` |

A `deliver` answer is *pending* when it says `pending`, `capture-running`, `running` or `ask_after`. A
CI read is *pending* when its output says `pending`, `in_progress` or `queued`.

## Unnecessary: a clock passed, not an answer

| | Calls | Wall clock | Context | Sessions (prefix: calls) |
|---|---:|---:|---:|---|
| Foreground loop waiting on a background job's output file | 34 | 233.3 min | 10.16 M | `cc87dae5` 27, `4629bd2b` 6, `f63b259a` 1 |
| Foreground loop polling `gh` until a run ends | 22 | 137.0 min | 7.46 M | `4629bd2b` 13, `2670e347` 7, `6ff0cf0d` 2 |
| `deliver` answered pending | 20 | 16.9 min | 2.95 M | `cc87dae5` 14, `4d6d9a8b` 3, `5c01241d` 2, `aa2994d1` 1 |
| CI read whose answer was still pending | 111 | 36.1 min | 34.74 M | 13 sessions; `4629bd2b` 42, `6ff0cf0d` 14, `f58c9623` 11, `cc87dae5` 11 |
| `sleep` then read | 60 | 13.4 min | 17.64 M | 16 sessions; `6ff0cf0d` 30 |
| **Total** | **247** | | **72.95 M** | about 1.4% of the 5,119.2 M |

Three patterns account for most of it.

- **One wait held twice.** In `cc87dae5` (2026-10-27) the run launched a background script that waited for
  a head's CI and wrote the answer to a file. It then polled that file in a foreground `until` loop.
  The host already notifies the session when a background job exits, so the loop added nothing but
  calls. Each loop ran into the host's 600 s ceiling on one `Bash` call and was called again. **15 of
  the 34** ended at that ceiling.
- **`deliver`'s ceiling turns one CI run into several calls.** When these transcripts were
  written, `deliver` waited at most 50 s per call (`MAX_WAIT_SECONDS` in `src/harness/mcp/deliver.rs`),
  so a CI run of ten minutes or more needs repeated
  calls. In `cc87dae5` the pending answers come in runs of three or four per head, about half an hour
  apart. The 50 s figure rests on the reading that some hosts give up on an MCP call near 60 s. On this
  host, `collect_review_input` calls with `wait_seconds` 900 ran past 60 s nine times, the longest for
  111 s, and none was cut off.
- **Reading CI while it is still running.** 111 reads answered with checks still pending. The match is on
  words in the output, so this is an upper bound: a read that also diagnosed a failure, or that listed
  one pending check beside a red one, is counted too.

The `sleep` row is mixed. Half of it (`6ff0cf0d`, 30 calls) is short `sleep 1`–`sleep 5` pauses while
driving a live runtime probe. That is a test harness waiting on a terminal, not a delivery waiting on CI.

## Kept: the model's own decision points, or already held by the runtime

| | Calls | Wall clock | Context | Why it stays |
|---|---:|---:|---:|---|
| `AskUserQuestion` | 40 | 3,493.3 min | 14.18 M | a person's answer; the wait is theirs |
| `review_status` | 92 | 13.1 min | 19.53 M | it names the next step; each answer is a decision |
| `collect_review_input` | 237 | 92.2 min | 86.96 M | 218 passed `wait_seconds` (211 of them 900), and none waited the whole window: the runtime held the wait |
| `deliver`, answered with a step or a hand-back | 30 | 8.5 min | 4.95 M | a merge, a refusal or a close to act on |
| CI watch in the foreground | 31 | 172.4 min | 5.54 M | one call per run, held by `gh` |
| Background watch or loop, the launch | 206 | 2.9 min | 74.20 M | one call to launch, then one notification |
| `Monitor`, `ScheduleWakeup` | 41 | 0.2 min | 13.36 M | a loop or a watch the person set up |

A claim adjudication, a refusal and a verdict are never waits in this sense: each is answered by the call
that asked.

One further category belongs to neither table: **52 foreground loops that waited on local experiments**,
not on a delivery. 43 of them in `2670e347` drove a benchmark's runs, and the rest polled `tmux` panes
or a local `git` count (368.9 min, 19.79 M context, 27 of them at the 600 s ceiling). They are excluded
from both totals.

## Which existing path could have carried them

- **The background job's notification** carries every row of the first pattern. The waiting script was
  already in the background, so the foreground loop only had to be left out.
- **A background `gh run watch`** carries a foreground loop polling `gh`, and it carries most pending CI
  reads: one launch and one notification per run, which is how 206 of the waits above were already held.
- **`deliver`'s own wait** carries the pending `deliver` answers when the caller asks for it. Its
  ceiling is now 900 s, the one `collect_review_input` has. After the first 50 s it reads the pull
  request every 20 s instead of every 2 s, and the default stays 20 s. Whether a host holds a call that
  long is the question the 50 s reading left open. This count shows only that Claude Code held 111 s.
- **No path is needed for the kept rows.** Each is a decision the model or a person makes, or a wait the
  runtime already holds.

## What this page is not

It is not a saving. 72.95 M tokens of context is what the matched calls' requests carried. A call
replaced by a notification still costs the one request that reads the notification, so the saving is
the calls removed beyond that one, and it was not computed. It is also not billed tokens, because almost
all of that context is cache read, which is priced differently.
The figures are one machine, one host and one person's pace, over four weeks, and the session prefixes
name transcripts on that machine only.
