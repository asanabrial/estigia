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

   **Run everything**: `cargo test`, `cargo clippy --all-targets`, `cargo fmt`, `cargo doc
   --no-deps`. Ninety-two tests **require** `node` on the path, and all ninety-two drive the OpenCode plugins and
   the Pi extensions this crate generates — because those are JavaScript and asserting on their source text is what
   let a gate defect stand. Nine are on the gate plugin,
   `the_plugin_hands_the_gate_the_directory_the_call_runs_in`,
   `the_plugin_hands_the_gate_the_directory_every_write_lands_in`,
   `the_opencode_plugin_hands_an_oversized_input_to_the_gate_on_standard_input`,
   `the_plugin_says_so_when_the_gate_returns_no_exit_code`, and five that drive its mail delivery,
   `the_opencode_plugin_appends_waiting_mail_to_a_tool_result_after_registering_the_session`,
   `the_opencode_plugin_steers_mail_in_through_the_service_prompt_when_it_answers`,
   `the_opencode_plugin_falls_back_to_a_synthetic_message_when_the_prompt_fails_or_is_absent`,
   `the_opencode_plugin_says_on_standard_error_that_mail_is_lost_when_both_hand_overs_fail` and
   `the_opencode_plugin_leaves_the_mailbox_unread_when_the_host_offers_no_way_to_hand_mail_over`; two drive the Pi gate extension's display,
