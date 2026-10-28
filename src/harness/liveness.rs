#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Liveness {
    Live(u64),
    Gone,
    Unknown,
}

impl Liveness {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Live(_) => "live",
            Self::Gone => "gone",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg_attr(not(unix), allow(dead_code))]
const WALK_LIMIT: usize = 8;

#[cfg_attr(not(unix), allow(dead_code))]
fn is_wrapper(comm: &str) -> bool {
    let name = comm
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim_start_matches('-');
    matches!(
        name,
        "sh" | "bash" | "zsh" | "dash" | "fish" | "ksh" | "estigia" | "estigia.exe"
    )
}

/// The first ancestor from `start` that is neither a shell nor `estigia`, or
/// `None` when the chain cannot be read, ends at pid 1, or stays wrappers for
/// more than eight levels. A guess here would let `--exec` start a second copy
/// of an open session, so every doubt is `None`.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn agent_in(start: u64, lookup: impl Fn(u64) -> Option<(u64, String)>) -> Option<u64> {
    let mut pid = start;
    for _ in 0..WALK_LIMIT {
        if pid <= 1 {
            return None;
        }
        let (parent, comm) = lookup(pid)?;
        if !is_wrapper(&comm) {
            return Some(pid);
        }
        pid = parent;
    }
    None
}

#[cfg(unix)]
fn ps_lookup(pid: u64) -> Option<(u64, String)> {
    let output = std::process::Command::new("ps")
        .args(["-o", "ppid=,comm=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let line = text.trim();
    let (parent, comm) = line.split_once(char::is_whitespace)?;
    let comm = comm.trim();
    (!comm.is_empty()).then_some(())?;
    Some((parent.parse().ok()?, comm.to_owned()))
}

/// The agent's own process: the nearest ancestor of the registering hook or
/// command that is not a shell. `None` where the platform offers no portable
/// way to read it, or where the ancestry cannot be read.
pub(crate) fn agent_pid() -> Option<u64> {
    #[cfg(unix)]
    {
        agent_in(u64::from(std::os::unix::process::parent_id()), ps_lookup)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

#[cfg(unix)]
fn program_of(pid: u64) -> Option<String> {
    #[cfg(target_os = "linux")]
    if let Ok(path) = std::fs::read_link(format!("/proc/{pid}/exe")) {
        return path.to_str().map(str::to_owned);
    }
    ps_lookup(pid).map(|(_, comm)| comm)
}

/// The program the agent process runs, as the platform names it: a full path
/// where it gives one, else the bare command name. `None` where it cannot be
/// read.
pub(crate) fn agent_program() -> Option<String> {
    #[cfg(unix)]
    {
        program_of(agent_pid()?)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

pub(crate) fn host_name() -> Option<String> {
    #[cfg(unix)]
    {
        let mut buffer = [0u8; 256];
        // SAFETY: the buffer is writable for its whole length, which is passed.
        let status = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
        if status != 0 {
            return None;
        }
        let end = buffer.iter().position(|byte| *byte == 0)?;
        let name = String::from_utf8_lossy(&buffer[..end]).trim().to_owned();
        (!name.is_empty()).then_some(name)
    }
    #[cfg(not(unix))]
    {
        std::env::var("COMPUTERNAME")
            .ok()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
    }
}

#[cfg(unix)]
fn exists(pid: u64) -> Option<bool> {
    let pid = i32::try_from(pid).ok().filter(|pid| *pid > 0)?;
    // SAFETY: signal 0 delivers nothing; it only asks whether the process exists.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return Some(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Some(false),
        Some(libc::EPERM) => Some(true),
        _ => None,
    }
}

#[cfg(not(unix))]
fn exists(_pid: u64) -> Option<bool> {
    None
}

pub(crate) fn of(pid: Option<u64>, host: Option<&str>) -> Liveness {
    let (Some(pid), Some(host), Some(here)) = (pid, host, host_name()) else {
        return Liveness::Unknown;
    };
    if host != here {
        return Liveness::Unknown;
    }
    match exists(pid) {
        Some(true) => Liveness::Live(pid),
        Some(false) => Liveness::Gone,
        None => Liveness::Unknown,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn here() -> String {
        host_name().expect("this machine has a name")
    }

    fn dead_pid() -> u64 {
        let mut child = std::process::Command::new("true")
            .spawn()
            .expect("a child starts");
        let pid = u64::from(child.id());
        child.wait().expect("the child is waited for");
        pid
    }

    #[test]
    fn a_row_with_no_token_is_unknown() {
        assert_eq!(of(None, None), Liveness::Unknown);
        assert_eq!(of(Some(std::process::id().into()), None), Liveness::Unknown);
        assert_eq!(of(None, Some(&here())), Liveness::Unknown);
    }

    #[test]
    fn a_token_from_another_machine_is_unknown_even_for_a_pid_that_exists_here() {
        assert_eq!(
            of(Some(std::process::id().into()), Some("elsewhere.invalid")),
            Liveness::Unknown
        );
    }

    #[test]
    fn a_pid_that_ended_is_gone() {
        assert_eq!(of(Some(dead_pid()), Some(&here())), Liveness::Gone);
    }

    #[test]
    fn a_pid_that_exists_is_live_and_is_named() {
        let own = u64::from(std::process::id());
        assert_eq!(of(Some(own), Some(&here())), Liveness::Live(own));
    }

    #[test]
    fn a_pid_that_cannot_be_probed_safely_is_unknown() {
        assert_eq!(of(Some(0), Some(&here())), Liveness::Unknown);
        assert_eq!(of(Some(u64::MAX), Some(&here())), Liveness::Unknown);
    }

    fn chain(links: &'static [(u64, u64, &'static str)]) -> impl Fn(u64) -> Option<(u64, String)> {
        move |pid| {
            links
                .iter()
                .find(|(own, _, _)| *own == pid)
                .map(|(_, parent, comm)| (*parent, (*comm).to_owned()))
        }
    }

    #[test]
    fn a_direct_non_shell_parent_is_taken_as_is() {
        let lookup = chain(&[(500, 400, "/usr/local/bin/claude")]);
        assert_eq!(agent_in(500, lookup), Some(500));
    }

    #[test]
    fn shells_and_estigia_are_skipped_up_to_the_agent() {
        let lookup = chain(&[
            (500, 400, "-zsh"),
            (400, 300, "/bin/sh"),
            (300, 200, "estigia"),
            (200, 100, "/opt/codex"),
        ]);
        assert_eq!(agent_in(500, lookup), Some(200));
    }

    #[test]
    fn a_chain_of_only_shells_or_one_reaching_init_is_none() {
        let shells = chain(&[(500, 400, "bash"), (400, 300, "dash"), (300, 1, "ksh")]);
        assert_eq!(agent_in(500, shells), None);
        let long = |pid: u64| Some((pid + 1, "fish".to_owned()));
        assert_eq!(agent_in(500, long), None);
    }

    #[test]
    fn a_failed_lookup_is_none() {
        assert_eq!(agent_in(500, |_| None), None);
        let lookup = chain(&[(500, 400, "sh")]);
        assert_eq!(agent_in(500, lookup), None);
    }

    #[test]
    fn the_real_walk_from_this_process_does_not_panic_and_never_names_pid_1() {
        assert_ne!(agent_pid(), Some(1));
    }
}
