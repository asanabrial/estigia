# Changelog

Notable changes, newest first. The format is the one `changelog_notes` reads:
a version opens its heading, and the entry under it is what the tag and the
Release carry. That command fails closed on a missing or empty entry, because a
tag is immutable and notes invented at tag time are permanent.

## 0.1.0 — unreleased

The first version. Estigia is a **harness**: it does not ask an agent to follow
the workflow, it holds the tools.

### The harness

- **The board mirror leaves a card that belongs to another repository alone, and says whose it is.**
  Matching on issue number alone moved another project's cards: measured on
  2026-08-19, when a transition on this repository's #83 moved another
  project's #83 out of `Review`. The card is now picked by repository and not
  by number, and `audit_board` reports foreign cards rather than repairing
  them.

  **A foreign card is left alone, and the transition it collided with still
  runs.** The first shape of this refused the whole call, which was wrong twice
  over: it aborted a `transition` whose label had every right to move — on a
  shared board a number collision is the normal case, not a misconfiguration —
  and in `create` it answered *nothing was written* over an issue `gh` had
  already filed, leaving a caller who retries to file a duplicate. The mirror
  runs before the label edit and is best-effort by construction, so what it does
  with somebody else's card is leave it and say so.

  **The read-back picks the same card the writer picked.** It matched on number
  while the writer matched on repository, so a correct transition passed or
  hard-failed depending on the order the API listed the cards in: the label
  landed, the read-back read another repository's column, and the call reported
  that nothing was written. That was a third spelling of one rule, contradicting
  the two that agreed.

  **And a card that does not say where it comes from is not this repository's.**
  `belongs.is_empty() || belongs == home` took a content node carrying no
  `repository` as ours and mirrored it, leaving the `an unnamed repository` arm
  unreachable. That spelling arrived mid-change, in a commit whose subject was
  not asking `gh repo view` when the board is off; the first commit had it
  right.

  **A repository whose identity cannot be read mirrors nothing**, and cannot be
  confused with a repository named "". The mirror takes an `Option`, so a failed
  `gh repo view` arrives as `None` and the answer says the question could not be
  asked rather than reporting a sentence about the card. The identity is read
  once, before anything writes: it was asked again through `?` below the label
  edit, where a failure reported *nothing was written* over an edit that had
  landed.

  **The audit says how many cards it compared**, beside how many the board
  returned. The two differ by the foreign ones it set aside, and reading only the
  first told an operator a pass had examined cards it never looked at.

- **The gate records which role each delegated context ran as, for the length of the run.** Claude
  Code sends `agent_type` on every tool event fired inside a sub-agent, so the run pointer now carries
  what the gate **saw** rather than what a launch prompt declared, and `SessionStart` reads it back on
  a resume.

  Recording it on the allow path alone recorded nothing for exactly the contexts this is about — a
  judge measuring in its own directory makes calls the gate answers `Outside`, not `Allow`, and the
  reads it makes never reach the gate at all — so the store is keyed on the role set having grown as
  well. `docs/honesty.md` carries what that still leaves empty.

  **It is not an audit trail.** The pointer is keyed on the session and `SessionEnd` removes it, so no
  later run reads it; issue #83’s criterion asking for one is split out as #91. One publication
  claimed otherwise and two judges disproved it through the built binary; `docs/honesty.md` carries
  the measurement rather than a correction made quietly.

- **A delegated role can now do the work its own contract asks of it, and the judges that measure run
  as that role instead of outside it.** Estigia shipped six role definitions and **not one of them
  could run anything**: the reviewer carried a fixed `tools: Read, Grep, Glob`, and the planning
  phases' substitutions were `Read, Grep, Glob`, `Read, Grep, Glob, Write, Edit` and
  `Read, Grep, Glob, WebFetch, WebSearch` — two of them strict supersets of the third, and one of
  those two writes. So *read-only* was never the property all six shared; **no branch produced a
  shell** was. In a repository whose findings are established by running something — building the change, reddening a test, turning the
  fix off to see the suite go green — that reviewer cannot check anything, so every panel worth having
  was launched under a generic type, outside the role and outside every guarantee the contract hangs
  on it. Measured on issue #36 before this landed: twelve independent judge launches, and the
  read-only guarantee governed **none** of them.

  A new setting, `Evidence standard`, states the one fact only an operator knows — whether a verdict
  here is backed by reading or by running — and the reviewer's tool grant is derived from it instead
  of being a constant. `measuring` adds a shell **and nothing else**: `Write`, `Edit`, `Agent` and
  `Task` stay refused. That withholds four tools and not the capability — a shell writes, and it
  launches — so what confines a measuring judge is the isolation rule, not the grant. The default is
  `reading`, which is what every installation already had, so no upgrade widens anything by itself.

  Two consequences, both deliberate. `reviewer_is_static` used to mean one spelling and now means one
  of the spellings Estigia can produce, enumerated under an exhaustive match so a third standard
  cannot be added without arriving there. And the gate carries the row in `GateContext` beside the
  renewal window, narrowed the same way for the same reason: an unreadable contract answers `reading`,
  because a fault must not hand out a capability.

  The isolation rule moved with the capability rather than after it. It bound a run's checkout and
  said nothing about anywhere else a delegated context writes; it now covers every location. That was
  not a precaution — a five-judge panel sharing one scratch directory had one judge's script
  overwritten by another's and executed inside a third judge's checkout, while a fourth read the
  implementing run's planning notes. No checkout rule was broken and two verdicts still stopped being
  independent readings.

  `docs/honesty.md` carries what this does not buy, and the sharp edge is worth repeating here: a
  judge that measures writes inside the directory its launch hands it, and giving the role a shell
  does **not** put its mutations under the gate. What makes them safe is the isolation rule placing
  that directory outside the claimed checkout — a rule an orchestrator follows. A judge handed a
  directory inside it is measured against the launching run's claim and allowed.
- **A discharged human decision is not a verdict.** Returning built work from
  `blocked` to `review` still requires the configured panel against the exact
  latest receipt. Step 7 now routes an unresolved choice about what should be
  built to `analysis`, the same as a contradiction.

- **Delivery proof no longer inherits a steered Git environment.**
  `same_git_repository` and `head_of` used to shell out with this process's
  `GIT_DIR`, so an unrelated clone could spend another checkout's verdict.
  Both now use the unsteered invocation the fast-forward proof already built.
  A test with `GIT_DIR` and `GIT_COMMON_DIR` exported goes red against the
  previous callers.

- **`reclaim` takes the same optional `state` `claim` already publishes.**
  A takeover of a `review` issue used to stamp the pointer `in-progress`, a
  state nobody named, and the gate then refused every write with
  `unexpected-state`. The default remains `in-progress` and belongs to the
  caller. `Renew` still fills a missing state and never overwrites one.

- **A claim whose write lands and whose readback fails can be retried.**
  The run already holds the issue on the timeline; the retry used to mint a
  fresh operation id and refuse `already-owned-by-different-operation`. It now
  adopts the epoch the timeline names, writes no second comment, reports the
  horizon already on the marker, and lets the pointer record `issue`, `state`
  and `repo_dir`. A post-write view failure is a Write, not `read-failed`.

