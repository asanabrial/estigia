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

use super::population::{Disposition, Population};

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
    // The rest of the pack-and-unpack family: the declaration names *unpack* as
    // in scope, and these are the same utilities under different names.
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
    "su",
    "script",
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
/// PowerShell resolves a parameter from **any prefix that is not ambiguous**, so
/// a list of spellings is always one spelling short; matched by prefix, and
/// ambiguity is deliberately not modelled — a prefix PowerShell would reject
/// runs nothing, so reading it as code costs one question that never had to be
/// asked, while the other way round is a write nobody saw.
///
/// `ec` is here beside the two full names because a prefix rule alone was **not
/// enough**: `-ec` is a prefix of neither parameter name, and it is one of the
/// two short aliases Microsoft publishes for `-EncodedCommand`.
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

/// The population and boundary `RUNS_ANOTHER_COMMAND` enforces.
pub const RUNS_ANOTHER_COMMAND_POPULATION: Population = Population {
    family: "running-shell",
    disposition: Disposition::TooTight,
    subject: "RUNS_ANOTHER_COMMAND",
    text: "the prefixes that put somebody else's command after their own, in the spellings a \
           person types. Legitimate population: every wrapper that runs the next word. \
           Boundary: the wrapped command is still **visible on the command line**, which is \
           the same boundary `writing-shell` draws \u{2014} and that is why these belong to it \
           rather than being an escape from it.",
};

/// Commands whose own name is not the command being run.
///
/// `sudo rm -rf src` hides nothing — the head of the segment was `sudo` and
/// `sudo` writes no files — and the same is true of every wrapper here: a
/// spelling of a write the list above already knows, wearing a hat. The
/// population's own sentence — *every wrapper that runs the next word* — is
/// what fills the list, not a reading of it.
///
/// `chrt` is here as `nice` and `ionice`'s third sibling: it sets the
/// scheduling policy and runs the rest of the line. Said rather than left to
/// be noticed, because a list is only as good as the account of how each entry
/// got in.
///
/// `su -c '…'` is **not** here, and the difference is the boundary itself: it
/// takes its command as a quoted argument rather than as the words that
/// follow, so it is inline code and belongs to the interpreter arm of
/// `writing-shell`, which reads it. `script` is **here as well as there**:
/// util-linux's `script -c '…'` is that same quoted form, while BSD `script`
/// — darwin's — takes its command as
/// the words after the optional typescript file, where no code flag looks.
/// `script -q /dev/null echo …` printed the command's output; measured on
/// 2026-09-27.
///
/// After one of these, **any** later word naming a write is taken as the write.
/// Not the next word: `sudo -u bob rm x` puts two between them, and a scheme
/// that tries to know which flags take arguments has to know it per wrapper.
/// The cost is `sudo grep -r rm .` — or `script -c 'echo hi' rm`, a typescript
/// file named for a write — reading the tracker once, which is the asymmetry
/// this whole module is built on: a false positive costs a read, a false
/// negative costs the guarantee.
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
    "script",
    "xvfb-run",
    "proxychains",
    "torify",
];

/// The population and boundary [`writes_a_file`] enforces.
pub const WRITES_A_FILE_POPULATION: Population = Population {
    family: "writing-shell",
    disposition: Disposition::TooTight,
    subject: "writes_a_file",
    text: "the shell constructs whose visible effect is writing a file \u{2014} a redirect, an \
           in-place edit, an interpreter handed code, and the utilities that copy, move, \
           delete or unpack \u{2014} in their POSIX *and* Windows spellings, since this crate's \
           own platform is the one where `cmd /c` and `Set-Content` are the ordinary way to \
           write. Legitimate population: every shell spelling of a repository write. Boundary: \
           what is *visible on the command line*. A program that writes as a side effect \u{2014} \
           `make`, `cargo build`, `npm run`, an interpreter given a script file \u{2014} is not \
           matched, and is not meant to be.",
};

/// The spelling [`writes_a_file`] answers for a redirect into a file.
pub const REDIRECT: &str = "a redirect into a file";

/// Whether this command line writes a file, and the spelling that says so.
///
/// That boundary is why a **long write flag** on any command counts, whatever
/// the command is: `--write`, `--in-place` and `--fix` are as visible as a
/// redirect. Long forms only — `-i` is *ignore case* to `grep` and `-w` is
/// *whole word*, so reading either everywhere would report a search as a write.
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
    if runs_a_program_after_pre(command) {
        return Some("rg --pre".to_owned());
    }
    if redirects_into_a_file(command) {
        return Some(REDIRECT.to_owned());
    }

    for segment in segments(command) {
        for (wrapper, words) in candidate_commands(segment) {
            let head = basename(words[0]);
            let rest = &words[1..];
            if wrapper.is_none()
                && RUNS_ANOTHER_COMMAND.contains(&head)
                && let Some(wrapped) = rest
                    .iter()
                    .map(|word| basename(word))
                    .find(|word| WRITES_A_FILE.contains(word))
            {
                return Some(format!("{wrapped} under {head}"));
            }
            if let Some(found) = reads_as_a_write(head, rest) {
                return Some(match wrapper {
                    Some(wrapper) => format!("{found} under {wrapper}"),
                    None => found,
                });
            }
        }
    }

    None
}

