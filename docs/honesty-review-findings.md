# Findings, grants and locations

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what a recorded finding, a grant and a location do not establish. The index says which area holds which question.

## Findings, grants and locations

- **A structured finding is a legible claim, not a checked one.** `record_review_finding` refuses a
  class outside `severe`/`warning`/`suggestion`, a finding missing its identity, evidence or impact or
  supplying filler for either of the last two, a severe finding whose `evidence_class` or
  `causal_disposition` is absent or outside its vocabulary, a receipt that is not the current one, and —
  across a repair — a parent identity the parent receipt never recorded or a new severe finding stating
  no origin. The computed verdict reads the ledger those gates keep honest.

  Every one is a check on **shape and reference**. Nothing here can check that a `severe` finding is
  severe, that a `suggestion` is not a defect in disguise, that the evidence reproduces, that the impact
  is real, or that a repair `introduced` what its reviewer says. A reviewer that classifies every
  preference as `severe` clears the rule as a careful one does. The two severe declarations are checked
  for presence and vocabulary alone, so a severe finding declaring `insufficient` and `unknown` has
  satisfied the gate while saying neither (though the guard refuses a `rejected` verdict whose severe
  finding declares `insufficient`).

  Narrower limits:

  - **A finding identity is a spelling, not a comparison.** Two markers with the same `id` name one
    finding; two lenses describing one defect differently record two. Nothing computes agreement from
    identities. The policy asks reviewers to derive the identity from the affected behaviour and
    location; nothing measures whether they did.
  - **The fix-delta digest names a repair; it does not scope a review.** A publication over an earlier
    one records the whole parent receipt and a digest covering both ends, derived from the timeline.
    The targeted validator (payload built in `src/transport/claim/capture.rs` from `delta_parent` and
    `review_payload_declared_at`) and every tier lens on a correction whose recorded range is honest
    (`delta-range` present, no `delta-range-reason`, not a recovered successor, same pull request) get
    `parent-head..head` with that payload's digest; a lens on a dishonest range, a marker written before
    `delta-range` existed, or a first publication gets the whole `base..head` (`review_payload`), as does
    the refuter. Each fallback and each of the four conditions `lens_reads_delta` joins is held by a
    capture test that fails when the condition is removed. When the parent head cannot be resolved the
    capture refuses `read-failed` and launches no lens. A lens handed the delta is not prevented from
    reading more, and a lens finding is checked against `base..head`, not the delta. `publish_with`
    shares `repair_range` with `review_tier_for_repair` to record whether the range was honest
    (`delta-range`) or why not (`delta-range-reason`: `history-rewritten`, `empty-repair-range`,
    `base-advanced`, `merged-range`). Whether a reviewer reads that field is unmeasured: the criterion is
    met by the *record*, not an enforcement.
  - **A lineage is silently absent when the parent publication came from another account.**
    `latest_publication` keeps only comments `viewerDidAuthor` vouches for. `republish_review` refuses
    (`published-transaction-missing`); `publish_review` does not, so a continuation after a cross-account
    reclaim publishes as a **first** publication and neither lineage rule applies to its findings:
    permissive rather than a wrong delivery.
  - **`introduced` and `exposed` do not cover a severe defect that was always present, always
    reachable, and simply missed.** Against a repair publication such a reviewer has no honest origin
    word: `--parent` names nothing recorded, both origin words are false, a warning means `rejected` is
    refused, and recording nothing leaves a real defect unable to block. Measured on epoch
    `3ac4a576aa80ec96bc42ce4283e94f93` by one of five judges (no quorum). Widening the vocabulary is its
    own issue.

  **Judge isolation is unproved.** Judges run only as Judgment Day. The role gate is fed the *embedded*
  reviewer definition rendered with the effective `Evidence standard`, and
  `the_shipped_blind_reviewer_gets_the_grant_its_standard_decides` pins both standards: `Write`, `Edit`,
  `Agent` and `Task` denied under either, `Bash` following the row. Under `reading` two judges cannot
  write over each other; under `measuring` they can by design and the isolation rule answers it. A panel
  this harness cannot place in the role is invisible both to the reserved-role prelaunch check and to
  that gate. Nothing here records how many such judges ran,
  whether two of them shared a working directory, or what they wrote into it. The isolation rule in
  `skill/policies/judgment-day.md` is prose an orchestrator follows or does not, on the same footing as
  panel size and blindness, which [`honesty-review-panels.md`](honesty-review-panels.md) records as not observed.

- **A non-severe finding is informational, and nothing about delivery waits on it.** The
  `disposition-missing` gate is retired, following `gentle-ai` v3.7.0
  (`internal/reviewtransaction/advisory_disposition.go`, read at `e2cb2bf2`): a `WARNING` or
  `SUGGESTION` "requires no action of any kind". `record_disposition` remains but is optional and may
  answer a subset; an unrecorded finding is delivered as informational by class alone. The recorder
  checks **shape**: the id names a finding this exact receipt recorded, no identity whose every recording
  is `severe` is named (one recorded at two classes is answered for its non-severe recording), a `filed`
  entry carries an issue number, and a reason is neither empty nor filler. Nothing checks that an
  `acted-on` repair happened, that a `filed` number names a real issue, or that a `declined` reason is
  real. The answer is the **author's own** and requires no distinct context, so the record is auditable,
  not independent. The pre-change disposition ratio (18 closed issues: 30 `severe`, 232 `warning`, 69
  publications, 14 rejected verdicts) was never re-derived; the "share of publications spent on changes
  no reviewer rejected" figure is owed to whatever re-measures repair rounds under the informational rule.

