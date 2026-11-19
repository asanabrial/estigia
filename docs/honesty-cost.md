# Measurements and cost

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what a measurement reconstructs and what the token-efficiency programme does not show. The index says which area holds which question.

## What a measurement reconstructs

- **The review-cost measurement does not close what it was launched to answer.** No counterfactual
  quantifies the rework, claim races or unreviewed delivery the panel avoids. The two 2026-08-30 trials
  in [`review-cost.md`](review-cost.md) have these limits:
  - The first changed its definition mid-run; eight judges lacked the severity vocabulary. It ran the
    external rubric inside this host's `analyst`, not that project's binary, with two judges per arm,
    no variance or retained artifacts, on an already-merged PR with no receipt. The 13.7% brief effect
    and +18.2% summed judge time cannot be separated from sampling.
  - The second ran the external binary but admitted none of three answers (two discarded after
    181/265 seconds). It compares inputs, not findings, mixing bytes and agent tokens at an assumed
    four bytes/token. Its observed `high`/four-lens tier falsely matched a Rust `#!` attribute;
    corrected `medium`/one-lens was read, not run. External revisions were `a537c63e` and `4db804e5`;
    no artifact remains, and its relative external citations cannot be checked from this repository.
  No judge-versus-judge or whole-delivery figure exists, despite the orchestrator spend channel.
  `collect_review_input --wait-seconds` has only a before measurement: one session, 16 calls, one
  `capture-running`. Zero subsequent polls is by construction, not an after measurement. No suite
  test supplies the missing live cost evidence.

- **A preserved usage artifact is not a measurement, and no total folds it in yet.** The runner writes
  the reviewer CLI's own counters to `usage.json` in a job's directory, beside `result.json` and never
  inside it, and `job::read_usage` reads them back; the admitted answer's bytes are identical whether or
  not the CLI reported usage. Only the Claude adapter's `--output-format json` result carries a `usage`
  object, so a Codex review and a relayed review preserve nothing. `scripts/measure-transcripts.py` reads
  Claude Code's own `.jsonl` transcripts and does not fold this in, and nothing in production calls
  `read_usage`, so what a native review cost stays unmeasured. The artifact outlives the result it
  belongs to (`discard_result` deletes `result.json` and leaves `usage.json`), so a reader checks
  `job_status` first. **A missing artifact is unknown, not zero**: `read_usage` answers `None` both when
  the CLI reported no usage and when the counters could not be stored (a failed write goes to
  `runner.err`), and the two are not distinguished; an artifact that is not JSON, names no provider or
  carries a non-whole counter is an `Err`, never a zero. No consumer exists yet to read `None` as
  unknown.

  **The failed-write diagnostic is emitted but unproved.** `report_usage_failure` writes to the
  runner's standard error, and no test added by this change reads that stream: the outcome a failed
  write leaves is asserted, the message is not, so a change that stopped emitting it would pass every
  test while the promise in `run`'s own comment stayed. The test that would capture `runner.err`
  drives the detached runner, and it lives outside the paths this change may touch.

## The token-efficiency programme

The programme asked for less redundant input and output across every supported runtime, with no lost
function. It closed with only its deterministic units delivered: the bounded session walk
([`sessions.md`](sessions.md)), the compact task acknowledgement and review view, the server-side
receipt expansion, the reviewer's own usage kept apart from its answer, the per-class transcript
accounting ([`review-cost.md`](review-cost.md)), the instruction budgets, the waits audit
([`waits.md`](waits.md)) and the mailbox crash cases ([*The session mailbox*](honesty-sessions.md#the-session-mailbox)).
What it did not establish:

- **No saving is measured.** No paired whole-task run compares input, output, cost, time and quality
  before and after these units, on any runtime. A smaller answer is not shown to be a cheaper task: a
  compact form that costs an extra read, or a lost cache prefix, could cost more.
- **One host's telemetry.** Accounting reads Claude Code transcripts. Codex, Pi, OpenCode and the other
  registered adapters report no usage this crate reads, so their figures are unknown, not zero.
- **No attention-changing change shipped.** Deferred loading, filtering, compaction and checkpointing
  were left out rather than tried. Nothing here says whether they would preserve task quality.
- **No immutable-data reuse, no raw-artifact store.** Neither was built, so no security or lifecycle
  contract for one exists.
- **Mail stays at-least-once.** No supported host gives the durable acknowledgement that exactly-once
  delivery to the model would need.
