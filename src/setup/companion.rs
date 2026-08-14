//! Tools Estigia works better beside, declared rather than special-cased.
//!
//! A table, not `if leteo`. And Estigia does **not** reimplement a companion's
//! installer: Leteo's already detects the target triple, downloads, checks
//! `SHA256SUMS` and refuses to install without verifying. Copying that is two
//! copies of the trust logic, and the second one is always the stale one.
//!
//! So the order of preference is:
//!
//! 1. **Detect and name the command.** Trust surface: none. The default.
//! 2. `cargo install <crate>`, when `cargo` is on the path.
//! 3. `curl … | sh` only after an explicit opt-in, with the URL shown first.
//!
//! Estigia never runs step 3 on its own. Declare the dependency; let whoever
//! owns it install it.
//!
//! Step 3 has no code, and the URL it would show is not carried here. A field
//! holding an installer nothing offers is a field that reads as though the
//! offer exists.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;

use crate::outcome::{NoCommandReason, Resolution};

/// A tool Estigia can detect and name, but does not install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Companion {
    /// The name typed on the command line, and the executable probed for.
    pub slug: &'static str,
    /// What it does, in a clause, for the line that reports it.
    pub purpose: &'static str,
    /// The crate that carries it, when it is published.
    pub crate_name: &'static str,
    /// Whether a release exists yet.
    ///
    /// Leteo's `main` had no commits on 2026-07-31, so "not published yet" is a
    /// first-class result rather than a detection failure — the difference
    /// between "I could not find it" and "there is nothing to find" is the
    /// difference between a bug report and a note.
    pub published: bool,
}

/// Every companion Estigia knows about.
pub const COMPANIONS: &[Companion] = &[Companion {
    slug: "leteo",
    purpose: "persistent memory across sessions",
    crate_name: "leteo",
    published: false,
}];

/// What probing found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionState {
    /// On the path, and this is what it reports.
    Present {
        /// The trimmed first line of `<slug> --version`.
        version: String,
    },
    /// Not on the path, but installable today.
    Absent,
    /// Not on the path, and there is nothing published to install.
    ///
    /// Carries no resolution that runs now, on purpose — see
    /// [`CompanionState::resolution`].
    Unpublished,
}

impl CompanionState {
    /// What to do about this state.
    ///
    /// The ratchet: a command is named only when running it discharges the
    /// block. [`Self::Unpublished`] names none, because none exists — and it
    /// says which kind of gap that is rather than suggesting an install that
    /// would 404.
    pub fn resolution(&self, companion: &Companion) -> Resolution {
        match self {
            Self::Present { .. } => {
                Resolution::run(format!("estigia setup --companion {}", companion.slug))
            }
            Self::Absent => Resolution::run(format!("cargo install {}", companion.crate_name)),
            Self::Unpublished => Resolution::no_command(
                NoCommandReason::WorldAction,
                format!(
                    "{} has no published release yet; when it does, run `estigia setup \
                     --companion {}`",
                    companion.slug, companion.slug
                ),
            ),
        }
    }

    /// One line for a person, matching the shape the handoff specified.
    pub fn describe(&self, companion: &Companion) -> String {
        let head = format!("{} — {}", companion.slug, companion.purpose);
        match self {
            Self::Present { version } => format!("{head}\n  found: {version}"),
            Self::Absent => format!("{head}\n  not found.\n  {}", self.resolution(companion)),
            Self::Unpublished => format!(
                "{head}\n  not found, and no release published yet.\n  Nothing to do now. When \
                 there is:  estigia setup --companion {}",
                companion.slug
            ),
        }
    }
}

