/// Every verb on the list has a command line somebody wrote, and the other way.
///
/// An entry nothing crosses is one that can be dropped, renamed or misspelled
/// in silence. The lines are **written out** rather than built from the
/// entries: a loop that formats `"{verb} src/x.rs"` renames the question with
/// the answer, while written out, renaming an entry leaves a line nothing
/// matches and adding one leaves the count short.
#[test]
fn every_verb_that_writes_has_a_line_that_says_so() {
    const WRITING: &[&str] = &[
        "cp src/x.rs",
        "mv src/x.rs",
        "rm src/x.rs",
        "rmdir src/x.rs",
        "tee src/x.rs",
        "dd of=src/x.rs",
        "truncate src/x.rs",
        "touch src/x.rs",
        "mkdir src/x.rs",
        "ln src/x.rs",
        "install src/x.rs",
        "patch src/x.rs",
        "unzip src/x.rs",
        "tar src/x.rs",
        "zip src/x.rs",
        "gzip src/x.rs",
        "gunzip src/x.rs",
        "bzip2 src/x.rs",
        "bunzip2 src/x.rs",
        "xz src/x.rs",
        "unxz src/x.rs",
        "zstd src/x.rs",
        "unzstd src/x.rs",
        "7z src/x.rs",
        "7za src/x.rs",
        "compress-archive src/x.rs",
        "expand-archive src/x.rs",
        "rsync src/x.rs",
        "scp src/x.rs",
        "shred src/x.rs",
        "split src/x.rs",
        "wget src/x.rs",
        "copy src/x.rs",
        "xcopy src/x.rs",
        "robocopy src/x.rs",
        "move src/x.rs",
        "del src/x.rs",
        "erase src/x.rs",
        "ren src/x.rs",
        "rename src/x.rs",
        "md src/x.rs",
        "rd src/x.rs",
        "set-content src/x.rs",
        "add-content src/x.rs",
        "clear-content src/x.rs",
        "out-file src/x.rs",
        "new-item src/x.rs",
        "remove-item src/x.rs",
        "copy-item src/x.rs",
        "move-item src/x.rs",
        "rename-item src/x.rs",
    ];
    const WRAPPED: &[&str] = &[
        "sudo rm src/x.rs",
        "doas rm src/x.rs",
        "runuser rm src/x.rs",
        "chroot rm src/x.rs",
        "command rm src/x.rs",
        "busybox rm src/x.rs",
        "env rm src/x.rs",
        "nice rm src/x.rs",
        "ionice rm src/x.rs",
        "taskset rm src/x.rs",
        "chrt rm src/x.rs",
        "setarch rm src/x.rs",
        "eatmydata rm src/x.rs",
        "stdbuf rm src/x.rs",
        "unbuffer rm src/x.rs",
        "nohup rm src/x.rs",
        "setsid rm src/x.rs",
        "time rm src/x.rs",
        "timeout rm src/x.rs",
        "flock rm src/x.rs",
        "watch rm src/x.rs",
        "systemd-run rm src/x.rs",
        "xargs rm src/x.rs",
        "parallel rm src/x.rs",
        "strace rm src/x.rs",
        "ltrace rm src/x.rs",
        "script /dev/null rm src/x.rs",
        "xvfb-run rm src/x.rs",
        "proxychains rm src/x.rs",
        "torify rm src/x.rs",
    ];

    // Neither side is allowed to grow without the other.
    assert_eq!(
        WRITING.len(),
        super::WRITES_A_FILE.len(),
        "a verb was added to the list and no line was written for it"
    );
    assert_eq!(
        WRAPPED.len(),
        super::RUNS_ANOTHER_COMMAND.len(),
        "a wrapper was added to the list and no line was written for it"
    );

    for line in WRITING {
        assert!(
            super::writes_a_file(line).is_some(),
            "`{line}` visibly writes a file and was not read as one"
        );
    }
    for line in WRAPPED {
        assert!(
            super::writes_a_file(line).is_some_and(|spelling| spelling.starts_with("rm under ")),
            "`{line}` hides a write behind a wrapper and was not read as one"
        );
    }

    // The floor: an ordinary read is still nothing, or the loops above would
    // pass against a function that answers yes to every line.
    for read in ["ls -la", "grep -r estigia src", "cargo test"] {
        assert_eq!(
            super::writes_a_file(read),
            None,
            "`{read}` writes nothing and was read as a write"
        );
    }
}

#[test]
fn quoted_command_wrappers_are_read_through_their_code_flag() {
    for wrapping in ["su -c 'rm -rf src'", "script -c 'rm -rf src' /dev/null"] {
        assert!(
            super::writes_a_file(wrapping).is_some(),
            "`{wrapping}` runs its quoted command and was not read as a write"
        );
    }
    for bare in ["su", "su bob", "script", "script /dev/null"] {
        assert_eq!(
            super::writes_a_file(bare),
            None,
            "`{bare}` runs no quoted command and was read as a write"
        );
    }
}

/// darwin's `script` has no `-c`, and its command is the trailing words.
///
/// `man script` gives `script [-aeFkqr] [-t time] [file [command ...]]`, and
/// the manual says a command cannot be given without the file. Measured on
/// 2026-09-27: `script /dev/null echo …` and `script -q /dev/null echo …` both
/// printed the command's output. The code-flag arm reads only `script -c '…'`
/// — util-linux's spelling, an illegal option on darwin — so
/// `script /dev/null rm -rf src` ran the deletion and classified `Untouched`
/// on darwin.
///
/// `script` therefore joined `RUNS_ANOTHER_COMMAND` as well as `INLINE_CODE`:
/// the wrapper reading takes any later word naming a write, so it never has to
/// decide which word is the typescript file — the same refusal to model
/// per-wrapper flag arguments the list beside it states. The class is exact:
/// a read naming **no** write stays `None`, and a read whose typescript file
/// names a write is `Some`, at the one-tracker-read cost this module always
/// picks. `a_write_named_typescript_file_is_part_of_the_accepted_surface`
/// pins that case.
#[test]
fn a_bsd_scripts_trailing_command_is_read_as_a_write() {
    for writing in [
        "script /dev/null rm -rf src",
        "script -q /dev/null rm -rf src",
        "script -a out.log tee src/x.rs",
    ] {
        assert!(
            super::writes_a_file(writing).is_some(),
            "`{writing}` runs the words after its typescript file and was not read as a write"
        );
    }

    // The decision, not only the reader underneath it: `Some` here is worth
    // nothing if the classifier above still answers `Untouched`.
    let (action, _) = crate::harness::classify(
        "Bash",
        &serde_json::json!({ "command": "script /dev/null rm -rf src" }),
    );
    assert_ne!(
        action,
        crate::harness::Action::Untouched,
        "the gate decided BSD script's trailing command was none of its business"
    );

    // The quoted spelling covered earlier keeps its answer, and a recording
    // session whose command only reads is still not a write.
    assert!(
        super::writes_a_file("script -c 'rm -rf src' /dev/null").is_some(),
        "the quoted spelling that joined `INLINE_CODE` stopped being read"
    );
    for quiet in [
        "script -q /dev/null",
        "script out.log",
        "script -q /dev/null echo hi",
        "script -q /dev/null ls -la",
    ] {
        assert_eq!(
            super::writes_a_file(quiet),
            None,
            "`{quiet}` runs nothing that writes and was read as a write"
        );
    }
}

/// A write-named typescript file is part of the accepted surface.
///
/// `script -c 'echo hi' rm` runs `echo hi` and records it to a typescript file
/// literally named `rm`. The wrapper arm takes any later word naming a write
/// rather than deciding which word is the typescript file, so this reads as a
/// write — one tracker read, the direction this module always picks. The
/// decision is the carve-out, not a narrower reader: the quoted spelling's
/// criterion admits it, because the file name is on the command line and the
/// asymmetry pays for the read.
///
/// The quoted redirect is the other half: `script -c 'echo hi > out.txt'
/// /dev/null` writes only inside the quoted command, which the wrapper arm
/// cannot see, so the `INLINE_CODE`/code-flag arm is what reads it. A test that
/// pinned only the wrapper arm would stay green with that arm turned off.
#[test]
fn a_write_named_typescript_file_is_part_of_the_accepted_surface() {
    for writing in [
        "script -c 'echo hi' rm",
        "script -c 'echo hi' rm src/x.rs",
        "script rm",
    ] {
        assert!(
            super::writes_a_file(writing).is_some(),
            "`{writing}` names a write as its typescript file and was not read as a write"
        );
    }

    // The code-flag arm's own spellings: the redirect is inside the quotes, so
    // nothing but `INLINE_CODE` plus `-c` can see it.
    for quoted in [
        "script -c 'echo hi > out.txt' /dev/null",
        "script -c 'rm -rf src' /dev/null",
    ] {
        assert!(
            super::writes_a_file(quoted).is_some(),
            "`{quoted}` runs quoted code and was not read as a write"
        );
    }
}

