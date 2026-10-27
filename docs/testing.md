# Which test drives which integration, and what each one needs

The tests that run something outside this process: which driver carries them, which generated
integration each one exercises, and what the machine has to have for them to run at all. It answers
*is this red line about the change or about the machine* — not *what to run before publishing*,
which is [`AGENTS.md`](../AGENTS.md)'s **Run everything** item, and not what the suite proves, which
is [`honesty.md`](honesty.md)'s subject.

The counts on this page are prose, read from the suite by whoever last moved the list. No test
derives them from the test functions, so a new driver that spawns `node` changes the truth of this
page without reddening anything.

## The generated JavaScript, and `node`

One hundred and eleven tests **require** `node` on the path, and one hundred and six of them drive the
OpenCode plugins and the Pi extensions this crate generates — because those are JavaScript, and
asserting on their source text is what let a gate defect stand.

### The OpenCode gate plugin

Nine are on the gate plugin, `the_plugin_hands_the_gate_the_directory_the_call_runs_in`,
`the_plugin_hands_the_gate_the_directory_every_write_lands_in`,
`the_opencode_plugin_hands_an_oversized_input_to_the_gate_on_standard_input` and
`the_plugin_says_so_when_the_gate_returns_no_exit_code`, and five that drive its mail delivery,
`the_opencode_plugin_appends_waiting_mail_to_a_tool_result_after_registering_the_session`,
`the_opencode_plugin_steers_mail_in_through_the_service_prompt_when_it_answers`,
`the_opencode_plugin_falls_back_to_a_synthetic_message_when_the_prompt_fails_or_is_absent`,
`the_opencode_plugin_says_on_standard_error_that_mail_is_lost_when_both_hand_overs_fail` and
`the_opencode_plugin_leaves_the_mailbox_unread_when_the_host_offers_no_way_to_hand_mail_over`.

### The Pi gate extension

Two drive its display,
`the_pi_extension_shows_the_claim_the_binary_reports_and_clears_it_when_there_is_none` and
`the_pi_extension_draws_a_refusal_card_with_the_binarys_words_and_asks_the_model_for_nothing`.

Nine drive the extension itself,
`the_pi_extension_blocks_on_one_and_two_and_lets_a_gate_that_did_not_answer_through_with_a_word`,
`the_pi_extension_gates_a_tool_by_the_shape_of_its_input_and_not_only_by_its_name`,
`the_pi_extension_bounds_the_gate_call_and_shows_the_person_a_write_that_went_out_unchecked`,
`the_pi_extension_blocks_an_answer_larger_than_it_can_read_and_does_not_call_it_a_timeout`,
`the_pi_extension_hands_an_oversized_input_to_the_gate_on_standard_input`,
`the_pi_extension_lets_a_gate_that_cannot_be_started_through_with_a_word_whatever_the_ui_does`,
`the_pi_extension_puts_a_question_to_a_person_only_in_the_tui_and_blocks_on_any_code_it_cannot_read`,
`the_pi_extension_hands_the_binary_the_root_session_only_from_a_process_that_inherited_one` and
`the_pi_extension_links_a_child_by_its_parent_header_whatever_its_pid`.

Three drive its `/estigia-doctor` and `/estigia-changes` commands and its uncommitted-work widget
through `run_pi_claim_driver`,
`the_pi_extension_shows_estigia_doctor_unchanged_to_the_person_and_asks_the_model_for_nothing`,
`the_pi_extension_shows_estigia_changes_to_the_person_and_asks_the_model_for_nothing` and
`the_pi_extension_shows_a_widget_counting_the_checkouts_that_hold_uncommitted_work`.

Eight drive its session context through `drive_pi_session` —
`the_pi_extension_appends_the_binarys_session_context_to_the_prompt_sections_and_replaces_nothing`,
`the_pi_extension_forwards_why_a_session_started_and_which_session_it_came_from`,
`the_pi_extension_says_the_session_context_is_unknown_when_the_binary_does_not_answer`,
`the_pi_extension_asks_again_for_every_run_so_a_claim_made_since_is_never_stale`,
`the_pi_extension_ends_the_session_once_even_when_shutdown_fires_twice`,
`the_pi_extension_delivers_waiting_mail_as_its_own_prompt_section_at_before_agent_start`,
`the_pi_extension_adds_no_mail_section_when_nothing_waits_or_the_mailbox_cannot_be_read` and
`a_pi_child_asks_about_the_root_session_and_never_ends_it`.

Four drive its wake of an idle session through `drive_pi_wake` —
`an_idle_pi_session_with_waiting_mail_is_woken_with_a_follow_up_and_then_takes_it`,
`a_wake_that_throws_or_cannot_be_made_leaves_the_mail_pending_for_the_next_turn`,
`a_pi_child_never_polls_and_the_root_stops_at_a_turn_and_at_shutdown` and
`a_turn_started_by_a_wake_reads_the_mailbox_only_after_the_wake_has_taken_its_mail`.

