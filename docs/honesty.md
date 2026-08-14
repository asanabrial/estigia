# What this instrument does not measure

An axis list that names what is not covered is the difference between "zero defects found" and
"zero defects found where we looked".

This is the document to read before adding a claim anywhere else in the repository: it carries what
this crate knows it does not check, and a new document that contradicts it is a document that will
be believed. It lived inside [`README.md`](../README.md) until it was 55% of that file — a reference
list a reader had to scroll past to reach the layout, which is how the most important section became
the least read.

`tests/honesty.rs` crosses the *countable* claims here against the code: the number of agents, the
number of tools, the number of doctor checks, the settings rows. A count that drifts fails the
suite. Everything else here is prose held by review.

- **Model catalogs do not measure runtime availability or capability.** The Claude Code and Codex
  lists are reviewed snapshots; they can become stale between releases. OpenCode's list is whatever
  the configured host reports through `opencode models` at that moment, without a refresh, and a
  successful line is still not proof that a later run accepts or uses the ID. Estigia does not
  validate catalog membership, execute models, or inspect or filter their tool-call capability.
  Only Claude Code currently receives planned phase definitions; OpenCode and every other host keep
  these values as routing declarations. `orchestrate`, `apply`, and a visible route are likewise not
  proof that a host executes them.
  OpenCode process-tree cleanup is likewise not containment proof: the controller bounds how long the
  TUI waits, while process-group and Job Object cleanup remain best-effort OS operations.

- **The transport is ported, and the Python is not shipped.** issue-flow's `gh` and `git` calls used
  to run as a Python script this crate installed, so every machine carried a second implementation of
  every decision the gate makes — running beside the first and able to disagree with it. It did:
  about which comments name a claim, about when a takeover's evidence is bound, about whether a
  branch is linked to its issue. Each of those was found by putting one input through both.
  The operations are answered in this process now; the script is in `RETIRED`, so a `sync` takes it
  off a machine that has it — and the directory it lived in goes with it — and nothing on `doctor`'s
  list asks for an interpreter.

  **The Python is gone from the tree, and so is the recording of it.**
  `skill/scripts/github.py` and the 2.364-line suite its authors wrote for it were deleted when this
  repository stopped depending on an interpreter. The hundred and eleven crossings that replayed
  against `tests/transport/oracle.json` — 228 recorded answers — went with them, in the change that
  renamed the installed skill directory to `flow`. Both numbers are now facts about deleted files.

  **Why the recording could not be kept.** A crossing's key was a fingerprint of its *whole
  question*, and the question carried the installed layout: the skill directory's name in every
  fixture path, and the directive block that names it. So the corpus did not only record the
  transport's answers — it froze the shape of the installation they were recorded on. Renaming that
  directory asked one crossing something the corpus had never been asked, and the only way to extend
  a corpus is to run the reference implementation, which no longer exists and is not coming back.
  Measured before deciding: with the directory renamed and nothing else touched, one crossing of a
  hundred and eleven answered `no recorded answer`; with the contract's *prose* rewritten and the
  directory left alone, all hundred and eleven passed. The corpus never froze the contract. It froze
  the directory's name.

  **What that cost, plainly.** The port's agreement with the retired implementation is no longer
  asserted anywhere. Every defect those crossings found is still fixed, and most are still held by a
  unit test written beside the fix — the entries below name them one at a time. What is gone is the
  *mechanism* that found them: putting one input through two implementations and requiring one
  answer. There is one implementation now, and nothing left that can disagree with it. A regression
  in the port is a regression nothing here will notice unless a unit test happens to name it.

  Two guards in `tests/honesty.rs` went with the corpus, because both policed it and neither had
  anything left to measure once it was gone: `the_oracles_corpora_still_reach_past_the_easy_half`,
  which read the crossings' alphabets to check they still reached the characters where two runtimes
  stop agreeing, and `no_test_decides_for_itself_what_an_undescribed_comment_means`, which counted
  how many crossings decoded a comment through the port's own reader.

  **Nothing here has been tested against a live tracker**: no check in this suite reaches the
  network.
- **Estigia cannot prove reviewers or blind judges ran.** `publish_review` mechanically freezes a
  coherent clean draft receipt over epoch, PR, head, base and manifest digest. `handoff_review`
  records that exact receipt before releasing one ownership epoch, and `review_verdict` records an
  immutable outcome crediting a reviewer that is not the publishing run. Either outcome resolves the
  handoff so the publisher can resume; rejection permits repair but never delivery. `release_ci`
  checks the globally latest receipt, that accepted marker, the current draft PR and a re-derived
  clean target before marking ready.

  **What that is worth differs by route, and the marker says which.** After a handoff the reviewing
  run holds the claim and records its own verdict, so the tracker timeline attributes it to a run
  that really did acquire the issue. A run that acquires a reviewer without releasing the claim
  records that reviewer's outcome itself; the marker's `run-id` and `reviewer` then differ, and the
  answer says `self_attested`. In that case the distinctness check is comparing a name the recording
  run chose — it establishes that the run declared somebody else reviewed, not that anybody did.
  Estigia asks for the declaration because an unstated review is one nobody can audit, not because
  it can check it.

  None of it proves an independent context existed, that one or two judges read those bytes, that
  two judges were blind to each other, or that their verdicts were honest. A marker can still be
  forged by a collaborator acting outside Estigia. `single` and `two blind` remain operator-selected
  review contracts, not observations the harness can make.
- **One review-protocol readback is held by no test, and one is held only by a string search.** The
  review protocol's guards were mutated one at a time — each disabled, the whole suite run, the tree
  restored from a byte copy. Held by a test that goes red: all three enforcement points of the
  requester exclusion (`claim`, `reclaim`, the review queue), the queue's fail-closed candidate read,
  the verdict's distinctness rule on *both* its halves, its live-claim requirement, the receipt's
  exactness on each write path **before** it writes, the CI-release gate, the comment escaping, the
  read-side requester filter, and every field-shape validator on the three markers — receipt widths,
  the handoff's authority, target, timestamps and its blocker/discharger, the verdict's two
  identities and its outcome vocabulary.

  That sentence used to say "on the write paths" without the qualifier, and it was false: neutering
  both of `handoff_review`'s receipt checks left the suite green, and a handoff recorded against a
  superseded receipt excludes nobody while its ownership epoch has already gone — the publishing run
  eligible again for the item it may not review, which is this repository's own livelock with an
  audit trail. It is held now. The *second* copy, the one guarding a retry, is a different matter and
  is named below.

  **Many of the smaller ones are not, and this entry does not claim to have found them all.**
  `handoff_review`'s post-release readback, which proves the released ownership epoch is no longer
  authoritative, can be disabled with the suite entirely green. Its retry-path receipt recheck is
  caught only by `the_compound_handoff_records_before_release_and_checks_review_afterwards`, which
  searches the source text for the call: that catches deletion, and would not catch the call being
  made and its answer ignored. Beyond those, an adversarial sweep of forty-nine mutations found a
  further score of defensive checks — replay-path repeats of rules whose first-write copy *is* held,
  operation-id and marker-field validators, idempotency and visibility readbacks — that can each be
  removed with the suite green. They are one class: belt to a brace that a test does hold, which is
  why the load-bearing list above is the one this file stands behind.

  Two earlier versions of this entry were wrong, in both directions, and how they were wrong is the
  useful part. The first named five checks as unmeasured on somebody else's report rather than a
  measurement run here — one of the five was in fact held, and there were more than five. The second
  said only two were unheld, which understated it by roughly a factor of ten. A count nobody counted
  is exactly what this document exists to refuse, and it refused itself twice before this sentence
  was written.

  One caution for whoever mutates next: this crate has tests that anchor on exact whitespace in
  `claim.rs`, so a mutation harness that rewrites line endings — Python's text mode on Windows will,
  silently — produces failures that look like a guard being caught and are not. Two of the
  measurements behind this entry had to be discarded and rerun for that reason.
