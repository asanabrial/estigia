# The gate's boundary

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: where the gate's shell and write boundary stops reading. The index says which area holds which question.

## The gate's boundary

- **A redirect to `/dev/null`, `/dev/stdout`, `/dev/stderr` or `/dev/fd/<n>` is not read as a write.**
  `shell::redirects_into_a_file` matches the target word exactly, quotes allowed; a word that only begins
  with one of them (`/dev/nullx`, `/dev/null/x`) or a target it cannot parse stays a write. Not seen: a
  redirect through a variable or substitution (`> $sink`), or a `/dev/null` that is a regular file. Held by
  `a_redirect_to_a_stream_is_not_a_write_and_any_other_target_is`.
- **The redirect reader models the quote rules that hide a `>`, and fails closed where it cannot.** It ends
  a double-quoted region only at an unescaped `"`, reads `$'...'` with backslash escapes, treats an unquoted
  `#` at a word start as a comment (a `>` inside it is still a redirect: `echo x # > y` is a write), and
  skips a here-document body as data; a line that ends inside a quote after hiding a `>` is a write. Not
  modelled: a body `here_document` refuses (an unquoted delimiter with `$`, a backtick or a backslash in the
  body) is scanned as code, and `$"..."`, `${...}` and `$(...)` nesting are not tracked
  (`a_redirect_after_a_quote_the_scan_could_misread_is_still_a_write`,
  `a_quoted_arrow_stays_data_whichever_quote_holds_it`).

- **An unclaimed run is refused a commit or a push only where the push guard is installed, and only for the
  spellings the command line names.** `unclaimed-commit` is raised when no claim holds the checkout and
  `guard::state` answers `Installed` or `Chained` (running `estigia guard` is the opt-in), on any branch,
  since any unclaimed commit or push skips the flow. A guard reading `Unreadable`, `Inert`, `Foreign` or
  `Absent` stands aside (denying would lock every commit because of a hook file; `doctor` reports the first
  two as broken), as does a repository whose tracker cannot adjudicate an unsworn write, whose skill has no
  contract file, or a checkout with no remote for a GitHub tracker that names no repository
  (`unclaimed::applies`). `--abort`/`--quit`/`--skip` recovery and `git pull` are not refused;
  an alias, script, editor or `gh api` writing a commit passes; `git push --no-verify` skips the hook half; a
  **claimed** run is not held to its worktree (its claim covers the checkout it was made in too). Held by
  `a_commit_without_a_claim_is_refused_where_the_guard_is_installed`,
  `a_push_without_a_claim_is_refused_on_any_branch_where_the_guard_is_installed` and
  `the_pre_push_hook_refuses_an_unclaimed_push_where_the_guard_is_installed`; nothing drives the two shell
  entry points (`estigia gate` with no session, the dialect hook with an empty session id) end to end.
