// An assertion that panics is the assertion working. See `tests/pipe.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The honesty contract, checked against the thing it describes.
//!
//! [`docs/honesty.md`](docs/honesty.md) is the most important document in this
//! repository, and until this file existed it was the least verified: prose,
//! written by hand, describing what the code does and does not do. Every other
//! hand-written description in this crate is now crossed against its code — the
//! gate matchers against the classifier, the tool flags against the transport's
//! parser, a refusal's accepted values against the parser that takes them. This
//! one was the exception, and it is the one whose being wrong matters most: a
//! limits list nobody checks becomes a limits list nobody can trust, which is
//! worse than not writing one.
//!
//! # The proof boundary
//!
//! Only the **countable** claims can be checked here: how many agents are
//! gated, how many things `doctor` looks at, which names the code still uses.
//! The claims about *kind* — that a guard rail is not a lock, that nothing has
//! met a live tracker — are judgements, and a test that pretended to check them
//! would be the false comfort this file exists to prevent.

use estigia::config::{SETTINGS, Scope};
use std::path::PathBuf;

fn readme() -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md"))
        .expect("the README ships with the crate")
}

/// The honesty contract, which is its own document.
///
/// It lived inside the README until it was 55% of that file — a reference list
/// a reader had to scroll past to reach anything else, which is how the section
/// this file exists to verify became the one nobody finished. Read whole here:
/// the file *is* the section, so there is no heading to find and no next `## `
/// to stop at.
///
/// The length check is not decoration. Every assertion below looks for a phrase
/// *inside* this text, so a read that returned nothing would make all of them
/// pass by finding nothing to disagree with — the exact failure the extraction
/// this replaced was written to avoid.
fn honesty() -> String {
    let text =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/honesty.md"))
            .expect("docs/honesty.md ships with the crate");
    assert!(
        text.len() > 10_000,
        "docs/honesty.md is {} bytes, too short to be the contract this file checks",
        text.len()
    );
    text
}

/// Everything this repository claims about itself, in one string.
///
/// The README and the honesty contract were a single file until the contract
/// reached 55% of it. A check that asks *does the documentation still say this?*
/// does not care which half says it, and pinning every phrase to a file would
/// make this suite fail on an editorial move instead of on a drifted claim —
/// which is the opposite of what it is for.
///
/// Checks that mean *the README specifically* — the settings table, the exit
/// codes, the tool list — still read `readme()` on its own.
/// The settings table, which is its own document.
///
/// It followed the honesty contract out of the README for the same reason: a
/// reference table is looked up, not read, and 188 lines of it sat between a
/// reader and everything after it.
///
/// The guard is on the table rather than on the length, because this file's
/// checks parse it: a document that still exists but no longer holds the table
/// would make the cross-checks below compare against nothing and pass.
fn configuration() -> String {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/configuration.md"),
    )
    .expect("docs/configuration.md ships with the crate");
    assert!(
        text.contains("| Setting | Accepts |"),
        "docs/configuration.md no longer holds the settings table these tests cross against"
    );
    text
}

fn documented() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut all = readme();
    // Every document, discovered rather than listed. Naming them here meant a
    // sentence moving from the README into `docs/` broke a check that only ever
    // asked *does the documentation still say this?* — three times in one
    // afternoon, each time reported as a drifted claim it was not. A file added
    // to `docs/` joins this on its own.
    let mut docs: Vec<_> = std::fs::read_dir(root.join("docs"))
        .expect("docs/ ships with the crate")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "md"))
        .collect();
    assert!(
        docs.len() >= 2,
        "docs/ holds {} markdown files, so this reader has stopped finding them",
        docs.len()
    );
    docs.sort();
    for path in docs {
        all.push('\n');
        all.push_str(&std::fs::read_to_string(&path).expect("a readable document"));
    }
    all
}

#[test]
fn five_blind_keeps_one_enforced_verdict_and_names_every_unproved_panel_property() {
    let text = honesty().to_ascii_lowercase();
    assert!(
        readme().contains(r#"estigia config set "Blind judges" "two blind" --agent claude-code"#)
    );
    assert!(
        text.contains("one aggregate exact-receipt verdict"),
        "the honesty boundary does not preserve the transport's one-verdict floor"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill");
    for path in ["protocols/rdd.md", "policies/blind-judges.md"] {
        let contract = std::fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("{path} is unreadable: {error}"))
            .lines()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        assert!(
            contract.contains("one aggregate exact-receipt verdict"),
            "{path} contradicts the aggregate evidence floor"
        );
        if path == "protocols/rdd.md" {
            assert!(contract.contains("per-lens or per-judge verdicts"));
            assert!(contract.contains("remains future design"));
        }
    }
    for unproved in [
        "panel size",
        "concurrency",
        "independence",
        "blindness",
        "same-finding identity",
        "quorum",
        // Was "judge isolation outside the reserved role" until issue 83, which
        // removed the distinction: every judge runs as the role now, and the
        // isolation is what is unproved whichever standard the role carries.
        "judge isolation",
        "shared a working directory",
    ] {
        assert!(
            text.contains(unproved),
            "docs/honesty.md does not disclaim proof of {unproved}"
        );
    }
}

/// The number written just before `phrase`, as a word.
///
/// The claims are written for a person — "five agents", "six things" — so the
/// check has to read them the way a person does.
fn number_before(text: &str, phrase: &str) -> Option<usize> {
    let at = text.find(phrase)?;
    let window = &text[at.saturating_sub(60)..at];
    // Whole words, because "ten" sits inside "written" and "one" inside "none".
    // The old reading took the last *substring* hit, which happened to be right
    // while the words were short and would have quietly stopped being right.
    //
    // The hyphen is part of a word here, and that is the whole of what makes
    // this able to count past twenty. Splitting on it turns `twenty-one` into
    // `twenty` and `one`, and the reader takes the **last** number it sees — so
    // a README saying twenty-one reported **1**, and the count guard failed
    // naming a number nobody had written. A reading that is wrong by twenty is
    // worse than one that gives up, because the message it produces sends the
    // reader to the wrong file.
    let words: Vec<&str> = window
        .split(|character: char| !(character.is_ascii_alphabetic() || character == '-'))
        .filter(|word| !word.is_empty())
        .collect();
    [
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        // Past twelve because the crate went past it. The tools are eighteen
        // operations and `doctor` is twelve checks — one short of a ceiling
        // that answers `None` for anything above it, which every caller turns
        // into `expect("the README counts …")`. That reads as *the README has
        // no number there* when what happened is that this reader cannot count
        // that high, and it would have arrived on the day somebody added a
        // thirteenth check.
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        ("twenty", 20),
        // Hyphenated, because the words are split on whitespace and `twenty-one`
        // arrives as one of them: an entry for `twenty` does not answer for it,
        // and the reader would report *no number there* for a sentence carrying
        // one. Two past the current count rather than one, which is the same
        // ceiling the note above describes arriving on the day somebody adds a
        // thirteenth check.
        //
        // The hyphen binds both ways, and the other way is worth knowing before
        // it bites: a compound like `one-off` or `three-way` is now one word and
        // matches no entry, so a sentence counting something a few words after
        // it reads as having no number rather than the wrong one. Nothing in the
        // corpus does that today, and failing to find a number is the loud
        // direction — every caller `expect`s one — so it surfaces as a panic
        // naming the phrase rather than as a silently wrong count.
        ("twenty-one", 21),
        ("twenty-two", 22),
    ]
    .iter()
    .filter_map(|(name, value)| {
        words
            .iter()
            .rposition(|word| word.eq_ignore_ascii_case(name))
            .map(|at| (at, *value))
    })
    .max_by_key(|(at, _)| *at)
    .map(|(_, value)| value)
}

/// The settings table in the README names the rows this binary has.
///
/// It named nine of eighteen, and one of the nine was `Task body language` — a
/// **legacy alias** kept so an older contract still parses, for a row now
/// called `Summary language`. There is a second language row beside it,
/// `Issue body language`, and the alias's own note in `settings.rs` says what
/// confusing them costs: an answer lands on a row that decides something
/// different, and the row it was meant for stays at its default. A reader who
/// set what the README named got exactly that.
///
/// The count was already crossed — this file's own `README says eighteen`
/// assertion — and the count was right while the names were not, which is the
/// difference between counting a list and reading it.
/// The exit-code table in the README is the taxonomy this binary exits with.
///
/// It said **three** — `0`, `1`, `2` — for as long as there were four:
/// `ExitCode::Unreadable` is `3`, and the whole reason it is apart from `1` is
/// that the hooks read the status and treat a refusal as a decision to
/// propagate. A script written from this section treats `3` as "something went
/// wrong", which is the one sentence the section opens by disclaiming.
///
/// Counted **and** read, for the reason the settings table taught one round
/// earlier: the count of a list can be right while its contents are not.
/// The tool list in the README is the tools this server exposes.
///
/// The section opened with *Thirteen operations* and, six lines further down,
/// said *Seventeen tools* — about the same list, in the same section. The count
/// check above did not see it: `counted` takes the **last** number word in the
/// text it is given, so the sentence that was right hid the sentence that was
/// wrong.
///
/// Names, then, and not only a number. An agent reads `tools/list` rather than
/// this section, so what a stale line here costs is a person deciding whether
/// to reach for a tool that is not there — or not reaching for one that is.
/// The layout block names the paths this repository has, and the payload's own
/// line names what is in it.
///
/// It said the payload held `SKILL.md, bindings/, references/, assets/,
/// scripts/`. `scripts/` is the directory that held `github.py`, and it went
/// with the Python — in the same release whose paragraph three sections down
/// says *the script is deleted: no `.py` file in the tree, no interpreter on
/// any path*. The map still sent a reader to look for it, and did not name
/// `policies/` or `protocols/`, which are there.
///
/// Both directions, because a map is wrong when it points at nothing and wrong
/// again when it leaves something out.
/// The settings table in the README names the **values** each row takes, and
/// not only the row.
///
/// The guard beside this one crossed the names and stopped there, so the values
/// column drifted unread — and it did: `Planning` gained `auto` and four
/// spellings with it, the row here went on listing five, and every check in this
/// file stayed green. A reader who sets what the README names gets a refusal
/// from a tool whose own `--help` accepts the value; a reader who trusts the
/// README's list concludes the value does not exist.
///
/// That is the same defect the names guard was written for, one column to the
/// right, which is why this is a second assertion rather than a wider first one:
/// each says what it measures.
///
/// # What this measures, and what it leaves alone
///
/// **Closed vocabularies only** — the rows whose answers are a fixed list, which
/// is where a value can be *added in code* and go unmentioned. Those are checked
/// word by word: every choice the picker offers must appear in the row.
///
/// The open ones — `Model routing`, `Irreversible commands`, the two language
/// rows — are deliberately not held to their `accepted()` sentence. That
/// sentence names twenty-two keys for `Model routing` alone, and a table cell
/// carrying it is a table nobody reads. What that leaves unmeasured is real and
/// worth naming: an open row's README prose can be thinner than the parser's,
/// and `Model routing`'s was — *"comma-separated `key=model` pairs"*, naming no
/// key at all, on the row an operator opens looking for exactly one. Prose is
/// what fixed it, and prose is what would have to go stale again.
#[test]
fn the_settings_table_names_every_value_a_closed_row_takes() {
    let readme = documented();
    let mut silent: Vec<String> = Vec::new();
    for setting in SETTINGS {
        let answers = setting.answers();
        // Open vocabularies are out, for the reason above.
        if !answers.closed {
            continue;
        }
        let label = setting.label();
        // The row as the table holds it: `| Label | values |`.
        let Some(row) = readme
            .lines()
            .find(|line| line.starts_with(&format!("| {label} |")))
        else {
            silent.push(format!("{label}: the README has no row at all"));
            continue;
        };
        let missing: Vec<&str> = answers
            .choices
            .iter()
            .filter(|choice| !row.contains(&format!("`{choice}`")))
            .copied()
            .collect();
        if !missing.is_empty() {
            silent.push(format!("{label} does not name: {}", missing.join(", ")));
        }
    }
    assert!(
        silent.is_empty(),
        "the README's settings table names fewer values than the picker offers, so a value an \
         operator can choose is one they cannot discover:\n  {}",
        silent.join("\n  ")
    );
}

#[test]
fn model_catalog_suggestions_keep_their_honesty_boundary() {
    let readme = documented();
    let behavior = readme
        .split_once("### Model routing suggestions are agent-specific")
        .expect("the Model routing selector is documented")
        .1
        .split_once("`ask` proposes")
        .expect("the selector section has an end")
        .0
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for fact in [
        "`key=model`",
        "CLI remains the place to edit",
        "`Planning` is the last primary row",
        "Claude Code and Codex offer three reviewed presets",
        "Choosing one replaces that agent's complete model route",
        "profile row reads `custom`",
        "Choosing `custom` preserves the current route",
        "OpenCode's catalog is dynamic",
        "active `Planning::phases()`",
        "opens its concrete host's advisory model list directly",
        "`inherit` removes only that target",
        "Space remains literal inside the custom editor",
        "`opencode models`",
        "`--refresh`",
        "advisory suggestions, not validation",
        "does not change `Planning`, choose SDD",
        "shared/uniform answer",
        "currently selected agents",
        "destination's own installed route",
        "aggregates each target independently",
        "disagree on `Planning`",
        "A uniform `Planning` edit uses that same selected-agent set",
        "Successful TUI saves acknowledge only",
        "portable contract-only config",
        "operator's local override",
        "explicit row ownership",
        "`ConfigLayers::effective_over`",
        "scope-filtered before value validation",
        "forbidden hand-edited rows are ignored",
        "Invalid owned rows still",
        "never promoted into the shared contract",
        "Dynamic host artifacts such as Claude SDD definitions use the effective view",
        "one explicit row never widens into every repository setting",
        "only `NotFound`",
        "invalid UTF-8",
        "`SetupResult::completed` and `InstallReceipt::completed`",
        "One repository document snapshot",
        "write proved before a later failure remains",
        "missing or unreadable at read-back",
        "partial receipt",
        "`SetupFailure` carries every action proved",
        "`write_attempted`",
        "explicit prevalidation, preview, or mutation phase",
        "typed malformed-JSON `NotEditable` refusals",
        "preflight refusal remains `NotStarted`",
        "setup stopped before changing any files",
        "never renders as a dry-run",
        "planned actions never promote it to `Committed`",
        "later failure is `Committed`",
        "is `Unknown`",
        "continues with unaffected adapters",
        "keeps the aggregate batch `Unknown`",
        "first controlling `Unknown` supplies the aggregate code and remedy",
        "same unique paths, change kinds and count",
        "Dry-run plans are not mutation evidence",
        "dry-run writes nothing and acknowledges nothing",
        "a successful retry replaces the pending refusal",
        "model panel derives one viewport offset",
        "both reset to zero",
        "opening a row picker transfers ownership back",
        "Space on the agent choice remains an ordinary agent toggle",
        "Help uses the same runtime translation table",
        "5.6 seconds",
        "PATH resolution",
        "`1 MiB`",
        "strict UTF-8",
        "best-effort descendant cleanup",
        "Every completed direct process",
    ] {
        assert!(
            behavior.contains(fact),
            "the Model routing behavior no longer states {fact:?}"
        );
    }

    let contract = honesty();
    let honesty = contract.split_whitespace().collect::<Vec<_>>().join(" ");
    for boundary in [
        "Model catalogs do not measure runtime availability or capability",
        "does not validate catalog membership",
        "execute models",
        "filter their tool-call capability",
    ] {
        assert!(
            honesty.contains(boundary),
            "the honesty contract no longer names {boundary:?}"
        );
    }
    let exclusively_materializes_claude = |claim: &str| {
        claim.contains("Only Claude Code currently receives host-routable definitions")
            && claim
                .contains("OpenCode and every other host keep these values as routing declarations")
    };
    assert!(
        exclusively_materializes_claude(&honesty),
        "the honesty contract does not say exclusively that Claude Code receives phase definitions \
         while all other hosts retain routing declarations"
    );
    for broadened in [
        honesty.replace("Only Claude Code", "Claude Code"),
        honesty.replace("Only Claude Code", "Only Claude Code and OpenCode"),
        honesty.replace("every other host", "some other hosts"),
    ] {
        assert!(
            !exclusively_materializes_claude(&broadened),
            "the exclusive materialization guard accepted broadened wording"
        );
    }
}

#[test]
fn the_layout_block_names_what_is_here_and_only_that() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = std::fs::read_to_string(root.join("README.md")).expect("the README reads");
    let block = readme
        .split_once("## Layout")
        .expect("the layout section is where it was")
        .1;
    let block = block
        .split_once("```")
        .expect("the block is fenced")
        .1
        .split_once("```")
        .expect("the fence closes")
        .0;

    // Every path it names is one that is here.
    for line in block.lines().filter(|line| !line.trim().is_empty()) {
        let named = line.split_whitespace().next().expect("a path");
        assert!(
            root.join(named).exists(),
            "the layout names `{named}`, which is not in this repository"
        );
    }

    // And the payload's own line names every directory the payload has.
    let listed: std::collections::BTreeSet<String> = block
        .lines()
        .find(|line| line.trim_start().starts_with("skill/"))
        .expect("the payload has a line")
        .split(&[',', ' '][..])
        .filter(|part| part.ends_with('/') && *part != "skill/")
        .map(str::to_owned)
        .collect();
    let here: std::collections::BTreeSet<String> = std::fs::read_dir(root.join("skill"))
        .expect("the payload reads")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .is_dir()
                .then(|| format!("{}/", entry.file_name().to_string_lossy()))
        })
        .collect();
    assert_eq!(
        listed, here,
        "the payload's line is not the payload's directories"
    );
}

