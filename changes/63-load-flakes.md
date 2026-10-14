section: The harness

- **Three test fixtures no longer fail under load for reasons the product does not share.** Issue #63. A scripted `gh`
  inherited the stand-in reviewer's launch variables from a concurrent test and answered nothing, and two review
  launches were held to a wall-clock bound that machine load exceeds. The scripted `gh` now drops those variables unless
  its fixture sets them, and both launch tests wait on a file instead of the clock. No behaviour a run sees changes
  (`docs/honesty.md`).
