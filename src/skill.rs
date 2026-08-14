//! The markdown the agent reads, embedded and written out unchanged.
//!
//! The skill is not compiled. `SKILL.md`, `bindings/` and `references/` are
//! text an agent opens, and the property that makes them worth keeping is that
//! a person can read them too. They are embedded with [`include_str!`] so the
//! binary installs offline with no trust surface at all — no clone, no
//! download, no second checksum to get wrong.
//!
//! # The skill keeps its own name
//!
//! Estigia consumes the skill; it does not absorb it. The installed directory
//! is `issue-flow`, the frontmatter is upstream's, and the two version at
//! different rates on purpose. Estigia performs exactly **one** transformation
//! on the way to disk — it replaces the managed configuration block in
//! `SKILL.md` with the operator's resolved [`Config`]. That block is the one
//! thing Estigia owns, and `configuration_block_is_the_only_transformation`
//! holds it to that.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::{CONFIG_FENCE, Config};
use crate::outcome::{NoCommandReason, Refusal, Resolution};

pub(crate) mod record;

#[cfg(test)]
std::thread_local! {
    static REPOSITORY_SNAPSHOT_REPLACEMENT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The directory name the skill is installed under.
///
/// Upstream's, not ours — see the module docs. Checked against the embedded
/// frontmatter by a test, so renaming one without the other does not compile
/// into a silent mismatch.
pub const DIRECTORY: &str = "flow";

/// The SDD planning phases, as sub-agent definitions the host can route to.
///
/// # Why these are not in [`FILES`]
///
/// They do not belong to the skill directory. A sub-agent definition is only
/// read where the host looks for one — `~/.claude/agents/`, and OpenCode's two
/// roots — and [`crate::harness::roles::definition_for`] reads exactly those.
/// Shipped inside the payload they would install cleanly, be found by nobody,
/// and declare a tool list nothing enforces. That is the arrangement this crate
/// criticises in others, committed by this crate, so the destination is what
/// separates them rather than a comment asking somebody to remember.
///
/// # What makes them worth shipping at all
///
/// Other harnesses already ship these by the handful; each writes a
/// `tools:` line and then relies on the host to honour it. None of them sits at
/// the tool boundary. Estigia does — Claude Code sends `agent_type` on every tool
/// event fired inside a sub-agent — so the same declaration becomes a refusal
/// rather than a request. Same names, same host routing, and the list is true.
///
/// `{{MODEL}}` and `{{TOOLS}}` are substituted at install: the model from
/// `Model routing`'s phase key, and the tool list from whether the operator's
/// `Planning` row keeps artifacts on the issue or under `openspec/`.
pub const PHASE_AGENTS: &[SkillFile] = &[
    SkillFile {
        path: "agents/sdd-explore.md",
        contents: include_str!("../skill/agents/sdd-explore.md"),
    },
    SkillFile {
        path: "agents/sdd-propose.md",
        contents: include_str!("../skill/agents/sdd-propose.md"),
    },
    SkillFile {
        path: "agents/sdd-spec.md",
        contents: include_str!("../skill/agents/sdd-spec.md"),
    },
    SkillFile {
        path: "agents/sdd-design.md",
        contents: include_str!("../skill/agents/sdd-design.md"),
    },
    SkillFile {
        path: "agents/sdd-tasks.md",
        contents: include_str!("../skill/agents/sdd-tasks.md"),
    },
];

/// One embedded file, and where it lands relative to the skill directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillFile {
    /// Forward-slashed, relative to the skill directory.
    pub path: &'static str,
    /// The file's bytes, embedded at build time.
    pub contents: &'static str,
}

/// Every file the skill is made of.
///
/// `include_str!` is the manifest check: a file removed from `skill/` stops the
/// build rather than becoming a reference the contract names and nobody ships.
pub const FILES: &[SkillFile] = &[
    SkillFile {
        path: CONTRACT,
        contents: CONTRACT_CONTENTS,
    },
    SkillFile {
        path: "bindings/github.md",
        contents: include_str!("../skill/bindings/github.md"),
    },
    SkillFile {
        path: "bindings/linear.md",
        contents: include_str!("../skill/bindings/linear.md"),
    },
    SkillFile {
        path: "bindings/trello.md",
        contents: include_str!("../skill/bindings/trello.md"),
    },
    SkillFile {
        path: "references/domain-composition.md",
        contents: include_str!("../skill/references/domain-composition.md"),
    },
    // Ships, and nothing in the payload names it. Upstream's own test suite
    // draws the same conclusion and makes the same exception, verbatim:
    //
    //     check("every companion on disk is reachable from the contract",
    //           sorted(on_disk - linked - {"references/migration-inventory.md"}), [])
    //
    // Dropping it was a divergence dressed as a cleanup. Upstream's payload is
    // upstream's payload, and upstream's own suite runs
    // those 328 checks against exactly what Estigia installs — which it cannot
    // do if what Estigia installs is a subset. The test is
    // `the_payload_passes_the_suite_its_authors_wrote_for_it`, in
    // `tests/payload.rs`.
    SkillFile {
        path: "references/migration-inventory.md",
        contents: include_str!("../skill/references/migration-inventory.md"),
    },
    SkillFile {
        path: "references/repository-delivery.md",
        contents: include_str!("../skill/references/repository-delivery.md"),
    },
    SkillFile {
        path: "policies/blind-judges.md",
        contents: include_str!("../skill/policies/blind-judges.md"),
    },
    SkillFile {
        path: "protocols/rdd.md",
        contents: include_str!("../skill/protocols/rdd.md"),
    },
    SkillFile {
        path: "protocols/sdd.md",
        contents: include_str!("../skill/protocols/sdd.md"),
    },
    SkillFile {
        path: "references/runtime-notes.md",
        contents: include_str!("../skill/references/runtime-notes.md"),
    },
    SkillFile {
        path: "references/safety-incidents.md",
        contents: include_str!("../skill/references/safety-incidents.md"),
    },
    SkillFile {
        path: "assets/analyst-issue-template.md",
        contents: include_str!("../skill/assets/analyst-issue-template.md"),
    },
];

/// The directories whose files a setting chooses rather than a link.
///
/// A binding is chosen by `Tracker`, a protocol by `Planning` or by
/// `Review protocol`, a policy by its own setting. None of them is linked from the contract, and none should
/// be: linking all of them would tell the agent to read every alternative.
///
/// Declared once, because it began as one `starts_with` and became two, and two
/// scattered special cases are how a third gets added somewhere else. Each
/// prefix owes a seam test of the shape `every_<axis>_names_a_<file>_that_ships`.
pub const SELECTED_BY_SETTING: &[&str] = &["bindings/", "protocols/", "policies/"];

/// What this crate shipped once and does not ship now.
///
/// A path removed from [`FILES`] is not a path removed from anybody's machine:
/// `uninstall_from` walks that list, so what leaves the list simply stops being
/// anyone's to take away. This is the list that says *take it away*.
///
/// It is not a general cleanup and must not become one. Every entry is a file
/// **this crate wrote**, named here on the way out, and removed only where the
/// install record says Estigia created it — the operator's own files are not on
/// this list and cannot be.
pub const RETIRED: &[&str] = &[
    // The transport. `bindings/github.md` used to route every reversible
    // operation through it, so it shipped — and shipping it meant every machine
    // carried a second implementation of every decision the gate makes, running
    // beside the first and able to disagree with it. They did, repeatedly, and
    // each disagreement was found by putting the same input through both.
    //
    // The binding names Estigia's tools now and the operations are answered in
    // process, so the file is not the transport any more: it is a copy of one,
    // sitting in a directory the contract no longer mentions, that anything
    // going looking would still find and run.
    "scripts/github.py",
];

/// The contract file, whose managed block carries the configuration.
pub const CONTRACT: &str = "SKILL.md";

/// The operations `SKILL.md` requires every binding to map.
///
/// Read out of the contract rather than copied beside it: a list maintained in
/// two places is a list that disagrees with itself, and the whole point of a
/// seam test is that neither side gets to drift quietly.
pub fn required_operations() -> Vec<String> {
    let line = CONTRACT_CONTENTS
        .lines()
        .find(|line| line.contains("MUST map"))
        .unwrap_or_default();
    let mut operations = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        let name = &rest[..close];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            operations.push(name.to_owned());
        }
        rest = &rest[close + 1..];
    }
    operations
}

/// The contract's bytes, named once so nothing has to search [`FILES`] for them
/// and unwrap the result. A lookup that "cannot fail" still has to be written
/// as though it can, and the honest way out is not to look it up.
pub const CONTRACT_CONTENTS: &str = include_str!("../skill/SKILL.md");

/// The skill's own version, from the `SKILL.md` frontmatter.
///
/// Separate from the crate version on purpose: the contract and the tool are
/// published at different rates, which was the reason for keeping them apart.
pub fn version() -> Option<&'static str> {
    frontmatter_field(CONTRACT_CONTENTS, "version")
}

/// The skill's declared name, from the `SKILL.md` frontmatter.
pub fn declared_name() -> Option<&'static str> {
    frontmatter_field(CONTRACT_CONTENTS, "name")
}

fn frontmatter_field(document: &'static str, key: &str) -> Option<&'static str> {
    let body = document.strip_prefix("---")?;
    let end = body.find("\n---")?;
    body[..end].lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"'))
    })
}

/// What one file's contents should be once installed.
///
/// Verbatim, except for the contract, whose managed block becomes the
/// operator's configuration.
pub fn rendered(file: &SkillFile, config: &Config) -> String {
    if file.path == CONTRACT {
        CONFIG_FENCE.upsert(file.contents, &configuration_body(config))
    } else {
        file.contents.to_owned()
    }
}

/// Takes away what this crate used to ship, and nothing else.
///
/// **Fed rather than reaching for the constant**, so the rule can be measured
/// against a list a test owns: `RETIRED` is a compile-time constant, and a test
/// that re-implements the decision instead of calling it is a test that agrees
/// with itself.
fn retire(
    root: &Path,
    retired: &[&str],
    created: &std::collections::BTreeSet<String>,
    pending: &mut Pending,
    dry_run: bool,
) -> Result<Vec<FileAction>> {
    let mut actions = Vec::new();
    for path in retired {
        let file = root.join(relative(path));
        let present = match pending.get(&file) {
            Some(remembered) => remembered.is_some(),
            None => file.is_file(),
        };
        // Present, and **named by the record as this crate's own**. A file
        // somebody else put at that path is theirs, and the same rule governs
        // every other removal this crate makes.
        if !present || !created.iter().any(|made| made == path) {
            continue;
        }
        // And still what this crate wrote there. The third removal path, and it
        // ignored the digest that the install and the uninstall already read —
        // so a transport somebody had patched was taken with the rest.
        //
        // Kept, like the other two, and the tension is real and worth naming: a
        // retired file is one Estigia asked to be gone, and leaving an
        // executable copy of an old transport beside a contract that no longer
        // mentions it is the hazard retirement exists to end. It is left anyway,
        // because taking somebody's work is the worse failure of the two and the
        // file is theirs now — nothing this crate runs will look for it. The
        // line it is reported on says which it is.
        if record::written(root, path).is_some_and(|last| {
            std::fs::read_to_string(&file).is_ok_and(|found| record::digest_of(&found) != last)
        }) {
            actions.push(FileAction {
                path: file,
                change: Change::Kept,
            });
            continue;
        }
        if !dry_run {
            std::fs::remove_file(&file)
                .with_context(|| format!("remove the retired {}", file.display()))?;
            // And the directory, when the file was the whole of it.
            //
            // `remove_empty_directories` sweeps the parents of `FILES`, and a
            // retired path is by definition not in that list — so `scripts/`
            // came through every removal and sat empty in a skill directory
            // whose owner had just been told the application was gone. The
            // comment above the retirement loop in `uninstall_from` claimed
            // both the file and the directory had been fixed. Only the file had.
            remove_if_empty(&file, root);
        }
        pending.insert(file.clone(), None);
        actions.push(FileAction {
            path: file,
            change: Change::Remove,
        });
    }
    Ok(actions)
}

/// The body of the managed block: the prose the agent needs to use the table,
/// and the table.
///
/// Public because `setup::rewrite_configuration` writes the same block and used
/// to build its own, shorter version inline — so `estigia config set` silently
/// deleted the paragraph naming `estigia.local.md`. The override mechanism then
/// existed in the code and nowhere in the text, which is precisely what the note
/// below says must not happen. One body, one caller-visible function.
///
/// It names the local override file, because Estigia *reads* one. The block it
/// replaces told the agent to look beside the contract for `operator.local.md`;
/// leaving that out would have shipped a contract where the override mechanism
/// exists in the code and nowhere in the text — and the agent looks in the text.
pub fn configuration_body(config: &Config) -> String {
    format!(
        "---\n\n\
         ## Operator configuration\n\n\
         Written by `estigia setup`. Edit it with `estigia config set` rather than here: this \
         block is replaced whole on the next run, and an edit made in place is lost without \
         being reported.\n\n\
         Before using the table, look for `{local}` beside this file. When present, its \
         same-named rows override this table and its additional instructions are local policy \
         too. That file holds permissions and machine-specific paths, is ignored by Git, and \
         MUST NOT be committed or published. `{legacy}` is read only when `{local}` is not \
         there, for installations that came from issue-flow \u{2014} with both present, \
         the older one is ignored entirely.\n\n\
         {method}{rows}",
        local = crate::config::LOCAL_FILE,
        legacy = crate::config::LEGACY_LOCAL_FILE,
        method = selected_documents(config),
        rows = config.render_rows()
    )
}

