//! Where Estigia looks for things on disk.
//!
//! One answer to "where is home", used everywhere: agent setup reads the
//! environment while other paths would ask the `directories` crate, and on
//! Windows those disagree, so overriding the environment must move the whole
//! program or none of it.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use sha2::Digest as _;

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
        // this decides, and a spelling this process cannot resolve must not be
        // what takes work out of the gate: `holders_of` finding nobody means
        // *this push is nothing to do with Estigia*, so the boundary goes
        // through unasked.
        //
        // Folded the whole way, because both transports already do and this was
        // the only place that did not — two live components folding differently
        // answer *is this the same checkout* differently.
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
/// answers a different question: a claim covers a *checkout*, one step below
/// the root the two answers part company, and the gate took the wrong one.
///
/// Reachable everywhere except the pre-push hook, which git always runs from
/// the top level — an agent's payload carries its own working directory, a
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

/// Canonicalises a path through the nearest ancestor that exists.
///
/// `canonicalize` resolves a path that exists through its symlinks and falls
/// back to the literal spelling for one that does not, so a home and a
/// candidate under it that nothing has created yet arrive in two vocabularies.
/// Containment compares exactly that pair, and the mismatch is what answers
/// `false` for a value plainly under a symlinked home. Resolving each side to
/// its nearest existing ancestor and re-appending the tail puts both in one
/// vocabulary without inventing a path.
fn resolved_through_existing_ancestor(path: &Path) -> PathBuf {
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut probe = path;
    loop {
        if let Ok(real) = probe.canonicalize() {
            let mut resolved = remove_windows_verbatim_prefix(real);
            for part in suffix.iter().rev() {
                resolved.push(part);
            }
            return resolved;
        }
        match (probe.parent(), probe.file_name()) {
            (Some(parent), Some(name)) => {
                suffix.push(name.to_os_string());
                probe = parent;
            }
            _ => return remove_windows_verbatim_prefix(path.to_path_buf()),
        }
    }
}

