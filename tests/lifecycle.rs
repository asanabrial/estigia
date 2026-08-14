#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Process and state tests for the secure binary lifecycle boundary.

use std::process::Command;

use estigia::lifecycle::{Provenance, Relation, StateRoot, Status};
use semver::Version;

fn estigia() -> &'static str {
    env!("CARGO_BIN_EXE_estigia")
}

fn command(home: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new(estigia())
        .args(arguments)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .output()
        .expect("the binary runs")
}

fn home_manifest(home: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    fn visit(
        root: &std::path::Path,
        path: &std::path::Path,
        found: &mut Vec<(std::path::PathBuf, Vec<u8>)>,
    ) {
        let mut entries = std::fs::read_dir(path)
            .expect("home directory reads")
            .collect::<Result<Vec<_>, _>>()
            .expect("home entries read");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("entry is below home")
                .to_path_buf();
            let metadata = std::fs::symlink_metadata(&path).expect("entry metadata reads");
            if metadata.is_dir() {
                found.push((relative, b"directory".to_vec()));
                visit(root, &path, found);
            } else if metadata.is_file() {
                found.push((relative, std::fs::read(&path).expect("file reads")));
            } else {
                found.push((relative, b"other".to_vec()));
            }
        }
    }

    let mut found = Vec::new();
    visit(home, home, &mut found);
    found
}

fn write_release_fixture(state: &StateRoot, version: &str, extra: &str) {
    std::fs::create_dir_all(state.releases()).expect("release fixture directory");
    std::fs::write(
        state.releases().join(format!("{version}.json")),
        format!(r#"{{"schema":3,"version":"{version}"{extra}}}"#),
    )
    .expect("release fixture writes");
}

fn lifecycle_manifest(home: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let root = home.join(".estigia").join("lifecycle");
    if !root.exists() {
        return Vec::new();
    }
    home_manifest(&root)
}

fn record_install(home: &std::path::Path) -> std::process::Output {
    command(home, &["__record-install"])
}

#[test]
fn candidate_recording_allows_plain_setup() {
    let home = tempfile::tempdir().expect("a temporary home");

    let recorded = record_install(home.path());
    assert!(
        recorded.status.success(),
        "{}",
        String::from_utf8_lossy(&recorded.stderr)
    );

    let setup = command(home.path(), &["setup", "claude-code"]);
    assert!(
        setup.status.success(),
        "{}",
        String::from_utf8_lossy(&setup.stderr)
    );
}

#[test]
fn candidate_recording_is_idempotent() {
    let home = tempfile::tempdir().expect("a temporary home");

    assert!(record_install(home.path()).status.success());
    let once = lifecycle_manifest(home.path());
    assert!(record_install(home.path()).status.success());

    assert_eq!(lifecycle_manifest(home.path()), once);
}

#[test]
fn candidate_recording_refuses_downgrade_before_publishing_any_candidate_record() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    write_release_fixture(&state, "2.0.0", "");
    let before = lifecycle_manifest(home.path());

    let output = record_install(home.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("installer-downgrade-blocked"));
    assert_eq!(lifecycle_manifest(home.path()), before);
}

#[test]
fn candidate_recording_refuses_malformed_state_without_publishing() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    std::fs::create_dir_all(state.releases()).expect("release fixture directory");
    std::fs::write(state.releases().join("broken.json"), "not json")
        .expect("malformed fixture writes");
    let before = lifecycle_manifest(home.path());

    let output = record_install(home.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("lifecycle-state-unreadable"));
    assert_eq!(lifecycle_manifest(home.path()), before);
}

#[test]
fn provenance_conflict_does_not_advance_release_history() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    let inspected = Status::inspect_executable(&state, std::path::PathBuf::from(estigia()));
    let digest = inspected
        .executable
        .observed_path_sha256
        .expect("the candidate pathname is readable");
    std::fs::create_dir_all(state.provenance()).expect("provenance fixture directory");
    std::fs::write(
        state.provenance().join(format!("{digest}.json")),
        format!(
            r#"{{"schema":3,"observed_path_sha256":"{digest}","version":"0.0.0","asset_set_sha256":"{}"}}"#,
            "0".repeat(64)
        ),
    )
    .expect("conflicting provenance fixture writes");

    let output = record_install(home.path());

    assert!(!output.status.success());
    assert!(!state.releases().exists(), "release history advanced");
}

