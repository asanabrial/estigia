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

/// The honesty contract, across the documents it now spans.
///
/// `docs/honesty.md` is the index and the last run lives in
/// `docs/honesty-review-tier.md`; a reader of one path would drop whatever
/// moved, and the length check is what keeps an empty read from passing the
/// negative assertions below.
fn honesty() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("docs/ ships with the crate")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "honesty.md" || name.starts_with("honesty-"))
        })
        .filter(|path| path.extension().is_some_and(|kind| kind == "md"))
        .collect();
    paths.sort();
    let text: String = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("a readable document"))
        .collect();
    assert!(
        text.len() > 10_000,
        "the honesty contract is {} bytes, too short to be the contract this file checks",
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
fn two_blind_keeps_one_enforced_verdict_and_names_every_unproved_panel_property() {
    let text = honesty().to_ascii_lowercase();
    assert!(
        readme().contains(
            r#"estigia config set "Model routing" "developer=inherit" --agent claude-code"#
        )
    );
    assert!(
        text.contains("one computed exact-receipt verdict"),
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
            contract.contains("one computed exact-receipt verdict"),
            "{path} contradicts the aggregate evidence floor"
        );
        if path == "protocols/rdd.md" {
            assert!(contract.contains("no per-judge verdict"));
            // And the half that went stale under this guard rather than around
            // it. The two lines above pinned the sentence "structured lens and
            // judge evidence remains future design" — which stopped being true
            // when the finding ledger landed, while both assertions stayed
            // green because the substrings survived the falsehood. A reviewer
            // found the document still saying it. So the pin is narrowed to the
            // claim that is still true, and this line holds the one that is not:
            // whatever `rdd.md` calls future design, it may not be the finding
            // evidence, and it has to name the ledger that replaced it.
            assert!(
                contract.contains("one marker per finding"),
                "protocols/rdd.md does not name the finding ledger, so its `future design` \
                 sentence is describing a protocol that no longer exists"
            );
            assert!(
                !contract.contains("structured lens and judge evidence remains future design"),
                "protocols/rdd.md still says structured judge evidence is future design; it \
                 shipped in the change that added `record_review_finding`"
            );
        }
    }
    for unproved in [
        "panel size",
        "concurrency",
        "independence",
        "blindness",
        "same-finding identity",
        "quorum",
        // Was "judge isolation outside the reserved role" until recently, which
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
        // Past twelve because the crate went past it. A table that stops at its
        // highest entry answers `None` for anything above it, which every caller
        // turns into `expect("the README counts …")` — that reads as *the README
        // has no number there* when what happened is that this reader cannot
        // count that high, so the ceiling arrives the day any counted claim
        // outgrows the table.
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
        // ceiling the note above describes arriving when any counted claim
        // outgrows the table.
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
        ("twenty-three", 23),
        ("twenty-four", 24),
        ("twenty-five", 25),
        ("twenty-six", 26),
        ("twenty-seven", 27),
        ("twenty-eight", 28),
        ("twenty-nine", 29),
        // One past the previous ceiling, on the day this crate's thirtieth
        // tool arrived — the same arrival the comment above `("thirteen", 13)`
        // already named as inevitable.
        ("thirty", 30),
        // One past the previous ceiling, on the day this crate's thirty-first
        // tool arrived — the same arrival the comment above `("thirty", 30)`
        // already named as inevitable.
        ("thirty-one", 31),
        // One past the previous ceiling, on the day this crate's thirty-second
        // tool arrived — the same arrival the comment above `("thirty-one", 31)`
        // already named as inevitable.
        ("thirty-two", 32),
        // One past the previous ceiling, on the day this crate's thirty-third
        // tool arrived — the same arrival the comment above `("thirty-two", 32)`
        // already named as inevitable.
        ("thirty-three", 33),
        ("thirty-four", 34),
        ("thirty-five", 35),
        ("thirty-six", 36),
        ("thirty-seven", 37),
        ("thirty-eight", 38),
        // One past the previous ceiling, on the day this crate's thirty-ninth
        // tool arrived — the arrival the comments above already named as
        // inevitable.
        ("thirty-nine", 39),
        ("forty", 40),
        ("forty-one", 41),
        ("forty-two", 42),
        ("forty-three", 43),
        ("forty-four", 44),
        ("forty-five", 45),
        ("forty-six", 46),
        ("forty-seven", 47),
        ("forty-eight", 48),
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

/// No guard reading the whole documentation set blames the README.
///
/// The third sweep of this, and the reason it is a test rather than a fourth.
/// `documented()` concatenates the README with every file in `docs/`, so a
/// failure from a guard that reads it may be about any of them — and three
/// of the phrases those guards key on appear **only** in `docs/honesty.md`. A
/// reviewer drifted one and was sent to `README.md`, where the sentence does not
/// exist.
///
/// Two sweeps missed it. The first reworded an `assert_eq!` and left the
/// `panic!` four lines above; the second reworded every `format!` message and
/// left five `.expect(...)` strings, then shipped a sentence saying every guard
/// had been corrected. Both were true of what they touched, which is what made
/// them easy to believe.
///
/// So the property is asserted instead of promised — but **only as far as the
/// admission test reaches**, and saying more than that is how the three sweeps
/// before it were believed. What it checks: a function whose text carries
/// `documented()` may not attribute a claim to the README in any
/// non-comment line. What it does not: a reader reached some other way, a
/// misdirection phrased without one of the attributing verbs, and any file but
/// this one.
///
/// Two judges wrote past the first version — one restored the removed message
/// into the count guard, which was skipped; the other
/// wrote `README.md counts` where it had said `the README counts`. A third
/// renamed the helper and left the guard reporting success over nothing, which
/// is what the floor below is for. Guards that read `readme()` are untouched:
/// theirs is the one document, and saying so is correct.
#[test]
fn no_guard_reading_every_document_blames_the_readme() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("honesty.rs"),
    )
    .expect("this file reads");
    let source = estigia::source_text::code_of(&source);

    let lines: Vec<&str> = source.lines().collect();
    let mut blaming = Vec::new();
    let mut examined = 0_usize;
    let mut cursor = 0;
    while cursor < lines.len() {
        // A top-level function runs from its `fn` line to the first `}` in
        // column zero, so a chunk is the body and nothing else. Splitting on
        // `\nfn ` ran each chunk into the *next* function's doc comment, so a
        // line could be reported under the name of the function above it, and
        // the position printed beside it counted from the wrong place. Two
        // judges on separate panels found the same misreport, in the guard
        // whose whole subject is a message that sends a reader to the wrong
        // file.
        let Some(rest) = lines[cursor].strip_prefix("fn ") else {
            cursor += 1;
            continue;
        };
        let start = cursor;
        let mut end = start + 1;
        while end < lines.len() && lines[end] != "}" {
            end += 1;
        }
        cursor = end + 1;
        let function = &lines[start..end.min(lines.len())];
        let name = rest.split('(').next().unwrap_or_default().trim().to_owned();
        // Both readers, and neither spelled with its semicolon. The first
        // version admitted on `documented();`, which excluded the one function
        // this guard was written for: the count guard binds `documented()`, the
        // widest reader of all, and a judge put the removed message back into it
        // with the whole target green. `documented().to_string()` slipped
        // through the same way.
        let reads = function.iter().any(|line| line.contains("documented()"));
        if !reads || name == "no_guard_reading_every_document_blames_the_readme" {
            continue;
        }
        examined += 1;
        for (offset, line) in function.iter().enumerate() {
            // What is refused is **attribution**, not mention. A `what`
            // description saying *the sentence that opens the README’s
            // configuration section* is precise and helpful — one of these
            // sentences really is the README’s. What misleads is a failure
            // saying the README *claims* or *counts* something that lives
            // somewhere else.
            //
            // Both spellings of the subject, because a judge evaded the first
            // version by writing `README.md counts the dialects` where it had
            // said `the README counts` — the same misdirection in different
            // words.
            let attributes = [
                "claims",
                "says",
                "counts",
                "no longer",
                "makes no",
                "has no",
                "tabulates",
                "ships",
            ];
            if line.contains("README") && attributes.iter().any(|verb| line.contains(verb)) {
                blaming.push(format!(
                    "tests/honesty.rs:{} in {name}: {}",
                    start + offset + 1,
                    line.trim()
                ));
            }
        }
    }

    // The floor every other reader in this file has, and this one did not.
    // Renaming the helper left zero functions examined and the guard reporting
    // success — a read that finds nothing makes every assertion pass by
    // having nothing to disagree with, which the module docstring names as the
    // shape to guard against.
    assert!(
        examined >= 8,
        "only {examined} guards were found reading the documentation set, which is fewer than \
         this file has \u{2014} the admission test has stopped matching and this guard is now \
         passing by examining nothing"
    );
    assert!(
        blaming.is_empty(),
        "these guards read every document and blame the README, which sends a reader to a file \
         that may not carry the sentence at all: {blaming:#?}"
    );
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
/// sentence names sixteen keys for `Model routing` alone, and a table cell
/// carrying it is a table nobody reads. What that leaves unmeasured is real and
/// worth naming: an open row's README prose can be thinner than the parser's,
/// and `Model routing`'s was — *"comma-separated `key=model` pairs"*, naming no
/// key at all, on the row an operator opens looking for exactly one. Prose is
/// what fixed it, and prose is what would have to go stale again.
#[test]
fn the_settings_table_names_every_value_a_closed_row_takes() {
    let documents = documented();
    let mut silent: Vec<String> = Vec::new();
    for setting in SETTINGS {
        let answers = setting.answers();
        // Open vocabularies are out, for the reason above.
        if !answers.closed {
            continue;
        }
        let label = setting.label();
        // The row as the table holds it: `| Label | values |`.
        let Some(row) = documents
            .lines()
            .find(|line| line.starts_with(&format!("| {label} |")))
        else {
            silent.push(format!("{label}: no document carries that row at all"));
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
        "the documented settings table names fewer values than the picker offers, so a value an \
         operator can choose is one they cannot discover:\n  {}",
        silent.join("\n  ")
    );
}

#[test]
fn model_catalog_suggestions_keep_their_honesty_boundary() {
    let documents = documented();
    let behavior = documents
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
        "The retired phase keys",
        "Claude Code and Codex offer three reviewed presets",
        "Choosing one replaces that agent's complete model route",
        "profile row reads `custom`",
        "Choosing `custom` preserves the current route",
        "OpenCode's catalog is dynamic",
        "are not keys at all",
        "opens its concrete host's advisory model list directly",
        "`inherit` removes only that target",
        "Space remains literal inside the custom editor",
        "`opencode models`",
        "`--refresh`",
        "advisory suggestions, not validation",
        "an all-installed-agents answer",
        "currently ticked agents",
        "destination's own installed route",
        "aggregates each target independently",
        "disagree on their route",
        "A `Model routing` edit on the all-installed-agents page uses that same ticked-agent set",
        "Successful TUI saves acknowledge only",
        "portable contract-only config",
        "operator's local override",
        "explicit row ownership",
        "`ConfigLayers::effective_over`",
        "scope-filtered before value validation",
        "forbidden hand-edited rows are ignored",
        "Invalid owned rows still",
        "never promoted into the shared contract",
        "Dynamic host artifacts such as the delegated role definitions use the effective view",
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
        "both follow the highlighted row",
        "Space on the agents list remains an ordinary agent toggle",
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
        claim.contains("Only Claude Code receives host-routable reviewer definitions")
            && claim.contains(
                "OpenCode and every other host keep the reviewer values as routing declarations",
            )
            && claim.contains(
                "The two delegated roles `developer` and `analyst` are installed for OpenCode as well as Claude Code",
            )
    };
    assert!(
        exclusively_materializes_claude(&honesty),
        "the honesty contract does not say that Claude Code alone receives reviewer \
         definitions, that OpenCode receives the two delegated workers besides, and that all other \
         hosts retain routing declarations"
    );
    for broadened in [
        honesty.replace("Only Claude Code receives", "Claude Code receives"),
        honesty.replace(
            "Only Claude Code receives",
            "Only Claude Code and OpenCode receive",
        ),
        honesty.replace("OpenCode and every other host", "Every other host"),
        honesty.replace(
            "The two delegated roles `developer` and `analyst` are installed for OpenCode as well as Claude Code",
            "The two delegated roles are installed for OpenCode",
        ),
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
        "the README's tool list is not the set this server dispatches"
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
                22 => "Twenty-two",
                23 => "Twenty-three",
                24 => "Twenty-four",
                25 => "Twenty-five",
                26 => "Twenty-six",
                27 => "Twenty-seven",
                28 => "Twenty-eight",
                29 => "Twenty-nine",
                30 => "Thirty",
                31 => "Thirty-one",
                32 => "Thirty-two",
                33 => "Thirty-three",
                34 => "Thirty-four",
                35 => "Thirty-five",
                36 => "Thirty-six",
                37 => "Thirty-seven",
                38 => "Thirty-eight",
                39 => "Thirty-nine",
                40 => "Forty",
                41 => "Forty-one",
                42 => "Forty-two",
                43 => "Forty-three",
                44 => "Forty-four",
                45 => "Forty-five",
                46 => "Forty-six",
                47 => "Forty-seven",
                48 => "Forty-eight",
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
        "the configuration reference's settings table is not this binary's rows, in its order"
    );
}

/// One table for every count a document claims about the crate.
///
/// Merged from six one-purpose tests. Each row names the document it
/// reads, because `number_before` takes the first occurrence in the text and
/// pooling the documents would move which sentence answers for a phrase that
/// appears in more than one. The badge's two spellings, the gated-agent forms
/// and the doctor check's two-phrase agreement are checked after the table,
/// where none of them is a `(phrase, value)` pair.
#[test]
fn every_count_a_document_claims_is_one_the_crate_has() {
    #[derive(Clone, Copy)]
    enum Source {
        Readme,
        Documented,
    }
    fn source_text(source: Source) -> String {
        match source {
            Source::Readme => readme(),
            Source::Documented => documented(),
        }
    }

    let tools = estigia::harness::mcp::tools::TOOLS.len();
    let settings = SETTINGS.len();
    let operations = estigia::skill::required_operations().len();
    let adapters = estigia::setup::adapters_on_the_neutral_root();
    let gated = estigia::setup::AGENTS
        .iter()
        .filter(|adapter| adapter.can_gate_tools())
        .count();
    let dialects = estigia::harness::hook::Dialect::all().len();

    for (source, phrase, actual, what) in [
        (
            Source::Documented,
            "settings, all typed",
            settings,
            "settings, in the second sentence of the configuration reference",
        ),
        (
            Source::Documented,
            "typed settings, read from one table",
            settings,
            "settings, in the sentence that opens the configuration reference",
        ),
        (
            Source::Documented,
            "typed settings, in",
            settings,
            "settings, in the sentence that opens the README's configuration section",
        ),
        (
            Source::Documented,
            "rows and `config list` reports",
            settings,
            "rows, in the sentence about what `config list` reports",
        ),
        (
            Source::Documented,
            "adapters share `~/.agents/skills`",
            adapters,
            "adapters sharing the neutral root",
        ),
        (
            Source::Readme,
            "operations with schemas",
            tools,
            "operations, in the README's tools section",
        ),
        (
            Source::Readme,
            "tools. The contract names",
            tools,
            "tools, in the README's tools section",
        ),
        (
            Source::Readme,
            "operations every binding must map",
            operations,
            "operations, in the README's tools section",
        ),
        (Source::Documented, "of them, in", gated, "gated agents"),
        (Source::Documented, "dialects", dialects, "dialects"),
    ] {
        let text = source_text(source);
        let claimed = number_before(&text, phrase).unwrap_or_else(|| {
            panic!(
                "no document carries a number before {phrase:?} any more, so the {what} count is \
                 unchecked again"
            )
        });
        assert_eq!(
            claimed, actual,
            "the documents claim {claimed} {what}, and the crate has {actual}"
        );
    }

    // The badge's number appears twice on its line, in the alt text and inside
    // the shields URL, and they are asserted separately: a badge whose picture
    // and description disagree is the same defect wearing one line.
    let readme = readme();
    for claim in [
        format!("alt=\"MCP: {tools} tools\""),
        format!("badge/MCP-{tools}%20tools-"),
    ] {
        assert!(
            readme.contains(&claim),
            "the README badge does not say {claim:?}, and the crate has {tools} tools"
        );
    }

    // The gated-agent claim, in the other form it takes. It is only true while
    // every entry that is not gated is named here on purpose — the neutral root,
    // which is not an agent, and the agents Estigia cannot gate yet, each of
    // which joins this list in the same change that adds it rather than by
    // failing a count.
    let documents = documented();
    if let Some(claimed) = number_before(&documents, "agents of seven") {
        assert_eq!(
            gated, claimed,
            "the documents claim {claimed} gated agents and {gated} are gated"
        );
    } else {
        assert!(
            documents.contains("every agent Estigia knows"),
            "no document makes a checkable claim about how many agents are gated"
        );
        let ungated: Vec<&str> = estigia::setup::AGENTS
            .iter()
            .filter(|adapter| !adapter.can_gate_tools())
            .map(|adapter| adapter.slug)
            .collect();
        assert_eq!(
            ungated,
            vec!["agents", "zcode", "deepseek-harness"],
            "the documents claim every agent is gated, and these are not: {ungated:?}"
        );
    }

    // The doctor checks are held to each other rather than to code: both phrases
    // live in `docs/honesty.md`, and their agreement is what can be held here.
    let phrases = ["things, not everything", "are about the past"];
    let claimed: Vec<usize> = phrases
        .iter()
        .map(|phrase| {
            number_before(&documents, phrase).unwrap_or_else(|| {
                panic!("no document carries a number before {phrase:?} any more")
            })
        })
        .collect();
    assert_eq!(
        claimed[0], claimed[1],
        "the documents say {} checks in one entry and {} in the next, three lines apart",
        claimed[0], claimed[1]
    );
}

/// The README's `setup` refusal offers exactly the adapters `setup` installs
/// into, and never a retired one.
///
/// The list is a copy of `AGENTS` a person maintains by hand, so nothing held
/// it: the retired slugs sat in it through an agent's retirement and every later
/// slice, and a reader who copied the line met `agent-retired`.
#[test]
fn the_readme_names_every_supported_agent_and_no_retired_one_as_an_install_target() {
    let readme = readme();
    let line = readme
        .lines()
        .find(|line| line.contains("setup --all") && line.contains("or one of:"))
        .expect("the README shows the no-agent refusal's list");
    let listed: Vec<&str> = line
        .split_once("or one of:")
        .expect("the list follows `or one of:`")
        .1
        .split(',')
        .map(str::trim)
        .collect();
    let supported: Vec<&str> = estigia::setup::AGENTS.iter().map(|a| a.slug).collect();
    assert_eq!(
        listed, supported,
        "the README's setup list is not the adapters `setup` installs into"
    );
}

#[test]
fn the_number_of_declared_populations_is_the_number_the_readme_claims() {
    // `tests/guards.rs` checks that every declaration is well formed and still
    // affirmed. Nothing checked that the README's count of them was true, so
    // adding `writing-shell` would have left the prose saying two.
    //
    // A declaration is a `pub const … : Population = Population { .. }` — see
    // `src/harness/population.rs` — since the vocabulary moved out
    // of doc comments and into code, so no test depends on the text of a Rust
    // comment. Matched on the type annotation rather than parsed with `syn`:
    // this test only has to count how many exist, not resolve or fingerprint
    // them, and `tests/guards.rs` already does the stronger reading.
    let documents = documented();
    let claimed = number_before(&documents, "`Population` consts")
        .expect("no document counts the declared populations");

    let mut declared = 0;
    let mut pending = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("a readable source directory") {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).expect("a readable source");
                declared += estigia::source_text::code_of(&source)
                    .matches(estigia::harness::population::DECLARATION_ASCRIPTION)
                    .count();
            }
        }
    }

    assert_eq!(
        declared, claimed,
        "the documents claim {claimed} declared populations and the source has {declared}"
    );
}

