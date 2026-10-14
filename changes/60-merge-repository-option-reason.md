section: The harness

- **A merge that names a repository is refused for that option, not for a missing number.** Issue #60. A merge command
  carrying `--repo`, `--repo=` or a short option bundle containing `R` was refused as `delivery-pr-unidentified` with
  "does not name one positive numeric pull request", although the number was there. It is now refused as
  `delivery-pr-repository-option`, which names the option and says to run the merge from the checkout without it. The
  option is still not accepted, even for the run's own repository: the gate reads one spelling (`docs/honesty.md`).