fn reads_as_a_write(head: &str, flags: &[&str]) -> Option<String> {
    if head == "find" {
        if flags.contains(&"-delete") {
            return Some("find -delete".to_owned());
        }
        if let Some(run) = flags
            .iter()
            .map(|word| basename(word))
            .find(|word| WRITES_A_FILE.contains(word))
            && flags.iter().any(|word| word.starts_with("-exec"))
        {
            return Some(format!("{run} under find -exec"));
        }
    }
    if head == "dd" {
        return flags
            .iter()
            .any(|word| word.starts_with("of="))
            .then(|| "dd of=".to_owned());
    }
    if WRITES_A_FILE.contains(&head) {
        return Some(head.to_owned());
    }
    if head == "curl" && curl_writes_a_file(flags) {
        return Some("curl -o".to_owned());
    }
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
    if head == "eval" && !flags.is_empty() {
        return Some("eval with code on the command line".to_owned());
    }
    if IN_PLACE.contains(&head)
        && flags
            .iter()
            .any(|word| *word == "-i" || word.starts_with("-i.") || word.starts_with("--in-place"))
    {
        return Some(format!("{head} -i"));
    }
    flags
        .iter()
        .find(|word| matches!(**word, "--write" | "--in-place" | "--fix"))
        .map(|flag| format!("{head} {flag}"))
}

/// Shell grammar words that precede the command they introduce.
const RESERVED_WORDS: &[&str] = &[
    "if", "then", "else", "elif", "do", "while", "until", "!", "{",
];

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn skip_prefix<'a, 'b>(mut words: &'b [&'a str]) -> &'b [&'a str] {
    while let Some(first) = words.first() {
        if RESERVED_WORDS.contains(&first.to_ascii_lowercase().as_str()) || is_assignment(first) {
            words = &words[1..];
        } else {
            break;
        }
    }
    words
}

/// Every command a segment may run: its head once reserved words and
/// assignments are skipped, and, behind a wrapper, every later word read as a
/// head of its own, since which wrapper options take an argument is not known.
fn candidate_commands(segment: &str) -> Vec<(Option<&str>, Vec<&str>)> {
    let words: Vec<&str> = segment.split_whitespace().collect();
    let start = skip_prefix(&words);
    let Some(first) = start.first() else {
        return Vec::new();
    };
    let mut out = vec![(None, start.to_vec())];
    let wrapper = basename(first);
    if RUNS_ANOTHER_COMMAND.contains(&wrapper) {
        for at in 1..start.len() {
            let tail = skip_prefix(&start[at..]);
            if !tail.is_empty() {
                out.push((Some(wrapper), tail.to_vec()));
            }
        }
    }
    out
}

/// `rg --pre COMMAND` runs COMMAND on every file it searches, so the program is
/// on the line and is read the way a wrapper's is. `rg` alone is a search.
pub fn runs_a_program_after_pre(command: &str) -> bool {
    segments(command).into_iter().any(|segment| {
        candidate_commands(segment).iter().any(|(_, words)| {
            basename(words[0]) == "rg"
                && words[1..]
                    .iter()
                    .any(|word| *word == "--pre" || word.starts_with("--pre="))
        })
    })
}

