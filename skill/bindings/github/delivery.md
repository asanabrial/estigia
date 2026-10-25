`publish-review` scans for a keyword **before its first remote mutation** — before it lists open pull
requests, before a reused one is drafted or its body refreshed, and before the push. Reads precede it —
the claim renewal and the base fetch — and no write does. The sources are
the commit messages `origin/<base>..<branch>` adds and the body it is about to write, both readable
locally, so that refusal leaves the remote exactly as it found it and says so.

*Before the push* is not far enough, and the difference is not academic: a reused pull request is
drafted and has its title and body replaced first, and a body carrying `Closes #<n>` is written
verbatim, so a refusal there has already published the hazard it exists to refuse.

The scan after the readback stays — a keyword can also arrive from the remote side of a pull request
this run did not write, and it settles a branch-derived link that needs the PR to exist. A refusal
from *there* carries `"world": "committed"`, which every surface reads as **the write landed; what
failed came after it** rather than as nothing having happened. Any refusal without that field means
what it always meant. When the keyword from *there* names another issue, the refusal carries
`"cause": "closing-keyword-elsewhere"` — the same cause `check-closing-keywords` reports, because the
remedy is read from the cause and a branch link, the cause `assess_autoclose` itself gives when no
keyword names this issue, is not one an edit removes.

A `git log` that does not answer is not a branch with no keywords: both this scan and
`check-closing-keywords` refuse on it rather than continuing with an empty list.

**An issue that is not finished when its PR merges is a separate case**, because the auto-close does
not care whether the delivery was the whole issue or one slice of it. Marking a partly-delivered
parent `done` because GitHub closed it is the failure; the continuation model — a verified renewal
under `--allow-closed-by-pr`, an explicit reopen, and a transition back into the state that should
survive — is in *Closing* below. Nothing there is a shortcut around this section: it exists so a
retained issue can outlive its own merge without ever being reported as delivered.

**Write this, so the wording is not improvised per PR.** Knowing the rule is demonstrably not enough
— `Fixes #<n>` is the muscle-memory opening of a PR body, and a run reaches for it while composing
the caveat that forbids it. The safe form:

```
Refs #<n> — a plain reference, deliberately NOT a closing keyword.
```

The forbidden set is not just `fixes`: GitHub honours **`close`, `closes`, `closed`, `fix`, `fixes`,
`fixed`, `resolve`, `resolves`, `resolved`**, case-insensitively, in a PR body or any commit message
on the branch. `Refs`, `Implements`, `Part of` and a bare `#<n>` all link without closing.
The scan refuses a keyword naming **any** issue, not only the one under delivery: `close #464` in this
branch's body closes #464 on merge, whose `done` transition then never runs.

**Then verify it mechanically, because reading your own prose is how it slipped through.**
`publish-review` runs the check as part of publishing and refuses to finish while a keyword is live;
`check-closing-keywords` runs it standalone. Run it again before merging — the branch's commit
messages can introduce one after the body is already clean. Editing the body does remove the link,
though the value can lag a few seconds, so re-run rather than trusting the first response.

**One consequence to state rather than rediscover.** A project's native *Linked pull requests* column
is populated by these references, so under `gh issue develop` it fills itself and under a
hand-created branch it stays empty. Either way, never add a keyword to populate it - that trades a
cosmetic gap for the one cause this section actually forbids. The durable joins are the timeline
cross-reference (`#<n>` in the PR body), the branch linkage, and the delivering SHA in the close
comment.

### The repository's daily schedule for pushes

An enabled git operation window denies `git push`, `git commit`, and local branch creation outside the
configured daily range in this machine's local time — `publish_review` is denied before its push;
`start_branch` makes the branch and worktree and queues the link. The refusal carries
`git-operation-window-denied`, and a write the transport meets inside a closed window `outbox-bypass`;
wait for the range, widen it, or set the row to `disabled`. File edits and worktree operations on an
existing branch are unaffected. A hook still cannot prove no push happened: `git push --no-verify`,
direct ref writes, hosting APIs, and other clones bypass it, so treat a quiet period as observed rather
than proven.

While the window is closed, `claim`, `transition`, `comment`, `create`, `unassign` and `start_branch` answer
from the local store and queue their GitHub writes; an issue created then carries a provisional number until
the queue is sent. The `flush` operation sends the queue in order once the window is open, and every write to
GitHub and every publication runs it first. A queue that does not drain refuses `outbox-pending`, naming the
write it stopped at and how many remain: settle that write, then run `flush` again.

