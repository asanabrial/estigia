//! The harness: Estigia holding the tools rather than asking politely.
//!
//! Everything under `skill/` is text an agent may read and may ignore. That is
//! the gap the honesty contract already named — *the seam tests prove the
//! contract does not link to a file that is missing; they do not prove an agent
//! obeys it.* This module closes it for the one rule worth closing:
//!
//! > The gate performs the pre-write verification: a repository write it
//! > observes is re-verified against the claim whenever the renewal window has
//! > passed, and at every expensive or irreversible boundary. An unreadable
//! > control surface permits no write.
//!
//! (`skill/SKILL.md`, Hard Rules.)
//!
//! With the harness installed, that stops being a sentence in a contract and
//! becomes a `PreToolUse` hook that returns `deny`.
//!
//! # What the oath binds
//!
//! **A run that has sworn nothing is not under Estigia's authority.** Gating
//! every write in every session would mean a person asking an unrelated question
//! cannot edit a file, which is not workflow authority, it is a lock. The oath
//! binds once sworn: the moment a run claims an issue, every repository write it
//! makes is measured against that claim until it releases.
//!
//! That is the honest reading of the metaphor and it is also where the value is.
//! Incident I07 is not "an agent wrote without claiming" — it is a run that
//! **lost a claim race by five seconds, was told so 33 seconds later, and worked
//! another 48 minutes** because nothing in its loop read the timeline again.
//! That run had sworn. The gate kills exactly that case.

use std::cmp::Reverse;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::outcome::{
    CommandObserved, Established, NoCommandReason, Observed, Refusal, Resolution,
};
use population::{Disposition, Population};

pub mod assisted;
pub mod doctor;
pub mod guard;
pub mod hook;
pub mod issue_creation;
pub mod issue_requirement;
pub mod loop_text;
pub mod mcp;
pub mod operation_window;
pub mod person_only;
pub mod population;
pub mod prune;
pub mod refusal_class;
pub mod roles;
pub mod session;
pub mod shell;
pub mod standdown;
pub mod tracker;
pub mod unclaimed;

pub use session::Run;

/// How long a routine write may ride on the previous answer.
pub const RENEWAL_WINDOW: Duration = Duration::from_secs(120);

/// Why a merge command's pull request stayed unidentified although the line
/// may well carry a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrUnidentified {
    /// The byte filter disqualified the line.
    Byte(String),
    /// The line passed the filter and carries an option naming a repository.
    RepositoryOption,
}

/// What the agent is about to do, as far as the gate is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// A write to the working tree.
    Write {
        /// What is being written, for the message.
        target: String,
    },
    /// A step that cannot be taken back: a push, a merge, a tag, a release.
    Boundary {
        /// The command, for the message.
        command: String,
        /// The exact pull request named by a narrowly recognised merge command.
        pr: Option<u64>,
        /// Why a merge command's PR stayed unidentified, when the line named
        /// a number or could have.
        pr_unidentified_reason: Option<PrUnidentified>,
        /// The target of an exact local `git merge --ff-only`, when the shell
        /// payload was narrow enough to prove rather than interpret.
        local_fast_forward_target: Option<String>,
        /// Whether the literal shell payload is nothing but `git merge`, with at
        /// most a leading `cd <dir> &&` or `git -C <dir>`; `false` when the
        /// payload was not narrow enough to prove, which refuses rather than
        /// exempts.
        is_exact_integration_merge: bool,
        /// The directory a git delivery runs in, as the line itself spells it.
        runs_in: RunsIn,
        /// The commit a `git tag` or an exact `git merge --ff-only` acts on.
        target: CommitTarget,
    },
    /// A write to GitHub that is no delivery boundary. The gate leaves it alone
    /// while the git operation window is open and denies it while it is closed.
    Remote {
        /// The call, for the message.
        command: String,
    },
    /// Nothing Estigia claims authority over.
    Untouched,
}

/// The commit a git delivery acts on, as far as the line spells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitTarget {
    /// The line names no commit: the checkout's own `HEAD`, or a command that
    /// acts on no commit at all.
    Checkout,
    /// A ref or SHA the line names as a plain word.
    Named(String),
    /// The line names a commit in a way that is not read without running it.
    Unread,
}

/// Where a git delivery line says it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunsIn {
    /// The line names no directory: the session's own.
    Session,
    /// A plain path from a leading `cd <dir> &&` or `git -C <dir>`.
    Named(String),
    /// The line changes directory in a way that cannot be read without running
    /// it, so no checkout can be named for it.
    Unestablished,
}

impl Action {
    /// What this action is *about*: the target, or the command, prefixed onto a
    /// refusal. A target is a path only for a write tool; a shell write names
    /// the matched fragment, the verb or the tool.
    pub fn subject(&self) -> Option<String> {
        match self {
            Self::Write { target } => Some(target.clone()),
            Self::Boundary {
                command,
                pr: Some(pr),
                ..
            } if command == "gh pr merge" => Some(format!("{command} {pr}")),
            Self::Boundary { command, .. } => Some(command.clone()),
            Self::Remote { command } => Some(command.clone()),
            Self::Untouched => None,
        }
    }

    /// The pull request lineage explicitly proven by this action.
    pub fn pr(&self) -> Option<u64> {
        match self {
            Self::Boundary { pr, .. } => *pr,
            _ => None,
        }
    }
}

/// How hard the gate looks before letting this through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    /// May ride on an answer from inside [`RENEWAL_WINDOW`].
    Routine,
    /// Always re-reads. No window, no exceptions.
    Boundary,
}

/// Why the gate had no call to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aside {
    /// The action is not one the harness claims authority over.
    NotWatched,
    /// No oath of this run's covers it: nothing was sworn, or nothing here was.
    NothingSworn,
    /// The configured tracker ships no executable, so nothing can be asked.
    NoTracker,
    /// Sworn, but over a different checkout than this write is in.
    AnotherCheckout,
    /// Sworn, and this write lands where no covered checkout reaches.
    ///
    /// Distinct from [`Self::AnotherCheckout`], which is about *this* checkout
    /// being somebody else's. This one is about the **path**: a scratch note or
    /// an agent's own store is in no checkout at all, and answering
    /// `another-checkout` for it would name a thing that does not exist.
    OutsideTheClaim,
}

impl Aside {
    /// Every reason, for the crossing that keeps their names honest.
    pub const ALL: &'static [Self] = &[
        Self::NotWatched,
        Self::NothingSworn,
        Self::NoTracker,
        Self::AnotherCheckout,
        Self::OutsideTheClaim,
    ];

    /// The stable name a program matches on, spelled the way
    /// [`crate::outcome::Refusal::code`] is spelled: a code is matched on, and
    /// never reworded to improve the prose.
    pub fn code(self) -> &'static str {
        match self {
            Self::NotWatched => "not-watched",
            Self::NothingSworn => "nothing-sworn",
            Self::NoTracker => "no-tracker",
            Self::AnotherCheckout => "another-checkout",
            Self::OutsideTheClaim => "outside-the-claim",
        }
    }

    /// What to tell somebody who asked, given the tool they asked about.
    pub fn why(self, tool: &str) -> String {
        match self {
            Self::NotWatched => format!("{tool} is not something this run's oath covers"),
            Self::NothingSworn => format!(
                "{tool} is watched, and this run holds no issue \u{2014} the oath binds once sworn"
            ),
            Self::NoTracker => format!(
                "{tool} is watched, and the configured tracker ships no executable, so there is \
                 nothing to ask"
            ),
            Self::AnotherCheckout => format!(
                "{tool} is watched, and this run's claim covers a different checkout than this one"
            ),
            Self::OutsideTheClaim => format!(
                "{tool} is watched, and this write lands outside every checkout this run's claim \
                 covers \u{2014} a claim governs a repository, not the machine"
            ),
        }
    }
}

/// What the gate decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Go ahead, and why — shown to nobody unless something asks.
    Allow(String),
    /// Stop, with everything needed to act on it.
    Deny(Box<Refusal>),
    /// Put to the person, where the host can ask; a host that cannot answers it as
    /// [`Self::Deny`] carrying the same refusal, and never as [`Self::Allow`].
    Ask(Box<Refusal>),
    /// Not Estigia's call, and which of the five reasons it was.
    Outside(Aside),
}

impl Decision {
    /// Whether this decision stops the tool call.
    pub fn denies(&self) -> bool {
        matches!(self, Self::Deny(_))
    }

    /// This decision as a host that cannot ask has to answer it.
    pub fn without_ask(self) -> Self {
        match self {
            Self::Ask(refusal) => Self::Deny(refusal),
            other => other,
        }
    }
}

/// The population and boundary `WRITE_TOOLS` enforces.
pub const WRITE_TOOLS_POPULATION: Population = Population {
    family: "write-tools",
    disposition: Disposition::TooTight,
    subject: "WRITE_TOOLS",
    text: "the tools an agent uses to edit files in the repository under review. \
           Legitimate population: every built-in editing tool of every agent Estigia gates. \
           Boundary: a tool not on this list writes without being gated, and each agent names \
           its own.",
};

/// Tool names whose whole purpose is to write to the working tree.
pub(crate) const WRITE_TOOLS: &[&str] = &[
    "edit",
    "write",
    "multiedit",
    "notebookedit",
    "update",
    "apply_patch",
    "write_file",
    "replace",
    "patch",
    "pre_write_code",
];

/// The population and boundary `SHELL_TOOLS` enforces.
pub const SHELL_TOOLS_POPULATION: Population = Population {
    family: "shell-tools",
    disposition: Disposition::TooTight,
    subject: "SHELL_TOOLS",
    text: "the tools through which an agent runs a command line. Legitimate population: every \
           built-in shell tool of every agent Estigia gates. Boundary: a shell tool not named \
           here has its command line unread, so an irreversible step inside it is invisible.",
};

/// Tool names that hand a command to a shell.
///
/// Separated from the writing tools because their *argument* decides what they
/// are: `git status` through one of these is a read, and `git push` through the
/// same tool is a boundary.
pub(crate) const SHELL_TOOLS: &[&str] = &["bash", "run_shell_command", "shell", "pre_run_command"];

/// The population and boundary `CONTROL_SURFACE` enforces.
pub const CONTROL_SURFACE_POPULATION: Population = Population {
    family: "control-surface",
    disposition: Disposition::TooTight,
    subject: "CONTROL_SURFACE",
    text: "the files Estigia's decisions are read from \u{2014} its state directory, the \
           contract, the entries it writes into an agent's own settings, and the push hook \u{2014} \
           **and the files that carry its authority to an agent**: the instruction file each \
           adapter's `setup` writes the workflow-authority directive into. Legitimate \
           population: any path a write to which changes what this harness enforces, or what \
           an agent is told this harness may enforce.",
};

/// Paths whose contents are this harness's own authority.
///
/// That second clause is a widening, recorded rather than slipped in. The instruction files were
/// added here, and a reviewer was right that they do not fit the
/// rule as it stood: the gate reads nothing from `~/.claude/CLAUDE.md`, so a write
/// there changes what the agent is *told* and not what the binary *does*. The
/// reason they belong anyway is that this crate's authority over an agent is
/// exactly that sentence — `README.md` says the harness does not ask an agent to
/// follow the workflow, it holds the tools, and the directive is what makes the
/// agent reach for those tools at all. An agent that rewrites it disarms the half
/// of the system that is not code. Enforcement inputs and compliance inputs are
/// different things and the population now names both. Boundary: matched
/// on path fragments — after separators are folded, and after `/./`, `//` and
/// **`../`** are collapsed away, because a matcher a redundant segment defeats
/// is measuring a spelling and not a path. So a hard link, a junction, or a
/// checkout of the state directory somewhere else is not matched; nor is a
/// fragment that merely *ends* the same way rejected — `..claude/settings.json`
/// matches, which is a false positive costing one tracker read, and that is the
/// direction this chooses on purpose.
const CONTROL_SURFACE: &[&str] = &[
    ".config/gh/hosts.yml",
    "cli/hosts.yml",
    ".estigia/",
    "skills/issue-flow/",
    "hooks/pre-push",
    ".claude/settings",
    ".claude/agents/",
    ".cursor/rules/",
    ".claude.json",
    ".codex/hooks.json",
    ".codex/config.toml",
    ".config/opencode/",
    "opencode/agents/",
    "opencode/plugins/",
    "opencode/opencode.json",
    ".opencode/agents/",
    ".pi/agent/extensions/",
    ".pi/agent/prompts/",
    ".pi/agent/mcp.json",
    ".pi/agent/settings.json",
    ".pi/extensions/",
    ".pi/prompts/",
    ".pi/settings.json",
    ".pi/mcp.json",
    ".gemini/settings.json",
    "gemini/settings.json",
    ".qwen/settings.json",
    ".cursor/hooks.json",
    ".cursor/mcp.json",
    ".continue/settings.json",
    "crush/crush.json",
    ".codeium/windsurf/hooks.json",
    ".cline/hooks",
    ".claude/plugins/",
    ".claude/commands/",
    ".codex/prompts/",
    ".cursor/commands/",
    ".cursor/cli-config.json",
    ".gemini/extensions/",
    "gemini/extensions/",
    ".qwen/extensions/",
    ".continue/config",
    ".continue/assistants/",
    ".continue/rules/",
    ".cline/rules/",
    "windsurf/memories/",
    ".codeium/windsurf/mcp_config.json",
    ".codeium/windsurf/workflows/",
    ".zcode/cli/config.json",
    ".agents/mcp.json",
    ".clinerules",
    ".windsurf/rules/",
    ".windsurfrules",
    ".cursorrules",
    ".github/copilot-instructions.md",
    ".mcp.json",
    ".opencode/agent/",
    "claude.md",
    "claude.local.md",
    "agents.md",
    "agents.local.md",
    "gemini.md",
    "qwen.md",
    "crush.md",
];

/// The population and boundary `CONTROL_SURFACE_CORPUS` enforces.
///
/// The declaration above sits on `CONTROL_SURFACE`, which is a **list**, so it
/// fingerprints the population and not the code that matches against it. A measurement
/// showed what that leaves open: three changes to the matcher rode through
/// one pull request with the registry green, two of them narrowings. This corpus
/// is the matcher's second subject — each entry is a path and the answer **both**
/// roads must keep giving it, so an edit that moves an answer reddens
/// `the_control_surface_matcher_answers_its_corpus` before review, and an edit
/// that records the new answer moves the fingerprint and reopens this claim.
pub const CONTROL_SURFACE_CORPUS_POPULATION: Population = Population {
    family: "control-surface-corpus",
    disposition: Disposition::TooTight,
    subject: "CONTROL_SURFACE_CORPUS",
    text: "the paths the control-surface matcher is held to answering, checked on both the \
           write road and the shell road. Legitimate population: the seams the matcher decides \
           at \u{2014} each of its three anchoring clauses (a leading dot, a trailing separator, \
           no separator at all) against a lookalike only that clause can hold, and both halves \
           of the anchor itself, a path that begins with a fragment as well as one that carries \
           it after a separator; a directory fragment against its bare spelling and against a \
           nested lookalike; a separator-bearing fragment matched mid-segment; the installed \
           skill tree both as the list spells it and as `skill::DIRECTORY` derives it; and \
           a bare file name, with ordinary paths so that Boundary and Routine are each exercised. Boundary: \
           `the_control_surface_matcher_answers_its_corpus` in `src/harness/tests.rs` asserts \
           every entry, and this const's own tokens are what the fingerprint covers \u{2014} \
           recording a moved answer is the re-affirmation.",
};

