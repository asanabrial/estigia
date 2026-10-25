# The review tier, the lens and the finding

The limits on the cost and FLOW measurements are in
[`review-cost.md`](review-cost.md#what-these-measurements-do-not-establish). What follows is what the
review tier, the lens and the finding do not establish.

- **A `never-ran` verdict is recorded evidence, not verified evidence.** Read through
  `record_computed_verdict`: it approves under a publication lane whose failing runs report
  `steps: []` on every job only when the caller supplies `--never-ran-evidence-file`, and it stores
  that file's content in the marker verbatim, on the same footing as a finding's `evidence`. Estigia
  does not confirm the account is billing-blocked, does not re-run the local check the file
  describes, and does not confirm the jobs listing it read is the one the caller looked at.
  `steps: []` proves only that GitHub's jobs API reported no steps for that run when Estigia asked; a
  caller who typed a false claim into the file clears the same gate a true one does. The check-run
  annotation (`repos/<owner>/<name>/check-runs/<id>/annotations`) is carried into the refusal to tell
  an operator why nothing ran and is never the predicate, because an annotation is prose a platform can
  reword. `classify_reds` excludes a `cancelled` conclusion before asking a run's jobs anything, so a
  run cancelled before any step cannot reach the branch; every other non-green conclusion with
  `steps: []` is still trusted at face value, and the branch does not distinguish a platform outage
  from any other way a run's containers report no steps.

- **Nothing detects a launch prompt that overrides the judge's classification rule.**
  `skill/agents/review-blind.md` holds the rule for choosing between `severe`, `warning` and
  `suggestion`. A launch prompt that supplies its own rule replaces it silently: a sub-agent's prompt
  is not a write the gate sees and no test reads one. The repair is an instruction to prefer the
  definition and record the conflict, which is enforcement by the reviewed party alone; Estigia cannot
  prove a judge applied the definition rather than the brief. One review spent two full-panel
  rejections on four `severe` documentation findings graded under a prompt-supplied rule, a figure not
  re-derivable because the receipts were not retained with a method.

- **`review_tier` carries two named gaps against `ClassifyRisk`, and one widening.** It ports the
  hot-path tokens, `isServiceTokenReviewPath`, `isShellReviewPath` and `executableModeChanged`, four
  producers reaching `auth`, `update`, `security`, `payments`, `permissions` and `shell_process`. The
  content scan (`processBoundaryRiskReasons`, in `src/transport/tier/content_scan.rs`) is ported and
  reads `high` on an added process-spawn construct, a `#!` line in either tree, a scan past
  `SCAN_BYTE_BUDGET` or an unattributable diff (`process_scan_limit`), and the ten dangerous-sink
  families. Where it differs from the source project:
  - **Matching.** Go regexes and `git grep -E` become the `regex` crate and a line test over the blob.
    Every catalogue pattern is unchanged, but `\b`, `\w` and `[^...]` are Unicode-aware here and
    ASCII-only in Go, so a non-ASCII identifier can match here and not there. A blob containing a NUL
    is skipped, standing in for `git grep -I`.
  - **Interpreter directive.** Only the first line of a blob, in either tree, is read, and a first line
    starting `#![` is not a directive. The source project's `git grep '^#!'` matches any line, which
    made every change touching a Rust file with an inner attribute read `high`. The cost is a
    directive on a later line, or a script whose first line is `#![`.
  - **Signal.** A sink reason carries its family (`tls_verification`, `unsafe_deserialization`, `eval`,
    `shell_composition`, `weak_credential_hash`, `credentialed_cors`, `world_writable`,
    `unverified_jwt`, `csrf_opt_out`, `xxe`) as its `signal`, where the source writes `dangerous_sink`.
    `canonical_risk_reasons` keeps one reason per `(code, signal)` for the whole change, so the
    published list names a family once, on whichever path sorts first.
  - **Bound.** The blob-size total is bounded before any blob is read; the added-line diff only after
    it was read, and not by the blobs, so a diff can exceed the blob total. Over the 8 MiB budget, or a
    diff header whose path cannot be attributed, answers `process_scan_limit`. A blob or diff `git`
    cannot read is a read failure and answers no tier.
  - **Test paths** are not scanned, so a spawn or sink added only in a test reads nothing. The rules
    are the source project's (a `test`, `tests`, `__tests__`, `testdata` or `spec` segment, a
    `_test.go` or `_test.py` suffix, a `.test.` or `.spec.` infix, a `test_*.py` name) plus two this
    port adds for Rust: a basename of `tests.rs` and a `_tests.rs` suffix.

  `data_exposure` and `data_loss` are declared in the eight-name closed signal set but nothing produces
  them: `ClassifyRisk` has one production call site in the source (`risk.go`) passing signals computed
  there, and grepping its tree finds the names only in two `switch` statements that render a meaning.
  They appear dead there too, and their absence here narrows nothing reachable.

  `isStaticMDXDocument` drops the source's per-line bound (`bufio.Scanner` capped at 1 MiB per line,
  answering not-static on overflow). This port reads the whole string with `str::lines()` and relies on
  the 8 MiB total budget. A single-line `.mdx` document over 1 MiB is proven not passive there and
  answers `medium` with one reviewer; here it can be proven passive and answer `low` with none, a
  missed reviewer.

  **The source's six-path self-reference, `isNativeReviewAuthorityPath`, has no counterpart here**,
  measured rather than assumed. Substituting this repository's gate files (`src/harness/mod.rs`,
  `src/harness/guard.rs`, `src/harness/roles.rs`, `src/harness/tracker.rs`, `src/transport/claim.rs`,
  `src/transport/ownership.rs`) answered `high` for all three real deliveries tried, every one on
  `src/transport/claim.rs`, which was touched in 32 of the last 40 commits (`src/harness/mod.rs`, the
  next busiest, in 24). That made the classifier constant on this repository's history. Dropping
  `claim.rs` for being touched often would exclude the file holding `record_computed_verdict` and
  `release_ci`, so the producer is removed until the review authority is separable by filename. This
  gap narrows what reaches `high`.

  One property is a widening. `isGeneratedGoldenPath` gates a generated `testdata/golden/*.golden`
  fixture out of risk classification at seven call sites in the source project (`CountChangedLines`,
  `AssessSnapshotRisk`, `activePassiveContentPaths`, `deriveSnapshotRiskReasons`, `fallbackRiskReason`,
  `isInspectableStat` and `touchesHotPath`, plus `SnapshotBuilder`'s own marking). All seven have
  counterparts here (`count_changed_lines`, `review_tier`, `active_passive_content_paths`,
  `derive_snapshot_risk_reasons`, `fallback_risk_reason`, `is_inspectable_stat`, `touches_hot_path`)
  and none gates on a golden path, because this repository has no such convention. A path shaped like
  a golden fixture answers `high` here and would not there; it never turns `high` into `low`.

- **`review_tier` reads two tree-to-tree `git diff` calls and never the worktree, and the tier-licensing
  sentence is held by review alone.** Both ends are resolved to commits (the two `pinned_diff_args`
  calls in `src/transport/tier.rs`), so uncommitted work is invisible to the tier. The routed path does
  not pay that: the orchestrator reads `review_tier` after `publish_review`, whose `clean_target`
  (`src/transport/target.rs`) refuses to publish while any worktree byte is uncommitted. Called
  directly against an arbitrary worktree the blind spot is real. `Context::get` still reads a rendered
  default the same as a written answer for `worktree location` and `Review budget`, through the merge
  in `Context::live`; `get_written` answers only rows somebody wrote and has no caller left.

  The lens counts the tier licenses are `0`, `1` and `4` for `low`, `medium` and `high`, and
  `review_tier` answers no `judges` key at any tier; `the_review_tier_answer_carries_no_judges_at_any_tier`
  pins that and `no_text_an_agent_acts_on_for_ordinary_review_acquires_judges` holds the contract, its
  rendering and every other text under `skill/` to it. The rendered carriers are crossed
  (`the_two_src_only_carriers_render_the_lens_counts_the_tier_derives_and_no_judges`) and
  `the_lens_count_a_publication_acquires_is_the_one_review_tier_answers` crosses
  `skill/policies/blind-judges.md`'s "Lens acquisition" section, but the contract sentence in developer
  step 5 is not itself crossed. Judges exist only as Judgment Day, which licenses nothing in the
  transport and records nothing. **That a Judgment Day finding is never recorded with
  `record_review_finding` is stated in `skill/policies/judgment-day.md` and not enforced**: a finding
  recorded against a receipt would enter the ledger like any other.

- **`RiskLevel::lenses` scales the review; nothing proves the licensed lenses ran.** `release_ci`
  requires the computed verdict at every tier, and the verdict refuses `approved` without a lens report
  for every role the tier licenses, but a run records that report on the lens's behalf and cannot be
  shown to have launched it (the lens-report entry in `docs/honesty.md`). A `high` change reviewed by
  two lenses instead of four is indistinguishable at the transport from one reviewed by four unless a
  report is missing. `no_carrier_of_the_tiers_allocation_states_a_judge_count` proves the carriers (the
  two MCP tool descriptions, `skill/SKILL.md`, `skill/policies/blind-judges.md`,
  `skill/references/runtime-notes.md`, `skill/bindings/github.md`,
  `skill/bindings/github/operations.md`, `README.md` and `docs/configuration.md`) state no per-tier judge
  count; it does not prove an orchestrator reads any of them before acquiring the lenses.

- **A repair's tier is derived from a range that is honest only when the timeline can prove it, and
  nothing verifies that the orchestrator reads that range rather than the whole change.**
  `review_tier_for_repair` takes commit-ish arguments and calls [`repair_range`](../src/transport/tier.rs),
  the one function `publish_with` also calls to record which case a publication landed in. When the
  parent publication's head is still an ancestor, the range is `parent-head..head` (`range: "repair"`,
  `delta-range` recorded verbatim). Otherwise it is `range: "whole-change"` with `whole_change_reason`
  (and `delta-range-reason` on the marker and the answer) one of `history-rewritten`, `base-advanced`,
  `merged-range` or `empty-repair-range`; a base advance merged rather than rebased is `base-advanced`
  and a side branch merged back inside `parent_head` is `merged-range`. In the narrow case the answer's
  `"base"` carries `parent_head`, since that is the range measured. The guard is not `correction_budget_check`'s
  `is_ancestor(merge_base(base, head), parent.head)`, which answers a different question; the budget's
  `base_advanced_past` reads the same fork point from the parent's base. Branch protection forces an
  integration whenever `main` advances during review, so **the saving does not apply to a rebased
  repair round**, and that bound is not to be softened.

  What this does not close:
  - **Nothing verifies the call.** An orchestrator that reads `review_tier` over `base..head` on every
    round gets a well-formed answer, only a larger one than the rule intends. `skill/policies/blind-judges.md`
    and `skill/bindings/github/operations.md` state which range to read; nothing reads it back.
  - **The range is necessary, not sufficient.** Run `claude-57cb079c6abe0a87` delivered
    `d75407ff..31b2df54`: seven files, fifteen insertions, fourteen deletions, three `.rs`, two of those
    only `///` comments. Without comment lines the repair is **two** lines: a string literal in
    `src/setup/tests.rs` that is a shipped paragraph byte for byte, and a registered byte count in
    `tests/instruction_ceiling.rs`. Both are `.rs`, so the repair classifies `medium` over its own
    range. The cause is this repository's guards, `the_default_rendering_is_the_bytes_already_installed`
    and `documents_stay_under_their_registered_byte_ceilings`: the two mechanisms that make a prose edit
    trustworthy are the two that stop a prose repair looking like prose. `tests/instruction_ceiling.rs`
    registers `skill/**` only, so a repair editing shipped contract text is policed twice where one
    editing `docs/` is not. What share of real repairs falls on each side is not measured. Admitting
    either as passive would carve a hole in the classifier to make an answer come out right, so the
    classifier is believed and not changed.
  - The recorded range does not compute the diff, hand it to a judge, or restrict what one reads, the
    limit the fix-delta-digest entry in [`honesty.md`](honesty.md) states.