- **A cwd no live claim covers can no longer take a write out of the public gate.**
  payload_cwd still reads the key — the hook may name a checkout this process was not
  launched in — but a directory no holder covers is discarded and the call is answered the
  way the identical payload carrying no cwd is answered. Measured on 2026-08-16: cwd of
  C:\Windows, nested tool_input.cwd of .., and a write carrying cwd of
  C:\Windows were outside with exit zero. The doc no longer claims the model cannot
  compose the key.

- **`verify_claim` treats this run''s own branch-link auto-close as delivery, not as a stand-down.** After merge, the ordinary renewal answers `closed-by-own-delivery` with the delivering SHA and receipt epoch so `transition --to done` is reachable. A close that is not this run''s receipt is still `issue-not-open`. An unreadable closer listing remains a failed read.

- **OpenCode is told to launch Task judges, not to hand off because review-blind is a Claude type.**
  `skill/references/runtime-notes.md` said the reserved role does not cover OpenCode launches and
  that an absent capability uses the durable handoff. Read together, every OpenCode publication
  handed off. The reserved type is a launch mechanism; a host that can spawn a subagent already has
  the capability. The installed `estigia.opencode.md` now states the Task path. Handoff remains for
  a launch that failed. Estigia still cannot prove the panel ran.

- **A run pointer that is there and cannot be opened now stops the write and names the file.**
  `session::load` collapsed every `read_to_string` failure into a fresh unsworn run, so a pointer the
  filesystem refused — a directory at its path, a permission failure, a transient I/O error — read as
  *this run swore nothing*. The gate answers that reading `outside` **before** the tracker is asked,
  so the writes of a run that still held its claim stood aside from measurement while `verify_claim`
  went on answering `ok`: measured on 2026-08-15, when `claude-81d69d3e372497b6` held issue #26
  through a reinstall that took its pointer, then published, released CI, merged and closed with the
  gate reading nothing. A disarmed gate looks exactly like a gate with nothing to do, which is why
  this direction is the one that had to change. Only `ErrorKind::NotFound` answers absence now; every
  other read failure loads as unreadable and carries the pointer path and the underlying error into
  the refusal, so `run-pointer-unreadable` — from the gate and from the MCP boundary, which build the
  same sentence — names the file an operator has to look at instead of only the run that lost it. The
  reason word is unchanged, so the documented refusal inventory is unchanged.

  The installer half is the question the issue recorded as **not measured**, and it is a measurement
  now rather than a reading of the call graph: `forget_state` is wired only into the take-out path, and
  `a_live_run_pointer_survives_a_plain_reinstall` sets up every adapter twice over one live pointer and
  asserts the bytes are the same afterwards. `uninstall --all` still takes the state with the last
  agent — the operator's recorded requirement, and `taking_estigia_out_takes_its_own_state_with_it_and_not_before`
  still holds it — so `docs/what-it-writes.md` now draws the line between live run state and what the
  person keeps, where it used to say `~/.estigia/` stays unconditionally.

  **`NotFound` alone does not prove absence, and Windows is where that stops being pedantic.** The
  acceptance criteria asked for a pointer whose parent is a file, expecting a read failure that is
  not `NotFound`; measured, Windows answers `ERROR_PATH_NOT_FOUND` — os error 3, mapped to
  `NotFound` — while Linux answers `NotADirectory` for the same shape. Reading the error kind alone
  would therefore have left the whole point of this change open on one platform in its widest form:
  a state root that is a file makes **every** pointer read as absence, so every run reads as unsworn
  and every gate stands aside at once. Absence is now confirmed against the root rather than taken
  from the kind, and a root that is simply not there is still absence — that is what a machine looks
  like before anything was ever claimed, and refusing it would refuse every first run.

  Two deterministic fixtures reach the arm on both platforms, because the incident itself does not
  reduce to a script: a directory at the pointer path is a read failure that is not `NotFound`, and it
  is posed at the loader, at the gate, and across every MCP tool that takes a run id. One older test
  moved with the fix rather than being kept green: the directory fixture in
  `a_pointer_that_could_not_be_written_says_so` was posing a write failure, and the same bytes are now
  refused one step earlier, at the read — the remaining write failures are environmental, no std-only
  fixture poses them on every platform, and the test says so rather than claiming the coverage.
- **A rejection now rests on a severe finding, and a repair records what it repairs.** Durable review
  evidence reduced every observation a reviewer could make to one bit. A preference about a word cost
  exactly what a reproducible correctness defect cost: a rejection, a republish, a new epoch and a full
  re-review of work that was already settled. This repository paid that on its own deliveries, and
  afterwards the tracker could not tell the two apart.

  `record_review_finding` is a new operation — the twenty-second — writing one immutable marker per
  finding, bound to the exact publication receipt, carrying an identity, concrete evidence, stated
  material impact and one of `severe`, `warning` or `suggestion`. It refuses a finding missing any of
  the three, because a classification that cannot be re-run cannot be checked and one with no stated
  impact cannot be weighed. `record_review_verdict` then refuses `rejected` unless **that reviewer**
  has recorded a `severe` finding against that exact receipt — the reviewer's own findings, not the
  panel's pool, since two contexts each holding one suspicion is not one confirmed defect. An
  acceptance carrying warnings and suggestions is still an acceptance and still releases CI.

  Operational failures deliberately keep their existing fail-closed refusals. A missing reviewer, an
  unreadable target or a stale receipt is not a review finding, and recording an outage as a cosmetic
  acceptance is the mislabelling this rule exists to stop.

  The second half is lineage. A publication over an earlier one records the **whole parent receipt**
  and a delta digest covering both ends — **derived from the timeline, never supplied by the run being
  reviewed**, because a run that could name its own parent could name the epoch whose findings were
  mildest. The receipt rather than its epoch: an epoch is not a function of the bytes it names, and a
  finding's epoch field is whatever the finding says it is, so a parent ledger matched on the epoch
  alone could be written into after the fact by recording a finding that named the parent epoch and
  carried the repair's own bytes. The parent's findings stay where they are; nothing rewrites them. What a re-review owes
  is the reference: a finding that reassesses one names it, and the name must exist against the parent
  receipt — an epoch alone is claimable by a marker written after the repair; a `severe` finding new to the repair states whether the repair `introduced` the defect or
  `exposed` one already there. A warning or a suggestion new to a repair owes nothing, because pricing
  the cheap observation is the defect this whole change repairs.

  Every finding must also name the publication **under review**, not merely a well-formed receipt.
  That check was documented before it existed: the first version of this change shipped two sentences
  saying a stale receipt was refused while the operation only validated the receipt's shape. Four
  reviewers found it, two drove it, and the repair is the check rather than the retraction —
  `docs/honesty.md` records that history beside the limit.

  What is checked is shape and reference. Nothing here can tell whether a `severe` finding is severe
  or whether a repair introduced what its reviewer says it did, and the delta digest names a repair
  rather than scoping a review — `docs/honesty.md` carries both limits in those words.
