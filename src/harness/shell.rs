//! What a command line does to the filesystem.
//!
//! The classifier reads a shell tool's *argument* to decide what the call is.
//! For git it reads command names; this module reads the rest — the spellings
//! that write a file without naming git at all.
//!
//! It exists because the two lists beside it left a hole they both declared: a
//! write denied as `Edit` could be spelled `echo … > src/x.rs` and go through
//! untouched. The comment on the classifier called that "a smaller gap than a
//! harness that gates `ls`", which framed it as a choice between letting shell
//! writes past and gating every command. It is not a choice. The constructs
//! that *visibly* write a file are a small, nameable population, and this reads
//! for exactly those.
//!
//! Borrowed from [statewright](https://github.com/statewright/statewright),
//! whose engine states the same rule as "bash discernment": block `echo > file`,
//! `sed -i` and scripting interpreters when the write tools are not allowed.

/// Commands whose whole purpose is to put bytes somewhere.
///
/// Matched as a whole word in command position, never as a substring: `rm` sits
/// inside `confirm` and `cp` inside `tcp`, and a guard that fires on those is a
/// guard people switch off.
const WRITES_A_FILE: &[&str] = &[
    // POSIX.
    "cp",
    "mv",
    "rm",
    "rmdir",
    "tee",
    "dd",
    "truncate",
    "touch",
    "mkdir",
    "ln",
    "install",
    "patch",
    "unzip",
    "tar",
    // The rest of the pack-and-unpack family. `unzip` and `tar` were here and
    // their siblings were not, which is not a boundary — the declaration below
    // names *unpack* as in scope, and these are the same utilities under
    // different names. Measured: `zip -r out.zip src`, `gzip big.log` and
    // `7z a out.7z src` all classified `Untouched`, beside `unzip x.zip`
    // classifying as a write.
    "zip",
    "gzip",
    "gunzip",
    "bzip2",
    "bunzip2",
    "xz",
    "unxz",
    "zstd",
    "unzstd",
    "7z",
    "7za",
    "compress-archive",
    "expand-archive",
    // Copy, over a network shape. `cp` and `mv` are above; these are the same
    // act with a host on one end, and the file they land on is a file.
    "rsync",
    "scp",
    // Delete, and make many out of one. `rm` is above; `shred` is `rm` that
    // also overwrites, and `split` puts `xaa`, `xab` … where there was nothing.
    "shred",
    "split",
    // Downloads to a file by default: `wget URL` leaves the file in the working
    // directory whether or not `-O` names one. `curl` is the other way round
    // and is read below, where the flag is what decides.
    "wget",
    // Windows, which is this crate's own platform and where an agent's shell
    // tool hands over the native spellings. Their readers — `dir`, `type`,
    // `where`, `get-content` — are deliberately absent.
    "copy",
    "xcopy",
    "robocopy",
    "move",
    "del",
    "erase",
    "ren",
    "rename",
    "md",
    "rd",
    // PowerShell writes through verbs rather than through `>`.
    "set-content",
    "add-content",
    "clear-content",
    "out-file",
    "new-item",
    "remove-item",
    "copy-item",
    "move-item",
    "rename-item",
];

/// Interpreters, and the flags that hand them code on the command line.
///
/// The payload is quoted, so [`redirects_into_a_file`] cannot see the `>` that
/// `python -c "open('x','w')"` never needs anyway. The invocation is the only
/// visible part, so the invocation is what is read.
const INLINE_CODE: &[&str] = &[
    "python",
    "python3",
    "node",
    "perl",
    "ruby",
    "sh",
    "bash",
    "zsh",
    "pwsh",
    "powershell",
    "deno",
    "bun",
    // `cmd /c "echo x > f"` hides its redirect behind quotes exactly the way
    // `python -c` does, and on Windows it is the ordinary spelling rather than
    // the exotic one.
    "cmd",
];

/// The flags that mean "the code is right here".
const CODE_FLAGS: &[&str] = &[
    "-c",
    "-e",
    "--eval",
    "-command",
    "--command",
    "-encodedcommand",
    // `cmd`'s own two.
    "/c",
    "/k",
];

