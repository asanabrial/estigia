/// Every verb on the list has a command line somebody wrote, and the other way.
///
/// Found by reading: **nineteen** of the fifty-one verbs here appear in no test
/// in this repository, and four of the twenty-nine wrappers do. All twelve
/// Windows spellings are among them — `add-content`, `new-item`, `copy-item`,
/// `robocopy`, `ren` — on the platform this list's own note says it carries them
/// for: *this crate's own platform is the one where `cmd /c` and `Set-Content`
/// are the ordinary way to write*.
///
/// An entry nothing crosses is one that can be dropped, renamed or misspelled
/// in silence, and the cost is written a few lines up in the same file: a shell
/// line that visibly writes a control surface classifies as `Routine` rather
/// than `Boundary`, so the gate lets it through on an answer from inside the
/// renewal window.
///
/// The lines are **written out** rather than built from the entries. A loop
/// that formats `"{verb} src/x.rs"` cannot fail for a renamed verb — it renames
/// the question with the answer — which is what the first version of this test
/// did, and it passed against `add-contents` and against `niced` alike.
/// Written out, the two sides are independent: renaming an entry leaves a line
/// nothing matches, and adding one leaves the count short.
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

use super::*;

/// Every spelling that visibly puts bytes on disk is read as a write.
///
/// The declaration above `writes_a_file` says the constructs that visibly write
/// a file are "a small, nameable population, and this reads for exactly those".
/// Measured rather than read: thirty command lines that visibly write were
/// classified and twenty-six came back `Untouched`.
///
/// The sharpest were siblings of names already in the list — `unzip` and `tar`
/// were writes while `zip`, `gzip`, `xz` and `7z` were not, which is a list that
/// stopped halfway rather than a boundary. And `wget -O src/main.rs URL` is this
/// module's own opening sentence with a different verb: "a write denied as
/// `Edit` could be spelled `echo … > src/x.rs` and go through untouched".
///
/// The second half matters as much. The declaration excludes programs that
/// write as a *side effect* — `make`, `cargo build`, `npm install` — by name,
/// and the reason is written there: the alternative is gating every command that
/// could write, which is every command. Widening the list until those match
/// would satisfy the first half of this test and destroy the thing it guards.
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

/// Every ordinary way git writes the working tree is one the gate sees.
///
/// `REPOSITORY_SHELL` says the rule this holds it to, in its own words:
/// *too-tight is a licence to be incomplete, not to omit a member sitting next
/// to one that is here*. Each of these sits next to one that is:
///
/// - `git worktree add` is listed and `git worktree remove` was not — one makes
///   a checkout, the other deletes it.
/// - `git clean` is listed, which deletes untracked files, and `git rm` was not,
///   which deletes tracked ones.
/// - `git cherry-pick` is listed and `git revert` was not, and they are the same
///   operation in opposite directions.
/// - `git commit` is listed and `git am` was not, which applies a patch *and*
///   commits it.
/// - `git reset --hard` is listed and `--keep` and `--merge` were not, and all
///   three move the working tree to another commit.
///
/// Every one of them names git, every one changes files the gate exists to
/// measure, and every one read as `Untouched` — invisible, while its neighbour
/// on the next line renewed the claim.
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
#[test]
fn an_option_before_the_subcommand_does_not_hide_it() {
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
        ("git -c user.name=x merge main", "git merge main"),
        ("git --no-pager tag v1", "git tag v1"),
        ("git -P clean -fd", "git clean -fd"),
        ("git --literal-pathspecs rm -f a.rs", "git rm -f a.rs"),
        (
            "git -c a=b -C src -c d=e worktree add ../w",
            "git worktree add ../w",
        ),
    ] {
        let read = |command: &str| {
            crate::harness::classify_with("Bash", &serde_json::json!({ "command": command }), &[]).0
        };
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
    // unclassified — on the platform this crate is developed on.
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