use super::*;

/// Every spelling that visibly puts bytes on disk is read as a write.
///
/// The second half matters as much: the declaration excludes programs that
/// write as a *side effect* — `make`, `cargo build`, `npm install` — by name,
/// and widening the list until those match would destroy the thing this test
/// guards.
#[test]
fn every_spelling_that_visibly_writes_is_read_as_one_and_nothing_else_is() {
    // Downloads, copies, archives, destruction, and the two that write only
    // when the line names where.
    const WRITES: &[&str] = &[
        "curl -o out.txt https://example.com/x",
        "curl -O https://example.com/x.tar.gz",
        "curl --output out.txt https://example.com/x",
        "curl -fsSLo src/main.rs https://example.com/x",
        "wget https://example.com/x.tar.gz",
        "wget -O src/main.rs https://example.com/x",
        "rsync -a src/ dst/",
        "scp file.txt host:/tmp/",
        "gzip big.log",
        "gunzip big.log.gz",
        "bzip2 big.log",
        "xz big.log",
        "zip -r out.zip src",
        "unzip x.zip",
        "tar -xzf x.tar.gz",
        "7z a out.7z src",
        "shred -u secret.key",
        "split -b 1M big.bin part-",
        "openssl req -new -out cert.csr",
        // Already read before this round, here so the corpus proves the whole
        // population and not only what was added to it.
        "cp a b",
        "rm -rf build",
        "echo hola > src/main.rs",
        "tee out.txt",
        "sed -i s/a/b/ file.txt",
        // And under a wrapper, because the list next door is what reads that
        // and the two have to keep agreeing.
        "sudo gzip big.log",
        "xargs shred",
    ];

    // Reads, and the side-effect writers the declaration excludes on purpose.
    // A guard that fired on these is one people switch off, which is the
    // sentence the module opens with.
    const UNTOUCHED: &[&str] = &[
        "ls -la",
        "cat file.txt",
        "grep -r foo src",
        "wc -l file.txt",
        "head -20 file.txt",
        "diff a b",
        "ps aux",
        "curl https://example.com/x",
        "curl -sS https://example.com/api",
        "openssl dgst -sha256 file.txt",
        "make install",
        "cargo build",
        "npm install",
        "pip install requests",
    ];

    let missed: Vec<&&str> = WRITES
        .iter()
        .filter(|command| writes_a_file(command).is_none())
        .collect();
    assert!(
        missed.is_empty(),
        "{} command line(s) visibly write a file and go through untouched: {missed:#?}",
        missed.len()
    );

    let overreached: Vec<String> = UNTOUCHED
        .iter()
        .filter_map(|command| {
            writes_a_file(command).map(|why| format!("{command:?} read as {why:?}"))
        })
        .collect();
    assert!(
        overreached.is_empty(),
        "{} command line(s) that write nothing, or write only as a side effect, are now gated \
         \u{2014} which is the boundary the declaration draws and the reason it draws it: \
         {overreached:#?}",
        overreached.len()
    );
}

#[test]
fn a_redirect_is_a_write_however_it_is_spelled() {
    for command in [
        "echo fn main() {} > src/main.rs",
        "cat >src/lib.rs",
        "printf 'x' >> notes.md",
        "cargo tree &> deps.txt",
        "python gen.py 2> errors.log",
    ] {
        assert!(
            writes_a_file(command).is_some(),
            "`{command}` writes a file and was not read as one"
        );
    }
}

#[test]
fn a_greater_than_that_opens_nothing_is_not_a_write() {
    // The three ways a `>` appears without a file behind it. Each of these
    // reaching the gate would cost a tracker read on `cargo test`, which is how
    // a guard earns its way off somebody's machine.
    for command in [
        r#"echo "a > b""#,
        "echo 'x -> y'",
        "cargo test 2>&1",
        "exec 3>&-",
        "grep -c foo file.txt",
    ] {
        assert_eq!(
            writes_a_file(command),
            None,
            "`{command}` opens no file and was read as a write"
        );
    }
}

#[test]
fn a_redirect_after_a_quote_the_scan_could_misread_is_still_a_write() {
    for command in [
        "echo \"\\\"\" > src/lib.rs",
        "echo $'\\'' > src/x",
        "true # it's\necho x > src/lib.rs",
        "cat <<EOF\nit\"s\nEOF\necho x > src/lib.rs",
        "cat <<'EOF'\n\"\nEOF\necho x > src/lib.rs",
        "cat <<-EOF\n\t'\n\tEOF\necho x > src/lib.rs",
        "echo x # > y",
        "echo \"unterminated > src/x",
    ] {
        assert!(
            writes_a_file(command).is_some(),
            "`{command}` writes a file and was not read as one"
        );
    }
}

#[test]
fn a_quoted_arrow_stays_data_whichever_quote_holds_it() {
    for command in [
        "echo \"a \\\" > b\"",
        "echo $'a \\' > b'",
        "echo 'a > b'",
        "cat <<EOF\na > b\nEOF",
        "cat <<'EOF'\n\"\n> b\nEOF",
        "grep x src 2>/dev/null",
        "cmd 2>&1",
        "echo a#b",
    ] {
        assert_eq!(
            writes_a_file(command),
            None,
            "`{command}` opens no file and was read as a write"
        );
    }
}

#[test]
fn the_redirect_scan_reads_the_character_after_the_arrow() {
    // Written against surviving mutants rather than against intuition: `cargo
    // mutants --file src/harness/shell.rs` left eight alive, all of them here,
    // and each one below kills a specific survivor. A guard nothing distinguishes
    // is a guard that passes for the wrong reason.

    // Killing `&& → ||` in the descriptor check. A filename may start with a
    // digit or a dash, and only the `&` in front of it means "duplicate a
    // descriptor". Read either half alone and `> 1.log` stops being a write.
    assert!(writes_a_file("echo hi > 1.log").is_some());
    assert!(writes_a_file("echo hi > -weird.txt").is_some());

    // Killing the deletion of the backslash arm. An escaped `>` is a literal
    // character, so nothing is opened; drop the arm and this reads as a write.
    assert_eq!(writes_a_file(r"echo a\> b"), None);

    // The same arm, killed at the one offset where `index += 2` and `index *= 2`
    // part company usefully: the backslash at position 1, the character it hides
    // at position 2. Double instead of advance and the scan lands *on* the `>`
    // it was told to skip.
    assert_eq!(writes_a_file(r"a\>b"), None);

    // Killing the index arithmetic at the front of the line, where `index + 1`
    // and `index * 1` are the same number everywhere except position zero.
    assert!(writes_a_file(">out.txt").is_some());
    assert!(writes_a_file(">>out.txt").is_some());

    // Killing `after + 2` into `after * 2`, where the scan resumes after a
    // duplicated descriptor. Only a redirect that comes *after* a `2>&1` can
    // tell the two apart, and that is an ordinary thing to write.
    assert!(writes_a_file("cargo test 2>&1 > out.log").is_some());
}

