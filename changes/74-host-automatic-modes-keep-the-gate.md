section: The harness

- **A host's automatic mode keeps Estigia's gate, tested and reported.** Issue #74. Nothing pinned that Claude Code's
  `bypassPermissions` and `auto`, OpenCode's `--auto` and Codex's `never` approval policy leave Estigia's refusals and
  questions standing, so a host that ordered its permission layer before hooks, or an Estigia change that answered
  `allow` where it stands aside, would have gone unseen. Tests now hold the hook answer on Claude Code and Codex, and the
  OpenCode plugin's throw in both loader generations, equal with and without the mode's fields. `doctor` gains a
  `permission` row per configured Claude Code, Codex, OpenCode and Pi, reading the mode from the host's settings files and
  saying so when it cannot read it; it is never broken. No gate decision changes. `docs/honesty.md` records the live
  checks (OpenCode, Claude Code, Pi) and the one not run (Codex, not signed in).
