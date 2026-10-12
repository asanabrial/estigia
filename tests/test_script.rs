#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Drives the real `scripts/test.sh` against a stand-in `cargo` that appends
//! its argv to a log, so the flags it forwards to the fixture build are held
//! to the exact command line. `sh` is part of the platform: failing to spawn
//! it is a broken fixture, never a skip.

#![cfg(unix)]

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

const CARGO_SHIM: &str = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$STUB_LOG\"\n";

fn invocations(arguments: &[&str]) -> Vec<String> {
    let bin = tempfile::tempdir().expect("a temporary bin directory");
    let shim = bin.path().join("cargo");
    std::fs::write(&shim, CARGO_SHIM).expect("the shim is written");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))
            .expect("the shim is made executable");
    }
    let log = bin.path().join("log");
    let output = Command::new("sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test.sh"))
        .args(arguments)
        .env_clear()
        .env(
            "PATH",
            OsString::from(format!("{}:/usr/bin:/bin", bin.path().display())),
        )
        .env("STUB_LOG", &log)
        .output()
        .expect("sh runs");
    assert!(
        output.status.success(),
        "{:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(&log)
        .expect("the stand-in cargo ran")
        .lines()
        .map(str::to_owned)
        .collect()
}

fn assert_forwards(arguments: &[&str], build_flags: &str) {
    let expected_build = if build_flags.is_empty() {
        "build --bins --examples".to_owned()
    } else {
        format!("build --bins --examples {build_flags}")
    };
    let expected_test = format!("test {}", arguments.join(" "));
    assert_eq!(
        invocations(arguments),
        vec![expected_build, expected_test.trim_end().to_owned()],
        "arguments: {arguments:?}"
    );
}

#[test]
fn no_flags_build_plainly_and_test_everything_given() {
    assert_forwards(&[], "");
    assert_forwards(&["some_filter", "--lib"], "");
}

#[test]
fn release_is_forwarded_to_the_build() {
    assert_forwards(&["--release", "name"], "--release");
}

#[test]
fn target_is_forwarded_in_both_forms() {
    assert_forwards(&["--target", "x", "name"], "--target x");
    assert_forwards(&["--target=x", "name"], "--target=x");
}

#[test]
fn manifest_path_is_forwarded_in_both_forms() {
    assert_forwards(&["--manifest-path", "p", "name"], "--manifest-path p");
    assert_forwards(&["--manifest-path=p", "name"], "--manifest-path=p");
}

#[test]
fn profile_is_forwarded_in_both_forms() {
    assert_forwards(&["--profile", "q", "name"], "--profile q");
    assert_forwards(&["--profile=q", "name"], "--profile=q");
}

#[test]
fn nothing_after_the_separator_reaches_the_build() {
    assert_forwards(
        &[
            "--release",
            "name",
            "--",
            "--release",
            "--target",
            "x",
            "--profile=q",
        ],
        "--release",
    );
}
