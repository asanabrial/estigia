use serde_json::{Value, json};

use super::{MAX_LIMIT, one_page};
use crate::harness::session::run_id;

const T: u64 = 1_800_000_000;

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn root(&self) -> std::path::PathBuf {
        self.directory.path().join("state")
    }

    fn ask(&self, arguments: Value) -> Result<Value, crate::outcome::Refusal> {
        one_page(&arguments, &self.root(), Some(T + 1_000)).map(|page| page.answer())
    }

    fn events(&self, arguments: Value) -> Vec<Value> {
        let answer = self.ask(arguments).expect("the feed answers");
        answer["events"].as_array().expect("a list").clone()
    }
}

fn rig() -> Rig {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    drop(crate::transport::store::Store::open(&path).expect("the store opens"));
    let connection = rusqlite::Connection::open(&path).expect("the store opens");
    let pi = run_id("pi", "alpha");
    let claude = run_id("claude", "beta");
    let iso = |offset: u64| {
        format!(
            "strftime('%Y-%m-%dT%H:%M:%SZ', {}, 'unixepoch')",
            T + offset
        )
    };
    let script = format!(
        "INSERT INTO repos (id, host, owner, name) VALUES (1, 'github.com', 'o', 'a'), (2, 'github.com', 'o', 'b');
         INSERT INTO issues (repo_id, number, state, history) VALUES (1, 5, 'review', 'native'), (1, 6, 'ready', 'native'), (2, 7, 'ready', 'native');
         INSERT INTO entries (repo_id, issue, created_at, body, kind, run_id, viewer_did_author, includes_created_edit, source)
           VALUES (1, 5, {e10}, 'SECRET claim body', 'claim', '{pi}', 1, 0, 'local'),
                  (2, 7, {e30}, 'SECRET spend body', 'spend', '{claude}', 1, 0, 'local'),
                  (1, 5, {e5}, 'a plain comment', NULL, NULL, 1, 0, 'local');
         INSERT INTO transitions (repo_id, issue, at, run_id, from_state, to_state)
           VALUES (1, 5, {e20}, '{pi}', 'in-progress', 'review'), (1, 6, {e25}, NULL, NULL, 'ready');
         INSERT INTO messages (sender_run_id, recipient_run_id, body, created_at)
           VALUES ('{claude}', '{pi}', 'SECRET message body', {m40});",
        e5 = iso(5),
        e10 = iso(10),
        e20 = iso(20),
        e25 = iso(25),
        e30 = iso(30),
        m40 = T + 40,
    );
    connection
        .execute_batch(&script)
        .expect("the fixture seeds");
    Rig { directory }
}

fn kinds(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(|event| event["kind"].as_str().expect("a kind").to_owned())
        .collect()
}

#[test]
fn a_transition_is_listed_with_its_time_run_and_both_states() {
    let events = rig().events(json!({"kind": "transition", "issue": 5}));

    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["at"], T + 20);
    assert_eq!(events[0]["run_id"], run_id("pi", "alpha"));
    assert_eq!(events[0]["from"], "in-progress");
    assert_eq!(events[0]["to"], "review");
    assert_eq!(events[0]["repo"], "o/a");
}

#[test]
fn the_three_sources_share_one_chronological_scale() {
    let events = rig().events(json!({}));

    assert_eq!(
        kinds(&events),
        ["claim", "transition", "transition", "spend", "message"]
    );
    let times: Vec<u64> = events.iter().map(|e| e["at"].as_u64().unwrap()).collect();
    assert_eq!(times, [T + 10, T + 20, T + 25, T + 30, T + 40]);
}

#[test]
fn each_filter_narrows_the_feed() {
    let rig = rig();

    assert_eq!(kinds(&rig.events(json!({"repo": "o/b"}))), ["spend"]);
    assert_eq!(
        kinds(&rig.events(json!({"run": run_id("claude", "beta")}))),
        ["spend", "message"]
    );
    assert_eq!(
        kinds(&rig.events(json!({"runtime": "pi"}))),
        ["claim", "transition"]
    );
    assert_eq!(
        kinds(&rig.events(json!({"runtime": "claude"}))),
        ["spend", "message"]
    );
    assert_eq!(kinds(&rig.events(json!({"issue": 6}))), ["transition"]);
    assert_eq!(
        kinds(&rig.events(json!({"since": "2027-01-15T08:00:24Z"}))),
        ["transition", "spend", "message"]
    );
}

#[test]
fn an_unknown_repository_is_refused_not_shown_empty() {
    let refused = rig()
        .ask(json!({"repo": "o/missing"}))
        .expect_err("refused");

    assert_eq!(refused.code, "feed-repo-unknown");
}

#[test]
fn an_unreadable_since_is_refused() {
    let refused = rig()
        .ask(json!({"since": "yesterday-ish"}))
        .expect_err("refused");

    assert_eq!(refused.code, "feed-since-malformed");
}

#[test]
fn no_body_appears_unless_bodies_is_asked_for() {
    let rig = rig();

    let plain = rig.ask(json!({})).expect("answers").to_string();
    assert!(!plain.contains("SECRET"), "{plain}");

    let asked = rig
        .ask(json!({"bodies": true}))
        .expect("answers")
        .to_string();
    for body in [
        "SECRET claim body",
        "SECRET spend body",
        "SECRET message body",
    ] {
        assert!(asked.contains(body), "{body} missing from {asked}");
    }
}

#[test]
fn the_answer_names_what_it_cannot_see() {
    let answer = rig().ask(json!({})).expect("answers");
    let blind = answer["cannot_see"].to_string();

    for word in ["merge", "consent", "turn"] {
        assert!(blind.contains(word), "{word} not named in {blind}");
    }
}

#[test]
fn a_walk_in_pages_has_neither_omission_nor_duplicate() {
    let rig = rig();
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut arguments = json!({"limit": 2});
        if let Some(token) = &cursor {
            arguments["cursor"] = json!(token);
        }
        let answer = rig.ask(arguments).expect("the page answers");
        assert!(answer["events"].as_array().unwrap().len() <= 2);
        seen.extend(kinds(answer["events"].as_array().unwrap()));
        match answer["next_cursor"].as_str() {
            Some(token) => cursor = Some(token.to_owned()),
            None => break,
        }
    }

    assert_eq!(
        seen,
        ["claim", "transition", "transition", "spend", "message"]
    );
}

#[test]
fn a_cursor_is_refused_under_other_selectors_and_over_the_limit_bound() {
    let rig = rig();
    let first = rig.ask(json!({"limit": 1})).expect("answers");
    let token = first["next_cursor"].as_str().expect("more remain");

    let other = rig.ask(json!({"limit": 1, "issue": 5, "cursor": token}));
    assert_eq!(other.expect_err("refused").code, "sessions-cursor-mismatch");

    let big = rig.ask(json!({"limit": MAX_LIMIT + 1}));
    assert_eq!(big.expect_err("refused").code, "sessions-limit-too-large");
}

#[test]
fn reading_a_machine_with_no_store_creates_none() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("state");

    let answer = one_page(&json!({}), &root, Some(T))
        .map(|page| page.answer())
        .expect("an absent store reads as no events");

    assert_eq!(answer["events"].as_array().expect("a list").len(), 0);
    assert!(!directory.path().join("state.db").exists());
}
