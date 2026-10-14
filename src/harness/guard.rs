//! The gate no agent can route around.
//!
//! An agent hook fires only for the agent that installed it. A `pre-push` hook
//! sits **under git**: it refuses a push typed by any agent, by a person, or by
//! a script, at the boundary that cannot be taken back.
//!
//! # What this is not
//!
//! It is not a replacement for the `PreToolUse` gate. That one catches a write
//! the moment it is attempted, which is where a lost claim race should be
//! caught — hours before a push. This catches the *last* moment, and catches it
//! for everybody.
//!
//! # What routes around it anyway
//!
//! `git push --no-verify`, a push from another checkout of the same repository,
//! and any tool that writes refs without invoking the hook. Git hooks are a
//! guard rail, not a lock, and a guard rail that claims to be a lock is worse
//! than one that does not.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::outcome::{NoCommandReason, Refusal, Resolution};

use super::{Action, Decision, GateContext, Sensitivity, session};

/// The hook Estigia installs, and the only one it installs.
pub const HOOK: &str = "pre-push";

/// The marker that says a hook file is Estigia's: a hook is a script, not a
/// structured file, so a line saying who wrote it is what there is.
pub const MARKER: &str = "# managed by estigia guard";

/// What one repository's guard looks like.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Estigia's hook is installed.
    Installed,
    /// A hook is there, somebody else wrote it, and it calls Estigia's: the
    /// arrangement Estigia itself asks for when it finds a hook it did not
    /// write.
    Chained,
    /// A hook is there, somebody else wrote it, and it does not call Estigia's.
    Foreign,
    /// The gate is in the file, and git will not run it: on a system where git
    /// consults the file mode, a hook without the execute bit is skipped
    /// **silently**.
    Inert,
    /// Nothing is installed.
    Absent,
    /// A hook file is there and reading it failed — not `Absent`, and the
    /// difference is a file somebody else wrote whose ownership cannot be
    /// established at all.
    Unreadable,
}

/// Where git keeps this checkout's hooks.
///
/// `git rev-parse --git-path hooks` rather than `.git/hooks`: in a worktree the
/// admin directory is a file pointing elsewhere, and the isolated checkout is
/// exactly where a delivery is pushed from. Guessing the path would install the
/// guard in the one place the push does not come from.
pub fn hooks_directory(repo_dir: &Path) -> Result<PathBuf, Refusal> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-parse", "--git-path", "hooks"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let path = PathBuf::from(path);
            let path = if path.is_absolute() {
                path
            } else {
                repo_dir.join(path)
            };
            // Git answers in its own separators, always forward slashes. Joined
            // onto a Windows directory that gives `C:\work\repo\.git/hooks`,
            // which every API here accepts and which this command **prints** —
            // and a path an operator is shown is one they will paste somewhere.
            // Walking the components rebuilds it in the platform's own.
            Ok(path.components().collect())
        }
        _ => Err(Refusal::not_started(
            "not-a-repository",
            format!("{} is not a git repository", repo_dir.display()),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "a git repository to install the push guard into — run this from the checkout \
                 the delivery is pushed from",
            ),
        )),
    }
}

/// What is installed in this repository.
pub fn state(repo_dir: &Path) -> State {
    let Ok(hooks) = hooks_directory(repo_dir) else {
        return State::Absent;
    };
    let path = hooks.join(HOOK);
    // The distinction is made here, where the filesystem is, so `state_of` stays
    // the pure function its own doc describes: it is handed the text or told
    // there is none, and "there is a file and it will not open" is not that.
    match fs::read_to_string(&path) {
        Ok(text) => state_of(Some(&text), git_would_run(&path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::Absent,
        Err(_) => State::Unreadable,
    }
}

/// What a hook file is, given its text and whether git would run it.
///
/// **Pure and fed**: the filesystem is asked one question outside.
fn state_of(text: Option<&str>, runnable: bool) -> State {
    let Some(text) = text else {
        return State::Absent;
    };
    if !(ours(text) || calls_estigia(text)) {
        return State::Foreign;
    }
    if !runnable {
        return State::Inert;
    }
    // Somebody else's hook that calls this one is `Chained`.
    if ours(text) {
        State::Installed
    } else {
        State::Chained
    }
}

/// Whether an uninstall may take this hook away.
///
/// **Pure and fed**, like `state_of`. `Inert` is the case this was written
/// for: it means *git will not run this*, which is a fact about the mode and
/// not about who wrote the file. Read for the marker rather than for the
/// state, because a *chained* hook of theirs without the bit is `Inert` too,
/// and that one stays.
fn removable(state: State, text: Option<&str>) -> bool {
    let unchanged = |text: Option<&str>| text.is_some_and(ours) && !edited(text);
    match state {
        State::Installed | State::Inert => unchanged(text),
        State::Absent | State::Foreign | State::Chained | State::Unreadable => false,
    }
}

/// Whether Estigia's hook carries somebody's additions: compared against every
/// line this binary writes, not against a hash, because the script names the
/// executable's path and two machines produce two different files.
fn edited(text: Option<&str>) -> bool {
    let Some(text) = text else {
        return false;
    };
    // Rendered for the path this file itself names, so the comparison is about
    // *content*: a hook installed by an Estigia that has since moved is still
    // unedited.
    let rendered = script(Path::new("x"));
    let mine: std::collections::BTreeSet<&str> = rendered
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && !line.contains("estigia"))
        .collect();
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && !line.contains("estigia"))
        .any(|line| !mine.contains(line))
}

