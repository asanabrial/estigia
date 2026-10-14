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

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

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
        /// The way out. Kept typed rather than rendered: a caller that turns a
        /// broken check into a refusal has to carry *this* resolution through.
        resolution: Resolution,
    },
    /// Not applicable here, which is not a failure.
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

/// The checks that report on runs that **already happened**: they answer
/// *were the runs that already happened gated at all*, not *could a run work
/// now*, and must never block readiness. Declared here rather than asked of
/// each check, beside [`CHECKS`], which is already the one place the names
/// live.
const LOOKS_BACK: &[&str] = &["silence", "stale-run-pointer", "prune", "gate-version"];

impl Check {
    /// The lines that report this check, shared by `estigia doctor` and the
    /// screen's Doctor page; a broken check carries its resolution on a second.
    pub fn report_lines(&self) -> Vec<String> {
        match &self.health {
            Health::Fine { detail } => vec![format!("  ok       {:<10} {detail}", self.name)],
            Health::Skipped { detail } => {
                vec![format!("  skipped  {:<10} {detail}", self.name)]
            }
            Health::Broken { detail, resolution } => vec![
                format!("  BROKEN   {:<10} {detail}", self.name),
                format!("           {:<10} {resolution}", ""),
            ],
        }
    }

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

/// Whether the installed skill assets came from this build's asset set.
///
/// The per-file comparison the `skill` row makes answers file by file; this
/// answers the install as a whole, in one comparison of the recorded
/// aggregate against this binary's. A mismatch is a stale install and names
/// `sync`, the same remedy — an install written by an older build is
/// refreshed by running the newer one. No record at all is skipped rather
/// than cleared: it says nothing about the files, and an unknown answer is
/// not clearance.
pub fn skill_assets(skill_root: &Path) -> Check {
    let about = "whether the installed skill assets came from this build's asset set";
    let running = crate::skill::assets_digest();
    match crate::skill::record::assets(skill_root) {
        Some(recorded) if recorded == running => Check {
            name: "skill-assets",
            about,
            health: Health::Fine {
                detail: "installed assets match this build".to_owned(),
            },
        },
        Some(_) => Check {
            name: "skill-assets",
            about,
            health: Health::broken(
                "the installed skill assets came from an older build \u{2014} a stale install \
                 reports its own age as current",
                Resolution::run("estigia sync"),
            ),
        },
        None => Check {
            name: "skill-assets",
            about,
            health: Health::Skipped {
                detail: "no asset digest was recorded at install — sync to record one".to_owned(),
            },
        },
    }
}

/// Runs every check against one installation.
///
/// Ordered so the first failure is the one worth fixing first: an absent skill
/// makes every later check meaningless, and an absent interpreter makes the
/// tracker checks meaningless in turn.
pub fn examine(skill_root: Option<&Path>, repo_dir: &Path) -> Vec<Check> {
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
            // Unreadable is not current: the operator table carries a value
            // this binary does not recognise, so there is nothing to compare
            // against and staleness is unknown rather than absent. The remedy
            // is the row, not `sync`, which refuses such a contract unchanged.
            crate::skill::Presence::Unreadable => Health::broken(
                format!(
                    "the skill in {} cannot be judged against this binary \u{2014} the operator \
                     table carries a row value it does not recognise, so whether the contract the \
                     agent reads is current is unknown",
                    skill_root.display()
                ),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "fix the row carrying the unrecognised value in the installed contract or \
                     the local override, then run `estigia sync`",
                ),
            ),
            _ => Health::Fine {
                detail: skill_root.display().to_string(),
            },
        },
    });

    checks.push(skill_assets(skill_root));

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

    // Reported, never installed here: `doctor` is read-only.
    checks.push(Check {
        name: "push-guard",
        about: "the pre-push hook that refuses a push no live claim authorises",
        health: match super::guard::state(repo_dir) {
            super::guard::State::Installed => Health::Fine {
                detail: "installed — a push from a checkout a live claim holds is refused \
                         unless that claim justifies it; \
                         with no claim, the hook refuses every push here and the agent gate every commit"
                    .to_owned(),
            },
            // Absent is not broken: a repository where nobody works under
            // Estigia has no reason to carry it.
            super::guard::State::Absent => Health::Skipped {
                detail: "not installed — `estigia guard` adds it, and only the push boundary goes ungated without it"
                    .to_owned(),
            },
            // Broken rather than skipped: a hook nothing can read is a question
            // nobody answered, and `estigia guard` refuses to write over it.
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
            super::guard::State::Chained => Health::Fine {
                detail: "in force from a hook Estigia did not write \u{2014} a push from a checkout \
                         a live claim holds is refused unless that claim justifies it; \
                         with no claim, the hook refuses every push here and the agent gate every commit"
                    .to_owned(),
            },
            super::guard::State::Foreign => Health::Skipped {
                detail: "a pre-push hook is here and Estigia did not write it; chaining \
                         `estigia hook pre-push` from it is the way in"
                    .to_owned(),
            },
            // The only one of the states that reads as working and is not; the
            // install sets the bit, so the way out is a command.
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

/// How long a check waits on a program before calling it broken.
const PROGRAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Runs a program and judges the result.
fn program(
    name: &str,
    args: &[&str],
    describe: impl Fn(&str) -> String,
    missing: Resolution,
) -> Health {
    program_within(name, args, describe, missing, PROGRAM_TIMEOUT)
}

fn program_within(
    name: &str,
    args: &[&str],
    describe: impl Fn(&str) -> String,
    missing: Resolution,
    limit: Duration,
) -> Health {
    let answer = crate::bounded::run(
        name,
        std::path::PathBuf::from(name),
        args.iter().map(|arg| (*arg).to_owned()).collect(),
        limit,
        None,
    );
    match answer {
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
        Err(reason) => Health::broken(
            format!("`{name}` is not on PATH or did not answer: {reason}"),
            missing,
        ),
    }
}

/// The URL out of one line of `git remote -v`: split on the tab git puts there,
/// and take the ` (fetch)`/` (push)` off the end rather than the first space —
/// a URL may hold a space, and a line with no tab is kept whole.
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
        // Not a repository is not a broken machine.
        _ => Health::Skipped {
            detail: format!("{} is not a git repository", repo_dir.display()),
        },
    }
}

/// Every verdict that means a call reached the gate and was not decided on.
/// Held in one place because it is read in two.
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
    pub shadowed_local: Option<String>,
    /// Rows read as less than the file says, with the file and what cut them.
    pub cut_short: Vec<(String, crate::config::CutShort)>,
    /// Settings a hand-edited file names twice, with the file that names them.
    pub duplicated: Vec<(String, &'static str)>,
    /// Rows in the operator's own file whose label resolves to no setting, with
    /// the file that holds them. Labels only: the values are not read here.
    pub unknown: Vec<(String, String)>,
    /// Agent-scoped rows the shared contract holds away from their default
    /// that this adapter reads because it has no file of its own for them:
    /// the row's label and its default value.
    pub inherited: Vec<(&'static str, String)>,
    /// Why its configuration cannot be read, if it cannot.
    pub unreadable: Option<String>,
    /// The instruction file whose directive is not this binary's, if it drifted.
    /// A separate field from [`Self::stale`] so the row can name **the file
    /// that moved**.
    pub directive: Option<std::path::PathBuf>,
    /// The instruction file that holds no directive at all, if there is one.
    pub directive_absent: Option<std::path::PathBuf>,
    /// Whether what is installed here is not what this binary would write.
    pub stale: bool,
}

/// The rows in this root's operator file that resolve to no setting, named by
/// the file they are in.
///
/// The reader answers labels only; naming the file is what lets the `contract`
/// row tell an operator where to look, and it is done here rather than in the
/// reader so the reader stays a pure label list.
fn unknown_override_rows(root: &Path) -> Vec<(String, String)> {
    let Some(path) = crate::skill::local_override(root) else {
        return Vec::new();
    };
    let file = path.display().to_string();
    crate::skill::unknown_override_rows(root)
        .into_iter()
        .map(|label| (file.clone(), label))
        .collect()
}

/// Where each configured agent's contract actually landed, and whether it is
/// there. Three adapters have a skill directory of their own; the other ten
/// write to the shared neutral root, and this ships the thing that verifies one
/// wherever it is, reporting the file by its full path.
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
                // Not a resolution that names a command: two of the three
                // documents are the operator's, and Estigia does not edit those.
                Health::broken(
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
                let label = &cut.label;
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
            } else if let Some((label, default)) = entry.inherited.first() {
                Health::broken(
                    format!(
                        "{agent}: {file} sets {rows}, which is answered per agent, and {agent} \
                         has no file of its own for {it} \u{2014} so {agent} and every other \
                         agent on this root without one reads a value written for somebody else",
                        agent = entry.agent,
                        file = entry.path.display(),
                        rows = entry
                            .inherited
                            .iter()
                            .map(|(label, _)| format!("`{label}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        it = if entry.inherited.len() == 1 { "it" } else { "them" },
                    ),
                    Resolution::run(format!(
                        "estigia config set --agent {} {label:?} {default:?}",
                        entry.agent
                    )),
                )
            } else if entry.stale {
                // The rows above say the contract is *wrong*; this one says it
                // is *old*, and the resolution is a command Estigia owns.
                Health::broken(
                    format!(
                        "{}: the skill installed in {} is not this binary's copy, so the rules \
                         the agent reads are not the rules the gate enforces",
                        entry.agent,
                        entry.path.parent().unwrap_or(&entry.path).display()
                    ),
                    Resolution::run("estigia sync"),
                )
            } else if let Some(instructions) = &entry.directive_absent {
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
                let unresolved = match entry.unknown.first() {
                    None => String::new(),
                    Some((file, _)) => {
                        let labels: Vec<&str> =
                            entry.unknown.iter().map(|(_, label)| label.as_str()).collect();
                        format!(
                            " \u{2014} {file} holds {} no setting resolves: {}",
                            if labels.len() == 1 {
                                "one row".to_owned()
                            } else {
                                format!("{} rows", labels.len())
                            },
                            labels.join(", ")
                        )
                    }
                };
                Health::Fine {
                    detail: format!(
                        "{}: {}{}{unresolved}",
                        entry.agent,
                        entry.path.display(),
                        if entry.own_root {
                            " (this agent's own skill directory, verified)"
                        } else {
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
/// **Pure and fed**: the running executable is handed in. Compared through
/// `canonicalize`, so a symlink or a shim onto the same file is not reported as
/// drift; and silent when either side will not resolve, because an alarm
/// invented from a failed read is worse than none. Not `Broken`, deliberately:
/// a path that differs is sometimes exactly right — it must not be invisible.
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

/// Whether the gate each configured agent registers would actually run, read
/// back rather than trusted. Fed the wiring rather than sent to find it: the
/// interesting cases are a file that is absent and one naming an executable
/// that has been deleted, and a function that goes looking cannot be shown to
/// handle either.
pub fn gates(
    agents: &[(&'static str, crate::setup::wiring::Registration, bool)],
    running: Option<&std::path::Path>,
    skipped: &[(&'static str, crate::setup::Skipped)],
) -> Vec<Check> {
    agents
        .iter()
        .map(|(name, files, gated)| {
            let wires: Vec<&crate::setup::wiring::Wire> =
                files.iter().flat_map(|(_, found)| found).collect();
            // The matcher, before the wires: that field is invisible to `Wire`,
            // which reads command lines.
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
                            detail: if *gated {
                                format!(
                                    "{name}: gated by its own file rather than a settings entry, \
                                     so there is no wiring here to be wrong"
                                )
                            } else {
                                format!("{name}: no gate registered — the contract only")
                            },
                        }
                    } else if let Some((_, skip)) = skipped.iter().find(|(agent, _)| agent == name) {
                        Health::broken(
                            format!("{name}: {}", skip.detail),
                            if skip.cleared_by_setup {
                                Resolution::run(format!("estigia setup {name}"))
                            } else {
                                Resolution::no_command(
                                    NoCommandReason::OperatorKnowledge,
                                    skip.advice.clone(),
                                )
                            },
                        )
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
                    Resolution::run(format!("estigia setup {name}")),
                ),
            }
        })
        .collect()
}

/// The root of the checkout `repo_dir` sits in, so a command that must run
/// from the manifest directory works from any subdirectory of it. Git answers
/// this for any directory inside a working tree; `repo_dir` itself is the
/// fallback when it cannot.
fn checkout_root(repo_dir: &Path) -> PathBuf {
    let root = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|root| !root.is_empty());
    root.map(PathBuf::from)
        .unwrap_or_else(|| repo_dir.to_path_buf())
}

/// What reinstalls the gate from the checkout it is being compared against, on
/// the platform this binary itself was built for. On Windows the executable is
/// held open by the MCP server while it runs, so Estigia is stopped first — and
/// the shell that can stop a process is named rather than assumed, because
/// `Stop-Process` is a PowerShell cmdlet and a bare `;` under `cmd.exe` or `sh`
/// would carry on past a command that never ran. On other platforms an in-use
/// executable can be replaced under a running process.
fn reinstall_command(reinstall_from: &Path) -> String {
    let path = reinstall_from.display();
    if cfg!(windows) {
        format!(
            "powershell -NoProfile -Command \"Stop-Process -Name estigia -Force \
             -ErrorAction SilentlyContinue; cargo install --path '{path}' --force\""
        )
    } else {
        format!("cargo install --path '{path}' --force")
    }
}

/// Whether this build of the binary is current against the checkout it is run
/// in. It compares **this binary** against the checkout in the
/// working directory. Fed rather than sent to find out: the pure decision lives
/// in a private `gate_age_verdict`, and this is the git half that supplies what
/// it needs.
pub fn gate_version(repo_dir: &Path) -> Check {
    gate_version_for(
        repo_dir,
        crate::build_info::commit(),
        crate::build_info::dirty(),
    )
}

/// [`gate_version`] for a build identity handed in rather than read from this
/// binary, so a checkout and an embedded commit can be chosen independently.
pub fn gate_version_for(repo_dir: &Path, binary_commit: Option<&str>, binary_dirty: bool) -> Check {
    let reinstall_from = checkout_root(repo_dir);

    let head = git_head(repo_dir);
    let head_reachable = binary_commit.and_then(|commit| git_is_ancestor(repo_dir, "HEAD", commit));
    let commits_ahead = binary_commit.and_then(|commit| git_commits_ahead(repo_dir, commit));

    Check {
        name: "gate-version",
        about: "whether the gate this command runs is the checkout's own build",
        health: gate_age_verdict(
            &reinstall_from,
            binary_commit,
            binary_dirty,
            head.as_deref(),
            head_reachable,
            commits_ahead,
        ),
    }
}

/// Whether `gate_version_for` would be unable to answer only because the
/// embedded commit is not an object in `repo_dir`: a clean build with a commit,
/// a readable head, an ancestry question git does not answer, and a repository
/// that does not hold the commit. A dirty build, one with no commit and a
/// directory with no head are not this, since each says something the operator
/// can act on.
pub fn commit_is_foreign_to(
    repo_dir: &Path,
    binary_commit: Option<&str>,
    binary_dirty: bool,
) -> bool {
    let Some(commit) = binary_commit else {
        return false;
    };
    !binary_dirty
        && git_head(repo_dir).is_some()
        && git_is_ancestor(repo_dir, "HEAD", commit).is_none()
        && !git_holds_commit(repo_dir, commit)
}

fn git_holds_commit(repo_dir: &Path, commit: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["cat-file", "-e"])
        .arg(format!("{commit}^{{commit}}"))
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The current git `HEAD` of a directory, when there is one.
fn git_head(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!head.is_empty()).then_some(head)
}

/// Whether `ancestor` is an ancestor of `descendant` in `repo_dir`:
/// `Some(true)`, `Some(false)`, or `None` when the read itself could not be
/// answered. Mirrors git's `merge-base --is-ancestor`, which exits non-zero on
/// a normal *no*, so it cannot be read through `status.success()` alone:
/// `Some(0)` is yes, `Some(1)` is no, every other exit is unanswered — the
/// shape of a binary whose embedded commit belongs to a history this checkout
/// does not share, not the shape of an older build.
fn git_is_ancestor(repo_dir: &Path, ancestor: &str, descendant: &str) -> Option<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["merge-base", "--is-ancestor"])
        .args([ancestor, descendant])
        .output()
        .ok()?;
    match output.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// How many commits `repo_dir`'s `HEAD` reaches that `commit` does not, when
/// that count can be read. This is `commit..HEAD`, which is a distance only
/// where `commit` is an ancestor of `HEAD`, and the arm that renders it is
/// reached when that has *not* been established — so the count is stated as
/// what it counts rather than as how far behind the build is.
///
/// `None` for a spawn failure, a non-zero exit, an empty reply, or a reply
/// that does not parse — the same discipline [`git_is_ancestor`] applies,
/// carried over to a command whose exit code carries no second meaning to
/// distinguish from a failure to answer.
fn git_commits_ahead(repo_dir: &Path, commit: &str) -> Option<u32> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-list", "--count"])
        .arg(format!("{commit}..HEAD"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// The pure decision behind [`gate_version`]: what the facts mean.
///
/// Deliberately fed so the stale and cannot-tell cases are testable without
/// building two binaries, which is the whole point of separating it from the
/// git half above.
fn gate_age_verdict(
    reinstall_from: &Path,
    binary_commit: Option<&str>,
    binary_dirty: bool,
    checkout_head: Option<&str>,
    head_reachable: Option<bool>,
    commits_ahead: Option<u32>,
) -> Health {
    // A build from a dirty tree cannot be related to the checkout's committed
    // state, whatever the git half says. *Cannot tell*, never a pass.
    if binary_dirty {
        return Health::Skipped {
            detail:
                "this build came from a working tree with uncommitted changes, so its exact bytes \
                 cannot be compared to the checkout"
                    .to_owned(),
        };
    }

    let Some(commit) = binary_commit else {
        return Health::Skipped {
            detail:
                "this binary carries no commit (it was likely built outside a checkout or from a \
                 release), so whether it is older than the checkout cannot be told"
                    .to_owned(),
        };
    };
    let Some(head) = checkout_head else {
        return Health::Skipped {
            detail:
                "this directory's git head could not be read — it may not be a git repository, \
                     may have no commits yet, or `git` itself may not be on the path — so whether \
                     the gate is older than the checkout cannot be told"
                    .to_owned(),
        };
    };

    let Some(reachable) = head_reachable else {
        return Health::Skipped {
            detail: format!(
                "whether this build ({commit}) is older than the checkout ({head}) could not be \
                 established: this repository does not answer whether its head is an ancestor of \
                 {commit}, which is what a commit this checkout's object database does not hold \
                 at all looks like — an unrelated history, a shallow clone, or a checkout of a \
                 different repository"
            ),
        };
    };

    // The one *yes* is current: the checkout's HEAD is reachable from this
    // build's commit, so this binary is at-or-ahead of the checkout. Every
    // failure to establish that is reported, not passed.
    if reachable {
        return Health::Fine {
            detail: format!("this build ({commit}) is current against the checkout"),
        };
    }
    // `reachable` false means only that the checkout's head is not a fact of
    // the history this build already held; nothing here asks the reverse
    // question, so the message says what was read rather than which shape this
    // is.
    let distance = match commits_ahead {
        Some(count) => {
            format!("the checkout holds {count} commit(s) this build's history does not")
        }
        None => "how many commits the checkout holds that this build does not could not be read"
            .to_owned(),
    };
    Health::broken(
        format!(
            "this build ({commit}) is not established as current against the checkout's head \
             ({head}): the checkout may have moved on since this gate was built, or it may simply \
             be a different line of history than the one this build came from — either way, the \
             rulebook you are reading may not be the one running; {distance}; the MCP tools only \
             pick up a reinstalled binary once the session restarts"
        ),
        Resolution::run(reinstall_command(reinstall_from)),
    )
}

/// Lockfile basenames `doctor` offers a declaration for. None carries a regenerating command: each
/// common one re-resolves dependencies rather than reproducing the committed file.
const LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lock",
    "bun.lockb",
    "poetry.lock",
    "uv.lock",
    "Pipfile.lock",
    "go.sum",
    "composer.lock",
    "Gemfile.lock",
    "Podfile.lock",
    "pubspec.lock",
    "mix.lock",
    "flake.lock",
    "gradle.lockfile",
];

fn toml_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

/// The `lockfiles` row: tracked lockfiles `estigia.generated.toml` does not declare, each with the
/// entry to complete. Absent when there is nothing to say; advisory when there is, because the operator
/// decides what is generated and the row never writes the declaration.
pub fn lockfiles(repo_dir: &Path) -> Option<Check> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["ls-files", "-z"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let unreadable = match std::fs::read_to_string(repo_dir.join("estigia.generated.toml")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
        Ok(text) => crate::transport::generated::parse(&text)
            .map(|declarations| {
                declarations
                    .into_iter()
                    .map(|declaration| declaration.path)
                    .collect()
            })
            .map_err(|failure| match failure {
                crate::transport::Failure::Stop(value) => value
                    .get("detail")
                    .and_then(|detail| detail.as_str())
                    .unwrap_or("it does not parse")
                    .to_owned(),
                _ => "it does not parse".to_owned(),
            }),
    };
    let mut found: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|path| !path.is_empty() && !path.chars().any(char::is_control))
        .filter_map(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            LOCKFILES.contains(&name).then(|| path.to_owned())
        })
        .collect();
    if found.is_empty() {
        return None;
    }
    found.sort();
    let declared = match unreadable {
        Ok(declared) => declared,
        Err(reason) => {
            return Some(Check {
                name: "lockfiles",
                about: "whether a tracked lockfile is sent to every reviewer in full because nothing declares it generated",
                health: Health::Skipped {
                    detail: format!(
                        "estigia.generated.toml could not be read ({reason}), so which of the {} \
                         tracked lockfile{} it declares is unknown; correct the file and run \
                         doctor again",
                        found.len(),
                        if found.len() == 1 { "" } else { "s" },
                    ),
                },
            });
        }
    };
    found.retain(|path| !declared.contains(path));
    if found.is_empty() {
        return None;
    }
    let mut detail = format!(
        "{} tracked lockfile{} not declared in estigia.generated.toml, so a change to {} reaches \
         every reviewer in full; to have it checked by reproduction instead, declare it with",
        found.len(),
        if found.len() == 1 { " is" } else { "s are" },
        if found.len() == 1 { "it" } else { "one" },
    );
    let margin = format!("\n{}", " ".repeat(13));
    for path in found {
        detail.push_str(&format!("{margin}[[generated]]"));
        detail.push_str(&format!("{margin}path = {}", toml_string(&path)));
        if let Some((directory, _)) = path.rsplit_once('/') {
            detail.push_str(&format!("{margin}cwd = {}", toml_string(directory)));
        }
    }
    detail.push_str(&format!(
        "{margin}and a `command` you have verified reproduces the file byte for byte; none is \
         suggested here, because the usual ones re-resolve dependencies instead"
    ));
    Some(Check {
        name: "lockfiles",
        about: "whether a tracked lockfile is sent to every reviewer in full because nothing declares it generated",
        health: Health::Skipped { detail },
    })
}

/// Everything `doctor` looks at, by the name it reports under. Three of the
/// twenty (`contract`, `gate`, `tools`, `permission`) produce one row per configured agent.
pub const CHECKS: &[&str] = &[
    "skill",
    "skill-assets",
    "gh",
    "push-guard",
    "remote",
    "contract",
    "canonical",
    "gate",
    "tools",
    "run-pointer",
    "stale-run-pointer",
    "prune",
    "stand-down",
    "silence",
    "gate-version",
    "store",
    "pi-config",
    "pi-subagents",
    "lockfiles",
    "permission",
];

/// One row two roots answer differently: the setting, what the agent reads, and
/// what the gate decides. Both values are carried rather than a bare "these
/// differ", because which of the two is the one they meant is the whole
/// question.
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
/// Named `canonical` because that is the noun the rest of the crate uses for
/// the one root [`super::discover_skill_root`] picks. **Which half is a fault
/// is decided by [`crate::config::Setting::scope`], not here**: `Agent` scope
/// differing by design is named and `ok`; `Everywhere` and `Machine` scope are
/// `Broken`, and both are offered the same command.
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

    // What differs by design, said in both branches rather than dropped in one.
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
    // The first agent, and its rows, rather than every pair; how many were left
    // out is said rather than dropped.
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
            crate::outcome::Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                way_out,
            ),
        ),
    }
}

/// The tracker in force in a checkout, for the checks that ask about it: read
/// the way the gate reads it, keeping every row that parses, and never a
/// refusal.
pub fn tracker_in_force(skill_root: Option<&Path>, repo_dir: &Path) -> crate::config::Tracker {
    skill_root
        .map(|root| crate::skill::installed_config_in_keeping_what_parses(root, repo_dir).0)
        .unwrap_or_default()
        .tracker
}

/// The `Workflow` in force here, read the same way [`tracker_in_force`] reads
/// the tracker: so `doctor` can tell a read-only tracker under assisted work
/// from the same tracker under autonomous work, which is refused.
pub fn workflow_in_force(skill_root: Option<&Path>, repo_dir: &Path) -> crate::config::Workflow {
    skill_root
        .map(|root| crate::skill::installed_config_in_keeping_what_parses(root, repo_dir).0)
        .unwrap_or_default()
        .workflow
}

/// Every check `estigia doctor` runs, assembled, so the claim about how many
/// there are can be measured in one place.
pub fn full(
    skill_root: Option<&Path>,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    options: &crate::setup::SetupOptions,
) -> Vec<Check> {
    let mut checks = examine(skill_root, repo_dir);

    let configured: Vec<&'static crate::setup::AgentAdapter> = crate::setup::AGENTS
        .iter()
        .filter(|adapter| crate::setup::is_present(adapter, options))
        .collect();

    checks.extend(
        crate::setup::RETIRED_AGENTS
            .iter()
            .filter(|adapter| crate::setup::is_present(adapter, options))
            .map(|adapter| Check {
                name: "retired-agent",
                about: "an agent Estigia no longer installs into that is still installed",
                health: Health::broken(
                    format!(
                        "{} is retired and still installed; Estigia no longer sets it up or \
                         syncs it",
                        adapter.slug
                    ),
                    Resolution::run(format!("estigia uninstall {}", adapter.slug)),
                ),
            }),
    );

    let agents: Vec<Contract> = configured
        .iter()
        .filter_map(|adapter| contract_entry(adapter, options))
        .collect();
    checks.extend(contracts(&agents));

    // The gate's side is read without a slug, and the repository layer is left
    // off **both** sides: symmetry is the property that matters.
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

    let wired: Vec<(&'static str, crate::setup::wiring::Registration, bool)> = configured
        .iter()
        .map(|adapter| {
            (
                adapter.slug,
                crate::setup::wiring::registered(adapter, options),
                crate::setup::is_gated(adapter, options),
            )
        })
        .collect();
    let running = std::env::current_exe().ok();
    let skipped: Vec<(&'static str, crate::setup::Skipped)> = configured
        .iter()
        .filter_map(|adapter| {
            crate::setup::skipped_by_agent(adapter, options).map(|skip| (adapter.slug, skip))
        })
        .collect();
    checks.extend(gates(&wired, running.as_deref(), &skipped));

    let servers: Vec<(&'static str, Option<std::path::PathBuf>, Option<bool>)> = configured
        .iter()
        .map(|adapter| {
            (
                adapter.slug,
                crate::setup::tools_command(adapter, options),
                crate::setup::tools_start_the_server(adapter, options),
            )
        })
        .collect();
    checks.extend(tool_servers(&servers, running.as_deref()));

    let pi_agent_dir = crate::setup::find_agent("pi")
        .ok()
        .filter(|adapter| configured.contains(adapter))
        .and_then(|adapter| crate::setup::resolve_paths(adapter, options).ok())
        .and_then(|paths| paths.instructions.parent().map(Path::to_path_buf));
    checks.push(pi_config(
        pi_agent_dir.as_deref(),
        repo_dir,
        crate::setup::installed_pi_version(),
    ));
    checks.push(pi_subagents(pi_agent_dir.as_deref(), repo_dir));
    let modes: Vec<(&'static str, ModeReading)> = configured
        .iter()
        .filter(|adapter| matches!(adapter.slug, "claude-code" | "codex" | "opencode" | "pi"))
        .map(|adapter| {
            let paths = crate::setup::resolve_paths(adapter, options).ok();
            let user_file = paths.and_then(|paths| match adapter.slug {
                "claude-code" => paths.hooks,
                _ => paths.mcp_config,
            });
            (
                adapter.slug,
                read_permission_mode(adapter.slug, user_file.as_deref(), repo_dir),
            )
        })
        .collect();
    checks.extend(permission_modes(&modes));
    checks.extend(lockfiles(repo_dir));

    // Carried rather than defaulted: on an empty path the store's own path is
    // the *relative* `state.db`, and this row would answer about whatever file
    // sat in the directory the operator was standing in.
    // Amended here rather than inside `examine`, which does not know about the
    // stand-down and must not learn by reading the machine.
    let home = options.home_dir.as_deref();
    let standing = crate::harness::session::state_root(home)
        .map_or(crate::harness::standdown::Standing::Away, |root| {
            crate::harness::standdown::standing(&root)
        });
    let now = crate::harness::session::now_seconds();
    amend_push_guard(&mut checks, &standing, now);

    // Before the silence check, which is about the past: this one is about now.
    checks.push(standing_down(&standing, now));
    checks.push(run_pointers(
        &crate::harness::session::state_root(home)
            .map(|root| crate::harness::session::unreadable_holdings(&root))
            .unwrap_or_default(),
    ));
    let holdings = crate::harness::session::state_root(home)
        .map(|root| crate::harness::session::holdings(&root))
        .unwrap_or_default();
    checks.push(stale_run_pointers(skill_root, repo_dir, tracker, &holdings));
    checks.push(sweep(skill_root, repo_dir, &holdings));
    checks.push(silence(&ledger_state(crate::harness::session::state_root(
        home,
    ))));

    // Whether the binary doing all of this is the checkout's own build.
    // Reported, never a blocker, for the reason `LOOKS_BACK` gives.
    checks.push(gate_version(repo_dir));
    checks.push(store(home));
    checks
}

/// One configured agent's contract, read as the `contract` row needs it.
pub fn contract_entry(
    adapter: &'static crate::setup::AgentAdapter,
    options: &crate::setup::SetupOptions,
) -> Option<Contract> {
    let paths = crate::setup::resolve_paths(adapter, options).ok()?;
    let config = crate::skill::installed_config_for(&paths.skill_root, Some(adapter.slug));
    Some(Contract {
        agent: adapter.slug,
        path: paths.skill_root.join("SKILL.md"),
        own_root: adapter.discovers_skills(),
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
        unknown: unknown_override_rows(&paths.skill_root),
        cut_short: crate::skill::rows_cut_short(&paths.skill_root, adapter.slug),
        inherited: inherited_agent_rows(adapter, &paths.skill_root),
    })
}

fn inherited_agent_rows(
    adapter: &crate::setup::AgentAdapter,
    root: &Path,
) -> Vec<(&'static str, String)> {
    if adapter.discovers_skills() {
        return Vec::new();
    }
    let (Ok(shared), Ok(mine)) = (
        crate::skill::installed_config(root),
        crate::skill::installed_config_for(root, Some(adapter.slug)),
    ) else {
        return Vec::new();
    };
    let default = crate::config::Config::default();
    crate::config::SETTINGS
        .iter()
        .filter(|setting| setting.scope() == crate::config::Scope::Agent)
        .filter(|setting| setting.applies_to_agent(adapter))
        .filter(|setting| setting.value_of(&shared) != setting.value_of(&default))
        .filter(|setting| setting.value_of(&mine) == setting.value_of(&shared))
        .map(|setting| (setting.label(), setting.value_of(&default)))
        .collect()
}

/// The local workflow store: whether it opens, which schema it carries, where
/// it is, and what the call counter saw over the last hour.
///
/// Never creates the file: a store nobody has written yet is skipped, and one
/// that exists and will not open is broken, and blocks: every claim, renewal and
/// release reads it, so a run cannot swear on a store it cannot open.
pub fn store(home: Option<&Path>) -> Check {
    use crate::transport::store::Store;
    let about = "whether the local workflow store opens, what it counted, and how old its newest snapshot is";
    let path = match crate::transport::store::default_path(home) {
        Ok(path) => path,
        Err(failure) => {
            return Check {
                name: "store",
                about,
                health: Health::broken(
                    failure.detail(),
                    Resolution::no_command(
                        NoCommandReason::WorldAction,
                        "a HOME or USERPROFILE the process can read",
                    ),
                ),
            };
        }
    };
    if !path.exists() {
        return Check {
            name: "store",
            about,
            health: Health::Skipped {
                detail: format!(
                    "{} does not exist yet; the first counted `gh` call or import creates it",
                    path.display()
                ),
            },
        };
    }
    let opened = Store::open(&path).and_then(|store| {
        let version = store.schema_version()?;
        let since = crate::harness::session::now_seconds()
            .unwrap_or_default()
            .saturating_sub(3600);
        Ok((version, store.calls_since(since)?, store.record_failures()?))
    });
    let health = match opened {
        Ok((version, calls, failures)) => {
            let missed: u64 = failures.iter().map(|(_, count)| count).sum();
            let now = crate::harness::session::now_seconds().unwrap_or_default();
            let snapshot = match crate::transport::store::snapshot_age(&path, now) {
                Some(age) => format!(
                    "last snapshot {} ago",
                    crate::transport::store::age_in_words(age)
                ),
                None => "no snapshot yet".to_owned(),
            };
            let detail = format!(
                "{} opens at schema {version}; last hour: {} GraphQL, {} REST, {} other `gh` calls; \
                 {snapshot}",
                path.display(),
                calls.graphql,
                calls.rest,
                calls.other
            );
            if missed == 0 {
                Health::Fine { detail }
            } else {
                let kinds = failures
                    .iter()
                    .map(|(kind, count)| format!("{count} {kind}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                Health::broken(
                    format!(
                        "{detail}; {missed} record(s) could not be written down and were counted \
                         instead: {kinds}"
                    ),
                    Resolution::no_command(
                        NoCommandReason::OperatorKnowledge,
                        "why the append failed — a decision that could not be written down is \
                         not on the record, and this is where it is counted",
                    ),
                )
            }
        }
        Err(failure) => Health::broken(
            format!("{} does not open: {}", path.display(), failure.detail()),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "inspect the file; every claim, renewal and release decides from it, and moving it aside \
                 is safe only while markers are still written to GitHub, where the next touch \
                 imports them again",
            ),
        ),
    };
    Check {
        name: "store",
        about,
        health,
    }
}

/// Run pointers on this machine that are there and cannot be read.
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

/// Run pointers that parse cleanly and whose own reading is not simply
/// "still open": one that names an issue the tracker reports closed, one
/// whose issue closed behind this run's own delivery and never released, and
/// one whose answer this row could not read as either **or which answered
/// nothing at all** — three populations, reported together in one row. The
/// third is fed by two arms: `Standing::Unread`, an answer no caller can read
/// as pass or stop, and the outer `None` from `tracker_answer_for_pointer`,
/// which is no answer to read in the first place.
///
/// **Filtered to this repository before anything is asked** through
/// `same_git_repository`, the predicate `holders_for_action` already owns.
/// **`same_git_repository` fails closed to `false`**: a pointer whose recorded
/// `repo_dir` no longer resolves as a git checkout at all is silently excluded
/// from this row entirely. A tracker read that fails answers nothing about
/// staleness — the same rule `holder_standing` is built on — so a pointer this
/// could not ask about is its own count, named separately.
pub fn stale_run_pointers(
    skill_root: Option<&Path>,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    holdings: &[super::session::Run],
) -> Check {
    let about = "whether a readable run pointer covering this repository still names an issue the \
         tracker holds open";
    let (assisted, covering): (Vec<&super::session::Run>, Vec<&super::session::Run>) = holdings
        .iter()
        .filter(|run| {
            run.covered()
                .any(|covered| super::same_git_repository(covered, repo_dir))
        })
        .partition(|run| run.assisted);
    let gone: Vec<String> = assisted
        .iter()
        .filter(|run| {
            !run.worktree
                .as_ref()
                .is_some_and(|worktree| worktree.is_dir())
        })
        .map(|run| {
            format!(
                "{} (#{})",
                run.run_id,
                run.issue
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            )
        })
        .collect();
    let gone_section = (!gone.is_empty()).then(|| {
        format!(
            "{} assisted run pointer(s) here name a worktree that is gone, so the gate admits no write \
             under them once its renewal window ends: {}",
            gone.len(),
            gone.join(", ")
        )
    });
    let gone_check = |section: String| Check {
        name: "stale-run-pointer",
        about,
        health: Health::broken(
            section,
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "those pointers taken away \u{2014} they are Estigia's own files and no claim \
                 of anybody's, and `release` writes the tracker, which an assisted repository \
                 does not",
            ),
        ),
    };
    if let Some(section) = &gone_section
        && (tracker.transport().is_none() || skill_root.is_none())
    {
        return gone_check(section.clone());
    }
    if tracker.transport().is_none() {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::Skipped {
                detail: "this tracker has no executable transport to ask".to_owned(),
            },
        };
    }
    let Some(skill_root) = skill_root else {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::Skipped {
                detail: "no skill root is installed to ask the tracker through".to_owned(),
            },
        };
    };
    let mut stale = Vec::new();
    let mut stranded = Vec::new();
    let mut unread = Vec::new();
    let mut passed = 0usize;
    for run in covering {
        match super::tracker_answer_for_pointer(skill_root, repo_dir, tracker, run) {
            Some(answer) => match answer.standing() {
                super::tracker::Standing::Passed if own_delivery_close(&answer) => {
                    stranded.push(format!(
                        "{} (#{})",
                        run.run_id,
                        run.issue
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    ));
                }
                super::tracker::Standing::Passed => passed += 1,
                super::tracker::Standing::Stopped("issue-not-open") => {
                    stale.push(format!(
                        "{} (#{})",
                        run.run_id,
                        run.issue
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    ));
                }
                super::tracker::Standing::Stopped(_) => {}
                super::tracker::Standing::Unread => unread.push(run.run_id.clone()),
            },
            None => unread.push(run.run_id.clone()),
        }
    }
    if stale.is_empty() && stranded.is_empty() && unread.is_empty() && gone_section.is_none() {
        return Check {
            name: "stale-run-pointer",
            about,
            health: Health::Fine {
                detail: format!(
                    "no run pointer covering this repository answered with `issue-not-open`, \
                     answered with `Passed` and an own-delivery close, or carried an answer \
                     this row could not read \u{2014} {passed} of them read as still open and \
                     held by this run"
                ),
            },
        };
    }
    if stale.is_empty()
        && stranded.is_empty()
        && unread.is_empty()
        && let Some(section) = gone_section.clone()
    {
        return gone_check(section);
    }
    let releasable: Vec<String> = stale.iter().chain(stranded.iter()).cloned().collect();
    let mut sections = Vec::new();
    if !stale.is_empty() {
        sections.push(format!(
            "{} run pointer(s) here are readable and name an issue the tracker reports \
             closed \u{2014} one still refuses every write in the checkout it names, and \
             two or more covering the same checkout, with nothing live also covering it, \
             reconcile away to unclaimed instead: \
             {}",
            stale.len(),
            stale.join(", ")
        ));
    }
    if !stranded.is_empty() {
        sections.push(format!(
            "{} run pointer(s) here name an issue that closed behind this run's own \
             delivery, and the delivery skipped `release` \u{2014} unlike a stale pointer, \
             this one does not refuse a single-holder write on its own, but it still holds \
             the checkout until `release` clears it: {}",
            stranded.len(),
            stranded.join(", ")
        ));
    }
    if !unread.is_empty() {
        sections.push(if releasable.is_empty() {
            format!(
                "{} run pointer(s) could not be checked against the tracker, so whether \
                 they are stale is unknown: {}",
                unread.len(),
                unread.join(", ")
            )
        } else {
            format!(
                "{} run pointer(s) could not be checked against the tracker, so whether \
                 they are stale is unknown \u{2014} they are named here, but the release \
                 command below does not cover them: {}",
                unread.len(),
                unread.join(", ")
            )
        });
    }
    if let Some(section) = gone_section {
        sections.push(section);
    }
    let detail = sections.join("; ");
    let resolution = if releasable.is_empty() {
        Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "why `gh issue view` failed for the pointer(s) named above \u{2014} read \
             from the tracker directly, since a machine-wide row cannot say which of \
             several possible causes this one was",
        )
    } else {
        Resolution::run(format!(
            "estigia release --run-id <run-id>   # once for each of: {}",
            releasable.join(", ")
        ))
    };
    Check {
        name: "stale-run-pointer",
        about,
        health: Health::broken(detail, resolution),
    }
}

/// The branches and checkouts no delivery owns, listed.
///
/// Read-only, like every row: it names `estigia prune`, which is what removes
/// them, and it removes nothing itself. In `LOOKS_BACK` for the same reason
/// `stale-run-pointer` is — everything it can name is a leftover from a run
/// that already happened, and a lingering branch does not stop a fresh claim
/// from swearing right now.
///
/// A machine it cannot read is `Skipped`, not `Broken`: a repository with no
/// remote, or a `gh` that cannot answer, is not an accumulation. It is the same
/// rule the other rows follow — an unread answer is not the fault the row
/// reports.
pub fn sweep(
    skill_root: Option<&Path>,
    repo_dir: &Path,
    holdings: &[super::session::Run],
) -> Check {
    let about = "whether branches and checkouts no delivery owns have accumulated";
    let Some(skill_root) = skill_root else {
        return Check {
            name: "prune",
            about,
            health: Health::Skipped {
                detail: "no skill root is installed to read the worktree row through".to_owned(),
            },
        };
    };
    let context =
        crate::transport::Context::live(skill_root.to_path_buf(), repo_dir.to_path_buf(), None);
    let root = crate::harness::prune::root_prefix(context.get("Worktree location"));
    let view = crate::harness::prune::View {
        context: &context,
        base: "main",
        holdings,
        root: root.as_deref(),
    };
    match crate::harness::prune::sweep(&view) {
        Ok(swept) => {
            let deletable: Vec<&crate::harness::prune::Swept> =
                swept.iter().filter(|entry| entry.delete).collect();
            if deletable.is_empty() {
                return Check {
                    name: "prune",
                    about,
                    health: Health::Fine {
                        detail: format!(
                            "no merged or closed branch or checkout is unowned; {} listed and kept \
                             (live claim, unpublished work, or not yet merged or closed)",
                            swept.len()
                        ),
                    },
                };
            }
            let listed: Vec<String> = deletable
                .iter()
                .take(8)
                .map(|entry| format!("{} {}", entry.kind.word(), entry.name))
                .collect();
            let rest = deletable.len().saturating_sub(listed.len());
            let more = match rest {
                0 => String::new(),
                1 => " (and one more)".to_owned(),
                many => format!(" (and {many} more)"),
            };
            Check {
                name: "prune",
                about,
                health: Health::broken(
                    format!(
                        "{} branch(es) or checkout(s) are merged or closed and held by no live \
                         claim: {}{more}",
                        deletable.len(),
                        listed.join(", ")
                    ),
                    Resolution::run("estigia prune --confirm"),
                ),
            }
        }
        Err(failure) => Check {
            name: "prune",
            about,
            health: Health::Skipped {
                detail: format!(
                    "the sweep could not read the machine, so whether anything accumulated is \
                     unknown: {}",
                    failure.detail()
                ),
            },
        },
    }
}

/// Whether `answer` is the `closed-by-own-delivery` population
/// `src/transport/claim.rs` builds — the sha and epoch it carries are not read
/// here, since nothing in this row needs them. The token itself has one
/// spelling, in [`crate::transport::claim::OWN_DELIVERY_CLOSE_PREFIX`].
fn own_delivery_close(answer: &super::tracker::Answer) -> bool {
    answer.is_own_delivery_close()
}

/// What the ledger says, or why it could not be asked. Takes the result rather
/// than a path, because on an empty path the store's own path is the
/// **relative** `state.db`.
fn ledger_state(state_root: Result<std::path::PathBuf, crate::outcome::Refusal>) -> Ungated {
    match state_root {
        Ok(root) => ungated_calls(&root),
        Err(refusal) => Ungated {
            unreadable: Some(refusal.message),
            ..Ungated::default()
        },
    }
}

/// What the ledger could be made to say about calls that went undecided: a
/// list is an answer, and not having been able to read the store is not one.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ungated {
    /// The calls recorded as reaching the gate and not being decided on.
    pub calls: Vec<(String, u64)>,
    /// Why the ledger could not be read, if it is there and could not.
    pub unreadable: Option<String>,
    /// Whether there is no ledger at all, which is not a ledger holding nothing.
    pub missing: bool,
    /// What the most recent undecided call recorded about itself.
    pub latest: Option<String>,
    /// Which agent sent the most recent one, and through which hook.
    pub source: Option<String>,
    /// Sessions whose lifecycle hook arrived with no readable payload.
    pub unminted: Vec<(String, u64)>,
}

