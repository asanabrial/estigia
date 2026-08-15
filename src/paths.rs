//! Where Estigia looks for things on disk.
//!
//! One answer to "where is home", used everywhere — copied from Leteo, which
//! learned it the hard way: agent setup read the environment while other paths
//! asked the `directories` crate, and on Windows those disagree, so overriding
//! the environment moved half the program and left the other half pointing at
//! the real profile.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// The user's home directory.
///
/// The environment wins. `HOME` and `USERPROFILE` are the documented ways to
/// say where home is, and honouring them is what makes it possible to point
/// Estigia somewhere else without touching the real profile.
pub fn home_dir() -> Result<PathBuf> {
    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    if let Some(home) = env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    if let (Some(drive), Some(path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
        let mut home = PathBuf::from(drive);
        home.push(path);
        return Ok(home);
    }
    if let Some(dirs) = directories::UserDirs::new() {
        return Ok(dirs.home_dir().to_path_buf());
    }
    bail!("could not determine the user home directory")
}

/// Strips the `\\?\` verbatim prefix Windows canonicalization adds.
///
/// Canonicalization resolves symlinks, which is what we want, but on Windows it
/// also returns a verbatim path that agent launchers refuse to execute. UNC
/// paths need their network root restored: `\\?\UNC\server\share` becomes
/// `\\server\share`, never the relative path `UNC\server\share`.
pub fn remove_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        let prefix = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
        let unc = [b'U' as u16, b'N' as u16, b'C' as u16, b'\\' as u16];
        if let Some(stripped) = units.strip_prefix(&prefix) {
            let plain = if let Some(network) = stripped.strip_prefix(&unc) {
                [b'\\' as u16, b'\\' as u16]
                    .into_iter()
                    .chain(network.iter().copied())
                    .collect()
            } else {
                stripped.to_vec()
            };
            return PathBuf::from(std::ffi::OsString::from_wide(&plain));
        }
    }
    path
}

