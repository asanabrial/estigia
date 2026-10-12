## Recovering a worktree or a branch lock

Every stop in delivery.md names a directory or a lock and refuses to touch it. That is deliberate — the thing
being refused may hold work nobody has pushed — so the recovery is yours, in this order. It is the
same on Windows and POSIX except where noted.

**One stop is not a recovery case.** `case-insensitive-branch-collision` means the path this run
composed differs from a registered checkout's only by letter case, so on a case-insensitive
filesystem they are ONE directory — and the registered checkout may be a live run's. Nothing below
removes it. Rename one branch so the two differ by more than case, or point the worktree template at
a root where they do; do not treat it as an orphan.

1. **Look before deciding anything.** `git -C <path> status --short --branch` and
   `git -C <path> log --oneline @{u}.. ` show uncommitted and unpushed work. `git worktree list`
   shows what the clone thinks it owns; the lock file named in the stop payload shows which run
   claimed the branch and when.
2. **Preserve anything useful first.** Commit and push to the branch, or `git -C <path> stash create`
   and note the object, or simply copy the directory aside. Nothing below is reversible.
3. **Prove the holder is stopped — do not infer it from elapsed time.** The lock names the run,
   issue, branch and acquisition time, but deliberately carries no machine identity or PID it cannot
   authenticate. Use the run's tracker activity and the runtime/session named by `run_id`; if that
   evidence cannot establish that the holder stopped, do not remove the lock. **This is why nothing
   here expires on a timeout**: the case a timeout gets wrong is a slow-but-live run whose checkout
   is taken while it is writing into it, which is the corruption all of this exists to prevent.
4. **Remove the checkout cleanly.** `git worktree remove <path>` (add `--force` only when step 2 is
   done and you accept the loss), then `git worktree prune`. Deleting the directory by hand leaves a
   registry entry that keeps refusing the path; `prune` is what clears that.
5. **Remove a proven-stale lock** by deleting the file named in the stop payload. It is a plain JSON
   file under `<common-git-dir>/issue-flow/branch-locks/`. Only after step 3.
6. **Re-run `start-branch` unchanged.** It is idempotent: it re-reads everything and either creates
   the checkout or tells you what still refuses it. A lock naming your OWN run is your own retry and
   never blocks you — it is *adopted*, so the attempt that succeeds is also the one that releases
   it. A run that merely stepped past its own leftover lock would leave nobody entitled to remove
   it, and the next different run would be blocked by it forever.

Windows: `git worktree remove` fails while any process holds a handle in the directory — an editor,
a terminal, an antivirus scan. Close them rather than forcing. POSIX: a worktree whose path contains
newline bytes is legal and is handled by the NUL-delimited read below; quote paths in the shell.

## Migrating a worktree template that gained a scope

A `Worktree location` missing `<branch>` or `<run-id>` gets the in-memory sibling described in delivery.md,
and that is usually the end of it. The one hard case is a checkout at the legacy path that is
**still registered to a branch**: it may hold unpushed work, the scope it lacks
is exactly what would prove who it belongs to, and starting a sibling beside it would leave two live
checkouts of one branch. (Registered at all: a legacy checkout left detached carries no branch in the
registry and stops this just the same, reporting `occupied_by_branch: null`. An earlier version of
this sentence said a detached one does not stop it, which is measurably untrue.) `start-branch` stops
with `legacy-worktree-registered` and neither removes nor writes into it. Push or preserve its work,
`git worktree remove` it, then re-run — steps 1, 2 and 4 above. An unregistered leftover directory at
the legacy path blocks nothing, because the sibling is a different directory.

**Which scope was added decides whether the stop can fire.** A template naming `<run-id>` and not
`<branch>` moves on upgrade, and its raw path is what earlier builds created — so the stop fires
there. A branch-only template does not move at all: it composed `…~<run-id>` before and composes it
still, so there is nothing to stop. For a template naming **neither** — a bare directory — the path
does move, but the legacy path is the raw template, which no build created: what it created was that directory with
`~<run-id>` appended. So on upgrade, a run resuming such a branch is not stopped here; it is stopped by
`branch-checked-out-elsewhere`, which names the checkout still holding the branch, before any remote
state is written. The recovery is the same `git worktree remove`, and it is written here because nothing else
tells you.

**"Not registered" and "the registry could not be read" are different answers, and only one of them
is a clearance.** That question is answered by `git worktree list`, so how it is read decides
whether the paragraph above means anything. The read is NUL-delimited (`--porcelain -z`) because a
worktree path may legally contain spaces and, on POSIX, newline bytes — splitting the stream on
lines turns one such path into two fragments, and the surviving fragment names a directory nobody
owns. Every record is then required to be coherent before any of it is believed: a nonzero exit, a
stream that stops mid-record, a repeated path, a repeated field, a field this parser does not know,
an attribute with no record, a checkout with no HEAD or no branch/detached/bare state, and any
bare-plus-checked-out or branch-plus-detached contradiction are all refused as exit `3`.

The failure that motivates it is the quiet one: the read used to ignore its exit code, so git being
unable to describe its own worktrees returned exactly the same answer as a repository with nothing
checked out anywhere — and that answer is the one that lets a run write into a directory another
run already owns.

**A fresh worktree does not have the files git never tracked.** Everything gitignored — environment
files, secrets, credentials, local settings — is simply absent, and the failure it produces is
confusing rather than obvious: the tool starts normally and then dies on a variable it has never had
trouble with, in a tree that looks identical to the one that works. `start_branch` says so in its
answer; copying them across is still yours to do.
