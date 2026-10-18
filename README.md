<p align="center">
  <img width="1024" alt="estigIA — AI harness. Light. Local. Agentic. A pixel-art night desk: a cat at a console reading HARNESS CONTROL, status running, flow stable, agents online, beside a lit chain of cards — input, router, tools, memory, output — joined by one glowing stream"
       src="assets/branding/estigia-banner.png" />
</p>

<p align="center">
  <a href="#install">Install</a> &bull;
  <a href="#a-harness-not-a-document">How it works</a> &bull;
  <a href="#the-tools">Tools</a> &bull;
  <a href="#configuration">Configuration</a> &bull;
  <a href="docs/honesty.md">What it does not measure</a>
</p>

<p align="center">
  <a href="https://github.com/asanabrial/estigia/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/asanabrial/estigia/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-blue.svg" /></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.97+" src="https://img.shields.io/badge/rust-1.97%2B-orange.svg" /></a>
  <a href="#the-tools"><img alt="MCP: 35 tools" src="https://img.shields.io/badge/MCP-35%20tools-purple.svg" /></a>
  <a href="#install"><img alt="Linux, macOS, Windows" src="https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg" /></a>
</p>

---

# estig**IA** — workflow authority for AI coding agents

Your agent says it claimed the issue. It says the review passed. It says the branch is current.
Estigia is what turns those sentences from things you trust into things that were checked: the claim
is recorded in the local workflow store (`~/.estigia/state.db`), every write the agent makes afterwards is measured
against it, and a review verdict is bound to exact bytes — so a later push invalidates it, and nobody
has to remember to notice.

One binary. It sits under whatever agent you already use, and it does not run your model. The one exception is a review: `launch_review_input` launches a reviewer CLI for a lens, refuter or validator: the first of `claude` then `codex` found on `PATH`, with no setting to choose between them. `record_lens_report`, `record_refutation` and `record_validator_result` are refused by hand as `recorded-by-capture-only`; only the capture writes them.

> *El río del juramento. Lo que un agente promete, queda ligado a unos bytes exactos.*

**Leteo** is the river of forgetting. **Estigia** is the river of the oath — the gods swore by its
waters, and perjury cost nine years outside Olympus. Two rivers of the same underworld, opposite jobs:
one makes you forget, the other holds you to it.

## What it feels like

Mostly, nothing. You do not call Estigia and neither does your agent, beyond claiming the issue it is
about to work on. After that it is simply there, and the only time you hear from it is when it says
no.

A run that has claimed nothing is not under its authority at all and works exactly as it did before —
gating every session would be a lock rather than a workflow. The moment your agent claims issue 12,
every file it writes is measured against that claim, and every step that cannot be undone — a push, a
merge, a tag, a release — re-verifies the claim against the store before it happens.

When it does refuse, it says what it read and what to do about it:

```
$ git push origin HEAD
estigia: git push: gh issue view failed (1): none of the git remotes ... (read-failed)
  [world-action] the tracker could not be read; write nothing and retry the read
error: failed to push some refs
```

*The tracker could not be read* is a different answer from *you may not push*, and Estigia will not
turn the first into the second. That distinction is most of the design.

## What it is not

