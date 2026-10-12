# Binary lifecycle

What `estigia update` reports, what the installers record before replacing an executable, and the
preflight every real `setup` or `sync` runs before it writes. Read this when you need to know what the
recorded provenance does and does not prove — the short answer is in the last paragraph, and it is
narrower than the machinery suggests.

It is one document in [the reference set](README.md); [`what-it-writes.md`](what-it-writes.md)
covers the files setup writes into an agent, and [`configuration.md`](configuration.md) the settings
a run reads.

`estigia update` is read-only. Text and global `--json` output report the pathname returned by
`current_exe`, a SHA-256 inventory of bytes reopened through that pathname, whether a local installer
record uses that lookup key, the package version and embedded skill/agent-definition digest compiled into
this process, their relation to this machine's recorded release high-water, and the latest
public-release status. Portable Rust does not expose mapped-image identity here: pathname bytes are
raceable and the digest is not a claim about the exact machine code currently executing. This slice makes
no network request and no public Release is configured, so it says **latest public release is not
verifiable**. It never turns absence or an unavailable remote into `current`.

Lifecycle evidence lives under `~/.estigia/lifecycle`, separately from each skill's
`.estigia/installed.json`, and survives adapter uninstall. Provenance records are create-once and
keyed by the observed pathname-bytes SHA-256; the publisher derives canonical SemVer and a
deterministically typed/count-framed digest of the two top-level asset collections,
`skill::FILES` and `AGENT_DEFINITIONS`, from its own build. The latter contains the one stable
blind-review definition — rendered twice at install, sealed and
unsealed, under two reserved names — and the two delegated roles, plus the seven reviewer
templates the binary embeds and `launch_review_input` launches (the five sealed lenses, the
refuter and the validator), which setup does not install; none is
part of `skill::FILES`. A
record is accepted only when both equal the currently compiled values. Release records are create-once and keyed by canonical SemVer;
build metadata is rejected, and the greatest readable key is the high-water, so an older cooperating
writer has no mutable value it can lower. Records reject unknown fields. Existing semantically equal
records are idempotent; different values conflict. Readers open once and inspect/read the same handle.
Unix rejects a final symlink with `O_NOFOLLOW`; Windows opens the reparse point itself and rejects its
attributes. Intermediate path components remain subject to platform pathname resolution. A malformed,
unreadable, symlinked, non-regular, non-canonical, or key-mismatched record is unknown state, not an
absent record, and Estigia does not overwrite it.

The install scope does not move the workflow store. A `--scope=workspace` install writes each
runtime's files into the checkout — [`what-it-writes.md`](what-it-writes.md) has the scope and its
per-adapter destinations — but the store stays where it already is, `<home>/.estigia/state.db`,
resolved from this machine's home. A workspace install and a machine-wide one therefore **share one
store**, and a run started from a checkout reads the same entries, claims and run pointers as a run
started outside it. The sharing is deliberate: the store records what runs did on this machine, not
what any one checkout holds, and a per-checkout copy would let two runs adjudicate the same claim
against two timelines. The rule that follows is the one the contract states from the other side — a
run is refused rather than adjudicated against a store it cannot see — so a checkout that carries an
Estigia install but cannot read the machine's store gets a refusal, never a silent local answer.
Because the store is shared, the machine-side state an `uninstall` reclaims — the run pointers, the
decision and role history the store holds, stand-down files, stop-after requests and the screen
language — goes only once nothing on the machine reads it: a workspace install records itself under
`<home>/.estigia/workspaces`, so a machine-wide `uninstall`, which cannot enumerate the checkouts,
sees that one still reads the store and leaves the state alone, and the record goes when the last
adapter leaves the checkout. A global install records the configuration roots it resolved under
`<home>/.estigia/roots` in the same way, so an `uninstall` under one `CLAUDE_CONFIG_DIR` keeps the
store while an install under another root still reads it, and a root record it cannot read keeps the
store too and is named in the report. The store file itself, `<home>/.estigia/state.db` and its `-wal` and
`-shm` neighbours, goes with that last adapter, and
[`what-it-writes.md`](what-it-writes.md) says what it is.

The official release installers now extract the checksum-verified archive and ask that extracted
candidate to run the hidden, argument-free `__record-install` admission command before replacing the
installed executable. The candidate resolves its own `current_exe`, hashes bytes reopened through
that pathname, derives its compiled canonical SemVer and typed/count-framed embedded-asset digest,
checks readable high-water before publishing anything, then publishes provenance before release.
Exact replay is idempotent. A downgrade, malformed/non-regular state, or conflicting provenance
refuses replacement; a provenance failure cannot advance release history. PowerShell explicitly
checks the candidate process's native exit code. The installers never run the previously installed
binary and never pass version, executable digest, or asset identity through shell arguments.

These local JSON files are **installer-recorded provenance/history, not authenticated evidence**. The bounded
anti-downgrade object is the embedded asset set setup/sync can deploy plus compiled package SemVer. A
pathname replacement with the same version and exact asset digest cannot downgrade those assets;
different version/assets fail against the record. This does not authenticate mapped code, exact
executing bytes, arbitrary code authenticity, or state against a malicious same-user writer. The
hidden command is an admission seam for cooperating official installers, not a user security boundary
against arbitrary invocation.

