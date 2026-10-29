# What a review costs, and what actually spends it

Ordinary review acquires lenses only; Judgment Day is the opt-in that acquires judges. A review's
forecast counts lenses alone. This document holds what each piece of that costs, the method for
measuring it again, and one result table per question. The limits of every measurement are in the
last section, so a figure and the reason to distrust it are read together.

It is one document in [the reference set](README.md); the rows it measures are in
[`configuration.md`](configuration.md), and [`honesty.md`](honesty.md) says what the recording cannot
count.

## What spent the cost of a panel: grading, not reading

Two panels of two judges each reviewed one change (5 files, 796 insertions, 54 deletions), launched
concurrently on 2026-08-30 with the design fixed before either ran. Arm E was the installed
`review-blind` definition; Arm C was the comparison project's rubric pasted into an `analyst` carrier.

| | tokens | tool uses | panel latency (concurrent) | summed judge time | findings (summed) | severe (identities) |
|---|---|---|---|---|---|---|
| Arm E | 291,408 | 69 | 558.1 s | 1078.7 s | 17 | 1 |
| Arm C | 268,162 | 72 | 491.0 s | 912.8 s | 4 | 0 |
| **E / C** | **+8.7%** | −4% | **+13.7%** | +18.2% | 4.25x | 1 vs 0 |

The figures are what the agent harness's completion report returned for each judge. The panels came
within nine percent on tokens and fourteen on latency against a 4.25x difference in what they
reported, so the judges were not slower readers. What spent the cost was grading: both Arm E judges
filed the one behavioural finding `severe`, the only class that blocks, and a block forces a
republish that invalidates every verdict and pays the full panel again. The multiplier is the round
count, and the grading sets the round count. Every prose finding in Arm E came back `warning` or
`suggestion`, so the documentation rule was not what made the review expensive. The judges used
`severe` as an escalation channel because no choosing rule reached them; that rule now lives in
`skill/agents/review-blind.md`, and the policy keeps it because the definition is installed on
Claude Code alone.

## Against the other project's own binary

Run on 2026-08-30 against that project at `a537c63e`, re-confirmed at `4db804e5`, on a review
transaction this repository produced (PR 167, head `61e50ae3`, base `29893655`, 12 files, 2,199
changed lines by its own count). Its risk classifier is deterministic Go that runs before any model is
called, and the reviewer it launches has no tools: the whole candidate patch is in the prompt.

| | as answered | as derived |
|---|---|---|
| `risk_level` | `high` | `medium` |
| lenses | four | one |
| prompt bytes per lens | 158,056 to 158,102 | ~158,000 |
| four-lens total | 632,309 | — |
| `correction_budget` / corrections permitted | 200 lines / 1 | 200 lines / 1 |

The answer `high` is a false positive: a content scan matched the crate-level inner attribute
`#![allow(...)]` at the top of `tests/honesty.rs` as a shebang. The `medium` column is derived from
its source, not observed. Change size never chooses its tier: line counts are computed and unused.

What follows from the table:

- **Bytes against tokens.** At four bytes per token, an assumption and not a measurement, 632,309
  bytes is roughly 158,000 tokens, about one Estigia judge, and one lens is about a quarter of one.
  Three of the six judges on the issue's receipts reported 147,249, 178,100 and 171,667 tokens when
  each was handed a checkout to search; under the payload-first shape the same harness reported
  121,973 and 145,840, and later 105,250 and 97,485. Which shape a figure came from is part of the
  figure. The orchestrator hands a judge the receipt's payload as primary evidence, see
  [judgment day](../skill/policies/judgment-day.md).
- **A tier with no reviewer exists there too.** Its `low` answers zero lenses when every authored
  path is passive content. Anything under a `skill`, `agent`, `prompt`, `instruction` or `workflow`
  segment is not passive there, so this repository's `skill/` would not qualify; `docs/`, `README.md`
  and `CHANGELOG.md` would.
- **Bounding the rounds.** That project allows one correction, capped at
  `min(200, ceil(originalChangedLines / 2))`. Here a lineage gets the corrections the
  `Corrections per lineage` row allows (default 3), and `publish_review` refuses the next as
  `correction-already-spent` before anything is pushed. `correction_budget_check` refuses a repair
  whose cumulative changed lines exceed the frozen budget, matching their formula. A round over an
  honest correction range hands each lens and the refuter only `parent-head..head`; a round whose
  range is not honest (`delta-range-reason` recorded) hands the whole target again. The targeted
  validator reads a correction beside the tier's lenses rather than instead of them, so the saving a
  correction buys there is not bought here. Their maintainer-authorised successor lineage is
  `estigia recover`, a person's command: a recovered change pays a full new review and never at a
  lower tier than the lineage it replaces. `review_tier`'s `raise` can lift a tier to `high` and
  never lower it, see [`operations.md`](../skill/bindings/github/operations.md).

