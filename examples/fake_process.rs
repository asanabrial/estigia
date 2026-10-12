//! A stand-in for `gh` and `git`, for the differential tests.
//!
//! The transport has one process boundary, so replacing what is on `PATH`
//! replaces the whole world both implementations can see. Doing that needs a
//! *real executable*: a shell script works on Unix and not on Windows, where
//! resolving a bare `gh` finds `gh.exe` and never `gh.cmd` — which is how an
//! earlier fixture silently ran the machine's real `gh` on both sides and
//! compared two genuine network errors to each other.
//!
//! An example rather than a `[[bin]]`: `cargo test` builds it, and
//! `cargo install` does not ship it.
//!
//! It answers from `ESTIGIA_FAKE_ANSWERS`, a JSON list of
//! `{"matches": "...", "stdout": "...", "status": 0}`. The first entry whose
//! `matches` is a substring of the joined arguments wins. No entry matching is
//! success with empty output, because most calls in these fixtures are writes
//! nobody reads back.
//!
//! Three spellings of standard output, because a fixture that cannot pose a
//! shape makes the oracle answer "no difference" about it:
//!
//! - `stdout` — the text, and a newline. Right for output that is lines.
//! - `stdout_exact` — the text, byte for byte. For `-z` output, which ends in a
//!   NUL and nothing else.
//! - `stdout_hex` — hex pairs, decoded to bytes. For what JSON cannot hold: a
//!   path that is not UTF-8 is bytes a filesystem keeps and `git` hands back.
//!
//! An entry may also carry `nth`, which answers only the nth time its `matches`
//! is asked. It needs `ESTIGIA_FAKE_COUNT` naming a file, because every call is
//! its own process and that file is the only thing they share. Without it a
//! world cannot change its mind, and a command that reads, writes and reads
//! back can only be posed on the paths where it refuses.
//!
//! `ESTIGIA_FAKE_LABELS` names a file holding one issue's labels, and it is the
//! same gap for the one write this stand-in can model exactly. `nth` lets a
//! world change its mind on a *schedule*; it cannot let a read answer with what
//! the write before it actually did. So a fixture answered the same labels
//! before and after `gh issue edit`, and a transition that stripped the label it
//! was asked to set was indistinguishable from one that kept it — the shape of
//! a defect the whole suite was green over. With this, `--add-label` and
//! `--remove-label` change what the next `issue view --json labels` reports, so
//! a test can measure the label rather than the answer.
//!
//! `ESTIGIA_FAKE_ENV_LOG` names a file this process appends `GH_REPO=…`,
//! `GH_HOST=…` and `GH_TOKEN=…` lines to, one per invocation, `(absent)` where
//! a name is not set. Opt-in and a separate file from `ESTIGIA_FAKE_LOG`, so
//! every fixture that never asks for it is untouched. It exists to answer a
//! question `ESTIGIA_FAKE_LOG` cannot: not *what was this process asked*, but
//! *what did the environment carrying that call actually hold* — the question
//! `tests/pipe.rs`'s own `run_with_path_locked` clears three names against, with
//! nothing before this reading them back out of a child to say the clearing
//! landed.

