//! What has to be true before a run can swear to anything, checked.
//!
//! Read-only, and that is the whole design. The harness sits on the critical
//! path of every edit, so the moment somebody hits a refusal they need to know
//! whether the problem is the tracker, the interpreter, the credentials or the
//! install — and finding that out must not be able to change any of them.
//!
//! Every check that fails names a [`Resolution`], which is the ratchet applied
//! to an environment rather than to a command line: a health report that says
//! "python: missing" and stops is the dead end the ratchet forbids.
//!
//! # Why this exists at all
//!
//! It is the one idea worth taking from the advisory harnesses this crate was
//! measured against. Their enforcement is advisory, so nothing there transfers
//! to the gate — but a `doctor` command, a read-only check of the binaries and
//! state the tool depends on, is exactly right for a harness whose refusals
//! otherwise arrive one edit at a time with no way to tell which of five
//! things is wrong.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::outcome::{NoCommandReason, Resolution};

/// How one check came out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Health {
    /// Working, and this is what it reported.
    Fine {
        /// What was found, for the line that reports it.
        detail: String,
    },
    /// Not working, and this is what to do.
    Broken {
        /// What is wrong.
        detail: String,
        /// The way out.
        ///
        /// Kept typed rather than rendered: a caller that turns a broken check
        /// into a refusal has to carry *this* resolution through, and a string
        /// forces it to invent a generic one instead. That is the same loss the
        /// taxonomy exists to prevent — `[operator-knowledge] which repository
        /// holds the issues` became `[world-action] remote: ...` the first time
        /// this was a `String`.
        resolution: Resolution,
    },
    /// Not applicable here, which is not a failure.
    ///
    /// A repository with no tracker configured is not a broken machine, and
    /// reporting it as one teaches people to ignore the report.
    Skipped {
        /// Why the check did not apply.
        detail: String,
    },
}

impl Health {
    /// Whether this check found something that has to be fixed.
    pub fn is_broken(&self) -> bool {
        matches!(self, Self::Broken { .. })
    }

    fn broken(detail: impl Into<String>, resolution: Resolution) -> Self {
        Self::Broken {
            detail: detail.into(),
            resolution,
        }
    }

    /// The way out, when there is one to carry.
    pub fn resolution(&self) -> Option<&Resolution> {
        match self {
            Self::Broken { resolution, .. } => Some(resolution),
            _ => None,
        }
    }
}

/// One thing that has to be true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// A short stable name, for a script matching on it.
    pub name: &'static str,
    /// What this check is for, for a person reading the report.
    pub about: &'static str,
    /// How it came out.
    pub health: Health,
}

/// The checks that report on runs that **already happened**.
///
/// One, and [`silence`]'s own note says why it is the only one: *everything
/// else here asks whether a run could work; this asks whether the runs that
/// already happened were gated at all*.
///
/// The distinction is not decorative. `estigia doctor` turns the first broken
/// check into `environment-not-ready` — *"silence is not usable, so a run
/// cannot swear yet"* — and for this one that sentence is false: five ungated
/// calls from three days ago stop nothing, and the ledger keeps them, so the
/// command exited non-zero forever on a machine where a run could swear
/// perfectly well. A readiness probe with a permanent false negative is a
/// readiness probe somebody switches off.
///
/// Declared here rather than asked of each check, beside [`CHECKS`], which is
/// already the one place the names live.
const LOOKS_BACK: &[&str] = &["silence"];

impl Check {
    /// Whether this check answers *could a run work now*.
    pub fn about_the_present(&self) -> bool {
        !LOOKS_BACK.contains(&self.name)
    }
}

/// The broken check a run is actually stopped by, and what to do about it.
///
/// Pure and fed, because the selection is the decision and everything around it
/// in `cli::doctor` needs a machine with an installation on it.
pub fn first_blocking(checks: &[Check]) -> Option<&Check> {
    checks
        .iter()
        .filter(|check| check.about_the_present())
        .find(|check| matches!(check.health, Health::Broken { .. }))
}

/// Runs every check against one installation.
///
/// Ordered so the first failure is the one worth fixing first: an absent skill
/// makes every later check meaningless, and an absent interpreter makes the
/// tracker checks meaningless in turn.
pub fn examine(
    skill_root: Option<&Path>,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
) -> Vec<Check> {
    let mut checks = Vec::new();

    let Some(skill_root) = skill_root else {
        checks.push(Check {
            name: "skill",
            about: "the contract and the transport the agent reads",
            health: Health::broken(
                "no installed skill was found",
                Resolution::run("estigia setup --all"),
            ),
        });
        return checks;
    };
    checks.push(Check {
        name: "skill",
        about: "the contract and the transport the agent reads",
        // What is installed, not merely that something is. This row named the
        // directory and checked nothing in it, and the `transport` row below
        // asked only whether `github.py` was a file — so a payload left behind
        // by an older Estigia reported `ok` on both while the binary and the
        // script disagreed about the flags between them. That disagreement is
        // what `every_tool_sends_flags_the_transport_accepts` exists to prevent,
        // crossed against the **shipped** transport; nothing crossed the
        // installed one.
        //
        // The comparison already existed and is already trusted: `presence_of`
        // renders every embedded file against what is on disk, and the session
        // start uses it to tell the *agent* the contract is not this binary's
        // copy. The operator asking whether their machine is right was the one
        // nobody told.
        //
        // `Unreadable` is left to the `contract` row, which names which value
        // it could not read. Saying it twice in different words would send an
        // operator looking for two faults.
        health: match crate::skill::presence_of(skill_root) {
            crate::skill::Presence::Stale => Health::broken(
                format!(
                    "the skill in {} is not this binary's copy \u{2014} the contract the agent \
                     reads and the transport it runs may not be the ones this build was tested \
                     against",
                    skill_root.display()
                ),
                Resolution::run("estigia sync"),
            ),
            _ => Health::Fine {
                detail: skill_root.display().to_string(),
            },
        },
    });

    // A tracker with no executable is not a broken machine — it is a binding the
    // agent reads and works by hand. Saying so is the difference between a
    // harness that is inert and one that is broken, and only one of those needs
    // fixing.
    let Some(relative) = tracker.transport() else {
        checks.push(Check {
            name: "transport",
            about: "the executable every reversible tracker operation runs through",
            health: Health::Skipped {
                detail: format!(
                    "`{}` ships a binding and no executable — the harness holds no tools for it, and its operations are run by hand from the binding",
                    tracker.as_value()
                ),
            },
        });
        return checks;
    };

    // Answered by this binary, so what there was to check is gone. The row named
    // a **file** — `scripts/github.py` — and asked whether it existed; the file
    // is retired, and asking after it now would report a machine broken for the
    // absence of something nothing runs. The `python` row beside it went the
    // same way: there is no interpreter on the path any more.
    //
    // The drift those rows used to catch is the `skill` row's, which compares
    // every installed file against this binary's copy. This one is kept all the
    // same, because an operator reading the list asks *can my tracker be
    // operated*, and a list that answers by saying nothing is a list they have
    // to interpret.
    checks.push(Check {
        name: "transport",
        about: "what every reversible tracker operation runs through",
        health: Health::Fine {
            detail: format!(
                "{relative}: answered in process — no interpreter and no script on the path"
            ),
        },
    });

    checks.push(Check {
        name: "gh",
        about: "the GitHub CLI the transport reaches the tracker through",
        health: program(
            "gh",
            &["auth", "status"],
            |output| {
                // `gh auth status` writes to stderr on both paths, and its exit
                // code is the part that answers the question.
                if output.trim().is_empty() {
                    "authenticated".to_owned()
                } else {
                    output
                        .lines()
                        .next()
                        .unwrap_or("authenticated")
                        .trim()
                        .to_owned()
                }
            },
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                crate::harness::tracker::UNAUTHENTICATED,
            ),
        ),
    });

    // Reported, never installed here. `doctor` is read-only, and a check that
    // quietly wrote a hook into whatever repository somebody was standing in
    // would be the opposite of what a health report is for.
    checks.push(Check {
        name: "push-guard",
        about: "the pre-push hook that refuses a push no live claim authorises",
        health: match super::guard::state(repo_dir) {
            super::guard::State::Installed => Health::Fine {
                // Narrower than "every push is checked", because it is: a
                // checkout no run holds is outside the gate, and this hook lets
                // that push through. A health report that overstates its own
                // coverage is the one kind of report worth less than none.
                detail: "installed — a push from a checkout a live claim holds is refused \
                         unless that claim justifies it; an unclaimed checkout is outside \
                         the gate"
                    .to_owned(),
            },
            // Absent is not broken. A repository where nobody works under
            // Estigia has no reason to carry it, and reporting every checkout
            // on the machine as broken teaches people to ignore the report.
            super::guard::State::Absent => Health::Skipped {
                detail: "not installed — `estigia guard` adds it, and only the push boundary goes ungated without it"
                    .to_owned(),
            },
            // Broken rather than skipped, unlike `Absent`: a repository that
            // carries no hook is a choice, and a hook nothing can read is a
            // question nobody answered. `estigia guard` refuses to write over
            // it, so it stays until somebody looks.
            super::guard::State::Unreadable => Health::broken(
                "a pre-push hook is here and cannot be read, so whether the push boundary is \
                 gated is unknown"
                    .to_owned(),
                crate::outcome::Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "that file readable, or moved aside \u{2014} `estigia guard` will not \
                     replace a hook it cannot identify, because replacing one somebody relies \
                     on is the failure it exists to avoid",
                ),
            ),
            // In force, from somebody else's script. Reported `Skipped` with
            // the advice to chain it, an operator who had chained it was told
            // to do what they had already done, under a check that called the
            // push boundary ungated while the gate ran on every push.
            super::guard::State::Chained => Health::Fine {
                detail: "in force from a hook Estigia did not write \u{2014} a push from a checkout \
                         a live claim holds is refused unless that claim justifies it; an \
                         unclaimed checkout is outside the gate"
                    .to_owned(),
            },
            super::guard::State::Foreign => Health::Skipped {
                detail: "a pre-push hook is here and Estigia did not write it; chaining \
                         `estigia hook pre-push` from it is the way in"
                    .to_owned(),
            },
            // The only one of the four that reads as working and is not.
            // Reinstalling is the way out because the install sets the bit, so
            // the resolution is a command rather than knowledge.
            super::guard::State::Inert => Health::broken(
                "the gate is in the hook and git will not run it \u{2014} the file has no execute \
                 bit, so git skips it silently and every push goes through ungated",
                Resolution::run("estigia guard"),
            ),
        },
    });

    checks.push(Check {
        name: "remote",
        about: "the git remote the tracker repository is discovered from",
        health: remote(repo_dir),
    });

    checks
}

/// Runs a program and judges the result.
fn program(
    name: &str,
    args: &[&str],
    describe: impl Fn(&str) -> String,
    missing: Resolution,
) -> Health {
    match Command::new(name).args(args).output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let text = if text.trim().is_empty() {
                String::from_utf8_lossy(&output.stderr).into_owned()
            } else {
                text.into_owned()
            };
            Health::Fine {
                detail: describe(&text),
            }
        }
        Ok(output) => Health::broken(
            format!(
                "`{name} {}` exited {}",
                args.join(" "),
                output.status.code().unwrap_or(-1)
            ),
            missing,
        ),
        Err(_) => Health::broken(format!("`{name}` is not on PATH"), missing),
    }
}

/// The URL out of one line of `git remote -v`.
///
/// The line is `<name>\t<url> (fetch)`, and it was read with
/// `split_whitespace().nth(1)` — which is the name and the URL only while the
/// URL has no space in it. A checkout whose remote was
/// `../el remoto con espacios.git` had its row read `remote  ../el`: a doctor
/// answering *which repository holds the issues* with a path that is not one,
/// and no sign that anything had been cut.
///
/// Split on the tab git puts there, and take the ` (fetch)` off the end rather
/// than the first space. A remote whose line carries no tab is not one git
/// wrote; the whole line is kept then, because a line this cannot read is
/// still worth showing whole to whoever can.
fn remote_url(line: &str) -> &str {
    let Some((_, rest)) = line.split_once('\t') else {
        return line;
    };
    // Only the kind git appends, and only at the end: a URL ending in
    // `(fetch)` is not one, and one containing it elsewhere keeps it.
    for kind in [" (fetch)", " (push)"] {
        if let Some(url) = rest.strip_suffix(kind) {
            return url;
        }
    }
    rest
}

/// Whether this checkout has a remote for the tracker to be discovered from.
fn remote(repo_dir: &Path) -> Health {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["remote", "-v"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            match text.lines().next() {
                Some(first) => Health::Fine {
                    detail: remote_url(first).to_owned(),
                },
                None => Health::broken(
                    format!("{} has no git remote", repo_dir.display()),
                    Resolution::no_command(
                        NoCommandReason::OperatorKnowledge,
                        "which repository holds the issues — add a remote, or set `Tracker` to \
                         `github <owner>/<name>`",
                    ),
                ),
            }
        }
        // Not a repository is not a broken machine. A person running `doctor`
        // from their home directory should not be told their git is wrong.
        _ => Health::Skipped {
            detail: format!("{} is not a git repository", repo_dir.display()),
        },
    }
}

/// Every verdict that means a call reached the gate and was not decided on.
///
/// Held in one place because it is read in two: the walk that collects them and
/// the sentence that says which repair each needs. They were two lists, one of
/// them written as *everything that is not the first*, and a third way to go
/// undecided appeared in the ledger, was skipped by the walk, and left `doctor`
/// answering **every call the ledger records was decided on** about the three
/// records saying otherwise.
const UNDECIDED: &[&str] = &[
    "payload-unreadable",
    "payload-absent",
    "tool-unnamed",
    "identity-unminted",
];