#[test]
fn an_interpreter_handed_code_is_a_write_and_one_handed_a_script_is_not() {
    // The boundary the declaration draws, held from both sides. `python -c` is
    // opaque and unusual; `python manage.py` is opaque and ordinary, and gating
    // it is gating every build.
    assert!(writes_a_file(r#"python -c "open('x','w').write('y')""#).is_some());
    assert!(writes_a_file("node -e 'require(\"fs\").writeFileSync(\"a\",\"b\")'").is_some());
    assert!(writes_a_file("bash -c 'echo hi > f'").is_some());

    assert_eq!(writes_a_file("python manage.py migrate"), None);
    assert_eq!(writes_a_file("node build.js"), None);
    assert_eq!(writes_a_file("cargo build --release"), None);
    assert_eq!(writes_a_file("make test"), None);
}

#[test]
fn an_in_place_edit_is_a_write_and_the_same_filter_streaming_is_not() {
    assert!(writes_a_file("sed -i s/a/b/ src/main.rs").is_some());
    assert!(writes_a_file("sed -i.bak s/a/b/ src/main.rs").is_some());
    assert!(writes_a_file("perl -i -pe s/a/b/ x").is_some());

    // Without `-i` the same program is a filter, and filtering is reading.
    assert_eq!(writes_a_file("sed s/a/b/ src/main.rs"), None);
    assert_eq!(writes_a_file("perl -pe s/a/b/ x"), None);
}

#[test]
fn a_write_after_a_separator_is_still_a_write() {
    // The head of the *first* command is a read in each of these. A reader that
    // only looked at the front would let all four through.
    for command in [
        "cd src && rm main.rs",
        "cargo build; cp target/x /usr/local/bin/x",
        "git status | tee status.txt",
        "true && sed -i s/a/b/ f",
    ] {
        assert!(
            writes_a_file(command).is_some(),
            "`{command}` writes after a separator and was not read as one"
        );
    }
}

#[test]
fn a_writing_command_is_matched_as_a_word_not_as_a_substring() {
    // `rm` sits inside `confirm` and `cp` inside `tcp`. Substring matching —
    // which is what the two lists beside this one use — reads all three of
    // these as writes.
    for command in [
        "echo please confirm the plan",
        "ss -tcp",
        "grep -r movement src/",
    ] {
        assert_eq!(
            writes_a_file(command),
            None,
            "`{command}` was matched as a substring"
        );
    }

    // And the real command still is, including through an absolute path.
    assert_eq!(writes_a_file("rm -rf build"), Some("rm".to_owned()));
    assert_eq!(writes_a_file("/usr/bin/cp a b"), Some("cp".to_owned()));
}

#[test]
fn the_windows_spellings_of_a_write_are_writes_too() {
    // This crate's own platform. `cmd /c "…"` hides its redirect behind quotes
    // exactly the way `python -c` does, and the shell tool of an agent running
    // on Windows hands it the native spellings rather than the POSIX ones.
    for command in [
        r#"cmd /c "echo fn main(){} > src\x.rs""#,
        "cmd /k del src\\x.rs",
        "copy a.rs b.rs",
        "move a.rs b.rs",
        "del src\\x.rs",
        "powershell -Command \"Set-Content src/x.rs 'y'\"",
        "set-content src/x.rs 'y'",
        "out-file -filepath src/x.rs",
        "remove-item src/x.rs",
    ] {
        assert!(
            writes_a_file(&command.to_ascii_lowercase()).is_some(),
            "`{command}` writes a file on Windows and was not read as one"
        );
    }

    // The same command under the three names Windows gives it.
    assert!(writes_a_file(r#"c:\windows\system32\cmd.exe /c "type a > b""#).is_some());
    assert!(writes_a_file("python.exe -c \"open('x','w')\"").is_some());
    assert_eq!(writes_a_file("del.exe x"), Some("del".to_owned()));

    // And the Windows reads stay free, the same as `ls` and `cargo test`.
    for command in [
        "dir /b",
        "type readme.md",
        "get-content src/x.rs",
        "where cargo",
    ] {
        assert_eq!(
            writes_a_file(command),
            None,
            "`{command}` reads and was gated"
        );
    }
}

#[test]
fn no_reader_has_been_added_to_a_writing_list() {
    // The gap in the guard on the guard. `writing-shell` is declared on
    // `writes_a_file`, so its fingerprint covers the function — while the
    // population it describes lives in these constants, which the fingerprint
    // never reaches. Growing a list therefore does *not* reopen the affirmed
    // claim, and the claim would go on reading correctly while the code stopped
    // matching it.
    //
    // Enumerating the right members is not something a test can do. Catching the
    // worst mistake is: a command that only reads has no business in a list that
    // means "this wrote something", and one arriving there gates `ls` — the
    // outcome the whole declaration is written to avoid.
    const READERS: &[&str] = &[
        "ls",
        "cat",
        "head",
        "tail",
        "grep",
        "find",
        "file",
        "stat",
        "diff",
        "wc",
        "echo",
        "dir",
        "type",
        "where",
        "get-content",
        "get-childitem",
        "select-string",
        "cargo",
        "make",
        "npm",
    ];
    for reader in READERS {
        for (list, name) in [
            (WRITES_A_FILE, "WRITES_A_FILE"),
            (INLINE_CODE, "INLINE_CODE"),
            (IN_PLACE, "IN_PLACE"),
        ] {
            assert!(
                !list.contains(reader),
                "`{reader}` reads and is in {name}, so every one of them now costs a tracker read"
            );
        }
        assert_eq!(
            writes_a_file(&format!("{reader} something")),
            None,
            "`{reader}` reads and was classified as a write"
        );
    }
}

#[test]
fn dd_is_a_write_only_when_it_is_told_where_to_put_it() {
    assert!(writes_a_file("dd if=/dev/zero of=disk.img bs=1m").is_some());
    assert_eq!(writes_a_file("dd if=disk.img | sha256sum"), None);
}

#[test]
fn rg_pre_is_a_write_and_a_plain_rg_search_is_not() {
    for command in [
        "rg --pre /bin/rm x",
        "rg --pre=/tmp/s.sh x",
        "rg --pre /bin/rm x > /tmp/out",
    ] {
        assert!(writes_a_file(command).is_some(), "{command}");
    }
    for command in ["rg pattern src", "rg --pre-glob x pattern src"] {
        assert_eq!(writes_a_file(command), None, "{command}");
    }
}

/// Every ordinary way git writes the working tree is one the gate sees.
///
/// `REPOSITORY_SHELL` says the rule this holds it to: *too-tight is a licence
/// to be incomplete, not to omit a member sitting next to one that is here*.
/// Each of these sits next to one that is — a worktree made and a worktree
/// removed, untracked files deleted and tracked ones, a revert beside a
/// cherry-pick, a patch applied-and-committed beside a commit, three reset
/// modes beside one.
#[test]
fn every_ordinary_way_git_writes_the_tree_is_one_the_gate_sees() {
    for command in [
        "git apply patch.diff",
        "git am patch.mbox",
        "git rm -f src/main.rs",
        "git mv a.rs b.rs",
        "git revert HEAD",
        "git reset --keep HEAD~1",
        "git reset --merge HEAD~1",
        "git submodule update --init",
        "git worktree remove ../wt",
        "git filter-branch --force",
    ] {
        let (action, _) =
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]);
        assert!(
            matches!(
                action,
                crate::harness::Action::Write { .. } | crate::harness::Action::Boundary { .. }
            ),
            "`{command}` changes this repository and the gate does not see it: {action:?}"
        );
    }

    // The floor, and it is the reason `git checkout <branch>` is deliberately
    // absent from that list: reading every git command as a write would make
    // the list decide nothing.
    for command in ["git status", "git log --oneline", "git diff HEAD"] {
        let (action, _) =
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]);
        assert!(
            matches!(action, crate::harness::Action::Untouched),
            "`{command}` reads and is being gated as a write: {action:?}"
        );
    }
}

/// The two spellings of command substitution are read the same way.
///
/// `segments` splits on `(` and `)`, so `echo $(rm -rf src)` puts `rm -rf src`
/// in a segment of its own and it is read. The backtick spelling of the *same
/// command* has no split point, so the whole line came back as `echo` — which
/// is not a write, and neither is anything else about that line.
///
/// This is not the declared escape. That one is `eval "$payload"`, where the
/// write is genuinely not on the line. Here it is written out in full, in the
/// spelling half the shell scripts in the world use, and the module's own
/// boundary is *what is visible on the command line*. It is a list that stopped
/// halfway rather than a boundary — the same shape as `unzip` being a write
/// while `zip` was not.
///
/// `eval` with a literal goes the same way and for the same reason: it names
/// another command, the command is right there, and every other wrapper that
/// does that is already in `RUNS_ANOTHER_COMMAND`.
#[test]
fn a_write_inside_backticks_is_read_like_one_inside_a_dollar_paren() {
    for (backticks, dollar) in [
        ("echo `cp a.txt b.txt`", "echo $(cp a.txt b.txt)"),
        ("echo `rm -rf src`", "echo $(rm -rf src)"),
        ("out=`tee report.txt`", "out=$(tee report.txt)"),
    ] {
        // The floor: the spelling that already worked still does. A change that
        // stopped reading `$( )` would satisfy the equality below.
        assert!(
            super::writes_a_file(dollar).is_some(),
            "`{dollar}` stopped being read as a write"
        );
        assert!(
            super::writes_a_file(backticks).is_some(),
            "`{backticks}` writes a file and `{dollar}` is read as one"
        );
    }

    // And a wrapper that names its command on the line, like every other one.
    assert!(
        super::writes_a_file("eval 'rm -rf src'").is_some(),
        "`eval` with the command written out is not read, while `sh -c` is"
    );

    // The floor for both: a line with no write on it is still not one.
    //
    // `eval 'git log'` is deliberately **not** here. It is read as a write for
    // the same reason `sh -c 'git log'` already was: an interpreter handed code
    // is read by its own name, because what it was handed is opaque. That is a
    // false positive the module's stated asymmetry buys on purpose — one
    // tracker read, against a guarantee.
    for quiet in ["echo `git status`", "out=$(pwd)", "echo `pwd`"] {
        assert_eq!(
            super::writes_a_file(quiet),
            None,
            "`{quiet}` writes nothing and is being read as a write"
        );
    }
}