## Branch, worktree and the linked issue

`gh issue develop <n> --name <branch> --base <base>` creates the branch **server-side, from the fresh
base, already linked** in the issue's Development sidebar — one command replacing branch creation AND
recording. A branch nobody can find from the issue is work nobody can follow.

`start-branch` runs it **last**, not first. It fetches, resumes an existing local or remote-only
branch without moving it — only a branch absent everywhere starts from the fetched base — creates
the worktree and records ownership, and only then links the issue. *Nothing is created remotely
until the local checkout is reserved*, below, is where that order and its reasons live; the summary
here exists so the sequence is not learned from this paragraph in reverse. It records branch, base
SHA, worktree path and holding run-id on the issue either way.

The worktree path comes from the `Worktree location` configuration row, with `<repo>`, `<branch>`,
`<issue>` and `<run-id>` substituted. The repository name and the issue number are flattened, so an
`owner/repo` repository identity does not create a stray `owner/` directory under the worktree root.

**`<run-id>` is validated and `<branch>` is encoded, neither flattened**, and the difference is the whole
isolation guarantee. Flattening is not injective: `a/b` and `a-b` are two distinct valid run
IDs that flatten onto ONE directory, and the run that arrives second finds a registered worktree
carrying its own branch at its own path — the exact shape of a legitimate resume. So a run ID must
already be a safe directory name and is refused otherwise: lowercase `a-z0-9` groups joined by
single `-`, nothing else. Each exclusion is a collapse rather than tidiness — separators restructure
the path, uppercase folds into lowercase on Windows and macOS so isolation would depend on the
filesystem, and Windows silently strips trailing dots and spaces so `run-a.` and `run-a` are one
directory to the OS and two keys to us. Reserved device names (`con`, `nul`, `com1`…) are refused in
**every** substituted component, because they name a device rather than a directory in any case and
with any extension.

