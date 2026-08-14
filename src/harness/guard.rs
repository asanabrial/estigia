//! The gate no agent can route around.
//!
//! An agent hook fires only for the agent that installed it, and only one of the
//! seven has an event that can deny. A `pre-push` hook sits **under git**: it
//! refuses a push typed by any agent, by a person, or by a script, at the
//! boundary that cannot be taken back.
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

/// The marker that says a hook file is Estigia's.
///
/// A hook is a script, not a structured file, so there is nowhere to put a
/// fenced block that survives somebody else's edits. What there is instead is a
/// line that says who wrote it — enough for [`uninstall`] to know whether it is
/// looking at its own work or at somebody else's.
pub const MARKER: &str = "# managed by estigia guard";

/// What one repository's guard looks like.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Estigia's hook is installed.
    Installed,
    /// A hook is there, somebody else wrote it, and it calls Estigia's.
    ///
    /// The arrangement Estigia itself asks for when it finds a hook it did not
    /// write: *chain `estigia hook pre-push` from it*. An operator who does
    /// that has the guard in force, and it used to be reported as `Foreign`
    /// — told, on every run of `doctor`, to do the thing they had already
    /// done, under a check that said the push boundary was ungated when it was
    /// not.
    Chained,
    /// A hook is there, somebody else wrote it, and it does not call Estigia's.
    Foreign,
    /// The gate is in the file, and git will not run it.
    ///
    /// Present, ours, and inert. On a system where git consults the file mode,
    /// a hook without the execute bit is skipped **silently** — no warning, no
    /// exit code — so `estigia guard` reported the guard installed, `doctor`
    /// reported it in force, and every push went through ungated. Installed,
    /// looks installed, decides nothing: the one state this crate exists to
    /// refuse, and it had no name here.
    Inert,
    /// Nothing is installed.
    Absent,
    /// A hook file is there and reading it failed.
    ///
    /// Not `Absent`, and the difference is a file somebody else wrote. `state`
    /// read the hook with `.ok()`, so a `pre-push` that exists and will not open
    /// arrived as *nothing is installed* — and `install` writes over nothing.
    ///
    /// That is the harm the `Chained` arm above already records, reached by a
    /// different door: *"this fell into the `_` arm and was overwritten … the
    /// `npx lint-staged` line it told you to keep is gone."* The whole point of
    /// telling `Foreign` from `Absent` is not replacing what somebody relies on,
    /// and a file nothing can read is the one case where **whose it is** cannot
    /// be established at all.
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
/// **Pure and fed**, because the case worth having cannot be produced on every
/// platform that has to be able to read this: the answer is decided here, and
/// the filesystem is asked one question outside.
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
    // Somebody else's hook that calls this one is `Chained`. Looked for the
    // same way the agents' settings files are read: the invocation is what says
    // the gate runs, whatever wrote the file around it.
    if ours(text) {
        State::Installed
    } else {
        State::Chained
    }
}

/// Whether an uninstall may take this hook away.
///
/// **Pure and fed**, for the reason `state_of` gives two functions up: the case
/// worth having is a file mode, and not every platform that has to read this
/// code can produce one. The decision is made here and the filesystem is asked
/// its two questions outside.
///
/// `Inert` is the case this was written for. It means *git will not run this*,
/// which is a fact about the mode and not about who wrote the file — so a hook
/// of Estigia's that lost its execute bit was reported by `doctor` as the gate
/// not running, told the operator to run `estigia guard`, and could not be
/// taken off the machine by `estigia guard --uninstall`, which read the same
/// file as somebody else's.
///
/// Read for the marker rather than for the state, because a *chained* hook of
/// theirs without the bit is `Inert` too, and that one stays.
fn removable(state: State, text: Option<&str>) -> bool {
    let unchanged = |text: Option<&str>| text.is_some_and(ours) && !edited(text);
    match state {
        State::Installed | State::Inert => unchanged(text),
        State::Absent | State::Foreign | State::Chained | State::Unreadable => false,
    }
}