Three drive the Pi review relay extension —
`the_pi_review_relay_materialises_the_prompt_and_admits_the_answer`,
`the_pi_review_relay_keeps_the_reason_when_the_binary_exits_before_reading_stdin` and
`the_pi_review_relay_keeps_unavailable_and_stale_apart`.

Eight drive the Pi subagent runner through `run_pi_agents_driver`, on Unix only, since the stand-in
`pi` is an executable script —
`the_pi_subagent_runner_links_the_child_to_the_parent_and_shows_what_the_events_say`,
`a_child_refused_by_the_gate_shows_the_refusal_on_its_card_unchanged`,
`stopping_a_pi_subagent_terminates_its_process_and_answers_the_tool`,
`a_pi_subagent_is_not_given_work_unless_it_loaded_the_gate_and_is_linked_to_a_session`,
`the_pi_subagent_view_lists_children_and_shows_each_transcript_with_no_model_call`,
`a_running_pi_subagent_is_steered_from_the_view_and_a_finished_one_is_not`,
`the_pi_subagent_view_stops_one_child_and_stops_all_only_after_a_confirm` and
`finished_pi_subagent_tasks_are_kept_as_session_entries_and_never_sent_to_the_model`.

Seven drive its parent and child messages through `run_pi_messages_driver`, Unix only for the same
reason —
`a_parent_steers_a_running_pi_child_and_a_finished_or_unknown_one_is_an_error`,
`a_pi_child_notification_reaches_the_parent_as_a_follow_up_that_wakes_it`,
`a_pi_child_query_blocks_until_the_parent_answers_and_the_answer_is_its_tool_result`,
`an_unanswered_pi_child_query_ends_at_its_timeout_with_an_error` and
`the_child_side_message_tool_exists_only_in_a_session_the_runner_spawned`,
`a_background_pi_subagent_returns_at_once_and_delivers_its_answer_as_a_waking_follow_up` and
`a_query_from_a_child_the_parent_is_blocked_on_fails_at_once_and_is_never_relayed`.

### The OpenCode 2 gate plugin's wake

Five drive it through `run_opencode_wake_driver` —
`an_idle_opencode_root_session_with_mail_gets_one_queued_prompt_and_then_a_take_through_the_peeked_row`,
`an_opencode_wake_that_the_host_refuses_or_cannot_make_leaves_the_rows_pending`,
`an_opencode_session_is_polled_only_while_it_is_idle_and_a_root`,
`disposing_the_opencode_plugin_stops_the_poll_and_the_event_stream` and
`two_opencode_polls_are_never_in_flight_for_one_session_and_a_session_that_started_meanwhile_is_not_prompted`.

### The loop plugins and the generic goal

Seven drive the optional loop plugin (`the_loop_plugin_continues_while_estigia_says_there_is_work`,
`the_loop_plugin_waits_on_held_or_pending_work_and_asks_again`,
`the_loop_plugin_waits_while_a_child_session_runs_and_resumes_when_it_ends`,
`the_loop_plugin_defaults_its_objective_to_flow_dev`,
`the_loop_plugin_records_what_it_saw_and_listens_again`,
`the_loop_plugin_continues_a_live_claim_with_the_held_text_alone` and
`the_loop_plugin_stops_on_a_live_answer_that_names_no_issue`).

Eight drive the generic goal and loop plugin through `run_goal_loop_driver`, among them
`the_generic_goal_judges_every_third_turn_and_continues_the_others_unasked` and
`the_generic_goal_judges_the_turn_that_reaches_its_cap`.

Eleven drive the Pi loop extension through `run_pi_loop_driver` —
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
`the_pi_loop_reports_what_it_holds_and_what_the_tracker_says`.

Twelve drive the Pi goal and loop extension through `run_pi_goal_loop_driver` —
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
`the_pi_loop_repeats_on_an_interval_and_after_each_turn_and_stops`.

### The review relay plugin

Seven drive it —
`the_review_relay_materialises_the_prompt_and_admits_the_answer`,
`the_review_relay_intercepts_the_launch_the_relay_answer_prescribes`,
`the_review_relay_keeps_the_reason_when_the_relay_exits_before_reading_stdin` and
`the_review_relay_keeps_unavailable_and_stale_apart`,
`the_review_relay_admits_the_answer_of_a_v2_call_by_the_id_before_remembered`,
`the_review_relay_attaches_the_answer_by_redirect_and_never_runs_with_an_inherited_stdin` and
`the_review_relay_serves_the_opencode_1_hook_shape` — the first four reproduce the two v4 relay
defects. The acknowledgment-channel regression
`an_opencode_reviewer_can_acknowledge_repository_instructions_before_its_result` also drives the
generated relay under Node and hands its output to real ledger admission, and
`a_relayed_answer_is_admitted_against_the_job_receipt` also drives the generated plugin's native
structured completion under Node before handing its bytes to real ledger admission.

