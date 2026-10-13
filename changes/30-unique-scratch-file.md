section: The harness

- **Two calls in one process no longer stage a file under the same name.** Issue #30. `paths::scratch_file` named a
  staged body by process id and the caller's name, so two threads staging for the same pull request or issue shared one
  path and the temporary beside it, and the later rename failed with `the body could not be staged`. It now adds a
  per-call counter. Nothing a run is told changes (`docs/honesty.md`).