/// The last path component, so `/usr/bin/cp` and `cp` are one command.
///
/// The extension goes with it, because on Windows the same command arrives as
/// `cmd`, `cmd.exe` and `C:\Windows\System32\cmd.exe`, and a list that only
/// knows the bare word knows one of the three.
fn basename(word: &str) -> &str {
    let last = word.rsplit(['/', '\\']).next().unwrap_or(word);
    match last.rsplit_once('.') {
        // No guard on an empty stem: `.exe` alone returns `""`, which is in
        // none of the lists — and so is `.exe` itself, so a guard here changes
        // no answer.
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
        // command, and a list that stops halfway is not a boundary.
        .split(['\n', ';', '|', '&', '(', ')', '`'])
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .collect()
}

/// Whether a `>` outside quotes opens a file.
///
/// Quote-aware because `echo "a > b"` writes nothing, and descriptor-aware
/// because `2>&1` opens nothing. A target that is `/dev/null`, `/dev/stdout`,
/// `/dev/stderr` or `/dev/fd/<n>` opens nothing either. Everything else that reaches a bare `>` is a
/// file being created or truncated. A line that ends inside a quote that hid a `>` is read as a
/// write.
fn redirects_into_a_file(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut quote: Option<u8> = None;
    let mut ansi = false;
    let mut hid_an_arrow = false;
    let mut comment = false;
    let mut pending: Vec<(String, bool, bool)> = Vec::new();
    // The open parentheses of each `$(` entered from inside double quotes, where quoting restarts.
    let mut substitutions: Vec<usize> = Vec::new();
    let nests = !command.contains("case");
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(open) = quote {
            if byte == b'\\' && (ansi || open == b'"') {
                index += 2;
                continue;
            }
            if nests
                && open == b'"'
                && byte == b'$'
                && bytes.get(index + 1) == Some(&b'(')
                && bytes.get(index + 2) != Some(&b'(')
            {
                substitutions.push(0);
                quote = None;
                index += 2;
                continue;
            }
            if byte == b'>' {
                hid_an_arrow = true;
            }
            if byte == open {
                quote = None;
                ansi = false;
            }
            index += 1;
            continue;
        }
        if comment {
            if byte == b'\n' {
                comment = false;
                continue;
            }
            if byte != b'>' {
                index += 1;
                continue;
            }
        }
        match byte {
            b'(' if !substitutions.is_empty() => {
                if let Some(open) = substitutions.last_mut() {
                    *open += 1;
                }
                index += 1;
            }
            b')' if !substitutions.is_empty() => {
                if substitutions.last() == Some(&0) {
                    substitutions.pop();
                    quote = Some(b'"');
                } else if let Some(open) = substitutions.last_mut() {
                    *open -= 1;
                }
                index += 1;
            }
            b'\\' => index += 2,
            b'\'' | b'"' => {
                quote = Some(byte);
                ansi = byte == b'\'' && index > 0 && bytes[index - 1] == b'$';
                hid_an_arrow = false;
                index += 1;
            }
            b'#' if index == 0 || b" \t\n;|&()".contains(&bytes[index - 1]) => {
                comment = true;
                index += 1;
            }
            b'<' if bytes.get(index + 1) == Some(&b'<') => {
                if bytes.get(index + 2) == Some(&b'<') {
                    index += 3;
                } else if let Some((delimiter, strip_tabs, quoted, next)) =
                    heredoc_word(command, index)
                {
                    pending.push((delimiter, strip_tabs, quoted));
                    index = next;
                } else {
                    index += 2;
                }
            }
            b'\n' => {
                index += 1;
                for (delimiter, strip_tabs, quoted) in pending.drain(..) {
                    match here_document(
                        command,
                        index,
                        &delimiter,
                        strip_tabs,
                        quoted,
                        &mut Vec::new(),
                    ) {
                        Some(next) => index = next,
                        None => break,
                    }
                }
            }
            b'>' => {
                // `>>` appends and is one redirect, not two. (A mutant survives
                // here and is *equivalent*, not a gap: both spellings land on
                // the same byte.)
                let mut after = index + 1;
                while bytes.get(after) == Some(&b'>') {
                    after += 1;
                }
                // `2>&1` and `>&2` duplicate a descriptor. `&> log` does not:
                // there the `&` sits *before* the `>`, and the name after it is
                // a file. No spaces are skipped first, deliberately: a
                // descriptor is duplicated only when the `&` is adjacent.
                let duplicates = bytes.get(after) == Some(&b'&')
                    && bytes
                        .get(after + 1)
                        .is_some_and(|next| next.is_ascii_digit() || *next == b'-');
                if !duplicates {
                    match discarded_target(bytes, after) {
                        Some(end) => index = end,
                        None => return true,
                    }
                } else {
                    index = after + 2;
                }
            }
            _ => index += 1,
        }
    }

    quote.is_some() && hid_an_arrow
}

const DISCARDING_TARGETS: [&str; 3] = ["/dev/null", "/dev/stdout", "/dev/stderr"];

fn curl_writes_a_file(flags: &[&str]) -> bool {
    let mut index = 0;
    while index < flags.len() {
        let word = flags[index];
        let output = if word == "--output" {
            index += 1;
            Some(flags.get(index).copied())
        } else if let Some(target) = word.strip_prefix("--output=") {
            Some(Some(target))
        } else if word == "--remote-name" {
            Some(None)
        } else if word.starts_with('-') && !word.starts_with("--") && word.contains(['o', 'O']) {
            let after = &word[word.find(['o', 'O']).map_or(word.len(), |at| at + 1)..];
            if after.is_empty() {
                index += 1;
                Some(flags.get(index).copied())
            } else {
                Some(Some(after))
            }
        } else {
            None
        };
        if let Some(target) = output
            && !target.is_some_and(|target| DISCARDING_TARGETS.contains(&target))
        {
            return true;
        }
        index += 1;
    }
    false
}

/// The word after a redirect operator, and where it ends: optionally quoted,
/// and `None` when a quote never closes or a word is glued to the closing one.
fn target_word(bytes: &[u8], from: usize) -> Option<(&str, usize)> {
    let mut start = from;
    while matches!(bytes.get(start), Some(b' ' | b'\t')) {
        start += 1;
    }
    let quote = bytes
        .get(start)
        .copied()
        .filter(|byte| matches!(byte, b'\'' | b'"'));
    let word_start = start + usize::from(quote.is_some());
    let mut end = word_start;
    while let Some(&byte) = bytes.get(end) {
        let ends = match quote {
            Some(open) => byte == open,
            None => byte.is_ascii_whitespace() || b";|&()<>`".contains(&byte),
        };
        if ends {
            break;
        }
        end += 1;
    }
    if quote.is_some() {
        bytes.get(end)?;
        let closed = end + 1;
        if bytes
            .get(closed)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !b";|&()<>`".contains(byte))
        {
            return None;
        }
        end = closed;
    }
    let word = std::str::from_utf8(&bytes[word_start..end - usize::from(quote.is_some())]).ok()?;
    Some((word, end))
}

