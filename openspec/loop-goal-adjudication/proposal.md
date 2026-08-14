# Loop goal adjudication

Give Estigia the continuation loop that keeps an agent working across turns, with the one property
the existing implementations lack: **completion is a tracker transition the authority adjudicates,
not a sentence the executor writes about itself.**

## Why this, and why now

On 2026-08-13 a run under `@bybrawe/opencode-loop` closed a goal like this:

```text
tool   : opencode_loop_goal_complete
summary: "Perfiles de modelos por defecto y edicion custom implementados y documentados."
evidence: "cargo test verde: 905 unitarias, 5 guards, 35 honesty, 2 payload, 93 integracion
           y 15 release. cargo clippy --all-targets, cargo fmt --check y cargo doc --no-deps
           verdes. Revision independiente completada y hallazgos corregidos."
output : "No active experimental goal was found."          <- title: "Goal not found"
```

Two separate defects, and the second is the one that matters here.

**The lost goal.** The plugin could not find the goal it was being told about. Its own README calls
`/loop-goal` an "older experimental" mode and points elsewhere for durable contracts. That is a bug
in somebody else's code and not our problem to fix.

**The unchecked evidence.** `evidence` is declared as a free-text string:

> `evidence: tool.schema.string().describe("Concrete evidence that the goal is complete, such as
> commands run, passing checks, files changed, and important results.")`

Nothing reads it. Nothing re-runs those commands. Nothing binds them to the bytes they were run
against. A run that had done none of that work could produce a better-looking string, and the loop
would have accepted it identically. The plugin *does* own a real verifier — `verifyCommand`, executed
post-turn through `runShellCommand` — but it is a per-job option wired to the schedule, **not to goal
completion**. The two never meet.

This is `A claim is adjudicated, not asserted` (`openspec/config.yaml`) stated as a defect. It is
also the failure the whole crate exists to refuse, arriving through a door Estigia does not yet watch:
not a write, not an irreversible boundary, but a *declaration that the work is done*.

The convergent evidence is worth recording. The same author's newer plugin, `@bybrawe/opencode-goal`,
was built specifically to fix this, and reaches our rule in its own words:

> "**Independent semantic verifier** — the executor does not get to mark itself successful just
> because it says the work is done."
> "**False-completion protection** — missing, stale, indirect, or invented evidence fails closed."

Two projects with no shared code arrived at the same rule. That is the strongest argument that the
rule belongs in the authority rather than in each agent's plugin — and Estigia is already the process
that holds it.

## What is being changed

A continuation loop owned by Estigia, in which:

- A goal is a claim on a tracker issue, with the run id Estigia already issues.
- `progress` and `blocked` are activity on the tracker timeline, beside the `heartbeat`, `branch` and
  `published` markers `src/transport/markers.rs` already defines.
- `complete` is a **transition request**, adjudicated by the same path that already refuses a bad
  one. The executor's summary is recorded; it is not the thing that decides.
- Evidence is bound to bytes. `A verdict is bound to exact bytes; every push invalidates it` already
  holds for reviews; a completion verified against a tree that has since moved is the same defect
  under a different name.

## What is NOT being changed

- **Not a port.** The plugin is 4,875 lines of JavaScript, and most of it — `src/source/opencode/`
  (`host.js`, `sdk.js`, `session-context.js`) — is opencode SDK plumbing that Estigia replaces with
  what it already owns. Translating it line by line would import an architecture built around a
  single agent's runtime.
- **Not a general job runner.** `/loop-shell`, `/loop-cmd` and the `opencode-loopd` daemon schedule
  arbitrary shell work. Estigia has no authority claim over that, and taking it on would widen the
  gate for no adjudicated benefit.
- **Not opencode-specific.** opencode is one agent among the ten `estigia setup --all` registers.
  A loop that only works there is a plugin, which is the thing being replaced.

## The seven pieces, and where each one lands

