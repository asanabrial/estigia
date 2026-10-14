section: The harness

- **The skill says how long a claim outlives silence.** Issue #35. `runtime-notes.md` now states that a claim lives to
  its horizon or four hours past the run's last activity marker written since the claim, whichever is later (four
  hours past the last marker when it has no horizon), and asks for a covering horizon or
  an orchestrator heartbeat at least every four hours before a longer phase, since a delegated context renews nothing.
  `skill/bindings/github.md` no longer limits the four-hour window to horizonless claims, and a test in
  `tests/honesty.rs` ties the figure and the activity kinds to `SILENCE` and `ACTIVITY_KINDS`; the two files' byte ceilings and the figure in `docs/honesty.md` moved with them.