fn is_discarded(word: &str) -> bool {
    DISCARDING_TARGETS.contains(&word)
        || word
            .strip_prefix("/dev/fd/")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Where the redirect target starting at `from` ends, when it names a stream
/// rather than a file: an exact word, optionally quoted. Anything else,
/// including a word that merely begins with one of them, is a file.
fn discarded_target(bytes: &[u8], from: usize) -> Option<usize> {
    let (word, end) = target_word(bytes, from)?;
    is_discarded(word).then_some(end)
}

/// How many bytes of `path` are a drive letter and its colon, as in `C:/…`: two
/// when a slash follows them, and none for `C:`, `C:name` or anything else.
pub(crate) fn drive_prefix_len(path: &str) -> usize {
    match path.as_bytes() {
        [letter, b':', b'/', ..] if letter.is_ascii_alphabetic() => 2,
        _ => 0,
    }
}

fn is_absolute_literal(target: &str) -> bool {
    target.starts_with('/') || drive_prefix_len(target) > 0
}

/// The target of a `cat` that writes one quoted heredoc to one literal absolute path, and nothing
/// else.
///
/// The first line is exactly `cat > TARGET <<'DELIM'` (or `>>`, or the redirect after the heredoc
/// word), the delimiter quoted with `'` or `"`, and the first line equal to `DELIM` is the last line.
/// The body is never read. Anything else, including a target outside `[A-Za-z0-9_./+@%:,-]`, a
/// relative one or one holding `..`, is `None`. An absolute target is `/…` or a drive-lettered `X:/…`;
/// `X:` alone and `X:name` are drive-relative and are `None`.
pub fn quoted_heredoc_target(command: &str) -> Option<String> {
    let (head, rest) = command.split_once('\n')?;
    if head.starts_with([' ', '\t']) || head.ends_with([' ', '\t']) {
        return None;
    }
    let tokens: Vec<&str> = head
        .split([' ', '\t'])
        .filter(|token| !token.is_empty())
        .collect();
    let (redirect, target, opener) = match tokens.as_slice() {
        ["cat", redirect, target, opener] if opener.starts_with("<<") => (redirect, target, opener),
        ["cat", opener, redirect, target] if opener.starts_with("<<") => (redirect, target, opener),
        _ => return None,
    };
    if !matches!(*redirect, ">" | ">>") {
        return None;
    }
    let quoted = opener.strip_prefix("<<")?;
    let delimiter = ["'", "\""].iter().find_map(|quote| {
        quoted
            .strip_prefix(quote)
            .and_then(|inner| inner.strip_suffix(quote))
    })?;
    if delimiter.is_empty()
        || !delimiter
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    if !is_absolute_literal(target)
        || target.contains("..")
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./+@%:,-".contains(&byte))
    {
        return None;
    }
    let mut remaining = rest;
    loop {
        let (line, after) = match remaining.split_once('\n') {
            Some((line, after)) => (line, Some(after)),
            None => (remaining, None),
        };
        if line == delimiter {
            return after
                .is_none_or(str::is_empty)
                .then(|| (*target).to_owned());
        }
        remaining = after?;
    }
}

/// The files a command line writes, when redirects are the only thing on it that does.
///
/// The one quoted-heredoc shape [`quoted_heredoc_target`] accepts is answered first, line breaks
/// and quotes included. Otherwise `None` unless the line is one plain command whose program only prints (`echo`, `printf`, `cat`,
/// `ls`, `grep`, `rg`, `head`, `tail`, `wc`, `date`, `pwd`): any quote, `$`, backtick, backslash, `<`, `|`, `;`, `&`,
/// parenthesis, line break or `..` refuses it, as does a non-literal
/// target, another writer on the line (`cp`, `sed -i`, `tee`), or no file redirected into at all.
pub fn redirect_targets(command: &str) -> Option<Vec<String>> {
    if let Some(target) = quoted_heredoc_target(command) {
        return Some(vec![target]);
    }
    const PRINTS_ONLY: &[&str] = &[
        "echo", "printf", "cat", "ls", "grep", "rg", "head", "tail", "wc", "date", "pwd",
    ];
    if !command
        .split_whitespace()
        .next()
        .is_some_and(|word| PRINTS_ONLY.contains(&word))
    {
        return None;
    }
    if command.contains([
        '<', '|', ';', '&', '(', ')', '$', '`', '\'', '"', '\\', '\n', '\r',
    ]) || command.contains("..")
    {
        return None;
    }
    let bytes = command.as_bytes();
    let mut targets = Vec::new();
    let mut rest = Vec::with_capacity(bytes.len());
    let mut quote: Option<u8> = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(open) = quote {
            if byte == open {
                quote = None;
            }
            rest.push(byte);
            index += 1;
            continue;
        }
        match byte {
            b'\\' => {
                rest.extend_from_slice(&bytes[index..bytes.len().min(index + 2)]);
                index += 2;
            }
            b'\'' | b'"' => {
                quote = Some(byte);
                rest.push(byte);
                index += 1;
            }
            b'>' => {
                let mut after = index + 1;
                while bytes.get(after) == Some(&b'>') {
                    after += 1;
                }
                let duplicates = bytes.get(after) == Some(&b'&')
                    && bytes
                        .get(after + 1)
                        .is_some_and(|next| next.is_ascii_digit() || *next == b'-');
                if duplicates {
                    rest.extend_from_slice(&bytes[index..after + 2]);
                    index = after + 2;
                    continue;
                }
                let (word, end) = target_word(bytes, after)?;
                if !is_discarded(word) {
                    if word.is_empty() || word.contains(['$', '\\', '*', '?', '[', '{', '~', '=']) {
                        return None;
                    }
                    targets.push(word.to_owned());
                }
                rest.push(b' ');
                index = end;
            }
            _ => {
                rest.push(byte);
                index += 1;
            }
        }
    }
    if targets.is_empty() {
        return None;
    }
    let rest = String::from_utf8(rest).ok()?;
    writes_a_file(&rest.to_ascii_lowercase())
        .is_none()
        .then_some(targets)
}