/// PowerShell's switches that carry code, and every name that reaches them.
///
/// A list of spellings does not work for this shell. PowerShell resolves a
/// parameter from **any prefix that is not ambiguous**, so `-EncodedCommand` is
/// also `-enc`, `-ec` and `-en`, and `-Command` is also `-comm` and `-Comman`.
/// [`CODE_FLAGS`] held `-command` and `-encodedcommand` — the two spellings
/// nobody types — so `powershell -nop -enc <base64>`, the canonical form in
/// every script ever written, read as an opaque argument to a program that
/// writes nothing, and the `git push` inside it went through unclassified. On
/// the platform this crate is developed on.
///
/// Measured against `powershell.exe` here: `-enc`, `-ec`, `-comm` and `-Comman`
/// each ran their argument.
///
/// Matched by prefix rather than by a longer list because a list of spellings
/// for a shell with N-letter abbreviations is a list that is always one spelling
/// short. Ambiguity is deliberately not modelled: a prefix PowerShell would
/// reject as ambiguous runs nothing, so reading it as code costs one question
/// that never had to be asked, while the other way round is a write nobody saw.
///
/// `ec` is here because a prefix rule alone was **not enough**, and running it
/// is what said so: `powershell -nop -ec <base64>` ran its argument and `ec` is
/// a prefix of neither name. `powershell.exe` does not use the cmdlet parameter
/// binder for its own command line — its host parser takes documented short
/// aliases as well, and `-e` and `-ec` are the two Microsoft publishes for
/// `-EncodedCommand`. Modelling only the tidy rule would have shipped a fix that
/// still let the canonical spelling through.
const POWERSHELL_CODE: &[&str] = &["command", "encodedcommand", "ec"];

/// Whether `flag`, handed to `head`, means the code follows on the same line.
fn carries_code(head: &str, flag: &str) -> bool {
    let flag = flag.to_ascii_lowercase();
    CODE_FLAGS.contains(&flag.as_str())
        || (matches!(head, "powershell" | "pwsh")
            && flag.strip_prefix('-').is_some_and(|name| {
                !name.is_empty() && POWERSHELL_CODE.iter().any(|full| full.starts_with(name))
            }))
}

/// Filters that rewrite their input in place when asked to.
const IN_PLACE: &[&str] = &["sed", "perl", "ruby", "gawk"];