#[test]
fn the_number_of_things_doctor_checks_is_the_number_the_readme_claims() {
    let documents = documented();
    let claimed = number_before(&documents, "things, not everything")
        .expect("no document counts doctor's checks");
    // The **kinds**, not the rows. `contract`, `gate` and `tools` produce one
    // row per configured agent, so the rows a run reports depend on how many
    // adapters are configured; the forty-four this used to name was measured
    // when the table held eleven. The README's count was once being measured
    // against `examine`, which answered six checks at the time while the
    // command printed eight. A claim checked at the wrong scope, in the file
    // that exists to catch exactly that.
    let actual = estigia::harness::doctor::CHECKS.len();

    assert_eq!(
        actual, claimed,
        "the documents say `doctor` checks {claimed} things and it checks {actual}"
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
fn the_number_of_reserved_reviewers_is_the_number_every_document_claims() {
    // `docs/what-it-writes.md` also states the composition by naming every
    // file rather than counting them, so `number_before` has nothing to anchor
    // to there and it is left uncrossed rather than reworded to fit this test.
    let reserved = estigia::harness::roles::RESERVED_REVIEWERS.len();
    // The judge pair alone, both rows rendered from `REVIEW_AGENT` — distinct
    // from `reserved`, which also counts the lens's own row. Some sentences
    // state this narrower count on purpose, because they describe
    // `review-blind` and `review-blind-sealed` by name rather than every
    // reserved name; a lens is not a judge, so it is never one of the roles
    // those sentences enumerate.
    let judge_roles = estigia::harness::roles::RESERVED_REVIEWERS
        .iter()
        .filter(|&&(_, _, template)| template == estigia::skill::REVIEW_AGENT.contents)
        .count();

    let installed = estigia::harness::roles::RESERVED_REVIEWERS
        .iter()
        .filter(|&&(name, _, _)| !estigia::harness::roles::binary_launched_reviewer(name))
        .count();
    assert_eq!(
        installed, judge_roles,
        "the reviewers setup installs are no longer exactly the judges"
    );

    let documents = documented();
    let claimed = number_before(&documents, "reserved names carries which").expect(
        "no document counts the reserved reviewers beside \"reserved names carries which\"",
    );
    assert_eq!(
        reserved, claimed,
        "a document claims {claimed} reserved reviewers and the code has {reserved}"
    );
    for phrase in [
        "stable inert reviewer definitions",
        "inert definitions",
        // The path-check entry in `docs/honesty.md` counts the loop over the
        // installed `RESERVED_REVIEWERS` rows. This phrase sits in the line
        // that carries the number, so the count is crossed without a word
        // added for the test's benefit — a phrase the sentence would not
        // otherwise need is one a later reword drops and the crossing rots.
        "passes, one per",
    ] {
        let claimed = number_before(&documents, phrase).unwrap_or_else(|| {
            panic!("no document counts the installed reviewers beside {phrase:?}")
        });
        assert_eq!(
            installed, claimed,
            "a document claims {claimed} installed reviewers beside {phrase:?} and the code installs {installed}"
        );
    }
    // `docs/lifecycle.md` counts the judge's own two reserved names here, not
    // every reserved name — the sentence is about how many names one
    // definition (`REVIEW_AGENT`) is rendered under, and the lens is a second,
    // separate definition rather than a third name for the same one.
    let claimed = number_before(&documents, "names — and").unwrap_or_else(|| {
        panic!("no document counts the judge's own reserved names beside \"names — and\"")
    });
    assert_eq!(
        judge_roles, claimed,
        "a document claims {claimed} reserved names for the judge beside \"names — and\" and the code has {judge_roles}"
    );

    // Which definitions carry the basis paragraph is the judge count, not the
    // reserved count: the lens template omits `{{DISCIPLINE}}`, so its
    // rendering has no such paragraph. Four documents say it and each is
    // crossed where it lives — `docs/what-it-writes.md` and `README.md`
    // through `documented()` below, `skill/SKILL.md` and
    // `skill/policies/blind-judges.md` through their own reads further down.
    let claimed = number_before(&documents, "judge definitions carry the paragraph")
        .unwrap_or_else(|| {
            panic!("no document counts the definitions carrying the basis paragraph")
        });
    assert_eq!(
        judge_roles, claimed,
        "a document claims {claimed} definitions carry the basis paragraph and the code has {judge_roles}"
    );
    let claimed = number_before(&documents, "a paragraph asking").unwrap_or_else(|| {
        panic!(
            "no document counts the definitions carrying the basis paragraph beside \"a paragraph asking\""
        )
    });
    assert_eq!(
        judge_roles, claimed,
        "a document claims {claimed} definitions carry the basis paragraph and the code has {judge_roles}"
    );

    let blind_judges = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/policies/blind-judges.md"),
    )
    .expect("blind-judges ships");
    let judgment_day = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/policies/judgment-day.md"),
    )
    .expect("judgment-day ships");
    let claimed = number_before(&blind_judges, "judge definitions carry the paragraph")
        .unwrap_or_else(|| {
            panic!(
                "blind-judges.md does not count the definitions carrying the basis paragraph beside \"judge definitions carry the paragraph\""
            )
        });
    assert_eq!(
        judge_roles, claimed,
        "blind-judges.md claims {claimed} definitions carry the basis paragraph and the code has {judge_roles}"
    );
    // "Launches N reserved roles from one template" names the judge's own
    // template specifically — the lens renders from a second template, so it
    // is not one more role from this one.
    let claimed = number_before(&judgment_day, "reserved roles from one template").unwrap_or_else(|| {
        panic!(
            "judgment-day.md does not count the judge roles beside \"reserved roles from one template\""
        )
    });
    assert_eq!(
        judge_roles, claimed,
        "judgment-day.md claims {claimed} judge roles beside \"reserved roles from one template\" and the code has {judge_roles}"
    );
    let claimed =
        number_before(&blind_judges, "stable inert reviewer definitions").unwrap_or_else(|| {
            panic!(
                "blind-judges.md does not count the reserved reviewers beside \"stable inert reviewer definitions\""
            )
        });
    assert_eq!(
        installed, claimed,
        "blind-judges.md claims {claimed} installed reviewers beside \"stable inert reviewer definitions\" and the code installs {installed}"
    );

    let runtime_notes = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/references/runtime-notes.md"),
    )
    .expect("runtime-notes ships");
    let claimed = number_before(&runtime_notes, "definitions, `review-blind`")
        .expect("runtime-notes.md does not count the reserved reviewers");
    assert_eq!(
        installed, claimed,
        "runtime-notes.md claims {claimed} installed reviewers and the code installs {installed}"
    );
}

/// `number_before` has no entry for "zero" — the reserved-reviewer counts
/// above never need one, this one does for `low`. So the `low` claim (`0`)
/// is crossed by spelling the count into a fixed phrase instead and checking
/// that phrase for the word the derived count actually maps to, which still
/// fails if `RiskLevel::lenses()` changes under it. `medium` and `high` used
/// to share one prose sentence because both answered `1`; `high` was later given
/// its own lenses instead, so the two are now crossed against
/// separate phrases and against the reserved-lens count, which is what makes
/// this reddening rather than a number re-typed by hand alongside a
/// registry that moved.
#[test]
fn the_lens_count_stated_beside_review_tier_is_the_one_the_tier_derives() {
    fn spelled(count: u8) -> &'static str {
        match count {
            0 => "zero",
            1 => "one",
            4 => "four",
            other => panic!(
                "RiskLevel::lenses() answered {other}, which this reader has no spelled word \
                 for; extend it before trusting a prose claim about that count"
            ),
        }
    }

    let low = estigia::transport::tier::RiskLevel::Low.lenses();
    let medium = estigia::transport::tier::RiskLevel::Medium.lenses();
    let high = estigia::transport::tier::RiskLevel::High.lenses();
    assert_eq!(low, 0, "RiskLevel::Low.lenses() moved off zero");
    assert_eq!(medium, 1, "RiskLevel::Medium.lenses() moved off one");

    // `High` answers every sealed lens the registry carries except
    // `review-lens`, which only the delta-lens repair round launches.
    let sealed_lenses = estigia::harness::roles::RESERVED_REVIEWERS
        .iter()
        .filter(|&&(name, _, _)| estigia::harness::roles::reserved_lens(name))
        .count();
    assert_eq!(
        high as usize + 1,
        sealed_lenses,
        "RiskLevel::High.lenses() answers {high} and RESERVED_REVIEWERS carries {sealed_lenses} \
         sealed lenses, one of which is the delta lens"
    );
    for role in estigia::transport::tier::RiskLevel::High
        .lens_roles(estigia::transport::tier::ReviewFocus::default())
    {
        assert!(
            estigia::harness::roles::reserved_lens(role) && role != "review-lens",
            "{role} is allocated at high and is not a tier lens"
        );
    }

    let description = estigia::harness::mcp::tools::find("review_tier")
        .expect("review_tier is a registered tool")
        .description;
    let claimed = number_before(description, " lens, the `")
        .expect("the review_tier tool description does not count the medium lens question");
    assert_eq!(
        medium as usize, claimed,
        "the review_tier tool description claims {claimed} for the medium lens count and \
         RiskLevel::lenses() answers {medium}"
    );
    let claimed = number_before(description, "lenses, `review-risk`")
        .expect("the review_tier tool description does not count the high lens question");
    assert_eq!(
        high as usize, claimed,
        "the review_tier tool description claims {claimed} for the high lens count and \
         RiskLevel::lenses() answers {high}"
    );
    assert!(
        description.contains(&format!("`low` answers {}", spelled(low))),
        "the review_tier tool description does not say \"`low` answers {}\", which is what \
         RiskLevel::Low.lenses() ({low}) now claims",
        spelled(low)
    );

    let github = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/bindings/github/operations.md"),
    )
    .expect("skill/bindings/github/operations.md ships");
    let claimed = number_before(&github, " lens, the `").expect(
        "skill/bindings/github/operations.md's review_tier section does not count the medium lens \
         question",
    );
    assert_eq!(
        medium as usize, claimed,
        "skill/bindings/github/operations.md claims {claimed} for the medium lens count and \
         RiskLevel::lenses() answers {medium}"
    );
    let claimed = number_before(&github, "lenses, `review-risk`").expect(
        "skill/bindings/github/operations.md's review_tier section does not count the high lens \
         question",
    );
    assert_eq!(
        high as usize, claimed,
        "skill/bindings/github/operations.md claims {claimed} for the high lens count and \
         RiskLevel::lenses() answers {high}"
    );
    assert!(
        github.contains(&format!("`low` answers {}", spelled(low))),
        "skill/bindings/github/operations.md does not say \"`low` answers {}\", which is what \
         RiskLevel::Low.lenses() ({low}) now claims",
        spelled(low)
    );
}

