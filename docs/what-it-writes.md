# What it writes, and how it takes it back

Estigia goes into files somebody else owns. This is the account of what it puts there, what it
removes when it leaves, and the two things it deliberately refuses to remove.

It is one document in [the reference set](README.md); [`configuration.md`](configuration.md) says
which setting chooses each root, and [`lifecycle.md`](lifecycle.md) says what an install records
about the binary itself.

**A file that is somebody's link, or somebody's mode.** Setup and uninstall replace a configuration
file by staging a sibling and renaming it over the destination. When the destination is a symlink
(a dotfiles manager's `settings.json`, `CLAUDE.md`, `opencode.json`, `config.toml`), the sibling is
staged beside the file the link names and replaces that file, so the link survives and the bytes land
in the dotfiles copy; a link that names nothing is refused and left as it was. The replacement takes
the permission bits of the file it replaces (on Windows, the read-only flag), so a `0600`
`~/.claude.json` stays `0600`; a file that did not exist is created with the platform default as
before. Windows ACLs and ownership are not carried over. The staging lives in `paths::replace_bytes_atomically`.

**Not only setup writes, and not only uninstall takes back.** A run records the branch it started as
a `branch` marker in the local store, and `release` — the last step of a delivery — deletes every
remote head those markers name, reading each one back, then puts down every isolated checkout those markers name that the run owns and the delivery
contains, with its local branch. `estigia prune` is the sweep for what no delivery owned: merged or closed
heads and checkouts, plus [remote-gone orphan checkouts](#explicit-orphan-pruning), kept when a live claim holds them or they carry commits the remote does not
have, removed only on `--confirm`. The rules, and what each still cannot see, are in the contract's
delivery reference and [`honesty.md`](honesty.md); this document does not restate them.

## Explicit orphan pruning

`estigia prune` retains its existing merged/closed cleanup. An additional eligibility route handles
a branch checkout that would otherwise be kept: its configured upstream must be absent after
`git fetch --prune --all`, its working tree must be clean, and `git rev-list --count <proven-head>
--not --remotes` must be zero. The upstream identity comes from `branch.<name>.remote` and
`branch.<name>.merge`, not the local branch name or a presumed `origin`; the configuration remains
readable when fetching removes a stale tracking ref. Reachability from any refreshed remote-tracking
branch suffices, not only the default branch. A configured upstream still present on its actual remote
is never eligible through this route. The safe fallback for genuinely untracked branches is to keep
them: absent, incomplete, multiple, invalid, local-only (`remote = .`) or unknown tracking identities
are not inferred to mean a deleted remote branch.

The additional route keeps any checkout named by a run pointer, even when its issue is closed. It
also keeps a checkout named by a session registry entry not explicitly ended; a stale heartbeat is
not an ended session. Unreadable pointers, registry reads, remote reads and local Git reads keep the
candidate and name the reason. This deliberately retains some abandoned checkouts rather than
guessing that their holders died. The remaining observation limits are in
[`honesty.md`](honesty.md), under “What the prune sweep cannot see”.

Without `--confirm`, the command only lists candidates and refreshes remote-tracking refs; it removes
no checkout or branch. Confirmation rereads pointers and the session registry for the new route,
repeats its Git checks, then rereads ownership and checks branch identity at the retirement boundary.
The reachability query uses a captured commit id, and that exact id is passed to the existing
retirement operation's HEAD comparison; a newer HEAD is never substituted for the proven revision.
Confirmation then uses the existing
checkout retirement operation: `git worktree remove` without force, followed by local branch
retirement. It introduces no remote-branch deletion and no session-start deletion. `release` and
the ordinary `estigia changes` listing are unchanged. These observations are not a repository or
ownership lock; the remaining race after the final checks is described in `honesty.md`.

**What Estigia writes to GitHub.** Issues and their prose, a person's `comment` prose, labels,
open or closed, pull requests and branches — and no `issue-flow` marker anywhere. The markers of the
claim family (`claim`, `reclaim`, `unassign`, `heartbeat`, `progress`, `blocked`, `branch`) and of
the review family (`published`, `review-handoff`, finding, lens report, refutation, disposition,
validator result, spend, computed verdict, receipt) are recorded in `~/.estigia/state.db` alone, and
no sentence is posted beside them. What reaches GitHub from a claim is the projection, written after
the local commit and never read back to decide: the assignee, the `dev:<runtime>` label (created on
the first claim for a runtime), and for `start-branch` the develop link and the branch. The
`comment` tool posts the escaped prose and nothing else, and `create` files the caller's body
unchanged. [`honesty.md`](honesty.md) names what recording locally costs.

Five invariants, and the tests are named after them:

1. **`is_configured` is a query**, separate from `setup`. "Where did I install this?" is the
   question people actually arrive with.
2. **Install says when it is writing over somebody else's file.** Estigia ships upstream's skill
   under upstream's name, so an operator who already runs `issue-flow` has files at those paths.
   Install writes its own copy over them and reports each as `REPLACE` rather than `update`, and
   `--dry-run` says so before anything happens. Nothing keeps the old contents: uninstall leaves
   the file, because it is not Estigia's to remove, so what stands afterwards is Estigia's copy.
3. **Uninstall is the exact inverse.** It removes what Estigia **created** and nothing else — not
   another tool's block, not your own notes, not a file you dropped beside the references, and not a
   file that was already there when Estigia wrote over it. That last one is the common case, not the
   corner: Estigia installs upstream's skill under upstream's name, so a directory that already holds
   `issue-flow` is the *same* directory. Install therefore records what it created, in
   `<skill>/.estigia/installed.json`, because nothing on disk can tell the two apart afterwards —
   an overwritten file and a created one both hold Estigia's text. Delete that record and uninstall
   removes nothing, which is the honest reading of no evidence.

   Claude Code's `review-blind.md` and `review-blind-sealed.md`
   live outside the skill tree and use that same path-only ownership record. They are the only
   reviewer definitions setup installs: Judgment Day's two judges. The other seven reserved names —
   `review-lens.md`, `review-risk.md`, `review-readability.md`, `review-reliability.md`,
   `review-readback.md`, `review-refuter.md` and `review-validator.md` — are launched by
   `launch_review_input` from templates embedded in the binary, so setup writes none of them and
   retracts a copy an earlier build wrote (below). Setup recursively parses the
   user agent tree for the two judges
   before writing any setup artifact: a
   second file carrying the reserved YAML identity, an unprovable candidate, an unowned canonical file,
   or an owned canonical file whose bytes differ from **every current and previous rendering Estigia can produce** of the
   embedded definition is preserved and refused. Every rendering, not the embedded bytes themselves:
    `Evidence standard` decides the unsealed file's tool grant; each sealed file's grant is fixed.
    The two judge definitions carry the paragraph asking a verdict for its basis — so the embedded copy is a
   template and the raw template is exactly what a valid install never holds. Those two definitions
   are Judgment Day's judges, and no tier acquires them.
   Setup repeats that decision immediately before the reviewer step. A fresh path is recorded as owned
   before its bytes are written, then a complete staged body is linked into place without replacement;
   a file another actor created after either read is preserved, and ownership added by the losing create
   is removed. An interrupted create remains exactly retryable. Comparison follows established setup
    text normalization: CRLF versus LF and a missing final newline do not count as an edit; every other
    textual difference does. An owned canonical file whose bytes match a previous rendering Estigia
    shipped is ours-older: setup rewrites it to the current rendering rather than refusing. A body matching
    neither also survives if it is exactly what **this build itself** last wrote
    at that exact path: every real write of a reviewer definition records a digest of the bytes it left
    behind, under the same outside-path record `created_outside` already keeps, and a later run
    compares the file it reads back against that digest before falling through to the rendering ledger.
    This is what an in-flight `--allow-source-build` install needs and the ledger alone cannot give it —
    a template that moves again before it is ever published joins no ledger, because nothing publishes
    an unpublished round — and it is one function, `reviewer_is_ours`, read by both the setup preflight
    and uninstall's own delete-or-keep choice below, so a body one door accepts cannot be a body the
    other keeps as foreign. Bytes matching neither the recorded digest nor any current or previous
    rendering remain an operator edit and are refused. An absent digest — a record written before this
    existed, or a path this build never wrote to — is not read as a match either way; it falls straight
    through to the rendering ledger exactly as it did before. `docs/honesty.md` has the fuller account,
    including why the ledger by itself could never close this and what the digest still does not reach.
    Fresh
    and upgrade plans report the resulting `installed.json` ownership mutation exactly once as a skill
   create or update, matching the real action manifest.

   The seven binary-launched names are taken back by the reviewer's own content-aware road rather
   than by `RETIRED_AGENTS`, which retracts by ownership record alone and would delete a copy the
   operator edited. `setup` and `uninstall` both ask `reviewer_is_ours` of an owned copy — a current
   or previous rendering, or the recorded digest: a match is deleted and forgotten, a mismatch is
   kept, reported as `Kept` and its path relinquished, and a path with no ownership record is never
   touched and never refused. The preflight does not read these seven, so an edited or unowned copy
   does not block a setup. The names stay reserved for the launch gate: a project definition
   carrying one is still refused as `reviewer-project-shadow`, and a hand `Task` launch of one is
   refused as `reviewer-launched-by-the-binary`, whatever the user tree holds.

   Two more definitions live outside the skill tree under that same record, and their rule is a
   third one rather than either of the two above. `developer.md` and `analyst.md` are written only
   where `Model routing` names their role, and the same entry says what the role
   runs on. `by task` names no role, so it writes neither and retracts an owned one on the next
   run. A definition already at one of those paths that
   Estigia did not create is **refused**, in the preflight and again immediately before the write,
   rather than overwritten the way a re-rendered reviewer is: `analyst` is a name another harness's
   orchestrator answers to, so replacing it would take somebody's own file with nothing recorded and
   nothing to give back. Taking a role out of the routing retracts the file on the next run,
   which is what makes naming it consent rather than a one-way door. They are not alone in that: a
   plain `setup` also retracts an owned `sdd-*.md` — a definition the retired protocol wrote —
   through a retired-names list rather than the install list, so it comes out even though the
   protocol that wrote it is gone, and drops a retired skill file. A definition that leaves the
   install lists entirely — `implementer.md`, renamed to `developer.md` — is named in
   `RETIRED_AGENTS` and retracted by `setup` and `uninstall` on the same ownership gate, so a name
   no walk of the install lists reaches still comes off a machine that has it. What is particular to
   these two is that a **configuration entry** and not a retirement list decides it.

   Uninstall deletes an owned reviewer whose bytes satisfy the same `reviewer_is_ours` the setup
   preflight does — a current or previous rendering, or a match against the digest recorded for that
   exact path — before forgetting it or removing the skill ledger. Sharing the one function is the
   point: a digest arm added at only one door would let them disagree, accepting a body as ours-older
   at setup and then keeping the same bytes as somebody's edit at uninstall, orphaning it at the
   reserved path for the next install to meet as unowned. A
   failed deletion therefore retains the evidence needed by an exact retry. Bytes that match neither
   are kept and their path is relinquished. This rule is deliberately specific to the reviewer; it does not broaden
   or repair the retraction rule for a definition a retired protocol wrote, and the delegated roles'
   refuse-rather-than-replace is a third rule beside both. Configuration writes never touch the file.

   **By name, and by content on the way in — not yet on the way out.** The record used to store the
   paths an install created and nothing about what it wrote there, so *did you change this, or did an
   older build write it* had no answer: a file Estigia created and you later edited was rewritten by
   `sync` on a plain `update` line, the same word a version bump gets. It now stores a **digest of
   what it last wrote** at each path, which answers exactly that question and nothing else. An
   install that finds one of its own files changed since says `OVERWRITE`, and a machine merely one
   release behind still reads as `update`, because its record predates digests and holds none.

   **`OVERWRITE` and `REPLACE` are two words because they are two things**, and they were one for
   several rounds. `REPLACE` is a file that was never Estigia's; `OVERWRITE` is Estigia's file with
   your edit in it. Sharing the word meant sharing the sentence under it, and `sync` announced *"1
   file(s) were already here and are not Estigia's"* about a file `estigia setup` had installed
   thirty seconds earlier — which sends somebody whose own edit has just been discarded looking for
   a file that was never there. What you do next differs: one is a stranger's file you may not have
   known about, the other is work you have lost.

   **And on the way out.** `uninstall` used to take a file you had edited with the rest, named on its
   own `remove` line like every other and in no way marked as one you had touched — measured: a
   `bindings/github.md` rewritten whole and an `rdd.md` with a note appended were both taken, each
   printed among the eighteen. The same digest answers it: a file whose contents have moved since
   Estigia wrote them is **kept**, and named on its own line so nothing is left silently. An
   untouched one still goes, and so does every file on a machine whose record predates digests —
   otherwise the fix would leave whole installations behind, which is the failure it exists to
   prevent, inverted.

   Two things Estigia created it deliberately leaves, and they are named here because "removes what
   Estigia created" would otherwise read as *all* of it. A checkout's own rows in
   `.git/estigia/` stay: uninstall takes an **agent** out and was never given a repository, so
   reaching into whichever checkout somebody happened to be standing in is the failure this design
   refuses — `config forget` is the one command that removes them, and it says which file it
   removed. Both are measured by `uninstall_leaves_the_checkouts_answers_and_the_list_of_checkouts`.

   `~/.estigia/` splits the difference, and the line between the two halves is *live run state*
   against *what the person keeps*. The run pointers, `state.db` — which holds the decision history,
   the durable role log, the per-issue workflow history, the session registry and the mailbox — any stand-down record and the stop-after
   requests hold what and what was last asked: they go with the **last** agent, so `uninstall --all`
   with nothing left installed takes them — by name, and an empty directory only, so a file of the
   operator's own keeps the directory. Taking them at that point is the operator's recorded
   requirement and it is deliberate; taking them while any agent remains would silently strip a live
   run of the file its gate reads, which is why a plain reinstall never touches them and `setup` does
   not either. The machine-wide binary lifecycle evidence under `~/.estigia/lifecycle` belongs to the
   person rather than to any adapter and is not named by the removal, so it stays with whatever else
   is there. Both halves are measured by
   `taking_estigia_out_takes_its_own_state_with_it_and_not_before` and
   `a_live_run_pointer_survives_a_plain_reinstall`.

   `~/.estigia/state.db`, with its `-wal` and `-shm` files, is written at run time and never by
   `setup`. The workflow state that has no GitHub object to live in is only here: the dedup
   identity and the `blocked-until` clauses `create` and `blocked_until` record, and the task
   checklist `task` records, none of which is written to the tracker. The transport creates it: the first dispatched operation that spawns `gh` records its
   call counts there, the first operation of the claim family that touches an issue imports that
   issue's history there, and the gate appends every decision and role observation there. `estigia
   doctor`'s `store` row never creates the file, but `doctor` as a whole can: its `stale-run-pointer`
   row replays `verify-claim` for every run pointer it finds, and that is an operation that imports.
   `estigia store import` is the operator's own door into it: it admits every open issue the store
   does not hold, reading each issue with `gh issue view` and writing only the store.
   The write path also snapshots the store with SQLite's `VACUUM INTO` into
   `~/.estigia/snapshots/`, when the newest snapshot is a day old or older, keeping the newest
   three; `doctor`'s `store` row reports the newest snapshot's age. A snapshot is a copy for a
   person to restore by hand, not one Estigia reads back.
   Removal names it, along with its `-wal` and `-shm`, and the `snapshots/` directory beside it,
   and takes them with the last agent, wherever that agent was installed: a global install writes one
   file under `~/.estigia/roots/` naming the Claude, XDG and `APPDATA` roots it resolved, so an
   `uninstall` under one `CLAUDE_CONFIG_DIR` still sees an install under another and leaves the
   store alone. A record that cannot be read keeps the store, and the uninstall report names it. The
   per-issue workflow history is rebuilt from GitHub on the next touch; the decision history is not,
   and losing it is the point of the removal — it is the record of what this machine's gate decided,
   and the operator asked that the last uninstall take everything Estigia wrote. What it holds and
   what decides from it is in [`honesty.md`](honesty.md).

   `~/.estigia/jobs/<id>/` is a fifth, written at run time and never by `setup`. A review job's
   directory holds the manifest, the prompt, the runner's lock and logs, its `result.json`, and a
   `usage.json` carrying the reviewer CLI's own counters when it reported any. A check job, started
   by `run_checks` through `__check-job`, leaves its directory in the same place with its own
   manifest, lock, logs and result ([`generated-files.md`](generated-files.md)). Nothing removes either yet: the runner is only added, and
   [`honesty.md`](honesty.md) says what is not wired.

   A third leave is temporary: five adapters share `~/.agents/skills`, so taking one of them out
   leaves the skill for the agents that remain and it goes with the last of them, a retired agent that is still installed counting as one that
   remains. The note that
   accounts for the surviving directory names **only the files no line of the run already named** —
   not everything still on disk. Naming everything was measured, and it said sixteen files were the
   operator's where one was, listing `SKILL.md`, the transport and Estigia's own record among them:
   the sentence that exists to answer *it did not touch my things* both buried its real answer and
   invited a person to delete a skill two configured agents were reading.
   `the_skill_left_for_another_agent_is_not_reported_as_the_operator_s` holds both halves, the second
   being that a file reported `unknown` — no record, so nothing shown to be Estigia's — was reported
   as the operator's in the same output.
4. **Everything Estigia writes into a shared file lives between markers**, and a file that held
   nothing but our block does not survive as an empty husk. There are no backups because there is
   nothing to restore. A file that carries the block more than once, from a merge or a restored
   copy, comes out of `setup` with exactly one, holding the new body, and out of `uninstall` with
   none; text outside the markers is not touched. Markdown comes back byte for byte, and so does JSON: the indentation a file
   was written with is read off it and used again, so a settings file Estigia went into to add one
   hook is not handed back reindented.
5. **`--dry-run` reports what the real run does**, checked by a test that runs both and compares.
   It runs every read-only probe the real run does, the lifecycle read included, so a file or state
   the real run would refuse is refused by the dry run too.

## The optional loop plugin

OpenCode reads plugins, so Estigia can offer continuation there without a hook file, and it offers
it only when the `Loop plugin` row is `on`. `estigia setup opencode` then writes a second
single-file plugin, `flow-loop.js`, beside the gate plugin `estigia-workflow-authority.js` under
the same `plugins/` directory, marked `estigia:loop`. The row's default is `off`, so a plain
`estigia setup opencode`, `estigia setup --all`, and `scripts/build-install.sh`'s non-interactive
install write nothing new — a plugin nobody asked for is a behaviour change to every OpenCode
install this crate touches. The setting is authoritative in both directions: `on` writes the plugin
and `off` takes back one Estigia wrote, so running `setup` with the row `off` after it was `on`
leaves no loop plugin behind. Only OpenCode reads a plugin Estigia writes, so the plugin is written
only for OpenCode, and the row is offered for OpenCode and for Codex, which continues through hook
entries instead ([below](#the-same-loop-on-codex)); the write is gated on the effective setting, and
the path being named is not what decides it.

The loop plugin continues a session while the run still holds its issue, or — once the run's own
issue is done and released — while another issue is claimable by it, or while a `blocked` item's
recorded exit condition already holds and only the discharge stands between it and a claim. It
decides that by asking a read-only verb, `estigia goal`, which answers whether this session's run
holds a live claim, on which issue, and, when it holds none, how many issues it could claim and how
many it could discharge, and what is held or pending. **A live claim is continued with the held
text alone** (issue #3): the same sentence the Claude and Codex `Stop` hooks hand back, naming the issue
and the run, with no preamble and no `/flow dev` — the session already holds that context, and
repeating the objective every turn only grew it. The preamble and the objective go out once, at
`/flow-loop`, and again whenever the run holds no live claim and the loop continues on claimable or
dischargeable work. A live answer that names no issue stops the loop. Completion is not a value the
agent supplies: the plugin reads no `<promise>` and takes no done input. **The objective defaults
to `/flow dev`**: `/flow-loop` with no text, or with only flags, stores and enqueues
`/flow dev`; text whose first word is `/flow` or `flow` is used as given; any other text is
prefixed with `/flow`, so `/flow-loop only Alicante and Murcia` enqueues `/flow only Alicante and
Murcia`, a bare `flow`: the combined analyst-developer, which takes the held issue, then an
unassigned `review`, then an unassigned `ready`, then an `analysis` item to specify, and triages the
text only when every queue is empty. A leading role word (`dev`,
`triage`, `analyst`, `developer`) picks that role instead of routing, and working one issue needs
`dev <n>`, because a bare number is read as conditions or domain rules. The first word is
matched whole, so `flowchart x` is prefixed, and `stop` is still the command that
ends the loop. The stored goal holds the prefixed text, which is what each re-enqueue sends and what
`/flow-status` prints. Its exits are that verb's
answer and an optional `--max <n>`, which counts **issues that reached `done` under this run** rather
than turns: the plugin asks `estigia goal --completed`, and that count is enumerated from the tracker
— the closed, `status:done` issues the run's own delivery closed, proved one by one the way
`verify-claim` proves a close — never inferred by the plugin and never kept locally. An answer that
could not be read, a queue count Estigia could not report, or a completion count Estigia could not
report, stops the loop rather than
continuing it, and so does a claim answer Estigia could not confirm — an unknown result is not
clearance, and claimable work does not make it one. **A stop keeps the goal.** The plugin records the
reason and the moment on the goal it holds, and a stopped goal is terminal: the next turn of the
session does nothing rather than starting the loop again, so a wait on a person is a stop to
re-invoke deliberately and not a poll — a wait on another run or on a condition is the exception
below. `/flow-status` reports the stored objective, the run id, and the
reason the loop stopped, beside the live answer.

**A reaction leaves a record, and a failure is a stop.** Every turn end the owning instance reacts to
writes `last: { at, outcome }` into the stored goal, with `outcome` one of `enqueued`,
`stopped: <why>` or `failed: <why>`. An exception thrown while reacting is recorded as a stop naming
the error, exactly as an unreadable answer is, so the goal never keeps claiming to run after the
plugin failed. Only the instance whose location is the goal's `directory` writes `last`: any other instance
writes nothing, so it cannot overwrite what the owner recorded and one turn end still yields one
record. What an instance observes that belongs to no session goal — a
turn end with no session id, and an event stream that ended or threw — is written to
its own key, `estigia:loop-watch:<location directory>`, as `{ at, outcome }` with `outcome` beginning
`ignored:`, `failed:` or `interrupted:`. The watcher subscribes again one second after the stream
ends or throws, and stops only when the plugin is torn down. `/flow-status` prints the goal's `last` (or says
that no turn end has been observed yet) and that per-location observation; it no longer says the loop
has not stopped, only that no stop is recorded.

**One reaction per event, in the repository the goal was made in.** OpenCode loads a plugin once
per location and delivers every event to every instance, so an instance acts only on a session
whose goal was asked in its own location, and never reads the event's `location` at all: OpenCode
2.0.20 sends `session.execution.succeeded` without one, and a task worked in its own worktree
raises events located there while the goal was asked in the base checkout. The goal records the
`directory` `/flow-loop` asked in, and every later `estigia goal` runs there, never in the event's
location. `estigia goal` itself answers a run's pointer only from a checkout of the repository the
claim was made in — from any other it says `unreadable`, which the plugin treats as a claim it could
not confirm. An answer that could not be read, or whose standing is `unread` or `unreadable`, is
retried on the loop's own wait timer, up to a bounded number of attempts; a later confirmed read
continues the loop, and only once those attempts are spent does it stop — with a resumable notice
whose reason says the read was retried. The goal is read again right before a prompt is
enqueued, and an operator stop that arrives while a decision is in flight is held until that decision finishes, so no prompt is enqueued and no timer armed after it. Continuation
prompts go through `session.prompt` with `delivery: "queue"`; an ordinary stop and `/flow-status`
are admitted
through `session.synthetic` with `resume: false`, so they are visible in the session and schedule no
model turn that could re-arm the loop, while the stop an exhausted retry reaches is sent with
`resume: true`, so the agent gets a turn in which to see it rather than a notice that schedules
none. OpenCode 2.0.20's notice renderer shows a synthetic
message's `description` and never its `text`, so every message is sent with its full content as
`description` (prefixed `Estigia loop: ` unless it already begins `Estigia`) and again as `text`;
that renderer has no truncation and collapses whitespace only for shell and subagent messages, so a
multi-line report keeps its newlines. `/flow-loop stop` records an operator stop — at once when nothing is deciding, and otherwise when the decision in flight finishes, with an immediate "stopping as soon as the decision in flight finishes" reply; a request is satisfied by that one stop and never outlives it, so it cannot stop a later `/flow-loop` — and `/flow-loop` on a
session whose goal is still running enqueues nothing and says so. The same verb is the run's own goal
surface: `estigia goal progress
<note>` and `estigia goal blocked <reason>` record activity that renews its claim, and `estigia goal
complete` adjudicates, read-only, whether delivery evidence proves the goal complete — see
[`honesty.md`](honesty.md) for what that does not establish.

**The agent can ask the loop to stop after its current task.** `estigia goal stop-after
[--reason <text>]` records the request on a file of its own beside `runs/` under `~/.estigia/`,
rather than on the run pointer — `release` forgets the pointer and the loop reads the request only
after the claim is released, so a flag on the pointer would be gone before the loop could act on
it. The next `estigia goal` carries it as `stop_after`, and when the run is not live the plugin
stops with the request as the reason instead of enqueueing or waiting; while the run holds a live
claim the request changes nothing and the loop continues. The stop consumes it, and `estigia goal
stop-after --clear` takes one back, so a later `/flow-loop` starts without it; a fresh `/flow-loop`
also clears a request an earlier loop left behind, so it never stops on a stale one. `/flow-status`
prints a pending request beside the tracker answer.

**One clause is what makes a blocked item dischargeable**, and the plugin never reads prose for it.
A blocked issue records one clause per `blocked_until` call (or per `<issue>=<state>` pair of
`create`'s `blocked_until`), in the local store and nowhere on GitHub; a body that still carries a
`<!-- issue-flow: blocked-until issue=<n> state=<s> -->` marker has it imported on first touch, so
an issue filed before this change stays resumable. The clause holds when the local store puts issue `<n>` in state `<s>`, and an item counts as
dischargeable only when every clause it recorded holds now. A clause that is not well formed, or
whose named issue the store has never admitted, is reported as one that does not hold — never
dropped, because an unevaluated condition is not a satisfied one — and the item carries `unparsed`
or `unread` so the typo or the unreadable issue stays visible instead of looking like a wait with no
clause. An item with no clause at all — a human adjudication, a condition nobody wrote down — counts
nothing and keeps its own surfacing. `list-state --state blocked` reads the clauses from the store
and answers `dischargeable` beside a per-item `conditions` list, which is one store read per blocked
item and no GitHub read at all; `estigia goal` reports the count, and
[`honesty.md`](honesty.md) says what it does not establish.

**Work that is held or pending is waited on, not stopped on.** When the run holds nothing and
`claimable + dischargeable` is zero, `estigia goal --json` still says whether the queue can open by
itself: `held` counts open issues in `in-progress` or `review` that this run could not claim,
`pending` counts `blocked` items that recorded at least one well-formed clause that does not all hold
yet, and `unconditioned` counts `blocked` items that recorded no clause, or one that is not well
formed — a person's to settle. `unadmitted` counts open issues the local store has never admitted,
which `list-state` reports under `unadmitted` and no state; nothing places them but an explicit
`transition`, and no `status:` label is read. Each is a number, or `null` when the read behind it failed, and an
unread count is never zero. With `held + pending + unadmitted > 0` the plugin does not stop, and its wait names the unadmitted
count and says to admit each issue with an explicit `transition`: it records
`waiting: <counts>` as the goal's `last.outcome` and `waiting: { since, next, on }` on the goal, and
re-asks `estigia goal` when a timer fires, every `--wait=<minutes>` minutes (default 5, fixed, no
backoff; `/flow-loop <objective> --wait=10`). The interval is whole minutes from 1 to 1440, and a value
outside that — `--wait=0`, `--wait=30s`, `--wait=0.5` — is refused with the reason and the loop does
not start, rather than being clamped. Each tick makes the same decision a turn end makes: it enqueues
the continuation prompt once claimable or dischargeable work appears, keeps waiting while something is
held or pending, and stops as before when nothing can open, naming the `unconditioned` count when
there is one. An unconfirmed read waits the same way, its attempt count carried in the goal's
`retries` beside `waiting`, so the bound survives the reboot the timer's own re-arm answers.
**A wait costs one tracker read per interval and no model turn:** while waiting the
plugin calls neither `session.prompt` nor `session.synthetic`, because anything posted into the
session enters the next turn's context; the wait is written only to the goal in plugin storage, where
`/flow-status` prints what it waits on, since when, the interval and the time of the next re-ask.

There is at most one timer per session. Only the instance whose location is the goal's `directory`
arms or runs it, so the per-location instances of one server never hold two; a turn end located anywhere
else is answered by that same instance and by no other. The timer is cleared on
any stop, on `/flow-loop stop`, when work is enqueued, and when the plugin is torn down. OpenCode evicts idle
locations and reboots them, and hot-reloads a plugin whose file changed, and a waiting session is
idle by definition; teardown clears the timer, so on setup the plugin scans its storage for goals
under `estigia:loop:` that are running, carry `waiting`, and whose `directory` is its own location,
and arms each for the time left until `waiting.next` (at once if that is past or unreadable). A scan
that fails is recorded as `failed: could not re-arm waiting loops (...)` under the location's watch
key. A turn end and a
tick share one in-flight decision per session: whichever arrives second while the first is reading
does nothing, so the two enqueue once. `held` is read from `list-state`, which now also answers
`open` for `review` and `in-progress` (every non-pull-request issue in the state, assigned or not) and
`pending` and `unconditioned` beside `dischargeable` for `blocked`; one `estigia goal` therefore
reads four queues, and the blocked queue's per-item body and label reads are shared by all three
blocked counts. What the wait cannot see is in [`honesty.md`](honesty.md).

**A running child session is waited on, not re-prompted over.** A turn can end only because the
session delegated work to a child session (a `Task` subagent) and has nothing to do until it reports;
continuing at once would spend a model call every turn end to learn that the child still runs. The
plugin therefore also listens to `session.created` and to `session.execution.started`, `succeeded`,
`failed` and `interrupted`, and keeps in memory which sessions it has seen start with a parent. A
session counts as a child when it has no loop goal of its own and a parent is known, from the
`parentID` of a `session.created` it saw or else from `ctx.session.get`; a lookup that throws leaves it
unknown, never an error out of the hook. When a turn ends and the decision would otherwise enqueue the
continuation, a running child of that session turns it into a wait: `waiting: N child session(s) still
running: <ids>` as `last.outcome` and `waiting.on`, on the same timer and with the same
no-model-turn cost as above. When a child ends, however it ends, and no other child of that parent is
still running, the plugin decides for the parent once, provided the parent's goal is running, belongs
to this instance's location and is waiting — a parent still inside its turn is not continued twice.
The wait timer is the backstop: a tick continues the parent even if a child is still recorded, so a
child whose end was never observed costs one continuation per interval and cannot strand the loop. The
ownership rule is unchanged and an event's `location` is still never read.

A plain `estigia setup opencode --uninstall` removes every plugin Estigia wrote, as it removes everything Estigia
wrote — a plugin left behind that shells out to an uninstalled binary would be a dangling file, not
an uninstall. Uninstall reads the loop plugin's path rather than the setting: a file an earlier
install wrote is taken back even when the row is `off` again. A file carrying no `estigia:loop`
marker is neither replaced nor removed, exactly as the gate plugin's own marker already decides, so
`off` leaves a foreign `flow-loop.js` where it is rather than overwriting it. What the loop does
not prove is in [`honesty.md`](honesty.md).

## The same loop on Codex

Codex reads no plugin, but it runs a `Stop` hook that can keep a turn going, so the `Loop plugin` row
(default `off`, per agent, edited in the TUI or with `estigia config set "Loop plugin" on --agent
codex`) is offered for Codex too. With it `on`, `estigia setup codex` writes two entries into
`hooks.json` beside the gate, each with its own trust table in `config.toml`
([below](#what-setup-writes-into-codexs-configuration)):

| Event | Command | Timeout |
|---|---|---|
| `SessionStart` | `"<estigia>" hook session-start --agent codex --dialect claude-code` | 5 s |
| `Stop` | `"<estigia>" hook stop --agent codex --dialect claude-code` | 180 s |

`SessionStart` tells the model `Run id codex-…` through `additionalContext`, which Codex hands the
model as a developer message; that is how a Codex run learns the id the loop and `estigia goal
--runtime codex` use. The row is authoritative both ways: `off` takes back both entries and their
trust, foreign entries and foreign trust beside them stay, and `owned` in `src/setup/codex_trust.rs`
recognises exactly these commands — the event, `--agent codex` and `--dialect claude-code` at the end
of the command — and nothing that merely names a hook. Uninstall reads the file, not the setting.
Every other agent gets neither entry, and with the row `off` Claude Code's `Stop` and
`SessionStart` are unchanged; with it `on` Claude Code gets a loop of its own, below.

**What `Stop` decides for `--agent codex`.** The run id is `codex-…`, derived from the payload's
`session_id`. `stop_hook_active` is not consulted: Codex sets it on every `Stop` after the first in a
turn and never resets it, so honouring it would end every loop after one continuation. The tracker read
is the only terminator, and Codex imposes no cap of its own.

| The run | The hook answers |
|---|---|
| holds a live claim | `{"decision":"block","reason":…}` with the reason Claude Code's `Stop` gives |
| holds none and a stop-after request is pending | allows the stop, with `systemMessage` naming the request, and consumes it |
| holds none, and `claimable + dischargeable > 0` | `block`, reason = the shared preamble naming the `codex-…` run id, a blank line, and `$flow dev` |
| holds none, and only `held` or `pending` work is left | allows the stop, with `systemMessage` naming what the queue waits on |
| holds none, and only `unadmitted` issues are left | allows the stop without ending the loop (`terminal: false`), with `systemMessage` naming the count and saying to admit each with an explicit `transition` |
| holds none and nothing can open | allows the stop, with `systemMessage` naming the `unconditioned` count when there is one |
| has mail waiting, whichever of the rows above applies | the same answer with `decision: block` and a `reason` that is the loop's reason (when it has one), a blank line, then the mail; `systemMessage` is kept. The mail is taken by the delivery in [`sessions.md`](sessions.md#the-mailbox), so the row is stamped once. Codex has no `Stop` entry without the `Loop plugin` row `on`, so this is where Codex mail is delivered at `Stop`. |
| has a claim answer that stays `unread` or `unreadable` past the bounded retries, or a queue count that could not be read | allows the stop, with `systemMessage` naming what could not be confirmed and, for a retried claim, saying the read was retried |

The reading is `goal_reading`, the function behind `estigia goal`, not a copy of it, and an
unconfirmed answer is asked again, with a pause between attempts, up to a bounded number of attempts
before the stop is allowed — the OpenCode plugin retries the same way on its wait timer.
`systemMessage` is the field Codex shows as a warning on a hook that allows (`codex-rs/hooks/src/events/stop.rs`
turns it into a warning entry); a block's `reason` reaches the model as the next user message inside
the same turn. A Codex that gets no answer within 180 s, or an exit other than 0 and 2, ends the turn:
a failed read fails closed. With the row `off`, Claude Code's answer is byte for byte what it was,
including standing aside on `stop_hook_active`, and it never claims the next issue; with it `on` the
loop below takes that entry over.

**One objective rule and one preamble.** `src/harness/loop_text.rs` holds the rule (`objective`: no
text is `dev`, text starting with the word `flow` is used as given, anything else follows `flow`) and
the preamble text, and the held sentence (`held`) that `harness::hook::stop_block` fills and both
plugins fill from `preamble_parts`. The OpenCode plugin is rendered from those strings at `setup`, and Codex's hook
builds its prompt from the same functions at run time; a table of cases is asserted by the Rust
tests and by the node-driven plugin test. What differs is the invocation: Codex has no custom slash
commands and invokes a skill by mention, so the same rule renders `$flow dev` there and `/flow dev`
on OpenCode. Measured on codex-cli 0.159.2 with the installed skill: a first prompt of `$flow dev`
put the skill's `SKILL.md` into the model's input, and `/flow dev` and `flow dev` did not. Codex
resolves mentions from the operator's input at the start of a turn, so a `$flow dev` inside a
continuation is text the model sees beside the skill list, not an injection of the skill body;
`docs/honesty.md` says what that leaves unmeasured. On Codex the operator's own text is simply the
first prompt they type.

| | OpenCode plugin | Codex hooks |
|---|---|---|
| Written | `flow-loop.js`, a file Estigia owns | a `Stop` and a `SessionStart` entry in `hooks.json`, plus their trust |
| Continues by | queueing a prompt after the turn ends | blocking the stop, inside the same turn |
| Objective | `/flow-loop <text>`, rule applied to the text | `$flow dev`; the operator's text is their first prompt |
| `--wait` | re-asks the tracker on a timer while work is held or pending | not ported: a `Stop` hook cannot schedule a re-ask, so it allows the stop and says what the queue waits on |
| `--max` | stops at `n` issues delivered under the run | not ported |
| Stop the loop | `/flow-loop stop`, or `estigia goal stop-after` | `estigia goal stop-after`, honoured at the next turn end; or interrupt the turn, or turn the row `off` and run `setup` |
| Status | `/flow-status` | `estigia goal --session-id <id> --runtime codex`, with the id from the `SessionStart` line |
| Grows | one turn per continuation | one turn for the whole loop |

## The same loop on Claude Code

Claude Code reads custom slash commands and runs a `Stop` hook that can keep a turn going, so the
`Loop plugin` row (default `off`) is offered for it too. With it `on`, `estigia setup claude-code`
writes two slash commands under `$CLAUDE_CONFIG_DIR/commands/` — `flow-loop.md` and
`flow-status.md`, each carrying `<!-- estigia:loop-command -->` — and registers the `Stop` entry as
`"<estigia>" hook stop --agent claude-code --dialect claude-code` with a 180 s timeout, while
`SessionStart` and `SessionEnd` stay exactly as they were. With it `off` nothing is written and an
Estigia-owned command is taken back; a command Estigia did not write is neither replaced nor removed,
and a plain uninstall removes them.

On every install, whatever that row says, `estigia setup claude-code` also writes one more slash
command, `estigia-changes.md` in the same directory, carrying the same marker. It is a prompt that
asks Claude Code to run `estigia changes` and show the output unchanged, and not to act on it. It
follows the same ownership rule: a file Estigia did not write is neither replaced nor removed, and a
plain uninstall takes back one it did.

- `/flow-loop [objective] [--max n]` stores the loop for the session's run
  (`estigia goal start`), applying the same objective rule OpenCode's plugin does — no text is
  `/flow dev`, text already naming `/flow` is used as given, anything else is prefixed — and records
  the cap. Only a numeric `--max` is split out; one whose value will not parse is left in the
  objective rather than eating the word that followed it. Then it works the objective.
- `/flow-status` runs `estigia goal` and reports the objective, the run id, the last outcome and, when
  the loop has ended, why (`estigia goal` answers all of these beside the tracker line).
- `/flow-loop stop` records the portable `estigia goal stop-after` request, the same one the agent
  itself can make, so ending the loop needs no Claude-only mechanism. It is honoured at the next turn
  end.

**What the `Stop` hook decides.** While a stored loop is running, the hook continues the turn with the
next `/flow` prompt — the preamble from `harness::loop_text` and the `Slash` invocation, the same
shared decision Codex's hook uses — or, while the run holds a live claim, with the held text alone
(`harness::hook::stop_block`) — whenever `estigia goal` says the run is live or another issue is
claimable or dischargeable; it honours a pending `stop-after`; it stops at the stored `--max`, counted
by `estigia goal --completed` as issues that reached `done` under the run; it lets the turn end and
says what the queue waits on when only held or pending work is left, since a `Stop` hook cannot wait;
and an answer it cannot confirm is retried, with a pause between attempts, up to the same bound
before the loop stops — that stop is not terminal, so a later turn reads again, and its record says
the read was retried. It
records the last outcome and the stop on the stored goal, and the request behind a `stop-after` stop is
consumed only once that record has reached disk, so a goal that could not be written leaves the
request for the next turn instead of losing the stop; a fresh `/flow-loop` clears any request a
previous loop left. A stopped goal is terminal for the loop, not for the gate: the next turn gets the
ordinary claim-holding answer rather than starting the loop again. It stands aside on an unreadable
payload and on a transcript that names an unanswered background launch, exactly as the ordinary Claude
`Stop` does. It does **not** honour `stop_hook_active` while a loop is running: Claude Code sets it on
every `Stop` after the first in a turn, so honouring it would end a loop after one continuation, as it
would for Codex. With no loop stored, the hook is the ordinary claim-holding one, byte for byte.

That ordinary `Stop` also stands aside for a run waiting on a person: one whose pointer records the
state `blocked`, or whose newest receipt on the held issue in the local store is `escalated` (a
publication recorded after it ends the wait). It reads the store alone, never the tracker, and a store
it cannot read exempts nothing. The loop's own `Stop` and Codex's keep their answers.

## The Claude Code mail waiter

Whatever the `Loop plugin` row says, `estigia setup claude-code` writes one more `Stop` group, after the ordinary
one and in a group of its own:

```json
{"hooks": [{"type": "command",
            "command": "\"<estigia>\" hook stop --agent claude-code --dialect claude-code --wait-mail",
            "asyncRewake": true, "timeout": 3600}]}
```

It waits for mail addressed to the session and wakes it by exiting 2; what it does is in
[`sessions.md`](sessions.md#waking-an-idle-claude-code-session). Setup rewrites it in place, so a second run adds no
duplicate, and uninstall removes it with the other Estigia hooks because it has Estigia's shape (below) while another
tool's `Stop` entries, the loop's included, stay. Only Claude Code gets it; Codex's `Stop` is the loop's.

## Which hook commands setup treats as its own

`setup` and `uninstall` take out, and `status` counts as the gate, a settings-file hook command only
when it has Estigia's shape: a program, then `hook` as its own word, then an event slug
(`pre-tool-use`, `session-start`, `session-end`, `stop`), read by `runs_estigia_hook` in
`src/setup/render.rs` over the words `src/setup/wiring.rs` splits a command line into. The program may
be quoted, contain spaces, be a Windows path or be a renamed copy. A command that merely contains the
text, such as `/home/me/bin/slack-webhook stop`, is another tool's and is written back byte for byte.
`render_cursor_hooks`' gate check and `setup::is_gated` use the same predicate.

## The optional goal and loop plugin

A third file, `goal-loop.js`, is written beside the other two while the `Goal loop plugin` row is
`on` (default `off`), marked `estigia:goal-loop`. It is independent of `Loop plugin`: either row can
be on without the other, `off` takes back only the file its own row wrote, and a plain uninstall
removes both. A `goal-loop.js` carrying no marker is neither replaced nor removed. The file holds no
path to the Estigia executable and runs no command at all — a test reads its source for both — and
its commands are the generic `/goal` and `/loop`, which is why the Estigia loop plugin's own commands
are named `/flow-loop` and `/flow-status` (an Estigia loop plugin written by an earlier build,
which registered `loop` and `goal`, is replaced by the next `setup` because it carries the same
marker).

- `/goal <condition> [--max <n>]` stores the condition for the session, in the location it was asked
  in, and enqueues a first prompt. After every `session.execution.succeeded` for that session the
  plugin counts the turn, and on every third turn (`JUDGE_EVERY`) and on the turn that reaches the cap
  asks an evaluator whether the condition holds; the turns between enqueue the condition and "Keep
  working toward it" without asking (issue #3). **Met** stops the goal with a visible message.
  **Not met** enqueues another prompt carrying the condition and the evaluator's reason, until the
  turn cap (default 25, `--max` overrides) stops it. Bare `/goal` prints the condition, the turns,
  the last judgement and whether it stopped; `/goal clear` stops it.
- `/loop [interval] <prompt> [--max <n>]` enqueues the prompt now and again every interval (`10s`
  up to 2147483647 ms, about 24 days, in `s`, `m`, `h` or `d`), or after every turn end of the session when no interval is
  given. `/loop stop` ends it. A timer is cleared on stop and on teardown, and re-armed from storage
  when a location boots again.

**The evaluator is `ctx.session.generate({ sessionID, prompt })`.** It is the plugin-side face of
OpenCode 2.0.20's `POST /api/session/:sessionID/generate`, whose own description is "Generate
transient text from the current session context without mutating session history", with a
`{ prompt }` payload and a `{ data: { text } }` response. That is the honest fit: the judge sees
the transcript the session already holds, no child session is created and none is left behind, and
nothing is added to the history the next turn reads. The host builds that context itself, so on
OpenCode only the cadence bounds what a judgement costs; the plugin cannot cut it. The prompt asks for `MET` or `NOT MET` on the
first line and a reason on the second; the plugin reads the first line case-insensitively, ignoring trailing punctuation, and **anything
else, an empty answer, a thrown error or a missing method stops the goal** — never continues it.
The answer is read as `{ text }` or `{ data: { text } }`, since which of the two the plugin
context returns is not measured (see [`honesty.md`](honesty.md)). Stops and reports go through
`session.synthetic` with `resume: false`, carrying the message as `description` (prefixed `Goal loop: `
unless it already begins `Goal`) as well as `text`, because OpenCode 2.0.20 renders only the
`description`, continuations through `session.prompt` with
`delivery: "queue"`, and state through `ctx.storage` under `goal-loop:goal:` and `goal-loop:loop:`.
Ownership follows the stored `directory`, never the event's location, exactly as the Estigia loop
plugin's does.

## The OpenCode gate plugin's mailbox

The gate plugin `estigia-workflow-authority.js` also delivers the session mailbox ([`sessions.md`](sessions.md)),
and it decides nothing by doing so. After a tool call completes it asks `estigia session deliver --session-id <id>
--runtime opencode`, and the first time it sees a session id it runs `estigia session start` for it, which registers
the session so another session can address it. OpenCode 1's `tool.execute.after` appends the text to the tool's
string result. OpenCode 2's `execute.after` hands it to `ctx.session.prompt` with `delivery: "steer"` and, when that
is absent or throws, to `ctx.session.synthetic`. On OpenCode 2 it reads nothing when the host offers neither of those two functions, so the rows stay pending. A call the gate refused never reaches the after hook, so it takes no
mail, and a result the plugin cannot append to is left without a read of the mailbox. Unlike the gate, this runs after
every tool, not only the writes. A host without an `execute.after` hook keeps the gate and delivers nothing.

## The OpenCode review relay plugin

`estigia setup opencode` writes a third single-file plugin, `estigia-review-relay.js`, beside the
gate plugin and the two optional loop plugins under the same `plugins/` directory, marked
`estigia:review-relay`. Unlike the loop plugins it is **ungated**: it is written on every OpenCode
install, because a machine with neither the `claude` nor the `codex` CLI cannot run a sealed
reviewer at all, and a machine with one reaches it only from an OpenCode run. No setting decides it — `Review
delegation` is `auto` and says nothing about which binary runs a reviewer, so a gate here would be a
row that decides nothing.

The plugin hooks OpenCode 2's `execute.before` and `execute.after` for the sub-agent tool, whether the host names it `task`, `agent` or `subagent`. When the
host launches a task naming one of the seven reserved reviewers — `review-lens`, `review-risk`,
`review-readability`, `review-reliability`, `review-readback`, `review-refuter`, `review-validator`
— the plugin asks the binary for the prompt `launch_review_input` recorded, with its job-bound output channel, and puts it in the
task input; when the task finishes it hands the reviewer's raw output back to the binary for
admission and replaces the result, so the orchestrator never receives the reviewer's answer. It also
accepts a task of any sub-agent whose input carries `estigia:review name=<role>`, for a host that has
no reviewer agent of its own. `estigia setup opencode` installs no reviewer agent, so the form a
host launches is the second: the `relay: true` answer on an OpenCode run carries
`launch: {agent, prompt, background: false}`, the built-in `general` sub-agent and the prompt
`<!-- estigia:review name=<role> -->`, and the plugin replaces that prompt with the binary's. A task
naming a reserved reviewer is relayed too, for a host that has defined one itself.

The binary appends an OpenCode-only output contract to the stored role prompt. Repository-required
acknowledgments may precede the result, followed by exactly one `<estigia-review-result job="ID">`
line, the complete role JSON, and a `</estigia-review-result>` line. Nothing but whitespace may follow.
Only that explicitly designated channel is parsed as a role answer; the binary never searches prose
for arbitrary JSON. Missing, repeated or incomplete delimiters and invalid role JSON are refused.
The job ID must equal the current attempt, so a delayed answer cannot satisfy a replacement attempt.
The whole response, including acknowledgments, is limited to 4 MiB. Other hosts keep their existing
JSON-only contract.

A rejected answer abandons its job and records no report. A subsequent collect can therefore answer
`capture-job-missing`: ask `review_status`, then call `launch_review_input` again for the still-offered
name and launch its returned agent/prompt. This creates a new job against the same immutable receipt;
do not republish merely to retry formatting. If the host cannot produce the required channel, preserve
the receipt and hand off review rather than treating visible JSON as approval.

`launch_review_input` selects the reviewer by the host that launched the run, read from the runtime
its run id names. On an `opencode` run with this plugin installed it answers `relay: true` before it
looks for a CLI, so the review is run by the session's own agent and subscription rather than by an
installed `claude` or `codex`. On every other run, and on an OpenCode run without the plugin, the
order is the first installed `claude` or `codex` CLI, then a review relay installed for any host,
then a coded `unavailable` naming the CLI to install. A run id whose runtime cannot be read keeps that
second order and never takes the relay ahead of a CLI. The detached runner's own selection has no host
and never relays.

The OpenCode relay supports foreground review only. A request with `background: true` is refused
before the host spawns a child. A foreground tool that returns a running child, including a host-side
transition to background, is refused without submitting its launch notice for admission. Tool errors,
incomplete native results and contradictory child identity/status metadata are also refused without
admission; their raw output is replaced with a diagnostic. Asynchronous completion would require a
correlated private notification channel, which this plugin does not register.

After such a refusal, the relay job has no completed result. Call `collect_review_input` once to
abandon its runner-less slot (`capture-runner-lost`), then ask `review_status` and relaunch foreground
on the unchanged publication. Do not republish or count a later visible background notification as
review evidence. If foreground completion is unavailable, retain the exact receipt and hand off.

The slot a launch was relayed for is remembered from `execute.before` by the call's `id`, because the
prompt `before` puts in the task input carries no marker and the sub-agent is `general`; `execute.after` reads it
there first and forgets it. A native structured subagent completion designates its answer in
`result.output.output`; `result.content` is a rendered `<subagent>` wrapper, never the review payload.
The plugin validates the native child identity/status against metadata and admits only that designated
completed string. Legacy unstructured results still join text parts of `content`, else `output`, and
replaces both with the admission note. Every command it runs has its standard input attached with the
shell's `<` redirect, an empty one for `prompt`, because Bun's shell has no `.stdin()` and a command
left without one inherits the host's terminal and blocks on it; where that cannot be attached the plugin
refuses with `authority_unavailable` and runs nothing. The OpenCode 1 `server` path takes the host's
`(input, output)` hooks, with the arguments in `output.args` before and the text in `output.output` after, and
throws `review-admission-refused` when the binary refuses an answer.

A plain `estigia setup opencode --uninstall` removes it, as it removes every plugin Estigia wrote: a
file left behind that shells out to an uninstalled binary would be a dangling file, not an
uninstall. A file carrying no `estigia:review-relay` marker is neither replaced nor removed. What
the relay does not prove is in [`honesty.md`](honesty.md).

## The Pi gate extension

Every `~/.pi/agent` below is Pi's agent directory: `$PI_CODING_AGENT_DIR` when that is set to an absolute path
(or one starting with `~`), `~/.pi/agent` otherwise. `estigia setup pi` writes nine things: the fenced directive into `~/.pi/agent/AGENTS.md`, the skill
into the shared `~/.agents/skills` tree, Estigia's server into `~/.pi/agent/mcp.json` under
`mcpServers` (`command`, `args: ["mcp"]`, an `exposure` and, on a Pi that defers, a `description`; see below), and four
extensions: `~/.pi/agent/extensions/estigia-workflow-authority.ts`, marked `estigia:workflow-authority`
like OpenCode's plugin, the review relay and the subagent runner, both described in the sections after the loop extensions, and the look extension, `~/.pi/agent/extensions/estigia-look.ts`, marked `estigia:look`; the two prompt templates,
described after the extension; and the theme `~/.pi/agent/themes/estigia.json`. The look and the theme are
[`The Pi look and theme`](#the-pi-look-and-theme). Where `Model routing` names
`developer` or `analyst`, setup also writes `~/.pi/agent/agents/developer.md` and `analyst.md`, under
the ownership record every delegated definition shares (see the agent-definitions section below). Pi loads the extension through its own TypeScript loader with no build step;
the file is plain JavaScript, which is also valid TypeScript, so the tests load the same bytes under
`node`.

The extension decides nothing. On Pi's `tool_call` event it runs
`estigia gate <tool> --input - --session-id <id> --runtime pi` in `ctx.cwd`, with the tool's input as JSON on the child's standard input, with the session id
from `ctx.sessionManager.getSessionId()`, and the same three outcomes OpenCode's plugin has: exit `0`
lets the call through, exit `1` or `2` returns `{ block: true, reason }` with the binary's standard
error as the reason, and a binary that is gone, a timeout or a signal — no exit code at all — logs
`went out unchecked` and lets the call through, because a hook that breaks does not deny. With a UI
the same sentence is also shown through `ctx.ui.notify(text, "error")`, because the person is the one
who most needs to know a write was not checked; the outcome does not change. The call is asynchronous
and bounded at thirty seconds: a binary that has not answered by then is killed and counts as having
given no answer, never as a refusal, so a hung binary cannot freeze Pi. The call reads at most eight
mebibytes from the binary; an answer larger than that is not a timeout and is not waved through: the
call is blocked with a reason saying the gate's answer was larger than the extension can read.

Pi decides what it loads from its own files, and those are control surface: `~/.pi/agent/settings.json`,
`~/.pi/agent/mcp.json`, the project's `.pi/settings.json`, `.pi/mcp.json` and `.pi/extensions/`, as well
as `~/.pi/agent/extensions/`. When `PI_CODING_AGENT_DIR` is set in the gate's own environment, the same files
under that directory, and its `AGENTS.md`, `AGENTS.override.md`, `agents/` and `prompts/`, are control surface too. A write to any of them is a boundary, because an `extensions` exclusion
or a project `mcp.json` override unloads the gate or the tool server from the next session. `estigia
doctor`'s `pi-config` row reads them and reports such an entry; it never rewrites one. What it cannot
tell is in [`honesty.md`](honesty.md).

Which calls reach the binary is decided by name and then by shape. `edit`, `write` and `bash` go
under their own names. Any other tool whose input has a string `command` is sent as `bash`, with its
input unchanged: gentle-shell's `bash_background` and `monitor` and Pi's `powershell` among them. A
tool whose input has a string `path` and a string `content` is sent as `write`, and one with a string
`path` and an `edits` array as `edit`. Only those exact keys are read, so a custom tool that names its
command or path differently is not gated. The binary classifies by the name it is given, so the
extension names the class and keeps the tool's real name for its own messages and the refusal card;
the text the binary prints says `bash` or `write`. `powershell` is gated as `bash` on purpose: the
shell classifier reads POSIX shell, so a PowerShell cmdlet that writes (`Set-Content`, `Out-File`,
`Remove-Item`) is not recognised as a write, while a `git` or `gh` command line, a redirect and
`powershell -Command` nested inside are read as they are in `bash`. Gating it as `bash` therefore
checks the claim for what the classifier can see and nothing more; leaving it out would check
nothing (see `docs/honesty.md`). A numeric
exit code it does not recognise is not that case: the binary did answer, in a way this extension cannot
read, so the call is blocked with a reason saying so and naming `estigia setup pi`, which rewrites the
extension. A call with no session id goes to the gate without `--session-id`, as OpenCode's does.

Both gate plugins hand the input on standard input, never as an argument: `--input -` tells the binary
to read the JSON from there, up to 32 MiB, and refuse anything unreadable or not JSON as it refuses a
malformed `--input`. An argument larger than the operating system's limit would fail to start the binary
and read as "did not answer"; standard input has no such limit. Pi's extension writes it with
`child.stdin.end` on the `execFile` child and ignores a write error, so a binary that exits first does not
crash the host; OpenCode's plugins redirect a `Buffer` into Bun's shell (`< ${stdin}`). A plugin written by
this version needs the binary installed with it, which `setup` always arranges; an older binary reads `-`
as the JSON, refuses with exit `1`, and so blocks the call.

A fourth code, `4`, is a question for the person. The extension adds `--can-ask` to the gate call only
when `ctx.mode === "tui"` and the call has a session id; `ctx.hasUI` is not enough, because Pi sets it
in `rpc` mode too, where a client answers dialogs programmatically. The binary honours the flag for
`--runtime pi` alone: a decision that would be asked then exits `4` with the question, in the words of
a refusal, on standard error; without the flag every such decision stays a refusal, exit `1`, so an
extension written before this flag never receives `4`. On `4` the extension shows
`ctx.ui.confirm("Estigia", <the binary's text>)` and lets the call through only on an explicit yes. A
decline, a cancelled dialog, a `confirm` that throws, and a `4` it did not ask for all return
`{ block: true, reason }` with the binary's text.

Pi has no subagents of its own: the extensions that add them start a child `pi` process, or run a child
session inside Pi's own process. The gate extension links a child to its parent in two ways. A session
whose header names a `parentSession` is a child of that session, in this process or another, and
the gate call carries `--parent-session-id <the parent's session id>`: the id exported as
`ESTIGIA_PI_PARENT_SESSION` when there is one that is not the child's own, and otherwise the `id` in the
parent's session file, or failing that the id in its file name. A session with no such header falls back
to the pid: the extension exports `ESTIGIA_PI_PARENT_SESSION` (its session id) and `ESTIGIA_PI_PARENT_PID`
(its process id) into `process.env` on `session_start` and on every `tool_call` of a session that is not a
child, so a switched session re-exports and a child `pi` spawned afterwards inherits both, and a process
whose id differs from the exported pid and whose session id differs from the exported session adds
`--parent-session-id <exported id>`. The binary then decides the child as the run derived from that id,
the parent's, exactly as a Claude Code subagent is. A child never exports anything, and the exporting
process never names its own session as a parent.

`estigia doctor`'s `pi-subagents` row names the Pi subagent runners it can see, from a `packages` entry
of the user or project `settings.json` or an extension in either `extensions/` directory whose name
says subagent, and says whether their children load the gate. A runner that starts children with
`--no-extensions`, or runs them inside Pi's process, or whose source cannot be read, is reported like a
skipped check with `WARNING` and its name; it installs and rewrites nothing. What it cannot tell is in
[`honesty.md`](honesty.md). Estigia's own runner (see *The Pi subagent runner extension* below) is named in the same row: alone,
as installed, or beside any other runner it found, in which case the row says a delegation can reach
either. `estigia setup pi` says the same in a note when it finds another runner while installing its
own; it does not refuse and does not skip the install.

The same file also draws two things for the person, and neither decides anything. When `ctx.hasUI`
is true it keeps a status line, key `estigia-claim`, through `ctx.ui.setStatus`: on `session_start`,
`tool_result` and `agent_settled` it runs `estigia goal --json --session-id <id> --runtime pi` in
`ctx.cwd` asynchronously with a five-second bound and shows the `issue`, `state` and, when the claim
is not live, the `standing` that answer carries; an answer with no issue clears the line, and an
answer that did not arrive, did not parse or lacks `issue` shows `estigia: claim unknown`, never the
previous claim. A child process asks about the exported root session, the run its gate calls are
decided as. When the gate refuses, the extension still returns `{ block: true, reason }` and, with a
UI, also appends a session entry `estigia-refusal` through `pi.appendEntry`, which a renderer
registered with `pi.registerEntryRenderer` draws as a heading naming the tool followed by the
reason, the binary's standard error unchanged and only wrapped to the width: each line carries a `▎`
bar in the error tone and is painted on `toolErrorBg`. It is an entry and not
a `pi.sendMessage` message because Pi converts every custom message to a user message for the model
whatever its `display` flag says, which would put the reason in front of the model a second time.

Pi has no `SessionStart` hook, so the extension carries the same context through `estigia session`.
On `before_agent_start` it runs `estigia session start --session-id <id> --runtime pi` in `ctx.cwd`
asynchronously with a five-second bound, and sets `systemPromptOptions.sections.estigia` to the
binary's text, which Pi renders as an `<estigia>` section appended to the prompt; it returns nothing,
so it never replaces the prompt the way a returned `systemPrompt` does. The text is
`harness::hook::session_start_text`, which wraps `session_start_response`, the function the Claude
Code hook uses, so the run id, what the run holds, the `Issue requirement` note, the build note (absent in a repository that does not hold the
build's commit, `docs/honesty.md`) and the
queue note are one text for both apart from the queue count. It asks on every run rather than caching: the answer depends on a
claim made since the last turn and costs one local process, and a section that changed is what Pi
persists as a patch, so a compaction keeps the current one. The queue note in this section states that GitHub writes are queued without counting them: a changed section
rewrites the leading prompt for most providers, and a count that grew with every queued write would do that
on every turn while nothing else had changed. A binary that does not answer, answers
with a code, a signal or nothing, or is past the bound, gives a section reading `Estigia: session
context unknown (<why>)`, never an empty one and never the last text. A child process asks about the
exported root session, like the status line.

`session_start` records its `reason` and the id in the first line of `previousSessionFile`, and
forwards both as `--reason` and `--previous-session-id`. When the reason is `fork` or `new`, this
session holds nothing and the previous session's run holds issue #N, the text adds a paragraph saying so
and naming `estigia release --run-id <previous run>`, after which this session can claim the issue. The
claim is never moved. On `session_shutdown` the extension runs `estigia session end --session-id <id>
--runtime pi --reason <reason>` once per session and reason; the binary forgets the run's pointer unless
the reason is `new`, `resume`, `fork` or `reload`, because a session that was only replaced may be
resumed, and a reload keeps the same session id. A child process never ends the session it was started
under.

While the root session is idle the extension also polls the mailbox every 10 seconds and wakes the session with
`pi.sendMessage`; [`sessions.md`](sessions.md#waking-an-idle-pi-session) has the order and what it refuses.

Pi's own argument names need no translation. Its `write` takes `path` and `content`, its `edit` takes
`path` and an `edits` list, and its `bash` takes `command`; `harness::path_argument` and the shell
reader already accept `path` and `command`, so the rule for what a write targets stays in the binary
and the extension passes `event.input` through unchanged. The run id the gate derives is
`session::run_id("pi", <session id>)`: `pi-`, the first eight alphanumeric characters of Pi's UUIDv7
session id, then eight hex digits of its FNV-1a hash.

Pi's default MCP `exposure` is `codemode`, which registers a server's tools for codemode scripts but
does not declare them to the model, so the entry carries an explicit `exposure`, chosen by the Pi
version `pi --version` reports on `PATH` when `setup` or `sync` runs (or the guided screen):

| Pi | `exposure` | Why |
|---|---|---|
| 1.2.0 or later | `deferred`, with a `description` | Pi lists the server in the system prompt and loads its tools on demand through `tool_search`, instead of declaring all of them on every request |
| before 1.2.0 | `direct`, no `description` | The tools are declared like built-ins; `deferred` and the server listing do not exist |
| not found, or a version that does not parse | `direct` | The conservative value every Pi can use |

The `description` is `Estigia workflow authority: claim an issue, record tasks and reviews, transition
state, release`. Pi uses it to list the server and to rank `tool_search` results, and none of that
applies to a `direct` server, so that entry carries none. `estigia setup pi --dry-run` prints a `note:`
naming the exposure it would write and the version reading behind it. Estigia owns the entry and
rewrites it whole on every setup, so an entry from an older setup is migrated on the next run, an
`exposure` an operator edited into it is put back, a Pi upgraded to 1.2.0 gets `deferred` on the next
setup, and a second run with the same Pi changes nothing. `estigia doctor`'s `pi-config` row names the
exposure it finds in the user `mcp.json` and the Pi version, and says so when a Pi at 1.2.0 or later
still carries `direct`; that is a warning inside an otherwise fine row, not a break. Every other
adapter's entry is unchanged.

The same extension registers one command, `/estigia-doctor`, with `pi.registerCommand`. It runs
`estigia doctor` in `ctx.cwd` asynchronously with a sixty-second bound and shows what the binary
printed, unchanged except that trailing newlines are dropped; standard error follows standard output
when the binary wrote to both, so a refusal line is not lost. With a UI the text is appended as a
session entry `estigia-doctor` through `pi.appendEntry` and drawn by a renderer registered with
`pi.registerEntryRenderer`, wrapped to the width and framed like the refusal card: a `▎` bar in the
border tone on `toolPendingBg` when the binary answered, and in the error tone on `toolErrorBg` when
it did not; it is an entry and not a
`pi.sendMessage` message for the reason the refusal card is, so the report never reaches the model. A
Pi without `appendEntry` gets the whole text through `ctx.ui.notify` instead. Without a UI the
report goes to standard output. A binary that is gone, a timeout, a signal, an output larger than the
extension can read, or an exit with nothing printed is never shown as a report: the text reads
`estigia doctor did not answer (<why>)` and is drawn as an error. A second command,
`/estigia-changes`, does the same with `estigia changes` and draws the same entry; it reads the
listing and decides nothing. The command decides nothing and
changes nothing; the report is the binary's.

The extension also keeps a widget, `estigia-changes`, through `ctx.ui.setWidget`, refreshed at
session start and after each settled turn. It runs `estigia changes --json` in `ctx.cwd` with a
twenty-second bound and counts the entries: `uncommitted work in N checkouts`, with `, M unreadable`
when git could not be read in some, and `N checkouts unreadable` when that is all there is, each
followed by `/estigia-changes`. A repository with nothing uncommitted and nothing unreadable clears
the widget. A binary that is gone, a timeout, a signal or output that is not the listing shows
`uncommitted work unknown`, never a count. Without a UI, or on a Pi whose `ctx.ui` has no
`setWidget`, nothing is run. The widget counts and decides nothing.

Setup also writes two prompt templates into `~/.pi/agent/prompts/`: `claim.md` and `review-status.md`,
each a markdown file with a `description` and an `argument-hint` in its frontmatter and a body of
one instruction that names the Estigia MCP operation to call (`claim`, with the issue number given as
the template's argument and the run id Estigia reported at session start; `review_status` for the
issue). They carry no rule of their own. Each holds the comment `<!-- estigia:pi-prompt -->`, which is
the ownership marker: setup writes a template that is absent or carries it, a second setup changes
nothing, uninstall removes exactly the templates that carry it, and a `claim.md` somebody else wrote
is neither replaced nor removed. Setup writes them on every install, not under the `Loop plugin`
setting, because they are not part of the loop. There is no `flow` template: Pi already registers
`/skill:flow` for the skill setup installs. Pi reads templates from the top of the directory only and
expands `$ARGUMENTS` itself; run `/reload` in an open session after setup. A prompt template tells the
run what to do, so `~/.pi/agent/prompts/` and the project's `.pi/prompts/` are control surface like
Claude Code's `commands/` directory, and `prompts/` is added to the files watched under a relocated
`PI_CODING_AGENT_DIR`. `doctor` does not read them: it reads what unloads the gate, and a missing
template unloads nothing.

A plain `estigia uninstall pi` removes the directive block, Estigia's `mcpServers` entry, the four
extensions, the two prompt templates, the look and the theme, and leaves every other server and every other line of `AGENTS.md`. An extension at that path
carrying no marker is neither replaced nor removed, and `status` then reports the gate off. `--scope=workspace`
refuses Pi. What none of this proves — chiefly that Pi acts on the returned `block` — is in
[`honesty.md`](honesty.md).

## The Pi look and theme

`estigia setup pi` writes two artefacts that draw and decide nothing.

`~/.pi/agent/themes/estigia.json` is a Pi theme named `estigia`, appearance `dark`, in the palette of
the project banner: a night background, an azure accent, cyan code and azure links, gold headings and
borders, olive success, ember error and amber warning. It carries all 51 color tokens Pi's schema
requires and no token outside the set Pi accepts, and every value is either a literal color or a
variable the file defines. Pi's schema rejects an unknown top-level key and an unknown color token, so
the file cannot hold a marker: **it is Estigia's while its bytes are the embedded theme.** Setup
writes an absent theme or one that is byte-identical, uninstall removes only a byte-identical one, and
a theme somebody edited is neither replaced nor removed.

`~/.pi/agent/extensions/estigia-look.ts` is a display-only extension marked `estigia:look`, with the
same ownership rule as every other extension. It registers one command, `/estigia-look on|off`, and on
`session_start` installs a startup header through `ctx.ui.setHeader` and a footer through
`ctx.ui.setFooter`. The header draws the wordmark, the tagline `Light. Local. Agentic.` and the
abbreviated working directory, branch and model: the wordmark alone below 24 columns, the wordmark and
the subtitle `AI harness` from 24, the tagline, the directory and the branch from 40, the model from
60, and the rule from 40. The
footer's first line carries `estigia`, the claim read from the `estigia-claim` status, the git branch
from `footerData.getGitBranch()`, the model and thinking level, the context percentage and the session
cost; its second line carries the statuses other extensions published, so installing the look never
hides one. As the terminal narrows the line gives segments up in a fixed order — cost, then model,
then branch, then context — and the claim is never dropped. In any mode but `tui`, or on a host whose
`ctx.ui` has no `setHeader` or `setFooter`, nothing is installed. `/estigia-look off` puts Pi's own
header and footer back; the choice lasts the session and is written nowhere. The extension registers
no tool, sends no message, appends no entry, runs no process and imports nothing but `node:fs`.

Both are written on every install, like the gate plugin and unlike the loop plugin. Neither touches
`~/.pi/agent/settings.json`, which is control surface: the theme is selected by the person with
`/settings` → Theme or `pi --use-theme estigia`, and the run that writes it says so in a note.
`PI_CODING_AGENT_DIR` moves both with the rest of the agent directory.

## The Pi loop extension

With the `Loop plugin` row `on` for Pi (default `off`), `estigia setup pi` also writes
`~/.pi/agent/extensions/estigia-loop.ts`, marked `estigia:loop` like OpenCode's `flow-loop.js`. One
file holds both the loop and its two commands, `/flow-loop` and `/flow-status`, registered through
`pi.registerCommand`. That mirrors OpenCode, whose plugin registers its own commands, and not Claude
Code, whose commands are markdown prompt files: Pi's prompt templates cannot hold a loop's state or run a
command, so they would put the decision somewhere other than the binary.

The extension decides nothing. `/flow-loop [objective] [--max n] [--wait minutes]` reads the session's
run id with `estigia goal --json --session-id <id> --runtime pi`, stores the goal in the extension's
memory and sends the first turn. On Pi's `agent_settled` event — the final boundary, after retries and
compaction, not `agent_end` — it asks again, and while Estigia says the run is live, or another issue is
claimable or dischargeable, it sends the next turn with `pi.sendMessage(..., { triggerTurn: true,
deliverAs: "followUp" })`. Work that is only held or blocked on a condition that may still come true waits
on a timer (`--wait`, default five minutes) and asks again; an answer that arrived but could not be read
is read again on the wait timer, three reads in all, and then stops the loop, as for OpenCode. The objective, the preamble, the
held text a live claim is continued with, and the `--max` counting are the ones `harness::loop_text` renders for OpenCode.

Five things differ from OpenCode by design. The binary is called asynchronously with a 30-second
timeout, and a timeout, a binary that could not be run or an exit with no code is "did not answer": the
loop stops and says so, and never reads it as work or as done. After every `await` the session id is
read again and the loop acts only if it is unchanged. A session with a `parentSession` in its header
refuses `/flow-loop`, so a child session does not run one; no environment variable marks a child. A turn
Pi reports as aborted stops the loop. There is no cross-process storage: the goal lives in the extension's
memory, so `/flow-status` after a restart reports that nothing is stored.

`off` removes the file when it carries `estigia:loop`; a file without the marker is left alone, and
`estigia uninstall pi` removes it whether or not this install asked for it. The generic `goal-loop.js` is
not written for Pi; its own extension is [below](#the-pi-goal-and-loop-extension). What was read from Pi's source and what was not is in [`honesty.md`](honesty.md).

## The Pi goal and loop extension

With the `Goal loop plugin` row `on` for Pi (default `off`), `estigia setup pi` writes
`~/.pi/agent/extensions/estigia-goal-loop.ts`, marked `estigia:goal-loop` like OpenCode's `goal-loop.js`.
It is independent of the `Loop plugin` row, `off` takes back only the file its own row wrote, a plain
uninstall removes it, and a file at that path carrying no marker is neither replaced nor removed. It
registers `/goal` and `/loop` through `pi.registerCommand`, names that the Estigia loop extension's
`/flow-loop` and `/flow-status` never shadow, and holds no path to the Estigia executable and runs no
command; a test reads its source for both. OpenCode's semantics are kept: `/goal <condition> [--max n]`,
bare `/goal` to report, `/goal clear`, `/loop [interval] <prompt> [--max n]` and `/loop stop`.

Four things differ from OpenCode by design. The judge is the session's model, asked through
`ctx.modelRegistry.streamSimple` with a plain-text rendering of the session's user, assistant and
tool-result messages (a user or assistant message cut at 4,000 characters, tool output at 2,000, the
whole at 20,000 keeping the newest) and the OpenCode judgement prompt, asked on every third turn
(`JUDGE_EVERY`) and on the turn that reaches the cap as for OpenCode, with the turns between continuing
the goal without a call (issue #3); the call carries an `AbortSignal` that fires after 120 seconds, and a timeout, a thrown
error, a stop reason of `error`, no model or an answer whose first line is neither `MET` nor `NOT MET`
stops the goal and says so. A goal continues on `agent_settled` only while `ctx.isIdle()`, and a turn Pi
reports as aborted stops the goal and a self-paced loop; an interval loop runs on its timer and ignores
turn ends. State is in memory and `session_shutdown` clears it, so a restart, `/new` or `/resume` ends a
goal or loop, where OpenCode re-arms from plugin storage. Bare `/loop` is refused as for OpenCode, and so is a `--max` with no whole number after it;
`/loop status` reports the loop, because Pi has no storage to read it back from. A session with a
`parentSession` in its header refuses `/goal` and `/loop`; no environment variable marks a child. Continuations
go out as `pi.sendMessage(..., { triggerTurn: true, deliverAs: "followUp" })` and every stop and report is a
`ctx.ui.notify`, so none of them starts a model turn. After every `await` the session id and the
goal's identity are read again, so a `/goal clear`, a replacement or a session change that lands while the
judgement is in flight is not overwritten. What was read from Pi's source and what was not is in
[`honesty.md`](honesty.md).

## The Pi review relay extension

`estigia setup pi` also writes `~/.pi/agent/extensions/estigia-review-relay.ts`, marked
`estigia:review-relay` like OpenCode's relay and, like it, **ungated** and written on every install: a
machine with neither the `claude` nor the `codex` CLI cannot run a sealed reviewer at all, and on a
machine with one it is reached only after the CLI order has found none. The file is plain JavaScript, which is also valid TypeScript, so the tests
load the same bytes under `node`.

Pi has no sub-agent tool for a relay to intercept, so the extension registers one tool of its own,
`estigia_review_lens`, taking the reserved reviewer name (`Role::ALL` is the only list; the extension is
rendered from it). The tool does three things and decides none of them:

1. runs `estigia review prompt --name <slot> --session-id <id> --runtime pi` in `ctx.cwd` and takes
   standard output, verbatim, as the prompt;
2. runs `estigia review model --name <slot> …`, which prints the model the `reviewer` routing row named
   when `launch_review_input` recorded the job, and runs **one** completion through
   `ctx.modelRegistry.streamSimple`: a single user message holding the prompt, no system prompt, no tools,
   no session, bounded by the same deadline a CLI reviewer has (`DEFAULT_DEADLINE`), and authenticated by
   the registry itself, extension-registered providers included;
3. pipes the raw answer into `estigia review admit --name <slot> …` and returns only a note that it was
   admitted, so the model that called the tool never reads the answer.

**Which model.** The `reviewer` row of `Model routing` is the one place that names it, read once by the
binary at launch. A `provider/id` that Pi's registry lists runs on that model. `inherit`, or no row at
all, runs on the session's current model, which is what the same value means for an OpenCode sub-agent
(no model flag). A row Pi's registry cannot resolve — a bare alias such as `opus`, or an id it does not
list — is refused `reviewer-unavailable` before any completion runs; there is no silent fall back to the
session model for a model the operator named. A failed completion is refused with the codes a CLI
reviewer's failures already have (`reviewer-deadline`, `reviewer-error-result`, `reviewer-no-answer`)
and nothing is admitted, so the slot stays offered.

The two refusals the OpenCode relay keeps apart are kept apart here, by the same reading of the binary's
exit codes (one shared `parseCode`): an authority that did not answer is `authority_unavailable`, never
`capture-result-stale`, and a refusal is read from standard error only after standard output is read to
the end, so a binary that exits before reading the answer does not hide the reason it refused.

`launch_review_input` takes this relay behind the CLIs: with neither on `PATH` and a review relay
installed for any host, it records the relay job and answers `relay: true`; with neither, it is refused `unavailable` naming the CLI to install. `estigia uninstall pi` removes the file;
a file carrying no `estigia:review-relay` marker is neither replaced nor removed. What the relay does not
prove is in [`honesty.md`](honesty.md).

## The Pi subagent runner extension

`estigia setup pi` also writes `~/.pi/agent/extensions/estigia-agents.ts`, marked `estigia:agents`,
**ungated** and written on every install, like the review relay beside it. `--dry-run` reports it,
`estigia uninstall pi` removes it, and a file carrying no `estigia:agents` marker is neither replaced nor
removed. The file is plain JavaScript, which is also valid TypeScript.

It registers one tool, `estigia_subagent` (`agent`, `task`, optional `label`), and the command
`/estigia-agents`. The tool reads `<agent dir>/agents/<agent>.md`, the definition setup writes for
`developer` and `analyst`, and starts `pi --mode rpc --session-dir <temporary directory>` with that
definition's `model` and `tools` when it names them and its body as `--append-system-prompt`, in its own
process group, **inheriting the environment unchanged**. The gate extension has exported
`ESTIGIA_PI_PARENT_SESSION` by then, so the child's own gate extension adds `--parent-session-id` and the
binary decides every write the child makes as the parent's run. The runner decides nothing about authority;
it spawns, relays and draws. Two checks come before any work is sent:

- the tool is refused, and nothing is spawned, when the gate extension has not exported
  `ESTIGIA_PI_PARENT_SESSION`, because the child could not be linked to a claim;
- the child is asked for its commands (`get_commands`) and must list `estigia-doctor`, a command only the
  gate extension registers. A child that does not is stopped before it receives the prompt, and the tool
  answers that it did not load the gate. The runner never passes `--no-extensions`.

The tool's answer is the child's last assistant text once `agent_settled` arrives. A dialog the child asks
for is cancelled, since a child in RPC mode has nobody to ask.

The **card** is drawn with `ctx.ui.setWidget("estigia-agents", …)` when the session has a UI: one row per
running child, then the three most recent finished ones for sixty seconds, each `label (agent) model
tokens tok $cost elapsed activity`. The model is from `get_state`; tokens and cost are the sum of the
`usage` on each finished assistant message plus the one in flight; the activity is the last tool. A tool
that ends in error shows its result text, which is the binary's refusal for a gated write, on following
lines unchanged. Rows are redrawn once a second while anything is shown, and the widget is cleared when
nothing is. Drawing makes no model call.

`/estigia-agents` lists the card, `/estigia-agents stop <id>` stops one child and `/estigia-agents stop
all` asks for confirmation when there is a UI. A stop sends `SIGTERM` to the child's process group and
`SIGKILL` three seconds later; the tool then answers that the child was stopped. Aborting the tool call
stops it the same way.

The **view** is `/estigia-agents` with no argument when the session has a UI, and the shortcut
`ctrl+alt+a`: a `ui.custom` overlay listing running children first, then finished ones, then the tasks
kept from earlier in the session's history, with the selected child's transcript below. The transcript
is what the runner already read from the child's events: the task, each tool started, each result or
refusal, each assistant message and each follow-up sent, the last two hundred lines. Keys: up/down or
`k`/`j` select, `s` stops the selected child, `x` stops every running child after a confirmation, `f`
asks for a follow-up and sends it to the selected running child as an RPC `steer`, `q` or escape closes.
The view is redrawn once a second while open and calls no model.

**History.** When a child finishes, fails or is stopped, the runner appends one `estigia-agent` entry
with `pi.appendEntry`: id, agent, label, status, model, tokens, cost, times, the task and the answer
(first 4000 characters). Pi keeps such entries in the session and never sends them to the model; the
runner calls no other method that could on this path (`sendUserMessage`; `sendMessage` is used only by
the child messages below). On `session_start` the runner reads them back from
`ctx.sessionManager.getEntries()` so a resumed session lists them in the view, with the task and answer as
the transcript.

**Parent and child messages.** Three more tools, none of which touches the gate or a claim; a delivered
message is text for the model and never a permission decision.

- `estigia_subagent_message` (`id`, `text`), on the parent: sends the text to a running child as an RPC
  `steer`, through the same writer the view's `f` uses. A finished or unknown child is a tool error.
- `estigia_parent_message` (`kind`: `notification` or `query`, `text`), on the child. It is registered
  only in a process the runner spawned, which the runner opens an IPC channel to at spawn
  (`stdio: [pipe, pipe, ignore, ipc]`, so `process.send` exists in the child; this needs the `pi` command to
  be a Node program). A `notification` returns once it is sent. A `query` carries a request id and blocks
  that tool call until the parent answers, for at most 120 seconds, then the tool is an error; the error
  also arrives if the channel closes first.
- `estigia_subagent_reply` (`id`, `request_id`, `text`), on the parent: answers a pending query, and the
  child's `estigia_parent_message` result is the text. An unknown or already-answered request, or a child
  that has finished, is a tool error.

**Background.** `estigia_subagent` takes `background` (boolean, default false). Foreground, the tool blocks
the parent's turn until the child settles, so the parent model cannot call anything else meanwhile: it can
neither steer the child nor answer it. With `background: true` the tool returns at once with the subagent
id and label, and when the child settles its final answer, or its failure, reaches the parent as a
`sendMessage` follow-up with `triggerTurn: true`, so an idle parent is woken. Steering and replying
therefore need `background: true`, and the tool descriptions say so. A query from a child whose parent is
blocked on it in the foreground fails in the child at once, with a message saying the parent is waiting
and the question belongs in the final answer; it is not relayed. A notification from a foreground child is
still queued for after the call returns.

The parent receives a notification or a query as `pi.sendMessage` with `deliverAs: "followUp"` and
`triggerTurn: true`, the same call the loop uses: Pi queues it for the next turn boundary of a busy session
and starts a turn in an idle one. A query's message names the child id and the request id so the model
can call the reply tool.

## What setup writes into Codex's configuration

A Codex that enforces hook trust (0.159 measured; the `trusted_hash` field exists from 0.129) runs a hook from the user's `hooks.json` only when it is trusted, and skip an
untrusted one without a message. Registering the gate in `~/.codex/hooks.json` is therefore not
enough: `setup codex` also writes one table into `~/.codex/config.toml` for each Estigia-owned hook,
in the shape Codex writes when a person trusts a hook in its own interface:

```toml
[hooks.state."<hooks.json path>:pre_tool_use:<group>:<handler>"]
trusted_hash = "sha256:<hex>"
```

The key is the absolute path of `hooks.json`, the event in snake case, and the group and handler
index in that file. The hash is Codex's own: SHA-256 of the canonical JSON of the hook's normalised
identity — the event, the matcher, and the handler's type, command, timeout (600 when absent),
`async` and `statusMessage`. It moves whenever the registered command, matcher or timeout moves, so
`setup` recomputes it on every run, and a hook edited by hand after it was trusted is reported by
`doctor` as trusted against another hash. `hooks.state` is edited through a format-preserving TOML
parser (`toml_edit`), so a comment, the order of keys, spacing, CRLF line endings and a leading BOM
all survive, and every spelling of the same table — `[hooks.state."<key>"]`,
`[hooks."state"."<key>"]`, `[ hooks.state."<key>" ]`, a single-quoted key, a dotted definition —
names one entry rather than being duplicated. `[mcp_servers.estigia]` is still edited by the line: the removal runs through its keys, including past a comment that a later key follows, and stops at the next table header or at a comment with no key after it, which stays.
Before writing, `setup` re-parses the whole file it is about to write and reads the trust back under
`hooks.state`: a file that does not parse, or whose trust is not the one the entry earned, is refused
rather than written, and that guard covers the `[mcp_servers.estigia]` edit in the same file.

- **Re-running `setup` keeps Estigia's entry where it was** in the event's array. Trust is keyed by
  position, so moving the entry would silently untrust every hook after it.
- **A hook that moves takes its old entry with it.** Setup removes the entry for the position Estigia's
  hook held before, and writes the one for the position it holds now. An entry left at a position by
  somebody else's hand is not touched.
- **A `config.toml` that defines `hooks` or `hooks.state` as an inline table is refused** before
  anything is written, because a second table under the same key would make the whole file invalid
  and stop Codex reading its configuration. A dotted definition (`hooks.state.<key>.trusted_hash = …`)
  and every valid spelling of the table header are the same table, so they are recognised and edited
  in place rather than refused or duplicated. A `["hooks.state"]` key is a different key and is not
  read as the table.
- **`uninstall` removes the `trusted_hash` key it wrote**, and the table with it when nothing else
  is under it. An `enabled = false` line, or any other key an operator put under the same table,
  stays. The keys come from the `hooks.json` being uninstalled, so an uninstall run after that file
  is already gone has nothing to name.
- **A Codex older than the trust mechanism ignores the table.** Its `config.toml` reader does not
  reject unknown keys; `docs/honesty.md` says which versions were read and what that does not prove.

`estigia doctor` reads both files and answers for Codex by what Codex would do: `live` only when the
trust entry holds the hook's current hash and the hook is not disabled.

## Which runtimes Estigia adapts, and on what evidence

An adapter is a claim that Estigia can stand in front of a runtime, and the claim rests on evidence
read from that runtime rather than on a wish. A runtime is admitted when both of these can be
verified from what the runtime itself ships — its published configuration reference, its own source,
or its documented file layout:

- **an instruction file the host loads into every session**, where the short directive goes; and
- **where the host discovers a skill**, either a root of its own or the shared `~/.agents/skills`
  tree the Agent-neutral row names.

Where the runtime also exposes an event able to refuse a tool call, the gate hooks are registered
there too; `estigia status`'s `harness` line reports `gate on` only where one is wired, and the gate
stays off for a runtime with no such event rather than being faked. A runtime whose skill discovery
is read from a page rather than from its own shipping source is admitted in the tier below — ZCode is
the current one — because the weaker evidence is worth stating rather than hiding.

What each admitted runtime carries is held in one place, `src/setup/capabilities.rs`'s `canonical`
table, and every adapter is checked against it at compile time; a runtime with no row there carries
nothing. `docs/honesty.md` says what that table does and does not establish.

**One managed block per runtime, and the same pointer in each.** The directive is the only block
`setup` fences in an adapter's instruction file, between `BEGIN ESTIGIA WORKFLOW AUTHORITY` and
`END ESTIGIA WORKFLOW AUTHORITY`. A second `setup`, or a first one over a block an earlier build
wrote, replaces the span in place and keeps what the operator wrote around it; it never appends a
second block. It carries the three rules and names the skill, and it does not restate the FLOW contract
or the review rules: those are in the skill the block names, so a runtime reads them where the tokens
are paid for when they are needed. The binary-launched review (`launch_review_input` and
`collect_review_input`, through a `claude` or `codex` CLI on the machine) depends on that CLI and not
on the runtime, so the skill describes it for every adapter and nothing is stripped per runtime.
Wording that is only true of a review relay is held by `Capabilities::review_relay`, `true` for
OpenCode and Pi, whose relays are built, and a test fails when a runtime without one is rendered a
relay marker (`relay`, `review transport`, `--input`) in its directive or in a review section.
[`parity.md`](parity.md) says how this differs from gentle-ai v4's per-runtime block.

Three things follow, and they are the shape of the rule rather than three separate judgements:

1. **A listed adapter does not promise a gate.** It promises the skill and the directive where the
   root resolves, and the hook where the runtime can refuse: the tables below give the root each row
   resolves and whether it has an agent-definitions surface, and `estigia status`'s `harness` line
   gives whether its gate is wired. A row with no agent-definitions surface and no refusing event is
   still adapted, for the workflow text it carries.
2. **An unadapted runtime gets nothing.** Estigia is not in front of it, installs no file for it, and
   has no branch to protect. There is no refusal line to read because there is no run for one to fire
   in; the only refusal is `setup`'s own, and it names the agents it knows when asked for a slug it
   does not. Nothing here invents a command: adding a runtime is a change to the table and its
   evidence, which is not something a user runs.
3. **Admission is per runtime and never inherited.** A second adapter sharing another's instruction
   file or skill root is a decision its own row records; nothing is admitted by resembling a row
   already here.

A request for `command-code` gets the answer any runtime gets: it is
admitted when its instruction file and its skill discovery are verified from what it ships. Neither
has been, so it is not on the table, and the rule rather than the asking decides when it is. Counts
that read the table from prose drift, and the ones known to are left to the work that
owns those sentences.

## Where each root resolves, and what governs it

Every adapter's files sit under one resolved home directory — `HOME`, or `USERPROFILE` when `HOME`
is unset, with `HOMEDRIVE`+`HOMEPATH` and the OS's own user-directories lookup as further fallbacks,
in that order. Some variables name a root of their own instead — `XDG_CONFIG_HOME`, `APPDATA` on
Windows only, `CLAUDE_CONFIG_DIR` for Claude Code, and `DSH_HOME` for DeepSeek Harness — each with
its own home-relative fallback when that variable is absent or not absolute.
Which adapters those are is in the table below rather than counted here, so that adding
one does not leave a number behind to go stale.

The `Root` column below is where an adapter's *instructions* resolve, not the whole of what it
resolves under: Claude Code, Codex and DeepSeek Harness have a skill root of their own —
`$CLAUDE_CONFIG_DIR/skills` (falling back to `~/.claude/skills`), `~/.codex/skills` and
`~/.dsh/skills` — and every other row also writes the shared skill tree at the Agent-neutral row's
own root, `~/.agents/skills/<flow>`, governed the same way that row's `Root` column already says.

| Adapter | Root | Governed by |
|---|---|---|
| Agent-neutral (`~/.agents`) | `~/.agents` | Home |
| Claude Code | `$CLAUDE_CONFIG_DIR`, and `$CLAUDE_CONFIG_DIR/.claude.json` for the MCP registration — falling back to `~/.claude` and `~/.claude.json` | `CLAUDE_CONFIG_DIR`, falling back to home |
| Codex | `~/.codex` | Home |
| OpenCode | `$XDG_CONFIG_HOME/opencode` | `XDG_CONFIG_HOME`, falling back to `~/.config` |
| Pi | `$PI_CODING_AGENT_DIR` (a leading `~` is the home), falling back to `~/.pi/agent` | `PI_CODING_AGENT_DIR`, followed wherever it points and not contained under the home; a relative value is dropped with a note — see [`honesty.md`](honesty.md) |
| Gemini CLI (retired, uninstall only) | `%APPDATA%\gemini` on Windows, `~/.gemini` elsewhere | `APPDATA` on Windows; home elsewhere |
| Cursor | `~/.cursor` | Home |
| Continue (retired, uninstall only) | `~/.continue` | Home |
| Cline (retired, uninstall only) | `~/.cline` | Home |
| Crush (retired, uninstall only) | `$XDG_CONFIG_HOME/crush` | `XDG_CONFIG_HOME`, falling back to `~/.config` |
| Windsurf (retired, uninstall only) | `~/.codeium/windsurf` | Home |
| Qwen (retired, uninstall only) | `~/.qwen` | Home |
| ZCode | `~/.zcode` | Home |
| DeepSeek Harness | `$DSH_HOME`, falling back to `~/.dsh` | `DSH_HOME`, falling back to home |

**Where an agent-scoped row lives on a shared root.** The adapters that write the shared
`~/.agents/skills/<flow>` tree have no skill directory of their own, so the rows answered per agent
(`Profile`, `Delivery authorisation`, `Transition authorisation`, `Model routing`, `Loop plugin`,
`Goal loop plugin`) are written to `estigia.<slug>.md` beside that tree's `SKILL.md`, never into
the `SKILL.md` table: an agent with no such file reads the table, so a row written there reaches every
one of them. `config set` and `config edit` without `--agent` refuse such a row
(`setting-needs-an-agent`) and write nothing, and the answer to a `config set` names the file it
wrote. `estigia doctor` reports a non-default agent-scoped row in a shared `SKILL.md` on the `contract`
row of each agent with no file of its own, and names `config set --agent <slug> <setting> <default>`
without `--contract`: that writes `estigia.<slug>.md` and leaves the shared table, which every other
agent on the root reads, untouched. `config set --agent <slug> --contract` on such a row is refused
(`contract-is-shared`) and writes nothing. Adapters with a skill directory of
their own (Claude Code, Codex, DeepSeek Harness) keep these rows in their own `SKILL.md`, which
`--agent <slug>` writes without `--contract`.

`APPDATA`, `XDG_CONFIG_HOME`, `CLAUDE_CONFIG_DIR` and `DSH_HOME` are read only when the caller left
the home to the process environment — a caller that names its own home explicitly and names none of
these gets all of them under that home too, with no separate variable to move along with it — and a
caller that names one of `APPDATA`, `XDG_CONFIG_HOME` or `CLAUDE_CONFIG_DIR` explicitly gets what it
named, contained or not, **when what it named is absolute**: a relative or empty override falls
through to the same home-relative fallback an absent variable gets. That is the override this rule
leaves alone; `DSH_HOME` has no explicit override.

A `CLAUDE_CONFIG_DIR` read from the environment that is not absolute is dropped with a note naming
it, in the shape the out-of-home notes below already have — the drop is no longer silent.
`APPDATA`, `XDG_CONFIG_HOME` and `DSH_HOME` are unchanged and still drop a relative value in
silence. An explicit override that is not absolute falls through silently in every case, because it
is the caller's own value rather than one this run read from the environment.

And a value `APPDATA`, `XDG_CONFIG_HOME` or `DSH_HOME` holds is used only when it
**resolves under that same resolved home** — `CLAUDE_CONFIG_DIR` is exempt from this check, and a
later paragraph in this section says why. That is not literally `%USERPROFILE%`: `paths::home_dir()` prefers `HOME` over
`USERPROFILE`, on every platform, with no `cfg(windows)` gate, so the comparison is against whatever
this run actually resolved to — one of the four sources listed at the top of this section, not one of
the first two. On the ordinary machine — `HOME` unset, so the resolved home is `%USERPROFILE%`, and
`%APPDATA%` sitting at `%USERPROFILE%\AppData\Roaming` under it — the two agree, the check does not
fire, the value is kept and the path is the one it always was.

It fires wherever they do not agree, and `HOME` being unset is not on its own enough to keep them
together. The case this covers is a home redirected to a sandbox while `APPDATA` still names
the real profile. A home exported to some other absolute path for an unrelated reason, with
`APPDATA` never moved to follow, is the same shape. So is an `%APPDATA%` genuinely relocated by
folder redirection — a roaming profile on a share — where `HOME` is unset and the check fires
anyway, because it is `%APPDATA%` rather than the home that moved. `docs/honesty.md`'s entry on
it enumerates those cells and what the substitution costs each of them. There the home-relative fallback is used instead of the real `%APPDATA%`,
and the substitution is named as a note in the per-adapter report rather than refusing the run. The
note goes to the adapter the substitution moved, not to every line: an adapter resolving under no
root that variable governs has nothing to report, and says nothing. `sync` with no adapter named
reports only the adapters it finds
installed, and it looks for them under the substituted root, so the one adapter the substitution
moved is the one it can stop finding. The note no longer goes with it: `sync` reads the substitution
independently of any target, prints the notes no target already reports beside the run, and carries
them in the `no agent has Estigia installed` refusal when the substitution is why the list came back
empty. What remains is that the adapter itself is still not operated on, because `is_present` reads
the substituted root; `docs/honesty.md`'s entry on it holds that and what the substitution
costs when `HOME` was not, in fact, a sandbox.

Git Bash is **not** that machine, though it is the first candidate that comes to mind: it sets
`HOME=/c/Users/<user>`, and MSYS hands a native child `C:\Users\<user>`, so the resolved home is the
profile and the two agree — the value is kept, the path is the one it always was, and no note
prints.

Before that, an isolated `HOME`/`USERPROFILE` left these two roots resolving against the *real*
machine while every other root moved with the sandbox.

The containment rule above does **not** apply to `CLAUDE_CONFIG_DIR`. It names the whole Claude
Code configuration rather than a root sitting beside the home, and Claude Code reads it wherever it
points, inside the home or not. A value dropped for being outside the home would land the gate
registration under `~/.claude`, exactly where that session does not read it.

## The scope an install is rooted at

An install is machine-wide by default. `setup`, `sync` and `uninstall` each take `--scope` —
`global`, the default, or `workspace` — and, with `workspace`, `--workspace <path>`, the checkout the
install belongs to. The global scope is every root in the section above. The workspace scope writes
the same install into one checkout, so it travels with the repository rather than staying on the
machine that happened to run `setup`.

A workspace install does **not** reuse the global roots moved under the workspace, because each
runtime reads its project-level files somewhere of its own choosing. The destination each adapter
gets, and the runtime's own reference it was read from, are the table below. A path a runtime does not
read reports success and configures nothing — the same rule the global rows answer to.

| Adapter | Skill root | Instruction file | Gate | MCP | Agent definitions |
|---|---|---|---|---|---|
| Agent-neutral | `.agents/skills/` | `AGENTS.md` | — | — | — |
| Claude Code | `.claude/skills/` | `CLAUDE.md` | `.claude/settings.json` | `.mcp.json` | `.claude/agents/` |
| Codex | `.agents/skills/` | `AGENTS.md` | `.codex/hooks.json` | `.codex/config.toml` | — |
| OpenCode | `.agents/skills/` | `AGENTS.md` | `.opencode/plugins/` | `opencode.json` | `.opencode/agents/` |
| Cursor | `.agents/skills/` | `AGENTS.md` | `.cursor/hooks.json` | `.cursor/mcp.json` | — |

Every path is relative to the named workspace. The sources are the runtimes' own references: Claude
Code's `.claude` directory and MCP setup, OpenCode's Agent Skills page, Codex's *Custom instructions
with AGENTS.md* and configuration pages, and Cursor's hooks and rules pages.

**ZCode, DeepSeek Harness and Pi get no workspace install.** Where each reads a project-level file has
not been verified, so `--scope=workspace` refuses them by name rather than writing a path they may not
read. They remain adapted globally.

In a workspace the runtimes that read `AGENTS.md` — Agent-neutral, Codex, OpenCode and Cursor — land
on **one instruction file** and **one skill root**, `.agents/skills`. That is what each runtime reads
in a checkout, so there is one file for the four of them to read and one directory for the directive
in it to name. What that file carries is still **one adapter's** directive: the fenced block names a
skill path and a per-agent override file, `Fence::upsert` replaces the block wholesale, and the
runtime installed last is the one named — the path is the shared root either way, while the override
file the block names belongs to whichever wrote it. The shared directive and the shared skill
therefore go with the **last** recorded adapter, not the first: uninstalling one of several leaves
both for its siblings, and an uninstall of an adapter the record does not name does nothing at all —
as in the global scope, where a per-adapter instruction file makes that automatic. Uninstalling the
last of them is what leaves the checkout clean.
[`honesty.md`](honesty.md) records what that leaves unmeasured.

OpenCode's gate is a plugin rather than a hook file — the `Gate` cell above — and when the
`Loop plugin` setting is on its optional loop plugin lands beside it under the same
`.opencode/plugins/`. Pi's is a sibling file in `~/.pi/agent/extensions/`, described under
[The Pi loop extension](#the-pi-loop-extension) and
[The Pi goal and loop extension](#the-pi-goal-and-loop-extension).

Three names that would do nothing are refused rather than ignored. `--scope=workspace` with no
`--workspace` refuses, because the alternative is writing to the machine's home under a flag that asked
for a checkout. `--workspace` with the default global scope refuses for the same reason in the other
direction: the path was named and nothing would read it. And `--companion <name>` with a workspace
refuses: a companion installs nothing into a checkout, so that path would be read by nothing either.

A global install is recorded too, under `<home>/.estigia/roots/`: the configuration roots it resolved,
dropped when no adapter is left under them. It is what lets an `uninstall` under one Claude profile
see an install under another.

The scope is **recorded** as well as named. A workspace install writes one file under
`<home>/.estigia/workspaces/`, naming the checkout and the adapters installed in it, and drops it when
the last of them leaves. That record is not an adapter file — it sits beside the run pointers, where
Estigia's own machine-side state already is. It is what lets a machine-wide `uninstall` know a checkout
still reads the shared state, which it could not discover from the machine's roots alone, and what
tells a shared instruction file apart from a second install — so uninstalling an adapter that was never
installed leaves a sibling's skill and directive alone, as the global scope does by having an
instruction file per adapter.

An uninstall reads the same scope and workspace it is given, so a workspace install is taken back with
`--scope=workspace --workspace <path>`. A global uninstall does not reach into a checkout, and a
workspace uninstall does not reach into the machine; naming the wrong one removes nothing rather than
removing the other install. The record is keyed by the checkout's own path, so a checkout that was
moved or renamed is one the record no longer names: uninstalling it removes the adapter's own files
and leaves the shared directive and skill where they were. [`honesty.md`](honesty.md) qualifies that
along with what a record that cannot be read does.

## Which adapters have a routable agent-definitions surface

`Model routing` still takes an answer everywhere — it declares models as well as installing the two
role definitions — and the definitions themselves are written only where an adapter has a
sub-agent-definition surface, the rows below. Its `by task` value declares no model and names no
role, so it installs nothing anywhere; it is still a value every adapter's contract records. `Evidence standard` is read by the gate
itself on every adapter, agent-definitions root or none, so it always takes an answer.
`AgentAdapter::agent_definitions_root` names that directory, matched over the instruction-file enum
rather than `adapter.slug`, so an adapter added later has to answer `None` on purpose rather than get
one silently. That sentence is about the adapter only because adapters and instruction variants stand
one-to-one: a test beside the per-adapter matrix in `src/setup/tests.rs` asserts the table's
instruction files are distinct, so a second adapter reusing a variant reddens it, naming both,
rather than borrowing the variant's answer. Each host is written its own dialect — Claude Code's
comma-separated `tools:` line, OpenCode's `permission:` map, Pi's `name`/`description`/`tools`/`model` frontmatter — and the two installed judges remain Claude-only while the two delegated roles are installed for all three.

| Adapter | Agent-definitions root |
|---|---|
| Agent-neutral (`~/.agents`) | none |
| Claude Code | `$CLAUDE_CONFIG_DIR/agents`, falling back to `~/.claude/agents` |
| Codex | none |
| OpenCode | `$XDG_CONFIG_HOME/opencode/agents`, falling back to `~/.config/opencode/agents` |
| Pi | `~/.pi/agent/agents` |
| Gemini CLI (retired, uninstall only) | none |
| Cursor | none |
| Continue (retired, uninstall only) | none |
| Cline (retired, uninstall only) | none |
| Crush (retired, uninstall only) | none |
| Windsurf (retired, uninstall only) | none |
| Qwen (retired, uninstall only) | none |
| ZCode | none |
| DeepSeek Harness | none |

Three of the eight supported adapters have a surface. The eleven rows reading a bare "none" — the
other five supported adapters and all six retired ones — have not been checked
against a schema at all — that ground is simply unclaimed, distinct from OpenCode's, whose surface
was verified from OpenCode's own documentation rather than assumed, and from Pi's, which is read from
the example subagent extension and not from Pi's core (see [`honesty.md`](honesty.md)).

Claude Code's row above is where `setup` writes the reviewer definition, and OpenCode's and Pi's are
where it writes the two delegated workers; `harness::roles::definition_for` is where the gate reads the
reviewer definition back, and until the `$CLAUDE_CONFIG_DIR/agents` root was added it read only the home-relative fallback, never
`$CLAUDE_CONFIG_DIR/agents` itself. Added now, beside the home root rather than instead of it — the
same asymmetry the paragraph above states for `APPDATA` and `XDG_CONFIG_HOME` not applying to this
variable. `docs/honesty.md`'s entry on `CLAUDE_CONFIG_DIR` carries the full account, including
`authorize_review_blind_launch`'s canonical-reviewer proof and the control-surface fragments, which
had the same gap.

## Which runtime word each agent's hook derives

A hook derives the run id `<runtime>-<session prefix><hash>` from the session id in the payload, and
the runtime word comes from the `--agent` that `setup` registered it with
(`session::runtime_of_agent`, over `session::AGENT_RUNTIMES`). It has to be the word the run itself
uses, because the claim pointer is keyed by the whole id: `estigia goal --runtime <word>` and the
skill's `<runtime>-<session-prefix>` rule derive the same string only when the two agree.

| Registered with | Runtime word |
|---|---|
| `--agent codex` | `codex` |
| `--agent claude-code` | `claude` |
| none: Pi's extension calls `estigia gate --runtime pi` itself and registers no hook | `pi` |
| no `--agent` (settings written by an older build) | `claude` |
| every other adapter | `claude`, unchanged — see [`honesty.md`](honesty.md) |

## What setup installs besides the skill, and what the host's permission mode does not change

`setup` installs three core things: the skill an agent reads, a short directive in that agent's
always-loaded instruction file, and the lifecycle hooks that make the authority mechanical rather than
advisory. `--skill-only` stops before the directive. Estigia knows eight agents; the hooks go into
every one with an event able to refuse — five of them, in a gate that reads five dialects, three only for retired hosts.

A host's automatic permission mode approves the host's own prompts. It does not approve Estigia's
decisions: a refusal stays a refusal and a question stays a question under Claude Code's
`bypassPermissions` and `auto`, OpenCode's `--auto` and Codex's `never` approval policy, because the
gate answers before the host's permission layer is asked and nothing in the gate reads the mode.
Tests hold the hook and plugin answers equal with and without the mode, and `estigia doctor` prints the
mode each of Claude Code, Codex, OpenCode and Pi is configured with. What was run live, and what was
not, is in [`honesty.md`](honesty.md).

The setup screen (`estigia tui`) is the one path that needs a person, and it ends by printing the
commands that reproduce the same result without it:

```
the same result without the questions:
  estigia setup claude-code
  estigia config set "Model routing" "developer=inherit" --agent claude-code
```

Whoever does this again — on another machine, in a script, in a container image — needs the spelling
that does not ask questions, and the run that just worked is the only honest place to learn it. Those
commands produce a byte-identical result to the one the questions produced, and a test runs both to
check that they do.

`status` reports two things separately, because *the agent was told about Estigia* and *Estigia can
stop it* are different answers, and somebody looking at a run that wrote without a claim needs the
second one. The tools half can be permanently unavailable: an adapter with no verified place to
register a server prints `tools off` and the reason, where one that is merely not registered yet
prints the command that fixes it:

```
claude-code    configured
               harness: gate on, tools on
codex          configured
               harness: gate off, tools on
```

## The reviewer and role definitions setup installs, and the ones it never does

Every Claude Code setup installs two stable inert reviewer definitions: `review-blind` and
`review-blind-sealed`, one template rendered twice, which are Judgment Day's two judges and nothing
else. The two judge definitions carry `model: inherit` and a paragraph asking a verdict to say what it
rests on. `Evidence standard` decides the unsealed grant — read-only under `reading`, a shell and
nothing else added under `measuring` — the sealed grant is fixed, one tool, a web search, no repository
read. Both are inert unless a launch names the immutable target — a head, a diff or a commit range — and
criteria. The orchestrator passes the effective `judge` model for each Judgment Day launch. Ordinary
review licenses no judge at any tier; Judgment Day, gentle-ai v4's opt-in, is the only place they
exist — see [judgment day](../skill/policies/judgment-day.md). Setup never creates numbered judge files.

The reviewers of an ordinary review are not installed. The sealed lenses `review_tier` licenses —
`review-lens`, `review-risk`, `review-readability`, `review-reliability` and `review-readback` —
the refuter `review-refuter` and the validator `review-validator` are launched by
`launch_review_input` from templates embedded in the binary, with no tools, and it never reads
`~/.claude/agents`. Setup retracts a copy an earlier build installed under one of those seven names: an owned,
unedited one is deleted; an edited one is kept and its path relinquished; one Estigia never recorded
is left alone. The names stay reserved, so a project definition cannot stand in for one, and a hand
`Task` launch of one is refused as `reviewer-launched-by-the-binary` — there is no hand path.

Two further definitions, `developer.md` and `analyst.md`, are installed only where `Model routing`
names their role, and are retracted when the entry goes away, because a definition carrying a tool
grant is authority and an upgrade may not hand out authority nobody asked for. They are installed for
Claude Code and OpenCode, each in its host's own dialect: Claude Code's comma-separated `tools:` line
and `effort:`, OpenCode's `permission:` map and file name, with no `effort:` field there. The routing
is empty by default; naming a role installs it, and the model beside it says only what it runs on. The
row's third value, `by task`, names no role and installs nothing: the shape of the work decides the
model and the effort, and the launching agent records what it chose — a declaration, not a check, as
[`honesty.md`](honesty.md) states. A definition at one of those paths that Estigia did not author is
refused rather than replaced: in the preflight, before anything at all is written, and again
immediately before the bytes go down, which catches one that arrived in between and by then has the
skill and the directive already written behind it. Direct `config set` and `config edit` writes do not
mutate external definitions.

### How a reserved role is proven before it launches

Every reserved role is operator-owned. Before Claude Code's current `Agent` or legacy `Task` launches
any reserved role, Estigia walks from the launch cwd through the first `.git` repository root and
recursively rejects any project `.claude/agents` definition that uses its filename or declares the same
name in valid YAML. For a judge it then proves the canonical user definition is the only user-scoped
definition with that identity and matches one of the renderings Estigia can produce of it — not the
embedded bytes themselves, which the unrendered template would be; for one of the seven
binary-launched names the project walk is the whole proof and the launch is then refused, whatever
sits in the user tree. Setup performs the same recursive user-tree uniqueness preflight for the two
judges before writing, revalidates at the reviewer boundary, and creates a fresh reviewer without
replacing a path another actor won meanwhile. Missing, unreadable, changed, duplicated or ambiguous
evidence refuses. Ordinary agents retain project-first resolution. A refused or unprovable reviewer
does not count, and the fallback is a separate session or durable review handoff, never a smaller or
serialized review.

## Which processes Estigia launches

Estigia does not run your working model. It launches two kinds of process, both detached. A reviewer
CLI, through `launch_review_input`, for a lens, refuter or validator: the first of `claude` then
`codex` found on `PATH`, with no setting to choose between them. And `__check-job`, through
`run_checks`, which runs the commands the base declares as checks and records a receipt for the
published digest; see [`generated-files.md`](generated-files.md). `record_lens_report`,
`record_refutation` and `record_validator_result` are refused by hand as `recorded-by-capture-only`;
only the capture writes them. Both kinds leave their working directory under `~/.estigia/jobs/<id>/`.

## The gate, the hooks and the loop plugins

The contract under `skill/` is text an agent may read and may ignore, which meant "perjury detected
mechanically" was aspirational: nothing stopped a run from writing without a claim, or from
delivering on a verdict bound to a SHA that no longer existed. The harness closes the first of those,
and binds the second without re-checking it at the moment of delivery; [`honesty.md`](honesty.md) says
exactly where that line falls.

```
agent (Claude Code)
  │
  ├─ tool: Edit / Write / Bash
  │    └─ PreToolUse → estigia hook pre-tool-use
  │         └─ verify_claim against the tracker → deny
  │
  ├─ turn about to end
  │    └─ Stop → estigia hook stop
  │         └─ while the run still holds its issue → block, so the turn continues
  │
  └─ tool: mcp__estigia__claim / verify_claim / transition / …
       └─ estigia mcp → answered in this process, against `gh`

git
  └─ pre-push → estigia hook pre-push
       └─ refuses a push no live claim authorises, whoever typed it
```

Two halves, and both are needed. The **tools** are how an agent does workflow work at all: the
operations with schemas, instead of composing `gh` by hand and hoping. The one force-push the delivery
path needs is among them, so it is adjudicated like every other write rather than typed into a shell.
The **gate** is what happens when it tries to work around them.

**The local workflow store, `~/.estigia/state.db`, is the authority**: claims, states, receipts,
findings, verdicts, spend, tasks and `blocked-until` clauses are rows in it, and two runs on one
machine adjudicate against each other in one transaction. The tracker keeps issues and their prose,
pull requests, branches, and labels and an assignee written after the local commit as a projection
that never decides — the price is that the store is one copy on one machine, as
[`honesty.md`](honesty.md) says. A run that dies leaves no *authority* behind: whatever the store says
a checkout is bound by is the whole of what governs it, whether or not the run that wrote it is still
there to be asked. The **local** run pointer a dead session leaves behind is a different question — a
machine-readable note, never authority — and `estigia doctor`'s `stale-run-pointer` row and the
reconciliation against the store that the pre-push hook `estigia guard` installs runs, and that
`estigia gate` reaches equally, exist for exactly the phantom pointer a dead run does leave.
`estigia doctor`'s `prune` row, and the `estigia prune` command behind it, answer the other half a
dead run leaves: the branch and the checkout no live claim owns, listed and removed only when a person
confirms.

### The gate no agent can route around

An agent hook fires only for the agent that installed it. A `pre-push` hook sits **under git**:
`estigia guard` installs it into a repository and `estigia guard --uninstall` removes only a hook
Estigia wrote. It catches the *last* moment rather than the first, and it catches it for every agent,
and for a person. The `PreToolUse` gate is still the one that matters most — it catches a lost claim
race hours before a push — but only one agent has it. Claude Code also registers a `Stop` hook: while
the run still holds its issue the turn continues instead of ending. That hook reads the claim at most
once per renewal window after a successful read (a failed or unknown read is read again), so a long
run of short turns does not spend the GitHub API budget.

OpenCode can do the same, behind the `Loop plugin` setting the TUI edits: with it `on`,
`estigia setup opencode` writes an optional plugin that continues a session while Estigia says the run
is live, another issue is claimable, or a `blocked` item's recorded exit condition already holds — and
it keeps the goal, with the reason it stopped, so a stopped loop is legible and `/flow-status` reports
it; the agent can ask it to end after the current task with `estigia goal stop-after`, which a live
claim ignores and a released one honours. A turn that ends while the run still holds a claim is
continued with one sentence naming the issue and the run, not the whole objective again. A session
that ended its turn only to wait on a running child session is waited on, not re-prompted, and
continued when the child ends ([details](#the-optional-loop-plugin)). Its commands are `/flow-loop` and
`/flow-status`, named so that they never shadow the generic `/goal` and `/loop`. Pi gets it as an owned
extension, `estigia-loop.ts`, that continues the session when Pi reports it settled and registers the
same two commands; its goal lives in memory, so a restarted Pi forgets it. Codex gets the same loop as
a `Stop` and a `SessionStart` hook entry under the same row, each trusted in `config.toml`, without
`--wait`, `--max` or `/flow-status`. Claude Code gets it too: two slash commands under its `commands/`
directory and a `Stop` entry, with the cap and `/flow-status`. The default `off` writes nothing. A
separate `Goal loop plugin` row, also default `off`, writes a generic `goal-loop.js` for OpenCode, or
`estigia-goal-loop.ts` for Pi, that offers `/goal <condition>` and `/loop [interval] <prompt>` with no
Estigia command behind them: a model judges the transcript, on every third turn and on the cap turn,
and [`honesty.md`](honesty.md) says why that is never evidence. `estigia status` reports each agent's
continuation word beside its harness line.

### The oath binds once sworn

A run that has claimed nothing is not under Estigia's authority, except where the repository's
`Unclaimed write policy` says otherwise: by default a repository write by such a run that the gate can
place inside the checkout is put to the person first, a write it cannot place is allowed with a
dirtied checkout reported at the next session start, and with the row `off` every write it makes goes
through untouched. Gating every session would be a lock, not workflow authority.

**One exception, and the operator opts into it.** In a checkout where `estigia guard` installed the
`pre-push` hook, a run holding no claim is refused every commit (`git commit`, `cherry-pick`,
`revert`, `merge`, `rebase`, `am`) and every `git push`, on every branch, with the refusal
`unclaimed-commit`; the `pre-push` hook refuses the same push when git runs it. The way out is to
claim an issue and work through `start_branch`, whose isolated checkout the claim covers. Reads stay
ungated, edits and tests are held only by `Unclaimed write policy`, and a checkout without the guard
has no commit refusal, as does one with no tracker transport or no installed contract.

**The other exception is a repository setting, and it is on by default.** `Unclaimed write policy`
(`ask`, `off` or `required`, answered only by `<repo>/.git/estigia/estigia.local.md`) governs a run
holding no claim in a checkout with a tracker. `required` refuses every repository write it cannot
place outside the checkout, with `issue-required`; `ask`, the default, asks the person on Claude Code
for one it can place inside, refuses it on a host that cannot ask, and allows one it cannot place,
naming the checkout's dirty paths at the next session start; `off` is the behaviour described above.
See [`configuration.md`](configuration.md) for what it covers and [`honesty.md`](honesty.md) for what
it does not.

The moment a run swears — `estigia claim 12` — every repository write it makes is measured against
that claim, and every irreversible boundary re-verifies it against the store. That is where the value
is. Incident I07 in the skill's own ledger is not "an agent wrote without claiming"; it is a run that
**lost a claim race by five seconds, was told 33 seconds later, and worked another 48 minutes**
because nothing in its loop read the claim again. That run had sworn. The gate kills that case.

A routine write, and the Claude Code `Stop` hook, may ride on an answer from inside a two-minute
renewal window — the cadence `SKILL.md` already asks for. **An irreversible boundary never does**:
`git push`, `git merge`, `git pull`, `git tag` when it creates or deletes a tag (listing is a read),
`gh pr merge`, `gh pr create`, `gh release create` and `gh release edit` always re-verify. The gate only ever *reads* the claim, which is what lets it sit on the critical path of
every edit: being wrong costs a refused write, never a damaged issue. A test enforces that
structurally.

## A run id, `release` and `prune`

```sh
estigia claim 12 --run-id claude-0198fe1c --horizon 2026-08-01T18:00Z
estigia gate Edit --run-id claude-0198fe1c --input '{"file_path":"src/x.rs"}'
estigia release --run-id claude-0198fe1c
```

An agent does this through the MCP tools instead; nobody types these often. The run id is minted from
the agent's session and reported by `SessionStart`, so the agent already has it. It is asked for
rather than guessed, because a claim recorded under the wrong run id is a claim the gate will never
match, and being asked beats being silently wrong. `release` also puts down the isolated checkout this
run created — every slice's, not only the last — and the local branch checked out there, once the
tracker recognises the delivery and nothing unpublished remains; what it kept, and why, is named in
its answer. It deletes every remote head the run recorded starting, too, because the branch
`start_branch` linked is not always the branch published and GitHub's own auto-delete removes only the
latter. `estigia prune` clears what no delivery owned: it lists the merged or closed branches and
checkouts, keeps any a live claim holds or that carries commits the remote does not have, and removes
the rest only on `--confirm`. It also finds clean orphan checkouts whose remote branch is gone and
whose commits remain on another remote ref; [the pruning rules](#explicit-orphan-pruning) describe
that additional route and its conservative ownership checks.

## What `~/.estigia/` holds, and what each uninstall step leaves

| Entry | Holds | Taken by `uninstall --all` |
|---|---|---|
| `state.db`, `-wal`, `-shm` | the workflow store: claims, states, receipts, findings, verdicts, spend, tasks, the decision and role history, the session registry and the mailbox | yes, with the last agent, when no workspace or root record still reads it |
| `runs/`, `stop-after/`, stand-down files | run pointers and requests, live run state | yes, with the last agent |
| `workspaces/`, `roots/` | which workspaces and configuration roots hold an install | yes, as the last adapter in each leaves |
| `screen`, `repositories/` | the setup screen's language, the list of checkouts | no: the person's, not any agent's |
| `lifecycle/` | machine-wide binary provenance records, see [`lifecycle.md`](lifecycle.md) | no |
| `jobs/` | one directory per review job and per check job: manifest, prompt, lock, logs, `result.json`, `usage.json` | no |
| `snapshots/` | up to three daily copies of the store, for a person to restore by hand | no |

Whatever the last column says no to stays until a person removes the directory, and
`<checkout>/.git/estigia/` stays until `estigia config forget` runs in that checkout.
