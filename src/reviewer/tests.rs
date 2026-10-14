use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::adapters::{self, claude_args, codex_args, indicates_unavailable};
use super::scratch::Scratch;
use super::*;
use crate::test_env::with_fake_clis;

fn invocation(prompt: &[u8], model: Option<&str>) -> Invocation {
    Invocation {
        role: "review-risk".to_owned(),
        receipt: Receipt {
            epoch: "e1".to_owned(),
            pr: 7,
            head: "aaa".to_owned(),
            base: "bbb".to_owned(),
            digest: "ddd".to_owned(),
            payload_digest: "ppp".to_owned(),
        },
        prompt: prompt.to_vec(),
        model: model.map(str::to_owned),
    }
}

struct Seen {
    argv: Vec<String>,
    cwd: String,
}

fn read_argv(path: &Path) -> Seen {
    let bytes = std::fs::read(path).expect("the fake recorded its launch");
    let mut records: Vec<String> = bytes
        .split(|byte| *byte == 0)
        .map(|record| String::from_utf8_lossy(record).into_owned())
        .collect();
    records.pop();
    let cwd = records
        .pop()
        .expect("the working directory is recorded last");
    Seen { argv: records, cwd }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn claude_base() -> Vec<String> {
    let mut args = strings(&[
        "--print",
        "--output-format",
        "json",
        "--tools",
        "",
        "--permission-mode",
        "dontAsk",
        "--no-session-persistence",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--system-prompt",
    ]);
    args.push(super::adapters::ISOLATED_SYSTEM_PROMPT.to_owned());
    args
}

fn on_path(bin: &Path, name: &str) -> PathBuf {
    let program = bin.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    });
    program
        .is_file()
        .then_some(program)
        .expect("the fake CLI exists")
}

fn select_with(bin: &Path, relay: bool, relay_first: bool) -> Result<Box<dyn Adapter>, Refusal> {
    select_adapter_with(
        |name| {
            let program = bin.join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.to_owned()
            });
            program.is_file().then_some(program)
        },
        relay,
        relay_first,
    )
}

fn select_in(bin: &Path) -> Result<Box<dyn Adapter>, Refusal> {
    select_with(bin, false, false)
}

fn select_relay_in(bin: &Path) -> Result<Box<dyn Adapter>, Refusal> {
    select_with(bin, true, false)
}

#[test]
fn claude_runs_with_exactly_the_isolating_arguments() {
    let argv = tempfile::tempdir().unwrap();
    let log = argv.path().join("argv");
    with_fake_clis(
        &["claude"],
        &[
            ("ESTIGIA_FAKE_ARGV_LOG", log.as_os_str()),
            (
                "ESTIGIA_FAKE_ANSWERS",
                OsStr::new(
                    r#"[{"stdout_exact":"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}"}]"#,
                ),
            ),
        ],
        |bin| {
            let adapter = ClaudeCode::new(on_path(bin, "claude"));
            adapter.review(&invocation(b"p", None)).unwrap();
            assert_eq!(read_argv(&log).argv, claude_base());
            adapter.review(&invocation(b"p", Some("opus"))).unwrap();
            let mut with_model = claude_base();
            with_model.extend(strings(&["--model", "opus"]));
            assert_eq!(read_argv(&log).argv, with_model);
        },
    );
}

#[test]
fn codex_runs_with_exactly_the_isolating_arguments() {
    let argv = tempfile::tempdir().unwrap();
    let log = argv.path().join("argv");
    with_fake_clis(
        &["codex"],
        &[
            ("ESTIGIA_FAKE_ARGV_LOG", log.as_os_str()),
            ("ESTIGIA_FAKE_WRITE_ARG_FILE", OsStr::new("answer")),
        ],
        |bin| {
            let adapter = Codex::new(on_path(bin, "codex"));
            adapter.review(&invocation(b"p", None)).unwrap();
            let seen = read_argv(&log);
            let scratch = PathBuf::from(&seen.argv[6]);
            let mut plain = strings(&[
                "exec",
                "--skip-git-repo-check",
                "--ignore-user-config",
                "--sandbox",
                "read-only",
                "-C",
            ]);
            plain.push(scratch.to_string_lossy().into_owned());
            plain.push("--output-last-message".to_owned());
            plain.push(
                scratch
                    .join("last-message.txt")
                    .to_string_lossy()
                    .into_owned(),
            );
            for feature in [
                "shell_tool",
                "unified_exec",
                "unified_exec_tty",
                "write_stdin_approval",
                "sleep_tool",
                "code_mode_host",
                "view_image",
                "image_generation",
                "apps",
                "plugins",
                "plugin_sharing",
                "remote_plugin",
                "skill_mcp_dependency_install",
                "skill_search",
                "tool_suggest",
                "browser_use",
                "browser_use_external",
                "browser_use_full_cdp_access",
                "computer_use",
                "in_app_browser",
                "in_app_local_automation",
                "hooks",
                "multi_agent",
            ] {
                plain.push("--disable".to_owned());
                plain.push(feature.to_owned());
            }
            plain.extend(strings(&["-c", "shell_environment_policy.inherit=none"]));
            assert_eq!(seen.argv, plain);
            assert_eq!(
                std::fs::canonicalize(scratch.parent().unwrap()).unwrap(),
                std::fs::canonicalize(std::env::temp_dir()).unwrap()
            );

            adapter.review(&invocation(b"p", Some("gpt-5"))).unwrap();
            let seen = read_argv(&log);
            assert_eq!(
                &seen.argv[seen.argv.len() - 2..],
                &strings(&["--model", "gpt-5"])
            );
            assert_eq!(seen.argv.len(), plain.len() + 2);
        },
    );
}