/// The commands a data span may hide behind: a program with no code flag, no
/// `--exec`-shaped option, and no subcommand that runs its argument.
///
/// Every entry reads the bytes it is handed. `rg` and `sort` are deliberately
/// absent — `rg --pre` and `sort --compress-program` run the program they are
/// given — and so is every interpreter, wrapper, scheduler and package manager,
/// which is the whole point of the list being an allowlist rather than a
/// denylist.
const READS_ONLY: &[&str] = &[
    "gh", "cat", "grep", "head", "tail", "wc", "jq", "ls", "uniq", "cut", "tr", "echo", "date",
    "pwd", "true", "false", "basename", "dirname", "realpath", "stat", "file", "which", "type",
];

/// What stands in for a run of shell text that is data rather than code.
///
/// Not whitespace, so the word boundary the quote sat on survives, and no
/// command word holds this byte, so nothing matches through it.
const DATA: char = '\u{1}';

/// The `gh` subcommands that read or write the tracker and run nothing the line
/// hands them. `browse` opens the user's browser, `auth` exposes credential
/// helpers, `extension`, `codespace`, `alias` and an alias a user defined run
/// what they are given, and `repo` and `gist` hand the words after `--` to
/// `git`, whose `--upload-pack` and `--config` run a program, so none of them is
/// here.
const GH_READS: &[&str] = &[
    "api", "issue", "pr", "run", "release", "search", "status", "label", "project", "workflow",
];

/// The bytes an unquoted word may hold besides ASCII alphanumerics. `>` is
/// here because a redirect writes a file and runs nothing, and `writes_a_file`
/// reads the view, in which `>` and its unquoted target are never blanked.
const SAFE_BYTES: &[u8] = b"-_./:,=+@%>";

/// `command` with the quotes taken off every quoted run that holds only word
/// bytes, as bash's quote removal does before it runs a word.
///
/// A fragment list searches text, and `git 'push'` holds no `git push`. Only a
/// run of `[A-Za-z0-9_\-./:,@%+]` is unquoted: a run holding a blank, an
/// expansion, a backslash or a glob keeps its quotes, so nothing is unquoted
/// that the shell would read differently. The scan follows `$(…)`, backticks and
/// double quotes, so a quoted word inside a substitution is unquoted too. A
/// quote that never closes, or a substitution that does, returns the line as it
/// came: what this cannot read, it does not touch.
pub(crate) fn unquote_plain_words(command: &str) -> String {
    let mut out = Vec::with_capacity(command.len());
    match unquote(command.as_bytes(), 0, &mut out, Scope::Line) {
        Some(_) => String::from_utf8(out).unwrap_or_else(|_| command.to_owned()),
        None => command.to_owned(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Line,
    Parens,
    Backticks,
    Double,
}

fn is_plain(content: &[u8]) -> bool {
    content
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_-./:,@%+".contains(byte))
}

fn names_a_here_document_delimiter(out: &[u8]) -> bool {
    let mut end = out.len();
    while end > 0 && is_blank(out[end - 1]) {
        end -= 1;
    }
    out[..end].ends_with(b"<<") || out[..end].ends_with(b"<<-")
}

fn begins_a_word(out: &[u8]) -> bool {
    out.last()
        .is_none_or(|byte| is_blank(*byte) || b";|&(".contains(byte))
}

fn drop_marker_before_quote(out: &mut Vec<u8>) -> bool {
    if out.last() != Some(&b'$') {
        return true;
    }
    if begins_a_word(&out[..out.len() - 1]) {
        out.pop();
        return true;
    }
    false
}

fn unquote(bytes: &[u8], mut index: usize, out: &mut Vec<u8>, scope: Scope) -> Option<usize> {
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b'\\' => {
                out.push(byte);
                if let Some(next) = bytes.get(index + 1) {
                    out.push(*next);
                }
                index += 2;
            }
            b'\'' if scope != Scope::Double => {
                let close = index + 1 + bytes[index + 1..].iter().position(|b| *b == b'\'')?;
                let content = &bytes[index + 1..close];
                let keep = !is_plain(content)
                    || names_a_here_document_delimiter(out)
                    || !drop_marker_before_quote(out);
                if keep {
                    out.extend_from_slice(&bytes[index..=close]);
                } else {
                    out.extend_from_slice(content);
                }
                index = close + 1;
            }
            b'"' if scope != Scope::Double => {
                let dollar = out.last() == Some(&b'$');
                let marker = dollar && begins_a_word(&out[..out.len() - 1]);
                let start = out.len();
                out.push(b'"');
                index = unquote(bytes, index + 1, out, Scope::Double)?;
                let plain = is_plain(&out[start + 1..out.len() - 1]);
                let delimiter = names_a_here_document_delimiter(&out[..start]);
                if plain && !delimiter && (!dollar || marker) {
                    out.pop();
                    out.remove(start);
                    if marker {
                        out.remove(start - 1);
                    }
                }
            }
            b'"' => {
                out.push(byte);
                return Some(index + 1);
            }
            b'$' if bytes.get(index + 1) == Some(&b'(') => {
                out.extend_from_slice(b"$(");
                index = unquote(bytes, index + 2, out, Scope::Parens)?;
            }
            b'(' if scope == Scope::Parens => {
                out.push(byte);
                index = unquote(bytes, index + 1, out, Scope::Parens)?;
            }
            b')' if scope == Scope::Parens => {
                out.push(byte);
                return Some(index + 1);
            }
            b'`' if scope == Scope::Backticks => {
                out.push(byte);
                return Some(index + 1);
            }
            b'`' => {
                out.push(byte);
                index = unquote(bytes, index + 1, out, Scope::Backticks)?;
            }
            _ => {
                out.push(byte);
                index += 1;
            }
        }
    }
    (scope == Scope::Line).then_some(index)
}