/// Whether two paths name the same directory, as far as we can tell.
///
/// One function, because the callers that ask about a directory ask one
/// question. It began as two byte-identical copies in two modules with nothing
/// crossing them — the shape this crate has already paid for once, where "the
/// two of them read the same file and read it differently" cost a whole
/// contract.
///
/// **Not** the question the gate asks. *Is a write inside what this run's oath
/// covers* is [`covers`], and asking it with this — as all three gate paths did
/// — puts everything below the checkout root outside the gate. The two answers
/// are the same only at the root, which is why it went unnoticed.
///
/// Canonicalisation is attempted and its failure is not fatal: a path that
/// cannot be canonicalised still compares literally. The verbatim prefix comes
/// off both sides afterwards, because `canonicalize` adds it on Windows and a
/// path that could not be canonicalised does not have it — so the same
/// directory, spelled two ways and existing on neither side, compared as two.
pub fn same_directory(left: &Path, right: &Path) -> bool {
    let resolve = |path: &Path| {
        remove_windows_verbatim_prefix(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
    };
    let (left, right) = (resolve(left), resolve(right));
    if cfg!(windows) {
        // Case only matters when canonicalisation did *not* happen: it answers
        // with the real spelling on disk, so two spellings of a live directory
        // already agree by the time they get here. The fallback path is where
        // this decides — and it decides the wrong way round. `covered` holding
        // one spelling and `repo_dir` another made `holders_of` find nobody,
        // and nobody holding a checkout is not a denial: it means *this push is
        // nothing to do with Estigia*, so the boundary goes through unasked.
        // A path this process cannot resolve loosening the gate is the declared
        // asymmetry run backwards, which is the fault `standdown` opens by
        // naming.
        //
        // Folded the whole way, because both transports already do and this was
        // the only place that did not. `branch::spelling` calls
        // `to_lowercase()`, and the Python's `fold_case` calls `.lower()` — the
        // comment here used to claim ASCII folding was "what the transport does
        // too", and that claim was simply wrong about both of them.
        //
        // It mattered because both are live: the harness is this Rust and the
        // transport that runs is that Python, and two live components folding
        // differently answer *is this the same checkout* differently. Windows
        // folds more than ASCII, so `Café` and `CAFÉ` are one directory there —
        // one that this function called two.
        left.as_os_str()
            .to_string_lossy()
            .to_lowercase()
            .eq(&right.as_os_str().to_string_lossy().to_lowercase())
    } else {
        left == right
    }
}

/// Whether a claim over `covered` reaches work happening in `working_dir`.
///
/// Three places asked this and all three asked it with [`same_directory`], which
/// answers a different question. A claim covers a *checkout*; `same_directory`
/// compares two directories. One step below the checkout root the two answers
/// part company, and the gate took the wrong one.
///
/// Measured, with a run pointer holding issue #42 over a checkout: `estigia gate
/// Write` at the root reached the tracker, and the same call from `src/` — the
/// same checkout, one directory down — came back
/// *"outside — this run's claim covers a different checkout than this one"*.
/// So did `git push` from there. Every write and every irreversible boundary an
/// agent makes below the root was outside the gate, and the message asserted
/// something false to say so: `src/` is not a different checkout.
///
/// It is reachable everywhere except the pre-push hook, which git always runs
/// from the top level. An agent's payload carries its own working directory, a
/// package directory in a monorepo is one, and so is anywhere a person stands
/// when they type `estigia gate`.
///
/// Downwards only. A claim over an isolated worktree must not start covering the
/// checkout that contains it — that would let a claim reach work it was never
/// made over, which is the direction that costs the guarantee rather than a
/// tracker read.
pub fn covers(covered: &Path, working_dir: &Path) -> bool {
    coverage_depth(covered, working_dir).is_some()
}

/// The same question, answered with *how closely* rather than only whether.
///
/// Nesting is what makes the difference matter. Two runs of the same repository
/// each hold the base checkout in their pointer and each get an isolated
/// worktree, and an operator is free to put those worktrees inside the
/// repository — `Worktree location` is their text and nothing refuses a nested
/// one. Then work in run A's own worktree is covered twice: exactly, by A's
/// worktree, and from a long way up, by B's base checkout.
///
/// Answering that with a plain yes made both runs holders of A's worktree, and
/// `holders_of`'s many-holders arm denies. Measured, with two pointers over one
/// repository: a push from `wt-a` came back *"2 runs on this machine hold this
/// checkout"* — the deny landing on the one directory isolation exists to give
/// each run. The pair already collided at the base checkout, so this was the
/// only place either of them could still work.
///
/// So the depth is the answer, and the closest cover wins — the rule every
/// path-scoped system uses. Genuine ambiguity survives it: two runs covering one
/// directory *at the same depth* are still two holders, and still a refusal.
///
/// The count is of `covered`, not of `working_dir`: it says how specific the
/// claim is, which is what has to be compared between runs.
pub fn coverage_depth(covered: &Path, working_dir: &Path) -> Option<usize> {
    let resolve = |path: &Path| {
        remove_windows_verbatim_prefix(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
    };
    let (resolved, working_dir) = (resolve(covered), resolve(working_dir));
    let inside = if cfg!(windows) {
        // Folded, for the reason [`same_directory`] folds: the fallback path is
        // where this decides, and a spelling this process could not resolve
        // must not be what takes work out of the gate. Component-wise still —
        // through `Path`, not through a string prefix, so a sibling named
        // `srcbin` is not read as living inside `src`.
        let fold = |path: &Path| {
            std::path::PathBuf::from(path.as_os_str().to_string_lossy().to_lowercase())
        };
        fold(&working_dir).starts_with(fold(&resolved))
    } else {
        working_dir.starts_with(&resolved)
    };
    // `same_directory` first, because it is the one that already knows what to
    // do with a path neither side can canonicalise, and the exact match is the
    // deepest cover there is.
    (inside || same_directory(covered, &working_dir)).then(|| resolved.components().count())
}

/// Where a write will actually land, as far as this process can place it.
///
/// [`coverage_depth`] was written for **working directories**, which exist, and
/// its fallback resolves an unresolvable path literally. A write target usually
/// does not exist yet — that is the ordinary case, not the edge — so handing it
/// to that comparison answers on two different spellings of one path:
///
/// - `<root>/decoy/../repo/src/main.rs` compared literally is not inside
///   `<root>/repo`, and the write lands there anyway;
/// - a checkout reached through a junction resolves on the covered side and not
///   on the target side, so a **new** file inside it reads as outside while an
///   **existing** one reads as inside. The only difference is whether the file
///   is there yet.
///
/// Both were measured against the gate this feeds, where a wrong `outside`
/// removes the gate. So: collapse the spelling first, then resolve as much of
/// the filesystem as exists.
///
/// `None` when the path cannot be placed at all, and the caller must read that
/// as **inside**. `same_directory` states the rule this follows: *a path this
/// process cannot resolve loosening the gate is the declared asymmetry run
/// backwards.*
pub fn placed(target: &Path) -> Option<PathBuf> {
    // A drive has more than one name, and only one of them can be compared.
    //
    // Windows serves every local drive as an administrative share, so
    // `\\localhost\C$\repo\src\main.rs` is the same file as `C:\repo\src\main.rs`
    // with no link in it anywhere. `canonicalize` keeps the vocabulary it was
    // given — the share resolves to `\\?\UNC\localhost\C$\...` and the covered
    // checkout to `C:\...` — so one file was compared under two spellings,
    // neither a prefix of the other, and the gate stood aside for a write that
    // landed inside the claimed checkout.
    //
    // Mapping a share back to the drive it serves means asking the machine what
    // it is sharing, which is a question this process cannot answer offline and
    // a wrong answer here removes a gate. So a path that names anything other
    // than a drive is declined, and the caller reads that as *inside*.
    if let Some(std::path::Component::Prefix(prefix)) = target.components().next()
        && !matches!(
            prefix.kind(),
            std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
        )
    {
        return None;
    }
    // `..` is where the two platforms genuinely disagree, and answering for the
    // wrong one is what puts the spelling and the landing in different places.
    //
    // Windows collapses it **lexically**, before touching the filesystem, so
    // `outside/link/../x` names `outside/x` however `link` is defined. POSIX
    // resolves the link first and applies `..` to what it resolved to, so the
    // same spelling lands beside the link's *target* — inside a checkout, if
    // that is where the link points. Reading it lexically on POSIX classified a
    // repository write as outside and stood the gate aside, on four of the six
    // platforms this crate ships.
    //
    // So each `..` is applied the way the platform applies it: to the resolved
    // prefix where the filesystem is what decides, to the spelling where it is
    // not. Never past the root — a path that climbs out of what it names cannot
    // be placed.
    let mut lexical = PathBuf::new();
    for component in target.components() {
        match component {
            std::path::Component::ParentDir => {
                if cfg!(unix)
                    && let Ok(resolved) = lexical.canonicalize()
                {
                    lexical = resolved;
                }
                if !lexical.pop() {
                    return None;
                }
            }
            // No arm for `CurDir`. There was one, and both reviewers of this
            // change measured it surviving deletion with the suite green: an
            // interior `.` never reaches here because `components` drops it, and
            // a leading one only survives on a relative path, where `pop` eats it
            // and the walk up ends at the same `None` either way. A branch no
            // input can distinguish is a branch that reads as a guard and is not
            // one.
            other => lexical.push(other.as_os_str()),
        }
    }
    // Then the filesystem, for as much of it as is there. A junction is not a
    // spelling, and the deepest existing ancestor is the most this process can
    // honestly resolve — resolving it puts both sides of the comparison in the
    // same vocabulary.
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut probe = lexical.as_path();
    loop {
        // An entry that is **there** and will not resolve is not the same as no
        // entry at all, and walking up past it is how a dangling symlink got
        // placed at its own spelling: `<outside>/alias.rs` pointing at
        // `<repo>/src/planted.rs` answered *outside*, and writing through it
        // created the file inside the checkout. `canonicalize` fails the same
        // way for both, so the difference has to be asked for.
        if probe.symlink_metadata().is_ok() && probe.canonicalize().is_err() {
            return None;
        }
        if let Ok(real) = probe.canonicalize() {
            let mut placed = remove_windows_verbatim_prefix(real);
            for part in suffix.iter().rev() {
                placed.push(part);
            }
            return Some(placed);
        }
        let parent = probe.parent()?;
        suffix.push(probe.file_name()?.to_os_string());
        probe = parent;
    }
}

/// Rejects a path that is not absolute, naming which one it was.
pub fn require_absolute(path: &Path, name: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("{name} must be absolute, got {}", path.display());
    }
    Ok(())
}

/// A temporary file this process alone will use.
///
/// Every staged body used to be a **fixed** name under the temporary directory —
/// `issue-flow-heartbeat-7.md`, and one with no discriminator at all,
/// `issue-flow-link-probe`. On Windows that directory is per-user and nothing
/// came of it. On Unix it is `/tmp`, shared by everything on the machine, and
/// this crate exists for the case of two runs on one machine.
///
/// The worst of them was the link probe: `expected-target` writes a symlink's
/// text there and hashes the file, so two runs overlapping meant one of them
/// recorded the *other's* blob for its own path — a wrong entry in the manifest
/// a reviewer's approval is bound to.
///
/// The process id is enough for that, because two runs are two processes. It is
/// not a secret and does not pretend to be: the transport uses `mkstemp` and
/// gets unpredictability with it, which this cannot without a dependency. What
/// keeps the write itself honest is [`replace_atomically`] — it renames over
/// the destination, and a rename does not follow a symlink there.
pub fn scratch_file(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("issue-flow-{}-{name}", std::process::id()))
}

