// Same exception `tests/pipe.rs` and `tests/release.rs` take: an integration
// test is a separate crate, so the crate-level `allow` does not reach it.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Drives the real `scripts/build-install.sh` against stub `rustup`/`cargo`
//! binaries on `PATH`, so its install-root resolution, its relative-path
//! handling and its refusals are exercised without a network or a real build.
//!
//! `RUSTUP_SHIM` stands in for `rustup`: it logs every invocation to
//! `STUB_LOG`, and for `cargo install --root <R> ...` it also writes an
//! executable `<R>/bin/estigia` that itself logs its own argv. `CARGO_SHIM`
//! only has to exist so `command -v cargo` finds something.

#![cfg(unix)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUSTUP_SHIM: &str = r#"#!/bin/sh
printf '%s\t%s\n' "$PWD" "$*" >> "$STUB_LOG"
if [ "$1" = "toolchain" ]; then
    exit 0
fi
if [ "$1" = "run" ] && [ "$3" = "cargo" ] && [ "$4" = "install" ]; then
    root=""
    prev=""
    for arg in "$@"; do
        if [ "$prev" = "--root" ]; then
            root="$arg"
        fi
        prev="$arg"
    done
    mkdir -p "$root/bin"
    cat > "$root/bin/estigia" <<'SHIM'
#!/bin/sh
printf '%s\t%s\n' "$PWD" "$*" >> "$STUB_LOG"
exit 0
SHIM
    chmod 755 "$root/bin/estigia"
fi
exit 0
"#;

const FAILING_RUSTUP_SHIM: &str = r#"#!/bin/sh
printf '%s\t%s\n' "$PWD" "$*" >> "$STUB_LOG"
exit 7
"#;

const CARGO_SHIM: &str = r#"#!/bin/sh
printf '%s\t%s\n' "$PWD" "$*" >> "$STUB_LOG"
exit 0
"#;

const SYSTEM_PATH: &str = "/usr/bin:/bin";

fn path_with(dir: &Path) -> OsString {
    OsString::from(format!("{}:{SYSTEM_PATH}", dir.display()))
}

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/build-install.sh")
}

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("the shim script is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("the shim is made executable");
    }
}

fn shim_dir(shims: &[(&str, &str)]) -> tempfile::TempDir {
    let bin = tempfile::tempdir().expect("a temporary bin directory");
    for (name, body) in shims {
        write_executable(&bin.path().join(name), body);
    }
    bin
}

/// A directory holding executable `rustup` and `cargo` shims.
fn stub_bin() -> tempfile::TempDir {
    shim_dir(&[("rustup", RUSTUP_SHIM), ("cargo", CARGO_SHIM)])
}

/// Runs the real script under `sh`, with exactly the given environment and
/// working directory. Unix-only (see the crate attribute): `sh` is part of the
/// platform, so a failure to spawn it is a broken fixture, not a skip.
fn run(vars: Vec<(&str, OsString)>, cwd: &Path) -> Output {
    Command::new("sh")
        .arg(script_path())
        .env_clear()
        .envs(vars)
        .current_dir(cwd)
        .output()
        .expect("sh runs")
}

fn log_of(cwd: &Path) -> String {
    std::fs::read_to_string(cwd.join("log")).unwrap_or_default()
}

#[test]
fn a_path_without_rustup_fails_naming_it_before_anything_builds() {
    let bin = shim_dir(&[("cargo", CARGO_SHIM)]);
    let cwd = tempfile::tempdir().expect("a temporary cwd");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(cwd.path())),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(!output.status.success(), "{:?}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("'rustup'"), "{stderr}");
    assert!(stderr.contains("https://rustup.rs"), "{stderr}");
    assert_eq!(
        log_of(cwd.path()),
        "",
        "something ran before the missing tool was reported"
    );
}

#[test]
fn a_path_without_cargo_fails_naming_it_before_anything_builds() {
    let bin = shim_dir(&[("rustup", RUSTUP_SHIM)]);
    let cwd = tempfile::tempdir().expect("a temporary cwd");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(cwd.path())),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(!output.status.success(), "{:?}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("'cargo'"), "{stderr}");
    assert!(stderr.contains("https://rustup.rs"), "{stderr}");
    assert_eq!(
        log_of(cwd.path()),
        "",
        "something ran before the missing tool"
    );
}