- **The issue identity is readable for open `analysis` findings only, and the collision is not
  mechanised.** The identity lives in the `analysis` control marker, not the title, and
  `list_identities` reads it from the local store's `issues.identity` with the open issues from one
  REST list: `null` for an issue that recorded none. It does not read a closed issue's identity (the
  listing is `state=open`), does not read the `review` marker an issue filed with `--epoch` carries
  (recorded in the store, not rendered into the body, read by nothing), and does not refuse a `create`
  whose identity exists. An analyst can dedup by the recorded key; the collision stays the analyst's
  to judge.

- **A lens is a sealed reader with one narrow question, not a third judge.** Sealing a judge cost two
  severe findings that rested on files no hunk touched; a lens has no such scope to lose, because its
  question, *does any sentence this change adds contradict any code this change adds?*, is answerable
  from the diff by construction. The question was derived by classifying the 885 `review-finding`s this
  tracker held on 2026-09-02 by identity slug, a keyword classification and approximate: 436 of them
  (63 `severe`) read as prose disagreeing with code beside it, 36 as behaviour.

  `review-lens` is a reserved row and rendered definition (`RESERVED_REVIEWERS`,
  `skill::AGENT_DEFINITIONS`). `record_review_finding --lens` is validated against
  `crate::harness::roles::reserved_lens` (five shipped roles) rather than minted from caller text, and
  every lens finding owes a `location` whose path `base..head` changed, refused as
  `lens-finding-location-required` or `lens-finding-location-outside-the-diff`. That check is the path
  alone; a lens finding that is also `severe` with `introduced`/`worsened` owes the line containment
  every severe causal finding owes, refused as `finding-location-outside-the-change`. The five-lens set
  is a divergence: the source project ships four, and `review-lens` has no counterpart there. The tier
  allocates the source's `0`/`1`/`4`; only the delta-lens repair round launches `review-lens`.

  A lens decides its own severity. What decides whether a finding blocks is `evidence_class`
  (`FINDING_EVIDENCE_CLASSES`): `deterministic` qualifies a rejection alone, `inferential` needs
  corroboration, and `insufficient` is recorded and carried but never blocks. The corroboration is
  `review-refuter` (a reserved sealed row with its own template, `skill/agents/review-refuter.md`):
  `record_refutation` records its one batch per receipt (`corroborated`, `refuted` or `inconclusive`
  per finding, never one task per candidate), and the computed verdict drops a finding recorded
  `refuted`.

  Divergences and limits:
  - The source's `transaction.go` binds a lens finding's declared `Lens` to the lens it launched and
    rejects the result on mismatch. `--lens` here is checked for membership only; `record_review_finding`
    is handed a completed finding and has no channel to which lens was launched.
  - **What `review_findings` answers is not checked.** It walks `first_protocol_markers`, which keeps
    only a marker whose comment is `unedited`, so an edited finding comment is not answered and cannot
    be told apart from one never recorded. It derives `blocking` and two byte counts; everything else
    is the reviewer's declaration. `FINDING_TEXT_CEILING` (32,768 bytes) is chosen inside the
    observed-clear half of the MCP surface's band, answers carrying 73,272 and 145,756 payload bytes
    refused and 36,288 not (`Publication::inline_payload`'s doc comment in `claim.rs`), not measured
    against where `review_findings`'s own answer stops fitting.
  - **Estigia cannot prove a lens actually ran**, was launched sealed rather than with a wider grant,
    saw only the diff it was handed, or ran concurrently with the others. Nor can it prove which context
    produced `evidence_class` and `causal_disposition`: what reaches `record_review_finding` is
    whatever the orchestrator passes. `RiskLevel::lenses` and `--lens` validation are enforced in code;
    the launch that turns a count into a subagent call is read by an agent from prose.
  - **The five lens definitions' stale-prose floor is written and not crossed.** They carry the
    never-`severe` treatment of stale prose and its carve-out, but
    `the_stale_prose_rule_and_its_carve_out_reach_both_carriers` (`tests/payload.rs`) crosses
    `STALE_PROSE_NEVER_SEVERE` and `CARVE_OUT_BOUND` (`src/skill.rs`) against the two judge carriers
    only. The lens copies are paraphrases no test crosses, so nothing keeps them agreeing with the
    constants or each other.

- **A lens count is a fixed allocation per tier, not a risk dial, and `RiskLevel::lenses` cannot
  silently answer zero for a new tier.** A lens is one narrow question, and N copies of it measure how
  consistently an agent answers it, not how much more of the change gets looked at. So `1` for
  `medium` is the lens the caller's `focus` names (`reliability` by default); `4` for `high` is
  `review-risk`, `review-readback`, `review-readability` and `review-reliability`, four distinct
  questions; `0` for `low`, because a change proven to add no code leaves nothing for a lens to be
  about. The answer's `lens_roles` names which. The source's resilience slot is filled by
  `review-readback`, whose question is re-derived rather than ported (`src/skill.rs` says why), and its
  readability override for large pure documentation has no subject (`DominantLens` is declared "no
  longer derived" in `risk.go` and only read to replay an old authority), so a large pure-documentation
  change is `low` here. `RiskLevel::lenses` takes no `Context` and answers a plain `u8`;
  `the_lens_count_stated_beside_review_tier_is_the_one_the_tier_derives` crosses the MCP tool
  description and `skill/bindings/github.md`'s row against it.

  `no_shipped_surface_still_claims_nothing_launches_a_lens` reddens if a shipped surface says nothing
  launches a lens. It catches only the phrasings in its `CLAIMS` list and only in the files it reads
  (this file is excluded wholesale, and `skill/references/runtime-notes.md` and `docs/configuration.md`
  are tolerated only with the owner clause, `OWNER_REFERENCE`, within `OWNER_WINDOW` bytes of the
  claim). A sentence worded some other way passes, and two independent readers by grep each missed
  `skill/references/runtime-notes.md` once.

- **The severity floor, the `evidence_class` routing and the `pre-existing`/`base-only` non-blocking
  convention are stated in prose, and nothing in the harness enforces them.** The source's floor is
  that only BLOCKER/CRITICAL findings that survive adversarial verification enter the fix loop, and
  WARNING/SUGGESTION findings are reported once and never block. Mapped onto this binding's one
  blocking class, `severe`, it is `skill/policies/blind-judges.md`'s "Severity floor" subsection. No
  refuter is wired into Judgment Day, as in the source ("its two-judge convergence satisfies
  adversarial verification and it spawns no `review-refuter` tasks"); ordinary review has lenses only,
  and a lens's `inferential` severe finding follows its refuter. The convergence statement (a bound on
  fix rounds, then reported as open to the person) already held in developer step 5 and the policy's
  repair paragraph, now as a spent correction rather than a ceiling. The measurement behind stating the
  floor as a named rule: across two issues in one session, four judges recorded zero severe findings and
  three repair rounds were elected anyway, roughly 570,000 tokens re-reviewing prose no judge had blocked
  on. The rule that would have prevented it was written and not followed, a reading discipline and not a
  missing gate.

- **The first cut crosses three symbol classes `skill/` names against the code that owns them, and two
  more are not taken.** The extractor `crate::markdown::markdown_inline_code_spans` reads every inline
  code span in a Markdown document, skipping fenced blocks (``` or ~~~) and ignoring prose outside
  backticks, so the population is derived from `skill/` on every run. It lives in `src/markdown.rs` and
  not beside `source_text::code_of` because `build.rs` compiles `source_text.rs` standalone
  (`#[path = "src/source_text.rs"] mod source_text;`), where every Markdown function would sit unused,
  red under `-D warnings`.
  - **Refusal codes, the direction nothing crossed before.**
    `every_refusal_code_skill_names_is_one_the_crate_can_answer_with` is the reverse of
    `every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer` (named in
    [`honesty.md`](honesty.md)): a code renamed in the transport would otherwise leave a table row
    pointing at an answer that can never arrive. Re-measured by walking
    `markdown_inline_code_spans` over every `skill/**/*.md`: 184 distinct lowercase-kebab spans, 117
    resolving as exact literals. Every pair written earlier drifted under later `skill/` edits, so
    these are a reading somebody re-runs, not a number the suite holds (the guard in `tests/honesty.rs`
    floors `read` and `candidates.len()` against the walk's own counts).
    `closed-by-own-delivery` resolves by a `"closed-by-own-delivery:` prefix, because it is built with
    `format!` and never spelled as a literal; the prefix reader cuts the source where `build.rs` does,
    at an inline `#[cfg(test)] mod tests {`. The rest are workflow states, setting values, agent names
    and operation verbs, subtracted by extracting each from its own source (`config::STATES`,
    `Setting::answers()`, the `skill/agents` listing, `transport::commands::SCRIPTED`). What is left
    is the data table `NOT_A_REFUSAL_CODE` (44 entries, each with a one-line reason), checked for
    staleness in both directions.
  - **The check is scoped to the transport's vocabulary.** `TRANSPORT_VOCABULARY` is generated from
    `src/transport/` only; roughly 28 refusal codes `src/harness/**` answers with directly are not in
    it. None is named in `skill/` today, but one that were, such as `harness-not-installed`, would be
    reported unknown and fail, the wrong verdict for a real code. That is the harness half of the
    hole the mapping pass found, *none of the gate's own refusal codes is documented in `skill/`*, and
    it is not closed.
  - **Workflow states, both directions.** `STATES` against `skill/SKILL.md` already crossed forward; the
    reverse now extracts the contract's *"Keep exactly one of `analysis`, `ready`, `in-progress`,
    `review`, `blocked`, or `done`"* sentence through the same extractor, in
    `the_states_a_routing_accepts_are_the_ones_the_contract_declares`.
  - **Configuration rows.** `every_row_the_shipped_config_block_names_is_a_real_setting_label` reads
    `skill/SKILL.md`'s shipped table through `config::table_rows` and checks each label against
    `Setting::label()` exactly, never the alias-tolerant `Setting::from_label`. The shipped block carries
    9 of the settings table's 26 rows; `Integration mode`, `Renewal window` and
    `Irreversible commands` are never mentioned in `SKILL.md`, and nothing requires every setting be.
  - **Not taken.** Tool names (MCP operation names, `contract_name`s, JSON answer fields, GitHub event
    names) are spelled two ways, mostly backticked `snake_case`, which the kebab filter never admits,
    and a few kebab (`parent-pr`, `parent-head`, `parent-base`, `parent-digest`, `payload-digest`),
    which is why they sit in the exemption table; an extractor over either spelling alone sees part of
    the class. Command verbs are one distinct verb (`estigia setup`), and a guard over one verb
    protects nothing.
  - **Adjudicated by a test that reddens on drift:** refusal codes both directions (59 frozen
    exceptions on the code side), workflow states both directions, the shipped config block's labels,
    the four finding vocabularies against every reviewer definition that records a finding, MCP
    operation names, agent definition names, lens and reviewer counts, and the per-file byte ceiling.
    **Not adjudicated by anything:** the gate's own refusal codes against `skill/`; the
    `SCRIPT`-verb-to-dispatcher crossing and the transport CLI flags inside `bindings/github.md`; the
    four vocabularies against `blind-judges.md`; the exit-code table's meanings (the number set is
    checkable, `4` is unchased against `Failure::code`); the literals `262144`, `150000` and the `20`
    card cap; configuration row value words (`direct`, `ask`, `branch`), which are ordinary English;
    and the severity floor, Judgment Day's convergence rule and the repair-ceiling narrative.
  - **The finding-vocabulary crossing is a presence check.** One test walks the seven
    `RESERVED_REVIEWERS` rows that record findings and asserts each value the transport accepts appears
    in the definition's template bytes (`render_reviewer_agent` substitutes three literals none of the
    values is a substring of, so a value in the template is in every rendering). On the rendered body it
    asserts only the fields `record_review_finding` reads, `origin`, `evidence_class` and
    `causal_disposition`, once per evidence grant for the unsealed row and once for the six sealed rows.
    The vocabularies are read from their `pub const`s, the list is pinned at four and pairwise distinct,
    and the row count is pinned. It cannot attribute a value to a field (`introduced` belongs to both
    `FINDING_ORIGINS` and `FINDING_CAUSAL_DISPOSITIONS`); it cannot see a value removed from one place
    while it survives in another (deleting the `severe` row from the judge's classification table
    leaves it green, because the word appears at eight other points); and it crosses one direction
    only, so a vocabulary that shrinks leaves stale prose behind. `review-refuter` is outside it,
    adjudicating through `record_refutation`, which carries no causal field.

- **A finding filed as its own tracker issue names its receipt, and Estigia cannot see whether filing
  happened.** `create` takes optional `--epoch` and `--findings` (`+`-separated identities); `--epoch`
  renders the issue's marker as `review` instead of `analysis`, carrying `epoch` and `findings`. That
  is provenance recorded when supplied, not adjudicated: nothing checks that a finding was recorded
  against the epoch, that the epoch was a real publication, or that the issue was filed at all. The
  `--findings`-without-`--epoch` refusal is asked in `dispatch`'s `create` arm ahead of
  `commands::create`, so it creates no label. The `analyst:<runtime>` label is applied identically to
  both marker kinds. On 2026-09-06, 34 of 129 open issues named an epoch in free prose in thirteen
  spellings, which is what a grep could read before this existed, and no field on three issues
  recording one finding twice (filed 2026-08-19, 08-21 and 08-24) carries another's identity.

- **`finding-citation-unresolvable` checks that a `path:line` citation names something that exists
  and has enough lines, not that it still means what the finding says.** It ports
  `artifactReferenceTokens`, `artifactRepositoryPathReference` and
  `frozenRepositoryPathLookup.contains` (`artifact_admission.go`). `check_finding_citations` in
  `src/transport/claim.rs` runs over `evidence` and `impact` after the location block and before the
  lineage rules, and answers under one of four causes, `malformed-path`, `path-not-in-either-tree`,
  `path-is-not-a-file` or `line-past-end-of-file`. It **reads both `base` and `head` from the recording
  checkout** (`citation_ls_tree`, `tier::blob_content` under `How::read()`) for **every** class, so a
  run recording a `suggestion` from a checkout that never fetched the receipt's `head` fails with a git
  read error instead of recording.
  - **A path that exists and has moved lines still passes.** The line is compared with the larger of
    `base`'s and `head`'s blob line counts, not the diff between them.
  - **A path no tree holds as a blob is admitted at any line.** Where either tree holds a `120000`
    symlink or `160000` submodule commit, `resolve_citation_path` answers
    `CitationResolution::Found { line_count: None }`, because there is no blob to count.
  - **Six token shapes are skipped, not resolved:** a citation missing either half; a line half that is
    not all digits or overflows `u32`; a zero or reversed range (`0`, `5-3`); a path containing `://`;
    a path ending in a raw `/`; and a token failing the `pathLike` gate, so an unquoted extensionless
    root-level citation such as `Makefile:12`, `Dockerfile:5` or `justfile:9` is never resolved.
  - **A citation inside a backtick-fenced block is skipped.** `citation_tokens` treats a backtick as a
    quote opener that closes at the next backtick, so a fence's first two backticks form an empty span
    and the third opens one closing at the closing fence: the whole block is one quoted token whose
    tail is almost never all digits. The skip is a consequence of the quote grammar, not a fence rule.
  - **A quoted token bypasses the `pathLike` gate**, and the grammar ends a quote at the next
    occurrence of the same character, not a matched pair. An unbalanced apostrophe in prose opens a span
    to the next one however far, and what lands between is checked as a path. The same bypass lets
    `` `LICENSE:1` `` be recognised;
    `a_quoted_non_path_token_is_refused_as_the_pathlike_bypass_it_is` (`claim/tests.rs`) asserts the false
    positive: `` `ratio 1:3` `` refuses on `ratio 1`.
  - **A citation naming the ported Go by `path:line` is refused**, correctly: `artifact_admission.go:654-695`
    satisfies `pathLike` and resolves `path-not-in-either-tree`, so a finding that repeats one of this
    repository's origin coordinates in `--evidence` or `--impact` is refused for a reason unrelated to
    its own claim.

  **Five deliberate divergences from the origin**, named on `citation_reference` (the first, second,
  fourth and fifth) and `resolve_citation_path` (the third): (1) the line grammar accepts `N-M` ranges,
  where `artifactRepositoryPathReference` accepts a bare digit run and silently drops ranges; (2) the
  `pathLike` gate is `token.quoted || path_part.contains('/') || citation_dot_then_letter(path_part)`, a
  disjunction, so only a slashless unquoted token is held to the dot-then-letter clause, where theirs
  accepts any `.` and reads `1.2:3` as the path `1.2`; (3) a `100644`/`100755` match is read with
  `tier::blob_content` and its line count checked against the citation's `end`, where theirs validates
  the line syntactically and never opens the blob; (4) an embedded non-leading backslash is rewritten to
  `/`, consistent with `parse_finding_location`'s normalisation of `--location`, where theirs, on this
  repository's reading of `risk.go`'s `normalizeLogicalPath` (not in this repository, so unverifiable
  from here), rejects any backslash; (5) a path ending in a raw `/` answers `None` where theirs, via
  `path.Clean(value) != value`, rejects the whole result; a path ending in a backslash after a slash is
  refused as `malformed-path` once normalised.

- **Estigia cannot observe which bytes a judge was actually handed.** `publish_review` materialises the
  payload and binds its digest as `payload-digest`; it cannot see whether the run pasted the whole file
  into a judge's context, summarised it, or handed over something else. `publication-base-not-ancestor`
  closes one way a correct digest could cover a corrupted payload (a `base..head` diff built against a
  base that moved past the branch's rebase, carrying the base's history inverted into what reads as this
  change's deletions), but it gates how the payload is built, not what happens after it leaves this
  process. Run against this repository's own 270 publications, `git merge-base --is-ancestor` found 7
  where the base was not an ancestor of the head, 6 carrying another change's work inverted, worst a
  receipt claiming 1,338 deletions for a change that deleted 26. That is a count on this timeline, not a
  rate, and says nothing about the rounds those cost. The refusal's fetch (`git fetch origin -- <base>`)
  is observed by exactly one of three tests,
  `an_unmerged_base_advance_pushed_from_elsewhere_is_refused_once_the_fetch_moves_the_ref`; all three run
  against a local bare origin and a scripted `gh`, so a real remote's fetch is never crossed.
  `publish_with` resolves `payload_base` from `origin/<base>` before the check and reads
  `baseRefOid` afterwards, and no test establishes that order.

- **A second delivery on one command line travelled on the first one's label, and the repair does not
  close every shape of that.** `classify_observing` records the fragment the strictest check reads:
  `gh pr merge`, then the rest of `DELIVERS`, then everything else. Ranking promotes; it cannot
  adjudicate, so where a line's fragments are all one tier nothing is promoted. That case is closed by
  refusing a line whose built-in `IRREVERSIBLE` fragments put two or more in one tier, with
  `several-irreversible-fragments-on-one-line` naming the fragments. Declared boundaries never count
  toward it.
  - **No compound line can establish a pull-request identity.** `pr_merge_target` first requires every
    byte alphanumeric or one of ``space tab - _ . / : = +``, which excludes `&`, `;`, `|` and newline, so
    a compound line carrying a pull-request merge answers `delivery-pr-unidentified`, never
    `delivery-pr-mismatch`, before any receipt is consulted. The word test beside it is not a second
    barrier: `words.get(..3) == ["gh", "pr", "merge"]` is a prefix, so `gh pr merge 66 && git push`
    satisfies it.
  - **Ranking reads the fragment's spelling, never its origin.** `delivery_tier` takes a `&str`, so a
    declared boundary spelled `git tag` ranks as one; a declared fragment that is not a built-in
    spelling is tier 0 whatever it delivers, because Estigia cannot know whether `make deploy` delivers
    or rehearses.
  - **Newly refused populations.** `git merge --ff-only origin/main && git push` from `in-progress`,
    under `Integration::Branch`, answered `allow` before and answers `out-of-phase` now, recording
    `git merge`: the literal carries `&`, so neither `exact_local_fast_forward_target` nor
    `exact_integration_merge` applies and the generic refusal does not name splitting the line. Under
    `Integration::Trunk` it answers `unflagged-on-trunk` with no flag and is allowed with one.
    `git commit -m "git tag the release" && git push` likewise answers `out-of-phase`, recording
    `git tag`, which `contains_whole_command` matches inside the quoted message.
  - **Not measured:** how often a compound delivery line is written in ordinary use.

- **`tests/pipe.rs` can fail under machine contention, and nothing here makes that stop.** Measured on
  `017cd5a1`: 14 runs of `cargo test --test pipe` under sustained external load produced 2 failures, a
  14% rate, consistent with the roughly one-in-five previously reported. Both captured panics named a
  fixture command that failed under load and not the test's subject. Most of the suite's `git` fixture
  closures now go through one `checked_git` helper that panics naming the argument list, exit status and
  stderr, and the discarded `run(...)` calls a later line depends on now assert the outcome first.
  The `SKIP` sites keep their own git result as a condition, and raw
  `assert!(Command::new("git") ... .is_ok_and(...))` sites outside any fixture closure stay opaque. No
  retry, sleep or widened timeout was added, and the new abort points can leave the rate equal or
  higher; the 14% figure was not re-measured after the change. A green run under load is not proof that
  a red one earlier was a real defect. Staging a fixture checkout costs a measured 138 ms uncontended and
  336 ms under 8-way contention (1,383 of 7,200 calls over 500 ms, the slowest 1,302 ms, one `mktemp`
  failing outright); `scripted_gh` is now cached per test binary, so the `src/` call sites stage once
  and the pipe suite's `tracker_rig()` calls account for the staging that remains.

- **`CLAUDE_CONFIG_DIR` is followed by `setup` and the gate, and is deliberately not contained-checked.**
  `resolve_paths`, `definition_for`, `authorize_review_blind_launch` and
  `harness::matches_relocated_claude_config` all resolve under `$CLAUDE_CONFIG_DIR` when it is set.
  `authorize_review_blind_launch` proves the canonical reviewer against `$CLAUDE_CONFIG_DIR/agents`
  alone and never against `~/.claude/agents`, which belongs to the profile the operator is not running
  under. The relocated-root match requires the root to begin at a path boundary and asks the shell
  road's folded spelling of the root with one separator and with two, so a root containing a space or
  an escaped space still matches. Unlike `APPDATA` and `XDG_CONFIG_HOME`, the variable is not checked
  for containment in the home; `docs/what-it-writes.md`'s paragraph after its section on a redirected
  home says why.

- **Filing with a priority no declared scale allows is accepted, and the queue read says so.** `create`
  attaches whatever `priority` it is given and answers `priority_checked: true` only when the routed
  domain's scale is one this binary can read (today the `general` domain's `priority` scale in
  `skill/references/domain-composition.md`) and the label after the colon is non-empty. Anything else is
  filed untouched with `priority_checked: false`, because refusing would stop an analyst mid-file over
  one reversible label edit. `list_state` names, per partition, the items carrying no value on a readable
  scale under `unchecked`; a partition whose scale lives in a rule book the transport cannot read
  contributes nothing. `priority:urgent` on a `general` issue counts as a value, because what a value
  means belongs to the domain.

- **A developer's out-of-scope finding is handed to an analyst by prose alone, and the handoff is
  unobserved.** The Developer execution steps say a defect outside the issue in hand goes to an
  analyst run holding its own claim, never filed by the developer, because the context that produced a
  finding judges it worst. A subagent launch is not a write the gate sees, no tool knows the
  intersection of a run's findings with its issue's scope, and a developer that files its own finding is
  not refused, so a run that ignores the step is indistinguishable on the timeline from one that
  followed it.

- **The Stop hook reads a transcript it cannot prove is complete, and the approved-republish gate reads
  order, not intent.** A run whose only remaining work is a background launch that has not reported ends
  its turn instead of being kept for a poll, and a publication over an approved parent on the same pull
  request is refused unless a finding has been recorded against that parent receipt since the verdict,
  or the base advanced past the parent receipt's recorded base. The hook counts a launch only from the
  host's record that it started (a `toolUseResult` with a non-empty `backgroundTaskId`, or one reporting
  `async_launched`; result text is never read) and counts completion only from a `<task-notification>`
  naming that launch's `<tool-use-id>`. A host that reports completion in another shape leaves the
  launch unanswered, which ends the turn; a transcript rotated past the launch reads as no launch, which
  keeps the older block; a call the host refused or never answered is not a launch. None of the three is
  a clearance: the first can release a turn the run would have used, the other two spend one. The gate's
  "declared defect" is a finding's position in the timeline, not its truth: one recorded after the
  verdict clears the refusal whether or not the defect is real. Its advanced-base exception is
  direction-aware and proves only that the parent's recorded base is a proper ancestor of this
  publication's, not that the movement was compatible, the integration correct or the setting on, and a
  merged advance is read whole so the delta lens is never the panel over base content. Both readers fail
  closed: an unreadable transcript keeps the turn and a timeline that will not read refuses the
  publication.

- **A `raise` is a declaration, recorded at publication and not checked.** `publish_review` and
  `republish_review` record the raise, its reason, the focus, its reason and the computed tier on the
  published marker, `review_read` with `view: transaction` answers them, and `policy_for` lifts the
  gate's tier to `high` for a recorded item from the list. Nothing checks that the item applies to the
  diff. A raise made on `review_tier` alone is not stored. A repair publication that does not repeat the
  raise is sized by its diff alone, since the declaration is read from the receipt's own marker and not
  its parent's, and a raise on a repair takes the whole change's `high` panel rather than the narrower
  repair panel.