#[test]
fn every_tool_the_readme_lists_is_one_this_server_exposes() {
    let readme =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
            .expect("the README reads");
    let block = readme
        .split_once("## The tools")
        .expect("the tools section is where it was")
        .1
        .split_once("```")
        .expect("the list is fenced")
        .1
        .split_once("```")
        .expect("the fence closes")
        .0;
    let mut listed: Vec<&str> = block
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .collect();
    listed.sort_unstable();

    let mut exposed: Vec<&str> = estigia::harness::mcp::tools::TOOLS
        .iter()
        .map(|tool| tool.name)
        .collect();
    exposed.sort_unstable();
    assert_eq!(
        listed, exposed,
        "the README's tool list is not the one `tools/list` answers with"
    );

    // And the sentence that opens the section counts them.
    assert!(
        readme.contains(&format!(
            "{} operations, table-driven",
            match exposed.len() {
                17 => "Seventeen",
                18 => "Eighteen",
                20 => "Twenty",
                21 => "Twenty-one",
                other => panic!("{other} tools, and this test has no word for that"),
            }
        )),
        "the sentence that opens the section does not say there are {}",
        exposed.len()
    );
}

#[test]
fn every_exit_code_the_readme_lists_is_one_this_binary_has() {
    use estigia::outcome::ExitCode;

    let readme =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
            .expect("the README reads");
    let table = readme
        .split_once("| Code | Meaning |")
        .expect("the exit-code table is where it was")
        .1;
    let listed: Vec<u8> = table
        .lines()
        .skip(1)
        .take_while(|line| line.starts_with('|'))
        .filter_map(|line| line.split('|').nth(1))
        .filter_map(|cell| cell.trim().trim_matches('`').parse().ok())
        .collect();

    let mine: Vec<u8> = [
        ExitCode::Success,
        ExitCode::Refused,
        ExitCode::Indeterminate,
        ExitCode::Unreadable,
    ]
    .into_iter()
    .map(|code| code as u8)
    .collect();
    assert_eq!(
        listed, mine,
        "the README's exit codes are not the ones this binary exits with"
    );

    // And the sentence above the table counts them, in words.
    let word = match mine.len() {
        3 => "Three",
        4 => "Four",
        5 => "Five",
        other => panic!("{other} codes, and this test has no word for that"),
    };
    assert!(
        readme.contains(&format!("{word}, and none of them means")),
        "the sentence above the table does not say there are {}",
        mine.len()
    );
}

#[test]
fn every_setting_the_readme_tabulates_is_one_this_binary_has() {
    let readme = configuration();
    let table = readme
        .split_once("| Setting | Accepts |")
        .expect("the settings table is where it was")
        .1;
    let tabulated: Vec<&str> = table
        .lines()
        .skip(1)
        .take_while(|line| line.starts_with('|'))
        .filter_map(|line| line.split('|').nth(1))
        .filter(|cell| !cell.trim().starts_with('-'))
        .map(str::trim)
        .collect();

    let labels: Vec<&str> = estigia::config::SETTINGS
        .iter()
        .map(|setting| setting.label())
        .collect();
    assert_eq!(
        tabulated, labels,
        "the README's settings table is not this binary's rows, in its order"
    );
}

