# Sessions, mail and the records a run leaves

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the session registry, the mailbox and the local records do not establish. The index says which area holds which question.

## What `estigia changes` does not do

`estigia changes` is the one read of uncommitted work per checkout, and `estigia prune` lists every
checkout it keeps (unit tests build real repositories with a dirty orphan checkout, a held one, a
directory git cannot read and a repository with no commits, and one pipe test drives the binary). What it
does not measure:

- **Two surfaces render the listing, and neither has run live.** The Pi `/estigia-changes` command and
  widget (`ctx.ui.setWidget`) are driven by node against a stand-in binary whose `setWidget` is a
  recorder; the Claude Code `/estigia-changes` slash command is tested as a file `setup` writes and
  removes, not as a command Claude Code ran. Not built: the OpenCode TUI plugin, the Claude Code
  status-line segment, and a Codex notice of its own. OpenCode, Claude Code and Codex see the
  session-start notice and nothing else, and none of it has run in a live session.
- **The session-start notice is the binary's text, not a per-runtime surface.** It counts uncommitted
  files with one `git status` per linked checkout under a three-second budget and names what it did not
  reach; the base checkout is never named, and a checkout a pointer names but git does not register is
  not read.
- **The `done`/`release` notice is tested on its function, not through a tracker**, and reads the
  pointer's checkout, so a run whose pointer recorded none gets no notice. It is a notice only
  (`lifecycle.md` documents the decision not to refuse).
- **No session-end notice.** `SessionEnd` answers `{}` and no runtime is known to show that output.
- **`orphan` is a readable run pointer, not the claim**; the issue's state is not read.
- **Line counts are Git's, and partial.** An untracked file is counted only when it is a text file of at
  most one mebibyte; a binary file, or one counted against a checkout with no commits, has none. An
  unmerged path is marked `M`. **Fifteen seconds is the bound on each Git call**, chosen rather than
  measured.

## The feed

`estigia feed` ([`feed.md`](feed.md)) projects the local store and records nothing (eleven unit tests seed a store with
claims, transitions, a marker entry in a second repository and a message, and drive each filter, the paging, the
refusals and the absent store). The `feed` MCP tool calls the same function; two tests hold it to the command's rows
for one seeded transition and to the unknown-repository refusal, not to every filter. What it does not measure:

- **It sees only what the store recorded.** A merge is read live from the tracker and is not an event; a consent
  prompt, a turn that ended and a gate decision older than the 30-day retention are not in the store. A state change
  made before the `transitions` table existed has no event. Every answer says so in `cannot_see`.
- **Quiet is not proof of a quiet run.** Runtimes with no gate (`zcode`, `deepseek-harness`, `agents`) or a
  pre-tool-only gate (`cursor`) produce no lifecycle events; the feed lists none for them and cannot tell that from
  a run that did nothing. A Cursor run registered before `AGENT_RUNTIMES` listed `cursor` is filtered as
  `claude`; one registered since is filtered as `cursor`.
- **The window is bounded, the scan is not indexed.** `entries` and `transitions` carry no index on time, so each
  call evaluates the time of every row before applying `--since`; not measured against a large store. An `entries`
  row whose `created_at` SQLite cannot read as a time (the compact `20260726T005435Z` form) is not listed.
- **The snapshot is a filtered re-read.** A row written in the same second as `taken_at` can join a later page,
  as with `estigia sessions`; the cursor is a digest, not a MAC. A `runtime` is the one the run id names, so a
  message is filed under its sender's runtime.
- **There is no link and no session start or end event.** Sessions are in `estigia sessions`; the feed lists
  transitions, markers and messages. A message's recipient may be a run that never registered.
- **Previews are tested for the three bodies seeded, not for every marker field.** A preview is the kind and the
  issue; nothing of a marker's other fields is shown without `--bodies`.

## Where

`estigia where` ([`where.md`](where.md)) is tested against a two-repository store and a stand-in tracker (fourteen unit
tests for the answer, six for the resolver, three for `tasks` and the identity parser); the live `gh` reader is not
exercised by any of them. What it does not measure:

- **The holder is the registry's, not the timeline's.** A run on a runtime with no gate leaves no pointer, so an
  issue it holds shows no holder; the store's claim markers are not consulted for it.
