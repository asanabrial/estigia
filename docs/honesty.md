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
  Only Claude Code currently receives host-routable definitions: the planning phases selected by
  `Planning`, and one static `review-blind` definition installed in every mode. OpenCode and every
  other host keep these values as routing declarations. `orchestrate`, `apply`, `judge`, and a visible
  route or installed definition are likewise not proof that a host executes them.
  Claude's generated matcher now wakes for current `Agent` and legacy `Task` launches of the exact
  `review-blind` target. Estigia walks from the launch cwd through the first `.git` repository root,
  parses candidate YAML, recursively refuses project-scoped shadows, and requires the normalized
  canonical user definition to be the only user-scoped definition with that identity; setup performs
  the same user-tree preflight. A running reviewer uses the embedded policy. This is Claude-only and
  proves what Estigia read before launch, not that Claude launched the context, kept judges concurrent
  or blind, withheld sibling output, or received an honest verdict. A refused launch is no evidence and
  never reduces or serializes the configured panel; the fallback is separate-session or durable handoff.
  OpenCode process-tree cleanup is likewise not containment proof: the controller bounds how long the
  TUI waits, while process-group and Job Object cleanup remain best-effort OS operations.

- **Raising the checkout action did not close this repository's Node 20 exposure.** `actions/checkout`
  moved from `v4` to `v7` because `v4` declares `runs.using: node20` and every run printed *"being
  forced to run on Node.js 24"*. It was one of **five**. Every `uses:` line in both workflows, with
  `runs.using` read from that action's own `action.yml` at the ref the workflow names:

  | Action | Where | `runs.using` |
  |---|---|---|
  | `actions/checkout@v7` | `ci.yml`, `release.yml` | `node24` — raised here |
  | `dtolnay/rust-toolchain@stable` | `ci.yml` | `composite`, shell steps only |
  | `Swatinem/rust-cache@v2` | `ci.yml` | `node24` |
  | `actions/setup-node@v4` | `ci.yml`, `release.yml` | **`node20`** |
  | `actions/upload-artifact@v4` | `release.yml` | **`node20`** |
  | `actions/download-artifact@v4` | `release.yml` | **`node20`** |
  | `softprops/action-gh-release@v2` | `release.yml` | **`node20`** |
  | `actions/attest-build-provenance@v2` | `release.yml` | `composite` — see below |

  So the warning still appears, and the argument used to raise the checkout in `release.yml` — that a
  tag-triggered workflow which will not start publishes nothing and cannot be retried by pushing a
  fix — still applies to that lane **four times over**. Three of the four are in that lane alone.

  `attest-build-provenance@v2` is composite and pins two actions of its own, both at commits rather
  than tags, and **both declare `node20`** — read from those exact commits, not inferred from a tag:
  `actions/attest@ce27ba3b4a9a139d9a20a4a07d69fabb52f1e5bc` and
  `actions/attest-build-provenance/predicate@1176ef556905f349f669722abf30bce1a6e16e01`. So the
  transitive exposure is two more, measured. It is not counted in the four above, which are direct
  `uses:` lines.

  `attest-build-provenance@v3` is composite too and pins `actions/attest@daf44fb9…` and
  `.../predicate@864457a5…`, **both `node24`** — read at those commits. So the transitive pair goes
  away with a one-line `@v2` → `@v3` at `release.yml:178`. The input surface is not *absent* —
  `release.yml:180` passes `subject-path` — it is **unchanged**: between v2's and v3's `action.yml`
  the only differences are the two pin lines and an added `NODE_OPTIONS`, with inputs and outputs
  identical. What a manifest diff cannot tell you is behaviour: v3 repins `actions/attest` to a
  release whose notes name a checksum-parsing change and a minimum runner version. Neither reaches
  this repository — the step passes `subject-path`, and the lane runs on hosted runners — but that is
  a thing read, not a thing the diff established.

  The guard is named for the checkout and checks only the checkout; it is not weaker than it says,
  but it is narrower than the problem.

  Left rather than folded in because the issue that raised the checkout asked about the checkout.
  What each remaining bump costs, read from each major's own `action.yml`:

  | Action | Majors to cross | Manifest change |
  |---|---|---|
  | `actions/setup-node@v4` → `v5` | one | not measured |
  | `softprops/action-gh-release@v2` → `v3` | one | one line: `using: node20` → `node24`; inputs, outputs and every description byte-identical |
  | `actions/upload-artifact@v4` → `v6` | two — `v5` is still `node20` | not measured |
  | `actions/download-artifact@v4` → `v7` | three — `v4`, `v5` and `v6` are all `node20` | not measured |

  The two columns are not the same kind of thing. **Majors to cross** is an ordering, and the rows
  are sorted by it: it was read for all four, so the comparison is paid for. **Manifest change** is
  not ordered at all — one cell holds a reading and three hold nothing, and a filled cell beside
  three empty ones ranks nothing. The row order carries no claim either: within a tie it is the
  order the table was written with, restored after one commit swapped it and the next put it back.
  Which commit did which is in the log, and this document has now got that history wrong twice, in
  consecutive attempts to tell it. Nothing here says which bump to make first.

  That distinction is what this entry kept getting wrong. It has now carried a wrong count, a wrong
  containment claim, a wrong remediability claim and two wrong superlatives — both written in one
  commit, one of them still standing after the next one, whose message was *"stop ranking things I
  have not measured"*. Every one was written about the thing being looked at while its neighbours
  went unread, and so was the sentence that used to stand here: it named the wrong commit for the
  surviving superlative, and one `git log -S` would have said so. **A comparative is a claim about
  everything, so it costs a measurement of everything.** Filed as its own item.

  **This list was wrong once already, and how it was wrong is the point.** It first named three,
  taken from a review's findings and re-presented as a measurement without the files being swept.
  `softprops/action-gh-release@v2` was missing, in the very lane the paragraph argues about, so
  anyone scoping the follow-up from it would have raised three actions and still had a `node20`
  workflow. An enumeration in this document is a claim to have looked everywhere; inheriting one is
  not looking.

  **And the guard's other half did not decide.** Seven rounds of review on this change found defects
  only in prose, which made a comfortable story: the code was right and only the writing was wrong.
  The eighth round mutated the workflow instead of reading it, and the story was wrong too. `cache-on-failure: true` was checked by searching the whole file for that text, so three
  ways of turning the fix off left the suite green — commenting the option out above a `rust-cache`
  left on its defaults, parking the option on some other step, and deleting the caching step
  altogether. The checkout half had stripped comments from its first line; the cache half never did,
  and the two sat eight lines apart in one function. It now reads the block under
  `Swatinem/rust-cache@` with comments stripped, and all three are red. **A setting is only on if it
  is under the step that reads it; a guard that searches the file instead checks that somebody typed
  the words.**

  That same half also carried a borrowed justification. Its comment said the issue had ruled out
  caching anywhere but `ci.yml`. The issue's out-of-scope line rules out caching *anything other than
  cargo* — which is exactly what `rust-cache` caches, so it permits what the assertion forbids.
  Keeping the release lane uncached is this repository's own judgement, and the comment says so now,
  with the condition under which the line should go. It used to give a mechanism — that the lane
  builds each target once, so a cache would be written and never read — which is a claim about
  GitHub's cache scoping that was not measured either.

  **What the guard has been found not to catch.** Every row below was produced by writing the
  workflow a different legal way and running the guard against it. This is a list of what has been
  run, not a census: nobody has enumerated the blind spots of a matcher that reads lines, and the
  count is left off because a count is a claim to have finished looking. The third column is what
  was *not* run — `lookup-only:`'s effect on the action's saving, not the guard's answer, which is
  measured and is that it does not read it.

  | Passes but should not | Fails but should not | Not measured |
  |---|---|---|
  | a checkout pinned to a **commit** — no version string to floor | a flow-style step, `- { uses: …, with: { … } }` | `cache-on-failure: yes` — refused, and whether GitHub hands the action `yes` or `true` was never read |
  | a caching step under `if: false`, **wherever a cache is allowed** | | `lookup-only:` — not read, and its effect on saving not measured |
  | a `uses:` folded onto the next line, **in a third workflow** | | |

  They are not one kind of thing, and an earlier version of this paragraph said they were. Two are
  about **meaning** a line cannot carry — a commit pin names no version, and `if: false` reads
  exactly like a step that runs. Two are about **syntax this reader does not handle**, flow style and
  folded values, and those a YAML parser would close. And two are simply **unread**, which is a third
  column rather than a hedge inside one of the other two: `yes` sat under *fails but should not*, and
  whether it should is a question about GitHub's parser that nobody here has asked. The action
  compares its input against the literal string `true`, so refusing `yes` may well be right.

  **A sixth row was taken off this table as fixed and was not.** It read *the option written under
  `env:` or in any key but `with:`*. What got measured was a `with:` holding a `NOTE:` that quotes
  the words — a different thing — and the row was deleted on it. Written as the row says, `env:` then
  `cache-on-failure: true` under the caching step, the suite stayed green while the option was inert:
  the action reads the input `INPUT_CACHE-ON-FAILURE`, which only `with:` writes, and its `post-if`
  looks for `CACHE_ON_FAILURE`. Deleting a row for a run of something adjacent to it is the failure
  this entry is a record of, committed inside the paragraph describing it. It is closed now, by
  reading the inputs from under `with:` rather than from the step, and this paragraph is what is left
  of the row.

  The folded-scalar row carries *in a third workflow* because it does not reproduce where a reader
  would first try it: written into `ci.yml` or `release.yml` the guard goes red instead, on the floor
  that says those two lanes must check something out. The row used to be unqualified, and a review
  ran it in the two files the entry is about and got the opposite column — a row describing a
  behaviour nobody had reproduced in the place it names.

  That distinction matters because the paragraph that stood here said *"four holes and two false
  alarms"*, said the left column needed a YAML parser, and said the right one needed a decision. A
  review then found six more by the method this paragraph prescribes, and none of the three sentences
  survived them: two of the six were case — `Actions/Checkout@v4` names the action a floor exists to
  refuse and slipped both floors — one was a different action whose path ends in this one's name, and
  three were a step title, an `if:` and a `with:` value quoting the word `uses:` beside a version. Not
  one is a meaning problem, and a parser would have closed none of them; they were comparisons of raw
  text that never normalised case or found where the key ended. All six are closed, by reading each
  line's key and value instead of searching it. **A number after a list of limits is the same
  assertion this whole entry is about** — the edge of what was measured, stated as the edge of what
  is there.

  The round after that made the point again on the same function. `runs_action` was given three
  normalisations — the key, the prefix, the case — and its comment said the six were closed by them.
  Quoting is the fourth axis of the same normalisation and was not looked at, so `uses:
  "actions/checkout@v7"`, ordinary YAML, stopped being a step: a correct workflow refused *and* a
  quoted `@v4` waved through a floor in a third lane, which is the case the widening exists for.
  This document had already recorded a guard of this repository's defeated by a leading quote,
  twice. Values are unquoted now, and the caching option is read as a key and a value too — searching
  the step for the literal text refused `cache-on-failure:  true` over a second space, eight lines
  from the half that had just been taught to read keys.

  **`save-if` was read from its description and not from its code.** The description says *"if
  `false`, the cache is only restored"*, so the guard refused the literal `false` — and `no`, `off`,
  `0`, `n`, `nope` and a quoted `"no"` each stopped every run saving while the suite stayed green.
  The action's `save.ts` does not test for `false`; it saves only when the lowercased input reads
  exactly `true`. That one line is the whole rule, and reading the prose beside it instead was the
  same act as inheriting a count: taking the nearest available sentence for the measurement. The
  guard now asks what the action asks, and leaves a `${{ … }}` alone, because what an expression
  evaluates to is not in this file.

  **And the fold reached the action and the value, not the key beside them.** `Save-If: false`
  turned off saving on **every** run — green ones too, which is worse than anything round eight
  closed — and the suite stayed green, while `Cache-On-Failure: true`, a spelling that works, was
  refused. GitHub hands an input to an action as `INPUT_<NAME>` upper-cased and its own input map
  compares without case, so those are the same settings as the lowercase ones. Three rounds running,
  one normalisation was applied to whatever was being looked at and not to its neighbour: the action
  name and not the key, the value and not the key, the key's spacing and not its case. Both sides are
  folded now.

  A step's keys are an unordered mapping too, and `- with:` written above the step's own `uses:` was
  refused, because the search for `with:` began one line after the step opened.

  **The refusal written for that was wrong in both directions at once.** Refusing every `- {` in a
  workflow refused a `matrix: include:` entry and a flow mapping used as an item of an input's
  value — correct files, told they had written a step they had not — while a whole `steps:` written
  as a flow *sequence*, `steps: [{ uses: Swatinem/rust-cache@v2 }]`, still passed green with the fix
  off, because it has no `- ` items to refuse. A step is an item of a job's `steps:`, so that is
  what is read now: items at the shallowest `- ` depth inside a `steps:` block, plus the block
  written as a sequence. Two more of the same shape went with it — a block step whose `with:` is one
  flow mapping was refused, and the option written a level *below* `with:` passed, which is the row
  this entry says it closed surviving one level in.

  **The run of rounds this took is itself the measurement, and the table below is where it is
  counted.** Each one found another legal spelling; each fix was right and the next spelling was not
  covered. That is not bad luck, it is what reading YAML a line at a time costs, and the entry has
  said since the twelfth round that closing the left-hand column properly means parsing rather than
  reading. Nothing here adds a parser — that is a dependency this issue did not ask for — so what is
  written instead is the ceiling: this guard reads the block style these two workflows are written
  in, refuses what it cannot take apart, and the table below records every legal thing it got wrong
  on the way. This paragraph gave the count as a number for three rounds after the number stopped
  being right, which is the smaller cousin of the branch that could not count its own commits: the
  row count is a fact about the table and stays current; a round count is a fact about the future.

  **Four rounds of closing one spelling each, and a newline walked past all four.** `- {`, items in
  the key's own column, `-  {` with two spaces, `steps: [` — each was measured, each fix was right,
  and putting the `[` on the line *after* `steps:` was read by none of them: no `- ` items, so
  nothing to inspect, so nothing refused. A lane checking out with `@v4` and carrying a bare
  `rust-cache` inside one measured green. A `jobs:` written as a single flow mapping did the same,
  with no `steps:` line to find at all.

  What is written now is not the fifth spelling. **A `steps:` that yields no step is refused,
  whatever the reason**, and so is a `jobs:` whose value is a mapping. That is the invariant the
  four were each an instance of, and it inverts the failure mode: a spelling this reader does not
  handle is a red with a message rather than a file nobody read. It is the shape the entry should
  have reached for at the twelfth round, when it first wrote that this needs parsing — refusing what
  cannot be read is the honest half of that sentence, and it costs no dependency.

  **And the flow-style row was in the wrong column outside the two lanes it had been run in.** It
  sat under *fails but should not*, which is what happens in `ci.yml` and `release.yml`: the guard
  reads `- { uses: … }` as no step at all, and the floors that say those two files must have a
  checkout and a caching step turn the miss into a red. A **third** lane has no such floor, so
  `- { uses: Swatinem/rust-cache@v2 }` discarded every red run's cache and the suite stayed green —
  the opposite column, in the case the loop was widened for. The same row shape as the folded scalar
  one, whose qualifier was added two rounds earlier by a review that ran it somewhere else; this one
  was never run anywhere else. A step written in flow style is refused rather than skipped, with a
  message saying the guard cannot read it rather than that it has checked it. Refusing a correct
  flow-style step is the price, and it is what the row already said.

  **That refusal said *everywhere* and meant one indentation.** A block sequence may begin in its
  key's own column — `steps:` and then `- uses:` at the same indent is ordinary GitHub Actions
  style — and the walk that collected a `steps:` block required its items to be *deeper*. At that
  indentation the block came back empty, so nothing was read and nothing was refused: a third lane
  could check out with `@v4` **and** carry a bare `rust-cache`, both halves of this change off, with
  the suite green and no message. The one-column-deeper form was refused correctly, which is why the
  round that wrote it saw what it expected. The block now ends at the first line that is neither
  deeper nor an item at the key's own column, and this paragraph is what is left of the word
  *everywhere*.

  **And the refusal that replaced it matched two literal bytes.** A block sequence item is a dash and
  any run of spaces, so `-  { uses: … }` counted as an item and was not refused — neither read nor
  rejected. Measured: `ci.yml` itself checking out with `@v4`, `ci.yml` carrying a bare
  `rust-cache`, `release.yml` gaining one past the assertion written to stop exactly that, and a
  third lane with both halves off — all green, at two, three and five spaces, while one space went
  red. Round twenty-one closed one indentation; this was the same defect at one spacing. An item is
  read by what it holds now, not by how far the dash is from it.

  The same round's other new rule read a flow mapping only when it closed on the line that opened
  it. `with: {` with the pairs beneath and the brace on its own line is legal, correct, and was
  refused — with a message saying a red run's cache was being discarded, which is a diagnosis about
  the workflow rather than about the reader that stopped.

  **Two rows of that same table, each fixed alone, were never run together.** A `cache-directories:`
  sequence ends a step early; `- with:` above `uses:` hides the step's opening. Written at once — a
  `- with:` step whose `with:` holds a sequence — the walk back for the opening landed on the
  sequence item *inside* the step, and a correctly configured action was reported as discarding its
  caches. A step's opening is now required to be shallower than everything between it and the line
  being read.

  Its neighbour `lookup-only:` is in the table above rather than in the code, because what it does
  to saving is a thing to measure and I have not — which is the difference between an unread input
  and one whose behaviour was guessed at from the sentence beside it.

  **A branch cannot count its own commits in a document it commits.** The pull request body said
  *seventeen commits, sixteen corrections, eleven touching the test*. It had been written at the
  seventeenth and published on top of the eighteenth, so all three numbers were short by one — the
  same defect as the round before, where the body had been left ten rounds unread and was wrong by
  nine. The second time it was not staleness: the arithmetic was true when written and the act of
  publishing it made it false. So the body says no number now. A count of a moving thing, written
  into that thing, is not a measurement that can hold, and rewriting it more carefully would only
  have moved when it broke.

  **And the same body halved the exposure it was summarising.** It said *"four `node20` actions
  remain, two of them transitive"*. This entry says four **direct** `uses:` lines and, in its own
  words, that the transitive pair *"is not counted in the four above"*: the measurement is six.
  Somebody scoping the follow-up from the summary rather than the entry would have raised four and
  left two — which is, to the action, the same harm this entry already records from the round where
  the list named three and `action-gh-release` was the one missing.

  **This guard has failed correct workflows twenty-three times, each for the same reason.** Every one was
  found by writing the workflow a different legal way and running it, and every one was a rule
  asserted from the single form sitting in front of the author:

  | It refused | Because it assumed |
  |---|---|
  | a `cache-directories:` sequence above the option | a step ends at the first `- ` |
  | a scheduled labeller or stale sweep | every workflow checks something out |
  | `actions/checkout@v4` named in a `run:` line | any line naming a version runs it |
  | a caching step written `- name:` then `uses:` | a step opens on its `uses:` line |
  | `rust-cache` named in a `run:` line or a step title | any mention of the action is a cache |
  | a `run: \|` body quoting a `- uses:` line | a block scalar's body is YAML |
  | `actions/checkout@v10` | `@v1` names one major |
  | a directory named `shared.yml` | a name ending `.yml` is a file |
  | a `run: \|-2` body quoting a `- uses:` line | a header's indicators come in one order |
  | `myorg/tools/actions/checkout@v4` | a name found anywhere in the value is the value |
  | a step title, `if:` or `with:` quoting `uses:` beside a version | a line holding `uses:` is a step |
  | a quoted `uses: "actions/checkout@v7"` | a value is written bare |
  | `cache-on-failure:  true` with a second space | a setting is found by searching for its text |
  | a quoted key, `- "uses": …` | only values are written in quotes |
  | `Cache-On-Failure: true` | an input's name is written in the case the docs use |
  | `- with:` written above the step's own `uses:` | a step's keys come in an order |
  | `- with:` above `uses:` **and** a sequence inside that `with:` | a step opens at the nearest `- ` above it |
  | a `matrix: include:` entry written `- { … }` | any `- {` in the file is a step |
  | a flow mapping as an item of an input's value | the same |
  | a block step whose `with:` is one flow mapping | an input block always has an empty value |
  | a `with: {` whose brace closes on a later line | a flow mapping closes where it opens |
  | `with:` with its flow mapping opening on the next line | a mapping opens beside its key |
  | a `#` inside a quoted value, in a flow `with:` | a `#` anywhere on a line opens a comment |

  The fourth is the sharpest to hit: `- name:` then `uses:` is the form this repository writes every
  step it names, including the `attest-build-provenance` step at `release.yml:177` that this entry
  argues about further up. Adding a name to the caching step — an ordinary edit — reported that a red
  run's cache was being discarded when it was not. That sentence used to say *two paragraphs above*,
  which was nine paragraphs out: a claim about where something sits in this document, made without
  scrolling to it.

  **The fifth is the sharpest to have written**, because this table already carried it, under a
  sentence claiming every row was red-to-green measured. It was not: the fix that closed it for one
  assertion was written up as covering the loop twenty lines above, and the loop was never run past
  the workflow that proves it. A correct file passed one half of the function and failed the other
  half of it. What the sentence was doing is what the whole entry is about — asserting the edge of
  what was measured — so the sentence is gone and the rows above stand on the runs in the commit
  that closed each one.

  **And widening the loop was done to one half of the guard.** The version floor was taken from two
  named files to the whole directory so a lane added later could not slip a `@v4` past it; the cache
  floor was left reading `ci.yml`. A third workflow copying a bare `rust-cache` discarded its red
  runs and the suite stayed green — which is exactly the copying the release-lane assertion beside it
  says it exists to prevent. Both halves read every file now, and the checkout-exists floor is the
  one thing scoped to the two lanes measured to have a checkout.

  **The number this whole change is argued from was inherited, and it was wrong.** Every document
  here said the episode cost *six* red runs, because the issue said so and named a run range. The
  run history holds **nine** consecutive failures of `ci` on `main` — `31753883982` through
  `31760121174`, no green between them — and the run the issue names as the first, `31752296629`,
  returns 404, so nine is a floor rather than a count. It took until the ninth round for anyone to
  run the one query that answers it, and the round before that had moved the figure *into* a
  sentence beginning "Measured on this repository" while deleting a different borrowed number from
  the same paragraph for being borrowed.