/// The mark on the temporary a write is staged in.
///
/// Named here, beside the code that chooses it, because a second reader has to
/// recognise one: the uninstaller walks the skill directory afterwards and
/// reports what it finds as the operator's. A residue Estigia left is not, and
/// the spelling that decides it must not be written twice.
const STAGED: &str = ".estigia-";

/// Whether this is a file a write of ours staged and did not finish.
///
/// The cleanup in [`replace_atomically`] runs when the write **fails**. A
/// process killed between the create and the rename leaves the temporary where
/// it is, and nothing sweeps it — so `SKILL.estigia-4321.tmp` sat in the skill
/// directory, and the uninstaller told the operator it was one of theirs. In
/// the one sentence that exists to answer *did it touch my things?*.
pub fn is_staged_write(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(STAGED) && name.ends_with(".tmp"))
}

/// Writes `desired` to `path` in one step, as far as a reader can tell.
///
/// These are not Estigia's files. `settings.json` belongs to Claude Code, the
/// skill is read while a prompt is assembled, and the push hook is read by git,
/// and all of them are read while Estigia is writing. `settings.json` is the
/// clearest case: it belongs to Claude Code, and it
/// is read while the agent runs — so a truncating write gives the agent a window
/// in which its own configuration is half a file. The failure would land on the
/// user's agent, at a moment that has nothing to do with Estigia, and look like
/// the agent's fault.
///
/// Written beside the target and renamed over it. The temporary carries this
/// process's id so two runs cannot collide on the name, and stays in the same
/// directory so the rename does not cross a filesystem.
pub fn replace_atomically(path: &Path, desired: &str) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("{}{}.tmp", &STAGED[1..], std::process::id()));
    let write = write_and_sync(&temporary, desired)
        .and_then(|()| std::fs::rename(&temporary, path))
        .and_then(|()| sync_directory(path));
    if write.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    write
}