/// The command line with the spans a single simple command merely reads blanked.
///
/// A line is eligible only when [`read_only_spans`] can parse the whole of it
/// by its grammar; anything else is returned unchanged, so the whole-text scan
/// stays the floor and a construct this reader does not model is read as code.
pub(crate) fn data_view(command: &str) -> String {
    match read_only_spans(command, false).or_else(|| compound_spans(command)) {
        Some(spans) => blank(command, &spans),
        None => command.to_owned(),
    }
}

/// Words that let a line redefine what a read-only name runs, choose the program it resolves to,
/// or hide a `)` the lexer below would take for a boundary.
const COMPOUND_REFUSED: &[&str] = &[
    "PATH", "ENV", "alias", "function", "hash", "enable", "case", "esac", "coproc",
];

/// The spans each top-level simple command of a compound line reads, by the grammar
/// [`read_only_spans`] applies to a lone command, or `None` when the line holds a construct
/// through which data could reach execution: a pipe into anything outside [`READS_ONLY`], a
/// redirect, a here-document, a process substitution, `${`, `$'`, a comment or a backslash.
/// Nothing inside a command substitution or backticks is blanked.
fn compound_spans(command: &str) -> Option<Vec<(usize, usize)>> {
    if COMPOUND_REFUSED.iter().any(|word| command.contains(word)) {
        return None;
    }
    let bytes = command.as_bytes();
    let mut pieces = Vec::new();
    let mut piped = false;
    let mut depth = 0usize;
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let separator = match byte {
            b'\'' => {
                index = single_quoted_end(bytes, index)?;
                continue;
            }
            b'"' => {
                index = double_quoted_end(bytes, index)?;
                continue;
            }
            b'`' => {
                index = backticks_end(bytes, index)?;
                continue;
            }
            b'$' => {
                index = dollar_end(bytes, index)?;
                continue;
            }
            b'#' if index == 0 || is_blank(bytes[index - 1]) => return None,
            b'\\' | b'\r' | b'>' => return None,
            b'<' => {
                match (bytes.get(index + 1), bytes.get(index + 2)) {
                    (Some(b'('), _) => return None,
                    (Some(b'<'), Some(b'<')) => index += 3,
                    (Some(b'<'), _) => return None,
                    _ => index += 1,
                }
                continue;
            }
            b'(' => {
                if matches!(bytes.get(index + 1), Some(b'(' | b')'))
                    || (index > 0 && bytes[index - 1] == b'=')
                {
                    return None;
                }
                depth += 1;
                1
            }
            b')' => {
                depth = depth.checked_sub(1)?;
                1
            }
            b';' | b'\n' => 1,
            b'&' | b'|' => {
                if byte == b'|' && bytes.get(index + 1) != Some(&b'|') {
                    piped = true;
                }
                1 + usize::from(matches!(bytes.get(index + 1), Some(b'&' | b'|')))
            }
            _ => {
                index += 1;
                continue;
            }
        };
        pieces.push((start, index));
        index += separator;
        start = index;
    }
    if depth != 0 {
        return None;
    }
    pieces.push((start, bytes.len()));
    let mut spans = Vec::new();
    for (start, end) in pieces {
        let piece = &command[start..end];
        let body = piece.len() - without_reserved_words(piece).len();
        let text = &piece[body..];
        if text.trim().is_empty() {
            continue;
        }
        if piped && !bare_word(text, 0).is_some_and(|(head, _)| READS_ONLY.contains(&head)) {
            return None;
        }
        if let Some(found) = read_only_spans(text.trim_end(), true) {
            let offset = start + body;
            spans.extend(
                found
                    .into_iter()
                    .map(|(from, to)| (from + offset, to + offset)),
            );
        }
    }
    Some(spans)
}