The layers compared, one against one, per the operator's 2026-08-31 ruling to compare layer against
layer and never totals (Estigia carries a task-flow layer with no counterpart there):

| layer | here (Estigia) | there (C) | comparable |
|---|---|---|---|
| lens | 0 / 1 / 4 across low/medium/high (`RiskLevel::lenses` in `src/transport/tier.rs`) | 0 / 1 / 4 (`internal/reviewtransaction/risk.go`) | yes |
| refuter | one, sealed, no `Read`, writes nothing (`src/harness/roles.rs`) | one, in a role vocabulary closed at three | yes |
| targeted validator | one, sealed, read-only (`RESERVED_VALIDATOR`); required of a correction beside the lenses | one, the terminal event of a correction | yes; here it adds to a correction's panel, there it replaces it |
| blind judges | none at any tier; Judgment Day is the opt-in | agent markdown a skill launches; nothing in its Go runs one | no |

A typical `medium` review was estimated at about 450,000 tokens with two judges and roughly a third of
that without them; no run reproduced the figure. Neither ~40,000 tokens per reviewer nor the 181 and
265 seconds of reviewer execution that project discarded on malformed output is a judge figure: the
first is one lens's prompt converted at four bytes per token and the others are lens executions.
No judge-versus-judge figure exists on either side.

## What a tool answer costs

Four tool answers carried fields nobody but a rare caller acts on, on every ordinary call. The fields
moved to `review_read` (`SCRIPT review-read --issue <n> --view transaction [--epoch <id>]`), which
answers the topology, the repair lineage, the publication lane's note, workflows and count, and the
autoclose verdict with its mandatory follow-up. All of it is also recorded on the `published` marker
that `review_read` reads back, so nothing a caller acts on was removed.

**Method.** *Reconstructed* means the exact JSON the pre-edit and post-edit source builds, assembled
in an inline `python3 -c` computation with representative field values and sized with
`json.dumps(obj, separators=(',', ':'))`, which is byte-identical in shape to `serde_json::to_string`.
*Measured* means a real `cargo test -- --nocapture` of the named test against the code, read from a
temporary `eprintln!` beside the ceiling assertion. The two agree where they overlap: 616
reconstructed against 610 measured for the same first-publication shape.

| Scenario | Method | Before (bytes) | After (bytes) | What moved |
|---|---|---:|---:|---|
| `publish_review`, first publication, no inline payload, one lane started | Reconstructed | 1,309 | 616 | topology, lineage, lane detail, autoclose |
| `publish_review`, repair over a parent, `inline` topology with a reason, branch-link autoclose, two lanes | Reconstructed | 2,089 | 617 | topology and reason, parent receipt, delta, lane detail, autoclose verdict, follow-up |
| `publish_review`, first publication, branch-link autoclose, on `a_branch_link_publication_keeps_the_follow_up_out_of_its_answer` | Measured | — | 610 | as row 1 |
| `record_review_finding`, first write, `suggestion`, every optional absent | Reconstructed | 248 | 144 | `parent`, `origin`, `evidence_class`, `causal_disposition`, `location`, `lens`, omitted rather than `null` |
| `record_review_finding`, replay, `severe`, every optional present, on `a_replay_of_a_recorded_finding_answers_from_the_marker_even_when_the_diff_cannot_be_read` | Measured | — | 213 | nothing; the ceiling guards regrowth |
| `record_review_verdict` (retired), `accepted`, long basis and never-ran evidence | Reconstructed | 853 | 518 | the free text, replaced by byte counts |

**What did not move.** `publish_review` and `republish_review` keep `ok`, `issue`, `pr`, `url`,
`created`, `draft`, `epoch`, `head`, `base`, `digest`, `payload_digest`, `payload_bytes`,
`publication_lane` (the state word only) and `next`, plus `payload` when `inline_payload` was asked.
`record_review_finding` keeps every field that has a value.

**Guarded against regrowth.** `publish_review`'s answer stays under 900 bytes on the branch-link
fixture (measured 610) and `record_review_finding`'s replay answer under 260 on the full-optional
fixture (measured 213). Each ceiling carries headroom over the real figure and is no production
limit; a field added to either answer is expected to raise the ceiling deliberately.

