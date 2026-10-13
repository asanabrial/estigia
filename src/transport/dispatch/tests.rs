use super::Flags;

fn context(root: &std::path::Path) -> crate::transport::Context {
    crate::transport::Context {
        skill_dir: root.to_path_buf(),
        repo_dir: root.to_path_buf(),
        config: Vec::new(),
        repo: None,
    }
}

/// A switch is not a flag that swallows the next one.
///
/// `SWITCHES` is a closed, named list rather than a guess from the next word's
/// shape, so a switch never takes a value whatever follows it — in the middle
/// of the list or at the end — and the value-taking flag right after one still
/// gets its own value.
#[test]
fn a_switch_does_not_swallow_the_flag_after_it() {
    let flags: Vec<String> = ["--force", "--issue", "12", "--fix"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    let read = Flags::read(&flags);

    assert!(read.on("force"), "a switch in the middle went unseen");
    assert!(read.on("fix"), "a switch at the end went unseen");
    assert_eq!(
        read.get("force"),
        None,
        "a switch took a value it has none of"
    );
    assert_eq!(
        read.get("issue"),
        Some("12"),
        "a valued flag lost its value"
    );
    assert_eq!(read.get("fix"), None, "a switch at the end took a value");
}

/// A value is read whole, whatever it looks like — including a value that
/// starts with `--`.
///
/// Before `SWITCHES` existed, `Flags::read` decided a flag had no value by
/// checking whether the *next word* started with `--`, on the theory that such
/// a word must be the next flag. That theory is only true for an actual
/// switch; for anything else it is a guess, and the first free-text argument
/// this transport took — `record_review_verdict`'s `basis` — was proof the
/// guess was wrong. A basis of `"--nothing was measured"` is a sentence, not a
/// flag, and reading it as one made `--basis` look as if it had never been
/// given a value at all: `get("basis")` answered `None` for a caller that had
/// supplied one, which is the *absent* answer and not the *empty* one —
/// exactly the distinction `record_review_verdict`'s blank-basis refusal
/// exists to keep apart (issue #35).
#[test]
fn a_flags_value_is_read_even_when_it_looks_like_a_flag() {
    let flags: Vec<String> = ["--outcome", "accepted", "--basis", "--nothing was measured"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    let read = Flags::read(&flags);

    assert_eq!(
        read.get("basis"),
        Some("--nothing was measured"),
        "a value beginning with -- was dropped rather than read"
    );
    assert_eq!(read.get("outcome"), Some("accepted"));
}

/// A switch nobody passed is off, for every switch the dispatch reads.
///
/// The test above this one asserts `on` in the direction that says **yes**, and
/// so did every other: measured by mutation, making `Flags::on` answer `true`
/// for everything left the whole suite green. Nothing anywhere said that a flag
/// which is not there is off.
///
/// The one that matters is `--force`. `reclaim` refuses with `holder-not-stale`
/// when the holder is alive *and this is not forced*; a reader that answers yes
/// to a flag nobody passed turns every reclaim into a forced one, and taking an
/// issue from a live holder stops being something anybody asked for.
///
/// The list is read out of the dispatch rather than written here, so a switch
/// added later is measured by this test or fails it — the same reason
/// `every_method_this_server_implements_is_answered` reads its own source.
#[test]
fn a_switch_nobody_passed_is_off() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/transport/dispatch.rs"),
    )
    .expect("the dispatch's own source");
    // Stripped, because the prose above `SWITCHES` names `.on("...")` to
    // describe this very scan and a reader of source text cannot tell that
    // apart from a call.
    let source = crate::source_text::code_of(&source);
    let switches: std::collections::BTreeSet<&str> = source
        .lines()
        .flat_map(|line| {
            line.match_indices(".on(\"")
                .filter_map(move |(at, opener)| {
                    line[at + opener.len()..]
                        .split_once('"')
                        .map(|(name, _)| name)
                })
        })
        .collect();
    // The floor: six switches reach `on` today, and a scan that read none
    // would satisfy every assertion below without making one.
    assert!(
        switches.len() >= 6 && switches.contains("force"),
        "the dispatch was not read: {switches:?}"
    );

    // And the hand-written list `Flags::read` decides by is exactly the set the
    // dispatch actually asks `on` about. Without this, adding a `.on("new")`
    // and forgetting the list makes `--new` take the next word as its value and
    // swallow the flag after it — the same silent drop issue #35 was filed for,
    // reintroduced from the other end. A list nothing crosses is the defect
    // class this repository refuses, and the scan above already knows the
    // answer.
    let listed: std::collections::BTreeSet<&str> = super::SWITCHES.iter().copied().collect();
    assert_eq!(
        switches, listed,
        "the switches the dispatch reads and the list `Flags::read` decides by have drifted apart"
    );

    // The other half of the same claim, which the crossing above does not ask:
    // a name on the list is read with `on` and never with a value reader. A
    // later `f.get("force")` would answer `None` forever and silently, because
    // `force` is on the list and so never reaches `named` — the hazard the
    // doc comment names, and until this it named it without holding it.
    for name in super::SWITCHES {
        for reader in ["get", "need", "number", "path"] {
            let call = format!(".{reader}(\"{name}\"");
            assert!(
                !source.lines().any(|line| line.contains(&call)),
                "`--{name}` is a switch and is also read for a value with `{reader}`, which answers `None` for every caller"
            );
        }
    }

    let nothing: Vec<String> = Vec::new();
    let empty = Flags::read(&nothing);
    for switch in &switches {
        assert!(
            !empty.on(switch),
            "`--{switch}` is read as passed by a caller who passed nothing"
        );
    }

    // And a switch's own name arriving as another flag's **value** is not that
    // switch: `--to force` says where to move something, not to force it.
    let disguised: Vec<String> = ["--to", "force"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    assert!(
        !Flags::read(&disguised).on("force"),
        "a word used as another flag's value was read as a switch"
    );
}

/// The last spelling of a repeated flag is the one that counts.
///
/// `argparse` keeps the last, so a caller that passes one twice already expects
/// it. Keeping the first would answer with a value the transport would not have
/// used.
#[test]
fn a_repeated_flag_keeps_the_last_value() {
    let flags: Vec<String> = ["--to", "review", "--to", "done"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    assert_eq!(Flags::read(&flags).get("to"), Some("done"));
}

/// An operation nobody implements is refused, and refused as the caller's defect.
///
/// Never `ok`, and never quietly handed anywhere else. This replaced a call that
/// spawned an interpreter, and the one outcome that must not survive the change
/// is a request that goes somewhere unnamed and comes back looking answered.
#[test]
fn an_operation_nobody_implements_is_refused() {
    let root = tempfile::tempdir().expect("a directory");
    let refusal = super::dispatch(
        &context(root.path()),
        "no-such-operation",
        &[],
        "2026-01-01T00:00:00Z",
    )
    .expect_err("an operation nobody implements answered");
    assert_eq!(
        refusal.code(),
        2,
        "an unknown operation was not the caller's own defect"
    );
    assert_eq!(
        refusal
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str),
        Some("unknown-operation")
    );
}

/// A missing argument is named, not guessed at.
#[test]
fn a_missing_argument_says_which_one() {
    let root = tempfile::tempdir().expect("a directory");
    let refusal = super::dispatch(
        &context(root.path()),
        "transition",
        &[],
        "2026-01-01T00:00:00Z",
    )
    .expect_err("a transition with no issue answered");
    let envelope = refusal.envelope();
    assert_eq!(
        envelope.get("reason").and_then(serde_json::Value::as_str),
        Some("missing-argument")
    );
    assert_eq!(
        envelope.get("argument").and_then(serde_json::Value::as_str),
        Some("--issue"),
        "the refusal does not name the argument it wanted: {envelope}"
    );
}

/// `--findings` without a receipt is refused before `create` writes anything.
///
/// The dependency check used to live only in `body_with_marker`, which `create`
/// reaches after it has created every label, so a call naming a finding and no
/// epoch left labels behind and filed nothing. Mutation: drop the dispatch
/// check and this reddens, because `gh label create` runs before the refusal.
#[test]
fn findings_without_a_receipt_are_refused_before_any_label_is_created() {
    const BODY: &str = "## Description for dumb humans\n\n> [!NOTE]\n> A probe.\n\n\
        ## Problem\nA probe.\n\n\
        ## Why it matters\nA probe.\n\n\
        ## Proposed approach\nA probe.\n\n\
        ## Acceptance criteria\nA probe.\n\n\
        ## Out of scope\nA probe.\n\n\
        ## Evidence\nA probe.\n";
    let root = tempfile::tempdir().expect("a directory");
    let count = root.path().join("count");
    let body = root.path().join("body.md");
    std::fs::write(&body, BODY).expect("the body file");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        serde_json::json!({ "matches": "label create", "status": 0 }),
        serde_json::json!({ "matches": "issue create", "status": 0 }),
    ]);
    let flags: Vec<String> = [
        "--identity",
        "a-probe",
        "--title",
        "A probe",
        "--body-file",
        body.to_str().expect("a utf-8 path"),
        "--findings",
        "finding-one",
    ]
    .iter()
    .map(|part| (*part).to_owned())
    .collect();
    let refusal = crate::test_env::with_counting_gh(bin.path(), &script, &count, || {
        super::dispatch(
            &context(root.path()),
            "create",
            &flags,
            "2026-01-01T00:00:00Z",
        )
    })
    .expect_err("a finding identity with no receipt was accepted");
    assert_eq!(
        refusal
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str),
        Some("finding-without-transaction")
    );
    let seen = std::fs::read_to_string(&count).unwrap_or_default();
    assert!(
        !seen.contains("label create"),
        "a label was created before the refusal: {seen}"
    );
}

