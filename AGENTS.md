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

Four properties, and they are checkable by reading:

- **Current.** If the behaviour changed, the sentence describing it changed. If a gap was closed,
  the `docs/honesty.md` entry naming that gap is gone or narrowed. If a gap was found and not closed,
  it is *in* `docs/honesty.md` with the measurement that found it.
- **Structured.** One document answers one question. Do not add a paragraph to the README because
  it is the file you had open; put it where somebody would look for it. The map is in
  [`docs/README.md`](docs/README.md).
- **Related.** Link the neighbouring document rather than restating it. A fact written twice is a
  fact that will disagree with itself, and this crate has spent whole rounds finding exactly that —
  a matcher held in two places, a refusal reviewed as a copy, a rule ported to a second language
  where no crossing could see it.
- **Located.** A citation to a file this repository ships names the item inside it, never a line
  number: a symbol survives a merge and a coordinate does not. A citation into a project this
  repository does not contain is named as external and left to the reader who can open it.
  `tests/honesty.rs` fails on a `path:line` citation it resolves to a repository source file, on a carried line
  (`:NNNN`), on a `path:line` whose repository-prefixed path is not there — a bare path is left to
  prose, since in prose it may be illustrative, historical or external — and on a crate item one of
  the two forms it recognises binds to a file that does not hold it.

This file carries a byte ceiling of its own in `tests/instruction_ceiling.rs`, beside the shipped
skill's: growth past it fails the suite, and moving the number is the review-visible edit a budget
exists to force. Inventory belongs in a document that answers its own question — which is why the test
catalogue is in [`docs/testing.md`](docs/testing.md) rather than here.

Everything here is written in **English**, including comments, commit prose and specifications. The
audience is whoever reads this repository next, and they may not share your language.

## Where things are

| Path | What it is |
|---|---|
| `src/harness/` | The gate: what a run may write, and what it may never write without asking again |
| `src/transport/` | The tracker transport, ported from the retired Python. Nothing crosses it against a second implementation any more — the corpus that did is deleted, and `docs/honesty.md` says what that cost |
| `src/setup/` | Registering Estigia in somebody else's agent, and taking it back out |
| `skill/` | The contract an agent reads, embedded and installed unchanged |
| `README.md` | What this does |
| `docs/` | The reference documents, mapped in [`docs/README.md`](docs/README.md): what is not measured, the settings table, what setup writes, the binary lifecycle, what a blind review costs, what parity is read against |

**There is no working log, and where the work stands is on the tracker.** There was one until
2026-08-14 — a scratch document in another language, carrying absolute paths from one profile, none
of which survives being cloned. Everything it held was filed as issues or kept as the operator's own
notes, and then it was removed rather than gitignored, because a file that only one machine has is a
place for a rule to hide.

Ask the tracker what is open, not a file in the checkout. Ask the local store
(`~/.estigia/state.db`) who holds an issue and in what state: claims, states, receipts and tasks are
its rows and nothing else is authority, so a checkout file or a tracker comment that records them is
a second copy that will disagree.

## How a change is made here

1. **Reproduce it first.** A fix starts with a test that goes red against the code as it stands, for
   the reason the defect exists. If turning the fix off leaves the suite green, the fix is not
   tested — that is a finding, not a formality.
2. **Code carries no comments by default.** The names, the types and the shape of the code say what
   it does; `docs/` says why the subsystem is the way it is. A comment is an exception you justify,
   not a habit.

   **Scope: `src/` and `tests/` alike, test modules included.** This supersedes the narrower
   boundary set earlier, which swept `src/` only and left `tests/` out as a scoping decision
   rather than a judgement that the prose there differs.

   Write one only when all three hold, and then write one or two lines and stop:

   - it points at something **specific to this expression** — not to the subsystem, which is a
     document's work;
   - it says something a reader **cannot see** in the code and **will not find** in `docs/`;
   - without it, somebody changing this line would get it wrong.

   Everything else goes: incidents, measurements, arguments, history, restatements of the line below,
   and anything that would still read as true if the code changed underneath it. Those belong in
   `docs/`, or nowhere.

   `///` on a **public** item keeps a short statement of what the item is and what it refuses,
   because `cargo doc --no-deps` publishes it — and because `missing_docs` is `warn` in `Cargo.toml`
   while CI sets `RUSTFLAGS: -D warnings`, so removing it fails the build rather than merely reading
   worse. `///` on a private item is judged as any comment is, by the three conditions above.

   Nothing enforces this and nothing may: no test reads the text of a comment. It is
   held by review, not by the suite, and [`docs/honesty.md`](docs/honesty.md) says what that costs.

   **No comment is load-bearing, and one rule keeps it that way: a test that reads a Rust source
   reads it through [`source_text::code_of`](src/source_text.rs), which strips every comment first.**
   That is the whole discipline, and it is a shape you can check while writing rather than a run you
   hope catches you. It holds for `include_str!` and for `std::fs::read_to_string` alike — the second
   is the one a sweep forgets, and forgetting it sent the work back from review twice, the second
   time over two readers the first repair had not found. Strip **before** you slice, too: a boundary
   taken from raw text, `#[cfg(test)]` most of all, is one a comment can move.

   Three mechanisms used to break it, and each was replaced rather than merely deleted.
   `guard:population` declarations were paragraphs inside doc comments; they are `Population` consts
   now — the type in [`src/harness/population.rs`](src/harness/population.rs), each const beside the
   code it describes — and `tests/guards.rs` resolves the `subject` each one names and fails when it
   resolves to nothing. The refusal taxonomy was scraped out of `src/transport/` doc comments;
   `src/harness/tracker.rs` crosses the vocabulary `build.rs` extracts from the transport's own code.
   And one test's entire subject was a sentence written in nine doc comments; it is gone.