/// An option before the subcommand does not hide the subcommand.
///
/// Every fragment in the classifier's git lists names `git <subcommand>`, and
/// git takes its own options *before* that word. So one `-c` between the two
/// defeated all of them at once — measured, and it is not a corner: `git -c
/// core.hooksPath=/dev/null push origin HEAD` is a push **with every hook in
/// the repository turned off**, and it classified `Untouched`. Not a boundary,
/// not a write, nothing.
///
/// `git -c user.name=x merge main` went the same way, and merging is a
/// delivery. So did `commit`, `reset --hard`, `tag`, `clean`, `rm` and
/// `worktree add` behind `-C`, `--git-dir` or `--no-pager`.
///
/// The `git merge` pair is no longer folded into the equality loop below. It
/// used to be, and that equality is exactly the severe finding a
/// second repair round closed: the call site that decides the integration
/// exemption keyed on the fragment `classify_with` matched — "git merge" for
/// both spellings here — rather than on the literal line, so a `-C`- or
/// `-c`-prefixed merge earned the same exemption a bare one does, reading the
/// wrong checkout's `HEAD` while advancing trunk. `is_exact_integration_merge`
/// reads the raw literal, so the two spellings are deliberately read
/// differently now — asserted on its own below, where the difference is the
/// point rather than a regression.
#[test]
fn an_option_before_the_subcommand_does_not_hide_it() {
    let read = |command: &str| {
        crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]).0
    };
    for (spelled, plain) in [
        (
            "git -c core.hooksPath=/dev/null push origin HEAD",
            "git push origin HEAD",
        ),
        ("git -C src commit -m x", "git commit -m x"),
        (
            "git --git-dir=.git --work-tree=. reset --hard",
            "git reset --hard",
        ),
        ("git --no-pager tag v1", "git tag v1"),
        ("git -P clean -fd", "git clean -fd"),
        ("git --literal-pathspecs rm -f a.rs", "git rm -f a.rs"),
        (
            "git -c a=b -C src -c d=e worktree add ../w",
            "git worktree add ../w",
        ),
    ] {
        // The floor: the plain spelling is still read. A change that stopped
        // reading `git push` would satisfy the equality below.
        assert!(
            !matches!(read(plain), crate::harness::Action::Untouched),
            "`{plain}` stopped being read at all"
        );
        assert_eq!(
            read(spelled),
            read(plain),
            "`{spelled}` is the same act as `{plain}` and is read differently"
        );
    }

    // `git merge`, on its own: still one boundary, `DELIVERS` and
    // `out-of-phase` still see both — but the integration exemption must not
    // follow the `-c`-prefixed spelling to a checkout it never proved.
    for command in ["git -c user.name=x merge main", "git merge main"] {
        assert!(
            !matches!(read(command), crate::harness::Action::Untouched),
            "`{command}` stopped being read at all"
        );
    }
    let crate::harness::Action::Boundary {
        command: spelled_command,
        is_exact_integration_merge: spelled_exempt,
        ..
    } = read("git -c user.name=x merge main")
    else {
        panic!("`git -c user.name=x merge main` stopped being a boundary");
    };
    let crate::harness::Action::Boundary {
        command: plain_command,
        is_exact_integration_merge: plain_exempt,
        ..
    } = read("git merge main")
    else {
        panic!("`git merge main` stopped being a boundary");
    };
    assert_eq!(
        (spelled_command.as_str(), plain_command.as_str()),
        ("git merge", "git merge"),
        "the fragment classifier stopped reading one of the two spellings as `git merge`"
    );
    assert!(
        plain_exempt,
        "`git merge main` lost the integration exemption it is meant to carry"
    );
    assert!(
        !spelled_exempt,
        "`git -c user.name=x merge main` earned the integration exemption through a fragment \
         match alone"
    );

    // And the floor for the normalisation itself: a word that is not an option
    // is still the subcommand.
    assert!(
        matches!(
            crate::harness::classify_with(
                "Bash",
                &serde_json::json!({ "command": "git status" }),
                &[]
            )
            .0,
            crate::harness::Action::Untouched
        ),
        "a read is being gated as a write"
    );
}

/// Turning every hook in the repository off is a boundary, not a nothing.
///
/// `git config core.hooksPath` needs no file the shell analyser can see and
/// names no command in any list: one line, and the push guard — *the gate no
/// agent can route around*, in this crate's own words — stops running. It read
/// as `Untouched`.
///
/// It belongs with `estigia stand-down` rather than with a write: what it does
/// is stand the harness down, and a run holding a claim must renew it at that
/// boundary rather than do it quietly halfway through work it swore to.
#[test]
fn turning_the_repositorys_hooks_off_is_a_boundary() {
    for command in [
        "git config core.hooksPath /dev/null",
        "git config --local core.hooksPath nowhere",
        "git config --global core.hooksPath /dev/null",
    ] {
        let (action, sensitivity) =
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]);
        assert!(
            matches!(action, crate::harness::Action::Boundary { .. }),
            "`{command}` stops every hook in this repository and the gate reads it as {action:?}"
        );
        assert_eq!(sensitivity, crate::harness::Sensitivity::Boundary);
    }

    // The floor: an ordinary `git config` is not a disarm.
    assert!(
        matches!(
            crate::harness::classify_with(
                "Bash",
                &serde_json::json!({ "command": "git config user.name Ada" }),
                &[]
            )
            .0,
            crate::harness::Action::Untouched
        ),
        "every `git config` is being read as a disarm"
    );
}

/// A parameter makes `gh api` a write, because gh says so.
///
/// `gh api --help`, in its own words: *the default HTTP request method is `GET`
/// normally and `POST` if any parameters are added*. So the ordinary way to
/// create something through that command names no method at all —
/// `gh api repos/o/r/issues -f title=x` opens an issue, and it read as
/// `Untouched`.
///
/// The sharpest of them is the one the binding itself uses:
/// `gh api graphql -f query='mutation { … }'` is a tracker write with no `-X`
/// anywhere on the line, and the tracker is the thing this whole harness
/// adjudicates against.
///
/// `--input` goes the same way and for the same reason: a body is a parameter.
#[test]
fn a_parameter_makes_gh_api_a_write_because_gh_says_so() {
    for command in [
        "gh api repos/o/r/issues -f title=x",
        "gh api repos/o/r/issues -F body=@note.md",
        "gh api repos/o/r/issues --field title=x",
        "gh api repos/o/r/issues --raw-field title=x",
        "gh api --input body.json repos/o/r/issues",
        "gh api graphql -f query='mutation { closeIssue }'",
    ] {
        let (action, sensitivity) =
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]);
        assert!(
            matches!(action, crate::harness::Action::Boundary { .. }),
            "`{command}` changes the tracker and the gate reads it as {action:?}"
        );
        assert_eq!(sensitivity, crate::harness::Sensitivity::Boundary);
    }

    // The floor: a read through the same command is still a read. Without it a
    // change that called every `gh api` a boundary would pass.
    for command in ["gh api repos/o/r", "gh api /rate_limit"] {
        let (action, _) =
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]);
        assert!(
            matches!(action, crate::harness::Action::Untouched),
            "`{command}` reads and is being gated: {action:?}"
        );
    }
}

/// A redundant separator does not turn the control surface into an ordinary file.
///
/// `rm ~/.claude/settings.json` is a boundary — it ends the gate — and
/// `rm ~/.claude/./settings.json` is the same act. It read as `Routine`, which
/// means it may ride on an answer from inside the renewal window instead of
/// re-reading the timeline: the harness disarmed on a two-minute-old claim.
///
/// The same shape as the option before the subcommand one file over. A matcher
/// that a redundant separator defeats is not measuring a path, it is measuring
/// a spelling.
#[test]
fn a_redundant_separator_does_not_hide_the_control_surface() {
    for (spelled, plain) in [
        ("rm ~/.claude/./settings.json", "rm ~/.claude/settings.json"),
        ("rm ~/.claude//settings.json", "rm ~/.claude/settings.json"),
        ("rm .git/hooks/./pre-push", "rm .git/hooks/pre-push"),
        ("rm -rf ~/.estigia/./runs", "rm -rf ~/.estigia/runs"),
    ] {
        let read = |command: &str| {
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]).1
        };
        // The floor: the plain spelling is still a boundary.
        assert_eq!(
            read(plain),
            crate::harness::Sensitivity::Boundary,
            "`{plain}` stopped being read as a boundary"
        );
        assert_eq!(
            read(spelled),
            read(plain),
            "`{spelled}` is the same file as `{plain}` and is read differently"
        );
    }

    // The floor for the collapsing itself: an ordinary file is still ordinary.
    assert_eq!(
        crate::harness::classify_with(
            "Bash",
            &serde_json::json!({ "command": "rm src/./main.rs" }),
            &[]
        )
        .1,
        crate::harness::Sensitivity::Routine,
        "an ordinary file is being read as the control surface"
    );
}

