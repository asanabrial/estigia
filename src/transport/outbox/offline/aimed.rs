use super::*;

const DEST: &str = "dest/repo";

fn dest_repo() -> Repo {
    Repo {
        host: "github.com".to_owned(),
        owner: "dest".to_owned(),
        name: "repo".to_owned(),
    }
}

fn created_at(rig: &Rig, clock: fn() -> Option<u16>) -> Result<serde_json::Value, Failure> {
    let body = rig.path("body.md");
    let flags = [
        "--identity",
        "a-probe",
        "--title",
        "A probe",
        "--body-file",
        body.as_str(),
        "--domain",
        "transport",
        "--priority",
        "p2",
    ];
    rig.call_at(clock, "create", &flags)
}

fn aimed_at(rig: &mut Rig, window: &str) {
    rig.context.repo = Some(DEST.to_owned());
    rig.context.config = vec![(
        "Git operation window".to_owned(),
        window.to_owned(),
        Source::Written,
    )];
}

#[test]
fn a_create_aimed_at_another_repository_is_judged_by_that_repositorys_window() {
    let mut rig = rig();
    aimed_at(&mut rig, "disabled");
    let home = tempfile::tempdir().expect("a home");
    let _destination = destination(home.path(), DEST, "enabled mon-fri 08:00-17:00 inverted");

    with_home(home.path(), || {
        let answer =
            created_at(&rig, noon).expect("a create inside the destination's closed window");

        let provisional = answer["issue"].as_u64().expect("a number");
        assert!(outbox::is_provisional(provisional), "{answer}");
        assert_eq!(answer["provisional"], true, "{answer}");
        assert_eq!(answer["url"], serde_json::Value::Null, "{answer}");
        assert_eq!(
            rig.queue_for(&dest_repo()),
            vec![
                (OutboxKind::Create, provisional),
                (OutboxKind::Reconverge, provisional)
            ],
            "{answer}"
        );
        assert_eq!(rig.queue(), Vec::new(), "the caller's own queue was used");
        assert_eq!(
            rig.writes(),
            Vec::<String>::new(),
            "a closed destination's write reached GitHub"
        );
    });
}

#[test]
fn a_flush_readback_reports_the_window_of_the_repository_it_is_aimed_at() {
    let mut rig = rig();
    aimed_at(&mut rig, "disabled");
    let home = tempfile::tempdir().expect("a home");
    let _destination = destination(home.path(), DEST, "enabled mon-fri 08:00-17:00 inverted");

    with_home(home.path(), || {
        let provisional = created_at(&rig, noon).expect("a create")["issue"]
            .as_u64()
            .expect("a number");

        let report = rig
            .call_at(noon, "flush", &["--read-only"])
            .expect("a readback");

        assert_eq!(report["window"], "closed", "{report}");
        assert_eq!(report["remaining"], 2, "{report}");
        assert_eq!(
            report["writes"][0]["issue"].as_u64(),
            Some(provisional),
            "{report}"
        );
    });
}

#[test]
fn a_closed_caller_filing_into_an_open_repository_is_not_deferred() {
    let mut rig = rig();
    aimed_at(&mut rig, "enabled 09:00-17:00");
    let home = tempfile::tempdir().expect("a home");
    let _destination = destination(home.path(), DEST, "disabled");
    rig.script = crate::test_env::answers(&[
        serde_json::json!({ "matches": "issues?state=open", "stdout": "[[]]", "status": 0 }),
        serde_json::json!({
            "matches": "issue create",
            "stdout": "https://github.com/dest/repo/issues/41",
            "status": 0,
        }),
    ]);

    with_home(home.path(), || {
        let answer = created_at(&rig, night).expect("a create");

        assert_ne!(answer["provisional"], true, "{answer}");
        assert!(
            rig.writes()
                .iter()
                .any(|line| line.starts_with("issue create")),
            "an open destination's write was deferred: {:?}",
            rig.writes()
        );
        assert_eq!(rig.queue_for(&dest_repo()), Vec::new(), "{answer}");
    });
}

#[test]
fn a_repository_with_no_readable_rows_closes_rather_than_inheriting_the_caller() {
    let mut rig = rig();
    aimed_at(&mut rig, "disabled");
    rig.context.repo = Some("nobody/nothing".to_owned());
    let home = tempfile::tempdir().expect("a home");

    with_home(home.path(), || {
        let answer = created_at(&rig, noon).expect("a create");

        let provisional = answer["issue"].as_u64().expect("a number");
        assert!(outbox::is_provisional(provisional), "{answer}");
        assert_eq!(answer["provisional"], true, "{answer}");
        assert_eq!(rig.writes(), Vec::<String>::new());

        let denied = rig
            .call_at(noon, "flush", &[])
            .expect_err("a flush aimed at a repository with no readable rows");
        let envelope = denied.envelope();
        assert_eq!(
            envelope["reason"], "git-operation-window-denied",
            "{envelope}"
        );
        assert!(
            !envelope["message"]
                .as_str()
                .unwrap_or_default()
                .contains("08:00"),
            "the denial claimed a range the repository does not have: {envelope}"
        );
    });
}
