# Configuration

Eighteen typed settings, read from one table. Reading it produces a valid configuration or a refusal
that names what may be written instead — there is no third outcome where a misspelled value is
quietly ignored.

`tests/honesty.rs` crosses this table against the binary in both directions: every row here is a row
the code has, and every value the picker offers is a value named here. A setting that grew a value
and did not grow a row fails the suite.

Nineteen settings, all typed. Reading the table produces a valid configuration or a refusal that
names what may be written instead — never a value guessed at and discovered halfway through a
checkout.

```sh
estigia config list
estigia config set "Merge strategy" squash
```

| Setting | Accepts |
|---|---|
| Delivery authorisation | `auto`, `ask`, or `ask` with a duration such as `ask 30m` |
| Delivery route | `direct` |
| Review delegation | `auto`, `ask`, or `ask` with a duration such as `ask 30m` |
| Transition authorisation | `auto`, `ask`, or `ask` with a duration such as `ask 30m` |
| Merge strategy | `merge commit`, `squash`, or `rebase` |
| Worktree location | `unset`, or an absolute directory — `<repo>`, `<branch>`, `<run-id>` and `<issue>` are substituted, and a template naming neither `<branch>` nor `<run-id>` gains them both in memory, as siblings of the path you configured, so two checkouts do not share a directory |
| Tracker | `github`, `github <owner>/<name>`, `linear`, or `trello` — only `github` has an executable |
| Planning | `direct`, `sdd`, `sdd lite`, `sdd openspec`, or `sdd lite openspec` — `sdd` makes the phases available, and `protocols/sdd.md` engages them per change on ambiguity; `auto` is accepted as a spelling of `sdd` |
| Model routing | `unset`, or comma-separated `key=model` pairs. A key is a delegated role (`implementer`, `reviewer`, `judge`), a workflow state (`analysis`, `ready`, `in-progress`, `review`, `blocked`, `done`), a phase of thinking (`explore`, `propose`, `spec`, `design`, `tasks`, `apply`, `orchestrate`), or a sub-agent somebody's orchestrator spawns (`strategist`, `analyst`, `builder`, `refactorer`, `validator`, `auditor`) — Estigia spawns none of these and does not run models; this is a declaration the agent reads |
| Integration | `branch`, or `trunk` |
| Renewal window | `default`, or a shorter duration such as `30s` or `1m` |
| Review protocol | `standard`, or `receipt-driven` (also accepted as `rdd`) |
| Blind judges | `single`, `two blind`, or `five blind` |
| Evidence standard | `reading`, or `measuring` |
| Change size | a number of lines, such as `800` |
| Irreversible commands | `none`, or commands separated by commas |
| Project board | `none`, or a board as `<owner>/<number>` |
| Summary language | a plain language name |
| Issue body language | a plain language name |

Every one of them, and every one spelled the way `estigia config list` spells it. This table
used to show nine, and one of the nine was `Task body language` — a **legacy alias**, kept so an
older contract still parses, for a row now called `Summary language`. There is a second language
row beside it, `Issue body language`, and the alias's own note says why the two must not be
confused: reading one as the other *would move an operator's answer onto a row that decides
something different, and leave the row it actually decided sitting at the default*. A reader who
set what this table named got exactly that.

`Review delegation` controls permission, not runtime capability. `auto` permits the run to acquire a
distinct reviewer but does not make Estigia spawn one. If none is available, the run records a durable
exact-receipt handoff and releases ownership. `ask <duration>` records one request deadline in that
handoff; no scheduler, sleep, deadline reset, retained claim, or expiry-as-verdict behavior is implied.

`single` remains the default. Every Claude Code setup installs one static read-only definition at
`~/.claude/agents/review-blind.md`, including in single mode. Its file says `model: inherit` and is
inert unless the launch prompt names an active blind mode, exact publication receipt and criteria.
The orchestrator supplies `Model routing`'s effective `judge` assignment when it instantiates that
definition twice or five times concurrently over the identical target and criteria; there are no
numbered definitions. In five-blind mode, three independent confirmations of the same severe finding
are required to block or authorize automatic repair. One or two remain suspicions; dissent, warnings
and suggestions survive, and ambiguous finding identities never aggregate. Estigia records only one
aggregate exact-receipt verdict and cannot prove panel size, concurrency, independence, blindness,
same-finding identity or quorum. `config set` and `config edit` write configuration only; changing
these rows never rewrites that external definition.