**`<branch>` is encoded, not folded.** Folding separators is not injective: `fix/1-a`, `fix-1-a`,
`fix//1-a` and `fix\1-a` would all flatten onto `fix-1-a`, and the four take four different branch
locks, so nothing would notice them sharing one directory. So `<branch>` is substituted through one
escape, `+`: `+` becomes `+p`, `/` becomes `+s`, `\` becomes `+b`, everything else is unchanged.
`docs/931-hold` lands in `docs+s931-hold`; a branch with none of the three keeps the component it
always had, so an existing checkout still resolves. `+` is safe in paths and file URLs, where `%` is
mishandled by several build tools. The repository name and the issue number stay flattened: the issue
is a `u64` that cannot carry a separator, and the repository name is `owner/repo` as the tracker
spells it, so refusing its separator would refuse every ordinary call.

**Case and normalization are the second half of the same rule.** Two branches differing only in case
— `Fix-1-a` and `fix-1-a` — or only in Unicode normalization — `café` composed and `café` decomposed
— compose two strings that are ONE directory on a filesystem that folds either (case on macOS and
Windows; normalization on default APFS, which is normalization-insensitive). The run ID is lowercase by
construction, so such a difference can only enter through the branch. `start-branch` refuses it as
`case-insensitive-branch-collision` when the path it composes matches a registered checkout's path
once both are folded, naming both branches and telling you not to delete the registered checkout. The
fold is over the spellings, not the filesystem, so a volume that distinguishes them refuses too:
there the two spellings are two directories, and refusing them is the conservative direction rather
than leaving the hazard standing on the volumes that fold.

**What the path must guarantee is that no two live checkouts share a directory — across runs and
across branches.** Those are two dimensions and a template missing either one collides. Without
`<run-id>` every run of a branch lands in the same directory; without `<branch>` every branch of one
run does, so a run working a queue meets the checkout it made for its previous issue on its second
issue, every time.

Every template is now made to carry both: a missing placeholder is added **in memory** as a sibling —
`…/<branch>` becomes `…/<branch>~<run-id>`, `…/<run-id>` becomes `…/<run-id>~<branch>`, and a
template naming neither becomes `…~<branch>~<run-id>`. `estigia.local.md` is never rewritten: a transport
command that silently edits the operator's own policy file is a worse failure than the one it fixes,
and that file may be shared across machines where the migration is not wanted. The result reports
`template_migrated` so the substitution is visible rather than assumed.

This matters most for the template an operator is likeliest to write. The `Worktree location` row
accepts *"an absolute directory"* and the skill ships it `unset`, so a bare path with no placeholder
in it is the ordinary answer — and it is the one shape that collides in both dimensions at once.

A sibling and not a child, so that a legacy checkout is neither a parent nor a child of the new one:
colliding inside a directory somebody else owns is the entire defect. The branch dimension needs that
more sharply than the run one — a run-scoped legacy path **is** a checkout that run owns, so nesting
the new one under it would put a worktree inside a worktree.

**The migration joins the two halves with `~`, and that is not cosmetic.** Joining with `-` makes
the composed name ambiguous even though each half is unambiguous on its own: branch `fix-6` with run
`a-b` and branch `fix-6-a` with run `b` both spell `fix-6-a-b`. Those are two *different* branches,
so they take two different branch locks, and nothing downstream would notice them sharing one
directory. Git rejects `~` in a ref name and the run-ID alphabet rejects it too, so it appears in
neither value being joined: a template that gains both scopes composes three parts across two joins,
and every part still ends where the next begins. Nothing reads the composed name back apart; what the
property buys is that two different pairs cannot spell one directory. If you write your own template,
join `<branch>` and `<run-id>` with a character that cannot occur in either — `~` is the one this
binding uses.

Relying on git to catch the collision instead was tried and does not hold. `worktree add` refuses a
branch already checked out elsewhere (`fatal: '<branch>' is already used by worktree at …`), but that
is a read followed by a write with no lock between them, so two concurrent runs can both succeed. Git
loses the race its check exists to win, so `start-branch` serializes on its own **branch-scoped
lock** first (below).

**So `start-branch` asks whether the directory is YOURS, and requires proof.** "The registry says
this path holds this branch" is not proof: two runs of one branch satisfy it equally well, which is
how the second one checks out on top of the first. So the registry answers only half, and a durable
**ownership marker**, written by the run that created the checkout into that worktree's private git
admin directory, answers the other half. It lives there rather than in the working tree because a
marker that shows up in every `git status` is a marker people delete, and `git worktree remove`
clears it along with everything else about the checkout, so a clean removal cannot leave a stale
claim behind.

Eight outcomes, and only two of them proceed:

| What is at the path | Outcome |
|---|---|
| nothing | fresh checkout |
| the composed path matches a registered checkout's path once folded, differing only by case or normalization | stop — `case-insensitive-branch-collision` |
| nothing, and another worktree holds this branch | stop — `branch-checked-out-elsewhere` |
| a registered worktree for this branch, marker names THIS run | `resumed_existing_worktree` |
| a registered worktree for this branch, marker names another run | stop — it may hold unpushed work |
| a registered worktree for this branch, **no marker** | stop — `worktree-ownership-unproven` |
| a registered worktree for this branch, marker present but unreadable | exit `3` — a failed READ |
| anything else (stranger's checkout, orphan directory) | stop — `worktree-path-occupied` |

The third row is the one a run that **inherits** an issue meets. The path is keyed to the run
id, so a successor never lands on the checkout its predecessor made: the first row answers
*fresh*, and git is then asked for a branch another worktree already holds. It refuses with
`fatal: '<branch>' is already used by worktree at '<path>'` and exit 128 — no reason to branch
on, and no way out. The stop names the holder instead. It does not adopt that checkout: the
worktree is the unit of task identity, and resuming another run's directory is the operator's
call, not this command's.

The first row is decided before asking git for a private admin directory: a checkout that has not
been created has no directory in which `git rev-parse --git-dir` can run and no ownership marker to
read. Existing checkouts still fail closed on an unreadable marker as the rows below require.

The seventh row is not a pedantic distinction. A marker that exists and cannot be parsed says the
ownership question went UNANSWERED, and an unanswered read is not a permissive default any more
than a missing one is — it is the same fail-closed rule the registry read follows.

The sixth row is the one worth defending: an unproven claim is not a permissive default. A
registered checkout of your branch with no marker cannot be told apart from one a run is using right
now, and the recovery for it is documented in recovery.md rather than guessed at.

A worktree directory that is itself a symlink or junction is refused as a configuration defect
(`aliased-worktree-path`), since two run IDs pointing into one real directory defeats every rule
above. An *ancestor* being a link — a worktree root parked on another volume, `/tmp` on macOS — is
ordinary and is resolved rather than refused: it redirects every run's directory identically, so it
cannot fold two runs together.

**Two junctions that fold two runs together are caught, but not here — and the distinction is worth
stating because three attempts to catch it here all shipped defects.** With a hand-written
`…/<run-id>/checkout` template, links at `run-a` and `run-b` both pointing at one directory collapse
two runs while every leaf resolves cleanly. That is not a property of one path: the other run's
junction does not appear in your spelling, so no inspection of it can answer the question. Each
heuristic that tried failed in a different direction — refusing a link that redirects every run
identically, refusing a path that merely mentions the run ID twice, and failing open whenever the
run ID happened to survive elsewhere in the path.

What answers it is **resolution followed by ownership**, and resolution happens twice on purpose.
`canonical_worktree_path` resolves the path before anything is created, so the checkout is made and
registered under one spelling; `normalise_path` resolves again when the registry is consulted, so
the lookup compares real directories rather than spellings. Either one alone is enough to make two
aliased spellings meet — established by mutation, not by argument: breaking either resolution on its
own leaves the collapse still refused, and only breaking BOTH degrades the answer, and then only to
`worktree-path-occupied`, which is still a refusal. The redundancy is the point; neither resolution
is load-bearing alone, which is exactly what makes the property hard to remove by accident.

Once the two runs name one directory, the ownership marker decides: it names the first run, so the
second is refused as `worktree-owned-by-another-run` — from evidence rather than inference, and a
more accurate message than any path heuristic could have produced. A single path refuses only what
a single path can prove: its own leaf being an alias.

That refusal is exact for the case it names — two runs on ONE branch, sharing a branch lock, the
second arriving after the first recorded ownership. Two runs on *different* branches take different
locks, and a template that aliases them onto one directory is an operator misconfiguration rather
than a race. The loser is still refused, as `worktree-path-occupied`, because the directory it
resolves onto holds a checkout of somebody else's branch. Git's own "already used by worktree"
guard cannot help there: it compares branches, and these are two different branches.

### Nothing is created remotely until the local checkout is reserved

The order of operations inside `start-branch` is a guarantee, not an implementation detail. The
branch lock, the path decision, the local branch, the worktree and the ownership marker all happen
**before** the first call that mutates GitHub. Reversed,
a run created a server-side branch and a sidebar link, then stopped on the local check, leaving
remote state advertising work no checkout was ever made for.

Once local isolation is reserved, `gh issue develop` runs. A **nonzero exit from it is not an
answer.** It mutates two remote things at once, a ref and a link, and its failures are not one
thing: "a branch of that name already exists" (the ordinary resume, where the link may well be
present), a partial failure that created the ref and not the link, and a connection that dropped
after the server had committed both. The exit code cannot tell them apart, and in the last two
"not linked" is simply false. So a nonzero exit and a timeout are treated identically — **re-read
the Development sidebar** and let the read decide:

| Re-read says | Outcome |
|---|---|
| the branch is linked | success, `link_outcome: already-linked` |
| conclusively not linked, and the command exited nonzero | proceed without a native link |
| conclusively not linked, after a **timeout** | ambiguous write, exit `5` — a ref may exist right now |
| the read itself failed | exit `3`; after a timeout, exit `5` |

One asymmetry inside that table is deliberate: only the ABSENT answer needs a complete page. Finding
the branch in the sidebar is conclusive whatever else the connection holds, because a later page
cannot un-link it, while concluding it absent rests on having seen everything. So a truncated
connection that contains the branch is a yes, and a truncated connection that does not is a failed
read — refusing both would turn an answered question into an unretryable exit `3`.

Existence probes follow the same rule. `git rev-parse --verify --quiet` was used before and cannot
answer the question: it exits `1` for "no such ref" AND for a corrupt object store, an unreadable
`.git`, a broken packed-refs file — every way the question can go unanswered. `git for-each-ref`
separates them, because a successful exit with no output is a real absence and anything else is a
read that did not answer. A run that reads "absent" from a failed read creates the branch again from
the base, and a resumed branch silently restarts from zero.

**Its argument is a pattern, not a path, so the refname is matched again on the way out.** A
`for-each-ref` pattern matches a ref completely OR from the beginning up to a slash, so
`refs/heads/foo` also matches `refs/heads/foo/bar` — and asking about a branch `foo` that does not
exist would otherwise return the child's object id. That is not an unhelpful answer but a wrong one:
`foo` would be reported as present at a commit belonging to another branch. Only an exact refname
counts; children are ignored rather than mistaken for the parent.

Finally, **a successful native creation must tell one story.** The named base is fetched before its
remote-tracking ref is recorded or used as the local start point; otherwise a stale clone creates
locally from yesterday's base while GitHub creates remotely from today's. The local head in that story is read
from the isolated worktree, not from the caller's main checkout: the latter may legitimately be
ahead, behind, or carrying unrelated work and says nothing about the new branch. `gh issue develop` branches from the
base as the SERVER sees it, which is not necessarily the base this run fetched and recorded. If the
base moved in between, the branch starts at a commit the run never saw while the command reports the
recorded base as its own — and every later `Reviewed-Base:` claim inherits that. So a fresh creation
requires local head, published head and recorded base to be equal, and a resume requires the
published head to be reachable from the local head. Local ahead of remote is ordinary unpushed work.
A published head that is NOT reachable from the local one is refused as
`remote-not-reachable-from-local` — named for what was established rather than for the likeliest
cause, because it covers both somebody pushing ahead of you and somebody rebasing or force-pushing
the branch out from under you, and pointing the reader at commits to fast-forward that may not exist
is worse than saying less. Either way, continuing would build on a head this checkout has never seen.
An ancestry that could not be established is not a pass.

A coherence stop is the one refusal that comes *after* a remote write, necessarily: the disagreement
cannot be seen until the branch has been published. Nothing is reported as delivered, but the branch
does exist remotely, so the payload says so rather than implying a clean slate — re-running resumes
it as it now stands.

## CI, merge and delivery — the agent's half

CI interpretation and merge are not scripted. Releasing the exact reviewed draft to CI is mechanical
and is the `release_ci` operation in github.md; deciding that the configured review evidence is sufficient
remains with the agent because Estigia cannot observe whether the lenses ran or were blind.

**`deliver` runs the steps that need no judgement.** After `publish_review`, a run whose verdict is
computable calls `deliver`: it records the computed verdict, releases CI, waits for it, merges at the
reviewed head, reads the topology back and closes, and stops at the first step that needs a decision,
answering `stopped` with the `step`, a `reason` and `review_status`'s `next`. The hand-backs are
`receipt` (none held, or it moved), `review` (a verdict that is not `approved`, or a next that needs a
context), `authorisation` (`Merge by` is not `agent`, or `Delivery authorisation` is not `auto`),
`ci` (`ci-red`; `head-moved`; `ci-absent`, no check ten minutes after the release; `appendix-required`),
`base` (`base-advanced`: the base moved and does not merge clean; `base-unreadable`: the merge test
itself failed; `base-fetch-failed`: the base could not be fetched), `version` (`version-changed`, so *Version tags* applies, a rename included;
`version-unreadable`: a changed manifest could not be read), `merge` (the gate's refusal, GitHub's, or
a queue or approval rule that left it unmerged; `merge-not-covered`: no claim of this run covers the merge;
`merge-unreadable`: merged, with no merge commit named; `merge-unknown`: no read-back answered, so the next
call re-reads and never re-merges), `close` (`evidence-unwritable`: the evidence file could not be written)
and `appendix` (`appendix-unreadable`: `appendix_file` could not be read). At any step that reads the pull
request, `pull-request-unreadable` hands back the read that failed. A step that has to wait answers `pending` with `ask_after`
instead of blocking past 50 s. It reads the pull request's whole rollup as the required set, whatever the lane says (a
`not-consulted` lane is no evidence CI does not run), though on that lane an empty rollup passes at once
rather than waiting; only where the lane reads `disabled` or
`never-ran`, when no CI can run, does it skip the rollup and need `appendix_file` with the local suite command and
result, which is appended to the generated evidence.

**Before `ci_status`, call `release_ci` with the latest reviewed receipt.** It records the release for
that exact receipt, and the `gh pr merge` boundary refuses a merge whose receipt it never released,
whichever route took the pull request out of draft. A compatible repository
starts PR CI on `ready_for_review` and not on topic push, open, synchronize, or reopen; Estigia's own
`.github/workflows/ci.yml` is the example. Any active workflow under `.github/workflows/` that accepts
`workflow_dispatch` is a **publication lane** candidate: `publish_review` and `republish_review`
discover that set (or take an explicit `Publication lanes` list), start each once against the head
they just pushed, once per publication epoch and not once per push, and record which files started
and how many on the receipt. Dispatching does not mark the pull request ready. A workflow that must
not run per publication (a release lane, a nightly) is named on `Publication lanes exclude`.
`record_computed_verdict` then refuses an `approved` verdict for a head whose check runs are red or
unfinished, so the cheapest disqualifying signal comes before the most expensive spend. A repository with no dispatchable candidate gets the old ordering:
Estigia cannot prove a lane exists, and absence still proceeds. `publish_review` makes a
reused PR draft and reads that back before push, or creates a new PR with `--draft`; the lenses can
therefore run before ordinary cooperative CI. `start_review` and `record_verdict` fold the
transition, launches and verdict into them; `release_ci` stays its own.
`release_ci` replays the live claim, the newest receipt on its own pull request, draft PR and coherent target, then
marks ready and reads the result back.

**`release_ci` answers whether CI will run.** Its answer carries `lane`, the reading `review_status`
gives, and `next`: `merge` when `lane.state` is `not-consulted`, `disabled` or `never-ran`, so run no
`ci_status` and watch no checks; `ci_status` for every other reading, an unreadable one included.
`ci_released` is `true` only then.

**Draft toggle is the flow's job, not a manual gesture.** `publish_review` / `republish_review` call
`ensure_draft` before the push so a ready PR cannot emit the per-push pull-request event and expose
the new head before the review barrier is restored; `release_ci` marks ready afterwards. An operator may still
toggle draft → ready by hand to get checks onto a head; the flow does not require it. The publication lane's check runs do **not**
populate the pull request merge box; only a `pull_request`
event does. The workflow also carries `merge_group:` so a merge-queue entry can run the same suite
after ready; whether that run satisfies required contexts is a live-repository
measurement, not something Estigia asserts from documentation.

GitHub cannot atomically say "mark ready only if head/base still equal these values". A collaborator
or repository workflow acting outside Estigia can mark ready, push, start another workflow, or forge
comments, and a repository may configure additional triggers Estigia does not parse. This is a
cooperative ordering contract, not a security boundary or malicious-collaborator authentication.
Estigia does not inspect arbitrary consumer YAML with weak substring checks and cannot prove the lenses
ran, were independent, or were blind.

**CI is a delivery requirement only where it can run.** Three cases make it not one, and each is
read rather than declared. `review_status`'s `lane` answers `disabled` when
`repos/<owner>/<name>/actions/permissions` reported `enabled: false`, or when every workflow under
`.github/workflows/` is `disabled_manually`; it answers `never-ran` when every distinct workflow run
behind the head's failing check runs reports `steps: []` on every job — commonly a billing or
spending-limit failure — and it carries GitHub's own annotation of why, when there is one. In all
three the merge rests on the approved computed verdict for the exact receipt and on the local suite run over
those same bytes, never on a lane that did not run. `not-consulted`, the repository's own `Publication
lane` row set to `off`, is the fourth: it needs no comment, because the row is the repository's recorded
decision and the comment records an observation the run made, which a lane that was not read does not
give. For `disabled` and `never-ran`, before merging, the run records what it read,
verbatim, as an issue comment (`SCRIPT comment --issue <n> --run-id <id> --kind note --body-file
<f>`): the `read` field of the `disabled` answer, or the `annotation` field of the `never-ran` answer
when it carries one, exactly as the answer carried it, beside the receipt's epoch, head and base and
the command and result of the local suite. A run that executed and failed, and a
`cancelled` conclusion, stay red: neither this rule nor `review_status`'s `lane` `disabled` state
reclassifies them, and repository guides do not restate any of this.

**`ci_status`.** Capture `headRefOid` and `baseRefOid`; run `gh pr checks <pr> --watch --fail-fast`,
then separately read `gh pr checks <pr> --json name,workflow,state,bucket` (`--watch` and `--json` are
separate invocations). Build the expected set as the union of host-required names and every
applicable repository-required lane. For the host set, `gh pr checks <pr> --required --json name`
returning exactly `no required checks reported on the '<branch>' branch` means the empty set; every
other command error fails closed. When repository policy names workflow job ids rather than visible
checks, resolve each id through `jobs.<id>.name` in the workflow file at the captured head SHA before
comparing; never compare ids directly with `gh pr checks.name`. Each expected visible name must be
present exactly once with `bucket=pass`; `skipping`, `cancel`, a missing name or an unexpected
duplicate is not green. Re-read both SHAs afterwards; if the head changed, or the base moved other than as `merge` allows, discard review and CI.

**`merge`.** Require `reviewed head == CI head == current headRefOid`, and `baseRefOid` either the
reviewed base or a descendant of it that GitHub reports `MERGEABLE` and not `DIRTY`: `review_status`
answers `released` only then, and stops while GitHub is still computing it (unknown is never
clean), so ask again. CI is not repeated on an advanced base. Then run
`gh pr merge <pr> --merge --match-head-commit <head-sha>`. Never use `--admin` or `--delete-branch`:
deletion can fail after a successful API merge in a multi-worktree checkout and make the result look
retryable. If a merge queue owns delivery, first prove its configured method preserves merge commits
or stop. Poll `gh pr view <pr> --json state,mergedAt,mergeCommit` until merged and take
`.mergeCommit.oid` as the delivered SHA; fetch it and verify it has exactly two parents, that its
second parent is exactly the reviewed head, and that the reviewed base is an ancestor of its first
parent. A second parent that is not the reviewed head, or a first parent unrelated to the reviewed
base, means the host used another strategy or the history was rewritten: leave the issue in `review`
and invoke the repository's fix-forward/revert policy. A first parent that is not the reviewed base
but descends from it is a forward integration under concurrency, not a mismatch. If
exact-delivered-SHA CI is required, locate every required workflow with
`gh run list --commit <merge-sha> --workflow <workflow> --json databaseId,headSha,status,conclusion`,
wait with `gh run watch <id> --exit-status`, then verify
`gh run view <id> --json headSha,status,conclusion,jobs`: the head must equal `<merge-sha>`, the run
and every applicable required job must be `completed/success`, never `skipped`.

**Delivery merges with a merge commit on this path.** The `Merge strategy` row is
retired: an installed row still loads and changes nothing, and a new write is refused
by name. The topology is hard-coded, so a delivered SHA that is not a two-parent
merge refuses `delivery-topology-mismatch` naming the strategy — do not treat it as a failed claim
read.

**Run `SCRIPT verify-claim` again immediately before merging, before publishing tags, and before
closing.** Merge, version publication and close are separate irreversible boundaries, and a long
quiet phase between them cannot be allowed to bypass the renewal.

### Version tags

**`publish_version`.** After every required delivered-SHA gate, renew the claim BEFORE the first
publication write, then detect each component whose declared version changed from `<old>` to `<new>`.
Enumerate `git ls-remote --tags origin` — never local tags — to derive the component's established
naming convention and previous component tag. Require exactly one convention; if history is empty,
require an unambiguous single-product/component classification before using `v<new>` or
`<component>/v<new>`.

**A tag carries what a human wrote about the version.** Before creating it, extract the version's
changelog entry:

```bash
SCRIPT changelog-notes --version <new> --file <component-changelog> --out <notes-file>
```

It **fails closed** when the version has no entry, or has a heading with nothing under it. That is
not an obstacle to route around: the entry is part of what "delivered" means, and a tag is immutable,
so notes improvised at tag time are permanent. Write the entry first: the agent cutting the version writes
it, a few one-line bullets, from the pull requests merged since the previous tag.

The extraction anchors on the version OPENING the heading, which matters more than it sounds. Seen
live: an entry headed `### 2026-07-25 — (sin bump de versión) … (sigue en v6.9.8)` — whose entire
point is that 6.9.8 did *not* ship in it — was matched for 6.9.8 ahead of the genuine entry by a
looser pattern. That would have produced a tag whose notes describe a different change and disclaim
the version they are named after.