/// Every operation the tool table names is one this can answer.
///
/// The floor under the whole change. A dispatcher that knew sixteen of the
/// nineteen would route three tools into `unknown-operation`, and the agent
/// would read a refusal where it used to read an answer — from a table that
/// still advertises the tool.
#[test]
fn every_operation_the_tools_name_is_dispatched() {
    let root = tempfile::tempdir().expect("a directory");
    let context = context(root.path());
    for tool in crate::harness::mcp::tools::TOOLS {
        let refusal = super::dispatch(&context, tool.operation, &[], "2026-01-01T00:00:00Z")
            .err()
            .map(|failure| failure.envelope());
        // Called with no flags at all, so every one of them refuses. What is
        // asserted is *which* refusal: a missing argument means the operation
        // was recognised and asked for what it needs.
        let reason = refusal
            .as_ref()
            .and_then(|envelope| envelope.get("reason"))
            .and_then(serde_json::Value::as_str);
        assert_ne!(
            reason,
            Some("unknown-operation"),
            "the tool table offers `{}` and nothing answers it",
            tool.operation
        );
    }
}

/// What `estigia config` claims it performs is what a tool can actually reach.
///
/// `SCRIPTED` calls itself *a claim about the contract, not a reflection of the
/// dispatch table*, and says the thing that used to hold it to the dispatch is
/// deleted. That left both directions uncrossed, and one of them drifted the
/// first time an operation was added: `republish-review` was dispatched and
/// exposed, and `config` went on reporting the list without it. An operator
/// reading that answer is told this transport does not perform an operation it
/// performs — and this is the document a binding author reads to find out what
/// they have to map.
///
/// Deliberately one-directional. A name in `SCRIPTED` that no tool offers is the
/// case that comment already describes, and it is not always a defect: the list
/// is a claim about the *contract*, which may name something the MCP surface
/// does not expose. What cannot be true is the reverse.
#[test]
fn every_operation_a_tool_offers_is_claimed_or_declared_unscripted() {
    for tool in crate::harness::mcp::tools::TOOLS {
        let claimed = super::super::commands::SCRIPTED.contains(&tool.operation);
        // `NOT_SCRIPTED` spells its keys the way the contract does, with
        // underscores, and pairs two of them on one row.
        let declared = super::super::commands::NOT_SCRIPTED
            .iter()
            .any(|(name, _)| name.split('/').any(|name| name == tool.contract_name));
        assert!(
            claimed || declared,
            "`{}` is dispatched and offered as the `{}` tool, and `estigia config` claims neither \
             that it performs it nor a reason it does not",
            tool.operation,
            tool.name
        );
    }
}