- **`Issue required` holds only what the gate can read as work reaching the default branch.** With the row
  `on`, a run holding no claim is refused (`issue-required`), in a checkout whose tracker can name an issue
  there and that has an installed contract, a `git commit`, `git cherry-pick`, `git revert` or `git am`
  while the checkout's current branch is the default branch, a `git merge` there, a push whose destination
  is the default branch, and a `gh pr merge`. The default branch is what `refs/remotes/origin/HEAD` names,
  else a branch called `main` or `master`; a detached `HEAD` is not it. Every other modification passes, placed
  or not: a file edit, a redirect, `sed -i`, inline interpreter code, `git stash`, `git rm`, branch and worktree
  commands, a commit on another branch, `git pull`, `gh pr create`, a push to another branch and a GitHub
  write that is no boundary. Nothing records or compares a checkout's `git status` afterwards, and a stored
  `required`, `watch` or `ask` reads as `on`. What is unknown is held, for that set only: a current branch
  that cannot be read, a `cd` or `git -C` directory the line does not spell as a literal absolute path, a push
  destination the line does not establish (`--all`, `--mirror`, a variable, a glob). Held by
  `what_puts_work_on_the_default_branch_is_held_without_a_claim`,
  `everything_else_passes_without_a_claim_on_either_branch`,
  `a_push_destination_the_line_does_not_establish_is_held`,
  `a_failed_current_branch_read_is_held_under_a_commit_or_a_merge`,
  `a_modification_is_never_held_for_being_unplaceable` and
  `a_stored_watch_reads_as_on_and_refuses_an_unclaimed_commit_on_the_default_branch`. It was narrowed to
  this from holding almost every local write after one operator's store (2026-10-20 to 2026-11-19) showed
  most refusals were modifications that integrated nothing and the operator turned the row off rather than
  keep it. What it does not hold:
  - **The branch is read when the gate looks, not when the line runs.** `git checkout main && git commit`
    from a side branch runs unasked, and `git checkout -b topic && git commit` from the default branch is
    refused though it commits on `topic`. A commit made by a script, an alias, a `make` target, an editor
    or `python3 -c` that runs git passes, as does a push git itself runs where `estigia guard` installed no
    `pre-push` hook. A push destination spelled as a variable, a quote or a glob in the refspec is held as an
    unestablished one; a destination that comes from git's configuration is not read, so `push.default =
    matching` (every branch the remote shares, with no refspec on the line) passes.
    `git pull` and `git rebase` can put commits on the default branch from the checkout and pass, and `git
    merge --abort` read through the boundary list is held like a merge.
  - **`gh pr merge`, `git merge` on the default branch and the boundaries below are held under `on`, and
    `Merge by` is not read on this path.** An unclaimed `git tag`, release, `gh api` mutation or declared
    irreversible command is refused under `on`, and so are `gh pr merge` and a `git merge` on the default
    branch, through `issue_requirement::decision` from both `decide_observing` and `adjudicate_pushing`.
    No row loosens it: a repository file still carrying the removed `Unclaimed GitHub writes` row is read
    with that row ignored. **What this costs:** a person's own `git tag` in a session that holds no claim
    is refused; a skill that reviews and then merges meets one refusal per merge; and a pull request
    approved at the exact head does not clear it, because the skills that need this road do not take it. A
    command the fragment lists do not name is not found, and a GraphQL mutation behind a shell variable
    still passes. A GitHub write that is no boundary (`gh pr review`, `gh pr comment`) is never held. A
    call launched outside every repository is left alone, and the git operation window's own refusals are
    unchanged. Held by `an_unclaimed_github_write_is_refused_under_required`,
    `an_unclaimed_github_write_passes_when_the_policy_is_off`,
    `a_github_write_that_is_no_integration_passes_under_every_value`,
    `a_github_read_passes_under_every_value`, `an_unclaimed_github_write_is_held_through_the_gate_command_too`,
    `a_tag_and_a_release_are_held_off_the_default_branch_too` and
    `a_leftover_unclaimed_github_writes_row_does_not_let_an_unclaimed_merge_through`.
  - **An assisted run cannot carry a GitHub write past the claim beside an admitted boundary.** `git pull;
    gh pr review 3 --approve` and a declared boundary followed by a GitHub write are refused
    `assisted-remote-write-unverified`, checked only after `assisted-does-not-deliver`,
    `assisted-publishes-through-draft-pr` and `assisted-api-mutation-unverified` have answered, so a merge
    still names the delivery. Held by `an_admitted_boundary_does_not_carry_a_remote_write_past_the_claim`
    and `the_specific_assisted_refusals_answer_before_the_remote_write`. The write is found by the same
    fragment lists as one alone.
  - **A GitHub tracker that names no repository infers one from the remote, so zero remotes is a tracker
    that cannot name an issue.** A write landing only in such repositories is not held (no ask, no deny) and
    the SessionStart text says there is nothing to claim. The read is `git remote` in the repository each
    target lands in, not the launch directory, so a no-remote checkout writing into a nested checkout that
    has one is still held. A failed read or a `.git` entry git will not use is not zero remotes. A
    non-GitHub remote still counts, because the URL is not read. Open: a refusal that survives there (an
    unplaceable git write under `on`, a nested target the tracker can name) still names `claim`/`create`,
    which cannot run from the no-remote checkout. Held by `a_checkout_with_no_remote_is_not_held_for_an_issue`,
    `adding_a_remote_holds_the_same_write_again`,
    `a_named_repository_holds_the_write_where_the_checkout_has_no_remote`,
    `a_write_into_a_nested_checkout_that_has_a_remote_is_still_held`,
    `a_failed_remote_read_is_not_zero_remotes` and `a_guarded_checkout_with_no_remote_refuses_no_commit`.
  - **A write the hook does not intercept is not held** (a tool no matcher wakes for, a script or formatter
    whose shell line names no git fragment, another clone), and **only a routine write, a boundary and a GitHub write are
    held**: a push is held as a commit is (`an_unclaimed_push_is_held_as_an_unclaimed_commit_is`), while an
    unclaimed run writing `.git/estigia/estigia.local.md` through a shell line is not stopped by this row and
    can set it to `off`.
  - **A write that lands outside every repository the gate can place is not a repository write.** The gate
    asks git once per such write (`rev-parse --show-toplevel`) and, where git names no working tree, walks
    the ancestors for a `.git` entry. **No entry anywhere** is outside every repository; **an entry git will
    not use, or an ancestor this process cannot read**, is no answer and the write is held. The walk is not
    git's, so a work tree named only through `GIT_DIR`, a ceiling directory, or a bare or mirror layout is
    read as a repository only where an ancestor has a `.git` entry; it costs one subprocess per such write,
    plus one `git remote` per landing repository for a GitHub tracker that names none.
  - **A write tool whose paths live in a patch body is placed by the body's own paths** (for `patch` and
    `apply_patch` only: `*** Add File:`/`Update File:`/`Delete File:`/`Move to:` lines; a write to **any**
    repository among them is held; no other write tool's body is read as paths, which a `pre_write_code` call
    with a `*** Add File:` line in its `content` regression-tests). A path that cannot be placed is read as
    inside.
  - **A shell write is placed inside only by its literal redirect targets or by naming a repository
    operation.** A redirect to a literal absolute path outside every repository is not held when the line is
    one plain printing command (`echo`, `printf`, `cat`, `ls`, `grep`, `rg`, `head`, `tail`, `wc`, `date`,
    `pwd`) with no quote, `$`, backtick, backslash, `<`, `|`, `;`, `&`, parenthesis, line break or `..`.
    **Everything else a shell line can write is unplaceable** (a `cd` before a redirect, a variable, `&&`, a
    second writer, inline interpreter code): it runs under `on`, wherever the call was launched,
    unless the line names `git` or `gh`; a symlink outside the checkout pointing in is not resolved. **Exactly one shape is added**
    (`shell::quoted_heredoc_target`): `cat > TARGET <<'DELIM'`, `cat >> TARGET <<'DELIM'` or `cat <<'DELIM' >
    TARGET`, a literal absolute `TARGET` without `..`, any body (**not read**; how an issue body is drafted in
    the scratchpad) and a closing `DELIM` line. An "any shape" relaxation was abandoned after five review
    rounds each found a case the reader modelled differently from bash (operator decision 2026-10-02).
  - **A named git write sent into another checkout is unplaceable, not inside** (`issue_requirement::git_directories`):
    when every git command on the line runs in a literal absolute directory — a leading `cd DIR &&` for the
    whole line, or `git -C DIR` on the command — and each lands outside the launch repository, the write is
    treated as one the gate cannot place, because the rest of the line (an interpreter heredoc beside a
    `git stash`) is not read; being a git write, it is still refused under `on`. A second `cd`, `pushd`, `sh`/`bash`/`zsh`/`eval`, `GIT_DIR`, `--git-dir`,
    `--work-tree`, a git word inside a subshell or quotes, or a directory with a `$`, quote, glob or `~` keeps
    it inside. Not placed by this: a git command inside `"$(…)"`, which the classifier does not read as a
    write at all.
  - **A write inside an isolated checkout that a claim names is exempt, whichever session made it** (a
    delegated writer, such as an OpenCode child session deriving its own run, holds no claim of its own): the
    exemption applies when a pointer holding an issue has a `worktree` covering both the call's directory and
    the write's landing and differing from its `repo_dir`. It is path-based, so any session with that working
    directory is exempt, claimed or not, for as long as the pointer holds an issue (no expiry); an unreadable
    pointer grants nothing. Measured at the library level, not through `estigia gate` with a child
    `--session-id` or a live OpenCode host. The stand-down covers an `ask` as it covers a refusal.
- **`Claim workflow` `assisted` takes an issue through a local claim and publishes it as a draft pull
  request.** Every tool in `TRACKER_WRITERS` is refused `workflow-assisted-writes-no-tracker`, `estigia goal`
  answers no claimable work, and `assist` (offered only there) reads the issue and makes the isolated
  checkout without writing the tracker. The claim is a local run pointer that `harness::assisted` reads
  against the ownership marker inside the worktree and the branch it has checked out, so a pointer whose
  marker names another run, whose branch was switched or whose worktree is gone is refused at the next
  boundary or when the renewal window ends; the tracker's `verify-claim` is never asked. A write is admitted
  only inside its worktree on its branch. A write tool's absolute path is placed by that path, whatever
  directory the session runs in: inside the run's repository it is judged against the worktree, in another
  repository it is that checkout's, and anywhere else (a session scratchpad) it is outside the claim, and a
  refusal names that path. A shell write is placed by its first literal redirect destination when the
  line names one, as a write tool's path is: inside the worktree it is admitted, in the base checkout
  the refusal names that file, and outside every repository it is outside the claim
  (`a_redirect_inside_the_worktree_is_admitted_from_the_base_checkout`,
  `a_redirect_into_the_base_checkout_names_the_file_it_would_write`). A shell write with no placeable
  redirect names no path, so it is placed by the session's directory, and
  `cd <worktree> && sed -i …` from the primary checkout is refused `assisted-outside-the-worktree` naming
  where the call runs (`harness::assisted`,
  `a_shell_write_from_the_base_checkout_is_still_placed_by_where_it_runs`). A write tool whose paths live in its body and a shell command
  whose destination cannot be placed (`patch -p1 < x.diff`) are refused `assisted-write-unplaced`; delivery
  and `gh pr create` from a shell are refused with `draft_pr` named. `estigia release --run-id <run>` puts an
  assisted run down whether its issue is a number or a key: it forgets the pointer, writes no tracker and keeps
  the worktree, because nothing local reads whether a person delivered the branch
  (`releasing_an_assisted_run_forgets_its_pointer_whatever_names_the_issue`); `already-holding` from an
  assisted holder and the refused `release` tool name that command. What it does not hold:
  - **A shell GitHub write is refused `assisted-remote-write-unverified`**, whether alone or beside a write in
    the worktree (`an_assisted_run_reaches_a_pull_request_only_through_the_typed_tools`,
    `a_write_in_the_worktree_does_not_carry_a_remote_write_past_the_claim`). `gh pr edit` and `gh pr comment` have
    no typed tool, so in an assisted run they are a person's. Only a call deciding on this run's pointer is held: a
    call with no session id goes through `guard::adjudicate_command`, which still passes it.
  - **`git push origin HEAD` is refused**: the hook reads `HEAD` as the local ref
    (`git_hands_the_hook_head_and_not_the_branch_for_a_push_of_head`). A push is admitted only when every
    line has the run's branch as both refs; `branch:main`, a tag and a deletion are refused
    `assisted-push-unconfined`; a shell push names no refs to the `PreToolUse` gate.
  - **The renewal window rides a local check, not a tracker answer.** Only `harness::assisted` stamps an
    assisted pointer, after the marker and `git symbolic-ref HEAD` agreed, so a marker or branch changed
    inside the window is seen at the next boundary
    (`no_effect_but_the_assisted_decision_leaves_a_stamp_on_an_assisted_pointer`).
  - **The marker and the branch are bytes the agent can write**, an unsworn session in the worktree is not
    stopped for a routine write (`Issue required` holds no routine write at all; a push there to the default
    branch is held wherever it runs), and an unclaimed shell write to the repository's own `estigia.local.md`
    can set `assisted`.
  - **One session can hold several assisted claims, one child run per worktree**
    ([`several-assisted-claims.md`](several-assisted-claims.md)). Measured on Claude Code 2.1.318 from the raw
    `PreToolUse` payload of real subagents, in a throwaway repository with a logging hook: a subagent's
    `session_id` is its parent's; it adds `agent_id`, distinct per subagent, and `agent_type`, its role; and
    `cwd` is the session's directory on every call, including the call after one whose command did `cd`. Only
    `isolation: "worktree"` changes it, to a fresh worktree under `.claude/worktrees/`. A `cwd` argument on the
    launch is accepted and ignored: the subagent's `pwd` printed the primary checkout. Parent and subagent
    calls reached one MCP server process, by its pid. So a shell call by a plainly launched subagent can only
    be placed by its own text, which is why one `git -C` is read. What this does not hold:
    - **A worker for A editing B by absolute path is decided under B.** The landing is attributed correctly, and
      the gate cannot see that the wrong worker wrote it. `agent_id` would tell the workers apart, and nothing
      ties it to a claim.
    - **A shell call physically inside one child's worktree is decided under that child** unless its line moves
      directory by a word the gate reads. `sed -i ../B/x` from inside A still lands in B under A's claim; a
      session in A always had this hole, and under fan-out it is a wrong-issue write.
    - **`git -C A <subcommand>` is judged under A and not read further**, so a subcommand that writes outside A
      by its own arguments is attributed to A.
    - **A child's pointer that cannot be read is nobody's child**, so a call landing there falls to the session's
      `Issue required`, which holds only what reaches the default branch. Under `off` that admits it.
    - **`git pull` is admitted inside a worktree whatever it names**, while `git merge` is admitted only for the
      run's base. The narrower reading is the second; nothing here proves what a pull brought in.
    - **The pointer lock is bounded**: a writer that cannot have it within two seconds falls back to the revision
      check, which can lose an update to another writer in that same moment
      (`concurrent_writers_do_not_lose_a_pointer_update` measures sixteen writers holding it).
  - **`assist` adopts an existing branch only through linkage the tracker records.** A branch origin already
    has is taken only when it heads exactly one open pull request that the host links to the issue: for a
    number, a closing reference on the pull request (`closingIssuesReferences`) or the branch in the issue's
    Development sidebar; for a Linear key, the pull request's URL among the issue's Linear attachments.
    Otherwise it is refused `assist-branch-not-adoptable`, and a failed read of the list, the sidebar or the
    attachments stops before any repository write. The free-text mention (`Refs #12`, a key in the title or
    branch name) was dropped as an admission path: anyone able to open a pull request could write it, and
    assisted pull requests never carry a closing keyword, which is why the sidebar link counts. What stays
    open: the author of the pull request or of the link is not checked, so whoever can link a branch to the
    issue can make it adoptable, and still only for a branch the operator names to `assist`; a branch
    published without being linked (a plain `git push` and `draft_pr`) cannot be taken again until it is.
    The sidebar link names a branch, not a pull request, and a closing reference is matched on the issue number alone, not on the
    repository it names. Measured by the `assist_*existing_branch*`, `assist_adopts_*`, `assist_refuses_*`
    and `only_a_closing_reference_to_the_number_closes_the_issue` tests in `src/transport/branch/tests.rs`, and
    by `a_key_is_adopted_only_when_linear_attaches_the_pull_request` in `src/transport/linear/tests.rs`.
  - **`draft_pr` publishes a draft and guarantees little else.** It pushes the pointer's branch
    fast-forward, opens a draft and reads it back as open, a draft and at the pushed head; it writes nothing
    to the tracker and adds no marker, receipt or `Refs` line. The team's own CI and reviews are theirs; a
    closing keyword is refused in the title, body and branch commit messages and not in a later host edit or
    commit; the branch is not checked to be the one named for the issue. An open ready pull request is
    handled as `Corrections after ready` says (`refuse`: refused; `redraft`: asked back to draft; `push-ready`:
    pushed to as it stands, so CI and reviewers see the new head at once), a draft that exists is reused with
    title and body unchanged (`"title_and_body_applied": false`), and a failed read-back answers in the
    `committed` world. It never creates a pull request open; under `Ready by: agent` with `Self review: off`
    it then marks the draft ready and reads that back, and a mark that fails after the push is
    `draft-pr-ready-not-read-back` in the `committed` world, never a call to publish again. Under
    `Self review: on`, `ready_pr` requires `reviewed_head` and refuses one that is not the published head
    (`ready-pr-review-not-this-head`). That is an **attestation, not proof**: no receipt binds a review to
    bytes, so nothing shows a review happened, in a context that did not write the change, or that it read
    the head it names; it stops a correction pushed after the review from opening unreviewed by accident,
    and a run that states a head it never reviewed passes. A recorded verdict replaces it once assisted
    runs have a review receipt. Held against a fake `gh` by `src/transport/draft/tests.rs` and the
    `draft_pr` tests in `src/harness/mcp/tests.rs`.
  - **`review_stages` answers `ci: none` on three conditions and a clock it cannot read exactly.** The
    rollup must be empty, the head must hold no `.github/workflows` file (one extra read, made once per
    head and only then) and no `check:` stage may be declared. GitHub exposes no time at which a head was
    pushed, so the 120 seconds are counted from the pull request's `updatedAt`: a comment or an edit
    delays `none`, never hastens it. A repository whose CI is external and reports late, with no workflow
    file, can read `none`; declaring that CI as a `check:` stage closes it. A bot with no check of its own
    cannot be waited for: only a declared `check:<name>@<login>` can be, so a repository with such a bot
    must declare it. Nothing makes a run call `review_stages` after `ready_pr`. The answers name it, and a
    `Stop` hook that holds the session open is not built. Held against a fake `gh` by
    `src/transport/stages/tests/no_ci.rs`.
  - **`draft_pr` counts corrections on the run pointer, so the count is only as durable as the pointer.**
    The first head a run lands is the original and each different head after it a correction, a pushed merge
    of the base included, because a merge can carry any edit and nothing here tells a clean one from another;
    the same head again is a retry and counts nothing. A new head past `Corrections per lineage` is
    `draft-pr-corrections-spent` before any host call. What it does not hold: `release` removes the pointer,
    so a run that releases and calls `assist` again starts at zero; a second run on the same branch from
    another machine has its own count; the first head of an adopted branch reads as an original although the
    branch already had a pull request; a failed call is counted from the checkout's remote-tracking ref, which
    a clone whose fetch refspec omits the branch does not update, and such a failure is not counted; and a
    pointer that will not write answers `corrections_not_recorded` instead of holding the next call. The way
    past it, raising the row with `estigia config set`, is refused to an agent by the gate's substring match,
    with the limits that matching has everywhere. Held by
    `draft_pr_counts_each_new_head_and_refuses_the_one_past_the_row` and its neighbours in
    `src/harness/mcp/tests.rs`.
  - **Coordination is left to the team**: nothing reads another person's claim, assignee or label, `assist`
    refuses a second run on one branch on this machine only, and Estigia cannot tell that the person, not the
    agent, named the issue.
  - **An unreadable contract is not treated alike by every tool.** `GateContext.workflow` is `None`; `assist`
    is refused and an assisted pointer is `assisted-claim-not-in-force`, but tracker writers are not refused
    (that would stop every tracker write in every repository whenever one unrelated row was malformed), so a
    broken contract in a repository whose row says `assisted` leaves tracker writes open until it reads again
    (`an_unreadable_contract_names_no_workflow_at_all`,
    `an_assisted_only_tool_is_refused_unless_the_row_says_assisted`). This is the contract chain — the
    versioned block, the operator's `estigia.local.md` and an agent's own file — and not the repository's own
    file: one that is there and cannot be read reads as the denying answers (`on` for `Issue required`,
    `unreadable` for `Git operation window`), so the gate refuses what those rows could have refused,
    and the refusal that stops an unclaimed write names the file and the read error.
  - A test classifies every tool in the server's table as tracker writer, assisted-only, follow-up
    (`review_stages` and `review_thread`, offered under both workflows), pull review, local writer or
    read, so a new tool fails until placed; a writer a person runs by hand through `gh` is not a tool call.
