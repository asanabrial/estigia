section: The harness

- **A slice that is not the last of its chain can start its branch unlinked.** Issue #31. `start_branch` takes
  `no_link: true` (`--no-link`), which skips the native link, so the first merge of a chain no longer closes the
  issue through the branch link. The answer reports `natively_linked: false` and `link_outcome: skipped`, queues no
  link while the schedule is closed, and the default still links (`docs/github-binding.md`).
