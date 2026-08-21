# Secure binary lifecycle tasks

1. Add parser and process tests for update and source override shapes.
2. Add pure SemVer relation, malformed/key/version mismatch state, canonical release, monotonic history,
   payload mismatch, unknown-field, symlink publication, and native whole-home zero-write tests.
3. Implement create-once machine lifecycle records, typed compiled asset identity, single-handle reads,
   and read-only pathname inventory without claiming mapped-code identity.
4. Wire one preflight before setup, sync, and guided setup mutation, with a tested guided-entry seam.
   *The guided-entry seam is superseded by #118, which moved that check to the screen's install; the
   seam and its test are gone. Recorded here rather than rewritten, the way `design.md` records it,
   so one policy covers this directory.*
5. Update README, changelog, handoff, and structured change documentation.
6. Run formatting, focused tests, and every repository gate.