#[test]
fn argument_builders_name_no_model_flag_without_a_model() {
    assert!(!claude_args(None).iter().any(|arg| arg == "--model"));
    assert!(
        !codex_args(Path::new("s"), Path::new("f"), None)
            .iter()
            .any(|arg| arg == "--model")
    );
}

#[test]
fn the_prompt_arrives_on_standard_input_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let stdin_log = dir.path().join("stdin");
    let prompt: Vec<u8> = (0..300_000_u32).map(|n| (n % 251) as u8).collect();
    for name in ["claude", "codex"] {
        let _ = std::fs::remove_file(&stdin_log);
        with_fake_clis(
            &[name],
            &[
                ("ESTIGIA_FAKE_STDIN_LOG", stdin_log.as_os_str()),
                ("ESTIGIA_FAKE_WRITE_ARG_FILE", OsStr::new("answer")),
                (
                    "ESTIGIA_FAKE_ANSWERS",
                    OsStr::new(
                        r#"[{"stdout_exact":"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}"}]"#,
                    ),
                ),
            ],
            |bin| {
                let adapter = select_in(bin).unwrap();
                adapter.review(&invocation(&prompt, None)).unwrap();
            },
        );
        assert_eq!(std::fs::read(&stdin_log).unwrap(), prompt, "{name}");
    }
}

#[test]
fn the_working_directory_is_a_fresh_scratch_directory_that_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("argv");
    with_fake_clis(
        &["claude"],
        &[
            ("ESTIGIA_FAKE_ARGV_LOG", log.as_os_str()),
            (
                "ESTIGIA_FAKE_ANSWERS",
                OsStr::new(
                    r#"[{"stdout_exact":"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}"}]"#,
                ),
            ),
        ],
        |bin| {
            ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap();
            let cwd = PathBuf::from(read_argv(&log).cwd);
            let temp = std::env::temp_dir();
            assert!(
                crate::paths::same_directory(cwd.parent().unwrap(), &temp),
                "{} is under {}",
                cwd.display(),
                temp.display()
            );
            assert!(
                cwd.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("estigia-reviewer-")
            );
            assert!(!cwd.exists(), "{} outlived the run", cwd.display());
        },
    );
}

#[test]
fn a_scratch_directory_starts_empty_and_is_unique() {
    let first = Scratch::new().unwrap();
    let second = Scratch::new().unwrap();
    assert_ne!(first.path(), second.path());
    assert_eq!(std::fs::read_dir(first.path()).unwrap().count(), 0);
    let gone = first.path().to_path_buf();
    drop(first);
    assert!(!gone.exists());
}

#[test]
fn claude_returns_what_the_cli_printed() {
    with_fake_clis(
        &["claude"],
        &[(
            "ESTIGIA_FAKE_ANSWERS",
            OsStr::new(
                r#"[{"stdout_exact":"{\"type\":\"result\",\"is_error\":false,\"result\":\"the answer\"}"}]"#,
            ),
        )],
        |bin| {
            let answer = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap();
            assert_eq!(answer, b"the answer");
        },
    );
}