### What the tool menu itself costs

Every host that loads schemas up front pays this on every session whether or not a tool is called.
The figure is the compact `serde_json::to_string` of the `result` object a real `estigia mcp` answers
`tools/list` with, which is the exact bytes a client receives.

| | Advertised names | `tools/list` result (bytes) |
|---|---:|---:|
| Before consolidation: 48 acting tools plus 3 `NOT_EXPOSED` refusal schemas | 51 | 75,913 |
| Consolidated table, `NOT_EXPOSED` no longer advertised | 32 | 55,381 |
| Descriptions trimmed, from 56,133 at that head | 32 | 44,325 |
| Shared argument text, no refusal-code lists, no `lens` argument | 33 | 42,643 |
| `review_tier` gains `raise`, `raise_reason`, `focus_reason`; `publish_review` and `republish_review` take the same | 33 | 44,277 |
| `send_message` and `list_sessions` join, the latter with bounded discovery | 35 | 46,744 |
| `task` `evidence`, then `create` `repo` and the `flush` window wording | 35 | 47,044 |
| Ten core tools carry `_meta` (+380) and every tool takes `compact` (+1,015, paid for by trimming) | 36 | 48,496 |
| `release` `spend` fields, then `ready_pr`, `review_stages`, `run_checks` and `deliver` join | — | 51,747 |
| `review_thread` and exact-head CI polling parameters; typed mutations replace unverified API writes | 41 | 53,203 |
| `ensure_states` says a closed git operation window answers `deferred` and the labels come from the flush | 41 | 53,359 |
| `feed` joins: the store's transitions, markers and messages as a bounded page | 42 | 54,779 |
| `status` joins: a run's self-reported line, local to the store | 43 | 55,520 |
| `list_sessions` takes `fields`: `status` adds the self-report to each row | 43 | 55,645 |
| `where` joins: one read-only answer to where an issue stands | 44 | 56,332 |
| `inbox` joins; `send_message` takes `type`, `thread` and `expires_in` | 45 | 58,148 |

Nineteen names left the surface between the first two rows and 20,532 bytes went with them: the
capture-only recorders, the retired aggregate verdict and the `NOT_EXPOSED` names were removed, and the
rest were absorbed by a surviving name reached through a parameter or a `view`. `TOOLS_LIST_CEILING` in
`src/harness/mcp/tests.rs` bounds the list, and each growth above was a reviewed raise by exactly the
measured difference. It is the largest list a process can answer, the unreadable-context one.

The list depends on the process's `GateContext`, fixed for the process, so `listChanged` stays false.
Where `Workflow` is `assisted` the list omits every `TRACKER_WRITERS` tool; under any other readable
workflow it omits `ASSISTED_ONLY`; an unreadable context or an unknown workflow lists all. A hidden
name still dispatches and refuses with its own typed reason.

| `tools/list` result, issue #5 | Names | Bytes |
|---|---:|---:|
| Before, every context | 33 | 44,318 |
| After, context unreadable (every tool) | 33 | 42,643 |
| After, autonomous | 31 | 40,901 |
| After, assisted | 13 | 14,380 |

- **Payload threshold.** Answers carrying 73,272 and 145,756 payload bytes were refused by the MCP
  surface and one carrying 36,288 was not, so the threshold sits inside the ordinary range; that is
  why `inline_payload` defaults to false.
- **`ALWAYS_LOADED`** (eight tools in `src/harness/mcp/tools.rs`) carry
  `"_meta": {"anthropic/alwaysLoad": true}`, which Claude Code reads to load a schema at session
  start instead of spending one to three `ToolSearch` turns before the first `claim`. In the lever
  bench below, 16 of 18 publishing runs spent a `ToolSearch` turn on `record_spend` or `review_read`
  to learn the receipt fields, and 6 had a first `release` refused `missing-argument`; one turn costs
  about 50,000 tokens of context and the hundred bytes are paid once.
  The other side of the trade is that a marked schema is paid on every turn of every session. A tool
  stays marked only while most deliveries call it. Over the operator's sessions from 2026-10-24 to
  2026-10-28 (15 sessions, 7,033 turns), `launch_review_input` was called twice, because
  `publish_review`'s `start_review` launches the slots. Over MCP, `review_status` now names `deliver`
  in place of `record_computed_verdict` and `release_ci` wherever `deliver` carries an approval to the
  merge. Those three schemas, about 4 KB together, were unmarked; the run that needs one of them loads
  it with one `ToolSearch`.