#[test]
fn the_number_of_gated_agents_is_the_number_the_readme_claims() {
    let readme = documented();
    let gated = estigia::setup::AGENTS
        .iter()
        .filter(|adapter| adapter.can_gate_tools())
        .count();

    if let Some(claimed) = number_before(&readme, "agents of seven") {
        assert_eq!(
            gated, claimed,
            "the README claims {claimed} gated agents and {gated} are gated"
        );
        return;
    }

    // The other form the claim takes. It is only true while every entry that is
    // not gated is not an agent.
    assert!(
        readme.contains("every agent Estigia knows"),
        "the README makes no checkable claim about how many agents are gated"
    );
    let ungated: Vec<&str> = estigia::setup::AGENTS
        .iter()
        .filter(|adapter| !adapter.can_gate_tools())
        .map(|adapter| adapter.slug)
        .collect();
    assert_eq!(
        ungated,
        vec!["agents"],
        "the README claims every agent is gated, and these are not: {ungated:?}"
    );

    // The hole this used to have. Satisfying the sentence above ended the check,
    // and the rest of the same sentence went on counting out loud: it said "six
    // of them, in three dialects" while ten were gated in five. A claim that is
    // half-checked reads exactly like one that is checked.
    let counted = number_before(&readme, "of them, in")
        .expect("the README counts the gated agents beside the claim that all of them are");
    assert_eq!(
        gated, counted,
        "the README says {counted} agents are gated and {gated} are"
    );

    let dialects = estigia::harness::hook::Dialect::all().len();
    let claimed = number_before(&readme, "dialects").expect("the README counts the dialects");
    assert_eq!(
        dialects, claimed,
        "the README claims {claimed} dialects and the code has {dialects}"
    );
}

#[test]
fn the_number_of_declared_populations_is_the_number_the_readme_claims() {
    // `tests/guards.rs` checks that every declaration is well formed and still
    // affirmed. Nothing checked that the README's count of them was true, so
    // adding `writing-shell` would have left the prose saying two.
    let readme = documented();
    let claimed = number_before(&readme, "`guard:population` comments")
        .expect("the README counts the declared populations");

    let mut declared = 0;
    let mut pending = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("a readable source directory") {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                // A *declaration* sits in a doc comment, which is what binds it
                // to an item and what `tests/guards.rs` fingerprints. A `//`
                // line mentioning a family is a reference — three tests point
                // back at the population they exercise — and counting those
                // would inflate the number every new test could change.
                declared += std::fs::read_to_string(&path)
                    .expect("a readable source")
                    .lines()
                    .filter(|line| {
                        let line = line.trim_start();
                        line.starts_with("/// guard:population ")
                            || line.starts_with("//! guard:population ")
                    })
                    .count();
            }
        }
    }

    assert_eq!(
        declared, claimed,
        "the README claims {claimed} declared populations and the source has {declared}"
    );
}

#[test]
fn the_number_of_things_doctor_checks_is_the_number_the_readme_claims() {
    let readme = documented();
    let claimed = number_before(&readme, "things, not everything")
        .expect("the README counts doctor's checks");
    // The **kinds**, not the rows. Two of the eleven produce one row per
    // configured agent, so a run on a bare machine reports seven rows and a run
    // on a busy one reports twenty — and the README's count was being measured
    // against `examine`, which answers six while the command prints eight. A
    // claim checked at the wrong scope, in the file that exists to catch
    // exactly that.
    let actual = estigia::harness::doctor::CHECKS.len();

    assert_eq!(
        actual, claimed,
        "the README says `doctor` checks {claimed} things and it checks {actual}"
    );

    // And the list is the truth about the command, not a second thing to keep
    // in step: every row `full` emits reports under one of those names.
    let root = tempfile::tempdir().expect("a temporary directory");
    let home = tempfile::tempdir().expect("a temporary home");
    let options = estigia::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join("config")),
        app_data: Some(home.path().join("appdata")),
        ..estigia::setup::SetupOptions::default()
    };
    for check in estigia::harness::doctor::full(
        Some(root.path()),
        root.path(),
        &estigia::config::Tracker::Github { repo: None },
        &options,
    ) {
        assert!(
            estigia::harness::doctor::CHECKS.contains(&check.name),
            "`doctor` reports {:?}, which the list does not declare",
            check.name
        );
    }
}

#[test]
fn every_mechanism_the_readme_names_is_one_the_code_still_uses() {
    // A name that has drifted sends a reader to look for something that is not
    // there — and these are the names somebody greps for when a gate did not
    // fire.
    let readme = documented();
    for phrase in [
        "PreToolUse",
        "BeforeTool",
        "preToolUse",
        "tool.execute.before",
        "permissionDecision",
        "pre-push",
    ] {
        assert!(
            readme.contains(phrase),
            "the README no longer names `{phrase}`, which the code still uses"
        );
    }
}

#[test]
fn every_agent_the_readme_leaves_ungated_says_why_in_the_tool() {
    // The README is where somebody reads the limits before installing; the tool
    // is where they read them after a write went through. Neither is allowed to
    // know something the other does not.
    for adapter in estigia::setup::AGENTS {
        if adapter.can_gate_tools() {
            assert!(
                adapter.gate_gap().is_none(),
                "{} is gated and still explains why it is not",
                adapter.slug
            );
        } else {
            assert!(
                adapter.gate_gap().is_some_and(|gap| gap.len() > 60),
                "{} has no gate and the tool says nothing useful about why",
                adapter.slug
            );
        }
    }
}

#[test]
fn the_honesty_contract_still_exists_and_names_what_is_not_covered() {
    // The guard on the guard. A section that gets deleted takes every check
    // above with it, silently, because they would all find nothing to compare
    // against and pass.
    let contract = honesty();
    let section = contract.as_str();

    let limits = section
        .lines()
        .filter(|line| line.starts_with("- **"))
        .count();
    assert!(
        limits >= 8,
        "the honesty contract lists only {limits} limits; it has never got shorter except by a \
         thing being fixed and its entry going with it"
    );
}

#[test]
fn the_documents_count_what_the_crate_actually_has() {
    // This crossed the working log, which stated four counts in one table. That
    // document is kept outside the repository now, so the test that read it was
    // green on the machine that writes it and failed on the first clone -- the
    // worst way round, and the same shape as every other failure this file
    // exists to catch.
    //
    // Repointed at the documents a clone actually has. Two of the four counts
    // survive here: the settings count is crossed by
    // `the_readme_counts_what_the_crate_actually_has`, and the companion count
    // is claimed nowhere in the repository now that the log has gone. The
    // companion *table* is still covered, by `setup::tests`; it is only the
    // number that no published sentence asserts, and inventing one to give this
    // test something to check would be writing documentation for a test to read.
    let documents = documented();
    for (phrase, actual, what) in [
        (
            "agents; the hooks",
            estigia::setup::AGENTS.len(),
            "agent adapters",
        ),
        (
            "operations with schemas",
            estigia::harness::mcp::TOOLS.len(),
            "MCP tools",
        ),
    ] {
        let claimed = number_before(&documents, phrase).unwrap_or_else(|| {
            panic!(
                "no document carries a number before {phrase:?}, so the {what} count is unchecked"
            )
        });
        assert_eq!(
            claimed, actual,
            "the documents claim {claimed} {what} and the crate has {actual}"
        );
    }
}

/// One function's body, from its signature to the next item at column zero.
///
/// Public **or** private. It read only `pub fn` while every body it was asked
/// for was an entry point; a body shared by two entry points is not one, and
/// panicking `publish_with is not in that file` would have read as the function
/// having been deleted rather than as this helper not looking for it.
fn body_of(source: &str, name: &str) -> String {
    let start = [format!("pub fn {name}("), format!("\nfn {name}(")]
        .iter()
        .find_map(|signature| source.find(signature).map(|at| at + signature.len()))
        .unwrap_or_else(|| panic!("{name} is not in that file"));
    let rest = &source[start..];
    let end = rest
        .match_indices("\npub fn ")
        .chain(rest.match_indices("\nfn "))
        .map(|(at, _)| at)
        .min()
        .unwrap_or(rest.len());
    rest[..end].to_owned()
}

#[test]
fn no_comment_spells_a_character_it_could_just_write() {
    // A unicode escape written inside a `//` is not an escape. It is the eight
    // characters that spell one, and `cargo doc` publishes them verbatim — 21
    // of the 113 this crate had were in `///`, which is its public documentation.
    //
    // They arrive from scripts that edit this source, where the same eight
    // characters *are* an escape one language up. Nothing tells the two apart by
    // reading, so this reads for them.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut spelled: Vec<String> = Vec::new();
    let mut read = 0usize;
    let mut stack = vec![root.join("src"), root.join("tests")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs") {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                read += 1;
                for (number, line) in text.lines().enumerate() {
                    if line.trim_start().starts_with("//") && line.contains("\\u{") {
                        spelled.push(format!("{}:{}", path.display(), number + 1));
                    }
                }
            }
        }
    }
    assert!(read > 40, "the walk stopped finding source: {read} read");
    assert!(
        spelled.is_empty(),
        "these comments spell an escape instead of writing the character: {spelled:?}"
    );
}

