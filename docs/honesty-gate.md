# What the gate covers and narrows

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the gate covers, narrows and records, and the refusals it names. The index says which area holds which question.

## What the gate covers

- **The gate covers two directories: the checkout the claim was made in, and the isolated one
  the claim created or a resumed run adopted.** A run that makes its own worktree by hand (one git
  has never registered, or whose ownership marker names no issue) gets that write gated but the directory
  is never recorded, so subsequent writes there read as `Outside`. Use the tool, or resume through a
  checkout the transport already knows about.

  `claim` with `branch` and `base` records the claim's checkout as well as the worktree, filling and never overwriting. A
  `claim` whose tracker write lands and whose readback fails returns before the pointer effect runs, so no
  checkout is recorded and the worktree would become the run's **only** covered directory; the server is
  then outside its own run and every later call is refused `run-id-names-another-checkout`
  (`one_server_survives_the_isolation_it_created` drives one real server through both calls and reads the
  pointer **between** them). The dispatch guard measures `repo_dir`, not `covered().count()`, so a record
  that never named its checkout stands aside like a run that has claimed nothing (standing aside is not
  clearance), and a renewal answered `ok` completes such a record with no tracker **write** and no
  invented authority (`a_renewal_completes_a_record_the_tracker_has_just_agreed_with`,
  `a_stranded_run_recovers_from_the_checkout_it_is_running_in`).

  **The same renewal discovers a checkout this run never built.** `claim`, `reclaim` and every renewal read
  every checkout `git worktree list` has registered, find the one whose ownership marker names this issue,
  and fill `worktree` from it (`a_renewal_records_the_checkout_a_resumed_run_adopted`,
  `a_reclaim_records_the_checkout_it_takes_over`,
  `a_write_inside_an_adopted_checkout_is_gated_once_a_renewal_records_it`). It records nothing for **more
  than one checkout naming the same issue**, nor for **a read that fails**; the scan's `?` stops at the
  first unreadable marker, which disables discovery for every issue and run this clone holds until that
  marker reads again, without turning a successful renewal into a refusal. It cannot see a checkout git
  never registered or whose marker was never written in the shape `read_ownership` reads.

  **The renewal's fill accepts the caller's working directory on no directory evidence.** `verify_claim`'s
  tracker read binds no directory, and run ids are public, so a call under run A's id from directory B,
  while `A.repo_dir` is unset, writes B into A's record permanently (A's own server is then refused in its
  real checkout), and `discover_adopted_worktree` plants a `worktree` the same way from B's clone. Not
  closed: the shape needed is the one the isolation fix stops producing, not filling on renewal is the
  stranding itself, and deleting the pointer is the only correction. **The same rule is spelled two
  ways**: the dispatch guard asks `repo_dir.is_some()`, the write gate in `src/harness/mod.rs` asks
  `covered().count() > 0`, so a worktree-only record makes writes in the base checkout read as `Outside`
  and go ungated; unifying them is a change to the write gate that belongs to its own issue.
- **A branch differing from a registered one only by case or Unicode normalization is refused, on a
  filesystem that distinguishes them too.** `worktree_path` composes `<branch>~<run-id>`, so `Fix-1-a` and
  `fix-1-a` under one run ID are two strings and one directory on default APFS and NTFS, and the registry
  (keyed by `normalise_path`, which folds neither) answered `worktree-path-occupied` with
  `occupied_by_branch: null`, the orphan diagnosis whose action invited removing the first run's live
  checkout. `may_occupy` compares the **spellings** (separators unified, NFC-normalized, lowercased,
  normalized again) and refuses a collision `case-insensitive-branch-collision`, naming both branches. The
  fold ignores the filesystem on purpose (the conservative direction). The case half is
  `str::to_lowercase`, not full Unicode case folding, so a pair the volume merges but `to_lowercase` does
  not (`Σ`/`ς`, `µ`/`μ`, `ſ`/`s`) is not refused. Held by
  `two_branches_differing_only_by_case_are_refused_rather_than_called_an_orphan`,
  `two_branches_differing_only_by_normalization_are_refused_rather_than_called_an_orphan` and
  `a_case_only_difference_is_the_collision_and_a_separator_one_is_not`.
- **The closed-window refusal covers the transport's own spawns, and the gate reads an agent's line by the same
  matcher.** While the git operation window is closed, `window::tripwire` refuses a `gh` write or `git push`
  that `transport::run` is asked to start (`outbox-bypass`), and a `gh` line an agent runs itself is
  classified through `window::is_outbound_write` as an `Action::Remote` and denied
  `git-operation-window-denied` (`gh pr merge`/`create`, `gh release create`/`edit`, `git merge`, `git tag`,
  `gh issue create`/`develop`, a mutating `gh api`, `gh cache delete`, `gh ssh-key`/`gpg-key` and `gh repo
  deploy-key`/`autolink` writes: `the_gh_writes_outside_the_old_verb_table_are_remote_and_denied_while_closed`).
  The matcher is a table of the write verbs it knows, so a `gh` subcommand a later release adds reads as a
  non-write until named; it does not read an alias, a script, a shell variable or substitution that spells the
  call, or a `gh` word that is not first in its command, and a `gh api graphql` whose query comes from
  `--input` or an `@file` field is treated as a write while a mutation behind a shell variable passes. While
  closed, `create`, `comment`, `transition`, `claim`, `claim --takeover` and `unassign` queue
  their GitHub writes, a publication is refused with the number of writes waiting, and `release_ci` is
  refused (`release_ci_denies_outside_the_period_before_reading_anything_else`).