/// What the ledger says about calls that reached the gate undecided.
///
/// Read here rather than inside the check, which stays pure and fed.
fn ungated_calls(state_root: &std::path::Path) -> Ungated {
    let path = crate::transport::store::beside(state_root);
    let records = match crate::transport::store::open_for_read(&path) {
        Ok(Some(store)) => match store.records(crate::transport::store::Record::Decision) {
            Ok(records) => records,
            Err(failure) => {
                return Ungated {
                    unreadable: Some(failure.detail()),
                    ..Ungated::default()
                };
            }
        },
        Ok(None) => {
            return Ungated {
                missing: true,
                ..Ungated::default()
            };
        }
        Err(failure) => {
            return Ungated {
                unreadable: Some(failure.detail()),
                ..Ungated::default()
            };
        }
    };
    // A store that holds no decision is a store nothing has reached the gate
    // through, which is not the same as one holding no undecided call.
    if records.is_empty() {
        return Ungated {
            missing: true,
            ..Ungated::default()
        };
    }
    let mut found = Ungated::default();
    for entry in records {
        let Some(verdict) = entry.get("verdict").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !UNDECIDED.contains(&verdict) {
            continue;
        }
        // The timestamp is for the message; the verdict is the finding.
        let at = entry
            .get("at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        // By `at` rather than by position: the last line written is not always
        // the last call made.
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
/// Only a record covering the present moment suspends it. An *unreadable*
/// stand-down is not honoured by the gate either, so the push guard really is
/// still refusing, and softening this row for it would trade one false
/// sentence for another. Applied here rather than inside [`examine`], which
/// does not know about the stand-down and must not learn by reading the
/// machine.
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

/// The way out of a stand-down this build will not honour: a person lifts it.
///
/// Deliberately not a [`Resolution::run`]. An agent is refused that command by
/// `person_only`'s `LIFT` row, and a message may name a command only when
/// running it clears the block. Rendered the way `person_only::refusal` renders
/// its own, so the person still gets the exact command to run.
fn lift_the_stand_down() -> Resolution {
    Resolution::no_command(
        NoCommandReason::OperatorKnowledge,
        "tell the person the exact command, and let them run it in their own shell, or with `!` \
         in Claude Code, which does not pass through the agent's tools: `estigia stand-down \
         --lift --reason cleared`",
    )
}

/// Whether the operator has the gate standing down right now.
///
/// `Skipped` rather than `Broken`, following `push-guard`: a real reduction in
/// coverage, deliberately made, named with what it costs, and not something to
/// refuse a run over.
pub fn standing_down(standing: &crate::harness::standdown::Standing, now: Option<u64>) -> Check {
    let about = "whether the operator has the gate standing down";
    // A file that is there and will not open answers nothing, and "the gate is
    // not standing down" is an answer; nothing else on the machine will ever
    // mention it.
    if let crate::harness::standdown::Standing::Unreadable(why) = standing {
        return Check {
            name: "stand-down",
            about,
            health: Health::broken(
                format!(
                    "a stand-down file is here and cannot be read, so whether the gate is standing \
                     down is unknown \u{2014} it is not being honoured meanwhile: {why}"
                ),
                lift_the_stand_down(),
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
        // because the clock would not answer.
        (Some(_), None) => Health::Skipped {
            detail: "a stand-down is recorded and this machine's clock cannot be read, so \
                     whether it still applies is unknown \u{2014} the gate does not honour one it \
                     cannot time"
                .to_owned(),
        },
        // Declared, not yet over by the clock, and longer than the cap this
        // build honours. `covers` refuses it, and without a row of its own it
        // read as *the gate is not standing down*. An expired record stays in
        // that arm and stays quiet, deliberately.
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
                lift_the_stand_down(),
            )
        }
        // Declared, still ahead by the clock, and **not yet begun**: a legal
        // window the gate refuses to honour, which read as *the gate is not
        // standing down* without a row of its own.
        (Some(record), Some(now)) if now < record.declared_at => Health::broken(
            format!(
                "a stand-down here is declared to begin in {} minute(s), so it is not in force \
                 and writes are being adjudicated as though none had been declared \u{2014} this \
                 machine's clock was ahead when it was written, or the record came from one that \
                 was: {}",
                record.declared_at.saturating_sub(now).div_ceil(60),
                record.reason
            ),
            lift_the_stand_down(),
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
/// The other half of [`gates`]. **Pure and fed**, like `gates`: the interesting
/// case is an executable that is not there, and a function that goes looking
/// cannot be shown to handle it.
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
                // Not a fault: an agent may be gated and hold no tools.
                None => Health::Skipped {
                    detail: format!("{name}: no tool server registered"),
                },
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
                    Resolution::run(format!("estigia setup {name}")),
                ),
            },
        })
        .collect()
}

/// Whether Pi's own settings would unload Estigia's gate or tool server.
///
/// Read only, and it stays that way: the files belong to the person, so the
/// remedy is theirs to apply. Judges the `extensions` array of the user and
/// project `settings.json` and the project `mcp.json` override of the `estigia`
/// server; a file that is missing or does not parse is not a finding, because
/// Pi would not read it either.
pub fn pi_config(
    agent_dir: Option<&Path>,
    repo_dir: &Path,
    pi_version: Option<crate::setup::PiVersion>,
) -> Check {
    let about = "whether Pi's settings leave Estigia's gate, tool server and directive in force";
    let Some(agent_dir) = agent_dir else {
        return Check {
            name: "pi-config",
            about,
            health: Health::Skipped {
                detail: "pi is not configured".to_owned(),
            },
        };
    };
    let project = repo_dir.join(".pi");
    let mut findings: Vec<String> = Vec::new();
    for settings in [
        agent_dir.join("settings.json"),
        project.join("settings.json"),
    ] {
        for entry in pi_extension_exclusions(&settings) {
            findings.push(format!("{} lists `{entry}`", settings.display()));
        }
    }
    let mcp = project.join("mcp.json");
    if let Some(override_) = pi_json(&mcp).and_then(|value| {
        value
            .pointer(&format!("/mcpServers/{}", crate::NAME))
            .cloned()
    }) {
        let off = override_
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            == Some(false);
        let exposure = override_
            .get("exposure")
            .and_then(serde_json::Value::as_str);
        if off {
            findings.push(format!(
                "{} sets `enabled: false` on the `{}` server",
                mcp.display(),
                crate::NAME
            ));
        }
        if exposure.is_some_and(|exposure| !matches!(exposure, "direct" | "deferred")) {
            findings.push(format!(
                "{} sets `exposure: {}` on the `{}` server",
                mcp.display(),
                exposure.unwrap_or_default(),
                crate::NAME
            ));
        }
    }
    let override_file = agent_dir.join("AGENTS.override.md");
    let hidden = override_file.is_file();
    let health = if findings.is_empty() && !hidden {
        Health::Fine {
            detail: format!(
                "no Pi setting excludes Estigia's extensions or overrides its tool server, and \
                 no override file hides its directive; {}",
                pi_exposure_reading(agent_dir, pi_version)
            ),
        }
    } else {
        let mut parts: Vec<String> = Vec::new();
        if !findings.is_empty() {
            parts.push(format!(
                "{} \u{2014} Pi will not load Estigia's gate or reach its tools from the next \
                 session",
                findings.join("; ")
            ));
        }
        if hidden {
            parts.push(format!(
                "{} exists, and Pi reads it in place of AGENTS.md in the same directory \u{2014} \
                 Estigia's directive there is not read",
                override_file.display()
            ));
        }
        Health::broken(
            parts.join("; "),
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "remove the named entry or file yourself; `estigia setup pi` does not touch Pi's \
                 settings or an override file, and Estigia never rewrites them",
            ),
        )
    };
    Check {
        name: "pi-config",
        about,
        health,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerChildren {
    LoadTheGate,
    SkipTheGate(&'static str),
    Unread,
}

const RUNNER_SOURCE_LIMIT: u64 = 1 << 20;
const RUNNER_SOURCE_DEPTH: usize = 3;
const RUNNER_SOURCE_EXTENSIONS: [&str; 4] = ["ts", "js", "mjs", "cjs"];
const RUNNER_SPAWN_CALLS: [&str; 3] = ["spawn(", "execFile(", "spawnSync("];
const RUNNER_PI_COMMAND: &str = "pi";

struct RunnerSources {
    texts: Vec<String>,
    complete: bool,
}

fn pi_runner_sources(path: &Path, depth: usize, out: &mut RunnerSources, budget: &mut u64) {
    if path.is_dir() {
        if depth == 0 {
            out.complete = false;
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            out.complete = false;
            return;
        };
        let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for entry in paths {
            if entry.file_name().is_some_and(|name| name == "node_modules") {
                continue;
            }
            pi_runner_sources(&entry, depth - 1, out, budget);
        }
        return;
    }
    let readable = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| RUNNER_SOURCE_EXTENSIONS.contains(&extension));
    if !readable {
        return;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        out.complete = false;
        return;
    };
    if metadata.len() > *budget {
        out.complete = false;
        return;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => {
            *budget -= metadata.len();
            out.texts.push(text);
        }
        Err(_) => out.complete = false,
    }
}

fn spawns_pi(text: &str) -> bool {
    RUNNER_SPAWN_CALLS.iter().any(|call| {
        text.match_indices(call).any(|(at, _)| {
            let argument = text[at + call.len()..].trim_start();
            let Some(quote) = argument
                .chars()
                .next()
                .filter(|c| matches!(c, '"' | '\'' | '`'))
            else {
                return false;
            };
            let Some(end) = argument[1..].find(quote) else {
                return false;
            };
            let command = &argument[1..=end];
            command == RUNNER_PI_COMMAND
                || command
                    .strip_suffix(RUNNER_PI_COMMAND)
                    .is_some_and(|head| head.ends_with('/'))
        })
    })
}

fn pi_runner_children(path: Option<&Path>) -> RunnerChildren {
    let mut sources = RunnerSources {
        texts: Vec::new(),
        complete: true,
    };
    let mut budget = RUNNER_SOURCE_LIMIT;
    if let Some(path) = path {
        pi_runner_sources(path, RUNNER_SOURCE_DEPTH, &mut sources, &mut budget);
    }
    let text = sources.texts.join("\n");
    if text.contains("--no-extensions") || text.contains("--no-extension\"") {
        return RunnerChildren::SkipTheGate("starts its children with `--no-extensions`");
    }
    if text.contains("createAgentSession") || text.contains("new AgentSession") {
        return RunnerChildren::SkipTheGate("runs its children inside Pi's own process");
    }
    if sources.complete && spawns_pi(&text) {
        return RunnerChildren::LoadTheGate;
    }
    RunnerChildren::Unread
}

fn pi_runner_name(entry: &serde_json::Value) -> Option<(String, Option<std::path::PathBuf>)> {
    let source = entry
        .as_str()
        .or_else(|| entry.get("source").and_then(serde_json::Value::as_str))?;
    let lowered = source.to_ascii_lowercase();
    if !(lowered.contains("subagent") || lowered.contains("sub-agent")) {
        return None;
    }
    let path = Path::new(source)
        .is_absolute()
        .then(|| std::path::PathBuf::from(source));
    Some((source.to_owned(), path))
}

/// Whether the Pi subagent runners installed would give their children Estigia's gate.
///
/// Read only, and advisory: a runner is named from a `packages` entry of the user or project
/// `settings.json`, or an extension file or directory, whose name says subagent. Its children are read
/// from its source when that is on disk. A runner whose children skip the gate, or whose source cannot
/// be read, is reported like a skipped check; nothing is installed or rewritten.
pub fn pi_subagents(agent_dir: Option<&Path>, repo_dir: &Path) -> Check {
    let about = "whether the Pi subagent runners installed give their children Estigia's gate";
    let Some(agent_dir) = agent_dir else {
        return Check {
            name: "pi-subagents",
            about,
            health: Health::Skipped {
                detail: "pi is not configured".to_owned(),
            },
        };
    };
    let project = repo_dir.join(".pi");
    let mut found: Vec<(String, Option<std::path::PathBuf>)> = Vec::new();
    for settings in [
        agent_dir.join("settings.json"),
        project.join("settings.json"),
    ] {
        if let Some(entries) = pi_json(&settings)
            .and_then(|value| value.get("packages").cloned())
            .and_then(|value| value.as_array().cloned())
        {
            found.extend(entries.iter().filter_map(pi_runner_name));
        }
    }
    for directory in [agent_dir.join("extensions"), project.join("extensions")] {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name.contains("subagent") || name.contains("sub-agent") {
                found.push((path.display().to_string(), Some(path)));
            }
        }
    }
    if found.is_empty() {
        return Check {
            name: "pi-subagents",
            about,
            health: Health::Fine {
                detail: "no Pi subagent runner is installed in the settings or extension \
                         directories it reads"
                    .to_owned(),
            },
        };
    }
    let mut warnings: Vec<String> = Vec::new();
    let mut gated: Vec<String> = Vec::new();
    for (name, path) in &found {
        match pi_runner_children(path.as_deref()) {
            RunnerChildren::LoadTheGate => gated.push(name.clone()),
            RunnerChildren::SkipTheGate(how) => warnings.push(format!(
                "{name} {how}, so its children write with no Estigia gate"
            )),
            RunnerChildren::Unread => warnings.push(format!(
                "{name} could not be read in full or does not visibly start `pi`, so whether its children \
                 load the gate is unknown"
            )),
        }
    }
    let mut detail = String::new();
    if !warnings.is_empty() {
        detail.push_str(&format!("WARNING {}", warnings.join("; ")));
    }
    if !gated.is_empty() {
        if !detail.is_empty() {
            detail.push_str("; ");
        }
        detail.push_str(&format!(
            "{} start{} a separate pi that loads the gate",
            gated.join(", "),
            if gated.len() == 1 { "s" } else { "" }
        ));
    }
    let health = if warnings.is_empty() {
        Health::Fine { detail }
    } else {
        Health::Skipped { detail }
    };
    Check {
        name: "pi-subagents",
        about,
        health,
    }
}

fn pi_exposure_reading(agent_dir: &Path, version: Option<crate::setup::PiVersion>) -> String {
    let seen = version.map_or_else(
        || "an unreadable version".to_owned(),
        |version| format!("Pi {version}"),
    );
    let exposure = pi_json(&agent_dir.join("mcp.json")).and_then(|value| {
        value
            .pointer(&format!("/mcpServers/{}/exposure", crate::NAME))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    });
    let Some(exposure) = exposure else {
        return format!("the tool server's exposure is not set in the user mcp.json ({seen})");
    };
    let mut reading = format!("the tool server's exposure is `{exposure}` ({seen})");
    if exposure == "direct" && version.is_some_and(|version| version >= crate::setup::DEFERRED_FROM)
    {
        reading.push_str(&format!(
            "; warning: this Pi lists deferred servers and loads their tools on demand, while \
             `direct` declares every tool on every request. `estigia setup pi` writes `{}`",
            crate::setup::PiExposure::Deferred.word()
        ));
    }
    reading
}

fn pi_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn pi_extension_exclusions(settings: &Path) -> Vec<String> {
    let estigia_files = [
        crate::setup::plugin::PI_FILE,
        crate::setup::plugin::PI_REVIEW_FILE,
    ];
    let Some(entries) = pi_json(settings)
        .and_then(|value| value.get("extensions").cloned())
        .and_then(|value| value.as_array().cloned())
    else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|entry| {
            let spelled = entry.replace('\\', "/");
            if let Some(exact) = spelled.strip_prefix('-') {
                return exact == "builtin:mcp"
                    || estigia_files
                        .iter()
                        .any(|file| exact.rsplit('/').next() == Some(*file));
            }
            spelled.strip_prefix('!').is_some_and(|glob| {
                estigia_files.iter().any(|file| {
                    pi_glob(glob, file)
                        || pi_glob(
                            glob,
                            &format!("{}/{file}", crate::setup::plugin::PI_DIRECTORY),
                        )
                })
            })
        })
        .map(str::to_owned)
        .collect()
}