3. **Prefer removing a copy to adding a check.** When two places hold one rule, the fix is usually
   one place, not two guards.
4. **While you iterate, run the area you changed; run everything once, before publishing.** Build the
   fixtures once (`cargo build --bins --examples`), then run only the tests for the area being
   changed — a filtered run selects a target and leaves the example unbuilt, which *Anything but a
   bare `cargo test`* below explains.

   - `cargo test --lib <module path>` — e.g. `harness::guard`, `transport::claim::tests::capture`,
     `config::tests`, `tui::tests`.
   - `cargo test --test pipe <name>`, or `cargo test --test honesty` and
     `cargo test --test instruction_ceiling` when documentation or the shipped skill changes.
   - One test: `cargo test --lib <full::path> -- --exact`.

   The four commands in the next paragraph run once, before publishing — not on every edit.

   **Run everything**: `cargo test` (CI runs `cargo nextest run --profile ci` plus `cargo test
   --doc`, which kills a hung test after the `.config/nextest.toml` bound), `cargo clippy
   --all-targets`, `cargo fmt`, `cargo doc --no-deps`. One hundred and eleven tests **require** `node` on the path, four of them `tar` too,
   one requires `bash`, nine in `tests/usage_accounting.rs` require `python3` on Unix; all of
   those fail rather than skip without it, and `doctor` asks for none. Which test drives which
   generated integration, which driver carries it and what each one needs is
   [`docs/testing.md`](docs/testing.md) — inventory, kept there so that this file carries what
   governs a decision.

   **The toolchain is pinned, not floated.** `rust-toolchain.toml` names the channel and the
   `rustfmt`/`clippy` components, and `ci.yml` runs no toolchain action of its own, so rustup
   resolves that one file for CI and for a fresh clone alike: `cargo clippy --version` here is the
   version the lane runs. `@stable` used to float, and a Rust release could redden the lane with
   `-D warnings` on a line no diff touched.

   **Anything but a bare `cargo test` needs `cargo build --bins --examples` first.** `--bins`
   builds the `estigia` binary `tests/pipe.rs` spawns; `--examples` builds the stand-in `gh`
   (`fake_process`) that the pipe tests and the `harness::guard`/`harness::doctor` unit tests read.
   A bare `cargo test` builds both, so the list above is complete as it stands. Narrowing the run
   does not: `cargo test --test pipe` and `cargo test --lib <name>` select a target and leave the
   example unbuilt. Neither does **widening** it, which is the surprising half — `cargo test
   --all-targets` builds the example as a hashed *test* target and never writes the plain
   `fake_process` binary, so it answers `90 passed; 16 failed` on every run until the examples are
   built separately. That is the same flag this list prescribes for clippy one line above.

   **One command for a filtered run on a cold worktree: `scripts/test.sh <cargo test arguments>`.** It
   runs `cargo build --bins --examples` with the `--release`, `--profile`, `--target` and
   `--manifest-path` it finds among those arguments, then `cargo test` with all of them, so the
   fixtures land in the directory the filtered run looks in. The failure message of a missing fixture
   derives the same flags from the test binary's own path (`tests/support/fixture_flags.rs`, shared by
   `src/test_env.rs` and `tests/pipe.rs`), so the command it names is the one that clears it.
   `tests/test_script.rs` runs the script under `sh` against a stand-in `cargo` that logs its argv and
   holds both invocations to the exact command line; like the other `sh` fixtures it fails, not skips,
   without `sh`.

   Tests in `tests/pipe.rs` drive the binary against a stand-in `gh` that is an example. They used to
   report **pass** having executed nothing; they now fail loudly, naming the command that clears it.
   This matters because a filtered run is how mutation is measured here, which is what
   `docs/honesty.md` is made of.

   The same example now reaches `cargo test --lib` too, not only `tests/pipe.rs`: `src/test_env.rs`'s
   `scripted_gh` copies it once per test binary into a directory the transport resolves a scripted
   `gh` from — not onto the process `PATH`, which is what used to serialise every such fixture on one
   environment lock — for unit tests in `harness::guard::tests` and `harness::doctor`'s own module
   that need a `gh` answering a scripted `issue view`, the same way `tests/pipe.rs`'s own
   `tracker_rig` does for the binary. `cargo test --lib guard` or `cargo test
   --lib doctor` on a cold worktree fails the same way a filtered `cargo test --test pipe` does, and
   for the same reason — the fixture is not built, and the failure names the command that builds it
   rather than reporting a pass that measured nothing.
5. **A change does not edit `CHANGELOG.md`.** The agent cutting a version writes that version's entry
   from the pull requests merged since the previous tag, as in
   [Version tags](skill/bindings/github/delivery.md#version-tags).

## What not to do

- Do not widen a gate to make a test pass. A gate that decides nothing is the failure this crate
  exists to refuse, and it always looks like working correctly.
- Do not report a state you did not read back. When a write's outcome cannot be confirmed, say so
  and stop; never report the nearest named state instead.
- Do not name a command in a message unless running it clears the block. Naming a dead end is worse
  than naming nothing.
