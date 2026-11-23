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

/// One embedded file, and where it lands relative to the skill directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillFile {
    /// Forward-slashed, relative to the skill directory.
    pub path: &'static str,
    /// The file's bytes, embedded at build time.
    pub contents: &'static str,
}

/// The blind-review definition installed for Claude Code, as a template:
/// rendered before it is written, and matched on read by `reviewer_is_static`
/// against any rendering `Evidence::all()` × `RESERVED_REVIEWERS` produces —
/// not these bytes. Launch itself uses a narrower check bound to the
/// installed file's own row.
pub const REVIEW_AGENT: SkillFile = SkillFile {
    path: "agents/review-blind.md",
    contents: include_str!("../skill/agents/review-blind.md"),
};

/// The lens definition: sealed, one narrow question, the diff as its whole
/// world. Not a third judge — `crate::harness::roles::RESERVED_LENS` is its
/// own row in `RESERVED_REVIEWERS`, carrying this template rather than
/// [`REVIEW_AGENT`]'s.
pub const REVIEW_LENS: SkillFile = SkillFile {
    path: "agents/review-lens.md",
    contents: include_str!("../skill/agents/review-lens.md"),
};

/// A sealed lens ported from the source project's `review-risk`: security
/// risk narrowed to what this crate has a subject for. Its own row in
/// `RESERVED_REVIEWERS`, carrying this template rather than [`REVIEW_AGENT`]'s
/// or [`REVIEW_LENS`]'s.
pub const REVIEW_RISK: SkillFile = SkillFile {
    path: "agents/review-risk.md",
    contents: include_str!("../skill/agents/review-risk.md"),
};

/// A sealed lens ported from the source project's `review-readability`: code
/// clarity, re-anchored to this repository's own comment rule. Its own row
/// in `RESERVED_REVIEWERS`.
pub const REVIEW_READABILITY: SkillFile = SkillFile {
    path: "agents/review-readability.md",
    contents: include_str!("../skill/agents/review-readability.md"),
};

/// A sealed lens ported from the source project's `review-reliability`: test
/// and behavior risk, with its sharpest bullet re-derived into this
/// repository's own "can this suite report a pass having executed nothing?"
/// defect class. Its own row in `RESERVED_REVIEWERS`.
pub const REVIEW_RELIABILITY: SkillFile = SkillFile {
    path: "agents/review-reliability.md",
    contents: include_str!("../skill/agents/review-reliability.md"),
};

/// The resilience slot, re-derived rather than ported — the source
/// project's `review-resilience` has no subject in this crate, so this
/// answers a different question, legibility of failure, under its own
/// name (`docs/honesty.md`). Its own row in `RESERVED_REVIEWERS`.
pub const REVIEW_READBACK: SkillFile = SkillFile {
    path: "agents/review-readback.md",
    contents: include_str!("../skill/agents/review-readback.md"),
};

/// The refuter: sealed, adjudicates a panel's own blocking findings rather
/// than performing a second re-review. Its own row in `RESERVED_REVIEWERS`,
/// carrying this template rather than [`REVIEW_AGENT`]'s or any of the five
/// lens templates — it is neither a judge nor a lens.
pub const REVIEW_REFUTER: SkillFile = SkillFile {
    path: "agents/review-refuter.md",
    contents: include_str!("../skill/agents/review-refuter.md"),
};

/// The targeted validator: sealed and read-only, it checks a correction against the
/// parent's frozen findings and the corrected paths. Its own row in
/// `RESERVED_REVIEWERS`; neither a judge nor a lens.
pub const REVIEW_VALIDATOR: SkillFile = SkillFile {
    path: "agents/review-validator.md",
    contents: include_str!("../skill/agents/review-validator.md"),
};

/// What a verdict must report, defined once so no launch path can drift from
/// another's wording.
///
/// Several shipped surfaces state it by hand — both arms of
/// `render_reviewer_agent`, the OpenCode `## Review on this host` block and
/// `skill/policies/judgment-day.md` — and a test crosses all of them rather
/// than trusting them to stay in step.
pub const VERDICT_BASIS_REQUIREMENT: &str = "A verdict states what it rests on: what was executed \
and its outcome, the mutation run behind each severe finding and whether it failed for the reason \
the defect exists, and what was not checked. Measuring nothing is a legitimate basis when it says so \
plainly rather than leaving the gap implied.";

/// The rule that a stale or incorrect comment or document never earns `severe`, hand-maintained
/// in seven places: `skill/agents/review-blind.md`, `skill/policies/blind-judges.md`, and the five
/// lens definitions `skill/agents/review-lens.md`, `review-risk.md`, `review-readability.md`,
/// `review-reliability.md` and `review-readback.md`. Only the first two are crossed against this
/// constant, because the five lens copies paraphrase the rule rather than carrying its text.
///
/// `skill/SKILL.md` no longer restates the rule at all: developer step 5 links to
/// `skill/policies/blind-judges.md`, which owns it. `blind-judges.md` says the duplication between
/// the two carriers below is deliberate.
pub const STALE_PROSE_NEVER_SEVERE: &str = "Incorrect or stale comments and documentation are \
observations about the words beside the behaviour, not about the behaviour itself, and they are \
**never `severe`** — not under a demonstrated cost, not under a reader who might be misled, not \
under any condition";

