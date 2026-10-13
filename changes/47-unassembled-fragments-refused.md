section: The harness

- **`changelog-notes` refuses while a changelog fragment is still unassembled.** Issue #47. The operation reads
  only the version's entry in the changelog, so a cut that skipped `scripts/assemble-changelog.sh` produced notes
  without the fragments' changes, and an immutable tag and Release carried them. It now refuses
  `unassembled-changelog-fragments` when `changes/` holds a fragment of this format (another tool's files do not count), listing the names and naming the script;
  with none it answers as before. The test of this repository's own fragments no longer returns early when
  `changes/` is gone: it asserts the fragments assemble, or that the directory is absent or empty
  (`docs/changelog-fragments.md`).