/// The lines that send the agent to the documents this configuration selects.
///
/// A setting the agent cannot act on is a setting that does nothing. `Tracker`
/// works because the contract tells the agent to read the named binding; every
/// other axis needs the same sentence, or the table records a choice and no
/// behaviour follows it.
///
/// Silent for the values that add nothing: pointing a reader at a document that
/// only repeats the contract they already have open is noise, and noise in a
/// block regenerated on every run is noise nobody can delete.
fn selected_documents(config: &Config) -> String {
    let mut lines = String::new();
    if let Some(document) = config.planning.document() {
        let value = config.planning.as_value();
        lines.push_str(&format!(
            "Planning here follows `{document}` (`{value}`): read it before writing.

"
        ));
    }
    if let Some(document) = config.review_protocol.document() {
        let value = config.review_protocol.as_value();
        lines.push_str(&format!(
            "Review here follows `{document}` (`{value}`): read it before a verdict.

"
        ));
    }
    if let Some(document) = config.judges.document() {
        let value = config.judges.as_value();
        lines.push_str(&format!(
            "Independent review here follows `{document}`: {value} over one target.

"
        ));
    }
    // Which commands this repository treats as one-way, when it named any.
    //
    // The contract already tells the agent to renew "at every expensive or
    // irreversible boundary" — a judgement it has to make. The operator's list
    // is the part it cannot derive: `terraform apply` is a one-way door here
    // and nowhere in the sentence that asks for the judgement. Without the
    // list, the agent judges, guesses low, and is refused; the refusal is
    // recoverable, and being refused is still a worse way to learn a fact
    // somebody had already written down.
    //
    // Not a repeat of the contract, which is the bar `selected_documents` sets
    // for saying anything here: the rule is general and this is particular.
    if !config.boundaries.is_empty() {
        lines.push_str(&format!(
            "This repository treats these as one-way doors, on top of the ones the contract \
             already names: {}. Renew the claim before each of them, the same as any other \
             irreversible boundary.

",
            config.boundaries.join(", ")
        ));
    }

    // Where work integrates, when it is not the branch this skill assumes.
    //
    // The third and worst of these. The other two were inert rows; this one
    // **changes the route** and the word `trunk` appears nowhere in the skill
    // — not once, in any file the agent reads. So an agent on a trunk
    // repository takes the branch route it was taught, reaches the delivery
    // boundary, and is refused `unflagged-on-trunk`: told to name a flag it was
    // never told this repository runs on, at the one moment it is most
    // expensive to learn.
    //
    // The gate keeps refusing either way. What changes is whether being refused
    // is how the agent finds out.
    if matches!(config.integration, crate::config::Integration::Trunk) {
        lines.push_str(
            "Work here lands on **trunk**, not through a reviewed branch. A change may land \
             before a verdict exists only if it is behind a feature flag, and the flag has to be \
             named \u{2014} declare it in `ESTIGIA_FLAG` before the delivering write. Landing switched \
             on still needs a review. Naming a flag is not proof that the change is behind it; it \
             is a claim, recorded, that somebody can be held to.

",
        );
    }

    // What a model routing obliges, when one is named.
    //
    // The same rule, one row over, and this one was further gone. `ask` at
    // least carries an obvious plain meaning; `analyst=opus` means nothing to
    // an agent that was never told the row exists. Nothing in Estigia reads it
    // either — `ModelRouting` has lookups by role, by phase and by state, and
    // no caller outside the configuration module.
    if !config.models.by_role.is_empty() {
        lines.push_str(&format!(
            "Delegated work here runs on named models (`{}`): when you hand a role, a phase or a \
             state to another context, start it on the model named for it and say which one you \
             used. A row naming a model that nobody reads is a choice the operator made and \
             nothing followed.

",
            config.models.as_value()
        ));
    }

    // Which language each part of an issue is written in, when either row is
    // away from English.
    //
    // The same rule again, and this pair earned it the hard way. There was one
    // row, `Task body language`, and the only thing in the whole skill that
    // consulted it was one sentence of `analyst-issue-template.md` — the
    // `Description for dumb humans` callout. So the row was named for the body
    // and reached the summary: a setting widened by being renamed rather than
    // by anything reading it.
    //
    // Two rows now, and the template names both. Said here as well because the
    // template is one document read at one moment — filling a new finding —
    // and an issue's prose is written in more places than that.
    let defaults = Config::default();
    let summary = config.summary_language.as_str();
    let body = config.body_language.as_str();
    if summary != defaults.summary_language.as_str() || body != defaults.body_language.as_str() {
        lines.push_str(&format!(
            "Write the summary sentence at the top of an issue in **{summary}**, and the rest of \
             the body in **{body}**. They are separate rows because they are read by different \
             people: the summary is for somebody who will never read past it, and the body is for \
             whoever implements the thing. Headings are never translated \u{2014} `## Problem` is \
             `## Problem` in every installation, because the contract and its markers are written \
             against those exact words.

"
        ));
    }

    // What an `ask` row obliges, when one of them is set to it.
    //
    // The three authorisation rows accept `ask`, and `ask` accepts a duration.
    // What the duration *does* was written in a Rust doc comment on
    // `Authority::Ask` — "on expiry, records the proposed transition as a
    // comment on the issue instead of applying it. A run that dies leaves a
    // legible record rather than a state nobody wrote" — and nowhere the agent
    // reads. No Estigia code enforced it either, and nothing read the timeout.
    // So the operator could set `ask 30m`, be shown it on the screen and in the
    // table, and have it oblige nobody.
    //
    // One of the three is no longer in that state: `review_authority` reads
    // `Review delegation`'s timeout and stamps the deadline into the handoff
    // marker. It still obliges nobody to *wait* — Estigia does not sleep — but
    // the deadline is now a durable server-visible fact rather than a number
    // only the screen ever saw. Its sentence is written separately below,
    // because what that row asks for is a reviewer and not a transition, and a
    // row whose guidance describes the wrong action is guidance for a different
    // setting.
    //
    // This is the rule three lines above applied to a fourth axis: *a setting
    // the agent cannot act on is a setting that does nothing.*
    // Asked through `Authority::is_autonomous` rather than by matching the
    // variant here. That predicate answers exactly this question — *may this be
    // taken without a person present* — and it was called by nothing but its
    // own tests, all of which asserted it in the direction that says no. So it
    // could answer `false` for everything with the whole suite green, while the
    // one place that needed the answer spelled it out again, inverted.
    if [config.delivery, config.transitions]
        .iter()
        .any(|authority| !authority.is_autonomous())
    {
        lines.push_str(
            "A row set to `ask` means **propose and wait**, not decide. Where it carries a \
             duration (`ask 30m`), that is how long to wait for an answer before recording the \
             proposed transition as a comment on the issue **instead of applying it** \u{2014} so a run \
             that dies leaves a legible record rather than a state nobody wrote.

",
        );
    }
    if !config.review.is_autonomous() {
        lines.push_str(
            "`Review delegation` set to `ask` means **propose and wait** for permission to acquire \
             a reviewer \u{2014} not a workflow transition, and nothing here proposes one. Where it \
             carries a duration (`ask 30m`), that duration is recorded once, as the deadline on \
             the durable review handoff. Estigia does not sleep, schedule a wake-up, reset that \
             deadline when the request is retried, hold the issue until it passes, or read expiry \
             as a verdict: a review that nobody performed is still a review that nobody performed.

",
        );
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
/// What one file needed on the way to disk.
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// The file was not there and is written.
    Create,
    /// The file was there with different contents, and Estigia had written it
    /// before.
    Update,
    /// The file was there with different contents and Estigia did **not** put
    /// it there.
    ///
    /// Apart from [`Self::Update`] because they are different sentences said to
    /// a different person. Updating Estigia's own copy is bookkeeping. Writing
    /// over a file somebody else wrote is replacing their work with a version
    /// of it that is not theirs — and the round trip does not give it back:
    /// uninstall leaves the file, because it is not Estigia's to remove, so
    /// what is left standing is Estigia's copy under their name.
    ///
    /// Estigia installs upstream's skill under upstream's name, so this is the
    /// case an operator who already runs `issue-flow` walks into. Reported as
    /// `update`, the one word covered both, and `--dry-run` — the command whose
    /// whole job is to be believed before anything happens — said `update` for
    /// a file it was about to overwrite.
    ///
    /// **And this word covered two states in its turn**, which is the same
    /// defect one level in. A file the record claims, whose contents have moved
    /// since Estigia wrote them, is not a stranger's — it is Estigia's with
    /// somebody's edit in it, and it is [`Self::Overwrite`]. `sync` announced
    /// *"1 file(s) were already here and are not Estigia's"* about a file
    /// `estigia setup` had installed thirty seconds earlier, which sends an
    /// operator whose own edit has just been discarded looking for a file that
    /// was never there.
    Replace,
    /// The file is Estigia's, and somebody edited it since Estigia wrote it.
    ///
    /// Apart from [`Self::Replace`] for the reason `Replace` is apart from
    /// [`Self::Update`]: they are different sentences said to a different
    /// person. A stranger's file being written over is something the operator
    /// may not have known was there. Their own edit being discarded is work
    /// they have just lost — and `uninstall` already tells these two apart,
    /// keeping the edited file and naming it on its own line.
    ///
    /// `sync` is still allowed to write over it: bringing the payload up to the
    /// binary's copy is what `sync` is for. What it may not do is call the file
    /// somebody else's on the way past.
    Overwrite,
    /// The file was there and already correct.
    Unchanged,
    /// The file was there and Estigia took it away. Only ever a file in
    /// [`FILES`] — the inverse removes what it installed and nothing else.
    Remove,
    /// The file is the skill, and another configured agent still reads it.
    ///
    /// Eight of the eleven adapters have no skill directory of their own and
    /// share the neutral root. The skill goes out with the last of them, not
    /// the first.
    Shared,
    /// Estigia has no record of installing here, so nothing is shown to be
    /// its to remove — this file included, whoever wrote it.
    ///
    /// Apart from [`Change::Kept`] because the two are different sentences.
    /// "This was here before Estigia" is a fact a record can establish. "There
    /// is no record" is the absence of one, and saying the first while meaning
    /// the second tells an operator their files predate an install that in fact
    /// wrote them.
    Unrecorded,
    /// The file was there **before** Estigia, so uninstalling leaves it.
    ///
    /// Estigia installs upstream's skill under upstream's name, so a skill
    /// directory that already held `issue-flow` is the arrangement the design
    /// expects rather than an accident. Removing those files on the way out
    /// deleted somebody else's checkout.
    Kept,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// One file of the skill tree, and what happened to it.
pub struct FileAction {
    /// The absolute path.
    pub path: PathBuf,
    /// What it needed.
    pub change: Change,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// The outcome of writing or removing the skill tree.
pub struct SkillResult {
    /// The directory the skill tree lives in.
    pub root: PathBuf,
    /// Whether anything was actually written.
    pub dry_run: bool,
    /// One entry per file of the skill.
    pub actions: Vec<FileAction>,
}

impl SkillResult {
    /// How many files were, or would be, touched.
    pub fn changed_files(&self) -> usize {
        self.actions
            .iter()
            // `Kept` is a file nothing happened to. Counted as a change, an
            // uninstall over an existing checkout reported a dozen of them and
            // then left the directory exactly as it found it.
            .filter(|action| {
                !matches!(
                    action.change,
                    Change::Unchanged | Change::Kept | Change::Shared | Change::Unrecorded
                )
            })
            .count()
    }
}

/// What is actually at `root`.
///
/// Four states rather than two, because they are four different sentences and
/// only one of them was ever printed. An operator whose skill has never been
/// installed was told it was *out of date*, which describes a world where a file
/// exists and says something older than this binary. The command to run happens
/// to be the same, so nobody was sent to a dead end — but the line still
/// asserted a state it had not distinguished.
///
/// The fourth is that story with the harm the first one lacked. Comparing what
/// is installed against what this binary would write needs the operator's
/// configuration, and every caller read it with `unwrap_or_default()` — so a
/// table carrying one value this binary does not recognise was compared against
/// the *defaults*, did not match, and was reported as **out of date**. The state
/// that names leads to `estigia sync`, and `sync` refuses that contract with
/// `config-value-unrecognised` and changes nothing. Three audiences were told
/// it: `status`, its JSON, and the agent itself at every `SessionStart`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// Every file is there and matches what this binary would write.
    Current,
    /// Some of it is there, and some of it differs or is missing.
    Stale,
    /// None of it is there.
    Absent,
    /// The files are there and cannot be judged: the operator table carries a
    /// value this binary does not recognise, so there is nothing to compare
    /// against. Distinct from `Stale` because the remedy is the row, not `sync`.
    Unreadable,
}

/// What is at `root`, reading the operator's configuration first.
///
/// The one place allowed to answer "is the installed contract current": the
/// answer depends on a configuration that may not be readable, and a caller that
/// defaults it asks a question about a table nobody wrote. Every caller that
/// used to pair `installed_config(..).unwrap_or_default()` with [`presence`]
/// now comes through here.
pub fn presence_of(root: &Path) -> Presence {
    // Two questions, and both have to be asked.
    //
    // *Can the configuration be read at all* — the operator's override
    // included, because an unreadable one is not an absence of one and whatever
    // is underneath it was chosen by somebody else or by nobody. That is
    // `installed_config`, and its failure is what makes this `Unreadable`.
    //
    // *What should these files carry* — which is [`contract_config`], the
    // contract's own table and never an override. Layering the override into
    // this second question compared the installed `SKILL.md` against a value
    // the override exists to keep out of it, so using that file at all reported
    // the skill as out of date.
    match installed_config(root).and_then(|_| contract_config(root)) {
        Ok(config) => presence(root, &config),
        // Not `Absent`: something is installed, and saying otherwise would send
        // an operator to `setup` over a file that is already there.
        Err(_) if presence(root, &Config::default()) == Presence::Absent => Presence::Absent,
        Err(_) => Presence::Unreadable,
    }
}

/// Reads `root` and says which of the three it is.
pub fn presence(root: &Path, config: &Config) -> Presence {
    let mut matching = 0;
    let mut found = 0;
    for file in FILES {
        let Ok(text) = fs::read_to_string(root.join(relative(file.path))) else {
            continue;
        };
        found += 1;
        if normalize(&text) == normalize(&rendered(file, config)) {
            matching += 1;
        }
    }
    if matching == FILES.len() {
        Presence::Current
    } else if found == 0 {
        Presence::Absent
    } else {
        Presence::Stale
    }
}

/// Writes the skill under `root`, reporting what each file needed.
pub fn install(root: &Path, config: &Config, dry_run: bool) -> Result<SkillResult> {
    install_into(root, config, dry_run, &mut Pending::new())
}

/// What earlier steps of this run have already written — or, under `--dry-run`,
/// would have.
///
/// `None` records a file the run discarded, which is not the same as one that
/// was never there.
pub type Pending = std::collections::BTreeMap<PathBuf, Option<String>>;

/// [`install`], remembering what the rest of this run already did.
///
/// Eight of the eleven adapters write to the same neutral skill root. In a real
/// run the first creates those files and the other seven find them unchanged;
/// under `--dry-run` nothing is written, so without this each of the eight read
/// the untouched disk and planned to create all fifteen again. The plan said
/// 182 files where the run does 70 — and `--dry-run` is the one command whose
/// entire job is to be believed before anything happens.
pub fn install_into(
    root: &Path,
    config: &Config,
    dry_run: bool,
    pending: &mut Pending,
) -> Result<SkillResult> {
    // What Estigia put here on an earlier run. A file it wrote before is its
    // own to rewrite; one it did not is somebody else's.
    //
    // Only when there is a record to say so. Without one, "not in the record"
    // means nothing was written down — an install from a build that predates
    // the record, or one whose record somebody removed — and calling those
    // files somebody else's would tell an operator upgrading in the ordinary
    // way that Estigia was about to write over work of theirs. It was not, and
    // saying it is the same fault this crate keeps finding: an absence of
    // evidence read as evidence of absence.
    // Whether a file is somebody else's has three answers, not two, and only
    // two of them are knowable.
    //
    // With a record, it is knowable: the install wrote down what it created, so
    // a file that is not in it was not created here. Without one, there is a
    // second signal and it settles a different question — whether Estigia has
    // ever installed here at all. `<!-- estigia:config:start -->` is written by
    // Estigia and by nothing else; upstream's own block carries the
    // `issue-flow` name. A contract holding it belongs to a directory Estigia
    // has been in before, so the files around it are most likely its own from a
    // build that predates the record, and accusing an ordinary upgrade of
    // writing over somebody's work is the worse mistake.
    //
    // Neither one: Estigia has never been here, and everything already at these
    // paths is somebody else's.
    let recorded = record::exists(root);
    let created = record::created(root);
    // Read through `pending`, like everything else here. Eight adapters share
    // this root, and under `--dry-run` nothing reaches the disk — so an eighth
    // adapter asking the disk would get the answer the first adapter had, not
    // the one the run will have produced by then. It does not diverge today,
    // because the rows it would differ on come out `Unchanged` and go unprinted;
    // that is a coincidence of what is currently reported, not a property.
    let contract_path = root.join(relative(CONTRACT));
    let contract_now = match pending.get(&contract_path) {
        Some(remembered) => remembered.clone(),
        None => read_optional(&contract_path)?,
    };
    let installed_before =
        recorded || contract_now.is_some_and(|text| text.contains(crate::config::BLOCK_BEGIN));
    let mut actions = Vec::with_capacity(FILES.len());
    // What this run puts at each path, so the next one can tell an operator's
    // edit from a stale copy.
    let mut wrote_digests: Vec<(&'static str, String)> = Vec::new();
    for file in FILES {
        let path = root.join(relative(file.path));
        let desired = rendered(file, config);
        let existing = match pending.get(&path) {
            Some(remembered) => remembered.clone(),
            None => read_optional(&path)?,
        };
        let change = match existing.as_deref() {
            None => Change::Create,
            Some(found) if normalize(found) == normalize(&desired) => Change::Unchanged,
            // Proven Estigia's, or vouched for by a record that does not name
            // it, or written before there were records — in that order.
            //
            // And within "proven Estigia's", the question the record could not
            // answer until it held digests: *did you change this, or did an
            // older build write it*. A file whose contents are what this crate
            // last wrote there is stale and updating it is bookkeeping; one that
            // has moved since is a file with somebody's work in it, and writing
            // over that is `Replace` — the word this crate reserves for taking
            // somebody's text away, said loudly on purpose.
            //
            // A record with **no** digest for the path answers `Update`, not
            // `Replace`: every record written before this existed has none, and
            // reading absence as "edited" would announce a false alarm on every
            // installation there is.
            Some(found) if recorded => {
                // Two ways to be somebody else's: the record does not claim
                // the path at all, or it claims it and the contents have moved
                // since this crate last wrote them.
                let ours = created.contains(file.path)
                    && record::written(root, file.path)
                        .is_none_or(|last| last == record::digest_of(found));
                match (ours, created.contains(file.path)) {
                    (true, _) => Change::Update,
                    // Claimed by the record and changed since: theirs is the
                    // edit, not the file.
                    (false, true) => Change::Overwrite,
                    (false, false) => Change::Replace,
                }
            }
            Some(_) if installed_before => Change::Update,
            Some(_) => Change::Replace,
        };
        if !dry_run && !matches!(change, Change::Unchanged) {
            let parent = path
                .parent()
                .with_context(|| format!("{} has no parent directory", path.display()))?;
            fs::create_dir_all(parent)
                .with_context(|| format!("create directory {}", parent.display()))?;
            crate::paths::replace_atomically(&path, &desired)
                .with_context(|| format!("write {}", path.display()))?;
        }
        // Remembered whether or not it was written: under `--dry-run` this is
        // the whole point, and in a real run it agrees with the disk.
        if !dry_run && !matches!(change, Change::Unchanged) {
            wrote_digests.push((file.path, record::digest_of(&desired)));
        }
        pending.insert(path.clone(), Some(desired));
        actions.push(FileAction { path, change });
    }
    // What this crate used to ship and no longer does.
    //
    // `uninstall_from` walks `FILES`, so a path simply deleted from that list is
    // a file that stays on every machine that ever had it — invisible to
    // `status`, untouched by `uninstall`, and still there for anything that goes
    // looking. For the transport that is not untidiness: a stale
    // `scripts/github.py` beside a contract that no longer names it is the
    // second implementation coming back, and the whole cost of two
    // implementations is that they disagree in ways nobody is watching for.
    //
    // Removed **only** when the record says Estigia created it. A file somebody
    // else put at that path is theirs, and the same rule governs every other
    // removal this crate makes.
    actions.extend(retire(root, RETIRED, &created, pending, dry_run)?);

    // Named before it is written, and named under `--dry-run` too. It is a file
    // this run puts on the operator's disk, and a plan that leaves it out is a
    // plan that undercounts by the one file `uninstall` depends on.
    let made: Vec<&'static str> = actions
        .iter()
        .zip(FILES)
        .filter(|(action, _)| action.change == Change::Create)
        .map(|(_, file)| file.path)
        .collect();
    match record::note_would(root, made.iter().copied()) {
        record::Wrote::Nothing => {}
        record::Wrote::Created => actions.push(FileAction {
            path: record::path(root),
            change: Change::Create,
        }),
        _ => actions.push(FileAction {
            path: record::path(root),
            change: Change::Update,
        }),
    }
    if !dry_run {
        // Which of them were not there before, so the inverse can tell its own
        // files from the ones it wrote over. Nothing else can tell them apart
        // afterwards: both hold Estigia's text.
        // `made`, not a second walk of `actions`: the record's own line is in
        // there now, and zipping a list one longer than `FILES` works only
        // because `zip` stops at the shorter one — which is a thing to know
        // rather than a thing to rely on.
        record::note_created(root, made.iter().copied())?;
    }
    // What was written, beside what was created. Recorded on every real run and
    // not only on the first: the question it answers is about the *last* thing
    // this crate put there, and a record that stopped at the install would call
    // every file an upgrade touched somebody's work.
    if !dry_run && !wrote_digests.is_empty() {
        record::note_written(
            root,
            wrote_digests
                .iter()
                .map(|(path, digest)| (*path, digest.clone())),
        )?;
    }
    Ok(SkillResult {
        root: root.to_owned(),
        dry_run,
        actions,
    })
}

/// The exact inverse of [`install`]: removes the files it wrote, and nothing
/// else.
///
/// Invariant two of four. The skill directory is not deleted wholesale, even
/// when it looks like Estigia put everything in it — an operator's own note
/// dropped beside the references is theirs, and a tool that takes it away on
/// the way out is a tool nobody uninstalls twice. Directories are removed only
/// when they came out empty.
///
/// **"The files it wrote" means the files it created**, not the files it last
/// wrote to. Estigia installs upstream's skill under upstream's name so that a
/// directory already holding `issue-flow` is the same directory; over such a
/// checkout most of [`FILES`] is a file Estigia overwrote or found already
/// correct, and taking those out on the way back deleted the checkout. What was
/// created is read from `record`, and anything else is left where it is.
pub fn uninstall(root: &Path, dry_run: bool) -> Result<SkillResult> {
    uninstall_from(root, dry_run, &mut Pending::new())
}

/// [`uninstall`], remembering what the rest of this run already took out.
///
/// The mirror of [`install_into`], and it has the same fault without it: eight
/// adapters share a skill root, so under `--dry-run` each of the eight found
/// the files still on disk and planned to remove all fifteen again. The plan
/// said seventeen files for OpenCode where the run takes out three.
pub fn uninstall_from(root: &Path, dry_run: bool, pending: &mut Pending) -> Result<SkillResult> {
    let created = record::created(root);
    let recorded = record::exists(root);
    let mut actions = Vec::with_capacity(FILES.len());
    let mut taken: Vec<&'static str> = Vec::new();
    for file in FILES {
        let path = root.join(relative(file.path));
        // A file an earlier step of this run took out is gone, whether or not
        // the disk has caught up — which under `--dry-run` it never will.
        let present = match pending.get(&path) {
            Some(remembered) => remembered.is_some(),
            None => path.exists(),
        };
        // Absent a record, nothing under this root is Estigia's to remove.
        // That is the honest reading of an empty record and not a cautious one:
        // an install writes it, so a root with files and no record is a root
        // somebody else filled.
        // Ours to take, and **still** ours. The record claiming the path is the
        // first half; the second is that what is on disk is what this crate last
        // wrote there. A file Estigia created and somebody then edited is a file
        // with their work in it, and taking it away is taking that work — which
        // is the half the removal did not have until the record held digests.
        //
        // Kept rather than removed-and-marked, and for the reason the guard's
        // own refusal gives about a push hook: an uninstall that carries off
        // somebody's additions is the thing an operator notices about other
        // people's software and does not forgive. What is left is named on its
        // own line, so nothing is left silently.
        //
        // A record with no digest for the path answers "unchanged since": every
        // record written before digests existed has none, and reading that
        // absence as an edit would leave the whole installation behind on every
        // machine that upgraded into this.
        let edited = || {
            record::written(root, file.path).is_some_and(|last| {
                fs::read_to_string(&path).is_ok_and(|found| record::digest_of(&found) != last)
            })
        };
        let ours = created.contains(file.path);
        let change = match (present, ours) {
            (false, _) => Change::Unchanged,
            (true, false) if recorded => Change::Kept,
            (true, false) => Change::Unrecorded,
            (true, true) if edited() => Change::Kept,
            (true, true) => Change::Remove,
        };
        if change == Change::Remove && !dry_run {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
        if change == Change::Remove {
            taken.push(file.path);
            pending.insert(path.clone(), None);
        }
        actions.push(FileAction { path, change });
    }

    // And what this crate used to ship. `RETIRED` is not in `FILES`, so this
    // loop walks straight past it — and an uninstall that leaves part of the
    // application behind is the failure an operator notices about somebody
    // else's tool and never forgives. Measured before it was written: after
    // `uninstall`, the retired `scripts/github.py` was still there. The
    // directory holding it outlived that fix by several rounds — the sweep at
    // the end walks the parents of `FILES`, and a retired path is not in that
    // list — and `retire` takes it now, on the way past.
    //
    // Worse than untidy here. What is left is an executable copy of a transport
    // the contract no longer names, in a machine whose owner believes Estigia is
    // gone.
    for taken_away in retire(root, RETIRED, &created, pending, dry_run)? {
        if let Some(path) = RETIRED
            .iter()
            .find(|path| taken_away.path.ends_with(relative(path)))
        {
            taken.push(path);
        }
        actions.push(taken_away);
    }

    // Named on the way out as well. A file that appears without being announced
    // and disappears the same way is one an operator has no way to account for,
    // and this is the one that decides what the removal is allowed to touch.
    match record::forget_would(root, taken.iter().copied()) {
        record::Wrote::Nothing => {}
        record::Wrote::Removed => actions.push(FileAction {
            path: record::path(root),
            change: Change::Remove,
        }),
        _ => actions.push(FileAction {
            path: record::path(root),
            change: Change::Update,
        }),
    }
    if !dry_run {
        // The record goes first: it lives under the skill root, so a sweep
        // that ran before it found the directory still occupied and left the
        // whole tree standing.
        record::forget(root, taken.into_iter())?;
        remove_empty_directories(root);
    }
    Ok(SkillResult {
        root: root.to_owned(),
        dry_run,
        actions,
    })
}

/// Reports the skill as left where it is, and touches nothing.
///
/// For the adapter that is on its way out of a skill root somebody else is
/// still using. It is a report rather than a no-op so that the run says what it
/// did not do: an uninstall that silently skipped the skill would look like an
/// uninstall that had already been run.
pub fn keep_in(root: &Path) -> Result<SkillResult> {
    let actions = FILES
        .iter()
        .map(|file| {
            let path = root.join(relative(file.path));
            let change = if path.exists() {
                Change::Shared
            } else {
                Change::Unchanged
            };
            FileAction { path, change }
        })
        .collect();
    Ok(SkillResult {
        root: root.to_owned(),
        dry_run: true,
        actions,
    })
}

/// Removes the skill's directories, deepest first, and only while empty.
fn remove_empty_directories(root: &Path) {
    let mut directories = FILES
        .iter()
        .filter_map(|file| Path::new(file.path).parent())
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| root.join(parent))
        .collect::<Vec<_>>();
    directories.sort();
    directories.dedup();
    // Deepest first, so a nested empty directory does not keep its parent alive.
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        let _ = fs::remove_dir(directory);
    }
    let _ = fs::remove_dir(root);
    // The `skills/` directory above is deliberately left alone, even empty. It
    // is the *agent's* namespace, not a file Estigia created — the same reason
    // `~/.claude` is not ours to remove. What comes out is the tree we wrote.
}

/// Takes the directories above `file` that are now empty, up to `root`.
///
/// Empty is the whole of the test, and it is enough: a directory with anything
/// at all in it holds somebody's work or somebody else's, and neither is this
/// crate's to remove. `root` itself is left to the sweep that runs at the end
/// of an uninstall, which has the record to consult; this exists for the
/// removals that happen on the way *in*, where no sweep runs at all.
fn remove_if_empty(file: &Path, root: &Path) {
    let mut at = file.parent();
    while let Some(directory) = at {
        if directory == root || !directory.starts_with(root) {
            return;
        }
        if fs::remove_dir(directory).is_err() {
            return;
        }
        at = directory.parent();
    }
}

/// The operator's own override file beside the contract, when there is one.
///
/// Named rather than merely read, because a value that file shadows is a value
/// nobody can change from the contract — and a tool that says otherwise is
/// reporting an effect it did not have.
pub fn local_override(root: &Path) -> Option<PathBuf> {
    [crate::config::LOCAL_FILE, crate::config::LEGACY_LOCAL_FILE]
        .into_iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

/// The operator's overrides and the file they came from, or why there are none.
///
/// One reader for both ways the configuration is assembled. They were two, and
/// they agreed on the defect: `installed_config` chained
/// `read(new).or_else(|_| read(old)).ok()` and `installed_config_for` picked a
/// path with `is_file()` and then `.ok()`'d the read. Either way a file that is
/// **there and will not open** was indistinguishable from one that is not there.
///
/// What that cost, measured: with `estigia.local.md` unreadable the machine ran
/// `Merge strategy: merge commit` — the shipped default — while the operator's
/// file said `rebase`; and with an abandoned `operator.local.md` beside it, the
/// machine ran *that* one instead. Silently, in both cases: `doctor` said `ok
/// contract` and `status` said `configured`.
///
/// This is the file where an operator narrows what the tool may do, so falling
/// back is the one thing it must not do — *configuration may only tighten, and
/// an unreadable value must never become a looser default*. Absent is still
/// ordinary, and the older spelling is still read; it is only consulted when the
/// newer one is genuinely not there, rather than whenever reading it failed.
pub fn local_overrides(root: &Path) -> Result<Option<(PathBuf, String)>, Refusal> {
    for name in [crate::config::LOCAL_FILE, crate::config::LEGACY_LOCAL_FILE] {
        let path = root.join(name);
        if let Some(text) = override_text(&path)? {
            return Ok(Some((path, text)));
        }
    }
    Ok(None)
}

/// Where a repository keeps the rows that are about **it**.
///
/// Inside the repository's git directory, which is the answer to "created
/// automatically when it is used, and belonging to this checkout". It is not
/// versioned, so these rows are this clone's rather than the team's — the
/// operator chose that, and the consequence worth knowing is that a declared
/// boundary lives with whoever made it rather than with the repository.
pub fn repository_config_path(repo_dir: &Path) -> PathBuf {
    git_common_dir(repo_dir)
        .join("estigia")
        .join(crate::config::LOCAL_FILE)
}

/// The git directory shared by a checkout and every worktree linked to it.
///
/// Not `repo_dir/.git`, which is a **file** in a linked worktree. Joining onto
/// it gave a path no directory could be created under, and this crate's own
/// delivery topology works in worktrees: `config set --repo` in one refused
/// with *could not write … (os error 3)* and told the operator to run `estigia
/// doctor`, which fixes nothing about it; and, worse, the same repository
/// answered `Merge strategy = squash` in the checkout and `merge commit` inside
/// its own worktree, silently, because the file the worktree looked for could
/// not exist. A boundary declared for a repository must hold wherever that
/// repository is being worked on.
///
/// [`crate::harness::guard::hooks_directory`] had already learned this and asks
/// git; this reads the layout instead, because the answer is wanted once per
/// recorded repository on every `config repos` and must not fail.
///
/// The layout, which git documents and keeps stable: a linked worktree's `.git`
/// is a file reading `gitdir: <path>`, and that directory holds a `commondir`
/// file naming the shared one. A submodule's `.git` is the same kind of file
/// with no `commondir` beside it — and that is right, because a submodule is
/// its own repository and answers for itself.
///
/// Anything else — no `.git`, an unreadable one, a file that does not say
/// `gitdir:` — resolves to `repo_dir/.git`, which is where every ordinary
/// checkout keeps it and where every file written before this existed still is.
fn git_common_dir(repo_dir: &Path) -> PathBuf {
    let dot_git = repo_dir.join(".git");
    if dot_git.is_dir() {
        return dot_git;
    }
    let Some(gitdir) = fs::read_to_string(&dot_git)
        .ok()
        .and_then(|text| Some(text.trim().strip_prefix("gitdir:")?.trim().to_string()))
        .filter(|pointed| !pointed.is_empty())
        .map(|pointed| resolve_against(repo_dir, Path::new(&pointed)))
    else {
        return dot_git;
    };
    match fs::read_to_string(gitdir.join("commondir")) {
        Ok(common) if !common.trim().is_empty() => {
            resolve_against(&gitdir, Path::new(common.trim()))
        }
        _ => gitdir,
    }
}

/// `path` if it is absolute, and otherwise `base` with it appended — **and the
/// `..` in it taken out**.
///
/// Both pointers git writes here may be relative — `commondir` normally is
/// `../..` — and each is relative to the file that holds it, not to the
/// process's working directory, which is why this takes the base rather than
/// using [`fs`].
///
/// Joining alone gives a path that works and cannot be read: `config set
/// --repo` in a worktree answered *written to
/// `…/.git/worktrees/wt1/../../estigia/estigia.local.md`*, which is a correct
/// answer to "where is it" that nobody can follow. Taken out here rather than
/// with [`fs::canonicalize`], which needs the file to exist already — it is
/// being created — and which on Windows returns a `\\?\` prefix no operator
/// asked to read.
///
/// Textual, so a `..` that crosses a symlink would resolve differently than the
/// filesystem does. It is the right kind for these two pointers: git writes
/// them itself, as a plain step up out of a directory it created.
fn resolve_against(base: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut plain = PathBuf::new();
    for part in joined.components() {
        match part {
            // Only a step that has something to step out of. A leading `..`,
            // or one straight after a root, names a place `pop` cannot reach
            // and is kept as written rather than silently dropped.
            std::path::Component::ParentDir
                if plain
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, std::path::Component::Normal(_))) =>
            {
                plain.pop();
            }
            std::path::Component::CurDir => {}
            other => plain.push(other),
        }
    }
    plain
}

/// Where the list of repositories that answer for themselves is kept.
///
/// Beside the screen's language and the run pointers, because it is the same
/// kind of thing: something this machine knows, in no agent's contract.
pub fn known_repositories_path(home: &Path) -> PathBuf {
    home.join(".estigia").join("repositories")
}

/// The repositories that answer for themselves, **pruned as it is read**.
///
/// A checkout whose file has gone — deleted, or the whole clone thrown away —
/// is dropped rather than listed. A list that offers a repository which is not
/// configured any more is worse than no list: somebody picks it, gets the
/// contract's answers, and believes they are that repository's.
///
/// Never a refusal. This is a convenience for finding a checkout again, and a
/// machine with an unreadable list is a machine with none — which is exactly
/// how every machine starts.
pub fn known_repositories(home: &Path) -> Vec<PathBuf> {
    recorded_repositories(home)
        .into_iter()
        .filter(|path| repository_config_path(path).is_file())
        .collect()
}

/// Every checkout the list names, reachable or not.
///
/// The pruning above is what the *offer* is made from, and it is not what the
/// file is rewritten from. `remember_repository` read the pruned list, so
/// registering one checkout quietly dropped every other one that happened to be
/// unreachable at that moment — an unplugged drive, a share that was down — and
/// dropped it permanently, which no amount of plugging it back in undoes.
///
/// Taking a checkout off the list is [`crate::cli`]'s `config forget`, and it
/// says so by removing that checkout's own file. Nothing else may decide it.
fn recorded_repositories(home: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(known_repositories_path(home)) else {
        return Vec::new();
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        let path = PathBuf::from(line.trim());
        if line.trim().is_empty() || seen.contains(&path) {
            continue;
        }
        seen.push(path);
    }
    seen
}

/// Records that this checkout answers for itself, if it is not recorded.
///
/// Best effort, and deliberately: the answer is already in the repository's own
/// file, and a machine that cannot keep a convenience list has not lost
/// anything the gate reads. Failing the write here would refuse a setting that
/// was already set.
pub fn remember_repository(home: &Path, repo_dir: &Path) {
    let mut known = recorded_repositories(home);
    let repo_dir = repo_dir.to_path_buf();
    if known.contains(&repo_dir) {
        return;
    }
    known.push(repo_dir);
    let path = known_repositories_path(home);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body: String = known
        .iter()
        .map(|path| {
            format!(
                "{}
",
                path.display()
            )
        })
        .collect();
    let _ = crate::paths::replace_atomically(&path, &body);
}

/// That file's text, when a repository has one.
///
/// Absent is the ordinary case and answers `None`: a repository that has never
/// been configured is not a repository with a problem, and every install that
/// exists today has no such file. Unreadable is **not** absent — the same rule
/// `override_text` already keeps, for the same reason.
pub fn repository_rows(repo_dir: &Path) -> Result<Option<(PathBuf, String)>, Refusal> {
    let path = repository_config_path(repo_dir);
    Ok(override_text(&path)?.map(|text| (path, text)))
}

/// One immutable read of a repository override document.
#[derive(Debug, Clone)]
pub(crate) struct RepositorySnapshot {
    path: PathBuf,
    document: String,
}

/// Repository values and ownership derived together from one snapshot.
#[derive(Debug, Clone)]
pub(crate) struct RepositoryLayer {
    pub config: Config,
    pub settings: Vec<crate::config::Setting>,
}

impl RepositorySnapshot {
    /// Applies this exact document to one lower-layer baseline.
    pub(crate) fn layer_over(&self, base: &Config) -> Result<RepositoryLayer, Refusal> {
        let (config, settings) = in_file(
            &self.path,
            Config::read_scope_over(base, &self.document, crate::config::Scope::Everywhere),
        )?;
        Ok(RepositoryLayer { config, settings })
    }

    fn layer_over_keeping_what_parses(&self, base: &Config) -> RepositoryLayer {
        let (config, settings) = Config::read_scope_over_keeping_what_parses(
            base,
            &self.document,
            crate::config::Scope::Everywhere,
        );
        RepositoryLayer { config, settings }
    }
}

/// Reads a repository override exactly once for snapshot-bound layering.
pub(crate) fn repository_snapshot(repo_dir: &Path) -> Result<Option<RepositorySnapshot>, Refusal> {
    let snapshot =
        repository_rows(repo_dir)?.map(|(path, document)| RepositorySnapshot { path, document });
    #[cfg(test)]
    REPOSITORY_SNAPSHOT_REPLACEMENT.with(|replacement| {
        if let Some(document) = replacement.borrow_mut().take() {
            fs::write(repository_config_path(repo_dir), document)
                .expect("the injected repository replacement is writable");
        }
    });
    Ok(snapshot)
}

#[cfg(test)]
pub(crate) fn inject_repository_snapshot_replacement(document: String) {
    REPOSITORY_SNAPSHOT_REPLACEMENT.with(|replacement| *replacement.borrow_mut() = Some(document));
}

/// The exact repository-scoped rows this checkout's document owns.
pub fn repository_owned_settings(repo_dir: &Path) -> Result<Vec<crate::config::Setting>, Refusal> {
    repository_snapshot(repo_dir)?
        .map(|snapshot| {
            snapshot
                .layer_over(&Config::default())
                .map(|layer| layer.settings)
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// One override file: its text, nothing there, or why it could not be read.
///
/// Shared by the operator's file and by an agent's own, because they are one
/// question asked twice and both were asked with `.ok()`. A table that narrows
/// what the tool may do is the last place a read failure may pass for an
/// absence: whatever is underneath it was chosen by somebody else, or by nobody.
fn override_text(path: &Path) -> Result<Option<String>, Refusal> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Refusal::not_started(
            "config-local-unreadable",
            format!("{}: {error}", path.display()),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "that file readable, or moved aside if those overrides are no longer wanted \
                 \u{2014} until then the values in it cannot be honoured, and running on the \
                 ones underneath would be running on settings nobody chose",
            ),
        )),
    }
}

