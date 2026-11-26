# Changelog

One `##` heading per version, newest first, and a few one-line bullets under it. A pull request does
not edit this file: the agent cutting a version writes its entry in the release pull request, from
the pull requests merged since the previous tag. `changelog-notes` reads that entry for the tag and
the Release, and refuses a missing or empty one.

## 0.2.0 — 2026-11-26

- The MCP surface is the twenty-seven tools the work needs: one `pr_review` route, one `review` route, compact answers, and the tools every turn uses loaded every turn.
- A pull request this run did not build is reviewed under its own claim: `flow review`, the blind panel, and the verdict submitted from that context.
- A review thread is answered, resolved and voiced from the same tools, with findings posted inline.
- `deliver` runs the tail of a delivery in one call — lenses, verdict, CI release, merge, close — and hands back at the first decision only.
- The gate sees a git write inside a command substitution, behind a quoted option, or beside an interleaved flag, and holds an unclaimed merge, tag, release or GitHub write.
- `Issue required` replaces the unclaimed-write policy rows, and `estigia config list` says which layer answered each setting.
- `estigia where`, `feed`, `sessions`, `resume` and the mailbox answer what a run is doing and what it was asked.
- The git operation window queues GitHub writes, lets work continue locally while it is closed, and wakes the run when it opens.
- Uninstall leaves an operator's hook files alone and restores a pre-existing empty wrapper instead of taking it away.
- A claim creates its checkout; a release puts down every slice checkout the delivery contains and keeps the claims it parked.
- `setup` writes the Pi look, the Codex operator keys and a resume command that names the stored binary.
- The `honesty` document is split into the areas it measures, and the Windows suite runs in three partitions.
- The aarch64 Windows target is gone; the five archive names the installers fetch are unchanged.

## 0.1.1 — 2026-10-17

- The README carries `mcp-name: io.github.asanabrial/estigia`, which the MCP registry reads in the published crate before it accepts the `cargo` package; 0.1.0 was refused for its absence.

## 0.1.0 — 2026-10-17

The first version. Estigia is a harness: it does not ask an agent to follow the workflow, it holds
the tools.

- A claim on an issue, adjudicated in the local store (`~/.estigia/state.db`); every repository write is measured against it.
- A gate under Claude Code, Codex, OpenCode and Pi that refuses writes, pushes and merges the claim does not justify.
- Review verdicts and CI runs bound to an exact head and base; any push invalidates them.
- Receipt-driven blind review: tiered lenses, refuter and validator launched and captured by the binary.
- Delivery checks: merge only reviewed, green, at the reviewed head, on its base or a cleanly advanced one.
- GitHub tracker binding, with workflow states as labels; Linear read-only for assisted work.
- Assisted mode: work on a named issue with no tracker writes, ending in a draft pull request.
- A git operation window that queues GitHub writes while it is closed.
- Sessions, a session registry and a mailbox delivered at hook points.
- Loop and goal-loop plugins that keep a run going until the tracker says it is done.
- Pi subagents with a live card, a full view and history.
- `estigia setup` and `uninstall` for each supported agent, and `doctor`, `changes` and `prune` to inspect and clean up.
- Linux, macOS and Windows builds.
- Installed by `install.sh` / `install.ps1`, by `npx @asanabrial/estigia`, or built with `cargo install estigia`; listed in the MCP registry as `io.github.asanabrial/estigia`.