- **`compact: true`** omits `build` from an answer and drops from `review_status`'s `next.arguments`
  the fields `receipt` already holds. Without it every answer is byte-identical to before.
- **`ensure_states`** says that while the git operation window is closed it answers `deferred` and
  writes nothing, and that a queued create, claim or transition creates the labels at the flush: a caller
  that reads `deferred` as the whole answer has no reason to wait for the flush.
- **`target`** answers the brief form by default, so the manifest, about 100 bytes per tracked file,
  is paid only by a call that passes `manifest`.
- **`republish_review`** stays a tool of its own although its arguments equal `publish_review`'s: its
  name is the guard on a call that destroys remote history.

## Waiting for a reviewer instead of polling

`collect_review_input` used to answer `capture-running` while a launched reviewer worked, so a run
waiting on a lens polled it, one full-context turn per poll. It now takes `wait_seconds`, blocking up
to that many seconds, capped at 900 and never past the point the job would be abandoned.

Measured 2026-10-09 over every `mcp__estigia__collect_review_input` call in the Claude transcripts
under `~/.claude-personal/projects`, paired with its `tool_result`, the corpus
`scripts/measure-transcripts.py` reads:

| | sessions reaching the tool | `collect_review_input` calls | answered `capture-running` |
|---|---:|---:|---:|
| Before | 1 | 16 | 1 |
| After | — | — | 0 by construction |

The after is by construction: with `wait_seconds` at or above the job's remaining runtime the call
returns the recorded answer, and a job past its deadline is still killed and refused
`reviewer-deadline` with nothing recorded.
`a_bounded_wait_records_the_answer_once_the_reviewer_finishes` and
`a_bounded_wait_that_elapses_is_refused_capture_running_and_writes_nothing` drive both paths.

## The orchestrator's own phase is recorded, not measured

`review_read` with `view: spend` and `phase: orchestrator` answers whatever `record_spend` was asked to
record for that phase: the delivering run's own report of its context, tokens and time. Nothing
re-derives it from a transcript, because Estigia has no channel into the harness that ran the
orchestrator, as it has none into any subagent. The delivery step records one `record_spend` for
`orchestrator` and one per delegated phase before the transition to `done`; `skill/SKILL.md`'s
developer step 6 states this inside the delivery sentence, and no test pins that clause.

## The reviewer prompt order buys no cache hits, and is not shipped

Moving the role template to the end of the prompt, so the shared receipt, criteria and diff lead and
share a prompt-cache prefix, was measured on 2026-10-09 and **not shipped**: the shared bytes are
never cache-read in any position. Four `claude -p --model claude-sonnet-4-5 --output-format json`
calls, reading `usage.cache_read_input_tokens` and `usage.cache_creation_input_tokens`, over the bytes
`build` in `src/reviewer/prompt.rs` produces for a real lens (68,164 bytes):

| call | prompt | bytes | cache read | cache created |
|---|---|---:|---:|---:|
| 1 | lens prompt, role first, cold | 68,164 | 0 | 70,980 |
| 2 | the same bytes again | 68,164 | 18,289 | 52,717 |
| 3 | the same prompt, role moved last | 68,170 | 18,289 | 52,770 |
| 4 | an unrelated two-word prompt | — | 18,289 | 32,665 |

The read-back is 18,289 on three calls that share nothing but the binary, including the unrelated one,
so it is the CLI's own invariant prefix and not any reviewed byte. The user message that carries the
receipt, criteria and diff is never cache-read, so the role's position in it cannot share what was
never shared. Concurrent lenses would also not reuse a prefix the first had not yet returned, and
serialising them is out of scope.

## Preserving the reviewer CLI's own usage

`parse_result` in `src/reviewer/adapters.rs` used to drop the CLI's `usage` object. The runner now
writes the counters into the job's directory as `usage.json`, beside `result.json` and never inside
it, so a counter can never contaminate what a reviewer is admitted to have said:

```json
{"format":1,"provider":"claude","counters":{"cache_creation_input_tokens":0,"cache_read_input_tokens":250,"input_tokens":100,"output_tokens":20}}
```

- A counter the CLI did not report is **absent**, never `0`: unobserved is unknown. A `usage` object
  that is absent or malformed preserves nothing, and the review succeeds or fails as before.
- `estigia::reviewer::job::read_usage(dir)` answers the counters; `None` is the unknown state, from
  either no report or a failed store. An artifact that exists but is not JSON, is another format,
  names no provider, holds no counters, or carries a non-whole-number counter is an `Err`: corrupt
  rather than unknown.
