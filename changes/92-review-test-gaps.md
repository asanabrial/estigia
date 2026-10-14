section: The harness

- **Two test gaps carried from review are closed, and two defects they hid are fixed.**  Issue #92. `merge_cause` and
  `match_merge` now take a delivering SHA only when it is forty hex digits, not merely forty characters long, so a
  malformed SHA of the right length is named `merge-commit-unreadable` instead of blaming the second parent; the MCP
  test helper `function_body_in` pads each file with a leading newline, so a function on a file's first line is found.
  New tests pin the non-hex SHA and the catch-all cause, the function at the start of a file, a padded
  `--topology "inline  "` recorded trimmed on the marker, and the silence-window test now holds the GitHub binding to the
  horizon-or-silence wording and the activity kinds to the rule paragraph alone.
- **Three review test gaps are closed.** Issue #92. `create` with `repo` while the git window is closed is now tested to
  queue the issue under that repository and not the checkout's own, and to be filed by a `flush` from the target's
  checkout and by none other. `spawns_pi` is tested for its depth limit, a directory that cannot be listed, the three
  quote styles, every spawn call and the near-miss commands it must refuse. The `doctor` `permission` row is tested for
  an unreadable Claude Code or OpenCode file, for the OpenCode and Codex notes and for Claude Code's local, project and
  user precedence, and the existing row test finds its rows by their structured detail rather than by a substring of
  their Debug output. No behaviour changes.