/// One configured agent, as the contract check needs to see it.
pub struct Contract {
    /// The adapter slug.
    pub agent: &'static str,
    /// The file it reads.
    pub path: std::path::PathBuf,
    /// Whether this adapter has a skill directory of its own.
    pub own_root: bool,
    /// Rows this adapter's own file sets that nothing will read.
    pub ignored: Vec<&'static str>,
    /// A legacy operator file kept alive by nothing, if one is there.
    ///
    /// Estigia reads `estigia.local.md`, and `operator.local.md` only when the
    /// first is absent. An installation that came from issue-flow carries the
    /// older name; the day somebody writes the newer one, the older stops being
    /// read and says nothing about it — rows an operator still believes are in
    /// force, in a file that was authoritative until it silently was not.
    pub shadowed_local: Option<String>,
    /// Rows read as less than the file says, with the file and what cut them.
    ///
    /// A cell separator or a line separator inside a value ends the row early,
    /// and it is read as less than it says. `config set` refuses three such
    /// characters by name; a file Estigia never writes has nothing refusing
    /// anything. Measured twice on the row that declares one-way doors: an
    /// `estigia.local.md` holding a pipeline is read up to the bar, and one
    /// holding a pasted U+2028 loses everything after it — `npm publish` stops
    /// being a declared boundary, which is configuration *loosening* silently.
    pub cut_short: Vec<(String, crate::config::CutShort)>,
    /// Rows this checkout's chosen tracker has nothing to do with.
    ///
    /// The screen stops offering the board row on a tracker that has no board,
    /// which is right for a row nobody has answered yet and does nothing for the
    /// one already written. Measured: with `acme/7` set and the tracker moved to
    /// Linear, `config list` still reported `acme/7`, the contract the **agent
    /// reads** still carried `| Project board | acme/7 |`, and nothing said the
    /// transport under that tracker will never look at it.
    ///
    /// Hiding it made the operator stop seeing the problem rather than stop
    /// having it. It used to be told apart from a second row — rows the gate
    /// read and the transport could not match, for want of the whitespace and
    /// backtick handling this crate does and the retired Python did not. That
    /// row is gone with the Python: there is one reader now, and a row this
    /// crate matches is a row the transport matches. This one is the fact that
    /// survived — a row spelled perfectly, for a tracker that does not read it.
    pub inert_for_this_tracker: Vec<(&'static str, String)>,
    /// Settings a hand-edited file names twice, with the file that names them.
    ///
    /// Rows apply in the order they are met, so the lower of two rows for one
    /// setting is the one in force — decided by where a person happened to type
    /// it, which is not a rule anything states. An alias makes the pair harder
    /// to spot still: the two rows need not look alike.
    pub duplicated: Vec<(String, &'static str)>,
    /// Why its configuration cannot be read, if it cannot.
    ///
    /// A file that exists is not a file a run can use. Three documents layer
    /// into what an agent actually reads, two of them hand-editable, and one
    /// bad row in any of them refuses every command that reads configuration.
    /// Checked by statting the contract alone, `doctor` reported `ok contract`
    /// on a machine where `config list` would not answer at all.
    pub unreadable: Option<String>,
    /// The instruction file whose directive is not this binary's, if it drifted.
    ///
    /// A separate field from [`Self::stale`] so the row can name **the file
    /// that moved**. Round 61 learned that naming `SKILL.md` when the transport
    /// had drifted sends an operator to read a file that is already current.
    pub directive: Option<std::path::PathBuf>,
    /// The instruction file that holds no directive at all, if there is one.
    ///
    /// Its own field beside [`Self::directive`], because the two are different
    /// states and only one of them was reported: a directive that **drifted**
    /// was on the report and one that was **gone** was not — the wrong rules
    /// against no rules, and the worse of the two missing. See
    /// [`crate::setup::directive_installed`] for the measurement.
    pub directive_absent: Option<std::path::PathBuf>,
    /// Whether what is installed here is not what this binary would write.
    ///
    /// The binary and the skill are upgraded separately. `status` says "skill
    /// out of date" and the gate says it again at every `SessionStart` — "what
    /// you read and what the gate enforces may differ" — and `doctor`, the one
    /// command whose whole job is to answer *is my environment right*, said
    /// `ok` on all ten. Three commands on one machine, and the two that were
    /// right were the two nobody runs to find out.
    pub stale: bool,
}

/// Where each configured agent's contract actually landed, and whether it is
/// there.
///
/// # Why this exists rather than a longer list of verified adapters
///
/// Only two adapters have a skill directory of their own, because a directory
/// is only claimed once somebody has checked a **real installation** — see
/// `setup::tests::only_the_two_verified_adapters_have_a_skill_root_of_their_own`.
/// The other eight write to the shared neutral root, which works, and the
/// directive names the path.
///
/// The gap was never the code: it is that nobody could tell, on their own
/// machine, whether their agent is reading what Estigia wrote. Promoting an
/// adapter by reading its documentation would be exactly the unchecked claim
/// that test forbids. So instead of verifying eight installations here, this
/// ships the thing that verifies one **wherever it is** — and reports the file
/// by its full path, so an operator can hand back an answer nobody had to
/// guess at.
pub fn contracts(agents: &[Contract]) -> Vec<Check> {
    if agents.is_empty() {
        return vec![Check {
            name: "contract",
            about: "where each configured agent reads its contract",
            health: Health::broken(
                "no agent is configured, so nothing reads a contract",
                Resolution::run("estigia setup --all"),
            ),
        }];
    }

    agents
        .iter()
        .map(|entry| Check {
            name: "contract",
            about: "the file this agent reads, and whether a run can read it",
            health: if let Some(why) = entry.unreadable.as_deref() {
                // Not a resolution that names a command. Two of the three
                // documents are the operator's, and Estigia does not edit
                // those — which is why it can say where the row is but not
                // fix it.
                Health::broken(
                    // What it costs, not only what is wrong — and what it costs
                    // has narrowed, so this sentence had to. It used to say the
                    // gate runs on defaults and *no* command declared
                    // irreversible is treated as one, which was true while
                    // `Config::read` discarded a whole document over one bad
                    // row. The gate now keeps every row that parses, so the loss
                    // is this row and the rows the file never carried. The
                    // renewal window is taken back to zero meanwhile.
                    //
                    // Left saying it out loud rather than trimmed to "this row
                    // is wrong": an operator reading `BROKEN` wants to know what
                    // is not being enforced while it stands, and "the setting
                    // this row was for" is the answer.
                    format!(
                        "{}: {why} — until it is fixed the gate runs without the setting that \
                         row was for; every other row still stands and the renewal window is \
                         held at zero",
                        entry.agent
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        "that row corrected in the file the message names",
                    ),
                )
            } else if let Some((file, cut)) = entry.cut_short.first() {
                // Before the others, because this one is not about what a value
                // costs: it is about the value not being the one that was
                // written. `config set` refuses a `|` by name and this door had
                // nothing refusing anything, so the row was read up to the bar
                // and the rest was gone without a word.
                let label = &cut.label;
                // Two characters, two things to do about them, and the second
                // is the reason this is not one sentence: an operator told to
                // look for a `|` in a row that has none learns nothing. The
                // code point is spelled out because that is what a search box
                // takes — the character itself would paste as nothing.
                let (what, wanted) = match cut.by {
                    crate::config::CutBy::Bar => (
                        "a `|`, which is what separates cells".to_owned(),
                        "that value written without a `|`: it is one cell of a one-line table \
                         row, and nothing escapes either"
                            .to_owned(),
                    ),
                    crate::config::CutBy::Unseen(character) => (
                        format!(
                            "U+{:04X}, a character that draws as nothing and ends a line for \
                             everything that reads this table",
                            character as u32
                        ),
                        format!(
                            "that U+{:04X} removed from the row \u{2014} a paste out of a \
                             browser or a word processor is where it comes from, and no editor \
                             shows it unless asked to",
                            character as u32
                        ),
                    ),
                };
                Health::broken(
                    format!(
                        "{}: `{label}` in {file} holds {what} \u{2014} so that row is read up \
                         to it and the rest of the value is not there",
                        entry.agent
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        wanted,
                    ),
                )
            } else if let Some((label, tracker)) = entry.inert_for_this_tracker.first() {
                // Answered, kept, and read by nothing. The screen stops offering
                // the row once the tracker changes, which takes the question
                // away and leaves the answer — so `config list` reports a value
                // and the contract the agent reads still carries it.
                //
                // Not a refusal to fix it: the row is the operator's answer, and
                // it becomes live again the moment they move back. What it needs
                // is to be said.
                Health::broken(
                    format!(
                        "{}: `{label}` is answered and `{tracker}` does not read it \u{2014} that \
                         binding declares nothing for this row, so the value stands in the \
                         contract and no operation will ever consult it",
                        entry.agent
                    ),
                    Resolution::run(format!("estigia config set {label:?} none")),
                )
            } else if let Some(older) = entry.shadowed_local.as_deref() {
                Health::broken(
                    format!(
                        "{}: {older} is here and nothing reads it \u{2014} `estigia.local.md` beside \
                         it takes precedence, and the older file is ignored entirely rather \
                         than merged",
                        entry.agent
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        "its rows moved into `estigia.local.md`, or the older file removed \
                         once nothing in it is still wanted",
                    ),
                )
            } else if let Some((file, setting)) = entry.duplicated.first() {
                // Ahead of `ignored`: a row nothing reads is a value that did
                // not hold, and this is worse — which value holds depends on
                // which line it was typed on.
                Health::broken(
                    format!(
                        "{}: {file} names {setting} twice \u{2014} the lower row is the one that \
                         holds, so which value is in force depends on line order rather than \
                         on anything the file says",
                        entry.agent
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        "those rows collapsed to the one that was meant to hold \u{2014} an older \
                         label for the same setting counts as naming it again",
                    ),
                )
            } else if !entry.ignored.is_empty() {
                // Broken rather than a note: the operator wrote a value and
                // every command reports a different one. Nothing is damaged,
                // but something they believe is in force is not.
                Health::broken(
                    format!(
                        "{}: {} in its own file that nothing reads. {} \u{2014} the repository \
                         answers for {}, and the contract's value is what every command reports",
                        entry.agent,
                        if entry.ignored.len() == 1 {
                            "one row".to_owned()
                        } else {
                            format!("{} rows", entry.ignored.len())
                        },
                        entry.ignored.join(", "),
                        if entry.ignored.len() == 1 {
                            "it"
                        } else {
                            "them"
                        },
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        "those rows taken out of the agent's own file, and set for the \
                         repository instead if they were meant to hold",
                    ),
                )
            } else if entry.stale {
                // Last of the failures and ahead of `Fine`: the rows above say
                // the contract is *wrong*, and this one says it is *old*. Both
                // are worth a line and only one of them has a command.
                //
                // Broken rather than a note on an `ok` line, because an `ok`
                // line with a caveat is the thing this row exists to stop. The
                // resolution is a command Estigia owns and runs unattended,
                // which makes this strictly more actionable than `silence` —
                // and that one already refuses to let a run swear.
                Health::broken(
                    format!(
                        // The root, not the contract. `presence` compares every
                        // file the skill ships, and on the machine this was
                        // written on the one that differed was the transport —
                        // so a message naming `SKILL.md` would have sent the
                        // operator to read a file that was already current.
                        "{}: the skill installed in {} is not this binary's copy, so the rules \
                         the agent reads are not the rules the gate enforces",
                        entry.agent,
                        entry.path.parent().unwrap_or(&entry.path).display()
                    ),
                    Resolution::run("estigia sync"),
                )
            } else if let Some(instructions) = &entry.directive_absent {
                // Gone, rather than drifted. The rules the agent loads every
                // session are not *different* here — there are none, and this
                // row said `verified` about it while `status` on the same
                // machine said "skill present, directive missing".
                Health::broken(
                    format!(
                        "{}: {} holds no directive, so the agent loads none of the rules the \
                         gate enforces",
                        entry.agent,
                        instructions.display()
                    ),
                    Resolution::run(format!("estigia setup {}", entry.agent)),
                )
            } else if let Some(instructions) = &entry.directive {
                // The one text every session loads, and it had no check at all:
                // `skill::presence` compares the files under the skill root and
                // the directive is somewhere else entirely. Rewriting the first
                // of its three rules by hand left `status` saying `configured`
                // and this row saying `verified`.
                Health::broken(
                    format!(
                        "{}: the directive in {} is not this binary's, so the rules the agent \
                         loads every session are not the rules it was given",
                        entry.agent,
                        instructions.display()
                    ),
                    Resolution::run("estigia sync"),
                )
            } else if entry.path.exists() {
                Health::Fine {
                    detail: format!(
                        "{}: {}{}",
                        entry.agent,
                        entry.path.display(),
                        if entry.own_root {
                            " (this agent's own skill directory, verified)"
                        } else {
                            // Said plainly. An operator who confirms their agent
                            // reads this is the only person who can promote it,
                            // and they cannot confirm what nobody showed them.
                            " (shared neutral root — confirm your agent reads it)"
                        }
                    ),
                }
            } else {
                Health::broken(
                    format!("{}: nothing at {}", entry.agent, entry.path.display()),
                    Resolution::run(format!("estigia setup {}", entry.agent)),
                )
            },
        })
        .collect()
}

/// What to add to a row that names a binary which is not the one running.
///
/// **Pure and fed**: the running executable is handed in, because a function
/// that reads it cannot be shown the case that matters.
///
/// The gate registered in a settings file names a path, and that path is what
/// the agent will execute — not whichever build the operator happens to be
/// typing to. They are the same until somebody installs a second one, moves a
/// profile, or upgrades to a different prefix, and then `doctor` was reporting
/// a healthy gate that runs a build the operator has not seen in a while.
///
/// The sibling of this drift is already said: *the installed contract is not
/// this binary's copy, so what you read and what the gate enforces may differ*.
/// This is the same sentence about the binary, and it was the half nobody said.
///
/// Not `Broken`, deliberately. The gate does run, and a path that differs is
/// sometimes exactly right — a system install answering while a development
/// build is being typed to. What it must not be is invisible.
///
/// Compared through `canonicalize`, so a symlink or a shim onto the same file
/// is not reported as drift; and silent when either side will not resolve,
/// because an alarm invented from a failed read is worse than none.
fn other_build(named: &std::path::Path, running: Option<&std::path::Path>) -> String {
    let Some(running) = running else {
        return String::new();
    };
    let (Ok(named_real), Ok(running_real)) = (named.canonicalize(), running.canonicalize()) else {
        return String::new();
    };
    if named_real == running_real {
        return String::new();
    }
    format!(
        " \u{2014} not this build ({}), so what you are typing to and what the gate runs may \
         differ; `estigia setup` re-registers this one",
        running.display()
    )
}

/// Whether the gate each configured agent registers would actually run.
///
/// `status` reports `gate on` for an agent whose settings file carries a hook
/// entry. It does not read what the entry *says*. The command registered on the
/// machine this was written on named a **debug build inside a working tree** —
/// `cargo clean` deletes it, moving the checkout moves it, and either way the
/// agent goes on calling a command that is not there, the call fails, the tool
/// proceeds, and Estigia keeps saying `gate on`.
///
/// Installed, looks installed, enforces nothing. So it is read back.
///
/// Fed the wiring rather than sent to find it: the interesting cases are a file
/// that is absent and one naming an executable that has been deleted, and a
/// function that goes looking cannot be shown to handle either.
pub fn gates(
    agents: &[(&'static str, crate::setup::wiring::Registration, bool)],
    running: Option<&std::path::Path>,
) -> Vec<Check> {
    agents
        .iter()
        .map(|(name, files, gated)| {
            let wires: Vec<&crate::setup::wiring::Wire> =
                files.iter().flat_map(|(_, found)| found).collect();
            // No entry at all is not a fault here. `status` already says which
            // agents are gated, and an agent that was never gated has no wiring
            // to be wrong — reporting it broken would make a deliberate choice
            // look like damage.
            // The matcher, before the wires. A gate whose binary is there and
            // whose event resolves still runs for nothing if the field that
            // says *which tools* names none this build judges — and that field
            // is invisible to `Wire`, which reads command lines. Narrowed by
            // hand to a tool that does not exist, this row said `3 live`.
            let judged: Vec<&str> = crate::harness::WRITE_TOOLS
                .iter()
                .chain(crate::harness::SHELL_TOOLS)
                .copied()
                .collect();
            let narrowed: Vec<String> = files
                .iter()
                .filter_map(|(file, _)| std::fs::read_to_string(file).ok())
                .flat_map(|text| crate::setup::wiring::narrowed(&text, &judged))
                .collect();
            if let Some(matcher) = narrowed.first() {
                return Check {
                    name: "gate",
                    about: "whether the gate this agent registers would actually run",
                    health: Health::broken(
                        format!(
                            "{name}: the gate is registered and its matcher is `{matcher}`, which \
                             names no tool this build gates \u{2014} it wakes for nothing, so every \
                             write goes through and the entry looks installed"
                        ),
                        Resolution::run("estigia setup --all"),
                    ),
                };
            }
            let Some(faulty) = wires.iter().find(|wire| wire.fault().is_some()) else {
                return Check {
                    name: "gate",
                    about: "whether the gate this agent registers would actually run",
                    // An entry is registered here, this build read none of it,
                    // and the agent is one gated **through a settings file** —
                    // so there is wiring, and what is unknown is whether it
                    // would run. Reported as broken rather than folded into the
                    // sentence below, which says the opposite in as many words:
                    // *"there is no wiring here to be wrong"*.
                    //
                    // Measured: pointing `.claude/settings.json` at
                    // `…\.cargo\bin\ausente.exe` — a binary that is not there —
                    // made `doctor` say Claude Code *"is gated by its own file
                    // rather than a settings entry"*, which it is not, and
                    // `status` on the same machine said `gate on`. The two
                    // readers disagree by design: `is_gated` recognises the
                    // entry by `hook pre-tool-use`, and `wire_in` requires the
                    // executable's own name to hold `estigia`. A copy renamed to
                    // something else is invisible to the second and plain to the
                    // first, and a disagreement between them is the fact worth
                    // reporting.
                    health: if wires.is_empty()
                        && *gated
                        && crate::setup::find_agent(name)
                            .is_ok_and(crate::setup::AgentAdapter::supports_hooks)
                    {
                        Health::broken(
                            format!(
                                "{name}: an entry is registered and this build cannot read it as \
                                 one of its own, so whether the gate would run is unknown"
                            ),
                            Resolution::run(format!("estigia setup {name}")),
                        )
                    } else if wires.is_empty() {
                        Health::Skipped {
                            // Two mechanisms, one question. `is_gated` says so
                            // in as many words — "an agent gated by a plugin
                            // rather than by a settings hook... an operator
                            // looking at a run that wrote without a claim needs
                            // the answer, not the implementation" — and this
                            // check knew only about the settings hooks. OpenCode
                            // gates through a plugin file, produces no wires,
                            // and was reported here as `no gate registered`
                            // while `status` on the same machine said `gate on`.
                            detail: if *gated {
                                format!(
                                    "{name}: gated by its own file rather than a settings entry, \
                                     so there is no wiring here to be wrong"
                                )
                            } else {
                                format!("{name}: no gate registered — the contract only")
                            },
                        }
                    } else {
                        Health::Fine {
                            detail: format!(
                                "{name}: {} live, running {}{}",
                                wires.len(),
                                wires[0].executable.display(),
                                other_build(&wires[0].executable, running)
                            ),
                        }
                    },
                };
            };
            let file = files
                .iter()
                .find(|(_, found)| found.iter().any(|wire| wire == *faulty))
                .map(|(file, _)| file.display().to_string())
                .unwrap_or_default();
            Check {
                name: "gate",
                about: "whether the gate this agent registers would actually run",
                health: Health::broken(
                    format!(
                        "{name}: the gate is registered and would not run — it {} ({file})",
                        faulty.fault().unwrap_or_default()
                    ),
                    // Re-registering is what fixes both faults: it rewrites the
                    // entry with this build's event names and this executable's
                    // real path.
                    Resolution::run(format!("estigia setup {name}")),
                ),
            }
        })
        .collect()
}

/// Everything `doctor` looks at, by the name it reports under.
///
/// The README counts these, and a count is the wrong shape for what `full`
/// returns: two of the thirteen produce **one row per configured agent**, so a
/// run on a bare machine reports eight rows and a run on a busy one reports
/// twenty-one. What a reader means by "doctor checks eight things" is the
/// concerns, and this is them — crossed against what `full` actually emits,
/// both ways, by `the_number_of_things_doctor_checks_is_the_number_the_readme_claims`
/// and `every_name_this_list_declares_is_one_doctor_can_report`.
pub const CHECKS: &[&str] = &[
    "skill",
    "transport",
    "gh",
    "push-guard",
    "remote",
    "contract",
    "canonical",
    "gate",
    "tools",
    "run-pointer",
    "stale-run-pointer",
    "stand-down",
    "silence",
];

/// One row two roots answer differently: the setting, what the agent reads, and
/// what the gate decides.
///
/// The **setting** rather than its label, so the scope can be asked of it where
/// the row is judged. Carrying the label meant matching that string back
/// against a list of settings to recover what the type already knew, which is
/// the copy this repository's rules say to remove rather than to guard.
///
/// Both values are carried rather than a bare "these differ", because an
/// operator cannot act on the second: which of the two is the one they meant is
/// the whole question, and it is answered by seeing them side by side.
pub type DivergentRow = (crate::config::Setting, String, String);

/// One configured agent reading a row the gate does not decide by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// Which agent.
    pub agent: &'static str,
    /// Each row they disagree about.
    pub rows: Vec<DivergentRow>,
}

/// Whether the root the gate decides in carries what the agents read.
///
/// The report had eleven contract rows and one `skill` row and never related
/// the two, so a machine could hold — and did hold — an agent activating a
/// contract that says `Delivery authorisation: auto` while the gate adjudicated
/// `ask`, with every row of the report `ok`. Both halves were telling the truth
/// about their own file. Nothing was asking whether they were the same file.
///
/// Named `canonical` because that is the noun the rest of the crate uses for
/// the one root [`super::discover_skill_root`] picks, and this row is about
/// that pick rather than about any one agent's contract.
///
/// **Which half is a fault is decided by [`crate::config::Setting::scope`], not
/// here** — the crate's own answer to *who does this row belong to*, and it has
/// three values rather than two:
///
/// - `Agent` — the answer depends on which agent holds the tools. Differing
///   between two roots is the feature working: `config set --agent opencode
///   "Planning" "sdd lite"` is a documented command, and calling the machine
///   BROKEN for having run it reports a supported configuration as a fault.
///   Named, and `ok`.
/// - `Everywhere` — a fact about the repository, the same whichever agent asks.
///   Two roots answering differently means one agent is being decided for by a
///   file it does not read, and one of the two is the root the gate decides in.
///   `Broken`.
/// - `Machine` — a fact about this machine, the same whichever repository is
///   open. Also `Broken`, and offered the same resolution as the row above; it
///   needed its own until the plain `config set` learned to propagate one, and
///   the paragraph below says what that cost to find.
///
/// The first cut of this asked `AGENT_SETTINGS` instead, which is one of two
/// lists rather than the three-valued question, so the two `Machine` rows were
/// classified by falling off the end of it.
///
/// **Both faulty scopes are offered the same command, and that is new.** The
/// resolution used to fork, because only one of them had a way out: a repository
/// row set with no `--agent` was written into every installed contract, while a
/// machine row had no command at all. Two rounds of review were spent
/// establishing that — the plain form wrote the canonical contract alone because
/// `elsewhere` was asked only for `Everywhere`, and the per-agent form could not
/// hold one either, since a shared root's per-agent file is rendered through
/// `render_some_agent_rows` and read through `Scope::Agent`, so the row was
/// dropped on the way out and the command exited on its own read-back. Four
/// judges ran first one form and then the other, verbatim, and watched the row
/// stay exactly as red. That was issue #62, and both halves are now fixed: the
/// plain form propagates a machine row, and the per-agent form refuses one. So
/// the fork is gone rather than kept as a branch describing a closed gap.
///
/// **Both halves are named in both branches.** They were not: the broken branch
/// built its sentence from the faulty rows alone, so a deliberately-set
/// per-agent row vanished from the report the moment anything else disagreed —
/// the same *shown without a word* failure the count of other agents below
/// exists to prevent.
pub fn canonical(root: Option<&Path>, divergent: Option<&[Divergence]>) -> Check {
    let about = "whether the root the gate decides in carries what the agents read";
    let Some(root) = root else {
        return Check {
            name: "canonical",
            about,
            health: Health::Skipped {
                detail: "no skill root was resolved, so there is nothing to compare".to_owned(),
            },
        };
    };
    // A comparison that could not be made is never reported as one that agreed.
    // `Fine` on an unreadable canonical configuration would say the two halves
    // match, which is the silence this row exists to end.
    let Some(divergent) = divergent else {
        return Check {
            name: "canonical",
            about,
            health: Health::Skipped {
                detail: format!(
                    "the configuration in {} could not be read, so nothing was compared against \
                     it \u{2014} the `contract` row says why",
                    root.display()
                ),
            },
        };
    };
    use crate::config::Scope;

    // Split before anything is said, because the halves are different sentences
    // and only one of them is a fault.
    let at_odds: Vec<(&'static str, Vec<DivergentRow>)> = divergent
        .iter()
        .filter_map(|entry| {
            let rows: Vec<DivergentRow> = entry
                .rows
                .iter()
                .filter(|(setting, _, _)| setting.scope() != Scope::Agent)
                .cloned()
                .collect();
            (!rows.is_empty()).then_some((entry.agent, rows))
        })
        .collect();

    // What differs by design, said in both branches rather than dropped in one:
    // an operator who set one deliberately should see it named where they went
    // looking for divergence, and an operator who did not should learn that
    // somebody or something did.
    let by_design: Vec<String> = divergent
        .iter()
        .filter_map(|entry| {
            let rows: Vec<&str> = entry
                .rows
                .iter()
                .filter(|(setting, _, _)| setting.scope() == Scope::Agent)
                .map(|(setting, _, _)| setting.label())
                .collect();
            (!rows.is_empty()).then(|| format!("{} ({})", entry.agent, rows.join(", ")))
        })
        .collect();
    let deliberate = match by_design.is_empty() {
        true => String::new(),
        false => format!(
            "; what differs by design is per-agent: {}",
            by_design.join(", ")
        ),
    };

    let Some((agent, rows)) = at_odds.first() else {
        return Check {
            name: "canonical",
            about,
            health: Health::Fine {
                detail: match by_design.is_empty() {
                    true => format!(
                        "{} \u{2014} and every configured agent reads the rows it decides by",
                        root.display()
                    ),
                    false => format!(
                        "{} \u{2014} every shared row agrees{deliberate}",
                        root.display()
                    ),
                },
            },
        };
    };
    // The first agent, and its rows, rather than every pair: nine adapters
    // sharing one root produce nine copies of one sentence. How many were left
    // out is said rather than dropped — a report that shows two of eleven
    // without a word reads as two.
    let listed: Vec<String> = rows
        .iter()
        .map(|(setting, theirs, ours)| format!("`{}` {theirs} against {ours}", setting.label()))
        .collect();
    let others = at_odds.len() - 1;
    let rest = match others {
        0 => String::new(),
        1 => " (one other agent diverges too)".to_owned(),
        many => format!(" ({many} other agents diverge too)"),
    };
    // One sentence for both faulty scopes, because there is now one answer.
    // This used to fork: a repository row was offered the plain `config set`
    // and a machine row was told, correctly at the time, that nothing cleared
    // it — `elsewhere` was asked only for `Everywhere`, so the plain form wrote
    // the canonical contract alone, and the per-agent form could not hold a
    // machine row in a shared root either. Both halves are fixed (issue #62),
    // so the fork went with them rather than staying as a branch describing a
    // gap that is closed.
    let way_out = "those rows made to agree \u{2014} `estigia config set \"<row>\" \"<value>\"` \
                   with no `--agent` writes a row about the repository or about this machine into \
                   every installed contract";
    Check {
        name: "canonical",
        about,
        health: Health::broken(
            format!(
                "{agent} reads {} where the gate decides in {}{rest}{deliberate}",
                listed.join(", "),
                root.display()
            ),
            // Which of the two values is the right one stays the operator's:
            // this names the shape of the write, never the answer.
            crate::outcome::Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                way_out,
            ),
        ),
    }
}

/// The tracker in force in a checkout, for the checks that ask about it.
///
/// The layer, not the contract underneath it. Both callers had this written out
/// and both read one level too low, so a checkout that had chosen another
/// tracker was still checked against the GitHub CLI — and, with `gh`
/// unauthenticated, told a run it could not swear yet over a program that
/// checkout does not use.
///
/// Never a refusal: `doctor` is what somebody runs *because* something is
/// wrong, and a health report that will not open on an unreadable file has
/// nothing left to report the unreadable file with. The `contract` check is
/// where that is said.
/// Read the way the gate reads it, keeping every row that parses. Strictly, one
/// mistyped value anywhere in any of the three documents fell back to
/// `Tracker::default()` — and the fallback is not neutral. Measured on the
/// installed binary, on a machine configured for Linear, after a `30 days`
/// typed into `Renewal window`:
///
/// ```text
/// $ estigia doctor
/// estigia: gh is not usable, so a run cannot swear yet: the GitHub CLI the
///          transport reaches the tracker through (environment-not-ready)
/// ```
///
/// A refusal, out of the command whose whole point is that it never is one — and
/// it buried the real fault under a GitHub CLI the operator does not use. The
/// `Tracker` row had parsed; it was thrown away with the row beside it.
pub fn tracker_in_force(skill_root: Option<&Path>, repo_dir: &Path) -> crate::config::Tracker {
    skill_root
        .map(|root| crate::skill::installed_config_in_keeping_what_parses(root, repo_dir).0)
        .unwrap_or_default()
        .tracker
}

/// Every check `estigia doctor` runs, assembled.
///
/// [`examine`] is the six that need nothing but a skill root; the rest need to
/// know which agents are configured, and used to be stitched together inside the
/// command. That put half of what `doctor` reports somewhere no test could
/// reach — and the README's own count was then checked against a function that
/// answers six while the command prints eight.
///
/// A promise held at the wrong scope is the same fault twice in one session, so
/// the whole thing lives here where the claim about it can be measured.
pub fn full(
    skill_root: Option<&Path>,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    options: &crate::setup::SetupOptions,
) -> Vec<Check> {
    let mut checks = examine(skill_root, repo_dir, tracker);

    // Where each configured agent actually reads from. Eight of the eleven
    // adapters share a neutral root because nobody has checked a real
    // installation for them, and reading their documentation instead would be
    // the unchecked claim `setup::tests` forbids. This does not verify them —
    // it puts the exact path in front of the one person who can.
    let configured: Vec<&'static crate::setup::AgentAdapter> = crate::setup::AGENTS
        .iter()
        .filter(|adapter| crate::setup::is_present(adapter, options))
        .collect();

    let agents: Vec<Contract> = configured
        .iter()
        .filter_map(|adapter| {
            let paths = crate::setup::resolve_paths(adapter, options).ok()?;
            // The layered read, not a stat: what every command that reads
            // configuration will get. Bound once because the currency check
            // needs the same answer — rendering the expected contract takes the
            // configuration that is in it.
            let config = crate::skill::installed_config_for(&paths.skill_root, Some(adapter.slug));
            Some(Contract {
                agent: adapter.slug,
                path: paths.skill_root.join("SKILL.md"),
                own_root: adapter.discovers_skills(),
                // A contract nothing can read is reported by the row above, and
                // asking whether an unreadable one is *current* would compare
                // against a configuration that is not the operator's.
                // Through `presence_of`, which asks the contract what it says
                // about itself. This passed `config` — the *effective* one,
                // carrying `estigia.local.md` and the per-agent file on top —
                // and so reported a contract as not this binary's copy for
                // holding exactly what this binary wrote, whenever an operator
                // used the override file the contract tells them to use.
                // `Unreadable` is not `Stale`, which is what the guard here
                // used to be for.
                stale: matches!(
                    crate::skill::presence_of(&paths.skill_root),
                    crate::skill::Presence::Stale
                ),
                // Read through the same `upsert` setup runs, so this cannot
                // drift from what a repair would produce.
                directive: (crate::setup::directive_is_current(adapter, options) == Some(false))
                    .then(|| paths.instructions.clone()),
                // Only where the skill is installed. An agent nobody set up has
                // an instruction file of its own with no block in it, and that
                // is not a fault — it is somebody who declined.
                directive_absent: (paths.skill_root.join(crate::skill::CONTRACT).is_file()
                    && crate::setup::directive_installed(adapter, options) == Some(false))
                .then(|| paths.instructions.clone()),
                unreadable: config
                    .err()
                    .filter(|refusal| refusal.code != "skill-not-installed")
                    .map(|refusal| refusal.message),
                ignored: crate::skill::overridden_rows(&paths.skill_root, adapter.slug),
                shadowed_local: crate::skill::shadowed_local(&paths.skill_root),
                duplicated: crate::skill::duplicated_rows(&paths.skill_root, adapter.slug),
                cut_short: crate::skill::rows_cut_short(&paths.skill_root, adapter.slug),
                inert_for_this_tracker: crate::skill::installed_config_for(
                    &paths.skill_root,
                    Some(adapter.slug),
                )
                .map(|config| {
                    crate::config::SETTINGS
                        .iter()
                        .filter(|setting| !setting.applies_to(&config.tracker))
                        .filter(|setting| {
                            setting.value_of(&config)
                                != setting.value_of(&crate::config::Config::default())
                        })
                        .map(|setting| (setting.label(), config.tracker.as_value()))
                        .collect()
                })
                .unwrap_or_default(),
            })
        })
        .collect();
    checks.extend(contracts(&agents));

    // And whether the root the gate decides in is the root they read. Both
    // sides are already resolved here; what was missing was anybody comparing
    // them.
    //
    // The gate's side is read without a slug — that half *is* how
    // `gate_context` reads it, and it is what makes the per-agent rows the gate
    // is blind to visible here. The repository layer is left off **both** sides
    // instead: `gate_context` does apply it, so the honest thing to say is that
    // this compares the two files rather than reproducing the gate's effective
    // value, and a layer applied to neither side cannot invent a divergence
    // between them. Symmetry is the property that matters, and this is the
    // pair that has it.
    let decided_by = skill_root.and_then(|root| crate::skill::installed_config(root).ok());
    let divergent: Option<Vec<Divergence>> = decided_by.map(|ours| {
        configured
            .iter()
            .filter_map(|adapter| {
                let paths = crate::setup::resolve_paths(adapter, options).ok()?;
                let theirs =
                    crate::skill::installed_config_for(&paths.skill_root, Some(adapter.slug))
                        .ok()?;
                let rows: Vec<DivergentRow> = crate::config::SETTINGS
                    .iter()
                    .filter_map(|setting| {
                        let read = setting.value_of(&theirs);
                        let decided = setting.value_of(&ours);
                        (read != decided).then_some((*setting, read, decided))
                    })
                    .collect();
                (!rows.is_empty()).then_some(Divergence {
                    agent: adapter.slug,
                    rows,
                })
            })
            .collect()
    });
    checks.push(canonical(skill_root, divergent.as_deref()));

    // And whether the gate each of them registers would actually run. `status`
    // says `gate on` for any agent whose settings file carries an entry, which
    // is not the same as an entry naming an event this build has and an
    // executable that is still on disk.
    let wired: Vec<(&'static str, crate::setup::wiring::Registration, bool)> = configured
        .iter()
        .map(|adapter| {
            (
                adapter.slug,
                crate::setup::wiring::registered(adapter, options),
                // Whether it is gated *at all*, by either mechanism — the
                // question this check was answering from half the evidence.
                crate::setup::is_gated(adapter, options),
            )
        })
        .collect();
    let running = std::env::current_exe().ok();
    checks.extend(gates(&wired, running.as_deref()));

    // And the other half of the same question. `status` reports the gate and
    // the tools side by side; only one of them was read back.
    let servers: Vec<(&'static str, Option<std::path::PathBuf>, Option<bool>)> = configured
        .iter()
        .map(|adapter| {
            (
                adapter.slug,
                crate::setup::tools_command(adapter, options),
                // `Some(false)` is an entry that names the server and
                // nowhere says how to start it. `None` is one this reader
                // could not take apart, which is not the same and says
                // nothing.
                crate::setup::tools_start_the_server(adapter, options),
            )
        })
        .collect();
    checks.extend(tool_servers(&servers, running.as_deref()));

    // And whether the gate has been standing aside without anybody hearing it.
    //
    // Carried rather than defaulted. `unwrap_or_default()` here handed the
    // reader an empty path, and `ledger_path` on an empty path returns the
    // *relative* `decisions.jsonl` — so a machine whose home cannot be resolved
    // had this check answering about whatever file happened to sit in the
    // directory the operator was standing in, or reporting confidently about
    // one that was never the ledger. A machine with no resolvable home is
    // exactly the machine `doctor` is for.
    // And the row that promises a push is adjudicated stops promising it while
    // the gate is down. Two rows, three lines apart, said opposite things about
    // the same moment: `ok push-guard — a push … is refused unless that claim
    // justifies it`, above `stand-down — writes go through unadjudicated`.
    //
    // Measured: with a stand-down in force, the gate answers `allow` to
    // `git push`, `gh pr merge` and `git tag`, and so does the pre-push hook. An
    // operator checking this report before a risky push reads `ok` on the row
    // whose whole subject is that push.
    //
    // Amended here rather than inside `examine`, which does not know about the
    // stand-down and must not learn by reading the machine: it is called with
    // fake homes throughout the tests, and a row that consulted the real state
    // root would answer differently depending on who ran the suite.
    // The same liveness test the row below applies, and for the same reason: an
    // *unreadable* stand-down is not honoured by the gate, so the promise still
    // holds and this row must not be softened. Only a record that covers the
    // present moment suspends it.
    // Three rows below read the state root, and all three used to ask for it as
    // `state_root(None)` — the real machine's home — while holding an explicit
    // override in `options.home_dir` that a caller had gone to the trouble of
    // setting. `full`'s own test already passes a temporary home and was being
    // answered about the operator's real one: the stand-down, the run pointers
    // and the ledger of the machine running the suite. Nothing in `doctor`
    // honoured the flag its own signature carries.
    let home = options.home_dir.as_deref();
    let standing = crate::harness::session::state_root(home)
        .map_or(crate::harness::standdown::Standing::Away, |root| {
            crate::harness::standdown::standing(&root)
        });
    let now = crate::harness::session::now_seconds();
    amend_push_guard(&mut checks, &standing, now);

    // Before the silence check, which is about the past: this one is about now,
    // and an operator reading down the page needs to know the gate is open
    // before they read what went through it.
    checks.push(standing_down(&standing, now));
    checks.push(run_pointers(
        &crate::harness::session::state_root(home)
            .map(|root| crate::harness::session::unreadable_holdings(&root))
            .unwrap_or_default(),
    ));
    checks.push(stale_run_pointers(
        skill_root,
        repo_dir,
        tracker,
        &crate::harness::session::state_root(home)
            .map(|root| crate::harness::session::holdings(&root))
            .unwrap_or_default(),
    ));
    checks.push(silence(&ledger_state(crate::harness::session::state_root(
        home,
    ))));
    checks
}

/// Run pointers on this machine that are there and cannot be read.
///
/// Pure and fed, like every check beside it: the walk belongs to `session`, and
/// what is done about the answer belongs here.
///
/// It is here because `doctor` is where an operator goes when their agent has
/// stopped being able to write, and a pointer nothing can parse is exactly that
/// — the gate refuses every write from that run by name, and the push guard
/// refuses a push from a checkout nothing else holds. Both are right. Neither
/// is `doctor`, which reported `ok` on all eleven rows while the machine was in
/// that state, and `status` alone carried the news. Two commands describing one
/// machine and disagreeing is the shape the `status` half was added to end; it
/// was ended on one side only.
///
/// Broken rather than skipped, and the difference is the point: a stand-down is
/// a loosening somebody chose and can wait out, while this is a fault nobody
/// chose and waiting does not fix.
pub fn run_pointers(unreadable: &[String]) -> Check {
    let about = "whether every run pointer on this machine can be read";
    if unreadable.is_empty() {
        return Check {
            name: "run-pointer",
            about,
            health: Health::Fine {
                detail: "every run pointer here answers what it holds".to_owned(),
            },
        };
    }
    Check {
        name: "run-pointer",
        about,
        health: Health::broken(
            format!(
                "{} run pointer(s) are here and cannot be read, so what they hold is unknown \u{2014} \
                 every write from those runs is refused, and so is a push from a checkout nothing \
                 else holds: {}",
                unreadable.len(),
                unreadable.join(", ")
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what those runs hold, read from the tracker \u{2014} then claimed again, or \
                 their pointers taken away",
            ),
        ),
    }
}

/// Run pointers that parse cleanly and name an issue the tracker reports
/// closed.
///
/// [`run_pointers`] answers *which pointers cannot be read at all*; this
/// answers the shape issue #90 was filed against — a pointer that reads back
/// fine and names an issue nobody holds any more, left on disk with nothing
/// to expire it once the issue closed. `estigia guard` already reconciles
/// this same question away when **two or more** pointers name one checkout
/// (`guard::adjudicate_action`), so what is left for a person to act on here
/// is the case that reconciliation deliberately leaves alone: a single stale
/// pointer, which still gates every write in the checkout it names — see
/// `docs/honesty.md` for why that residue is not closed by this row either.
///
/// A tracker read that fails answers nothing about staleness — the same rule
/// `issue_is_closed_per_tracker` is built on — so a pointer this could not
/// ask about is never folded into either verdict; it is its own count, named
/// separately, because reporting it as "fine" would hide the exact machine an
/// operator most needs this row on, and reporting it as "stale" would tell
/// them to release a claim that may still be live.
pub fn stale_run_pointers(
    skill_root: Option<&Path>,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    holdings: &[super::session::Run],
) -> Check {
    let about = "whether a readable run pointer still names an issue the tracker holds open";
    if tracker.transport().is_none() {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::Fine {
                detail: "skipped: this tracker has no executable transport to ask".to_owned(),
            },
        };
    }
    let Some(skill_root) = skill_root else {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::Fine {
                detail: "skipped: no skill root is installed to ask the tracker through".to_owned(),
            },
        };
    };
    let mut stale = Vec::new();
    let mut unread = Vec::new();
    for run in holdings {
        match super::tracker_answer_for_pointer(skill_root, repo_dir, tracker, run) {
            Some(answer) if answer.code == 1 && answer.reason() == Some("issue-not-open") => {
                stale.push(format!(
                    "{} (#{})",
                    run.run_id,
                    run.issue.unwrap_or_default()
                ));
            }
            // Read, and answered — with anything else. `unexpected-state`,
            // `not-current-live-holder`, or a plain pass at code `0`: the
            // tracker was reached and this is not the one answer that means
            // "closed".
            Some(answer) if matches!(answer.code, 0 | 1) => {}
            // Everything else is a call that did not land: a read that failed,
            // a clock this machine could not answer, or a `gh` that would not
            // spawn. Not stale, and not fine either.
            Some(_) | None => unread.push(run.run_id.clone()),
        }
    }
    if !stale.is_empty() {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::broken(
                format!(
                    "{} run pointer(s) here are readable and name an issue the tracker reports \
                     closed \u{2014} the checkout each names is still gated by it: {}",
                    stale.len(),
                    stale.join(", ")
                ),
                Resolution::run(format!(
                    "estigia release --run-id <run-id>   # once for each of: {}",
                    stale.join(", ")
                )),
            ),
        };
    }
    if !unread.is_empty() {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::broken(
                format!(
                    "{} run pointer(s) could not be checked against the tracker, so whether \
                     they are stale is unknown: {}",
                    unread.len(),
                    unread.join(", ")
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::WorldAction,
                    "a tracker read that succeeds \u{2014} the `gh` row above says what stopped \
                     this one",
                ),
            ),
        };
    }
    Check {
        name: "stale-run-pointer",
        about,
        health: Health::Fine {
            detail: "every readable run pointer still names an issue the tracker holds open"
                .to_owned(),
        },
    }
}

/// What the ledger says, or why it could not be asked.
///
/// Takes the result rather than a path, because the defect was in the handling
/// of the failure and nowhere else: `unwrap_or_default()` handed the reader an
/// empty path, and [`ledger_path`] on an empty path yields the **relative**
/// `decisions.jsonl` — so a machine whose home will not resolve had this check
/// answering about whatever file sat in the directory the operator was standing
/// in, or saying confidently that nothing had gone ungated about a file that was
/// never the ledger. A machine with no resolvable home is exactly the machine
/// `doctor` exists for.
///
/// [`ledger_path`]: crate::harness::session::ledger_path
fn ledger_state(state_root: Result<std::path::PathBuf, crate::outcome::Refusal>) -> Ungated {
    match state_root {
        Ok(root) => ungated_calls(&root),
        Err(refusal) => Ungated {
            unreadable: Some(refusal.message),
            ..Ungated::default()
        },
    }
}

/// What the ledger could be made to say about calls that went undecided.
///
/// A list is an answer. Not having been able to read the file is not one, and
/// both used to arrive here as the same empty vector — which the check then
/// reported as "every call the ledger records was decided on". The one check
/// whose subject is a silence had three of its own.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ungated {
    /// The calls recorded as reaching the gate and not being decided on.
    pub calls: Vec<(String, u64)>,
    /// Lines that are not JSON, so nothing can say what they recorded.
    pub unparsed: usize,
    /// Why the ledger could not be read, if it is there and could not.
    pub unreadable: Option<String>,
    /// Whether there is no ledger at all, which is not a ledger holding nothing.
    pub missing: bool,
    /// What the most recent undecided call recorded about itself.
    ///
    /// The counts say which repair; this says which agent, and where the read
    /// stopped. Without it the resolution below asks the operator "whether the
    /// agent that sent them is one this build knows" — and the answer is in a
    /// file nothing tells them to open.
    ///
    /// The most recent only. An operator acts on the one happening now, and a
    /// check that printed every distinct reason would bury it.
    pub latest: Option<String>,
    /// Which agent sent the most recent one, and through which hook.
    ///
    /// The resolution below used to ask the operator *"whether the agent that
    /// sent them is one this build knows"* — a question the writer could answer
    /// and dropped, so the only route to it was a file nothing tells anybody to
    /// open. Lines written before the writer kept it have none, and the check
    /// says so rather than guessing.
    pub source: Option<String>,
    /// Sessions whose lifecycle hook arrived with no readable payload.
    ///
    /// A different fault from an ungated call and counted apart, because
    /// counting them together is what this check was doing wrong: a
    /// `session-end` fired with nothing on standard input decides nothing and
    /// cannot let a write through. What it does cost is the session's identity
    /// — with no session id there is no run id, and nothing can be sworn in it.
    pub unminted: Vec<(String, u64)>,
}

