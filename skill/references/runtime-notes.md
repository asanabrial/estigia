# Runtime notes

Runtime capabilities change how Issue Flow is invoked and how independent work is obtained. They do
not weaken workflow state, review independence, attribution, or durable tracker evidence.

## Invocation sigils

Claude Code invokes skills with `/`:

```text
/flow triage [domain-rules | instructions]
/flow dev [domain-rules] [issue-number]
/flow
```

Codex invokes skills with `$` and rejects `/`, which resolves built-in commands only:

```text
$flow triage [$<domain-rules> | instructions]
$flow dev
$flow
```

The skill and arguments are otherwise identical.

`triage` and `dev` are the roles' short names. `analyst` and `developer` are still accepted
everywhere the short names are, and mean the same thing — a contract that renamed the words an
operator has in their fingers would break every habit at once for two saved characters.

The bare form takes no role word and is the combined analyst-developer, routing to which issue; the
Decision Gates table in `SKILL.md` owns that order, and this file does not restate it.

`flow <issue-number>` is a form with no role word, and it exists only where the repository's
`Claim workflow` row is `assisted`: a refusal that names the `Claim workflow` row — a tracker write refused under `assisted`, or `assist`
refused outside it — is how a runtime learns which mode it is in. The Decision Gates row owns the
path (`assist`, work in its worktree, `draft_pr`, report the URL, stop); it needs no claim and no
heartbeat, so the team-mechanics, identity and renewal sections below do not apply to it.

## Independent review contexts

Adversarial review is an analyst run over a bounded change. It is repository-read-only, produces
findings rather than commits, and MUST NOT be performed by the context that wrote the change. On a
repository whose `Evidence standard` is `measuring` a Judgment Day judge also builds, tests and mutates —
inside the isolated directory its launch hands it and nowhere else. What leaves the repository
read-only to it is `policies/judgment-day.md` requiring that directory to be outside the claimed
checkout: a rule the launch obeys, not one the gate enforces.

The runtime decides how to obtain the second context: subagent, teammate, or separate session all
qualify. If the runtime or operator forbids delegation, use a separate session.