/// The same question, answered with *how closely* rather than only whether.
///
/// Nesting is what makes the difference matter: two runs of the same repository
/// each get an isolated worktree, and an operator may put those worktrees inside
/// the repository, so work in run A's own worktree is covered twice. Answering
/// that with a plain yes made both runs holders, and the many-holders arm
/// denies. The depth is the answer, and the closest cover wins; genuine
/// ambiguity survives it — two runs covering one directory *at the same depth*
/// are still two holders, and still a refusal.
///
/// The count is of `covered`, not of `working_dir`: it says how specific the
/// claim is, which is what has to be compared between runs.
pub fn coverage_depth(covered: &Path, working_dir: &Path) -> Option<usize> {
    let (resolved, working_dir) = (
        resolved_through_existing_ancestor(covered),
        resolved_through_existing_ancestor(working_dir),
    );
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
            // A leading `.` is dropped rather than pushed, and the difference is
            // not cosmetic. `push(".")` on an empty buffer yields `"."`, which
            // canonicalises to the working directory — so `placed(".\x")` would
            // answer `Some(<cwd>\x)` while `placed("x")`, the same file, answers
            // `None`. The loose one of the two is the answer that can classify a
            // path as outside; `components` only drops an *interior* `.`.
            std::path::Component::CurDir => {}
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
            // A drive has more than one name, and only one of them can be
            // compared. Windows serves every local drive as an administrative
            // share, so `\\localhost\C$\repo\src\main.rs` is the same file as
            // `C:\repo\src\main.rs` — one file compared under two spellings,
            // neither a prefix of the other, and the gate stood aside.
            //
            // The check is on the **landing**, not on the spelling handed in:
            // reading the input's first component misses every path that
            // arrives wearing a drive letter and resolves onto a share anyway
            // (a mapped drive, a directory link pointing there).
            //
            // Mapping a share back to the drive it serves cannot be answered
            // offline, and a wrong answer removes a gate. So a landing that is
            // not on a drive is declined, and the caller reads that as
            // **inside**.
            //
            // Written as a requirement rather than a rejection, deliberately: a
            // landing with no prefix component at all would sail past this and
            // be compared as though it were relative. Nothing this process can
            // canonicalise produces one, so the branch is written the way whose
            // failure is a refusal.
            if cfg!(windows)
                && !matches!(
                    placed.components().next(),
                    Some(std::path::Component::Prefix(prefix))
                        if matches!(
                            prefix.kind(),
                            std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                        )
                )
            {
                return None;
            }
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

/// A temporary file no other call in this machine will use.
///
/// The process id discriminates it, because two runs are two processes — and a
/// fixed name under a shared temporary directory meant two runs overlapping
/// recorded one another's bytes for their own paths. A counter discriminates
/// the calls inside one process, because two threads staging the same name
/// shared the path and the staging temporary beside it, and one rename found
/// the other's file already moved. It is not a secret and does not pretend to
/// be: what keeps the write itself honest is [`replace_atomically`] — it renames
/// over the destination, and a rename does not follow a symlink there.
pub fn scratch_file(name: &str) -> std::path::PathBuf {
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("issue-flow-{}-{call}-{name}", std::process::id()))
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
/// These are not Estigia's files: `settings.json` is read while the agent runs,
/// so a truncating write gives it a window in which its own configuration is
/// half a file. Written beside the target and renamed over it; the temporary
/// carries this process's id so two runs cannot collide, and stays in the same
/// directory so the rename does not cross a filesystem.
pub fn replace_atomically(path: &Path, desired: &str) -> std::io::Result<()> {
    replace_bytes_atomically(path, desired.as_bytes())
}

/// [`replace_atomically`] for content that need not be text.
///
/// A symlinked destination is resolved to the file it names and that file is replaced, so the link
/// survives; a link that names nothing is refused. The replacement keeps the mode of the file it
/// replaces.
pub fn replace_bytes_atomically(path: &Path, desired: &[u8]) -> std::io::Result<()> {
    let target = resolve_symlink(path)?;
    let temporary = target.with_extension(format!("{}{}.tmp", &STAGED[1..], std::process::id()));
    let write = write_and_sync(&temporary, desired)
        .and_then(|()| copy_existing_permissions(&target, &temporary))
        .and_then(|()| {
            if take_silent_swap_noop_for(path) {
                return Ok(());
            }
            std::fs::rename(&temporary, &target)
        })
        .and_then(|()| sync_parent(&target))
        .and_then(|()| verify_landed(path, desired, "replace"));
    if write.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    write
}

fn resolve_symlink(path: &Path) -> std::io::Result<std::path::PathBuf> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            std::fs::canonicalize(path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    std::io::Error::new(
                        error.kind(),
                        format!(
                            "{} is a dangling symlink; it names nothing to replace",
                            path.display()
                        ),
                    )
                } else {
                    error
                }
            })
        }
        _ => Ok(path.to_path_buf()),
    }
}

fn copy_existing_permissions(target: &Path, staged: &Path) -> std::io::Result<()> {
    match std::fs::metadata(target) {
        Ok(existing) => std::fs::set_permissions(staged, existing.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Publishes a complete new file without replacing anything that reached the path first.
///
/// The body is written and synced under a sibling name, then linked into place. A competing create
/// therefore wins with `AlreadyExists` and keeps its bytes; readers never see the staged partial body.
pub fn create_atomically(path: &Path, desired: &str) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("{}{}-new.tmp", &STAGED[1..], std::process::id()));
    let write = write_and_sync(&temporary, desired.as_bytes()).and_then(|()| {
        if take_silent_swap_noop_for(path) {
            return Ok(());
        }
        std::fs::hard_link(&temporary, path)
    });
    let _ = std::fs::remove_file(&temporary);
    write
        .and_then(|()| sync_parent(path))
        .and_then(|()| verify_landed(path, desired.as_bytes(), "create"))
}

/// What the destination holds, against what was staged for it.
///
/// The digest is taken from a re-read of the destination, never from the
/// stream that wrote it: a stream digest verifies its own copy and cannot see
/// a destination that ended up holding something else. Both sides travel in
/// the refusal — byte count and digest each — so a reader can tell a
/// short write from a foreign one without re-running anything.
fn verify_landed(path: &Path, staged: &[u8], action: &str) -> std::io::Result<()> {
    fn digest(bytes: &[u8]) -> String {
        let mut hasher = sha2::Sha256::new();
        hasher.update(bytes);
        hex_digest(hasher.finalize())
    }
    let landed = std::fs::read(path)?;
    if landed.as_slice() == staged {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "{action} {} atomically: the replacement did not land. The destination holds {} bytes ({}); {} bytes ({}) were staged",
        path.display(),
        landed.len(),
        digest(&landed),
        staged.len(),
        digest(staged)
    )))
}