- **What the OpenCode plugin knows about where a call runs, and what it does not.** The gate decides
  which run a write belongs to by the directory the write happens in, and OpenCode's plugin context
  offers only a project — it carries no session identity to mint a run id from. So the plugin
  launches the gate from `worktree ?? directory ?? process.cwd()`, the project root, and forwards the
  tool's own arguments verbatim as the payload.

  For a **Bash** call those arguments carry `workdir`, the directory the command will actually run
  in, and `narrowed_by_the_call` reads it. That is the whole of the per-call evidence: one key, on one
  tool. Every other tool in the gated list supplies no working directory, and the key is not read for
  them even if a payload contains it — an argument no host documents sending is not evidence, and
  reading it there was measured to be an escape rather than an improvement.

  **What that key may and may not do is the part worth stating plainly.** `cwd` is written by an
  adapter's hook, which knows what it is gating, and is taken as given. `workdir` is a tool
  *argument*, so whatever composed the call wrote it — a model, on every runtime here. It may
  therefore only **narrow**: a relative value is resolved against the directory this process was
  launched in, the result is **placed** — the spelling collapsed the way this platform collapses it,
  then resolved as far as the filesystem exists — and a value landing outside the launch directory is
  discarded in favour of it. Measured before that clamp existed, with two live pointers and a
  `git commit` under a claim: `workdir` of `..`, of the parent checkout, and of `C:\Windows` all
  resolved, were covered by no run, and were answered `outside` with exit **zero** — the command
  still running where it was going to run, with the gate no longer adjudicating it.

  Placing it is not decoration on the comparison, it is the comparison. The first clamp compared with
  `covers`, which is written for working directories *that exist* and falls back to the path as
  written when resolution fails. `..` was then never cancelled, so `wt-a/../../nope` still started
  with the launch directory, was called inside, and was attributed to the holder of the component it
  climbed **through** — measured as `allow`, exit zero, under a claim the call had nothing to do
  with. A run holding one worktree could borrow another's authority by writing one `..`, and that is
  worse than the escape it replaced: the first reached `outside`, this reached permission. Both were
  found by review rather than by a test, on the road neither list of spellings covered, because every
  spelling either list held happened to canonicalise.

  Where two runs hold isolated worktrees inside one base checkout, a write arriving through any tool
  that names no directory is covered by both at equal depth and is refused
  `several-runs-hold-this-checkout` — correctly, on the evidence available, and indistinguishably
  from the Bash case that is now resolved. Estigia does not infer the directory from a file path in
  the payload, and it does not use OpenCode's session id: no invariant binds that id to the run id
  the tracker claim was made under, and inferring one would be ownership decided by something no
  timeline records.

  **The spelling is not crossed against the host, and neither is the resolution base.** `workdir` was
  read out of OpenCode's own shell-tool schema in the installed binary during review — *"The working
  directory to run the command in"* — so it is not guessed. But nothing in this tree holds a copy of
  that schema, and every test supplies the key itself, so if the host renames the argument the suite
  goes on passing while the fix is inert in production. The same staleness this file records for the
  model catalogs, with the same absence of a crossing.

  The resolution base is the sharper of the two. A relative `workdir` is resolved here against the
  directory this process was launched in, which the plugin sets from `worktree ?? directory`.
  OpenCode resolves it against its tool context's `directory`. Those are two fields of one instance
  record and they coincide in the ordinary case; started in a subdirectory of the project they do
  not, and then the shell runs in `<subdir>/x` while the gate adjudicates `<root>/x`. Both lie
  inside the project, so the clamp still holds and nothing escapes — what is at risk is *which*
  holder answers, which is the same class of wrong as the defect this entry exists for. Closing it
  means the plugin forwarding its resolution base rather than only its own working directory, and
  that is a second owner for the rule; it is recorded rather than done.

