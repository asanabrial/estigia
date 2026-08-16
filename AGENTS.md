# Working in this repository

Estigia is a harness: it does not ask an agent to follow a workflow, it holds the tools. Everything
here is written so that the next person can tell what a rule prevents, not only what it says.

`CLAUDE.md` is this file. Keep them one file, not two copies.

## The documentation is part of the work

**Every change that alters behaviour updates the documentation in the same change.** Not afterwards,
and not "when it settles" — a document that describes the previous behaviour is worse than no
document, because it will be believed. This is the one rule in this file that has teeth, and the
crate already enforces neighbouring versions of it: `tests/honesty.rs` fails when a count claimed in
`README.md` or `docs/honesty.md` drifts from the code, and the population registry in `tests/guards.rs`
reopens a claim whose rule has moved.

Three properties, and they are checkable by reading:

- **Current.** If the behaviour changed, the sentence describing it changed. If a gap was closed,
  the `docs/honesty.md` entry naming that gap is gone or narrowed. If a gap was found and not closed,
  it is *in* `docs/honesty.md` with the measurement that found it.
- **Structured.** One document answers one question. Do not add a paragraph to the README because
  it is the file you had open; put it where somebody would look for it. The map is in
  [`openspec/README.md`](openspec/README.md).
- **Related.** Link the neighbouring document rather than restating it. A fact written twice is a
  fact that will disagree with itself, and this crate has spent whole rounds finding exactly that —
  a matcher held in two places, a refusal reviewed as a copy, a rule ported to a second language
  where no crossing could see it.

Everything here is written in **English**, including comments, commit prose and specifications. The
audience is whoever reads this repository next, and they may not share your language.

## Where things are

| Path | What it is |
|---|---|
| `src/harness/` | The gate: what a run may write, and what it may never write without asking again |
| `src/transport/` | The tracker transport, ported from the retired Python. Nothing crosses it against a second implementation any more — the corpus that did is deleted, and `docs/honesty.md` says what that cost |
| `src/setup/` | Registering Estigia in somebody else's agent, and taking it back out |
| `skill/` | The contract an agent reads, embedded and installed unchanged |
| `openspec/` | What a change is for, agreed before it is written — see [`openspec/README.md`](openspec/README.md) |
| `README.md` | What this does |
| `docs/` | The reference documents: what is not measured, the settings table, what setup writes, the binary lifecycle |

**There is no working log, and where the work stands is on the tracker.** There was one until
2026-08-14 — a scratch document in another language, carrying absolute paths from one profile, none
of which survives being cloned. Everything it held was filed as issues or kept as the operator's own
notes, and then it was removed rather than gitignored, because a file that only one machine has is a
place for a rule to hide.

Ask the tracker what is open, not a file in the checkout. That is the same answer this crate gives
about claims — the timeline is the only source of truth, and a local record of it is a second copy
that will disagree.

## How a change is made here

1. **Reproduce it first.** A fix starts with a test that goes red against the code as it stands, for
   the reason the defect exists. If turning the fix off leaves the suite green, the fix is not
   tested — that is a finding, not a formality.
2. **Say what it prevents.** A comment that repeats the code earns nothing. A comment naming the
   failure the line exists to stop is what makes the line safe to change later.
3. **Prefer removing a copy to adding a check.** When two places hold one rule, the fix is usually
   one place, not two guards.
4. **Run everything**: `cargo test`, `cargo clippy --all-targets`, `cargo fmt`, `cargo doc
   --no-deps`. One test needs `node` on the path — `the_plugin_hands_the_gate_the_directory_the_call
   _runs_in`, which *executes* the OpenCode plugin this crate generates, because that plugin is
   JavaScript and every other test of it reads its source text. It does not skip when `node` is
   missing; it fails and says so, since a skip spelled as a pass is a defect this repository has
   filed against itself. Nothing else here asks for an interpreter, and neither does `doctor`.
   `cargo doc` is on this list because it was not, and
   had been failing outright — four public pages linking to private items, and neither the list here
   nor CI ever asked. A crate that keeps its reasoning in doc comments and cannot build them is one
   nobody reads.

   **Anything but a bare `cargo test` needs `cargo build --examples` first.** A bare `cargo test`
   builds examples, so the list above is complete as it stands. Narrowing the run does not:
   `cargo test --test pipe` and `cargo test --lib <name>` select a target and build nothing else.
   Neither does **widening** it, which is the surprising half — `cargo test --all-targets` builds the
   example as a hashed *test* target and never writes the plain `fake_process` binary, so it answers
   `90 passed; 16 failed` on every run until the examples are built separately. That is the same flag
   this list prescribes for clippy one line above.

   Sixteen tests in `tests/pipe.rs` drive the binary against a stand-in `gh` that is an example. They
   used to report **pass** having executed nothing; they now fail loudly, naming the command that
   clears it. This matters because a filtered run is how mutation is measured here, which is what
   `docs/honesty.md` is made of.

## What not to do

- Do not widen a gate to make a test pass. A gate that decides nothing is the failure this crate
  exists to refuse, and it always looks like working correctly.
- Do not report a state you did not read back. When a write's outcome cannot be confirmed, say so
  and stop; never report the nearest named state instead.
- Do not name a command in a message unless running it clears the block. Naming a dead end is worse
  than naming nothing.
