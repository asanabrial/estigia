# Recording a change: fragments, and the version cut

A change does not edit [`CHANGELOG.md`](../CHANGELOG.md). It adds one file under `changes/`.

Every change used to add its entry as the first bullet of the same `###` group in the unreleased
section, so two branches cut from one `main` inserted at the same line and the second to merge
conflicted. A `merge=union` attribute was considered and rejected: GitHub's server-side merge does
not apply custom or union merge drivers. Two branches that add two differently named files cannot
conflict, whatever the files say.

## The fragment

`changes/<issue>-<slug>.md`, for example `changes/43-changelog-fragments.md`:

```
section: The harness

- **What changed, in one sentence.** Issue #43. The rest of the entry, wrapped
  as the changelog wraps it.
```

- The first line is `section: ` and the text of the `###` heading the entry belongs under in the
  version being cut (`The harness`, `Setup`, `Refusals`, ...).
- The body is the bullet, exactly as it should read in the changelog: it must open with `- `, and
  continuation lines are indented as in the changelog.
- The name opens with the issue number. That number is the whole of the ordering: within a section,
  fragments are placed newest issue first (numerically, so `10` before `9`), ties broken by the rest
  of the name. The order does not depend on when a branch was cut or merged.

## The version cut

```
scripts/assemble-changelog.sh <version>
```

It folds every fragment into the `##` heading of `CHANGELOG.md` that opens with `<version>`,
at the top of the `###` group the fragment names, and deletes the fragments. Edit that heading
(`unreleased` to a date) in the same change. The script takes an optional changelog path and
fragments directory as second and third arguments.

It refuses, and changes nothing, when:

- no heading, or more than one, opens with the version;
- a fragment names a `###` group the version does not have (a new group is created by hand first);
- a file in `changes/` is not named `<issue>-<slug>.md`, lacks the `section:` line, or has no
  `- ` bullet after it. A file is never skipped: a skipped fragment is a change missing from the
  release notes.

Run it **before** `changelog-notes`. That operation reads only the version's entry in the file it is
given, so a fragment not yet assembled would be missing from the notes the tag and the Release carry,
and a tag is immutable. It therefore refuses, `unassembled-changelog-fragments`, while
`<repo-dir>/changes/` holds a fragment: the answer lists the names and says to run
`scripts/assemble-changelog.sh <version>`. The directory is the repository's, whatever `--file`
names, so a component changelog is held to the same `changes/`. Only this repository's format counts: a file named
`<issue>-<slug>.md` whose first line is `section: <heading>`, the two tests the script applies, held to the
same cases by a test. A directory another tool keeps there (`123.feature.md`, a `.gitkeep`) does not trip it,
and neither does an absent or empty directory, the normal state after a cut. A file whose name has the fragment shape but whose first line cannot be read, or a `changes/` that cannot be listed, is a `read` failure naming it, not a file passed over: an unreadable fragment is the omission this refusal exists to prevent. The refusal is
checked before the entry is looked up, so a version cut that skipped assembly is told so rather than
told its entry is missing. `tests/changelog_assembly.rs` drives the script, including against this
repository's own `changes/`; in whichever state the checkout is in, that test asserts something: the
fragments assemble, or the directory is absent or empty.

`CHANGELOG.md` stays the published history. Entries already in it were not moved.

## What is and is not checked

- The present-tense count guards in `tests/honesty.rs` read the unreleased entry; they read the
  fragments too, so a number in a fragment is held to the code like one in the changelog.
- Nothing requires a change to carry a fragment. That is review's to ask for, as a changelog entry
  was.
- Nothing checks that the prose of a fragment is true; [`honesty.md`](honesty.md) says what the
  documentation checks do not reach.
