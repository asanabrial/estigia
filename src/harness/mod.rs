//! The harness: Estigia holding the tools rather than asking politely.
//!
//! Everything under `skill/` is text an agent may read and may ignore. That is
//! the gap the honesty contract already named — *the seam tests prove the
//! contract does not link to a file that is missing; they do not prove an agent
//! obeys it.* This module closes it for the one rule worth closing:
//!
//! > Run `verify_claim` before the first repository write, every heartbeat, and
//! > every expensive or irreversible boundary; an unreadable control surface
//! > permits no write.
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

use std::path::PathBuf;
// `Path` is used by this module's tests, which are a child module and see what
// it imports. Kept named rather than re-imported there: one import for one type.
#[cfg(test)]
use std::path::Path;
use std::time::Duration;

use crate::outcome::{NoCommandReason, Refusal, Resolution};

pub mod doctor;
pub mod guard;
pub mod hook;
pub mod mcp;
pub mod roles;
pub mod session;
pub mod shell;
pub mod standdown;
pub mod tracker;

pub use session::Run;

/// How long a routine write may ride on the previous answer.
///
/// Not a cache of authority — a cadence. `SKILL.md` already asks for renewal
/// "every heartbeat" rather than every keystroke, so a window is the shape the
/// contract specifies; what the window must never cover is an irreversible
/// boundary, and [`Sensitivity::Boundary`] never consults it.
pub const RENEWAL_WINDOW: Duration = Duration::from_secs(120);

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
    },
    /// Nothing Estigia claims authority over.
    Untouched,
}