- **The window's scope is the repository a write is aimed at, and the gate's reading of an agent's own
  shell line still belongs to the checkout it runs in.** The transport resolves the pin `Context::repo` to
  a checkout whose `origin` names it (the caller's own, else one among `known_repositories`) and reads that
  checkout's rows; a repository with no such checkout is closed rather than inheriting the caller's
  schedule. The gate classifies a hand-written `gh` line through
  `harness::operation_window::check_protected`, which reads `GateContext.operation_window` built from the
  **caller's** checkout, so a shell `gh issue create -R <destination>` is judged by the caller's schedule;
  only the transport's own `create` and the flush moved. A destination whose checkout is not registered, or
  whose `origin` is not a GitHub remote, reads as no rows and closes.
- **The store holds a write queue and one permitted rewrite of an entry.** Schema 9 adds `outbox` and
  `provisional_issues`, and the `entries_no_update` trigger lets exactly one update through, moving an
  entry's `issue` from a provisional number to the real number `provisional_issues` records
  (`an_entry_may_move_from_a_provisional_issue_to_its_recorded_real_number_and_nowhere_else`). Provisional
  numbers start at `PROVISIONAL_BASE` from one counter in `store_meta`, so two machines can allocate the
  same number and file the same issue twice. `Store::rekey` moves a provisional issue's rows in one
  transaction and `outbox::rekey_files` follows with the run pointers and the ownership record; a branch or
  worktree directory named with the provisional number keeps that name. A claim whose flush issues the
  real number writes that number and the checkout `claim` started into the run pointer, read from the
  transport's answer rather than from the provisional argument the call was made with. A provisional
  issue opens its ledger without any GitHub call, and an unallocated or unrecorded number is refused.
  A provisional number is read from this machine's store alone: `list_state` with `issue`, and the gate's
  `provisional-issue-number` answer to `gh issue view|comment|edit|close|reopen` or a GraphQL `issue(number:)`
  naming one. The match is on the command line's words, so a number reached through a variable or a script
  goes to GitHub and reads as not found, and a number another machine allocated reads as never allocated
  here. `-R` naming another repository than the allocation's is left to GitHub. The tests are
  `provisional_read` in `src/harness/` and the offline cases in `src/transport/outbox/`.
- **The window's wake is told, not held, on two hosts.** `opens_at` is computed from this machine's
  local zone; under a test clock the offset is held fixed, and off Unix no daylight-saving change is
  read. In the refusal it is text, not a field. Claude Code's loop only tells the turn to schedule its
  own wake and lets the second stop for one opening through, so a turn that schedules nothing is not
  woken. Codex's `Stop` keeps no loop record, so it is told on every stop. Pi's and OpenCode's loops hold
  the timer themselves (`windowWait`), and lose it when the host process ends; they wait at the first
  closed-window stop even while another issue is claimable, where Claude Code's and Codex's `Stop` keep
  the turn going and name `park: true`. The tests are
  `next_opening` in `src/harness/operation_window/tests.rs`, the closed-window cases in
  `src/cli/tests.rs`, and the `window` scenarios of both loop drivers in `src/setup/tests.rs`.
- **A parked claim is the pointer's note, and nothing renews it.** `claim` with `park: true` keeps
  several claims under one run id, so the store's adjudication is untouched. Only the active one is
  covered and verified. Nothing checks that a parked issue's work is built, only that the window
  was closed when it was parked. A parked claim's horizon passes unrenewed, and once it has lapsed
  another run may `reclaim` it without `force`. A release puts down only the claim it names: the
  parked list outlives the active claim, a release naming an issue the run does not hold changes nothing, and the pointer goes once nothing is left on it. A pointer that still lists parked claims is kept when storing it fails; no test injects that failure. `SessionEnd` forgets the pointer and the parked list
  with it, as it does a single claim, so after that a parked issue is found again only from the
  store. The tests are `park_tests` in `src/harness/mcp/tests/`, plus the prune and rekey cases that
  name a parked claim.
- **The transition record is the store's own, and it is not a log of everything that moved a state.** Schema 16
  adds `transitions` (`at`, `run_id`, `from_state`, `to_state` per issue), append-only like `entries`, and
  `Ledger::set_state` writes the row in the same transaction as the state, so a history that cannot be written
  leaves the state unchanged and the transition fails. Every state change a command makes goes through
  `set_state` (`transition`, `transition --to` on `claim`, the merged-release close, the begin-review step, and the
  flush's reconverge), so none bypasses it; setting the state an issue already has records nothing, which
  makes the flush's re-projection invisible. What it cannot see: a transition names no caller, so `run_id` is
  whoever holds the issue at `at` and is empty when nobody does; the state an import reads from the labels is a starting point, not a transition, and states set before
  schema 16 have no row, and
  the first change after it records `from_state` as the state the store held; a label edited on GitHub by hand
  never reaches the store, and another machine's changes are another machine's store. Nothing reads the
  record yet.
- **The flush sends the queue in one line, and a stalled write holds everything behind it.**
  `outbox::flush::run` refuses while the window is closed, takes a per-repository lease in `store_meta`
  (a pid and an expiry; a dead or expired holder is taken over), rekeys pointers and ownership files, and
  sends unsent writes oldest first, marking each `sending` before and `sent` only after its own readback (a
  comment by its exact text written by this account — nothing is appended to the body, so an identical
  comment the same account already posted on that issue also reads as landed —, a create by `gh issue view` on the number printed,
  a link by the Development sidebar, a reconverge by `transition`'s readbacks). A write whose outcome cannot
  be read stays `sending` and stops the flush with `outbox-pending`; a failure that learned nothing on a first
  attempt goes back to `queued`, a write that has been `sending` never does. The line is global: one ambiguous
  write stalls every write behind it, other issues' included. A `sending` create is resolved by searching for
  exactly its title, and that search is the index GitHub updates after the issue exists, so a flush seconds
  after an ambiguous create can find nothing and file a second issue; two issues with the title stop the
  flush. The search adopts only an issue the authenticated user (`--author @me`) created within five minutes
  before the row was enqueued whose number the store does not hold (the bound is the enqueue time, not the
  first attempt, so a same-user same-title issue filed after enqueue is still adopted, and an open-window
  `pending-create` mark has no row time and is bound by author and store alone). A reconverge rebuilds labels,
  board and owner from the store, overwriting changes made on GitHub meanwhile; the comment readback lists
  the thread without a `since` filter (a thread of a thousand comments or more is refused as unread). Writes
  give up after `WRITE_SECONDS`, inside `LEASE_SECONDS`, and a flusher renews the lease before it sends and
  before it settles a row. **Open:** the renewal and the `gh` call are not one step, so a flusher that loses
  the lease in that instant can send once more, and a write that times out has an unknown outcome resolved by
  the next flush's readback, which for a create is the lagging search.
- **The queue is sent before anything else writes, and a detached flush is the machine's, not the
  session's.** With the window open and a write waiting, `comment`, `create`, `transition`, `claim`,
  `unassign`, `start-branch`, `publish-review`, `republish-review`, `release-ci`, `draft-pr` and
  `ensure-states` run the flush first and are refused `outbox-pending` when it stops; other operations do
  not. The list is written in `outbox::flush`, not derived from what each operation spawns, so a new
  operation that writes to GitHub is unflushed until added. With nothing waiting the check opens the store
  read-only and asks GitHub nothing. `estigia flush --detach` reports the pid it started, not whether the
  flush then succeeded; the lease keeps it from running beside another.
- **"Offline" means the window is closed, not that the network is down.** Reads still go to GitHub while
  closed. `list_state` answers from the local store and lists issues created while closed as `provisional`;
  another machine, a person and the board see none of it until the flush, so labels, board column, assignee
  and comments disagree with the store for as long as writes are queued, and a second machine can file the
  same issue under the same provisional number. A task whose work is a commit cannot be marked done while
  closed (the window denies the commit); one closed by an `evidence` note can. `SessionStart` and `Stop` count
  unsent writes of every repository from a read-only open and say so (`Stop` as a `systemMessage`, never
  blocking), and with the window open they start `estigia flush` detached and report that it was started, not
  that it succeeded.
- **One row is read by nobody.** `Delivery preset` stores nothing of its own and only writes the rows it
  stands for. `State names` is read by `assist`, `draft_pr`, `ready_pr` and `review_stages`; `Corrections after ready`
  by `draft_pr`; `Ready by` by `draft_pr` and `ready_pr`; `Self review` by `draft_pr` and `ready_pr`; `External reviewers` by `review_stages`; `Merge by` by
  `review_stages` and `deliver`, which hands back at `authorisation` when it is not `agent`; all through
  `Context::layered`. `READ_BY_NOBODY` in `tests/honesty.rs` lists `Delivery preset` and fails when it is
  read and not taken off the list.
  - **`review_stages` reads, and decides nothing.** Its answer is a reading of the pull request at the
    moment of the call; nothing is recorded, so a run that does not ask again acts on a stale one. A check
    stage's comments are its author's **unresolved review threads** (a comment outside a thread, or a
    summary review, is not read, and a thread resolved without addressing it counts as addressed). For
    `any` and `org/team` a review from any account other than the author, a `[bot]` or a check stage's
    author satisfies the stage; team membership is not read. An approval completes a review stage only
    for the current head; GitHub does not expose the historical base that reviewer read. Exact-head
    polling, full thread conversations, guarded reply/resolution and their non-atomic mutation limit are
    in [assisted review](assisted-review.md). A list with a next page (more than a hundred threads,
    comments per thread, reviews, requests or check runs) refuses the reading. A blocking check not
    reported on the head holds the stages behind it and says it was never reported, since a check not yet
    started and a stage name matching no check look the same; an advisory check never holds them. Held
    against a fake `gh` by `src/transport/stages/tests.rs`.
  - **The autonomous road proves less about the checkout than the assisted one, and leaves part of the
    shell open.** `review_stages` and `review_thread` under an autonomous claim check the run's complete
    receipt, the ownership marker and branch of its checkout, the live claim (`review_thread` only) and
    that the single open pull request is the receipt's ([assisted review](assisted-review.md)). They
    do not prove that nobody pushed between the last read and the mutation, that the receipt's
    pull request is the one the person meant, or that the reply says what the fix did. The pointer
    records no branch there, so the marker is the only record of it, and a marker the run itself
    could rewrite is not evidence against that run. A shell `gh api` call the gate already reads as a
    write is refused under a live autonomous claim (`review-thread-api-mutation-unverified`, after the
    claim verifies) when it is a GraphQL write any word of which names `resolveReviewThread`,
    `unresolveReviewThread`, `addPullRequestReviewThreadReply` or `addPullRequestReviewComment`, a
    mutating call on `pulls/<n>/comments`, `pulls/<n>/comments/<id>/replies` or
    `pulls/comments/<id>`, or a GraphQL call whose query is not on the line (`--input`, an `@file`
    field). The line is split on whitespace with its quoting lost, so the name is looked for in every
    word and one written inside another field's value refuses too. Every other API write is decided as
    before, and a read stays a read. Still open: a mutation
    spelled through a shell variable, substitution, alias or script, which the gate does not read, and
    `gh pr comment` and `gh pr review`. Held by `src/harness/mcp/tests/autonomous_review_tests.rs`,
    `src/transport/stages/tests/autonomous.rs`, `src/transport/window/tests.rs` and
    `src/harness/tests.rs`.
- **No other setting is read by nobody.**
  An operator sets thirty-four rows and `config list` reports all of them. Measured: `context.get` is
  called for the labels in `READ_BY_THE_TRANSPORT` — `worktree location` and `Review budget` (for the review forecast line). The gate reads
  `Irreversible commands`, and `Git operation window` is read from the repository's own file by the transport and the gate alike, never through `context.get`; and the payload's prose names the rest of the acted-on rows. The four rows that
  had exactly one answer (`Delivery route`, `Review delegation`, `Merge strategy`, `Review protocol`) were
  retired and hard-coded, and the guard fails on any row the gate, the transport and the prose all leave
  unread (`every_setting_is_read_by_the_gate_the_transport_or_the_prose`). `Delivery authorisation` and
  `Merge by` are read in code (`deliver` hands back `authorisation` unless they are `auto` and `agent`), and
  a merge made by hand through the shell is held only by the prose. `Model routing` is closed by a narrower
  claim than its name suggests: `setup` reads it for the `model:` and `effort:` lines of the definitions it
  writes, and the managed block of the **installed** `SKILL.md` carries a generated sentence telling the
  agent to honour the row (`selected_documents` builds it and `configuration_body` splices it, so it is in
  no file in this repository). Five of the eight supported adapters share the neutral skill root
  (`adapters_on_the_neutral_root()` computes it; `docs/what-it-writes.md` is held to the same number), so
  they share one installed contract and one rendered sentence, which names Claude Code as the one adapter
  that receives the definitions `setup` installs. Naming a role in `Model routing` installs its definition
  and removing the entry retracts it. Nothing here starts a model; what changed is that something reads
  the row.

## The populations and the prose they cross

- **Every population declaration is found by walking `src/`, resolved to the item its `subject` names,
  and fingerprinted together with it.** A declaration is a `pub const … : Population` (the type in
  `src/harness/population.rs`, each const beside the code it describes), read by `tests/guards.rs`
  with `syn`. The sources are read off the directory rather than listed, and a `subject` naming no real
  item in the same file, or more than one, is refused loudly. Changing the rule or the code beneath it
  reopens the claim: `tests/guards.rs` fails and names the new fingerprint, and somebody has to read the
  declaration again before recording it. That proves the pair has been read since it last moved, not
  that any declaration is true, and not that every guard needing one has one.

  **A list is a population; the matcher is a decision.** The declaration on `CONTROL_SURFACE`
  fingerprints the list, so changes to `is_anchored_fragment` once moved which paths the gate watched
  with this registry green. `CONTROL_SURFACE_CORPUS` is the matcher's subject: a fixed corpus of paths
  with the answer both roads must give each one, asserted by
  `the_control_surface_matcher_answers_its_corpus`. The corpus is representative rather than exhaustive.

  **Where the walk reaches, and what catches the rest.** `collect_found` reads top-level items,
  associated consts inside `impl` blocks, and the bodies of inline `mod`s, but does **not** walk
  statements, so a `const … : Population` inside a function body is invisible to it.
  `no_population_declaration_hides_from_the_walk` counts the type ascription in each file's
  comment-stripped text and fails, naming the file, when the text writes more declarations than the walk
  found. **Two spellings still escape both ends**: the ascription both counters match is one constant,
  `harness::population::DECLARATION_ASCRIPTION`, a contiguous substring, so `: population::Population =`
  does not contain it, and a declaration behind a type alias (`type Pop = Population;`) is invisible to
  the counters *and* to `is_population_type`. Neither is a plausible accident and neither is closed.
- **The seam guards read two shapes, not every shape.** They resolve markdown links and `<skill>/`
  rooted command lines. A payload that names a file some third way would slip past both.
- **The directive's rules are crossed against the gate by name, not by reading.** The text every agent
  carries in context states three rules, and each is pinned to a named test that holds the gate to it:
  a rule deleted from the prose, or a test renamed out from under one, fails the build. What no test
  can check is whether the sentence still *means* what the test enforces; that reading is a person's
  job.
- **The transport and Estigia read the operator table the same way, and the crossing that held it is
  deleted.** Both read `SKILL.md` between the same markers (`estigia:config:*`, with the superseded
  `issue-flow:` pair still read), normalising a hand-written cell (backticked token, `**bold**`, an
  explanation after an em dash or in parentheses). Nothing now crosses the two readers over the same
  bytes; `a_row_the_binding_acts_on_is_a_row_this_reader_finds` holds one side.

## Reachability, registration and the rounds around them

- **The integration exemption proves *a* branch is checked out where the merge runs, not that it is the
  issue's own branch.** `is_integration_in_the_runs_own_checkout` checks `run.worktree` alone, an
  unredirected environment and any `refs/heads/*` symbolic ref. `claim` returns a branch but the
  MCP pointer effect retains only worktree; the branch remains in `ownership_path`'s private git file.
  `checkout`/`switch` are not observed. Git prevents checking out a branch held by another worktree,
  not trunk after the base moves away, `develop`, `release/1.x` or another feature branch. No guard
  distinguishes these from the issue's branch. A direct transport call also bypasses pointer effects.

  `integration_environment_does_not_redirect_the_repository` refuses `GIT_DIR`, `GIT_WORK_TREE`,
  `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_INDEX_FILE`,
  `GIT_NAMESPACE`, `GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM`, `GIT_CONFIG`,
  `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_COUNT`,
  `GIT_CONFIG_PARAMETERS`, `GIT_SHALLOW_FILE`, `GIT_REPLACE_REF_BASE`, `BASH_ENV`, `ENV` and
  `BASH_FUNC_git%%` by exact case-insensitive name, plus `GIT_CONFIG_KEY_*`/`GIT_CONFIG_VALUE_*`: a list,
  not an exhaustive proof. `GIT_EDITOR`, `GIT_PAGER` and `GIT_ASKPASS` are permitted. The config family
  can redirect worktrees or hooks. `is_safe_local_fast_forward` shares the list. The eventual merge
  runs after this process exits; repository/ref/worktree/PATH races remain
  (`local_fast_forward_proof_fails_closed_on_repository_state`,
  `an_exact_local_fast_forward_is_allowed_without_widening_other_merges`).

  **An integrating merge spends no verdict, and it is read in the directory it names.** The shapes are
  exactly `git merge [--no-edit|--no-ff|--ff|--quiet|--continue|--abort] [<ref>]`, alone, behind one leading
  `cd <dir> &&`, or as `git -C <dir> merge ...` with `<dir>` a plain path (optionally in one pair of quotes,
  not starting with `-` or `~`, or a drive-lettered `X:/` path; a relative one is taken from the session
  directory). It is allowed when the directory is inside `run.worktree`, the environment does not redirect
  Git and `HEAD` there is on a branch. The same line in another checkout, `--ff-only`, a second command, a
  redirect or an expansion, `git tag` and `gh pr merge` stay deliveries. A line that changes directory any
  other way (`cd $VAR`, a second `cd`, a subshell, `pushd`, `--git-dir`) is refused
  `review-checkout-unreadable` (class `unavailable`) once a review is recorded, for every git command in
  `DELIVERS`. **The run's own merge commit (#58)**: the prescribed `git tag -a "<tag>" -F "<notes-file>"
  "<merge-sha>"` and the `git merge --ff-only origin/main` that brings the base checkout to the delivered
  merge act on a commit that is not the reviewed head, so that commit is admitted when it is a two-parent
  merge whose second parent is the receipt's head and whose first parent descends from its base
  (`ReviewTransaction::delivered_by_merge`, shared with `match_merge`); a tag line is read only when plain
  words (`tag_target`) and a tag that names a commit is judged on that commit. Not covered: `git -C <dir>
  merge --ff-only ...`, `cd <dir> && git merge --ff-only ...`, a merge whose base the checkout cannot
  resolve (refused), and a squash or rebase merge (no second parent).

  **The near-miss diagnostic stops at a JSON string.** `integration-shape-not-exact` names the accepted
  `git -C <worktree> merge --no-edit origin/<base>` in `in-progress`; it never admits. But
  `shell_command_argument` sets `literal` only for one normalised command key holding a JSON string.
  An argv-array `git merge origin/main` still refuses `out-of-phase`. This is fail-closed; adapters
  sending arrays have not been measured.

- **Nothing counts the rounds a change takes, and nothing bounds one that is not converging.** The
  two-round limit is gone: `skill/policies/blind-judges.md` carries no count and no path reaches `blocked`
  because one ran out. The judgement triggers (the put-down row of `skill/references/decision-gates.md`: a
  known mistake repeated, three hypotheses without narrowing, more editing than measuring) are prose a run
  applies to itself. A claim's horizon plus the renewal window bounds a run that has stopped reporting, not
  one reporting diligently on its ninth publication. What stops an unconverging change is a person noticing.

- **ZCode registers its instruction file and its MCP server; three things remain unverified.**
  ZCode CLI 0.16.5 logged `mcp.server.connected` for `mcp.servers` in `~/.zcode/cli/config.json`, with no
  fallback file present. The shared skill and `~/.zcode/AGENTS.md` follow its configuration reference.
  Remaining limits:
  - The stated fallback loads `~/.agents/mcp.json` only when the scope has no servers. Adding Estigia
    could hide every fallback server while setup/status report success; this was not run.
  - `is_now_empty`/`discard` can delete the whole config after `strip_zcode_mcp` leaves only empty/null
    values, e.g. `{"hooks": {}}`. A nonempty `hooks.enabled` preserves it; unchanged files are untouched.
  - `~/.zcode/skills` is documented but unverified from shipped source, so the neutral root is used
    and that tree remains ungated. Hooks require global `hooks.enabled`; denial and payload are
    unverified, so no tool gate is installed (`gate_gap`).
  `every_control_file_an_adapter_has_is_one_the_gate_measures` crosses `.zcode/agents` and the MCP
  path; `a_host_read_root_carries_the_same_authority_as_the_directive` crosses the neutral fallback.

- **DeepSeek Harness registers, with its skill root and instruction target verified.** The adapter installs
  the skill into `~/.dsh/skills` (the harness's `user-dsh` root, from its `skill-filesystem` provider) and
  fences the directive into `~/.dsh/AGENTS.md`, both from the harness's own shipping source. `$DSH_HOME` is
  read and contained as `XDG_CONFIG_HOME` is (an absolute value under the call's home is honoured, one
  outside is dropped for `~/.dsh` with a per-adapter note). **No MCP registration** (where an operator
  registers a server has not been crossed against a running client, so `paths_in` answers `None`), **no
  tool-call gate** (whether a plugin can **deny** a call like OpenCode's `tool.execute.before`, and under what
  payload, has not been crossed against a shipped harness, so it is contract-only and nothing checks the
  agent honoured the directive) and **no role/sub-agent definitions** (`agents_root` stays `None`, yet the
  tree is gated by a **prefix**: the extension-trimmed fragment `.dsh/agents` reaches `~/.dsh/agents/**`
  only because it is right-unanchored, and narrowing it to `.dsh/agents.md` would drop the tree to
  `Routine`; the `~/.dsh/agents/reviewer.md` row in
  `a_host_read_root_carries_the_same_authority_as_the_directive` pins that). The fragments `.dsh/agents` and
  `.dsh/skills/` are crossed by `every_control_file_an_adapter_has_is_one_the_gate_measures`.

- **A workspace install covers the runtimes whose project-level files could be read from the runtime's own
  reference, and refuses the rest.** `setup --scope=workspace` writes each adapted runtime's files into a
  checkout (destinations: [`what-it-writes.md`](what-it-writes.md)); ZCode and DeepSeek Harness have no
  verified project-level destination, so a workspace install refuses them by name and a workspace
  *uninstall* of them (or a retired adapter) is a no-op
  (`a_workspace_install_needs_a_workspace_and_a_verified_adapter`,
  `a_workspace_refusal_names_the_command_that_clears_it`,
  `a_workspace_install_writes_under_the_workspace_and_nowhere_else`,
  `a_workspace_uninstall_takes_the_skill_back_from_one_shared_root`). **Not covered:** that each runtime reads
  every path in the table was read from documentation, not crossed against a host; Codex's project
  `hooks.json` is reported by an open upstream issue as not loaded where `.git` is a file (a git worktree);
  Cursor's project `hooks.json` has a stricter schema than the user-level file (a required top-level
  `version`), read from a community issue, so whether the workspace hook loads is untested; both
  project-level gates load only in a project the runtime trusts, which nothing reports. In a workspace the
  runtimes that read `AGENTS.md` share one instruction file and one skill root
  (`the_runtimes_that_share_an_instruction_file_share_its_skill_root`), so the shared directive and skill go
  with the **last** adapter the record names, and the block names whichever adapter wrote it last, so the
  per-agent override file it points at may belong to a runtime no longer installed (nothing asserts that
  file). A record is found by the digest of the checkout's canonical path, so a moved or renamed checkout is
  one no record names, and a fresh clone carrying an install in its files carries no record. The record is
  read, changed and written without a lock (two runs against one checkout can lose one adapter's name) and
  is written before the adapter's files (a failed `setup` leaves the adapter named). `is_configured` answers
  "not installed" for every adapter when the record cannot be read, the one reader that turns this unknown
  into an absence: the TUI uninstall reads `read_configured`, which keeps `Unknown` apart
  (`uninstall-not-read-back`), but the CLI `uninstall` output, `sync` and the status listings still turn an
  unreadable file into "not configured", and in the workspace scope `read_configured` takes an unreadable
  record as absent. `workspace_installed_adapters` answers `None` for a record it cannot read, an install
  that would merge into one refuses, and a record whose checkout is `NotFound` reads as gone. `setup --all
  --scope=workspace` was observed once to stop with a refusal naming the two adapters it cannot place after
  the five writable ones were done; no test drives that CLI path.

- **A global install is found through a record of its roots, and only installs made after the record
  existed are in it.** `forget_state` keeps the store while any recorded root, under
  `<home>/.estigia/roots`, still holds an adapter, and when a record cannot be read. An install by a build
  that did not write the record, under a `CLAUDE_CONFIG_DIR` other than the uninstall's, is invisible until
  `setup` runs again under that root. The record holds the Claude, XDG and `APPDATA` roots, not `CODEX_HOME`
  or any other root `Environment::resolve` does not read. The command line names the record on its own
  `create`/`remove` lines after a real run; `--dry-run` names neither. Held by
  `uninstalling_one_claude_root_keeps_the_store_the_other_root_reads`.

## What the gate narrows, and what it records

- **`observed` is attached on more roads than it is rendered on, and the register naming what is still
  missing has twice claimed more than the code does.** A gate refusal carries a structured
  `Refusal.observed` account of what the gate parsed: the command fragment matched and the checkout the
  call was adjudicated against. `gate_observing`/`decide_observing` receive the door's own reading and
  `prefixed` attaches it to every `Deny` they produce; `gate`/`decide` receive nothing (`decide` calls
  `decide_observing` with `&Observed::default()`), and `prefixed` emits `observed: None` rather than an
  empty object for an empty reading. `guard::adjudicate_action`'s single-holder arm is on this second
  road. `cli::show_gate`'s session-less branch passes the `Decision` through `report_gate`, whose `Deny`
  arm attaches the door's reading to any refusal that carries none, closing the gap for that caller
  under `--json` for every refusal `adjudicate_action` can produce there
  (`complete_review_receipt_not_selected`, `run-pointers-unreadable`,
  `several-runs-hold-this-checkout`, all via `Refusal::not_started`, which never sets `observed`).
  **Attached is not rendered.** `report` writes `Display`, which never mentions `observed`, unless `json` is
  `true`, and `src/setup/plugin.rs` invokes `${ESTIGIA} gate ${tool} --input ${payload}` without `--json`, so
  the one shipped agent that drives the session-less branch never sees the account. What remains open:
  - **The push road carries nothing.** `cli::git_hook`'s `PrePush` arm calls the same
    `adjudicate_action` through `guard::decide` but answers its `Decision::Deny` straight to `report`
    and never reaches `report_gate`, so every refusal it can answer with carries nothing, and nothing
    can top it up. Closing it needs two more readings: `session::holdings` drops the path it read once
    the run is built, and `guard::holders_of` discards every coverage depth but the winner's.
  - **`show_gate`'s own early failures carry nothing**: `gate-input-not-json` fires before `parsed`
    exists, and `working-directory-unknown` and `home-not-resolvable` can propagate out of
    `context_the_payload_names` before the `CheckoutObserved` half is built; each reaches `report` as a
    bare `Refusal::not_started`.
  - **The MCP road builds no account and adjudicates a checkout anyway**:
    `run-id-names-another-checkout` in `src/harness/mcp/mod.rs` compares `context.repo_dir` against
    `run.covered()` and answers through `Refusal::not_started` directly.
  - **The receipt-comparison half**: nothing reports what a delivery refusal actually compared a stated
    PR or head against (`delivery_pr_mismatch`, `stale_verdict` carry their own prose).
  - **No hook dialect renders it, and a role-refused `PreToolUse` never builds it.** The `PreToolUse`
    arm in `src/harness/hook.rs` builds the full account only for a call that clears
    `authorize_review_blind_launch`, the reviewer's own `roles::gate`, `roles::definition_for`'s `Err`
    arm and the named-agent `roles::gate` check; for a call that does reach the build, `Dialect::deny`
    emits only `deny_reason` prose for all five `Dialect` variants (`claude-code`, `gemini-cli`,
    `cursor`, `exit-code`, `cline`). Left open deliberately: a hook's answer belongs to the *host's*
    protocol, and a sibling structured key inside somebody else's schema is a decision with its own
    tradeoffs.

- **The clamp `narrowed_by_the_call` applies trusts one thing beyond `launched`: a pointer a claim
  already wrote.** `Worktree location` on a real install is a **sibling** of the repository it was
  minted from, never a child, so a call naming that worktree could never resolve through a sibling-blind
  clamp, and a run that had claimed, published and released CI could not deliver (`estigia gate`
  adjudicated `gh pr merge` against the launch checkout on `main` and answered
  `verdict-bound-to-other-bytes`). The invariant this repository runs on: several tasks per agent and
  several agents are fine, but a worktree holds exactly one task, so the worktree is the unit of task
  identity and `repo_dir` is not (several runs sharing one launch checkout is the *designed* state).
  `held_by_a_pointer` honours a directory when it lies inside `launched` **or** is a checkout a pointer
  records, drawn from `worktree` alone, so neither road reads `repo_dir`. A directory no pointer reaches
  is not admitted, and `checkout_root_covering` never climbs above `launched`
  (`checkout_root_covering_never_climbs_above_the_launch_directory`).

  With a run id (`--run-id`, a session id with `--runtime`, or ambient `ESTIGIA_RUN_ID`),
  `held_by_a_pointer` is restricted to that run's own `worktree`, so a call naming run B's worktree gains
  nothing and is decided against `launched` under the run id it carried. Without one, `named` goes through
  `harness::guard::holders_of`, restricted to a single holder whose own `worktree` covers it (never one whose
  `repo_dir` merely is an ancestor). What a run-id-less call cannot do is tell one caller from another, so it
  cannot refuse a call that *reaches into* another task's worktree: that flow violation is not enforced on
  this road, every call with no identity takes it, and `stale_verdict` reads `context.repo_dir`, so a
  delivery boundary the call takes is adjudicated against that other task's verdict binding. An operator who
  exports `ESTIGIA_RUN_ID` for a calling road that does not set it closes this without a code change; the one
  shipped adapter that invokes `estigia gate`, the OpenCode plugin, forwards `--session-id <session>
  --runtime opencode` and `gate_identity` derives the run id through `session::run_id`. The forwarded identity
  cannot establish that the claim was sworn under it: a claim sworn under a hand-minted id
  (`opencode-f2ae8c`) is not found by the derivation, the gate loads a fresh run and answers `outside`
  (permitted, no ledger line), so every action from such a run, delivery boundaries included, is permitted
  rather than judged. The derivation's pinning is prose, not enforcement.

  **Three readers of the resolved `repo_dir`, not one.** `launched_context` sets the field from
  `gate_context(&looking_at)` after the narrowing, so a model-written `workdir` or write target that a
  pointer records reaches all three:

  | Reader | What it decides from `repo_dir` | Where |
  |---|---|---|
  | Installed configuration | `tracker`, `boundaries`, `evidence`, `integration`, `window` | `gate_context` in `src/cli/mod.rs` — `skill::installed_config_in_keeping_what_parses(&skill_root, &repo_dir)` |
  | Role definition | which agent allowlist applies to the tool | the gate path in `src/cli/mod.rs` — `harness::roles::definition_for(&context.repo_dir, …)` |
  | Verdict binding | whether a delivery is bound to the bytes under review | `stale_verdict` in `src/harness/mod.rs` |

  **Cross-repository resolution is admitted when a pointer records the worktree.** A worktree path under
  `Worktree location` can belong to another clone on the same machine; if a live pointer names it,
  `held_by_a_pointer` honours it as it honours a sibling worktree, and nothing asks whether `repo_dir`'s
  git identity matches `launched`'s. That is the invariant as written for task identity, and it means
  the other repository's `.estigia/` rows and role definitions decide the call. No test drives a second
  repository's pointer through the identity-less road. **A corrupt pointer can remove a holder from
  consideration on that road**: `holders_of` walks every pointer through `session::holdings`, whose
  `pointer_at` answers `None` for a pointer that will not parse (unlike `session::load`, which loads it
  as `unreadable: true`), so a directory two holders cover at equal depth can answer down to the one whose
  pointer still parses. No test drives it. This is the `estigia gate` CLI door only; the `PreToolUse`
  hook road builds its `GateContext` from the payload's own `cwd` and reaches neither
  `narrowed_by_the_call` nor `checkout_root_covering`.

## Refusals, and the way out they name

- **A refusal that names no way out is measured at build time, and the 67 the guard reports are frozen
  as debt rather than fixed.** `build.rs`'s `assert_every_refusal_names_a_way_out` reads the same walk
  over `src/transport` that fills `TRANSPORT_VOCABULARY` and holds three rules that were held by review
  alone: *do not name a command unless running it clears the block* (`AGENTS.md`), *an unknown result is
  not clearance*, and the contract's *never reinterpret an unknown result as clearance*. The population
  is every distinct reason the walk extracts, because a reason is the operator-visible identity of a
  refusal: **224 at this head**. A reason is satisfied when its envelope carries an `action`
  (`reasons_naming_a_continuation`; **154** do), when `BY_DESIGN` declares a shape from
  `NO_CONTINUATION_SHAPES` (`operator-knowledge`, `world-action`, `human-authority`, which is
  `src/outcome.rs`'s own `NoCommandReason` vocabulary, crossed by
  `tests/guards.rs::the_refusal_guard_vocabulary_is_the_harness_own`; **3** are declared:
  `blank-argument`, `invalid-horizon`, `invalid-operation-id`, all `operator-knowledge`), or when
  `refusal-baseline.txt` freezes it (**67** do: the operation-conflict and metadata-mismatch families,
  every `*-readback-failed`, and the generic argument refusals `malformed-argument`,
  `missing-argument` and `unknown-operation` among them). A `BY_DESIGN` declaration for a reason whose
  envelope already carries an action fails the build. `refusal-baseline.txt` may only shrink: an entry
  that matches no unsatisfied refusal fails the build naming it, so *the refusal was fixed* and *the
  refusal moved into a shape the walk cannot see* are told apart, and the walk's floor is the count that
  file records (`population 224`), so a walk that finds fewer fails naming the count.

  **What the guard cannot do.** It reads the operator-facing text for the presence of an `action`; it
  cannot tell a correct continuation from one that parses, runs and does not help, and only a person
  hitting it can see that. It cannot tell a genuine `BY_DESIGN` from a fixable refusal someone declared
  to silence the guard, and it does not reach a refusal whose reason is built by a shape the extractors
  do not read (`assert_reasons_covered` closes that class, not this). It was measured by mutation: a
  refusal added with no action and no declaration fails naming it, a baseline line matching no refusal
  fails naming the line, and removing a refusal from the source fails the floor.

  Three earlier refusals broke the rule and were found by a person hitting them
  (`several-runs-hold-this-checkout` naming runs that no longer exist, a phantom pointer's refusal naming an
  `estigia release` that did not discharge it, `reviewer-definition-changed` naming a world-action for a body
  Estigia itself wrote).

- **The per-runtime capability table is canonical; it does not measure that any host obeys a capability
  it is declared to carry.** `src/setup/capabilities.rs` holds `canonical(InstructionFile) ->
  Capabilities`, a total `match` with no `_` arm, and every entry of `AGENTS` declares its own
  `Capabilities`. A declaration that disagrees with the table fails the build (a `const` assertion runs
  `capabilities_agree` over the registry: `error[E0080]`), the same predicate crosses the table against
  `gate_spec`'s events, matcher and dialect and `agent_definitions_root`, `AgentAdapter::checked`
  refuses a mismatched adapter at construction
  (`an_adapter_whose_declaration_disagrees_with_the_table_fails_at_construction`), and
  `capabilities_for_slug` is fail-closed with `Capabilities::NOTHING`
  (`a_runtime_absent_from_the_table_carries_nothing`). What it does **not** measure is whether a host
  refuses a tool call, reads a hook, or discovers a skill at the root the table names; every column is a
  claim derived from what each host ships, so a column can be wrong about a host with the build green.
  What it buys is that a disagreement **inside the crate** is a compile error or a construction refusal.
  One disagreement is recorded and not fixed: ZCode's row carries `gate: Gate::None` while it carries
  the contract and the MCP server, the gap the ZCode entry in
  [The gate's reach across the agents](honesty-gate-reach.md#the-gates-reach-across-the-agents) states. This narrows the reporting gap (one
  word covering four states in what a person reads) and does not close it.

- **The merge-commit topology rule admits a forward-integrated delivery, and no longer proves the first
  parent was exactly the reviewed base.** `skill/SKILL.md` step 6 and `skill/bindings/github.md`'s
  `verify_claim` exception require the delivered SHA's second parent to be exactly the reviewed head and
  the reviewed base to be an **ancestor** of its first parent; `delivery_close` computes it with `git
  merge-base --is-ancestor <receipt.base> <first parent>` in `context.repo_dir`, reusing
  `transport::target::is_ancestor` (exit 0 is an ancestor, exit 1 is not, any other exit is a `Read`
  failure, because an unknown answer is not clearance). A delivery GitHub merged after `main` moved is
  recognised even though its first parent is a descendant, and a first parent unrelated to the reviewed
  base is still refused: `a_forward_integrated_delivery_is_recognised_as_this_runs_own_close` and
  `a_delivery_whose_first_parent_is_unrelated_is_not_this_runs_own_close` in
  `transport::claim::close::tests` measure both against a real repository. Estigia never guaranteed the
  first parent *equals* the base; the repository's strict check was that guarantee.

- **A Linear source reads an issue, writes nothing but a mapped state, and its transport is the `linear`
  CLI.** `Tracker linear <team-key>` is admitted only under `Workflow: assisted`, answered in a repository's
  own file (`config set` and `config edit` refuse it without `--repo`, `config set --repo` where `Claim
  workflow` is not assisted, and `config set Workflow autonomous --repo` where `Tracker` is already Linear;
  `doctor` and run start refuse it). `assist` reads the issue's title and description with one constant
  GraphQL query (`a_read_spawns_linear_api_with_the_query_and_sends_no_mutation`); `where` reads a key's
  state and URL with `STANDING_QUERY`, a finished issue included; `STATES_QUERY` and
  `MOVE_MUTATION` are sent only through `move_issue`, which `follow_state` calls only for a state the
  repository's `State names` row maps and only for a Linear key
  (`follow_state_moves_only_a_mapped_state_of_a_linear_issue`). A move is read back, reported under
  `linear_state` and never retried; a read-back that cannot be completed after Linear accepted the move
  answers `linear-move-not-read-back` with `world: unknown`, never the not-done `linear-unreachable`
  (`an_accepted_move_whose_read_back_fails_is_an_unknown_world`), so a run that ignores a failed one leaves Linear behind the work. Every
  operation is `linear api --variables-json <variables> <operation>` through the crate's one process boundary
  with a 30-second deadline; the CLI holds the credential, so Estigia never reads or passes the key. What it
  does **not** do: it writes Linear only to move a mapped state, and does not prove a credential can be
  scoped read-only. It depends on the CLI's behaviour measured against version 1.10.0 (`api`,
  `--variables-json`, exit status 1 on a failed request, the text `No API key configured`), so a changed
  message would be reported `linear-unreachable`; the workspace is the CLI's choice, so a repository's
  `.linear.toml` is not guaranteed to be the one read; only the two reads ran against the live API, the move
  only against the stand-in; a key the credential cannot see and a key that names nothing both answer
  `linear-issue-unknown`; authentication and "not found" errors in a GraphQL `errors` array are **matched
  heuristically**; `assist` refuses a key whose team is not the configured `<team-key>`
  (`linear-team-mismatch`). Nothing crosses the Linear client against a second implementation: the stand-in
  `linear` is the `fake_process` example.

  **The unclaimed-write refusal and `Issue required` hold under `Tracker linear` with
  `Workflow: assisted`, as they do under GitHub.** Both ask `Tracker::adjudicates_unclaimed_writes`
  (true for a tracker a run can hold a claim against, which an assisted run does against Linear
  locally); Linear with workflow unset or autonomous stands down
  (`a_read_only_tracker_without_an_assisted_workflow_refuses_nothing`; `doctor` flags that
  configuration). Under `assisted` both refusals and the SessionStart text name `assist`, not `claim`,
  `create` or the checkout `claim` makes, which are refused there
  (`an_assisted_run_is_told_to_assist_not_to_claim`,
  `an_assisted_session_start_names_assist_and_not_the_refused_tools`). The tests build the gate context
  with `Tracker::Linear` directly, so the repository-file layer that produces it is read by its own
  tests, not crossed here. `estigia guard` and the pre-push hook say a read-only tracker without
  `assisted` adjudicates nothing.

## Which hook commands are Estigia's

A settings-file hook is recognised by its shape, `<program> hook <event>` (`runs_estigia_hook` in
`src/setup/render.rs`), not by a substring, so `slack-webhook stop` is not taken for the gate. What this
measures is the one regression test that writes such a command and runs `setup` then `uninstall` over it.
What it does not measure:

- **The program is not checked.** Renamed binaries and shims are accepted on purpose, so another tool
  whose command line is literally `<program> hook stop` is still read as Estigia's.
- **Only the first word is the program.** A wrapper in front of it (`env X=1 estigia hook stop`,
  `sh -c "…"`) is not recognised, so setup leaves it and writes its own entry beside it.
