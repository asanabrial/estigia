//! Containing and killing a child process tree: a process group on Unix, a job object on Windows.

#[cfg(unix)]
mod unix {
    use std::path::Path;
    use std::process::{Child, Command};

    use std::os::unix::process::CommandExt;

    pub(crate) struct Prepared;

    pub(crate) struct Owner {
        group: i32,
    }

    pub(crate) fn prepare(command: &mut Command) -> Result<Prepared, String> {
        command.process_group(0);
        Ok(Prepared)
    }

    impl Prepared {
        pub(crate) fn attach(self, child: &Child) -> Result<Owner, String> {
            let group = i32::try_from(child.id())
                .map_err(|_| format!("child PID {} does not fit a process group", child.id()))?;
            Ok(Owner { group })
        }
    }

    impl Owner {
        pub(crate) fn terminate(&self) -> Result<(), String> {
            // SAFETY: the child was placed in a fresh process group whose ID is
            // its PID. A negative PID addresses that group and no other one.
            if unsafe { libc::kill(-self.group, libc::SIGKILL) } == 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error.to_string())
            }
        }
    }

    pub(crate) fn launcher_can_be_contained(_program: &Path) -> bool {
        true
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::mem;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::process::{Child, Command};
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    pub(crate) struct Prepared {
        job: Job,
    }

    pub(crate) struct Owner {
        job: Job,
    }

    struct Job(HANDLE);

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: this type exclusively owns the handle returned by
            // CreateJobObjectW and closes it exactly once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    pub(crate) fn prepare(_command: &mut Command) -> Result<Prepared, String> {
        // SAFETY: null security attributes and name request an unnamed job with
        // the caller's default security descriptor.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let job = Job(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` has the exact layout and size required by the named
        // information class and remains alive for the duration of the call.
        let configured = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast::<c_void>(),
                mem::size_of_val(&limits) as u32,
            )
        };
        if configured == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Prepared { job })
    }

    impl Prepared {
        pub(crate) fn attach(self, child: &Child) -> Result<Owner, String> {
            // SAFETY: Child owns a live process handle for this call; the job
            // remains owned by `self` until assignment succeeds.
            let assigned = unsafe {
                AssignProcessToJobObject(self.job.0, child.as_raw_handle().cast::<c_void>())
            };
            if assigned == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Owner { job: self.job })
        }
    }

    impl Owner {
        pub(crate) fn terminate(&self) -> Result<(), String> {
            // SAFETY: `job` is a live handle owned by this value.
            if unsafe { TerminateJobObject(self.job.0, 1) } != 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error().to_string())
            }
        }
    }

    pub(crate) fn launcher_can_be_contained(program: &Path) -> bool {
        program
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com")
            })
    }
}

#[cfg(unix)]
pub(crate) use unix::{Owner, launcher_can_be_contained, prepare};
#[cfg(windows)]
pub(crate) use windows::{Owner, launcher_can_be_contained, prepare};
