use std::collections::{BTreeMap, HashSet};

use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{VOICE_CAP, read_capped, refuse, voice_path};
use crate::transport::Failure;

const LEADING: [char; 5] = ['¿', '¡', '*', '"', '\''];
const TRAILING: [char; 7] = ['.', ',', ';', ':', '!', '?', '*'];
const NOT_A_WORD: [char; 8] = ['_', '$', '/', '\\', '.', ':', '(', '`'];
const PROTECTED: &str = r#"(?s)```.*?(?:```|\z)|`[^`]*`|https?://\S+|"[^"]*"|@[\w-]+"#;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Spec {
    accents: Accents,
    lowercase_start: f64,
    final_stop_dropped: f64,
    opening_marks_dropped: f64,
    space_before_closing_mark: f64,
    typo: Typo,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Accents {
    dropped: BTreeMap<String, f64>,
    other: f64,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Typo {
    share: f64,
    min_words: usize,
    kinds: Option<Kinds>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct Kinds {
    swapped: f64,
    dropped: f64,
    doubled: f64,
    neighbour: f64,
}

#[derive(Clone, Copy)]
enum Kind {
    Swapped,
    Dropped,
    Doubled,
    Neighbour,
}

impl Kinds {
    fn weights(&self) -> [(Kind, f64); 4] {
        [
            (Kind::Swapped, self.swapped),
            (Kind::Dropped, self.dropped),
            (Kind::Doubled, self.doubled),
            (Kind::Neighbour, self.neighbour),
        ]
    }

    fn draw(&self, stream: &mut Stream) -> Kind {
        let total: f64 = self.weights().iter().map(|(_, weight)| weight).sum();
        let mut left = stream.next() * total;
        let mut last = Kind::Swapped;
        for (kind, weight) in self
            .weights()
            .into_iter()
            .filter(|(_, weight)| *weight > 0.0)
        {
            last = kind;
            if left < weight {
                break;
            }
            left -= weight;
        }
        last
    }
}

const ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

fn beside(letter: char) -> Vec<char> {
    let lower = letter.to_ascii_lowercase();
    ROWS.iter()
        .find_map(|row| {
            let at = row.find(lower)?;
            let row: Vec<char> = row.chars().collect();
            Some(
                [at.checked_sub(1), Some(at + 1)]
                    .into_iter()
                    .flatten()
                    .filter_map(|near| row.get(near).copied())
                    .map(|near| {
                        if letter.is_uppercase() {
                            near.to_ascii_uppercase()
                        } else {
                            near
                        }
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn swap_inner(letters: &mut [char], stream: &mut Stream) -> bool {
    let inner: Vec<usize> = (1..letters.len() - 2)
        .filter(|at| letters[*at] != letters[*at + 1])
        .collect();
    if inner.is_empty() {
        return false;
    }
    let at = inner[stream.pick(inner.len())];
    letters.swap(at, at + 1);
    true
}

fn edit_inner(letters: &mut Vec<char>, kind: Kind, stream: &mut Stream) -> bool {
    let at = 1 + stream.pick(letters.len() - 2);
    match kind {
        Kind::Dropped => {
            letters.remove(at);
        }
        Kind::Doubled => letters.insert(at, letters[at]),
        _ => {
            let near = beside(letters[at]);
            if near.is_empty() {
                return false;
            }
            letters[at] = near[stream.pick(near.len())];
        }
    }
    true
}

impl Spec {
    fn rates(&self) -> impl Iterator<Item = f64> {
        [
            self.accents.other,
            self.lowercase_start,
            self.final_stop_dropped,
            self.opening_marks_dropped,
            self.space_before_closing_mark,
            self.typo.share,
        ]
        .into_iter()
        .chain(self.accents.dropped.values().copied())
        .chain(
            self.typo
                .kinds
                .iter()
                .flat_map(|kinds| kinds.weights().map(|(_, weight)| weight)),
        )
    }
}

pub(super) struct Habits {
    pub(super) source: String,
    spec: Spec,
    rates: BTreeMap<String, f64>,
    listed: HashSet<String>,
    protected: Regex,
}

struct Stream {
    seed: [u8; 32],
    at: u64,
}

impl Stream {
    fn seeded(source: &str, text: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(source.as_bytes());
        digest.update([0]);
        digest.update(text.as_bytes());
        Self {
            seed: digest.finalize().into(),
            at: 0,
        }
    }

    fn next(&mut self) -> f64 {
        let mut digest = Sha256::new();
        digest.update(self.seed);
        digest.update(self.at.to_le_bytes());
        self.at += 1;
        let mut eight = [0u8; 8];
        eight.copy_from_slice(&digest.finalize()[..8]);
        (u64::from_le_bytes(eight) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, probability: f64) -> bool {
        self.next() < probability
    }

    fn pick(&mut self, of: usize) -> usize {
        ((self.next() * of as f64) as usize).min(of - 1)
    }
}

struct Word {
    lead: String,
    core: String,
    trail: String,
}

enum Piece {
    Space(String),
    Fixed(String),
    Word(Word),
}

fn folded(word: &str) -> String {
    word.nfc().collect::<String>().to_lowercase()
}

fn without_acutes(word: &str) -> Option<String> {
    let mut out = String::new();
    let mut changed = false;
    let mut after_vowel = false;
    for letter in word.nfd() {
        if letter == '\u{301}' && after_vowel {
            changed = true;
            after_vowel = false;
            continue;
        }
        after_vowel = "aeiouAEIOU".contains(letter);
        out.push(letter);
    }
    changed.then(|| out.nfc().collect())
}

fn plain(core: &str) -> bool {
    !core.contains("->")
        && !core
            .chars()
            .any(|letter| letter.is_numeric() || NOT_A_WORD.contains(&letter))
        && !core.chars().skip(1).any(char::is_uppercase)
}

fn token(run: &str) -> Piece {
    let body = run.trim_start_matches(LEADING);
    let lead = &run[..run.len() - body.len()];
    let core = body.trim_end_matches(TRAILING);
    if !plain(core) {
        return Piece::Fixed(run.to_owned());
    }
    Piece::Word(Word {
        lead: lead.to_owned(),
        core: core.to_owned(),
        trail: body[core.len()..].to_owned(),
    })
}

fn free(segment: &str, pieces: &mut Vec<Piece>) {
    let mut rest = segment;
    while let Some(first) = rest.chars().next() {
        let space = first.is_whitespace();
        let end = rest
            .find(|letter: char| letter.is_whitespace() != space)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        pieces.push(if space {
            Piece::Space(run.to_owned())
        } else {
            token(run)
        });
        rest = tail;
    }
}

impl Habits {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, String> {
        let source =
            std::str::from_utf8(bytes).map_err(|error| format!("is not UTF-8: {error}"))?;
        let spec: Spec = serde_json::from_str(source)
            .map_err(|error| format!("is not the habits shape: {error}"))?;
        if !spec.rates().all(|rate| (0.0..=1.0).contains(&rate)) {
            return Err("holds a probability outside 0 to 1".to_owned());
        }
        let rates: BTreeMap<String, f64> = spec
            .accents
            .dropped
            .iter()
            .map(|(word, rate)| (folded(word), *rate))
            .collect();
        let listed = rates
            .keys()
            .flat_map(|word| [Some(word.clone()), without_acutes(word)])
            .flatten()
            .collect();
        Ok(Self {
            source: source.to_owned(),
            spec,
            rates,
            listed,
            protected: Regex::new(PROTECTED).map_err(|error| error.to_string())?,
        })
    }

    pub(super) fn apply(&self, text: &str) -> String {
        let mut stream = Stream::seeded(&self.source, text);
        let mut pieces = self.segment(text);
        self.lowercase_first(&mut pieces, &mut stream);
        for piece in &mut pieces {
            if let Piece::Word(word) = piece {
                self.respell(word, &mut stream);
            }
        }
        self.drop_final_stop(&mut pieces, &mut stream);
        self.slip(&mut pieces, &mut stream);
        pieces
            .into_iter()
            .map(|piece| match piece {
                Piece::Space(text) | Piece::Fixed(text) => text,
                Piece::Word(word) => word.lead + &word.core + &word.trail,
            })
            .collect()
    }

    fn segment(&self, text: &str) -> Vec<Piece> {
        let mut pieces = Vec::new();
        let mut from = 0;
        for found in self.protected.find_iter(text) {
            free(&text[from..found.start()], &mut pieces);
            pieces.push(Piece::Fixed(found.as_str().to_owned()));
            from = found.end();
        }
        free(&text[from..], &mut pieces);
        pieces
    }

    fn lowercase_first(&self, pieces: &mut [Piece], stream: &mut Stream) {
        let Some(Piece::Word(word)) = pieces
            .iter_mut()
            .find(|piece| !matches!(piece, Piece::Space(_)))
        else {
            return;
        };
        let mut letters = word.core.chars();
        let Some(first) = letters.next().filter(|letter| letter.is_uppercase()) else {
            return;
        };
        if stream.below(self.spec.lowercase_start) {
            word.core = first.to_lowercase().chain(letters).collect();
        }
    }

    fn respell(&self, word: &mut Word, stream: &mut Stream) {
        word.lead = word
            .lead
            .chars()
            .filter(|mark| {
                !(matches!(mark, '¿' | '¡') && stream.below(self.spec.opening_marks_dropped))
            })
            .collect();
        if let Some(bare) = without_acutes(&word.core) {
            let rate = self
                .rates
                .get(&folded(&word.core))
                .copied()
                .unwrap_or(self.spec.accents.other);
            if stream.below(rate) {
                word.core = bare;
            }
        }
        if word.trail.starts_with(['?', '!'])
            && word
                .core
                .chars()
                .next_back()
                .is_some_and(char::is_alphabetic)
            && stream.below(self.spec.space_before_closing_mark)
        {
            word.trail.insert(0, ' ');
        }
    }

    fn drop_final_stop(&self, pieces: &mut [Piece], stream: &mut Stream) {
        let Some(Piece::Word(word)) = pieces
            .iter_mut()
            .rev()
            .find(|piece| !matches!(piece, Piece::Space(_)))
        else {
            return;
        };
        if word.trail.ends_with('.')
            && !word.trail.ends_with("..")
            && stream.below(self.spec.final_stop_dropped)
        {
            word.trail.pop();
        }
    }

    fn slip(&self, pieces: &mut [Piece], stream: &mut Stream) {
        let words = pieces
            .iter()
            .filter(|piece| match piece {
                Piece::Space(_) => false,
                Piece::Fixed(_) => true,
                Piece::Word(word) => !word.core.is_empty(),
            })
            .count();
        if words < self.spec.typo.min_words || !stream.below(self.spec.typo.share) {
            return;
        }
        let eligible: Vec<usize> = (0..pieces.len())
            .filter(|at| match &pieces[*at] {
                Piece::Word(word) => self.can_slip(&word.core),
                _ => false,
            })
            .collect();
        if eligible.is_empty() {
            return;
        }
        let Piece::Word(word) = &mut pieces[eligible[stream.pick(eligible.len())]] else {
            return;
        };
        let mut letters: Vec<char> = word.core.chars().collect();
        let drawn = self
            .spec
            .typo
            .kinds
            .as_ref()
            .filter(|kinds| kinds.weights().iter().any(|(_, weight)| *weight > 0.0));
        let kind = drawn.map_or(Kind::Swapped, |kinds| kinds.draw(stream));
        let done = match kind {
            Kind::Swapped => swap_inner(&mut letters, stream),
            _ => edit_inner(&mut letters, kind, stream),
        };
        if !done && drawn.is_none() {
            return;
        }
        if !done {
            edit_inner(&mut letters, Kind::Dropped, stream);
        }
        word.core = letters.into_iter().collect();
    }

    fn can_slip(&self, core: &str) -> bool {
        core.len() >= 6
            && core.chars().all(|letter| letter.is_ascii_alphabetic())
            && !self.listed.contains(&core.to_lowercase())
    }
}

pub(super) fn read(name: &str) -> Result<Option<Habits>, Failure> {
    let path = voice_path(&format!("{name}.habits.json"))?;
    let invalid = |cause: String| {
        refuse(
            "review-voice-habits-invalid",
            format!("the habits file {} {cause}", path.display()),
            "correct the habits file or remove it; nothing was posted",
        )
    };
    let unreadable = |error: std::io::Error| invalid(format!("could not be read: {error}"));
    match std::fs::metadata(&path) {
        Ok(found) if found.file_type().is_file() => {}
        Ok(_) => return Err(invalid("is not a regular file".to_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(unreadable(error)),
    }
    let bytes = read_capped(std::fs::File::open(&path).map_err(unreadable)?).map_err(unreadable)?;
    if bytes.len() as u64 > VOICE_CAP {
        return Err(invalid(format!(
            "is {} bytes or more and the cap is {VOICE_CAP}",
            bytes.len()
        )));
    }
    Habits::parse(&bytes).map(Some).map_err(invalid)
}

#[cfg(test)]
mod tests;