/// Whether Estigia wrote this hook, which is what makes it Estigia's to
/// remove.
fn ours(text: &str) -> bool {
    text.contains(MARKER)
}

/// Whether git would run this hook at all.
///
/// Git skips a hook without the execute bit silently, so the only place that
/// difference exists is the file mode and asking for it is the whole check.
#[cfg(unix)]
fn git_would_run(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
}

/// Git for Windows runs hooks through its bundled shell and never consults the
/// file mode, so there is no bit to be missing and nothing to report.
#[cfg(not(unix))]
fn git_would_run(_path: &Path) -> bool {
    true
}

/// Whether a hook somebody else wrote hands the push to Estigia.
///
/// By the two words that have to be next to each other, not by the path. A
/// **mention** is not a call, and this counted one once. Two shapes are
/// refused, and both are ones that certainly do not run: the words inside a
/// quoted string, and a line whose command word only prints. What this still
/// cannot know is whether a line that *is* a call ever executes. Narrowing is
/// the safe direction but it is not free: missing a real call brings back the
/// era when chained operators were told to chain again at every `doctor`.
fn calls_estigia(text: &str) -> bool {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter(|line| !only_prints(line))
        .any(|line| {
            let words: Vec<&str> = unquoted_words(line);
            words.windows(2).any(|pair| {
                pair[0] == "hook" && pair[1] == crate::harness::hook::Event::PrePush.slug()
            })
        })
}

/// The words of a line that are not inside quotes.
///
/// `'C:\Program Files\estigia.exe' hook pre-push` is the shape Estigia writes
/// itself, so the quoted run is dropped whole and the two words that matter are
/// kept — while `echo "… hook pre-push"` keeps nothing. A quote left open runs
/// to the end of the line, which is what a shell does with it too.
fn unquoted_words(line: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut quote: Option<char> = None;
    let mut word_start: Option<usize> = None;
    let mut characters = line.char_indices().peekable();
    while let Some((at, character)) = characters.next() {
        // A backslash escapes the next character everywhere except inside
        // single quotes, where nothing does: `paths::shell_quoted` writes an
        // apostrophe as `'\''`, and reading that middle quote as a real one
        // swallows the call Estigia itself wrote.
        if character == '\\' && quote != Some('\'') {
            word_start.get_or_insert(at);
            characters.next();
            continue;
        }
        match quote {
            Some(open) => {
                if character == open {
                    quote = None;
                }
            }
            None if character == '\'' || character == '"' => {
                quote = Some(character);
                if let Some(start) = word_start.take() {
                    words.push(&line[start..at]);
                }
            }
            None if character.is_whitespace() => {
                if let Some(start) = word_start.take() {
                    words.push(&line[start..at]);
                }
            }
            None => {
                word_start.get_or_insert(at);
            }
        }
    }
    if let Some(start) = word_start {
        words.push(&line[start..]);
    }
    words
}

/// Whether a line's command word does nothing but write text.
///
/// `echo estigia hook pre-push` carries the words outside any quotes and runs
/// the gate exactly as much as the quoted spelling does — not at all.
fn only_prints(line: &str) -> bool {
    let line = line.trim_start();
    let command = line.split_whitespace().next().unwrap_or_default();
    matches!(
        command.rsplit(['/', '\\']).next().unwrap_or(command),
        "echo" | "printf" | ":"
    )
}

