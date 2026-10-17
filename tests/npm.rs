//! The npm installer and the MCP registry entry, held to the release they publish.
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|why| panic!("{relative} ships with the repository: {why}"))
}

fn json(relative: &str) -> serde_json::Value {
    serde_json::from_str(&read(relative))
        .unwrap_or_else(|why| panic!("{relative} is not JSON: {why}"))
}

fn release_targets() -> Vec<String> {
    let mut targets: Vec<String> = read(".github/workflows/release.yml")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- target:"))
        .map(|target| target.trim().to_owned())
        .collect();
    targets.sort();
    targets
}

fn installer_targets() -> Vec<String> {
    let installer = read("npm/bin/estigia.js");
    let mut targets: Vec<String> = installer
        .split("triple: \"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_owned)
        .collect();
    targets.sort();
    targets
}

fn crate_version() -> String {
    read("Cargo.toml")
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("Cargo.toml names no package version"))
}

#[test]
fn the_npm_installer_offers_exactly_the_targets_the_release_builds() {
    let release = release_targets();
    assert_eq!(release.len(), 6, "release.yml builds {release:?}");
    assert_eq!(installer_targets(), release);
}

#[test]
fn every_registry_publishes_the_version_the_crate_declares() {
    let version = crate_version();
    let npm = json("npm/package.json");
    let server = json("server.json");
    assert_eq!(npm["version"], version.as_str(), "npm/package.json");
    assert_eq!(server["version"], version.as_str(), "server.json");
    let packages = server["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("server.json lists no packages"));
    for package in packages {
        assert_eq!(
            package["version"],
            version.as_str(),
            "server.json {package}"
        );
    }
}

#[test]
fn the_mcp_registry_entry_names_the_packages_this_repository_publishes() {
    let npm = json("npm/package.json");
    let server = json("server.json");
    assert_eq!(npm["mcpName"], server["name"]);
    let description = server["description"].as_str().unwrap_or_default();
    assert!(
        !description.is_empty() && description.chars().count() <= 100,
        "the registry refuses a description over 100 characters: {description:?}"
    );
    let published: Vec<(String, String)> = server["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|package| {
            (
                package["registryType"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                package["identifier"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect();
    assert!(published.contains(&(
        "npm".to_owned(),
        npm["name"].as_str().unwrap_or_default().to_owned()
    )));
    assert!(published.contains(&("cargo".to_owned(), "estigia".to_owned())));
}

#[cfg(unix)]
mod installer {
    use super::root;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::process::{Command, Output, Stdio};
    use std::time::{Duration, Instant};

    const TAG: &str = "v9.9.9";

    fn host_triple() -> &'static str {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            ("macos", "aarch64") => "aarch64-apple-darwin",
            (os, arch) => panic!("no release target for {os}-{arch}"),
        }
    }

    fn package() -> String {
        format!("estigia-{TAG}-{}", host_triple())
    }

    fn sha256(bytes: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn archive(dir: &Path, record_exit: u8) -> Vec<u8> {
        let staged = dir.join("stage").join(package());
        std::fs::create_dir_all(&staged).unwrap_or_else(|why| panic!("stage: {why}"));
        let fake = format!(
            "#!/bin/sh\n\
             echo \"$*\" >> \"$FAKE_LOG\"\n\
             case \"$1\" in\n\
             --version) echo 'estigia 9.9.9 (fake)' ;;\n\
             __record-install) exit {record_exit} ;;\n\
             *) echo \"ran $*\" ;;\n\
             esac\n"
        );
        let binary = staged.join("estigia");
        std::fs::write(&binary, fake).unwrap_or_else(|why| panic!("fake binary: {why}"));
        let tarball = dir.join("archive.tar.gz");
        let made = Command::new("tar")
            .arg("-czf")
            .arg(&tarball)
            .arg("-C")
            .arg(dir.join("stage"))
            .arg(package())
            .status()
            .unwrap_or_else(|why| panic!("these tests need tar: {why}"));
        assert!(made.success(), "tar could not build the archive");
        std::fs::read(&tarball).unwrap_or_else(|why| panic!("archive: {why}"))
    }

    fn serve(files: HashMap<String, Vec<u8>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|why| panic!("bind: {why}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|why| panic!("address: {why}"));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut request = [0_u8; 4096];
                let read = stream.read(&mut request).unwrap_or(0);
                let line = String::from_utf8_lossy(&request[..read]).into_owned();
                let path = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/');
                let (status, body) = match files.get(path) {
                    Some(body) => ("200 OK", body.clone()),
                    None => ("404 Not Found", Vec::new()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        format!("http://{address}")
    }

    fn run(home: &Path, base: &str) -> Output {
        let mut child = Command::new("node")
            .arg(root().join("npm/bin/estigia.js"))
            .args(["mcp", "--flag"])
            .env("HOME", home)
            .env("ESTIGIA_INSTALL_DIR", home.join("bin"))
            .env("ESTIGIA_VERSION", TAG)
            .env("ESTIGIA_BASE_URL", base)
            .env("FAKE_LOG", home.join("log"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|why| panic!("these tests need node on the path: {why}"));
        let deadline = Instant::now() + Duration::from_secs(120);
        while child.try_wait().ok().flatten().is_none() {
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("the npm installer did not finish within 120 s");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        child
            .wait_with_output()
            .unwrap_or_else(|why| panic!("node: {why}"))
    }

    fn log(home: &Path) -> Vec<String> {
        std::fs::read_to_string(home.join("log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn rig(record_exit: u8, sum: Option<&str>) -> (tempfile::TempDir, String) {
        let home = tempfile::tempdir().unwrap_or_else(|why| panic!("tempdir: {why}"));
        let bytes = archive(home.path(), record_exit);
        let name = format!("{}.tar.gz", package());
        let mut files = HashMap::new();
        if let Some(sum) = sum {
            let sum = if sum.is_empty() {
                sha256(&bytes)
            } else {
                sum.to_owned()
            };
            files.insert(
                format!("{name}.sha256"),
                format!("{sum}  {name}\n").into_bytes(),
            );
        }
        files.insert(name, bytes);
        let base = serve(files);
        (home, base)
    }

    #[test]
    fn the_npm_installer_records_and_places_a_verified_binary_and_runs_it() {
        let (home, base) = rig(0, Some(""));
        let first = run(home.path(), &base);
        let stderr = String::from_utf8_lossy(&first.stderr);
        assert!(first.status.success(), "the installer failed: {stderr}");
        assert_eq!(
            String::from_utf8_lossy(&first.stdout),
            "ran mcp --flag\n",
            "stderr: {stderr}"
        );
        assert!(home.path().join("bin/estigia").is_file());
        assert_eq!(log(home.path()), ["__record-install", "mcp --flag"]);

        let again = run(home.path(), "http://127.0.0.1:9");
        assert!(
            again.status.success(),
            "{}",
            String::from_utf8_lossy(&again.stderr)
        );
        assert_eq!(
            log(home.path()),
            ["__record-install", "mcp --flag", "--version", "mcp --flag"],
            "an installed binary at this version is run without fetching or recording again"
        );
    }

    #[test]
    fn the_npm_installer_refuses_an_archive_that_does_not_match_its_sum() {
        let (home, base) = rig(0, Some(&"0".repeat(64)));
        let answer = run(home.path(), &base);
        assert!(!answer.status.success());
        assert!(String::from_utf8_lossy(&answer.stderr).contains("checksum mismatch"));
        assert!(answer.stdout.is_empty());
        assert!(!home.path().join("bin/estigia").exists());
        assert!(log(home.path()).is_empty());
    }

    #[test]
    fn the_npm_installer_refuses_an_archive_with_no_published_sum() {
        let (home, base) = rig(0, None);
        let answer = run(home.path(), &base);
        assert!(!answer.status.success());
        assert!(String::from_utf8_lossy(&answer.stderr).contains("no checksum published"));
        assert!(!home.path().join("bin/estigia").exists());
        assert!(log(home.path()).is_empty());
    }

    #[test]
    fn the_npm_installer_places_nothing_the_lifecycle_record_refused() {
        let (home, base) = rig(1, Some(""));
        let answer = run(home.path(), &base);
        assert!(!answer.status.success());
        assert!(String::from_utf8_lossy(&answer.stderr).contains("lifecycle admission failed"));
        assert!(!home.path().join("bin/estigia").exists());
        assert_eq!(log(home.path()), ["__record-install"]);
    }
}
