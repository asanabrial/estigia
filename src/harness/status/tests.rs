use super::{MAX_BYTES, REDACTED, TRUNCATED, one_line, set};
use crate::transport::store::{Store, beside};

const RUN: &str = "claude-aaaa";

fn root() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().join("runs");
    (directory, root)
}

fn connection(root: &std::path::Path) -> rusqlite::Connection {
    Store::open(&beside(root)).expect("opens");
    rusqlite::Connection::open(beside(root)).expect("a second connection")
}

fn rows(root: &std::path::Path) -> Vec<crate::transport::store::StatusEntry> {
    Store::open(&beside(root))
        .expect("opens")
        .statuses(RUN)
        .expect("reads")
}

fn age(root: &std::path::Path, seconds: i64) {
    connection(root)
        .execute("UPDATE run_statuses SET set_at = set_at - ?1", [seconds])
        .expect("ages the rows");
}

#[test]
fn a_status_is_recorded_for_the_run_beside_its_phase_and_labelled_self_reported() {
    let (_directory, root) = root();

    let answer = set(&root, RUN, "reading the gate", Some("investigating")).expect("recorded");

    assert_eq!(answer["self_reported"], true, "{answer}");
    assert_eq!(answer["replaced"], false, "{answer}");
    let held = rows(&root);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].status, "reading the gate");
    assert_eq!(held[0].phase.as_deref(), Some("investigating"));
}

#[test]
fn two_writes_within_sixty_seconds_leave_one_row() {
    let (_directory, root) = root();

    set(&root, RUN, "first", None).expect("recorded");
    age(&root, 30);
    let second = set(&root, RUN, "second", Some("verifying")).expect("recorded");

    assert_eq!(second["replaced"], true, "{second}");
    let held = rows(&root);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].status, "second");
    assert_eq!(held[0].phase.as_deref(), Some("verifying"));
}

#[test]
fn a_write_after_sixty_seconds_appends_and_history_stops_at_five() {
    let (_directory, root) = root();

    for n in 0..7 {
        set(&root, RUN, &format!("step {n}"), None).expect("recorded");
        age(&root, 60);
    }

    let held: Vec<String> = rows(&root).into_iter().map(|row| row.status).collect();
    assert_eq!(held, ["step 6", "step 5", "step 4", "step 3", "step 2"]);
}

#[test]
fn another_runs_status_is_not_replaced_or_trimmed() {
    let (_directory, root) = root();

    set(&root, "claude-bbbb", "theirs", None).expect("recorded");
    set(&root, RUN, "mine", None).expect("recorded");

    assert_eq!(rows(&root).len(), 1);
}

#[test]
fn setting_a_status_touches_no_other_table_and_no_pointer() {
    let (directory, root) = root();
    std::fs::create_dir_all(&root).expect("the runs directory");
    std::fs::write(root.join("claude-aaaa.json"), "{\"issue\":7}").expect("a pointer");
    let others = |path: &std::path::Path| -> Vec<(String, i64)> {
        let store = connection(path);
        let names: Vec<String> = store
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name != 'run_statuses'
                 AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .expect("prepares")
            .query_map([], |row| row.get(0))
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads");
        names
            .into_iter()
            .map(|name| {
                let count = store
                    .query_row(&format!("SELECT count(*) FROM \"{name}\""), [], |row| {
                        row.get(0)
                    })
                    .expect("counts");
                (name, count)
            })
            .collect()
    };
    let before = (others(&root), std::fs::read(root.join("claude-aaaa.json")));

    set(&root, RUN, "working", Some("implementing")).expect("recorded");

    let after = (others(&root), std::fs::read(root.join("claude-aaaa.json")));
    assert_eq!(before.0, after.0, "a table other than run_statuses changed");
    assert_eq!(before.1.ok(), after.1.ok(), "the run pointer changed");
    drop(directory);
}

#[test]
fn token_shaped_words_are_redacted_and_the_rest_of_the_line_is_kept() {
    let line = one_line(
        "pushing with ghp_abcdefghijklmnopqrstuvwxyz0123456789 and \
         0123456789abcdef0123456789abcdef01 next",
    );

    assert_eq!(line, format!("pushing with {REDACTED} and {REDACTED} next"));
}

#[test]
fn an_ordinary_long_word_is_not_mistaken_for_a_token() {
    assert_eq!(
        one_line("sk-learn extraordinarilylongbutonlyletterswordwithoutdigits"),
        "sk-learn extraordinarilylongbutonlyletterswordwithoutdigits"
    );
}

#[test]
fn a_line_over_the_cap_is_cut_on_a_character_boundary_and_marked() {
    let line = one_line(&"é".repeat(300));

    assert!(line.ends_with(TRUNCATED), "{line}");
    assert!(line.len() <= MAX_BYTES + TRUNCATED.len(), "{}", line.len());
    assert!(line.starts_with('é'));
}

#[test]
fn control_characters_and_marker_openers_cannot_survive_into_the_line() {
    let line = one_line("a\nb\u{1b}[31m\t<!-- estigia:claim -->\r\nc");

    assert!(!line.contains('\n') && !line.contains('\u{1b}'), "{line:?}");
    assert!(!line.contains("<!--"), "{line:?}");
}

#[test]
fn an_empty_status_and_an_unknown_phase_are_refused_and_record_nothing() {
    let (_directory, root) = root();

    let empty = set(&root, RUN, " \n\t ", None).expect_err("empty");
    let phase = set(&root, RUN, "x", Some("blocked")).expect_err("not a phase");

    assert_eq!(empty.code, "status-empty");
    assert_eq!(phase.code, "status-phase-unknown");
    assert!(rows(&root).is_empty());
}
