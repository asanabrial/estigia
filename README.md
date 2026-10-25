<p align="center">
  <img width="1024" alt="estigIA — AI harness. Light. Local. Agentic." src="assets/branding/estigia-banner.png" />
</p>

<p align="center">
  <a href="https://github.com/asanabrial/estigia/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/asanabrial/estigia/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-blue.svg" /></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.97+" src="https://img.shields.io/badge/rust-1.97%2B-orange.svg" /></a>
  <a href="#the-tools"><img alt="MCP: 40 tools" src="https://img.shields.io/badge/MCP-40%20tools-purple.svg" /></a>
  <a href="#install"><img alt="Linux, macOS, Windows" src="https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg" /></a>
  <a href="https://glama.ai/mcp/servers/asanabrial/estigia"><img alt="Glama MCP server" src="https://glama.ai/mcp/servers/asanabrial/estigia/badges/score.svg" /></a>
</p>

---

# estig**IA** — workflow authority for AI coding agents

Your agent says it claimed the issue. It says the review passed. It says the branch is current.
Estigia is what turns those sentences from things you trust into things that were checked: the claim
is recorded in the local workflow store (`~/.estigia/state.db`), every write the agent makes afterwards is measured
against it, and a review verdict is bound to exact bytes — so a later push invalidates it, and nobody
has to remember to notice.

