# Operations in detail

The operations table in [`github.md`](../github.md) keeps one line per operation. The cells whose
detail would run past the paragraph ceiling live here, and each table cell names the section that
carries it.

## Claim

a retry reuses the same operation ID — never a fresh one, or a dropped write would read as a second
claim event — and this call cannot move a live holder's own horizon; which refusal it meets depends
on which call wrote the live holding. Against a holding **`claim`** itself wrote: reusing the id and
asking for a different horizon is refused `claim-metadata-mismatch` against the persisted marker,
and asking under a fresh id instead adopts the live claim as already written, reporting the horizon
already on the timeline rather than the one just asked for. Against a holding this run took over —
its own marker, written by **`reclaim`** — both routes are refused `operation-id-kind-conflict`
instead: reusing the id meets a marker whose kind is already `reclaim`, and a fresh id is adopted
into that same marker, whose kind is checked before any horizon is compared. The two routes reach
that marker differently. The reused-id route finds it by the id this call supplied, on a comment
this account authored, and then holds it to the run id and horizon it recorded — a disagreement
there is the `claim-metadata-mismatch` above; its runtime is attribution, never compared. The
fresh-id route finds nothing under the supplied id at all: `may_claim` picks this run's own live
acquisition by run id, and the call carries on under whichever id that marker already had. Neither
refusal tells this run that it *holds* the issue. The reused-id route is read from the whole comment
history rather than from who holds now, so a run that lost a race while its own marker stayed live
meets the same refusal without being the holder. Ask `verify_claim` instead of inferring ownership
from either. A run whose own prior acquisition on this issue has since died — lapsed, or pushed out
of the live epoch by a later foreign `reclaim` — is the one exception: the id it would reuse names a
dead event rather than one to resume, so this mints a fresh id and reissues once instead of
refusing. The reissue is a fresh claim, not a reversal of the takeover: a foreign `reclaim` whose
holder is still live is left holding the issue, and the fresh claim loses the race as any fresh
contender does. A publishing/requesting run cannot acquire its unresolved review handoff; expiry
never changes that answer.

`--transition-to <state>` runs the `transition` that always follows a claim in the same call. The
claim is adjudicated first, so its refusals are what a caller meets before anything moves, and the
transition's own refusals follow; omitting the flag claims without moving, which leaves the separate
`transition` call available.

## Handing an issue to another runtime

`handoff` states where an unheld issue is going, so a session of that runtime in that repository can
find it at `SessionStart`. It writes a local row naming the issue, the runtime, the run that handed it
over and the time, then projects exactly one `dev:<runtime>` label and reads it back twice. While the
git operation window is closed the projection is queued as a `reconverge` and the answer is
`projection: deferred` with the queue position.

An issue a live claim holds is refused `handoff-issue-held` before anything is written: the label
states a destination and must not overwrite the holder's. A `--to` that is not a single runtime word
GitHub accepts in a label is refused `handoff-runtime-unreadable`, and nothing is written either. The
record also explains that label to the
recipient's later `claim`, which otherwise refuses a `dev:` label this store cannot account for. A
handoff is not a claim, not a release and not a message: the recipient claims the issue, and delivering
the instruction is `send_message` addressed as `runtime:<name>` with `repo_dir`.

## Publishing a review target