- **A Linear key is answered only in part.** `issue`, `workflow`, `holders` and `messages` are read; the
  transitions, tasks, clauses, queued writes, pull request and review stay `unavailable`, because the local store
  keys them by repository and number and a key has neither. `holders` matches the key alone, with no repository
  check. The Linear read is measured against the stand-in CLI
  (`a_finished_issue_is_read_as_closed_where_assist_would_refuse_it`), never the live API.
- **The repository of a pointer is read from its checkout's `origin`.** A checkout with no GitHub `origin`
  contributes no candidate; a bare number held only there falls back to the caller's checkout.
- **The tracker reads are two `gh` calls with no retry.** A rate limit or a missing login leaves `issue` and
  `pull_request` `unavailable` with the reason, and `bound_to_live_head` `null`.
- **Message previews carry bodies.** Unlike `feed`, which omits them by default, the last messages are shown with
  200 characters, because the question is what the sessions said.
- **A bare-number reference to `tasks` that no repository holds** answers `unavailable` only when the store or
  the repository is missing; a repository the store knows with no tasks recorded is a real, empty checklist.

## The session registry

`estigia sessions` lists the rows `estigia session start` and `estigia session end` write to the `sessions`
table, joined with the run pointers (five pipe tests drive the binary through a started, a crashed, an
unregistered and an ended session, a bounded walk with neither omission nor duplicate, each refusal a page
answers and an empty registry offering no cursor; four store unit tests). What it does not measure:

- **`last_seen` is refreshed by a session start and by a verified claim, not by every gated write**, so a
  session holding no claim past the six-hour bound reads `stale` while alive (the bound errs toward showing a
  live session as stale). **Only sessions whose host calls `estigia session start` or fires a registered
  `SessionStart` hook are registered**: Pi, Claude Code and Codex do, OpenCode's gate plugin registers a
  session the first time a tool call completes, and the rest appear as `unregistered` only while their
  pointer names an issue. There is no pid and no host query.
- **A pointer is read through `holdings`, which finds a file only under the name its run id gives itself**: a
  run id with an uppercase letter is filed under a digested name, so its pointer and issue are not shown
  (minted ids from the hosts measured are lowercase; not fixed).
- **A bounded snapshot is a filtered re-read, not one consistent read of the machine.** Each page keeps rows
  whose `started_at <= taken_at`; the ordering key is immutable, so a traversal has neither omissions nor
  duplicates under the tested mutations, but `status` and `last_seen` are re-derived per page. Membership is
  frozen only to the second (a session registering in the same second as `taken_at` can join a later page:
  `a_session_registered_in_the_same_second_as_taken_at_can_join_the_walk`). The cursor's digest detects
  corruption and an edit that does not recompute it; it is **not a MAC**, and possessing a cursor is not
  authorization. Where `store_identity` cannot be read (the Windows arm when its handle read fails, any other
  platform) every such store shares `unknown`, so the cursor's store check cannot tell two apart.
- **A `status`-filtered traversal is not a complete set** (the value is read afresh per page, so a row that
  ends between pages can leave the walk; `a_status_filtered_traversal_reads_the_status_afresh_and_can_omit_a_changed_row`
  asserts the omission), and **a bounded page covers the registry rows only** (a pointer-only run has no
  immutable start; `status=unregistered` is refused `sessions-unregistered-not-bounded`). A bounded answer
  has no fixed byte ceiling: it is bounded by `limit` (default 50, at most 500) and the row shape; one real
  call on 2026-10-18 returned 433 sessions as 141,698 UTF-8 bytes (a response size, not a token estimate).
- **The derived "task k of n" is matched by issue number alone.** A session row names no repository
  identity, so `fields: status` reads the checklists of the whole store and drops an issue number that two
  repositories both have a checklist for rather than guess which one the run holds; the row then shows nothing.
  A `status_text` is what the run wrote, redacted and capped but not checked, and is never evidence.
- **The pane is one environment variable, read once.** `HERDR_PANE_ID` was measured on 2026-10-22 inside herdr
  0.9.3 and 0.9.5 (the OpenCode processes in `w0:p1` and `wE:pA` carried it, matching `herdr pane list`). Not
  measured: a host exporting the same name for something else, a Windows pane, or a pane id that outlives its
  pane. The row records what the registering process inherited, a locator rather than a way in. **No service
  URL is recorded for any runtime**: the OpenCode service seen on 2026-10-21 (`127.0.0.1:49374`) answered
  `GET /session` with HTML and `GET /api/session` with 401, so a recorded URL would not have been a way in.

