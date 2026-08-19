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
//! issue #31, which the whole suite was green over. With this, `--add-label` and
//! `--remove-label` change what the next `issue view --json labels` reports, so
//! a test can measure the label rather than the answer.

/// One issue's labels, after whatever this invocation did to them.
///
/// `None` when no test asked for a store, which is every fixture that existed
/// before this one and the reason they answer exactly as they did.
///
/// Additions are applied **before** removals, so `--add-label status:done
/// --remove-label status:done` leaves the label off. That is not a guess about
/// `gh`: it is what was measured on issue #3 on 2026-08-15, where a transition
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

    for answer in answers {
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
    std::process::exit(0);
}
