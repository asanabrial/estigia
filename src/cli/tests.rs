use clap::Parser;
use std::fs;

use super::*;
use crate::config::Config;
use crate::outcome::{MutationOutcome, NoCommandReason};

#[test]
fn the_screens_install_proceeds_on_a_source_build() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };

    let (receipt, _report) = super::guided_install(&screen_plan(), &options, false)
        .expect("the screen refused a source build the person at it had confirmed");

    assert!(
        receipt.completed.contains("claude-code"),
        "the screen accepted and the adapter was still not installed"
    );
}

/// One agent, default rows: what the two tests above are about is the gate in
/// front of the write, not the rows going through it.
fn screen_plan() -> crate::tui::Plan {
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("claude-code is an agent this build knows");
    crate::tui::Plan {
        agents: vec![adapter],
        opened: [("claude-code", Config::default())].into_iter().collect(),
        rows: [("claude-code", Config::default())].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    }
}

fn inventory() -> Vec<Refusal> {
    let mut refusals = vec![
        setup::no_agent_named(),
        setup::retired_agent(&setup::RETIRED_AGENTS[0]),
        setup::no_terminal(),
        super::workspace_without_scope("setup"),
        super::workspace_without_a_path("setup"),
        super::companion_with_a_workspace("setup"),
        crate::tui::repository_rows_unsaved(),
        crate::harness::person_only::refusal(&crate::harness::Action::Write {
            target: crate::harness::person_only::COMMANDS[0].target.to_owned(),
        })
        .expect("a person-only target is refused"),
        crate::harness::person_only::refusal(&crate::harness::Action::Write {
            target: crate::harness::person_only::COMMANDS[1].target.to_owned(),
        })
        .expect("a person-only target is refused"),
        crate::harness::issue_requirement::refusal(
            false,
            crate::config::IssueRequirement::Required,
            &crate::harness::issue_requirement::Scope::Repository(std::path::PathBuf::from(
                "/a/checkout",
            )),
            crate::harness::issue_requirement::Reach::Placed,
        ),
        crate::harness::unclaimed::not_claimed(false),
        crate::harness::decision_mode::refusal(&[], true),
        crate::harness::issue_creation::refusal(Some("gh issue create"))
            .expect("a line that files an issue is refused"),
        // The role gate's only refusal, by its real constructor.
        crate::harness::roles::out_of_role(
            "builder",
            "WebFetch",
            &crate::harness::roles::Policy::Allowlist(vec!["Read".to_owned(), "Write".to_owned()]),
        ),
        Refusal::not_started(
            "reviewer-project-shadow",
            "a project definition shadows the operator-owned reviewer",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "remove or rename the project reviewer, or use separate-session review",
            ),
        ),
        Refusal::not_started(
            "reviewer-project-unprovable",
            "project reviewer candidates cannot be proved unique and readable",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "repair the project agent tree, or use durable handoff review",
            ),
        ),
        Refusal::not_started(
            "reviewer-launched-by-the-binary",
            "a binary-launched reviewer has no hand path",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "call `launch_review_input` and then `collect_review_input` for the slot review status offers",
            ),
        ),
        Refusal::not_started(
            "reviewer-canonical-unavailable",
            "the canonical user reviewer cannot be proved current",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "restore it with Claude setup, or use separate-session review",
            ),
        ),
        Refusal::not_started(
            "prune-read-failed",
            "the sweep could not read the machine, so what accumulated is unknown",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a git checkout with a remote, and an authenticated `gh`",
            ),
        ),
        // The bulk import's read failure, by its real constructor.
        super::store_import_unreadable(crate::transport::Failure::Read(
            "the open issues and the local store could not be read".to_owned(),
        )),
        super::sessions::unreadable("the local store could not be read"),
        super::sessions::source_unreadable(std::path::Path::new("note.txt"), "no such file"),
        super::sessions::cursor_too_large(crate::cli::sessions::MAX_CURSOR_BYTES + 1),
        super::sessions::cursor_malformed("it carries no `.`"),
        super::sessions::cursor_tampered(),
        super::sessions::cursor_mismatch("selectors"),
        super::sessions::cursor_store_replaced(),
        super::sessions::cursor_expired(),
        super::sessions::limit_too_large(crate::cli::sessions::MAX_LIMIT + 1),
        super::sessions::selector_too_large(
            "repo_dir",
            crate::cli::sessions::MAX_SELECTOR_BYTES + 1,
        ),
        super::sessions::unregistered_not_bounded(),
        super::feed::store_unreadable("the local store could not be read"),
        super::feed::repo_unknown("o/a"),
        super::feed::since_malformed("soon"),
        super::sessions::listing_flags_beside_send(&["--all", "--limit"]),
        super::resume::run_unknown("opencode-gone"),
        super::resume::no_native("pi-ab12", "pi", "its session file is not recorded"),
        super::resume::misattributed("claude-ab12", "claude", "cursor-7"),
        super::resume::run_ambiguous("repo-1", &[]),
        crate::harness::mailbox::empty(),
        crate::harness::status::empty(),
        crate::harness::status::phase_unknown("blocked"),
        crate::harness::status::unavailable("the local store could not be opened"),
        crate::harness::mailbox::too_long(9000),
        crate::harness::mailbox::recipient_unknown("pi-ghost"),
        crate::harness::mailbox::unavailable("the local store could not be opened"),
        crate::harness::mcp::messaging::needs_a_person("claude-abcd1234"),
        crate::harness::mcp::messaging::not_allowed("claude-abcd1234", "pi-ghost"),
        crate::harness::mcp::messaging::reply_unknown(12),
        crate::harness::mcp::messaging::reply_has_no_sender(12),
        crate::harness::mcp::messaging::reply_sender_gone("pi-ghost"),
        crate::harness::mcp::messaging::question_failed("the client closed its input"),
        {
            let empty = crate::harness::mcp::address::Narrowed {
                matched: Vec::new(),
                closest: Vec::new(),
                live: 0,
            };
            crate::harness::mcp::address::no_live_session("opencode", &empty)
        },
        {
            let both = crate::harness::mcp::address::Narrowed {
                matched: Vec::new(),
                closest: Vec::new(),
                live: 2,
            };
            crate::harness::mcp::address::recipient_ambiguous("opencode", &both)
        },
        {
            let holder = crate::cli::sessions::Listed {
                run_id: "claude-holder0".to_owned(),
                name: None,
                runtime: "claude".to_owned(),
                session_id: None,
                status: "live",
                issue: Some(crate::identity::Identity::Number(12)),
                state: Some("in-progress".to_owned()),
                last_seen: Some(1),
                silent_for: Some(0),
                repo_dir: Some("/repo".to_owned()),
                worktree: None,
                ended_at: None,
                end_reason: None,
                waiting: 0,
                undelivered_sent: 0,
                build: None,
                wake: None,
                pane: None,
                handoffs: Vec::new(),
            };
            crate::harness::mcp::messaging::flow_conflict(&holder, "flow dev 13")
                .expect("a session holding #12 told to work #13 is refused")
        },
        Refusal::not_started(
            "read-failed",
            "the reviewer's answer could not be read from standard input",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a readable standard input carrying the reviewer's answer",
            ),
        ),
        // A read-only Linear tracker is admitted only under assisted
        // work, and each refusal names the way out.
        Refusal::not_started(
            "linear-is-per-repository",
            "`Tracker linear` is read-only and admitted only under `Workflow: assisted`, which a \
             repository answers in its own file",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the repository-scoped form, naming the team key",
            ),
        ),
        Refusal::not_started(
            "linear-needs-assisted",
            "`Tracker linear` is read-only and supported only under `Workflow: assisted`",
            Resolution::run("estigia config set Workflow assisted --repo"),
        ),
        Refusal::not_started(
            "autonomous-needs-github-tracker",
            "`Workflow: autonomous` and a read-only `Tracker linear` cannot both hold",
            Resolution::run("estigia config set Tracker github --repo"),
        ),
        Refusal::not_started(
            "autonomous-fixes-delivery-rows",
            "`Claim workflow` is `autonomous`, which fixes `Ready by` at the autonomous preset's value",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "set `Delivery preset` to `team` or `assisted`, or `Claim workflow` to `assisted`",
            ),
        ),
        Refusal::not_started(
            "assisted-merge-is-the-persons",
            "`Merge by` is `agent` and `Claim workflow` is `assisted`",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "set `Merge by` to `person`, or `Delivery preset` to `autonomous`",
            ),
        ),
        Refusal::not_started(
            "state-names-need-linear",
            "`State names` maps states onto a tracker whose states Estigia writes as labels",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "set `State names` to `none`, or `Tracker` to `linear <team-key>`",
            ),
        ),
        Refusal::not_started(
            "delivery-preset-custom-sets-nothing",
            "`Delivery preset` reads `custom` when the rows below it match no preset",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a preset (`autonomous`, `assisted` or `team`), or the row you want to change",
            ),
        ),
        Refusal::not_started(
            "read-only-tracker-admits-only-assisted",
            "`Tracker linear` is read-only and admits only assisted work",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "`Tracker linear` is supported only under `Workflow: assisted`",
            ),
        ),
        Refusal::not_started(
            "assist-issue-does-not-match-the-tracker",
            "`assist` was handed an issue whose shape does not match the configured tracker",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "an issue number under `Tracker github`, or a Linear key such as `ENG-123` under \
                 `Tracker linear`",
            ),
        ),
        nothing_configured(&[]),
        crate::config::Config::read_folding_retired_workers(
            "| Setting | Value here | Skill default |\n|---|---|---|\n\
             | Planning | sdd | ignored |\n",
            None,
        )
        .expect("a retired protocol still loads")
        .1
        .expect("the retirement is reported"),
        crate::config::retired_setting_report("Repair ceiling", "3")
            .expect("the retired setting is reported"),
        crate::config::retired_setting_report("Delivery route", "direct")
            .expect("the retired setting is reported"),
        crate::config::retired_setting_report("Review delegation", "ask")
            .expect("the retired setting is reported"),
        crate::config::retired_setting_report("Merge strategy", "merge commit")
            .expect("the retired setting is reported"),
        crate::config::retired_setting_report("Review protocol", "rdd")
            .expect("the retired setting is reported"),
        // Knowable from the command line: a value the table cannot carry.
        super::config_set(
            "Irreversible commands",
            "make deploy | tee log",
            None,
            false,
            false,
            &SetupOptions::default(),
            false,
        )
        .unwrap_err(),
        super::config_set(
            "Model routing",
            "unset",
            None,
            false,
            true,
            &SetupOptions::default(),
            false,
        )
        .unwrap_err(),
        super::agent_row_needs_an_agent(crate::config::Setting::Models, &sandbox().1),
        crate::outcome::Refusal::not_started(
            "setting-not-for-this-agent",
            "Loop plugin has no meaning for qwen: it reads no plugin Estigia writes",
            crate::outcome::Resolution::run("estigia config edit --agent opencode"),
        ),
        {
            let root = tempfile::tempdir().expect("a temporary root");
            crate::setup::rewrite_configuration(&root.path().join("SKILL.md"), &Config::default())
                .expect_err("a contract that is not installed is not a contract")
        },
        {
            let root = tempfile::tempdir().expect("a temporary root");
            let contract = root.path().join("SKILL.md");
            std::fs::create_dir_all(&contract).expect("something unreadable in its place");
            crate::setup::rewrite_configuration(&contract, &Config::default())
                .expect_err("a contract nothing can read is not a contract")
        },
        {
            let root = tempfile::tempdir().expect("a temporary root");
            std::fs::create_dir_all(root.path().join(crate::config::LOCAL_FILE))
                .expect("something unreadable in its place");
            crate::skill::local_overrides(root.path())
                .expect_err("a directory in the override's place is not an override")
        },
        {
            let root = tempfile::tempdir().expect("a temporary checkout");
            let git = |arguments: &[&str]| {
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(root.path())
                    .args(arguments)
                    .output()
                    .is_ok_and(|output| output.status.success())
            };
            assert!(
                git(&["init", "--quiet"]),
                "the fixture checkout initialises"
            );
            assert!(
                git(&[
                    "-c",
                    "user.email=nobody@example.invalid",
                    "-c",
                    "user.name=nobody",
                    "commit",
                    "--allow-empty",
                    "--quiet",
                    "-m",
                    "one",
                ]),
                "the fixture checkout has a commit"
            );
            let mut run = crate::harness::session::Run::new("claude-ratchet2".to_owned());
            run.worktree = Some(root.path().to_path_buf());
            crate::harness::out_of_phase(
                "git merge",
                "analysis",
                &crate::identity::Identity::Number(12),
                crate::config::Integration::Branch,
                None,
                &run,
                root.path(),
            )
            .expect("an integration outside in-progress is refused")
        },
        Refusal::not_started(
            "integration-out-of-phase",
            "this integrates the base into this run's own checkout from a state where no \
             integration is allowed",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the transition of this issue back to in-progress, then the integration again",
            ),
        ),
        Refusal::not_started(
            "integration-shape-not-exact",
            "this is a `git merge` in this run's own checkout, but its shape is not the one the \
             gate proves as a base integration",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the integration again as that one bare command",
            ),
        ),
        Refusal::not_started(
            "run-id-names-another-checkout",
            "claude-other holds a claim over /elsewhere, and this server is running in /here",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "this run's own id, the one `SessionStart` reported",
            ),
        ),
        // Built by hand rather than by calling `release`, which would need a
        // state directory: the point here is the ratchet, not the path.
        Refusal::not_started(
            "run-pointer-unreadable",
            "r1 wrote a pointer Estigia can no longer read".to_owned(),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "what that run holds, read from the tracker",
            ),
        ),
        crate::harness::delivery_pr_unidentified(None),
        crate::harness::delivery_pr_repository_option(),
        crate::harness::complete_review_transaction_missing(54),
        crate::harness::merge_without_ci_release(54),
        crate::harness::delivery_pr_mismatch(54, 55),
        crate::harness::complete_review_transaction_not_selected(),
        crate::harness::several_irreversible_fragments(&[
            "git merge".to_owned(),
            "git tag".to_owned(),
        ]),
        // Raised before anything on disk is read, so it is built the way the
        // command builds it: a repository-wide row asked for one agent.
        super::config_set(
            "Tracker",
            "github",
            Some("opencode"),
            false,
            false,
            &SetupOptions::default(),
            false,
        )
        .unwrap_err(),
        find_agent("emacs").unwrap_err(),
        crate::tui::words::remember(
            Some(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")),
            crate::tui::words::Tongue::Spanish,
        )
        .unwrap_err(),
        skill::no_skill_root(),
        // The real constructor, not a copy of it: a hand-written twin here would
        // pass the ratchet while the refusal the gate actually raises drifted.
        crate::harness::out_of_phase(
            "gh pr merge",
            "in-progress",
            &crate::identity::Identity::Number(12),
            crate::config::Integration::Branch,
            None,
            &crate::harness::session::Run::new("claude-ratchet0".to_owned()),
            std::path::Path::new("/repo"),
        )
        .expect("a delivery from in-progress is refused"),
        // Every way a stand-down can fail to be one, by the real mapping — a
        // hand-written twin here would pass while the refusals drifted.
        Refusal::not_started(
            crate::harness::standdown::Rejected::NoReason.code(),
            format!(
                "this is not a stand-down: {}",
                crate::harness::standdown::Rejected::NoReason.action()
            ),
            Resolution::run("estigia stand-down --reason \"...\" --minutes 30"),
        ),
        Refusal::not_started(
            crate::harness::standdown::Rejected::NoTime.code(),
            format!(
                "this is not a stand-down: {}",
                crate::harness::standdown::Rejected::NoTime.action()
            ),
            Resolution::run("estigia stand-down --reason \"...\" --minutes 30"),
        ),
        Refusal::not_started(
            crate::harness::standdown::Rejected::TooLong.code(),
            format!(
                "this is not a stand-down: {}",
                crate::harness::standdown::Rejected::TooLong.action()
            ),
            Resolution::run("estigia stand-down --reason \"...\" --minutes 30"),
        ),
        Refusal::not_started(
            "stand-down-unwritable",
            "could not write the stand-down record",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "write access to that file",
            ),
        ),
        Refusal::not_started(
            "stand-down-unserialisable",
            "the record did not serialise",
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a build that can write its own record",
            ),
        ),
        // The screen's only refusal, by its real constructor too.
        crate::tui::io_refusal(
            "read a key",
            &std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the terminal went away"),
        ),
        Refusal::not_started(
            "agent-configuration-unwritable",
            "could not write this adapter's own configuration file",
            Resolution::run("estigia doctor"),
        ),
        // The same refusal one scope over: a repository that says it answers
        // for itself, in a file that will not take the answer.
        Refusal::not_started(
            "repository-configuration-unwritable",
            "could not write this repository's own configuration file",
            Resolution::run("estigia doctor"),
        ),
        // The list of repositories lives under home, so a machine that cannot
        // say where home is cannot be asked for it.
        Refusal::not_started(
            "home-unknown",
            "could not determine the user home directory",
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a home directory this process can read",
            ),
        ),
        // And its read: a file that exists and will not parse is a stop, or
        // writing over it throws away an answer somebody gave.
        Refusal::not_started(
            "not-a-repository",
            "this folder is not a git checkout",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "run it inside a checkout, or set the row without `--repo`",
            ),
        ),
        Refusal::not_started(
            "repository-configuration-unreadable",
            "this repository's own configuration file will not parse",
            Resolution::run("estigia config list"),
        ),
        // The one row a repository may not answer for.
        Refusal::not_started(
            "setting-not-the-repositorys",
            "that is what one agent does, not what this repository is",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "set it with `--agent <agent>` instead, naming the setting the same way",
            ),
        ),
        Refusal::not_started(
            "setting-only-the-repositorys",
            "that row is answered by a repository's own file and has no machine-wide value",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the same `estigia config set` with `--repo`, run inside the checkout that should answer for it",
            ),
        ),
        Refusal::not_started(
            "repository-cannot-loosen",
            "a repository may only tighten an authorisation, and this would loosen one",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "that row changed first \u{2014} `estigia config set \"Delivery authorisation\" auto --agent <agent>`",
            ),
        ),
        // The trunk-based refusal, by its real constructor: a delivery landing
        // early with nothing named to switch it off.
        crate::harness::out_of_phase(
            "gh pr merge",
            "in-progress",
            &crate::identity::Identity::Number(12),
            crate::config::Integration::Trunk,
            None,
            &crate::harness::session::Run::new("claude-ratchet1".to_owned()),
            std::path::Path::new("/repo"),
        )
        .expect("an unflagged delivery on trunk is refused"),
        Refusal::not_started(
            "companion-unknown",
            "\"sardi\" is not a companion Estigia knows",
            Resolution::run(format!(
                "estigia setup --companion {}",
                COMPANIONS
                    .iter()
                    .map(|companion| companion.slug)
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ),
        Refusal::not_started(
            "contract-is-shared",
            "Profile is answered per agent, and zcode reads the contract on the shared skill root",
            Resolution::run("estigia config set Profile balanced --agent zcode"),
        ),
        Refusal::not_started(
            "setting-unknown",
            "\"Merge\" is not a setting",
            Resolution::run("estigia config list"),
        ),
        crate::harness::issue_not_a_number("twelve"),
        Refusal::not_started(
            "harness-not-installed",
            "no installed skill carries the tracker transport",
            Resolution::run("estigia setup --all"),
        ),
        Refusal::not_started(
            "nothing-held",
            "claude-abcd1234 holds no issue",
            Resolution::run("estigia status"),
        ),
        Refusal::not_started(
            "release-repository-mismatch",
            "claude-abcd1234 recorded its claim against a different repository than the one \
             this release was run against, so a successful read here says nothing about that \
             pointer's own timeline \u{2014} its local record is left exactly as it was",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "run `estigia release --run-id <run-id>` from the checkout that pointer's own \
                 claim covers, if it still exists \u{2014} and if it does not, its pointer is \
                 Estigia's own local state and no claim of anybody's, removable by hand",
            ),
        ),
        Refusal::not_started(
            "hook-event-unknown",
            "\"on-tuesday\" is not a lifecycle event",
            Resolution::run("estigia setup --all"),
        ),
        Refusal::not_started(
            "gate-identity-over-specified",
            "--run-id and --session-id both name this call's run",
            Resolution::run("estigia gate <tool> --session-id <session-id> --runtime <runtime>"),
        ),
        Refusal::not_started(
            "gate-identity-incomplete",
            "--session-id was given without the --runtime that derives its run id",
            Resolution::run("estigia gate <tool> --session-id <session-id> --runtime <runtime>"),
        ),
        Refusal::not_started(
            "gate-input-not-json",
            "--input is not JSON",
            Resolution::run(
                r#"estigia gate Edit --run-id <run-id> --input '{"file_path":"src/x.rs"}'"#,
            ),
        ),
        // it answered *scripts/github.py is not installed* and sent an operator
        // to run `estigia setup --all` — the command that had just run, and that
        // deliberately does not install it.
        Refusal::not_started(
            "control-surface-not-installed",
            "the contract is not installed",
            Resolution::run("estigia setup --all"),
        ),
        // A clock that will not answer, which the gate asks before it asks the
        // tracker: a claim is only good until a time, so a run that cannot say
        // when now is cannot say whether it still holds anything.
        Refusal::not_started(
            "clock-unreadable",
            "the system clock could not be read",
            Resolution::no_command(NoCommandReason::WorldAction, "a working system clock"),
        ),
        // The schedule's own denial, by its real constructor: a commit outside
        // the repository's permitted period names the range and the way out.
        crate::harness::operation_window::denial(
            &crate::config::GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"),
            Some(20 * 60),
        ),
        // The mirror of `push-hook-belongs-to-somebody-else`: there the hook was
        // theirs and Estigia was asked to chain from it; here the hook is
        // Estigia's and somebody added their check to it. Both refuse rather
        // than rewrite, because the work is the same work.
        Refusal::not_started(
            "push-hook-carries-your-lines",
            "the push hook has lines Estigia did not write",
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "a decision about your additions",
            ),
        ),
        Refusal::not_started(
            "home-not-resolvable",
            "no home directory",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a HOME or USERPROFILE the process can read",
            ),
        ),
        Refusal::not_started(
            "run-pointer-not-writable",
            "the state directory is not writable",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "write permission on the Estigia state directory",
            ),
        ),
        // which pointer it wanted, and the push guard is reading all of them at
        // once to ask whether *any* claim covers this checkout. Both refuse
        // rather than read an unreadable file as one holding nothing.
        Refusal::not_started(
            "run-pointers-unreadable",
            "1 run pointer(s) on this machine cannot be read",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "those files readable, or taken away if the runs that wrote them are over",
            ),
        ),
        Refusal::not_started(
            "agent-file-not-editable",
            "the agent's file is not a JSON object",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a JSON object in that file, or the file moved aside",
            ),
        ),
        Refusal::not_started(
            "setup-prevalidation-failed",
            "setup could not read an input before writing",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the named input readable and valid before retrying",
            ),
        ),
        Refusal::not_started(
            "run-id-names-no-run",
            "that run id is the name every session without an identity is given",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a session Estigia can derive a run id from",
            ),
        ),
        crate::harness::mcp::runtime_names_no_runtime("Not A Runtime"),
        Refusal::not_started(
            "executable-path-not-quotable",
            "the path this binary is at holds a character a shell reads out of it",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "Estigia installed somewhere without `$`, a backtick, `\"` or `%` in the path",
            ),
        ),
        Refusal::not_started(
            "not-a-repository",
            "this directory is not a git repository",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a git repository to install the push guard into",
            ),
        ),
        Refusal::not_started(
            "push-hook-belongs-to-somebody-else",
            "a pre-push hook exists and Estigia did not write it",
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "a decision about the existing hook: chain Estigia from it, or move it aside",
            ),
        ),
        Refusal::not_started(
            "verdict-bound-to-other-bytes",
            "the review was published against one head and this checkout is at another",
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                concat!(
                    "a review of the bytes being delivered: publish the target again and ask ",
                    "for a verdict on the new head, or reset this checkout to the head that ",
                    "was reviewed"
                ),
            ),
        ),
        Refusal::unavailable(
            "flush-detach-failed",
            "the flush could not be started in the background",
            Resolution::run("estigia flush"),
        ),
        Refusal::unavailable(
            "review-head-unreadable",
            "the review was published against one head and this checkout's head cannot be read",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a checkout whose head `git rev-parse HEAD` can read; then repeat the command",
            ),
        ),
        Refusal::unavailable(
            "review-checkout-unreadable",
            "this command changes directory in a way that cannot be read",
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "the directory spelled as a plain path, or the command run from inside the checkout",
            ),
        ),
        Refusal::not_started(
            "verdict-bound-to-other-repository",
            "the checkout is not a checkout of the repository the review was published in",
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "a review of the bytes being delivered: publish the target again and ask for a verdict on the new head",
            ),
        ),
        Refusal::not_started(
            "stand-down-not-lifted",
            "the stand-down record is still on disk, so the gate is still standing down",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "whatever is holding that file released, or the file removed by hand",
            ),
        ),
        Refusal::not_started(
            "install-record-unreadable",
            "the record of what an install created is there and cannot be read",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "that record deleted, which also gives up the list of files it held",
            ),
        ),
        Refusal::not_started(
            "agent-definition-unreadable",
            "a sub-agent's definition is there and cannot be read",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "that definition readable, or moved aside",
            ),
        ),
        Refusal::not_started(
            "push-hook-unreadable",
            "a pre-push hook is there and cannot be read",
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "a decision about the existing hook: make it readable, or move it aside",
            ),
        ),
        Refusal::not_started(
            "push-hook-not-writable",
            "the hooks directory is not writable",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "write permission on the repository's hooks directory",
            ),
        ),
        Refusal::not_started(
            "several-runs-hold-this-checkout",
            "2 runs on this machine hold this checkout",
            // The shape when at least one survivor is `not-current-live-holder` —
            // the only standing this refusal may ever offer `estigia release`
            // for. Two genuinely live claims name no command at all instead
            // (`Resolution::no_command`), which is why this entry is not the
            // whole population: a several-runs refusal is one code with two
            // resolution shapes, and only the runnable one belongs in this list.
            Resolution::run("estigia release --run-id <run-id>   # for: claude-abcd1234"),
        ),
        Refusal::not_started(
            "executable-not-resolvable",
            "the Estigia executable path could not be resolved",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a resolvable path to the Estigia executable",
            ),
        ),
        Refusal::not_started(
            "environment-not-ready",
            "python is not usable, so a run cannot swear yet",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "python: the interpreter the GitHub binding is written for",
            ),
        ),
        Refusal::not_started(
            "tracker-has-no-transport",
            "`linear` has a binding the agent reads and no executable",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "either a tracker with a scripted binding, or the operations run by hand",
            ),
        ),
        Refusal::not_started(
            "repository-host-has-no-transport",
            "`none` is the repository host and has no executable, so `publish_review` cannot run against it",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a repository host with a scripted binding — `estigia config set \"Repository host\" github`",
            ),
        ),
        Refusal::not_started(
            "workflow-assisted-writes-no-tracker",
            "`claim` writes the tracker, and the `Workflow` row is `assisted` in this repository's own file",
            Resolution::run("estigia config set Workflow autonomous --repo"),
        ),
        Refusal::not_started(
            "already-holding",
            "claude-abcd1234 already holds issue #12 and cannot also swear to #34",
            Resolution::run("estigia release --run-id claude-abcd1234"),
        ),
        Refusal::not_started(
            "tool-arguments-invalid",
            "claim needs `run_id`",
            Resolution::run("estigia claim --help"),
        ),
        Refusal::not_started(
            "mcp-stream-failed",
            "the MCP stream ended badly",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a client that keeps standard input and output open",
            ),
        ),
        Refusal::not_started(
            "working-directory-unknown",
            "the working directory could not be read",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a working directory the process can read",
            ),
        ),
        Refusal::not_started(
            "source-build-not-allowed",
            "this build has no matching observed-path installer record",
            Resolution::run("estigia setup --all --allow-source-build"),
        ),
        Refusal::not_started(
            "recorded-downgrade-blocked",
            "the running installer-recorded release is below machine high-water",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "an installer-recorded Estigia release at or above the recorded high-water",
            ),
        ),
        Refusal::not_started(
            "installer-downgrade-blocked",
            "the extracted installer candidate is below machine high-water",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "an official installer candidate at or above the recorded high-water",
            ),
        ),
        Refusal::not_started(
            "lifecycle-state-unreadable",
            "the lifecycle evidence is unreadable",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the lifecycle evidence readable and valid",
            ),
        ),
        Refusal::not_started(
            "goal-identity-missing",
            "no run was named: a goal is asked about one run",
            Resolution::run("estigia goal --session-id <session-id> --runtime <runtime>"),
        ),
        Refusal::not_started(
            "goal-activity-body-unwritable",
            "the activity text could not be staged",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "write access to the temporary directory the note is staged in",
            ),
        ),
        Refusal::not_started(
            "stop-after-not-writable",
            "the stop-after request could not be written",
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "write permission on the Estigia state directory",
            ),
        ),
        Refusal::not_started(
            "loop-not-writable",
            "the loop record could not be written",
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "write permission on the Estigia state directory",
            ),
        ),
    ];

    // Configuration defects, raised by reading a table.
    let bad = |label: &str, value: &str| {
        let table = format!(
            "| Setting | Value here | Skill default |\n|---|---|---|\n| {label} | {value} | x |\n"
        );
        Config::read(&table, None).expect_err("this table is invalid")
    };
    refusals.push(bad("Integration", "fast-forward"));
    refusals.push(bad("Evidence standard", "sometime"));
    refusals.push(bad("Worktree location", "../relative"));
    refusals.push(bad("Tracker", "github not-a-repo"));
    refusals.push(bad("Task body language", ""));

    // Companion states.
    for companion in COMPANIONS {
        for state in [
            crate::setup::CompanionState::Absent,
            crate::setup::CompanionState::Unpublished,
            crate::setup::CompanionState::Present {
                version: "leteo 0.1.0".into(),
            },
        ] {
            refusals.push(Refusal::not_started(
                "companion-state",
                "probed",
                state.resolution(companion),
            ));
        }
    }

    // Every deliberately unexposed operation, by the real constructor: the
    // reason the runtime delivers is the one NOT_EXPOSED writes.
    for (name, reason) in crate::harness::mcp::NOT_EXPOSED {
        refusals.push(crate::harness::mcp::operation_has_no_tool(name, reason));
    }

    for code in [
        "workflow-not-assisted",
        "assisted-claim-not-in-force",
        "assisted-claim-stale",
        "assisted-worktree-not-started",
        "assisted-outside-the-worktree",
        "assisted-write-unplaced",
        "assisted-does-not-deliver",
        "assisted-publishes-through-draft-pr",
        "assisted-push-unconfined",
        "assisted-api-mutation-unverified",
    ] {
        refusals.push(Refusal::not_started(
            code,
            "this assisted run's claim does not admit that",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the work done the way the assisted claim admits, or by a person",
            ),
        ));
    }
    refusals.push(Refusal::not_started(
        "draft-pr-run-not-assisted",
        "claude-abcd1234 has no local claim from `assist` naming a branch and a checkout",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`assist` called first, under this run's own id, for the issue the person named",
        ),
    ));

    refusals
}

/// The commands the parser must accept, extracted from the resolutions.
fn commands_named() -> Vec<String> {
    inventory()
        .into_iter()
        .filter_map(|refusal| match refusal.resolution {
            Resolution::Run { command } => Some(command),
            Resolution::NoCommand { .. } => None,
        })
        .collect()
}

/// Splits a suggestion into argv, dropping a trailing `# comment`.
fn argv(command: &str) -> Vec<String> {
    command
        .split('#')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// one row; a screen that writes fourteen has fourteen chances to do it, so it
/// a screen reporting an answer that is not in force, in the configuration of a
/// tool whose entire purpose is refusing exactly that.
#[test]
fn the_install_screen_refuses_a_row_the_operators_own_file_overrides() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("claude-code is an agent this build knows");

    let chosen = Config {
        evidence: crate::config::Evidence::Measuring,
        ..Config::default()
    };
    let plan = |config: &Config| crate::tui::Plan {
        agents: vec![adapter],
        opened: [("claude-code", Config::default())].into_iter().collect(),
        rows: [("claude-code", config.clone())].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    };

    // The floor first: with no override file, the same screen writes the same
    // value and says so. A check that refused every install would satisfy the
    // assertion below and take the product with it.
    super::install_planned(&plan(&chosen), &options, false)
        .expect("an install with nothing shadowing it was refused");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("the adapter resolves its paths")
        .skill_root;
    assert_eq!(
        crate::skill::installed_config_for(&root, Some("claude-code"))
            .expect("the contract reads")
            .evidence,
        crate::config::Evidence::Measuring,
        "the value the screen wrote is not the one a run reads"
    );

    // And now the operator's own file, overriding that row.
    std::fs::write(
        root.join(crate::config::LOCAL_FILE),
        "| Setting | Value here | Skill default |\n|---|---|---|\n\
         | Evidence standard | reading | reading |\n",
    )
    .expect("the override is written");

    let refusal = super::install_planned(&plan(&chosen), &options, false)
        .expect_err("the screen wrote a value nobody will read and said nothing");
    assert_eq!(refusal.code, "setting-shadowed-by-local-file");
    assert!(
        refusal.message.contains("Evidence standard"),
        "the refusal does not name the row that did not take: {refusal}"
    );
    // Committed, because the table was written. Telling an operator nothing
    // happened would send them to repeat a write that already landed.
    assert_eq!(refusal.outcome, MutationOutcome::Committed);

    // The same act through the other door, so the two are held to one answer.
    let theirs = super::config_set(
        "Evidence standard",
        "measuring",
        None,
        false,
        false,
        &options,
        false,
    )
    .expect_err("`config set` stopped refusing a shadowed row");
    assert_eq!(theirs.code, refusal.code);
}

#[test]
fn every_command_a_rejection_names_parses() {
    for command in commands_named() {
        let argv = argv(&command);
        assert!(
            !argv.is_empty(),
            "a resolution named an empty command: {command:?}"
        );
        // A resolution may name another tool — `cargo install leteo` is the
        if argv[0] == "estigia" {
            for expanded in expand(&argv) {
                if let Err(error) = Cli::try_parse_from(&expanded) {
                    use clap::error::ErrorKind;
                    assert!(
                        matches!(
                            error.kind(),
                            ErrorKind::DisplayHelp
                                | ErrorKind::DisplayVersion
                                | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                        ),
                        "a rejection names `{}`, which this binary cannot parse:\n{error}",
                        expanded.join(" ")
                    );
                }
            }
        } else {
            assert!(
                on_path(&argv[0]),
                "a rejection names `{command}`, and `{}` is not on the path — naming a dead \
                 end is worse than naming nothing",
                argv[0]
            );
        }
    }
}

/// Whether an external program a resolution names can actually be run.
fn on_path(program: &str) -> bool {
    std::process::Command::new(program)
        .arg("--version")
        .output()
        .is_ok()
}

/// suggested a command with no `--run-id`, which is required, so the suggestion
/// would have failed the moment anybody pasted it.
fn expand(argv: &[String]) -> Vec<Vec<String>> {
    let Some(position) = argv.iter().position(|word| word.starts_with('<')) else {
        return vec![argv.to_vec()];
    };
    let tail = argv[position..].join(" ");
    // A choice is a single bracketed group holding commas and nothing after it.
    let bracketed = tail.trim_start_matches('<').trim_end_matches('>');
    if tail.starts_with('<') && tail.ends_with('>') && bracketed.contains(',') {
        return bracketed
            .split(',')
            .map(|choice| choice.trim().to_owned())
            .filter(|choice| !choice.is_empty())
            .map(|choice| {
                let mut expanded = argv[..position].to_vec();
                expanded.push(choice);
                expanded
            })
            .collect();
    }
    // Otherwise every `<placeholder>` becomes a value of the right shape.
    vec![
        argv.iter()
            .map(|word| {
                if word.starts_with('<') && word.ends_with('>') {
                    "placeholder".to_owned()
                } else {
                    word.clone()
                }
            })
            .collect(),
    ]
}

#[test]
fn a_rejection_that_names_no_command_says_which_kind_of_gap_it_is() {
    for refusal in inventory().into_iter().chain(after_writing()) {
        if let Resolution::NoCommand { reason, detail } = &refusal.resolution {
            assert!(
                matches!(
                    reason,
                    NoCommandReason::OperatorKnowledge
                        | NoCommandReason::WorldAction
                        | NoCommandReason::HumanAuthority
                ),
                "{} reached outside the vocabulary",
                refusal.code
            );
            assert!(
                !detail.trim().is_empty(),
                "{} says no command exists and does not say what is missing",
                refusal.code
            );
        }
    }
}

#[test]
fn every_refusal_carries_a_stable_kebab_case_code() {
    for refusal in inventory().into_iter().chain(after_writing()) {
        assert!(
            !refusal.code.is_empty()
                && refusal
                    .code
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
            "{:?} is not a stable kebab-case code",
            refusal.code
        );
    }
}

///
/// They go through the same ratchet: a named command has to parse, and a named
/// non-command has to carry one of the closed reasons.
fn after_writing() -> Vec<Refusal> {
    vec![
        crate::tui::guard_not_read_back(
            std::path::Path::new("."),
            &crate::harness::guard::State::Absent,
        ),
        crate::tui::uninstall_not_read_back("codex"),
        crate::tui::guard_removal_not_read_back(
            std::path::Path::new("."),
            &crate::harness::guard::State::Installed,
        ),
        crate::tui::guard_removal_left_alone(
            std::path::Path::new("."),
            &crate::harness::guard::State::Chained,
        ),
        Refusal {
            code: "repository-readback-missing",
            message: "the repository configuration disappeared after writing".to_owned(),
            outcome: MutationOutcome::Unknown,
            class: crate::harness::refusal_class::class_of_code("repository-readback-missing"),
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "read the repository configuration before deciding whether to retry",
            ),
            observed: None,
        },
        //
        // And it is the one whose way out changed — from `estigia status`,
        Refusal {
            code: "setup-write-failed",
            message: "the write did not land".to_owned(),
            outcome: crate::outcome::MutationOutcome::Unknown,
            class: crate::harness::refusal_class::class_of_code("setup-write-failed"),
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "write access to that file",
            ),
            observed: None,
        },
        Refusal {
            code: "write-failed",
            message: "the reviewer's prompt could not be written to standard output".to_owned(),
            outcome: crate::outcome::MutationOutcome::Unknown,
            class: crate::harness::refusal_class::class_of_code("write-failed"),
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a writable standard output for the relay",
            ),
            observed: None,
        },
        Refusal {
            code: "handoff-row-unwritten",
            message: "the `dev:<runtime>` projection landed and the handoff row did not".to_owned(),
            outcome: MutationOutcome::Unknown,
            class: crate::harness::refusal_class::class_of_code("handoff-row-unwritten"),
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                NoCommandReason::WorldAction,
                "read the issue's labels to establish what landed, then repeat the handoff",
            ),
            observed: None,
        },
        Refusal {
            code: "lifecycle-release-publication-failed",
            message: "candidate provenance landed but release publication failed".to_owned(),
            outcome: MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code(
                "lifecycle-release-publication-failed",
            ),
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "inspect lifecycle evidence before retrying the installer",
            ),
            observed: None,
        },
        Refusal {
            code: "setting-shadowed-by-local-file",
            message: "the row is overridden by the operator's local file".to_owned(),
            outcome: crate::outcome::MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code("setting-shadowed-by-local-file"),
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "that row changed or removed in the local file",
            ),
            observed: None,
        },
        // The third of the family, and the one that exists because the first
        // used to absorb it: the table took the value, the row reads as
        // something else, and no operator file is there to explain it. No
        Refusal {
            code: "setting-not-read-back",
            message: "the row was written and reads as something else, with no override there"
                .to_owned(),
            outcome: crate::outcome::MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code("setting-not-read-back"),
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "what answers that row instead, which `estigia config list --agent <agent>` reports",
            ),
            observed: None,
        },
        // The sibling above it is the operator's file and has no command; this
        // one is a file Estigia wrote, so there is one and it clears the block.
        Refusal {
            code: "setting-shadowed-by-agent-file",
            message: "an agent's own file answers that row instead of the table".to_owned(),
            outcome: crate::outcome::MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code("setting-shadowed-by-agent-file"),
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::run(
                "estigia config set \"Model routing\" \"developer=inherit\" --agent claude-code",
            ),
            observed: None,
        },
        Refusal::not_started(
            "reviewer-definition-unowned",
            "the stable reviewer path belongs to another tool",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "move that definition aside before setup",
            ),
        ),
        Refusal::not_started(
            "delegated-definition-unowned",
            "a delegated worker's path belongs to another tool",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "move that definition aside, or stop naming that worker",
            ),
        ),
        Refusal::not_started(
            "reviewer-definition-changed",
            "the setup-owned reviewer contains different bytes",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "move the changed definition aside before setup",
            ),
        ),
    ]
}

/// A home nothing outside the test can reach.
fn sandbox() -> (tempfile::TempDir, SetupOptions) {
    crate::paths::set_fsync_skip_for_tests(true);
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(crate::setup::Platform::Unix),
        ..SetupOptions::default()
    };
    (home, options)
}

/// Opencode is on the neutral root, and neither claude-code nor codex is, so
/// a fixture using all three to see a fresh root among configured neighbours
/// depends on this holding. Asserted rather than branched on: a branch that
/// cannot fail is not a guard, it is an assertion nothing is holding.
fn assert_opencode_is_fresh(opencode: &'static AgentAdapter, options: &SetupOptions) {
    assert!(
        crate::skill::installed_config(
            &setup::resolve_paths(opencode, options)
                .expect("paths")
                .skill_root,
        )
        .is_err(),
        "opencode's neutral root already carries a contract, so this fixture cannot see a fresh root"
    );
}

#[test]
fn each_agent_keeps_its_own_configuration_when_setup_runs_over_all_of_them() {
    // The defect, held: `existing_config` read the *first* root that answered
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    // If these two shared a root the test would prove nothing: it would be one
    // file read twice, which is agreement rather than a flattened disagreement.
    let root_of = |adapter| {
        setup::resolve_paths(adapter, &options)
            .expect("an adapter resolves its paths")
            .skill_root
    };
    assert_ne!(
        root_of(claude),
        root_of(codex),
        "these two adapters share a skill root, so this test cannot see the defect"
    );

    let theirs = Config {
        models: crate::config::ModelRouting::parse("orchestrate=opus").expect("a routing"),
        ..Config::default()
    };
    assert_ne!(theirs, Config::default(), "the two tables must differ");

    setup::setup(claude, &Config::default(), &options).expect("claude-code installs");
    setup::setup(codex, &theirs, &options).expect("codex installs");

    // Two answers are in play, and each adapter's own is the one it keeps.
    let shared = installed_configs(&[claude, codex], &options);
    assert_eq!(
        shared.len(),
        2,
        "the two tables were read as one: {shared:?}"
    );
    assert_eq!(
        config_for(claude, &shared, &options).expect("the contract reads"),
        Config::default()
    );
    assert_eq!(
        config_for(codex, &shared, &options).expect("the contract reads"),
        theirs
    );

    // And running setup again over both leaves each where it was, which is the
    // property the operator actually cares about.
    for adapter in [claude, codex] {
        setup::setup(
            adapter,
            &config_for(adapter, &shared, &options).expect("the contract reads"),
            &options,
        )
        .expect("a re-run installs");
    }
    assert_eq!(
        crate::skill::installed_config(&root_of(codex)).expect("codex kept a contract"),
        theirs,
        "the second agent was overwritten by the first one's table"
    );
}

#[test]
fn a_fresh_agent_inherits_agreed_rows_but_never_an_agent_scoped_one() {
    // This test used to assert whole-`Config` equality: one shared answer, and
    // a fresh root took it entire, `Agent`-scoped rows included. That is the
    // defect the per-row fold in `inherited_from_shared` replaces —
    // `Agent`-scoped rows (here, `models`) belong to whichever agent is
    // holding the tools and are never inherited from a neighbour, single
    // answer or not. What survives from the original test is the shape: one
    // answer in play inherits what there is to inherit, and two disagreeing
    // answers fall back to the default rather than guessing between them.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    let theirs = Config {
        models: crate::config::ModelRouting::parse("orchestrate=opus").expect("a routing"),
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-fresh-agent-inherits-test"),
        ),
        ..Config::default()
    };

    // One answer in play: a root with no contract of its own inherits the
    // `Everywhere`-scoped row it has an unambiguous answer for — `worktree` —
    // which is what keeps adding an agent from resetting somebody's table to
    // defaults. The `Agent`-scoped row (`models`) stays at the portable
    // default regardless, because a single neighbour is still not this
    // adapter's own answer to a question it must answer for itself.
    setup::setup(claude, &theirs, &options).expect("claude-code installs");
    let one = installed_configs(&[claude, codex], &options);
    assert_eq!(one.len(), 1);
    let landed = config_for(codex, &one, &options).expect("the contract reads");
    assert_eq!(
        landed.worktree, theirs.worktree,
        "the sole answer for an `Everywhere`-scoped row was not inherited"
    );
    assert_eq!(
        landed.models,
        Config::default().models,
        "an `Agent`-scoped row was inherited from a single neighbour"
    );

    // Two answers in play: there is nothing unambiguous to inherit for the row
    // they disagree on (`summary_language`), so a fresh root gets the default
    // there rather than whichever one happened to be read first. Guessing here
    // is the whole defect.
    let third = Config {
        summary_language: crate::config::Language::parse("Spanish")
            .expect("a usable language name"),
        ..Config::default()
    };
    setup::setup(codex, &third, &options).expect("codex installs");
    let opencode = find_agent("opencode").expect("opencode is an adapter");
    let two = installed_configs(&[claude, codex], &options);
    assert_eq!(two.len(), 2);
    assert_opencode_is_fresh(opencode, &options);
    assert_eq!(
        config_for(opencode, &two, &options).expect("the contract reads"),
        Config::default(),
        "worktree and summary language disagree and default; every other Everywhere/Machine row \
         agrees, but at the default value already, and models is Agent-scoped and never inherited \
         — the result is Config::default() without a row failing to carry over"
    );
}

#[test]
fn two_agents_that_only_disagree_on_agent_scoped_rows_still_hand_a_fresh_root_their_agreement() {
    // The shape of a real machine: two independently configured agents rarely
    // agree on their `Agent`-scoped rows — who plans, what runs which model,
    // which roles are installed — but there is no reason for them to
    // disagree on a fact about the repository or the machine. The
    // whole-`Config` equality this replaces could not see that: it read the
    // two as flatly different configurations and a fresh root got the
    // portable defaults even though both installed contracts agreed on every
    // row a fresh root could actually inherit.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    let agreed_worktree = crate::config::WorktreeRoot::Path(
        std::env::temp_dir().join("estigia-agreement-test-worktree"),
    );
    let claude_config = Config {
        models: crate::config::ModelRouting::parse("orchestrate=opus").expect("a routing"),
        worktree: agreed_worktree.clone(),
        ..Config::default()
    };
    let codex_config = Config {
        models: Config::default().models,
        worktree: agreed_worktree.clone(),
        ..Config::default()
    };
    assert_ne!(
        claude_config.models, codex_config.models,
        "the fixture must disagree on an `Agent`-scoped row"
    );
    assert_eq!(
        claude_config.worktree, codex_config.worktree,
        "the fixture must agree on an `Everywhere`-scoped row"
    );

    setup::setup(claude, &claude_config, &options).expect("claude-code installs");
    setup::setup(codex, &codex_config, &options).expect("codex installs");

    let shared = installed_configs(&[claude, codex], &options);
    assert_eq!(
        shared.len(),
        2,
        "two adapters disagreeing only on an `Agent`-scoped row must still read as two answers: \
         {shared:?}"
    );

    let opencode = find_agent("opencode").expect("opencode is an adapter");
    assert_opencode_is_fresh(opencode, &options);
    let landed = config_for(opencode, &shared, &options).expect("the contract reads");
    assert_eq!(
        landed.worktree, agreed_worktree,
        "a row both installed contracts agreed on was not inherited by a fresh root"
    );
    assert_eq!(
        landed.models,
        Config::default().models,
        "an `Agent`-scoped row was inherited from a shared answer"
    );
}

#[test]
fn a_named_agents_fresh_root_takes_the_machines_shared_answers() {
    // The defect, held: `estigia setup <slug>` composes its target list through
    // `select`, which for a named agent is one adapter. `installed_configs` used
    // to be computed from that one-element list, so a fresh root's neighbour was
    // never read and `config_for`'s catch-all handed back `Config::default()`
    // with nothing said. A hand-assembled `&[claude, codex]` would not see this:
    // the command never builds its target list that way, so the test has to go
    // through `select` the same way the command does.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    // If these two shared a root the test would prove nothing about a *fresh*
    // root: it would be one file, already read.
    let root_of = |adapter| {
        setup::resolve_paths(adapter, &options)
            .expect("an adapter resolves its paths")
            .skill_root
    };
    assert_ne!(
        root_of(claude),
        root_of(codex),
        "these two adapters share a skill root, so this test cannot see the defect"
    );

    let theirs = Config {
        models: crate::config::ModelRouting::parse("orchestrate=opus").expect("a routing"),
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-fresh-root-test-worktree"),
        ),
        summary_language: crate::config::Language::parse("Spanish")
            .expect("a usable language name"),
        ..Config::default()
    };
    assert_ne!(
        theirs,
        Config::default(),
        "the fixture must differ from the defaults"
    );

    setup::setup(claude, &theirs, &options).expect("claude-code installs");

    // codex's own root has never been written to: this is the fresh root the
    // named-agent command actually operates over.
    let targets = select(Some(codex.slug), false, true).expect("codex is a known slug");
    let shared = installed_configs(&targets, &options);
    let landed = config_for(codex, &shared, &options).expect("the contract reads");

    // The `Everywhere` and `Machine` rows come from the machine's one answer.
    assert_eq!(
        landed.worktree, theirs.worktree,
        "an `Everywhere`-scoped row was not inherited from the machine's only answer"
    );
    assert_eq!(
        landed.summary_language, theirs.summary_language,
        "a `Machine`-scoped row was not inherited from the machine's only answer"
    );
    // The `Agent`-scoped row must stay at the portable default: inheriting it
    // would be a second defect wearing this one's fix.
    assert_eq!(
        landed.models,
        Config::default().models,
        "an `Agent`-scoped row was inherited, which is a different defect"
    );
}

#[test]
fn a_fresh_roots_inheritance_reads_the_contract_and_not_the_operators_local_override() {
    // The local override changes what this adapter reads, not what the table
    // says: `skill::installed_config_for(..).unlayered()` prefers
    // `estigia.local.md`, read with no scope filter, so it can carry an
    // `Everywhere` row — unlike the per-agent layer, which is
    // `Scope::Agent`-only. `existing_config` already draws this line
    // (`skill::contract_config`, not the unlayered value) under the
    // measurement that a machine-local choice must not become the team's;
    // `installed_configs` has to draw the same line, because what it feeds
    // the fold is about to be written into a fresh root's **versioned**
    // block.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    setup::setup(claude, &Config::default(), &options).expect("claude-code installs");
    let root = setup::resolve_paths(claude, &options)
        .expect("claude-code resolves its paths")
        .skill_root;
    std::fs::write(
        root.join("estigia.local.md"),
        "| Setting | Value |\n|---|---|\n| Evidence standard | measuring |\n",
    )
    .expect("the operator's own file");

    let targets = select(Some(codex.slug), false, true).expect("codex is a known slug");
    let shared = installed_configs(&targets, &options);
    let landed = config_for(codex, &shared, &options).expect("the contract reads");
    assert_ne!(
        landed.evidence,
        crate::config::Evidence::Measuring,
        "a fresh root inherited the operator's local override rather than the installed contract"
    );
    assert_eq!(
        landed.evidence,
        Config::default().evidence,
        "claude-code's own contract holds the portable default and the local override does not \
         reach the population, so the fresh root should land on that same default, not on \
         `measuring`"
    );
}

#[test]
fn a_root_whose_local_override_will_not_parse_contributes_nothing_to_the_population() {
    // `installed_configs` reads two things about each root: whether it is
    // readable at all (`skill::installed_config_for`, the gate) and, only if
    // so, what its contract says (`skill::contract_config`, the value). The
    // local-override test above pins the *value* half — a valid override
    // must not be promoted into a fresh root's table. This pins the *gate*
    // half: deleting it leaves the value read intact, since
    // `skill::contract_config` never looks at `estigia.local.md` at all, so
    // a valid contract sitting under an unreadable override would still be
    // read and folded in.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    let theirs = Config {
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-broken-override-test-worktree"),
        ),
        ..Config::default()
    };
    setup::setup(claude, &theirs, &options).expect("claude-code installs");
    let root = setup::resolve_paths(claude, &options)
        .expect("claude-code resolves its paths")
        .skill_root;
    std::fs::write(
        root.join("estigia.local.md"),
        "| Setting | Value |\n|---|---|\n| Evidence standard | not-a-real-standard |\n",
    )
    .expect("the operator's own file");
    assert!(
        skill::installed_config_for(&root, Some(claude.slug)).is_err(),
        "the fixture's override reads cleanly, so this test cannot see the gate"
    );

    // claude-code reaches `installed_configs`' population only through
    // `configured(options)` — it is not among `targets`, which is `[codex]`
    // alone. Without this, `shared.is_empty()` below would hold just as
    // well if `configured` had stopped returning claude-code for an
    // unrelated reason, and the gate this test means to pin would be doing
    // nothing.
    assert!(
        setup::is_present(claude, &options),
        "claude-code is not present, so it was never going to reach the population regardless of the gate"
    );

    let targets = select(Some(codex.slug), false, true).expect("codex is a known slug");
    let shared = installed_configs(&targets, &options);
    assert!(
        shared.is_empty(),
        "a root whose override will not parse still contributed an answer to the population: \
         {shared:?}"
    );
    let landed = config_for(codex, &shared, &options).expect("the contract reads");
    assert_eq!(
        landed,
        Config::default(),
        "a fresh root inherited from a neighbour whose local override will not parse"
    );
}

#[test]
fn fresh_root_notes_names_the_root_when_nothing_else_is_installed() {
    // Half of the silence: `shared.len() < 2` covered two answers and
    // not zero, so a lone named agent with a fresh root said nothing at all.
    // Criterion 5 asks that a run never write defaults into a fresh root
    // without naming the root and the rows it defaulted — this is the case
    // the old guard could never reach.
    let (_home, options) = sandbox();
    let codex = find_agent("codex").expect("codex is an adapter");
    let root = setup::resolve_paths(codex, &options)
        .expect("codex resolves its paths")
        .skill_root
        .display()
        .to_string();

    let shared = installed_configs(&[codex], &options);
    assert!(
        shared.is_empty(),
        "nothing was installed, so there is nothing to share: {shared:?}"
    );

    let lines = fresh_root_notes(&shared, &[codex], &options);
    assert_eq!(
        lines.len(),
        1,
        "one fresh root with nothing installed should print exactly one line: {lines:?}"
    );
    assert!(
        lines[0].contains(&root),
        "the line does not name the fresh root: {lines:?}"
    );
    assert!(
        lines[0].contains("nothing else is installed") && lines[0].contains("every row"),
        "the line does not say every row took the default: {lines:?}"
    );
}

#[test]
fn fresh_root_notes_names_the_disagreeing_row() {
    // Two installed contracts that disagree on an `Everywhere`-scoped row —
    // not the `Agent`-scoped shortcut, so this exercises the fold's own
    // disagreement path rather than the exclusion every row of that scope
    // gets regardless.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");
    let opencode = find_agent("opencode").expect("opencode is an adapter");

    let claude_config = Config {
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-notes-test-worktree-a"),
        ),
        ..Config::default()
    };
    let codex_config = Config {
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-notes-test-worktree-b"),
        ),
        ..Config::default()
    };
    assert_ne!(
        claude_config.worktree, codex_config.worktree,
        "the fixture must disagree on an `Everywhere`-scoped row"
    );

    setup::setup(claude, &claude_config, &options).expect("claude-code installs");
    setup::setup(codex, &codex_config, &options).expect("codex installs");

    let targets = [claude, codex, opencode];
    let shared = installed_configs(&targets, &options);
    assert_eq!(
        shared.len(),
        2,
        "the two disagreeing tables read as one: {shared:?}"
    );

    assert_opencode_is_fresh(opencode, &options);
    let root = setup::resolve_paths(opencode, &options)
        .expect("opencode resolves its paths")
        .skill_root
        .display()
        .to_string();
    let lines = fresh_root_notes(&shared, &targets, &options);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("different configurations")),
        "two disagreeing answers should be announced: {lines:?}"
    );
    let fresh_line = lines
        .iter()
        .find(|line| line.contains(&root))
        .unwrap_or_else(|| panic!("no line names the fresh root {root}: {lines:?}"));
    assert!(
        fresh_line.contains(crate::config::Setting::Worktree.label()),
        "the disagreeing row's label is not named: {fresh_line:?}"
    );
    assert!(
        fresh_line.contains("except"),
        "a row that disagreed must be named as an exception, not folded silently: {fresh_line:?}"
    );
}

#[test]
fn fresh_root_notes_names_no_rows_on_full_agreement() {
    // Two agents disagreeing only on an `Agent`-scoped row, agreeing on every
    // `Everywhere`- and `Machine`-scoped one: a fresh root inherits cleanly and
    // the note says so without naming any row, because none defaulted.
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");
    let opencode = find_agent("opencode").expect("opencode is an adapter");

    let agreed_worktree = crate::config::WorktreeRoot::Path(
        std::env::temp_dir().join("estigia-notes-test-agreement"),
    );
    let claude_config = Config {
        models: crate::config::ModelRouting::parse("orchestrate=opus").expect("a routing"),
        worktree: agreed_worktree.clone(),
        ..Config::default()
    };
    let codex_config = Config {
        models: Config::default().models,
        worktree: agreed_worktree,
        ..Config::default()
    };
    assert_ne!(
        claude_config.models, codex_config.models,
        "the fixture must disagree on an `Agent`-scoped row"
    );

    setup::setup(claude, &claude_config, &options).expect("claude-code installs");
    setup::setup(codex, &codex_config, &options).expect("codex installs");

    let targets = [claude, codex, opencode];
    let shared = installed_configs(&targets, &options);
    assert_eq!(shared.len(), 2, "the two tables read as one: {shared:?}");

    assert_opencode_is_fresh(opencode, &options);
    let root = setup::resolve_paths(opencode, &options)
        .expect("opencode resolves its paths")
        .skill_root
        .display()
        .to_string();
    let lines = fresh_root_notes(&shared, &targets, &options);
    let fresh_line = lines
        .iter()
        .find(|line| line.contains(&root))
        .unwrap_or_else(|| panic!("no line names the fresh root {root}: {lines:?}"));
    // "the machine's answers" and "not this agent's own" are true of the
    // disagreement branch's sentence too (`src/cli/mod.rs`) — it opens with
    // the same words before naming what disagreed. What actually separates
    // this branch from that one is the absence of "except", which the
    // disagreement branch always names. The first two assertions below rule
    // out only the `shared.is_empty()` branch's own sentence, which says
    // neither.
    assert!(
        fresh_line.contains("the machine's answers"),
        "full agreement should say it took the machine's answers: {fresh_line:?}"
    );
    assert!(
        !fresh_line.contains("nothing else is installed"),
        "this is the full-agreement branch, not the nothing-installed one: {fresh_line:?}"
    );
    assert!(
        fresh_line.contains("not this agent's own"),
        "full agreement should say which rows it took answers for: {fresh_line:?}"
    );
    assert!(
        !fresh_line.contains("except"),
        "full agreement must not name a row that disagreed, which is what actually tells this branch apart from the disagreement one: {fresh_line:?}"
    );
}

#[test]
fn fresh_root_notes_answers_nothing_when_no_target_is_fresh() {
    // Both installed with `Config::default()` would de-duplicate to one
    // shared answer, so the `shared.len() >= 2` header would never have fired
    // either way — a test that cannot hold the branch it claims to. The two
    // contracts here genuinely disagree, on an `Everywhere`-scoped row, so
    // this actually exercises "two answers in play, nobody fresh to tell".
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    let claude_config = Config {
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-notes-test-no-fresh-a"),
        ),
        ..Config::default()
    };
    let codex_config = Config {
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-notes-test-no-fresh-b"),
        ),
        ..Config::default()
    };
    assert_ne!(
        claude_config.worktree, codex_config.worktree,
        "the fixture must disagree on an `Everywhere`-scoped row"
    );

    setup::setup(claude, &claude_config, &options).expect("claude-code installs");
    setup::setup(codex, &codex_config, &options).expect("codex installs");

    let targets = [claude, codex];
    let shared = installed_configs(&targets, &options);
    assert_eq!(
        shared.len(),
        2,
        "the two disagreeing tables read as one: {shared:?}"
    );
    let lines = fresh_root_notes(&shared, &targets, &options);
    assert!(
        lines.is_empty(),
        "nothing was fresh, so there is nothing to say: {lines:?}"
    );
}

#[test]
fn the_settings_fold_round_trips_every_everywhere_and_machine_row() {
    // `inherited_from_shared` treats an `apply` that fails on a value
    // `value_of` just rendered as impossible, and reports it apart from a
    // real disagreement rather than folding the two into one message. This
    // pins the totality that rests on, over a configuration that differs
    // from the default on every row this loop touches. Not
    // `Setting::answers()`, whose lists are placeholders
    // (`["unset"]`, `["none"]`, `["800", "400"]`) rather than the values
    // this fold actually renders.
    let sample = Config {
        tracker: crate::config::Tracker::Github {
            repo: Some(crate::config::RepoRef::parse("acme/widgets").expect("a usable repo ref")),
        },
        worktree: crate::config::WorktreeRoot::Path(
            std::env::temp_dir().join("estigia-settings-fold-round-trip"),
        ),
        integration: crate::config::Integration::Trunk,
        window: std::time::Duration::from_secs(60),
        evidence: crate::config::Evidence::Measuring,
        change_size: 400,
        boundaries: vec!["terraform apply".to_owned()],
        summary_language: crate::config::Language::parse("Spanish").expect("a usable language"),
        body_language: crate::config::Language::parse("French").expect("a usable language"),
        ..Config::default()
    };

    for &setting in crate::config::SETTINGS {
        if !matches!(
            setting.scope(),
            crate::config::Scope::Everywhere | crate::config::Scope::Machine
        ) {
            continue;
        }
        let rendered = setting.value_of(&sample);
        let mut round_tripped = sample.clone();
        setting
            .apply(&mut round_tripped, &rendered)
            .unwrap_or_else(|error| {
                panic!("{setting:?} rendered {rendered:?} but could not read it back: {error:?}")
            });
        assert_eq!(
            setting.value_of(&round_tripped),
            rendered,
            "{setting:?} did not round-trip through its own value_of/apply pair"
        );
    }
}

#[test]
fn a_refusal_that_wrote_nothing_says_so() {
    // These are all pre-flight defects: argument parsing and table reading,
    // none of which reaches the tracker. If one of them ever reports `Unknown`
    // the caller is told to go and check, for a write that never happened.
    for refusal in inventory() {
        assert_eq!(
            refusal.outcome,
            MutationOutcome::NotStarted,
            "{} touched the world before validating",
            refusal.code
        );
    }
}

/// The `nothing_configured` refusal is byte-identical when no
/// substitution produced it, and carries the note when one did.
#[test]
fn nothing_configured_is_unchanged_without_a_substitution() {
    let plain = nothing_configured(&[]);
    assert_eq!(plain.code, "nothing-configured");
    assert_eq!(plain.message, "no agent has Estigia installed");

    let note = "XDG_CONFIG_HOME points to /elsewhere outside the resolved home; using /home/.config instead";
    let noted = nothing_configured(&[note.to_owned()]);
    assert_eq!(
        noted.message,
        format!("no agent has Estigia installed\n  note: {note}"),
        "the substitution note did not reach the refusal"
    );
}

///
/// Named modules and not just `mod tests`, because `outcome.rs` calls its
/// `mod message_shape`, and a rule that only knew one name would swap this
/// blindness for a narrower one.
fn shipped_part(source: &str) -> &str {
    let mut at = 0;
    while let Some(found) = source[at..].find("#[cfg(test)]") {
        let start = at + found;
        let rest = source[start + "#[cfg(test)]".len()..].trim_start();
        if rest.starts_with("mod ") {
            return &source[..start];
        }
        at = start + "#[cfg(test)]".len();
    }
    source
}

fn crate_sources() -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a readable source");
            // Strip before splitting: a comment naming `#[cfg(test)]` would
            // otherwise move the cut in the raw text.
            let stripped = crate::source_text::code_of(&source);
            found.push(shipped_part(&stripped).to_owned());
        }
    }
    assert!(
        found.len() > 15,
        "the walk found {} sources, too few to be reading the tree",
        found.len()
    );
    found
}

#[test]
fn the_inventory_covers_every_code_the_crate_raises() {
    let sources = crate_sources();
    let covered = inventory()
        .into_iter()
        .chain(after_writing())
        .map(|refusal| refusal.code.to_owned())
        .collect::<Vec<_>>();
    // files and an error on the third says nothing about the first two. Putting
    // it in was this round's own mistake, and the crate caught it.
    let elsewhere = [
        // Raised after the world was touched, so it cannot join the pre-flight
        // inventory. Checked here instead, on the property that makes it
        // different: an outcome nobody can read has to send the reader to look.
        "setup-write-failed",
        "transport-refused",
        // The five reasons the gate stands **aside**, which are not refusals at
        // all: nothing is refused, nothing is written, and the caller is told
        // Estigia has no authority over what it asked. They are spelled like
        // codes because `gate --json` publishes them and a program matches on
        // them, and each carries its own sentence in `Aside::why` — crossed,
        // with the shape of the names, by
        // `every_reason_for_standing_aside_has_a_stable_name`.
        "not-watched",
        "nothing-sworn",
        "no-tracker",
        "another-checkout",
        "outside-the-claim",
    ];
    // The exemption, checked rather than asserted. A carve-out that names a code
    // nothing raises, or one whose shape nobody ever looked at, is a code the
    // ratchet never sees — which is the whole of what a carve-out costs.
    let unknown_write = super::setup_failed(
        &crate::setup::AGENTS[0],
        &anyhow::anyhow!("the disk filled up halfway through"),
    );
    assert_eq!(unknown_write.code, "setup-write-failed");
    assert_eq!(
        unknown_write.outcome,
        MutationOutcome::Unknown,
        "the one refusal kept out of the pre-flight inventory is now a pre-flight refusal, so \
         nothing is checking it and nothing needs to keep it out"
    );
    assert_eq!(
        unknown_write.replay,
        crate::outcome::Replayability::StatusRequired,
        "an outcome nobody can read was made replayable, which is the lie the taxonomy exists \
         to prevent"
    );
    let translated = crate::harness::tracker::TRANSPORT_VOCABULARY;

    for source in &sources {
        for raised in codes_in(source) {
            assert!(
                covered.contains(&raised)
                    || elsewhere.contains(&raised.as_str())
                    || translated.contains(&raised.as_str()),
                "`{raised}` is raised but never put through the ratchet; add it to inventory()"
            );
        }
    }
}

#[test]
fn a_workspace_refusal_names_the_command_that_clears_it() {
    // One constructor serves setup, sync and uninstall, so a way out hard-coded
    // to `setup` would send an uninstall at the install it was asked to remove.
    // The verb is the caller's, and the command the refusal names is theirs.
    use crate::cli::args::ScopeArg;
    for (verb, scope, workspace) in [
        ("setup", ScopeArg::Workspace, None),
        ("setup --uninstall", ScopeArg::Workspace, None),
        ("uninstall", ScopeArg::Workspace, None),
        (
            "sync",
            ScopeArg::Global,
            Some(std::path::PathBuf::from("/tmp/a-workspace")),
        ),
    ] {
        let refusal = super::install_options(false, scope, workspace, verb)
            .expect_err("a workspace that names nothing is refused");
        assert!(
            refusal
                .resolution
                .to_string()
                .contains(&format!("estigia {verb} ")),
            "the way out under `{verb}` does not name `{verb}`: {}",
            refusal.resolution
        );
    }
}

#[test]
fn the_setup_uninstall_arm_names_uninstall_in_its_way_out() {
    // The `Command::Setup` arm branches on `--uninstall` after it builds the
    // options, so the verb it hands the refusal has to follow that flag. The
    // test above calls `install_options` directly and cannot see the branch.
    let cli = Cli::try_parse_from(["estigia", "setup", "--uninstall", "--scope=workspace"])
        .expect("the command parses");
    let refusal = super::dispatch(&cli).expect_err("a workspace with no path is refused");
    assert!(
        refusal.resolution.to_string().contains("uninstall"),
        "setup --uninstall named a way out that does not uninstall: {}",
        refusal.resolution
    );
}

#[test]
fn the_companion_path_refuses_a_scope_that_names_nothing_too() {
    // The companion branch installs nothing, but a `--scope` typo there is
    // still a flag that names nothing, which the two refusals exist to refuse.
    let cli = Cli::try_parse_from([
        "estigia",
        "setup",
        "--companion",
        "leteo",
        "--scope=workspace",
    ])
    .expect("the command parses");
    let refusal = super::dispatch(&cli).expect_err("a workspace with no path is refused");
    assert_eq!(refusal.code, "workspace-without-a-path");

    // And a workspace that IS named: a companion installs nothing into it, so
    // the path would be read by nothing.
    let cli = Cli::try_parse_from([
        "estigia",
        "setup",
        "--companion",
        "leteo",
        "--scope=workspace",
        "--workspace",
        "/tmp/a-workspace",
    ])
    .expect("the command parses");
    let refusal = super::dispatch(&cli).expect_err("a companion with a workspace is refused");
    assert_eq!(refusal.code, "companion-with-a-workspace");
    assert!(
        refusal
            .resolution
            .to_string()
            .contains("estigia setup --companion"),
        "the way out does not name the command that clears it: {}",
        refusal.resolution
    );
}

/// The codes a `fn code(…) -> &'static str` hands out, one per match arm.
///
/// A third way a refusal gets its code, and the walk could not see it: the
/// yields nothing, and the test passes louder the less it reads.
///
/// Scoped to the body of such a function rather than to every `=> "..."` in the
/// tree, because a literal wrongly counted as raised is a code the *forward*
/// guard then demands an inventory entry for.
fn codes_named_by_a_code_method(source: &str) -> Vec<String> {
    let mut codes = Vec::new();
    let mut rest = source;
    while let Some(position) = rest.find("fn code(") {
        rest = &rest[position + "fn code(".len()..];
        let Some(arrow) = rest.find("-> &'static str") else {
            continue;
        };
        let body = &rest[arrow..];
        let Some(open) = body.find('{') else { continue };
        let mut depth = 0usize;
        let mut end = body.len();
        for (at, character) in body[open..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + at;
                        break;
                    }
                }
                _ => {}
            }
        }
        for found in body[open..end].split("=> \"").skip(1) {
            let Some(close) = found.find('"') else {
                continue;
            };
            let code = &found[..close];
            if !code.is_empty()
                && code.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                && code.contains('-')
            {
                codes.push(code.to_owned());
            }
        }
    }
    codes
}

#[test]
fn an_unreadable_contract_narrows_the_reviewers_grant() {
    use crate::config::Evidence;

    for installed in Evidence::all() {
        assert_eq!(
            effective_evidence(true, installed),
            Evidence::Reading,
            "{installed:?}: a contract that could not be read handed out the wider grant"
        );
        assert_eq!(
            effective_evidence(false, installed),
            installed,
            "{installed:?}: a readable contract was not believed"
        );
    }
}

/// The first string literal after each `Refusal::not_started(`, `Refusal::stale(`,
/// `Refusal::unavailable(`, `code:` or `const CODE: &str =`.
fn codes_in(source: &str) -> Vec<String> {
    let mut codes = Vec::new();
    codes.extend(codes_named_by_a_code_method(source));
    for marker in [
        "Refusal::not_started(",
        "Refusal::stale(",
        "Refusal::unavailable(",
        "code: ",
        "const CODE: &str = ",
    ] {
        let mut rest = source;
        while let Some(position) = rest.find(marker) {
            rest = &rest[position + marker.len()..];
            let leading = rest.len() - rest.trim_start().len();
            if !rest[leading..].starts_with('"') {
                continue;
            }
            let Some(open) = rest.find('"') else { break };
            let Some(close) = rest[open + 1..].find('"') else {
                break;
            };
            let code = &rest[open + 1..open + 1 + close];
            // Only the kebab-case code shape; message strings are skipped.
            if !code.is_empty()
                && code.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                && code.contains('-')
            {
                codes.push(code.to_owned());
            }
        }
    }
    codes.sort_unstable();
    codes.dedup();
    codes
}

#[test]
fn the_parser_accepts_the_shapes_the_handoff_specified() {
    for command in [
        vec!["estigia", "tui"],
        vec!["estigia", "setup"],
        vec!["estigia", "setup", "claude-code"],
        vec!["estigia", "setup", "--all"],
        vec!["estigia", "setup", "--all", "--dry-run"],
        vec!["estigia", "setup", "claude-code", "--uninstall"],
        vec!["estigia", "setup", "--companion", "leteo"],
        vec!["estigia", "install", "claude-code"],
        vec!["estigia", "sync"],
        vec!["estigia", "sync", "--allow-source-build"],
        vec!["estigia", "setup", "claude-code", "--allow-source-build"],
        vec!["estigia", "update"],
        vec!["estigia", "update", "--json"],
        vec!["estigia", "status"],
        vec!["estigia", "doctor"],
        vec!["estigia", "guard"],
        vec!["estigia", "guard", "--uninstall"],
        vec!["estigia", "hook", "pre-push"],
        vec!["estigia", "doctor", "--json"],
        vec!["estigia", "uninstall", "--all"],
        vec!["estigia", "config", "list"],
        vec!["estigia", "config", "set", "Merge strategy", "squash"],
        vec!["estigia", "status", "--json"],
    ] {
        Cli::try_parse_from(&command)
            .unwrap_or_else(|error| panic!("`{}` must parse:\n{error}", command.join(" ")));
    }
}

#[test]
fn an_agent_and_all_together_are_refused_by_the_parser() {
    assert!(Cli::try_parse_from(["estigia", "setup", "claude-code", "--all"]).is_err());
}

#[test]
fn tui_is_its_own_verb_and_setup_does_not_open_the_screen() {
    match Cli::try_parse_from(["estigia", "tui"]) {
        Ok(Cli {
            command: Command::Tui,
            ..
        }) => {}
        other => panic!("`estigia tui` parsed as {other:?}"),
    }
    match Cli::try_parse_from(["estigia", "setup"]) {
        Ok(Cli {
            command: Command::Setup { agent: None, .. },
            ..
        }) => {}
        other => panic!("`estigia setup` parsed as {other:?}"),
    }
    assert!(
        Cli::try_parse_from(["estigia", "setup", "--interactive"]).is_err(),
        "setup still has an interactive flag"
    );
    assert!(
        Cli::try_parse_from(["estigia", "tui", "--allow-source-build"]).is_err(),
        "`estigia tui` still takes a source-build flag"
    );
}

#[test]
fn wrapping_counts_characters_rather_than_bytes() {
    let accented = "eñe eñe eñe eñe";
    let plain = "abc abc abc abc";
    // Counting lines is not enough: two different wrappings can land on the
    // same number of them. What has to match is how many words each line got.
    let shape = |text: &str| -> Vec<usize> {
        super::wrapped(text, 11)
            .iter()
            .map(|line| line.split_whitespace().count())
            .collect()
    };
    assert_eq!(
        shape(accented),
        shape(plain),
        "the same shape in characters wrapped differently: {:?} against {:?}",
        super::wrapped(accented, 11),
        super::wrapped(plain, 11)
    );

    // The boundary itself: two words that fit exactly stay together, and one
    // character more moves the second down.
    assert_eq!(super::wrapped("ab cd", 5), vec!["ab cd"]);
    assert_eq!(super::wrapped("ab cde", 5), vec!["ab", "cde"]);
}

#[test]
fn wrapping_never_splits_a_word_and_never_loses_one() {
    // The long words in these messages are filesystem paths, where a break in
    // the middle is worse than a long line. And nothing may be dropped: a
    // wrapper that silently ate a word would take the path out of the one
    // message that names it.
    let path = "C:/Users/alex/AppData/Local/estigia/skills/issue-flow/scripts/github.py";
    let text = format!("run {path} now");
    for width in [1, 2, 5, 20, 80] {
        let lines = super::wrapped(&text, width);
        assert!(
            lines.iter().any(|line| line.contains(path)),
            "width {width}"
        );
        assert_eq!(
            lines.join(" ").split_whitespace().collect::<Vec<_>>(),
            text.split_whitespace().collect::<Vec<_>>(),
            "width {width} changed the words"
        );
    }

    // Degenerate inputs answer rather than loop or panic.
    assert!(super::wrapped("", 40).is_empty());
    assert!(super::wrapped("   ", 40).is_empty());
    assert_eq!(super::wrapped("solo", 0), vec!["solo"]);
}

#[test]
fn a_window_is_named_in_the_words_a_person_uses_for_it() {
    // `1 minute(s)` reads as a defect in the tool rather than as a rounding of
    // English, and this is the one message an operator sees at the moment they
    // are loosening their own gate.
    assert_eq!(super::minutes_left(60), "1 minute");
    assert_eq!(super::minutes_left(120), "2 minutes");
    assert_eq!(super::minutes_left(1800), "30 minutes");
    // Rounded up, never down: a window with forty seconds left has not run out,
    // and reporting "0 minutes" would say it had.
    assert_eq!(super::minutes_left(40), "1 minute");
    assert_eq!(super::minutes_left(61), "2 minutes");
    assert_eq!(super::minutes_left(0), "less than a minute");
}

#[test]
fn declaring_and_lifting_a_stand_down_go_on_the_record() {
    // which is the whole shape this feature claims to have. The ledger is
    // append-only and is where it belongs.
    //
    // It also closes the rolling window: the four-hour cap is enforced per
    // declaration, so re-declaring resets the clock. That is still allowed —
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    for claim in [
        "\"verdict\": \"stand-down-declared\"",
        "\"verdict\": \"stand-down-lifted\"",
        "\"superseded\"",
    ] {
        assert!(
            source.contains(claim),
            "{claim} is not written to the ledger, so the act leaves no trace"
        );
    }
    // And the ledger's writer is the one that already exists, not a second
    // history file for somebody to find later and wonder about.
    assert!(
        source.contains("harness::session::record"),
        "the stand-down keeps a record of its own somewhere else"
    );
    // Replacing somebody's window says so on the way past. Silently taking over
    // a colleague's declaration is the failure the trail alone cannot fix,
    // because nobody reads a ledger at the moment they type a command.
    assert!(
        source.contains("this replaces one declared by"),
        "a stand-down can replace another without saying it did"
    );
    // And lifting one that had already expired says that too, so nobody walks
    // away believing they closed a window that was open.
    assert!(
        source.contains("nothing was in force"),
        "lifting nothing reports the same as lifting something"
    );
}

#[test]
fn a_value_the_command_line_alone_can_refute_is_refused_before_the_machine_is_read() {
    // `config set --help` promises "validating it before anything is written",
    // and the label was checked that way while the value was not — two
    // different answers to one question.
    //
    let refusal = super::config_set(
        "Integration",
        "octopus",
        None,
        false,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("octopus is not an integration mode");
    assert_eq!(refusal.code, "config-value-unrecognised");
    // And it names what may be written instead, which is the whole point of
    // getting this refusal rather than the other one.
    let message = format!("{refusal}");
    assert!(message.contains("branch"), "{message}");

    // The label is still checked first, because a value cannot be judged
    // against a setting nobody named.
    let refusal = super::config_set(
        "Nonexistent thing",
        "octopus",
        None,
        false,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("that is not a setting");
    assert_eq!(refusal.code, "setting-unknown");

    // An agent nobody has heard of is refused before the value, because it
    // names which table the value would be judged against.
    let refusal = super::config_set(
        "Integration",
        "octopus",
        Some("nosuchagent"),
        false,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("that is not an agent");
    assert_eq!(
        refusal.code, "agent-unknown",
        "with an unknown agent the agent went unmentioned"
    );
}

/// `Delivery route`, `Review delegation`, `Merge strategy`, `Review protocol`
/// and the `Repair ceiling` setting are retired: an installed one still loads,
/// and a new write of any is refused by name through `config_set`, before the
/// machine is read, saying what the row governs instead.
#[test]
fn a_retired_setting_is_refused_through_config_set_by_its_own_code() {
    let home = tempfile::tempdir().expect("a home the write would land in");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData")),
        claude_config_dir: Some(home.path().join(".claude")),
        ..SetupOptions::default()
    };
    for (label, value, code, instead) in [
        (
            "Delivery route",
            "direct",
            "delivery-route-retired",
            "Delivery route",
        ),
        (
            "Review delegation",
            "ask",
            "review-delegation-retired",
            "Review delegation",
        ),
        (
            "Merge strategy",
            "merge commit",
            "merge-strategy-retired",
            "Merge strategy",
        ),
        (
            "Review protocol",
            "rdd",
            "review-protocol-retired",
            "Review protocol",
        ),
        (
            "Repair ceiling",
            "3",
            "repair-ceiling-retired",
            "Repair ceiling",
        ),
    ] {
        let refusal = super::config_set(label, value, None, false, false, &options, false)
            .expect_err("a retired row was written");
        assert_eq!(refusal.code, code, "{label} {value}");
        assert!(
            format!("{refusal}").contains(instead),
            "{label} {value}: the refusal does not name the retired row"
        );
    }
}

#[test]
fn a_row_an_agent_acts_on_nothing_for_is_refused_before_the_machine_is_read() {
    // `Loop plugin` is OpenCode's: every other adapter reads no plugin Estigia
    // writes, so setting it there is refused before the machine is read rather
    // than written and then read back as somebody else's answer.
    let refusal = super::config_set(
        "Loop plugin",
        "on",
        Some("cursor"),
        false,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("cursor reads no plugin Estigia writes");
    assert_eq!(refusal.code, "setting-not-for-this-agent");
}

#[test]
fn status_does_not_say_a_gate_is_on_when_it_could_not_run() {
    // `gated` says an entry exists in a settings file. That is not the same as
    //
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    // row's `(true, true)` arm to `"on"` left this green, and *that is the
    // defect it was written for* — `status` knew how to say `gate REGISTERED
    // BUT DEAD` and went on saying `tools on` with the server pointing at
    // nothing. The guard could not tell which row had lost it.
    //
    // Counted off the arms rather than a number in a test: a third row added
    // later inherits the rule instead of quietly halving the check.
    let line = source
        .find("harness: gate {}, tools {}")
        .expect("`status` reports the harness on one line");
    let rows = &source[line..];
    let rows = &rows[..rows.find("\n            );").unwrap_or(rows.len())];
    let registered = rows.matches("(true, true) =>").count();
    assert!(
        registered >= 2,
        "the harness line reports fewer than two things, so this is measuring one of them"
    );
    assert_eq!(
        rows.matches("(true, true) => \"REGISTERED BUT DEAD\"")
            .count(),
        registered,
        "one of the harness rows says a registered thing is on without asking whether it \
         would run:\n{rows}"
    );

    // And every fault names the way out on the spot, rather than leaving
    // somebody to guess which of the eleven agents to re-register. Both faults,
    // for the same reason: dropping the command from one of them left this
    // green too.
    let faults = source.matches("if let Some(fault) = &row.").count();
    assert!(
        faults >= 2,
        "the report has fewer than two faults to name a command for"
    );
    assert_eq!(
        source.matches("run: estigia setup {}").count(),
        faults,
        "a reported fault comes with no command that discharges it"
    );

    // The fault itself comes from the reader that knows, not from a second
    // opinion about what a live gate looks like.
    assert!(
        source.contains("setup::wiring::registered"),
        "`status` decides for itself whether a gate is live"
    );
}

/// The harness line reads `(true, true)` as `REGISTERED BUT DEAD`, so a fault
/// firing for a healthy entry contradicts the `tools on` printed beside it.
#[test]
fn a_healthy_opencode_entry_is_not_reported_as_no_registration() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(setup::Platform::Unix),
        ..SetupOptions::default()
    };
    let adapter = setup::find_agent("opencode").expect("opencode is an agent this build knows");
    setup::setup(adapter, &Config::default(), &options).expect("setup writes the entry");
    assert!(
        setup::exposes_tools(adapter, &options),
        "the fixture did not register the server, so this measures the wrong state"
    );
    assert_eq!(
        super::tools_fault(adapter, &options),
        None,
        "a registered OpenCode server was read as no registration, which the harness line \
         prints as REGISTERED BUT DEAD"
    );
}

#[test]
fn status_reports_whether_a_hook_can_continue_a_turn() {
    // The row carries it, so `status --json` does too; and the human report
    // prints it beside the harness line. Read off the source the way the
    // neighbouring status test does: the words themselves are pinned in
    // `setup`, this holds the wiring.
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    assert!(
        source.contains("continuation: adapter.continuation()"),
        "`status` stopped asking each agent whether a hook can continue its turn"
    );
    assert!(
        source.contains("continuation: {}"),
        "`status` stopped printing the continuation word beside the harness line"
    );
}

#[test]
fn nothing_the_gate_reads_is_a_row_one_agent_can_answer_for_itself() {
    for setting in [
        crate::config::Setting::Window,
        crate::config::Setting::Tracker,
        crate::config::Setting::Boundaries,
        crate::config::Setting::Integration,
    ] {
        assert_eq!(
            setting.scope(),
            crate::config::Scope::Everywhere,
            "{setting:?} reaches the gate, which cannot tell which agent it is \
             answering, so it cannot be a row one agent answers for itself"
        );
    }

    // And the command line says so rather than writing it anyway.
    let refusal = super::config_set(
        "Renewal window",
        "30m",
        Some("opencode"),
        false,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("a repository row was written for one agent");
    assert_eq!(refusal.code, "setting-not-per-agent");
}

#[test]
fn the_push_hook_does_not_invent_a_working_directory_it_could_not_read() {
    // push left through `Decision::Outside`: the branch that lets it through,
    // reached because a question could not be asked rather than because it was
    // answered. Silently, which is the shape the ledger's `silence` check
    // exists to catch.
    //
    // Structural for the reason the gate's clock guard is: a test cannot take
    // this process's working directory away, and a guard that needs one to
    // disappear is a guard that never fires. Only the boundaries get this
    // treatment — `doctor` still defaults to `.` and reports about it, which is
    // a wrong line in a report rather than an unchecked push.
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    assert!(
        source.contains("let Ok(repo_dir) = std::env::current_dir() else {"),
        "the push hook takes a working directory it may not have been given"
    );
    assert!(
        source.contains("so this push was not checked"),
        "a push that skipped the check because of a fault says nothing about it"
    );
}

#[test]
fn a_contract_that_will_not_parse_does_not_hand_out_what_it_could_not_grant() {
    use std::time::Duration;

    // `gate_context` falls back to `Config::default()` when the contract will
    // not parse, and that is deliberate: stopping an edit mid-flight because a
    // operator is likely to have written. Pinned here, because the whole
    // fallback rests on it and a future default that quietly changed either
    // would move the harness's floor without anybody noticing.
    let default = Config::default();
    assert!(
        default.boundaries.is_empty(),
        "the default declares boundaries now, so falling back to it no longer \
         silently drops the operator's"
    );
    assert!(
        default.window > Duration::ZERO,
        "the default window is no longer a grant, so the fallback below is moot"
    );

    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.mark_verified();
    assert!(
        !run.within_window(Duration::ZERO),
        "a zero window still let a write ride an answer the tracker gave earlier"
    );

    // the same excuse, three blind reviewers each defeated the source-text guard
    // it justified while the suite stayed green, and the crossing that replaced it
    // drives the binary against a throwaway home — which `tests/pipe.rs` already
    // does routinely, in most of its tests. This said *sixteen* until a reviewer
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    assert!(
        source.contains("window: if unreadable {"),
        "a contract nothing could read is granting a renewal window again"
    );

    // The other half was said here to be untakeable back — the phase check needs
    // the list that went missing — and it was untakeable only because one bad row
    // discarded the whole document. The gate now reads what parses, so the loss
    // is the bad row and nothing beside it.
    assert!(
        source.contains("installed_config_in_keeping_what_parses"),
        "the gate is back to losing every row over one it cannot read"
    );

    let doctor = crate::source_text::code_of(include_str!("../harness/doctor.rs"));
    assert!(
        doctor.contains("runs without the setting that"),
        "doctor reports the bad row without saying what it costs"
    );
    assert!(
        !doctor.contains("command declared irreversible is treated as one"),
        "doctor still tells an operator that every declared boundary is gone"
    );
}

#[test]
fn a_table_that_does_not_read_back_is_not_a_table_that_reads_back_as_written() {
    // Three sites write the configuration and then read it back, for the reason
    // one of them puts in words: "write, then read *both* back, and believe the
    // readback rather than the write". All three wrote `unwrap_or(config)`,
    // which believes the write — in the one case where the readback had
    // anything to say.
    //
    // Reaching it means Estigia wrote a table Estigia cannot read: every other
    // way to an unparseable contract is refused before the write, by
    // `writable_config`. That is the argument for reporting it rather than
    // against: nothing else would ever notice.
    let refusal = read_back(Err(Refusal::not_started(
        "config-value-unrecognised",
        "`Integration` is \"octopus\", which is not one of its values",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`Integration`: `branch`, or `trunk`",
        ),
    )))
    .expect_err("a table that will not parse is not a configuration");

    // The write landed, so `not_started` would send an operator to repeat a
    // write that already happened — the distinction `shadowed` makes two
    // functions above, for the same reason.
    assert_eq!(refusal.outcome, MutationOutcome::Committed);
    assert!(
        refusal.message.contains("does not read back"),
        "it does not say what happened: {}",
        refusal.message
    );
    // And it keeps the row's own answer rather than inventing a generic one.
    assert!(
        format!("{refusal}").contains("`Integration`: `branch`, or `trunk`"),
        "the readback's own resolution was dropped: {refusal}"
    );

    // A table that reads back is handed through untouched.
    let config = Config::default();
    assert_eq!(
        read_back(Ok(config.clone())).expect("a readable table"),
        config
    );
}

#[test]
fn a_contract_that_will_not_parse_is_not_a_root_with_no_contract() {
    // The distinction both write paths now turn on. `sync` and `setup` reach it
    // through `config_for`; the screen reads every table to show it, and read
    // the unreadable ones as `Config::default()` — so it showed an operator
    // `branch` where their file said `trunk`, and saving wrote back what it had
    // shown. One function, asked at both writes.
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(setup::Platform::Unix),
        ..SetupOptions::default()
    };
    let adapter = find_agent("codex").expect("codex is an agent");

    // Nothing installed: no answer yet, which inherits rather than refusing.
    assert_eq!(
        existing_config(adapter, &options).expect("an absent contract is not a fault"),
        None
    );

    let mut config = Config::default();
    config.evidence = crate::config::Evidence::Measuring;
    setup::setup(adapter, &config, &options).expect("the install writes");
    assert_eq!(
        existing_config(adapter, &options)
            .expect("a readable contract")
            .map(|held| held.evidence),
        Some(crate::config::Evidence::Measuring),
        "the contract that was just written does not read back"
    );

    // A value the parser refuses, in the operator's own file beside it.
    let paths = setup::resolve_paths(adapter, &options).expect("paths");
    std::fs::write(
        paths.skill_root.join(crate::config::LOCAL_FILE),
        "| Setting | Value |\n|---|---|\n| Evidence standard | octopus |\n",
    )
    .expect("their file");
    let refusal = existing_config(adapter, &options)
        .expect_err("a contract that will not parse is not an absent one");
    assert_ne!(
        refusal.code, "skill-not-installed",
        "an unreadable contract is being reported as an absent one, which is the \
         answer that inherits the defaults and writes them back"
    );
}

#[test]
fn no_two_things_a_run_can_do_to_a_file_are_said_with_the_same_word() {
    use crate::skill::Change;

    let vocabulary = [
        Change::Create,
        Change::Update,
        Change::Replace,
        Change::Remove,
        Change::Kept,
        Change::Shared,
        Change::Unrecorded,
    ];
    // Both tenses, because a plan and a run say the same things in different
    // words and each vocabulary has to stay unambiguous on its own. The plan's
    // words are what stop a line read on its own from being mistaken for one
    // that happened.
    for planned in [false, true] {
        let mut said: Vec<&'static str> = Vec::new();
        for change in vocabulary {
            let mark = word(change, planned)
                .unwrap_or_else(|| panic!("{change:?} is reported as nothing"));
            assert!(
                !said.contains(&mark),
                "{change:?} is said with {mark:?}, which already means something else"
            );
            assert!(mark.len() <= 13, "{mark:?} does not fit the column");
            said.push(mark);
        }
        // A plan says what it *would* do, or its lines cannot be told from a
        // run's. `shared` and `unknown` are states rather than acts and stay as
        // they are.
        if planned {
            let acts = said
                .iter()
                .filter(|mark| mark.starts_with("would "))
                .count();
            assert_eq!(
                acts, 5,
                "a plan says {acts} of its acts in the conditional: {said:?}"
            );
        } else {
            assert!(
                !said.iter().any(|mark| mark.starts_with("would ")),
                "a run reports what it did in the conditional: {said:?}"
            );
        }
    }

    // And the one that earns no row: a file this run did not touch.
    assert_eq!(word(Change::Unchanged, false), None);
    assert_eq!(word(Change::Unchanged, true), None);
}

#[test]
fn an_agent_estigia_gates_does_not_read_like_one_it_has_never_touched() {
    use crate::skill::Presence;

    // the skill, the gate and the MCP server, and it printed the same words as
    // an agent Estigia has never been near. `harness: gate on` underneath and
    // `not configured` above it, on the same screen.
    let untouched = standing(false, Presence::Current, true, false);
    let gated = standing(false, Presence::Current, true, true);
    assert_ne!(
        untouched, gated,
        "an agent whose every write goes through Estigia is described \
         exactly as one Estigia does not know about"
    );

    // The seven still read as before: this narrows the special case, it does
    // not take it away.
    assert_eq!(untouched, "not configured");

    // And nothing else moved. A directive makes it configured whoever else
    // shares the root, and an absent skill is absent whatever is registered.
    for shared in [true, false] {
        for touched in [true, false] {
            assert_eq!(
                standing(true, Presence::Current, shared, touched),
                "configured"
            );
            assert_eq!(
                standing(false, Presence::Absent, shared, touched),
                "not configured"
            );
        }
    }
}

#[test]
fn every_setting_the_gate_reads_is_one_that_cannot_vary_by_agent() {
    use crate::config::{EVERYWHERE_SETTINGS, Setting};

    for setting in [
        Setting::Integration,
        Setting::Window,
        Setting::GitOperationWindow,
        Setting::Tracker,
        Setting::RepositoryHost,
        Setting::Boundaries,
        // Arrived with the role work, and this test is what said it had to be here:
        // the gate reads it to render the reserved reviewer's tool grant, and a
        // per-agent answer to it would be written, read back, and never
        // consulted by the one gate that acts on it.
        Setting::Evidence,
        Setting::DecisionMode,
    ] {
        assert!(
            EVERYWHERE_SETTINGS.contains(&setting),
            "{setting:?} is read by the gate and can be written per agent, so a \
             per-agent value for it would be written, read back, and ignored"
        );
    }

    assert!(crate::config::REPOSITORY_SETTINGS.contains(&Setting::IssueRequirement));
    assert!(crate::config::REPOSITORY_SETTINGS.contains(&Setting::Workflow));

    let source = crate::source_text::code_of(include_str!("mod.rs"));
    let (start, _) = source
        .match_indices("fn gate_context(")
        .next()
        .expect("the gate's context is built here");
    let body = &source[start..];
    let end = body.find("\n}\n").unwrap_or(body.len());
    assert_eq!(
        body[..end].matches("installed.").count(),
        10,
        "the gate reads a different number of settings than this test names, \
         and every one of them has to be repository-wide"
    );
}

/// `sync` keeps both halves of what it promises, for every adapter.
///
/// *"Bring an installed skill up to this binary's copy, **keeping the
#[test]
fn sync_moves_the_markdown_forward_and_keeps_the_table() {
    use crate::setup::AGENTS;

    let mine = Config {
        evidence: crate::config::Evidence::Measuring,
        integration: crate::config::Integration::Trunk,
        models: crate::config::ModelRouting::parse("orchestrate=opus").expect("a routing"),
        boundaries: vec!["npm publish".to_owned()],
        // Away from the default on purpose. This loop asserts every row, and a
        // row left at its default is asserted by a `sync` that dropped it — the
        // shape of test that passes while the thing it names does not work.
        change_size: 300,
        ..Config::default()
    };

    for adapter in AGENTS {
        let home = tempfile::tempdir().expect("a temporary home");
        let options = SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            platform: Some(setup::Platform::Unix),
            ..SetupOptions::default()
        };
        setup::setup(adapter, &mine, &options).expect("setup writes my answers");

        let root = setup::resolve_paths(adapter, &options)
            .expect("paths resolve")
            .skill_root;
        // The one file Estigia never writes, holding one row of their own.
        let ours = root.join("operator.local.md");
        std::fs::write(
            &ours,
            "| Setting | Value |\n|---|---|\n| Review budget | 5000 |\n",
        )
        .expect("their own file");

        // Exactly what `Command::Sync` does, per adapter.
        let shared: Vec<Config> = Vec::new();
        let theirs = SetupOptions {
            skip_directive: !setup::is_configured(adapter, &options),
            ..options.clone()
        };
        let resolved =
            super::config_for(adapter, &shared, &options).expect("the installed table reads");
        let mut pending = setup::Pending::new();
        setup::setup_into(adapter, &resolved, &theirs, &mut pending).expect("sync runs");

        let after = crate::skill::installed_config_for(&root, Some(adapter.slug))
            .expect("the contract reads");

        let mut expected = mine.clone();
        expected.review_budget = crate::config::ReviewBudget::Tokens(5000);
        for setting in crate::config::SETTINGS {
            assert_eq!(
                setting.value_of(&after),
                setting.value_of(&expected),
                "{}: sync did not keep {}",
                adapter.slug,
                setting.label()
            );
        }
        // The installed files carry the operator's own override, and asking
        // whether they match *that* is asking whether `SKILL.md` carries a value
        // the override exists to keep out of it. It cannot, by design, so this
        // assertion held only while `sync` was writing the override into the
        // versioned block. That is what it was doing, and it is the defect this
        // pair now names.
        assert_eq!(
            crate::skill::presence_of(&root),
            crate::skill::Presence::Current,
            "{}: sync left the skill behind the binary",
            adapter.slug
        );
        // And the override stayed out of the file that is committed.
        assert_eq!(
            crate::skill::contract_config(&root)
                .expect("the contract reads")
                .review_budget,
            mine.review_budget,
            "{}: sync promoted an `estigia.local.md` row into the versioned block",
            adapter.slug
        );

        // And their file is still theirs.
        assert!(
            std::fs::read_to_string(&ours)
                .unwrap_or_default()
                .contains("| Review budget | 5000 |"),
            "{}: sync wrote over the one file Estigia never writes",
            adapter.slug
        );
    }
}

/// A `Resolution::run` argument the source scan could not read as a command,
/// named so a reader can find it rather than have it silently skipped.
#[derive(Debug)]
struct UnreadArgument {
    file: String,
    line: usize,
    argument: String,
}

/// What a scan of `Resolution::run` sites found: the commands it could read,
/// and the arguments it could not.
#[derive(Debug)]
struct Suggested {
    commands: Vec<(String, String)>,
    unread: Vec<UnreadArgument>,
}

/// The text of a `Resolution::run` argument, up to its matching close paren,
/// and how far past the site that is.
///
/// `rest` starts just after `Resolution::run(`. Nested parens are counted and
/// parens inside string literals ignored, so a `format!(...)` or a call with
/// its own arguments is read whole rather than cut at the first `)`.
fn resolution_argument(rest: &str) -> Option<(String, usize)> {
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escaped = false;
    for (at, ch) in rest.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((rest[..at].trim().to_owned(), at + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// The contents of a string literal at the start of `text` — a `"..."`, a raw
/// `r"..."`, or a raw `r#"..."#` with any number of hashes — if there is one.
fn string_literal(text: &str) -> Option<String> {
    let text = text.trim_start();
    if let Some(body) = text.strip_prefix('"') {
        let mut escaped = false;
        for (at, ch) in body.char_indices() {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some(body[..at].to_owned());
            }
        }
        return None;
    }
    let rest = text.strip_prefix('r')?;
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    let body = rest[hashes..].strip_prefix('"')?;
    let close = format!("\"{}", "#".repeat(hashes));
    let end = body.find(&close)?;
    Some(body[..end].to_owned())
}

/// The command a `Resolution::run` argument states, when the argument is a
/// string literal or a `format!` whose first literal is the command.
fn command_of_argument(argument: &str) -> Option<String> {
    let argument = argument.trim_start();
    let literal =
        if argument.starts_with('"') || argument.starts_with("r\"") || argument.starts_with("r#") {
            string_literal(argument)
        } else if let Some(format) = argument.strip_prefix("format!") {
            string_literal(format.trim_start().trim_start_matches('('))
        } else {
            None
        }?;
    (literal.starts_with("estigia ") || literal.starts_with("cargo ")).then_some(literal)
}

/// Every `Resolution::run` in the shipped `src/` under `root`.
///
/// Two populations come back. `commands` are the arguments the scan could read:
/// a string literal, or a `format!`'s first literal. `unread` are the ones it
/// could not — a `const`, a function call, anything whose command is not at the
/// call site — kept rather than skipped, so the guard fails loudly naming them
/// instead of passing on what it never read. The scan used to take
/// the next `"` anywhere in the file, so a non-literal argument was read as some
/// unrelated string or dropped, and either way never checked.
fn suggested_in_source(root: &std::path::Path) -> Suggested {
    let mut commands = Vec::new();
    let mut unread = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.extension().is_some_and(|kind| kind == "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            // `crate_sources` strips before splitting on the bare attribute
            // for the same reason this does: a comment naming `#[cfg(test)]`
            // would otherwise move the cut in the raw text.
            let stripped = crate::source_text::code_of(&text);
            let text = shipped_part(&stripped);
            let mut rest = text;
            let mut consumed = 0usize;
            while let Some(at) = rest.find("Resolution::run(") {
                let after = at + "Resolution::run(".len();
                let Some((argument, length)) = resolution_argument(&rest[after..]) else {
                    consumed += after;
                    rest = &rest[after..];
                    continue;
                };
                let line = text[..consumed + at].matches('\n').count() + 1;
                match command_of_argument(&argument) {
                    Some(command) => commands.push((path.display().to_string(), command)),
                    None => unread.push(UnreadArgument {
                        file: path.display().to_string(),
                        line,
                        argument,
                    }),
                }
                consumed += after + length;
                rest = &rest[after + length..];
            }
        }
    }
    Suggested { commands, unread }
}

/// `Resolution::run` arguments the source scan cannot read, each with where the
/// command it builds is checked instead. A new one fails the guard below rather
/// than passing unread.
const UNREAD_ARGUMENTS_COVERED: &[&str] = &[
    // `doctor`'s one command built from the checkout path and the platform.
    "reinstall_command(",
    // `gate_identity`/`goal_identity` interpolate the verb their caller passes;
    // the refusals they build are in the inventory that
    // `every_command_a_rejection_names_parses` walks, and `goal progress`/
    // `blocked` have `the_goal_activity_identity_road_names_a_command_that_parses`.
    "{command} --session-id",
];

#[test]
fn every_command_the_source_suggests_parses_and_not_only_the_listed_ones() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let suggested = suggested_in_source(&root);
    assert!(
        suggested.commands.len() > commands_named().len(),
        "the source scan stopped finding what the hand list already has, so it \
         is checking nothing: {} found",
        suggested.commands.len()
    );

    for (file, command) in suggested.commands {
        // A `format!` hole where the subcommand goes: which verb it is depends
        // on a value, and `estigia claim --help` in the inventory is what
        // exercises that shape.
        if command.contains("estigia {") {
            continue;
        }
        // Everything else has its holes filled with something of the right
        // shape, the way `expand` fills a `<placeholder>`.
        let filled = command
            .replace("{}", "claude-code")
            .replace("{name}", "claude-code")
            .replace("{agent}", "claude-code")
            .replace("{known}", "claude-code")
            .replace("{supported}", "claude-code")
            .replace("{holder}", "claude-abcd1234")
            .replace("{}", "x");
        let argv = argv(&filled);
        if argv.first().is_none_or(|program| program != "estigia") {
            assert!(
                argv.first().is_some_and(|program| on_path(program)),
                "{file} suggests `{command}`, and that program is not here"
            );
            continue;
        }
        for expanded in expand(&argv) {
            if let Err(error) = Cli::try_parse_from(&expanded) {
                use clap::error::ErrorKind;
                assert!(
                    matches!(
                        error.kind(),
                        ErrorKind::DisplayHelp
                            | ErrorKind::DisplayVersion
                            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    ),
                    "{file} suggests `{}`, which this binary cannot parse:\n{error}",
                    expanded.join(" ")
                );
            }
        }
    }

    // A non-literal argument is named rather than skipped, so a new one is
    // covered the day it is written and not the day somebody notices it is
    // missing.
    let uncovered: Vec<String> = suggested
        .unread
        .iter()
        .filter(|unread| {
            !UNREAD_ARGUMENTS_COVERED
                .iter()
                .any(|argument| unread.argument.contains(argument))
        })
        .map(|unread| {
            format!(
                "{}:{} runs `Resolution::run({})`, whose command this scan cannot read",
                unread.file, unread.line, unread.argument
            )
        })
        .collect();
    assert!(
        uncovered.is_empty(),
        "a `Resolution::run` argument this scan cannot read needs a test that builds the check \
         and runs the same bar on its resolution, or the guard is passing on what it never read:\n{}",
        uncovered.join("\n")
    );
}

/// The scan must name a non-literal argument rather than skip it, which is the
/// defect that was filed: `Resolution::run(some_call())` was read as the next `"`
/// in the file, or dropped, and either way never checked.
#[test]
fn the_source_scan_names_a_non_literal_argument_instead_of_skipping_it() {
    let dir = tempfile::tempdir().expect("a temporary tree");
    std::fs::write(
        dir.path().join("sample.rs"),
        "fn f() {\n    let _ = Resolution::run(some_command());\n}\n",
    )
    .expect("a source file");

    let suggested = suggested_in_source(dir.path());
    assert!(
        suggested.commands.is_empty(),
        "a call site with no literal is being read as a command: {:?}",
        suggested.commands
    );
    assert_eq!(
        suggested.unread.len(),
        1,
        "the scan skipped a non-literal argument instead of naming it"
    );
    assert!(
        suggested.unread[0].argument.contains("some_command"),
        "the unread argument is not named: {:?}",
        suggested.unread[0].argument
    );
    assert_eq!(
        suggested.unread[0].line, 2,
        "the named line is not the call site's"
    );
}

/// Lifting a stand-down says so only when the record is actually gone.
///
/// The record is the whole of the loosening: while it is on disk and in force,
/// the gate stands aside and writes go through unadjudicated. `--lift` removed
/// it with the result discarded and then printed *the gate decides on its own
/// again* either way, so a removal that failed was reported as a lift — with
/// exit 0, which is what a script wrapping this reads.
#[test]
fn a_stand_down_that_could_not_be_removed_is_not_reported_as_lifted() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("stand-down.json");

    // Nothing there: a lift with nothing to remove is still a lift, so the
    // error alone must not be the finding.
    lift_record(&file).expect("removing an absent record is not a failure");

    // There, and removable.
    std::fs::write(&file, "{}").expect("a record");
    lift_record(&file).expect("an ordinary lift");
    assert!(!file.exists());

    // There, and not removable: a directory under the record's name is the
    // portable way to make `remove_file` fail while the path stays.
    std::fs::create_dir(&file).expect("something remove_file cannot take");
    let refusal = lift_record(&file).expect_err("a record that is still on disk");
    assert_eq!(refusal.code, "stand-down-not-lifted");
    assert!(
        refusal.outcome.is_clean(),
        "a lift that did not happen reported that it might have"
    );
    assert!(
        refusal.message.contains("still standing down"),
        "the refusal does not say the gate is still loose: {}",
        refusal.message
    );
    assert!(
        file.exists(),
        "the test proved nothing: the path went away by itself"
    );
}

/// keeps reviewing and no operator can ever meet — the review passes, the
#[test]
fn every_code_the_inventory_carries_is_one_the_crate_raises() {
    // Not a refusal code at all: `companion-state` is the wrapper the inventory
    const NOT_A_RAISED_CODE: &str = "companion-state";

    let mut raised: Vec<String> = Vec::new();
    for source in &crate_sources() {
        raised.extend(codes_in(source));
    }
    raised.sort();
    raised.dedup();

    assert!(
        !raised.contains(&NOT_A_RAISED_CODE.to_owned()),
        "{NOT_A_RAISED_CODE} is raised somewhere, so it is a real code and the carve-out below \
         is hiding it from this crossing"
    );

    for refusal in inventory().into_iter().chain(after_writing()) {
        if refusal.code == NOT_A_RAISED_CODE {
            continue;
        }
        assert!(
            raised.contains(&refusal.code.to_owned()),
            "the inventory carries `{}` and nothing in the crate raises it — either the refusal \
             went and the entry stayed, or the walk that reads the tree cannot see where it lives",
            refusal.code
        );
    }

    // Floors. A crossing over an empty side proves nothing, and this one goes
    // quiet in exactly the way it exists to catch.
    assert!(
        raised.len() >= 55,
        "the walk found only {} codes",
        raised.len()
    );
    assert!(
        inventory().len() + after_writing().len() >= 55,
        "the inventory shrank to {}",
        inventory().len() + after_writing().len()
    );
}

///
/// the same broken check back, which is the dead end the ratchet forbids. It sat
#[test]
fn an_entry_names_the_same_kind_of_way_out_as_the_code_that_raises_it() {
    /// The resolution kind written beside a code in the crate's own source.
    fn kind_in_source(code: &str) -> Option<&'static str> {
        for source in &crate_sources() {
            let mut rest = source.as_str();
            while let Some(at) = rest.find(&format!("\"{code}\"")) {
                rest = &rest[at + code.len() + 2..];
                // Only as far as the next refusal: a resolution belongs to the
                // constructor it sits inside.
                let end = rest.find("Refusal::not_started(").unwrap_or(rest.len());
                let mut cap = end.min(900);
                while !rest.is_char_boundary(cap) {
                    cap -= 1;
                }
                let block = &rest[..cap];
                if block.contains("Resolution::run(") {
                    return Some("run");
                }
                if block.contains("Resolution::no_command(") {
                    return Some("no_command");
                }
            }
        }
        None
    }

    let mut compared = 0;
    for refusal in inventory().into_iter().chain(after_writing()) {
        let Some(theirs) = kind_in_source(refusal.code) else {
            continue;
        };
        let ours = match refusal.resolution {
            Resolution::Run { .. } => "run",
            Resolution::NoCommand { .. } => "no_command",
        };
        compared += 1;
        assert_eq!(
            ours, theirs,
            "the inventory reviews `{}` as a `{ours}` and the crate raises it as a `{theirs}` — \
             the ratchet is reading the wrong way out",
            refusal.code
        );
    }

    // A floor, because the comparison goes quiet by finding nothing to compare.
    assert!(
        compared >= 20,
        "only {compared} entries could be crossed against a literal in the source"
    );
}

#[test]
fn setting_a_row_on_the_repository_writes_its_own_file_and_refuses_an_agents_row() {
    // This is also the **only** place that creates the file. Everywhere else
    // keeps it current and never conjures it, because a file made on every
    // install would move every operator's rows out of the contract they are in
    // today. Typing `--repo` is a repository saying *I answer for myself*.
    let repo = tempfile::tempdir().expect("a repository");
    // An isolated home, because the convenience list this writes lives under it:
    // with the machine's own, every run of this test recorded a temporary
    // directory into the developer's real `~/.estigia/repositories`.
    let elsewhere = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(elsewhere.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let path = crate::skill::repository_config_path(repo.path());
    assert!(!path.exists(), "the fixture already has one");

    // A folder that is not a checkout gets no `.git` invented for it. Found by
    // running this for real in a directory that deliberately has none: it made
    // one.
    let refusal = super::repository_set(Setting::Evidence, "measuring", repo.path(), &options)
        .expect_err("a folder that is not a checkout is not a repository");
    assert_eq!(refusal.code, "not-a-repository");
    assert!(
        !repo.path().join(".git").exists(),
        "a `.git` was created where there was no checkout"
    );
    std::fs::create_dir_all(repo.path().join(".git")).expect("now it is one");

    super::repository_set(Setting::Evidence, "measuring", repo.path(), &options)
        .expect("the repository takes it");
    let written = std::fs::read_to_string(&path).expect("the file was created");
    assert!(
        written.contains("measuring"),
        "the answer is not in the file it was written to: {written}"
    );

    // And a second row does not undo the first.
    super::repository_set(Setting::Tracker, "github", repo.path(), &options)
        .expect("the repository takes it");
    let written = std::fs::read_to_string(&path).expect("the file is still there");
    assert!(
        written.contains("measuring"),
        "a second `--repo` undid the first one's row: {written}"
    );

    // A row about an agent is refused at the door rather than written and put
    // back silently.
    let refusal = super::repository_set(Setting::Models, "unset", repo.path(), &options)
        .expect_err("an agent's row is not the repository's to answer");
    assert_eq!(refusal.code, "setting-not-the-repositorys");
}

#[test]
fn setting_a_row_on_the_repository_from_a_subdirectory_writes_the_checkouts_file() {
    let repo = tempfile::tempdir().expect("a repository");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout");
    let deep = repo.path().join("apps").join("engine");
    std::fs::create_dir_all(&deep).expect("a subdirectory");
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };

    super::repository_set(Setting::Evidence, "measuring", &deep, &options)
        .expect("a subdirectory of a checkout is inside it");

    let written = std::fs::read_to_string(crate::skill::repository_config_path(repo.path()))
        .expect("the checkout's own file was written");
    assert!(written.contains("measuring"));
    assert!(!deep.join(".git").exists());
    assert_eq!(
        crate::skill::known_repositories(home.path()),
        vec![repo.path().to_path_buf()]
    );
}

/// The screen writes the checkout it is **pointed at**, not the one it is in.
///
/// The dropdown lets somebody load another checkout's answers without going
/// thing the page that offers the choice must not do.
///
/// Both halves are asserted, because either alone passes for the wrong reason:
/// the checkout named has the value, and the one the process is standing in is
/// untouched.
#[test]
fn the_screen_writes_the_checkout_it_was_pointed_at() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("claude-code is an agent this build knows");

    // Two checkouts that answer for themselves, both saying the same thing, so
    // that whichever one moves is the one the write chose.
    let rows = "| Setting | Value here | Skill default |\n|---|---|---|\n\
                | Integration | trunk | branch |\n";
    let named = tempfile::tempdir().expect("the checkout the page names");
    let standing = tempfile::tempdir().expect("the checkout the process stands in");
    for dir in [named.path(), standing.path()] {
        let file = crate::skill::repository_config_path(dir);
        std::fs::create_dir_all(file.parent().expect("that file has a directory"))
            .expect("the checkout's directory is made");
        std::fs::write(&file, rows).expect("the checkout's rows are written");
    }

    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [("claude-code", Config::default())].into_iter().collect(),
        // The merged view, which is what this checkout's own file is written
        // from — and the value the assertion below looks for.
        rows: [(
            "claude-code",
            Config {
                evidence: crate::config::Evidence::Measuring,
                ..Config::default()
            },
        )]
        .into_iter()
        .collect(),
        repository: named.path().to_path_buf(),
    };
    super::install_planned(&plan, &options, false).expect("the install was refused");

    let read = |dir: &std::path::Path| {
        std::fs::read_to_string(crate::skill::repository_config_path(dir))
            .expect("the checkout's rows are still there")
    };
    assert!(
        read(named.path()).contains("measuring"),
        "the checkout the page named did not get the answer: {}",
        read(named.path())
    );
    assert_eq!(
        read(standing.path()),
        rows,
        "a checkout the page was not pointed at was written into"
    );
}

/// A checkout whose own file cannot be read stops the write; it is not skipped.
///
/// The write asked `if let Ok(Some(..))` and did nothing on the `Err`. So an
/// install into a checkout with an unreadable file reported everything it wrote
#[test]
fn a_checkout_whose_own_file_is_unreadable_stops_the_write() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("claude-code is an agent this build knows");
    let plan = |dir: &std::path::Path| crate::tui::Plan {
        agents: vec![adapter],
        opened: [("claude-code", Config::default())].into_iter().collect(),
        rows: [("claude-code", Config::default())].into_iter().collect(),
        repository: dir.to_path_buf(),
    };

    // The floor: a checkout that answers for itself readably is installed into.
    // A check that refused every install would satisfy the assertion below and
    // take the product with it.
    let readable = tempfile::tempdir().expect("a checkout");
    let file = crate::skill::repository_config_path(readable.path());
    std::fs::create_dir_all(file.parent().expect("that file has a directory"))
        .expect("the checkout's directory is made");
    std::fs::write(
        &file,
        "| Setting | Value here | Skill default |\n|---|---|---|\n",
    )
    .expect("the checkout's rows are written");
    super::install_planned(&plan(readable.path()), &options, false)
        .expect("an install into a readable checkout was refused");

    // And now one whose file cannot be read at all.
    let unreadable = tempfile::tempdir().expect("another checkout");
    std::fs::create_dir_all(crate::skill::repository_config_path(unreadable.path()))
        .expect("something unreadable in its place");
    let refusal = super::install_planned(&plan(unreadable.path()), &options, false)
        .expect_err("the write skipped a checkout it could not read and reported success");
    assert_eq!(refusal.code, "config-local-unreadable");
    assert!(
        refusal
            .message
            .contains(&unreadable.path().display().to_string()),
        "the refusal does not name the checkout it could not read: {refusal}"
    );
}

/// convenience, it is how OpenCode is gated, because that adapter's plugin
/// shells out to it on every edit. They disagreed about the same bytes twice
/// - `cwd` was lifted out of a nested payload by the hook and **discarded** by
///   this door, which always asked the process where it was standing. Which
///   checkout the gate measures against is what tells a write in this run's
///   worktree from one in somebody else's.
#[test]
fn both_doors_to_the_gate_read_one_payload_the_same_way() {
    // The checkout, in the two shapes agents send it in.
    for payload in [
        serde_json::json!({"cwd": "H:/somewhere", "command": "git push"}),
        serde_json::json!({"tool_input": {"cwd": "H:/somewhere"}}),
    ] {
        assert_eq!(
            super::payload_cwd(&payload),
            "H:/somewhere",
            "the checkout this payload names is not the one the gate would measure: {payload}"
        );
    }
    // And the floor: a payload naming none still falls back, rather than
    // resolving to an empty path that names the filesystem root.
    assert_eq!(
        super::payload_cwd(&serde_json::json!({"command": "git push"})),
        "",
        "a payload that names no checkout is inventing one"
    );

    // The spelling, through the door that never reshapes anything. This is what
    // the removed copy was doing for the other door only.
    for spelling in ["command", "command_line", "commandLine"] {
        let payload = serde_json::json!({ spelling: "git push origin HEAD" });
        assert_eq!(
            crate::harness::classify_with("bash", &payload, &[]).0,
            crate::harness::Action::Boundary {
                command: "git push".to_owned(),
                pr: None,
                pr_unidentified_reason: None,
                local_fast_forward_target: None,
                is_exact_integration_merge: false,
                runs_in: crate::harness::RunsIn::Session,
                target: crate::harness::CommitTarget::Checkout,
            },
            "`{spelling}` is a push and this door does not see it"
        );
    }
}

/// Every command the parser has is one the JSON guard poses.
///
/// `--json` is declared once, on the root parser, and honoured in as many
/// be chosen by hand, because a dry run and a harmless id are not something a
/// parser can invent.
#[test]
fn every_command_the_parser_has_is_one_the_json_guard_poses() {
    use clap::CommandFactory;
    let guard = crate::source_text::code_of(include_str!("../../tests/pipe/cli_process.rs"));
    let posed = guard
        .split_once("fn every_command_that_prints_honours_the_global_json_flag")
        .map(|(_, rest)| rest)
        .expect("the guard is still called that");
    let posed = posed
        .split_once("\n}\n")
        .map(|(body, _)| body)
        .unwrap_or(posed);

    let mut checked = 0;
    for command in Cli::command().get_subcommands() {
        if matches!(
            command.get_name(),
            "mcp" | "hook" | "__record-install" | "__review-job" | "__check-job"
        ) {
            continue;
        }
        let leaves: Vec<String> = if command.get_subcommands().next().is_some() {
            command
                .get_subcommands()
                .map(|inner| format!("\"{}\", \"{}\"", command.get_name(), inner.get_name()))
                .collect()
        } else {
            vec![format!("\"{}\"", command.get_name())]
        };
        for leaf in leaves {
            checked += 1;
            // The one that draws a screen. There is no prose for `--json` to
            // be an alternative to, and a test cannot drive a terminal.
            if leaf == "\"config\", \"edit\"" || leaf == "\"tui\"" {
                continue;
            }
            assert!(
                posed.contains(&leaf),
                "`{leaf}` is a command this binary has and the JSON guard never runs it"
            );
        }
    }
    // The floor: a scan that matched nothing would pass in silence.
    assert!(
        checked >= 12,
        "only {checked} commands were read off the parser, so this checked almost nothing"
    );
}

#[test]
fn an_agents_own_table_is_read_back_before_it_is_called_written() {
    // Two branches wrote an edited table and only one read it back. The shared
    // contract checked every row and named the ones `estigia.local.md` still
    // overrode, on the argument written beside it: *`config set` already refuses
    // that for one row; a screen that writes fourteen has fourteen chances to do
    // it.* The branch above wrote an adapter's own file, printed `configuration
    // for <agent> written to …` and returned, having read nothing back — and an
    // adapter's file sits under the same local override, so it has the same
    // fourteen chances.
    //
    // Only the rows that file can carry. An adapter's own file holds
    // `AGENT_SETTINGS`; a repository fact in it is put back from the contract by
    // the reader, so comparing the whole table would report every repository row
    // as overridden and refuse a write that was perfectly good — the fix that
    // would have been worse than the defect.
    let (_home, options) = sandbox();
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(crate::skill::DIRECTORY);
    crate::skill::install(&skill, &Config::default(), false).expect("the skill installs");

    let mut wanted = Config::default();
    Setting::Models
        .apply(&mut wanted, "developer=opus")
        .expect("the value is one it takes");

    // Nothing shadowing it: the write reads back as written.
    let file = crate::skill::agent_override(&skill, "claude-code");
    let snapshot = crate::setup::agent_configuration_snapshot(&file).expect("the file is absent");
    super::write_edited_table(
        &file,
        Some(("claude-code", &snapshot)),
        &Config::default(),
        &wanted,
        &options,
    )
    .expect("a table nothing overrides is written and read back");
    assert_eq!(
        crate::skill::installed_config_for(&skill, Some("claude-code"))
            .expect("the config reads")
            .models,
        wanted.models,
        "the row did not survive the write, so this measures nothing"
    );

    // Now the operator's own file answers that row, and the write is one no run
    // will ever read.
    std::fs::write(
        skill.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |\n|---|---|\n| Model routing | developer=sonnet |\n",
    )
    .expect("the override is written");
    let snapshot = crate::setup::agent_configuration_snapshot(&file).expect("the file reads");
    let refusal = super::write_edited_table(
        &file,
        Some(("claude-code", &snapshot)),
        &Config::default(),
        &wanted,
        &options,
    )
    .expect_err("a row nothing will read was reported as written");
    assert_eq!(refusal.code, "setting-shadowed-by-local-file");
    assert!(
        refusal.message.contains("Model routing"),
        "the refusal does not name the row: {}",
        refusal.message
    );

    // And a repository row is not reported as shadowed by the reader putting it
    // back where it belongs, which is what comparing the whole table would do.
    let mut repository = Config::default();
    Setting::Evidence
        .apply(&mut repository, "measuring")
        .expect("Evidence is accepted");
    std::fs::remove_file(skill.join(crate::config::LOCAL_FILE)).expect("the override goes");
    let snapshot = crate::setup::agent_configuration_snapshot(&file).expect("the file reads");
    super::write_edited_table(
        &file,
        Some(("claude-code", &snapshot)),
        &Config::default(),
        &repository,
        &options,
    )
    .expect("a repository row an agent file cannot carry was called shadowed");
}

#[test]
fn a_shared_screen_save_reaches_every_installed_contract() {
    // `config set` carries a row that is not per-agent into every installed
    // contract; the screen behind `config edit` wrote only the one it was opened
    // on. The two must agree, so this holds the shared screen to the command.
    let (_home, options) = sandbox();
    let claude = crate::setup::find_agent("claude-code").expect("claude-code is an adapter");
    let codex = crate::setup::find_agent("codex").expect("codex is an adapter");
    crate::setup::setup(claude, &Config::default(), &options).expect("claude-code installs");
    crate::setup::setup(codex, &Config::default(), &options).expect("codex installs");

    let root_of = |adapter| {
        crate::setup::resolve_paths(adapter, &options)
            .expect("an adapter resolves its paths")
            .skill_root
    };
    let claude_root = root_of(claude);
    let codex_root = root_of(codex);
    assert_ne!(
        claude_root, codex_root,
        "these two share a skill root, so this test cannot see the defect"
    );

    let installed = Config::default();
    let mut edited = installed.clone();
    Setting::Summary
        .apply(&mut edited, "Español")
        .expect("Summary takes a language");
    Setting::Delivery
        .apply(&mut edited, "ask")
        .expect("Delivery takes an authorisation");
    assert_ne!(
        Setting::Summary.value_of(&installed),
        Setting::Summary.value_of(&edited),
        "the two tables must differ, or this measures nothing"
    );

    super::write_edited_table(
        &claude_root.join(crate::skill::CONTRACT),
        None,
        &installed,
        &edited,
        &options,
    )
    .expect("the shared screen's table is written");

    assert_eq!(
        Setting::Summary.value_of(
            &crate::skill::installed_config(&codex_root).expect("codex's contract reads")
        ),
        Setting::Summary.value_of(&edited),
        "the shared-screen save did not reach codex's own contract"
    );
    // The per-agent row went to the table it was written into, and no further:
    // `Delivery` is not a fact about this machine or this repository.
    assert_eq!(
        Setting::Delivery.value_of(
            &crate::skill::installed_config(&codex_root).expect("codex's contract reads")
        ),
        Setting::Delivery.value_of(&installed),
        "a per-agent row was carried into codex's root, which is not its own"
    );
}

/// An agent whose files another entry of the same run repairs is not current.
///
/// Most adapters have no skill directory of their own and share the neutral
/// root. The run writes it once — correctly — so an adapter sharing it is
/// repaired under a name the operator has no reason to connect to the agent
/// they asked about.
///
/// The floor is the other half: the phrase has to survive where it is true.
#[test]
fn a_shared_skill_repaired_above_is_not_reported_as_already_current() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(setup::Platform::Unix),
        ..SetupOptions::default()
    };
    let neutral = setup::find_agent("agents").expect("the neutral root is an adapter");
    let cursor = setup::find_agent("cursor").expect("cursor is an adapter");
    let config = Config::default();

    // Both installed, sharing one skill root, and then the shared contract is
    // left stale — the ordinary state of a machine one release behind.
    setup::setup(neutral, &config, &options).expect("the neutral root installs");
    setup::setup(cursor, &config, &options).expect("cursor installs");
    let root = setup::resolve_paths(cursor, &options)
        .expect("paths resolve")
        .skill_root;
    let contract = root.join("SKILL.md");
    assert!(
        contract.exists(),
        "the shared contract is where both read it"
    );
    std::fs::write(&contract, "a contract from an older release\n").expect("the stale copy");

    let plan = |options: &SetupOptions| {
        let mut pending = setup::Pending::new();
        let done: Vec<_> = [neutral, cursor]
            .into_iter()
            .map(|adapter| {
                let result = setup::setup_into(adapter, &config, options, &mut pending)
                    .expect("the plan runs");
                (adapter, result)
            })
            .collect();
        super::describe_all(&done, false)
    };

    let dry = SetupOptions {
        dry_run: true,
        ..options.clone()
    };
    let stale = plan(&dry);
    let cursor_section = stale
        .split(cursor.display_name)
        .nth(1)
        .unwrap_or_else(|| panic!("cursor has a section of its own:\n{stale}"));

    // The floor for the fixture itself: the plan does name the broken file, so
    // a run of this test against a contract that was never broken would fail
    // here rather than pass on a report with nothing in it.
    //
    // Any of the three words: the fixture rewrites the contract **by hand**
    assert!(
        (stale.contains("would OVERWRITE")
            || stale.contains("would REPLACE")
            || stale.contains("would update"))
            && stale.contains("SKILL.md"),
        "the stale contract is repaired somewhere in this plan:\n{stale}"
    );
    assert!(
        !cursor_section.contains("already current"),
        "cursor's contract is stale and repaired above, and it was called \
         already current:\n{stale}"
    );
    assert!(
        cursor_section.contains("shares the ones listed above"),
        "cursor is told where its files are dealt with:\n{stale}"
    );

    plan(&options);
    let current = plan(&dry);
    let section = current
        .split(cursor.display_name)
        .nth(1)
        .expect("cursor has a section of its own");
    assert!(
        section.contains("already current"),
        "with the contract current, cursor says so:\n{current}"
    );
}

/// An operator's change size survives the release that follows it.
/// lines` to `300` in `SKILL.md` and `references/repository-delivery.md` got
///
/// Named rather than left to the loop over `SETTINGS` in
/// `sync_moves_the_markdown_forward_and_keeps_the_table`. That loop asserts
/// whatever the list holds, so deleting the row from the list deletes the
/// assertion with it — measured, by doing exactly that: the suite stayed green.

#[test]
fn an_operator_change_size_survives_a_sync() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(setup::Platform::Unix),
        ..SetupOptions::default()
    };
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let mine = Config {
        change_size: 300,
        ..Config::default()
    };
    setup::setup(claude, &mine, &options).expect("setup writes my answer");

    let root = setup::resolve_paths(claude, &options)
        .expect("paths resolve")
        .skill_root;
    assert_eq!(
        crate::skill::installed_config_for(&root, Some(claude.slug))
            .expect("the contract reads")
            .change_size,
        300,
        "the answer never reached the contract"
    );

    // The contract must not restate the number anywhere, or the row and the
    // prose are two places holding one rule and the prose is the stale one.
    let contract = std::fs::read_to_string(root.join("SKILL.md")).expect("the contract");
    let delivery = std::fs::read_to_string(root.join("references").join("repository-delivery.md"))
        .expect("the delivery reference");
    assert!(
        !contract.contains("800 changed lines") && !delivery.contains("800 changed lines"),
        "the shipped prose still names a number the table decides"
    );

    // The release that follows: the same sync an upgrade runs.
    let shared: Vec<Config> = Vec::new();
    let resolved = super::config_for(claude, &shared, &options).expect("the installed table reads");
    let theirs = SetupOptions {
        skip_directive: !setup::is_configured(claude, &options),
        ..options.clone()
    };
    let mut pending = setup::Pending::new();
    setup::setup_into(claude, &resolved, &theirs, &mut pending).expect("sync runs");

    assert_eq!(
        crate::skill::installed_config_for(&root, Some(claude.slug))
            .expect("the contract reads")
            .change_size,
        300,
        "sync put its own number back over the operator's"
    );
}

#[test]
fn every_refusal_the_crate_raises_has_a_row_in_this_file() {
    let reviewed: std::collections::BTreeSet<&str> = inventory()
        .into_iter()
        .chain(after_writing())
        .map(|refusal| refusal.code)
        .collect();

    let mut raised: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for source in &crate_sources() {
        // Both spellings a refusal is built in: the constructor, and the struct
        // literal that hid one of them.
        for opener in [
            "Refusal::not_started(",
            "Refusal::stale(",
            "Refusal::unavailable(",
            "code: ",
        ] {
            let mut rest = source.as_str();
            while let Some(at) = rest.find(opener) {
                rest = &rest[at + opener.len()..];
                let Some(body) = rest.trim_start().strip_prefix('"') else {
                    continue;
                };
                let Some(end) = body.find('"') else { break };
                let code = &body[..end];
                if code.contains('-') && code.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    raised.insert(code.to_owned());
                }
            }
        }
    }

    // The floor: the walk found the crate's refusals. An empty walk agrees with
    // a complete inventory and never fails.
    assert!(
        raised.len() > 50,
        "only {} refusal codes were found in the crate, so this compared almost nothing",
        raised.len()
    );
    let unreviewed: Vec<&String> = raised
        .iter()
        .filter(|code| !reviewed.contains(code.as_str()))
        .collect();
    assert!(
        unreviewed.is_empty(),
        "these refusals can be raised and no row here reviews the way out they name: \
         {unreviewed:#?}"
    );
}

#[test]
fn a_bare_set_cannot_land_beneath_an_agents_own_file() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    super::config_set(
        "Model routing",
        "developer=opus",
        Some("opencode"),
        false,
        false,
        &options,
        false,
    )
    .expect("the per-agent write lands");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("paths")
        .skill_root;
    let overlay = crate::skill::agent_override(&root, "opencode");
    let overlay_before = fs::read(&overlay).expect("the overlay reads");
    let contract = root.join(crate::skill::CONTRACT);
    let contract_before = fs::read(&contract).expect("the contract reads");

    let refusal = super::config_set(
        "Model routing",
        "developer=haiku",
        None,
        false,
        false,
        &options,
        false,
    )
    .expect_err("the shared write was accepted while an agent read otherwise");
    assert_eq!(refusal.code, "setting-needs-an-agent");
    assert!(
        refusal.message.contains("opencode"),
        "the refusal does not name the agent that would read it: {refusal}"
    );
    assert_eq!(
        fs::read(&overlay).expect("the overlay reads"),
        overlay_before
    );
    assert_eq!(
        fs::read(&contract).expect("the contract reads"),
        contract_before
    );
}

#[test]
fn config_writes_never_mutate_the_installed_reviewer() {
    let (home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let before = fs::read(&reviewer).expect("the installed reviewer reads");

    super::config_set(
        "Model routing",
        "judge=opus",
        Some(adapter.slug),
        false,
        false,
        &options,
        false,
    )
    .expect("model routing is written");
    assert_eq!(fs::read(&reviewer).expect("the reviewer remains"), before);

    let writable = super::writable_config(Some(adapter.slug), &options)
        .expect("the installed configuration is writable");
    let mut edited = writable.config.clone();
    Setting::Models
        .apply(&mut edited, "developer=inherit, analyst=inherit")
        .expect("both roles are named");
    let agent_write = writable
        .agent_snapshot
        .as_ref()
        .map(|snapshot| (adapter.slug, snapshot));
    super::write_edited_table(
        &writable.target,
        agent_write,
        &writable.config,
        &edited,
        &options,
    )
    .expect("the edited table is written");
    assert_eq!(fs::read(&reviewer).expect("the reviewer remains"), before);
}

#[test]
fn a_failure_after_setup_never_claims_that_nothing_was_written() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let repaired = root.join("references").join("domain-composition.md");
    std::fs::remove_file(&repaired).expect("one setup file is made stale");
    let mut selected = Config::default();
    Setting::Delivery
        .apply(&mut selected, "ask")
        .expect("Delivery is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterSetup);

    let refusal = super::install_planned(&plan, &options, false)
        .expect_err("the blocked auxiliary write was reported as success");

    assert!(repaired.is_file(), "setup did not land before the failure");
    assert_eq!(refusal.outcome, MutationOutcome::Committed);
    assert_eq!(
        refusal.replay,
        crate::outcome::Replayability::ExactReplaySafe,
        "the idempotent setup batch cannot be retried"
    );
    assert!(
        refusal
            .receipt
            .acknowledged
            .get(adapter.slug)
            .is_some_and(Vec::is_empty),
        "an override that had not run was acknowledged"
    );
}

#[test]
fn a_failure_after_one_override_is_classified_as_partial_and_safe_to_retry() {
    let (_home, options) = sandbox();
    let first = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    let second = crate::setup::find_agent("cursor").expect("Cursor is an adapter");
    for adapter in [first, second] {
        crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    }
    let root = crate::setup::resolve_paths(first, &options)
        .expect("the shared root resolves")
        .skill_root;
    let mut selected = Config::default();
    Setting::Delivery
        .apply(&mut selected, "ask")
        .expect("Delivery is accepted");
    let plan = crate::tui::Plan {
        agents: vec![first, second],
        opened: [
            (first.slug, Config::default()),
            (second.slug, Config::default()),
        ]
        .into_iter()
        .collect(),
        rows: [(first.slug, selected.clone()), (second.slug, selected)]
            .into_iter()
            .collect(),
        repository: std::path::PathBuf::new(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterOverride(1));

    let refusal = super::install_planned(&plan, &options, false)
        .expect_err("the blocked second override was reported as success");

    assert_eq!(
        Setting::Delivery.value_of(
            &crate::skill::installed_config_for(&root, Some(first.slug))
                .expect("the first override reads back")
        ),
        "ask",
        "the first override did not land before the second failed"
    );
    assert_eq!(refusal.outcome, MutationOutcome::Committed);
    assert_eq!(
        refusal.replay,
        crate::outcome::Replayability::ExactReplaySafe,
        "the idempotent local batch cannot be retried after the obstacle is removed"
    );
    assert!(
        refusal
            .receipt
            .acknowledged
            .get(first.slug)
            .is_some_and(|settings| settings.contains(&Setting::Delivery))
            && refusal
                .receipt
                .acknowledged
                .get(second.slug)
                .is_some_and(|settings| !settings.contains(&Setting::Delivery)),
        "partial evidence did not distinguish the written override from the unwritten one"
    );
}

#[test]
fn a_repository_write_failure_keeps_agent_evidence_and_repository_scope_dirty() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        crate::config::EVERYWHERE_SETTINGS,
    )
    .expect("the repository layer exists");
    let repository_before =
        std::fs::read(&repository_path).expect("the repository layer reads before the failure");
    let mut selected = Config::default();
    Setting::Delivery
        .apply(&mut selected, "ask")
        .expect("Delivery is accepted");
    let mut effective = selected.clone();
    Setting::Evidence
        .apply(&mut effective, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, effective)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AtRepository);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("the injected repository failure was reported as success");

    assert_eq!(failure.outcome, MutationOutcome::Committed);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::ExactReplaySafe
    );
    assert!(
        failure
            .receipt
            .acknowledged
            .get(adapter.slug)
            .is_some_and(|settings| settings.contains(&Setting::Delivery)),
        "the proven agent override was discarded with the repository failure"
    );
    assert!(
        failure.receipt.repository.is_none()
            && failure
                .receipt
                .acknowledged
                .get(adapter.slug)
                .is_some_and(|settings| !settings.contains(&Setting::Integration)),
        "the unwritten repository scope was acknowledged"
    );
    assert_eq!(
        std::fs::read(repository_path).expect("the repository layer still reads"),
        repository_before,
        "the failed repository write changed bytes it did not confirm"
    );
}

#[test]
fn an_unconfirmed_repository_attempt_is_unknown_when_nothing_earlier_moved() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        crate::config::EVERYWHERE_SETTINGS,
    )
    .expect("the repository layer exists");
    let mut effective = Config::default();
    Setting::Evidence
        .apply(&mut effective, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, effective)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AtRepository);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("the injected repository failure was reported as success");

    assert_eq!(failure.outcome, MutationOutcome::Unknown);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert!(failure.receipt.repository.is_none());
}

#[test]
fn an_unrelated_tui_save_keeps_a_one_row_repository_override_one_row_owned() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let mut repository_config = Config::default();
    Setting::Tracker
        .apply(&mut repository_config, "github acme/issues")
        .expect("the tracker is accepted");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &repository_config,
        &[Setting::Tracker],
    )
    .expect("the one-row repository layer exists");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter table reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut selected = opened.clone();
    Setting::Delivery
        .apply(&mut selected, "ask")
        .expect("Delivery authorisation is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened)].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };

    super::install_planned(&plan, &options, false).expect("the unrelated save lands");

    let written = std::fs::read_to_string(repository_path).expect("the repository layer reads");
    assert_eq!(
        crate::config::table_rows(&written)
            .into_iter()
            .map(|(label, _)| label)
            .collect::<Vec<_>>(),
        vec![Setting::Tracker.label()],
        "an unrelated save widened the rows this repository owns:\n{written}"
    );
}

#[test]
fn a_tui_save_unions_only_the_repository_row_changed_in_this_session() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let mut repository_config = Config::default();
    Setting::Evidence
        .apply(&mut repository_config, "measuring")
        .expect("Evidence is accepted");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &repository_config,
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter table reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut selected = opened.clone();
    Setting::Integration
        .apply(&mut selected, "trunk")
        .expect("the integration route is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened)].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };

    super::install_planned(&plan, &options, false).expect("the repository save lands");

    let written = std::fs::read_to_string(repository_path).expect("the repository layer reads");
    let rows = crate::config::table_rows(&written)
        .into_iter()
        .map(|(label, _)| label)
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![Setting::Integration.label(), Setting::Evidence.label()],
        "the save either lost the owned row or claimed inherited Tracker/Worktree/Window rows:\n{written}"
    );
}

#[test]
fn a_one_row_repository_receipt_updates_only_that_row_across_divergent_baselines() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let claude = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let codex = crate::setup::find_agent("codex").expect("Codex is an adapter");
    let mut claude_config = Config::default();
    Setting::Tracker
        .apply(&mut claude_config, "github other/issues")
        .expect("the Claude tracker is accepted");
    let mut codex_config = Config::default();
    Setting::Tracker
        .apply(&mut codex_config, "github acme/issues")
        .expect("the Codex tracker is accepted");
    crate::setup::setup(claude, &claude_config, &options).expect("Claude installs");
    crate::setup::setup(codex, &codex_config, &options).expect("Codex installs");

    let mut repository_config = Config::default();
    Setting::Evidence
        .apply(&mut repository_config, "measuring")
        .expect("Evidence is accepted");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &repository_config,
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let layer = |adapter: &'static crate::setup::AgentAdapter| {
        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("the adapter resolves")
            .skill_root;
        let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
            .expect("the adapter baseline reads");
        crate::skill::layer_repository(&bare, repository.path())
            .expect("the repository layer reads")
    };
    let claude_opened = layer(claude);
    let codex_opened = layer(codex);
    let mut selected = claude_opened.clone();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let mut before_the_edit = claude_opened.clone();
    Setting::Evidence
        .apply(
            &mut before_the_edit,
            &Setting::Evidence.value_of(&Config::default()),
        )
        .expect("the default Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![claude],
        opened: [(claude.slug, before_the_edit)].into_iter().collect(),
        rows: [(claude.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };

    let (receipt, _) =
        super::install_planned(&plan, &options, false).expect("the one-row repository save lands");

    assert_eq!(receipt.repository_settings, vec![Setting::Evidence]);
    let mut app = crate::tui::app::App::with_configs(
        &std::collections::BTreeMap::from([
            (claude.slug, claude_opened),
            (codex.slug, codex_opened),
        ]),
        &[],
    );
    app.installed_now(receipt);
    assert_eq!(
        Setting::Evidence.value_of(&app.installed[claude.slug]),
        "measuring"
    );
    assert_eq!(
        Setting::Evidence.value_of(&app.installed[codex.slug]),
        "measuring"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[claude.slug]),
        "github other/issues"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[codex.slug]),
        "github acme/issues",
        "the receipt copied Claude's inherited Tracker onto Codex"
    );
}

#[test]
fn repository_receipt_values_and_ownership_share_one_disk_snapshot() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let replacement = tempfile::tempdir().expect("a replacement repository");
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("Claude installs");
    let mut first = Config::default();
    Setting::Evidence
        .apply(&mut first, "measuring")
        .expect("Evidence is accepted");
    crate::setup::write_repository_configuration(
        &crate::skill::repository_config_path(repository.path()),
        &first,
        &[Setting::Evidence],
    )
    .expect("the first repository snapshot exists");
    let mut second = Config::default();
    Setting::Tracker
        .apply(&mut second, "github acme/repo")
        .expect("the replacement tracker is accepted");
    let replacement_path = crate::skill::repository_config_path(replacement.path());
    crate::setup::write_repository_configuration(&replacement_path, &second, &[Setting::Tracker])
        .expect("the replacement snapshot exists");
    let replacement_document =
        std::fs::read_to_string(replacement_path).expect("the replacement snapshot reads");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter baseline reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the first repository snapshot layers");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened.clone())].into_iter().collect(),
        rows: [(adapter.slug, opened)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    crate::skill::inject_repository_snapshot_replacement(replacement_document);

    let receipt = super::receipt_from_disk(
        &plan,
        &options,
        "repository proved".to_owned(),
        true,
        [adapter.slug],
    )
    .expect("the receipt reads one repository snapshot");

    assert_eq!(receipt.repository_settings, vec![Setting::Evidence]);
    assert_eq!(
        Setting::Evidence.value_of(receipt.repository.as_ref().expect("repository evidence")),
        "measuring"
    );
}

#[test]
fn a_failure_after_proven_repository_write_carries_only_that_repository_row() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let claude = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let codex = crate::setup::find_agent("codex").expect("Codex is an adapter");
    let mut claude_config = Config::default();
    Setting::Tracker
        .apply(&mut claude_config, "github other/issues")
        .expect("the Claude tracker is accepted");
    let mut codex_config = Config::default();
    Setting::Tracker
        .apply(&mut codex_config, "github acme/issues")
        .expect("the Codex tracker is accepted");
    crate::setup::setup(claude, &claude_config, &options).expect("Claude installs");
    crate::setup::setup(codex, &codex_config, &options).expect("Codex installs");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let layer = |adapter: &'static crate::setup::AgentAdapter| {
        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("the adapter resolves")
            .skill_root;
        let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
            .expect("the adapter baseline reads");
        crate::skill::layer_repository(&bare, repository.path())
            .expect("the repository layer reads")
    };
    let claude_opened = layer(claude);
    let codex_opened = layer(codex);
    let mut selected = claude_opened.clone();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![claude],
        opened: [(claude.slug, claude_opened.clone())].into_iter().collect(),
        rows: [(claude.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterRepository);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("the injected post-repository failure was reported as success");

    assert_eq!(failure.receipt.repository_settings, vec![Setting::Evidence]);
    let mut app = crate::tui::app::App::with_configs(
        &std::collections::BTreeMap::from([
            (claude.slug, claude_opened),
            (codex.slug, codex_opened),
        ]),
        &[],
    );
    app.installed_partially(*failure.receipt);
    assert_eq!(
        Setting::Evidence.value_of(&app.installed[claude.slug]),
        "measuring"
    );
    assert_eq!(
        Setting::Evidence.value_of(&app.installed[codex.slug]),
        "measuring"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[claude.slug]),
        "github other/issues"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[codex.slug]),
        "github acme/issues",
        "the partial receipt copied Claude's inherited Tracker onto Codex"
    );
}

#[test]
fn a_proven_repository_write_that_disappears_before_readback_is_unknown() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("Claude installs");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter baseline reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut selected = opened.clone();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened)].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::RemoveRepositoryAfterWrite);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("missing proven repository readback was accepted");

    assert_eq!(failure.outcome, MutationOutcome::Unknown);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert_eq!(failure.code, "repository-readback-missing");
    assert!(failure.receipt.repository.is_none());
    assert!(failure.receipt.repository_settings.is_empty());
}

#[test]
fn a_proven_repository_write_that_is_unreadable_at_readback_is_unknown() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("Claude installs");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter baseline reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut selected = opened.clone();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened)].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::CorruptRepositoryAfterWrite);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("unreadable proven repository readback was accepted");

    assert_eq!(failure.code, "config-local-unreadable");
    assert_eq!(failure.outcome, MutationOutcome::Unknown);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert!(failure.receipt.repository.is_none());
    assert!(failure.receipt.repository_settings.is_empty());
    assert!(
        failure
            .receipt
            .acknowledged
            .values()
            .all(|settings| !settings.contains(&Setting::Integration)),
        "unreadable readback produced row-level repository evidence"
    );
}

#[test]
fn after_repository_failure_keeps_missing_readback_as_the_controlling_unknown() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("Claude installs");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter baseline reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut selected = opened.clone();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened)].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterRepository);
    super::inject_repository_readback_removal();

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("missing partial repository readback was accepted");

    assert_eq!(failure.outcome, MutationOutcome::Unknown);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert_eq!(failure.code, "repository-readback-missing");
    assert!(failure.receipt.repository.is_none());
    assert!(failure.receipt.repository_settings.is_empty());
    assert!(
        failure
            .receipt
            .acknowledged
            .values()
            .all(|settings| !settings.contains(&Setting::Integration)),
        "missing partial readback produced row-level repository evidence"
    );
}

#[test]
fn after_repository_failure_keeps_unreadable_readback_as_the_controlling_unknown() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("Claude installs");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        &[Setting::Evidence],
    )
    .expect("the one-row repository layer exists");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude resolves")
        .skill_root;
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the adapter baseline reads");
    let opened = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut selected = opened.clone();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, opened)].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterRepository);
    super::inject_repository_readback_corruption();

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("unreadable partial repository readback was accepted");

    assert_eq!(failure.code, "config-local-unreadable");
    assert_eq!(failure.outcome, MutationOutcome::Unknown);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert!(
        failure
            .message
            .contains("injected failure at AfterRepository"),
        "the earlier failure was discarded: {failure}"
    );
    assert!(failure.receipt.repository.is_none());
    assert!(failure.receipt.repository_settings.is_empty());
    assert!(
        failure
            .receipt
            .acknowledged
            .values()
            .all(|settings| !settings.contains(&Setting::Integration)),
        "unreadable partial readback produced row-level repository evidence"
    );
}

#[test]
fn a_first_adapter_late_setup_failure_carries_the_writes_that_already_landed() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::inject_setup_failure(adapter.slug, crate::setup::SetupFailureBoundary::AtMcp);
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, Config::default())].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    };

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("the malformed MCP document was reported as success");

    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude Code resolves")
        .skill_root;
    assert!(root.join(crate::skill::CONTRACT).is_file());
    assert_eq!(failure.outcome, MutationOutcome::Committed);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::ExactReplaySafe
    );
    assert!(failure.receipt.completed.is_empty());
}

#[test]
fn command_line_setup_continues_after_a_partly_written_adapter() {
    let (_home, options) = sandbox();
    let claude = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let opencode = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::inject_setup_failure(claude.slug, crate::setup::SetupFailureBoundary::AfterSkill);

    let refusal = super::run_setup_over(&[claude, opencode], &[], &options, false)
        .expect_err("the failed Claude setup was hidden");

    assert_eq!(refusal.outcome, MutationOutcome::Committed);
    assert!(
        crate::setup::is_configured(opencode, &options),
        "the failed adapter stopped setup before OpenCode"
    );
    assert!(
        !crate::setup::is_configured(claude, &options),
        "partial action evidence marked Claude lifecycle-complete"
    );
}

fn unknown_refusal(code: &'static str) -> Refusal {
    Refusal {
        code,
        message: "the write may have landed".to_owned(),
        outcome: MutationOutcome::Unknown,
        class: crate::outcome::RefusalClass::Unknown,
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "read back the uncertain file before deciding whether to retry",
        ),
        observed: None,
    }
}

#[test]
fn a_later_unknown_controls_identity_after_an_earlier_preflight_refusal() {
    let preflight = Refusal::not_started(
        "early-preflight",
        "the earlier adapter was not usable",
        Resolution::run("estigia config list"),
    );
    let unknown = unknown_refusal("uncertain-write");

    let aggregate =
        super::refusal_over(&[], vec![preflight, unknown.clone()]).expect("the batch refused");

    assert_eq!(aggregate.code, unknown.code);
    assert_eq!(aggregate.resolution, unknown.resolution);
    assert_eq!(aggregate.outcome, MutationOutcome::Unknown);
    assert!(
        aggregate
            .message
            .contains("the earlier adapter was not usable")
            && aggregate.message.contains("the write may have landed"),
        "the controlling identity discarded batch context: {}",
        aggregate.message
    );
}

#[test]
fn a_later_unknown_controls_identity_after_an_earlier_committed_refusal() {
    let committed = Refusal {
        code: "earlier-committed",
        message: "an earlier adapter changed a file before refusing".to_owned(),
        outcome: MutationOutcome::Committed,
        class: crate::harness::refusal_class::class_of_code("earlier-committed"),
        replay: crate::outcome::Replayability::ExactReplaySafe,
        resolution: Resolution::run("estigia setup opencode"),
        observed: None,
    };
    let unknown = unknown_refusal("later-uncertain-write");

    let aggregate =
        super::refusal_over(&[], vec![committed, unknown.clone()]).expect("the batch refused");

    assert_eq!(aggregate.code, unknown.code);
    assert_eq!(aggregate.resolution, unknown.resolution);
    assert_eq!(aggregate.outcome, MutationOutcome::Unknown);
    assert!(
        aggregate
            .message
            .contains("an earlier adapter changed a file before refusing")
            && aggregate.message.contains("the write may have landed"),
        "the controlling identity discarded batch context: {}",
        aggregate.message
    );
}

#[test]
fn an_unconfirmed_adapter_write_stays_unknown_when_a_later_adapter_completes() {
    let (_home, options) = sandbox();
    let claude = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let opencode = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(claude, &Config::default(), &options).expect("Claude starts current");
    crate::setup::inject_setup_failure(claude.slug, crate::setup::SetupFailureBoundary::AtMcp);

    let refusal = super::run_setup_over(&[claude, opencode], &[], &options, false)
        .expect_err("the unconfirmed Claude write was hidden");

    assert_eq!(refusal.outcome, MutationOutcome::Unknown);
    assert_eq!(
        refusal.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert!(
        crate::setup::is_configured(opencode, &options),
        "preserving uncertainty stopped the unaffected adapter"
    );
}

#[test]
fn each_proven_setup_boundary_is_committed_and_exactly_replay_safe() {
    crate::setup::with_preview(|| {
        for boundary in [
            crate::setup::SetupFailureBoundary::AfterSkill,
            crate::setup::SetupFailureBoundary::AfterDirective,
            crate::setup::SetupFailureBoundary::AfterDefinitions,
            crate::setup::SetupFailureBoundary::AfterHooks,
        ] {
            let (_home, options) = sandbox();
            let adapter =
                crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
            let mut selected = Config::default();
            Setting::Delivery
                .apply(&mut selected, "ask")
                .expect("Delivery authorisation is accepted");
            let plan = crate::tui::Plan {
                agents: vec![adapter],
                opened: [(adapter.slug, Config::default())].into_iter().collect(),
                rows: [(adapter.slug, selected)].into_iter().collect(),
                repository: std::path::PathBuf::new(),
            };
            crate::setup::inject_setup_failure(adapter.slug, boundary);

            let failure = super::install_planned(&plan, &options, false)
                .expect_err("the injected boundary was reported as success");

            assert_eq!(
                failure.outcome,
                MutationOutcome::Committed,
                "{boundary:?} discarded proven writes"
            );
            assert_eq!(
                failure.replay,
                crate::outcome::Replayability::ExactReplaySafe,
                "{boundary:?} made an idempotent partial setup unsafe to retry"
            );
            assert!(failure.receipt.completed.is_empty());
            super::install_planned(&plan, &options, false)
                .unwrap_or_else(|retry| panic!("{boundary:?} did not retry cleanly: {retry}"));
        }
    });
}

#[test]
fn a_deterministic_late_render_failure_is_prevalidated_before_any_write() {
    crate::setup::with_preview(|| {
        let (home, options) = sandbox();
        let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
        std::fs::write(home.path().join(".claude.json"), "not json")
            .expect("the malformed MCP document exists");
        let plan = crate::tui::Plan {
            agents: vec![adapter],
            opened: [(adapter.slug, Config::default())].into_iter().collect(),
            rows: [(adapter.slug, Config::default())].into_iter().collect(),
            repository: std::path::PathBuf::new(),
        };

        let mut pending = crate::setup::Pending::new();
        let setup_failure = crate::setup::setup_adapter_into(
            adapter,
            &Config::default(),
            &Config::default(),
            &options,
            &mut pending,
            true,
        )
        .expect_err("the malformed MCP document passed prevalidation");
        assert_eq!(
            setup_failure.phase,
            crate::setup::SetupFailurePhase::Prevalidation
        );
        assert!(
            !setup_failure.result.dry_run,
            "a real setup became a dry-run report"
        );
        assert!(
            setup_failure.result.actions.is_empty(),
            "planned preview actions escaped as real evidence"
        );
        assert!(
            !setup_failure.write_attempted,
            "prevalidation claimed a write was attempted"
        );
        let refusal = super::setup_failure_refusal(adapter, &setup_failure);
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        let report = super::describe(
            adapter,
            &setup_failure.result,
            &std::collections::BTreeSet::new(),
            false,
        );
        assert!(
            report.contains("setup stopped before changing any files")
                && !report.contains("would change"),
            "a real prevalidation refusal rendered as a preview:\n{report}"
        );

        let failure = super::install_planned(&plan, &options, false)
            .expect_err("the malformed MCP document was reported as success");

        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("Claude Code resolves")
            .skill_root;
        assert_eq!(failure.outcome, MutationOutcome::NotStarted);
        assert!(
            !root.exists(),
            "prevalidation found the late render failure only after writing the skill"
        );
    });
}

#[test]
fn an_after_skill_failure_in_a_real_invocations_prevalidation_is_not_mutation_evidence() {
    crate::setup::with_preview(|| {
        let (home, options) = sandbox();
        let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
        crate::setup::inject_setup_prevalidation_failure(
            adapter.slug,
            crate::setup::SetupFailureBoundary::AfterSkill,
        );
        let mut pending = crate::setup::Pending::new();

        let failure = crate::setup::setup_adapter_into(
            adapter,
            &Config::default(),
            &Config::default(),
            &options,
            &mut pending,
            true,
        )
        .expect_err("the injected real prevalidation failure was reported as success");

        assert_eq!(
            failure.phase,
            crate::setup::SetupFailurePhase::Prevalidation
        );
        assert!(!failure.result.dry_run);
        assert!(
            failure.result.actions.is_empty(),
            "planned AfterSkill actions escaped as mutation evidence"
        );
        assert!(!failure.write_attempted);
        let refusal = super::classified_setup_failure(adapter, &failure);
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        assert_eq!(
            refusal.replay,
            crate::outcome::Replayability::ExactReplaySafe
        );
        assert!(!format!("{refusal}").contains("write access"));
        let report = super::describe(
            adapter,
            &failure.result,
            &std::collections::BTreeSet::new(),
            false,
        );
        assert!(
            report.contains("setup stopped before changing any files")
                && !report.contains("would change"),
            "a real prevalidation refusal rendered planned action wording:\n{report}"
        );
        assert!(
            std::fs::read_dir(home.path())
                .expect("the sandbox home exists")
                .next()
                .is_none(),
            "the failed real prevalidation changed bytes"
        );
    });
}

#[test]
fn a_retired_host_s_appdata_substitution_is_named_in_the_per_adapter_report() {
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    crate::test_env::with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let adapter = crate::setup::find_agent("gemini-cli").expect("gemini-cli is an adapter");
        let options = crate::setup::SetupOptions {
            platform: Some(crate::setup::Platform::Windows),
            ..crate::setup::SetupOptions::default()
        };
        // blaming the fixture instead of the assertion below with the note
        // that went missing.
        crate::test_env::assert_inherited_roots_escape(home.path());
        let paths = crate::setup::resolve_paths(adapter, &options).expect("the paths resolve");
        let result = crate::setup::SetupResult {
            agent: adapter.slug,
            dry_run: true,
            actions: Vec::new(),
            completed: true,
            notes: paths.notes,
        };
        let report = super::describe(adapter, &result, &std::collections::BTreeSet::new(), false);
        assert!(
            report.contains("APPDATA"),
            "the per-adapter report never names the substituted variable:\n{report}"
        );
    });
}

#[test]
fn a_setup_that_stopped_before_any_file_still_names_a_retired_host_s_appdata_substitution() {
    // The `!dry_run && !completed && changed == 0` early return in `describe`
    // has its own note loop, said to matter because "the substitution
    // happens before the first file is touched" — but the only test that took
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    crate::test_env::with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let adapter = crate::setup::find_agent("gemini-cli").expect("gemini-cli is an adapter");
        let options = crate::setup::SetupOptions {
            platform: Some(crate::setup::Platform::Windows),
            ..crate::setup::SetupOptions::default()
        };
        crate::test_env::assert_inherited_roots_escape(home.path());
        let paths = crate::setup::resolve_paths(adapter, &options).expect("the paths resolve");
        let result = crate::setup::SetupResult {
            agent: adapter.slug,
            dry_run: false,
            actions: Vec::new(),
            completed: false,
            notes: paths.notes,
        };
        let report = super::describe(adapter, &result, &std::collections::BTreeSet::new(), false);
        assert!(
            report.contains("setup stopped before changing any files"),
            "this test's own fixture is not exercising the early-return branch it means to \
             cover:\n{report}"
        );
        assert!(
            report.contains("APPDATA"),
            "a setup that stopped before touching a file still owes the operator the \
             substitution note that explains why the resolved paths are not what the machine \
             configured:\n{report}"
        );
    });
}

#[test]
fn malformed_json_keeps_its_typed_not_editable_prevalidation_refusal() {
    crate::setup::with_preview(|| {
        let (home, options) = sandbox();
        let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
        std::fs::write(home.path().join(".claude.json"), "not json")
            .expect("the malformed MCP document exists");
        let mut pending = crate::setup::Pending::new();

        let failure =
            crate::setup::setup_into_evidenced(adapter, &Config::default(), &options, &mut pending)
                .expect_err("the malformed MCP document passed prevalidation");
        let refusal = super::setup_failure_refusal(adapter, &failure);

        assert_eq!(
            failure.phase,
            crate::setup::SetupFailurePhase::Prevalidation
        );
        assert_eq!(refusal.code, "agent-file-not-editable");
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
    });
}

#[test]
fn an_unreadable_prevalidation_input_is_not_classified_as_a_write_failure() {
    crate::setup::with_preview(|| {
        let (home, options) = sandbox();
        let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
        let hooks = home.path().join(".claude").join("settings.json");
        std::fs::create_dir_all(&hooks).expect("the unreadable path is a directory");
        let mut pending = crate::setup::Pending::new();

        let failure =
            crate::setup::setup_into_evidenced(adapter, &Config::default(), &options, &mut pending)
                .expect_err("the unreadable prevalidation path was reported as success");
        let refusal = super::setup_failure_refusal(adapter, &failure);

        assert_eq!(
            failure.phase,
            crate::setup::SetupFailurePhase::Prevalidation
        );
        assert_eq!(refusal.code, "setup-prevalidation-failed");
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        assert_eq!(
            refusal.replay,
            crate::outcome::Replayability::ExactReplaySafe
        );
        assert!(
            !format!("{refusal}").contains("write access"),
            "prevalidation failure asked for write access: {refusal}"
        );

        let plan = crate::tui::Plan {
            agents: vec![adapter],
            opened: [(adapter.slug, Config::default())].into_iter().collect(),
            rows: [(adapter.slug, Config::default())].into_iter().collect(),
            repository: std::path::PathBuf::new(),
        };
        let classified = super::install_planned(&plan, &options, false)
            .expect_err("the unreadable real invocation was reported as success");
        assert_eq!(classified.code, "setup-prevalidation-failed");
        assert_eq!(classified.outcome, MutationOutcome::NotStarted);
    });
}

#[test]
fn an_unconfirmed_setup_write_without_an_earlier_change_is_unknown() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let mut edited = Config::default();
    Setting::IssueRequirement
        .apply(&mut edited, "required")
        .expect("IssueRequirement is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, edited)].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    };
    crate::setup::inject_setup_failure(adapter.slug, crate::setup::SetupFailureBoundary::AtMcp);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("the unconfirmed MCP attempt was reported as success");

    assert_eq!(failure.outcome, MutationOutcome::Unknown);
    assert_eq!(
        failure.replay,
        crate::outcome::Replayability::StatusRequired
    );
}

#[test]
fn a_failure_after_setup_planning_in_dry_run_is_not_mutation_evidence() {
    let (home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    let dry = SetupOptions {
        dry_run: true,
        ..options
    };
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, Config::default())].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterSetup);

    let failure = super::install_planned(&plan, &dry, false)
        .expect_err("the injected preview failure was reported as success");

    assert_eq!(failure.outcome, MutationOutcome::NotStarted);
    assert!(
        std::fs::read_dir(home.path())
            .expect("the sandbox home exists")
            .next()
            .is_none(),
        "a failed setup preview changed bytes"
    );
}

#[test]
fn an_after_skill_preview_failure_is_not_promoted_by_planned_actions() {
    let (home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let dry = SetupOptions {
        dry_run: true,
        ..options
    };
    crate::setup::inject_setup_failure(
        adapter.slug,
        crate::setup::SetupFailureBoundary::AfterSkill,
    );

    let refusal = super::run_setup_over(&[adapter], &[], &dry, false)
        .expect_err("the injected preview failure was reported as success");

    assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
    assert_eq!(
        refusal.replay,
        crate::outcome::Replayability::ExactReplaySafe
    );
    assert_eq!(refusal.code, "setup-prevalidation-failed");
    let rendered = format!("{refusal}");
    assert!(rendered.contains("input path or configuration"));
    assert!(!rendered.contains("write access"));
    assert!(
        std::fs::read_dir(home.path())
            .expect("the sandbox home exists")
            .next()
            .is_none(),
        "the failed preview changed bytes"
    );

    let planned = crate::setup::SetupFailure {
        error: anyhow::anyhow!("injected preview failure after planning a file"),
        result: crate::setup::SetupResult {
            agent: adapter.slug,
            dry_run: true,
            actions: vec![crate::setup::SetupAction {
                kind: crate::setup::ActionKind::Skill,
                path: home.path().join("planned-only"),
                change: crate::skill::Change::Create,
            }],
            completed: false,
            notes: Vec::new(),
        },
        write_attempted: false,
        phase: crate::setup::SetupFailurePhase::Preview,
    };
    let classified = super::classified_setup_failure(adapter, &planned);
    assert_eq!(classified.outcome, MutationOutcome::NotStarted);
    assert_eq!(
        classified.replay,
        crate::outcome::Replayability::ExactReplaySafe
    );
}

#[test]
fn a_failure_at_repository_planning_in_dry_run_is_not_a_write_attempt() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        &[Setting::Evidence],
    )
    .expect("the repository layer exists");
    let before = std::fs::read(&repository_path).expect("the repository layer reads");
    let mut selected = Config::default();
    Setting::Evidence
        .apply(&mut selected, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    let dry = SetupOptions {
        dry_run: true,
        ..options
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterRepository);

    let failure = super::install_planned(&plan, &dry, false)
        .expect_err("the injected repository preview failure was reported as success");

    assert_eq!(failure.outcome, MutationOutcome::NotStarted);
    assert_eq!(
        std::fs::read(repository_path).expect("the repository layer still reads"),
        before,
        "a failed repository preview changed bytes"
    );
}

#[test]
fn interactive_dry_run_and_real_run_share_one_complete_unique_manifest() {
    let (_home, options) = sandbox();
    let repository = tempfile::tempdir().expect("a repository");
    let first = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    let second = crate::setup::find_agent("cursor").expect("Cursor is an adapter");
    for adapter in [first, second] {
        crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    }
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        crate::config::EVERYWHERE_SETTINGS,
    )
    .expect("the repository layer exists");
    let root = crate::setup::resolve_paths(first, &options)
        .expect("the shared root resolves")
        .skill_root;
    let mut first_config = Config::default();
    Setting::Delivery
        .apply(&mut first_config, "ask")
        .expect("Delivery is accepted");
    let mut second_config = Config::default();
    Setting::Transitions
        .apply(&mut second_config, "ask")
        .expect("Transitions is accepted");
    let mut repository_config = first_config.clone();
    Setting::Evidence
        .apply(&mut repository_config, "measuring")
        .expect("Evidence is accepted");
    let plan = crate::tui::Plan {
        agents: vec![first, second],
        opened: [
            (first.slug, Config::default()),
            (second.slug, Config::default()),
        ]
        .into_iter()
        .collect(),
        rows: [
            (first.slug, repository_config),
            (second.slug, second_config),
        ]
        .into_iter()
        .collect(),
        repository: repository.path().to_path_buf(),
    };
    let dry = SetupOptions {
        dry_run: true,
        ..options.clone()
    };

    let (dry_receipt, dry_json) =
        super::install_planned(&plan, &dry, true).expect("the dry-run plans");
    let (real_receipt, real_json) =
        super::install_planned(&plan, &options, true).expect("the real run lands");
    let manifest = |json: &str| {
        let value: serde_json::Value = serde_json::from_str(json).expect("a JSON setup report");
        value
            .as_array()
            .expect("one result per agent")
            .iter()
            .flat_map(|result| result["actions"].as_array().expect("an action list").iter())
            .filter_map(|action| {
                let change = action["change"].as_str()?;
                (!matches!(change, "unchanged" | "kept" | "shared" | "unrecorded")).then(|| {
                    (
                        action["path"].as_str().unwrap_or_default().to_owned(),
                        change.to_owned(),
                    )
                })
            })
            .collect::<std::collections::BTreeSet<_>>()
    };
    let dry_manifest = manifest(&dry_json);
    let real_manifest = manifest(&real_json);

    for required in [
        crate::skill::agent_override(&root, first.slug),
        crate::skill::agent_override(&root, second.slug),
        repository_path,
    ] {
        assert!(
            dry_manifest
                .iter()
                .any(|(path, _)| path == &required.display().to_string()),
            "the interactive preview omitted {}: {dry_json}",
            required.display()
        );
    }
    assert_eq!(
        dry_manifest, real_manifest,
        "the preview and act disagree on unique path/change pairs"
    );
    let expected_count = dry_manifest.len();
    assert!(
        dry_receipt.summary.ends_with(&format!(
            "{expected_count} file{}",
            if expected_count == 1 { "" } else { "s" }
        )) && real_receipt.summary.ends_with(&format!(
            "{expected_count} file{}",
            if expected_count == 1 { "" } else { "s" }
        )),
        "the summaries do not count the unique manifest: dry={:?}, real={:?}",
        dry_receipt.summary,
        real_receipt.summary
    );
}

/// A session id derives the run id the hook would mint for the same session,
/// and supersedes the ambient fallback rather than colliding with it.
///
/// The host sends the session id and the crate composes the id, so a plugin
/// cannot invent a second derivation; the crossing is against `session::run_id`
/// itself rather than against a spelling.
#[test]
fn a_session_id_derives_the_run_id_the_hook_mints() {
    let session = "ses_0123456789abcdef0123456789abcdef";
    assert_eq!(
        super::gate_identity(
            "estigia gate <tool>",
            None,
            Some(session),
            Some("opencode"),
            None
        )
        .expect("a session id with its runtime derives"),
        Some(crate::harness::session::run_id("opencode", session)),
        "the gate composed a run id of its own"
    );
    assert_eq!(
        super::gate_identity(
            "estigia gate <tool>",
            Some("opencode-aaaa1111"),
            None,
            None,
            None
        )
        .expect("an explicit id"),
        Some("opencode-aaaa1111".to_owned())
    );
    assert_eq!(
        super::gate_identity("estigia gate <tool>", None, None, None, None).expect("no identity"),
        None
    );

    // The ambient fallback is read last, and never instead of a derived id:
    // an operator's exported variable must not refuse a host that forwards one.
    assert_eq!(
        super::gate_identity(
            "estigia gate <tool>",
            None,
            None,
            None,
            Some("claude-aaaa1111")
        )
        .expect("the exported fallback"),
        Some("claude-aaaa1111".to_owned())
    );
    assert_eq!(
        super::gate_identity(
            "estigia gate <tool>",
            None,
            Some(session),
            Some("opencode"),
            Some("claude-aaaa1111")
        )
        .expect("a derived id supersedes the fallback"),
        Some(crate::harness::session::run_id("opencode", session)),
        "the ambient variable collided with the forwarded identity"
    );
    assert_eq!(
        super::gate_identity("estigia gate <tool>", None, None, None, Some("  "))
            .expect("a blank fallback names nothing"),
        None
    );

    let over = super::gate_identity(
        "estigia gate <tool>",
        Some("claude-aaaa1111"),
        Some(session),
        Some("opencode"),
        None,
    )
    .expect_err("two identities are refused rather than ordered");
    assert_eq!(over.code, "gate-identity-over-specified");
    for (run_id, session_id, runtime) in [
        (None, Some(session), None),
        (None, None, Some("opencode")),
        (Some("claude-aaaa1111"), None, Some("opencode")),
    ] {
        let incomplete =
            super::gate_identity("estigia gate <tool>", run_id, session_id, runtime, None)
                .expect_err("half an identity pair is refused");
        assert_eq!(incomplete.code, "gate-identity-incomplete");
    }
}

/// A pointer for one run, stored where [`goal_reading`] reads it.
fn goal_pointer(state_root: &std::path::Path, run_id: &str, issue: Option<u64>) {
    std::fs::create_dir_all(state_root).expect("the state directory");
    let mut run = harness::session::Run::new(run_id.to_owned());
    run.issue = issue.map(crate::identity::Identity::Number);
    run.state = Some("in-progress".to_owned());
    assert!(
        harness::session::store(state_root, &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );
}

/// `estigia goal` reads a live claim: `live:true`, the issue, and `passed`.
#[test]
fn goal_answers_a_live_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let session_id = "goal-live-1";
    let run_id = harness::session::run_id("opencode", session_id);
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert!(
        reading.live,
        "a live claim was not read as live: {}",
        super::goal_line(&run_id, &reading)
    );
    assert_eq!(reading.standing, "passed");
    assert_eq!(reading.issue, Some(crate::identity::Identity::Number(7)));
    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["run_id"], run_id);
    assert_eq!(json["issue"], 7);
    assert_eq!(json["state"], "in-progress");
    assert_eq!(json["standing"], "passed");
    assert_eq!(json["live"], true);
    assert!(
        json.get("reason").is_none(),
        "a live answer named a reason: {json}"
    );
}

/// The JSON the loop plugin reads carries `completed`, so `--max` has a count
/// to compare against; a reading that did not ask for it carries `null` rather
/// than a zero the plugin could read as "nothing done". Dropping the key from
/// `goal_json` must redden this.
#[test]
fn goal_json_carries_the_completed_count() {
    let reading = super::GoalReading {
        issue: Some(crate::identity::Identity::Number(7)),
        state: Some("in-progress".to_owned()),
        standing: "passed",
        live: true,
        reason: None,
        claimable: None,
        dischargeable: None,
        held: None,
        pending: None,
        unconditioned: None,
        unadmitted: None,
        completed: Some(2),
        stop_after: None,
    };
    let json = super::goal_json("opencode-abc", &reading);
    assert_eq!(json["completed"], 2, "{json}");
    let line = super::goal_line("opencode-abc", &reading);
    assert!(
        line.contains("2 completed"),
        "the human line dropped the count: {line}"
    );
}

/// The JSON the loop plugin reads carries `dischargeable`, so a queue holding
/// only blocked work whose condition already holds is not read as an empty
/// tracker. Dropping the key from `goal_json` must redden this.
#[test]
fn goal_json_carries_the_dischargeable_count() {
    let reading = super::GoalReading {
        issue: None,
        state: None,
        standing: "nothing-sworn",
        live: false,
        reason: None,
        claimable: Some(0),
        dischargeable: Some(2),
        held: None,
        pending: None,
        unconditioned: None,
        unadmitted: None,
        completed: None,
        stop_after: None,
    };
    let json = super::goal_json("opencode-abc", &reading);
    assert_eq!(json["claimable"], 0, "{json}");
    assert_eq!(json["dischargeable"], 2, "{json}");
    let line = super::goal_line("opencode-abc", &reading);
    assert!(
        line.contains("2 dischargeable"),
        "the human line dropped the count: {line}"
    );
}

/// A pointer is answered only from a checkout of the repository its claim was
/// made in: from another repository the same issue number names a different
/// issue, and the queues counted there are not this run's.
#[test]
fn goal_does_not_answer_a_pointer_against_another_repository() {
    let (home, other) = (release_repo(), release_repo());
    let state = tempfile::tempdir().expect("a temporary state root");
    let run_id = harness::session::run_id("opencode", "goal-elsewhere-1");
    let mut run = harness::session::Run::new(run_id.clone());
    run.issue = Some(crate::identity::Identity::Number(7));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(home.path().to_path_buf());
    assert!(harness::session::store(state.path(), &run).expect("the pointer writes"));

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    let from_home = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&release_context(home.path(), state.path()), &run_id, false)
    });
    assert_eq!(from_home.standing, "passed", "the run's own repository");

    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    let from_other = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&release_context(other.path(), state.path()), &run_id, false)
    });
    assert_eq!(from_other.standing, "unreadable", "{from_other:?}");
    assert!(!from_other.live);
    assert_eq!(from_other.claimable, None);
    assert_eq!(from_other.dischargeable, None);
}

/// A run that holds nothing answers `nothing-sworn`, and never `live`.
#[test]
fn goal_answers_nothing_sworn_when_the_pointer_holds_no_issue() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    let run_id = harness::session::run_id("opencode", "goal-free-1");
    goal_pointer(&state, &run_id, None);
    let context = release_context(&repo, &state);

    let reading = super::goal_reading(&context, &run_id, false);
    assert_eq!(reading.standing, "nothing-sworn");
    assert!(!reading.live, "a run holding nothing was read as live");
    assert_eq!(reading.issue, None);
    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["standing"], "nothing-sworn");
    assert_eq!(json["live"], false);
    assert!(json["issue"].is_null());
}

/// A tracker answer that could not be read never continues a loop: whatever
/// the standing, `live` is false.
#[test]
fn goal_never_calls_an_unreadable_tracker_answer_live() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-refused-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert!(
        !reading.live,
        "an unreadable answer was read as live: {}",
        super::goal_line(&run_id, &reading)
    );
    assert_ne!(
        reading.standing, "passed",
        "an unreadable answer was read as a pass: {reading:?}"
    );
    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["live"], false);
}

/// A scripted `gh api --paginate --slurp` answer for the open issues, as the
/// queue read lists them now: no label filter, so one answer serves every state
/// and the local store decides which state each issue is in.
fn open_answer(issues: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "matches": "issues?state=open",
        "stdout": serde_json::json!([issues]).to_string(),
        "status": 0,
    })
}

/// Makes `dir` a checkout whose `origin` names `o/r`, so the store key and the
/// queue read agree on the repository without a configured one.
fn git_repo_with_origin(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).expect("the checkout the tracker reads in");
    let git = |arguments: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(arguments)
                .output()
                .is_ok_and(|output| output.status.success()),
            "git {arguments:?}"
        );
    };
    git(&["init", "--quiet", "-b", "main"]);
    git(&["remote", "add", "origin", "git@github.com:o/r.git"]);
}

/// Seeds the thread's scratch store with one row per `(number, state, body,
/// comments)`.
fn seed_store(rows: &[(u64, &str, &str, Vec<serde_json::Value>)]) {
    crate::transport::store::forget_scratch_store();
    let path = crate::transport::store::store_path().expect("the scratch store path");
    let mut store = crate::transport::store::Store::open(&path).expect("the scratch store opens");
    let repo = crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    };
    for (number, state, body, comments) in rows {
        store
            .import(
                &repo,
                *number,
                &serde_json::json!({
                    "body": body,
                    "labels": [{ "name": format!("status:{state}") }],
                    "comments": comments,
                }),
                "2026-01-01T00:00Z",
            )
            .expect("the issue imports");
    }
}

/// A live claim marker by `run`, as a comment, so the store folds the issue as
/// held by somebody else.
fn held_claim(run: &str, op: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("IC_{op}"),
        "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim encoding=pct run-id={run} runtime=opencode \
             horizon=2099-01-01T00:00Z op-id={op} -->\n"
        ),
    })
}

/// One issue, the shape the REST issues endpoint answers with, so `list-state`
/// counts it as claimable.
fn claimable_issue(number: u64) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "title": format!("issue {number}"),
        "labels": [],
        "created_at": "2026-01-01T00:00Z",
        "assignees": [],
    })
}

/// One issue, the shape `gh issue view --json body,labels,comments` answers
/// with, so an import records the state the body claims.
fn imported_issue(state: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "body": body,
        "labels": [{ "name": format!("status:{state}") }],
        "comments": [],
    })
}

/// A context whose repository is configured, so no `git remote` is read.
fn store_import_context(root: &std::path::Path) -> crate::transport::Context {
    crate::transport::Context {
        skill_dir: root.to_path_buf(),
        repo_dir: root.to_path_buf(),
        config: Vec::new(),
        repo: Some("o/r".to_owned()),
    }
}

/// `store import` admits every open issue the store has never touched, so the
/// queue that decides from the store reports none of them `unadmitted`.
#[test]
fn store_import_admits_every_open_issue_and_leaves_none_unadmitted() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let context = store_import_context(root.path());
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        open_answer(serde_json::json!([
            claimable_issue(21),
            claimable_issue(22)
        ])),
        crate::test_env::issue_view_answer(21, imported_issue("ready", "Body 21.")),
        crate::test_env::issue_view_answer(22, imported_issue("ready", "Body 22.")),
    ]);
    let admitted = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || super::store_import_in(&context))
    })
    .expect("the import ran");
    assert_eq!(
        admitted.admitted, 2,
        "not every open issue was admitted: {admitted:?}"
    );
    assert_eq!(
        admitted.already_held, 0,
        "a store that held nothing reported an issue already held: {admitted:?}"
    );

    let answer = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            crate::transport::commands::list_state(
                &context,
                "ready",
                "opencode-test",
                200,
                "2026-01-01T00:00Z",
            )
        })
    })
    .expect("the queue was read");
    assert!(
        answer.get("unadmitted").is_none(),
        "an issue the import admitted is still reported unadmitted: {answer}"
    );
    assert_eq!(
        answer["count"], 2,
        "the admitted issues are not in the ready queue: {answer}"
    );
}

/// An issue the store already holds is counted and left alone: the import does
/// not read it again and does not write a second time.
#[test]
fn store_import_leaves_an_issue_the_store_already_holds_alone() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let repo = crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    };
    {
        let mut store =
            crate::transport::store::Store::open(&store_path).expect("the fixture store opens");
        store
            .import(
                &repo,
                21,
                &imported_issue("ready", "Body 21."),
                "2026-01-01T00:00Z",
            )
            .expect("the held issue imports");
    }
    let context = store_import_context(root.path());
    let bin = crate::test_env::scripted_gh();
    let log = root.path().join("gh.log");
    let script = crate::test_env::answers(&[
        open_answer(serde_json::json!([
            claimable_issue(21),
            claimable_issue(22)
        ])),
        crate::test_env::issue_view_answer(22, imported_issue("ready", "Body 22.")),
    ]);
    let admitted = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            super::store_import_in(&context)
        })
    })
    .expect("the import ran");
    assert_eq!(
        admitted.admitted, 1,
        "the unheld issue was not admitted: {admitted:?}"
    );
    assert_eq!(
        admitted.already_held, 1,
        "the held issue was not counted as already held: {admitted:?}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("view 21 --json"),
        "the held issue was read and imported again: {calls}"
    );
    assert!(
        calls.contains("view 22 --json"),
        "the unheld issue was never read: {calls}"
    );
}

/// A diverged import surfaces as its own refusal, names the issue, and claims
/// nothing was written.
#[test]
fn a_diverged_import_names_the_issue_and_writes_nothing() {
    let failure = crate::transport::Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "import-diverged",
        "issue": 42,
        "detail": "the holder over the stored timeline differs from the holder over GitHub's",
        "action": "the store was left untouched: re-read the issue and retry",
    }));
    let refusal = super::store_import_diverged(42, failure);
    assert_eq!(refusal.code, "import-diverged");
    assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
    assert!(
        refusal.message.contains("#42"),
        "the refusal does not name the issue that diverged: {refusal}"
    );
}

/// A run whose claim has ended but whose tracker still has work: `live:false`
/// with a positive `claimable`, so a loop has a next task to go on to.
#[test]
fn goal_answers_claimable_work_when_the_run_has_no_live_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("opencode", "goal-claimable-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    seed_store(&[
        (21, "ready", "Body.", Vec::new()),
        (22, "ready", "Body.", Vec::new()),
    ]);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(serde_json::json!([
            claimable_issue(21),
            claimable_issue(22)
        ])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert!(
        !reading.live,
        "a stopped claim was read as live: {}",
        super::goal_line(&run_id, &reading)
    );
    assert_eq!(reading.standing, "stopped");
    assert_eq!(
        reading.claimable,
        Some(2),
        "the claimable work was not read: {}",
        super::goal_line(&run_id, &reading)
    );
    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["live"], false);
    assert_eq!(json["claimable"], 2);
    assert!(
        super::goal_line(&run_id, &reading).contains("2 claimable"),
        "the human line dropped the claimable count: {}",
        super::goal_line(&run_id, &reading)
    );
}

#[test]
fn goal_answers_no_work_when_the_repository_is_assisted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-assisted-1");
    goal_pointer(&state, &run_id, Some(7));
    let mut context = release_context(&repo, &state);
    context.workflow = Some(crate::config::Workflow::Assisted);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(serde_json::json!([
            claimable_issue(21),
            claimable_issue(22)
        ])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert!(!reading.live);
    assert_eq!(
        (reading.claimable, reading.dischargeable),
        (Some(0), Some(0)),
        "an assisted repository offered work to a loop: {}",
        super::goal_line(&run_id, &reading)
    );
    let stop = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::loop_stop::decide(
            &run_id,
            crate::harness::loop_text::Invocation::Mention,
            None,
            || {},
            || super::goal_reading(&context, &run_id, false),
        )
        .into_value()
    });
    assert!(
        stop.to_string()
            .contains("no claimable or dischargeable work"),
        "the loop was not told to stop: {stop}"
    );
}

#[test]
fn goal_tells_a_person_the_queues_were_not_read_where_the_repository_is_assisted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-assisted-2");
    let mut context = release_context(&repo, &state);
    context.workflow = Some(crate::config::Workflow::Assisted);

    let line = super::goal_human_line(
        &context,
        &run_id,
        super::goal_reading(&context, &run_id, false),
    );
    assert!(
        line.contains("the queues were not read") && !line.contains("0 claimable"),
        "zeros were printed as though they were measured: {line}"
    );
    let reading = super::goal_reading(&context, &run_id, false);
    assert_eq!(super::goal_json(&run_id, &reading)["claimable"], 0);
}

/// The JSON the loop plugin reads carries `held`, `pending` and `unconditioned`
/// beside the counts it already had, and a count whose read failed is `null`,
/// never zero, so the plugin cannot wait on a queue nobody read or stop on one.
#[test]
fn goal_json_carries_what_is_held_pending_and_unconditioned() {
    let mut reading = super::GoalReading {
        issue: None,
        state: None,
        standing: "nothing-sworn",
        live: false,
        reason: None,
        claimable: Some(0),
        dischargeable: Some(0),
        held: Some(3),
        pending: Some(1),
        unconditioned: None,
        unadmitted: None,
        completed: None,
        stop_after: None,
    };
    let json = super::goal_json("opencode-abc", &reading);
    assert_eq!(json["held"], 3, "{json}");
    assert_eq!(json["pending"], 1, "{json}");
    assert!(
        json["unconditioned"].is_null(),
        "an unread count was reported as a number: {json}"
    );
    let line = super::goal_line("opencode-abc", &reading);
    assert!(
        line.contains("3 held") && line.contains("1 pending"),
        "the human line dropped the counts: {line}"
    );
    reading.held = None;
    assert!(
        super::goal_json("opencode-abc", &reading)["held"].is_null(),
        "an unread count was reported as a number"
    );
}

/// Read against a tracker, a run that holds nothing counts what others hold in
/// `in-progress` and `review` (the assigned items the claimable list leaves
/// out), and splits `blocked` into the items with a recorded clause that does not
/// hold yet and the ones with none.
#[test]
fn goal_counts_what_is_held_pending_and_unconditioned() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("opencode", "goal-held-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    seed_store(&[
        (
            30,
            "review",
            "Body.",
            vec![held_claim("someone-a", &"a".repeat(32))],
        ),
        (31, "review", "Body.", Vec::new()),
        (
            40,
            "in-progress",
            "Body.",
            vec![held_claim("someone-b", &"b".repeat(32))],
        ),
        (
            41,
            "in-progress",
            "Body.",
            vec![held_claim("someone-c", &"c".repeat(32))],
        ),
        (
            50,
            "blocked",
            "<!-- issue-flow: blocked-until issue=40 state=done -->",
            Vec::new(),
        ),
        (51, "blocked", "a person decides, no clause", Vec::new()),
    ]);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(serde_json::json!([
            claimable_issue(30),
            claimable_issue(31),
            claimable_issue(40),
            claimable_issue(41),
            claimable_issue(50),
            claimable_issue(51)
        ])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["claimable"], 1, "{json}");
    assert_eq!(
        json["held"], 3,
        "two in progress and the one assigned review item are somebody else's: {json}"
    );
    assert_eq!(json["dischargeable"], 0, "{json}");
    assert_eq!(json["pending"], 1, "{json}");
    assert_eq!(json["unconditioned"], 1, "{json}");
}

/// The queue is read from the REST issues endpoint. `gh issue list --label`
/// routes through GitHub search, which lags a claim by seconds, so a `gh` that
/// refuses `issue list` must still answer the queue.
#[test]
fn goal_reads_the_queue_from_the_issues_endpoint() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("opencode", "goal-no-search-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    seed_store(&[(21, "ready", "Body.", Vec::new())]);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        serde_json::json!({
            "matches": "issue list",
            "stdout": "",
            "stderr": "gh: issue list routes through the search index",
            "status": 1,
        }),
        open_answer(serde_json::json!([claimable_issue(21)])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert_eq!(
        reading.claimable,
        Some(1),
        "the queue was not read from the issues endpoint: {}",
        super::goal_line(&run_id, &reading)
    );
}

/// An assigned item, and a pull request, are not claimable, whatever the
/// endpoint returns.
#[test]
fn goal_does_not_count_an_assigned_item_as_claimable() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("opencode", "goal-assigned-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    seed_store(&[
        (21, "ready", "Body.", Vec::new()),
        (
            22,
            "ready",
            "Body.",
            vec![held_claim("someone-d", &"d".repeat(32))],
        ),
    ]);

    let mut pull = claimable_issue(23);
    pull["pull_request"] = serde_json::json!({});

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(serde_json::json!([
            claimable_issue(21),
            claimable_issue(22),
            pull
        ])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert_eq!(
        reading.claimable,
        Some(1),
        "an assigned item or a pull request was counted: {}",
        super::goal_line(&run_id, &reading)
    );
}

/// An open issue the store never admitted is counted under `unadmitted`, apart
/// from `claimable`, and a read that did not land leaves it `null`.
#[test]
fn goal_counts_an_issue_the_store_never_admitted_apart_from_claimable() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("opencode", "goal-unadmitted-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    seed_store(&[(21, "ready", "Body.", Vec::new())]);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(serde_json::json!([
            claimable_issue(21),
            claimable_issue(22),
            claimable_issue(23)
        ])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["claimable"], 1, "{json}");
    assert_eq!(json["unadmitted"], 2, "{json}");
    assert!(
        super::goal_line(&run_id, &reading).contains("2 unadmitted"),
        "the human line dropped the unadmitted count: {}",
        super::goal_line(&run_id, &reading)
    );

    let script = crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(7)]);
    let unread = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });
    assert_eq!(unread.unadmitted, None, "{unread:?}");
    assert!(
        super::goal_json(&run_id, &unread)["unadmitted"].is_null(),
        "a read that did not land was reported as a number"
    );
}

/// A run whose claim has ended and whose queue is empty: `claimable` is zero,
/// a readable answer that says there is nothing to go on to, not an unknown.
#[test]
fn goal_answers_no_claimable_work_when_the_queue_is_empty() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("opencode", "goal-empty-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    seed_store(&[]);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(serde_json::json!([])),
    ]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert_eq!(reading.standing, "stopped");
    assert_eq!(reading.claimable, Some(0));
    let json = super::goal_json(&run_id, &reading);
    assert_eq!(json["claimable"], 0);
}

/// A queue that could not be read leaves `claimable` unknown — `null`, never
/// zero — so the loop stops on the fault rather than reading it as no work.
#[test]
fn goal_leaves_claimable_unknown_when_the_queue_cannot_be_read() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-unreadable-queue-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(7)]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::goal_reading(&context, &run_id, false)
    });

    assert_eq!(reading.standing, "stopped");
    assert_eq!(
        reading.claimable, None,
        "a queue nobody could read was answered as a count: {reading:?}"
    );
    let json = super::goal_json(&run_id, &reading);
    assert!(
        json["claimable"].is_null(),
        "an unreadable queue was reported as a number: {json}"
    );
}

/// `--run-id` and `--session-id`+`--runtime` are the two identity roads, and a
/// half identity refuses by naming `estigia goal` rather than a dead-end
/// `estigia gate`.
#[test]
fn goal_accepts_the_two_identity_roads_and_names_itself() {
    let session = "ses_0123456789abcdef0123456789abcdef";
    assert_eq!(
        super::gate_identity("estigia goal", None, Some(session), Some("opencode"), None)
            .expect("a session id with its runtime derives"),
        Some(harness::session::run_id("opencode", session)),
    );
    assert_eq!(
        super::gate_identity("estigia goal", Some("opencode-aaaa1111"), None, None, None)
            .expect("an explicit id"),
        Some("opencode-aaaa1111".to_owned()),
    );

    let incomplete = super::gate_identity("estigia goal", None, Some(session), None, None)
        .expect_err("half an identity pair is refused");
    assert_eq!(incomplete.code, "gate-identity-incomplete");
    let Resolution::Run { command } = incomplete.resolution else {
        panic!("a malformed identity must name a runnable command");
    };
    assert!(
        command.starts_with("estigia goal"),
        "a `goal` refusal named a different verb: {command}"
    );
    assert!(
        !command.contains("estigia gate"),
        "a `goal` refusal sent the caller to `gate`: {command}"
    );
}

/// The two identity roads parse on `goal`, its subcommands parse, and the
/// identity flags are global so they may be given before or after the verb.
///
/// The task this shape serves is the one the flags exist for: the OpenCode loop
/// plugin forwards `--session-id` and the operator types `--run-id`, and neither
/// should have to know whether it comes before or after `progress`.
#[test]
fn the_goal_subcommands_parse_with_the_identity_flags_before_or_after() {
    for arguments in [
        vec![
            "estigia",
            "goal",
            "progress",
            "working",
            "--run-id",
            "opencode-aaaa1111",
        ],
        vec![
            "estigia",
            "goal",
            "--run-id",
            "opencode-aaaa1111",
            "progress",
            "working",
        ],
        vec![
            "estigia",
            "goal",
            "blocked",
            "waiting on review",
            "--run-id",
            "opencode-aaaa1111",
        ],
        vec![
            "estigia",
            "goal",
            "complete",
            "--run-id",
            "opencode-aaaa1111",
        ],
        vec![
            "estigia",
            "goal",
            "stop-after",
            "--reason",
            "the operator asked",
            "--run-id",
            "opencode-aaaa1111",
        ],
        vec![
            "estigia",
            "goal",
            "stop-after",
            "--clear",
            "--run-id",
            "opencode-aaaa1111",
        ],
        vec![
            "estigia",
            "goal",
            "--json",
            "--session-id",
            "ses_x",
            "--runtime",
            "opencode",
        ],
    ] {
        super::Cli::try_parse_from(&arguments)
            .unwrap_or_else(|error| panic!("{arguments:?} did not parse: {error}"));
    }
}

/// The identity refusal for `goal progress`/`blocked`/`stop-after` names a road
/// that parses.
///
/// `goal_identity` interpolates the verb into its resolution, and `progress` and
/// `blocked` take a required positional: a road that omitted it would be one the
/// binary refuses to parse, the dead end `AGENTS.md` forbids.
#[test]
fn the_goal_activity_identity_road_names_a_command_that_parses() {
    for command in [
        "estigia goal progress <note>",
        "estigia goal blocked <reason>",
        "estigia goal stop-after",
    ] {
        let refusal = super::goal_identity(command, None, Some("ses_x"), None)
            .expect_err("half an identity refuses");
        let Resolution::Run { command: road } = &refusal.resolution else {
            panic!("{command} named no road: {:?}", refusal.resolution);
        };
        let argv: Vec<&str> = road.split_whitespace().collect();
        Cli::try_parse_from(&argv).unwrap_or_else(|error| {
            panic!("`{road}` is named by a refusal and this binary cannot parse it:\n{error}")
        });
    }
}

/// A pointer for one run, stored where [`goal_completion_reading`] reads it,
/// carrying the review receipt this run published when it has one.
fn goal_receipt_pointer(
    state_root: &std::path::Path,
    run_id: &str,
    issue: u64,
    receipt: Option<crate::transport::claim::ReviewTransaction>,
) {
    std::fs::create_dir_all(state_root).expect("the state directory");
    let mut run = harness::session::Run::new(run_id.to_owned());
    run.issue = Some(crate::identity::Identity::Number(issue));
    run.state = Some("in-progress".to_owned());
    run.review_transaction = receipt;
    assert!(
        harness::session::store(state_root, &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );
}

fn goal_completion_receipt() -> crate::transport::claim::ReviewTransaction {
    crate::transport::claim::ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    }
}

fn goal_change(
    repo: &std::path::Path,
    file: &str,
    before: &str,
    after: &str,
) -> crate::transport::claim::ReviewTransaction {
    let git = |arguments: &[&str]| {
        let output = std::process::Command::new("git")
            .args(arguments)
            .current_dir(repo)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {arguments:?} failed");
        String::from_utf8(output.stdout).expect("utf-8")
    };
    git(&["init", "--quiet", "-b", "main"]);
    git(&["config", "user.email", "estigia@example.invalid"]);
    git(&["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join(file), before).expect("content");
    git(&["add", file]);
    git(&["commit", "--quiet", "-m", "base"]);
    let base = git(&["rev-parse", "HEAD"]).trim().to_owned();
    std::fs::write(repo.join(file), after).expect("content");
    git(&["add", file]);
    git(&["commit", "--quiet", "-m", "head"]);
    crate::transport::claim::ReviewTransaction {
        head: git(&["rev-parse", "HEAD"]).trim().to_owned(),
        base,
        ..goal_completion_receipt()
    }
}

fn goal_low_change(repo: &std::path::Path) -> crate::transport::claim::ReviewTransaction {
    goal_change(repo, "notes.md", "A line.\n", "A line, reworded.\n")
}

fn goal_medium_change(repo: &std::path::Path) -> crate::transport::claim::ReviewTransaction {
    goal_change(repo, "foo.rs", "fn main() {}\n", "fn main() { 1; }\n")
}

fn goal_comment(at: &str, body: String) -> serde_json::Value {
    serde_json::json!({
        "id": format!("IC_{at}"),
        "createdAt": at,
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": body,
    })
}

fn goal_marker(kind: &str, fields: &[(&str, &str)]) -> String {
    crate::transport::markers::render(kind, fields).expect("a protocol marker")
}

/// A timeline this run published against, and the two answers the completion
/// read asks of it.
fn goal_completion_script(
    run_id: &str,
    receipt: &crate::transport::claim::ReviewTransaction,
    verdict: Option<&str>,
    lane: serde_json::Value,
) -> String {
    let claim = goal_comment(
        "2026-01-01T00:00:00Z",
        format!(
            "Claimed.\n\n{}\n",
            goal_marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            )
        ),
    );
    let published = goal_comment(
        "2026-01-01T00:01:00Z",
        goal_marker(
            "published",
            &[
                ("run-id", run_id),
                ("epoch", receipt.epoch.as_str()),
                ("pr", &receipt.pr.to_string()),
                ("head", receipt.head.as_str()),
                ("base", receipt.base.as_str()),
                ("digest", receipt.digest.as_str()),
            ],
        ),
    );
    let mut comments = vec![claim, published];
    if let Some(outcome) = verdict {
        comments.push(goal_comment(
            "2026-01-01T00:02:00Z",
            goal_marker(
                "review-verdict",
                &[
                    ("run-id", "codex-b"),
                    ("reviewer", "codex-b"),
                    ("op-id", &"3".repeat(32)),
                    ("epoch", receipt.epoch.as_str()),
                    ("pr", &receipt.pr.to_string()),
                    ("head", receipt.head.as_str()),
                    ("base", receipt.base.as_str()),
                    ("digest", receipt.digest.as_str()),
                    ("outcome", outcome),
                ],
            ),
        ));
    }
    let full = serde_json::json!({
        "state": "OPEN",
        "labels": [{ "name": "status:in-progress" }],
        "comments": comments,
    });
    let only_comments = serde_json::json!({ "comments": comments });
    crate::test_env::answers(&[
        crate::test_env::issue_view_answer(7, full),
        serde_json::json!({
            "matches": "view 7 --json comments",
            "stdout": only_comments.to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        }),
        lane,
    ])
}

/// A closed issue is complete on the tracker's own word, and nothing about the
/// run's own publication is read.
#[test]
fn goal_complete_answers_complete_on_a_closed_issue() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-closed");
    goal_receipt_pointer(&state, &run_id, 7, None);
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::closed_issue(7)]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(
        reading.complete,
        "{}",
        super::goal_completion_line(&run_id, &reading)
    );
    assert_eq!(reading.standing, "closed");
    assert_eq!(reading.issue, Some(crate::identity::Identity::Number(7)));
    let json = super::goal_completion_json(&run_id, &reading);
    assert_eq!(json["complete"], true);
    assert_eq!(json["standing"], "closed");
    assert_eq!(json["run_id"], run_id);
}

/// An open issue whose latest receipt its computed verdict clears and a green
/// lane is complete, naming the head.
#[test]
fn goal_complete_answers_complete_on_a_computed_clearance_and_a_green_lane() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-green");
    let receipt = goal_low_change(&repo);
    goal_receipt_pointer(&state, &run_id, 7, Some(receipt.clone()));
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let lane = serde_json::json!({
        "matches": "check-runs",
        "stdout": serde_json::json!({
            "total_count": 1,
            "check_runs": [{
                "name": "ci",
                "status": "completed",
                "conclusion": "success",
                "html_url": "https://github.com/o/r/actions/runs/1/job/2",
                "id": 1,
            }],
        })
        .to_string(),
        "status": 0,
    });
    let script = goal_completion_script(&run_id, &receipt, None, lane);

    let bin = crate::test_env::scripted_gh();
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(
        reading.complete,
        "{}",
        super::goal_completion_line(&run_id, &reading)
    );
    assert_eq!(reading.standing, "cleared");
    assert_eq!(reading.head.as_deref(), Some(receipt.head.as_str()));
    let json = super::goal_completion_json(&run_id, &reading);
    assert_eq!(json["complete"], true);
    assert!(json.get("reviewer").is_none(), "{json}");
    assert_eq!(json["head"], receipt.head);
}

/// An open issue this run has published nothing for is not complete, and the
/// reason says so rather than reporting an unreadable.
#[test]
fn goal_complete_is_not_complete_when_nothing_was_published() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-unpublished");
    goal_receipt_pointer(&state, &run_id, 7, None);
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(!reading.complete, "{reading:?}");
    assert_eq!(reading.standing, "unpublished");
    assert!(
        reading.reason.is_some(),
        "an unpublished goal named no reason: {reading:?}"
    );
    assert!(
        super::goal_completion_line(&run_id, &reading).contains("goal not complete"),
        "the human line does not say the goal is incomplete"
    );
}

/// An open issue whose latest receipt carries a legacy rejected verdict is not
/// complete.
#[test]
fn goal_complete_is_not_complete_over_a_legacy_rejected_verdict() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-rejected");
    let receipt = goal_completion_receipt();
    goal_receipt_pointer(&state, &run_id, 7, Some(receipt.clone()));
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let lane = serde_json::json!({
        "matches": "check-runs",
        "stdout": serde_json::json!({ "total_count": 0, "check_runs": [] }).to_string(),
        "status": 0,
    });
    // A rejected verdict names the receipt and still does not qualify delivery.
    let script = goal_completion_script(&run_id, &receipt, Some("rejected"), lane);

    let bin = crate::test_env::scripted_gh();
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(!reading.complete, "{reading:?}");
    assert_eq!(reading.standing, "open");
}

/// A legacy accepted verdict alone clears nothing: a `medium` receipt needs its
/// lens reports and a recorded computed verdict.
#[test]
fn goal_complete_is_not_complete_on_a_legacy_accepted_verdict_alone() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-legacy");
    let receipt = goal_medium_change(&repo);
    goal_receipt_pointer(&state, &run_id, 7, Some(receipt.clone()));
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let lane = serde_json::json!({
        "matches": "check-runs",
        "stdout": serde_json::json!({ "total_count": 0, "check_runs": [] }).to_string(),
        "status": 0,
    });
    let script = goal_completion_script(&run_id, &receipt, Some("accepted"), lane);

    let bin = crate::test_env::scripted_gh();
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(!reading.complete, "{reading:?}");
    assert_eq!(reading.standing, "open");
}

/// A tracker that did not answer never reads as complete: an unknown result is
/// not clearance.
#[test]
fn goal_complete_never_answers_complete_on_an_unreadable_tracker() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-unread");
    let receipt = goal_completion_receipt();
    goal_receipt_pointer(&state, &run_id, 7, Some(receipt));
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(!reading.complete, "{reading:?}");
    assert_eq!(reading.standing, "unread");
}

/// A computed clearance with a lane that could not be read never reads as
/// complete: a failed lane read is not a green one.
#[test]
fn goal_complete_never_answers_complete_on_an_unreadable_lane() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-complete-lane-fail");
    let receipt = goal_low_change(&repo);
    goal_receipt_pointer(&state, &run_id, 7, Some(receipt.clone()));
    let context = release_context(&repo, &state);
    let run = harness::session::load(&state, &run_id);

    let lane = serde_json::json!({
        "matches": "check-runs",
        "stderr": "gh: HTTP 403",
        "status": 1,
    });
    let script = goal_completion_script(&run_id, &receipt, None, lane);

    let bin = crate::test_env::scripted_gh();
    let reading = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        harness::goal_completion_reading(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            &run,
        )
    });

    assert!(!reading.complete, "{reading:?}");
    assert_eq!(reading.standing, "unreadable");
}

/// `goal progress` records a progress marker through the tool path it shares
/// with the CLI, and posts no comment to GitHub.
#[test]
fn goal_progress_records_a_progress_marker_for_a_live_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("opencode", "goal-progress-1");
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    let note = root.path().join("note.md");
    std::fs::write(&note, "working on the goal half\n").expect("the note");

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    let log = root.path().join("gh.log");
    let answer = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        harness::mcp::run_tool(
            "heartbeat",
            &serde_json::json!({
                "issue": 7,
                "run_id": run_id,
                "expect_state": "in-progress",
                "body_file": note.display().to_string(),
                "kind": "progress",
            }),
            Ok(&context),
        )
    })
    .unwrap_or_else(|failure| panic!("goal progress was refused: {failure:?}"));

    assert!(
        answer.to_string().contains("recorded\\\":true"),
        "the progress marker was not recorded: {answer}"
    );
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(
        !calls.contains("issue comment"),
        "a progress marker posts no comment: {calls}"
    );
}

/// `--loop` is gone: the loop plugin is the `Loop plugin` setting now, and a
/// flag that no longer exists is a clap error rather than a command that
/// installs nothing.
#[test]
fn the_loop_flag_is_not_accepted_by_setup() {
    use clap::Parser;
    let refused = super::Cli::try_parse_from(["estigia", "setup", "opencode", "--loop"])
        .expect_err("`setup --loop` must not parse");
    assert_eq!(
        refused.kind(),
        clap::error::ErrorKind::UnknownArgument,
        "`setup --loop` is still accepted"
    );
}

/// The state root this fixture builds records no pointers at all, deliberately:
/// with nothing stored, `held_by_a_pointer` can only ever answer `false`, so
/// every row below exercises the `launched`-only half of the clamp and none
/// of them says anything about a call naming a checkout a pointer records —
/// that road is `a_call_naming_this_run_s_own_sibling_worktree_is_admitted`
/// and its neighbours, further down this file.
#[test]
fn a_directory_the_call_names_may_narrow_the_decision_and_not_move_it() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path();
    let inside = launched.join("wt-a");
    std::fs::create_dir_all(&inside).expect("a directory inside");

    let narrowed = |payload: serde_json::Value, tool: &str| {
        super::narrowed_by_the_call(&payload, tool, launched, state.path(), None).0
    };
    // right everywhere, so the expectation goes through the same normalisation as
    // the answer and the assertion compares places rather than names.
    //
    // from its expectation in shape rather than in spelling. What survives
    // unseen is a normalisation that is wrong, applied identically to both sides,
    // and still landing under `launched`. The instance that matters is a leaked
    // verbatim prefix, owned by
    // `a_placed_path_carries_no_verbatim_prefix` in `src/paths.rs`. Not the
    // *only* one constructible: appending or popping a component survives these
    // rows too, and both are caught elsewhere in the suite.
    //
    //
    // Found in CI after two contexts had accepted the change, which is the
    // ordering this repository has filed against itself: the first
    // cross-platform signal arrives once the reviewing is over, so a one-line
    // platform defect spends every verdict the change had earned.
    let placed = |path: &std::path::Path| {
        crate::paths::placed(path).expect("the fixture path can be placed")
    };
    let workdir = |named: &str| serde_json::json!({ "command": "git commit", "workdir": named });

    // What it is for: somewhere under the directory this process stands in.
    assert_eq!(
        narrowed(workdir(&inside.display().to_string()), "bash"),
        Some(placed(&inside)),
        "an absolute directory inside the launch directory was not honoured"
    );
    assert_eq!(
        narrowed(workdir("wt-a"), "bash"),
        Some(placed(&launched.join("wt-a"))),
        "a relative directory was not resolved against the launch directory"
    );
    // Wrapped, because a hook that nests the tool's arguments is why the nesting
    // is read at all, and this key has to travel the same way.
    assert_eq!(
        narrowed(
            serde_json::json!({ "tool_input": { "workdir": "wt-a" } }),
            "bash"
        ),
        Some(placed(&launched.join("wt-a"))),
        "a wrapped payload lost the working directory"
    );

    for out in [
        "..",
        "wt-a/../..",
        &launched.join("..").display().to_string(),
        if cfg!(windows) { "C:\\Windows" } else { "/etc" },
    ] {
        assert_eq!(
            narrowed(workdir(out), "bash"),
            None,
            "a call steered the gate to {out:?}, outside the directory it was launched in"
        );
    }

    // And the road the first clamp did not cover, which is the one worth having
    // opened falls back to the spelling as written. `..` is then never cancelled,
    // so `wt-a\..\..\nope` still *starts with* the launch directory and the clamp
    // called it inside — for a path that is not. Worse than the escape it
    // collapses the spelling before resolving what exists, and its own doc names
    // this failure. Every spelling here canonicalises to nothing, which is
    // exactly why the rows above stayed green while the gate was open.
    let from_absolute = if cfg!(windows) {
        format!("{}\\..\\..\\nope", inside.display())
    } else {
        format!("{}/../../nope", inside.display())
    };
    let unopenable: Vec<&str> = if cfg!(windows) {
        vec![
            "wt-a\\..\\..\\nope",
            "wt-a/../../../nope",
            "wt-a\\..\\..\\..\\..\\Windows-that-is-not-there",
            &from_absolute,
        ]
    } else {
        vec![
            "wt-a/../../nope",
            "wt-a/../../../nope",
            "wt-a/../../../../etc-that-is-not-there",
            &from_absolute,
        ]
    };
    for out in unopenable {
        assert_eq!(
            narrowed(workdir(out), "bash"),
            None,
            "{out:?} climbed out of the launch directory through a path that \
             cannot be opened, and the clamp compared the spelling instead of \
             where it lands"
        );
    }

    // The other half of the same road, and the one that says the repair is a
    // resolution rather than a rejection. This climbs *back* to somewhere that
    // is genuinely inside, so it is honoured — and it is honoured as
    // `<launched>/nope`, not as something under `wt-a`. That distinction is the
    assert_eq!(
        narrowed(
            workdir(if cfg!(windows) {
                "wt-a\\..\\nope"
            } else {
                "wt-a/../nope"
            }),
            "bash"
        ),
        Some(placed(&launched.join("nope"))),
        "a path that climbs back inside was attributed to the component it \
         climbed through rather than to where it lands"
    );

    // And the tools it is not read for. Bash is the only one measured to carry
    // the key and the only one this closes; honouring it elsewhere is inventing
    // evidence from an argument nothing documents, and it is how the escape
    // above reached `write` and `edit` too.
    for tool in [
        "write",
        "edit",
        "patch",
        "multiedit",
        "notebookedit",
        "update",
    ] {
        assert_eq!(
            narrowed(workdir(&inside.display().to_string()), tool),
            None,
            "`{tool}` honoured a working directory no host documents sending it"
        );
    }
    // Spelled as the plugin lowercases it, and as a host might not.
    assert!(
        narrowed(workdir("wt-a"), "Bash").is_some(),
        "the tool name was matched case-sensitively"
    );

    // Nothing named, nothing narrowed — the fallback the project context keeps.
    assert_eq!(
        narrowed(serde_json::json!({ "command": "git commit" }), "bash"),
        None,
        "a call naming no directory had one invented for it"
    );
    assert_eq!(
        narrowed(workdir("   "), "bash"),
        None,
        "a blank directory was treated as one"
    );

    // The host's own key is a separate door and keeps its own contract: it is
    // still read, including from outside this process's directory, because the
    // hook that writes it may run from anywhere. Whether an uncovered name is
    // used is decided in show_gate, not here.
    assert_eq!(
        super::payload_cwd(&serde_json::json!({ "cwd": "/a", "workdir": "/b" })),
        "/a",
        "the call's argument overtook what the host named"
    );
    assert_eq!(
        super::payload_cwd(&serde_json::json!({ "workdir": "/b" })),
        "",
        "the call's argument was read as though the host had named it"
    );
}

/// A write-shaped tool call narrows by the directory its target lands in.
///
/// The earlier cure read the one tool that names a working directory
/// (Bash's `workdir`) and left the six that name a **file** — every `Write`,
/// `Edit` and `Patch` was adjudicated at the project root, so two concurrent
/// isolated runs refused each other's writes with `several-runs-hold-this-
/// checkout`. The write tools carry a target path, which is evidence about
/// where the write lands, and the same clamp that keeps a model-written
/// `workdir` from steering the gate must hold it: the target's directory is
/// narrowed to somewhere the shared clamp admits — inside the directory this
/// process was launched in, or inside a checkout `held_by_a_pointer` covers —
/// or it is no evidence at all.
///
/// The state root this fixture builds records no pointers at all, for the
/// same reason and with the same consequence as
/// `a_directory_the_call_names_may_narrow_the_decision_and_not_move_it`'s own
/// doc, above: every row below exercises the `launched`-only half of the
/// clamp, and the road where a call names a checkout a pointer records is
/// measured by `a_call_naming_this_run_s_own_sibling_worktree_is_admitted`
/// and its neighbours, further down this file.
#[test]
fn a_write_tool_call_narrows_by_the_directory_its_target_lands_in() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path();
    let inside = launched.join("wt-a");
    std::fs::create_dir_all(&inside).expect("a directory inside");

    let narrowed = |payload: serde_json::Value, tool: &str| {
        super::narrowed_by_the_call(&payload, tool, launched, state.path(), None).0
    };
    let placed = |path: &std::path::Path| {
        crate::paths::placed(path).expect("the fixture path can be placed")
    };
    let target = |named: &str| serde_json::json!({ "file_path": named });

    // What it is for: a file inside one run's worktree is adjudicated in that
    // worktree, not at the shared base checkout. The directory containing the
    // target is the answer — the most precise place the write lands, which is
    // covered by the worktree holder and not by the base checkout at equal
    // depth.
    assert_eq!(
        narrowed(
            target(&inside.join("src/main.rs").display().to_string()),
            "edit"
        ),
        Some(placed(&inside.join("src"))),
        "a target inside the launch directory did not narrow to its parent"
    );
    // Wrapped, because the hook payload nests the tool's arguments.
    assert_eq!(
        narrowed(
            serde_json::json!({ "tool_input": { "file_path": "wt-a/src/main.rs" } }),
            "write"
        ),
        Some(placed(&launched.join("wt-a").join("src"))),
        "a wrapped payload lost the target path"
    );
    // A target in a subdirectory that does not exist yet resolves to its
    // nearest existing ancestor — creating a file in a new subdirectory of a
    // worktree is ordinary, and answering "no evidence" there would leave the
    // defect for exactly the case a fresh worktree hits first.
    assert_eq!(
        narrowed(target("wt-a/new-mod/src/lib.rs"), "patch"),
        Some(placed(&launched.join("wt-a").join("new-mod").join("src"))),
        "a target under a new subdirectory was not resolved to its existing ancestor"
    );
    // Every write-shaped tool carries the same evidence, because the gate
    // cannot know which host will send which.
    for tool in [
        "write",
        "edit",
        "patch",
        "multiedit",
        "notebookedit",
        "update",
    ] {
        assert_eq!(
            narrowed(target("wt-a/src/main.rs"), tool),
            Some(placed(&launched.join("wt-a").join("src"))),
            "`{tool}` did not narrow by its target path"
        );
    }

    // The clamp holds exactly as it does for `workdir`: a target resolving
    // outside every covered checkout answers no evidence, and `..` or an
    // absolute path elsewhere cannot choose a directory the gate was not
    // already going to look at.
    for out in [
        "..",
        "wt-a/../../nope.rs",
        &launched.join("..").join("nope.rs").display().to_string(),
        if cfg!(windows) {
            "C:\\Windows\\x.rs"
        } else {
            "/etc/x.rs"
        },
    ] {
        assert_eq!(
            narrowed(target(out), "edit"),
            None,
            "a target at {out:?} narrowed the gate outside the launch directory"
        );
    }
    // A target that climbs back inside lands where it lands, like `workdir`.
    assert_eq!(
        narrowed(target("wt-a/../nope.rs"), "edit"),
        Some(placed(launched.join("nope.rs").parent().unwrap())),
        "a target that climbs back inside was not attributed to where it lands"
    );

    // Nothing named, nothing narrowed.
    assert_eq!(
        narrowed(serde_json::json!({ "command": "git commit" }), "edit"),
        None,
        "a call naming no target had one invented for it"
    );
    assert_eq!(
        narrowed(serde_json::json!({ "file_path": "   " }), "edit"),
        None,
        "a blank target was treated as one"
    );
    // And Bash does not narrow by a file path: its evidence is `workdir`.
    assert_eq!(
        narrowed(target("wt-a/src/main.rs"), "bash"),
        None,
        "Bash narrowed by a file path no host documents sending it"
    );
}

/// `checkout_root_covering` must never hand back a root above the
/// directory this process was launched in.
///
/// `narrowed_by_the_call`'s own contract is that its answer lies inside
/// `launched`, or inside a checkout a pointer already records — it may only
/// narrow to one of those two. A holding's *recorded* root carries no
/// such promise: `launched` can itself sit below a wider checkout's own
/// root, a package directory in a monorepo or anywhere a person stands when
/// they type `estigia gate`, both named in [`crate::paths::covers`]'s own
/// doc. Left unfiltered, a write below `launched` resolved
/// `context.repo_dir` to that wider root — above `launched`, the one
/// direction the clamp exists to refuse: `roles::definition_for` and the
/// repository's own configuration are both read from `context.repo_dir`
/// with no upward search, so climbing out of `launched` reaches a directory
/// this call was never adjudicated against.
#[test]
fn checkout_root_covering_never_climbs_above_the_launch_directory() {
    let state = tempfile::tempdir().expect("a temporary state root");
    let root = tempfile::tempdir().expect("a temporary checkout root");
    let launched = root.path().join("packages").join("foo");
    std::fs::create_dir_all(launched.join("src")).expect("a subdirectory below the launch point");

    // A holding recorded over the *whole* checkout, not the package
    // directory this process happens to be launched in — the ordinary shape
    // for a monorepo, where one claim covers the top-level root.
    let mut run = harness::session::Run::new("claude-monorepo0".to_owned());
    run.issue = Some(crate::identity::Identity::Number(1));
    run.repo_dir = Some(root.path().to_path_buf());
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    // A write inside the package directory narrows to somewhere under
    // `launched`, exactly as the clamp promises.
    let candidate = super::narrowed_by_the_call(
        &serde_json::json!({
            "file_path": launched.join("src").join("main.rs").display().to_string()
        }),
        "edit",
        &launched,
        state.path(),
        None,
    )
    .0
    .expect("a target inside the launch directory narrows");
    assert!(
        crate::paths::covers(&launched, &candidate),
        "the candidate itself already escaped `launched`, so this proves nothing: {candidate:?}"
    );

    // Resolving it back to the holder's recorded root must not climb above
    // the directory this process was launched in: the checkout root is real
    // evidence, but it is evidence about a directory the call never named,
    // and role-definition lookup and repository configuration both read
    // `context.repo_dir` with no upward search.
    let (resolved, _) = super::checkout_root_covering(state.path(), &candidate, &launched);
    assert_eq!(
        resolved, None,
        "the covering root climbed above the launch directory: {resolved:?}"
    );
}

/// `Worktree location` on a real install is a **sibling** of the
/// repository it was minted from — `H:\REPO\worktree\<repo>\<branch>`, never
/// a child of `H:\REPO\<repo>` — so a call naming that worktree could never
/// resolve, and a run holding a completed claim, a verdict and a green CI run
/// still had `estigia gate` adjudicate `gh pr merge` against the launch
/// checkout on `main`. `held_by_a_pointer` is the fix's whole surface: a
/// second admissible target drawn from a pointer `start_branch` wrote, never
/// from the call's own argument.
#[test]
fn a_call_naming_this_run_s_own_sibling_worktree_is_admitted() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path().join("repo");
    let worktree = root.path().join("worktree");
    std::fs::create_dir_all(&launched).expect("the launch directory");
    std::fs::create_dir_all(&worktree).expect("a worktree beside it, not inside it");

    let mut run = harness::session::Run::new("claude-issue131a1".to_owned());
    run.issue = Some(crate::identity::Identity::Number(131));
    run.repo_dir = Some(launched.clone());
    run.worktree = Some(worktree.clone());
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    let placed = |path: &std::path::Path| {
        crate::paths::placed(path).expect("the fixture path can be placed")
    };
    let payload = serde_json::json!({
        "command": "gh pr merge",
        "workdir": worktree.display().to_string(),
    });

    let candidate =
        super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), Some(&run.run_id)).0;
    assert_eq!(
        candidate,
        Some(placed(&worktree)),
        "a worktree this run's own pointer records was refused for sitting beside the launch \
         directory instead of inside it — the sibling clamp"
    );
    let candidate = candidate.expect("asserted above");
    let resolved = super::checkout_root_covering(state.path(), &candidate, &launched).0;
    assert_eq!(
        resolved,
        Some(worktree),
        "the covering-root resolution did not admit a sibling worktree this run's own pointer \
         covers: {resolved:?}"
    );
}

/// The lever the clamp exists to close, proven still shut beside the widening
/// above: a sibling directory *no* pointer records is still refused, even
/// though a covered sibling sits right next to it under the same parent.
#[test]
fn a_sibling_directory_no_pointer_covers_stays_refused() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path().join("repo");
    let worktree = root.path().join("worktree");
    let uncovered = root.path().join("elsewhere");
    std::fs::create_dir_all(&launched).expect("the launch directory");
    std::fs::create_dir_all(&worktree).expect("a worktree this run's pointer covers");
    std::fs::create_dir_all(&uncovered).expect("a sibling directory no pointer names");

    let mut run = harness::session::Run::new("claude-issue131b2".to_owned());
    run.issue = Some(crate::identity::Identity::Number(131));
    run.repo_dir = Some(launched.clone());
    run.worktree = Some(worktree);
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": uncovered.display().to_string(),
    });
    assert_eq!(
        super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), Some(&run.run_id)).0,
        None,
        "a directory no pointer records was admitted because a covered sibling sat beside it — \
         the widening opened the lever the clamp exists to close"
    );
}

/// The `None` arm's own version of the refusal above. This is a shipped
/// road, not a corner: a call with no identity at all takes it — the
/// OpenCode plugin's when its host handed no session id and no run id was
/// exported around the plugin's process — so this is the road such a call
/// takes when it names a directory no pointer covers. It names a directory
/// *no* pointer covers at all, so it
/// stays green under both the old widening and the repaired narrowing — it
/// closes the zero-holder gap, not the narrowing itself; its neighbour below
/// measures that.
#[test]
fn without_a_run_id_a_sibling_directory_no_pointer_covers_stays_refused() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path().join("repo");
    let worktree = root.path().join("worktree");
    let uncovered = root.path().join("elsewhere");
    std::fs::create_dir_all(&launched).expect("the launch directory");
    std::fs::create_dir_all(&worktree).expect("a worktree this run's pointer covers");
    std::fs::create_dir_all(&uncovered).expect("a sibling directory no pointer names");

    let mut run = harness::session::Run::new("claude-issue131f7".to_owned());
    run.issue = Some(crate::identity::Identity::Number(131));
    run.repo_dir = Some(launched.clone());
    run.worktree = Some(worktree);
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": uncovered.display().to_string(),
    });
    assert_eq!(
        super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), None).0,
        None,
        "a directory no pointer records was admitted with no run id to ask with"
    );
}

/// The property `held_by_a_pointer`'s identity-less arm exists to enforce,
/// pinned directly: `Run::covered()` chains `repo_dir` and `worktree`, so a
/// single holder's `repo_dir` alone used to be enough to admit any directory
/// inside it — an unrelated repository's own base checkout included — because
/// the old arm only checked "exactly one holder", never *which* of the two
/// values that holder covered `named` through. This is a shipped road: a
/// call with no identity at all takes this arm — the OpenCode plugin's when
/// its host handed no session id and no run id was exported, a hand-run
/// `estigia gate` given neither. Unlike its
/// neighbour above, the directory here *is* covered — by `other_repo`'s
/// `repo_dir` alone — so this is the row that actually measures the repaired
/// narrowing.
#[test]
fn without_a_run_id_a_directory_only_a_holders_repo_dir_covers_stays_refused() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path().join("repo-a");
    let other_repo = root.path().join("repo-b");
    let other_worktree = root.path().join("repo-b-worktree");
    std::fs::create_dir_all(&launched).expect("the launch directory");
    std::fs::create_dir_all(&other_repo).expect("an unrelated repository's own base checkout");
    std::fs::create_dir_all(&other_worktree).expect("that repository's own worktree, elsewhere");

    let mut run = harness::session::Run::new("claude-issue131g8".to_owned());
    run.issue = Some(crate::identity::Identity::Number(999));
    run.repo_dir = Some(other_repo.clone());
    run.worktree = Some(other_worktree);
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": other_repo.display().to_string(),
    });
    assert_eq!(
        super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), None).0,
        None,
        "a call with no run id was admitted to another repository's own base checkout because a \
         single holder's `repo_dir` covered it — the identity-less arm widened on the wrong field"
    );
}

/// A call carrying its own identity cannot reach into another task's
/// worktree by naming it: run A's `--run-id` (or `ESTIGIA_RUN_ID`) restricts
/// `held_by_a_pointer` to run A's own `worktree`, and a sibling
/// worktree only run B's pointer covers is not among them.
#[test]
fn a_call_carrying_an_identity_cannot_name_another_run_s_worktree() {
    let (root, state, launched, run_a, wt_b) = two_pointers_over_one_launch_directory();
    let _ = root;

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": wt_b.display().to_string(),
    });
    assert_eq!(
        super::narrowed_by_the_call(
            &payload,
            "bash",
            &launched,
            state.path(),
            Some(&run_a.run_id)
        )
        .0,
        None,
        "run A's own identity reached a sibling worktree only run B's pointer covers"
    );
}

/// The same payload as the test above, with no run id at all. A worktree holds exactly
/// one task, so resolving it to run B — the run that minted it — is the
/// invariant working correctly, not a fallback: the write is judged against
/// run B's own live claim and recorded under run B's identity, exactly as it
/// would be if run B's own agent had made the call. What a run-id-less call
/// cannot do is tell run A's identity apart from run B's, so it cannot
/// refuse a call that *reaches into* another task's worktree — that flow
/// violation is not enforced on this road, and is named in `docs/honesty.md`.
#[test]
fn without_a_run_id_a_named_worktree_still_resolves_to_the_run_that_holds_it() {
    let (root, state, launched, _run_a, wt_b) = two_pointers_over_one_launch_directory();
    let _ = root;

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": wt_b.display().to_string(),
    });
    let candidate = super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), None)
        .0
        .expect("a directory a holder covers narrowed even with no run id to ask with");
    let resolved = super::checkout_root_covering(state.path(), &candidate, &launched).0;
    assert_eq!(
        resolved,
        Some(wt_b),
        "a call with no run id naming a worktree did not resolve to the run that holds it: \
         {resolved:?}"
    );
}

/// The fixture the identity tests share: two runs, one launch directory, two
/// sibling worktrees — the ordinary shape of two isolated runs started from
/// one repository.
fn two_pointers_over_one_launch_directory() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    std::path::PathBuf,
    harness::session::Run,
    std::path::PathBuf,
) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path().join("repo");
    let wt_a = root.path().join("wt-a");
    let wt_b = root.path().join("wt-b");
    std::fs::create_dir_all(&launched).expect("the launch directory");
    std::fs::create_dir_all(&wt_a).expect("run A's worktree");
    std::fs::create_dir_all(&wt_b).expect("run B's worktree");

    let mut run_a = harness::session::Run::new("claude-issue131c3".to_owned());
    run_a.issue = Some(crate::identity::Identity::Number(131));
    run_a.repo_dir = Some(launched.clone());
    run_a.worktree = Some(wt_a);
    assert!(
        harness::session::store(state.path(), &run_a).expect("run A's pointer writes"),
        "run A's fixture pointer was not stored"
    );

    let mut run_b = harness::session::Run::new("claude-issue131c4".to_owned());
    run_b.issue = Some(crate::identity::Identity::Number(132));
    run_b.repo_dir = Some(launched.clone());
    run_b.worktree = Some(wt_b.clone());
    assert!(
        harness::session::store(state.path(), &run_b).expect("run B's pointer writes"),
        "run B's fixture pointer was not stored"
    );

    (root, state, launched, run_a, wt_b)
}

/// The layout `a_call_carrying_an_identity_cannot_name_another_run_s_worktree`,
/// above, cannot reach, and why: there `repo_dir` and both worktrees are
/// siblings under one temp root, so `repo_dir` is never an ancestor of the
/// worktree named and the bug this guards against never gets a chance to
/// fire. Here `repo_dir` is the base checkout and both worktrees sit
/// **inside** it — a layout `coverage_depth`'s own doc contemplates
/// (`src/paths.rs:124-127`, *"an operator may put those worktrees inside the
/// repository"*) and this repository's own configuration permits. Before this
/// fix, `run.covered()` chained `repo_dir` before `worktree`, so run A's
/// `repo_dir` — an ancestor of run B's nested worktree — admitted a directory
/// that only run B's own `worktree` covers; `checkout_root_covering` would
/// then drop `repo_dir` as a strict ancestor of `launched` and resolve to run
/// B's worktree, handing run A's identity a stale-verdict check bound to run
/// B's own head.
#[test]
fn a_call_carrying_an_identity_cannot_reach_a_sibling_run_s_worktree_nested_inside_the_base() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let base = root.path().join("base");
    let wt_a = base.join("wt-a");
    let wt_b = base.join("wt-b");
    std::fs::create_dir_all(&wt_a).expect("run A's worktree, nested inside the base");
    std::fs::create_dir_all(&wt_b).expect("run B's worktree, nested inside the base");

    let mut run_a = harness::session::Run::new("claude-issue131h9".to_owned());
    run_a.issue = Some(crate::identity::Identity::Number(131));
    run_a.repo_dir = Some(base.clone());
    run_a.worktree = Some(wt_a.clone());
    assert!(
        harness::session::store(state.path(), &run_a).expect("run A's pointer writes"),
        "run A's fixture pointer was not stored"
    );

    let mut run_b = harness::session::Run::new("claude-issue131h0".to_owned());
    run_b.issue = Some(crate::identity::Identity::Number(132));
    run_b.repo_dir = Some(base);
    run_b.worktree = Some(wt_b.clone());
    assert!(
        harness::session::store(state.path(), &run_b).expect("run B's pointer writes"),
        "run B's fixture pointer was not stored"
    );

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": wt_b.display().to_string(),
    });
    assert_eq!(
        super::narrowed_by_the_call(&payload, "bash", &wt_a, state.path(), Some(&run_a.run_id)).0,
        None,
        "run A's identity, launched from its own nested worktree, reached run B's worktree \
         through run A's `repo_dir` covering it"
    );
}

/// The resolved directory is what `stale_verdict` is handed as its `checkout`,
/// and this fix only changes what `context.repo_dir` may resolve to — never
/// `harness`'s own `stale_verdict`, which stays untouched and private to it.
/// For `gh pr merge` that directory decides `same_repository`, and the head the
/// receipt is compared against comes from the tracker; the directory's own
/// `HEAD` is read for `git merge`, `git tag` and `gh release`. Reaching the
/// refusal it produces needs `harness::gate`, which
/// past the coverage check dispatches a live `verify-claim` to the tracker
/// transport; `src/cli/tests.rs` has no rig for that (unlike `tests/pipe.rs`'s
/// `tracker_rig`, which drives the compiled binary as a subprocess against a
/// scripted `gh`), so this test cannot be widened to observe
/// `verdict-bound-to-other-bytes` itself without fabricating a transport this
/// module does not otherwise have. What it does assert is the half this fix
/// actually touches: that the narrowing resolves to the worktree whose head
/// moved, and — reading `HEAD` back from *that resolved directory* — that it is
/// the moved head (`now`, below) and not the earlier one (`reviewed`). Falling
/// back to `launched` instead, the value this test was denied before this fix,
/// carries no `HEAD` of either commit to disagree with, which is why both
/// commits and the read-back are load-bearing rather than decoration.
///
/// This module already drives real `git` elsewhere — `release_repo`, further
/// down this file, is the closer precedent — and a machine where `git` is
/// unusable panics rather than standing this test down: the tolerance of the
/// last `Option` fixture and of this inline one was removed,
/// because a `return;` after an `eprintln!` reports an unmeasured test as `ok`.
#[test]
fn the_narrowing_resolves_to_the_worktree_the_call_is_adjudicated_in() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let state = tempfile::tempdir().expect("a temporary state root");
    let launched = root.path().join("repo");
    let worktree = root.path().join("worktree");
    std::fs::create_dir_all(&launched).expect("the launch directory");
    std::fs::create_dir_all(&worktree).expect("the worktree directory");

    let git_in = |dir: &std::path::Path, args: &[&str]| -> Option<std::process::Output> {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
    };
    let succeeded = |dir: &std::path::Path, args: &[&str]| {
        git_in(dir, args).is_some_and(|out| out.status.success())
    };
    let head_at = |dir: &std::path::Path| -> Option<String> {
        git_in(dir, &["rev-parse", "HEAD"]).and_then(|out| {
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        })
    };

    let setup = (|| -> Option<(String, String)> {
        if !succeeded(&worktree, &["init", "-q"]) {
            return None;
        }
        if !succeeded(&worktree, &["config", "user.email", "t@example.invalid"]) {
            return None;
        }
        if !succeeded(&worktree, &["config", "user.name", "t"]) {
            return None;
        }
        std::fs::write(worktree.join("a.txt"), "one").ok()?;
        if !(succeeded(&worktree, &["add", "-A"])
            && succeeded(&worktree, &["commit", "-qm", "one"]))
        {
            return None;
        }
        let reviewed = head_at(&worktree)?;

        // The push that moves it — the shape a review-then-repair round takes.
        std::fs::write(worktree.join("a.txt"), "two").ok()?;
        if !(succeeded(&worktree, &["add", "-A"])
            && succeeded(&worktree, &["commit", "-qm", "two"]))
        {
            return None;
        }
        let now = head_at(&worktree)?;
        Some((reviewed, now))
    })();
    let (reviewed, now) = setup.unwrap_or_else(|| {
        panic!(
            "this fixture needs a usable `git` that can init, commit and read back a worktree; \
             a test that reports `ok` without running is a defect this repository has filed against itself"
        )
    });
    assert_ne!(
        now, reviewed,
        "the fixture did not move the head, so this scenario proves nothing about a stale verdict"
    );

    let mut run = harness::session::Run::new("claude-issue131d5".to_owned());
    run.issue = Some(crate::identity::Identity::Number(131));
    run.repo_dir = Some(launched.clone());
    run.worktree = Some(worktree.clone());
    // No `reviewed_head` on the pointer: nothing this test reaches reads one,
    // and a field set for atmosphere is a field a later reader takes for a
    // precondition. The moved head is held in `now` and `reviewed` above,
    // where the assertions actually compare it.
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    let payload = serde_json::json!({
        "command": "gh pr merge",
        "workdir": worktree.display().to_string(),
    });
    let candidate =
        super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), Some(&run.run_id))
            .0
            .expect("a call naming this run's own worktree did not narrow");
    let resolved = super::checkout_root_covering(state.path(), &candidate, &launched).0;
    assert_eq!(
        resolved,
        Some(worktree),
        "the narrowing did not resolve to the worktree whose head moved past the reviewed one, \
         which is the directory `stale_verdict` is handed as its `checkout`: {resolved:?}"
    );
    let resolved = resolved.expect("asserted above");

    // `HEAD` read back from the checkout the narrowing resolved to: the
    // directory `stale_verdict` is handed as its `checkout`. It establishes that the resolution
    // names a live checkout sitting at the moved head rather than a path that
    // merely spells one, which is what makes the two commits above matter.
    // It does not reach `stale_verdict` itself — see this test's own doc.
    // Not a stand-down, and not measured either: the assertion above has
    // already pinned `resolved` to the worktree the fixture committed in, so
    // a `None` here is unreachable short of git breaking between two calls.
    // It panics rather than standing down because an unreachable branch must
    // not lie if it ever stops being unreachable — this one printed `SKIPPED`,
    // which is a pass reporting a measurement it never made. On the mutation
    // that removes the fix, `narrowed_by_the_call` returns `None` and the
    // `.expect("a call naming this run's own worktree did not narrow")`
    // above fires first, so nothing reaches this line; that was run, not
    // supposed.
    let head_at_resolved = head_at(&resolved).unwrap_or_else(|| {
        panic!(
            "HEAD was unreadable from {resolved:?}, the checkout the narrowing resolved to, \
             even though git was already proved usable above"
        )
    });
    assert_eq!(
        head_at_resolved, now,
        "the resolved checkout's HEAD was not the moved head: {head_at_resolved}"
    );
    assert_ne!(
        head_at_resolved, reviewed,
        "the resolved checkout's HEAD matched the earlier, reviewed head, so the directory handed \
         to a stale-verdict check does not sit at the moved head"
    );
}

/// The operator's own acceptance, measured against the running system on
/// 2026-08-29: two live claims sharing one launch checkout — the *designed*
/// steady state here, not a conflict — each isolated into its own sibling
/// worktree, and a call at the base carrying no run id at all. A write named
/// at a live worktree must be adjudicated as the run that holds it, not
/// refused `several-runs-hold-this-checkout` merely because another run also
/// shares the base: a worktree holds exactly one task, and the base checkout
/// carries no task identity of its own to be confused with it.
///
/// The shape is `harness::guard::tests::two_holders_with_worktrees` —  two
/// real git worktrees, one pointer per holder, both `repo_dir`-ed at the
/// same base — but `narrowed_by_the_call` and `checkout_root_covering` are
/// private to `src/cli`, so that fixture cannot be called from here without
/// widening either function's visibility; this rebuilds its shape instead,
/// with plain directories rather than real worktrees, since nothing below
/// needs `git` to answer. It stops at the resolved root and the single-
/// holder read `holders_of` gives back at that root: driving this the rest
/// of the way to a `Decision` needs the same live `verify-claim` dispatch the
/// stale-head test above names, which this module has no rig for.
#[test]
fn a_call_with_no_run_id_naming_a_live_worktree_resolves_to_it_and_not_to_the_ambiguity_at_the_base()
 {
    let (root, state, launched, run_a, _wt_b) = two_pointers_over_one_launch_directory();
    let _ = root;
    let worktree_a = run_a
        .worktree
        .clone()
        .expect("the fixture set run A's worktree");

    // The base itself is genuinely ambiguous: two live claims share it, and
    // asking `holders_of` about it answers with both. Naming the base is not
    // this test's scenario — naming *a worktree* is — but this is the
    // ambiguity a run-id-less call would otherwise have to resolve, and it is
    // the reason `held_by_a_pointer` never asks about `launched` itself.
    assert_eq!(
        harness::guard::holders_of(state.path(), &launched).len(),
        2,
        "the fixture does not carry the ambiguity this test exists to route around"
    );

    let payload = serde_json::json!({
        "command": "git commit",
        "workdir": worktree_a.display().to_string(),
    });
    let candidate = super::narrowed_by_the_call(&payload, "bash", &launched, state.path(), None)
        .0
        .expect("a call with no run id naming a live worktree did not narrow");
    let resolved = super::checkout_root_covering(state.path(), &candidate, &launched)
        .0
        .expect("a call with no run id naming a live worktree did not resolve a covering root");
    assert_eq!(
        resolved, worktree_a,
        "the resolved root was not the worktree the call actually named: {resolved:?}"
    );

    let holders = harness::guard::holders_of(state.path(), &resolved);
    assert_eq!(
        holders.len(),
        1,
        "the resolved worktree was still ambiguous between runs: {holders:?}"
    );
    assert_eq!(
        holders[0].run_id, run_a.run_id,
        "the resolved worktree was not adjudicated as the run that holds it"
    );
}

#[test]
fn a_row_that_reads_back_wrong_with_no_override_beside_it_blames_no_file() {
    // `shadowed` ended in `unwrap_or_else(|| "the local override")`, so a row
    // that read back wrong for any reason at all was reported as an operator
    // file overriding it — and the resolution said to change "those rows" in a
    // file that need not exist. That is a diagnosis the tool cannot make,
    // dressed as one it did: the same failure as reporting a state nobody read
    // back, in the sentence an operator acts on.
    // rather than the route to it, and it is worth holding for that reason: a
    // branch nothing exercises is where a message goes back to naming a file.
    let root = tempfile::tempdir().expect("a root with no local override");
    assert!(
        crate::skill::local_override(root.path()).is_none(),
        "the fixture root already carries an override, so this proves nothing"
    );

    let setting = crate::config::Setting::Summary;
    let mut written = Config::default();
    setting
        .apply(&mut written, "Spanish")
        .expect("Spanish is an accepted answer");
    let effective = Config::default();

    let refusal = super::shadowed(root.path(), setting, &written, &effective);
    assert_eq!(refusal.code, "setting-not-read-back");
    let said = format!("{} {}", refusal.message, refusal.resolution);
    assert!(
        !said.contains("estigia.local.md"),
        "a file that is not there was named as the cause: {said}"
    );
    assert!(
        !said.contains("the local override overrides"),
        "the placeholder sentence survived: {said}"
    );
    // What it may say is what it saw: both values, and where it looked.
    assert!(
        said.contains("Spanish") && said.contains(&root.path().display().to_string()),
        "the refusal does not say what was written or where: {said}"
    );

    // And with an override actually there, the old sentence is still the right
    // one — this narrows the message, it does not remove it.
    let with_file = tempfile::tempdir().expect("a root that carries an override");
    let override_path = with_file.path().join("estigia.local.md");
    std::fs::write(&override_path, "# local\n").expect("the override writes");
    // Unconditionally, because an assertion behind an `if` reports pass when it
    assert!(
        crate::skill::local_override(with_file.path()).is_some(),
        "the fixture did not produce the override this half is about"
    );
    let refusal = super::shadowed(with_file.path(), setting, &written, &effective);
    assert_eq!(refusal.code, "setting-shadowed-by-local-file");
    assert!(
        refusal.message.contains("estigia.local.md"),
        "the override was there and went unnamed: {}",
        refusal.message
    );
}

#[test]
fn the_checkout_answers_sentence_is_written_once() {
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    assert_eq!(
        source.matches("this checkout answers").count(),
        1,
        "the sentence `config set` and `config list` share is written more than once, so the two \
         copies can disagree"
    );
}

/// A bare-bones context this file's own `forget_since_nothing_to_release`
/// tests share, standing in one repository.
fn release_context(
    repo_dir: &std::path::Path,
    state_root: &std::path::Path,
) -> harness::GateContext {
    harness::GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: state_root.join("skill"),
        repo_dir: repo_dir.to_path_buf(),
        state_root: state_root.to_path_buf(),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: std::time::Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

/// A repository `same_git_repository` — what the mismatch guard is built on
/// — can actually answer about: it runs `git rev-parse`, and a bare directory
/// answers neither side of that comparison.
///
/// It panics rather than answering `None`, the repair it was given: this
/// is a git workflow harness, so a machine without a usable `git` cannot
/// measure the release guard at all, and a `None` the caller turns into
/// `return;` after an `eprintln!` reports an unmeasured test as `ok`.
fn release_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(directory.path())
        .args(["init", "--quiet"])
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "this fixture needs a usable `git` on PATH and could not run \
                 `git init --quiet`: {error}. Install git, or fix the command, because a \
                 test that reports `ok` without running is a defect this repository has filed against itself."
            )
        });
    assert!(
        output.status.success(),
        "`git init --quiet` failed: {}. This fixture needs a usable `git` on PATH — \
         `docs/honesty.md` records why an unmeasured test must not report `ok`.",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    directory
}

/// The discovery half of `release` finding no epoch of its own to put down
/// forgets the local pointer, so `estigia release --run-id <run-id>` —
/// `several-runs-hold-this-checkout`'s own resolution — actually clears the
/// this pins the one write the discovery answer causes, given the answer
/// already parsed. A tracker read that fails or cannot be parsed never
/// reaches this function at all — `release`'s `tool(…)?` returns before it —
/// so there is nothing to pin about the "removes nothing" half beyond that
/// early return already existing, which
/// `a_run_pointer_nobody_can_read_is_not_a_run_that_holds_nothing` in
/// `tests/pipe.rs` already measures for the two refusals above this one.
#[test]
fn forgetting_a_pointer_the_tracker_says_holds_no_epoch_removes_it() {
    let repo = release_repo();
    let state = tempfile::tempdir().expect("a temporary state root");
    let mut run = harness::session::Run::new("claude-phantom0".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.repo_dir = Some(repo.path().to_path_buf());
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );
    assert_eq!(
        harness::session::load(state.path(), "claude-phantom0").issue,
        Some(crate::identity::Identity::Number(12)),
        "the fixture did not leave a pointer to forget"
    );

    let context = release_context(repo.path(), state.path());
    forget_since_nothing_to_release(
        &context,
        &run,
        "claude-phantom0",
        &crate::identity::Identity::Number(12),
        false,
    )
    .expect("forgetting a pointer with nothing to release is not itself a refusal");

    let after = harness::session::load(state.path(), "claude-phantom0");
    assert_eq!(
        after.issue, None,
        "the pointer is still on disk after the tracker said there was nothing of this run's \
         left to release: {after:?}"
    );
}

/// A discovery read that succeeded against the **wrong** repository must not
/// forget a pointer that names a different one.
///
/// `tool` reaches the tracker through `context.repo_dir` — whichever
/// checkout `release` was launched in — so a successful read there is a
/// successful read of *that* repository's timeline. Without this guard,
/// running `estigia release --run-id <run-id>` from a checkout the pointer
#[test]
fn a_read_against_the_wrong_repository_forgets_nothing() {
    let claimed_in = release_repo();
    let run_from = release_repo();
    let state = tempfile::tempdir().expect("a temporary state root");
    let mut run = harness::session::Run::new("claude-phantom1".to_owned());
    run.issue = Some(crate::identity::Identity::Number(34));
    run.repo_dir = Some(claimed_in.path().to_path_buf());
    assert!(
        harness::session::store(state.path(), &run).expect("the pointer writes"),
        "the fixture pointer was not stored"
    );

    // `release` launched from a *different* repository than the pointer names.
    let context = release_context(run_from.path(), state.path());
    let refusal = forget_since_nothing_to_release(
        &context,
        &run,
        "claude-phantom1",
        &crate::identity::Identity::Number(34),
        false,
    )
    .expect_err("a read against the wrong repository was not refused");
    assert_eq!(refusal.code, "release-repository-mismatch");

    let after = harness::session::load(state.path(), "claude-phantom1");
    assert_eq!(
        after.issue,
        Some(crate::identity::Identity::Number(34)),
        "the pointer was forgotten on a read of a repository it does not name: {after:?}"
    );
}

#[test]
fn a_transport_answer_printed_to_a_person_is_indented_again() {
    // The compact spelling is for the model that pays per byte. This road
    // hands the same string to a terminal, and a 14 KB single line is what
    // the trade must not cost a person.
    let compact = r#"{"ok":true,"issue":12,"run_id":"claude-abcd1234"}"#;
    let shown = indented_for_a_person(compact.to_owned());
    assert!(
        shown.lines().count() > 1,
        "a person was handed the model's single line: {shown}"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&shown).expect("indented is still JSON"),
        serde_json::from_str::<serde_json::Value>(compact).expect("compact is still JSON"),
        "re-indenting changed what the answer says"
    );
}

#[test]
fn an_answer_carrying_a_note_after_the_json_reaches_the_person_whole() {
    // `run_tool` appends prose when the pointer write failed, so the text is
    // not always JSON. A renderer that swallowed what it could not parse
    // would drop exactly the sentence saying the gate is now adjudicating as
    // though the call had not been made.
    let with_note = concat!(
        "{\"ok\":true}\n\n",
        "estigia: this happened on the tracker and could not be written to this run's pointer"
    );
    assert_eq!(
        indented_for_a_person(with_note.to_owned()),
        with_note,
        "an answer that does not parse was not passed through untouched"
    );
}

struct AuthorisationRig {
    _home: tempfile::TempDir,
    options: SetupOptions,
    root: std::path::PathBuf,
    repo: tempfile::TempDir,
}

fn authorisation_rig(agent: &Config) -> AuthorisationRig {
    authorisation_rig_for("claude-code", agent)
}

/// [`authorisation_rig`] for a named adapter: a shared-root adapter keeps its
/// own answers in `estigia.<slug>.md`, an adapter that discovers skills in its contract.
fn authorisation_rig_for(slug: &'static str, agent: &Config) -> AuthorisationRig {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == slug)
        .unwrap_or_else(|| panic!("{slug} is an agent this build knows"));
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(slug, Config::default())].into_iter().collect(),
        rows: [(slug, agent.clone())].into_iter().collect(),
        repository: std::path::PathBuf::new(),
    };
    super::install_planned(&plan, &options, false).expect("the agent installs");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("the adapter resolves its paths")
        .skill_root;
    let repo = tempfile::tempdir().expect("a repository");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout");
    AuthorisationRig {
        _home: home,
        options,
        root,
        repo,
    }
}

fn effective_in(rig: &AuthorisationRig) -> Config {
    crate::skill::config_layers(&rig.root, Some("claude-code"), Some(rig.repo.path()))
        .expect("the layers read")
        .effective()
        .clone()
}

#[test]
fn a_repository_may_ask_where_the_agent_would_not() {
    let rig = authorisation_rig(&Config::default());
    for setting in [Setting::Delivery, Setting::Transitions] {
        super::repository_set(setting, "ask", rig.repo.path(), &rig.options)
            .unwrap_or_else(|refusal| panic!("{} was refused: {refusal}", setting.label()));
    }
    let refusal = super::repository_set(Setting::Models, "unset", rig.repo.path(), &rig.options)
        .expect_err("a repository may not tighten an agent-only row");
    assert_eq!(refusal.code, "setting-not-the-repositorys");
    super::repository_set(Setting::Delivery, "ask 30m", rig.repo.path(), &rig.options)
        .expect("a duration is a tightening too");

    let written = std::fs::read_to_string(crate::skill::repository_config_path(rig.repo.path()))
        .expect("the file was created");
    assert!(
        written.contains("| Delivery authorisation | ask 30m |"),
        "the row is not in the file: {written}"
    );
    assert!(
        !written.contains("Tracker") && !written.contains("Integration"),
        "a row nobody asked for was written: {written}"
    );

    let effective = effective_in(&rig);
    assert!(!effective.delivery.is_autonomous());
    assert!(!effective.transitions.is_autonomous());
    let agent = crate::skill::installed_config_for(&rig.root, Some("claude-code"))
        .expect("the agent reads");
    assert!(
        agent.delivery.is_autonomous(),
        "the agent's own row moved when the repository's was written"
    );
}

#[test]
fn a_repository_cannot_loosen_an_ask_the_contract_holds() {
    // `claude-code` discovers skills, so an install-time agent `ask` lands in the contract.
    let rig = authorisation_rig(&Config {
        delivery: crate::config::Authority::Ask {
            timeout: crate::config::DEFAULT_ASK_TIMEOUT,
        },
        ..Config::default()
    });
    let contract = rig.root.join(crate::skill::CONTRACT);
    let refusal = super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect_err("a repository loosened an ask the contract holds");
    assert_eq!(refusal.code, "repository-cannot-loosen");
    assert!(
        refusal.message.contains(&contract.display().to_string()),
        "the refusal does not name the contract holding the stricter answer: {refusal}"
    );
    let resolution = format!("{}", refusal.resolution);
    assert!(
        resolution.contains("estigia config set") && resolution.contains("--agent claude-code"),
        "the resolution names a command that cannot clear the contract's ask: {resolution}"
    );
    assert!(
        !crate::skill::repository_config_path(rig.repo.path()).exists(),
        "a refused write left a file behind"
    );

    // The command the refusal names clears the block: run it, then the same
    // `--repo auto` is accepted.
    super::config_set(
        "Delivery authorisation",
        "auto",
        Some("claude-code"),
        false,
        false,
        &rig.options,
        false,
    )
    .expect("the named command clears the contract's ask");
    super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect("the repository may answer auto once the contract no longer asks");

    super::repository_set(Setting::Delivery, "ask 2h", rig.repo.path(), &rig.options)
        .expect("an ask over an ask is accepted whatever the durations");
}

/// An `ask` the operator's own file holds is named, with no command: `unlayered()`
/// reads local, then agent, then contract, and only the operator can change that file.
#[test]
fn a_repository_cannot_loosen_an_ask_the_operators_file_holds() {
    let rig = authorisation_rig(&Config::default());
    let local = rig.root.join(crate::config::LOCAL_FILE);
    std::fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n| Delivery authorisation | ask |\n",
    )
    .expect("the operator's file is written");

    let refusal = super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect_err("a repository loosened an ask the operator's file holds");
    assert_eq!(refusal.code, "repository-cannot-loosen");
    assert!(
        refusal.message.contains(&local.display().to_string()),
        "the refusal does not name the operator's file that holds the ask: {refusal}"
    );
    assert!(
        matches!(
            refusal.resolution,
            crate::outcome::Resolution::NoCommand { .. }
        ),
        "the resolution names a command that cannot clear the block: {}",
        refusal.resolution
    );
    assert!(
        !crate::skill::repository_config_path(rig.repo.path()).exists(),
        "a refused write left a file behind"
    );
}

/// An `ask` the agent's own file holds is named, with the command that clears it.
#[test]
fn a_repository_cannot_loosen_an_ask_the_agents_own_file_holds() {
    // `opencode` shares the neutral root, so `config set --agent` writes `estigia.opencode.md`.
    let rig = authorisation_rig_for("opencode", &Config::default());
    let agent_file = crate::skill::agent_override(&rig.root, "opencode");
    let mut asking = Config::default();
    Setting::Delivery
        .apply(&mut asking, "ask")
        .expect("ask is accepted");
    crate::setup::write_agent_configuration_wholly(&agent_file, "opencode", &asking)
        .expect("the agent's own file is written");

    let refusal = super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect_err("a repository loosened an ask the agent's own file holds");
    assert_eq!(refusal.code, "repository-cannot-loosen");
    assert!(
        refusal.message.contains(&agent_file.display().to_string()),
        "the refusal does not name the agent's own file that holds the ask: {refusal}"
    );
    assert!(
        format!("{}", refusal.resolution).contains("--agent opencode"),
        "the resolution does not name the command that clears the block: {}",
        refusal.resolution
    );
    assert!(
        !crate::skill::repository_config_path(rig.repo.path()).exists(),
        "a refused write left a file behind"
    );

    super::config_set(
        "Delivery authorisation",
        "auto",
        Some("opencode"),
        false,
        false,
        &rig.options,
        false,
    )
    .expect("the named command clears the agent's ask");
    super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect("the repository may answer auto once the agent no longer asks");
}

#[test]
fn cautious_on_a_repository_writes_the_two_asks_and_balanced_is_refused() {
    let rig = authorisation_rig(&Config::default());
    let refusal =
        super::repository_set(Setting::Profile, "balanced", rig.repo.path(), &rig.options)
            .expect_err("balanced would be a grant");
    assert_eq!(refusal.code, "repository-cannot-loosen");
    super::repository_set(Setting::Profile, "cautious", rig.repo.path(), &rig.options)
        .expect("cautious is a tightening");
    let written = std::fs::read_to_string(crate::skill::repository_config_path(rig.repo.path()))
        .expect("the file was created");
    for label in ["Delivery authorisation", "Transition authorisation"] {
        assert!(
            written.contains(&format!("| {label} | ask |")),
            "{label} is not `ask` in the file: {written}"
        );
    }
    assert!(!written.contains("Model routing"));
    assert!(!written.contains("Profile"));
}

#[test]
fn the_issue_requirement_is_written_to_the_repository_file_and_nowhere_else() {
    let rig = authorisation_rig(&Config::default());
    super::repository_set(
        Setting::IssueRequirement,
        "required",
        rig.repo.path(),
        &rig.options,
    )
    .expect("the repository may answer its own row");
    let written = std::fs::read_to_string(crate::skill::repository_config_path(rig.repo.path()))
        .expect("the file was created");
    assert!(
        written.contains("| Unclaimed write policy | required |"),
        "{written}"
    );
    let layered = crate::skill::layer_repository(&Config::default(), rig.repo.path())
        .expect("the file reads back");
    assert_eq!(
        layered.issue_requirement,
        crate::config::IssueRequirement::Required
    );

    let refusal = super::config_set(
        "Issue requirement",
        "off",
        None,
        false,
        false,
        &rig.options,
        false,
    )
    .expect_err("a machine-wide value was accepted");
    assert_eq!(refusal.code, "setting-only-the-repositorys");
    let refusal = super::config_set(
        "Issue requirement",
        "off",
        Some("claude-code"),
        false,
        false,
        &rig.options,
        false,
    )
    .expect_err("a per-agent value was accepted");
    assert_eq!(refusal.code, "setting-not-per-agent");
}

#[test]
fn the_workflow_is_written_to_the_repository_file_and_nowhere_else() {
    let rig = authorisation_rig(&Config::default());
    super::repository_set(Setting::Workflow, "assisted", rig.repo.path(), &rig.options)
        .expect("the repository may answer its own row");
    let written = std::fs::read_to_string(crate::skill::repository_config_path(rig.repo.path()))
        .expect("the file was created");
    assert!(
        written.contains("| Claim workflow | assisted |"),
        "{written}"
    );
    let layered = crate::skill::layer_repository(&Config::default(), rig.repo.path())
        .expect("the file reads back");
    assert_eq!(layered.workflow, crate::config::Workflow::Assisted);

    let refusal = super::config_set(
        "Workflow",
        "autonomous",
        None,
        false,
        false,
        &rig.options,
        false,
    )
    .expect_err("a machine-wide value was accepted");
    assert_eq!(refusal.code, "setting-only-the-repositorys");
    let refusal = super::config_set(
        "Workflow",
        "autonomous",
        Some("claude-code"),
        false,
        false,
        &rig.options,
        false,
    )
    .expect_err("a per-agent value was accepted");
    assert_eq!(refusal.code, "setting-not-per-agent");
}

#[test]
fn an_unreadable_contract_names_no_workflow_at_all() {
    for installed in crate::config::Workflow::all() {
        assert_eq!(super::effective_workflow(false, installed), Some(installed));
        assert_eq!(super::effective_workflow(true, installed), None);
    }
}

#[test]
fn model_routing_stays_the_agents_alone() {
    let rig = authorisation_rig(&Config::default());
    for (setting, value) in [(Setting::Models, "unset"), (Setting::LoopPlugin, "on")] {
        let refusal = super::repository_set(setting, value, rig.repo.path(), &rig.options)
            .expect_err("an agent-only row was written for a repository");
        assert_eq!(refusal.code, "setting-not-the-repositorys");
    }
}

#[test]
fn a_repository_that_tightens_is_not_broken_for_doctor() {
    let rig = authorisation_rig(&Config::default());
    let bin = crate::test_env::scripted_gh();
    let answers =
        serde_json::json!([{ "matches": "auth status", "stdout": "", "status": 0 }]).to_string();
    let broken = |rig: &AuthorisationRig| {
        crate::test_env::with_scripted_gh(bin.path(), &answers, || {
            crate::harness::doctor::full(
                Some(&rig.root),
                rig.repo.path(),
                &Config::default().tracker,
                &rig.options,
            )
        })
        .iter()
        .filter(|check| matches!(check.health, crate::harness::doctor::Health::Broken { .. }))
        .map(|check| check.name)
        .collect::<Vec<_>>()
    };
    let before = broken(&rig);
    assert!(
        !before.contains(&"gh"),
        "the scripted gh was not the one doctor asked: {before:?}"
    );
    super::repository_set(Setting::Delivery, "ask", rig.repo.path(), &rig.options)
        .expect("the repository tightens");
    assert_eq!(
        broken(&rig),
        before,
        "doctor called a tightening repository broken"
    );
}

fn stop_input(session_id: &str, stop_hook_active: bool) -> harness::hook::Input {
    harness::hook::Input {
        session_id: session_id.to_owned(),
        stop_hook_active,
        ..harness::hook::Input::default()
    }
}

fn stop_answer(
    agent: &str,
    input: &harness::hook::Input,
    context: &harness::GateContext,
    script: &str,
) -> serde_json::Value {
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_scripted_gh(bin.path(), script, || {
        super::loop_stop::answer(
            harness::hook::Dialect::ClaudeCode,
            Some(agent),
            harness::hook::Event::Stop,
            input,
            Some(context),
        )
    })
}

fn stop_rig(session_id: &str) -> (tempfile::TempDir, harness::GateContext, String) {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("codex", session_id);
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    (root, context, run_id)
}

/// The script for a queue read: the run's own liveness answer plus the one open
/// list every state is filtered from, since the local store decides the state.
fn queues(issues: serde_json::Value) -> Vec<serde_json::Value> {
    vec![
        crate::test_env::not_current_live_holder_issue(7),
        open_answer(issues),
    ]
}

#[test]
fn a_codex_stop_blocks_with_the_claim_reason_while_its_run_holds_a_live_claim_even_when_already_active()
 {
    let (_root, context, run_id) = stop_rig("codex-stop-live");
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    for active in [false, true] {
        let answer = stop_answer(
            "codex",
            &stop_input("codex-stop-live", active),
            &context,
            &script,
        );
        assert_eq!(
            answer,
            harness::hook::stop_block(&crate::identity::Identity::Number(7), &run_id),
            "stop_hook_active {active}"
        );
    }
}

#[test]
fn a_codex_stop_with_no_claim_and_claimable_work_continues_with_the_shared_prompt_however_often_it_is_asked()
 {
    let (_root, context, run_id) = stop_rig("codex-stop-claimable");
    seed_store(&[
        (21, "ready", "Body.", Vec::new()),
        (22, "ready", "Body.", Vec::new()),
    ]);
    let script = crate::test_env::answers(&queues(serde_json::json!([
        claimable_issue(21),
        claimable_issue(22)
    ])));
    for active in [false, true, true] {
        let answer = stop_answer(
            "codex",
            &stop_input("codex-stop-claimable", active),
            &context,
            &script,
        );
        assert_eq!(
            answer["decision"], "block",
            "stop_hook_active {active}: {answer}"
        );
        assert_eq!(
            answer["reason"],
            harness::loop_text::continuation(harness::loop_text::Invocation::Mention, &run_id, 0),
            "stop_hook_active {active}"
        );
        assert!(
            answer["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("codex-") && reason.ends_with("$flow dev"))
        );
    }
}

#[test]
fn a_codex_stop_with_only_held_work_lets_the_turn_end_and_says_what_it_waits_on() {
    let (_root, context, _) = stop_rig("codex-stop-held");
    seed_store(&[(
        40,
        "in-progress",
        "Body.",
        vec![held_claim("someone-e", &"e".repeat(32))],
    )]);
    let script = crate::test_env::answers(&queues(serde_json::json!([claimable_issue(40)])));
    let answer = stop_answer(
        "codex",
        &stop_input("codex-stop-held", false),
        &context,
        &script,
    );
    assert!(answer.get("decision").is_none(), "{answer}");
    let message = answer["systemMessage"].as_str().unwrap_or_default();
    assert!(
        message.contains("1 in progress or in review") && message.contains("cannot wait"),
        "{answer}"
    );
}

fn leave_mail(context: &harness::GateContext, run_id: &str, body: &str) {
    crate::transport::store::Store::open(&crate::transport::store::beside(&context.state_root))
        .expect("the store opens")
        .message_sent(
            &crate::transport::store::MessageSent {
                sender_run_id: None,
                recipient_run_id: run_id,
                body,
            },
            10,
        )
        .expect("the message is recorded");
}

fn mail_waiting(context: &harness::GateContext, run_id: &str) -> Option<u64> {
    crate::transport::store::Store::open(&crate::transport::store::beside(&context.state_root))
        .expect("the store opens")
        .messages_waiting_for()
        .expect("counts")
        .get(run_id)
        .copied()
}

#[test]
fn a_codex_stop_carries_waiting_mail_and_the_loop_continuation_in_one_reason() {
    let (_root, context, run_id) = stop_rig("codex-stop-mail-live");
    leave_mail(&context, &run_id, "the review is done");
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    let answer = stop_answer(
        "codex",
        &stop_input("codex-stop-mail-live", false),
        &context,
        &script,
    );
    let held = harness::hook::stop_block(&crate::identity::Identity::Number(7), &run_id);
    let reason = answer["reason"].as_str().unwrap_or_default();
    assert_eq!(answer["decision"], "block", "{answer}");
    assert!(
        reason.starts_with(held["reason"].as_str().expect("a reason"))
            && reason.contains("the review is done"),
        "{answer}"
    );
    assert_eq!(mail_waiting(&context, &run_id), None);
}

#[test]
fn a_codex_stop_with_nothing_to_continue_still_blocks_on_waiting_mail_and_keeps_the_loop_note() {
    let (_root, context, run_id) = stop_rig("codex-stop-mail-held");
    leave_mail(&context, &run_id, "look at this");
    seed_store(&[(
        40,
        "in-progress",
        "Body.",
        vec![held_claim("someone-e", &"e".repeat(32))],
    )]);
    let script = crate::test_env::answers(&queues(serde_json::json!([claimable_issue(40)])));
    let answer = stop_answer(
        "codex",
        &stop_input("codex-stop-mail-held", false),
        &context,
        &script,
    );
    assert_eq!(answer["decision"], "block", "{answer}");
    assert!(
        answer["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("look at this")),
        "{answer}"
    );
    assert!(answer["systemMessage"].is_string(), "{answer}");
    assert_eq!(mail_waiting(&context, &run_id), None);
}

#[test]
fn claude_code_stop_answers_as_before_and_never_claims_the_next_issue() {
    let (_root, context, _) = stop_rig("claude-stop-claimable");
    seed_store(&[(21, "ready", "Body.", Vec::new())]);
    let script = crate::test_env::answers(&queues(serde_json::json!([claimable_issue(21)])));
    for active in [false, true] {
        let input = stop_input("claude-stop-claimable", active);
        let bin = crate::test_env::scripted_gh();
        let direct = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            harness::hook::run_as(
                harness::hook::Dialect::ClaudeCode,
                Some("claude-code"),
                harness::hook::Event::Stop,
                &input,
                Some(&context),
            )
        });
        assert_eq!(
            stop_answer("claude-code", &input, &context, &script),
            direct,
            "Claude Code's Stop answer changed (stop_hook_active {active})"
        );
        assert_eq!(direct, serde_json::json!({}));
    }
}

/// A `Stop` rig for a Claude loop: a run with issue 7 and a stored, running
/// loop.
fn claude_loop_rig(
    session_id: &str,
    max: Option<u64>,
) -> (tempfile::TempDir, harness::GateContext, String) {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    git_repo_with_origin(&repo);
    let run_id = harness::session::run_id("claude", session_id);
    goal_pointer(&state, &run_id, Some(7));
    let context = release_context(&repo, &state);
    harness::session::set_loop_goal(
        &context.state_root,
        &run_id,
        &harness::session::LoopGoal {
            objective: "/flow dev".to_owned(),
            max,
            stopped: None,
            last: None,
        },
    )
    .expect("the loop is stored");
    (root, context, run_id)
}

#[test]
fn a_claude_stop_with_a_stored_loop_continues_with_the_slash_prompt() {
    let (_root, context, run_id) = claude_loop_rig("claude-loop-claimable", None);
    seed_store(&[
        (21, "ready", "Body.", Vec::new()),
        (22, "ready", "Body.", Vec::new()),
    ]);
    let script = crate::test_env::answers(&queues(serde_json::json!([
        claimable_issue(21),
        claimable_issue(22)
    ])));
    for active in [false, true] {
        let answer = stop_answer(
            "claude-code",
            &stop_input("claude-loop-claimable", active),
            &context,
            &script,
        );
        assert_eq!(
            answer["decision"], "block",
            "stop_hook_active {active}: {answer}"
        );
        assert_eq!(
            answer["reason"],
            harness::loop_text::continuation(harness::loop_text::Invocation::Slash, &run_id, 0),
            "stop_hook_active {active}"
        );
        assert!(
            answer["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("claude-") && reason.ends_with("/flow dev"))
        );
    }
    let goal = harness::session::loop_goal(&context.state_root, &run_id)
        .expect("the loop is still stored");
    assert!(goal.last.is_some(), "the answer was not recorded");
    assert!(
        goal.stopped.is_none(),
        "a continued loop was marked stopped"
    );
}

#[test]
fn a_claude_stop_with_a_stopped_loop_stands_aside() {
    let (_root, context, run_id) = claude_loop_rig("claude-loop-stopped", None);
    let mut goal = harness::session::loop_goal(&context.state_root, &run_id).expect("stored");
    goal.stopped = Some(harness::session::LoopEnd {
        reason: "the completion cap (1) was reached".to_owned(),
        at: 0,
    });
    harness::session::set_loop_goal(&context.state_root, &run_id, &goal)
        .expect("the stop is stored");

    seed_store(&[(21, "ready", "Body.", Vec::new())]);
    let script = crate::test_env::answers(&queues(serde_json::json!([claimable_issue(21)])));
    assert_eq!(
        stop_answer(
            "claude-code",
            &stop_input("claude-loop-stopped", false),
            &context,
            &script
        ),
        serde_json::json!({}),
        "a stopped loop continued"
    );
}

#[test]
fn a_claude_stop_honours_a_stop_after_request_and_marks_the_loop_stopped() {
    let (_root, context, run_id) = claude_loop_rig("claude-loop-stopofter", None);
    harness::session::set_stop_after(
        &context.state_root,
        &run_id,
        Some("the operator stopped the loop"),
    )
    .expect("the request is written");
    seed_store(&[(21, "ready", "Body.", Vec::new())]);
    let script = crate::test_env::answers(&queues(serde_json::json!([claimable_issue(21)])));
    let answer = stop_answer(
        "claude-code",
        &stop_input("claude-loop-stopofter", false),
        &context,
        &script,
    );
    assert!(answer.get("decision").is_none(), "{answer}");
    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("stop after the current task")),
        "{answer}"
    );
    assert!(
        harness::session::stop_after(&context.state_root, &run_id).is_none(),
        "the request was not consumed"
    );
    let goal = harness::session::loop_goal(&context.state_root, &run_id)
        .expect("the loop is still stored");
    assert!(goal.stopped.is_some(), "the loop was not marked stopped");
}

/// The terminal stop is durable before the request is consumed: an unwritable
/// loop record leaves the stop-after request for the next turn rather than
/// losing the operator's stop.
#[test]
#[cfg(unix)]
fn a_claude_stop_keeps_the_stop_after_request_when_the_loop_record_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;

    let (_root, context, run_id) = claude_loop_rig("claude-loop-unwritable", None);
    harness::session::set_stop_after(
        &context.state_root,
        &run_id,
        Some("the operator stopped the loop"),
    )
    .expect("the request is written");

    let directory = harness::session::loop_dir(&context.state_root);
    let writable = directory
        .metadata()
        .expect("the loop directory")
        .permissions();
    let mut readonly = writable.clone();
    readonly.set_mode(0o555);
    std::fs::set_permissions(&directory, readonly).expect("a read-only loop directory");

    seed_store(&[(21, "ready", "Body.", Vec::new())]);
    let script = crate::test_env::answers(&queues(serde_json::json!([claimable_issue(21)])));
    let answer = stop_answer(
        "claude-code",
        &stop_input("claude-loop-unwritable", false),
        &context,
        &script,
    );

    std::fs::set_permissions(&directory, writable).expect("the loop directory is writable again");

    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("stop after the current task")),
        "the stop was not reported: {answer}"
    );
    assert!(
        harness::session::stop_after(&context.state_root, &run_id).is_some(),
        "the request was consumed even though the terminal stop could not be recorded"
    );
    let goal = harness::session::loop_goal(&context.state_root, &run_id)
        .expect("the loop is still stored");
    assert!(
        goal.stopped.is_none(),
        "the loop read as stopped even though the record was not written"
    );
}

/// A stopped loop is terminal for the loop, not for the gate: a run still
/// holding a live claim gets the ordinary claim-holding answer.
#[test]
fn a_stopped_claude_loop_still_lets_the_gate_hold_its_claim() {
    let (_root, context, run_id) = claude_loop_rig("claude-loop-stopped-live", None);
    let mut goal = harness::session::loop_goal(&context.state_root, &run_id).expect("stored");
    goal.stopped = Some(harness::session::LoopEnd {
        reason: "the completion cap (1) was reached".to_owned(),
        at: 0,
    });
    harness::session::set_loop_goal(&context.state_root, &run_id, &goal)
        .expect("the stop is stored");

    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        &run_id,
        7,
        "in-progress",
    )]);
    assert_eq!(
        stop_answer(
            "claude-code",
            &stop_input("claude-loop-stopped-live", false),
            &context,
            &script
        ),
        harness::hook::stop_block(&crate::identity::Identity::Number(7), &run_id),
        "a stopped loop skipped the ordinary claim-holding Stop"
    );
}

/// Starting a loop clears a stop-after request left by the previous one, so a
/// restart is not ended before it opens anything.
#[test]
fn starting_a_claude_loop_clears_a_stop_after_request_left_by_a_previous_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout the tracker reads in");
    let run_id = harness::session::run_id("claude", "claude-loop-restart");
    let context = release_context(&repo, &state);
    harness::session::set_stop_after(&context.state_root, &run_id, Some("an earlier stop"))
        .expect("the request is written");

    let stored =
        super::start_loop(&context, &run_id, "do the next thing").expect("the loop starts");
    assert!(
        matches!(stored, super::StartOutcome::Stored(_)),
        "start did not store a loop"
    );
    assert!(
        harness::session::stop_after(&context.state_root, &run_id).is_none(),
        "a stop-after request survived into the new loop"
    );
    let goal =
        harness::session::loop_goal(&context.state_root, &run_id).expect("the loop is stored");
    assert_eq!(goal.objective, "/flow do the next thing");
    assert!(goal.stopped.is_none(), "the new loop is already stopped");
}

#[test]
fn a_loop_stops_at_its_completion_cap() {
    let mut over = reading(
        "stopped",
        false,
        [Some(5), Some(5), Some(0), Some(0), Some(0)],
    );
    over.completed = Some(2);
    let answer = super::loop_stop::decide(
        "claude-abc",
        harness::loop_text::Invocation::Slash,
        Some(1),
        || {},
        || over.clone(),
    )
    .into_value();
    assert!(answer.get("decision").is_none(), "{answer}");
    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("completion cap (1)")),
        "{answer}"
    );

    let mut under = reading(
        "stopped",
        false,
        [Some(5), Some(0), Some(0), Some(0), Some(0)],
    );
    under.completed = Some(0);
    let answer = super::loop_stop::decide(
        "claude-abc",
        harness::loop_text::Invocation::Slash,
        Some(2),
        || {},
        || under.clone(),
    )
    .into_value();
    assert_eq!(answer["decision"], "block", "{answer}");

    let unknown = reading(
        "stopped",
        false,
        [Some(5), Some(0), Some(0), Some(0), Some(0)],
    );
    let answer = super::loop_stop::decide(
        "claude-abc",
        harness::loop_text::Invocation::Slash,
        Some(2),
        || {},
        || unknown.clone(),
    )
    .into_value();
    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("did not report how many issues")),
        "{answer}"
    );
}

fn reading(standing: &'static str, live: bool, counts: [Option<u64>; 5]) -> super::GoalReading {
    super::GoalReading {
        issue: live.then_some(crate::identity::Identity::Number(7)),
        state: None,
        standing,
        live,
        reason: None,
        claimable: counts[0],
        dischargeable: counts[1],
        held: counts[2],
        pending: counts[3],
        unconditioned: counts[4],
        unadmitted: None,
        completed: None,
        stop_after: None,
    }
}

fn decided(readings: Vec<super::GoalReading>) -> (serde_json::Value, usize) {
    let mut asked = 0;
    let mut readings = readings.into_iter();
    let answer = super::loop_stop::decide(
        "codex-abc",
        crate::harness::loop_text::Invocation::Mention,
        None,
        || {},
        || {
            asked += 1;
            readings.next().expect("no more readings were scripted")
        },
    )
    .into_value();
    (answer, asked)
}

#[test]
fn a_codex_stop_retries_an_unconfirmed_claim_before_it_continues_or_stops() {
    // A later confirmed read continues the loop rather than stopping on the
    // earlier unknown ones, however many of them there were.
    let (answer, asked) = decided(vec![
        reading(
            "unread",
            false,
            [Some(5), Some(0), Some(0), Some(0), Some(0)],
        ),
        reading("unreadable", false, [None; 5]),
        reading("passed", true, [None; 5]),
    ]);
    assert_eq!(asked, 3, "an unconfirmed read was not retried: {answer}");
    assert_eq!(answer["decision"], "block", "{answer}");

    // Reads that stay unconfirmed past the bound stop, and say the read was
    // retried rather than naming a second read.
    let (answer, asked) = decided(vec![
        reading(
            "unread",
            false,
            [Some(5), Some(0), Some(0), Some(0), Some(0)],
        ),
        reading(
            "unread",
            false,
            [Some(5), Some(0), Some(0), Some(0), Some(0)],
        ),
        reading(
            "unread",
            false,
            [Some(5), Some(0), Some(0), Some(0), Some(0)],
        ),
    ]);
    assert_eq!(asked, 3);
    assert!(
        answer.get("decision").is_none(),
        "claimable work made an exhausted retry a continue: {answer}"
    );
    let said = answer["systemMessage"].as_str().expect("the message");
    assert!(
        said.contains("could not confirm") && said.contains("unread") && said.contains("2 retries"),
        "the exhausted stop did not say the read was retried: {answer}"
    );
    assert!(
        !said.contains("second read"),
        "the exhausted stop still names a second read: {answer}"
    );

    // A confirmed first read is asked for once and only once.
    let (_, asked) = decided(vec![reading("passed", true, [None; 5])]);
    assert_eq!(asked, 1, "a confirmed answer was asked for twice");
}

fn unadmitted_reading(unadmitted: Option<u64>) -> super::GoalReading {
    super::GoalReading {
        unadmitted,
        ..reading(
            "nothing-sworn",
            false,
            [Some(0), Some(0), Some(0), Some(0), Some(0)],
        )
    }
}

#[test]
fn a_stop_with_unadmitted_issues_names_the_count_and_is_not_terminal() {
    let answer = super::loop_stop::decide(
        "codex-abc",
        crate::harness::loop_text::Invocation::Mention,
        None,
        || {},
        || unadmitted_reading(Some(3)),
    );
    let super::loop_stop::Decision::Stop {
        reason, terminal, ..
    } = answer
    else {
        panic!("unadmitted issues made the loop continue");
    };
    assert!(!terminal, "an unadmitted backlog ended the loop for good");
    assert!(
        reason.contains("3 open issue(s)") && reason.contains("`transition`"),
        "{reason}"
    );
}

#[test]
fn a_stop_with_no_unadmitted_issues_keeps_its_terminal_reason() {
    for unadmitted in [Some(0), None] {
        let answer = super::loop_stop::decide(
            "codex-abc",
            crate::harness::loop_text::Invocation::Mention,
            None,
            || {},
            || unadmitted_reading(unadmitted),
        );
        let super::loop_stop::Decision::Stop {
            reason, terminal, ..
        } = answer
        else {
            panic!("an empty queue continued");
        };
        assert!(terminal, "{unadmitted:?}");
        assert_eq!(
            reason,
            "Estigia reports no live claim and no claimable or dischargeable work"
        );
    }
}

#[test]
fn an_unconfirmed_read_waits_between_attempts() {
    let readings = [
        reading("unread", false, [None; 5]),
        reading("unread", false, [None; 5]),
        reading("passed", true, [None; 5]),
    ];
    let mut readings = readings.into_iter();
    let mut asked = 0;
    let mut pauses = 0;
    let answer = super::loop_stop::decide(
        "codex-abc",
        crate::harness::loop_text::Invocation::Mention,
        None,
        || pauses += 1,
        || {
            asked += 1;
            readings.next().expect("no more readings were scripted")
        },
    )
    .into_value();
    assert_eq!(asked, 3);
    assert_eq!(
        pauses, 2,
        "the retry read three times without waiting between attempts"
    );
    assert_eq!(answer["decision"], "block", "{answer}");
}

#[test]
fn a_codex_stop_lets_the_turn_end_when_a_queue_count_is_unreadable_or_nothing_can_open() {
    let (answer, _) = decided(vec![reading(
        "stopped",
        false,
        [Some(1), None, None, None, None],
    )]);
    assert!(answer.get("decision").is_none(), "{answer}");
    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("did not report how much work")),
        "{answer}"
    );

    let (answer, _) = decided(vec![reading(
        "nothing-sworn",
        false,
        [Some(0), Some(0), Some(0), Some(0), Some(2)],
    )]);
    assert!(answer.get("decision").is_none(), "{answer}");
    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("no claimable or dischargeable work")
                && text.contains("2 blocked with no recorded condition")),
        "{answer}"
    );

    let (answer, _) = decided(vec![reading(
        "nothing-sworn",
        false,
        [Some(0), Some(0), None, Some(0), Some(0)],
    )]);
    assert!(
        answer["systemMessage"]
            .as_str()
            .is_some_and(|text| text.contains("what is held or pending could not be read")),
        "{answer}"
    );
}

#[test]
fn a_codex_stop_counts_dischargeable_work_in_the_prompt() {
    let (answer, _) = decided(vec![reading(
        "nothing-sworn",
        false,
        [Some(0), Some(2), Some(0), Some(0), Some(0)],
    )]);
    assert_eq!(answer["decision"], "block", "{answer}");
    assert_eq!(
        answer["reason"],
        harness::loop_text::continuation(harness::loop_text::Invocation::Mention, "codex-abc", 2)
    );
}

/// A stop-after request ends the Codex loop once the claim is released, and the
/// request is consumed; a live claim keeps the loop going regardless of it.
#[test]
fn a_codex_stop_honours_a_stop_after_request_once_the_claim_is_released() {
    let reason = "the operator asked to stop after the current task";
    let requested = || {
        let mut reading = reading(
            "stopped",
            false,
            [Some(2), Some(0), Some(0), Some(0), Some(0)],
        );
        reading.stop_after = Some(reason.to_owned());
        reading
    };

    let decision = super::loop_stop::decide(
        "codex-abc",
        crate::harness::loop_text::Invocation::Mention,
        None,
        || {},
        requested,
    );
    assert!(
        decision.consumes_stop_request(),
        "a stop-after request did not ask for the stop"
    );
    let answer = decision.into_value();
    assert!(
        answer.get("decision").is_none(),
        "a stop-after request still queued another turn: {answer}"
    );
    assert!(
        answer["systemMessage"].as_str().is_some_and(|text| text
            .contains("stop after the current task")
            && text.contains(reason)),
        "the stop did not name the request: {answer}"
    );

    let live = super::loop_stop::decide(
        "codex-abc",
        crate::harness::loop_text::Invocation::Mention,
        None,
        || {},
        || {
            let mut reading = reading("passed", true, [None; 5]);
            reading.stop_after = Some(reason.to_owned());
            reading
        },
    );
    assert!(
        !live.consumes_stop_request(),
        "a live claim consumed the request"
    );
    assert_eq!(
        live.into_value(),
        harness::hook::stop_block(&crate::identity::Identity::Number(7), "codex-abc"),
        "a live claim did not keep the loop going past a pending request"
    );

    let (answer, _) = decided(vec![reading(
        "stopped",
        false,
        [Some(2), Some(0), Some(0), Some(0), Some(0)],
    )]);
    assert_eq!(
        answer["decision"], "block",
        "a run with no request stopped queuing work: {answer}"
    );
}

#[test]
fn a_codex_stop_with_no_identity_or_no_payload_stands_aside_as_before() {
    let (_root, context, _) = stop_rig("codex-stop-nameless");
    let nameless = stop_input("", false);
    assert_eq!(
        stop_answer("codex", &nameless, &context, "[]"),
        serde_json::json!({})
    );
    let unreadable = harness::hook::Input {
        unreadable: true,
        ..stop_input("codex-stop-nameless", false)
    };
    assert_eq!(
        stop_answer("codex", &unreadable, &context, "[]"),
        serde_json::json!({})
    );
}

fn tree_of(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut found = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                found.insert(path, bytes);
            }
        }
    }
    found
}

fn two_installed_agents() -> (
    tempfile::TempDir,
    SetupOptions,
    &'static AgentAdapter,
    &'static AgentAdapter,
) {
    let (home, options) = sandbox();
    let claude = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let codex = crate::setup::find_agent("codex").expect("Codex is an adapter");
    crate::setup::setup(claude, &Config::default(), &options).expect("Claude installs");
    crate::setup::setup(codex, &Config::default(), &options).expect("Codex installs");
    (home, options, claude, codex)
}

fn edited(setting: Setting, value: &str) -> Config {
    let mut config = Config::default();
    setting
        .apply(&mut config, value)
        .expect("the value is accepted");
    config
}

fn plan_over(agents: &[&'static AgentAdapter], rows: &[Config]) -> crate::tui::Plan {
    crate::tui::Plan {
        agents: agents.to_vec(),
        opened: agents
            .iter()
            .map(|adapter| (adapter.slug, Config::default()))
            .collect(),
        rows: agents
            .iter()
            .zip(rows)
            .map(|(adapter, row)| (adapter.slug, row.clone()))
            .collect(),
        repository: std::path::PathBuf::new(),
    }
}

#[test]
fn an_agent_row_runs_setup_for_that_agent_and_leaves_the_other_byte_identical() {
    let (home, options, claude, codex) = two_installed_agents();
    let plan = plan_over(
        &[claude, codex],
        &[edited(Setting::Delivery, "ask"), Config::default()],
    );
    assert_eq!(super::agents_to_touch(&plan, &options), vec![claude]);
    let codex_files = |tree: std::collections::BTreeMap<_, _>| {
        tree.into_iter()
            .filter(|(path, _): &(std::path::PathBuf, Vec<u8>)| {
                path.starts_with(home.path().join(".codex"))
            })
            .collect::<Vec<_>>()
    };
    let before = codex_files(tree_of(home.path()));
    assert!(!before.is_empty(), "Codex left nothing to compare");

    super::install_planned(&plan, &options, false).expect("the save lands");

    assert_eq!(codex_files(tree_of(home.path())), before);
}

#[test]
fn an_everywhere_row_runs_setup_for_every_agent_it_lands_in() {
    let (_home, options, claude, codex) = two_installed_agents();
    let row = edited(Setting::Tracker, "github acme/issues");
    let plan = plan_over(&[claude, codex], &[row.clone(), row]);

    assert_eq!(super::agents_to_touch(&plan, &options), vec![claude, codex]);
}

#[test]
fn a_failure_in_an_untouched_agent_does_not_reach_the_save() {
    let (_home, options, claude, codex) = two_installed_agents();
    let plan = plan_over(
        &[claude, codex],
        &[edited(Setting::Delivery, "ask"), Config::default()],
    );
    crate::setup::inject_setup_failure(codex.slug, crate::setup::SetupFailureBoundary::BeforeSkill);

    super::install_planned(&plan, &options, false).expect("Codex was never asked to run");
}

#[test]
fn a_save_with_nothing_changed_writes_nothing_and_says_so() {
    let (home, options, claude, codex) = two_installed_agents();
    let plan = plan_over(&[claude, codex], &[Config::default(), Config::default()]);
    let before = tree_of(home.path());

    let (_, report) = super::install_planned(&plan, &options, false).expect("a no-op save");

    assert_eq!(tree_of(home.path()), before);
    assert_eq!(report, "nothing changed, nothing written");
}

/// The population [`super::INSTALL_PLANNED_CALLERS_POPULATION`] declares, enforced.
///
/// `install_planned` deploys adapter assets and is itself ungated: what makes
/// the guided route safe is that `guided_install` calls `lifecycle_preflight`
/// first. Nothing held the two together, so a second production caller added
/// later would write adapter files with no lifecycle read and the suite would
/// stay green. This reads the stripped source of every `src/` file outside a
/// `*tests.rs` module, finds each call site of `install_planned`, and fails,
/// naming the rule, when the enclosing function carries no `lifecycle_preflight`
/// call before it.
///
/// It is a text guard. What it does not hold: a call written inside a
/// `#[cfg(test)]` block in a production file (this walk does not split those
/// out), or one reached through a macro, a function pointer or a `use` alias.
/// It catches a caller copied or moved without the preflight in front of it,
/// which is how the second caller would arrive; deliberate circumvention is not
/// in reach of reading text. `docs/honesty.md` records that boundary.
#[test]
fn every_production_caller_of_install_planned_is_preceded_by_the_lifecycle_preflight() {
    let mut offenders: Vec<String> = Vec::new();
    let mut callers = 0usize;
    let mut shipping_files = 0usize;
    let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("a readable source directory") {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            // A test module is not shipping code, and the tests that drive
            // `install_planned` directly live in `tests.rs` files.
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy() == "tests.rs")
            {
                continue;
            }
            shipping_files += 1;
            let relative = path
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .expect("every source sits under the crate root")
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            let source = std::fs::read_to_string(&path).expect("a readable source");
            // Strip comments first, so a comment naming `install_planned(` or
            // `lifecycle_preflight(` is not read as code.
            let stripped = crate::source_text::code_of(&source);
            let lines: Vec<&str> = stripped.lines().collect();
            for (number, line) in lines.iter().enumerate() {
                if !line.contains("install_planned(") || line.contains("fn install_planned(") {
                    continue;
                }
                callers += 1;
                let start = (0..number)
                    .rev()
                    .find(|&earlier| starts_a_function(lines[earlier]))
                    .unwrap_or(0);
                let preceded = lines[start..number]
                    .iter()
                    .any(|earlier| earlier.contains("lifecycle_preflight("));
                if !preceded {
                    offenders.push(format!("{relative}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "every production call site of `install_planned` must be preceded by a `lifecycle_preflight` \
         call in the same function: it is the function that deploys adapter assets, and the \
         preflight is the only thing that reads this machine's lifecycle first. These call it with \
         no preflight in front of them: {offenders:#?}"
    );
    assert!(
        callers >= 1,
        "no production call site of `install_planned` was found, so this guard is reading a spelling \
         the shipping code no longer uses and would pass on a tree where nothing calls it at all"
    );
    assert!(
        shipping_files > 40,
        "only {shipping_files} shipping source(s) were read, which is fewer than this crate has \
         \u{2014} an empty walk finds no offenders and reads exactly like a clean one"
    );
}

/// Whether `line` begins a function definition, however its visibility and
/// `async` are spelled.
fn starts_a_function(line: &str) -> bool {
    let mut line = line.trim_start();
    for prefix in ["pub(crate) ", "pub(super) ", "pub(in crate) ", "pub "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            line = rest;
        }
    }
    if let Some(rest) = line.strip_prefix("async ") {
        line = rest;
    }
    line.starts_with("fn ")
}

#[test]
fn a_pre_push_line_keeps_its_local_and_its_remote_ref() {
    let pushed = pushed_refs_of(
        "refs/heads/issue-7 aaa refs/heads/main bbb\n(delete) 000 refs/tags/v1 ccc\n",
    );
    let pairs: Vec<(&str, &str)> = pushed
        .iter()
        .map(|line| (line.local.as_str(), line.remote.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("refs/heads/issue-7", "refs/heads/main"),
            ("(delete)", "refs/tags/v1")
        ]
    );
}

fn assert_retired_refusal(refusal: &Refusal, slug: &str, what: &str) {
    assert_eq!(refusal.code, "agent-retired", "{what} {slug}");
    assert_eq!(
        refusal.outcome,
        MutationOutcome::NotStarted,
        "{what} {slug}"
    );
    assert_eq!(
        refusal.resolution,
        crate::outcome::Resolution::run(format!("estigia uninstall {slug}")),
        "{what} {slug} names no way out"
    );
}

#[test]
fn config_set_edit_and_list_refuse_every_retired_agent_and_write_nothing() {
    for adapter in setup::RETIRED_AGENTS {
        let (_home, options) = sandbox();
        let paths = setup::resolve_paths(adapter, &options).expect("paths resolve");
        crate::setup::tests::install_as_an_earlier_build(adapter, &options);
        let leaked = crate::skill::agent_override(&paths.skill_root, adapter.slug);

        let set = super::config_set(
            "Delivery authorisation",
            "ask",
            Some(adapter.slug),
            false,
            false,
            &options,
            false,
        )
        .expect_err("config set reached a retired agent");
        assert_retired_refusal(&set, adapter.slug, "config set");
        let edit = super::config_edit(Some(adapter.slug), &options)
            .expect_err("config edit reached a retired agent");
        assert_retired_refusal(&edit, adapter.slug, "config edit");
        let list = super::config_list(Some(adapter.slug), &options, false)
            .expect_err("config list reached a retired agent");
        assert_retired_refusal(&list, adapter.slug, "config list");

        assert!(
            !leaked.exists(),
            "{} wrote {}",
            adapter.slug,
            leaked.display()
        );
    }
}

#[test]
fn a_machine_wide_linear_tracker_is_refused_for_the_machine_and_for_an_agent() {
    let (_home, options) = sandbox();
    for agent in [None, Some("claude-code")] {
        let refusal = super::config_set(
            "Tracker",
            "linear ENG",
            agent,
            false,
            false,
            &options,
            false,
        )
        .expect_err("a machine-wide Linear tracker was accepted");
        assert_eq!(refusal.code, "linear-is-per-repository");
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
    }
}

fn delivery_rig() -> (tempfile::TempDir, tempfile::TempDir, SetupOptions) {
    let repo = tempfile::tempdir().expect("a repository");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout");
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    (repo, home, options)
}

fn repository_file(repo: &std::path::Path) -> String {
    std::fs::read_to_string(crate::skill::repository_config_path(repo)).unwrap_or_default()
}

#[test]
fn a_delivery_preset_is_written_as_the_rows_it_stands_for_and_moves_between_presets_whole() {
    let (repo, _home, options) = delivery_rig();
    super::repository_set(Setting::DeliveryPreset, "team", repo.path(), &options)
        .expect("team is taken");
    let written = repository_file(repo.path());
    for row in [
        "| Claim workflow | assisted |",
        "| Self review | on |",
        "| Ready by | agent |",
        "| Corrections after ready | push-ready |",
        "| Merge by | person |",
    ] {
        assert!(written.contains(row), "{row} missing: {written}");
    }
    assert!(
        !written.contains("Delivery preset"),
        "the preset is a reading of the rows, not a row of its own: {written}"
    );
    super::repository_set(Setting::DeliveryPreset, "autonomous", repo.path(), &options)
        .expect("leaving team for autonomous is one write, not five that disagree on the way");
    assert!(repository_file(repo.path()).contains("| Merge by | agent |"));
    let refusal = super::repository_set(Setting::DeliveryPreset, "custom", repo.path(), &options)
        .expect_err("custom names no rows");
    assert_eq!(refusal.code, "delivery-preset-custom-sets-nothing");
}

#[test]
fn a_delivery_row_autonomous_fixes_is_refused_and_not_written() {
    let (repo, _home, options) = delivery_rig();
    let refusal = super::repository_set(Setting::MergeBy, "person", repo.path(), &options)
        .expect_err("autonomous fixes Merge by");
    assert_eq!(refusal.code, "autonomous-fixes-delivery-rows");
    assert!(!repository_file(repo.path()).contains("Merge by"));

    super::repository_set(Setting::Workflow, "assisted", repo.path(), &options)
        .expect("assisted is taken");
    super::repository_set(Setting::MergeBy, "person", repo.path(), &options)
        .expect("under assisted a person merges");
    let refusal = super::repository_set(Setting::MergeBy, "agent", repo.path(), &options)
        .expect_err("an assisted run cannot deliver");
    assert_eq!(refusal.code, "assisted-merge-is-the-persons");
    assert!(repository_file(repo.path()).contains("| Merge by | person |"));
}

#[test]
fn the_screen_saves_a_changed_preset_as_one_write() {
    let (repo, _home, options) = delivery_rig();
    let mut installed = Config::default();
    Setting::DeliveryPreset
        .apply(&mut installed, "team")
        .expect("team");
    super::repository_set(Setting::DeliveryPreset, "team", repo.path(), &options).expect("team");
    let mut edited = installed.clone();
    Setting::DeliveryPreset
        .apply(&mut edited, "autonomous")
        .expect("autonomous");
    super::spread_repository_rows(&installed, &edited, repo.path(), &options)
        .expect("the rows a preset changes are saved together, never one by one");
    let written = repository_file(repo.path());
    assert!(
        written.contains("| Claim workflow | autonomous |"),
        "{written}"
    );
    assert!(written.contains("| Merge by | agent |"), "{written}");
    assert!(!written.contains("Delivery preset"), "{written}");
}

#[test]
fn doctor_refuses_a_repository_file_holding_delivery_rows_that_cannot_both_hold() {
    let (repo, _home, options) = delivery_rig();
    let skill_root = tempfile::tempdir().expect("a skill root");
    std::fs::write(
        skill_root.path().join(crate::skill::CONTRACT),
        crate::skill::CONTRACT_CONTENTS,
    )
    .expect("a contract");
    super::repository_set(Setting::DeliveryPreset, "team", repo.path(), &options).expect("team");
    assert_eq!(
        crate::harness::doctor::delivery_conflict_in_force(Some(skill_root.path()), repo.path()),
        None
    );
    let path = crate::skill::repository_config_path(repo.path());
    let edited =
        repository_file(repo.path()).replace("| Merge by | person |", "| Merge by | agent |");
    std::fs::write(&path, edited).expect("a hand edit");
    let refusal =
        crate::harness::doctor::delivery_conflict_in_force(Some(skill_root.path()), repo.path())
            .expect("doctor reads the repository file's delivery rows");
    assert_eq!(refusal.code, "assisted-merge-is-the-persons");
}

#[test]
fn a_repository_cannot_go_autonomous_while_its_tracker_is_linear() {
    let repo = tempfile::tempdir().expect("a repository");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout");
    let elsewhere = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(elsewhere.path().to_path_buf()),
        ..SetupOptions::default()
    };
    super::repository_set(Setting::Workflow, "assisted", repo.path(), &options)
        .expect("assisted is taken");
    super::repository_set(Setting::Tracker, "linear ENG", repo.path(), &options)
        .expect("Linear is taken under assisted work");
    let refusal = super::repository_set(Setting::Workflow, "autonomous", repo.path(), &options)
        .expect_err("autonomous was accepted over a Linear tracker");
    assert_eq!(refusal.code, "autonomous-needs-github-tracker");
    let written = std::fs::read_to_string(crate::skill::repository_config_path(repo.path()))
        .expect("the file");
    assert!(
        written.contains("| Claim workflow | assisted |"),
        "the refused row was written: {written}"
    );
}

#[test]
fn the_transportless_sentence_does_not_say_a_linear_assisted_run_cannot_work() {
    let linear = crate::config::Tracker::Linear {
        team: "ENG".to_owned(),
    };
    let sentence = super::transportless_sentence(&linear);
    assert!(
        sentence.contains("assisted run reads its issue"),
        "{sentence}"
    );
    assert!(!sentence.contains("adjudicates nothing"), "{sentence}");
}

#[test]
fn a_role_record_is_read_and_filtered_by_a_key_as_well_as_a_number() {
    use crate::identity::Identity;
    let keyed = serde_json::json!({"run_id": "claude-a", "agent_type": "dev", "issue": "ENG-123"});
    let numbered = serde_json::json!({"run_id": "claude-a", "agent_type": "dev", "issue": 12});
    assert!(super::role_line(&keyed).contains("issue ENG-123"));
    assert!(super::role_line(&numbered).contains("issue #12"));
    let key = Identity::Key("ENG-123".to_owned());
    assert!(super::role_issue_matches(&keyed, Some(&key)));
    assert!(!super::role_issue_matches(&numbered, Some(&key)));
    assert!(super::role_issue_matches(
        &numbered,
        Some(&Identity::Number(12))
    ));
    assert!(super::role_issue_matches(&keyed, None));
}

#[test]
fn select_keeps_retired_agents_out_of_an_install_and_in_reach_of_an_uninstall() {
    for adapter in setup::RETIRED_AGENTS {
        let refusal = select(Some(adapter.slug), false, true).unwrap_err();
        assert_eq!(refusal.code, "agent-retired", "{}", adapter.slug);
        let named = select(Some(adapter.slug), false, false).expect("uninstall reaches it");
        assert_eq!(named[0].slug, adapter.slug);
    }

    let installing = select(None, true, true).expect("--all installs");
    let removing = select(None, true, false).expect("--all uninstalls");
    for adapter in setup::RETIRED_AGENTS {
        assert!(installing.iter().all(|chosen| chosen.slug != adapter.slug));
        assert!(removing.iter().any(|chosen| chosen.slug == adapter.slug));
    }
    assert_eq!(installing.len(), setup::AGENTS.len());
    assert_eq!(
        removing.len(),
        setup::AGENTS.len() + setup::RETIRED_AGENTS.len()
    );
}

const NOT_REFUSALS: [&str; 6] = [
    "companion-state",
    "not-watched",
    "nothing-sworn",
    "no-tracker",
    "another-checkout",
    "outside-the-claim",
];

#[test]
fn every_code_the_crate_raises_has_a_class_and_the_class_agrees_with_the_outcome() {
    let mut all = std::collections::BTreeSet::new();
    for source in &crate_sources() {
        all.extend(codes_in(source));
    }
    let built: Vec<Refusal> = inventory().into_iter().chain(after_writing()).collect();
    for refusal in &built {
        all.insert(refusal.code.to_owned());
    }
    let unclassed: Vec<&String> = all
        .iter()
        .filter(|code| !NOT_REFUSALS.contains(&code.as_str()))
        .filter(|code| crate::harness::refusal_class::class_of(code).is_none())
        .collect();
    assert!(unclassed.is_empty(), "raised and unclassed: {unclassed:?}");

    let reviewer = [
        "reviewer-cli-missing",
        "reviewer-launch-failed",
        "reviewer-exited-nonzero",
        "reviewer-no-answer",
        "reviewer-error-result",
        "reviewer-output-too-large",
        "reviewer-deadline",
        "reviewer-unavailable",
        "reviewer-result-malformed",
        "reviewer-payload-mismatch",
        "reviewer-input-too-large",
        "reviewer-inspect-transaction-unresolved",
        "reviewer-inspect-absent-side",
        "reviewer-inspect-outside-correction",
        "reviewer-inspect-unreadable",
    ];
    for code in reviewer {
        assert!(
            crate::harness::refusal_class::class_of(code).is_some(),
            "`{code}` has no class"
        );
    }
    let unraised: Vec<&str> = crate::harness::refusal_class::classified()
        .map(|(code, _)| code)
        .filter(|code| {
            !all.contains(*code)
                && !reviewer.contains(code)
                && !crate::harness::tracker::TRANSPORT_VOCABULARY.contains(code)
                && *code != "transport-refused"
        })
        .collect();
    assert!(
        unraised.is_empty(),
        "classed and raised nowhere: {unraised:?}"
    );

    for refusal in &built {
        if NOT_REFUSALS.contains(&refusal.code) {
            continue;
        }
        assert_eq!(
            crate::harness::refusal_class::class_of(refusal.code),
            Some(refusal.class),
            "`{}` is built with a class the table does not hold for it",
            refusal.code
        );
        assert_eq!(
            refusal.class == crate::outcome::RefusalClass::Unknown,
            refusal.outcome == MutationOutcome::Unknown,
            "`{}` pairs class {:?} with outcome {:?}",
            refusal.code,
            refusal.class,
            refusal.outcome
        );
    }
}

#[test]
fn flush_takes_a_status_or_a_detach_and_not_both() {
    assert!(Cli::try_parse_from(["estigia", "flush"]).is_ok());
    assert!(Cli::try_parse_from(["estigia", "flush", "--status"]).is_ok());
    assert!(Cli::try_parse_from(["estigia", "flush", "--detach"]).is_ok());
    assert!(Cli::try_parse_from(["estigia", "flush", "--status", "--detach"]).is_err());
}

#[test]
fn the_startup_list_keeps_an_agent_whose_instruction_file_cannot_be_read() {
    assert!(listed_at_startup(&setup::Installed::Unknown(
        "denied".into()
    )));
    assert!(listed_at_startup(&setup::Installed::Configured));
    assert!(!listed_at_startup(&setup::Installed::Absent));
}

#[test]
fn the_guided_startup_list_asks_the_decision_with_the_read_not_a_boolean() {
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    let start = source.find("fn guided(").expect("guided is defined");
    let body = &source[start..];
    assert!(
        body.contains("listed_at_startup(&setup::read_configured(adapter, options))"),
        "the startup list no longer decides from the three-way read"
    );
}

fn shared_root_contract(options: &SetupOptions) -> std::path::PathBuf {
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::resolve_paths(adapter, options)
        .expect("paths resolve")
        .skill_root
        .join(crate::skill::CONTRACT)
}

#[test]
fn a_set_without_an_agent_refuses_every_agent_scoped_row_and_writes_nothing() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    let contract = shared_root_contract(&options);
    let before = fs::read(&contract).expect("the contract reads");

    for setting in crate::config::SETTINGS
        .iter()
        .filter(|setting| setting.scope() == crate::config::Scope::Agent)
    {
        let value = setting.value_of(&Config::default());
        let refusal =
            super::config_set(setting.label(), &value, None, false, false, &options, false)
                .expect_err("an agent-scoped row was written without naming an agent");
        assert_eq!(
            refusal.code,
            "setting-needs-an-agent",
            "{}",
            setting.label()
        );
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        let shown = format!("{refusal}");
        assert!(shown.contains("--agent"), "{shown}");
        assert!(shown.contains("opencode"), "{shown}");
    }
    assert_eq!(fs::read(&contract).expect("the contract reads"), before);
}

#[test]
fn a_set_without_an_agent_still_writes_a_row_that_is_not_agent_scoped() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");

    super::config_set("Change size", "300", None, false, false, &options, false)
        .expect("a row every agent shares is still set without naming one");

    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("paths resolve")
        .skill_root;
    let read = crate::skill::installed_config(&root).expect("the contract reads");
    assert_eq!(
        crate::config::Setting::ChangeSize.value_of(&read),
        "300",
        "the write did not land"
    );
}

#[test]
fn an_edited_table_that_moves_an_agent_scoped_row_is_refused_and_writes_nothing() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    let contract = shared_root_contract(&options);
    let before = fs::read(&contract).expect("the contract reads");
    let installed = Config::default();
    let mut edited = installed.clone();
    crate::config::Setting::Models
        .apply(&mut edited, "developer=opus")
        .expect("a routing value");

    let writable = super::writable_config(None, &options).expect("the contract is writable");
    let refusal = super::commit_edit(None, writable, &edited, &options)
        .expect_err("the editor wrote an agent-scoped row into the shared contract");

    assert_eq!(refusal.code, "setting-needs-an-agent");
    assert!(format!("{refusal}").contains("--agent"));
    assert_eq!(fs::read(&contract).expect("the contract reads"), before);

    let mut edited = installed;
    crate::config::Setting::ChangeSize
        .apply(&mut edited, "300")
        .expect("a size");
    let writable = super::writable_config(None, &options).expect("the contract is writable");
    super::commit_edit(None, writable, &edited, &options)
        .expect("a row every agent shares is still written by the editor");
}

#[test]
fn a_set_names_the_file_it_wrote() {
    let file = std::path::Path::new("/home/x/.agents/skills/flow/SKILL.md");
    for change in [Change::Unchanged, Change::Update] {
        let said = super::set_answer("Change size", "300", change, file);
        assert!(
            said.contains("/home/x/.agents/skills/flow/SKILL.md"),
            "{said}"
        );
        assert!(said.contains("Change size"), "{said}");
    }
}

#[test]
fn contract_refuses_an_agent_scoped_row_on_a_shared_root_and_writes_nothing() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    let contract = shared_root_contract(&options);
    let root = contract.parent().expect("a root").to_path_buf();
    let contract_before = fs::read(&contract).expect("the contract reads");
    let overlay = crate::skill::agent_override(&root, "opencode");

    for setting in crate::config::SETTINGS
        .iter()
        .filter(|setting| setting.scope() == crate::config::Scope::Agent)
        .filter(|setting| setting.applies_to_agent(adapter))
    {
        let value = setting.value_of(&Config::default());
        let refusal = super::config_set(
            setting.label(),
            &value,
            Some("opencode"),
            false,
            true,
            &options,
            false,
        )
        .expect_err("--contract wrote an agent-scoped row into the shared table");
        assert_eq!(refusal.code, "contract-is-shared", "{}", setting.label());
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        let crate::outcome::Resolution::Run { command, .. } = &refusal.resolution else {
            panic!("the refusal names no command: {}", refusal.resolution);
        };
        assert!(!command.contains("--contract"), "{command}");
        assert!(command.contains("--agent opencode"), "{command}");
    }
    assert_eq!(
        fs::read(&contract).expect("the contract reads"),
        contract_before
    );
    assert!(!overlay.exists(), "a refused set created the agent's file");

    let refusal = super::config_set(
        "Model routing",
        "developer=haiku",
        Some("opencode"),
        false,
        true,
        &options,
        false,
    )
    .expect_err("refused above");
    let crate::outcome::Resolution::Run { command, .. } = refusal.resolution else {
        panic!("no command");
    };
    let words = words_of(&command);
    let parsed = Cli::try_parse_from(&words).expect("the named command parses");
    let Command::Config {
        action:
            ConfigAction::Set {
                setting,
                value,
                agent,
                repo,
                contract: to_contract,
            },
    } = parsed.command
    else {
        panic!("the named command is not a config set: {command}");
    };
    super::config_set(
        &setting,
        &value,
        agent.as_deref(),
        repo,
        to_contract,
        &options,
        false,
    )
    .expect("the named command clears the refusal");
    assert!(overlay.is_file());
    assert_eq!(
        fs::read(&contract).expect("the contract reads"),
        contract_before
    );
}

#[test]
fn contract_still_writes_the_table_of_an_agent_with_a_skill_directory_of_its_own() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("an adapter");
    assert!(adapter.discovers_skills());
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");

    super::config_set(
        "Model routing",
        "developer=haiku",
        Some("claude-code"),
        false,
        true,
        &options,
        false,
    )
    .expect("an agent's own contract takes the row");

    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("paths")
        .skill_root;
    let read = crate::skill::installed_config(&root).expect("the contract reads");
    assert_eq!(
        crate::config::Setting::Models.value_of(&read),
        "developer=haiku"
    );
}

#[test]
fn contract_without_an_agent_is_refused() {
    let (_home, options) = sandbox();
    let refusal = super::config_set(
        "Model routing",
        "developer=haiku",
        None,
        false,
        true,
        &options,
        false,
    )
    .expect_err("--contract named no agent");
    assert_eq!(refusal.code, "contract-needs-an-agent");
    assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
}

#[test]
fn a_model_from_another_host_is_stored_and_advised_never_refused() {
    let (_home, options) = sandbox();
    let codex = crate::setup::find_agent("codex").expect("an adapter");
    crate::setup::setup(codex, &Config::default(), &options).expect("setup runs");

    super::config_set(
        "Model routing",
        "developer=sonnet",
        Some("codex"),
        false,
        false,
        &options,
        false,
    )
    .expect("a Claude model in Codex's routing is stored");

    let root = crate::setup::resolve_paths(codex, &options)
        .expect("paths")
        .skill_root;
    let stored = crate::skill::installed_config_for(&root, Some("codex")).expect("reads");
    assert_eq!(stored.models.for_target("developer"), Some("sonnet"));

    let advised = super::model_advisories(codex, &stored);
    assert_eq!(advised.len(), 1, "{advised:?}");
    assert!(advised[0].contains("sonnet"), "{advised:?}");
    assert!(advised[0].contains("Claude Code"), "{advised:?}");
    assert!(advised[0].contains("Codex"), "{advised:?}");

    let mut own = Config::default();
    crate::config::Setting::Models
        .apply(&mut own, "developer=gpt-5.5, analyst=my-fine-tune")
        .expect("a routing");
    assert!(super::model_advisories(codex, &own).is_empty());
    let claude = crate::setup::find_agent("claude-code").expect("an adapter");
    let advised = super::model_advisories(claude, &own);
    assert_eq!(advised.len(), 1, "{advised:?}");
    assert!(advised[0].contains("Codex"), "{advised:?}");
}

fn words_of(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in command.chars() {
        match character {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[test]
fn doctor_names_an_agent_scoped_row_in_a_shared_contract_and_its_command_gives_that_agent_its_own_file()
 {
    let (_home, options) = sandbox();
    let opencode = crate::setup::find_agent("opencode").expect("an adapter");
    let zcode = crate::setup::find_agent("zcode").expect("an adapter");
    for adapter in [opencode, zcode] {
        crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    }
    let contract = shared_root_contract(&options);
    let root = contract.parent().expect("a root").to_path_buf();

    let report = |adapter: &'static crate::setup::AgentAdapter| {
        let entry = crate::harness::doctor::contract_entry(adapter, &options).expect("an entry");
        crate::harness::doctor::contracts(&[entry]).remove(0)
    };
    assert!(
        !report(zcode).health.is_broken(),
        "a default shared contract was reported"
    );

    let mut shared = Config::default();
    crate::config::Setting::Models
        .apply(&mut shared, "developer=sonnet, analyst=opus")
        .expect("a routing");
    crate::setup::rewrite_configuration(&contract, &shared).expect("the shared row is written");
    super::config_set(
        "Model routing",
        "developer=haiku",
        Some("opencode"),
        false,
        false,
        &options,
        false,
    )
    .expect("opencode takes its own row");
    let opencode_before = crate::skill::installed_config_for(&root, Some("opencode"))
        .expect("opencode's table reads");
    let contract_before = fs::read(&contract).expect("the contract reads");
    let zcode_file = crate::skill::agent_override(&root, "zcode");
    assert!(!zcode_file.exists(), "zcode already had a file of its own");

    assert!(
        !report(opencode).health.is_broken(),
        "an agent with a file of its own was reported"
    );
    let crate::harness::doctor::Health::Broken { detail, resolution } = report(zcode).health else {
        panic!("a Claude routing in the shared contract went unreported");
    };
    assert!(detail.contains("Model routing"), "{detail}");
    assert!(detail.contains(&contract.display().to_string()), "{detail}");
    assert!(detail.contains("zcode"), "{detail}");
    let crate::outcome::Resolution::Run { command, .. } = resolution else {
        panic!("the row names no command");
    };

    let words = words_of(&command);
    let parsed = Cli::try_parse_from(&words).expect("the named command parses");
    let Command::Config {
        action:
            ConfigAction::Set {
                setting,
                value,
                agent,
                repo,
                contract: to_contract,
            },
    } = parsed.command
    else {
        panic!("the named command is not a config set: {command}");
    };
    assert!(
        !to_contract && agent.as_deref() == Some("zcode"),
        "{command}"
    );
    super::config_set(
        &setting,
        &value,
        agent.as_deref(),
        repo,
        to_contract,
        &options,
        false,
    )
    .expect("the named command is refused");

    assert!(
        zcode_file.is_file(),
        "the named command wrote no file for zcode"
    );
    let own = fs::read_to_string(&zcode_file).expect("zcode's file reads");
    assert!(own.contains("Model routing"), "{own}");
    assert_eq!(
        fs::read(&contract).expect("the contract reads"),
        contract_before,
        "the named command rewrote the contract every agent on the root reads"
    );
    assert!(
        !report(zcode).health.is_broken(),
        "running the named command did not clear the row"
    );
    let opencode_after = crate::skill::installed_config_for(&root, Some("opencode"))
        .expect("opencode's table reads");
    assert_eq!(
        crate::config::Setting::Models.value_of(&opencode_after),
        crate::config::Setting::Models.value_of(&opencode_before),
        "the named command moved another agent's answer"
    );
}

#[test]
fn the_command_that_clears_an_ask_a_shared_contract_holds_is_one_that_runs() {
    let mut asking = Config::default();
    Setting::Delivery
        .apply(&mut asking, "ask")
        .expect("ask is accepted");
    let rig = authorisation_rig_for("opencode", &Config::default());
    crate::setup::rewrite_configuration(&rig.root.join(crate::skill::CONTRACT), &asking)
        .expect("the shared contract is written");

    let refusal = super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect_err("a repository loosened an ask the shared contract holds");
    assert_eq!(refusal.code, "repository-cannot-loosen");
    let crate::outcome::Resolution::Run { command, .. } = &refusal.resolution else {
        panic!("no command was named: {}", refusal.resolution);
    };
    assert!(command.contains("--agent opencode"), "{command}");
    assert!(!command.contains("--contract"), "{command}");

    super::config_set(
        "Delivery authorisation",
        "auto",
        Some("opencode"),
        false,
        false,
        &rig.options,
        false,
    )
    .expect("the named command clears the contract's ask");
    super::repository_set(Setting::Delivery, "auto", rig.repo.path(), &rig.options)
        .expect("the repository may answer auto once the agent's own file no longer asks");
}

fn asked_refusal() -> Refusal {
    Refusal::not_started(
        "issue-required",
        "this checkout is not claimed",
        Resolution::run("estigia claim"),
    )
}

fn nothing_observed() -> crate::outcome::Observed {
    crate::outcome::Observed {
        command: None,
        checkout: None,
        push_refs: Vec::new(),
    }
}

fn gate_exit(decision: harness::Decision, asks: bool) -> ExitCode {
    let answered = super::gate_answer(decision, asks);
    let refusal = super::report_gate("write", false, answered, &nothing_observed())
        .expect_err("a refusal or a question is an error");
    super::gate_exit_code(&refusal)
}

#[test]
fn a_question_is_answered_with_its_own_exit_code_only_to_a_caller_that_can_carry_it() {
    let asked = || harness::Decision::Ask(Box::new(asked_refusal()));
    assert_eq!(gate_exit(asked(), true), ExitCode::Asked);
    assert_eq!(gate_exit(asked(), false), ExitCode::Refused);
    assert_eq!(
        gate_exit(harness::Decision::Deny(Box::new(asked_refusal())), true),
        ExitCode::Refused
    );
}

#[test]
fn a_question_does_not_leave_the_next_refusal_answered_as_one() {
    gate_exit(harness::Decision::Ask(Box::new(asked_refusal())), true);
    assert_eq!(
        super::gate_exit_code(&asked_refusal()),
        ExitCode::Refused,
        "the previous question marked this refusal as asked"
    );
}

#[test]
fn only_the_pi_runtime_is_asked_on_a_caller_that_can_carry_the_question() {
    assert!(super::asks_through(true, Some("pi")));
    assert!(!super::asks_through(false, Some("pi")));
    for runtime in [None, Some("claude-code"), Some("codex"), Some("opencode")] {
        assert!(!super::asks_through(true, runtime), "{runtime:?}");
    }
}

#[test]
fn the_gate_command_takes_a_flag_saying_the_caller_can_ask() {
    let parsed = Cli::try_parse_from([
        "estigia",
        "gate",
        "write",
        "--runtime",
        "pi",
        "--session-id",
        "sess-1",
        "--can-ask",
    ])
    .expect("the flag parses");
    assert!(matches!(
        parsed.command,
        Command::Gate { can_ask: true, .. }
    ));
    let bare = Cli::try_parse_from(["estigia", "gate", "write"]).expect("the bare form parses");
    assert!(matches!(bare.command, Command::Gate { can_ask: false, .. }));
}

#[test]
fn resume_takes_a_run_and_requires_print() {
    assert!(Cli::try_parse_from(["estigia", "resume", "opencode-ab12"]).is_err());
    let parsed = Cli::try_parse_from(["estigia", "resume", "opencode-ab12", "--print"])
        .expect("resume --print parses");
    assert!(matches!(
        parsed.command,
        Command::Resume { run, print: true } if run == "opencode-ab12"
    ));
}

#[test]
fn status_set_parses_beside_the_bare_status_and_takes_only_a_listed_phase() {
    use crate::cli::args::StatusAction;

    let bare = Cli::try_parse_from(["estigia", "status"]).expect("bare status parses");
    assert!(matches!(bare.command, Command::Status { action: None }));
    let set = Cli::try_parse_from([
        "estigia",
        "status",
        "set",
        "reading the gate",
        "--phase",
        "verifying",
        "--run-id",
        "claude-abcd1234",
    ])
    .expect("status set parses");
    assert!(matches!(
        set.command,
        Command::Status {
            action: Some(StatusAction::Set { .. })
        }
    ));
    assert!(
        Cli::try_parse_from(["estigia", "status", "set", "x", "--phase", "blocked"]).is_err(),
        "blocked is a workflow state and not a phase"
    );
}