- **One refusal still says nothing was written after a write, and it is not the one this was about.**
  A stop can now declare that it already wrote, and `publish_review`'s two post-push refusals do —
  each held by a test that fails when its own marker is renamed. `ensure_draft` is not one of them,
  and it has **two** doors onto the same lie: it runs `gh pr ready --undo`, a remote write, and after
  that succeeds both its `draft-readback-failed` stop and a failed `view_pr` read report *nothing was
  written*. Measured rather than read: driving the operation with a reused ready pull request and an
  unreadable body, the reverted read reaches `ensure_draft`, the wire log carries
  `pr ready 99 --undo`, and the answer is *nothing was written*. Wrapping the `draft-readback-failed`
  condition in `if false &&` leaves the whole suite green, so neither door is held. They sit before
  the push rather than after it, which is how both fell outside the bar the issue set and outside the
  enumeration that answered it; it is the same lie in the same function, and naming it here is cheaper
  than pretending the sweep was complete. It is two doors onto a **gate** and not only onto a report:
  with `draft-readback-failed` disabled nothing stops a still-ready reused pull request exposing the
  new head to CI, which is the barrier that refusal exists to hold.

  Three more, from the same reviews and left with their measurements. The `[world-action]` guidance
  naming `Refs #<n>` is held by no test — stripping it from either refusal leaves the suite green —
  though the issue lists it under *unchanged*. `Answer::already_wrote` decides which exit-code arm
  `translate` takes and sits outside the `exit-code` population fingerprint, and that arm's
  `StatusRequired` axis is held by the fingerprint alone: a tripwire that says *go and read this*
  rather than a test of what it does. And the pull-request body is read three times — by the scan,
  by `edit_pr`, and by `pr create` — so a body edited between the scan and the write is published
  unscanned. Narrow, and one place rather than three is the fix if it ever matters.

  Two smaller things found in the same review and left: the commit-range scan is one function now, but
  its two callers still read from different checkouts — `publish_review` from the isolated one where
  the commits are, `assess_autoclose` from the repository root — and unifying that changes which tree
  a standalone `check-closing-keywords` inspects, which is a decision rather than a tidy-up. And
  `keyword_sources` carries two shapes under one key, a list of strings from the precondition and a
  list of `{where, text}` from `assess_autoclose`. Nothing parses it today, which is the only reason
  that is a note and not a defect.

- **A deleted comment is missing evidence, never satisfied evidence.** The verdict requirement does
  not appear only once a handoff exists — if it did, deleting the handoff comment would lower the
  bar from *a distinct reviewer accepted these bytes* to *nothing*, and an erased record would read
  as clearance. Delivery asks for the same accepted verdict on both routes, so no deletion can
  manufacture evidence that was never recorded.

  It can still **restore** evidence that a later marker had disqualified, and that is a narrower
  claim than "a deletion can only refuse". Two cases, both reachable only with a `gh` call outside
  Estigia, which exposes no operation that edits or deletes a comment. Deleting a handoff comment
  lifts the requester exclusion it carries, returning the publishing run to the queue for work no
  verdict covers — liveness, not integrity. And where a run recorded a verdict and *then* requested
  a review handoff for the same receipt, that request disqualifies its own earlier verdict;
  deleting the handoff makes the verdict qualify again, turning a refused `release_ci` into a
  cleared one. The bytes it clears were still reviewed and still accepted by a run that was not the
  publisher, so this is a lost later objection rather than an unreviewed merge — but it is a
  deletion that clears, and saying otherwise would be the sentence, not the code, doing the work.
- **The draft/ready CI barrier is cooperative, not adversarial.** Compatible repositories start PR
  CI on `ready_for_review`, not topic push/open/synchronize/reopen. GitHub has no atomic
  conditional-ready operation. Collaborators or repository workflows acting outside Estigia can mark
  ready, push, trigger other CI or forge comments; Estigia neither parses arbitrary consumer YAML with
  weak substring checks nor claims malicious-collaborator authenticity.
- **The two readers of the configuration table are not identical, and one half is now closed.**
  Both read the same block of the same file, and they resolve a row differently. The binding looks a
  setting up by **prefix** — `cfg` returns the first key that `startswith` the label — and this crate
  compared whole words, so `| Project board (mine) | acme/7 |` was a row the transport acted on and
  Estigia reported as unset. Measured on the installed pair: `board.enabled=True owner=acme number=7`
  there, `Project board  none` here. That half is closed —
  `a_row_the_binding_acts_on_is_a_row_this_reader_finds` holds it — by trying the prefix **after** the
  exact match, so an exact spelling never loses to a looser one.
  **The other half is closed too, and by neither of the two ways this entry said it would take.**
  It said closing it meant resurrecting the silent drop, or reporting the row as one nobody reads —
  and reporting it is what `doctor` did, on a **BROKEN** row, for several rounds. The third way is
  that the transport stopped reading the operator's cells at all: `Context::live` reads *this
  crate's own rendering* of the layered configuration, so `| Project  board |` with a doubled space
  and `` | `Project board` | `` with backticks both reach it as the value this crate read out of
  them. The `doctor` row went with the gap, and
  `a_label_this_crate_forgives_is_one_the_transport_reads_too` stands where it was.

  What is left is not a difference between two readers: `get` answers with the **first** row whose
  label the question is a prefix of, so `| Project board (mine) |` still answers `project board`.
  That is written on `Context::get` and is the same surprise on both sides.
- **An uninstall takes an empty `hooks` wrapper it may not have written.** When Estigia's entries
  leave a settings file, the now-empty `hooks` object goes with them, on the reading that Estigia put
  it there — which is right whenever it did. An operator who wrote `"hooks": {}` themselves loses it,
  and it is measured rather than argued: `estigia setup cursor --uninstall`, on a machine where
  nothing was ever installed, rewrites that file and reports `update`. Telling the two apart needs a
  record of what this crate created; one exists for the instruction file (`created_outside`) and not
  for this. `an_uninstall_leaves_no_empty_husk_of_its_own_making` asserts the half that is decidable
  and says in its own body why it stops there.
- **The oracle covered 125 of the transport's 131 reachable functions when it was retired.**
  That number is now a fact about two deleted files — the reference implementation and the recording
  of it — and it is kept because it is the high-water mark of what was ever crossed. Six functions
  were never crossed and never will be: `read_ownership` and `read_lock_record`,
  which needed a real checkout on disk rather than a scripted `git`; `reclaim_landed`, the read-back
  inside `cmd_reclaim` that every reclaim crossing stopped before; and three legacy command bodies
  kept for older markers. The port's behaviour in those six places has never been crossed against a
  second implementation and cannot be now.

  **The numerator was measured with a tracer that loses lines.** Four traced runs of the same suite
  disagreed — 124 and 125, and one of them dropped `cmd_config`, which every run of `config`
  executes — because the crossings ran in parallel and each recorded into one file. The number above
  is the union of those runs, which is the honest reading of a counter that can miss and cannot
  invent.

  **What replaced it can fail, and that was measured.** Corrupting one recorded answer turns its
  crossing red; the corpus is load-bearing rather than decorative. What it cannot do is answer a
  question it was never asked: a fixture that changes fails with *no recorded answer for this
  crossing* rather than quietly agreeing, and re-blessing one from the port itself is possible,
  named in that message, and costs the case its independence.

  **One crossing diverges on purpose, and says so where it is asserted.** The corpus records an
  implementation that is gone, so the port improving on it is a disagreement by design — and the
  crossing is right to notice. `audit_board` names the board it could not conclude about, where the
  reference said only *"the board returned zero cards"* beside an action asking the operator to
  confirm the owner, the scope and the number. Two fields are declared there, with the reason, and
  every other field of the envelope is still compared.

  The refusal for a case with no recorded answer used to name an `ESTIGIA_ORACLE_BLESS` that was
  **never implemented** — a message promising a way out that does not exist, which is worse than
  promising none. It now says the only thing that can extend the corpus: bring the reference back,
  re-record, delete it again.

  **The question was wider than the answer, which made ordinary editing expensive.** A crossing's
  fingerprint took every file of a fixture's payload, so rewording a sentence in
  `bindings/github.md` invalidated a recorded answer about the *configuration block* — and the only
  remedy on offer was the blessing above. It now takes the three files the transport opens out of a
  payload, at any depth: the contract and the two spellings of the operator's own overrides. The
  corpus was re-recorded against the reference, which had to be brought back to do it and was
  deleted again after — and narrowed once more when a sentence added to the contract's step 6
  invalidated a crossing about the table: of a payload document it now takes the **configuration
  block**, which is the part `Context::live` parses.