/// Asks the system whether a companion is there.
///
/// Detection only. Nothing is downloaded and nothing is installed, which is
/// what makes the trust surface of the default path exactly zero.
///
/// The program is resolved against `PATH` by hand and run by absolute path.
/// `Command::new("leteo")` would be shorter and wrong on Windows, where process
/// creation searches the **current directory first**: `estigia status` run
/// inside a repository somebody else wrote would execute a `leteo.exe` sitting
/// beside its README. A tool whose job is to say what is installed must not
/// become a way to run what is merely lying around.
pub fn probe_companion(companion: &Companion) -> CompanionState {
    let Some(program) = resolve_on_path(companion.slug) else {
        return if companion.published {
            CompanionState::Absent
        } else {
            CompanionState::Unpublished
        };
    };
    match Command::new(program).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let version = text.lines().next().unwrap_or_default().trim().to_owned();
            CompanionState::Present {
                version: if version.is_empty() {
                    companion.slug.to_owned()
                } else {
                    version
                },
            }
        }
        // A companion that answers `--version` with a failure is still there,
        // and reporting it absent would send somebody to install what they
        // already have.
        Ok(_) => CompanionState::Present {
            version: companion.slug.to_owned(),
        },
        // The program resolved on PATH and still would not run — a broken
        // binary, a permission bit, an exec format error. It is there, so
        // sending somebody to install it would be naming a dead end.
        Err(_) => CompanionState::Present {
            version: companion.slug.to_owned(),
        },
    }
}

/// The companion a slug names, or `None`.
pub fn find_companion(slug: &str) -> Option<&'static Companion> {
    COMPANIONS.iter().find(|companion| companion.slug == slug)
}

/// Finds `program` on `PATH`, returning an absolute path or nothing.
///
/// Only `PATH` — never the current directory, and never a relative entry in
/// `PATH` either, which would reintroduce exactly what this avoids. On Windows
/// the extensions in `PATHEXT` are tried, falling back to the usual three when
/// it is unset.
pub(crate) fn resolve_on_path(program: &str) -> Option<PathBuf> {
    resolve_on_path_with(
        program,
        env::var_os("PATH").as_deref(),
        env::var_os("PATHEXT").as_deref(),
    )
}