/// A `high`-tier publication acquires the number of lenses `review_tier`
/// answers, and `medium` one — crossed against `RiskLevel::lenses()` rather
/// than asserted in prose, the same shape the tier's own count already gets
/// above. `skill/policies/blind-judges.md`'s "Lens acquisition" section states
/// the acquisition, and developer step 5 links to it rather than repeating the
/// count.
#[test]
fn the_lens_count_a_publication_acquires_is_the_one_review_tier_answers() {
    let medium = estigia::transport::tier::RiskLevel::Medium.lenses() as usize;
    let high = estigia::transport::tier::RiskLevel::High.lenses() as usize;

    let blind_judges = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/policies/blind-judges.md"),
    )
    .expect("skill/policies/blind-judges.md ships");
    let claimed = number_before(&blind_judges, " lens, the `").expect(
        "skill/policies/blind-judges.md's Lens acquisition section does not count the medium \
         acquisition",
    );
    assert_eq!(
        medium, claimed,
        "skill/policies/blind-judges.md claims {claimed} lenses acquired for medium and \
         RiskLevel::lenses() answers {medium}"
    );
    let claimed = number_before(&blind_judges, "lenses, `review-risk`").expect(
        "skill/policies/blind-judges.md's Lens acquisition section does not count the high \
         acquisition",
    );
    assert_eq!(
        high, claimed,
        "skill/policies/blind-judges.md claims {claimed} lenses acquired for high and \
         RiskLevel::lenses() answers {high}"
    );
}

/// Ordinary review licenses lenses only, so no carrier of the tier's allocation states a
/// judge count any more. Crosses every carrier the removed two-judge sentence lived in, and the
/// `review_tier` answer itself, against the phrases that sentence was made of.
#[test]
fn no_carrier_of_the_tiers_allocation_states_a_judge_count() {
    const TWO_JUDGE_ANCHOR: &str = "judges, one sealed and one unsealed; `low` acquires";

    let description = estigia::harness::mcp::tools::find("review_tier")
        .expect("review_tier is a registered tool")
        .description;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let carriers: [(&str, String); 8] = [
        (
            "the review_tier MCP tool description",
            description.to_owned(),
        ),
        (
            "skill/SKILL.md",
            std::fs::read_to_string(root.join("skill/SKILL.md")).expect("skill/SKILL.md ships"),
        ),
        (
            "skill/policies/blind-judges.md",
            std::fs::read_to_string(root.join("skill/policies/blind-judges.md"))
                .expect("skill/policies/blind-judges.md ships"),
        ),
        (
            "skill/references/runtime-notes.md",
            std::fs::read_to_string(root.join("skill/references/runtime-notes.md"))
                .expect("skill/references/runtime-notes.md ships"),
        ),
        (
            "skill/bindings/github.md",
            std::fs::read_to_string(root.join("skill/bindings/github.md"))
                .expect("skill/bindings/github.md ships"),
        ),
        (
            "skill/bindings/github/operations.md",
            std::fs::read_to_string(root.join("skill/bindings/github/operations.md"))
                .expect("skill/bindings/github/operations.md ships"),
        ),
        (
            "README.md",
            std::fs::read_to_string(root.join("README.md")).expect("README.md ships"),
        ),
        (
            "docs/configuration.md",
            std::fs::read_to_string(root.join("docs/configuration.md"))
                .expect("docs/configuration.md ships"),
        ),
    ];

    for (name, text) in &carriers {
        assert!(
            !text.contains(TWO_JUDGE_ANCHOR),
            "{name} still states a per-tier judge count beside {TWO_JUDGE_ANCHOR:?}"
        );
    }
}

#[test]
fn the_two_src_only_carriers_render_the_lens_counts_the_tier_derives_and_no_judges() {
    let opencode = estigia::setup::opencode_review_section();
    let opencode_compact: String = opencode.chars().filter(|c| !c.is_whitespace()).collect();

    let medium_lenses = estigia::transport::tier::RiskLevel::Medium.lenses_spelled();
    let high_lenses = estigia::transport::tier::RiskLevel::High.lenses_spelled();
    let low_lenses = estigia::transport::tier::RiskLevel::Low.lenses_spelled();
    assert_eq!(medium_lenses, "one");
    assert_eq!(high_lenses, "four");
    assert_eq!(low_lenses, "zero");
    for (phrase, what) in [
        (format!("{medium_lenses} at `medium`"), "medium lens count"),
        (format!("{high_lenses} at `high`"), "high lens count"),
        (format!("{low_lenses} at `low`"), "low lens count"),
    ] {
        let compact: String = phrase.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            opencode_compact.contains(&compact),
            "opencode_review_section does not render {phrase:?} ({what})"
        );
    }

    let selected = estigia::skill::selected_documents(&estigia::config::Config::default());
    for (phrase, what) in [
        (format!("{medium_lenses} at `medium`"), "medium lens count"),
        (format!("{high_lenses} at `high`"), "high lens count"),
        (format!("{low_lenses} at `low`"), "low lens count"),
    ] {
        assert!(
            selected.contains(&phrase),
            "selected_documents does not render {phrase:?} ({what})"
        );
    }
}

#[test]
fn the_opencode_section_names_every_launch_lane_state_the_contract_names() {
    // Crosses the rendered section against the contract's own lane list rather
    // than typing the states twice: a review found the copies had drifted.
    // The two lists are compared for equality, not one against the other: a
    // rendered list that *adds* a state the contract does not name is the same
    // drift in the other direction, and the old containment let it pass.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let notes = std::fs::read_to_string(root.join("skill/references/runtime-notes.md"))
        .expect("runtime-notes.md ships");
    let contract = lane_states(&notes);
    assert!(
        contract.len() >= 5,
        "only {} lane states were read from the contract — the crossing is broken",
        contract.len()
    );

    let opencode = estigia::setup::opencode_review_section();
    assert_eq!(
        lane_states(&opencode),
        contract,
        "the rendered section and the contract name different launch lane states: {opencode}"
    );
    let compact: String = opencode.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("notred,notunreadable"),
        "the rendered section no longer excludes red and unreadable: {opencode}"
    );
}

/// The launch lane states a text names between `a publication lane that is `
/// and the exclusion clause that follows, whitespace-normalised so two copies
/// can be compared as whole lists rather than one against the other.
fn lane_states(text: &str) -> Vec<String> {
    let anchor = "a publication lane that is ";
    let start = text
        .find(anchor)
        .unwrap_or_else(|| panic!("the text no longer names the launch lane states: {text}"))
        + anchor.len();
    let rest = &text[start..];
    let end = rest
        .find("not red")
        .unwrap_or_else(|| panic!("the lane list no longer carries its exclusion clause: {text}"));
    rest[..end]
        .trim_end_matches(|c: char| {
            c == '(' || c == '\u{2014}' || c == '\u{2013}' || c == '-' || c.is_whitespace()
        })
        .split(',')
        .map(|state| {
            state
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        })
        .filter(|state| !state.is_empty())
        .collect()
}

/// The completeness guard, widened in a repair
/// round after a frozen four-phrase list missed all three prose severes that
/// round's panel found, and widened a second time after the widened list
/// itself missed a fourth — `"both confirm"`, caught by nothing because the
/// list held `"both judges"`. Adding a fifth phrase would be the same
/// failure a third time: a remembered set losing to whatever words the next
/// round happens to write. This inverts the check instead. The invariant is
/// not a list of forbidden sentences; it is that a sentence making a
/// panel-count or confirmation claim must be **scoped** to the tier it is
/// true of. So this flags a *clause* that pairs a duality word with a panel
/// noun and does not also name `medium`, `high` or `low` in that same
/// clause — whatever words the duality claim itself uses — with only a
/// named, reasoned exception let through. A new absolute is then caught
/// however it is phrased; only its scoping saves it.
///
/// **Not swept: `src/`.** `src/setup/reviewer_before_*.md` are frozen
/// historical templates — including `reviewer_before_tier_scaled_panel.md` — and they are *supposed* to carry the old
/// wording, since upgrade detection depends on recognising exactly that
/// text. A blanket `src/` sweep would fail on the files this repository
/// requires to stay unchanged. Two `src/`-only carriers this change
/// touches are reached below by calling them and reading what they render,
/// not by scanning the source that builds them: `opencode_review_section()`,
/// the panel-launch text an OpenCode host reads, and `selected_documents()`,
/// which renders the unconditional blind-judges pointer into every
/// installed instruction file.
///
/// **The substring hole is closed, not merely narrowed.** A first version of
/// this sweep matched `SCOPE_WORDS` with plain `str::contains`, so `low`
/// matched inside `below`, `follow`, `allow`, `flow` and `lower`, and `high`
/// matched inside `higher` and `highlight` — a panel found it by mutation,
/// changing one word at `skill/policies/blind-judges.md:53` from `below` to
/// `beneath` and watching a clause the sweep had accepted turn red. The same
/// shape over-fires the other way for `DUALITY_WORDS` and `PANEL_NOUNS` —
/// `two` inside `twofold` — so all three lists are matched at word
/// boundaries now (a plural `s` still counts as the same word: `judge`
/// matches `judges`), the same technique `number_before` above already uses
/// and explains why "one" must not match inside "none".
///
/// **What is still true after that fix, stated plainly rather than left
/// implied.** `DUALITY_WORDS` and `PANEL_NOUNS` remain finite lists, chosen
/// from words this repository's own carriers have actually used — a clause
/// that asserts an unreduced panel with neither a listed duality word nor a
/// listed panel noun, in vocabulary this list was never given, passes
/// unseen. That is the same shape of gap the four-phrase list this sweep
/// replaces had, one level up, and it is inherent to a lexical sweep bounded
/// by any finite vocabulary — closing the substring hole did not touch it.
/// `CLAUSE_BOUNDARIES` approximates a sentence with punctuation, which is
/// blind to a duality word and its scoping tier separated by a comma inside
/// one long clause, or joined falsely across a boundary this crude a
/// splitter should not have crossed. `EXEMPTED` is adjudicated by a reviewer
/// reading each reason, not verified mechanically against anything except
/// that its text still exists — a reason that stops being true is a reason
/// nobody here re-checks. Below, every word list and the clause split
/// itself are checked for finding *something* in the live corpus, so a
/// typo or a retired word fails loudly instead of the sweep quietly passing
/// because it stopped checking anything.
///
/// Whether `needle` occurs in `haystack` as a whole word, the same rule
/// `number_before` above states for why "one" must not match inside "none":
/// the byte before the match, if any, and the byte after it are not
/// alphanumeric. One extension past that function's own rule — this sweep's
/// nouns plainly pluralise where a spelled-out number never does — a single
/// trailing `s` still counts as the boundary, so `judge` matches `judges`
/// and `` `task` context `` matches `` `task` contexts ``.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut search_from = 0;
    while let Some(relative) = haystack[search_from..].find(needle) {
        let start = search_from + relative;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_ascii_alphanumeric());
        let after = if bytes.get(end) == Some(&b's') {
            end + 1
        } else {
            end
        };
        let after_ok = haystack[after..]
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        search_from = start + 1;
    }
    false
}

