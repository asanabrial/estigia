use std::path::PathBuf;

use super::{Renderer, habits, read_voice, refuse, reworded, voice_path, with_habits};
use crate::config::ReviewVoice;
use crate::transport::Failure;

/// What `voice check` found: the files a post would read, and their sizes.
#[derive(Debug)]
pub(crate) struct Checked {
    pub(crate) voice: PathBuf,
    pub(crate) voice_bytes: u64,
    pub(crate) habits: Option<(PathBuf, u64)>,
}

/// The texts `voice preview` produced, and whether a habits file was applied.
#[derive(Debug)]
pub(crate) struct Previewed {
    pub(crate) texts: Vec<String>,
    pub(crate) habits: bool,
}

fn standalone(failure: Failure) -> Failure {
    match failure {
        Failure::Stop(mut payload)
            if payload["reason"]
                .as_str()
                .is_some_and(|reason| reason.starts_with("review-voice-")) =>
        {
            payload["action"] = "nothing was posted; correct the voice files and run the command \
                                 again"
                .into();
            Failure::Stop(payload)
        }
        other => other,
    }
}

fn named(name: &str) -> Result<(), Failure> {
    match ReviewVoice::parse(name) {
        Some(ReviewVoice::Named(_)) => Ok(()),
        _ => Err(refuse(
            "review-voice-file-missing",
            format!(
                "`{name}` is not a voice name (lowercase letters, digits and `-`), so no voice \
                 file is called that"
            ),
            "name the voice by the word its file is called, as in `voices/<name>.md`",
        )),
    }
}

fn size(path: &std::path::Path) -> Result<u64, Failure> {
    std::fs::metadata(path)
        .map(|found| found.len())
        .map_err(|error| {
            Failure::Read(format!("{} could not be measured: {error}", path.display()))
        })
}

/// Reads a voice's two files as a post would and reports them, or refuses as the
/// post would, without the advice that belongs to a repository's row.
pub(crate) fn check(name: &str) -> Result<Checked, Failure> {
    named(name)?;
    read_voice(name).map_err(standalone)?;
    let habits = habits::read(name).map_err(standalone)?;
    let voice = voice_path(&format!("{name}.md"))?;
    let habits = match habits {
        Some(_) => {
            let path = voice_path(&format!("{name}.habits.json"))?;
            let bytes = size(&path)?;
            Some((path, bytes))
        }
        None => None,
    };
    Ok(Checked {
        voice_bytes: size(&voice)?,
        voice,
        habits,
    })
}

/// Passes `texts` through the voice's job and habits, or through the habits
/// alone, and keeps nothing: the store is neither read nor written.
pub(crate) fn preview(
    routing: &str,
    renderer: &Renderer,
    name: &str,
    texts: &[String],
    habits_only: bool,
) -> Result<Previewed, Failure> {
    named(name)?;
    let habits = habits::read(name).map_err(standalone)?;
    let applied = habits.is_some();
    let texts = if habits_only {
        texts
            .iter()
            .map(|text| with_habits(habits.as_ref(), text))
            .collect()
    } else if texts.is_empty() {
        Vec::new()
    } else {
        let voice = read_voice(name).map_err(standalone)?;
        let inputs: Vec<&str> = texts.iter().map(String::as_str).collect();
        reworded(routing, renderer, name, &voice, habits.as_ref(), &inputs).map_err(standalone)?
    };
    Ok(Previewed {
        texts,
        habits: applied,
    })
}