/// What the ledger says about calls that reached the gate undecided.
///
/// Read here rather than inside the check, which stays pure and fed.
fn ungated_calls(state_root: &std::path::Path) -> Ungated {
    let path = crate::harness::session::ledger_path(state_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ungated {
                missing: true,
                ..Ungated::default()
            };
        }
        Err(error) => {
            return Ungated {
                unreadable: Some(format!("{}: {error}", path.display())),
                ..Ungated::default()
            };
        }
    };
    // The half set aside when the ledger last passed its cap. A call this check
    // exists to find does not stop counting because the file it is in was
    // rotated — and the rotation is what a run crossing the cap does, so the
    // records nearest it are exactly the ones most likely to be over there.
    //
    // Absent is ordinary: most machines never reach the cap. **Unreadable is
    // not**, and defaulting the read treated the two as one — so a rotated half
    // that exists and cannot be opened became a half holding nothing, and this
    // check reported on the other one alone. That is the rule the half above
    // already follows, not carried the two lines down to its neighbour, in the
    // one check whose whole purpose is finding calls nobody decided on: an
    // unknown result is not clearance, and here it read as silence.
    let older =
        match std::fs::read_to_string(crate::harness::session::previous_ledger_path(state_root)) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                return Ungated {
                    unreadable: Some(format!(
                        "{}: {error}",
                        crate::harness::session::previous_ledger_path(state_root).display()
                    )),
                    ..Ungated::default()
                };
            }
        };

    let mut found = Ungated::default();
    for line in older
        .lines()
        .chain(text.lines())
        .filter(|line| !line.trim().is_empty())
    {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            // Counted, not skipped. Every writer of this file puts a `verdict`
            // in every line, so a line nothing can parse is a line that may
            // have recorded anything — including the thing this check looks for.
            found.unparsed += 1;
            continue;
        };
        let Some(verdict) = entry.get("verdict").and_then(serde_json::Value::as_str) else {
            found.unparsed += 1;
            continue;
        };
        // The ways a call reaches the gate and is not decided on. Every one of
        // them is recorded precisely so this can find them — and a verdict
        // added to that list and not to this one is a record nothing reads,
        // which is how `tool-unnamed` came to sit in the ledger under a row
        // answering *every call the ledger records was decided on*.
        if !UNDECIDED.contains(&verdict) {
            continue;
        }
        // The timestamp is for the message; the verdict is the finding. Dropping
        // the record because its `at` will not parse loses the call itself to
        // keep the sentence tidy.
        let at = entry
            .get("at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        // By `at` rather than by position: the ledger is appended to by every
        // run on this machine, and the last line written is not always the last
        // call made.
        if found.latest.is_none() || found.calls.iter().all(|(_, seen)| *seen <= at) {
            found.latest = entry
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);
            let named = |key| {
                entry
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
            };
            // The dialect when the agent is missing, and said to be a dialect.
            // A line written before the hook command carried `--agent` knows
            // only the protocol shape, and several agents share each one — so
            // this is the half of the answer that exists, offered as half.
            // Naming it as the agent is the defect this pair was split to fix.
            found.source = match (named("agent"), named("dialect"), named("event")) {
                (Some(agent), _, Some(event)) => Some(format!("{agent}'s {event} hook")),
                (Some(agent), _, None) => Some(agent.to_owned()),
                (None, Some(dialect), event) => Some(format!(
                    "a{} hook this line names no agent for, speaking the `{dialect}` dialect",
                    event.map(|event| format!(" {event}")).unwrap_or_default()
                )),
                _ => None,
            };
        }
        let named = |key| {
            entry
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        };
        if verdict == "identity-unminted" {
            found
                .unminted
                .push((named("agent").unwrap_or_default(), at));
            continue;
        }
        found.calls.push((verdict.to_owned(), at));
    }
    found
}