fn pi_glob(pattern: &str, text: &str) -> bool {
    fn at(pattern: &[char], text: &[char]) -> bool {
        match pattern.split_first() {
            None => text.is_empty(),
            Some(('*', rest)) => (0..=text.len()).any(|skip| at(rest, &text[skip..])),
            Some(('?', rest)) => text.split_first().is_some_and(|(_, tail)| at(rest, tail)),
            Some((literal, rest)) => text
                .split_first()
                .is_some_and(|(head, tail)| head == literal && at(rest, tail)),
        }
    }
    at(
        &pattern.chars().collect::<Vec<_>>(),
        &text.chars().collect::<Vec<_>>(),
    )
}

/// What a host's own settings say about its automatic permission mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeReading {
    /// A setting that names the mode, and the file it came from.
    Set {
        /// The setting as the file spells it.
        value: String,
        /// Where it was read.
        file: PathBuf,
    },
    /// Every file that could hold the setting was read and none sets it.
    Unset,
    /// A file that could hold the setting exists and could not be parsed.
    Unreadable {
        /// The file.
        file: PathBuf,
        /// What the parser said.
        why: String,
    },
    /// The host has no permission prompts for a mode to approve.
    NoPrompts,
}

/// Reads one host's configured permission mode, most specific file first.
///
/// Only settings files are read. A mode chosen on the host's command line,
/// such as `opencode run --auto` or `claude --permission-mode`, is invisible
/// from here, and `Unset` says exactly that rather than claiming the default.
pub fn read_permission_mode(slug: &str, user_file: Option<&Path>, repo_dir: &Path) -> ModeReading {
    if slug == "pi" {
        return ModeReading::NoPrompts;
    }
    let files: Vec<PathBuf> = match slug {
        "claude-code" => [
            Some(repo_dir.join(".claude").join("settings.local.json")),
            Some(repo_dir.join(".claude").join("settings.json")),
            user_file.map(Path::to_path_buf),
        ]
        .into_iter()
        .flatten()
        .collect(),
        "opencode" => [
            Some(repo_dir.join("opencode.json")),
            user_file.map(Path::to_path_buf),
        ]
        .into_iter()
        .flatten()
        .collect(),
        _ => user_file.map(Path::to_path_buf).into_iter().collect(),
    };
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let found = if slug == "codex" {
            codex_policy(&text)
        } else {
            json_mode(slug, &text)
        };
        match found {
            Ok(Some(value)) => return ModeReading::Set { value, file },
            Ok(None) => {}
            Err(why) => return ModeReading::Unreadable { file, why },
        }
    }
    ModeReading::Unset
}

