# The session registry

`estigia sessions [--json]` answers one question: which sessions has this machine seen, which issue does
each hold, and is each still there. `estigia sessions send` puts a message in another session's mailbox, and the
hook point of each runtime delivers it. The last section lists what does not exist yet.

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
  is the record of the send. No MCP tool mirrors it, because every MCP tool is a tracker operation and a mailbox row
  is not one.
- `estigia sessions` shows `waiting` (rows addressed to this session and not yet delivered) and `undelivered_sent`
  (rows this session sent that nobody has taken) beside each session.

Delivery is `estigia session deliver --session-id <id> --runtime <runtime>`, which prints the pending rows as one
text block and stamps each `delivered_at` in the same statement, so two callers racing for one session split the
rows and never both receive one. It prints nothing when none wait. A store that cannot be read or written delivers
nothing and leaves the rows pending; a delivery that failed is never recorded as made.

| Runtime | Delivery point | How |
|---|---|---|
| Claude Code | `PreToolUse` hook | The hook's answer carries the text as `hookSpecificOutput.additionalContext`, beside whatever it decided. |
| Codex | `PreToolUse` hook | The same hook and the same answer, registered with `--agent codex`. |
| Pi | `before_agent_start` | The extension runs `estigia session deliver` and adds the text as its own `estigia-mail` prompt section. |
| OpenCode 1 | `tool.execute.after` of the gate plugin | The text is appended to the tool's string result, after the plugin registered the session with `estigia session start`. |
| OpenCode 2 | `execute.after` of the gate plugin | `ctx.session.prompt` with `delivery: "steer"` (the accelerator); when the service is absent or throws, `ctx.session.synthetic`. With neither function the mailbox is not read and the rows stay pending. |

What delivery refuses to do:

- It never turns an answer into a decision. A `PreToolUse` answer that denies or asks carries no mail and leaves the
  rows pending, and an answer that allows or stands aside carries the mail without changing the permission.
- It is delivered only to the session's own context. A sub-agent's `PreToolUse` call (one carrying an `agent_type`)
  and a Pi child session do not take the root session's mail.
- It is recorded when it is handed to the host, not when the model reads it: no host reports a read, so "delivered"
  is the observable event and there is no separate "read" stamp.

## What does not exist yet

- Nothing refreshes `last_seen` on a gated write, so a session that holds no claim and runs longer than six hours
  reads `stale` while it is alive. Touching the registry from the gate is the next slice.
- No pid is recorded, and no host API is asked whether a session is alive.
- There is no Claude Code `UserPromptSubmit` or `PostToolUse` hook, since setup registers neither; a Claude Code or
  Codex session that makes no tool call receives nothing until it makes one. The same holds for an OpenCode session
  that makes no tool call.
- No accelerator wakes an idle session. OpenCode's `steer` is used at the hook point, where the session is mid-turn;
  nothing watches the mailbox between hook points, because the plugin is told nothing when a row is written, and
  OpenCode's `generate` is not used. Codex `turn/steer` needs the daemon and is not built.

[`honesty.md`](honesty.md) records what this does not measure.