/// Commands whose own name is not the command being run.
///
/// guard:population running-shell too-tight: the prefixes that put somebody
/// else's command after their own, in the spellings a person types. Legitimate
/// population: every wrapper that runs the next word. Boundary: the wrapped
/// command is still **visible on the command line**, which is the same boundary
/// `writing-shell` draws — and that is why these belong to it rather than being
/// an escape from it.
///
/// `sudo rm -rf src` hides nothing. It was missed all the same, because the
/// head of the segment was `sudo` and `sudo` writes no files. So did
/// `env FOO=1 rm`, `nohup rm`, `time rm`, `doas rm`, `busybox rm` and
/// `xargs rm` — seven spellings of a write that the list above already knows,
/// wearing a hat.
///
/// Fifteen more were found the same way, by running the population's own
/// sentence — *every wrapper that runs the next word* — past the list rather
/// than reading it. `flock /var/lock/x rm -rf src`, `taskset 0x1 rm`,
/// `parallel rm`, `runuser -u bob rm`, `chroot /jail rm`, `strace rm`,
/// `ltrace rm`, `watch rm`, `systemd-run rm`, `eatmydata rm`, `unbuffer rm`,
/// `xvfb-run rm`, `proxychains rm`, `torify rm` and `setarch x86_64 rm` each
/// classified as nothing at all, while `timeout 5 rm`, `stdbuf -o0 rm` and
/// `nice rm` — the same shape, the same visibility — classified as writes. A
/// list this one drew a boundary for and then did not fill to it is worse than
/// a shorter one, because the boundary is what a reader trusts.
///
/// `chrt` is here without a measurement of its own, as `nice` and `ionice`'s
/// third sibling: it sets the scheduling policy and runs the rest of the line.
/// Said rather than left to be noticed, because a list is only as good as the
/// account of how each entry got in.
///
/// `su -c '…'` and `script -c '…'` are **not** here, and the difference is the
/// boundary itself: both take their command as a quoted argument rather than as
/// the words that follow, so they are inline code and belong to the interpreter
/// arm of `writing-shell`, which reads it.
///
/// After one of these, **any** later word naming a write is taken as the write.
/// Not the next word: `sudo -u bob rm x` puts two between them, and a scheme
/// that tries to know which flags take arguments has to know it per wrapper.
/// The cost is `sudo grep -r rm .` reading the tracker once, which is the
/// asymmetry this whole module is built on — a false positive costs a read, a
/// false negative costs the guarantee.
const RUNS_ANOTHER_COMMAND: &[&str] = &[
    // As somebody else, or somewhere else.
    "sudo",
    "doas",
    "runuser",
    "chroot",
    "command",
    "busybox",
    // With the environment, the scheduler or the buffering changed.
    "env",
    "nice",
    "ionice",
    "taskset",
    "chrt",
    "setarch",
    "eatmydata",
    "stdbuf",
    "unbuffer",
    // Detached, timed, locked, repeated, or handed to the service manager.
    "nohup",
    "setsid",
    "time",
    "timeout",
    "flock",
    "watch",
    "systemd-run",
    // Once per input.
    "xargs",
    "parallel",
    // Under something that watches it run.
    "strace",
    "ltrace",
    "xvfb-run",
    "proxychains",
    "torify",
];