#[test]
fn nothing_the_transport_stages_shares_a_name_between_runs() {
    // Every staged body used to be a fixed name under the temporary directory,
    // and one — the symlink probe `expected-target` hashes — had no
    // discriminator at all. On Windows that directory is per-user and nothing
    // came of it. On Unix it is `/tmp`, shared by the whole machine, and this
    // crate exists for the case of two runs on one machine: they overlap, one
    // hashes the other's link text, and the manifest a reviewer's approval is
    // bound to records a blob for a path that never had it.
    //
    // Read at the source, because the population is *which names get staged*
    // and a run only ever reaches the ones it needs.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut fixed: Vec<String> = Vec::new();
    let mut read = 0usize;
    let mut stack = vec![root.join("src").join("transport")];
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
            // A `tests.rs` may put whatever it likes where it likes, and
            // `board.rs` holds the mirror cache, which is shared between runs
            // on purpose: a cache only one process can read is not one.
            if !path.extension().is_some_and(|kind| kind == "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
                || path.file_name().is_some_and(|name| name == "board.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            read += 1;
            for (number, line) in text.lines().enumerate() {
                if line.contains("temp_dir()") {
                    fixed.push(format!("{}:{}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        read > 5,
        "the walk stopped finding the transport: {read} read"
    );
    assert!(
        fixed.is_empty(),
        "these stage a file under a name another run would pick too — \
         `paths::scratch_file` is the one that does not: {fixed:?}"
    );
}

#[test]
fn no_file_this_crate_ships_hides_from_a_search() {
    // `src/transport/commands.rs` held a literal NUL byte, inside what was meant
    // to be `split('\0')` and was written as the byte itself. Rust does not
    // care. ripgrep does: it classifies the file as binary and skips it
    // *silently*, so every search over this repository — including the ones
    // these audits are made of — answered as though 45 KB of the transport were
    // not there. It answered "no matches" for a call that is on line 174.
    //
    // A tool that reports nothing found in a file it never opened is worse than
    // one that fails, because the reader cannot tell the two apart. Nothing here
    // needs a raw control byte, so nothing here may have one.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut hidden: Vec<String> = Vec::new();
    let mut read = 0usize;
    let mut stack = vec![root.join("src"), root.join("tests"), root.join("skill")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                read += 1;
                if std::fs::read(&path).is_ok_and(|bytes| bytes.contains(&0)) {
                    hidden.push(path.display().to_string());
                }
            }
        }
    }
    // The guard on the guard: a walk that reaches nothing refutes nothing, and
    // a renamed directory is how it would come to.
    assert!(read > 50, "the walk stopped finding files: {read} read");
    assert!(
        hidden.is_empty(),
        "a search over this crate skips these files without saying so: {hidden:?}"
    );
}

#[test]
fn every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer() {
    // A `reason` is not a log line. It is the word the agent branches on, and
    // `SKILL.md` and the bindings say what to do for each — so a reason only
    // the port can produce is a refusal with no instructions attached.
    //
    // `may_occupy` had one. It answered three different situations with a
    // single invented `worktree-not-owned-by-this-run` where the transport
    // gives `worktree-path-occupied`, `worktree-ownership-unproven` and
    // `worktree-owned-by-another-run`, all three named in the prose and all
    // three carrying different instructions — including the only recovery of
    // the set, which the collapse withheld.
    //
    // Checked at the source rather than by running: the population is *which
    // refusals exist*, and a run only ever shows the one it reached.
    // The reference implementation used to count as a place a reason could be
    // explained, which made this weaker than it read: a word only `github.py`
    // spelled satisfied it. That file is gone, and with it the loophole — a
    // reason the port can give has to be in the prose an agent reads.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut prose = String::new();
    let mut stack = vec![root.join("skill")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "md")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                prose.push_str(&text);
            }
        }
    }

    let mut unknown: Vec<String> = Vec::new();
    let mut read = 0usize;
    let mut crossed = 0usize;
    let mut files = vec![root.join("src").join("transport")];
    while let Some(directory) = files.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.push(path);
                continue;
            }
            // A `tests.rs` may invent whatever it needs to provoke a branch.
            if path.file_name().is_some_and(|name| name == "tests.rs")
                || !path.extension().is_some_and(|kind| kind == "rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            read += 1;
            for reason in reasons_in(&text) {
                crossed += 1;
                if !prose.contains(&reason) {
                    unknown.push(format!("{}: {reason}", path.display()));
                }
            }
        }
    }
    // Both floors. The walk skips a directory or a file it cannot read, and an
    // empty walk produces an empty `unknown` — which is the same answer as a
    // clean one. Everything here ships with the crate, so "there was nothing to
    // read" is a broken checkout rather than a passing test.
    assert!(
        read > 8 && crossed > 30,
        "{read} port source(s) were read and {crossed} refusal(s) crossed, which is fewer than \
         this crate has \u{2014} a walk that finds nothing reports exactly what a clean one does"
    );
    // The ones already known to be undocumented do not fail this. New ones do.
    //
    // **This is a gap, and it is open.** The check used to accept a reason
    // spelled anywhere in `github.py` as documented, and the binding's source
    // is not something an agent reads — it stood in for the prose and hid
    // forty-seven refusals that the prose has never named. Deleting the binding
    // did not create them; it stopped concealing them, and the honesty contract
    // carries the number.
    //
    // Frozen rather than dropped, because the direction that matters still
    // holds: a *new* refusal with no instructions is red on the review that
    // adds it, and the list can only be made shorter.
    let fresh: Vec<&String> = unknown
        .iter()
        .filter(|entry| {
            !UNDOCUMENTED_REFUSALS
                .iter()
                .any(|known| entry.ends_with(known))
        })
        .collect();
    assert!(
        fresh.is_empty(),
        "these refusals are spelled by nothing the agent reads: {fresh:?}"
    );
    // And the list is not allowed to rot: an entry that no longer names a
    // refusal is one somebody documented, or renamed, and either way the line
    // stops describing the code.
    let stale: Vec<&&str> = UNDOCUMENTED_REFUSALS
        .iter()
        .filter(|known| !unknown.iter().any(|entry| entry.ends_with(**known)))
        .collect();
    assert!(
        stale.is_empty(),
        "these are listed as undocumented and are not: {stale:?} \u{2014} take them off the list"
    );
}

/// The refusals the port can answer with that no prose an agent reads names.
///
/// Measured, on the day `skill/scripts/github.py` was deleted. The check above
/// counted that file as documentation, which it never was for anybody running
/// an agent — so this is not a regression, it is a measurement that was being
/// prevented. See the README's honesty contract.
const UNDOCUMENTED_REFUSALS: &[&str] = &[
    "already-owned-by-different-operation",
    "ambiguous-changelog-entry",
    "ambiguous-open-prs",
    "board-empty-inconclusive",
    "board-readback-failed",
    "branch-locked-by-another-run",
    "changelog-not-found",
    "claim-operation-expired",
    "claim-operation-no-longer-current",
    "closing-keyword-live",
    "comment-body-invalid",
    "comment-marker-incomplete",
    "empty-changelog-entry",
    "force-reason-invalid",
    "force-reason-required",
    "force-required-for-reason",
    "heartbeat-body-invalid",
    "held-by-other-holder-disappeared",
    "held-by-other-without-other-holder",
    "holder-not-stale",
    "holder-runtime-missing",
    "invalid-horizon",
    "invalid-marker-attribute",
    "invalid-operation-id",
    "invalid-unassign-target",
    "issue-not-open",
    "label-readback-failed",
    "lost-claim-race",
    "lost-reclaim-race",
    "missing-pr-body",
    "no-changelog-entry",
    "no-worktree-location",
    "nothing-to-unassign",
    "operation-id-kind-conflict",
    "ownership-changed-projections-repaired",
    "ownership-kept-changing-before-projection",
    "publication-readback-disagrees",
    "reclaim-metadata-mismatch",
    "reclaim-operation-no-longer-current",
    "reserved-comment-kind",
    "reserved-device-component",
    "review-target-mismatch",
    "stale-foreign-requires-reclaim",
    "target-operation-mismatch",
    "unassign-metadata-mismatch",
    "unsafe-run-id",
    "unsafe-worktree-component",
];

