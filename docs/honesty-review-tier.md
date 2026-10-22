# The review tier, the lens and the finding

- **The tool-answer shrink is reconstructed for most of its rows, not captured from a running
  pre-change binary.** `docs/review-cost.md`'s "What a tool answer costs" measures four scenarios
  two ways: *reconstructed* — the exact JSON shape the pre-edit and post-edit source builds,
  assembled with representative field values and sized with a compact encoder byte-identical in
  output shape to `serde_json::to_string`'s own — and *measured* — a real `cargo test -- --nocapture`
  run of a named test against this change's actual code. Only the *after* side of the shrink can be
  measured directly: the pre-edit code no longer exists in this checkout to run once the edit lands,
  so every *before* figure and the one repair-publication scenario are reconstructed rather than
  captured live. The two methods agree closely where both exist for the same shape (616
  reconstructed against 610 measured for an equivalent first-publication answer), which is the
  evidence offered that the reconstruction is trustworthy — not a substitute for having run the
  old code.

  **The re-measured transcript corpus is compared against the baseline, corpus-to-corpus.** The acceptance criteria asked for a comparison against the 2026-09-24 baseline, and
  `docs/review-cost.md`'s "Re-measuring after a change" now draws it. The limit that section states
  is the one that survives here: `scripts/measure-transcripts.py` aggregates the whole growing
  corpus, so the comparison sets a later corpus against the baseline rather than measuring this
  change alone, and nothing in it separates a post-change session from a pre-change one. That
  section carries the figures and the limits it records beside them; this entry records only that
  the comparison is drawn, and that its aggregation is what it costs.

- **The usage classes are one CLI's counters, and the fixture behind them is synthetic.**
  `scripts/measure-transcripts.py` reports `input`, `cache_read`, `cache_creation` and `output`
  separately, and the mapping it uses is `CLAUDE_USAGE_KEYS` — Claude Code's own keys. No other
  provider's transcripts are read by this script at all, so the classes say nothing about Codex, Pi,
  OpenCode, native reviewers or auxiliary requests, and a provider whose cached counters already sit
  inside its input total would need a mapping that does not exist yet. A class no measured record
  carried is `null` and never `0`, and `usage.records_missing_a_counter` counts the records lacking
  any of the four; on the corpus read on 2026-10-18 no record was missing a counter, so both
  branches — the unknown class and the floor — are exercised only by the hand-written fixture in
  `tests/usage_accounting.rs`, not by live data. That fixture is not host evidence: it is
  Claude-shaped JSONL written so that a total is checkable by hand. The parity the same corpus shows
  against the base script covers the fields the classes do not reach, and says nothing about whether
  the classes are the ones a cost comparison needs.

- **A session's span, turns and models are what its transcript says, not what was billed.**
  `wall_clock_seconds` runs from a session's first timestamped record to its last, subagents
  included: it is not *time to a verified delivery* unless the session ended at that delivery, and
  idle time inside it is counted. `served_models` is the model the API response names and
  `requested_models` the one Claude Code recorded asking for; neither is a billing record, and a
  record that names neither is counted `unknown`. A turn is a distinct assistant message identity,
  and a tool call a distinct `tool_use` identity, so a retry the CLI recorded under a new identity
  counts again. Codex, Pi and OpenCode transcripts are not read at all.

- **A `never-ran` verdict is recorded evidence, not verified evidence.** Read through `record_computed_verdict`: it approves under a publication lane whose failing
  runs report `steps: []` on every job only when the caller supplies `--never-ran-evidence-file`,
  and it stores that file's content in the marker verbatim — on the same footing as a finding's
  `evidence`, read back and never re-run. Estigia does not confirm the account is actually billing-blocked, does not re-run the
  local check the file describes, and does not confirm the jobs listing it read at classification
  time is the same one the caller looked at. `steps: []` proves only that GitHub's own jobs API
  reported no steps for that run at the moment Estigia asked; a caller who typed anything into the
  evidence file — including a false claim — clears the same gate a true one does. This is the same
  honest limit `evidence` and `impact` state for a review finding, extended to the one place a
  publication lane cannot answer at all.

  The annotation this classification reads (`repos/<owner>/<name>/check-runs/<id>/annotations`)
  is carried into the refusal for the same reason: it is what tells an operator, in GitHub's own
  words, why nothing ran. It is never the predicate — the branch is `steps: []` alone — because an
  annotation is prose a platform can reword without notice, and a gate that changed meaning when a
  product it does not control edited a sentence is the failure this repository has already found
  once, when a stop was decided by a bounded phrase list rather than a marker.

  A run cancelled before any step starts reports the identical `steps: []` shape a genuine billing
  failure does — the one way the run being judged, not a platform failure, could reach this branch
  for itself. `classify_reds` excludes a `cancelled` conclusion before it ever asks a run's jobs
  anything, so that specific path is closed rather than merely disclosed. What is not closed is
  every other conclusion GitHub reports: the branch still trusts `completed` with a non-green,
  non-`cancelled` conclusion and `steps: []` at face value, and does not itself distinguish a
  platform outage from some other way a run's containers might report no steps.

- **Nothing detects a launch prompt that overrides the judge's classification rule.**
  `skill/agents/review-blind.md` holds the rule for choosing between `severe`, `warning` and
  `suggestion`, and states why it lives there rather than in a launch prompt. A prompt that supplies
  its own rule anyway replaces it silently. The gate has no decision to make here — a sub-agent's
  prompt is not a write — and no test reads one. The repair is an instruction to the judge to prefer the definition
  and record the conflict, which is enforcement by the reviewed party alone: a judge that ignores the
  instruction leaves no trace, and Estigia cannot prove a judge applied the definition rather than the
  brief. It is worth stating what this cost before it was found: in one review, four `severe` findings
  across two panels were documentation defects graded under a prompt-supplied rule, and the two
  rejections they forced spent a full panel of tokens each — the figure is not re-derived here
  because the panel receipts were not retained with a method a later reader can re-run.

- **`review_tier` carries two named gaps against `ClassifyRisk` (a third, the process-boundary
  scan, was closed since), and one widening.** It
  ports the hot-path tokens, `isServiceTokenReviewPath`, `isShellReviewPath` and
  `executableModeChanged` — four producers reaching `auth`, `update`, `security`, `payments`,
  `permissions` and `shell_process`.

  The first gap, **`processBoundaryRiskReasons`**, is closed
  (`src/transport/tier/content_scan.rs`): a content scan over the files a change touches reads
  `high` on an added process-spawn construct (`process_boundary`, signal `shell_process`), on a
  `#!` line in either tree (`process_boundary`), on a scan past `SCAN_BYTE_BUDGET` or a diff whose
  paths cannot be attributed (`process_scan_limit`), and on the ten dangerous-sink families of
  `risk_dangerous_sink.go` (`dangerous_sink`). Where it still differs from the source project:
  - **Matching.** The source project's Go regexes and its `git grep -E` for the shebang become the
    `regex` crate and a line test over the blob. Every catalogue pattern is carried over unchanged,
    but `\b`, `\w` and `[^...]` are Unicode-aware here and ASCII-only in Go, so a non-ASCII
    identifier can match here where it would not there. A blob containing a NUL is skipped,
    standing in for `git grep -I`.
  - **Interpreter directive.** Only the first line of a blob, in either tree, is read, and a first
    line starting `#![` is not a directive. The source project runs `git grep '^#!'`, which matches
    any line of the whole file; that was built for Go, where `#!` never starts anything but a
    directive. Here it made every change touching a Rust file with an inner attribute
    (`#![deny(...)]` in `src/lib.rs`) read `high` — the false positive `docs/honesty.md` already
    names in the source project's own scan. The cost is a directive on a later line, or a script
    whose first line is `#![`, which this port does not read.
  - **Signal.** A sink reason carries its family (`tls_verification`, `unsafe_deserialization`,
    `eval`, `shell_composition`, `weak_credential_hash`, `credentialed_cors`, `world_writable`,
    `unverified_jwt`, `csrf_opt_out`, `xxe`) as its `signal`, where the source project writes
    `dangerous_sink` for every one of them. The scan reports each family once per path, but
    `canonical_risk_reasons` keeps one reason per `(code, signal)` for the whole change, so the
    published list names a family once, on whichever path sorts first.
  - **Bound.** The blob-size total is bounded before any blob is read, as there. The added-line
    diff is bounded only after it was read, not while it is read, and it is not bounded by the
    blobs: each added line costs a `+` and a newline, so a diff can exceed the blob total. One
    over the 8 MiB budget answers `process_scan_limit`, and so does a diff header whose path
    cannot be attributed to a scanned file.
  - **Unreadable blobs.** A blob or diff `git` cannot read is a read failure and no tier is
    answered; the source project's own reads fail the whole assessment the same way. Only the
    over-budget and unattributable cases answer `high`.
  - **Test paths.** Test paths are not scanned, so a spawn or a sink added only in a test reads
    nothing. The rules are the source project's — a `test`, `tests`, `__tests__`, `testdata` or
    `spec` segment, a `_test.go` or `_test.py` suffix, a `.test.` or `.spec.` infix, a `test_*.py`
    name — plus two this port adds for Rust, where unit tests live beside the code: a basename of
    `tests.rs` and a `_tests.rs` suffix.
  `data_exposure` and `data_loss` are declared in the same eight-name closed signal set but neither
  is produced anywhere reachable: `ClassifyRisk` has exactly one production call site in the source
  project, `risk.go:293`, and it passes signals computed by `risk.go` itself — nothing outside this
  file supplies either one, and grepping the whole source tree finds them only inside two `switch`
  statements that render a signal's meaning for display, never one that produces it. They appear to
  be dead in the source project too, which this port does not attempt to correct or match; their
  absence here is not a narrowing of anything reachable, only of the eight-name set nominally
  available. A prior version of this entry claimed they arrive from a caller outside this file —
  unsupported, and cut rather than repeated.

  `isStaticMDXDocument` also drops the source project's per-line bound: `risk.go` scans with a
  `bufio.Scanner` capped at 1 MiB per line and answers `false` (not static) on overflow; this port
  reads the whole decoded string with `str::lines()` and has no equivalent per-line cap, relying only
  on the 8 MiB total content-scan budget every passive candidate already reads behind
  (`SCAN_BYTE_BUDGET`). A single `.mdx` line between those two limits is scanned in full here and
  would be refused there — fail-open relative to the source project, bounded by the total rather than
  the line, and named rather than ported: the total budget is what makes every passive-content read
  in this port bounded at all, and a second, narrower bound for one extension is not this port's
  standing practice for any other producer.

  This gap runs the opposite direction from the other two: it narrows what reaches `medium`, not
  `high`, and it costs a reviewer rather than bytes. A single-line `.mdx` document over 1 MiB is
  refused as non-static by the source project's own bound (two spans of `risk.go`,
  `scanner.Buffer(make([]byte, 64*1024), 1024*1024)` and the `scanner.Err() == nil` it returns on
  overflow) — proven not passive there, so the source project answers `medium` and one reviewer.
  This port has no per-line bound, so the same document is scanned in full and can be proven
  passive, answering `low` and zero reviewers. That is a missed reviewer, not a bytes-read cost.

  The remaining gap is not a narrowing this port chose: **the source project's own six-path
  self-reference, `isNativeReviewAuthorityPath`, has no counterpart here at all**, and this was
  measured rather than assumed. A first attempt substituted this repository's own gate files
  (`src/harness/mod.rs`, `src/harness/guard.rs`, `src/harness/roles.rs`, `src/harness/tracker.rs`,
  `src/transport/claim.rs`, `src/transport/ownership.rs`) for the source project's six, on the theory
  that a small set of rarely-touched files carrying this repository's own review authority would work
  the same way. Run against three real deliveries of this repository, it answered `high` for all
  three, every one on the same file: `src/transport/claim.rs`. That module is 7,163 lines and was
  touched in 32 of the repository's last 40 commits — `src/harness/mod.rs`, the next busiest of the
  six, in 24 — so the substitution made the classifier a constant function on this repository's own
  history, which is the same as having no classifier. The fix is not a shorter list: dropping
  `claim.rs` for being touched often would exclude the file holding `record_computed_verdict` and
  `release_ci`, optimising the metric against the exact safety the rule exists to protect. The Go's
  version works because that project's review authority lives in six small, rarely-touched files;
  this repository's does not, because it is not yet separable by filename — splitting
  `src/transport/claim.rs` and its neighbours is what would let this producer be ported faithfully.
  Until then the producer is removed rather than shipped as a rule that always answers `high`: a
  classifier that cannot say no is not a classifier, and this repository would rather carry a named
  gap than a rule nobody could trust the reading of. This gap also narrows what reaches `high`: a
  file the source project's own six-path self-reference would escalate on, this port does not.

  One property runs the other way, and it is a widening rather than a narrowing:
  `isGeneratedGoldenPath` gates a generated `testdata/golden/*.golden` fixture out of risk
  classification at seven call sites in the source project, not the two a prior version of this
  entry named. One in each of `CountChangedLines`, `AssessSnapshotRisk`,
  `activePassiveContentPaths`, `deriveSnapshotRiskReasons`, `fallbackRiskReason`,
  `isInspectableStat` and `touchesHotPath` in `risk.go`, plus `SnapshotBuilder`'s
  own build step marking `DiffStat.Generated` for the same paths (`snapshot.go`).

  All seven have counterparts here — `count_changed_lines`, `review_tier` itself,
  `active_passive_content_paths`, `derive_snapshot_risk_reasons`, `fallback_risk_reason`,
  `is_inspectable_stat` and `touches_hot_path` — and not one of them gates on a golden path,
  because this repository has no established convention for that shape of file. Only
  `is_inspectable_stat`'s own doc comment records the omission; the other six are silent about it.
  A path shaped like the source project's golden fixture answers `high` here and would not there.
  It never turns a `high` answer into a `low` one, but it is the one place this port over-fires
  rather than under-fires. That conclusion stands; the two-call-site count behind it did not, and
  is corrected here.