/// A fixed corpus of paths, each with the control-surface answer both roads must
/// give it — the behavioural half of the claim above.
///
/// Chosen at the seams rather than exhaustively: the list and the matcher decide
/// differently for a fragment naming a directory, a dot-directory, a bare file
/// name and a fragment carrying a separator, and the lookalikes at each seam are
/// here so that both answers are exercised. A matcher change that moves one of
/// these reddens its test; a refactor that leaves every answer alone moves
/// neither the test nor the fingerprint.
pub const CONTROL_SURFACE_CORPUS: &[(&str, bool)] = &[
    // Matched: a dot-directory fragment, and a file under a directory fragment.
    ("c:/users/me/.claude/settings.json", true),
    ("c:/users/me/.claude/agents/reviewer.md", true),
    // Matched: a directory fragment named with and without a trailing separator.
    ("c:/users/me/.estigia/stand-down.json", true),
    ("c:/users/me/.opencode/agents", true),
    // Matched: the installed skill tree, a bare file name, and a dot file.
    ("c:/users/me/skills/issue-flow/SKILL.md", true),
    ("c:/users/me/skills/flow/SKILL.md", true),
    ("c:/users/me/.claude.json", true),
    ("c:/users/me/agents.md", true),
    // Matched: a relative path that begins with the fragment rather than carrying
    // it after a separator, so the `starts_with` half of the anchor is held as
    // well as the `contains` half its absolute twin exercises.
    (".claude/settings.json", true),
    ("agents.md", true),
    // Matched: a directory fragment spelled in no instruction file's path.
    ("c:/users/me/.codeium/windsurf/memories/reviewer.md", true),
    // Matched: a separator-bearing fragment with no leading dot, mid-segment,
    // and a parent segment that is collapsed away before the match.
    ("c:/users/me/GitHub CLI/hosts.yml", true),
    ("c:/users/me/.claude/skills/../settings.json", true),
    // Not matched: lookalikes that merely end the same way, and a nested one.
    ("c:/users/me/.estigiaignore", false),
    ("c:/users/me/skills/flow.md", false),
    ("c:/users/me/.claude/agentsmith.md", false),
    ("c:/users/me/xyzopencode/agents", false),
    ("c:/users/me/notwindsurf/memories", false),
    // Not matched: one lookalike for each of the three anchoring clauses, each
    // shaped so that only its own clause can hold it — dropping `starts_with('.')`
    // for `my.claude/settings.json`, `ends_with('/')` for `myskills/issue-flow/`,
    // or `!contains('/')` for `myagents.md` moves an answer here.
    ("c:/users/me/my.claude/settings.json", false),
    ("c:/users/me/myskills/issue-flow/SKILL.md", false),
    ("c:/users/me/myagents.md", false),
    // Not matched: an ordinary source file, so both answers are exercised.
    ("c:/users/me/src/main.rs", false),
];

/// The population and boundary `NEUTRALISES_A_FILE` enforces.
pub const NEUTRALISES_A_FILE_POPULATION: Population = Population {
    family: "neutralising-shell",
    disposition: Disposition::TooTight,
    subject: "NEUTRALISES_A_FILE",
    text: "the commands that take a file's power away without changing a byte of it \u{2014} \
           its mode, its attributes, its owner. Legitimate population: every way to make a \
           path whose contents decide what the gate enforces inert while leaving it in place \
           and reporting as installed. Boundary: these three verbs, over a control surface or \
           the repository's own Estigia configuration.",
};

/// Commands that make a file useless without writing a byte of it.
const NEUTRALISES_A_FILE: &[&str] = &["chmod ", "chattr ", "chown "];

/// Characters a shell puts next to a path that a path segment cannot contain.
///
/// Folded to a separator before matching, so the left anchoring reaches a
/// fragment however the line is punctuated. `-` is deliberately not here: three
/// fragments carry one, and folding it would cut them in half.
const NOT_IN_A_PATH_SEGMENT: &str = "\"'`<>=|;&()$*,{}%^";

/// How sensitive a command line is, by what it names.
fn surface_of(command: &str) -> Sensitivity {
    if text_names_a_control_surface(command) {
        Sensitivity::Boundary
    } else {
        Sensitivity::Routine
    }
}

/// Whether free text names a control surface anywhere in it.
fn text_names_a_control_surface(text: &str) -> bool {
    text_names_path(text, &matches_a_fragment)
}

/// Whether free text names the repository's own Estigia configuration
/// anywhere in it, sharing the whole reader with
/// [`text_names_a_control_surface`] rather than duplicating it.
fn text_names_the_repository_configuration(text: &str) -> bool {
    text_names_path(text, &matches_repository_configuration)
}

/// Whether a shell line names a boundary: a control surface, or the
/// repository's own Estigia configuration.
///
/// The second file is deliberately **not** on the control surface, so the two
/// are different questions — and every shell arm that watches one watches both,
/// or the same line is a boundary on one road and invisible on another.
fn names_a_boundary(command: &str) -> bool {
    surface_of(command) == Sensitivity::Boundary || text_names_the_repository_configuration(command)
}