/// Stops the push-guard row promising what a live stand-down has suspended.
///
/// The two rows sat three lines apart in the same report and said opposite
/// things about the same moment:
///
/// ```text
/// ok       push-guard  installed — a push from a checkout a live claim holds is refused …
/// skipped  stand-down  … writes go through unadjudicated until it expires
/// ```
///
/// Measured: with a stand-down in force the gate answers `allow` to `git push`,
/// `gh pr merge` and `git tag`, and the pre-push hook exits 0. The row whose
/// entire subject is that push was the one still saying `ok`, and an operator
/// checks this report *before* a risky push, not after.
///
/// Only a record covering the present moment suspends it. An *unreadable*
/// stand-down is not honoured by the gate either — the gate treats it as absent
/// on purpose, so the push guard really is still refusing, and softening this
/// row for it would trade one false sentence for another.
///
/// Applied here rather than inside [`examine`], which does not know about the
/// stand-down and must not learn by reading the machine: it is called with fake
/// homes throughout the tests, and a row that consulted the real state root
/// would answer differently depending on who ran the suite.
fn amend_push_guard(
    checks: &mut [Check],
    standing: &crate::harness::standdown::Standing,
    now: Option<u64>,
) {
    let live = matches!(
        (standing, now),
        (crate::harness::standdown::Standing::Declared(record), Some(now)) if record.covers(now)
    );
    if live
        && let Some(guard) = checks.iter_mut().find(|check| check.name == "push-guard")
        && let Health::Fine { detail } = &guard.health
    {
        guard.health = Health::Skipped {
            detail: format!(
                "{detail} \u{2014} but the gate is standing down, so it is refusing nothing \
                 until that expires"
            ),
        };
    }
}

/// Whether the operator has the gate standing down right now.
///
/// A stand-down is legitimate, bounded and self-expiring, and every write it
/// lets through is recorded as having gone through it. None of that is a reason
/// for it to be **invisible**: it is machine-wide, it lasts up to four hours,
/// and neither `status` nor this said a word while it was in force. The gate
/// was open and the one command whose stated job is *what has to be true before
/// a run can swear to anything* answered `ok` down the page.
///
/// That is the shape this crate has now found three times — `status` saying a
/// skill was out of date while this said `ok contract`, a push going out
/// unmeasured without a word — and this is the case where the loosening is on
/// purpose, which makes seeing it more useful rather than less.
///
/// `Skipped` rather than `Broken`, following `push-guard`: a real reduction in
/// coverage, deliberately made, named with what it costs, and not something to
/// refuse a run over. An operator stands the gate down in order to get work
/// done, and a check that blocked them for doing it is one they would take out.
pub fn standing_down(standing: &crate::harness::standdown::Standing, now: Option<u64>) -> Check {
    let about = "whether the operator has the gate standing down";
    // A file that is there and will not open answers nothing, and "the gate is
    // not standing down" is an answer. The gate treats it as absent on purpose —
    // a corrupt file must not be able to open it — so nothing else on the
    // machine will ever mention it, and an operator who declared a stand-down
    // and got a broken file is told everything is fine while their writes are
    // refused for a reason nothing names.
    if let crate::harness::standdown::Standing::Unreadable(why) = standing {
        return Check {
            name: "stand-down",
            about,
            health: Health::broken(
                format!(
                    "a stand-down file is here and cannot be read, so whether the gate is standing \
                     down is unknown \u{2014} it is not being honoured meanwhile: {why}"
                ),
                // Named, because there **is** one. `no_command` says no command
                // Estigia can name will fix this, and that was false:
                // `stand-down --lift` removes a record it cannot read and says
                // so — *"a stand-down file was here and could not be read; it
                // has been taken away"*. The operator was sent to edit or
                // delete a file by hand while their own tool did exactly that.
                //
                // With the reason, because the flag alone does not parse, and a
                // named command the parser refuses is the dead end the ratchet
                // forbids. Naming half of one is naming one.
                Resolution::run("estigia stand-down --lift --reason cleared"),
            ),
        };
    }
    let record = match standing {
        crate::harness::standdown::Standing::Declared(record) => Some(record),
        _ => None,
    };
    let health = match (record, now) {
        (Some(record), Some(now)) if record.covers(now) => Health::Skipped {
            detail: format!(
                "the gate is standing down for another {} minute(s), declared by {}: {} \u{2014} \
                 writes go through unadjudicated until it expires",
                record.remaining(now).div_ceil(60),
                record.declared_by,
                record.reason
            ),
        },
        // A record that is there and cannot be told apart from one that is not,
        // because the clock would not answer. `gate` already refuses to honour
        // a stand-down it cannot time; saying `ok` here would be this command
        // answering a question it did not ask.
        (Some(_), None) => Health::Skipped {
            detail: "a stand-down is recorded and this machine's clock cannot be read, so \
                     whether it still applies is unknown \u{2014} the gate does not honour one it \
                     cannot time"
                .to_owned(),
        },
        // Declared, not yet over by the clock, and longer than the cap this
        // build honours — see [`crate::harness::standdown::StandDown::covers`]
        // for why the cap belongs on the way in as well as on the way out.
        // `covers` refuses it, and without a row of its own it fell into the arm
        // below and read as *the gate is not standing down*. True about the
        // gate; a lie about the machine. An operator who edited the file to buy
        // a bit longer is told everything is normal while every write is
        // adjudicated as though they had declared nothing — the same silence the
        // unreadable arm above exists to break.
        //
        // An expired record stays in that arm and stays quiet, deliberately:
        // `in_force` keeps one on purpose as evidence, and reporting every past
        // stand-down as a fault is how a report gets ignored.
        (Some(record), Some(now))
            if now < record.until
                && record.until.saturating_sub(record.declared_at)
                    > crate::harness::standdown::LONGEST =>
        {
            Health::broken(
                format!(
                    "a stand-down here declares {} minute(s) and this build honours at most {} \
                     \u{2014} it is not in force, and writes are being adjudicated as though none \
                     had been declared: {}",
                    record.until.saturating_sub(record.declared_at) / 60,
                    crate::harness::standdown::LONGEST / 60,
                    record.reason
                ),
                // Removes the record and says it did, which is what clears this
                // row. Declaring again is the same command, so naming it is not
                // a dead end.
                Resolution::run("estigia stand-down --lift --reason cleared"),
            )
        }
        // Declared, still ahead by the clock, and **not yet begun**. The arm
        // above bounds how wide a window may be and says nothing about where it
        // sits, so a record stamped in the future is a legal window the gate
        // refuses to honour — and without a row of its own it fell into the
        // quiet arm below and read as *the gate is not standing down*. True
        // about the gate, and a lie about the machine, exactly as the arm above
        // says of its own case.
        //
        // The clock is what an operator has to look at, so the row names it: a
        // stand-down declared for later is what a machine running ahead writes
        // when somebody declares one now.
        (Some(record), Some(now)) if now < record.declared_at => Health::broken(
            format!(
                "a stand-down here is declared to begin in {} minute(s), so it is not in force \
                 and writes are being adjudicated as though none had been declared \u{2014} this \
                 machine's clock was ahead when it was written, or the record came from one that \
                 was: {}",
                record.declared_at.saturating_sub(now).div_ceil(60),
                record.reason
            ),
            Resolution::run("estigia stand-down --lift --reason cleared"),
        ),
        _ => Health::Fine {
            detail: "the gate is not standing down".to_owned(),
        },
    };
    Check {
        name: "stand-down",
        about,
        health,
    }
}

