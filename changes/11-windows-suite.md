section: The harness

- **The reviewer CLI keeps `Path` and `SystemRoot` on Windows.** Issue #11. The reviewer child's environment
  allow-list matched names case-sensitively, so a CLI launched on Windows started with no `PATH` and no
  `SystemRoot`, and `PATHEXT` and `COMSPEC` were never on it. It now uses the tracker transport's own
  case-insensitive list for those names, which adds `PATHEXT`, `COMSPEC`, `SYSTEMDRIVE` and `WINDIR`; `GH_TOKEN` is
  still dropped and `OPENAI_API_KEY` still kept (`docs/honesty.md`).
- **The suite runs on Windows.** Issue #11. `ci.yml` runs `cargo test` on `windows-latest` and `release.yml` runs it on
  the Windows targets, after the fixtures that assumed POSIX paths, shells and separators were made portable
  (`test_env::shell_path`, `test_env::outside_the_checkout`).