/// The script Estigia writes: deliberately tiny, because everything it knows
/// is `estigia hook pre-push`, so a binary that moves needs no reinstall. It
/// passes git's stdin through, because `pre-push` receives the refs there and
/// a hook that swallows them cannot be chained to.
fn script(executable: &Path) -> String {
    let path = crate::paths::shell_quoted(executable);
    [
        "#!/bin/sh",
        MARKER,
        "#",
        "# Refuses a push that no live claim authorises. Removed by",
        "# `estigia guard --uninstall`; bypassed by `git push --no-verify`,",
        "# which is a guard rail working as one.",
        "#",
        "# The exit code is read rather than inherited. `exec` would hand git",
        "# whatever the shell produced, and a shell that cannot find the binary",
        "# produces 127 — which git blocks on. So deleting Estigia, or moving it,",
        "# or a `cargo clean`, used to leave a repository nobody could push from,",
        "# with a fix nobody would guess. A panic does the same at 101.",
        "#",
        "# Only the two codes Estigia defines are decisions: 1 is a refusal and 2",
        "# is an outcome it could not read back. Everything else means it did not",
        "# answer, and a hook that breaks does not deny — the rule this file's own",
        "# module states, applied at the one boundary where it was not.",
        "#",
        "# And it says so. Letting a push through unchecked is the stance here;",
        "# doing it without a word is not the same stance, which `git_hook`",
        "# states in those words for the working directory it could not read.",
        "# The binary that is not there is the case this script exists for, and",
        "# it was the silent one: `cargo clean` and every push after it went out",
        "# unmeasured, looking exactly like a push the gate had approved.",
        &format!("{path} hook {HOOK}"),
        "status=$?",
        "case \"$status\" in",
        "  0) exit 0 ;;",
        "  1|2) exit \"$status\" ;;",
        "  *)",
        "    echo \"estigia: the guard did not answer (exit $status), so this push went out \
         unchecked\" >&2",
        "    exit 0 ;;",
        "esac",
        "",
    ]
    .join(
        "
",
    )
}

/// Installs the push guard into one repository.
///
/// Refuses rather than overwrites when somebody else's hook is there. A
/// `pre-push` is somebody's test runner or somebody's secret scanner as often as
/// it is nothing, and replacing it would take away a check they are relying on
/// to add one they did not ask for.
pub fn install(repo_dir: &Path, executable: &Path, dry_run: bool) -> Result<State, Refusal> {
    let hooks = hooks_directory(repo_dir)?;
    let path = hooks.join(HOOK);

    match state(repo_dir) {
        State::Foreign => {
            return Err(Refusal::not_started(
                "push-hook-belongs-to-somebody-else",
                format!("{} exists and Estigia did not write it", path.display()),
                Resolution::no_command(
                    NoCommandReason::HumanAuthority,
                    format!(
                        "a decision about the existing hook: chain `{} hook {HOOK}` from it, or \
                         move it aside — replacing it would take away a check somebody relies on",
                        executable.display()
                    ),
                ),
            ));
        }
        // Estigia's own hook, with somebody's lines added to it: refused for
        // the same reason `Foreign` is — rewriting the file here would take
        // their lines away.
        State::Installed | State::Inert if edited(fs::read_to_string(&path).ok().as_deref()) => {
            return Err(Refusal::not_started(
                "push-hook-carries-your-lines",
                format!("{} has lines Estigia did not write", path.display()),
                Resolution::no_command(
                    NoCommandReason::HumanAuthority,
                    format!(
                        "a decision about your additions: move them into a hook of your own and chain `{} hook {HOOK}` from it, or take them out and run this again — rewriting the file here would take them away",
                        executable.display()
                    ),
                ),
            ));
        }
        State::Installed if dry_run => return Ok(State::Installed),
        // Somebody else's hook that hands the push to Estigia: the gate is
        // already running, and the file around it is theirs.
        State::Chained => return Ok(State::Chained),
        // Refused for the same reason `Foreign` is, with less to go on: a hook
        // that will not open cannot be shown to be Estigia's.
        State::Unreadable => {
            return Err(Refusal::not_started(
                "push-hook-unreadable",
                format!("{} is there and cannot be read", path.display()),
                Resolution::no_command(
                    NoCommandReason::HumanAuthority,
                    "a decision about the existing hook: make it readable, or move it aside \
                     \u{2014} replacing a file nothing can open would take away whatever it \
                     was doing, and nothing here can say whose it is",
                ),
            ));
        }
        _ => {}
    }

    if dry_run {
        return Ok(State::Absent);
    }

    fs::create_dir_all(&hooks).map_err(|error| unwritable(&hooks, &error))?;
    crate::paths::replace_atomically(&path, &script(executable))
        .map_err(|error| unwritable(&path, &error))?;
    make_executable(&path).map_err(|error| unwritable(&path, &error))?;
    Ok(State::Installed)
}

