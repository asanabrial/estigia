use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::Renderer;
use crate::outcome::Refusal;
use crate::reviewer::job::OnThread;
use crate::reviewer::{Adapter, Invocation};

pub(crate) const PREFIX: &str = "voiced: ";

#[derive(Clone, Copy)]
pub(crate) enum Answers {
    Prefixed,
    Counting,
    OneShort,
    NotJson,
    Blank,
}

struct Voiced {
    answers: Answers,
    probe: Probe,
}

#[derive(Clone, Default)]
pub(crate) struct Probe {
    pub(crate) launches: Arc<AtomicUsize>,
    pub(crate) prompts: Arc<Mutex<Vec<String>>>,
}

impl Probe {
    pub(crate) fn launched(&self) -> usize {
        self.launches.load(Ordering::SeqCst)
    }

    pub(crate) fn prompt(&self) -> String {
        self.prompts
            .lock()
            .expect("the lock")
            .last()
            .cloned()
            .expect("a job was launched")
    }
}

impl Adapter for Voiced {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        let launch = self.probe.launches.fetch_add(1, Ordering::SeqCst) + 1;
        let prompt = String::from_utf8_lossy(&invocation.prompt).into_owned();
        let (_, after) = prompt
            .split_once("## Texts\n\n")
            .expect("the prompt carries its texts");
        let line = after.lines().next().expect("a line of texts");
        let texts: Vec<String> = serde_json::from_str(line).expect("a JSON array of texts");
        self.probe.prompts.lock().expect("the lock").push(prompt);
        let mut voiced: Vec<String> = texts.iter().map(|text| format!("{PREFIX}{text}")).collect();
        Ok(match self.answers {
            Answers::Prefixed => serde_json::json!({ "texts": voiced }).to_string(),
            Answers::Counting => {
                let numbered: Vec<String> = texts
                    .iter()
                    .map(|text| format!("voiced #{launch}: {text}"))
                    .collect();
                serde_json::json!({ "texts": numbered }).to_string()
            }
            Answers::OneShort => {
                voiced.pop();
                serde_json::json!({ "texts": voiced }).to_string()
            }
            Answers::NotJson => "no JSON here".to_owned(),
            Answers::Blank => serde_json::json!({ "texts": vec!["  "; texts.len()] }).to_string(),
        }
        .into_bytes())
    }
}

pub(crate) fn renderer(answers: Answers) -> (Renderer, Probe) {
    let probe = Probe::default();
    let handed = probe.clone();
    let spawner = OnThread(move || {
        Ok(Box::new(Voiced {
            answers,
            probe: handed.clone(),
        }) as Box<dyn Adapter>)
    });
    (Renderer::on(spawner), probe)
}

pub(crate) fn voices_dir() -> std::path::PathBuf {
    crate::transport::store::store_path()
        .expect("a store path")
        .parent()
        .expect("a state directory")
        .join("voices")
}

pub(crate) fn write_voice(name: &str, text: &str) {
    let dir = voices_dir();
    std::fs::create_dir_all(&dir).expect("a voices directory");
    crate::paths::replace_atomically(&dir.join(format!("{name}.md")), text).expect("a voice file");
}

pub(crate) fn name_voice(repo_dir: &Path, value: &str) {
    let path = crate::skill::repository_config_path(repo_dir);
    std::fs::create_dir_all(path.parent().expect("a directory")).expect("a state directory");
    crate::paths::replace_atomically(
        &path,
        &format!("| Setting | Value here |\n|---|---|\n| Review voice | {value} |\n"),
    )
    .expect("a repository file");
}

struct Refuses;

impl Adapter for Refuses {
    fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
        Err(crate::reviewer::ReviewerCode::Unavailable.refusal("the quota is spent"))
    }
}

pub(crate) fn renderer_that_refuses() -> Renderer {
    Renderer::on(OnThread(|| Ok(Box::new(Refuses) as Box<dyn Adapter>)))
}

struct Hangs;

impl Adapter for Hangs {
    fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
        std::thread::sleep(std::time::Duration::from_secs(3));
        Ok(b"{\"texts\":[]}".to_vec())
    }
}

pub(crate) fn renderer_that_hangs() -> Renderer {
    Renderer::on(OnThread(|| Ok(Box::new(Hangs) as Box<dyn Adapter>)))
}
