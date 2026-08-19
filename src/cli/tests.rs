//! The ratchet, enforced.
//!
//! > A message may name a command only when running it discharges the block.
//! > Naming a dead end is worse than naming nothing.
//!
//! Two guards, one direction each:
//!
//! - every command a rejection **names** exists in the real dispatch and in its
//!   parser — `every_command_a_rejection_names_parses`;
//! - every rejection that **has** a resolution names it — enforced by the type:
//!   [`Refusal`] has no constructor that omits [`Resolution`], so the second
//!   guard is a compile error rather than a test.
//!
//! Measured against issue-flow on 2026-07-31: **87** `raise Stop(...)` /
//! `ConfigDefect(...)` sites, **zero** naming an executable continuation. The
//! codes were well chosen (`stale-foreign-requires-reclaim`,
//! `force-required-for-reason`); none said what to run. Starting at zero is the
//! only moment a ratchet can start.

use clap::Parser;
use std::fs;

use super::*;
use crate::config::Config;
use crate::outcome::{MutationOutcome, NoCommandReason};

#[test]
fn guided_entry_is_not_called_when_lifecycle_preflight_refuses() {
    let entered = std::cell::Cell::new(false);
    let refusal = Refusal::not_started(
        "source-build-not-allowed",
        "preflight refused",
        Resolution::run("estigia setup --interactive --allow-source-build"),
    );

    let result = after_lifecycle_preflight(Err(refusal), || {
        entered.set(true);
        Ok(())
    });

    assert!(result.is_err());
    assert!(!entered.get());
}

/// Every refusal this crate can construct without touching the filesystem.
///
/// Assembled by hand rather than by scanning the source: a scan that misses a
/// site reports a clean ratchet, which is the failure mode the guard exists to
/// prevent. A refusal added without a line here fails
/// `the_inventory_covers_every_code_the_crate_raises`.
fn inventory() -> Vec<Refusal> {
    let mut refusals = vec![
        setup::no_agent_named(),
        setup::no_terminal(),
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
            "reviewer-canonical-unavailable",
            "the canonical user reviewer cannot be proved current",
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "restore it with Claude setup, or use separate-session review",
            ),
        ),
        nothing_configured(),
        // Knowable from the command line: a value the table cannot carry.
        super::config_set(
            "Irreversible commands",
            "make deploy | tee log",
            None,
            false,
            &SetupOptions::default(),
            false,
        )
        .unwrap_err(),
        // A row the chosen tracker has nothing to do with. Raised by its real
        // constructor rather than written out here: the resolution names
        // `config set Tracker github`, and the ratchet has to be applied to the
        // command that sentence actually offers.
        crate::outcome::Refusal::not_started(
            "setting-not-for-this-tracker",
            "Project board has no meaning under linear: that binding declares no board mirror",
            crate::outcome::Resolution::run("estigia config set Tracker github"),
        ),
        // A contract that is not there, and one that is there and will not open.
        // Both come out of `rewrite_configuration`, and a temporary directory
        // reaches both — which is why the carve-out that excused them for
        // needing a filesystem was excusing them for nothing.
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
        // Reachable only with a filesystem, and raised here by the real
        // constructor rather than copied: the sentence names the file, and a
        // hand-written twin would put the ratchet on words nobody raises.
        {
            let root = tempfile::tempdir().expect("a temporary root");
            std::fs::create_dir_all(root.path().join(crate::config::LOCAL_FILE))
                .expect("something unreadable in its place");
            crate::skill::local_overrides(root.path())
                .expect_err("a directory in the override's place is not an override")
        },
        // The other way a run id can name something that is not this run: one
        // that holds a claim over a checkout this server is not in. Built by
        // hand for the same reason as the one below — the point is the ratchet.
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
        crate::harness::delivery_pr_unidentified(),
        crate::harness::complete_review_receipt_missing(54),
        crate::harness::delivery_pr_mismatch(54, 55),
        crate::harness::complete_review_receipt_not_selected(),
        // Raised before anything on disk is read, so it is built the way the
        // command builds it: a repository-wide row asked for one agent.
        super::config_set(
            "Tracker",
            "linear",
            Some("opencode"),
            false,
            &SetupOptions::default(),
            false,
        )
        .unwrap_err(),
        find_agent("emacs").unwrap_err(),
        // The screen's own language could not be remembered. Through the
        // ratchet like every other refusal: the screen already changed, so
        // what this message owes somebody is the part that did *not* happen.
        // Built against a file that cannot be created rather than by driving
        // a terminal, because the point here is the message, not the screen.
        crate::tui::words::remember(
            Some(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")),
            crate::tui::words::Tongue::Spanish,
        )
        .unwrap_err(),
        skill::no_skill_root(),
        setup::needs_operator_answer("which board this repository projects onto"),
        // The real constructor, not a copy of it: a hand-written twin here would
        // pass the ratchet while the refusal the gate actually raises drifted.
        crate::harness::out_of_phase(
            "gh pr merge",
            "in-progress",
            12,
            crate::config::Integration::Branch,
            None,
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
        // Both named `estigia doctor`, which reports and writes nothing. The
        // first is a file only a person can free — and `doctor` does not even
        // mention it, because the record stays readable and the stand-down row
        // answers about the window in force. The second is a defect in this
        // build, where no command exists at all.
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
        // The trunk-based refusal, by its real constructor: a delivery landing
        // early with nothing named to switch it off.
        crate::harness::out_of_phase(
            "gh pr merge",
            "in-progress",
            12,
            crate::config::Integration::Trunk,
            None,
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
            "setting-unknown",
            "\"Merge\" is not a setting",
            Resolution::run("estigia config list"),
        ),
        // The harness. Every one of these can be raised on the critical path of
        // an edit, which is the worst possible place for a message that names a
        // dead end.
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
        // The discovery half of `release` answering that it found nothing of
        // its own to put down. Not a release, and this used to be printed as
        // one: `released: <run> no longer holds #<issue>` over a pointer that
        // still held it.
        Refusal::not_started(
            "release-not-performed",
            "claude-abcd1234 still holds #12: the release read the timeline and found no epoch \
             of its own to put down",
            Resolution::run("estigia status"),
        ),
        Refusal::not_started(
            "hook-event-unknown",
            "\"on-tuesday\" is not a lifecycle event",
            Resolution::run("estigia setup --all"),
        ),
        Refusal::not_started(
            "gate-input-not-json",
            "--input is not JSON",
            Resolution::run(
                r#"estigia gate Edit --run-id <run-id> --input '{"file_path":"src/x.rs"}'"#,
            ),
        ),
        // The surface the gate's authority rests on. It replaced
        // `transport-not-installed`, which detected a missing Python script and
        // happened to be right because nothing was installed at all. That code
        // is gone with the spawning it belonged to: after a **complete** install
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
        // `run-pointer-unreadable` is carried once, further up, with the way out
        // the code actually gives. A second entry sat here naming
        // `estigia release --run-id ...`, which `harness::gate` had already
        // stopped giving and says why in its own words: *with an unreadable
        // pointer that command cannot say what to put down either, so it
        // refuses with this same code.* The code dropped the dead end and the
        // replica kept it, so the ratchet went on reviewing the one shape the
        // ratchet forbids — and passing, because nothing crossed the entry
        // against the constructor it was copied from.
        //
        // The same fault met from the other end: the single-run path knows
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
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "which claim this work belongs to — release the runs that do not, then retry",
            ),
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
    ];

    // Configuration defects, raised by reading a table.
    let bad = |label: &str, value: &str| {
        let table = format!(
            "| Setting | Value here | Skill default |\n|---|---|---|\n| {label} | {value} | x |\n"
        );
        Config::read(&table, None).expect_err("this table is invalid")
    };
    refusals.push(bad("Merge strategy", "fast-forward"));
    refusals.push(bad("Worktree location", "../relative"));
    refusals.push(bad("Tracker", "github not-a-repo"));
    refusals.push(bad("Task body language", ""));
    // A pipe cannot reach the parser through a table — it would split the row
    // into another cell first — so the board's own guard is exercised directly
    // rather than through a document that cannot express the defect.
    refusals.push(crate::config::BoardRef::parse("|").expect_err("a pipe is not a board"));

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