#[test]
fn record_install_is_hidden_and_takes_no_identity_arguments() {
    let help = command(tempfile::tempdir().expect("a home").path(), &["--help"]);
    assert!(help.status.success());
    assert!(!String::from_utf8_lossy(&help.stdout).contains("__record-install"));

    let home = tempfile::tempdir().expect("a temporary home");
    let rejected = command(home.path(), &["__record-install", "--version", "1.2.3"]);
    assert!(!rejected.status.success());
    assert!(lifecycle_manifest(home.path()).is_empty());
}

#[test]
fn semver_relations_distinguish_current_downgrade_and_ahead() {
    let running = Version::parse("1.2.3").expect("a version");
    assert_eq!(
        Relation::between(&running, Some(&running)),
        Relation::Current
    );
    assert_eq!(
        Relation::between(&running, Some(&Version::parse("2.0.0").expect("a version"))),
        Relation::DowngradeBlocked
    );
    assert_eq!(
        Relation::between(&running, Some(&Version::parse("1.0.0").expect("a version"))),
        Relation::AheadOfRecorded
    );
}

#[test]
fn malformed_lifecycle_evidence_is_not_an_absent_record() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    std::fs::create_dir_all(state.releases()).expect("the release directory");
    std::fs::write(state.releases().join("1.2.3.json"), "not json").expect("malformed evidence");

    let status = Status::inspect_executable(&state, std::env::current_exe().expect("this binary"));

    assert!(
        status.state_error.is_some(),
        "malformed evidence read as absent"
    );
    assert_eq!(status.relation, Relation::Unknown);
}

#[test]
fn immutable_release_records_make_high_water_monotonic() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    write_release_fixture(&state, "2.0.0", "");
    write_release_fixture(&state, "1.0.0", "");

    assert_eq!(
        state.high_water().expect("the records read"),
        Some(Version::parse("2.0.0").expect("a version"))
    );
}