- **A transition to the state an issue already holds no longer strips its state label, and a
  read-back that disagrees no longer says nothing was written.** `transition` appended the removal
  whichever state `--from` named, so `--from done --to done` built
  `gh issue edit N --add-label status:done --remove-label status:done` — a self-cancelling edit that
  `gh` settles by dropping the label. Measured on issue #3 on 2026-08-15: the timeline shows
  `unlabeled status:done`, nothing put it back, and the issue carried **no** state label at all —
  invisible to `list_state` in every partition, and disagreeing with every
  `verify_claim --expect-state`. The removal is now skipped when it names the label being added, so
  the call still ends with the issue in `--to` and there is no window where it is in nothing.

  The half with teeth is the report. `label-readback-failed` is a stop and it carried no `world`, so
  the envelope rendered *nothing was written* with *do not repeat this call* under it — over an edit
  that had already run, and against the one call that repairs what it left. It now carries
  `"world": "committed"`, the road `MutationOutcome::Committed` opened after issue #1 in
  `publish_review`, so the sentence reads *the write landed; what failed came after it*; and its
  resolution names the repair — the same transition with `--from` omitted, which removes whatever
  stale state labels are found and is what actually restored issue #3 — instead of forbidding it.
  The reason word is unchanged, so the documented refusal inventory is unchanged.

  Both halves are separately measured, and the stand-in `gh` gained a **label store** to measure
  them at all: `nth` lets a scripted world change its mind on a schedule and cannot let a read answer
  with what the write before it did, so every fixture answered the same labels before and after
  `gh issue edit` and a transition that destroyed the label was indistinguishable from one that kept
  it. The board mirror is untouched and still runs first, which is asserted on the wire in the same
  call rather than inferred.

- **CI now answers before the reviewing, instead of after it.** `.github/workflows/ci.yml` gains a
  `workflow_dispatch` trigger, `publish_review` and `republish_review` start **one run of it per
  publication epoch** against the head they just pushed, and `record_review_verdict` refuses to bank
  an `accepted` verdict for a head whose lane concluded red or has not finished. Before this, the
  first cross-platform signal arrived when `release_ci` marked the pull request ready — after every
  verdict had been obtained — so a platform-only defect killed the receipt and every verdict bound
  to it. Measured on issue #2: sixteen reviewers across eight rounds discarded for a missing
  `create_dir_all` in a fixture that only POSIX could see.

  Both halves, because the first alone is a rule somebody has to remember, which is the failure
  issue #28 concluded against. The refusal sits on `record_review_verdict` because it is the single
  writer of a verdict marker — `release_ci` reads what it wrote — so one enforcement point covers
  every route to one, including the handoff route. Check runs attach to a SHA by construction, so the
  evidence is read for the receipt's own head and `ReviewReceipt` is unchanged: no run id, and
  nothing new for a republish to invalidate.

  **The receipt comment on the timeline now names the lane this epoch got**, because the sentence it
  used to carry — *CI remains blocked while the PR is draft* — became false in the same call that
  writes it. That comment is what a blind reviewer reads, so one operation was answering that a lane
  had started and an accepted verdict waited on it while posting a note saying CI would not run at
  all. It now says which of `started`, `absent` and `unknown` this publication got, and says the
  ordinary pull-request-event lane still waits for the pull request to be marked ready — the draft
  barrier is unchanged, and only the publication lane is new.

  **A head with no check runs at all proceeds**, unchanged. A repository with no dispatchable
  `ci.yml` has none, Estigia does not parse consumer YAML to find out whether it has a lane, and
  refusing there would have broken every consumer in order to protect the ones that answer. Only a
  token that may not dispatch refuses, `publication-lane-forbidden`, and it refuses **after the push
  and before the receipt** — so the branch is on the remote, no epoch exists, and running the same
  call again after granting `actions: write` publishes cleanly. Red and unfinished lanes refuse
  `publication-lane-red` and `publication-lane-unfinished`, each naming the lane, the run and the one
  command that clears it. An unreadable listing is a failed read and never a green lane.

  One run per **epoch** is not the cardinality the draft barrier refused: topic push, open,
  synchronize and reopen still start nothing, and a dispatch does not mark the pull request ready.
  What this cannot do is protect its own delivery — `workflow_dispatch` has to exist on the default
  branch before it can be dispatched against a topic ref — and what it does not prove is in
  `docs/honesty.md`.

- **A run's second issue no longer collides with its own first checkout.** The worktree template was
  made to carry `<run-id>` and nothing made it carry `<branch>`, so a template without one gave every
  branch of a run the same directory: a run working a queue met the checkout it made for its previous
  issue and was refused `worktree-path-occupied` — correct for what the check could see, wrong that
  two branches of one run were asked to share a path at all. The workaround was a manual
  `git worktree remove`, which is the one action that refusal's own guidance says not to take lightly.
  Both scopes are now added in memory when absent, as siblings and never as nested children:
  `…/<branch>` becomes `…/<branch>~<run-id>`, `…/<run-id>` becomes `…/<run-id>~<branch>`, and a
  template naming neither becomes `…~<branch>~<run-id>`. `estigia.local.md` is still never rewritten
  and `template_migrated` still reports the substitution. The sibling rule matters more in the new
  dimension than the old one, because a run-scoped legacy path is a checkout that run owns and a
  nested child would put a worktree inside a worktree. `run_scoped_template` is now
  `scoped_template`, since it scopes two things.

  Worth stating because it is why this took two attempts: the collision was invisible on the machine
  that found it, whose operator had since configured a template naming `<branch>`. Four checkouts,
  one run, no collision — and a configuration difference read as a fix. The composition code had
  never changed. The tests now use the templates an operator actually writes, including the bare
  absolute directory the settings table documents and the skill ships `unset`.

  The `legacy-worktree-registered` stop, its payload key and the binding section that is its only
  written recovery all described one dimension and now describe both. One residual is recorded in
  `docs/honesty.md` rather than closed: the stop cannot name the legacy path of a template that named
  no placeholder at all, so on upgrade git refuses that resume instead of Estigia — before any remote
  write and with nothing lost. The delivery reference, which framed worktree uniqueness as per-run
  only, the framing that hid this defect, says both dimensions now. And a sentence in the operator's
  own recovery claimed a detached legacy checkout does not stop `start-branch`; it does, reporting
  `occupied_by_branch: null`, and there is now a test posing that case rather than a proofread.

- A row about **this machine** now reaches every installed contract, and cannot be given a different
  answer for one agent. `Setting::scope()` has three values and both write paths were written as
  though it had two: the plain `config set` asked `elsewhere()` only for `Scope::Everywhere`, so
  `Summary language` and `Issue body language` landed in the canonical contract and nowhere else;
  and `config set --agent <slug>` refused an `Everywhere` row while accepting a `Machine` one it
  could not hold, since a shared root's per-agent file is rendered through `render_some_agent_rows`
  and read through `Scope::Agent` — the row was dropped on the way out and the command exited on its
  own read-back, reporting `setting-shadowed-by-local-file` and blaming an operator file that need
  not exist in that root at all. Two agents on one machine could therefore answer a question that
  has one answer differently, with **no command that made them agree**, and nine of the eleven
  adapters share the neutral root, so the unreachable case was the common one. The plain form now
  propagates a machine row exactly as it propagates a repository one, the per-agent form refuses it
  and names the command that holds it, and `doctor`'s `canonical` row offers that command instead of
  the sentence saying nothing cleared it — a fork that existed only to describe the gap, removed
  with it. `config set --repo` refuses a machine row saying it is a fact about the machine rather
  than what one agent does, and points at the command that holds it instead of at `--agent`, which
  is the refusal one command later. The three `shadowed*` refusals no longer name a local override
  they did not find: one lookup answers `Option` for all of them, and where there is no
  `estigia.local.md` the refusal is `setting-not-read-back`, saying what was observed rather than
  naming a cause it cannot see. Only the first of the three is reached today — `local_override` tests
  `is_file()`, so an unreadable file still answers `Some`, and a root with no file at all round-trips
  what was written. The other two branches are held by the type rather than by a test.

