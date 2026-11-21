use super::*;

fn voices(home: &std::path::Path) -> std::path::PathBuf {
    let dir = home.join(".estigia").join("voices");
    std::fs::create_dir_all(&dir).expect("a voices directory");
    dir
}

fn home() -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    home
}

#[test]
fn check_names_both_files_and_says_when_there_are_no_habits() {
    let home = home();
    let dir = voices(home.path());
    std::fs::write(dir.join("house.md"), "Plain and brief.").expect("a voice");
    let (out, _, ok) = run(home.path(), &["voice", "check", "house"], "");
    assert!(ok, "{out}");
    assert!(out.contains("house.md"), "{out}");
    assert!(out.contains("habits: none"), "{out}");
    std::fs::write(dir.join("house.habits.json"), "{}").expect("habits");
    let (out, _, ok) = run(home.path(), &["--json", "voice", "check", "house"], "");
    assert!(ok, "{out}");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("one object");
    assert!(
        parsed["habits"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("house.habits.json")),
        "{out}"
    );
}

#[test]
fn check_exits_non_zero_with_the_code_a_post_would_refuse_with() {
    let home = home();
    let dir = voices(home.path());
    let (out, err, ok) = run(home.path(), &["voice", "check", "house"], "");
    assert!(!ok);
    assert!(format!("{out}{err}").contains("review-voice-file-missing"));
    std::fs::write(dir.join("house.md"), "x".repeat(16 * 1024 + 1)).expect("a long voice");
    let (out, err, ok) = run(home.path(), &["voice", "check", "house"], "");
    assert!(!ok);
    assert!(format!("{out}{err}").contains("review-voice-file-too-large"));
    std::fs::write(dir.join("house.md"), "Plain.").expect("a voice");
    std::fs::write(dir.join("house.habits.json"), r#"{ "unknown": 1 }"#).expect("habits");
    let (out, err, ok) = run(home.path(), &["voice", "check", "house"], "");
    assert!(!ok);
    assert!(format!("{out}{err}").contains("review-voice-habits-invalid"));
}

#[test]
fn habits_only_preview_applies_the_habits_wherever_it_runs() {
    let home = home();
    let dir = voices(home.path());
    std::fs::write(
        dir.join("house.habits.json"),
        r#"{ "lowercase_start": 1.0, "final_stop_dropped": 1.0 }"#,
    )
    .expect("habits");
    let first = home.path().join("first.txt");
    let second = home.path().join("second.txt");
    std::fs::write(&first, "Hello there.").expect("a text");
    std::fs::write(&second, "Another one.").expect("a text");
    let (first_arg, second_arg) = (first.to_string_lossy(), second.to_string_lossy());
    let arguments = [
        "voice",
        "preview",
        "house",
        "--habits-only",
        &first_arg,
        &second_arg,
    ];
    let (out, _, ok) = run(home.path(), &arguments, "");
    assert!(ok, "{out}");
    assert_eq!(out, "hello there\n---\nanother one\n");
    let repository = home.path().join("repository");
    std::fs::create_dir_all(&repository).expect("a directory");
    let (inside, _, ok) = run_in(home.path(), &repository, &arguments, "");
    assert!(ok);
    assert_eq!(inside, out);
}

#[test]
fn habits_only_without_a_habits_file_leaves_the_text_and_says_so_on_stderr() {
    let home = home();
    let text = home.path().join("text.txt");
    std::fs::write(&text, "Hello there.").expect("a text");
    let (out, err, ok) = run(
        home.path(),
        &[
            "voice",
            "preview",
            "house",
            "--habits-only",
            &text.to_string_lossy(),
        ],
        "",
    );
    assert!(ok, "{err}");
    assert_eq!(out, "Hello there.\n");
    assert!(err.contains("no habits file"), "{err}");
}
