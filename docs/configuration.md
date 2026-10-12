# Configuration

Twenty-two typed settings, read from one table. Reading it produces a valid configuration or a refusal
that names what may be written instead — there is no third outcome where a misspelled value is
quietly ignored.

This is one document in [the reference set](README.md); [`honesty.md`](honesty.md) says what its rows
cannot check, and [`review-cost.md`](review-cost.md) measures the one row that only reports.

`tests/honesty.rs` crosses this table against the binary in both directions: every row here is a row
the code has, and every value the picker offers is a value named here. A setting that grew a value
and did not grow a row fails the suite.

Twenty-two settings, all typed. Reading the table produces a valid configuration or a refusal that
names what may be written instead — never a value guessed at and discovered halfway through a
checkout.

```sh
estigia config list
estigia config set "Integration mode" trunk
```

| Setting | Accepts | Group | Scope |
|---|---|---|---|
| Authorisation posture | `balanced` (both authorisations `auto`), `cautious` (both authorisations `ask`), or `custom` when the two are a mix. Default `balanced`. Naming a profile writes the two rows below it that can ask, `Delivery authorisation` and `Transition authorisation`, and stores nothing of its own, so an explicit one written after it wins | Authority | agent |
| Delivery authorisation | `auto`, `ask`, or `ask` with a duration such as `ask 30m` | Authority | agent |
| Transition authorisation | `auto`, `ask`, or `ask` with a duration such as `ask 30m` | Authority | agent |
| Tracker | `github`, `github <owner>/<name>`, or `linear <team-key>` — where issues live and claims are adjudicated. `linear` is **read-only** and admitted only under `Claim workflow: assisted`; under autonomous work it is refused (issue #847), and it is set only in a repository's own file (`config set Tracker "linear <team-key>" --repo`): machine-wide and per-agent forms are refused (`linear-is-per-repository`), as is `config set Workflow autonomous --repo` while the repository's `Tracker` is `linear` (`autonomous-needs-github-tracker`). `estigia roles --issue` takes a number or a key. And `assist` refuses an issue whose shape does not match the tracker (`assist-issue-does-not-match-the-tracker`) | Repository & tracker | everywhere |
| Repository host | `github`, or `github <owner>/<name>` — where the code lives and publications are verified (pull requests, tags, receipt-bound review). Defaults to GitHub matching the tracker when that tracker is GitHub, so an installation that only sets `Tracker` keeps one place to configure (issue #180) | Repository & tracker | everywhere |
| Claim workflow | `autonomous`, or `assisted`. Default `autonomous`. **Answered only by a repository's own file**, exactly as `Unclaimed write policy` is: `config set` refuses it without `--repo`, and a row for it in the contract is read by nobody. It decides who witnesses a claim in this repository. `autonomous` is today's behaviour. `assisted` means the team coordinates who takes an issue and Estigia writes nothing to the tracker: every tool that writes the tracker or the repository host (`claim`, `reclaim`, `release`, `heartbeat`, `transition`, `comment`, `create`, `ensure_states`, `handoff_review`, the `record_*` tools, `publish_review`, `republish_review`, `release_ci` and `start_branch`) is refused with `workflow-assisted-writes-no-tracker`, naming the row, the value and `estigia config set Workflow autonomous --repo`; reads and `changelog_notes` stay allowed. The two tools offered only there are `assist` and `draft_pr`, and both are refused with `workflow-not-assisted` anywhere else. `assist` reads the issue, makes the isolated checkout and records a local claim confined to that worktree, writing nothing to the tracker. `draft_pr` publishes the branch that pointer names as a draft pull request — a fast-forward push, the title and body as given, then a read-back — and writes nothing to the tracker either: no marker, no receipt, no `Refs` line. It refuses a closing keyword in the title, the body or the commits, and a branch whose open pull request is already ready for review. `estigia goal` — what the loop plugins ask — answers no claimable and no dischargeable work there, so a loop stops instead of spinning on issues nobody will claim; its human line says the queues were not read, rather than printing the zeros. A contract the gate could not read gives the gate no `Claim workflow` at all: `assist` and an assisted pointer fail closed on that, and the tracker writers are not refused on it, because one malformed unrelated row would otherwise stop every tracker write in every repository (`honesty.md` says what that leaves open). What `assisted` does not do yet is in [`honesty.md`](honesty.md) | Repository & tracker | repository |
| Unclaimed write policy | `ask`, `off`, or `required`. Default `ask`. **Answered only by a repository's own file** (`<repo>/.git/estigia/estigia.local.md`) — there is no machine-wide or per-agent value, so `config set` refuses it without `--repo` and a row for it in the contract is read by nobody. It governs a run that holds **no claim**, in a checkout with a tracker and an installed contract. `required` refuses its repository writes (a file edit or write, and the `git commit`, branch and worktree fragments the gate already classifies as writes) with `issue-required`, naming `claim` and `create`. `ask` puts a repository write it can place inside the checkout to the person where the host can ask — Claude Code's `PreToolUse` `ask`, and Pi's `ctx.ui.confirm` in its terminal UI — and refuses it with the same code everywhere else; a write it **cannot place** inside or outside the checkout without running it — inline interpreter code, a utility naming its destination as an argument, a redirect it cannot resolve — is allowed rather than asked for, and a checkout it may have dirtied is named at the next session start (issue #719). `off` is today's behaviour. A checkout with no installed contract, reads, writes outside the checkout, and a run that holds a claim are unaffected under every value | Repository & tracker | repository |
| Integration mode | `branch`, or `trunk` | Repository & tracker | everywhere |
| Worktree location | `unset`, or an absolute directory — `<repo>`, `<branch>`, `<run-id>` and `<issue>` are substituted, and a template naming neither `<branch>` nor `<run-id>` gains them both in memory, as siblings of the path you configured, so two checkouts do not share a directory | Repository & tracker | everywhere |
| Irreversible commands | `none`, or commands separated by commas — the options page suggests `git push --force`, `gh release create`, `gh repo delete` and `npm publish`, one at a time; a longer list is typed | Gate limits | everywhere |
| Renewal window | `default`, or a shorter duration such as `30s` or `1m` | Gate limits | everywhere |
| Git operation window | `disabled`, or `enabled [days] HH:MM-HH:MM` with an optional ` inverted`, such as `enabled 09:00-17:00`, `enabled 22:00-06:00 inverted`, `enabled mon-fri 08:00-17:00 inverted` or `enabled sat,sun 10:00-14:00` — one daily range in this machine's local time, optionally on a set of weekdays (`mon` to `sun`, ranges such as `mon-fri`, commas, case-insensitive; a range may wrap the week, as `fri-mon` does; a repeated or empty day is invalid); no day set means every day; a start later than the end crosses midnight and the part after midnight belongs to the day the range started on; equal endpoints are invalid; half-open `[start, end)`; `inverted` permits exactly the rest of the week | Gate limits | everywhere |
| Evidence standard | `reading`, or `measuring` | Review & change | everywhere |
| Publication lane | `on` or `off`. Default `on`: accepted verdicts are gated on the head's check runs. `off` skips the check-run read and records `classification: not-consulted` (issue #192). A `cancelled` conclusion still refuses when the gate is on. | Review & change | everywhere |
| Review budget | `none`, or a token count such as `150000`. Default `150000`, per review context — an inherited comparison target, not a number derived from the distribution below. Twenty-eight judge contexts measured across five issues, when ordinary review still acquired judges (issue #994 ended that), read from harness completion reports, bracket it rather than produce it: minimum 96,073, median 127,025, maximum 182,136 tokens, with 18% over 150,000. **Advisory, and reported rather than enforced**: nothing here refuses a panel that runs over it, and `record_spend` is recorded, not verified, so an under-report is undetectable. `none` switches it off. `review_tier`'s `forecast` line multiplies it by the lenses the tier licenses and states the product; the forecast is a statement, not a limit. See [`docs/honesty.md`](honesty.md) for what the number cannot count | Review & change | everywhere |
| Change size | a number of lines, such as `800` | Review & change | everywhere |
| Corrections per lineage | a positive whole number. Default `3`: a review lineage — an original candidate and its repairs on one pull request — may publish that many corrections after a rejection. A publication past it is refused as `correction-already-spent` before anything is pushed, `review_status` and `repair_rounds` report it as `correction_limit`, and a rejected correction with corrections remaining, one the targeted validator failed included, is due another rather than escalated; once the limit is spent, `escalate_review` is the exit and a person's `recover` continues. `1` is the limit gentle-ai v4 enforces (see [`docs/parity.md`](parity.md)). The size budget and the reviewed-path scope are unchanged, and the budget stays cumulative across corrections. `0`, a negative number and a non-integer are refused | Review & change | everywhere |
| Model routing | `unset`, `by task`, or one cell holding two lists separated by `;`. The first is comma-separated `key=model` pairs, as in `orchestrate=fable, apply=sonnet`; the second, after the `;`, is comma-separated `key=effort` pairs, as in `apply=low`. `by task` is the whole cell and names no route: the shape of the work decides the model and the effort, and the launching agent records what it chose (see [`docs/honesty.md`](honesty.md)). A cell mixing `by task` with pairs is refused rather than merged. A key is a role (`developer`, `analyst`, `reviewer`, `judge`), `apply`, or `orchestrate`. The retired phase keys — `explore`, `propose`, `spec`, `design`, `tasks` — are dropped by the tolerant read and named in a report, and the strict read refuses the cell. A model ID may use any catalog spelling, or `inherit`, but no comma, semicolon, pipe, or line break. An effort is `low`, `medium`, `high`, `xhigh` or `max`. Naming a role is what installs its definition and removing the entry retracts it, where the host reads sub-agent definitions (Claude Code, OpenCode, and Pi's `~/.pi/agent/agents/`); `judge` installs nothing and is a declaration read only by the orchestrator, under Judgment Day, when it launches a judge; `reviewer` installs nothing and names the model `launch_review_input` runs every sealed reviewer on (`inherit`, or no entry, leaves the CLI's own choice; the transport reads the layered table and not an `estigia.<agent>.md` file, which `docs/honesty.md` records), and its effort half is read by nothing. A role named with no model of its own runs on the inherited model at the effort beside it. The rest is a declaration the agent reads: the one model Estigia itself launches is the `reviewer` run `launch_review_input` starts | Agent runtime | agent |
| Loop plugin | `on` or `off`. Default `off`: `setup` writes `flow-loop.js` beside the gate plugin only when the row is `on`, and takes back one Estigia wrote when it is `off`. Its commands are `/flow-loop` and `/flow-status`, so it never shadows the generic `/goal` and `/loop` (issue #639). `/flow-loop` with no text runs `/flow dev`, and text whose first word is not `/flow` or `flow` is prefixed with `/flow`, which routes itself (issues #642, #646). A leading role word (`dev`, `triage`, `analyst`, `developer`) picks that role instead of routing, and working one issue needs `dev <n>`, because a bare number is read as conditions or domain rules. The plugin continues a session while `estigia goal` says the run is live, or — its own issue done and released — while another issue is claimable or a `blocked` item's recorded exit condition already holds. `--max <n>` caps the loop at `n` issues that reached `done` under the run, counted from the tracker rather than turns. Codex gets the same loop as a `Stop` and a `SessionStart` entry in `hooks.json`, each trusted in `config.toml`, when the row is `on` for Codex, and takes them back when it is `off`; the continuation is `$flow dev`, and `--wait`, `--max`, per-session text and `/flow-status` are not ported (issue #637, `docs/what-it-writes.md`). Claude Code gets the same loop as two slash commands (`flow-loop.md` and `flow-status.md` under its `commands/` directory) and a `Stop` entry naming `--agent claude-code`, when the row is `on` for it; the continuation is `/flow dev`, the cap is counted the same way, and `/flow-loop stop` records the portable `stop-after` request (issue #891, `docs/what-it-writes.md`). Pi gets the same loop as one owned extension, `estigia-loop.ts` beside its gate extension, which registers `/flow-loop` and `/flow-status` itself and continues the session on Pi's `agent_settled` event; its goal is held in memory, so a restarted Pi forgets it (issue #1178, `docs/what-it-writes.md`). An agent's page on the setup screen offers the row for OpenCode, Pi, Codex and Claude Code and no other agent (issue #600) | Agent runtime | agent |
| Goal loop plugin | `on` or `off`. Default `off`: `setup` writes `goal-loop.js` beside the gate plugin only when the row is `on`, and takes back one Estigia wrote when it is `off`. It offers the generic `/goal <condition>` and `/loop [interval] <prompt>` and is independent of `Loop plugin`: the file names no Estigia path and runs no Estigia command, and a `/goal` completes when a model judges the transcript, which is never Estigia evidence. Pi gets the same pair as one owned extension, `estigia-goal-loop.ts` beside its gate extension, which judges through Pi's model registry and keeps its state in memory (issue #1178, `docs/what-it-writes.md`). Only OpenCode and Pi read a plugin or extension Estigia writes, so the row is offered for those two and no other agent (issues #639, #1178) | Agent runtime | agent |
| Summary language | a plain language name | This machine | machine |
| Issue body language | a plain language name | This machine | machine |

Every one of them, and every one spelled the way `estigia config list` spells it. This table
used to show nine, and one of the nine was `Task body language` — a **legacy alias**, kept so an
older contract still parses, for a row now called `Summary language`. There is a second language
row beside it, `Issue body language`, and the alias's own note says why the two must not be
confused: reading one as the other *would move an operator's answer onto a row that decides
something different, and leave the row it actually decided sitting at the default*. A reader who
set what this table named got exactly that.

`Repair ceiling` is retired and is not a row any more: it was instruction rather than enforcement. The
count of corrections a lineage may spend is `Corrections per lineage`, enforced at publication as
`correction-already-spent`. An installed row
still loads, changes nothing, and is reported as `repair-ceiling-retired` in the shared table, an agent
file, `estigia.local.md` and a repository's file alike; `config set` refuses it, and `setup` leaves it
out of the shared table it rewrites. Four more rows retired in issue #1044 because each had exactly
one answer, which is now hard-coded: `Delivery route` was `direct`, so delivery is direct; `Merge
strategy` was `merge commit`, so delivery merges with a merge commit; `Review protocol` was
`receipt-driven`, so every verdict is bound to a digest over the complete intended target and
`protocols/rdd.md` installs with the contract; and `Review delegation` was `auto`, so a run acquires
its reviewers without asking. Each installed row still loads and changes nothing, and `config set`
refuses a new write with `delivery-route-retired`, `merge-strategy-retired`,
`review-protocol-retired` or `review-delegation-retired`.

Review permission is no longer a row: a run acquires a distinct reviewer unconditionally. The
binary still launches a lens, refuter or validator only when the run calls `launch_review_input`,
and nobody is asked first: before launching a lens the run says the one-line `forecast` that
`review_tier` answers — the tier, the lens count and the expected spend, the `Review budget` figure
times the lenses the tier licenses. If no reviewer is available, the run records a durable
exact-receipt handoff and releases ownership.

`Git operation window` denies `git commit`, every ordinary `git push`, plain and porcelain
`git branch` creation — the copy spelling included — `git checkout -b/-B/--orphan`,
`git switch -c/-C/--create/--force-create/--orphan`, and `git worktree add -b/-B/--orphan`
outside the permitted period — including the internal push `publish_review` performs, and the branch
creation `start_branch` would perform, which is not denied: it makes the local branch and the
worktree, and queues the native link of the branch to its issue for when the window opens, answering
`natively_linked: false` and `link_outcome: deferred`. Ordinary pushes are also evaluated in the pre-push
guard whether or not the checkout is claimed. The schedule never grants: inside the period every
claim, state, review, and irreversible-boundary check still applies. Worktree creation on an existing
branch, file edits, tests, and builds are unaffected. An enabled value that does not parse, or a
clock that cannot be read, denies the protected operations rather than falling back to unrestricted.
The window is the base period "the day is in the set and the minute is in the range", read in the
process's local time zone; a range that crosses midnight belongs to the day it starts on, so
`fri 22:00-06:00` covers Friday 22:00 to Saturday 06:00 and not Saturday 22:00. `inverted` is the exact
complement over the whole week: `enabled mon-fri 08:00-17:00 inverted` permits every weekend minute and
every weekday evening and early morning. Daylight saving time is evaluated on the wall clock: the
local weekday and minute are read as the machine reports them, so on a changeover day the range
covers whichever real minutes carry those wall-clock labels, and an hour that is skipped or repeated
is skipped or repeated here too.
What the schedule cannot stop — `git push --no-verify`, direct ref writes, hosting APIs, another
uninstrumented clone — is stated in [`docs/honesty.md`](honesty.md), not here.

Every transport operation reads the window once, where it starts, and decides everything after it
from that one answer. While the answer is closed, a `gh` call that writes — one declared as a write,
`issue`/`pr` create, edit, comment, close, ready or merge, `issue develop`, `label create`, a
`graphql` mutation, a REST call with a non-`GET` method — and every `git push` the transport itself
spawns is refused with `outbox-bypass` before any process starts. Reads are never refused.

The operations that write to GitHub keep working while the answer is closed, against the local
store, and queue the write they would have made: this is the offline mode. `transition` records the state and queues one convergence of the
labels and the board per issue, answering `"projection":"deferred"` and `"queued":<seq>`.
`comment` queues the escaped body and answers `"queued":<seq>`. `create` records the issue under a
provisional number — far above any number GitHub issues — queues the filing and one convergence, and
answers `"provisional":true` with `"url":null`. `claim`, `claim --takeover` and `unassign` record
the ownership in the store as they always do, queue one convergence of the assignee and the `dev:`
label, and answer `"projection":"deferred"`; an `unassign` that carries `--evidence-file` queues the
evidence as a comment. A provisional issue is claimed, given tasks and heartbeats, verified and
released without a call to GitHub, which has no such issue. `list_state` still reads GitHub's open
list, takes every state from the store, and appends the provisional issues in the state asked for,
marked `provisional`, so a run sees the work it filed itself; `list_state --identities` includes
their identity. Nothing leaves the machine until the queue is sent.

`estigia flush` sends it, oldest first, once the window is open, and stops at the first write it cannot
read back; `estigia flush --status` lists the queue and sends nothing, and `estigia flush --detach`
starts the flush in the background and returns. While the window is open and writes are queued, every
operation that writes to GitHub and every publication runs the flush first and is refused
`outbox-pending` when the queue does not drain. A publication refused because the window is closed says
how many writes are queued and that `estigia flush` runs when the window opens. The same operation is the
`flush` tool, with `read_only` for the listing.

The gate holds the agent's own hand to the same window. While it is closed, an agent's shell line
that merges, tags, creates or readies a pull request, changes a release, or creates, comments on,
edits or develops an issue, creates a label, or calls `gh api` with a method other than `GET` or a
`graphql` mutation is denied with `git-operation-window-denied`, as a commit is; `gh issue view`,
`gh issue list`, `gh pr view` and a `GET` call stay allowed. `release_ci` is refused the same way. A
session start reports the queue without asking GitHub: with the window closed, how many writes wait and
when they can go; with it open, that `estigia flush` was started in the background. The `Stop` hook
says it too and never keeps the session from stopping.

Ordinary review licenses lenses only — none at `low`, one at `medium`, four at `high` — and
[blind judges](../skill/policies/blind-judges.md) states it. Judges exist only as Judgment Day, gentle-ai v4's opt-in
([judgment day](../skill/policies/judgment-day.md)). Every Claude Code setup
installs two inert definitions: `~/.claude/agents/review-blind.md` and
`~/.claude/agents/review-blind-sealed.md`, rendered from one template with a different `Seal`. They are
Judgment Day's judges, and this composition never launches either. The other seven reserved
names — the five sealed lenses `review-lens`, `review-risk`, `review-readability`, `review-reliability`
and `review-readback`, the refuter `review-refuter` and the validator `review-validator` — are not
installed: `launch_review_input` launches each from a template embedded in the binary with no tools and
never reads `~/.claude/agents`, so there is no hand path for them and setup retracts an owned,
unedited copy an earlier build wrote (an edited copy is kept, an unowned one left alone).
The two judge files say `model:
inherit` and are inert unless
the launch prompt names the immutable target — a head, a diff or a commit range — and criteria. `review-blind`'s tool grant is
`Evidence standard`'s answer; `review-blind-sealed`'s is fixed to one tool, a web search, and
`skill/policies/judgment-day.md` states what that seal does and does not buy. The orchestrator
supplies `Model routing`'s
effective `judge` assignment only when it launches Judgment Day, which is never a tier's allocation.
A lens's severe finding is settled by the ledger at every tier, in this order: a missing `evidence_class` or `causal_disposition`, `insufficient` evidence or an `unknown` cause escalates; a `pre-existing` or `base-only` cause is a follow-up that blocks nothing; `deterministic` is open and rejects; `inferential` follows its refuter (`refuted` drops it, `corroborated` or `inconclusive` keep it, no result escalates). Dissent,
warnings and suggestions survive. Estigia records only one computed exact-receipt verdict and
cannot prove lens count, concurrency, independence, blindness or same-finding identity. `config set` and `config edit` write configuration only; changing
these rows never rewrites either external definition — including `Evidence standard`, which decides
`review-blind`'s tool grant and, under `measuring`, its basis paragraph. **The gate takes the new row at once; the file on disk keeps the old grant until `estigia setup` or
`estigia sync` next writes it**, so until then the host offers the previous grant while the gate
enforces the new one. The narrowing direction fails closed at the gate; the widening direction is refused only by the host’s stale `tools:` line, if it enforces one. Nothing reports either, and
`docs/honesty.md` records it as open.

Every reserved role holds its own operator-owned user
file. Claude's generated `PreToolUse` matcher wakes for `Agent` and legacy `Task` launches and, before
repository classification, checks the launched name's exact target for recursive project collisions
from the launch cwd through the first
`.git` repository root, using parsed YAML frontmatter, then proves the canonical user text is the only
user-scoped definition carrying that identity. Setup performs that same recursive user-tree uniqueness
preflight before writing, repeats it at the external reviewer boundary, and creates a fresh canonical
file without replacing a path another actor created meanwhile. Unreadable or duplicate candidates and
a missing, unreadable or changed canonical copy fail closed. This exception does not change project-
first precedence for ordinary agents and does not add OpenCode launch enforcement. A refused launch
contributes no reviewer; use a separate session or durable handoff rather than reducing or serializing the
configured review.

### What the listing says about where a value came from

`estigia config list` names, on every row, the scope its winning value came from — `repository`,
`user local`, `agent file`, `versioned block`, or `skill default`. The order is the order the layers
are applied in: a repository's own file, then the operator's `estigia.local.md`, then the adapter's
own file, then the versioned block, and the shipped default underneath them all. `skill default`
means no document carries the row at all — `Unclaimed write policy` is the ordinary one, because the
contract has no value for it and only a repository's own file may answer it.

When a nearer scope shadows a farther one, the listing prints the shadowed value and its scope
beside the winner rather than the winner alone, so a reader comparing the listing against their own
file sees the disagreement instead of inferring it. A row this checkout answers differently also
prints the sentence `config set` writes — *this checkout answers … instead — it says so in its own
file, and `estigia config set --repo` is what changes that* — which is one sentence in one place,
shared by the two commands rather than written twice. `--json` carries the same facts as `source`
and, only where a row is shadowed, `shadowed`: a list of `{value, source}` pairs.

The listing is a read. Precedence is unchanged, and `config set` without `--repo` still refuses to
edit the operator's own file.

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

Which divergence is a **fault** is decided by the scope each row already has — the same question the
rest of the crate asks, not another list:

- **Agent** — `Authorisation posture`, `Delivery authorisation`, `Transition authorisation`,
  `Model routing`, `Loop plugin`, `Goal loop plugin`. These are *meant* to differ by agent;
  `config set --agent <slug> …` is how you set one; `config set` and `config edit` without
  `--agent` refuse these rows with `setting-needs-an-agent`, naming `--agent <slug>` and the
  installed agents that read the row, and write nothing (a bare set used to land in whichever
  contract discovery picked, which every agent without a file of its own then read — issue #1213).
  `--contract`, valid only with `--agent`, writes that agent's contract table instead of its own
  file. On a shared root an agent-scoped row is refused with `contract-is-shared` and nothing is
  written, because that `SKILL.md` is read by every agent there without a file of its own; the refusal
  names the same set without `--contract`, which writes `estigia.<slug>.md`, and so does the command
  `estigia doctor` names for an agent that inherits such a row (issue #1233). An agent with a skill
  directory of its own (Claude Code, Codex, DeepSeek Harness) owns its `SKILL.md`, and `--contract` still
  writes it. `config set` names the file it wrote on every
  branch. `config set`, `config edit` and `config list`
  refuse a retired slug with `agent-retired`, naming `estigia uninstall <slug>`. On a shared skill root that answer lands in
  `estigia.<slug>.md` beside the contract, and the paragraph above the contract's own table names
  that file and the order — `estigia.local.md`, then `estigia.<slug>.md`, then the table — because
  the table is where an agent decides, and a pointer only in the agent's instruction file left an
  OpenCode orchestrator applying Claude's `judge=opus` (issue #656). Two of them — the authorisations — also have
  a repository half that can only **tighten**, below; that does not move their scope, so a
  repository tightening one is never a divergence here. Named on the row, and the row stays `ok`:
  calling a machine broken for a supported configuration names a fault with no way out of it.
  `Evidence standard` is deliberately **not** among them: the gate reads it to render the reserved
  reviewer's tool grant and the paragraph asking a verdict for its basis, and a
  per-agent answer would be written, read back, and never consulted by the one gate that acts on it.
- **Everywhere** — a fact about the repository, the same whichever agent asks, including `Evidence standard`. Two roots answering
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
  with no way out to offer. That was measured and fixed.

- **Repository** — `Unclaimed write policy` and `Claim workflow`, each answered by a repository's own file and by
  nothing else. The contract carries no row for either, so two roots cannot diverge on them and
  `doctor` has nothing to compare. `Scope::Repository` is a fourth value rather than an `Everywhere` row because an
  `Everywhere` row has a machine-wide value a repository overrides, and this one is meant to have
  none: an `off` could then never be a repository silently loosening what the operator required. If a
  machine-wide value is ever added, a repository file may only tighten it, like the two
  authorisations. The options page offers them under the repository and writes them to that file,
  creating it when the checkout has none.

  `Claim workflow` is not an exception to the tighten-only rule, and the argument is worth having in one
  place. That rule governs the two authorisations, and `assisted` leaves those rows as they are
  while refusing every tool through which Estigia acts on them — `transition`, the review-recording
  tools, `handoff_review` and `release_ci` — so on those axes a repository file choosing it can only
  remove what the operator allowed, never add to it. What `assisted` changes is a different axis — who witnesses the
  claim, Estigia or the team — and that is the axis `Unclaimed write policy` already holds at repository
  scope, for the same reason: it is a fact about how this repository is worked, and no machine-wide
  answer to it exists to be tightened. The mode is read only from a contract the gate could read; an
  unreadable one gives the gate no `Claim workflow` at all, which no assisted path reads as `assisted`.

Nine rows reach `GateContext`, seven of them **Everywhere**: `Tracker`, `Repository host`, `Integration mode`,
`Renewal window`, `Git operation window`, `Evidence standard` and `Irreversible commands`; the eighth and
ninth are `Unclaimed write policy` and `Claim workflow`. Eight of those feed the gate's
own allow/deny decision — `Claim workflow` only for the pointer an `assisted` run left; `Repository host` is carried so MCP publication tools can bind to it
(issue #180) rather than so `decide` branches on it, and `Claim workflow` is also read where the tool server
runs a tool and where `estigia goal` counts work. Every other repository-scoped row, `Repair
ceiling` included, is read by an agent or by the transport and never by the gate's own decision.

That split used to decide how a write to `<repo>/.git/estigia/estigia.local.md` — the file `--repo`
writes into, below — was adjudicated: which row a write named. It no longer does on the write-tool
road. **Every write to this file through a write tool is a boundary, whichever row it names, and a
shell write to it gets the same answer a shell write to any other control-surface file gets** — and
that holds even when `REPOSITORY_SHELL` matches first: `git commit -m wip && printf x >>
.git/estigia/estigia.local.md` answers `Boundary`, because `REPOSITORY_SHELL`'s own arm now asks the
same control-surface and repository-configuration question `shell::writes_a_file`'s arm already asked,
before it answers. That shadow was issue #85; it is closed.
Six attempts tried
to gate the row instead of the file, each one closing the escape a blind panel found in the one
before it, and the last two escapes were the same shape as each other: a payload that named no
guarded row at all could still move where `crate::config::rows` reads the boundary between one row
and the next — a deleted line break merging two rows into one, or a fence marker assembled from
fragments that were each, read alone, an ordinary line. `crate::config::rows` reads a **region**,
not a payload, and an anchored write can move that region's boundary with text that names no row —
which no per-payload reading can see, because it is a property of the *document* an edit produces,
not of the edit itself. `docs/honesty.md` carries all six attempts, the payload each one died on, and
why the seventh answer is to stop asking the question at all rather than to ask it a seventh way.

The control surface is still asked **first**, unconditionally, so a path that happens to be on both
(`~/.estigia/estigia.local.md`, which matches the control surface's `.estigia/` fragment and this
file's own name both) answers through that branch rather than this one — both answer `Boundary`
today, so the ordering no longer changes the result, but it is still the branch that is asked.

A shell write to this file was never offered a per-row reading either — a shell line has no named
field a row can be read out of, only text that was typed — so `printf …
> .git/estigia/estigia.local.md` already got the same answer a shell write to any other
control-surface file gets. What changed is that the write-tool road now agrees with it on every
payload it is asked about, including a row the gate never consults, rather than only on a guarded row and the
fail-closed default. A shell line `REPOSITORY_SHELL` matches first used to be decided before either
question was asked at all — issue #85 — so the two roads diverged there; `REPOSITORY_SHELL`'s own arm
now asks the same question before it answers, and that shape is closed.

The cost, stated plainly rather than argued away: a row nothing in the gate reads to decide what it
enforces — `Review budget` is one — and an edit that touches only that row now costs one tracker
read on every write, the same as `Tracker` or `Evidence standard` does. That is not what acceptance
criterion 3 asked for, read strictly, and this document says so rather than qualifying the criterion
until it appears to be met. `estigia config set --repo` writes this same file directly, without
going through a gated tool, and is unaffected by any of this.

Rows that differ by design are named in both branches, not only in the quiet one.

The row is `skipped`, never `ok`, when the canonical configuration cannot be read: a comparison that
was not made is not agreement.

### How the screen is navigated

One level per screen, and one list per level; there is no focus to move between panes.

- **Menu** (`Agents`, `Options`, `Push guard`, `Doctor`, `Help`, `Quit`). A `•` beside `Agents` or
  `Options` says that page carries an answer nobody has installed.
  Beside it the screen draws Tantan, the kitten from the README cover, when the body is at least the
  drawing's width plus the menu's (105 columns in practice) and leaves the drawing four spare rows
  between the header and the footer, split around it, so at least two blank rows sit above the footer
  (43 rows in practice with the two-line footer, one more for each line the footer gains). Smaller
  than that it falls back to the wordmark, and when that does not fit either, to the menu alone.
- **Agents.** The first row is **All chosen agents**; below it is every adapter with its state
  (`gated` or `contract only`, `installed`, `will be installed`). Space ticks or unticks the adapter under
  the cursor, installed or not; unticking an installed one writes nothing, and `u` is what removes it. Enter opens the adapter under the cursor, or all of them,
  and refuses an adapter that is not ticked rather than answering into a table this run will not write.
- **An agent's page** is that adapter's `Agent`-scope rows, and the model rows beneath the settings. On
  **All chosen agents** the same page writes each answer to every ticked adapter, which is what the
  retired "every agent" stop of the wizard did; a row only some of them act on is not offered there, and
  the page reads `different values` where they disagree.
- **Options** is the repository's own rows and, above them, the two preferences of the screen.

| Key | Does |
|---|---|
| ↑↓ or `j` `k` | move the cursor |
| ⏎ | open the agent under the cursor; on a row, open its list of answers, as Space does |
| ←→ or `h` `l` | change the highlighted row, on the row |
| Space | tick an agent; on a row, open its list of answers |
| `u` | on an installed agent, or on `Push guard` in the menu: list what the CLI uninstall would remove; ⏎ removes it and reads the state back, Esc removes nothing. The footer offers it only there |
| `r` | put the highlighted row back to what is installed |
| `s` | lay what a save would write over the screen, from any page; ⏎ confirms, Esc writes nothing |
| `?` | the help, from any page; any key returns to where it was opened |
| ⌫ or Esc | back one level; Esc quits only from the menu, and from `config edit`, which has no level above it |
| `q`, Ctrl-C | quit, asking once first when an answer is unsaved |

Every key that changes a value acts on the highlighted row and no other, and every page keeps its
cursor for the session. After a save the screen stays on the page and row it was asked from. The
review is the plan computed over the screen's state — `Plan::to_touch`, the same computation
`cli::agents_to_touch` runs at the write — so it names exactly the agents the write will touch: the
ones to install, and for every touched agent the settings that moved. With nothing to write `s` says
so and shows nothing. `estigia config edit [--agent <slug>]` opens directly on the page of the table
it was handed, and asks for the same confirmation. A row whose answer is about this machine or this
repository is carried into every installed contract, exactly as
`estigia config set "<row>" "<value>"` writes it; `--agent <slug>` opens on that agent's own rows
alone, so a machine-wide row is not offered where only one agent would answer it. Without
`--agent`, a save that moved an agent-scoped row is refused before anything is written.

A model ID that another host's curated catalog lists and the target's own does not is **advised,
never refused**: the value is stored as typed and one sentence names the host that owns it. It is
said by `config set --agent <slug> "Model routing" …`, on the agent page's detail for a stored
route, and on the every-agent page once one typed ID goes to ticked agents that do not list it. An
ID in no curated catalog says nothing, and `Model routing` still validates no ID against a catalog.

### What the options page draws for a row

The page draws a control only where there is a decision to make.

- **An open row offers real suggestions.** `Tracker` and `Repository host` offer `github`, and
  `github <owner>/<name>` when the remote of the repository being configured — `origin`, else the
  first remote — is a GitHub URL (`https://github.com/o/n`, `git@github.com:o/n` or
  `ssh://git@github.com/o/n`, with or without `.git`). Any other host adds nothing. `Worktree
  location` offers `unset` and `<home>/worktrees/<repo>/<branch>`, an absolute template built from
  this machine's home directory. `Irreversible commands` offers `none` and the four commands in the
  table above. Every suggestion is a value the row accepts and reads back unchanged.
- **An open row with nothing to suggest is a text field.** With no GitHub remote, `Tracker` and
  `Repository host` have a single answer, so Space opens the field directly and the row says what it
  accepts instead of drawing a one-entry picker.

The remote is read by the terminal shell when the page opens and whenever another repository is
picked, not by the screen's state machine.

### Model routing suggestions are agent-specific

The stored value remains exactly the one cell above — comma-separated `key=model` pairs and, after
the `;`, `key=effort` pairs — and the CLI remains the place to edit that complete cell. The TUI does not show a synthetic `Model routing` setting, raw editor, clear-all,
or Advanced stage. A separate block beneath the settings projects
an adapter-specific model profile, where one is reviewed, followed by `orchestrate`, universal
`apply`, and the delegated roles
`developer`/`analyst`/`reviewer`/`judge`. The retired phase keys — `explore`, `propose`, `spec`,
`design`, `tasks` — are not keys at all: a stored cell naming one is dropped by the tolerant read and
named in a report, and the strict read refuses the cell. Workflow states are no longer keys either: they named no context to start, reached nothing,
and a stored cell holding one is refused rather than read past. A table written before that carried
one is not stranded: a write takes the same keep-what-parses path `config set` uses for a row an older
build accepted, keeps every entry it can, and prints a report naming each key it had to drop and, where
a rename removed it, the key that replaced it — the cell is never rewritten to `unset` in silence.
`estigia setup` and `estigia tui` refuse until a write clears it. Run one before anything else on
that machine: every other `config set` takes the same tolerant read and re-renders the whole table,
reporting the same dropped keys.

Claude Code and Codex offer three reviewed presets: `balanced`, `performance`, and `economy`.
Choosing one replaces that agent's complete model route; it never merges with stale custom targets.
The profile row reads `custom` whenever the complete route does not exactly match a preset. Choosing
`custom` preserves the current route, and the target rows below are its editor. Profiles expand into
the same persisted cell, name only the roles, `apply` and `orchestrate`, do not prove model availability,
and do not make Estigia run a model. Naming a role in the routing is what installs its definition, so a
preset that names `developer` installs it, and every reviewed preset names `analyst` too, at that
preset's own `judge` tier. OpenCode's catalog is dynamic, and adapters without a stable reviewed
catalog offer only custom target editing rather than invented defaults. The all-installed-agents
page also offers no profile because they have no single adapter whose model namespace can own it.

`by task` is the row's third value and is not a preset: it names no route, so it installs no role
definition and switching to it retracts any `developer` or `analyst` a previous value installed, on
the next `setup`. It says the shape of the work decides the model and the effort, and the launching
agent records what it chose; what that record is and is not is in
[`docs/honesty.md`](honesty.md).

The `Authorisation posture` row above is a different thing, and it is the shipped default. It carries the two
authorisations that can ask: `balanced` makes `Delivery authorisation` and `Transition
authorisation` both `auto`, and `cautious` makes both `ask`. An explicit authorisation written
after it wins, and `Model routing` is not part of it — it stays exactly as the operator left it.
A reviewed preset is an answer the operator chooses on the screen, never one `setup` reaches for on
its own: `setup` installs a delegated role only when the effective `Model routing` names it, so with
the row unset no role definition is written at all. The reserved
reviewer `review-blind` keeps its literal `model: inherit` in every posture: its bytes are pinned
against an earlier release and shared by the launch contract, so a model named for `judge` reaches
it through the prompt the orchestrator supplies, not through the file.

The effort is not in that list. `←`/`→` on a target's row step it through the host's own default,
`low`, `medium`, `high`, `xhigh` and `max`, wrapping both ways, and write it to every destination the
row writes; a row whose destinations disagree starts from the host's default. The effort is its own
decision, so a target may run on the inherited model at a named effort — which is how a role is
installed before a model is chosen for it. Choosing a different model keeps the effort already named,
and the custom field edits only the model.

Each projected row displays its exact assignment or `inherit` — the whole route, effort included — and opens its concrete host's advisory model list directly.
A current custom ID remains in that list, custom input is always available, and
`inherit` removes only that target. Enter and Space both choose a picker entry; Space remains literal inside the custom editor.
Every per-target update uses one destination set: the all-installed-agents page means the
currently ticked agents, while one agent's page means that agent. Each destination's own
route is staged and unrelated assignments survive. `r` restores only the selected target from each
destination's own installed route. Every destination validates before the staged map commits.

The all-installed-agents page aggregates each target independently: one shared model, shared `inherit`, or
`different values`. If selected agents disagree on their route, every target's row reads that
disagreement; the detail asks the operator to unify the routing or edit agents one at a time.

Choosing another checkout in `Repository shown` replaces the Everywhere and Repository rows with that
checkout's own answers and makes them the installed ones, so they are not read as unsaved; it refuses
while those rows carry an answer that is not installed rather than overwriting it. A typed value on
an open row is left alone by ←→, which only step between answers the row offers. A failed OpenCode
model catalog is not cached: the next time the model list is opened it is asked for again.

A `Model routing` edit on the all-installed-agents page uses that same ticked-agent set; a loaded but unticked agent is neither
changed nor consulted for its dynamic rows. Successful TUI saves acknowledge only effective
selected-agent settings that the installer persisted and read back. `ConfigLayers` retains four
cumulative snapshots instead of flattening provenance: portable contract-only config, that adapter's
own override, the operator's local override, and the repository layer. It also retains the explicit row
ownership of each override document; `ConfigLayers::effective_over` applies only those owned rows over
a new portable candidate. Ownership is scope-filtered before value validation: an agent file parses and
owns only agent-scoped rows, a repository file only repository-scoped rows, and forbidden hand-edited
rows are ignored even when invalid rather than rejecting or pinning the layer. Invalid owned rows still
refuse. One selected adapter owns each shared
skill-root setup action; `SKILL.md` is
rendered once from contract-only portable config, while a selected shared-root agent's model
changes land only in its own override. Local, repository, and another agent's values are never promoted
into the shared contract — not by the screen, and not by `config set` or `config edit` without
`--agent`, which refuse an agent-scoped row, and not by `config set --agent <slug> --contract` on a shared
root, which refuses it too (`contract-is-shared`) — and a private-root agent keeps its answer in its contract without a duplicate
override. Dynamic host artifacts such as the delegated role definitions use the effective view, so
local and per-agent model rows materialize them and retract them when those rows stop naming the
role. The reviewer is configuration output too, since `Evidence standard` decides its tool grant and,
  the paragraph asking a verdict for its basis; launch-time routing still selects its
model. A repository save preserves the rows that document already owns and unions only
repository-scoped rows changed in that session; one explicit row never widens into every repository
setting. Direct agent
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
warning; later saves compose with earlier model edits in either order.

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
target such as `judge` therefore stays visible with its picker attached above or below it, and a
truncated model panel shows its scrollbar. Rows and
scrollbar use the same cursor-aware target, so both follow the highlighted row. Space on the agents
list remains an ordinary agent toggle rather than opening a row.
Help uses the same runtime translation table as every other screen line: display labels are localized,
while persisted setting names, accepted values, model IDs, and printed CLI commands remain canonical.

There is deliberately no universal model list. `AgentAdapter` owns the source for its host:

- Claude Code suggests `fable`, `opus`, `sonnet`, and `haiku`.
- Codex suggests `gpt-6-astra`, `gpt-6.1-sol`, `gpt-6-luna`, and `gpt-5.5`.
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
- Every other adapter, the neutral adapter, and an all-installed-agents answer invent no catalog and borrow
  none from the agent under the cursor. Custom input and target-local `inherit` remain available.

These are **advisory suggestions, not validation**. Unknown and future model IDs remain valid and
visible when they fit one persisted entry: no comma, pipe or line break. Dynamic suggestions that do
not fit that same canonical grammar are omitted, and choosing an ID that cannot be assigned does not
close the picker as though it succeeded.
Selecting one does not prove the host accepts the ID, filter the
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

A write to that file through a write tool is not adjudicated as an ordinary write, and it is a
blanket boundary — see [*Which root the gate decides in*](#which-root-the-gate-decides-in) above for
the full account: every such write re-reads the tracker rather than riding the renewal window,
whichever row it names, including one the gate itself never consults. A
shell write to it gets the same answer a shell write to any other control-surface file gets, and that
answer no longer changes when `REPOSITORY_SHELL` matches the same line first — issue #85, closed
above.

**A repository may tighten the two authorisations.**

`Delivery authorisation` and `Transition authorisation` stay agent rows, and a repository's file may
also carry them. Precedence is repository, then agent, then contract, with
one rule: **the repository may only tighten**. Whether a merge or a tracker move may happen
unattended is a property of the repository's risk as much as of which agent holds the tools, and the
file lives in `.git/` and is written by `config set --repo` without adjudication — a layer that can
only remove autonomy cannot reopen the shape #32 closed, a row there that *granted* a capability.

The agent reads the effective answer from the contract's own sentence, which points at the
repository file by the route that resolves wherever the checkout is worked on —
`$(git rev-parse --git-common-dir)/estigia/estigia.local.md` — because a linked worktree's `.git`
is a file, not the directory that holds it.

- `config set "Delivery authorisation" ask --repo` (and `ask 30m`) writes only that row into the
  repository's file. `config list` in that repository shows the value and marks the row
  `(repository)`; the agent's own row is unchanged.
- `auto --repo` over an `ask` any layer holds is refused (`repository-cannot-loosen`), naming the
  file that holds the `ask` — the operator's `estigia.local.md`, the agent's own file, or the
  contract — and naming a command only where running it clears the block. The agent's own file gets
  `estigia config set … --agent <slug>`; the contract gets `--agent <slug>` when the agent discovers
  its skills, whose contract that form writes, and the same form on a shared root, where it writes the agent's
  own file. The operator's file is named with no command, because Estigia will not edit it. Nothing is written.
  `auto --repo` over an agent that is already `auto` is accepted as a no-op.
- A hand-written `auto` over an `ask` is **not honoured when read**: the effective value stays `ask`
  and the row is not reported as one the repository answers for.
- Between two `ask` rows the repository's wins, whatever the durations: a longer or shorter wait is
  not autonomy, so it is not compared.
- `Profile cautious --repo` writes the two rows as `ask`. `Profile balanced --repo` is refused
  (`repository-cannot-loosen`) — it would be a grant; `estigia config forget` takes the repository's
  rows away if that is what is wanted.
- `Model routing`, `Loop plugin` and `Goal loop plugin` remain agent-only, and `--repo` still refuses them
  (`setting-not-the-repositorys`).
- The installed contract says the repository file may tighten these three rows, and that the
  stricter answer wins, because the agent obeys the table it reads first.

What the write-time refusal cannot see: an agent installed after the repository row was written, or
whose own row is changed to `ask` later, is not consulted then — read time is what holds, and it
puts the stricter answer back.