fn json_mode(slug: &str, text: &str) -> Result<Option<String>, String> {
    let root: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let pointer = if slug == "claude-code" {
        "/permissions/defaultMode"
    } else {
        "/permission"
    };
    Ok(root.pointer(pointer).map(|value| match value.as_str() {
        Some(text) => text.to_owned(),
        None => value.to_string(),
    }))
}

fn codex_policy(text: &str) -> Result<Option<String>, String> {
    let document: toml_edit::DocumentMut =
        text.parse().map_err(|error: toml_edit::TomlError| {
            error
                .to_string()
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned()
        })?;
    let parts: Vec<String> = ["approval_policy", "sandbox_mode"]
        .iter()
        .filter_map(|key| {
            let value = document.get(key)?.as_str()?;
            Some(format!("{key} = {value}"))
        })
        .collect();
    Ok((!parts.is_empty()).then(|| parts.join(", ")))
}

/// One row per host whose permission mode `doctor` reads, saying what the
/// mode is and that Estigia's decisions are not among the prompts it approves.
///
/// Never broken: the mode is the operator's to choose, and the row exists so
/// that it is visible, not so that it can block a run.
pub fn permission_modes(readings: &[(&'static str, ModeReading)]) -> Vec<Check> {
    let about = "each host's configured permission mode, and that Estigia's decisions are not approved by it";
    readings
        .iter()
        .map(|(slug, reading)| {
            let holds = format!(
                "{slug}: an automatic mode approves the host's own prompts, and Estigia's refusals \
                 and questions are not approved by it"
            );
            let health = match reading {
                ModeReading::Set { value, file } => {
                    let flag = if *slug == "opencode" {
                        "; `--auto` is a command-line flag and is not readable here"
                    } else if *slug == "codex" {
                        "; not measured live"
                    } else {
                        ""
                    };
                    Health::Fine {
                        detail: format!("{value} ({}); {holds}{flag}", file.display()),
                    }
                }
                ModeReading::Unset => Health::Fine {
                    detail: format!(
                        "no permission mode is set in the settings files read; a mode chosen on \
                         the command line is not readable here; {holds}"
                    ),
                },
                ModeReading::Unreadable { file, why } => Health::Skipped {
                    detail: format!(
                        "{slug}: the mode could not be read ({}: {why}); Estigia's gate decides \
                         whatever it is",
                        file.display()
                    ),
                },
                ModeReading::NoPrompts => Health::Fine {
                    detail: format!(
                        "{slug} has no permission prompts to approve; tools always run and the \
                         gate decides every call"
                    ),
                },
            };
            Check {
                name: "permission",
                about,
                health,
            }
        })
        .collect()
}

/// Whether the gate has been letting calls through without deciding on them.
///
/// One of three checks about the past rather than the present — `stale-run-pointer` and
/// `gate-version` joined it in `LOOKS_BACK`. Everything else
/// here asks whether a run
/// *could* work; this asks whether the runs that already happened were gated at all — because the
/// two ways they are not are both silent by construction.
pub fn silence(found: &Ungated) -> Check {
    let about = "whether any call reached the gate and went undecided";
    // A file that will not open answers nothing.
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
    // Sessions that could not be identified: with no payload there is no
    // session id, so nothing can be sworn in that session.
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
    if found.calls.is_empty() && !found.unminted.is_empty() {
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
            health: Health::Fine {
                detail: "every call the ledger records was decided on".to_owned(),
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
    // Which repair, not just how many: counted apart so the message can say
    // which. By verdict rather than by subtraction.
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
                 {what}{unminted}{latest}",
                ungated.len(),
                found.source.as_deref().map_or_else(
                    || "from an agent no line names".to_owned(),
                    |who| format!("from {who}"),
                ),
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

    fn pi_dirs() -> (tempfile::TempDir, tempfile::TempDir) {
        let agent = tempfile::tempdir().expect("an agent directory");
        let repo = tempfile::tempdir().expect("a repository");
        std::fs::create_dir_all(repo.path().join(".pi")).expect("a project .pi directory");
        (agent, repo)
    }

    fn pi_verdict(agent: &tempfile::TempDir, repo: &tempfile::TempDir) -> Health {
        pi_config(Some(agent.path()), repo.path(), None).health
    }

    #[test]
    fn an_agents_override_file_in_pis_agent_directory_hides_the_directive() {
        let (agent, repo) = pi_dirs();
        assert!(matches!(pi_verdict(&agent, &repo), Health::Fine { .. }));
        std::fs::write(agent.path().join("AGENTS.override.md"), "mine\n").expect("an override");
        let Health::Broken { detail, .. } = pi_verdict(&agent, &repo) else {
            panic!("an override file left the row fine");
        };
        assert!(detail.contains("AGENTS.override.md"), "{detail}");
        assert!(detail.contains("directive"), "{detail}");
    }

    #[test]
    fn pi_settings_that_exclude_the_extension_or_the_server_are_broken() {
        for (file, body, names) in [
            (
                "agent",
                r#"{"extensions":["-extensions/estigia-workflow-authority.ts"]}"#,
                "estigia-workflow-authority.ts",
            ),
            (
                "project",
                r#"{"extensions":["-../x/estigia-workflow-authority.ts"]}"#,
                "estigia-workflow-authority.ts",
            ),
            (
                "agent",
                r#"{"extensions":["!**/estigia-*.ts"]}"#,
                "!**/estigia-*.ts",
            ),
            (
                "project",
                r#"{"extensions":["!extensions/*"]}"#,
                "!extensions/*",
            ),
            (
                "agent",
                r#"{"extensions":["-extensions/estigia-review-relay.ts"]}"#,
                "estigia-review-relay.ts",
            ),
            ("agent", r#"{"extensions":["-builtin:mcp"]}"#, "builtin:mcp"),
            (
                "project",
                r#"{"extensions":["+a.ts","-builtin:mcp"]}"#,
                "builtin:mcp",
            ),
        ] {
            let (agent, repo) = pi_dirs();
            let path = if file == "agent" {
                agent.path().join("settings.json")
            } else {
                repo.path().join(".pi").join("settings.json")
            };
            std::fs::write(&path, body).expect("a settings file");
            let Health::Broken { detail, resolution } = pi_verdict(&agent, &repo) else {
                panic!("{body} in the {file} settings did not break the row");
            };
            assert!(detail.contains(names), "{detail}");
            assert!(detail.contains(&path.display().to_string()), "{detail}");
            assert!(
                matches!(resolution, Resolution::NoCommand { .. }),
                "no command clears a hand-edited settings file: {resolution:?}"
            );
        }
    }

    #[test]
    fn a_pi_subagent_runner_whose_children_skip_the_gate_is_named_and_one_that_loads_it_is_not_a_warning()
     {
        let (agent, repo) = pi_dirs();
        let verdict = |agent: &tempfile::TempDir, repo: &tempfile::TempDir| {
            pi_subagents(Some(agent.path()), repo.path()).health
        };
        let Health::Fine { detail } = verdict(&agent, &repo) else {
            panic!("no runner left the row fine");
        };
        assert!(detail.contains("no Pi subagent runner"), "{detail}");
        assert!(matches!(
            pi_subagents(None, repo.path()).health,
            Health::Skipped { detail } if detail == "pi is not configured"
        ));

        let extensions = agent.path().join("extensions");
        std::fs::create_dir_all(extensions.join("subagent-loud")).expect("a runner directory");
        std::fs::write(
            extensions.join("subagent-loud").join("index.ts"),
            "import { spawn } from \"node:child_process\";\nspawn(\"pi\", [\"-p\", task]);\n",
        )
        .expect("a runner that starts pi");
        let Health::Fine { detail } = verdict(&agent, &repo) else {
            panic!("a runner whose children load the gate was warned about");
        };
        assert!(detail.contains("subagent-loud"), "{detail}");
        assert!(detail.contains("loads the gate"), "{detail}");

        std::fs::write(
            extensions.join("subagent-quiet.ts"),
            "spawn(\"pi\", [\"--no-extensions\", \"-p\", task]);\n",
        )
        .expect("a runner that disables extensions");
        let package = repo.path().join("subagent-package");
        std::fs::create_dir_all(&package).expect("a package directory");
        std::fs::write(
            package.join("index.js"),
            "const { session } = await createAgentSession({});\n",
        )
        .expect("an in-process runner");
        std::fs::write(
            repo.path().join(".pi").join("settings.json"),
            format!(
                r#"{{"packages":["npm:pi-subagents","npm:unrelated",{{"source":{}}}]}}"#,
                serde_json::to_string(&package.display().to_string()).expect("a path")
            ),
        )
        .expect("a settings file");
        let Health::Skipped { detail } = verdict(&agent, &repo) else {
            panic!("runners whose children skip the gate were not reported");
        };
        assert!(detail.starts_with("WARNING"), "{detail}");
        assert!(
            detail.contains("subagent-quiet.ts") && detail.contains("--no-extensions"),
            "{detail}"
        );
        assert!(
            detail.contains("subagent-package") && detail.contains("inside Pi's own process"),
            "{detail}"
        );
        assert!(
            detail.contains("npm:pi-subagents") && detail.contains("unknown"),
            "{detail}"
        );
        assert!(!detail.contains("unrelated"), "{detail}");
        assert!(detail.contains("subagent-loud"), "{detail}");
    }

    #[test]
    fn a_pi_runner_is_gated_only_when_it_is_read_in_full_and_visibly_starts_pi() {
        let unknown = |files: &[(&str, String)]| {
            let (agent, repo) = pi_dirs();
            let runner = agent.path().join("extensions").join("subagent-x");
            std::fs::create_dir_all(&runner).expect("a runner directory");
            for (name, body) in files {
                std::fs::write(runner.join(name), body).expect("a runner file");
            }
            pi_subagents(Some(agent.path()), repo.path()).health
        };
        let spawn_pi = ("index.ts", "spawn(\"pi\", [\"-p\", task]);\n".to_owned());

        assert!(matches!(
            unknown(std::slice::from_ref(&spawn_pi)),
            Health::Fine { .. }
        ));
        assert!(matches!(
            unknown(&[(
                "index.ts",
                "spawn(\"/usr/local/bin/pi\", args);\n".to_owned()
            )]),
            Health::Fine { .. }
        ));

        let Health::Skipped { detail } =
            unknown(&[("index.ts", "spawn(\"git\", [\"status\"]);\n".to_owned())])
        else {
            panic!("a runner that spawns something else was called gated");
        };
        assert!(detail.contains("unknown"), "{detail}");
        assert!(matches!(
            unknown(&[("index.ts", "spawn(binary, args);\n".to_owned())]),
            Health::Skipped { .. }
        ));

        let oversized = "x".repeat(RUNNER_SOURCE_LIMIT as usize + 1);
        let Health::Skipped { detail } = unknown(&[spawn_pi.clone(), ("big.js", oversized)]) else {
            panic!("a runner with a file left unread was called gated");
        };
        assert!(detail.contains("unknown"), "{detail}");

        assert!(matches!(
            unknown(&[spawn_pi, ("notes.md", "not source".to_owned())]),
            Health::Fine { .. }
        ));
    }

    fn runner_health(populate: impl FnOnce(&Path)) -> Health {
        let (agent, repo) = pi_dirs();
        let runner = agent.path().join("extensions").join("subagent-x");
        std::fs::create_dir_all(&runner).expect("a runner directory");
        populate(&runner);
        let health = pi_subagents(Some(agent.path()), repo.path()).health;
        restore_directory_modes(&runner);
        health
    }

    fn restore_directory_modes(root: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755));
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        restore_directory_modes(&entry.path());
                    }
                }
            }
        }
        #[cfg(not(unix))]
        let _ = root;
    }

    fn starts_pi(health: &Health) -> bool {
        matches!(health, Health::Fine { detail } if detail.contains("loads the gate"))
    }

    const SPAWNS_PI: &str = "spawn(\"pi\", [\"-p\", task]);\n";

    #[test]
    fn a_pi_runner_nested_deeper_than_the_depth_limit_is_not_read_in_full() {
        let at_the_limit = runner_health(|runner| {
            std::fs::create_dir_all(runner.join("a").join("b")).expect("nested directories");
            std::fs::write(runner.join("a").join("b").join("index.ts"), SPAWNS_PI)
                .expect("a source file at the limit");
        });
        assert!(starts_pi(&at_the_limit), "{at_the_limit:?}");

        let beyond = runner_health(|runner| {
            std::fs::write(runner.join("index.ts"), SPAWNS_PI).expect("a source file");
            std::fs::create_dir_all(runner.join("a").join("b").join("c"))
                .expect("a directory beyond the limit");
        });
        let Health::Skipped { detail } = beyond else {
            panic!("a runner with a directory left unread was called gated: {beyond:?}");
        };
        assert!(detail.contains("unknown"), "{detail}");
    }

    #[cfg(unix)]
    #[test]
    fn a_pi_runner_with_a_directory_that_cannot_be_listed_is_not_called_gated() {
        use std::os::unix::fs::PermissionsExt;
        let health = runner_health(|runner| {
            std::fs::write(runner.join("index.ts"), SPAWNS_PI).expect("a source file");
            let sealed = runner.join("sealed");
            std::fs::create_dir_all(&sealed).expect("a directory");
            std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000))
                .expect("the directory is sealed");
        });
        let Health::Skipped { detail } = health else {
            panic!(
                "a runner with a directory that could not be listed was called gated: {health:?}"
            );
        };
        assert!(detail.contains("unknown"), "{detail}");
    }

    #[test]
    fn spawns_pi_reads_every_quote_style_and_every_spawn_call_and_nothing_near_them() {
        for spawned in [
            "spawn(\"pi\", args)",
            "spawn('pi', args)",
            "spawn(`pi`, args)",
            "spawn( 'pi', args)",
            "spawn(\n  \"pi\",\n  args)",
            "execFile(\"pi\", args)",
            "spawnSync('pi', args)",
            "spawn(\"/usr/local/bin/pi\", args)",
            "spawn('./node_modules/.bin/pi', args)",
            "const first = spawn(\"git\"); spawn('pi', args)",
        ] {
            assert!(spawns_pi(spawned), "{spawned} was not read as starting pi");
        }
        for near in [
            "spawn(\"pip\", args)",
            "spawn(\"pi-subagent\", args)",
            "spawn(\"api\", args)",
            "spawn(\"/usr/bin/api\", args)",
            "spawn(\"pi/\", args)",
            "spawn(\"\", args)",
            "spawn(\"pi', args)",
            "spawn(\"pi, args)",
            "spawn(pi, args)",
            "spawn(binary, [\"pi\"])",
            "exec(\"pi\", args)",
            "fork(\"pi\", args)",
            "spawn \"pi\"",
            "const pi = \"pi\";",
        ] {
            assert!(!spawns_pi(near), "{near} was read as starting pi");
        }
    }

    fn write_in(root: &Path, relative: &str, body: &str) -> PathBuf {
        let file = root.join(relative);
        std::fs::create_dir_all(file.parent().expect("a parent")).expect("a directory");
        std::fs::write(&file, body).expect("a settings file");
        file
    }

    #[test]
    fn claude_code_settings_are_read_local_then_project_then_user_and_the_first_to_set_a_mode_wins()
    {
        let repo = tempfile::tempdir().expect("a repository");
        let user = tempfile::tempdir().expect("a user directory");
        let user_file = write_in(
            user.path(),
            "settings.json",
            r#"{"permissions":{"defaultMode":"acceptEdits"}}"#,
        );
        let read = || read_permission_mode("claude-code", Some(&user_file), repo.path());
        assert_eq!(
            read(),
            ModeReading::Set {
                value: "acceptEdits".to_owned(),
                file: user_file.clone()
            }
        );

        let project = write_in(
            repo.path(),
            ".claude/settings.json",
            r#"{"permissions":{"defaultMode":"plan"}}"#,
        );
        assert_eq!(
            read(),
            ModeReading::Set {
                value: "plan".to_owned(),
                file: project.clone()
            }
        );

        let local = write_in(
            repo.path(),
            ".claude/settings.local.json",
            r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#,
        );
        assert_eq!(
            read(),
            ModeReading::Set {
                value: "bypassPermissions".to_owned(),
                file: local.clone()
            }
        );

        std::fs::write(&local, r#"{"permissions":{}}"#).expect("a local file that sets nothing");
        assert_eq!(
            read(),
            ModeReading::Set {
                value: "plan".to_owned(),
                file: project
            }
        );
    }

    #[test]
    fn a_claude_code_or_opencode_file_that_cannot_be_parsed_is_unreadable_and_stops_the_search() {
        let repo = tempfile::tempdir().expect("a repository");
        let user = tempfile::tempdir().expect("a user directory");

        let claude_user = write_in(
            user.path(),
            "claude.json",
            r#"{"permissions":{"defaultMode":"plan"}}"#,
        );
        let claude_project = write_in(repo.path(), ".claude/settings.json", "{ not json");
        let ModeReading::Unreadable { file, why } =
            read_permission_mode("claude-code", Some(&claude_user), repo.path())
        else {
            panic!("a Claude Code settings file that does not parse was read past");
        };
        assert_eq!(file, claude_project);
        assert!(!why.is_empty());

        let opencode_user = write_in(
            user.path(),
            "opencode-user.json",
            r#"{"permission":"allow"}"#,
        );
        let opencode_project = write_in(repo.path(), "opencode.json", "{ not json");
        let ModeReading::Unreadable { file, why } =
            read_permission_mode("opencode", Some(&opencode_user), repo.path())
        else {
            panic!("an OpenCode config that does not parse was read past");
        };
        assert_eq!(file, opencode_project);
        assert!(!why.is_empty());

        let broken_user = write_in(user.path(), "broken-user.json", "[");
        let empty_repo = tempfile::tempdir().expect("a repository holding nothing");
        assert!(matches!(
            read_permission_mode("opencode", Some(&broken_user), empty_repo.path()),
            ModeReading::Unreadable { file, .. } if file == broken_user
        ));
        assert_eq!(
            read_permission_mode("opencode", None, empty_repo.path()),
            ModeReading::Unset
        );
    }

    fn permission_detail(slug: &'static str, reading: ModeReading) -> (bool, String) {
        let rows = permission_modes(&[(slug, reading)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "permission");
        match &rows[0].health {
            Health::Fine { detail } => (true, detail.clone()),
            Health::Skipped { detail } => (false, detail.clone()),
            other => panic!("the permission row is never broken: {other:?}"),
        }
    }

    #[test]
    fn the_permission_row_notes_what_opencode_and_codex_cannot_tell_and_nothing_for_claude_code() {
        let set = |value: &str| ModeReading::Set {
            value: value.to_owned(),
            file: PathBuf::from("/settings/file.json"),
        };
        let (fine, opencode) = permission_detail("opencode", set("allow"));
        assert!(fine);
        assert!(
            opencode.starts_with("allow (/settings/file.json); opencode:")
                && opencode.ends_with("`--auto` is a command-line flag and is not readable here"),
            "{opencode}"
        );
        assert!(!opencode.contains("not measured live"), "{opencode}");

        let (fine, codex) = permission_detail("codex", set("approval_policy = never"));
        assert!(fine);
        assert!(
            codex.starts_with("approval_policy = never (/settings/file.json); codex:")
                && codex.ends_with("; not measured live"),
            "{codex}"
        );
        assert!(!codex.contains("--auto"), "{codex}");

        let (fine, claude) = permission_detail("claude-code", set("bypassPermissions"));
        assert!(fine);
        assert!(
            claude.ends_with("are not approved by it")
                && !claude.contains("--auto")
                && !claude.contains("not measured live"),
            "{claude}"
        );
    }

    #[test]
    fn the_permission_row_of_an_unreadable_file_is_skipped_and_names_the_file_and_the_host() {
        for slug in ["claude-code", "opencode", "codex"] {
            let (fine, detail) = permission_detail(
                slug,
                ModeReading::Unreadable {
                    file: PathBuf::from("/settings/broken.json"),
                    why: "expected value".to_owned(),
                },
            );
            assert!(!fine, "{slug}: an unreadable file was reported as read");
            assert!(
                detail.starts_with(&format!("{slug}: the mode could not be read"))
                    && detail.contains("/settings/broken.json: expected value"),
                "{detail}"
            );
        }
        let (fine, unset) = permission_detail("claude-code", ModeReading::Unset);
        assert!(fine);
        assert!(unset.starts_with("no permission mode is set"), "{unset}");
        let (fine, pi) = permission_detail("pi", ModeReading::NoPrompts);
        assert!(fine);
        assert!(pi.starts_with("pi has no permission prompts"), "{pi}");
    }

    #[test]
    fn a_project_mcp_override_that_turns_the_server_off_or_hides_it_is_broken() {
        for body in [
            r#"{"mcpServers":{"estigia":{"enabled":false}}}"#,
            r#"{"mcpServers":{"estigia":{"exposure":"hidden"}}}"#,
            r#"{"mcpServers":{"estigia":{"exposure":"codemode","enabled":true}}}"#,
        ] {
            let (agent, repo) = pi_dirs();
            std::fs::write(repo.path().join(".pi").join("mcp.json"), body).expect("an mcp file");
            let Health::Broken { detail, .. } = pi_verdict(&agent, &repo) else {
                panic!("{body} did not break the row");
            };
            assert!(detail.contains("mcp.json"), "{detail}");
        }
    }

    fn pi_exposure_detail(body: Option<&str>, version: Option<crate::setup::PiVersion>) -> String {
        let (agent, repo) = pi_dirs();
        if let Some(body) = body {
            std::fs::write(agent.path().join("mcp.json"), body).expect("an mcp file");
        }
        let Health::Fine { detail } = pi_config(Some(agent.path()), repo.path(), version).health
        else {
            panic!("an exposure alone broke the row");
        };
        detail
    }

    #[test]
    fn the_row_names_the_exposure_and_the_pi_version_and_warns_on_direct_under_1_2_0() {
        let at = |minor| crate::setup::PiVersion::parse(&format!("1.{minor}.0"));
        let direct = r#"{"mcpServers":{"estigia":{"exposure":"direct"}}}"#;
        let deferred = r#"{"mcpServers":{"estigia":{"exposure":"deferred"}}}"#;

        let warned = pi_exposure_detail(Some(direct), at(2));
        assert!(
            warned.contains("`direct`") && warned.contains("Pi 1.2.0"),
            "{warned}"
        );
        assert!(warned.contains("warning"), "{warned}");

        for (body, version, exposure) in [
            (Some(direct), at(1), "`direct`"),
            (Some(deferred), at(2), "`deferred`"),
            (Some(direct), None, "`direct`"),
            (None, at(2), "not set"),
        ] {
            let detail = pi_exposure_detail(body, version);
            assert!(detail.contains(exposure), "{detail}");
            assert!(!detail.contains("warning"), "{detail}");
        }
        let unread = pi_exposure_detail(Some(direct), None);
        assert!(unread.contains("unreadable version"), "{unread}");
    }

    #[test]
    fn a_project_mcp_override_that_defers_the_server_is_not_a_finding() {
        let (agent, repo) = pi_dirs();
        std::fs::write(
            repo.path().join(".pi").join("mcp.json"),
            r#"{"mcpServers":{"estigia":{"exposure":"deferred"}}}"#,
        )
        .expect("an mcp file");
        assert!(matches!(pi_verdict(&agent, &repo), Health::Fine { .. }));
    }

    #[test]
    fn pi_settings_that_leave_estigia_loaded_are_fine_and_nothing_is_rewritten() {
        let (agent, repo) = pi_dirs();
        let settings = r#"{"extensions":["-extensions/other.ts","!vendor/**","+builtin:mcp"],"skills":["-estigia-workflow-authority.ts"]}"#;
        let mcp = r#"{"mcpServers":{"estigia":{"exposure":"direct","enabled":true},"other":{"enabled":false}}}"#;
        std::fs::write(agent.path().join("settings.json"), settings).expect("settings");
        std::fs::write(repo.path().join(".pi").join("mcp.json"), mcp).expect("mcp");
        assert!(matches!(pi_verdict(&agent, &repo), Health::Fine { .. }));
        assert_eq!(
            std::fs::read_to_string(agent.path().join("settings.json")).expect("read back"),
            settings
        );

        let empty = tempfile::tempdir().expect("an empty pair");
        assert!(matches!(pi_verdict(&empty, &empty), Health::Fine { .. }));
        std::fs::write(agent.path().join("settings.json"), "{ not json").expect("settings");
        assert!(matches!(pi_verdict(&agent, &repo), Health::Fine { .. }));
        assert!(matches!(
            pi_config(None, repo.path(), None).health,
            Health::Skipped { .. }
        ));
    }

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
        // clears the row, and `--lift` removes the record. It is not a
        // `Resolution::run`, because an agent is refused that command by the
        // `LIFT` row and doctor cannot tell its caller; the person is handed it
        // in the detail instead.
        assert!(
            matches!(
                &resolution,
                Resolution::NoCommand {
                    reason: NoCommandReason::OperatorKnowledge,
                    detail,
                } if detail.contains("--lift") && detail.contains("`!`")
            ),
            "the row named a runnable command for a person-only lift: {resolution:?}"
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
            unminted: vec![("claude-code".to_owned(), 1_785_904_685)],
            ..Ungated::default()
        };
        let Health::Broken { detail, .. } = silence(&found).health else {
            panic!("a session nothing could identify is not reported at all");
        };
        assert!(
            detail.contains("claude-code"),
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

    #[test]
    fn every_name_that_looks_back_is_one_doctor_reports() {
        for name in LOOKS_BACK {
            assert!(
                CHECKS.contains(name),
                "{name:?} looks back but is not a check this build runs"
            );
        }
    }

    #[test]
    fn the_silence_names_who_and_when_in_words_a_person_reads() {
        let found = Ungated {
            calls: vec![("payload-absent".to_owned(), 1_785_904_685)],
            source: Some("claude-code's pre-tool-use hook".to_owned()),
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
            detail.contains("claude-code's pre-tool-use hook"),
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
        // store path, it gives a relative one. That is what made defaulting
        // the path worse than imprecise — the reader went to the operator's
        // working directory and answered about whatever it found there.
        let nowhere = std::path::PathBuf::default();
        assert!(
            !crate::transport::store::beside(&nowhere).is_absolute(),
            "an empty state root no longer yields a relative store path — \
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
        // record. Every way of failing to read the store arrived here as the
        // same empty list, and came back out as "every call the ledger records
        // was decided on" — a clean bill of health issued by a reader that read
        // nothing. Driven through `ungated_calls` rather than the pure function
        // alone, because the losses were all in the reading.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        let path = crate::transport::store::beside(&state);

        // Nothing has run: not a fault, and not a clean bill either.
        let nothing = ungated_calls(&state);
        assert!(nothing.missing, "an absent store was not noticed as absent");
        assert!(
            matches!(silence(&nothing).health, Health::Skipped { .. }),
            "a machine that has run nothing was given a clean bill of health"
        );

        // There and unopenable. A directory in its place fails the open with
        // something other than absence, on every platform.
        std::fs::create_dir_all(&path).expect("something unreadable in its place");
        let shut = ungated_calls(&state);
        assert!(
            shut.unreadable.is_some(),
            "an unopenable store read as empty"
        );
        assert!(
            silence(&shut).health.is_broken(),
            "a store nothing can open was reported as one holding nothing"
        );
        std::fs::remove_dir(&path).expect("clear the way for a real one");

        // A decided call and an undecided one. The undecided one still surfaces
        // when its timestamp will not parse: the verdict is the finding, the
        // time only dresses the sentence.
        crate::harness::session::record(
            &state,
            &serde_json::json!({ "verdict": "allow", "at": 1 }),
        );
        crate::harness::session::record(
            &state,
            &serde_json::json!({ "verdict": "payload-absent" }),
        );
        let undated = ungated_calls(&state);
        assert_eq!(
            undated.calls.len(),
            1,
            "a call that went ungated was dropped for having no usable timestamp"
        );
        assert!(silence(&undated).health.is_broken());
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
            shadowed_local: None,
            duplicated: Vec::new(),
            unknown: Vec::new(),
            inherited: Vec::new(),
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
            shadowed_local: None,
            duplicated: Vec::new(),
            unknown: Vec::new(),
            inherited: Vec::new(),
        }]);
        assert!(
            !current[0].health.is_broken(),
            "an up-to-date contract was reported broken: {:?}",
            current[0].health
        );
    }

    #[test]
    fn a_skill_that_is_not_this_binarys_copy_is_reported_as_one() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let skill = root.path().join(crate::skill::DIRECTORY);
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");

        let fresh = examine(Some(&skill), root.path());
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

        let drifted = examine(Some(&skill), root.path());
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
    fn a_skill_that_cannot_be_judged_is_not_reported_as_current() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let skill = root.path().join(crate::skill::DIRECTORY);
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");

        // A value no binary recognises, in the row that carries it. The files
        // are all present, so this is unjudgeable rather than absent — and
        // `sync` refuses such a contract unchanged, so it must not be named.
        let contract = skill.join(crate::skill::CONTRACT);
        let text = std::fs::read_to_string(&contract).expect("the contract is installed");
        let drifted = text
            .lines()
            .map(|line| {
                if line.starts_with("| Integration mode |") {
                    "| Integration mode | banana |"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(drifted, text, "the fixture left nothing to judge");
        std::fs::write(&contract, drifted).expect("an unrecognised value");

        let checks = examine(Some(&skill), root.path());
        let row = checks
            .iter()
            .find(|check| check.name == "skill")
            .expect("the skill is checked");
        let Health::Broken { detail, resolution } = &row.health else {
            panic!(
                "an install that cannot be judged was reported as current: {:?}",
                row.health
            );
        };
        assert!(
            detail.contains("cannot be judged"),
            "behind and unreadable read the same: {detail}"
        );
        assert!(
            detail.contains("row"),
            "the row that carries the unrecognised value is not named: {detail}"
        );
        // No runnable discharger: `sync` refuses this contract unchanged, so
        // unlike the stale row this one must not name it as the way out —
        // only as what follows the row fix.
        assert!(
            !format!("{resolution:?}").starts_with("Run"),
            "sync refuses this contract unchanged, so it must not be named as the discharge: {resolution:?}"
        );
    }

    #[test]
    fn installed_assets_report_the_build_that_installed_them() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let skill = root.path().join(crate::skill::DIRECTORY);
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");

        // A fresh install records this binary's own aggregate.
        let current = skill_assets(&skill);
        assert!(
            !current.health.is_broken(),
            "a fresh install was reported stale: {:?}",
            current.health
        );

        // An older build's install: the recorded aggregate differs.
        let record = crate::skill::record::path(&skill);
        let mut stored: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&record).expect("the record reads"))
                .expect("the record parses");
        stored["assets"] = serde_json::json!("sha256:0");
        std::fs::write(
            &record,
            serde_json::to_string_pretty(&stored).expect("renders"),
        )
        .expect("an older install");
        let stale = skill_assets(&skill);
        let Health::Broken { detail, resolution } = &stale.health else {
            panic!(
                "an older build's install was not reported stale: {:?}",
                stale.health
            );
        };
        assert!(
            detail.contains("older build"),
            "stale assets read as something else: {detail}"
        );
        assert!(
            format!("{resolution:?}").contains("estigia sync"),
            "stale assets name no remedy: {resolution:?}"
        );

        // No record at all: unknown, neither current nor stale.
        let bare = tempfile::tempdir().expect("a directory with no install");
        let unknown = skill_assets(bare.path());
        assert!(
            matches!(unknown.health, Health::Skipped { .. }),
            "no record read as an answer: {:?}",
            unknown.health
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
        // A throwaway checkout: the push guard lives in `.git/hooks`, so there
        // is no way to report on it without one.
        let repo = git_repo();
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

    #[test]
    fn an_override_row_whose_setting_is_gone_is_surfaced_and_the_others_still_apply() {
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

        let local = paths.skill_root.join(crate::config::LOCAL_FILE);
        std::fs::write(
            &local,
            "| Setting | Value here |\n|---|---|\n\
             | Evidence standard | measuring |\n\
             | Blind judges | hunter2-doctor-value |\n",
        )
        .expect("the operator's own file");

        let held = crate::skill::installed_config_for(&paths.skill_root, Some(adapter.slug))
            .expect("the table reads");
        assert_eq!(
            held.evidence,
            crate::config::Evidence::Measuring,
            "a row whose setting is gone took a row that resolves with it"
        );

        let details: Vec<String> = full(
            Some(&paths.skill_root),
            home.path(),
            &crate::config::Tracker::Github { repo: None },
            &options,
        )
        .into_iter()
        .filter(|check| check.name == "contract")
        .filter_map(|check| match check.health {
            Health::Fine { detail } | Health::Broken { detail, .. } => Some(detail),
            Health::Skipped { .. } => None,
        })
        .collect();
        assert!(
            details.iter().any(|detail| detail.contains("Blind judges")),
            "the row no setting resolves is not on the report: {details:#?}"
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.contains(&local.display().to_string())),
            "the report does not name the file the row is in: {details:#?}"
        );
        assert!(
            !format!("{details:#?}").contains("hunter2-doctor-value"),
            "a value from the operator's file reached the report: {details:#?}"
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
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo_dir.to_path_buf());
        run
    }

    /// A real repository, because [`same_git_repository`](super::super::same_git_repository)
    /// — what this row's filter is built on — runs `git rev-parse` and a bare
    /// directory answers neither side of that comparison.
    ///
    /// It panics rather than answering `None`, the repair it was given:
    /// this is a git workflow harness, so a machine without a usable `git`
    /// cannot measure these rows at all, and a `None` the caller turns into
    /// `return;` after an `eprintln!` reports an unmeasured test as `ok`.
    fn git_repo() -> tempfile::TempDir {
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

    fn track(repo: &Path, files: &[(&str, &str)]) {
        for (name, text) in files {
            let path = repo.join(name);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            std::fs::write(&path, text).expect("a file");
        }
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["add", "--all"])
            .status()
            .expect("git add");
        assert!(status.success());
    }

    #[test]
    fn an_undeclared_tracked_lockfile_is_named_with_its_path_and_no_command() {
        let repo = git_repo();
        track(
            repo.path(),
            &[("Cargo.lock", "x"), ("web/package-lock.json", "y")],
        );
        let check = lockfiles(repo.path()).expect("a row for undeclared lockfiles");
        assert_eq!(check.name, "lockfiles");
        let Health::Skipped { detail } = check.health else {
            panic!("an advisory row is not broken: {:?}", check.health);
        };
        for line in [
            "[[generated]]",
            "path = \"Cargo.lock\"",
            "path = \"web/package-lock.json\"",
            "cwd = \"web\"",
        ] {
            assert!(
                detail.lines().any(|found| found.trim() == line),
                "{line:?} is not a line of {detail}"
            );
        }
        for command in ["generate-lockfile", "npm install", "package-lock-only"] {
            assert!(
                !detail.contains(command),
                "{command:?} is named in {detail}"
            );
        }
        assert!(
            !detail
                .lines()
                .any(|line| line.trim().starts_with("command =")),
            "{detail}"
        );
    }

    #[test]
    fn an_unreadable_declaration_is_said_so_and_names_no_lockfile_as_undeclared() {
        let repo = git_repo();
        track(
            repo.path(),
            &[
                ("Cargo.lock", "x"),
                ("estigia.generated.toml", "[[generated]\npath = "),
            ],
        );
        let Some(Check {
            name,
            health: Health::Skipped { detail },
            ..
        }) = lockfiles(repo.path())
        else {
            panic!("a skipped row for the unreadable declaration");
        };
        assert_eq!(name, "lockfiles");
        assert!(detail.contains("could not be read"), "{detail}");
        assert!(detail.contains("not valid TOML"), "{detail}");
        for absent in ["[[generated]]", "path =", "not declared", "Cargo.lock"] {
            assert!(!detail.contains(absent), "{absent:?} is in {detail}");
        }
    }

    #[test]
    fn a_declaration_that_cannot_be_opened_is_said_so() {
        let repo = git_repo();
        track(repo.path(), &[("Cargo.lock", "x")]);
        std::fs::create_dir(repo.path().join("estigia.generated.toml")).expect("a directory");
        let Some(Check {
            health: Health::Skipped { detail },
            ..
        }) = lockfiles(repo.path())
        else {
            panic!("a skipped row");
        };
        assert!(detail.contains("could not be read"), "{detail}");
        assert!(!detail.contains("not declared"), "{detail}");
    }

    #[test]
    fn an_unreadable_declaration_with_no_lockfile_has_no_row() {
        let repo = git_repo();
        track(
            repo.path(),
            &[
                ("src/main.rs", "x"),
                ("estigia.generated.toml", "[[generated]"),
            ],
        );
        assert_eq!(lockfiles(repo.path()), None);
    }

    #[test]
    fn the_undeclared_row_says_no_command_is_suggested() {
        let repo = git_repo();
        track(repo.path(), &[("Cargo.lock", "x")]);
        let Some(Check {
            health: Health::Skipped { detail },
            ..
        }) = lockfiles(repo.path())
        else {
            panic!("a skipped row");
        };
        assert!(
            detail.contains(
                "none is suggested here, because the usual ones re-resolve dependencies instead"
            ),
            "{detail}"
        );
    }

    #[test]
    fn a_declared_lockfile_has_no_row() {
        let repo = git_repo();
        track(
            repo.path(),
            &[
                ("Cargo.lock", "x"),
                (
                    "estigia.generated.toml",
                    "[[generated]]\npath = \"Cargo.lock\"\ncommand = [\"cargo\", \"generate-lockfile\"]\n",
                ),
            ],
        );
        assert_eq!(lockfiles(repo.path()), None);
    }

    #[test]
    fn a_repository_with_no_lockfile_has_no_row() {
        let repo = git_repo();
        track(repo.path(), &[("src/main.rs", "fn main() {}")]);
        assert_eq!(lockfiles(repo.path()), None);
    }

    #[test]
    fn an_untracked_lockfile_has_no_row() {
        let repo = git_repo();
        std::fs::write(repo.path().join("Cargo.lock"), "x").expect("a file");
        assert_eq!(lockfiles(repo.path()), None);
    }

    #[test]
    fn only_the_undeclared_lockfile_is_named() {
        let repo = git_repo();
        track(
            repo.path(),
            &[
                ("Cargo.lock", "x"),
                ("go.sum", "y"),
                (
                    "estigia.generated.toml",
                    "[[generated]]\npath = \"Cargo.lock\"\ncommand = [\"cargo\", \"generate-lockfile\"]\n",
                ),
            ],
        );
        let Some(Check {
            health: Health::Skipped { detail },
            ..
        }) = lockfiles(repo.path())
        else {
            panic!("a row for go.sum");
        };
        assert!(
            detail.contains("go.sum") && !detail.contains("Cargo.lock"),
            "{detail}"
        );
    }

    #[test]
    fn an_undeclared_tracked_lockfile_has_no_row_when_the_directory_is_not_a_repository() {
        let directory = tempfile::tempdir().expect("a directory");
        std::fs::write(directory.path().join("Cargo.lock"), "x").expect("a file");
        assert_eq!(lockfiles(directory.path()), None);
    }

    #[test]
    fn an_older_binary_than_the_checkout_is_reported_broken_with_a_reinstall() {
        let health = gate_age_verdict(
            Path::new("/some/checkout"),
            Some("1111111"),
            false,
            Some("cf9ab5a"),
            Some(false),
            Some(3),
        );
        let Health::Broken { detail, resolution } = health else {
            panic!("a stale gate was not reported broken: {health:?}");
        };
        assert!(
            detail.contains("not established as current"),
            "the staleness was not the headline of the detail: {detail}"
        );
        assert!(
            detail.contains("cf9ab5a"),
            "the checkout's head should be named so the operator sees what they are behind: {detail}"
        );
        let Resolution::Run { command } = resolution else {
            panic!("a stale gate needs a command to repair it, not a reason it cannot have one");
        };
        // The command must replace the executable from the checkout the row is
        // about rather than the process's current directory, and — under the
        // Windows constraint the issue hands down — must stop Estigia first,
        // through a shell that can actually run a cmdlet.
        assert!(
            command.contains("cargo install --path '/some/checkout' --force"),
            "the command must reinstall from the checkout the row is about: {command}"
        );
        if cfg!(windows) {
            assert!(
                command.starts_with("powershell"),
                "a cmdlet is being printed for a shell that cannot run one: {command}"
            );
        } else {
            assert_eq!(
                command, "cargo install --path '/some/checkout' --force",
                "a non-Windows reinstall command should not open with a PowerShell cmdlet nothing \
                 there can run: {command}"
            );
        }
    }

    /// A `doctor` resolution is a `Health::Broken`, not a `Refusal`, so the
    /// refusal ratchet never sees it; and this one is built from a value rather
    /// than a literal, so the source scan cannot read it either. Its first
    /// token must still be a program this machine can run.
    #[test]
    fn the_reinstall_command_names_a_program_that_runs() {
        let command = reinstall_command(Path::new("/some/checkout"));
        let program = command.split_whitespace().next().expect("a first token");
        assert!(
            std::process::Command::new(program)
                .arg("--version")
                .output()
                .is_ok(),
            "the reinstall command opens with `{program}`, which is not on the path: {command}"
        );
    }

    #[test]
    fn a_broken_verdict_names_the_distance_when_it_is_known() {
        let health = gate_age_verdict(
            Path::new("."),
            Some("1111111"),
            false,
            Some("cf9ab5a"),
            Some(false),
            Some(7),
        );
        let Health::Broken { detail, .. } = health else {
            panic!("a stale gate was not reported broken: {health:?}");
        };
        assert!(
            detail.contains('7') && detail.contains("commit"),
            "the count of commits the checkout holds that the build lacks should be on the record: {detail}"
        );
    }

    #[test]
    fn a_broken_verdict_says_the_distance_could_not_be_read_rather_than_current() {
        let health = gate_age_verdict(
            Path::new("."),
            Some("1111111"),
            false,
            Some("cf9ab5a"),
            Some(false),
            None,
        );
        let Health::Broken { detail, .. } = health else {
            panic!("a stale gate was not reported broken: {health:?}");
        };
        assert!(
            detail.contains("could not be read"),
            "an unread distance should say so rather than be silently omitted: {detail}"
        );
        assert!(
            !detail.contains("is current"),
            "an unread distance must never render as though the build were current: {detail}"
        );
    }

    #[test]
    fn a_dirty_build_is_reported_skipped_not_stale() {
        let health = gate_age_verdict(
            Path::new("."),
            Some("cf9ab5a"),
            true,
            Some("cf9ab5a"),
            Some(true),
            None,
        );
        let Health::Skipped { detail } = health else {
            panic!("a dirty build must be cannot-tell, not a verdict: {health:?}");
        };
        assert!(
            detail.contains("uncommitted changes"),
            "the reason the comparison cannot be made should be on the record: {detail}"
        );
    }

    #[test]
    fn a_binary_with_no_commit_is_reported_skipped_not_current() {
        let health = gate_age_verdict(
            Path::new("."),
            None,
            false,
            Some("cf9ab5a"),
            Some(false),
            None,
        );
        let Health::Skipped { detail } = health else {
            panic!("a binary with no commit must be cannot-tell: {health:?}");
        };
        assert!(
            detail.contains("carries no commit"),
            "the reason the comparison cannot be made should be on the record: {detail}"
        );
    }

    #[test]
    fn a_checkout_with_no_head_is_reported_skipped_not_stale() {
        let health = gate_age_verdict(
            Path::new("."),
            Some("cf9ab5a"),
            false,
            None,
            Some(false),
            None,
        );
        let Health::Skipped { detail } = health else {
            panic!("a directory with no head must be cannot-tell: {health:?}");
        };
        assert!(
            detail.contains("could not be read"),
            "the reason should say the directory has no head to compare to: {detail}"
        );
    }

    #[test]
    fn an_unreadable_ancestry_is_reported_skipped_not_stale() {
        let health = gate_age_verdict(
            Path::new("."),
            Some("cf9ab5a"),
            false,
            Some("1111111"),
            None,
            None,
        );
        let Health::Skipped { detail } = health else {
            panic!(
                "an ancestry git could not answer must be cannot-tell, not a verdict: {health:?}"
            );
        };
        assert!(
            detail.contains("cf9ab5a") && detail.contains("1111111"),
            "the reason should name both commits it could not compare: {detail}"
        );
    }

    #[test]
    fn a_binary_current_with_the_checkout_is_reported_fine() {
        let health = gate_age_verdict(
            Path::new("."),
            Some("cf9ab5a"),
            false,
            Some("cf9ab5a"),
            Some(true),
            None,
        );
        let Health::Fine { detail } = health else {
            panic!("a current binary was not reported fine: {health:?}");
        };
        assert!(
            detail.contains("current against the checkout"),
            "the fine report should say it is current: {detail}"
        );
    }

    #[test]
    fn gate_version_reports_a_row_against_a_real_checkout() {
        let repo = git_repo();
        let check = gate_version(repo.path());
        assert_eq!(check.name, "gate-version");
        assert!(
            matches!(check.health, Health::Skipped { .. }),
            "a repository with no commits cannot be compared against, so the row must say so \
             rather than pass or break: {:?}",
            check.health
        );
    }

    #[test]
    fn the_row_still_explains_a_commit_the_repository_does_not_hold() {
        let repo = git_repo();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .status()
                .is_ok_and(|status| status.success())
        };
        assert!(
            git(&["config", "user.email", "gate-version@estigia.test"])
                && git(&["config", "user.name", "estigia"])
                && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]),
            "the fixture could not commit"
        );
        let foreign = "1111111111111111111111111111111111111111";
        let check = gate_version_for(repo.path(), Some(foreign), false);
        assert!(
            matches!(&check.health, Health::Skipped { detail } if detail.contains("different repository")),
            "{:?}",
            check.health
        );
        assert!(commit_is_foreign_to(repo.path(), Some(foreign), false));
        assert!(!commit_is_foreign_to(repo.path(), Some(foreign), true));
        assert!(!commit_is_foreign_to(repo.path(), None, false));
    }

    #[test]
    fn git_is_ancestor_answers_a_real_ancestor_pair() {
        let repo = git_repo();
        let git = |args: &[&str]| -> bool {
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .status()
                .is_ok_and(|status| status.success())
        };
        let rev_parse_head = || -> String {
            String::from_utf8(
                Command::new("git")
                    .arg("-C")
                    .arg(repo.path())
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .expect("git runs")
                    .stdout,
            )
            .expect("git prints utf-8")
            .trim()
            .to_owned()
        };
        let configured = git(&["config", "user.email", "gate-version@estigia.test"])
            && git(&["config", "user.name", "estigia"])
            && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]);
        assert!(
            configured,
            "this fixture needs a usable `git` that can commit in a disposable repository, so \
             the row under test can be measured at all; a test that reports `ok` without running \
             is a defect this repository has filed against itself"
        );
        let first = rev_parse_head();
        assert!(
            git(&["commit", "--quiet", "--allow-empty", "-m", "second"]),
            "the second commit could not be made"
        );
        let second = rev_parse_head();

        assert_eq!(
            git_is_ancestor(repo.path(), &first, &second),
            Some(true),
            "the first commit is a real ancestor of the second, and the check must say so"
        );
        assert_eq!(
            git_is_ancestor(repo.path(), &second, &first),
            Some(false),
            "the second commit is not an ancestor of the first, and the check must say so"
        );
    }

    #[test]
    fn git_commits_ahead_counts_real_commits_and_answers_none_off_the_repository() {
        let repo = git_repo();
        let git = |args: &[&str]| -> bool {
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .status()
                .is_ok_and(|status| status.success())
        };
        let rev_parse_head = || -> String {
            String::from_utf8(
                Command::new("git")
                    .arg("-C")
                    .arg(repo.path())
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .expect("git runs")
                    .stdout,
            )
            .expect("git prints utf-8")
            .trim()
            .to_owned()
        };
        let configured = git(&["config", "user.email", "gate-version@estigia.test"])
            && git(&["config", "user.name", "estigia"])
            && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]);
        assert!(
            configured,
            "this fixture needs a usable `git` that can commit in a disposable repository, so \
             the row under test can be measured at all; a test that reports `ok` without running \
             is a defect this repository has filed against itself"
        );
        let first = rev_parse_head();
        assert!(
            git(&["commit", "--quiet", "--allow-empty", "-m", "second"])
                && git(&["commit", "--quiet", "--allow-empty", "-m", "third"]),
            "the two commits ahead of `first` could not be made"
        );

        assert_eq!(
            git_commits_ahead(repo.path(), &first),
            Some(2),
            "HEAD is two real commits ahead of `first`, and the count must say so"
        );
        assert_eq!(
            git_commits_ahead(repo.path(), "0000000000000000000000000000000000000000"),
            None,
            "a commit the repository does not hold makes the command fail, and a failure must not \
             be reported as a distance"
        );
    }

    #[test]
    fn gate_version_reports_fine_and_genuinely_stale_against_real_commits() {
        let Some(commit) = crate::build_info::commit() else {
            eprintln!("SKIPPED: this build carries no commit.");
            return;
        };
        if crate::build_info::dirty() {
            eprintln!("SKIPPED: this build came from an uncommitted tree.");
            return;
        }
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

        let fine = gate_version(manifest_dir);
        assert!(
            matches!(fine.health, Health::Fine { .. }),
            "this binary's own checkout was not reported current: {:?}",
            fine.health
        );

        let Ok(scratch) = tempfile::tempdir() else {
            eprintln!("SKIPPED: no scratch directory available.");
            return;
        };
        let cloned = Command::new("git")
            .args(["clone", "--quiet"])
            .arg(manifest_dir)
            .arg(scratch.path())
            .status();
        assert!(
            matches!(cloned, Ok(status) if status.success()),
            "this fixture needs a usable `git` that can clone the checkout, so the row under \
             test can be measured at all; a test that reports `ok` without running is the defect \
             this repository has filed against itself"
        );
        let git = |args: &[&str]| -> bool {
            Command::new("git")
                .arg("-C")
                .arg(scratch.path())
                .args(args)
                .status()
                .is_ok_and(|status| status.success())
        };
        // A local `git clone` of a shallow source (CI checks out this crate
        // itself at `fetch-depth: 1`) disables the hardlink optimisation and
        // can leave the clone without this binary's own commit object at all
        // — the same *cannot tell* this test exists to rule out, reached
        // through the clone rather than through `git_is_ancestor`. Ask the
        // question the assertion below assumes rather than let `gate_version`
        // answer `Skipped` where the test expects `Broken`.
        if !git(&["cat-file", "-e", commit]) {
            eprintln!(
                "SKIPPED: the clone does not hold this binary's own commit ({commit}), most \
                 likely because the source checkout is itself shallow."
            );
            return;
        }
        let configured = git(&["config", "user.email", "gate-version@estigia.test"])
            && git(&["config", "user.name", "estigia"])
            && git(&[
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "a real commit ahead of the binary under test",
            ]);
        assert!(
            configured,
            "this fixture needs a usable `git` that can commit in the clone, so the row under \
             test can be measured at all; a test that reports `ok` without running is the defect \
             this repository has filed against itself"
        );

        let stale = gate_version(scratch.path());
        let Health::Broken { detail, .. } = stale.health else {
            panic!(
                "a checkout genuinely ahead of this binary's own commit was not reported stale: {:?}",
                stale.health
            );
        };
        assert!(
            detail.contains(commit),
            "the stale report should name this binary's own commit: {detail}"
        );
        // The clone made exactly one commit past the binary's own, over a real
        // `git rev-list --count` rather than a fed value.
        assert!(
            detail.contains("1 commit"),
            "a checkout genuinely one real commit ahead should have that distance named: {detail}"
        );
    }

    #[test]
    fn a_broken_verdict_over_a_real_git_commits_ahead_names_the_distance() {
        // Unlike the test above, this does not depend on this binary's own
        // commit or dirty state: it feeds `gate_age_verdict` the real
        // `git_commits_ahead` answer over a disposable repository rather than
        // this checkout's own build.
        let repo = git_repo();
        let git = |args: &[&str]| -> bool {
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .status()
                .is_ok_and(|status| status.success())
        };
        let rev_parse_head = || -> String {
            String::from_utf8(
                Command::new("git")
                    .arg("-C")
                    .arg(repo.path())
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .expect("git runs")
                    .stdout,
            )
            .expect("git prints utf-8")
            .trim()
            .to_owned()
        };
        let configured = git(&["config", "user.email", "gate-version@estigia.test"])
            && git(&["config", "user.name", "estigia"])
            && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]);
        assert!(
            configured,
            "this fixture needs a usable `git` that can commit in a disposable repository, so \
             the row under test can be measured at all; a test that reports `ok` without running \
             is a defect this repository has filed against itself"
        );
        let first = rev_parse_head();
        assert!(
            git(&["commit", "--quiet", "--allow-empty", "-m", "second"]),
            "the commit ahead of `first` could not be made"
        );
        let second = rev_parse_head();

        let ahead = git_commits_ahead(repo.path(), &first);
        let health = gate_age_verdict(
            Path::new("."),
            Some(&first),
            false,
            Some(&second),
            Some(false),
            ahead,
        );
        let Health::Broken { detail, .. } = health else {
            panic!("a stale gate was not reported broken: {health:?}");
        };
        assert!(
            detail.contains("1 commit"),
            "one real commit ahead, read through the real git half, should be named: {detail}"
        );
    }

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

    #[test]
    fn a_missing_skill_root_is_skipped_for_stale_pointers() {
        let root = tempfile::tempdir().expect("a temporary root");
        let check = stale_run_pointers(None, root.path(), &github(), &[]);
        assert_eq!(check.name, "stale-run-pointer");
        assert!(
            matches!(check.health, Health::Skipped { .. }),
            "a machine with no skill root to ask the tracker through was reported as something \
             else: {:?}",
            check.health
        );
    }

    #[test]
    fn a_readable_stale_pointer_is_reported_and_names_the_release_command() {
        let repo = git_repo();
        install_contract(repo.path());
        let run = holder("claude-stale0000", 12, repo.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
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

    #[test]
    fn a_readable_live_pointer_is_reported_fine() {
        let repo = git_repo();
        install_contract(repo.path());
        let run = holder("claude-live00000", 34, repo.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::open_but_unmatched_issue(34)]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
        });

        assert!(
            matches!(check.health, Health::Fine { .. }),
            "a pointer whose issue is open was reported stale: {:?}",
            check.health
        );
    }

    fn assisted_holder(repo: &Path, worktree: &Path) -> super::super::session::Run {
        let mut run = holder("claude-assist0000", 12, repo);
        run.assisted = true;
        run.branch = Some("issue-12".to_owned());
        run.worktree = Some(worktree.to_path_buf());
        run
    }

    #[test]
    fn an_assisted_pointer_is_never_sent_to_the_tracker_and_one_whose_worktree_is_gone_is_flagged()
    {
        let repo = git_repo();
        install_contract(repo.path());
        let present = tempfile::tempdir().expect("a worktree directory");
        let bin = crate::test_env::scripted_gh();
        let log = repo.path().join("gh.log");
        let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
        let check = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            stale_run_pointers(
                Some(repo.path()),
                repo.path(),
                &github(),
                &[assisted_holder(repo.path(), present.path())],
            )
        });
        assert!(
            matches!(check.health, Health::Fine { .. }),
            "an assisted pointer was read against the tracker: {:?}",
            check.health
        );
        assert!(
            std::fs::read_to_string(&log)
                .unwrap_or_default()
                .trim()
                .is_empty(),
            "the tracker was asked about a pointer it never held"
        );

        let gone = repo.path().join("no-such-worktree");
        let check = stale_run_pointers(
            Some(repo.path()),
            repo.path(),
            &github(),
            &[assisted_holder(repo.path(), &gone)],
        );
        let Health::Broken { detail, resolution } = &check.health else {
            panic!("a gone worktree was not reported: {:?}", check.health);
        };
        assert!(
            detail.contains("claude-assist0000")
                && detail.contains("gone")
                && detail.contains("renewal window"),
            "{detail}"
        );
        assert!(
            !resolution.to_string().contains("estigia release"),
            "the way out is a command assisted refuses: {resolution}"
        );
    }

    #[test]
    fn a_tracker_read_that_fails_is_reported_as_unread_not_stale() {
        let repo = git_repo();
        install_contract(repo.path());
        let run = holder("claude-unknown00", 56, repo.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
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

    #[test]
    fn a_pointer_for_another_repository_is_not_named() {
        let repo = git_repo();
        let elsewhere = git_repo();
        install_contract(repo.path());
        // Closed, if it were ever asked about — the shape most likely to be
        // silently believed rather than caught, since a false "stale" here
        // reads as an ordinary broken row rather than as an obviously wrong
        // machine-wide answer.
        let foreign = holder("claude-foreign00", 12, elsewhere.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[foreign])
        });

        assert!(
            matches!(check.health, Health::Fine { .. }),
            "a pointer for a different repository was asked about and reported: {:?}",
            check.health
        );
    }

    #[test]
    fn a_genuinely_passing_pointer_is_reported_fine() {
        let repo = git_repo();
        install_contract(repo.path());
        let run = holder("claude-passing00", 78, repo.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
            "claude-passing00",
            78,
            "in-progress",
        )]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
        });

        let Health::Fine { detail } = &check.health else {
            panic!(
                "a run pointer this run genuinely, currently holds \u{2014} the tracker's own \
                 code-0 answer \u{2014} was reported as something other than fine: {:?}",
                check.health
            );
        };
        assert!(
            detail.contains("1 of them read as still open and held by this run"),
            "the pointer's own code-0 answer was not counted as read; the row said: {detail}"
        );
    }

    /// The `Fine` sentence names exactly the row's three buckets, and denies
    /// neither state that reaches `Fine` with a closed issue behind it: an aged
    /// stranded pointer, and a label-mismatched own-delivery close. Neither is
    /// drivable through the scripted-`gh` fixtures, so the sentence is what this
    /// pins.
    #[test]
    fn the_fine_sentence_claims_only_the_buckets_the_row_empties() {
        let root = tempfile::tempdir().expect("a temporary root");
        let check = stale_run_pointers(Some(root.path()), root.path(), &github(), &[]);
        let Health::Fine { detail } = &check.health else {
            panic!(
                "a machine holding nothing was reported as something else: {:?}",
                check.health
            );
        };
        assert!(
            detail.contains("issue-not-open"),
            "the fine sentence does not name the stale bucket's own reason: {detail}"
        );
        assert!(
            !detail.contains("names an issue the tracker reports closed"),
            "the fine sentence denies a closed issue the row never asked about: {detail}"
        );
        assert!(
            !detail.contains("still holds one closed behind this run's own delivery"),
            "the fine sentence denies an own-delivery close a swallowed answer never \
             established: {detail}"
        );
    }

    #[test]
    fn own_delivery_close_reads_the_transports_own_prefix_and_nothing_else() {
        let answer = |checked: serde_json::Value| super::super::tracker::Answer {
            code: 0,
            body: Some(serde_json::json!({ "checked": checked })),
        };
        assert!(
            super::own_delivery_close(&answer(serde_json::json!([
                format!(
                    "{}deadbeef:epoch",
                    crate::transport::claim::OWN_DELIVERY_CLOSE_PREFIX
                ),
                "single-expected-state",
            ]))),
            "the transport's own `checked[0]` was not read as its own-delivery close"
        );
        assert!(
            !super::own_delivery_close(&answer(serde_json::json!(["issue-open"]))),
            "an ordinary open answer was read as an own-delivery close"
        );
        for body in [
            serde_json::json!({}),
            serde_json::json!({ "checked": "not an array" }),
            serde_json::json!({ "checked": [7, "single-expected-state"] }),
        ] {
            assert!(
                !super::own_delivery_close(&super::super::tracker::Answer {
                    code: 0,
                    body: Some(body.clone()),
                }),
                "a body with no readable `checked[0]` was read as an own-delivery close: {body}"
            );
        }
    }

    #[test]
    fn a_stale_pointer_does_not_hide_an_unread_one_in_the_same_row() {
        let repo = git_repo();
        install_contract(repo.path());
        let stale = holder("claude-stale1111", 21, repo.path());
        let unread = holder("claude-unread111", 22, repo.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[
            crate::test_env::closed_issue(21),
            crate::test_env::unreachable_tracker_answer(),
        ]);
        let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[stale, unread])
        });

        let Health::Broken { detail, resolution } = &check.health else {
            panic!(
                "one stale pointer and one unread pointer covering the same checkout were not \
                 reported broken: {:?}",
                check.health
            );
        };
        assert!(
            detail.contains("claude-stale1111") && detail.contains("#21"),
            "the stale pointer dropped out of the row once an unread one joined it: {detail}"
        );
        assert!(
            detail.contains("claude-unread111"),
            "the unread pointer was hidden behind the stale one \u{2014} the early-return \
             defect this test exists to catch: {detail}"
        );
        assert!(
            resolution.to_string().contains("estigia release --run-id")
                && resolution.to_string().contains("claude-stale1111"),
            "the row still owes the release command for the stale pointer: {resolution}"
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
        for entry in [
            serde_json::json!({"at":10,"verdict":"payload-unreadable","detail":"x: an older reason"}),
            serde_json::json!({"at":40,"verdict":"payload-unreadable","detail":"x: the newest reason"}),
            serde_json::json!({"at":20,"verdict":"payload-absent","detail":"x: a middling reason"}),
        ] {
            crate::harness::session::record(&state, &entry);
        }

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
            &[("claude-code", vec![(file.clone(), vec![dead])], true)],
            None,
            &[],
        );
        assert_eq!(checks.len(), 1);
        assert!(checks[0].health.is_broken(), "a dead gate passed");
        let Health::Broken { detail, resolution } = &checks[0].health else {
            unreachable!("just asserted")
        };
        // Names the agent, what is wrong, and the file it is wrong in — a
        // report that said only "broken" sends somebody to read four files.
        assert!(detail.contains("claude-code"), "{detail}");
        assert!(detail.contains("ungated"), "{detail}");
        assert!(detail.contains("settings.json"), "{detail}");
        // And the way out is the command that rewrites the entry with this
        // executable's real path.
        assert!(
            format!("{resolution}").contains("estigia setup claude-code"),
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
        let checks = gates(
            &[("claude-code", vec![(file, vec![live])], true)],
            None,
            &[],
        );
        assert!(
            !checks[0].health.is_broken(),
            "a live gate was called broken"
        );
    }

    #[test]
    fn a_gate_codex_will_skip_is_not_reported_live_and_names_what_clears_it() {
        use crate::setup::Skipped;
        use crate::setup::wiring::Wire;

        let root = tempfile::tempdir().expect("a temporary root");
        let executable = root.path().join("estigia");
        std::fs::write(&executable, "").expect("the executable");
        let wire = || Wire {
            command: format!("{} hook pre-tool-use", executable.display()),
            executable: executable.clone(),
            event: Some("pre-tool-use"),
            named: "pre-tool-use".to_owned(),
        };
        let registered = || vec![(root.path().join("hooks.json"), vec![wire()])];

        let live = gates(&[("codex", registered(), true)], None, &[]);
        assert!(matches!(&live[0].health, Health::Fine { detail } if detail.contains("1 live")));

        let untrusted = Skipped {
            detail: "Codex skips the gate: config.toml holds no trust for it".to_owned(),
            cleared_by_setup: true,
            advice: "remove the `enabled = false` line".to_owned(),
        };
        let skipped = gates(
            &[("codex", registered(), true)],
            None,
            &[("codex", untrusted)],
        );
        let Health::Broken { detail, resolution } = &skipped[0].health else {
            unreachable!(
                "a gate Codex skips was not reported broken: {:?}",
                skipped[0].health
            )
        };
        assert!(
            detail.contains("codex") && detail.contains("skips"),
            "{detail}"
        );
        assert!(!detail.contains("live"), "{detail}");
        assert_eq!(*resolution, Resolution::run("estigia setup codex"));

        let disabled = Skipped {
            detail: "Codex skips the gate: `enabled = false`".to_owned(),
            cleared_by_setup: false,
            advice: "remove the `enabled = false` line".to_owned(),
        };
        let skipped = gates(
            &[("codex", registered(), true)],
            None,
            &[("codex", disabled)],
        );
        let Health::Broken { resolution, .. } = &skipped[0].health else {
            unreachable!("a disabled gate was not reported broken")
        };
        assert!(
            matches!(resolution, Resolution::NoCommand { .. }),
            "a command was named that does not clear `enabled = false`: {resolution}"
        );

        let elsewhere = gates(
            &[("claude-code", registered(), true)],
            None,
            &[(
                "codex",
                Skipped {
                    detail: String::new(),
                    cleared_by_setup: true,
                    advice: "remove the `enabled = false` line".to_owned(),
                },
            )],
        );
        assert!(
            !elsewhere[0].health.is_broken(),
            "another agent's trust was read"
        );
    }

    #[test]
    fn full_reports_a_codex_gate_live_only_while_codex_trusts_it() {
        let home = tempfile::tempdir().expect("a temporary home");
        let repo = tempfile::tempdir().expect("a temporary repository");
        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            ..crate::setup::SetupOptions::default()
        };
        let adapter = crate::setup::find_agent("codex").expect("a declared agent");
        crate::setup::setup(adapter, &crate::config::Config::default(), &options)
            .expect("setup runs");
        let paths = crate::setup::resolve_paths(adapter, &options).expect("paths resolve");
        let skill = Some(paths.skill_root.as_path());
        let tracker = crate::config::Tracker::Github { repo: None };
        let row = || {
            full(skill, repo.path(), &tracker, &options)
                .into_iter()
                .find(|check| {
                    check.name == "gate" && format!("{:?}", check.health).contains("codex")
                })
                .map(|check| check.health)
                .expect("the codex gate row is on the report")
        };

        assert!(
            matches!(row(), Health::Fine { ref detail } if detail.contains("1 live")),
            "{:?}",
            row()
        );

        let config = paths.mcp_config.expect("codex reads a config");
        let held = std::fs::read_to_string(&config).expect("read");
        std::fs::write(
            &config,
            held.replace("trusted_hash = \"sha256:", "trusted_hash = \"sha256:0"),
        )
        .expect("drift the hash");
        let Health::Broken { detail, resolution } = row() else {
            unreachable!("a gate Codex skips was reported live")
        };
        assert!(detail.contains("another hash"), "{detail}");
        assert_eq!(resolution, Resolution::run("estigia setup codex"));

        crate::setup::setup(adapter, &crate::config::Config::default(), &options)
            .expect("setup runs again");
        assert!(matches!(row(), Health::Fine { .. }), "{:?}", row());
    }

    #[test]
    fn an_agent_with_no_gate_registered_is_skipped_rather_than_broken() {
        // Some adapters have no gate, by design.
        // Reporting a deliberate choice as damage teaches people to ignore the
        // report — which is the one thing a health report cannot survive.
        let checks = gates(&[("codex", Vec::new(), false)], None, &[]);
        assert_eq!(checks.len(), 1);
        assert!(matches!(checks[0].health, Health::Skipped { .. }));
        assert!(!checks[0].health.is_broken());
    }

    #[test]
    fn an_entry_this_build_cannot_read_is_not_an_agent_gated_somewhere_else() {
        // Settings-gated, registered, and nothing parsed out of it.
        let unreadable = gates(&[("claude-code", Vec::new(), true)], None, &[]);
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
        let plugin = gates(&[("cline", Vec::new(), true)], None, &[]);
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
        let none = gates(&[("codex", Vec::new(), false)], None, &[]);
        assert!(
            !none[0].health.is_broken(),
            "an agent that was never gated was reported as damage: {:?}",
            none[0].health
        );
    }

    #[test]
    fn a_missing_skill_is_the_only_thing_reported_because_nothing_after_it_means_anything() {
        let checks = examine(None, Path::new("."));
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "skill");
        assert!(checks[0].health.is_broken());
    }

    #[test]
    fn every_broken_check_names_a_way_out() {
        // The ratchet applied to an environment. A report that says what is
        // wrong and not what to do is the dead end it exists to prevent.
        let root = tempfile::tempdir().expect("a temporary root");
        for check in examine(Some(root.path()), root.path()) {
            if check.health.is_broken() {
                assert!(
                    check.health.resolution().is_some(),
                    "{} is broken and says nothing about what to do",
                    check.name
                );
            }
        }
    }

    #[test]
    fn a_directory_that_is_not_a_repository_is_skipped_rather_than_failed() {
        // Running `doctor` from a home directory must not report that git is
        // broken. A report people learn to ignore is a report that has stopped
        // working.
        let root = tempfile::tempdir().expect("a temporary root");
        let checks = examine(Some(root.path()), root.path());
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
        for check in examine(Some(root.path()), root.path()) {
            assert!(
                check.about.len() > 20,
                "{} does not say what it is for",
                check.name
            );
            assert!(!check.name.is_empty());
        }
    }

    #[test]
    fn nothing_on_the_list_asks_for_an_interpreter() {
        let root = tempfile::tempdir().expect("a temporary root");
        let checks = examine(Some(root.path()), root.path());
        assert!(
            !checks.iter().any(|check| check.name == "python"),
            "the health report asks for an interpreter this release does not use"
        );
        // The floor: the walk produced the rows it should, so a list that came
        // back empty — or came back without the row this one replaced — cannot
        // pass for a clean one.
        for wanted in ["skill", "gh"] {
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
                shadowed_local: None,
                duplicated: Vec::new(),
                unknown: Vec::new(),
                inherited: Vec::new(),
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
            entry("claude-code", absent.clone(), false),
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
        let shared = contracts(&[entry("claude-code", present.clone(), false)]);
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
            unreadable: Some("`Integration` is \"octopus\" in estigia.local.md".to_owned()),
            ignored: Vec::new(),
            cut_short: Vec::new(),
            shadowed_local: None,
            duplicated: Vec::new(),
            unknown: Vec::new(),
            inherited: Vec::new(),
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
            format!("{resolution:?}").contains("claude-code"),
            "the way out does not name the agent to fix"
        );
    }

    #[test]
    fn an_ungated_line_is_attributed_to_its_agent_and_never_to_its_dialect() {
        let source_of = |line: &str| {
            let root = tempfile::tempdir().expect("a temporary root");
            let state = root.path().join("state");
            std::fs::create_dir_all(&state).expect("the state directory");
            let entry: serde_json::Value = serde_json::from_str(line).expect("a record");
            crate::harness::session::record(&state, &entry);
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
                        crate::config::Setting::Integration,
                        "trunk".to_owned(),
                        "branch".to_owned(),
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
                for expected in ["claude-code", "Integration", "trunk", "branch"] {
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
        // a shared root at all. Both halves are fixed, so this
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
        // opencode "Delivery authorisation" ask` is a documented command, and
        // calling the machine broken for having run it names a fault with no way
        // out — measured by both halves of the first review of this row, one on
        // `Model routing` and one on `Delivery authorisation`. Named, never
        // broken.
        let per_agent = canonical(
            Some(&root),
            Some(&[Divergence {
                agent: "opencode",
                rows: vec![
                    (
                        crate::config::Setting::Delivery,
                        "ask".to_owned(),
                        "auto".to_owned(),
                    ),
                    (
                        crate::config::Setting::Models,
                        "developer=opus".to_owned(),
                        "unset".to_owned(),
                    ),
                ],
            }]),
        );
        match per_agent.health {
            super::Health::Fine { detail } => {
                for expected in [
                    "opencode",
                    "Delivery authorisation",
                    "Model routing",
                    "by design",
                ] {
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
                        crate::config::Setting::Delivery,
                        "ask".to_owned(),
                        "auto".to_owned(),
                    ),
                ],
            }]),
        );
        match both.health {
            super::Health::Broken { detail, .. } => {
                assert!(detail.contains("Change size"), "{detail}");
                assert!(
                    detail.contains("Delivery authorisation") && detail.contains("by design"),
                    "the deliberate per-agent row vanished as soon as a fault appeared: {detail}"
                );
            }
            other => panic!("a repository row at odds was reported as {other:?}"),
        }
    }

    /// `tracker_in_force` reads the checkout's own layer, not the contract alone.
    ///
    /// The test that held this was deleted with the tracker whose *kind* it
    /// switched, and the kind is what became unposable — not the value. With one
    /// variant left, the observable difference is the repository the tracker
    /// names, and it is load-bearing: `stale_run_pointers` branches on
    /// `named_repo()`, so a doctor reading the contract alone asks the wrong
    /// repository whether a pointed-at issue is still open.
    ///
    /// The contract is given a named repository of its own, `contract/one`, so
    /// the floor below is not `None` — a `tracker_in_force` short-circuited to
    /// `Config::default().tracker` would answer `None` for **both**
    /// assertions, passing the second and reading the first as vacuous. With a
    /// contract that actually names something, the floor requires the contract
    /// to be read at all, and the second assertion still requires the
    /// checkout's own layer to override it.
    #[test]
    fn the_tracker_in_force_is_the_one_this_checkout_names_not_the_contracts() {
        let home = tempfile::tempdir().expect("a temporary home");
        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            platform: Some(crate::setup::Platform::Unix),
            skip_harness: true,
            ..crate::setup::SetupOptions::default()
        };
        let adapter = crate::setup::find_agent("agents").expect("a declared agent");
        crate::setup::setup(
            adapter,
            &crate::config::Config {
                tracker: crate::config::Tracker::Github {
                    repo: Some(
                        crate::config::RepoRef::parse("contract/one").expect("a valid repo ref"),
                    ),
                },
                ..crate::config::Config::default()
            },
            &options,
        )
        .expect("the install writes");
        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("paths")
            .skill_root;

        let repo = tempfile::tempdir().expect("a checkout");
        std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");

        // The floor: with no layer of its own, the checkout takes the
        // contract's answer — `contract/one`, and not `None`, so a function
        // that never read the contract at all fails here rather than passing
        // vacuously.
        assert_eq!(
            super::tracker_in_force(Some(&root), repo.path()).named_repo(),
            Some("contract/one".to_owned()),
            "the contract's own repository was not read"
        );

        let path = crate::skill::repository_config_path(repo.path());
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
        std::fs::write(
            &path,
            "| Tracker | github acme/repo |
",
        )
        .expect("the layer is writable");

        assert_eq!(
            super::tracker_in_force(Some(&root), repo.path()).named_repo(),
            Some("acme/repo".to_owned()),
            "the checkout's own row was not read: {}",
            path.display()
        );
    }

    /// The wiring, not only the function: `tracker_in_force`'s answer has to
    /// actually reach the check that reads it, the way `cli::doctor` feeds one
    /// into the other at its own call site rather than each reading the
    /// contract on its own.
    ///
    /// The test above asserts on `tracker_in_force`'s return value directly —
    /// a correct answer nobody downstream consumes would still pass it. This
    /// composes it with `stale_run_pointers` the same way `cli::doctor` does,
    /// and reads back which repository `gh` was actually asked about through
    /// `GH_REPO`, the variable `tracker_answer_for_pointer` sets from
    /// `named_repo()` before every read. Confirmed red against the
    /// contract-only mutation named above: `tracker_in_force` short-circuited
    /// to `Config::default().tracker` sends this to `gh` asking about neither
    /// repository the fixture names, and the assertion for `acme/repo` fails.
    #[test]
    fn the_stale_run_pointer_check_asks_gh_about_the_checkout_s_own_repository() {
        let repo = git_repo();

        let home = tempfile::tempdir().expect("a temporary home");
        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            config_home: Some(home.path().join(".config")),
            app_data: Some(home.path().join("AppData").join("Roaming")),
            platform: Some(crate::setup::Platform::Unix),
            skip_harness: true,
            ..crate::setup::SetupOptions::default()
        };
        let adapter = crate::setup::find_agent("agents").expect("a declared agent");
        crate::setup::setup(
            adapter,
            &crate::config::Config {
                tracker: crate::config::Tracker::Github {
                    repo: Some(
                        crate::config::RepoRef::parse("contract/one").expect("a valid repo ref"),
                    ),
                },
                ..crate::config::Config::default()
            },
            &options,
        )
        .expect("the install writes");
        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("paths")
            .skill_root;

        // The checkout's own layer names a different repository — the one
        // this call has to ask `gh` about.
        let path = crate::skill::repository_config_path(repo.path());
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
        std::fs::write(&path, "| Tracker | github acme/repo |\n").expect("the layer is writable");

        // Exactly the seam `cli::doctor` wires at its own call site: the
        // tracker `tracker_in_force` derives, fed straight into the check
        // that reads it.
        let tracker = super::tracker_in_force(Some(&root), repo.path());
        let run = holder("claude-wiring0000", 91, repo.path());

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::closed_issue(91)]);
        let log = home.path().join("gh-env.log");
        let logged = crate::test_env::with_env_logging_gh(bin.path(), &script, &log, || {
            let _ = super::stale_run_pointers(Some(&root), repo.path(), &tracker, &[run]);
            std::fs::read_to_string(&log).unwrap_or_default()
        });

        assert!(
            logged.contains("GH_REPO=acme/repo"),
            "the checkout's own repository was not the one gh was asked about: {logged}"
        );
        assert!(
            !logged.contains("GH_REPO=contract/one"),
            "the contract's repository reached gh instead of the checkout's: {logged}"
        );
    }

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
            "| Setting | Value here |\n|---|---|\n| Model routing | developer=opus |\n",
        )
        .expect("the operator's own file");
        match named(&report(&options)) {
            super::Health::Fine { detail } => assert!(
                detail.contains("Model routing") && detail.contains("by design"),
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

    const STALLED_FIXTURE: &str = "harness::doctor::tests::a_stalled_program_fixture";

    #[test]
    #[ignore = "the child a_program_that_stalls_is_broken_within_its_limit launches"]
    fn a_stalled_program_fixture() {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    #[test]
    fn a_program_that_stalls_is_broken_within_its_limit() {
        let executable = std::env::current_exe().expect("the test executable");
        let resolution = Resolution::run("estigia setup --all");
        let started = std::time::Instant::now();
        let health = super::program_within(
            &executable.display().to_string(),
            &["--ignored", "--exact", STALLED_FIXTURE, "--nocapture"],
            |output| output.to_owned(),
            resolution.clone(),
            std::time::Duration::from_millis(300),
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the check waited {:?} on a stalled program",
            started.elapsed()
        );
        match health {
            super::Health::Broken {
                detail,
                resolution: carried,
            } => {
                assert_eq!(carried, resolution);
                assert!(detail.contains("did not finish within"), "{detail}");
            }
            other => panic!("a stalled program was reported as {other:?}"),
        }
    }
}