requires a clean target. It **reuses** the single open PR or creates one and refuses on more than
one. A reused ready PR is converted to draft and draft readback is confirmed **before push**; a new
PR is created with `--draft` after its CI-silent topic push. Reused title/body are refreshed.
Readback must confirm draft plus exact head/base. The published marker and answer bind a fresh
epoch, PR, head, base, and complete path/mode/blob manifest digest; every republish creates a new
epoch and invalidates prior evidence. It also materialises per-path unified hunks against the
receipt's recorded base, once per epoch, and binds their digest as `payload-digest` on the published
marker. `payload_bytes` and `payload_digest` are always in the answer; the payload text itself is
**omitted by default** and `--inline-payload` is what asks for it; ask only for a reviewer you launch
yourself.
The text is what can make an answer too large for the MCP surface to return, and the threshold sits
inside the ordinary range rather than at the large end: answers carrying 73,272 and 145,756 payload
bytes were refused, one carrying 36,288 was not. Exceeding `REVIEW_PAYLOAD_CEILING` (262144 bytes)
refuses `review-payload-too-large` before any remote write; the payload is never truncated, whether
it was asked for or not. `next` names only the sequence that follows — transition to `review`, the
verdicts, `release_ci` with this receipt — and offers no second place to fetch the text: a republish
mints a new epoch and invalidates every verdict recorded against the first, so `--inline-payload` is
asked for at publication time or not at all. **It then starts the publication lane set**: every
active workflow under `.github/workflows/` is a candidate (minus the optional
`Publication lanes exclude` row, or an explicit `Publication lanes` list that replaces discovery),
and each is started once with `gh workflow run <file> --ref <branch>` against the head just pushed,
once per publication epoch and never per push, after the push and before the receipt is recorded so
a token that may not dispatch refuses while no epoch exists. A candidate without `workflow_dispatch`
answers `HTTP 422` and is not counted as started; discovery does not parse consumer YAML for the
trigger. Dispatching does not mark the PR ready. A repository with no dispatchable candidate is
reported in `publication_lane` and never refused; only `HTTP 401`/`HTTP 403` refuses, as
`publication-lane-forbidden`. The published marker records `lanes` / `lane-count` / `lane-note` for
every file that actually started, or the reason none did, so a narrowed gate cannot pass for a wide
one in silence; the answer itself carries only the state word as `publication_lane`, and
`review_read` with `view: transaction` answers `publication_lane_workflows`, `publication_lane_count` and
`publication_lane_note`. The published comment states the lane set this epoch started, so a reviewer
reading the timeline is told it whether or not the caller reads the receipt, and says the ordinary
pull-request-event lane still waits for the PR to be marked ready. A publication over one this issue
already recorded also records the **whole parent receipt** on the marker — `parent`, `parent-pr`,
`parent-head`, `parent-base`, `parent-digest` — and a `delta` digest covering both ends, derived
from the timeline rather than supplied by the caller; `publish_review`'s own answer carries none of
it, and `review_read` with `view: transaction` answers it back as
`parent`/`parent_pr`/`parent_head`/`parent_base`/`parent_digest`/`delta`. `parent-head..head` is the
delta a re-review reads **only when it is honest**: `parent-head` still an ancestor of `head`, the
base not having moved, and that range touching at least one path. A rebase onto an advanced base
breaks the first and a merge of one breaks the second; an identical-bytes republish breaks the third. The marker records which case this publication landed
in: `delta-range` carries `parent-head..head` verbatim when it is honest, `delta-range-reason` names
`"history-rewritten"`, `"base-advanced"`, `"merged-range"` or `"empty-repair-range"` (`review_tier` with `parent_head`'s own words) when it is
not, answered back by `review_read` with `view: transaction` as `delta_range`/`delta_range_reason`, never by
`publish_review`'s own answer. When the reason is `"base-advanced"` or `"merged-range"`, the head contains
the advanced base and `git merge-tree` merges the parent head and that base without a conflict, the
marker also records `delta-parent=integrated`: the targeted validator then reads the correction's own
diff from that merged tree to the head instead of escalating. A rewritten history, or a merge that
conflicts, records no such field and still escalates. Absent on a first publication and on any marker written before this
pair existed. The receipt is what makes the parent ledger readable by exact identity rather than by
epoch, which one backfilled marker could claim. A first publication carries none of them.
`--topology` records this delivery's execution topology — free text against `SKILL.md`'s Direct Work
Delegation table — on the published marker as `topology`, answered back by `review_read` with `view: transaction`
rather than by `publish_review`'s own answer; unlike `head`/`base`/`digest` it is **recorded, not
bound**, the same standing `payload-digest` already has, because Estigia cannot observe how an agent
wrote code. No declared topology is refused for its value: `inline` over any number of changed files is
recorded as typed, with or without a `--topology-reason`, because a file count cannot judge the
table's writing row. The same point also measures the caller's own fields: a
`--run-id`/`--topology`/`--topology-reason` whose marker-and-prose renderings would leave no room
under GitHub's 65,536-character comment cap — the frozen genesis manifest's own ceiling and the
publication lane's own measured width reserved — refuses `publication-comment-too-large` before any
remote write. A repair — a publication over one this issue already recorded — also runs the
**correction budget**, before the push, and the budget is scoped to the pull request rather than the
issue: the recorded parent is only ever one on this same pull request, so a pull request's first
publication records none; the same exclusion `review_status`'s `corrections` applies to its correction count, so the first publication of a second
pull request on this issue is never budgeted against a different pull request's frozen size.
Admission before arithmetic, both measured between two frozen Git tree object ids and never a
payload or a mutable ref. The first repair on a lineage freezes `budget-genesis-tree`,
`budget-genesis-paths`, `budget-original-lines` and `budget` from the *original* candidate's own
size, floored so a positive original never freezes a budget its own repair could not spend once;
every later repair only grows `budget-cumulative`. That frozen manifest is itself bounded before
anything is recorded: `budget-genesis-paths` is measured by the percent-encoded length
`\u{1f}`-joining it would actually produce on the marker, not the raw joined byte count — every
separator, and every byte outside the marker's own safe set, costs three characters once encoded
rather than one — and clearing `GENESIS_MANIFEST_CEILING` (32768 bytes) refuses
`genesis-manifest-too-large`, naming the encoded length, the ceiling and the path count, before
anything remote moves. A path this repair changed that is outside the frozen genesis paths — and,
when the base advanced and was integrated, outside the paths that merge conflicted on and the
advance's rename destinations — and is not an admissible companion test refuses
`correction-outside-reviewed-scope`, naming it; growing the cumulative total past the frozen budget
refuses `correction-budget-exceeded`. None of the three writes anything remote. The lineage's **corrections** are checked at the same point, before the budget: a publication over
a parent on the same pull request that stands rejected or escalated — a computed `rejected` or
`escalated` verdict, a retired `rejected` `review-verdict`, or a local `escalated` receipt — is a
correction, and when `review_status`'s `corrections` already counts as many on that lineage as
`Corrections per lineage` allows it refuses `correction-already-spent`. A publication over a parent nobody has judged is not a correction and is
not refused as one. A publication over a parent on the same pull request that was approved — by a
computed `approved`, or by a retired `accepted` verdict — is a **new candidate** and zeroes the count,
but it is not a new budget: it spends no correction and is still bounded by
`correction-budget-exceeded` and `correction-outside-reviewed-scope` against its pull request's
frozen genesis. It refuses
`approved-republish-without-cause` unless a `review-finding` has been recorded against that exact
parent receipt since that verdict, or the base **advanced** past the parent receipt's own recorded
base — that base is a proper ancestor of this publication's, direction included, so a base that
merely differs earns nothing — where the head carries the advanced base by the
`publication-base-not-ancestor` check above, and the integration may be a merge that keeps the parent
head or a rebase that rewrites it. A base that did not move forward is the author's republish
whatever the push shape, so an amend cannot pose as an integration. Nothing has been pushed and no
pull request was opened. An unmerged base advance never reaches this check at all:
`publication-base-not-ancestor` refuses it earlier, before the payload is built. When the base has
moved past what the parent publication's own history contained by being merged or rebased forward
into this branch, the round is measured against the tree git merges the parent head and the new base
into (`git merge-tree --write-tree`, git 2.38 or newer, refused naming the cause when it cannot run),
the frozen genesis, budget and cumulative carry over, the paths the merge conflicted on and the
advance's rename destinations are admitted beside the reviewed ones, and `budget-parent=integrated`
records that it happened; `budget-refrozen` is left
to a base that did not advance past the parent's. A first publication is never budgeted, and
only `budget-candidate-tree` is written for it.

`--with-review` adds the review tier and the first `review_status` to the answer. A
failed read is reported under `review_reads_unavailable`, not refused: a second
publication mints a new epoch. `--start-review` implies it, moves the issue to `review` and launches
every `launch_review_input` input unless the lane is `red` or unreadable; the outcome is
`review_start`. `--record-verdict` on `collect_review_input` records the computed verdict only when
nothing runs and `review_status` names `record_computed_verdict`; else it returns that status.

## Recording a finding

requires a live claim in `review` by the recording run, and records **one** observation against
**one** exact receipt. `severe` is a reproducible correctness, security, reliability or contract
defect with concrete impact; `warning` is a bounded risk with no demonstrated incorrectness;
`suggestion` covers optional improvement, cosmetic wording and preference. An unrecognised class is
refused rather than defaulted, because a classification nobody chose is how a preference becomes a
blocker. A finding with no `--id`, `--evidence` or `--impact` is refused for the same reason: one
that cannot be re-run cannot be checked and one with no impact cannot be weighed. Every `path:line`
or `path:start-end` citation `--evidence` or `--impact`'s prose contains is resolved against
`base`/`head` the same way `--location` is, and refused as `finding-citation-unresolvable` under one
of four causes — `malformed-path`, `path-not-in-either-tree`, `path-is-not-a-file` or
`line-past-end-of-file`, each named in the refusal table in [`github.md`](../github.md) — the free
text is checked, not merely stored. The lookup reads both trees from the recording checkout, so a run that has not
fetched the receipt's `base` and `head` fails with a git read error rather than recording, under any
class. It resolves only tokens that look like a path citation; the six shapes it skips, including a
raw trailing slash and an unquoted extensionless root-level path such as `Makefile:12`, are
enumerated in the `finding-citation-unresolvable` refusal. **A severe finding must also state both declarations** — how the claim is established
(`--evidence-class`) and who owns its cause on the timeline (`--causal-disposition`) — where the
same refusal as above names whichever one is missing; warnings and suggestions state neither, which
keeps the cheap observation cheap. The two words are stored outside the operation key like the rest
of the wording below, so a retry reads back what stands rather than minting a second event over
different declarations. `--id` is derived from the affected behaviour and location, never from the
run or the wording, so two reviewers reporting one defect agree rather than counting twice and a repair can
name what it answers. **`--location` names where in the diff the defect lives** — `path:line` or
`path:start-end`, the path repository-relative — and is required of a severe finding whose
`--causal-disposition` is `introduced` or `worsened`: the one claim `base..head` can actually
settle. A malformed value is refused as `finding-location-malformed` whenever one is given at all,
under any class or disposition. Whether it falls inside the change is checked only where a location
is owed, and by containment, not overlap: every line the location names must fall inside **one**
changed hunk of the named path, not merely share a line with one — a well-formed location that fails
this, or that names a path the diff never mentions, is `finding-location-outside-the-change` and the
refusal names the first line inside the claimed span that the diff never touched — except a path the
diff touched but changed only by deletion, by a binary edit or by mode alone, which settles by
having been touched, since no such hunk adds a new-side line to fall inside. That check is
consistency, not causation: it refuses a location the diff could not possibly support, and says
nothing about whether the finding is correct. The diff is read from the checkout this call runs
against, not from GitHub, so a clone missing `base` or `head` answers a failed read rather than a
location refusal. The operation is keyed on nine fields — run, reviewer, epoch, PR, head, base,
digest, `--id` and `--class` — so a retry after an unreadable answer reads back what it already
wrote rather than turning one observation into two confirmations. `--evidence`, `--impact`,
`--parent`, `--origin`, `--evidence-class`, `--causal-disposition`, `--location` and `--lens` are
deliberately outside that key: re-recording one finding with different wording is the same operation
and the first body stands, so correcting the wording means a new `--id`, not a replay. Findings bind
to the receipt and are **not** carried across a republish. Every finding must name the publication
**under review**; a well-formed receipt that is not the current one is refused with
`published-transaction-mismatch`, the same refusal the verdict gives. Across a repair, `--parent` names
the finding this one reassesses and must name one the parent **receipt** recorded — not merely one
carrying the parent epoch, which any later marker can claim; a `severe` finding naming no parent
must state `--origin introduced` or `--origin exposed`, while a warning or a suggestion new to the
repair states neither. `--lens` names the shipped role this finding came from — `review-lens`,
`review-risk`, `review-readability`, `review-reliability` or `review-readback` — refused as
`invalid-finding-lens` unless it names one of the five; a lens finding owes `--location` under any
class, checked on its path alone unless it is also severe with `--causal-disposition introduced` or
`worsened`, which owes the same containment above

## Recording a verdict

retired. No tool records an aggregate verdict: the verdict is computed from the receipt's ledger and
recorded with `review_computed_verdict`, and no reviewer records a verdict. A `review-verdict` marker an
earlier release wrote still parses and is read where history needs it: a distinct reviewer's
`rejected` one still refuses release at every tier as `legacy-verdict-rejected`, an `accepted` one
releases nothing, either resolves a handoff it was recorded against, and `review_status`'s `corrections` still reads a
rejected one as a receipt that stands rejected. An `approved` terminal receipt an accepted verdict recorded before the
retirement is history too: it releases nothing, but it still makes an approval final against
`escalate_review`.

## Recovering an escalated review

Recovery is **a person's command**, not an operation an agent can call: `estigia recover <issue>
--reason "<why>"`. `--epoch` defaults to the issue's one escalated, unrecovered receipt — the newest
publication of each pull request that stands escalated — and is refused `recover-no-escalated-receipt`
when there is none and `recover-epoch-ambiguous` (naming the candidates) when there are several;
`--authorized-by` defaults to the login `gh api user` reports, refused `recover-login-unresolved` when
`gh` names none. The long form, with `--epoch <epoch> --authorized-by <login>`, stays accepted and its
flags win. `review_status`'s `escalated` stop carries a plain `why` sentence beside the reason code
(and `record_computed_verdict`'s `escalated` entries carry one per cause), saying what happened and
what the person decides. It is not an MCP tool, and the gate refuses
an agent's shell line for it (recover-is-for-a-person) on every host, outside any stand-down, before any claim is read and
without asking, because a hook's `ask` is not a human step under every permission mode. An agent whose
receipt stands escalated with its lineage's corrections spent, or handed to a person by
`escalate_review`, tells the person that command and waits; while a correction is unspent it may
still be corrected. The command prints the pinned tier and lens focus in the person's own terminal, which
the agent cannot read; the person passes them on, and the agent acquires the panel the pinned tier licenses for the
successor — four lenses at `high`, the pinned focus's lens at `medium` — since `review_tier` does not
apply the floor and does not size it.

It recovers the publication named by `--epoch`, which must be the newest on its pull request and stand
`escalated` — a computed `escalated` verdict, or a local `escalated` receipt no distinct approval
superseded; the exact receipt is read off the timeline, not typed. The transport then asks GitHub, not
the caller, of the repository that hosts the pull request — the Repository host row's, while the
timeline is read from the tracker's — `gh api repos/{owner}/{repo}/collaborators/{login}/permission`. The role GitHub reports in
`role_name` must be `admin` or `maintain` — the `permission` field alone never says `maintain` — and
without a `role_name` the permission must be `admin`. `recover-not-a-maintainer` names what the login
holds, and an answer that failed or names neither is `recover-permission-unreadable`, never clearance.
The record, of kind review-recovery, is **local**: it keeps the receipt, login, role, reason, and the
tier and lens focus the successor inherits. Nothing is posted to GitHub. Repeating it reads back.

It releases nothing. The next publication on the pull request over the recovered receipt is the root of
a **successor lineage**: its `published` marker carries a recovered-from field, it is neither a correction
nor refused as a second one, no correction budget or scope is carried over, it owes the tier's lenses and
no validator, and `release_ci` still needs an approved computed verdict on its own receipt. The successor
and its corrections keep the tier the recovered lineage was reviewed at as a floor, and its lens focus. A
publication that carries the field when no readable recovery record matches its parent is refused
`recovery-record-missing`, not read as inheriting nothing. A lineage that begins by a first publication,
or over an approved parent, inherits nothing.

## Escalating a review

requires a live claim in `review`, a recorded receipt, and `review_status`'s `corrections` answering `correction_spent`;
`--escalated-to` names the person who takes the decision and `--reason` states the condition the
decision is handed over with — refused `invalid-escalate-target` and `blank-escalate-reason`
respectively. It records an `escalated` receipt in the local store — a **local record**, so this
operation writes no GitHub comment — the exit a spent correction forces: a change whose last
allowed correction is used stays in `review` and a person decides, and the decision is recorded rather than left in
prose. It does **not** release CI. `release_ci` refuses an `escalated` receipt as
`review-receipt-escalated` exactly as an escalation cannot substitute for an approved computed
verdict. An approval is final: a transaction that already carries an `approved` receipt is refused
`escalate-review-already-approved`, because an escalation cannot overwrite the approval that is the
only basis `release_ci` releases on. One `escalated` receipt stands per transaction, so a retry reads
back the escalation already recorded.

## Recording a refutation

requires a live claim in `review` by the recording run, and a credited `--refuter` distinct from the
publisher and every requester — the same distinctness rule a reviewer was once held to, asked of a refuter. Records **one** batch against
**one** exact receipt: `--outcomes` is a JSON array of
`{"id": <finding id>, "outcome": "corroborated"\|"refuted"\|"inconclusive"}` objects, rendered whole
into the marker attribute so `markers::render`'s own percent-encoding — not a hand-rolled delimiter
— is what keeps a finding id carrying a comma or a colon from corrupting a neighbouring entry. An
outcome word outside the three is refused as `invalid-refutation-outcome`; the same id named twice
is refused as `refutation-batch-duplicate-finding`; an id naming no severe `review_finding` recorded
against this exact receipt is refused as `refutation-unknown-finding`; an id whose finding's
`evidence_class` is `deterministic` is refused as `refutation-includes-deterministic-finding` — that
class already qualifies a rejection with no confirmation owed, so it never enters this batch.
Completeness is asked only of the `inferential` population: a batch that omits a standing
`inferential` severe finding this receipt recorded is refused as `refutation-batch-incomplete`,
naming every identity still missing; an empty array is well-formed when nothing standing is
`inferential`. `--outcomes` is outside the operation key, like `review_finding`'s
`--evidence`/`--impact`: a retry naming a corrected batch for the same receipt reads back the first
one rather than minting a second event. A receipt holds one batch: a later batch, from any refuter,
that is not the same set of outcomes is refused as `refutation-batch-already-recorded` rather than
overriding the first, and the same set again reads back. The immutable marker is
operation-idempotent.
`review_computed_verdict` reads what this recorded: a `refuted` finding is dropped from the ledger,
while `corroborated` and `inconclusive` keep it open

## Recording a disposition

requires a live claim in `review` by the recording run, and deliberately **no distinct context**: a
disposition is the author's own answer — what the run that answers for the change did about a
finding — so the distinctness rule a refutation carries would make it
unrecordable by the only context that can record it. Records **one** batch against **one** exact
receipt: `--dispositions` is a JSON array of
`{"id": <finding id>, "disposition": "acted-on"\|"filed"\|"declined", "reason": <text>}` objects,
with `"filed_as": <issue number>` on every `filed` entry, rendered whole into the marker attribute
so `markers::render`'s own percent-encoding keeps an id carrying a comma or a colon from corrupting
a neighbouring entry. A word outside the three is refused as `invalid-disposition-word`; a blank or
filler reason as `blank-disposition-reason`; a `filed` entry naming no issue as
`disposition-filed-issue-missing`, and `filed_as` on any other word as
`disposition-unexpected-filed-issue`; the same id twice as `disposition-batch-duplicate-finding`; an
id naming no finding this receipt recorded as `disposition-unknown-finding`; an id whose every
recording is `severe` as `disposition-includes-severe-finding` — such an identity is adjudicated by
the verdict and never by a disposition, while one also recorded as a `warning` or `suggestion` is
answered for that non-severe recording. The call is **optional**: a `warning` or a `suggestion` is
informational and blocks nothing, so no finding owes an answer and a batch may answer a subset of
the receipt's findings. The operation's identity is the set of findings it answers: the same set
answered again reads back the first batch rather than minting a second event; the answers themselves
are outside the key, so a retry never replaces them. Nothing reads the record as a gate. The record
is prospective and auditable, not independent: Estigia stores the author's declaration that a
finding was repaired, filed or declined and never confirms the repair landed, that the named issue
exists or carries the finding, or that the reason is true.

## Recording a lens report

A hand call records nothing: the report is written only by `collect_review_input`, after the binary ran the lens. What follows describes that record.

requires a live claim in `review` by the recording run and a `--role` one the receipt's review panel
can license — the four tier lenses `review-risk`, `review-readback`, `review-readability`,
`review-reliability`, or `review-lens` for the delta-lens repair below — refused as
`invalid-lens-role` otherwise. Records that one sealed lens **finished** against **one** exact
receipt, the newest on its own pull request, whether or not it found anything: a clean lens records
no finding, so without this event an empty ledger could not be told from a review that never ran.
`--focus` names the lens a `medium` change ran when it was not the default `reliability`, and is
refused as `invalid-lens-focus` outside the four. The record is idempotent on the receipt and the
role: a second report for one role, from whichever run, reads back the first. `review_read` with `view: verdict`
reads it, and so does `release_ci`. At `medium` every report names the focus its lens ran, an
unnamed one being the default; reports that disagree escalate the receipt as a finding
named lens-focus. The focus is carried by the reports themselves, not by the publication, so one wrong
report cannot be told from a right one until a second disagrees with it.

## Capturing a review input

`launch_review_input` and `collect_review_input` are the two calls that turn a lens, the refutation batch or the validator a
`collect` names into a recorded result. Both take `--name` for one slot or `--names <name,...>` for
every slot the `collect` offers in the same state, so a `high` review launches its four lenses in one
call and collects them in the next; each slot keeps its own claim check, and a batch reports a slot a
running job already holds (`capture-already-running`, a launch) or one still running (`capture-running`,
a collect) rather than refusing the whole call. `launch_review_input` re-reads `review_status` and refuses `capture-input-not-offered` unless the
answer is a `collect` that offers `--name` and `--name` is a lens role, `refutation-batch` or
`review-validator`; no
reviewer starts before that. The receipt is the one the
`collect` names. The reviewer is handed the diff rebuilt from that receipt, which must hash to the
`payload-digest` the publication recorded (`payload-digest-unavailable` when the marker carries none),
and the issue's body as the criteria. It is the first installed `claude` or `codex` CLI (the relay on an OpenCode run with its plugin installed, launched as the answer's `launch` prescribes), run with no
tools, run by a detached job under a 15 minute deadline; the launch answers `{job, deadline_at}` and records nothing. A slot a running job holds is refused `capture-already-running`. `collect_review_input` records the answer and lists what it recorded (`findings`: id, class, blocking, lens, location; `release_ci` lists the receipt's findings the same way), and only for the run that launched the job (`capture-job-other-run`; `capture-job-missing` when no job holds the slot). It reads `review_status` again and, unless it is still a `collect` offering the name against the job's own receipt, discards the job as `capture-result-stale`. `review_status` annotates each input it offers: `launch_review_input` when no job holds the slot, `collect_review_input` with the `job` when the job has finished, its runner died or it is a minute past its deadline, and `capture: null` with `running {job, deadline_at}` while it runs, in which case a collect is refused and the answer is to ask `review_status` again (`next.ask_after` is the earliest `deadline_at` when every input is running). A job still running is refused `capture-running` with its `deadline_at`, unless `--wait-seconds` is given: the call then waits that many seconds (at most 900, and never past the point the job would be abandoned, its `deadline_at` plus a minute) for the job to finish before refusing, so a caller need not poll with `review_status`; one a minute past it is killed and refused `reviewer-deadline`; a runner that died is `capture-runner-lost`. `reviewer-cli-missing` names neither being on `PATH`, and arrives from the job's result. Every finding in the answer is checked as
`record_review_finding` checks it before any is written, so one finding outside the change rejects the
whole answer as `reviewer-result-malformed` and leaves the timeline as it was. Otherwise each finding
is recorded under the operation id `record_review_finding` mints for it, with the lens role as both
`--reviewer` and `--lens`, and then the lens report is recorded. A refusal of any of those writes is
the recorder's own and passes through under its own code; one that follows a finding already written is
reported with the world `committed`. A second collect of a recorded job writes nothing. Launching again after an abandoned job starts a new reviewer: the findings that
landed stay, a finding the new answer repeats under the same `id` and `class` is read back, and any
other is added under its own id. Over a repair, the reviewer is told the parent epoch, that a new severe
finding owes `origin`, and the parent's recorded finding ids.

For `refutation-batch` the reviewer is the refuter, handed the same diff and criteria and, as its
findings, exactly the ids the `collect` lists. Its answer must name each of them once and nothing
else: a finding it names that was not offered, or one it leaves out, rejects the answer as
`reviewer-result-malformed` and writes nothing. Otherwise the whole batch is recorded in one
refutation write made inside the collect, credited to `review-refuter`; the recording run keeps its own claim and is
the attester. A refusal of that write passes through under its own code, `refutation-batch-already-recorded`
and `reviewer-not-distinct` among them, and a refusal of the answer's own contents rejects the answer.

For `review-validator` the reviewer is the validator, handed the correction's diff over its parent
(`parent.head..head`, from the publication's recorded lineage), the parent receipt's severe findings
and the issue's body. Its answer is checked with the shape check the validator result applies, and
an answer that is not a well-formed inspection is `reviewer-result-malformed` and writes nothing.
The attempt is not the answer's: it is the inconclusive results recorded when the job was launched, plus one, frozen so a collect that is repeated after a crash records the same result once. A
conclusive `fail` and an `unavailable` inspection are recorded, not rejected, and the collect's answer
carries the recorder's `outcome`; `validator-result-already-recorded`, `validator-attempts-exhausted`
and `validator-transaction-not-a-correction` pass through under their own codes.

## Recording a validator result

The validator result records what the read-only targeted validator found over one **correction**
— a publication over a parent on its own pull request that stands rejected or escalated; any other
receipt is refused `validator-transaction-not-a-correction`. It checks the correction's exact receipt
against two criteria, `--original-criteria` (did it fix the parent receipt's frozen severe findings)
and `--correction-regression` (did it break anything on the corrected paths), each `pass` or `fail`.
It is a **local** record in the store and posts no GitHub comment, and it requires a live claim in
`review`. A conclusive result is recorded once (`validator-result-already-recorded`), and a retry
under the same operation id reads it back.

A correction's computed verdict waits for it: until a conclusive pass stands, `review-validator` is a
missing role beside the tier's lenses, so `review_computed_verdict` refuses `computed-verdict-incomplete`.
A pass never overrides the ledger, so an open severe finding still computes `rejected`; a
conclusive `fail` computes `rejected` while the lineage has a correction left, naming the failure for the
next one, and `escalated` once it has none, whatever else the ledger holds.

A conclusive `fail` with a correction left writes nothing: the receipt stands rejected and `review_status`
offers the next correction. At the limit it writes the local `escalated` receipt, which `release_ci` refuses as
`review-receipt-escalated`, naming `original_criteria_failed`, or `correction_regression_failed`
when only the second failed. `--inspection unavailable` — the frozen trees could not be read — carries
`--reason` and no criterion, spends nothing, and is retried: each attempt is its own operation, named
by `--attempt` 1 to 3, and the third writes the escalated receipt with cause `validator_inconclusive`
(`validator-attempts-exhausted` refuses a fourth). `budget_exceeded` is not a cause here: an
over-budget correction is refused `correction-budget-exceeded` before anything is pushed, so no
receipt exists to escalate.

## Running declared checks

`run_checks` runs the `[[check]]` tables the base's `estigia.generated.toml` declares, as a detached job against the clean checkout, and records a receipt for the published target's digest and the declared commands. It answers `declared: false` and records nothing when the base declares none, `cached` when a receipt already stands, and `running` with `ask_after` while a job is out: call again, with `wait_seconds` (at most 50) to wait. A finished answer carries each check's exit, seconds and output tail. While the base declares checks, the verdict and `release_ci` refuse without a passing receipt for the exact digest: `checks-receipt-missing`, or `checks-failed`.

## Recording a computed verdict

`review_read` with `view: verdict` answers, and `review_computed_verdict` records once, the verdict one exact
receipt's ledger supports. The caller names the receipt and never the outcome. Each severe finding is
settled by the order in *What the ledger computes* in `policies/blind-judges.md`, which owns it. The outcome is `escalated` over `rejected` over `approved`, and
`approved` additionally needs every lens role the receipt's review panel licenses to have filed a
`review_lens_report`: for a first publication the tier is re-derived from `base..head` with
`review_tier`, and for a repair whose recorded lineage names the honest narrow range the panel is the
one `review_tier` with `parent_head` answers over `parent-head..head` — the delta lens alone for a
`warning`/`suggestion`-cleared narrow repair, or the repair's own smaller panel in place of the whole
change's. `low` licenses no lens and closes as `approved` on the structural readback alone, and a
licensed role with no report is listed in `missing_roles` and answers `incomplete` unless a finding
already decides the outcome as `escalated` or `rejected`.

The answer carries `outcome`, `open`, `escalated` (each with its `reason`), `follow_ups` and
`missing_roles`. Recording requires a live claim in `review` and refuses every outcome as
`computed-verdict-incomplete` while a licensed role has not reported, so the one marker a receipt
holds is final and records its tier as `level`. `approved` additionally requires the **publication lane for
the receipt's own head** to have answered green: check runs are read from
`repos/<owner>/<name>/commits/<head>/check-runs`, a failed conclusion refuses `publication-lane-red`,
a queued or running one refuses `publication-lane-unfinished`, an unreadable listing is a failed
read, and **no check runs at all proceeds** — Estigia cannot prove a repository has a lane. A
`rejected` or `escalated` verdict is never gated on it: a rejection over red bytes is the route out,
not the waste. `approved` also regenerates every file `estigia.generated.toml` declares on the base
and the change adds or modifies, which the payload showed as one `# estigia-generated` line instead of
hunks, and records only when each is byte-identical: `generated-reproduction-mismatch` or
`generated-reproduction-unavailable` otherwise, naming each path. Every distinct workflow run behind a red lane has to report `steps: []` for it to
classify `never-ran` instead of `red`: `--never-ran-evidence-file` names the caller's own local
verification, refused `publication-lane-never-ran` when absent or `blank-never-ran-evidence` when
blank or filler, and the marker records the classification and the evidence. Any one run that
genuinely ran and failed still refuses `publication-lane-red`. A retry
under the same operation id reads back the marker it wrote without recomputing; a recording under a
new operation id reads the first back only while the ledger still computes the same words and is
otherwise refused as `computed-verdict-already-recorded`.

`release_ci` and `goal_completion` never trust the marker alone. When the receipt carries a marker
or a lens report they recompute the verdict over the live ledger and the re-derived tier, and clear
only when it is `approved` and equals the marker: a ledger changed since is `computed-verdict-stale`,
a different tier `computed-verdict-tier-mismatch`, and a recomputed `escalated`, `rejected` or
`incomplete` is refused under that name. A complete ledger with no marker is
`computed-verdict-unrecorded`, except at `low`, which licenses no lens and needs none. An `approved` verdict also records its `approved` terminal receipt in the local store — a **local
record**, never a comment — crediting the recording run. It is what makes the approval final against
a later `escalate_review`, and a replay of the verdict records a receipt the first call could not.
An `escalated` receipt refuses `release_ci` as `review-receipt-escalated` whatever the ledger
computes, and only an `approved` receipt recorded by a run that is neither the publisher nor a
requester of a handoff supersedes it: the publisher cannot approve its own escalation away.

Only a computed
`approved` releases. A `low` receipt licenses no lens, so the same recomputation releases it with or
without a marker — the structural readback — and `release_ci` goes on to check receipt equality, the
live draft PR and `release-target-mismatch`. A `medium` or `high` receipt with neither a marker nor
a lens report is `qualifying-review-verdict-missing`. A `review-verdict` clears nothing: an
`accepted` one is ignored, and a distinct reviewer's `rejected` one refuses as
`legacy-verdict-rejected` at every tier, because the marker binds those exact bytes; a republish is a new receipt and starts without it.
The answer's `basis` names which route cleared it.

## Deriving the review tier

read-only, no claim required. Derives the review tier for `base..head` from two tree-to-tree
`git diff` calls, never the worktree: `high` when a hot-path token (`auth`, `update`,
`security`/`webhook`, `payments`), an adjacent `service`/`token` path pair, a shell-source extension
(`.sh`, `.bash`, `.zsh`, or a `.yml`/`.yaml` under `.github/workflows/` — this repository's own
publication lane included), or an execute-bit flip between the two modes (no eligibility gate — a
chmod inside `docs/` still escalates) is touched; `low` when at least one path changed and every
authored byte of it is proven passive prose from its own bytes and mode, and nothing configures
anything; `medium` otherwise, which includes a range that changed no path at all — that answers
`medium` carrying a `nothing_measured` reason and never `low`, because "every authored byte is
passive" is vacuously true of no bytes, and nothing was proven passive where nothing was read. What
the tier reads is a boundary rather than a size — the changed-path count at zero, the per-path
authored line count at zero, and the summed candidate blob bytes against the scan budget — so a
five-thousand-line documentation change is still `low`. `lenses` is a
fixed allocation per tier, not an audit of which lens could find a subject in
the diff — more than one sealed lens can plausibly have a subject in an ordinary code-touching
change, and this count does not measure that: `medium` answers one lens, the `--focus` argument's
(`risk`, `resilience`, `readability` or `reliability`, default `reliability`; anything else fails
the read); `high` answers four lenses, `review-risk`, `review-readback` (the resilience slot),
`review-readability` and `review-reliability`, whatever the focus; `low` answers zero, since nothing
authored leaves that one question a subject. `lens_roles` names the reserved reviewers the count
licenses. `--raise` with `--raise-reason` lifts the tier to `high` for one of six risk items the refusal
`invalid-risk-raise` lists, and never lowers it; `computed_level` keeps the derived tier. `--focus-reason`
is answered back. `review_tier` stores nothing; `publish_review`/`republish_review` take the same `--raise`,
`--raise-reason`, `--focus` and `--focus-reason` and record them with the computed tier on the
published marker, and the tier `review_status`, the computed verdict and `release_ci` derive then reads a
recorded raise as `high`, never below the computed one. A publication that declares no raise is sized by the diff alone. `review-lens` is in no tier's allocation; only the delta-lens repair below launches it. An unreadable byte in
`git diff`'s output, a tier its reasons do not explain, a path this port cannot normalise, or a raw
diff record it cannot parse are all read failures, not `medium`. Three conditions fail closed into a
tier rather than into a read failure: an over-budget passive-content scan, whose every deferred
candidate is marked active so it can never be `low`; an over-budget or unattributable content scan,
which reads `high`; and a range that changed nothing, which answers `medium` for the reason above.
A content scan reads `high` on an added process spawn or dangerous sink, or a first-line `#!`. Not ported: the source
project's own six-path review-authority self-reference — this repository's review authority is not
separable by filename.
Ordinary review licenses lenses only: the answer carries no judge count at any tier. The operation
still decides nothing itself — acquiring the lenses is developer step 5's to do, stated in prose there
rather than derived here.
`forecast` is the one line the run says before it launches anything: the tier, the lens count and the
expected spend, which is the `Review budget` figure times the lenses the answer licenses
(or a plain statement that no budget is declared). It is stated, never put to anybody for approval,
and the same line is carried by `review_tier` with `parent_head`.
Reads `base..head`, the whole change — a first publication's own range; a repair round reads a
narrower one through `review_tier` with `parent_head` instead, when that narrower range is honestly
recoverable

## Deriving a repair's tier

read-only, no claim required. The same classification as `review_tier`, over the range a repair
round actually reads: when `parent-head` is still an ancestor of `head`, the base did not move, and
`parent-head..head` touches at least one path, the answer reads that narrower range — the repair's own edits alone — and
carries `range: "repair"`; `"base"` in that answer holds `parent-head`, not `base`, because that is
the range actually measured. Otherwise the answer falls back to the whole `base..head` with
`range: "whole-change"` and a `whole_change_reason`: `"history-rewritten"` when `parent-head` is not
an ancestor of `head` — the branch was rebased onto a moved base or amended, and there is no
repair-only range left to recover from what the timeline records — `"base-advanced"` when the base
advanced past where this branch forked and was integrated, so the range still carries the advance,
which is read whole and never granted the delta lens — `"merged-range"` when the base did not move
but `parent-head..head` contains a merge, most often a side branch merged back, also read whole and
never granted the delta lens — or `"empty-repair-range"` when
`parent-head..head` touched nothing at all, which an identical-bytes republish
(`parent-head == head`) reaches directly: `git merge-base --is-ancestor A A` exits `0`, so ancestry
alone is not enough. A frozen tree pair was never a defence against any of this — a candidate tree
is the tree of a commit, and `git diff tree(A) tree(B)` is the same diff as `git diff A B`, so after
a rebase the repair head's own content already carries the base's advance regardless of which ids
name the ends. A first publication has no parent and reads `review_tier` instead, since there is
nothing yet to narrow against. `clearing_finding` names the class of the `review_finding` that
licensed the republish over an approved parent receipt — `severe`, `warning` or `suggestion`;
anything else fails the read, naming the allowed values. When it names a `warning` or a `suggestion`
and the range is the honest narrow one (`range: "repair"`), the answer additionally carries
`panel: "lens"`, `lenses: 1`, `lens_roles: ["review-lens"]` — the delta lens alone reviews it, in place of `level`'s
own allocation, which keeps its ordinary meaning either way. A `severe` clearing finding keeps the
tier's own `lenses`; so does the whole-change fallback, whichever reason it names — see
[blind judges](../../policies/blind-judges.md)'s repair-loop section

## Releasing an issue

`unassign` puts the issue down. Discovery is read-only, writes nothing whatever the call
carries, and answers the epoch to name; `--target-operation` authorises only that epoch. A run
holding the issue names none: the tool fills it from the claim's operation id, and after
a takeover the old epoch is refused `target-operation-mismatch` before any write. The release
converges the `dev:` label and, once the close is the registered delivery and nothing is
unpublished, puts the isolated checkout down.

`--evidence-file <f>` and `--spend <json>` (one object with `record_spend`'s fields, or an array)
fold the close in. A held run may omit a record's receipt fields; the tool fills them from its
review transaction and keeps any the record names. The close folds in, in the order the store allows: each spend record while the issue is in `review`,
`done` (from `review` only), the evidence note, the release. A bad spend refuses before any write. A stop after a
write landed is not a refusal: the answer carries `close_incomplete` (`step`, `failure`) beside what
landed and the claim is still held: repeat with the same `spend` (it replays), and without
`evidence_file` once `evidence` landed. Call it from `review`; after `done` a new spend is
`unexpected-state`.

## Delivering in one call

`deliver` is the tool that runs the tail of a publication that needs no judgement: the computed
verdict, `release_ci` when no `ci-release` marker names the receipt, the CI read, the merge, the
topology readback and the close, each through the operation that owns it. The merge goes through the
decision the `gh pr merge` boundary makes. It answers `delivered`; `pending` with `ask_after`, after
`wait_seconds` (1 to 50, default 20) of waiting inside the call; or `stopped` with the `step`, a
`reason` and `review_status`'s `next` when there is one. A stop writes nothing past its step, and a
second call resumes from the store without repeating a landed write. `spend` is `release`'s;
`appendix_file` is the agent's text appended to the generated evidence, required where no CI can
run. The steps and what each hands back are in *CI, merge and delivery* in delivery.md; a stop after
the merge landed carries `merged` and, from the release, `close_incomplete`, which is repeated as
*Releasing an issue* says.
