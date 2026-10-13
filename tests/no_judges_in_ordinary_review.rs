//! No text an orchestrating agent acts on for ordinary review acquires judges.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

const JUDGE_WORDS: &[&str] = &["judge", "judges"];

const ACQUIRING_FORMS: &[&str] = &[
    "run",
    "runs",
    "running",
    "ran",
    "spawn",
    "spawns",
    "spawned",
    "spawning",
    "use",
    "uses",
    "used",
    "using",
    "require",
    "requires",
    "required",
    "requiring",
    "need",
    "needs",
    "needed",
    "needing",
    "add",
    "adds",
    "added",
    "adding",
    "dispatch",
    "dispatches",
    "dispatched",
    "dispatching",
    "acquire",
    "acquires",
    "acquired",
    "acquiring",
    "launch",
    "launches",
    "launched",
    "launching",
    "convene",
    "convenes",
    "convened",
    "convening",
    "license",
    "licenses",
    "licensed",
    "licensing",
];

const ALLOWED_SENTENCES: &str = r##"
negates acquisition :: Ordinary review licenses lenses only: the answer carries no judge count at any tier
negates acquisition :: It acquires no judge at any tier: `review_tier` answers no judge count, and the computed verdict `release_ci` requires reads lens reports and the finding ledger alone
negates acquisition :: none of them convenes a judge
negates acquisition :: A lens is not a judge, and no tier acquires a judge
negates acquisition :: Judgment Day's two judges are `review-blind` and `review-blind-sealed`, one template rendered twice, and no tier acquires them
Judgment Day scope :: Judgment Day's two judge definitions are fixed, not configured: one sealed, one unsealed, launched only for Judgment Day
Judgment Day scope :: For Judgment Day the orchestrator remains responsible for passing the effective judge model, launching both judges concurrently, withholding sibling output and preserving dissent
negates spawning; Judgment Day judge directory :: Estigia does not spawn the judge: the orchestrating context creates each directory and is the only context that could remove it
negates acquisition :: Judgment Day's two judges are `review-blind` and `review-blind-sealed`: no tier acquires them, and they launch only when the operator explicitly asks for Judgment Day — see [judgment day](../policies/the-policy-file)
negates acquisition; run is the noun in "that run" :: It files at most one evidenced issue, save for the filing duty [blind judges](policies/the-policy-file) places on the run that records the computed verdict — that run files one issue for every `severe` finding it records `pre-existing`, `base-only` or `unknown`, however many that is
negates acquisition; the judge word is only a link text :: Use the lenses the tier licenses: a severe finding is settled by the ledger at every tier, stated in full in [blind judges](../../policies/the-policy-file)
Judgment Day scope; launch is a noun :: Analysis is cheap, parallel, and repository-read-only — a Judgment Day judge measuring under `Evidence standard: measuring` writes only in the isolated directory its launch hands it, outside the claimed checkout
Judgment Day scope; launch is a noun :: On a repository whose `Evidence standard` is `measuring` a Judgment Day judge also builds, tests and mutates — inside the isolated directory its launch hands it and nowhere else
launch is a noun :: The orchestrator passes `Model routing`'s effective `judge` model on each launch
"##;

fn allowed() -> Vec<(&'static str, &'static str)> {
    ALLOWED_SENTENCES
        .lines()
        .filter_map(|line| line.split_once(" :: "))
        .map(|(reason, sentence)| (sentence, reason))
        .collect()
}

const VERB_USES_OF_JUDGE: &[&str] = &["and judge nothing outside the corrected paths"];

fn tokens(sentence: &str) -> Vec<String> {
    sentence
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn names_an_acquiring_verb(words: &[String]) -> bool {
    words
        .iter()
        .any(|word| ACQUIRING_FORMS.contains(&word.as_str()))
}

fn is_abbreviation(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "e.g" | "i.e" | "etc" | "vs" | "cf"
    )
}