- **Forty-seven refusals the transport can give are named in no prose an agent reads.**
  `every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer` accepted a reason
  spelled anywhere in `github.py` as documented — and the binding's source is not something an agent
  reads. Deleting that file did not create these; it stopped concealing them. `holder-not-stale`,
  `lost-claim-race`, `publication-readback-disagrees` and forty-four more reach a run as a word to
  branch on, with the instructions that go with them living only in the `action` field of the
  envelope itself.

  The list is frozen in `UNDOCUMENTED_REFUSALS` and the check is a ratchet: a **new** refusal with no
  prose is red on the review that adds one, an entry that stops being undocumented has to be taken
  off, and the list can only get shorter. Closing it means writing the instructions into
  `skill/references/`, and that is not written yet.
- **The operator's local file used to reach the gate and not the transport, and it now reaches both.**
  Kept here because how it was closed is the part worth reading. `load_config` opened
  `SKILL_DIR / "operator.local.md"` and nothing else, while Estigia had renamed that file to
  `estigia.local.md` and said so in the contract it writes; and `_parse_config_block` answered `{}`
  for any document it could not find a marker pair in, while `settings::rows` falls back to the whole
  document. Both are closed in the vendored copy, and **neither is carried upstream yet**.

  Which side moved was the decision. Teaching the gate to read nothing would have agreed just as
  well and would have **loosened** it — no declared boundaries, the widest renewal window, every
  narrowing the operator typed silently dropped. Moving the transport can only let it see rows the
  gate is already enforcing, so that is the direction both fixes went.

  Closing the second half found a third thing nobody had measured: a document whose closing marker
  sits **above** its opening one — which a truncated rewrite produces — satisfied
  `start in text and end in text`, so the transport believed it had found a block and read the empty
  tail of the file. There is no fallback from a block you think you have. The gate answered `linear`
  on that document and the transport answered nothing.
  `a_document_with_no_usable_block_is_read_the_same_by_both` now puts six shapes of unmarked and
  half-written document to both readers, with a floor that fails if the transport reads nothing
  anywhere — because agreement about nothing is what the defect looked like.

  What stands is the risk the fallback inherits: an unmarked document is read whole, so a prose table
  in it is offered as configuration, and nothing comes of that only because no prose row's first cell
  spells a setting. The reader's own comment calls that *luck, not a property*, and
  `no_prose_table_in_the_contract_spells_a_setting` holds the shipped contract to it — measured with
  its closing marker removed, 8 configuration rows become 25, the extra 17 being prose
  (`Situation | Action`, `Load | When`), and the resulting configuration is still identical.
- **Skill directories are verified for three agents.** `claude-code`, `codex` and the neutral
  `~/.agents` root are known. The other four adapters have a verified *instruction* file and an
  unverified skill directory, so their skill lands in the shared root and the directive names the
  path. `estigia setup` says so on the line where it matters.
- **No public Release exists, and `update` does not query a remote in this slice.** Six build targets
  have to be produced before `install.sh` has anything to download. `estigia update` can report only
  local recorded provenance and release high-water; it explicitly reports the latest public release
  as unavailable and does not claim currency from absence. The JSON is not authenticated against a
  malicious same-user writer. Official release installers publish candidate-derived local records
  before replacement, but publication and copy are not atomic and concurrent installers remain
  unserialized. There is no remote update, self-upgrade, rollback, or mutating public upgrade command.
- **The seam tests check the payload against itself**, not against an agent that read it. They
  prove the contract does not link to a file that is missing; they do not prove an agent obeys it.
- **The harness gates named populations, not every write.** The editing tools each agent ships, the
  git spellings that change the tree, the irreversible steps, and — since 2026-08-01 — the shell
  constructs that visibly write a file: a redirect, a `sed -i`, an interpreter handed code, and the
  utilities that copy, move or delete — including under a prefix that runs them, since `sudo rm -rf src`
  hides nothing and was missed for no better reason than its first word. That list of prefixes had
  been filled to thirteen and not to its own boundary: measured on 2026-08-06, `flock … rm -rf src`,
  `taskset`, `parallel`, `runuser`, `chroot`, `strace`, `ltrace`, `watch`, `systemd-run`,
  `eatmydata`, `unbuffer`, `xvfb-run`, `proxychains`, `torify` and `setarch` each classified as
  nothing at all while `timeout 5 rm` beside them classified as a write — the same shape and the
  same visibility. All fifteen are matched now. What is still not matched is a
  write that shows nothing: an `eval`, a wrapper *script*, a merge reached by an alias or `gh api`.
  Since 2026-08-06 a **long write flag** counts whatever the command is — `--write`, `--in-place`,
  `--fix` — because the boundary is what is visible on the line and those are as visible as a
  redirect; they had been read only for the few commands that take `-i`, so `prettier --write src`
  and `eslint --fix .` rewrote every file in a tree and classified as nothing at all. What is
  **still** not matched is a formatter whose default is to rewrite: `cargo fmt`, `black .` and
  `gofmt -w` show no long flag, and the last shows a short one that means *whole word* to `grep`.
  Reading short flags everywhere would report a search as a write, so that gap is left open and
  written down rather than closed by guessing.
  Since 2026-08-04 the gate also measures **its own disarmament**: `estigia stand-down`,
  `guard --uninstall` and `uninstall` classified as nothing at all, so the one thing the whole model
  rests on — that an agent cannot switch the gate off — was reachable through the tool it uses most.
  It also measures writes to the files those decisions are read from — its own state, the
  contract, and the entry in each agent's settings — because closing the road through the shell
  left the shorter one open. Eleven `guard:population` comments in the source name each legitimate
  population and what falls
  outside it, and that number is checked here.
- **A delivery is refused in a state that holds no verdict.** Holding an issue and being allowed to
  land it were one question, and only the first was ever put: a run in `in-progress` that ran
  `gh pr merge` was told yes, because the tracker confirmed the claim and nobody asked what the
  claim was *for*. `git merge`, `git tag`, `gh pr merge` and the two `gh release` steps now need the
  issue to be in `review` or `done`. `git push` and `gh pr create` are deliberately exempt —
  publishing is how a run reaches review, and gating it on review is a deadlock. Boundaries the
  operator declared are never read as deliveries: Estigia cannot know whether a `make deploy`
  delivers or rehearses, and guessing would refuse a step it never understood.
- **That narrowing is not configurable, and the asymmetry is the reason.** Every other axis here can
  be switched; this one only tightens, and a setting that could loosen a guard rail turns it into a
  preference. It is the same rule as the operator's boundary list, which adds and never removes.
- **Closing the shell hole changed a declaration that had been wrong.** `repository-shell` claimed
  "every spelling of a repository write reachable from a shell" while matching only spellings that
  name git. A redirect writes the same tree and was never in reach of that list. The claim is now
  split in two, because the two halves have two different boundaries.
- **Nothing here has driven a real agent.** The hook protocol is exercised by feeding the binary the
  payloads an agent sends and reading what it prints. That proves Estigia answers correctly; it does
  not prove Claude Code asks.
