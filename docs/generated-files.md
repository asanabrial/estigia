# Generated files, verified by reproduction

A lockfile or a generated client is too long to read and cheap to regenerate. Estigia can check such a
file by running the command that makes it, instead of reading its hunks. This document says how a
repository declares one, which commit that declaration is read from, and what the check answers.

Two places read the declaration. The review payload shows a declared path as a manifest line instead
of its hunks, and `review_computed_verdict` refuses to record `approved` unless each such path
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
value cannot forge a line. The header keeps `changed_file_count`, which the delegation topology
refusal reads, unchanged. A deletion, a mode or type change and any undeclared path keep their hunks,
and a declaration the change itself adds is not read, so it cannot hide its own content.

The ceiling is measured after the substitution, which is how a regenerated lockfile past it publishes.
A base declaration that does not parse refuses the payload as `generated-declaration-malformed`; it is
never read as no declaration. A validator's payload over a repair's `parent..head` uses the declaration
and the eligibility of the receipt's own base, so a declaration added on the repair branch never turns
hunks into a line.

The reviewer prompt tells the reviewer a manifest line is verified by byte-identical regeneration and
is not to be reviewed by reading.

## The approval gate

When `review_computed_verdict` would record `approved`, and after the publication lane has cleared, it
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