- **One test executes the generated plugin, and it is the only one that *requires* `node`.**
  `the_plugin_hands_the_gate_the_directory_the_call_runs_in` writes the plugin `setup` would install,
  drives its `tool.execute.before` hook with a stand-in shell, and reads back both the payload and
  the directory the gate would have been launched in. It asks for an interpreter because the artefact
  is JavaScript: the plugin is the fourth copy of the gated-tool rule and the only one in another
  language, and every other test of it asserts on its **source text**. Text is what let the
  working-directory defect stand — the source plainly said
  `const cwd = worktree ?? directory ?? process.cwd()` and plainly forwarded `output?.args`, and both
  sentences were true of a plugin handing the gate the wrong directory. It does not skip when `node`
  is absent; it fails and names what is missing, and both workflows install `node` rather than
  relying on the runner image having it.

  **It is not the first test to reach for `node`, and the older one skips.**
  `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer` already spawns
  `node --input-type=module --check` to prove the generated plugin parses, and when `node` is absent
  it prints a line and returns — a pass that measured nothing, one screen from the sentence above
  saying a skip spelled as a pass is a defect. It is left as it is here rather than quietly changed
  alongside an unrelated fix, and it is the same shape as the skip already filed against this
  repository's own push-guard fixtures.

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
- **Estigia cannot prove reviewers or blind judge panels ran.** `publish_review` mechanically freezes a
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

  None of it proves an independent context existed or establishes panel size, concurrency,
  independence, blindness, same-finding identity or quorum. It cannot prove one, two or five judges
  read those bytes or that their verdicts were honest. A marker can still be forged by a collaborator
  acting outside Estigia. `single`, `two blind` and `five blind` remain operator-selected review
  contracts, not observations the harness can make. The enforced floor remains
  one aggregate exact-receipt verdict: the transport has no per-judge marker and does not implement structured
  multi-verdict adjudication.
- **Estigia cannot make a person decide or start a turn.** For an exceptional human-adjudication
  wait it can preserve the built branch, PR, receipt, checks, and evidence, and record `blocked`
  with the exact decision or exit condition and discharger before ownership is released. That
  durable record is not a scheduler, notification guarantee, or decision; only the named person can
  discharge it. An ordinary delivery-evidence wait is not this case: missing review, CI,
  current-head/base evidence, or delivery permission stays in `review` with its exact blocker, even
  when a named person must supply the permission.
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
- **`ensure_draft`'s two doors are closed.** This entry named them and it is kept as a record of what
  the gap was, narrowed to what is now true. It runs `gh pr ready --undo`, a remote write, and both
  its `draft-readback-failed` stop and a failed `view_pr` read used to answer *nothing was written*
  afterwards — the same lie the post-push refusals had already been fixed for, sitting before the push
  rather than after it, which is how both fell outside the bar the issue set. `un_readied` carries the
  world on both now, and both are driven end to end: the read door by a readback that fails, the stop
  door by a pull request that comes back still ready.
  What the second door is worth is more than a report. Wrapping the `draft-readback-failed` condition
  in `if false &&` used to leave the whole suite green, so **nothing stopped a still-ready reused pull
  request exposing the new head to CI** — the barrier that refusal exists to hold. That mutation is
  red now, and so is routing its refusal through the shared wording: doing so replaced the action
  naming the hazard with one inviting an operator to `gh pr ready`, which is the exposure itself.

  Three more, from the same reviews and left with their measurements. The `[world-action]` guidance
  naming `Refs #<n>` is held by no test — stripping it from either refusal leaves the suite green —
  though the issue lists it under *unchanged*. `Answer::already_wrote` decides which exit-code arm
  `translate` takes and sits outside the `exit-code` population fingerprint, and that arm's
  `StatusRequired` axis is held by the fingerprint alone: a tripwire that says *go and read this*
  rather than a test of what it does. The pull-request body used to be read three times — by the scan,
  by `edit_pr`, and by `pr create` — so a body edited between the scan and the write was published
  unscanned, and the `Closes #<n>` it could gain auto-closes the issue on merge. It is one read now,
  hoisted above the pull-request listing and carried down as bytes, which is the *one place rather
  than three* this entry called the fix.

  Two smaller things found in the same review and left: the commit-range scan is one function now, but
  its two callers still read from different checkouts — `publish_review` from the isolated one where
  the commits are, `assess_autoclose` from the repository root — and unifying that changes which tree
  a standalone `check-closing-keywords` inspects, which is a decision rather than a tidy-up. And
  `keyword_sources` carries two shapes under one key, a list of strings from the precondition and a
  list of `{where, text}` from `assess_autoclose`. Nothing parses it today, which is the only reason
  that is a note and not a defect.