- A failed usage write is reported on the runner's standard error and leaves the counters unknown; it
  never turns a successful review into a refusal.
- Only the Claude adapter's `--output-format json` result carries `usage`. The Codex adapter's
  answer comes from `--output-last-message`, whose standard output carries none, and the relay runs
  no CLI in this process, so neither preserves anything.
- `scripts/measure-transcripts.py` reads Claude Code's `.jsonl` transcripts, not `usage.json`, and
  `read_usage` has no measurement caller yet.

## Re-measuring after a change

`scripts/measure-transcripts.py` (`python3 scripts/measure-transcripts.py --projects
~/.claude-personal/projects`, the default) repeats the 2026-09-24 baseline, which measured
orchestrator context by phase, forced `Stop` turns and per-issue input tokens from Claude Code's
`.jsonl` transcripts. It reads every transcript it can reach and reports a JSON object and a readable
table, per repository and overall:

- session count, and input tokens by context (orchestrator, developer, reviewers, analyst), taken from
  the `agentType` each subagent's sibling `.meta.json` records;
- forced-`Stop` turns, a `system` record whose `stop_hook_summary` names `estigia` among its hook
  errors, with the next assistant turn's context;
- per-issue input tokens where a transcript names an issue, and each repository's most recent session.

The four usage classes, `input`, `cache_read`, `cache_creation` and `output`, are reported separately
per context from each record's own counters (`CLAUDE_USAGE_KEYS` in the script). A class no record
carried is `null` and never `0`, and `usage.records_missing_a_counter` counts records lacking any of
the four, so the classes read as a floor. Against the base script on the same corpus the two agree
byte for byte on `session_count`, `context_input_tokens`, the forced-`Stop` figures and
`per_issue_input_tokens` (2026-10-18: 179 sessions, 90,563 usage records, none missing a counter).

The JSON also carries, and the readable table does not: `turn_profile` per context (model turns by
distinct assistant message identity, `served_models` from `message.model`, `requested_models` from
`requestedModel`, effort, tool calls by distinct `tool_use` identity); `usage_by_model`, the four
classes split by the answering model; and `sessions`, each with its dominant issue,
`wall_clock_seconds` (first timestamped record to last, subagents included, `null` when a stamp has no
offset) and the subagents it launched. A field a record does not carry is counted under `unknown`,
never inferred. Against the base script on a frozen copy of the corpus (2026-10-23: 206 sessions,
87,020 usage records) every field the base reports is byte-identical, table included.
`tests/usage_accounting.rs` pins the classes and the new fields against a hand-written fixture,
including the unknown and missing-counter cases the live corpus does not exercise, and pins the
readable table.

## The FLOW workflow against its baseline

FLOW replaced the seven ODD steps in `skill/SKILL.md`. Three scenarios were run under frozen
conditions: S1, a one-file fix plus its test; S2, a two-unit change; S3, a no-change outcome that
closes with evidence and no pull request. Each was a fresh issue handed to `/flow dev <n>`, headless,
with no human input, in a scratch `HOME` against a private seed repository byte-identical to
`7bf02ee9`, `claude-opus-5-5` at `medium`, only the `estigia` MCP server. Acceptance is checked by the
operator's script against the merged commit or the issue, not by the run's own report. A run that
stops to ask counts as a failure. Cost is the CLI's `total_cost_usd`, its own tariff arithmetic and
not a bill, and it excludes the lenses. Tokens, turns and tool calls come from the run's transcript
through `scripts/measure-transcripts.py`; active seconds run from the first stream event to the last
assistant turn.

### Baseline, arm A (`estigia 0.1.1 (7fa8b68ebd)`)

| scenario | runs | accepted | median active s | median CLI cost USD | cost per accepted task USD | median turns | median tool calls |
|---|---|---|---|---|---|---|---|
| S1 | 3 | 3 | 532 | 1.34 | 1.33 | 36 | 40 |
| S2 | 3 | 3 | 561 | 1.52 | 1.52 | 38 | 44 |
| S3 | 3 | 3 | 83 | 0.59 | 0.69 | 13 | 15 |

Every run was accepted; the 9 counted runs cost 10.62 USD. What it showed about the contract: the ODD
checklist fired unevenly on S2 (2, 0 and 1 task rows over three runs of one issue); the no-change
close needed no publication and no lens and cost under half of S1; and every S1 and S2 run was refused
by the gate at least once before delivering, on `delivery-pr-unidentified` for a merge spelling (all
six), `closing-keyword-live` on a publication body (four), `unexpected-state` (two), and
`delivery-pr-repository-option` (one).