- **The tool-call gate reaches every agent Estigia knows.** Ten of them, in five dialects, each
  taken from that agent's published reference or its source: Claude Code, Codex and Continue
  (`PreToolUse` → `permissionDecision`), Gemini CLI (`BeforeTool`) and Qwen (`PreToolUse`) →
  `{"decision":"deny","reason":…}`, Cursor (`preToolUse` → `{"permission":"deny",…}`), Windsurf and
  Crush (exit 2, with stderr sent to the model), Cline (`{"review":true}`, which pauses rather than
  denies) and OpenCode (a plugin that blocks by throwing). The remaining entry is the agent-neutral
  skills root, which is not an agent and has no tool loop to gate.
- **Three of those gates fire on every tool, and only Estigia's own lists narrow them.** Cursor and
  Windsurf register no matcher, so their host wakes the hook for whatever the model called and the
  tool name arrives in the payload. **Cline** is the third and arrives a different way: its gate is
  a script Estigia writes whole, and that script pipes every `PreToolUse` payload straight through —
  no matcher, and no named list of its own. For every other agent something is crossed against the
  classifier: a settings matcher for six of them, and for OpenCode the `GATED` array its plugin
  carries. For these three there is nothing to cross: this build has not been shown which names
  those hosts send, so a tool of theirs that is missing from `WRITE_TOOLS` or `SHELL_TOOLS` reaches
  the gate, classifies as `Untouched`, and is answered "not something this run's oath covers" — a
  sentence that asserts coverage nobody checked. Closing it takes a payload captured from a real
  session of each, which is the same evidence the rest of this table is built on and the reason this
  line is here instead. Measured, so the size of it is not a guess: `git push --force` reaches the
  classifier as a boundary under `Bash` and as `Untouched` under three other plausible spellings of
  a shell tool, and an edit to `src/main.rs` the same way.

  Cline was missing from this entry until the guard that enforces it was widened to reach it. That
  guard walked the agents with a settings gate spec, which is precisely the set that excludes the
  two gated through a file Estigia owns whole — the two where the classifier's list does all the
  work. It now walks every agent whose tool calls Estigia gates by any mechanism, and a
  plugin-written gate that carries no list of its own has to be named here.
- **`ensure-states` cannot tell "already there" from "gh could not run".** `gh label create
  --force` exits non-zero for both, and the command reads that status as idempotence — which is
  right for the first and wrong for the second. Measured on both sides against a `gh` that refuses
  every call: each answers `ok: true` with all six labels listed as `ensured`, and not one of them
  exists. The workflow then proceeds to label an issue with names the tracker has never heard of.
  It is not a divergence, so the differential oracle could not have failed on it even while it
  existed. Closing it means reading
  what `gh` printed rather than only its exit code, and this crate has never been shown those exact
  words — the same reason `icacls` is named beside `neutralising-shell` and not added to it.
- **Both of those numbers used to drift unread.** The check accepted the phrase "every agent Estigia
  knows" and then stopped, so the sentence went on to say "six of them, in three dialects" while ten
  were gated in five. The counts in that sentence are now crossed against the adapter table and the
  dialect list, which is the same defect this project keeps finding: one end written by hand, the
  other in code, and only *part* of the prose crossing over.
- **A sub-agent's declared tool list is enforced only for the tools the gate wakes for, and only
  where an agent name arrives.** The role gate runs in `PreToolUse` and, since it was added there
  too, in `estigia gate` — but that second door is reached in this tree only by OpenCode's plugin,
  which sends the tool's own arguments and no agent name, and whose own documented limit is that
  `tool.execute.before` never sees a call made by a subagent at all. So the second door closes
  nothing today: it is open for a caller that names a sub-agent, and there is not one yet. Where the
  question *is* asked, it sees exactly what the matcher wakes the hook for —
  `Edit|Write|MultiEdit|NotebookEdit|Update|Bash` for Claude Code — and a tool outside that set is
  never offered to it. Measured against the list this crate itself cites as the case it exists for,
  a published `builder` sub-agent declaring `Read, Write, Edit, Glob, Grep, Bash`: three of the six are tools the
  gate can judge, and `Read`, `Glob` and `Grep` are never seen. What it can therefore refuse is a
  tool **in the matcher and not in the list** — `MultiEdit`, `NotebookEdit`, `Update` for that
  builder — and not `WebFetch`, `WebSearch` or `Task`, which are the ones an operator narrowing a
  sub-agent usually means. The module says it *makes the author's policy true*; it makes the part
  of it that overlaps the gate true. Widening the matcher would close it and is the one thing the
  matcher exists to avoid — waking this process for every `Read` and every `Grep` is a cost paid
  thousands of times to answer "not mine" — so this is stated rather than fixed.
- **The verdict's binding is checked at delivery only for what this run published through Estigia's
  own tools, with an earlier exact-receipt check at CI release.** `publish_review` records a fresh
  epoch over PR/head/base/digest while the PR is confirmed draft. `release_ci` re-verifies the live
  `review` claim, globally latest receipt across runs, latest distinct accepted verdict marker,
  current draft PR and coherent clean target, then marks ready and reads back every outcome. The
  gate asks for one accepted verdict; it does not enforce the two-blind policy's two-context
  agreement, and it cannot tell one context from two. An identical-byte republish still invalidates old evidence because it creates a new
  epoch. GitHub has no atomic conditional-ready operation, so an out-of-band ready or push can bypass
  this cooperative order.
  The local delivery gate still checks the published head at the
  boundary that spends it: a `git merge`, `gh pr merge`, `git tag` or `gh release` from a run whose
  recorded review head is not this checkout's head is refused as `verdict-bound-to-other-bytes`.
  `git push` and `gh pr create` are deliberately not gated on it, because pushing after a review is
  how a run fixes what the review found and the answer to a moved head is *re-publish*, not *stop*.
  What it does not see: a run that bypasses the tools and changes GitHub directly. The gate's own question to the tracker is still
  `verify-claim --issue --run-id --expect-state`; the head comparison is local, against the checkout.
- **This section is checked against the code.** `tests/honesty.rs` crosses the *countable* claims
  here — how many agents are gated, how many things `doctor` looks at, which mechanisms the code
  still uses — against the thing they describe, and fails when they drift. The claims about *kind*
  are judgements and are not checked, because a test that pretended to would be the false comfort
  the section exists to prevent.
- **The seams are checked along the path, not at its ends.** Three places had each end verified and
  the middle assumed: the gates, the flag lists the MCP tools build for the transport, and the
  values a configuration refusal tells an operator to write. Each is now crossed against the thing
  it has to agree with — the classifier, the transport's own `argparse`, and the parser itself.
  In a refusal, a backticked value is a literal a person types, and every one of them is checked to
  parse.
- **A registered gate is not an enforcing gate.** Five of them registered, fired, reached the
  classifier, found a tool name it did not know and stood aside — for four rounds, while `status`
  correctly reported `gate on`. Two guards now cross-check every matcher and the OpenCode plugin's
  own list against the names the classifier recognises. Two further instances turned up later, from
  the other end of the same path: an entry naming an event this build refuses (`PreToolUse`, the
  spelling the host's own documentation prints — a non-blocking error, so the tool call went through
  ungated with one line in a transcript), and an entry naming an executable that is no longer there
  (`cargo clean` deletes it; so does moving the checkout). Both were live on the machine this was
  written on. `estigia status` now reads the registered command back and says `gate REGISTERED BUT
  DEAD` rather than `gate on`, and `doctor` reports the file and the fault.
- **That count went 1 → 2 → 4 → 5 → 6**, once per round of actually checking. Every stop along the
  way was a verification route not yet exhausted rather than an agent that could not be gated —
  twice the answer was in a source repository the published documentation did not mention.
- **A worktree Claude Code makes for itself is not seen.** `--worktree`, `isolation: "worktree"` and
  background sessions create one through the runtime rather than through a shell, so no `Bash` call
  carries `git worktree add` and `PreToolUse` never fires. The event that does fire is
  `WorktreeCreate`, and it is not a permission gate: it *replaces* git's behaviour, expects the
  worktree path on standard output, and fails creation when the hook fails or prints nothing.
  Registering there to deny would make Estigia responsible for creating every worktree, and would
  break the feature for anyone whose run holds no issue. So the shell spelling is gated and the
  native path is not — and a write inside that worktree is still measured, because the claim covers
  the directory rather than the way it was made.

