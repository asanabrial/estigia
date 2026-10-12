use super::*;

fn context(root: &std::path::Path, repo: &std::path::Path, template: &str) -> Context {
    Context {
        skill_dir: root.join("skill"),
        repo_dir: repo.to_path_buf(),
        config: vec![(
            "Worktree location".to_owned(),
            template.to_owned(),
            crate::transport::Source::Written,
        )],
        repo: Some("owner/repo".to_owned()),
    }
}

struct Receipt {
    epoch: String,
    head: String,
    base: String,
    digest: String,
}

fn receipt() -> Receipt {
    Receipt {
        epoch: "ab".repeat(16),
        head: "12".repeat(20),
        base: "34".repeat(20),
        digest: "56".repeat(32),
    }
}

fn panel<'a>(receipt: &'a Receipt, count: u64) -> Panel<'a> {
    Panel {
        issue: crate::identity::Identity::Number(88),
        run_id: "claude-judges0a1b2c3d",
        epoch: &receipt.epoch,
        pr: 7,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        count,
        worktree_root: None,
    }
}

fn panel_template(root: &std::path::Path) -> String {
    root.join("panels")
        .join("<repo>")
        .join("<branch>")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_panel_gets_one_distinct_directory_per_judge_outside_the_claimed_checkout() {
    let repo = crate::test_env::repository_with_commit();
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let template = panel_template(outside.path());
    let context = context(outside.path(), repo.path(), &template);
    let receipt = receipt();
    let answer = judge_workspaces(
        &context,
        &panel(&receipt, 3),
        context.get("worktree location"),
    )
    .expect("the panel mints");

    let listed = answer["directories"]
        .as_array()
        .expect("a list of directories");
    assert_eq!(
        listed.len(),
        3,
        "the panel did not get one directory per judge"
    );
    let directories: Vec<PathBuf> = listed
        .iter()
        .map(|value| PathBuf::from(value.as_str().expect("a path")))
        .collect();

    for directory in &directories {
        assert!(
            directory.is_dir(),
            "{} was reported and is not there",
            directory.display()
        );
        assert!(
            !directory.join(".git").exists(),
            "{} is a checkout, not a bare directory",
            directory.display()
        );
        assert!(
            !super::inside_the_claimed_checkout(repo.path(), directory),
            "{} is inside the claimed checkout",
            directory.display()
        );
    }

    let canonical: Vec<PathBuf> = directories
        .iter()
        .map(|directory| std::fs::canonicalize(directory).expect("it was just read back"))
        .collect();
    let distinct: std::collections::BTreeSet<String> = canonical
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    assert_eq!(distinct.len(), 3, "two judges were handed one directory");
    for left in &canonical {
        for right in &canonical {
            if left != right {
                assert!(
                    !left.starts_with(right),
                    "{} is nested inside {}",
                    left.display(),
                    right.display()
                );
            }
        }
    }
}

#[test]
fn a_panel_that_cannot_be_given_one_directory_each_refuses_and_keeps_none() {
    let repo = crate::test_env::repository_with_commit();
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let template = panel_template(outside.path());
    let context = context(outside.path(), repo.path(), &template);
    let receipt = receipt();
    let root = panel_root(
        &template,
        "repo",
        "claude-judges0a1b2c3d",
        &crate::identity::Identity::Number(88),
        &receipt.head,
    )
    .expect("the panel root composes");
    std::fs::create_dir_all(&root).expect("the panel root exists");
    std::fs::write(root.join("judge-2"), "not a directory").expect("a file in the way");

    let failure = judge_workspaces(
        &context,
        &panel(&receipt, 3),
        context.get("worktree location"),
    )
    .expect_err("a short panel is not a panel");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "judge-workspaces-uncreatable");
    assert_eq!(envelope["requested"], 3);
    assert_eq!(envelope["creatable"], 2);
    assert!(
        !root.join("judge-1").exists(),
        "a half-built panel was left behind"
    );
    assert!(
        !root.join("judge-3").exists(),
        "a half-built panel was left behind"
    );
    assert!(
        root.join("judge-2").is_file(),
        "the file in the way was removed"
    );
}