For `<tag>`, inspect both local and remote direct plus peeled refs. A valid annotated tag has a tag
object and a `refs/tags/<tag>^{}` target equal to `<merge-sha>`; a lightweight tag, another target or
ambiguous state blocks without rewriting anything. If the remote tag is absent, reuse a matching
annotated local tag or create it with `git tag -a "<tag>" -F "<notes-file>" "<merge-sha>"`, then
attempt `git push origin "refs/tags/<tag>"`. After ANY push result — success, rejection or timeout —
re-read the remote: a conclusive peeled target equal to `<merge-sha>` is idempotent success, another
target is conflict, and no conclusive read fails closed.

When GitHub Releases are required, first reverify the remote peeled target, then query
`gh release view "<tag>" --json tagName,isDraft,isPrerelease`. A matching non-draft Release with the
SemVer-derived prerelease flag is idempotent success; incompatible metadata blocks. Only a confirmed
not-found result permits `gh release create "<tag>" --verify-tag --notes-file "<notes-file>"` — the
same notes the tag carries (add `--prerelease` for a prerelease).

**Do not use `--generate-notes`.** It substitutes a list of commit subjects for the entry a human
wrote, which reads like documentation without being any — and it does so silently, so a component
whose changelog was never updated still gets a Release that looks complete. The `changelog-notes`
failure is the signal you want there. After ANY create result, re-read the Release JSON and the remote peeled
tag; both must match the expected metadata and `<merge-sha>`. Auth, network or API ambiguity fails
closed. Tag or Release failure leaves the issue in `review` even though the merge already exists.

