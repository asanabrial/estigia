section: The harness

- **Delegation is sized by reason, not by file count, and a declared `inline` topology is no longer refused.**
  Issue #68. The Direct Work Delegation writing row now delegates one writer only for a named reason, independent
  units or the parent's context limit, and a read batch inside the reading bound stays inline even when it prepares
  a write. `publish_review` and `republish_review` no longer refuse `topology-inline-over-threshold`; `--topology`
  and `--topology-reason` are still recorded on the marker and answered by `review_read` with `view: transaction`.
  The skill and the `developer` definition gain a fresh worker past about 150k tokens of parent context, the spec
  passed by reference, and one bounded correction batch.
