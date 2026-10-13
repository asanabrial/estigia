section: The harness

- **`done` verifies the merge.** Issue #34. `transition --to done` on an issue with a confirmed review receipt reads
  the receipt's pull request and refuses `delivery-not-verified` unless its merge commit has the reviewed head as second
  parent and a first parent that descends from the reviewed base; the answer carries the merge commit and parents. With no
  receipt it behaves as before (`docs/github-binding.md`).