- `Blind judges` now has canonical `five blind` beside `single` and `two blind`, with `single` still
  the default. Five independent reviewer contexts run concurrently over the identical immutable
  target and criteria; a severe finding blocks or authorizes automatic repair only when 3-of-5
  independently confirm the same finding. One or two confirmations remain suspicions, ambiguous
  identities do not aggregate, and dissent, warnings and suggestions survive. Every Claude Code setup
  installs one inert `review-blind` definition with `model: inherit`; it remains inert unless
  a launch names the active blind mode, exact receipt and criteria. The orchestrator supplies the
  effective `judge` model when it launches that definition twice or five times. Config writes never
  mutate it. Setup refuses an unowned or changed copy before writing other artifacts, and uninstall
  deletes only a textually unchanged owned copy after normalizing line endings and the final newline,
  preserving other changes and deletion retry evidence. Fresh and upgrade action manifests name the
  ownership-record mutation exactly once. The lifecycle
  digest now inventories exactly five SDD definitions and that reviewer in one of its two top-level
  asset collections. Transport still enforces one aggregate exact-receipt verdict only: Estigia cannot
  prove panel size, concurrency, independence, blindness, same-finding identity or quorum.
  `review-blind` is now reserved to the operator-owned Claude definition. Generated Claude hooks wake
  for current `Agent` and legacy `Task` launches, walk from the launch cwd through the first `.git`
  repository root, parse candidate frontmatter as YAML, recursively reject project shadows or
  unprovable candidates, require the canonical user text to be the unique user-scoped reserved
  definition, and enforce the embedded policy once running. Setup performs the same user-tree
  uniqueness preflight before writing, revalidates at the reviewer boundary, and publishes a fresh
  reviewer with a no-replace atomic create so a concurrent file is preserved rather than overwritten.
  Refused launches never count or degrade the panel; ordinary agents remain project-first and OpenCode
  is out of scope.
- `estigia mcp` serves the workflow operations as **21 MCP tools** over stdio —
  claim, verify_claim, heartbeat, transition, comment, reclaim, release,
  start_branch, publish_review, and the read-only checks. Hand-written
  JSON-RPC rather than `rmcp`: the same binary answers a `PreToolUse` hook on
  every edit, so an async runtime per process is a cost paid thousands of times
  to move a few lines of JSON across a pipe.
- The one force-push the delivery path needs is inside the harness. After a rebase onto a moved base,
  or an amended commit, the ordinary push is refused as a non-fast-forward, and the sequence a run
  actually performed was *leave Estigia, `git push --force-with-lease` by hand, come back and publish
  again* — measured on a live delivery where the base moved mid-task. The claim itself was checked
  even then: `git push --force` is a boundary, which always re-reads and has no renewal window. What
  the hand-run push lacked is the rest — no lease against the head a receipt recorded, so it
  overwrote whatever the remote held; no record tying the new bytes to that receipt; and no cover at
  all when typed into a terminal the gate does not watch. `republish_review` is that step,
  adjudicated: it reads the head the latest `published` marker
  recorded and pushes `--force-with-lease=<branch>:<that head>`, so a remote somebody else moved
  refuses the push instead of losing their commit. The renewal stands **immediately** before it,
  after the fetch, target derivation, keyword scan, pull-request listing and draft conversion that
  separate it from the first check — for the fast-forward push that gap costs a refused push, and for
  this one it costs history. An issue with no recorded publication is refused rather than forced over:
  the first publication is `publish_review`'s, whose non-fast-forward refusal is the check that would
  be skipped. It is a separate operation rather than a flag because the last thing this must keep is
  that `publish_review` never force-pushes implicitly, and two entry points hold that where one
  function with a boolean would only ask a reviewer to. **Every round of independent review it went through**, two
  blind contexts each, rejected it before this shape, and every finding is fixed in it. The count is
  deliberately not written: it drifted twice, and a number nothing crosses is a number that will. The ones worth
  naming: the refusals downstream of a write said *nothing was written*, because they reached the
  agent through the same `stop()` and `?` every other path uses and those envelopes carry no `world`
  — they now name **which** writes happened, one clause each, and stay silent when none did; the
  harness did not fill in the isolated checkout for this operation as it does for `publish_review`,
  so an agent omitting the optional argument had its target derived from the base checkout and
  learned about it *after* the force-push, in a message blaming a push nobody had made;
  `estigia config`'s list of the operations this transport performs had drifted from the dispatch by
  three, now crossed by a test rather than by a comment saying nothing catches it; and the guard that
  proves the ordinary publication adjudicates its claim is behavioural, because two attempts to hold
  it by counting a function body were both satisfied by a call the mutated route never made.
- The gate now adjudicates an OpenCode call against the directory it will actually run in. That
  plugin launches the gate from its project root, because OpenCode's plugin context carries a project
  and no session identity to mint a run id from — so with two runs each holding an isolated worktree
  inside one base checkout, the gate was handed the base. Both runs cover it at equal depth, and the
  call was refused *"2 runs on this machine hold this checkout"*: correct about the directory it was
  given, and the directory was the wrong one. Measured on 2026-08-16 with two live holders of this
  repository, a `git commit` and an `estigia config set` explicitly targeting one worktree were both
  refused, and the refusal advised releasing one of the runs — which is the concurrent isolation both
  were using. For a Bash call the arguments the plugin already forwards carry `workdir`, the
  execution directory, and nothing read it; `narrowed_by_the_call` reads it now, for Bash and no
  other tool, resolving a relative one against the directory the process was launched in.

  **It may only narrow, and that distinction is the change.** `cwd` is written by an adapter's hook,
  which knows what it is gating, and is taken as given. `workdir` is a tool *argument*, so whatever
  composed the call wrote it — a model, on every runtime here. Read as freely as `cwd` it stops being
  evidence and becomes a lever: measured under review, with two live pointers and a `git push` under
  a claim, a `workdir` of `..`, of the parent checkout, or of `C:\Windows` resolved, was covered by no
  run, and was answered `outside` with exit **zero**. The command still ran where it was going to
  run; the gate simply stopped adjudicating it, and the same spelling reached `write` and `edit` too.
  A payload that can move the decision is a payload that can leave the gate, which is worse than the
  false ambiguity the key was read to fix — the widened gate that looks exactly like working
  correctly. So the resolved value is **placed** — its spelling collapsed the way the platform
  collapses it, then resolved as far as the filesystem exists — and one landing outside the launch
  directory is discarded in favour of it, which is where the decision sat before this key was read at
  all. Both blind reviews of the unclamped version rejected it on precisely that.

  Placing it is the clamp rather than a refinement of it, and the second round of review is why. A
  first attempt compared with `covers`, which is written for working directories *that exist* and
  falls back to the path as written when resolution fails; `..` was then never cancelled, so
  `wt-a/../../nope` still started with the launch directory and was attributed to the holder of the
  component it climbed **through** — measured as `allow`, exit zero, under a claim the call had
  nothing to do with. Strictly worse than the escape it replaced, which at least reached `outside`.
  `placed` already existed for exactly this, and its own doc names the failure; what it needed was to
  be the thing the gate asked.

  `holders_of` is untouched: its closest-worktree selection was always right when given the real
  directory, and the equal-depth ambiguity at a shared base is genuine and still refused. The alias
  is interpreted in one language rather than two — the plugin forwards and translates nothing — and
  the test that proves the plugin forwards it **executes** the plugin under `node` rather than
  reading its source. Both workflows install `node` now rather than trusting the runner image to
  carry it, on the reasoning the interpreter guard already stated for Python: a step needing a tool
  the job never sets up fails on a tag, which is the worst moment there is. `docs/honesty.md` records
  what per-call evidence exists, for which tools it does not, and the two things about it that are
  still uncrossed — the argument's spelling, and the base a relative one is resolved against.