- **The control surface reaches further than it did, and what it costs is here too.** Issue 26. A
  write whose path lies outside every checkout the claim covers is answered `outside-the-claim`
  without asking the tracker — right for a scratch note, right for a `Boundary` write because those
  stay gated, and wrong for a `Routine` write to a file that governs the harness. The instruction file
  each adapter's `setup` writes its workflow-authority directive into was exactly that, and so were
  `~/.claude/settings.local.json` and `~/.claude/agents`.

  Counted on one platform: ten of the eleven adapters' instruction files answered `Routine` at the
  base commit, OpenCode's being already covered by the `.config/opencode/` entry. Counting spellings
  rather than adapters adds gemini-cli's `%APPDATA%` path, and the two `~/.claude` entries make
  thirteen. The basis is stated because three drafts of this sentence carried a bare number, each
  measured wrong by a different reviewer, and the fourth said twelve over a list of thirteen.

  The instruction files are derived from the adapter table rather than spelled, the way the skill tree
  is derived from `skill::DIRECTORY` — a hand-spelled copy agrees with the installer only until
  somebody renames one, which this crate has already paid for once. The crossing that keeps them
  honest is `every_control_file_an_adapter_has_is_one_the_gate_measures`: it resolves the real path
  per adapter, on all three platforms, under two `XDG_CONFIG_HOME` layouts, on both roads, and asks
  about the bare root as well as a file inside it. Every one of those four dimensions was added because a reviewer found a hole the crossing
  could not see, and the holes are the entry below. Only the first of them was then found by the
  crossing itself, once it had the dimension — the rest were measured by people, which is the
  honest attribution and not the flattering one.

  And the crossing catches a stale fragment **only where nothing else already matches the path**.
  Measured by staling each of the eleven in turn: it reddens for eight and stays green for three —
  continue, cline and windsurf, whose paths the rules-directory entries cover independently.
  OpenCode's *is* caught, because the crossing walks the relocated `XDG_CONFIG_HOME` layout where
  nothing else matches `<moved>/opencode/AGENTS.md`. All eleven are caught by
  `the_spelled_instruction_files_and_the_adapter_table_agree`, which reads the spelled list against
  the table. Three drafts of this sentence were wrong — it was deleted with the version of the fix it
  described, restored saying four, and inverted to read as though eight was the small number.

  **What the missing dimensions were hiding.**
  `%APPDATA%\gemini\settings.json` — where this harness registers Gemini's own deny hook — answered
  `Routine`, because only the POSIX spelling with its leading dot was listed. `opencode`'s plugin,
  that adapter's only deny mechanism, and `crush`'s settings answered `Routine` whenever
  `XDG_CONFIG_HOME` was moved. Four entries carried a trailing slash, which split the two roads:
  `surface_of` appends a separator, so `rm <dir>` was `Boundary` while a write to the bare directory
  was `Routine`. And the crossing asked only about writes, so a path containing a space — the shape
  that produced the `cli/hosts.yml` entry — would have arrived unnoticed. All closed here, each with
  a dimension of the crossing that now holds it.

  **Gating a file in a directory the host reads whole is defeated by a neighbour.** `paths_in`'s own
  comments say so for two of them: Continue applies any rule with no frontmatter, and Cline loads its
  rules directory for every task. Estigia's filename was gated and the directory was not, so
  `~/.cline/rules/zz-override.md` answered `Routine` — and a sibling saying *Estigia is retired*
  changes what an agent is told this harness may enforce without touching a `Boundary` path.

  Two more directories are named alongside them on a weaker basis, and it is worth being exact about
  which: `windsurf/memories` and `.cursor/rules` are locations those hosts read, but `paths_in` says
  nothing about them reading a directory whole — its Windsurf comment is about a six-thousand
  character cap on one file — and Estigia writes nothing into `.cursor/rules` at all. They are in the
  population because the restated rule covers them, not because this crate has verified the
  read-whole behaviour. A first draft of this paragraph claimed all four were documented; a reviewer
  read `paths_in` and found two.

  **The cost, which the population's own declaration understated.** That declaration says a false
  positive costs one tracker read "and that is the direction this chooses on purpose", illustrated by
  a one-character typo on a path already listed. A `Boundary` never rides the renewal window and never
  stands aside outside the claim, so it is a live `gh issue view` every time — measured, not inferred:
  a reviewer drove 30 invocations of each class through the real hook against a `gh` with a real
  process boundary and counted **30 tracker calls out of 30** for a `Boundary` against **0 out of 30**
  for a `Routine`. With no network it is a refusal rather than a delay.

  **What that read costs is the number to distrust, and three independent measurements disagree.**
  0.61–1.22 s (mean 0.93, n=8); 0.58–0.92 s (mean 0.66, n=8); and 0.85–3.20 s (mean 1.74, n=10) —
  the last one roughly double the first two and 2.6× their ceiling, taken against the same tracker on
  a different machine and a different network. Against 0.05–0.42 s for a `Routine` write inside the
  renewal window, likewise machine-dependent. So the honest statement is a **round trip to GitHub per
  write to a watched path, between half a second and three seconds**, and any tighter figure here is a
  sample of one environment rather than a property of the change. The earlier drafts of this paragraph
  quoted the first measurement alone as though it bounded the cost, which is what a reader would have
  believed.
  Two attempts at the `XDG_CONFIG_HOME` fix were wrong in opposite directions before this one, and
  both were found by reviewers rather than by a test. Dropping the `.config/` prefix left `opencode`
  as a bare directory name matched anywhere: `node_modules/opencode/**`, `packages/opencode/**` and a
  checkout *named* `opencode` answered `Boundary` on every file in them. Replacing the directory
  entry with three tails then **loosened** it — everything under `~/.config/opencode/` that Estigia
  does not write went `Boundary` to `Routine`, which is an existing entry's sensitivity changing and
  was recorded here as an over-gating fix. Not a countable set: it is whatever OpenCode itself keeps
  there, and three drafts of this sentence said *nine* without anybody being able to enumerate them.
  A reviewer measured `auth.json`, `config.json`, `themes/`, `command/`, `instructions/`, `tui.json`
  and `mcp.json` among them, and the bare directory. The directory entry is back *beside* the tails, which
  cover the relocated root it cannot.

  What that leaves, measured rather than claimed: `node_modules/opencode/index.js` and
  `packages/opencode/src/main.ts` are `Routine` again, and `node_modules/opencode/agents/**`,
  `node_modules/opencode/plugins/**` and `node_modules/opencode/opencode.json` are not — a vendored
  copy of that agent with those exact subdirectories pays a tracker read per write. An earlier
  sentence here said six shapes had come back, then three; the number was never the point and
  neither figure was measured. This is the list.

  What over-gating remains, measured rather than argued. A fragment ending in `/` names a directory
  and the matcher honours that, so a name that merely *ends* alike — `.estigiaignore`,
  `skills/flow.md`, `.claude/agentsmith.md`, `.cursor/rulesets.md` — stays `Routine`. A fixture holds
  that direction, which nothing did before: every other guard here asserts something *is* `Boundary`,
  so over-gating was invisible to all of them.

  The left side is anchored for two kinds of fragment: those beginning with a **dot**, and those
  naming a **directory**. A dot-directory is always a whole segment, so `my.claude/agents` is not
  `.claude/agents`. Every real target of `opencode/agents/`, `opencode/plugins/`, `windsurf/memories/`
  and `skills/issue-flow/` has that first segment whole too, so those anchor without losing anything —
  measured, `~/.config/opencode/agents` stays `Boundary` on both roads.

  It took three attempts, and each one is why the fixture lists what it lists. The first anchored
  `ends_with` and left `contains` alone, so a bare `my.claude/agents` came back `Routine` while a file
  under it stayed `Boundary`, and three documents said the case was closed. The second anchored the
  dot fragments only — which left the road split alive on precisely the directory entries this change
  added: `surface_of` gives every token a trailing `/`, so `/repo/.opencode/agents`, `/repo/.opencode/plugins`,
  `/repo/xyzopencode/agents` and `/repo/notwindsurf/memories` each answered `Routine` to `Write` and
  `Boundary` to `rm`. The suite was green over all of them, because nothing asserted the `Routine`
  direction for a non-dot fragment. All four are in the fixture now, on both roads.

  Anchoring cost coverage on the shell road three separate times, and the shape of that mistake is
  worth more than the fix. `surface_of` builds a view of the command and matches the fragments against
  it. Anchoring means a fragment now needs a separator in front of it, and each attempt supplied one
  in a narrower place than the shell can put punctuation:

  1. **Split on whitespace, append a separator.** A relative operand then has a *space* in front of
     it and is never at position 0, because the verb is. Removing the state directory by its bare
     relative name, or truncating the file the gate is registered in, went `Boundary` at the base to
     `Routine`.
  2. **Wrap each token on both sides.** That reached exactly the operands whose first character is
     the fragment's, and no others. A quoted operand, a redirect written with no space after it, and
     an operand joined to a long flag all put a character between the token boundary and the
     fragment, so all three stayed `Routine` — again reaching the run pointer and the settings file,
     by the shortest line there is. The drive-relative Windows spelling `C:` did the same thing and
     reached the **write** road as well, which none of the others did.
  3. **Fold every character a path segment cannot contain into a separator**, and wrap the line. That
     reached the quotes, the redirects and the long flags at once — and not the fourth family, because
     the character in the way there is an ordinary **letter**: a short option carrying its value
     attached, `-o.estigia`, `-C.estigia`, `-oskills/flow/SKILL.md`. Brace expansion was the same
     miss one character wide.
  4. **Fold the braces too, and offer every split point of a token beginning with `-`.** The braces
     could join the folded set because no fragment carries one. `-` could not: `hooks/pre-push`,
     `.estigia/stand-down.json` and cursor's derived fragment all carry one, and folding it cuts them
     in half and loses them outright. A shell gives no way to know where the option letters end and
     the value begins, so every split point is offered instead. That is what is in the code.

  5. **Normalise each token on its own and join afterwards.** Folding whitespace made the whole line
     read as one path, and the parent-segment collapse then reached **across an operand boundary**:
     `mv .estigia ..` became `mv/.estigia/../`, collapsed to `mv/`, and the surface being moved was
     gone. At the base the tokens were joined with a space, so `/../` never formed there. 66 measured,
     all of the shape *surface operand followed by an operand beginning `..`* — `mv .estigia ..`,
     `cp -r .estigia ../snapshot`, `mv ~/.claude/settings.json ..`. These are the ordinary way to move
     a config file aside, and this was the worst of the six because of that.
  6. **Read the option prefix off the folded token, and let `~` start one too.** The suffix rule
     filtered on the *raw* token beginning with `-`, so a leading quote defeated it while the fold
     still left the option letters in the way: `wget "-O.claude/settings.json"` and
     `7z x a.7z '-o.estigia/run.json'`, 38 measured. Quoting `-o<dir>` is the documented 7-Zip habit
     for the reason the rule exists — the directory may hold a space. `%~dp0` expands *with* a
     trailing separator, so `%~dp0.estigia\run.json` is the correct batch idiom and puts a digit where
     the anchoring wants one; `%` joined the folded set and `~` joined `-` as a prefix marker. The
     caret joined the set on the same round: under `cmd.exe` `^` escapes the next character, so
     `del ^.estigia\run.json` names the state directory. It was the one character of 660 a reviewer
     swept where the base's answer was right for a reason rather than by accident — `!`, `#`, `@`, `[`
     and the rest name different files, and head stopped gating those on purpose.
  7. **Ask each rung of the ladder on its own.** The suffixes were appended into one view, separated
     by the same character that separates path segments, so adjacent rungs concatenated into a path
     that was never in the command. The ladder for `~/.claude` is `.claude/claude/laude/…`, and
     `.claude/claude` is ClaudeCode's derived instruction fragment — so a recursive delete of the home
     config directory answered `Boundary` for a path nobody wrote, and so did `~/backup.claude`,
     `~/notes.agents` and `-obackup.claude`. `.agents/agents` has the same `A/A[1..]` shape and did the
     same. Introduced by the sixth, found by a reviewer, and it had already made the
     containing-directories paragraph below false about the very directories that paragraph declares
     open.
  8. **Bound the ladder to the marker's own segment, and read a patch body the same way.** Two halves,
     both found by one reviewer. Unbounded, the ladder voided the left anchoring for every token
     beginning with `~` — which is how a home path is written — because the rungs of
     `~/my.claude/agents` include `.claude/agents`. Eighteen of the thirty-one rows of the anchoring
     fixture answered the opposite when respelled with `~`, against three paragraphs here that name
     those paths as `Routine`, and every row of that fixture was spelled absolute. An option prefix and
     a shell expansion both end before the first separator, so cutting past one is not reading a
     prefix, it is deleting path segments. And the payload road — the fallback `classify_with` takes
     for Codex's `apply_patch` and OpenCode's `patch`, where the path sits in a patch body rather than
     a field — handed the whole blob to `is_control_surface` directly. That was right under a bare
     `contains` and wrong once the fragments were anchored: thirteen relative spellings, the run
     pointer and the stand-down record among them, stopped matching on that road while `Write` still
     gated the same file. Both roads read free text the same way now.
  9. **Read the ladder off the same string the matcher reads.** The view is built from
     `normalise(&fold(token))` and the ladder's entry test read `fold(token)` alone — `normalise`
     folds `:` and `\` and `fold` does not. So in `${P:-.claude/settings.json}` the `-` starts a
     segment in the string that is matched and did not in the string the ladder inspected: no rung,
     and **fourteen of the nineteen** relative surfaces answered `Routine` where the base answered
     `Boundary`. The basis, because a number here is worth nothing without one: one relative spelling
     per fragment in the **base's own** fragment set. The five that were never lost are the two file
     fragments carrying no leading dot, `hooks/pre-push` and `cli/hosts.yml`, which the anchoring
     never touched so the ladder was never needed to reach them, and `.config/opencode/opencode.json`,
     `.gemini/settings.json` and `.config/crush/crush.json`, each reached by an unanchored sibling this
     branch added. This sentence has now been wrong twice: first *thirteen*, with no basis and
     unreproducible, then *every relative surface*, which is checkable and false — it would have told a
     reader the push guard stopped being gated, and it never did. A reviewer measured both.
     `TARGET=${1:-.claude/settings.json}` then `rm -f "$TARGET"` is the ordinary script idiom for a
     defaulted path. A reviewer found it by sweeping **two**-character contexts; the 660-character
     sweep recorded above is one character wide and could not reach `:-`. The same fix reaches `:~`,
     `\-` and `\~`, which that reviewer's exhaustive two-character sweep names as the only other
     non-lookalike contexts that were open.

     **What stays open, why it is a limit rather than an oversight, and what it costs.** `${P-path}`,
     `${P:+path}` and `${P:?path}` answer `Routine`. Not *still* — they answered `Boundary` at the base
     commit, so this is a **loss against the base** on the same fourteen of nineteen relative surfaces
     counted on the same basis as above, and unlike every
     other spelling recorded in this section those three do address the file. Named that way because a
     reviewer pointed out that the earlier wording read like a gap that had always been there.
     Once `$`, `{`, `}` and `:` are folded away, `P-path` is
     indistinguishable from a file named `P-path`, and `+`/`?` sit where a segment's first character
     would. Laddering from a `-` in the *middle* of a segment would reach them and would also gate
     `x-.claude/agents` and every `foo-bar` tail like it — trading a hole for over-gating on ordinary
     names. Measured and left, rather than closed by widening.

  `7z` is why the fourth is not optional: its extract-to spelling is `-oDIR`, a space there is a
  syntax error, and it is in `WRITES_A_FILE` deliberately — so the only correct way to write "extract
  into the state directory" was the one spelling that was not gated.

  Each attempt was measured against the base by a reviewer classifying raw command strings through
  the built binary, and each time the whole suite was green over the loss — because every fixture in
  this crate spelled its commands the way the last fix had taught it to. That is the same blind spot
  **nine** times: a narrowing nothing asserts, found by a reader who spells the command differently
  than the fixture does. It is recorded at this length because the count is the finding, and the count
  is the honest summary of this work: anchoring a matcher against a shell is not one change. Each
  spelling that reaches a path is its own claim, the suite went green over every loss until somebody
  wrote the spelling down, and **five of the nine were introduced by the fix for the one before it**.

  The eighth paragraph of this section predicted a ninth and a reviewer found one in the next round,
  which is the closest thing here to a measurement of the process rather than the code. A tenth is
  likelier than not. The fixtures hold the nine spellings that were found, plus the eighteen folded
  characters and the ordinary-line direction. They do not hold *the space of spellings*, because nothing in this crate
  enumerates it. A property test over generated command lines, crossed against a second reading of the
  same path, is the shape that would — and there is none. `punctuation_does_not_hide_a_control_surface`,
  `a_drive_relative_spelling_is_measured_like_a_relative_one` and
  `a_relative_operand_is_measured_like_an_absolute_one` hold the direction now, and
  `folding_punctuation_does_not_gate_an_ordinary_line` holds the other one, because reading
  punctuation as separators makes the view more permissive and a false `Boundary` costs a live
  tracker read on every build.

  What this leaves is stated rather than measured away. A command that *mentions* a control surface
  while writing somewhere else is `Boundary`, which is the asymmetry `surface_of` already ran on and
  pays one extra tracker read. Substitutions are still invisible — a path assembled from a variable or
  a subshell names nothing any of this can match, at the base commit as much as here. So is a surface
  split across a `cd`, and so is a glob that removes a directory's contents without naming the
  directory; a reviewer measured all three at both commits, and they are the same at both.

  The option-prefix rule is generous in one direction only — it can gate a token that merely ends like
  a surface, never miss one that is a surface. That cost was priced rather than assumed: across 50
  ordinary developer command lines, including the write-heavy ones whose flags take attached paths,
  it gates **none**. `an_option_prefix_does_not_gate_an_ordinary_line` holds twelve of them. A reviewer
  measured 149 lines the same way, 30 of them deliberately punctuation-heavy, and found the folding
  costing **nothing** on any of them: folding only creates separators, and a separator only helps a
  fragment that is already in the line.

  What the *list* costs on an ordinary path is separate and is real. `mkdir -p
  test/fixtures/windsurf/memories` — a project directory that happens to be spelled exactly like a
  fragment — is `Boundary` here and was `Routine` at the base. That is not the lookalike class named
  above, which is about names that merely resemble one; it is an exact collision inside somebody's own
  tree, and it costs a tracker read per write. The same holds for a project's own `.claude/agents` or
  rules directory, which is intended. A reviewer found this one by classifying paths a developer would
  plausibly create rather than paths this crate already talks about.

  **The two halves of the wrap are not alike, and the paragraph that used to stand here got it
  backwards one commit before it was read.** It said the trailing half was not load-bearing. That was
  true when it was written and false as soon as the option-prefix rule landed, because the suffixes
  that rule offers are appended *after* the wrap: without the trailing separator the first suffix runs
  straight into the folded line and the last segment of a command stops being a whole one. **Two**
  fixtures redden without it now — `a_relative_operand_is_measured_like_an_absolute_one` and
  `a_later_operand_does_not_collapse_an_earlier_surface` — and it is the separator pushed **between
  tokens** that carries it, not a wrap at the end of the line: dropping the final separator alone, or
  the one closing each ladder rung, leaves the whole suite green. This paragraph said *eight*, which
  was measured before the ladder rungs were split out and never measured again; a reviewer counted. Nobody re-measured the sentence when the code under it moved, which
  is the failure this document exists to catch, committed inside this document.

  The **leading** half is the inert one, and it is inert structurally rather than by luck: `anchored`
  tests `starts_with` as well as `contains`, so a fragment at position 0 is reached either way, and
  dropping the wrap can only *add* matches. Measured over every fragment the gate consults, five
  spellings and five write verbs — 975 command lines, **zero answers changed**. It stays for symmetry
  and nothing holds it, which a reader should know rather than infer a guard that is not there.

  Each of the eighteen folded characters is now held individually, and that took two attempts as well.
  A reviewer mutated the set character by character and found ten of the then-fourteen could be
  dropped with the whole suite green — the set had been justified four characters at a time by
  measurement and the rest by the sentence *"the rest terminate a word the same way"*, which the
  missing brace had already disproved. The first fixture written for it **iterated the constant**,
  which reads like coverage and is none: dropping a character from the set also drops it from the
  test, and eleven mutations still survived. The characters are spelled in the fixture now and crossed
  against the constant, so all eighteen die.

  A fragment naming a **file** without a leading dot still cannot be anchored, because `cli/hosts.yml`
  exists to match **mid-segment**: the Windows path is `%APPDATA%\GitHub CLI\hosts.yml`, and
  `github cli` is one segment holding a space. The cost is that a vendored copy of somebody else's
  agent answers `Boundary`, and will keep doing so: `~/.codeium/notwindsurf/memories/global_rules.md`
  through windsurf's derived fragment, and `vendor/myopencode/agents.md`, `vendor/mygemini/gemini.md`
  and `vendor/mycrush/crush.md` through theirs. A draft of the fixture asserted the windsurf one away
  and went red, which is why it is written here rather than believed.

  The other three are here because two reviewers measured this paragraph and found it naming
  `vendor/myopencode/agents/a.md`, which answers `Routine` — `opencode/agents/` is a **directory**
  fragment and is therefore anchored, so a lookalike cannot reach it. The derived fragment that
  demonstrates the limit is `opencode/agents.md`; the written path was off by a separator, and nothing
  in the suite held it, because this file asserted the ends-alike direction and never this one. It is
  held now, on both roads — which is the point of the paragraph: a declared limit nothing measures is
  a limit that drifts from the code while the document keeps saying it.

  What still over-matches is the fragments that name a file: `.claude/settings` reaches
  `.claude/settingsmap.ts`, and `crates/crush/crush.json`, `docs/gemini/settings.json` and
  `tests/fixtures/gemini/settings.json` answer `Boundary` because two-component fragments match
  anywhere. Deliberate — `.claude/settings` is trimmed on purpose to reach `settings.local.json`.

  And a per-project cost that is new and worth naming plainly: `<repo>/.claude/agents/*`,
  `<repo>/.cursor/rules/*`, `<repo>/.cline/rules/*`, `<repo>/.continue/rules/*` and
  `<repo>/.claude/settings.local.json` are `Boundary` now. A project that keeps its own agent
  definitions or rules pays a live tracker read on every write to them. This crate ships agent
  definitions under `skill/agents/`, which is not one of those paths — measured, no file tracked in
  this repository trips any fragment today.

  **Two things this change leaves open, both measured by reviewers of it.**

  Over-gating is held only for hand-spelled shapes, and it is held **unevenly**. The fixtures that
  watch the `Routine` direction list paths by name, and nothing derives an over-gating check from the
  adapter table, so whether widening a *derived* fragment is caught depends on whether some hand-
  written row happens to sit under it. Measured, one adapter at a time, each widened to its bare
  directory and the full suite re-run:

  - `crush/crush.md` → `crush/` **reddens** — `the_declared_over_gating_is_the_shape_the_document_names`
    names `vendor/mycrush/crush.md`. So does `.cursor/estigia-workflow-authority.md` → `.cursor/`,
    through the `.cursor/rulesets.md` row in the ends-alike fixture. `gemini/gemini.md` and
    `opencode/agents.md` are covered by the same over-gating fixture.
  - **Two** are silent, measured one at a time with the full suite: `.qwen/qwen.md` → `.qwen/` and
    `.cline/rules/estigia.md` → `.cline/` leave every test green. Nothing in this crate names a path
    under those two directories in the `Routine` direction. This list has now said *one*, then *four*,
    then two — the four was true until `the_suffix_ladder_does_not_synthesise_a_path` landed in this
    same branch and began catching `.agents/` and `.codex/`, and nobody re-ran it. Both re-counts came
    from a reviewer rather than from this repository.

  The earlier draft of this paragraph used crush as its example of the silent case, which stopped
  being true in the same change that added the fixture above; qwen is the measurement now. The
  under-gating direction is crossed against `resolve_paths` for all eleven adapters. This one is
  crossed against nothing, and the coverage it has is incidental rather than derived.

  And the **containing** directories are `Routine` on both roads while their contents are now
  `Boundary`: `~/.claude`, `~/.claude/skills`, `~/.codex`, `~/.cursor`, `~/.qwen`, `~/.agents`,
  `~/.gemini`, `~/.cline`, `~/.continue`, `~/.codeium`, `~/.codeium/windsurf`, `~/.config/crush`,
  `~/.config/gh`. So a single recursive delete of `~/.claude` — taking the settings file the gate is
  registered in, the workflow-authority directive, the agent definitions and the installed contract —
  answers `Routine`, and being outside every checkout it is answered by `outside-the-claim` before the
  tracker is asked. Identical at the base commit, so not a regression; recorded because *this* change
  is what makes it matter, and re-measured because for one head it was **false** — the suffix ladder
  concatenated `.claude/claude` out of its own rungs and answered `Boundary` for `~/.claude`, so this
  paragraph declared open a gap that was not open, in the spelling it writes here and in no other. A
  reviewer measured it; a fixture holds the sentence now, by moving thirteen files inside those directories to `Boundary` while the
  directory around them stays open. It is not the declared hard-link limit — it is an ordinary path a
  shell writes. The shape that would close it already works next door: `~/.config/opencode` and
  `~/.estigia` are `Boundary` because those entries name the directory.

  **There are two readings of the same text, and the difference is deliberate.** The write road hands
  `is_control_surface` a `file_path`, which is a path; the shell and payload roads hand
  `text_names_a_control_surface` a command line or a patch body, which is prose with a path in it.
  Only the second folds punctuation, because only the second can have punctuation *next to* a path
  that is not part of it. A reviewer measured what that leaves and then measured whether it matters:
  on the write road a decorated relative spelling — `".claude/settings.json"` with the quotes inside
  the field, or with a leading space, tab, `~`, `$`, `%`, `{`, backtick or `=` — is `Routine` at this
  head and was `Boundary` at the base. A draft of this paragraph used `.claude/CLAUDE.md` as the
  worked example, which cannot be true of it: that file was not on the control surface at the base at
  all — it is the file issue 26 was filed about, and the fragment that reaches it arrived in this
  branch's first commit. A reviewer caught the example contradicting the sentence around it. It is not a hole, and that was checked rather than assumed: writing
  each of those spellings on this machine either fails outright or resolves to a different,
  non-existent directory. Only the bare spelling reaches the file, and the bare spelling is gated. The
  same reviewer confirmed it end to end — the quoted `file_path` costs zero tracker reads while the
  bare one, the quoted absolute, the shell road and a patch body naming it each cost exactly one.

  What is true and worth a reader's attention is that `punctuation_does_not_hide_a_control_surface`
  drives only the shell road. The write road has no punctuation fixture, because there is no
  punctuation behaviour there to hold — which is a fine reason and an easy one to forget.

  Two more `Routine` at both commits, each found by a different reviewer and neither closed here.
  `<repo>/.opencode/plugins` is not reached: `.opencode/agents/` was added for the repository-local
  definition root, and the plugin directory beside it has no entry, because `opencode/plugins/` is
  anchored and `.opencode` is not it. Estigia reads nothing from a project-local plugin directory, so
  it is not the same class as the definition root — but it is the same *shape* as the hole this change
  closed one directory over, and it is named here so the next reader does not have to find it twice.
  And `curl "-o<path>"` with the value attached **and quoted** is not seen as a write at all: the gap
  is upstream of `surface_of`, in `shell::writes_a_file`'s reading of the `curl -o` idiom, which this
  change does not touch — `src/harness/shell.rs` is byte-identical between the two commits. `wget
  "-O<path>"` is a `Boundary` in the same spelling, which is what makes the asymmetry visible.

  Still `Routine`, and not closed here: a project's own `AGENTS.md` or `CLAUDE.md` at its root,
  `.cursorrules`, `.clinerules`, `.windsurfrules`, `.github/copilot-instructions.md` and `.mcp.json`.
  They carry the same always-loaded authority, and they are **inside** a checkout, so they stay
  measured against the claim rather than standing aside — which is why they are a smaller thing than
  what this issue closed, not the same thing. `harness::roles::definition_for` reads its **five**
  definition roots from a hand-spelled list that nothing crosses against the gate. All five answer
  `Boundary` today and a sixth would not — but that sentence said *four*, and said it while one of
  them was `Routine` on both roads. `<repo>/.opencode/agents` is searched beside `<repo>/.claude/agents`
  in a single `vec!`, and the two were gated by different fragments: `.claude/agents/` reached the
  first, `opencode/agents/` is anchored on the left so `.opencode` is not it, and nothing reached the
  second. A definition that is not found is `Ok(None)`, which `declared_policy` reads as *every tool
  allowed*, so the file that writes an agent its own allowlist was the one riding the renewal window.
  A reviewer measured it; the fixture in this repository had gone further and asserted the hole,
  listing that root among the ordinary paths a directory entry must not gate. `.opencode/agents/` is
  a `CONTROL_SURFACE` entry now, `every_definition_root_is_a_boundary_on_both_roads` crosses all five,
  and one of them — `~/.claude/agents` — is additionally `paths.agents_root`, reached by the walk.

  What is still not crossed is the **list itself**: adding a sixth root to `definition_for` reddens
  nothing. The fixture spells the five by hand, exactly as the roots are spelled by hand.

  `gh`'s hosts file would have belonged on that list and is not on it. It decides which account
  every tracker call acts as, so it is named in `CONTROL_SURFACE` instead — which is what the issue
  asked for: *if one can reach tracker state, it is that path that needs naming rather than the whole
  class*. Both spellings, and through the shell as well as the write tool, which the first attempt
  got wrong by using a fragment with a space in it. (An earlier draft of this paragraph said the file
  "was on that list until this change", which was a claim about a previous shipped state that never
  existed — the whole entry is new. A reviewer checked it against `2477de0` and it was not there.)

  The hole predates this: the instruction files and the two `~/.claude` paths were never `Boundary`,
  so before issue 2 they were measured against the claim and allowed under a valid one. What was newly lost is the refusal when the claim
  is *not* valid — which is what issue 26 closed for them, and this paragraph is kept because the
  ordering it goes on to describe is what makes any of it hold. It is no longer lost when no contract is installed **outside the
  renewal window**: the answer is given after the `control-surface-not-installed` refusal, and
  `an_unreadable_control_surface_refuses_even_a_write_outside_the_claim` is what stops that drifting
  back. One published head had it the other way round and both reviewers of that head raised the
  cost, so it moved rather than staying here as a declared limit.

  Inside the window it is still lost, and that is issue #29 rather than this entry: the window's
  `Allow` sits above the contract refusal, so for its duration a routine write is permitted with no
  `SKILL.md` on disk at all — measured on both roads, and the guard above passes only because its
  fixture is outside the window. An earlier draft of this paragraph said the rule now applies
  *without an exception*, which was an absolute the code does not hold; a reviewer measured it by
  adding one `mark_verified()` line to that fixture.

  What was still lost when this entry first said so is closed, and the entry above says how. Three
  drafts of this paragraph described the state before the fix that shipped in the same commit — the
  last of them naming two `CONTROL_SURFACE` entries that the same diff had already replaced. Four
  reviewers found it, one after another, which is the measure of how easily a paragraph written
  before the code survives the code.

  And one boundary this cannot cross at all: a **hard link** outside the checkout pointing at a
  file inside it. Measured — `mklink /H <outside>/alias.rs <repo>/src/kept.rs`, classified
  outside, and writing through the alias rewrote the file in the checkout; `ln` does the same on
  POSIX. A hard link has no path to resolve to, so no amount of placing finds it.
  `is_control_surface` declares that same limit for its own matcher, and `writes_outside_the_claim`'s
  own doc comment now carries it too. A Linux `mount --bind` is the same shape and equally out of
  reach. Two more that path resolution cannot answer: the classification is taken before the write,
  so a link created in between is not seen, and a write tool that creates missing parents can make a
  directory inside the checkout on the way to a target that is honestly outside it.

  `gh`'s hosts file is named at its default location only. `GH_CONFIG_DIR` and `XDG_CONFIG_HOME` move
  it, and those spellings answer `Routine` — measured at `/home/me/xdg/gh/hosts.yml` and at a
  `gh-config` directory under a Windows profile. Narrowing the entry from the directory to the file
  also left `config.yml` `Routine`, which is deliberate: it holds the default host and protocol, not
  the credential that decides which account acts.

  Three limits of the stand-aside itself, none closed here.
  A landing that is not on a drive cannot be compared to a checkout that is, so `placed` declines it
  and the caller reads that as *inside*. That is what shuts the administrative-share hole, and it
  also means the whole feature is **off** on UNC ground: measured, a claim over `C:\...\repo` gates
  a write to `\\192.168.1.10\pikaflix\notes\scratch.md` rather than standing aside, and a run whose
  checkout is itself `\\192.168.1.10\pikaflix\repo` stands aside for nothing at all. An operator
  whose profile is redirected to a share, or whose checkout lives on one, gets the field failure
  this issue exists to end. Closing it means deciding what a share is the same *place* as, which is
  the question that made declining the honest answer in the first place.

  The second limit is not over-gating but measurement: the guard has none on a machine that does
  not serve its drives as shares. Widening its allow-list to accept UNC reddens both share fixtures
  here; on a runner without `\\localhost\C$` it would redden nothing and they would report pass
  having asserted nothing. A machine-independent companion was written, measured passing against
  that same widened allow-list, and deleted rather than kept — the guard only fires on a landing
  `canonicalize` actually produced, and producing one takes a real share. There is a second silent
  road on the newer of the two fixtures: it also passes when `mklink /D` is refused for want of
  Developer Mode or elevation. Measured by sabotaging the flag with the guard deleted — it printed
  its skip line and reported pass, while the older fixture still went red, so a machine that will
  not link loses the landing-versus-spelling measurement and not the guard's existence.

  The third is latent and could not be measured at all. The guard requires the landing to carry a
  drive prefix rather than rejecting the prefixes it knows, so a landing with no prefix component
  would be refused instead of compared as though it were relative. Nothing `canonicalize` produced
  on this machine has that shape and two reviewers went looking, so the branch is written for its
  failure direction and is held by no test.

  Separately from those three, and about the review rather than the change: **the POSIX half of
  this is measured by nobody who reads it.** The `..` handling in `placed` is where the two
  platforms genuinely disagree — Windows collapses the segment in the spelling, POSIX resolves the
  link first — and the arm that says so is a `cfg!(unix)` *expression*, so it is compiled on this
  desk and never runs here. The CI lanes are the only place it runs, and CI does not start on a
  topic branch: it starts when `release_ci` marks the pull request ready, which is after every
  review has been obtained.

  What that costs, measured on this change:
  `a_write_that_lands_inside_the_claim_is_gated_however_it_is_spelled` wrote through
  `<root>/decoy/../repo/src/main.rs` without creating `decoy`. Windows does not need it to exist;
  POSIX answers `ENOENT`. Seven published heads of this branch carried it, every one of them
  reviewed and none of the reviews able to run it, and it went red on both POSIX lanes the first
  time CI saw the branch — after the last verdict was recorded and the target released.

  The protocol did not lack the means, it lacked the question. Two reviewers of the fix reached the
  same failure from this same desk in minutes, by extracting `placed` into one file and running it
  under the WSL `rustc` that is already installed; one reproduced the `ENOENT` byte for byte and
  also on a symlinked root, which is the shape macOS gives every temporary directory. Nothing in
  `skill/SKILL.md` or `skill/references/` mentions a platform at all, so nothing asks for it. An
  earlier draft of this entry said the arm *cannot be compiled* here and that nothing in the
  protocol *could have* caught it. Both were measured false by the reviewers of the very head that
  claimed them — the first with a deliberate type error inside the arm, which failed the build on
  Windows.

  And the lanes measure less than "the lanes" suggests: `cargo test` is fail-fast across targets, so
  when the lib target failed on Linux and macOS the integration targets never ran there at all —
  `tests/pipe.rs`, which is where this change's end-to-end evidence lives, never executed on POSIX on
  this branch. That is issue #30.

  What is **not** true, and was written here for one commit: that those sixteen tests had never run
  on any runner. They had. A bare `cargo test` builds examples — cargo says so, and
  `cargo test --all-features --no-run` in a cold tree at this base leaves a runnable
  `fake_process.exe`. The measurement that produced the wrong claim was taken with
  `cargo test --test pipe`, a **filtered** invocation that builds no other target and appears nowhere
  in the workflow. A reviewer caught it before it shipped. The exposure is real and narrower: a
  filtered run in a cold tree, which is exactly how every mutation measurement in this document is
  taken. Issue #22 removed the skip from the rig's type; `cargo clippy --all-targets` remains no help
  there, leaving a `.d` and a zero-byte `.rmeta`.

- **A skip spelled as a pass, in the places issue #22 did not reach.** That issue took the skip out of
  `tracker_rig`'s *type*, so no caller of it can be handed a value meaning *did not run*. Two other
  fixtures still answer one, and neither was measured before.

  `repository()` in `src/harness/guard/tests.rs` is the exact twin — `-> Option<tempfile::TempDir>`,
  with twelve call sites: ten early returns and two that skip the body instead. Measured with `git`
  off `PATH` and a POSIX shell still present: the guard module answers `28 passed; 1 failed`, with
  six `SKIPPED:` lines that are only visible under `--nocapture`. A normal run shows six plain `ok`.
  (A first draft of this said thirteen and seven. Thirteen counted the definition line; the seventh
  skip was `no POSIX shell here`, which appeared only because the stripped `PATH` used to take git
  away had removed bash as well — and that one is a legitimate skip by the test below, not one of
  these.) Its subject is the **push guard** — the one gate no agent can
  route around, because it sits under git rather than under an agent, and the one `estigia doctor`
  currently reports as not installed on the operator's machine.

  Seven more skip sites sit in `tests/pipe.rs` itself, across five test functions, at the
  read-only-file capability and at git usability. They are the same shape and were not touched,
  because whether they should raise or be `#[ignore]` is a decision rather than a mechanical edit:
  `#[ignore]` is reported as *ignored* and a `return;` after an `eprintln!` is reported as *passed*.
  One of the seven is worse than the others and is counted here for the first time:
  `a_row_that_is_broken_comes_out_of_the_report_broken` prints its skip and **does not return** — it
  drops two assertions and carries on to report `ok`, which is the same failure in a smaller dose and
  invisible even to a reader looking for early returns.

  The line between this and a legitimate skip is whether the machine could have run it. Symlink
  privilege, `mklink /D`, administrative shares and drive letters cannot be conjured, and the ten
  skip sites in `src/harness/tests.rs` that answer to those are correct. (Sites, not functions, on
  both sides of that comparison — a reviewer found the two numbers here were counted in different
  units, which is how a count stops being checkable.) `git` is not that: this is a git workflow harness, and a
  machine without git cannot measure the push guard **at all**, so `ok` claims something never
  checked. Issue #22 already decided this for the tracker rig, where git absence is now a hard
  failure, so the crate currently holds two opposite policies for one condition — set in the same
  change. Filed as its own issue.

  **The guard that holds the rig catches an accident. It does not catch an author, and this is the
  measurement of how far short it falls.** Reviewers walked past
  `the_tracker_rig_cannot_answer_that_it_did_not_run` twenty-three times — eleven of those routes are
  held now and twelve are not, which is the last column below. That split is counted from the table
  rather than remembered: every version of this paragraph that carried a number from memory carried
  a wrong one. Each attempt was measured with the
  fixture removed — except those about *where* the fixture is looked for, which are measured with it
  present, since their point is that the rig looks in the wrong place rather than that it is missing.
  One row is unchanged applied alone and only bites paired with another, and its outcome column says
  so; a reviewer found that reading `no result line` there and measuring something else.
  Where the outcome is `106 passed`, that is issue 22's defect reproduced whole with every gate in
  this repository green.

  | Route | Outcome | Held now |
  |---|---|---|
  | `type TrackerRigMaybe = Option<TrackerRig>` in the signature | 106 passed | yes |
  | a caller split over two lines | 106 passed | yes |
  | `if … { return; }` before the call | 106 passed | yes |
  | the fixture located from `CARGO_MANIFEST_DIR` again | whole suite green | yes |
  | `return Default::default();` | 106 passed | yes |
  | `std::process::exit(0)` in the rig | **no result line at all**, exit 0 | yes |
  | a decoy comment *naming* the rig | unchanged alone; pairs with an exit | yes |
  | a `//` comment carrying the **whole signature line** | whole suite green | yes |
  | a `/* */` block putting the signature at column zero | 106 passed | yes |
  | `process::exit` in a caller, or in a helper below the rig | no result line, exit 0 | yes |
  | `use std::process::exit as leave;` | no result line, exit 0 | yes |
  | `use std::process as sys;` then `sys::exit(0)` | no result line, exit 0 | **no** |
  | `use std::process::{exit as leave};` | no result line, exit 0 | **no** |
  | a decoy `current_exe()` call, the real lookup in a helper | whole suite green | **no** |
  | a macro expanding to `return`, defined before the first `#[test]` | 106 passed | **no** |
  | `'rig: { … break 'rig; }` around each body | 106 passed | **no** |
  | `#[ignore]` on all sixteen | 90 passed, 16 ignored | **no** |
  | `#[should_panic]` on all sixteen | 106 passed — the rig's own panic is swallowed | **no** |
  | `#[cfg(feature = "rig-tests")]` on all sixteen | 90 passed, **0 ignored** — no trace at all | **no** |
  | a block decoy plus the real definition written `pub fn` | whole suite green | **no** |
  | a `}` at column zero inside a string, truncating the body | whole suite green | **no** |
  | a second `tests/*.rs` with its own `Option` rig | its own suite green | **no** |
  | a body skip — `if built { … } else { eprintln!(…) }`, no return | 106 passed | **no** |

  Two of those were fixed because the guard was not reading what it claimed to read: the body
  extraction bound to the first text in the file matching `fn tracker_rig()`, which a two-line comment
  could claim, and the `process::exit` refusal was scoped to the rig's body when its sentence meant
  the suite. Both fixes are **narrower than the first draft of this table said**, and a reviewer
  measured the difference. The decoy took three rounds to close and each round narrowed it rather
  than ending it: from *any text matching `fn tracker_rig()`* to *the whole signature line* to — now —
  the definition itself. The last step is what a check should have done first: there must be exactly
  one line the *filter* reads as a definition, and the body runs from that line rather than from the
  first substring that matches it. A reviewer had put the signature inside a `/* */` block at column
  zero, where a comment does not begin with `//`, and taken the signature check itself. All three
  comment shapes are red now — **against a definition the filter recognises**, which is the load in
  that sentence. The filter is `trim_start().starts_with("fn tracker_rig(")`, so a real definition
  written `pub fn` is invisible to it and a decoy becomes the unique match: measured, with
  `pub struct TrackerRig` to keep `-D warnings` quiet, the whole suite is green and so is the guard,
  while the fixture is looked for under the manifest again. The body's end is the first line that
  trims to `}`, which a multi-line string literal can supply from column zero — also measured, also
  green, also `cargo fmt` clean. Both restore a route this table calls held, and both are one more
  needle away from being caught; that is the reason neither is chased. Recognising a Rust item is
  parsing it. And the file-wide
  `process::exit` scan catches `use std::process::exit as leave;`, because that line contains the
  substring, while `use std::process as sys;` and `use std::process::{exit as leave};` do not. Rows of
  this table said the opposite of each, every time in the direction that flattered the guard, which is
  the direction to be suspicious of. The rest are declared rather than chased. A seventh substring would not close them: a
  macro puts the refused word in a region the scan never visits, a labelled `break` is a return under
  another name, and a body skip never leaves the function at all. Telling any of them from correct
  code needs to know which statements assert, which is a semantic question and not a textual one.

  **What actually holds the accidental path is the compiler.** `let Some(rig) = tracker_rig()` does
  not compile against a non-`Option` — measured, `error[E0308]: mismatched types` — so nobody
  reintroduces this by copying a neighbour, which is how all sixteen callers came to have it. The
  guard adds four things a single careless edit could do and the compiler would not see: reverting the
  signature, moving where the fixture is looked for, ending the process instead of failing, and
  writing `return` in a test that reaches the rig. It holds those against a decoy definition too —
  which it did not until a reviewer took the signature check with a block comment — but only while the
  real definition is spelled the way its filter reads. An earlier draft of this paragraph said three and
  left out the `process::exit` refusal — which is the check that actually catches four of the rows
  above, so the omission understated the guard in the paragraph somebody reads to learn what it
  holds. That is its honest scope. An earlier version
  of this entry said six routes were closed and two things remained unseen; both numbers were wrong
  when they were written, and the sentence claiming completeness is the reason they were worth
  writing down.

  The `return` scan reads whole lines, string literals included, and that is a decision made twice.
  A version of it cut strings out first, to spare an assertion message with the word in it. A reviewer
  measured what that cost: the stripper toggled on any `"`, a char literal among them, so
  `let _sep = '"'; if !built { return; }` hid a plain `return;` and put 106 tests back to passing on
  nothing — a false negative on the one spelling this guard exists to catch, introduced by the change
  meant to make it kinder. It also guarded a hazard that cannot arise where it runs: this reads
  `tests/pipe.rs`, and the two shipped messages saying *"did not return"* are in `src/tui/models.rs`,
  which it never opens.

  So the scan is back to whole lines, and its two false positives are declared rather than engineered
  around. A `return` in a string literal reddens it, and so does one inside a **closure**, which
  cannot skip the test at all. Both measured. The failure message names the offending line, says both
  cases are known, and says what to do — move the line into a helper outside the test, or rephrase the
  message — because an earlier version sent the reader to an entry about closures while they were
  holding a string.

  One more control-surface path of the hosts file's shape, found by a reviewer of this change and
  not closed here:
  `<checkout>/.git/config`, which answers `Routine` while `<checkout>/.git/hooks/pre-push` answers
  `Boundary`. `core.hooksPath` in that file decides whether the push guard runs at all. In the
  ordinary shape it is harmless because `covered()` yields both `repo_dir` and the worktree, so the
  base checkout's `.git` is inside the claim — but measured with a pointer carrying only a worktree
  (`repo_dir = <project>/worktrees/wt-a`), writes to `<project>/src/x.rs` **and** to
  `<project>/.git/config` both stand aside. Whether the fix is naming the file or widening what a
  worktree run covers is the question, and it is not this issue's.

  Narrower since `start_branch` began recording the claim's checkout beside the worktree, which is
  the entry further down about the gate covering two directories: Estigia's own tools no longer
  *produce* a pointer carrying only a worktree. The residual is unchanged for a pointer that carries
  one anyway — a run whose `repo_dir` genuinely is a worktree, or a hand-written record — so the
  entry is narrowed rather than removed, and the question it ends on is still open.

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
  measure both directions: every markdown file the payload links to is shipped, and every shipped
  runtime companion is reachable from another shipped file through a link or its canonical routed
  identity. The sole exception is `references/migration-inventory.md`, a historical migration
  ledger, and the guard fails if that exception grows or becomes stale. These prose-to-prose checks
  cannot prove that an agent opened or obeyed any document.
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

  One `git merge` is preparation rather than delivery: an exact literal `git merge --ff-only
  <target>` (also accepting `--` before the target) may run from `in-progress` after the tracker
  agrees when local Git proves all of it. The checkout is on a branch, its upstream is a canonical
  `refs/remotes/*` ref, the worktree is clean, the target is that upstream's exact short or full
  canonical name, or a full object ID of the length this repository's object format requires, and
  ancestry runs from `HEAD` through the target to the upstream. An object ID must resolve to itself
  as a commit; an annotated tag's ID does not qualify merely because it peels to one. Every proof Git
  process removes every inherited `GIT_*` variable, and the exception is denied when any such
  variable was present at entry. It is also denied when `BASH_ENV`, `ENV`, or Bash's exported `git`
  function name (`BASH_FUNC_git%%`) is present, compared without case so Windows environment names
  cannot evade the check. Those variables can make the shell execute a different `git` from the
  direct process the proof resolved; `PATH` remains eligible because both resolve through the same
  inherited path when no shell-only steering is present. The `GIT_*` check prevents the named
  repository, object-store, index, shallow-file and configuration steering, while the three shell
  checks prevent those specific initialization and inherited-function paths from substituting a
  different command. It remains a `Boundary`, so it still pays the live tracker read and is recorded
  in the decision ledger. The
  verdict-to-bytes check runs first; a stale verdict is refused before this exception can skip only
  the `out-of-phase` check, and only when the tracker answered exactly `in-progress`. `analysis`,
  `ready` and `blocked` keep the ordinary refusal. Wrappers, compound commands, duplicate command
  aliases, quoting, expansion, alternate git directories, extra flags or targets, detached branches,
  dirty trees and any unreadable Git answer retain the existing refusal.

  **The proof is local and can therefore be stale.** It never fetches and cannot establish that the
  remote-tracking ref still matches the server. It proves only that the merge is safe relative to
  the tracking ref already present in this checkout; a later fetch may reveal commits it did not
  know about. Avoiding network access keeps the gate from changing repository state or turning every
  local update into another remote boundary, and this limitation is recorded rather than hidden.

  **The proof and the merge are not one atomic operation.** Attachment, upstream, cleanliness,
  object resolution and ancestry are separate local Git processes, and the shell resolves and runs
  the merge only after the gate returns. Another process can move a ref, change the worktree, or
  replace what `PATH` resolves between any two of those reads, or between the final read and
  execution. `--ff-only` still prevents a merge commit, but Estigia takes no repository or process
  resolution lock and cannot claim the executed command saw the exact state or executable it proved.
  The decision ledger has the same pre-existing resolution limit: its allow line names the
  subject as `git merge` and the live claim, not whether this local proof or an ordinary reviewed
  delivery admitted it. Changing that shared allow vocabulary would reach every gate consumer, so
  this narrow exception records the residual ambiguity here instead.
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
  `Edit|Write|MultiEdit|NotebookEdit|Update|Bash|Agent|Task` for Claude Code. `Agent` and `Task` first
  take the dedicated reserved-reviewer prelaunch path rather than repository classification. A tool outside that set is
  never offered to it. Measured against the list this crate itself cites as the case it exists for,
  a published `builder` sub-agent declaring `Read, Write, Edit, Glob, Grep, Bash`: three of the six are tools the
  gate can judge, and `Read`, `Glob` and `Grep` are never seen. What it can therefore refuse is a
  tool **in the matcher and not in the list** — `MultiEdit`, `NotebookEdit`, `Update` for that
  builder — and not `WebFetch` or `WebSearch`, which are among those an operator narrowing a
  sub-agent usually means. The module says it *makes the author's policy true*; it makes the part
  of it that overlaps the gate true. Widening the matcher would close it and is the one thing the
  matcher exists to avoid — waking this process for every `Read` and every `Grep` is a cost paid
  thousands of times to answer "not mine" — so this is stated rather than fixed.