- **A sealed reviewer's grant is enforced by this gate, and what the gate reads is never the file on
  disk.** `reserved_reviewer` answers which of nine reserved names carries which seal. Both CLI and
  hook render the embedded `reserved_reviewer_row` template and seal.
  `a_tampered_sealed_definition_does_not_widen_what_its_doors_allow` replaces disk tools with
  `Read, Grep, Glob, Bash` and still requires both doors to refuse `Read`; removing the sealed name
  reddens it. Nine reserved names comprise two installed judges and seven binary-launched reviewers;
  seven rows are sealed. Actual launch composition remains unobserved.

  Claude Code's 2026-09-02 probe found `WebSearch` launched with exactly one tool and no filesystem
  reach; this is the sealed rendering. It still reaches the public internet without waking the gate.
  Other hosts were not measured. `none` and `TodoWrite` were refused as "unrecognized";
  `ReportFindings` and `ListAgents` as "not available to subagents". Both `[]` and `ToolSearch`
  launched with zero tools, but `tools_line` reads `[]` as no policy (all permitted), and
  `ToolSearch`'s purpose is fetching schemas.
  **Unmeasured:** host enforcement of grants; names sent by the two matcher-less hosts; project-shadow
  refusal outside Claude; and whether a judge stays inside its supplied evidence. `Read`, `Grep` and
  `Glob` wake none of the matchers this repository writes (`src/setup/render.rs`), so the gate cannot
  enforce their absence even though embedded rendering is tested.

- **`reviewer_is_static` keeps the plain cross product `Evidence::all()` × every seal
  `RESERVED_REVIEWERS` carries; no reachability predicate narrows it, and that is a decision.** Byte
  acceptance follows the renderer, not the settings table. Setup's path-based
  `validate_reviewer_definition` runs two passes, one per installed row, and can nevertheless refuse
  a safer sealed body at the unsealed path.
  Measured: an owned `~/.claude/agents/review-blind.md` containing the sealed rendering passes the
  unsealed pass, then refuses `reviewer-definition-unowned` in the sealed-name pass before writes.
  Setup and sync stay blocked until `move <path> aside before setup`; static-byte acceptance does not
  establish path ownership.

- **A location checks consistency with the diff, not causation.** `record_review_finding` refuses a
  severe finding whose `causal_disposition` is `introduced` or `worsened` and carries no `location`. A
  value that does not parse as `path:line` or `path:start-end` with a repository-relative path is
  refused under any class or disposition. Hunk membership is checked only where a location is owed
  (`severe` with `introduced` or `worsened`) and proves only that the diff touched that line; the check
  refuses the **impossible** and is silent on everything merely wrong. `warning` and `suggestion` never
  owe a location, but a malformed one is still refused. `pre-existing` and `base-only` are unverified by
  construction: a `pre-existing` finding may name a line the change never touched, because refusing it
  would reward withholding the coordinate.

  **The predicate is containment, not overlap:** a range settles only when one hunk's range contains the
  whole claimed range (`range.start() <= claimed.start() && claimed.end() <= range.end()`), and the
  refusal names the first unchanged line inside the span. Measured over the tracker's 885 recorded
  `review-finding` markers, 14 were in the gated class and 5 carried a span; 2 would fail containment
  (both on PR 209, one unchanged line each).

  **A lens is told the rule; one location outside it still costs the whole answer.** The lens
  prompt's answer format (`answer_section` in `src/reviewer/prompt.rs`) says a causal `location`
  names lines of one unbroken run of `+` lines, not a span from one run to another, because the
  payload is a `--unified=3` diff and the check reads `--unified=0` runs: two edits a few lines apart
  read as one hunk to the lens and as two here. Nothing makes a lens follow it, and a refused location
  is answer-scoped (`ANSWER_SCOPED` in `src/transport/claim/capture.rs`), so `collect_review_input`
  discards every finding in that answer and the lens is launched again. The Judgment Day judge
  definition (`skill/agents/review-blind.md`) does not carry the sentence. Measured 2026-11-28: a
  `review-reliability` lens located one finding in `release_ci` across two runs of `+` lines 25
  unchanged lines apart, on three launches in a row.

  **Containment assumes separated new-side hunks.** `changed_new_side_lines` uses `--unified=0`,
  `--no-renames` and `--inter-hunk-context=0`. Four modify/delete/keep fixture shapes in one repository
  produced `2`/`4` or `2`, never adjacent `2`/`3`; this probes, not proves, the precondition. Split ranges
  over-refuse; fused ranges wrongly admit unchanged lines. The invocation pins inter-hunk context,
  removes `GIT_DIFF_OPTS` in `super::run`, and disables external diff/textconv. Measured hazards:
  `GIT_DIFF_OPTS=-u3` widened a one-line hunk to seven; `diff.interHunkContext=5` fused edits at 3/7
  across unchanged line 5. These fixtures do not exhaust Git's possible hunk shapes.

  **A path the diff touched but that carries no new-side hunk settles by having been touched at all** (a
  pure deletion, deleted file, binary or mode-only change): an empty range vector settles by the exact
  `diff --git a/<path> b/<path>` header having appeared; no header settles nothing.

  The pathspec is `:(literal)<path>`, not `<path>` after `--`, because pathspec magic like `:!x` selects
  every other path. `:(literal)` still prefix-matches directories (measured on git 2.55), so only hunks
  under a header naming **exactly** `path` are kept. `parse_finding_location` refuses any path containing
  a colon, which also rejects `path:line:col`.

  **Renames admit broad locations.** One-path filtering precedes rename detection (Git 2.55): old path
  is deletion-only, admitting any line; new path is an addition, admitting every file line.
  `--no-renames` pins that treatment rather than proving a cited line caused the defect.

  **The diff is read locally**, from `context.repo_dir`. A checkout missing the objects for `base` or
  `head` meets a failed read naming both revisions and the path (`super::run`'s detail is truncated to
  the first three arguments, so `changed_new_side_lines` adds them back). No test drives the missing-
  objects shape, only a directory holding no repository.

  **The header pin covers all four of git's prefix variables.** `pinned_diff_args` pins
  `diff.mnemonicPrefix=false`, `diff.noprefix=false`, `core.quotePath=false`, `diff.srcPrefix=a/` and
  `diff.dstPrefix=b/`: a repository with only `diff.srcPrefix` set rendered `diff --git x/<path>
  b/<path>`, `touched` stayed `false` and a correct location was refused
  `finding-location-outside-the-change`. `--default-prefix` is not used because it needs git ≥ 2.41 and an
  older git would silently ignore the unrecognised `-c` keys.