The `review-blind` role is reserved to that operator-owned user file. Claude's generated `PreToolUse`
matcher wakes for `Agent` and legacy `Task` launches and, before repository classification, checks an
exact `review-blind` target for recursive project collisions from the launch cwd through the first
`.git` repository root, using parsed YAML frontmatter, then proves the canonical user text is the only
user-scoped definition carrying that identity. Setup performs that same recursive user-tree uniqueness
preflight before writing, repeats it at the external reviewer boundary, and creates a fresh canonical
file without replacing a path another actor created meanwhile. Unreadable or duplicate candidates and
a missing, unreadable or changed canonical copy fail closed. This exception does not change project-
first precedence for ordinary agents and does not add OpenCode launch enforcement. A refused launch
contributes no judge; use a separate session or durable handoff rather than reducing or serializing the
configured panel.

### Which root the gate decides in

An agent reads the contract in **its own** skill root. The gate reads **one** root for the whole
machine — the canonical one — and `estigia config list` without `--agent` reports that same root, so
the two answers only agree while the roots do.

That root is chosen by what each candidate carries, not by which agent is running: a contract with a
configuration block is preferred over one without, and among those, a root holding the operator's own
`estigia.local.md` is preferred over one that does not. Where nothing distinguishes them the declared
adapter order decides, which puts the shared neutral root first.

The second half of that rule exists because `setup --all` writes the configuration block into every
root, so the first half cannot tell two installed roots apart; the selection then fell to the order
and took a root that held none of the operator's values, and the gate adjudicated at defaults while
the agent read the operator's table. Preferring "a configuration that differs from the defaults"
does not separate them either — setup writes real values into the neutral root's block as well.

Two roots can still legitimately hold different tables; `setup` preserves each rather than flattening
them. What must not stay silent is the gate deciding by rows an agent never reads, so `estigia doctor`
carries a `canonical` row naming the agent, the setting and both values.

Which divergence is a **fault** is decided by the scope each row already has — the same three-valued
question the rest of the crate asks, not a fourth list:

- **Agent** — `Delivery authorisation`, `Review delegation`, `Transition authorisation`, `Planning`,
  `Model routing`, `Blind judges`. `Evidence standard` is deliberately **not** among them: the gate
  reads it to render the reserved reviewer's tool grant, and a per-agent answer would be written,
  read back, and never consulted by the one gate that acts on it. These are *meant* to differ by agent; `config set --agent <slug> …`
  is how you set one. Named on the row, and the row stays `ok`: calling a machine broken for a
  supported configuration names a fault with no way out of it.
- **Everywhere** — a fact about the repository, the same whichever agent asks. Two roots answering
  differently means one agent is being decided for by a file it does not read: `BROKEN`, and the
  resolution names `estigia config set "<row>" "<value>"` with no `--agent`, which writes it into
  every installed contract.
