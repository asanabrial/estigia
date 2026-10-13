section: The harness

- **A change records itself in its own file, so changelog entries no longer conflict between pull requests.**
  Issue #43. Every change inserted its entry at the same line of `CHANGELOG.md`, so the second of two branches cut
  from one `main` conflicted once the first merged. A change now adds `changes/<issue>-<slug>.md`, whose first
  line names the `###` group (`section: The harness`) and whose body is the bullet. `scripts/assemble-changelog.sh
  <version>` folds the fragments into that version's heading, newest issue first within a group, and deletes them;
  it refuses a missing or ambiguous heading, an unknown group and a malformed file without changing anything, and
  runs before `changelog-notes` so the tag's notes carry every change. Entries already in `CHANGELOG.md` stay where
  they are, and the count guards read the fragments as they read the unreleased entry
  (`docs/changelog-fragments.md`).
