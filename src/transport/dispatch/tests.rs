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
/// `Flags::read` walks pairs, and reading the word after every `--name` blindly
/// makes `--force --json` set `force` to the string `--json` **and** lose
/// `--json` entirely. Both halves are asserted, because a reader that only kept
/// the switch would still have eaten what followed it.
#[test]
fn a_switch_does_not_swallow_the_flag_after_it() {
    let flags: Vec<String> = ["--force", "--json", "--issue", "12", "--fix"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    let read = Flags::read(&flags);

    assert!(read.on("force"), "a switch in the middle went unseen");
    assert!(read.on("json"), "the flag after a switch was swallowed");
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
    let switches: std::collections::BTreeSet<&str> = source
        .match_indices(".on(\"")
        .filter_map(|(at, opener)| {
            let rest = &source[at + opener.len()..];
            rest.split_once('"').map(|(name, _)| name)
        })
        .collect();
    // The floor: five switches reach `on` today, and a scan that read none
    // would satisfy every assertion below without making one.
    assert!(
        switches.len() >= 5 && switches.contains("force"),
        "the dispatch was not read: {switches:?}"
    );

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
    let source = include_str!("../dispatch.rs");
    let shipped = source
        .find("#[cfg(test)]")
        .map_or(source, |at| &source[..at]);

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
    // Flags an arm reads that are not a caller's to send, with why.
    //
    // Every entry is an operator switch or a value derived from the checkout. A
    // flag that changes what the operation *checks* does not belong here — that
    // is `expect-state`, and leaving it off the tool is how a run already in
    // `review` was left unable to publish at all.
    const OPERATOR_ONLY: &[(&str, &str, &str)] = &[
        (
            "create",
            "no-cache",
            "the operator's switch for the board mirror",
        ),
        (
            "transition",
            "no-cache",
            "the operator's switch for the board mirror",
        ),
        (
            "audit-board",
            "no-cache",
            "the operator's switch for the board mirror",
        ),
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
            if COMMON.contains(&wanted) || MINTED.contains(&wanted) {
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
/// nothing — so `Board::parse("")` turned the **board mirror off for every run
/// on the machine**, and `start-branch` read an empty worktree template
/// whatever the operator had set.
///
/// Nothing failed. That is what makes it worth a test of its own: the module's
/// own header says what is not allowed is *not trying*, and a mirror that never
/// fires reports exactly what a mirror with nothing to do reports.
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
            "# Contract\n\n{}\n| Setting | Value |\n|---|---|\n| Project board | asanabrial/12 |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("a contract");

    let live =
        crate::transport::Context::live(root.path().to_path_buf(), root.path().to_path_buf(), None);
    assert_eq!(
        live.get("project board"),
        Some("asanabrial/12"),
        "a live context did not read the operator's table: {:?}",
        live.config
    );

    // And the half that turns a read row into a working mirror, because reading
    // it and using it are different facts.
    let board = crate::transport::board::Board::parse(
        live.get("project board").unwrap_or_default(),
        &live,
        false,
    );
    assert!(board.enabled, "the board the operator configured is off");

    // The floor, and the shape of the defect: the same call with the empty table
    // the two callers used to build answers the same as one with no board at all.
    let empty = crate::transport::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    assert!(
        !crate::transport::board::Board::parse(
            empty.get("project board").unwrap_or_default(),
            &empty,
            false,
        )
        .enabled,
        "an empty table no longer disables the mirror, so this test measures nothing"
    );

    // And that the live callers build it that way. A context assembled by hand
    // beside this constructor is how the empty one got there in the first place.
    //
    // By walking the crate, not by naming files. This named two of them â€”
    // `harness/mod.rs` and `harness/mcp/mod.rs`, the two that had the defect â€”
    // and there was a third: `tui.rs` filled the board picker through a context
    // it built by hand with an empty table. A guard that lists the places it has
    // already been burned by cannot see the next one, and reads like coverage
    // while it does it.
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
            walked += 1;
            let shipped = source
                .find("#[cfg(test)]\nmod tests {")
                .map_or(source.as_str(), |end| &source[..end]);
            // The literal in code, not in prose. `Context::live`'s own doc
            // comment recounts the incident by quoting the line, and a guard
            // that cannot tell an explanation from a construction would make
            // writing the explanation down the thing that fails.
            let constructed = shipped.lines().any(|line| {
                !line.trim_start().starts_with("//") && line.contains("config: Vec::new()")
            });
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
