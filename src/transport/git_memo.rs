use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use super::{EnvMode, How, Output};

pub(crate) const ISOLATED_DIR_PREFIX: &str = ".estigia-isolated-diff-";

const REPOSITORY_LOOKUP: [&str; 5] = [
    "rev-parse",
    "--path-format=absolute",
    "--git-dir",
    "--git-common-dir",
    "--show-object-format",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    args: Vec<String>,
    cwd: Option<PathBuf>,
    replace: bool,
    envs: Vec<(String, String)>,
}

struct Scope {
    depth: usize,
    answers: HashMap<Key, Output>,
}

thread_local! {
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
    #[cfg(test)]
    static SPAWNED: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct Guard {
    not_send: PhantomData<*const ()>,
}

pub(crate) fn scope() -> Guard {
    SCOPE.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_mut() {
            Some(open) => open.depth += 1,
            None => {
                *slot = Some(Scope {
                    depth: 1,
                    answers: HashMap::new(),
                });
            }
        }
    });
    Guard {
        not_send: PhantomData,
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        SCOPE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let closed = slot.as_mut().is_some_and(|open| {
                open.depth -= 1;
                open.depth == 0
            });
            if closed {
                *slot = None;
            }
        });
    }
}

pub(crate) fn key_for(
    args: &[&str],
    cwd: Option<&Path>,
    how: How,
    envs: &[(&str, &str)],
    mode: EnvMode,
) -> Option<Key> {
    if how.writes {
        return None;
    }
    if !SCOPE.with(|slot| slot.borrow().is_some()) || !memoisable(args) {
        return None;
    }
    let mut kept: Vec<(String, String)> = envs
        .iter()
        .filter(|(name, value)| !(*name == "GIT_DIR" && is_isolated_dir(value)))
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    kept.sort();
    Some(Key {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: cwd.map(Path::to_path_buf),
        replace: mode == EnvMode::Replace,
        envs: kept,
    })
}

pub(crate) fn recall(key: &Key) -> Option<Output> {
    SCOPE.with(|slot| slot.borrow().as_ref()?.answers.get(key).cloned())
}

pub(crate) fn remember(key: Key, answer: &Output) {
    if answer.status != 0 {
        return;
    }
    SCOPE.with(|slot| {
        if let Some(open) = slot.borrow_mut().as_mut() {
            open.answers.insert(key, answer.clone());
        }
    });
}

fn is_isolated_dir(value: &str) -> bool {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(ISOLATED_DIR_PREFIX))
}

fn is_full_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn memoisable(args: &[&str]) -> bool {
    let Some((&"git", rest)) = args.split_first() else {
        return false;
    };
    let mut rest = rest;
    while let [first, tail @ ..] = rest {
        match *first {
            "-c" => match tail {
                [_, after @ ..] => rest = after,
                [] => return false,
            },
            _ => break,
        }
    }
    let Some((&subcommand, operands)) = rest.split_first() else {
        return false;
    };
    match subcommand {
        "rev-parse" => rest == REPOSITORY_LOOKUP || fixed_commit_lookup(operands),
        "ls-tree" => revisions_of(operands).is_some_and(|revisions| revisions.len() == 1),
        "diff" => revisions_of(operands).is_some_and(|revisions| revisions.len() == 2),
        "cat-file" => matches!(
            operands,
            ["blob", spec] if spec
                .split_once(':')
                .is_some_and(|(oid, _)| is_full_object_id(oid))
        ),
        _ => false,
    }
}

fn fixed_commit_lookup(operands: &[&str]) -> bool {
    matches!(
        operands,
        ["--verify", spec] if spec
            .strip_suffix("^{commit}")
            .is_some_and(is_full_object_id)
    )
}

fn revisions_of<'a>(operands: &[&'a str]) -> Option<Vec<&'a str>> {
    let mut revisions = Vec::new();
    for operand in operands {
        if *operand == "--" {
            return Some(revisions);
        }
        if operand.starts_with('-') {
            continue;
        }
        if !is_full_object_id(operand) {
            return None;
        }
        revisions.push(*operand);
    }
    Some(revisions)
}

#[cfg(test)]
pub(crate) fn note_spawn(args: &[&str]) {
    SPAWNED.with(|log| {
        log.borrow_mut()
            .push(args.iter().map(|arg| (*arg).to_owned()).collect());
    });
}

#[cfg(test)]
pub(crate) fn spawned() -> Vec<Vec<String>> {
    SPAWNED.with(|log| log.borrow().clone())
}

#[cfg(test)]
pub(crate) fn forget_spawns() {
    SPAWNED.with(|log| log.borrow_mut().clear());
}

#[cfg(test)]
mod tests;