fn sentences_of(unit: &str) -> Vec<String> {
    let characters: Vec<char> = unit.chars().collect();
    let mut sentences = Vec::new();
    let mut current = String::new();
    for (at, &character) in characters.iter().enumerate() {
        let closes = match character {
            ';' | '?' | '!' => true,
            '.' => {
                let followed_by_space = characters
                    .get(at + 1)
                    .is_none_or(|next| next.is_whitespace());
                let word: String = current
                    .chars()
                    .rev()
                    .take_while(|c| !c.is_whitespace())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                followed_by_space && !is_abbreviation(word.trim_matches(['(', '`', '*', '_']))
            }
            _ => false,
        };
        if closes {
            sentences.push(std::mem::take(&mut current));
        } else {
            current.push(character);
        }
    }
    sentences.push(current);
    sentences
        .into_iter()
        .map(|sentence| sentence.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|sentence| !sentence.is_empty())
        .collect()
}

fn units_of(text: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let starts_unit = trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('|')
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.split_once(". ").is_some_and(|(number, _)| {
                !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
            });
        if starts_unit && !current.is_empty() {
            units.push(std::mem::take(&mut current));
        }
        if !trimmed.is_empty() {
            current.push_str(trimmed);
            current.push(' ');
        }
    }
    if !current.is_empty() {
        units.push(current);
    }
    units
}

fn unlisted(text: &str) -> Vec<String> {
    judges_to_acquire(text)
        .into_iter()
        .filter(|sentence| !allowed().iter().any(|(allowed, _)| allowed == sentence))
        .collect()
}

fn judges_to_acquire(text: &str) -> Vec<String> {
    let text = text
        .replace("blind-judges.md", "the-policy-file")
        .replace("judgment-day.md", "the-policy-file")
        .replace("judge_workspaces", "the-workspaces-tool")
        .replace("judge-workspaces", "the-workspaces-tool")
        .replace("run-id", "runid");
    units_of(&text)
        .iter()
        .flat_map(|unit| sentences_of(unit))
        .filter(|sentence| {
            let checked = VERB_USES_OF_JUDGE
                .iter()
                .fold(sentence.to_owned(), |text, phrase| text.replace(phrase, ""));
            let words = tokens(&checked);
            words
                .iter()
                .any(|word| JUDGE_WORDS.contains(&word.as_str()))
                && names_an_acquiring_verb(&words)
        })
        .collect()
}

fn markdown_files_under(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).expect("the directory is readable") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files.extend(markdown_files_under(&path));
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_checker_flags_the_sentence_it_exists_to_refuse() {
    for sentence in [
        "the judges `review_tier` licenses \u{2014} two at `medium`/`high`, sealed and unsealed",
        "`medium` and `high` acquire two judges, one sealed and one unsealed",
        "then launch the judges the tier licenses",
        "Unlike Judgment Day, ordinary review launches two judges",
        "Ordinary review runs two judges beside the lenses",
        "A medium change spawns a sealed judge",
        "Every review uses two judges",
        "A high tier requires two judges to agree",
        "The orchestrator dispatches the judges after publication",
        "Read it before launching judges.",
        "Add a judge when the tier is high",
        "A correction needs a judge to confirm it",
        "the tier convened two judges, see `tier.rs` for e.g. the count, and launches a judge",
        "Every ordinary review has two judges that run concurrently.",
        "Ordinary review gets two judges, which launch in parallel.",
        "Ordinary review launches the [blind judges](policies/blind-judges.md).",
        "Ordinary review's two judges each run concurrently.",
        "The two judges each launch in parallel.",
    ] {
        assert!(
            !judges_to_acquire(sentence).is_empty(),
            "the checker let through: {sentence}"
        );
    }
}

#[test]
fn the_checker_flags_every_evasion_and_every_negation() {
    let missed: Vec<&str> = [
        "Ordinary review launches two blind judges.",
        "Never skip this step: launch two judges.",
        "If it isn't clean, launch two judges.",
        "Judgment Day aside, ordinary review launches two judges.",
        "Answer: no. Launch two judges.",
        "No judge is needed for a lens and ordinary review launches two judges.",
        "At every tier, ordinary review launches the following: two blind judges.",
        "Judgment Day aside, every review launches two judges.",
        "Ordinary review never launches a judge",
        "no tier acquires a judge",
        "It acquires no judge at any tier",
        "Judgment Day launches two judges, and only when the operator explicitly asks for it",
    ]
    .into_iter()
    .filter(|sentence| judges_to_acquire(sentence).is_empty())
    .collect();
    assert!(missed.is_empty(), "the checker let through: {missed:#?}");
}

