# Generated files and declared checks

A lockfile or a generated client is too long to read and cheap to regenerate. Estigia can check such a
file by running the command that makes it, instead of reading its hunks. This document says how a
repository declares one, which commit that declaration is read from, and what the check answers. It
also covers the other thing a repository declares, `[[check]]` commands: how `run_checks` runs them
in the background and binds a receipt to the reviewed digest, and how the verdict gate reads it.

Two places read the declaration. The review payload shows a declared path as a manifest line instead
of its hunks, and `record_computed_verdict` refuses to record `approved` unless each such path
regenerates byte-identically. The two ship together: a manifest without the gate would let content
skip review. `review_read` with `view: verdict` also carries the result as `generated_files`; it
executes nothing unless the caller passes `reproduce: true`, because the command is repository code
that may use the network and a read needs no claim. Without it each touched declared path reads
`not-run`. [`honesty.md`](honesty.md) says what that leaves open.

## The declaration

`estigia.generated.toml` sits at the repository root and is checked in. One `[[generated]]` table per
file:

```toml
[[generated]]
path = "dashboard/package-lock.json"
command = ["npm", "install", "--package-lock-only"]
cwd = "dashboard"
```

- `path` is the generated file, relative to the repository root.
- `command` is an argv list, never a shell string: the transport spawns with no shell. It must name a
  program.
- `cwd` is optional, relative to the repository root. The command runs there.

A file that is not valid TOML, has a key other than these, repeats a `path`, gives an empty `command`,
or gives a `path` or `cwd` that is absolute, contains `..` or contains a control character is refused as
`generated-declaration-malformed`. It is never read as an empty declaration.