#[test]
fn codex_returns_the_last_message_file_and_not_standard_output() {
    with_fake_clis(
        &["codex"],
        &[
            ("ESTIGIA_FAKE_WRITE_ARG_FILE", OsStr::new("from the file")),
            (
                "ESTIGIA_FAKE_ANSWERS",
                OsStr::new(r#"[{"stdout_exact":"progress noise"}]"#),
            ),
        ],
        |bin| {
            let answer = Codex::new(on_path(bin, "codex"))
                .review(&invocation(b"p", None))
                .unwrap();
            assert_eq!(answer, b"from the file");
        },
    );
}

#[test]
fn codex_without_a_last_message_file_refuses_rather_than_reading_stdout() {
    with_fake_clis(
        &["codex"],
        &[(
            "ESTIGIA_FAKE_ANSWERS",
            OsStr::new(r#"[{"stdout_exact":"progress noise"}]"#),
        )],
        |bin| {
            let refusal = Codex::new(on_path(bin, "codex"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(refusal.code, "reviewer-no-answer", "{}", refusal.message);
        },
    );
}

#[test]
fn a_program_that_will_not_start_is_reviewer_cli_missing() {
    let refusal = ClaudeCode::new(PathBuf::from("/nonexistent/estigia/claude"))
        .review(&invocation(b"p", None))
        .unwrap_err();
    assert_eq!(refusal.code, "reviewer-cli-missing", "{}", refusal.message);
    let refusal = Codex::new(PathBuf::from("/nonexistent/estigia/codex"))
        .review(&invocation(b"p", None))
        .unwrap_err();
    assert_eq!(refusal.code, "reviewer-cli-missing", "{}", refusal.message);
}

#[test]
fn a_non_zero_exit_is_reviewer_exited_nonzero_and_names_stderr() {
    with_fake_clis(
        &["claude", "codex"],
        &[(
            "ESTIGIA_FAKE_ANSWERS",
            OsStr::new(r#"[{"stderr":"segfault in the thing","status":3}]"#),
        )],
        |bin| {
            for refusal in [
                ClaudeCode::new(on_path(bin, "claude"))
                    .review(&invocation(b"p", None))
                    .unwrap_err(),
                Codex::new(on_path(bin, "codex"))
                    .review(&invocation(b"p", None))
                    .unwrap_err(),
            ] {
                assert_eq!(
                    refusal.code, "reviewer-exited-nonzero",
                    "{}",
                    refusal.message
                );
                assert!(refusal.message.contains("segfault in the thing"));
                assert!(refusal.message.contains("exited with 3"));
            }
        },
    );
}

#[test]
fn quota_or_login_trouble_on_a_failed_exit_is_reviewer_unavailable() {
    for stderr in [
        "Credit balance is too low",
        "You have hit your usage limit",
        "Error: not logged in. Please run /login",
        "429 rate limit exceeded",
    ] {
        let answers = serde_json::json!([{ "stderr": stderr, "status": 1 }]).to_string();
        with_fake_clis(
            &["claude"],
            &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&answers))],
            |bin| {
                let refusal = ClaudeCode::new(on_path(bin, "claude"))
                    .review(&invocation(b"p", None))
                    .unwrap_err();
                assert_eq!(refusal.code, "reviewer-unavailable", "{stderr}");
            },
        );
    }
}

#[test]
fn a_login_refusal_names_the_environment_the_reviewer_was_launched_with() {
    let answers = serde_json::json!([{
        "stderr": "Not logged in \u{b7} Please run /login",
        "status": 1
    }])
    .to_string();
    with_fake_clis(
        &["claude"],
        &[
            ("ESTIGIA_FAKE_ANSWERS", OsStr::new(&answers)),
            ("HOME", OsStr::new("/launch/home")),
            ("CLAUDE_CONFIG_DIR", OsStr::new("/launch/claude-config")),
        ],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(refusal.code, "reviewer-unavailable", "{}", refusal.message);
            assert!(
                refusal.message.contains("HOME=/launch/home"),
                "{}",
                refusal.message
            );
            assert!(
                refusal
                    .message
                    .contains("CLAUDE_CONFIG_DIR=/launch/claude-config"),
                "{}",
                refusal.message
            );
            assert!(
                refusal.message.contains("different environment"),
                "{}",
                refusal.message
            );
        },
    );
}

#[test]
fn a_quota_refusal_keeps_its_wording_and_names_no_environment() {
    let answers =
        serde_json::json!([{ "stderr": "Credit balance is too low", "status": 1 }]).to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&answers))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert!(
                refusal.message.ends_with(
                    "exited with 1 and reported that it is unavailable: Credit balance is too low"
                ),
                "{}",
                refusal.message
            );
            assert!(!refusal.message.contains("HOME="), "{}", refusal.message);
        },
    );
}

#[test]
fn an_unavailable_marker_on_a_zero_exit_is_not_a_refusal() {
    with_fake_clis(
        &["claude"],
        &[(
            "ESTIGIA_FAKE_ANSWERS",
            OsStr::new(
                r#"[{"stdout_exact":"{\"type\":\"result\",\"is_error\":false,\"result\":\"quota discussed\"}","stderr":"login"}]"#,
            ),
        )],
        |bin| {
            let answer = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap();
            assert_eq!(answer, b"quota discussed");
        },
    );
}

#[test]
fn the_unavailable_matcher_is_narrow() {
    assert!(indicates_unavailable("Invalid API key"));
    assert!(indicates_unavailable("Authentication failed"));
    assert!(indicates_unavailable("BILLING problem"));
    assert!(!indicates_unavailable("panic: index out of bounds"));
    assert!(!indicates_unavailable(""));
}

#[test]
fn output_past_the_cap_is_reviewer_output_too_large() {
    let over = (run::MAX_OUTPUT + 1).to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_BIG_STDOUT", OsStr::new(&over))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(
                refusal.code, "reviewer-output-too-large",
                "{}",
                refusal.message
            );
        },
    );
}

#[test]
fn output_exactly_at_the_cap_is_not_too_large() {
    let exact = run::MAX_OUTPUT.to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_BIG_STDOUT", OsStr::new(&exact))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(
                refusal.code, "reviewer-launch-failed",
                "{}",
                refusal.message
            );
        },
    );
}