- A claim governs a repository, not the machine. The gate classified writes by the checkout the hook
  was invoked in and never by the **path being written**, so a scratch note or an agent's own memory
  store, written from inside a claimed repository, was a repository write — and once the issue closed
  on merge it was refused with *issue is CLOSED*, at the exact moment a run had a delivery to write
  down. A write whose path lies outside every checkout the claim covers now stands aside as
  `outside-the-claim`, decided before the tracker is asked so the issue's state never reaches it. It
  is deliberately narrow: only an absolute path, only a run that covers something, and only a
  `Routine` write — the control surface sits outside the repository by nature, and watching it is the
  defence against an agent switching the gate off with the tool it uses most. `gh`'s hosts file joins
  that surface in the same change: it decides which account every tracker call acts as, so it is a
  boundary write in both spellings rather than something the stand-aside carries past. The answer is
  given after the contract check, not before it, so *an unreadable control surface permits no write*
  reaches it as it reaches everything else. That held only outside the renewal window until issue #29:
  the window's `Allow` answered above the contract refusal, so for its width a routine write went
  through with no `SKILL.md` on disk at all. The contract check now answers above the window, and the
  window still short-circuits below it when the contract is present — so the sentence reaches inside
  the window too, and the fast path it protects is unchanged. A path that **lands** anywhere other than a drive is
  declined rather than placed, and a declined path reads as inside. Windows serves every local drive
  as an administrative share, so `\\localhost\C$\repo\src\main.rs` against `C:\repo\...` is one file
  under two spellings, which stood the gate aside for a write that landed inside the claim. Judging
  the landing rather than the spelling is what also covers a drive letter mapped onto that share, and
  a directory link pointing at it: both arrive wearing a drive and resolve onto the share anyway. A
  share is the only non-drive landing anyone has produced — the device namespaces read as though they
  would be another, and `\\.\C:\Windows`, `\\?\Volume{...}\Windows` and
  `\\?\GLOBALROOT\Device\HarddiskVolume3\Windows` all resolve to `C:\Windows`, so they place normally
  and a write through one of them outside every checkout now stands aside where it used to be gated.
- The files carrying Estigia's own authority are watched. `setup` writes a workflow-authority
  directive — the sentence telling an agent this harness holds the authority at all — into each
  agent's instruction file, and the gate called ten of the eleven `Routine`; OpenCode's was already
  covered by the `.config/opencode/` entry. Those ten, plus gemini-cli's second spelling under
  `%APPDATA%` — it is the one adapter whose file moves with the platform — plus
  `~/.claude/settings.local.json`, the file an operator is told to put machine-local overrides in, and
  `~/.claude/agents`, where a definition carries a tool allowlist: **thirteen**. Each is outside every
  checkout by construction, which is what made it urgent — the stand-aside added for a closed issue's
  scratch note answers before the tracker, so they had moved from *measured against the claim* to *not
  gated at all*. They are `Boundary` now, on both roads.

  The instruction files are derived from the adapter table rather than spelled, because a hand-written
  copy agrees with the installer only until somebody renames one. The population test that crosses
  them against `resolve_paths` gained four dimensions on the way, each because a reviewer found a
  surface behind one: three platforms, two `XDG_CONFIG_HOME` layouts, both roads, and the bare root as
  well as a file inside it. What that uncovered is in the same change —
  `%APPDATA%\gemini\settings.json`, where the Gemini gate is registered on Windows; opencode's plugin
  and crush's settings, anchored to what the installer writes *beside* the directory entry rather than
  replacing it; and the rules **directories**, because gating a filename inside a directory the host
  reads whole is defeated by a neighbour. A fragment ending in `/` names a directory now, matched by
  what is under it or by itself, and not by a name that merely ends alike — nor by one that
  starts alike, because a fragment naming a directory is anchored on the left as a dot-fragment
  is. Anchoring only the dot ones kept the two roads disagreeing on the four entries this change
  added: `surface_of` gives every token a trailing separator, so a bare `.opencode/agents` was
  `Routine` to `Write` and `Boundary` to `rm`, with nothing red because no fixture asserted the
  `Routine` direction for a fragment without a dot. What stays unanchored is the fragments naming
  a **file** without one, and `cli/hosts.yml` is why: `%APPDATA%\GitHub CLI\hosts.yml` holds it
  mid-segment. `docs/honesty.md` measures what that leaves.

  Anchoring cost coverage on the shell road three times before it was right, and each loss was a
  spelling the base commit had gated with a bare `contains`. Splitting on whitespace and appending a
  separator left a relative operand with a space in front of it, never a separator, and never at
  position 0 because the verb is there. Wrapping each token reached only the operands whose first
  character *is* the fragment's — a quoted operand, a redirect written with no space, and an operand
  joined to a long flag each put a character in between, and the drive-relative Windows `C:` did the
  same while also reaching the **write** road. Every one of them named the run pointer or the file the
  gate is registered in, by the shortest line there is, and every one of them left the whole suite
  green, because the fixtures here spelled their commands plainly and their paths absolutely.

  What is in the code is one rule rather than a list of shell shapes: every character a path segment
  cannot contain is read as a separator, and the line is wrapped in one. Four fixtures hold it — three
  for the surfaces, one for the ordinary commands a run issues all day, since a more permissive view
  is a live tracker read on every build if it over-matches. Reviewers measured all three attempts;
  `docs/honesty.md` carries the sequence, and says what the folding still cannot see.

  `<repo>/.opencode/agents` joined the list for the same reason it was missed: `definition_for`
  searches it beside `<repo>/.claude/agents` in one `vec!`, `.claude/agents/` reached the first,
  and `opencode/agents/` is anchored so `.opencode` is not it. A definition that is not found is
  `Ok(None)`, which `declared_policy` reads as *every tool allowed* — so the file that writes an
  agent its own tool allowlist rode the renewal window, while two documents said every definition
  root was watched and a fixture listed that root among the ordinary paths. All five roots are
  crossed now, on both roads.

  `harness::roles::definition_for` also stopped spelling OpenCode's definition root by hand: it now
  searches the default root **and** the one `XDG_CONFIG_HOME` names, through the same rule `setup`
  uses. Searching only one meant a definition it did not find — which `declared_policy` reads as
  *every tool allowed*.

  The declared population was **restated** rather than stretched: the gate reads nothing from an
  instruction file, so it covers what an agent is *told* as well as what the binary *enforces*. The
  cost is recorded with it — a `Boundary` never rides the renewal window, so it is a live tracker read
  on every write — counted, 30 tracker calls out of 30 against 0 out of 30 for a routine one — and a
  refusal rather than a delay with no network. Three independent measurements of what that read costs
  disagree by a factor of two (0.61-1.22 s, 0.58-0.92 s, 0.85-3.20 s), so `docs/honesty.md` states it
  as a round trip between half a second and three seconds rather than quoting the first sample as a
  bound.