#[test]
fn an_absolute_install_root_is_used_as_is_and_the_five_steps_run_in_order() {
    let bin = stub_bin();
    let cwd = tempfile::tempdir().expect("a temporary cwd");
    let root = tempfile::tempdir().expect("a temporary install root");
    let decoy = tempfile::tempdir().expect("a temporary decoy root");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(cwd.path())),
        ("CARGO_INSTALL_ROOT", OsString::from(root.path())),
        ("CARGO_HOME", OsString::from(decoy.path())),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let logged = log_of(cwd.path());
    assert!(
        logged.contains(&format!("cargo install --root {}", root.path().display())),
        "{logged}"
    );
    let steps = [
        "--version",
        "update",
        "setup --all --allow-source-build",
        "status",
        "doctor",
    ];
    let positions: Vec<usize> = steps
        .iter()
        .map(|step| {
            // The step as the logged command, not the word anywhere in the
            // line: a checkout path containing it (a worktree named for a
            // `doctor` change) would otherwise be found first.
            logged
                .find(&format!("\t{step}"))
                .unwrap_or_else(|| panic!("{step:?} missing from the log:\n{logged}"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "the five steps did not run in order:\n{logged}"
    );
    assert!(
        !logged.contains(&format!("--root {}", decoy.path().display())),
        "CARGO_INSTALL_ROOT must win over CARGO_HOME:\n{logged}"
    );
    let install = logged
        .lines()
        .find(|line| line.contains("cargo install --root"))
        .expect("the install was logged");
    assert!(
        install.starts_with(&Path::new(env!("CARGO_MANIFEST_DIR")).display().to_string()),
        "the build and install must run from the checkout:\n{install}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let last_line = stdout.lines().last().unwrap_or_default();
    let estigia = root.path().join("bin").join("estigia");
    assert!(
        last_line.contains(&estigia.display().to_string()),
        "{stdout}"
    );
}

#[test]
fn a_relative_install_root_resolves_from_the_callers_cwd_not_the_checkout() {
    let bin = stub_bin();
    let cwd = tempfile::tempdir().expect("a temporary cwd outside the checkout");
    let relative = "wherever/it/lands";
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(cwd.path())),
        ("CARGO_INSTALL_ROOT", OsString::from(relative)),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The script expands a relative root against `$PWD` before any `cd`; with
    // the environment cleared the shell derives that from `getcwd(3)`, which
    // resolves symlinks — on macOS `/var/...` (what `tempfile` hands back)
    // becomes `/private/var/...`, so the expectation goes through the same
    // resolution or the two paths disagree over a symlink neither side is
    // wrong about.
    let expected_root = cwd
        .path()
        .canonicalize()
        .expect("the temporary cwd resolves")
        .join(relative);
    let logged = log_of(cwd.path());
    assert!(
        logged.contains(&format!("cargo install --root {}", expected_root.display())),
        "{logged}"
    );
    let estigia = expected_root.join("bin").join("estigia");
    assert!(estigia.is_file(), "{}", estigia.display());
}

#[test]
fn no_install_root_home_or_cargo_home_fails_with_a_clear_reason() {
    let bin = stub_bin();
    let cwd = tempfile::tempdir().expect("a temporary cwd");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(!output.status.success(), "{:?}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot tell where Cargo installs binaries"),
        "{stderr}"
    );
    assert_eq!(
        log_of(cwd.path()),
        "",
        "the refusal must come before anything runs"
    );
}

#[test]
fn cargo_home_is_the_root_when_cargo_install_root_is_unset() {
    let bin = stub_bin();
    let cwd = tempfile::tempdir().expect("a temporary cwd");
    let root = tempfile::tempdir().expect("a temporary CARGO_HOME");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(cwd.path())),
        ("CARGO_HOME", OsString::from(root.path())),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let logged = log_of(cwd.path());
    assert!(
        logged.contains(&format!("cargo install --root {}", root.path().display())),
        "{logged}"
    );
    assert!(
        root.path().join("bin").join("estigia").is_file(),
        "{logged}"
    );
}

#[test]
fn the_default_root_is_cargo_home_under_home() {
    let bin = stub_bin();
    let home = tempfile::tempdir().expect("a temporary HOME");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(home.path())),
        ("STUB_LOG", OsString::from(home.path().join("log"))),
    ];
    let output = run(vars, home.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = home.path().join(".cargo");
    let logged = log_of(home.path());
    assert!(
        logged.contains(&format!("cargo install --root {}", expected.display())),
        "the fallback must be Cargo's own default, $HOME/.cargo:\n{logged}"
    );
    assert!(expected.join("bin").join("estigia").is_file(), "{logged}");
}

#[test]
fn a_failing_toolchain_step_stops_everything_after_it() {
    let bin = shim_dir(&[("rustup", FAILING_RUSTUP_SHIM), ("cargo", CARGO_SHIM)]);
    let cwd = tempfile::tempdir().expect("a temporary cwd");
    let vars = vec![
        ("PATH", path_with(bin.path())),
        ("HOME", OsString::from(cwd.path())),
        ("STUB_LOG", OsString::from(cwd.path().join("log"))),
    ];
    let output = run(vars, cwd.path());
    assert!(!output.status.success(), "{:?}", output.status);
    let logged = log_of(cwd.path());
    assert!(logged.contains("toolchain install"), "{logged}");
    assert!(!logged.contains("cargo install"), "{logged}");
    assert!(!logged.contains("--version"), "{logged}");
}
