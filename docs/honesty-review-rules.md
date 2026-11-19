# Review rounds, roles and corrections

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the roles, the round bounds, the correction budget and the publication lane do not establish. The index says which area holds which question.

## Roles and the definitions they need

- **`Evidence standard` gives the reviewer a shell, and a shell is not under the gate.** The reserved
  reviewer's grant is derived from `Evidence standard` (`reading`, the default, or `measuring`). Under
  `measuring` the role gets a shell and nothing else: `Write`, `Edit`, `Agent` and `Task` stay refused
  (a test walks `Evidence::all()` asserting that shape), which withholds four tools and not the
  capability, since a shell writes and launches. A judge that measures writes inside the directory its
  launch hands it, and **when that directory is outside the claimed checkout** the stand-aside answers
  before the tracker is asked. That condition is the orchestrator's to meet and nothing enforces it: a
  delegated context's tool events carry the launching session's id, so a judge handed a directory *inside*
  the claimed checkout is measured against the parent run's live claim and **allowed**. A five-judge panel
  sharing one scratch directory once had a judge's script overwritten by another's and executed in a
  third's checkout, so two verdicts stopped being independent. The surviving statement of what a shell does
  not buy is in `skill/policies/judgment-day.md`. The gate reads the row from `GateContext` and **an
  unreadable contract answers `reading`**, because a fault must not hand out a capability.

  `reviewer_is_static` means *one of the spellings Estigia can produce*, enumerated over `Evidence::all()` ×
  `crate::harness::roles::RESERVED_REVIEWERS`; the compiler holds the `Evidence` axis closed, and the seal
  axis (a plain slice) is held by `every_seal_has_a_reserved_name`. **No acceptance test enumerates the
  sealed rows**: the walk over `Evidence::all()` is one block inside
  `the_default_rendering_is_the_bytes_already_installed`, two sealed bodies are pinned one at a time
  (`a_sealed_rendering_is_static`, `a_sealed_lens_rendering_is_static`), and `review-risk`,
  `review-readability`, `review-reliability` and `review-readback` sit where no acceptance assertion reaches
  them; `seal_has_exactly_two_values` pins only `Seal::all().len()`. The crossing that binds the *installed*
  bytes, `the_reserved_reviewer_grant_is_the_same_through_both_doors`, walks a hand-written pair of rows, so
  a third standard reaches it only if somebody adds it.

  **The workflow front door is reachable the same way.** `estigia claim` and `estigia transition` are CLI
  entrances onto the operations the tool server runs, and the role gate bars tool *names*, not shell
  commands, so under `measuring` the shell that reaches the target reaches those. What holds a judge off
  them is the sentence *"Still do NOT change tracker state"* in its own definition: an instruction, not a
  gate. A judge holds no claim, so a write against the repository still refuses; what is unguarded is the
  tracker call in its own name. At the grant "no delegated context gains orchestration authority" is true;
  at the shell it is prose.

  **`setup` and `sync` reach further than a redirected `HOME`.** A measuring judge's scratch-home sync
  once rewrote two real-profile files through `%APPDATA%`. `Environment::resolve` now accepts inherited
  `APPDATA`/`XDG_CONFIG_HOME` only under the supplied home (`paths::covers`, through nearest existing
  ancestors), otherwise substituting a home-relative fallback with a note, not a refusal.
  **The gate differs:** `harness::roles::definition_for` reads `XDG_CONFIG_HOME` raw and searches both
  the default and relocated OpenCode roots; nothing tests that divergence.

  Three enumerated cases lose a legitimate location: relocated `%APPDATA%`, `XDG_CONFIG_HOME` outside
  `HOME`, and `HOME` differing from `%USERPROFILE%` with unchanged `%APPDATA%`. Setup can report success
  after registering where the host never reads. The note is the only warning; the process cannot tell
  intentional relocation from incomplete home isolation. This is measured on one machine, not an
  exhaustive population (`what-it-writes.md`). The cost remains for OpenCode and uninstall of retired
  hosts. **Uninstall uses the same substitution:** real `%APPDATA%\gemini\settings.json` `BeforeTool`/MCP entries
  and `GEMINI.md` can remain live, with no route back except the note.

  **The test-only environment lock excludes writers, not all readers.** `src/test_env.rs`'s
  `ENVIRONMENT` also excludes readers opting into `with_environment_read_locked`, but unlocked reads of
  `HOME`, `USERPROFILE`, `APPDATA` and `XDG_CONFIG_HOME` remain, including `harness::roles::tests` calls to
  `definition_for`. Thus unsafe `set_var`'s precondition is not met. A picker race failed 3/15
  `cargo test --lib home -- --test-threads=8` runs and is fixed for that test only. The hand-maintained
  `// SAFETY:` notes name known readers, not a complete inventory; no test detects a new unlocked reader.

  **One row, one file, and a machine can hold two answers.** `Evidence standard` is `Scope::Everywhere`, so
  it is set per repository, while the artifact it decides is a single user-level `review-blind.md` rendered
  from whichever checkout last ran `setup` or `sync`. On a machine holding one `reading` and one `measuring`
  repository, the drift `docs/configuration.md` frames as lasting until the next `setup` or `sync` is
  permanent for whichever was not last written. The gate answers per repository and narrows on an unreadable
  contract, so this costs a capability rather than granting one.

  **Still open.**

  - No derived directory family is compiler-forced: `skills_root_fragment` matches `SkillsRoot`, whose
    variants a new adapter reuses. The `_` arms on the instruction enum are in
    [What the gate writes, and who else reads](honesty-gate-surface.md#what-the-gate-writes-and-who-else-reads).
  - **The roles a run's delegated contexts ran as are recorded, within the run and durably.** New
    `agent_type` values reach the session pointer and append-only `roles` table independently of claim
    decisions, including `Outside`; `estigia roles` reads by run/issue. Model/effort are declarations.
    This is neither a count nor proof of isolation or a panel: absent host metadata leaves no record.
    Role-gate refusals are unrecorded; claim-gate refusals are recorded. The default reviewer's
    `Read, Grep, Glob` never wake the matcher. Built-binary measurement: `review-blind` `Bash` under
    `reading` denies without a pointer; under `measuring` records `roles: ["review-blind"]`.
    A new role on a denied call can also persist a verification watermark, letting the next routine
    write ride renewal, and create `<runtime>-unknown` for a blank session id, not removed by session
    end. `session::store` drops stale revisions, so concurrent tool writes can lose pointer roles;
    the append-only log avoids that race. No test establishes complete host role reporting.
  - **`Run.inline_writes` (`src/harness/session.rs`) is only a within-run record.** It adds a target
    (path or shell fragment/verb/tool) on an allowed call with missing, not blank, `agent_type`, only
    after a role was seen. Hosts omitting metadata, runs never delegating, earlier inline writes and
    role-refused calls produce nothing, not a floor. `Outside` writes are excluded; unplaceable and
    relative paths count inside. No model identity is established and no inline write is refused:
    the gate cannot judge "non-trivial"; a changed-file proxy over-fired on this history.

  - **A write to `.git/estigia/estigia.local.md` is a boundary whichever row it names.**
    `crate::config::rows` reads regions: changing a newline, fence or preserved tail can change guarded
    rows without naming them. `is_repository_configuration` therefore guards the whole file on the
    write-tool road; shell writes and `chmod`/`chattr`/`chown` use the control-surface boundary checks.
    `REPOSITORY_SHELL` also asks these checks (`git commit -m wip && printf x >>
    .git/estigia/estigia.local.md` is `Boundary`); `IRREVERSIBLE` still precedes it.
    **Over-gating:** even an ordinary row such as the retired fixture string `Repair ceiling` costs a
    tracker read. `estigia config set --repo` is unaffected; allowed scopes are in
    [configuration.md](configuration.md), *Which root the gate decides in*.
    **Under-gating:** `rm -rf .git/estigia` and `mv .git/estigia .git/estigia.bak` remain `Routine`;
    neither the configuration-file spelling nor a control-surface fragment names the bare directory.

    **Mentioning a surface also pays.** Whole-line matching makes `git commit -m update CLAUDE.md`
    and a message naming `.claude/settings.json` require a live read. `CONTROL_SURFACE` has twelve
    separator-free entries, seven instruction files (`claude.md`, `claude.local.md`, `agents.md`,
    `agents.local.md`, `gemini.md`, `qwen.md`, `crush.md`). This is the fail-closed asymmetry of
    `WRITES_A_FILE_POPULATION`; the frequency of such ordinary commits is unmeasured. Debug timings:
    `git stash` 4.6→29 µs, a long `-`/`~` message 39→470 µs, against roughly 75 ms end to end for
    `estigia gate Bash`. The suffix ladder is unbounded; an 832-byte `rm` operand cost 1.70 ms.

  - Estigia still cannot prove a Judgment Day ran, how many contexts it had, or that any two of them were
    given separate directories: this narrows what a judge **can** do and proves nothing about what a judge
    **did**.
  - **A reviewer's directory has no remover, and Estigia is not one.** Each Judgment Day round mints one
    directory per judge, but Estigia does not spawn the judge and `release` puts down only the checkout
    the claim created, so they accumulate until an operator sweeps them by hand, after the judge has
    returned its findings and only once `git status --porcelain` is empty and every commit is on the remote.
    They live under the configured `Worktree location`, which makes them look like registered worktrees when
    Git considers them no such thing (a separate root is an open question). `estigia prune` lists a detached
    checkout no branch owns but cannot tell a judge's directory from any other orphan: a name to check, not a
    disposal.
  - **What a delivery's remote deletion cannot see.** `release` deletes every remote head the run recorded
    starting, read back through `ls-remote`, but the record is only what the claim's checkout wrote: a branch pushed
    by hand, or under an identity the issue's timeline never named, is not in the set, and GitHub's
    `delete_branch_on_merge` covers the pull request's own head only. `estigia prune` catches it later, and
    only when the operator confirms.
  - **What the prune sweep cannot see.** It attributes a head or checkout to an issue by the `issue-<n>-…`
    convention in a branch name or by a run pointer, so a head naming no issue and carrying no pointer is
    listed only when merged into the base, while a checkout is always listed, kept when nothing says it is
    finished. The orphan route recognises a clean checkout whose commits remain on any refreshed
    remote ref even when the branch is neither merged nor closed, and one whose upstream is still present
    when its head is contained in what that upstream lists
    ([what-it-writes.md](what-it-writes.md#explicit-orphan-pruning);
    `a_remote_gone_checkout_reachable_from_another_remote_is_removed_only_on_confirmation`,
    `a_clean_pushed_checkout_of_an_open_branch_is_removed_only_on_confirmation`). A session that died
    without its registry row recording an end still holds its checkout here, so the dead-session case
    that route exists for is cleared only once that row is ended. The legacy route's
    "a live claim holds it" is a readable run pointer whose issue the tracker still reports open, not a re-read
    of the claim (a pointer left behind reads as live, a claim the pointer never recorded as unheld, and
    `held_worktree` stops protecting a readable one when its issue reads closed). The orphan route keeps every
    matching pointer and every checkout named by a registry session not explicitly ended, refuses unreadable
    ownership, and rereads both at confirmation with the exact commit inspected as retirement's expected HEAD.
    These are observations, not a lock: a claim with no pointer and no session directory cannot be attributed,
    and another process can acquire ownership, recreate the upstream or change Git state between the last read
    and `git worktree remove`.
  - **The sweep's remote deletion bypasses the push guard, and that is a bypass.** `estigia prune --confirm`
    runs `git push --no-verify --delete` for each head, because the guard refuses a push no live claim
    authorises and the sweep holds none. A deletion is not work entering the flow and `--confirm` is the
    authorisation, passed by the operator or by an agent (finished cleanup is the run's to execute), but
    `--no-verify` is the rail the guard's own script names, so anything that can run `estigia prune` can
    delete a remote head without a claim. The listing before the deletion, and the rule that a live claim's
    head or a checkout with unpublished commits is never marked, bound it.
  - `Evidence standard` is one row for one shipped reviewer definition, not per role: the delegated roles
    carry fixed grants, and a repository wanting a different grant for one has no way to say so.

## Skips, deletions and the bounds of a round

- **A skip spelled as a pass: the remaining skip sites, and the routes the rig guard does not hold.**
  `tracker_rig`, `repository()` in `src/harness/guard/tests.rs`, `repository_with_commit` in
  `src/test_env.rs`, `git_repo()` in `src/harness/doctor.rs` and `release_repo()` in `src/cli/tests.rs`
  return the directory (or rig) and panic, naming the failing command, when `git` is absent or any call
  fails, so a machine without git fails those tests loudly instead of reporting them `ok`. **The remaining
  skip sites:** seven in the `pipe` suite itself, across five test functions, at the read-only-file
  capability and at git usability, left because raising or `#[ignore]` is a decision (`#[ignore]` is
  reported as *ignored*, a `return;` after an `eprintln!` as *passed*).
  `a_row_that_is_broken_comes_out_of_the_report_broken` prints its skip and **does not return**, dropping
  two assertions and reporting `ok`. The read-only capability is a genuine machine property and the git
  sites are not, so one policy cannot close both. A legitimate skip is one the machine could not have run:
  symlink privilege, `mklink /D`, administrative shares, drive letters (the ten skip sites in
  `src/harness/tests.rs`) and build-state properties (no commit, a dirty tree, a shallow clone, no scratch
  directory). `git` is not that: a machine without git cannot measure the push guard **at all**.

  **The guard that holds the rig catches an accident, not an author.** Reviewers walked past
  `the_tracker_rig_cannot_answer_that_it_did_not_run` twenty-three times; eleven routes are held and twelve
  are not (counted from the table). Each attempt was measured with the fixture removed, except those about
  *where* the fixture is looked for (measured with it present); `106 passed` is the defect reproduced whole
  with every gate green.

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

  The filter is `trim_start().starts_with("fn tracker_rig(")`, so a real definition written `pub fn` is
  invisible to it and a decoy becomes the unique match (measured: the suite and the guard stay green), and the
  body ends at the first line that trims to `}`, which a multi-line string literal can supply from column
  zero; both are one more needle away from being caught and not chased, because recognising a Rust item is
  parsing it. The file-wide `process::exit` scan catches `use std::process::exit as leave;` because that line
  contains the substring, and not the two other spellings; a macro, a labelled `break` and a body skip evade
  any substring. **What actually holds the accidental path is the compiler**: `let Some(rig) = tracker_rig()`
  does not compile against a non-`Option` (`error[E0308]`), so nobody reintroduces this by copying a
  neighbour. The `return` scan reads whole lines, string literals included (a version that cut strings out
  was defeated by `let _sep = '"'; if !built { return; }`), and its two false positives (a `return` in a
  string literal, and one inside a **closure**) are declared in the failure message.

  One more control-surface path of the hosts file's shape, not closed: `<checkout>/.git/config` answers
  `Routine` while `<checkout>/.git/hooks/pre-push` answers `Boundary`, and `core.hooksPath` in that file
  decides whether the push guard runs at all. Normally harmless, since `covered()` yields both `repo_dir`
  and the worktree; with a pointer carrying only a worktree (`repo_dir = <project>/worktrees/wt-a`) writes
  to `<project>/src/x.rs` **and** `<project>/.git/config` both stand aside. `claim` with `branch` and `base` now records the
  claim's checkout beside the worktree, so Estigia's own tools no longer *produce* such a pointer; it stands
  for a run whose `repo_dir` genuinely is a worktree or a hand-written record. Whether the fix is naming the
  file or widening what a worktree run covers is open.

- **A deleted comment is missing evidence, never satisfied evidence.** The verdict requirement does not
  appear only once a handoff exists (else deleting the handoff comment would lower the bar from *a computed
  verdict approved these bytes* to *nothing*); delivery asks for the same computed `approved` on every
  route. The one exception is the `low` tier, which licenses no lens and needs no marker: a deleted finding
  reads as absent and the receipt recomputes as `approved`, and a deleted lens report reads as a lens that
  never ran at `medium` and `high`. A deletion can still **restore** something, but only with a `gh` call
  outside Estigia (no operation edits or deletes a comment): deleting a handoff comment lifts the requester
  exclusion it carries, returning the publishing run to the queue for work no verdict covers (liveness, not
  integrity); deleting a `rejected` `review-verdict` marker lifts the `legacy-verdict-rejected` refusal over
  those bytes, the one deletion that clears a release (the ledger recomputes `approved` over the same
  receipt, so a lost objection rather than an unreviewed merge).
- **The draft/ready CI barrier is cooperative, not adversarial.** Compatible repositories start PR CI on
  `ready_for_review`, not topic push/open/synchronize/reopen, and carry a `workflow_dispatch`
  **publication lane** that `publish_review` starts once per publication epoch against the head it pushed
  (not once per push; dispatching does not mark the PR ready). GitHub has no atomic conditional-ready
  operation. Collaborators or repository workflows outside Estigia can mark ready, push, trigger other CI or
  forge comments; Estigia neither parses arbitrary consumer YAML with weak substring checks nor claims
  malicious-collaborator authenticity.

- **Corrections per lineage are enforced where a correction is published, and counted from markers anyone
  who can write a comment can also delete.** The `Corrections per lineage` row (default 3) counts an
  original candidate and its repairs on one pull request, and the publish call refuses the correction past
  the limit (`correction-already-spent`) before it pushes. Nothing is stored: `repair_rounds` derives the
  count from the `parent` receipts on the timeline, so a deleted `published` or verdict comment lowers it
  and a forged one raises it. It does not read: **a correction nobody published** (an attempt that never
  reached `publish_review` spends nothing, and a republish over a parent no panel has judged is not a
  correction, so a run can republish as often as it likes before a verdict lands, bounded by the size
  budget and scope admission); **a new pull request** (the lineage boundary: a chained slice or replacement
  begins afresh, and nothing tells a new change from the same one republished under a new branch name); **a
  forced base advance** (the integration is the lineage's correction when it is the first, refused when the
  parent already was one; how often the base moves is not measured); **a republish over an approved
  receipt** (zeroes the count, spends no correction, still bounded by budget and scope and refusable
  `correction-outside-reviewed-scope`; a warning- or suggestion-cleared one is narrowed to one lens and no
  judge by `review_tier_for_repair`); and **the escalation that follows** (`escalate_review` opens when
  `repair_rounds` answers `correction_spent`, whether or not the publication was refused, and nothing reads
  whether the person escalating had cause). Nothing refuses a panel launched over a spent lineage or
  requires `repair_rounds` to be called, and Estigia cannot prove a panel ran, so it cannot prove a
  correction was reviewed; delivery still requires an approved computed verdict. Judges are never told the
  count. `Repair ceiling` is retired and an installed row is reported `repair-ceiling-retired`.

- **A validator result records an answer the binary was handed, not proof the validator read the trees.**
  A hand `record_validator_result` is refused `recorded-by-capture-only`. It stores that a read-only
  validator answered `pass` or `fail` for `original_criteria` and `correction_regression` over one
  correction, written by the run holding the `review` claim because a sealed validator holds no tools;
  Estigia cannot see which diff or findings it was handed, whether it inspected the frozen trees, or
  whether `unavailable` was honest. A conclusive result is recorded once, a pass never overrides an open
  severe finding, a failure once the lineage has spent its `Corrections per lineage` or a third
  `unavailable` writes the `escalated` receipt `release_ci` refuses, and a failure with a correction left
  computes `rejected`. Limits: the validator is required **in addition to** the tier's lenses
  (`docs/review-cost.md`); nothing sets the attempt number except the caller (a skipped number is refused,
  but a run that never records `unavailable` is never escalated by it); `budget_exceeded` is not a cause;
  `escalated-to` is the word `maintainer`, not a person; and a receipt's standing does not fall back to an
  empty publisher when the timeline cannot be read, while a pull request's first publication records no
  parent.

- **A recovery is a person's command, and what the gate can prove about that is narrow.** `estigia
  recover` is a CLI command with no MCP tool; the gate denies an agent's shell line for it
  (`recover-is-for-a-person`, any host), decided by `harness::person_only` before any claim, stand-down or
  role road, and the named login must hold the `admin` or `maintain` role on GitHub's
  `collaborators/{login}/permission` read (`role_name`). **What keeps an agent out is the gate's shell
  match, not the login check**: the login is not authenticated, so anyone with shell access and a gh token
  can name another maintainer. The Bash rule matches the words `estigia recover` wherever they stand, on the
  line and a dequoted copy, so a commit message that merely names both is refused with no stand-down escape,
  and an alias or wrapper that never writes them as two words is not matched. `!` in Claude Code and the
  person's own terminal never pass through the agent's tools. The record is local, so a lost store loses the
  recovery, and a publication carrying a recovered-from field with no readable record is refused
  `recovery-record-missing`. The inherited tier is a floor taken when the recovery is recorded; `review_tier`
  and `review_tier_for_repair` read only base and head and do not apply it, so a successor's advertised lens
  count can be smaller than its verdict will demand. A half-finished review that never escalated is not
  covered. The marker is a timeline entry like every other, so a comment synced under the same identity can
  carry it. A correction on a new pull request over a rejected parent on another is a new change and owes
  no validator.

- **150,000 is an inherited comparison target, not a number derived from the measured distribution.**
  `Review budget` defaults to 150,000 tokens **per context** (a lens or a judge), not per panel or issue,
  and accepts `none`. The figure was inherited from an external project; at its `v4.0.0` that project's only
  `150000` is the parent-context length at which a long session is advised to delegate, a different quantity
  from a per-judge spend. The distribution supplies a check, not a derivation: twenty-eight judges across
  five issues, from harness completion reports, put the minimum at 96,073, the median at 127,025 and the
  maximum at 182,136 tokens, and 150,000 sits above that median while 18% (5 of 28) still ran over it.
  a `spend` record states a panel total beside a `contexts` count (a convention; validation refuses only a
  zero). The row is **advisory**: nothing refuses a panel whose recorded tokens exceed it, and a recorded
  `spend` is a declaration, not a measurement (an under-report is undetectable). **None of the
  twenty-eight figures include the orchestrating run itself**; the delivery step records one `spend` record
  with `phase: orchestrator` before it closes (`docs/review-cost.md`), which closes the *channel*
  and not the *trust* gap. `skill/policies/judgment-day.md` and `skill/bindings/github/operations.md` name
  the row, and nothing in the always-routed contract reads or reports the figure, so naming it there would
  describe an action no run takes.

## The correction budget and the publication lane

- **The correction budget is a genesis tree pair, held and never recomputed, and it has narrower
  guarantees than the machine it ports.** `publish_review`/`republish_review` measure a repair between two
  frozen Git tree object ids (`git rev-parse <rev>^{tree}`, `git diff --numstat` between two trees), never
  a payload and never a mutable ref. The parent is **selected, not validated**: `publish_with` picks the
  latest publication on *this pull request* rather than the issue's newest, and `repair_rounds` scopes the
  same way and shares whatever this leaves open. Five things this port does not prove:
  - **A first publication is never budgeted.** Nothing freezes it against, so `budget-candidate-tree` is
    written alone and a candidate of any size publishes.
  - **A base advance does not reset the budget, and what replaced the reset is measured against a tree
    nobody authored.** When the base advanced past the parent's (`base_advanced_past`) and the branch
    integrated it, the round is measured from `git merge-tree --write-tree <parent head> <new base>` to
    this head's tree; the genesis, budget and cumulative carry over (`budget-parent=integrated`), and the
    conflicted paths and the advance's rename destinations are admitted beside the genesis paths.
    `budget-refrozen=base-advanced` remains for the one shape that re-freezes deliberately: the fork point
    is not in the parent's history and the base was replaced rather than advanced. Open: **conflict
    resolution is charged as authored lines** (a conflict-heavy integration can consume the budget on lines
    the author did not choose to write); **only conflicted paths and rename destinations are admitted**
    (an edit to a cleanly merged path is a new change, refused for scope, reporting `genesis_paths` and
    `integrated_paths` separately); **`git merge-tree --write-tree` needs git 2.38** and a failure fails
    the publication as a failed read with nothing pushed and no fallback (the fixture pins an unresolvable
    ref, not an old git, which this repository's CI does not run); and an unmerged advance never reaches
    the check, because `publication-base-not-ancestor` refuses it first, naming merge or rebase outside a
    correction and only the merge during one (a rebase rewrites the parent head and the validator refuses
    `validator-delta-carries-upstream`), at the cost of two reads (pull request listing and timeline).
  - **The forecast is not measured, only the actual.** The source project estimates a correction's size
    before it runs; every figure here is measured afterwards from two trees that exist.
  - **There is no compare-and-set and no lock**, because an issue timeline is append-only. Two publications
    racing one lineage can read the same parent marker and both post; the second marker still carries a
    correct measured total, but the single-round rule a lock would make load-bearing is advisory.
  - **The companion-test extension table is this port's own addition, and it admits more than its prose
    discloses.** The source has no row for Rust, because Rust tests are separated by build configuration
    rather than by a name, and names the hazard: admitted shapes "compile into the shipped artifact", so a
    correction can ship unreviewed production code inside an approved candidate.
    `correction_companion_directory("tests", ".")` is true and this repository's genesis directory is `.`
    on nearly every candidate (`CHANGELOG.md` moves with most documented changes), so for any candidate
    touching a root-level file the entire top-level `tests/` tree is admissible on every repair. The mirror
    image: `correction_test_file_name("src/transport/target/tests.rs")` is false, so this repository's
    dominant unit-test location is admissible only when the candidate already touched that file. What
    bounds it: admitted lines are charged against the budget and every republish is re-reviewed in full.

- **Bounding the frozen manifest bounds the comment it lands in, by measuring the comment.** The publish
  call refuses `genesis-manifest-too-large` when `budget-genesis-paths`, percent-encoded, would clear
  `GENESIS_MANIFEST_CEILING` (32,768 bytes). `topology`, `topology_reason` and `run-id` are caller-supplied
  with no length limit of their own (the MCP argument definitions declare a type and a description, never a
  maximum), so `publication-comment-too-large` fires from `publish_with_sleep` after trimming and before
  the fetch when the caller's marker fields plus `topology_line` would exceed `COMMENT_BODY_CEILING`
  (65,536) minus `PUBLICATION_FIXED_OVERHEAD` (8,192, an upper bound on the prose template and every
  generated field other than the manifest's and the lane's, derived by enumerating the fields `published`
  writes) minus `GENESIS_MANIFEST_CEILING` minus the lane's measured width (`lane_comment_bytes`). Every
  bound has to be reserved or they do not compose, and the reservations are conservative (the manifest
  ceiling is charged even on a first publication) because a comment GitHub rejects leaves the branch pushed
  with no receipt. Nothing is pushed on refusal
  (`an_oversize_topology_is_refused_before_any_remote_write`, `tests/pipe/part_10.rs`). Measured in the
  percent-encoded quantity: the largest manifest across the 109 merge commits then in this history encoded
  to 1,452 characters over 64 paths, a second larger repository to 13,095 over 195 (4% and 40% of the
  ceiling); the encoded-to-raw ratio on real path sets is 1.03 to 1.10, while bytes outside the marker's
  safe set would sit at the 3x bound, which is why the check measures.

- **The budget's own tree diff runs isolated from every `.gitattributes`.** A local `.git/info/attributes`
  carrying `* -diff` marks every path binary, which `count_changed_lines` skips, and this repository's
  committed `.gitattributes` already marks `*.png`, `*.jpg`, `*.gif` and `*.ico` `binary`, so one more line
  would silence the budget. `diff_stats`'s `DiffIsolation::Isolated` reaches the child through
  `run_with_isolated_env`, which **replaces** the environment (a fresh `GIT_DIR` with no worktree, index or
  attribute source, and no `HOME` or `XDG_CONFIG_HOME`) rather than layering onto the caller's as
  `run_with_env` does, so committed and untracked attributes and the caller's global or system git config
  and default global attributes file are invisible (measured: `* -diff` in the default global attributes
  file made a two-line change count `-\t-` under layered isolation). Binary detection still runs on content.
  `review_tier`'s own `diff_stats` call stays ambient, and since a `low` answer lets `release_ci` release on
  the structural readback, that read moves a clearance and is out of this entry's scope.
  `transport::tests::essential_env_drops_home_and_keeps_path_case_insensitively` tests the filter and
  `tier::tests::a_default_global_attributes_file_does_not_reach_the_isolated_diff` the composed path (its rule
  is narrowed to one unused name because `* -diff` poisoned concurrent tests). **Deleting
  `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_COUNT`, `GIT_ATTR_NOSYSTEM`
  and `LANG` from `tier.rs` leaves the whole suite green.** `GIT_CONFIG_NOSYSTEM` and `GIT_ATTR_NOSYSTEM`
  alone stand between the isolated diff and a real system-wide install (proving it would mean writing to a
  shared, privileged path); the other four are redundant by construction (the replacement already drops what
  they intercepted, and `LC_ALL=C` shadows `LANG=C`). The source's `--text` flag belongs to an `Inspect` this
  port does not implement; the function `diff_stats` mirrors carries no `--text`, so a binary-content path
  is charged zero lines by the source too.

- **The review payload a judge is handed runs isolated the same way, and that moves a digest wherever a
  repository's own config sets a diff knob.** `review_payload_bounded` (`src/transport/target.rs`) builds
  the unified diff every judge reads through `isolated_diff_env` and `run_with_isolated_env`. Without it an
  attribute source marked every path binary and the payload shrank to one `Binary files differ` line per
  path, small enough that `REVIEW_PAYLOAD_CEILING` stopped binding. Isolation makes the payload independent
  of ambient diff configuration too, a behaviour change: `diff.algorithm`, `diff.interHunkContext` and
  `diff.indentHeuristic` are not attribute sources and none is pinned by `pinned_diff_args`, so for a
  repository whose own config sets one the bytes move once and an earlier `payload-digest` will not
  reproduce (measured: two one-line edits ten lines apart give 23 lines under an ambient
  `diff.interHunkContext=10` and 20 isolated;
  `a_local_diff_inter_hunk_context_moves_the_ambient_payload_but_not_the_isolated_one`). **No digest already
  on this timeline moved**, a fact about one machine on the day measured (no `diff.*` key locally or
  globally, the system `diff.astextplain.textconv` never invoked under `--no-textconv`,
  `core.attributesFile` and `core.bigFileThreshold` unset, no untracked `.git/info/attributes` or default
  global attributes file, and the committed `binary` marking renders the line content sniffing already
  produces); a fork or a contributor's machine could set something none of that measured.

- **Reading the lane before launching a panel is instruction, not enforcement.** `publication_lane`
  answers green, red, never-ran, running, absent or disabled (an unreadable listing or permissions answer is
  a failed read), and Step 5 routes on all of them. Nothing refuses a panel launched over a red lane,
  because a sub-agent launch is not a write the gate sees. `record_computed_verdict` still refuses an
  approval on a lane whose run genuinely ran and failed or is unfinished; the one case it does not refuse is
  a lane whose run never executed a step, gated on the caller's own evidence rather than on green.

- **Estigia starts the publication lane and cannot prove a repository has one.** The refusal in
  `record_computed_verdict` is real, but everything upstream is best-effort. `publish_review` discovers
  every active workflow under `.github/workflows/` (or takes `Publication lanes`) and dispatches each once
  per publication epoch. It **does not parse consumer YAML** for `workflow_dispatch`: a candidate without
  the trigger is told apart only by `HTTP 422` and dropped from the started set, not refused. Estigia sees
  which workflows exist and accepted the dispatch, never which a repository considers load-bearing, so a
  release lane or nightly is excluded only when named on `Publication lanes exclude`. A repository with no
  dispatchable candidate gets no check runs, and **a head with no check runs proceeds**, deliberately:
  refusing would break every consumer that has not adopted the lane. If the workflows listing cannot be
  read, `ci.yml` is tried, which is not a claim that every lane is called that. Eight narrower limits:
  - **The dispatch names a branch, not a commit.** A push over the branch between Estigia's push and its
    dispatch starts a lane on someone else's bytes; the published head then has no check runs and clears.
  - **`HTTP 404` is read as *no lane*, not *no permission*.** GitHub answers `404` for both, so a token
    that cannot see the workflow loses the protection silently; only `401` and `403` refuse
    (`publication-lane-forbidden`).
  - **Only `accepted` is gated.** A rejection over a red head records. `neutral` and `skipped` clear; every
    other conclusion, including none, refuses.
  - **More than one page of check runs (or of jobs, in `never_ran_signal`) is a failed read.** Both are
    asked at `per_page=100` and refused when `total_count` outruns what arrived, so a head with more check
    runs cannot record an accepted verdict until the read is widened (the jobs read answers `None`, which
    `classify_reds` reads as `Ran`). Fail-closed and untested at that size.
  - **The read side is fail-closed on a permission error, where the dispatch side is lenient.** A token
    that can push, comment and dispatch but lacks `Checks: read` publishes normally and can never record an
    approved computed verdict: `read-failed`, permanently. Its **detail** names the read, but the shared
    envelope still ends *retry the read*, advice no retry can take. Nothing detects the shape.
  - **A dispatched run is not visible the instant the dispatch returns**, so a verdict recorded inside that
    window sees an empty listing, and an empty listing proceeds.
  - **The listing is every check run on the head, not the lane's.** A coverage bot or scanner sitting
    `queued` refuses an accepted verdict naming a lane the reviewer cannot start, watch or clear.

  A dispatched run's check runs **do not** satisfy branch protection's required contexts on a pull request
  head. Measured on this repository (2026-08-27, head `ead3ca84`): `gh workflow run ci.yml --ref <branch>`
  completed success with three check runs carrying the required context names, yet `gh pr checks 110`
  answered `no checks reported`, `statusCheckRollup` was `[]` and `mergeStateStatus` stayed `BLOCKED`;
  toggling draft to ready produced a run with `event: pull_request` and the rollup went from 0 to 3. The
  lane is the cheapest disqualifying signal **in the Actions tab**, where `record_computed_verdict` reads
  it, not a claim about mergeability. The draft barrier still closes the merge hole: `release_ci` verifies a
  draft before marking ready, and GitHub refuses to merge a draft.

- **A marker field that is present but unreadable is refused by name rather than read as an absent one.**
  `ReviewTransaction::from_marker` and `CorrectionState::from_marker` answer absent, present-and-valid or
  present-but-unreadable (`published-transaction-unparseable`, `correction-state-unparseable`, naming the
  `epoch` and first offending `field`), before anything remote moves, and every reader of the lineage
  refuses rather than proceeding over a dropped publication. **The absent case is the designed migration
  path**: a marker with none of the budget fields reads `Ok(None)`; only one carrying *some* and failing to
  read them all is refused. Reachability is the weak part: markers are written and read by Estigia, `comment`
  escapes control-shaped prose and `all_publications` filters to self-authored, unedited comments, so **no
  malformed marker has been observed**. The two ways in are a partially written marker (the unbounded
  `budget-genesis-paths` failure lands after the push) and version skew between a swapped binary and an MCP
  server still holding the old executable. How often a base advance occurs during a review round is not
  answered (an audit counted 7 of 270 publications whose base was not an ancestor of the head: a floor for
  movement, not a measure).

## The review-only claim

- **A shell write is placed by where the session runs.** `harness::review_claim` reads a shell write
  the way `harness::assisted` does, by the session's directory, so a write run from a directory
  outside the repository that reaches into it by a path the gate cannot read is not seen. Boundaries
  and remote writes do not depend on this: they are refused wherever they run. Nothing here is a
  sandbox for an arbitrary network client.
- **A tool with no `run_id` cannot be held to the claim.** The refusal of other writing tools reads
  the pointer the call names. `comment` takes its `run_id` as optional, so a call that omits it is not
  attributed to this run and not refused for it. Nothing tells the server which session called it.
- **A dead holder holds until its horizon.** `pull_reviews` has no liveness of its own: a run that
  crashed keeps the pull request until the horizon it declared, and its worktree stays as a detached
  checkout `estigia prune` lists. No run takes it sooner, and no person command does either.
- **A tracker issue in the review's criteria proves no link.** Closing references and a key in a head
  branch are the author's own text. They choose among real issues, and the body is the tracker's, but
  an author can point the panel at the wrong one. A read that fails leaves the fixed lenses and the
  answer says `criteria: fixed`.
- **Generated hunks are read, not reproduced.** A review-only claim never regenerates a declared file,
  so a reader is all that stands behind one.
- **A review verdict is not atomic against a push, and is not a merge's evidence.** See
  [assisted review](assisted-review.md). Dismissal on push or retarget and a squash merge's new commit
  are the host's behaviour and were not exercised live; neither was any submission, whose tests run
  against a scripted `gh`.
- **Fork and self-approval are read, not proved.** `isCrossRepository` must read exactly `false` or the
  claim is refused. That an author cannot approve their own pull request is GitHub's documented
  behaviour and was not exercised live.

## Re-running a red lane

`review_status` offers, once per epoch, `gh run rerun <run-id> --failed` for a red lane
([`review-status.md`](review-status.md)). What the offer cannot prove:

- **The offer is recorded, not the re-run.** The store row (`store_meta`, keyed by repository, issue and
  epoch) is written when the offer is made, because Estigia does not see the agent run `gh`. An agent that
  asks twice before re-running is told the head needs a correction, and a re-run that was never started
  reads the same as one that came back red.
- **"Does not reproduce locally" is the agent's word.** Nothing runs the suite on the failing job's
  platform. A failure that exists only on a platform the agent cannot run (Windows, say) can pass
  the condition and still be a defect in the bytes.
- **"Outside the change's paths" is read from the failed log.** The offer is withheld when the log names a
  path in the diff. A log that names no source path, or a path the change does not touch while a changed
  file caused it, is offered a re-run. A green re-run after a failure in a test the change touched is not
  proof the change is sound.
- **One re-run per epoch, whatever the cause.** A second flake on the same epoch is a correction and a new
  epoch, paid again.