/// Removes the push guard, and only if Estigia wrote it.
pub fn uninstall(repo_dir: &Path, dry_run: bool) -> Result<Removal, Refusal> {
    let hooks = hooks_directory(repo_dir)?;
    let path = hooks.join(HOOK);
    // `Inert` as well: a hook carrying Estigia's marker is Estigia's whatever
    // its file mode says — the mode decides whether **git** runs it, not who
    // wrote it.
    let found = state(repo_dir);
    let taken = removable(found.clone(), fs::read_to_string(&path).ok().as_deref());
    if taken {
        if dry_run {
            return Ok(Removal::WouldBeTaken);
        }
        fs::remove_file(&path).map_err(|error| unwritable(&path, &error))?;
        return Ok(Removal::Taken);
    }
    // Not ours, so not ours to remove.
    if found == State::Absent {
        return Ok(Removal::NothingThere);
    }
    Ok(Removal::LeftAlone(found))
}

/// What an uninstall **did**, which is not the same as what is there
/// afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// Estigia's hook was here, and it is gone.
    Taken,
    /// Estigia's hook is here, and a run without `--dry-run` would take it.
    WouldBeTaken,
    /// There was nothing here to take.
    NothingThere,
    /// Something is here that is not Estigia's to remove, in the state it is in.
    LeftAlone(State),
}

fn unwritable(path: &Path, error: &std::io::Error) -> Refusal {
    Refusal::not_started(
        "push-hook-not-writable",
        format!("{}: {error}", path.display()),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "write permission on the repository's hooks directory",
        ),
    )
}

/// Gives the hook the bit git needs to run it at all; a hook without it is
/// skipped silently.
#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    // Git for Windows runs hooks through its bundled shell and does not
    // consult the file mode.
    Ok(())
}

/// Decides whether this push may proceed, and returns the one holder it was
/// decided on when there was one — the caller records that against the
/// ledger, so this hands back the whole [`Adjudication`] rather than only the
/// [`Decision`] half of it.
///
/// The run is found by asking which of this machine's pointers covers the
/// checkout being pushed from — a git hook has no session and cannot be told.
/// **nobody holds this checkout** — the push is refused where this guard is
/// installed and a claim can be made (see [`super::unclaimed::applies`]);
/// **one run holds it** — the claim is verified against the tracker, at the
/// boundary, with no window; **several do** — `refs` are the local refs git
/// names on the hook's stdin, read for the branch they select among holders,
/// never for what the push is; when they narrow the field to one, that run
/// takes the single-holder path.
pub fn decide(context: &GateContext, repo_dir: &Path, refs: &[String]) -> Adjudication {
    decide_pushing(context, repo_dir, refs, &[])
}

/// [`decide`], given the remote ref of each pushed line as well — the half an
/// assisted run is confined by, and which no other claim reads.
pub fn decide_pushing(
    context: &GateContext,
    repo_dir: &Path,
    refs: &[String],
    remotes: &[String],
) -> Adjudication {
    adjudicate_pushing(
        context,
        repo_dir,
        &Action::Boundary {
            command: "git push".to_owned(),
            pr: None,
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
        },
        Sensitivity::Boundary,
        (refs, remotes),
        None,
        &[],
    )
}

