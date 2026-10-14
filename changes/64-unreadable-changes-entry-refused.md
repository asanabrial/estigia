section: The harness

- **A `changes/` entry the listing cannot read refuses `changelog-notes` instead of being skipped.** Issue #64.
  `unassembled_fragments` iterated the directory with `flatten()`, which drops an entry whose read fails, so a fragment
  that could not be listed was omitted from the check that exists to catch omitted fragments. Such an entry is now a
  `read` failure (`docs/changelog-fragments.md`). The same change adds tests for the open-window and singular
  forms of the queued-writes note and for `record` filing a decision under the decision kind.