- **Claude Code subagents are covered; that is why the line below is OpenCode's and not the
  design's.** A subagent's tool calls fire the same `PreToolUse` hooks as the main thread, carrying
  `agent_id` and `agent_type`. Checked in the runtime's own hook reference rather than assumed from
  the parent behaviour, because the sibling runtime answers differently.

- **An extension point is not automatically a place to deny.** Three were checked and refused for
  the same reason, and it is worth stating once rather than three times. Claude Code's
  `WorktreeCreate` *replaces* git's behaviour and fails creation when the hook says nothing, so
  registering there to deny would make Estigia responsible for making every worktree. Goose's
  classification API is an HTTP service that labels text as prompt injection. OpenHands'
  `SecurityAnalyzerBase` is an in-process Python class returning a **risk level**, and what happens
  next is the user's confirmation setting. Estigia's refusal means *this run holds no claim covering
  this write* — rendering that as "high risk", or as an injection score, is a different sentence, and
  one the receiving tool is entitled to overrule. A gate is a place that asks permission and obeys
  the answer; anything else is a suggestion with a hook's costs.

- **Kiro's terminal agent can deny, and one unknown keeps it out.** Not the IDE — that was the wrong
  target: the IDE's hooks live in a workspace's `.kiro/hooks/` and `setup` writes user-level files.
  `kiro-cli`, from the Amazon Q Developer CLI lineage, is the gateable one. Its `PreToolUse` is the
  only blocking event, it rejects with **exit code 2 and the reason on standard error**, and any
  other code lets the action proceed — which is Estigia's own rule about a broken hook, and its
  existing exit-code dialect. The payload is `{hook_event_name, cwd, session_id, tool_name,
  tool_input}`: Estigia's shape exactly, with nothing to rename.

  What is missing is where the hook is registered. Kiro's own reference documents the events and the
  exit codes and **not the configuration path**; the only evidence for one is a third-party
  integration naming `~/.kiro/agents/<name>.json` and the key `preToolUse` — lower case, where the
  reference writes `PreToolUse`. Two unknowns, and both fail silently: a key in the wrong case is a
  hook that parses and never fires, and `<name>` is the operator's *default agent*, whose file
  Estigia cannot name. Settling it needs a real `kiro-cli` installation, not another documentation
  page. Everything else is already built.

- **An extension point is not automatically a place to deny.** Three were checked and refused for
  the same reason, and it is worth stating once rather than three times. Claude Code's
  `WorktreeCreate` *replaces* git's behaviour and fails creation when the hook says nothing, so
  registering there to deny would make Estigia responsible for making every worktree. Goose's
  classification API is an HTTP service that labels text as prompt injection. OpenHands'
  `SecurityAnalyzerBase` is an in-process Python class returning a **risk level**, and what happens
  next is the user's confirmation setting. Estigia's refusal means *this run holds no claim covering
  this write* — rendering that as "high risk", or as an injection score, is a different sentence, and
  one the receiving tool is entitled to overrule. A gate is a place that asks permission and obeys
  the answer; anything else is a suggestion with a hook's costs.

- **Kiro can deny and is not here yet, on purpose.** Its `PreToolUse` blocks on exit 2 and hands
  standard error to the agent — the same shape Windsurf uses — and its steering file at
  `~/.kiro/steering/` is global and always loaded. What is missing is where its *hooks* live for
  every workspace: only `.kiro/hooks/` inside a workspace is documented, and `setup` writes
  user-level files. Adding it ungated would buy a contract nobody enforces, which is the state
  Estigia exists to leave. The one thing to check is whether `~/.kiro/hooks/` is read; if it is, this
  is a table entry and a `GateSpec`, and nothing else.

- **Continue is gated twice over, and that is worth knowing.** Its hooks are Claude Code's by its
  own account — "these types match the exact schemas from Claude Code so that any hook written for
  `claude` works with `cn` out of the box" — and every piece was checked rather than taken on that
  word: the events, the envelope, the dialect, and the tool names `Edit`, `Write`, `MultiEdit` and
  `Bash`. Its directive goes in `~/.continue/rules/`, where a markdown file with no frontmatter is
  applied to every turn.

  It also loads hooks from `~/.claude/settings.json`, and **merges** them with its own. So a machine
  with `claude-code` and `continue` both configured runs the gate **twice per tool call** — a
  doubled tracker round trip at every boundary. `is_estigia_hook` deduplicates inside one file; it
  cannot deduplicate across two that another program merges. Setting up only one of the two is the
  answer, and Continue users who do not run Claude Code should pick this one.

- **Cline is gated by pausing, not by denying.** It has no "refuse this call" at all: its two
  stopping shapes are `cancel`, which kills the whole task, and `review`, which pauses for a person.
  Estigia uses `review` — a claim that could be renewed in one command is not worth throwing a task
  away over — which means a person can approve past it. That is the property `git push --no-verify`
  already has. Its hook is a script Estigia owns whole under `~/.cline/hooks/`, in PowerShell on
  Windows and shell elsewhere, because that is how Cline launches it; and where it reads MCP servers
  has not been verified, so none is registered.

- **Crush is gated and gets the tools.** Its `PreToolUse` blocks on exit 2, its matcher is a regex
  on the tool name, and its payload is `{event, session_id, cwd, tool_name, tool_input}` — the shape
  Estigia already reads, with nothing renamed. Hooks and the MCP server share one `crush.json`, and
  its MCP entry needs a `type` the other formats do not have. It gets a renderer of its own rather
  than Cursor's, which it resembles: Cursor's writes a `version` beside the hooks and Crush's schema
  has no such field, and one tool's scaffolding in another tool's configuration is how a file ends up
  parsing, looking right, and failing validation somewhere nobody connects back here.

- **Windsurf gets the gate and not the tools.** Its hooks file and its always-on rules file are both
  verified, so writes and shell commands are gated there — through an exit code rather than a JSON
  decision, which is the fourth dialect. Where its MCP servers are configured has *not* been
  verified, so Estigia registers none: an entry in the wrong file is a server that never starts under
  a `status` line saying it did. `claim` and the rest stay available as subcommands.

- **The OpenCode plugin does not see subagents.** `tool.execute.before` is reported not to
  intercept tool calls made by subagents spawned through the task tool. A gate with a hole is still
  a gate; a gate whose hole nobody mentions is a lie, so it is written into the plugin file itself.
- **The OpenCode plugin costs a process per gated call.** It shells out to the binary for `edit`,
  `write`, `patch`, `multiedit` and `bash` — named rather than `*`, because waking a process for
  every read would be a cost paid thousands of times to answer "not mine".
- **The push guard is a guard rail, not a lock.** `git push --no-verify` bypasses it, so does a push
  from another checkout of the same repository, and so does anything that writes refs without
  invoking hooks. A guard rail that claims to be a lock is worse than one that does not.
- **Nothing adjudicates what a push is aimed at.** git hands a `pre-push` hook the refs it is about
  to write, on standard input, for free. They are read and written into the ledger, so the record
  names *which* push was decided on rather than only that one was — and no decision is given them:
  what `guard::decide` takes is the checkout, and what it adjudicates is the three words `git push`.
  So a run holding a live claim
  may `git push origin HEAD:main`: the guard sees a claimed checkout and allows it, and the
  byte-binding check never fires, because `git push` is deliberately absent from the delivery
  population — *publishing a review target is how a run reaches review, and pushing after one is how
  it fixes what the review found*. That reasoning assumes a push is a step **towards** delivery. A
  push aimed at the base branch **is** the delivery, and it never runs `git merge`, so no verdict is
  ever asked for. For the eight adapters that get the contract and no tool gate, this hook is the
  only enforcement there is. Measured by `what_a_push_is_aimed_at_is_recorded_and_not_adjudicated`.

  It is not closed here because closing it wrongly is worse than the gap. The check needs the base
  branch and the `Integration` setting — under `trunk` the base branch is exactly where work is
  meant to land — and a boundary that refuses a legitimate push is how a harness gets uninstalled,
  which gates nothing at all.
