use serde_json::{Value, json};

use super::*;
use crate::harness::hook::{Dialect, Event, Input, run_as};
use crate::identity::Identity;

const SESSION: &str = "abcd1234";

struct Fan {
    repo: tempfile::TempDir,
    _trees: tempfile::TempDir,
    _root: tempfile::TempDir,
    context: GateContext,
    parent: String,
    a: session::Run,
    b: session::Run,
}

fn child(state_root: &Path, repo: &Path, trees: &Path, parent: &str, issue: u64) -> session::Run {
    let branch = format!("issue-{issue}");
    let id = session::child_run_id(parent, &Identity::Number(issue), &branch);
    let worktree = trees.join(&branch);
    git(
        repo,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            worktree.to_str().expect("a path"),
        ],
    );
    write_marker(&worktree, &id, issue, &branch);
    let mut run = session::Run::new(id);
    run.issue = Some(Identity::Number(issue));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    run.worktree = Some(crate::paths::placed(&worktree).expect("a placed worktree"));
    run.branch = Some(branch);
    run.base = Some("main".to_owned());
    run.assisted = true;
    run.parent = Some(parent.to_owned());
    session::store(state_root, &run).expect("the pointer writes");
    run
}

fn fan() -> Fan {
    let rig = new_rig();
    git(
        rig.repo.path(),
        &["remote", "add", "origin", "https://example.invalid/o/r.git"],
    );
    let parent = session::run_id("claude", SESSION);
    let trees = tempfile::tempdir().expect("a directory for worktrees");
    let mut context = rig.context.clone();
    context.repo_dir = rig.repo.path().to_path_buf();
    context.issue_requirement = crate::config::IssueRequirement::Watch;
    let a = child(
        &context.state_root,
        rig.repo.path(),
        trees.path(),
        &parent,
        11,
    );
    let b = child(
        &context.state_root,
        rig.repo.path(),
        trees.path(),
        &parent,
        12,
    );
    Fan {
        repo: rig.repo,
        _trees: trees,
        _root: rig.root,
        context,
        parent,
        a,
        b,
    }
}

fn input(tool: &str, tool_input: Value) -> Input {
    Input {
        session_id: SESSION.to_owned(),
        agent_type: Some("developer".to_owned()),
        tool_name: tool.to_owned(),
        tool_input,
        ..Input::default()
    }
}

fn pre_tool(context: &GateContext, input: &Input) -> Value {
    run_as(
        Dialect::ClaudeCode,
        Some("claude-code"),
        Event::PreToolUse,
        input,
        Some(context),
    )
}

fn bash(context: &GateContext, command: &str) -> Value {
    pre_tool(context, &input("Bash", json!({ "command": command })))
}

fn edit(context: &GateContext, target: &Path) -> Value {
    pre_tool(
        context,
        &input(
            "Edit",
            json!({ "file_path": target.display().to_string(), "old_string": "a", "new_string": "b" }),
        ),
    )
}

fn worktree(run: &session::Run) -> &Path {
    run.worktree.as_deref().expect("a worktree")
}

fn verdict(answer: &Value) -> String {
    answer["hookSpecificOutput"]["permissionDecision"]
        .as_str()
        .unwrap_or("none")
        .to_owned()
}

