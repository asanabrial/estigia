# The gate's reach across the agents

One area of [`honesty.md`](honesty.md), the record of what this instrument does not measure: what the gate reaches in each agent it is installed into, and what it does not. The index says which area holds which question.

## The gate's reach across the agents

- **`onFailure: "block"` is honoured from Claude Code v2.1.295, and no host was run to see it.** The
  `PreToolUse` handler carries the field because Claude Code's hooks reference says a timed-out
  `PreToolUse` hook does not block the tool call otherwise, and the gate's own worst case is the
  fifteen-minute transport wait `The gate's boundary` records. Read, not run: the reference states
  v2.1.295 (read 2026-11-16); the field is absent from the published settings schema at
  `json.schemastore.org/claude-code-settings.json` read the same day, whose `hookCommand` declares
  `additionalProperties: false`, and whose own documentation says the schema lags; and a newer Claude
  Code was observed elsewhere to strip keys it does not know from a hook handler when it rewrites
  `settings.json`. So an older host may ignore the field, warn about it or drop it, and a newer one may
  drop it on its own write; `doctor` reports a registered gate whose handler lacks it, which is the
  only visible sign a host took it away. Codex, Gemini, Qwen, Continue and Cursor register no such
  field — their references were not checked for one, and whether a timed-out hook blocks on those
  hosts is unmeasured.

- **`Decision mode: auto` is enforced on Claude Code alone.** The gate refuses `AskUserQuestion`, the one
  question tool its matcher is woken for, and records the recommended answer from the payload. Codex,
  OpenCode and Pi register no question tool with the gate, so on them `auto` is the contract's sentence
  and nothing more, and on every agent a question asked in prose rather than through the tool is not seen.
  "Recommended" is the option the agent itself labelled `(Recommended)`, or its first option: the gate reads
  which answer the agent ranked first and does not judge it. Measured by
  `src/harness/decision_mode/tests.rs`; turning the refusal off reddens two of its six tests.

