use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::transport::{Failure, How, run, with_process_override};

const REFUSED: &str = "git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.\n";
const REJECTED: &str =
    "! [rejected] main -> main (non-fast-forward)\nerror: failed to push some refs\n";

struct Shim {
    directory: tempfile::TempDir,
    log: PathBuf,
}

impl Shim {
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }
}

fn shim(program: &str, first: (&str, i32), second: &str) -> Shim {
    let directory = tempfile::tempdir().expect("a shim directory");
    let log = directory.path().join("calls.log");
    let (stderr, status) = first;
    crate::test_env::write_executable_script(
        &directory.path().join(program),
        &format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$*\" in\n*insteadOf*) {second} ;;\nesac\nprintf '%s' '{stderr}' >&2\nexit {status}\n",
            log = log.display(),
        ),
    );
    Shim { directory, log }
}

fn over_https_works() -> &'static str {
    "echo landed; exit 0"
}

fn over_https_fails() -> &'static str {
    "echo 'fatal: unable to access github.com' >&2; exit 128"
}

#[test]
fn a_push_ssh_never_connected_for_is_repeated_over_https_and_answers_with_that_attempt() {
    let shim = shim("git", (REFUSED, 128), over_https_works());
    let answer = with_process_override(shim.path(), &[], || {
        run(
            &["git", "push", "-u", "origin", "--", "fix"],
            None,
            How::write(),
        )
    })
    .expect("the second attempt landed");

    assert_eq!(answer.stdout.trim(), "landed");
    let calls = shim.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert_eq!(calls[0], "push -u origin -- fix");
    assert!(
        calls[1].contains("url.https://github.com/.insteadOf=git@github.com:")
            && calls[1].contains("url.https://github.com/.insteadOf=ssh://git@github.com/")
            && calls[1].contains("credential.https://github.com.helper=!gh auth git-credential")
            && calls[1].ends_with("push -u origin -- fix"),
        "{calls:?}"
    );
}

#[test]
fn every_network_verb_falls_back_and_a_local_one_does_not() {
    for verb in ["fetch", "ls-remote", "pull", "push"] {
        let shim = shim("git", (REFUSED, 128), over_https_works());
        with_process_override(shim.path(), &[], || {
            run(&["git", verb, "origin"], None, How::read())
        })
        .expect("the second attempt answered");
        assert_eq!(shim.calls().len(), 2, "{verb}");
    }

    let shim = shim("git", (REFUSED, 128), over_https_works());
    let answer = with_process_override(shim.path(), &[], || {
        run(&["git", "status"], None, How::read())
    });
    assert!(matches!(answer, Err(Failure::Read(_))), "{answer:?}");
    assert_eq!(shim.calls().len(), 1);
}

#[test]
fn a_program_other_than_git_is_never_repeated() {
    let shim = shim("gh", (REFUSED, 128), over_https_works());
    let answer = with_process_override(shim.path(), &[], || {
        run(&["gh", "push", "origin"], None, How::read())
    });
    assert!(matches!(answer, Err(Failure::Read(_))), "{answer:?}");
    assert_eq!(shim.calls().len(), 1);
}

#[test]
fn a_failure_after_the_connection_is_not_repeated() {
    let shim = shim("git", (REJECTED, 1), over_https_works());
    let answer = with_process_override(shim.path(), &[], || {
        run(&["git", "push", "origin", "main"], None, How::write())
    });
    assert!(matches!(answer, Err(Failure::Write(_))), "{answer:?}");
    assert_eq!(shim.calls().len(), 1);
}

#[test]
fn a_refused_key_message_alone_does_not_prove_the_connection_never_opened() {
    let printed_by_a_hook = "hook said: Permission denied (publickey)\n";
    let shim = shim("git", (printed_by_a_hook, 1), over_https_works());
    let answer = with_process_override(shim.path(), &[], || {
        run(&["git", "push", "origin", "main"], None, How::write())
    });
    assert!(matches!(answer, Err(Failure::Write(_))), "{answer:?}");
    assert_eq!(shim.calls().len(), 1);
}