One binary. It sits under whatever agent you already use, and it does not run your model. It launches
two kinds of detached process and no others: a reviewer CLI for a lens, refuter or validator
(`launch_review_input`), and the declared checks (`run_checks`, through `__check-job`); see
[what it writes](docs/what-it-writes.md#which-processes-estigia-launches).

> *El río del juramento. Lo que un agente promete, queda ligado a unos bytes exactos.*

**Leteo** is the river of forgetting. **Estigia** is the river of the oath — the gods swore by its
waters, and perjury cost nine years outside Olympus. Two rivers of the same underworld, opposite jobs:
one makes you forget, the other holds you to it.

## What it feels like

Mostly, nothing. Your agent calls Estigia only to claim the issue it is about to work on; a run that
has claimed nothing works as before. The moment it claims issue 12, every file it writes is measured against that claim, and every step that cannot be undone —
a push, a merge, a tag, a release — re-verifies the claim against the store first. When it refuses, it
says what it read and what to do about it:

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
It does not run your working model, define your tools, or manage your context — it holds one thing: the
workflow authority. A feature that does not fit that sentence does not go in.

## FLOW

FLOW is how a run works under Estigia: the claimed issue is the record of the work, and the
publication is the review candidate. The contract an agent reads is
[`skill/SKILL.md`](skill/SKILL.md#flow); the GitHub half is [`delivery.md`](skill/bindings/github/delivery.md).

1. **Six states, one at a time.** `analysis` (not yet a valid specification), `ready` (implementable,
   unassigned), `in-progress` (claimed, being built), `review` (built, awaiting delivery evidence or
   permission), `blocked` (an exact exit condition and who discharges it) and `done` (merged and
   verified). The local store is authoritative; labels are a projection, written and read back.
2. **Claim before the first write.** `claim` takes a run id and a horizon and the store adjudicates:
   the earliest live claim wins. The gate re-verifies it on the writes it sees and at every
   irreversible boundary.
3. **Build, then publish.** Understand, route, execute test-first, verify. The publication — a draft
   pull request with its epoch, head, base and digest — is the review candidate; it says `Refs #<n>`,
   never a closing keyword.
4. **Review sized by risk.** `review_tier` reads the diff: no lens at `low`, one at `medium`, four at
   `high`, over one immutable target, by a context that did not write the change. Per-tier cost is in
   [`review-cost.md`](docs/review-cost.md); [`blind-judges.md`](skill/policies/blind-judges.md) settles findings.
5. **A verdict bound to exact bytes.** The computed verdict records the epoch, pull request, head, base
   and digest it was produced on. Any push moves one of them and invalidates it: `republish_review`
   and ask again.
6. **`deliver`.** One call records the verdict, releases CI, waits for it, merges at the reviewed head,
   reads the topology back and closes. It stops at the first step that needs a decision (receipt,
   review, authorisation, CI, base, version, merge, close) and names it with the `next` to follow.
7. **Close with evidence, then release.** `release` records the spend, moves the issue to `done`, posts
   the evidence (review, CI, tests, delivering SHA, run id) and puts the claim down, in that order
   ([details](docs/what-it-writes.md#a-run-id-release-and-prune)).

## Install

1. Run the installer. Nothing to install first; it checks the archive against its published checksum
   and refuses to install what does not match.

   ```sh
   curl -fsSL https://raw.githubusercontent.com/asanabrial/estigia/main/scripts/install.sh | sh   # Linux, macOS
   irm https://raw.githubusercontent.com/asanabrial/estigia/main/scripts/install.ps1 | iex       # Windows (PowerShell)
   ```

   The binary lands in `~/.local/bin/estigia`, or `%LOCALAPPDATA%\estigia\bin\estigia.exe` on Windows;
   `ESTIGIA_INSTALL_DIR` moves it and `ESTIGIA_VERSION` takes a tag other than the latest.
2. Or, with Node: `npx -y @asanabrial/estigia setup --all`. The package carries no binary; it fetches
   the same archive, checks the same `.sha256` and places it in the same directory
   ([why](docs/lifecycle.md)). It is also what the [MCP registry](https://registry.modelcontextprotocol.io)
   entry starts: `mcp-name: io.github.asanabrial/estigia`, which the registry reads to accept the
   `cargo` package.
3. Or build it, the one route that needs Rust: `cargo install estigia` (or `--git
   https://github.com/asanabrial/estigia` for main). A source build has no installer record, so the
   commands that write into your agents ask for `--allow-source-build` ([why](docs/lifecycle.md)).
   The [`Dockerfile`](Dockerfile) builds a server-only image; none is published.

## Set it up in your agents

Installing the binary changes nothing on its own.

1. `estigia tui` opens the setup screen; `s` shows what it will write. Without it:
   `estigia setup claude-code` for one agent, `estigia setup --all` for all; `--dry-run` on either
   writes nothing.
2. Optionally route a role to a model, the spelling the screen ends by printing:

   ```sh
   estigia config set "Model routing" "developer=inherit" --agent claude-code
   ```

3. In a repository where any agent may write, `estigia guard` installs a `pre-push` hook that refuses a
   push no live claim authorises, whoever typed it.

`setup` installs the skill, a directive in the agent's instruction file and the hooks, which go into
every agent with an event able to refuse — five of them, in a gate that reads five dialects. What else it
writes is in [what it writes](docs/what-it-writes.md#what-setup-installs-besides-the-skill-and-what-the-hosts-permission-mode-does-not-change).

## Verify

1. `estigia status`: what is installed, where, and whether each agent is told (`tools on`) and stopped
   (`gate on`).
2. `estigia doctor`: skill, gh, remote, guard, prune, permission and the rest, read-only.
3. `estigia sessions` and `estigia changes` (uncommitted work in every checkout) are read-only too.

## Uninstall

Uninstall removes what Estigia created and nothing else: not another tool's block, not your notes, not
a file of its own you have since edited, which it keeps and names.

1. Take it out of the agents. The first command changes nothing and says what the second will do;
   `estigia uninstall claude-code` leaves the other agents alone.

   ```sh
   estigia uninstall --all --dry-run
   estigia uninstall --all
   ```

2. In each repository that has the push guard, run `estigia guard --uninstall`. It removes only a hook
   Estigia wrote, and must go before the binary, which the hook calls.
3. In each checkout, run `estigia config forget`. `<checkout>/.git/estigia/` holds its repository
   settings and survives every uninstall, which was given an agent and not a repository; the command
   says which file it removed.
4. Remove the binary by the route you installed with; uninstalling the agents does not touch it.
   - Installer script or npm: delete `~/.local/bin/estigia` (or `$ESTIGIA_INSTALL_DIR/estigia`). On
     Windows delete `%LOCALAPPDATA%\estigia\bin\estigia.exe` (or under `%ESTIGIA_INSTALL_DIR%`) and
     take that directory out of the user `PATH`, where the installer added it.
   - Source build: `cargo uninstall estigia`.
5. Know what `~/.estigia/` holds before deleting it. `state.db` is the workflow store, one copy on this
   machine: claims, states, receipts, findings, verdicts, spend, tasks, the decision history, the
   session registry and the mailbox. Beside it: run pointers; `jobs/` (review and check jobs);
   `snapshots/` (up to three copies of the store); `lifecycle/` (the binary's provenance records);
   `workspaces/` and `roots/` (where installs are); the screen's language; your checkouts list. The
   last `uninstall --all` takes the store and the run state, and keeps `jobs/`, `snapshots/`,
   `lifecycle/`, the language and the list
   ([per entry](docs/what-it-writes.md#what-estigia-holds-and-what-each-uninstall-step-leaves)).
   Delete it only when you no longer want the history of what this machine's gate decided:

   ```sh
   rm -rf ~/.estigia
   ```

What `REPLACE` and `OVERWRITE` mean is in [`docs/what-it-writes.md`](docs/what-it-writes.md).

## A harness, not a document

The contract under `skill/` is text an agent may ignore, so Estigia holds the tools: a `PreToolUse`
gate, a `Stop` hook, a `pre-push` hook under git, and forty operations with schemas below. The local store is the
authority; the tracker keeps a projection that never decides. The diagram, the loop plugins and the
gate's rules are in [what it writes](docs/what-it-writes.md#the-gate-the-hooks-and-the-loop-plugins).

## The tools

Forty operations, table-driven. Thirty-seven each map to one the contract requires the binding to provide;
`send_message` and `list_sessions` are answered from the local store, and `deliver` composes the others
and the merge gate; none of the three is a binding operation. `tools/list` carries each schema and a
`readOnlyHint`. The eleven delivery tools (`claim`, `start_branch`, `publish_review`,
`launch_review_input`, `collect_review_input`, `record_computed_verdict`, `release_ci`, `deliver`,
`release`, `transition`, `review_status`) carry `_meta["anthropic/alwaysLoad"]`; every tool takes
`compact` ([cost](docs/review-cost.md#what-the-tool-menu-itself-costs)):

```
read   verify_claim
read   list_state
read   check_closing_keywords
read   review_status
read   review_read
read   review_tier
read   target
read   tasks
read   list_sessions
WRITE  claim
WRITE  handoff
WRITE  heartbeat
WRITE  transition
WRITE  comment
WRITE  blocked_until
WRITE  task
WRITE  send_message
WRITE  release
WRITE  ensure_states
WRITE  flush
WRITE  start_branch
WRITE  judge_workspaces
WRITE  assist
WRITE  draft_pr
WRITE  ready_pr
WRITE  review_stages
WRITE  create
WRITE  publish_review
WRITE  republish_review
WRITE  handoff_review
WRITE  record_review_finding
WRITE  record_disposition
WRITE  launch_review_input
WRITE  collect_review_input
WRITE  record_computed_verdict
WRITE  escalate_review
WRITE  record_spend
WRITE  release_ci
WRITE  deliver
WRITE  run_checks
```

Forty tools. The contract names seventeen operations every binding must map; the tools absorbed some
under a surviving name ([why](docs/github-binding.md#why-the-transport-is-in-this-process-and-why-the-server-is-hand-written)).
Arguments are in each tool's schema.

## Configuration

Thirty typed settings, in [`docs/configuration.md`](docs/configuration.md), grouped by theme and shown
in one order by `estigia config list`, the setup screen and the reference alike. Each row says whose
answer it is: this agent's, this repository's, every checkout's or this machine's.

## The ratchet

A message may name a command only when running it discharges the block; a refusal carries a typed code
and a runnable resolution ([classes](docs/refusal-classes.md#the-ratchet-and-what-it-was-started-from)):

```
  run: estigia setup --all   # or one of: agents, claude-code, codex, opencode, pi, cursor, zcode, deepseek-harness
```

## What this instrument does not measure

[`docs/honesty.md`](docs/honesty.md) names what this crate does **not** check, each with the
measurement that found it; [`docs/README.md`](docs/README.md) maps the rest of the set.

## Layout

```
src/harness/   the gate, the run pointer, the transport, the lifecycle protocol, doctor
src/setup/     agent adapters, companions, dry-run, exact inverse
src/config/    typed model, the settings table as data
src/cli/       dispatch, and the only place a refusal becomes an exit code
skill/         SKILL.md, bindings/, policies/, protocols/, references/, assets/, agents/ — not compiled
docs/          the reference the sections above point at, one question each
scripts/       the two installers a release is fetched with, and the Windows source reinstall
```

[`AGENTS.md`](AGENTS.md) — which `CLAUDE.md` is a link to — is how an agent works here.

## Exit codes

Four, and none of them means "something went wrong":

| Code | Meaning |
|---|---|
| `0` | the command did what it said |
| `1` | the command refused; the refusal says whether anything **had already been written** |
| `2` | the world may have changed and this process cannot tell |
| `3` | the invocation could not be read: nothing was attempted, and **nothing was decided** |

Why there are four: [`docs/refusal-classes.md`](docs/refusal-classes.md#the-exit-codes-and-why-there-are-four).

## Licence

MIT. Files derived from another MIT-licensed project carry its notice in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