/// `piece` with the reserved words that may open a simple command taken off its front.
fn without_reserved_words(piece: &str) -> &str {
    let mut rest = piece.trim_start();
    loop {
        let word = rest.split([' ', '\t']).next().unwrap_or("");
        if ![
            "do", "then", "else", "elif", "if", "while", "until", "!", "{", "}",
        ]
        .contains(&word)
        {
            return rest;
        }
        rest = rest[word.len()..].trim_start();
    }
}

fn single_quoted_end(bytes: &[u8], open: usize) -> Option<usize> {
    Some(open + 2 + bytes[open + 1..].iter().position(|byte| *byte == b'\'')?)
}

fn double_quoted_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut index = open + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some(index + 1),
            b'\\' if bytes.get(index + 1) == Some(&b'\n') => return None,
            b'\\' => index += 2,
            b'`' => index = backticks_end(bytes, index)?,
            b'$' if bytes.get(index + 1) == Some(&b'{') => return None,
            b'$' if bytes.get(index + 1) == Some(&b'(') => index = dollar_end(bytes, index)?,
            _ => index += 1,
        }
    }
    None
}

fn backticks_end(bytes: &[u8], open: usize) -> Option<usize> {
    let close = open + 1 + bytes[open + 1..].iter().position(|byte| *byte == b'`')?;
    (!bytes[open + 1..close]
        .iter()
        .any(|byte| b"\\'\"".contains(byte)))
    .then_some(close + 1)
}

/// The index after a `$` expansion; a command substitution is skipped whole and read nowhere.
fn dollar_end(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at + 1) {
        Some(b'(') if bytes.get(at + 2) == Some(&b'(') => None,
        Some(b'(') => {
            let mut depth = 0usize;
            let mut index = at + 2;
            while index < bytes.len() {
                match bytes[index] {
                    b'\'' => index = single_quoted_end(bytes, index)?,
                    b'"' => index = double_quoted_end(bytes, index)?,
                    b'`' => index = backticks_end(bytes, index)?,
                    b'$' => index = dollar_end(bytes, index)?,
                    b'#' if is_blank(bytes[index - 1]) => return None,
                    b'\\' | b'\r' => return None,
                    b'<' if bytes.get(index + 1) == Some(&b'<') => return None,
                    b'(' => {
                        depth += 1;
                        index += 1;
                    }
                    b')' if depth == 0 => return Some(index + 1),
                    b')' => {
                        depth -= 1;
                        index += 1;
                    }
                    _ => index += 1,
                }
            }
            None
        }
        Some(b'{' | b'\'' | b'"') => None,
        _ => Some(at + 1),
    }
}

/// The byte ranges of `command` a single simple command reads rather than runs,
/// or `None` when the line is not in the grammar `docs/honesty.md` records.
///
/// It reads the line as bash does, case included: a here-document terminator
/// is compared byte for byte, so a caller must not hand it a case-folded copy.
///
/// `piece` reads one command of a compound line: a plain `$name` expansion is accepted, quoted or
/// not, and a here-document or a line break is refused.
fn read_only_spans(command: &str, piece: bool) -> Option<Vec<(usize, usize)>> {
    if command.contains('\r') {
        return None;
    }
    let bytes = command.as_bytes();
    let (head, mut index) = bare_word(command, 0)?;
    if !READS_ONLY.contains(&head) {
        return None;
    }
    if head == "gh" {
        let (subcommand, next) = bare_word(command, index)?;
        if !GH_READS.contains(&subcommand) {
            return None;
        }
        index = next;
        let verb = index
            + bytes[index..]
                .iter()
                .take_while(|byte| is_blank(**byte))
                .count();
        if subcommand != "api" && matches!(bytes.get(verb), Some(b'\'' | b'"')) {
            return None;
        }
    }
    let mut spans = Vec::new();
    let mut pending: Vec<(String, bool, bool)> = Vec::new();
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b' ' | b'\t' => index += 1,
            b'\'' | b'"' => {
                if !is_blank(bytes[index - 1])
                    || bytes[..index].iter().rev().find(|byte| !is_blank(**byte)) == Some(&b'>')
                {
                    return None;
                }
                let end = if piece {
                    piece_quoted_end(bytes, index)?
                } else {
                    quoted_end(bytes, index)?
                };
                if bytes.get(end + 1).is_some_and(|next| !is_blank(*next)) {
                    return None;
                }
                spans.push((index, end + 1));
                index = end + 1;
            }
            b'$' if piece && bytes.get(index + 1).is_some_and(|next| is_name_byte(*next)) => {
                index += 1;
            }
            b'<' | b'\n' if piece => return None,
            b'<' => {
                if !is_blank(bytes[index - 1]) {
                    return None;
                }
                let (delimiter, strip_tabs, quoted, next) = heredoc_word(command, index)?;
                pending.push((delimiter, strip_tabs, quoted));
                index = next;
            }
            b'\n' => {
                if pending.is_empty() {
                    return None;
                }
                index += 1;
                for (delimiter, strip_tabs, quoted) in pending.drain(..) {
                    index =
                        here_document(command, index, &delimiter, strip_tabs, quoted, &mut spans)?;
                }
                return (index == bytes.len()).then_some(spans);
            }
            _ if byte.is_ascii_alphanumeric() || SAFE_BYTES.contains(&byte) => index += 1,
            _ => return None,
        }
    }
    pending.is_empty().then_some(spans)
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// [`quoted_end`] for one command of a compound line: a double-quoted region may also hold a
/// plain `$name`, and nothing else the shell would expand.
fn piece_quoted_end(bytes: &[u8], open: usize) -> Option<usize> {
    if bytes[open] == b'\'' {
        return quoted_end(bytes, open);
    }
    let end = open + 1 + bytes[open + 1..].iter().position(|byte| *byte == b'"')?;
    let inside = &bytes[open + 1..end];
    let plain = inside.iter().enumerate().all(|(at, byte)| match byte {
        b'`' | b'\\' | b'!' => false,
        b'$' => inside.get(at + 1).is_some_and(|next| is_name_byte(*next)),
        _ => true,
    });
    plain.then_some(end)
}