- **The verdict's binding is checked at delivery only for what this run published through Estigia's
  own tools, with an earlier exact-receipt check at CI release.** `publish_review` records a fresh
  epoch over PR/head/base/digest while the PR is confirmed draft. `release_ci` re-verifies the live
  `review` claim, globally latest receipt across runs, latest distinct accepted verdict marker,
  current draft PR and coherent clean target, then marks ready and reads back every outcome. The
  gate asks for one accepted aggregate verdict; it does not enforce the blind policies' panel size,
  concurrency, independence, blindness, same-finding identity or quorum, and it cannot tell one
  context from two or five. An identical-byte republish still invalidates old evidence because it creates a new
  epoch. GitHub has no atomic conditional-ready operation, so an out-of-band ready or push can bypass
  this cooperative order.
  The local delivery gate persists the complete epoch/PR/head/base/digest receipt and still checks
  the published head at the boundary that spends it: a `git merge`, `gh pr merge`, `git tag` or
  `gh release` from a run whose recorded review head is not the invoking checkout's head is refused as
  `verdict-bound-to-other-bytes`, naming the path inspected and the head found there. The invoking
  checkout is the covered directory the gate already selected and verified for this call; it is not
  inferred again from the run pointer, whose worktree can be absent after a handoff. For a delivery
  `gh pr merge` boundary only, a linked sibling sharing the pointer checkout's Git common directory
  can reach that verification. The classifier first retains one positive numeric PR only from one
  literal `gh pr merge <number> ...`; malformed, omitted, URL, branch, long, attached and bundled
  uppercase-`R` foreign-repository options, duplicate, compound and shell-evaluated targets remain
  irreversible boundaries but cannot select sibling evidence. Candidate
  pointers are then filtered by complete `receipt.pr == command PR`, exactly one holder must remain,
  and only then is the invoking HEAD compared with that holder's receipt and its live tracker claim
  verified. Equal HEADs never select between PR lineages. A legacy `reviewed_head` pointer remains
  readable after upgrade but cannot qualify PR-targeted sibling delivery because it carries no PR,
  epoch, base or digest. Ambiguous candidates and unreadable pointers select no holder, including for
  ledger attribution; the recorded holder comes from the same adjudication as the decision rather
  than a later directory scan. An
  unrelated or unreadable checkout named with that run still refuses
  `verdict-bound-to-other-bytes`; a sessionless call in an unrelated clone has no local holder to
  adjudicate and remains outside. Successful publish and republish effects first invalidate prior
  complete and legacy local authority, then retain output only when all five receipt fields are
  complete. Their refusals also invalidate that authority when the transport says a write committed
  or cannot exclude one; a conclusive pre-write or read-only refusal preserves it. Verdict and
  CI-release effects restore a supplied complete receipt atomically into a reviewer's pointer after a
  handoff removed the publisher's pointer.
  `git push` and `gh pr create` are deliberately not gated on it, because pushing after a review is
  how a run fixes what the review found and the answer to a moved head is *re-publish*, not *stop*.
  What it does not see: a run that bypasses the tools and changes GitHub directly. The gate's own question to the tracker is still
  `verify-claim --issue --run-id --expect-state`; the head comparison is local, against that invoking
  checkout.
