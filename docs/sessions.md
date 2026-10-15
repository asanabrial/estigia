# The session registry

`estigia sessions [--json]` answers one question: which sessions has this machine seen, which issue does
each hold, and is each still there. `estigia sessions send` puts a message in another session's mailbox, and the
hook point of each runtime delivers it. The `send_message` and `list_sessions` MCP tools are the same two
doors for an agent ([below](#the-mcp-tools)). The last section lists what does not exist yet.

## What is recorded

`~/.estigia/state.db` carries a `sessions` table, one row per run id: the runtime, the host's session id, the
directory the session started in, when it started, when Estigia last saw it, and when and why it ended.

- `estigia session start` and the Claude Code and Codex `SessionStart` hook write or refresh the row. A session that starts again under the same run id
  keeps its first start time and is no longer ended.
- `estigia session end` and the `SessionEnd` hook stamp `ended_at` (and `end_reason`, when the host gave one) when the session is really over. A `new`, `resume`,
  `fork` or `reload` end changes nothing, as for the run pointer. The row is never deleted, so an ended
  session stays visible to the sender of a later message.
- Both writes are best effort: a registry that cannot be written does not stop a session from starting or ending.

The issue and the state shown for a session are read from its run pointer when the pointer names an issue. The
registry holds no claim of its own, and the tracker remains the authority on whether a claim is live.

## What `estigia sessions` shows

Each entry carries `run_id`, `runtime`, `session_id`, `status`, `issue`, `state`, `last_seen`, `silent_for`,
`repo_dir`, `worktree`, `ended_at`, `end_reason`, `waiting` and `undelivered_sent`, under `--json` as a `sessions` list beside `stale_after`.

| `status` | Meaning |
|---|---|
| `live` | Seen within `stale_after` seconds (six hours). |
| `stale` | Silent for longer. It is shown, with the issue its pointer still names, and never removed. |
| `ended` | `session end` recorded the end. |
| `unregistered` | A run pointer names an issue and no session row exists: the session started before the registry did, or its start was never recorded. |
| `unknown` | The clock could not be read, so silence cannot be measured. |

`last_seen` is the later of the registry's stamp and the pointer's `verified_at`, because a claim or a verified
write refreshes the second. A machine whose store does not exist yet lists no sessions and the read does not create it.
A store that is present and cannot be read is refused, not listed as empty.

## The mailbox

`estigia sessions send <run-id> "<text>"` (or `--file <path>`) writes one row to the `messages` table of
`state.db` (schema 11): the sender's run id when one is known, the recipient's run id, the text, and when it was
sent. The sender is `--run-id`, or `--session-id` with `--runtime`, or the ambient `ESTIGIA_RUN_ID`; a person at a
terminal sends with none, and the message is shown as from a person.

- The recipient must be a session the registry knows. A run id nobody registered is refused with
  `message-recipient-unknown` and nothing is recorded.
- A recipient that has ended is accepted: the row is written, it stays undelivered, and `estigia sessions` shows it
  against the sender (`undelivered_sent`) for as long as it stays so. An empty message and one over 8192 bytes are refused.
- Sending is a write to Estigia's own state, like `estigia goal stop-after`, not a write to the repository: it needs
  no claim and the gate classifies the shell line that runs it as it classifies any other `estigia` command. The row
  is the record of the send. The MCP tools in [the next section](#the-mcp-tools) reach the same row.
- `estigia sessions` shows `waiting` (rows addressed to this session and not yet delivered) and `undelivered_sent`
  (rows this session sent that nobody has taken) beside each session.

Delivery is `estigia session deliver --session-id <id> --runtime <runtime>`, which prints the pending rows as one
text block and stamps each `delivered_at` in the same statement, so two callers racing for one session split the
rows and never both receive one. It prints nothing when none wait. A store that cannot be read or written delivers
nothing and leaves the rows pending; a delivery that failed is never recorded as made.

| Runtime | Delivery point | How |
|---|---|---|
| Claude Code | `PreToolUse` hook | The hook's answer carries the text as `hookSpecificOutput.additionalContext`, beside whatever it decided. |
| Claude Code | `Stop` hook, `--wait-mail` | Outlives the turn (`asyncRewake`) and, when mail arrives, exits 2 with the text on standard error, which starts a new turn in an idle session. [Below](#waking-an-idle-claude-code-session). |
| Codex | `PreToolUse` hook | The same hook and the same answer, registered with `--agent codex`. |
| Codex | `Stop` hook, only with the `Loop plugin` row `on` | `{"decision":"block","reason":…}` with the text as the reason; when the loop also continues, one reason carries the loop's text, a blank line, then the mail. A turn in progress only: an idle Codex session is not woken. |
| Pi | `before_agent_start` | The extension runs `estigia session deliver` and adds the text as its own `estigia-mail` prompt section. |
| Pi, idle | A poll every 10 seconds between turns | The extension runs `estigia session deliver --peek --json`; with mail it calls `pi.sendMessage` with `deliverAs: "followUp"` and `triggerTurn: true`, and only when that returned without throwing it runs `estigia session deliver --through <id>`. See [Waking an idle Pi session](#waking-an-idle-pi-session). |
| OpenCode 1 | `tool.execute.after` of the gate plugin | The text is appended to the tool's string result, after the plugin registered the session with `estigia session start`. |
| OpenCode 2 | `execute.after` of the gate plugin | `ctx.session.prompt` with `delivery: "steer"` (the accelerator); when the service is absent or throws, `ctx.session.synthetic`. With neither function the mailbox is not read and the rows stay pending. |

### Waking an idle Pi session

A Pi session that is between turns reaches no hook point, so the extension polls. The root session starts a 10-second
timer (`WAKE_INTERVAL_MS`) at `session_start` and again at `agent_settled`, the event Pi fires once a run has fully
settled with no retry, compaction or queued continuation pending; it stops the timer at `agent_start`, so nothing
polls while a turn runs, and at `session_shutdown`. A child session never starts one. Both events are
`AgentStartEvent` and `AgentSettledEvent` in `dist/core/extensions/types.d.ts` of `@earendil-works/pi-coding-agent`
1.1.0.

Each tick is `estigia session deliver --peek --json`, which prints `{"messages": <text or null>, "through": <id of
the last message or null>}` and stamps nothing; `--through <id>` stamps and prints only the rows up to that id, so
mail that arrives between the two calls is left pending. With mail, the extension calls `pi.sendMessage` with a
custom message of type `estigia-mail`, `deliverAs: "followUp"` and `triggerTurn: true`. In `AgentSession.sendCustomMessage`
(`dist/core/agent-session.js`) an idle session with `triggerTurn` runs a new turn on that message, and one that began
streaming meanwhile queues it as a follow-up. `ExtensionAPI.sendMessage` returns nothing and reports an asynchronous
failure only as a runtime error event, so "handed over" means the call did not throw. That is the order the take
follows: a `sendMessage` that throws, or a host without one, leaves the rows pending and `before_agent_start` takes them
as before. If the take fails after a successful hand-over the rows stay pending and are delivered a second time.
`before_agent_start` waits for a wake in flight, so the turn a wake starts does not take the same rows into its prompt.

The wake is read from Pi's type definitions and source and driven under `node` against a stand-in `pi` and `estigia`.
It was also run once live, on Pi 1.1.0 on 2026-10-15. An interactive session loaded only this extension and was left
idle after one turn. A message sent to it with `estigia sessions send` started a turn within 30 seconds with no
input, and the row was stamped delivered. Not measured live: a `sendMessage` that fails, and mail arriving while a
turn runs.

What delivery refuses to do:

- It never turns an answer into a decision. A `PreToolUse` answer that denies or asks carries no mail and leaves the
  rows pending, and an answer that allows or stands aside carries the mail without changing the permission.
- It is delivered only to the session's own context. A sub-agent's `PreToolUse` call (one carrying an `agent_type`)
  and a Pi child session do not take the root session's mail.
- It is recorded when it is handed to the host, not when the model reads it: no host reports a read, so "delivered"
  is the observable event and there is no separate "read" stamp.

## The MCP tools

`list_sessions` takes no arguments and answers the object `estigia sessions --json` prints (`sessions` beside
`stale_after`), built by the same function.

`send_message` takes `run_id` (the sender, this run's id, held to the shape every tool's `run_id` is), `to` and
`text`, and calls the same `mailbox::send` as `estigia sessions send`, so the recipient, empty-text and size
refusals above are the same ones. `to` is a run id, or an issue number written `12` or `#12`. An issue resolves to
the run the local store's timeline shows holding it (the claim reducer over the stored timeline, as for any holder
decision), never to a run pointer or a label. An issue with no live holder is refused with
`message-issue-has-no-holder` and nothing is recorded. Both tools answer from the local store and bypass the
tracker transport, so they need neither a tracker nor a repository host transport and are not binding operations.

Consent follows who started the send. A person at a terminal is never asked. An agent's `send_message` is asked
about before any row is written, through MCP elicitation (protocol `2025-06-18`):

- At `initialize` the server records whether the client sent a `capabilities.elicitation` object. Its shape is not
  inspected: `{}` and `{"form":{},"url":{}}` both count.
- With no such capability, and no session grant, the send is refused with `message-needs-a-person` and nothing is
  recorded.
- Otherwise the server sends `elicitation/create` with a message naming the sender, the recipient run id, the issue
  the recipient holds when it is known, and the text, and a schema with one required string `choice`: `allow_once`,
  `allow_session` or `deny`.
- `accept` with `allow_once` sends. `accept` with `allow_session` sends and remembers the grant, keyed by the
  sender's run id, until that server process exits (one MCP server process is one agent session). Everything else
  is refused with `message-not-allowed` and nothing is recorded: `deny`, `decline`, `cancel`, an unknown choice, an
  error response, or no answer in 15 minutes. A question that could not be put or whose answer could not be read
  (a failed write, a read error, a closed input) is refused `message-question-failed` instead, because the person
  was never asked and the refusal must not read as their decision.
- An issue number is resolved against the clock; an unreadable clock is `message-holder-unreadable`, never a holder.
- While the server waits, any other line from the client is queued and served after the tool call answers. Requests
  are still served one at a time.
- The tool has no argument that skips the question, and the binary cannot tell whether a person asked the agent to
  send. A person who wants no prompt sends from the CLI.

The issue named in the question for a run-id recipient is read from that run's pointer, for display only; the
store decides only the resolution of an issue number.

What was measured, on Claude Code 2.1.300 (2026-10-15):

- Against a probe MCP server: Claude Code sends a `server/discover` request before `initialize` (a method-not-found
  answer is accepted), declares `capabilities.elicitation` as `{"form":{},"url":{}}`, and accepted this server's
  `2025-06-18` answer to its `2025-11-25` request.
- Against Estigia's own server under `claude -p`: `send_message` was refused `message-not-allowed`, because the
  elicitation came back `{"action":"cancel"}`, and no row was written. A non-interactive session refuses every send.
- Against Estigia's own server in an interactive session: the question appeared as a form showing the sender, the
  recipient, the issue it holds and the text. The enum rendered as a single select field listing the three
  `enumNames`. Choosing *Allow once* and accepting recorded the row, and the recipient took it at its next
  `PreToolUse`.

### Waking an idle Claude Code session

Setup adds a second `Stop` entry beside the ordinary one:
`"<estigia>" hook stop --agent claude-code --dialect claude-code --wait-mail`, with `"asyncRewake": true` and
`"timeout": 3600`. The process reads the `Stop` payload and resolves the run id exactly as the `PreToolUse` delivery
does (`hook::mail_recipient`: the `claude-code` dialect, no `agent_type`, a named session); with no such run it exits 0 at
once. Otherwise it asks the store for mail addressed to that run every second, up to 3595 seconds (the timeout less
five), and takes it with the same single-statement delivery as every other point. Mail found: the rendered text goes
to standard error and the exit status is 2. Nothing by the deadline: exit 0 and no row touched. A read that fails
three times running also exits 0, leaving the rows pending for the `PreToolUse` delivery; one failure is
tolerated because a send in progress can briefly hold the store.

**One waiter per run.** Every turn's `Stop` starts a waiter, so each one first writes a fresh token for its run to
the `store_meta` row `mail-waiter:<run id>` (one small row per run, overwritten, never deleted), and every poll
reads that row before it takes anything. A waiter whose token is no longer the current one exits 0 at its next poll
(within about a second) and takes nothing; the newest waiter keeps waiting. Waiters of different runs do not touch
each other's rows.

Measured on Claude Code 2.1.300, 2026-10-15, interactive TUI: a project `.claude/settings.json` `Stop` hook with
`asyncRewake: true`, a `timeout` and no `async` key keeps running after the turn ends; when it exits 2 while the
session is idle, Claude Code starts a turn by itself, the transcript shows "Stop hook feedback", and the model acts on
the hook's standard error. `Stop` fires again at the end of that turn, so the waiter re-arms after every turn. The
documented default command-hook timeout is 600 s, and `timeout` is enforced on `asyncRewake` hooks, which is why a long
one is written.

Measured live on Claude Code 2.1.300, 2026-10-15, with a session installed from this branch's
`estigia setup claude-code`: the session was left idle after one turn, and `estigia sessions send <its run id>
"<text>"` from another process woke it within 15 seconds ("Stop hook feedback"; the model answered the message as
another session's request). The row was stamped delivered, a new waiter was running afterwards, and when the session
was killed no waiter process from it remained.

What this does not cover is in [`honesty.md`](honesty.md).

What was not measured: *Allow for this session* and *Deny* in the live dialog (the scripted tests cover both), and
Codex and OpenCode.

## What does not exist yet

- Nothing refreshes `last_seen` on a gated write, so a session that holds no claim and runs longer than six hours
  reads `stale` while it is alive. Touching the registry from the gate is the next slice.
- No pid is recorded, and no host API is asked whether a session is alive.
- There is no Claude Code `UserPromptSubmit` or `PostToolUse` hook, since setup registers neither. A Codex or
  OpenCode session that makes no tool call receives nothing until it makes one, and a Codex session also at the end of
  a turn when the loop is on. A Claude Code session is woken while idle for up to 3600 seconds after its last turn
  ended (the waiter's `timeout`); after that, and in a session set up before the waiter existed, mail waits for the next
  turn's `PreToolUse`.
- Only Claude Code and Pi wake an idle session. Codex's `Stop` hook runs synchronously and continues the turn that is
  ending; it cannot start one. OpenCode's `steer` is used at the hook point, where the session is mid-turn;
  nothing watches the mailbox between hook points, because the plugin is told nothing when a row is written, and
  OpenCode's `generate` is not used. Codex `turn/steer` needs the daemon and is not built.

[`honesty.md`](honesty.md) records what this does not measure.