- A test can no longer report pass without running. Sixteen tests in `tests/pipe.rs` drive the real
  binary against a stand-in `gh`, and that stand-in is a cargo example; the rig answered `Option` and
  every caller opened with `let Some(rig) = … else { return; }`, so wherever the example was missing
  all sixteen returned early **reporting pass**. Measured at the base commit with the fixture moved
  aside: `cargo test --test pipe` answered *106 passed*. The skip is now gone from the type
  rather than from the callers — the rig raises, naming the command that fixes it, so there is no
  value it can return that means *did not run*, and no later caller can reintroduce the early return
  by copying its neighbours, which is how sixteen came to have it. The fixture is located from the
  running test binary rather than from a hardcoded `target/debug`, so it is found under whatever
  profile and target triple built it. Which invocations were exposed: a bare `cargo test` builds
  examples and always did, so CI was not blind; a **filtered** run — `--test`, `--lib`, any target
  selection — does not, and that is how every mutation measurement in this repository is taken.
- A refusal can now say that it already wrote. The outcome an agent is told was derived from the exit
  code alone, so every stop reported *nothing was written* — including `publish_review` refusing a
  closing keyword after it had pushed the branch and opened the pull request, which left both
  orphaned for a run that believed the message. `MutationOutcome::Committed` already carried the
  right words and the transport had no way to reach it; it does now, and a refusal that does not
  claim to have written behaves exactly as before. The same operation also scans for a closing
  keyword *before its first remote mutation* — before listing pull requests, before a reused one is
  drafted and its body replaced, and before the push — so that refusal leaves the remote exactly as
  it found it. Reads precede it, the claim renewal and the base fetch, and no write does. That scan is one function now rather than two that disagreed about what a `git log`
  which did not answer means; it refuses, on both sides, because a source nobody read is not a source
  with no keyword in it.
- Review handoff is now a durable, receipt-bound compound operation. It records and reads back the
  latest publication receipt and exact ownership epoch before idempotently unassigning, keeps the
  issue in `review`, and excludes the publishing/requesting run from selection and reclaim until a
  distinct reviewer's immutable exact-receipt verdict. Rejection returns the work for repair. Timed
  requests record one deadline without scheduling or treating expiry as review.
- Delivery now requires one accepted `review_verdict` over the exact latest receipt, crediting a
  reviewer that is neither the publisher nor any run that asked for review. Both routes record it:
  after a handoff the reviewing run names itself, and a run that acquired a reviewer without
  releasing the claim names that reviewer, marked `self_attested`. The requirement is unconditional
  precisely so that no deletion can manufacture evidence that was never recorded; `docs/honesty.md`
  names the narrower cases where a deletion restores evidence a later marker disqualified.
- Review publication now forms a cooperative draft/ready CI barrier. `publish_review` drafts and
  confirms reused PRs before push, creates new PRs draft, and records a fresh epoch over
  PR/head/base/clean-target digest, invalidating old evidence even for identical bytes.
  `release_ci` re-verifies the live review claim, latest full receipt, current draft PR and
  coherent re-derived clean target before marking the PR ready and confirming every ambiguous outcome
  by readback. GitHub cannot condition ready atomically; out-of-band collaborators and repository
  workflows can bypass the order, so no malicious-collaborator authenticity is claimed.
  Topic pushes and PR open/synchronize events no longer start CI; default-branch pushes
  remain, PR CI starts on `ready_for_review`, and — since the publication lane above —
  `publish_review` dispatches one run of the workflow per publication epoch.
- `estigia hook <event>` gates repository writes. Once a run has claimed an
  issue, every write is measured against that claim and every irreversible
  boundary re-reads the tracker timeline. A run that has sworn nothing is not
  under Estigia's authority — the oath binds once sworn.
- The tool-call gate reaches **every agent Estigia knows** — Claude Code,
  Codex, Gemini CLI, Qwen, Cursor and OpenCode — in three dialects and three
  file shapes, each from that agent's published reference or its source. None of them spells a decision the same way, and a gate answered in
  the wrong dialect runs, decides correctly and is ignored. The OpenCode plugin
  declares its own known hole — it does not see subagent calls — in the file it
  installs. For the three agents with no gate, `estigia status` says why and
  what would close it — "gate off" alone leaves an operator unable to tell a
  declined gate from an unavailable one.
- `estigia guard` installs a `pre-push` hook: the one gate no agent can route
  around, because it sits under git rather than under an agent. It refuses a
  push that no live claim authorises, whoever typed it, and it refuses to
  replace a `pre-push` somebody else wrote.
- **The tracker's timeline is the only source of truth.** No local database of
  claims, so two runs on two machines still adjudicate against each other and a
  run that dies leaves no phantom state.
- `estigia doctor` checks, read-only, that everything a run needs before it
  swears actually works: the skill, the transport, a Python that runs, an
  authenticated `gh`, and a git remote. Every failure names a resolution.
- `start_branch` now **advances the caller's own base branch** to the tip it
  just fetched. Every new branch already started from `origin/<base>`, which the
  preflight refreshes and reads back — so worktrees were never stale. What was
  stale is `refs/heads/<base>` itself, leaving `git log main` in the primary
  checkout describing a base no worktree had been cut from for days. It refuses
  every state it cannot prove safe, and each refusal is a different way the
  obvious `git pull` here goes wrong: a **diverged** base would discard commits
  the remote has never seen, an **unestablished ancestry** is not clearance,
  moving a ref under a **dirty checked-out** tree leaves the index describing a
  commit the files do not match, and a base **checked out in another worktree**
  is the same corruption in the tree this run cannot see — `git update-ref` does
  not refuse that one, measured at exit 0, after which the untouched checkout
  reports `D <file>` for a file nobody deleted. The outcome — advanced, current, or the hold
  reason — is reported as `base_advance`, and it can never fail the run: the
  worktree's start point does not depend on it, so trading a real branch for a
  cosmetic ref update would be the wrong exchange.

### Setup

- Added `scripts/build-install.ps1` for a complete Windows source reinstall. It installs Rust 1.97 when
  needed, compiles and force-installs the locked release build, runs all-agent setup with explicit
  source-build consent, and verifies the result with lifecycle status, installation status, and
  doctor.