- **The harness gates named populations, not every write.** The editing tools each agent ships, the git
  spellings that change the tree, the irreversible steps, and the shell constructs that visibly write a
  file: a redirect, a `sed -i` (a `curl -o` only when some output is not `/dev/null`, `/dev/stdout` or
  `/dev/stderr`), an interpreter handed code, the utilities that copy, move or delete (also
  under fifteen prefixes that run them: `sudo`, `flock`, `timeout`, …), and any command with a **long write
  flag** (`--write`, `--in-place`, `--fix`). The command a segment runs is resolved first
  (`shell::candidate_commands`: `if`, `then`, `else`, `elif`, `do`, `while`, `until`, `!`, `{` and
  `NAME=value` are skipped; behind a `RUNS_ANOTHER_COMMAND` wrapper every later word is a head of its own).
  **Not matched:** a write that shows nothing (an `eval`, a wrapper *script*, a merge reached by an alias or
  `gh api`), a formatter whose default is to rewrite (`cargo fmt`, `black .`, `gofmt -w`; reading short flags
  everywhere would report a search as a write), the keywords `case` and `select`, and a quoted command word
  except as below.

  **`git pull` is an `IRREVERSIBLE` boundary and deliberately not a `DELIVERS` one.** It runs a merge
  internally (a run once reached the identical merge with `git pull origin main --no-rebase --no-edit` after
  the merge spelling was refused), but pulling is an integration step and `stale_verdict` would otherwise
  refuse the contract's own `integrate before publishing` default. Classifying by *direction* was rejected
  (`origin/main` into a feature branch is an integration, `origin/<feature>` into `main` a delivery, and the
  arguments do not say which). `git pull --rebase` is intercepted too.

  **Provably data spans are blanked; the whole-text scan is the floor.** The fragment and disarm lists, the
  unclaimed-commit matcher and `shell::writes_a_file` read a line whose data spans are blanked, so a command
  name written as data (the `split` inside a single-quoted `gh issue view --jq '…'` program, `estigia
  stand-down` quoted in a `cat > file <<EOF` body) is not adjudicated as a command. Eligibility is a
  **grammar checked byte by byte** in `data_view`, an allowlist (the denylist it replaced lost seven review
  rounds, each to one more spelling bash reads differently): a bare first word from a bounded list of
  read-only commands, a restricted byte set outside quotes, quotes opened at a word start and plain
  here-documents. A **compound line** is read one top-level simple command at a time by the same grammar,
  which there also accepts a plain `$name`: a single-quoted `--jq` program inside a `for` loop is data. A
  pipe is judged at the boundary it crosses — both commands it joins must be readers — so `cd x && grep
  "git push origin" y | head` blanks while `cat x | sh` does not. For a lone simple command, `2>&1` is a
  redirect to a descriptor rather than a background operator, so it blanks like `2>/dev/null`; a compound
  line holding any redirect is still left whole. The
  whole line keeps the whole-text scan when data could reach execution through it — a pipe into anything
  outside the list, a redirect, a here-document, `<(`, `${`, `$'`, a comment, a backslash, or a name
  (`PATH`, `alias`, `function`, `hash`, `case`) that could change what a command resolves to or hide a
  boundary — and nothing inside `$(…)` or backticks is blanked. `gh api` has no verb, so a quoted endpoint
  after it is data. A reader whose own options decide whether it runs something is read by those options:
  `rg` without `--pre`, `git log`/`grep`/`show` (whose global `-c`, `-C` and the rest take their value as
  one word, and whose `git grep -O`/`--open…` runs its value through the shell or whose `git log --output`
  writes it), and `sed -n` when its script is on the command line, carries no option this reader does not
  know, and holds no `e`, `w` or `W` — those run a command or write a file in GNU sed. An option is read
  with its quotes removed, so `git grep "-O…"` and `sed -n p "-e" …` are the options they spell. `sort`, `printf` and
  `test` are absent because they run what they
  are handed (`sort --compress-program`, array subscripts), and `gh repo`/`gist` because `clone`, `fork`
  and `create` forward words after `--` to `git`. **Not closed:** an unquoted argument, so `echo git push`
  still reads as a boundary; a here-document beside a redirect target the reader cannot place (`> $file`,
  `> "out file"`) is left whole; a `~` or `$HOME` redirect target is placed at the home the hook was
  launched with when it is unquoted, so a line that reassigns `HOME` before the redirect is not modelled
  and a quoted `~` stays a directory named `~`; what a user's shell profile or an alias changes about the
  allowlisted names is unproved. **A differential test holds the property against bash itself**:
  `nothing_bash_executes_is_blanked_as_data` runs the measured inputs and 4000 generated cases under `bash
  --noprofile --norc -c` and fails when a marker command bash executed (`touch ran_N_`) is not still in
  `data_view`'s line. It cannot pass vacuously (a timeout fails, every measured input must run its marker on a
  bash new enough, a floor of 4500 executed markers with 4790 measured), requires `bash` and fails without
  it; the versions needed are `printf -v` subscripts 4.0, `test -v` 4.2 and `${ cmd; }` 5.3, the local bash
  3.2 rejects all three so unit tests carry those, and the version numbers are unmeasured here. **A quoted
  command word is read as the word it spells** (`shell::unquote_plain_words`: `pu'sh'`, `"gi"t`, `$'push'`);
  not read are a backslash-escaped quote, a word spelled through a variable, `$(…)`, an alias or script, and
  non-plain content (`git "pu\sh"`).

  **What exact-literal matching leaves** (traced by reading on 2026-09-07, re-read on 2026-11-16, not run):
  *not matched at all* are a backslash line continuation, an `.exe` suffix on a spelling the substring
  lists would otherwise catch (`git.exe commit`), `git update-ref refs/tags/…`, `gh release upload`/`delete`/`delete-asset`, `git send-pack`, `gh
  repo sync` and `git filter-repo`. A subcommand that writes only with a deciding
  option (`checkout -b`/`--`/`-f`, `switch -c`, `branch -d`, and every `reset`) is read per invocation
  by its program word however it is spelled — `/usr/bin/git reset --hard`, `\git checkout -b x` and
  `git.exe reset --hard` count — so an option between the subcommand and the deciding one no longer
  hides it (`a_git_write_is_a_write_whatever_order_its_options_come_in`), and plain `git branch <name>`
  is a protected write through `is_branch_creation`; a
  quoted argument of a command that writes is read as code, because `data_view` blanks quoted spans only for
  commands that read and that same breadth is what refuses a stand-down named in a commit message, so `git
  commit -m "git merge origin"` is a merge while a `;`, `&`, `|`, `)` or a backtick inside the quotes does
  not end it — `git commit -m "git merge;"` is not. The quote scan reads `'…'` with a literal
  backslash, `"…"` with backslash escapes and the ANSI-C `$'…'`, where a backslash escapes the next
  byte (`a_quote_scan_reads_an_ansi_c_span_and_an_escaped_double_quote_as_bash_does`), and no other
  quoting style; *matched but never read as a delivery* are `gh api`
  against the merge, release and tag-ref endpoints and the pull spelling. The `gh` delivery match skips
  an option and its value between `gh`, its verb and the delivery word, so `gh pr -R owner/repo merge 12`
  reads as the same `gh pr merge` as `gh pr merge 12 -R owner/repo` (and the same unidentified PR, since
  the line names a repository); whether `gh` accepts that order was not run.

  **The gate also measures its own disarmament.** `estigia stand-down`, `guard --uninstall`, `uninstall` and
  writes to the files those decisions are read from (its state, the contract, each agent's settings entry)
  are on the disarm list with the sibling subcommands (`estigia config edit`/`forget` beside `set`); a
  crossing over the parser's own tree holds every leaf below a listed verb to that list or to `DISARM_READS`
  (`config list`, `config repos`), so a third writing subcommand fails a test, while a new **top-level** verb
  is a review decision. `without_leading_options` moves a global option out from between the binary and the
  verb (`estigia --json config edit` is refused), reads a quoted value as one word so `git -c
  user.name="Bot Name" commit -m x` still names the commit, and the binary is recognised by basename.
  The relay's two commands are not on that list. `estigia review prompt` and `estigia review admit`
  carry a reviewer's prompt and answer, not a way to stand the gate down; a run that holds a complete
  published receipt is refused them anyway (`review-relay-is-the-reviewers`), because the secret a
  reviewer's answer must name is written into the prompt they materialise. The line is read after the
  normalisation the other shell arms use — a path-qualified or wrapped binary is already the word
  `estigia`, and an option before the verb is moved out — and so, like the disarm list, it refuses a
  line that merely mentions the two words. The reviewer's own context holds no receipt and is not
  refused, which is what keeps the relay working
  ([what each road establishes](honesty-review-panels.md)).

  **Declaring a stand-down is a person's act, and that covers the record as well as the command.**
  `harness::person_only` is decided first by `gate`, `gate_observing`, the push guard and the role road, so
  no stand-down, claim or unreadable definition lifts it. The gate refuses an agent's shell line running
  `estigia stand-down` (`stand-down-is-for-a-person`, any host, sub-agent or orchestrator), a file-tool,
  patch-body or shell write naming the stand-down record under `~/.estigia` (also via the control directory
  and the bare name), deleting it, and a line carrying `--lift` (it cannot be told from a declaration that
  only quotes the flag). A word after `estigia` that carries a shell expansion or a glob (`$`, a
  backtick, `{`, `*`, `?`, `[`) is refused as either command, because the gate cannot read which one
  runs and never evaluates the expansion; a quoted span is one word of data and not an invocation.
  The person lifts with `estigia stand-down --lift --reason <why>` in their own
  shell, or `!` in Claude Code. **Open by declaration:** a spelling that never writes `estigia stand-down`
  as two words (an alias, wrapper script, `cargo run -- stand-down`, a renamed binary), a write the shell
  reader does not recognise (an interpreter's `open(...)`), a glob or directory target that never spells the
  file name, a directory change in a separate call or through a variable or `popd`, a name the shell expands
  (`stand-$'down'.json`, `${x:-stand-down.json}`) and a name split across a line continuation. The match is
  broad on purpose: a commit message, `rg`, `git grep` or `git log --grep` naming `estigia` then `stand-down`
  and a patch body naming the record are refused with no lift; `cat` and the `Read` tool stay allowed.

  Twenty `Population` consts in the source (`src/harness/population.rs` holds the type) name each legitimate population and what falls outside it, and that number is checked here.
- **An issue is refused from a shell line, and the template is checked in one place.** `create` refuses a
  body whose first `##` heading outside a code fence is not `## Summary` followed by a
  `> [!NOTE]` alert, or that lacks a required heading or has them out of order, before it calls `gh`. The
  required headings are the `##` lines of the fenced block in `skill/assets/analyst-issue-template.md`
  (extracted by `build.rs`, so the file is the only list); the `###` sub-sections are not enforced (a
  test only checks the template asks for closed decisions, the mismatch action and self-sufficiency),
  and the check is structural, not about the prose beneath. `gh issue create`/`new`, `gh api` posting to an
  `/issues` path and a GraphQL `createIssue` are refused `issue-creation-outside-flow` whoever holds the
  checkout, naming `create` and its `repo` argument. Matching is on substrings of the command line, so a
  line that merely mentions the command is refused, `gh` after `sudo` is matched, and an alias or script
  that never spells `gh` still files an issue nothing checks. The read exemption knows only `-X GET` and
  `--method GET` anywhere on the line, so a post beside a `-X GET` read (`gh api -X GET a; gh api
  repos/o/r/issues -f t=x`) is unchecked. An issue filed in the web interface or by a token outside this
  harness is unchecked too.
- **A delivery by a run holding a claim is refused in a state that holds no verdict.** `git merge`, `git tag`,
  `gh pr merge` and the two `gh release` steps need the issue in `review` or `done`; `git push` and `gh pr create` are exempt
  (publishing is how a run reaches review). Operator-declared boundaries are never read as deliveries,
  since Estigia cannot know whether a `make deploy` delivers or rehearses. Two `git merge` shapes are
  preparation, not delivery; the integration merge has its own entry in [`honesty-gate.md`](honesty-gate.md) (*The integration exemption proves a branch is checked out…*). The narrower: an exact literal
  `git merge --ff-only <target>` (also `--` before the target) may run from `in-progress` after the tracker
  agrees when local Git proves all of it: on a branch, upstream a canonical `refs/remotes/*` ref, clean
  worktree, target that upstream's exact name or a full commit object ID, and ancestry from `HEAD` through
  the target to the upstream. Every proof Git process strips inherited `GIT_*`, and the exception is denied
  when any was present at entry or when `BASH_ENV`, `ENV` or `BASH_FUNC_git%%` is (compared without case).
  It stays a `Boundary` (a live tracker read, a ledger entry), skips only `out-of-phase`, only when the
  tracker answered exactly `in-progress`, after the verdict-to-bytes check. `analysis`, `ready` and
  `blocked` keep the refusal. `exact_integration_merge` spawns no process, so it refuses wrappers,
  compounds, quoting, expansion, alternate git directories and extra flags on the text alone; a detached
  branch is refused by `is_integration_in_the_runs_own_checkout`'s `symbolic-ref`; a dirty tree is refused
  nowhere on that path, deliberately, since `--continue` runs against a mid-merge tree
  (`a_conflicting_integration_is_resolved_without_leaving_in_progress`). **The proof is local and can be
  stale** (it never fetches), **and the proof and the merge are not atomic**: another process can move a
  ref, change the worktree or replace what `PATH` resolves between any two reads, and Estigia takes no
  lock. The ledger's allow line names `git merge` and the live claim, not which exemption admitted it.
- **That narrowing is not configurable, and the asymmetry is the reason.** Every other axis here can be
  switched; this one only tightens, and a setting that could loosen a guard rail turns it into a
  preference, as with the operator's boundary list, which adds and never removes.
- **Nothing here has driven a real agent.** The hook protocol is exercised by feeding the binary the
  payloads an agent sends and reading what it prints. That proves Estigia answers correctly; it does not
  prove Claude Code asks.
- **The tool-call gate reaches every agent Estigia knows but two.** Five agents, in four shapes, each
  taken from that agent's published reference or its source: Claude Code and Codex (`PreToolUse` →
  `permissionDecision`), Cursor (`preToolUse` → `{"permission":"deny",…}`), OpenCode (a plugin that blocks
  by throwing) and Pi (an extension that blocks by returning `{ block: true, reason }`). The two it does not
  reach are ZCode and DeepSeek Harness, and the ZCode and DeepSeek Harness entries in [`honesty-gate.md`](honesty-gate.md) say why. The eighth entry is the
  agent-neutral skills root, which is not an agent and has no tool loop to gate. The gate also still
  answers for six retired hosts — Gemini CLI, Continue, Cline, Crush, Windsurf and Qwen — because a machine
  that installed one before it was retired still carries its hook and dialect; `setup` installs into none
  of them and `uninstall` is the only path that touches them.
- **Three gates fire on every tool, and only Estigia's own lists narrow them; one of the three is
  still installable.** Cursor registers no matcher, so its host wakes the hook for whatever the model
  called and the tool name arrives in the payload; the retired Windsurf does the same, and the
  retired **Cline** arrives a different way: its gate is a script Estigia wrote whole, and that script
  pipes every `PreToolUse` payload straight through — no matcher, and no named list of its own. For
  every other agent something is crossed against the classifier: a settings matcher for Claude Code
  and Codex, and for OpenCode and Pi the `GATED` array their plugin and extension carry. For these three there is nothing
  to cross: this build has not been shown which names those hosts send, so a tool of theirs that is
  missing from `WRITE_TOOLS` or `SHELL_TOOLS` reaches the gate, classifies as `Untouched`, and is
  answered "not something this run's oath covers" — a sentence that asserts coverage nobody checked.
  Closing it takes a payload captured from a real session of each, which is the same evidence the
  rest of this table is built on and the reason this line is here instead. Measured, so the size of
  it is not a guess: `git push --force` reaches the classifier as a boundary under `Bash` and as
  `Untouched` under three other plausible spellings of a shell tool, and an edit to `src/main.rs` the
  same way.

  The guard that enforces this entry walks every agent whose tool calls Estigia gates by any
  mechanism, retired ones included, because their hooks are still on machines that installed them; a
  plugin-written gate that carries no list of its own has to be named here.
- **`ensure-states` cannot tell "already there" from "gh could not run".** `gh label create --force` exits
  non-zero for both, and the command reads that as idempotence. Measured against a `gh` that refuses every
  call: each answers `ok: true` with all six labels listed as `ensured`, and none exists. Closing it means
  reading what `gh` printed, and this crate has never been shown those exact words (the reason `icacls` is
  named beside `neutralising-shell` and not added to it). A closed git operation window is the one case
  where the answer says so (`deferred: true`, `window: closed`, nothing created).
- **The counts in the sentence about gated agents are crossed against the adapter table and the dialect
  list.** The check accepts the phrase "every agent Estigia knows" and used to stop there, while the
  sentence went on to say "six of them, in three dialects" when ten were gated in five: one end written by
  hand and the other in code, with only *part* of the prose crossing over.
- **A sub-agent's declared tool list is enforced only for the tools the gate wakes for, and only where an
  agent name arrives.** The role gate runs in `PreToolUse` and in `estigia gate`, but the second door is
  reached in this tree only by OpenCode's plugin, which sends no agent name, and whose documented limit is
  that `tool.execute.before` never sees a subagent's call, so it closes nothing today. Where the question
  *is* asked it sees exactly what the matcher wakes the hook for
  (`Edit|Write|MultiEdit|NotebookEdit|Update|Bash|PowerShell|Agent|Task|AskUserQuestion` for Claude Code; `Agent` and
  `Task` take the reserved-reviewer prelaunch path first). Measured against a published `builder`
  sub-agent declaring `Read, Write, Edit, Glob, Grep, Bash`: three of the six are tools the gate can judge
  and `Read`, `Glob` and `Grep` are never seen. It can refuse a tool **in the matcher and not in the
  list** (`MultiEdit`, `NotebookEdit`, `Update` for that builder), and not `WebFetch` or `WebSearch`,
  which an operator narrowing a sub-agent usually means. Widening the matcher would close it and is the
  one thing the matcher exists to avoid: waking this process for every `Read` and `Grep` is a cost paid
  thousands of times to answer "not mine".
- **The verdict's binding is checked at delivery only for what this run published through Estigia's own
  tools, with an earlier exact-receipt check at CI release.** `publish_review` records a fresh epoch over
  PR/head/base/digest while the PR is confirmed draft. `release_ci` re-verifies the live `review` claim, the
  newest receipt on the receipt's pull request across runs, the verdict recomputed over the live ledger, a
  current draft PR and a coherent clean target, then marks ready and reads back every outcome. The gate asks
  for one approved computed verdict; it does not enforce the blind policies' lens count, concurrency,
  independence, blindness or same-finding identity, and cannot tell one context from two or five. GitHub has
  no atomic conditional-ready operation, so an out-of-band ready or push can bypass this cooperative order.

  The local delivery gate persists the complete epoch/PR/head/base/digest receipt and checks the published
  head at the boundary that spends it, against different things per boundary. A `git merge`, `git tag` or
  `gh release` from a run whose recorded review head is not the invoking checkout's own head is refused
  `verdict-bound-to-other-bytes` (the run's own merge commit is the #58 exception); the invoking checkout is
  the covered directory selected for this call, not inferred from the pointer, whose worktree can be absent
  after a handoff. A `gh pr merge` is refused against the PR head the tracker reports (one `gh pr view
  <number>` read per attempt), never the invoking checkout's `HEAD`, which under Claude Code is structurally
  the launch checkout. A linked sibling sharing the pointer checkout's Git common directory can reach that
  verification: the classifier retains one positive numeric PR only from one literal `gh pr merge <number>
  ...`, candidates are filtered by complete `receipt.pr == command PR`, exactly one holder must remain, and
  only then are the tracker's PR head and that holder's claim compared. Refusals name the cause:
  `delivery-pr-repository-option` (`--repo`, `-R`), `delivery-pr-unidentified` (quoting, escaping, expansion,
  a redirect), `delivery-pr-mismatch`, `complete-review-transaction-missing` (a pointer with only a legacy
  `reviewed_head`) and `complete-review-transaction-not-selected` (no unique sibling holder); ambiguous
  candidates and unreadable pointers select no holder, and a sessionless call in an unrelated clone remains
  outside.

  Publish and republish first invalidate prior complete and legacy local authority and retain output only
  when all five receipt fields are complete (also when a refusal says a write committed or cannot exclude
  one); verdict and CI-release effects restore a supplied receipt into a reviewer's pointer after a handoff
  removed the publisher's. `gh pr merge` is also refused `merge-without-ci-release` unless the store holds a
  trusted `ci-release` marker for exactly the pointer's receipt (written by this identity, never edited, so
  an imported comment carrying one counts the same; an unreadable store refuses), and refused when the
  verdict that release was taken over no longer clears: the gate recomputes the receipt's basis as
  `release_ci` does, so a finding recorded after the release refuses the merge instead of being merged
  anyway. The line's own words are read too — `--squash`, `--rebase`, no `--merge`, `--admin` and any flag
  the gate does not place refuse, as does a missing or mismatched `--match-head-commit`. A `claim` restores a
  receipt when its answer carries the issue's newest confirmed, approved, unescalated publication whose pull
  request is open at the receipt's head with that base or one advanced cleanly past it, at the price of one
  `gh pr view` read (a failed read restores nothing); it does not read CI state, and a push between the
  claim's read and the merge is caught by `--match-head-commit`, which the gate now requires on the line it
  read. `git push` and `gh pr create` are not
  gated on it (the answer to a moved head is *re-publish*). Not seen: a run that bypasses the tools and
  changes GitHub directly. For `git merge`, `git tag` and `gh release` both halves of the comparison run
  through the same unsteered git invocation, so an exported `GIT_DIR` cannot make an unrelated clone look
  like the reviewed checkout.
- **No live Pi turn has run any Estigia extension; each is measured under `node` against a stand-in binary
  and a fake `pi`, and the rest is read from the installed Pi 1.1.0 package.** The stand-ins assert the
  arguments handed to `estigia gate`, `goal`, `review prompt|model|admit`, `session start|end` and `doctor`,
  the working directory, the three outcomes and the `{ block: true, reason }`, the relay's refusal codes and
  failed-completion paths, the loop's continuation, waits and stops, the status text, the refusal card and
  the subagent runner's RPC handling (the stand-in `pi` speaks only the records the runner reads and loads
  the real generated gate). `loadExtensions` was run once by hand against the generated gate and relay files
  (not in the suite). **Not observed:** that Pi **acts** on a returned `block` and awaits an async handler; a
  model calling the review tool or a real `streamSimple` returning an `AssistantMessage` (shape read from the
  `.d.ts`); a provider honouring the `AbortSignal`; a model turn started by the loop's continuation, and
  `agent_settled` in print and RPC modes (which have no UI); the footer line, the card on screen,
  `appendEntry` inside a streaming `tool_call` handler, the completion list, prompt-template expansion,
  `ctx.ui.custom` with `overlay: true`, `registerShortcut("ctrl+alt+a")` being free; that Pi accepts a section
  added in place by a `before_agent_start` handler, awaits an async `session_shutdown` handler and reports
  `fork` for both `/fork` and `/clone`; RPC `steer` accepted by a real child; the subagent runner's stall
  watchdog (none: a child that never settles runs until stopped; only the thirty-second first-answer deadline
  is enforced) and Windows (it kills the child, not its group). The built-in MCP client the `mcp.json` entry
  depends on exists from Pi 0.99.0 (an older Pi ignores the file while `status` reports the tools
  registered), and `exposure: "direct"` is read from `docs/mcp.md`. Pi 1.1.0 has no sub-agent concept, so
  "child session" is a heuristic (a `parentSession` in the session header). The tool is `estigia_subagent`,
  not `subagent`, so the issue's wording is not met literally.