#[test]
fn an_allowed_sentence_passes_and_any_one_word_change_to_it_is_flagged() {
    let (sentence, _) = allowed()[0];
    assert!(unlisted(sentence).is_empty());
    assert!(!unlisted(&sentence.replacen(' ', " really ", 1)).is_empty());
}

#[test]
fn every_allowed_sentence_still_occurs_in_the_scanned_texts() {
    let texts = scanned_texts();
    for (sentence, reason) in allowed() {
        assert!(reason.len() > 10, "{sentence:?} says nothing about why");
        assert!(
            texts.iter().any(|(_, text)| judges_to_acquire(text)
                .iter()
                .any(|found| found == sentence)),
            "a stale ALLOWED entry excuses nothing and should be deleted: {sentence:?}"
        );
    }
}

#[test]
fn the_splitter_keeps_dotted_tokens_inside_their_sentence() {
    let sentences = sentences_of(
        "Read `tier.rs` before you launch the lens, e.g. the one `risk` names. Ordinary review launches two judges.",
    );
    assert_eq!(
        sentences,
        vec![
            "Read `tier.rs` before you launch the lens, e.g. the one `risk` names".to_owned(),
            "Ordinary review launches two judges".to_owned(),
        ]
    );
    assert!(
        !judges_to_acquire(
            "Judgment Day is separate. Read `tier.rs` first, then launch two judges for every tier."
        )
        .is_empty(),
        "a dotted token hid a sentence from the checker"
    );
}

#[test]
fn the_checker_reads_a_list_item_and_a_table_row_as_their_own_sentences() {
    let text = "- Judgment Day launches two judges\n- ordinary review launches two judges\n\n| a | b |\n| the tier | acquires a judge |\n";
    assert_eq!(judges_to_acquire(text).len(), 3);
}

fn scanned_texts() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let skill_root = root.join("skill");
    let mut texts: Vec<(String, String)> = Vec::new();
    for path in markdown_files_under(&skill_root) {
        let name = path
            .strip_prefix(&root)
            .expect("under the manifest")
            .to_string_lossy()
            .into_owned();
        let name = name.replace('\\', "/");
        if name.starts_with("skill/agents/review-blind") {
            continue;
        }
        if name == "skill/policies/judgment-day.md" {
            continue;
        }
        let contents = std::fs::read_to_string(&path).expect("a skill file is readable");
        texts.push((name, contents));
    }
    assert!(
        texts.len() >= 20,
        "the walk found only {} skill files",
        texts.len()
    );

    let config = estigia::config::Config::default();
    for file in estigia::skill::FILES {
        if file.path == "policies/judgment-day.md" {
            continue;
        }
        let rendered = estigia::skill::rendered(file, &config);
        texts.push((format!("the rendered {}", file.path), rendered));
    }
    texts.push((
        "the OpenCode review block".to_owned(),
        estigia::setup::opencode_review_section(),
    ));
    texts
}

#[test]
fn no_text_an_agent_acts_on_for_ordinary_review_acquires_judges() {
    let mut offending = Vec::new();
    for (name, text) in &scanned_texts() {
        for sentence in unlisted(text) {
            offending.push(format!("{name}: {sentence}"));
        }
    }
    assert!(
        offending.is_empty(),
        "text an agent acts on tells it to acquire or launch judges outside Judgment Day: {offending:#?}"
    );
}

#[test]
fn the_rendered_contract_states_how_many_lenses_each_tier_licenses() {
    let config = estigia::config::Config::default();
    let contract_file = estigia::skill::FILES
        .iter()
        .find(|file| file.path == estigia::skill::CONTRACT)
        .expect("the contract ships");
    let rendered = flat(&estigia::skill::rendered(contract_file, &config));
    assert!(
        rendered.contains("one at `medium`, four at `high`, zero at `low`"),
        "the rendered contract no longer states the lens count per tier"
    );
}

#[test]
fn the_opencode_block_states_how_many_lenses_each_tier_licenses() {
    let block = flat(&estigia::setup::opencode_review_section());
    assert!(
        block.contains("one at `medium`, four at `high`, zero at `low`"),
        "the OpenCode block no longer states the lens count per tier: {block}"
    );
}