fn reason(answer: &Value) -> String {
    answer["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

// Allowed calls recorded under anything but the session's own run.
fn allowed_under(context: &GateContext) -> Vec<String> {
    let parent = session::run_id("claude", SESSION);
    session::decision_records(&context.state_root)
        .iter()
        .filter(|record| record["verdict"] == "allow")
        .filter_map(|record| record["run_id"].as_str().map(str::to_owned))
        .filter(|run_id| *run_id != parent)
        .collect()
}

// What the parent's policy answers for this call when no child exists.
fn without_children(fan: &Fan, call: impl Fn(&GateContext) -> Value) -> Value {
    let empty = tempfile::tempdir().expect("an empty state root");
    let mut context = fan.context.clone();
    context.state_root = empty.path().join("state");
    call(&context)
}

#[test]
fn an_edit_from_the_base_checkout_is_decided_under_the_claim_it_lands_in() {
    let fan = fan();
    let answer = edit(&fan.context, &worktree(&fan.b).join("x.txt"));
    assert_eq!(verdict(&answer), "allow", "{answer}");
    assert_eq!(allowed_under(&fan.context), vec![fan.b.run_id.clone()]);
}

#[test]
fn an_edit_into_no_child_is_decided_by_the_parents_policy() {
    let fan = fan();
    let target = fan.repo.path().join("x.txt");
    let answer = edit(&fan.context, &target);
    let policy = without_children(&fan, |context| edit(context, &target));
    assert_ne!(verdict(&answer), "allow", "{answer}");
    assert_eq!(verdict(&answer), verdict(&policy), "{answer} / {policy}");
}

#[test]
fn an_edit_where_two_children_cover_the_landing_is_decided_by_the_parents_policy() {
    let fan = fan();
    let nested = worktree(&fan.a).join("nested");
    std::fs::create_dir_all(&nested).expect("a nested directory");
    session::update(&fan.context.state_root, &fan.b.run_id, |run| {
        run.worktree = Some(nested.clone());
    });
    let target = nested.join("x.txt");
    let answer = edit(&fan.context, &target);
    let policy = without_children(&fan, |context| edit(context, &target));
    assert_ne!(verdict(&answer), "allow", "{answer}");
    assert_eq!(verdict(&answer), verdict(&policy), "{answer} / {policy}");
}

#[test]
fn a_single_git_dash_c_is_decided_under_the_child_it_names() {
    let fan = fan();
    let command = format!(
        "git -C {} commit -m \"fix: one; two\"",
        worktree(&fan.a).display()
    );
    let answer = bash(&fan.context, &command);
    assert_eq!(verdict(&answer), "allow", "{answer}");
    assert_eq!(allowed_under(&fan.context), vec![fan.a.run_id.clone()]);
}

#[test]
fn two_git_dash_c_in_one_call_are_decided_under_neither_child() {
    let fan = fan();
    for separator in [";", "&&", "|", "\n"] {
        let command = format!(
            "git -C {} commit -m x {separator} git -C {} commit -m y",
            worktree(&fan.a).display(),
            worktree(&fan.b).display()
        );
        let answer = bash(&fan.context, &command);
        let policy = without_children(&fan, |context| bash(context, &command));
        assert_eq!(verdict(&answer), verdict(&policy), "{command}: {answer}");
        assert!(allowed_under(&fan.context).is_empty(), "{command}");
    }
}

#[test]
fn a_git_dash_c_hiding_code_or_a_second_directory_is_decided_under_no_child() {
    let fan = fan();
    let a = worktree(&fan.a).display().to_string();
    let b = worktree(&fan.b).display().to_string();
    for command in [
        format!("git -C {a} commit -m \"$(sed -i x {b}/y)\""),
        format!("git -C {a} commit -m `sed -i x {b}/y`"),
        format!("git -C {a} -C {b} commit -m x"),
        format!("git -C {a} --work-tree={b} commit -m x"),
        format!("git -C {a} -c core.hooksPath=/tmp/h commit -m x"),
        format!("git -C {a} commit -m x > {b}/y"),
        format!("cd {a} && git commit -m x"),
    ] {
        let answer = bash(&fan.context, &command);
        let policy = without_children(&fan, |context| bash(context, &command));
        assert_eq!(verdict(&answer), verdict(&policy), "{command}: {answer}");
        assert!(allowed_under(&fan.context).is_empty(), "{command}");
    }
}

#[test]
fn a_shell_write_inside_one_child_stays_there_unless_it_changes_directory() {
    let fan = fan();
    let mut inside = fan.context.clone();
    inside.repo_dir = worktree(&fan.a).to_path_buf();
    let answer = bash(&inside, "git commit -m x");
    assert_eq!(verdict(&answer), "allow", "{answer}");

    assert_eq!(allowed_under(&inside), vec![fan.a.run_id.clone()]);

    let elsewhere = format!("cd {} && sed -i s/a/b/ x", worktree(&fan.b).display());
    let answer = bash(&inside, &elsewhere);
    assert_eq!(allowed_under(&inside), vec![fan.a.run_id], "{answer}");
}

#[test]
fn git_dash_c_from_inside_one_child_is_decided_under_the_one_it_names() {
    let fan = fan();
    let mut inside = fan.context.clone();
    inside.repo_dir = worktree(&fan.a).to_path_buf();
    let command = format!("git -C {} commit -m x", worktree(&fan.b).display());
    let answer = bash(&inside, &command);
    assert_eq!(verdict(&answer), "allow", "{answer}");
    assert_eq!(allowed_under(&inside), vec![fan.b.run_id]);
}

#[test]
fn a_stale_child_is_refused_rather_than_stepped_over() {
    let fan = fan();
    write_marker(worktree(&fan.b), "claude-ffff0000", 12, "issue-12");
    let answer = edit(&fan.context, &worktree(&fan.b).join("x.txt"));
    assert_eq!(verdict(&answer), "deny", "{answer}");
    assert!(reason(&answer).contains("assisted-claim-stale"), "{answer}");
}

#[test]
fn a_merge_of_the_base_inside_a_childs_worktree_is_admitted() {
    let fan = fan();
    let a = worktree(&fan.a).display().to_string();
    for command in [
        format!("git -C {a} merge main"),
        format!("git -C {a} merge --no-edit origin/main"),
        format!("git -C {a} merge --continue"),
        format!("git -C {a} merge --abort"),
    ] {
        let answer = bash(&fan.context, &command);
        assert_eq!(verdict(&answer), "allow", "{command}: {answer}");
    }
}

#[test]
fn a_merge_of_anything_but_the_base_is_still_refused() {
    let fan = fan();
    let a = worktree(&fan.a).display().to_string();
    for command in [
        format!("git -C {a} merge issue-12"),
        format!("git -C {a} merge"),
        format!("git -C {a} merge main issue-12"),
        format!("git -C {a} merge --ff-only main"),
    ] {
        let answer = bash(&fan.context, &command);
        assert_eq!(verdict(&answer), "deny", "{command}: {answer}");
        assert!(
            reason(&answer).contains("assisted-does-not-deliver"),
            "{command}: {answer}"
        );
    }
}

#[test]
fn a_merge_is_refused_for_a_pointer_that_recorded_no_base() {
    let fan = fan();
    session::update(&fan.context.state_root, &fan.a.run_id, |run| {
        run.base = None;
    });
    let command = format!("git -C {} merge main", worktree(&fan.a).display());
    let answer = bash(&fan.context, &command);
    assert!(
        reason(&answer).contains("assisted-does-not-deliver"),
        "{answer}"
    );
}

fn single_claim_decides(rig: &mut Rig, command: &str) -> Decision {
    let (action, how, observed) = crate::harness::classify_observing(
        "Bash",
        &json!({ "command": command }),
        &rig.context.boundaries,
    );
    let observed = Observed {
        command: Some(observed),
        ..Observed::default()
    };
    crate::harness::gate_observing(&rig.context, &mut rig.run, &action, how, &observed)
}

#[test]
fn a_single_assisted_claim_merges_its_base_from_its_own_worktree_only() {
    let mut rig = new_rig();
    rig.run.base = Some("main".to_owned());
    let answer = single_claim_decides(&mut rig, "git merge main");
    assert!(matches!(answer, Decision::Allow(_)), "{answer:?}");

    let elsewhere = rig.repo.path().display().to_string();
    let answer = single_claim_decides(&mut rig, &format!("git -C {elsewhere} merge main"));
    assert_eq!(denied_as(&answer), "assisted-does-not-deliver");
}

#[test]
fn session_end_leaves_no_child_behind() {
    let fan = fan();
    let answer = run_as(
        Dialect::ClaudeCode,
        Some("claude-code"),
        Event::SessionEnd,
        &input("", json!({})),
        Some(&fan.context),
    );
    assert_eq!(answer, json!({}));
    assert!(
        session::children(&fan.context.state_root, &fan.parent).is_empty(),
        "a child outlived its session"
    );
}

#[test]
fn session_start_names_each_child() {
    let fan = fan();
    let answer = run_as(
        Dialect::ClaudeCode,
        Some("claude-code"),
        Event::SessionStart,
        &input("", json!({})),
        Some(&fan.context),
    );
    let text = answer.to_string();
    for child in [&fan.a, &fan.b] {
        assert!(text.contains(&child.run_id), "{text}");
    }
}

#[test]
fn another_sessions_children_are_not_this_ones() {
    let fan = fan();
    let target = worktree(&fan.b).join("x.txt");
    let answer = pre_tool(
        &fan.context,
        &Input {
            session_id: "ffff0000".to_owned(),
            ..input("Edit", json!({ "file_path": target.display().to_string() }))
        },
    );
    assert!(allowed_under(&fan.context).is_empty(), "{answer}");
}