/// Hex digest shared with [`crate::lifecycle`], which formats its own the
/// same way: one spelling for digests printed beside byte counts, rather than
/// a second copy of five lines that drift.
pub(crate) fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Writes the bytes and asks the filesystem to keep them.
///
/// A rename is atomic against another *process*; it is not durable against a
/// power cut. Without this a crash can leave the rename recorded and the
/// contents not — and for a run pointer that means a claim reading as absent, so
/// a run that swore reads as one that never did. Fail-open, on the one event
/// nobody gets to retry.
fn write_and_sync(path: &Path, desired: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(desired)?;
    sync_durably(&file)
}

/// Asks the filesystem to keep a file's bytes, unless a debug-only override
/// is armed. Compiled out of `--release`: production always syncs. See
/// [`fsync_skipped`] and `docs/honesty.md`.
pub(crate) fn sync_durably(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    if fsync_skipped() {
        return Ok(());
    }
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
pub(crate) fn sync_parent(path: &Path) -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    if fsync_skipped() {
        return Ok(());
    }
    sync_directory(path)
}

/// The unconditional parent-directory sync behind [`sync_parent`].
fn sync_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && let Ok(directory) = std::fs::File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// Whether durable syncs are currently skipped (debug builds only).
///
/// Two roads, both compiled out of `--release`:
///
/// - **Spawned test binaries** (`tests/pipe.rs`) set
///   `ESTIGIA_UNSAFE_SKIP_FSYNC=1` on every `Command`; the first read caches
///   that answer for the child's whole life.
/// - **In-process lib tests** never set the variable (a process-wide env flip
///   would race every concurrent reader). They arm
///   [`set_fsync_skip_for_tests`] instead, which stores an atomic.
///
/// [`with_real_fsync`] clears the skip only for its own thread, so a concurrent
/// `sandbox()` arming the process atomic cannot flip a real-fsync test mid-body.
#[cfg(debug_assertions)]
pub(crate) fn fsync_skipped() -> bool {
    use std::sync::atomic::Ordering;
    if FORCE_REAL_FSYNC.with(|flag| flag.get()) {
        return false;
    }
    if FSYNC_SKIP_FOR_TESTS.load(Ordering::SeqCst) {
        return true;
    }
    static FROM_ENV: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("ESTIGIA_UNSAFE_SKIP_FSYNC").is_some());
    *FROM_ENV
}

#[cfg(debug_assertions)]
thread_local! {
    static FORCE_REAL_FSYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(debug_assertions)]
static FSYNC_SKIP_FOR_TESTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(cfg!(test));

/// Arms or clears the in-process fsync skip (debug builds only).
///
/// Test binaries start with the skip already on (`cfg!(test)` above), so a
/// helper that builds `SetupOptions` without going through `sandbox()` still
/// skips. Production never calls this; a release build does not contain it.
#[cfg(test)]
pub(crate) fn set_fsync_skip_for_tests(skip: bool) {
    #[cfg(debug_assertions)]
    FSYNC_SKIP_FOR_TESTS.store(skip, std::sync::atomic::Ordering::SeqCst);
    #[cfg(not(debug_assertions))]
    let _ = skip;
}