/// The carve-out's own bound, restated wherever the carve-out is stated: it answers only a
/// contradiction between two instructions a repository's own agents execute, and explicitly does
/// not reach staleness — so it never widens [`STALE_PROSE_NEVER_SEVERE`], only names what that
/// rule was never about.
///
/// Carried by the same seven files as the rule it bounds, and the changelog's 0.1.0 entry
/// besides; crossed against the two shipped carriers and the changelog.
/// `skill/SKILL.md` states no carve-out at all, which is the
/// question this sentence's presence there would settle if it ever grew one.
pub const CARVE_OUT_BOUND: &str = "The carve-out is narrow: it is gated on a contradiction between \
two instructions a repository's own agents execute, not on staleness, and it does not reach a \
stale comment or a stale document the never-`severe` rule above already covers.";

/// The worker a run hands a bounded piece of an understood change to.
const DEVELOPER_AGENT: SkillFile = SkillFile {
    path: "agents/developer.md",
    contents: include_str!("../skill/agents/developer.md"),
};

/// The read-only half of the same pair; the grants genuinely differ, so one
/// definition would have to carry the wider of the two.
const ANALYST_AGENT: SkillFile = SkillFile {
    path: "agents/analyst.md",
    contents: include_str!("../skill/agents/analyst.md"),
};

/// The SDD phase definitions retraction reads, by file name.
///
/// Nothing installs from this list: it is the names a run may find on disk once
/// the SDD protocol that wrote them is gone. It names every phase the retired
/// protocol defined, not only the five executors, because ownership — not
/// membership here — decides what comes out.
pub const RETIRED_PHASE_AGENTS: &[&str] = &[
    "sdd-explore.md",
    "sdd-propose.md",
    "sdd-spec.md",
    "sdd-design.md",
    "sdd-tasks.md",
    "sdd-apply.md",
    "sdd-verify.md",
    "sdd-archive.md",
];

/// The definitions this crate shipped once and does not ship now, by file name,
/// read the way [`RETIRED_PHASE_AGENTS`] is.
///
/// Nothing installs from this list. A name lands here when its definition
/// leaves [`DELEGATED_AGENTS`] or [`AGENT_DEFINITIONS`], so `setup` and
/// `uninstall` can still take an owned copy off a machine that has one. The
/// reviewer's own road is separate and stays that way: it carries the
/// historical-rendering ledger that lets a retired body be recognised, and this
/// list is for definitions no such ledger covers.
///
/// Ownership, not membership here, decides what comes out: a definition Estigia
/// did not create is somebody else's and is left where it is.
pub const RETIRED_AGENTS: &[&str] = &[
    // The delegated worker, renamed to `developer.md` without a
    // list to take the old name away.
    "implementer.md",
];

/// The delegated roles with an installed definition, each beside the
/// `Model routing` key that installs it by. Naming the role in the routing is
/// the switch — neither file is written or kept without it — and ownership
/// decides whether a definition Estigia did not author is refused rather than
/// replaced. `reviewer` and `judge` are roles without a file, so they are not
/// here.
pub const DELEGATED_AGENTS: &[(&str, SkillFile)] =
    &[("developer", DEVELOPER_AGENT), ("analyst", ANALYST_AGENT)];