### FLOW, `estigia 0.1.1 (fdef71b93a)`

| scenario | arm A median cost | FLOW median cost | Δ cost | A median active s | FLOW median active s | Δ time | A / FLOW median turns | A / FLOW cost per accepted task |
|---|---|---|---|---|---|---|---|---|
| S1 | 1.34 | 1.10 | **−18 %** | 532 | 500 | −6 % | 36 / 28 | 1.33 / 1.41 |
| S2 | 1.52 | 1.33 | **−13 %** | 561 | 549 | −2 % | 38 / 31 | 1.52 / 1.34 |
| S3 | 0.59 | 0.61 | +3 % | 83 | 91 | +10 % | 13 / 13 | 0.69 / 0.99 |

Medians are over three counted runs per scenario. Two runs (F-S1-c3 and F-S3-c3) were cut by the
harness, which ended a headless session 120 s after an assistant turn with no tool call while the run
waited on a background task; the harness was corrected to idle-cut only after 300 s with no background
task pending, each cut run was replaced once as the frozen design allows, both replacements were
accepted, and the cut runs' cost is counted. That is why the cost-per-accepted-task column is worse on
S1 and S3.

- **Delivery refusals disappeared.** Six of six publishing runs in arm A were refused on a merge
  spelling or closing keyword; none of six under FLOW, because *Deliver* now states both spellings.
  One `unexpected-state` per publishing run remained, which arm A had in two of six; its cause was not
  isolated.
- **No checklist on one-publication work**: 0 task rows on every FLOW run, as the span rule asks, and
  S2 still met acceptance three times of three.
- **Turns fell 18 to 22 % on the publishing scenarios; time did not move**, because active time is
  `cargo` and four lenses, which FLOW changes neither of.
- **The programme's targets (−40 % cost, −30 % time) were not met**: −13 to −18 % cost and −2 to −6 %
  time where a publication happens, nothing measurable on S3.
- **Arm B**, `Model routing` unset so the lenses ran on the CLI default instead of `sonnet`, was
  accepted six of six. The orchestrator's cost barely moved (median 1.19 against 1.10 on S1); the
  lenses' output rose (2,689 against 403 tokens on S1) and median active time went from 500 to 766 s
  on S1 and from 549 to 797 s on S2. The lenses' model is recorded only when a row names one.
- **Arm C**, Claude Code with no MCP, hooks or skill, cost about a third of FLOW and took about 60 %
  of its active time (S1 median 0.31 USD, 291 s, 2 of 3 accepted; S2 0.47 USD, 357 s, 3 of 3; S3
  0.21 USD, 52 s, 2 of 3). Its guarantees are not equivalent: no claim, no gate, no independent
  review. One run ended its turn waiting for the suite with nothing committed; another, told to change
  nothing if the behaviour was right, fixed two neighbouring crashes and merged them unreviewed into
  `main`. These figures measure the price of the claim, the review and the continuation; they do not
  argue it should not be paid.
- **Retired**: the seven ODD steps, which FLOW's 9 of 9 acceptance lost no guarantee by. Kept: the
  *References* row loading `protocols/rdd.md`, the file itself, the ODD and RDD attribution in
  `docs/parity.md`, and the retired `Review protocol` row's migration message in
  `src/config/settings.rs`, because no figure here argues against any of them.

### The cost and time levers (`estigia 0.1.1 (5bb61940a3)`)

Four changes made against the anatomy of the FLOW runs: `release` closes in one call; `publish_review`
can start the review and `collect_review_input` can record a computed verdict; the delivery tools are
always loaded and every tool accepts `compact`; each tracker fact is read once per call. Arms were
interleaved run by run (G, T, D) so none ran on a cache another had left hours before:

- **G**: the build with the installation's defaults on `claude-opus-5-5`.
- **T**: G with `CLAUDE_CODE_PROMPT_CACHE_TTL=5m` in the session's environment, an operator setting
  documented by Claude Code and not Estigia configuration. Opus 5.5 writes the one-hour cache at twice
  the input price and the five-minute cache at the input price.
- **D**: G with the orchestrator on `claude-sonnet-5-5`, an experiment on quality that nothing adopts.

A fourth arm, setting the profile's unrelated skills to `skillOverrides: "off"`, was dropped after a
one-turn probe: the listing shrank from 18,396 to 599 bytes but the first turn's context went from
33,770 to 33,939 tokens, so the arm would have measured nothing.