/// Whether Estigia's hook carries somebody's additions.
///
/// The marker says Estigia wrote the file. It does not say the file is still
/// what Estigia wrote — and `guard --uninstall` is documented, in the README's
/// own words, as *removes only a hook Estigia wrote*. Measured before this
/// existed: an operator who appended two lines to the hook and then uninstalled
/// lost them, reported as `removed`.
///
/// That case is not exotic. It is the arrangement Estigia **asks for**: finding
/// a hook it did not write, it says *chain `estigia hook pre-push` from it*. An
/// operator who does the reverse — takes Estigia's hook and adds their own check
/// to it — has done the same thing from the other end.
///
/// Compared against every line this binary writes, not against a hash: the
/// script names the executable's path, so two machines produce two different
/// files and a digest of one means nothing on the other. What is asked is
/// whether anything is here that this binary would not have written.
fn edited(text: Option<&str>) -> bool {
    let Some(text) = text else {
        return false;
    };
    // Rendered for the path this file itself names, so the comparison is about
    // *content* and not about which binary wrote it: a hook installed by an
    // Estigia that has since moved is still unedited.
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

/// Whether Estigia wrote this hook, which is what makes it Estigia's to remove.
///
/// One function because two questions ask it: what state the file is in, and
/// whether an uninstall may take it away. They were asked in different words —
/// the second asked *is the state `Installed`*, which is not the same question,
/// because a hook of ours that lost its execute bit is `Inert`.
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
/// By the two words that have to be next to each other, not by the path: the
/// binary may be `estigia`, an absolute path, or a shim, and any of them is the
/// gate running. `hook pre-push` is what only Estigia answers to.
///
/// A **mention** is not a call, and this counted one. Measured on three hooks in
/// one repository, with `doctor` asked after each:
///
/// ```text
/// estigia hook pre-push || exit $?              ok       in force
/// echo "to enable: estigia hook pre-push" >&2   ok       in force   <- nothing calls the gate
/// ./check.sh                                    skipped  a hook is here
/// ```
///
/// The middle row is this crate's cardinal sin: the push boundary reported as
/// gated while nothing runs the gate, on the line an operator reads to decide
/// their machine is in order.
///
/// Two shapes are refused, and both are ones that certainly do not run: the
/// words inside a quoted string, and a line whose command word only prints. What
/// this still cannot know is whether a line that *is* a call ever executes — a
/// call inside a branch nobody takes counts here, and saying so is better than
/// pretending to parse `sh`.
///
/// Narrowing is the safe direction and it is not free. `Chained` exists because
/// operators who had already chained were told to do it again at every `doctor`;
/// missing a real call brings that back. Being told to do something twice is a
/// worse experience than a false `ok` and a smaller failure — one wastes a
/// minute, the other reports a gate that decides nothing as deciding.
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
        // single quotes, where nothing does. That exception is the whole
        // reason this is not a parity count: `paths::shell_quoted` writes an
        // apostrophe as `'\''` — close, escaped quote, reopen — and reading
        // that middle quote as a real one leaves the rest of the line inside a
        // string that never ends, swallowing the call Estigia itself wrote.
        if character == '\\' && quote != Some('\'') {
            word_start.get_or_insert(at);
            characters.next();
            continue;
        }
        match quote {
            // Inside a quoted run: only its own closing quote ends it, and
            // whatever it held was text rather than a command word.
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

/// The script Estigia writes.
///
/// Deliberately tiny: everything it knows is `estigia hook pre-push`, so a
/// binary that moves or a decision that changes needs no reinstall. It passes
/// git's stdin through, because `pre-push` receives the refs there and a hook
/// that swallows them cannot be chained to.
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
        // Estigia's own hook, with somebody's lines added to it. Refused for the
        // same reason `Foreign` is, and it is the mirror of that case: there the
        // operator's hook came first and Estigia was asked to chain from it;
        // here Estigia's came first and the operator added their check to it.
        // The work is the same work.
        //
        // It fell into the `_` arm and was **overwritten**, and the report said
        // `already current` — a claim about a file this run had just rewritten.
        // `uninstall` learned to leave an edited hook alone one round before
        // this; the install kept replacing it, which is the half that loses the
        // lines rather than merely keeping them.
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
        // Somebody else's hook that hands the push to Estigia. The gate is
        // already running, so there is nothing to install — and the file around
        // it is theirs.
        //
        // This fell into the `_` arm and was **overwritten**, which is worse
        // than it sounds: the refusal three lines above tells an operator to
        // chain Estigia from their existing hook, and that is exactly the file
        // this then replaced. Follow the advice, run `estigia guard` again, and
        // the `npx lint-staged` line it told you to keep is gone.
        //
        // `state` draws the distinction, `uninstall` honours it — "not ours, so
        // not ours to remove" — and `doctor` reports it. This was the one place
        // it was built and then thrown away.
        State::Chained => return Ok(State::Chained),
        // Refused for the same reason `Foreign` is, and with less to go on: a
        // hook that will not open cannot be shown to be Estigia's, so replacing
        // it is replacing a file whose owner is unknown. The `_` arm below
        // writes, and `Absent` used to arrive here from a failed read.
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
    // `Inert` as well, and it is the case that was being left behind. A hook
    // carrying Estigia's marker is Estigia's whatever its file mode says: the
    // mode decides whether **git** runs it, not who wrote it. Every other
    // surface already reads it that way — `doctor` says *the gate is in the
    // hook and git will not run it*, `guard --status` says *push guard present
    // and NOT running* — and only the uninstall treated it as somebody else's,
    // so an operator whose hook lost its execute bit could not take Estigia's
    // own file off their machine with Estigia. `rsync` without `-p`, a
    // restrictive umask, an archive restored without modes: the bit is not hard
    // to lose, and losing it made the removal permanent in the wrong direction.
    //
    // The comment here used to say this was reachable only if the file stopped
    // being runnable between the read and the removal. It is reachable straight
    // from the file's own state, which `state_of` answers in one line.
    //
    // Read for the marker rather than for the state, because `Inert` is also
    // what a *chained* hook of somebody else's looks like without the bit, and
    // that one stays.
    let found = state(repo_dir);
    let taken = removable(found.clone(), fs::read_to_string(&path).ok().as_deref());
    if taken {
        if dry_run {
            return Ok(Removal::WouldBeTaken);
        }
        fs::remove_file(&path).map_err(|error| unwritable(&path, &error))?;
        return Ok(Removal::Taken);
    }
    // Not ours, so not ours to remove. Reported rather than refused: an
    // uninstall that stops on somebody else's file has not failed.
    if found == State::Absent {
        return Ok(Removal::NothingThere);
    }
    Ok(Removal::LeftAlone(found))
}

/// What an uninstall **did**, which is not the same as what is there afterwards.
///
/// This answered with [`State`], and `Absent` is what it answered for two
/// different things: Estigia's hook was here and is gone, and there was never
/// one here. The renderer printed `<path> removed` for both — and for a third,
/// because `--dry-run` took the same arm and had removed nothing at all.
///
/// One word for three states, in the command whose job is taking a file off
/// somebody's machine. The other arms of that match exist for exactly this
/// reason — *nothing was taken out, and saying so beats reporting a removal
/// that did not happen* — and the main one collapsed it.
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

/// Gives the hook the bit git needs to run it at all.
///
/// The result used to be dropped. On a system where git consults the mode, a
/// hook without it is not a hook: git skips it, silently and with no warning, so
/// `estigia guard` reported the guard installed, `doctor` reported it in force,
/// and every push went through ungated. That is the shape this whole crate
/// exists to refuse — installed, looks installed, decides nothing — and the one
/// place it could arise here was a discarded `Result`.
#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    // Git for Windows runs hooks through its bundled shell and does not consult
    // the file mode, so there is nothing to set. Saying so beats an empty
    // function somebody later reads as an oversight.
    Ok(())
}