- **`review_tier`'s tier classification carried three blind spots. Two of them were closed by
  deleting the mechanism they were about; one of those two was reopened in a different shape,
  and the third survives untouched.**

  It reads two tree-to-tree `git diff` calls and never the worktree — the two `pinned_diff_args` calls
  in `src/transport/tier.rs`, not the doc comment paraphrasing them: both ends
  are resolved to commits, so uncommitted work is invisible to the tier that the diff derives. That
  cost is not paid on the routed path: the orchestrator reads `review_tier` only
  after `publish_review`, and `publish_review`'s `clean_target` (`src/transport/target.rs`) refuses
  to publish while any worktree byte is uncommitted, so by the time the
  tier is derived there is nothing uncommitted left for it to miss. The blind spot is real for
  `review_tier` itself, called directly or against an arbitrary worktree; the routed path does not
  reach it. This one is not about the deleted `judges` field and does not close with it — it is a
  property of the tier classification `lenses` still reads today.

  **Closed with the deletion, and the later removal of `RiskLevel::judges` leaves it closed:**
  the old `RiskLevel::judges` was unable to distinguish an operator's own `Blind judges` row from a
  rendered default — `Context::live`'s unconditional `render_rows()` merge at
  `src/transport/mod.rs` folded in every modelled setting's default when nothing was
  written, so a repository whose contract could not be read was sized as though the operator had
  chosen `single`. The ambiguity needed a row to misread, and the row is gone; the judge count
  that was derived from the tier alone is gone with it, since ordinary review licenses lenses only.

  **The ceiling half closed the same way, by reading provenance instead of deleting the
  row, and the row was then retired.** `repair_rounds` read `Repair ceiling` through
  `Context::get_written` (`src/transport/mod.rs`), which answers only rows somebody wrote, so a
  `Context::live` over directories holding no contract answered `null` where it had answered the
  default `3`. The setting is gone and `repair_rounds` reads no setting at all, so `get_written` has no
  caller left; it stays, unread, until a separate change removes the provenance machinery with it.
  Every other modelled row read through `Context::get` — `worktree location`, `Review budget` — still
  reads a rendered default the same as a written answer, by the same merge
  this entry names above.

  **The third bullet was closed: the tier-licensing sentence no longer
  lives in `skill/SKILL.md`.** Developer step 5 now links to `skill/policies/blind-judges.md`, whose
  "Lens acquisition" section states the rule and is crossed by
  `the_lens_count_a_publication_acquires_is_the_one_review_tier_answers`. The rendered carriers are
  crossed (`the_two_src_only_carriers_render_the_lens_counts_the_tier_derives_and_no_judges` reads what
  `opencode_review_section()` and `selected_documents()` render against `RiskLevel::lenses_spelled()`),
  but the contract sentence itself is held by review alone. The same bullet's settings half is closed
  by deletion: the typed-count block `src/config/settings.rs` no longer exists, so there is no
  wording left to revert and no pin to write.

  **Closed, and later widened: the floor is gone and so are the
  judges.** `RiskLevel::lenses` answers `Low => 0`, as the origin project closes a `RiskLow` review
  with no reviewer. `review_tier` and `review_tier_for_repair` answer no `judges` key at any tier, and
  `forecast` counts lenses alone; `the_review_tier_answer_carries_no_judges_at_any_tier` pins that,
  and `no_text_an_agent_acts_on_for_ordinary_review_acquires_judges` holds the contract, its rendering
  and every other text under `skill/` to it. Judges exist only as Judgment Day, which licenses
  nothing in the transport and which `skill/policies/judgment-day.md` states as an explicit-request-only method that
  records nothing. That a Judgment Day finding is never recorded with `record_review_finding` is
  stated there and **not enforced**: nothing refuses it, and a finding recorded against a receipt
  would enter the ledger like any other.

- **`RiskLevel::lenses` scales the review; nothing proves the licensed lenses actually ran.** `review_tier` was given a count derived the same way `lenses` already was; the
  judge half was later removed and what remains is the lens count, and the property below is unchanged by it. The entry immediately below names the same unproved property one layer earlier: which range a caller
  reads before that count is even derived.

  **Nothing observes that the derived number of lenses actually ran.** `release_ci` requires the
  computed verdict at every tier, and the verdict refuses `approved` without a lens report for every
  role the tier licenses, but a run records that report on the lens's behalf and cannot be shown to
  have launched it (the lens-report entry in `docs/honesty.md`). A `high` change reviewed by two lenses instead
  of four is indistinguishable, at the transport, from one reviewed by four unless a report is
  missing.

  `tests/honesty.rs`'s `no_carrier_of_the_tiers_allocation_states_a_judge_count` proves the carriers —
  the two MCP tool descriptions, `skill/SKILL.md`, `skill/policies/blind-judges.md`,
  `skill/references/runtime-notes.md`, `skill/bindings/github.md`,
  `skill/bindings/github/operations.md`, `README.md` and `docs/configuration.md` — no longer state a
  per-tier judge count. It does **not** prove an orchestrator reads any of them before acquiring the
  lenses.

- **An empty range answered the zero tier, and it was reachable today, not latent.**
  `review_tier` with `base == head` — or any pair whose diff touches no path at all — read as `low`
  with zero lenses, measured directly against the shipped binary rather than assumed from the code.
  `review_tier` is read-only and requires no claim, so any caller could reach this by passing one
  commit twice. The mechanism was the same vacuous-truth shape `review_tier_for_repair`'s own
  `empty-repair-range` fallback (immediately below) already had to guard against: `classify_risk`'s
  `only_passive` is `stats.iter().all(...)`, true over an empty slice with nothing to falsify it, and
  `fallback_risk_reason` fell through its own loop over zero stats to `non_executable_only` — a code
  that names a change *measured and found* to touch only non-executable, passive-shaped paths, said
  here of a change that was never measured at all. The fail-closed cross-check
  (`risk_level_from_reasons`) did not catch it, because it maps a lone `non_executable_only` to `Low`
  too, so the tier and the reason that was supposed to explain it agreed — both wrong together.

  The fix adds a fifth fallback reason code, `nothing_measured`, distinct from `non_executable_only`: emitted
  only when `stats` is empty, at the one place `fallback_risk_reason` already falls through to a
  reason with no stat left to name. `classify_risk` gained one explicit `stats.is_empty()` branch
  answering `Medium` — never `Low`, since nothing was proven passive, and never `High`, since nothing
  was proven risky either — so an empty range now answers `medium`, one lens, with a reason a reader
  can tell apart from a measured `medium` at a glance. No non-empty range's classification changed:
  that branch is reached only for the stat set an empty diff alone produces.

  **`risk_level_from_reasons` has no such branch, and the gap that leaves is measured rather than
  argued.** One was written and deleted. It read `if reasons.len() == 1 && reasons[0].code ==
  REASON_NOTHING_MEASURED { return RiskLevel::Medium; }`, immediately above a fallthrough that
  already returned `Medium` — so it changed the answer for no input, and its removal was undetectable
  by the suite: deleting it and running `cargo test --lib transport::tier` answered 42 passed at the time. Both
  judges on this change found it independently, one from the diff text and one by enumerating that
  private function's three call sites. It was removed rather than pinned with a test, per this
  repository's preference for removing a copy over adding a check — and because a branch whose
  presence is indistinguishable from its absence is the same shape as the vacuous predicate this
  change exists to refuse, added inside the repair for it.