#[test]
fn a_reviewer_that_outlives_the_deadline_is_killed_and_refused() {
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_DELAY_MS", OsStr::new("20000"))],
        |bin| {
            let started = Instant::now();
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .with_deadline(Duration::from_millis(300))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(refusal.code, "reviewer-deadline", "{}", refusal.message);
            assert!(started.elapsed() < Duration::from_secs(10));
        },
    );
}

#[test]
fn selection_prefers_claude_then_codex_then_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("argv");
    let variables: [(&'static str, &OsStr); 3] = [
        (
            "ESTIGIA_FAKE_ANSWERS",
            OsStr::new(
                r#"[{"stdout_exact":"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}"}]"#,
            ),
        ),
        ("ESTIGIA_FAKE_ARGV_LOG", log.as_os_str()),
        ("ESTIGIA_FAKE_WRITE_ARG_FILE", OsStr::new("answer")),
    ];
    with_fake_clis(&["claude", "codex"], &variables, |bin| {
        select_in(bin)
            .unwrap()
            .review(&invocation(b"p", None))
            .unwrap();
        assert_eq!(read_argv(&log).argv[0], "--print");
    });
    with_fake_clis(&["codex"], &variables, |bin| {
        select_in(bin)
            .unwrap()
            .review(&invocation(b"p", None))
            .unwrap();
        assert_eq!(read_argv(&log).argv[0], "exec");
    });
    with_fake_clis(&[], &variables, |bin| {
        let refusal = select_in(bin).err().expect("no reviewer CLI is installed");
        assert_eq!(refusal.code, "reviewer-cli-missing", "{}", refusal.message);
    });
}