/// Every `"reason": "..."` literal in a chunk of source.
fn reasons_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("\"reason\"") {
        rest = &rest[at + "\"reason\"".len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(':') else {
            continue;
        };
        let after = after.trim_start();
        let Some(after) = after.strip_prefix('"') else {
            continue;
        };
        let word: String = after
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
            .collect();
        if !word.is_empty() && after[word.len()..].starts_with('"') {
            found.push(word);
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The subcommands the retired binding renewed the claim inside, by name.
///
/// Measured off `github.py` while it was here — every `cmd_*` whose body called
/// `do_verify_claim` — and kept as a list because the file it was measured from
/// is gone. It is the *population* the check below walks, and a population that
/// can no longer reopen itself is one that has to say so out loud.
const ADJUDICATED_BY_THE_BINDING: &[&str] = &[
    "verify_claim",
    "heartbeat",
    "start_branch",
    "publish_review",
    // Not measured off `github.py`, which never had it: the retired binding had
    // no republish at all, which is the defect the operation was added to close.
    // It is in the population because it is a boundary that adjudicates, and the
    // sentence above says a boundary added to the port has to be added here by
    // the review that adds one.
    "republish_review",
    "release_ci",
];

/// Entry points whose adjudication lives in a body they share, and how many
/// verifications that body must hold.
///
/// `publish_review` and `republish_review` differ in the push and in nothing
/// else, so the verification either performs is written **once**, in the body
/// they both call. Following one named hop is what keeps this measuring the same
/// thing after the split — but following it *and asking only whether the word
/// appears* is what broke it.
///
/// Measured, and this is the whole reason for the third column. Before the
/// split, `publish_review`'s body held exactly one `verify_claim(` and deleting
/// it turned this test red. After it, the shared body holds **two**: the
/// entry-level one that both routes run, and the pre-push renewal that only the
/// leased route reaches. `contains` is satisfied by either, so deleting the
/// entry-level verification — taking claim adjudication off the ordinary
/// publication path entirely — left the **whole suite green**. The guard written
/// to catch exactly that was answered by a call the mutated route never makes.
///
/// So the count is pinned — and pinning it is **not** what makes the ordinary
/// publication safe, which is the correction a third review round had to make.
/// A count measures arity; the property is reachability, and they are not the
/// same. Both verifications can be moved behind `if matches!(push,
/// Push::Leased { .. })`, leaving the total at two and `publish_review`
/// adjudicating nothing at all — measured, and the whole suite stayed green.
/// A source-text count cannot see a conditional, and no amount of pinning will
/// teach it to.
///
/// What holds that property is a **behavioural** test:
/// `pipe::a_publication_refuses_at_entry_when_the_claim_has_moved` denies the
/// first timeline read and proves the branch never reaches the remote. It is
/// red for all three shapes — the call deleted, its result discarded, or the
/// call moved behind the discriminant.
///
/// This stays anyway, and the division of labour is worth stating rather than
/// leaving to be re-derived: the behavioural test proves the *ordinary* route
/// adjudicates, and the count is what still notices a verification quietly
/// added or removed from a body two routes share. Neither covers the other.
/// Both rows name the same body and the same count today, so this measures one
/// thing twice — kept as two rows because the population above is entry points,
/// and an entry point that stops sharing the body is the case the count is for.
const ADJUDICATED_THROUGH: &[(&str, &str, usize)] = &[
    ("publish_review", "publish_with", 2),
    ("republish_review", "publish_with", 2),
];

#[test]
fn every_boundary_the_binding_adjudicates_is_one_the_port_adjudicates_too() {
    // The Rust transport is a port of `scripts/github.py`, and the differential
    // oracle is what keeps the two answering the same. It compares by *running*
    // subcommands, and it never runs `start-branch` — so the port could omit
    // the one thing that command is for, and did: `cmd_start_branch` opens with
    // `do_verify_claim` and `start_branch` had no verification at all.
    //
    // Nothing caught it. The MCP server shells out to the binding, so the port
    // is unreached today and the live behaviour was always Python's; the day it
    // is switched on, a worktree and a remote branch would have been the first
    // write nobody checked — which is the phrase the `start_branch` tool's own
    // description uses for what it prevents.
    //
    // Checked at the source, because the population is *which boundaries are
    // adjudicated*, and that is a property of the pair rather than of a run.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // The population was read out of the binding: every `cmd_*` whose body
    // called `do_verify_claim`. The binding is gone, so it is written down —
    // and written down is what it is now, not what it measures. A boundary
    // added to the port after this line will not appear here by itself, and the
    // review that adds one is where it has to be added.
    //
    // What did not weaken: a `verify_claim` taken *out* of any of these is
    // still red, which is the direction the incident went. `start_branch` had
    // no verification at all while `cmd_start_branch` opened with one, and the
    // oracle never ran that subcommand.
    let adjudicated: Vec<String> = ADJUDICATED_BY_THE_BINDING
        .iter()
        .map(|name| (*name).to_owned())
        .collect();

    let claim = std::fs::read_to_string(root.join("src").join("transport").join("claim.rs"))
        .expect("the port ships with the crate");
    let branch = std::fs::read_to_string(root.join("src").join("transport").join("branch.rs"))
        .expect("the port ships with the crate");

    for name in &adjudicated {
        // `verify_claim` is the boundary rather than a caller of one.
        if name == "verify_claim" {
            continue;
        }
        let source = if name == "start_branch" {
            &branch
        } else {
            &claim
        };
        let through = ADJUDICATED_THROUGH.iter().find(|(entry, ..)| entry == name);
        let body = body_of(source, through.map_or(name, |(_, shared, _)| shared));
        let found = body.matches("verify_claim(").count();
        // One is the floor for a body that adjudicates at all. Where the body is
        // shared, the exact count is the floor instead, because a body reached by
        // two routes can satisfy `at least one` with a call only one of them
        // makes — which is how deleting the ordinary publication's verification
        // came to leave every test in this repository green.
        let wanted = through.map_or(1, |(.., count)| *count);
        assert!(
            found >= wanted,
            "`cmd_{name}` adjudicates the claim and `{name}` reaches {found} verification(s) \
             where {wanted} are required \u{2014} and the differential oracle does not run that \
             subcommand, so nothing else would say so"
        );
        assert!(
            through.is_none() || found == wanted,
            "`{name}` now shares a body holding {found} verifications rather than {wanted}. A new \
             adjudication point in the publication path is not a thing to absorb silently: say \
             which route reaches it and update the count."
        );
    }
}

#[test]
fn every_clock_the_port_is_handed_says_where_it_must_come_from() {
    // `now` decides whether a claim is still live. The binding takes it from
    // `utc_now_stamp()`; the port takes it as an argument, so that a test can
    // stand at a chosen moment. That is a reasonable trade and a dangerous one
    // to leave unwritten: a run that supplies this value decides whether its
    // own claim has expired, and one of the five declarations used to describe
    // it as *this run's clock reading* — pointing at exactly the source that
    // must never be used.
    //
    // Checked at the source because there is nothing else to check it against:
    // nothing in production constructs these structs yet, so the requirement
    // has no runtime to be enforced by. The day something does, this is what
    // the person wiring it will read.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("transport");
    let mut declarations = 0;
    for entry in std::fs::read_dir(&root).expect("the transport is a directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|kind| kind != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a source file");
        for (at, _) in source.match_indices("pub now:") {
            declarations += 1;
            // The doc comment sits immediately above; take the lines before it.
            let before = &source[..at];
            let doc: String = before.lines().rev().take(12).collect::<Vec<_>>().join("\n");
            assert!(
                doc.contains("never from the run being judged"),
                "{}: a `now` is declared without saying where it must come from",
                path.display()
            );
        }
    }
    assert!(
        declarations >= 5,
        "the search found {declarations} clocks, which is fewer than the port has"
    );
}

/// Reads one of this crate's own source files.
///
/// Line endings normalised: some files in this crate are CRLF and some are LF,
/// and a scan that anchors on a closing brace at the start of a line would
/// silently measure nothing on half of them.
fn source(relative: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("{relative} ships with the crate: {error}"))
        .replace("\r\n", "\n")
}

/// The identifier following each occurrence of `prefix` in `text`, with the
/// byte it started at.
fn named_after(text: &str, prefix: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(hit) = text[at..].find(prefix) {
        let start = at + hit + prefix.len();
        let name: String = text[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            found.push((at + hit, name));
        }
        at = start.max(at + hit + 1);
    }
    found
}

/// Every setting the gate reads is a fact about the repository, not about one
/// agent.
///
/// Eight of the ten adapters share one skill root, so the only way to say
/// "claude-code differs from opencode" is a per-agent file — and the gate does
/// not read one. `config set --agent` already refuses a `Scope::Everywhere`
/// row for that reason, and says why: such a setting "has one answer, and the
/// gate reads it without asking which agent is holding the tools."
///
/// The other direction was unguarded. Making a gate-read setting `Scope::Agent`
/// would let `config set --agent` write it, `config list --agent` read it back,
/// and the gate ignore it — an operator narrowing one agent's boundaries and
/// getting the wide ones, told nothing. Every symptom points at the agent's
/// file being wrong, and the file is right.
///
/// Both populations are derived rather than listed: which fields the gate reads
/// comes from `gate_context`'s own body, and which setting writes each field
/// comes from `Setting::apply`'s own arms. A field added to either is covered
/// the day it is added.
#[test]
fn every_setting_the_gate_reads_is_one_that_does_not_differ_by_agent() {
    // What the gate takes off the contract, from the function that takes it.
    let cli = source("src/cli/mod.rs");
    let opens = cli
        .find("fn gate_context(")
        .expect("the gate builds its context somewhere");
    let body = &cli[opens..];
    let closes = body.find("\n}\n").expect("the function ends");
    let body = &body[..closes];
    assert!(
        body.contains("Ok(harness::GateContext"),
        "the slice is not the whole function, so what it does not mention proves nothing"
    );
    let mut reads: Vec<String> = named_after(body, "installed.")
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    reads.sort();
    reads.dedup();
    assert!(
        reads.len() >= 3,
        "the gate reads {reads:?} off the contract, which is too few to be the whole of it"
    );

    // Which setting writes each field, from the arms that write them. Only
    // assignments: `value_of` mentions the same fields to render them, and
    // rendering a row is not owning it.
    let settings = source("src/config/settings.rs");
    // `parse_into`, not `apply`: `apply` delegates to it, and slicing `apply`
    // yielded two lines and no arms at all. The floor below is what said so.
    let opens = settings
        .find("fn parse_into(")
        .expect("settings are parsed into a configuration somewhere");
    let arms = &settings[opens..];
    let closes = arms.find("\n    }\n").expect("the method ends");
    let arms = &arms[..closes];
    assert!(
        arms.contains("Self::Boundaries"),
        "the slice is not the whole match, so a setting it omits would read as agent-safe"
    );
    let variants = named_after(arms, "Self::");
    let mut writes: Vec<(String, String)> = Vec::new();
    for (at, field) in named_after(arms, "config.") {
        let after = arms[at + "config.".len() + field.len()..].trim_start();
        if !after.starts_with('=') || after.starts_with("==") {
            continue;
        }
        let Some((_, variant)) = variants.iter().rev().find(|(where_, _)| *where_ < at) else {
            continue;
        };
        writes.push((field, variant.clone()));
    }
    assert!(
        writes.len() >= 10,
        "only {} settings were seen to write anything: {writes:?}",
        writes.len()
    );

    let mut wrong: Vec<String> = Vec::new();
    let mut checked = 0;
    for field in &reads {
        let Some((_, variant)) = writes.iter().find(|(written, _)| written == field) else {
            // Not a hole to skip past: a field the gate reads and no setting
            // writes is either dead or written somewhere this test cannot see,
            // and both mean it stopped measuring.
            wrong.push(format!(
                "the gate reads `installed.{field}` and no arm of `apply` writes it"
            ));
            continue;
        };
        let Some(setting) = SETTINGS
            .iter()
            .find(|setting| format!("{setting:?}") == *variant)
        else {
            wrong.push(format!("`Self::{variant}` is not in SETTINGS"));
            continue;
        };
        checked += 1;
        if setting.scope() != Scope::Everywhere {
            wrong.push(format!(
                "`{}` writes `{field}`, which the gate reads, but its scope is {:?} — \
                 `config set --agent` would take it and the gate would not",
                setting.label(),
                setting.scope()
            ));
        }
    }
    assert!(
        checked >= 3,
        "only {checked} of the gate's settings were resolved, so this refutes nothing"
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Every name the list declares is one `doctor` can actually report.
///
/// The way back. Its neighbour walks the rows `full` emits and proves the list
/// declares each one; this walks the list and proves `doctor` can produce it.
/// Only one of the two was written, and the unwritten one is the direction the
/// README leans on: the count in *"`doctor` checks eleven things"* comes from
/// this list, so a name left here after its check was taken out of `full`
/// inflates a number the honesty contract exists to keep true — and the
/// neighbour goes on passing, because it only ever looks at rows that exist.
///
/// With an agent configured, and that is the whole reason this is a separate
/// test: two of the eleven produce one row per configured agent and appear in
/// nothing on a bare machine. Crossing this direction against an empty home
/// would report `gate` and `tools` as phantoms every time.
#[test]
fn every_name_this_list_declares_is_one_doctor_can_report() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let home = tempfile::tempdir().expect("a temporary home");
    let options = estigia::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join("config")),
        app_data: Some(home.path().join("appdata")),
        ..estigia::setup::SetupOptions::default()
    };
    let adapter = estigia::setup::find_agent("claude-code").expect("a declared agent");
    estigia::setup::setup(adapter, &estigia::config::Config::default(), &options)
        .expect("an agent to report per-agent rows about");

    let reported: Vec<&str> = estigia::harness::doctor::full(
        Some(root.path()),
        root.path(),
        &estigia::config::Tracker::Github { repo: None },
        &options,
    )
    .iter()
    .map(|check| check.name)
    .collect();

    for name in estigia::harness::doctor::CHECKS {
        assert!(
            reported.contains(name),
            "the list declares {name:?} and `doctor` never reports it \u{2014} the README counts \
             this list, so a name outliving its check is a number that stops being true"
        );
    }

    // A floor: the crossing says nothing if the run reported almost nothing.
    assert!(
        reported.len() >= estigia::harness::doctor::CHECKS.len(),
        "only {} rows came back for {} declared checks",
        reported.len(),
        estigia::harness::doctor::CHECKS.len()
    );
}

/// The role gate's reach is stated where an operator reads it.
///
/// `harness::roles` says it *makes the author's policy true*. It makes the part
/// of it that overlaps the gate's matcher true: the gate runs inside
/// `PreToolUse`, so a tool the matcher does not wake the hook for never arrives
/// to be judged. Measured against the list that module itself cites as the case
/// it exists for — a published `builder` sub-agent, declaring `Read, Write,
/// Edit, Glob, Grep, Bash` — three of the six are tools the gate can judge and
/// three are never seen.
///
/// Held here rather than left in a comment, for the reason the OpenCode
/// subagent hole is: a gate with a hole is still a gate, and a gate whose hole
/// nobody mentions is a lie. Closing it means waking this process for every
/// `Read`, which is the one cost the matcher exists to refuse.
#[test]
fn the_reach_of_the_role_gate_is_stated_where_an_operator_reads_it() {
    let contract = honesty();
    let section = contract.as_str();

    assert!(
        section.contains("declared tool list is enforced only for the tools the gate wakes for"),
        "the role gate's reach is not stated in the honesty contract"
    );
    // The measurement, not only the claim: a reader has to be able to tell
    // which half is enforced without going to the source.
    for named in ["Read", "Glob", "Grep", "WebFetch"] {
        assert!(
            section.contains(named),
            "the contract does not name {named}, which the gate never sees"
        );
    }
    for held in [
        "current `Agent` and legacy `Task`",
        "recursively refuses project-scoped shadows",
        "running reviewer uses the embedded policy",
        "not that Claude launched the context",
        "never reduces or serializes the configured panel",
    ] {
        assert!(
            section.contains(held),
            "the reviewer launch boundary omits {held:?}"
        );
    }
}