- **A `republish_review` that refuses has already written to the pull request, and names which
  writes.** Its refusals arrive after the reused pull request has been edited — the renewal stands
  immediately before the push, and the lease is evaluated by git at the push itself — so a remote
  somebody else moved leaves the branch untouched, which is what the operation is for, and leaves the
  pull request altered. **Every** refusal site downstream of a write carries that fact, so none answers
  *nothing was written* — the draft readback, the failed edit, the renewal and the push. Counted rather
  than named, that sentence said *three* and a later change made it four without moving it; the number
  is the part that had no reason to be there.
  Each had reached the agent through the same `stop()` and `?` every other path uses, whose envelopes
  carry no `world`, so *the absence of a claim about the world was read as a claim that the world was
  untouched*.
  **Which** writes is the part that took four rounds. The report was built from one boolean and named
  two writes: an edit, and a conversion back to draft. `ensure_draft` un-readies only a pull request
  that was *ready*, and at republish time the reused one is normally already draft — `publish_review`
  drafts it and only `release_ci` makes it ready — so the common path claimed a conversion nobody
  performed. An operator putting that back runs `gh pr ready`, which exposes the branch to CI: the
  exact outcome `ensure_draft` exists to prevent. The report now names each write separately and only
  when it happened, which is the same rule this document applies to everything else — say what was
  measured, not what usually accompanies it. What is still on the reader is the decision: Estigia
  reports the write, it does not undo it.