/// The screen refuses a row the operator's own file overrides, like `config set`.
///
/// `config_edit` says it one screen over: *`config set` already refuses that for
/// one row; a screen that writes fourteen has fourteen chances to do it, so it
/// checks all of them and names each one.* The screen that **installs** writes
/// the same fourteen and checked none.
///
/// Measured through both doors before this existed: `config set "Merge strategy"
/// squash` refused with `setting-shadowed-by-local-file`, the guided install
/// accepted the identical value, and what the operator then read was `rebase` —
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
        merge: crate::config::MergeStrategy::Squash,
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
            .merge,
        crate::config::MergeStrategy::Squash,
        "the value the screen wrote is not the one a run reads"
    );

    // And now the operator's own file, overriding that row.
    std::fs::write(
        root.join(crate::config::LOCAL_FILE),
        "| Setting | Value here | Skill default |\n|---|---|---|\n\
         | Merge strategy | rebase | merge commit |\n",
    )
    .expect("the override is written");

    let refusal = super::install_planned(&plan(&chosen), &options, false)
        .expect_err("the screen wrote a value nobody will read and said nothing");
    assert_eq!(refusal.code, "setting-shadowed-by-local-file");
    assert!(
        refusal.message.contains("Merge strategy"),
        "the refusal does not name the row that did not take: {refusal}"
    );
    // Committed, because the table was written. Telling an operator nothing
    // happened would send them to repeat a write that already landed.
    assert_eq!(refusal.outcome, MutationOutcome::Committed);

    // The same act through the other door, so the two are held to one answer.
    let theirs = super::config_set("Merge strategy", "squash", None, false, &options, false)
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
        // honest answer for a companion Estigia refuses to install itself. The
        // ratchet is that the command runs, not that Estigia owns it, so the
        // check splits: our own invocations go through the real parser, and a
        // foreign one has to at least be on this machine's path.
        if argv[0] == "estigia" {
            // A suggestion may offer a choice — `estigia setup <a, b, c>` — so
            // the placeholder is expanded into one real invocation per
            // alternative rather than being fed to the parser as a literal.
            for expanded in expand(&argv) {
                if let Err(error) = Cli::try_parse_from(&expanded) {
                    // `--help` and `--version` are commands that *work*: clap
                    // reports them as errors because it is about to print
                    // something and exit, not because it could not parse them.
                    // Treating them as dead ends would forbid the one
                    // suggestion that discharges "I do not know the arguments".
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

/// Turns a suggestion into every concrete invocation it stands for.
///
/// Two shapes reach here and both are legitimate:
///
/// - a **choice**, `estigia setup <a, b, c>`, which is one real command per
///   alternative;
/// - a **placeholder**, `estigia gate Edit --run-id <run-id>`, which the
///   operator fills in.
///
/// Substituting rather than choking matters: the guard's job is to prove the
/// *shape* parses. It caught a real defect this way — `gate-input-not-json`
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
    // The closed vocabulary is the escape hatch, and it is deliberately narrow.
    // A rejection that reaches past it is one that has a resolution and has not
    // written it down.
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

/// Refusals raised **after** a write landed.
///
/// A separate list, because [`inventory`] carries a claim these cannot make:
/// that nothing was touched. `config set` writes the versioned table and then
/// reads back what the operator will actually read — and when the local
/// override shadows that row, the write happened and had no effect. Reporting
/// `NotStarted` there would be the lie the taxonomy exists to prevent, and
/// reporting nothing at all was the defect that made this list necessary.
///
/// They go through the same ratchet: a named command has to parse, and a named
/// non-command has to carry one of the closed reasons.
fn after_writing() -> Vec<Refusal> {
    vec![
        Refusal {
            code: "repository-readback-missing",
            message: "the repository configuration disappeared after writing".to_owned(),
            outcome: MutationOutcome::Unknown,
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "read the repository configuration before deciding whether to retry",
            ),
        },
        // Built as a struct literal by `write_failed`, which is why it was the
        // one code in the crate with no row here: everything else arrives
        // through `Refusal::not_started` or a constructor this file calls.
        //
        // And it is the one whose way out changed — from `estigia status`,
        // which answers *skill out of date* and clears nothing, to the write
        // access only a person can give. Nothing noticed, because nothing was
        // looking.
        Refusal {
            code: "setup-write-failed",
            message: "the write did not land".to_owned(),
            outcome: crate::outcome::MutationOutcome::Unknown,
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "write access to that file",
            ),
        },
        Refusal {
            code: "lifecycle-release-publication-failed",
            message: "candidate provenance landed but release publication failed".to_owned(),
            outcome: MutationOutcome::Committed,
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "inspect lifecycle evidence before retrying the installer",
            ),
        },
        Refusal {
            code: "setting-shadowed-by-local-file",
            message: "the row is overridden by the operator's local file".to_owned(),
            outcome: crate::outcome::MutationOutcome::Committed,
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "that row changed or removed in the local file",
            ),
        },
        // The third of the family, and the one that exists because the first
        // used to absorb it: the table took the value, the row reads as
        // something else, and no operator file is there to explain it. No
        // command, because none is known — and it says what it saw rather than
        // naming a cause it cannot see, which is what the first one did when it
        // fell back to the words "the local override".
        Refusal {
            code: "setting-not-read-back",
            message: "the row was written and reads as something else, with no override there"
                .to_owned(),
            outcome: crate::outcome::MutationOutcome::Committed,
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "what answers that row instead, which `estigia config list --agent <agent>` reports",
            ),
        },
        // The sibling above it is the operator's file and has no command; this
        // one is a file Estigia wrote, so there is one and it clears the block.
        Refusal {
            code: "setting-shadowed-by-agent-file",
            message: "an agent's own file answers that row instead of the table".to_owned(),
            outcome: crate::outcome::MutationOutcome::Committed,
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::run(
                "estigia config set \"Planning\" \"direct\" --agent claude-code",
            ),
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

#[test]
fn each_agent_keeps_its_own_configuration_when_setup_runs_over_all_of_them() {
    // The defect, held: `existing_config` read the *first* root that answered
    // and handed that table to every target, so an operator who had deliberately
    // configured two agents differently lost the second one — rewritten to the
    // first one's values, with nothing said and nothing asked.
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
        judges: crate::config::Judges::TwoBlind,
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
fn a_fresh_agent_inherits_only_when_there_is_one_answer_to_inherit() {
    let (_home, options) = sandbox();
    let claude = find_agent("claude-code").expect("claude-code is an adapter");
    let codex = find_agent("codex").expect("codex is an adapter");

    let theirs = Config {
        judges: crate::config::Judges::TwoBlind,
        ..Config::default()
    };

    // One answer in play: a root with no contract of its own takes it, which is
    // what keeps adding an agent from resetting somebody's table to defaults.
    setup::setup(claude, &theirs, &options).expect("claude-code installs");
    let one = installed_configs(&[claude, codex], &options);
    assert_eq!(one.len(), 1);
    assert_eq!(
        config_for(codex, &one, &options).expect("the contract reads"),
        theirs
    );

    // Two answers in play: there is nothing unambiguous to inherit, so a fresh
    // root gets the defaults rather than whichever one happened to be read
    // first. Guessing here is the whole defect.
    let third = Config {
        summary_language: crate::config::Language::parse("Spanish")
            .expect("a usable language name"),
        ..Config::default()
    };
    setup::setup(codex, &third, &options).expect("codex installs");
    let opencode = find_agent("opencode").expect("opencode is an adapter");
    let two = installed_configs(&[claude, codex], &options);
    assert_eq!(two.len(), 2);
    if crate::skill::installed_config(
        &setup::resolve_paths(opencode, &options)
            .expect("paths")
            .skill_root,
    )
    .is_err()
    {
        assert_eq!(
            config_for(opencode, &two, &options).expect("the contract reads"),
            Config::default()
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

/// Every Rust source in the crate that ships, read from disk.
///
/// Test code is cut away — files named `tests.rs`, and everything after a
/// `#[cfg(test)]` in a file that has one. A fixture is allowed to invent a code
/// (`outcome.rs` builds a `board-unknown` to show what a missing resolution
/// prints), and holding the inventory to codes no build can raise would turn
/// this guard into a chore that gets weakened rather than obeyed.
/// Everything in a source file before its tests begin.
///
/// The tests begin at a `#[cfg(test)]` that introduces a **module** — whatever
/// that module is called. This used to cut at the first `#[cfg(test)]` of any
/// kind, and the attribute also sits on test-only `use` lines: `harness/mod.rs`
/// carries one on line 32, so the file where the gate's own refusals live was
/// truncated to its imports and handed the guard nothing. Five codes went
/// unseen by the guard whose whole purpose is finding refusals nobody
/// ratcheted, in the file that matters most for it, and it went on passing —
/// which is the defect its own comment says it was written to end.
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

/// The same source with its comment lines gone.
///
/// A structural pin asks whether the code still says something, and prose in a
/// comment answers for it. Measured: `declaring_and_lifting_a_stand_down_go_on_
/// the_record` pins `"nothing was in force"`, which is a `say!` on line 1711
/// and, forty-four lines below, a sentence in a comment explaining what `null`
/// means. Deleting the `say!` left the test green.
///
/// This crate keeps its reasoning in comments on purpose, so the density of
/// prose that can stand in for code is unusually high — which makes stripping
/// it the difference between a pin and a coincidence.
///
/// Whole lines only, and deliberately: a `//` inside a string literal is not a
/// comment, and a stripper that did not know the difference would cut a URL out
/// of the code it is checking.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
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
            let shipped = shipped_part(&source).to_owned();
            found.push(shipped);
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
    // The guard on the guard. A `Refusal::not_started(` in the crate whose code
    // is not in the inventory is a rejection the ratchet never checked.
    //
    // This used to be fourteen hand-written `include_str!` lines, and it was the
    // same defect this crate keeps finding: one end written by hand, the other
    // in code, nothing crossing them. A refusal raised in a *fifteenth* file was
    // read by nobody and ratcheted by nobody, while the test went on passing —
    // which is exactly what happened when `src/wizard.rs` arrived carrying two.
    // The other end is the directory, so the fix is to read the directory, the
    // same way `tests/guards.rs` reads it for the population declarations.
    let sources = crate_sources();
    let covered = inventory()
        .into_iter()
        .chain(after_writing())
        .map(|refusal| refusal.code.to_owned())
        .collect::<Vec<_>>();
    // What is left of a carve-out that used to hold four, and the reasons the
    // two survivors are here — which are not the reason that was written.
    //
    // It read "codes only reachable with a filesystem, exercised in their own
    // modules". `skill-not-installed` and `contract-not-writable` do need a
    // filesystem, and a temporary directory is one: both come out of
    // `rewrite_configuration` and are in `inventory()` now, by their real
    // constructors, the same way `config-local-unreadable` is. Neither was
    // exercised anywhere before that — they were excused from the ratchet by a
    // sentence that happened to be true and was never acted on.
    //
    // `setup-write-failed` stays, and the sentence was wrong about it twice:
    // `setup_failed` touches no disk at all, and what actually keeps it out is
    // that `inventory()` is the *pre-flight* refusals — every member has to be
    // `NotStarted`, and this one reports `Unknown` because setup writes several
    // files and an error on the third says nothing about the first two. Putting
    // it in was this round's own mistake, and the crate caught it.
    let elsewhere = [
        // Raised after the world was touched, so it cannot join the pre-flight
        // inventory. Checked here instead, on the property that makes it
        // different: an outcome nobody can read has to send the reader to look.
        "setup-write-failed",
        // The transport's own vocabulary. Estigia translates these rather than
        // raising them, and `harness::tracker` exercises every arm of the
        // translation against a real answer shape — including this one, at
        // `tracker.rs`'s own assertion on the code.
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
    // `harness::tracker::stable_code` lists the transport's own reason strings so
    // a borrowed one never leaks into a `&'static str` code. They are answers
    // Estigia *translates*, not refusals it raises, and every arm of that
    // translation is exercised in `harness::tracker`.
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

/// The codes a `fn code(…) -> &'static str` hands out, one per match arm.
///
/// A third way a refusal gets its code, and the walk could not see it: the
/// stand-down rejections carry theirs on an enum, and `stand_down` builds the
/// refusal with `Refusal::not_started(why.code(), …)` — no literal anywhere
/// near the constructor. Found by the crossing that reads the inventory and
/// looks for where each entry is raised, which is the direction that fails when
/// this reader goes blind: from the other side, a shape it cannot see simply
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

/// A contract nobody could read narrows the reviewer's grant, measured.
///
/// This replaces an assertion that read `gate_context`'s **source** for the words
/// `if unreadable`. Three blind reviewers each defeated that one independently —
/// `if unreadable && false`, a second guard bound to `false`, and a `let narrow`
/// that swapped the arms — every time with the whole suite green. A guard that a
/// reader can satisfy while the property is false is not a guard.
///
/// It was written that way on a stated excuse: that a behavioural twin would have
/// to move `HOME`, and moving `HOME` on this platform has destroyed a profile
/// twice. The danger is real and the excuse was not — the narrowing is a pure
/// function of two values, and nothing about testing it touches a home directory.
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

/// The first string literal after each `Refusal::not_started(` or `code:`.
fn codes_in(source: &str) -> Vec<String> {
    let mut codes = Vec::new();
    codes.extend(codes_named_by_a_code_method(source));
    for marker in ["Refusal::not_started(", "code: "] {
        let mut rest = source;
        while let Some(position) = rest.find(marker) {
            rest = &rest[position + marker.len()..];
            // Only a literal that follows the marker immediately. `code: 5` in a
            // test used to reach forward to the next quote on the line and
            // report whatever string it found as a raised code.
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
fn wrapping_counts_characters_rather_than_bytes() {
    // A terminal wraps at columns. `len()` answers in bytes, so a sentence with
    // accents wrapped short — invisibly, because ragged output looks like a
    // choice rather than a defect, and it only showed up for people reading
    // messages in a language that is not English.
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
    // The record on disk is **state**: the next declaration writes over it, and
    // the reason and the declarer of the one it replaced are gone. For the one
    // act that loosens the gate itself that leaves nothing to answer for —
    // which is the whole shape this feature claims to have. The ledger is
    // append-only and is where it belongs.
    //
    // It also closes the rolling window: the four-hour cap is enforced per
    // declaration, so re-declaring resets the clock. That is still allowed —
    // refusing it would make the harness the thing people uninstall — but every
    // one of them is now a line saying so, rather than one file that only ever
    // shows the latest.
    let source = code_of(include_str!("mod.rs"));
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
    // On a machine with nothing installed, `config set "Merge strategy" octopus`
    // said "run setup". So the operator installed, retyped, and only then
    // learned that `octopus` was never a merge strategy: two round trips for
    // two faults both knowable from the command line, and the one they could
    // fix without leaving the terminal was the one that went unmentioned.
    let refusal = super::config_set(
        "Merge strategy",
        "octopus",
        None,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("octopus is not a merge strategy");
    assert_eq!(refusal.code, "config-value-unrecognised");
    // And it names what may be written instead, which is the whole point of
    // getting this refusal rather than the other one.
    let message = format!("{refusal}");
    assert!(message.contains("squash"), "{message}");

    // The label is still checked first, because a value cannot be judged
    // against a setting nobody named.
    let refusal = super::config_set(
        "Nonexistent thing",
        "octopus",
        None,
        false,
        &SetupOptions::default(),
        false,
    )
    .expect_err("that is not a setting");
    assert_eq!(refusal.code, "setting-unknown");

    // An agent nobody has heard of is refused before the value, because it
    // names which table the value would be judged against — and because the
    // rule is worth being able to state in one line: everything knowable from
    // the command line is settled before anything on disk is read, in the order
    // the arguments were written.
    for value in ["squash", "octopus"] {
        let refusal = super::config_set(
            "Merge strategy",
            value,
            Some("nosuchagent"),
            false,
            &SetupOptions::default(),
            false,
        )
        .expect_err("that is not an agent");
        assert_eq!(
            refusal.code, "agent-unknown",
            "with {value:?} the agent went unmentioned"
        );
    }
}

#[test]
fn status_does_not_say_a_gate_is_on_when_it_could_not_run() {
    // `gated` says an entry exists in a settings file. That is not the same as
    // an entry naming an event this build has and an executable still on disk —
    // and the entry on the machine this was written on named a debug build
    // inside a working tree, which `cargo clean` deletes.
    //
    // `doctor` reports it in full. This is checked separately because `status`
    // is what people read first, and somebody who reads `gate on` closes the
    // terminal.
    let source = code_of(include_str!("mod.rs"));
    // **Per row**, not per file. There are two — the gate and the tool server —
    // and this asked only whether the words appeared somewhere, which is
    // satisfied by either one of them alone. Measured: changing the `tools`
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

#[test]
fn nothing_the_gate_reads_is_a_row_one_agent_can_answer_for_itself() {
    // `gate_context` builds what every decision consults, from a contract read
    // **without a slug** — and it has no slug to read with. What reaches the
    // gate is a dialect: a response shape, not an identity. Two adapters
    // register `exit-code` between them and three send no dialect at all.
    //
    // So a row the gate consults cannot differ by agent, however much it might
    // want to. `Renewal window` was declared per agent and enforced from the
    // shared table for years of this file\'s life: `config set --agent` wrote
    // it, `config list --agent` read it back, and `within_window` never saw it.
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
        &SetupOptions::default(),
        false,
    )
    .expect_err("a repository row was written for one agent");
    assert_eq!(refusal.code, "setting-not-per-agent");
}

#[test]
fn the_push_hook_does_not_invent_a_working_directory_it_could_not_read() {
    // The gate decides coverage by comparing the checkouts a claim records —
    // absolute, every one — against the directory it was handed. This handed it
    // `.` whenever `current_dir()` failed, which matches none of them, so the
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
    let source = code_of(include_str!("mod.rs"));
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
    // row is malformed would make the harness the thing people uninstall.
    //
    // What was not deliberate is that the default is *looser* than what an
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

    // The mechanism the fix rests on: a window of zero is not a short window,
    // it is no permission at all. A run verified this very second still may not
    // ride the answer.
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(12);
    run.mark_verified();
    assert!(
        !run.within_window(Duration::ZERO),
        "a zero window still let a write ride an answer the tracker gave earlier"
    );

    // And the wiring, structurally: this process reads the real home, so a test
    // cannot hand `gate_context` a contract of its own. What it *can* do is
    // hold the file to the rule.
    let source = code_of(include_str!("mod.rs"));
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

    // What is left of it is still said where the operator already looks, and the
    // sentence had to narrow with the behaviour: a message describing what the
    // gate used to do is worse than none, because it will be believed.
    let doctor = include_str!("../harness/doctor.rs");
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
        "`Merge strategy` is \"octopus\", which is not one of its values",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`Merge strategy`: `merge commit`, `squash`, or `rebase`",
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
        format!("{refusal}").contains("`merge commit`, `squash`, or `rebase`"),
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
    // `merge commit` where their file said `squash`, and saving wrote back what
    // it had shown. One function, asked at both writes.
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
    config.merge = crate::config::MergeStrategy::Squash;
    setup::setup(adapter, &config, &options).expect("the install writes");
    assert_eq!(
        existing_config(adapter, &options)
            .expect("a readable contract")
            .map(|held| held.merge),
        Some(crate::config::MergeStrategy::Squash),
        "the contract that was just written does not read back"
    );

    // A value the parser refuses, in the operator's own file beside it.
    let paths = setup::resolve_paths(adapter, &options).expect("paths");
    std::fs::write(
        paths.skill_root.join(crate::config::LOCAL_FILE),
        "| Setting | Value |\n|---|---|\n| Merge strategy | octopus |\n",
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

    // `Change` splits hairs on purpose: `Replace` exists apart from `Update`
    // because "Estigia's own copy" and "somebody else's file" are different
    // sentences, and `Unrecorded` exists apart from `Kept` because "this was
    // here before Estigia" is a fact a record establishes and "there is no
    // record" is the absence of one. Its own doc spells out the harm: saying
    // the first while meaning the second "tells an operator their files predate
    // an install that in fact wrote them".
    //
    // Three variants then shared the word `kept` in the column somebody reads.
    // An operator whose record had been deleted was shown fourteen rows saying
    // `kept` about fourteen files Estigia had written itself.
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

    // Eight adapters share the neutral skill root, so a sharer with no
    // directive of its own is told "not configured" — otherwise installing for
    // one agent reported seven faults it had not caused. That is right for the
    // seven, and it swallowed the eighth: `setup opencode --skill-only` leaves
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

    // `gate_context` reads `installed_config`, not `installed_config_for`:
    // there is no agent to narrow to at gate time — the same gate answers for
    // whichever agent is holding the tools. So a setting the gate consults that
    // could be written per agent is one whose per-agent value nothing would
    // ever read.
    //
    // That is not hypothetical. `Window` was declared per agent once, and its
    // note still says what that cost: "a row `config set --agent` would write,
    // `config list --agent` would read back, and no decision would ever
    // consult". It was moved here. Nothing stops the next one going the other
    // way, and it would go quietly — the gate would keep working, on a value
    // the operator had changed and it never saw.
    for setting in [
        Setting::Integration,
        Setting::Window,
        Setting::Tracker,
        Setting::Boundaries,
        // Arrived with issue 83, and this test is what said it had to be here:
        // the gate reads it to render the reserved reviewer's tool grant, and a
        // per-agent answer to it would be written, read back, and never
        // consulted by the one gate that acts on it.
        Setting::Evidence,
    ] {
        assert!(
            EVERYWHERE_SETTINGS.contains(&setting),
            "{setting:?} is read by the gate and can be written per agent, so a \
             per-agent value for it would be written, read back, and ignored"
        );
    }

    // And the list above is the whole of it. Counted from the source, because
    // the failure this guards is a *new* field arriving from the configuration
    // — which a hand-written list cannot notice about itself.
    let source = code_of(include_str!("mod.rs"));
    let (start, _) = source
        .match_indices("fn gate_context(")
        .next()
        .expect("the gate's context is built here");
    let body = &source[start..];
    let end = body.find("\n}\n").unwrap_or(body.len());
    assert_eq!(
        body[..end].matches("installed.").count(),
        5,
        "the gate reads a different number of settings than this test names, \
         and every one of them has to be repository-wide"
    );
}

/// `sync` keeps both halves of what it promises, for every adapter.
///
/// *"Bring an installed skill up to this binary's copy, **keeping the
/// configuration that is already there**"* — two claims, and nothing tested
/// either end to end. The dangerous one is quiet: a sync that resets the table
/// hands an operator the defaults, and defaults **loosen**.
///
/// Both are checked here, on the whole table rather than on a few rows of it —
/// which is the mistake the round before this one made and did not catch.
#[test]
fn sync_moves_the_markdown_forward_and_keeps_the_table() {
    use crate::setup::AGENTS;

    let mine = Config {
        merge: crate::config::MergeStrategy::Squash,
        integration: crate::config::Integration::Trunk,
        judges: crate::config::Judges::TwoBlind,
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
            "| Setting | Value |\n|---|---|\n| Planning | sdd |\n",
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

        // Half one: every row survives — except the one their own file
        // overrides, which is the design and is asserted as such rather than
        // excused.
        let mut expected = mine.clone();
        expected.planning = crate::config::Planning::Sdd {
            openspec: false,
            lite: false,
        };
        for setting in crate::config::SETTINGS {
            assert_eq!(
                setting.value_of(&after),
                setting.value_of(&expected),
                "{}: sync did not keep {}",
                adapter.slug,
                setting.label()
            );
        }
        assert_eq!(
            after.planning, expected.planning,
            "{}: their own file stopped overriding the contract",
            adapter.slug
        );

        // Half two: the markdown is this binary's copy now.
        //
        // Asked of the contract, not of `after`. `after` is the **layered**
        // configuration — the operator's `Planning` override on top — and
        // asking whether the installed files match *that* is asking whether
        // `SKILL.md` carries a value the override exists to keep out of it. It
        // cannot, by design, so this assertion held only while `sync` was
        // writing the override into the versioned block. That is what it was
        // doing, and it is the defect this pair now names.
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
                .planning,
            mine.planning,
            "{}: sync promoted an `estigia.local.md` row into the versioned block",
            adapter.slug
        );

        // And their file is still theirs.
        assert!(
            std::fs::read_to_string(&ours)
                .unwrap_or_default()
                .contains("sdd"),
            "{}: sync wrote over the one file Estigia never writes",
            adapter.slug
        );
    }
}

/// Every `Resolution::run` string this crate holds, read out of its own source.
///
/// The inventory above is built by hand, and a hand-built population is one
/// somebody has to remember to add to. It covered fourteen commands where the
/// source held twenty-two, and the module this tests says it "runs the real
/// parser over every string this crate suggests" — which was not true.
///
/// The four it could not reach are the ones an operator is most likely to type:
/// `estigia sync`, `estigia guard` and `estigia setup <agent>` come from
/// `doctor`, and a doctor's resolution is a `Health::Broken`, not a `Refusal`,
/// so no list of refusals can contain it however carefully it is kept.
fn suggested_in_source() -> Vec<(String, String)> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    let mut stack = vec![root.join("src")];
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
            // A `#[cfg(test)]` module may build whatever fixture it needs.
            //
            // Through `shipped_part`, which is the same rule the refusal walk
            // uses and for the same reason: splitting on the bare attribute
            // stops at a test-only `use`, and `harness/mod.rs` carries one on
            // line 32 of a thousand. Every command that file suggests was
            // outside this guard, in the file the gate refuses from — and the
            // whole promise being checked is that a message names a command
            // only when running it discharges the block.
            let text = shipped_part(&text);
            let mut rest = text;
            while let Some(at) = rest.find("Resolution::run(") {
                rest = &rest[at + "Resolution::run(".len()..];
                let Some(open) = rest.find('"') else { continue };
                let Some(close) = rest[open + 1..].find('"') else {
                    continue;
                };
                let literal = &rest[open + 1..open + 1 + close];
                if literal.starts_with("estigia ") || literal.starts_with("cargo ") {
                    found.push((path.display().to_string(), literal.to_owned()));
                }
            }
        }
    }
    found
}

#[test]
fn every_command_the_source_suggests_parses_and_not_only_the_listed_ones() {
    let suggested = suggested_in_source();
    assert!(
        suggested.len() > commands_named().len(),
        "the source scan stopped finding what the hand list already has, so it \
         is checking nothing: {} found",
        suggested.len()
    );

    for (file, command) in suggested {
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
}

/// Lifting a stand-down says so only when the record is actually gone.
///
/// The record is the whole of the loosening: while it is on disk and in force,
/// the gate stands aside and writes go through unadjudicated. `--lift` removed
/// it with the result discarded and then printed *the gate decides on its own
/// again* either way, so a removal that failed was reported as a lift — with
/// exit 0, which is what a script wrapping this reads.
///
/// Measured on the product before the fix, with the file held open by another
/// process: *lifted*, exit 0, and `doctor` in the same breath reporting the
/// gate standing down for another 30 minutes.
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

/// Every code the inventory carries is one the crate can actually raise.
///
/// The way back, and it was never written. Its neighbour walks the tree and
/// proves each refusal reached the ratchet; nothing walked the inventory. An
/// entry that outlives the refusal it described is a rejection the ratchet
/// keeps reviewing and no operator can ever meet — the review passes, the
/// vocabulary grows a word nothing says, and the count of what this crate
/// refuses stops being true.
///
/// It is also the half that keeps the *other* half honest. The guard on raised
/// codes reads the tree, and reading it wrongly is invisible from that side:
/// a file it fails to open simply yields no codes, and a test looking only for
/// codes-without-entries passes louder the less it can see. This direction
/// fails when the reader goes blind, because entries stop finding their sites.
#[test]
fn every_code_the_inventory_carries_is_one_the_crate_raises() {
    // Not a refusal code at all: `companion-state` is the wrapper the inventory
    // builds to put each companion's *resolution* through the ratchet, which is
    // why its message is the word `probed`. The exemption is checked rather
    // than asserted — if something ever raises it, this is a real code wearing
    // a placeholder's name and the carve-out is hiding it.
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

/// A code's entry says the same **kind** of way out as the code that raises it.
///
/// The ratchet crosses which codes exist, both ways, and stopped there. Thirty-six
/// of the entries are hand-built replicas rather than calls into the code that
/// raises them, and a replica agrees on the one attribute that is crossed and
/// may differ on every other. What the ratchet actually reviews about a refusal
/// is its *way out* — whether it names a command, and whether that command
/// discharges the block — so an entry naming a different kind than the real one
/// puts the wrong thing through the review and passes.
///
/// It found one. `environment-not-ready` was reviewed as a `no_command`, while
/// the code fell back to `Resolution::run("estigia doctor")` — on a refusal
/// whose whole content is *this doctor check is broken*, a command that prints
/// the same broken check back, which is the dead end the ratchet forbids. It sat
/// in a branch nothing could reach, because a broken check always carries a
/// resolution and only broken ones get here; the fallback is gone and the
/// resolution is taken apart from the check rather than asked for twice.
///
/// Codes built dynamically carry no literal to compare and are skipped, which
/// is stated rather than silent: the floor below fails if that becomes most of
/// them.
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
                let end = rest
                    .find("Refusal::not_started(")
                    .map_or(rest.len(), |next| next);
                // Back off to a character boundary. The cap is a byte count and
                // this crate's source is full of em dashes, so slicing at a flat
                // 900 panics the moment one straddles it — a test that fails on
                // where a sentence happens to fall, not on what it says.
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
    // The door the layer needed. Reading a repository's own file has been
    // there since it existed; creating one took a text editor, and a feature
    // that needs one is a feature nobody has.
    //
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
    let refusal = super::repository_set(Setting::Merge, "rebase", repo.path(), &options)
        .expect_err("a folder that is not a checkout is not a repository");
    assert_eq!(refusal.code, "not-a-repository");
    assert!(
        !repo.path().join(".git").exists(),
        "a `.git` was created where there was no checkout"
    );
    std::fs::create_dir_all(repo.path().join(".git")).expect("now it is one");

    super::repository_set(Setting::Merge, "rebase", repo.path(), &options)
        .expect("the repository takes it");
    let written = std::fs::read_to_string(&path).expect("the file was created");
    assert!(
        written.contains("rebase"),
        "the answer is not in the file it was written to: {written}"
    );

    // And a second row does not undo the first.
    super::repository_set(Setting::Tracker, "github", repo.path(), &options)
        .expect("the repository takes it");
    let written = std::fs::read_to_string(&path).expect("the file is still there");
    assert!(
        written.contains("rebase"),
        "a second `--repo` undid the first one's row: {written}"
    );

    // A row about an agent is refused at the door rather than written and put
    // back silently.
    let refusal = super::repository_set(Setting::Models, "unset", repo.path(), &options)
        .expect_err("an agent's row is not the repository's to answer");
    assert_eq!(refusal.code, "setting-not-the-repositorys");
}

/// The screen writes the checkout it is **pointed at**, not the one it is in.
///
/// The dropdown lets somebody load another checkout's answers without going
/// there. The write never learned about it: it asked the process where it was
/// standing. So loading repository B, changing a row and saving wrote B's value
/// into **A**'s file — one repository's rows into another, which is the single
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
                | Merge strategy | merge commit | merge commit |\n";
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
                merge: crate::config::MergeStrategy::Squash,
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
        read(named.path()).contains("squash"),
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
/// and said nothing about the rows it did not — the operator reads a success
/// and the repository keeps answering with what it said before.
///
/// It is the same distinction `override_text` exists to hold, at the door where
/// getting it wrong costs a write rather than a reading.
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

/// Both doors to the gate read one payload the same way.
///
/// There are two: the lifecycle hook, and `estigia gate` — which is not a
/// convenience, it is how OpenCode is gated, because that adapter's plugin
/// shells out to it on every edit. They disagreed about the same bytes twice
/// over, and each disagreement cost the second door the decision:
///
/// - Windsurf's `command_line` was copied to `command` inside the hook, so the
///   spelling was translated for one door and invisible to the other. The copy
///   is gone; the classifier reads a key by its letters, which serves both.
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
                local_fast_forward_target: None,
            },
            "`{spelling}` is a push and this door does not see it"
        );
    }
}

/// Every command the parser has is one the JSON guard poses.
///
/// `--json` is declared once, on the root parser, and honoured in as many
/// places as there are commands — so the guard that runs them all is a **list**,
/// and a list beside the code is a list that stops matching it. It did:
/// `config repos` and `config forget` were written after it and never added, so
/// both printed prose under the flag for as long as they existed.
///
/// This does not run anything. It asks the parser for its own commands and
/// requires each to be named in that guard's source, which is what makes
/// forgetting one impossible rather than unlikely — the arguments still have to
/// be chosen by hand, because a dry run and a harmless id are not something a
/// parser can invent.
#[test]
fn every_command_the_parser_has_is_one_the_json_guard_poses() {
    use clap::CommandFactory;
    let guard = include_str!("../../tests/pipe.rs");
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
        // The ones that print nothing to read: `mcp` speaks a protocol and
        // `hook` answers an agent, and neither has prose for `--json` to be an
        // alternative to.
        if matches!(command.get_name(), "mcp" | "hook" | "__record-install") {
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
            if leaf == "\"config\", \"edit\"" {
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
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(crate::skill::DIRECTORY);
    crate::skill::install(&skill, &Config::default(), false).expect("the skill installs");

    let mut wanted = Config::default();
    Setting::Judges
        .apply(&mut wanted, "two blind")
        .expect("the value is one it takes");

    // Nothing shadowing it: the write reads back as written.
    let file = crate::skill::agent_override(&skill, "claude-code");
    let snapshot = crate::setup::agent_configuration_snapshot(&file).expect("the file is absent");
    super::write_edited_table(&file, Some(("claude-code", &snapshot)), &wanted)
        .expect("a table nothing overrides is written and read back");
    assert_eq!(
        crate::skill::installed_config_for(&skill, Some("claude-code"))
            .expect("the config reads")
            .judges,
        wanted.judges,
        "the row did not survive the write, so this measures nothing"
    );

    // Now the operator's own file answers that row, and the write is one no run
    // will ever read.
    std::fs::write(
        skill.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |\n|---|---|\n| Blind judges | single |\n",
    )
    .expect("the override is written");
    let snapshot = crate::setup::agent_configuration_snapshot(&file).expect("the file reads");
    let refusal = super::write_edited_table(&file, Some(("claude-code", &snapshot)), &wanted)
        .expect_err("a row nothing will read was reported as written");
    assert_eq!(refusal.code, "setting-shadowed-by-local-file");
    assert!(
        refusal.message.contains("Blind judges"),
        "the refusal does not name the row: {}",
        refusal.message
    );

    // And a repository row is not reported as shadowed by the reader putting it
    // back where it belongs, which is what comparing the whole table would do.
    let mut repository = Config::default();
    Setting::Merge
        .apply(&mut repository, "rebase")
        .expect("the value is one it takes");
    std::fs::remove_file(skill.join(crate::config::LOCAL_FILE)).expect("the override goes");
    let snapshot = crate::setup::agent_configuration_snapshot(&file).expect("the file reads");
    super::write_edited_table(&file, Some(("claude-code", &snapshot)), &repository)
        .expect("a repository row an agent file cannot carry was called shadowed");
}

/// An agent whose files another entry of the same run repairs is not current.
///
/// Eight of the eleven adapters have no skill directory of their own and share
/// the neutral root. The run writes it once — correctly — so the other seven
/// come out at nought files, and every one of them said `already current`.
/// That is a claim about the operator's disk, and on a stale install it is
/// false: those files are out of date, `status` says so, and `sync --agent
/// <that one>` on its own changes them. The plan repaired them three lines up
/// under a name the operator has no reason to connect to the agent they asked
/// about.
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
    let gemini = setup::find_agent("gemini-cli").expect("gemini-cli is an adapter");
    let config = Config::default();

    // Both installed, sharing one skill root, and then the shared contract is
    // left stale — the ordinary state of a machine one release behind.
    setup::setup(neutral, &config, &options).expect("the neutral root installs");
    setup::setup(gemini, &config, &options).expect("gemini-cli installs");
    let root = setup::resolve_paths(gemini, &options)
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
        let done: Vec<_> = [neutral, gemini]
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
    let gemini_section = stale
        .split(gemini.display_name)
        .nth(1)
        .unwrap_or_else(|| panic!("gemini-cli has a section of its own:\n{stale}"));

    // The floor for the fixture itself: the plan does name the broken file, so
    // a run of this test against a contract that was never broken would fail
    // here rather than pass on a report with nothing in it.
    //
    // Any of the three words: the fixture rewrites the contract **by hand**
    // after an install recorded what it wrote there, so the record can tell the
    // file apart from the one Estigia left. That is `OVERWRITE` — the record
    // claims the path and the contents have moved, which is Estigia's file with
    // somebody's edit in it. It read `REPLACE` until the two states were told
    // apart, and a machine merely one release behind reads as `update`, because
    // its record predates digests and holds none for the path.
    assert!(
        (stale.contains("would OVERWRITE")
            || stale.contains("would REPLACE")
            || stale.contains("would update"))
            && stale.contains("SKILL.md"),
        "the stale contract is repaired somewhere in this plan:\n{stale}"
    );
    assert!(
        !gemini_section.contains("already current"),
        "gemini-cli's contract is stale and repaired above, and it was called \
         already current:\n{stale}"
    );
    assert!(
        gemini_section.contains("shares the ones listed above"),
        "gemini-cli is told where its files are dealt with:\n{stale}"
    );

    // The other half. With nothing stale the phrase is true, and has to stay:
    // a fix that never says `already current` again has only moved the lie.
    //
    // Repaired for real first — a second plan against the same untouched disk
    // would measure the stale case twice and call the agreement a floor.
    plan(&options);
    let current = plan(&dry);
    let section = current
        .split(gemini.display_name)
        .nth(1)
        .expect("gemini-cli has a section of its own");
    assert!(
        section.contains("already current"),
        "with the contract current, gemini-cli says so:\n{current}"
    );
}

/// An operator's change size survives the release that follows it.
///
/// The number lived in prose, in two shipped files, and `estigia sync` rewrites
/// both. Measured before this row existed: a team that lowered `800 changed
/// lines` to `300` in `SKILL.md` and `references/repository-delivery.md` got
/// `800` back on the next sync, under two lines reading `update` — the same two
/// words a version bump writes, so nothing said their policy had been dropped.
/// There was no supported way to keep it either: the local override file
/// replaces **rows**, and this was a sentence.
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

/// Every refusal the crate can raise is one this file reviews.
///
/// The guard above crosses the *kind* of way out against the source, and it can
/// only do that for a code with a row. So a code without one is a way out
/// nobody reviews — and there was exactly one: `setup-write-failed`, built as a
/// struct literal rather than through `Refusal::not_started`, which is how it
/// stayed out of a list assembled from constructors.
///
/// It is also the one whose way out changed. It named `estigia status`, a
/// command that answers *skill out of date* and discharges nothing, and moving
/// it to the write access only a person can give passed a green suite. Nothing
/// noticed, because nothing was looking at that code at all.
///
/// The population is walked rather than listed, for the reason the walk in
/// `a_live_call_carries_the_operators_table` gives: a guard that names the
/// places it has already been burned by cannot see the next one.
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
        for opener in ["Refusal::not_started(", "code: "] {
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

/// A row an agent's own file answers is not reported as now in force.
///
/// Measured on a real machine before this existed: `config set Planning direct`
/// answered *Planning is now direct*, the three shared tables on disk all said
/// `direct`, and `config list` one command later said `sdd lite` — because nine
/// `estigia.<slug>.md` files still carried that row and the readback looked at
/// the shared table only.
///
/// The comment over that readback says *"read back what the operator will
/// actually read"*, and it was not true of what it read: eight adapters share
/// the neutral root, `installed_config_for` lays each one's own file over the
/// table, and the value a run gets is the overlaid one. Reporting an effect that
/// did not happen, in the configuration of the tool whose whole purpose is
/// refusing exactly that.
#[test]
fn a_row_an_agents_own_file_answers_is_not_reported_as_in_force() {
    let (home, options) = sandbox();
    // An adapter that **shares** the neutral root, which is where the defect
    // lives: one with a skill directory of its own has `--agent` write the
    // contract itself, so there is no second file to disagree with.
    let adapter = crate::setup::find_agent("opencode").expect("an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");

    // The agent's own file takes the row, through the door that writes one.
    super::config_set(
        "Planning",
        "sdd lite",
        Some("opencode"),
        false,
        &options,
        false,
    )
    .expect("the per-agent write lands");

    // Now the shared table is told something else. It is written — and it is
    // not what that agent reads, so saying it is now in force would be false.
    let refusal = super::config_set("Planning", "direct", None, false, &options, false)
        .expect_err("the shared write was reported as in force while an agent read otherwise");
    assert_eq!(refusal.code, "setting-shadowed-by-agent-file");
    assert!(
        refusal.message.contains("opencode"),
        "the refusal does not name the agent still reading something else: {refusal}"
    );
    // Committed, because the table really was written: telling the operator
    // nothing happened would send them to repeat a write that landed.
    assert_eq!(refusal.outcome, MutationOutcome::Committed);
    // And the way out is a command that clears it, unlike its operator-file
    // sibling where there is nothing Estigia may edit.
    assert!(
        format!("{refusal}").contains("--agent"),
        "the resolution does not name the command that discharges it: {refusal}"
    );

    // The floor: with no per-agent file in the way, the same write is accepted.
    let (_second, fresh) = sandbox();
    crate::setup::setup(adapter, &Config::default(), &fresh).expect("setup runs");
    super::config_set("Planning", "sdd", None, false, &fresh, false)
        .expect("a shared write nothing shadows was refused");
    let _ = home;
}

#[test]
fn config_writes_never_mutate_the_static_reviewer() {
    let (home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("setup runs");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let before = fs::read(&reviewer).expect("the static reviewer reads");

    super::config_set(
        "Model routing",
        "judge=opus",
        Some(adapter.slug),
        false,
        &options,
        false,
    )
    .expect("model routing is written");
    assert_eq!(fs::read(&reviewer).expect("the reviewer remains"), before);

    let writable = super::writable_config(Some(adapter.slug), &options)
        .expect("the installed configuration is writable");
    let mut edited = writable.config.clone();
    Setting::Judges
        .apply(&mut edited, "five blind")
        .expect("five blind parses");
    let agent_write = writable
        .agent_snapshot
        .as_ref()
        .map(|snapshot| (adapter.slug, snapshot));
    super::write_edited_table(&writable.target, agent_write, &edited)
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
    Setting::Planning
        .apply(&mut selected, "sdd lite")
        .expect("Planning is accepted");
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
    let second = crate::setup::find_agent("gemini-cli").expect("Gemini is an adapter");
    for adapter in [first, second] {
        crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    }
    let root = crate::setup::resolve_paths(first, &options)
        .expect("the shared root resolves")
        .skill_root;
    let mut selected = Config::default();
    Setting::Planning
        .apply(&mut selected, "sdd lite")
        .expect("Planning is accepted");
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
        Setting::Planning.value_of(
            &crate::skill::installed_config_for(&root, Some(first.slug))
                .expect("the first override reads back")
        ),
        "sdd lite",
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
            .is_some_and(|settings| settings.contains(&Setting::Planning))
            && refusal
                .receipt
                .acknowledged
                .get(second.slug)
                .is_some_and(|settings| !settings.contains(&Setting::Planning)),
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
    Setting::Planning
        .apply(&mut selected, "sdd lite")
        .expect("Planning is accepted");
    let mut effective = selected.clone();
    Setting::Merge
        .apply(&mut effective, "squash")
        .expect("Merge strategy is accepted");
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
            .is_some_and(|settings| settings.contains(&Setting::Planning)),
        "the proven agent override was discarded with the repository failure"
    );
    assert!(
        failure.receipt.repository.is_none()
            && failure
                .receipt
                .acknowledged
                .get(adapter.slug)
                .is_some_and(|settings| !settings.contains(&Setting::Merge)),
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
    Setting::Merge
        .apply(&mut effective, "squash")
        .expect("Merge strategy is accepted");
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
    Setting::Planning
        .apply(&mut selected, "sdd lite")
        .expect("Planning is accepted");
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
    Setting::Merge
        .apply(&mut repository_config, "rebase")
        .expect("the merge strategy is accepted");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &repository_config,
        &[Setting::Merge],
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
        vec![Setting::Merge.label(), Setting::Integration.label()],
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
        .apply(&mut claude_config, "linear")
        .expect("the Claude tracker is accepted");
    let mut codex_config = Config::default();
    Setting::Tracker
        .apply(&mut codex_config, "github acme/issues")
        .expect("the Codex tracker is accepted");
    crate::setup::setup(claude, &claude_config, &options).expect("Claude installs");
    crate::setup::setup(codex, &codex_config, &options).expect("Codex installs");

    let mut repository_config = Config::default();
    Setting::Merge
        .apply(&mut repository_config, "rebase")
        .expect("the initial merge strategy is accepted");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &repository_config,
        &[Setting::Merge],
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
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the new merge strategy is accepted");
    let plan = crate::tui::Plan {
        agents: vec![claude],
        opened: [(claude.slug, claude_opened.clone())].into_iter().collect(),
        rows: [(claude.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };

    let (receipt, _) =
        super::install_planned(&plan, &options, false).expect("the one-row repository save lands");

    assert_eq!(receipt.repository_settings, vec![Setting::Merge]);
    let mut app = crate::tui::app::App::with_configs(
        &std::collections::BTreeMap::from([
            (claude.slug, claude_opened),
            (codex.slug, codex_opened),
        ]),
        &[],
    );
    app.installed_now(receipt);
    assert_eq!(
        Setting::Merge.value_of(&app.installed[claude.slug]),
        "squash"
    );
    assert_eq!(
        Setting::Merge.value_of(&app.installed[codex.slug]),
        "squash"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[claude.slug]),
        "linear"
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
    Setting::Merge
        .apply(&mut first, "squash")
        .expect("the first merge strategy is accepted");
    crate::setup::write_repository_configuration(
        &crate::skill::repository_config_path(repository.path()),
        &first,
        &[Setting::Merge],
    )
    .expect("the first repository snapshot exists");
    let mut second = Config::default();
    Setting::Tracker
        .apply(&mut second, "linear")
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

    assert_eq!(receipt.repository_settings, vec![Setting::Merge]);
    assert_eq!(
        Setting::Merge.value_of(receipt.repository.as_ref().expect("repository evidence")),
        "squash"
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
        .apply(&mut claude_config, "linear")
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
        &[Setting::Merge],
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
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the new merge strategy is accepted");
    let plan = crate::tui::Plan {
        agents: vec![claude],
        opened: [(claude.slug, claude_opened.clone())].into_iter().collect(),
        rows: [(claude.slug, selected)].into_iter().collect(),
        repository: repository.path().to_path_buf(),
    };
    super::inject_install_failure(super::InstallFailurePoint::AfterRepository);

    let failure = super::install_planned(&plan, &options, false)
        .expect_err("the injected post-repository failure was reported as success");

    assert_eq!(failure.receipt.repository_settings, vec![Setting::Merge]);
    let mut app = crate::tui::app::App::with_configs(
        &std::collections::BTreeMap::from([
            (claude.slug, claude_opened),
            (codex.slug, codex_opened),
        ]),
        &[],
    );
    app.installed_partially(*failure.receipt);
    assert_eq!(
        Setting::Merge.value_of(&app.installed[claude.slug]),
        "squash"
    );
    assert_eq!(
        Setting::Merge.value_of(&app.installed[codex.slug]),
        "squash"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[claude.slug]),
        "linear"
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
        &[Setting::Merge],
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
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the new merge strategy is accepted");
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
        &[Setting::Merge],
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
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the new merge strategy is accepted");
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
            .all(|settings| !settings.contains(&Setting::Merge)),
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
        &[Setting::Merge],
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
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the new merge strategy is accepted");
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
            .all(|settings| !settings.contains(&Setting::Merge)),
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
        &[Setting::Merge],
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
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the new merge strategy is accepted");
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
            .all(|settings| !settings.contains(&Setting::Merge)),
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
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "read back the uncertain file before deciding whether to retry",
        ),
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
        replay: crate::outcome::Replayability::ExactReplaySafe,
        resolution: Resolution::run("estigia setup opencode"),
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
    for boundary in [
        crate::setup::SetupFailureBoundary::AfterSkill,
        crate::setup::SetupFailureBoundary::AfterDirective,
        crate::setup::SetupFailureBoundary::AfterPhase,
        crate::setup::SetupFailureBoundary::AfterHooks,
    ] {
        let (_home, options) = sandbox();
        let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
        let mut selected = Config::default();
        Setting::Planning
            .apply(&mut selected, "sdd lite")
            .expect("Planning is accepted");
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
}

#[test]
fn a_deterministic_late_render_failure_is_prevalidated_before_any_write() {
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
}

#[test]
fn an_after_skill_failure_in_a_real_invocations_prevalidation_is_not_mutation_evidence() {
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
}

#[test]
fn malformed_json_keeps_its_typed_not_editable_prevalidation_refusal() {
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
}

#[test]
fn an_unreadable_prevalidation_input_is_not_classified_as_a_write_failure() {
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
}

#[test]
fn an_unconfirmed_setup_write_without_an_earlier_change_is_unknown() {
    let (_home, options) = sandbox();
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial setup");
    let plan = crate::tui::Plan {
        agents: vec![adapter],
        opened: [(adapter.slug, Config::default())].into_iter().collect(),
        rows: [(adapter.slug, Config::default())].into_iter().collect(),
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
        &[Setting::Merge],
    )
    .expect("the repository layer exists");
    let before = std::fs::read(&repository_path).expect("the repository layer reads");
    let mut selected = Config::default();
    Setting::Merge
        .apply(&mut selected, "squash")
        .expect("the merge strategy is accepted");
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
    let second = crate::setup::find_agent("gemini-cli").expect("Gemini is an adapter");
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
    Setting::Planning
        .apply(&mut first_config, "sdd lite")
        .expect("Planning is accepted");
    let mut second_config = Config::default();
    Setting::Planning
        .apply(&mut second_config, "sdd")
        .expect("Planning is accepted");
    let mut repository_config = first_config.clone();
    Setting::Merge
        .apply(&mut repository_config, "squash")
        .expect("Merge strategy is accepted");
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

/// A directory the call named may narrow the decision, and may not move it.
///
/// `tests/pipe.rs` drives the whole path against the real binary and proves both
/// halves end to end. This is the same rule stated where it is written, on the
/// inputs that are awkward to reach through a process: the tool the key is
/// honoured for, the spellings that climb out, and the wrapped payload.
///
/// The distinction it rests on is not stylistic. `cwd` is written by the
/// adapter's hook, which knows what it is gating. `workdir` is a tool
/// **argument**, so whatever composed the call wrote it — a model, in every
/// runtime here. Measured before the clamp existed: `..`, the parent checkout
/// and `C:\Windows` all resolved, were covered by no run, and took the write out
/// of the gate with exit zero while the command ran anyway.
#[test]
fn a_directory_the_call_names_may_narrow_the_decision_and_not_move_it() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let launched = root.path();
    let inside = launched.join("wt-a");
    std::fs::create_dir_all(&inside).expect("a directory inside");

    let narrowed = |payload: serde_json::Value, tool: &str| {
        super::narrowed_by_the_call(&payload, tool, launched)
    };
    // Every expectation is spelled the way the answer is spelled, because the
    // answer is a **placed** path and a directory has more than one name.
    //
    // Comparing against the raw fixture path failed on **both** CI platforms
    // that are not Linux, by two different mechanisms, and that pair is the
    // reason this is a normalisation rather than a spelling:
    //
    // - macOS: the temporary directory is `/var/…`, and `/var` is a symlink to
    //   `/private/var`. Answer `/private/var/…/wt-a`, expectation `/var/…/wt-a`.
    // - Windows: the runner's temporary directory sits under `C:\Users\RUNNER~1`,
    //   the **8.3 short name**. Answer `…\runneradmin\…`, expectation `…\RUNNER~1\…`.
    //   Nothing to do with links. What decides it is how `TEMP` is *spelled*,
    //   which is not the same question as whether a short name exists: this
    //   developer's profile has one (`dir /x C:\Users` → `ASANAB~1  asanabrial`)
    //   and the assertions passed here anyway, because the local variable carries
    //   the long spelling and the runner's carries the short one.
    //
    // Canonicalising the fixture instead only moves the break: on Windows that
    // returns the `\\?\` verbatim form, which `placed` strips. No one spelling is
    // right everywhere, so the expectation goes through the same normalisation as
    // the answer and the assertion compares places rather than names.
    //
    // What that costs, narrowly: a `placed` that normalises wrongly is caught
    // here only when the wrongness moves the answer — because the clamp compares
    // `placed(resolved)` against the raw `launched`, and `wt-a/../nope` differs
    // from its expectation in shape rather than in spelling. What survives
    // unseen is a normalisation that is wrong, applied identically to both sides,
    // and still landing under `launched`. The instance that matters is a leaked
    // verbatim prefix, owned by
    // `a_placed_path_carries_no_verbatim_prefix` in `src/paths.rs`. Not the
    // *only* one constructible: appending or popping a component survives these
    // rows too, and both are caught elsewhere in the suite.
    //
    // **Not** every part of `placed` is owned next door: gutting its `..` collapse
    // leaves all of `paths`' own tests green, and what catches it is
    // `a_write_that_lands_inside_the_claim_is_gated_however_it_is_spelled`, in
    // `src/harness/tests.rs`. Spelled on one line because a name split across two
    // is a name nobody can grep, which this change has already had to fix once.
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

    // What it must never do. Each of these resolves to a real place the gate was
    // not looking, and each one answered `outside` with exit zero before the
    // clamp — the write leaving the gate while the command ran regardless.
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
    // a name for: a `..` that climbs out **past a component that exists**, onto
    // somewhere this process cannot open.
    //
    // Comparison is by resolved path, and resolution of a path that cannot be
    // opened falls back to the spelling as written. `..` is then never cancelled,
    // so `wt-a\..\..\nope` still *starts with* the launch directory and the clamp
    // called it inside — for a path that is not. Worse than the escape it
    // replaced: the first one reached `outside`, and this one reached **allow**,
    // under the claim of whichever worktree the lexical prefix happened to name.
    // A run holding B could borrow A's authority by writing one `..`.
    //
    // The fix is not a second comparison, it is the right primitive: `placed`
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
    // finding: attributed to its spelling it reached the holder of `wt-a` and
    // was **allowed**, where the directory it names is the shared base that two
    // runs cover at equal depth and the ambiguity refusal is what it should get.
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
    // taken as given, including from outside this process's directory, because
    // the hook that writes it may run from anywhere.
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

#[test]
fn a_row_that_reads_back_wrong_with_no_override_beside_it_blames_no_file() {
    // `shadowed` ended in `unwrap_or_else(|| "the local override")`, so a row
    // that read back wrong for any reason at all was reported as an operator
    // file overriding it — and the resolution said to change "those rows" in a
    // file that need not exist. That is a diagnosis the tool cannot make,
    // dressed as one it did: the same failure as reporting a state nobody read
    // back, in the sentence an operator acts on.
    //
    // Reached in the field through `config set --agent <slug> "Summary
    // language"`, which is now refused at the door — so this holds the arm
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
    // ran nothing — the shape this repository has already filed against itself,
    // where sixteen tests answered pass having returned on their first line.
    // The precondition is asserted; the assertions are not guarded by it.
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
