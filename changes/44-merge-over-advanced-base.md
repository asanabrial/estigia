section: The harness

- **An approved receipt stays live when the base advanced cleanly.** Issue #44. A receipt was recognised only while
  the pull request's `baseRefOid` was the reviewed base, so each merge to `main` sent every other approved, green
  pull request back through publish, review and CI. `review_status` and the claim's restoration of the receipt now
  keep it live when the head is the reviewed head, the pull request is ready, the current base descends from the
  reviewed base and GitHub reports it `MERGEABLE` with a `mergeStateStatus` that is not `DIRTY`; the answer is
  `released` and names the new base. A mergeability GitHub is still computing is answered `mergeability-unknown`,
  never clean. `release_ci`, a draft, and an unapproved receipt stay strict about the base. The accepted risk, that
  two independently green changes are not tested together before the merge, is in `docs/honesty.md`; the `merge` step
  of `skill/bindings/github/delivery.md` states the rule, and three `tests/instruction_ceiling.rs` ceilings moved
  by the words it took.