#[test]
fn no_skill_clause_asserts_an_unscoped_panel_count_or_confirmation() {
    /// Words that turn a panel-noun clause into an absolute claim about how
    /// many judges there are or what they must agree on — not every word
    /// that could ever mean that, only the ones a real carrier has actually
    /// used, across two rounds of this sweep missing different ones.
    const DUALITY_WORDS: &[&str] = &["both", "two", "always"];
    /// What makes a duality word above actually a claim about the panel,
    /// rather than about something else entirely that happens to share a
    /// clause with it.
    const PANEL_NOUNS: &[&str] = &["judge", "panel"];
    /// What saves a clause that pairs a duality word with a panel noun: it
    /// also names which tier the claim is true of, in the same clause.
    const SCOPE_WORDS: &[&str] = &["medium", "high", "low"];
    /// Punctuation this prose uses as a clause boundary at least as often as
    /// a period — this repository writes long compound sentences joined by
    /// colons, semicolons and em dashes, and splitting only on `.` would
    /// leave an unscoped claim and its scoping tier looking like one clause
    /// merely because a period never separated them.
    const CLAUSE_BOUNDARIES: &[char] = &['.', ';', ':', '\u{2014}'];

    /// One clause that is still true unscoped, and the reason it is
    /// exempted rather than fixed or banned. Matched by its own exact text,
    /// not by file and phrase alone, so it excuses only this occurrence and
    /// not every occurrence of the same words in the same file.
    struct Exempted {
        file: &'static str,
        text: &'static str,
        reason: &'static str,
    }

    const EXEMPTED: &[Exempted] = &[
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "No two judges are ever pointed at one directory",
            reason: "a general isolation invariant over every judge that may write; not a \
                     claim about how many judges exist",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "Two measuring judges in one directory are concurrent writers",
            reason: "an illustrative example inside the isolation rule, not a claim about how \
                     many judges a review acquires",
        },
        Exempted {
            file: "skill/policies/blind-judges.md",
            text: "Estigia records the two ends and does not compute the",
            reason: "'two ends' names the delta's base and head commits, not a judge count; \
                     'judge' appears later in the same clause by coincidence",
        },
        Exempted {
            file: "skill/policies/blind-judges.md",
            text: "The two judge definitions carry the paragraph asking for\nwhat a closing statement rests on",
            reason: "describes the two fixed judge-role templates, not how many a review \
                     launches",
        },
        Exempted {
            file: "skill/agents/review-blind.md",
            text: "without both, stop and report that the panel invocation is incomplete",
            reason: "'both' refers to the receipt and the criteria a launch prompt must name, \
                     not to judge count",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "Judgment Day is a two-judge adversarial method",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "Estigia installs its two judges and states the rules below",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "Two judges agreeing is its corroboration",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "only as much of the earlier two-judge panel as Judgment Day still needs",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "One immutable target, two judges, in parallel, blind",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/judgment-day.md",
            text: "Launch both judge\ncontexts concurrently",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/blind-judges.md",
            text: "Under Judgment Day, two judges agreeing on a finding",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/blind-judges.md",
            text: "Judgment Day's two\njudges are `review-blind`",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/blind-judges.md",
            text: "Judgment Day's two judge definitions are fixed, not configured",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/policies/blind-judges.md",
            text: "launching both judges",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/agents/review-blind.md",
            text: "One of Judgment Day's two judges",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
        Exempted {
            file: "skill/references/runtime-notes.md",
            text: "Judgment Day's two judges are",
            reason: "states Judgment Day's fixed two-judge composition, which no tier selects \
                     and which ordinary review never launches",
        },
    ];

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let skill_root = root.join("skill");
    let mut files = Vec::new();
    let mut stack = vec![skill_root];
    while let Some(directory) = stack.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }
    assert!(
        files.len() > 10,
        "{} file(s) found under skill/, fewer than this crate ships — the walk stopped finding files",
        files.len()
    );

    let mut sources: Vec<(String, String)> = files
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{rel} must be readable: {error}"));
            (rel, text)
        })
        .collect();
    // Two `src/`-only carriers, reached by calling them and reading what
    // they render — see the doc comment above for which and why.
    sources.push((
        "src/setup/mod.rs: opencode_review_section()".to_owned(),
        estigia::setup::opencode_review_section(),
    ));
    sources.push((
        "src/skill.rs: selected_documents()".to_owned(),
        estigia::skill::selected_documents(&estigia::config::Config::default()),
    ));

    for exempted in EXEMPTED {
        assert!(
            exempted.reason.len() > 20,
            "{}: an exemption for {:?} does not say why it is still true",
            exempted.file,
            exempted.text
        );
        let source = sources
            .iter()
            .find(|(rel, _)| rel == exempted.file)
            .unwrap_or_else(|| panic!("{} names no swept source", exempted.file));
        assert!(
            source
                .1
                .to_lowercase()
                .contains(&exempted.text.to_lowercase()),
            "{}: the exempted text {:?} is not in the file any more \u{2014} a stale exemption \
             excuses nothing and should be deleted",
            exempted.file,
            exempted.text
        );
    }

    // Every extraction below must find *something* in the live corpus, the
    // same way the source project this sweep's shape is borrowed from fails
    // a guard whose "extraction regexp is stale" or that "has lost its
    // subject" rather than letting it report a clean pass it never actually
    // ran. A word list is exercised at all only if some source, somewhere,
    // uses that exact word — checked per word, not per list, so one typo in
    // `DUALITY_WORDS` or `PANEL_NOUNS` cannot hide behind its neighbours
    // still matching, and per separator for the same reason. Counting the
    // clauses `split` returns would prove nothing: it yields the whole slice
    // when the pattern never matches, so a count is positive even when every
    // boundary is stale.
    assert!(
        !sources.is_empty(),
        "the file walk above swept nothing at all \u{2014} every source below would be \
         checked against an empty corpus and pass"
    );
    for boundary in CLAUSE_BOUNDARIES {
        assert!(
            sources.iter().any(|(_, text)| text.contains(*boundary)),
            "no swept source contains the clause boundary {boundary:?} \u{2014} splitting on it \
             divides nothing, so it is stale or the corpus stopped using it"
        );
    }
    for word in DUALITY_WORDS {
        assert!(
            sources
                .iter()
                .any(|(_, text)| contains_word(&text.to_lowercase(), word)),
            "no swept source uses the duality word {word:?} at all \u{2014} this sweep would \
             pass clean while checking nothing for it; the word is stale or the corpus moved"
        );
    }
    for noun in PANEL_NOUNS {
        assert!(
            sources
                .iter()
                .any(|(_, text)| contains_word(&text.to_lowercase(), noun)),
            "no swept source uses the panel noun {noun:?} at all \u{2014} this sweep would pass \
             clean while checking nothing for it; the noun is stale or the corpus moved"
        );
    }
    for scope in SCOPE_WORDS {
        assert!(
            sources
                .iter()
                .any(|(_, text)| contains_word(&text.to_lowercase(), scope)),
            "no swept source names the tier {scope:?} at all \u{2014} the scoping half of this \
             sweep would exempt nothing it should, silently"
        );
    }

    let mut offending: Vec<String> = Vec::new();
    for (rel, text) in &sources {
        let lower = text.to_lowercase();
        let exempted_clauses: Vec<&str> = EXEMPTED
            .iter()
            .filter(|exempted| exempted.file == rel)
            .map(|exempted| exempted.text)
            .collect();
        for clause in lower.split(CLAUSE_BOUNDARIES) {
            let has_duality = DUALITY_WORDS.iter().any(|word| contains_word(clause, word));
            let has_panel_noun = PANEL_NOUNS.iter().any(|noun| contains_word(clause, noun));
            if !has_duality || !has_panel_noun {
                continue;
            }
            if SCOPE_WORDS.iter().any(|scope| contains_word(clause, scope)) {
                continue;
            }
            let exempt = exempted_clauses
                .iter()
                .any(|exempted| clause.contains(exempted.to_lowercase().as_str()));
            if exempt {
                continue;
            }
            offending.push(format!("{rel}: unscoped clause: {:?}", clause.trim()));
        }
    }
    assert!(
        offending.is_empty(),
        "a shipped clause claims a panel count or confirmation without naming which tier it is \
         true of: {offending:#?}"
    );
}

/// The first acceptance criterion, still owed until this test: every
/// sentence that used to say nothing launches a lens is gone or narrowed,
/// and this reddens if a shipped surface still says so.
///
/// **`SCOPED` is not a wholesale exemption.** Its two files are swept the
/// same as every other one below; what excuses one occurrence of a `CLAIMS`
/// phrase there is an `OWNER_REFERENCE` naming the owner within
/// `OWNER_WINDOW` bytes of it, not the file carrying that reference
/// anywhere at all — delete the clause beside the claim and that
/// occurrence offends like any other, proven as a mutation on both
/// `SCOPED` files, not only one.
///
/// **Matching is case-insensitive on the document side.** A sentence-initial
/// *"Nothing launches a lens"* used to pass a byte-exact walk against seven
/// lowercase phrases; the text this sweep reads is lowered once before any
/// `CLAIMS`/`OWNER_REFERENCE` search, and every entry in both lists is
/// already lowercase, so a capitalised occurrence is caught the same as a
/// lowercase one — proven as a mutation, not only asserted.
///
/// **What this cannot do, stated plainly rather than left implied.** A
/// phrase sweep catches the phrasings named in `CLAIMS` and nothing past
/// them — a rewording that avoids every one of them would not be caught
/// here, the same limit `docs/honesty.md` names for this exact test.
/// Measured, not assumed: an earlier pass over these eleven surfaces, done
/// by eye rather than by this sweep, missed
/// `skill/references/runtime-notes.md`'s wording — *"none of which either of
/// these two paragraphs launches"* — on the first read, and this sweep's own
/// first `CLAIMS` list missed it too, on its own first run — one surface,
/// missed twice, by two independent readers, both by grep. A round later, a
/// judge found a thirteenth surface neither reader nor this sweep could
/// reach: `CHANGELOG.md:69` carried a `CLAIMS` phrase verbatim, surviving
/// for the two reasons this sweep still cannot close on its own —
/// `CHANGELOG.md` is outside the walk below, deliberately, the same
/// exclusion `documented()` gives it elsewhere in this file for being an
/// append-only record rather than a current claim; the text there is
/// corrected at the source instead. `docs/honesty.md` states the full
/// measurement. `docs/honesty.md` itself is not scanned here for a related
/// but different reason: it is where the old, false claim is narrated by
/// name on purpose, in the past tense, so a literal-phrase sweep cannot
/// tell that narration from an assertion.
#[test]
fn no_shipped_surface_still_claims_nothing_launches_a_lens() {
    const CLAIMS: &[&str] = &[
        "none launched yet",
        "launched yet",
        "nothing yet launches a lens",
        "nothing launches a lens",
        "none launches",
        // Measured, not assumed: the operator's own phrase-grep missed
        // `skill/references/runtime-notes.md:48` when the eleven surfaces
        // were first listed, and this sweep missed it too on its first
        // run — one surface, two independent readers, both by grep. Both
        // phrasings below are the ones that slipped through; `docs/honesty.md`
        // states the measurement.
        "never launches any of them",
        "paragraphs launches",
        // Wordings this change itself deleted — from `docs/what-it-writes.md`
        // and `skill/agents/review-lens.md`, both at base — that the list above
        // never gained, so restoring either passed the sweep silently until now.
        "nothing yet launches any of them",
        "though nothing does so yet",
    ];
    // Legitimately scoped to a narrower subject than "does a lens ever run
    // anywhere in this repository": each names one paragraph, or one
    // installed composition, that itself launches nothing. Named here, but
    // the tolerance below is earned per file, not granted by this list alone.
    const SCOPED: &[&str] = &["skill/references/runtime-notes.md", "docs/configuration.md"];
    // What buys a `SCOPED` file its tolerance: the owner reference has to sit
    // beside the specific claim occurrence, not merely appear somewhere in
    // the same file. `text.contains(owner)` over the whole file used to be
    // the check, and `docs/configuration.md` proved it decorative: line 58
    // names `skill/SKILL.md` developer step 5` for an unrelated sentence
    // 611 bytes from the added clause at line 64, so deleting that clause
    // left the file still exempt on the strength of a phrase nowhere near
    // the claim. `OWNER_WINDOW` is measured, not guessed: the owner clause
    // this issue actually wrote sits 22 and 32 bytes from its claim in the
    // two `SCOPED` files, and the pre-existing unrelated occurrence sits 611
    // — wide enough apart that no window between those two numbers is a
    // close call. Measured in bytes, not characters: a naive `str::find`
    // over Python's Unicode string type undercounts by a few for every
    // multi-byte em dash it crosses, which is exactly the gap between the
    // 590/47 an earlier pass reported and the 611/32 this comment now
    // states — re-derive with the bytes read from disk, never a decoded
    // string's character offsets. `skill/references/runtime-notes.md` currently
    // names `skill/SKILL.md` developer step 5 for that; a file naming
    // `blind-judges.md`'s "Lens acquisition" section instead, and not this
    // phrase, would need its own entry here rather than being silently accepted
    // on the strength of the other file's wording.
    const OWNER_REFERENCE: &[&str] = &["skill/SKILL.md` developer step 5"];
    const OWNER_WINDOW: usize = 200;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut documents: Vec<PathBuf> = vec![root.join("README.md")];
    let mut stack = vec![root.join("skill"), root.join("docs")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "md") {
                documents.push(path);
            }
        }
    }
    assert!(
        documents.len() > 20,
        "{} document(s) found, fewer than this crate ships \u{2014} the walk stopped finding files",
        documents.len()
    );

    let mut offending: Vec<String> = Vec::new();
    for path in documents {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        // the honesty documents narrate the retired claim by name; see the
        // reasoning above.
        if rel.starts_with("docs/honesty") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Lowered once, here, so every `CLAIMS`/`OWNER_REFERENCE` entry — already
        // lowercase — is matched case-insensitively: a sentence-initial
        // "Nothing launches a lens" is the same offence as the lowercase
        // form. Safe for byte offsets: every character either lowering
        // touches in this crate's own prose is ASCII, one byte before and
        // after, so `claim_at`/`owner_at` still index the same bytes.
        let text = text.to_lowercase();
        let scoped = SCOPED.iter().any(|scoped| rel == *scoped);
        for claim in CLAIMS {
            for (claim_at, _) in text.match_indices(claim) {
                // A `SCOPED` file is tolerated per occurrence, not per file:
                // this exact claim needs an owner reference within
                // `OWNER_WINDOW` bytes of it, not merely somewhere in the
                // same document. Delete the clause naming the owner beside
                // this claim and the occurrence offends like any other,
                // whatever else the file happens to say elsewhere.
                let earned = scoped
                    && OWNER_REFERENCE.iter().any(|owner| {
                        text.match_indices(&owner.to_lowercase())
                            .any(|(owner_at, _)| claim_at.abs_diff(owner_at) <= OWNER_WINDOW)
                    });
                if !earned {
                    offending.push(format!("{rel}: {claim:?}"));
                }
            }
        }
    }
    assert!(
        offending.is_empty(),
        "a shipped surface still claims nothing launches a lens, though \
         skill/policies/blind-judges.md now acquires one: {offending:?}"
    );
}