#[test]
fn a_success_runs_once() {
    let shim = shim("git", ("", 0), over_https_works());
    with_process_override(shim.path(), &[], || {
        run(&["git", "fetch", "origin"], None, How::read())
    })
    .expect("the first attempt answered");
    assert_eq!(shim.calls().len(), 1);
}

#[test]
fn when_both_attempts_fail_the_answer_carries_both_and_keeps_its_kind() {
    let shim_write = shim("git", (REFUSED, 128), over_https_fails());
    let Err(Failure::Write(detail)) = with_process_override(shim_write.path(), &[], || {
        run(&["git", "push", "origin", "main"], None, How::write())
    }) else {
        panic!("a failed write stays a write failure");
    };
    assert!(
        detail.contains("Permission denied") && detail.contains("unable to access"),
        "{detail}"
    );

    let shim_read = shim("git", (REFUSED, 128), over_https_fails());
    let Err(Failure::Read(detail)) = with_process_override(shim_read.path(), &[], || {
        run(&["git", "fetch", "origin"], None, How::read())
    }) else {
        panic!("a failed read stays a read failure");
    };
    assert!(
        detail.contains("Permission denied") && detail.contains("unable to access"),
        "{detail}"
    );
}

#[test]
fn a_retry_that_outlives_its_deadline_stays_an_ambiguous_write() {
    let shim = shim("git", (REFUSED, 128), "sleep 5; exit 0");
    let answer = with_process_override(shim.path(), &[], || {
        run(
            &["git", "push", "origin", "main"],
            None,
            How::write_within(Duration::from_millis(500)),
        )
    });
    assert!(matches!(answer, Err(Failure::Timeout(_))), "{answer:?}");
}

#[test]
fn a_tolerated_call_answers_with_the_first_status_when_both_fail() {
    let shim = shim("git", (REFUSED, 128), over_https_fails());
    let answer = with_process_override(shim.path(), &[], || {
        run(&["git", "fetch", "origin"], None, How::tolerated())
    })
    .expect("a tolerated failure is an answer");
    assert_eq!(answer.status, 128);
    assert!(answer.stderr.contains("Permission denied"), "{answer:?}");
}

#[test]
fn a_network_call_runs_untranslated_so_its_failure_can_be_read() {
    let directory = tempfile::tempdir().expect("a shim directory");
    crate::test_env::write_executable_script(
        &directory.path().join("git"),
        "#!/bin/sh\nprintf '%s' \"$LC_ALL\"\n",
    );
    let answer = with_process_override(directory.path(), &[], || {
        run(&["git", "fetch", "origin"], None, How::read())
    })
    .expect("the shim answered");
    assert_eq!(answer.stdout, "C");
}

#[test]
fn real_git_prints_what_is_read_when_ssh_refuses_the_key() {
    let directory = tempfile::tempdir().expect("a directory for the refusing ssh");
    let refusing = directory.path().join("refusing-ssh");
    crate::test_env::write_executable_script(
        &refusing,
        "#!/bin/sh\necho 'git@github.com: Permission denied (publickey).' >&2\nexit 255\n",
    );
    let output = std::process::Command::new("git")
        .args(["ls-remote", "git@github.com:fixture/repo.git"])
        .env("GIT_SSH_COMMAND", &refusing)
        .env("LC_ALL", "C")
        .output()
        .expect("git runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(super::never_connected(&stderr), "{stderr}");
}

#[test]
fn every_message_of_a_connection_that_never_opened_qualifies() {
    for line in [
        "ssh: connect to host github.com port 22: Connection refused",
        "ssh: connect to host github.com port 22: Connection timed out",
        "ssh: connect to host github.com port 22: Operation timed out",
        "ssh: Could not resolve hostname github.com: nodename nor servname provided",
        "kex_exchange_identification: Connection closed by remote host",
        "git@github.com: Permission denied (publickey).",
    ] {
        let stderr = format!("{line}\nfatal: Could not read from remote repository.\n");
        assert!(super::never_connected(&stderr), "{line}");
    }
    assert!(!super::never_connected(
        "Host key verification failed.\nfatal: Could not read from remote repository.\n"
    ));
}
