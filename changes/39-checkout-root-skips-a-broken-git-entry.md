section: The harness

- **A `.git` that is not a checkout no longer hides the one above it.** Issue #39. `checkout_root`, which finds
  the repository file from any subdirectory, took any `.git` entry for a checkout, so a dangling symlink, an empty
  file or a file that did not say `gitdir:` in a nested directory hid the real checkout above it and the
  repository's own settings stopped applying. Only a directory or a readable `gitdir: <path>` file counts now, and
  the walk goes on above anything else. The submodule test now writes a repository file for the submodule and a
  different one for its parent and asserts the submodule's is the one read (`docs/configuration.md`).
