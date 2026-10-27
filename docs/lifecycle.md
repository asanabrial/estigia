# Binary lifecycle

What `estigia update` reports, what the installers record before replacing an executable, and the
preflight every real `setup` or `sync` runs before it writes. Read this when you need to know what the
recorded provenance does and does not prove — the short answer is in the last paragraph, and it is
narrower than the machinery suggests.

It is one document in [the reference set](README.md); [`what-it-writes.md`](what-it-writes.md)
covers the files setup writes into an agent, and [`configuration.md`](configuration.md) the settings
a run reads.

Three routes install a released binary, and all three are one sequence: download the archive for the
platform, check it against the `.sha256` published beside it, run `__record-install` on the extracted
candidate, and only then replace the installed executable. `scripts/install.sh` and
`scripts/install.ps1` are two of them; the npm package `@asanabrial/estigia` (`npm/bin/estigia.js`) is
the third, and it runs the installed binary afterwards with the arguments it was given. It installs into
the directory the scripts use rather than into npm's cache because `setup` writes the canonicalised
path of the running binary into every agent's hooks (`setup::executable_path`): a path that changed
with each version would leave those hooks naming a file the next upgrade removed. It reinstalls when
`estigia --version` at that path does not answer the package's version, or the one `ESTIGIA_VERSION` names. `cargo install estigia` is a
source build, exactly as `cargo install --git` is. The `mcp-name: io.github.asanabrial/estigia` line in
`README.md` is what the MCP registry reads to accept the `cargo` package, so it stays in the README.

**The Linux floor.** The `*-unknown-linux-gnu` archives are built on Ubuntu 24.04 runners and need glibc
2.39 or newer, which excludes Debian 12, Ubuntu 22.04 and RHEL 9. On those, `cargo install estigia`
is the route; `cargo install --git https://github.com/asanabrial/estigia` takes main. The floor is
kept for now and stated rather than lowered. Both installers tell a candidate that could not be
executed (the shell's 126 and 127, or a spawn error) from one that ran and refused: the first
is reported with the loader's own words when it printed any, says the platform is the reason, and names
those two commands; the second keeps the admission refusal above.

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

Work is lost by being forgotten more often than by being deleted, so `estigia changes [--json]` reads
every checkout git has registered for the repository, and every one a run pointer names, and reports
what each holds that no commit does. It is read-only and Git is its only source:

- the path, and the branch (`detached`, or `no commits yet`, when there is none);
- the uncommitted files, each with a marker (`M` modified, `A` added, `D` deleted, `R` renamed, `U`
  untracked) and its added and removed line counts where Git or the file can say;
- the commits not on the upstream, when it has one, and the newest modification among the files;
- the run and issue whose pointer names the checkout, and `orphan` when no readable pointer does.

A checkout git cannot be read in, or does not answer within fifteen seconds, is reported
`unreadable` with the reason, never omitted. `estigia prune` lists every checkout it keeps with its
reason, an unmerged and unclosed branch included, and names the uncommitted files it carries; what
`--confirm` removes is unchanged. What `changes` does not yet do is in [`honesty.md`](honesty.md).

Three notices carry the same reading to a session, each naming the checkouts and `estigia changes`:

- **At session start**, in the text every runtime already receives (Claude Code, Codex, OpenCode and
  Pi): the linked checkouts no readable run pointer holds that carry uncommitted files, with their
  branch and file count. The base checkout is not named, and neither is a bare repository registered
  as one. The read stops after three seconds and says how many checkouts it did not reach, since one
  not read is not a clean one.
- **At `done` and `release`**, appended to the tool answer: the files the run's own checkout still
  holds uncommitted. It is a notice and never a refusal. `release` already keeps a checkout that
  carries them, `done` removes nothing, and refusing would block delivery on files the person may mean
  to leave. A checkout git cannot be read in, or that `release` took down, says nothing.
- **In Pi**, `/estigia-changes` shows the listing to the person as a session entry that never reaches
  the model, and a widget counts the checkouts that hold uncommitted work from `estigia changes
  --json`, at session start and after each settled turn.
- **In Claude Code**, `/estigia-changes` is a slash command `setup` writes on every install; it asks
  the agent to run `estigia changes` and show the output unchanged (see
  [`what-it-writes.md`](what-it-writes.md) for both).

Codex has no surface of its own beyond the session-start text, and OpenCode and Claude Code have no
indicator beyond it either; [`honesty.md`](honesty.md) says what is not built.

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

## Where each install route puts the binary, and how to remove it

| Route | The binary | Removed by |
|---|---|---|
| `scripts/install.sh` (Linux, macOS) | `$ESTIGIA_INSTALL_DIR/estigia`, by default `~/.local/bin/estigia` | `rm` of that file |
| `scripts/install.ps1` (Windows) | `%ESTIGIA_INSTALL_DIR%\estigia.exe`, by default `%LOCALAPPDATA%\estigia\bin\estigia.exe`; the script adds that directory to the user `PATH` when it is not there | delete the file, then take the directory out of the user `PATH` |
| `npx -y @asanabrial/estigia` | the same archive, placed in the same directory the script above uses; npm's cache holds only the launcher, never the binary Estigia runs | the same removal as the script on that platform |
| `cargo install estigia` or `--git` | Cargo's bin directory, by default `~/.cargo/bin/estigia` | `cargo uninstall estigia` |

The provenance records the release installers write are under `~/.estigia/lifecycle` and are not
named by `estigia uninstall`; [`what-it-writes.md`](what-it-writes.md#what-estigia-holds-and-what-each-uninstall-step-leaves)
lists what the rest of `~/.estigia/` holds.

`scripts/build-install.sh` (macOS and Linux) or `scripts/build-install.ps1` (Windows) does the whole
loop for someone developing Estigia itself, from a checkout: it requires `rustup` and `cargo` already
on `PATH`, installs the pinned toolchain through that existing `rustup` if needed, compiles the locked
release build, reinstalls through Cargo, runs setup for every agent, and finishes with `status` and
`doctor`. Close the agents using Estigia first — Windows will not replace a running binary, and any
platform can refuse one an agent holds locked.