/// Writes the bytes and asks the filesystem to keep them.
///
/// A rename is atomic against another *process*; it is not durable against a
/// power cut. Without this a crash can leave the rename recorded and the
/// contents not — and for a run pointer that means a claim reading as absent, so
/// a run that swore reads as one that never did. Fail-open, on the one event
/// nobody gets to retry.
fn write_and_sync(path: &Path, desired: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(desired.as_bytes())?;
    file.sync_all()
}

/// Asks the filesystem to keep the *rename* as well.
///
/// The directory entry is a write of its own: syncing the file and not its
/// parent leaves durable contents under a name that may not survive.
///
/// Best effort, deliberately. Windows will not open a directory as a file, and a
/// platform that refuses the handle is not a reason to fail a write that has
/// already landed.
fn sync_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && let Ok(directory) = std::fs::File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// One path, quoted so a shell reads it as itself.
///
/// Single quotes, not double. Inside double quotes a shell still expands
/// `$(…)`, `` `…` `` and `${…}`, and still reads `\` as an escape — and this is
/// a **path**, which may contain any of them. `$` and a backtick are legal in a
/// directory name on every platform Estigia installs on, Windows included.
///
/// Both places that write a path into a command line use this one. They were
/// two copies of `format!("\"{}\"", …)`, in `harness::guard` and in
/// `setup::render`, and only one of them was found: a checkout whose binary
/// lived under `/tmp/$(id)/` wrote a pre-push hook that ran `id` on every push
/// and then looked for the gate at whatever came back.
///
/// It also settles the thing `setup::render`'s own comment says this crate
/// learned the expensive way — *bash treats a backslash as an escape* — for
/// good rather than for the spellings that happen not to be special: inside
/// single quotes nothing is. The only character to handle is a single quote
/// itself: close, escape one, reopen.
pub fn shell_quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// The same, for a shell that escapes its own quote by doubling it.
///
/// PowerShell has no `'\''`: inside a single-quoted string the only escape is
/// `''`. One function each rather than a flag, because the two languages differ
/// in exactly this and a caller that picked the wrong one would produce a file
/// that parses everywhere except where it runs.
///
/// Both exist because a path may hold an apostrophe and nothing refuses one:
/// `setup::UNQUOTABLE` deliberately permits it, having *measured* that
/// `"C:\Users\O'Brien\estigia.exe"` survives **double** quoting — which is what
/// the agent settings hooks use. Cline's hook is single-quoted, and there the
/// same path produced `exec '/home/o'brien/estigia' …`, which `sh -n` answers
/// with *unexpected EOF while looking for matching `''*. Not a gate that
/// misbehaves: a gate whose script is not shell.
pub fn powershell_quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `placed` hands back is a spelling anything else can be compared to.
    ///
    /// Both reviewers of this change measured the same thing: the line that
    /// strips the verbatim prefix could be deleted and the whole suite stayed
    /// green, because every caller today puts *both* sides of its comparison
    /// through here and two verbatim spellings compare as well as two literal
    /// ones. That makes it unmeasured, not unimportant — the next caller to
    /// compare this output against a path that came from anywhere else inherits
    /// `\\?\C:\x` against `C:\x`, which is the exact two-vocabularies failure
    /// this function exists to end. So the guarantee is asserted here, on the
    /// output itself, rather than left resting on what the callers happen to do.
    #[test]
    fn a_placed_path_carries_no_verbatim_prefix() {
        let root = tempfile::tempdir().expect("a root");
        let deep = root.path().join("a").join("b");
        std::fs::create_dir_all(&deep).expect("a directory to place");
        // A tail that does not exist yet, which is the ordinary case for a write
        // target and the one that takes the literal-suffix road.
        let target = deep.join("not-yet.rs");

        let placed = placed(&target).expect("a real directory places");
        assert!(
            !placed.display().to_string().starts_with(r"\\?\"),
            "placed handed back a verbatim spelling: {placed:?}"
        );
        assert!(
            placed.ends_with("not-yet.rs"),
            "placed lost the part of the path that does not exist yet: {placed:?}"
        );
    }

    #[test]
    fn home_resolves_to_an_absolute_path() {
        let home = home_dir().expect("a home directory is available in the test environment");
        assert!(home.is_absolute(), "home must be absolute, got {home:?}");
    }

    /// The same directory, however it is spelled, on both sides.
    ///
    /// Two byte-identical copies of this lived in two modules — the gate used
    /// one, the push guard the other, and nothing crossed them. They agreed the
    /// day they were written, which is the only day that shape ever agrees.
    ///
    /// The verbatim prefix is why it is worth a test rather than a `==`:
    /// `canonicalize` adds `\?\` on Windows and a path that does not exist
    /// cannot be canonicalised, so the same directory spelled two ways compared
    /// as two.
    #[test]
    fn one_directory_spelled_two_ways_is_one_directory() {
        let here = std::env::current_dir().expect("a working directory");
        let canonical = here.canonicalize().unwrap_or_else(|_| here.clone());
        let plain = remove_windows_verbatim_prefix(canonical.clone());

        for (name, left, right) in [
            ("itself", here.clone(), here.clone()),
            (
                "canonical against as-typed",
                canonical.clone(),
                here.clone(),
            ),
            ("verbatim against canonical", plain, canonical.clone()),
            // Neither side exists, so neither canonicalises and the spelling is
            // all there is to compare.
            (
                "two spellings of a directory that is not there",
                here.join("no-such-directory"),
                canonical.join("no-such-directory"),
            ),
        ] {
            assert!(
                same_directory(&left, &right),
                "{name}: {} and {} are one directory and did not compare as one",
                left.display(),
                right.display()
            );
        }

        // And two directories are still two.
        assert!(!same_directory(&here, &here.join("src")));
    }

    #[test]
    #[cfg(windows)]
    fn a_checkout_this_process_cannot_resolve_is_still_one_checkout() {
        // The fallback is the only place case decides: `canonicalize` answers
        // with the real spelling on disk, so a live directory spelled two ways
        // agrees without any folding. Measured before the fix — `H:\…\gone` and
        // `h:\…\GONE` compared as two, and two is the answer that *opens* the
        // gate, because `holders_of` finding nobody means the push is nothing to
        // do with Estigia.
        let here = std::env::current_dir().expect("a working directory");
        let missing = here.join("no-such-directory").join("worktrée");
        let shouted = PathBuf::from(missing.display().to_string().to_uppercase());
        assert_ne!(
            missing, shouted,
            "the two spellings are the same bytes, so this test measures nothing"
        );
        assert!(
            same_directory(&missing, &shouted),
            "{} and {} are one directory on this platform and compared as two",
            missing.display(),
            shouted.display()
        );
    }

    #[test]
    fn an_empty_variable_does_not_count_as_an_answer() {
        // Some shells export HOME as the empty string rather than unsetting it.
        // Taking that literally resolves every Estigia path to the filesystem
        // root. The variables are process-wide, so this reads the rule rather
        // than mutating them.
        let empty = std::ffi::OsString::new();
        assert!(Some(empty).filter(|value| !value.is_empty()).is_none());
    }

    #[test]
    fn a_verbatim_prefix_is_removed_and_a_plain_path_is_left_alone() {
        #[cfg(windows)]
        {
            assert_eq!(
                remove_windows_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\a\estigia.exe")),
                PathBuf::from(r"C:\Users\a\estigia.exe")
            );
            assert_eq!(
                remove_windows_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\estigia.exe")),
                PathBuf::from(r"\\server\share\estigia.exe")
            );
        }
        assert_eq!(
            remove_windows_verbatim_prefix(PathBuf::from("/usr/local/bin/estigia")),
            PathBuf::from("/usr/local/bin/estigia")
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_verbatim_prefix_is_removed_without_decoding_windows_path_units() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        let verbatim = [
            b'\\' as u16,
            b'\\' as u16,
            b'?' as u16,
            b'\\' as u16,
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xd800,
        ];
        let plain =
            remove_windows_verbatim_prefix(PathBuf::from(std::ffi::OsString::from_wide(&verbatim)));
        assert_eq!(
            plain.as_os_str().encode_wide().collect::<Vec<_>>(),
            [b'C' as u16, b':' as u16, b'\\' as u16, 0xd800]
        );
    }
}

#[cfg(test)]
mod staged_tests {
    use super::*;

    #[test]
    fn the_name_a_write_stages_under_is_one_the_sweep_recognises() {
        // The two halves of one rule: the name `replace_atomically` chooses, and
        // the question the uninstaller asks about a file it finds. Written
        // apart, a rename of one leaves the other reporting Estigia's own
        // residue as the operator's — which is what it did.
        let root = tempfile::tempdir().expect("a temporary root");
        for name in ["SKILL.md", "settings.json", "pre-push", "estigia.local.md"] {
            let target = root.path().join(name);
            // Fails on the rename only if something is in the way; what this
            // measures is the staged name, so the write is left to succeed and
            // the directory read for anything it left.
            replace_atomically(&target, "x").expect("the write lands");
            assert!(
                !is_staged_write(&target),
                "{name} is its own destination and reads as a staged write"
            );
        }
        assert!(
            std::fs::read_dir(root.path())
                .expect("the directory reads")
                .flatten()
                .all(|entry| !is_staged_write(&entry.path())),
            "a write that succeeded left a staged file behind"
        );

        // And the name it would have used is recognised. Built the way
        // `replace_atomically` builds it rather than typed out, so a change to
        // one is a change to both.
        let staged = root.path().join("SKILL.md").with_extension(format!(
            "{}{}.tmp",
            &STAGED[1..],
            std::process::id()
        ));
        assert!(
            is_staged_write(&staged),
            "the name a write stages under is not one the sweep would recognise: {}",
            staged.display()
        );
        // The floor: it does not answer yes to everything.
        for theirs in ["NOTAS.md", "estigia.local.md", "notes.tmp", "SKILL.md"] {
            assert!(
                !is_staged_write(&root.path().join(theirs)),
                "{theirs} would be swept as Estigia's own residue"
            );
        }
    }
}
