section: The harness

- **The reviewer CLI keeps `Path` and `SystemRoot` on Windows.** Issue #11. The reviewer child's environment
  allow-list matched names case-sensitively, so a CLI launched on Windows started with no `PATH` and no
  `SystemRoot`, and `PATHEXT` and `COMSPEC` were never on it. It now uses the tracker transport's own
  case-insensitive list for those names, which adds `PATHEXT`, `COMSPEC`, `SYSTEMDRIVE` and `WINDIR`; `GH_TOKEN` is
  still dropped and `OPENAI_API_KEY` still kept (`docs/honesty.md`).
- **Most of the suite now passes on Windows; the lane still skips it.** Issue #11, first slice. The fixtures that
  assumed POSIX paths, shells and separators go through `test_env::shell_path` and `test_env::outside_the_checkout`.
  On a Windows runner every integration target now passes and 13 lib tests still fail, down from 55, so `ci.yml` and
  `release.yml` keep skipping the Windows test step until those are settled (`docs/honesty.md`).