- **The push guard refuses to replace somebody else's hook.** A `pre-push` is a test runner or a
  secret scanner as often as it is nothing, and Estigia will not take away a check somebody relies
  on to add one they did not ask for. It says so and names the way out.
- **The MCP server has never met a real client.** Every response is checked against the JSON-RPC
  envelope rules a client's parser enforces before any content matters, and the server is driven
  through a full session in tests. That proves the envelopes are legal and the answers are right; it
  does not prove a particular client is happy with them, and the protocol subset is a subset.
- **`label` is unmapped upstream and unexposed here.** The contract requires it; `bindings/github.md`
  does not map it and does not declare it unsupported either, which its own rules require. Frozen as
  a baseline that may only shrink.
- **The renewal window is a cadence, and a cadence is a gap.** A routine write inside two minutes of
  the last answer is not re-verified. A claim lost during those two minutes is a claim the gate does
  not catch until the window closes or a boundary arrives.
- **A JSON file Estigia edits keeps its shape and loses its shaping.** `settings.json` and
  `.claude.json` have to be parsed to be edited safely, and four things are read off the original
  and given back: the indentation, a byte-order mark, the line endings, and whether the file ended
  with a newline. Key order is preserved too — `serde_json` is built with `preserve_order` for
  exactly this. Measured rather than argued: every operator file of every adapter, written the
  ordinary Windows way, comes back **byte for byte** through an install and an uninstall, held by
  `every_agents_own_files_come_back_byte_for_byte`.
  What is *not* kept is the formatting inside the document. A hand-written
  `"allow": ["Read", "Grep"]` on one line comes back over four, and a blank line between two keys
  is gone — the value is re-serialised, and only the four properties above are read off what was
  there. So a diff on a file Estigia edited shows the entry it added, and shows the operator's own
  arrays reflowed around it. Markdown is spliced textually and does not go through any of this.
- **`skills/` is left behind, even empty.** It is the agent's namespace, not a file Estigia created.
  What comes out is the tree that was written.
- **The gate covers two directories: the checkout the claim was made in, and the isolated one
  `start_branch` created.** A run that makes its own worktree by hand gets that write gated but the
  directory is never recorded, so subsequent writes there read as `Outside`. Use the tool.
- **Two checkouts are told apart by resolving them, and by case when they will not resolve.**
  `canonicalize` answers with the real spelling on disk, so a live directory spelled two ways is one
  directory whatever the operator typed. A path this process cannot resolve has only its spelling
  left, and there the answer is folded the way Windows folds — the whole way, not only ASCII. This
  entry used to record the ASCII half as a residual, on the strength of a code comment claiming the
  transport folded that way too. It did not: `branch::spelling` calls `to_lowercase()` and the
  retired binding's `fold_case` called `.lower()`, and **both of those ran** — the harness was Rust
  and the transport that executed was Python, so two live components folding differently answered
  *is this the same checkout* differently. One of the two is gone and the rule that survived is the
  wider one, which is the direction that keeps a checkout from being mistaken for somebody else's.
  Measured by
  `a_checkout_this_process_cannot_resolve_is_still_one_checkout`, whose corpus differs by an
  accented pair rather than an ASCII one, because an ASCII pair cannot tell the two rules apart.
- **Three settings are read by nobody, and two more were until this change.**
  An operator sets eighteen rows and `config list` reports all of them. Measured: `context.get` is
  called for exactly three labels — `project board`, `worktree location` and, since the review
  handoff, `Review delegation`, which the transport reads to stamp one deadline on a request it
  never waits for. The gate reads `Irreversible commands`, and the payload's prose
  named eleven — leaving `Delivery authorisation`, `Transition authorisation`, `Delivery route`,
  `Merge strategy` and `Model routing` read by nothing at all. `setup::Applies::Asked` says of three
  of them *"the contract asks, and the agent may still honour it, but nothing checks"*, and for
  three the contract did not ask either. `Review delegation` left that arm with the review handoff:
  it carries its own sentence now, because what it asks of a runtime is a reviewer, not a decision.

  The two authorisations are closed: `SKILL.md` now says, in the steps where a state moves and where
  a change is delivered, that doing it unasked needs the row's permission. The other three stay
  open, declared in `every_setting_is_read_by_the_gate_the_transport_or_the_prose` and shorter-only:
  `Delivery route` accepts one value, `Merge strategy` names a topology for a `merge` this crate
  does not run, and nothing here starts a model. Acting on those is a design call, not a gap to
  paper over.
- **`doctor`'s rows were measured as builders and not as report — closed, with the measurement.**
  Each row's own function was tested. The **assembly** was not: downgrading every `Broken` of one
  row family on its way out of `doctor::full` left the whole suite green for ten of the eleven. A row
  could therefore be built correctly and dropped, filtered or softened on the way into the report,
  and every unit test on its builder would still pass — which was not hypothetical, because `full`
  once failed to pass the home directory to `state_root`, and the push-guard row vanished entirely
  under a tracker with no transport.

  All ten rows that **can** break are now forced through the binary, on states any machine can be put
  into: no skill; a contract taken from under a registered agent; a gate and a tool server whose
  settings name a binary that is not there; a checkout with no remote; a `pre-push` hook that is not
  text; an unreadable stand-down; an unreadable run pointer; a ledger line saying a call went through
  ungated; and, for the one row that is about the machine rather than the installation, a search path
  with no `gh` on it. The eleventh, `transport`, has no broken state: it answers `ok`, or `skipped`
  for a tracker with no executable.

  A row added later fails the same test until somebody forces it or says why they cannot.

  The same lens was then turned on the other two assembled pages, and found two more of the same
  shape. Blanking the **skill root** printed under a configured agent in `estigia status` left the
  suite green — the line that answers *which directory*, for somebody who has just been told their
  skill is out of date. And the whole of `estigia setup --companion <slug>` was crossed by nothing:
  its one `say!` could be replaced with a discard, and the unpublished companion could be answered
  with `cargo install leteo`, a command that would 404, without a single test objecting. Both are
  now read back out of the binary by `what_status_prints_is_what_somebody_needs_to_act_on` and
  `what_the_companion_verb_prints_is_crossed`.

  A row can also be wrong in the direction that reads as working. The `gate` row and `status`'s
  `gate on` both come from one question — *is Estigia's hook registered for this agent* — and
  `is_estigia_hook`, which answers half of it, could be made to say **yes to every entry** with the
  suite still green: the only files anything crossed it against held Estigia's own hook or held
  nothing at all. Between those sits an ordinary machine, an operator with their own `PreToolUse`
  hook and no Estigia entry beside it, and there a yes prints `gate on` over no gate. That machine
  is now built and read back by `somebody_elses_hook_is_not_this_agents_gate`.
- **Estigia cannot check who reviewed.** The contract requires that a review *"MUST NOT be
  performed by the context that wrote the change"*. The harness now sees a `review-verdict` marker
  and checks that the reviewer it credits is neither the publishing run nor any run that asked for
  review. Where the reviewing run recorded its own verdict that is timeline attribution; where the
  claim holder recorded somebody else's, it is a name that run supplied. The judgement still belongs
  to the agent; Estigia records its outcome and receipt but does not observe who or what produced
  it.
- **The harness holds tools for GitHub only.** `linear` and `trello` ship a binding the agent reads
  and no executable, so the tools refuse (`tracker-has-no-transport`) and the gate stands aside.
  Estigia can install and configure those trackers; it cannot enforce anything for them.
