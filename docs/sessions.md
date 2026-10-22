# The session registry

`estigia sessions [--json]` answers one question: who is around on this machine, which issue does each
hold, and how can it be reached. `estigia sessions --all` answers the wider one: every row the registry
has ever written, and `--limit N` with `--cursor TOKEN` answers it one page at a time. `estigia sessions
send` puts a message in another session's mailbox, and the hook point of each runtime delivers it. The
`send_message` and `list_sessions` MCP tools are the same two doors for an agent
([below](#the-mcp-tools)), and all four take the same addresses. The last section lists what does not
exist yet.

## What is recorded

`~/.estigia/state.db` carries a `sessions` table, one row per run id: the runtime, the host's session id, the
directory the session started in, when it started, when Estigia last saw it, when and why it ended, the
short name the registry minted for it, the build that registered it, how its adapter says an idle
session is woken, and the host pane the registering process was started in.

The **short name** is `<repository>-<n>` (`estigia-1`), minted when the row is first written from a
per-repository ordinal in the store. It is kept for the life of the run id and never reused, so it is what
a person can say and what a recipient can read back. A row written before names existed has none, and is
still addressable by its run id.

The **build** is the `estigia` binary that registered the row (`0.1.1 (ad3e8a07e0)`). A running session
keeps the build it started with, so a row can name a build older than the one answering; the answer says
so, and says that restarting the session, or the MCP server it runs, is what updates it.

The **wake** is what the adapter recorded: `poll` (Pi and OpenCode 2 ask the mailbox repeatedly while
idle), `stop-waiter` (Claude Code parks a process at the end of a turn), or nothing. Nothing is a fact
about the row, not about the runtime: an adapter that records no wake gets no claim that one exists.

The **pane** is the host identifier the adapter could observe, and only that: `HERDR_PANE_ID`, read
from the environment of the process that called `estigia session start`. It was measured on herdr
0.9.3 and 0.9.5, where a process inside a pane carries `HERDR_PANE_ID=w0:p1` — and on the OpenCode
process in that pane, which is the process the adapter runs in. A session started outside a pane
carries none, and a field the adapter cannot observe is absent rather than guessed. No service URL is
recorded: no adapter sees one, and the OpenCode service's `/api/session` answers 401 to an
unauthenticated caller (2026-10-21, `127.0.0.1:49374`), so a URL alone would not be a way in.

- `estigia session start` and the Claude Code and Codex `SessionStart` hook write or refresh the row. A session that starts again under the same run id
  keeps its first start time and is no longer ended.
- `estigia session end` and the `SessionEnd` hook stamp `ended_at` (and `end_reason`, when the host gave one) when the session is really over. A `new`, `resume`,
  `fork` or `reload` end changes nothing, as for the run pointer. The row is never deleted, so an ended
  session stays visible to the sender of a later message.
- Both writes are best effort: a registry that cannot be written does not stop a session from starting or ending.

The issue and the state shown for a session are read from its run pointer when the pointer names an issue. The
registry holds no claim of its own, and the tracker remains the authority on whether a claim is live.

## What `estigia sessions` shows

Each entry carries `name`, `run_id`, `runtime`, `session_id`, `status`, `issue`, `state`, `last_seen`,
`silent_for`, `repo_dir`, `worktree`, `ended_at`, `end_reason`, `waiting`, `undelivered_sent`, `build`,
`wake`, `pane` and `handoffs`, under `--json` as a `sessions` list beside `stale_after`, `of` (how many
rows the registry holds) and `all`. `pane` is absent when the adapter recorded none, and `handoffs` is
absent when the store holds none for that runtime in that repository; both absences are facts, not
nulls to be filled in. `handoffs` is a list of `{issue, from_run_id, at, projection}`, oldest first. `estigia sessions` prints the live rows only, one line each; `estigia sessions --all` prints
every row with its status. The live view is the answer to "who is around"; the full registry is opt-in
because it is mostly rows whose sessions are gone. `--limit` or `--cursor` replaces that whole list with
one page of it, as [Bounded session discovery](#bounded-session-discovery) describes.

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

`estigia sessions send <address> "<text>"` (or `--file <path>`) writes one row to the `messages` table of
`state.db`: the sender's run id when one is known, the recipient's run id, the text, and when it was
sent. The address is the same vocabulary the MCP tools take ([below](#addressing-a-session)). The sender is `--run-id`, or `--session-id` with `--runtime`, or the ambient `ESTIGIA_RUN_ID`; a person at a
terminal sends with none, and the message is shown as from a person.

- The recipient must resolve to one registry row. An address that names nobody live is refused with
  `message-no-live-session`, and one that names several with `message-recipient-ambiguous`; neither
  records anything. The mailbox itself still refuses a run id no row carries with
  `message-recipient-unknown`, which the address path reaches only when a row disappears between
  resolution and the write.
- A recipient that has ended is accepted: the row is written, it stays undelivered, and `estigia sessions` shows it
  against the sender (`undelivered_sent`) for as long as it stays so. An empty message and one over 8192 bytes are refused.
- Sending is a write to Estigia's own state, like `estigia goal stop-after`, not a write to the repository: it needs
  no claim and the gate classifies the shell line that runs it as it classifies any other `estigia` command. The row
  is the record of the send. The MCP tools in [the next section](#the-mcp-tools) reach the same row.
- `estigia sessions` shows `waiting` (rows addressed to this session and not yet delivered) and `undelivered_sent`
  (rows this session sent that nobody has taken) beside each session.
- A send is not a listing and takes no listing flag; [Bounded session discovery](#bounded-session-discovery)
  names the three that belong to the listing and the refusal they get beside `send`.

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
| OpenCode 2, idle | A poll every 10 seconds between turns | The plugin runs `estigia session deliver --peek --json`; with mail it calls `ctx.session.prompt` with `delivery: "queue"`, and only when that resolved it runs `estigia session deliver --through <id>`. See [Waking an idle OpenCode session](#waking-an-idle-opencode-session). |

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

### Waking an idle OpenCode session

OpenCode 2 only; OpenCode 1 (the function export) has no event stream the plugin uses and is not woken. The gate plugin
subscribes to `ctx.event.subscribe` (resubscribing after one second when the stream ends or fails, as the loop plugin
does) and reads four events: `session.created`, `session.execution.started`, and the terminal
`session.execution.succeeded`, `session.execution.failed` and `session.execution.interrupted`. A root session is idle
from a terminal event until the next `session.execution.started`; a session with a `parentID`, learned from
`session.created` or from `ctx.session.get`, is never polled, and neither is one that has not yet finished a turn.

While a session is idle the plugin ticks every 10 seconds (`WAKE_INTERVAL_MS`), never two ticks in flight for one
session, and a tick is `estigia session deliver --peek --json` followed, with mail, by `ctx.session.prompt` with
`delivery: "queue"` and then `estigia session deliver --through <id>`. A prompt that throws, a host with no
`session.prompt`, or a session that began executing while the peek was in flight, takes nothing, and the rows stay
pending for `execute.after`. If the take fails after the prompt resolved the rows are delivered a second time. Polling
stops at `session.execution.started` and when the plugin is disposed.

That a `queue` prompt starts a turn on an idle session is not read from OpenCode's own source: the loop plugin in
`src/setup/plugin.rs` (`enqueue`, reacting to `session.execution.succeeded`) relies on it, against OpenCode 2.0.28.
The wake is driven under `node` against a stand-in host and `estigia`. It was also run once live, on OpenCode 2.0.28,
2026-10-15, as an interactive session with a private server (`opencode --standalone`), whose configuration loaded only
this plugin. The session was left idle after one turn. A message sent with `estigia sessions send` then started a turn
with no input, and the row was stamped delivered. The prompt began with the same "not from your user" framing that
`PreToolUse` delivery uses.

What delivery refuses to do:

- It never turns an answer into a decision. A `PreToolUse` answer that denies or asks carries no mail and leaves the
  rows pending, and an answer that allows or stands aside carries the mail without changing the permission.
- It is delivered only to the session's own context. A sub-agent's `PreToolUse` call (one carrying an `agent_type`)
  and a Pi child session do not take the root session's mail.
- It is recorded when it is handed to the host, not when the model reads it: no host reports a read, so "delivered"
  is the observable event and there is no separate "read" stamp.

## The Pi parent-child channel

A Pi subagent runner child and its parent also talk directly, and this is not the mailbox: nothing is
written to the registry, nothing is persisted, and delivery is at-most-once. The runner opens a Node IPC
channel to each child it spawns; the child's `estigia_parent_message` tool sends a notification or a query
over it, and the parent's `estigia_subagent_reply` answers a query over it. The parent steers a child with
`estigia_subagent_message`, an RPC `steer` on the child's standard input. The tools are described in
[`what-it-writes.md`](what-it-writes.md).

A foreground `estigia_subagent` call blocks the parent's turn, so a query from that child is refused at once
rather than left to time out; answering a query, like steering, needs the child to have been started with
`background: true`. Measured live on 2026-10-15 with Pi 1.1.0, a real `pi` parent and child under a
temporary home and an agent definition `worker`: the IPC channel and the child's
`estigia_parent_message` worked and the query reached the parent's runner, but the parent had launched the
child in the foreground, could not call `estigia_subagent_reply`, and the query failed after the full 120
seconds. That is what the background mode and the immediate refusal answer. A second live run the same day used
`background: true`. The child's query woke the idle parent, `estigia_subagent_reply` returned the answer as the
child's tool result, and the child's final answer reached the parent as a follow-up, within 15 seconds.

A message sent while the other side is gone is lost and the sender is told when it can be. A query pending
when the child ends is dropped, and a reply to it is a tool error. A query pending when the parent ends
fails in the child as soon as the channel closes; one the parent never answers fails after 120 seconds.
When the child gives up, it tells the parent first. The parent then forgets the query and tells its model that
the child stopped waiting, so a later reply is refused rather than reported as answered. If that notice never
arrives, the parent forgets the query five seconds after its own 120 seconds. A reply is forgotten only once the channel has accepted it, so a reply that fails to
send can be sent again.
Neither side's session is changed by what arrives: a message is text for a model, not an instruction the
gate treats differently.

## Handing an issue to another runtime

`handoff` (and `estigia handoff <issue> --to <runtime> --run-id <run>`) records `{issue, runtime,
from_run, at}` in `~/.estigia/state.db` and projects `dev:<runtime>` on the issue. It is the one call
the 2026-10-21 operator request needed: the issue, who handed it over, and where it went. It does not
message a session and does not claim anything — the recipient still claims through `claim`.

- **The row** is keyed by `(repo_dir, issue, runtime)`, so a retry of the same handoff refreshes who
  made it and when rather than reading as two handoffs. `projection` records `landed` (the label was
  read back) or `deferred` (it was queued).
- **The label** is the protocol: exactly one `dev:<runtime>` on the issue, the same label a claim
  projects. It is written and read back twice; a read-back that never agrees is a write failure, not
  an answer. While the git operation window is closed the projection is queued as a `reconverge` in
  the outbox and the answer says `projection: deferred` with the queue position; a later `flush`
  projects it, reads it back, and marks the row `landed`. Re-running `handoff` is how a caller
  reconciles its answer.
- **A held issue is refused** with `handoff-issue-held` and nothing is written, whichever runtime
  holds it — the local ledger is what is read, since that is where a claim lives. A handoff is a
  destination, not a claim: overwriting the holder's `dev:` label would tell every reader the wrong
  runtime. Release the issue first. A queued projection is checked again when the outbox flushes it,
  and there the one holder that does not refuse it is a run of the runtime it was handed to: the
  recipient took it before the label landed.
- **The label is explainable.** `claim` refuses a `dev:` label the store cannot explain, and a
  handoff row is that explanation — so the recipient's `claim` is not blocked by the handoff that
  sent it there.
- **A later session sees it.** `SessionStart` of a session of that runtime whose repository is that
  directory names each waiting issue, who handed it over and when, and says a handoff is a request,
  never a claim. A row whose projection is still `deferred` says its `dev:` label is queued and not
  yet read back, so a label that may never land is not presented as one that did. `list_sessions` and `estigia sessions --json` carry the same list as `handoffs` on
  every row of that runtime in that repository.

The directory recorded is the one the handoff ran in. A session started in a *worktree* of that
repository registered a different directory and does not see it; hand over from where the recipient
will run, or address the recipient directly with `send_message`.

## Addressing a session

`send_message`, `list_sessions`, `estigia sessions send` and `estigia sessions` all take one address
vocabulary. It is resolved against the registry **at the moment of the call**, over the live rows the
listing shows, never against an earlier listing — a session that registered a second ago is found.

| What a person says | What it names |
|---|---|
| `opencode` | every live session of that runtime |
| `runtime:opencode` | every live session of that runtime, and nothing else — an unknown runtime is no match, never a short name |
| `opencode@estigia` | that runtime in that repository (`@` takes a repository name or a directory) |
| `#96` | every live session whose pointer names issue 96, in any repository |
| `estigia#96` | the same, in that repository only |
| `estigia-2` | one session by the short name the registry gave it |
| `opencode-sesea451598815b2` | one session by its run id |

A bare number is an issue number, as it always was. A run id or a short name names that row whether it
is live or not — a sender who names it exactly meant it, and a message to an ended session is accepted
and stays undelivered. Every other form is about who is around, so it reads live rows only.

- **One match** is delivered to, and the answer names it: short name, run id, runtime, repository, the
  issue it holds, how it is woken, and that the row is *queued for the recipient's next hook point*. No
  answer claims a wake that happened; whether the recipient woke is not observable here.
- **No match** is refused with `message-no-live-session`, which names the address, how many sessions are
  live, and the closest ones — same runtime, or same repository — one line each, so the sender can relay
  it to a person without parsing anything.
- **Several matches** are refused with `message-recipient-ambiguous`, listing every candidate the same
  way. Nothing is sent. Ambiguity is a question, not a guess.

`send_message` also takes `repo_dir`, which narrows `to` to sessions registered in that directory: it
is how a run says *the live OpenCode session of `investora`* in one call. The comparison is by
directory, so a trailing separator or a symlink is the same checkout, and a session that registered no
directory is in none. `reply_to` refuses `repo_dir`, because a reply's recipient is the sender of the
message being answered. The CLI has no `--repo-dir` on `sessions send`; `opencode@investora` names the
same session there.

**A message that tells a session to work another issue is refused.** If the text carries a
`flow dev <n>` instruction and the recipient's pointer names an issue other than `n`, the send is
refused `message-recipient-holds-another-issue` before the person is asked and before any row is
written. A recipient holding nothing, or holding `n`, is sent to. This is a refusal rather than a
warning because a warning is a message that still arrives, and two runs believing they own two
different things in one session is the state this check exists to prevent. The instruction is read
from the words of the message, so a phrase quoted as something *not* to run is refused too; the
person can rephrase it. `estigia sessions send` applies the same rule.

A delivered message opens with the sender in the same form — `From estigia-2 (opencode, estigia), \`run-id\``
— and carries the id of its own row. Answering is `send_message {reply_to: <id>, text}`: the recipient is
the session that sent it, with no run id to copy. A row a person sent has no session to answer and is
refused `message-reply-has-no-sender`; an id the mailbox does not hold is `message-reply-unknown`.

`list_sessions` with no arguments is the bounded default: the live sessions only, one line each, beside
`live_count`, `of` (how many rows the registry holds), `build` and `skew` (which live rows registered
with another build). With `address` it answers `matched` and `live`, plus `closest` when nothing matched.
With any of `status`, `runtime`, `run`, `session_id`, `repo_dir`, `limit` or `cursor` it answers the
bounded, versioned page described in [Bounded session discovery](#bounded-session-discovery) — that is
the opt-in for the whole registry.

## The MCP tools

`send_message` takes `run_id` (the sender, this run's id, held to the shape every tool's `run_id` is),
`to` (an address) or `reply_to` (a message being answered), and `text`, and calls the same
`mailbox::send` as `estigia sessions send`, so the empty-text and size refusals above are the same ones.
Both tools answer from the local store and bypass the tracker transport, so they need neither a tracker
nor a repository host transport and are not binding operations.

Consent follows who started the send. A person at a terminal is never asked. An agent's `send_message` is asked
about before any row is written, through MCP elicitation (protocol `2025-06-18`):

- At `initialize` the server records whether the client sent a `capabilities.elicitation` object. Its shape is not
  inspected: `{}` and `{"form":{},"url":{}}` both count.
- With no such capability, and no session grant, the send is refused with `message-needs-a-person` and nothing is
  recorded.
- Otherwise the server sends `elicitation/create` with a message naming the sender, the recipient in the
  addressable form (`estigia-1 (run id), opencode in estigia, holds #96, seen just now`), and the text, and a
  schema with one required string `choice`: `allow_once`, `allow_session` or `deny`.
- `accept` with `allow_once` sends. `accept` with `allow_session` sends and remembers the grant, keyed by the
  sender's run id, until that server process exits (one MCP server process is one agent session). Everything else
  is refused with `message-not-allowed` and nothing is recorded: `deny`, `decline`, `cancel`, an unknown choice, an
  error response, or no answer in 15 minutes. A question that could not be put or whose answer could not be read
  (a failed write, a read error, a closed input) is refused `message-question-failed` instead, because the person
  was never asked and the refusal must not read as their decision.
- An address that names nobody live, or several, is refused before the question is put, with the candidates
  and nothing recorded.
- While the server waits, any other line from the client is queued and served after the tool call answers. Requests
  are still served one at a time.
- The tool has no argument that skips the question, and the binary cannot tell whether a person asked the agent to
  send. A person who wants no prompt sends from the CLI.

The issue the question shows is read from the recipient's pointer, for display only; it is not what decides
the recipient. The address does, and a pointer is never a claim.

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

## Bounded session discovery

`list_sessions` with any option, or `estigia sessions` with `--limit` and/or `--cursor`, answers a **page**
of the registry rather than the whole list. Both are adapters of one bounded read, so the accounting and the
cursors are the same from either. The answer is a versioned object: `version`, the page's `sessions`, its own
accounting (`limit`, `returned`, `matched` — the rows the selectors matched as this page was taken — and
`more`), a `next_cursor` that is `null` once the snapshot is exhausted, and the snapshot's identity
(`taken_at`, and `stale_after` as before). The page reads the clock once, for `taken_at` and a cursor's age; a
clock that will not answer is refused `clock-unreadable`, never read as no time. A call with no page option on
either door is unbounded, and a caller that asks for nothing still gets every row.

The selectors are `status` (`live`, `stale`, `ended` or `unknown`), `runtime`, `run` (a run id),
`session_id` and `repo_dir`. Each is at most 256 bytes: a longer one is refused when it is given, with
`sessions-selector-too-large`, because a selector is carried in the cursor the page issues and five at the
bound hex-encode to 3,029 of the 4,096 bytes a cursor may be, leaving room for the run id and the store
identity. `limit` is a positive whole number, default 50 and at most 500; a value that is
not one — including an explicit `null` — is refused by the server's argument boundary (`flags_for`), which
every tool call passes through before it is dispatched, and one above 500 by the page itself, rather than
clamped. `cursor` is the `next_cursor` of the page before. The CLI takes no selector: `--limit N` is a page
size and `--cursor TOKEN` continues a walk, and it asks for `status=live` unless `--all` is given, when it
asks for every status. `estigia sessions --limit 20` therefore walks the live rows and `estigia sessions
--all --limit 20` walks the registry; a cursor issued under one is refused `sessions-cursor-mismatch` under
the other. A `limit` above the maximum is `sessions-limit-too-large` from either door, and `--limit 0` is
refused by the parser before anything is read.

**The listing flags belong to the listing door.** `--all`, `--limit` and `--cursor` decide which rows a
page holds, and `send` has no rows: given beside it they are refused `sessions-send-takes-no-listing-flag`
before the message is written, and the refusal says the same call without the flag delivers. The
`sessions` position is where they parse; after `send` the parser refuses them as unexpected arguments.

**What each door prints.** `list_sessions` answers the page's JSON. `estigia sessions --json` with a page
flag prints the same bytes, the compact JSON the tool's caller sees; without `--json` it prints each row in
the line the unbounded list uses, then one footer naming how many rows the page returned, how many matched,
whether more remain, and the next cursor when there is one — `2 of 5 row(s) on this page; more remain — the
next cursor is <token>`, and `1 of 5 row(s) on this page; nothing more remains` at the end of a traversal.
A traversal continues by giving `--cursor <token>` again; `--all` must stay as it was, because the cursor
is bound to the selectors it was issued for, while `--limit` may change from page to page — the cursor
names the last row returned, not a page size.

**Ordering and the snapshot.** A bounded page's snapshot is the registry rows with an immutable start, and a page
orders them by `(started_at, run_id)`; the snapshot is every such row whose `started_at <= taken_at`. `started_at`
is written once, when the session first registers, and never changes; `run_id` is unique, so the pair is a total
order with a unique tie-breaker. Ordering by `last_seen` (a heartbeat) or by `status` (derived from a heartbeat
and from the clock) is what this avoids: a heartbeat mid-traversal would move a row past the cursor and omit it,
or move it back and duplicate it. A session that ends, restarts or is otherwise mutated between pages keeps its
key and its place. Membership is frozen only to the second, because `started_at` has second granularity: a
session that registers in the same second as `taken_at` — after that page was taken — is admitted by
`started_at <= taken_at` and can appear in a later page, while one that registers after `taken_at` is not in the
snapshot and never appears. A pointer-only
run — a run pointer with no registry row — has no immutable start, so it is not in the snapshot at all; it is not
bounded, and the no-omissions property is a property of the registry.

**What a selector reads is not frozen.** `taken_at` freezes the set of rows and their order, and that part is
sound; the value a selector compares is not in the snapshot, so it cannot be. `status` and `last_seen` are derived
from the row as each page reads it, which is the only place they exist, so a row whose status changes between
pages can leave a `status`-filtered traversal, and `matched` is the count as that page was taken rather than a
count held constant across the walk. A caller that needs every row matching a `status` — a complete set rather
than a walk — should read the whole list (no page flag, on either door) and filter it itself: that is one
read of the registry against one clock, so the statuses in one answer agree with each other. A bounded traversal
is for walking a large registry a page at a time; a selector on it is a convenience that reads the row as each
page is taken.

**The cursor is bound, not trusted.** A `next_cursor` carries the response `version` (which fixes the
ordering), the selectors it was issued for, the snapshot's `taken_at`, the key of the last row returned, and an
identity of the store file — its device and inode on Unix, its volume and file index on Windows. A path with no
store answers `absent`, and a store whose identity cannot be read answers `unknown`. A later call must repeat the
same selectors; the cursor continues one snapshot, not "the same query run again". The CLI keeps every code in
the table below, because it maps `--limit` and `--cursor` onto the page and answers what the page answers. The
cursor is refused, never silently restarted, when:

| Refusal | When |
|---|---|
| `sessions-cursor-too-large` | The token is longer than a cursor can be, refused before it is decoded. |
| `sessions-cursor-malformed` | The token cannot be decoded, or is not the JSON a cursor is. |
| `sessions-cursor-tampered` | The token's digest does not match its payload: it was edited after it was issued. |
| `sessions-cursor-mismatch` | The call's selectors or the response version differ from the cursor's. |
| `sessions-cursor-store-replaced` | The store file the cursor was issued against is gone or is another file. |
| `sessions-cursor-expired` | The cursor is older than `CURSOR_TTL_SECONDS` (15 minutes). |
| `sessions-limit-too-large` | `limit` is above the maximum. |
| `sessions-selector-too-large` | A selector value is longer than the 256 bytes one may be. |
| `sessions-unregistered-not-bounded` | `status=unregistered`: a pointer-only run is not a registry row and is not in the bounded snapshot; the full list still shows it. |

The digest detects corruption and an edit that does not recompute it. **It is not a MAC, and possession of a
cursor is not authorization**: anybody who can read a `next_cursor` can hand it back, and the answer is the same
rows the person it was issued to would get. Nothing here is a secret.

**Bounds are stated, not implied.** What bounds a bounded answer is the page size (`limit`, default 50 and at most
500) and the row shape; there is no fixed byte ceiling, because 500 rows of session records can be larger than any
single number written here. A caller that passes no options is not bounded at all and pays for every row.

**Two things a snapshot's `status` is not.** It is computed from the row as the snapshot was taken, so it is not
current liveness: a session shown `live` may have died since, and one shown `stale` may be alive but holding no
claim. And it is not claim authority: whether a claim is live is read from the tracker, and the registry holds no
claim of its own.

## What does not exist yet

- Nothing refreshes `last_seen` on a gated write, so a session that runs longer than six hours without a
  `SessionStart` reads `stale` while it is alive, and no row is ever archived: the full registry grows
  without bound. A heartbeat from the gate, a configured retention horizon, and a default view that hides
  what it archived are the next slice of the session-registry work.
- No pid is recorded, and no host API is asked whether a session is alive. `wake` is what the adapter named
  when it registered the session; nothing here asks the host whether that is still true.
- There is no Claude Code `UserPromptSubmit` or `PostToolUse` hook, since setup registers neither. A Codex or
  OpenCode session that makes no tool call receives nothing until it makes one, and a Codex session also at the end of
  a turn when the loop is on. A Claude Code session is woken while idle for up to 3600 seconds after its last turn
  ended (the waiter's `timeout`); after that, and in a session set up before the waiter existed, mail waits for the next
  turn's `PreToolUse`.
- Only Claude Code, Pi and OpenCode 2 wake an idle session. The wake each one recorded is in its row; a row
  with no wake says so rather than claiming one. Codex's `Stop` hook runs synchronously and continues the turn that is
  ending; it cannot start one. OpenCode 2 is woken by a poll, not by a notice that a row was written, and
  OpenCode 1 is not woken; OpenCode's `generate` is not used. Codex `turn/steer` needs the daemon and is not built.
- A handoff is never consumed or expired. Every later session of that runtime in that repository sees
  it at `SessionStart`, whether or not anybody took the issue; nothing records that it was read or
  declined. A store that grows one row per `(repository, issue, runtime)` is the whole cost until a
  retention rule exists.
- `handoff` records a reachability hint and does not deliver through it. There is no `herdr` prompt,
  no terminal injection and no read receipt: delivering the instruction is `send_message`, and the
  host-specific path stays outside Estigia because it bypasses consent and stamping.
- No adapter records an OpenCode service URL. The session id and the pane are what the adapter can
  observe; the service's own API answered 401 to an unauthenticated caller on 2026-10-21, so a URL
  without the adapter's credential would not be a way in even if it were recorded.

[`honesty.md`](honesty.md) records what this does not measure.