## The deadline every `node` driver runs under

The last of them, `a_node_driver_that_never_exits_fails_within_its_deadline_and_shows_what_it_printed`,
drives no generated code: it holds the deadline every one of the others runs under. Every `node`
launch in `src/setup/tests.rs` goes through `run_node`, which drains both pipes on threads and, past
120 s (the slowest driver measured 12 s under the full parallel run), kills and reaps `node` and
fails naming the test and printing what it had written, so a hung driver reads as a failure with
evidence instead of a cancelled job.

## How the CI lane runs the suite, and the deadline on every test

The `check` job runs the suite under [`cargo-nextest`](https://nexte.st), installed by
`taiki-e/install-action`: `cargo build --bins --examples` first, then
`cargo nextest run --profile ci --all-features --no-fail-fast --locked` (on Windows, for every target
but the lib; see below), then
`cargo test --doc --all-features --locked`, because nextest runs no doc tests. The bound is
[`.config/nextest.toml`](../.config/nextest.toml): a test still running after `60s` is reported slow,
and after three periods (180 s) it is killed and the job fails with its name in the log. Before this
a hung test was ended only by the job's limit, and a job ended that way left no log to read (run
40399731794, `windows-11-arm`, 55 minutes lost). The `check` limit is 30 minutes for that reason: it
only has to cover a normal run. `tests/release.rs` pins the step order, the flags, the limit and the
config.

nextest runs one process per test. `ENVIRONMENT` in `src/test_env.rs` and the other process-wide
statics only exclude threads of one process, so with one test per process they are uncontended and
exclude nothing that was not already alone; the `OnceLock` caches (the fixture copies, the generated
scripts) are rebuilt per test rather than once per binary, which costs time and decides nothing. A
filtered local run is still `cargo test` or `scripts/test.sh`; nextest is not required to work here.

**On Windows the lib target is not bounded.** There a process per test costs what the other runners do
not: with everything under nextest, `check (windows-latest)` took 21m04s, 16m27s and 20m34s on one head
(median 20m34s) against a libtest median of 16m55s over the thirteen green runs before it, a fifth
more. So on Windows nextest runs every target but the lib (`-E "not kind(lib)"`), and the lib's
~3,700 tests run under `cargo test --lib --no-fail-fast`, where a hang still ends only at the job's
30-minute limit. Every other runner bounds every test. The doc tests and the Windows lib step run
after a red step too (`if: success() || failure()`), so a red suite hides neither.

The `node` drivers keep their own 120 s deadline above, which sits inside nextest's 180 s.

## Interpreters, and what happens without one

None of those skips when `node` is missing; all fail and say so, since a skip spelled as a pass is a
defect this repository has filed against itself. Others *ask* for an interpreter and tolerate its
absence — `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer` spawns `node` to check the
plugin parses, and several fixtures spawn `sh` — so the honest line is that one hundred and eleven
tests require `node`, four of them `tar` too, and one requires `bash`, and nothing else does except `sh`: the checks tests in
`src/transport/claim/tests/checks.rs` declare their commands as `sh -c`, so they need `sh` on the path
and a declared check that cannot start does not pass them. `doctor` asks for none. The `bash` one, `nothing_bash_executes_is_blanked_as_data`, fails, not
skips, without it.

Four in [`tests/npm.rs`](../tests/npm.rs) drive the npm installer, `npm/bin/estigia.js`, on Unix only
and with `tar` as well:
`the_npm_installer_records_and_places_a_verified_binary_and_runs_it`,
`the_npm_installer_refuses_an_archive_that_does_not_match_its_sum`,
`the_npm_installer_refuses_an_archive_with_no_published_sum` and
`the_npm_installer_places_nothing_the_lifecycle_record_refused`.

One more interpreter joined that list with the transcript accounting: the nine tests in
[`tests/usage_accounting.rs`](../tests/usage_accounting.rs) run the measurement script under
`python3`, the interpreter that script is written in, and they are `#[cfg(unix)]` because `python3`
is not the name Windows ships. They fail, never skip, without it, so the honest line is that one
test requires `bash`, nine require `python3` on Unix, and nothing else does but the `sh` the checks tests
spawn; all of those read a
missing interpreter as a broken build rather than an unbuilt fixture.

## Why `cargo doc` is on the list

`cargo doc --no-deps` is one of the four commands [`AGENTS.md`](../AGENTS.md) requires, and it was
not before: it had been failing outright on four public pages linking to private items, and neither
the list nor CI ever asked. A crate whose public documentation does not build is one nobody reads,
and the rules of this repository leave that documentation carrying what an item is and what it
refuses.

Reading a transcript is [`review-cost.md`](review-cost.md)'s subject; what that accounting cannot
establish is [`honesty.md`](honesty.md)'s.