#[test]
fn every_mechanism_the_readme_names_is_one_the_code_still_uses() {
    // A name that has drifted sends a reader to look for something that is not
    // there — and these are the names somebody greps for when a gate did not
    // fire.
    let documents = documented();
    for phrase in [
        "PreToolUse",
        "BeforeTool",
        "preToolUse",
        "tool.execute.before",
        "permissionDecision",
        "pre-push",
    ] {
        assert!(
            documents.contains(phrase),
            "no document names `{phrase}` any more, which the code still uses"
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
    // `every_count_a_document_claims_is_one_the_crate_has`, and the companion count
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
            // A `tests.rs` may put whatever it likes where it likes.
            if !path.extension().is_some_and(|kind| kind == "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            read += 1;
            let text = estigia::source_text::code_of(&text);
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

    // The population is the vocabulary the binary ships — `TRANSPORT_VOCABULARY`,
    // generated by `build.rs` from all three spellings a refusal can take and
    // `include!`d into `harness::tracker`, where `stable_code` looks each reason
    // up. A walk here over `src/transport` would be a second extractor, and it
    // was: it read one spelling of the three and reported green over the rest.
    let unknown: Vec<&str> = estigia::harness::tracker::TRANSPORT_VOCABULARY
        .iter()
        .copied()
        .filter(|code| !prose.contains(code))
        .collect();
    // The ones already known to be undocumented do not fail this. New ones do.
    //
    // **This is a gap, and it is open.** The check used to accept a reason
    // spelled anywhere in `github.py` as documented, and the binding's source
    // is not something an agent reads — it stood in for the prose and hid
    // every refusal this list now names. Deleting the binding did not create
    // them; it stopped concealing them. The count lives in the honesty
    // contract and not here, because a second copy of a number that nothing
    // checks is how it goes out of date.
    //
    // Frozen rather than dropped, because the direction that matters still
    // holds: a *new* refusal with no instructions is red on the review that
    // adds it, and the list can only be made shorter.
    let fresh: Vec<&str> = unknown
        .iter()
        .copied()
        .filter(|code| !UNDOCUMENTED_REFUSALS.contains(code))
        .collect();
    assert!(
        fresh.is_empty(),
        "these refusals are spelled by nothing the agent reads: {fresh:?}"
    );
    // And the list is not allowed to rot: an entry that no longer names a
    // refusal is one somebody documented, or renamed, and either way the line
    // stops describing the code.
    let stale: Vec<&str> = UNDOCUMENTED_REFUSALS
        .iter()
        .copied()
        .filter(|code| !unknown.contains(code))
        .collect();
    assert!(
        stale.is_empty(),
        "these are listed as undocumented and are not: {stale:?} \u{2014} take them off the list"
    );
}

/// The refusals the transport can answer with that no prose an agent reads
/// names, re-derived against [`estigia::harness::tracker::TRANSPORT_VOCABULARY`]
/// — the list `build.rs` generates and `stable_code` reads.
///
/// Measured, on the day `skill/scripts/github.py` was deleted. The check above
/// counted that file as documentation, which it never was for anybody running
/// an agent — so this is not a regression, it is a measurement that was being
/// prevented. See the honesty contract for the count and for the seven codes
/// the full vocabulary added to it.
const UNDOCUMENTED_REFUSALS: &[&str] = &[
    "already-owned-by-different-operation",
    "already-yours",
    "ambiguous-changelog-entry",
    "ambiguous-write",
    "ancestry-unknown",
    "base-moved-during-creation",
    "board-readback-failed",
    "branch-locked-by-another-run",
    "changelog-not-found",
    "claim-operation-conflict",
    "claim-operation-expired",
    "claim-operation-no-longer-current",
    "comment-body-invalid",
    "comment-marker-incomplete",
    "empty-changelog-entry",
    "force-reason-invalid",
    "force-reason-required",
    "force-required-for-reason",
    "fresh-branch-diverged",
    "heartbeat-body-invalid",
    "held-by-other-holder-disappeared",
    "held-by-other-without-other-holder",
    "holder-runtime-missing",
    "invalid-horizon",
    "invalid-marker-attribute",
    "invalid-unassign-target",
    "label-readback-failed",
    "lost-claim-race",
    "missing-pr-body",
    "never-ran-evidence-invalid",
    "no-changelog-entry",
    "no-worktree-location",
    "not-current-live-holder",
    "nothing-to-reclaim",
    "nothing-to-unassign",
    "ownership-changed-projections-repaired",
    "ownership-kept-changing-before-projection",
    "read-failed",
    "reclaim-metadata-mismatch",
    "reclaim-operation-conflict",
    "reclaim-operation-no-longer-current",
    "reserved-comment-kind",
    "reserved-device-component",
    "review-finding-metadata-mismatch",
    "review-handoff-metadata-mismatch",
    "review-target-mismatch",
    "spend-metadata-mismatch",
    "stale-foreign-requires-reclaim",
    "unassign-metadata-mismatch",
    "unassign-operation-conflict",
    "unsafe-run-id",
    "unsafe-worktree-component",
    "write-failed",
];

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
    ("publish_review", "publish_with_sleep", 2),
    ("republish_review", "publish_with_sleep", 2),
];
// `publish_with` itself is a one-line delegation to the sleep seam and holds
// no verification — point this at it and the count reads the wrapper instead
// of the body both routes share.

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

    let claim = estigia::source_text::code_of(
        &std::fs::read_to_string(root.join("src").join("transport").join("claim.rs"))
            .expect("the port ships with the crate"),
    );
    let branch = estigia::source_text::code_of(
        &std::fs::read_to_string(root.join("src").join("transport").join("branch.rs"))
            .expect("the port ships with the crate"),
    );

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
        // `verify_held` is `verify_claim` over a ledger the caller already opened,
        // so the write that follows records into the same store without asking
        // GitHub twice whose issue it is; it is the same verification.
        let found = body.matches("verify_claim(").count() + body.matches("verify_held(").count();
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

/// Reads one of this crate's own source files, with every comment taken out.
///
/// Line endings normalised: some files in this crate are CRLF and some are LF,
/// and a scan that anchors on a closing brace at the start of a line would
/// silently measure nothing on half of them. Comments stripped for the same
/// reason every other reader in this file is: the callers slice a function
/// body out of the raw text and pull identifiers out of it by searching for a
/// literal prefix, and a comment inside that slice naming the same prefix
/// would otherwise be read as a second occurrence.
fn source(relative: &str) -> String {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("{relative} ships with the crate: {error}"))
        .replace("\r\n", "\n");
    estigia::source_text::code_of(&text)
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
/// Most adapters share one skill root, so the only way to say
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
        if !matches!(setting.scope(), Scope::Everywhere | Scope::Repository) {
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

/// The crossing a blind panel asked for after a fourth cut: the
/// config fields `GateContext` (`src/harness/mod.rs`) is built from, tested
/// against the settings `Setting::decides_what_the_gate_enforces` must
/// answer `true` for.
///
/// Not a `Population` declaration, and deliberately: that predicate is a
/// method on `impl Setting`, and a declaration binds to an item rather than
/// to a method. `tests/guards.rs` does now reach an associated const inside
/// an `impl`, so the old reason — that its walker read only top-level items
/// and a declaration written there would look checked and never be found —
/// no longer holds; this crossing is simply the thing that measures the
/// property, and a declaration would only restate it.
///
/// The predicate's own `match` is exhaustive over `Setting`, which stops a
/// new *row* from riding unanswered, but nothing stopped a new
/// `GateContext` *field* from doing exactly that — add one tomorrow, wire
/// it from a sixth setting, and the suite stayed green while that row rode
/// the window. Both sides are derived rather than listed: which fields the
/// gate reads comes from `gate_context`'s own body, the same extraction
/// `every_setting_the_gate_reads_is_one_that_does_not_differ_by_agent`
/// above already trusts for the same file, read again for a different
/// question; which setting answers `true` comes from
/// `Setting::decides_what_the_gate_enforces` itself. A row `gate_context`
/// reads and the predicate does not mark, or a row the predicate marks
/// that `gate_context` never reads, is either a row that rides the renewal
/// window unguarded or a claim about the gate that the gate does not
/// back.
#[test]
fn every_setting_the_gate_enforces_is_one_gate_context_actually_carries() {
    let cli = source("src/cli/mod.rs");
    let opens = cli
        .find("fn gate_context(")
        .expect("the gate builds its context somewhere");
    let body = &cli[opens..];
    let closes = body
        .find(
            "
}
",
        )
        .expect("the function ends");
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

    let settings_source = source("src/config/settings.rs");
    let opens = settings_source
        .find("fn parse_into(")
        .expect("settings are parsed into a configuration somewhere");
    let arms = &settings_source[opens..];
    let closes = arms
        .find(
            "
    }
",
        )
        .expect("the method ends");
    let arms = &arms[..closes];
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

    // Every field the gate actually reads, resolved to the setting that
    // writes it — the population `decides_what_the_gate_enforces` has to
    // match exactly.
    let mut carried: Vec<String> = reads
        .iter()
        .filter_map(|field| writes.iter().find(|(written, _)| written == field))
        .map(|(_, variant)| variant.clone())
        .collect();
    carried.sort();
    carried.dedup();
    assert!(
        carried.len() >= 5,
        "only {} settings were resolved from GateContext's own fields, so this refutes nothing:          {carried:?}",
        carried.len()
    );

    let mut answers_true: Vec<String> = SETTINGS
        .iter()
        .filter(|setting| setting.decides_what_the_gate_enforces())
        .map(|setting| format!("{setting:?}"))
        .collect();
    answers_true.sort();

    assert_eq!(
        carried, answers_true,
        "GateContext's own fields and `Setting::decides_what_the_gate_enforces` name different          settings — a field was added to one without the other, so a row the gate reads to          decide is not marked guarded, or a row marked guarded is not one the gate actually          reads"
    );
}

/// Every name the list declares is one `doctor` can actually report.
///
/// The way back. Its neighbour walks the rows `full` emits and proves the list
/// declares each one; this walks the list and proves `doctor` can produce it.
/// Only one of the two was written, and the unwritten one is the direction the
/// README leans on: the count before *"things, not everything"* comes from this
/// list, so a name left here after its check was taken out of `full` inflates a
/// number the honesty contract exists to keep true — and the neighbour goes on
/// passing, because it only ever looks at rows that exist.
///
/// With an agent configured, and that is the whole reason this is a separate
/// test: `gate` and `tools` produce one row per configured agent and appear in
/// nothing on a bare machine. Crossing this direction against an empty home
/// would report them as phantoms every time.
#[test]
fn every_name_this_list_declares_is_one_doctor_can_report() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(root.path().join("Cargo.lock"), "").expect("a lockfile");
    for arguments in [["init", "--quiet"], ["add", "Cargo.lock"]] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(arguments)
            .status()
            .expect("git on the path");
        assert!(status.success(), "git {arguments:?} failed");
    }
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
            "the list declares {name:?} and `doctor` never reports it \u{2014} the honesty contract counts \
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
        "never reduces or serializes the configured review",
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
    let source = estigia::source_text::code_of(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/harness/mod.rs"),
        )
        .expect("the harness ships with the crate"),
    );
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
/// `CHANGELOG.md`, and everything under `docs/`, which cross-reference each
/// other on purpose so that one document answers one question.
///
/// `docs/` is walked rather than listed, and that is not tidiness: documents
/// there were carved out of the README, every pointer left behind is a link,
/// and the walk is what makes splitting a document safe to do again.
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
    let mut pending = vec![root.join("docs")];
    let mut documents: Vec<PathBuf> = ["README.md", "AGENTS.md", "CHANGELOG.md"]
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

/// Every backticked span on a line, with the byte offset of its opening backtick.
///
/// A ``double-backtick`` span is a literal quoting backticks, not a citation, so
/// its content is skipped rather than read as one.
fn backticked(line: &str) -> Vec<(usize, &str)> {
    let mut spans = Vec::new();
    let mut rest = line;
    let mut offset = 0;
    while let Some(open) = rest.find('`') {
        let run = rest[open..].chars().take_while(|c| *c == '`').count();
        if run >= 2 {
            let fence = "`".repeat(run);
            let after = &rest[open + run..];
            let Some(close) = after.find(&fence) else {
                break;
            };
            let consumed = open + run + close + run;
            offset += consumed;
            rest = &rest[consumed..];
            continue;
        }
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        spans.push((offset + open, &after[..close]));
        let consumed = open + 1 + close + 1;
        offset += consumed;
        rest = &rest[consumed..];
    }
    spans
}

/// The definition a line of Rust introduces, if it introduces one.
///
/// Deliberately shallow: the test needs to tell a backticked item from ordinary
/// prose, not to parse Rust. A name that is *introduced* somewhere in the crate
/// is an item a citation can be wrong about; anything else is a word.
fn defined_name(line: &str) -> Option<&str> {
    const KEYWORDS: [&str; 9] = [
        "fn", "struct", "enum", "trait", "type", "union", "mod", "static", "const",
    ];
    let mut previous = "";
    for token in line.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if token.is_empty() {
            continue;
        }
        if KEYWORDS.contains(&previous) && token.len() > 3 {
            return Some(token);
        }
        previous = token;
    }
    None
}

/// A `path` or `path:line` citation split into its parts.
fn split_citation(span: &str) -> (&str, Option<&str>) {
    match span.rsplit_once(':') {
        Some((path, line))
            if (path.contains('.') || repository_path(path))
                && !line.is_empty()
                && line.chars().all(|c| c.is_ascii_digit() || c == '-') =>
        {
            (path, Some(line))
        }
        _ => (span, None),
    }
}

/// Whether a span carries a line forward from the file named before it.
fn carried_line(span: &str) -> bool {
    span.len() > 1
        && span.starts_with(':')
        && span[1..].chars().any(|c| c.is_ascii_digit())
        && span[1..].chars().all(|c| c.is_ascii_digit() || c == '-')
}

/// Whether a span names a file this repository ships, or one it is meant to.
fn repository_path(span: &str) -> bool {
    const PREFIXES: [&str; 7] = [
        "src/",
        "tests/",
        "docs/",
        "skill/",
        "examples/",
        "scripts/",
        "assets/",
    ];
    const ROOT_FILES: [&str; 8] = [
        "AGENTS.md",
        "CLAUDE.md",
        "README.md",
        "CHANGELOG.md",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "LICENSE",
    ];
    PREFIXES.iter().any(|prefix| span.starts_with(prefix)) || ROOT_FILES.contains(&span)
}

/// The repository file a citation names, if it names one.
///
/// A citation may name a path in full, a path that is a suffix of one
/// (`harness/mcp/mod.rs`), or a bare file name (`claim.rs`). A name that matches
/// no file — an illustrative `foo.rs`, or a path into another project — is not a
/// repository file and is left alone. An ambiguous suffix is left alone too,
/// because it names no one file.
fn resolved_path(span: &str, root: &std::path::Path, crate_files: &[String]) -> Option<String> {
    if std::path::Path::new(span).has_root() {
        return None;
    }
    if root.join(span).exists() {
        return Some(span.to_string());
    }
    let suffix = format!("/{span}");
    let mut found: Option<&String> = None;
    for file in crate_files {
        if file == span || file.ends_with(&suffix) {
            if found.is_some() {
                return None;
            }
            found = Some(file);
        }
    }
    found.cloned()
}

/// The defect one backticked span carries, if it carries one.
fn span_defect(span: &str, root: &std::path::Path, crate_files: &[String]) -> Option<String> {
    if carried_line(span) {
        return Some(format!(
            "cites `{span}`, a line carried forward from the file before it"
        ));
    }
    let (path, line) = split_citation(span);
    if path.contains('*') {
        return None;
    }
    if resolved_path(path, root, crate_files).is_some() {
        if line.is_some() {
            return Some(format!("cites `{span}`, a line pin on a repository file"));
        }
        return None;
    }
    if line.is_some() && repository_path(path) {
        return Some(format!(
            "cites `{span}`, a repository file that is not there"
        ));
    }
    None
}

/// The items a line binds to repository files that do not hold them.
///
/// The item a citation binds to its file: `` `item` (`path`) `` and
/// `` `path`'s `item` ``. That binding is what a line number used to stand in
/// for, so it is the half a `path` citation cannot be checked without.
fn item_binding_defects(
    raw: &str,
    spans: &[(usize, &str)],
    root: &std::path::Path,
    crate_files: &[String],
    items: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut defects = Vec::new();
    for (position, (start, span)) in spans.iter().enumerate() {
        let (path, _) = split_citation(span);
        let Some(resolved) = resolved_path(path, root, crate_files) else {
            continue;
        };
        let mut items_here: Vec<&str> = Vec::new();
        if position > 0 {
            let (before_start, before) = spans[position - 1];
            let gap = &raw[before_start + before.len() + 2..*start];
            if gap.trim() == "(" && items.contains(before) {
                items_here.push(before);
            }
        }
        if position + 1 < spans.len() {
            let (after_start, after) = spans[position + 1];
            let gap = &raw[*start + span.len() + 2..after_start];
            if gap.trim() == "'s" && items.contains(after) {
                items_here.push(after);
            }
        }
        let held = std::fs::read_to_string(root.join(&resolved)).unwrap_or_default();
        let held = if resolved.ends_with(".rs") {
            estigia::source_text::code_of(&held)
        } else {
            held
        };
        for item in items_here {
            if !held.contains(item) {
                defects.push(format!(
                    "cites `{item}` beside `{resolved}`, which does not hold it"
                ));
            }
        }
    }
    defects
}