/// One path, quoted so a shell reads it as itself.
///
/// Single quotes, not double: inside double quotes a shell still expands
/// `$(…)`, `` `…` `` and `${…}`, and still reads `\` as an escape — and `$` and
/// a backtick are legal in a directory name on every platform Estigia installs
/// on. Both places that write a path into a command line use this one; inside
/// single quotes nothing is special except the quote itself: close, escape
/// one, reopen.
pub fn shell_quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// The same, for a shell that escapes its own quote by doubling it.
///
/// PowerShell has no `'\''`: inside a single-quoted string the only escape is
/// `''`. One function each rather than a flag, because the two languages differ
/// in exactly this and a caller that picked the wrong one would produce a file
/// that parses everywhere except where it runs — a path may hold an apostrophe
/// and nothing refuses one.
pub fn powershell_quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

/// Fault injection for the silent-swap proof, test builds only.
///
/// A rename or link that reports success without taking effect is
/// unconstructible from user space, so the end-to-end read-back proof needs a
/// stand-in. Armed per destination path, as a set rather than a single slot:
/// one flag for the process would let tests running in parallel fault one
/// another's writes and flake the suite — measured, not theorised. Every test
/// stages under its own temporary directory, so armed paths never collide.
/// Mirrors the `REVIEWER_DEFINITION_WRITE_FAILURE` precedent in `setup`:
/// without it the no-op case the read-back exists for could never be run.
/// Production builds carry neither the armed set nor the branch that reads it.
///
/// Placed after the last truncating call in this file, not beside its
/// readers: the `nothing_writes_a_file_another_program_reads_by_truncating_it`
/// guard splits shipping code at the first `#[cfg(test)]` line, so an
/// attribute above `write_and_sync` would move `File::create` out of the
/// half it counts and fail the exemption as vacuous.
#[cfg(test)]
pub(crate) static INJECT_SILENT_SWAP_NOOP: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<PathBuf>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Arms the silent-swap fault for one destination. The matching swap consumes
/// only its own entry; a path nobody swaps stays armed, which is harmless
/// because every test stages under its own temporary directory.
#[cfg(test)]
pub(crate) fn inject_silent_swap_noop_for(path: &Path) {
    INJECT_SILENT_SWAP_NOOP
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(path.to_path_buf());
}

/// Runs `body` with the fsync skip **off** on this thread, so the real
/// `sync_all` paths execute (debug builds only).
///
/// Thread-local rather than a save/restore of the process atomic: a concurrent
/// `sandbox()` arming the atomic must not flip this body mid-run, and this body
/// must not clear a skip another thread armed. Test-only; absent from
/// `--release`.
#[cfg(test)]
pub(crate) fn with_real_fsync<T>(body: impl FnOnce() -> T) -> T {
    #[cfg(debug_assertions)]
    {
        let was = FORCE_REAL_FSYNC.with(|flag| flag.replace(true));
        let answer = body();
        FORCE_REAL_FSYNC.with(|flag| flag.set(was));
        answer
    }
    #[cfg(not(debug_assertions))]
    {
        body()
    }
}