`the_pi_extension_shows_the_claim_the_binary_reports_and_clears_it_when_there_is_none` and
`the_pi_extension_draws_a_refusal_card_with_the_binarys_words_and_asks_the_model_for_nothing`; seven drive the optional loop
   plugin (`the_loop_plugin_continues_while_estigia_says_there_is_work`,
   `the_loop_plugin_waits_on_held_or_pending_work_and_asks_again`,
   `the_loop_plugin_waits_while_a_child_session_runs_and_resumes_when_it_ends`,
   `the_loop_plugin_defaults_its_objective_to_flow_dev`,
   `the_loop_plugin_records_what_it_saw_and_listens_again`,
   `the_loop_plugin_continues_a_live_claim_with_the_held_text_alone` and
   `the_loop_plugin_stops_on_a_live_answer_that_names_no_issue`); eight drive the generic goal and loop
   plugin through `run_goal_loop_driver`, among them
   `the_generic_goal_judges_every_third_turn_and_continues_the_others_unasked` and
   `the_generic_goal_judges_the_turn_that_reaches_its_cap`; seven drive the review relay plugin —
   `the_review_relay_materialises_the_prompt_and_admits_the_answer`,
   `the_review_relay_intercepts_the_launch_the_relay_answer_prescribes`,
   `the_review_relay_keeps_the_reason_when_the_relay_exits_before_reading_stdin` and
   `the_review_relay_keeps_unavailable_and_stale_apart`,
   `the_review_relay_admits_the_answer_of_a_v2_call_by_the_id_before_remembered`,
   `the_review_relay_attaches_the_answer_by_redirect_and_never_runs_with_an_inherited_stdin` and
   `the_review_relay_serves_the_opencode_1_hook_shape` — the first four reproduce the two v4 relay defects; nine drive
   the Pi gate extension, `the_pi_extension_blocks_on_one_and_two_and_lets_a_gate_that_did_not_answer_through_with_a_word`,
   `the_pi_extension_gates_a_tool_by_the_shape_of_its_input_and_not_only_by_its_name`,
   `the_pi_extension_bounds_the_gate_call_and_shows_the_person_a_write_that_went_out_unchecked`,
   `the_pi_extension_blocks_an_answer_larger_than_it_can_read_and_does_not_call_it_a_timeout`,
   `the_pi_extension_hands_an_oversized_input_to_the_gate_on_standard_input`,
   `the_pi_extension_lets_a_gate_that_cannot_be_started_through_with_a_word_whatever_the_ui_does`,
   `the_pi_extension_puts_a_question_to_a_person_only_in_the_tui_and_blocks_on_any_code_it_cannot_read`,
   `the_pi_extension_hands_the_binary_the_root_session_only_from_a_process_that_inherited_one`
   and `the_pi_extension_links_a_child_by_its_parent_header_whatever_its_pid`;
   three drive its `/estigia-doctor` and `/estigia-changes` commands and its uncommitted-work widget through `run_pi_claim_driver`,
   `the_pi_extension_shows_estigia_doctor_unchanged_to_the_person_and_asks_the_model_for_nothing`,
   `the_pi_extension_shows_estigia_changes_to_the_person_and_asks_the_model_for_nothing` and
   `the_pi_extension_shows_a_widget_counting_the_checkouts_that_hold_uncommitted_work`;
   eight drive the Pi gate extension's session context through `drive_pi_session` —
   `the_pi_extension_appends_the_binarys_session_context_to_the_prompt_sections_and_replaces_nothing`,
   `the_pi_extension_forwards_why_a_session_started_and_which_session_it_came_from`,
   `the_pi_extension_says_the_session_context_is_unknown_when_the_binary_does_not_answer`,
   `the_pi_extension_asks_again_for_every_run_so_a_claim_made_since_is_never_stale`,
   `the_pi_extension_ends_the_session_once_even_when_shutdown_fires_twice`,
   `the_pi_extension_delivers_waiting_mail_as_its_own_prompt_section_at_before_agent_start`,
   `the_pi_extension_adds_no_mail_section_when_nothing_waits_or_the_mailbox_cannot_be_read` and
   `a_pi_child_asks_about_the_root_session_and_never_ends_it`;
   four drive the Pi gate extension's wake of an idle session through `drive_pi_wake` —
   `an_idle_pi_session_with_waiting_mail_is_woken_with_a_follow_up_and_then_takes_it`,
   `a_wake_that_throws_or_cannot_be_made_leaves_the_mail_pending_for_the_next_turn`,
   `a_pi_child_never_polls_and_the_root_stops_at_a_turn_and_at_shutdown` and
   `a_turn_started_by_a_wake_reads_the_mailbox_only_after_the_wake_has_taken_its_mail`;
   and three drive the Pi review relay extension —
   `the_pi_review_relay_materialises_the_prompt_and_admits_the_answer`,
   `the_pi_review_relay_keeps_the_reason_when_the_binary_exits_before_reading_stdin` and
   `the_pi_review_relay_keeps_unavailable_and_stale_apart`; and eleven drive the Pi loop extension
   through `run_pi_loop_driver` —
   `the_pi_loop_registers_its_commands_and_listens_for_the_settled_turn`,
   `the_pi_loop_continues_while_estigia_says_there_is_work`,
   `the_pi_loop_waits_on_held_or_pending_work_and_asks_again`,
   `the_pi_loop_stops_and_says_so_when_estigia_does_not_answer`,
   `the_pi_loop_acts_only_in_the_session_that_asked_and_on_a_settled_idle_turn`,
   `the_pi_loop_defaults_its_objective_to_flow_dev`,
   `a_pi_loop_timer_enqueues_through_the_extension_that_armed_it`,
   `the_pi_loop_stops_on_a_requested_stop_and_clears_the_request_once`,
   `the_pi_loop_continues_a_live_claim_with_the_held_text_alone`,
   `the_pi_loop_stops_on_a_live_answer_that_names_no_issue` and
   `the_pi_loop_reports_what_it_holds_and_what_the_tracker_says`; and twelve drive the Pi goal and loop
   extension through `run_pi_goal_loop_driver` —
   `the_pi_goal_loop_registers_goal_and_loop_and_nothing_that_collides`,
   `the_pi_goal_continues_on_an_unmet_judgement_and_stops_on_a_met_one`,
   `the_pi_goal_stops_at_its_turn_cap`,
   `the_pi_goal_stops_on_an_evaluator_answer_it_cannot_read`,
   `a_judgement_that_never_answers_is_bounded_and_read_as_no_answer`,
   `a_stop_a_replacement_or_a_session_change_mid_judgement_is_not_overwritten`,
   `the_pi_goal_reports_clears_and_acts_only_on_a_settled_idle_turn_of_its_session`,
   `a_child_session_runs_neither_goal_nor_loop`,
   `the_pi_goal_judges_every_third_turn_and_continues_the_others_unasked`,
   `the_pi_goal_judges_the_turn_that_reaches_its_cap`,
   `the_pi_goal_bounds_the_transcript_the_evaluator_is_shown` and
   `the_pi_loop_repeats_on_an_interval_and_after_each_turn_and_stops`; and eight drive the Pi subagent
   runner through `run_pi_agents_driver`, on Unix only, since the stand-in `pi` is an executable script —
   `the_pi_subagent_runner_links_the_child_to_the_parent_and_shows_what_the_events_say`,
   `a_child_refused_by_the_gate_shows_the_refusal_on_its_card_unchanged`,
   `stopping_a_pi_subagent_terminates_its_process_and_answers_the_tool`,
   `a_pi_subagent_is_not_given_work_unless_it_loaded_the_gate_and_is_linked_to_a_session`,
   `the_pi_subagent_view_lists_children_and_shows_each_transcript_with_no_model_call`,
   `a_running_pi_subagent_is_steered_from_the_view_and_a_finished_one_is_not`,
   `the_pi_subagent_view_stops_one_child_and_stops_all_only_after_a_confirm` and
   `finished_pi_subagent_tasks_are_kept_as_session_entries_and_never_sent_to_the_model`.
   None skips when `node` is missing; all fail and say so, since a skip
   spelled as a pass is a defect this repository has filed against itself. Others *ask* for an
   interpreter and tolerate its absence — `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer`
   spawns `node` to check the plugin parses, and several fixtures spawn `sh` — so the honest line is
   that ninety-two tests require `node` and one requires `bash`, and nothing else does. `doctor` asks for
   none. The `bash` one, `nothing_bash_executes_is_blanked_as_data`, fails, not skips, without it.
   `cargo doc` is on this list because it was not, and
   had been failing outright — four public pages linking to private items, and neither the list here
   nor CI ever asked. A crate whose public documentation does not build is one nobody
   reads, and item 2 leaves that documentation carrying what an item is and what it refuses.

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