- **`doctor` checks eleven things, not everything.** Skill, transport, `gh`
  authentication, a git remote, this repository's push guard, the contract each configured agent
  reads, whether the gate each of them registers would actually run, whether the tool server each
  of them registers would actually start, whether the operator has the gate standing down right
  now, whether every run pointer on the machine can still say what it holds, and whether any call
  has reached that gate and gone undecided. It does not check the tracker's labels, the board, or
  whether the repository it found is the one the issues live in.
- **One of the eleven is about the past.** A call the gate cannot decide on — a payload it cannot
  parse, or one that never arrived — is waved through, and that is the right answer: a schema this
  build does not know could be wrapping a read as easily as a write. What is wrong is doing it
  quietly. Both leave a ledger line, and `doctor` is what reads those lines back, because an
  operator opens the ledger after being stopped and this is the case where they never were.
- **A run that swore nothing is never gated, including when Estigia is broken.** Failing closed on
  everybody would be a lock rather than authority, and would teach people to remove the hook. So a
  missing transport denies a sworn run's write and lets an unsworn one through — the oath is what
  brings a run inside, and nothing else does.

- **An unreadable run record stops a write, and does not stop a push.** The gate denies it: a
  pointer that exists and will not parse says a run existed and not what it swore, and an unknown is
  not clearance. The push guard cannot answer the same way, because it asks *which runs hold this
  checkout* and an unreadable pointer does not say. Denying every push in the repository over a file
  that may belong to unrelated work would be a lock, so the push goes through and this is written
  down instead.

- **A contract row the gate cannot read costs that row, and no longer costs the rows beside it.** The
  gate reads the installed contract with a fallback to defaults rather than refusing, because a file
  that will not parse is a problem `doctor` reports and not a reason to stop an agent mid-edit. What
  that fallback loosens is taken back two ways. The renewal window goes to zero, so nothing rides a
  cached answer. And the read keeps every row that parses: this entry used to say the rest "cannot
  be" taken back, on the grounds that the list which made a command a boundary was the thing that
  went missing — true only when the boundary row is *itself* the bad one. It usually was not.
  `Config::read` applied each setting with `?`, so a mistyped `Renewal window` three lines away threw
  a perfectly readable `| Irreversible commands | terraform apply |` out with it, and the gate
  classified `terraform apply` as a routine write. Measured by
  `one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it`, and salvaging can only narrow —
  `Config::default` is the loosest configuration there is, and every value a row can carry is refused
  unless it narrows that.

  What remains is the row itself: a *boundary* row that will not parse still costs the boundaries it
  named, and there is nothing to recover it from.
  `an_unreadable_contract_costs_a_declared_boundary_its_boundary_status` measures what an empty list
  buys the operator. That is the one place where *configuration may only tighten* runs backwards, and
  it is the cost of not refusing.

- **Every script this crate writes reads the binary's answer instead of inheriting it, and a script
  that cannot get one lets the call through with a word.** Only the codes Estigia defines are
  decisions: `1` is a refusal and `2` is an outcome it could not read back. Anything else — a missing
  binary at `127`, a panic at `101` — means it did not answer, and the write or the push goes
  through. The rule is written in the harness (*"a hook that breaks does not deny"*), and **two** of
  the four boundaries did not keep it. The push guard used `exec`, so deleting Estigia or moving it
  left a repository nobody could push from, with a fix nobody would guess. The OpenCode plugin
  caught every failure of its call and threw it as a refusal, so the same missing binary blocked
  **every write in the session** behind a message that was not a refusal —
  `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer` holds it to the three codes now, and
  checks that the file is still JavaScript while it is there.

  What that costs is the other half, and it is the honest half: on a machine whose binary has gone,
  every gated write goes out unmeasured. It says so on standard error each time. Failing closed
  instead would be a harness people uninstall the first time it is wrong at a bad moment, and an
  uninstalled harness gates nothing at all.

- **Nothing here resists a local actor running as you.** The gate reads a run pointer, a contract and
  a hook command, all of them files in your home directory with your permissions. Anything able to
  write them can rewrite what Estigia believes, and a binary that can be replaced answers whatever it
  likes. No authenticity or tamper-resistance is claimed and none is implemented — the threats this
  is built against are accidental: a truncated write, a crash mid-rename, two processes racing on one
  pointer. Somebody who already has your account does not need to defeat Estigia.

- **A stale writer is dropped, not merged.** A pointer on disk written more times than the one in
  hand is news the caller has not seen, and its store is skipped rather than applied. That keeps the
  fresher answer, which is the direction that fails closed; it does not reconcile two concurrent
  edits, and the dropped one is not reported — storing is best effort by design, and turning a
  missed record into a denial would make the gate loud in exactly the situation it should be quiet.

- **The boundary list is compiled, and can only be added to.** Estigia knows git and GitHub; it
  cannot know that a repository ships with `npm publish` or `terraform apply`, so `Irreversible
  commands` lets an operator name those and the gate treats them as boundaries — no renewal window,
  a fresh read every time. It **only adds**: no value removes a built-in, because a setting that can
  make the gate looser is not a guard rail. What it still does not do is let anybody express a rule
  the classifier has no shape for; matching is substring, on a normalised command line, and nothing
  more.

- **Decisions are written down; the reasoning behind them is not.** Every allow and every deny is
  appended to `~/.estigia/decisions.jsonl` with the run, the tool, **what the decision was about**,
  the verdict and the refusal code, so "why did it stop me?" has an answer after the terminal
  scrolls — and so does "what went through?". That second half was missing: a refusal is prefixed
  with its command and an allow was not, so `git tag v1.0`, `gh release create v1.0` and `git push
  --force origin main` under one claim left three identical lines reading `tool=Bash verdict=allow
  detail=issue #12 is held by <run>`. The record named everything Estigia stopped and nothing it
  let through, which is the half that changed the world. `Outside` is not recorded — that
  is Estigia standing aside, and a line per tool call of every session that never swore would bury
  the ones that matter. The file keeps its newer half past two megabytes, which means the oldest
  history is lost rather than the newest, and it is written best-effort: a refusal that could not be
  written down is still a refusal.

- **A payload Estigia cannot parse does not stop the call — and now says so.** A schema this build
  does not know could be wrapping `Read` as easily as `Write`, so refusing it would refuse reads;
  the hook stands aside, which is the right answer and has always been the documented one. What was
  wrong was that it was silent. `Outside` is not recorded, so an agent that changed its payload
  shape would take the gate out of the loop for *every* call while looking installed — the exact
  shape of the five defects this project has already paid for. An unreadable body now writes
  `payload-unreadable` to the ledger. It is a trace, not a denial: nothing is refused, and the only
  claim made is that somebody can now find out.

- **The ledger keeps one previous file, and two runs crossing the cap at once can cost it.** Every
  run on the machine appends to one history, and the run that finds it oversized renames it aside
  rather than trimming it — a rename moves no bytes, so nothing mid-append is torn in half. What it
  does not survive is two runs both finding it oversized before either renames: the second one's
  `.1` is the file the first just started, and the older history goes. No decision recorded *now* is
  ever lost that way, only what was set aside earlier, which is the trade a rotation makes and the
  trim did not. Measured by `every_decision_survives_the_ledger_passing_its_cap_while_runs_are_writing`,
  which is why that test asserts the history is whole or gone and never halved.

- **Held runs are pointers, not authority.** `status` lists what this machine's runs recorded
  holding, oldest answer first. Whether those claims are still live is the tracker's answer, and
  the list exists to say which issues to go and ask about.
- **Every `guard:population` declaration is found by walking `src/`, bound to its node and
  fingerprinted.** The sources are read off the directory rather than listed, so a declaration in a
  file nobody remembered to add is still checked. Changing the rule
  or the code beneath it reopens the claim: `tests/guards.rs` fails and names the new fingerprint, and
  somebody has to read the declaration again before recording it. That proves the pair has been read
  since it last moved — not that any declaration is true, and not that every guard needing one has one.
- **The seam guards read two shapes, not every shape.** They resolve markdown links and `<skill>/`
  rooted command lines. A payload that names a file some third way would slip past both — the
  `scripts/github.py` defect did exactly that until the second guard was written.