It is not memory ([Leteo](https://github.com/asanabrial/leteo) does that), and it is not planning.
It does not run your working model (a reviewer is the one process it launches), define your tools, or manage your context — it sits under whatever
agent you already use and holds one thing: the workflow authority. A feature that does not fit that
sentence does not go in.

## Install

```sh
# Linux and macOS
curl -fsSL https://raw.githubusercontent.com/asanabrial/estigia/main/scripts/install.sh | sh
```

```powershell
# Windows
irm https://raw.githubusercontent.com/asanabrial/estigia/main/scripts/install.ps1 | iex
```

That is all of it. Nothing to install first — not Rust, not Python, not a runtime. The archives are
prebuilt binaries, and each script checks its download against the checksum published beside it and
refuses to install anything that does not match. The binary lands in `~/.local/bin`, or on your PATH
on Windows; `ESTIGIA_INSTALL_DIR` moves that and `ESTIGIA_VERSION` takes a tag other than the latest.

With Node already on the machine, npm is the same installer:

```sh
npx -y @asanabrial/estigia setup --all
```

The package carries no binary. It fetches the same archive, checks the same `.sha256`, records the
candidate the same way, places it in the same directory, and runs it; see
[`docs/lifecycle.md`](docs/lifecycle.md) for why it never runs Estigia out of npm's cache. The same
package is what the [MCP registry](https://registry.modelcontextprotocol.io) entry starts, as
`npx -y @asanabrial/estigia mcp`. Its name there is `mcp-name: io.github.asanabrial/estigia`, written out
rather than in a comment because the registry reads that line in the crate's README to accept the
`cargo` package, and crates.io strips HTML comments.

To build it yourself instead, which is the one route that needs Rust:

```sh
cargo install estigia                                     # the published crate
cargo install --git https://github.com/asanabrial/estigia  # main as it stands
```

A source build is treated as a source build from then on: it has no installer record, so the commands
that write into your agents ask for `--allow-source-build` before they will. That is deliberate — see
[`docs/lifecycle.md`](docs/lifecycle.md) for what the record proves, which is narrower than it sounds.

If you develop Estigia itself, `scripts/build-install.sh` (macOS and Linux) or
`scripts/build-install.ps1` (Windows) does the whole loop from a checkout: requires `rustup` and
`cargo` already on `PATH`, installs the pinned toolchain through that existing `rustup` if needed,
compiles the locked release build, reinstalls through Cargo, runs setup for every agent, and
finishes with `status` and `doctor`. Close the agents using Estigia first — Windows will not
replace a running binary, and any platform can refuse one an agent holds locked.

## Set it up in your agents

Installing the binary changes nothing on its own. This is the step that puts Estigia in front of an
agent:

```sh
estigia tui                    # the setup screen: a list of agents, each opening onto its own settings; `s` shows what it will write; `u` uninstalls an installed agent, or the push guard from the menu, after listing what would go
estigia setup claude-code      # or just one, without the screen
estigia setup --all            # or all of them
```

Look before you leap, at any time:

```sh
estigia setup claude-code --dry-run   # exactly what would change, writes nothing
estigia status                        # what is installed, where, and what runs still hold
estigia sessions                      # the sessions seen on this machine, the issue each holds, and which went silent
estigia sessions send <run-id> "..."   # leave a message in another session's mailbox; its next hook point delivers it
estigia doctor                        # skill, skill-assets, gh, remote, guard, prune, pi-subagents, lockfiles, permission — read-only
estigia prune                         # branches and checkouts no delivery owns; --confirm removes
estigia changes                       # uncommitted work in every checkout, held or orphan — read-only
estigia flush --status               # the GitHub writes a closed git operation window queued; plain `flush` sends them
```

A host's automatic permission mode approves the host's own prompts. It does not approve Estigia's
decisions: a refusal stays a refusal and a question stays a question under Claude Code's
`bypassPermissions` and `auto`, OpenCode's `--auto` and Codex's `never` approval policy, because the
gate answers before the host's permission layer is asked and nothing in the gate reads the mode.
Tests hold the hook and plugin answers equal with and without the mode, and `estigia doctor` prints the
mode each of Claude Code, Codex, OpenCode and Pi is configured with. What was run live, and what was
not, is in [`docs/honesty.md`](docs/honesty.md).

`setup` installs three core things: the skill an agent reads, a short directive in that agent's
always-loaded instruction file, and the lifecycle hooks that make the authority mechanical rather than
advisory. `--skill-only` stops before the directive. Estigia knows eight agents; the hooks go into
every one with an event able to refuse — five of them, in a gate that reads five dialects, three only for retired hosts. Every Claude Code setup also
installs two stable inert reviewer definitions: `review-blind` and `review-blind-sealed`, one
template rendered twice, which are Judgment Day's two judges and nothing else. The two judge
definitions carry `model: inherit` and a paragraph asking a verdict to say what it rests on.
`Evidence standard` decides the unsealed grant — read-only under `reading`, a shell and nothing else
added under `measuring` — the sealed grant is fixed, one tool, a web search, no repository read. Both
are inert unless a launch names the immutable target — a head, a diff or a commit range — and criteria. The
orchestrator passes the effective `judge` model for each Judgment Day launch. Ordinary review licenses no judge at any tier; Judgment Day, gentle-ai v4's opt-in, is the only place they exist — see
[judgment day](skill/policies/judgment-day.md). Setup never creates numbered judge files.

The reviewers of an ordinary review are not installed. The sealed lenses `review_tier` licenses —
`review-lens`, `review-risk`, `review-readability`, `review-reliability` and `review-readback` —
the refuter `review-refuter` and the validator `review-validator` are launched by
`launch_review_input` from templates embedded in the binary, with no tools, and it never reads
`~/.claude/agents`. Setup retracts a copy an earlier build installed under one of those seven names: an owned,
unedited one is deleted; an edited one is kept and its path relinquished; one Estigia never recorded
is left alone. The names stay reserved, so a project definition cannot stand in for one, and a hand
`Task` launch of one is refused as `reviewer-launched-by-the-binary` — there is no hand path.
Two further definitions,
`developer.md` and `analyst.md`, are installed only where `Model routing` names their role, and are
retracted when the entry goes away, because a definition
carrying a tool grant is authority and an upgrade may not hand out authority nobody asked for. They
are installed for Claude Code and OpenCode, each in its host's own dialect: Claude Code's
comma-separated `tools:` line and `effort:`, OpenCode's `permission:` map and file name, with no
`effort:` field there. The routing is empty by default; naming a role installs it, and the model
beside it says only what it runs on. The row's third value, `by task`, names no role and installs
nothing: the shape of the work decides the model and the effort, and the launching agent records
what it chose — a declaration, not a check, as [`docs/honesty.md`](docs/honesty.md) states.
A definition at one of those paths that Estigia did not author is refused rather than replaced:
in the preflight, before anything at all is written, and again immediately before the bytes go
down, which catches one that arrived in between and by then has the skill and the directive already
written behind it. Direct `config set` and
`config edit` writes do not mutate external definitions.

Every reserved role is operator-owned. Before Claude Code's
current `Agent` or legacy `Task` launches any reserved role, Estigia walks from the launch cwd through
the first `.git` repository root and recursively rejects any project `.claude/agents` definition that
uses its filename or declares the same name in valid YAML. For a judge it then proves the canonical user definition
is the only user-scoped definition with that identity and matches one of the renderings Estigia can
produce of it — not the embedded bytes themselves, which the unrendered template would be; for one of
the seven binary-launched names the project walk is the whole proof and the launch is then refused,
whatever sits in the user tree. Setup performs the same recursive user-tree
uniqueness preflight for the two judges before writing, revalidates at the reviewer boundary, and creates a fresh reviewer
without replacing a path another actor won meanwhile. Missing, unreadable, changed, duplicated or
ambiguous evidence refuses. Ordinary agents retain project-first resolution. A refused or unprovable
reviewer does not count, and the fallback is a separate session or durable review handoff, never a smaller
or serialized review.

The setup screen is the one path here that needs a person, and it ends by printing the commands that
reproduce the same result without it:

```
the same result without the questions:
  estigia setup claude-code
  estigia config set "Model routing" "developer=inherit" --agent claude-code
```

Ordinary review licenses lenses over one immutable target and criteria: none at `low`, one at `medium`,
four at `high`. A lens's severe finding is settled by the ledger at every tier — no dial, no majority
and no agreement policy — stated in full in `skill/policies/blind-judges.md`. The transport records
one computed exact-receipt verdict; it does not prove the lenses ran. Judgment Day runs only on an
explicit request, replaces the lens review for that target, and issues no receipt.

What the retired judge panel cost is measured history in
[`docs/review-cost.md`](docs/review-cost.md); what a lens review costs per tier is not measured yet.

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

## Uninstall

```sh
estigia uninstall --all --dry-run   # what would go, changes nothing
estigia uninstall --all             # carry it out
```

The first tells you what the second will do. To leave one agent and stay in the rest:

```sh
estigia uninstall claude-code
```

And the push guard, which lives in a repository rather than in an agent:

```sh
estigia guard --uninstall
```

**Uninstall removes what Estigia created, and nothing else.** Not another tool's block, not your own
notes, not a file that was already there when Estigia wrote over it — and not a file of Estigia's own
that you have since edited, which it keeps and names on its own line rather than taking silently. It
knows the difference because install wrote down what it created and a digest of what it put there;
nothing on disk could tell them apart afterwards.

Two things it deliberately leaves. `~/.estigia/` is yours rather than any agent's — the screen's
language, your list of checkouts — and taking one agent out must not take it. A checkout's
own rows in `.git/estigia/` stay too: uninstall was given an agent, not a repository, and reaching
into whichever checkout you happened to be standing in is the failure that design refuses.
`estigia config forget` is the one command that removes those, and it says which file it removed.

The whole account, including what `REPLACE` and `OVERWRITE` mean and why they are two words, is in
[`docs/what-it-writes.md`](docs/what-it-writes.md).

## Swear to an issue

```sh
estigia claim 12 --run-id claude-0198fe1c --horizon 2026-08-01T18:00Z
estigia gate Edit --run-id claude-0198fe1c --input '{"file_path":"src/x.rs"}'
estigia release --run-id claude-0198fe1c
```

An agent does this through the MCP tools instead — you will not type these often. The run id is
minted from the agent's session and reported by `SessionStart`, so the agent already has it. It is
asked for rather than guessed, because a claim recorded under the wrong run id is a claim the gate
will never match, and being asked beats being silently wrong. `release` also puts down the isolated
checkout this run created — and the local branch checked out there — once the tracker recognises the
delivery and nothing unpublished remains; what it kept, and why, is named in its answer. It deletes
every remote head the run recorded starting, too, because the branch `start_branch` linked is not
always the branch published and GitHub's own auto-delete removes only the latter. `estigia prune`
clears what no delivery owned: it lists the merged or closed branches and checkouts, keeps any a live
claim holds or that carries commits the remote does not have, and removes the rest only on
`--confirm`. It also finds clean orphan checkouts whose remote branch is gone and whose commits
remain on another remote ref; [the pruning rules](docs/what-it-writes.md#explicit-orphan-pruning)
describe that additional route and its conservative ownership checks.


## A harness, not a document

Estigia is a **harness**: it does not ask an agent to follow the workflow, it holds the tools.

The contract under `skill/` is text an agent may read and may ignore — which meant "perjury detected
mechanically" was aspirational. Nothing stopped a run from writing without a claim, or from
delivering on a verdict bound to a SHA that no longer existed. The harness closes the first of
those, and binds the second without re-checking it at the moment of delivery — see [`docs/honesty.md`](docs/honesty.md) for exactly
where that line falls:

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

Two halves, and both are needed. The **tools** are how an agent does workflow work at all — thirty-five
operations with schemas, instead of composing `gh` by hand and hoping. A name the contract requires
and no tool performs is declared with the reason it is not exposed rather than advertised, so the
surface never offers a call nothing answers. The one force-push the
delivery path needs is among them, so it is adjudicated like every other write rather than typed into
a shell. The **gate** is what happens
when it tries to work around them.

Estigia does not run your working model — it starts a reviewer only through `launch_review_input` — and holds no authority of its own. **The local workflow store,
`~/.estigia/state.db`, is the authority**: claims, states, receipts, findings, verdicts, spend, tasks
and `blocked-until` clauses are rows in it, and two runs on one machine adjudicate against each
other in one transaction. The tracker keeps issues and their prose, pull requests, branches, and
labels and an assignee written after the local commit as a projection that never decides — the
price is that the store is one copy on one machine, as [`docs/honesty.md`](docs/honesty.md) says. A
run that dies leaves no *authority* behind: whatever the store says a checkout is bound by is the
whole of what governs it, whether or not the run that wrote it is still there to be asked. The **local** run pointer a dead session leaves behind is a different
question — a machine-readable note, never authority — and `estigia doctor`'s `stale-run-pointer` row
and the reconciliation against the store that the pre-push hook `estigia guard` installs runs —
and that `estigia gate` reaches equally — exist for exactly the phantom pointer a dead run does
leave. `estigia doctor`'s `prune` row, and the `estigia prune` command behind it, answer the other
half a dead run leaves: the branch and the checkout no live claim owns, listed and removed only when
a person confirms.

### The gate no agent can route around

An agent hook fires only for the agent that installed it. A `pre-push` hook sits **under git**:

```sh
estigia guard              # into this repository
estigia guard --uninstall  # removes only a hook Estigia wrote
```

```
$ git push origin HEAD
estigia: git push: gh issue view failed (1): none of the git remotes ... (read-failed)
  [world-action] the tracker could not be read; write nothing and retry the read
error: failed to push some refs
```

It catches the *last* moment rather than the first, and it catches it for every agent, and for a
person. The `PreToolUse` gate is still the one that matters most — it catches a lost claim race
hours before a push — but only one agent has it. Claude Code also registers a `Stop` hook: while
the run still holds its issue the turn continues instead of ending. That hook reads the claim at
most once per renewal window after a successful read (a failed or unknown read is read again), so a
long run of short turns does not spend the GitHub API budget. OpenCode can do the same, behind
the `Loop plugin` setting the TUI edits: with it `on`, `estigia setup opencode` writes an optional
plugin that continues a session while Estigia says the run is live, another issue is claimable, or a
`blocked` item's recorded exit condition already holds — and it keeps the goal, with the reason it
stopped, so a stopped loop is legible and `/flow-status` reports it; the agent can ask it to end
after the current task with `estigia goal stop-after`, which a live claim ignores and a released one
honours. A turn that ends while the run still holds a claim is continued with one sentence naming the issue and the run, not the whole objective again. A session that ended its turn only to wait on a running child session is waited on, not re-prompted, and continued when the child ends ([details](docs/what-it-writes.md#the-optional-loop-plugin)). Its commands are
`/flow-loop` and `/flow-status`, named so that they never shadow the generic `/goal` and
`/loop`. Pi gets it as an owned extension, `estigia-loop.ts`, that continues the session when Pi reports it settled and registers the same two commands; its goal lives in memory, so a restarted Pi forgets it. Codex gets the same loop as a `Stop` and a `SessionStart` hook entry under the same row, each trusted in `config.toml`, without `--wait`, `--max` or `/flow-status`. Claude Code gets it too: two slash commands under its `commands/` directory and a `Stop` entry, with the cap and `/flow-status`. The default `off` writes nothing. A separate `Goal loop plugin` row, also default `off`,
writes a generic `goal-loop.js` for OpenCode, or `estigia-goal-loop.ts` for Pi, that offers `/goal <condition>` and `/loop [interval] <prompt>`
with no Estigia command behind them: a model judges the transcript, on every third turn and on the cap turn, and
[`docs/honesty.md`](docs/honesty.md) says why that is never evidence. `estigia status` reports each agent's continuation word beside its harness line.

### The oath binds once sworn

A run that has claimed nothing is not under Estigia's authority, except where the repository's
`Unclaimed write policy` says otherwise, below: by default a repository write by such a run that the gate
can place inside the checkout is put to the person first, a write it cannot place is allowed with a
dirtied checkout reported at the next session start, and with the row `off` every write it makes goes
through untouched. Gating every session would be a lock, not workflow authority.

**One exception, and the operator opts into it.** In a checkout where `estigia guard` installed the
`pre-push` hook, a run holding no claim is refused every commit (`git commit`, `cherry-pick`,
`revert`, `merge`, `rebase`, `am`) and every `git push`, on every branch, with the refusal
`unclaimed-commit`; the `pre-push` hook refuses the same push when git runs it. The way out is to
claim an issue and work through `start_branch`, whose isolated checkout the claim covers. Reads
stay ungated, edits and tests are held only by `Unclaimed write policy` below, and a checkout without the
guard has no commit refusal, as does one with no tracker transport or no installed contract.

**The other exception is a repository setting, and it is on by default.** `Unclaimed write policy`
(`ask`, `off` or `required`, answered only by `<repo>/.git/estigia/estigia.local.md`) governs a run
holding no claim in a checkout with a tracker. `required` refuses every repository write it cannot
place outside the checkout, with `issue-required`; `ask`, the default, asks the person on Claude Code
for one it can place inside, refuses it on a host that cannot ask, and allows one it cannot place,
naming the checkout's dirty paths at the next session start; `off` is the behaviour described above.
See
[`docs/configuration.md`](docs/configuration.md) for what it covers and
[`docs/honesty.md`](docs/honesty.md) for what it does not.

The moment a run swears — `estigia claim 12` — every repository write it makes is measured against
that claim, and every irreversible boundary re-verifies it against the store. That is where the value is.
Incident I07 in the skill's own ledger is not "an agent wrote without claiming"; it is a run that
**lost a claim race by five seconds, was told 33 seconds later, and worked another 48 minutes**
because nothing in its loop read the claim again. That run had sworn. The gate kills that case.

```
$ estigia gate Edit --run-id claude-0198fe1c --input '{"file_path":"src/x.rs"}'
estigia: src/x.rs: gh issue view failed (1): no git remotes found (read-failed)
  [world-action] the tracker could not be read; write nothing and retry the read
```

A routine write, and the Claude Code `Stop` hook, may ride on an answer from inside a two-minute
renewal window — the cadence `SKILL.md` already asks for. **An irreversible boundary never does**: `git push`, `git merge`,
`git pull`, `git tag` when it creates or deletes a tag (listing is a read), `gh pr merge`,
`gh pr create`, `gh release create` and `gh release edit` always re-verify.

The gate only ever *reads* the claim, which is what lets it sit on the critical path of every edit:
being wrong costs a refused write, never a damaged issue. A test enforces that structurally.


## The tools

Thirty-five operations, table-driven. Thirty-three each map to one the contract requires the binding to provide;
`send_message` and `list_sessions` are answered from the local store and are not binding operations.
`tools/list` carries a schema and a `readOnlyHint`, so an agent can tell a read from a write without
reading this source:

```
read   verify_claim     (issue, run_id, expect_state)
read   list_state       (state, run_id, identities)
read   check_closing_keywords (issue)
read   review_status    (issue)   # carries `lane` and `corrections`
read   review_read      (issue, view, epoch, id, with_text, candidate, request, path, side, pr, head, base, digest, phase, worktree)
read   review_tier      (base, head, parent_head, clearing_finding)
read   target           (base, recorded_base, worktree, native_start, manifest)
read   tasks            (issue)   # the ODD checklist, from the local store
read   list_sessions    ()   # the entries `estigia sessions --json` prints
WRITE  claim            (issue, run_id, horizon[, takeover, target_operation, force, reason_file])
WRITE  heartbeat        (issue, run_id, expect_state, body_file)
WRITE  transition       (issue, to, run_id)
WRITE  comment          (issue, body_file)
WRITE  blocked_until    (issue, run_id, until_issue, until_state)   # local store only: no tracker write
WRITE  task             (issue, run_id, id, text, route)   # local store only: no tracker write
WRITE  send_message     (run_id, to, text)   # local store only; the person is asked first, over MCP elicitation
WRITE  release          (issue, run_id)
WRITE  ensure_states    ()
WRITE  flush            ([read_only])   # sends what a closed git operation window queued, oldest first; refuses while it is closed
WRITE  start_branch     (issue, run_id, branch, base)
WRITE  judge_workspaces (issue, run_id, epoch, pr, head, base, digest, count)   # bare per-judge directories, Judgment Day's
WRITE  assist           (issue, run_id, branch, base)         # assisted repositories only
WRITE  draft_pr         (run_id, title, body_file)            # assisted repositories only
WRITE  create           (identity, title, body_file, priority, domain, runtime, run_id)   # identity and clauses go to the local store, not the body; optional `repo` (owner/name) files in another repository, unclaimed
WRITE  publish_review   (issue, run_id, branch, base, pr_title, pr_body_file)
WRITE  republish_review (issue, run_id, branch, base, pr_title, pr_body_file)   # leased force-push
WRITE  handoff_review   (issue, run_id, target_operation, epoch, pr, head, base, digest, blocker, discharger)
WRITE  record_review_finding (issue, run_id, reviewer, epoch, pr, head, base, digest, id, class, evidence, impact)
WRITE  record_disposition (issue, run_id, epoch, pr, head, base, digest, dispositions)
WRITE  launch_review_input (issue, run_id, name)   # starts the lens, refuter or validator a review_status collect offers, detached
WRITE  collect_review_input (issue, run_id, name[, wait_seconds])   # records what the launched reviewer answered
WRITE  record_computed_verdict (issue, run_id, epoch, pr, head, base, digest)
WRITE  escalate_review  (issue, run_id, epoch, pr, head, base, digest, escalated_to, reason)
WRITE  record_spend     (issue, run_id, phase, epoch, pr, head, base, digest, conditions, input, tokens, seconds, contexts)
WRITE  release_ci       (issue, run_id, epoch, pr, head, base, digest)
```

Thirty-five tools. The contract names seventeen operations every binding must map. The operations the
tools absorbed keep their capability under the surviving name: `claim` performs `reclaim` through
`takeover`, `list_state` answers identities through `identities`, `review_status` carries the
publication lane and the correction count, `review_read` reads what six tools used to, `review_tier`
narrows a repair's range, and `target` classifies base movement. A name the contract requires and no
tool performs is declared in `NOT_EXPOSED` with the reason and the section of `bindings/` that carries
it; it is not advertised, so the surface never offers a call nothing answers. A seam test holds the two
lists together: an operation that is neither exposed nor declared fails the build.

The MCP server is hand-written rather than built on `rmcp`. The same binary answers a `PreToolUse`
hook on **every edit**, so a `tokio` runtime spun up per process is a cost paid thousands of times to
move a few lines of JSON across a pipe. Keeping the crate synchronous is what keeps the gate cheap.
What that costs is in `UNIMPLEMENTED`: no resources, no prompts, no sampling, no elicitation beyond `send_message`'s one question, no notifications beyond
`initialized`, and one request at a time.

## Configuration

Twenty-two typed settings, in [`docs/configuration.md`](docs/configuration.md), grouped by theme —
Authority, Repository & tracker, Gate limits, Review & change, Agent runtime, This machine — and
shown in that one order by `estigia config list`, the setup screen and the reference alike. Each row
says its scope: whose answer it is, whether this agent's, this repository's, every checkout's or this
machine's. Reading the table produces a valid configuration or a refusal naming what may be written
instead.

## The ratchet

> A message may name a command only when running it discharges the block.
> Naming a dead end is worse than naming nothing.

Every refusal carries a typed code, what happened to the world, whether a replay is safe, and a
resolution. The resolution is either a runnable invocation — checked against the real parser by
`every_command_a_rejection_names_parses` — or a reason from a closed vocabulary saying why no
command can honestly exist: `operator-knowledge`, `world-action`, `human-authority`.

```
$ estigia setup
estigia: setup needs to know which agent to configure (agent-not-named)
  run: estigia setup --all   # or one of: agents, claude-code, codex, opencode, pi, cursor, zcode, deepseek-harness
```

Measured against issue-flow on 2026-07-31: **87** rejection sites, **zero** naming an executable
continuation. Starting at zero is the only moment a ratchet can start.

## What this instrument does not measure

It has its own document: [`docs/honesty.md`](docs/honesty.md). Its entries name what this crate does
**not** check, each with the measurement that found it. A claim added anywhere else in this repository
is weighed against that file first, and [`docs/README.md`](docs/README.md) is the map of the set it
belongs to.

## Layout

```
src/harness/   the gate, the run pointer, the transport, the lifecycle protocol, doctor
src/harness/mcp/  the workflow tools and the stdio JSON-RPC they arrive on
src/setup/plugin.rs  the OpenCode gate, the Pi gate extension, the two review relays and the optional loop plugin and Pi loop extension, because those hosts read a module rather than a settings line
src/setup/pi_prompts/  the two Pi prompt templates, `claim.md` and `review-status.md`, embedded and written unchanged
src/setup/goal_loop.js  the generic `/goal` and `/loop` plugin, embedded and written unchanged
build.rs       derives the transport's refusal vocabulary from the transport itself, and embeds the build's commit and dirtiness
src/fence.rs   a marked block in somebody else's file — invariant three, written once
src/config/    typed model, the settings table as data, which markers the table lives in
src/setup/     agent adapters, companions, dry-run, exact inverse
src/skill.rs   the embedded markdown and its install/status/uninstall
src/outcome.rs the failure taxonomy: what landed, what may be replayed, what to run
src/cli/       dispatch, and the only place a refusal becomes an exit code
skill/         SKILL.md, bindings/, policies/, protocols/, references/, assets/, agents/ — not compiled
docs/          the reference the sections above point at, one question each, review cost included — map: docs/README.md
scripts/       the two installers a release is fetched with, and the Windows source reinstall
assets/        branding, referenced from this file
```

One document sits beside the code rather than inside it, and it answers a different question from
this one. [`AGENTS.md`](AGENTS.md) — which `CLAUDE.md` is a link to — is how an agent works here and
what it owes the documentation. This repository's planning artifacts live on the issue, and this
checkout keeps none.

`src/transport/` is the transport, and it is the only one. It began as `skill/scripts/github.py`,
shipped inside the payload because `bindings/github.md` routed every reversible operation through
it — a payload without it was a binding whose first command did not exist. The binding now names
Estigia's own tools, the operations are answered in this process, and the script is deleted: no
`.py` file in the tree, no Python on any path, nothing in CI that installs one. What the
crossings against it are worth now is in the honesty contract above, under its own heading.

*No interpreter at all* is what this sentence used to say, and it stopped being true when one test
began **executing** the OpenCode plugin this crate generates rather than reading its source. That
plugin is JavaScript; a gate written in a second language and never run is a copy of a rule nothing
crosses. So `node` is on the verification list in [`AGENTS.md`](AGENTS.md), both workflows install it
rather than trusting the runner image to, and `docs/honesty.md` records what those tests measure.
That first test has since gained two siblings, so three tests now require the runtime — the original,
which drives where the shell's command runs through both host generations,
`the_plugin_hands_the_gate_the_directory_every_write_lands_in`, which drives where every gated write
tool lands on OpenCode 1's `file_path` and OpenCode 2's `path`, and
`the_loop_plugin_continues_while_estigia_says_there_is_work`, which drives the optional loop
plugin's exits.

One crate. Five on day one would split boundaries before any of them had earned the split; a
boundary gets split out when it has earned the seat.

## Exit codes

Four, and none of them means "something went wrong":

| Code | Meaning |
|---|---|
| `0` | the command did what it said |
| `1` | the command refused; the refusal says whether anything **had already been written** |
| `2` | the world may have changed and this process cannot tell |
| `3` | the invocation could not be read: nothing was attempted, and **nothing was decided** |

The fourth is apart from `1` because the difference is the whole reason the hooks read a status at
all. A refusal is a decision — the world said no — and every script this crate writes propagates it,
which is what blocks a push. A usage error is not a decision, and propagating it blocks a push for a
reason the person typing it cannot act on. It was measured: a `pre-push` hook left from a build whose
`hook` took one more flag exited `1`, and `git push` came back `error: unexpected argument
--from-a-newer-build` with the push refused.

This section said **three** for as long as the fourth existed, which is the drift the first rule of
`CLAUDE.md` forbids — and the readers it describes are scripts. `every_exit_code_the_readme_lists_is_one_this_binary_has`
crosses the table against the enum now.

The harness translates the transport's own five codes into that taxonomy rather than collapsing
them. `1` the control surface was read and answered stop — do not replay. `2` the operator's
configuration is wrong — the one with a command that fixes it. `3` the control surface answered
**nothing** — retry the read, and never treat it as a stand-down or as clearance. `4` and `5` a
write may have landed and nobody saw — go and look.

The distinctions live in the refusal, where they can be read. issue-flow's `5` meant both "nothing
was written" and "something was written and I do not know what"; reporting the nearest named state
is a lie told with confidence.

## Licence

MIT. Files derived from another MIT-licensed project carry its notice in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