/// The runs whose oath covers this checkout.
///
/// The answer for every caller that has no identity to ask with: a git hook,
/// and a call carrying none at all — no `--run-id`, no `--session-id` with a
/// runtime, and no exported `ESTIGIA_RUN_ID`.
pub fn holders_of(state_root: &Path, repo_dir: &Path) -> Vec<session::Run> {
    // The closest cover, not every cover: work in run A's worktree inside the
    // repository is covered exactly by A, and from the base checkout by B.
    // Ambiguity that is real survives: two runs covering this directory at the
    // same depth are still two holders, and still the refusal.
    let held: Vec<(usize, session::Run)> = session::holdings(state_root)
        .into_iter()
        .filter_map(|run| {
            let depth = run
                .lands_in()
                .into_iter()
                .filter_map(|covered| crate::paths::coverage_depth(covered, repo_dir))
                .max();
            depth.map(|depth| (depth, run))
        })
        .collect();
    let Some(closest) = held.iter().map(|(depth, _)| *depth).max() else {
        return Vec::new();
    };
    held.into_iter()
        .filter(|(depth, _)| *depth == closest)
        .map(|(_, run)| run)
        .collect()
}

/// The holders relevant to this action, including a reviewed sibling only when
/// the action would spend delivery evidence.
pub fn holders_for_action(
    state_root: &Path,
    repo_dir: &Path,
    action: &Action,
) -> Vec<session::Run> {
    let covered = holders_of(state_root, repo_dir);
    if !covered.is_empty() || !super::is_delivery(action) {
        return covered;
    }
    let Some(pr) = action.pr() else {
        return Vec::new();
    };
    session::holdings(state_root)
        .into_iter()
        .filter(|run| {
            run.review_transaction
                .as_ref()
                .filter(|receipt| receipt.is_complete())
                .is_some_and(|receipt| receipt.pr == pr)
                && run
                    .covered()
                    .any(|covered| super::same_git_repository(covered, repo_dir))
        })
        .collect()
}

fn linked_siblings(state_root: &Path, repo_dir: &Path) -> Vec<session::Run> {
    session::holdings(state_root)
        .into_iter()
        .filter(|run| {
            run.covered()
                .any(|covered| super::same_git_repository(covered, repo_dir))
        })
        .collect()
}

/// One decision and the holder selected while making it.
#[derive(Debug)]
pub struct Adjudication {
    /// The gate's answer.
    pub decision: Decision,
    /// The one holder selected while producing that answer, when there was one.
    pub holder: Option<String>,
}

/// Decides one action, finding the run by the checkout it happens in.
pub fn decide_action(
    context: &GateContext,
    repo_dir: &Path,
    action: &Action,
    how: Sensitivity,
) -> Decision {
    adjudicate_action(context, repo_dir, action, how, &[]).decision
}

/// Decides one action and returns the holder selected by that same decision.
///
/// `refs` are the local refs a `git push` names on its stdin — empty for every
/// caller but the push guard, which is the only one with refs to give. They
/// select **who**, never **what**: read only to narrow several holders to the
/// one whose isolated checkout is the branch being pushed, before the tracker
/// reconciliation below ever runs.
pub fn adjudicate_action(
    context: &GateContext,
    repo_dir: &Path,
    action: &Action,
    how: Sensitivity,
    refs: &[String],
) -> Adjudication {
    adjudicate_command(context, repo_dir, action, how, refs, None, &[])
}

/// [`adjudicate_action`], given the shell command line the action was
/// classified from, when there is one. A run that holds nothing is refused a
/// commit or a push in a checkout where the push guard is installed, and that
/// is read from the command line rather than from the single fragment the
/// classifier reported. `same_tier_fragments` is the tie `classify_observing`
/// read off the line, carried here so the no-session doors refuse a
/// same-tier compound line exactly as the run-id door does.
pub fn adjudicate_command(
    context: &GateContext,
    repo_dir: &Path,
    action: &Action,
    how: Sensitivity,
    refs: &[String],
    literal: Option<&str>,
    same_tier_fragments: &[String],
) -> Adjudication {
    if let Some(denied) = super::person_only::deny(action) {
        return Adjudication {
            decision: denied,
            holder: None,
        };
    }
    adjudicate_pushing(
        context,
        repo_dir,
        action,
        how,
        (refs, &[]),
        literal,
        same_tier_fragments,
    )
}