- **The directive's rules are crossed against the gate by name, not by reading.** The text every
  agent carries in context states three rules, and each is pinned to a named test that holds the gate
  to it — a rule deleted from the prose, or a test renamed out from under one, fails the build. What
  no test can check is whether the sentence still *means* what the test enforces. That reading is a
  person's job, and a guard that implied otherwise would be the false comfort this section exists to
  prevent.

- **The transport could not read the contract Estigia installs, and nothing said so.** Estigia
  renamed the configuration block to `estigia:config:*` and treats the `issue-flow:` pair as
  superseded, so an installed contract carries only the new spelling. The transport knew only the
  old one — and a marker it cannot find is not an error there, it returns an empty table. So after
  **every real `estigia setup`** the transport read *no operator configuration at all*: not the
  tracker, not the board, not the merge strategy, not delivery authorisation. It ran on empty
  defaults while the harness enforced the operator's actual table. Nothing failed and nothing was
  logged; the two halves of one decision simply stopped agreeing. The transport now reads either
  spelling. What held it was a crossing that installed through `setup` and required both sides to
  read the same thirteen rows; that crossing is deleted with the rest of the corpus, and the fix
  itself is what remains.

- **Estigia and the transport read the operator table the same way, and that is now checked.** Both
  read `SKILL.md` between the same two markers, and nothing compared them. They did not agree: the
  transport normalises a hand-written cell — backticked token, `**bold**`, an explanation after an
  em dash or in parentheses — and Estigia refused any cell carrying decoration. Refusing was not
  where it ended, because the gate reads the configuration with `unwrap_or_default()`: it would have
  gone on enforcing the *defaults* while the transport honoured what the operator wrote. The shipped
  table teaches that style itself (``| Tracker | `github` | …``), so this was reachable by following
  the example. What crossed the two readers over the same bytes is deleted with the rest of the
  corpus; `a_row_the_binding_acts_on_is_a_row_this_reader_finds` is what still holds this side of
  it, and the other side no longer exists to be held.

- **A legacy ownership epoch hashed the word `None`.** When a comment carries no id, its epoch is a
  digest of `createdAt` and `body` — and the transport reads those with `.get()`, which answers the
  same for a missing key and a JSON `null`. Interpolating that into the hashed string wrote the four
  letters `None` into the identity, so a comment with no `createdAt` hashed as `"None\n<body>"` there
  and as `"\n<body>"` in the port, which takes `&str` and can only say nothing at all. Two epochs for
  one comment is a release that never finds its target, which is the one thing this identity exists
  to prevent — and nothing chose to hash `None`; it is what an f-string does with an absence. Found
  by sweeping the crossings for the shape below rather than by waiting for it: the corpus posed the
  empty string and never the absence.

- **A forced takeover's evidence hashed differently on the two sides when a field was absent.** The
  digest that binds a forced reclaim to its reason is built from a JSON list, and the transport
  builds it out of a dictionary — so a marker carrying no `runtime`, `horizon` or `from` puts `null`
  in that list. The port took those three as strings, so an absent one arrived as `""`. Measured on
  the pair: `bcb711ad…` there and `4e09f435…` here, for the same event. Whichever side did not write
  the marker then reads the evidence as unbound and refuses a takeover the other performed. The
  crossing had existed all along and could not see it — its corpus posed no absence, and it adapted
  each case with `unwrap_or_default()`, which is the difference itself written into the test.
  Absence is `Option` on this side now, the corpus poses all four shapes, and the empty string is
  posed beside them so a fix that folded the two together would fail instead of agreeing.

- **The port stopped a `start-branch` the transport performs.** A branch-only worktree template is
  migrated to a run-scoped one, and both sides then refuse when the *pre-migration* checkout is still
  registered — it may hold unpushed work, so neither removes it. The transport asks
  `legacy.exists()` before it consults the registry and the port asked the registry alone, so they
  parted on the most ordinary state a worktree reaches: a directory removed with `rm -rf` instead of
  `git worktree remove`. Measured on a real repository, git goes on listing it — `prunable gitdir
  file points to non-existent location` — and neither registry reader filters that out. What the
  refusal protects is work in a directory, and its own way out says so; a registration whose
  directory is gone sends an operator to rescue a tree that is not there. The rule has a name now,
  `legacy_worktree_block`, so it can be measured without a tracker and a remote standing by.

- **A board mirror stamped ahead of the clock never went stale, on either side.** The window was
  `now - cached_at < 86400`, and the age of a stamp in the *future* is negative — so a mirror written
  while the machine's clock was a year ahead stayed fresh for a year. What it holds is the board's
  project id, field id and **option ids**, so an issue goes on being moved through columns the board
  may have renamed or deleted. The port's own `fresh` had already been rewritten once for the
  neighbouring half — a clock that would not answer at all — and left this one; it is the third time
  this crate has met the shape, after `standdown::covers` gained `declared_at <= now`. Both ends are
  checked now, on both sides, and the transport's rule has a name so the crossing asks it rather than
  re-typing it. Two more crossings went in with it: the board's **column matcher** was one rule
  written twice, in Rust and in Python, and nothing compared them — two copies agreeing is not a
  crossing.

- **The gate waited on the transport with no bound at all.** `Command::output` returns when the
  child does, and the transport bounds only *some* of its own `gh` calls — `DEVELOP_TIMEOUT_SECONDS`
  is the one it names — so a call left without a timeout held the gate open for as long as the
  socket stayed open. The measurement that found it never finished: a `cargo test` against a
  transport that sleeps was still running ten minutes later. A harness that hangs is worse than one
  that refuses, because the run has no verdict and no way to ask for one, and an operator whose
  harness hangs switches it off. The wait is now bounded at fifteen minutes — far past anything a
  working transport reaches, since its own longest documented wait is two — and what comes back is
  `Unknown`, never "nothing happened": the command may have posted a comment or opened a pull
  request before it stopped. For the same reason the **next interpreter is not tried**, which is the
  half a bound alone would not have fixed.

- **The two readers of the same prose folded different alphabets, and the population is now
  counted.** `re.IGNORECASE` matches `ſ` against `s`, and `ı` and `İ` against `i` — the last two are
  Unicode's Turkic-only mappings, applied unconditionally — while the port lowercases and compares
  ASCII-wise, which folds none of the three. Sweeping U+0020–U+30000 for characters the two rules
  disagree about returns exactly those, and they land in the words these matchers are made of: `you
  loſt` and `backıng off` were a stand-down the transport reads and the gate cannot see, which is
  what leaves a displaced run writing; `Claımed by <run>` was a held issue on one side and a free
  one on the other, which is rule one failing at the parse step. `### verſion 6.9.8` was an entry on
  one side and `no-changelog-entry` on the other, and where a file holds both spellings the
  transport refuses as ambiguous while the port picks one silently — the outcome that function's
  contract forbids, because the tag it feeds is immutable. `closing_hits` had met this same
  character and narrowed itself for it; the three readers beside it were left as they were, which is
  what a fault costs when nothing counts the population. All four are now narrowed on the transport
  side, crossed against their port, and
  `every_case_insensitive_pattern_of_the_transport_is_answered_for` asks the module — not the file —
  for the list, so a fifth cannot be added without an answer.

- **`setup --all` keeps each agent's own configuration.** It used to read the first one it found and
  install that everywhere, so two agents configured differently on purpose ended the run agreeing —
  the second rewritten to the first one's table, unasked and unmentioned. The configuration is now
  read from the skill root it is about to be written to. A root with no contract has no answer of
  its own and inherits, **but only when there is a single answer to inherit**: with two in play,
  picking one is the guess that caused this, so it takes the portable defaults and the run says so.
  What remains an assumption rather than a check is the inheritance itself — a new agent adopting
  the one configuration in use is a convenience, and an operator who wanted otherwise has to say so
  with `estigia config set`.
