# Several assisted claims in one session

Under `Claim workflow: assisted` a session normally holds one issue, in one worktree. An orchestrator
that fans out one subagent per issue holds several: `assist` with `several: true`, once per worktree.
What the gate cannot tell about this is in [`honesty.md`](honesty.md), with the measurement it rests on.

## The shape

`assist` with `several: true` mints a **child run**, `session::child_run_id(parent, issue, branch)`,
stable across retries, and takes the claim under that id. The marker in the worktree and the pointer
both name the child, and the child's pointer records `parent`. Everything that reads a pointer, such
as `standing`, `holders_of`, `prune` and `doctor`, reads an ordinary single-claim run. The session's
own run stays unsworn.

The answer names the child's id. `draft_pr`, `ready_pr`, `review_stages`, `review_thread` and
`estigia release --run-id` take it. Hooks cannot be handed an id, so they find the child by where the
call lands.

`several` is refused `already-holding` for a run that holds an issue itself, plain `assist` is refused
the same way for a run that holds children, and `several-claims-not-on-this-host` answers any runtime
but Claude Code. Every other host keeps one claim per run.

## How a call finds its claim

The hook reads the session's children and selects exactly one, by landing:

| Call | Placed by |
|---|---|
| A write tool naming an absolute path | That path |
| A shell call that is exactly one `git -C <plain path> <subcommand> …` | That path. The call is then decided as though it ran there |
| Any other shell call, or a write tool with no absolute path | The session's directory, unless the line moves directory |
| A shell call that moves directory any other way (`cd`, `pushd`, a second `-C`, `--git-dir`, `--work-tree`) | Nothing |

A call selecting zero children, or several, stays on the session's run, whose answer is the `Unclaimed
write policy`, exactly as if no child existed. So does a read: selection only runs for a call the gate
would judge. `git -C` is the row a plainly launched subagent needs, because its payload's `cwd` is the
primary checkout whatever its commands `cd` into.

"Exactly one" is read strictly. Outside quotes, `; & | < > ( ) $` backticks, a backslash, `#` and a line
break all disqualify the line; inside double quotes, `$`, a backtick and a backslash do. A word starting
with `-` straight after the path (`-c`, a second `-C`) disqualifies it too. So `git -C A … ; git -C B …`
is decided under neither.

`SessionEnd` forgets the children with the session's own pointer, and `SessionStart` lists each one.

## Merging the base

`git merge` is in `DELIVERS`, and an assisted run delivers nothing. Refusing it outright left a worker
no way to resolve a conflicting pull request, so one shape is admitted, for every assisted run and not
only a child: an exact `git merge` (the same literal the autonomous road calls an exact integration)
running in the run's own worktree, whose one target is the base `assist` recorded or `origin/<base>`,
or which only finishes a merge with `--continue` or `--abort`. It moves the claimed branch and nothing
else, and publishing it is still `draft_pr`.

Everything else stays `assisted-does-not-deliver`: another branch, no target, `--ff-only`, a pointer
written before the base was recorded. `git pull` was never in `DELIVERS` and is still admitted inside
the worktree as a boundary, whatever it names; [`honesty.md`](honesty.md) says so.

## For the orchestrator

- **Publish by the child's id, and only through `draft_pr`.** A shell push is refused, as for any
  assisted run.
- **Two repositories need two sessions.** `assist` takes no repository; the server's context is the
  directory it was launched in.
- **Subagents share one MCP server** (measured, in [`honesty.md`](honesty.md)), so a long
  `review_stages` `wait_seconds` from one worker delays its siblings' tool calls. Prefer short waits.
- **The orchestrator's turn stays open.** The rule in `SKILL.md` that no turn ends waiting on a
  background delegation still holds: wait for the workers, or do without their answers.
- **A pointer with no session row** is listed by `estigia sessions` as `unregistered`, the class every
  pointer-only run already has. A child is one.
- **A repeated `assist` leaves the state alone.** A resume no longer moves a Linear issue back to the
  started state, so a retry cannot undo a move to review.
- **Mirror branches stay unsupported.** `branch_pinned_elsewhere` still stops `assist` while the primary
  checkout has the branch.