- **A verdict's basis is a declaration, not a check.** `basis` is optional on the retired verdict
  operation, stored as sent, never re-run. A rejection rests on a recorded `severe` finding, not on
  `basis`. An accepted verdict with zero findings and no `basis` is well-formed: a `reading` judge has no
  commands to name, and Estigia cannot tell that from a `measuring` judge that skipped stating one. What
  changed is that "measured and said so" versus "said nothing" is a field on the timeline.

- **A recorded `spend` is a declaration, not a measurement.** Estigia cannot see a subagent's token count,
  wall-clock seconds, or how many contexts produced a figure. `input`, `tokens`, `seconds` and `contexts`
  are supplied numbers stored and read back; nothing confirms them, a run that under-reports is
  undetectable, and `conditions` is free text trusted the way `basis` is. The orchestrating run's own
  consumption is the sharpest gap, since a context cannot read its own count; the hand-measured cost table
  says its task-flow layer is not instrumented and its total is low by an unmeasured amount. `input` and
  `tokens` stay separate fields because dividing one by the other would look like a measurement while
  resting on the same unverified declaration twice.

- **The crossing test proves presence, not consistency.** The requirement has one home,
  `skill::VERDICT_BASIS_REQUIREMENT`. Three of seven surfaces carrying it are rendered from those bytes
  (both reviewer-definition arms and the OpenCode review block); four are hand-authored (the policy, the
  MCP tool schema, the contract's developer step, the operations companion's `Recording a verdict`
  section). `the_verdict_basis_requirement_reaches_every_generated_launch_text` crosses all seven; two of
  the four are checked against the region that must carry it. The count of carriers checked that way is
  prose, here and in `CHANGELOG.md`, and nothing crosses it. `str::contains` does not catch a surface that
  keeps the requirement and contradicts it elsewhere (which has produced real defects, including two
  inside crossed carriers) nor one that discusses `basis` without being a carrier.
  `the_stale_prose_rule_and_its_carve_out_reach_both_carriers` asserts within sections (`Review contract`
  in the definition, `What a finding is` in the policy); the `skill/SKILL.md` paraphrase is deliberately
  uncrossed.

- **A frozen-body ledger cannot be a definitive fix.** `REVIEWER_TEMPLATE_BEFORE_*` in `src/setup/mod.rs`
  requires a human to freeze each superseded body before release; omission leaves nothing a test can
  detect. `write_reviewer_definition` also records the exact path's digest through
  `written_outside`/`note_written_outside` in `src/skill/record.rs`, falling back to the ledger when
  absent. Neither rescues a body stranded without either record (`reviewer-definition-changed`),
  and skipped setups leave no digest for intermediate published renderings.
  `historical_reviewer_renderings` spans every frozen template and reserved `(name, seal)` pair;
  digest acceptance does not make bytes current for `reviewer_is_static_for_seal`.
  Single-writer ownership is unenforced: one Claude skill root's record is asked; another adapter
  sharing its directory under another root would be invisible. One machine's 2026-09-05 `review-lens`
  measurement confirmed exit 1→0 and all seven digests bootstrapped despite `changed 0 file(s)`;
  it establishes neither retroactive rescue nor multi-writer safety.