- **The timeline does not record that a publication was a republish.** The answer `republish_review`
  returns carries `republished` and the head it leased against, but the `published` marker it writes to
  the issue is byte-identical to an ordinary publication's. So an incident review reading the timeline
  — which is the only record that outlives the call — sees a new epoch and cannot tell whether the
  bytes arrived by fast-forward or over a force-push. Naming it in the marker would change what every
  consumer of that marker parses, so it is stated here rather than claimed.
- **A republish cannot lease against a receipt published from a different GitHub account.**
  `latest_publication` keeps only comments the authenticated identity authored, because a marker this
  identity did not write is one anybody could have forged. The consequence is narrow and worth naming
  rather than leaving to be discovered: a run that reclaims an abandoned issue published from another
  account finds no receipt, is refused `published-receipt-missing`, and then has no route inside
  Estigia at all — `publish_review` cannot fast-forward a rewritten branch either. It fails closed,
  which is the right direction, and it leaves that reclaim with nothing to do but step outside the
  tools, which is the thing this operation exists to stop.
  What the lease does **not** prove is that the rewritten history still contains the reviewed change.
  It compares one commit id to another; a rebase that dropped a hunk leases exactly as cleanly as one
  that did not. Every republish creates a new epoch and invalidates the prior review evidence for
  that reason, and the answer to *is this still the change that was approved* is a fresh verdict
  against the new receipt, never the lease.
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
  `write`, `patch`, `multiedit`, `notebookedit`, `update` and `bash` — named rather than `*`, because
  waking a process for every read would be a cost paid thousands of times to answer "not mine".
  `notebookedit` and `update` were missing from this sentence for as long as it has existed, which is
  the shape of drift this file is most exposed to: the list is crossed against the classifier by
  `the_plugin_gates_the_tools_the_classifier_judges`, and prose naming the same list is crossed by
  nothing.
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

  *Two* is now what the isolation step writes, rather than what it assumes it will find. It used to
  record only the worktree, which is correct whenever the claim already recorded a checkout and
  wrong in the one shape nobody has to do anything strange to reach: a `claim` whose tracker write
  lands and whose readback fails returns before the pointer effect runs, so no checkout is recorded
  at all. From there the dispatch guard's own precondition — *a run that has claimed nothing has
  nothing to be outside of* — lets `start_branch` through, and the worktree it writes becomes the
  run's **only** covered directory. The server that made the call is then outside its own run, every
  later call is refused `run-id-names-another-checkout`, and repeating it cannot help: an MCP
  server's directory does not change when a child command uses another one. The only way out
  observed in the field was restarting the agent from a path the workflow had just created, which
  loses the live context and can mint a different runtime identity — the thing claim attribution
  exists to prevent.

  So `start_branch` fills the claim's checkout in as well as the worktree, and fills rather than
  overwrites: a run whose claim named checkout A keeps A when a server standing in B calls under its
  id, because refusing B for that run is exactly what the guard is for. It is not coverage
  manufactured from a client's path — `start_branch` verifies the claim against the tracker from
  that same directory before it creates anything. Measured by
  `one_server_survives_the_isolation_it_created`, which drives one real server through both calls
  down one pipe, because no per-call `GateContext` can pose *the same server asking twice* — and
  which reads the pointer **between** the two, because the renewal below fills the same field by its
  own route. Asserting only at the end measured the pair and not the halves: with the check at the
  end, removing the isolation's line left that test green while the unit test beside it reddened.
  Two independent reviewers of this change found that, separately, by deleting the line and running
  the suite.

  **And a run already in that state gets itself back**, which prevention does not do for the runs
  that are in it. Two things were needed. The refusal above measured `covered().count()`, which is
  the right question asked of the wrong field: the worktree is the *additional* directory a claim
  covers, never the one that says where it was sworn. So a record holding only a worktree could
  ground a refusal — and an incomplete record is not a narrower claim, it is an unknown one. It
  measures `repo_dir` now, so a record that never named its checkout stands aside exactly as a run
  that has claimed nothing does. Nothing is widened for a record that *does* name one: a foreign run
  id, an unrecorded worktree and a directory outside the coverage are refused as before, and
  standing aside is not clearance — the call still goes to the tracker, which is the only thing that
  adjudicates.

  Then the way back is a call the contract already requires. A renewal answered `ok` is the tracker
  saying, at that moment, that this run is the live holder of this issue in this state — the same
  fact `Swear` writes, from the same authority — so the pointer is completed from it rather than
  left broken until something writes to the tracker again. It takes no tracker **write**, which is
  what makes it reachable during exactly the outage that causes the damage: the state is produced by
  a `claim` whose write lands and whose readback fails, and during that outage there is no write to
  be had. Filled and never overwritten, and unable to invent authority, because a run that does not
  hold the issue is refused before the pointer is touched. Measured by
  `a_renewal_completes_a_record_the_tracker_has_just_agreed_with` and, as a process,
  `a_stranded_run_recovers_from_the_checkout_it_is_running_in` — which asserts both halves, since a
  run readmitted on an empty record is a run whose writes are still measured against nothing.

  **What the renewal's fill accepts on no directory evidence.** `start_branch` at least performs its
  tracker read from the directory it then records; `verify_claim` performs the same read and neither
  read binds a directory at all, so the renewal takes the caller's working directory as coverage
  because it is the only one on offer. Run ids are public — every claim comment carries one — so a
  call under run A's id arriving from directory B, while `A.repo_dir` is unset, writes B into A's
  record permanently, and A's own server is then refused in its real checkout. That is this defect
  in mirror image, and it is not closed: what bounds it is that the shape it needs is the one the
  isolation fix stops producing, and that the alternative — not filling on renewal — is the stranding
  itself. Deleting the pointer is the only correction. Both reviewers raised it and neither blocked
  on it.

  **And the same rule is now spelled two ways.** The dispatch guard asks `repo_dir.is_some()`; the
  write gate in `src/harness/mod.rs` still asks `covered().count() > 0` about the same question. The
  semantic argued for above — *an incomplete record is unknown rather than narrow* — applies verbatim
  to the second, where the direction of failure is the opposite: a worktree-only record makes writes
  in the base checkout read as `Outside` and go ungated rather than refused. Not a regression, and
  not made worse here, but this repository's own rule is that a fact written twice is a fact that
  will disagree with itself, and these two now do. Unifying them is a change to the write gate and
  belongs to its own issue.

  What this does **not** do is repair the claim itself. The `claim` that failed this way cannot be
  retried: its operation id is reused only when the pointer already names the issue, which is the
  field the failed call did not write, so every retry mints a fresh key and the transport answers
  `already-owned-by-different-operation` for as long as the claim is live. `release` with the exact
  epoch and a fresh `claim` is still the only way to re-swear, and both are tracker writes. That is
  a different defect with a different fix and it is filed separately.
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

  All eleven rows that **can** break are now forced through the binary, on states any machine can be
  put into: no skill; a contract taken from under a registered agent; two installed roots whose
  machine-wide rows disagree; a gate and a tool server whose
  settings name a binary that is not there; a checkout with no remote; a `pre-push` hook that is not
  text; an unreadable stand-down; an unreadable run pointer; a ledger line saying a call went through
  ungated; and, for the one row that is about the machine rather than the installation, a search path
  with no `gh` on it. The twelfth, `transport`, has no broken state: it answers `ok`, or `skipped`
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
- **`doctor` checks twelve things, not everything.** Skill, transport, `gh`
  authentication, a git remote, this repository's push guard, the contract each configured agent
  reads, whether the root the gate decides in carries the rows those agents read, whether the gate
  each of them registers would actually run, whether the tool server each
  of them registers would actually start, whether the operator has the gate standing down right
  now, whether every run pointer on the machine can still say what it holds, and whether any call
  has reached that gate and gone undecided. It does not check the tracker's labels, the board, or
  whether the repository it found is the one the issues live in.
- **A row about this machine can be made to differ per agent, and nothing refuses it.**
  `config set --agent <slug>` refuses a `Scope::Everywhere` row and does not refuse a
  `Scope::Machine` one, so `estigia config set --agent claude-code "Summary language" Spanish`
  is accepted and reports success — on an adapter with a skill root of its own, where no read-back
  catches it. **And once two roots disagree about one, no command makes them agree**: the plain
  `config set` writes a machine row into the canonical contract alone, and the per-agent form cannot
  hold one in a shared root, where `render_some_agent_rows` drops it and the command exits on its own
  read-back. So `doctor`'s `canonical` row reports that divergence and names no way out, which is the
  honest shape rather than a satisfying one. Measured on 2026-08-17 and 2026-08-18, on
  `Summary language` and `Issue body language`, by four independent reviews of the check itself, each
  running the named command verbatim. Filed as issue #62; which half is wrong — refuse the per-agent
  write, or propagate the plain one — is not decided here, and nothing guesses at it.
- **One of the twelve is about the past.** A call the gate cannot decide on — a payload it cannot
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