#[test]
fn selection_takes_the_relay_when_no_cli_is_installed() {
    let variables: [(&'static str, &OsStr); 0] = [];
    with_fake_clis(&[], &variables, |bin| {
        let relay = select_relay_in(bin).expect("the relay is selected");
        assert!(relay.relays(), "the relay does not say it relays");
        let refusal = relay.review(&invocation(b"p", None)).unwrap_err();
        assert_eq!(refusal.code, "reviewer-unavailable", "{}", refusal.message);
        assert!(
            refusal.message.contains("claude") && refusal.message.contains("codex"),
            "the refusal does not name the CLI to install: {}",
            refusal.message
        );
    });
    with_fake_clis(&["claude"], &variables, |bin| {
        let adapter = select_relay_in(bin).expect("the CLI is preferred");
        assert!(!adapter.relays(), "a CLI was displaced by the relay");
    });
}

#[test]
fn an_opencode_run_with_its_relay_installed_takes_the_relay_over_both_clis() {
    let variables: [(&'static str, &OsStr); 0] = [];
    with_fake_clis(&["claude", "codex"], &variables, |bin| {
        let adapter = select_with(bin, true, true).expect("the relay is selected");
        assert!(adapter.relays(), "an installed CLI displaced the relay");
    });
}

#[test]
fn an_opencode_run_without_its_relay_keeps_claude_then_codex_then_refuses() {
    let variables: [(&'static str, &OsStr); 0] = [];
    with_fake_clis(&["claude", "codex"], &variables, |bin| {
        let adapter = select_with(bin, false, false).unwrap();
        assert!(!adapter.relays());
    });
    with_fake_clis(&["codex"], &variables, |bin| {
        let adapter = select_with(bin, false, false).unwrap();
        assert!(!adapter.relays());
    });
    with_fake_clis(&[], &variables, |bin| {
        let refusal = select_with(bin, false, false)
            .err()
            .expect("no reviewer is installed");
        assert_eq!(refusal.code, "reviewer-cli-missing", "{}", refusal.message);
    });
}

#[test]
fn a_run_that_is_not_opencode_keeps_the_cli_first_order_with_a_relay_installed() {
    let log = tempfile::tempdir().unwrap();
    let argv = log.path().join("argv");
    let variables: [(&'static str, &OsStr); 1] = [("ESTIGIA_FAKE_ARGV_LOG", argv.as_os_str())];
    with_fake_clis(&["claude", "codex"], &variables, |bin| {
        let adapter = select_relay_in(bin).unwrap();
        assert!(!adapter.relays(), "a relay displaced the CLI");
    });
}

#[test]
fn only_an_opencode_run_id_prefers_the_relay() {
    assert!(runs_on_opencode("opencode-ses12345deadbeef"));
    assert!(runs_on_opencode("opencode-unknown"));
    assert!(!runs_on_opencode("claude-1f55aecb8cd8068a"));
    assert!(!runs_on_opencode("codex-1f55aecb8cd8068a"));
    assert!(!runs_on_opencode("opencode"));
    assert!(!runs_on_opencode(""));
    assert!(!runs_on_opencode("opencode-"));
}

#[test]
fn the_reviewer_model_reads_its_routing_target_and_inherit_names_none() {
    let mut routing = ModelRouting::default();
    assert_eq!(reviewer_model(&routing), None);
    assert!(routing.assign("reviewer", "opus"));
    assert_eq!(reviewer_model(&routing).as_deref(), Some("opus"));
    assert!(routing.assign("reviewer", "inherit"));
    assert_eq!(reviewer_model(&routing), None);
}

fn routed(spec: &str) -> ModelRouting {
    ModelRouting::parse(spec).expect("a routing")
}

fn models_by_role(routing: &ModelRouting) -> Vec<(Role, Option<String>)> {
    Role::ALL
        .into_iter()
        .map(|role| (role, model_for_role(routing, role)))
        .collect()
}

#[test]
fn with_only_a_reviewer_model_every_role_launches_on_it() {
    for (role, model) in models_by_role(&routed("reviewer=opus")) {
        assert_eq!(model.as_deref(), Some("opus"), "{role:?}");
    }
    for (role, model) in models_by_role(&ModelRouting::default()) {
        assert_eq!(model, None, "{role:?}");
    }
}

#[test]
fn a_refuter_model_moves_only_the_refuter() {
    for (role, model) in models_by_role(&routed("reviewer=opus, refuter=haiku")) {
        let expected = if role == Role::Refuter {
            "haiku"
        } else {
            "opus"
        };
        assert_eq!(model.as_deref(), Some(expected), "{role:?}");
    }
}

#[test]
fn a_validator_model_moves_only_the_validator() {
    for (role, model) in models_by_role(&routed("reviewer=opus, validator=haiku")) {
        let expected = if role == Role::Validator {
            "haiku"
        } else {
            "opus"
        };
        assert_eq!(model.as_deref(), Some(expected), "{role:?}");
    }
}

#[test]
fn a_role_model_stands_without_a_reviewer_model_and_inherit_names_none() {
    let alone = routed("refuter=haiku");
    assert_eq!(
        model_for_role(&alone, Role::Refuter).as_deref(),
        Some("haiku")
    );
    assert_eq!(model_for_role(&alone, Role::Risk), None);
    assert_eq!(model_for_role(&alone, Role::Validator), None);
    let inherited = routed("reviewer=opus, validator=inherit");
    assert_eq!(model_for_role(&inherited, Role::Validator), None);
    assert_eq!(
        model_for_role(&inherited, Role::Lens).as_deref(),
        Some("opus")
    );
}

#[test]
fn every_code_spells_its_stable_name() {
    let names: Vec<&str> = [
        ReviewerCode::CliMissing,
        ReviewerCode::LaunchFailed,
        ReviewerCode::ExitedNonzero,
        ReviewerCode::NoAnswer,
        ReviewerCode::ErrorResult,
        ReviewerCode::OutputTooLarge,
        ReviewerCode::Deadline,
        ReviewerCode::Unavailable,
    ]
    .into_iter()
    .map(ReviewerCode::code)
    .collect();
    assert_eq!(
        names,
        [
            "reviewer-cli-missing",
            "reviewer-launch-failed",
            "reviewer-exited-nonzero",
            "reviewer-no-answer",
            "reviewer-error-result",
            "reviewer-output-too-large",
            "reviewer-deadline",
            "reviewer-unavailable",
        ]
    );
}

#[test]
fn an_empty_answer_is_reviewer_no_answer_not_success() {
    with_fake_clis(
        &["claude", "codex"],
        &[("ESTIGIA_FAKE_WRITE_ARG_FILE", OsStr::new(""))],
        |bin| {
            for refusal in [
                ClaudeCode::new(on_path(bin, "claude"))
                    .review(&invocation(b"p", None))
                    .unwrap_err(),
                Codex::new(on_path(bin, "codex"))
                    .review(&invocation(b"p", None))
                    .unwrap_err(),
            ] {
                assert_eq!(refusal.code, "reviewer-no-answer", "{}", refusal.message);
            }
        },
    );
}

#[cfg(unix)]
#[test]
fn a_spawn_error_other_than_not_found_is_reviewer_launch_failed() {
    let dir = tempfile::tempdir().unwrap();
    let refusal = ClaudeCode::new(dir.path().to_path_buf())
        .review(&invocation(b"p", None))
        .unwrap_err();
    assert_eq!(
        refusal.code, "reviewer-launch-failed",
        "{}",
        refusal.message
    );
}

#[test]
fn a_prompt_the_cli_never_read_is_reviewer_launch_failed_on_a_zero_exit() {
    let prompt = vec![b'x'; 2 * 1024 * 1024];
    with_fake_clis(
        &["claude"],
        &[
            ("ESTIGIA_FAKE_EXIT_WITHOUT_READING", OsStr::new("1")),
            (
                "ESTIGIA_FAKE_ANSWERS",
                OsStr::new(r#"[{"stdout_exact":"looks fine"}]"#),
            ),
        ],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(&prompt, None))
                .unwrap_err();
            assert_eq!(
                refusal.code, "reviewer-launch-failed",
                "{}",
                refusal.message
            );
            assert!(refusal.message.contains("prompt"));
        },
    );
}

#[test]
fn the_unavailable_matcher_reads_stderr_and_only_a_short_stdout_beside_an_empty_one() {
    let long_review = format!("login {}", "the review text ".repeat(100));
    let answers = serde_json::json!([
        { "stdout_exact": long_review, "stderr": "panic: unrelated", "status": 2 }
    ])
    .to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&answers))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(
                refusal.code, "reviewer-exited-nonzero",
                "{}",
                refusal.message
            );
        },
    );
    let long_only = serde_json::json!([{ "stdout_exact": long_review, "status": 2 }]).to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&long_only))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(
                refusal.code, "reviewer-exited-nonzero",
                "{}",
                refusal.message
            );
        },
    );
    let short_only =
        serde_json::json!([{ "stdout_exact": "Invalid API key", "status": 1 }]).to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&short_only))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(refusal.code, "reviewer-unavailable", "{}", refusal.message);
        },
    );
}