/// A coordinate to a missing repository file is caught; the same path bare is not.
#[test]
fn a_coordinate_to_a_repository_file_that_is_not_there_is_a_defect() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_files: Vec<String> = Vec::new();
    let missing = "src/this_sample_file_is_not_on_disk.rs";
    assert!(
        span_defect(&format!("{missing}:12"), &root, &crate_files)
            .is_some_and(|defect| defect.contains("a repository file that is not there")),
        "a `path:line` aimed at a repository file the tree does not hold went unreported"
    );
    assert_eq!(
        span_defect(missing, &root, &crate_files),
        None,
        "a bare path is prose and must be left alone, whatever the tree holds"
    );
}

/// An absolute path is never a repository file, so a device it names is never opened.
///
/// `Path::join` with an absolute path discards the root, and reading
/// `/dev/stdout` on Linux blocks on the test harness's own pipe for good.
#[test]
fn an_absolute_path_is_not_a_repository_file() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_files: Vec<String> = Vec::new();
    for absolute in ["/dev/null", "/dev/stdout", "/etc"] {
        assert_eq!(
            resolved_path(absolute, &root, &crate_files),
            None,
            "{absolute} resolved as a repository file"
        );
    }
}

/// A line pin on a repository file is caught; the same file without a coordinate is not.
#[test]
fn a_line_pin_on_a_repository_file_is_a_defect() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_files: Vec<String> = Vec::new();
    let file = "src/lib.rs";
    assert!(
        span_defect(&format!("{file}:12"), &root, &crate_files)
            .is_some_and(|defect| defect.contains("a line pin on a repository file")),
        "a `path:line` naming a repository file went unreported"
    );
    assert_eq!(
        span_defect(file, &root, &crate_files),
        None,
        "a bare path is prose and must be left alone, whatever the tree holds"
    );
}

/// A line carried forward from the file before it is caught; a colon-led word is not.
#[test]
fn a_line_carried_forward_from_the_file_before_it_is_a_defect() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_files: Vec<String> = Vec::new();
    assert!(
        span_defect(":12", &root, &crate_files)
            .is_some_and(|defect| defect.contains("a line carried forward from the file before it")),
        "a `:NNNN` carried forward from the file before it went unreported"
    );
    assert_eq!(
        span_defect(":notes", &root, &crate_files),
        None,
        "a colon-led span that carries no line is prose and must be left alone"
    );
}

/// An item bound to a repository file that does not hold it is caught; one it holds is not.
#[test]
fn an_item_bound_to_a_repository_file_that_does_not_hold_it_is_a_defect() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_files: Vec<String> = Vec::new();
    let mut items: std::collections::HashSet<String> = std::collections::HashSet::new();
    let item = "a_sample_item_no_file_here_names";
    items.insert(item.to_string());
    let missing = format!("`{item}` (`src/lib.rs`)");
    let spans = backticked(&missing);
    assert!(
        item_binding_defects(&missing, &spans, &root, &crate_files, &items)
            .iter()
            .any(|defect| defect.contains("which does not hold it")),
        "an item bound to a file that does not hold it went unreported"
    );
    items.insert("source_text".to_string());
    let present = "`source_text` (`src/lib.rs`)";
    let spans = backticked(present);
    assert!(
        item_binding_defects(present, &spans, &root, &crate_files, &items).is_empty(),
        "an item bound to a file that holds it must be left alone"
    );
}

/// The documentation cites a file and the item inside it, never a line.
///
/// A line pin rots on every base integration and nothing crossed it before this
/// test. The rule, and what this crossing cannot see, are in `docs/honesty.md`.
/// A citation into a project this repository does not contain is not resolved.
#[test]
fn every_repository_citation_in_the_documentation_holds_up() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Every name this shallow reader can see the crate introduce, so a
    // backticked identifier can be told from a word without guessing.
    let mut items: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut crate_files: Vec<String> = Vec::new();
    for tree in ["src", "tests"] {
        let mut pending = vec![root.join(tree)];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|kind| kind == "rs") {
                    if let Ok(relative) = path.strip_prefix(&root) {
                        crate_files.push(relative.display().to_string());
                    }
                    let source = std::fs::read_to_string(&path).unwrap_or_default();
                    let text = estigia::source_text::code_of(&source);
                    for line in text.lines() {
                        if let Some(name) = defined_name(line) {
                            items.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }
    assert!(
        items.len() > 100,
        "only {} items were read out of the crate, so this proves little about a citation",
        items.len()
    );

    let mut documents: Vec<PathBuf> = vec![root.join("README.md")];
    let mut pending = vec![root.join("docs")];
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
        let relative = document
            .strip_prefix(&root)
            .unwrap_or(document)
            .display()
            .to_string();
        let text = std::fs::read_to_string(document).expect("a readable document");
        let mut fence = false;
        for (index, raw) in text.lines().enumerate() {
            if raw.trim_start().starts_with("```") {
                fence = !fence;
                continue;
            }
            if fence {
                continue;
            }
            let spans = backticked(raw);
            for (_, span) in &spans {
                let (path, _) = split_citation(span);
                if !path.contains('*') && resolved_path(path, &root, &crate_files).is_some() {
                    checked += 1;
                }
                if let Some(defect) = span_defect(span, &root, &crate_files) {
                    broken.push(format!("{relative}:{} {defect}", index + 1));
                }
            }
            for defect in item_binding_defects(raw, &spans, &root, &crate_files, &items) {
                broken.push(format!("{relative}:{} {defect}", index + 1));
            }
        }
    }

    // A floor: this goes quiet by reading nothing, which is what a walk that
    // stopped at the first directory looks like from the outside.
    assert!(
        checked >= 100,
        "only {checked} citations were read, so this proves little about the rest"
    );
    assert!(
        broken.is_empty(),
        "the documentation cites lines and items that do not hold up: {broken:#?}"
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
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("transport")
            .join("branch.rs"),
    )
    .expect("the port's branch module reads");
    // Its own module comment describes the difference, so the search looks
    // for the call rather than for the words.
    let stripped = estigia::source_text::code_of(&text);
    let ported = stripped.lines().any(|line| line.contains("\"develop\""));

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
fn every_row_the_shipped_config_block_names_is_a_real_setting_label() {
    // The reverse of the test above: not whether prose outside the block
    // accidentally spells a setting, but whether every row *inside* it names
    // one for real. `table_rows` is the same parser the transport reads the
    // block with, so this walks the identical corpus rather than a second
    // one written for the test.
    //
    // Canonical labels only, not `Setting::from_label`'s alias-tolerant
    // lookup: an alias exists so an operator's own already-written file keeps
    // meaning what it meant, not so the versioned template may ship one. The
    // shipped block naming a deprecated alias is exactly the drift this
    // guards — `Setting::label()` is what a fresh install is supposed to see.
    let contract =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/SKILL.md"))
            .expect("the shipped contract is readable");
    let rows = estigia::config::table_rows(&contract);
    assert!(
        rows.len() > 5,
        "the walk stopped finding rows in the shipped config block: {} rows",
        rows.len()
    );
    let canonical: Vec<&str> = SETTINGS.iter().map(|setting| setting.label()).collect();
    let unknown: Vec<&String> = rows
        .iter()
        .map(|(label, _)| label)
        .filter(|label| !canonical.contains(&label.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "the shipped config block names a row no `Setting::label()` spells: {unknown:?}"
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

/// Every setting is acted on by something, and the something is named.
///
/// An operator sets twenty-two rows. `config list` reports them, the contract
/// carries them, and `doctor` has a row for the ones a *tracker* declares
/// nothing for. Nothing asked the plainer question: does anything at all read
/// this row.
///
/// Measured, and five did not. `context.get` is called for the labels
/// `READ_BY_THE_TRANSPORT` names — `worktree location`, `Review budget` and
/// `Git operation window` — the gate read `Irreversible commands`, and the
/// payload's prose named the rest of the acted-on rows. That left `Delivery
/// authorisation`, `Transition authorisation`, `Delivery route`, `Merge
/// strategy`, `Review delegation` and `Model routing` read by nobody: not the
/// gate, not the transport, and no sentence an agent is given. `Model routing`
/// left that list when `setup` began writing the model and the effort it names
/// into the definitions it installs, and gave the contract a sentence naming
/// what the row obliges. `Merge strategy` left when `closing_merge` read it.
/// The four rows that had one answer each — `Delivery
/// route`, `Review delegation`, `Merge strategy` and `Review protocol` — were
/// then retired, so they are not settings to read any more, and `READ_BY_NOBODY` below is empty.
///
/// `setup::Applies::Asked` says of some of them *"the contract asks, and the
/// agent may still honour it, but nothing checks"* — and for some the contract
/// did not ask either. A sentence that describes a mechanism is not the
/// mechanism.
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
                let text = estigia::source_text::code_of(&text);
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
                // The gate reads these two rows as fields on `GateContext` rather
                // than through `context.get`, so the field is the read.
                if shipped.contains("context.integration") {
                    reads.push("integration mode".to_owned());
                }
                if shipped.contains("context.issue_requirement") {
                    reads.push("unclaimed write policy".to_owned());
                }
            }
        }
    }

    // Rows nothing reads today. Shorter only, never longer: a new setting nothing
    // acts on fails. `Delivery preset` stores nothing and only writes the rows
    // it stands for, so there is nothing at run time for anything to read.
    const READ_BY_NOBODY: &[&str] = &["Delivery preset"];

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
/// `Tracker::transport` answers it. The production call sites ask it rather
/// than deciding for themselves: the `doctor` command in `src/cli/mod.rs`,
/// `stale_run_pointers` in `src/harness/doctor.rs` (its own skip — the doctor
/// `Check` row this test's doc once named was deleted in the same change that
/// removed Linear and Trello, for the reason `docs/honesty.md` gives), the
/// tool server's `run_tool` in `src/harness/mcp/mod.rs`, and the gate's
/// `decide_observing`, `tracker_answer_for_pointer`, `queue_read` and
/// `completed_work` in `src/harness/mod.rs`.
///
/// This test used to hold a second half: a text search for `["linear",
/// "trello"]` written out as a literal pair, the shape of a second
/// implementation of "is this tracker operable" that would drift from
/// `transport()` silently, since nothing at runtime compares the two. With
/// `Tracker` down to the one variant `transport()` always answers, those two
/// names are gone from every `.rs` file under `src/` — measured directly,
/// grepping for them returns nothing — so that half could never fail for any
/// edit anyone could make: it was inert and unnamed as such, which is the
/// failure `docs/honesty.md`'s own entry on this change exists to name. It is
/// deleted here rather than re-aimed at a shape that does not exist yet: a
/// generic detector for "some future second tracker decided by a hand-rolled
/// match instead of `transport()`" has no real violation to prove itself
/// against today, and a guard that cannot be shown red is not measured, only
/// asserted. `docs/honesty.md` records the gap this leaves: a tracker added
/// later with no transport could again be decided by a second, drifting
/// implementation, and nothing here would catch it until this test is
/// rewritten against that tracker's actual shape.
///
/// What is left is the floor alone: the function really is the one every
/// production caller asks.
#[test]
fn every_reader_of_a_transportless_tracker_asks_the_same_function() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut asked = 0;
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
            let text = estigia::source_text::code_of(&text);
            let shipped = text
                .find("#[cfg(test)]\nmod tests {")
                .map_or(text.as_str(), |end| &text[..end]);
            asked += shipped.matches(".transport()").count();
        }
    }
    assert!(
        asked >= 4,
        "only {asked} callers ask `transport()`, so this is not watching the rule it claims to"
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
            let text = estigia::source_text::code_of(&text);
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
                    .filter(|item| !item.is_empty())
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
            let text = estigia::source_text::code_of(&text);
            let shipped = text
                .find("#[cfg(test)]\nmod tests {")
                .map_or(text.as_str(), |end| &text[..end]);
            for (number, line) in shipped.lines().enumerate() {
                // The code, not the prose: these prefixes are named in comments
                // on purpose, which is how the rule stays readable — and
                // `code_of` has already taken every comment out of `text`.
                for prefix in ["\"status:\"", "\"issue-flow:\""] {
                    if line.contains(prefix) {
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

/// Every `context.get("...")` and `context.get_written("...")` call in `text`,
/// tolerating the whitespace `rustfmt` inserts when the receiver and the call
/// split across lines.
/// `context.get("worktree location")` and `context\n    .get("review
/// budget")` name the same call; a literal search for `"context.get(\""`
/// finds only the first shape, and a guard that only matched it passed while
/// declaring "three, and only three" over a crate that already called
/// `context.get` five times. The same guard matches `get_written` on its own
/// shape rather than letting a renamed reader leave the list.
fn context_get_labels(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut labels = Vec::new();
    let mut search_from = 0;
    while let Some(offset) = text[search_from..].find("context") {
        let at = search_from + offset;
        let word_boundary_before =
            at == 0 || !matches!(bytes[at - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_');
        let after = text[at + "context".len()..].trim_start();
        if word_boundary_before
            && let Some(literal) = after
                .strip_prefix(".get_written(\"")
                .or_else(|| after.strip_prefix(".get(\""))
            && let Some(end) = literal.find('"')
        {
            labels.push(literal[..end].to_lowercase());
        }
        search_from = at + "context".len();
    }
    labels
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
                let text = estigia::source_text::code_of(&text);
                // The shipped half only: a fixture building a `Context` is not
                // the transport consulting the operator's table.
                let shipped = text
                    .find("#[cfg(test)]\nmod tests {")
                    .map_or(text.as_str(), |end| &text[..end]);
                read.extend(context_get_labels(shipped));
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

#[test]
fn the_incomplete_verdict_row_names_both_ways_out_of_it() {
    let binding = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/bindings/github/refusals.md"),
    )
    .expect("the refusals companion ships");
    let row = binding
        .lines()
        .find(|line| line.starts_with("| `computed-verdict-incomplete` |"))
        .expect("the binding names the refusal");
    let way_out = row.split('|').nth(3).expect("the row has an action cell");
    for needle in [
        "`review_lens_report`",
        "`review_validator_result`",
        "`review-validator`",
    ] {
        assert!(
            way_out.contains(needle),
            "the way out does not name {needle}, so an agent is sent the wrong way"
        );
    }
}

#[test]
fn opencode_is_not_told_to_handoff_because_review_blind_is_claude_only() {
    // Measured: those two sentences, read together, made every
    // OpenCode publication call handoff_review. The reserved type is Claude's
    // launch mechanism, not the capability. Restoring the old pair must go red.
    let notes = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/references/runtime-notes.md"),
    )
    .expect("runtime-notes ships");

    assert!(
        !notes.contains("When the capability is absent, use the binding's durable review handoff"),
        "runtime-notes again names handoff as the answer to an absent capability, \
         which is how a missing Claude type became an ordinary OpenCode stop"
    );
    assert!(
        notes.contains("cover OpenCode launches"),
        "the Claude-only bound was deleted rather than followed by the launch path"
    );
    assert!(
        notes.contains("Absence of that reserved type is not an absent capability"),
        "OpenCode is no longer told what to do instead of handoff"
    );
    let notes_compact: String = notes.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        notes_compact.contains("Task")
            && notes_compact.contains("onlywhenthatlaunchfailed")
            && notes_compact.contains("after`publish_review`")
            && notes_compact.contains("withoutlaunchingfirstisadefectagainstthissentence"),
        "the OpenCode post-publish launch or the handoff-first defect bound is gone: {notes}"
    );
    assert!(
        notes_compact.contains("arunacquiresonewhenitsruntimecanprovideone,anddoessowithoutasking"),
        "runtime notes no longer says the runtime is the capability"
    );
    assert!(
        notes_compact.contains("Whenthehostcannotspawnahelperatall,oreverylaunchofthepanelfailed"),
        "runtime notes no longer bounds handoff to a failed launch"
    );
    // The rendered OpenCode section carries the same two sentences.
    let opencode = estigia::setup::opencode_review_section();
    let opencode_compact: String = opencode.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        opencode_compact.contains("After`publish_review`")
            && opencode_compact.contains("withoutlaunchingfirstisadefectagainstthissentence"),
        "opencode_review_section no longer states the acceptance sentences: {opencode}"
    );
}

#[test]
fn the_filing_step_still_names_the_route_it_hands_off_to() {
    // Nothing pinned step 8, so deleting it left every binary green
    // -- the change's user-visible payload was unguarded. This asserts the
    // step's own bytes rather than a phrase a rewrite would survive: the line is
    // compared whole, and a second candidate is a failure rather than a silent
    // pick, the shape the step 6 pin above uses.
    const STEP_EIGHT: &str = "8. A defect, risk or improvement outside this issue's scope is handed to an analyst run — a separate context holding its own claim — and never filed by this run. The runtime obtains that second context: a subagent, a teammate or a separate session all qualify, as [runtime notes](references/runtime-notes.md) states. Hand over what is held: the reproduction, the file and line, the command and its output; or say plainly that it could not be reduced to a reproduction, rather than filing an impression. That run applies the analyst contract this file states, including its bound of at most one evidenced issue or none, and it is the run that files. A delegated `analyst` worker definition, where the host installs one, can carry the reading, but it is repository-read-only and files nothing. Do not block delivery on the answer; record the handoff and its outcome in the close evidence when the answer has come back, or say the handoff is still open, so a later run can see that this one looked and what came of it.";

    let contract =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/SKILL.md"))
            .expect("the contract ships");

    let matches: Vec<&str> = contract
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("8. ") && trimmed.contains("handed to an analyst run")
        })
        .collect();

    let step_eight = match matches.as_slice() {
        [] => panic!(
            "SKILL.md no longer has a Developer step 8 line (starts with `8. ` and names \
             `handed to an analyst run`)"
        ),
        [only] => *only,
        several => panic!(
            "{} lines in SKILL.md start with `8. ` and carry `handed to an analyst run` -- a \
             second line matched this pin and the guard can no longer tell which one is the \
             Developer step: {several:?}",
            several.len()
        ),
    };

    assert_eq!(
        step_eight, STEP_EIGHT,
        "SKILL.md step 8 no longer carries its own bytes; if the change is deliberate, update \
         STEP_EIGHT in tests/honesty.rs so the pin keeps asserting the step rather than a \
         phrase a rewrite could survive"
    );
}

/// Every `*Section Name* in <path>.md` reference a stop payload in `src/`
/// writes resolves to a file this crate ships and a heading that file actually
/// has.
///
/// The split of `skill/bindings/github.md` into routed companions moved four
/// sections out from under two runtime refusals in `src/transport/branch.rs`
/// (`worktree-ownership-unproven`, `branch-locked-by-another-run`) without
/// touching the string naming where to read the recovery -- an operator
/// reading `action` was sent to a document that no longer had that heading.
/// Nothing caught it: the split verified the documents against each other,
/// and this is the first check that verifies a reference *into* them from
/// code.
///
/// Scoped to `code_of`-stripped text -- payload strings, not doc comments.
/// This repository's rule is that no test may read comment text (AGENTS.md,
/// `src/source_text.rs`), and the two `board.rs` doc comments this issue also
/// repairs are prose describing the binding rather than an operator-facing
/// refusal; a comment naming a stale file is a finding for a human reading the
/// source, not a runtime answer an operator acts on blind. The pattern this
/// guard closes is the one with a blast radius nobody reads before it fires.
#[test]
fn every_section_reference_a_stop_names_resolves_in_the_shipped_skill() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![root.join("src")];
    let mut sources: Vec<PathBuf> = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    assert!(
        sources.len() > 20,
        "only {} .rs files were found under src/, so this walk stopped early",
        sources.len()
    );

    let mut checked = 0;
    let mut broken = Vec::new();
    for path in &sources {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()))
            .replace("\r\n", "\n");
        let code = estigia::source_text::code_of(&raw);
        for (section, target) in asterisk_section_references(&code) {
            checked += 1;
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string();
            let Some(file) = estigia::skill::FILES.iter().find(|f| f.path == target) else {
                broken.push(format!(
                    "{relative} names *{section}* in {target}, which is not a path in \
                     estigia::skill::FILES"
                ));
                continue;
            };
            let has_heading = file
                .contents
                .lines()
                .any(|line| line.trim_start().starts_with('#') && line.contains(&section));
            if !has_heading {
                broken.push(format!(
                    "{relative} names *{section}* in {target}, but {target} has no heading \
                     containing that text"
                ));
            }
        }
    }

    assert!(
        checked >= 2,
        "only {checked} `*Section* in <path>.md` references were found in src/, so this guard \
         proves little about the rest"
    );
    assert!(
        broken.is_empty(),
        "a stop payload or a code string names a section that moved: {broken:#?}"
    );
}