/// A write reaches the control surface whatever the tool calls its path.
///
/// The gate read `file_path`, `path` and `notebook_path`, which is Claude
/// Code's spelling and Gemini's. Four of the eleven write tools do not use it:
///
/// - Codex's `apply_patch` and OpenCode's `patch` carry the path **inside the
///   patch body**, so there is no field to read at all.
/// - A tool spelling the same key `filePath` was a different key.
///
/// For all four the target fell back to the *tool name*, which is not a path
/// and is never the control surface — so `~/.claude/settings.json` could be
/// rewritten through them and the gate answered `Routine`: it may ride on an
/// answer from inside the renewal window. Ending the gate on a two-minute-old
/// claim.
///
/// Two rules, and neither guesses a field name. A key that differs only in case
/// or separators is the same key, which is what `Event::from_slug` already does
/// for event names. And when no key names a path, the payload itself is read —
/// a patch body naming the control surface is the control surface. That last
/// one costs a false positive on a write whose *content* mentions one of those
/// paths, which is one tracker read, in the direction this module always picks.
#[test]
fn a_write_reaches_the_control_surface_whatever_the_tool_calls_its_path() {
    let surface = "/home/me/.claude/settings.json";
    for (tool, input) in [
        ("edit", serde_json::json!({ "filePath": surface })),
        (
            "notebookedit",
            serde_json::json!({ "notebookPath": surface }),
        ),
        (
            "apply_patch",
            serde_json::json!({
                "input": format!("*** Begin Patch\n*** Update File: {surface}\n*** End Patch")
            }),
        ),
        (
            "patch",
            serde_json::json!({ "content": format!("--- a{surface}\n+++ b{surface}\n") }),
        ),
        ("pre_write_code", serde_json::json!({ "file": surface })),
    ] {
        let (action, sensitivity) = crate::harness::classify_with(tool, &input, &[]);
        assert!(
            matches!(action, crate::harness::Action::Write { .. }),
            "`{tool}` stopped being read as a write at all: {action:?}"
        );
        assert_eq!(
            sensitivity,
            crate::harness::Sensitivity::Boundary,
            "`{tool}` can end the gate and it is read as an ordinary write"
        );
    }

    // The floor: an ordinary write is still ordinary, whichever key it uses.
    // Without it, reading every payload as the control surface would pass.
    for (tool, input) in [
        ("write", serde_json::json!({ "file_path": "src/main.rs" })),
        ("edit", serde_json::json!({ "filePath": "src/main.rs" })),
        (
            "apply_patch",
            serde_json::json!({ "input": "*** Update File: src/main.rs" }),
        ),
    ] {
        assert_eq!(
            crate::harness::classify_with(tool, &input, &[]).1,
            crate::harness::Sensitivity::Routine,
            "`{tool}` writing an ordinary file is being read as the control surface"
        );
    }
}

/// A shell call is read whatever shape its command arrives in.
///
/// The gate read `input["command"]` as a **string**. Codex's `shell` tool sends
/// it as the argv array — `["bash", "-lc", "git push origin HEAD"]` — so
/// `as_str()` answered nothing, the command was the empty string, and **every
/// shell call from that agent classified `Untouched`**. A push, a merge, a
/// `reset --hard`: all of them, from an agent this build installs a gate into
/// and reports as `gate on`.
///
/// The two rules are yesterday's, one field over: a key differing only in case
/// or separators is the same key, and when no key names the command the payload
/// itself is read. What is new is the *shape* — an argv array is a command line
/// with the spaces taken out.
#[test]
fn a_shell_call_is_read_whatever_shape_its_command_arrives_in() {
    for (tool, input) in [
        (
            "shell",
            serde_json::json!({ "command": ["bash", "-lc", "git push origin HEAD"] }),
        ),
        (
            "shell",
            serde_json::json!({ "command": ["git", "push", "origin", "HEAD"] }),
        ),
        (
            "bash",
            serde_json::json!({ "commandLine": "git push origin HEAD" }),
        ),
        (
            "pre_run_command",
            serde_json::json!({ "command_line": "git push origin HEAD" }),
        ),
        ("bash", serde_json::json!({ "cmd": "git push origin HEAD" })),
    ] {
        let (action, _) = crate::harness::classify_with(tool, &input, &[]);
        assert_eq!(
            action,
            crate::harness::Action::Boundary {
                command: "git push".to_owned(),
                pr: None,
                pr_unidentified_reason: None,
                local_fast_forward_target: None,
                is_exact_integration_merge: false,
                runs_in: crate::harness::RunsIn::Session,
                target: crate::harness::CommitTarget::Checkout,
            },
            "`{tool}` was handed a push in {input} and the gate saw nothing"
        );
    }

    // The floor: a read is still a read in every one of those shapes, so this
    // is not a change that gates every shell call.
    for input in [
        serde_json::json!({ "command": ["git", "status"] }),
        serde_json::json!({ "commandLine": "git status" }),
        serde_json::json!({ "command": "git status" }),
    ] {
        assert_eq!(
            crate::harness::classify_with("bash", &input, &[]).0,
            crate::harness::Action::Untouched,
            "a read is being gated: {input}"
        );
    }
}

#[test]
fn powershell_hides_code_behind_a_switch_it_lets_you_abbreviate() {
    // The table of "the code is right here" flags is matched by exact spelling,
    // and PowerShell does not have exact spellings: it resolves a parameter name
    // from **any prefix that is not ambiguous**, so `-EncodedCommand` is also
    // `-enc`, `-ec` and `-en`, and `-Command` is also `-comm` and `-Comman`.
    //
    // Measured against real `powershell.exe` on this machine — every one of
    // these printed its argument's output:
    //
    // ```
    // powershell -nop -enc <base64>   → ABREVIADO
    // powershell -nop -ec  <base64>   → ABREVIADO
    // powershell -nop -comm 'echo …'  → ABREVIADO
    // powershell -nop -Comman 'echo …'→ ABREVIADO
    // ```
    //
    // And the table held `-command` and `-encodedcommand`: the two spellings
    // nobody types. `-enc` is the canonical one, in every script that has ever
    // been written. So the gate read the whole family as an opaque argument to a
    // program that writes nothing, and a `git push` inside it went through
    // unclassified — on Windows.
    //
    // Matching by prefix rather than by a longer list is also the only version
    // that stays true: a list of spellings for a shell with N-letter
    // abbreviations is a list that is always one spelling short.
    for hidden in [
        "powershell -nop -enc ZwBpAHQA",
        "powershell -nop -ec ZwBpAHQA",
        "powershell -comm \"git push\"",
        "powershell -Comman \"git push\"",
        "pwsh -Comm \"git push\"",
        "pwsh -EnC ZwBpAHQA",
    ] {
        assert!(
            super::writes_a_file(hidden).is_some(),
            "code on the command line was read as an ordinary argument: {hidden}"
        );
    }

    // The floor: the two spellings that always worked still do, so a fix that
    // simply answered `Some` to everything would not pass here.
    for known in [
        "powershell -Command \"git push\"",
        "powershell -EncodedCommand ZwBpAHQA",
    ] {
        assert!(
            super::writes_a_file(known).is_some(),
            "the spelling that already worked stopped: {known}"
        );
    }

    // And the decision, not only the reader underneath it. `writes_a_file`
    // answering `Some` is worth nothing if the classifier above it still lets
    // the call past, and this is the level a gate is actually judged at.
    for hidden in [
        "powershell -nop -enc ZwBpAHQAIABwAHUAcwBoAA==",
        "powershell -comm \"git push\"",
    ] {
        let (action, _) =
            crate::harness::classify("Bash", &serde_json::json!({ "command": hidden }));
        assert_ne!(
            action,
            crate::harness::Action::Untouched,
            "the gate decided this was none of its business: {hidden}"
        );
    }

    // And a prefix rule that matched anything would swallow the switches that
    // carry no code. `-NoProfile`, `-ExecutionPolicy` and `-File` are on every
    // one of the hook scripts Estigia itself writes: reading those as inline
    // code would make the gate ask about its own launcher.
    for plain in [
        "powershell -NoProfile -File C:/hooks/PreToolUse.ps1",
        "powershell -nop -w hidden -ExecutionPolicy Bypass -File run.ps1",
        "pwsh -NoLogo -Version",
    ] {
        assert_eq!(
            super::writes_a_file(plain),
            None,
            "a switch that carries no code was read as if it did: {plain}"
        );
    }
}