/// The legacy operator file, when a current one is there to shadow it.
///
/// `local_override` answers *which file is read*; this answers *which one is
/// not*. Both names exist so an installation that came from issue-flow keeps
/// working, and the moment somebody writes the newer name the older stops being
/// read — entirely, not row by row. Nothing said so.
pub fn shadowed_local(root: &Path) -> Option<String> {
    let older = root.join(crate::config::LEGACY_LOCAL_FILE);
    (root.join(crate::config::LOCAL_FILE).is_file() && older.is_file())
        .then(|| older.display().to_string())
}

/// The installed configuration, with what **this repository** says on top.
///
/// The layer the operator asked for: rows about the repository, kept in the
/// repository. It goes last and carries only the rows whose scope is the
/// repository, which is the same asymmetry the per-agent files already keep
/// from the other side — *a file may narrow what its own agent does, never
/// restate what the repository is*. A repository file restating an agent row
/// would be the mirror of that, and is put back the same way.
///
/// A repository with no such file reads exactly as it did before this existed.
/// That is deliberate: every install that exists today has none, and a layer
/// that changed their answers would be a migration rather than an addition.
pub fn installed_config_in(root: &Path, repo_dir: &Path) -> Result<Config, Refusal> {
    layer_repository(&installed_config(root)?, repo_dir)
}

