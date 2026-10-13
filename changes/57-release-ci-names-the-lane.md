section: The harness

- **`release_ci` says whether CI will run.** Issue #57. It answered the same fixed payload, `ci_released: true`, in a
  repository where no CI can run, so the agent read it as "CI started" and watched checks that never came, while
  `review_status` already knew the lane was `not-consulted`. `release_ci` now carries the publication lane reading
  `review_status` gives, from the same function, and an explicit `next`: `merge` when the lane is `not-consulted`,
  `disabled` or `never-ran`, `ci_status` otherwise, an unreadable lane included. `ci_released` is `true` only when
  `next` is `ci_status`; the fields are added, none removed. The `released` continuation of `review_status` names the
  same branch instead of "when CI is green". `skill/bindings/github/delivery.md` states the branch at the
  `release_ci` to `ci_status` hand-off and decides the question the issue raised: a `Publication lane` row set to `off`
  waives the issue comment the `disabled` rule asks for, because the row is the repository's own recorded decision and
  the comment records an observation the run made, which a lane that was not read does not give. A head whose check runs have not
  registered yet right after the ready mark reads `absent` and goes to `ci_status`. The `delivery.md` ceiling in
  `tests/instruction_ceiling.rs` and the figure in `docs/honesty.md` moved from 38,021 to 38,654 bytes by the words
  that state the branch and the decision.