#[test]
fn a_panel_root_inside_the_claimed_checkout_refuses_before_creating_anything() {
    let repo = crate::test_env::repository_with_commit();
    let template = repo.path().join("panels").to_string_lossy().into_owned();
    let context = context(repo.path(), repo.path(), &template);
    let receipt = receipt();
    let root = panel_root(
        &template,
        "repo",
        "claude-judges0a1b2c3d",
        &crate::identity::Identity::Number(88),
        &receipt.head,
    )
    .expect("the panel root composes");

    let failure = judge_workspaces(
        &context,
        &panel(&receipt, 2),
        context.get("worktree location"),
    )
    .expect_err("a directory inside the checkout is refused");
    assert_eq!(
        failure.envelope()["reason"],
        "judge-workspaces-inside-the-claimed-checkout"
    );
    assert!(!root.exists(), "the refused panel root was created anyway");
}

#[test]
fn two_runs_mint_two_panel_roots() {
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let template = panel_template(outside.path());
    let receipt = receipt();
    let issue = crate::identity::Identity::Number(88);
    let one = panel_root(
        &template,
        "repo",
        "claude-one0a1b2c3d",
        &issue,
        &receipt.head,
    )
    .expect("the first root");
    let two = panel_root(
        &template,
        "repo",
        "claude-two0a1b2c3d",
        &issue,
        &receipt.head,
    )
    .expect("the second root");
    assert_ne!(one, two, "two runs composed one panel root");
}

#[test]
fn a_panel_of_zero_judges_refuses_before_creating_anything() {
    let repo = crate::test_env::repository_with_commit();
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let template = panel_template(outside.path());
    let context = context(outside.path(), repo.path(), &template);
    let receipt = receipt();
    let root = panel_root(
        &template,
        "repo",
        "claude-judges0a1b2c3d",
        &crate::identity::Identity::Number(88),
        &receipt.head,
    )
    .expect("the panel root composes");

    let failure = judge_workspaces(
        &context,
        &panel(&receipt, 0),
        context.get("worktree location"),
    )
    .expect_err("a panel counts from 1");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "malformed-argument");
    assert_eq!(envelope["argument"], "--count");
    assert_eq!(envelope["value"], "0");
    assert!(!root.exists(), "a refused count created a directory anyway");
}

#[test]
fn a_panel_above_the_judge_ceiling_refuses_before_creating_anything() {
    let repo = crate::test_env::repository_with_commit();
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let template = panel_template(outside.path());
    let context = context(outside.path(), repo.path(), &template);
    let receipt = receipt();
    let root = panel_root(
        &template,
        "repo",
        "claude-judges0a1b2c3d",
        &crate::identity::Identity::Number(88),
        &receipt.head,
    )
    .expect("the panel root composes");

    let failure = judge_workspaces(
        &context,
        &panel(&receipt, MAX_JUDGES + 1),
        context.get("worktree location"),
    )
    .expect_err("a count above the ceiling is refused");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "judge-workspaces-count-too-large");
    assert_eq!(envelope["requested"], MAX_JUDGES + 1);
    assert_eq!(envelope["max"], MAX_JUDGES);
    assert!(!root.exists(), "a refused count created a directory anyway");
}

#[test]
fn an_absent_or_unset_worktree_location_refuses_the_panel() {
    let repo = crate::test_env::repository_with_commit();
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let receipt = receipt();
    for template in ["", "unset"] {
        let context = context(outside.path(), repo.path(), template);
        let failure = judge_workspaces(
            &context,
            &panel(&receipt, 2),
            context.get("worktree location"),
        )
        .expect_err("a panel needs a worktree location");
        assert_eq!(
            failure.envelope()["reason"],
            "no-worktree-location",
            "template {template:?}"
        );
    }
}

#[test]
fn a_relative_worktree_root_refuses_the_panel() {
    let repo = crate::test_env::repository_with_commit();
    let outside = tempfile::tempdir().expect("somewhere to keep panels");
    let template = panel_template(outside.path());
    let context = context(outside.path(), repo.path(), &template);
    let receipt = receipt();
    let mut what = panel(&receipt, 2);
    what.worktree_root = Some("relative/panels");

    let failure = judge_workspaces(&context, &what, context.get("worktree location"))
        .expect_err("a relative override is refused");
    assert_eq!(
        failure.envelope()["reason"],
        "worktree-location-not-absolute"
    );
}