fn take_silent_swap_noop_for(path: &Path) -> bool {
    #[cfg(test)]
    {
        INJECT_SILENT_SWAP_NOOP
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(path)
    }
    #[cfg(not(test))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `placed` hands back is a spelling anything else can be compared to.
    ///
    /// The line that strips the verbatim prefix is unmeasured by the callers
    /// today — every caller puts *both* sides of its comparison through here —
    /// but the next caller to compare this output against a path from anywhere
    /// else inherits `\\?\C:\x` against `C:\x`, the two-vocabularies failure
    /// this function exists to end. So the guarantee is asserted on the output
    /// itself.
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

    /// One file does not get two answers because of how it was spelled: `x`
    /// and `.\x` name the same file, and `placed` answered `None` for one and
    /// `Some(<cwd>\x)` for the other for as long as the `CurDir` arm was
    /// missing. The gate cannot reach it — it requires an absolute target;
    /// `paths` is a public module, and the next caller is who this is for.
    #[test]
    fn a_leading_current_directory_does_not_change_the_answer() {
        assert_eq!(placed(Path::new(".")), None);
        assert_eq!(placed(Path::new("./x")), placed(Path::new("x")));
    }

    #[test]
    fn home_resolves_to_an_absolute_path() {
        let home = home_dir().expect("a home directory is available in the test environment");
        assert!(home.is_absolute(), "home must be absolute, got {home:?}");
    }

    /// The same directory, however it is spelled, on both sides.
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

    /// Issue #74: a candidate under a symlinked home is contained whether or
    /// not anything has created it yet.
    ///
    /// `canonicalize` resolves the home through its symlink and falls back to
    /// the literal spelling for the candidate, so containment compared
    /// `/data/me` against `/home/me/xdg` and answered `false` — dropping an
    /// `XDG_CONFIG_HOME` plainly under the home, which wrote OpenCode's and
    /// Crush's files where neither host reads them.
    #[test]
    #[cfg(unix)]
    fn a_candidate_that_does_not_exist_is_contained_under_a_symlinked_home() {
        let real = tempfile::tempdir().expect("a real home");
        let holder = tempfile::tempdir().expect("a directory to hold the link");
        let home = holder.path().join("home");
        std::os::unix::fs::symlink(real.path(), &home).expect("the home symlink");

        let candidate = home.join("xdg");
        assert!(
            !candidate.exists(),
            "the candidate exists, so this measures nothing: {}",
            candidate.display()
        );
        assert!(
            covers(&home, &candidate),
            "{} is under {} and was read as outside it",
            candidate.display(),
            home.display()
        );
        // Creating it does not move the answer: the two sides were already in
        // one vocabulary.
        std::fs::create_dir_all(&candidate).expect("the candidate is created");
        assert!(
            covers(&home, &candidate),
            "{} is under {} once created and was read as outside it",
            candidate.display(),
            home.display()
        );
    }

    #[test]
    #[cfg(windows)]
    fn a_checkout_this_process_cannot_resolve_is_still_one_checkout() {
        // The fallback is the only place case decides: `canonicalize` answers
        // with the real spelling on disk, so a live directory spelled two ways
        // agrees without any folding — and two is the answer that *opens* the
        // gate.
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

    #[test]
    fn an_atomic_create_never_replaces_the_file_that_won_the_path() {
        with_real_fsync(|| {
            let root = tempfile::tempdir().expect("a temporary directory");
            let target = root.path().join("review-blind.md");
            std::fs::write(&target, "theirs\n").expect("the competing file exists");

            let error = create_atomically(&target, "ours\n")
                .expect_err("the no-replace create overwrote a competing file");
            assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
            assert_eq!(
                std::fs::read_to_string(target).expect("the competing file survives"),
                "theirs\n"
            );
        });
    }

    /// The real `sync_all` path still runs when the debug-only skip is off.
    ///
    /// `sandbox()`-armed setup tests leave the skip on for the rest of the
    /// process; without this, a green pass would never have executed either
    /// `sync_durably`'s or `sync_parent`'s unconditional sync in a normal run.
    #[test]
    fn the_real_fsync_paths_run_when_the_debug_skip_is_off() {
        with_real_fsync(|| {
            let root = tempfile::tempdir().expect("a temporary directory");
            let target = root.path().join("settings.json");
            replace_atomically(&target, "durable\n").expect("the write lands");
            assert_eq!(
                std::fs::read_to_string(&target).expect("the file reads"),
                "durable\n"
            );
            #[cfg(debug_assertions)]
            assert!(!fsync_skipped(), "the skip is armed inside with_real_fsync");
        });
    }

    /// A `--release` build has no branch that reads any override.
    ///
    /// The env reads live under `#[cfg(debug_assertions)]` in each file; a
    /// source assertion is what proves the release binary cannot honour them
    /// without paying for a second build in the suite.
    #[test]
    fn the_debug_only_overrides_are_compiled_out_of_release() {
        for (file, label, needle) in [
            (
                include_str!("paths.rs"),
                "paths",
                "ESTIGIA_UNSAFE_SKIP_FSYNC",
            ),
            (
                include_str!("transport/claim.rs"),
                "claim",
                "ESTIGIA_TEST_READBACK_NO_WAIT",
            ),
            (
                include_str!("setup/mod.rs"),
                "setup",
                "ESTIGIA_TEST_SKIP_PREVIEW",
            ),
        ] {
            let source = crate::source_text::code_of(file);
            let position = source
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} is read somewhere in {label}.rs"));
            let before = &source[..position];
            let gate = before.rfind("#[cfg(debug_assertions)]").unwrap_or_else(|| {
                panic!("{needle} in {label}.rs is not under a preceding #[cfg(debug_assertions)]")
            });
            let test_gate = before.rfind("#[cfg(test)]");
            assert!(
                test_gate.is_none_or(|test| gate > test),
                "{needle} in {label}.rs sits under #[cfg(test)] rather than \
                 #[cfg(debug_assertions)], so a non-test debug binary would not see the gate"
            );
        }
    }

    /// A rename that reports success without taking effect is refused, naming
    /// both sides: what the destination holds against what was staged. Issue
    /// #226. Deleting the read-back this proves turns this red at the
    /// assertion below — the write would report success over foreign bytes.
    #[test]
    fn a_silent_rename_noop_is_refused_with_both_sides_named() {
        with_real_fsync(|| {
            let root = tempfile::tempdir().expect("a temporary directory");
            let target = root.path().join("settings.json");
            std::fs::write(&target, "stale\n").expect("the destination starts elsewhere");

            inject_silent_swap_noop_for(&target);
            let error = replace_atomically(&target, "fresher\n")
                .expect_err("a rename that took no effect reported success");
            let report = error.to_string();
            assert!(
                report.contains("6 bytes") && report.contains("8 bytes"),
                "the refusal does not name both byte counts: {report}"
            );
            assert!(
                report.contains(&digest_of("stale\n")) && report.contains(&digest_of("fresher\n")),
                "the refusal does not name both digests: {report}"
            );
            assert_eq!(
                std::fs::read_to_string(&target).expect("the destination reads"),
                "stale\n",
                "the failed write left anything but the bytes it found"
            );
            assert!(
                std::fs::read_dir(root.path())
                    .expect("the directory reads")
                    .flatten()
                    .all(|entry| !is_staged_write(&entry.path())),
                "a write that failed left a staged file behind"
            );
        });
    }

    /// The same shape through the no-replace create: a link that reports
    /// success without taking effect meets the destination holding something
    /// else, and that is refused rather than reported as published.
    #[test]
    fn a_silent_link_noop_is_refused_with_both_sides_named() {
        with_real_fsync(|| {
            let root = tempfile::tempdir().expect("a temporary directory");
            let target = root.path().join("review-blind.md");
            std::fs::write(&target, "theirs\n").expect("the competing file exists");

            inject_silent_swap_noop_for(&target);
            let error = create_atomically(&target, "ours\n")
                .expect_err("a link that took no effect reported success");
            let report = error.to_string();
            assert!(
                report.contains("7 bytes") && report.contains("5 bytes"),
                "the refusal does not name both byte counts: {report}"
            );
            assert!(
                report.contains(&digest_of("theirs\n")) && report.contains(&digest_of("ours\n")),
                "the refusal does not name both digests: {report}"
            );
        });
    }

    #[test]
    fn two_scratch_files_of_one_name_in_one_process_are_two_paths() {
        let first = scratch_file("same");
        let second = scratch_file("same");
        assert_ne!(first, second);
        replace_atomically(&first, "first").expect("the first is staged");
        replace_atomically(&second, "second").expect("the second is staged");
        std::fs::remove_file(&first).expect("the first cleans up");
        assert_eq!(
            std::fs::read_to_string(&second).expect("the second survives"),
            "second"
        );
        std::fs::remove_file(&second).expect("the second cleans up");
    }

    fn digest_of(text: &str) -> String {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        hasher.update(text.as_bytes());
        hex_digest(hasher.finalize())
    }
}
