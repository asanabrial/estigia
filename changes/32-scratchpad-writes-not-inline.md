section: The harness

- **A write outside the claimed checkout is no longer counted as an inline write.** Issue #32. `decide_observing`
  answered a routine write to a path no covered checkout reaches (a scratchpad PR body) `Allow` inside the
  120-second renewal window and `Outside` after it, so `saw_new_write` recorded it only while the verification was
  fresh and the `SessionStart` line was noisy by timing. The out-of-claim check now runs before the window shortcut,
  so such a write is `Outside` at every age. Nothing is newly permitted: both answers skipped the tracker, and
  `Outside` stands aside where `Allow` pre-approved the call. A write inside the claimed checkout is still counted
  (`docs/honesty.md`).
