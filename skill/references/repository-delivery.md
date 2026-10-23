# Repository delivery

Use this fallback when a repository does not define stricter isolation, integration, or review-unit
sizing policy. The
repository's instructions always win; this reference supplies a floor, not a competing convention.
The runtime contract still owns role and state decisions, and the
[abstract binding contract](../SKILL.md#hard-rules) selects
the operation implementation and exact tracker commands.

## Delivery invariants

- Treat the issue, diff, or review named by the caller as the bounded scope while still obeying local
  repository and routed domain rules.
- Keep the base checkout read-only. One issue uses one traceable branch and one isolated worktree.
- Renew ownership before the first repository write and every publish, merge, tag, or close boundary.
- Publish clean shared bytes as a draft before review. The receipt names epoch, PR, head, base, and
  complete-target digest; any republish invalidates prior review and CI evidence.
- Preserve unfinished work and its diagnosis. A handoff is safer than an unverified merge.

## Keep review units coherent

Aim for no more than the `Change size` row of the operator table in `SKILL.md` — changed lines in one
pull request — so an independent reviewer can inspect the
complete delivery target without losing context. This is a recommendation, not a gate. Use a larger
coherent pull request when splitting would create unsafe intermediate states, duplicate migrations,
or otherwise reduce review quality. Before review, the developer records that rationale on the issue
or review target; the independent reviewer assesses scope coherence and the risk of splitting. Size
never waives exact-SHA review, tests, CI, authority checks, or any stricter repository rule.

**The candidate is the publication, and no review is due before one.** Estigia reviews one candidate
per publication: the pull request `publish_review` records an epoch, head, base and digest for. A
commit is not a candidate — a verdict binds that receipt, and `record_computed_verdict` refuses one
without it — so nothing obliges a unit of work to be committed on its own as a review boundary, and
there is no "review due" question to ask between the first write and the publication. Its answer
would always be the publication step `SKILL.md` already states: publish, then transition to
`review`.

A change past `Change size` may be delivered as a chain of pull requests, each its own slice. A slice
is a coherent unit that can land on its own; its boundary is recorded on the issue before the first
write, with the commits it will carry. Each slice publishes on its own — its own receipt, its own
review and its own `release_ci` — under the one claim that covers the issue, and the transport keys
a receipt to its own pull request, so an earlier slice's receipt stays usable after a later one
publishes, and each slice's first publication records no lineage from another. Every slice but the last starts its branch with `no_link: true`, so its merge does not close the issue. Only `stacked-to-main` is taken: every slice's base is the default branch, and a chain
targeting another branch is declined because it conflicts with the merge-parents rule and
`target`'s base. `Change size` stays a planning heuristic, and a single larger coherent pull
request remains the ordinary answer when splitting would reduce review quality.

The reads that exist answer the questions that do: `review_tier` sizes
the panel a range licenses — with `parent_head` for a repair — and `review_status`'s `corrections`
answers whether the lineage of the issue's latest publication
has spent its corrections.

## Start from a fresh base

1. Read the repository instructions — conventionally `AGENTS.md` or `CLAUDE.md` at the root, plus
   anything they include — before choosing branch, worktree, integration, test, or merge behavior.
   **The repository's own rules win, always**, and this document is only the fallback for what they
   do not define. History can reveal naming style, but it cannot define concurrency safety.
2. Fetch the configured base and create the issue branch from that fresh remote revision. Record the
   branch and base through the selected binding as soon as they exist.
3. Do not edit or commit on the base branch. Move unpublished accidental commits to the issue branch;
   if base history was already shared, repair it without rewriting shared history.

The branch name should carry the issue identity because it survives local run attribution and joins
the eventual review and delivery record back to the work item.

## Isolate the worktree

Place each implementation checkout outside the base working tree and include the repository identity
in its parent path. A shared parent keyed only by branch can collide across repositories, while an
in-tree worktree pollutes status and ignore behavior for every run.

The worktree strategy MUST prevent two live checkouts from writing the same directory, and the path
is made unique per run **and per branch**. Both are needed and each alone is a collision: one run
working a queue takes several branches, and one branch may be taken by several runs. Relying instead
on the version-control system's registration of one common
issue branch was tried and does not hold: that check is a read followed by a write with no lock
between them, and two checkout processes have been observed registering one branch concurrently. A
path that is unique per run does not depend on winning that race.

**Registration is not ownership.** "This path holds this branch" is satisfied equally well by two
runs of the same branch, so a resume additionally requires durable evidence that THIS run created
the checkout. Absent evidence is not a permissive default — a registered checkout of your branch
with nobody's name on it cannot be told apart from one another run is writing into right now. Every
other case — a foreign checkout, an orphan directory, an unreadable ownership record — stops.

Tools with per-checkout indexes or caches get a fresh local instance rather than one copied or
linked from another checkout.

**Isolation covers every location the run writes, not only its checkout.** Scripts, intermediate
output, notes and saved measurements go somewhere of this run's own. A rule that binds the checkout
and says nothing about a shared scratch path has a hole the exact size of however many contexts share it.

Confirm isolation before the first edit and whenever the tree changes unexpectedly. Stop writing and
renew the claim. A losing run leaves the tree untouched and records where its own work is; the winning
run adopts useful foreign work instead of deleting it. Editing around an unexplained writer creates a
combined build owned by nobody.

Fresh worktrees contain only tracked files. Restore required ignored environment files, credentials,
and local settings before tests or tools depend on them. Reuse a proven interpreter or dependency
environment when safe, but keep the worktree as the working directory and ensure it loads that
worktree's code.

## Review the whole delivery, not the part you were looking at

**The diagnostic review target is the exact recorded base through `HEAD`, plus whatever is still uncommitted.**
Those two halves are easy to review separately and disastrous to review separately: an approval over
the committed prefix authorises nothing about the dirty suffix, and an approval over the suffix
authorises nothing about the commits already on the branch.

Derive the target once, mechanically, before any reviewer runs, and compare it against whatever the
reviewer is actually being given. Where the binding scripts it, that is `expected-target`: it emits
the path/mode/blob manifest and one digest over it, and fails closed when a supplied target differs
— naming the paths that would ship unreviewed. Mode is part of it, because a file that becomes
executable, or a regular file replaced by a symlink, changes the delivery while every content byte
stays identical.

**Ask for the manifest only when a path list is what you need.** It carries one object per tracked
file, so its size is set by the repository rather than by the change, and a caller binding or
comparing a digest never reads it. The default answer omits it and carries the same digest, and a
supplied target still fails closed against the same paths — so the comparison that matters is
unaffected. Pass `--manifest` (`manifest: true`) when a difference has to be read path by path, or
when the answer is being saved as the file a later `--native-start` compares — that comparison reads
the manifest and refuses without one.

A published receipt is narrower: it requires `uncommitted` to be empty. A PR head cannot identify
working-tree bytes, so publishing or releasing CI from a dirty checkout stops rather than binding a
digest the remote commit cannot reproduce.

## Do not merge the base as a routine step

**Branch from an exact, freshly fetched base and leave it there.** Merging the base branch back in
"to stay current" rewrites the review target for no reason, and invalidates a head that review and
CI are already bound to. What is required instead is to CLASSIFY later movement:

| The base moved and… | Then |
|---|---|
| it did not move | nothing to decide |
| every path it touched is disjoint from this delivery, and a read-only merge reports no conflict | leave the candidate branch alone when the repository lets you; under `required_status_checks.strict: true` the platform does not, so integrate by merging the base and republish over the parent receipt — a repair-shaped round whose panel reads the whole change, since a moved base is read whole however it was integrated, and re-reads the findings recorded against the parent receipt |
| it touched paths this delivery also touches, or the merge conflicts | integrate before final review and re-review the whole change |
| the merge result could not be established | integrate; an unanswered merge is never "compatible", and the whole change is re-reviewed |

Textual disjointness is evidence, not proof: two files that never overlap can still have to change
together. `target` reports refs, changed paths and the read-only merge result; whether the change
still MEANS what it did is a judgement, and it stays with the reviewer.

An operator may override any of this. When `~/.agents/AGENTS.md` exists it is the canonical
cross-runtime policy and it wins; this section is the default for when it does not. Its text is
deliberately not copied here — a second copy is a second owner, and the two drift.

## Integrate before publishing

Fetch the current base before publication; integrate it only when it moved, per the classification above. Resolve conflicts as code
changes: understand both sides, retain both intents where required, and rerun every affected check.
Choosing one side only to clear markers can silently delete a delivered fix.

Integrate by merging the base into the issue branch. A merge keeps the parent head an ancestor of the new
head, so a `compatible` movement's forced republish stays repair-shaped for the findings and the
correction count; the panel reads the whole change either way, because a moved base is read whole
whether it was merged or rebased. Rebasing rewrites commit identities
already used by review, CI, signatures, or deployments, and a published rebase can require a
force-push over work another actor fetched. Follow an explicit repository rebase policy only while no
evidence or shared consumer depends on the old identities.

When a rebase or an amend has already happened, that force-push is `republish_review` and never a
hand-run one: it leases against the head the last publication recorded, so a remote another actor
moved refuses the push instead of losing their commit, and it records a new epoch like any republish.
A non-fast-forward refusal from `publish_review` is the signal to use it, not the signal to leave.

## Publish, review, and verify

Publish the configured review target as a draft before moving work to review. Reuse its existing
review request when resuming; duplicate review targets split evidence. A reused ready PR must become
draft and be read back before push. A new PR is created draft after a topic push that does not trigger
CI. Capture epoch, PR, head, base, target digest and payload digest, and require every independent
verdict to identify that full receipt. `launch_review_input` builds each reviewer's payload itself, so ask for `inline_payload` only
for a reviewer you launch yourself (Judgment Day, a `handoff_review` host) and hand it over
as the primary evidence, naming the specific claims the change makes about text the payload does not
show and reserving the checkout for those. An oversize payload refuses
rather than truncates. Each observation worth carrying is recorded against that receipt as its own finding, classified
`severe`, `warning` or `suggestion` with concrete evidence and stated impact, before the verdict that
reduces them: a `severe` finding that stands makes the computed verdict `rejected`, and warnings and
suggestions stand beside an approval without withdrawing it.
After the lens reports and the lenses' findings are recorded,
the orchestrator records one computed exact-receipt verdict and `release_ci`
replays the newest recorded receipt on its own pull request, current draft PR and a clean target bounded by equal
HEAD/status observations before marking the PR ready and reading the outcome back. A republish creates
a new epoch and invalidates old evidence even for identical bytes; base movement also makes evidence
stale, and under `required_status_checks.strict: true` it forces an integrating republish. On a
`compatible` movement that republish is repair-shaped: the parent receipt and the findings recorded
against it are what the lenses re-read, over the whole change, and the republish spends no
correction when its parent was approved. Every other value integrates and re-reviews the whole change.

The repository compatibility requirement is explicit: PR CI starts on `ready_for_review`, not topic
push/open/synchronize/reopen. Beside it sits the **publication lane set**: every active
`.github/workflows/*` candidate that accepts `workflow_dispatch` (minus an optional exclude list, or
an explicit lane list that replaces discovery). `publish_review` and `republish_review` start each
once per publication epoch against the head they just pushed — not once per push — record which
files started and how many on the receipt, and dispatching does not mark the PR ready. Read
`review_status`'s `lane` for that head before launching the lenses: green, launch; red, do not launch; never-ran
(its jobs all report no steps), launch — gated on evidence rather than green; running, wait or
launch knowing the computed verdict may not be recordable; absent, proceed; disabled, CI cannot run, so proceed
and merge as *CI, merge and delivery* states for such a repository; unreadable, not clearance.
Nothing refuses a review launched over a red lane. `record_computed_verdict`
refuses an approval for a head whose lane is red or unfinished, so a platform-only failure is
found before the review rounds are spent rather than after. A repository with no dispatchable lane has
no check runs on its head, which proceeds unchanged: Estigia does not parse consumer YAML for the
trigger, and cannot prove a lane exists. GitHub cannot make the ready write conditional atomically, and
out-of-band collaborators or repository workflows can bypass the order, start other CI, push new
bytes, or forge comments. This is cooperative workflow ordering, not proof of panel size, concurrency,
independence, blindness, same-finding identity or quorum, and not malicious-collaborator authenticity.

The required CI set is the union of host-required checks and every applicable repository lane. Each
expected lane must exist once and succeed; absent, skipped, queued, cancelled, or failed work is not
green. A repository that requires the exact delivered revision also runs its post-merge gate.

Never bypass a failed gate, missing permission, conflicting rule, or unavailable reviewer. Published
work that cannot be delivered remains review work with the exact blocker, expected condition, and
next owner recorded, including ordinary delivery permission supplied by a named person. The exception
is a human adjudication that is not a routine wait on the ordinary review, CI, identity, and
delivery-permission gates: preserve the branch, PR, receipt, checks, and evidence, then record
`blocked` with the exact blocker or decision and its discharger as Developer step 7 requires before
releasing ownership.

## Merge and publish versions

Merge only when the head is the reviewed, green head and the base is the reviewed one or a cleanly
advanced one (binding `merge`), and bind the operation to that head. Use the configured strategy and verify the delivered revision's topology rather than
trusting the requested flag. A topology-preserving strategy retains branch ancestry; squash or rebase
creates different history and is valid only when repository policy selected it before review.

Delivery authorization belongs to repository or operator policy. A configured review route does not
silently grant merge permission, and an override does not cure a stale or failed gate.

After the delivered-revision gate, inspect every version changed by the diff. Publish an annotated,
immutable tag using the repository's one established product or component convention, then verify the
remote tag object and its peeled commit. Never move or force an existing version tag. Publish a host
release artifact only when repository history or configuration requires one, and build it from the
already-verified tag.

## Close with no publication

An issue that requires no change — a bookkeeping or publication task whose answer is a record
rather than a diff — reaches `done` without a publication, and this class is exceptional: it is the
one `done` with no receipt. `transition --to done` verifies the merge of the latest confirmed
publication when there is one and answers nothing to verify when there is none, so this close is
not refused for having no publication. The run records the close evidence exactly as any other
close does — the reason no change was required, and the name of the person or rule that decided it
— so a later reader sees the decision rather than inferring it from an empty diff. Its checklist
rows are done by an `evidence` note naming where the work is recorded rather than by a
`commit`, since there is no work-unit commit to name.

## Handoff and cleanup

Choose the destination the Decision Gates table in `SKILL.md` names. This file does not
own a second table of those rows.

Do not discard another run's branch, diagnosis, measurements, or ruled-out hypotheses. Record all of
them on the durable work item so the next run resumes rather than reconstructs.

Clean up only after delivery is verified or abandonment is explicitly recorded. `release` puts down
the isolated checkout this run created, and the local branch checked out there, once the tracker
recognises the issue's close as the registered delivery — the newest confirmed publication's
`published` marker, the closing
pull request, and a merge whose second parent is the reviewed head and whose first parent descends from the reviewed base — and the checkout carries no
unpublished work: its head is the delivered head and `git status --porcelain` is empty. Anything that
does not clear that bar is kept, and the release answer names the check it failed. A retried release
that finds its own marker already on the issue attempts the same retirement, so a release that posted
its marker and then failed before cleanup does not leave a delivered checkout behind. The delivering
run also deletes every **remote** head it recorded starting: `start_branch` writes a `branch` marker
naming it, and `release` deletes each recorded head and reads the remote back, because GitHub's
`delete_branch_on_merge` removes only the pull request's own head — a branch delivered through
another, or pushed under a different name, is never touched by the host. Each deletion is read back:
the remote answering nothing for the head is what makes it gone, not the push's exit code. Deleting
a merged head branch loses no audit evidence: the pull request, its review history, the merge commit
and any tag or Release are separate objects the host keeps. Preserve those.

**A delivery that skips `release` is reported, not refused.** `doctor`'s `stale-run-pointer` row
names a pointer whose issue closed behind a close Estigia recognises as the registered delivery, and its `prune` row names a branch
or checkout no live claim owns. `estigia prune` lists what either row named and removes it only on
`--confirm`; it never deletes what a live claim holds or what carries commits the remote does not
have. The two rows are red and block nothing: the skip is a leftover, not a reason a fresh run cannot
swear. The cost is paid later, by another run's first write in that checkout.

**A reviewer's directory is not a checkout, and `release` does not remove it.** The isolation rule
gives each Judgment Day judge a directory of its own, one per judge per review round. Estigia does not spawn
the judge: the orchestrating context creates each directory and is the only context that could remove it.
`release` reaches only the checkout `start_branch` created for the delivering run's own branch. Sweep
reviewer directories by hand once the judge that wrote one has returned its findings,
or let `estigia prune` list them — a detached checkout no branch owns is one of its cases — and only
after the same check `release` applies to its own checkout: `git status --porcelain` is empty and
every commit the directory holds is on the remote. A directory that fails that check is left alone
and named. Where they live is unsettled — they sit under the configured `Worktree location` today,
which is what makes them look like registered worktrees when Git considers them no such thing.