/// Where the verdict's binding is enforced is stated, and stated accurately.
///
/// The claim under the diagram is that the harness closes *delivering on a
/// verdict bound to a SHA that no longer existed*. Measured, it closes the
/// other half of that sentence: `git push`, `git merge` and `gh pr merge` are
/// irreversible boundaries and the gate re-adjudicates with no window, but the
/// only question it asks the transport is `verify-claim`, which reads the
/// issue's state and this run's live ownership — never the reviewed head or
/// base. The binding is mechanical where it is written, at publish time.
///
/// Held both ways: the contract has to name the gap, and the code has to still
/// have it. A boundary that grew a head/base check would make this test the
/// stale claim instead, and the assertion below says so by name.
#[test]
fn the_reach_of_the_verdict_binding_is_stated_where_an_operator_reads_it() {
    let section = honesty();
    assert!(
        section.contains("checked at delivery only for what this run published"),
        "the honesty contract does not say how far the verdict's binding reaches"
    );

    // And the code still is what the contract describes: the gate's boundary
    // question carries no head and no base. Read out of the source rather than
    // asserted, so this fails the day somebody closes the gap and leaves the
    // sentence behind.
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/harness/mod.rs"),
    )
    .expect("the harness ships with the crate");
    let at = source
        .find("\"verify-claim\",")
        .expect("the gate asks the transport to verify the claim");
    let call = &source[at..at + 400];
    for absent in ["--head", "--base", "--expected-target"] {
        assert!(
            !call.contains(absent),
            "the boundary now sends {absent}, so the contract's paragraph about where the \
             verdict's binding is enforced has gone stale"
        );
    }
}

/// Every relative link in this repository's own documentation resolves.
///
/// `lib.rs` denies broken intra-doc links for the same reason, in its own words:
/// *a broken doc link is a documented claim about this crate that is not true of
/// it*. The payload's half is held by `skill::tests::every_file_the_payload_
/// links_to_is_shipped`; this is the other set — `README.md`, `AGENTS.md`,
/// `CHANGELOG.md`, and everything under `docs/` and `openspec/`, which
/// cross-reference each other on purpose so that one document answers one
/// question.
///
/// `docs/` is walked rather than listed, and that is not tidiness: the four
/// documents there were carved out of the README, every pointer left behind is
/// a link, and the walk is what makes splitting a document safe to do again.
/// The working log is read when it is present and skipped when it is not — it
/// is kept outside the repository, so a clone has none, and a test that
/// required it would fail on every machine except the one that writes it.
///
/// Written because `AGENTS.md` now asks whoever works here to keep the
/// documentation *related* — to link the neighbouring document rather than
/// restate it — and an instruction with no mechanism is the kind of rule this
/// crate spends whole rounds finding on the other side of.
#[test]
fn every_link_in_this_repositorys_documentation_resolves() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![root.join("openspec"), root.join("docs")];
    let mut documents: Vec<PathBuf> = ["README.md", "AGENTS.md", "CHANGELOG.md", "HANDOFF.md"]
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.is_file())
        .collect();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|kind| kind == "md") {
                documents.push(path);
            }
        }
    }

    let mut checked = 0;
    let mut broken = Vec::new();
    for document in &documents {
        let text = std::fs::read_to_string(document).expect("a readable document");
        let here = document.parent().unwrap_or(&root);
        // `[text](target)`, minus the anchor and anything that leaves the disk.
        for piece in text.split("](").skip(1) {
            let Some(end) = piece.find(')') else { continue };
            let target = piece[..end].split('#').next().unwrap_or_default().trim();
            if target.is_empty()
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            checked += 1;
            if !here.join(target).exists() {
                broken.push(format!(
                    "{} links to {target}, which is not there",
                    document.strip_prefix(&root).unwrap_or(document).display()
                ));
            }
        }
    }

    // A floor: this goes quiet by finding no links, which is what a walk that
    // stopped reading looks like from the outside.
    // A floor, and a low one on purpose: these documents cross-reference each
    // other where it helps a reader and nowhere else, so the number is small
    // and padding it to raise the floor would be the *"add a paragraph because
    // it is the file you had open"* this repository tells people not to do.
    // What it catches is a walk that stopped reading.
    assert!(
        checked >= 8,
        "only {checked} relative links were read, so this proves little about the rest"
    );
    assert!(
        broken.is_empty(),
        "the documentation points at files that are not there: {broken:#?}"
    );
}

/// The badge at the top of the README counts the tools the crate has.
///
/// The first factual claim a reader meets, and nothing crossed it. Adding an
/// operation moved every count in the file and left the badge saying twenty over
/// a crate with twenty-one, with its own `href` pointing four lines down at the
/// section that disagreed.
///
/// *"Every other count is crossed"* is what this comment said, and it was not
/// true: the sentence four lines below the badge — *"N tools. The contract names
/// N operations every binding must map"* — was read by nothing either, measured
/// by rewriting both of its numbers and watching the whole suite stay green. It
/// is crossed now, by the test below this one. Claiming coverage is a claim, and
/// this change has been corrected for making one loosely more than once.
///
/// The number appears twice on the line, in the alt text and inside the shields
/// URL, and they are asserted separately: a badge whose picture and description
/// disagree is the same defect wearing one line.
#[test]
fn the_badge_counts_the_tools_the_crate_has() {
    let readme = readme();
    let tools = estigia::harness::mcp::tools::TOOLS.len();
    for claim in [
        format!("alt=\"MCP: {tools} tools\""),
        format!("badge/MCP-{tools}%20tools-"),
    ] {
        assert!(
            readme.contains(&claim),
            "the README badge does not say {claim:?}, and the crate has {tools} tools"
        );
    }
}

/// The tools section's own sentence counts both the tools and the operations.
///
/// The paragraph a binding author reads to learn what they have to map, and it
/// carried two numbers nothing checked. The scenario is not hypothetical: a
/// twentieth operation lands, `SKILL.md`'s MUST-map line and `src/skill.rs`'s
/// pinned count both go red and get updated, and this sentence goes on saying
/// nineteen with the suite green — telling the one reader who needs it exactly
/// wrong.
#[test]
fn the_tools_section_counts_the_tools_and_the_operations() {
    let readme = readme();
    let tools = estigia::harness::mcp::tools::TOOLS.len();
    let operations = estigia::skill::required_operations().len();
    assert_eq!(
        number_before(&readme, "tools. The contract names"),
        Some(tools),
        "the tools section does not count the {tools} tools the crate has"
    );
    assert_eq!(
        number_before(&readme, "operations every binding must map"),
        Some(operations),
        "the tools section does not count the {operations} operations the contract requires"
    );
}

/// The number of operations the tools expose is the number the README claims.
///
/// The honesty section says this file crosses the *countable* claims, and this
/// one was not among them: the README said **thirteen** operations while the
/// table held seventeen. Nothing was wrong with the code — the sentence had
/// simply stopped being true, four tools ago, in the paragraph that tells a
/// reader what they are buying.
///
/// Found by listing every numeric claim in the README and asking which ones
/// this file actually reads. Most are prose — *one thing*, *two runs*, *nine
/// years outside Olympus* — and this was the one that counted something the
/// crate defines.
#[test]
fn the_number_of_operations_is_the_number_the_readme_claims() {
    let readme = readme();
    let claimed =
        number_before(&readme, "operations with schemas").expect("the README counts the tools");
    let actual = estigia::harness::mcp::tools::TOOLS.len();
    assert_eq!(
        claimed, actual,
        "the README says the tools are {claimed} operations and the table holds {actual}"
    );
}

/// The port still does not link a branch to its issue, and says so.
///
/// The binding's `start-branch` runs `gh issue develop`: one call that creates
/// the remote ref *and* the issue's Development sidebar link, and then re-reads
/// the sidebar instead of believing an exit code. The port stops after the
/// local reservation. That is a difference the honesty contract names, and a
/// named difference has to stop being named the moment it stops being true —
/// otherwise the contract is a list of things that *used* to be missing, which
/// is worse than no list.
///
/// So this fails both ways: if the port learns to link and the entry stays, and
/// if the entry goes while the port still does not.
#[test]
fn the_port_still_does_not_link_a_branch_to_its_issue() {
    let ported = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("transport")
            .join("branch.rs"),
    )
    .expect("the port's branch module reads")
    // Its own module comment describes the difference, so the search has to
    // look for the call rather than for the words.
    .lines()
    .filter(|line| !line.trim_start().starts_with("//"))
    .any(|line| line.contains("\"develop\""));

    let named = readme().contains("never links the branch to its issue");
    assert_eq!(
        ported, !named,
        "the honesty contract and the port disagree about the sidebar link: \
         ported={ported}, still named in the README={named}"
    );
}

#[test]
fn no_prose_table_in_the_contract_spells_a_setting() {
    // `settings::rows` reads only the marked block when the document carries
    // one, and the comment saying why names the hazard it leaves standing: the
    // shipped contract holds 29 table rows, 8 of them configuration, and
    // "nothing broke, because no prose row's first cell happens to spell a
    // setting label — which is luck, not a property, and the kind that ends the
    // first time somebody writes a table row starting with `Tracker`".
    //
    // Measured, that luck is load-bearing. A contract whose closing marker never
    // reached the disk — a truncated write, which is this crate's own stated
    // threat model — has no locatable block, and the reader falls back to the
    // whole file:
    //
    // ```text
    // block intact     ->  8 rows
    // closing marker gone -> 25 rows, among them
    //                        Situation = Action
    //                        Load = When
    //                        State must be chosen = analysis
    // ```
    //
    // The resulting `Config` is still identical today, because none of those 17
    // prose labels is one Estigia published. This turns that into something a
    // test holds rather than something a comment hopes for.
    let contract =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/SKILL.md"))
            .expect("the shipped contract is readable");
    let block = estigia::config::CONFIG_FENCE
        .find(&contract)
        .expect("the shipped contract carries a config block");

    // Everything except the managed block: what the fallback would additionally
    // offer to the parser.
    let outside = format!("{}{}", &contract[..block.start], &contract[block.end..]);
    let offered = estigia::config::table_rows(&outside);

    // The floor. If the split stopped producing prose rows, every assertion
    // below would hold by measuring nothing — which is how a guard stops
    // guarding without ever failing.
    assert!(
        offered.len() > 10,
        "the contract outside its config block stopped holding tables: {} rows",
        offered.len()
    );

    let claimed: Vec<&(String, String)> = offered
        .iter()
        .filter(|(label, _)| estigia::config::Setting::from_label(label).is_some())
        .collect();
    assert!(
        claimed.is_empty(),
        "a table outside the config block now spells a setting, so a contract whose \
         block cannot be located reads it as configuration: {claimed:?}"
    );
}