#[test]
fn the_child_sees_only_the_allow_listed_environment() {
    let dir = tempfile::tempdir().unwrap();
    let names = dir.path().join("names");
    with_fake_clis(
        &["claude"],
        &[
            ("ESTIGIA_FAKE_ENV_NAMES", names.as_os_str()),
            (
                "ESTIGIA_FAKE_ANSWERS",
                OsStr::new(
                    r#"[{"stdout_exact":"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}"}]"#,
                ),
            ),
            ("GH_TOKEN", OsStr::new("ghp_secret")),
            ("OPENAI_API_KEY", OsStr::new("sk-kept")),
        ],
        |bin| {
            ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap();
            let seen = std::fs::read_to_string(&names).unwrap();
            let seen: Vec<&str> = seen.lines().collect();
            assert!(!seen.contains(&"GH_TOKEN"), "{seen:?}");
            assert!(seen.contains(&"OPENAI_API_KEY"), "{seen:?}");
            assert!(seen.contains(&"PATH"), "{seen:?}");
        },
    );
}

#[cfg(unix)]
#[test]
fn a_scratch_directory_is_private_to_its_owner() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new().unwrap();
    let mode = std::fs::metadata(scratch.path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

fn failing_codex_like(prompt: &str, tail: &str) -> String {
    let stderr = format!("OpenAI Codex v0\nuser\n{prompt}\nwarning: noise\n{tail}\n");
    serde_json::json!([{ "stderr": stderr, "status": 1 }]).to_string()
}

fn refusal_code_for(answers: &str, prompt: &str) -> (&'static str, String) {
    let mut seen = None;
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(answers))],
        |bin| {
            let refusal = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(prompt.as_bytes(), None))
                .unwrap_err();
            seen = Some((refusal.code, refusal.message));
        },
    );
    seen.unwrap()
}

#[test]
fn an_echoed_prompt_on_stderr_is_not_read_as_unavailable() {
    let prompt = "please review the login and api key handling";
    let (code, _) = refusal_code_for(
        &failing_codex_like(prompt, "ERROR: model not found"),
        prompt,
    );
    assert_eq!(code, "reviewer-exited-nonzero");
    let (code, _) = refusal_code_for(
        &failing_codex_like(prompt, "ERROR: unexpected status 401 Unauthorized"),
        prompt,
    );
    assert_eq!(code, "reviewer-unavailable");
}

#[test]
fn a_prompt_echo_cut_off_by_the_stderr_tail_is_still_removed() {
    let prompt = "login api key ".repeat(run::MAX_STDERR / 14 + 100);
    let (code, _) = refusal_code_for(
        &failing_codex_like(&prompt, "ERROR: model not found"),
        &prompt,
    );
    assert_eq!(code, "reviewer-exited-nonzero");
}

#[test]
fn only_the_tail_of_stderr_decides_unavailable() {
    let stderr = format!(
        "quota exceeded early\n{}\npanic: crash",
        "filler line\n".repeat(400)
    );
    let answers = serde_json::json!([{ "stderr": stderr, "status": 1 }]).to_string();
    let (code, _) = refusal_code_for(&answers, "p");
    assert_eq!(code, "reviewer-exited-nonzero");
}

#[test]
fn a_refusal_shows_the_end_of_stderr_not_its_head() {
    let stderr = format!(
        "banner\n{}ERROR: the real cause\n",
        "progress line\n".repeat(100)
    );
    let answers = serde_json::json!([{ "stderr": stderr, "status": 1 }]).to_string();
    let (_, message) = refusal_code_for(&answers, "p");
    assert!(message.contains("the real cause"), "{message}");
}

#[test]
fn the_kept_stderr_is_its_last_bytes() {
    let stderr = format!(
        "{}ERROR: the real cause\n",
        "x".repeat(run::MAX_STDERR + 4096)
    );
    let answers = serde_json::json!([{ "stderr": stderr, "status": 1 }]).to_string();
    let (_, message) = refusal_code_for(&answers, "p");
    assert!(message.contains("the real cause"), "{message}");
}