/// All host definitions, as one lifecycle-hashed asset collection.
pub const AGENT_DEFINITIONS: &[SkillFile] = &[
    REVIEW_AGENT,
    REVIEW_LENS,
    REVIEW_RISK,
    REVIEW_READABILITY,
    REVIEW_RELIABILITY,
    REVIEW_READBACK,
    REVIEW_REFUTER,
    REVIEW_VALIDATOR,
    DEVELOPER_AGENT,
    ANALYST_AGENT,
];

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
        path: "bindings/github/delivery.md",
        contents: include_str!("../skill/bindings/github/delivery.md"),
    },
    SkillFile {
        path: "bindings/github/operations.md",
        contents: include_str!("../skill/bindings/github/operations.md"),
    },
    SkillFile {
        path: "bindings/github/operations-guarantees.md",
        contents: include_str!("../skill/bindings/github/operations-guarantees.md"),
    },
    SkillFile {
        path: "bindings/github/recovery.md",
        contents: include_str!("../skill/bindings/github/recovery.md"),
    },
    SkillFile {
        path: "bindings/github/refusals.md",
        contents: include_str!("../skill/bindings/github/refusals.md"),
    },
    SkillFile {
        path: "bindings/github/setup.md",
        contents: include_str!("../skill/bindings/github/setup.md"),
    },
    SkillFile {
        path: "references/analyst.md",
        contents: include_str!("../skill/references/analyst.md"),
    },
    SkillFile {
        path: "references/decision-gates.md",
        contents: include_str!("../skill/references/decision-gates.md"),
    },
    SkillFile {
        path: "references/domain-composition.md",
        contents: include_str!("../skill/references/domain-composition.md"),
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
        path: "policies/judgment-day.md",
        contents: include_str!("../skill/policies/judgment-day.md"),
    },
    SkillFile {
        path: "protocols/rdd.md",
        contents: include_str!("../skill/protocols/rdd.md"),
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

/// Content digest over every embedded skill asset, independent of build metadata.
///
/// Sorted paths, NUL-delimited path and normalised content, `sha256:`-prefixed
/// hex: the shape that answers *these bytes came from these sources* rather
/// than *this build has this identity*. Two builds from identical sources
/// agree regardless of timestamps, revisions or versions, because no metadata
/// enters the hash — only paths and contents, mapped straight off [`FILES`],
/// so a file joining or leaving the set moves the digest with no second list
/// to keep in step. What is hashed is the embedded text, never a rendering:
/// a rendered contract carries the operator configuration, which differs
/// between machines running one build.
pub fn assets_digest() -> String {
    assets_digest_of(
        &FILES
            .iter()
            .map(|file| (file.path, file.contents))
            .collect::<Vec<_>>(),
    )
}

/// [`assets_digest`] over an explicit asset list, so the properties are
/// pinned without rebuilding: determinism, one-byte sensitivity, order
/// independence, and CRLF normalisation.
fn assets_digest_of(assets: &[(&str, &str)]) -> String {
    let mut sorted = assets.to_vec();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    let mut buffer = Vec::new();
    for (path, contents) in sorted {
        buffer.extend_from_slice(path.as_bytes());
        buffer.push(0);
        buffer.extend_from_slice(contents.replace("\r\n", "\n").as_bytes());
        buffer.push(0);
    }
    format!(
        "sha256:{}",
        crate::transport::ownership::sha256_hex(&buffer)
    )
}

/// The directories whose files a setting chooses rather than a link.
///
/// Bindings are selected by `Tracker`; protocol documents are conditionally
/// routed from the contract rather than loaded as alternatives.
/// `policies/blind-judges.md` used to be a third, chosen by `Blind judges`;
/// it has since become unconditional, and it is now linked
/// unconditionally from the contract's document table the same way every
/// other always-shipped reference is, so it left this list rather than staying
/// in it with nothing left to select it.
///
/// Declared once, because it began as one `starts_with` and became two, and two
/// scattered special cases are how a third gets added somewhere else. Each
/// prefix owes a seam test of the shape `every_<axis>_names_a_<file>_that_ships`.
pub const SELECTED_BY_SETTING: &[&str] = &["bindings/", "protocols/"];

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
    // The transport: a stale copy of it beside a contract that no longer
    // names it is a second implementation coming back. Removed only when the
    // record says Estigia created it.
    "scripts/github.py",
    // Both describe a tracker this build refuses: a binding an agent reads
    // and no executable, kept on the way out for the same reason the
    // transport above is — a stale copy of either is a document telling
    // somebody to run tools this build no longer exposes.
    //
    // `bindings/linear.md` left this list when Linear came back
    // as a read-only source: it is a shipped file again, so it is in `FILES`
    // and is installed and uninstalled as one.
    "bindings/trello.md",
    // The ledger guarded a migration whose frozen revision no longer exists
    // in this repository's history. Nothing reads it and nothing points to
    // it any more.
    "references/migration-inventory.md",
    // The board moved onto the issue's own projects: the mirror is read from
    // the issue now, and a stale copy of the old document would tell a run to
    // configure a row this build no longer has.
    "bindings/github/board.md",
    // The SDD protocol: a stale copy of it would tell a run to follow a
    // methodology whose setting, phases and definitions this build no longer
    // has.
    "protocols/sdd.md",
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
         When this skill directory is shared by several agents, look beside this file for \
         `estigia.<agent>.md`, named for the agent reading it (for example `estigia.opencode.md`), \
         and then for `{local}`. Read in that order \u{2014} this table, then the agent's own \
         file, then `{local}` \u{2014} and `{local}` wins. Only the agent-scoped rows of \
         `estigia.<agent>.md` \u{2014} `Profile`, `Delivery authorisation`, `Transition \
         authorisation`, `Model routing`, `Loop plugin` and `Goal loop \
         plugin` \u{2014} override this table for that agent alone; a row here written for another \
         agent is not yours when your own file answers it. `{local}` carries local policy too: it \
         holds permissions and machine-specific paths, is ignored by Git, and MUST NOT be \
         committed or published. `{legacy}` is read only when `{local}` is not there, for \
         installations that came from issue-flow \u{2014} with both present, the older one is \
         ignored entirely.\n\n\
         Read the configuration in force with `estigia config list`, run from the repository: \
         it prints every row's effective value and the layer that answered it. The \
         repository's own file, `estigia/{local}` under its Git common directory, answers the \
         repository-scoped rows such as `Claim workflow`, and may tighten `Delivery \
         authorisation` and `Transition authorisation`, and only those: an \
         `ask` there over an `auto` here is in force, and an `auto` there over an `ask` here is \
         not. The stricter answer wins.\n\n\
         {method}{rows}",
        local = crate::config::LOCAL_FILE,
        legacy = crate::config::LEGACY_LOCAL_FILE,
        method = selected_documents(config),
        rows = config.render_rows()
    )
}