fn is_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n')
}

/// The `[a-z]+` word after any spaces and tabs from `from`, and where it ends,
/// when a blank or the end of the line follows it.
fn bare_word(command: &str, from: usize) -> Option<(&str, usize)> {
    let bytes = command.as_bytes();
    let start = from
        + bytes[from..]
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
    let end = start
        + bytes[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_lowercase())
            .count();
    (end > start && bytes.get(end).is_none_or(|byte| is_blank(*byte)))
        .then(|| (&command[start..end], end))
}

/// The index of the quote closing the one at `open`, or `None` when it never
/// closes or a double-quoted region holds a byte the shell would expand.
fn quoted_end(bytes: &[u8], open: usize) -> Option<usize> {
    let quote = bytes[open];
    let end = open + 1 + bytes[open + 1..].iter().position(|byte| *byte == quote)?;
    (quote == b'\''
        || !bytes[open + 1..end]
            .iter()
            .any(|byte| b"$`\\!".contains(byte)))
    .then_some(end)
}

/// The here-document operator at `at`: the delimiter, whether `<<-` strips
/// leading tabs, whether the delimiter was quoted, and the index after it.
fn heredoc_word(command: &str, at: usize) -> Option<(String, bool, bool, usize)> {
    let bytes = command.as_bytes();
    if !command[at..].starts_with("<<") {
        return None;
    }
    let mut at = at + 2;
    let strip_tabs = bytes.get(at) == Some(&b'-');
    at += usize::from(strip_tabs);
    while matches!(bytes.get(at), Some(b' ' | b'\t')) {
        at += 1;
    }
    let quote = bytes
        .get(at)
        .copied()
        .filter(|byte| matches!(byte, b'\'' | b'"' | b'\\'));
    let start = at + usize::from(quote.is_some());
    let mut end = start;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    if end == start {
        return None;
    }
    let mut next = end;
    if matches!(quote, Some(b'\'' | b'"')) {
        if bytes.get(end) != quote.as_ref() {
            return None;
        }
        next += 1;
    }
    bytes.get(next).is_none_or(|byte| is_blank(*byte)).then(|| {
        (
            command[start..end].to_owned(),
            strip_tabs,
            quote.is_some(),
            next,
        )
    })
}

/// Consume one here-document body, recording its data lines and returning the
/// index after its terminator.
fn here_document(
    command: &str,
    mut index: usize,
    delimiter: &str,
    strip_tabs: bool,
    quoted: bool,
    spans: &mut Vec<(usize, usize)>,
) -> Option<usize> {
    let bytes = command.as_bytes();
    loop {
        let end = bytes[index..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|at| index + at)
            .unwrap_or(bytes.len());
        let line = &command[index..end];
        let candidate = if strip_tabs {
            line.trim_start_matches('\t')
        } else {
            line
        };
        if candidate == delimiter {
            return Some((end + 1).min(bytes.len()));
        }
        if !quoted && line.contains(['$', '`', '\\']) {
            return None;
        }
        spans.push((index, end));
        if end == bytes.len() {
            return None;
        }
        index = end + 1;
    }
}

/// `command` with each half-open span replaced by one [`DATA`].
fn blank(command: &str, spans: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(command.len());
    let mut at = 0;
    for &(start, end) in spans {
        out.push_str(&command[at..start]);
        out.push(DATA);
        at = end;
    }
    out.push_str(&command[at..]);
    out
}

#[cfg(test)]
mod tests;