#[test]
fn every_list_the_audit_answers_with_is_one_the_binding_names() {
    // The audit's answer is read by the **agent**, out of the binding. Nothing
    // in this crate summarises it, so a list added to that answer and not to the
    // binding is a list nobody is told to read — and this one decides whether a
    // board is reported as being in order.
    //
    // Measured by committing it: `unread_labels` was added to both
    // implementations, crossed by the differential, verified on the installed
    // script, and the whole suite stayed green while the document that teaches
    // the agent to read the answer still said the pass compares *every* card.
    // The zero-card rule is in that document precisely because a partial read
    // must not be reported as a clean board; the new list is the same rule one
    // field along, and it was invisible.
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/transport/commands.rs"),
    )
    .expect("the port is readable");
    let binding = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/bindings/github.md"),
    )
    .expect("the binding is readable");

    // The keys of the object `audit_board` answers with, read out of the literal
    // that builds it rather than listed here — a list written here is the second
    // copy this crate keeps finding disagreeing with itself.
    let at = source
        .find("pub fn audit_board")
        .expect("the audit is in this file");
    let body = &source[at..];
    let end = body
        .find("\n/// ")
        .or_else(|| body.find("\npub fn "))
        .unwrap_or(body.len());
    let mut keys: Vec<&str> = Vec::new();
    for (start, _) in body[..end].match_indices("\"") {
        let rest = &body[start + 1..];
        let Some(close) = rest.find('"') else {
            continue;
        };
        let key = &rest[..close];
        // The lists the verdict is made of, and only those: they are the ones an
        // agent has to know exist before it can say a board is in order.
        let is_a_verdict_list =
            key.ends_with("drift") || key.ends_with("_column") || key.ends_with("_labels");
        if is_a_verdict_list && !keys.contains(&key) {
            keys.push(key);
        }
    }

    // The floor: the scan found the answer's lists. An empty set would agree
    // with any binding at all.
    assert!(
        keys.len() >= 3,
        "the scan read {keys:?} out of `audit_board`, so it is not reading the answer"
    );

    let unnamed: Vec<&&str> = keys.iter().filter(|key| !binding.contains(**key)).collect();
    assert!(
        unnamed.is_empty(),
        "the audit answers with {unnamed:?} and the binding the agent reads never names them"
    );
}

/// The README says how many checks `doctor` runs twice, and the two agree.
///
/// Two adjacent entries carried it: *`doctor` checks twelve things, not
/// everything* and, three lines later, *one of the eleven is about the past*.
/// One of them was wrong and neither was read by anything — the count guard
/// above covers the agent totals and stopped there. A number in prose that
/// nothing reads is a number that drifts, which is the whole reason this file
/// exists.
///
/// **What this does not do** is check either number against the code, and the
/// reason is worth writing down rather than leaving as an omission: how many
/// checks `doctor::full` produces depends on the machine it is asked about. It
/// answers six kinds on a machine with nothing installed and twelve with one
/// adapter configured — measured, because the two numbers that stood here
/// before were each carried forward by hand and neither was ever true — and the
/// rows
/// themselves are more again — the contract, gate and tools checks are emitted
/// once per configured agent, so counting rows answers *how many agents does
/// this machine have*. Holding the two spellings to each other is what can be
/// held from here, and it is what the defect needed.
#[test]
fn the_number_of_doctor_checks_is_the_same_number_everywhere_it_is_claimed() {
    let readme = documented();
    let phrases = ["things, not everything", "is about the past"];
    let claimed: Vec<usize> = phrases
        .iter()
        .map(|phrase| {
            number_before(&readme, phrase).unwrap_or_else(|| {
                panic!("the README no longer carries a number before {phrase:?}")
            })
        })
        .collect();
    assert_eq!(
        claimed[0], claimed[1],
        "the README says {} checks in one entry and {} in the next, three lines apart",
        claimed[0], claimed[1]
    );
}

/// The prose an agent reads names no implementation this crate retired.
///
/// The payload is the one document with a reader who cannot check it. An agent
/// takes `SKILL.md` and its bindings as the description of the tool it is
/// holding, and there is nothing on the other side to notice that the
/// description stopped being true.
///
/// It had stopped. `bindings/github.md` said *"this binding is a Python script
/// and not a PowerShell/bash pair"* and reasoned from it, and
/// `references/safety-incidents.md` gave `scripts/github.py` as the place a
/// whole class of incident is fixed — both about a file that had been deleted
/// rounds earlier, in the prose that tells an agent what it is calling. The
/// code changed, the tests changed, the README changed; the payload did not,
/// and the payload is the only one of the four that is *installed*.
///
/// Two rules, and both are checkable rather than tasteful:
///
/// - No payload document names a path in [`estigia::skill::RETIRED`]. A file
///   this crate asked to be gone is one no prose may send anybody to, even to
///   explain that it is gone — that sentence can be written without the path.
/// - No payload document says the binding **is** a script or an interpreter.
///   The operations are answered in this process; a document that says
///   otherwise teaches a model of the tool that is wrong in the direction that
///   matters, because it implies a file the agent could go and read.
#[test]
fn the_payload_names_no_implementation_this_crate_retired() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill");
    let mut read = 0;
    let mut stale: Vec<String> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|kind| kind != "md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            read += 1;
            let named = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string();
            for (number, line) in text.lines().enumerate() {
                let at = format!("{named}:{}", number + 1);
                for retired in estigia::skill::RETIRED {
                    if line.contains(retired) {
                        stale.push(format!("{at} names the retired `{retired}`"));
                    }
                }
                for claim in [
                    "is a Python script",
                    "the script removes",
                    "the script never",
                    "the script writes",
                    "the script makes",
                    "the script says",
                    "the script already",
                    "the script tries",
                    "the script reports",
                    "in the script",
                ] {
                    if line.to_lowercase().contains(claim) {
                        stale.push(format!("{at} says `{claim}`"));
                    }
                }
            }
        }
    }
    // The floor: the walk found the payload. An empty walk reports exactly what
    // a clean one does.
    assert!(
        read >= 8,
        "only {read} payload document(s) were read, so this checked almost nothing"
    );
    assert!(
        stale.is_empty(),
        "the prose an agent reads describes an implementation this crate deleted: {stale:#?}"
    );
}

/// The README counts the same things the handoff counts, and both count the code.
///
/// `the_handoff_counts_what_the_crate_actually_has` opens by saying *"the
/// README's counts are crossed against the code above"*. They are not, for the
/// one a reader meets first: the Configuration section said **"Nine settings,
/// all typed"** and the crate has eighteen, while the handoff's `18 settings`
/// was crossed and correct three files away. The sentence claiming the coverage
/// was the reason nobody looked.
///
/// So the two documents are held to the same table. Every count here is a fact
/// about the crate that both prose files state, and a claim in either of them
/// that drifts fails the same way.
///
/// The README writes its numbers as words, because it is written for a person —
/// which is what `number_before` is for and why the phrase, not the number, is
/// what this names.
#[test]
fn the_readme_counts_what_the_crate_actually_has() {
    let readme = documented();
    for (phrase, actual, what) in [
        (
            "settings, all typed",
            estigia::config::SETTINGS.len(),
            "settings",
        ),
        // The same count, in the other sentence that states it. A reviewer
        // measured that this guard keyed on one phrase only: `docs/configuration.md`
        // opened with **eighteen** and said **nineteen** nine lines later, through a
        // change that edited the guarded sentence and not the unguarded one, and
        // `README.md`'s count could be set to anything at all.
        // Keyed on a phrase unique to `docs/configuration.md`'s opening line.
        // A first attempt used the bare `"typed settings"`, which `README.md`
        // also carries — and `number_before` takes the **first** occurrence over
        // a concatenation that puts the README first, so the pair guarded the
        // README twice and the sentence that had actually drifted not at all. A
        // reviewer measured that by changing the opening line and watching the
        // suite stay green.
        (
            "typed settings, read from one table",
            estigia::config::SETTINGS.len(),
            "settings, in the sentence that opens the configuration reference",
        ),
        // And the sentence a reader of the README meets first, which neither
        // pair above reaches. Three reviewers measured that: every phrase this
        // test keyed on resolves inside `docs/configuration.md` — `settings, all
        // typed` at its line 11, `typed settings, read from one table` at its
        // line 3 — while `README.md` says `typed settings, in` and matched
        // neither. The README could claim **ninety** settings with the whole
        // suite green, through a change whose own pull request said this guard
        // "crosses both sentences and bites on `README.md` too". It did not.
        //
        // Keyed on `typed settings, in`, which only the README carries: the
        // configuration reference ends that phrase with `read from one table`
        // and `all typed`. `number_before` takes the first occurrence over the
        // concatenation, so a phrase shared between the two files guards
        // whichever comes first and leaves the other unheld — the trap this
        // pair exists to stay out of, and the one the previous repair fell into.
        (
            "typed settings, in",
            estigia::config::SETTINGS.len(),
            "settings, in the sentence that opens the README's configuration section",
        ),
        (
            "adapters share",
            8_usize,
            "adapters sharing the neutral root",
        ),
    ] {
        let claimed = number_before(&readme, phrase).unwrap_or_else(|| {
            panic!("the README no longer carries a number before {phrase:?}, so the {what} count is unchecked again")
        });
        assert_eq!(
            claimed, actual,
            "the README claims {claimed} {what} and the crate has {actual}"
        );
    }
}