- Added the first secure binary lifecycle slice. `estigia update` reports read-only text or global
  `--json` inventory for bytes observed by reopening the `current_exe` pathname, installer-recorded or source/unrecorded
  provenance, relation to recorded machine high-water, and an honest unavailable public-release
  result without making a network request. Create-once records under `~/.estigia/lifecycle` bind the
  path-observation lookup key to the exact compiled package version and typed/count-framed embedded-asset
  digest; publishers derive those values, record reads use one inspected handle, reject final
  symlinks/reparse points, non-regular files and unknown fields, and malformed evidence fails closed.
  Publication uses unique create-new same-directory stages, sync, and create-once hard links.
  The corrected framing/field model is schema 3; schema-2 records fail closed as unsupported.
  These records are not authenticated against a malicious same-user writer, and preflight is not
  atomic with later setup/sync mutation. Source builds require
  per-command `--allow-source-build`, recorded downgrades remain blocked with it, and dry-run bypasses
  lifecycle inspection because it deploys no assets. The pathname digest is raceable inventory, not
  mapped-code or exact-executing-byte authentication.
- Official release installers now extract the verified candidate, invoke its hidden argument-free
  `__record-install` command, explicitly require success, and only then replace the destination.
  Candidate-derived orchestration reads high-water before publication, refuses typed downgrade,
  hashes its own pathname bytes, and publishes immutable provenance before release. Exact replay is
  idempotent; malformed/non-regular state and provenance conflicts fail closed without advancing
  release history. Attestation remains conditional. Publication then copy is not atomic, concurrent
  installers remain unserialized, and no public Release, remote update, self-upgrade, or rollback exists.

- `estigia setup` installs the skill, a short directive in the agent's
  always-loaded instruction file, the lifecycle hooks, and the MCP server
  registration for eleven adapters. `setup --all` continues after one adapter
  refuses, reports any actions already proved for that adapter, and still
  configures the unaffected adapters. One unconfirmed adapter keeps the batch
  `Unknown` even when later adapters complete.
- **Uninstall is the exact inverse.** It removes what Estigia wrote and nothing
  else, and a file that held nothing but Estigia's block does not survive as an
  empty husk. JSON keeps the operator's key order; markdown comes back byte for
  byte.
- Nineteen typed settings. Reading the table produces a valid configuration or a
  refusal that names what may be written instead.
- **The installed skill directory is `flow`.** It was `issue-flow`, upstream's
  name, kept so the two could be the same directory. Estigia now installs under
  its own, and the roles it answers to are `flow triage` and `flow dev` — the
  long forms `analyst` and `developer` still work. The gate's control surface
  derives that directory from `skill::DIRECTORY` instead of naming it a second
  time, which is what stopped the rename from leaving the contract writable on a
  routine answer.
- **Invoking `flow` with no role routes itself** from tracker state: this run's
  held issue resumes as dev, else an unassigned `review` then `ready`, else
  triage. A routing read that *fails* is not an empty queue and does not fall
  through to triage — triage writes, so a failed read would become a filed issue
  justified by a queue nobody saw.
- **`sdd` says the phases are *available*, not that they all run.** The screen
  said "five phases before code", flat, while `protocols/sdd.md` has always
  engaged them per change on ambiguity and skipped what an already specified
  issue answered. An operator reading the row was promised four artifacts on a
  one-line fix, which is the reading that makes a method feel like theatre and
  gets it switched off. The behaviour did not change; the sentences describing
  it did, in the screen and in the protocol. `auto` and `per-issue` are accepted
  as spellings of `sdd`, because somebody reaching for that word is asking for
  what `sdd` already does.
- **Direct work now has bounded delegation rules in the always-loaded
  contract.** `SKILL.md` owns the read and write thresholds, smallest-topology
  rule, single-writer invariant, and per-action worker exception. Crossing a
  threshold delegates direct work only: it never changes the configured
  `Planning` mode, creates SDD state or artifacts, or invokes an SDD phase. The
  harness does not claim to dispatch those workers; a scoped semantic test holds
  the contract instead.
- **Planning now owns an inline, agent-specific model section in the TUI.** The
  persisted `Model routing` row remains one `key=model` cell for config and CLI
  compatibility, but it is no longer a visible setting or two-stage target/raw
  picker. `Planning` is the last primary row. Beneath it, exact model rows are
  derived from `orchestrate`, the active planning phases, universal `apply`,
  delegated roles and external sub-agent names; inactive phases and workflow
  states stay persisted but hidden. Each row opens advisory models for the concrete host:
  curated lists for Claude Code and Codex, and a lazy `opencode models` read
  without `--refresh` for OpenCode. That read closes stdin and returns the TUI
  within 5.6 seconds through one deadline covering PATH resolution, validation,
  spawn, execution, cleanup and handoff; stdout above 1 MiB, outside strict UTF-8
  or without a valid ID is unavailable rather than parsed. Unix process groups
  and Windows Job Objects clean descendants after successful direct completion
  as well as failures, while Windows script launchers are refused. Other,
  neutral, uniform and shared answers invent no catalog; custom IDs and
  target-local `inherit` remain available. Dynamic suggestions and custom IDs may
  not contain the comma, pipe or line break that would end one `key=model` entry.
  One-target updates preserve the rest of each destination's cell. Assignment,
  inherit and target restore share one destination rule: the current agent in
  non-uniform mode, or only the currently selected agents in uniform mode.
  Restore returns each selected target to that destination's own installed
  assignment. Uniform display aggregates per target, and suppresses planning
  phases when selected agents disagree on Planning. All destinations validate
  before one atomic commit, model selection never changes `Planning`, and Enter
  and Space both choose while editor Space remains literal.
  Claude Code and Codex now add a synthetic profile row with independently
  reviewed `balanced`, `performance` and `economy` routes. A preset replaces the
  complete route; an exact mismatch is `custom`, and choosing `custom` preserves
  the target assignments edited below it. Profiles compile into the existing
  `key=model` cell and never change Planning. Dynamic OpenCode, shared/uniform
  views and adapters without stable catalogs invent no presets.
  Uniform `Planning` now uses the same selected-agent destination set rather
  than mutating every loaded table. `ConfigLayers` now preserves contract-only,
  per-agent, local and repository snapshots plus each override document's
  explicit row ownership. Agent documents parse and own only agent-scoped rows
  and repository documents only repository-scoped rows, so forbidden hand-edited
  rows are ignored before validation and cannot reject or pin lower-layer values;
  invalid owned rows still refuse. Each shared skill root renders one portable contract;
  selected agent rows land only in that adapter's override, while private roots
  keep them in their contract without a duplicate. Local, repository and
  unselected peer values are never promoted into shared `SKILL.md`; host phase
  definitions are instead materialized from the effective view and retracted
  with it. Repository saves preserve existing owned rows and union only rows
  changed in the session. Direct agent writers use one fail-closed document snapshot
  for ownership and output. Direct repository writers likewise treat only `NotFound`
  as creation; unreadable or invalid UTF-8 input remains byte-identical on both paths.
  Receipts carry every layer plus setting-level effective
  read-back, exact repository-owned settings and explicit lifecycle completion.
  One repository snapshot binds values and ownership across all loaded baselines;
  only owned rows advance, including after a later post-write failure. A proved
  repository document missing or unreadable at read-back is `Unknown`/status-required,
  with no row acknowledgement. Partial failures retain proved
  actions as `Committed`, or become `Unknown` when the attempted write itself
  cannot be confirmed, without marking an incomplete adapter installed; the TUI
  leaves unproven scopes dirty. The first `Unknown` refusal controls aggregate identity and remedy.
  `SetupFailure` names prevalidation, preview or mutation explicitly; any generic
  failure before mutation is `NotStarted` with an input/configuration remedy,
  while typed malformed JSON remains `NotEditable`. Failed real prevalidation
  carries no dry-run actions or wording. Preview failures remain `NotStarted`
  regardless of their planned actions. Planning-then-model and model-then-Planning still
  preserve both edits.
  Interactive dry-run uses the real batch's one pending action accumulator,
  including auxiliary files, so unique paths/change kinds/counts match while it
  writes and acknowledges nothing. Planned actions are never treated as mutation
  evidence; successful retry still replaces its refusal.
  Model rows, thumb and picker now share one focus-aware viewport; Tab resets
  both rows and thumb, picker opening restores row ownership, bottom targets stay
  attached at 80x24, and Space on the agent choice remains a toggle.
  Current and installed assignments share one bilingual renderer, and Help now
  has one table-backed translation whose English and Spanish pages both describe
  the inline selector and distinguish localized display labels from canonical
  persisted names, values, model IDs and printed CLI commands.