- **Machine** — `Summary language` and `Issue body language`, facts about this machine, the same
  whichever repository is open. Also `BROKEN`, and the resolution names the same command as the row
  above: `estigia config set "<row>" "<value>"` with no `--agent` writes it into every installed
  contract. A person writes in one language across every checkout they have, so there is nothing for
  one agent to answer differently — `config set --agent <slug>` refuses a machine row the way it
  refuses a repository one, and says which command holds it instead.

  It did not always. The plain form wrote a machine row into the canonical contract and nowhere
  else, and the per-agent form accepted one it could not hold: the per-agent file is rendered and
  read through the agent scope, so the row was dropped and the command exited on its own read-back,
  blaming a local override that need not exist. Two roots could then answer one machine-wide
  question differently with no command that made them agree, and the `canonical` row reported it
  with no way out to offer. That was [issue #62](https://github.com/asanabrial/estigia/issues/62).

Rows that differ by design are named in both branches, not only in the quiet one.

The row is `skipped`, never `ok`, when the canonical configuration cannot be read: a comparison that
was not made is not agreement.

### Model routing suggestions are agent-specific

The stored value remains exactly the one `key=model` cell above, and the CLI remains the place to edit
that complete route. The TUI does not show a synthetic `Model routing` setting, raw editor, clear-all,
or Advanced stage. `Planning` is the last primary row. A separate block beneath it projects
an adapter-specific model profile, where one is reviewed, followed by `orchestrate`, the active
`Planning::phases()`, universal `apply`, delegated roles
`implementer`/`reviewer`/`judge`, and the six external sub-agent names. Direct planning has no planning
phase rows; full SDD has `explore`, `propose`, `spec`, `design`, `tasks`; lite SDD has only `spec` and
`tasks`. Inactive phases, workflow-state overrides, and persisted hidden assignments remain valid but
are not presented as active choices.

Claude Code and Codex offer three reviewed presets: `balanced`, `performance`, and `economy`.
Choosing one replaces that agent's complete model route; it never merges with stale custom targets.
The profile row reads `custom` whenever the complete route does not exactly match a preset. Choosing
`custom` preserves the current route, and the target rows below are its editor. Profiles expand into
the same persisted `key=model` cell, do not select `Planning`, do not prove model availability, and do
not make Estigia run a model. OpenCode's catalog is dynamic, and adapters without a stable reviewed
catalog offer only custom target editing rather than invented defaults. Shared and uniform views also
offer no profile because they have no single adapter whose model namespace can own it.

Each projected row displays its exact assignment or `inherit` and opens its concrete host's advisory model list directly.
A current custom ID remains in that list, custom input is always available, and
`inherit` removes only that target. Enter and Space both choose a picker entry; Space remains literal inside the custom editor.
Every per-target update uses one destination set: uniform mode means the
currently selected agents, while non-uniform mode means the current agent. Each destination's own
route is staged and unrelated assignments survive. `r` restores only the selected target from each
destination's own installed route. Every destination validates before the staged map commits.

Uniform display aggregates each target independently: one shared model, shared `inherit`, or
`different values`. If selected agents disagree on `Planning`, only the fixed `orchestrate`, `apply`,
and delegated-agent rows remain; the detail asks the operator to unify Planning or edit agents one at
a time rather than borrowing one agent's phase list.

A uniform `Planning` edit uses that same selected-agent set; a loaded but unticked agent is neither
changed nor consulted for its dynamic phase rows. Successful TUI saves acknowledge only effective
selected-agent settings that the installer persisted and read back. `ConfigLayers` retains four
cumulative snapshots instead of flattening provenance: portable contract-only config, that adapter's
own override, the operator's local override, and the repository layer. It also retains the explicit row
ownership of each override document; `ConfigLayers::effective_over` applies only those owned rows over
a new portable candidate. Ownership is scope-filtered before value validation: an agent file parses and
owns only agent-scoped rows, a repository file only repository-scoped rows, and forbidden hand-edited
rows are ignored even when invalid rather than rejecting or pinning the layer. Invalid owned rows still
refuse. One selected adapter owns each shared
skill-root setup action; `SKILL.md` is
rendered once from contract-only portable config, while a selected shared-root agent's Planning/model
changes land only in its own override. Local, repository, and another agent's values are never promoted
into the shared contract, and a private-root agent keeps its answer in its contract without a duplicate
override. Dynamic host artifacts such as Claude SDD definitions use the effective view, so local and
per-agent Planning/model rows materialize them and retract them when those rows stop selecting the
phase. The static reviewer is not configuration output: setup installs it unchanged and launch-time
routing selects its model. A
repository save preserves the rows that document already owns and unions only repository-scoped rows
changed in that session; one explicit row never widens into every repository setting. Direct agent
writes read the override once and use that same snapshot for ownership and writing; only `NotFound`
means a new document, while invalid UTF-8 or any other read failure refuses without changing its bytes.
Direct repository writes follow the same fail-closed rule; `NotFound` creates a layer only for an
explicit repository-setting request, while an existing unreadable document remains byte-identical.

`InstallReceipt` carries contract, agent, local, unlayered, effective and repository read-back plus the
exact settings each effective read proved. One repository document snapshot supplies both its layered
values and exact explicit owned rows across every loaded baseline, so disk movement cannot pair values
from one version with ownership from another. Only those rows advance; inherited repository-capable
rows keep each agent's lower-layer value. A repository write proved before a later failure remains in
the partial receipt and advances on that same row-limited basis. If that proved document is missing or
unreadable at read-back, the result is `Unknown` with status/read-back required and no repository rows
are acknowledged. `SetupResult::completed` and
`InstallReceipt::completed`
separate lifecycle completion from action evidence: read-back of a partly attempted adapter does not
mark it installed, while settings advance only when their effective values were acknowledged. An
in-memory edit on an unticked agent and every unproven scope remain dirty and still cause the quit
warning; later saves compose with earlier Planning or model edits in either order.

An install failure carries that partial receipt. `SetupFailure` carries every action proved before the
failure plus `write_attempted`, which records only a sent write whose result was not proved, and an
explicit prevalidation, preview, or mutation phase. Any generic failure reached before a real mutation
attempt is `NotStarted` with a remedy for the unreadable path or invalid configuration, never write
access or status advice; typed malformed-JSON `NotEditable` refusals retain their specific taxonomy.
A preview failure is always `NotStarted`; planned actions never promote it to `Committed`. A real
invocation whose internal prevalidation fails reports no planned actions, no write attempt, and
`setup stopped before changing any files`; it never renders as a dry-run. A preflight refusal remains
`NotStarted`; after a verified earlier write, a later failure is `Committed` and the
idempotent batch may be retried; an attempt whose own result cannot be confirmed is `Unknown` and
requires status/read-back before retry. Command-line `setup --all` retains that partial evidence,
reports it, and continues with unaffected adapters without counting the failed lifecycle as done. An
unconfirmed adapter keeps the aggregate batch `Unknown` even when unaffected adapters complete. The
first controlling `Unknown` supplies the aggregate code and remedy, so the resolution addresses the
uncertain write rather than an earlier preflight refusal. The TUI
advances only settings in the partial evidence and never reports a nearest state. Interactive
dry-run and the real run share one pending action accumulator across the whole selected batch, including
shared roots, per-agent overrides and the repository file. Dry-run plans are not mutation evidence:
dry-run writes nothing and acknowledges nothing, but reports the same unique paths, change kinds and
count as the corresponding real run. After a failed attempt, a successful retry replaces the pending
refusal before its verified receipt is applied.

The model panel derives one viewport offset for both its rendered rows and the picker anchor. A bottom
target such as `auditor` therefore stays visible with its picker attached above or below it, and a
truncated model panel shows its scrollbar even while the primary settings panel has focus. Rows and
scrollbar use the same focus-aware target: when Tab moves focus to the agent panel both reset to zero;
opening a row picker transfers ownership back before either viewport is derived. Space on the agent
choice remains an ordinary agent toggle rather than opening the selected row.
Help uses the same runtime translation table as every other screen line: display labels are localized,
while persisted setting names, accepted values, model IDs, and printed CLI commands remain canonical.

There is deliberately no universal model list. `AgentAdapter` owns the source for its host:

- Claude Code suggests `fable`, `opus`, `sonnet`, and `haiku`.
- Codex suggests `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`, `gpt-5.4`,
  `gpt-5.4-mini`, `gpt-5.3-codex`, and `gpt-5.2-codex`.
- OpenCode is loaded lazily from `opencode models`, by absolute executable path, with null stdin, and
  never with `--refresh` implicitly. One outer deadline gives PATH resolution, launcher validation,
  spawn and execution five seconds, then 500 ms cleanup grace and 100 ms controller grace return the
  TUI within **5.6 seconds**. Both pipes drain concurrently without being joined by the caller; each
  retains at most `1 MiB`. Successful stdout above that cap, outside strict UTF-8, empty, or containing
  no structurally valid model ID makes the advisory catalog unavailable rather than parsing a prefix.
  A missing, failed, timed-out or rejected script-launcher probe leaves custom model input available.
- Every completed direct process triggers **best-effort descendant cleanup**; every timeout or
  lingering-pipe failure does too. A fresh Unix process group receives `SIGKILL`; a direct Windows `.exe` or
  `.com` is assigned to a kill-on-close Job Object and script launchers are refused. The direct child
  is killed and polled for reaping during failure cleanup, then handed to a background reaper if the OS
  does not report exit in that grace. Job assignment occurs just after Windows spawn, so a child that
  creates a breakaway descendant in that narrow interval is a residual race; group signalling, Job
  termination and reaping are OS operations Estigia attempts and reports, not proof every descendant
  was contained. A world-read that remains blocked past the outer deadline can retain one worker and
  its one-result channel for that call until the OS returns, but the expired deadline is checked before
  that worker may spawn OpenCode.
- Every other adapter, the neutral adapter, and a shared/uniform answer invent no catalog and borrow
  none from the agent under the cursor. Custom input and target-local `inherit` remain available.

These are **advisory suggestions, not validation**. Unknown and future model IDs remain valid and
visible when they fit one persisted entry: no comma, pipe or line break. Dynamic suggestions that do
not fit that same canonical grammar are omitted, and choosing an ID that cannot be assigned does not
close the picker as though it succeeded.
Selecting one does not change `Planning`, choose SDD, prove the host accepts the ID, filter the
model's tool-call capability, or make Estigia run a model.


`ask` proposes and waits. When its timeout expires it **records the proposed transition as a comment
on the issue** rather than applying it — so a run that dies leaves a legible record instead of a
state nobody wrote.

A row can also be set for **one repository**, with `estigia config set … --repo`. It is written into
that repository's git directory rather than into any agent's contract, so it is this clone's answer
and not the team's, and it overrides the contract's row wherever that repository is worked on —
including in a linked worktree, which shares the file with the checkout it was added from. That last
part was a bug: a worktree's `.git` is a file, the path was built by joining onto it, and the same
repository answered `squash` in the checkout and `merge commit` in its own worktree while `--repo`
inside the worktree refused outright. `estigia config repos` lists the repositories that answer for
themselves, and `estigia config forget` takes one repository's rows away again.