fn resolve_on_path_with(
    program: &str,
    path: Option<&OsStr>,
    pathext: Option<&OsStr>,
) -> Option<PathBuf> {
    let extensions: Vec<OsString> = if cfg!(windows) {
        pathext
            .unwrap_or_else(|| OsStr::new(".COM;.EXE;.BAT;.CMD"))
            .to_string_lossy()
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(OsString::from)
            .collect()
    } else {
        vec![OsString::new()]
    };

    path.map(|path| env::split_paths(path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|directory| directory.is_absolute())
        .flat_map(|directory| {
            extensions
                .iter()
                .map(|extension| {
                    let mut name = OsString::from(program);
                    name.push(extension);
                    directory.join(name)
                })
                .collect::<Vec<_>>()
        })
        .find(|candidate| is_executable_file(candidate))
}

#[cfg(windows)]
fn is_executable_file(candidate: &std::path::Path) -> bool {
    candidate.is_file()
}

#[cfg(unix)]
fn is_executable_file(candidate: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    candidate
        .metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESOLVER_FIXTURE: &str = "ESTIGIA_RESOLVER_FIXTURE";
    const RESOLVER_FIXTURE_TEST: &str = "setup::companion::tests::path_resolution_process_fixture";

    #[test]
    fn path_resolution_process_fixture() {
        let Ok(program) = env::var(RESOLVER_FIXTURE) else {
            return;
        };
        assert!(
            resolve_on_path(&program).is_none(),
            "resolved {program:?} from a hostile working directory or relative PATH"
        );
    }

    fn assert_real_resolver_ignores(program: &str, directory: &std::path::Path, path: &OsStr) {
        let executable = env::current_exe().expect("the test executable");
        let output = Command::new(executable)
            .args(["--exact", RESOLVER_FIXTURE_TEST, "--nocapture"])
            .current_dir(directory)
            .env("PATH", path)
            .env("PATHEXT", ".EXE")
            .env(RESOLVER_FIXTURE, program)
            .output()
            .expect("run the resolver fixture");
        assert!(
            output.status.success(),
            "the real resolver accepted a hostile cwd/PATH:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    fn plant_executable(directory: &std::path::Path, program: &str) {
        let planted = directory.join(if cfg!(windows) {
            format!("{program}.EXE")
        } else {
            program.to_owned()
        });
        std::fs::write(&planted, "not a real program").expect("plant the file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&planted, std::fs::Permissions::from_mode(0o755))
                .expect("make the planted file executable");
        }
    }

    #[test]
    fn an_unpublished_companion_names_no_command_to_run_now() {
        // The whole point: suggesting `cargo install leteo` for a crate that
        // does not exist is naming a dead end, which the ratchet forbids.
        let leteo = find_companion("leteo").expect("leteo is a declared companion");
        assert!(!leteo.published);
        let resolution = CompanionState::Unpublished.resolution(leteo);
        assert!(matches!(
            resolution,
            Resolution::NoCommand {
                reason: NoCommandReason::WorldAction,
                ..
            }
        ));
    }

    #[test]
    fn an_absent_but_published_companion_names_a_command_that_exists() {
        let leteo = find_companion("leteo").expect("leteo is a declared companion");
        let published = Companion {
            published: true,
            ..*leteo
        };
        assert_eq!(
            CompanionState::Absent.resolution(&published),
            Resolution::run("cargo install leteo")
        );
    }

    #[test]
    fn a_missing_binary_of_an_unpublished_companion_is_not_reported_as_absent() {
        let companion = Companion {
            slug: "estigia-companion-that-does-not-exist",
            purpose: "nothing",
            crate_name: "nothing",
            published: false,
        };
        assert_eq!(probe_companion(&companion), CompanionState::Unpublished);
    }

    #[test]
    fn the_unpublished_line_says_there_is_nothing_to_do() {
        let leteo = find_companion("leteo").expect("leteo is a declared companion");
        let described = CompanionState::Unpublished.describe(leteo);
        assert!(described.contains("no release published yet"));
        assert!(described.contains("Nothing to do now"));
    }

    #[test]
    fn a_program_beside_the_working_directory_is_not_resolved() {
        // The Windows footgun, pinned. `CreateProcess` searches the current
        // directory before PATH, so `estigia status` run inside a repository
        // somebody else wrote would execute a binary sitting beside its README.
        // Resolution goes through PATH only, and this stays true on every
        // platform so the guard cannot rot on the ones that never had the bug.
        let directory = tempfile::tempdir().expect("a temporary directory");
        plant_executable(directory.path(), "estigia-planted");
        let empty_path =
            env::join_paths(std::iter::empty::<&std::path::Path>()).expect("an empty PATH");
        assert_real_resolver_ignores("estigia-planted", directory.path(), &empty_path);
    }

    #[test]
    fn a_relative_path_entry_is_ignored() {
        // A relative PATH entry is the same hole wearing a different hat.
        let directory = tempfile::tempdir().expect("a temporary directory");
        plant_executable(directory.path(), "estigia-planted");
        let path = env::join_paths([std::path::Path::new(".")]).expect("a relative PATH");
        assert_real_resolver_ignores("estigia-planted", directory.path(), &path);
    }

    #[cfg(windows)]
    #[test]
    fn explicit_pathext_selects_a_program_from_an_absolute_path_entry() {
        let directory = tempfile::tempdir().expect("a PATH directory");
        let executable = directory.path().join("estigia-path-probe.EXE");
        std::fs::write(&executable, "fixture").expect("write the fixture executable");
        let path = env::join_paths([directory.path()]).expect("a PATH");

        assert_eq!(
            resolve_on_path_with("estigia-path-probe", Some(&path), Some(OsStr::new(".EXE")),),
            Some(executable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_non_executable_candidate_cannot_shadow_a_later_executable() {
        use std::os::unix::fs::PermissionsExt;

        let first = tempfile::tempdir().expect("a first PATH directory");
        let second = tempfile::tempdir().expect("a second PATH directory");
        let blocked = first.path().join("estigia-path-probe");
        let executable = second.path().join("estigia-path-probe");
        std::fs::write(&blocked, "blocked").expect("write the blocked candidate");
        std::fs::write(&executable, "executable").expect("write the executable candidate");
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o644))
            .expect("remove execute permission");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
            .expect("grant execute permission");
        let path = env::join_paths([first.path(), second.path()]).expect("a PATH");

        assert_eq!(
            resolve_on_path_with("estigia-path-probe", Some(&path), None),
            Some(executable)
        );
    }

    #[test]
    fn every_companion_that_is_unpublished_has_somewhere_to_point_later() {
        for companion in COMPANIONS {
            assert!(
                !companion.crate_name.is_empty(),
                "{} must name the crate that will carry it",
                companion.slug
            );
        }
    }
}
