//! Which repository an issue reference means.
//!
//! A bare number names an issue in some repository, and the checkout the caller stands in is
//! only one guess at which. The store knows every repository it has seen the number in and the
//! run pointers know every checkout a live run holds it from, so those are asked first; the
//! checkout answers only when nobody else can. One resolver, so a reader and a writer cannot
//! disagree about where an issue is.

use std::path::Path;

use super::claim::repo_of;
use super::store::{self, Repo};
use super::{Context, Failure, How, run};
use crate::config::RepoRef;
use crate::identity::{Identity, Reference};

/// How the repository of a reference was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// The reference named it.
    Given,
    /// The store or a live run pointer places the number in this repository alone.
    Candidate,
    /// Nothing else places the number, so the checkout's repository is the guess.
    Checkout,
}

impl Basis {
    /// The word an answer carries for how the repository was found.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Given => "given",
            Self::Candidate => "candidate",
            Self::Checkout => "checkout",
        }
    }
}

/// What to do when the checkout's repository is one of several candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prefer {
    /// Refuse: the number is ambiguous whichever repository is asking.
    Ambiguity,
    /// Keep the checkout's repository, as a command that always meant it still does.
    Checkout,
}

/// A reference placed in one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope {
    /// The repository the issue is in.
    pub(crate) repo: Repo,
    /// The issue number.
    pub(crate) number: u64,
    /// How `repo` was found.
    pub(crate) basis: Basis,
}

/// Where the resolver looks besides the checkout.
pub(crate) struct Places<'a> {
    /// The local store file.
    pub(crate) store_file: &'a Path,
    /// The run pointers' directory, when one can be named.
    pub(crate) state_root: Option<&'a Path>,
}

pub(crate) fn spelled(repo: &Repo) -> String {
    format!("{}/{}", repo.owner, repo.name)
}

/// The repository a checkout's `origin` names, or `None` when it names none on GitHub.
pub(crate) fn repo_at(directory: &Path) -> Option<Repo> {
    let url = run(
        &["git", "remote", "get-url", "origin"],
        Some(directory),
        How::tolerated(),
    )
    .ok()
    .filter(|url| url.status == 0)?;
    RepoRef::from_github_remote(&url.stdout).map(|found| Repo {
        host: "github.com".to_owned(),
        owner: found.owner,
        name: found.name,
    })
}

fn ambiguous(number: u64, candidates: &[Repo]) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "ambiguous-issue",
        "issue": number,
        "candidates": candidates.iter().map(spelled).collect::<Vec<_>>(),
        "action": "name the repository: `owner/name#N`",
    }))
}

fn candidates(number: u64, places: &Places<'_>) -> Result<Vec<Repo>, Failure> {
    let mut found = match store::open_for_read(places.store_file)? {
        Some(opened) => opened.repos_holding(number)?,
        None => Vec::new(),
    };
    for pointer in places
        .state_root
        .map(crate::harness::session::holdings)
        .unwrap_or_default()
    {
        if pointer.issue == Some(Identity::Number(number))
            && let Some(repo) = pointer.repo_dir.as_deref().and_then(repo_at)
            && !found.contains(&repo)
        {
            found.push(repo);
        }
    }
    Ok(found)
}

/// The repository of a number-bearing reference.
///
/// Refused `ambiguous-issue` with every candidate when several repositories hold the number and
/// `prefer` leaves no way to choose; read-only throughout.
pub(crate) fn resolve(
    context: &Context,
    reference: &Reference,
    places: &Places<'_>,
    prefer: Prefer,
) -> Result<Scope, Failure> {
    let number = match reference {
        Reference::Qualified {
            owner,
            name,
            number,
        } => {
            return Ok(Scope {
                repo: Repo {
                    host: "github.com".to_owned(),
                    owner: owner.clone(),
                    name: name.clone(),
                },
                number: *number,
                basis: Basis::Given,
            });
        }
        Reference::Local(Identity::Number(number)) => *number,
        Reference::Local(Identity::Key(key)) => {
            return Err(Failure::Read(format!(
                "`{key}` is a tracker key and names no GitHub repository"
            )));
        }
    };
    let found = candidates(number, places)?;
    let at = |repo: Repo, basis| Scope {
        repo,
        number,
        basis,
    };
    let checkout = repo_of(context);
    match found.as_slice() {
        [] => checkout.map(|repo| at(repo, Basis::Checkout)),
        [only] => Ok(at(only.clone(), Basis::Candidate)),
        _ => match checkout {
            Ok(repo) if prefer == Prefer::Checkout && found.contains(&repo) => {
                Ok(at(repo, Basis::Checkout))
            }
            _ => Err(ambiguous(number, &found)),
        },
    }
}

#[cfg(test)]
mod tests;