- **The Pi loop forgets its goal when Pi restarts, and its `/goal` judge was read from Pi's declarations, not
  run.** The goal is held in the extension's memory (the run's claim and `stop-after` live in the store), a
  timer firing after the session was replaced stops the loop without notifying, and the objective is sent as
  a custom message Pi does not expand like `/skill:flow`. The judge asks `ctx.modelRegistry.streamSimple`
  and no test reaches a provider: not measured are request-time authentication, whether a model given 200
  output tokens answers on its first line (a reasoning model that spends them on thinking returns no text,
  which stops the goal as unreadable, never as met), and what the judge sees (a plain-text transcript cut at
  4,000 characters a message, 2,000 per tool output, 20,000 in all, keeping the newest; every third turn and
  the cap turn). Another extension registering `goal` or `loop` is not detected.
- **`deferred` exposure was measured with one model and one run per prompt, and setup reads the Pi version
  from `pi --version`.** On Pi 1.2.0 with `opencode-go/deepseek-v4.1-flash` a first request cost 3,255 input
  tokens with `deferred` against 13,868 with `direct` and 2,986 with the server off, and the model reached
  `mcp__estigia__*` tools through `tool_search` for an explicit prompt (8,720 tokens in all) and an implicit
  one (67,063): single runs, 2 to 15 turns, no other model, and a real flow under `deferred` is the open
  number. The version gate compares the first `x.y.z` in `pi --version` with 1.2.0; a `pi` off the `PATH` of
  the process running `setup`, or printing no version, gets `direct`, and a Pi upgraded afterwards keeps
  `direct` until the next setup (the `pi-config` warning names it). Pi 1.2.0 was not installed on the build
  machine.