/// One issue's labels, after whatever this invocation did to them.
///
/// `None` when no test asked for a store, which is every fixture that existed
/// before this one and the reason they answer exactly as they did.
///
/// Additions are applied **before** removals, so `--add-label status:done
/// --remove-label status:done` leaves the label off. That is not a guess about
/// `gh`: it is what was measured on 2026-08-15, where a transition
/// sending exactly that pair left the issue carrying no state label at all. A
/// store that let the addition win would answer *the label survived* about the
/// call that destroyed it, which is the failure this store exists to expose.
fn labels_after_this_call(arguments: &[String], line: &str) -> Option<Vec<String>> {
    let path = std::env::var("ESTIGIA_FAKE_LABELS").ok()?;
    let mut labels: Vec<String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    if !line.contains("issue edit") {
        return Some(labels);
    }
    let mut removals: Vec<&str> = Vec::new();
    let mut flag: Option<&str> = None;
    for argument in arguments {
        match flag.take() {
            Some("--add-label") => {
                if !labels.iter().any(|held| held == argument) {
                    labels.push(argument.clone());
                }
            }
            Some(_) => removals.push(argument),
            None => {
                if argument == "--add-label" || argument == "--remove-label" {
                    flag = Some(argument);
                }
            }
        }
    }
    labels.retain(|held| !removals.iter().any(|dropped| dropped == held));
    let _ = std::fs::write(
        &path,
        serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_owned()),
    );
    Some(labels)
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let line = arguments.join(" ");

    // Every invocation, appended, when a test asks for them.
    //
    // Without this the fixture can only show that the two sides reached the
    // same *answer*. It cannot show they asked the world the same questions —
    // and an unmatched call here is success with empty output, so a side that
    // makes an extra call, or skips one, is answered identically to a side that
    // does not. Two implementations asking different things and landing on the
    // same result look like agreement, which is the one thing an oracle over a
    // process boundary must not report.
    if let Ok(log) = std::env::var("ESTIGIA_FAKE_LOG") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            // And **where** it was asked from. Some arguments do not change what
            // is run, only where: `publish-review` takes a worktree and pushes
            // there, and a fixture that logs the command line alone cannot tell
            // a push from the isolated checkout from a push from the base. The
            // two spell the same words.
            let _ = match std::env::current_dir() {
                Ok(here) => writeln!(file, "{line}\t{}", here.display()),
                Err(_) => writeln!(file, "{line}"),
            };
        }
    }

    // Opt-in, and a different file: what this call's environment actually
    // held for the three names `gh` itself steers by. `ESTIGIA_FAKE_LOG`
    // above answers *what was this process asked*; this answers *what did the
    // world asking it look like*, which is the half a fixture that only reads
    // stdout can never see.
    if let Ok(path) = std::env::var("ESTIGIA_FAKE_ENV_LOG") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            // One write: three `writeln!`s let two concurrent `gh` children
            // interleave mid-line (`GH_REPO=GH_REPOacme/repo=`), which reads
            // as neither value and fails the assertion that asked for one.
            let mut block = String::new();
            for name in ["GH_REPO", "GH_HOST", "GH_TOKEN"] {
                let value = std::env::var(name).unwrap_or_else(|_| "(absent)".to_owned());
                block.push_str(&format!("{name}={value}\n"));
            }
            let _ = file.write_all(block.as_bytes());
        }
    }
    // Opt-in, and read **before** the real `gh` deletes what it staged: the
    // caller's own cleanup (`let _ = std::fs::remove_file(&staged);`) runs only
    // after this process has already exited, so the window in which the body
    // is readable belongs entirely to this call, not to a race with it. Without
    // this, nothing outside the write itself ever reads a comment's own text —
    // `staged_receipt` in `tests/pipe.rs` exists only because one write path
    // happens to leave its temporary behind; every other write deletes it, and
    // a defect in what that body actually says had no way to be measured.
    if let Some(at) = arguments
        .iter()
        .position(|argument| argument == "--body-file")
        && let Some(staged) = arguments.get(at + 1)
        && let Ok(body) = std::fs::read_to_string(staged)
        && body.contains("<!-- issue-flow:")
    {
        eprintln!("stand-in gh: a body bound for GitHub carries an issue-flow marker: {line}");
        std::process::exit(97);
    }
    if let Ok(path) = std::env::var("ESTIGIA_FAKE_BODY_LOG")
        && let Some(at) = arguments
            .iter()
            .position(|argument| argument == "--body-file")
        && let Some(staged) = arguments.get(at + 1)
        && let Ok(body) = std::fs::read_to_string(staged)
    {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "--- {line}\n{body}");
        }
    }
    observe_a_reviewer_launch(&arguments);

    let script = std::env::var("ESTIGIA_FAKE_ANSWERS").unwrap_or_else(|_| "[]".to_owned());
    let answers: Vec<serde_json::Value> = serde_json::from_str(&script).unwrap_or_default();

    // The one write this stand-in can model exactly, applied before anything is
    // answered so a read later in the same process sees it.
    let labels = labels_after_this_call(&arguments, &line);

    // How many times each `matches` has been asked, counting this call.
    //
    // A world that cannot change its mind can only pose commands that read. The
    // three that matter most — claiming an issue, taking one over, putting one
    // down — all read, write, and **read back**, and a stand-in answering the
    // same `gh issue view` identically both times cannot pose the path that
    // lets work through. Two sides refusing agree perfectly and prove nothing.
    //
    // On disk because every call is its own process: this file is the only
    // thing the second `gh` shares with the first. Off unless a test asks for
    // it, so every existing fixture answers exactly as it did.
    let counts: std::collections::BTreeMap<String, u64> = match std::env::var("ESTIGIA_FAKE_COUNT")
    {
        Ok(path) => {
            let mut counts: std::collections::BTreeMap<String, u64> =
                std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str(&text).ok())
                    .unwrap_or_default();
            // One count per distinct `matches`, per call. Two entries sharing a
            // `matches` are two answers to one question, not two questions, so
            // the key is counted once however many entries carry it.
            let asked: std::collections::BTreeSet<String> = answers
                .iter()
                .map(|answer| {
                    answer
                        .get("matches")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_owned()
                })
                .filter(|matches| matches.is_empty() || line.contains(matches.as_str()))
                .collect();
            for key in asked {
                *counts.entry(key).or_insert(0) += 1;
            }
            let _ = std::fs::write(
                &path,
                serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_owned()),
            );
            counts
        }
        Err(_) => std::collections::BTreeMap::new(),
    };

    for answer in &answers {
        let matches = answer.get("matches").and_then(|v| v.as_str()).unwrap_or("");
        if !matches.is_empty() && !line.contains(matches) {
            continue;
        }
        // `nth` answers the nth time this `matches` is asked, and is ignored
        // when no counter file was asked for.
        if let Some(nth) = answer.get("nth").and_then(serde_json::Value::as_u64)
            && counts.get(matches).copied().unwrap_or(0) != nth
        {
            continue;
        }
        if let Some(append) = answer.get("store_append") {
            append_to_store(append);
        }
        // `gh issue view <n> --json labels`, answered from the store rather than
        // from the script. A scripted answer is the fixture repeating itself; the
        // point of the store is that the edits this same run made are what comes
        // back.
        if answer.get("labels").and_then(serde_json::Value::as_bool) == Some(true) {
            let names: Vec<serde_json::Value> = labels
                .iter()
                .flatten()
                .map(|name| serde_json::json!({ "name": name }))
                .collect();
            println!("{}", serde_json::json!({ "labels": names }));
        }
        if let Some(text) = answer.get("stdout").and_then(|v| v.as_str()) {
            println!("{text}");
        }
        // The same, byte for byte. `stdout` appends a newline, which is right
        // for the commands whose output is lines and cannot express the ones
        // whose output is not: `git diff --name-only -z` ends every path with a
        // NUL and the last one too, with nothing after it. Scripted through
        // `stdout`, that trailing newline arrives as one more path — both sides
        // kept it, so the crossing still agreed, but it agreed about a fixture
        // no `git` produces. An oracle whose fixtures cannot spell a shape the
        // world has is one that reports "no difference" for the case it cannot
        // pose.
        if let Some(text) = answer.get("stdout_exact").and_then(|v| v.as_str()) {
            use std::io::Write;
            let _ = std::io::stdout().write_all(text.as_bytes());
        }
        // Bytes JSON cannot hold. A path is bytes on Unix and git hands them
        // back unchanged, so a repository can legitimately contain a name that
        // is not UTF-8 — and the two transports decode that differently, which
        // is a claim the fixture had no way to pose: `stdout` and
        // `stdout_exact` both take a `&str`, and a `&str` is already valid.
        if let Some(text) = answer.get("stdout_hex").and_then(|v| v.as_str()) {
            use std::io::Write;
            let bytes: Vec<u8> = text
                .as_bytes()
                .chunks(2)
                .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
                .collect();
            let _ = std::io::stdout().write_all(&bytes);
        }
        if let Some(text) = answer.get("stderr").and_then(|v| v.as_str()) {
            eprintln!("{text}");
        }
        let status = answer
            .get("status")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        std::process::exit(status as i32);
    }
    if let Some(text) = rest_issue_answer(&line, &answers) {
        println!("{text}");
    }
    std::process::exit(0);
}

