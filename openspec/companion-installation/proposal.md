# Companion installation

Let `estigia setup` offer to install the tools it already knows it works better beside — codegraph,
context7, and the companion set the operator asked to mirror — instead of only naming them.

Filed as a note. Unlike [`loop-goal-adjudication`](../loop-goal-adjudication/proposal.md), nothing
here needs to be rebuilt under the authority: these are somebody else's installers, and the work is
to *call* them, not to reimplement them.

## Why

`src/setup/companion.rs` is already the right shape — "Tools Estigia works better beside, declared
rather than special-cased. A table, not `if leteo`" — with a deliberate ladder:

1. Detect and name the command. Trust surface: none. The default.
2. `cargo install <crate>`, when `cargo` is on the path.
3. `curl … | sh` only after an explicit opt-in, with the URL shown first.

And the rule that keeps it honest: *"Estigia does not reimplement a companion's installer … Copying
that is two copies of the trust logic, and the second one is always the stale one."*

The table currently declares exactly one entry: `leteo`. Everything else an operator actually runs
beside Estigia has to be found and installed by hand, which is how a machine ends up with Estigia
registered in ten agents and none of the tools those agents' instructions assume.

`estigia setup --companion` already exists and, by its own help text, "installs nothing". Closing
that gap is this change.

## What is being added

Declared entries for the companions in real use:

| Companion | What it is | Install route |
|---|---|---|
| `codegraph` | MCP server; a SQLite symbol/edge graph queried per project | npm global — **new route** |
| `context7` | Remote MCP server (`https://mcp.context7.com/mcp`) | **Nothing to install.** Configuration only |
| The mirrored set | The companion plugins another harness registers | Follow each companion's own route |

## The problem this surfaces

The existing ladder has two rungs — `cargo install` and `curl | sh`. Neither fits the companions
above, and that mismatch is the actual design work:

- **codegraph is npm.** A third rung, `npm install -g`, has a different trust surface from `cargo`
  and needs the same explicitness step 3 already gets. Adding it silently would widen the gate.
- **context7 installs nothing at all.** It is a URL in an MCP config. The table has no concept of a
  companion that is pure configuration, and forcing it into an "installer" field would make it read
  as though something gets downloaded.
- **A registered-but-absent companion is worse than an undeclared one.** If `setup` writes an MCP
  entry for a server whose binary is not there, every agent it registered starts a session against a
  server that fails to launch. Declaration and installation must not be able to drift apart.

## What is NOT being changed

- Estigia still does not rewrite anybody's installer, and step 3 keeps its explicit opt-in.
- No companion is installed as a side effect of `setup --all`. `Configuration may only tighten`
  (`openspec/config.yaml`) reads here as: adding Estigia must not add software the operator did not
  ask for.

## Open questions

1. Does the ladder grow an `npm install -g` rung, or does codegraph get the "detect and name" rung
   only, leaving the install to the operator?
2. Does the table need a `configuration-only` kind for context7, or does that belong somewhere else
   entirely — it is an MCP entry, and `setup/` already writes those?
3. Which of the mirrored companions are actually wanted here? The list has to be read from the
   reference installation rather than recalled, and this proposal does not yet name them.

## Status

Draft note, no issue filed. Requested 2026-08-13 alongside `loop-goal-adjudication`; kept separate
because the two share no code and only one of them needs building from scratch.