/// The "Measured at this HEAD" table in `docs/honesty.md` matches the files it
/// names, including the rendered installed contract and the reading-grant
/// reviewer body.
///
/// That paragraph was hand-edited three times in one day and wrong
/// every time. Nothing crossed the figures against the files. This does.
#[test]
fn the_measured_judge_context_paragraph_matches_the_files_it_names() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let honesty = honesty().replace("\r\n", "\n");
    let marker = "Measured at this HEAD";
    let start = honesty
        .find(marker)
        .unwrap_or_else(|| panic!("docs/honesty.md must still carry `{marker}`"));
    let rest = &honesty[start..];
    let end = rest.find("\n- **").unwrap_or(rest.len());
    let section = &rest[..end];

    let lf_bytes = |relative: &str| -> usize {
        let text = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        text.len()
    };

    let named = |path: &str| -> usize {
        // Table row: `| `path` | 12,345 | ...`
        let needle = format!("`{path}`");
        let at = section
            .find(&needle)
            .unwrap_or_else(|| panic!("measured paragraph must name `{path}`"));
        let after = &section[at + needle.len()..];
        let pipe = after
            .find('|')
            .unwrap_or_else(|| panic!("no size cell after `{path}`"));
        let cell = after[pipe + 1..]
            .split('|')
            .next()
            .unwrap_or_default()
            .trim()
            .replace(',', "");
        cell.parse::<usize>()
            .unwrap_or_else(|_| panic!("size cell for `{path}` is not an integer: {cell:?}"))
    };

    assert_eq!(
        named("skill/bindings/github.md"),
        lf_bytes("skill/bindings/github.md"),
        "github.md byte figure drifted"
    );
    assert_eq!(
        named("skill/policies/blind-judges.md"),
        lf_bytes("skill/policies/blind-judges.md"),
        "blind-judges.md byte figure drifted"
    );
    assert_eq!(
        named("skill/policies/judgment-day.md"),
        lf_bytes("skill/policies/judgment-day.md"),
        "judgment-day.md byte figure drifted"
    );
    assert_eq!(
        named("skill/bindings/github/delivery.md"),
        lf_bytes("skill/bindings/github/delivery.md"),
        "delivery.md byte figure drifted"
    );

    let skill_file = estigia::skill::FILES
        .iter()
        .find(|file| file.path == estigia::skill::CONTRACT)
        .expect("SKILL.md is a shipped skill file");
    let installed = estigia::skill::rendered(skill_file, &estigia::config::Config::default());
    let installed_bytes = installed.replace("\r\n", "\n").replace('\r', "\n").len();
    // The table marks installed SKILL as rendered rather than quoting a hand
    // figure; still prove rendering is available and larger than the shipped
    // block (the configuration fence grows it).
    let shipped = lf_bytes("skill/SKILL.md");
    assert!(
        installed_bytes > shipped,
        "installed SKILL.md ({installed_bytes}) must exceed the shipped file ({shipped}) once the \
         configuration block is rendered"
    );
    assert!(
        section.contains("skill::rendered") && section.contains("Config::default()"),
        "the paragraph must say how the installed size is derived rather than inventing a figure"
    );

    let reviewer = estigia::setup::render_reviewer_agent(
        estigia::skill::REVIEW_AGENT.contents,
        estigia::config::Evidence::Reading,
        estigia::config::Seal::Unsealed,
        "review-blind",
    );
    let reviewer_bytes = reviewer.replace("\r\n", "\n").replace('\r', "\n").len();
    assert!(
        section.contains("render_reviewer_agent")
            || section.contains("setup::render_reviewer_agent"),
        "the paragraph must name how the review-blind body is measured"
    );
    // Pin the reading-grant body so a silent shrink or growth is review-visible.
    // Not a hand figure in the prose table; a floor the test holds.
    assert!(
        reviewer_bytes > 5_000,
        "rendered review-blind body is unexpectedly small: {reviewer_bytes}"
    );
    let _ = reviewer_bytes;
}

/// The `*Section Name* in <path>.md` occurrences in `text`, as
/// `(section, path)` pairs.
///
/// Looks for the closing `* in ` first and walks backward to the matching
/// opening `*`, then reads forward from `in ` for a bare path ending in
/// `.md` -- matching the exact shape the two runtime refusals in
/// `src/transport/branch.rs` write, not markdown's `[text](target)` link
/// syntax, which `every_link_in_this_repositorys_documentation_resolves`
/// already covers for the documents themselves.
fn asterisk_section_references(text: &str) -> Vec<(String, String)> {
    let marker = "* in ";
    let mut found = Vec::new();
    let mut search_from = 0;
    while let Some(offset) = text[search_from..].find(marker) {
        let close_star = search_from + offset;
        search_from = close_star + marker.len();
        let Some(open_star) = text[..close_star].rfind('*') else {
            continue;
        };
        let name = &text[open_star + 1..close_star];
        if name.is_empty() || name.contains('\n') {
            continue;
        }
        let rest = &text[search_from..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '\\' | ':' | ',' | ')' | '"'))
            .unwrap_or(rest.len());
        let candidate = &rest[..end];
        if candidate.ends_with(".md") {
            found.push((name.to_owned(), candidate.to_owned()));
        }
    }
    found
}