For Claude Code, every setup installs two definitions, `review-blind` and `review-blind-sealed`,
with `model: inherit`. The sealed lenses, the refuter and the validator are not installed:
`launch_review_input` launches each from a template embedded in the binary, so neither of these two
paragraphs launches them and there is no hand launch — the launch gate refuses a `Task` launch of one
of those seven names; `skill/SKILL.md` developer step 5 and
`blind-judges.md`'s "Lens acquisition", "Refutation" and "The targeted validator" sections say how
the binary acquires them. Judgment Day's two judges are `review-blind` and
`review-blind-sealed`: no tier acquires them, and they launch only when the operator
explicitly asks for Judgment Day — see [judgment day](../policies/judgment-day.md). Each is inert unless its launch
prompt names the immutable target — a head, a diff or a commit range — and the review criteria. The orchestrator passes `Model
routing`'s effective `judge` model on each launch. Both run concurrently, with the identical target and
criteria. The unsealed context's grant is `Evidence
standard`'s answer; the sealed one's is fixed to a web search. Neither delegates or inspects sibling
output. Config writes do not mutate either definition. A
definition is a host constraint and a contract, not evidence that a review was actually instantiated.

Setup also installs the delegated roles `developer` and `analyst` for Claude Code and OpenCode,
each in its host's own dialect, on a switch of their own:
naming the role in `Model routing`. That entry decides whether either
definition exists: a model named for a role the routing does not name reaches
nothing, so removing the entry retracts the definition and adding it installs one. `Model routing`
says both what a named role runs on and whether it exists. Check the entry before relying on a
definition being there. Its other value, `by task`, names no role and so installs neither
definition: the shape of the work decides the model and the effort, and the launcher records what
it chose.

`review-blind` is reserved to the operator-owned canonical user definition, and so is
`review-blind-sealed`. The preflight below runs for those two names alone, since they are the only
installed reviewer definitions; a lens, the refuter and the validator have none to preflight,
because the binary starts each from an embedded template. Claude's current `Agent`
and legacy `Task` launch surfaces are preflighted before repository-write classification: recursively
parse project `.claude/agents` YAML from the launch cwd through the first `.git` repository root for the
reserved filename or frontmatter name, fail closed on unreadable or duplicate candidates, and require
the normalized canonical user text to be the only user-scoped definition with that identity. Setup
performs the same recursive user-tree uniqueness preflight before writing. The top-level `agent_type` is the caller;
only nested `tool_input.subagent_type` is the launch target. An already-running reviewer always receives
the embedded policy, never whatever is on disk. This does not alter ordinary project-first agent resolution and does not
cover OpenCode launches. Absence of that reserved type is not an absent capability: a host without
`review-blind` still gets the panel, because the binary launches it, after `publish_review` and a publication lane that is green, never-ran, absent, disabled, or has no lane at all (not red, not unreadable), `review_status` offers each lens the tier licenses as a `collect` input whose `capture` is `launch_review_input` — one at `medium`, four at `high` and none at `low`, except a `warning`- or `suggestion`-cleared repair over its own honest narrow non-`high` range, which `review_tier` with `parent_head` answers with the delta lens alone.
The host calls `launch_review_input` for each offered input — `names` starts them all in one call — then `collect_review_input` for each when `review_status` names that as the input's `capture` (again, `names` collects them all in one call; an input with `capture: null` is running: ask `review_status` again), and launches no lens `Task` of its own: the binary starts the first installed `claude` or `codex` CLI with no tools, hands it the diff and the criteria, and records the findings and then the lens report itself. OpenCode can make that call, because `estigia setup opencode` registers the Estigia MCP server in `opencode.json`. On an OpenCode run with the review relay plugin installed, or on a machine with neither CLI, `launch_review_input` answers `relay: true` and starts nothing: the host launches the reviewer through its own sub-agent tool, and on an OpenCode run the answer carries `launch`, `{agent, prompt}`: launch exactly that built-in `agent` with exactly that `prompt`, whose `<!-- estigia:review name=<role> -->` marker is what the OpenCode review relay plugin (`estigia-review-relay.js`) intercepts to supply the binary's exact prompt and hand the answer back for admission, so the orchestrator never receives it.
Never name a reserved reviewer as the agent: `estigia setup opencode` installs none, and OpenCode refuses an unknown agent. That is a launch, not a handoff: `handoff_review` remains only for a host that can neither start a CLI nor relay. A CLI that is missing, out of quota or not logged in refuses the capture and records nothing, and the lens stays offered. A not-logged-in refusal names the `HOME` and config-dir variable the reviewer was launched with; compare that environment with the operator's before asking for a login, since the CLI may be logged in under a different one. Use a separate session or durable handoff only when that launch failed, never as the first move and never a smaller or serialized panel. A host that can call the tool and calls `handoff_review` without launching first is a defect against this sentence.

The OpenCode relay prompt separates required repository acknowledgments from its job-bound structured
result; follow the binary's channel contract rather than stripping prose or reconstructing a result.
After a malformed answer, `capture-job-missing` on collect means that attempt was abandoned, not that
the publication must change. Ask `review_status` and relaunch the offered input against the unchanged
receipt. The binary's supplied prompt is the output contract, not a host-authored replacement.

Acquiring a reviewer is not gated on a row: a run acquires one when its runtime can provide one, and
does so without asking. It does not make Estigia spawn a model for the work, open a session, or prove a
context is independent; the one launch Estigia makes itself is the reviewer `launch_review_input`
starts. Being able is not the only thing that decides it: a host whose always-loaded instructions
say to ask before launching a subagent does not make an authorised launch unauthorised, because
Estigia writes the standing grant into that same always-loaded file — a launch is made and its result
reported, not asked for again. When the host cannot spawn a helper at all, or every launch of the panel failed, use the
binding's durable review handoff so another run can discover the exact receipt. A handoff written
while `Review delegation` was still a row may carry `authority=ask` and a deadline; it still binds
its requester.

The orchestrator holds no agreement rule over the lenses when it records the computed verdict. A severe finding is
settled by the ledger at every tier, stated in full in `blind-judges.md`. Preserve dissent, warnings and
suggestions. Estigia cannot prove panel size, concurrency, independence, blindness, same-finding identity or
quorum, and its transport has no per-reviewer verdict mechanics.

## Analyst enforcement and delegation

Where supported, enforce analyst read-only behavior with a tool allowlist that excludes file-writing
tools. On Claude Code with `analyst` named in `Model routing`, that allowlist is already shipped
as the `analyst` definition, so a launch of that role carries it. Check the entry rather than assume
it: the routing is empty by default, and a model named for `analyst` in a cell that does not name the
role reaches nothing. Where
the definition is absent — every adapter but Claude Code and OpenCode, and either of those until its
row names the role — write the allowlist into the launch. Either way it reduces accidental edits and does not seal the
boundary: a shell can write files and is still needed for measurements, so the prose prohibition
remains authoritative.

Claude Code applies a subagent definition's tool allowlist when that definition runs as a teammate,
but does not apply the definition's `skills` field. Put required domain rule books in the teammate
prompt rather than assuming they were inherited. A delegating dev MUST load the routed domain itself
before choosing a delegation plan because that domain may prescribe orchestration constraints.

## Runtime identity derivation

Create one run-id and reuse it for the full run. The gate admits only the named shape
`session::run_id` mints: a runtime component — lowercase `a-z0-9` groups joined by single `-` — a
dash, then a dashless suffix of eight to sixteen ASCII alphanumerics ending in eight lowercase hex
digits. The `<runtime>-unknown` an identity-less session would mint is refused by every
run-id-bearing tool before it reaches the tracker, so mint an identity instead. A suffix that carries
a dash is refused unless the fragment after the last dash is itself that shape — the single residual — because the shape is split on the **last** dash: `codex-01k4-9f2a`
projects the well-formed `codex-01k4` and would mint `dev:codex-01k4`, a permanent repository-level
label the `gh issue list --label dev:codex` query never finds. Let the crate mint the id where a
hook reports the session; compose one yourself only from the runtime word and eight hex digits
(`claude-3edc7a52`).

Claude Code exposes the same session prefix in its team and task paths, including
`~/.claude/tasks/session-<8 chars>/`, which is what joins remote evidence to the local transcript.
When no usable session id exists, use eight random hex digits; untraceable per-run identity is still
safer than shared runtime identity.

On OpenCode the session id is the `ses_…` value the host gives the session — OpenCode 1 hands it to
the `tool.execute.before` hook, OpenCode 2 carries it in the `execute.before` event — and the run id
is `session::run_id(<runtime>, <session id>)`: the runtime word and a dash, then the first eight
alphanumeric characters of the session id, then eight hex digits of its FNV-1a hash. The rendered
plugin forwards that session id to the gate as `--session-id`, so the composition happens once, in
the crate. A run id composed any other way names no pointer to the gate's identity road: the call is
answered as a run that holds nothing, which permits, and nothing detects the mismatch — and one whose
shape the crate could not have minted is refused outright, before it reaches the tracker. A host that
hands the hook no session id, with no run id exported around the plugin, keeps the checkout road.
OpenCode also reads the optional `Loop plugin` row: with it `on`, `setup` writes a plugin that
continues the session while `estigia goal` reports the run live or another issue is claimable by it;
`off`, the default, writes nothing, and no other host reads a plugin Estigia writes; its commands are `/flow-loop` and `/flow-status`. The separate `Goal loop plugin` row writes a generic `/goal` and `/loop` that run no Estigia command. The `goal` verb
is also the run's own goal surface: `estigia goal progress <note>` and `estigia goal blocked <reason>`
record timeline activity that renews the claim, `estigia goal stop-after [--reason <text>]` asks the
loop to end once the current task's claim is released (a live claim keeps the loop going, and the
request is consumed when the loop stops on it), and `estigia goal complete` adjudicates, read-only,
whether delivery evidence — a closed issue, or the latest publication's computed verdict, `approved`
over the live ledger, over a lane the gate would clear — proves the goal complete.

On Pi the session id is the UUIDv7 `ctx.sessionManager.getSessionId()` answers, and the run id is
`session::run_id("pi", <session id>)`, composed as for OpenCode. The extension `setup pi` writes
forwards the session id with `--runtime pi`, so the composition happens once, in the crate; a call
without a session id keeps the checkout road. Pi has the gate, the MCP tools, the directive and a review relay, and no
loop yet. With no sub-agent tool to intercept, its relay is a tool: the host calls
`estigia_review_lens` with the reserved reviewer's name, and the extension runs the
reviewer in process and admits the answer, which the host never reads.

## In-session teams are ephemeral

Claude Code agent teams and Issue Flow coordinate different lifetimes:

| Property | Agent-team task list | Issue Flow |
|---|---|---|
| Lifetime | one session; teams cannot be shared across sessions | repository lifetime |
| Coordination | machine-local file lock | server-side claim plus readback |
| Survives run end | no | yes |
| Reachable by another runtime | no | yes |

Use teams to parallelise within one run and issues to preserve work across runs. Parallel analyst
investigation is a strong team use case, but task status, mailbox content, and teammate context vanish
with the session. The shared task list is a temporary view, never authoritative workflow state.

A teammate that finishes without recording its result on the issue has produced no durable output.
Where available, Claude Code's `TaskCompleted` and `TeammateIdle` hooks can enforce this by refusing
with exit code 2.

Do not assume another team mechanism repairs abandonment. Claude Code teams have documented cases
where teammates fail to mark tasks complete and block dependants, requiring manual status repair.
External-session recovery therefore remains the durable workflow's responsibility.

## Waiting on delegated work

Delegated work outlives the call that launched it. A run that has launched background work — a
subagent, or a command the host will report on — and has nothing to do until it reports **ends its
turn and lets the host resume it**. It does not hold the turn open with a `sleep` loop or a poll:
every later turn re-reads the context this one leaves behind, so a polled minute is paid for again
in tokens by every turn that follows, and the completion wakes the run the same way.

The `Stop` hook enforces the other half. A run holding a live claim is kept going while its
transcript shows no background launch still unanswered; a run whose transcript holds one is let go,
whether or not other work remains: the hook cannot see unfinished work that answers to nothing in
the transcript. A run waiting on a person is let go too: its pointer is `blocked`, or its newest
receipt on the held issue in the local store is `escalated`. With no transcript path, or one this
build cannot read, the claim keeps the turn, the conservative answer.

The hook reads the tracker at most once per renewal window; a failed or unknown read is never kept.

A `rate-limited` refusal means the GitHub API budget is spent. Write nothing and do not retry: a
retry spends the budget being waited for. The reset time is named only when `gh` printed one.

## Honest runtime limits

- A claim is not a lock. Near-simultaneous claims can race, and single-valued or shared-account claim
  fields can hide the loser. The selected binding defines authoritative verification.
- A horizon is a heuristic, not a lease. Timestamps cannot distinguish a dead run from a live run
  thinking silently. Early reclaim costs bounded duplicate work and leaves an audit record in the store; never
  reclaiming leaves work stuck forever.
- A transcript is not a completion record. A finished Claude Code subagent writes no terminal record
  into its own transcript, and the completion notice can claim background work that no process is
  running; neither is a measurement that the agent is done. A run waiting on delegated work waits
  for the harness notification, or reads `ListAgents` for a `completed` status, and never polls the
  transcript.
- Credentials must be reachable from the execution environment. Keyring tokens are often invisible
  inside sandboxes. Prove access with a cheap read before analysis that must be filed remotely.
- A tracker serializes claims, not CPU, RAM, accelerators, databases, ports, or other machine
  resources. Two valid holders can still destroy each other's heavy phases through contention.
- Issue Flow moves tasks; it does not judge them. Analyst domain rules decide value, and a standalone
  dev inherits that recorded judgement rather than silently reinterpreting it.

A claim with a horizon stays live until that horizon, or until four hours after the run's last `heartbeat`,
`progress`, `blocked`, `branch` or `published` marker written since the claim, whichever is later. A claim
with no horizon stays live four hours after the last such marker. Before a phase expected to run longer than four hours,
declare a horizon that covers it, or heartbeat at least every four hours from the orchestrator: a delegated
context renews nothing.

Before a machine-heavy phase, mention it in a heartbeat and inspect other in-progress heartbeats.
When contention kills a run, rerun the work alone before interpreting the failure. Resource coordination signals are advisory and never replace durable
issue state.