### `estigia resume`

Unit tests cover the command per runtime, each refusal, liveness from a row with no process, from another machine, with a dead pid and with a live one, the `--exec` refusals, the command and directory `--exec` prepares and the exit code it returns through an injected launcher, a stale OpenCode run with its worktree, an ended run whose
claim is live and one whose claim lapsed, and a worktree with a dirty file and an unpushed commit. What it does not
measure:

- **No resume command was run against a live session.** On 2026-10-27 only the installed help was read:
  `claude --resume [value]` (Claude Code 2.1.311), `codex resume [SESSION_ID]` (codex-cli 0.159.2) and
  `opencode --session, -s` (OpenCode v2.1.4) exist. That the same session id and context come back was not
  measured for any of them. Pi and Cursor have no command.
- **The shape check is a heuristic.** A UUID for `claude` and `codex` and a `ses_` prefix for `opencode` are what
  the sessions seen so far looked like; a Cursor session whose id is a UUID is registered as `claude` and gets a
  `claude --resume` command that will not find it.
- **The claim is read from the store's entries by a substring of the run id**, which imported entries need
  because their `run_id` column is empty. A prefix collision would only add a row the reducer then drops.
- **The push state is the commit count against the branch's upstream**, not `branch_identity_verdict`, and it is
  read from the last fetch; it never asks the remote.
- **A pid can be reused.** The probe asks whether a process with the recorded pid exists on this machine; a
  session that ended and whose pid an unrelated process took reads as `live`, and `--exec` refuses it. The safe
  direction, and not fixed: the process start time is not recorded.
- **The recorded process is the nearest ancestor that is not a shell, found by `ps`.** From the parent of the
  registering hook or command, `ps -o ppid=,comm= -p <pid>` is walked up at most eight levels, skipping `sh`,
  `bash`, `zsh`, `dash`, `fish`, `ksh` (with or without a leading `-`) and `estigia`; the first other ancestor is
  recorded. A `ps` that fails or cannot be parsed, a walk that reaches pid 1, and eight levels of wrappers all
  record no token, so liveness is `unknown` and `--exec` is refused. The limits: an agent launched through a
  wrapper that is not a shell (`env`, `node` running a script, `npx`, a supervisor) is recorded as that wrapper, and
  the row then reads `gone` or `live` for the wrapper's life rather than the agent's; a wrapper that outlives the
  agent reads `live`, which refuses safely. **None of this was measured against real hosts**: the walk is tested
  on fixture chains, and which ancestors Claude Code, Codex, OpenCode and Pi put between themselves and their
  hooks was not read.
- **Windows records no process**, so every row there is `unknown` and `--exec` is always refused. A row
  registered before the process columns existed is the same.
- **`--exec` was never run against a real agent.** Its launcher is injected in the tests, and the native
  commands remain unverified against the installed CLIs, as the first bullet says.
- **The stored binary is the one seen at registration.** The registry records the program of the agent process
  (`/proc/<pid>/exe` on Linux, `ps -o comm=` elsewhere, so a full path on macOS and possibly a bare name on other
  Unixes) and `resume` uses it only when it is an absolute path whose file name is exactly the runtime's command.
  A Node-hosted agent that `ps` names `node`, a binary since moved or upgraded, and a row registered before
  migration 20 all fall back to the bare name. Which program each host's `ps` entry names was not read on real
  hosts.
- **Pi's session-file path comes from `ctx.sessionManager.getSessionFile()`**, which the installed Pi's type
  declarations carry; the Node driver supplies a fake of it, and no live Pi session was resumed with the path.
  Only a root session files its path: a child process registers under its root's id, and its own file would be
  wrong for that row. A Pi row registered before migration 20, or by an extension older than this one, has no file
  and `resume` refuses it.
- **OpenCode still never sends `session end`, and no event was found that could.** The strings of OpenCode 2.1.4
  list `session.created`, `session.execution.*`, `session.idle`, `session.archived`, `session.deleted` and the
  like, but no event of a session being closed; `deleted` and `archived` are actions on a stored session, not the end
  of an open one. The plugin's dispose hook ends the instance, and nothing shows it fires when the process exits. No
  hook was shipped on a guess, so OpenCode rows only ever go `stale`; a live OpenCode session ending was not
  observed.