/// The reader both [`text_names_a_control_surface`] and
/// [`text_names_the_repository_configuration`] share: fold a command line or a
/// patch body into the same normalised spelling a path is matched in, per
/// token, and ask `matches` of the whole view and of every `-`/`~` ladder rung
/// beside it.
fn text_names_path(text: &str, matches: &impl Fn(&str) -> bool) -> bool {
    let command = text;
    let fold = |text: &str| {
        text.chars()
            .map(|c| {
                if NOT_IN_A_PATH_SEGMENT.contains(c) {
                    '/'
                } else {
                    c
                }
            })
            .collect::<String>()
    };
    // Per token, and joined afterwards rather than normalised as one string:
    // normalising the joined line let the parent-segment collapse reach across
    // an operand boundary and delete the surface named by the operand before
    // it (`mv .estigia ..` read `Routine`).
    let mut view = String::from("/");
    for token in command.split_whitespace() {
        view.push_str(&normalise(&fold(token)));
        view.push('/');
    }
    // A segment beginning with `-` or `~` is an option or an expansion, and the
    // shell gives no way to know where its prefix ends and the path begins.
    // Every split point from there is offered to the matcher, which can gate a
    // token that merely ends like a surface and can never miss one that is a
    // surface. Read off the **folded** token: a leading quote defeats the raw
    // one. Each rung is asked **on its own**, never appended to the view —
    // adjacent rungs concatenate into paths that were never in the command.
    let ladder = |token: &str| {
        // The same string the matcher sees: `normalise` folds `:` and `\`, and
        // a ladder over the unfused token offered no rung for
        // `${P:-.claude/settings.json}`.
        let folded = normalise(&fold(token));
        let starts_a_segment = |at: usize| at == 0 || folded.as_bytes()[at - 1] == b'/';
        folded
            .bytes()
            .enumerate()
            .filter(|&(at, byte)| (byte == b'-' || byte == b'~') && starts_a_segment(at))
            .flat_map(|(at, _)| {
                // Only within the marker's **own segment**: an option prefix ends
                // before the first separator, and unbounded rungs voided the left
                // anchoring for every token beginning with `~`.
                let segment = folded[at..].find('/').map_or(folded.len(), |end| at + end);
                (at + 1..=segment)
                    .filter(|cut| folded.is_char_boundary(*cut))
                    .map(|cut| format!("/{}/", normalise(&folded[cut..])))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    matches(&view)
        || command
            .split_whitespace()
            .flat_map(ladder)
            .any(|rung| matches(&rung))
}

/// Whether this path is one whose contents decide what the harness enforces.
///
/// Read with both separators folded to `/`, because the tool sends whatever the
/// platform hands it and half of these are written by this crate on Windows.
fn is_control_surface(target: &str) -> bool {
    matches_a_fragment(&normalise(target))
}

/// Whether `path` contains `needle` as a fragment, anchored the way every
/// fragment in `CONTROL_SURFACE` and [`matches_repository_configuration`]
/// is read rather than by a private rule of its own: a dot-directory or a
/// fragment naming a **directory** (trailing `/`) is anchored on the left,
/// because its first segment is always whole in every real target this
/// crate writes; a fragment holding **no** separator at all is anchored the
/// same way, because it names a whole file name and left unanchored would
/// reach `myagents.md` from `agents.md`; and a bare file fragment carrying a
/// separator but no leading dot is deliberately left unanchored, because
/// `cli/hosts.yml` exists to match **mid-segment** against the Windows
/// spelling `GitHub CLI/hosts.yml`.
fn is_anchored_fragment(path: &str, needle: &str) -> bool {
    if needle.starts_with('.') || needle.ends_with('/') || !needle.contains('/') {
        path.starts_with(needle) || path.contains(&format!("/{needle}"))
    } else {
        path.contains(needle)
    }
}

/// Whether this path is the repository's own Estigia configuration — the
/// shape `skill::repository_config_path` composes, wherever
/// `git_common_dir` resolves it. An ordinary checkout answers under
/// `<repo>/.git/estigia/…`; a linked worktree shares that same answer with
/// the checkout it was added from; a submodule's is
/// `<super>/.git/modules/<name>/estigia/…`; and a `--separate-git-dir`
/// checkout's is `<name>.git/estigia/…` — `src/skill.rs` asserts the first
/// two shapes on purpose. Only the last two path segments are common to
/// every one of those, which is what [`matches_repository_configuration`]
/// matches on. Only the write-tool road consults it — the shell road asks
/// `text_names_the_repository_configuration` instead.
fn is_repository_configuration(target: &str) -> bool {
    matches_repository_configuration(&normalise(target))
}

/// The same question as [`is_repository_configuration`], over an
/// already-normalised path — split out so [`text_names_the_repository_configuration`]
/// can apply it per token. Matched with [`is_anchored_fragment`], the rule every
/// other path fragment in this module is read by. The fragment is
/// `estigia/{LOCAL_FILE}` rather than `.git/estigia/{LOCAL_FILE}`: only the last
/// two segments are common to every shape `git_common_dir` produces.
fn matches_repository_configuration(path: &str) -> bool {
    is_anchored_fragment(path, &format!("estigia/{}", crate::config::LOCAL_FILE))
}

/// One path, folded and collapsed into the spelling the fragments are written
/// in. Split out from the matching so that `surface_of` can apply it **per
/// token**.
fn normalise(target: &str) -> String {
    // A drive prefix is a separator too: `C:.estigia\stand-down.json` is the
    // drive-relative spelling and names the same file as
    // `.estigia/stand-down.json`.
    let mut path = target.replace(['\\', ':'], "/").to_ascii_lowercase();
    // A separator that names nothing does not make this another file; a matcher
    // a redundant separator defeats is measuring a spelling, not a path.
    while path.contains("/./") || path.contains("//") {
        path = path.replace("/./", "/").replace("//", "/");
    }
    // A parent segment is the same class: the declared boundary is a hard link,
    // a junction or a copy, and a `..` is none of them.
    //
    // One segment at a time, and never past the start: over-collapsing costs a
    // false positive, under-collapsing costs the boundary.
    while let Some(at) = path.find("/../") {
        let before = &path[..at];
        let Some(cut) = before.rfind('/') else { break };
        if &before[cut + 1..] == ".." {
            break;
        }
        path = format!("{}{}", &path[..cut], &path[at + 3..]);
    }
    path
}

/// Whether an already-normalised path names something on the control surface.
fn matches_a_fragment(path: &str) -> bool {
    let path = path.to_string();
    // The skill tree is derived from `skill::DIRECTORY` rather than spelled, so
    // a rename cannot leave the contract writable on a `Routine` answer.
    let installed = format!("skills/{}/", crate::skill::DIRECTORY);
    // The instruction files, derived from the adapter table for the same reason
    // the skill tree is derived from `skill::DIRECTORY`: a hand-spelled copy
    // agrees with the installer only until somebody renames one.
    let anchored = |needle: &str| is_anchored_fragment(&path, needle);
    let names = |fragment: &str| {
        fragment.strip_suffix('/').map_or_else(
            || anchored(fragment),
            |bare| {
                // Both sides: the right by the trailing separator, the left so a
                // path merely *ending* the same way does not match through
                // `ends_with`.
                anchored(fragment) || path == bare || path.ends_with(&format!("/{bare}"))
            },
        )
    };

    names(&installed)
        || crate::setup::every_adapter().any(|adapter| {
            names(adapter.instruction_fragment())
                // The skills tree around Estigia's own installed contract.
                || names(adapter.skills_root_fragment())
        })
        || CONTROL_SURFACE.iter().any(|fragment| names(fragment))
        || matches_relocated_claude_config(&path)
        || matches_relocated_pi_agent_dir(&path)
}

/// Whether `path` (already normalised) is a watched name directly under the
/// resolved `$CLAUDE_CONFIG_DIR`.
fn matches_relocated_claude_config(path: &str) -> bool {
    matches_relocated_root(path, crate::setup::claude_config_dir(), |first| {
        first == ".claude.json"
            || first == "claude.md"
            || first == "claude.local.md"
            || first == "agents"
            || first == "skills"
            || first.starts_with("settings")
    })
}

/// Whether `path` (already normalised) is a file Pi reads from the agent
/// directory `$PI_CODING_AGENT_DIR` names.
///
/// The static `.pi/agent/` fragments cannot see a directory that name moved.
/// The variable is the gate process's own, so a Pi launched with a different
/// one is not covered; `docs/honesty.md` says so.
fn matches_relocated_pi_agent_dir(path: &str) -> bool {
    matches_relocated_root(path, crate::setup::pi_agent_dir(), |first| {
        matches!(first, "extensions" | "mcp.json" | "agents" | "prompts")
            || first.starts_with("settings")
            || first.starts_with("agents.")
    })
}

fn matches_relocated_root(
    path: &str,
    root: Option<std::path::PathBuf>,
    watched: fn(&str) -> bool,
) -> bool {
    let Some(root) = root else {
        return false;
    };
    let root = normalise(&root.display().to_string());
    // The shell road folds every `NOT_IN_A_PATH_SEGMENT` byte and every
    // whitespace run into a separator before it asks, so a root carrying one is
    // spelled with `/` there while the write road spells it as it is. Both
    // spellings are asked; without the second, a root with a space is a boundary
    // on one road and `Routine` on the other.
    let fold = |run: usize| {
        root.chars()
            .map(|c| {
                if c.is_whitespace() || NOT_IN_A_PATH_SEGMENT.contains(c) {
                    "/".repeat(run)
                } else {
                    c.to_string()
                }
            })
            .collect::<String>()
    };
    let folded = normalise(&fold(1));
    // The escaped spelling of a space leaves the folded byte and the token
    // boundary side by side, so the shell view carries `//` where the folded
    // root carries one `/`. Ask the doubled spelling as well, or the shell road
    // misses a root it folds while the write road answers `Boundary`. It is not
    // normalised: collapsing would fold the run this spelling is for.
    let doubled = fold(2);
    under_root(path, &root, watched)
        || under_root(path, &folded, watched)
        || under_root(path, &doubled, watched)
}

/// Whether `path` names one of the watched files directly under `root`.
///
/// `root` must be a whole path prefix — the path's own start, or after a
/// separator — so a path that merely carries the root's text mid-segment is not
/// under it.
fn under_root(path: &str, root: &str, watched: fn(&str) -> bool) -> bool {
    let anchor = format!("{}/", root.trim_end_matches('/'));
    let Some(at) = path.find(&anchor) else {
        return false;
    };
    if at > 0 && path.as_bytes()[at - 1] != b'/' {
        return false;
    }
    let relative = &path[at + anchor.len()..];
    let first = relative.split('/').next().unwrap_or("");
    watched(first)
}

/// The bare binary name a word carries, once a path prefix, a `.exe` suffix and
/// surrounding quotes are removed.
fn binary_name(word: &str) -> &str {
    let last = word
        .trim_matches(['"', '\''])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(word);
    last.strip_suffix(".exe").unwrap_or(last)
}

/// The command line with every spelling of this binary reduced to `estigia`.
fn as_estigia(command: &str) -> String {
    command
        .split_whitespace()
        .map(|word| {
            if binary_name(word) == "estigia" {
                "estigia"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Options a command carries **before** its subcommand, which take a value of
/// their own. Spelled lowercase, because the line is lowercased before it gets
/// here.
const TAKES_THE_NEXT_WORD: &[&str] = &[
    "-c",
    "--config-env",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--attr-source",
    "--super-prefix",
    "--exec-path",
    "-r",
    "--repo",
];

/// The command line with a subcommand's own options moved out of the way, so
/// `git -c core.hooksPath=/dev/null push` cannot hide a boundary behind a
/// leading option — and so a global option between `estigia` and its verb
/// cannot hide a listed `estigia` fragment the same way.
fn without_leading_options(command: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut words = command.split_whitespace().peekable();
    while let Some(word) = words.next() {
        kept.push(word);
        if word != "git" && word != "gh" && binary_name(word) != "estigia" {
            continue;
        }
        // Its own options, up to the subcommand. A word that does not start
        // with `-` is the subcommand, and the walk stops there.
        while let Some(option) = words.peek() {
            if !option.starts_with('-') {
                break;
            }
            let option = words.next().unwrap_or_default();
            if TAKES_THE_NEXT_WORD.contains(&option)
                && words.peek().is_some_and(|next| !next.starts_with('-'))
            {
                words.next();
            }
        }
    }
    kept.join(" ")
}

/// The population and boundary `DISARMS_THE_HARNESS` enforces.
pub const DISARMS_THE_HARNESS_POPULATION: Population = Population {
    family: "disarming-shell",
    disposition: Disposition::TooTight,
    subject: "DISARMS_THE_HARNESS",
    text: "the commands that stand the gate down, remove it, or widen what it permits. \
           Estigia's own subcommands, and **the ones that disarm it without naming it** \u{2014} \
           `core.hooksPath` points git away from every hook in this repository, so the push \
           guard stops running with nothing on the line that says so. Legitimate population: \
           any shell command that reduces what this harness enforces. Boundary: matched on \
           substrings of the command line, so a spelling that reaches the same effect another \
           way \u{2014} an alias, a wrapper script, editing `~/.estigia` by hand \u{2014} is not \
           matched. Completeness over Estigia's own verbs is crossed against the parser's tree \
           by `the_gate_measures_the_commands_that_switch_it_off`, which holds every leaf below \
           a verb named here to this list or to `DISARM_READS`.",
};

/// Shell fragments that turn Estigia's own enforcement off, or loosen it.
const DISARMS_THE_HARNESS: &[&str] = &[
    "estigia stand-down",
    "estigia guard --uninstall",
    "estigia uninstall",
    "estigia config set",
    "estigia config edit",
    "estigia config forget",
    "estigia setup",
    "estigia install",
    "estigia tui",
    // The key alone, not `git config core.hooksPath`: a flag between the two
    // words defeats the longer fragment. Lowercase because the line is.
    "core.hookspath",
];

/// The subcommands of a verb `DISARMS_THE_HARNESS` names that only read, so
/// their absence from it is deliberate rather than an oversight.
///
/// Crossed against the parser's own tree by
/// `the_gate_measures_the_commands_that_switch_it_off`: every leaf below a verb
/// the list names is gated or one of these, so a subcommand that writes fails
/// that test until somebody decides which side it is on.
///
/// `pub` for the reason the declarations in
/// [`crate::harness::population`] are: a `pub` item in a library target is
/// never dead code, and this is a statement of what the gate deliberately does
/// not watch rather than an implementation detail.
pub const DISARM_READS: &[&str] = &["estigia config list", "estigia config repos"];

/// The population and boundary `DISARM_READS` enforces.
pub const DISARM_READS_POPULATION: Population = Population {
    family: "disarm-reads",
    disposition: Disposition::FailClosed,
    subject: "DISARM_READS",
    text: "the subcommands of a verb `DISARMS_THE_HARNESS` names that change nothing, and whose \
           absence from that list is deliberate. Legitimate population: every spelling of \
           `estigia config` that only reads. Boundary: exact equality against the parser's own \
           leaf spellings, crossed by `the_gate_measures_the_commands_that_switch_it_off` \u{2014} \
           a leaf outside this list and outside the disarm list fails rather than being cleared, \
           so a subcommand that writes cannot arrive unclassified.",
};

/// The verb one disarming spelling names, when it names one: the first word
/// after `estigia`, since gating keys on the pair (`estigia guard --uninstall`).
pub fn disarming_verb(fragment: &str) -> Option<&str> {
    fragment
        .strip_prefix("estigia ")
        .map(|tail| tail.split_whitespace().next().unwrap_or(tail))
}

/// The population and boundary `IRREVERSIBLE` enforces.
pub const IRREVERSIBLE_POPULATION: Population = Population {
    family: "irreversible-shell",
    disposition: Disposition::TooTight,
    subject: "IRREVERSIBLE",
    text: "the delivery steps `SKILL.md` calls irreversible boundaries \u{2014} publishing a \
           review target, merging, tagging, releasing. Legitimate population: any shell command \
           that makes one of them happen. Boundary: matched as whole commands within the \
           command line by [`contains_whole_command`], so a command that reaches the same \
           effect by another spelling (an alias, a script) is not matched. The one spelling \
           named here that is now matched is `gh api` that changes something, by \
           [`mutates_through_the_api`]: a mutating method, **or any parameter** of that \
           `gh api` command, because gh's own default turns the second into the first. `git tag -l`, `git tag --list` and \
           bare `git tag` list and are not boundaries; every other `git tag` spelling stays \
           one, including `git tag -d`, which deletes rather than delivers but stays here \
           because it is destructive and rides the same renewal window.",
};

/// Shell fragments that mark a step nobody can take back.
const IRREVERSIBLE: &[&str] = &[
    "git push",
    "git merge",
    "git pull",
    "git tag",
    "gh pr merge",
    "gh pr create",
    "gh release create",
    "gh release edit",
];

/// Whether `fragment` appears in `text` as a whole command rather than as the
/// beginning of a longer one: the fragment ends where the command does or at a
/// space. A subcommand wearing the same prefix (`git merge-base`) is a
/// different command.
fn contains_whole_command(text: &str, fragment: &str) -> bool {
    let mut from = 0;
    while let Some(at) = text[from..].find(fragment) {
        let start = from + at;
        let end = start + fragment.len();
        // A boundary at the end of the text, or followed by a separator. Not by
        // `-`, which is how `merge-base` and `merge-tree` are spelled.
        if text[end..]
            .chars()
            .next()
            .is_none_or(|next| next.is_whitespace())
        {
            return true;
        }
        from = start + 1;
    }
    false
}

fn git_tag_needs_boundary(normalized: &str) -> bool {
    let fragment = "git tag";
    let mut from = 0;
    while let Some(at) = normalized[from..].find(fragment) {
        let start = from + at;
        let end = start + fragment.len();
        if normalized[end..]
            .chars()
            .next()
            .is_none_or(|next| next.is_whitespace())
        {
            let rest = &normalized[end..];
            let seg_end = rest
                .find([';', '&', '|', '(', ')', '\n'])
                .unwrap_or(rest.len());
            let mut words = rest[..seg_end].split_whitespace();
            let Some(first) = words.next() else {
                from = start + 1;
                continue;
            };
            let first = first.trim_matches(|next| next == '\'' || next == '"');
            let lists = first == "-l" || first == "--list";
            if !lists {
                return true;
            }
        }
        from = start + 1;
    }
    false
}

pub(crate) fn delivery_pr_unidentified(reason: Option<String>) -> Refusal {
    let message = match &reason {
        Some(byte) => format!(
            "the merge command carries a `{byte}` byte the literal parser cannot establish an \
             identity for, so no review transaction can be selected"
        ),
        None => "the merge command does not name one positive numeric pull request, so no review \
             transaction can be selected"
            .to_owned(),
    };
    let detail = match reason {
        Some(reason) => {
            format!(
                "the command carries shell syntax the literal parser cannot establish an identity \
                 for: the byte `{reason}` disqualified it. One literal `gh pr merge <number> ...` \
                 command, the number in the fourth position — before every flag — and spelled \
                 without leading zeros, and with no quoting, escaping, expansion or redirect"
            )
        }
        None => {
            "one literal `gh pr merge <number> ...` command, the number in the fourth position — \
             before every flag — and spelled without leading zeros"
                .to_owned()
        }
    };
    Refusal::not_started(
        "delivery-pr-unidentified",
        message,
        Resolution::no_command(NoCommandReason::OperatorKnowledge, detail),
    )
}

pub(crate) fn delivery_pr_repository_option() -> Refusal {
    Refusal::not_started(
        "delivery-pr-repository-option",
        "the merge command names a repository with `--repo` or `-R`, and the gate reads only the \
         spelling that targets the checkout's own repository, so no review transaction can be \
         selected"
            .to_owned(),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "run the same `gh pr merge <number> ...` command from the checkout, without `--repo` \
             or `-R`",
        ),
    )
}

pub(crate) fn complete_review_transaction_missing(pr: u64) -> Refusal {
    Refusal::not_started(
        "complete-review-transaction-missing",
        format!(
            "PR #{pr} has no complete local review transaction; a legacy reviewed head cannot \
             select a PR lineage"
        ),
        Resolution::no_command(
            NoCommandReason::HumanAuthority,
            "the immutable epoch, PR, head, base and digest restored for this exact PR",
        ),
    )
}

pub(crate) fn delivery_pr_mismatch(command_pr: u64, receipt_pr: u64) -> Refusal {
    Refusal::not_started(
        "delivery-pr-mismatch",
        format!(
            "the merge targets PR #{command_pr}, but this run's review transaction is for PR \
             #{receipt_pr}"
        ),
        Resolution::no_command(
            NoCommandReason::HumanAuthority,
            "the complete review transaction for the pull request being merged",
        ),
    )
}

pub(crate) fn complete_review_transaction_not_selected() -> Refusal {
    Refusal::not_started(
        "complete-review-transaction-not-selected",
        "this linked checkout has no unique complete review transaction for the pull request \
         named by the merge command"
            .to_owned(),
        Resolution::no_command(
            NoCommandReason::HumanAuthority,
            "one selected holder whose complete receipt names this exact pull request",
        ),
    )
}

/// The bytes a merge command may carry before its PR identity is trustworthy:
/// alphanumerics plus the separators and option spellings a literal command
/// needs.
fn allowed_pr_merge_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b' ' | b'\t' | b'-' | b'_' | b'.' | b'/' | b':' | b'=' | b'+'
        )
}

/// The first byte that disqualifies a merge command from retaining a PR, so
/// the refusal can say which byte did it instead of claiming the command names
/// no PR.
fn pr_merge_disqualifying_byte(command: &str) -> Option<char> {
    command
        .bytes()
        .find(|byte| !allowed_pr_merge_byte(*byte))
        .map(char::from)
}

fn names_a_repository(word: &str) -> bool {
    word == "--repo"
        || word.starts_with("--repo=")
        || word
            .strip_prefix('-')
            .is_some_and(|short| !short.starts_with('-') && short.contains('R'))
}

/// Whether a merge command the byte filter admits names a repository, which
/// is why `pr_merge_target` refuses a line that does carry a number.
fn pr_merge_names_a_repository(command: &str) -> bool {
    if !command.bytes().all(allowed_pr_merge_byte) {
        return false;
    }
    let words: Vec<&str> = command.split_ascii_whitespace().collect();
    words.get(..3) == Some(&["gh", "pr", "merge"][..])
        && words.iter().any(|word| names_a_repository(word))
}

/// The one PR target that can be retained without becoming a shell parser: the
/// number is the fourth word — before every flag — spelled as digits with no
/// leading zero. A retained number selects which review transaction may
/// authorise the merge, so only a command needing no shell interpretation can
/// select one.
fn pr_merge_target(command: &str) -> Option<u64> {
    if !command.bytes().all(allowed_pr_merge_byte) {
        return None;
    }
    let words: Vec<&str> = command.split_ascii_whitespace().collect();
    if words.get(..3) != Some(&["gh", "pr", "merge"][..])
        || words.iter().any(|word| names_a_repository(word))
    {
        return None;
    }
    let target = *words.get(3)?;
    // A leading zero is a number the payload did not spell: `007` parses to `7`,
    // so the spelling is taken literally or left unidentified, never
    // interpreted.
    if !target.bytes().all(|byte| byte.is_ascii_digit())
        || (target.len() > 1 && target.starts_with('0'))
    {
        return None;
    }
    let pr = target.parse::<u64>().ok()?;
    if pr == 0
        || words
            .get(4..)
            .unwrap_or_default()
            .iter()
            .any(|word| word.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(pr)
}

/// A delivery about to spend a verdict bound to bytes that have moved.
///
/// **Additive, and only after a review exists.** No recorded head or a command
/// that is not a delivery leaves the decision exactly as it was before this
/// existed. It sees what this run published **through Estigia's own tools**.
///
/// `gh pr merge` is bound to the PR head `pr_head` reports, not to
/// `checkout`'s own `HEAD`; every other delivery in `DELIVERS`
/// still compares against `checkout`, where `checkout` is the directory a git
/// command's own `cd` or `-C` names. A `git tag`, and an exact `git merge
/// --ff-only`, are also admitted on the run's own merge commit.
pub(crate) fn stale_verdict(
    action: &Action,
    run: &Run,
    checkout: &std::path::Path,
    pr_head: &dyn Fn(u64) -> Result<String, crate::transport::Failure>,
) -> Option<Refusal> {
    let Action::Boundary {
        command,
        pr,
        pr_unidentified_reason,
        ..
    } = action
    else {
        return None;
    };
    if !DELIVERS.contains(&command.as_str()) {
        return None;
    }
    if command == "gh pr merge" {
        let command_pr = match pr {
            Some(pr) => *pr,
            None => {
                return Some(match pr_unidentified_reason {
                    Some(PrUnidentified::RepositoryOption) => delivery_pr_repository_option(),
                    Some(PrUnidentified::Byte(byte)) => {
                        delivery_pr_unidentified(Some(byte.clone()))
                    }
                    None => delivery_pr_unidentified(None),
                });
            }
        };
        let Some(receipt) = run
            .review_transaction
            .as_ref()
            .filter(|receipt| receipt.is_complete())
        else {
            return Some(complete_review_transaction_missing(command_pr));
        };
        if receipt.pr != command_pr {
            return Some(delivery_pr_mismatch(command_pr, receipt.pr));
        }
        if let Some(refusal) = checkout_refusal(covers_checkout(run, checkout), checkout) {
            return Some(refusal);
        }
        return match pr_head(command_pr) {
            Ok(found) if found == receipt.head => None,
            Ok(found) => Some(Refusal::stale(
                "verdict-bound-to-other-bytes",
                format!(
                    "PR #{command_pr} was reviewed at {} and its head is now {} \u{2014} every \
                     push invalidates the verdict and the CI run that went with it",
                    short(&receipt.head),
                    short(&found)
                ),
                Resolution::no_command(
                    NoCommandReason::HumanAuthority,
                    "a review of the bytes being delivered: publish the target again and ask \
                     for a verdict on the new head",
                ),
            )),
            Err(failure) => tracker::translate(
                &tracker::Answer {
                    code: failure.code(),
                    body: Some(failure.envelope()),
                },
                "boundary",
            ),
        };
    }
    let reviewed = run
        .review_transaction
        .as_ref()
        .filter(|receipt| receipt.is_complete())
        .map(|receipt| receipt.head.as_str())
        .or(run.reviewed_head.as_deref())?;
    let Some(checkout) = directory_of(action, checkout) else {
        return Some(Refusal::unavailable(
            "review-checkout-unreadable",
            format!(
                "the review was published against {} and this command changes directory in a \
                 way that cannot be read, so the gate cannot tell which checkout it runs in",
                short(reviewed)
            ),
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "spell the directory as a plain path, `cd <path> && git merge <ref>` or `git -C \
                 <path> merge <ref>`, or run the merge from inside the checkout",
            ),
        ));
    };
    let checkout = checkout.as_path();
    if matches!(
        action,
        Action::Boundary {
            command,
            is_exact_integration_merge: true,
            ..
        } if command == "git merge"
    ) && is_integration_in_the_runs_own_checkout(run, checkout)
    {
        return None;
    }
    let relation = covers_checkout(run, checkout);
    if relation == Relation::Different {
        return checkout_refusal(relation, checkout);
    }
    let now = head_of(checkout);
    let Some(found) = now.as_deref().map(short) else {
        return Some(Refusal::unavailable(
            "review-head-unreadable",
            format!(
                "the review was published against {} and the head of checkout {} cannot be read, \
                 so nothing can be said about whether the verdict still holds",
                short(reviewed),
                checkout.display()
            ),
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a checkout whose head `git rev-parse HEAD` can read; then repeat the command",
            ),
        ));
    };
    if let Some(refusal) = checkout_refusal(relation, checkout) {
        return Some(refusal);
    }
    let head = now.as_deref().unwrap_or_default();
    let receipt = run
        .review_transaction
        .as_ref()
        .filter(|receipt| receipt.is_complete());
    let delivers = |commit: &str| {
        receipt.is_some_and(|receipt| is_the_receipts_merge(receipt, checkout, commit))
    };
    let (judged, accepted) = match action {
        Action::Boundary {
            command,
            target: CommitTarget::Named(named),
            ..
        } if command == "git tag" => match commit_of(checkout, named) {
            Some(commit) => {
                let accepted = commit == reviewed || delivers(&commit);
                (short(&commit), accepted)
            }
            None => (found, head == reviewed),
        },
        Action::Boundary {
            command,
            target: CommitTarget::Checkout,
            ..
        } if command == "git tag" => (found, head == reviewed || delivers(head)),
        Action::Boundary {
            command,
            target: CommitTarget::Named(named),
            local_fast_forward_target: Some(_),
            ..
        } if command == "git merge" => (
            found,
            head == reviewed || commit_of(checkout, named).is_some_and(|commit| delivers(&commit)),
        ),
        _ => (found, head == reviewed),
    };
    if accepted {
        return None;
    }
    Some(Refusal::stale(
        "verdict-bound-to-other-bytes",
        format!(
            "the review was published against {} and checkout {} is at {} \u{2014} every push \
             invalidates the verdict and the CI run that went with it",
            short(reviewed),
            checkout.display(),
            judged
        ),
        Resolution::no_command(
            NoCommandReason::HumanAuthority,
            "a review of the bytes being delivered: publish the target again and ask for a \
             verdict on the new head, or reset this checkout to the head that was reviewed",
        ),
    ))
}

fn covers_checkout(run: &Run, checkout: &std::path::Path) -> Relation {
    let relations: Vec<Relation> = run
        .covered()
        .map(|covered| repository_relation(covered, checkout))
        .collect();
    if relations.contains(&Relation::Same) {
        Relation::Same
    } else if relations.contains(&Relation::Unreadable) {
        Relation::Unreadable
    } else {
        Relation::Different
    }
}

fn checkout_refusal(relation: Relation, checkout: &std::path::Path) -> Option<Refusal> {
    match relation {
        Relation::Same => None,
        Relation::Different => Some(Refusal::not_started(
            "verdict-bound-to-other-repository",
            format!(
                "checkout {} is not a checkout of the repository the review was published in",
                checkout.display()
            ),
            Resolution::no_command(
                NoCommandReason::HumanAuthority,
                "a review of the bytes being delivered: publish the target again and ask for a \
                 verdict on the new head",
            ),
        )),
        Relation::Unreadable => Some(Refusal::unavailable(
            "review-checkout-unreadable",
            format!(
                "git could not say whether checkout {} belongs to the repository the review was \
                 published in",
                checkout.display()
            ),
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "a checkout git can read; then repeat the command",
            ),
        )),
    }
}

/// A reader of the PR head the tracker reports, pinning `GH_REPO` when the
/// claim names a tracker repository, so `gh` resolves that repository rather
/// than whatever the invoking checkout falls back to.
///
/// Everything here is deferred into the closure body: nothing runs until
/// [`stale_verdict`] calls it, which is only for `gh pr merge` and only after
/// the checks ahead of it in that function pass.
pub(crate) fn live_pr_head_reader(
    context: &GateContext,
) -> impl Fn(u64) -> Result<String, crate::transport::Failure> + '_ {
    move |number: u64| {
        let _gh_pin = gh_repo_pin();
        apply_gh_repo_locked(context.tracker.named_repo().as_deref());
        let transport = crate::transport::Context::live(
            context.skill_root.clone(),
            context.repo_dir.clone(),
            context.tracker.named_repo(),
        );
        let pr = crate::transport::claim::view_pr(&transport, number)?;
        pr.get("headRefOid")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                crate::transport::Failure::Read(format!(
                    "gh pr view {number} returned no headRefOid"
                ))
            })
    }
}