#[test]
fn a_redirect_to_a_stream_is_not_a_write_and_any_other_target_is() {
    for read in [
        "grep -rn \"itp\" ../../api ../../src 2>/dev/null | head",
        "ls .github/ISSUE_TEMPLATE 2>/dev/null",
        "ls > /dev/null",
        "ls >/dev/null 2>&1",
        "ls &> /dev/null",
        "ls >> /dev/null",
        "ls > \"/dev/null\"",
        "ls 2> '/dev/stderr'",
        "ls > /dev/fd/2",
        "ls > /dev/null; ls",
    ] {
        assert_eq!(
            super::writes_a_file(read),
            None,
            "`{read}` was read as a write"
        );
    }
    for write in [
        "ls > out.txt",
        "ls 2> err.log",
        "ls > /dev/nullx",
        "ls > /dev/null/x",
        "ls > /dev/fd/",
        "ls > /dev/fd/x",
        "ls > \"/dev/null\"x",
        "ls > /dev/null > out.txt",
        "ls >",
        "ls > \"/dev/null",
    ] {
        assert!(
            super::writes_a_file(write).is_some(),
            "`{write}` was not read as a write"
        );
    }
}

#[test]
fn a_read_only_line_has_its_data_blanked() {
    for (command, fragment) in [
        ("echo 'git push'", "git push"),
        ("echo \"git push\"", "git push"),
        ("cat <<EOF\nestigia stand-down\nEOF", "estigia stand-down"),
        (
            "gh issue view 61 --json comments --jq '.comments[] | split(\"issue-flow: \")[1]'",
            "split",
        ),
        ("grep 'git tag v1' f", "git tag"),
    ] {
        let view = super::data_view(command);
        assert!(
            !view.contains(fragment),
            "`{command}` kept `{fragment}` in `{view}`"
        );
    }
}

#[test]
fn a_line_an_executor_owns_is_left_whole() {
    for command in [
        "echo 'git push' | sh",
        "printf 'git push' | bash",
        "sh <<EOF\ngit push\nEOF",
        "bash <<'EOF'\ngit push\nEOF",
        "bash <<< 'git push'",
        "bash -lc 'git push'",
        "dash -c 'git push'",
        "eval 'git push'",
        "ssh host 'git push'",
        "sh -c 'estigia stand-down'",
        "python -c 'import os; os.system(\"git push\")'",
        "git config \"core.hooksPath\" /dev/null",
        "cat <<EOF\n$(git push)\nEOF",
        "echo \"$(git push)\"",
        "echo `git push`",
        "echo 'git push'\ngit push",
        "# <<EOF\ngit push",
        "rg --pre 'git push' f",
        "sort --compress-program 'git push' f",
        "gh codespace ssh -c box -- 'git push'",
        "gh cs ssh -c box -- 'git push'",
        "gh extension exec 'git push'",
        "gh 'codespace' 'ssh' -c box -- 'git push'",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as data and left unread"
        );
    }
}

#[test]
fn an_arithmetic_shift_or_a_comment_opens_no_here_document() {
    for command in [
        "echo $((1 <<2))\ngit push",
        "echo hi # <<EOF\nrm -rf src\nEOF",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as data through a `<<` that opens nothing"
        );
    }
}

#[test]
fn a_quoted_here_document_delimiter_makes_its_body_data() {
    let command = "cat <<'EOF'\n$(git push)\nEOF";
    assert!(
        !super::data_view(command).contains("git push"),
        "a quoted delimiter's body is literal and was read as code"
    );
}

#[test]
fn a_quoted_listing_flag_survives_for_the_flag_reader() {
    for listing in ["git tag '--list'", "git tag '-l'", "git tag \"--list\""] {
        assert_eq!(
            super::data_view(listing),
            listing,
            "`{listing}` was masked and lost the flag the listing reader needs"
        );
    }
}

#[test]
fn a_command_after_a_here_document_terminator_is_not_read_as_data() {
    for command in [
        "cat <<EOF\nhello\nEOF\nsh -c 'git push origin main'",
        "cat <<EOF\nhello\nEOF\n\"estigia\" stand-down",
        "cat <<EOF\nhello\nEOF\nsh -c 'git commit -m x'",
        "cat <<'EOF'\nhello\nEOF\nsh -c 'git push'",
        "cat <<A <<B\nx\nA\ny\nB\nsh -c 'git push'",
        "cat <<EOF\nhello\nEOF\ncat <<EOF\nx\nEOF\nsh -c 'git push'",
        "cat <<E\"O\"F\nE\nsh -c 'git push'\nEOF",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` read the command after the terminator as part of one read-only line"
        );
    }
}

#[test]
fn a_here_document_alone_still_has_its_body_blanked() {
    for command in [
        "cat <<EOF\nestigia stand-down\nEOF",
        "cat <<EOF\nestigia stand-down\nEOF\n",
        "cat <<A <<B\nestigia stand-down\nA\ngit push\nB",
    ] {
        let view = super::data_view(command);
        assert!(
            !view.contains("estigia stand-down") && !view.contains("git push"),
            "`{command}` kept its body in `{view}`"
        );
    }
}

#[test]
fn a_gh_word_the_shell_forms_into_a_subcommand_is_not_read_only() {
    for command in [
        "gh code\\space s\\sh -c box -- 'git push'",
        "gh co\"de\"space ssh -c box -- 'git push'",
        "gh co''despace ss\"h\" -c box -- 'git push'",
        "gh $'codespace' $'ssh' -c box -- 'git push'",
        "gh \"$A\" 'git push'",
        "gh $A 'git push'",
        "gh c* ssh -c box -- 'git push'",
        "gh {codespace,x} ssh -- 'git push'",
        "gh -R o/r codespace ssh -- 'git push'",
        "gh myalias 'git push'",
        "gh browse 'git push'",
        "gh auth git-credential 'git push'",
        "\\gh issue view 1 --jq '.a | split(\"x\")'",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as a read-only gh line"
        );
    }
}

#[test]
fn a_gh_read_subcommand_still_has_its_data_blanked() {
    for command in [
        "gh api repos/o/r --jq '.[] | split(\"x\")'",
        "gh pr view 3 --jq '.title | split(\"x\")'",
        "gh run list --json name --jq '.[] | split(\"x\")'",
    ] {
        assert!(
            !super::data_view(command).contains("split"),
            "`{command}` lost its read-only reading"
        );
    }
}

#[test]
fn a_here_document_the_shell_reads_differently_is_left_whole() {
    for command in [
        "cat <<EOF\n$\\\n(git push origin main)\nEOF",
        "cat <<EOF\n\\$(git push)\nEOF",
        "cat <<EOF\n${ git push; }\nEOF",
        "cat <<EOF\n${|git push; }\nEOF",
        "echo \"${ git push; }\"",
        "echo \"${|git push; }\"",
        "echo \"${VAR} git push\"",
        "cat <<END-X\nEND-X\ngit push origin main\nEND",
        "cat <<E'OF'\nE\nsh -c 'git push'\nEOF",
        "cat <<'E'OF\nE\nsh -c 'git push'\nEOF",
        "cat <<EOF>f\nEOF\nsh -c 'git push'",
        "echo $'git push'",
        "echo $'\\x27'; sh -c 'git push'",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as data"
        );
    }
}

#[test]
fn a_plain_here_document_is_still_data() {
    for command in [
        "cat <<'EOF'\n$\\\n(git push)\n${ git push; }\nEOF",
        "cat <<\\EOF\nestigia stand-down\nEOF",
        "cat <<\"EOF\"\nestigia stand-down\nEOF",
        "cat <<-EOF\n\testigia stand-down\n\tEOF",
    ] {
        let view = super::data_view(command);
        assert!(
            !view.contains("git push") && !view.contains("stand-down"),
            "`{command}` kept its data in `{view}`"
        );
    }
}

#[test]
fn a_quoted_here_document_delimiter_holding_a_backslash_is_left_whole() {
    for command in [
        "cat <<\"E\\\\\"\nE\\\ngit push origin main\nE\\\\",
        "cat <<\"E\\$\"\nE$\nsh -c 'git push'\nE\\$",
        "cat <<\"E\\`\"\nE`\nsh -c 'git push'\nE\\`",
        "cat <<'E\\'\nE\\\nsh -c 'git push'\nE\\",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as data"
        );
    }
}

