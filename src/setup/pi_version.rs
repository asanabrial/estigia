use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use super::companion::resolve_on_path;

/// A Pi release, read as three numbers.
///
/// A pre-release or build suffix is dropped: `1.2.0-beta.1` is read as `1.2.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PiVersion {
    /// The major number.
    pub major: u64,
    /// The minor number.
    pub minor: u64,
    /// The patch number.
    pub patch: u64,
}

/// The first Pi that lists `deferred` servers in the system prompt and loads their tools through
/// `tool_search`.
pub const DEFERRED_FROM: PiVersion = PiVersion {
    major: 1,
    minor: 2,
    patch: 0,
};

impl PiVersion {
    /// The first `major.minor.patch` triple found in `text`, or nothing.
    pub fn parse(text: &str) -> Option<Self> {
        text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .find_map(|word| {
                let mut numbers = word.split('.');
                let major = numbers.next()?.parse().ok()?;
                let minor = numbers.next()?.parse().ok()?;
                let patch = numbers.next()?.parse().ok()?;
                numbers.next().is_none().then_some(Self {
                    major,
                    minor,
                    patch,
                })
            })
    }
}

impl fmt::Display for PiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// How the Estigia server is exposed to the model by Pi's MCP client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiExposure {
    /// Every tool is declared on every request. The only exposure Pi before 1.2.0 can use.
    Direct,
    /// Tools are loaded on demand through `tool_search`. Pi 1.2.0 and later.
    Deferred,
}

impl PiExposure {
    /// The exposure written for a Pi of this version; an unreadable version gets [`Self::Direct`].
    pub fn for_version(version: Option<PiVersion>) -> Self {
        if version.is_some_and(|version| version >= DEFERRED_FROM) {
            Self::Deferred
        } else {
            Self::Direct
        }
    }

    /// The value of the `exposure` field.
    pub fn word(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Deferred => "deferred",
        }
    }
}

/// The version of the `pi` on `PATH`, or nothing when it is absent or does not say.
pub fn installed_pi_version() -> Option<PiVersion> {
    version_of(resolve_on_path("pi")?, VERSION_PROBE_TIMEOUT)
}

const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

fn version_of(program: PathBuf, timeout: Duration) -> Option<PiVersion> {
    let output = crate::bounded::run(
        "pi --version",
        program,
        vec!["--version".to_owned()],
        timeout,
        None,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    PiVersion::parse(&String::from_utf8_lossy(&output.stdout))
        .or_else(|| PiVersion::parse(&String::from_utf8_lossy(&output.stderr)))
}

pub(super) fn why(version: Option<PiVersion>) -> String {
    match (PiExposure::for_version(version), version) {
        (PiExposure::Deferred, Some(version)) => {
            format!("Pi {version} lists deferred servers and loads their tools on demand")
        }
        (_, Some(version)) => {
            format!("Pi {version} predates {DEFERRED_FROM}, which has no deferred exposure to list")
        }
        (_, None) => format!(
            "the installed Pi's version was not read, and only {DEFERRED_FROM} or later defers"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_read_from_the_first_triple_and_a_suffix_is_dropped() {
        for (text, expected) in [
            ("1.2.0\n", Some((1, 2, 0))),
            ("pi v1.10.3", Some((1, 10, 3))),
            ("1.2.0-beta.1", Some((1, 2, 0))),
            ("1.2", None),
            ("1.2.3.4", None),
            ("none", None),
        ] {
            assert_eq!(
                PiVersion::parse(text).map(|v| (v.major, v.minor, v.patch)),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn deferred_starts_at_1_2_0_and_an_unreadable_version_stays_direct() {
        let at = |major, minor, patch| {
            Some(PiVersion {
                major,
                minor,
                patch,
            })
        };
        assert_eq!(PiExposure::for_version(at(1, 1, 9)), PiExposure::Direct);
        assert_eq!(PiExposure::for_version(at(0, 99, 0)), PiExposure::Direct);
        assert_eq!(PiExposure::for_version(at(1, 2, 0)), PiExposure::Deferred);
        assert_eq!(PiExposure::for_version(at(2, 0, 0)), PiExposure::Deferred);
        assert_eq!(PiExposure::for_version(None), PiExposure::Direct);
    }

    #[cfg(unix)]
    fn stub_pi(dir: &std::path::Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("pi");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn the_version_probe_reads_stdout_or_stderr_and_gives_up_on_failure_or_a_hang() {
        let dir = tempfile::tempdir().unwrap();
        let short = Duration::from_millis(500);
        let on_stdout = stub_pi(dir.path(), "echo 'pi 1.2.3'");
        assert_eq!(
            version_of(on_stdout, short).map(|v| v.to_string()),
            Some("1.2.3".to_owned())
        );
        let on_stderr = stub_pi(dir.path(), "echo '1.2.0' >&2");
        assert_eq!(
            version_of(on_stderr, short).map(|v| v.to_string()),
            Some("1.2.0".to_owned())
        );
        let failing = stub_pi(dir.path(), "echo '1.2.0'; exit 3");
        assert_eq!(version_of(failing, short), None);
        let hanging = stub_pi(dir.path(), "sleep 30");
        let started = std::time::Instant::now();
        assert_eq!(version_of(hanging, short), None);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the probe waited on a hung pi"
        );
    }
}
