# Binary lifecycle

What `estigia update` reports, what the installers record before replacing an executable, and the
preflight every real `setup` or `sync` runs first. Read this when you need to know what the
recorded provenance does and does not prove — the short answer is in the last paragraph, and it is
narrower than the machinery suggests.


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
`skill::FILES` and `AGENT_DEFINITIONS`, from its own build. The latter contains exactly five SDD
planning phases, the one stable blind-review definition, and the two delegated workers; it is not
part of `skill::FILES`. A
record is accepted only when both equal the currently compiled values. Release records are create-once and keyed by canonical SemVer;
build metadata is rejected, and the greatest readable key is the high-water, so an older cooperating
writer has no mutable value it can lower. Records reject unknown fields. Existing semantically equal
records are idempotent; different values conflict. Readers open once and inspect/read the same handle.
Unix rejects a final symlink with `O_NOFOLLOW`; Windows opens the reparse point itself and rejects its
attributes. Intermediate path components remain subject to platform pathname resolution. A malformed,
unreadable, symlinked, non-regular, non-canonical, or key-mismatched record is unknown state, not an
absent record, and Estigia does not overwrite it.

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

Before a real `setup` or `sync` changes any adapter, one global preflight applies to the whole batch,
guided setup included. An unrecorded or source-built running binary refuses by default and requires
that command's explicit `--allow-source-build`. The flag does not create recorded provenance or advance,
erase, or lower high-water. An installer-recorded release below high-water refuses even with the flag; unknown
lifecycle state fails closed. `--dry-run` performs no lifecycle read or write, and uninstall remains
available because it removes rather than deploys binary-owned assets. Preflight is an unlocked
snapshot before a batch: it does not make the later adapter mutations atomic with history publication.
Installer publication followed by executable replacement is fail-closed but not atomic: a copy failure
can advance high-water while leaving the old binary installed. Concurrent installers are not
serialized. Re-reading per adapter would still race and would permit partial-batch behavior, so this
slice does not pretend it closes that concurrency boundary.

Where guided setup applies it is the screen's **install**, not the command that opens the screen.
`estigia setup` with no arguments — and its `install` and `tui` aliases — draws the screen on a
source build, and the refusal arrives when a plan is confirmed, carrying the same code and the same
way out as the shell's. It is shown over the screen and still leaves as the process's exit code.
Refusing at the door was the earlier behaviour and it protected nothing: opening the screen deploys
no asset, and the way out the refusal named was that same screen one flag later. What it cost was the
read-only half — the rows a person who has just built the binary opens it to read.

Then swear to an issue:

```sh
estigia claim 12 --run-id claude-0198fe1c --horizon 2026-08-01T18:00Z
estigia gate Edit --run-id claude-0198fe1c --input '{"file_path":"src/x.rs"}'
estigia release --run-id claude-0198fe1c
```

An agent does this through the MCP tools instead. `estigia status` reports both halves separately,
because "the agent was told about Estigia" and "Estigia can stop it" are different answers and an
operator looking at a run that wrote without a claim needs the second one:

```
claude-code    configured
               harness: gate on, tools on
codex          configured
               harness: gate off, tools on
```

The run id is minted from the agent's session and reported by `SessionStart`, so the agent has it in
context. It is required rather than guessed: a claim recorded under the wrong run-id is a claim the
gate will never match, and being asked beats being silently wrong.