- **A repair's tier is derived from a range that is honest only when the timeline can prove it —
  and nothing verifies that the orchestrator reads that range rather than the whole change.** `review_tier` answered `base..head` on a repair round exactly as it did on a first
  publication, so a repair that only corrected sentences was sized as the code change it sat on top
  of — measured as the largest single cost in this repository's own review budget (8 judges,
  3 rejections, zero code defects, on prose alone).

  **The first design was wrong, and the panel reviewing the pull request caught it before it
  merged.** That design, `review_tier_over_trees`, read two frozen tree object ids — the parent
  publication's own candidate tree and this publication's own, both recorded unconditionally by
  every publication's `budget-candidate-tree` field (`src/transport/claim.rs`) — on the belief
  that a frozen id excludes base movement. It does not: a candidate tree is the tree *of a commit*
  (`tree_of(context, ..., head)` at `claim.rs`), and `git diff tree(A) tree(B)` is the same
  diff as `git diff A B`, so that pair *was* `parent-head..head` under a different name — exactly
  the range an earlier measurement already showed folding `main`'s own advance into what looked like the
  repair's own size (1,205 insertions where the repair was 562, on a real repair).
  Freezing the id never froze the range; after a rebase the repair head's own content already
  carries the base's advance regardless of which object ids name the ends.

  **The replacement, `review_tier_for_repair`, answers honestly instead of freezing wrongly.** It
  takes commit-ish arguments and an ancestry guard: when the parent publication's own head is
  still an ancestor of this one, nothing rewrote history under the repair, and the answer is the
  narrower `parent-head..head`. When it is not — the base moved and the branch was rebased onto
  it — there is no repair-only range left to recover from what the timeline records, and the
  answer falls back to the whole `base..head`.

  **The panel reviewing the pull request rejected that first shape a second time, on two more
  defects the ancestry guard alone did not close.** `git merge-base --is-ancestor A A` exits `0`,
  so an identical-bytes republish — `parent_head == head`, a real input the contract itself names,
  since a republish invalidates evidence "even when the bytes are identical" — makes
  `parent_head..head` an empty range that the guard alone waves through: a probe over a
  `src/auth.rs` change answered `high` with five lenses over the whole range and `low` with zero
  lenses over the reflexive one, because an empty diff yielded no candidates,
  `fallback_risk_reason` answered `non_executable_only`, `only_passive` was vacuously true over an
  empty slice, and the fail-closed cross-check did not fire — level and reasons agreed, both wrong
  together. A later change replaced that reading: an empty stat set now answers
  `nothing_measured`, and `review_tier` answers `medium` for such a range rather than deriving
  nothing from it. The fix here is not to special-case that one input: an empty
  `parent_head..head` carries no evidence about the repair's own edits, so it falls
  back to the whole `base..head` the same way a rewritten history does. Separately, the
  `base_advanced` boolean the first shape answered with was false in two further histories: an
  amend with no base movement reported `true`, and a base advance *merged* rather than rebased
  into the branch reported a clean `false` while `parent_head..head` still carried the advance,
  because `parent_head` stays an ancestor across a merge. A boolean claiming a base movement the
  code never tested cannot be patched into honesty, so it is gone; the answer instead names the
  range it actually measured — `range: "repair"` or `range: "whole-change"` — and, only in the
  latter case, `whole_change_reason`: `"history-rewritten"`, `"base-advanced"`, `"merged-range"` or
  `"empty-repair-range"`. In the
  narrow case the answer's `"base"` field carries `parent_head`, not `base`, since that is the
  range actually measured. And the earlier claim that this guard "mirrors" `correction_budget_check`
  is also gone: that function tests `is_ancestor(merge_base(base, head), parent.head)`, different
  operands answering a different question, and the two disagreed on exactly the merge-from-base
  case above. A later change closed that case by adding the fork-point test to `repair_range`
  after the ancestry guard: a base advance merged into the branch is `whole-change` with
  `whole_change_reason: "base-advanced"`, which the budget's `base_advanced_past` reads the same way
  from the parent's base, so a one-lens panel is never granted over content the base wrote. A merge
  commit in `parent_head..head` whose fork point is still inside `parent_head` — a side branch
  merged back with `--no-ff`, or a caller passing a base older than the one the head merged — is a
  different shape and answers `whole_change_reason: "merged-range"`, also read whole and never
  granted the delta lens. The guard here stands
  on being correct, not on resembling another one.

  **So the saving does not apply to a rebased repair round, and that bound is not to be softened.**
  Branch protection here forces an integration whenever `main` advances during review; a run that
  rebases rewrites the parent head and falls back to the whole change, and one that merges
  the base also falls back to it, as `base-advanced` — and the forced integration
  over an approved parent is now publishable either way, where before the elective gate left the rebase
  as the only route for that round. The
  narrower tier is available exactly when no rebase happened since the parent publication, and the
  operation admits plainly when it is not rather than guessing narrower than the change actually
  was.

  What this still does not close: **nothing verifies that an orchestrator calls
  `review_tier_for_repair` for a repair round rather than `review_tier` over the whole change**, the
  same gap the entry above names for the judge count itself. `skill/policies/blind-judges.md` and
  `skill/bindings/github/operations.md` state which range to read and when; nothing in this transport reads it
  back, requires it, or can tell the two calls apart from their answers alone — a caller that
  ignores the rule and reads `base..head` on every round gets a well-formed answer either way, only
  a larger one than the rule intends.

  **And the range is necessary rather than sufficient, measured on a repair this repository ran the
  same day.** Run `claude-57cb079c6abe0a87` reported a repair round, delivered commits
  `d75407ff..31b2df54`: seven files, fifteen insertions, fourteen deletions, three of them `.rs`, two
  of those carrying only `///` comments. Filtering every comment line out leaves **two** changed
  lines in the whole repair — a string literal in `src/setup/tests.rs` whose content *is* a shipped
  paragraph byte for byte, and a registered byte count in `tests/instruction_ceiling.rs` whose only
  content is that paragraph’s size. Neither is behaviour, both are `.rs`, so that repair would
  classify `medium` over its own range too.

  The cause is this repository’s own guards: `the_default_rendering_is_the_bytes_already_installed`
  crosses a shipped paragraph against a byte-identical constant, and
  `documents_stay_under_their_registered_byte_ceilings` ratchets its size. The two mechanisms that
  make a prose edit trustworthy here are the two that stop a prose repair looking like prose.

  **The exposure is bounded, and the bound is measured:** `tests/instruction_ceiling.rs` registers
  `skill/**` and nothing else — no `docs/` path and not `README.md` carries a ceiling — so a repair
  editing shipped contract text is doubly policed where one editing `docs/` prose is not, and only
  the first is dragged out of `low` this way. What share of real repair rounds falls on each side is
  **not measured**, and nothing here claims one.

  Whether a string literal reconstructing a shipped document, or the registered size of one, should
  count as passive is left open deliberately. Widening `is_passive_document_extension` to admit
  either would carve a hole in the classifier to make an answer come out right, which is the shape
  `CLAUDE.md` refuses; the classifier is believed here, not changed.

  **The adjacent gap was closed, one level up from this classifier: the range a re-review is
  told to read, not the tier derived from it.** `parent-head..head` is what every shipped surface
  named unconditionally as the delta a re-review reads, and after a rebase that range also absorbs
  everything the base gained in between — measured on a real repair, epoch
  `48efa712a453694413b0614f2933c37b`: `parent-head..head` answered 14 files/1,205 insertions/103
  deletions where the repair itself was 10 files/562/26, `git merge-base --is-ancestor` on the two
  heads exiting `1`. The decision above — ancestry, then whether the narrow range touched a path —
  is now [`repair_range`](../src/transport/tier.rs), one function `review_tier_for_repair` calls
  rather than keeping its own copy, and `publish_with` calls the same function to record which case
  this publication landed in: `delta-range` carrying `parent-head..head` verbatim when it is honest,
  `delta-range-reason` naming `history-rewritten`, `base-advanced`, `merged-range` or `empty-repair-range` when it is not, on both the
  marker and the answer. What this still does not do: compute the diff, hand it to a judge, or
  restrict what one reads — the same limit the fix-delta-digest entry in
  [`honesty.md`](honesty.md) states, now with a recorded field to check that limit against instead of
  an unqualified claim.

- **The issue identity is readable for open `analysis` findings only, and the collision is still
  not mechanised.** The identity was taken off the issue title — `create` had composed
  `format!("{identity}: {title}")` for every issue this tracker holds — and put it in the `analysis`
  control marker instead, leaving the key
  [`domain-composition.md`](../skill/references/domain-composition.md) calls the one on which
  equivalent findings collide on a surface no operation returned. `list_identities`
  reads it back from the local store's `issues.identity`, which `create` and the lazy import
  recorded from the `analysis` marker and never from the title, with the open issues from one REST
  list: `null` for an issue that recorded none. What it does
  **not** do, and nothing here claims it: read a closed issue's identity, since the listing is
  `state=open`; read the `review` marker an issue filed with `--epoch` carries, which is recorded in the store
  and not rendered into the body and which nothing reads;
  or refuse a `create` whose identity already exists, which the issue left out of scope. So an
  analyst can now dedup by the recorded key rather than by reading titles and guessing, and the
  collision itself remains the analyst's to judge.