The typed digest and renamed record fields use lifecycle schema 3. Earlier schema-2 records fail
closed as unsupported rather than being reinterpreted under this narrower model.

Before a real `setup` or `sync` changes any adapter, one global preflight applies to the whole batch.
An unrecorded or source-built running binary refuses by default and requires that command's explicit
`--allow-source-build`. The flag does not create recorded provenance or advance, erase, or lower
high-water. An installer-recorded release below high-water refuses even with the flag; unknown
lifecycle state fails closed. `--dry-run` performs the same lifecycle read and reports the same refusal the real run would, and writes no
lifecycle state; uninstall remains
available because it removes rather than deploys binary-owned assets. Preflight is an unlocked
snapshot before a batch: it does not make the later adapter mutations atomic with history publication.
`estigia tui` can uninstall too: `u` on an installed agent (or on `Push guard` in the menu) lists what
`uninstall --dry-run` (or `guard --uninstall --dry-run`) would remove, and ⏎ runs the same code the CLI
runs. The screen then re-reads the agent's block, or the hook, and shows it as gone only if that read
says so; a partial, failed or unread-back removal is reported as such and makes the TUI exit non-zero,
and a later successful install does not clear it. Like the CLI it needs no preflight.

Installer publication followed by executable replacement is fail-closed but not atomic: a copy failure
can advance high-water while leaving the old binary installed. Concurrent installers are not
serialized. Re-reading per adapter would still race and would permit partial-batch behavior, so this
slice does not pretend it closes that concurrency boundary.

`estigia tui` is the screen, with no flags: a menu, a list of agents each opening onto its own page, and
the repository's options, with `s` from any page laying what it will write over the screen first (see
[`configuration.md`](configuration.md#how-the-screen-is-navigated)). Confirming that review is the
source-build consent;
the preflight still runs at that install and still refuses a recorded downgrade or unreadable state.
Direct `setup` and `sync` keep the flag. Bare `estigia setup` does not open the screen.

**A save from the screen touches only the agents it changes something for.** `s` offers, and its
confirmation runs, setup for an agent that is ticked and not yet installed, or that has a setting differing from what the screen
opened on (an Agent-scope row of its own, or an Everywhere or Machine row that reached its rows);
`cli::agents_to_touch` computes that set at the write, over the same `Plan::to_touch` the review lists,
so the screen names exactly the agents it will touch. Other ticked agents are not
written and a failure in their setup cannot affect the save. A stale installed skill does not make
an agent changed: refreshing it is `estigia update`'s work. With nothing changed the save writes
nothing and says so. `estigia setup` from the command line keeps its full pass, and the read-back
receipt still fails the save on any changed setting it cannot acknowledge.

**The screen is not read-only before that install**, and this paragraph is not a claim that it is.
Three keys write before any plan is confirmed: the guard key installs a `pre-push` hook into the
repository the screen is showing, the language key writes the remembered tongue under the home, and
`n` in the folder picker creates a directory, saying so on the screen when it does. None is an
adapter asset, and the gate at the install covers none of them. The door refusal used to stop all
three, and it was never the only route to that hook: `estigia guard` writes it from the shell with
no preflight either. The guard key reads the hook back and reports the state it found by name: a
hook of the operator's own that already hands the push to Estigia is reported as chained with
nothing written, and a refusal, or a hook that does not read back as installed, is also the exit
code of `estigia tui`.

Leaving with answers nobody installed asks once, whatever else the screen last said, and Ctrl-C
leaves under the same rule. Every other Ctrl chord is ignored rather than read as its letter, so
Ctrl-S does not install. The warning is drawn on every page that can quit. `estigia config edit`
checks for a terminal before it reads anything, and its `s` writes nothing when nothing changed. A
session that installed more than once reports every install, and a later failure does not hide an
earlier one.

Then swear to an issue:

```sh
estigia claim 12 --run-id claude-0198fe1c --horizon 2026-08-01T18:00Z
estigia gate Edit --run-id claude-0198fe1c --input '{"file_path":"src/x.rs"}'
estigia release --run-id claude-0198fe1c
```

`release` is the last step, and it deletes every remote head the run recorded starting — the branch
`start_branch` linked is not always the branch published, and GitHub's own auto-delete removes only
the latter. `estigia prune` is the operator-invoked sweep for what no delivery owned; the contract's
delivery reference has the rules, and [`honesty.md`](honesty.md) what neither can see.

An agent does this through the MCP tools instead. `estigia status` reports both halves separately,
because "the agent was told about Estigia" and "Estigia can stop it" are different answers and an
operator looking at a run that wrote without a claim needs the second one:

```
claude-code    configured
               harness: gate on, tools on
codex          configured
               harness: gate off, tools on
```

The tools half can be permanently unavailable: an adapter Estigia has no verified place to register
a server with prints `tools off` and the reason, and no `setup` run changes it.

The run id is minted from the agent's session and reported by `SessionStart`, so the agent has it in
context. It is required rather than guessed: a claim recorded under the wrong run-id is a claim the
gate will never match, and being asked beats being silently wrong.