impl Action {
    /// What this action is *about*: the file, or the command.
    ///
    /// A refusal is prefixed with it — `git push: the review was published
    /// against …` — and an **allow** used to drop it. Measured: four
    /// irreversible steps under one claim, `git tag v1.0`, `gh release create
    /// v1.0` and `git push --force origin main`, left three ledger lines
    /// reading `tool=Bash verdict=allow detail=issue #12 is held by <run>` — the
    /// same sentence three times, for a tag, a release and a force-push to
    /// trunk. The record identified what Estigia **stopped** and not what it
    /// **let through**, and the ones it let through are the ones that changed
    /// the world.
    ///
    /// The pre-push door already recorded it, passing `git push` where an agent
    /// tool call passes `Bash`, so the same boundary was written down two ways
    /// depending on which door it came in by.
    ///
    /// A method rather than a third `match` on the same three variants: `decide`
    /// had one to build the prefix and the record needed the same answer.
    pub fn subject(&self) -> Option<&str> {
        match self {
            Self::Write { target } => Some(target),
            Self::Boundary { command } => Some(command),
            Self::Untouched => None,
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
///
/// Five situations, and they want five different things done about them. Held
/// as a type for the reason [`crate::harness::doctor::Health::Broken`] holds
/// its resolution as one: *a string forces the caller to invent a generic one
/// instead*.
///
/// It was a string here too, invented at the one place that printed it:
/// `estigia gate` answered `"{tool} is not something this run's oath covers"`
/// for all four. For a `git push` from a run holding no issue that is false
/// twice over — `Bash` is covered, `git push` is a boundary, and what is
/// missing is the oath. It is the sentence OpenCode's plugin gets back, because
/// that adapter gates by shelling out to `estigia gate`; and it is the answer
/// given by the one command whose stated purpose is *working out why a write
/// was refused*.
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
    ///
    /// Hand-written, and **not** held by the compiler — an earlier version of
    /// this comment said it was, on the strength of a `#[cfg(test)]` predicate
    /// that a normal build never compiles and that a new variant could satisfy
    /// without ever joining this list. What holds it is
    /// `every_reason_for_standing_aside_has_a_stable_name`, which reads the arms
    /// of `code` out of this file's own source and refuses to walk fewer than it
    /// finds. That is the technique the code inventory in `cli::tests` uses, and
    /// it is what caught the fifth reason arriving with no entry anywhere.
    pub const ALL: &'static [Self] = &[
        Self::NotWatched,
        Self::NothingSworn,
        Self::NoTracker,
        Self::AnotherCheckout,
        Self::OutsideTheClaim,
    ];

    /// The stable name a program matches on.
    ///
    /// `estigia gate --json` printed this field as `format!("{aside:?}")` — the
    /// Rust identifier, `NothingSworn`, in a document whose every other value is
    /// lower case and whose refusal codes are kebab-case by rule. Two things
    /// were wrong with that. A `Debug` rendering is not an interface: renaming
    /// the variant, or giving it a field, changes what a caller reads with
    /// nothing saying so. And a vocabulary that speaks Rust in one field and
    /// English in the rest is one nobody can match on with confidence.
    ///
    /// Spelled the way [`crate::outcome::Refusal::code`] is spelled, and for the
    /// reason written there: *a code is matched on, and never reworded to
    /// improve the prose*.
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
    /// Not Estigia's call, and which of the five reasons it was.
    Outside(Aside),
}

impl Decision {
    /// Whether this decision stops the tool call.
    pub fn denies(&self) -> bool {
        matches!(self, Self::Deny(_))
    }
}

/// Tool names whose whole purpose is to write to the working tree.
///
/// guard:population write-tools too-tight: the tools an agent uses to edit files
/// in the repository under review. Legitimate population: every built-in editing
/// tool of every agent Estigia gates. Boundary: a tool not on this list writes
/// without being gated, and each agent names its own.
///
/// The list was Claude Code's alone for four rounds, while gates were installed
/// for five more agents. Every one of those gates fired, reached this function,
/// found a name it did not know, and stood aside — registered, running, and
/// deciding nothing. Comparing it against the matchers those gates register is
/// now `every_matcher_names_tools_the_gate_can_classify`.
///
/// Declared **too-tight** rather than fail-closed on purpose. The alternative —
/// gate every tool and allow-list the readers — denies `Read`, `Grep` and every
/// MCP tool the operator installed, which is a harness nobody keeps switched on.
/// The proof boundary: this shows the listed tools are gated. It does not show
/// the list is complete, and a tool a future agent adds is a gap until it is
/// added here.
///
/// Matched case-insensitively: OpenCode sends `edit`, Claude Code sends `Edit`,
/// and the same tool under two spellings is one tool.
pub(crate) const WRITE_TOOLS: &[&str] = &[
    // Claude Code.
    "edit",
    "write",
    "multiedit",
    "notebookedit",
    "update",
    // Codex: `apply_patch` is how it writes, and it carries these aliases.
    "apply_patch",
    // Gemini CLI and Qwen Code.
    "write_file",
    "replace",
    // OpenCode.
    "patch",
    // Windsurf, which sends the *event* name rather than a tool name. Same
    // fact, different word.
    "pre_write_code",
];

/// Tool names that hand a command to a shell.
///
/// guard:population shell-tools too-tight: the tools through which an agent runs
/// a command line. Legitimate population: every built-in shell tool of every
/// agent Estigia gates. Boundary: a shell tool not named here has its command
/// line unread, so an irreversible step inside it is invisible.
///
/// They are separated from the writing tools because their *argument* decides
/// what they are: `git status` through one of these is a read, and `git push`
/// through the same tool is a boundary.
pub(crate) const SHELL_TOOLS: &[&str] = &["bash", "run_shell_command", "shell", "pre_run_command"];

/// Paths whose contents are this harness's own authority.
///
/// guard:population control-surface too-tight: the files Estigia's decisions
/// are read from — its state directory, the contract, the entries it writes
/// into an agent's own settings, and the push hook — **and the files that carry
/// its authority to an agent**: the instruction file each adapter's `setup`
/// writes the workflow-authority directive into. Legitimate population: any path
/// a write to which changes what this harness enforces, or what an agent is told
/// this harness may enforce.
///
/// That second clause is a widening, recorded rather than slipped in. Issue 26
/// added the instruction files, and a reviewer was right that they do not fit the
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
///
/// What one tracker read costs is worth stating where the trade is made, because
/// this sentence used to be the whole account of it and a reviewer measured the
/// rest. A `Boundary` never rides the renewal window and never stands aside
/// outside the claim, so it is a live `gh issue view` on **every** write — about
/// 0.5 to 1.2 seconds, no caching — and with no network it is a refusal rather
/// than a delay. The population grew in issue 26 to cover a project's own
/// `.claude/agents/` and rules directories, so that cost now lands on ordinary
/// per-project files and not only on an operator's home. `docs/honesty.md`
/// carries the measurements; this is here so the trade is not read as free.
///
/// `gh`'s hosts file joined the population on 2026-08-15, in both spellings and
/// the file rather than the directory around it. It is not Estigia's
/// file and no Estigia decision is read from it, so it sits at the edge of the
/// legitimate population as stated — but it decides **which account** every `gh`
/// call the transport makes acts as, which is what this list is for once a write
/// outside the repository can stand aside without the tracker being asked. Issue
/// 2 asked for exactly this: name the path that can reach tracker state rather
/// than relax the class around it.
///
/// That boundary sentence is about `is_control_surface`, and this declaration
/// sits on the **list**, so the fingerprint covers the rule and the population
/// and not the code that matches against them. Changing the matcher does not
/// reopen the claim — measured by changing it and watching the registry stay
/// green. `tests/guards.rs` carries that in its own *what this does not prove*.
///
/// # What this closes, and what it does not
///
/// `disarming-shell` gates the *subcommands* that switch the gate off. It left
/// the shorter road open: writing `~/.estigia/stand-down.json` stands the gate
/// down without a subcommand, writing a run pointer grants a claim nobody made,
/// and removing the entry from `~/.claude/settings.json` ends the gate
/// altogether. Every one of those was `Routine` — the same answer as
/// `src/main.rs`.
///
/// Both roads to them are measured now, and for a while only one was. This was
/// wired into the `Write` tool alone; a shell line reached the same files and
/// was answered `Routine`, because the shell branch had only the **verb** it
/// recognised to hand over and never a path. `rm ~/.claude/settings.json` is
/// the cheapest disarmament there is, and it was the unmeasured one.
///
/// It is a **boundary**, not a denial, for the reason every list here is: a run
/// holding no claim is `Outside` and unaffected, and a run holding one must
/// renew it here. An operator editing their own contract is not going through
/// this gate at all; an agent doing it mid-oath now says so on the record.
const CONTROL_SURFACE: &[&str] = &[
    // `gh`'s hosts file, which is not Estigia's and decides the identity every
    // tracker call acts as. Named here because issue 2 asked for exactly that:
    // *"whether any write outside the repository can still affect tracker
    // state. If one can, it is that path that needs naming rather than the
    // whole class."* This one can — rewriting it changes which account answers
    // every `gh` call the transport makes — and standing aside outside the
    // repository is what would otherwise have let it through ungated. The wider
    // gap it pointed at — the instruction file each adapter's `setup` writes —
    // was closed by issue 26, and those are derived below rather than listed
    // here.
    ".config/gh/hosts.yml",
    // No space, deliberately. `surface_of` splits a command on whitespace and
    // appends `/` to every token, so a fragment containing one can never match
    // through the shell — measured: `Write` on the Windows spelling answered
    // `Boundary` while `rm` on the same path answered `Routine`, which is the
    // road an agent would actually take. `cli/hosts.yml` is a substring of
    // `github cli/hosts.yml` on both roads.
    "cli/hosts.yml",
    // Estigia's own state: run pointers, the stand-down record, the ledger.
    ".estigia/",
    // The tree the **previous** name installed into. The tree this build writes
    // is derived from `skill::DIRECTORY` in `is_control_surface` rather than
    // named here, so a rename cannot leave it unmeasured; this entry is the
    // frozen older name, and it stays because an operator who ran `issue-flow`
    // still has that contract on disk until an upgrade moves it. Until then it
    // is a file an agent reads, and therefore one a run must not rewrite
    // unmeasured.
    "skills/issue-flow/",
    // The one boundary no agent can go around, when it is not going around it.
    "hooks/pre-push",
    // Where each adapter registers the gate, and where it registers the tools.
    //
    // Hand-written, and wrong in five of eleven places when it was: the
    // guessed spellings were `.codex/hooks.json` without its `config.toml`,
    // `.opencode/plugin` for a plugin that lives under `.config/opencode/`,
    // `.crush/crush.json` for a file under `.config/crush/`, and
    // `.continue/hooks.json` for one called `settings.json`. Crossed against
    // what `resolve_paths` actually produces, per adapter, in
    // `every_control_file_an_adapter_has_is_one_the_gate_measures` — which is
    // the half that catches the next one.
    // Without the extension, deliberately. `contains` is the match, so
    // `.claude/settings.json` does not reach `.claude/settings.local.json` —
    // which is the file an operator is *told* to put machine-local overrides in,
    // and which Claude Code reads with the same authority. It answered `Routine`
    // and the gate could be switched off through it. No trailing slash, because
    // this names a file and not a directory — so it is a prefix, and it reaches
    // `.claude/settingsmap.ts` too. Measured, and kept: one tracker read on a file
    // nobody has, against the gate switching off through one everybody is told to
    // edit.
    ".claude/settings",
    // The agent definitions, which are instructions with a tool allowlist. An
    // agent that writes one is choosing what a delegated context may do, which
    // is the same authority as the directive itself, and `harness::roles`
    // enforces exactly that allowlist.
    //
    // With a trailing slash, like every directory here. It lost one for a single
    // head, because the slash split the two roads: `surface_of` appends a
    // separator, so `rm ~/.claude/agents` was `Boundary` while a write to the bare
    // directory was `Routine`. Removing it closed that and opened a prefix bleed
    // instead — `.claude/agentsmith.md`. The matcher honours the slash now, so the
    // entry can say *directory* and mean it. The crossing found the split the
    // moment that root was added to what it walks, which is the better argument for
    // crossing a hand-spelled entry than any reasoning about reachability.
    ".claude/agents/",
    // The rules **directories**, not only Estigia's own filename inside them.
    // `paths_in`'s comments say so for two of these — Continue applies any rule
    // that is not `invokable` and declares no globs, Cline loads the directory
    // for every task. The other two are here on the restated rule rather than on
    // anything this crate verified: Windsurf's comment is about a character cap
    // on one file, and `.cursor/rules` holds no Estigia file at all — Cursor's
    // directive is `~/.cursor/estigia-workflow-authority.md`. `docs/honesty.md`
    // carries which is which. So gating `estigia.md` and leaving the directory open is defeated by
    // a neighbour: a reviewer measured `~/.cline/rules/zz-override.md` answering
    // `Routine`, and a sibling saying *Estigia is retired* changes exactly what
    // the restated population clause is about — what an agent is told this
    // harness may enforce — without touching a `Boundary` path.
    //
    // With trailing slashes, which `names` reads as *directory* — see the entry
    // above for the round these spent without them.
    ".cline/rules/",
    ".continue/rules/",
    "windsurf/memories/",
    ".cursor/rules/",
    ".claude.json",
    ".codex/hooks.json",
    ".codex/config.toml",
    // Anchored to what the installer writes, not to the directory around it.
    // `XDG_CONFIG_HOME` moves the `.config/` prefix, so anchoring there left
    // opencode's plugin — that adapter's only deny mechanism — and its MCP config
    // answering `Routine` when the variable was set. But dropping the prefix
    // entirely left `opencode` as a bare directory name matched anywhere: a
    // reviewer measured `node_modules/opencode/**`, `packages/opencode/**` and a
    // checkout *named* `opencode` answering `Boundary` on every file in them, at
    // about 1.2s of tracker read per write, unwindowed, and refused outright with
    // no network. Both ends were wrong; the tail the installer actually writes is
    // neither. `opencode/agents` covers the instruction file and the definition
    // root that `harness::roles` enforces from.
    ".config/opencode/",
    "opencode/agents/",
    "opencode/plugins/",
    "opencode/opencode.json",
    // Both roots, because Gemini keeps its settings under `%APPDATA%` on Windows
    // and `~/.gemini` everywhere else, and only the POSIX spelling was here. The
    // Windows one is where this harness's own gate is registered for that
    // adapter, it is outside every checkout by construction, and it answered
    // `Routine` — so the stand-aside issue 2 added waved it past without asking
    // the tracker. Found the moment the crossing below stopped walking one
    // platform.
    ".gemini/settings.json",
    "gemini/settings.json",
    ".qwen/settings.json",
    ".cursor/hooks.json",
    ".cursor/mcp.json",
    ".continue/settings.json",
    "crush/crush.json",
    ".codeium/windsurf/hooks.json",
    ".cline/hooks",
];

/// Commands that make a file useless without writing a byte of it.
///
/// A `pre-push` hook without its execute bit is not a hook: git skips it,
/// silently and with no warning. Nothing about the file's contents changed, so
/// `writes_a_file` is right not to call it a write — and the gate saw nothing
/// at all, which is how `chmod -x .git/hooks/pre-push` ends the push boundary
/// while leaving every report saying it is installed.
///
/// Only ever a boundary **together with** a control surface. `chmod +x
/// build.sh` is nobody's business, and a harness that gates it is one nobody
/// keeps on.
///
/// guard:population neutralising-shell too-tight: the commands that take a
/// file's power away without changing a byte of it — its mode, its attributes,
/// its owner. Legitimate population: every way to make a control surface inert
/// while leaving it in place and reporting as installed. Boundary: these three
/// verbs, over a path the control-surface list already knows.
///
/// **POSIX spellings only**, which is the boundary worth stating plainly
/// because `shell::WRITES_A_FILE` next door deliberately carries the Windows
/// ones and says why — this crate's own platform is the one where `cmd /c` is
/// ordinary. The asymmetry is not an oversight: the execute bit is what these
/// take away, and Git for Windows never reads it, so `chmod -x` has no Windows
/// equivalent that stops a hook. What would is denying read access with
/// `icacls`, and that is unmeasured: named here rather than added, because a
/// spelling nobody has watched neutralise a hook is an unchecked claim, and
/// this list is only ever consulted **together with** a control surface.
const NEUTRALISES_A_FILE: &[&str] = &["chmod ", "chattr ", "chown "];

/// How sensitive a command line is, by what it names.
///
/// A whole command rather than one operand, on the asymmetry this module
/// already runs on: a line that only *mentions* a control surface while writing
/// elsewhere costs one extra tracker read, and a line that removes one
/// unmeasured costs the gate for everything after it.
///
/// Each token is given a trailing `/` before matching, because the fragments
/// name directories with one and a command names them without: `rm -rf
/// ~/.claude/skills/issue-flow` is the same directory as the path a `Write`
/// would give as `.../skills/issue-flow/`.
fn surface_of(command: &str) -> Sensitivity {
    let view = command
        .split_whitespace()
        .map(|token| format!("{token}/"))
        .collect::<Vec<_>>()
        .join(" ");
    if is_control_surface(&view) {
        Sensitivity::Boundary
    } else {
        Sensitivity::Routine
    }
}

/// Whether this path is one whose contents decide what the harness enforces.
///
/// Read with both separators folded to `/`, because the tool sends whatever the
/// platform hands it and half of these are written by this crate on Windows.
fn is_control_surface(target: &str) -> bool {
    let mut path = target.replace('\\', "/").to_ascii_lowercase();
    // A separator that names nothing does not make this another file.
    // `~/.claude/./settings.json` is `~/.claude/settings.json`, and it read as
    // `Routine` — which means the harness could be disarmed on an answer from
    // inside the renewal window rather than on a fresh one. A matcher a
    // redundant separator defeats is measuring a spelling, not a path.
    while path.contains("/./") || path.contains("//") {
        path = path.replace("/./", "/").replace("//", "/");
    }
    // And a parent segment, which is the same class and far more ordinary than
    // either of those: `~/.claude/skills/../settings.json` names the file the
    // gate is registered in, and read as `Routine`. The declared boundary says
    // what this cannot reach — a hard link, a junction, a copy of the state
    // directory somewhere else — and a `..` is none of them. It is the same
    // path, spelled the way a shell writes it.
    //
    // One segment at a time, and never past the start: collapsing `a/../..`
    // into nothing would let a path climb out of what it names. Over-collapsing
    // costs a false positive, which is one tracker read before a write that was
    // going to be verified anyway; under-collapsing costs the boundary.
    while let Some(at) = path.find("/../") {
        let before = &path[..at];
        let Some(cut) = before.rfind('/') else { break };
        if &before[cut + 1..] == ".." {
            break;
        }
        path = format!("{}{}", &path[..cut], &path[at + 3..]);
    }
    // The skill tree is derived, not spelled. It used to be a literal in
    // `CONTROL_SURFACE`, and that put the installed directory's name in two
    // places: `skill::DIRECTORY`, which decides where the installer writes, and
    // a fragment here, which decides whether writing there is measured. They
    // agreed only because nobody had renamed one. Renaming `issue-flow` to
    // `flow` and leaving this alone made the contract writable on a `Routine`
    // answer — the cheapest disarmament there is, through the tool an agent uses
    // most, and the suite stayed green. One place now holds the name.
    let installed = format!("skills/{}/", crate::skill::DIRECTORY);
    // The instruction files, derived from the adapter table for the same reason
    // the skill tree is derived from `skill::DIRECTORY`: a hand-spelled copy
    // agrees with the installer only until somebody renames one. Each adapter
    // answers the tail of the file `setup` writes its directive into — the
    // sentence telling that agent this harness holds the authority — and
    // `every_control_file_an_adapter_has_is_one_the_gate_measures` resolves the
    // real path for every adapter and asks this function, so a fragment that
    // stops matching fails a test instead of quietly leaving a file open.
    //
    // Ten of the eleven were `Routine` until now — OpenCode's was already covered
    // by the `.config/opencode/` entry that stood here then — and issue 2 is what
    // made that matter: a
    // write outside every checkout the claim covers stands aside without asking
    // the tracker, and every one of these is outside every checkout by
    // construction. So they went from *measured against the claim* to *not gated
    // at all*.
    // A fragment ending in `/` names a **directory**, and a directory is matched
    // by what is under it or by itself — never by a name that merely starts the
    // same way.
    //
    // Both halves were wrong in turn, one round apart. With the slash, a write to
    // the bare directory answered `Routine` while `rm <dir>` answered `Boundary`,
    // because `surface_of` appends a separator: one road gated and the other not,
    // on the state directory among others. Dropping the slash closed that and
    // opened a prefix bleed instead — measured, `.estigiaignore`, `skills/flow.md`
    // and `.claude/agentsmith.md` all became `Boundary`, which is a tracker read
    // on ordinary files for nothing. Neither trade was necessary: the slash says
    // what the entry means and the matcher can honour it.
    // A fragment is anchored on the left when its first segment is a whole one in
    // every path it has to reach. Two kinds qualify. A dot-directory always is, so
    // `my.claude/agents` is not `.claude/agents`. A fragment naming a **directory**
    // always is too — every real target of `opencode/agents/`, `windsurf/memories/`,
    // `opencode/plugins/` and `skills/issue-flow/` has that first segment whole —
    // and anchoring it is what stops the two roads disagreeing: `surface_of` gives
    // every token a trailing `/`, so a bare `/repo/.opencode/agents` reached the
    // unanchored branch through the shell and not through `Write`, answering
    // `Boundary` one way and `Routine` the other. The right side was anchored from
    // the start; leaving the left unanchored kept the split alive on exactly the
    // fragments this entry added.
    //
    // A file fragment without a leading dot cannot be anchored: `cli/hosts.yml`
    // exists to match **mid-segment**, because the Windows spelling is
    // `GitHub CLI/hosts.yml` and `github cli` is one segment holding a space. That
    // is the whole of what stays unanchored, and `docs/honesty.md` measures it.
    let anchored = |needle: &str| {
        if needle.starts_with('.') || needle.ends_with('/') {
            path.starts_with(needle) || path.contains(&format!("/{needle}"))
        } else {
            path.contains(needle)
        }
    };
    let names = |fragment: &str| {
        fragment.strip_suffix('/').map_or_else(
            || anchored(fragment),
            |bare| {
                // Both sides. The right by the trailing separator the fragment
                // carries; the left because a reviewer measured `~/my.claude/agents`
                // and `/repo/my.claude/agents/note.md` answering `Boundary` — the
                // first through `ends_with`, which anchors nothing in front of it,
                // and the second through `contains`, which the first attempt at
                // this left alone while three documents said it was closed.
                anchored(fragment) || path == bare || path.ends_with(&format!("/{bare}"))
            },
        )
    };

    names(&installed)
        || crate::setup::AGENTS
            .iter()
            .any(|adapter| names(adapter.instruction_fragment()))
        || CONTROL_SURFACE.iter().any(|fragment| names(fragment))
}

/// The command line with every spelling of this binary reduced to `estigia`.
///
/// `estigia uninstall`, `/usr/local/bin/estigia uninstall` and
/// `C:\Users\me\.cargo\bin\estigia.exe uninstall` are one command, and the
/// third is the one Estigia writes into every hooks file it installs. Matching
/// the bare word alone let the spelling this crate authors itself go past.
fn as_estigia(command: &str) -> String {
    command
        .split_whitespace()
        .map(|word| {
            let last = word
                .trim_matches(['"', '\''])
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(word);
            let stem = last.strip_suffix(".exe").unwrap_or(last);
            if stem == "estigia" { "estigia" } else { word }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Options a command carries **before** its subcommand, which take a value of
/// their own.
///
/// Only these need the following word dropped with them. Everything else that
/// starts with `-` is dropped on its own, either because it takes nothing
/// (`--no-pager`) or because it carries its value with an `=`
/// (`--git-dir=.git`).
/// Spelled lowercase, because the line is lowercased before it gets here —
/// which is why git's `-c` and `-C` are one entry rather than two.
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

/// The command line with a subcommand's own options moved out of the way.
///
/// Every fragment in the lists below names `git <subcommand>` or
/// `gh <subcommand>`, and both take their own options *before* that word. One
/// `-c` between the two therefore defeated **all** of them at once, and it is
/// not a corner: `git -c core.hooksPath=/dev/null push origin HEAD` is a push
/// with every hook in the repository turned off, and it classified `Untouched`
/// — not a boundary, not a write, nothing. `git -c user.name=x merge main` went
/// the same way, and merging is a delivery.
///
/// Measured across the lists: `push`, `commit`, `merge`, `tag`, `reset --hard`,
/// `clean`, `rm` and `worktree add` all went past behind `-c`, `-C`,
/// `--git-dir` or `--no-pager`.
///
/// The same shape as [`as_estigia`] one function up, and for the same reason:
/// two spellings of one act, and only one of them seen.
fn without_leading_options(command: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut words = command.split_whitespace().peekable();
    while let Some(word) = words.next() {
        kept.push(word);
        if word != "git" && word != "gh" {
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

/// Shell fragments that turn Estigia's own enforcement off, or loosen it.
///
/// guard:population disarming-shell too-tight: the commands that stand the gate
/// down, remove it, or widen what it permits. Estigia's own subcommands, and
/// **the ones that disarm it without naming it** — `core.hooksPath` points git
/// away from every hook in this repository, so the push guard stops running
/// with nothing on the line that says so. Legitimate population: any shell
/// command that reduces what this harness enforces.
/// Boundary: matched on substrings of the command line, so a spelling that
/// reaches the same effect another way — an alias, a wrapper script, editing
/// `~/.estigia` by hand — is not matched.
///
/// # Why this is a family of its own
///
/// `IRREVERSIBLE` is the delivery steps `SKILL.md` names — publishing, merging,
/// tagging, releasing. Disarming the harness is not a delivery step, and
/// putting it there would widen a population that says in its own words what it
/// covers.
///
/// # What it was
///
/// Nothing. `estigia stand-down`, `estigia guard --uninstall` and
/// `estigia uninstall` all classified as `Untouched` — the answer that means
/// *this is not a write, carry on*. The whole authority model rests on the
/// agent not being able to switch the gate off, and it could, with the tool it
/// uses most and no trace beyond a shell line nobody reads.
///
/// # What gating it is worth, exactly
///
/// A run that has claimed nothing is `Outside` and is refused nothing, so this
/// changes nothing for it — and nothing needed changing, because it had no
/// gate to escape. What it changes is the case that matters: a run **holding a
/// claim** must renew it at this boundary, the same as at a merge. An agent
/// cannot quietly stand its own gate down halfway through work it swore to.
const DISARMS_THE_HARNESS: &[&str] = &[
    "estigia stand-down",
    "estigia guard --uninstall",
    "estigia uninstall",
    // Both rewrite the contract the gate reads, and `Delivery authorisation`
    // set to `auto` is the gate asking nobody.
    "estigia config set",
    "estigia setup",
    // The names the CLI answers to for that same verb. This list matches the
    // text of a command line, and `setup` carries
    // `visible_aliases = ["install", "tui"]` — so `estigia install --uninstall`
    // took Estigia out while classifying as `Untouched`, the answer that means
    // *this is not a write, carry on*. Not an exotic spelling: it is one the
    // product advertises in its own `--help`.
    //
    // The test beside this reads those aliases out of `cli/args.rs` rather than
    // trusting this comment, so a new one reopens the guard instead of the hole.
    "estigia install",
    "estigia tui",
    // And the one that disarms Estigia without naming it. `core.hooksPath`
    // points git somewhere else for **every** hook in the repository, so the
    // push guard — *the gate no agent can route around*, in this crate's own
    // README — simply stops running. One line, no file the shell analyser can
    // see, and it classified `Untouched`.
    //
    // The key alone, not `git config core.hooksPath`: a flag between the two
    // words defeats the longer fragment exactly as a flag before `push`
    // defeated `git push`, and `--local` is how the line is usually typed.
    // Reading the key costs one tracker read; not seeing it written costs the
    // guard.
    //
    // Lowercase because the line is: `core.hooksPath` as typed reaches this
    // list as `core.hookspath`.
    "core.hookspath",
];

/// The verb one disarming spelling names, when it names one.
///
/// `estigia guard --uninstall` is gated by the **pair**, not by `guard` alone —
/// installing a guard is not disarming one — so a reader asking *which verbs on
/// this list are gated* has to take the first word and stop. Comparing the whole
/// tail answered "no verb" for that entry, and the crossing that requires every
/// alias of a gated verb to be listed skipped it silently as a result.
///
/// Which is the hole that crossing exists for: it was written because `setup`
/// carries `visible_aliases = ["install", "tui"]` and `estigia install
/// --uninstall` took Estigia out while classifying as `Untouched`. An alias on
/// `guard` would do the same thing again, past the guard against it.
///
/// A function rather than the expression inlined in the test, because a
/// property nothing can call is a property nothing can fail.
pub fn disarming_verb(fragment: &str) -> Option<&str> {
    fragment
        .strip_prefix("estigia ")
        .map(|tail| tail.split_whitespace().next().unwrap_or(tail))
}

/// Shell fragments that mark a step nobody can take back.
///
/// guard:population irreversible-shell too-tight: the delivery steps
/// `SKILL.md` calls irreversible boundaries — publishing a review target,
/// merging, tagging, releasing. Legitimate population: any shell command that
/// makes one of them happen. Boundary: matched as whole commands within the
/// command line by [`contains_whole_command`], so a command that reaches the
/// same effect by another spelling (an alias, a script) is not matched. The one
/// spelling named here that is now
/// matched is `gh api` that changes something, by [`mutates_through_the_api`]:
/// a mutating method, **or any parameter**, because gh's own default turns the
/// second into the first.
///
/// Declared **too-tight** rather than fail-closed, after reading it: a spelling
/// that is not on the list escapes, and escaping is not failing closed. The
/// matching still leans that way — `git push` inside a longer line is a
/// boundary — because a false positive costs one extra tracker read before a
/// push and a false negative costs a merge that no live claim authorised.
///
/// It leaned too far once, and that is why the match now ends at a word.
/// `contains` alone made every fragment match any longer subcommand starting
/// with it, so `git merge-base --is-ancestor` was refused as `git merge`. That
/// is a **read**, it is what a run uses to ask whether a branch is already
/// integrated, and this crate's own transport calls it twice. Refusing work the
/// gate was never meant to see is not the cheap direction of the asymmetry: it
/// is how a gate teaches people to route around it, and a gate routed around
/// decides nothing. Reported from a real run on 2026-08-14.
///
/// Still open, and deliberately not fixed with it: `git tag --list` is a
/// listing, and there the fragment genuinely is the whole command. Telling it
/// from `git tag v1.2.3` means reading flags, which is a widening rather than a
/// matcher correction and wants its own change.
///
/// The proof boundary: this shows these spellings are gated, not that they are
/// the only spellings.
///
/// `gh api` used to be the example this boundary gave of a spelling it did not
/// match, while `delivery-phase` told its reader that this list "already owns"
/// exactly that escape. Both cannot be true, and the one that was not is the
/// one that mattered: `gh api -X PUT repos/o/r/pulls/7/merge` merges a pull
/// request, and it went past as `Untouched`. Two declarations handing a gap to
/// each other is the failure this registry exists to make impossible, so the
/// gap is closed rather than re-assigned.
const IRREVERSIBLE: &[&str] = &[
    "git push",
    "git merge",
    "git tag",
    "gh pr merge",
    "gh pr create",
    "gh release create",
    "gh release edit",
];

/// Whether `fragment` appears in `text` as a whole command rather than as the
/// beginning of a longer one.
///
/// `contains` alone was the matcher, and it gated reads. `git merge-base
/// --is-ancestor` — which writes nothing, answers whether one commit reaches
/// another, and is called twice by this crate's own transport — was refused as
/// `git merge`. `git tag --list` was refused as `git tag`. Reported from a run
/// that could not check whether a branch was already integrated, which is the
/// question `merge-base` exists to answer.
///
/// The fragment has to end where the command does or at a space. A subcommand
/// wearing the same prefix is a different command, and the gate that cannot
/// tell them apart refuses work it was never meant to see — which teaches
/// people to route around it, and a gate routed around decides nothing.
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

/// A delivery about to spend a verdict bound to bytes that have moved.
///
/// The rule this product is named for — *a verdict is bound to exact bytes;
/// every push invalidates it* — held where the verdict is **written** and not
/// where it is **used**: `publish-review` binds the target and reads it back,
/// and the boundary that delivers on it asked only whether the claim was still
/// live. A claim stays live across a push. The bytes do not.
///
/// **Additive, and only on positive evidence.** No recorded head, a command
/// that is not a delivery, a head this cannot read — every one of them leaves
/// the decision exactly as it was before this existed. That is not "an unknown
/// is clearance": the unknown falls back to the adjudication that already ran
/// and already denied what it denies, and the only thing this can do is add a
/// refusal when it has both SHAs in hand and they differ.
///
/// It sees what this run published **through Estigia's own tools**. A run that
/// shells out to the transport publishes without telling the pointer, and the
/// honesty contract states that reach rather than implying there is none.
fn stale_verdict(command: &str, run: &Run) -> Option<Refusal> {
    // The list already declared for exactly this population, rather than a
    // sixth copy of it: `git push` and `gh pr create` are absent from it for
    // the reason this check needs them absent — publishing a review target is
    // how a run *reaches* review, and pushing after one is how it fixes what
    // the review found. Refusing those would refuse the repair. What must not
    // happen is the delivery **after** them, where the stale verdict is spent.
    if !DELIVERS.contains(&command) {
        return None;
    }
    let reviewed = run.reviewed_head.as_deref()?;
    // The head the reviewer was shown is the one in the checkout the work
    // happens in — the same directory `publish-review` read it from.
    let here = run.worktree.as_ref().or(run.repo_dir.as_ref())?;
    let now = head_of(here)?;
    if now == reviewed {
        return None;
    }
    Some(Refusal::not_started(
        "verdict-bound-to-other-bytes",
        format!(
            "the review was published against {} and this checkout is at {} \u{2014} every push \
             invalidates the verdict and the CI run that went with it",
            short(reviewed),
            short(&now)
        ),
        Resolution::no_command(
            NoCommandReason::HumanAuthority,
            "a review of the bytes being delivered: publish the target again and ask for a \
             verdict on the new head, or reset this checkout to the head that was reviewed",
        ),
    ))
}

/// The first seven of a SHA, which is what a person reads.
fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// This checkout's head, when git will say.
///
/// `None` rather than a guess: a head nobody can read leaves the decision to
/// the adjudication that already ran.
fn head_of(directory: &std::path::Path) -> Option<String> {
    let answer = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "HEAD"])
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
/// Not a fragment, because it cannot be one: the method and the path can appear
/// in either order, so `gh api -X PUT <path>` and `gh api <path> -X PUT` are the
/// same call and no single substring is in both. Two conditions on one line
/// instead.
///
/// Every mutating method, not only the merge one. A false positive here is a
/// claim renewed before an API call that did not need it — the cheap direction
/// this module keeps choosing — and a false negative is the merge, the ref
/// deletion or the release that no live claim authorised.
fn mutates_through_the_api(normalized: &str) -> bool {
    const METHODS: &[&str] = &["put", "post", "patch", "delete"];
    // The method a parameter implies, which is the one nothing was reading.
    // `gh api --help`, in its own words: *the default HTTP request method is
    // `GET` normally and `POST` if any parameters are added*. So the ordinary
    // way to create something through that command names no method at all —
    // `gh api repos/o/r/issues -f title=x` opens an issue — and the sharpest of
    // them is the one the binding itself uses:
    // `gh api graphql -f query='mutation { … }'` is a tracker write with no
    // `-X` anywhere on the line.
    //
    // `--input` counts because a body is a parameter, and gh's own default
    // treats it as one.
    //
    // One entry for `-f` and `-F`: the line is lowercased before it gets here.
    const IMPLIES_POST: &[&str] = &["-f ", "-f=", "--field", "--raw-field", "--input"];
    normalized.contains("gh api")
        && (METHODS.iter().any(|method| {
            normalized.contains(&format!("-x {method}"))
                || normalized.contains(&format!("--method {method}"))
                || normalized.contains(&format!("--method={method}"))
        }) || IMPLIES_POST
            .iter()
            .any(|parameter| normalized.contains(parameter)))
}

/// Shell fragments that write to the repository without being irreversible.
///
/// guard:population repository-shell too-tight: the git commands that change the
/// working tree, the index or a ref. Legitimate population: every spelling of a
/// repository write that *names git*. Boundary: matched on substrings, so an
/// alias or a script wrapping one of these is not matched.
///
/// It used to read "every spelling of a repository write reachable from a
/// shell", which was a population this list never covered and never could: a
/// redirect and a `sed -i` write the same tree without naming git. That half now
/// belongs to `writing-shell`, which reads for it. Two families, because they
/// have two boundaries — the mistake was one declaration claiming both.
///
/// The list exists because `branch + worktree` is a *transport operation* — the
/// binding maps it to `start-branch`, which verifies the claim before it makes
/// the checkout. An agent that types `git worktree add` instead has made the
/// first write of a delivery with nothing checked, and the binding says plainly
/// what to do instead: *"Run executable reversible operations instead of
/// reconstructing them."* Gating these is how that sentence gets teeth.
///
/// Declared **too-tight** for the same reason as `write-tools`: the alternative
/// is gating every shell command, which is a harness nobody keeps on. The proof
/// boundary: these spellings are gated; the set is not proved complete.
///
/// Too-tight is a licence to be incomplete, not to omit a member sitting next to
/// one that is here. `git reset --hard` was listed and the four ordinary ways of
/// doing the same thing to the same tree were not: discarding named paths,
/// discarding all of them, moving the work to a stash, and deleting whatever was
/// never tracked. Each names git, each destroys uncommitted work, and each read
/// as `Untouched` — invisible to the gate, where their neighbour renewed the
/// claim.
///
/// `git checkout <branch>` stays out on purpose, and is why the two checkout
/// spellings are here rather than the bare word: switching branches does not
/// discard anything, because git refuses when it would.
const REPOSITORY_SHELL: &[&str] = &[
    "git worktree add",
    "git checkout -b",
    "git switch -c",
    "git commit",
    "git rebase",
    "git reset --hard",
    "git cherry-pick",
    "gh issue develop",
    // Deleting a ref, which this list's own population names beside the
    // working tree and the index.
    "git branch -d",
    "git branch --delete",
    // Discarding work that was never committed. Every one of these is the
    // population this list already declared: a repository write naming git.
    "git checkout --",
    "git checkout .",
    "git restore",
    "git stash",
    "git clean",
    // The same reading applied again, and it found ten more. Each of these sits
    // next to one already above, doing the same thing to the same tree:
    //
    // - `git worktree add` makes a checkout and `remove` deletes one.
    // - `git clean` deletes untracked files; `git rm` deletes tracked ones, and
    //   `git mv` moves them.
    // - `git cherry-pick` and `git revert` are one operation in two directions.
    // - `git commit` is here, and `git am` applies a patch *and* commits it.
    // - `git reset --hard` is here, and `--keep` and `--merge` move the same
    //   working tree to the same other commit.
    // - `git rebase` rewrites history, and so does `git filter-branch`.
    //
    // Every one names git, every one changes files this gate exists to measure,
    // and every one read as `Untouched` while its neighbour renewed the claim.
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

/// The boundaries that *deliver*, as opposed to the ones that publish.
///
/// guard:population delivery-phase too-tight: the built-in irreversible
/// spellings that end a delivery rather than open one. Legitimate population:
/// every step that lands work on a shared ref. Boundary: matched by exact
/// equality against the fragment [`classify_with`] recorded, so an
/// operator-declared boundary is never one of these — Estigia cannot know
/// whether somebody's `make deploy` delivers or rehearses, and guessing would
/// deny a step it never understood.
///
/// Declared **too-tight**: `git push` and `gh pr create` are deliberately absent
/// because publishing a review target is how a run *reaches* review, and gating
/// it on review would be a deadlock. A merge performed through the API is a
/// boundary `irreversible-shell` raises, and this list does not widen it into a
/// delivery: matched by exact equality, `gh api` is not one of these fragments,
/// so an API call renews the claim without being read as the merge itself.
const DELIVERS: &[&str] = &[
    "git merge",
    "git tag",
    "gh pr merge",
    "gh release create",
    "gh release edit",
];

/// The workflow states in which a delivery can be bound to a verdict.
///
/// Taken from `bindings/github.md` — *"The workflow's states are `analysis`,
/// `ready`, `in-progress`, `review`, `blocked` and `done`"* — and crossed
/// against that sentence by
/// `harness::tests::the_delivery_states_are_states_the_binding_declares`. Left
/// hand-written on both ends, a renamed state would make this match nothing and
/// the gate would refuse every delivery forever, which is the failure that looks
/// most like working correctly.
///
/// `done` is here beside `review` because the transition that closes an issue
/// can land before the last delivery step, and a guard that fires on correct
/// behaviour is a guard that gets switched off.
const DELIVERY_STATES: &[&str] = &["review", "done"];

/// Whether this boundary is a delivery the issue's state cannot support.
///
/// The narrowing borrowed from [statewright], whose engine gives each phase its
/// own tool list. Estigia already asked the tracker *whether* the run holds the
/// issue; it never asked whether the state it holds it in is one where the step
/// makes sense. A run in `in-progress` that ran `gh pr merge` was told yes,
/// because holding an issue and being allowed to land it are two questions and
/// only the first was being put.
///
/// [statewright]: https://github.com/statewright/statewright
pub(crate) fn out_of_phase(
    command: &str,
    state: &str,
    issue: u64,
    integration: crate::config::Integration,
    flag: Option<&str>,
) -> Option<Refusal> {
    if !DELIVERS.contains(&command) || DELIVERY_STATES.contains(&state) {
        return None;
    }
    // Trunk-based does not remove the protection, it swaps it. On a branch the
    // review *is* the protection — nothing reaches trunk until somebody
    // answered. Landing early trades that for a switch that is off, so the gate
    // asks for the switch by name.
    //
    // Naming a flag does not prove the change is behind it, and this does not
    // pretend otherwise: Estigia cannot read the code. What it does is make the
    // claim explicit and recorded, so "we thought it was flagged" stops being
    // something anybody can say afterwards.
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
    Some(Refusal::not_started(
        "out-of-phase",
        // The command is not named here: for a boundary the subject `prefixed`
        // puts in front *is* the command, and a refusal that says it twice reads
        // like a template nobody finished.
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
    let name = tool.to_ascii_lowercase();
    if SHELL_TOOLS.contains(&name.as_str()) {
        // The payload itself when no key names the command, for the reason the
        // write branch reads one: a tool that calls it something else is a
        // shell call the gate cannot see at all, and a payload naming `git
        // push` is a `git push`. The cost is a read on a payload that merely
        // mentions one, which is one tracker read.
        let command = command_argument(input).unwrap_or_else(|| input.to_string());
        // Two readings of the same line. The fragment lists want whitespace
        // collapsed, so `git   push` matches `git push`. `shell::writes_a_file`
        // wants it kept: it splits commands on newlines, and collapsing them
        // into spaces hides the `rm` on the second line of a two-line script
        // behind the `cd` on the first.
        let lowercased = command.to_ascii_lowercase();
        let normalized = without_leading_options(&lowercased);
        // Before the delivery boundaries, because this is the one that decides
        // whether any of them are still checked afterwards.
        //
        // Read through `as_estigia`, because the spelling that matters most is
        // the one Estigia writes itself: every hooks file it installs names the
        // binary by absolute path, `.exe` and all.
        let disarm_view = as_estigia(&normalized);
        if let Some(matched) = DISARMS_THE_HARNESS
            .iter()
            .find(|fragment| disarm_view.contains(*fragment))
        {
            return (
                Action::Boundary {
                    command: (*matched).to_owned(),
                },
                Sensitivity::Boundary,
            );
        }
        if mutates_through_the_api(&normalized) {
            return (
                Action::Boundary {
                    command: "gh api".to_owned(),
                },
                Sensitivity::Boundary,
            );
        }
        let declared = extra.iter().map(String::as_str);
        if let Some(matched) = IRREVERSIBLE
            .iter()
            .copied()
            .chain(declared)
            .find(|fragment| contains_whole_command(&normalized, fragment))
        {
            return (
                Action::Boundary {
                    command: matched.to_owned(),
                },
                Sensitivity::Boundary,
            );
        }
        if let Some(matched) = REPOSITORY_SHELL
            .iter()
            .find(|fragment| normalized.contains(*fragment))
        {
            return (
                Action::Write {
                    target: (*matched).to_owned(),
                },
                Sensitivity::Routine,
            );
        }
        // A write that does not name git. This used to be the sentence "a shell
        // that happens to redirect into a file is a gap, and it is a smaller gap
        // than a harness that gates `ls`" — which posed a choice between letting
        // `echo … > src/x.rs` past and gating every command. There is a third
        // answer, and `shell::writes_a_file` is it: read the constructs that
        // visibly write, and leave the rest alone.
        // Before the write reader, because none of these writes anything.
        if let Some(verb) = NEUTRALISES_A_FILE
            .iter()
            .find(|verb| normalized.contains(*verb))
            && surface_of(&lowercased) == Sensitivity::Boundary
        {
            return (
                Action::Write {
                    target: verb.trim().to_owned(),
                },
                Sensitivity::Boundary,
            );
        }
        if let Some(spelling) = shell::writes_a_file(&lowercased) {
            // A shell line that visibly writes and names a control surface is
            // the same event as the `Write` tool naming it, and only the second
            // was measured. `writes_a_file` answers with the **verb** it
            // recognised — `rm`, `mv`, "a redirect into a file" — so what the
            // check below was ever handed could not be a path, and
            // `rm ~/.claude/settings.json` came back `Routine` while
            // `Write(~/.claude/settings.json)` came back `Boundary`.
            //
            // The wrong way round, by this list's own argument: it exists
            // because an agent could switch the gate off "with the tool it uses
            // most", and the tool it uses most is the shell.
            return (Action::Write { target: spelling }, surface_of(&lowercased));
        }
        // Everything else really is left alone. `ls`, `grep` and `cargo test`
        // reach here and stay untouched, which is the property that keeps the
        // harness switched on.
        return (Action::Untouched, Sensitivity::Routine);
    }

    if WRITE_TOOLS.contains(&name.as_str()) {
        let named = path_argument(input);
        let target = named.clone().unwrap_or_else(|| tool.to_owned());
        // A write to what the harness reads its own answers from is not an
        // ordinary write. See [`CONTROL_SURFACE`].
        //
        // Read from the whole payload when no argument names a path, because
        // two of the eleven write tools put it in a patch body rather than in a
        // field — Codex's `apply_patch` and OpenCode's `patch`. For those the
        // target fell back to the *tool name*, which is never the control
        // surface, so `~/.claude/settings.json` could be rewritten through them
        // on an answer from inside the renewal window.
        let how = if is_control_surface(&target)
            || (named.is_none() && is_control_surface(&input.to_string()))
        {
            Sensitivity::Boundary
        } else {
            Sensitivity::Routine
        };
        return (Action::Write { target }, how);
    }

    (Action::Untouched, Sensitivity::Routine)
}

/// One of an agent's own argument names, whatever it spells it.
///
/// A key differing only in case or separators is the same key: `filePath` and
/// `file_path` are one argument, and reading only the second meant a tool
/// spelling it the first way had no path at all. The same rule
/// [`hook::Event::from_slug`] already applies to event names, for the same
/// reason — a name is copied out of somebody else's documentation and typed the
/// way that documentation spells it.
///
/// The value comes back as text whether it arrived as text or as a **list**.
/// Codex's `shell` tool sends its command as the argv array
/// `["bash", "-lc", "git push"]`, and reading it as a string answered nothing:
/// every shell call from that agent classified `Untouched`, from an agent this
/// build installs a gate into and reports as `gate on`.
fn argument_named(input: &serde_json::Value, names: &[&str]) -> Option<String> {
    let letters = |key: &str| -> String {
        key.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|character| character.to_ascii_lowercase())
            .collect()
    };
    input.as_object()?.iter().find_map(|(key, value)| {
        if !names.contains(&letters(key).as_str()) {
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

/// The path a write tool names.
fn path_argument(input: &serde_json::Value) -> Option<String> {
    argument_named(input, &["filepath", "path", "notebookpath"])
}

/// The command line a shell tool was handed.
fn command_argument(input: &serde_json::Value) -> Option<String> {
    argument_named(input, &["command", "commandline"])
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
    ///
    /// Carried because the harness can only hold the tools of a binding that has
    /// an executable, and running the wrong one is worse than running none.
    pub tracker: crate::config::Tracker,
    /// Where work integrates: a reviewed branch, or trunk behind a flag.
    pub integration: crate::config::Integration,
    /// The feature flag this run declared, if any.
    ///
    /// Read from the environment rather than the contract, because it is a
    /// property of **this change** and not of the repository — a flag written
    /// into the operator's table would cover every issue at once, which is the
    /// opposite of what a flag is for.
    pub flag: Option<String>,
    /// The stand-down the operator declared, if the record is on disk.
    ///
    /// Read once when the context is built rather than at each decision, so one
    /// tool call cannot be half inside a window that expires mid-check.
    pub stand_down: Option<standdown::StandDown>,
    /// Commands this operator declared irreversible, beyond the built-in list.
    ///
    /// Read from the contract, so a repository that publishes with `npm publish`
    /// can say so without a rebuild. See [`classify_with`] for why it only adds.
    pub boundaries: Vec<String>,
}

/// Decides whether one action may proceed.
///
/// The gate never writes to the tracker. It reads, and it says yes or no —
/// which is what lets it sit on the critical path of every edit without being
/// able to damage anything by being wrong.
pub fn gate(context: &GateContext, run: &mut Run, action: &Action, how: Sensitivity) -> Decision {
    // Whatever this decides, a stand-down the operator declared may let it
    // through — saying so. Wrapped here, at the one place a tool decision is
    // made, because a stand-down honoured on some paths and not others is worse
    // than none: an operator would learn it works and be surprised by the door
    // that ignored it.
    // The clock, carried as the answer it is. A fifth hand-rolled read of it
    // lived here; `session::now_seconds` is the one that already says `None`
    // when the machine cannot answer, and a stand-down decided on a clock
    // nobody could read is the gate opening on a fault.
    let now = session::now_seconds();
    standdown::over(
        decide(context, run, action, how),
        context.stand_down.as_ref(),
        now,
    )
}

/// Whether this write lands where no checkout the claim covers can reach.
///
/// A claim governs a repository. It was governing the machine: after an issue
/// auto-closed on merge the gate refused a scratch note and the agent's own
/// memory store, each with *issue #164 is CLOSED*, because nothing here
/// classified the **path being written** — [`Aside::AnotherCheckout`] compares
/// the checkout the hook was invoked in, not the file. A run that had just
/// delivered could not write down the evidence of the delivery, which is the
/// outcome this crate exists to prevent, and the way past it was raw tooling.
///
/// Narrow on purpose, because standing aside is a statement and an unknown is
/// not one:
///
/// - the target must be an **absolute** path. The shell classifier answers with
///   the verb it recognised — *"a redirect into a file"*, `rm`, `mv` — so a
///   shell write cannot be placed, and one that cannot be placed stays gated.
/// - a run covering nothing has no claim to be outside of.
///
/// And one limit no amount of placing reaches: a **hard link**, or a bind mount,
/// has no path to resolve to, so an alias outside the checkout pointing at a
/// file inside it classifies outside and the write lands inside. Both measured.
/// `is_control_surface` declares the same boundary for its own matcher and this
/// inherits it; `docs/honesty.md` carries the measurements.
///
/// The caller adds the last condition, which cannot be seen from here: only a
/// `Routine` write. The control surface lives outside the repository by nature,
/// and watching it is the whole defence against an agent switching the gate off
/// with the tool it uses most.
fn writes_outside_the_claim(run: &Run, action: &Action) -> bool {
    let Action::Write { target } = action else {
        return false;
    };
    let path = std::path::Path::new(target);
    if !path.is_absolute() {
        return false;
    }
    // Where it lands, not how it is spelled. Comparing the spelling answered
    // `outside` for a `..` that climbs back into the checkout, and for a new
    // file inside a checkout reached through a junction — both of them writes
    // this would have taken out of the gate. A path that cannot be placed is
    // read as inside, because a wrong answer here removes the gate.
    let Some(landing) = crate::paths::placed(path) else {
        return false;
    };
    let mut covered = run.covered().peekable();
    if covered.peek().is_none() {
        return false;
    }
    // Both sides through the same resolver. `covers` leaves an unresolvable
    // path literal, so a covered checkout that is not on disk — or a temp root
    // reached through a link, which is what macOS hands every test — left the
    // two sides in different vocabularies and a write into the checkout read as
    // outside. A checkout this process cannot place is one it cannot rule out.
    !covered.any(|checkout| {
        crate::paths::placed(checkout)
            .is_none_or(|checkout| crate::paths::covers(&checkout, &landing))
    })
}

/// What the gate decides before any stand-down is considered.
fn decide(context: &GateContext, run: &mut Run, action: &Action, how: Sensitivity) -> Decision {
    let verifier = match action {
        Action::Untouched => return Decision::Outside(Aside::NotWatched),
        Action::Write { .. } => "write",
        Action::Boundary { .. } => "boundary",
    };
    let subject = action.subject().unwrap_or_default().to_owned();

    // A pointer that is on disk and cannot be read says a run under this name
    // existed; it does not say what it swore. The directive tells every agent
    // that an unknown result is not clearance, and standing aside here would be
    // the code saying the opposite — most visibly right after an upgrade, when
    // every pointer the previous release wrote may be unreadable at once.
    if run.unreadable {
        return Decision::Deny(Box::new(Refusal::not_started(
            "run-pointer-unreadable",
            format!(
                "{}: this run's record exists and cannot be read, so whether it holds an issue \
                 is unknown",
                run.run_id
            ),
            // Not `estigia release`. It was named here and never discharged
            // this: with an unreadable pointer that command cannot say what to
            // put down either, so it refuses with this same code — a message
            // sending an agent to a command that answers the same thing again,
            // which is the one thing the ratchet forbids.
            //
            // Nor can a runnable one be written, because the issue number is
            // the missing fact. What clears this is knowing what the run holds:
            // claim it again, and the pointer is rewritten readable; or decide
            // it holds nothing and take the file away. Both are the operator's,
            // and both need the tracker read first.
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what that run holds, read from the tracker \u{2014} then claimed again, or its \
                 pointer removed",
            ),
        )));
    }

    let Some(issue) = run.issue else {
        // Sworn nothing. See the module docs: the oath binds once sworn.
        return Decision::Outside(Aside::NothingSworn);
    };

    // A tracker with no executable transport cannot be asked anything, so there
    // is nothing to enforce. Denying every write would be a lock rather than
    // authority, and running the GitHub transport against it would be worse:
    // `gh` calls to a tracker that is not there, answered as though they were.
    // `estigia doctor` says so out loud; here the harness simply stands aside.
    if context.tracker.transport().is_none() {
        return Decision::Outside(Aside::NoTracker);
    }

    // A claim covers the checkout it was made in and the isolated one this run
    // was given. A write anywhere else is not covered by it, and pretending
    // otherwise would gate the operator's unrelated work with somebody else's
    // issue.
    if run.covered().count() > 0
        && !run
            .covered()
            .any(|covered| crate::paths::covers(covered, &context.repo_dir))
    {
        return Decision::Outside(Aside::AnotherCheckout);
    }

    if how == Sensitivity::Routine && run.within_window(context.window) {
        return Decision::Allow(format!(
            "issue #{issue} was verified inside the renewal window"
        ));
    }

    let issue_argument = issue.to_string();
    // The same missing state, defaulted the other way round on purpose. A
    // pointer that never recorded one still has to be checked against
    // something, and `in-progress` is the state a claim starts in: if the issue
    // has since moved, the tracker answers that it has not got the expected one
    // and the write is refused. Guessing here **costs a refusal**; guessing in
    // `hook::state_clause` would have **announced a fact**, which is why that
    // one says the state is unrecorded and this one does not. Do not unify them
    // — they are one value serving two directions.
    let state = run
        .state
        .clone()
        .unwrap_or_else(|| "in-progress".to_owned());
    // The control surface first, and this check is **new work rather than the
    // same check moved**. Spawning the transport began by asking whether
    // `scripts/github.py` was on disk, and that file's absence stood in for the
    // skill's — so an uninstalled Estigia refused here for a reason that was
    // true by accident. Answering in this process removed the accident and, with
    // it, the check: the gate went on to ask the tracker while the operator's
    // contract was not installed at all.
    //
    // So it asks about the contract, which is what the authority actually rests
    // on. `SKILL.md` holds the operator table this gate enforces; without it
    // there is no surface to read, and *an unreadable control surface permits no
    // write* is the rule in the contract's own words.
    let contract = context.skill_root.join(crate::skill::CONTRACT);
    if !contract.is_file() {
        return Decision::Deny(Box::new(prefixed(
            crate::outcome::Refusal::not_started(
                "control-surface-not-installed",
                format!("{} is not installed", contract.display()),
                crate::outcome::Resolution::run("estigia setup --all"),
            ),
            &subject,
        )));
    }

    // After the contract, and before the tracker.
    //
    // Before the tracker because the tracker's answer is what refuses here: a
    // closed issue answers `issue-not-open`, and it was answering it about a
    // scratch note. A claim governs the repository it was made in; what a run
    // writes outside every checkout it covers is not the claim's to decide,
    // whatever state the issue is in.
    //
    // After the contract because *an unreadable control surface permits no
    // write* is written without an exception **in the contract**, and standing
    // aside is a permission like any other. It sat above that refusal for one published head, which
    // meant that with no `SKILL.md` installed the agent instruction files —
    // outside every checkout by construction, and carrying the directive that
    // says this harness holds the authority at all — were writable with nothing
    // consulted. Both reviewers of that head raised the ordering; one measured
    // that moving it down costs no test, so it moved rather than being written
    // up as a limit.
    //
    // That buys the rule here and not everywhere: the renewal window above
    // returns before the contract is looked at, so for its duration the same
    // files are writable with no contract on disk. Issue #29, and older than
    // this answer — but do not read this position as the rule being whole.
    // Moving *this* down was free; moving the window down changes the answer for
    // every routine write on the fast path this gate exists to keep cheap.
    //
    // The window also now answers first for a write outside the claim, with a
    // message crediting a claim renewal for clearing a path the claim does not
    // govern, and it takes the `session::store` branch that an `Outside` does
    // not. Both permit, so no gate moved; it is written down because a reviewer
    // had to measure it to find out.
    //
    // `Routine` only. A `Boundary` write is watched *because* of where it lands
    // — the control surface sits outside the repository by nature — so placing
    // it outside must never be what waves it through.
    if how == Sensitivity::Routine && writes_outside_the_claim(run, action) {
        return Decision::Outside(Aside::OutsideTheClaim);
    }

    // Asked in this process. This was the last call in the crate that spawned
    // `python <skill>/scripts/github.py`, and it is the one that mattered most:
    // it runs on **every gated write** that falls outside the renewal window, so
    // an interpreter start-up was on the critical path of the hook this whole
    // crate is built to keep cheap.
    //
    // Nothing about the decision changes. The flags are the same flags, the
    // answer is translated by the same `tracker::translate`, and a failure keeps
    // the exit code it always had — which is what lets the `match` below stay
    // exactly as it was.
    if let Some(named) = context.tracker.named_repo() {
        // SAFETY: the gate answers one hook invocation per process, and the
        // value comes from a configuration that does not change while it runs.
        unsafe { std::env::set_var("GH_REPO", named) };
    }
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
    // Through the one reader that can say it does not know, and a clock that
    // will not say is a **refusal**. Reading the wall clock here and defaulting
    // it to the epoch was this change's own first mistake, caught by the guard
    // that exists for it: the epoch is a perfectly good number, so every horizon
    // ever written is still ahead of it and every claim reads as live. The
    // question this stamp decides is *is that claim still good now*, and a run
    // that cannot say when now is cannot answer it.
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
        )));
    };
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
    .or_else(|failure| {
        Ok::<tracker::Answer, crate::outcome::Refusal>(tracker::Answer {
            code: failure.code(),
            body: Some(failure.envelope()),
        })
    });

    match answer {
        Ok(answer) => match tracker::translate(&answer, verifier) {
            None => {
                // The tracker agreed the run holds this issue in `state`, so
                // from here `state` is the tracker's answer rather than the
                // run's belief — which is what makes it safe to decide on.
                run.mark_verified();
                if let Action::Boundary { command } = action
                    && let Some(refusal) = stale_verdict(command, run)
                {
                    return Decision::Deny(Box::new(prefixed(refusal, &subject)));
                }
                if let Action::Boundary { command } = action
                    && let Some(refusal) = out_of_phase(
                        command,
                        &state,
                        issue,
                        context.integration,
                        context.flag.as_deref(),
                    )
                {
                    return Decision::Deny(Box::new(prefixed(refusal, &subject)));
                }
                Decision::Allow(format!("issue #{issue} is held by {}", run.run_id))
            }
            Some(refusal) => Decision::Deny(Box::new(prefixed(refusal, &subject))),
        },
        // The control surface could not be reached at all. "An unreadable
        // control surface permits no write" — this is the sentence, enforced.
        Err(refusal) => Decision::Deny(Box::new(prefixed(refusal, &subject))),
    }
}

/// Puts what was being attempted in front of why it was refused.
fn prefixed(refusal: Refusal, subject: &str) -> Refusal {
    Refusal {
        message: format!("{subject}: {}", refusal.message),
        ..refusal
    }
}

/// Where the transport lives, or where it would live.
///
/// [`discover_skill_root`] answers "which installed skill can be asked", and
/// refuses when none can. That refusal used to travel out to the hook, which
/// turned it into an empty answer — no decision, and the write went through. So
/// a run under oath stopped being gated the moment the skill directory was
/// deleted, which is exactly the case the directive promises is covered: an
/// unreadable control surface permitting a write. The push guard failed the same
/// way, and a push is the more expensive end.
///
/// The rule itself was never missing. [`gate`] already denies with
/// `transport-not-installed` when it cannot ask; it simply never got the chance,
/// because the context was gone before a decision was reached. The same rule had
/// two spellings — transport missing from the root that was found, and missing
/// from every root — and they answered opposite ways.
///
/// Answering with the place the transport *should* be gives the rule its chance
/// and costs nothing to a run that swore nothing: [`gate`] returns
/// [`Decision::Outside`] on an empty oath long before it looks here.
pub fn control_surface() -> PathBuf {
    discover_skill_root().unwrap_or_else(|_| {
        let options = crate::setup::SetupOptions::default();
        crate::setup::AGENTS
            .iter()
            .find_map(|adapter| crate::setup::resolve_paths(adapter, &options).ok())
            .map(|paths| paths.skill_root)
            // Nowhere to point at, so anywhere without a transport will do: the
            // only thing asked of this path is that asking it fails.
            .unwrap_or_else(|| PathBuf::from("estigia-has-no-installed-skill"))
    })
}

/// The first installed skill root, for a harness that was given no explicit one.
pub fn discover_skill_root() -> Result<PathBuf, Refusal> {
    let options = crate::setup::SetupOptions::default();
    let roots: Vec<PathBuf> = crate::setup::AGENTS
        .iter()
        .filter_map(|adapter| crate::setup::resolve_paths(adapter, &options).ok())
        .map(|paths| paths.skill_root)
        .filter(|root| root.join(crate::skill::CONTRACT).is_file())
        .collect();
    // The **contract** is what makes a directory a skill root, and this asked
    // for `scripts/github.py`. That file was retired, so the filter matched
    // nothing and the harness could not find its own installed skill at all —
    // `doctor` reported *no installed skill was found* on a machine that had
    // one. The transport was never the thing that made a root usable; it was
    // the thing that happened to be in every root.
    //
    // Estigia's own first, for the reason below. Estigia installs its skill
    // under upstream's name on purpose, so an operator who already runs
    // `issue-flow` has a root that passes this test and holds no configuration
    // Estigia ever wrote.
    //
    // Taking that one made the gate read a contract with no block in it, which
    // is every setting at its default: no declared boundary is a boundary, and
    // the renewal window is at its widest. Meanwhile `config list` read the
    // agent's own contract and reported the operator's values back to them,
    // correctly. Two commands, one machine, and the one that was right was the
    // one that does not decide anything.
    roots
        .iter()
        .find(|root| {
            std::fs::read_to_string(root.join(crate::skill::CONTRACT))
                .is_ok_and(|text| text.contains(crate::config::BLOCK_BEGIN))
        })
        .or_else(|| roots.first())
        .cloned()
        .ok_or_else(|| {
            Refusal::not_started(
                "harness-not-installed",
                "no installed skill was found",
                Resolution::run("estigia setup --all"),
            )
        })
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