| scenario | arm | median cost | Δ vs A | Δ vs FLOW | median active s | Δ vs A | Δ vs FLOW | median turns | cost per accepted task |
|---|---|---|---|---|---|---|---|---|---|
| S1 | G | 1.02 | −24 % | −7 % | 540 | +2 % | +8 % | 26 | 1.01 |
| S1 | T | 0.83 | **−38 %** | −24 % | 511 | −4 % | +2 % | 24 | 0.82 |
| S1 | D | 0.37 | −72 % | −66 % | 240 | −55 % | −52 % | 19 | 0.36 |
| S2 | G | 1.06 | −30 % | −20 % | 677 | +21 % | +23 % | 24 | 1.05 |
| S2 | T | 0.90 | **−41 %** | −32 % | 519 | −7 % | −5 % | 21 | 0.95 |
| S2 | D | 0.41 | −73 % | −69 % | 232 | −59 % | −58 % | 18 | 0.42 |
| S3 | G | 0.50 | −14 % | −17 % | 74 | −11 % | −19 % | 10 | 0.81 |
| S3 | T | 0.51 | −13 % | −16 % | 87 | +5 % | −4 % | 13 | 0.49 |
| S3 | D | 0.22 | −62 % | −63 % | 60 | −28 % | −34 % | 8 | 0.22 |

Over all three scenarios the cost per accepted task was 1.18 USD for A, 1.25 for FLOW (+6 %), 0.96 for
G (−19 %), 0.75 for T (−36 %) and 0.33 for D (−72 %). One G-S3 run was cut by the same harness
condition (a background analyst handoff the run had given a run id the store does not issue) and
replaced once, its cost counted.

Which lever moved what, read from each run's stream:

- **Estigia's own calls fell by a fifth.** Median time inside Estigia tools on a publishing run went
  from 103 to 107 s under FLOW to 84 to 85 s under G, and turns from 28 to 31 down to 24 to 26,
  because the close, the review start and the verdict no longer take a turn each.
- **Cache reads and output fell; the first turn grew.** Against FLOW on S1, G read 1.61 M cached tokens
  against 1.78 M and wrote 8.9k output tokens against 10.8k. The always-loaded schemas raised the
  first turn's context from 44.6k to 50.0k tokens, paid once per session.
- **Time did not fall, because `cargo` decides it.** The `cargo test` median on S2 was 431 s under G
  against 261 s under FLOW; two of three G-S2 runs ran the full suite three times where the repository
  asks for one.
- **The five-minute cache cut the price, not the tokens.** T wrote more to the cache (72k against 58k
  cache-creation tokens on S1) because a lapsed cache is written again, at half the price: −19 %
  against G on S1 and −15 % on S2, nothing on S3.
- **D was cheap and fast by doing less.** The Sonnet orchestrator ran the full suite before publishing
  in 2 of 6 publishing runs; every Opus run in every arm ran it at least once.

Against the criteria recorded before the first run: every run was accepted in every arm. T and D each
had one `delivery-pr-unidentified` refusal (T chained `git fetch … && gh pr merge`; D posted close
evidence in a heredoc that named the merge command), both recovered on the next turn and unrelated to
the cache or the levers. G's lenses filed two `warning` findings where FLOW's worst was `suggestion`
(untested invalid `--since` value and subagent-only timestamp), and nothing blocked. D skipped the
repository's full-suite rule in 4 of 6 publishing runs. **No arm meets both targets with every
criterion met.** The levers cut cost per accepted task by 19 % and left time where `cargo` puts it;
the five-minute cache reaches −36 %, and −40 % on the S2 median; D reaches both targets by not running
what the repository asks to be run. Nothing here recommends T or D. A publishing run's active time is
still about half `cargo test`, and nothing Estigia sends changes how often the model runs the suite.

## `deliver` against the baseline: arms H, I and J

Three further arms measured the same scenarios against arm A (S1 to S3, three runs each, opus, default
settings, against the `flow-bench` repository). The figures are medians.

| Arm | Build | What it is | Cost against A | Active time against A |
|---|---|---|---|---|
| H | `6006c6af` | the levers, before `deliver` | −43 %, −36 %, −15 % (S1, S2, S3) | −19 %, −17 %, −2 % |
| I | `3fa8c9b6` | `deliver` | not a gain on S1 and S2 | slower: 596 s and 673 s |
| J | `1ecf4f44` | `deliver` with the empty-rollup fix (S1 and S2) | −46 %, −42 % | +16 % (532 to 619 s), −12 % (561 to 492 s) |

I was slow because `deliver` waited on a pull request rollup that was empty on a lane where no CI is
consulted. The empty-rollup fix passes it at once, and J is the arm that measures that.

