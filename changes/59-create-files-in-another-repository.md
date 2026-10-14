section: The harness

- **`create` can file an issue in another repository.** Issue #59. A session working in one repository had no
  supported way to report a finding in another: the `create` tool always filed against its own checkout's
  repository, and a bare `gh issue create` is refused with `issue-creation-outside-flow`. `create` now takes an
  optional `repo` (`owner/name`, refused as `repo-ref-malformed` when it is anything else, before the body is read
  or anything is filed). It files there under the same template, label and identity rules, records the issue in the
  local store under that repository, and starts it unclaimed; no claim crosses repositories. The refusal of a bare
  issue filing now names `create` with `repo` as the way to reach another repository. The `tools/list` ceiling and
  the byte ceilings of `skill/SKILL.md` and `skill/bindings/github/operations-guarantees.md` rose by the text that
  names the argument (`skill/bindings/github/operations-guarantees.md`, `docs/configuration.md`).