A Git tag and a GitHub Release are different artifacts: the tag is mandatory; the Release is created
only when the repository already publishes them for that component. Never move, overwrite or
force-push an existing version tag — a tag that already points elsewhere is a delivery blocker, not
permission to rewrite release history.

### Closing

`close` moves the state to `done` **first**, then posts the closing note, then a bare `gh issue close
<n>`. A run's own delivery does the first two as one `release` from `review` (operations.md,
*Releasing an issue*).
`gh issue close` has NO file variant (`-c/--comment` is inline-only), so the note goes through the file-based comment path and the close carries
no body at all.

**Run the transition even when GitHub already closed the issue.** Under `gh issue develop` the merge
auto-closes it (see *Auto-close, and why it does not end the work* in github.md), and the temptation is to skip
the transition because the issue is already closed. That is precisely the failure: the auto-close
moved no label, so skipping leaves a CLOSED issue wearing `status:review`. `transition` is
idempotent, so running it against an
already-closed issue costs one call. The final `gh issue close` is then a no-op and may report the
issue as already closed — that is success, not an error.

**`done` verifies the merge.** With a confirmed review receipt on the issue, `transition --to done` first
reads the receipt's pull request and is refused as `delivery-not-verified` unless it is merged by a merge
commit whose second parent is the reviewed head and whose first parent descends from the reviewed base. The
refusal names the merge commit and its parents. It runs after the merge and after the spend is recorded, needs no closing
reference, and the answer carries `delivery` (pull request, merge commit, parents). No receipt, no check.