/// Every setting is acted on by something, and the something is named.
///
/// An operator sets nineteen rows. `config list` reports them, the contract
/// carries them, and `doctor` has a row for the ones a *tracker* declares
/// nothing for. Nothing asked the plainer question: does anything at all read
/// this row.
///
/// Measured, and five did not. `context.get` is called for exactly three labels —
/// `project board`, `worktree location` and `Review delegation`, which is what
/// `READ_BY_THE_TRANSPORT`
/// says — the gate reads `Irreversible commands`, and the payload's prose names
/// eleven. That left `Delivery authorisation`, `Transition authorisation`,
/// `Delivery route`, `Merge strategy` and `Model routing` read by nobody: not
/// the gate, not the transport, and no sentence an agent is given.
///
/// `setup::Applies::Asked` says of three of them *"the contract asks, and the
/// agent may still honour it, but nothing checks"* — and for three the contract
/// did not ask either. A sentence that describes a mechanism is not the
/// mechanism.
///
/// Two are closed by this change: the authorisations, whose meaning is not a
/// design question — `ask` means ask a person first, which is the shape
/// `Review delegation` already had in `SKILL.md`. The other three are declared
/// below and named in the README's honesty contract, because deciding what an
/// agent does about a merge strategy or a model route is a design call and not
/// a gap to paper over here.
#[test]
fn every_setting_is_read_by_the_gate_the_transport_or_the_prose() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // The prose an agent is given, minus the configuration table itself: a row
    // restating its own name teaches nobody what to do with it.
    let mut prose = String::new();
    let mut stack = vec![root.join("skill")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "md")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                for line in text
                    .lines()
                    .filter(|line| !line.trim_start().starts_with('|'))
                {
                    prose.push_str(&line.to_lowercase());
                    prose.push('\n');
                }
            }
        }
    }

    // And the code that acts on a row. **The call, not the name**: every label
    // in this crate is written down somewhere in a doc comment or a message, so
    // asking whether the source mentions it answers *is this documented*, which
    // is not the question. The transport reads a row through `context.get`, and
    // the gate through the one field the operator's boundaries arrive in.
    let mut reads: Vec<String> = Vec::new();
    let mut stack = vec![root.join("src")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs")
                && path.file_name().is_some_and(|name| name != "tests.rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let shipped = text
                    .find("#[cfg(test)]\nmod tests {")
                    .map_or(text.as_str(), |end| &text[..end]);
                let mut rest = shipped;
                while let Some(at) = rest.find("context.get(\"") {
                    rest = &rest[at + "context.get(\"".len()..];
                    if let Some(end) = rest.find('"') {
                        reads.push(rest[..end].to_lowercase());
                    }
                }
                // The operator's declared boundaries reach the classifier as
                // `context.boundaries`, which is the gate acting on that row.
                if shipped.contains("context.boundaries") {
                    reads.push("irreversible commands".to_owned());
                }
            }
        }
    }

    // Rows nothing reads today, declared with the reason each is still here.
    // Shorter only, never longer: a new setting nothing acts on fails.
    const READ_BY_NOBODY: &[&str] = &[
        // Accepts one value, so there is nothing yet to act on differently.
        "Delivery route",
        // The history the base is required to end up with. `merge` is declared
        // *(agent, not scripted)*, so acting on it is a sentence somebody has
        // to write about a command Estigia does not run.
        "Merge strategy",
    ];

    let mut unread: Vec<&str> = Vec::new();
    for setting in estigia::config::SETTINGS {
        let label = setting.label();
        let lowered = label.to_lowercase();
        if prose.contains(&lowered) || reads.contains(&lowered) {
            continue;
        }
        unread.push(label);
    }
    // The floor: most settings *are* read, so a walk that found nothing would
    // agree with a clean run.
    assert!(
        unread.len() < estigia::config::SETTINGS.len() / 2,
        "{} of {} settings read as unread, so this walk is not finding the prose or the code",
        unread.len(),
        estigia::config::SETTINGS.len()
    );

    let fresh: Vec<&&str> = unread
        .iter()
        .filter(|label| !READ_BY_NOBODY.contains(label))
        .collect();
    assert!(
        fresh.is_empty(),
        "an operator can set these rows and nothing reads them \u{2014} not the gate, not the \
         transport, and no sentence an agent is given: {fresh:?}"
    );
    let stale: Vec<&&str> = READ_BY_NOBODY
        .iter()
        .filter(|label| !unread.contains(*label))
        .collect();
    assert!(
        stale.is_empty(),
        "these are declared as read by nobody and something reads them now: {stale:?} \u{2014} \
         take them off the list"
    );
}

/// Which trackers can be operated is one rule, in one place.
///
/// `Tracker::transport` answers it, and four callers ask: the doctor's row, the
/// tool server, the gate, and the guard's caveats. The fifth was written as a
/// list — `["linear", "trello"]`, a round old and mine — which is a second copy
/// of the rule this crate's own contributing note says to remove rather than
/// guard. A tracker added with no transport would have been absent from it, and
/// the guard would have gone on promising refusals that cannot happen for
/// exactly the tracker nobody had thought about.
///
/// Checked as text because that is where the copy appears: someone reaching for
/// the two names is writing the list again, and nothing at runtime would object
/// while the two happen to agree.
#[test]
fn every_reader_of_a_transportless_tracker_asks_the_same_function() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut asked = 0;
    let mut copies: Vec<String> = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|kind| kind != "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let shipped = text
                .find("#[cfg(test)]\nmod tests {")
                .map_or(text.as_str(), |end| &text[..end]);
            asked += shipped.matches(".transport()").count();
            // The enum's own definition names them, which is the one place that
            // may. Everywhere else, the pair spelled together is the copy.
            let defines = shipped.contains("pub fn transport(&self)");
            for line in shipped.lines() {
                let code = line.split("//").next().unwrap_or_default();
                // The pair **without** `github` is the copy. With it, the line
                // is the list of values the setting accepts — a different rule,
                // about what may be typed rather than what can be operated, and
                // one this must not drag in: the first cut did, and named a
                // line that is right where it is.
                if !defines
                    && code.contains("\"linear\"")
                    && code.contains("\"trello\"")
                    && !code.contains("\"github\"")
                {
                    copies.push(format!("{}: {}", path.display(), line.trim()));
                }
            }
        }
    }
    // The floor: the function really is the one everybody asks.
    assert!(
        asked >= 4,
        "only {asked} callers ask `transport()`, so this is not watching the rule it claims to"
    );
    assert!(
        copies.is_empty(),
        "the rule about which trackers can be operated is written a second time: {copies:#?}"
    );
}

/// No vocabulary is declared twice.
///
/// The contributing note puts this first — *prefer removing a copy to adding a
/// check* — and a sweep found two pairs nothing was watching.
///
/// `STATES` was spelled out in `config` and again in `transport::commands`. The
/// second creates the `status:` labels and the first types the tool schemas, so
/// a state renamed in one would have `ensure_states` making six labels and
/// `transition` moving to a seventh no label exists for.
///
/// `COMMENT_KINDS` was the sharper of the two: `config`'s copy is published to
/// the agent as the values it may send, and `markers`' copy is what `comment`
/// refuses from. A kind added to either is a kind the agent is offered and the
/// transport rejects, or one the transport takes and the schema hides. They
/// agreed, in different orders, and nothing made them go on agreeing.
///
/// This is a check about copies, which is the thing the note says not to add —
/// and it is added for the one case the note cannot cover: it does not stop a
/// copy being written, it stops one being *kept*. Both are re-exports now.
#[test]
fn no_vocabulary_is_declared_in_two_places() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut declared: Vec<(Vec<String>, String)> = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|kind| kind != "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let shipped = text
                .find("#[cfg(test)]\nmod tests {")
                .map_or(text.as_str(), |end| &text[..end]);
            let mut rest = shipped;
            while let Some(at) = rest.find(": &[&str] = &[") {
                let name: String = rest[..at]
                    .rsplit(|c: char| !(c.is_ascii_uppercase() || c == '_'))
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                rest = &rest[at + ": &[&str] = &[".len()..];
                let Some(end) = rest.find("];") else { break };
                let mut items: Vec<String> = rest[..end]
                    .split(',')
                    .map(|item| item.trim().trim_matches('"').to_owned())
                    .filter(|item| !item.is_empty() && !item.starts_with("//"))
                    .collect();
                if items.len() >= 3 {
                    items.sort();
                    declared.push((items, format!("{}::{name}", path.display())));
                }
            }
        }
    }
    // The floor: the walk found the crate's vocabularies.
    assert!(
        declared.len() >= 6,
        "only {} string lists were found, so this compared almost nothing",
        declared.len()
    );

    let mut twice: Vec<String> = Vec::new();
    for (index, (items, at)) in declared.iter().enumerate() {
        for (other, elsewhere) in declared.iter().skip(index + 1) {
            if items == other {
                twice.push(format!("{at} and {elsewhere} hold the same list"));
            }
        }
    }
    assert!(
        twice.is_empty(),
        "one vocabulary, declared twice \u{2014} they agree today and nothing makes them agree \
         tomorrow: {twice:#?}"
    );
}

/// A prefix that decides what a string *is* is written once.
///
/// The sibling of `no_vocabulary_is_declared_in_two_places`, for the rules that
/// are one literal rather than a list. Two of them were spread across the
/// transport:
///
/// - `"status:"`, which says a label carries the workflow state, in three
///   places — and two of those are the halves of one decision: `verify_claim`
///   read the state from an inlined copy while `transition` set it through
///   `status_labels`, character for character the same body.
/// - `"issue-flow:"`, which says an HTML comment is a control marker, in three
///   places across two modules: the parser, the escaper that stops a body
///   faking one, and the scan the ownership reducer adjudicates from.
///
/// Each is one predicate now. What this holds is that they stay one: the
/// literal may appear where the predicate is defined and nowhere else.
#[test]
fn a_prefix_that_decides_a_meaning_is_written_once() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sites: Vec<(String, String)> = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|kind| kind != "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let shipped = text
                .find("#[cfg(test)]\nmod tests {")
                .map_or(text.as_str(), |end| &text[..end]);
            for (number, line) in shipped.lines().enumerate() {
                // The code, not the prose: these prefixes are named in comments
                // on purpose, which is how the rule stays readable.
                let code = line.split("//").next().unwrap_or_default();
                for prefix in ["\"status:\"", "\"issue-flow:\""] {
                    if code.contains(prefix) {
                        sites.push((
                            prefix.to_owned(),
                            format!("{}:{}", path.display(), number + 1),
                        ));
                    }
                }
            }
        }
    }
    // The floor: each prefix is still somewhere, or this passes on a crate that
    // has lost the rule altogether.
    for prefix in ["\"status:\"", "\"issue-flow:\""] {
        let found = sites.iter().filter(|(what, _)| what == prefix).count();
        assert!(found > 0, "{prefix} is not in the crate at all");
        assert_eq!(
            found,
            1,
            "{prefix} decides what a string is, and it is written in {found} places: {:#?}",
            sites
                .iter()
                .filter(|(what, _)| what == prefix)
                .map(|(_, at)| at)
                .collect::<Vec<_>>()
        );
    }
}

/// The transport reads the settings this crate says it reads.
///
/// `READ_BY_THE_TRANSPORT` claimed this crossing in its own doc comment and no
/// such test existed: the list was read by a re-export and nothing else, so
/// "two, and only two" — and then "three, and only three" — was prose wearing
/// the clothes of a measurement, in the one crate whose point is that its
/// measurements are real. A row added to the transport without being declared
/// here, or declared here without being read, now fails.
#[test]
fn the_transport_reads_the_settings_this_crate_says_it_does() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut read: Vec<String> = Vec::new();
    let mut stack = vec![root.join("src")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs")
                && path.file_name().is_some_and(|name| name != "tests.rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                // The shipped half only: a fixture building a `Context` is not
                // the transport consulting the operator's table.
                let shipped = text
                    .find("#[cfg(test)]\nmod tests {")
                    .map_or(text.as_str(), |end| &text[..end]);
                let mut rest = shipped;
                while let Some(at) = rest.find("context.get(\"") {
                    rest = &rest[at + "context.get(\"".len()..];
                    if let Some(end) = rest.find('"') {
                        read.push(rest[..end].to_lowercase());
                    }
                }
            }
        }
    }
    read.sort();
    read.dedup();

    let mut declared: Vec<String> = estigia::config::READ_BY_THE_TRANSPORT
        .iter()
        .map(|setting| setting.label().to_lowercase())
        .collect();
    declared.sort();

    assert_eq!(
        read, declared,
        "the transport's `context.get` labels and `READ_BY_THE_TRANSPORT` disagree"
    );
}