/// Every flag a tool sends is one this reads.
///
/// The half `every_operation_the_tools_name_is_dispatched` does not cover: that
/// test proves the operation is recognised, and this one proves the arguments
/// arrive. A tool that sends `--expect-state` while this reads `expect_state`
/// loses the value in silence — the call is dispatched, the flag is parsed into
/// a map nobody asks, and the operation runs as though the caller had said
/// nothing.
///
/// Checked by **asking the dispatcher for each name**, because the alternative
/// is a second list of flags in a test, and a second list is the thing this
/// crate keeps finding disagreeing with the first.
///
/// Asked of the operation's **own arm**, not of the file. Searching the whole of
/// `dispatch.rs` means a flag another operation reads answers for this one, and
/// two defects walked through that gap:
///
/// - `publish_review` declares `worktree` — *"The isolated checkout"* — the
///   original pushes from it, and this arm had no field for it. The flag was
///   parsed into a map nobody read and the push ran in the base checkout every
///   time. It passed here because `base-movement` and `expected-target` read a
///   `"worktree"` of their own, thirty lines up.
/// - `start-branch` demanded `--repo-name`, which no tool declares and no
///   binding mentions, so every call an agent could make refused. That is the
///   same seam facing the other way, and this only walked one way.
///
/// One rule covers both: a flag exists to carry a value from the caller to the
/// operation, and a flag only one end knows about carries nothing.
#[test]
fn every_flag_a_tool_sends_is_one_the_dispatcher_reads() {
    let source = crate::source_text::code_of(include_str!("../dispatch.rs"));
    let shipped = source
        .find("#[cfg(test)]")
        .map_or(source.as_str(), |at| &source[..at]);

    // The body of one `"operation" =>` arm, to the next one. A boundary rather
    // than a count of characters: `harness::tests` has already paid for a pin on
    // a fixed slice, where a comment pushed what it looked for out of the
    // window.
    let arm = |operation: &str| -> Option<&str> {
        let head = format!("\n        \"{operation}\" =>");
        let at = shipped.find(&head)? + head.len();
        let rest = &shipped[at..];
        Some(&rest[..rest.find("\n        \"").unwrap_or(rest.len())])
    };

    // Read by the closures at the top of `dispatch`, not inside any arm.
    const COMMON: &[&str] = &["issue", "run-id"];
    // Minted by the tool server, so no tool declares them.
    const MINTED: &[&str] = &["operation-id", "runtime"];
    // Read from the run pointer by the tool server, so a caller cannot name
    // another branch or checkout.
    const FROM_THE_POINTER: &[(&str, &str)] = &[("draft-pr", "branch"), ("draft-pr", "worktree")];
    // Flags an arm reads that are not a caller's to send, with why.
    //
    // Every entry is an operator switch or a value derived from the checkout. A
    // flag that changes what the operation *checks* does not belong here — that
    // is `expect-state`, and leaving it off the tool is how a run already in
    // `review` was left unable to publish at all.
    const OPERATOR_ONLY: &[(&str, &str, &str)] = &[
        (
            "changelog-notes",
            "include-heading",
            "a rendering choice the CLI makes, not a fact about the issue",
        ),
        (
            "verify-claim",
            "allow-closed-by-pr",
            "passed by `start-branch` and the gate, never by a caller: it names the PR whose \
             merge closed the issue",
        ),
        (
            "reclaim",
            "force",
            "an operator override; a run reclaims on the timeline or not at all",
        ),
        (
            "reclaim",
            "reason-file",
            "the prose that goes with `--force`, and the operator writes it",
        ),
        (
            "unassign",
            "worktree",
            "filled by the harness from the run pointer, never by a caller: it names the directory \
             this operation may remove, and a removal a caller could aim would be the unsafe \
             direction of the one operation that has to be safe",
        ),
        (
            "start-branch",
            "repo-name",
            "derived from the checkout when absent, as the original derives it",
        ),
    ];

    let mut checked = 0;
    let mut missing: Vec<String> = Vec::new();
    let mut demanded: Vec<String> = Vec::new();
    for tool in crate::harness::mcp::tools::TOOLS {
        let Some(body) = arm(tool.operation) else {
            // An operation with no arm is a tool that cannot run at all, which
            // `every_operation_the_tools_name_is_dispatched` owns.
            continue;
        };
        for argument in tool.arguments {
            // Bookkeeping the server keeps and never sends: see `POINTER_ONLY`.
            if crate::harness::mcp::tools::is_pointer_only(tool.name, argument.name) {
                continue;
            }
            let flag = argument.as_flag();
            let name = flag.trim_start_matches('-');
            if COMMON.contains(&name) {
                continue;
            }
            checked += 1;
            if !body.contains(&format!("\"{name}\"")) {
                missing.push(format!(
                    "{} sends {flag} and `{}` never reads it",
                    tool.name, tool.operation
                ));
            }
        }

        // A third direction, and the one that hid the worst defect of the
        // three: an argument this arm *reads* — optionally, so it never
        // refuses — that no tool declares. Sometimes that is right: `--force`,
        // `--no-cache` and their kind are the operator's switches and reach
        // this from the CLI. It is wrong when the value decides what the
        // operation checks, and `publish-review` read `expect-state` while its
        // tool declared none — so a run already in `review` could not publish,
        // which is the loop this crate's second rule requires: *every push
        // invalidates the verdict; re-publish and ask again*.
        let mut rest = body;
        loop {
            let (head, at) = match (rest.find("f.get(\""), rest.find("f.path(\"")) {
                (Some(get), Some(path)) if get < path => ("f.get(\"", get),
                (Some(_), Some(path)) => ("f.path(\"", path),
                (Some(get), None) => ("f.get(\"", get),
                (None, Some(path)) => ("f.path(\"", path),
                (None, None) => break,
            };
            rest = &rest[at + head.len()..];
            let Some(end) = rest.find('"') else { break };
            let read = &rest[..end];
            if COMMON.contains(&read)
                || MINTED.contains(&read)
                || tool
                    .arguments
                    .iter()
                    .any(|argument| argument.as_flag().trim_start_matches('-') == read)
            {
                continue;
            }
            checked += 1;
            assert!(
                OPERATOR_ONLY
                    .iter()
                    .any(|(operation, flag, _)| *operation == tool.operation && *flag == read),
                "the `{}` arm reads `--{read}` and `{}` declares no such argument, so it is \
                 whatever the default is and no agent can change it — put it on the tool, or on \
                 OPERATOR_ONLY with the reason it is not a caller's to send",
                tool.operation,
                tool.name
            );
        }

        // And the other way: an argument this arm demands that nothing sends.
        let mut rest = body;
        while let Some(at) = rest.find("need(\"") {
            rest = &rest[at + "need(\"".len()..];
            let Some(end) = rest.find('"') else { break };
            let wanted = &rest[..end];
            if COMMON.contains(&wanted)
                || MINTED.contains(&wanted)
                || FROM_THE_POINTER.contains(&(tool.operation, wanted))
            {
                continue;
            }
            checked += 1;
            if !tool
                .arguments
                .iter()
                .any(|argument| argument.as_flag().trim_start_matches('-') == wanted)
            {
                demanded.push(format!(
                    "`{}` demands --{wanted} and `{}` declares no such argument",
                    tool.operation, tool.name
                ));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these flags are sent and never read, so their values are lost in silence: {missing:#?}"
    );
    assert!(
        demanded.is_empty(),
        "these arguments are demanded and nothing sends them, so every call refuses: {demanded:#?}"
    );
    // The floor: a walk that inspected nothing would assert nothing, and the
    // tool table is the thing being crossed.
    assert!(
        checked > 30,
        "{checked} arguments were crossed, which is fewer than the tool table has"
    );
}

/// A live call carries the operator's table.
///
/// The defect this exists for was silent and total: both live callers built
/// their context with `config: Vec::new()` when the operations moved into this
/// process, and `Context::get` prefix-matches that list. Every row resolved to
/// nothing, so `start-branch` read an empty worktree template whatever the
/// operator had set.
///
/// Nothing failed. That is what makes it worth a test of its own.
///
/// Structural, because the two callers are the gate and the tool server and
/// neither can be run here without a tracker: what is checked is that they build
/// the context through the constructor that reads the contract, and that the
/// constructor reads it.
#[test]
fn a_live_call_carries_the_operators_table() {
    let root = tempfile::tempdir().expect("a skill root");
    std::fs::write(
        root.path().join(crate::skill::CONTRACT),
        format!(
            "# Contract\n\n{}\n| Setting | Value |\n|---|---|\n| Worktree location | /work/trees |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("a contract");

    let live =
        crate::transport::Context::live(root.path().to_path_buf(), root.path().to_path_buf(), None);
    assert_eq!(
        live.get("worktree location"),
        Some("/work/trees"),
        "a live context did not read the operator's table: {:?}",
        live.config
    );

    // The floor, and the shape of the defect: the same call with the empty table
    // the two callers used to build reads nothing at all.
    let empty = crate::transport::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    assert_eq!(
        empty.get("worktree location"),
        None,
        "an empty table no longer reads nothing, so this test measures nothing"
    );

    // And that the live callers build it that way. A context assembled by hand
    // beside this constructor is how the empty one got there in the first place.
    //
    // By walking the crate, not by naming files. This named two of them â€”
    // `harness/mod.rs` and `harness/mcp/mod.rs`, the two that had the defect â€”
    // and there was a third: `tui.rs` built one by hand with an empty table. A
    // guard that lists the places it has already been burned by cannot see the
    // next one, and reads like coverage while it does it.
    let mut walked = 0;
    let mut by_hand: Vec<String> = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            // A fixture may build whatever context it needs to provoke a branch;
            // the floor below is one of them.
            if path.extension().is_none_or(|kind| kind != "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Stripped, because `Context::live`'s own doc comment recounts the
            // incident by quoting the line, and a guard that cannot tell an
            // explanation from a construction would make writing the
            // explanation down the thing that fails.
            let source = crate::source_text::code_of(&source);
            walked += 1;
            let shipped = source
                .find("#[cfg(test)]\nmod tests {")
                .map_or(source.as_str(), |end| &source[..end]);
            let constructed = shipped
                .lines()
                .any(|line| line.contains("config: Vec::new()"));
            if constructed {
                by_hand.push(path.display().to_string());
            }
        }
    }
    assert!(
        walked > 20,
        "only {walked} sources were read, so this walk is not seeing the crate"
    );
    assert!(
        by_hand.is_empty(),
        "a live caller builds its transport context by hand, which is how the operator's \
         table went missing: {by_hand:?}"
    );
}

/// `start-branch` does not demand a repository name nobody sends.
///
/// The original derives it — `_, repo_name = repo_identity(cwd)` — and the port
/// turned it into `f.need("repo-name")`. Nothing supplies it: the MCP tool's
/// argument list has no `repo_name` in any spelling, and the binding never
/// mentions one, so the one operation whose whole job is making the isolated
/// checkout refused every call an agent could make.
///
/// Measured through the server: `start_branch` with exactly the arguments its
/// own schema declares came back `transport-refused`, and the envelope
/// underneath said `{"reason":"missing-argument","argument":"--repo-name"}`.
///
/// Asserted on **which** refusal, not on success: with no `gh` on the path the
/// call cannot get far, and what this pins is that it stops for a reason that
/// belongs to the world rather than for an argument the caller was never told
/// to send.
#[test]
fn start_branch_does_not_demand_a_repository_name_nobody_sends() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let flags = [
        "--issue".to_owned(),
        "12".to_owned(),
        "--run-id".to_owned(),
        "claude-abcd1234".to_owned(),
        "--branch".to_owned(),
        "fix/12".to_owned(),
        "--base".to_owned(),
        "main".to_owned(),
    ];

    let missing = |failure: &crate::transport::Failure| {
        failure
            .envelope()
            .get("argument")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    };
    let answered = super::dispatch(&context, "start-branch", &flags, "2026-01-01T00:00Z");
    if let Err(failure) = &answered {
        assert_ne!(
            missing(failure).as_deref(),
            Some("--repo-name"),
            "start-branch still refuses for an argument nothing sends"
        );
    }

    // The floor: an argument that really is the caller's is still demanded, or
    // this would pass on a dispatcher that asked for nothing at all.
    let bare = [
        "--issue".to_owned(),
        "12".to_owned(),
        "--run-id".to_owned(),
        "claude-abcd1234".to_owned(),
    ];
    let refused = super::dispatch(&context, "start-branch", &bare, "2026-01-01T00:00Z")
        .expect_err("a start-branch with no branch is not a call that can run");
    assert_eq!(
        missing(&refused).as_deref(),
        Some("--branch"),
        "the branch stopped being the caller's to name: {}",
        refused.envelope()
    );
}

fn hand_call(operation: &str, extra: &[&str]) -> serde_json::Value {
    let root = tempfile::tempdir().expect("a scratch directory");
    let mut flags: Vec<String> = [
        "--issue",
        "7",
        "--run-id",
        "run",
        "--operation-id",
        "op",
        "--epoch",
        "e",
        "--pr",
        "7",
        "--head",
        "h",
        "--base",
        "b",
        "--digest",
        "d",
    ]
    .iter()
    .map(|part| (*part).to_owned())
    .collect();
    flags.extend(extra.iter().map(|part| (*part).to_owned()));
    let refused = super::dispatch(
        &context(root.path()),
        operation,
        &flags,
        "2026-01-01T00:00:00Z",
    )
    .expect_err("a hand call to a capture-only record answered");
    assert_eq!(
        std::fs::read_dir(root.path())
            .expect("a readable scratch directory")
            .count(),
        0,
        "the refusal wrote something"
    );
    refused.envelope()
}

fn merged_release(extra: &[&str]) -> Vec<String> {
    let mut flags: Vec<String> = ["--issue", "7", "--run-id", "run"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    flags.extend(extra.iter().map(|part| (*part).to_owned()));
    flags
}

/// A merged `release` refuses what its parts refuse, in the order they run.
///
/// The close-evidence comment and the spend record are the two writes that end
/// a delivery beside the release, so `unassign` carries them: the comment runs
/// first, then the measurement, then the release. A missing evidence file is
/// the comment's own `comment-body-invalid`, and it fires before the spend is
/// even read; a malformed spend is the spend's own `malformed-argument`, and it
/// fires before the release reaches the tracker. Both are asserted with no `gh`
/// on the path, so a part that had run out of order would answer with its own
/// read failure instead of the refusal named here.
#[test]
fn a_merged_release_refuses_its_parts_in_order() {
    let root = tempfile::tempdir().expect("a directory");
    let context = context(root.path());

    let evidence_first = super::dispatch(
        &context,
        "unassign",
        &merged_release(&[
            "--evidence-file",
            "/no/such/close-evidence.md",
            "--spend",
            "not json",
        ]),
        "2026-01-01T00:00:00Z",
    )
    .expect_err("a missing evidence file was accepted")
    .envelope();
    assert_eq!(
        evidence_first["reason"], "comment-body-invalid",
        "the evidence comment did not run before the spend: {evidence_first}"
    );

    let spend_second = super::dispatch(
        &context,
        "unassign",
        &merged_release(&["--spend", "not json"]),
        "2026-01-01T00:00:00Z",
    )
    .expect_err("a malformed spend was accepted")
    .envelope();
    assert_eq!(
        spend_second["reason"], "malformed-argument",
        "the spend did not run before the release: {spend_second}"
    );
    assert_eq!(spend_second["argument"], "--spend", "{spend_second}");
}

/// A merged `claim` accepts the state it moves to and refuses a claim with no
/// state to move to exactly as the separate calls do.
///
/// The move is opt-in: without `--transition-to` the arm is the claim alone,
/// and the flag is read from the arm rather than invented. The floor is that
/// the argument reaches the dispatcher at all — a `--transition-to` it never
/// read would be a value lost in silence, which
/// `every_flag_a_tool_sends_is_one_the_dispatcher_reads` also crosses.
#[test]
fn a_merged_claim_reads_the_state_it_moves_to() {
    let root = tempfile::tempdir().expect("a directory");
    let context = context(root.path());
    let flags = [
        "--issue",
        "7",
        "--run-id",
        "claude-abcd1234",
        "--horizon",
        "2099-01-01T00:00Z",
        "--runtime",
        "claude",
        "--operation-id",
        "op",
        "--transition-to",
        "in-progress",
    ]
    .iter()
    .map(|part| (*part).to_owned())
    .collect::<Vec<_>>();

    // No `gh` on the path, so the claim itself cannot run; what is asserted is
    // that the dispatcher recognised the arm and asked the tracker rather than
    // reporting the argument unknown.
    let refused = super::dispatch(&context, "claim", &flags, "2026-01-01T00:00:00Z")
        .expect_err("a claim with no tracker answered");
    assert_ne!(
        refused.envelope()["reason"],
        "unknown-operation",
        "the merged claim arm is not dispatched: {}",
        refused.envelope()
    );
    assert_ne!(
        refused.envelope()["reason"],
        "missing-argument",
        "the merged claim dropped an argument it needs: {}",
        refused.envelope()
    );
}

#[test]
fn a_hand_finding_naming_a_lens_is_refused_and_a_judges_is_not() {
    let finding = [
        "--reviewer",
        "judge",
        "--id",
        "f",
        "--class",
        "warning",
        "--evidence",
        "seen",
        "--impact",
        "costs",
    ];
    let mut with_lens = finding.to_vec();
    with_lens.extend(["--lens", "review-risk"]);
    let refused = hand_call("review-finding", &with_lens);
    assert_eq!(refused["reason"], "recorded-by-capture-only", "{refused}");

    let root = tempfile::tempdir().expect("a scratch directory");
    let mut flags: Vec<String> = ["--issue", "7", "--run-id", "run", "--operation-id", "op"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    flags.extend(finding.iter().map(|part| (*part).to_owned()));
    let judged = super::dispatch(
        &context(root.path()),
        "review-finding",
        &flags,
        "2026-01-01T00:00:00Z",
    )
    .expect_err("a finding with no receipt answered");
    assert_ne!(
        judged.envelope()["reason"],
        "recorded-by-capture-only",
        "a judge's finding was refused as a lens's"
    );
}

/// Every `review_read` view is routed, and each names the argument it cannot do
/// without rather than falling through to another view.
///
/// The whole point of folding six reads behind one `--view` is that each is
/// still reachable; a view that quietly became a synonym for its neighbour, or
/// an unknown one answered as a read, is a name that decides nothing.
#[test]
fn review_read_routes_every_view_and_names_its_missing_argument() {
    let root = tempfile::tempdir().expect("a directory");
    let context = context(root.path());
    let call = |flags: &[&str]| -> serde_json::Value {
        super::dispatch(
            &context,
            "review-read",
            &flags
                .iter()
                .map(|part| (*part).to_owned())
                .collect::<Vec<_>>(),
            "2026-01-01T00:00:00Z",
        )
        .expect_err("a review_read with no answer")
        .envelope()
    };

    let bare = call(&["--issue", "7"]);
    assert_eq!(bare["reason"], "missing-argument", "{bare}");
    assert_eq!(bare["argument"], "--view", "{bare}");

    let invalid = call(&["--issue", "7", "--view", "nonsense"]);
    assert_eq!(invalid["reason"], "malformed-argument", "{invalid}");
    assert_eq!(invalid["argument"], "--view", "{invalid}");

    // One missing argument per view, each raised inside that view's own arm:
    // `verdict`'s receipt is demanded in order, `membership` needs its
    // candidate, `inspect` its epoch and request, and `spend` its phase.
    for (flags, argument) in [
        (vec!["--issue", "7", "--view", "verdict"], "--pr"),
        (
            vec!["--issue", "7", "--view", "verdict", "--pr", "7"],
            "--epoch",
        ),
        (
            vec![
                "--issue", "7", "--view", "verdict", "--pr", "7", "--epoch", "e",
            ],
            "--head",
        ),
        (
            vec![
                "--issue", "7", "--view", "verdict", "--pr", "7", "--epoch", "e", "--head", "h",
            ],
            "--base",
        ),
        (
            vec![
                "--issue", "7", "--view", "verdict", "--pr", "7", "--epoch", "e", "--head", "h",
                "--base", "b",
            ],
            "--digest",
        ),
        (vec!["--issue", "7", "--view", "membership"], "--candidate"),
        (vec!["--issue", "7", "--view", "inspect"], "--epoch"),
        (
            vec!["--issue", "7", "--view", "inspect", "--epoch", "e"],
            "--request",
        ),
        (vec!["--issue", "7", "--view", "spend"], "--phase"),
    ] {
        let envelope = call(&flags);
        assert_eq!(
            envelope["reason"], "missing-argument",
            "{flags:?}: {envelope}"
        );
        assert_eq!(envelope["argument"], argument, "{flags:?}: {envelope}");
    }

    // A phase this transport does not know is refused by name, and only the
    // spend arm can answer it.
    let unknown_phase = call(&[
        "--issue",
        "7",
        "--view",
        "spend",
        "--phase",
        "no-such-phase",
    ]);
    assert_eq!(
        unknown_phase["reason"], "invalid-spend-phase",
        "{unknown_phase}"
    );

    // `transaction` carries no required argument beyond `--issue`, so the only
    // proof it is routed is that it reaches its read rather than a view defect.
    // A `gh` that cannot answer is enough: what is asserted is which refusal,
    // not whether the read succeeds.
    let bin = crate::test_env::scripted_gh();
    let answers = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
    let routed = crate::test_env::with_scripted_gh(bin.path(), &answers, || {
        call(&["--issue", "7", "--view", "transaction"])
    });
    assert_ne!(routed["reason"], "missing-argument", "{routed}");
    assert_ne!(routed["reason"], "malformed-argument", "{routed}");
    assert_ne!(routed["reason"], "unknown-operation", "{routed}");
}

/// A flag that means something only with the mode that selects it is refused
/// when that mode is absent, rather than silently dropped.
///
/// `--force` without `--takeover`, `--clearing-finding` without
/// `--parent-head`, `--native-start`/`--manifest` with `--recorded-base`, and
/// `--state` beside `--identities` all name two different reads at once.
/// Answering one of them is the silent drop this crate refuses.
#[test]
fn a_mode_flag_given_without_its_mode_is_refused() {
    let root = tempfile::tempdir().expect("a directory");
    let context = context(root.path());
    let refuse = |operation: &str, flags: &[&str]| -> serde_json::Value {
        super::dispatch(
            &context,
            operation,
            &flags
                .iter()
                .map(|part| (*part).to_owned())
                .collect::<Vec<_>>(),
            "2026-01-01T00:00:00Z",
        )
        .expect_err("a mode flag without its mode answered")
        .envelope()
    };

    let cases: [(&str, &[&str], &str); 6] = [
        (
            "claim",
            &[
                "--issue",
                "7",
                "--run-id",
                "run",
                "--runtime",
                "claude",
                "--horizon",
                "2099-01-01T00:00Z",
                "--force",
            ],
            "--force",
        ),
        (
            "claim",
            &[
                "--issue",
                "7",
                "--run-id",
                "run",
                "--runtime",
                "claude",
                "--horizon",
                "2099-01-01T00:00Z",
                "--reason-file",
                "/no/such/reason",
            ],
            "--reason-file",
        ),
        (
            "review-tier",
            &["--base", "b", "--head", "h", "--clearing-finding", "severe"],
            "--clearing-finding",
        ),
        (
            "target",
            &[
                "--base",
                "b",
                "--recorded-base",
                "r",
                "--native-start",
                "/no/such",
            ],
            "--native-start",
        ),
        (
            "target",
            &["--base", "b", "--recorded-base", "r", "--manifest"],
            "--manifest",
        ),
        (
            "list-state",
            &["--run-id", "run", "--state", "review", "--identities"],
            "--state",
        ),
    ];
    for (operation, flags, argument) in cases {
        let envelope = refuse(operation, flags);
        assert_eq!(
            envelope["reason"], "malformed-argument",
            "{operation} {flags:?}: {envelope}"
        );
        assert_eq!(
            envelope["argument"], argument,
            "{operation} {flags:?}: {envelope}"
        );
    }
}

/// A hand call to a name only capture once wrote is refused as unknown, and
/// never reaches a record.
///
/// The three capture-only recorders and the aggregate verdict left the surface
/// in #1054. What must not survive that change is a hand call answered `ok`, or
/// one routed to an interpreter: the guarantee that these records are written
/// only by the capture, or not at all.
#[test]
fn a_hand_call_to_a_retired_capture_only_record_is_unknown() {
    let root = tempfile::tempdir().expect("a scratch directory");
    let context = context(root.path());
    for operation in [
        "review-lens-report",
        "review-validator-result",
        "review-refutation",
        "review-verdict",
    ] {
        let envelope = super::dispatch(
            &context,
            operation,
            &["--issue", "7", "--run-id", "run"]
                .iter()
                .map(|part| (*part).to_owned())
                .collect::<Vec<_>>(),
            "2026-01-01T00:00:00Z",
        )
        .expect_err("a retired capture-only record answered")
        .envelope();
        assert_eq!(
            envelope["reason"], "unknown-operation",
            "{operation}: {envelope}"
        );
        assert_eq!(envelope["operation"], operation, "{operation}: {envelope}");
    }
    assert_eq!(
        std::fs::read_dir(root.path())
            .expect("a readable scratch directory")
            .count(),
        0,
        "a refused hand call wrote something"
    );
}

#[test]
fn an_operation_naming_a_rekeyed_provisional_number_runs_against_the_real_one() {
    use crate::transport::store::{Repo, Store, store_path};
    let repo = Repo {
        host: "github.com".to_owned(),
        owner: "asanabrial".to_owned(),
        name: "estigia".to_owned(),
    };
    let mut store = Store::open(&store_path().expect("a path")).expect("opens");
    let provisional = store.allocate_provisional(&repo).expect("allocates");
    store
        .import(
            &repo,
            provisional,
            &serde_json::json!({ "body": "", "labels": [], "comments": [] }),
            "2026-01-01T00:00Z",
        )
        .expect("held");
    store.rekey(&repo, provisional, 55, None).expect("rekeys");
    drop(store);

    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    context.repo = Some("asanabrial/estigia".to_owned());
    let flags = ["--issue".to_owned(), provisional.to_string()];

    let answered =
        super::dispatch(&context, "tasks", &flags, "2026-01-01T00:00Z").expect("the tasks read");

    assert_eq!(answered["issue"], 55, "{answered}");
}