#[test]
fn a_whitespace_only_answer_is_reviewer_no_answer() {
    with_fake_clis(
        &["claude", "codex"],
        &[("ESTIGIA_FAKE_WRITE_ARG_FILE", OsStr::new(" \n\t\n"))],
        |bin| {
            let refusal = Codex::new(on_path(bin, "codex"))
                .review(&invocation(b"p", None))
                .unwrap_err();
            assert_eq!(refusal.code, "reviewer-no-answer", "{}", refusal.message);
        },
    );
    let answers = serde_json::json!([{
        "stdout_exact": "{\"type\":\"result\",\"is_error\":false,\"result\":\"  \\n\"}"
    }])
    .to_string();
    let (code, _) = refusal_code_for(&answers, "p");
    assert_eq!(code, "reviewer-no-answer");
}

fn result_object(is_error: bool, text: &str) -> String {
    let stdout = serde_json::json!({
        "type": "result", "subtype": "x", "is_error": is_error, "result": text
    })
    .to_string();
    serde_json::json!([{ "stdout_exact": stdout }]).to_string()
}

#[test]
fn an_error_result_on_a_zero_exit_is_never_the_answer() {
    let (code, message) = refusal_code_for(&result_object(true, "API Error: 429 rate limit"), "p");
    assert_eq!(code, "reviewer-unavailable");
    assert!(message.contains("429"));
    let (code, _) = refusal_code_for(&result_object(true, "something else broke"), "p");
    assert_eq!(code, "reviewer-error-result");
    let prompt = "discuss the login and api key rules at length";
    let (code, _) = refusal_code_for(&result_object(true, &format!("{prompt} crashed")), prompt);
    assert_eq!(code, "reviewer-error-result");
}

#[test]
fn a_result_object_gives_its_result_and_other_stdout_is_launch_failed() {
    with_fake_clis(
        &["claude"],
        &[(
            "ESTIGIA_FAKE_ANSWERS",
            OsStr::new(&result_object(false, "verdict")),
        )],
        |bin| {
            let answer = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap();
            assert_eq!(answer, b"verdict");
        },
    );
    for stdout in [
        "not json at all",
        r#"{"type":"assistant","is_error":false,"result":"x"}"#,
    ] {
        let answers = serde_json::json!([{ "stdout_exact": stdout }]).to_string();
        let (code, message) = refusal_code_for(&answers, "p");
        assert_eq!(code, "reviewer-launch-failed");
        assert!(message.contains("JSON result object"));
    }
}

#[test]
fn a_non_zero_exit_with_an_error_result_on_stdout_is_classified_by_its_text() {
    let long = format!("API Error: usage limit reached {}", "pad ".repeat(300));
    let stdout = serde_json::json!({"type":"result","is_error":true,"result":long}).to_string();
    let answers = serde_json::json!([{ "stdout_exact": stdout, "status": 1 }]).to_string();
    let (code, _) = refusal_code_for(&answers, "p");
    assert_eq!(code, "reviewer-unavailable");
}

#[test]
fn a_prompt_echo_cut_inside_a_multibyte_character_is_still_removed() {
    let prompt = "— login api key ".repeat(run::MAX_STDERR / 10);
    let tail = "\nERROR: model not found\n";
    let banner = "OpenAI Codex v0\nuser\n";
    let mut padding = String::new();
    let stderr = loop {
        let candidate = format!("{banner}{prompt}{padding}{tail}");
        let cut = candidate.len() - run::MAX_STDERR;
        if candidate.as_bytes()[cut] & 0xC0 == 0x80 {
            break candidate;
        }
        padding.push('\n');
    };
    let cut = stderr.len() - run::MAX_STDERR;
    assert!(!stderr.is_char_boundary(cut));
    let answers = serde_json::json!([{ "stderr": stderr, "status": 1 }]).to_string();
    let (code, _) = refusal_code_for(&answers, &prompt);
    assert_eq!(code, "reviewer-exited-nonzero");
}

#[test]
fn a_prompt_too_short_to_anchor_removes_nothing() {
    let answers =
        serde_json::json!([{ "stderr": "panic: login required", "status": 1 }]).to_string();
    let (code, message) = refusal_code_for(&answers, "p");
    assert_eq!(code, "reviewer-unavailable");
    assert!(message.contains("panic"), "{message}");
}

#[test]
fn codex_classifies_an_echoed_prompt_the_same_way() {
    let prompt = "please review the login and api key handling";
    for (tail, expected) in [
        ("ERROR: model not found", "reviewer-exited-nonzero"),
        (
            "ERROR: unexpected status 401 Unauthorized",
            "reviewer-unavailable",
        ),
    ] {
        let answers = failing_codex_like(prompt, tail);
        with_fake_clis(
            &["codex"],
            &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&answers))],
            |bin| {
                let refusal = Codex::new(on_path(bin, "codex"))
                    .review(&invocation(prompt.as_bytes(), None))
                    .unwrap_err();
                assert_eq!(refusal.code, expected, "{tail}");
            },
        );
    }
}