/// What a reviewer CLI launch looks like from inside the child, each opt-in by
/// its own variable so no earlier fixture is touched.
///
/// `ESTIGIA_FAKE_ARGV_LOG` is overwritten with every argument followed by a NUL
/// and then the working directory followed by a NUL, because a space-joined line
/// cannot tell two arguments from one. `ESTIGIA_FAKE_STDIN_LOG` receives
/// standard input read to its end. `ESTIGIA_FAKE_WRITE_ARG_FILE` is written to
/// the file named after `--output-last-message`. `ESTIGIA_FAKE_ENV_NAMES`
/// receives the name of every environment variable the process holds, one per
/// line, and `ESTIGIA_FAKE_EXIT_WITHOUT_READING` exits 0 before reading stdin. `ESTIGIA_FAKE_DELAY_MS` sleeps
/// and `ESTIGIA_FAKE_BIG_STDOUT` writes that many bytes to standard output.
fn observe_a_reviewer_launch(arguments: &[String]) {
    use std::io::{Read, Write};
    if std::env::var_os("ESTIGIA_FAKE_EXIT_WITHOUT_READING").is_some() {
        std::process::exit(0);
    }
    if let Ok(path) = std::env::var("ESTIGIA_FAKE_ENV_NAMES") {
        let names: String = std::env::vars_os()
            .map(|(name, _)| format!("{}\n", name.to_string_lossy()))
            .collect();
        let _ = std::fs::write(path, names);
    }
    if let Ok(path) = std::env::var("ESTIGIA_FAKE_ARGV_LOG") {
        let mut record = Vec::new();
        for argument in arguments {
            record.extend_from_slice(argument.as_bytes());
            record.push(0);
        }
        if let Ok(here) = std::env::current_dir() {
            record.extend_from_slice(here.to_string_lossy().as_bytes());
        }
        record.push(0);
        let _ = std::fs::write(path, record);
    }
    if let Ok(path) = std::env::var("ESTIGIA_FAKE_STDIN_LOG") {
        let mut received = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut received);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = file.write_all(&received);
        }
    }
    if let Ok(content) = std::env::var("ESTIGIA_FAKE_WRITE_ARG_FILE")
        && let Some(at) = arguments
            .iter()
            .position(|argument| argument == "--output-last-message")
        && let Some(target) = arguments.get(at + 1)
    {
        let _ = std::fs::write(target, content);
    }
    if let Some(millis) = std::env::var("ESTIGIA_FAKE_DELAY_MS")
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(millis));
    }
    if let Some(bytes) = std::env::var("ESTIGIA_FAKE_BIG_STDOUT")
        .ok()
        .and_then(|text| text.parse::<usize>().ok())
    {
        let block = vec![b'x'; 64 * 1024];
        let mut stdout = std::io::stdout().lock();
        let mut remaining = bytes;
        while remaining > 0 {
            let take = remaining.min(block.len());
            if stdout.write_all(&block[..take]).is_err() {
                break;
            }
            remaining -= take;
        }
        let _ = stdout.flush();
    }
}

