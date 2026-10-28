use super::{Basis, Places, Prefer, resolve, spelled};
use crate::identity::Reference;
use crate::transport::Context;

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn store(&self) -> std::path::PathBuf {
        self.directory.path().join("state.db")
    }

    fn root(&self) -> std::path::PathBuf {
        self.directory.path().join("state")
    }

    fn ask(&self, text: &str, prefer: Prefer) -> Result<super::Scope, crate::transport::Failure> {
        let context = Context {
            skill_dir: self.directory.path().to_path_buf(),
            repo_dir: self.directory.path().to_path_buf(),
            config: Vec::new(),
            repo: Some("o/a".to_owned()),
        };
        let root = self.root();
        resolve(
            &context,
            &Reference::parse(text).expect("a reference"),
            &Places {
                store_file: &self.store(),
                state_root: Some(&root),
            },
            prefer,
        )
    }
}

fn rig() -> Rig {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let rig = Rig { directory };
    drop(crate::transport::store::Store::open(&rig.store()).expect("the store opens"));
    rusqlite::Connection::open(rig.store())
        .expect("the store opens")
        .execute_batch(
            "INSERT INTO repos (id, host, owner, name) VALUES
               (1, 'github.com', 'o', 'a'), (2, 'github.com', 'o', 'b'), (3, 'github.com', 'o', 'c');
             INSERT INTO issues (repo_id, number, state, history) VALUES
               (1, 9, 'ready', 'native'), (2, 9, 'ready', 'native'),
               (2, 77, 'in-progress', 'native'),
               (2, 5, 'ready', 'native'), (3, 5, 'ready', 'native');",
        )
        .expect("the fixture seeds");
    rig
}

fn origin(directory: &std::path::Path, url: &str) {
    std::fs::create_dir_all(directory).expect("a checkout directory");
    for arguments in [
        vec!["init", "--quiet", "-b", "main"],
        vec!["remote", "add", "origin", url],
    ] {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(directory)
                .args(&arguments)
                .output()
                .is_ok_and(|output| output.status.success()),
            "git {arguments:?}"
        );
    }
}

#[test]
fn a_number_held_only_in_another_repository_resolves_there() {
    let found = rig().ask("77", Prefer::Ambiguity).expect("one candidate");

    assert_eq!(spelled(&found.repo), "o/b");
    assert_eq!(found.basis, Basis::Candidate);
}

#[test]
fn a_number_held_in_two_repositories_is_refused_with_both() {
    for prefer in [Prefer::Ambiguity, Prefer::Checkout] {
        let refused = rig().ask("5", prefer).expect_err("two candidates");

        let envelope = refused.envelope();
        assert_eq!(envelope["reason"], "ambiguous-issue", "{envelope}");
        assert_eq!(envelope["candidates"], serde_json::json!(["o/b", "o/c"]));
    }
}

#[test]
fn the_checkout_keeps_a_number_it_holds_only_where_the_caller_asked_for_that() {
    let rig = rig();

    let kept = rig.ask("9", Prefer::Checkout).expect("the checkout's own");
    assert_eq!(
        (spelled(&kept.repo).as_str(), kept.basis),
        ("o/a", Basis::Checkout)
    );
    let refused = rig.ask("9", Prefer::Ambiguity).expect_err("strict");
    assert_eq!(
        refused.envelope()["candidates"],
        serde_json::json!(["o/a", "o/b"])
    );
}

#[test]
fn a_number_nobody_holds_falls_back_to_the_checkout_and_says_so() {
    let found = rig().ask("123", Prefer::Ambiguity).expect("the checkout");

    assert_eq!(
        (spelled(&found.repo).as_str(), found.basis),
        ("o/a", Basis::Checkout)
    );
}

#[test]
fn a_qualified_reference_is_used_as_given_even_where_the_store_has_never_seen_it() {
    let found = rig().ask("x/y#5", Prefer::Ambiguity).expect("as given");

    assert_eq!(
        (spelled(&found.repo).as_str(), found.basis),
        ("x/y", Basis::Given)
    );
}

#[test]
fn a_live_run_pointer_places_a_number_the_store_never_saw() {
    let rig = rig();
    let checkout = rig.directory.path().join("elsewhere");
    origin(&checkout, "git@github.com:o/c.git");
    crate::harness::session::update(&rig.root(), "claude-aaaa", |run| {
        run.issue = Some(crate::identity::Identity::Number(300));
        run.repo_dir = Some(checkout.clone());
    });

    let found = rig
        .ask("300", Prefer::Ambiguity)
        .expect("the pointer's repository");

    assert_eq!(
        (spelled(&found.repo).as_str(), found.basis),
        ("o/c", Basis::Candidate)
    );
}