### Refusals

- Every refusal carries a stable code, what happened to the world, whether a
  replay is safe, and a resolution — either a runnable invocation checked
  against the real parser, or a reason from a closed vocabulary saying why no
  command can honestly exist.
- The transport's **69 refusal reasons** are derived from the transport itself
  at build time, so the two cannot drift.

### Guards

- Neither workflow may check out with a deprecated action or throw away a red run's cache, and both
  halves are crossed rather than trusted. `actions/checkout@v4` targets Node 20, and it was one of
  five actions that did — a warning that becomes a workflow which will not start, on GitHub's
  schedule rather than on ours. **Only the checkout is raised here**; `setup-node@v4`,
  `upload-artifact@v4`, `download-artifact@v4` and `softprops/action-gh-release@v2` are still on
  Node 20, so the runs still print the warning and the release lane — which carries **all four**,
  three of them only there — still would not start. Every `uses:` line in both files is enumerated
  with its runtime in `docs/honesty.md`, along with two more `node20` actions reached transitively
  through `attest-build-provenance`, rather than the gap being closed under an issue that asked about
  the checkout. Both files move to `v7`, which runs on `node24`. That crosses three majors and each
  carries something: `v5` requires a runner of at least `v2.327.1`, `v6` moves persisted credentials
  out of `.git/config` into a file under `$RUNNER_TEMP`, and `v7` refuses fork checkouts under
  `pull_request_target` and `workflow_run`. The `ref` and `persist-credentials` **inputs** are
  unchanged, which is not the same as their behaviour being unchanged: `ci.yml` opts out of
  persistence and `release.yml` does not, so the release lane takes `v6`'s new location — it runs no
  authenticated git command and no container job, so nothing there reads it. The lanes run on hosted
  runners, and this repository triggers on neither event `v7` blocks. `Swatinem/rust-cache` discards a failing run's cache by default, so the fix pushed after a
  red build recompiled the dependency tree from cold on all three platforms — closing one set of
  platform failures paid it at least nine times, `ci` on `main` having failed nine consecutive times
  before it went green. The issue that raised this said six and named a first run that no longer
  exists; nine is what the run history still holds. The guard names a version floor rather than the
  word *deprecated*, because a file cannot be asked whether it is: `v2` to `v4` run on Node 12, 16
  and 20, `v1` is a runner plugin with no Node runtime at all, and everything from `v5` is `node24`.
  A future deprecation moves that number to somewhere a person has to look at it.
- The shipped payload passes **upstream's own 328 checks**.
- Population declarations are bound to their syntax node and fingerprinted:
  changing the rule or the code beneath it reopens the claim.
- Seam guards hold the contract, the bindings, the transport and the tool table
  against each other in both directions.

- **Model rows now say exactly what the active protocol can route.** Direct has
  no planning phases, full SDD exposes five and lite exposes spec/tasks. Hidden
  assignments remain readable and CLI-editable, so changing Planning never makes
  an existing table invalid. A uniform Planning disagreement exposes only fixed
  rows instead of presenting one selected agent's phases as shared truth.

- **Claude Code's SDD planning phases ship as sub-agent definitions, and their
  tool lists are enforced.** `setup` writes `sdd-explore`, `sdd-propose`,
  `sdd-spec`, `sdd-design` and `sdd-tasks` into `~/.claude/agents/`, the verified definition
  root `harness::roles::definition_for` searches. OpenCode and other hosts receive
  routing declarations only, because a definition written where the gate cannot
  verify its format and enforcement would overclaim what the configuration does.
  Other orchestration harnesses ship these by the handful; each
  declares a list and relies on the host to honour it. Estigia declares the same
  kind of list and then refuses the call that leaves it.
  - Only the phases the protocol runs: none under `direct`, spec and tasks under
    `sdd lite`.
  - `Model routing`'s phase key becomes the `model:` line, which is the first
    time that setting does anything — it was, in its own words, a declaration
    the agent reads and no dispatch this binary performs.
  - `Planning`'s `openspec` axis decides the tool list: with the artifacts on the
    issue no planning phase may write at all; under `openspec/` the three that
    leave an artifact behind get `Write` and `Edit`, and the two that only think
    do not.
  - The enforced half is the half the `PreToolUse` matcher wakes — `Write`,
    `Edit`, `MultiEdit`, `NotebookEdit`, `Update`, `Bash`. A `Read` or a
    `WebFetch` never reaches the gate, and each shipped definition says so in its
    own body rather than leaving an operator to assume otherwise.

- **A row an agent's own file answers is no longer reported as in force.**
  `config set` read its own write back from the shared table, and what a run
  actually reads is that table with the agent's `estigia.<slug>.md` laid over
  it — nine adapters share the neutral root, so a row answered there is the row
  that arrives. Measured on a real machine: the command answered *Planning is
  now direct*, all three shared tables said `direct`, and `config list` one
  command later said `sdd lite`. It now checks each adapter under that root and
  refuses with `setting-shadowed-by-agent-file`, naming who still reads
  something else and the command that clears it.
- **`config set --repo` and `config set --agent` stopped writing into the
  machine's own profile.** Both resolved the real home whatever they were
  handed: `writable_config`'s per-agent branch built its own `SetupOptions`, and
  `repository_set` called `paths::home_dir()` unconditionally. So the test suite
  recorded into the developer's `~/.estigia/repositories` — 896 entries when it
  was read, every one of them a temporary directory from a test run — and a test
  driving `config set --agent` through a sandbox wrote outside it. Measured
  after: a full `cargo test` now leaves that file's line count unchanged.

### Removed

- **The differential suite and its corpus.** `tests/differential.rs` — a hundred
  and eleven crossings — and `tests/transport/oracle.json` — 228 answers recorded
  off the retired Python — are deleted. A crossing's key was a fingerprint of its
  whole question, and the question carried the installed layout, so the corpus
  froze the skill directory's name; renaming it asked one crossing something the
  corpus had never been asked, and extending a corpus means running a reference
  implementation this repository no longer has and does not want back. Two
  `tests/honesty.rs` guards that policed the corpus went with it. What this cost
  is written in `docs/honesty.md`: the
  port's agreement with a second implementation is no longer asserted anywhere.

### Known gaps

Written down rather than discovered — see *What this instrument does not
measure* in the README. The gate is installed for one agent, the MCP server has
never met a real client, nothing has been tested against a live tracker, and the
two-minute renewal window is a cadence and therefore a gap.

