use super::*;
use crate::harness::session;
use crate::transport::store::{Repo, Store};

fn repo() -> Repo {
    Repo {
        host: "github.com".to_owned(),
        owner: "asanabrial".to_owned(),
        name: "estigia".to_owned(),
    }
}

fn store_in(directory: &std::path::Path) -> Store {
    Store::open(&directory.join("state.db")).expect("the store opens")
}

fn issue() -> serde_json::Value {
    serde_json::json!({
        "body": "Body.\n",
        "labels": [{ "name": "status:in-progress" }],
        "comments": [],
    })
}

fn rekeyed(directory: &std::path::Path) -> (Store, u64) {
    let mut store = store_in(directory);
    let provisional = store.allocate_provisional(&repo()).expect("allocates");
    store
        .import(&repo(), provisional, &issue(), "2026-01-01T00:10Z")
        .expect("held");
    store.rekey(&repo(), provisional, 55, None).expect("rekeys");
    (store, provisional)
}

fn pointer(root: &std::path::Path, run_id: &str, issue: u64, worktree: Option<&std::path::Path>) {
    let mut run = session::Run::new(run_id.to_owned());
    run.issue = Some(crate::identity::Identity::Number(issue));
    run.worktree = worktree.map(std::path::Path::to_path_buf);
    assert!(session::store(root, &run).expect("stores"));
}

#[test]
fn only_a_provisional_number_with_a_known_real_one_is_resolved() {
    let directory = tempfile::tempdir().expect("a directory");
    let (store, provisional) = rekeyed(directory.path());
    let waiting = {
        let mut other = store_in(directory.path());
        other.allocate_provisional(&repo()).expect("allocates")
    };
    assert_eq!(resolve_in(&store, provisional).expect("resolves"), 55);
    assert_eq!(resolve_in(&store, waiting).expect("resolves"), waiting);
    assert_eq!(resolve_in(&store, 1078).expect("resolves"), 1078);
    assert_eq!(
        resolve_in(&store, PROVISIONAL_BASE + 999).expect("resolves"),
        PROVISIONAL_BASE + 999
    );
}

#[test]
fn a_session_pointer_naming_the_provisional_number_is_rewritten_once() {
    let directory = tempfile::tempdir().expect("a directory");
    let root = directory.path().join("runs");
    let (store, provisional) = rekeyed(directory.path());
    pointer(&root, "claude-aaaa", provisional, None);
    pointer(&root, "claude-bbbb", 77, None);

    assert_eq!(rekey_files(&store, &root).expect("rewrites"), 1);
    assert_eq!(session::load(&root, "claude-aaaa").issue_number(), Some(55));
    assert_eq!(session::load(&root, "claude-bbbb").issue_number(), Some(77));
    assert_eq!(
        rekey_files(&store, &root).expect("rewrites"),
        0,
        "the second pass found nothing left to do"
    );
}

#[test]
fn a_pointer_whose_provisional_number_has_no_real_one_yet_is_left_alone() {
    let directory = tempfile::tempdir().expect("a directory");
    let root = directory.path().join("runs");
    let mut store = store_in(directory.path());
    let waiting = store.allocate_provisional(&repo()).expect("allocates");
    pointer(&root, "claude-aaaa", waiting, None);
    assert_eq!(rekey_files(&store, &root).expect("rewrites"), 0);
    assert_eq!(
        session::load(&root, "claude-aaaa").issue_number(),
        Some(waiting)
    );
}

#[test]
fn the_ownership_file_of_the_pointed_worktree_follows_the_issue() {
    let directory = tempfile::tempdir().expect("a directory");
    let root = directory.path().join("runs");
    let worktree = directory.path().join("checkout");
    std::fs::create_dir_all(&worktree).expect("a directory");
    crate::transport::run(
        &["git", "init", "--quiet", "-b", "main"],
        Some(&worktree),
        crate::transport::How::write(),
    )
    .expect("a repository");
    let (store, provisional) = rekeyed(directory.path());
    pointer(&root, "claude-aaaa", provisional, Some(&worktree));
    let file = crate::transport::branch::ownership_path(&worktree).expect("a path");
    std::fs::write(
        &file,
        serde_json::json!({
            "run_id": "claude-aaaa", "issue": provisional, "branch": "fix/x",
            "worktree": worktree.display().to_string(), "claimed_at": "2026-01-01T00:00Z"
        })
        .to_string(),
    )
    .expect("an ownership record");

    rekey_files(&store, &root).expect("rewrites");

    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("reads")).expect("json");
    assert_eq!(record["issue"], 55);
    assert_eq!(record["branch"], "fix/x", "no other field moved");
    assert_eq!(record["run_id"], "claude-aaaa");
    assert_eq!(rekey_files(&store, &root).expect("rewrites"), 0);
}