- **A lens is a sealed reader with one narrow question, not a third judge, and sealing it costs
  nothing where sealing a judge cost two severe findings.** That gap was measured: a sealed
  judge is a full-scope reviewer denied its sight, and the two severe findings it missed both rested
  on files no hunk touched — the seal removed evidence the judge's own scope still claimed to cover.
  A lens has no such scope to lose. Its one question — *does any sentence this change adds
  contradict any code this change adds?* — is answerable entirely from the diff by construction: a
  contradiction between a claim and the code that makes it false lives inside the same payload the
  claim was added in, one earlier CHANGELOG-contradicts-`build.rs` shape being the case in point.
  Sealing costs a judge its sight because the judge's question reaches past what it was handed;
  sealing a lens costs nothing because the lens's question never did.

  The question was derived by classifying every `review-finding` this tracker held on 2026-09-02 —
  885 of them —
  by identity slug, a keyword classification and approximate rather than measured per-finding, and
  the entry on `review-lens` says so. 436 of those, 63 of them `severe`, read as prose disagreeing with
  code beside it; 36 read as behaviour, which is what a judge's open scope exists for and a lens does
  not attempt.

  **What was built.** `review-lens` is a reserved row and a rendered definition —
  `RESERVED_REVIEWERS`, `skill::AGENT_DEFINITIONS`, the embedded-asset digest, and every registry
  that walks either already knew it existed — and `skill/policies/blind-judges.md` now acquires
  it, scaled by `review_tier`'s own count, and when this was built it was beside two judges:
  `medium` acquired one lens, `high` five, `low` none, in two waves — every acquired lens first,
  concurrently, sealed to the identical diff and criteria and holding no `Read`, then the two judges
  over the merged list of lens findings. The second wave was later removed, and the judges with it,
  and the counts are the source project's `0`/`1`/`4` today: the lenses run concurrently and are
  the whole of ordinary review.
  `record_review_finding` now knows a lens: `--lens` names one of the five shipped roles, validated
  against `crate::harness::roles::reserved_lens` rather than minted from caller text, and every
  lens finding — under any class, not only a severe causal one — owes a `location` whose path
  `base..head` changed, refused as `lens-finding-location-required` or
  `lens-finding-location-outside-the-diff` otherwise. That refusal checks the path alone, never
  the line, for a finding not already owed the stricter check: one that is also `severe` with
  `introduced`/`worsened` owes the same line containment any severe causal finding owes, refused
  as `finding-location-outside-the-change` instead — the one rule applied to both, repaired in
  round 1 after shipping unconditional. That is the out-of-diff refusal this entry used to say
  was deferred.

  **The severity and routing decision is ported from the source project's Go, not from an earlier
  draft of this entry.** An earlier plan for this change would have forced a lens finding's class
  to `warning`; that plan was never theirs and is dropped. A lens decides its own severity and
  nothing here overrides it — like any reviewer, it may record `severe`. What decides whether
  that finding blocks is `evidence_class`, the vocabulary `FINDING_EVIDENCE_CLASSES`
  (`claim.rs`) already carried before this change: `deterministic` qualifies a rejection on
  its own, no confirmation owed, because the lens demonstrated the behaviour directly;
  `inferential` needed a judge to confirm it when this was written, in one pass over the whole merged
  list of lens findings for the round — never a second pass per finding, and `review-refuter`'s one
  batch is that pass today; `insufficient` does not qualify a
  rejection on its own and is recorded and carried rather than dropped. This repository built no
  refuter at that point — its own *Out of scope* says so. **One was built later, for a
  different question than the one this paragraph was about, and this entry is corrected rather
  than left describing a gap that closed.** `review_findings`
  (`claim.rs`) had already been added, a read-only operation answering every finding recorded against an exact resolved
  receipt, so the ground this entry used to give — *no read path returns findings for an epoch* —
  was not true even before the refuter. What that addition alone did not close is what the refuter did: `review-refuter`
  is now a reserved row in `RESERVED_REVIEWERS` (`src/harness/roles.rs`), sealed, carrying its own
  template (`skill/agents/review-refuter.md`) rather than the judge's or any lens's;
  `record_refutation` (`claim.rs`) records its one-batch-per-receipt answer —
  `corroborated`/`refuted`/`inconclusive` per finding, never one task per candidate; and
  the computed verdict drops a finding recorded `refuted` from the ledger.
  **What the refuter left open, on purpose, is a different question: whether this refuter should also
  perform the confirming pass an `inferential` **lens** finding already gets.** It does not. So the
  confirming pass for an `inferential` lens finding stood as it was — the judges' own
  single pass over the whole merged list of lens findings for the round, never a second pass per
  finding — with `review-refuter` wired only into the computed verdict's reading of refutation
  outcomes. The judges' pass was later removed, so the refuter's one batch is now the only
  confirming mechanism for an `inferential` finding, as in the source project.

  **What `review_findings` answers is not checked, and three things it does not prove are worth
  naming rather than assumed.** It walks `first_protocol_markers`, the same reader
  `record_computed_verdict` itself uses, which keeps only a marker whose comment is `unedited` — so
  an edited finding comment is not answered, and the answer cannot tell an edited finding apart from
  one never recorded at all. That is inherited from the reader every other ledger read here already
  uses, not a new gap `review_findings` introduces, but it is newly visible to a caller that did not
  read a ledger before. What the operation derives is `blocking` and the two byte counts;
  everything else it answers is read off the marker as the reviewer wrote it — a declaration, not
  a checked claim, and nothing here re-runs a finding to confirm one. And `FINDING_TEXT_CEILING`
  (32,768 bytes) is chosen against a bound, not measured: the only measurement this repository has
  of the MCP surface's own answer size is the band `Publication::inline_payload`'s own doc comment
  in `claim.rs` states, not the corresponding entry in [`honesty.md`](honesty.md) — which
  names only 145,756 bytes refused at 151,889 characters and never states 36,288 at all. That band:
  answers carrying
  73,272 and 145,756 payload bytes were refused and one carrying 36,288 was not. 32,768 sits inside
  the observed-clear half of it; that is a choice made against the band, not a measurement of where
  `review_findings`'s own answer stops fitting through the same surface.

  **Two divergences this port owns rather than inherits, decided and recorded
  here rather than smoothed over.** This build ships a fifth lens, `review-lens`, beside the
  source project's four; its question — *does any sentence this change adds contradict any code
  this change adds?* — has no counterpart in their set. A later change took it out of
  the tier's allocation, which is now the source project's `0`/`1`/`4`; only the delta-lens repair
  round launches it.

  A third divergence this build once carried is closed. It ran lenses beside two judges the tier
  licensed, where the source project's own Judgment Day mode states plainly that it *"replaces
  ordinary 4R as the adversarial method for that target; never run both"*. That is ported as
  stated: ordinary review acquires lenses and no judge, and Judgment Day, which runs only when the
  operator explicitly asks, replaces the lens review for its target and never runs beside it.

  A third, undeclared until this repair: `transaction.go:382-388` binds a lens finding's declared
  `Lens` field to the lens the source actually launched, defaulting an empty one to it and
  rejecting the whole result when a finding names a different one — the caller
  knows which lens it launched and checks the answer against that knowledge. `--lens` here is
  validated only for membership in the five shipped names (`reserved_lens`); nothing binds it to
  which lens the orchestrator actually launched for this call, because `record_review_finding`
  has no channel to that knowledge — it is handed a completed finding, not present at the launch
  that produced it, the same structural gap that leaves `reviewer` itself an unchecked claim for
  any finding it is handed.

  **What remains unproved, the same limits already recorded for a judge.** Estigia cannot prove a
  lens actually ran, that it was launched sealed rather than with a wider grant, that it saw only
  the diff its launch handed it, or that the lenses ran concurrently.
  And it cannot prove which context produced the two words the routing reads: every shipped lens
  definition now asks a lens for a class and, on a `severe` finding, for `evidence_class` and
  `causal_disposition`, but what reaches `record_review_finding` is whatever the orchestrator
  passes, so a value the launching context supplied is indistinguishable from one the lens
  declared. `RiskLevel::lenses` and `record_review_finding`'s `--lens` validation are enforced in
  code; the launch that turns a
  count into a subagent call is read by an agent from prose, the same as every other launch,
  and no code observes that act. A definition is a host constraint and a contract, not
  evidence that a lens — or the lenses beside it — was actually instantiated.

  **A recorded gap widened when its own bound was dropped, and is restated here with that bound
  gone.** The five lens definitions now carry both halves in prose — the never-`severe` treatment of
  stale prose and the carve-out that bounds it — so a lens reading a comment or a document has the
  written floor it did not have, and an earlier draft's bound, "because nothing launches a lens yet",
  went when something did. What is still true is narrower, and is why this entry stays rather than
  going: `tests/payload.rs`'s `the_stale_prose_rule_and_its_carve_out_reach_both_carriers` crosses
  `STALE_PROSE_NEVER_SEVERE` and `CARVE_OUT_BOUND` (`src/skill.rs`) against the two judge
  carriers only. The five lens copies are paraphrases no test crosses, so nothing keeps them
  agreeing with the constants or with each other — the floor is written, and the crossing is not.

- **A lens count is not a risk dial, and `RiskLevel::lenses` is built so
  a fourth tier cannot silently answer zero.** At the time this was measured, the judge count scaled
  with risk because judges are independent opinions on one open question — more risk, more opinions,
  and a bigger panel raises the odds that at least one of them catches what another missed; that scaling was later deleted entirely, along with the `judges` answer it described, because the
  source project this crate ports `review_tier` from never counts judges in code at all — it counts
  lenses in Go and states judge counts in prose. A lens is not an opinion; it is one
  narrow question, and N copies of that one question over one diff measure how consistently an agent
  answers it, not how much more of the change gets looked at. So `lenses` is a fixed allocation per
  tier, not a count of which lenses could find a subject in the diff — more than one sealed lens can
  plausibly have a subject in an ordinary code-touching change (`review-reliability` and
  `review-readability` both do, in most changes this repository ships), and this count was never
  measuring that: `1` for `medium`, the lens the caller's `focus` names (`reliability` by default,
  as in the source project); `4` for `high`, `review-risk`, `review-readback`,
  `review-readability` and `review-reliability` — four distinct questions, never four copies of
  `medium`'s one; `0` for `low`, because a change proven to add no code leaves nothing for a lens to
  be about. The answer's `lens_roles` names which. Two things the source project does not say
  hold here: its resilience slot is filled by `review-readback`, whose question is re-derived
  rather than ported (`src/skill.rs` says why), and the readability override for large pure
  documentation has no subject — `DominantLens` is declared there *"no longer derived"*
  (`internal/reviewtransaction/risk.go:108-111`) and nothing assigns it but a reset to empty
  (`internal/cli/review_start_contract.go:158,170`), so `SelectReviewLenses` reads it only to replay
  an old authority. Here a large pure-documentation change is `low` and licenses no lens at all.
  `RiskLevel::lenses` takes no `Context` and cannot fail the way the rest of the module's reads can, so
  it answers a plain `u8` — the only such field `review_tier`'s JSON answer carries now that
  `judges` is gone, and both the MCP `review_tier` tool description and `skill/bindings/github.md`'s
  row state the mapping. `tests/honesty.rs`'s
  `the_lens_count_stated_beside_review_tier_is_the_one_the_tier_derives` crosses both against
  `RiskLevel::lenses()` directly — the same crossing shape the reserved-reviewer count already gets
  from that file's own `the_number_of_reserved_reviewers_is_the_number_every_document_claims`.

  **Closed — four carriers this entry tracks, not a running total for the whole
  issue.** This entry used to say three shipped sentences claimed nothing launches a lens —
  `skill/policies/blind-judges.md`'s, `skill/references/runtime-notes.md`'s and
  `docs/configuration.md`'s. A fourth survived the same round, inside this very file: a
  paragraph under the "third change" bullet below, `docs/honesty.md` at
  the time, pre-existing and never
  read again during that change's own drafting, said the lens severity floor was "not yet closed" and
  "prose because nothing runs a lens yet" while the paragraphs beside it were rewritten to say
  the opposite. All four are corrected now. **Four is not the count of every sentence this issue
  has ever corrected, and this entry stops trying to be that count.** A prior repair round's own
  review found more, on that round's own payload, than this entry ever named — recorded on the
  issue's own ledger rather than restated here, because a number copied from a finding instead of
  re-derived is exactly the mistake this paragraph has already made twice. Re-derivation attempted
  in this round found a different number again: `git diff 424c6c48 c194b20b -- skill 'docs/*.md'
  README.md CHANGELOG.md | grep -E '^-[^-]' | grep -inE 'lens'` — `424c6c48` the head the original publication reached, `c194b20b` the head its first repair round produced —
  returns 23 removed lines
  mentioning the word, and reading every one by eye finds two carrying the launch-claim family
  this entry tracks — both in the one paragraph at `docs/honesty.md` at the time — with the rest
  split across the severity-routing and path-only families this entry is not about. That mismatch
  against the ledger's own figure is left unresolved rather than guessed at: the two counts may be
  answering different questions over different diff ranges, and this entry does not know which.
  What it commits to instead is a count that stays checkable without re-opening that question: the
  four carriers named above, by name, re-verifiable by reading each rather than by trusting a
  number. That no test in this crate could hold any of
  them true is still true of the act itself: Estigia still cannot observe a launch, the same limit
  named above. What changed is that the claim those sentences made is gone from two
  (`blind-judges.md`'s and this file's own) and narrowed in the other two, because something now
  launches a lens: `skill/policies/blind-judges.md` states the acquisition and the count in prose,
  the same way Judgment Day's rules are stated in prose rather than proven, and
  `tests/honesty.rs`'s `no_shipped_surface_still_claims_nothing_launches_a_lens` reddens if a
  shipped surface still says so — for the three it can reach; this file is not one of them, see
  below. `skill/references/runtime-notes.md` and `docs/configuration.md` are narrowed
  rather than rewritten wholesale, because each is genuinely scoped — one names its own two
  paragraphs, the other the set setup installs — and neither claim is false on its own subject;
  each now names the owner that does launch a lens (`blind-judges.md`'s "Lens acquisition"
  section, linked from developer step 5) in the same sentence rather than leaving a
  reader who reaches either document to answer "how do I launch a lens" told that nothing does.
  The sweep test does not simply skip these two files — it swept them, found them offending on
  the first pass, and was corrected rather than the files: its `SCOPED` list only tolerates a file
  that also carries the owner clause (`OWNER_REFERENCE`) within `OWNER_WINDOW` bytes of the
  claim, so deleting that clause from either file makes it offend exactly like any other —
  proven on both files, not only the one the first pass checked: an unrelated, pre-existing
  occurrence of the owner phrase 611 bytes from the claim in `docs/configuration.md` used to
  hold the file exempt on `text.contains` alone even with the added clause deleted, which is
  the defect the proximity check and the second mutation both answer.

  **What a phrase sweep cannot do, measured rather than merely disclaimed — twice over, on two
  different surfaces.** `skill/references/runtime-notes.md` was missed twice by two independent
  readers, both reading by grep rather than by eye: once when the eleven surfaces this issue's own
  history first named were listed, and again by
  `no_shipped_surface_still_claims_nothing_launches_a_lens`'s own first `CLAIMS` list, which held
  five phrasings and not the two this sentence used — *"paragraphs launches"* and *"never
  launches any of them"*. Both are in `CLAIMS` now, and the second was reddened on a real
  occurrence rather than only a synthetic one: it reached `skill/SKILL.md`'s blind-panel
  clarification during this same round's own repair, the sweep named the file and the exact
  phrase, and it was corrected before this entry was written. `docs/honesty.md` — the
  "third change" bullet's own lens-floor paragraph, at the time — was missed by
  three readings: the operator's phrase-grep, which did
  not name it among the eleven; the sweep, which cannot reach it by construction, wholesale-
  excluding this file and, independently, carrying no `CLAIMS` phrase that matches its wording
  either; and the publishing run's own drafting pass through this same file, which rewrote the
  paragraphs beside it and never re-read this one. A phrase sweep only ever catches the phrasings
  it is told about, and only in the files it is told to read; this is the measurement of how many
  passes two surfaces took — two misses on one, three on the other — not an argument that either
  count is now complete.