- **The screen's door and event loop are unmeasured.** `the_screens_install_proceeds_on_a_source_build`
  drives `guided_install`, which always consents a source build (a person confirming the screen is that
  consent), and nothing asserts that `dispatch` does not put a lifecycle preflight back in front of `guided`;
  `every_production_caller_of_install_planned_is_preceded_by_the_lifecycle_preflight` is a text guard that a
  `#[cfg(test)]` call, macro, function pointer or `use` alias escapes. The screen writes three things the
  lifecycle gate covers none of (the guard key's `pre-push` hook, the language key's remembered tongue, the
  folder picker's directory: `docs/lifecycle.md`), and `run` needs a terminal, so that it passes
  `app.repository` to `install_guard` and `report_of_checks`, keeps a guard refusal apart from an install
  refusal and calls `Reads::refresh` before each key are readings. `wrapped_rows` in `src/tui.rs` emulates
  ratatui's word wrap (`Paragraph::line_count` sits behind `unstable-rendered-line-info`), checked only
  against Help in both tongues and one synthetic long row.
- **The legacy-checkout stop cannot name the legacy path of a template that named no placeholder.** A
  `Worktree location` missing `<branch>` or `<run-id>` gains it in memory, and `claim` with `branch` and `base` stops with
  `legacy-worktree-registered` when the pre-migration checkout is still registered, computed from the raw
  template. For a bare directory naming **neither**, that raw path is one no build created (they appended
  `~<run-id>`), so a run resuming such a branch is stopped by `branch-checked-out-elsewhere` instead, before
  any remote state is written and with no data lost; the recovery is the same `git worktree remove`, written
  in the binding. Measured across twenty-four template shapes by the blind reviewer of receipt
  `6b192e0ee94f11004b621c06c8e8e5dd`.
- **Five of the twenty-one are about the past.** `LOOKS_BACK` excludes `silence`, `stale-run-pointer`,
  `prune`, `gate-version` and `store-records` from readiness: undecided past calls, another run's
  pointer, orphaned branches, a stale diagnostic binary and appends that already failed do not prove
  a fresh claim impossible. `store-records` reads counted append failures; a successful snapshot
  deletes the `snapshot` row, and a counted failure whose last occurrence is older than the thirty-day
  record retention is not broken and the daily pass deletes it. A row whose last occurrence the store
  did not record stays broken, because its age is unknown and an unknown is not clearance. The store
  row is not exempt: an unreadable authority prevents claiming now. This classification is a policy boundary,
  not proof that the historical problems were repaired.
- **The claim and review families decide from the local workflow store and write only to it; GitHub carries
  labels, the assignee and human prose.** First touch imports through `gh issue view`, refusing
  `import-diverged` for differing holder/latest publication. Open-held renewal costs two REST calls,
  held by `a_gated_write_on_an_open_held_issue_makes_no_graphql_call_and_two_rest_calls`; closed-issue
  delivery proof uses the stored receipt, held by
  `an_own_delivery_close_is_proved_from_the_store_when_github_carries_no_marker`. GitHub projection
  follows local commit; the stand-in rejects posted `issue-flow:` bodies. Closing-reference,
  projection-convergence, claim assignee/label and handoff label reads still use GraphQL. Limits:
  - **After the first touch only a person's stand-down or adjudication is admitted from GitHub**, from
    comments **whose markers are all `standdown` or `adjudication`, written by the authenticated user and
    never edited** (`after_the_first_touch_only_a_stop_is_admitted_from_github_comments`). An operation-id
    claim needs `adjudication`; comment stand-down cannot carry its scope. Later edits, labels and
    assignees are not reimported. GitHub `created_at` and local marker clocks can tie; insertion order
    may differ across stores. Sync fails at 10,000 comments. REST trust requires authenticated-login
    equality and flags any differing created/updated timestamps, stricter than GraphQL.
  - **A lost claim race rolls back; a lost reclaim race still commits** (`reclaim` adjudicates after its
    transaction). The claim makes no label, so a tracker that refuses label creation
    cannot fail it. The state compared is the store's, set at import from the single
    `status:` label (two or none import as no state: `unexpected-state` until a `transition`). The
    differential check compares the holder and the latest publication, and nothing else.
  - **An older binary on the same issue cannot see these markers**: it reads GitHub only, may claim an issue
    this binary holds, and its merge gate may clear a head this binary has not reviewed. Nothing detects the
    mix; the cure is reinstalling every agent profile before an issue is in flight on both. A person
    reading the issue page sees no claim or receipt comment, and there is no single chained lifecycle test.
  - **`create`'s store admission is recoverable only by title.** If admission fails after filing, a repeat
    searches `gh issue list --search "\"<title>\" in:title"`, adopting a single match, refusing two and
    filing anew on none, so a search index that has not caught up (a lag nobody measured) would let the retry
    file a duplicate.
  - **The queues decide from the same store.** `list-state`, `list-identities` and `completed_count` read the
    open issues from one REST list cut to the oldest 200 **open** issues across every state (a state whose
    items are all newer than that cut is not seen), decide state, holder and dedup key from the store (an
    open issue the store never admitted is `unadmitted`, reported by `estigia goal` as `null` when the read
    failed, never `0`), and read the review queue's per-candidate timeline without the sync a ledger open
    pays. `estigia store import` admits untouched open issues oldest first.
  - **The dedup identity, the `blocked-until` clauses and the task checklist live in the store and nowhere
    else**, so a lost `state.db` loses all three. `transition` records a clause without requiring a claim; a `task` is recorded, not
    verified (nothing checks its commit exists or its `evidence` note names anything real); `transition --to
    done` answers nothing to verify when the issue has no confirmed publication. FLOW's routing rules in
    `skill/SKILL.md` are instructions, enforced only by `task`'s `done` rule, and their effect on cost is not
    claimed (`docs/review-cost.md`).
- **The store is one copy, and its protection is local and partial.** Losing `~/.estigia/state.db`
  loses store-only identities, clauses, tasks, in-flight claims and review history. Reimport cannot
  recover what GitHub never held, even while its projection still names the run. On the write path,
  `VACUUM INTO` takes a snapshot when none exists or the newest is at least 24 hours old, keeping
  three in `~/.estigia/snapshots/`. Failures are counted with when they last happened and the reason
  the last one gave, not fatal; a later successful snapshot clears the snapshot row. Restoration is manual.
  A store about to migrate is copied first, in its old schema, and the migration is refused when the
  copy fails ([lifecycle](lifecycle.md)); a write another process lands between that copy and the
  migration's lock is in neither. A store mid-migration looks like an older one, so openers racing a
  fresh store or an upgrade may each stage a copy; each is named for the schema read back from the
  copy, one already in the build's schema is dropped, and pruning keeps the newest per schema, which
  can leave copies of intermediate schemas nobody shipped
  (`many_openers_of_an_older_store_all_open_and_its_old_schema_is_kept`).
  Doctor reports age, not a verified restore. Append-only triggers do not resist an OS user
  able to drop them. `store-unwritable` rejects detected network mounts on macOS/Linux, not Windows.
  `foreign-projection` refuses unexplained GitHub owners across stores; reclaim overrides it.
  The best-effort `gh` counter counts spawns, not cost, misses other threads/undispatched operations,
  and silently drops counts on open/insert failure or a store busy beyond 200 ms. No test here proves
  restoration, Windows mount detection or counter completeness.
- **A run that swore nothing is never gated, including when Estigia is broken.** Failing closed on everybody
  would be a lock rather than authority and would teach people to remove the hook: a missing transport denies
  a sworn run's write and lets an unsworn one through. **An unreadable run record stops a write**: a pointer
  that exists and will not read says a run existed and not what it swore. `session::load` treats only a
  `NotFound` read as absence, and only while the pointers' directory is one (Windows answers a path whose
  parent is not a directory with `ERROR_PATH_NOT_FOUND`, so a state root that is a file would otherwise read
  as *every run swore nothing*); other failures load as unreadable and name the path and error. The push
  guard denies `run-pointers-unreadable` when nothing readable covers the checkout and some holding is
  unreadable, and a readable holder that covers it makes the unknown one change nothing.