fn adjudicate_pushing(
    context: &GateContext,
    repo_dir: &Path,
    action: &Action,
    how: Sensitivity,
    (refs, remotes): (&[String], &[String]),
    literal: Option<&str>,
    same_tier_fragments: &[String],
) -> Adjudication {
    // Before asking who holds the checkout, because the answer cannot matter:
    // an action the harness does not watch is outside whoever swore what. The
    // branch below **denies** when a run pointer will not parse, which is
    // right for a write and wrong for a tool this build does not watch.
    if matches!(action, Action::Untouched) {
        return Adjudication {
            decision: Decision::Outside(super::Aside::NotWatched),
            holder: None,
        };
    }
    if !same_tier_fragments.is_empty() {
        return Adjudication {
            decision: super::standdown::over(
                Decision::Deny(Box::new(super::several_irreversible_fragments(
                    same_tier_fragments,
                ))),
                context.stand_down.as_ref(),
                session::now_seconds(),
            ),
            holder: None,
        };
    }
    // Ordinary pushes are evaluated against the repository's schedule before
    // anybody is asked who holds this checkout: a schedule that waited for a
    // holder would hold nothing where nobody swore anything.
    if super::operation_window::action_is_protected(action)
        && !super::operation_window::permits_now(
            &context.operation_window,
            super::operation_window::now_minutes(),
        )
    {
        let denial = super::operation_window::denial(
            &context.operation_window,
            super::operation_window::now_minutes(),
        );
        return Adjudication {
            decision: super::standdown::over(
                Decision::Deny(Box::new(denial)),
                context.stand_down.as_ref(),
                session::now_seconds(),
            ),
            holder: None,
        };
    }
    if matches!(action, Action::Remote { .. }) {
        return Adjudication {
            decision: Decision::Outside(super::Aside::NotWatched),
            holder: None,
        };
    }
    if let Some(refusal) = super::issue_creation::refusal(literal) {
        return Adjudication {
            decision: super::standdown::over(
                Decision::Deny(Box::new(refusal)),
                context.stand_down.as_ref(),
                session::now_seconds(),
            ),
            holder: None,
        };
    }
    let ordinary_holders = holders_of(&context.state_root, repo_dir);
    let used_sibling = ordinary_holders.is_empty()
        && matches!(action, Action::Boundary { command, .. } if command == "gh pr merge");
    let linked = used_sibling.then(|| linked_siblings(&context.state_root, repo_dir));
    let mut holders = holders_for_action(&context.state_root, repo_dir, action);
    if used_sibling && !session::unreadable_holdings(&context.state_root).is_empty() {
        // A sibling is not path-owned by the readable pointer, so an unreadable
        // pointer could be its actual holder. Fall into the fail-closed arm.
        holders.clear();
    }

    // Ahead of the tracker: the ref git names is a fact about the machine. Not
    // free, and the cost is one divergence — narrowing to a single holder skips
    // the `ClosedIssue` reconciliation below, so a stale holder whose branch a
    // push names is decided on its own `gate` read rather than dropped.
    // Against that reconciliation alone the answer only ever tightens; against
    // the several-holder refusal it loosens, which is what this whole rule is
    // for. `narrowing_before_the_tracker_can_deny_what_reconciliation_would_have_allowed`
    // measures the first.
    if holders.len() >= 2 {
        holders = narrow_to_pushed_branch(repo_dir, holders, refs);
    }

    // Reconciled against the tracker — and only when there is more than one
    // name to reconcile between. Guarded to `len() >= 2` on purpose: folding a
    // single holder into this loop would not repeat `gate`'s refusal, it would
    // erase it, because the filter below only drops.
    //
    // Every surviving holder is classified once, through
    // `super::holder_standing`. A holder is dropped only when the tracker names
    // its issue `issue-not-open`; every other answer, including one this call
    // could not get at all, keeps the holder counted — an unknown result is not
    // clearance. Read-only, like the rest of this reconciliation.
    let mut standings: Vec<super::HolderStanding> = Vec::new();
    if holders.len() >= 2 {
        let judged: Vec<(session::Run, super::HolderStanding)> = holders
            .into_iter()
            .map(|run| {
                // `repo_dir`, the argument this function was actually called
                // with — not `context.repo_dir`; `adjudicate_action` is `pub`,
                // and a caller that let them diverge would reconcile one
                // checkout's holders against a different repository's timeline.
                let standing =
                    super::holder_standing(&context.skill_root, repo_dir, &context.tracker, &run);
                (run, standing)
            })
            .filter(|(_, standing)| !matches!(standing, super::HolderStanding::ClosedIssue))
            .collect();
        holders = judged.iter().map(|(run, _)| run.clone()).collect();
        standings = judged.into_iter().map(|(_, standing)| standing).collect();
    }

    // Through the stand-down, like every other decision; the two refusals below
    // are raised here rather than by `gate`, so wrapping once here covers them
    // without deciding anything twice.
    let mut selected = None;
    let decided = match holders.len() {
        // Nothing holds this checkout — unless the reason nothing does is that
        // a pointer would not open. `Outside` is a statement, and a corrupt
        // pointer is how a claimed checkout looked unclaimed.
        0 => match session::unreadable_holdings(&context.state_root).as_slice() {
            [] if linked.as_ref().is_some_and(|runs| !runs.is_empty()) => {
                Decision::Deny(Box::new(super::complete_review_transaction_not_selected()))
            }
            [] => match super::unclaimed::refusal(context, repo_dir, action, literal) {
                Some(refusal) => Decision::Deny(Box::new(refusal)),
                None => super::issue_requirement::decision(context, action, how, literal)
                    .unwrap_or(Decision::Outside(super::Aside::NothingSworn)),
            },
            unreadable => Decision::Deny(Box::new(Refusal::not_started(
                "run-pointers-unreadable",
                format!(
                    "{} run pointer(s) on this machine cannot be read, so whether a claim \
                     covers this checkout is unknown: {}",
                    unreadable.len(),
                    unreadable.join(", ")
                ),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "those files readable, or taken away if the runs that wrote them are \
                     over — until then no push from here can be told apart from a claimed one",
                ),
            ))),
        },
        1 => {
            let mut run = holders.into_iter().next().unwrap_or_else(|| {
                // Unreachable: the length was just checked. Written as a value
                // rather than an unwrap because a panic in a push hook is a
                // push that fails with a backtrace.
                session::Run::new(String::new())
            });
            if run.run_id.is_empty() {
                return Adjudication {
                    decision: Decision::Outside(super::Aside::NothingSworn),
                    holder: None,
                };
            }
            selected = Some(run.run_id.clone());
            let decision = super::gate_observing(
                context,
                &mut run,
                action,
                how,
                &crate::outcome::Observed {
                    push_refs: refs
                        .iter()
                        .enumerate()
                        .map(|(at, local)| {
                            (local.clone(), remotes.get(at).cloned().unwrap_or_default())
                        })
                        .collect(),
                    ..crate::outcome::Observed::default()
                },
            );
            // The answer the tracker just gave, written down — best effort,
            // like every other store: failing to record costs one extra read
            // and must never become a denial.
            if matches!(decision, Decision::Allow(_)) {
                let _ = session::store(&context.state_root, &run);
            }
            decision
        }
        // Every name still here answered the tracker with something other
        // than `issue-not-open`; an unknown result is not clearance to offer a
        // command over.
        _ => {
            // Which of the survivors the tracker itself said `estigia
            // release` may be named for: only the tracker's own answer is
            // authoritative.
            let releasable: Vec<&session::Run> = holders
                .iter()
                .zip(standings.iter())
                .filter(|(_, standing)| matches!(standing, super::HolderStanding::NotCurrentHolder))
                .map(|(run, _)| run)
                .collect();
            let resolution = if releasable.is_empty() || holders.iter().any(|run| run.assisted) {
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "which of these claims this work belongs to \u{2014} the tracker named none \
                     of them a leftover, so work from the checkout that run's own claim covers \
                     rather than guessing between them",
                )
            } else {
                Resolution::run(format!(
                    "estigia release --run-id <run-id>   # for: {}",
                    releasable
                        .iter()
                        .map(|run| run.run_id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            };
            // Named when there was something to try: refs git handed the hook,
            // read to select a holder by branch and not one of the several
            // survivors here — a refusal is more useful for saying what was
            // attempted than for staying silent about it.
            let attempted = if refs.is_empty() {
                String::new()
            } else {
                format!(
                    "; this push named {} on the local side, and that did not select one \
                     holder",
                    refs.join(", ")
                )
            };
            Decision::Deny(Box::new(Refusal::not_started(
                "several-runs-hold-this-checkout",
                format!(
                    "{} runs on this machine hold this checkout: {}{}",
                    holders.len(),
                    holders
                        .iter()
                        .zip(standings.iter())
                        .map(|(run, standing)| match standing {
                            super::HolderStanding::NotCurrentHolder => format!(
                                "{} (#{}, not the tracker's current holder)",
                                run.run_id,
                                run.issue
                                    .as_ref()
                                    .map(ToString::to_string)
                                    .unwrap_or_default()
                            ),
                            _ => format!(
                                "{} (#{})",
                                run.run_id,
                                run.issue
                                    .as_ref()
                                    .map(ToString::to_string)
                                    .unwrap_or_default()
                            ),
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    attempted
                ),
                resolution,
            )))
        }
    };
    Adjudication {
        decision: super::standdown::over(
            decided,
            context.stand_down.as_ref(),
            session::now_seconds(),
        ),
        holder: selected,
    }
}

/// Narrows several holders to the one whose isolated checkout is the branch
/// `refs` names — never widens, and never guesses between two candidates
/// either. `refs` are local refs, in git's own words on the hook's stdin:
/// `refs/heads/<branch>` for an ordinary push, and the literal `(delete)`
/// for a branch deletion, which names no branch and therefore selects
/// nobody.
///
/// A union over whatever happens to resolve is not this rule: every ref must
/// resolve, to one worktree, owned by one holder, and every ref must agree on
/// that same holder. The full enumeration of what fails this closed —
/// `docs/honesty.md`'s "Nothing adjudicates what a push is aimed at" entry —
/// lives in one place, and this doc comment does not repeat it.
fn narrow_to_pushed_branch(
    repo_dir: &Path,
    holders: Vec<session::Run>,
    refs: &[String],
) -> Vec<session::Run> {
    if refs.is_empty() {
        return holders;
    }
    // Every local ref has to be an ordinary branch ref. A tag, `HEAD`, a raw
    // sha, or the literal `(delete)` cannot be attributed to a worktree, and
    // one such ref anywhere in the push fails the whole narrowing closed
    // rather than being dropped and letting the rest decide.
    let mut branches: Vec<&str> = Vec::with_capacity(refs.len());
    for reference in refs {
        let Some(branch) = reference.strip_prefix("refs/heads/") else {
            return holders;
        };
        branches.push(branch);
    }
    let Some(registry) = worktree_registry(repo_dir) else {
        return holders;
    };
    let mut target: Option<session::Run> = None;
    for branch in branches {
        let mut carrying = registry
            .values()
            .filter(|entry| entry.branch.as_deref() == Some(branch));
        let Some(worktree) = carrying.next() else {
            return holders; // no registered worktree carries this branch
        };
        if carrying.next().is_some() {
            return holders; // two or more worktrees carry this branch
        }
        let mut owning = holders.iter().filter(|run| {
            run.worktree
                .as_deref()
                .is_some_and(|owned| crate::paths::same_directory(owned, &worktree.path))
        });
        let Some(owner) = owning.next() else {
            return holders; // no holder owns this worktree
        };
        if owning.next().is_some() {
            return holders; // two or more holders record this same worktree
        }
        match &target {
            None => target = Some(owner.clone()),
            Some(already) if already.run_id == owner.run_id => {}
            Some(_) => return holders, // two refs landed on different holders
        }
    }
    // `branches` is non-empty whenever this line is reached — every ref
    // resolved or the loop above already returned — so `target` is always
    // `Some` here; the fallback exists only so this stays a function that
    // cannot panic if that invariant is ever wrong.
    target.map_or(holders, |run| vec![run])
}

/// The checkouts git has registered against `repo_dir`, or `None` when the
/// listing could not be read or parsed — the caller's signal to fail closed,
/// per `narrow_to_pushed_branch`'s own doc.
///
/// Through `super::proof_git_command`, the crate's one convention for a git
/// read that feeds a gate decision: a `pre-push` hook runs with `GIT_DIR`
/// already set by git itself, and an inherited `GIT_*` variable could point
/// this listing at a different repository's worktrees than the one being
/// pushed.
fn worktree_registry(
    repo_dir: &Path,
) -> Option<std::collections::BTreeMap<String, crate::transport::branch::RegisteredWorktree>> {
    let answer = super::proof_git_command(repo_dir, &["worktree", "list", "--porcelain", "-z"])
        .output()
        .ok()?;
    if !answer.status.success() {
        return None;
    }
    let listing = String::from_utf8_lossy(&answer.stdout);
    crate::transport::branch::parse_worktree_listing(&listing).ok()
}

#[cfg(test)]
mod tests;