| Piece (from the plugin) | Verdict | Why |
|---|---|---|
| Heartbeat scheduler | **Partly exists** | `transport::claim::heartbeat` already writes a heartbeat marker with the run id. Missing: the cadence that keeps it beating unattended. |
| Persistent goals | **In** | This is the change. A goal is a claim, and its lifecycle is the tracker's. |
| Verification | **In, and rebound** | Estigia already runs adjudication. The fix is to make completion *depend* on it, closing the gap where the plugin left the verifier beside the goal instead of under it. |
| Checkpoints | **In** | Needed for restart recovery: a loop that forgets its goal is the first defect above. |
| Idle-safe loops | **Open question** | Detecting "the agent went idle" is per-agent. Estigia sees hooks, not turns — see below. |
| Compact scheduling | **Out** | `/compact` is an agent's own context management. Estigia has no view of it and should not pretend to. |
| Scheduled shell/commands | **Out** | See "not a general job runner". |

## Rust, and what that rules out

`openspec/config.yaml` already states the constraint: *"Nothing here needs an interpreter: the Python
this transport was ported from is deleted."* This work must not reintroduce one, which excludes
embedding or shelling out to the existing plugin.

The Rust surface is smaller than the plugin's line count suggests, because the pieces Estigia already
holds are exactly the ones the plugin had to build for itself:

| Already in the crate | New Rust work |
|---|---|
| Run ids, claims, claim verification (`transport/claim.rs`) | Goal state machine over an existing claim |
| Tracker activity markers (`transport/markers.rs`) | `progress` / `blocked` as marker kinds |
| Session state and ledger (`harness/session.rs`) | Durable goal checkpoint, restart-recoverable |
| Hook dispatch into ten agents (`harness/hook.rs`, `setup/`) | The continuation trigger, per agent dialect |
| MCP tool surface (`harness/mcp/`) | `goal_progress`, `goal_blocked`, `goal_complete` tools |
| Refusal machinery (`harness/guard.rs`) | Refusal for a completion whose evidence does not hold |

The plugin's `core/` modules are the honest measure of the genuinely new part: `args.js` (281),
`state.js` (211), `jobs.js` (80), `process.js` (51) — about 620 lines of parsing, state and job
bookkeeping, of which parsing and state have Rust equivalents in the crate already.

## Open questions

1. **What starts the next turn?** The plugin injects a turn through the opencode SDK. Estigia sends
   hooks and answers MCP calls; it does not drive an agent's turn loop. Either the continuation
   arrives as a hook response the agent honours, or this is a per-dialect adapter in `setup/`, and
   the answer changes the size of the change substantially. **This is the question to settle first.**
2. **What counts as evidence?** The narrow reading — a recorded CI result already bound to a digest —
   is buildable today and refuses cheaply. The wide reading — re-running the suite — makes Estigia a
   build runner. The narrow one is proposed.
3. **`max-no-progress`.** The plugin stops after 3 turns without progress (`DEFAULT_GOAL_MAX_NO_PROGRESS`).
   A stall is not a refusal and must not be recorded as one; `An unknown result is not clearance`
   says it is also not a pass. Probably `blocked`, with the stall named.
4. **Does this need its own tracker issue before it goes further?** Per `openspec/README.md` the issue
   wins over this file. This document is the material for that issue, not a substitute for it.

## Status

Draft. Written from the plugin source at `ByBrawe/opencode-loop@0.5.27` and the transcript of the
2026-08-13 failure. Nothing has been implemented, and no issue has been filed yet.

**The plugin was deliberately not reinstalled.** It had been in use and was lost with everything else
on 2026-08-13; restoring it was declined in favour of this change, so an editor with no `/loop` is the
intended state and not a gap to be closed by putting it back. Reinstalling it would also make this
change harder to land rather than easier: two goal lifecycles over one repository, one of them
adjudicated and one asserting, is a worse arrangement than either alone.