/// Whether each agent's tool server would start.
///
/// The other half of [`gates`], and it had none. `exposes_tools` asks whether
/// an entry exists; nothing asked what it names. So an MCP server left pointing
/// at a binary that had moved — a `cargo clean`, an install from a checkout
/// since deleted, Estigia taken out and put back somewhere else — reported
/// `tools on` while every tool call failed, and `doctor` had no row for it at
/// all.
///
/// **Pure and fed**, like `gates`: the interesting case is an executable that
/// is not there, and a function that goes looking cannot be shown to handle it.
pub fn tool_servers(
    agents: &[(&'static str, Option<std::path::PathBuf>, Option<bool>)],
    running: Option<&std::path::Path>,
) -> Vec<Check> {
    agents
        .iter()
        .map(|(name, command, starts)| Check {
            name: "tools",
            about: "the server the agent reaches Estigia's own operations through",
            health: match command {
                // Not a fault. An agent may be gated and hold no tools, which
                // `setup --skill-only` and `skip_harness` both produce, and
                // reporting a deliberate choice as damage teaches people to
                // ignore the report.
                None => Health::Skipped {
                    detail: format!("{name}: no tool server registered"),
                },
                // Named and unstartable. The binary is there; the entry
                // does not say `mcp`, so the host runs it with no
                // subcommand, gets the usage and exit `2`, and every
                // operation the agent asks for fails behind a row that
                // said `running`.
                Some(path) if path.is_file() && *starts == Some(false) => Health::broken(
                    format!(
                        concat!(
                            "{}: the server names {} and never says `mcp`, so nothing ",
                            "starts \u{2014} the binary prints its usage and every ",
                            "operation the agent asks for fails"
                        ),
                        name,
                        path.display()
                    ),
                    Resolution::run("estigia setup --all"),
                ),
                Some(path) if path.is_file() => Health::Fine {
                    detail: format!(
                        "{name}: running {}{}",
                        path.display(),
                        other_build(path, running)
                    ),
                },
                Some(path) => Health::broken(
                    format!(
                        "{name}: the tool server names {}, which is not there \u{2014} every \
                         operation the agent asks for fails and nothing else says so",
                        path.display()
                    ),
                    // Registering again writes the path this binary is at.
                    Resolution::run(format!("estigia setup {name}")),
                ),
            },
        })
        .collect()
}

/// Whether the gate has been letting calls through without deciding on them.
///
/// The one check about the past rather than the present. Everything else here
/// asks whether a run *could* work; this asks whether the runs that already
/// happened were gated at all — because the two ways they are not are both
/// silent by construction. Nothing is denied, nothing is printed, and the only
/// trace is a ledger line nobody has a reason to open: an operator only reads
/// it after being stopped, and this is the case where they never were.
pub fn silence(found: &Ungated) -> Check {
    let about = "whether any call reached the gate and went undecided";
    // A file that will not open answers nothing, and saying "every call was
    // decided on" about it is the same false negative this check exists for.
    if let Some(why) = &found.unreadable {
        return Check {
            name: "silence",
            about,
            health: Health::broken(
                format!(
                    "the ledger cannot be read, so whether any call went ungated is unknown: {why}"
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "that file readable, or moved aside if its history is not wanted — \
                     until then this check has nothing to read",
                ),
            ),
        };
    }
    // Nothing has run yet. Not a clean bill of health, and not a fault either:
    // reporting either one teaches somebody to disbelieve this line later.
    if found.missing {
        return Check {
            name: "silence",
            about,
            health: Health::Skipped {
                detail: "no call has reached the gate yet — there is no ledger to read".to_owned(),
            },
        };
    }
    let unreadable_lines = match found.unparsed {
        0 => String::new(),
        n => format!(
            " — and {n} ledger line(s) could not be read, so this is a floor and not a count"
        ),
    };
    // Sessions that could not be identified. Not an ungated call, and not
    // nothing: with no payload there is no session id, so nothing can be sworn
    // in that session and the whole harness is inert for it. Reported here
    // because this is the one check that reads this ledger for hook faults, and
    // a record nobody reads is the defect this check exists against.
    let unminted = match found.unminted.len() {
        0 => String::new(),
        n => {
            let who = found
                .unminted
                .iter()
                .max_by_key(|(_, at)| *at)
                .map(|(agent, _)| agent.as_str())
                .filter(|agent| !agent.is_empty())
                .map_or_else(String::new, |agent| format!(" from {agent}"));
            format!(
                " — and {n} session hook(s){who} arrived with no readable payload, so those \
                 sessions carry no identity and nothing could be sworn in them"
            )
        }
    };
    if found.calls.is_empty() && !found.unminted.is_empty() && found.unparsed == 0 {
        return Check {
            name: "silence",
            about,
            health: Health::broken(
                format!("every call the ledger records was decided on{unminted}"),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "whether that agent's lifecycle hook is registered to send its payload — \
                     without one there is no session id, and a run id is derived from it",
                ),
            ),
        };
    }
    if found.calls.is_empty() {
        return Check {
            name: "silence",
            about,
            health: if found.unparsed == 0 {
                Health::Fine {
                    detail: "every call the ledger records was decided on".to_owned(),
                }
            } else {
                Health::broken(
                    format!(
                        "no ungated call was found, but {} ledger line(s) could not be read — \
                         this cannot say none went through",
                        found.unparsed
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        "those lines readable, or removed if they are damage rather than \
                         history — until then this check answers about the rest only",
                    ),
                )
            },
        };
    }
    let ungated = &found.calls;
    let count = |wanted: &str| {
        ungated
            .iter()
            .filter(|(verdict, _)| verdict == wanted)
            .count()
    };
    let last = ungated.iter().map(|(_, at)| *at).max().unwrap_or_default();
    // Which repair, not just how many: a payload this build cannot parse is a
    // schema to teach the classifier, one that never arrives is a registration
    // to fix, and one that parses and names no tool is an agent sending a shape
    // this build does not read. Counted apart so the message can say which.
    //
    // By verdict rather than by subtraction. `absent` was *everything that was
    // not unreadable*, so the day a third way to go undecided was recorded it
    // was reported as the second — a count that cannot be wrong about the total
    // and is wrong about the repair.
    let named: Vec<String> = [
        (
            "payload-unreadable",
            "arrived with a payload this build could not parse",
        ),
        ("payload-absent", "arrived with no payload at all"),
        (
            "tool-unnamed",
            "arrived with a payload that parses and names no tool",
        ),
        (
            "identity-unminted",
            "arrived with no readable payload to take an identity from",
        ),
    ]
    .into_iter()
    .filter_map(|(verdict, said)| {
        let found = count(verdict);
        (found > 0).then(|| format!("{found} {said}"))
    })
    .collect();
    let what = named.join(", and ");
    // What the last one said about itself, when it said anything. Lines written
    // by a build before there was one to say carry the bare sentence, and
    // repeating it under the count teaches nobody anything.
    let latest = found
        .latest
        .as_deref()
        .and_then(|detail| detail.split_once(": "))
        .map_or_else(String::new, |(_, why)| {
            format!("\n                      {why}")
        });
    Check {
        name: "silence",
        about: "whether any call reached the gate and went undecided",
        health: Health::broken(
            format!(
                "{} call(s) went through ungated, the most recent {} at {}: \
                 {what}{unreadable_lines}{unminted}{latest}",
                ungated.len(),
                // Named when the record names it. A line from a build that did
                // not keep this says nothing rather than guessing: there are
                // eleven agents and this check reports on all of them.
                found.source.as_deref().map_or_else(
                    || "from an agent no line names".to_owned(),
                    |who| format!("from {who}"),
                ),
                // Not the raw count of seconds. Five ungated calls yesterday and
                // five from last spring need opposite responses, and the number
                // told the two apart to nobody — measured on this crate's own
                // machine, where a reader took `1785904685` for old history and
                // it was the day before.
                crate::harness::session::stamp_of(last)
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "whether that hook is registered to send its payload, and \u{2014} for a payload \
                 this build could not parse \u{2014} whether that agent's schema is one this \
                 build knows. The line above names which agent and which hook, when the record \
                 does",
            ),
        ),
    }
}

#[cfg(test)]
mod tests {

    /// A remote whose path has a space in it is reported whole.
    ///
    /// `git remote -v` writes `<name>\t<url> (fetch)`, and reading it with
    /// `split_whitespace().nth(1)` returned the URL only while the URL had no
    /// space. Measured against a real checkout whose remote was
    /// `../el remoto con espacios.git`: the row read `remote  ../el`, which is
    /// the row answering *which repository holds the issues* with a path that
    /// is not one, and nothing in the line said it had been cut.
    ///
    /// `a_remote_with_a_space_in_it_is_named_whole` in `tests/pipe.rs` crosses
    /// the same thing through the binary against a remote git itself wrote.
    #[test]
    fn a_remote_url_is_read_off_the_tab_git_writes_and_not_off_the_first_space() {
        for (line, expected) in [
            (
                "origin\t../el remoto con espacios.git (fetch)",
                "../el remoto con espacios.git",
            ),
            (
                "origin\t../el remoto con espacios.git (push)",
                "../el remoto con espacios.git",
            ),
            (
                "origin\tC:/Users/Antonio Sanabria/repos/work.git (fetch)",
                "C:/Users/Antonio Sanabria/repos/work.git",
            ),
            // The ordinary one, unmoved.
            (
                "origin\tgit@github.com:asanabrial/estigia.git (fetch)",
                "git@github.com:asanabrial/estigia.git",
            ),
            // Only the kind git appends, and only where it appends it.
            (
                "origin\thttps://example.test/(fetch) (fetch)",
                "https://example.test/(fetch)",
            ),
            // A line with no tab is not one git wrote. Kept whole rather than
            // cut at a guess, because whoever reads it may know what it is.
            ("something else entirely", "something else entirely"),
        ] {
            assert_eq!(
                super::remote_url(line),
                expected,
                "the remote read out of {line:?} is not the one git named"
            );
        }
    }

    use super::*;

    /// A session that could not be identified is reported, and not as a silence.
    ///
    /// It was counted as a call that went through ungated, which is two errors
    /// at once: the number of ungated calls was wrong, and the operator was
    /// sent to look for a write nobody adjudicated when what happened was a
    /// session hook arriving empty. Counting it apart fixes the first; saying
    /// it out loud is what keeps the second from becoming a record nobody
    /// reads, which is the defect this whole check exists against.
    #[test]
    fn a_stand_down_this_build_will_not_honour_is_not_a_machine_with_none() {
        // The gate refuses a record declaring longer than the cap. Without a row
        // of its own that refusal is invisible: the operator reads *the gate is
        // not standing down* and everything looks normal, while every write is
        // adjudicated as though they had declared nothing. The unreadable arm
        // above exists to break exactly this silence, one state over.
        let now = crate::harness::session::now_seconds().expect("a clock");
        let stretched =
            crate::harness::standdown::Standing::Declared(crate::harness::standdown::StandDown {
                reason: "just a bit longer".to_owned(),
                declared_at: now,
                until: now + 24 * 60 * 60,
                declared_by: "operator".to_owned(),
            });
        let Health::Broken { detail, resolution } = standing_down(&stretched, Some(now)).health
        else {
            panic!("a stand-down this build ignores was reported as a machine with none");
        };
        assert!(
            detail.contains("1440 minute(s)") && detail.contains("240"),
            "the row did not say how long was asked for or how long is granted: {detail}"
        );
        assert!(
            detail.contains("just a bit longer"),
            "the reason is the one thing only the operator can recognise: {detail}"
        );
        // The ratchet: a resolution that names a command has to be one that
        // clears the row, and `--lift` removes the record.
        assert!(
            matches!(&resolution, Resolution::Run { command, .. } if command.contains("--lift")),
            "the row named no way out of itself: {resolution:?}"
        );

        // The floor, both ways. An honest stand-down still stands the gate down,
        // and an expired one is still quiet — it is kept on purpose as evidence,
        // and reporting every past stand-down as a fault is how a report gets
        // ignored.
        let honest = crate::harness::standdown::Standing::Declared(
            crate::harness::standdown::declare("something urgent", 30, now, "operator")
                .expect("a real one"),
        );
        assert!(matches!(
            standing_down(&honest, Some(now + 60)).health,
            Health::Skipped { .. }
        ));
        assert!(matches!(
            standing_down(&honest, Some(now + 3600)).health,
            Health::Fine { .. }
        ));
    }

    #[test]
    fn a_session_that_could_not_be_identified_is_reported_and_is_not_a_silence() {
        let found = Ungated {
            unminted: vec![("gemini-cli".to_owned(), 1_785_904_685)],
            ..Ungated::default()
        };
        let Health::Broken { detail, .. } = silence(&found).health else {
            panic!("a session nothing could identify is not reported at all");
        };
        assert!(
            detail.contains("gemini-cli"),
            "the report does not say whose hook arrived empty: {detail}"
        );
        assert!(
            detail.contains("every call the ledger records was decided on"),
            "an unidentified session is being counted among the ungated calls: {detail}"
        );

        // The floor: a ledger with nothing wrong in it is still reported fine,
        // so this is not a check that broke on every machine.
        assert!(
            matches!(
                silence(&Ungated {
                    calls: vec![("payload-absent".to_owned(), 1)],
                    ..Ungated::default()
                })
                .health,
                Health::Broken { .. }
            ),
            "a genuine ungated call stopped being reported"
        );
        assert!(
            matches!(silence(&Ungated::default()).health, Health::Fine { .. }),
            "a ledger with nothing wrong in it is being reported as faulty"
        );
        assert!(
            matches!(
                silence(&Ungated {
                    missing: true,
                    ..Ungated::default()
                })
                .health,
                Health::Skipped { .. }
            ),
            "a machine that has never run the gate is being reported as faulty"
        );
    }

    /// A check about the past does not say a run cannot swear now.
    ///
    /// `doctor` turns the first broken check into `environment-not-ready` with
    /// the sentence *"X is not usable, so a run cannot swear yet"*. For
    /// `silence` that is false — it reports on runs that already happened — and
    /// the ledger keeps its history, so the command exited non-zero forever on
    /// a machine where a run could swear. A readiness probe with a permanent
    /// false negative is one somebody switches off, and then the twelve checks
    /// that *are* about the present go unread with it.
    #[test]
    fn a_check_about_the_past_does_not_say_a_run_cannot_swear_now() {
        let broken = |name: &'static str| Check {
            name,
            about: "what it is for",
            health: Health::broken(
                "it is broken".to_owned(),
                crate::outcome::Resolution::run("estigia sync"),
            ),
        };
        assert!(
            first_blocking(&[broken("silence")]).is_none(),
            "a machine whose only fault is its history is being called not ready"
        );

        // The floor, both ways: a check about the present still stops a run,
        // and it does so even with a past-facing one broken in front of it.
        assert_eq!(
            first_blocking(&[broken("gh")]).map(|check| check.name),
            Some("gh"),
            "nothing stops a run any more, which is the opposite failure"
        );
        assert_eq!(
            first_blocking(&[broken("silence"), broken("gh")]).map(|check| check.name),
            Some("gh"),
            "a past-facing check in front of a present one hid it"
        );
    }

    /// Every name that looks back is one `doctor` can report.
    ///
    /// A rename would otherwise quietly turn the exemption off, and the check
    /// it exempts would go back to reporting a machine as not ready for its
    /// history — which is the failure above, restored by a typo.
    #[test]
    fn every_name_that_looks_back_is_one_doctor_reports() {
        for name in LOOKS_BACK {
            assert!(
                CHECKS.contains(name),
                "{name:?} looks back but is not a check this build runs"
            );
        }
    }

    /// The silence names which agent and which hook, and says when in words.
    ///
    /// Both halves were the same defect: the check reported a number of calls
    /// and a raw count of seconds, and then asked the operator to work out
    /// *which agent* — from a file nothing tells them to open. Five such lines
    /// on this crate's own machine read `most recently at 1785904685`, which a
    /// reader took for old history; it was the day before.
    #[test]
    fn the_silence_names_who_and_when_in_words_a_person_reads() {
        let found = Ungated {
            calls: vec![("payload-absent".to_owned(), 1_785_904_685)],
            source: Some("gemini-cli's pre-tool-use hook".to_owned()),
            ..Ungated::default()
        };
        let Health::Broken { detail, .. } = silence(&found).health else {
            panic!("an ungated call is no longer reported as broken");
        };
        assert!(
            detail.contains("2026-08-05T04:38:05Z"),
            "the report still dates the call in a way nobody reads: {detail}"
        );
        assert!(
            !detail.contains("1785904685"),
            "the raw count of seconds is still in the report: {detail}"
        );
        assert!(
            detail.contains("gemini-cli's pre-tool-use hook"),
            "the report does not say which agent or which hook: {detail}"
        );

        // And a line from a build that did not record it says so rather than
        // naming an agent it cannot know — there are eleven.
        let Health::Broken { detail, .. } = silence(&Ungated {
            source: None,
            ..found
        })
        .health
        else {
            panic!("an ungated call is no longer reported as broken");
        };
        assert!(
            detail.contains("no line names"),
            "an unattributed call is being attributed: {detail}"
        );
    }

    #[test]
    fn a_home_that_will_not_resolve_is_not_a_machine_where_nothing_went_ungated() {
        // The premise, pinned: an empty state root does not give an absolute
        // ledger path, it gives a relative one. That is what made defaulting
        // the path worse than imprecise — the reader went to the operator's
        // working directory and answered about whatever it found there.
        let nowhere = std::path::PathBuf::default();
        assert!(
            !crate::harness::session::ledger_path(&nowhere).is_absolute(),
            "an empty state root no longer yields a relative ledger path — \
             this test's premise has moved"
        );

        let refused = ledger_state(Err(crate::outcome::Refusal::not_started(
            "home-not-resolvable",
            "no HOME or USERPROFILE the process can read",
            Resolution::no_command(NoCommandReason::WorldAction, "a home this process can read"),
        )));
        assert!(
            refused.unreadable.is_some(),
            "a home that would not resolve was turned into a ledger holding nothing"
        );
        assert!(
            silence(&refused).health.is_broken(),
            "the one check about calls that went undecided decided it could tell, \
             on a machine it could not even find the ledger of"
        );
    }

    #[test]
    fn a_ledger_nothing_can_read_is_not_a_ledger_saying_nothing_happened() {
        // This check's whole subject is a failure that leaves no trace but one
        // ledger line. Every way of failing to read that file arrived here as
        // the same empty list, and came back out as "every call the ledger
        // records was decided on" — a clean bill of health issued by a reader
        // that read nothing. Driven through `ungated_calls` rather than the
        // pure function alone, because the losses were all in the reading.
        let root = tempfile::tempdir().expect("a temporary root");
        // A directory *under* the temporary one. `ledger_path` climbs to the
        // parent, so handing it the temporary root itself puts the ledger in
        // the shared system temp — somebody else's directory, and the place
        // where one run of this test then answered the next one.
        let state = root.path().join("state");
        let path = crate::harness::session::ledger_path(&state);

        // Nothing has run: not a fault, and not a clean bill either.
        let nothing = ungated_calls(&state);
        assert!(
            nothing.missing,
            "an absent ledger was not noticed as absent"
        );
        assert!(
            matches!(silence(&nothing).health, Health::Skipped { .. }),
            "a machine that has run nothing was given a clean bill of health"
        );

        // There and unopenable. A directory in its place fails the read with
        // something other than `NotFound`, on every platform.
        std::fs::create_dir_all(&path).expect("something unreadable in its place");
        let shut = ungated_calls(&state);
        assert!(
            shut.unreadable.is_some(),
            "an unreadable ledger read as empty"
        );
        assert!(
            silence(&shut).health.is_broken(),
            "a ledger nothing can open was reported as one holding nothing"
        );
        std::fs::remove_dir(&path).expect("clear the way for a real one");

        // A line nothing can parse, beside a good one. The answer is now a
        // floor, and saying otherwise is the same false negative.
        std::fs::write(&path, "{\"verdict\":\"allow\",\"at\":1}\ntorn\n").expect("a ledger");
        let torn = ungated_calls(&state);
        assert_eq!(torn.unparsed, 1, "a line nothing could read was dropped");
        assert!(
            silence(&torn).health.is_broken(),
            "an unreadable line was counted as a call that was decided on"
        );

        // And an ungated call still surfaces when its timestamp will not parse:
        // the verdict is the finding, the time only dresses the sentence.
        std::fs::write(&path, "{\"verdict\":\"payload-absent\"}\n").expect("a ledger");
        let undated = ungated_calls(&state);
        assert_eq!(
            undated.calls.len(),
            1,
            "a call that went ungated was dropped for having no usable timestamp"
        );
        assert!(silence(&undated).health.is_broken());

        // The other half, which this test's name always covered and its body
        // did not. The ledger is read in two pieces, and the rotated one is
        // where the records nearest the cap end up — the comment above the read
        // says so. It was the piece taken with `unwrap_or_default()`, so a
        // rotated half that exists and will not open became a half holding
        // nothing, and this check answered from the other one alone.
        //
        // The current half here is clean and readable, so the only thing that
        // can make the answer anything but "all decided" is the piece that
        // cannot be read.
        std::fs::write(&path, "{\"verdict\":\"allow\",\"at\":1}\n").expect("a clean ledger");
        let rotated = crate::harness::session::previous_ledger_path(&state);
        assert_ne!(
            rotated, path,
            "the two halves resolved to one file, so this measures nothing"
        );
        std::fs::create_dir_all(&rotated).expect("something unreadable in its place");
        let half = ungated_calls(&state);
        assert!(
            half.unreadable.is_some(),
            "a rotated half nothing can open read as a half holding nothing"
        );
        assert!(
            silence(&half).health.is_broken(),
            "the check answered from the half it could read and called the machine quiet"
        );
        std::fs::remove_dir(&rotated).expect("clear the way");

        // And absent really is ordinary: most machines never reach the cap, so
        // the fix must not turn every one of them into a fault.
        let usual = ungated_calls(&state);
        assert!(
            usual.unreadable.is_none() && usual.calls.is_empty(),
            "a machine that never rotated its ledger was reported as unreadable"
        );
    }

    #[test]
    fn a_gate_that_stood_aside_is_reported_by_the_one_thing_that_reads_the_ledger() {
        // Both ways a call goes undecided are silent by construction: nothing
        // is denied and nothing is printed. They leave a ledger line, and an
        // operator opens the ledger after being stopped — which is precisely
        // the case where they never were. So the line needed a reader.
        let quiet = silence(&Ungated::default());
        assert!(
            matches!(quiet.health, Health::Fine { .. }),
            "an empty ledger is not a fault"
        );

        // Which repair, not just how many: a payload this build cannot parse
        // is a schema to teach the classifier, and one that never arrives is a
        // registration to fix.
        let both = silence(&Ungated {
            calls: vec![
                ("payload-unreadable".to_owned(), 10),
                ("payload-absent".to_owned(), 20),
            ],
            ..Ungated::default()
        });
        let Health::Broken {
            detail, resolution, ..
        } = &both.health
        else {
            panic!("calls that went through ungated were reported fine");
        };
        assert!(detail.contains('2'), "how many is not said: {detail}");
        // The wording moved when a third way to go undecided was added: the
        // sentence is now built per verdict rather than by subtracting one
        // count from the total, which is what made the third one arrive
        // labelled as the second.
        assert!(
            detail.contains("could not parse") && detail.contains("no payload at all"),
            "the two are not told apart: {detail}"
        );
        assert!(
            detail.contains("20"),
            "when the last one was is not said: {detail}"
        );
        // No command: which agent sent them, and how its hook is registered,
        // is knowledge Estigia does not have.
        assert!(
            matches!(resolution, crate::outcome::Resolution::NoCommand { .. }),
            "a command was named for something only the operator can answer"
        );
    }

    #[test]
    fn a_contract_that_is_not_this_binarys_copy_is_reported_as_something_to_fix() {
        // `status` said "skill out of date" for all ten agents on the machine
        // this was written on, the gate said it again at every `SessionStart`,
        // and `doctor` — the command whose whole job is to answer *is my
        // environment right* — said `ok contract` eleven times.
        let root = tempfile::tempdir().expect("a temporary root");
        let path = root.path().join("SKILL.md");
        std::fs::write(&path, "# contract").expect("the contract");
        let drifted = contracts(&[Contract {
            agent: "claude-code",
            path: path.clone(),
            own_root: true,
            directive: None,
            directive_absent: None,
            stale: true,
            unreadable: None,
            ignored: Vec::new(),
            cut_short: Vec::new(),
            inert_for_this_tracker: Vec::new(),
            shadowed_local: None,
            duplicated: Vec::new(),
        }]);
        let Health::Broken { detail, resolution } = &drifted[0].health else {
            panic!("a contract the gate itself calls stale passed as fine");
        };
        assert!(
            detail.contains("claude-code") && detail.contains("not this binary's copy"),
            "the row does not say what is old: {detail}"
        );
        // The root rather than the contract: `presence` compares every file the
        // skill ships, and the one that differs is as often the transport. A
        // message naming `SKILL.md` sends the operator to read a current file.
        assert!(
            !detail.contains("SKILL.md") && detail.contains(&root.path().display().to_string()),
            "the row names a file that may not be the one that drifted: {detail}"
        );
        // A command, not a question. This is the whole reason it can be broken
        // rather than a note: Estigia owns the fix and can run it unattended.
        assert!(
            matches!(resolution, Resolution::Run { command, .. } if command.contains("sync")),
            "the one failure here with a command did not name it: {resolution:?}"
        );

        // And the same contract, current, is still fine — or every installation
        // reads as broken and the row teaches nobody anything.
        let current = contracts(&[Contract {
            agent: "claude-code",
            path,
            own_root: true,
            directive: None,
            directive_absent: None,
            stale: false,
            unreadable: None,
            ignored: Vec::new(),
            cut_short: Vec::new(),
            inert_for_this_tracker: Vec::new(),
            shadowed_local: None,
            duplicated: Vec::new(),
        }]);
        assert!(
            !current[0].health.is_broken(),
            "an up-to-date contract was reported broken: {:?}",
            current[0].health
        );
    }

    /// A row that names a binary other than the running one says so.
    ///
    /// The settings file names the path the agent will execute. `doctor`
    /// reported it as `running <path>` and stopped there, so an operator who had
    /// installed a second build, moved a profile, or upgraded to another prefix
    /// was told the gate was healthy while it ran a build they had not seen in a
    /// while — and, measured on the product, the `gate` and `tools` rows can name
    /// **two different binaries** with both reporting `ok`.
    ///
    /// The sibling of this drift was already said at session start — *the
    /// installed contract is not this binary's copy* — and this was the half
    /// nobody said. Not `Broken`: the gate does run, and a differing path is
    /// sometimes right. What it must not be is invisible.
    /// A skill that is not this binary's copy is reported as one.
    ///
    /// The row's own `about` says *the contract and the transport the agent
    /// reads*, and it reported the directory's path and checked nothing in it —
    /// while the `transport` row asked only whether `github.py` was a file. So a
    /// payload left by an older Estigia came back `ok` on both, with the binary
    /// and the script disagreeing about the flags between them: the failure
    /// `every_tool_sends_flags_the_transport_accepts` prevents against the
    /// *shipped* transport, arriving through the version axis instead.
    ///
    /// The comparison already existed and was already trusted — the session
    /// start tells the **agent** the contract is not this binary's copy. The
    /// operator asking whether their machine is right was the one nobody told.
    #[test]
    fn a_skill_that_is_not_this_binarys_copy_is_reported_as_one() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let skill = root.path().join(crate::skill::DIRECTORY);
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");

        let fresh = examine(
            Some(&skill),
            root.path(),
            &crate::config::Tracker::Github { repo: None },
        );
        let row = fresh
            .iter()
            .find(|check| check.name == "skill")
            .expect("the skill is checked");
        assert!(
            !row.health.is_broken(),
            "a freshly installed skill was reported as drifted: {:?}",
            row.health
        );

        // The binding, now that the transport is retired and no longer
        // shipped. It is the right stand-in for the same reason the
        // transport was: it is what the agent reads to know which
        // operations exist and what they take, so an older copy beside a
        // newer binary is a contract that has moved under its reader.
        let drifting = skill.join("bindings").join("github.md");
        let mut text = std::fs::read_to_string(&drifting).expect("the binding is installed");
        text.push_str(
            "
<!-- left behind by an older Estigia -->
",
        );
        std::fs::write(&drifting, text).expect("an older copy");

        let drifted = examine(
            Some(&skill),
            root.path(),
            &crate::config::Tracker::Github { repo: None },
        );
        let row = drifted
            .iter()
            .find(|check| check.name == "skill")
            .expect("the skill is checked");
        let Health::Broken { detail, resolution } = &row.health else {
            panic!(
                "a skill that is not this binary's copy was reported as fine: {:?}",
                row.health
            );
        };
        assert!(
            detail.contains("not this binary's copy"),
            "the row does not say what is wrong: {detail}"
        );
        // A command that discharges it, verified on the product: `sync`
        // rewrites the payload and the row goes back to `ok`.
        assert!(
            format!("{resolution:?}").contains("estigia sync"),
            "the row says the skill drifted and not how to end it: {resolution:?}"
        );
    }

    #[test]
    fn a_row_naming_another_build_says_it_is_another_build() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let registered = root.path().join("estigia-registered");
        let running = root.path().join("estigia-running");
        std::fs::write(&registered, "#!/bin/sh\n").expect("a binary to name");
        std::fs::write(&running, "#!/bin/sh\n").expect("a binary to be");

        let checks = tool_servers(
            &[("claude-code", Some(registered.clone()), None)],
            Some(running.as_path()),
        );
        let Health::Fine { detail } = &checks[0].health else {
            panic!(
                "a server that is there was not reported as running: {:?}",
                checks[0].health
            );
        };
        assert!(
            detail.contains("not this build"),
            "the row names a binary that is not the running one and does not say so: {detail}"
        );
        assert!(
            detail.contains("estigia setup"),
            "the row says the builds differ and not how to make them agree: {detail}"
        );

        // The ordinary case stays quiet, or the note is noise on every machine.
        let checks = tool_servers(
            &[("claude-code", Some(registered.clone()), None)],
            Some(registered.as_path()),
        );
        let Health::Fine { detail } = &checks[0].health else {
            panic!("the same binary was not reported as running");
        };
        assert!(
            !detail.contains("not this build"),
            "a row naming the running binary was reported as drift: {detail}"
        );

        // And nothing invented when the running binary cannot be resolved.
        let checks = tool_servers(&[("claude-code", Some(registered), None)], None);
        let Health::Fine { detail } = &checks[0].health else {
            panic!("an unresolved current build changed the verdict");
        };
        assert!(!detail.contains("not this build"), "{detail}");
    }

    /// A server registration that never says `mcp` starts nothing, and says so.
    ///
    /// The row read the `command` and stopped there, so an entry whose `args`
    /// no longer carry the subcommand reported `running <path>` — while the
    /// host ran a binary that prints its usage and exits `2`, and every
    /// operation the agent asked for failed. Measured on the product: emptying
    /// `args` left the row saying `ok`.
    ///
    /// The same shape the `gate` row had one round earlier, with the arguments
    /// in place of the matcher: both promise the agent reaches Estigia, and
    /// both were checking that a file exists rather than that the entry works.
    #[test]
    fn a_server_that_never_says_mcp_is_not_reported_as_running() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let binary = root.path().join("estigia");
        std::fs::write(&binary, "#!/bin/sh\n").expect("a binary to name");

        let unstartable = tool_servers(&[("claude-code", Some(binary.clone()), Some(false))], None);
        let Health::Broken { detail, resolution } = &unstartable[0].health else {
            panic!(
                "an entry that starts nothing was reported as running: {:?}",
                unstartable[0].health
            );
        };
        assert!(detail.contains("never says `mcp`"), "{detail}");
        assert!(
            format!("{resolution:?}").contains("estigia setup"),
            "the row says nothing starts and not how to fix it: {resolution:?}"
        );

        // The two quiet answers. `Some(true)` is an entry that says how to
        // start the server, and `None` is one this reader could not take apart
        // — an alarm invented from a failed read is worse than none.
        for reading in [Some(true), None] {
            let checks = tool_servers(&[("claude-code", Some(binary.clone()), reading)], None);
            assert!(
                !checks[0].health.is_broken(),
                "a server was reported as unstartable on a reading of {reading:?}"
            );
        }
    }

    #[test]
    fn a_tool_server_naming_a_binary_that_is_gone_is_not_reported_as_running() {
        // `gates` exists because a gate can be *registered* and point at a
        // binary that is not there — "installed, looks installed, enforces
        // nothing". The tools half of the same sentence had no check at all:
        // `exposes_tools` asks whether an entry exists and never what it names,
        // and `doctor` had no row for it.
        //
        // Measured: with the MCP entry pointed at a path that does not exist,
        // `estigia status` still answered `harness: gate on, tools on`.
        let root = tempfile::tempdir().expect("a temporary root");
        let real = root.path().join("estigia.exe");
        std::fs::write(&real, "#!/bin/sh\n").expect("a binary to name");

        let checks = tool_servers(
            &[
                ("claude-code", Some(real.clone()), None),
                ("codex", Some(root.path().join("moved-away.exe")), None),
                ("cursor", None, None),
            ],
            None,
        );
        assert_eq!(checks.len(), 3);
        assert!(checks.iter().all(|check| check.name == "tools"));

        let Health::Fine { detail } = &checks[0].health else {
            panic!(
                "a server that is there was not reported: {:?}",
                checks[0].health
            );
        };
        assert!(detail.contains(&real.display().to_string()), "{detail}");

        let Health::Broken { detail, resolution } = &checks[1].health else {
            panic!("a server naming nothing passed: {:?}", checks[1].health);
        };
        assert!(
            detail.contains("moved-away"),
            "it does not name what is missing: {detail}"
        );
        // A command, because registering again writes the path this binary is
        // at — Estigia owns the fix.
        assert!(
            matches!(resolution, Resolution::Run { command, .. } if command.contains("codex")),
            "{resolution:?}"
        );

        // No entry is not damage: `--skill-only` and `skip_harness` both produce
        // an agent with a contract and no tools, deliberately.
        assert!(
            matches!(checks[2].health, Health::Skipped { .. }),
            "an agent that was never given tools was reported broken: {:?}",
            checks[2].health
        );
    }

    #[test]
    fn full_reports_the_home_it_was_given_and_softens_the_push_guard_there() {
        // Through `full`, not through `amend_push_guard`. *A test of the reader
        // is not a test that anything calls it* — measured: deleting the call
        // from `full` left the whole suite green, which is the shape this file
        // has already been caught by once.
        //
        // And it could not be written at all until `full` stopped asking for
        // `state_root(None)`. Three rows — the stand-down, the run pointers and
        // the ledger's silence — read the real machine's home while `full` held
        // an explicit `home_dir` its own caller had set. The test below would
        // have been answered about the operator running the suite.
        let home = tempfile::tempdir().expect("a temporary home");
        let repo = tempfile::tempdir().expect("a temporary repository");
        // A throwaway checkout: the push guard lives in `.git/hooks`, so there
        // is no way to report on it without one.
        let git = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["init", "--quiet"])
            .status();
        if !matches!(git, Ok(status) if status.success()) {
            // No git on this machine: the row under test cannot exist, and
            // asserting about it would be measuring the absence of git.
            return;
        }
        crate::harness::guard::install(repo.path(), Path::new("estigia"), false)
            .expect("the push guard installs");

        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            ..crate::setup::SetupOptions::default()
        };
        // A skill root, because `examine` stops at the contract without one and
        // never reaches the row under test.
        let adapter = crate::setup::find_agent("claude-code").expect("a declared agent");
        crate::setup::setup(adapter, &crate::config::Config::default(), &options)
            .expect("setup runs");
        let paths = crate::setup::resolve_paths(adapter, &options).expect("paths resolve");
        let skill = Some(paths.skill_root.as_path());
        let tracker = crate::config::Tracker::Github { repo: None };
        let row = |checks: &[Check]| {
            checks
                .iter()
                .find(|check| check.name == "push-guard")
                .map(|check| check.health.clone())
                .expect("the push-guard row is on the report")
        };

        // With no stand-down in that home, the guard promises what it does.
        let quiet = full(skill, repo.path(), &tracker, &options);
        assert!(
            matches!(row(&quiet), Health::Fine { .. }),
            "the guard was installed and the report does not say so: {:?}",
            row(&quiet)
        );

        // Now declare one in *this* home, at the real clock `full` reads.
        let now = crate::harness::session::now_seconds().expect("a readable clock");
        let record =
            crate::harness::standdown::declare("a migration by hand", 5, now, "asanabrial")
                .expect("a stand-down with a reason and a window");
        let state_root =
            crate::harness::session::state_root(Some(home.path())).expect("a state root");
        let file = crate::harness::standdown::path(&state_root);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("somewhere to put it");
        }
        std::fs::write(
            &file,
            serde_json::to_string(&record).expect("a stand-down serialises"),
        )
        .expect("the stand-down is written");

        let open = full(skill, repo.path(), &tracker, &options);
        let Health::Skipped { detail } = row(&open) else {
            panic!(
                "the report still promises a push is refused while the gate is open: {:?}",
                row(&open)
            );
        };
        assert!(
            detail.contains("standing down"),
            "the row does not say what suspended it: {detail}"
        );
        // The row that exists to say the gate is open agrees with it, which is
        // the whole point: they were three lines apart saying opposite things.
        assert!(
            open.iter().any(|check| check.name == "stand-down"
                && matches!(&check.health, Health::Skipped { detail }
                    if detail.contains("a migration by hand"))),
            "the two rows do not agree about the same moment"
        );
    }

    #[test]
    fn a_directive_that_is_not_this_binarys_is_on_the_report() {
        // The directive is the one text **every session loads**, and it had no
        // check: `skill::presence` compares the files under the skill root, and
        // this lives in the agent's own instruction file. Measured by rewriting
        // the first of its three rules inside Estigia's own fence, from *a
        // claim is adjudicated, not asserted* to *a claim is whatever you say
        // it is* — after which `status` said `configured` and this row said
        // `verified`.
        //
        // Through `full`, not through `setup::directive_is_current`. A test of
        // the reader is not a test that anything calls it, which is how the
        // plugin reader of the round before this one passed while `registered`
        // ignored it.
        let home = tempfile::tempdir().expect("a temporary home");
        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            ..crate::setup::SetupOptions::default()
        };
        let adapter = crate::setup::find_agent("claude-code").expect("a declared agent");
        crate::setup::setup(adapter, &crate::config::Config::default(), &options)
            .expect("setup runs");
        let paths = crate::setup::resolve_paths(adapter, &options).expect("paths resolve");

        let contracts = |options: &crate::setup::SetupOptions| {
            full(
                Some(&paths.skill_root),
                home.path(),
                &crate::config::Tracker::Github { repo: None },
                options,
            )
            .into_iter()
            .filter(|check| check.name == "contract")
            .collect::<Vec<_>>()
        };

        assert!(
            contracts(&options)
                .iter()
                .all(|check| !check.health.is_broken()),
            "a fresh install was reported broken: {:?}",
            contracts(&options)
        );

        let text = std::fs::read_to_string(&paths.instructions).expect("the directive is there");
        let tampered = text.replace("adjudicated, not asserted", "whatever you say it is");
        assert_ne!(tampered, text, "the fixture did not change the directive");
        std::fs::write(&paths.instructions, &tampered).expect("their file");

        let broken: Vec<Health> = contracts(&options)
            .into_iter()
            .map(|check| check.health)
            .filter(Health::is_broken)
            .collect();
        assert!(
            !broken.is_empty(),
            "the rules every session loads were rewritten and nothing said so"
        );
        let Health::Broken { detail, resolution } = &broken[0] else {
            unreachable!("filtered above");
        };
        // The instruction file, not `SKILL.md`: round 61 learned that naming
        // the contract when something else drifted sends an operator to read a
        // file that is already current.
        assert!(
            detail.contains(&paths.instructions.display().to_string()),
            "the row does not name the file that moved: {detail}"
        );
        // And a command, because `sync` rewrites the directive — measured, not
        // assumed: running it puts the rule back.
        assert!(
            matches!(resolution, Resolution::Run { command, .. } if command.contains("sync")),
            "{resolution:?}"
        );

        // And the state one step worse, which this row did not have: a
        // directive that is **gone**. `directive_is_current` answers `None` for
        // both a file nothing can read and a file with no block in it, and this
        // row reported only its `Some(false)` — so drifted rules were on the
        // report and *no rules at all* were not.
        //
        // Measured on the installed binary: replacing `~/.claude/CLAUDE.md`
        // with an operator's own text left `doctor` saying `ok contract
        // claude-code … verified` while `status` on the same machine said
        // "skill present, directive missing". Two commands, one machine,
        // different answers — the shape this file's own prose says it has found
        // three times.
        std::fs::write(&paths.instructions, "# Mine\n\nDo it my way.\n").expect("their file");
        let gone: Vec<Health> = contracts(&options)
            .into_iter()
            .map(|check| check.health)
            .filter(Health::is_broken)
            .collect();
        let Some(Health::Broken { detail, resolution }) = gone.first() else {
            panic!("the rules every session loads are gone and nothing said so: {gone:?}");
        };
        assert!(
            detail.contains("holds no directive"),
            "an absent directive is reported as a drifted one: {detail}"
        );
        assert!(
            detail.contains(&paths.instructions.display().to_string()),
            "the row does not name the file that holds none: {detail}"
        );
        // `setup`, not `sync`: there is no block here to bring up to date, and
        // naming a command that does something else is the dead end the ratchet
        // forbids.
        assert!(
            matches!(resolution, Resolution::Run { command, .. } if command.contains("setup")),
            "{resolution:?}"
        );

        // The floor, and it is the whole of why the check is conditional: with
        // no skill installed there is nothing for a directive to point at, and
        // an instruction file without a block is somebody who declined rather
        // than a fault. Reporting a deliberate choice as damage teaches people
        // to ignore the report, which is the one thing a health report cannot
        // survive.
        //
        // Measured on this same agent with its skill taken away, because an
        // adapter nobody set up is not in this list at all — `full` walks the
        // agents that are *present*, so a second agent could not tell the two
        // conditions apart.
        std::fs::remove_file(paths.skill_root.join(crate::skill::CONTRACT))
            .expect("the skill was installed above");
        assert!(
            contracts(&options).iter().all(|check| match &check.health {
                Health::Broken { detail, .. } => !detail.contains("holds no directive"),
                _ => true,
            }),
            "an agent with no skill installed was reported for holding no directive: {:?}",
            contracts(&options)
        );
    }

    /// Builds the push-guard row as `examine` emits it when the hook is in.
    fn push_guard_row() -> Vec<Check> {
        vec![
            Check {
                name: "push-guard",
                about: "whether a push from this checkout is adjudicated",
                health: Health::Fine {
                    detail: "installed \u{2014} a push from a checkout a live claim holds is \
                             refused unless that claim justifies it"
                        .to_owned(),
                },
            },
            Check {
                name: "contract",
                about: "something else entirely",
                health: Health::Fine {
                    detail: "up to date".to_owned(),
                },
            },
        ]
    }

    #[test]
    fn the_push_guard_stops_promising_a_refusal_a_stand_down_has_suspended() {
        // Found by running it: `estigia stand-down --reason … --minutes 5`, then
        // `doctor`, which printed these two rows three lines apart —
        //
        //   ok       push-guard  … a push … is refused unless that claim justifies it
        //   skipped  stand-down  … writes go through unadjudicated until it expires
        //
        // Measured alongside: during that stand-down the gate answered `allow` to
        // `git push`, `gh pr merge 12` and `git tag v1.0`, and the pre-push hook
        // exited 0. The row whose entire subject is that push was the one still
        // saying `ok`, and an operator reads this report *before* pushing.
        let record = crate::harness::standdown::StandDown {
            reason: "a migration by hand".to_owned(),
            declared_at: 1_000,
            until: 1_300,
            declared_by: "asanabrial".to_owned(),
        };

        let mut checks = push_guard_row();
        amend_push_guard(
            &mut checks,
            &crate::harness::standdown::Standing::Declared(record.clone()),
            Some(1_000),
        );
        let Health::Skipped { detail } = &checks[0].health else {
            panic!(
                "the push guard still promises a refusal that is not happening: {:?}",
                checks[0].health
            );
        };
        assert!(
            detail.contains("standing down"),
            "the row does not say why it stopped promising: {detail}"
        );
        // The promise it *would* make is still there. An operator has to be able
        // to read what comes back when the stand-down expires.
        assert!(
            detail.contains("installed"),
            "the row lost what the guard is for: {detail}"
        );
        // Nothing else on the page is touched. The stand-down suspends the push
        // guard, not the contract check beside it.
        assert!(
            matches!(&checks[1].health, Health::Fine { detail } if detail == "up to date"),
            "an unrelated row was softened: {:?}",
            checks[1].health
        );

        // Expired is not standing down, on the same second the gate itself stops
        // honouring one: the guard is refusing again, so the row promises again.
        let mut expired = push_guard_row();
        amend_push_guard(
            &mut expired,
            &crate::harness::standdown::Standing::Declared(record),
            Some(1_300),
        );
        assert!(
            matches!(expired[0].health, Health::Fine { .. }),
            "an expired stand-down softened the row: {:?}",
            expired[0].health
        );

        // And an unreadable one is not honoured by the gate either — it is
        // treated as absent on purpose, so the push guard really is still
        // refusing. Softening the row here would trade one false sentence for
        // another, and `standing_down` says the file is broken on its own row.
        let mut unreadable = push_guard_row();
        amend_push_guard(
            &mut unreadable,
            &crate::harness::standdown::Standing::Unreadable("not json".to_owned()),
            Some(1_000),
        );
        assert!(
            matches!(unreadable[0].health, Health::Fine { .. }),
            "a stand-down nothing honours softened the row: {:?}",
            unreadable[0].health
        );

        // No stand-down at all: untouched, which is almost every machine.
        let mut away = push_guard_row();
        amend_push_guard(
            &mut away,
            &crate::harness::standdown::Standing::Away,
            Some(1_000),
        );
        assert!(
            matches!(away[0].health, Health::Fine { .. }),
            "the row was softened with no stand-down anywhere: {:?}",
            away[0].health
        );
    }

    #[test]
    fn an_open_gate_is_on_the_report_while_it_is_open() {
        // Found by running it: `estigia stand-down --reason … --minutes 5`, then
        // `doctor`, which answered `ok` down the whole page. Neither this nor
        // `status` mentioned that the gate was open — machine-wide, for up to four
        // hours, from the one command whose stated job is *what has to be true
        // before a run can swear to anything*.
        let record = crate::harness::standdown::StandDown {
            reason: "rotating a credential".to_owned(),
            declared_at: 1_000,
            until: 1_300,
            declared_by: "asanabrial".to_owned(),
        };

        let declared = crate::harness::standdown::Standing::Declared(record.clone());
        let away = crate::harness::standdown::Standing::Away;
        let open = standing_down(&declared, Some(1_000));
        let Health::Skipped { detail } = &open.health else {
            panic!("an open gate was not on the report: {:?}", open.health);
        };
        // Everything an operator needs to decide whether to lift it: how long is
        // left, who opened it, and the reason they gave.
        assert!(
            detail.contains('5'),
            "how much longer is not said: {detail}"
        );
        assert!(detail.contains("asanabrial"), "who is not said: {detail}");
        assert!(
            detail.contains("rotating a credential"),
            "why is not said: {detail}"
        );

        // Expired is closed. Exclusive at the far end, the same second the gate
        // itself stops honouring one.
        assert!(!matches!(
            standing_down(&declared, Some(1_300)).health,
            Health::Skipped { .. }
        ));
        assert!(!matches!(
            standing_down(&away, Some(1_000)).health,
            Health::Skipped { .. }
        ));

        // A record and no clock is not "no stand-down". `gate` refuses to honour
        // one it cannot time, and answering `ok` here would be this command
        // reporting on a question it could not ask.
        let untimed = standing_down(&declared, None);
        let Health::Skipped { detail } = &untimed.health else {
            panic!(
                "a stand-down nothing could time read as none: {:?}",
                untimed.health
            );
        };
        assert!(detail.contains("unknown"), "{detail}");

        // Never `Broken`: an operator stands the gate down in order to get work
        // done, and a check that refused them for it is one they would take out.
        for now in [Some(1_000), Some(1_300), None] {
            assert!(
                !standing_down(&declared, now).health.is_broken(),
                "a deliberate, bounded stand-down was reported as a broken machine"
            );
        }

        // A file that is there and answers nothing, driven through the read
        // rather than the pure function: the loss was in the reading, where two
        // `.ok()`s in a row turned both "no file" and "a file nothing can open"
        // into the same `None`, and this said `ok — the gate is not standing
        // down` about a file it had never read.
        //
        // Broken and not skipped, unlike a declared one: the operator did not
        // choose this, the gate is not honouring it, and nothing else on the
        // machine will ever mention the file.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        std::fs::create_dir_all(&state).expect("a state root");
        let file = crate::harness::standdown::path(&state);

        assert_eq!(
            crate::harness::standdown::standing(&state),
            crate::harness::standdown::Standing::Away,
            "a machine with no stand-down was not simply away"
        );

        // A directory in its place fails the read with something other than
        // `NotFound`, on every platform.
        std::fs::create_dir_all(&file).expect("something unreadable in its place");
        let shut = crate::harness::standdown::standing(&state);
        assert!(
            matches!(shut, crate::harness::standdown::Standing::Unreadable(_)),
            "a stand-down file nothing can open read as no stand-down at all"
        );
        assert!(
            standing_down(&shut, Some(1_000)).health.is_broken(),
            "the one check about whether the gate is lowered answered about a file it could \
             not read"
        );
        std::fs::remove_dir(&file).expect("clear the way");

        // And one that opens and says nothing a parser knows. Same answer: the
        // gate will not honour it either, and the difference between the two
        // failures is not the operator's to care about.
        std::fs::write(&file, "not json at all").expect("a corrupt record");
        let torn = crate::harness::standdown::standing(&state);
        assert!(
            matches!(torn, crate::harness::standdown::Standing::Unreadable(_)),
            "a stand-down nothing can parse read as no stand-down at all"
        );
        assert!(standing_down(&torn, Some(1_000)).health.is_broken());

        // A real one still reads, or the three states have collapsed the other
        // way and every machine is now broken.
        std::fs::write(
            &file,
            serde_json::to_string(&record).expect("the record serialises"),
        )
        .expect("a stand-down");
        assert_eq!(
            crate::harness::standdown::standing(&state),
            crate::harness::standdown::Standing::Declared(record),
            "a readable stand-down stopped being read"
        );
    }

    #[test]
    fn a_run_pointer_nobody_can_read_is_a_broken_machine_and_doctor_says_so() {
        // The state this was found in, on the product: a pointer truncated
        // mid-write, the gate refusing every write from that run by name, the
        // push guard refusing a push from its checkout, `status` naming the
        // file — and `doctor` reporting `ok` on all eleven rows. Two commands
        // describing one machine and disagreeing, which is the shape the
        // `status` half was added to end and ended on one side only.
        let clean = run_pointers(&[]);
        assert_eq!(clean.name, "run-pointer");
        assert!(
            matches!(clean.health, Health::Fine { .. }),
            "a machine with nothing broken was reported as something else"
        );

        let found = run_pointers(&["/home/a/.estigia/runs/claude-a.json".to_owned()]);
        let Health::Broken { detail, resolution } = &found.health else {
            panic!(
                "an unreadable pointer was not reported as broken: {:?}",
                found.health
            );
        };
        // Broken and not skipped: a stand-down is a loosening somebody chose
        // and can wait out; this is a fault nobody chose, and waiting does not
        // fix it.
        assert!(
            detail.contains("claude-a.json"),
            "the check does not name the file an operator has to go and look at: {detail}"
        );
        assert!(
            detail.contains("refused"),
            "the check does not say what the machine is doing about it: {detail}"
        );
        // A resolution, because a broken row without one is the dead end the
        // ratchet forbids — and no command of Estigia's settles this.
        assert!(
            !format!("{resolution:?}").is_empty(),
            "a broken check with nothing to do about it"
        );
    }

    fn install_contract(skill_root: &Path) {
        std::fs::create_dir_all(skill_root).expect("a skill root");
        std::fs::write(
            skill_root.join(crate::skill::CONTRACT),
            "the contract this row reads\n",
        )
        .expect("the contract is installed");
    }

    fn holder(run_id: &str, issue: u64, repo_dir: &Path) -> super::super::session::Run {
        let mut run = super::super::session::Run::new(run_id.to_owned());
        run.issue = Some(issue);
        run.repo_dir = Some(repo_dir.to_path_buf());
        run
    }

    /// No pointers at all: nothing stale, nothing unread.
    #[test]
    fn no_holdings_is_reported_fine() {
        let root = tempfile::tempdir().expect("a temporary root");
        let check = stale_run_pointers(Some(root.path()), root.path(), &github(), &[]);
        assert_eq!(check.name, "stale-run-pointer");
        assert!(
            matches!(check.health, Health::Fine { .. }),
            "a machine holding nothing was reported as something else: {:?}",
            check.health
        );
    }

    /// A pointer that parses cleanly and names an issue the tracker reports
    /// closed is the shape issue #90 was filed against: readable, and still
    /// gating a checkout nobody holds any more.
    #[test]
    fn a_readable_stale_pointer_is_reported_and_names_the_release_command() {
        let root = tempfile::tempdir().expect("a temporary root");
        install_contract(root.path());
        let run = holder("claude-stale0000", 12, root.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(root.path()), root.path(), &github(), &[run])
        });

        let Health::Broken { detail, resolution } = &check.health else {
            panic!(
                "a stale pointer was not reported broken: {:?}",
                check.health
            );
        };
        assert!(
            detail.contains("claude-stale0000") && detail.contains("#12"),
            "the row does not name the stale pointer: {detail}"
        );
        assert!(
            resolution.to_string().contains("estigia release --run-id"),
            "the row does not name the command that clears it: {resolution}"
        );
    }

    /// An issue that is open, whatever else it disagrees about, is not
    /// reported as stale.
    #[test]
    fn a_readable_live_pointer_is_reported_fine() {
        let root = tempfile::tempdir().expect("a temporary root");
        install_contract(root.path());
        let run = holder("claude-live00000", 34, root.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::open_but_unmatched_issue(34)]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(root.path()), root.path(), &github(), &[run])
        });

        assert!(
            matches!(check.health, Health::Fine { .. }),
            "a pointer whose issue is open was reported stale: {:?}",
            check.health
        );
    }

    /// A tracker read that fails answers nothing about staleness — reported
    /// separately from both a stale pointer and a clean one, never folded into
    /// either.
    #[test]
    fn a_tracker_read_that_fails_is_reported_as_unread_not_stale() {
        let root = tempfile::tempdir().expect("a temporary root");
        install_contract(root.path());
        let run = holder("claude-unknown00", 56, root.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(root.path()), root.path(), &github(), &[run])
        });

        let Health::Broken { detail, .. } = &check.health else {
            panic!(
                "a pointer this could not ask about was reported fine, which reads a read that \
                 failed as an issue that is open: {:?}",
                check.health
            );
        };
        assert!(
            !detail.contains("closed"),
            "a read that failed was reported as though the tracker said the issue was closed: \
             {detail}"
        );
        assert!(
            detail.contains("could not be checked"),
            "a read that failed does not say so: {detail}"
        );
    }

    /// A tracker with no executable transport is skipped rather than asked
    /// once per pointer — the same distinction `transport`'s own row draws.
    #[test]
    fn a_tracker_with_no_executable_is_skipped_for_stale_pointers_too() {
        let root = tempfile::tempdir().expect("a temporary root");
        let run = holder("claude-anything0", 1, root.path());
        let check = stale_run_pointers(
            Some(root.path()),
            root.path(),
            &crate::config::Tracker::Linear,
            &[run],
        );
        assert!(
            matches!(check.health, Health::Fine { .. }),
            "a tracker Estigia holds no tools for was asked about a pointer's issue: {:?}",
            check.health
        );
    }

    #[test]
    fn the_silence_says_what_the_last_undecided_call_said_about_itself() {
        // The counts say *which repair*. This says *which agent* — and it is
        // the half that was missing while four such lines sat on this session's
        // own machine, unexplained, with `doctor` asking the operator the
        // question the payload had already answered.
        let told = silence(&Ungated {
            calls: vec![("payload-unreadable".to_owned(), 30)],
            latest: Some(
                "a payload arrived and could not be parsed, so this call was not gated: \
                 valid JSON this build does not know, whose top-level keys are [hook_event, \
                 tool_call]; the read stopped at line 1 column 44 (Data)"
                    .to_owned(),
            ),
            ..Ungated::default()
        });
        let Health::Broken { detail, .. } = &told.health else {
            panic!("reported fine");
        };
        assert!(
            detail.contains("hook_event") && detail.contains("column 44"),
            "the reason the last call was not gated is not in the check: {detail}"
        );

        // And a line from a build that had nothing to say still reads as one
        // sentence. Half of any real ledger is older than this feature, and a
        // check that appended an empty reason to every one of them would be
        // noise on exactly the machines that already have a problem.
        let bare = silence(&Ungated {
            calls: vec![("payload-unreadable".to_owned(), 30)],
            latest: Some(
                "a payload arrived and could not be parsed, so this call was not gated".to_owned(),
            ),
            ..Ungated::default()
        });
        let Health::Broken { detail, .. } = &bare.health else {
            panic!("reported fine");
        };
        assert!(
            !detail.contains('\n'),
            "an old line with nothing to add still grew a second line: {detail:?}"
        );
    }

    #[test]
    fn the_reason_is_read_off_the_ledger_and_not_merely_carried() {
        // The check above hands `latest` in. Nothing yet says the reader takes
        // it *out of the file*, which is how a field ends up declared, consumed
        // and never written — the shape this crate has already found four times.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        std::fs::create_dir_all(&state).expect("the state directory");
        std::fs::write(
            crate::harness::session::ledger_path(&state),
            "{\"at\":10,\"verdict\":\"payload-unreadable\",\"detail\":\"x: an older reason\"}\n\
             {\"at\":40,\"verdict\":\"payload-unreadable\",\"detail\":\"x: the newest reason\"}\n\
             {\"at\":20,\"verdict\":\"payload-absent\",\"detail\":\"x: a middling reason\"}\n",
        )
        .expect("a ledger to read");

        let found = ungated_calls(&state);
        assert_eq!(found.calls.len(), 3, "not every undecided call was read");
        // The newest by `at`, not the last line: runs append concurrently.
        assert_eq!(
            found.latest.as_deref(),
            Some("x: the newest reason"),
            "the reason read back is not the most recent one"
        );

        // One kind alone says only that kind.
        let absent = silence(&Ungated {
            calls: vec![("payload-absent".to_owned(), 5)],
            ..Ungated::default()
        });
        let Health::Broken { detail, .. } = &absent.health else {
            panic!("reported fine");
        };
        assert!(
            !detail.contains("could not be parsed"),
            "a fault nobody hit was reported: {detail}"
        );
    }

    fn github() -> crate::config::Tracker {
        crate::config::Tracker::Github { repo: None }
    }

    #[test]
    fn a_gate_that_is_registered_and_would_not_run_is_reported_broken() {
        use crate::setup::wiring::Wire;

        // The failure this exists for. `status` says `gate on` for any agent
        // whose settings file carries an entry — and the entry on the machine
        // this was written on named a debug build inside a working tree.
        let root = tempfile::tempdir().expect("a temporary root");
        let file = root.path().join("settings.json");
        let gone = root.path().join("estigia.exe");
        let dead = Wire {
            command: format!("{} hook pre-tool-use", gone.display()),
            executable: gone.clone(),
            event: Some("pre-tool-use"),
            named: "pre-tool-use".to_owned(),
        };
        let checks = gates(
            &[("gemini-cli", vec![(file.clone(), vec![dead])], true)],
            None,
        );
        assert_eq!(checks.len(), 1);
        assert!(checks[0].health.is_broken(), "a dead gate passed");
        let Health::Broken { detail, resolution } = &checks[0].health else {
            unreachable!("just asserted")
        };
        // Names the agent, what is wrong, and the file it is wrong in — a
        // report that said only "broken" sends somebody to read four files.
        assert!(detail.contains("gemini-cli"), "{detail}");
        assert!(detail.contains("ungated"), "{detail}");
        assert!(detail.contains("settings.json"), "{detail}");
        // And the way out is the command that rewrites the entry with this
        // executable's real path.
        assert!(
            format!("{resolution}").contains("estigia setup gemini-cli"),
            "{resolution}"
        );

        // The same entry, with the executable actually there, is fine.
        std::fs::write(&gone, "").expect("the executable");
        let live = Wire {
            command: format!("{} hook pre-tool-use", gone.display()),
            executable: gone,
            event: Some("pre-tool-use"),
            named: "pre-tool-use".to_owned(),
        };
        let checks = gates(&[("gemini-cli", vec![(file, vec![live])], true)], None);
        assert!(
            !checks[0].health.is_broken(),
            "a live gate was called broken"
        );
    }

    #[test]
    fn an_agent_with_no_gate_registered_is_skipped_rather_than_broken() {
        // Eight of the eleven adapters get the contract and no gate, by design.
        // Reporting a deliberate choice as damage teaches people to ignore the
        // report — which is the one thing a health report cannot survive.
        let checks = gates(&[("codex", Vec::new(), false)], None);
        assert_eq!(checks.len(), 1);
        assert!(matches!(checks[0].health, Health::Skipped { .. }));
        assert!(!checks[0].health.is_broken());
    }

    /// An entry this build cannot read is not an agent that keeps its gate
    /// somewhere else.
    ///
    /// Measured on the installed binary: pointing `.claude/settings.json` at
    /// `…\.cargo\bin\ausente.exe` made this row answer *"claude-code: gated by
    /// its own file rather than a settings entry, so there is no wiring here to
    /// be wrong"* — about an agent gated by a settings entry, and about an entry
    /// that is wrong. `status` on the same machine said `gate on`.
    ///
    /// The two readers disagree by design: `is_gated` recognises the entry by
    /// `hook pre-tool-use`, and `wire_in` requires the executable's own file name
    /// to hold `estigia`, so a copy renamed to anything else is plain to one and
    /// invisible to the other. The disagreement is the fact worth reporting, and
    /// it was being reported as its opposite.
    #[test]
    fn an_entry_this_build_cannot_read_is_not_an_agent_gated_somewhere_else() {
        // Settings-gated, registered, and nothing parsed out of it.
        let unreadable = gates(&[("claude-code", Vec::new(), true)], None);
        assert!(
            unreadable[0].health.is_broken(),
            "an entry nothing could read was reported as nothing to read: {:?}",
            unreadable[0].health
        );
        let Health::Broken { resolution, .. } = &unreadable[0].health else {
            panic!("just asserted broken");
        };
        assert!(
            matches!(resolution, Resolution::Run { command, .. } if command.contains("claude-code")),
            "the fault names no command that re-registers it: {resolution:?}"
        );

        // The sentence it used to give belongs to an agent that really does keep
        // its gate in a file of its own, and it keeps it.
        let plugin = gates(&[("cline", Vec::new(), true)], None);
        let Health::Skipped { detail } = &plugin[0].health else {
            panic!(
                "a plugin-gated agent was reported broken: {:?}",
                plugin[0].health
            );
        };
        assert!(
            detail.contains("its own file"),
            "a plugin-gated agent stopped being told apart: {detail}"
        );

        // And the floor at the other end: an agent with no gate at all is still
        // skipped, not broken. Without this, refusing everything would pass the
        // assertion above.
        let none = gates(&[("codex", Vec::new(), false)], None);
        assert!(
            !none[0].health.is_broken(),
            "an agent that was never gated was reported as damage: {:?}",
            none[0].health
        );
    }

    #[test]
    fn a_tracker_with_no_executable_is_skipped_rather_than_reported_broken() {
        // Inert and broken are different, and only one of them needs fixing.
        let root = tempfile::tempdir().expect("a temporary root");
        let checks = examine(
            Some(root.path()),
            root.path(),
            &crate::config::Tracker::Linear,
        );
        let transport = checks
            .iter()
            .find(|check| check.name == "transport")
            .expect("the transport is checked");
        assert!(matches!(transport.health, Health::Skipped { .. }));
        assert!(!checks.iter().any(|check| check.health.is_broken()));
    }

    #[test]
    fn a_missing_skill_is_the_only_thing_reported_because_nothing_after_it_means_anything() {
        let checks = examine(None, Path::new("."), &github());
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "skill");
        assert!(checks[0].health.is_broken());
    }

    #[test]
    fn every_broken_check_names_a_way_out() {
        // The ratchet applied to an environment. A report that says what is
        // wrong and not what to do is the dead end it exists to prevent.
        let root = tempfile::tempdir().expect("a temporary root");
        for check in examine(Some(root.path()), root.path(), &github()) {
            if check.health.is_broken() {
                assert!(
                    check.health.resolution().is_some(),
                    "{} is broken and says nothing about what to do",
                    check.name
                );
            }
        }
    }

    /// A tracker Estigia can operate is not broken for a file nobody runs.
    ///
    /// The inverse of the test this replaced, which asserted that a **missing**
    /// `scripts/github.py` was broken and named `estigia sync`. The transport is
    /// retired; a machine without that file is a machine in the state this
    /// release intends, and reporting it broken would send an operator to repair
    /// something that is not wrong.
    ///
    /// The drift the old row caught has not gone unwatched — the `skill` row
    /// compares every installed file against this binary's copy, which is where
    /// the check belonged in the first place. What is asserted here is only that
    /// the row still says the tracker can be operated at all, because a tracker
    /// with no implementation still has to be `Skipped` and say so.
    #[test]
    fn a_tracker_answered_in_process_is_not_a_broken_machine() {
        let root = tempfile::tempdir().expect("a temporary root");
        let checks = examine(Some(root.path()), root.path(), &github());
        let transport = checks
            .iter()
            .find(|check| check.name == "transport")
            .expect("the transport is checked");
        assert!(
            !transport.health.is_broken(),
            "a machine with no interpreter and no script was called broken: {:?}",
            transport.health
        );

        // The floor, and the property the row exists for: a tracker with nothing
        // behind it is still told apart from one that works.
        let none = examine(
            Some(root.path()),
            root.path(),
            &crate::config::Tracker::Linear,
        );
        let skipped = none
            .iter()
            .find(|check| check.name == "transport")
            .expect("the transport is checked");
        assert!(
            matches!(skipped.health, Health::Skipped { .. }),
            "a tracker Estigia holds no tools for was not said to be inert: {:?}",
            skipped.health
        );
    }

    #[test]
    fn a_directory_that_is_not_a_repository_is_skipped_rather_than_failed() {
        // Running `doctor` from a home directory must not report that git is
        // broken. A report people learn to ignore is a report that has stopped
        // working.
        let root = tempfile::tempdir().expect("a temporary root");
        let checks = examine(Some(root.path()), root.path(), &github());
        let remote = checks
            .iter()
            .find(|check| check.name == "remote")
            .expect("the remote is checked");
        assert!(
            matches!(remote.health, Health::Skipped { .. }),
            "a non-repository was reported as a failure: {:?}",
            remote.health
        );
    }

    #[test]
    fn every_check_says_what_it_is_for() {
        let root = tempfile::tempdir().expect("a temporary root");
        for check in examine(Some(root.path()), root.path(), &github()) {
            assert!(
                check.about.len() > 20,
                "{} does not say what it is for",
                check.name
            );
            assert!(!check.name.is_empty());
        }
    }

    /// No check on this list asks after an interpreter any more.
    ///
    /// What this replaced was `a_broken_python_is_not_reported_as_a_working_one`,
    /// and it was worth having: the launcher shim on this crate's own platform
    /// starts, writes an install error and exits 2, so the reader had to require
    /// the output to actually say *Python*. Nothing runs an interpreter now, and
    /// a health report that still asked for one would send an operator to fix a
    /// dependency this release removed.
    ///
    /// Asserted rather than assumed, because the row is cheap to reintroduce by
    /// habit and an operator reading `python` on this list would reasonably go
    /// and install it.
    #[test]
    fn nothing_on_the_list_asks_for_an_interpreter() {
        let root = tempfile::tempdir().expect("a temporary root");
        let checks = examine(Some(root.path()), root.path(), &github());
        assert!(
            !checks.iter().any(|check| check.name == "python"),
            "the health report asks for an interpreter this release does not use"
        );
        // The floor: the walk produced the rows it should, so a list that came
        // back empty — or came back without the row this one replaced — cannot
        // pass for a clean one.
        for wanted in ["skill", "transport", "gh"] {
            assert!(
                checks.iter().any(|check| check.name == wanted),
                "the walk produced no `{wanted}` row, so its silence about `python` says nothing"
            );
        }
    }
    #[test]
    fn each_agent_is_reported_by_the_path_it_actually_reads() {
        let root = tempfile::tempdir().expect("a temporary root");
        let present = root.path().join("own/SKILL.md");
        std::fs::create_dir_all(present.parent().expect("a parent")).expect("the directory");
        std::fs::write(&present, "# contract").expect("the contract");
        let absent = root.path().join("shared/SKILL.md");

        fn entry(agent: &'static str, path: std::path::PathBuf, own_root: bool) -> Contract {
            Contract {
                agent,
                path,
                own_root,
                directive: None,
                directive_absent: None,
                stale: false,
                unreadable: None,
                ignored: Vec::new(),
                cut_short: Vec::new(),
                inert_for_this_tracker: Vec::new(),
                shadowed_local: None,
                duplicated: Vec::new(),
            }
        }

        // Nothing configured is a broken check, not an empty report: a doctor
        // that says nothing about agents reads as "all fine" to somebody who
        // installed none.
        let none = contracts(&[]);
        assert_eq!(none.len(), 1);
        assert!(matches!(none[0].health, Health::Broken { .. }));

        let checks = contracts(&[
            entry("codex", present.clone(), true),
            entry("gemini-cli", absent.clone(), false),
        ]);
        assert_eq!(checks.len(), 2);

        // The one that is there reports the full path, so an operator can hand
        // back an answer nobody had to guess at.
        let Health::Fine { detail } = &checks[0].health else {
            panic!("a contract that exists was reported broken");
        };
        assert!(detail.contains(&present.display().to_string()));
        assert!(
            detail.contains("verified"),
            "a verified root is not said so"
        );

        // The shared root says plainly what has to be confirmed. Promoting an
        // adapter by reading its documentation is the unchecked claim
        // `setup::tests` forbids; this is the only honest alternative.
        let shared = contracts(&[entry("gemini-cli", present.clone(), false)]);
        let Health::Fine { detail } = &shared[0].health else {
            panic!("the shared root was reported broken");
        };
        assert!(
            detail.contains("confirm your agent reads it"),
            "the operator is not told what to check: {detail}"
        );

        // A file that is there is not a file a run can use. Three documents
        // layer into what the agent reads, two of them hand-editable, and one
        // bad row in any of them refuses every command that reads
        // configuration — while this check said `ok` because the contract
        // existed.
        let unreadable = contracts(&[Contract {
            agent: "codex",
            path: present,
            own_root: true,
            directive: None,
            directive_absent: None,
            stale: false,
            unreadable: Some("`Merge strategy` is \"octopus\" in estigia.local.md".to_owned()),
            ignored: Vec::new(),
            cut_short: Vec::new(),
            inert_for_this_tracker: Vec::new(),
            shadowed_local: None,
            duplicated: Vec::new(),
        }]);
        let Health::Broken {
            detail, resolution, ..
        } = &unreadable[0].health
        else {
            panic!("a contract no run can read was reported fine");
        };
        assert!(detail.contains("octopus"), "{detail}");
        assert!(
            detail.contains("estigia.local.md"),
            "it does not say which file: {detail}"
        );
        // And it names no command: two of the three files are the operator's,
        // and Estigia does not edit those.
        assert!(
            matches!(resolution, crate::outcome::Resolution::NoCommand { .. }),
            "a row in the operator's own file was given a command to run"
        );

        // A contract that is not there is broken, with the command that fixes
        // it — not a bare "missing".
        let Health::Broken { detail, resolution } = &checks[1].health else {
            panic!("a missing contract was reported fine");
        };
        assert!(detail.contains(&absent.display().to_string()));
        assert!(
            format!("{resolution:?}").contains("gemini-cli"),
            "the way out does not name the agent to fix"
        );
    }

    /// Who an ungated line is attributed to, read off the line itself.
    ///
    /// Through `ungated_calls`, not by handing `silence` a made-up `Ungated`:
    /// the losses are in the reading, and the assembly is where the dialect
    /// used to be picked up under the name `agent`.
    #[test]
    fn an_ungated_line_is_attributed_to_its_agent_and_never_to_its_dialect() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        std::fs::create_dir_all(&state).expect("the state directory");
        let path = crate::harness::session::ledger_path(&state);

        let source_of = |line: &str| {
            std::fs::write(&path, format!("{line}\n")).expect("a ledger");
            ungated_calls(&state).source
        };

        // What the hook writes now. Codex speaks Claude Code's dialect, and
        // the line has to come back as Codex.
        let both = source_of(
            r#"{"at":1,"verdict":"payload-absent","agent":"codex","dialect":"claude-code","event":"pre-tool-use"}"#,
        );
        assert_eq!(
            both.as_deref(),
            Some("codex's pre-tool-use hook"),
            "an ungated call was attributed to the dialect its agent happens to speak"
        );

        // A line from a settings file written before `--agent` existed. Half an
        // answer, offered as half — several agents share each dialect, and
        // naming one of them is the defect this pair was split to fix.
        let dialect_only = source_of(
            r#"{"at":1,"verdict":"payload-absent","dialect":"claude-code","event":"pre-tool-use"}"#,
        );
        let dialect_only = dialect_only.expect("a line that knows its dialect says so");
        assert!(
            dialect_only.contains("claude-code") && dialect_only.contains("dialect"),
            "the one fact such a line does carry was thrown away: {dialect_only}"
        );
        assert!(
            !dialect_only.starts_with("claude-code'"),
            "a dialect is being read out as the agent's name: {dialect_only}"
        );

        // And a line from before either: nothing, which `silence` reports as
        // *from an agent no line names*.
        assert_eq!(
            source_of(r#"{"at":1,"verdict":"payload-absent"}"#),
            None,
            "a line naming nothing was attributed to somebody"
        );
    }

    /// A divergence between the gate's root and an agent's is named.
    ///
    /// The row exists because `doctor` had eleven contract rows and one `skill`
    /// row and never related them: an agent reading `Delivery authorisation:
    /// auto` beside a gate adjudicating `ask` was every row `ok` and no
    /// sentence anywhere saying they were different files.
    #[test]
    fn a_gate_deciding_by_rows_the_agent_never_reads_is_reported() {
        use super::{Divergence, canonical};

        let root = std::path::PathBuf::from("/home/somebody/.agents/skills/flow");

        // A comparison that was not made is never reported as agreement.
        assert!(matches!(
            canonical(Some(&root), None).health,
            super::Health::Skipped { .. }
        ));
        assert!(matches!(
            canonical(None, Some(&[])).health,
            super::Health::Skipped { .. }
        ));

        // Agreement is quiet, and still says where the deciding happens.
        let agreed = canonical(Some(&root), Some(&[]));
        assert_eq!(agreed.name, "canonical");
        match agreed.health {
            super::Health::Fine { detail } => assert!(
                detail.contains("flow"),
                "the row reporting agreement does not say where: {detail}"
            ),
            other => panic!("agreement was reported as {other:?}"),
        }

        // A row about the repository differing is the fault: those answer the
        // same whichever agent asks, so two answers means one agent is being
        // decided for by the other's file.
        let diverged = canonical(
            Some(&root),
            Some(&[
                Divergence {
                    agent: "claude-code",
                    rows: vec![(
                        crate::config::Setting::Board,
                        "asanabrial/12".to_owned(),
                        "none".to_owned(),
                    )],
                },
                Divergence {
                    agent: "codex",
                    rows: vec![(
                        crate::config::Setting::ChangeSize,
                        "120".to_owned(),
                        "800".to_owned(),
                    )],
                },
            ]),
        );
        match diverged.health {
            super::Health::Broken { detail, resolution } => {
                for expected in ["claude-code", "Project board", "asanabrial/12", "none"] {
                    assert!(
                        detail.contains(expected),
                        "the divergence does not name {expected:?}: {detail}"
                    );
                }
                assert!(
                    detail.contains("one other agent diverges too"),
                    "a second diverging agent was dropped without a word: {detail}"
                );
                // The plain form propagates a repository row, so it is the one
                // named here — and it is named because running it clears this.
                let way_out = format!("{resolution}");
                assert!(
                    way_out.contains("with no `--agent`"),
                    "a repository row was not offered the command that clears it: {way_out}"
                );
            }
            other => panic!("a gate deciding by rows nobody reads was reported as {other:?}"),
        }

        // A row about the machine is the fault too, and it now has the same way
        // out. It did not: two rounds of review measured both forms leaving the
        // row exactly as red, because the plain one wrote the canonical
        // contract alone and the per-agent one could not hold a machine row in
        // a shared root at all. Both halves are fixed (issue #62), so this
        // asserts the command is named — and, below, that the sentence saying
        // nothing clears it is gone. A report that goes on describing a closed
        // gap sends an operator away from the command that works.
        let machine = canonical(
            Some(&root),
            Some(&[Divergence {
                agent: "claude-code",
                rows: vec![(
                    crate::config::Setting::Summary,
                    "Spanish".to_owned(),
                    "English".to_owned(),
                )],
            }]),
        );
        match machine.health {
            super::Health::Broken { detail, resolution } => {
                assert!(detail.contains("Summary language"), "{detail}");
                let way_out = format!("{resolution}");
                assert!(
                    way_out.contains("config set \"<row>\""),
                    "a machine row was not offered the command that now propagates it: {way_out}"
                );
                assert!(
                    way_out.contains("with no `--agent`"),
                    "the resolution does not say which form of the command holds it: {way_out}"
                );
                assert!(
                    !way_out.contains("no command does that yet"),
                    "the report still describes a gap that is closed: {way_out}"
                );
            }
            other => panic!("a row about the machine differing was reported as {other:?}"),
        }

        // And a per-agent row differing is the feature. `config set --agent
        // opencode "Planning" "sdd lite"` is a documented command, and calling
        // the machine broken for having run it names a fault with no way out —
        // measured by both halves of the first review of this row, one on
        // `Model routing` and one on `Planning`. Named, never broken.
        let per_agent = canonical(
            Some(&root),
            Some(&[Divergence {
                agent: "opencode",
                rows: vec![
                    (
                        crate::config::Setting::Planning,
                        "sdd lite".to_owned(),
                        "direct".to_owned(),
                    ),
                    (
                        crate::config::Setting::Judges,
                        "two blind".to_owned(),
                        "single".to_owned(),
                    ),
                ],
            }]),
        );
        match per_agent.health {
            super::Health::Fine { detail } => {
                for expected in ["opencode", "Planning", "Blind judges", "by design"] {
                    assert!(
                        detail.contains(expected),
                        "a row that differs by design was not named: {detail}"
                    );
                }
            }
            other => panic!("a supported per-agent configuration was reported as {other:?}"),
        }

        // And it is still named when something else is broken. It was not: the
        // broken branch built its sentence from the faulty rows alone, so a row
        // somebody had set on purpose disappeared from the report exactly when
        // the report grew a second subject.
        let both = canonical(
            Some(&root),
            Some(&[Divergence {
                agent: "claude-code",
                rows: vec![
                    (
                        crate::config::Setting::ChangeSize,
                        "120".to_owned(),
                        "800".to_owned(),
                    ),
                    (
                        crate::config::Setting::Planning,
                        "sdd lite".to_owned(),
                        "direct".to_owned(),
                    ),
                ],
            }]),
        );
        match both.health {
            super::Health::Broken { detail, .. } => {
                assert!(detail.contains("Change size"), "{detail}");
                assert!(
                    detail.contains("Planning") && detail.contains("by design"),
                    "the deliberate per-agent row vanished as soon as a fault appeared: {detail}"
                );
            }
            other => panic!("a repository row at odds was reported as {other:?}"),
        }
    }

    /// The whole report, on the machine shape #41 was measured on.
    #[test]
    fn the_report_crosses_the_two_roots_rather_than_reporting_each_alone() {
        let home = tempfile::tempdir().expect("a temporary home");
        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            platform: Some(crate::setup::Platform::Unix),
            skip_harness: true,
            ..crate::setup::SetupOptions::default()
        };
        let config = crate::config::Config::default();
        for slug in ["agents", "claude-code"] {
            let adapter = crate::setup::find_agent(slug).expect("a declared agent");
            crate::setup::setup(adapter, &config, &options).expect("the install writes");
        }
        let claude = crate::setup::resolve_paths(
            crate::setup::find_agent("claude-code").expect("a declared agent"),
            &options,
        )
        .expect("paths")
        .skill_root;

        let named = |checks: &[super::Check]| {
            checks
                .iter()
                .find(|check| check.name == "canonical")
                .map(|check| check.health.clone())
                .expect("the report carries the row")
        };
        let report = |options: &crate::setup::SetupOptions| {
            let root = crate::harness::discover_skill_root_in(options).expect("a canonical root");
            super::full(
                Some(&root),
                home.path(),
                &crate::config::Tracker::Github { repo: None },
                options,
            )
        };

        // Two identical installs and no operator file: nothing diverges.
        assert!(
            matches!(named(&report(&options)), super::Health::Fine { .. }),
            "a machine whose roots agree is being told they do not"
        );

        // Their own file, in one root, setting a per-agent row the other cannot
        // see. Named, and still `Fine`: that row is allowed to differ.
        let theirs = claude.join(crate::config::LOCAL_FILE);
        std::fs::write(
            &theirs,
            "| Setting | Value here |\n|---|---|\n| Blind judges | two blind |\n",
        )
        .expect("the operator's own file");
        match named(&report(&options)) {
            super::Health::Fine { detail } => assert!(
                detail.contains("Blind judges") && detail.contains("by design"),
                "a row that may differ by agent was not named: {detail}"
            ),
            other => panic!("a supported per-agent configuration was reported as {other:?}"),
        }

        // A machine-wide row, which may not: one of these two agents is being
        // decided for by a file it does not read.
        std::fs::write(
            &theirs,
            "| Setting | Value here |\n|---|---|\n| Change size | 120 |\n",
        )
        .expect("the operator's own file");
        match named(&report(&options)) {
            super::Health::Broken { detail, .. } => assert!(
                detail.contains("Change size"),
                "the divergence does not name the row: {detail}"
            ),
            other => panic!("a silent divergence stayed silent: {other:?}"),
        }
    }
}