- **A contract row the gate cannot read costs that row, and no longer the rows beside it.** The gate reads
  the installed contract with a fallback to defaults rather than refusing, because a file that will not parse
  is a problem `doctor` reports, not a reason to stop an agent mid-edit; the fallback takes the renewal
  window to zero and keeps every row that parses
  (`one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it`; salvaging can only narrow, since
  `Config::default` is the loosest configuration). A *boundary* row that will not parse still costs the
  boundaries it named (`an_unreadable_contract_costs_a_declared_boundary_its_boundary_status`), the one place
  where *configuration may only tighten* runs backwards.
- **Every script this crate writes reads the binary's answer instead of inheriting it, and a script that
  cannot get one lets the call through with a word.** Only `1` (a refusal) and `2` (an outcome it could not
  read back) are decisions; anything else (a missing binary at `127`, a panic at `101`) means it did not
  answer and the write or push goes through (*"a hook that breaks does not deny"*). The OpenCode plugin holds
  the three codes (`the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer`) and a result with no numeric
  exit code is let through with a word (`the_plugin_says_so_when_the_gate_returns_no_exit_code`). The cost:
  on a machine whose binary has gone, every gated write goes out unmeasured, saying so on standard error;
  failing closed would be a harness people uninstall the first time it is wrong at a bad moment.