/// A candidate the kebab-case shape below matches that is not a refusal
/// code, with the reason it is not — kept minimal and checked for staleness
/// the same way `UNDOCUMENTED_REFUSALS` already holds the reverse crossing to
/// its list: an entry that stops appearing in `skill/` has to come off.
///
/// Each reason names the *actual* symbol family the candidate belongs to
/// instead of asserting it is nothing: a workflow state, a setting value, an
/// agent name and a `SCRIPT` operation verb are all extracted directly out of
/// `skill/` or the crate below and so never reach this table at all; what is
/// left after that subtraction is genuinely small.
const NOT_A_REFUSAL_CODE: &[(&str, &str)] = &[
    (
        "a-b",
        "a fictional identifier used as an example, not a real one",
    ),
    ("a-z0-9", "a regex character-class fragment, not a symbol"),
    (
        "acted-on",
        "a disposition vocabulary value (`DISPOSITION_WORDS`), a distinct symbol family this \
         change does not take",
    ),
    (
        "base-only",
        "a `--causal-disposition` vocabulary value (`FINDING_CAUSAL_DISPOSITIONS`), a distinct \
         symbol family this change does not take",
    ),
    (
        "blocked-until",
        "a control marker kind (`markers::parse`), a symbol family beside the refusal codes rather \
         than one of them",
    ),
    (
        "budget-candidate-tree",
        "a correction-budget field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "budget-cumulative",
        "a correction-budget field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "budget-genesis-paths",
        "a correction-budget field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "budget-genesis-tree",
        "a correction-budget field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "budget-original-lines",
        "a correction-budget field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "budget-refrozen",
        "a correction-budget field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "claude-3edc7a52",
        "a fictional run-id used as an example of the shape the gate admits",
    ),
    (
        "claude-code-cb8d3f2c",
        "a fictional run-id used to illustrate a race",
    ),
    (
        "claude-code-d7d8a22e",
        "a fictional run-id used to illustrate a race",
    ),
    (
        "codex-01k4",
        "the wrong runtime a dashed suffix projects, used as an example in the refusal it documents",
    ),
    (
        "codex-01k4-9f2a",
        "a fictional run-id whose suffix carries a dash, the example the gate refuses",
    ),
    ("codex-b91c4d2e", "a fictional run-id used as an example"),
    (
        "comment-edited",
        "a `ReclaimDiscard::as_str` condition a refusal carries, a distinct symbol family from the \
         refusal codes",
    ),
    (
        "comment-untrusted",
        "a `ReclaimDiscard::as_str` condition a refusal carries, a distinct symbol family from the \
         refusal codes",
    ),
    (
        "delta-range",
        "a publication lineage field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "delta-range-reason",
        "a publication lineage field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "evidence-unproven",
        "a `ReclaimDiscard::as_str` condition a refusal carries, a distinct symbol family from the \
         refusal codes",
    ),
    (
        "fix-1-a",
        "a fictional branch name used to show the fold's collision",
    ),
    ("fix-6", "a fictional branch name used to show the `~` join"),
    (
        "fix-6-a",
        "a fictional branch name used to show the `~` join",
    ),
    ("fix-6-a-b", "a fictional identifier used as an example"),
    (
        "for-each-ref",
        "a git subcommand, not a symbol this crate owns",
    ),
    (
        "hash-object",
        "a git subcommand, not a symbol this crate owns",
    ),
    (
        "lane-count",
        "a publication-lane width field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "lane-note",
        "a publication-lane detail field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "legacy-takeover",
        "a `ReclaimDiscard::as_str` condition the fold decides, carried by no refusal — a distinct \
         symbol family from the refusal codes",
    ),
    (
        "line-past-end-of-file",
        "a `finding-citation-unresolvable` `cause` value, not a refusal code on its own",
    ),
    ("ls-tree", "a git subcommand, not a symbol this crate owns"),
    (
        "malformed-path",
        "a `finding-citation-unresolvable` `cause` value, not a refusal code on its own",
    ),
    (
        "marker-malformed",
        "a `ReclaimDiscard::as_str` condition a refusal carries, a distinct symbol family from the \
         refusal codes",
    ),
    (
        "never-ran",
        "an answer value the read-only `publication_lane` operation reports, not a refusal code",
    ),
    (
        "not-consulted",
        "an answer value the read-only `publication_lane` operation reports, not a refusal code",
    ),
    (
        "parent-base",
        "a JSON answer field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "parent-digest",
        "a JSON answer field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "parent-head",
        "a JSON answer field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "parent-pr",
        "a JSON answer field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "path-is-not-a-file",
        "a `finding-citation-unresolvable` `cause` value, not a refusal code on its own",
    ),
    (
        "path-not-in-either-tree",
        "a `finding-citation-unresolvable` `cause` value, not a refusal code on its own",
    ),
    (
        "payload-digest",
        "a JSON answer field on `publish_review`'s marker \u{2014} the deferred 'tool names' class",
    ),
    (
        "pre-existing",
        "a `--causal-disposition` vocabulary value (`FINDING_CAUSAL_DISPOSITIONS`), a distinct \
         symbol family this change does not take",
    ),
    (
        "pre-push",
        "the name of the git hook Estigia installs, not a refusal code",
    ),
    (
        "refutation-batch",
        "the name of the `review_status` input `capture_review_input` accepts, not a refusal code",
    ),
    (
        "reserved-elsewhere",
        "a `ReclaimDiscard::as_str` condition a refusal carries, a distinct symbol family from the \
         refusal codes",
    ),
    (
        "review-blind-sealed",
        "a reserved reviewer role, not a refusal code",
    ),
    (
        "review-handoff",
        "the `handoff-review` operation named in the other word order, not a refusal code",
    ),
    ("run-a", "a fictional run-id used as an example"),
    ("run-b", "a fictional run-id used as an example"),
    (
        "stacked-to-main",
        "a delivery-strategy name taken from the delivery reference, a vocabulary value this crate \
         does not answer with",
    ),
    (
        "target-run-mismatch",
        "a `ReclaimDiscard::as_str` condition a refusal carries, a distinct symbol family from the \
         refusal codes",
    ),
];

/// Whether `code` exists in `src/transport` only as a `format!` prefix —
/// `"{code}:{}..."` — rather than as a standalone literal.
///
/// `closed-by-own-delivery` is the one measured case: its prefix is the const
/// `OWN_DELIVERY_CLOSE_PREFIX` at `src/transport/claim.rs`, and `verify_claim`
/// writes it as `format!("{OWN_DELIVERY_CLOSE_PREFIX}{}:{}", ...)`, so it is
/// real and `TRANSPORT_VOCABULARY` — which only extracts whole literals — does
/// not carry it. Matching the code side for a colon-suffixed prefix generalises
/// past this one instance rather than naming it by hand.
///
/// Reads only the shipped part, the same cut `build.rs` takes, so an inline
/// test module cannot supply a prefix the transport does not ship.
fn code_exists_as_a_format_prefix(code: &str) -> bool {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/transport");
    let prefix = format!("\"{code}:");
    let mut stack = vec![root];
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
            if path.file_name().is_some_and(|name| name == "tests.rs")
                || !path.extension().is_some_and(|kind| kind == "rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let shipped = estigia::source_text::code_of(&text);
            if estigia::source_text::shipped_part(&shipped).contains(&prefix) {
                return true;
            }
        }
    }
    false
}

#[test]
fn every_refusal_code_skill_names_is_one_the_crate_can_answer_with() {
    // The reverse of `every_refusal_the_port_can_give_is_one_somebody_has_been_told_how_to_answer`
    // above: that test asks whether every code the transport can answer with
    // is named in `skill/`; this asks whether every code `skill/` names is
    // one the transport can actually answer with. A code named here and
    // renamed or removed in the transport is an instruction branch no run can
    // ever reach, and the other direction cannot see that at all.
    //
    // The corpus is extracted, never enumerated: every inline code span in
    // every `skill/**/*.md`, kept only where it has the shape a refusal code
    // has — lowercase kebab-case, not opening with a hyphen (which is a CLI
    // flag, not a code). That shape also matches workflow states, setting
    // values, agent names and operation verbs, so all four are subtracted by
    // extracting them from their own source rather than guessed away.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill");
    let mut candidates: Vec<String> = Vec::new();
    let mut read = 0usize;
    let mut stack = vec![root.clone()];
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
            if !path.extension().is_some_and(|kind| kind == "md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            read += 1;
            for span in estigia::markdown::markdown_inline_code_spans(&text) {
                if span.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    && span.contains('-')
                    && span
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                {
                    candidates.push(span);
                }
            }
        }
    }
    assert!(
        read > 20,
        "the walk stopped finding skill/**/*.md: {read} read"
    );
    candidates.sort();
    candidates.dedup();
    assert!(
        candidates.len() > 175,
        "the walk stopped finding backticked kebab-case spans: {} found",
        candidates.len()
    );

    // Workflow states and setting values: extracted from the crate's own
    // closed lists, not retyped.
    let mut known: Vec<String> = estigia::config::STATES
        .iter()
        .map(|s| s.to_string())
        .collect();
    known.extend(SETTINGS.iter().flat_map(|setting| {
        setting
            .answers()
            .choices
            .iter()
            .map(|choice| choice.to_string())
    }));
    // Agent names: extracted from the shipped directory listing.
    let agents = root.join("agents");
    for entry in std::fs::read_dir(&agents)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", agents.display()))
        .flatten()
    {
        if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
            known.push(stem.to_owned());
        }
    }
    // Operation verbs: extracted from the transport's own operation table,
    // not from the corpus this test is crossing.
    known.extend(
        estigia::transport::commands::SCRIPTED
            .iter()
            .map(|operation| (*operation).to_owned()),
    );
    // `doctor`'s own row names: the shipped prose names them (`stale-run-pointer`,
    // `prune`), and a row name is something the crate answers with rather than a
    // refusal code. Extracted from the list the row itself reports under.
    known.extend(
        estigia::harness::doctor::CHECKS
            .iter()
            .map(|name| (*name).to_owned()),
    );

    let unknown: Vec<&String> = candidates
        .iter()
        .filter(|candidate| {
            !known.contains(candidate)
                && !estigia::harness::tracker::TRANSPORT_VOCABULARY.contains(&candidate.as_str())
                && !NOT_A_REFUSAL_CODE
                    .iter()
                    .any(|(name, _)| *name == candidate.as_str())
                && !code_exists_as_a_format_prefix(candidate)
        })
        .collect();
    assert!(
        unknown.is_empty(),
        "skill/ names a code nothing in this crate can answer with: {unknown:?}"
    );

    // The exemption table is not allowed to rot: an entry that no longer
    // names a span in the corpus is one somebody already fixed, and it has
    // to come off the list rather than sit there unread.
    let stale: Vec<&str> = NOT_A_REFUSAL_CODE
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !candidates.iter().any(|candidate| candidate == name))
        .collect();
    assert!(
        stale.is_empty(),
        "these are listed as exemptions and no longer appear in skill/: {stale:?} \u{2014} take them off"
    );
    // And the exemption is checked on the code side too, which the `skill/`-side
    // staleness above cannot see: an entry exempted as "not a refusal code" that
    // the transport now answers with would hide a real code from the crossing.
    let real: Vec<&str> = NOT_A_REFUSAL_CODE
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| {
            estigia::harness::tracker::TRANSPORT_VOCABULARY.contains(name)
                || code_exists_as_a_format_prefix(name)
        })
        .collect();
    assert!(
        real.is_empty(),
        "these are exempted as not refusal codes and the transport answers with them: {real:?}"
    );
}

fn markdown_under(directory: &std::path::Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("a readable directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            markdown_under(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            found.push(path);
        }
    }
}

#[test]
fn no_text_an_agent_executes_tells_a_run_to_record_an_aggregate_verdict() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("README.md"), root.join("docs/configuration.md")];
    markdown_under(&root.join("skill"), &mut files);
    let mut texts: Vec<(String, String)> = files
        .iter()
        .map(|path| {
            (
                path.display().to_string(),
                std::fs::read_to_string(path).expect("a readable document"),
            )
        })
        .collect();
    texts.push((
        "the rendered OpenCode review block".to_owned(),
        estigia::setup::opencode_review_section(),
    ));
    texts.push((
        "the rendered contract".to_owned(),
        estigia::skill::selected_documents(&estigia::config::Config::default()),
    ));
    for tool in estigia::harness::mcp::tools::TOOLS {
        texts.push((
            format!("the {} tool description", tool.name),
            tool.description.to_owned(),
        ));
        for argument in tool.arguments {
            texts.push((
                format!("the {} tool's `{}` argument", tool.name, argument.name),
                argument.description.to_owned(),
            ));
        }
    }
    for (name, text) in &texts {
        let text = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        for retired in [
            "record one aggregate",
            "record an aggregate",
            "records one aggregate",
            "aggregate `review_verdict`",
        ] {
            assert!(
                !text.contains(retired),
                "{name} still tells a run to record an aggregate verdict: {retired:?}"
            );
        }
    }
    let block = estigia::setup::opencode_review_section();
    for required in [
        "`collect_review_input`",
        "`record_computed_verdict`",
        "`release_ci`",
    ] {
        assert!(
            block.contains(required),
            "the OpenCode review block does not name {required}"
        );
    }
}

#[test]
fn no_instruction_promises_the_delta_for_a_merged_base_advance() {
    fn markdown_under(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                markdown_under(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                found.push(path);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    markdown_under(&root.join("skill"), &mut files);
    let mut texts: Vec<(String, String)> = files
        .iter()
        .map(|path| {
            (
                path.display().to_string(),
                std::fs::read_to_string(path).expect("a readable skill file"),
            )
        })
        .collect();
    for source in ["src/transport/claim.rs", "src/harness/mcp/tools.rs"] {
        let code = estigia::source_text::code_of(
            &std::fs::read_to_string(root.join(source)).expect("a readable source"),
        );
        texts.push((source.to_owned(), code.replace("\\\n", " ")));
    }
    for (name, text) in texts {
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for stale in [
            "reads only the delta",
            "keeps the parent head, so",
            "read `parent-head..head` as the delta",
            "panel reads `parent-head..head`",
            "`parent-head..head` delta are what the panel reads",
            "keeps the parent head and reads the narrower range",
        ] {
            assert!(
                !flat.contains(stale),
                "{name} still says a merged base advance is read by the delta: {stale:?}"
            );
        }
    }
}

#[test]
fn the_silence_the_skill_states_is_the_one_ownership_applies() {
    assert_eq!(
        estigia::transport::ownership::SILENCE,
        4 * 60 * 60,
        "the skill says four hours in two places; change them with the constant"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for path in [
        "skill/references/runtime-notes.md",
        "skill/bindings/github.md",
    ] {
        let text = std::fs::read_to_string(root.join(path)).expect("the skill ships");
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("four hours"),
            "{path} does not state the silence window"
        );
    }
    let binding = std::fs::read_to_string(root.join("skill/bindings/github.md"))
        .expect("the github binding ships");
    let binding = binding.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        binding.contains("at its horizon or four hours after trusted activity, whichever is later"),
        "the github binding does not state the window as horizon-or-silence, whichever is later"
    );
    assert!(
        !binding.contains("horizonless acquisitions expire"),
        "the github binding limits the four-hour window to horizonless acquisitions"
    );
    let notes = std::fs::read_to_string(root.join("skill/references/runtime-notes.md"))
        .expect("runtime-notes ships");
    let rule = notes
        .split("\n\n")
        .find(|paragraph| paragraph.starts_with("A claim with a horizon stays live"))
        .expect("runtime-notes states the rule in a paragraph that opens on the claim's horizon");
    let rule = rule.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        rule.contains("four hours"),
        "the rule paragraph does not state the silence window"
    );
    for kind in estigia::transport::markers::ACTIVITY_KINDS {
        assert!(
            rule.contains(&format!("`{kind}`")),
            "the rule paragraph does not name the activity kind `{kind}`"
        );
    }
}
/// Every test the catalogue names is a function this crate still has.
///
/// `docs/testing.md` is the inventory `AGENTS.md` gave up: which test drives which generated
/// integration, and what each one needs on the machine. It is prose, so a renamed or deleted test
/// left it naming something that is gone — and `honesty.md` said only a person following the link
/// would notice. This is that person, automated: the names are read back out of the source with
/// comments stripped, and a name the suite no longer holds fails here.
#[test]
fn every_test_the_catalogue_names_is_still_a_function() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalogue = std::fs::read_to_string(root.join("docs/testing.md"))
        .expect("the catalogue ships with the crate");

    let mut functions: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut read = 0_usize;
    let mut pending = vec![root.join("src"), root.join("tests")];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if !path.extension().is_some_and(|kind| kind == "rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            read += 1;
            for line in estigia::source_text::code_of(&text).lines() {
                let line = line.trim_start();
                let Some(rest) = line.strip_prefix("fn ") else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    functions.insert(name);
                }
            }
        }
    }
    assert!(
        read > 20,
        "the walk read {read} source files, too few to be this crate"
    );

    let mut named: Vec<String> = Vec::new();
    for piece in catalogue.split('`').skip(1).step_by(2) {
        let looks_like_a_test_name = piece.len() >= 12
            && piece.contains('_')
            && piece
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if looks_like_a_test_name {
            named.push(piece.to_owned());
        }
    }
    assert!(
        named.len() > 50,
        "only {} names were read out of the catalogue, so this guard would agree with almost \
         everything",
        named.len()
    );

    let missing: Vec<&String> = named
        .iter()
        .filter(|name| !functions.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/testing.md names tests this crate no longer holds: {missing:?}. Rename the entry with \
         the test, or delete it — a catalogue that names a test somebody removed is a red line \
         nobody can find"
    );
}