/// Whether this action spends delivery evidence rather than producing it.
pub(crate) fn is_delivery(action: &Action) -> bool {
    matches!(action, Action::Boundary { command, .. } if DELIVERS.contains(&command.as_str()))
}

/// Whether two checkouts belong to one clone and share its worktree registry.
pub(crate) fn same_git_repository(left: &std::path::Path, right: &std::path::Path) -> bool {
    repository_relation(left, right) == Relation::Same
}

/// What can be said about two checkouts: the same clone, different ones, or
/// nothing because git could not answer for one of them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Relation {
    Same,
    Different,
    Unreadable,
}

fn repository_relation(left: &std::path::Path, right: &std::path::Path) -> Relation {
    let common = |directory: &std::path::Path| {
        let answer = proof_git_command(
            directory,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .output()
        .ok()?;
        if !answer.status.success() {
            return None;
        }
        let path = String::from_utf8(answer.stdout).ok()?;
        let path = path.trim();
        (!path.is_empty()).then(|| std::path::PathBuf::from(path))
    };
    match (common(left), common(right)) {
        (Some(left), Some(right)) if crate::paths::same_directory(&left, &right) => Relation::Same,
        (Some(_), Some(_)) => Relation::Different,
        _ => Relation::Unreadable,
    }
}

/// The commit `revision` names in `directory`, when git will say; `None`
/// rather than a guess.
fn commit_of(directory: &std::path::Path, revision: &str) -> Option<String> {
    let expression = format!("{revision}^{{commit}}");
    let answer = proof_git_command(
        directory,
        &["rev-parse", "--verify", "--end-of-options", &expression],
    )
    .output()
    .ok()
    .filter(|answer| answer.status.success())?;
    let oid = String::from_utf8(answer.stdout).ok()?.trim().to_owned();
    (oid.len() == 40 && oid.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(oid)
}

/// Whether `commit` is the merge that delivered `receipt`: two parents, the
/// second the reviewed head and the first a descendant of the receipt's base.
/// Anything git cannot answer is `false`.
fn is_the_receipts_merge(
    receipt: &crate::transport::claim::ReviewTransaction,
    directory: &std::path::Path,
    commit: &str,
) -> bool {
    let Some(parents) = proof_git_command(
        directory,
        &[
            "rev-list",
            "--parents",
            "-n",
            "1",
            "--end-of-options",
            commit,
        ],
    )
    .output()
    .ok()
    .filter(|answer| answer.status.success())
    .and_then(|answer| String::from_utf8(answer.stdout).ok()) else {
        return false;
    };
    let [listed, first, second] = parents.split_ascii_whitespace().collect::<Vec<_>>()[..] else {
        return false;
    };
    listed == commit
        && receipt.delivered_by_merge(
            second,
            proof_git_command(
                directory,
                &["merge-base", "--is-ancestor", &receipt.base, first],
            )
            .output()
            .is_ok_and(|answer| answer.status.success()),
        )
}

/// The first seven of a SHA, which is what a person reads.
fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// This checkout's head, when git will say; `None` rather than a guess.
fn head_of(directory: &std::path::Path) -> Option<String> {
    let answer = proof_git_command(directory, &["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !answer.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&answer.stdout).trim().to_owned();
    (!head.is_empty()).then_some(head)
}

/// Whether a line calls the tracker's API in a way that changes something.
///
/// Read per command, from that command's own words, by the reader the
/// transport's tripwire applies to the calls Estigia spawns: a flag belonging
/// to another command on the line decides nothing, and an explicit `GET` stays
/// a read whatever fields it carries.
fn mutates_through_the_api(normalized: &str) -> bool {
    operation_window::shell_commands(normalized)
        .iter()
        .any(|command| {
            let words: Vec<&str> = command
                .split_whitespace()
                .map(|word| word.trim_matches(|c| c == '"' || c == '\''))
                .collect();
            words
                .iter()
                .position(|word| *word == "gh" || word.ends_with("/gh"))
                .is_some_and(|at| {
                    let mut call = words[at..].to_vec();
                    call[0] = "gh";
                    crate::transport::window::gh_api_writes(&call)
                })
        })
}

/// The population and boundary `REPOSITORY_SHELL` enforces.
pub const REPOSITORY_SHELL_POPULATION: Population = Population {
    family: "repository-shell",
    disposition: Disposition::TooTight,
    subject: "REPOSITORY_SHELL",
    text: "the git commands that change the working tree, the index or a ref. Legitimate \
           population: every spelling of a repository write that *names git*. Boundary: \
           matched on substrings, so an alias or a script wrapping one of these is not matched. \
           `git stash list` and `git stash show` are reads and are exempt at every occurrence.",
};

fn names_fragment(code: &str, fragment: &str) -> bool {
    if fragment != "git stash" {
        return code.contains(fragment);
    }
    code.match_indices(fragment).any(|(at, _)| {
        let rest = &code[at + fragment.len()..];
        let subcommand = rest
            .trim_start()
            .split(|c: char| c.is_whitespace() || ";|&)".contains(c))
            .next();
        let reads =
            rest.starts_with(char::is_whitespace) && matches!(subcommand, Some("list" | "show"));
        !reads
    })
}

/// Shell fragments that write to the repository without being irreversible.
const REPOSITORY_SHELL: &[&str] = &[
    "git worktree add",
    "git checkout -b",
    "git switch -c",
    "git switch --create",
    "git switch --force-create",
    "git switch --orphan",
    "git commit",
    "git rebase",
    "git reset --hard",
    "git cherry-pick",
    "gh issue develop",
    "git branch -d",
    "git branch --delete",
    "git checkout --",
    "git checkout .",
    "git restore",
    "git stash",
    "git clean",
    "git apply",
    "git am",
    "git rm",
    "git mv",
    "git revert",
    "git reset --keep",
    "git reset --merge",
    "git submodule update",
    "git worktree remove",
    "git filter-branch",
];

/// The population and boundary `DELIVERS` enforces.
pub const DELIVERS_POPULATION: Population = Population {
    family: "delivery-phase",
    disposition: Disposition::TooTight,
    subject: "DELIVERS",
    text: "the built-in irreversible spellings that end a delivery rather than open one. \
           Legitimate population: every step that lands work on a shared ref. One issue may \
           land several pull requests \u{2014} a chained delivery merges each slice on its own \
           \u{2014} so the population counts a step, not a delivery, and a second `gh pr merge` \
           on the same issue is the same spelling. Boundary: \
           matched by exact equality against the fragment [`classify_with`] recorded, so an \
           operator-declared boundary is never one of these \u{2014} Estigia cannot know \
           whether somebody's `make deploy` delivers or rehearses, and guessing would deny a \
           step it never understood.",
};

/// The boundaries that *deliver*, as opposed to the ones that publish.
const DELIVERS: &[&str] = &[
    "git merge",
    "git tag",
    "gh pr merge",
    "gh release create",
    "gh release edit",
];

/// How strictly [`stale_verdict`] checks a matched fragment, so a compound
/// line records the fragment the strictest check reads rather than whichever
/// the list names first.
fn delivery_tier(fragment: &str) -> u8 {
    if fragment == "gh pr merge" {
        2
    } else if DELIVERS.contains(&fragment) {
        1
    } else {
        0
    }
}

/// The built-in `IRREVERSIBLE` fragments among `matched` that share a tier with
/// another such fragment, when one tier holds two or more — the first tier in
/// ascending order, so the answer is deterministic. An operator-declared
/// fragment is never counted: `DELIVERS_POPULATION` already says Estigia cannot
/// judge whether one delivers, and the built-in copy of any spelling it shares
/// is already counted.
fn tied_tier_fragments(matched: &[(usize, &str)]) -> Vec<String> {
    let mut by_tier: std::collections::BTreeMap<u8, Vec<&str>> = std::collections::BTreeMap::new();
    for (index, fragment) in matched {
        if *index < IRREVERSIBLE.len() {
            by_tier
                .entry(delivery_tier(fragment))
                .or_default()
                .push(*fragment);
        }
    }
    by_tier
        .into_values()
        .find(|at_tier| at_tier.len() >= 2)
        .map(|at_tier| at_tier.into_iter().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The refusal for a line carrying more than one irreversible fragment of one
/// tier: naming one would decide the line on an answer that was never about the
/// other.
pub(crate) fn several_irreversible_fragments(fragments: &[String]) -> Refusal {
    let named = fragments
        .iter()
        .map(|fragment| format!("`{fragment}`"))
        .collect::<Vec<_>>()
        .join(", ");
    Refusal::not_started(
        "several-irreversible-fragments-on-one-line",
        format!(
            "this command line carries more than one irreversible fragment of the same tier \
             ({named}), so recording one of them would decide the line on an answer that was \
             never about the other"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "one irreversible step per line: split the line so each command is adjudicated on its \
             own",
        ),
    )
}

/// The workflow states in which a delivery can be bound to a verdict, taken
/// from `bindings/github.md`. `done` is here beside `review` because the
/// transition that closes an issue can land before the last delivery step.
const DELIVERY_STATES: &[&str] = &["review", "done"];

/// Whether this boundary is a delivery the issue's state cannot support.
pub(crate) fn out_of_phase(
    command: &str,
    state: &str,
    issue: &crate::identity::Identity,
    integration: crate::config::Integration,
    flag: Option<&str>,
    run: &Run,
    repo_dir: &std::path::Path,
) -> Option<Refusal> {
    if !DELIVERS.contains(&command) || DELIVERY_STATES.contains(&state) {
        return None;
    }
    if integration.admits_unreviewed(flag) {
        return None;
    }
    if matches!(integration, crate::config::Integration::Trunk) {
        return Some(Refusal::not_started(
            "unflagged-on-trunk",
            [
                "this lands on trunk from {state}, where no verdict exists, and no",
                "feature flag was named for issue #{issue}",
            ]
            .join(" ")
            .replace("{state}", state)
            .replace("{issue}", &issue.to_string()),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                [
                    "the flag this change is behind, declared with ESTIGIA_FLAG —",
                    "or a review, if it is landing switched on",
                ]
                .join(" "),
            ),
        ));
    }
    // `state != "in-progress"` is checked here, not left to the call site's
    // disjunct alone: `is_integration_in_the_runs_own_checkout` spawns its own
    // `git` and is evaluated twice, so the two calls are not guaranteed to
    // agree.
    if command == "git merge"
        && state != "in-progress"
        && is_integration_in_the_runs_own_checkout(run, repo_dir)
    {
        return Some(Refusal::not_started(
            "integration-out-of-phase",
            format!(
                "this integrates the base into this run's own checkout, which is permitted only \
                 from in-progress, and issue #{issue} is in {state}"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the transition of this issue back to in-progress, then the integration again",
            ),
        ));
    }
    Some(Refusal::not_started(
        "out-of-phase",
        format!(
            "this step lands the work and issue #{issue} is in {state}, where no verdict exists — \
             a delivery counts only against one"
        ),
        Resolution::no_command(
            NoCommandReason::HumanAuthority,
            "a review of this head. Publish the review target, move the issue to review, and \
             deliver once somebody has answered",
        ),
    ))
}

/// The refusal a near-miss integration earns.
///
/// A `git merge` in this run's own checkout whose shape is not the one literal
/// [`exact_integration_merge`] proves is refused either way; this exists so the
/// answer names the shape that would be admitted instead of calling the line a
/// delivery that needs a verdict. It is a refusal and never an allow, and it
/// fires only where the exemption lives — `in-progress` — so `out_of_phase`'s
/// own answer for every other state is untouched.
fn near_miss_integration(
    action: &Action,
    run: &Run,
    session: &std::path::Path,
    integration: crate::config::Integration,
    flag: Option<&str>,
) -> Option<Refusal> {
    let Action::Boundary {
        command,
        is_exact_integration_merge: false,
        local_fast_forward_target: None,
        ..
    } = action
    else {
        return None;
    };
    if command != "git merge"
        || integration.admits_unreviewed(flag)
        || matches!(integration, crate::config::Integration::Trunk)
    {
        return None;
    }
    let directory = directory_of(action, session)?;
    if !is_integration_in_the_runs_own_checkout(run, &directory) {
        return None;
    }
    Some(Refusal::not_started(
        "integration-shape-not-exact",
        "this is a `git merge` in this run's own checkout, but its shape is not the one the gate \
         proves as a base integration. The accepted literal is exactly one bare command — \
         `git -C <worktree> merge --no-edit origin/<base>` — with nothing piped, chained or \
         redirected beside it",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the integration again as that one bare command",
        ),
    ))
}

/// Classifies one tool call.
pub fn classify(tool: &str, input: &serde_json::Value) -> (Action, Sensitivity) {
    classify_with(tool, input, &[])
}

/// [`classify`], plus the boundaries this operator declared.
///
/// The built-in list is what Estigia knows about *git and GitHub*. It cannot
/// know that `npm publish`, `terraform apply` or `make deploy` are irreversible
/// in somebody's repository — and until an operator could say so, those ran as
/// routine writes and could ride the renewal window, which is the one thing a
/// boundary must never do.
///
/// **`extra` only adds.** There is no way to take a built-in boundary out, and
/// that is deliberate: a setting that could make the gate looser is a setting
/// that turns a guard rail into a preference. The operator may make it stricter
/// and nothing else.
pub fn classify_with(
    tool: &str,
    input: &serde_json::Value,
    extra: &[String],
) -> (Action, Sensitivity) {
    let (action, sensitivity, _) = classify_observing(tool, input, extra);
    (action, sensitivity)
}

/// [`classify_with`], plus what the gate read on its way to that answer.
///
/// Not folded into [`classify_with`]'s own return type: 25+ call sites
/// destructure that pair, and only the two production gates need the third
/// value — the same reason [`classify`] wraps [`classify_with`] rather than
/// the other way round.
pub fn classify_observing(
    tool: &str,
    input: &serde_json::Value,
    extra: &[String],
) -> (Action, Sensitivity, CommandObserved) {
    let name = tool.to_ascii_lowercase();
    if SHELL_TOOLS.contains(&name.as_str()) {
        // The payload itself when no key names the command: a tool that calls it
        // something else is a shell call the gate cannot see at all.
        let command = shell_command_argument(input);
        // The fragment lists want whitespace collapsed, so `git   push` matches
        // `git push`; `shell::writes_a_file` wants it kept, since it splits on
        // newlines.
        let unquoted = if command.literal.is_ok() {
            shell::unquote_plain_words(&command.text)
        } else {
            command.text.clone()
        };
        let lowercased = unquoted.to_ascii_lowercase();
        let normalized = without_leading_options(&lowercased);
        // The serialised payload's quotes are JSON's, not the shell's, so the
        // fallback is read raw: masking a JSON string would hide the command the
        // fallback exists to find. `data_view` blanks only the spans a single
        // simple command provably reads, and leaves every other line untouched.
        // Only a single string is one script for bash: joined keys or a joined
        // argv are not, so a quote in one value could blank another's boundary.
        let view = if command.literal.is_ok() {
            shell::data_view(&unquoted).to_ascii_lowercase()
        } else {
            lowercased.clone()
        };
        let code = without_leading_options(&view);
        let literal = match &command.literal {
            Ok(text) => Established::Known(text.clone()),
            Err(reason) => Established::Unestablished {
                unestablished: reason,
            },
        };
        let against = Established::Known(normalized.clone());
        // Before the delivery boundaries, because this is the one that decides
        // whether any of them are still checked afterwards.
        let disarm_view = as_estigia(&code);
        // First of every shell arm: it is a deny, so no boundary on the same line may
        // classify the line as something the deny does not read.
        if shell::writes_a_file(&view).is_some()
            && (person_only::names_the_record(&lowercased)
                || person_only::names_the_record_bare(&lowercased))
        {
            return (
                Action::Write {
                    target: person_only::RECORD.target.to_owned(),
                },
                Sensitivity::Routine,
                CommandObserved {
                    matched: Established::Known(person_only::RECORD.target.to_owned()),
                    against: Established::Known(normalized),
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        if let Some(command) = person_only::named_in(&disarm_view) {
            let command = person_only::refined(command, &lowercased);
            return (
                Action::Write {
                    target: command.target.to_owned(),
                },
                Sensitivity::Routine,
                CommandObserved {
                    matched: Established::Known(command.target.to_owned()),
                    against: Established::Known(disarm_view),
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        if let Some(matched) = DISARMS_THE_HARNESS
            .iter()
            .find(|fragment| disarm_view.contains(*fragment))
        {
            return (
                Action::Boundary {
                    command: (*matched).to_owned(),
                    pr: None,
                    pr_unidentified_reason: None,
                    local_fast_forward_target: None,
                    is_exact_integration_merge: false,
                    runs_in: crate::harness::RunsIn::Session,
                    target: crate::harness::CommitTarget::Checkout,
                },
                Sensitivity::Boundary,
                // This arm searched `disarm_view` — `normalized` with every
                // `estigia`/`estigia.exe`-basename word folded to the bare
                // word — not the shared `normalized` value the arms below it
                // report; a path prefix or `.exe` suffix would otherwise make
                // `against` a line that does not contain what `matched` names.
                CommandObserved {
                    matched: Established::Known((*matched).to_owned()),
                    against: Established::Known(disarm_view.clone()),
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        if mutates_through_the_api(&normalized) {
            return (
                Action::Boundary {
                    command: "gh api".to_owned(),
                    pr: None,
                    pr_unidentified_reason: None,
                    local_fast_forward_target: None,
                    is_exact_integration_merge: false,
                    runs_in: crate::harness::RunsIn::Session,
                    target: crate::harness::CommitTarget::Checkout,
                },
                Sensitivity::Boundary,
                CommandObserved {
                    matched: Established::Known("gh api".to_owned()),
                    against,
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        let declared = extra.iter().map(String::as_str);
        let matched_fragments: Vec<(usize, &str)> = IRREVERSIBLE
            .iter()
            .copied()
            .chain(declared)
            .enumerate()
            .filter(|(index, fragment)| {
                if *fragment == "git tag" && *index < IRREVERSIBLE.len() {
                    git_tag_needs_boundary(&code)
                } else {
                    contains_whole_command(&code, fragment)
                }
            })
            .collect();
        let same_tier_fragments = tied_tier_fragments(&matched_fragments);
        if let Some(matched) = matched_fragments
            .iter()
            .copied()
            .max_by_key(|(index, fragment)| (delivery_tier(fragment), Reverse(*index)))
            .map(|(_, fragment)| fragment)
        {
            let pr = if matched == "gh pr merge" {
                command.literal.as_deref().ok().and_then(pr_merge_target)
            } else {
                None
            };
            let pr_unidentified_reason = if matched == "gh pr merge" && pr.is_none() {
                let literal = command.literal.as_deref().ok();
                literal
                    .and_then(pr_merge_disqualifying_byte)
                    .map(|byte| PrUnidentified::Byte(byte.to_string()))
                    .or_else(|| {
                        literal
                            .is_some_and(pr_merge_names_a_repository)
                            .then_some(PrUnidentified::RepositoryOption)
                    })
            } else {
                None
            };
            let disqualifying_byte = match &pr_unidentified_reason {
                Some(PrUnidentified::Byte(byte)) => Some(byte.clone()),
                _ => None,
            };
            let directed = if DELIVERS.contains(&matched) && matched.starts_with("git ") {
                let literal_text = command.literal.as_deref().ok();
                directed_integration_merge(
                    literal_text.unwrap_or(command.text.as_str()),
                    literal_text.is_some(),
                )
            } else {
                (RunsIn::Session, false, String::new())
            };
            let local_fast_forward_target = (matched == "git merge")
                .then(|| {
                    command
                        .literal
                        .as_deref()
                        .ok()
                        .and_then(exact_local_fast_forward_target)
                })
                .flatten();
            let target = match matched {
                "git tag" => match command.literal {
                    Ok(_) => tag_target(&directed.2),
                    Err(_) => CommitTarget::Unread,
                },
                "git merge" => local_fast_forward_target
                    .clone()
                    .map_or(CommitTarget::Checkout, CommitTarget::Named),
                _ => CommitTarget::Checkout,
            };
            let boundary_source = if IRREVERSIBLE.contains(&matched) {
                "irreversible"
            } else {
                "operator-declared"
            };
            return (
                Action::Boundary {
                    command: matched.to_owned(),
                    pr,
                    pr_unidentified_reason,
                    local_fast_forward_target,
                    is_exact_integration_merge: directed.1,
                    runs_in: directed.0,
                    target,
                },
                Sensitivity::Boundary,
                CommandObserved {
                    matched: Established::Known(matched.to_owned()),
                    against,
                    literal,
                    boundary_source: Some(boundary_source),
                    disqualifying_byte,
                    same_tier_fragments,
                },
            );
        }
        if issue_creation::names_issue_creation(&normalized) {
            return (
                Action::Write {
                    target: "gh issue create".to_owned(),
                },
                Sensitivity::Routine,
                CommandObserved {
                    matched: Established::Known("gh issue create".to_owned()),
                    against,
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        if let Some(matched) = REPOSITORY_SHELL
            .iter()
            .find(|fragment| names_fragment(&code, fragment))
        {
            // A compound line carries one fragment per command, and the gate
            // decides on the one reported here: when any of them is one the
            // operation schedule may deny, that is the one reported, so the
            // schedule sees the commit riding beside the worktree it would
            // otherwise read alone.
            let target = operation_window::protected_write_target(&normalized).unwrap_or(*matched);
            let boundary = names_a_boundary(&lowercased);
            return (
                Action::Write {
                    target: target.to_owned(),
                },
                if boundary {
                    Sensitivity::Boundary
                } else {
                    Sensitivity::Routine
                },
                CommandObserved {
                    matched: Established::Known(target.to_owned()),
                    against,
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        // Plain and porcelain local branch creation, which no fragment above
        // names: deletions matched there first, so reaching this line with
        // `git branch` still on it means the command names no delete, move,
        // copy, list, or show flag beside the new name.
        if operation_window::is_branch_creation(&normalized) {
            let boundary = names_a_boundary(&lowercased);
            return (
                Action::Write {
                    target: "git branch".to_owned(),
                },
                if boundary {
                    Sensitivity::Boundary
                } else {
                    Sensitivity::Routine
                },
                CommandObserved {
                    matched: Established::Known("git branch".to_owned()),
                    against,
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        // Before the write reader, because none of these writes anything.
        if let Some(verb) = NEUTRALISES_A_FILE.iter().find(|verb| code.contains(*verb))
            && names_a_boundary(&lowercased)
        {
            return (
                Action::Write {
                    target: verb.trim().to_owned(),
                },
                Sensitivity::Boundary,
                CommandObserved {
                    matched: Established::Known(verb.trim().to_owned()),
                    against,
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        if let Some(spelling) = shell::writes_a_file(&view) {
            // `writes_a_file` answers with the **verb** it recognised, so what
            // this branch was ever handed could not be a path;
            // `surface_of` is asked first and unconditionally so a
            // control-surface match is never shadowed by a narrower question
            // answered after it.
            let boundary = names_a_boundary(&lowercased);
            // `writes_a_file` was handed `view`, not `normalized` — the two
            // differ whenever `without_leading_options` strips a pre-subcommand
            // option, and when a data span was blanked — so this arm reports the
            // string it actually searched rather than the shared value built
            // above for the fragment-list arms.
            let observed = CommandObserved {
                matched: Established::Known(spelling.clone()),
                against: Established::Known(view.clone()),
                literal,
                boundary_source: None,
                disqualifying_byte: None,
                same_tier_fragments: Vec::new(),
            };
            return (
                Action::Write { target: spelling },
                if boundary {
                    Sensitivity::Boundary
                } else {
                    Sensitivity::Routine
                },
                observed,
            );
        }
        if let Some(command) = operation_window::remote_write_target(&code) {
            return (
                Action::Remote {
                    command: command.clone(),
                },
                Sensitivity::Routine,
                CommandObserved {
                    matched: Established::Known(command),
                    against,
                    literal,
                    boundary_source: None,
                    disqualifying_byte: None,
                    same_tier_fragments: Vec::new(),
                },
            );
        }
        // Everything else really is left alone.
        return (
            Action::Untouched,
            Sensitivity::Routine,
            CommandObserved {
                matched: Established::Unestablished {
                    unestablished: "no boundary matched this command",
                },
                against,
                literal,
                boundary_source: None,
                disqualifying_byte: None,
                same_tier_fragments: Vec::new(),
            },
        );
    }

    // Neither a shell call nor evidence of one: a write tool's payload names a
    // file, not a command line, so there is nothing here for the fragment
    // lists to read.
    let not_a_shell_command = || CommandObserved {
        matched: Established::Unestablished {
            unestablished: "this tool call is not a shell command",
        },
        against: Established::Unestablished {
            unestablished: "this tool call is not a shell command",
        },
        literal: Established::Unestablished {
            unestablished: "this tool call is not a shell command",
        },
        boundary_source: None,
        disqualifying_byte: None,
        same_tier_fragments: Vec::new(),
    };

    if WRITE_TOOLS.contains(&name.as_str()) {
        let named = path_argument(input);
        let target = named.clone().unwrap_or_else(|| tool.to_owned());
        let payload_text = input.to_string();
        if person_only::is_the_record(&target)
            || (named.is_none() && person_only::names_the_record(&payload_text))
        {
            return (
                Action::Write {
                    target: person_only::RECORD.target.to_owned(),
                },
                Sensitivity::Routine,
                not_a_shell_command(),
            );
        }
        // The control surface, first and unconditionally — before the
        // repository configuration is even asked about. A path can be on both,
        // and it must stay a whole-file `Boundary` rather than fall into the
        // per-row question next.
        //
        // Read from the whole payload when no argument names a path, because
        // two of the eleven write tools put it in a patch body rather than in a
        // field — Codex's `apply_patch` and OpenCode's `patch`.
        if is_control_surface(&target)
            || (named.is_none() && text_names_a_control_surface(&payload_text))
        {
            return (
                Action::Write { target },
                Sensitivity::Boundary,
                not_a_shell_command(),
            );
        }
        // A write to the repository's own Estigia configuration, checked
        // second: that file is not on the control surface, and every write to
        // this path is a `Boundary`, whichever tool and whichever row — the
        // class is document structure, not payload content, and no reading of
        // a payload in isolation can have a property of the document that
        // results from applying it. `docs/honesty.md` carries the account.
        if is_repository_configuration(&target)
            || (named.is_none() && text_names_the_repository_configuration(&payload_text))
        {
            return (
                Action::Write { target },
                Sensitivity::Boundary,
                not_a_shell_command(),
            );
        }
        return (
            Action::Write { target },
            Sensitivity::Routine,
            not_a_shell_command(),
        );
    }

    (
        Action::Untouched,
        Sensitivity::Routine,
        not_a_shell_command(),
    )
}

/// One of an agent's own argument names, whatever it spells it. The value
/// comes back as text whether it arrived as text or as an **argv list**.
fn argument_named(input: &serde_json::Value, names: &[&str]) -> Option<String> {
    input.as_object()?.iter().find_map(|(key, value)| {
        if !names.contains(&argument_name(key).as_str()) {
            return None;
        }
        match value {
            serde_json::Value::String(text) => Some(text.clone()),
            // An argv list is a command line with the spaces taken out.
            serde_json::Value::Array(words) => {
                let joined: Vec<&str> =
                    words.iter().filter_map(serde_json::Value::as_str).collect();
                (!joined.is_empty()).then(|| joined.join(" "))
            }
            _ => None,
        }
    })
}

fn argument_name(key: &str) -> String {
    key.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// The path a write tool names.
///
/// `pub(crate)` because `cli::narrowed_by_the_call` reads the same aliases for
/// the same reason — the target path is the write tool's evidence of where the
/// write lands, and a second interpretation of these keys would be a second
/// copy of the rule. This is the one owner.
pub(crate) fn path_argument(input: &serde_json::Value) -> Option<String> {
    argument_named(input, &["filepath", "path", "notebookpath"])
}

/// One reading shared by boundary classification and proof metadata: more than
/// one key that normalises to a command is ambiguous even when the values
/// agree, so no one of them may donate literal metadata to a boundary found in
/// another.
struct ShellCommandArgument {
    text: String,
    /// The single command string, when exactly one command key held a value
    /// usable as one. `Err` names why not, from a closed vocabulary of four
    /// shapes: no command key at all, several command keys, the one command
    /// key holding an argv array rather than a string (Codex's own shape,
    /// not "zero or several" — the false cause this field exists to end),
    /// or the one command key holding neither. Held as a `Result` rather
    /// than two `Option`s that could disagree, so a caller reporting the
    /// reason outward reads it from the `Err` directly and has no default
    /// to fall back to if the invariant ever broke.
    literal: Result<String, &'static str>,
}

fn shell_command_argument(input: &serde_json::Value) -> ShellCommandArgument {
    let values: Vec<&serde_json::Value> = input
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| {
            matches!(argument_name(key).as_str(), "command" | "commandline").then_some(value)
        })
        .collect();
    let rendered = |value: &serde_json::Value| match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(words) => {
            let words: Vec<&str> = words.iter().filter_map(serde_json::Value::as_str).collect();
            (!words.is_empty()).then(|| words.join(" "))
        }
        _ => None,
    };
    if let [value] = values.as_slice()
        && let Some(text) = rendered(value)
    {
        // The one command key rendered — so there is a `text` — but was not
        // itself a string: the argv shape, `["bash", "-lc", "..."]`.
        let literal = value
            .as_str()
            .map(str::to_owned)
            .ok_or("the one command key holds an argv array rather than a string");
        return ShellCommandArgument { text, literal };
    }
    let commands: Vec<String> = values.iter().filter_map(|value| rendered(value)).collect();
    ShellCommandArgument {
        text: if commands.is_empty() {
            input.to_string()
        } else {
            commands.join("\n")
        },
        literal: Err(match values.len() {
            0 => "the payload named zero command keys",
            // A single key present but its value rendered nothing at all —
            // not a string, and not an array with anything in it.
            1 => "the one command key holds a value that is not a command",
            _ => "the payload named several command keys",
        }),
    }
}

/// The target of one literal local fast-forward command, without shell parsing.
/// Deliberately narrower than [`shell_command_argument`]: only a string made of
/// four words, or five with `--`, can earn the exception.
fn exact_local_fast_forward_target(command: &str) -> Option<String> {
    // Newlines separate commands; spaces and tabs carry no interpretation.
    if command
        .chars()
        .any(|character| character.is_whitespace() && !matches!(character, ' ' | '\t'))
    {
        return None;
    }
    let words: Vec<&str> = command.split_ascii_whitespace().collect();
    let target = match words.as_slice() {
        ["git", "merge", "--ff-only", target] | ["git", "merge", "--ff-only", "--", target] => {
            *target
        }
        _ => return None,
    };
    if target.starts_with('-')
        || target.is_empty()
        || !target
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character))
    {
        return None;
    }
    Some(target.to_owned())
}

/// The bytes a literal integration merge may carry. Deliberately narrower than
/// [`allowed_pr_merge_byte`] and not shared with it: widening this one would
/// let back in exactly the punctuation this check exists to keep out.
fn allowed_integration_merge_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'\t' | b'-' | b'_' | b'.' | b'/')
}

/// Whether `command`, read as the literal shell payload and nothing else, is
/// an integration `git merge` and nothing riding along beside it. True only
/// when every one of these holds:
/// - no whitespace besides space and tab (a newline hides a second command
///   after this one);
/// - every byte is [`allowed_integration_merge_byte`];
/// - the first two words are exactly `git` and `merge`, checked on this raw
///   literal and never after option-stripping;
/// - every remaining word is one of the allowed flags, or at most one target
///   shaped the way [`exact_local_fast_forward_target`] requires.
fn exact_integration_merge(command: &str) -> bool {
    if command
        .chars()
        .any(|character| character.is_whitespace() && !matches!(character, ' ' | '\t'))
    {
        return false;
    }
    if !command.bytes().all(allowed_integration_merge_byte) {
        return false;
    }
    const ALLOWED_FLAGS: &[&str] = &[
        "--no-edit",
        "--no-ff",
        "--ff",
        "--quiet",
        "--continue",
        "--abort",
        "--",
    ];
    let words: Vec<&str> = command.split_ascii_whitespace().collect();
    let [first, second, rest @ ..] = words.as_slice() else {
        return false;
    };
    if *first != "git" || *second != "merge" {
        return false;
    }
    let mut targets = 0;
    for word in rest {
        if ALLOWED_FLAGS.contains(word) {
            continue;
        }
        let is_target_shaped = !word.starts_with('-')
            && !word.is_empty()
            && word
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character));
        if !is_target_shaped {
            return false;
        }
        targets += 1;
        if targets > 1 {
            return false;
        }
    }
    true
}

/// Where `command` says it runs, and whether what runs there is an exact
/// integration merge.
///
/// Reads one leading `cd <dir> &&` or one `git -C <dir>` and nothing else. A
/// line that changes directory any other way is [`RunsIn::Unestablished`]; a
/// payload that is not a literal is never an integration.
fn directed_integration_merge(command: &str, literal: bool) -> (RunsIn, bool, String) {
    if literal && exact_integration_merge(command) {
        return (RunsIn::Session, true, command.trim().to_owned());
    }
    let plain_directory = |word: &str| {
        let unquoted = ['\'', '"']
            .iter()
            .find_map(|quote| word.strip_prefix(*quote)?.strip_suffix(*quote))
            .unwrap_or(word);
        let after_drive = match unquoted.as_bytes() {
            [letter, b':', b'/', ..] if letter.is_ascii_alphabetic() => &unquoted[2..],
            _ => unquoted,
        };
        (!unquoted.is_empty()
            && !unquoted.starts_with(['-', '~'])
            && after_drive
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/._-~+@".contains(&byte)))
        .then(|| unquoted.to_owned())
    };
    let changes_directory = |text: &str| {
        text.split(|character: char| character.is_whitespace() || ";&|()".contains(character))
            .any(|word| matches!(word, "cd" | "pushd" | "-C" | "--git-dir" | "--work-tree"))
    };
    let trimmed = command.trim();
    let named = if let Some((head, rest)) = trimmed.split_once("&&") {
        match head.split_ascii_whitespace().collect::<Vec<_>>().as_slice() {
            ["cd", directory] => plain_directory(directory)
                .map(|directory| (directory, rest.trim().to_owned(), rest.trim().to_owned())),
            _ => None,
        }
    } else {
        match trimmed
            .split_ascii_whitespace()
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["git", "-C", directory, rest @ ..] => plain_directory(directory).map(|directory| {
                let rest = rest.join(" ");
                (directory, format!("git {rest}"), rest)
            }),
            _ => None,
        }
    };
    if let Some((directory, merge, rest)) = named {
        if literal && exact_integration_merge(&merge) {
            return (RunsIn::Named(directory), true, merge);
        }
        return if changes_directory(&rest) {
            (RunsIn::Unestablished, false, merge)
        } else {
            (RunsIn::Named(directory), false, merge)
        };
    }
    let trimmed = trimmed.to_owned();
    if changes_directory(&trimmed) {
        (RunsIn::Unestablished, false, trimmed)
    } else {
        (RunsIn::Session, false, trimmed)
    }
}

/// The commit a literal `git tag` creating a tag names, read from the command
/// with its directory change already taken off.
///
/// Only a command made of plain words is read: a flag outside the few that
/// create a tag, a quoted space, any shell punctuation or a third positional is
/// [`CommitTarget::Unread`], which the verdict check treats as a command it
/// cannot attribute to a commit.
fn tag_target(command: &str) -> CommitTarget {
    let command = without_leading_options(command);
    let unquote = |word: &str| {
        ['\'', '"']
            .iter()
            .find_map(|quote| word.strip_prefix(*quote)?.strip_suffix(*quote))
            .unwrap_or(word)
            .to_owned()
    };
    let plain = |word: &str| {
        !word.is_empty()
            && word
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_./:=+~^".contains(&byte))
    };
    if !command
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b" \t-_./:=+~^'\"".contains(&byte))
    {
        return CommitTarget::Unread;
    }
    let words: Vec<String> = command.split_ascii_whitespace().map(unquote).collect();
    let [first, second, rest @ ..] = words.as_slice() else {
        return CommitTarget::Unread;
    };
    if first != "git" || second != "tag" {
        return CommitTarget::Unread;
    }
    let mut positionals = Vec::new();
    let mut words = rest.iter();
    let mut options_ended = false;
    while let Some(word) = words.next() {
        if options_ended || !word.starts_with('-') {
            positionals.push(word.as_str());
        } else if word == "--" {
            options_ended = true;
        } else if let Some(long) = word.strip_prefix("--") {
            match long {
                "annotate" | "sign" | "force" | "edit" | "no-sign" => {}
                "message" | "file" | "local-user" => {
                    if words.next().is_none() {
                        return CommitTarget::Unread;
                    }
                }
                _ if ["message=", "file=", "local-user=", "cleanup="]
                    .iter()
                    .any(|prefix| long.starts_with(prefix)) => {}
                _ => return CommitTarget::Unread,
            }
        } else {
            let cluster = &word[1..];
            let takes_value = cluster.ends_with(['m', 'F', 'u']);
            let flags = cluster.trim_end_matches(['m', 'F', 'u']);
            if cluster.is_empty()
                || !flags.chars().all(|flag| "asfe".contains(flag))
                || flags.len() + usize::from(takes_value) != cluster.len()
                || (takes_value && words.next().is_none())
            {
                return CommitTarget::Unread;
            }
        }
    }
    match positionals.as_slice() {
        [name] if plain(name) => CommitTarget::Checkout,
        [name, commit] if plain(name) && plain(commit) && !commit.starts_with('-') => {
            CommitTarget::Named((*commit).to_owned())
        }
        _ => CommitTarget::Unread,
    }
}

/// The checkout `action` runs in, from the session's own directory; `None`
/// when the line moves somewhere it cannot name.
fn directory_of(action: &Action, session: &std::path::Path) -> Option<std::path::PathBuf> {
    match action {
        Action::Boundary {
            runs_in: RunsIn::Named(directory),
            ..
        } => Some(session.join(directory)),
        Action::Boundary {
            runs_in: RunsIn::Unestablished,
            ..
        } => None,
        _ => Some(session.to_path_buf()),
    }
}

/// Whether local Git proves this merge can only fast-forward toward its upstream.
///
/// Every failed process, malformed answer, detached branch, dirty path or
/// ancestry miss is `false`: this proof removes one refusal, so unknown cannot
/// be clearance. It performs no fetch and consults only refs already present in
/// the checkout.
fn is_safe_local_fast_forward(repo_dir: &std::path::Path, target: &str) -> bool {
    is_safe_local_fast_forward_in(
        repo_dir,
        target,
        std::env::vars_os().map(|(name, _)| name).collect(),
    )
}

fn is_safe_local_fast_forward_in(
    repo_dir: &std::path::Path,
    target: &str,
    environment: Vec<std::ffi::OsString>,
) -> bool {
    // The shell command runs after this process exits: steering present at
    // entry is unsafe. The guard names the variables that redirect which
    // repository Git reaches — a list, not a `GIT_` prefix, so a host that
    // injects an editor or pager does not silently withdraw the exemption.
    if !integration_environment_does_not_redirect_the_repository(environment.iter().cloned()) {
        return false;
    }
    let git = |arguments: &[&str]| {
        proof_git_command_with_environment(repo_dir, arguments, environment.iter().cloned())
            .output()
            .ok()
            .filter(|answer| answer.status.success())
    };
    let text = |arguments: &[&str]| {
        let answer = git(arguments)?;
        let value = std::str::from_utf8(&answer.stdout).ok()?.trim();
        (!value.is_empty() && !value.contains(['\r', '\n'])).then(|| value.to_owned())
    };

    let branch = text(&["symbolic-ref", "--quiet", "HEAD"]);
    if !branch.is_some_and(|branch| branch.starts_with("refs/heads/")) {
        return false;
    }
    let upstream = match text(&["rev-parse", "--symbolic-full-name", "@{upstream}"]) {
        Some(upstream) if upstream.starts_with("refs/remotes/") => upstream,
        _ => return false,
    };
    let short_upstream = match text(&[
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        "@{upstream}",
    ]) {
        Some(upstream) => upstream,
        None => return false,
    };
    let oid_length = match text(&["rev-parse", "--show-object-format"]).as_deref() {
        Some("sha1") => 40,
        Some("sha256") => 64,
        _ => return false,
    };
    let full_oid =
        target.len() == oid_length && target.bytes().all(|byte| byte.is_ascii_hexdigit());
    if target != short_upstream && target != upstream && !full_oid {
        return false;
    }
    if git(&["status", "--porcelain=v1", "--untracked-files=normal"])
        .is_none_or(|answer| !answer.stdout.is_empty())
    {
        return false;
    }
    let commit = |revision: &str| {
        let expression = format!("{revision}^{{commit}}");
        let oid = text(&["rev-parse", "--verify", "--end-of-options", &expression])?;
        ((oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then_some(oid)
    };
    let (head, target_oid, upstream) = match (commit("HEAD"), commit(target), commit(&upstream)) {
        (Some(head), Some(target), Some(upstream)) => (head, target, upstream),
        _ => return false,
    };
    if full_oid && target != target_oid {
        return false;
    }
    git(&["merge-base", "--is-ancestor", &head, &target_oid]).is_some()
        && git(&["merge-base", "--is-ancestor", &target_oid, &upstream]).is_some()
}

/// Whether `repo_dir` is covered by this run's own `worktree`, the
/// environment leaves Git free to find that checkout's own repository, and
/// `HEAD` there names a branch rather than sitting detached.
///
/// Every failed process, malformed answer, detached `HEAD` or absent worktree
/// is `false`: this proof removes one refusal, so unknown cannot be clearance.
fn is_integration_in_the_runs_own_checkout(run: &Run, repo_dir: &std::path::Path) -> bool {
    let Some(worktree) = run.worktree.as_deref() else {
        return false;
    };
    if !crate::paths::covers(worktree, repo_dir) {
        return false;
    }
    if !integration_environment_does_not_redirect_the_repository(
        std::env::vars_os().map(|(name, _)| name),
    ) {
        return false;
    }
    let branch = proof_git_command(repo_dir, &["symbolic-ref", "--quiet", "HEAD"])
        .output()
        .ok()
        .filter(|answer| answer.status.success())
        .and_then(|answer| {
            std::str::from_utf8(&answer.stdout)
                .ok()
                .map(str::trim)
                .map(str::to_owned)
        });
    branch.is_some_and(|branch| branch.starts_with("refs/heads/"))
}

/// A proof subprocess with every inherited Git control variable removed.
fn proof_git_command(repo_dir: &std::path::Path, arguments: &[&str]) -> std::process::Command {
    proof_git_command_with_environment(
        repo_dir,
        arguments,
        std::env::vars_os().map(|(name, _)| name),
    )
}

fn proof_git_command_with_environment(
    repo_dir: &std::path::Path,
    arguments: &[&str],
    environment: impl IntoIterator<Item = std::ffi::OsString>,
) -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(repo_dir).args(arguments);
    for name in environment {
        if is_git_environment(&name) {
            command.env_remove(name);
        }
    }
    command
}

fn is_git_environment(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy()
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
}

/// Whether the environment leaves Git free to discover `repo_dir`'s
/// repository on its own, rather than being pointed at a different one.
///
/// A list, not a proof of completeness: variables that redirect *where* Git
/// looks are named exactly; ones that only name a *program* Git runs
/// (`GIT_EDITOR`, `GIT_PAGER`, `GIT_ASKPASS`) are deliberately not on it,
/// because an editor cannot change which branch a merge moves. The
/// `GIT_CONFIG_KEY_`/`GIT_CONFIG_VALUE_` prefixes are the one place this check
/// reads a prefix rather than a name.
fn integration_environment_does_not_redirect_the_repository(
    environment: impl IntoIterator<Item = std::ffi::OsString>,
) -> bool {
    const REDIRECTING: &[&str] = &[
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_SHALLOW_FILE",
        "GIT_REPLACE_REF_BASE",
        "BASH_ENV",
        "ENV",
        "BASH_FUNC_git%%",
    ];
    const NUMBERED_PREFIXES: &[&str] = &["GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_"];
    !environment.into_iter().any(|name| {
        name.to_str().is_some_and(|name| {
            REDIRECTING
                .iter()
                .any(|redirecting| name.eq_ignore_ascii_case(redirecting))
                || NUMBERED_PREFIXES.iter().any(|prefix| {
                    name.get(..prefix.len())
                        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
                })
        })
    })
}

/// Everything the gate needs that it cannot work out for itself.
#[derive(Debug, Clone)]
pub struct GateContext {
    /// Where the skill — and so the transport — is installed.
    pub skill_root: PathBuf,
    /// The repository the agent is working in.
    pub repo_dir: PathBuf,
    /// Where run pointers live.
    pub state_root: PathBuf,
    /// How long a routine write may ride on the previous answer.
    pub window: Duration,
    /// The tracker the operator configured.
    pub tracker: crate::config::Tracker,
    /// Where publications and the code host live.
    pub repository_host: crate::config::RepositoryHost,
    /// Where work integrates: a reviewed branch, or trunk behind a flag.
    pub integration: crate::config::Integration,
    /// What a verdict here has to be backed by. An unreadable contract answers
    /// [`crate::config::Evidence::Reading`], the narrower one — a fault must
    /// not hand out a capability.
    pub evidence: crate::config::Evidence,
    /// The feature flag this run declared, if any.
    pub flag: Option<String>,
    /// The stand-down the operator declared, if the record is on disk.
    pub stand_down: Option<standdown::StandDown>,
    /// Commands this operator declared irreversible, beyond the built-in list.
    /// See [`classify_with`] for why it only adds.
    pub boundaries: Vec<String>,
    /// The daily range commits, pushes, and branch creation may run in.
    /// Disabled by default. Deny-only: inside the permitted period the other
    /// checks still decide.
    pub operation_window: crate::config::GitOperationWindow,
    /// What a run holding no claim may write here. Read from the repository's own file only.
    pub issue_requirement: crate::config::IssueRequirement,
    /// Who witnesses a claim here. Read from the repository's own file only; `None` when
    /// the contract could not be read, which is not a value any row holds.
    pub workflow: Option<crate::config::Workflow>,
}

/// Decides whether one action may proceed.
///
/// The gate never writes to the tracker. It reads, and it says yes or no —
/// which is what lets it sit on the critical path of every edit without being
/// able to damage anything by being wrong.
pub fn gate(context: &GateContext, run: &mut Run, action: &Action, how: Sensitivity) -> Decision {
    if let Some(denied) = person_only::deny(action) {
        return denied;
    }
    let now = session::now_seconds();
    standdown::over(
        decide(context, run, action, how),
        context.stand_down.as_ref(),
        now,
    )
}

/// [`gate`], carrying what the caller already read while resolving the call
/// this decision is about — attached to every denial this produces.
pub fn gate_observing(
    context: &GateContext,
    run: &mut Run,
    action: &Action,
    how: Sensitivity,
    observed: &Observed,
) -> Decision {
    if let Some(Decision::Deny(refusal)) = person_only::deny(action) {
        let subject = action.subject().unwrap_or_default();
        return Decision::Deny(Box::new(prefixed(*refusal, &subject, observed)));
    }
    let now = session::now_seconds();
    standdown::over(
        decide_observing(context, run, action, how, observed),
        context.stand_down.as_ref(),
        now,
    )
}

/// Whether this write lands where no checkout the claim covers can reach.
///
/// Narrow on purpose, because standing aside is a statement and an unknown is
/// not one: the target must be an **absolute** path (a shell write cannot be
/// placed, and one that cannot be placed stays gated), and a run covering
/// nothing has no claim to be outside of.
fn writes_outside_the_claim(run: &Run, action: &Action) -> bool {
    let Action::Write { target } = action else {
        return false;
    };
    let path = std::path::Path::new(target);
    if !path.is_absolute() {
        return false;
    }
    // Where it lands, not how it is spelled; a path that cannot be placed is
    // read as inside, because a wrong answer here removes the gate.
    let Some(landing) = crate::paths::placed(path) else {
        return false;
    };
    let mut covered = run.covered().peekable();
    if covered.peek().is_none() {
        return false;
    }
    // Both sides through the same resolver: an unresolvable checkout is one
    // this process cannot rule out.
    !covered.any(|checkout| {
        crate::paths::placed(checkout)
            .is_none_or(|checkout| crate::paths::covers(&checkout, &landing))
    })
}

/// What the gate decides before any stand-down is considered.
fn decide(context: &GateContext, run: &mut Run, action: &Action, how: Sensitivity) -> Decision {
    decide_observing(context, run, action, how, &Observed::default())
}

/// [`decide`], attaching `observed` to every denial through [`prefixed`].
fn decide_observing(
    context: &GateContext,
    run: &mut Run,
    action: &Action,
    how: Sensitivity,
    observed: &Observed,
) -> Decision {
    let verifier = match action {
        Action::Untouched => return Decision::Outside(Aside::NotWatched),
        Action::Remote { command } => {
            return match operation_window::check_protected(context, action) {
                Some(refusal) => Decision::Deny(Box::new(prefixed(refusal, command, observed))),
                None => Decision::Outside(Aside::NotWatched),
            };
        }
        Action::Write { .. } => "write",
        Action::Boundary { .. } => "boundary",
    };
    let subject = action.subject().unwrap_or_default();

    if let Some(fragments) = observed
        .command
        .as_ref()
        .map(|command| command.same_tier_fragments.as_slice())
        .filter(|fragments| !fragments.is_empty())
    {
        return Decision::Deny(Box::new(prefixed(
            several_irreversible_fragments(fragments),
            &subject,
            observed,
        )));
    }

    let named = match observed.command.as_ref().map(|command| &command.literal) {
        Some(Established::Known(text)) => Some(text.as_str()),
        _ => None,
    };
    if let Some(refusal) = issue_creation::refusal(named) {
        return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
    }

    // A pointer that is on disk and cannot be read says a run under this name
    // existed; it does not say what it swore. An unknown result is not
    // clearance.
    if run.unreadable {
        let what = match run.unreadable_reason.as_deref() {
            Some(reason) => format!(
                "{}: this run's record exists and cannot be read, so whether it holds an issue \
                 is unknown \u{2014} {reason}",
                run.run_id
            ),
            None => format!(
                "{}: this run's record exists and cannot be read, so whether it holds an issue \
                 is unknown",
                run.run_id
            ),
        };
        return Decision::Deny(Box::new(prefixed(
            Refusal::not_started(
                "run-pointer-unreadable",
                what,
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "what that run holds, read from the tracker \u{2014} then claimed again, or \
                     its pointer removed",
                ),
            ),
            &subject,
            observed,
        )));
    }

    let Some(issue) = run.issue.clone() else {
        // Sworn nothing. See the module docs: the oath binds once sworn, except
        // where the push guard says work here goes through a claim.
        let literal = match observed.command.as_ref().map(|command| &command.literal) {
            Some(Established::Known(text)) => Some(text.as_str()),
            _ => None,
        };
        if let Some(refusal) = unclaimed::refusal(context, &context.repo_dir, action, literal) {
            return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
        }
        return match issue_requirement::decision(context, action, how, literal) {
            Some(Decision::Deny(refusal)) => {
                Decision::Deny(Box::new(prefixed(*refusal, &subject, observed)))
            }
            Some(Decision::Ask(refusal)) => {
                Decision::Ask(Box::new(prefixed(*refusal, &subject, observed)))
            }
            Some(other) => other,
            None => Decision::Outside(Aside::NothingSworn),
        };
    };

    if run.assisted {
        return assisted::decision(context, run, action, how, observed, &subject);
    }

    // A tracker with no executable transport cannot be asked anything.
    if context.tracker.transport().is_none() {
        return Decision::Outside(Aside::NoTracker);
    }

    // A claim covers the checkout it was made in and the isolated one this run
    // was given; a write anywhere else is not covered by it.
    let path_covered = run
        .covered()
        .any(|covered| crate::paths::covers(covered, &context.repo_dir));
    let same_repository = run
        .covered()
        .any(|covered| same_git_repository(covered, &context.repo_dir));
    let reviewed_sibling = matches!(
        (
            action,
            run.review_transaction
                .as_ref()
                .filter(|receipt| receipt.is_complete())
        ),
        (
            Action::Boundary {
                command,
                pr: Some(command_pr),
                ..
            },
            Some(receipt)
        ) if command == "gh pr merge" && *command_pr == receipt.pr && same_repository
    );
    if run.covered().count() > 0 && !path_covered && !reviewed_sibling {
        // Two halves, and the comment used to name only the first. A plain write
        // from another checkout is `Outside(AnotherCheckout)`: gating it would
        // gate the operator's unrelated work under somebody else's issue. A
        // *delivery* from another checkout whose run holds a review receipt — or
        // a legacy reviewed head — reaches `stale_verdict` and is refused,
        // because the receipt is a claim about bytes this checkout did not
        // review. A non-`gh pr merge` delivery in the same repository with
        // neither is still `Outside(AnotherCheckout)`: `stale_verdict` returns
        // `None` for want of a reviewed head to compare.
        if is_delivery(action)
            && (same_repository || run.review_transaction.is_some() || run.reviewed_head.is_some())
            && let Some(refusal) = stale_verdict(
                action,
                run,
                &context.repo_dir,
                &live_pr_head_reader(context),
            )
        {
            return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
        }
        return Decision::Outside(Aside::AnotherCheckout);
    }

    // The control surface first, above the renewal window: an unreadable
    // control surface permits no write, and the window caches a claim
    // verification, not the contract's presence.
    if let Some(refusal) = contract_not_installed(context) {
        return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
    }

    // The schedule is consulted before the renewal window: a cached claim
    // verification must not carry a commit past a range that has since
    // closed, and the schedule grants nothing — inside the period the checks
    // below still run.
    if let Some(refusal) = operation_window::check_protected(context, action) {
        return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
    }

    // After the contract and the schedule, before the renewal window, and
    // `Routine` only: a `Boundary` write is watched *because* of where it
    // lands, so placing it outside must never be what waves it through. Above
    // the window so a write outside the claim is `Outside` at every age of the
    // verification, not `Allow` while it is fresh.
    if how == Sensitivity::Routine && writes_outside_the_claim(run, action) {
        return Decision::Outside(Aside::OutsideTheClaim);
    }

    if how == Sensitivity::Routine && run.within_window(context.window) {
        return Decision::Allow(format!(
            "issue #{issue} was verified inside the renewal window"
        ));
    }

    let issue_argument = issue.to_string();
    // The same missing state, defaulted the other way round on purpose:
    // guessing here **costs a refusal**; guessing in `hook::state_clause` would
    // have **announced a fact**. Do not unify them — one value, two directions.
    let state = run
        .state
        .clone()
        .unwrap_or_else(|| "in-progress".to_owned());

    // Always pin or clear: a prior call's `Some` must not stick when this one
    // names none. Production is one invocation per process;
    // `cargo test` is not. The pin covers apply-through-dispatch only — dropped
    // before `stale_verdict`, whose `live_pr_head_reader` takes it again (a
    // non-reentrant mutex would deadlock if this still held it).
    let answer = {
        let _gh_pin = gh_repo_pin();
        apply_gh_repo_locked(context.tracker.named_repo().as_deref());
        let transport = crate::transport::Context::live(
            context.skill_root.clone(),
            context.repo_dir.clone(),
            context.tracker.named_repo(),
        );
        let flags: Vec<String> = [
            "--issue",
            &issue_argument,
            "--run-id",
            &run.run_id,
            "--expect-state",
            &state,
        ]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
        // A clock that will not say is a **refusal**: the epoch is a perfectly
        // good number, and against it every claim ever written reads as live.
        let Some(seconds) = session::now_seconds() else {
            return Decision::Deny(Box::new(prefixed(
                crate::outcome::Refusal::not_started(
                    "clock-unreadable",
                    "this machine could not say what time it is, and a claim is only good until a \
                     time"
                        .to_owned(),
                    crate::outcome::Resolution::no_command(
                        crate::outcome::NoCommandReason::WorldAction,
                        "a working system clock",
                    ),
                ),
                &subject,
                observed,
            )));
        };
        crate::transport::dispatch::dispatch(
            &transport,
            "verify-claim",
            &flags,
            &session::stamp_of(seconds),
        )
        .map(|value| tracker::Answer {
            code: 0,
            body: Some(value),
        })
        .or_else(|failure| {
            Ok::<tracker::Answer, crate::outcome::Refusal>(tracker::Answer {
                code: failure.code(),
                body: Some(failure.envelope()),
            })
        })
    };

    match answer {
        Ok(answer) => match tracker::translate(&answer, verifier) {
            None => {
                // The tracker agreed the run holds this issue in `state`, so
                // from here `state` is the tracker's answer rather than the
                // run's belief — which is what makes it safe to decide on.
                run.mark_verified();
                if state == "in-progress"
                    && let Some(refusal) = near_miss_integration(
                        action,
                        run,
                        &context.repo_dir,
                        context.integration,
                        context.flag.as_deref(),
                    )
                {
                    return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
                }
                if let Some(refusal) = stale_verdict(
                    action,
                    run,
                    &context.repo_dir,
                    &live_pr_head_reader(context),
                ) {
                    return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
                }
                if let Action::Boundary {
                    command,
                    local_fast_forward_target,
                    is_exact_integration_merge,
                    ..
                } = action
                    && !(state == "in-progress"
                        && (local_fast_forward_target.as_deref().is_some_and(|target| {
                            is_safe_local_fast_forward(&context.repo_dir, target)
                        }) || (*is_exact_integration_merge
                            && directory_of(action, &context.repo_dir).is_some_and(|directory| {
                                is_integration_in_the_runs_own_checkout(run, &directory)
                            }))))
                    && let Some(refusal) = out_of_phase(
                        command,
                        &state,
                        &issue,
                        context.integration,
                        context.flag.as_deref(),
                        run,
                        &context.repo_dir,
                    )
                {
                    return Decision::Deny(Box::new(prefixed(refusal, &subject, observed)));
                }
                Decision::Allow(format!("issue #{issue} is held by {}", run.run_id))
            }
            Some(refusal) => Decision::Deny(Box::new(prefixed(refusal, &subject, observed))),
        },
        // The control surface could not be reached at all. "An unreadable
        // control surface permits no write" — this is the sentence, enforced.
        Err(refusal) => Decision::Deny(Box::new(prefixed(refusal, &subject, observed))),
    }
}

fn contract_not_installed(context: &GateContext) -> Option<Refusal> {
    let contract = context.skill_root.join(crate::skill::CONTRACT);
    (!contract.is_file()).then(|| {
        Refusal::not_started(
            "control-surface-not-installed",
            format!("{} is not installed", contract.display()),
            Resolution::run("estigia setup --all"),
        )
    })
}

/// Puts what was being attempted in front of why it was refused, and attaches
/// what the gate read on its way there — every `Deny` [`decide_observing`]
/// produces funnels through here, so this is the one place that has to.
///
/// `None`, not an empty object, when `observed` established nothing at all:
/// `decide` (unlike `decide_observing`) hands this a default, and a caller on
/// the session-less road that never learned to build one does too. Both mean
/// no reading was taken, which `Observed::is_empty` distinguishes from a
/// reading that was taken and found nothing worth naming.
fn prefixed(refusal: Refusal, subject: &str, observed: &Observed) -> Refusal {
    Refusal {
        message: format!("{subject}: {}", refusal.message),
        observed: (!observed.is_empty()).then(|| Box::new(observed.clone())),
        ..refusal
    }
}

/// Serialises every `GH_REPO` write with every `gh` spawn that follows one.
///
/// Under `cargo test` the pin is process-wide and harness paths run on many
/// threads: a set on one thread and a clear on another between pin and spawn
/// is how `the_stale_run_pointer_check_asks_gh_about_the_checkout_s_own_repository`
/// saw `GH_REPO=(absent)`. Production is one invocation per process, so this
/// mutex is uncontended there.
static GH_REPO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Hold across [`apply_gh_repo`] (or [`apply_gh_repo_locked`]) and the `gh`
/// spawn that consumes it.
pub(crate) fn gh_repo_pin() -> std::sync::MutexGuard<'static, ()> {
    GH_REPO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Pin or clear `GH_REPO` for the next `gh` spawn, taking [`gh_repo_pin`].
///
/// A previous call's pin must not stick when this one names no repository.
/// Production is one invocation per process; under `cargo test` every harness
/// path shares one process with every other test, so a conditional write that
/// only sets and never clears is a leak. Test-only: the
/// shipping call sites hold [`gh_repo_pin`] across apply and dispatch.
#[cfg(test)]
pub(crate) fn apply_gh_repo(named: Option<&str>) {
    let _pin = gh_repo_pin();
    apply_gh_repo_locked(named);
}

/// [`apply_gh_repo`] for a caller that already holds [`gh_repo_pin`] across
/// the spawn that follows.
pub(crate) fn apply_gh_repo_locked(named: Option<&str>) {
    // SAFETY: every writer of `GH_REPO` in this crate takes `gh_repo_pin` for
    // the whole of the set-and-spawn window, so this is the only thread
    // writing the variable while the pin is held. Clearing on `None` is what
    // makes the shared test process honest when two sequential calls disagree.
    match named {
        Some(named) => unsafe { std::env::set_var("GH_REPO", named) },
        None => unsafe { std::env::remove_var("GH_REPO") },
    }
}

/// Asks the tracker about one run pointer's issue, the way [`gate`] asks for a
/// renewal: the same [`crate::transport::Context::live`], the same `GH_REPO`
/// pin via [`apply_gh_repo`] before dispatching, the same `--expect-state` —
/// the run's own recorded [`Run::state`] where it has one, `in-progress` where
/// it does not, exactly as [`gate`] defaults it.
///
/// `None` when nothing was asked at all: no issue recorded, no executable
/// transport for this tracker, or a clock this machine could not read. `Some`
/// otherwise, whatever the tracker said — a call that failed and a call that
/// answered "still open" are two different things to tell an operator.
pub(crate) fn tracker_answer_for_pointer(
    skill_root: &Path,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    run: &Run,
) -> Option<tracker::Answer> {
    if run.assisted {
        return None;
    }
    let issue = run.issue_number()?;
    tracker.transport()?;
    let seconds = session::now_seconds()?;
    let _gh_pin = gh_repo_pin();
    apply_gh_repo_locked(tracker.named_repo().as_deref());
    let transport = crate::transport::Context::live(
        skill_root.to_path_buf(),
        repo_dir.to_path_buf(),
        tracker.named_repo(),
    );
    let state = run
        .state
        .clone()
        .unwrap_or_else(|| "in-progress".to_owned());
    let flags: Vec<String> = [
        "--issue",
        &issue.to_string(),
        "--run-id",
        &run.run_id,
        "--expect-state",
        &state,
    ]
    .iter()
    .map(|part| (*part).to_owned())
    .collect();
    let answer = crate::transport::dispatch::dispatch(
        &transport,
        "verify-claim",
        &flags,
        &session::stamp_of(seconds),
    )
    .map(|value| tracker::Answer {
        code: 0,
        body: Some(value),
    })
    .unwrap_or_else(|failure| tracker::Answer {
        code: failure.code(),
        body: Some(failure.envelope()),
    });
    Some(answer)
}

/// One queue this run could act on, read through the transport's own read-only
/// `list-state`, on the same [`crate::transport::Context::live`] and `GH_REPO`
/// pin as [`tracker_answer_for_pointer`].
///
/// **One shape, four queues.** `estigia goal` asks it for `review`, `ready`,
/// `blocked` and `in-progress`; a second copy of this preamble is the copy this
/// file's own history keeps finding.
///
/// `None` when nothing could be read — no transport for this tracker, no
/// readable clock, or a list that failed — never zero. A queue a fault emptied
/// is not an empty queue, and the loop these answer would stop on the fault as
/// though the work were gone.
pub(crate) fn queue_read(
    skill_root: &Path,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    run_id: &str,
    state: &str,
) -> Option<serde_json::Value> {
    tracker.transport()?;
    let seconds = session::now_seconds()?;
    let _gh_pin = gh_repo_pin();
    apply_gh_repo_locked(tracker.named_repo().as_deref());
    let transport = crate::transport::Context::live(
        skill_root.to_path_buf(),
        repo_dir.to_path_buf(),
        tracker.named_repo(),
    );
    let flags: Vec<String> = ["--state", state, "--run-id", run_id]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    crate::transport::dispatch::dispatch(
        &transport,
        "list-state",
        &flags,
        &session::stamp_of(seconds),
    )
    .ok()
}

/// How many issues this run's own delivery has closed, read from the tracker.
///
/// `None` when nothing could be read — no transport for this tracker, or a
/// count that failed — never zero, for the reason [`queue_read`] gives.
pub(crate) fn completed_work(
    skill_root: &Path,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    run_id: &str,
) -> Option<u64> {
    tracker.transport()?;
    let _gh_pin = gh_repo_pin();
    apply_gh_repo_locked(tracker.named_repo().as_deref());
    let transport = crate::transport::Context::live(
        skill_root.to_path_buf(),
        repo_dir.to_path_buf(),
        tracker.named_repo(),
    );
    crate::transport::claim::completed_count(&transport, run_id)
        .ok()
        .flatten()
}

/// What the tracker's own timeline says about one surviving holder.
///
/// Three answers, and only one of them may ever be offered `estigia release`
/// for — a pointer's own fields cannot be trusted to draw this line. Read-only,
/// like [`tracker_answer_for_pointer`] it is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HolderStanding {
    /// `issue-not-open`. The caller drops this holder before deciding
    /// anything else about it.
    ClosedIssue,
    /// `not-current-live-holder`. This one covers three different timelines
    /// this classification cannot tell apart: no acquisition for this run at
    /// all, a stale one, or a live one that lost a claim race.
    /// `estigia release --run-id <run-id>` is the only command a
    /// several-holder refusal may name for this standing.
    NotCurrentHolder,
    /// Every other answer, including a read that failed to land at all: an
    /// unknown result is not clearance, so this counts as live and no
    /// command may be offered for it.
    Live,
}

pub(crate) fn holder_standing(
    skill_root: &Path,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    run: &Run,
) -> HolderStanding {
    let answer = tracker_answer_for_pointer(skill_root, repo_dir, tracker, run);
    match answer.as_ref().map(tracker::Answer::standing) {
        Some(tracker::Standing::Stopped("issue-not-open")) => HolderStanding::ClosedIssue,
        Some(tracker::Standing::Stopped("not-current-live-holder")) => {
            HolderStanding::NotCurrentHolder
        }
        _ => HolderStanding::Live,
    }
}

/// What `estigia goal complete` read about one run's goal.
///
/// The standings are the answer's whole vocabulary: `closed` and `cleared` are
/// the only two that make `complete` true, and everything else is a reason not
/// to continue. `unreadable`, `unread` and `nothing-sworn` never continue.
#[derive(Debug, Clone)]
pub(crate) struct GoalCompletion {
    /// The issue the pointer holds, when it holds one, as its tracker spells it.
    pub(crate) issue: Option<crate::identity::Identity>,
    /// `closed`, `cleared`, `open`, `lane-red`, `lane-running`, `unpublished`,
    /// `nothing-sworn`, `unread` or `unreadable`.
    pub(crate) standing: &'static str,
    /// Whether delivery evidence proves the goal complete.
    pub(crate) complete: bool,
    /// The sentence behind the answer, when there is one — what cleared it, or
    /// why it did not.
    pub(crate) reason: Option<String>,
    /// The head the computed verdict bound to, when one cleared it.
    pub(crate) head: Option<String>,
}

/// The delivery evidence behind `estigia goal complete`, read-only.
///
/// A closed issue is complete on the tracker's own word; an open one is complete
/// only when this run's latest publication is cleared by its computed verdict
/// and a lane that is not red. Nothing here re-reads the work or the merge —
/// `claim::goal_completion` reads the receipt and the lane at call time, and
/// `docs/honesty.md` says what that leaves unproven.
pub(crate) fn goal_completion_reading(
    skill_root: &Path,
    repo_dir: &Path,
    tracker: &crate::config::Tracker,
    run: &Run,
) -> GoalCompletion {
    let unreadable = |issue: Option<crate::identity::Identity>| GoalCompletion {
        issue,
        standing: "unreadable",
        complete: false,
        reason: None,
        head: None,
    };
    if run.unreadable {
        return unreadable(None);
    }
    let Some(issue) = run.issue.clone() else {
        return GoalCompletion {
            issue: None,
            standing: "nothing-sworn",
            complete: false,
            reason: None,
            head: None,
        };
    };
    match tracker_answer_for_pointer(skill_root, repo_dir, tracker, run) {
        // No transport, no readable clock: nothing was asked, which is not an
        // answer. Never `complete`.
        None => unreadable(Some(issue)),
        Some(answer) => match answer.standing() {
            tracker::Standing::Stopped("issue-not-open") => GoalCompletion {
                issue: Some(issue),
                standing: "closed",
                complete: true,
                reason: Some("the tracker says the issue is closed".to_owned()),
                head: None,
            },
            tracker::Standing::Unread => GoalCompletion {
                issue: Some(issue),
                standing: "unread",
                complete: false,
                reason: None,
                head: None,
            },
            // Open, or not the current holder: the issue is not closed, so the
            // only remaining evidence is this run's own publication.
            _ => {
                let Some(receipt) = run.review_transaction.as_ref() else {
                    return GoalCompletion {
                        issue: Some(issue),
                        standing: "unpublished",
                        complete: false,
                        reason: Some("this run has not published a review candidate".to_owned()),
                        head: None,
                    };
                };
                let context = crate::transport::Context::live(
                    skill_root.to_path_buf(),
                    repo_dir.to_path_buf(),
                    tracker.named_repo(),
                );
                let Some(number) = issue.number() else {
                    return unreadable(Some(issue));
                };
                let Some(seconds) = session::now_seconds() else {
                    return unreadable(Some(issue));
                };
                match crate::transport::claim::goal_completion(
                    &context,
                    number,
                    receipt,
                    None,
                    &session::stamp_of(seconds),
                ) {
                    Err(_) => unreadable(Some(issue)),
                    Ok(value) => {
                        let standing = value
                            .get("standing")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|word| match word {
                                "cleared" => Some("cleared"),
                                "open" => Some("open"),
                                "lane-red" => Some("lane-red"),
                                "lane-running" => Some("lane-running"),
                                _ => None,
                            });
                        let complete = value
                            .get("complete")
                            .and_then(serde_json::Value::as_bool)
                            .filter(|complete| !*complete || standing == Some("cleared"));
                        let (Some(standing), Some(complete)) = (standing, complete) else {
                            return unreadable(Some(issue));
                        };
                        GoalCompletion {
                            issue: Some(issue),
                            standing,
                            complete,
                            reason: value
                                .get("reason")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                            head: value
                                .get("head")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                        }
                    }
                }
            }
        },
    }
}

/// Where the transport lives, or where it would live: [`discover_skill_root`]
/// answers "which installed skill can be asked", and this gives the rule its
/// chance even when none can be found.
pub fn control_surface() -> PathBuf {
    discover_skill_root().unwrap_or_else(|_| {
        let options = crate::setup::SetupOptions::default();
        crate::setup::every_adapter()
            .find_map(|adapter| crate::setup::resolve_paths(adapter, &options).ok())
            .map(|paths| paths.skill_root)
            // Nowhere to point at, so anywhere without a transport will do: the
            // only thing asked of this path is that asking it fails.
            .unwrap_or_else(|| PathBuf::from("estigia-has-no-installed-skill"))
    })
}

/// The one installed skill root the gate decides in, for a harness that was
/// given no explicit one.
pub fn discover_skill_root() -> Result<PathBuf, Refusal> {
    discover_skill_root_in(&crate::setup::SetupOptions::default())
}

/// [`discover_skill_root`], for a caller that resolves its own environment.
pub fn discover_skill_root_in(options: &crate::setup::SetupOptions) -> Result<PathBuf, Refusal> {
    let roots: Vec<PathBuf> = crate::setup::every_adapter()
        .filter_map(|adapter| crate::setup::resolve_paths(adapter, options).ok())
        .map(|paths| paths.skill_root)
        .filter(|root| root.join(crate::skill::CONTRACT).is_file())
        .collect();
    // The **contract** is what makes a directory a skill root. Estigia's own
    // first, because Estigia installs its skill under upstream's name on
    // purpose, and an operator who already runs `issue-flow` has a root that
    // passes this test and holds no configuration Estigia ever wrote.
    //
    // `min_by_key` keeps the first of equal ranks, so where nothing
    // distinguishes two roots the `AGENTS` order above still decides.
    roots
        .iter()
        .min_by_key(|root| claim_to_decide_in(root))
        .cloned()
        .ok_or_else(|| {
            Refusal::not_started(
                "harness-not-installed",
                "no installed skill was found",
                Resolution::run("estigia setup --all"),
            )
        })
}

/// How strong a claim one root makes to be the place the gate decides in.
/// Ordered, lowest first, so [`discover_skill_root_in`] can sort by it. The
/// first half is whether the contract carries a configuration block; the
/// second — the whole fix — is presence of the operator's own `estigia.local.md`,
/// not "differs from the defaults". **A file that will not parse still counts.**
fn claim_to_decide_in(root: &Path) -> (u8, u8) {
    let carries_a_block = std::fs::read_to_string(root.join(crate::skill::CONTRACT))
        .is_ok_and(|text| text.contains(crate::config::BLOCK_BEGIN));
    let carries_the_operators_own = root.join(crate::config::LOCAL_FILE).is_file();
    // The block first, so a root with no configuration block is never preferred
    // over one that has it.
    (
        u8::from(!carries_a_block),
        u8::from(!carries_the_operators_own),
    )
}

/// A refusal for a caller that named an issue Estigia cannot parse.
pub fn issue_not_a_number(value: &str) -> Refusal {
    Refusal::not_started(
        "issue-not-a-number",
        format!("{value:?} is not an issue number"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the issue number this run holds, as a positive integer",
        ),
    )
}

#[cfg(test)]
mod tests;