/// Whether this command line writes a file, and the spelling that says so.
///
/// guard:population writing-shell too-tight: the shell constructs whose visible
/// effect is writing a file — a redirect, an in-place edit, an interpreter
/// handed code, and the utilities that copy, move, delete or unpack — in their
/// POSIX *and* Windows spellings, since this crate's own platform is the one
/// where `cmd /c` and `Set-Content` are the ordinary way to write. Legitimate
/// population: every shell spelling of a repository write. Boundary: what is
/// *visible on the command line*. A program that writes as a side effect —
/// `make`, `cargo build`, `npm run`, an interpreter given a script file — is not
/// matched, and is not meant to be.
///
/// That boundary is why a **long write flag** on any command counts, whatever
/// the command is: `--write`, `--in-place` and `--fix` are as visible as a
/// redirect. They were read only for the few commands `IN_PLACE` names, so
/// `prettier --write src` and `eslint --fix .` rewrote every file in a tree and
/// classified as nothing at all. Long forms only: `-i` is *ignore case* to
/// `grep` and `-w` is *whole word*, so reading either everywhere would report a
/// search as a write.
///
/// That boundary is the whole design, and it is drawn where it is because the
/// alternative is gating every command that could write, which is every command.
/// Inline code is read and a script file is not, because one is opaque *and*
/// unusual and the other is opaque and ordinary.
///
/// Declared **too-tight**, on the same reading as the lists beside it: an
/// escape is not a fail-closed. `eval "$payload"`, a wrapper script, a
/// base64-decoded here-doc and `$(printf '\x3e')` all reach a write without
/// showing one. The first of those is the **variable**: `eval` handed code that
/// is written out is read, like every other interpreter, and so is a write
/// inside backticks — the same construct as `$( )` in the other spelling, and
/// the only one of the two that nothing saw. The proof boundary: these spellings are gated; the set is not
/// complete, and a shell is not a language you can close this way.
///
/// A false positive costs one tracker read before a write that was going to be
/// verified anyway. A false negative costs the guarantee, so the asymmetry runs
/// the same direction as every other list here.
pub fn writes_a_file(command: &str) -> Option<String> {
    if redirects_into_a_file(command) {
        return Some("a redirect into a file".to_owned());
    }

    for segment in segments(command) {
        let mut words = segment.split_whitespace();
        let Some(head) = words.next().map(basename) else {
            continue;
        };
        // A wrapper runs the next command, and the next command is still there
        // to be read. See [`RUNS_ANOTHER_COMMAND`].
        if RUNS_ANOTHER_COMMAND.contains(&head)
            && let Some(wrapped) = words
                .clone()
                .map(basename)
                .find(|word| WRITES_A_FILE.contains(word))
        {
            return Some(format!("{wrapped} under {head}"));
        }
        // `find … -delete` deletes, and says so on the line. `-exec` hands the
        // rest to another command, which is the wrapper case one line up.
        if head == "find" {
            let rest: Vec<&str> = words.clone().collect();
            if rest.contains(&"-delete") {
                return Some("find -delete".to_owned());
            }
            if let Some(run) = rest
                .iter()
                .map(|word| basename(word))
                .find(|word| WRITES_A_FILE.contains(word))
                && rest.iter().any(|word| word.starts_with("-exec"))
            {
                return Some(format!("{run} under find -exec"));
            }
        }
        // `dd` writes only when told where to; `dd if=x` is a read.
        if head == "dd" {
            if words.any(|word| word.starts_with("of=")) {
                return Some("dd of=".to_owned());
            }
            continue;
        }
        if WRITES_A_FILE.contains(&head) {
            return Some(head.to_owned());
        }
        let flags: Vec<&str> = words.collect();
        // `curl URL` prints; `curl -o x`, `curl -O` and `curl -fsSLo x` put it
        // on disk. Read the same way `dd` is: the name alone says nothing, the
        // flag says everything. The short forms are clustered in practice —
        // `-fsSLo out.txt` is how it is actually typed — so a single-dash
        // cluster carrying an `o` counts, which costs a read on the rare
        // `curl -o` that meant something else and catches every spelling of
        // `curl -o src/main.rs`.
        if head == "curl"
            && flags.iter().any(|word| {
                *word == "--output"
                    || *word == "--remote-name"
                    || word.starts_with("--output=")
                    || (word.starts_with('-')
                        && !word.starts_with("--")
                        && word.contains(['o', 'O']))
            })
        {
            return Some("curl -o".to_owned());
        }
        // The same reading for the other common tool that names its output on
        // the line: `openssl … -out cert.csr` puts a file there, and `openssl
        // dgst` without one does not.
        if head == "openssl"
            && flags
                .iter()
                .any(|word| *word == "-out" || *word == "-keyout")
        {
            return Some("openssl -out".to_owned());
        }
        if INLINE_CODE.contains(&head) && flags.iter().any(|word| carries_code(head, word)) {
            return Some(format!("{head} with code on the command line"));
        }
        // The shell's own. `eval` is `sh -c` without the `sh` or the `-c`: it
        // takes code and runs it, and it was read as a word that writes
        // nothing. Read here rather than as a wrapper, because a wrapper is
        // asked which command follows it and `eval 'rm -rf src'` puts a quote
        // against the `rm` — the same reason every interpreter above is read by
        // its own name and not by what it was handed.
        if head == "eval" && !flags.is_empty() {
            return Some("eval with code on the command line".to_owned());
        }
        // `-i`, and also `-i.bak` and `-ne -i`, which are the same flag.
        if IN_PLACE.contains(&head)
            && flags.iter().any(|word| {
                *word == "-i" || word.starts_with("-i.") || word.starts_with("--in-place")
            })
        {
            return Some(format!("{head} -i"));
        }
        // A **long** write flag on any command at all. The population above is
        // drawn at *what is visible on the command line*, and `--write` is as
        // visible as a redirect — but it was read only for the handful of
        // commands `IN_PLACE` names, so `prettier --write src` and
        // `eslint --fix .` rewrote every file in a tree and classified as
        // nothing at all.
        //
        // Long forms only, and that is the whole reason this is a shape rather
        // than a catalogue: `-i` is *ignore case* to `grep` and `-w` is *whole
        // word*, so reading either everywhere would report a search as a write.
        // Nothing spells `--write` or `--in-place` to mean anything but writing,
        // and `--fix` is how `eslint`, `ruff` and `clippy` all say it.
        //
        // What is still not matched is a formatter whose default is to rewrite:
        // `cargo fmt`, `black .` and `gofmt -w` show no long flag, and the last
        // of those shows a short one that means something else elsewhere. That
        // is in the honesty contract with the measurement, because it is a gap
        // and not a decision.
        if let Some(flag) = flags
            .iter()
            .find(|word| matches!(**word, "--write" | "--in-place" | "--fix"))
        {
            return Some(format!("{head} {flag}"));
        }
    }

    None
}