#[test]
fn a_source_build_refuses_setup_all_without_changing_home() {
    let home = tempfile::tempdir().expect("a temporary home");
    let before = home_manifest(home.path());

    let output = command(home.path(), &["setup", "--all"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source-build-not-allowed"));
    assert_eq!(home_manifest(home.path()), before);
}

#[test]
fn a_source_build_refuses_sync_all_without_changing_home() {
    let home = tempfile::tempdir().expect("a temporary home");
    let setup = command(
        home.path(),
        &["setup", "claude-code", "--allow-source-build"],
    );
    assert!(
        setup.status.success(),
        "{}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let before = home_manifest(home.path());

    let output = command(home.path(), &["sync", "--all"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source-build-not-allowed"));
    assert_eq!(home_manifest(home.path()), before);
}

#[test]
fn an_explicit_source_override_allows_setup_without_advancing_high_water() {
    let home = tempfile::tempdir().expect("a temporary home");

    let output = command(
        home.path(),
        &["setup", "claude-code", "--allow-source-build"],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state = StateRoot::under(home.path());
    assert_eq!(state.high_water().expect("state reads"), None);
}

#[test]
fn an_installer_recorded_downgrade_refuses_even_with_the_source_override() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    assert!(record_install(home.path()).status.success());
    write_release_fixture(&state, "2.0.0", "");

    let output = command(
        home.path(),
        &["setup", "claude-code", "--allow-source-build"],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("recorded-downgrade-blocked"));
    assert!(!home.path().join(".claude").exists());
}

#[test]
fn provenance_rejects_mismatched_compiled_payload() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    let executable = std::path::PathBuf::from(estigia());
    let inspected = Status::inspect_executable(&state, executable.clone());
    let digest = inspected
        .executable
        .observed_path_sha256
        .expect("the observed pathname digest");
    std::fs::create_dir_all(state.provenance()).expect("provenance directory");
    std::fs::write(
        state.provenance().join(format!("{digest}.json")),
        format!(
            r#"{{"schema":3,"observed_path_sha256":"{digest}","version":"0.0.0","asset_set_sha256":"{}"}}"#,
            "0".repeat(64)
        ),
    )
    .expect("mismatched installer record");

    let status = Status::inspect_executable(&state, executable);

    assert_eq!(status.provenance, Provenance::Unknown);
    assert_eq!(status.relation, Relation::Unknown);
}

#[test]
fn provenance_release_must_equal_the_compiled_package_version() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    let executable = std::path::PathBuf::from(estigia());
    let inspected = Status::inspect_executable(&state, executable.clone());
    let digest = inspected
        .executable
        .observed_path_sha256
        .expect("the observed pathname digest");
    std::fs::create_dir_all(state.provenance()).expect("provenance directory");
    std::fs::write(
        state.provenance().join(format!("{digest}.json")),
        format!(
            r#"{{"schema":3,"observed_path_sha256":"{digest}","version":"99.0.0","asset_set_sha256":"{}"}}"#,
            inspected.executable.asset_set_sha256
        ),
    )
    .expect("version-mismatched record");

    assert_eq!(
        Status::inspect_executable(&state, executable).relation,
        Relation::Unknown
    );
}

#[test]
fn release_build_metadata_is_not_a_canonical_identity() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());

    write_release_fixture(&state, "1.2.3+local", "");
    let error = state.high_water().expect_err("build metadata must refuse");

    assert!(error.to_string().contains("build metadata"));
}

#[test]
fn provenance_key_mismatch_is_unknown() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    let executable = std::path::PathBuf::from(estigia());
    let inspected = Status::inspect_executable(&state, executable.clone());
    let digest = inspected
        .executable
        .observed_path_sha256
        .expect("the observed pathname digest");
    std::fs::create_dir_all(state.provenance()).expect("provenance directory");
    std::fs::write(
        state.provenance().join(format!("{digest}.json")),
        format!(
            r#"{{"schema":3,"observed_path_sha256":"{}","version":"{}","asset_set_sha256":"{}"}}"#,
            "0".repeat(64),
            env!("CARGO_PKG_VERSION"),
            inspected.executable.asset_set_sha256
        ),
    )
    .expect("key-mismatched record");

    assert_eq!(
        Status::inspect_executable(&state, executable).relation,
        Relation::Unknown
    );
}

#[cfg(unix)]
#[test]
fn preplanted_publication_symlink_refuses_without_truncating_target() {
    use std::os::unix::fs::symlink;

    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    let version = env!("CARGO_PKG_VERSION");
    std::fs::create_dir_all(state.releases()).expect("release directory");
    let target = home.path().join("target.txt");
    std::fs::write(&target, "keep me").expect("target contents");
    symlink(&target, state.releases().join(format!("{version}.json")))
        .expect("publication symlink");

    assert!(!record_install(home.path()).status.success());

    assert_eq!(
        std::fs::read_to_string(target).expect("target survives"),
        "keep me"
    );
}

#[test]
fn lifecycle_records_reject_unknown_fields() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = StateRoot::under(home.path());
    write_release_fixture(&state, "1.2.3", r#","unexpected":true"#);

    assert!(state.high_water().is_err());
}

#[test]
fn dry_run_does_not_create_lifecycle_state() {
    let home = tempfile::tempdir().expect("a temporary home");

    let output = command(home.path(), &["setup", "--all", "--dry-run"]);

    assert!(output.status.success());
    assert!(!home.path().join(".estigia").join("lifecycle").exists());
}

#[test]
fn update_json_inventories_source_and_unavailable_public_release() {
    let home = tempfile::tempdir().expect("a temporary home");

    let output = command(home.path(), &["update", "--json"]);
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("update prints JSON");

    assert!(output.status.success());
    assert_eq!(value["provenance"]["kind"], "source_or_unrecorded");
    assert_eq!(value["relation"], "source_or_unrecorded");
    assert_eq!(value["public_release"]["kind"], "unavailable");
    assert_eq!(value["public_release"]["checked"], false);
    assert!(
        value["executable"]["observed_path_sha256"]
            .as_str()
            .is_some()
    );
}
