use super::*;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;

const RUN_ID: &str = "claude-abcd1234";

struct World {
    answers: String,
    labels: std::path::PathBuf,
}

fn ask(
    rig: &TrackerRig,
    world: &World,
    tool: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments }
    })
    .to_string();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let mut child = tracker_command(home, repo, bin, &world.answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LABELS", &world.labels)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    response["result"].clone()
}

fn answer(result: &serde_json::Value) -> serde_json::Value {
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert_ne!(result["isError"], true, "refused: {text}");
    serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"))
}

fn world(
    rig: &TrackerRig,
    path: &str,
    state: &str,
    lane: serde_json::Value,
    reviewer: &str,
    publishing: bool,
) -> World {
    let repo = rig.repo.path();
    std::fs::create_dir_all(repo.join("src")).expect("a source directory");
    let (base, head) = commit_a_changed_line(repo, path);
    if publishing {
        let origin = tempfile::tempdir().expect("a bare origin").keep();
        assert!(
            Command::new("git")
                .args(["init", "--bare", "--quiet"])
                .arg(&origin)
                .output()
                .is_ok_and(|output| output.status.success())
        );
        let url = origin.display().to_string();
        checked_git(
            repo,
            &["remote", "add", "origin", "git@github.com:fixture/repo.git"],
        );
        let file = estigia::skill::repository_config_path(repo);
        std::fs::create_dir_all(file.parent().expect("a parent")).expect("the rows directory");
        std::fs::write(
            &file,
            "| Setting | Value |\n|---|---|\n| Tracker | github fixture/repo |\n| Repository host | github fixture/repo |\n| Git operation window | disabled |\n",
        )
        .expect("the rows");
        estigia::skill::remember_repository(rig.home.path(), repo);
        checked_git(
            repo,
            &[
                "config",
                &format!("url.{url}.insteadOf"),
                "git@github.com:fixture/repo.git",
            ],
        );
        checked_git(repo, &["branch", "-M", "fix/12-fold"]);
        checked_git(
            repo,
            &["push", "-q", "origin", &format!("{base}:refs/heads/main")],
        );
        checked_git(repo, &["push", "-q", "-u", "origin", "fix/12-fold"]);
        checked_git(repo, &["fetch", "-q", "origin"]);
    } else {
        let url = "https://github.com/fixture/repo.git";
        checked_git(repo, &["remote", "add", "origin", url]);
    }
    let payload = estigia::transport::target::review_payload(repo, &base, &head)
        .expect("the change has a payload");
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id, "createdAt": at, "viewerDidAuthor": true,
            "includesCreatedEdit": false, "body": body,
        })
    };
    let mut comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", RUN_ID),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", RUN_ID),
                    ("epoch", &"2".repeat(32)),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &"2".repeat(64)),
                    ("payload-digest", &payload.digest),
                ],
            ),
        ),
    ];
    if publishing {
        comments.extend(republish_cause_roles(
            7,
            RUN_ID,
            &head,
            &base,
            &"2".repeat(64),
            &"2".repeat(32),
            &[
                "review-reliability",
                "review-risk",
                "review-readback",
                "review-readability",
            ],
        ));
    }
    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head,
        "baseRefOid": base,
        "state": "OPEN",
        "isDraft": true,
    });
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "--json labels", "labels": true, "status": 0 },
        { "matches": "pr view", "stdout": pr.to_string(), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 },
        { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": format!("status:{state}")}],
                "body": "The change keeps its one line.",
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        { "matches": "check-runs", "stdout": lane.to_string(), "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let program = rig.bin.path().join("claude");
    std::fs::write(&program, reviewer).expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");

    let runs = rig.home.path().join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(RUN_ID.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some(state.to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(estigia::harness::session::store(&runs, &run).expect("the pointer is writable"));

    let labels = rig.home.path().join("labels.json");
    std::fs::write(&labels, format!("[\"status:{state}\"]")).expect("the label store");
    let world = World { answers, labels };
    let imported = ask(
        rig,
        &world,
        "verify_claim",
        serde_json::json!({"issue": 12, "run_id": RUN_ID, "expect_state": state}),
    );
    answer(&imported);
    world
}

fn instant_reviewer(dir: &std::path::Path) -> String {
    let said = serde_json::json!({
        "type": "result", "subtype": "success", "is_error": false,
        "result": serde_json::json!({"findings": []}).to_string(),
    });
    std::fs::write(dir.join("said.json"), said.to_string()).expect("the canned answer");
    format!(
        "#!/bin/sh\n/bin/cat > /dev/null\n/bin/cat '{}'\n",
        dir.join("said.json").display()
    )
}

fn gated_reviewer(dir: &std::path::Path) -> String {
    let instant = instant_reviewer(dir);
    format!(
        "#!/bin/sh\nn=0\nwhile [ ! -e '{}' ] && [ $n -lt 300 ]; do /bin/sleep 0.1; n=$((n+1)); done\n{}",
        dir.join("release").display(),
        instant.trim_start_matches("#!/bin/sh\n"),
    )
}

fn offered(rig: &TrackerRig, world: &World) -> Vec<String> {
    let status = answer(&ask(
        rig,
        world,
        "review_status",
        serde_json::json!({"issue": 12}),
    ));
    status["next"]["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|input| input["name"].as_str().map(str::to_owned))
        .collect()
}

fn verdict_arguments(rig: &TrackerRig, world: &World) -> serde_json::Value {
    let status = answer(&ask(
        rig,
        world,
        "review_status",
        serde_json::json!({"issue": 12}),
    ));
    let bound = &status["next"]["arguments"];
    serde_json::json!({
        "issue": 12, "run_id": RUN_ID, "epoch": bound["epoch"], "pr": bound["pr"],
        "head": bound["head"], "base": bound["base"], "digest": bound["digest"],
    })
}

#[test]
fn collect_records_the_verdict_only_when_the_review_names_it_next() {
    let rig = tracker_rig();
    let reviewer = tempfile::tempdir().expect("a reviewer directory");
    let lane = serde_json::json!({"total_count": 0, "check_runs": []});
    let world = world(
        &rig,
        "src/auth.rs",
        "review",
        lane,
        &instant_reviewer(reviewer.path()),
        false,
    );
    let names = offered(&rig, &world);
    assert_eq!(
        names.len(),
        4,
        "a high tier offers its four lenses: {names:?}"
    );

    answer(&ask(
        &rig,
        &world,
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": RUN_ID, "names": names}),
    ));
    let partial = answer(&ask(
        &rig,
        &world,
        "collect_review_input",
        serde_json::json!({
            "issue": 12, "run_id": RUN_ID, "names": &names[..1],
            "wait_seconds": 30, "record_verdict": true,
        }),
    ));
    assert!(
        partial.get("verdict").is_none(),
        "a verdict before every lens: {partial}"
    );
    assert_eq!(
        partial["review_status"]["next"]["kind"], "collect",
        "{partial}"
    );

    let rest = answer(&ask(
        &rig,
        &world,
        "collect_review_input",
        serde_json::json!({
            "issue": 12, "run_id": RUN_ID, "names": &names[1..],
            "wait_seconds": 30, "record_verdict": true,
        }),
    ));
    assert_eq!(rest["verdict"]["outcome"], "approved", "{rest}");
    assert_eq!(rest["verdict"]["reused_existing_verdict"], false, "{rest}");

    let again = answer(&ask(
        &rig,
        &world,
        "record_computed_verdict",
        verdict_arguments(&rig, &world),
    ));
    assert_eq!(again["reused_existing_verdict"], true, "{again}");
}

#[test]
fn collect_with_a_slot_still_running_records_no_verdict() {
    let rig = tracker_rig();
    let reviewer = tempfile::tempdir().expect("a reviewer directory");
    let lane = serde_json::json!({"total_count": 0, "check_runs": []});
    let world = world(
        &rig,
        "src/foo.rs",
        "review",
        lane,
        &gated_reviewer(reviewer.path()),
        false,
    );
    let names = offered(&rig, &world);
    answer(&ask(
        &rig,
        &world,
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": RUN_ID, "names": names}),
    ));
    let running = answer(&ask(
        &rig,
        &world,
        "collect_review_input",
        serde_json::json!({"issue": 12, "run_id": RUN_ID, "names": names, "record_verdict": true}),
    ));
    assert_eq!(running["verdict_skipped"], "capture-running", "{running}");
    assert!(running.get("verdict").is_none(), "{running}");
    std::fs::write(reviewer.path().join("release"), "").expect("the reviewer is released");
}

#[test]
fn publication_with_start_review_moves_the_issue_and_launches_every_offered_slot() {
    let rig = tracker_rig();
    let reviewer = tempfile::tempdir().expect("a reviewer directory");
    let lane = serde_json::json!({"total_count": 1, "check_runs": [{
        "name": "ci", "status": "completed", "conclusion": "success",
    }]});
    let world = published_world(&rig, "src/auth.rs", lane, &gated_reviewer(reviewer.path()));
    let published = answer(&ask(
        &rig,
        &world,
        "publish_review",
        publish_arguments(&rig, &world),
    ));
    let start = &published["review_start"];
    assert_eq!(start["transitioned"], true, "{published}");
    assert_eq!(
        start["launched"].as_array().map(Vec::len),
        Some(4),
        "{published}"
    );
    assert!(start.get("launch_skipped").is_none(), "{published}");
    let pointer = std::fs::read_to_string(
        rig.home
            .path()
            .join(".estigia")
            .join("runs")
            .join(format!("{RUN_ID}.json")),
    )
    .expect("the pointer is on disk");
    assert!(
        pointer.contains("\"state\": \"review\"") || pointer.contains("\"state\":\"review\""),
        "{pointer}"
    );
    std::fs::write(reviewer.path().join("release"), "").expect("the reviewer is released");
}

#[test]
fn a_red_lane_launches_nothing_and_the_answer_says_why() {
    let rig = tracker_rig();
    let reviewer = tempfile::tempdir().expect("a reviewer directory");
    let lane = serde_json::json!({"total_count": 1, "check_runs": [{
        "name": "ci", "status": "completed", "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/1/job/2",
        "output": {"title": "failed", "summary": "a step failed"},
        "check_suite": {"id": 5}, "app": {"slug": "github-actions"},
    }]});
    let world = published_world(&rig, "src/auth.rs", lane, &gated_reviewer(reviewer.path()));
    let published = answer(&ask(
        &rig,
        &world,
        "publish_review",
        publish_arguments(&rig, &world),
    ));
    let start = &published["review_start"];
    assert_eq!(start["transitioned"], true, "{published}");
    assert_eq!(
        start["launch_skipped"]["cause"], "publication-lane-red",
        "{published}"
    );
    assert!(start.get("launched").is_none(), "{published}");
}

fn published_world(rig: &TrackerRig, path: &str, lane: serde_json::Value, reviewer: &str) -> World {
    world(rig, path, "in-progress", lane, reviewer, true)
}

fn publish_arguments(rig: &TrackerRig, _world: &World) -> serde_json::Value {
    let body = rig.home.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");
    serde_json::json!({
        "issue": 12, "run_id": RUN_ID, "branch": "fix/12-fold", "base": "main",
        "pr_title": "Something", "pr_body_file": body.display().to_string(),
        "worktree": rig.repo.path().display().to_string(), "start_review": true,
    })
}

#[test]
fn an_unreadable_lane_launches_nothing_and_the_answer_says_why() {
    let rig = tracker_rig();
    let reviewer = tempfile::tempdir().expect("a reviewer directory");
    let lane = serde_json::json!({"total_count": 0, "check_runs": []});
    let world = published_world(&rig, "src/auth.rs", lane, &gated_reviewer(reviewer.path()));
    let published = answer(&ask(
        &rig,
        &world,
        "publish_review",
        publish_arguments(&rig, &world),
    ));
    let start = &published["review_start"];
    assert_eq!(start["transitioned"], true, "{published}");
    assert_eq!(
        start["launch_skipped"]["cause"], "publication-lane-unreadable",
        "{published}"
    );
    assert!(start.get("launched").is_none(), "{published}");
}