- **`resume` is CLI only, and `SessionStart`'s "holds no issue" text does not yet point at `claim`.**

## The session mailbox

`estigia sessions send` writes a `messages` row, `estigia session deliver` takes and stamps the pending rows,
and `before_agent_start` (Pi) and `PreToolUse` (Claude Code, Codex) call it (store unit tests, pipe tests through
send, list, deliver, refusal and an ended recipient, one pipe test through the Claude Code and Codex hook, one
unit test that a refusal keeps the mail waiting, Node tests on the generated Pi extension). What it does not
measure:

- **No runtime was run live for mail delivery.** One neighbouring fact was measured instead, on Claude Code
  2.1.319 (2026-11-07) against a probe hook answering `ask` with both a reason and `additionalContext`: when the
  person answered Yes, the model quoted the `additionalContext` and never the reason; when the person answered No,
  the model received only the rejection and no `additionalContext`. No `ask` Estigia answers carries mail on
  the strength of that now: `Issue required` no longer asks. Whether the same field beside an allow carries mail to the model, and whether the Codex build in use reads that field from
  `PreToolUse` at all, are taken from documentation; a Codex session that ignores the field receives nothing
  and the row is nonetheless stamped delivered, because the binary cannot tell. The Codex terminal UI and
  daemon were not driven. Pi's section lasts one turn (whether this Pi keeps it out of the stored
  conversation was not measured). Beyond `PreToolUse` only `Stop` delivers (Claude Code's waiter, Codex's loop
  `Stop`), so a session that makes no tool call and ends no turn is not reached; `UserPromptSubmit` and
  `PostToolUse` are not registered.
- **OpenCode was driven under `node` against stubs** (OpenCode 1's `tool.execute.after` appending to a string
  result, OpenCode 2's `execute.after` steering through a stub `ctx.session.prompt`, and the
  `ctx.session.synthetic` fallback). Unverified: that OpenCode 1 puts `output.output` in front of the model,
  that OpenCode 2's `ctx.session.prompt` accepts `delivery: "steer"` mid-turn and that every OpenCode 2 build
  has `execute.after`. A result whose `output` is not a string is left alone with its mail pending. **A known
  loss window**: when `ctx.session.prompt` or `synthetic` exists and throws after the rows were taken, the
  rows are stamped delivered that the host refused and the sender sees them delivered; no small fix exists,
  since take and stamp are one statement (a delivery is always stamped before the host acknowledges it, and a
  killed hook process loses the message as delivered; delivery never records "read", which is the recipient's
  own act, below). OpenCode costs one extra
  process per tool call.
- **The peek-then-stamp handover (Pi and OpenCode 2 idle wakes) is at-least-once, not exactly-once.** Store
  tests inject the failures: a poll that peeked and died before `--through` leaves every row pending for the
  next poll after a restart, so a host that accepted the mail before the crash is handed it again; a
  `--through` retried after its answer was lost stamps nothing twice and keeps the first stamp; two pollers
  stamping one peek at once deliver each of 40 rows to exactly one
  (`a_handover_that_dies_before_its_stamp_leaves_the_mail_for_the_next_poll_after_a_restart`,
  `a_stamp_whose_answer_was_lost_is_not_taken_twice_on_retry`,
  `two_pollers_stamping_one_peek_at_once_deliver_each_row_once`). No host gives a durable acknowledgment or
  a delivery key, so the duplicate window between host acceptance and the stamp stays.