J took the agent from 36 to 17 turns on S1 and from 38 to 18 on S2. All 6 of 6 runs were accepted on
the first attempt, and the worst lens finding class was `suggestion`. **The cost target of −40 % is
met on both scenarios; the time target of −30 % is not.** The time is the agent's own `cargo test`
reruns, which ranged from 222 to 533 s per run and swamp a 12 % to 16 % difference in either
direction; nothing `deliver` sends changes how often the model runs the suite.

## What these measurements do not establish

A later arm inherits every limit of the ones before it.

- **The tool-answer shrink is reconstructed for most rows.** Only the *after* side can be measured
  directly, because the pre-edit code no longer exists to run, so every *before* figure and the repair
  scenario are reconstructed. The 616 against 610 agreement is the evidence the reconstruction is
  trustworthy, not a substitute for having run the old code.
- **The transcript corpus is compared corpus to corpus.** `scripts/measure-transcripts.py` aggregates
  the whole growing corpus against the 2026-09-24 baseline, and nothing in it separates a post-change
  session from a pre-change one.
- **The usage classes are one CLI's counters.** The mapping is Claude Code's own keys; Codex, Pi,
  OpenCode, native reviewers and auxiliary requests are not read, and a provider whose cached counters
  sit inside its input total needs a mapping that does not exist. The unknown-class and missing-counter
  branches are exercised only by the synthetic, Claude-shaped fixture in `tests/usage_accounting.rs`,
  not by live data. The parity with the base script covers fields the classes do not reach.
- **A session's span, turns and models are what its transcript says, not what was billed.**
  `wall_clock_seconds` counts idle time and is not time to a verified delivery unless the session ended
  there; `served_models` and `requested_models` are not billing records; a retry recorded under a new
  identity counts again.
- **The FLOW baseline is three runs of three scenarios on one runtime and proves no effect.** Claude
  Code `2.1.309`, `claude-opus-5-5` at `medium`, one scratch repository. n = 3 supports a description,
  not significance. Prompt-cache state was recorded and not controlled, the lenses' cost is not
  computed, and the three scenarios all touch one Python script, so nothing follows for a change in
  `src/`, a resumed run, or a review that rejects. The pilot's two calibration runs (4.72 USD) are
  excluded.
- **The FLOW comparison inherits those limits and adds four.** The two arms ran hours apart on one
  cache; the harness's idle cut changed between them (120 s to 300 s, background-task aware), so
  arm-A runs never met the corrected harness, though active seconds do not move with it; arm B's
  lenses ran on a model nobody recorded; and arm C's reduced guarantees make its cost a reference, not
  a target. Gate refusals are counted from the run's own stream, so a code a run met but never printed
  would be missed.
- **The lever arms are n = 3, and their time figures track `cargo`.** Interleaving spreads cache state
  without controlling it. Arm T's environment was inherited by the MCP server that launches the
  lenses, so the saving may not be the orchestrator's alone. A run's active time moves with how often
  the model chose to run the full suite (once to three times on one scenario), and a difference
  between arms is within that spread. Whether a run ran the suite is read from its shell commands by
  pattern, so a wrapper the pattern does not name would be missed.
- **The `deliver` arms** ran S1 and S2 only for J, so S3 has no `deliver` figure. Active time moved
  from +16 % to −12 % while suite reruns spread from 222 to 533 s, so the time result proves no
  direction. "Accepted on the first attempt" is the acceptance script's reading of 6 runs.
- **The panel measurements** compare two briefs inside this repository and the grading of one change;
  they are a description, not a rate. The tokens, tool uses and durations are the harness's completion
  report for each judge, with no retained transcript, and the target was a merged pull request, not a
  review transaction in this repository's sense.
- **Every citation into the other project is unfalsifiable from here.** Its commit hashes, constants,
  quoted Go, regex, per-lens byte counts and durations are relative to a checkout a reader confined to
  this repository cannot open. No reviewer result was ever admitted from it across three attempts, so
  the comparison covers what each system sends a reviewer, how many it selects and how many corrections
  it permits, and says nothing about what either finds or how it grades. Nothing timed the two systems
  end to end, and no artifact of the measurement is retained.
- **The orchestrator's phase is a declaration.** `record_spend` reports what the run said, as stated in
  [`honesty.md`](honesty.md), and the corpus holds no post-change session to measure the polling
  removal against.

What else the review tier, the lens and the finding do not establish is in
[`honesty-review-tier.md`](honesty-review-tier.md).