#[test]
fn a_line_a_builtin_evaluates_or_a_carriage_return_changes_is_left_whole() {
    for command in [
        "cat <<A <<'B'\nA\r\n$(git push origin main)\nB",
        "cat <<EOF\nx\r\nEOF\r\nsh -c 'git push'",
        "echo 'git push'\r",
        "printf -v 'a[$(git push origin main)]' x",
        "test -v 'a[$(git push origin main)]'",
        "printf '%d' 'a[$(git push origin main)]'",
        "gh repo clone o/r -- '--upload-pack=git push'",
        "gh repo clone o/r '--' '--upload-pack=git push'",
        "gh repo fork o/r -- '--upload-pack=git push'",
        "gh gist clone abc -- '--upload-pack=git push'",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as data"
        );
    }
}

#[test]
fn a_head_word_that_is_not_a_bare_read_only_name_leaves_the_line_whole() {
    for command in [
        "a=/cat sh -c 'git push origin main'",
        "</bin/cat sh -c 'git push origin main'",
        "sh${IFS}-c${IFS}'$1'${IFS}x/cat 'git push origin main'",
        "A=1 cat 'git push'",
        ">f cat 'git push'",
        "/bin/cat 'git push'",
        "x/cat 'git push'",
        "c\\at 'git push'",
        "\"cat\" 'git push'",
        "ca''t 'git push'",
        "cat a'git push'",
        "cat 'a'\"b\"",
        "cat 'a'b",
        "cat 'x' > '.git/hooks/pre-push'",
        "cat 'x' >> \"f\"",
    ] {
        assert_eq!(
            super::data_view(command),
            command,
            "`{command}` was read as a read-only line"
        );
    }
}