#[test]
fn removing_a_prompt_from_a_four_mebibyte_text_with_an_adversarial_prompt_completes() {
    let text = vec![b'a'; 4 * 1024 * 1024];
    let mut prompt = vec![b'a'; 255];
    prompt.push(b'b');
    let kept = adapters::without_prompt(&text, &prompt);
    assert_eq!(kept.len(), run::MAX_STDERR);
}

#[test]
fn stderr_is_kept_by_its_last_bytes_and_stdout_by_its_first() {
    use std::io::Cursor;
    let mut data = vec![b'a'; 200 * 1024];
    data.extend_from_slice(b"END");
    let (sender, receiver) = std::sync::mpsc::channel();
    run::drain(
        run::Stream::Stderr,
        Cursor::new(data.clone()),
        run::MAX_STDERR,
        None,
        sender.clone(),
    );
    let (_, kept) = receiver.recv().unwrap();
    let kept = kept.unwrap();
    assert_eq!(kept.len(), run::MAX_STDERR);
    assert!(kept.ends_with(b"END"));
    let overflow = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    run::drain(
        run::Stream::Stdout,
        Cursor::new(data),
        1000,
        Some(overflow.clone()),
        sender,
    );
    let (_, kept) = receiver.recv().unwrap();
    assert_eq!(kept.unwrap(), vec![b'a'; 1000]);
    assert!(overflow.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn a_result_object_missing_what_it_needs_is_launch_failed() {
    for stdout in [
        r#"{"type":"result","result":"x"}"#,
        r#"{"type":"result","is_error":"no","result":"x"}"#,
        r#"{"type":"result","is_error":false}"#,
    ] {
        let answers = serde_json::json!([{ "stdout_exact": stdout }]).to_string();
        let (code, message) = refusal_code_for(&answers, "p");
        assert_eq!(code, "reviewer-launch-failed", "{stdout}");
        assert!(message.contains("\"type\""), "{message}");
    }
}

#[test]
fn a_result_object_followed_by_a_newline_is_read() {
    let stdout = "{\"type\":\"result\",\"is_error\":false,\"result\":\"verdict\"}\n";
    let answers = serde_json::json!([{ "stdout_exact": stdout }]).to_string();
    with_fake_clis(
        &["claude"],
        &[("ESTIGIA_FAKE_ANSWERS", OsStr::new(&answers))],
        |bin| {
            let answer = ClaudeCode::new(on_path(bin, "claude"))
                .review(&invocation(b"p", None))
                .unwrap();
            assert_eq!(answer, b"verdict");
        },
    );
}

#[test]
fn an_error_result_names_its_subtype_and_its_errors() {
    let stdout = serde_json::json!({
        "type": "result", "subtype": "error_during_execution", "is_error": true,
        "result": "", "errors": ["first problem", { "message": "second problem" }]
    })
    .to_string();
    let answers = serde_json::json!([{ "stdout_exact": stdout }]).to_string();
    let (code, message) = refusal_code_for(&answers, "p");
    assert_eq!(code, "reviewer-error-result");
    assert!(message.contains("error_during_execution"), "{message}");
    assert!(
        message.contains("first problem second problem"),
        "{message}"
    );
}

#[test]
fn the_reviewer_child_keeps_the_user_names_the_keychain_lookup_needs() {
    with_fake_clis(
        &[],
        &[
            ("USER", OsStr::new("someone")),
            ("LOGNAME", OsStr::new("someone-else")),
        ],
        |_| {
            let kept = run::child_environment();
            let value = |name: &str| {
                kept.iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.clone())
            };
            assert_eq!(value("USER"), Some("someone".into()));
            assert_eq!(value("LOGNAME"), Some("someone-else".into()));
        },
    );
}

#[test]
fn the_allow_list_matches_names_the_way_windows_spells_them() {
    let ambient = [
        "Path",
        "SystemRoot",
        "PathExt",
        "ComSpec",
        "OPENAI_API_KEY",
        "GH_TOKEN",
    ]
    .map(|name| (OsString::from(name), OsString::from("v")));
    let kept: Vec<String> = run::inherited_from(ambient)
        .into_iter()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        kept,
        ["Path", "SystemRoot", "PathExt", "ComSpec", "OPENAI_API_KEY"],
        "{kept:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_reviewer_that_exits_has_its_group_killed_before_it_is_reaped() {
    let scratch = std::env::temp_dir();
    let args = [
        std::ffi::OsString::from("-c"),
        std::ffi::OsString::from("exit 0"),
    ];
    let finished = run::execute(&run::Launch {
        program: Path::new("sh"),
        args: &args,
        cwd: &scratch,
        stdin: b"",
        environment: &[],
        deadline: Duration::from_secs(10),
    })
    .expect("a clean exit finishes");
    assert_eq!(finished.code, Some(0));
}