/// The last path component, so `/usr/bin/cp` and `cp` are one command.
///
/// The extension goes with it, because on Windows the same command arrives as
/// `cmd`, `cmd.exe` and `C:\Windows\System32\cmd.exe`, and a list that only
/// knows the bare word knows one of the three.
fn basename(word: &str) -> &str {
    let last = word.rsplit(['/', '\\']).next().unwrap_or(word);
    match last.rsplit_once('.') {
        // No guard on an empty stem. `.exe` alone would return `""`, which is in
        // none of the lists — and so is `.exe` itself, so the guard that used to
        // sit here changed no answer. `cargo mutants` found it by replacing it
        // with `true` and nothing noticed.
        Some((stem, "exe" | "cmd" | "bat" | "com")) => stem,
        _ => last,
    }
}

/// The command line split where one command ends and the next begins.
///
/// Only the separators that start a fresh command word, so the head of each
/// piece is a command name. `>` is not one of them — it belongs to the command
/// it follows, and [`redirects_into_a_file`] has already read it.
fn segments(command: &str) -> Vec<&str> {
    command
        // The backtick is here for the same reason `(` and `)` are: it opens a
        // command. Without it `echo $(rm -rf src)` was read and the identical
        // line spelled with backticks was not — one construct, two spellings,
        // and only one of them seen. A list that stops halfway is not a
        // boundary.
        .split(['\n', ';', '|', '&', '(', ')', '`'])
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .collect()
}

/// Whether a `>` outside quotes opens a file.
///
/// Quote-aware because `echo "a > b"` writes nothing, and descriptor-aware
/// because `2>&1` opens nothing. Everything else that reaches a bare `>` is a
/// file being created or truncated.
fn redirects_into_a_file(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut quote: Option<u8> = None;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(open) = quote {
            if byte == open {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            // A backslash outside quotes hides whatever follows it.
            b'\\' => index += 2,
            b'\'' | b'"' => {
                quote = Some(byte);
                index += 1;
            }
            b'>' => {
                // `>>` appends and is one redirect, not two.
                //
                // `cargo mutants` leaves one survivor here — `index + 1` into
                // `index * 1` — and it is *equivalent*, not a gap: the loop below
                // starts on the `>` itself and walks past it, so both spellings
                // land on the same byte. Recorded rather than chased, because an
                // unkillable mutant costs the next reader an afternoon.
                let mut after = index + 1;
                while bytes.get(after) == Some(&b'>') {
                    after += 1;
                }
                // `2>&1` and `>&2` duplicate a descriptor. `&> log` does not:
                // there the `&` sits *before* the `>`, and the name after it is
                // a file.
                //
                // No spaces are skipped before this, and that is deliberate: a
                // descriptor is duplicated only when the `&` is adjacent, so
                // `> &1` is not a shell that runs. The loop that used to skip
                // them changed no answer — and it made `> 1.log` unable to tell
                // this `&&` from an `||`, which is how a mutant survived a test
                // written to kill it.
                let duplicates = bytes.get(after) == Some(&b'&')
                    && bytes
                        .get(after + 1)
                        .is_some_and(|next| next.is_ascii_digit() || *next == b'-');
                if !duplicates {
                    return true;
                }
                index = after + 2;
            }
            _ => index += 1,
        }
    }

    false
}

#[cfg(test)]
mod tests;