/// How a document becomes a configuration: strict, or keeping what parses.
///
/// Threaded as a parameter rather than by writing the chain twice. The layering
/// rule — which document overrides which, and which rows a repository may speak
/// for — is the part that must exist once, and a second copy of it written to
/// get a tolerant read is exactly the shape this crate keeps paying for.
type Reader = fn(&str, Option<&str>) -> Result<Config, Refusal>;

/// The installed configuration, and what in it could not be read.
///
/// For the gate, which must not stop an agent mid-edit over a bad row and must
/// not silently lose the rows beside it either. `installed_config_in` answered
/// `Err` for the whole document, and the caller's only move was
/// [`Config::default`] — the loosest configuration there is. So a mistyped
/// `Renewal window` took the operator's declared boundaries with it, and
/// `terraform apply` went back to classifying as a routine write.
///
/// The strict chain runs first, so the refusal handed back is the same one every
/// other caller gets, naming the same file and row. Only when it fails does the
/// tolerant chain run — the identical chain, one function different.
pub fn installed_config_in_keeping_what_parses(
    root: &Path,
    repo_dir: &Path,
) -> (Config, Option<Refusal>) {
    match installed_config_in(root, repo_dir) {
        Ok(config) => (config, None),
        Err(refusal) => (
            installed_config_with(root, |versioned, local| {
                Ok(Config::read_keeping_what_parses(versioned, local))
            })
            .and_then(|base| layer_repository_keeping_what_parses(&base, repo_dir))
            .unwrap_or_default(),
            Some(refusal),
        ),
    }
}

/// One checkout's own rows, laid over what an agent answers with otherwise.
///
/// The rule this holds is *which rows a repository may speak for*: the ones
/// whose scope is the repository, and no others. An agent's own and this
/// machine's come back from `base` afterwards, so a repository file that names
/// one — by hand, or written by an older build — decides nothing.
///
/// One place, because it was three: this, the setup screen's opening read, and
/// the screen's reload. Two of them swallowed the unreadable case, which is the
/// difference between *this checkout says nothing* and *nobody knows what this
/// checkout says* — and a page that draws them the same has invented a verdict.
pub fn layer_repository(base: &Config, repo_dir: &Path) -> Result<Config, Refusal> {
    let Some(snapshot) = repository_snapshot(repo_dir)? else {
        return Ok(base.clone());
    };
    Ok(snapshot.layer_over(base)?.config)
}

fn layer_repository_keeping_what_parses(base: &Config, repo_dir: &Path) -> Result<Config, Refusal> {
    let Some(snapshot) = repository_snapshot(repo_dir)? else {
        return Ok(base.clone());
    };
    Ok(snapshot.layer_over_keeping_what_parses(base).config)
}

/// Reads the configuration currently installed under `root`.
pub fn installed_config(root: &Path) -> Result<Config, Refusal> {
    Ok(config_layers(root, None, None)?.unlayered().clone())
}

fn installed_config_with(root: &Path, read: Reader) -> Result<Config, Refusal> {
    let text = contract_text(root)?;
    let local = local_overrides(root)?;
    read(&text, local.as_ref().map(|(_, text)| text.as_str()))
}

fn contract_text(root: &Path) -> Result<String, Refusal> {
    contract_text_for(root, None)
}

/// The same, told which agent is being asked about.
///
/// The slug is not decoration: it is the difference between a command that
/// discharges the block and one that does not. Both sites used to name
/// `estigia install`, which is an alias for `setup` and refuses without an
/// agent — *`setup` needs to know which agent to configure* — so the one
/// instruction on the screen was a dead end, and the ratchet's one rule is that
/// a message may name a command only when running it clears the block.
fn contract_text_for(root: &Path, slug: Option<&str>) -> Result<String, Refusal> {
    let contract = root.join(CONTRACT);
    fs::read_to_string(&contract).map_err(|error| {
        Refusal::not_started(
            "skill-not-installed",
            format!("{}: {error}", contract.display()),
            match slug {
                Some(slug) => Resolution::run(format!("estigia setup {slug}")),
                None => Resolution::run("estigia setup --all"),
            },
        )
    })
}

/// The configuration the versioned block carries **on its own**.
///
/// Not [`installed_config`], and the difference is the whole point of the
/// override file: `estigia.local.md` changes what the tool does without
/// changing the table, so the two answers are *meant* to differ. Anything
/// asking "what does this contract say about itself" — is it the copy this
/// binary would write, and what should a rewrite of it carry — has to ask the
/// contract, or it compares a file against a value that file was never supposed
/// to hold.
///
/// Measured before this existed, on a fresh install and one override:
///
/// ```text
/// | Merge strategy | rebase |   in estigia.local.md
///
/// doctor : BROKEN skill — "not this binary's copy … the contract the agent
///          reads and the transport it runs may not be the ones this build was
///          tested against"
/// then   : "skill is not usable, so a run cannot swear yet"
/// ```
///
/// Nothing about the contract or the transport differed; the payload was this
/// binary's, byte for byte. What differed was a value the operator had put in
/// the one file the contract tells them to put it in. And the repair the message
/// named, `estigia sync`, discharged it by writing `rebase` **into the versioned
/// block** — the file that is committed and shared, under a note reading
/// *Configure the ignored local file, never this versioned block*. Deleting the
/// override afterwards did not undo it: the machine-local choice had become the
/// team's.
pub fn contract_config(root: &Path) -> Result<Config, Refusal> {
    Config::read(&contract_text(root)?, None)
}

/// The four configuration layers, retained at their actual read boundaries.
///
/// `agent`, `local`, and `repository` are cumulative snapshots after that
/// document was applied. `None` means the document is absent, not that it
/// repeated the layer below. Keeping absence prevents a save from manufacturing
/// an override merely because an effective value exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLayers {
    /// The portable configuration rendered in `SKILL.md`, on its own.
    pub contract: Config,
    /// The cumulative configuration after this adapter's own file.
    pub agent: Option<Config>,
    /// Agent-scoped settings explicitly named by this adapter's own file.
    pub agent_settings: Vec<crate::config::Setting>,
    /// The exact agent override document this layer was derived from.
    pub(crate) agent_document: Option<String>,
    /// The cumulative configuration after the operator's local file.
    pub local: Option<Config>,
    /// Settings explicitly named by the operator's local file.
    pub local_settings: Vec<crate::config::Setting>,
    /// The cumulative configuration after this repository's file.
    pub repository: Option<Config>,
    /// Repository-scoped settings explicitly named by this repository's file.
    pub repository_settings: Vec<crate::config::Setting>,
}

impl ConfigLayers {
    /// What the adapter reads before a repository says anything of its own.
    pub fn unlayered(&self) -> &Config {
        self.local
            .as_ref()
            .or(self.agent.as_ref())
            .unwrap_or(&self.contract)
    }

    /// What the adapter reads in the named repository.
    pub fn effective(&self) -> &Config {
        self.repository.as_ref().unwrap_or_else(|| self.unlayered())
    }

    /// Applies only the rows each override document explicitly owns over a new
    /// portable contract candidate.
    pub fn effective_over(&self, portable: &Config) -> Result<Config, Refusal> {
        let mut effective = portable.clone();
        if let Some(agent) = &self.agent {
            apply_settings(&mut effective, agent, &self.agent_settings)?;
        }
        if let Some(local) = &self.local {
            apply_settings(&mut effective, local, &self.local_settings)?;
        }
        if let Some(repository) = &self.repository {
            apply_settings(&mut effective, repository, &self.repository_settings)?;
        }
        Ok(effective)
    }
}

/// Applies selected typed rows from one configuration to another.
pub fn apply_settings(
    target: &mut Config,
    source: &Config,
    settings: &[crate::config::Setting],
) -> Result<(), Refusal> {
    for setting in settings {
        setting.apply(target, &setting.value_of(source))?;
    }
    Ok(())
}

/// Reads contract, adapter, operator, and repository layers without collapsing
/// their provenance.
pub fn config_layers(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
) -> Result<ConfigLayers, Refusal> {
    config_layers_with(root, slug, repo_dir, None)
}

/// Reads all existing layers while treating a genuinely absent contract as the
/// portable defaults an initial install will write.
pub fn config_layers_for_install(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
) -> Result<ConfigLayers, Refusal> {
    config_layers_with(root, slug, repo_dir, Some(&Config::default()))
}

fn config_layers_with(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
    absent_contract: Option<&Config>,
) -> Result<ConfigLayers, Refusal> {
    let contract_path = root.join(CONTRACT);
    let contract = if !contract_path.exists() {
        absent_contract.cloned().ok_or_else(|| {
            Refusal::not_started(
                "skill-not-installed",
                format!("{} does not exist", contract_path.display()),
                match slug {
                    Some(slug) => Resolution::run(format!("estigia setup {slug}")),
                    None => Resolution::run("estigia setup --all"),
                },
            )
        })?
    } else {
        let text = contract_text_for(root, slug)?;
        in_file(&contract_path, Config::read(&text, None))?
    };
    let mut agent_settings = Vec::new();
    let mut agent_document = None;
    let agent = match slug {
        Some(slug) => {
            let path = agent_override(root, slug);
            match override_text(&path)? {
                Some(document) => {
                    let (layered, settings) = in_file(
                        &path,
                        Config::read_scope_over(&contract, &document, crate::config::Scope::Agent),
                    )?;
                    agent_settings = settings;
                    agent_document = Some(document);
                    Some(layered)
                }
                None => None,
            }
        }
        None => None,
    };
    let before_local = agent.as_ref().unwrap_or(&contract);
    let mut local_settings = Vec::new();
    let local = match local_overrides(root)? {
        Some((path, document)) => {
            local_settings = explicit_settings(&document);
            Some(in_file(
                &path,
                Config::read(&before_local.render_rows(), Some(&document)),
            )?)
        }
        None => None,
    };
    let before_repository = local.as_ref().unwrap_or(before_local);
    let mut repository_settings = Vec::new();
    let repository = match repo_dir {
        Some(repo_dir) => match repository_snapshot(repo_dir)? {
            Some(snapshot) => {
                let layer = snapshot.layer_over(before_repository)?;
                repository_settings = layer.settings;
                Some(layer.config)
            }
            None => None,
        },
        None => None,
    };
    Ok(ConfigLayers {
        contract,
        agent,
        agent_settings,
        agent_document,
        local,
        local_settings,
        repository,
        repository_settings,
    })
}

fn explicit_settings(document: &str) -> Vec<crate::config::Setting> {
    crate::config::table_rows(document)
        .into_iter()
        .filter_map(|(label, _)| crate::config::Setting::from_label(&label))
        .fold(Vec::new(), |mut settings, setting| {
            if !settings.contains(&setting) {
                settings.push(setting);
            }
            settings
        })
}

/// Where one adapter's own table lives inside a shared skill root.
///
/// Eight of the ten adapters write to the same neutral root, so a single table
/// there is one table for all of them — and "claude-code runs Opus while
/// opencode runs Kimi" is unsayable. Per-adapter files make it sayable without
/// moving anybody's contract, which is the part that needs a real installation
/// to verify.
///
/// Named for the adapter, beside the contract, and read **after** the versioned
/// table so it overrides it row for row — the same shape `estigia.local.md`
/// already has, because an operator who understands one understands both.
pub fn agent_override(root: &Path, slug: &str) -> PathBuf {
    root.join(format!("estigia.{slug}.md"))
}

/// The rows in one adapter's own file that nothing will ever read.
///
/// A per-agent file may narrow what its own agent does; it may not restate what
/// the repository or machine says, and [`installed_config_for`] puts those rows
/// back from the contract. That is the right answer and it is a silent one: the
/// operator wrote a value, and every command reports the contract's.
///
/// `config set --agent` refuses to write such a row — `setting-not-per-agent`.
/// This is the same hazard arriving the other two ways it can: typed into the
/// file by hand, or written there by a build from before that row was a fact
/// about the repository. Reporting an effect that did not happen is the failure
/// the whole tool exists to refuse, so the silence is what needed fixing rather
/// than the behaviour.
pub fn overridden_rows(root: &Path, slug: &str) -> Vec<&'static str> {
    let Ok(text) = fs::read_to_string(agent_override(root, slug)) else {
        return Vec::new();
    };
    crate::config::table_rows(&text)
        .into_iter()
        .filter_map(|(label, _)| crate::config::Setting::from_label(&label))
        .filter(|setting| setting.scope() != crate::config::Scope::Agent)
        .map(crate::config::Setting::label)
        .collect()
}

/// Settings a hand-edited file names twice, and the file that names them.
///
/// [`Config::read`] applies rows in the order it meets them, so of two rows for
/// one setting the lower one wins. That is a rule no file states and no message
/// mentions: put the current label *above* the label it replaced and the dead
/// one is what holds. [`Setting::aliases`] makes it likelier and harder to see —
/// the two rows naming one setting need not look alike at all.
///
/// Only the documents a person writes. The contract's table is rendered from a
/// `Config`, which cannot hold one setting twice.
///
/// [`Setting::aliases`]: crate::config::Setting::aliases
pub fn duplicated_rows(root: &Path, slug: &str) -> Vec<(String, &'static str)> {
    [local_override(root), Some(agent_override(root, slug))]
        .into_iter()
        .flatten()
        .flat_map(|path| {
            let named = path.display().to_string();
            twice_named(&path)
                .into_iter()
                .map(move |setting| (named.clone(), setting))
        })
        .collect()
}

/// Rows in this adapter's documents read as less than the file says, with the
/// file each is in and what cut it.
///
/// A value carrying a cell separator or a line separator ends early, and the
/// row arrives truncated. `config set` refuses three such characters by name; a
/// file Estigia never writes has nothing refusing anything, and the loss was
/// silent — see [`crate::config::rows_split_by_a_separator`] for both
/// measurements.
pub fn rows_cut_short(root: &Path, slug: &str) -> Vec<(String, crate::config::CutShort)> {
    [
        Some(root.join(CONTRACT)),
        local_override(root),
        Some(agent_override(root, slug)),
    ]
    .into_iter()
    .flatten()
    .flat_map(|path| {
        let named = path.display().to_string();
        let text = fs::read_to_string(&path).unwrap_or_default();
        crate::config::rows_split_by_a_separator(&text)
            .into_iter()
            .map(move |cut| (named.clone(), cut))
    })
    .collect()
}

/// The settings one document names more than once, canonical label, in order.
fn twice_named(path: &Path) -> Vec<&'static str> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut seen: Vec<&'static str> = Vec::new();
    let mut twice: Vec<&'static str> = Vec::new();
    for (label, _) in crate::config::table_rows(&text) {
        let Some(setting) = crate::config::Setting::from_label(&label) else {
            continue;
        };
        let canonical = setting.label();
        if seen.contains(&canonical) {
            if !twice.contains(&canonical) {
                twice.push(canonical);
            }
        } else {
            seen.push(canonical);
        }
    }
    twice
}

/// [`installed_config`], narrowed to one adapter.
///
/// Precedence, least specific first: the versioned table, then this adapter's
/// own file, then `estigia.local.md`.
///
/// The operator's hand-written file stays the **last** word deliberately. It is
/// the one file Estigia never edits, and demoting it below a file Estigia
/// writes would mean `config set --agent` could silently overrule something
/// somebody typed by hand. `config set` already reports when that file shadows
/// a row, so the operator is told rather than surprised.
pub fn installed_config_for(root: &Path, slug: Option<&str>) -> Result<Config, Refusal> {
    Ok(config_layers(root, slug, None)?.unlayered().clone())
}

/// A writable per-agent view derived from one already-read override snapshot.
pub(crate) fn installed_config_for_agent_write(
    root: &Path,
    slug: &str,
    agent_document: Option<&str>,
) -> Result<Config, Refusal> {
    let contract_text = contract_text_for(root, Some(slug))?;
    let contract = Config::read_keeping_what_parses(&contract_text, None);
    let agent = agent_document.map_or_else(
        || contract.clone(),
        |document| {
            Config::read_scope_over_keeping_what_parses(
                &contract,
                document,
                crate::config::Scope::Agent,
            )
            .0
        },
    );
    Ok(match local_overrides(root)? {
        Some((_, document)) => {
            Config::read_keeping_what_parses(&agent.render_rows(), Some(&document))
        }
        None => agent,
    })
}

/// Says which file a bad row was in, without touching the rest of the refusal.
///
/// The code and the resolution are already right — `Config::read` names the
/// setting and what it accepts, and refuses to guess, which is the whole point
/// of that refusal. What it cannot know is the path, because it is handed text.
fn in_file<T>(path: &Path, read: Result<T, Refusal>) -> Result<T, Refusal> {
    read.map_err(|mut refusal| {
        refusal.message = format!("{} in {}", refusal.message, path.display());
        refusal
    })
}

/// A refusal for the case where the skill root cannot be decided.
pub fn no_skill_root() -> Refusal {
    Refusal::not_started(
        "skill-root-unknown",
        "no agent skill directory could be resolved",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "where this agent reads skills, passed as --skill-root <dir>",
        ),
    )
}

fn relative(path: &str) -> PathBuf {
    path.split('/').collect()
}