That is the **final-delivery** path. An intermediate slice of a retained issue must not borrow it.
Start its branch with `start_branch` `no_link: true` (`--no-link`): nothing links the branch, its merge
leaves the issue open, and `check_closing_keywords` answers `will_autoclose: false`. The last slice
starts linked. For a slice whose branch is already linked: record the workflow state that should survive before merging, renew with
`verify-claim --allow-closed-by-pr <pr>` after the linked PR closes the issue, reopen it with
`gh issue reopen <n>`, then transition from `review` into that recorded retained state and read it
back. Never transition an intermediate slice to `done`. A different closer, truncated connection,
or malformed pagination refuses the renewal, so none of those cases grants permission to reopen.

**The renewal's proof can fail on a merge that is still this run's own.** A branch link closes the
issue on the **first** of several merges, and that merge's shape need not match the receipt the
publication recorded — so `--allow-closed-by-pr` refuses even though the closer is this run's own
delivery. A write then hits `issue-not-open`, and that refusal names the same continuation rather
than calling the close somebody else's: reopen with `gh issue reopen <n>`, then transition back into
the retained state. The run still holds the issue; the close never took it away.

Two calls means a partial-failure case: if the comment lands but the close errors, the issue is left
open with its closing note already posted — retry only the bare `gh issue close <n>`, never re-post
the note.

`deliver` writes the closing note itself: the receipt, the tier and its lens roles, the computed
verdict, the findings by class, the checks receipt when one exists, the CI summary, the merge commit
and its parents, the spend and the run id, under a one-line summary in `Summary language`, then the
agent's appendix. It closes the GitHub issue if the merge left it open and reads it back.

Carry your run identity in the closing comment and state what was actually verified: review verdict,
CI run or checks, measured numbers, tests run, PR, **the delivering commit SHA**, and every version
tag or Release published. Never write just "done"; the state already says that, and the comment
exists to show what earned it.

**A shared GitHub account cannot manufacture independent approval.** The workflow's reviewer is a
fresh reasoning context, but GitHub sees the authenticated account, not that context. When the same
account authored the PR, record the reviewer verdict and evidence in the PR or issue with
`Reviewer-Run: <run-id>`, `Reviewed-Head: <full-head-sha>` and `Reviewed-Base: <full-base-sha>`; do
not claim a native `APPROVED` review that GitHub refused. Re-read both PR SHAs after the verdict —
matching prose without that race check is still stale evidence. This comment proves the workflow
review only; if repository protection requires a native approval, it does NOT substitute for one and
delivery stays blocked until a distinct identity supplies it.