#[cfg(unix)]
mod differential {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::super::data_view;

    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }

        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            &items[self.below(items.len())]
        }
    }

    struct Case {
        text: String,
        markers: Vec<String>,
    }

    struct Builder {
        rng: Xorshift,
        markers: Vec<String>,
    }

    const HEADS: &[&str] = &["cat", "echo", "printf", "test", "grep x", "gh issue view 1"];

    const SHAPED_HEADS: &[&str] = &[
        "a=/cat sh -c '{}'",
        "A=1 cat sh -c '{}'",
        "</bin/cat sh -c '{}'",
        "sh${IFS}-c${IFS}'{}'${IFS}x/cat",
        "x/cat sh -c '{}'",
        "/bin/cat '{}'",
        ">f cat '{}'",
        "s''h -c '{}'",
        "\"sh\" -c '{}'",
        "c\\at '{}'",
        "cat a'{}'",
        "cat '{}'\"b\"",
        "ca''t '{}'",
        "echo x>f '{}'",
    ];

    const DELIMITERS: &[(&str, &str)] = &[
        ("EOF", "EOF"),
        ("A", "A"),
        ("'B'", "B"),
        ("'EOF'", "EOF"),
        ("\"EOF\"", "EOF"),
        ("\\EOF", "EOF"),
        ("E-X", "E-X"),
        ("\"E\\\\\"", "E\\"),
        ("'E\\'", "E\\"),
        ("E\\X", "EX"),
        ("E'O'F", "EOF"),
        ("'E'OF", "EOF"),
        ("\"E$\"", "E$"),
        ("\"E\\$\"", "E$"),
        ("\"E\\`\"", "E`"),
        ("E_1", "E_1"),
        ("\"\"EOF", "EOF"),
    ];

    impl Builder {
        fn marker(&mut self) -> String {
            let text = format!("touch ran_{}_", self.markers.len());
            self.markers.push(text.clone());
            text
        }

        fn argument(&mut self) -> String {
            if self.rng.below(10) < 6 {
                return (*self
                    .rng
                    .pick(&["x", "'a b'", "\"c\"", "-n", "'q'\"r\"", "-v a"]))
                .to_owned();
            }
            let marker = self.marker();
            match self.rng.below(25) {
                0 => format!("'{marker}'"),
                1 => format!("\"{marker}\""),
                2 => format!("$'{marker}'"),
                3 => format!("\"$({marker})\""),
                4 => format!("\"`{marker}`\""),
                5 => format!("$({marker})"),
                6 => format!("`{marker}`"),
                7 => format!("$\\\n({marker})"),
                8 => format!("\"a\\\n$({marker})\""),
                9 => format!("'a'$'\\'' ; {marker} ; $'\\''"),
                10 => format!("\"${{ {marker};}}\""),
                11 => format!("x ; {marker}"),
                12 => format!("| {marker}"),
                13 => format!("&& {marker}"),
                14 => format!("\n{marker}"),
                15 => format!("# {marker}"),
                16 => "a\\ b".to_owned(),
                17 => format!("<({marker})"),
                18 => format!("; {{ {marker}; }}"),
                19 => format!("'a\n{marker}\nb'"),
                20 => format!("\"$\\\n({marker})\""),
                21 => format!("-v 'a[$({marker})]'"),
                22 => format!("-v \"a[$({marker})]\""),
                23 => format!("'a[$({marker})]'"),
                _ => format!("x\\\n; {marker}"),
            }
        }

        fn body_line(&mut self, delimiter: &str) -> String {
            if self.rng.below(10) < 4 {
                return (*self.rng.pick(&["hello", "x y", "E", "EO", "$X ${X}"])).to_owned();
            }
            let marker = self.marker();
            match self.rng.below(17) {
                14 => format!("$\\\n({marker})"),
                15 => delimiter.to_lowercase(),
                16 => delimiter.to_uppercase(),
                0 => "hello".to_owned(),
                1 => marker,
                2 => format!("$({marker})"),
                3 => format!("`{marker}`"),
                4 => format!("${{ {marker};}}"),
                5 => "$\\".to_owned(),
                6 => format!("({marker})"),
                7 => format!("\\$({marker})"),
                8 => format!("'{marker}'"),
                9 => "E".to_owned(),
                10 => "EO".to_owned(),
                11 => "$X ${X}".to_owned(),
                12 => format!("x \\\n$({marker})"),
                _ => format!("\"{marker}\""),
            }
        }

        fn follow_line(&mut self) -> String {
            let marker = self.marker();
            match self.rng.below(10) {
                7 => "E".to_owned(),
                8 => "EOF".to_owned(),
                9 => "E\\\\".to_owned(),
                0 => marker,
                1 => format!("sh -c '{marker}'"),
                2 => format!("eval \"{marker}\""),
                3 => format!("echo hi ; {marker}"),
                4 => format!("{marker} # c"),
                5 => format!("cat <<EOF\nx\nEOF\n{marker}"),
                _ => format!("true && {marker}"),
            }
        }

        fn terminator(&mut self, delimiter: &str, strip_tabs: bool) -> String {
            match self.rng.below(8) {
                0 | 1 => format!("{delimiter}\r"),
                2 => format!("{delimiter} "),
                3 => format!(" {delimiter}"),
                4 => format!("{delimiter}x"),
                5 if strip_tabs => format!("\t\t{delimiter}"),
                6 => String::new(),
                _ => delimiter.to_owned(),
            }
        }

        fn head(&mut self) -> String {
            match self.rng.below(20) {
                14..=19 => {
                    let marker = self.marker();
                    self.rng.pick(SHAPED_HEADS).replace("{}", &marker)
                }
                12 => {
                    let marker = self.marker();
                    format!("gh gist clone abc -- '--upload-pack={marker}'")
                }
                0 => {
                    let marker = self.marker();
                    format!("gh repo clone o/r -- '--upload-pack={marker}'")
                }
                1 => {
                    let marker = self.marker();
                    format!("gh repo clone o/r '--' '--upload-pack={marker}'")
                }
                _ => (*self.rng.pick(HEADS)).to_owned(),
            }
        }

        fn case(&mut self) -> Case {
            let mut text = self.head();
            for _ in 0..self.rng.below(3) {
                let argument = self.argument();
                text.push(' ');
                text.push_str(&argument);
            }
            let mut pending = Vec::new();
            let carriage_return_pair = self.rng.below(8) == 0;
            let spellings = self.rng.below(3);
            for index in 0..if carriage_return_pair { 2 } else { spellings } {
                let (spelling, delimiter) = if carriage_return_pair {
                    [("A", "A"), ("'B'", "B")][index]
                } else {
                    *self.rng.pick(DELIMITERS)
                };
                let strip_tabs = self.rng.below(3) == 0;
                text.push_str(if strip_tabs { " <<-" } else { " <<" });
                text.push_str(spelling);
                pending.push((delimiter, strip_tabs));
            }
            for (delimiter, strip_tabs) in pending {
                for _ in 0..self.rng.below(4) {
                    let line = self.body_line(delimiter);
                    text.push('\n');
                    text.push_str(&line);
                }
                let terminator = if carriage_return_pair && delimiter == "A" {
                    "A\r".to_owned()
                } else {
                    self.terminator(delimiter, strip_tabs)
                };
                text.push('\n');
                text.push_str(&terminator);
            }
            for _ in 0..self.rng.below(5) {
                let line = self.follow_line();
                text.push('\n');
                text.push_str(&line);
            }
            Case {
                text,
                markers: std::mem::take(&mut self.markers),
            }
        }
    }

    const STUB_GH: &str = "#!/bin/sh\ncase \"$1 $2\" in\n\"repo clone\" | \"repo fork\" | \"gist clone\") ;;\n*) exit 0 ;;\nesac\nseen=0\nfor word in \"$@\"; do\n  if [ \"$seen\" = 1 ]; then\n    case \"$word\" in\n      --upload-pack=*) sh -c \"${word#--upload-pack=}\" ;;\n    esac\n  fi\n  [ \"$word\" = \"--\" ] && seen=1\ndone\nexit 0\n";

    fn sandbox() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(dir.path().join("bin")).expect("a stub directory");
        let stub = dir.path().join("bin").join("gh");
        std::fs::write(&stub, STUB_GH).expect("a stub gh");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("stub mode");
        dir
    }

    fn bash_program() -> &'static Path {
        static PROGRAM: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        PROGRAM.get_or_init(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .chain(["/usr/bin", "/bin"].map(std::path::PathBuf::from))
                .map(|dir| dir.join("bash"))
                .find(|candidate| candidate.is_file())
                .unwrap_or_else(|| {
                    panic!(
                        "the differential test requires `bash` on the path or in /usr/bin or /bin"
                    )
                })
        })
    }

    fn bash(dir: &Path, cwd: &Path, arguments: &[&str]) -> std::io::Result<std::process::Child> {
        let path = format!("{}:/usr/bin:/bin", dir.join("bin").display());
        Command::new(bash_program())
            .args(arguments)
            .env_clear()
            .env("PATH", path)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }

    fn fresh_directory(dir: &Path) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let cwd = dir.join(format!(
            "case{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&cwd).expect("a case directory");
        cwd
    }

    fn bash_banner(dir: &Path) -> (String, (u32, u32)) {
        let output = Command::new(bash_program())
            .args([
                "--noprofile",
                "--norc",
                "-c",
                "echo ${BASH_VERSINFO[0]} ${BASH_VERSINFO[1]} $BASH_VERSION",
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .current_dir(dir)
            .output()
            .unwrap_or_else(|error| {
                panic!("the differential test requires `bash` on the path: {error}")
            });
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let mut words = text.split_whitespace();
        let mut number = || words.next().and_then(|word| word.parse().ok()).unwrap_or(0);
        let version = (number(), number());
        (text, version)
    }

    enum Outcome {
        Executed(Vec<usize>),
        TimedOut,
    }

    fn executed_by_bash(text: &str, markers: usize, dir: &Path) -> Outcome {
        let cwd = fresh_directory(dir);
        let mut child =
            bash(dir, &cwd, &["--noprofile", "--norc", "-c", text]).unwrap_or_else(|error| {
                panic!("the differential test requires `bash` on the path: {error}")
            });
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if child.try_wait().expect("bash was waited on").is_some() {
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Outcome::TimedOut;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Outcome::Executed(
            (0..markers)
                .filter(|number| cwd.join(format!("ran_{number}_")).exists())
                .collect(),
        )
    }

    fn judge(text: &str, markers: &[String], dir: &Path) -> (usize, Option<String>) {
        match executed_by_bash(text, markers.len(), dir) {
            Outcome::TimedOut => (0, Some(format!("bash timed out on {text:?}"))),
            Outcome::Executed(executed) => {
                let view = data_view(text);
                let failure = executed
                    .iter()
                    .find(|number| !view.contains(&markers[**number]))
                    .map(|number| {
                        format!(
                            "bash ran `{}` and data_view blanked it\ninput: {text:?}\nview:  {view:?}",
                            markers[*number]
                        )
                    });
                (executed.len(), failure)
            }
        }
    }

    const MEASURED_BY_EARLIER_ROUNDS: &[(&str, (u32, u32))] = &[
        ("cat <<EOF\nhello\nEOF\nsh -c 'touch ran_0_'", (3, 0)),
        ("cat <<EOF\n$\\\n(touch ran_0_)\nEOF", (3, 0)),
        ("cat <<END-X\nEND-X\ntouch ran_0_\nEND", (3, 0)),
        ("cat <<\"E\\\\\"\nE\\\ntouch ran_0_\nE\\\\", (3, 0)),
        ("cat <<E'OF'\nE\nEOF\ntouch ran_0_", (3, 0)),
        ("cat <<'E'OF\nE\nEOF\ntouch ran_0_", (3, 0)),
        ("cat <<EOF>f\nEOF\ntouch ran_0_", (3, 0)),
        ("echo $'\\'' ; touch ran_0_ ; $'\\''", (3, 0)),
        ("cat <<A <<B\nx\nA\ny\nB\ntouch ran_0_", (3, 0)),
        ("echo \"$\\\n(touch ran_0_)\"", (3, 0)),
        ("cat <<A <<'B'\nA\r\n$(touch ran_0_)\nB", (3, 0)),
        ("cat <<EOF <<'X'\neof\n$(touch ran_0_)\nEOF\nX", (3, 0)),
        ("printf -v 'a[$(touch ran_0_)]' x", (4, 0)),
        ("test -v 'a[$(touch ran_0_)]'", (4, 2)),
        ("echo \"${ touch ran_0_; }\"", (5, 3)),
        ("gh repo clone o/r -- '--upload-pack=touch ran_0_'", (3, 0)),
        (
            "gh repo clone o/r '--' '--upload-pack=touch ran_0_'",
            (3, 0),
        ),
    ];

    const GENERATED_CASES: usize = 4000;
    const EXECUTED_MARKER_FLOOR: usize = 4500;

    #[test]
    fn nothing_bash_executes_is_blanked_as_data() {
        let dir = sandbox();
        let (banner, version) = bash_banner(dir.path());
        let mut failures = Vec::new();
        for (text, since) in MEASURED_BY_EARLIER_ROUNDS {
            let markers = vec!["touch ran_0_".to_owned()];
            let (executed, failure) = judge(text, &markers, dir.path());
            failures.extend(failure);
            if version >= *since && executed == 0 {
                failures.push(format!("bash {banner} did not run the marker in {text:?}"));
            }
        }
        let mut builder = Builder {
            rng: Xorshift(0x9E37_79B9_7F4A_7C15),
            markers: Vec::new(),
        };
        let cases: Vec<Case> = (0..GENERATED_CASES).map(|_| builder.case()).collect();
        let workers = std::thread::available_parallelism().map_or(2, usize::from);
        #[allow(clippy::needless_collect)]
        let handles: Vec<_> = std::thread::scope(|scope| {
            let spawned: Vec<_> = cases
                .chunks(cases.len().div_ceil(workers))
                .map(|chunk| {
                    let dir = dir.path();
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|case| judge(&case.text, &case.markers, dir))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            spawned
                .into_iter()
                .flat_map(|handle| handle.join().expect("a worker finished"))
                .collect()
        });
        let executed: usize = handles.iter().map(|(count, _)| count).sum();
        failures.extend(handles.into_iter().filter_map(|(_, failure)| failure));
        println!("executed markers across {GENERATED_CASES} cases: {executed}");
        assert!(
            executed >= EXECUTED_MARKER_FLOOR,
            "bash {banner} executed only {executed} markers across {GENERATED_CASES} cases, so the corpus proves too little"
        );
        assert!(
            failures.is_empty(),
            "bash {banner}\n{}",
            failures[..failures.len().min(5)].join("\n\n")
        );
    }
}

#[test]
fn a_heredoc_target_may_be_drive_lettered_but_not_drive_relative() {
    let heredoc = |target: &str| format!("cat > {target} <<'EOF'\nx\nEOF");
    for target in [
        "C:/Users/runneradmin/AppData/Local/Temp/.tmpapCxiK/out.md",
        "c:/a",
    ] {
        assert_eq!(
            quoted_heredoc_target(&heredoc(target)).as_deref(),
            Some(target)
        );
    }
    for target in ["C:", "C:out.md", "1:/a", "CC:/a", "C:/../a"] {
        assert_eq!(quoted_heredoc_target(&heredoc(target)), None, "{target}");
    }
}