/// Compares content without arguing about line endings.
///
/// Git checkouts on Windows hand back CRLF for files written with LF. Treating
/// that as a difference would report every file as needing an update on every
/// run, which turns `status` into noise and `--dry-run` into a lie.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {

    /// A worktree answers with the repository it belongs to.
    ///
    /// The layout is built by hand rather than by git, so this measures on
    /// every machine and not only where git is installed —
    /// `a_worktree_answers_what_its_repository_answers` in `tests/pipe.rs`
    /// crosses the same thing against a real `git worktree add`.
    #[test]
    fn a_linked_worktree_keeps_its_answers_with_the_repository() {
        let outside = tempfile::tempdir().expect("somewhere to build a checkout");
        let repo = outside.path().join("main");
        let git_dir = repo.join(".git");
        let worktree = outside.path().join("wt");
        let linked = git_dir.join("worktrees").join("wt");
        std::fs::create_dir_all(&linked).expect("the linked worktree's git directory");
        std::fs::create_dir_all(&worktree).expect("the worktree");
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", linked.display()),
        )
        .expect("the pointer git writes in a linked worktree");
        // Relative, as git writes it, and relative to the directory holding it.
        std::fs::write(linked.join("commondir"), "../..\n").expect("the pointer to the shared one");

        let from_worktree = super::repository_config_path(&worktree);
        // Where it is, said in a way somebody can follow: joining the pointers
        // as git writes them gives `…/worktrees/wt/../../estigia/…`, which is
        // the answer `config set --repo` prints when it says what it wrote.
        assert!(
            !from_worktree
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir)),
            "the path an operator is shown steps back out of itself: {}",
            from_worktree.display()
        );
        assert_eq!(
            std::fs::canonicalize(from_worktree.parent().expect("a parent")).ok(),
            std::fs::canonicalize(
                super::repository_config_path(&repo)
                    .parent()
                    .expect("a parent")
            )
            .ok(),
            "a worktree looks for its answers somewhere the checkout does not keep them, so the \
             same repository answers two different ways depending on which directory you are in"
        );

        // And a submodule, whose `.git` is the same kind of file with no
        // `commondir` beside it, answers for itself rather than for its parent.
        let parent = outside.path().join("parent");
        let module = parent.join(".git").join("modules").join("child");
        let child = parent.join("child");
        std::fs::create_dir_all(&module).expect("the submodule's git directory");
        std::fs::create_dir_all(&child).expect("the submodule");
        std::fs::write(
            child.join(".git"),
            format!("gitdir: {}\n", module.display()),
        )
        .expect("the submodule's pointer");
        assert!(
            super::repository_config_path(&child).starts_with(&module),
            "a submodule was made to answer with its parent's rows: {}",
            super::repository_config_path(&child).display()
        );

        // The floor: an ordinary checkout is unmoved, or every file written
        // before this existed would have been left behind.
        std::fs::create_dir_all(&git_dir).expect("an ordinary git directory");
        assert!(
            super::repository_config_path(&repo).starts_with(&git_dir),
            "an ordinary checkout no longer keeps its answers under `.git`"
        );
    }

    /// Registering one checkout does not forget another that is merely away.
    ///
    /// The list is pruned as it is *read*, so an unreachable checkout is not
    /// offered — that part is right, and somebody who picked one would get the
    /// contract's answers believing they were that repository's. Rewriting the
    /// file from the pruned list is a different act: it drops the checkout
    /// permanently, and plugging the drive back in does not bring it back.
    ///
    /// Taking one off the list is `config forget`, which does it by removing
    /// that checkout's own file. Nothing else may decide it.
    #[test]
    fn recording_a_checkout_does_not_forget_one_that_is_only_away() {
        let home = tempfile::tempdir().expect("a temporary home");
        let outside = tempfile::tempdir().expect("somewhere to keep checkouts");
        let make = |name: &str| {
            let dir = outside.path().join(name);
            let file = repository_config_path(&dir);
            fs::create_dir_all(file.parent().expect("that file has a directory"))
                .expect("the checkout is made");
            fs::write(
                &file,
                "| Setting | Value here | Skill default |
",
            )
            .expect("its rows are written");
            dir
        };

        let away = make("away");
        remember_repository(home.path(), &away);
        // Gone from where it was — a drive unplugged, a share down. Not deleted.
        let moved = outside.path().join("away-elsewhere");
        fs::rename(&away, &moved).expect("the checkout is moved out of reach");
        assert!(
            !known_repositories(home.path()).contains(&away),
            "an unreachable checkout is being offered, which is the other failure"
        );

        // Registering something else, which is the act that used to drop it.
        remember_repository(home.path(), &make("here"));

        fs::rename(&moved, &away).expect("the checkout comes back");
        assert!(
            known_repositories(home.path()).contains(&away),
            "a checkout that was only away was forgotten by registering another"
        );
    }

    use super::*;

    #[test]
    fn a_skill_that_was_never_installed_is_absent_rather_than_out_of_date() {
        // The distinction `status` prints. Both send an operator to the same
        // command, so this was never a dead end — but a line that says a file is
        // out of date, about a directory with no files in it, is describing a
        // world that is not there.
        let root = tempfile::tempdir().expect("a temporary directory");
        let config = Config::default();
        assert_eq!(
            presence(&root.path().join("issue-flow"), &config),
            Presence::Absent
        );

        install(root.path(), &config, false).expect("the skill installs");
        assert_eq!(presence(root.path(), &config), Presence::Current);

        let first = root.path().join(relative(FILES[0].path));
        std::fs::write(&first, "something older").expect("age one file");
        assert_eq!(presence(root.path(), &config), Presence::Stale);

        // And one file left behind is still not a fresh machine: an operator
        // whose uninstall was interrupted must not read `not configured`.
        for file in &FILES[1..] {
            let _ = std::fs::remove_file(root.path().join(relative(file.path)));
        }
        assert_eq!(presence(root.path(), &config), Presence::Stale);
    }
    #[test]
    fn the_installed_directory_matches_the_declared_name() {
        assert_eq!(
            declared_name(),
            Some(DIRECTORY),
            "the skill directory and the frontmatter name must agree"
        );
    }

    #[test]
    fn the_skill_declares_a_version() {
        assert!(
            version().is_some(),
            "SKILL.md frontmatter must carry a version"
        );
    }

    /// Files that ship and that nothing in the payload names.
    ///
    /// A frozen baseline, and it may only ever **shrink** — the guard below
    /// fails on a new entry *and* on a stale one, so it cannot be padded and it
    /// cannot be left behind.
    ///
    /// - `references/migration-inventory.md` — it records where every section of
    ///   issue-flow's original single file went, and nothing an agent reads
    ///   links to it. Upstream reached the same conclusion and wrote the same
    ///   exception into its own suite, so this baseline matches theirs rather
    ///   than diverging from it. `examples/domain-test-coverage.md` stays out
    ///   entirely: a worked rule book a person copies into their own repository
    ///   is documentation, and upstream does not ship it in the payload either.
    const UNREFERENCED_BASELINE: &[&str] = &["references/migration-inventory.md"];

    /// Operations the contract requires that a binding does not map.
    ///
    /// Frozen, and it may only shrink. `SKILL.md` says every binding **MUST**
    /// map the eighteen operations it lists, and adds that *"bindings MUST
    /// declare unsupported capabilities and fail closed"* — so a gap here is
    /// either a missing row or an undeclared capability, and both are upstream's
    /// to settle. What is settled here is that the number cannot grow.
    ///
    /// - `bindings/github.md` / `label` — the GitHub binding maps seventeen of
    ///   the eighteen and never names `label`, neither as a row nor as a
    ///   declared gap. Linear and Trello both map it.
    const UNMAPPED_BASELINE: &[(&str, &str)] = &[("bindings/github.md", "label")];

    #[test]
    fn the_contract_still_names_the_operations_it_requires() {
        // The guard on the guard. If this line is reworded upstream, the seam
        // test below silently checks nothing.
        let required = super::required_operations();
        assert_eq!(
            required.len(),
            18,
            "the contract's MUST-map line changed shape: {required:?}"
        );
        assert!(required.contains(&"verify_claim".to_owned()));
        assert!(required.contains(&"close".to_owned()));
    }

    #[test]
    fn every_binding_maps_every_operation_the_contract_requires() {
        // Seam test: the contract against each binding that ships. An operation
        // the contract demands and no binding provides is a run that discovers
        // it mid-delivery.
        let required = super::required_operations();
        let mut gaps = Vec::new();
        for binding in FILES.iter().filter(|f| f.path.starts_with("bindings/")) {
            for operation in &required {
                if !binding.contents.contains(&format!("`{operation}`")) {
                    gaps.push((binding.path, operation.clone()));
                }
            }
        }

        for (binding, operation) in &gaps {
            assert!(
                UNMAPPED_BASELINE
                    .iter()
                    .any(|(known, name)| known == binding && name == operation),
                "{binding} does not map `{operation}`, which the contract requires. Map it, \
                 declare it unsupported, or the baseline is growing — and it may only shrink."
            );
        }
        for (binding, operation) in UNMAPPED_BASELINE {
            assert!(
                gaps.iter()
                    .any(|(path, name)| path == binding && name == operation),
                "{binding} now maps `{operation}`; take it out of the baseline so the guard \
                 keeps its grip"
            );
        }
    }

    #[test]
    fn every_file_the_payload_links_to_is_shipped() {
        // Seam test, the direction that bites at runtime: a reference the agent
        // is told to load and that is not installed is a dead end, and nothing
        // else would catch it.
        for file in FILES {
            for link in links_from(file) {
                assert!(
                    FILES.iter().any(|shipped| shipped.path == link),
                    "{} links to {link}, which is not installed",
                    file.path
                );
            }
        }
    }

    /// One file's links, resolved against its own directory.
    ///
    /// `references/repository-delivery.md` links to `../SKILL.md#hard-rules`;
    /// taken literally that is a path nothing ships, which is a false alarm the
    /// guard cannot afford — a seam test people learn to ignore is a seam test
    /// that has stopped working.
    fn links_from(file: &SkillFile) -> Vec<String> {
        let directory = Path::new(file.path).parent().unwrap_or(Path::new(""));
        markdown_link_targets(file.contents)
            .into_iter()
            .map(|target| {
                let target = target.split('#').next().unwrap_or_default();
                normalize_relative(directory, target)
            })
            .filter(|target| target.ends_with(".md"))
            .collect()
    }

    /// Joins a link to the directory it was written in, collapsing `..`.
    fn normalize_relative(directory: &Path, target: &str) -> String {
        let mut segments = directory
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        for segment in target.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    segments.pop();
                }
                other => segments.push(other.to_owned()),
            }
        }
        segments.join("/")
    }

    #[test]
    fn nothing_new_ships_that_the_payload_never_names() {
        // The other direction, ratcheted, minus the files a setting selects.
        // Those are not linked and must not be: a contract that linked every
        // methodology would be telling the agent to read three, and the seam
        // that matters for them is *the setting names a file that ships*, which
        // has a test each. See `SELECTED_BY_SETTING`.
        let unreferenced = FILES
            .iter()
            .filter(|file| {
                file.path != CONTRACT
                    && !SELECTED_BY_SETTING
                        .iter()
                        .any(|prefix| file.path.starts_with(prefix))
            })
            .filter(|file| {
                !FILES.iter().any(|other| {
                    other.path != file.path
                        && (links_from(other).contains(&file.path.to_owned())
                            || executables_named(other.contents).contains(&file.path.to_owned()))
                })
            })
            .map(|file| file.path)
            .collect::<Vec<_>>();

        for path in &unreferenced {
            assert!(
                UNREFERENCED_BASELINE.contains(path),
                "{path} ships and nothing names it. Either link it from the contract or drop \
                 it — the baseline may only shrink."
            );
        }
        for known in UNREFERENCED_BASELINE {
            assert!(
                unreferenced.contains(known),
                "{known} is in the baseline and is now referenced; take it out of the baseline \
                 so the guard keeps its grip"
            );
        }
    }

    #[test]
    fn every_file_the_payload_tells_an_agent_to_execute_is_shipped() {
        // The guard that was missing, and the reason it was missing is worth
        // keeping: `every_file_the_payload_links_to_is_shipped` reads markdown
        // links, and `bindings/github.md` does not *link* to its executable —
        // it prints the command line in a fence:
        //
        //     python <skill>/scripts/github.py --repo-dir <repo> <operation>
        //
        // A binding whose every operation runs through a file the installer
        // does not write is a dead end at the first tracker call, and the link
        // guard sails straight past it.
        for file in FILES {
            for named in executables_named(file.contents) {
                // A **retired** path is not a dead end: the prose that names one
                // is explaining what went away and why, which is worth keeping.
                // What must not survive is prose that still tells an agent to
                // run something nobody installs.
                if RETIRED.contains(&named.as_str()) {
                    continue;
                }
                assert!(
                    FILES.iter().any(|shipped| shipped.path == named),
                    "{} tells an agent to run {named}, which is not installed",
                    file.path
                );
            }
        }
    }

    /// Every `<skill>/…`-rooted path the payload hands to an interpreter.
    ///
    /// Deliberately narrow: it matches the `<skill>/` prefix the bindings use to
    /// mean "inside the installed directory", which is the only form that can
    /// name something the installer is responsible for. A bare `python foo.py`
    /// is the operator's own file and none of Estigia's business.
    fn executables_named(document: &str) -> Vec<String> {
        let mut named = Vec::new();
        let mut rest = document;
        while let Some(position) = rest.find("<skill>/") {
            rest = &rest[position + "<skill>/".len()..];
            let path = rest
                .split(|c: char| c.is_whitespace() || c == '`' || c == '"')
                .next()
                .unwrap_or_default();
            if !path.is_empty() {
                named.push(path.to_owned());
            }
        }
        named.sort_unstable();
        named.dedup();
        named
    }

    /// The `(path)` half of every markdown link in a document.
    fn markdown_link_targets(document: &str) -> Vec<String> {
        let mut targets = Vec::new();
        let mut rest = document;
        while let Some(open) = rest.find("](") {
            rest = &rest[open + 2..];
            let Some(close) = rest.find(')') else { break };
            let target = rest[..close].trim();
            if !target.is_empty() && !target.contains("://") {
                targets.push(target.to_owned());
            }
            rest = &rest[close + 1..];
        }
        targets
    }

    #[test]
    fn configuration_block_is_the_only_transformation() {
        let config = Config::default();
        for file in FILES {
            if file.path == CONTRACT {
                continue;
            }
            assert_eq!(
                rendered(file, &config),
                file.contents,
                "{} must be installed verbatim",
                file.path
            );
        }
    }

    #[test]
    fn the_installed_contract_names_the_override_file_estigia_actually_reads() {
        // Seam test: what `installed_config` opens against what the contract
        // tells the agent to open. issue-flow's block named `operator.local.md`;
        // Estigia reads `estigia.local.md` first. A contract that names the old
        // one, or names none at all, sends the agent to the wrong file — and
        // the agent has no other way to learn the mechanism exists.
        let rendered = rendered(&FILES[0], &Config::default());
        assert!(rendered.contains(crate::config::LOCAL_FILE));
        assert!(rendered.contains(crate::config::LEGACY_LOCAL_FILE));
    }

    #[test]
    fn one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it() {
        // `Config::read` applied each setting with `?`, so the first value it did
        // not recognise discarded the whole document — every row beside it
        // included, however well they read.
        //
        // That collateral is what made a declared gap look unavoidable. The
        // honesty contract said an unreadable contract costs the operator's
        // declared boundaries because "the list that made it a boundary is the
        // thing that went missing", and that holds only when the boundary row is
        // itself the bad one. Here it is not: `Irreversible commands` parses,
        // and a mistyped `Renewal window` three lines away threw it away. The
        // gate then classified `terraform apply` as a routine write, which never
        // reaches the phase question — the one place *configuration may only
        // tighten* runs backwards.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install succeeds");
        fs::write(
            skill.join(crate::config::LOCAL_FILE),
            "| Setting | Value here |
|---|---|
             | Irreversible commands | terraform apply |
             | Renewal window | 30 days |
",
        )
        .expect("write the override");
        let here = root.path();

        // The floor: this document really is one the strict read refuses, and
        // the row it refuses over is not the boundary row.
        let strict = installed_config_in(&skill, here);
        let refusal = strict.expect_err("the document parses, so this measures nothing");
        assert!(
            refusal.message.contains("Renewal window"),
            "the refusal is about the wrong row: {}",
            refusal.message
        );

        let (kept, complaint) = installed_config_in_keeping_what_parses(&skill, here);
        assert_eq!(
            kept.boundaries,
            vec!["terraform apply".to_owned()],
            "a row that parsed was thrown away with one that did not"
        );
        // And the complaint is the same one every other caller gets, so nothing
        // has become quieter: `doctor` still names the file and the row.
        assert_eq!(
            complaint.map(|refusal| refusal.message),
            Some(refusal.message),
            "the gate salvaged the rows and stopped saying anything was wrong"
        );

        // The window is *not* salvaged into something wider: `30 days` is refused
        // by its setter, so what is kept is the default, and the gate closes the
        // window itself whenever it had to salvage. Keeping what parses can only
        // narrow — falling back to `Config::default` is the loosest there is.
        assert_eq!(kept.window, crate::harness::RENEWAL_WINDOW);

        // And a document with nothing wrong in it still comes back through the
        // strict path, complaining about nothing.
        fs::write(
            skill.join(crate::config::LOCAL_FILE),
            "| Setting | Value here |
|---|---|
| Irreversible commands | terraform apply |
",
        )
        .expect("write the override");
        let (clean, quiet) = installed_config_in_keeping_what_parses(&skill, here);
        assert!(
            quiet.is_none(),
            "a document that parses was reported as broken"
        );
        assert_eq!(clean.boundaries, vec!["terraform apply".to_owned()]);
    }

    #[test]
    fn an_override_beside_the_contract_wins_and_the_legacy_name_still_works() {
        for name in [crate::config::LOCAL_FILE, crate::config::LEGACY_LOCAL_FILE] {
            let root = tempfile::tempdir().expect("a temporary root");
            let skill = root.path().join(DIRECTORY);
            install(&skill, &Config::default(), false).expect("install succeeds");
            fs::write(
                skill.join(name),
                "| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Merge strategy | rebase | merge commit |\n",
            )
            .expect("write the override");

            assert_eq!(
                installed_config(&skill).expect("the config reads").merge,
                crate::config::MergeStrategy::Rebase,
                "{name} beside the contract did not override it"
            );
        }
    }

    /// What this crate used to ship is taken off a machine that has it.
    ///
    /// The mirror of the test this replaced, which asserted the transport was
    /// **installed** beside the binding that ran it. Nothing runs it now — the
    /// operations are answered in process and the binding names Estigia's tools
    /// — so what has to be true is the opposite, and it is the harder half: a
    /// path dropped from `FILES` is a file nobody removes, left in a directory
    /// the contract no longer mentions for anything that goes looking to find
    /// and run.
    #[test]
    fn the_retired_transport_is_taken_off_a_machine_that_has_it() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install succeeds");

        let transport = skill.join("scripts").join("github.py");
        assert!(
            !transport.is_file(),
            "a fresh install still writes {}",
            transport.display()
        );

        // And the case that matters: a machine that already had it. The file is
        // put back **and named in the record**, which is the only thing that
        // makes it this crate's to remove.
        std::fs::create_dir_all(transport.parent().expect("a directory")).expect("the directory");
        std::fs::write(
            &transport,
            "print('the old transport')
",
        )
        .expect("the file");
        record::note_created(&skill, std::iter::once("scripts/github.py")).expect("the record");

        install(&skill, &Config::default(), false).expect("the next install succeeds");
        assert!(
            !transport.is_file(),
            "{} survived an install that retires it",
            transport.display()
        );
    }

    #[test]
    fn the_one_way_doors_this_repository_declared_are_named_where_the_agent_reads() {
        // The contract asks the agent to renew "at every expensive or
        // irreversible boundary" — a judgement. The operator's list is the part
        // it cannot derive: `terraform apply` is a one-way door *here*, and
        // nowhere in the sentence that asks for the judgement.
        //
        // The gate enforces the list either way, so this is not about letting
        // something through. It is about which of the two ways the agent finds
        // out: reading it, or being stopped.
        let mut declared = Config::default();
        crate::config::Setting::Boundaries
            .apply(&mut declared, "terraform apply, npm publish")
            .expect("a list the parser accepts");

        let body = configuration_body(&declared);
        assert!(
            body.contains("terraform apply") && body.contains("npm publish"),
            "the doors this repository declared are not named where the agent reads"
        );
        // On top of, not instead of: the built-in boundaries are not removable,
        // and a sentence that read like a replacement would say the opposite of
        // what the setting is.
        assert!(
            body.contains("on top of"),
            "the declared list reads as though it replaced the contract's own"
        );

        // A repository that declared none says nothing: the general rule is
        // already in the contract, and repeating it is the noise this block has
        // no room for.
        assert!(
            !configuration_body(&Config::default()).contains("one-way doors, on top"),
            "a repository with no declared boundaries is told about them anyway"
        );
    }

    #[test]
    fn a_repository_that_lands_on_trunk_says_so_before_the_refusal_does() {
        // The word `trunk` appears nowhere in the skill — not once, in any file
        // the agent reads. So an agent on a trunk repository took the branch
        // route it was taught, reached the delivering write, and met
        // `unflagged-on-trunk`: told to name a flag it had never been told this
        // repository runs on, at the most expensive moment to learn it.
        //
        // The gate refuses either way. What this changes is whether being
        // refused is how the agent finds out.
        let trunk = Config {
            integration: crate::config::Integration::Trunk,
            ..Config::default()
        };
        let body = configuration_body(&trunk);
        assert!(
            body.contains("lands on **trunk**"),
            "a trunk repository does not tell the agent where work lands"
        );
        // And names the mechanism the refusal will ask for, in the same words.
        assert!(
            body.contains("ESTIGIA_FLAG"),
            "it says work lands early and not what makes that allowed"
        );
        // Without overstating it: naming a flag is a claim, not a proof, and
        // `out_of_phase` says so where it accepts one.
        assert!(
            body.contains("not proof"),
            "it offers the flag as though Estigia could check it"
        );

        // A branch repository is the skill's own assumption and needs no
        // sentence: a block regenerated on every run has no room for prose
        // about the arrangement it already describes.
        assert!(
            !configuration_body(&Config::default()).contains("lands on **trunk**"),
            "a branch repository is told about trunk anyway"
        );
    }

    #[test]
    fn a_named_model_routing_reaches_the_agent_that_has_to_honour_it() {
        // `Model routing` says "which model each delegated role and phase runs
        // on". Only the agent can honour that, and the skill it reads mentioned
        // models four times — all of them incidental ("continuation model",
        // "state model", an incident narrative). Nothing in Estigia reads it
        // either: `ModelRouting` offers lookups by role, by phase and by state
        // and has no caller outside the configuration module.
        //
        // So an operator could set `implementer=opus`, watch the table record
        // it, and have it oblige nobody — the same fault as the `ask` row
        // above, one row over and further gone, because `analyst=opus` carries
        // no plain meaning to a reader who was never told the row exists.
        let mut routed = Config::default();
        crate::config::Setting::Models
            .apply(&mut routed, "implementer=opus, reviewer=sonnet")
            .expect("a routing the parser accepts");

        let body = configuration_body(&routed);
        assert!(
            body.contains("Delegated work here runs on named models"),
            "a contract with a routing does not tell the agent to honour it"
        );
        // With the value in it: a sentence about a routing that does not say
        // which one sends the reader back to the table to guess.
        assert!(
            body.contains("implementer=opus"),
            "the sentence does not name the routing it is about"
        );

        // And an unset routing says nothing, because there is nothing to say.
        assert!(
            !configuration_body(&Config::default())
                .contains("Delegated work here runs on named models"),
            "a contract with no routing explains routing anyway"
        );
    }

    #[test]
    fn a_row_set_to_ask_tells_the_agent_what_asking_obliges() {
        // `selected_documents` states the rule this checks: *a setting the
        // agent cannot act on is a setting that does nothing.* `ask` accepts a
        // duration, and what the duration does — wait, then record the proposed
        // transition as a comment instead of applying it — lived in a doc
        // comment on `Authority::Ask` and nowhere the agent reads. Nothing in
        // Estigia enforced it either, and nothing read the timeout. So
        // `ask 30m` was a value the screen offered, the table recorded, and
        // nobody was obliged by.
        //
        // `Review delegation` is now written separately, because what it asks
        // permission for is a **reviewer** and the shared sentence promised a
        // proposed transition. Guidance describing the wrong action is guidance
        // for a different setting, and an agent that believed it would wait for
        // a transition Estigia never proposes.
        let asking = Config::default();
        assert!(
            matches!(asking.delivery, crate::config::Authority::Ask { .. }),
            "the default carries an `ask`, which is what makes this the common case"
        );
        let body = configuration_body(&asking);
        assert!(
            body.contains("propose and wait"),
            "a contract with an `ask` row does not say what asking obliges"
        );
        assert!(
            body.contains("instead of applying it"),
            "it says to wait and not what the waiting ends in"
        );

        // The review row asks for a reviewer, and says so in its own words.
        let reviewing = Config {
            delivery: crate::config::Authority::Auto,
            transitions: crate::config::Authority::Auto,
            review: crate::config::Authority::Ask {
                timeout: std::time::Duration::from_secs(30 * 60),
            },
            ..Config::default()
        };
        let body = configuration_body(&reviewing);
        assert!(
            body.contains("permission to acquire a reviewer"),
            "the review row is explained as though it proposed a transition"
        );
        assert!(
            !body.contains("proposed transition"),
            "the review row still carries the transition wording it does not do"
        );
        assert!(
            body.contains("read expiry as a verdict"),
            "the review row does not say what its deadline is not"
        );

        // And a machine that decides on its own is not told about a rule it
        // will never meet: noise in a block regenerated on every run is noise
        // nobody can delete.
        let deciding = Config {
            delivery: crate::config::Authority::Auto,
            review: crate::config::Authority::Auto,
            transitions: crate::config::Authority::Auto,
            ..Config::default()
        };
        assert!(
            !configuration_body(&deciding).contains("propose and wait"),
            "a contract with no `ask` row explains asking anyway"
        );
    }

    #[test]
    fn the_two_language_rows_are_two_rows_everywhere_they_are_read() {
        // There was one row, `Task body language`, and the only thing in the
        // whole skill that consulted it was one sentence of the analyst
        // template — the `Description for dumb humans` callout. So the row was
        // named for the body and reached the summary: a setting widened by
        // being renamed rather than by anything reading it. The split is only
        // real if **both** rows reach the agent, so both are crossed here
        // against the two places that carry them.
        let template = include_str!("../skill/assets/analyst-issue-template.md");
        for setting in [
            crate::config::Setting::Summary,
            crate::config::Setting::Body,
        ] {
            assert!(
                template.contains(setting.label()),
                "`{}` is a row nothing in the template names, so nothing writes prose in it",
                setting.label()
            );
        }

        // Different answers, so a contract that carried one of them twice — or
        // collapsed them back into one — cannot pass.
        let split = Config {
            summary_language: crate::config::Language::parse("Español").expect("a language"),
            body_language: crate::config::Language::parse("Deutsch").expect("a language"),
            ..Config::default()
        };
        let body = configuration_body(&split);
        assert!(
            body.contains("in **Español**") && body.contains("in **Deutsch**"),
            "the contract does not carry both languages: {body}"
        );
        // And the headings are named as the thing that never moves. They are
        // the contract's own vocabulary and the machine marker's neighbours.
        assert!(
            body.contains("Headings are never translated"),
            "nothing stops an agent translating `## Problem` along with the prose"
        );

        // A repository that writes both in English is not told about a rule it
        // will never meet: noise in a block regenerated on every run is noise
        // nobody can delete.
        assert!(
            !configuration_body(&Config::default()).contains("Write the summary sentence"),
            "a contract at the default explains the languages anyway"
        );
        // But moving either one alone is enough to say it, or the row that was
        // moved would be the one that goes unmentioned.
        for moved in [
            Config {
                summary_language: crate::config::Language::parse("Español").expect("a language"),
                ..Config::default()
            },
            Config {
                body_language: crate::config::Language::parse("Español").expect("a language"),
                ..Config::default()
            },
        ] {
            assert!(
                configuration_body(&moved).contains("Write the summary sentence"),
                "one row moved away from the default and the contract said nothing"
            );
        }
    }

    #[test]
    fn the_contract_carries_the_resolved_configuration() {
        let rendered = rendered(&FILES[0], &Config::default());
        assert_eq!(FILES[0].path, CONTRACT);
        assert!(rendered.contains(crate::config::BLOCK_BEGIN));
        assert!(!rendered.contains("issue-flow:config:start"));
        assert!(rendered.contains("| Merge strategy | merge commit | merge commit |"));
        // Everything above the block is untouched contract.
        assert!(rendered.starts_with(&format!("---\nname: {DIRECTORY}")));
    }

    #[test]
    fn uninstalling_over_somebody_else_s_checkout_leaves_the_checkout() {
        // Found by running it: `estigia setup agents` on a machine whose
        // `~/.agents/skills/issue-flow` was a real issue-flow clone, then
        // `--uninstall`. It deleted `SKILL.md`, `bindings/`, `references/`,
        // `assets/` and `scripts/` — the clone. Estigia had created none of
        // them: it had overwritten two and found the rest already correct, and
        // afterwards no two of those cases can be told apart from the disk,
        // because all of them then hold Estigia's text.
        //
        // This is the arrangement Estigia is built for, not a mishap. It
        // installs upstream's skill under upstream's name so the two are one
        // directory.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        let theirs = skill.join("references");
        fs::create_dir_all(&theirs).expect("their directory");

        // Two shapes of pre-existing file: one Estigia will overwrite, and one
        // it will find already exactly right. Both were somebody else's.
        let overwritten = skill.join(relative(CONTRACT));
        fs::write(&overwritten, "# their contract\n").expect("their contract");
        let already_right = FILES
            .iter()
            .find(|file| file.path.starts_with("references/"))
            .expect("the skill ships a reference");
        let untouched = skill.join(relative(already_right.path));
        fs::write(&untouched, already_right.contents).expect("their reference");

        let installed = install(&skill, &Config::default(), false).expect("install");
        assert_eq!(
            installed
                .actions
                .iter()
                .filter(|action| action.change == Change::Create)
                .count(),
            // Plus the record itself, which this install creates and names.
            FILES.len() - 2 + 1,
            "the two that were already there were not created"
        );

        let removed = uninstall(&skill, false).expect("uninstall");
        assert!(
            overwritten.is_file(),
            "uninstall deleted a file that was here before Estigia overwrote it"
        );
        assert!(
            untouched.is_file(),
            "uninstall deleted a file Estigia never even changed"
        );
        assert_eq!(
            removed
                .actions
                .iter()
                .filter(|action| action.change == Change::Kept)
                .count(),
            2,
            "and says so, rather than leaving them behind silently"
        );
        // Everything Estigia did create is gone, so a second uninstall has
        // nothing to do and the directory is theirs again.
        for file in FILES {
            let path = skill.join(relative(file.path));
            if path == overwritten || path == untouched {
                continue;
            }
            assert!(!path.exists(), "{} outlived the uninstall", path.display());
        }
        assert!(
            !skill.join(".estigia").exists(),
            "the record outlived what it was recording"
        );
    }

    #[test]
    fn a_missing_record_and_a_file_that_predates_estigia_are_not_the_same_answer() {
        // Both leave the file alone, and only one of them is a fact. "This was
        // here before Estigia" is something a record establishes; "there is no
        // record" is the absence of one. Reported as the first, an operator
        // whose record had been deleted was told their files predated an
        // install that had in fact written every one of them.
        let root = tempfile::tempdir().expect("a temporary root");

        // A record that exists and does not name this file.
        let theirs = root.path().join("theirs");
        fs::create_dir_all(&theirs).expect("their directory");
        fs::write(theirs.join(relative(CONTRACT)), "# theirs\n").expect("their contract");
        install(&theirs, &Config::default(), false).expect("install");
        let over = uninstall(&theirs, false).expect("uninstall");
        assert_eq!(
            over.actions
                .iter()
                .filter(|action| action.change == Change::Kept)
                .count(),
            1,
            "the one file that was here first is the one that was here first"
        );
        assert!(
            !over
                .actions
                .iter()
                .any(|action| action.change == Change::Unrecorded),
            "a record was written, so nothing here is unaccounted for"
        );

        // No record at all.
        let ours = root.path().join("ours");
        install(&ours, &Config::default(), false).expect("install");
        fs::remove_dir_all(ours.join(".estigia")).expect("forget the record");
        let blind = uninstall(&ours, false).expect("uninstall");
        assert_eq!(
            blind
                .actions
                .iter()
                .filter(|action| action.change == Change::Unrecorded)
                .count(),
            FILES.len(),
            "without a record nothing here can be shown to be Estigia's"
        );
        assert!(
            !blind
                .actions
                .iter()
                .any(|action| action.change == Change::Kept),
            "nothing here predates Estigia, and saying so would be an invention"
        );
    }

    #[test]
    fn an_install_with_no_record_of_itself_removes_nothing() {
        // The record is how uninstall knows its own files. Absent one — a
        // directory somebody else's tool filled, or one installed by a build
        // that predates the record — the honest answer is that Estigia cannot
        // show it wrote any of this, and a tool whose subject is authority does
        // not delete on a guess.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install");
        fs::remove_dir_all(skill.join(".estigia")).expect("forget the record");

        let removed = uninstall(&skill, false).expect("uninstall");
        assert_eq!(removed.changed_files(), 0, "nothing was Estigia's to take");
        for file in FILES {
            assert!(
                skill.join(relative(file.path)).exists(),
                "{} was removed without a record saying Estigia wrote it",
                file.path
            );
        }
    }

    #[test]
    fn a_row_nobody_can_read_names_the_file_it_is_in() {
        // Three files can carry a table under one skill root, and only one of
        // them is Estigia\'s to fix — it will not edit the operator\'s. Refusing
        // with `Planning is "waterfall"` and no path left them opening all
        // three to find which one they had typed it into.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install");

        let bad = format!(
            "{}\n| Setting | Value here |\n|---|---|\n| Planning | waterfall |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        );
        for path in [
            agent_override(&skill, "codex"),
            skill.join(crate::config::LOCAL_FILE),
        ] {
            fs::write(&path, &bad).expect("a file with a row nobody can read");
            let refusal = installed_config_for(&skill, Some("codex"))
                .expect_err("`waterfall` is not a planning protocol");
            assert_eq!(refusal.code, "config-value-unrecognised");
            let message = format!("{refusal}");
            assert!(
                message.contains(&path.display().to_string()),
                "the refusal does not say which file: {message}"
            );
            fs::remove_file(&path).expect("clean up between the two");
        }
    }

    #[test]
    fn one_setting_named_twice_in_one_file_is_countable() {
        // Rows apply in the order they are met, so of two rows for one setting
        // the lower one holds. Measured, not assumed: with the current label
        // written *above* the label it replaced, the dead label is what every
        // command reports — position wins, not canonicity, and nothing said so.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install");
        let local = skill.join(crate::config::LOCAL_FILE);
        let alias = crate::config::Setting::Summary.aliases()[1];
        let header = "| Setting | Value |\n|---|---|\n";

        // Named once, however spelled: nothing to report.
        fs::write(&local, format!("{header}| {alias} | A |\n")).expect("their file");
        assert_eq!(duplicated_rows(&skill, "codex"), Vec::new());

        // Named twice, and the file's line order is what decides which holds.
        fs::write(
            &local,
            format!("{header}| Summary language | B |\n| {alias} | A |\n"),
        )
        .expect("their file");
        assert_eq!(
            duplicated_rows(&skill, "codex"),
            vec![(local.display().to_string(), "Summary language")],
            "one setting named twice is not counted"
        );

        // And the value in force is the lower row — which is what makes the
        // silence worth breaking rather than a note about tidiness.
        let held = installed_config_for(&skill, Some("codex")).expect("a configuration");
        assert_eq!(held.summary_language.as_str(), "A");
    }

    #[test]
    fn an_operator_file_the_newer_name_killed_is_countable() {
        // Both names exist so an installation that came from issue-flow keeps
        // working. The moment somebody writes `estigia.local.md`, the older
        // file stops being read — entirely, not row by row — and nothing said
        // so. The contract even promised the opposite: "`operator.local.md` is
        // still read where it exists", which is false in exactly the migration
        // it was written for.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install");

        let older = skill.join(crate::config::LEGACY_LOCAL_FILE);
        let newer = skill.join(crate::config::LOCAL_FILE);

        // Neither, and the older alone: nothing to report. An installation that
        // never migrated is working exactly as it should.
        assert_eq!(shadowed_local(&skill), None);
        fs::write(&older, "# theirs\n").expect("their older file");
        assert_eq!(
            shadowed_local(&skill),
            None,
            "the older file alone is the one being read"
        );

        // Both: the older one is dead and this is the only thing that says it.
        fs::write(&newer, "# theirs\n").expect("their newer file");
        assert_eq!(
            shadowed_local(&skill).as_deref(),
            Some(older.display().to_string().as_str()),
            "the file that stopped being read is not named"
        );

        // And `local_override` still answers the other half — which one *is*
        // read — so the two agree about which is which.
        assert_eq!(local_override(&skill), Some(newer));
    }

    #[test]
    fn a_row_an_agent_s_own_file_writes_in_vain_is_countable() {
        // The behaviour is right and it is silent: a per-agent file may narrow
        // what its own agent does and may not restate what the repository is,
        // so those rows are put back from the contract. The operator wrote a
        // value and every command answers a different one.
        //
        // `config set --agent` refuses to write such a row. It arrives the
        // other two ways anyway — typed by hand, or written by a build from
        // before that row belonged to the repository — and reporting an effect
        // that did not happen is the failure this whole tool exists to refuse.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install");

        let repository = crate::config::SETTINGS
            .iter()
            .find(|setting| setting.scope() == crate::config::Scope::Everywhere)
            .expect("some setting belongs to the repository");
        let theirs = crate::config::SETTINGS
            .iter()
            .find(|setting| setting.scope() == crate::config::Scope::Agent)
            .expect("some setting differs by agent");
        let machine = crate::config::SETTINGS
            .iter()
            .find(|setting| setting.scope() == crate::config::Scope::Machine)
            .expect("some setting belongs to the machine");

        // A file with one of each: only its own agent row is in force.
        fs::write(
            agent_override(&skill, "opencode"),
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | {} | {} | x |\n| {} | {} | x |\n| {} | {} | x |\n{}\n",
                CONFIG_FENCE.begin,
                repository.label(),
                repository.value_of(&Config::default()),
                machine.label(),
                machine.value_of(&Config::default()),
                theirs.label(),
                theirs.value_of(&Config::default()),
                CONFIG_FENCE.end
            ),
        )
        .expect("their file");

        let ignored = overridden_rows(&skill, "opencode");
        assert_eq!(
            ignored,
            vec![repository.label(), machine.label()],
            "the rows nothing reads are exactly the non-agent rows"
        );

        // No file at all, and a file with nothing of the repository's in it,
        // both count nothing — an operator with a plain per-agent file is not
        // told they have a problem.
        assert!(overridden_rows(&skill, "cursor").is_empty());
        fs::write(
            agent_override(&skill, "cursor"),
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n| {} | {} | x |\n{}\n",
                CONFIG_FENCE.begin,
                theirs.label(),
                theirs.value_of(&Config::default()),
                CONFIG_FENCE.end
            ),
        )
        .expect("their file");
        assert!(overridden_rows(&skill, "cursor").is_empty());
    }

    #[test]
    fn one_agent_s_file_cannot_restate_what_the_repository_is() {
        // The per-agent file exists so two agents can be told different things
        // about what *they* may do. It is not a second answer to what the
        // repository is: the gate reads those rows without asking whose turn it
        // is, so a tracker set for one adapter had the agent talking to Linear
        // while the gate read GitHub. `config set --agent` refuses to write
        // them, and this is the other half — the file is text, and an operator
        // or an older build can put anything in it.
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install");

        // A setting with only one answer cannot demonstrate anything, so each
        // side is the first that has a second answer to move to.
        let pick = |scope| {
            crate::config::SETTINGS
                .iter()
                .find_map(|setting| {
                    (setting.scope() == scope)
                        .then(|| other_answer(*setting, &Config::default()))
                        .flatten()
                        .map(|answer| (*setting, answer))
                })
                .expect("a setting with more than one answer")
        };
        let (repository, moved_repository) = pick(crate::config::Scope::Everywhere);
        let (mine, moved_mine) = pick(crate::config::Scope::Agent);

        // Both moved, in one file, by hand.
        let mut wanted = Config::default();
        repository
            .apply(&mut wanted, &moved_repository)
            .expect("a value the setting accepts");
        mine.apply(&mut wanted, &moved_mine)
            .expect("a value the setting accepts");
        fs::write(
            agent_override(&skill, "opencode"),
            format!(
                "# theirs\n\n{}\n{}\n{}\n",
                CONFIG_FENCE.begin,
                wanted.render_rows(),
                CONFIG_FENCE.end
            ),
        )
        .expect("their file");

        let read = installed_config_for(&skill, Some("opencode")).expect("the layered read");
        assert_eq!(
            mine.value_of(&read),
            moved_mine,
            "{} differs by agent and the agent\'s own file did not move it",
            mine.label()
        );
        assert_eq!(
            repository.value_of(&read),
            repository.value_of(&Config::default()),
            "{} is a fact about the repository and one agent\'s file changed it",
            repository.label()
        );
    }

    #[test]
    fn config_layers_keep_contract_agent_local_and_repository_snapshots_distinct() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        let repository = tempfile::tempdir().expect("a repository");
        install(&skill, &Config::default(), false).expect("the contract installs");

        let mut agent = Config::default();
        crate::config::Setting::Planning
            .apply(&mut agent, "sdd lite")
            .expect("Planning is accepted");
        crate::setup::write_agent_configuration_wholly(
            &agent_override(&skill, "opencode"),
            "opencode",
            &agent,
        )
        .expect("the agent layer is written");
        fs::write(
            skill.join(crate::config::LOCAL_FILE),
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Blind judges | two blind | single |\n{}\n",
                crate::config::BLOCK_BEGIN,
                crate::config::BLOCK_END
            ),
        )
        .expect("the local layer is written");
        let repository_path = repository_config_path(repository.path());
        crate::setup::write_repository_configuration(
            &repository_path,
            &Config {
                merge: crate::config::MergeStrategy::Squash,
                ..Config::default()
            },
            crate::config::EVERYWHERE_SETTINGS,
        )
        .expect("the repository layer is written");

        let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
            .expect("all four layers read");

        assert_eq!(
            crate::config::Setting::Planning.value_of(&layers.contract),
            "direct"
        );
        assert_eq!(
            crate::config::Setting::Planning
                .value_of(layers.agent.as_ref().expect("the agent snapshot")),
            "sdd lite"
        );
        assert_eq!(
            crate::config::Setting::Judges
                .value_of(layers.local.as_ref().expect("the local snapshot")),
            "two blind"
        );
        assert_eq!(
            crate::config::Setting::Merge
                .value_of(layers.repository.as_ref().expect("the repository snapshot")),
            "squash"
        );
    }

    #[test]
    fn an_agent_files_repository_row_cannot_pin_a_new_portable_value() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("the contract installs");
        fs::write(
            agent_override(&skill, "opencode"),
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Merge strategy | squash | merge commit |\n{}\n",
                CONFIG_FENCE.begin, CONFIG_FENCE.end
            ),
        )
        .expect("the hand-edited agent file exists");
        let layers = config_layers(&skill, Some("opencode"), None).expect("the layers read");
        let mut portable = Config::default();
        crate::config::Setting::Merge
            .apply(&mut portable, "rebase")
            .expect("the newer contract value is accepted");

        let effective = layers
            .effective_over(&portable)
            .expect("the effective view composes");

        assert_eq!(
            layers.agent_settings,
            Vec::<crate::config::Setting>::new(),
            "a forbidden repository row claimed agent-file ownership"
        );
        assert_eq!(
            crate::config::Setting::Merge.value_of(&effective),
            "rebase",
            "the ignored row reapplied the older contract value"
        );
    }

    #[test]
    fn a_repositorys_planning_row_cannot_pin_a_new_portable_value() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        let repository = tempfile::tempdir().expect("a repository");
        install(&skill, &Config::default(), false).expect("the contract installs");
        let repository_path = repository_config_path(repository.path());
        fs::create_dir_all(
            repository_path
                .parent()
                .expect("the repository file has a parent"),
        )
        .expect("the repository configuration directory exists");
        fs::write(
            repository_path,
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Planning | sdd | direct |\n{}\n",
                CONFIG_FENCE.begin, CONFIG_FENCE.end
            ),
        )
        .expect("the hand-edited repository file exists");
        let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
            .expect("the layers read");
        let mut portable = Config::default();
        crate::config::Setting::Planning
            .apply(&mut portable, "sdd lite")
            .expect("the newer Planning value is accepted");

        let effective = layers
            .effective_over(&portable)
            .expect("the effective view composes");

        assert_eq!(
            layers.repository_settings,
            Vec::<crate::config::Setting>::new(),
            "a forbidden agent row claimed repository-file ownership"
        );
        assert_eq!(
            crate::config::Setting::Planning.value_of(&effective),
            "sdd lite",
            "the ignored row reapplied the older contract value"
        );
    }

    #[test]
    fn an_invalid_forbidden_agent_row_is_ignored_before_validation() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("the contract installs");
        fs::write(
            agent_override(&skill, "opencode"),
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Merge strategy | impossible | merge commit |\n\
                 | Planning | sdd lite | direct |\n{}\n",
                CONFIG_FENCE.begin, CONFIG_FENCE.end
            ),
        )
        .expect("the hand-edited agent file exists");

        let layers = config_layers(&skill, Some("opencode"), None)
            .expect("the forbidden invalid row is outside this document's scope");

        assert_eq!(
            layers.agent_settings,
            vec![crate::config::Setting::Planning]
        );
        assert_eq!(
            crate::config::Setting::Planning
                .value_of(layers.agent.as_ref().expect("the agent layer exists")),
            "sdd lite"
        );
    }

    #[test]
    fn an_invalid_permitted_agent_row_still_refuses() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("the contract installs");
        fs::write(
            agent_override(&skill, "opencode"),
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Planning | impossible | direct |\n{}\n",
                CONFIG_FENCE.begin, CONFIG_FENCE.end
            ),
        )
        .expect("the hand-edited agent file exists");

        let refusal = config_layers(&skill, Some("opencode"), None)
            .expect_err("an invalid owned row was ignored");

        assert_eq!(refusal.code, "config-value-unrecognised");
    }

    #[test]
    fn an_invalid_forbidden_repository_row_is_ignored_before_validation() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        let repository = tempfile::tempdir().expect("a repository");
        install(&skill, &Config::default(), false).expect("the contract installs");
        let path = repository_config_path(repository.path());
        fs::create_dir_all(path.parent().expect("the repository file has a parent"))
            .expect("the repository configuration directory exists");
        fs::write(
            path,
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Planning | impossible | direct |\n\
                 | Merge strategy | squash | merge commit |\n{}\n",
                CONFIG_FENCE.begin, CONFIG_FENCE.end
            ),
        )
        .expect("the hand-edited repository file exists");

        let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
            .expect("the forbidden invalid row is outside this document's scope");

        assert_eq!(
            layers.repository_settings,
            vec![crate::config::Setting::Merge]
        );
        assert_eq!(
            crate::config::Setting::Merge.value_of(
                layers
                    .repository
                    .as_ref()
                    .expect("the repository layer exists")
            ),
            "squash"
        );
    }

    #[test]
    fn an_invalid_permitted_repository_row_still_refuses() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        let repository = tempfile::tempdir().expect("a repository");
        install(&skill, &Config::default(), false).expect("the contract installs");
        let path = repository_config_path(repository.path());
        fs::create_dir_all(path.parent().expect("the repository file has a parent"))
            .expect("the repository configuration directory exists");
        fs::write(
            path,
            format!(
                "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Merge strategy | impossible | merge commit |\n{}\n",
                CONFIG_FENCE.begin, CONFIG_FENCE.end
            ),
        )
        .expect("the hand-edited repository file exists");

        let refusal = config_layers(&skill, Some("opencode"), Some(repository.path()))
            .expect_err("an invalid owned row was ignored");

        assert_eq!(refusal.code, "config-value-unrecognised");
    }

    #[test]
    fn config_layers_bind_repository_values_and_ownership_to_one_snapshot() {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        let repository = tempfile::tempdir().expect("a repository");
        let replacement = tempfile::tempdir().expect("a replacement repository");
        install(&skill, &Config::default(), false).expect("the contract installs");
        let mut first = Config::default();
        crate::config::Setting::Merge
            .apply(&mut first, "squash")
            .expect("the first merge strategy is accepted");
        crate::setup::write_repository_configuration(
            &repository_config_path(repository.path()),
            &first,
            &[crate::config::Setting::Merge],
        )
        .expect("the first snapshot exists");
        let mut second = Config::default();
        crate::config::Setting::Tracker
            .apply(&mut second, "linear")
            .expect("the replacement tracker is accepted");
        let replacement_path = repository_config_path(replacement.path());
        crate::setup::write_repository_configuration(
            &replacement_path,
            &second,
            &[crate::config::Setting::Tracker],
        )
        .expect("the replacement snapshot exists");
        inject_repository_snapshot_replacement(
            fs::read_to_string(replacement_path).expect("the replacement snapshot reads"),
        );

        let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
            .expect("one repository snapshot supplies the layer");

        assert_eq!(
            layers.repository_settings,
            vec![crate::config::Setting::Merge]
        );
        assert_eq!(
            crate::config::Setting::Merge
                .value_of(layers.repository.as_ref().expect("repository values")),
            "squash"
        );
    }

    /// Any answer a setting accepts other than the one it currently holds.
    ///
    /// `None` where it offers only one: `Delivery route` accepts `direct` and
    /// nothing else, so it can never demonstrate a value moving.
    fn other_answer(setting: crate::config::Setting, config: &Config) -> Option<String> {
        let now = setting.value_of(config);
        setting
            .answers()
            .choices
            .iter()
            .find(|choice| **choice != now && setting.apply(&mut config.clone(), choice).is_ok())
            .map(|choice| (*choice).to_owned())
    }

    #[test]
    fn install_then_uninstall_leaves_nothing_behind() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join("skills").join(DIRECTORY);
        let config = Config::default();

        let installed = install(&skill, &config, false).unwrap();
        assert_eq!(installed.changed_files(), FILES.len() + 1, "the record too");
        assert_eq!(presence(&skill, &config), Presence::Current);

        let removed = uninstall(&skill, false).unwrap();
        assert_eq!(removed.changed_files(), FILES.len() + 1, "the record too");
        assert!(
            !skill.exists(),
            "the skill directory came out empty and went"
        );
        assert!(
            root.path().join("skills").exists(),
            "and nothing above it moved"
        );
    }

    #[test]
    fn uninstall_keeps_a_file_estigia_did_not_write() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).unwrap();
        let mine = skill.join("references").join("my-own-notes.md");
        fs::write(&mine, "mine").unwrap();

        uninstall(&skill, false).unwrap();
        assert!(
            mine.exists(),
            "an operator's own file is not Estigia's to remove"
        );
    }

    #[test]
    fn a_dry_run_writes_nothing_and_counts_everything() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(DIRECTORY);
        let planned = install(&skill, &Config::default(), true).unwrap();
        // Every shipped file, and the install record beside them: a plan that
        // leaves it out undercounts by the one file `uninstall` depends on.
        assert_eq!(planned.changed_files(), FILES.len() + 1);
        assert!(planned.actions.iter().all(|a| a.change == Change::Create));
        assert!(!skill.exists());
    }

    #[test]
    fn a_second_install_reports_no_change() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(DIRECTORY);
        let config = Config::default();
        install(&skill, &config, false).unwrap();
        let again = install(&skill, &config, false).unwrap();
        assert_eq!(again.changed_files(), 0);
    }

    #[test]
    fn the_list_of_repositories_drops_the_ones_that_no_longer_answer() {
        // The list exists so a checkout can be found again from anywhere. Its
        // one rule is that it says what is true **now**: a repository whose
        // file has gone — deleted, or the whole clone thrown away — is not
        // offered, because somebody who picks it gets the contract's answers
        // and believes they are that repository's.
        let home = tempfile::tempdir().expect("a home");
        let live = tempfile::tempdir().expect("a checkout that answers");
        let gone = tempfile::tempdir().expect("a checkout that will not");

        assert!(
            known_repositories(home.path()).is_empty(),
            "a machine with nothing recorded already lists something"
        );

        for repo in [live.path(), gone.path()] {
            let path = repository_config_path(repo);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                "| Setting | Value |
|---|---|
",
            )
            .unwrap();
            remember_repository(home.path(), repo);
        }
        assert_eq!(
            known_repositories(home.path()).len(),
            2,
            "both were recorded, so the pruning below measures something"
        );

        // One of them stops answering for itself.
        fs::remove_file(repository_config_path(gone.path())).unwrap();
        let known = known_repositories(home.path());
        assert_eq!(known, vec![live.path().to_path_buf()], "{known:?}");

        // And recording the same checkout twice lists it once: the list is
        // read on every `--repo`, and a line per write would grow forever.
        remember_repository(home.path(), live.path());
        remember_repository(home.path(), live.path());
        assert_eq!(known_repositories(home.path()).len(), 1);
    }

    #[test]
    fn a_repository_with_no_file_of_its_own_reads_exactly_as_before() {
        // The half that protects everybody who already has Estigia installed.
        // A layer that changed their answers would be a migration; this is an
        // addition, and an addition has to be invisible until it is used.
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(DIRECTORY);
        let repo = tempfile::tempdir().unwrap();
        let config = Config {
            merge: crate::config::MergeStrategy::Rebase,
            ..Config::default()
        };
        install(&skill, &config, false).unwrap();
        assert!(
            !repository_config_path(repo.path()).exists(),
            "the fixture already has a repository file, so this measures nothing"
        );
        assert_eq!(
            installed_config_in(&skill, repo.path()).unwrap(),
            installed_config(&skill).unwrap(),
            "a repository with nothing of its own answered differently"
        );
    }

    #[test]
    fn what_a_repository_says_about_itself_wins_and_says_nothing_about_an_agent() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(DIRECTORY);
        let repo = tempfile::tempdir().unwrap();
        install(&skill, &Config::default(), false).unwrap();
        let installed = installed_config(&skill).unwrap();

        // One row about the repository, and one about an agent, in the same
        // file. The first is this repository's to answer; the second is not.
        let path = repository_config_path(repo.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "| Setting | Value |
|---|---|
| Merge strategy | rebase |
             | Blind judges | two blind |
",
        )
        .unwrap();

        let layered = installed_config_in(&skill, repo.path()).unwrap();
        assert_eq!(
            layered.merge,
            crate::config::MergeStrategy::Rebase,
            "the repository did not get to say what history its base branch keeps"
        );
        assert_ne!(
            installed.merge,
            crate::config::MergeStrategy::Rebase,
            "the installed contract already said rebase, so the line above measures nothing"
        );
        assert_ne!(
            crate::config::Setting::Judges.value_of(&installed),
            "two blind",
            "the contract already says what the file asks for, so the line below would hold whether or not the row was put back"
        );
        assert_eq!(
            crate::config::Setting::Judges.value_of(&layered),
            crate::config::Setting::Judges.value_of(&installed),
            "a repository file restated a row that is not about the repository"
        );
    }

    #[test]
    fn the_installed_configuration_reads_back() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(DIRECTORY);
        let config = Config {
            merge: crate::config::MergeStrategy::Rebase,
            ..Config::default()
        };
        install(&skill, &config, false).unwrap();
        assert_eq!(installed_config(&skill).unwrap(), config);
    }

    /// A missing skill names a command that actually installs it.
    ///
    /// It named `estigia install`, and this test asserted that spelling while
    /// being called *the command that installs it*. It is not one: `install` is
    /// an alias for `setup`, and `setup` with no agent refuses — *`setup` needs
    /// to know which agent to configure*. So the only instruction on the screen
    /// was a dead end, which the ratchet forbids more strongly than saying
    /// nothing.
    ///
    /// Both spellings, because the difference is a fact this crate has in hand:
    /// asked about one agent it names that agent, and asked about none it names
    /// the command that covers all of them.
    #[test]
    fn a_missing_skill_refuses_with_the_command_that_installs_it() {
        let root = tempfile::tempdir().unwrap();
        let refusal = installed_config(root.path()).unwrap_err();
        assert_eq!(refusal.code, "skill-not-installed");
        assert!(
            refusal.to_string().contains("run: estigia setup --all"),
            "{refusal}"
        );

        let named = installed_config_for(root.path(), Some("claude-code")).unwrap_err();
        assert_eq!(named.code, "skill-not-installed");
        assert!(
            named.to_string().contains("run: estigia setup claude-code"),
            "the agent was named and the answer was not about it: {named}"
        );
    }

    /// A file Estigia wrote and somebody edited is left where it is.
    ///
    /// The removal's half of the question the record could not answer until it
    /// held digests. `uninstall` took a file the operator had edited with the
    /// rest, on its own `remove` line like every other and in no way marked as
    /// one they had touched — the honesty contract said exactly that, and said
    /// what closing it needed.
    ///
    /// Kept rather than removed-and-marked, for the reason the guard's refusal
    /// gives about a push hook: an uninstall that carries off somebody's
    /// additions is the thing an operator notices about other people's software
    /// and does not forgive.
    ///
    /// Three answers, and the last two are what keep the fix from becoming a
    /// refusal to uninstall anything: an untouched file still goes, and a record
    /// with no digest — every one written before this existed — still goes too.
    #[test]
    fn a_file_somebody_edited_is_kept_by_the_uninstall() {
        let root = tempfile::tempdir().expect("a skill root");
        install(root.path(), &Config::default(), false).expect("installs");

        let mine = root.path().join(relative("references/runtime-notes.md"));
        let untouched = root
            .path()
            .join(relative("references/repository-delivery.md"));
        let held = std::fs::read_to_string(&mine).expect("the file reads");
        std::fs::write(
            &mine,
            format!(
                "{held}
## my own note
"
            ),
        )
        .expect("the operator edits it");

        uninstall(root.path(), false).expect("uninstalls");
        assert!(mine.is_file(), "a file somebody edited was taken away");
        assert!(
            std::fs::read_to_string(&mine)
                .expect("the file reads")
                .contains("my own note"),
            "the file survived and its addition did not"
        );
        assert!(
            !untouched.is_file(),
            "an untouched file survived, so the uninstall stopped uninstalling"
        );

        // A record that predates digests: every machine that upgraded into this
        // has one, and reading its silence as an edit would leave whole
        // installations behind.
        let old = tempfile::tempdir().expect("a second root");
        install(old.path(), &Config::default(), false).expect("installs");
        let path = old.path().join(relative("references/runtime-notes.md"));
        let held = std::fs::read_to_string(&path).expect("the file reads");
        std::fs::write(
            &path,
            format!(
                "{held}
## edited
"
            ),
        )
        .expect("edited");
        let ledger = old.path().join(".estigia").join("installed.json");
        let text = std::fs::read_to_string(&ledger).expect("the record reads");
        let mut value: serde_json::Value = serde_json::from_str(&text).expect("json");
        value.as_object_mut().expect("an object").remove("digests");
        std::fs::write(&ledger, value.to_string()).expect("a record without digests");

        uninstall(old.path(), false).expect("uninstalls");
        assert!(
            !path.is_file(),
            "a record with no digests left its installation behind"
        );
    }

    /// An uninstall takes the retired file too, or it leaves the application behind.
    ///
    /// `uninstall_from` walks `FILES`, and a retired path is not in it — so the
    /// removal walked straight past `scripts/github.py`. Measured before this
    /// was written: after a full `uninstall` on a machine that had it, the file
    /// was still there and so was its directory.
    ///
    /// Worse than untidy. What survives is an **executable copy of a transport
    /// the contract no longer names**, on a machine whose owner believes Estigia
    /// is gone — and an uninstall that leaves part of the tool behind is the
    /// thing an operator notices about somebody else's software and does not
    /// forgive.
    #[test]
    fn an_uninstall_takes_the_retired_transport_with_it() {
        let root = tempfile::tempdir().expect("a skill root");
        install(root.path(), &Config::default(), false).expect("installs");

        // A machine that already had it: the file, and the record calling it
        // this crate's own — which is the only thing that makes it removable.
        let retired = root.path().join(relative("scripts/github.py"));
        std::fs::create_dir_all(retired.parent().expect("a directory")).expect("the directory");
        std::fs::write(
            &retired,
            "print('the old transport')
",
        )
        .expect("the file");
        record::note_created(root.path(), std::iter::once("scripts/github.py"))
            .expect("the record");

        uninstall(root.path(), false).expect("uninstalls");
        assert!(
            !retired.is_file(),
            "{} survived an uninstall",
            retired.display()
        );

        // The floor, and the rule that governs every removal here: a file no
        // record claims is somebody else's and stays.
        let theirs = tempfile::tempdir().expect("a second root");
        install(theirs.path(), &Config::default(), false).expect("installs");
        let stranger = theirs.path().join(relative("scripts/github.py"));
        std::fs::create_dir_all(stranger.parent().expect("a directory")).expect("the directory");
        std::fs::write(
            &stranger,
            "print('somebody else')
",
        )
        .expect("the file");
        uninstall(theirs.path(), false).expect("uninstalls");
        assert!(
            stranger.is_file(),
            "a file no record claims was removed by an uninstall"
        );
    }

    /// Line endings are not an edit.
    ///
    /// The digest that keeps somebody's work has to answer the same question the
    /// install already answers with `normalize(found) == normalize(desired)`, or
    /// it is a second rule about one thing. It was: hashed over raw bytes, a
    /// file converted to CRLF read as edited.
    ///
    /// Not exotic on this crate's own platform — `core.autocrlf` and half the
    /// editors rewrite line endings without anybody editing anything — and it
    /// failed in the expensive direction: the uninstall **kept** files nobody
    /// had touched, so a fix against leaving somebody's work behind began
    /// leaving the application behind.
    ///
    /// Both halves: a rewrapped file is the same file, and a changed one is not.
    #[test]
    fn changing_the_line_endings_is_not_editing_the_file() {
        let lines = ["# a note", "", "with two lines", ""];
        let lf = lines.join("\n");
        let crlf = lines.join("\r\n");
        // The fixture first: two spellings, and they have to be two. Written
        // with both joins the same, this compared a string with itself and
        // passed with the normalisation taken out — a test measuring nothing.
        assert_ne!(lf, crlf, "the two spellings are the same string");
        assert_eq!(
            record::digest_of(&lf),
            record::digest_of(&crlf),
            "a file rewrapped to CRLF read as one somebody had edited"
        );
        assert_ne!(
            record::digest_of(&lf),
            record::digest_of(&["# a note", "", "with three lines", ""].join("\n")),
            "the digest stopped telling two different files apart"
        );
    }

    /// The directory a retired file lived in goes with it.
    ///
    /// `remove_empty_directories` builds its candidates from `FILES` — what this
    /// crate ships **now** — and `RETIRED` is by definition not in that list. So
    /// `scripts/github.py` was taken and `scripts/` stayed, empty, in a skill
    /// directory whose owner had just been told the application was gone.
    ///
    /// The comment above the retirement loop in `uninstall_from` already claims
    /// otherwise: *"after `uninstall`, the retired `scripts/github.py` was still
    /// there, and so was the directory holding it"*. The file was fixed. The
    /// sentence went on naming both.
    ///
    /// Measured on the real binary before this was written: a `sync` over an
    /// install carrying the retired transport printed one `remove` line and left
    /// `issue-flow/scripts/` behind, and a full `uninstall` left it too — the
    /// only empty directory in the tree, and the only leftover the uninstall's
    /// own note does not account for.
    ///
    /// Both halves, because the second is what makes the first safe: an empty
    /// directory is taken, and a directory with anything at all in it is not.
    #[test]
    fn the_directory_a_retired_file_lived_in_goes_with_it() {
        for (name, theirs) in [
            ("scripts/gone.py", None),
            ("scripts/kept.py", Some("mine.py")),
        ] {
            let root = tempfile::tempdir().expect("a skill root");
            let config = crate::config::Config::default();
            install(root.path(), &config, false).expect("the skill installs");

            let file = root.path().join(relative(name));
            let directory = file.parent().expect("a directory").to_path_buf();
            std::fs::create_dir_all(&directory).expect("the directory");
            let body = "print('as this crate wrote it')\n";
            std::fs::write(&file, body).expect("the file");
            record::note_written(
                root.path(),
                std::iter::once((name, record::digest_of(body))),
            )
            .expect("the record");
            if let Some(theirs) = theirs {
                std::fs::write(directory.join(theirs), "# mine\n").expect("their file");
            }

            let created: std::collections::BTreeSet<String> =
                std::iter::once(name.to_owned()).collect();
            let taken = retire(root.path(), &[name], &created, &mut Pending::new(), false)
                .expect("the retirement runs");

            // The floor: the retirement did what this is about to judge it on.
            assert_eq!(taken.len(), 1, "{name}: nothing was retired");
            assert_eq!(taken[0].change, Change::Remove, "{name}");
            assert!(!file.is_file(), "{name}: the retired file was kept");

            match theirs {
                None => assert!(
                    !directory.exists(),
                    "{name}: the directory Estigia made to hold one file is empty and still there"
                ),
                Some(theirs) => {
                    assert!(
                        directory.join(theirs).is_file(),
                        "{name}: an operator's file went with the sweep"
                    );
                    assert!(
                        directory.is_dir(),
                        "{name}: a directory holding somebody's work was removed"
                    );
                }
            }
        }
    }

    /// A machine one release behind is not accused of writing over somebody.
    ///
    /// `OVERWRITE` is the loud word: it says an edit of yours has just been
    /// discarded. It is earned by a file whose contents have **moved since
    /// Estigia wrote them** — and the same branch decides `Update`, which is
    /// bookkeeping and says nothing about anybody's work.
    ///
    /// Both halves of that were unmeasured. Replacing the digest comparison
    /// with *the record has no digest at all* left the whole suite green, and
    /// under it every stale file on every ordinary upgrade would have been
    /// reported as an overwrite of the operator's work — the false alarm this
    /// branch's own comment says it exists to avoid, and the sentence the
    /// README makes about a machine one release behind.
    ///
    /// Two fixtures, and they differ only in the record: a file Estigia wrote
    /// and an older build's content, both stale against this build's copy. The
    /// one whose digest still matches what was written is `Update`; the one
    /// whose contents moved since is `Overwrite`.
    #[test]
    fn a_stale_file_nobody_edited_is_an_update_and_not_an_overwrite() {
        for (edited, wanted) in [(false, Change::Update), (true, Change::Overwrite)] {
            let root = tempfile::tempdir().expect("a skill root");
            let config = crate::config::Config::default();
            install(root.path(), &config, false).expect("the skill installs");

            // An older build's copy of a payload file: different from what this
            // build ships, so the install has something to do.
            let path = "references/runtime-notes.md";
            let file = root.path().join(relative(path));
            let older = "# Runtime notes\n\nas an older build wrote them\n";
            std::fs::write(&file, older).expect("the older copy");
            // And the record says so — this is the machine one release behind.
            record::note_written(
                root.path(),
                std::iter::once((path, record::digest_of(older))),
            )
            .expect("the record");

            if edited {
                // Their edit, after the record was written.
                std::fs::write(&file, format!("{older}\nand a line of mine\n"))
                    .expect("their edit");
            }

            let plan = install(root.path(), &config, true).expect("the plan");
            let action = plan
                .actions
                .iter()
                .find(|action| action.path.ends_with(relative(path)))
                .unwrap_or_else(|| panic!("the plan does not mention {path}"));
            assert_eq!(
                action.change,
                wanted,
                "a stale file {} was reported as {:?}",
                if edited {
                    "somebody edited"
                } else {
                    "nobody touched"
                },
                action.change
            );
        }
    }
    /// A retired file somebody patched is left where it is.
    ///
    /// The **third** removal path, and it read the record by name while the
    /// install and the uninstall had both learned to read the digest. Measured:
    /// a `scripts/github.py` an operator had patched was taken by the next
    /// `sync`, on a plain `remove` line.
    ///
    /// The tension here is real and the answer is deliberate. A retired file is
    /// one this crate asked to be gone, and an executable copy of an old
    /// transport left beside a contract that no longer names it is the hazard
    /// retirement exists to end. It is kept anyway: taking somebody's work is
    /// the worse of the two failures, the file is theirs now, and nothing this
    /// crate runs goes looking for it.
    #[test]
    fn a_retired_file_somebody_patched_is_kept() {
        let root = tempfile::tempdir().expect("a skill root");
        let config = crate::config::Config::default();
        install(root.path(), &config, false).expect("the skill installs");

        let path = "scripts/retired-by-a-test.py";
        let file = root.path().join(relative(path));
        std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory");
        std::fs::write(
            &file,
            "print('as this crate wrote it')
",
        )
        .expect("the file");
        let created: std::collections::BTreeSet<String> =
            std::iter::once(path.to_owned()).collect();
        record::note_written(
            root.path(),
            std::iter::once((
                path,
                record::digest_of(
                    "print('as this crate wrote it')
",
                ),
            )),
        )
        .expect("the record");

        // Untouched: taken, or the fix is a refusal to retire anything.
        let taken = retire(root.path(), &[path], &created, &mut Pending::new(), false)
            .expect("the retirement runs");
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].change, Change::Remove);
        assert!(!file.is_file(), "an untouched retired file was kept");

        // Patched: kept, and named on its own line so nothing is left silently.
        //
        // The directory is remade first: the removal above took it, because the
        // file was the whole of it. That is the behaviour
        // `the_directory_a_retired_file_lived_in_goes_with_it` is for, and this
        // fixture would otherwise fail on the write rather than on the claim.
        std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory again");
        std::fs::write(
            &file,
            "print('patched by the operator')
",
        )
        .expect("the file");
        let kept = retire(root.path(), &[path], &created, &mut Pending::new(), false)
            .expect("the retirement runs");
        assert_eq!(kept.len(), 1, "a kept file was not named");
        assert_eq!(kept[0].change, Change::Kept);
        assert!(
            std::fs::read_to_string(&file)
                .expect("the file reads")
                .contains("patched by the operator"),
            "a patched retired file was taken with the rest"
        );
    }

    /// A file this crate used to ship is taken away — and only that file.
    ///
    /// `uninstall_from` walks `FILES`, so a path deleted from that list stops being
    /// anybody's to remove: it stays on every machine that ever had it, invisible to
    /// `status` and untouched by `uninstall`. For the transport that is not
    /// untidiness — a `scripts/github.py` left beside a contract that no longer
    /// names it is the second implementation coming back, and two implementations
    /// cost exactly what they disagree about.
    ///
    /// Three properties, and the middle one is what keeps this from becoming a
    /// cleanup that eats somebody's work: a retired path is removed **only** where
    /// the install record says Estigia created it.
    #[test]
    fn a_retired_file_is_taken_away_and_a_stranger_s_is_not() {
        let root = tempfile::tempdir().expect("a skill root");
        let config = crate::config::Config::default();
        super::install(root.path(), &config, false).expect("the skill installs");

        // A path this crate's record claims, and one it does not, side by side in
        // one root — so the difference between them is the record and nothing else.
        let mine = "scripts/retired-by-a-test.py";
        let theirs = "scripts/somebody-elses.py";
        for path in [mine, theirs] {
            let file = root.path().join(relative(path));
            std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory");
            std::fs::write(
                &file,
                "print('hello')
",
            )
            .expect("the file");
        }
        let created: std::collections::BTreeSet<String> =
            std::iter::once(mine.to_owned()).collect();

        let taken = retire(
            root.path(),
            &[mine, theirs],
            &created,
            &mut Pending::new(),
            false,
        )
        .expect("the retirement runs");

        assert_eq!(taken.len(), 1, "one file was retired, not {}", taken.len());
        assert!(
            taken[0].path.ends_with("retired-by-a-test.py"),
            "the wrong file was taken: {:?}",
            taken[0].path
        );
        assert_eq!(taken[0].change, Change::Remove);
        assert!(
            !root.path().join(relative(mine)).is_file(),
            "the retired file is still there"
        );
        assert!(
            root.path().join(relative(theirs)).is_file(),
            "a file no record claims was removed anyway"
        );

        // And the floor on the list: a path nobody retired is left alone, however
        // plainly this crate's own it is.
        //
        // Written back first. Asked against the directory the call above left,
        // the file is already gone and an empty list agrees with a list that
        // retires everything — measured, by making the empty list retire this
        // very path: the assertion below passed.
        std::fs::write(
            root.path().join(relative(mine)),
            "print('hello')
",
        )
        .expect("the file");
        let none = retire(root.path(), &[], &created, &mut Pending::new(), false)
            .expect("an empty retirement runs");
        assert!(none.is_empty(), "a path nobody retired was removed anyway");
    }
}