The file may also hold `[[check]]` tables, which this section does not read and
[Declared checks](#declared-checks) does; any other top-level table is refused.

## Lockfiles nobody declared

A lockfile that `estigia.generated.toml` does not declare reaches every reviewer in full. `estigia doctor`
reports a `lockfiles` row when `git ls-files` lists a file with one of these basenames, anywhere in the
tree, that the working tree's declaration does not name: `Cargo.lock`, `package-lock.json`,
`npm-shrinkwrap.json`, `yarn.lock`, `pnpm-lock.yaml`, `bun.lock`, `bun.lockb`, `poetry.lock`, `uv.lock`,
`Pipfile.lock`, `go.sum`, `composer.lock`, `Gemfile.lock`, `Podfile.lock`, `pubspec.lock`, `mix.lock`,
`flake.lock`, `gradle.lockfile`.

The row is advisory (reported like a skipped check, never broken) and names, per file, the `[[generated]]`
entry to complete: the `path`, and a `cwd` for a file below the root. It suggests no `command`. The usual
generators (`cargo generate-lockfile`, `npm install`, `poetry lock` and the rest) re-resolve dependencies
against whatever the registry holds now rather than reproducing the committed file, and a declared path that
does not reproduce byte for byte is refused. The operator supplies a `command` they have verified. Nothing is
declared for the operator. The row is absent when every such file is declared or there is none. It reads the
working tree, unlike the review, which reads only the base.

When `estigia.generated.toml` exists but cannot be read or does not parse, and a lockfile is tracked, the row
says so and gives the reason instead of listing lockfiles. Which of them the file declares is unknown, so none
is called undeclared and no `[[generated]]` entry is shown. It stays advisory. A file that is absent declares
nothing, as before.

## Only the base declares

A declaration counts only when read from the **base** commit of the change, through `git` against
objects, with the same isolation the review payload uses. The working tree and the head are never
read. A path declared only in the head is not declared, because the head is the thing under review and
a change could otherwise declare its own exemption.

## What reproduction does

For each base-declared path the change touches, Estigia checks the head out into a scratch directory
under the repository's git directory, **removes every declared path from it**, runs the command there
with a deadline, and compares the Git
blob id of the regenerated file with the blob id of the head. The scratch directory is removed
afterwards, and the caller's worktree and index are never written.

| Outcome | When | Code |
|---|---|---|
| `match` | the command exited cleanly and the bytes equal the head blob | none |
| `mismatch` | the command exited cleanly and wrote other bytes | `generated-reproduction-mismatch` (`refused`) |
| `unavailable` | the command did not start, timed out, exited non-zero, or left no file at the declared path | `generated-reproduction-unavailable` (`unavailable`) |
| `not-run` | `reproduce` was not passed, so no command ran | none |
| `falls-back-to-hunks` | the path is deleted, changed mode, or is not a regular file, so there is no content to regenerate | none |

Removing the declared paths first means a command that writes nothing, such as `true`, leaves no file
and is `unavailable`, never a match; it also holds when one command regenerates several declared files.
`unavailable`, `not-run` and `falls-back-to-hunks` are never a match. A declared path the change does not touch
is not listed. A declaration that cannot be read at all appears as `refusal` beside an empty
`outcomes`; the read does not fail.

See [`refusal-classes.md`](refusal-classes.md) for the classes and
[`skill/bindings/github/refusals.md`](../skill/bindings/github/refusals.md) for what to do on each code.

## The manifest in the payload

`review_payload` is a pure function of the base and head objects. For each path that the base declares
and that `base..head` adds or modifies with a regular-file mode on every side where it exists, the
payload carries no hunks. After every other path's hunks, in path order, it carries the normal header
and one line:

```
diff --git a/<path> b/<path>
# estigia-generated {"path":"<path>","base-blob":"<oid>","head-blob":"<oid>","head-bytes":<n>,"cwd":<null|"<dir>">,"command":["<argv>", ...]}
```

`base-blob` is all zeros for a file the change adds. The values are JSON, so a quote or newline in a
value cannot forge a line. The header keeps one `diff --git` line per path, unchanged. A deletion, a mode or type change and any undeclared path keep their hunks,
and a declaration the change itself adds is not read, so it cannot hide its own content.

The ceiling is measured after the substitution, which is how a regenerated lockfile past it publishes.
A base declaration that does not parse refuses the payload as `generated-declaration-malformed`; it is
never read as no declaration. A validator's payload over a repair's `parent..head` uses the declaration
and the eligibility of the receipt's own base, so a declaration added on the repair branch never turns
hunks into a line.

The reviewer prompt tells the reviewer a manifest line is verified by byte-identical regeneration and
is not to be reviewed by reading.

## The approval gate

When `record_computed_verdict` would record `approved`, and after the publication lane has cleared, it
runs the reproduction above for the receipt's `base..head`. It records `approved` only when every
manifest path is `match`. A `mismatch` refuses `generated-reproduction-mismatch`, an `unavailable` path
refuses `generated-reproduction-unavailable`, and the refusal names each failing path with its outcome.
A `rejected` or `escalated` computation runs nothing. The command runs under the claim, the claim is
read again afterwards, and nothing is written to the worktree.

## What the tier, the budget and the anchors count

A path the payload shows as a manifest line is a path nobody is shown hunks for, so two figures that
were read from the real diff now leave it out. Both read one function,
`generated::manifested_paths`, which is the same eligibility the manifest uses: declared at the
receipt's base, added or modified as a regular file, and not a deletion or mode change.

- **`review_tier`.** `changed_lines` and the content scan that derives the reasons, and therefore the
  level and the lens count, are computed over the diff without those paths. The answer names them in
  `generated_paths` (an empty list when there are none), so the exclusion is visible rather than
  silent. A repair's narrow range uses the declaration of the receipt's base, as its payload does. A
  change of nothing but generated files measures no path and reads `medium`, the answer an empty
  measurement has always had, not `low`.
- **The correction budget.** The original candidate's changed lines, which freeze the budget, and each
  repair's changed lines, which are charged against it, leave those paths out. A correction that
  regenerates a lockfile is charged for its other edits, not for the lockfile's churn. The scope rule
  is unchanged: a regenerated path still has to be one of the reviewed candidate's own paths.
- **Finding anchors are unchanged.** A finding's location is still checked against the real diff, so a
  reviewer who opens a generated file through `inspect` can still anchor a finding on it.

[`honesty.md`](honesty.md) says what this trades away.

## Declared checks

The same file can declare the checks a change must pass. A repository that declares none behaves as it
always did; one that does cannot clear a verdict without a passing result for the exact bytes reviewed.

```toml
[[check]]
name = "unit-tests"
command = ["cargo", "test"]
cwd = "."
```

- `name` is required, unique, and lowercase `a-z0-9` groups joined by `-`.
- `command` is an argv list, never a shell string, and must name a program.
- `cwd` is optional and follows the rules `[[generated]]` gives its own.

A file that is not valid TOML, has a key other than these, repeats a `name`, gives an empty `command`
or a bad `cwd` is refused as `checks-declaration-malformed`, never read as no checks. An absent file or
one with no `[[check]]` table declares none. The declaration is read from the **base** commit only, as
[above](#only-the-base-declares), so a change cannot rewrite the checks it must pass.

**The tool.** `run_checks` (`issue`, `run_id`, optional `wait_seconds`) verifies the live `review` claim,
derives the clean target of the run's checkout against the base its published receipt names, and refuses
a dirty tree. It never runs a check inside the call: it starts the hidden `estigia __check-job <dir>`
detached (the same lock, liveness and detaching `__review-job` uses, and the same
`review-job-detach-refused` where the platform refuses), then waits up to `wait_seconds`, never more than
50, so no call outlasts the 60 seconds some hosts allow. In order:

1. Nothing declared: `{"ok": true, "declared": false}`, and nothing is recorded.
2. A `checks-receipt` already stands for this target digest and these commands: the answer is that
   receipt with `cached: true`, and nothing runs.
3. A job for the same digests is running: it is waited on, then answered `state: "running"` with `job` and
   `ask_after`.
4. Otherwise the job is launched and waited on.

The job runs each check in `<checkout>/<cwd>`, sequentially in declaration order, a failing check not
stopping the rest, under a 30 minute deadline for the whole job after which its process group is killed
and the answer is `timed-out`. Each check records its `name`, `exit` (`null` when killed or never
started), `seconds` and `tail`, the last 2,000 bytes of its combined stdout and stderr.

**The receipt.** A finished job is recorded as a `checks-receipt` marker in the issue's local ledger,
carrying `run-id`, `digest` (the target digest), `commands` (a SHA-256 over the declared checks in order,
each field length-prefixed), `passed`, and `results`, one `name=exit:seconds` entry per check with `none`
for an exit that was not a code. The tails are in the answer and never in the marker. A timed-out job
records nothing. The target is derived again when a finished job is about to be recorded; a tree that
changed or went dirty during the run records nothing and refuses `checks-target-moved`.

**The gate.** `record_computed_verdict`, before it records `approved`, and `release_ci`, through the
recomputation that clears a receipt, require a trusted `checks-receipt` for exactly the receipt's digest
and the base's current commands whenever the base declares checks. None recorded, or one for other bytes
or other commands, refuses `checks-receipt-missing`; one recorded with `passed=false` refuses
`checks-failed`. A `rejected`, `escalated` or `incomplete` computation is refused under its own name first.

**What is cached.** A receipt, passing or failing, answers every later call for the same digest and
commands without running anything. A new publication has a new digest and asks again.
[`honesty.md`](honesty.md) says what the key leaves out.
