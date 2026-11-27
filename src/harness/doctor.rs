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
const LOOKS_BACK: &[&str] = &[
    "silence",
    "stale-run-pointer",
    "prune",
    "gate-version",
    "store-records",
];

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
    let guard_state = super::guard::state(repo_dir);
    let guard_path = super::guard::hooks_directory(repo_dir)
        .ok()
        .map(|hooks| hooks.join(super::guard::HOOK));
    let guard_outside_the_surface = guard_path
        .as_ref()
        .is_some_and(|path| !super::is_control_surface(&path.to_string_lossy()));
    let push_guard_health = if guard_outside_the_surface
        && guard_state != super::guard::State::Absent
    {
        Health::broken(
            format!(
                "the push guard sits at {}, which the gate does not read as control surface \
                     \u{2014} an agent's write to that path is admitted, so the guard can be \
                     disarmed by the thing it guards against",
                guard_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default()
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "this checkout's hooks where the gate watches them: `git config \
                     core.hooksPath` decides the directory, and the ordinary `hooks` one is a \
                     control surface while a custom directory is not",
            ),
        )
    } else {
        match guard_state {
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
            // Marker present, body gone or replaced. Nothing this build wrote
            // answers, so the file is installed-looking and disarmed; the
            // reinstall names the hook's own path and replaces a body carrying
            // only Estigia's lines.
            super::guard::State::Disarmed => Health::broken(
                "the pre-push hook carries Estigia's marker and not the body this binary \
                 installs, so the gate in it does not answer and every push goes out unchecked",
                Resolution::run("estigia guard"),
            ),
            // Marker, a different body, and somebody's own lines: `estigia
            // guard` refuses to rewrite those, so naming it would be a dead
            // end.
            super::guard::State::Altered => Health::broken(
                "the pre-push hook carries Estigia's marker, a body this binary did not write, \
                 and lines Estigia did not write either \u{2014} what runs on a push cannot be \
                 shown to be this build's gate",
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "a decision about the lines Estigia did not write: move them into a hook of \
                     your own and chain `estigia hook pre-push` from it, or take them out and \
                     run `estigia guard` \u{2014} rewriting the file here would take them away",
                ),
            ),
            }
    };
    checks.push(Check {
        name: "push-guard",
        about: "the pre-push hook that refuses a push no live claim authorises",
        health: push_guard_health,
    });

    checks.push(Check {
        name: "remote",
        about: "the git remote the tracker repository is discovered from",
        health: remote(repo_dir),
    });

    checks.push(Check {
        name: "repository-config",
        about: "this checkout's own configuration rows",
        health: repository_config(skill_root, repo_dir),
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

/// Whether this checkout's own file reads as configuration, and names rows
/// this build knows.
///
/// Absent is not broken: a repository that has never answered for itself is the
/// ordinary case. A row that names no setting and a row that does not parse are
/// both reported here, because both are dropped by every reader — the first by
/// `filter_map`, the second by a tolerant read — and a dropped row is a rule
/// nobody is following.
fn repository_config(skill_root: &Path, repo_dir: &Path) -> Health {
    let rows = match crate::skill::repository_rows(repo_dir) {
        Ok(rows) => rows,
        Err(refusal) => return Health::broken(refusal.message, refusal.resolution),
    };
    let Some((path, text)) = rows else {
        return Health::Skipped {
            detail: format!("{} has no rows of its own", repo_dir.display()),
        };
    };
    let unknown: Vec<String> = crate::config::table_rows(&text)
        .into_iter()
        .filter(|(label, value)| {
            crate::config::Setting::from_label(label).is_none()
                && crate::config::retired_setting_report(label, value).is_none()
        })
        .map(|(label, _)| label)
        .collect();
    if !unknown.is_empty() {
        return Health::broken(
            format!(
                "{} names {} row(s) that resolve to no setting: {}",
                path.display(),
                unknown.len(),
                unknown.join(", ")
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the row's label corrected or removed; `estigia config set --repo` writes the \
                 spelling this build reads",
            ),
        );
    }
    let base = match crate::skill::installed_config(skill_root) {
        Ok(base) => base,
        // Without the machine's answers there is nothing to judge this
        // checkout's rows against, and defaulting to `Config::default` would
        // call a row that tightens the machine a loosening.
        Err(refusal) => {
            return Health::Skipped {
                detail: format!(
                    "the machine's own rows could not be read: {}",
                    refusal.message
                ),
            };
        }
    };
    match crate::skill::repository_complaint(&base, repo_dir) {
        Ok(None) => Health::Fine {
            detail: path.display().to_string(),
        },
        Ok(Some(refusal)) | Err(refusal) => Health::broken(refusal.message, refusal.resolution),
    }
}

/// Every verdict that means a call reached the gate and was not decided on.
/// One place, because `doctor` reads it and the daily cap must never evict one.
pub(crate) const UNDECIDED: &[&str] = &[
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
                let gate_event = crate::harness::hook::Event::PreToolUse.slug();
                let wants_a_blocking_failure = crate::setup::find_agent(name)
                    .is_ok_and(crate::setup::AgentAdapter::blocks_when_the_gate_fails);
                if wants_a_blocking_failure
                    && let Some(wire) = wires
                        .iter()
                        .find(|wire| wire.event == Some(gate_event) && wire.blocks_on_failure == Some(false))
                {
                    let file = files
                        .iter()
                        .find(|(_, found)| found.iter().any(|candidate| candidate == *wire))
                        .map(|(file, _)| file.display().to_string())
                        .unwrap_or_default();
                    return Check {
                        name: "gate",
                        about: "whether the gate this agent registers would actually run",
                        health: Health::broken(
                            format!(
                                "{name}: the gate is registered and a hook that fails or reaches \
                                 its timeout does not block \u{2014} it carries no `\"onFailure\": \
                                 \"block\"`, so a slow verification is released and the tool call \
                                 goes through ungated ({file})"
                            ),
                            Resolution::run(format!("estigia setup {name}")),
                        ),
                    };
                }
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
    "repository-config",
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
    "store-records",
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

/// The first pair of delivery rows in force here that cannot both hold.
pub fn delivery_conflict_in_force(
    skill_root: Option<&Path>,
    repo_dir: &Path,
) -> Option<crate::outcome::Refusal> {
    skill_root
        .map(|root| crate::skill::installed_config_in_keeping_what_parses(root, repo_dir).0)
        .unwrap_or_default()
        .delivery_conflict()
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
    checks.push(store_records(home));
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
        Ok((version, store.calls_since(since)?))
    });
    let health = match opened {
        Ok((version, calls)) => {
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
            Health::Fine { detail }
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

/// The records the store could not write down and counted instead.
///
/// A count describes appends that already failed, not whether a run can claim
/// now, so it is reported and never blocks. A snapshot that has succeeded since
/// has already deleted its row; one whose last failure is older than the record
/// retention is not a current fault either, and the daily pass deletes it. A row
/// with no recorded time stays broken: its age is unknown, and an unknown result
/// is not clearance.
pub fn store_records(home: Option<&Path>) -> Check {
    use crate::transport::store::Store;
    let about =
        "whether any record the local workflow store could not write down was counted instead";
    let skipped = |detail: String| Check {
        name: "store-records",
        about,
        health: Health::Skipped { detail },
    };
    let path = match crate::transport::store::default_path(home) {
        Ok(path) if path.exists() => path,
        _ => return skipped("there is no store to read counted failures from".to_owned()),
    };
    let health = match Store::open(&path).and_then(|store| store.record_failures()) {
        Err(failure) => Health::broken(
            format!(
                "the counted failures could not be read: {}",
                failure.detail()
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "inspect the store; the `store` row says whether it opens",
            ),
        ),
        Ok(failures) => {
            let now = crate::harness::session::now_seconds().unwrap_or_default();
            counted_failures(&failures, now)
        }
    };
    Check {
        name: "store-records",
        about,
        health,
    }
}

/// What the counted failures mean, fed the current time so the age bound is
/// testable without waiting a month.
fn counted_failures(failures: &[crate::transport::store::RecordFailure], now: u64) -> Health {
    if failures.is_empty() {
        return Health::Fine {
            detail: "no record was ever counted instead of written".to_owned(),
        };
    }
    let retention = crate::harness::session::RECORD_RETENTION_DAYS * 24 * 60 * 60;
    let cutoff = now.saturating_sub(retention);
    let current: Vec<&crate::transport::store::RecordFailure> = failures
        .iter()
        .filter(|failure| failure.last_at.is_none_or(|at| at >= cutoff))
        .collect();
    if current.is_empty() {
        return Health::Fine {
            detail: format!(
                "every counted failure is older than the {}-day record retention, and the \
                 records around it are gone as well",
                crate::harness::session::RECORD_RETENTION_DAYS
            ),
        };
    }
    let stamp = |failure: &crate::transport::store::RecordFailure| {
        failure.last_at.map_or_else(
            || "when the last one happened is unknown".to_owned(),
            |at| {
                format!(
                    "the most recent at {}",
                    crate::harness::session::stamp_of(at)
                )
            },
        )
    };
    let sentences: Vec<String> = current
        .iter()
        .map(|failure| match failure.kind.as_str() {
            "snapshot" => {
                let mut sentence = format!("{} snapshot(s) could not be written", failure.count);
                match failure.last_at {
                    Some(at) => sentence.push_str(&format!(
                        ", and no snapshot has succeeded since: the last failure at {}",
                        crate::harness::session::stamp_of(at)
                    )),
                    None => sentence.push_str(", and the store did not record when"),
                }
                if let Some(why) = failure.last_error.as_deref().filter(|why| !why.is_empty()) {
                    sentence.push_str(&format!(": {why}"));
                }
                sentence
            }
            kind => {
                let noun = match kind {
                    "decisions" => "decision record(s)",
                    "roles" => "role record(s)",
                    other => {
                        return format!(
                            "{} {other} record(s) could not be written down and were counted \
                         instead, {}",
                            failure.count,
                            stamp(failure)
                        );
                    }
                };
                format!(
                    "{} {noun} could not be written down and were counted instead, {}",
                    failure.count,
                    stamp(failure)
                )
            }
        })
        .collect();
    let only_snapshots = current.iter().all(|failure| failure.kind == "snapshot");
    let resolution = if only_snapshots {
        "why the snapshot could not be written \u{2014} until one succeeds the newest copy is \
         older than that failure; a later successful snapshot clears this row"
    } else {
        "why the append failed \u{2014} a record that could not be written down is not on the \
         record, and this is where it is counted"
    };
    Health::broken(
        sentences.join("; "),
        Resolution::no_command(NoCommandReason::OperatorKnowledge, resolution),
    )
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
        ownership: super::session::state_root(None)
            .map_err(|failure| failure.message)
            .and_then(|root| super::prune::ownership(&root)),
        ownership_root: super::session::state_root(None).ok(),
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
                            "no eligible branch or checkout is unowned; {} listed and kept \
                             (held, unpublished work, or no readable cleanup eligibility)",
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

/// The Pi subagent runners other than Estigia's own that the settings and extension directories under
/// `roots` name, each with its source path when it is on disk.
pub fn pi_other_runners(roots: &[std::path::PathBuf]) -> Vec<(String, Option<std::path::PathBuf>)> {
    let mut found: Vec<(String, Option<std::path::PathBuf>)> = Vec::new();
    for root in roots {
        if let Some(entries) = pi_json(&root.join("settings.json"))
            .and_then(|value| value.get("packages").cloned())
            .and_then(|value| value.as_array().cloned())
        {
            found.extend(entries.iter().filter_map(pi_runner_name));
        }
    }
    for directory in roots.iter().map(|root| root.join("extensions")) {
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
    found
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
    let found = pi_other_runners(&[agent_dir.to_path_buf(), project.clone()]);
    let own: Vec<std::path::PathBuf> = [agent_dir.join("extensions"), project.join("extensions")]
        .into_iter()
        .map(|directory| directory.join(crate::setup::plugin::PI_AGENTS_FILE))
        .filter(|file| {
            std::fs::read_to_string(file)
                .is_ok_and(|text| crate::setup::plugin::agents_is_ours(Some(&text)))
        })
        .collect();
    let own_reading = own
        .first()
        .map(|file| format!("Estigia's own runner {} is installed", file.display()));
    if found.is_empty() {
        return Check {
            name: "pi-subagents",
            about,
            health: Health::Fine {
                detail: own_reading.map_or_else(
                    || {
                        "no Pi subagent runner is installed in the settings or extension \
                         directories it reads"
                            .to_owned()
                    },
                    |own| format!("{own}, and no other Pi subagent runner is"),
                ),
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
    if let Some(own) = own_reading {
        detail.push_str(&format!(
            "; {own} beside {} other runner(s), so a delegation can reach either: name \
             `{}` when it is Estigia's to run",
            found.len(),
            crate::setup::plugin::PI_AGENTS_TOOL
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
        crate::setup::plugin::PI_AGENTS_FILE,
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
    let indent = " ".repeat(22);
    let when = if last == 0 {
        "at a time the store did not record".to_owned()
    } else {
        format!("at {}", crate::harness::session::stamp_of(last))
    };
    let ages_out = if last == 0 {
        format!(
            "\n{indent}the newest undecided call carries no date, so when it ages out is unknown"
        )
    } else {
        format!(
            "\n{indent}the newest ages out of this report on {}",
            crate::harness::session::stamp_of(
                last.saturating_add(crate::harness::session::RECORD_RETENTION_DAYS * 24 * 60 * 60)
            )
        )
    };
    Check {
        name: "silence",
        about: "whether any call reached the gate and went undecided",
        health: Health::broken(
            format!(
                "{} call(s) went through ungated, the most recent {} {when}: \
                 {what}{unminted}{latest}{ages_out}",
                ungated.len(),
                found.source.as_deref().map_or_else(
                    || "from an agent no line names".to_owned(),
                    |who| format!("from {who}"),
                ),
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
mod tests;