/// What `gh api repos/{owner}/{repo}/issues/<n>` answers when no entry names it,
/// and an empty list for that issue's comments.
///
/// Every operation that touches an issue asks GitHub that one question before it
/// reads the local store, and a fixture written before the store existed scripts
/// only `gh issue view`. So the question is answered from the `issue view`
/// entries: the issue is closed when the first entry for that number, or failing
/// that the first entry naming no number, carries a payload whose `state` is not
/// `OPEN`. Anything else is open.
fn rest_issue_answer(line: &str, answers: &[serde_json::Value]) -> Option<String> {
    let after = line.split_once("api repos/{owner}/{repo}/issues/")?.1;
    let digits = after.split(|c: char| !c.is_ascii_digit()).next()?;
    if after[digits.len()..].starts_with("/comments") {
        return Some("[]".to_owned());
    }
    let number: u64 = digits.parse().ok()?;
    let closed_in = |answer: &serde_json::Value| -> Option<bool> {
        let stdout = answer.get("stdout")?.as_str()?;
        let payload: serde_json::Value = serde_json::from_str(stdout).ok()?;
        Some(payload.get("state")?.as_str()? != "OPEN")
    };
    let mut anywhere = None;
    let mut named = None;
    for answer in answers {
        let Some(matches) = answer.get("matches").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !matches.contains("view") {
            continue;
        }
        let asked = matches
            .split_once("view ")
            .and_then(|(_, rest)| rest.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|digits| digits.parse::<u64>().ok());
        match asked {
            Some(asked) if asked == number => {
                named = named.or_else(|| closed_in(answer));
            }
            Some(_) => {}
            None => anywhere = anywhere.or_else(|| closed_in(answer)),
        }
    }
    let closed = named.or(anywhere).unwrap_or(false);
    Some(
        serde_json::json!({
            "state": if closed { "closed" } else { "open" },
            "repository_url": "https://api.github.com/repos/fixture/repo",
        })
        .to_string(),
    )
}

/// Another run writing to the local store while this call is being answered.
///
/// `{"path": <state.db>, "issue": <n>, "at": <stamp>, "bodies": [<comment>, …]}`
/// appends each body to that issue as an entry the store trusts, the way a
/// second Estigia process would. It exists because the store decides from what
/// it holds, so the only way for the world to move under a run mid-operation
/// is for something to write to it — and a scripted `gh issue view` can no
/// longer pose that.
fn append_to_store(append: &serde_json::Value) {
    let (Some(path), Some(issue), Some(at), Some(bodies)) = (
        append.get("path").and_then(serde_json::Value::as_str),
        append.get("issue").and_then(serde_json::Value::as_i64),
        append.get("at").and_then(serde_json::Value::as_str),
        append.get("bodies").and_then(serde_json::Value::as_array),
    ) else {
        return;
    };
    let Ok(connection) = rusqlite::Connection::open(path) else {
        return;
    };
    let _ = connection.busy_timeout(std::time::Duration::from_secs(5));
    for body in bodies.iter().filter_map(serde_json::Value::as_str) {
        let _ = connection.execute(
            "INSERT INTO entries (repo_id, issue, created_at, body, viewer_did_author,
                includes_created_edit, source)
             SELECT repo_id, number, ?2, ?3, 1, 0, 'local' FROM issues WHERE number = ?1",
            rusqlite::params![issue, at, body],
        );
    }
}