/// Decides whether this push may proceed.
///
/// The run is found by asking which of this machine's pointers covers the
/// checkout being pushed from — a git hook has no session and cannot be told.
///
/// Three answers, and the middle one is the interesting one:
///
/// - **nobody holds this checkout** — the push is nothing to do with Estigia,
///   and refusing it would make the guard a lock on the operator's own work;
/// - **one run holds it** — the claim is verified against the tracker, at the
///   boundary, with no window;
/// - **several do** — refused, because picking one would be guessing which
///   claim a push belongs to, and a wrong guess authorises the wrong delivery.
pub fn decide(context: &GateContext, repo_dir: &Path) -> Decision {
    decide_action(
        context,
        repo_dir,
        &Action::Boundary {
            command: "git push".to_owned(),
        },
        Sensitivity::Boundary,
    )
}

/// The runs whose oath covers this checkout.
///
/// The answer for every caller that has no session to ask with: a git hook, and
/// an agent whose plugin API hands out a project directory but no session id.
pub fn holders_of(state_root: &Path, repo_dir: &Path) -> Vec<session::Run> {
    // The closest cover, not every cover. Two runs of one repository each name
    // the base checkout in their pointer and each get an isolated worktree, and
    // an operator may put those worktrees inside the repository. Work in run A's
    // worktree is then covered exactly by A and from the base checkout by B —
    // two holders, and the arm below denies. Measured: a push from `wt-a` came
    // back *"2 runs on this machine hold this checkout"*, refusing the one
    // directory isolation exists to give each run.
    //
    // Ambiguity that is real survives: two runs covering this directory at the
    // same depth are still two holders, and still the refusal.
    let held: Vec<(usize, session::Run)> = session::holdings(state_root)
        .into_iter()
        .filter_map(|run| {
            run.covered()
                .filter_map(|covered| crate::paths::coverage_depth(covered, repo_dir))
                .max()
                .map(|depth| (depth, run))
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

/// Decides one action, finding the run by the checkout it happens in.
pub fn decide_action(
    context: &GateContext,
    repo_dir: &Path,
    action: &Action,
    how: Sensitivity,
) -> Decision {
    // Before asking who holds the checkout, because the answer cannot matter:
    // an action the harness does not watch is outside whoever swore what.
    //
    // Two things went wrong without it. The message — this path answered
    // `NothingSworn` for a `Read`, so `estigia gate Read` said "Read is
    // watched, and this run holds no issue" about a tool that is not watched at
    // all. And the decision: the branch below **denies** when a run pointer
    // somewhere on this machine will not parse, which is right for a write and
    // is a denied `Read` for OpenCode, whose plugin sends every tool call
    // through here. This file's neighbour already states the rule — *a schema
    // this build does not know could be wrapping `Read` as easily as `Write`,
    // and denying it would deny reads*.
    if matches!(action, Action::Untouched) {
        return Decision::Outside(super::Aside::NotWatched);
    }
    let holders = holders_of(&context.state_root, repo_dir);

    // Through the stand-down, like every other decision. `gate` states the rule
    // where it wraps its own: *a stand-down honoured on some paths and not
    // others is worse than none — an operator would learn it works and be
    // surprised by the door that ignored it.* This was that door.
    //
    // The two refusals below are raised here rather than by `gate`, so nothing
    // wrapped them: a machine carrying one corrupt run pointer refused every
    // push from every checkout, and `estigia stand-down` — the one command whose
    // whole purpose is getting past a gate that is wrong at a bad moment — did
    // not reach them. The `1 =>` arm goes through `gate`, which already wraps,
    // and `over` passes an allowance through untouched, so wrapping once here
    // covers the two that were missing without deciding anything twice.
    let decided = match holders.len() {
        // Nothing holds this checkout — unless the reason nothing does is that
        // a pointer would not open. `Outside` is a statement, and it is the one
        // statement this cannot make with a file it could not read: a corrupt
        // pointer is how a claimed checkout looked unclaimed, at the boundary
        // this file's own header calls "the more expensive end".
        //
        // Only when *nothing* readable was found. A readable holder still gates
        // the push on its own terms, so the unknown one changes no outcome
        // there — here it changes the only one there is.
        0 => match session::unreadable_holdings(&context.state_root).as_slice() {
            [] => Decision::Outside(super::Aside::NothingSworn),
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
                return Decision::Outside(super::Aside::NothingSworn);
            }
            let decision = super::gate(context, &mut run, action, how);
            // The answer the tracker just gave, written down.
            //
            // `gate` marks the run verified and this function used to drop it:
            // `run` is a local, and nothing stored it. For a push that costs
            // nothing — one call, and a boundary never rides the window anyway.
            // For **OpenCode** it is every edit: its plugin is gated through
            // `estigia gate`, which lands here, so `within_window` was always
            // false and every routine write paid a full tracker round trip that
            // the previous one had already paid for.
            //
            // Best effort, like every other store: failing to record when we
            // last asked costs one extra read and must never become a denial.
            if matches!(decision, Decision::Allow(_)) {
                let _ = session::store(&context.state_root, &run);
            }
            decision
        }
        _ => Decision::Deny(Box::new(Refusal::not_started(
            "several-runs-hold-this-checkout",
            format!(
                "{} runs on this machine hold this checkout: {}",
                holders.len(),
                holders
                    .iter()
                    .map(|run| format!("{} (#{})", run.run_id, run.issue.unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "which claim this work belongs to — release the runs that do not, then retry",
            ),
        ))),
    };
    super::standdown::over(decided, context.stand_down.as_ref(), session::now_seconds())
}

#[cfg(test)]
mod tests;