- **The third change: two brakes ported as prose, and one not ported at all — named as such
  rather than left silent.** The porting error this whole issue exists to fix — the
  source project's lens dial wired to this binding's judge count, `RiskLevel::judges` read through a
  `Blind judges` setting — is `src/transport/tier.rs`'s own module doc's admission (*"This module used
  to wire their lens dial to this binding's judge count instead… both are gone, and `skill/SKILL.md`
  and `skill/policies/blind-judges.md` are where the judge count lives now"*), and that judges are two,
  fixed, in prose rather than in code — because that is where the source project keeps its own judge
  count, in `internal/assets/skills/judgment-day/SKILL.md`, never in Go — is what the two entries above
  ("`review_tier`'s tier classification carried three blind spots" and "A lens count is not a risk
  dial") already carry in full, including the measurement behind keeping a fifth lens, which a later change then stopped allocating. This entry does not restate either.

  **The severity floor is theirs, byte-identical across all four of their lens bodies:** *"Severity
  floor. Only BLOCKER/CRITICAL findings that survive adversarial verification enter the fix → re-review
  loop. WARNING/SUGGESTION findings are reported once with status `info`, are never re-reviewed, and
  never block."* Mapped onto this binding's one blocking class, `severe`, the floor and the
  never-re-reviewed half of it are now `skill/policies/blind-judges.md`'s own "Severity floor"
  subsection and the paragraph after "Elect a round when a finding blocks…" — the second because
  `skill/SKILL.md` developer step 5 already stated the accepted-outcome half and only the repair half
  was missing.

  **No `review-refuter` is ported *into this loop*, and this is why — at `medium` and `high`.**
  (A `review-refuter` role was later built, for the distinct question of adjudicating a
  panel's own findings before a rejection spends a round rather than for this loop's own
  adversarial-verification floor — this document's own entry above, beside `FINDING_EVIDENCE_CLASSES`,
  says which and does not reopen this argument.) Their own adversarial-verification paragraph
  resolves its own floor:
  *"Judgment Day is the exception: its two-judge convergence satisfies adversarial verification and
  it spawns no `review-refuter` tasks."* Part 1 of this issue once made the panel two judges with unanimity, and by that rule the judge path
  carried the verification a `severe` finding needs before it drove a repair, so a refuter there
  would have been a second confirmation pass. The unanimity rule was retired, and the judge path
  was then removed from ordinary review, so the argument is moot rather than reversed:
  ordinary review has lenses only, a lens's `inferential` severe finding follows its refuter, and
  Judgment Day — where two-judge agreement is the corroboration — launches none, as the source
  project states. The measurement that
  justifies stating the floor as a named rule rather than leaving it implicit in the two tables: across
  two issues in one session, four judges recorded zero severe findings and three repair rounds were
  elected anyway, roughly 570,000 tokens spent re-reviewing prose no judge had blocked on. The rule that
  would have prevented it was already written and was not followed — which is a reading discipline, not
  a missing gate, and nothing here adds one.

  **The lens path's floor closed, inside the same file that once bounded it.** This
  entry used to say the floor was not yet closed here and was prose because nothing ran a lens yet: a
  lens carries no panel to confirm itself against, so its own `severe` finding could not be the
  adversarial verification the floor asks for, and `skill/policies/blind-judges.md`'s "Severity
  floor" subsection said a judge must confirm every lens severe finding before the fix loop applies.
  That sentence is gone, not merely unenforced: the source's own routing was ported in its
  place — `evidence_class` decides, `deterministic` qualifies a rejection with no confirmation owed,
  `inferential` needed the one pass over the whole merged finding list for the round, now the refuter's one batch — carrying
  the source's own structural task-ceiling rule (*"1 refuter task for a standard review or 3 total
  for full-4R, whether the list has 2 candidates or 20; **NEVER spawn one refuter task per
  candidate**"*) — never one per finding, and `insufficient` never blocks. Something now launches a
  lens too: `blind-judges.md`'s "Lens acquisition" section, linked from developer step 5.
  What is unchanged is the standing of the rule itself — `evidence_class`'s routing is stated in
  prose, the same way Judgment Day and the `pre-existing`/`base-only` non-blocking convention
  already are, and nothing in the harness enforces any of the three, the same limit the entry above
  names for the sentences that predate them.

  **The convergence statement needed nothing added.** Theirs: a maximum number of fix rounds, the loop
  never extends past it, and whatever is still open after the last round is reported to the person as
  open. `skill/SKILL.md` developer step 5 already said the identical shape before this change — a ceiling
  refuses the next launch, assembles what every round measured, hands it to a named person, and stays in
  `review` rather than `blocked` — and `skill/policies/blind-judges.md`'s repair paragraph says the
  same; the bound is now a spent correction rather than a ceiling, and the rest of the
  shape is unchanged. Comparing the two before writing found nothing genuinely absent, so nothing was added;
  a sentence restating a rule that already holds is the duplication this repository files issues about.

- **The first cut crosses three symbol classes `skill/` names against the code that owns
  them, and the mapping pass that scoped it named two more this change does not take.** The
  mechanism is an extractor, `crate::markdown::markdown_inline_code_spans`, that reads every inline
  code span in a Markdown document — fenced blocks (``` or ~~~) skipped entirely, a prose mention
  outside backticks never entering the corpus — so the population is derived from `skill/` itself on
  every run rather than a list somebody remembers to extend. It could not live beside
  `source_text::code_of`: `build.rs` compiles `source_text.rs` standalone
  (`#[path = "src/source_text.rs"] mod source_text;`, to reuse `code_of` without depending on the rest
  of the crate), and every Markdown function would have sat there unused in that separate
  compilation, red under `-D warnings`. It lives in a sibling module, `src/markdown.rs`, instead.

  **Refusal codes, the direction nothing crossed before this.**
  `every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer`, which
  [`honesty.md`](honesty.md) names, already
  crosses code → `skill/`; `every_refusal_code_skill_names_is_one_the_crate_can_answer_with` is the
  reverse, and it is the direction where a drift becomes an instruction branch no agent can reach —
  a code renamed or removed in the transport leaves a table row and a recovery paragraph pointing at
  an answer that can never arrive. Re-measured against the corpus as it stands by re-running the
  crossing's own walk — `estigia::markdown::markdown_inline_code_spans` over every `skill/**/*.md`,
  deduplicated, then resolved against `TRANSPORT_VOCABULARY`: 184 distinct lowercase-kebab backticked
  spans, of which 117 resolve as exact literals (a leading hyphen stays excluded, since that shape is
  a CLI flag, not a code). Every pair written here before — 107/63, then 124/70, then 125/71, then
  177/113 — was measured at the commit that wrote it and drifted under later edits to `skill/` that
  never revisited it; the guard at `tests/honesty.rs` floors `read` and `candidates.len()` close under
  the walk's own counts, so these two are a reading somebody re-runs rather than a number the suite
  holds. One more,
  `closed-by-own-delivery`, resolves by matching the code side for a `"closed-by-own-delivery:` prefix
  rather than a whole literal, because it is built as `format!("closed-by-own-delivery:{}:{}", …)` at
  `src/transport/claim.rs` and never spelled as a standalone string — a literal-equality check would
  have reported a real code missing. The prefix reader cuts the source where `build.rs` cuts it, at an
  inline `#[cfg(test)] mod tests {`, so a fixture cannot supply a prefix the transport does not ship.
  The rest are workflow states, setting values, agent
  names and operation verbs — four other symbol families sharing the identical shape,
  subtracted by extracting each from its own source (`config::STATES`, `Setting::answers()`, the
  `skill/agents` directory listing, and the crate's own `transport::commands::SCRIPTED` operation
  table) rather than guessed away; reading the table rather than the corpus's own `SCRIPT <verb>`
  mentions is what stops a symbol exempting itself by the way it is spelled. What is left after all of
  that, 44 entries counted in the table itself, is a data table,
  `NOT_A_REFUSAL_CODE`, each with a one-line reason — a fictional run-id, a regex fragment, a git
  subcommand named as an example, a JSON answer field, a reserved role, a `--causal-disposition`
  vocabulary value — checked for staleness in both directions, so an entry that stops appearing in
  `skill/` has to come off, and an entry the transport now answers with is a real code the exemption
  was hiding. The 64
  distinct-code, 63-plus-one figure the issue's own mapping pass estimated by reading rather than
  running turned out right; the population it was read against (211 raw kebab occurrences before this
  extractor's own leading-hyphen and shape rules, over all of `skill/`, not the four files it named)
  was not.

  **This check is scoped to the transport's vocabulary.** `TRANSPORT_VOCABULARY` is generated from
  `src/transport/` only (see the extractor entry in [`honesty.md`](honesty.md)); the roughly 28 refusal codes `src/harness/**` answers
  with directly are not in it, and grepping them against `skill/` today returns nothing, so none of
  them blocks this check as it stands. If one were later named in `skill/` — the gate's own
  `harness-not-installed` or `verdict-bound-to-other-bytes`, say — this test would report it
  **unknown** and fail, which is the wrong verdict for a code that is real; it would need `known` to
  include a harness vocabulary this change does not build. That gap is the harness half of the
  documentation hole the mapping pass found and named as separate from this crossing:
  *"none of the gate's own refusal codes is documented in `skill/`"* is still true after this change,
  and nothing here closes it.

  **Workflow states, both directions, and one hand-typed copy removed.** The forward crossing
  (`STATES` against `skill/SKILL.md`, whole-file `contains`) already existed. The reverse — the
  contract's own *"Keep exactly one of `analysis`, `ready`, `in-progress`, `review`, `blocked`, or
  `done`"* sentence against `STATES` — was a six-item literal retyped inside
  `the_states_a_routing_accepts_are_the_ones_the_contract_declares` rather than read from the
  sentence it claimed to cross. It now extracts the sentence's own
  backticked spans through the same extractor, so a seventh state added to the sentence and not to
  `STATES`, or the reverse, reddens the test rather than waiting for somebody to notice the two copies
  disagree — proved by inserting `` `fake-state` `` into the sentence and observing
  `the_states_a_routing_accepts_are_the_ones_the_contract_declares` fail with *"the contract declares
  `fake-state` and this crate does not hold it"*, then reverting.

  **Configuration rows, and a live drift the crossing caught in the same change.**
  `every_row_the_shipped_config_block_names_is_a_real_setting_label` reads `skill/SKILL.md`'s shipped
  operator-configuration table through `config::table_rows` — the same parser the transport itself
  reads the block with, so no second table reader was written — and checks each row's label against
  `Setting::label()` exactly, never `Setting::from_label`'s alias-tolerant lookup: an alias exists so
  an operator's own already-written file keeps meaning what it meant, not so the versioned template
  may ship one. `skill/SKILL.md` shipped `"Description for dumb humans" sentence language`, a
  retired alias for what `Setting::label()` calls `Summary language` — a fresh install would have seen
  a row `estigia config set` accepts by alias but the template itself never spells correctly. Restoring
  the old text and running the test reproduces the exact failure — *"the shipped config block names a
  row no `Setting::label()` spells: [\"\\\"Description for dumb humans\\\" sentence language\"]"* —
  before the fix is put back. `Integration mode`, `Renewal window` and `Irreversible commands` are not
  treated as a defect: the shipped block carries 9 of the settings table's 26 rows, and those three
  are the rows `SKILL.md` never mentions anywhere — not the block's whole omitted set. It is a sample
  of common rows, not an enumeration, and nothing requires every setting be discussed in the contract.

  **Two of the issue's five named symbol classes are not taken here, on the mapping pass's own
  reasoning, restated because a reader of this entry should not have to open the issue to know the
  boundary.** Tool names — MCP operation names, `contract_name`s, JSON answer fields and GitHub event
  names — need a namespace discriminator this change does not build, and the reason is sharper than
  one shared shape: **the family is spelled two ways.** Most of it is backticked `snake_case`, which
  the kebab filter above never admits at all, so those members are invisible to this change rather
  than exempted by it. A handful are spelled kebab and do reach that filter — `parent-pr`,
  `parent-head`, `parent-base`, `parent-digest` and `payload-digest`, five JSON answer fields on
  `publish_review`'s marker, which is why they sit in the exemption table above. An extractor over
  either spelling alone therefore sees only part of the class, which is the discriminator this change
  does not build. Command verbs are a population of one distinct verb
  (`estigia setup`, two occurrences in two files) and a guard over one verb would protect nothing.
  Both are left for a separate issue rather than folded in or silently dropped.

  **What this makes mechanically adjudicated, against what the contract only states.** Adjudicated,
  each by a test that reddens on drift: refusal codes named in `skill/` resolve in the transport
  (new, this change) and refusal codes the transport answers with are named somewhere in `skill/`
  (existing, 59 frozen exceptions); workflow states cross both ways (the reverse newly extracted
  rather than retyped); the shipped configuration block's row labels are real `Setting` labels
  (new); the four finding vocabularies against every reviewer definition that records a finding,
  both judge seals and the five sealed lenses (new, this change); MCP operation names, agent
  definition names, lens/reviewer counts and the per-file byte ceiling
  are each crossed elsewhere in this crate, named above. Asked for and not adjudicated by anything:
  the gate's own ~28 refusal codes against `skill/` (undocumented, not merely uncrossed); the
  `SCRIPT`-verb-to-dispatcher crossing and the transport CLI flags inside `bindings/github.md`; the
  same four finding vocabularies against `blind-judges.md`, which is orchestrator-facing prose and
  is crossed by nothing; the exit-code table's *meanings* (the number set is checkable, `4` is
  stated in the binding and unchased against `Failure::code`); the numeric literals `262144`,
  `150000` and the `20` card cap; configuration row **value** words (`direct`, `ask`, `branch`, …),
  which are ordinary English rather than a closed vocabulary a label lookup can check; and the
  severity floor, Judgment Day's convergence rule and the repair-ceiling narrative, each stated in
  prose with nothing in the harness enforcing it, named in the entries above this one.

  Since 2026-09-07 the four finding vocabularies are crossed against the definitions a reviewer
  actually starts from, and that crossing is described as what it is. One test walks
  the seven `RESERVED_REVIEWERS` rows that record findings and asserts each value the transport
  accepts appears in the definition's own template bytes — a template-level check, because
  `render_reviewer_agent` substitutes three literals none of the values is a substring of, so a value
  present in the template is present in every rendering, and the second, rendered assertion this
  crossing used to carry could never fail where the first passed. What is asserted on the rendered
  body instead is the fields `record_review_finding` reads — `origin`, `evidence_class`,
  `causal_disposition` — which is what the reviewer starts from; the body is rendered once per
  evidence grant for the one unsealed row whose rendering reads the grant, and once for the six
  sealed rows whose rendering ignores it, with the grants derived from `Evidence::all()` so a third
  grant is walked rather than silently uncovered. The count of rows walked is pinned, so a shrunk list
  cannot pass over fewer. The vocabularies are read from their `pub const`s rather than retyped, the
  length of that list is pinned at four, and the four are asserted pairwise distinct, so neither
  deleting an entry nor substituting one const for another drops a vocabulary from the crossing.
  Adding a sixth `causal_disposition` value without teaching it reddens, naming the template rather
  than the rendering; so does stripping the sentence from a single lens, substituting a const, or
  dropping a field name from one template. All were run. **What it is, exactly, is a presence check,
  and three things follow that a reader should not have to discover.**
  It cannot attribute a value to a field: `introduced` belongs to both `FINDING_ORIGINS` and
  `FINDING_CAUSAL_DISPOSITIONS`, so that assertion is satisfied by whichever sentence teaches
  either, and a value added under a spelling the document already uses lands untaught and green. It
  cannot see a value removed from one place while it survives in another — measured, not reasoned:
  deleting the `severe` row from the judge's classification table leaves the test green, because
  that word appears at eight other points in the same template. And it crosses one direction only,
  proving every accepted value is named, never that a word the prose teaches is still a value the
  transport accepts, so a vocabulary that *shrinks* leaves stale prose behind and nothing says so. A
  structural check — this value, in this table — is a different mechanism and is not here.
  `review-refuter` is outside the crossing deliberately: it adjudicates through `record_refutation`,
  which carries no causal field. `blind-judges.md` is outside it too, which is the half the item
  above still names.

- **A finding filed as its own tracker issue names its receipt now, and Estigia still cannot see
  whether filing happened at all.** `create` takes optional `--epoch` and `--findings` (one or more
  `+`-separated finding identities), and supplying `--epoch` renders the issue's marker as `review`
  instead of `analysis`, carrying `epoch` and, when given, `findings`. That is provenance recorded
  when a caller supplies it, not a fact adjudicated: nothing checks that a finding really was
  recorded against the named epoch, that the epoch itself was ever a real publication, or that the
  issue was filed at all — a run that never calls `create` looks identical to one whose filing
  failed silently. `epoch` and `findings` are stored and read back exactly as given, the same
  standing every other marker attribute already has. The `--findings`-without-`--epoch` refusal is
  asked in `dispatch`'s `create` arm, ahead of `commands::create`, so a call that names a finding
  and no receipt creates no label and files nothing; asked only inside the marker, as it was, it
  fired after every label had been created.

  What the marker now distinguishes: an issue filed with a receipt from one filed with none, so a
  reader — or a later crossing — can tell a review-sourced finding from an analyst's own defect
  report without parsing prose for the difference. What it still does not touch: the `analyst:<runtime>`
  label is applied identically to both, because changing it touches the attribution rule in
  `SKILL.md` and the `dev:` label removed at release, which is a separate change.

  **The measured cost of not having this, on this repository's own open issues.** 129 issues are
  open (measured 2026-09-06: `gh issue list --state open --limit 500 --json number`, run against
  this checkout). Grepping every open issue's body for the word `epoch` followed within twenty
  characters by a hexadecimal run of eight or more characters — a caller narrating in prose which
  publication a finding came from, because there was nowhere structured to put it — matches 34 of
  the 129 issues. Counting the word immediately before each match gives **thirteen** surface
  spellings over **ten** stems, three of which appear in both singular and plural: `publication
  epoch` (12 occurrences), a bare `epoch` (12), `against epoch` (7), `of epoch` (4), `receipt epoch`
  (3), and one each of `under epoch`, `recorded epoch`, `parent epoch`, `on epoch`, `at epoch`, plus
  the plurals `publication epochs`, `epochs` and `against epochs`. That enumeration is the whole set
  at this head rather than a sample, and it is a set nothing before this change could read: each is
  free text a human or a future grep has to parse, and a grep is exactly what produced this count.

  **Three filings** are the concrete case. The first (filed 2026-08-19) and the last (filed 2026-08-24) are
  two recordings of one finding — a stale adapter count on the neutral skill root — found
  independently five days apart by two different runs, each unaware the other had already filed it;
  the middle one (filed 2026-08-21), by a third run, is a related but genuinely different count on the same
  family of source comments. All three still carry `priority:low` and none is repaired as of
  2026-09-06. A comment posted to the first on 2026-09-06 now cross-links all three and says so
  in prose — but that link is itself only prose: no field on any of the three issues carries
  another's identity, `create` was never called with `--findings` naming a sibling, and nothing
  added since reads the comment back as data. The rule this change adds does not retrofit that link;
  it only means a finding filed from now on need not repeat the loss.

- **`finding-citation-unresolvable` checks that a `path:line` citation names something that exists
  and has enough lines, not that it still means what the finding says.** It ports
  `artifactReferenceTokens`/`artifactRepositoryPathReference`/`frozenRepositoryPathLookup.contains`
  (three spans in `artifact_admission.go`), and the check it adds to
  `record_review_finding` — `src/transport/claim.rs`'s `check_finding_citations`, run over `evidence`
  and `impact` after the location block and before the lineage rules — is existence and a line bound,
  nothing more. It answers `finding-citation-unresolvable` under one of four causes —
  `malformed-path`, `path-not-in-either-tree`, `path-is-not-a-file` or `line-past-end-of-file` — and
  **reads both `base` and `head` from the recording checkout**, through `citation_ls_tree` and
  `tier::blob_content` under `How::read()`. Before it only a severe `introduced`/`worsened` finding
  or a lens finding touched `base..head` locally; the citation check runs for **every** class, so a
  run recording a `suggestion` from a checkout that never fetched the receipt's `head` now fails with
  a git read error instead of recording:

  - **A path that exists and has moved lines still passes.** A citation naming `foo.rs:42` after a
    repair shifted that file's line 42 down to line 60 resolves the same as one naming the correct
    line — the check compares the citation's line against the larger of `base`'s and `head`'s blob
    line counts, not the diff between them, so it cannot tell a citation that used to be right from
    one that still is; a line that merely moved still passes, on either side.
  - **A path no tree holds as a blob, where either tree holds a `120000` symlink or `160000`
    submodule commit, is admitted at any line, with no line check at all** — `resolve_citation_path`
    (`claim.rs`) answers `CitationResolution::Found { line_count: None }`, because there is no blob
    `tier::blob_content` can count lines in, so no line check is possible for it.
  - **Six token shapes are skipped rather than resolved**, each returning `None` before any lookup:
    a citation missing either half (no `:`, or an empty path or line half); a line half that is not
    all digits **or that overflows `u32`** — `text.parse::<u32>().ok()` makes an absurdly large line
    a skip rather than a past-end refusal; a zero or reversed range (`0`, `5-3`); a path containing
    `://`, the scheme separator, so a URL is prose rather than a path; a path ending in a raw `/`,
    which names a directory but is treated as prose rather than resolved; and a token failing the
    `pathLike` gate below, so an unquoted extensionless root-level citation such as `Makefile:12`,
    `Dockerfile:5`, `justfile:9` or a `LICENSE` pinned to a line is never resolved whatever it names. The first four
    are deliberate narrowing; the trailing slash is a deliberate divergence from the origin, listed
    as the fifth below; the `pathLike` skip is deliberate parity with the origin's own gate, which is
    why the port keeps it.
  - **A citation inside a backtick-fenced code block is skipped, not scanned.** `citation_tokens` treats a
    backtick as a quote opener that closes at the next backtick, so a triple-backtick fence's first
    two backticks form an empty span and the third opens a quoted span closing at the closing fence —
    the whole block, trailing newline and all, becomes **one quoted token** whose tail is almost
    never all digits, so a citation inside a fence returns `None` rather than being checked. The
    tokeniser has no concept of Markdown; the skip is a consequence of the quote grammar, not a fence
    rule.
  - **A quoted token bypasses the `pathLike` gate**, and the quote grammar is "ends at the next
    occurrence of the same character," not a matched pair. An unbalanced apostrophe anywhere in
    ordinary prose opens a quoted span that runs to the next apostrophe, however far away, and
    whatever lands between them is checked as a path even if it never has a `/` or a `.` followed by
    a letter — either of which alone would satisfy the gate an unquoted token still has to clear. This
    is not narrowed away, because the same bypass is what lets a quoted citation without a `/` or a
    `.` (`` `LICENSE:1` ``) be recognised as one at all; `a_quoted_non_path_token_is_refused_as_the_pathlike_bypass_it_is`
    (`claim/tests.rs`) asserts the false positive this produces — `` `ratio 1:3` `` refuses on
    `ratio 1` — rather than leaving the class named here untested.
  - **A citation naming the ported Go by `path:line` is refused, and correctly so.** This document and
    several doc comments in `claim.rs` cite the origin by coordinates such as
    `artifact_admission.go:654-695`; that string satisfies `pathLike` (`.go` is a `.` followed by a
    letter) and is checked against `base`/`head` exactly like any other citation. `artifact_admission.go`
    is not a path this repository's tree has, so it resolves `path-not-in-either-tree` — the same cause
    a citation naming a file that never existed gets. The rule is applied correctly; the gap is only
    that nothing said so before this entry, so a finding that repeats one of this repository's own
    origin coordinates in its `--evidence` or `--impact` is refused for a reason that has nothing to do
    with the finding's own claim.

  **Five deliberate divergences from the origin**, each named on the definition it lives in —
  `citation_reference` for the first two, the fourth and the fifth, `resolve_citation_path` for the
  third: (1) the line grammar accepts `N-M` ranges, where `artifactRepositoryPathReference` accepts
  only a bare digit run and silently drops every range — this repository's own prose cites ranges far
  more than single lines; (2) the `pathLike` gate is `token.quoted || path_part.contains('/') ||
  citation_dot_then_letter(path_part)` — a disjunction, so a quoted token or any path containing `/`
  passes on its own and only a slashless, unquoted token is held to the `.`-then-letter clause, where
  theirs accepts any `.` at all and so reads `1.2:3` as the path `1.2` and refuses it as a false
  positive — a hand-trace of `artifact_admission.go` found no test in their own fixture reaches this,
  because it seeds `sha256` and `status` as real paths rather than arbitrary numbers; (3) a
  `100644`/`100755` match is read with `tier::blob_content` and its line count checked against the
  citation's `end`, where `frozenRepositoryPathLookup.contains` validates the line syntactically and
  never opens the blob at all; (4) an embedded, non-leading backslash in the path part is rewritten
  to `/` rather than refused for being one — on this repository's own reading of `risk.go`'s
  `normalizeLogicalPath`, which `artifact_admission.go` only calls and which is not itself in this
  repository and so cannot be checked from here, theirs rejects any backslash outright; this
  repository's choice is deliberate consistency with `parse_finding_location`'s own normalisation of
  `--location`, not an oversight, and is stated as a reading of code this repository cannot verify
  against its source, not as an established fact; and (5) a path ending in a raw `/` answers `None`
  rather than being routed through `normalizeLogicalPath`, whose `path.Clean(value) != value` makes
  `docs/` non-canonical and rejects the whole result — this repository drops the citation silently,
  deliberate because a trailing separator is not a path any tree records, while a slashless
  backslash-terminated path is skipped by the `pathLike` gate and a path ending in a backslash
  *after* a slash is refused as `malformed-path` once normalised.

- **Estigia cannot observe which bytes a judge was actually handed.** `publish_review` materialises
  the review payload and binds its digest as `payload-digest`; it cannot see whether the run that
  read it pasted the whole file into a judge's context, summarised it, or handed over something else
  entirely. `publication-base-not-ancestor` closes one way a *correct* digest could still
  cover a *corrupted* payload — a `base..head` diff built against a base that moved past the branch's
  own rebase, carrying the base's own history inverted into what reads as this change's deletions —
  but the refusal is a gate on how the payload is built, not a proof of what happened after it left
  this process. Nothing here closes that second gap, and nothing in this repository can.

  **What the audit measured, and what it did not.** `git merge-base --is-ancestor <base> <head>` run
  against every one of this repository's own 270 publications found 7 where the base was not an
  ancestor of the head, 6 of which carried another change's work inverted into the payload — worst
  case a receipt claiming 1,338 deletions for a change that deleted 26. That is a count of what
  happened on this repository's own timeline, not a rate: it says nothing about how often the
  hazardous condition arises elsewhere, or how often it would recur here absent the refusal this
  change adds. It also does not say how many review rounds the six inversions cost, and it does not
  say whether any of the six were caught by a reviewer before a verdict was recorded against them —
  neither was measured, and neither is claimed here.

  **No installed server has ever run this refusal.** The MCP server
  executes an installed binary. This machine's binary reports the commit it was built from,
  `53f5c0b`, dated 2026-09-06 01:42 — nineteen hours before the merge that delivered this refusal —
  and its own mtime, 01:51 the same morning, bounds any replacement since. Reading its bytes agrees:
  `publication-base-not-ancestor` is absent while `verdict-bound-to-other-bytes`, which predates the
  build, is present. Both are refusal strings that reach `.rodata`, which is what makes the absence
  evidence rather than a stripped symbol — a third reading, of the function name `judges_spelled`,
  was offered here in an earlier draft and proves nothing, since `[profile.release]` sets
  `strip = true` and that identifier is never a literal. So every publication this repository has
  recorded was answered by a server with no such check compiled into it, including the two whose base was not an ancestor of their head. Nothing above ever claimed otherwise; what is
  corrected here is a reading anybody would take from the present tense, that a delivered refusal is
  one that runs.

  **What the suite crosses, and the one thing only one fixture does.** Three tests name this
  refusal, and only one of them is blind to the fetch.
  `an_unmerged_base_advance_is_refused_before_the_correction_budget_ever_runs` (`tests/pipe/part_10.rs`)
  advances the base from the publishing repository itself, so its `refs/remotes/origin/main` is
  already current when `publish_review` is called.
  `a_base_that_advanced_past_the_branchs_own_rebase_is_refused_before_the_payload_is_built`
  (`src/transport/claim/tests.rs`) leaves it **present and stale**, as production does — it pushes
  from the publishing checkout once and then advances the base from a second one — but asserts
  nothing about the ref.
  `an_unmerged_base_advance_pushed_from_elsewhere_is_refused_once_the_fetch_moves_the_ref` is the only
  one that observes it, stale going in and moved coming out, so it is the only one whose failure names
  the fetch. Deleting `git fetch origin -- <base>` reddens it on the ref itself; the unit test reddens
  as `read-failed`, which does not tell a missing fetch from any other read failure; the `pipe.rs`
  sibling passes unchanged.

  That the fetch is what supplies the base the check decides against is settled by `publish_with`,
  which resolves `payload_base` from `origin/<base>` before the check and reads the pull request's
  `baseRefOid` only afterwards. No test establishes it; it is held by that reading.

  **What is still not measured.** All three run against a local bare origin and a scripted `gh`, so
  what is crossed is this process's own fetch-then-resolve and never a real remote's. And the audit
  above counted 270 publications made before this refusal existed; it is not a measurement of how
  often the refusal would fire now, which no run has yet been in a position to take.

- **A second delivery on one command line travelled on the first one's label, and the repair does
  not close every shape of that.** `classify_observing` recorded the first
  `IRREVERSIBLE` fragment *in list order*, and `stale_verdict` and `out_of_phase` both return early
  unless `DELIVERS` holds the fragment recorded. `git push` is on the first list and not the second,
  so `git push && gh pr merge <n>` skipped every delivery check there is. The fragment recorded is
  now the one the strictest check reads — `gh pr merge`, then the rest of `DELIVERS`, then
  everything else — and a line carrying exactly one fragment answers as it always did.

  **What the repair leaves to ranking, and where ranking is not enough.** Ranking
  promotes; it cannot adjudicate. Where a line's fragments are all one tier there is nothing to
  promote to, and the line used to be decided exactly as it was before: `git merge origin/main &&
  git tag v1` recorded `git merge`, and `git push && gh pr create`, both fragments tier 0, recorded
  the push and reached no delivery check at all. So this repair helps precisely when a line carries
  a fragment stronger than the one the list names first.
  The same-tier case is closed by refusing a line whose built-in fragments put two or more in
  one tier, with `several-irreversible-fragments-on-one-line` naming the fragments rather than any one
  of them, and it was handled separately rather than folded in because the original's own third acceptance criterion
  demands a refusal code that a multiplicity refusal can never reach.

  **Which is the thing this repair measured that no document said before: no compound line can
  establish a pull-request identity at all, and one check is all it takes.** `pr_merge_target`
  first requires every byte to be alphanumeric or one of ``space tab - _ . / : = +``, which excludes
  `&`, `;`, `|` and a newline — every separator that can make a line compound. So a compound line
  carrying a pull-request merge answers `delivery-pr-unidentified`, never `delivery-pr-mismatch`,
  and it answers it **before a receipt is consulted at all**. That is a stricter refusal than the
  mismatch the issue asked for, and the issue asked for one that cannot happen. The tests assert what
  the binary does rather than what the issue predicted. **The word test beside it is not a second
  barrier**: `words.get(..3) == ["gh", "pr", "merge"]` is a prefix, so `gh pr merge 66 && git push`
  satisfies it — and saying otherwise would credit this refusal with a defence it does not have.

  **What is still not measured.** How often a compound delivery line is written in ordinary use —
  this repair changes an allow into a refusal for that population and nobody has counted it. And the
  ranking reads the fragment's **spelling**, never where it came from: a declared boundary spelled
  `git tag` ranks as one, because `delivery_tier` takes a `&str` and knows nothing of provenance.
  Nothing observable turns on that — the built-in copy of any such spelling sits at a lower index in
  the same iteration, so the `Reverse(index)` tiebreak records it and the string is identical either
  way — but it is a property of the function rather than of the lists, and a declared fragment that
  is *not* a built-in spelling is tier 0 whatever it delivers, because Estigia cannot know whether
  somebody's `make deploy` delivers or rehearses, which is the reason `DELIVERS_POPULATION` already
  gives for excluding declared fragments from that list. The multiplicity count keeps the same
  exclusion: only built-in `IRREVERSIBLE` fragments are counted, so an operator's declared boundary
  never makes a line overfull on its own.

  **A population this repair newly refuses.**
  `git merge --ff-only origin/main && git push` from `in-progress`, under the default
  `Integration::Branch`, answered `allow` before this change, recording `git push`; it answers
  `out-of-phase` after it, recording `git merge`. Both exemption inputs are false because the
  literal carries `&` (`exact_local_fast_forward_target`, `exact_integration_merge`), so the line
  takes the generic refusal, whose resolution asks for a review of this head and does not name
  splitting the line. Under `Integration::Trunk` the same line answers `unflagged-on-trunk` with no
  flag and is allowed with one.

  **A second population this repair newly refuses.** `git commit -m "git tag the release" &&
  git push` from `in-progress` answered `allow` before this change, recording `git push`; it
  answers `out-of-phase` after it, recording `git tag`, which `contains_whole_command` matches
  inside the quoted message and which outranks `git push`. The over-match itself is a separate matter;
  what is recorded here is the population the ranking change moved.

- **`tests/pipe.rs` can fail under machine contention, and this repair does not make that stop
  happening.** Measured on `017cd5a1`: 14 runs of `cargo test --test pipe` under sustained
  external load produced 2 failures, a 14% rate — consistent with, though too small a sample to
  claim it matches, the roughly one-in-five rate previously reported. Both captured panics named a fixture
  command that failed under load, not a defect in the test's own subject —
  `what_a_half_written_file_left_behind_is_not_the_operators` hit `Os { code: 3, kind: NotFound,
  message: "El sistema no puede encontrar la ruta especificada." }` seven lines after a discarded
  `run(home.path(), &["setup", "claude-code"], "")` whose failure left the directory the test wrote
  into absent, and `a_republish_lands_a_rewritten_branch_and_stops_when_the_claim_moved` hit a bare
  `assert!(git(&["push", "-q", "origin", "main"]))`, whose `git` fixture closure discarded the exit
  code and stderr behind `.output().is_ok_and(|out| out.status.success())`. Neither test's own
  subject was implicated; both were victims of an environment write that failed and said nothing.

  This change makes such a failure say what failed: all but three of the pipe suite's 30 `git`
  fixture closures now go through one `checked_git` helper — defined once and called 69 times —
  which panics naming the argument list, the exit status and stderr on a nonzero exit, or the
  spawn error on a failure to run at all, where there is no exit status and no stderr to name.
  The three that do not are the declared `SKIP` sites in the pipe suite: each uses its own git
  result as an `if` condition to fall back to a `SKIPPED:` message when git is not usable in the
  environment, a behaviour `checked_git`'s panic-on-failure would remove rather than preserve. A
  further 31 raw `assert!(Command::new("git") ... .is_ok_and(|output| output.status.success()))`
  sites sit outside any fixture closure and outside this change's scope; they stay opaque, and
  converting them is a separate change. The discarded `run(...)` calls whose result a later line
  depends on now assert the outcome before relying on it — 23 such sites gained an `assert!(ok,
  ...)` they did not have at `258b828`, measured by counting `let (_, stderr, ok) = run(` in the
  suite at each commit (85 against 62).
  **What it explicitly does not do is remove the contention that causes the failure.** No retry, no
  sleep and no widened timeout was added anywhere in this repair — the trigger is machine load, which
  is not this change's to remove. This repair adds abort points of its own — the 23 new `assert!`
  sites above, plus the two `panic!` arms inside `checked_git` — so the rate at which the suite aborts
  under load can stay equal or rise; it is not true that only the panic naming the failure changed.
  The 14% figure above was measured on `017cd5a1`, before this change, and has not been re-measured
  on this head. A green run of this suite under load is still not proof that a red run earlier was a
  real defect in the code under test, only that this run did not hit the same contention.

  **The counterpart measurement, because it bears on the same suite and the same cause.** Staging a
  fixture checkout costs a measured 138 ms per call uncontended and 336 ms under 8-way contention, on
  the same machine and the same commit: 1,383 of 7,200 staging calls ran over 500 ms, and the slowest
  ran 1,302 ms. At that commit the suite staged **170** times per full `cargo test` — 87
  `scripted_gh()` call sites under `src/` (88 matches for `scripted_gh()`, one of them the definition
  at `src/test_env.rs`, which is not a call site — a correction the entry at `docs/honesty.md`
  still needs; its "74 call sites" was measured earlier and this entry supersedes it), and 83
  in `tests/pipe.rs`, being 82 `tracker_rig()` calls plus one one-off — so roughly 23 s of aggregate
  staging work per uncontended full run and roughly 57 s under load, spread across threads rather
  than wall clock. `cargo test --test pipe` alone accounts for 83 of them, about 11 s and 28 s
  respectively. One `mktemp` failed outright across those 7,200 calls. The `scripted_gh` directory has since been cached
  per test binary, so the 87 `src/` call sites stage once and the aggregate
  above is the pipe suite's 83 alone. None of this is closed by the change above — it is the same
  shape of gap, a deadline this repair does not and cannot remove, recorded here rather than left as
  a green run nobody questions.

- **`setup` follows `CLAUDE_CONFIG_DIR` and the gate did not — closed, with the
  measurement.** `resolve_paths` reads the variable, so `setup`, `sync`, `doctor`,
  `config` and `uninstall` all resolve Claude Code's five roots under `$CLAUDE_CONFIG_DIR` when it
  is set, but `definition_for`, `authorize_review_blind_launch` and the control-surface spellings the
  gate itself matches still resolved `.claude` under the home — on a machine whose Claude Code
  configuration was relocated, a `review-blind`/`review-blind-sealed` launch was refused
  `reviewer-canonical-unavailable` (measured live against a real two-profile machine on 2026-09-18:
  `~/.claude/agents` held a different agent's files and no Estigia reviewer, `~/.claude-personal/agents`
  held the eight Estigia definitions, and every reserved launch was refused), and a write to that
  relocated configuration's own `settings.json` or `agents/` was not measured as a control-surface
  write at all.

  `.claude.json`, `CLAUDE.md` and Estigia's own skill directory were never part of the gap: their
  fragments carry no `.claude/` directory name at all — `claude.md`/`claude.local.md`/`.claude.json`
  match on the bare file name, and `skills/<flow>/` (`matches_a_fragment`'s `installed` fragment)
  matches on the flow's own directory name — so both already matched wherever `$CLAUDE_CONFIG_DIR`
  pointed, before this issue touched anything.

  Three fixes close it. `harness::roles::definition_for` now also searches
  `$CLAUDE_CONFIG_DIR/agents`, beside `~/.claude/agents` rather than instead of it — the same
  asymmetry stated for the OpenCode root beside it in the same function.
  `harness::roles::authorize_review_blind_launch` proves the canonical reviewer against
  `$CLAUDE_CONFIG_DIR/agents` alone when the variable is set, and never against `~/.claude/agents` —
  that tree belongs to whichever profile the operator is *not* running under, and letting it stand in
  would let a stale or foreign file in the wrong profile satisfy or refuse a launch it has no standing
  over. `harness::matches_relocated_claude_config` closes the control-surface gap with a resolved-root
  check: it reads `$CLAUDE_CONFIG_DIR` (`setup::claude_config_dir`, mirroring `setup::xdg_config_home`)
  and asks whether a target is named `settings.json` (or any
  `settings*` sibling), `agents`, `skills`, `.claude.json`, `claude.md` or `claude.local.md` directly
  under that root — a
  question the fixed `CONTROL_SURFACE` fragments cannot answer, because `$CLAUDE_CONFIG_DIR` names an
  arbitrary directory and matching `settings.json` unanchored to it would turn every project's own
  `settings.json` into a boundary. The first spelling of that check read the root as a **substring**
  and asked only the write road's view, and two residuals survived: a path that merely carried the
  root's text mid-segment was measured as under it, and a root whose own name carries a fold
  character — a space most of all — stopped matching on the shell road, because `text_names_path`
  folds every whitespace run and every `NOT_IN_A_PATH_SEGMENT` byte into a separator before asking.
  A third survived the folded spelling the first fix added: the **escaped** spelling of the space
  leaves the folded byte and the token boundary side by side, so the view carries `//` where the
  folded root carries one `/`. All are closed: the root must now begin
  at a path boundary, and the shell road's folded spelling of the root is asked beside the root
  itself — with one separator and with two, the run the escaped spelling emits.

  Measured by `harness::roles::tests::a_relocated_claude_config_dir_is_searched_beside_the_default`,
  `a_relocated_claude_config_dir_does_not_hide_the_default_root`,
  `a_relocated_claude_config_dir_is_where_the_canonical_reviewer_is_proved`,
  `a_relocated_claude_config_dir_does_not_fall_back_to_the_default_home_tree`,
  `a_foreign_reviewer_under_a_relocated_claude_config_dir_is_refused`,
  `harness::tests::a_relocated_claude_config_dirs_settings_agents_and_skills_are_boundaries_on_both_roads`,
  `a_relocated_claude_config_dir_does_not_widen_the_gate_past_its_own_root`,
  `a_relocated_claude_config_dir_with_a_space_is_a_boundary_on_both_roads`,
  `a_path_that_merely_carries_the_relocated_root_is_not_a_boundary`, and the extended
  `setup::tests::every_control_file_an_adapter_has_is_one_the_gate_measures`, which now crosses a
  relocated `CLAUDE_CONFIG_DIR` layout the same way it already crossed a moved `XDG_CONFIG_HOME`.
  Each of the first two fixes reddens on its own when reverted — measured, not merely argued.

  `CLAUDE_CONFIG_DIR` remains deliberately not contained-checked against the home the way `APPDATA`
  and `XDG_CONFIG_HOME` are — `docs/what-it-writes.md`'s paragraph after its section on a redirected home says
  why, and that reasoning is unaffected by this fix.
- **Filing with a priority no declared scale allows is accepted, and the queue read says so.**
  `create` attaches whatever `priority` it is given and answers `priority_checked`: `true` only when
  the routed domain's scale is one this binary can read and the label after the colon is non-empty —
  today that is the `general` domain's `priority` scale from the autonomous contract in
  `skill/references/domain-composition.md`, the only declared scale the transport reads. Anything
  else is filed untouched and answered `priority_checked: false`: refusing would stop an analyst
  mid-file over one reversible label edit. `list_state` names, per partition, the returned items
  carrying no value on a readable scale under `unchecked`, and an answer with none is byte-identical
  to the one from before the key existed; a partition whose scale lives in a rule book the transport
  cannot read contributes nothing, because the transport cannot tell a value there from a missing one
  without reading a book it does not have. What the check does not do is judge a value's meaning:
  `priority:urgent` on a `general` issue counts as carrying a value, because what a value means
  belongs to the domain.

- **A developer's out-of-scope finding is handed to an analyst by prose alone, and the handoff is
  unobserved.** The Developer execution steps now say that a defect outside the issue in
  hand goes to an analyst run — a separate context holding its own claim — with the reproduction and
  the evidence the developer holds, and is never filed by the developer itself, because the context
  that produced a finding is the one that judges it worst.
  What holds the rule is review, not a check: a subagent launch is not a write the gate sees, no
  tool knows the intersection of a run's findings with its issue's scope, and a developer that
  files its own finding is not refused. A run that ignores the step is indistinguishable on the
  timeline from one that followed it, and that stays said here.

- **The Stop hook reads a transcript it cannot prove is complete, and the approved-republish gate reads
  order, not intent.** A run whose only remaining work is a background launch that has not
  reported now ends its turn instead of being kept for a poll, and a publication over an approved
  parent on the same pull request is refused unless a finding has been recorded against that parent
  receipt since the verdict — or the base advanced past the parent receipt's own recorded base, the
  integration `required_status_checks.strict: true` forces whether the run merged the base or rebased
  onto it. What neither
  check proves: the hook counts a launch only from the host's own record that it **started** — a
  `toolUseResult` carrying a non-empty `backgroundTaskId`, or one reporting `async_launched`; the
  text of a result is never read, so output that quotes the notice is not a launch — and it counts a
  launch reported only by a `<task-notification>` naming that launch's `<tool-use-id>`. A host that reports completion in
  another shape leaves that launch unanswered, which ends the turn; a transcript that has rotated
  past the launch reads as *no launch*, which keeps the older block; and a call the host refused or
  never answered is not a launch at all, so it does not stand the hook aside. None of the three
  directions is a clearance: the first can release a turn the run would have used, the other two
  spend one. The gate's "declared defect" is a finding's position in the timeline and not its
  truth: a finding recorded after the verdict clears the refusal whether or not the defect is real,
  the same declaration-not-proof standing every other field of a review finding has, and its
  advanced-base exception proves only that the parent's recorded base is a proper ancestor of this
  publication's (it is direction-aware now; it read inequality of the two tips before) — not that the movement
  was compatible, not that the integration is correct, not that the setting was on, and not that the
  panel's delta read is the whole of what changed (a merged advance is now read whole, so
  the delta lens is never the panel over base content). Both readers
  fail closed on a failed read — an unreadable transcript keeps the turn, and a timeline that will not
  read refuses the publication.


- **A `raise` is a declaration, recorded at publication and not checked.** `publish_review` and
  `republish_review` record the raise, its reason, the focus, its reason and the computed tier on the published
  marker, `review_read` with `view: transaction` answers them, and `policy_for` lifts the gate's tier to `high`
  for a recorded item from the list. Nothing checks that the item applies to the diff. A raise made on
  `review_tier` alone is not stored; only the publication carries it. A repair publication that does not repeat the
  raise is sized by its diff alone, since the declaration is read from the receipt's own marker and not from its
  parent's, and a raise on a repair takes the whole change's `high` panel rather than the narrower repair panel.