/// The policy a `by task` routing obliges, written once so every host routes it
/// rather than restating it.
///
/// One sentence, because the register is the contract's and a policy a host
/// paraphrases is a policy that disagrees with itself across hosts.
pub const BY_TASK_POLICY: &str = "\
`Model routing` is `by task`: the shape of the work decides the model and the effort \u{2014} \
mechanical work runs cheap and shallow, while a design or a verdict resting on a measurement runs \
expensive and deliberate \u{2014} and you, the launcher, decide, recording the model and the effort \
you chose for each delegated context.

";

/// What `Decision mode: auto` asks of the run, written only when it is in force.
pub const DECISION_MODE_AUTO: &str = "\
`Decision mode` is `auto` here: ask the person nothing, in prose or with a question tool. At each \
choice apply the most advisable answer and carry on; offer it through `AskUserQuestion` with that \
answer marked `(Recommended)`, where the host has the tool, and the gate refuses the question, \
records the answer and shows the person every question and decision when the turn ends. Where it \
has none, list each question and the decision taken in your final message. The authorisation rows \
still decide delivery and transitions.

";

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
///
/// This output goes into one contract that every adapter sharing a skill root
/// reads, so a host fact here names the host it means.
pub fn selected_documents(config: &Config) -> String {
    let mut lines = String::new();
    lines.push_str(
        "Review here follows `protocols/rdd.md` (`receipt-driven`): read it before a verdict.

",
    );
    lines.push_str(&format!(
        "Independent review here is the lenses `review_read` with `view: tier` licenses \u{2014} {medium} at \
         `medium`, {high} at `high`, {low} at `low` \u{2014} over one target.

",
        medium = crate::transport::tier::RiskLevel::Medium.lenses_spelled(),
        high = crate::transport::tier::RiskLevel::High.lenses_spelled(),
        low = crate::transport::tier::RiskLevel::Low.lenses_spelled(),
    ));
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

    if config.decision_mode == crate::config::DecisionMode::Auto {
        lines.push_str(DECISION_MODE_AUTO);
    }

    if config.models.by_task {
        lines.push_str(BY_TASK_POLICY);
    } else if !config.models.by_role.is_empty() {
        lines.push_str(
            "Where `Model routing` names a model for a role, `orchestrate` or `apply`, work here \
             runs on it, and a name in the effort list beside the models says how hard that \
             context is asked to think. Only Claude Code reads the sub-agent definitions `setup` \
             installs — the roles the routing names — so launching one of those needs no model \
             named in the prompt. Every other key except `reviewer` is a declaration nothing starts for you \
             (`reviewer` is read by `collect_review_input` with `launch: true` when it starts a lens, the refuter or \
             the validator): hand \
             the work to the model the row names for it yourself and say which one you used; where \
             the row names none, launch the context with no model named and let the default stand. \
             A row naming a model that nobody reads is a choice the operator made and nothing \
             followed.

",
        );
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
            "Write the summary sentence at the top of an issue in **{summary}**, and the title \
             and every section from `## Problem` down in **{body}**, and a pull request's title \
             and body in **{body}** as well \u{2014} whatever language the \
             repository's own documents, comments or existing issues are in. They are separate \
             rows because they are read by different people: the summary is for somebody who will never read past it, and the body is for \
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
    /// a different person, and `--dry-run` must not say `update` for a file it
    /// is about to overwrite. A file the record claims, whose contents have
    /// moved since Estigia wrote them, is not a stranger's — it is
    /// [`Self::Overwrite`].
    Replace,
    /// The file is Estigia's, and somebody edited it since Estigia wrote it.
    ///
    /// Apart from [`Self::Replace`] for the same reason; `uninstall` keeps the
    /// edited file and names it on its own line, while `sync` may bring it up
    /// to the binary's copy.
    Overwrite,
    /// The file was there and already correct.
    Unchanged,
    /// The file was there and Estigia took it away. Used only for a whole-file
    /// asset the ownership ledger says Estigia created.
    Remove,
    /// The file is the skill, and another configured agent still reads it:
    /// the skill goes out with the last of them, not the first.
    Shared,
    /// Estigia has no record of installing here, so nothing is shown to be
    /// its to remove — this file included, whoever wrote it.
    ///
    /// Apart from [`Change::Kept`] because the two are different sentences.
    Unrecorded,
    /// The file was there **before** Estigia, so uninstalling leaves it.
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
/// Four states rather than two, because they are four different sentences: in
/// particular `Unreadable` needs the operator's configuration, and comparing
/// against defaults instead reported a table carrying an unrecognised value as
/// **stale** — which leads to `estigia sync`, and `sync` refuses that contract
/// with `config-value-unrecognised` and changes nothing.
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
    // *What should these files carry* — which is
    // [`contract_config_folding_retired`], the contract's own table and never an
    // override. It folds a retired `Delegated workers` row the same way
    // `installed_config` above does, so a contract carrying one is compared
    // against the value it reads as rather than refused. Layering the override
    // into this second question compared the installed `SKILL.md` against a
    // value the override exists to keep out of it, so using that file at all
    // reported the skill as out of date.
    match installed_config(root).and_then(|_| contract_config_folding_retired(root)) {
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
/// Most adapters write to the same neutral skill root. In a real run the first
/// creates those files and the rest find them unchanged; under `--dry-run`
/// nothing is written, so without this each of them read the untouched disk and
/// planned to create all fifteen again. Measured: the plan said 182 files
/// where the run does 70 — and `--dry-run` is the one
/// command whose entire job is to be believed before anything happens.
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
    // Read through `pending`, like everything else here. Adapters share this
    // root, and under `--dry-run` nothing reaches the disk — so a later adapter
    // asking the disk would get the answer the first adapter had, not
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
    // Which build's asset set was last installed here, reconciled on every
    // real run rather than only when files moved: the aggregate names the
    // run, not the files, so a record that disagrees with untouched files —
    // tampered, or written by a run that died between the two notes — is
    // restored by the next sync instead of staying stale past it. The note
    // itself still skips the write when the value is already current.
    if !dry_run {
        record::note_assets(root, assets_digest())?;
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
/// The mirror of [`install_into`], and it has the same fault without it: most
/// adapters share a skill root, so under `--dry-run` each found the files still
/// on disk and planned to remove all fifteen again. The plan said seventeen
/// files for OpenCode where the run takes out three.
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
/// `Integration: branch` — the shipped default — while the operator's file said
/// `trunk`; and with an abandoned `operator.local.md` beside it, the machine ran
/// *that* one instead. Silently, in both cases: `doctor` said `ok contract` and
/// `status` said `configured`.
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

/// The top level of the checkout `dir` is inside: `dir` or its nearest ancestor
/// whose `.git` is a directory or a readable `gitdir: <path>` file, as a linked
/// worktree or submodule keeps. A `.git` that is neither — a dangling symlink, an
/// empty or unrelated file — is not a checkout, and the walk goes on above it.
/// `None` for a directory inside no checkout.
pub fn checkout_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|candidate| !candidate.as_os_str().is_empty() && is_checkout(candidate))
        .map(Path::to_path_buf)
}

fn is_checkout(candidate: &Path) -> bool {
    let dot_git = candidate.join(".git");
    dot_git.is_dir()
        || fs::read_to_string(&dot_git).is_ok_and(|text| {
            text.trim()
                .strip_prefix("gitdir:")
                .is_some_and(|pointed| !pointed.trim().is_empty())
        })
}

/// The git directory shared by a checkout and every worktree linked to it.
///
/// Not `repo_dir/.git`, which is a **file** in a linked worktree. Joining onto
/// it gave a path no directory could be created under, and this crate's own
/// delivery topology works in worktrees: `config set --repo` in one refused
/// with *could not write … (os error 3)* and told the operator to run `estigia
/// doctor`, which fixes nothing about it; and, worse, the same repository
/// answered `Integration = trunk` in the checkout and `branch` inside
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
/// `repo_dir` need not be the top level: the nearest ancestor holding a `.git`
/// is the checkout, as git itself finds it, so a session working from
/// `apps/engine` reads the rows of the repository it is inside.
///
/// Anything else — no `.git` in `repo_dir` or above it, or only ones that
/// [`checkout_root`] does not take for a checkout — resolves to `repo_dir`'s own
/// `.git`, which does not exist as a directory, and so has no layer.
pub(crate) fn git_common_dir(repo_dir: &Path) -> PathBuf {
    let repo_dir = checkout_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
    let repo_dir = repo_dir.as_path();
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

/// The key a recorded decision or need names a repository by, read without
/// spawning a process: `github.com/owner/name` when the shared git directory's
/// `origin` says so, otherwise the main checkout's path, and `None` for a
/// directory in no repository. The path is canonicalised so that one checkout
/// reached through a symlink keeps one key.
pub(crate) fn repository_key(repo_dir: &Path) -> Option<String> {
    let common = git_common_dir(repo_dir);
    if !common.is_dir() {
        return None;
    }
    if let Some(reference) = origin_reference(&common) {
        return Some(format!("github.com/{reference}"));
    }
    common
        .parent()
        .map(Path::to_path_buf)
        .filter(|root| root.join(".git").exists())
        .or_else(|| checkout_root(repo_dir))
        .map(|root| {
            fs::canonicalize(&root)
                .unwrap_or(root)
                .display()
                .to_string()
        })
}

/// The GitHub reference `[remote "origin"]`'s `url` names in a git directory's
/// `config`, or `None` when the file is absent, unreadable, or names no GitHub
/// remote. Several `url` lines are read in order; the first GitHub one wins.
fn origin_reference(common_dir: &Path) -> Option<crate::config::RepoRef> {
    let config = fs::read_to_string(common_dir.join("config")).ok()?;
    let mut origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            origin = section.trim() == "remote \"origin\"";
            continue;
        }
        if !origin {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "url" {
            continue;
        }
        if let Some(reference) = crate::config::RepoRef::from_github_remote(value.trim()) {
            return Some(reference);
        }
    }
    None
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

/// The GitHub repository a checkout's `origin` remote names — or its first
/// remote when there is no `origin` — when that is on GitHub.
///
/// Read from the checkout's own configured remote, not from a resolved URL: a
/// local `insteadOf` rewrite of the origin would otherwise hide the repository
/// the clone was made from, which is the one this names.
pub(crate) fn github_repository_of(checkout: &Path) -> Option<crate::config::RepoRef> {
    let git = |args: &[&str]| -> Option<String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let remotes = git(&["remote"])?;
    let name = remotes
        .lines()
        .find(|name| *name == "origin")
        .or_else(|| remotes.lines().next())?;
    let url = git(&["config", "--get", &format!("remote.{name}.url")])?;
    crate::config::RepoRef::from_github_remote(&url)
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
    let repo_dir = checkout_root(repo_dir).unwrap_or_else(|| repo_dir.to_path_buf());
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
    pub(crate) fn edit_scope_over(&self, base: &Config) -> Result<Config, Refusal> {
        in_file(
            &self.path,
            Config::read_scope_over(base, &self.document, crate::config::Scope::Repository),
        )
        .map(|(config, _)| config)
    }

    /// Applies this exact document to one lower-layer baseline.
    pub(crate) fn layer_over(&self, base: &Config) -> Result<RepositoryLayer, Refusal> {
        let (mut config, mut settings) = in_file(
            &self.path,
            Config::read_scope_over(base, &self.document, crate::config::Scope::Everywhere),
        )?;
        let (tightened, refused) = Config::tighten_over(base, &mut config, &self.document, false);
        if let Some(refusal) = refused {
            return Err(named_by(refusal, &self.path));
        }
        settings.extend(tightened);
        Ok(RepositoryLayer { config, settings })
    }

    fn review_voice(&self) -> Result<crate::config::ReviewVoice, Refusal> {
        Config::review_voice_in(&self.document).map_err(|refusal| named_by(refusal, &self.path))
    }

    fn layer_over_keeping_what_parses(&self, base: &Config) -> RepositoryLayer {
        let (mut config, mut settings) = Config::read_scope_over_keeping_what_parses(
            base,
            &self.document,
            crate::config::Scope::Everywhere,
        );
        let (tightened, _) = Config::tighten_over(base, &mut config, &self.document, true);
        settings.extend(tightened);
        RepositoryLayer { config, settings }
    }

    /// [`Self::layer_over`], folding a retired row rather than refusing it.
    pub(crate) fn layer_over_folding_retired_workers(
        &self,
        base: &Config,
    ) -> Result<(RepositoryLayer, Option<Refusal>), Refusal> {
        let (mut config, mut settings, folded) = in_file(
            &self.path,
            Config::read_scope_over_folding_retired_workers(
                base,
                &self.document,
                crate::config::Scope::Everywhere,
            ),
        )?;
        let (tightened, refused) = Config::tighten_over(base, &mut config, &self.document, false);
        if let Some(refusal) = refused {
            return Err(named_by(refusal, &self.path));
        }
        settings.extend(tightened);
        Ok((RepositoryLayer { config, settings }, folded))
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

/// The voice this checkout's own file names, read from that file alone.
///
/// A row for another setting that does not parse does not stop this read; an
/// unreadable file, or a `Review voice` row that does not parse, does.
pub(crate) fn repository_review_voice(
    repo_dir: &Path,
) -> Result<crate::config::ReviewVoice, Refusal> {
    match repository_snapshot(repo_dir)? {
        Some(snapshot) => snapshot.review_voice(),
        None => Ok(crate::config::ReviewVoice::None),
    }
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
///
/// The chain is told apart at the repository layer, which is what
/// `installed_config_in_keeping_what_parses_apart` adds: a repository file
/// that could not be read leaves the contract's rows standing, while a contract
/// that could not be read leaves nothing.
pub fn installed_config_in_keeping_what_parses(
    root: &Path,
    repo_dir: &Path,
) -> (Config, Option<Refusal>) {
    let (config, contract, repository) =
        installed_config_in_keeping_what_parses_apart(root, repo_dir);
    (config, contract.or(repository))
}

/// The same read, told apart into the failure that kept what parses and the
/// repository file that could not be read at all.
///
/// The gate needs the difference. A contract that did not parse, or a
/// repository row that did not, leaves nothing standing above the bad row but
/// still says no workflow; a repository file that is there and cannot be read
/// says nothing at all about the rows it carries, so they read as the denying
/// answer (`required` for the issue requirement) and the refusal names the file
/// and the read error. The first side is `None` when only the file could not be
/// read; the second is `None` when only a row did not parse; both are `None`
/// when everything parsed.
pub(crate) fn installed_config_in_keeping_what_parses_apart(
    root: &Path,
    repo_dir: &Path,
) -> (Config, Option<Refusal>, Option<Refusal>) {
    let contract_config = match installed_config(root) {
        Ok(config) => config,
        Err(failure) => {
            let base = match installed_config_with(root, |versioned, local| {
                Ok(Config::read_keeping_what_parses(versioned, local))
            }) {
                Ok(base) => base,
                Err(_) => {
                    let repository =
                        layer_repository_keeping_what_parses(&Config::default(), repo_dir).err();
                    return (Config::default(), Some(failure), repository);
                }
            };
            return match layer_repository_keeping_what_parses(&base, repo_dir) {
                Ok(config) => (config, Some(failure), None),
                Err(repository) => (base, Some(failure), Some(repository)),
            };
        }
    };
    match layer_repository(&contract_config, repo_dir) {
        Ok(config) => (config, None, None),
        Err(failure) => match layer_repository_keeping_what_parses(&contract_config, repo_dir) {
            Ok(config) => (config, Some(failure), None),
            Err(repository) => (contract_config, None, Some(repository)),
        },
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
/// | Integration | trunk |   in estigia.local.md
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

/// [`contract_config`] with a retired `Delegated workers` row folded.
///
/// A caller asking "what does this contract say about itself" wants the row's
/// meaning, and the meaning of a retired row is defined: each name it held
/// becomes an `inherit`. Refusing it instead made `status` call a contract that
/// `setup` could rewrite *not understood*, so the two disagreed about the same
/// file.
pub(crate) fn contract_config_folding_retired(root: &Path) -> Result<Config, Refusal> {
    Ok(Config::read_folding_retired_workers(&contract_text(root)?, None)?.0)
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
    /// The settings the versioned block explicitly names.
    pub contract_settings: Vec<crate::config::Setting>,
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
///
/// A retired `Delegated workers` row folds into the routing here rather than
/// refusing, so a contract that still carries it reads. Making the fold fatal
/// made `setup` unable to rewrite the contract the row lives in — it refused
/// every run and left the row where it was, which is the one state `setup`
/// exists to clear.
pub fn config_layers(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
) -> Result<ConfigLayers, Refusal> {
    Ok(config_layers_for_reporting(root, slug, repo_dir)?.0)
}

/// [`config_layers`] with the row a fold reported, for a caller that wants both.
pub(crate) fn config_layers_for_reporting(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
) -> Result<(ConfigLayers, Option<Refusal>), Refusal> {
    config_layers_with(root, slug, repo_dir, None)
}

/// Reads all existing layers while treating a genuinely absent contract as the
/// portable defaults an initial install will write, and carrying the retired
/// row a fold was made of.
pub fn config_layers_for_install(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
) -> Result<(ConfigLayers, Option<Refusal>), Refusal> {
    config_layers_with(root, slug, repo_dir, Some(&Config::default()))
}

/// One adapter's installed contract, with the fold it was read through.
///
/// [`installed_config_for`] is this without the report, for a caller that only
/// reads. A writer wants both: the contract it is about to render and the row
/// that is leaving the table.
pub(crate) fn installed_config_for_reporting(
    root: &Path,
    slug: Option<&str>,
) -> Result<(Config, Option<Refusal>), Refusal> {
    let (layers, report) = config_layers_for_reporting(root, slug, None)?;
    Ok((layers.unlayered().clone(), report))
}

fn config_layers_with(
    root: &Path,
    slug: Option<&str>,
    repo_dir: Option<&Path>,
    absent_contract: Option<&Config>,
) -> Result<(ConfigLayers, Option<Refusal>), Refusal> {
    let mut report = None;
    let mut contract_settings = Vec::new();
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
        contract_settings = explicit_settings(&text);
        let (config, folded) = Config::read_folding_retired_workers(&text, None)
            .map_err(|refusal| named_by(refusal, &contract_path))?;
        report = named_report(folded, &contract_path);
        config
    };
    let mut agent_settings = Vec::new();
    let mut agent_document = None;
    let agent = match slug {
        Some(slug) => {
            let path = agent_override(root, slug);
            match override_text(&path)? {
                Some(document) => {
                    let (layered, settings, folded) =
                        Config::read_scope_over_folding_retired_workers(
                            &contract,
                            &document,
                            crate::config::Scope::Agent,
                        )
                        .map_err(|refusal| named_by(refusal, &path))?;
                    report = report.or_else(|| named_report(folded, &path));
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
            let (config, folded) =
                Config::read_folding_retired_workers(&before_local.render_rows(), Some(&document))
                    .map_err(|refusal| named_by(refusal, &path))?;
            report = report.or_else(|| named_report(folded, &path));
            Some(config)
        }
        None => None,
    };
    let before_repository = local.as_ref().unwrap_or(before_local);
    let mut repository_settings = Vec::new();
    let repository = match repo_dir {
        Some(repo_dir) => match repository_snapshot(repo_dir)? {
            Some(snapshot) => {
                let (layer, folded) =
                    snapshot.layer_over_folding_retired_workers(before_repository)?;
                report = report.or_else(|| named_report(folded, &snapshot.path));
                repository_settings = layer.settings;
                Some(layer.config)
            }
            None => None,
        },
        None => None,
    };
    Ok((
        ConfigLayers {
            contract,
            contract_settings,
            agent,
            agent_settings,
            agent_document,
            local,
            local_settings,
            repository,
            repository_settings,
        },
        report,
    ))
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
/// Most adapters write to the same neutral root, so a single table
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

/// The rows in the operator's own file whose label resolves to no setting.
///
/// `estigia.local.md` is never regenerated, so a row outlives the setting it
/// names — and a typo is indistinguishable from a deleted setting, because
/// [`crate::config::Setting::from_label`] answers `None` for both and every
/// reader drops the row in a `filter_map`. Nothing refused it, warned about it
/// or reported it. This is the report: the labels, and never the values, since
/// the file holds permissions and machine paths and must not be published.
///
/// A file that cannot be opened yields nothing here. That case is not silence:
/// [`local_overrides`] refuses it by name, so every caller that reads the table
/// is already told, and this reader must not turn a report into a second
/// failure.
pub fn unknown_override_rows(root: &Path) -> Vec<String> {
    let Some(path) = local_override(root) else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    crate::config::table_rows(&text)
        .into_iter()
        .filter(|(label, value)| {
            crate::config::Setting::from_label(label).is_none()
                && crate::config::retired_setting_report(label, value).is_none()
        })
        .map(|(label, _)| label)
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
    Ok(installed_config_for_reporting(root, slug)?.0)
}

/// A writable per-agent view derived from one already-read override snapshot.
///
/// Every read here keeps what parses, so a retired routing key or a legacy row
/// in the agent's own file does not cost the whole table — and the row it could
/// not take comes back with the config rather than being discarded, so the
/// writer can name it instead of rewriting the cell in silence.
pub(crate) fn installed_config_for_agent_write(
    root: &Path,
    slug: &str,
    agent_document: Option<&str>,
) -> Result<(Config, Option<Refusal>), Refusal> {
    let contract_text = contract_text_for(root, Some(slug))?;
    let (contract, contract_report) =
        Config::read_keeping_what_parses_reporting(&contract_text, None);
    let (agent, mut report) = match agent_document {
        None => (contract, contract_report),
        Some(document) => {
            let (agent, _, scoped) = Config::read_scope_over_keeping_what_parses_reporting(
                &contract,
                document,
                crate::config::Scope::Agent,
            );
            (agent, contract_report.or(scoped))
        }
    };
    let config = match local_overrides(root)? {
        Some((_, document)) => {
            let (config, local) =
                Config::read_keeping_what_parses_reporting(&agent.render_rows(), Some(&document));
            report = report.or(local);
            config
        }
        None => agent,
    };
    Ok((config, report))
}

/// Says which file a bad row was in, without touching the rest of the refusal.
///
/// The code and the resolution are already right — `Config::read` names the
/// setting and what it accepts, and refuses to guess, which is the whole point
/// of that refusal. What it cannot know is the path, because it is handed text.
fn in_file<T>(path: &Path, read: Result<T, Refusal>) -> Result<T, Refusal> {
    read.map_err(|refusal| named_by(refusal, path))
}

/// A refusal's own words, with the file it was read from appended.
fn named_by(mut refusal: Refusal, path: &Path) -> Refusal {
    refusal.message = format!("{} in {}", refusal.message, path.display());
    refusal
}

/// A report named the same way, when there is one.
fn named_report(report: Option<Refusal>, path: &Path) -> Option<Refusal> {
    report.map(|refusal| named_by(refusal, path))
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
mod tests;
