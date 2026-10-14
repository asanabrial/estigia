section: The harness

- **Session start no longer warns about the build in every other project.** Issue #33. The build note was printed
  whenever the embedded commit is not an object of the repository the session runs in, which is every project but
  Estigia's own, and nothing there can be done about it. It is now silent in that case alone; a dirty build, a build
  with no commit, a directory with no head and a stale build in a checkout that holds its commit are still named, and
  `doctor`'s `gate-version` row is unchanged (`docs/honesty.md`).