- **Pi parent and child messages were run live twice, by hand** (2026-10-15, Pi 1.1.0 as both): a foreground
  `estigia_subagent` showed the parent cannot reply while it waits, and a `background: true` run woke the
  parent on the child's query, returned the reply as the child's tool result and delivered the child's final
  answer as a follow-up within 15 seconds. Not run live: the immediate refusal of a foreground query,
  steering through `estigia_subagent_message`, a query that times out after the background fix, a child under
  an agent definition restricting `--tools`, `deliverAs: "followUp"` waking an idle session, and ordering
  between two children. A notification sent while the parent is gone is lost (at-most-once), and the
  120-second query timeout is a constant. **A reply can still be reported as answered after its child gave
  up**: a reply that crosses the child's withdraw frame in flight, or a withdraw that never arrives (the
  parent's own expiry `WITHDRAW_GRACE_MS`, five seconds after the timeout, is all that closes it), makes
  `estigia_subagent_reply` answer `answered` though the child never receives the text; no test covers either
  window, a failed withdraw send, or a withdraw for an unknown request.
- **A Pi child is linked by its session header, with the pid as the fallback and an environment variable
  beneath that; the live check ran without a claim and under `-p` only.** The header link is held by
  `the_pi_extension_links_a_child_by_its_parent_header_whatever_its_pid`. The parent's id is the exported
  root, else the `id` in its session file, else the part of its file name after the first `_`; none is
  authenticated and `--parent-session-id` is an assertion the binary cannot check, like `--run-id`. Live
  (2026-10-14, isolated Pi home): a child `pi` started by Pi's `subagent` example was another process and its
  `write` and `bash` reached the gate with `--parent-session-id` equal to the parent's session; the write was
  refused `issue-required` for want of a claim, so what a child is *allowed* under a held claim was not
  observed. A forked or new session also carries a `parentSession` but is read as a root by its start reason;
  a replacement inside a separate child process is still linked by the pid; an extension passing its own
  `env` without the two variables is unlinked; a parent that names no run gets no fallback to the child's own
  (the child receives the parent's answer, `nothing-sworn` included). The `pi-subagents` row of `doctor`
  reads names and source text, not behaviour: a runner is recognised when `subagent` is in its package source
  or extension name and called gating only when every `.ts`/`.js`/`.mjs`/`.cjs` file within three levels was
  read inside a 1 MiB budget and a `spawn(`, `execFile(` or `spawnSync(` visibly takes `pi` as a
  string-literal first argument; anything else is answered unknown, and an npm or git package is unread.
- **Pi's reviewer model is only as precise as the `reviewer` routing row, and Pi's agent definitions follow an
  example extension's convention.** A `provider/id` Pi lists runs on that model; a bare alias such as `opus`
  is a valid CLI model and unresolvable for Pi (`reviewer-unavailable`); on a machine with the `claude` CLI
  Pi's relay is never reached; no reasoning effort is set; and neither the relay nor `admit` bounds the
  answer's size as a CLI reviewer's `MAX_OUTPUT` does. The agent definitions (`name`, `description`,
  `tools`, `model` frontmatter, `~/.pi/agent/agents/`, the `--tools`/`--model` pass-through) come from the
  shipped `examples/extensions/subagent` in Pi 1.1.0 and no child `pi` was launched against one. `tools`
  restricts only built-in tools (MCP tools stay unless an entry starts with `mcp__`), so what stops the
  analyst writing is the gate measuring the child against the parent's claim, and
  `harness::roles::definition_for` does not search this root.
- **Claude Code's `PowerShell` tool is read as POSIX shell, and the forms that grammar cannot carry are
  unread.** The matcher names it and `SHELL_TOOLS` holds `powershell`, so the reader Bash gets is the one
  it gets: `git commit`, `git push`, `gh pr merge` and a redirect that writes a file are classified the
  same through either tool. What the shared reader does not do for PowerShell: resolve its aliases (`sc`,
  `ni`, `iex`), read a command assembled at run time (`Invoke-Expression`, a variable holding the line),
  decode `-EncodedCommand`, or follow `cmd /c` and `wsl` indirection into another shell. A line written
  that way reaches the gate and is read as the literal it is, which is not the command it runs; this list
  is the statement of that gap, not a claim that it is closed.
- **Pi's `powershell` is read as POSIX shell, other extensions' tools are gated only by input shape, and
  Pi's own settings are not covered.** The extension sends any tool whose input has a string `command` to the
  gate as `bash` and a string `path` with `content` or an `edits` array as `write`/`edit`; a PowerShell
  cmdlet that writes (`Set-Content`, `Out-File`, `Remove-Item`) goes through, a tool naming its command or
  path differently (`cmd`, `file_path`, `script`) is not recognised, and a later `tool_call` handler can
  mutate `event.input` after this one answered. **Pi's settings can unload the gate and the tool server**:
  the `extensions` array of `settings.json` takes an exact exclusion `-path`, a glob `!pattern` and
  `-builtin:mcp`, a project's settings apply over the user's, and a project `mcp.json` entry with no
  `command`, `url` or `type` overrides `enabled`, `exposure` and `toolExposure` of the user's server, so one
  write to `~/.pi/agent/settings.json`, `.pi/settings.json` or `.pi/mcp.json` unloads them without touching
  a file Estigia owns. All of those, and `.pi/extensions/`, are on the control surface (a write to a
  *different* repository's `.pi/settings.json` too, since the matcher cannot tell home from project). The
  `pi-config` row of `doctor` reads them for an exclusion of `estigia-workflow-authority.ts`,
  `estigia-review-relay.ts` or `-builtin:mcp`, and the project `mcp.json` for `enabled: false` or an
  `exposure` other than `direct`/`deferred`; it is broken, rewrites nothing and names no command. Not
  measured: whether a project `-path` excludes a *user-level loose* extension at load time, Pi's glob
  dialect (the row lets `*` cross `/` and knows `?`), other resource arrays and `packages`, that the row's
  checkout is the project Pi opens, and project trust.
- **Pi's agent directory follows `PI_CODING_AGENT_DIR`, but only as the process that runs `setup`, `status`,
  `doctor` or the gate sees it.** One resolver reads the variable (a leading `~` is the home; anything else
  not absolute is dropped with a note, falling back to `~/.pi/agent`) and every Pi path comes from it;
  unlike `DSH_HOME` the directory is followed wherever it points. The gate treats the named directory's
  `extensions/`, `mcp.json`, `settings*`, `AGENTS*` and `agents/` as control surface, but the static
  `harness::CONTROL_SURFACE` cannot spell a directory the environment chooses, so a Pi started with a
  different variable is not covered. An install made under `~/.pi/agent` and then run under a variable
  naming another is not reported (`doctor` says only that Pi is not configured), Pi's expansion beyond the
  leading `~` is not reproduced, and nothing was run against a live Pi under the variable. Pi documents
  that `AGENTS.override.md` replaces `AGENTS.md` in the same directory, so the `pi-config` row is broken
  when one is in the agent directory and does not look beside a project's own `AGENTS.md`.
- **A gate plugin newer than its binary blocks every gated call.** Both plugins send `--input -` and the
  input on standard input; an older binary reads `-` as the JSON, refuses with exit `1`, and the plugin
  blocks the call. `setup` writes the plugins together with the installed binary, so the skew arises only
  when a plugin outlives a downgrade; re-running `setup` clears it.