- **Nothing here resists a local actor running as you.** The gate reads a run pointer, a contract and a hook
  command, all files in your home with your permissions; anything able to write them can rewrite what Estigia
  believes, and a replaced binary answers whatever it likes. No tamper-resistance is claimed: the threats are
  accidental (a truncated write, a crash mid-rename, two processes racing on one pointer). **A stale writer
  is dropped, not merged**: a pointer on disk written more times than the one in hand is news the caller has
  not seen and its store is skipped (the fail-closed direction), without reconciling two concurrent edits or
  reporting the dropped one.
- **The boundary list is compiled, and can only be added to.** Estigia knows git and GitHub, not that a
  repository ships `npm publish` or `terraform apply`, so `Irreversible commands` lets an operator name
  those as boundaries (no renewal window, a fresh read every time). It **only adds**: no value removes a
  built-in, because a setting that can make the gate looser is not a guard rail. Nobody can express a rule
  the classifier has no shape for; matching is substring, on a normalised command line whose
  provably-data spans are blanked only in read-only simple commands, alone or as the top-level commands of
  a compound line through which no data reaches execution.
- **Decisions are written down; the reasoning behind them is not.** Every allow and deny is appended to the
  local store's `decisions` table with the run, the tool, **what the decision was about**, the verdict, the
  refusal code and the repository it was made in — `github.com/owner/name` when the checkout's `origin`
  names one, otherwise the main checkout's path, and nothing when the directory is in no repository. A call
  identified only by `--run-id` — `estigia gate`, the question a person or an agent
  asks — answers and records nothing and touches no run pointer, because that road is how the gate is
  probed and it once wrote 75% of every recorded denial. `Outside` is not recorded (a record per tool call of every session that never swore would bury
  the ones that matter) and the write is best-effort (a refusal that could not be written down is still a
  refusal). **A payload Estigia cannot parse does not stop the call, and says so**: the hook stands aside
  (a schema this build does not know could wrap `Read` as easily as `Write`) and writes `payload-unreadable`
  to the ledger. Records older than thirty days are deleted at most once every twenty-four hours, by
  `SessionStart` only; each repository also keeps at most ten thousand decisions, newest first, and the
  undecided verdicts `doctor` reads are never removed by that cap, only by age. A record that could not be
  appended is counted (itself best-effort) and reported by
  `doctor`; the JSONL history this replaced was imported then deleted (a torn legacy line is skipped, so
  whether every record survived is not measured). `status` lists what this machine's runs recorded holding;
  whether those claims are live is the tracker's answer.