- **Waking an idle session was measured live once, by hand, per host, and not under load.** Claude Code 2.1.300
  woke an idle interactive session within 15 seconds when an `asyncRewake` `Stop` hook exited 2 (a new waiter
  was running afterwards, none after the session was killed;
  [`sessions.md`](sessions.md#waking-an-idle-claude-code-session)); the OpenCode 2 wake
  ([`sessions.md`](sessions.md#waking-an-idle-opencode-session)) was driven against a stand-in host and run once
  on OpenCode 2.0.28 with `--standalone`. Not measured live: a Claude Code session idle longer than 60 s,
  mail arriving mid-turn, two waiters starting at the same instant (the unit tests drive threads; each new
  waiter overwrites a token in `store_meta` and a superseded one exits at its next poll), OpenCode refusing
  a prompt, and a session woken while the background service rather than a private server runs the plugin.
  OpenCode 1 is not woken, a session that never finished a turn is not polled, an already idle Codex session
  is not woken (Codex's `Stop` is synchronous and has an entry only with the `Loop plugin` row `on`), and Codex
  `turn/steer` is not built. Mail a waiter took is stamped delivered before the host acknowledges the exit.
- **The MCP `send_message` door was driven live on one host only** (Claude Code 2.1.300: `claude -p` refused
  with no row written, the interactive dialog's *Allow once* recorded a row that was then delivered; *Allow for
  this session* and *Deny*, Codex and OpenCode were not run live). A client that declares the capability and
  never renders the question waits out the 15 minutes and the send is refused. The grant is per server
  process and keyed by the sender's run id, which the caller names, so an agent naming another run's id
  borrows its grant only if that id was granted in the same process.
- **`read` and `acked` are what the recipient said, not what a host observed.** Nothing watches a model read
  its context: `inbox --read` and `--ack` are explicit acts, so a recipient that never calls them leaves
  every row at `delivered` however carefully it was handled, and one that acks without acting makes the sender's
  view say so. `inbox` itself is covered by store, unit, MCP and pipe tests against stand-in sessions and was
  driven on no live host. A thread is the id of its first message and is not scoped to the parties: any run
  that knows a message id can join its thread. `expires_in` is recorded and shown and decides nothing, so an
  expired message is still delivered. Types, threads and acks are text and rows in a local file; no gate,
  permission or claim reads them.
- **A question and a wait wake only what can be woken, and were driven on no live host.** A question reaches a
  session at its next hook point or idle wake. Claude Code (`Stop` waiter), Pi and OpenCode 2 (polling)
  record a wake; Codex and OpenCode 1 record none, so a question to them returns `not_woken` after delivering
  the question and does not wait, and a model on Codex can answer only on a turn it is already running. The
  heartbeat is an MCP `notifications/progress` sent only to a call that carried a `progressToken`; a host
  that sends none and kills silent calls will still kill a long wait, and no host was run to measure
  which does. The question and its answer are separate writes: an answer admitted a moment before the asker's
  timeout is recorded can sit in the mailbox as mail for a question already reported `timed_out`, never as
  the answer to it. A session that dies without ending its row is not seen as `ended` and the ask times out.
  The Pi parent-child channel was not moved onto the mailbox: it shares the rules (one outcome, refused
  late answers) and nothing else.
- **Broadcast and the completion contract were driven on no live host.** A broadcast reaches the members the
  resolver lists at the moment of the call; one that registers a second later is not in it, and a member that
  ends between the question and the write still gets a row it will never take. `idle` means a live session
  whose pointer names no issue, which is not the same as a session between turns: Estigia does not observe
  that. The one consent covers the group as named in the question, so a person approving ten sessions has
  approved ten sessions, not a rule. Accepting a handoff is read from a successful `claim` by a run of the
  recipient runtime in the directory the handoff recorded, so a recipient that starts from a worktree, or that
  works the issue without claiming it, never accepts and owes nothing; a store that cannot be written at
  that moment leaves the handoff unaccepted without saying so. The duty is closed by the store, and the mail
  that tells the sender is a second write: a mailbox that refuses it leaves the outcome standing and
  reports `mail_failure` to the recipient, and the sender still sees the outcome through `inbox` with
  `sent`. An abandoned duty is reported when the registry records the session's end; a session that dies
  without ending its row is never reported, and one that resumes after its end finds the duty abandoned and a
  late `done` refused. The deadline is a time the sender reads as `overdue`, not a timer: nothing wakes anyone
  when it passes, and only `estigia handoff --deadline-seconds` sets it. The
  hook that accepts on `claim` has a unit test of the acceptance and none that drives a real claim.
- **A message is text from another session.** Delivery says so in its first lines; nothing stops a model from
  acting on it. **The `flow dev` conflict rule reads the words of the message**: `send_message` and
  `estigia sessions send` refuse text carrying `flow dev <n>` while the recipient's pointer names another
  issue, so a sentence quoting it as something *not* to run is refused too and a differently spelled
  instruction (`/flow`, tabs, a Linear key) is not detected. A handoff is never consumed, expired or
  read-stamped: every later session of that runtime in that repository is told again, and whether anybody
  took it is visible only from the tracker, through `claim`.
- **The handoff's label read-back is against a stand-in `gh`** (the fake models the label edit in a file; no
  handoff was run against GitHub), and the row is written through `store::beside(state_root)` while the label
  goes through `store::store_path()`: the same file in production, two scratch stores under `cargo test`.
  The counts in `estigia sessions` belong to listed sessions (a message whose sender has neither a registry
  row nor a run pointer is counted and shown against nobody).

## The needs-person record

`estigia needs-person` lists rows written at three store writes (`escalate_review`, a `blocked` transition
without a clause, a gate `ask`), measured by one unit test each and a store test for the dedup. What it does
not establish:

- **A notification is best effort and no guarantee.** With `Notifications` at `os` the binary hands the text to
  `osascript` (macOS), `notify-send` (Linux, when on `PATH`) or a PowerShell toast (Windows) and records
  `handed to the OS` on exit 0. That is not "seen": a focus mode, a missing notification daemon or a
  session with no display all exit 0 and show nothing. The argv of all three is measured against a
  stand-in, not a desktop: no real notification was shown while writing it. The default is `off`, and
  the record is written either way. The quiet hours read this machine's clock and, when it cannot say, hold the
  notification back; the hourly cap counts `handed to the OS` rows in the store, so a notification that
  failed frees its slot.
- **Three moments of the six named are not written yet.** `handoff_review`, a verified `done` and the delivery
  halt write nothing: the first has no behavioural fixture to measure it against, and the halt is a pure read
  that must not write.
- **`blocked` is judged at the transition.** A clause recorded afterwards does not retract the row, and an
  unreadable clause read records nothing.
- **A failed write is reported where the answer has room for it.** Recording never fails the decision it
  describes. `escalate_review` and a transition say `needs_person_unrecorded`; a gate `ask` leaves a decision
  with the verdict `unrecorded`, which is itself lost if the same store cannot be written. A host that hides
  the gate's decisions hides this too.
- **An `ask` from a sub-agent is recorded and notified.** The record path does not see `agent_type`, and in
  Claude Code a sub-agent's `ask` does reach the person, so skipping it would hide a real wait.
- **An `ask` repeats after thirty minutes.** The key carries a thirty-minute bucket: a wait that outlives it is
  recorded again, and one that straddles a bucket edge can be recorded twice in a minute.
- **`ask` is only as real as the gate's `ask`.** A `Delivery authorisation` of `ask` may not reach the gate at
  all, and hosts other than Claude Code convert `ask` into a deny before it is recorded.
- **The repository key is read, not proved.** It comes from the shared git directory's `config` without
  spawning a process: a remote in a shape `RepoRef::from_github_remote` does not accept, or a config this
  reader cannot parse, falls back to the main checkout's path (a submodule's own root, inside one); a
  decision from a directory in no repository
  records no key, an ask records that working directory instead, and two clones of one repository with no
  GitHub remote are two keys. The path fallback is canonicalised, so symlinked spellings of one checkout
  agree.
- **No runtime was run live.** The tests drive the functions, not a session of any host.

## The waits audit

[`waits.md`](waits.md) counts the calls this repository's deliveries made because a clock passed: 247
of 17,062 tool calls in 45 Claude Code transcripts, carrying 72.95 M of 5,119.2 M context tokens. What
the count cannot show:

- **One machine, one host, one pace.** The transcripts are Claude Code's, from one person's profile, over
  four weeks. OpenCode, Pi and Codex sessions, and every subagent's own transcript, are not read. A
  slower lane or a different habit of checking CI would give other figures.
- **The classification is read from command text.** A loop is matched by `until`/`while` and `sleep`, and
  a pending CI read by the words `pending`, `in_progress` or `queued` in its output. A read that also
  diagnosed a failure is counted as a wait, so the pending-read row is an upper bound. A wait written
  in another shape is not counted, so the total is not a ceiling.
- **Context is not saving, and not billing.** The figure is the context of the requests that carried
  the calls, nearly all of it cache read. A replaced call still costs the request that reads its
  notification, and that net was not computed.
- **Why a run polled is not in the transcript.** The count shows that a run polled a file a background
  job would have announced. It cannot show whether the run expected no notification, or followed an
  instruction to wait in the foreground.
- **The 111 s is the longest MCP call this host was seen to hold, not its limit.** No other host was
  run, so raising `deliver`'s ceiling to 900 s leaves the shorter-host case unmeasured
  ([*What `deliver` does not see*](honesty-delivery.md#what-deliver-does-not-see)).
