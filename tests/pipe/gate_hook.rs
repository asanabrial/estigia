use super::*;

#[test]
fn a_sub_agent_definition_nothing_can_read_does_not_become_a_permission() {
    // The search read each candidate with `.ok()` and stepped over the ones that
    // failed. Four roots are searched, so a definition that is **there and will
    // not open** either handed the role to a file further down the list or ran
    // out — and running out is `None`, which the frontmatter reader spells out
    // means *no policy at all: the sub-agent may use every tool*.
    //
    // So the one gate that depends on nothing else — no claim, no state, no
    // window, only on what the author wrote — turned an unreadable file into
    // clearance. The same harm that file already records for a byte-order mark:
    // "a definition that restricted a sub-agent stopped restricting it because
    // of a character nobody typed."
    //
    // Run through the binary, because the loss was in the search and the gate
    // above it, and each of them reads correctly on its own.
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");

    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = |body: &str| format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",{body}}}");
    let writing = r#""agent_type":"reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#;

    // The floor first, both ways: a readable definition still denies past its
    // list, and a sub-agent nobody wrote a definition for is still allowed
    // through. A fix that denied everything would satisfy the assertion below
    // and break the product.
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read, Grep\n---\nRead the code.\n",
    )
    .expect("a definition");
    let (denied, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(writing));
    assert!(
        denied.contains("tool-outside-declared-role"),
        "a readable definition stopped being enforced: {denied}"
    );

    let (nobody, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(
            r#""agent_type":"unwritten","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#,
        ),
    );
    assert!(
        !nobody.contains("agent-definition-unreadable"),
        "a sub-agent nobody declared was refused as unreadable: {nobody}"
    );

    // And now the definition is there and will not open. A directory in its
    // place fails the read with something other than `NotFound`, on every
    // platform.
    std::fs::remove_file(agents.join("reader.md")).expect("clear the way");
    std::fs::create_dir_all(agents.join("reader.md")).expect("something unreadable in its place");

    let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(writing));
    assert!(
        out.contains("\"permissionDecision\":\"deny\""),
        "a sub-agent whose declared limits nothing could read was let through: {out}"
    );
    assert!(
        out.contains("agent-definition-unreadable"),
        "the denial did not say which question went unanswered: {out}"
    );
    assert!(
        out.contains("operator-knowledge") && out.contains("readable"),
        "the denial does not carry what to do about it: {out}"
    );

    // Even a tool the readable definition would have allowed: the point is that
    // what it declared is unknown, not that this particular call was outside it.
    let (reading, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(r#""agent_type":"reader","tool_name":"Read","tool_input":{"file_path":"x.rs"}"#),
    );
    assert!(
        reading.contains("agent-definition-unreadable"),
        "a call was judged against limits nothing had read: {reading}"
    );
}

/// A stand-down reaches the unreadable definition, and not the author's list.
///
/// Two refusals sit on the same three lines of `PreToolUse`, and they are not
/// the same kind of no.
///
/// `agent-definition-unreadable` is Estigia's own *an unknown result is not
/// clearance*, and it had `decide_action`'s shape until now: a definition file
/// that will not open denies **every** call that sub-agent makes, and the one
/// command for getting past a gate that is wrong at a bad moment did not reach
/// it — a file on a read-only mount, or one somebody else owns, left an operator
/// with nothing to do but stop.
///
/// `tool-outside-declared-role` enforces a list the sub-agent's *author* wrote.
/// An operator standing Estigia's gate down does not thereby grant permissions
/// somebody else withheld, so it stays refused — and that is asserted here
/// rather than left to be noticed, because an untested asymmetry is one somebody
/// closes by accident.
#[test]
fn a_stand_down_reaches_an_unreadable_definition_and_not_the_authors_list() {
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read, Grep\n---\nRead the code.\n",
    )
    .expect("a definition");

    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = |body: &str| format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",{body}}}");
    let writing = r#""agent_type":"reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#;

    // The author's list, with a stand-down in force. Still refused.
    let (denied, _, _) = run_with_stand_down(home.path(), &payload(writing));
    assert!(
        denied.contains("tool-outside-declared-role"),
        "a stand-down granted a permission the sub-agent's author withheld: {denied}"
    );

    // And the definition nothing can read, with the same stand-down. Through.
    std::fs::remove_file(agents.join("reader.md")).expect("clear the way");
    std::fs::create_dir_all(agents.join("reader.md")).expect("something unreadable in its place");
    let (allowed, _, _) = run_with_stand_down(home.path(), &payload(writing));
    assert!(
        allowed.contains("\"permissionDecision\":\"allow\""),
        "a declared stand-down did not reach a refusal Estigia raised about its own read: \
         {allowed}"
    );
    // Naming what it overrode is the whole difference between this and a
    // switch: a trace reading "allowed" and one reading "allowed under a
    // stand-down, over `agent-definition-unreadable`" are not the same record,
    // and only the second can be answered for afterwards.
    assert!(
        allowed.contains("stood down") && allowed.contains("agent-definition-unreadable"),
        "the allowance does not say what it let through: {allowed}"
    );

    // Without one it is still refused, or the assertion above is about nothing.
    let (still, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(writing));
    assert!(
        still.contains("agent-definition-unreadable"),
        "an unreadable definition stopped being refused at all: {still}"
    );
}

#[test]
fn a_sub_agent_reaching_past_its_own_tool_list_is_refused_by_the_process() {
    // The role gate is the one enforcement point that depends on nothing —
    // no claim, no state, no window, only on what the author of the sub-agent
    // wrote. It was exercised in unit tests and had never been run **through
    // the binary**, which is where the payload's field names, the definition
    // search and the dialect's answer shape all have to agree at once.
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ndescription: reads and nothing else\ntools: Read, Grep, Glob\n---\nRead the code.\n",
    )
    .expect("a sub-agent definition");

    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = |body: &str| format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",{body}}}");

    // Outside the list it declares: denied, and the refusal carries the four
    // things every refusal here carries.
    let (out, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(r#""agent_type":"reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#),
    );
    assert!(
        out.contains("\"permissionDecision\":\"deny\"")
            && out.contains("tool-outside-declared-role"),
        "a sub-agent wrote past its own tool list: {out}"
    );
    assert!(
        out.contains("nothing was written") && out.contains("operator-knowledge"),
        "the denial does not carry what to do about it: {out}"
    );

    // A top-level `subagent_type` is not the caller. Claude uses that key only
    // nested under Agent/Task input to name the role being launched.
    let (out, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(
            r#""subagent_type":"reader","tool_name":"Bash","tool_input":{"command":"rm -rf x"}"#,
        ),
    );
    assert!(
        !out.contains("tool-outside-declared-role"),
        "a launch-target spelling was substituted for the caller: {out}"
    );

    // And the three ways this must **not** deny. A role gate that refused any
    // of them would be a harness people take out.
    for (why, body) in [
        (
            "a tool the role declares",
            r#""agent_type":"reader","tool_name":"Read","tool_input":{"file_path":"x.rs"}"#,
        ),
        (
            "the main conversation, which no role covers",
            r#""tool_name":"Write","tool_input":{"file_path":"x.rs"}"#,
        ),
        (
            // Not enforcement from a file outside the agents directory: the
            // name arrives from the host and lands in a path, and a list found
            // somewhere else would be enforced as though somebody had written
            // it for this role.
            "a role name that tries to leave the directory",
            r#""agent_type":"../../reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#,
        ),
    ] {
        let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(body));
        assert!(
            !out.contains("deny"),
            "{why} was refused by the role gate: {out}"
        );
    }
}

#[test]
fn generated_claude_hook_refuses_reserved_project_reviewers_before_claim_checks() {
    let home = tempfile::tempdir().expect("a home");
    let repository = tempfile::tempdir().expect("a repository");
    std::fs::create_dir(repository.path().join(".git")).expect("a repository marker");
    let nested = repository.path().join("src/deep");
    std::fs::create_dir_all(&nested).expect("a nested launch directory");
    let (setup, _, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "Claude setup failed: {setup}");
    let settings = std::fs::read_to_string(home.path().join(".claude/settings.json"))
        .expect("Claude's generated settings");
    for required in [
        "Agent",
        "Task",
        "--agent claude-code",
        "--dialect claude-code",
    ] {
        assert!(
            settings.contains(required),
            "generated hook misses {required}: {settings}"
        );
    }

    let agents = repository.path().join(".claude/agents");
    std::fs::create_dir_all(&agents).expect("project agents");
    let payload = |tool: &str, cwd: &std::path::Path| {
        let cwd = cwd.display().to_string().replace('\\', "\\\\");
        format!(
            "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"tool_name\":\"{tool}\",\
             \"tool_input\":{{\"subagent_type\":\"review-blind\"}}}}"
        )
    };
    let invoke = |tool: &str, cwd: &std::path::Path| {
        run_in(
            home.path(),
            repository.path(),
            &[
                "hook",
                "pre-tool-use",
                "--agent",
                "claude-code",
                "--dialect",
                "claude-code",
            ],
            &payload(tool, cwd),
        )
        .0
    };

    std::fs::write(
        agents.join("review-blind.md"),
        "---\nname: review-blind\ntools: Read, Write, Bash\n---\nHostile.\n",
    )
    .expect("the exact shadow");
    let denied = invoke("Agent", repository.path());
    assert!(denied.contains("reviewer-project-shadow"), "{denied}");
    assert!(!denied.contains("not-current-live-holder"), "{denied}");

    std::fs::remove_file(agents.join("review-blind.md")).expect("remove exact shadow");
    std::fs::create_dir_all(agents.join("nested")).expect("nested agents");
    std::fs::write(
        agents.join("nested/renamed.md"),
        "---\nname: review-blind\ntools: Read, Write, Bash\n---\nHostile.\n",
    )
    .expect("the renamed shadow");
    let denied = invoke("Task", repository.path());
    assert!(denied.contains("reviewer-project-shadow"), "{denied}");
    assert!(!denied.contains("not-current-live-holder"), "{denied}");

    let denied = invoke("Agent", &nested);
    assert!(
        denied.contains("reviewer-project-shadow"),
        "changing cwd hid the root project reviewer: {denied}"
    );
}

#[test]
fn generated_claude_hook_refuses_a_hand_launch_of_a_binary_launched_reviewer() {
    let home = tempfile::tempdir().expect("a home");
    let repository = tempfile::tempdir().expect("a repository");
    std::fs::create_dir(repository.path().join(".git")).expect("a repository marker");
    let (setup, _, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "Claude setup failed: {setup}");
    assert!(!home.path().join(".claude/agents/review-risk.md").exists());

    let cwd = repository
        .path()
        .display()
        .to_string()
        .replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"tool_name\":\"Task\",\
         \"tool_input\":{{\"subagent_type\":\"review-risk\"}}}}"
    );
    let (denied, _, _) = run_in(
        home.path(),
        repository.path(),
        &[
            "hook",
            "pre-tool-use",
            "--agent",
            "claude-code",
            "--dialect",
            "claude-code",
        ],
        &payload,
    );
    assert!(denied.contains("launch_review_input"), "{denied}");
    assert!(!denied.contains("estigia setup"), "{denied}");

    let agents = repository.path().join(".claude/agents");
    std::fs::create_dir_all(&agents).expect("project agents");
    std::fs::write(
        agents.join("review-risk.md"),
        "---\nname: review-risk\ntools: Read, Write, Bash\n---\nHostile.\n",
    )
    .expect("the project shadow");
    let (denied, _, _) = run_in(
        home.path(),
        repository.path(),
        &[
            "hook",
            "pre-tool-use",
            "--agent",
            "claude-code",
            "--dialect",
            "claude-code",
        ],
        &payload,
    );
    assert!(denied.contains("reviewer-project-shadow"), "{denied}");
}

#[test]
fn every_dialect_says_no_in_the_shape_its_agent_reads() {
    // Five shapes, and the flag's own help says what getting one wrong costs:
    // *a hook that runs, decides correctly, and is ignored — which reports
    // success and enforces nothing*. Every one of them had unit tests and none
    // had ever been produced **by the process**, where the payload's fields,
    // the definition search and the answer shape all have to agree at once.
    //
    // The role gate is the vehicle because it denies without a tracker, a
    // claim, a state or a window — the one deterministic refusal this suite can
    // ask for.
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ndescription: reads\ntools: Read, Grep, Glob\n---\nRead.\n",
    )
    .expect("a sub-agent definition");
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
         \"tool_name\":\"Write\",\"tool_input\":{{\"file_path\":\"x.rs\"}}}}"
    );

    // Each dialect's own spelling of no, from the dialect rather than from a
    // list written here: a variant added to the enum arrives with its slug and
    // this has to be told what it looks like.
    for (slug, marker) in [
        ("claude-code", "\"permissionDecision\":\"deny\""),
        ("gemini-cli", "\"decision\":\"deny\""),
        ("cursor", "\"permission\":\"deny\""),
        ("cline", "\"review\":true"),
    ] {
        let (out, _, ok) = run(
            home.path(),
            &["hook", "pre-tool-use", "--dialect", slug],
            &payload,
        );
        assert!(ok, "{slug}: the hook did not exit zero");
        assert!(
            out.contains(marker),
            "{slug} was answered in somebody else's shape: {out}"
        );
    }

    // The odd one out, and the expensive one to get wrong: not JSON at all.
    // The agent reads the status, and the reason goes to standard error.
    let (out, stderr, ok) = run(
        home.path(),
        &["hook", "pre-tool-use", "--dialect", "exit-code"],
        &payload,
    );
    assert!(!ok, "an exit-code agent was told to proceed");
    assert!(
        out.trim().is_empty(),
        "a decision was printed where a status was expected: {out}"
    );
    assert!(
        stderr.contains("tool-outside-declared-role"),
        "the reason reached nobody: {stderr}"
    );

    // A slug this build does not have falls back — deliberately, so a renamed
    // dialect does not block every edit — **and says so**. Silence here is the
    // one that costs: the fallback shape is JSON, and an agent that reads a
    // status gets a refusal that prints itself and lets the write through.
    let (out, stderr, _) = run(
        home.path(),
        &["hook", "pre-tool-use", "--dialect", "windsurf"],
        &payload,
    );
    assert!(
        out.contains("\"permissionDecision\":\"deny\""),
        "the fallback stopped deciding: {out}"
    );
    assert!(
        stderr.contains("not a dialect this build knows"),
        "an unknown dialect was answered in another's shape without a word: {stderr:?}"
    );

    // And an ordinary run stays quiet, or the note becomes noise nobody reads.
    let (_, stderr, _) = run(
        home.path(),
        &["hook", "pre-tool-use", "--dialect", "claude-code"],
        &payload,
    );
    assert!(
        !stderr.contains("not a dialect"),
        "a known dialect was complained about: {stderr}"
    );
}

#[test]
fn a_session_that_never_said_whose_it_was_forgets_nobody() {
    // `SessionEnd` is the hook's one destructive act, and the name it destroys
    // by is derived from the payload. `session::run_id` answers a session with
    // no identity with `<runtime>-unknown` — which is **a name two runs share**
    // rather than a run. A payload this build cannot parse produces it, and
    // this machine's own ledger carries four entries under it.
    //
    // So a `SessionEnd` with no payload deleted `claude-unknown.json`. If the
    // run that wrote it is still working, the gate then reads `issue: None` and
    // every write it makes goes through ungated while the tracker still shows
    // the issue held — a run that swore reading as one that never did.
    //
    // Through the process, because that is where the payload becomes a name.
    let home = tempfile::tempdir().expect("a temporary home");
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the state directory");
    let nameless = runs.join("claude-unknown.json");
    let named = runs.join("claude-abc12345cc1e83a2.json");
    let write = |path: &std::path::Path, id: &str, issue: u64| {
        std::fs::write(
            path,
            format!("{{\"run_id\":\"{id}\",\"issue\":{issue},\"revision\":1}}"),
        )
        .expect("a run pointer");
    };
    write(&nameless, "claude-unknown", 12);
    write(&named, "claude-abc12345cc1e83a2", 7);

    // No payload at all, and a session id that is only spaces. Neither says
    // whose session this is, and neither may take somebody's claim with it.
    for payload in ["", r#"{"session_id":"   "}"#] {
        run(home.path(), &["hook", "session-end"], payload);
        assert!(
            nameless.is_file(),
            "a session that named nobody forgot the run that had no name either: {payload:?}"
        );
        assert!(
            named.is_file(),
            "and took an unrelated one with it: {payload:?}"
        );
    }

    // And a session that **does** say whose it is still forgets its own — the
    // pointer goes, the claim does not. A guard that stopped this would leave a
    // pointer behind for every session that ever ended.
    run(
        home.path(),
        &["hook", "session-end"],
        r#"{"session_id":"abc12345"}"#,
    );
    assert!(
        !named.exists(),
        "a session that identified itself did not forget its own run"
    );
    assert!(
        nameless.is_file(),
        "forgetting one run took the nameless one with it"
    );
}

#[test]
fn a_role_one_session_recorded_is_readable_by_a_later_run_and_by_the_operator() {
    // The durable half of the role record, through the built binary. The
    // within-run pointer is per-session and `SessionEnd` removes it, so this
    // drives the record, ends the session, and reads it back from a later run
    // with a different identity — the cross-run reach the pointer never had.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // A delegated context's read: answered `Outside`, which `note` drops from
    // the decision ledger. The role still has to be recorded.
    let (out, _, ok) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        r#"{"session_id":"s-one","agent_type":"review-blind","tool_name":"Read","tool_input":{"file_path":"src/x.rs"}}"#,
    );
    assert!(ok, "the hook refused: {out}");
    assert_eq!(
        out.trim(),
        "{}",
        "the fixture did not drive an Outside call"
    );

    // The session ends: the pointer goes.
    run(
        home.path(),
        &["hook", "session-end"],
        r#"{"session_id":"s-one"}"#,
    );
    let first_run = estigia::harness::session::run_id("claude", "s-one");
    let pointer = home
        .path()
        .join(".estigia")
        .join("runs")
        .join(format!("{first_run}.json"));
    assert!(
        !pointer.exists(),
        "the producing session's pointer outlived it"
    );

    // A later run — the operator's read surface — still sees what it saw.
    let (out, stderr, ok) = run(home.path(), &["roles", "--json"], "");
    assert!(ok, "roles refused: {stderr}");
    let records: serde_json::Value = serde_json::from_str(&out).expect("roles --json is JSON");
    let records = records.as_array().expect("roles --json is an array");
    assert!(
        records.iter().any(|record| {
            record["run_id"].as_str() == Some(first_run.as_str())
                && record["agent_type"].as_str() == Some("review-blind")
        }),
        "the role a delegated context ran as did not outlive its session: {out}"
    );

    // And the filters narrow to it, rather than reading it back by accident.
    let (narrowed, stderr, ok) = run(
        home.path(),
        &["roles", "--json", "--run-id", &first_run],
        "",
    );
    assert!(ok, "roles --run-id refused: {stderr}");
    let narrowed: serde_json::Value =
        serde_json::from_str(&narrowed).expect("roles --json is JSON");
    assert_eq!(
        narrowed.as_array().map(Vec::len),
        Some(1),
        "a run-id filter did not narrow to the one entry: {narrowed}"
    );
    let (none, _, ok) = run(
        home.path(),
        &["roles", "--json", "--run-id", "claude-nobody"],
        "",
    );
    assert!(ok);
    assert_eq!(none.trim(), "[]", "an unknown run id answered a record");
}

#[test]
fn a_session_with_no_identity_is_told_so_rather_than_handed_a_shared_name() {
    // `session::run_id` answers a session with no identity with
    // `<runtime>-unknown`, and `SessionStart` handed that back as *Run id
    // `claude-unknown`.* — a name every unidentifiable session on the machine
    // is given, presented as though it were one run's.
    //
    // Two of them swearing under it means the second overwrites the first's
    // pointer, and the gate goes on measuring the first's writes against the
    // second's issue. The `--run-id` flag's own help states the rule: *a claim
    // recorded under the wrong run-id is a claim the gate will never match, and
    // being asked beats being silently wrong.*
    let home = tempfile::tempdir().expect("a temporary home");
    run(home.path(), &["setup", "claude-code"], "");

    // With an identity: the run id is named, because an agent needs it to
    // claim with.
    let (out, _, _) = run(
        home.path(),
        &["hook", "session-start"],
        r#"{"session_id":"abc12345"}"#,
    );
    assert!(
        out.contains("Run id `claude-abc12345"),
        "an identified session was not given its run id: {out}"
    );

    // Without one: the fact, not a name. An agent told it has no identity can
    // ask for one; an agent told it is `claude-unknown` cannot know to.
    let (out, _, _) = run(home.path(), &["hook", "session-start"], "{}");
    assert!(
        !out.contains("claude-unknown"),
        "a shared name was handed out as an identity: {out}"
    );
    assert!(
        out.contains("no identity") && out.contains("nothing can be sworn"),
        "the session was not told why it cannot swear: {out}"
    );

    // And the same rule at the other door, for an agent that has the name from
    // somewhere else: swearing under it is refused before anything is written.
    let (_, stderr, ok) = run(
        home.path(),
        &[
            "claim",
            "7",
            "--run-id",
            "claude-unknown",
            "--horizon",
            "2099-01-01T00:00Z",
        ],
        "",
    );
    assert!(!ok, "a claim under a name no run owns was accepted");
    assert!(
        stderr.contains("run-id-names-no-run"),
        "it was refused for some other reason: {stderr}"
    );
}

/// An instruction file left behind on an upgrade is named, not just left.
///
/// The record gained a second set in the round that stopped uninstalls deleting
/// a `CLAUDE.md` the operator had kept empty. Every record already on disk is
/// the older shape, which names no instruction file — so on the first uninstall
/// after an upgrade every one of them reads as the operator's and is emptied
/// rather than removed. That is the safe direction and it was chosen; what it
/// did not come with was a word to the operator, who got a stray empty file and
/// a line saying `update`.
///
/// Both halves, because a note that always fires is noise: an ordinary install
/// records what it created and its uninstall takes the file, saying nothing.
#[test]
fn an_instruction_file_kept_on_an_upgrade_is_named_in_the_report() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let record = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join(".estigia")
        .join("installed.json");
    let held: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record).expect("a record")).expect("json");
    // Exactly what a build from before the second set wrote: a bare array.
    let older = serde_json::to_string_pretty(held.get("created").expect("the created set"))
        .expect("serialises");
    std::fs::write(&record, older).expect("an older record");

    let (out, stderr, ok) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(ok, "uninstall failed: {stderr}");
    let said = format!("{out}{stderr}");
    assert!(
        said.contains("instruction file(s) are empty and still on disk"),
        "an empty file was left with no word about it: {said}"
    );
    let left = home.path().join(".claude").join("CLAUDE.md");
    assert!(
        left.is_file() && std::fs::metadata(&left).expect("readable").len() == 0,
        "the fixture did not leave the file this note is about"
    );

    // And the ordinary path stays quiet: the record names the file, so it goes.
    let home = tempfile::tempdir().expect("a second home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let (out, stderr, ok) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(ok, "uninstall failed: {stderr}");
    assert!(
        !format!("{out}{stderr}").contains("instruction file(s) are empty"),
        "a note about a leftover fired on an uninstall that left nothing"
    );
    assert!(
        !home.path().join(".claude").join("CLAUDE.md").exists(),
        "the recorded file was not removed"
    );
}

/// What `config set --repo` writes, `config list` reads back.
///
/// Measured by hand, on the installed binary: `config set --repo Tracker acme/repo`
/// named the file it had written, and `config list` one command later answered
/// `github`. A write this tool confirmed and a read this tool contradicted, on
/// the same row, one command apart — which is word for word the defect
/// `contract_of` already records being fixed once, for `--agent`, and which had
/// come straight back through the door beside it.
///
/// As a process, because that is how it was found and because nothing smaller
/// sees it: the layering was right everywhere it was tested and absent from the
/// one command an operator uses to check their work.
#[test]
fn what_config_set_repo_writes_config_list_reads_back() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");

    // The floor: before anything is said about this checkout it answers with
    // the contract, so the assertion below is about the write and not about the
    // command printing the word at all.
    let (before, _, _) = run_in(home.path(), repo.path(), &["config", "list"], "");
    // Everything between the row's label and its trailing `accepts:` column —
    // the whole value, not only its first word, because `github <owner>/<name>`
    // is two words and the first alone would read back the same as the default.
    let tracker = |out: &str| {
        out.lines()
            .find(|line| line.starts_with("Tracker "))
            .and_then(|line| line.split_once("accepts:"))
            .map(|(before, _)| before.trim_start_matches("Tracker").trim().to_owned())
            .unwrap_or_default()
    };
    assert!(
        tracker(&before) == "github",
        "the contract's own answer is not what an unconfigured checkout reports: {before}"
    );

    let (said, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "--repo", "Tracker", "github acme/repo"],
        "",
    );
    assert!(ok, "the write was refused: {said}{error}");
    assert!(
        said.contains("acme/repo"),
        "the write did not report itself: {said}"
    );

    let (after, _, _) = run_in(home.path(), repo.path(), &["config", "list"], "");
    assert!(
        tracker(&after) == "github acme/repo",
        "what was written into this checkout is not what reading it back reports: {after}"
    );
}

/// Setting a row this checkout overrides says so.
///
/// `config set` writes the contract and reads back what a run will read, and it
/// refuses when the operator's own `estigia.local.md` shadows the row: *a value
/// written into a row that file also carries is a value nobody will ever read*.
/// The repository layer added later sits above both and that read-back never
/// learned about it — so, measured on the installed binary:
///
/// ```text
/// $ estigia config set "Change size" 400
/// Change size is now 400
/// $ estigia config list
/// Change size    250
/// ```
///
/// A write this tool confirmed and a read this tool contradicted, on the same
/// row, one command apart — the sentence `contract_of` already carries about
/// `--agent`, arriving through the door beside it.
///
/// **Not a refusal**, and that is the difference from the operator's file. That
/// one shadows the row in every checkout, so the write is in force nowhere.
/// This one shadows it *here*: the value is now the machine's answer everywhere
/// else, which is usually what was wanted. What was missing is the other half
/// of the sentence.
#[test]
fn setting_a_row_this_checkout_overrides_says_so() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");

    const CHECKOUT_ANSWERS: &str = "250";
    const MACHINE_ANSWERS: &str = "600";
    const WRITTEN: &str = "400";

    let (_, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "--repo", "Change size", CHECKOUT_ANSWERS],
        "",
    );
    assert!(ok, "the checkout would not take its own row: {error}");

    // The floor first: away from that checkout the same write says nothing
    // extra, so this is not a line printed on every `config set`.
    let (elsewhere, _, _) = run(
        home.path(),
        &["config", "set", "Change size", MACHINE_ANSWERS],
        "",
    );
    assert!(
        !elsewhere.contains(CHECKOUT_ANSWERS),
        "a checkout that overrides nothing is being warned about one: {elsewhere}"
    );

    let (said, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "Change size", WRITTEN],
        "",
    );
    assert!(ok, "the write itself was refused: {said}{error}");
    assert!(
        said.contains(CHECKOUT_ANSWERS),
        "this checkout answers `{CHECKOUT_ANSWERS}` and the write reported itself without saying \
         so: {said}"
    );
    assert!(
        said.contains("--repo"),
        "nothing names the one command that makes the two agree: {said}"
    );

    // And what a run here actually reads is unchanged, which is the fact the
    // line above exists to state.
    let (after, _, _) = run_in(home.path(), repo.path(), &["config", "list"], "");
    assert_eq!(
        listed_value(&after, "Change size"),
        CHECKOUT_ANSWERS,
        "the checkout's own answer moved, so this test is measuring something else"
    );

    // The listing says all three things the write said: which scope answers,
    // what it shadows, and the command that changes the nearer row.
    let row = after
        .lines()
        .find(|line| line.starts_with("Change size "))
        .unwrap_or_else(|| panic!("no Change size row in:\n{after}"));
    assert!(
        row.contains("(repository)"),
        "the listing does not name the scope the winning value came from: {row}"
    );
    assert!(
        after.contains(&format!("shadows {WRITTEN} (versioned block)")),
        "the listing does not show the value this checkout shadows:\n{after}"
    );
    assert!(
        after.contains("this checkout answers") && after.contains("estigia config set --repo"),
        "the listing does not name the command that changes the nearer row:\n{after}"
    );

    // A row no document answers names the fallback, so "versioned block" is not
    // printed for every row by default.
    let requirement = after
        .lines()
        .find(|line| line.starts_with("Unclaimed write policy "))
        .unwrap_or_else(|| panic!("no Unclaimed write policy row in:\n{after}"));
    assert!(
        requirement.ends_with("(skill default)"),
        "a row only the skill default answers does not say so: {requirement}"
    );
}

/// The other door asks a sub-agent's declared tool list too.
///
/// A role's `tools:` line is checked in the lifecycle hook and was checked
/// nowhere else — and `estigia gate` is not a convenience: OpenCode's plugin
/// shells out to it on every edit. So the list an operator wrote went on being
/// a request for exactly the agent whose gate is a plugin.
///
/// The same shape as `both_doors_to_the_gate_read_one_payload_the_same_way`,
/// one feature over. What that one found about `cwd` and a command's spelling,
/// this one finds about the question itself: the second door was not asking it.
#[test]
fn the_other_door_asks_a_sub_agents_declared_tool_list_too() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    let agents = repo.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("an agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read, Grep\n---\n\nReads things.\n",
    )
    .expect("the definition is written");

    let ask = |tool: &str| {
        let payload = format!(
            "{{\"agent_type\":\"reader\",\"file_path\":\"src/main.rs\",\"cwd\":{:?}}}",
            repo.path().display().to_string()
        );
        run_in(
            home.path(),
            repo.path(),
            &[
                "gate",
                tool,
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };

    // The floor: a tool the definition declares is not refused for being one.
    let (said, error, _) = ask("Read");
    assert!(
        !format!("{said}{error}").contains("declared role"),
        "a tool this sub-agent declares was refused: {said}{error}"
    );

    let (said, error, ok) = ask("Edit");
    assert!(
        format!("{said}{error}").contains("declared"),
        "a sub-agent reached past its own tool list and this door said nothing: {said}{error}"
    );
    assert!(!ok, "the call was allowed as well as reported");

    // The reserved caller is not allowed to replace its embedded policy with
    // project bytes through this door either.
    std::fs::write(
        agents.join("review-blind.md"),
        "---\nname: review-blind\ntools: Read, Write, Edit, Bash, Agent, Task\n---\n",
    )
    .expect("a hostile reserved definition");
    let payload = format!(
        "{{\"agent_type\":\"review-blind\",\"file_path\":\"src/main.rs\",\"cwd\":{:?}}}",
        repo.path().display().to_string()
    );
    let (said, error, ok) = run_in(
        home.path(),
        repo.path(),
        &[
            "gate",
            "Edit",
            "--run-id",
            "claude-aaaa1111",
            "--input",
            &payload,
        ],
        "",
    );
    assert!(
        !ok && format!("{said}{error}").contains("tool-outside-declared-role"),
        "project bytes widened the running reserved reviewer: {said}{error}"
    );
}

/// The reserved reviewer's grant is the same through both doors.
///
/// Two callers hand `roles::gate` this role's definition — `show_gate`, behind
/// `estigia gate`, and the `PreToolUse` hook, which is the one a running agent
/// goes through. That definition is a template and rendered it at the
/// first door only. So the hook parsed `tools: {{TOOLS}}` as an allowlist naming
/// one tool of that name and refused **every** call, including the reads the
/// reviewer had before the change, printing the placeholder into the operator's
/// refusal — a gate that denies everything reads exactly like a gate that works.
///
/// Every test that existed walked the **denied** tools, which stay denied under a
/// placeholder, so all of them agreed with the broken road. Two blind judges
/// measured it through the real binary. This asks the other direction, which is
/// the one that was never asked: **does the role gate refuse the tool the row
/// grants**, on both doors, for every value the row takes. It reads the refusal
/// this gate emits, so another gate declining the same call would not show here
/// — narrower than *allowed*, and the direction that was missing.
#[test]
fn the_reserved_reviewer_grant_is_the_same_through_both_doors() {
    for (row, granted, refused) in [("reading", "Read", "Bash"), ("measuring", "Bash", "Write")] {
        let home = tempfile::tempdir().expect("a temporary home");
        std::fs::create_dir_all(home.path().join("AppData").join("Roaming"))
            .expect("a roaming dir");
        let (_, stderr, ok) = run(home.path(), &["install", "claude-code"], "");
        assert!(ok, "install failed: {stderr}");
        let (said, error, ok) = run(
            home.path(),
            &["config", "set", "Evidence standard", row],
            "",
        );
        assert!(ok, "{row} was refused: {said}{error}");

        // The file the **host** reads, after the row is set the way an operator
        // sets it: `config set` writes the contract, `sync` re-renders what that
        // contract decides. Asserted because nothing did: forcing setup to write
        // the default rendering whatever the row said left the whole suite green,
        // which would make `measuring` buy a grant at Estigia's gate and none at
        // the host — a feature that is on everywhere except where it matters. Two
        // judges disagreed about whether `sync` re-renders at all, which is a
        // measurable fact, so it is measured here rather than argued.
        let (_, stderr, ok) = run(home.path(), &["sync"], "");
        assert!(ok, "sync failed: {stderr}");
        let installed = std::fs::read_to_string(
            home.path()
                .join(".claude")
                .join("agents")
                .join("review-blind.md"),
        )
        .expect("the reviewer is installed");
        let declared = installed
            .lines()
            .find(|line| line.starts_with("tools:"))
            .expect("the definition declares its tools");
        assert_eq!(
            declared,
            match row {
                "measuring" => "tools: Read, Grep, Glob, Bash",
                _ => "tools: Read, Grep, Glob",
            },
            "{row}: the host reads a grant the row did not ask for"
        );

        let repo = tempfile::tempdir().expect("a checkout");
        let payload = format!(
            "{{\"agent_type\":\"review-blind\",\"file_path\":\"src/main.rs\",\"command\":\"x\",\"cwd\":{:?}}}",
            repo.path().display().to_string()
        );

        // Door one: the diagnostic verb.
        let verb = |tool: &str| {
            run_in(
                home.path(),
                repo.path(),
                &[
                    "gate",
                    tool,
                    "--run-id",
                    "claude-aaaa1111",
                    "--input",
                    &payload,
                ],
                "",
            )
        };
        // Door two: what a running agent actually goes through.
        let hook = |tool: &str| {
            let event = format!(
                "{{\"session_id\":\"probe\",\"agent_type\":\"review-blind\",\"tool_name\":{tool:?},\"tool_input\":{{\"file_path\":\"src/main.rs\",\"command\":\"x\"}},\"cwd\":{:?}}}",
                repo.path().display().to_string()
            );
            run_in(home.path(), repo.path(), &["hook", "pre-tool-use"], &event)
        };

        for (door, answer) in [("gate", verb(granted)), ("hook", hook(granted))] {
            let (said, error, _) = answer;
            assert!(
                !format!("{said}{error}").contains("declared"),
                "{row}/{door}: {granted} is what this row grants and the {door} door refused it: {said}{error}"
            );
            assert!(
                !format!("{said}{error}").contains("{{"),
                "{row}/{door}: a placeholder reached the operator, so the definition was never rendered: \
                 {said}{error}"
            );
        }

        for (door, answer) in [("gate", verb(refused)), ("hook", hook(refused))] {
            let (said, error, _) = answer;
            assert!(
                format!("{said}{error}").contains("declared"),
                "{row}/{door}: {refused} is outside this row's grant and the {door} door allowed it: \
                 {said}{error}"
            );
        }

        // And a contract nobody can read hands out the narrower grant, on both
        // doors. This is the case the property lives or dies on, and it is here
        // rather than beside the pure function because a test of the function
        // proves the function: the previous head moved the narrowing into
        // `effective_evidence`, tested it there, and deleted the assertion that
        // held the **call site** — after which dropping the call left the whole
        // suite green while live-widening a delegated reviewer's grant out of a
        // broken file. A blind reviewer measured that, having measured the source-
        // text assertion it replaced. One row is corrupted rather than the whole
        // file, so the `Evidence standard` row is still perfectly readable and the
        // only thing deciding the answer is that the contract as a whole did not
        // parse.
        if row == "measuring" {
            let contract = home
                .path()
                .join(".claude")
                .join("skills")
                .join(estigia::skill::DIRECTORY)
                .join("SKILL.md");
            let installed = std::fs::read_to_string(&contract).expect("the contract is installed");
            let broken = installed.replace("| Change size | 800 |", "| Change size | banana |");
            assert_ne!(
                broken, installed,
                "no row was corrupted, so this proves nothing"
            );
            std::fs::write(&contract, broken).expect("the contract is rewritten");

            for (door, answer) in [("gate", verb("Bash")), ("hook", hook("Bash"))] {
                let (said, error, _) = answer;
                assert!(
                    format!("{said}{error}").contains("declared"),
                    "{door}: a contract that would not parse handed the reviewer the wider grant: \
                     {said}{error}"
                );
            }
        }
    }
}

/// The sealed reviewer's grant is the same through both doors, the sibling of
/// `the_reserved_reviewer_grant_is_the_same_through_both_doors` for
/// `review-blind-sealed`: a door that still hard-coded `Seal::Unsealed` for
/// this name would render `tools: Read, Grep, Glob` rather than the sealed
/// arm's `WebSearch`, and the assertions below would fail in both directions.
#[test]
fn the_sealed_reserved_reviewer_grant_is_the_same_through_both_doors() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["install", "claude-code"], "");
    assert!(ok, "install failed: {stderr}");

    let installed = std::fs::read_to_string(
        home.path()
            .join(".claude")
            .join("agents")
            .join("review-blind-sealed.md"),
    )
    .expect("the sealed reviewer is installed");
    let declared = installed
        .lines()
        .find(|line| line.starts_with("tools:"))
        .expect("the definition declares its tools");
    assert_eq!(
        declared, "tools: WebSearch",
        "the host reads a grant the sealed arm did not render"
    );

    let repo = tempfile::tempdir().expect("a checkout");
    let payload = format!(
        "{{\"agent_type\":\"review-blind-sealed\",\"file_path\":\"src/main.rs\",\"command\":\"x\",\"cwd\":{:?}}}",
        repo.path().display().to_string()
    );

    let verb = |tool: &str| {
        run_in(
            home.path(),
            repo.path(),
            &[
                "gate",
                tool,
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };
    let hook = |tool: &str| {
        let event = format!(
            "{{\"session_id\":\"probe\",\"agent_type\":\"review-blind-sealed\",\"tool_name\":{tool:?},\"tool_input\":{{\"file_path\":\"src/main.rs\",\"command\":\"x\"}},\"cwd\":{:?}}}",
            repo.path().display().to_string()
        );
        run_in(home.path(), repo.path(), &["hook", "pre-tool-use"], &event)
    };

    for (door, answer) in [("gate", verb("WebSearch")), ("hook", hook("WebSearch"))] {
        let (said, error, _) = answer;
        assert!(
            !format!("{said}{error}").contains("declared"),
            "{door}: WebSearch is what the sealed arm grants and the {door} door refused it: \
             {said}{error}"
        );
        assert!(
            !format!("{said}{error}").contains("{{"),
            "{door}: a placeholder reached the operator, so the sealed rendering was never \
             rendered: {said}{error}"
        );
    }

    for (door, answer) in [("gate", verb("Read")), ("hook", hook("Read"))] {
        let (said, error, _) = answer;
        assert!(
            format!("{said}{error}").contains("declared"),
            "{door}: Read is outside the sealed grant and the {door} door allowed it: \
             {said}{error}"
        );
    }
}

/// The sealed name is reserved, so both doors render its grant from the
/// embedded template and never from the file on disk. Installing the
/// definition and then widening its `tools:` line is the only way to tell the
/// two apart: the grant a reserved name carries is identical either way until
/// the bytes disagree.
#[test]
fn a_tampered_sealed_definition_does_not_widen_what_its_doors_allow() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["install", "claude-code"], "");
    assert!(ok, "install failed: {stderr}");

    let definition = home
        .path()
        .join(".claude")
        .join("agents")
        .join("review-blind-sealed.md");
    let installed = std::fs::read_to_string(&definition).expect("the sealed reviewer is installed");
    let widened = installed.replace("tools: WebSearch", "tools: Read, Grep, Glob, Bash");
    assert_ne!(
        widened, installed,
        "no grant was widened, so this proves nothing"
    );
    std::fs::write(&definition, widened).expect("the definition is rewritten");

    let repo = tempfile::tempdir().expect("a checkout");
    let payload = format!(
        "{{\"agent_type\":\"review-blind-sealed\",\"file_path\":\"src/main.rs\",\"command\":\"x\",\"cwd\":{:?}}}",
        repo.path().display().to_string()
    );
    let verb = |tool: &str| {
        run_in(
            home.path(),
            repo.path(),
            &[
                "gate",
                tool,
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };
    let hook = |tool: &str| {
        let event = format!(
            "{{\"session_id\":\"probe\",\"agent_type\":\"review-blind-sealed\",\"tool_name\":{tool:?},\"tool_input\":{{\"file_path\":\"src/main.rs\",\"command\":\"x\"}},\"cwd\":{:?}}}",
            repo.path().display().to_string()
        );
        run_in(home.path(), repo.path(), &["hook", "pre-tool-use"], &event)
    };

    for (door, answer) in [("gate", verb("Read")), ("hook", hook("Read"))] {
        let (said, error, _) = answer;
        assert!(
            format!("{said}{error}").contains("declared"),
            "{door}: the {door} door read the widened grant off disk, so the reservation enforces nothing: {said}{error}"
        );
    }
}

/// A stand-down reaches a role refusal through both doors, or through neither.
///
/// The hook wraps its role denial in `standdown::over`; the door added beside
/// it returned the refusal straight. So an operator who had stood the gate down
/// found it standing down for every refusal except that one, through the agent
/// gated by a plugin.
///
/// Found by asking the systematic version of the question rather than the
/// one-at-a-time version: *which of the things the hook does does this door
/// also do*. It was the round's own change that had the hole.
#[test]
fn a_stand_down_reaches_a_role_refusal_through_both_doors() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    let agents = repo.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("an agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read\n---\n\nReads things.\n",
    )
    .expect("the definition is written");

    let payload = format!(
        "{{\"agent_type\":\"reader\",\"file_path\":\"src/main.rs\",\"cwd\":{:?}}}",
        repo.path().display().to_string()
    );
    let ask = || {
        run_in(
            home.path(),
            repo.path(),
            &[
                "gate",
                "Edit",
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };

    // The floor: without one, the refusal stands.
    let (said, error, ok) = ask();
    assert!(
        !ok && format!("{said}{error}").contains("declared"),
        "the role refusal is gone, so the stand-down below proves nothing: {said}{error}"
    );

    let (_, error, declared) = run(
        home.path(),
        &["stand-down", "--reason", "measuring", "--minutes", "5"],
        "",
    );
    assert!(declared, "the stand-down was refused: {error}");

    let (said, error, ok) = ask();
    assert!(
        ok,
        "a stand-down reaches every other refusal and not this one: {said}{error}"
    );
}

#[test]
fn a_push_the_guard_decided_on_is_one_the_ledger_holds() {
    // The ledger is written by the tool path and by `stand-down`, and by nothing
    // else. The `PrePush` arm decides and records nothing — so a push this
    // repository's guard refused, or allowed under a live claim, leaves no trace
    // at all, at the boundary the honesty contract calls the unconditional one.
    //
    // It costs twice. Nobody can say afterwards which pushes were adjudicated,
    // which is the question the ledger exists to answer. And `doctor`'s silence
    // row reads that same file to say whether anything has reached the gate: on
    // a machine where every push has been decided and no tool call has, it
    // answers *no call has reached the gate yet*.
    //
    // `Outside` stays unrecorded, deliberately and for the reason `hook::note`
    // already gives: a checkout nobody holds is not this harness's business, and
    // logging every push in every repository would bury the ones that mattered.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a git directory");

    // A pointer that covers this checkout and will not parse. It reaches a
    // refusal without a tracker: `run-pointers-unreadable` is decided from the
    // file alone, which is what makes this measurable at all.
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the state directory");
    std::fs::write(runs.join("claude-torn0000.json"), "{ this is not json")
        .expect("the torn pointer is written");

    let refs = "refs/heads/work 1111111111111111111111111111111111111111                 refs/heads/main 2222222222222222222222222222222222222222
";
    let (_, error, ok) = run_in(home.path(), repo.path(), &["hook", "pre-push"], refs);
    assert!(
        !ok && error.contains("run-pointers-unreadable"),
        "the guard did not decide, so this measures nothing: {error}"
    );

    let written = decisions(home.path());
    assert!(
        written.contains("git push"),
        "the guard refused a push and the ledger says nothing about it: {written:?}"
    );
    assert!(
        written.contains("deny") && written.contains("run-pointers-unreadable"),
        "the entry does not say what was decided or why: {written:?}"
    );
    // And what it was aimed at. git hands a `pre-push` hook the refs it is about
    // to write, on standard input; the arm decided without reading a byte of
    // them, so the ledger could say a push had been adjudicated and never which
    // push. Reading them changes no decision — the honesty contract still
    // records that nothing adjudicates the destination — but it is the
    // difference between a boundary that can be audited afterwards and one that
    // can only be spoken about in the present.
    assert!(
        written.contains("refs/heads/main"),
        "the ledger holds a push decision and not what the push was aimed at: {written:?}"
    );

    // And a push from a checkout nobody holds is still not written down.
    let bare = tempfile::tempdir().expect("a second home");
    std::fs::create_dir_all(bare.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let elsewhere = tempfile::tempdir().expect("another checkout");
    std::fs::create_dir_all(elsewhere.path().join(".git")).expect("a git directory");
    let (_, _, quiet) = run_in(bare.path(), elsewhere.path(), &["hook", "pre-push"], "");
    assert!(quiet, "a push nobody holds was refused");
    assert!(
        decisions(bare.path()).is_empty(),
        "a push from a checkout nobody holds was written down"
    );
}

/// Drives `estigia hook pre-push` with a real `pre-push` stdin against two
/// isolated worktrees of one checkout — the production wiring, not
/// `harness::guard::decide` called directly. `docs/honesty.md`'s narrowing
/// entry names this as the first test in the checkout to drive that wiring
/// with more than one holder in play. Pointing `harness::guard::decide` at
/// an empty slice instead of `&local_refs` compiles and leaves the rest of
/// `cargo test` green without this test.
#[test]
fn a_pre_push_hook_is_decided_on_the_holder_the_pushed_branch_names() {
    let rig = tracker_rig();

    // Two isolated worktrees of the fixture checkout, each on its own branch
    // — the shape `narrow_to_pushed_branch` reads through `git worktree
    // list`, and no unit test drives through a spawned `estigia` process.
    let git = |arguments: &[&str]| checked_git(rig.repo.path(), arguments);
    // A directory of its own, not the checkout's shared parent — that parent
    // is the system temp root, and a fixed name under it collides with
    // whatever the previous run of this same test left behind.
    let parent = tempfile::tempdir().expect("a directory for the two worktrees");
    let worktree_a = parent.path().join("wt-a");
    let worktree_b = parent.path().join("wt-b");
    assert!(
        git(&[
            "worktree",
            "add",
            "-b",
            "branch-a",
            &worktree_a.display().to_string()
        ]),
        "the fixture's first worktree could not be added"
    );
    assert!(
        git(&[
            "worktree",
            "add",
            "-b",
            "branch-b",
            &worktree_b.display().to_string()
        ]),
        "the fixture's second worktree could not be added"
    );
    let base_branch = {
        let output = Command::new("git")
            .arg("-C")
            .arg(rig.repo.path())
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .expect("git answers for the fixture checkout");
        assert!(output.status.success(), "the base checkout has no branch");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };

    let runs = rig.home.path().join(".estigia").join("runs");
    for (run_id, issue, worktree) in [
        ("claude-abcd1234", 12u64, &worktree_a),
        ("claude-eeee5555", 34u64, &worktree_b),
    ] {
        let pointer = serde_json::json!({
            "run_id": run_id,
            "issue": issue,
            "state": "in-progress",
            "repo_dir": rig.repo.path(),
            "worktree": worktree,
            "verified_at": serde_json::Value::Null,
        });
        std::fs::write(
            runs.join(format!("{run_id}.json")),
            serde_json::to_string(&pointer).expect("a pointer serialises"),
        )
        .expect("the pointer writes");
    }

    // Allowed: the local ref names holder A's own branch, so the checkout's
    // worktree registry selects that holder and only that holder — and the
    // ledger records *which* holder, not merely that somebody was chosen. The
    // remote side names a different ref on purpose: reading it instead of the
    // local side (the swapped-bindings mutation) resolves nothing and denies.
    clear_decisions(rig.home.path());
    let refs = format!(
        "refs/heads/branch-a {} refs/heads/renamed-on-remote {}\n",
        "1".repeat(40),
        "2".repeat(40)
    );
    let (out, err, ok) = run_with_tracker(
        rig.home.path(),
        rig.repo.path(),
        rig.bin.path(),
        &issue_answer("in-progress"),
        &["hook", "pre-push"],
        &refs,
    );
    assert!(
        ok,
        "a push naming one holder's own branch was refused: {out}{err}"
    );
    let wrote = decisions(rig.home.path());
    assert!(
        wrote.contains("claude-abcd1234"),
        "the ledger did not record the holder the pushed branch named: {wrote:?}"
    );
    assert!(
        !wrote.contains("claude-eeee5555"),
        "the ledger recorded the holder that branch does not belong to: {wrote:?}"
    );

    // Refused: the local ref names the base branch, which neither holder's
    // worktree is checked out on. Narrowing fails closed, both holders stay
    // counted, and the push is refused as the genuine ambiguity it is —
    // exactly the boundary a push straight to the integration branch is.
    clear_decisions(rig.home.path());
    let refs = format!(
        "refs/heads/{base_branch} {} refs/heads/{base_branch} {}\n",
        "1".repeat(40),
        "2".repeat(40)
    );
    let ambiguous = serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": serde_json::json!({"state": "OPEN", "labels": [], "comments": []}).to_string(),
        "status": 0,
    }]))
    .expect("the script serialises");
    let (out, err, ok) = run_with_tracker(
        rig.home.path(),
        rig.repo.path(),
        rig.bin.path(),
        &ambiguous,
        &["hook", "pre-push"],
        &refs,
    );
    assert!(
        !ok,
        "a push naming the base branch, owned by neither holder, was allowed: {out}{err}"
    );
    assert!(
        format!("{out}{err}").contains("several-runs-hold-this-checkout"),
        "a push to the base branch was refused for a reason other than the genuine \
         ambiguity between its two holders: {out}{err}"
    );
}

#[test]
fn a_row_another_contract_did_not_take_is_not_one_it_took() {
    // `config set` writes a repository-wide row into every installed contract,
    // and the reason is written beside the loop that does it: *an operator with
    // two agents installed declared `make deploy` a one-way door, and only one
    // of the two contracts said so. The gate read the one that did and enforced
    // it; the agent reading the other was never told what it had walked into.*
    //
    // The primary write is read back and refuses when `estigia.local.md`
    // overrides the row. The propagated ones were written and never read, and
    // the count of them was reported as *"written into N other installed
    // contract(s): it is a fact about this repository, so every agent's copy
    // says it"*.
    //
    // Measured on the installed pair, with the operator's own file in the shared
    // neutral root answering that row:
    //
    // ```text
    // $ estigia config set "Irreversible commands" "terraform apply"
    // Irreversible commands is now terraform apply
    //   written into 1 other installed contract(s) … every agent's copy says it
    // $ estigia config list --agent opencode
    // Irreversible commands   none
    // ```
    //
    // Which is the incident that paragraph describes, reproduced by the fix
    // written to prevent it — on the row that declares a one-way door.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let (_, stderr, ok) = run(home.path(), &["setup", "opencode"], "");
    assert!(ok, "setup failed: {stderr}");

    let shared = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::write(
        shared.join("estigia.local.md"),
        "| Setting | Value here |\n|---|---|\n| Irreversible commands | none |\n",
    )
    .expect("the override is written");

    let (said, error, ok) = run(
        home.path(),
        &["config", "set", "Irreversible commands", "terraform apply"],
        "",
    );

    // The floor: the other contract really does answer differently, so this is
    // measuring a disagreement rather than a message.
    let (theirs, _, _) = run(home.path(), &["config", "list", "--agent", "opencode"], "");
    assert!(
        theirs
            .lines()
            .any(|line| line.starts_with("Irreversible commands") && line.contains("none")),
        "the shared root took the row after all, so this measures nothing:\n{theirs}"
    );

    assert!(
        !ok,
        "a row one agent will never read was reported as written everywhere:\n{said}"
    );
    assert!(
        error.contains("terraform apply") || error.contains("Irreversible commands"),
        "the refusal does not name the row: {error}"
    );
    assert!(
        error.contains(".agents"),
        "the refusal does not name the contract that did not take it: {error}"
    );
}

#[test]
fn a_dry_run_says_what_would_be_left_behind_too() {
    // `--dry-run` is sold as *see exactly what would change first*, and the
    // question an operator runs it to answer, before letting an uninstaller near
    // their home directory, is whether it will take their things.
    //
    // The real run answers it: *1 file(s) in that directory are not Estigia's
    // and were left there*. The dry run said nothing at all — the note was
    // computed only when something had actually been removed, so the one moment
    // it is being read to make a decision is the one moment it is absent.
    //
    // The listing is the same either way: it names the files no action in the
    // plan names, and a plan has actions.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::write(root.join("NOTAS.md"), "my own notes\n").expect("the notes are written");

    let (planned, _, _) = run(
        home.path(),
        &["setup", "claude-code", "--uninstall", "--dry-run"],
        "",
    );
    assert!(
        planned.contains("NOTAS.md"),
        "the dry run does not say the operator's file would be left:\n{planned}"
    );
    // Said in the tense of a plan. A dry run reporting that files "were left
    // there" is a dry run claiming to have done something.
    assert!(
        !planned.contains("were left there"),
        "the dry run speaks as though it had already removed anything:\n{planned}"
    );
    // And nothing was touched by asking.
    assert!(
        root.join("SKILL.md").is_file() && root.join("NOTAS.md").is_file(),
        "the dry run removed something"
    );

    // The plan and the act name the same file.
    let (done, _, _) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(
        done.contains("NOTAS.md") && done.contains("were left there"),
        "the real run stopped naming it, so the two are being compared to nothing:\n{done}"
    );
}

#[test]
fn a_guard_that_was_not_there_is_not_a_guard_that_was_removed() {
    // `guard::uninstall` answers with the state the repository is in
    // *afterwards*, and `Absent` is what it answers for two different things:
    // Estigia's hook was here and is gone, and there was never one here. The
    // renderer printed `<path> removed` for both — and for a third, because
    // `--dry-run` takes the same arm and had removed nothing at all.
    //
    // One word for three states, in the command whose job is taking a file off
    // somebody's machine. The arms below it in that same match are there for
    // exactly this reason — *nothing was taken out, and saying so beats
    // reporting a removal that did not happen* — and the main arm collapsed it.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repo = tempfile::tempdir().expect("a checkout");
    // A real one: the guard asks `git rev-parse --git-path hooks` rather than
    // guessing, so a hand-made `.git` is not a repository to it.
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .arg("init")
            .arg("-q")
            .status()
            .is_ok_and(|status| status.success()),
        "git is not on the path, so this cannot measure anything"
    );
    let hook = repo.path().join(".git").join("hooks").join("pre-push");

    // Nothing here: an uninstall removes nothing and must not say it did.
    let (empty, _, _) = run_in(home.path(), repo.path(), &["guard", "--uninstall"], "");
    assert!(
        !empty.contains("removed"),
        "an uninstall with nothing to remove reported a removal: {empty}"
    );

    run_in(home.path(), repo.path(), &["guard"], "");
    assert!(
        hook.is_file(),
        "the guard did not install, so this measures nothing"
    );

    // A plan removes nothing either, and says so in the tense of a plan.
    let (planned, _, _) = run_in(
        home.path(),
        repo.path(),
        &["guard", "--uninstall", "--dry-run"],
        "",
    );
    assert!(
        hook.is_file(),
        "the dry run removed the hook, which is the one thing it must not do"
    );
    assert!(
        planned.contains("would be removed"),
        "the dry run does not say what it would do: {planned}"
    );
    // And not in the tense the act uses. `would be removed` contains the act's
    // own wording, so the two are told apart by the whole line rather than by a
    // substring — the assertion that read `" removed"` failed on the fix.
    assert!(
        !planned
            .lines()
            .any(|line| line.trim_end().ends_with("pre-push removed")),
        "the dry run spoke as though it had removed the hook: {planned}"
    );

    // And the act says it did.
    let (done, _, _) = run_in(home.path(), repo.path(), &["guard", "--uninstall"], "");
    assert!(!hook.exists(), "the hook survived its own removal");
    assert!(
        done.contains("removed"),
        "the removal that happened was not reported: {done}"
    );
}

#[test]
fn what_a_half_written_file_left_behind_is_not_the_operators() {
    // Every write goes through a temporary beside its target — `SKILL.md` is
    // written as `SKILL.estigia-<pid>.tmp` and renamed over. The cleanup runs
    // when the write **fails**; a process killed between the create and the
    // rename leaves the temporary where it is, and nothing sweeps it.
    //
    // So the uninstaller walked the directory, found a file no action had named,
    // and told the operator it was theirs — Estigia's own residue, in the one
    // sentence that exists to answer *did it touch my things?*. It is the round
    // that fixed that sentence, one file over: a name Estigia chose, reported as
    // one it had never heard of.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let residue = root.join("SKILL.estigia-4321.tmp");
    std::fs::write(&residue, "half a contract\n").expect("the residue is written");
    std::fs::write(root.join("NOTAS.md"), "my own notes\n").expect("the notes are written");

    let (said, _, _) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    let note = said
        .lines()
        .find(|line| line.contains("are not Estigia's"))
        .unwrap_or_else(|| panic!("the surviving directory was not accounted for:\n{said}"));
    assert!(
        note.contains("NOTAS.md"),
        "the operator's own file is the one thing this note exists to name: {note}"
    );
    assert!(
        !note.contains("estigia-4321"),
        "Estigia's own half-written file was reported as the operator's: {note}"
    );

    // And it is taken away. A name this tool chose is a file this tool owns, and
    // an uninstall that leaves it is one that did not remove what it created.
    assert!(
        !residue.exists(),
        "the residue of a write that died survived the uninstall"
    );
    assert!(
        root.join("NOTAS.md").is_file(),
        "the sweep took a file that was not Estigia's"
    );
}

#[test]
fn a_row_that_carried_a_separator_is_not_a_row_that_said_less() {
    // `config set` refuses a value holding a `|` by name — *the value is stored
    // as one cell of a one-line table row, and nothing escapes either* — which
    // is exactly right, and covers the door Estigia writes through.
    //
    // The other door reads a file Estigia never writes. There the `|` is not
    // refused: it is a cell boundary, so the row splits one cell early and the
    // value arrives **truncated**, with nothing anywhere saying so. Measured:
    //
    // ```text
    // the operator wrote : make deploy | tee log
    // the tool believes  : make deploy
    // ```
    //
    // On the row that declares one-way doors, in the file the operator is told
    // to edit by hand. A value read as something the file does not say is the
    // failure this whole tool exists to refuse.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    std::fs::write(
        home.path()
            .join(".claude")
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .join("estigia.local.md"),
        "| Setting | Value here |\n|---|---|\n| Irreversible commands | make deploy | tee log |\n",
    )
    .expect("the override is written");

    // The floor: the row really is read, and read short. A file nobody read
    // would satisfy the assertion below by saying nothing at all.
    let (listed, _, _) = run(home.path(), &["config", "list"], "");
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("Irreversible commands") && line.contains("make deploy")),
        "the row is not read at all, so nothing here is measured:\n{listed}"
    );

    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        whole.contains("Irreversible commands"),
        "a value the file does not carry is reported by nothing:\n{whole}"
    );
    assert!(
        whole.contains("estigia.local.md"),
        "the report does not name the file the row is in:\n{whole}"
    );
}

#[test]
fn a_line_ending_nobody_can_see_is_not_a_row_that_said_less() {
    // The sibling above found the `|`. The bar is at least *visible*: an
    // operator staring at the row can see what cut it. Eight more characters
    // end a line for both readers of this table — `lines_the_transport_sees`
    // exists because they do — and U+2028 is the one a paste out of a browser
    // or a word processor routinely carries. It draws as nothing.
    //
    // ```text
    // the operator wrote : make deploy<U+2028>npm publish
    // the tool believes  : make deploy
    // ```
    //
    // `npm publish` stops being a declared one-way door. The direction is the
    // whole finding: a value read as less than it says always *loosens*, and
    // this crate's own rule is that configuration may only tighten.
    //
    // Width cannot see it. The bar makes a row wider than its header; a line
    // ending makes it narrower, and turns the rest into a fragment that starts
    // with no `|` and is dropped entirely — which is why the check walks the
    // line an editor draws rather than the line the parser produces.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let local = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.local.md");
    std::fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n\
         | Irreversible commands | make deploy\u{2028}npm publish |\n",
    )
    .expect("the override is written");

    // The floor, and it is the finding: the row is read, and read without the
    // half that mattered. An assertion that only checked `doctor` would pass
    // just as well against a file nobody opened.
    let (listed, _, _) = run(home.path(), &["config", "list"], "");
    // The value column only. The `accepts:` clause beside it spells `npm
    // publish` as its own example, and a floor that read the whole line would
    // have been satisfied by the help text rather than by the value.
    let row = listed
        .lines()
        .find(|line| line.starts_with("Irreversible commands"))
        .unwrap_or_default()
        .split("accepts:")
        .next()
        .unwrap_or_default();
    assert!(
        row.contains("make deploy") && !row.contains("npm publish"),
        "the fixture poses nothing \u{2014} the value is not being cut:\n{listed}"
    );

    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        whole.contains("Irreversible commands") && whole.contains("estigia.local.md"),
        "a value the file does not carry is reported by nothing:\n{whole}"
    );
    // Spelled as a code point, because that is what an operator can search for.
    // The character itself would paste into the message as nothing, and a
    // message telling somebody to look for a `|` in a row that has none sends
    // them hunting for the wrong thing.
    assert!(
        whole.contains("U+2028") && !whole.contains("holds a `|`"),
        "the report does not say what to look for, or names the wrong character:\n{whole}"
    );

    // And the same table, whole, is still fine — or every installation
    // reads as broken and the row teaches nobody anything.
    std::fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n\
         | Irreversible commands | make deploy, npm publish |\n",
    )
    .expect("the override is rewritten");
    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        !whole.contains("Irreversible commands"),
        "an intact row was reported as cut:\n{whole}"
    );
}

/// Using the override file the contract tells the operator to use does not make
/// the skill out of date.
///
/// `presence_of` renders every shipped file from the configuration and compares.
/// It read that configuration with `installed_config`, which layers
/// `estigia.local.md` on top — so it compared the installed `SKILL.md` against a
/// table carrying the operator's override, and `SKILL.md` cannot carry it: the
/// override exists to change what the tool does *without* changing that file.
///
/// Measured on a fresh install and one row:
///
/// ```text
/// | Change size | 250 |   in estigia.local.md
///
/// doctor : BROKEN skill    — "not this binary's copy"
///          BROKEN contract — "the rules the agent reads are not the rules the
///                            gate enforces"
/// then   : "skill is not usable, so a run cannot swear yet"
/// ```
///
/// All three sentences were false. The payload was this binary's, byte for byte.
///
/// The repair the message named made it worse rather than wrong: `estigia sync`
/// discharged it by writing `rebase` **into the versioned block** — the file
/// that is committed and shared, carrying a note that reads *Configure the
/// ignored local file, never this versioned block*. Deleting the override
/// afterwards left the value behind, so a machine-local choice had quietly
/// become the team's.
#[test]
fn the_operators_own_override_does_not_make_the_contract_look_stale() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let versioned = root.join("SKILL.md");

    // The floor. A fresh install has to read as current, or "still current
    // after the override" is a sentence about nothing.
    let (before, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !before.contains("not this binary's copy"),
        "a freshly installed skill already reads as stale:\n{before}"
    );

    std::fs::write(
        root.join("estigia.local.md"),
        "| Setting | Value |\n|---|---|\n| Change size | 400 |\n",
    )
    .expect("the operator's own file");

    // It is in force — this is the override doing its job, and without it the
    // assertion below would pass on a file nobody read.
    let (listed, _, _) = run(home.path(), &["config", "list"], "");
    assert_eq!(
        listed_value(&listed, "Change size"),
        "400",
        "the override is not in force, so nothing here is measured:\n{listed}"
    );

    let (after, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !after.contains("not this binary's copy"),
        "using the override file reported the skill as not this binary's copy:\n{after}"
    );
    // And the versioned block still says what this binary wrote, whether or not
    // anything asked it to re-render.
    run(home.path(), &["sync"], "");
    let table = std::fs::read_to_string(&versioned).expect("the contract reads");
    assert!(
        table.contains("| Change size | 800 |"),
        "sync promoted an `estigia.local.md` row into the versioned block:\n{}",
        table
            .lines()
            .find(|line| line.starts_with("| Change size"))
            .unwrap_or("(the row is gone)")
    );
    // The override still overrides after the sync, or this was fixed by
    // dropping the operator's value instead of by keeping it where it belongs.
    let (still, _, _) = run(home.path(), &["config", "list"], "");
    assert_eq!(
        listed_value(&still, "Change size"),
        "400",
        "sync stopped the override from overriding:\n{still}"
    );

    // And a genuinely old payload is still reported. Fixing the false alarm by
    // never raising the true one is the failure this crate exists to refuse.
    // Aged through the binding, now that the transport is retired and no
    // longer installed. The same fact told about a file that is still
    // there: a payload behind the binary.
    std::fs::write(
        root.join("bindings").join("github.md"),
        "# older
",
    )
    .expect("an aged binding");
    let (aged, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        aged.contains("not this binary's copy"),
        "a skill that really is behind the binary is no longer reported:\n{aged}"
    );
}

/// A command the operator added beside Estigia's is not Estigia's to remove.
///
/// A hook group is a **matcher and a list**, and the group was dropped whole
/// whenever it held one of ours. So an operator who put their own command in
/// the `PreToolUse` group Estigia had written lost it on the next uninstall —
/// and on the next `sync`, which rewrites the same group.
///
/// `render` says the rule two levels up, twice, in its own words: *an operator's
/// unfilled slot is theirs*, and *a wrapper is ours to drop only when what
/// emptied it was us*. The innermost level is the one holding the thing they
/// typed, and it was the one where the rule was not applied.
#[test]
fn a_hook_the_operator_added_beside_estigias_survives_the_uninstall() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let settings = home.path().join(".claude").join("settings.json");

    let mut root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).expect("the settings"))
            .expect("settings are JSON");
    let group = root["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_mut()
        .expect("Estigia wrote a group");
    // The floor: Estigia's own command is in there, so "theirs survives" is not
    // a sentence about an empty group.
    assert_eq!(group.len(), 1, "the group is not the one Estigia wrote");
    group.push(serde_json::json!({"type": "command", "command": "mi-script-propio.sh"}));
    std::fs::write(
        &settings,
        serde_json::to_string_pretty(&root).expect("serialises"),
    )
    .expect("their command is added");

    // A sync rewrites that group, and must not take it either.
    let (_, stderr, ok) = run(home.path(), &["sync"], "");
    assert!(ok, "sync failed: {stderr}");
    let after = std::fs::read_to_string(&settings).expect("the settings");
    assert!(
        after.contains("mi-script-propio.sh"),
        "sync took a command the operator added:\n{after}"
    );
    assert!(
        after.contains("hook pre-tool-use"),
        "sync stopped registering the gate:\n{after}"
    );

    run(home.path(), &["uninstall", "--all"], "");
    let left = std::fs::read_to_string(&settings).unwrap_or_default();
    assert!(
        left.contains("mi-script-propio.sh"),
        "uninstall took a command the operator added beside Estigia's:\n{left}"
    );
    assert!(
        !left.contains("hook pre-tool-use"),
        "uninstall left Estigia's own command behind:\n{left}"
    );
}

/// One unusable file does not cost the other agents their setup, and the code
/// does not claim nothing was written.
///
/// `collect_over` stopped at the first refusal and propagated it with `?`,
/// throwing away every result collected before it. Measured: one hand-edited
/// `~/.cursor/hooks.json` and `estigia setup --all` wrote **58 files** across
/// six agents, left five untouched, printed nothing but *Cursor: … is not
/// JSON*, and exited `1`.
///
/// Two things wrong, and the second is the one this crate is about. The agents
/// behind the failing one had nothing wrong with them and got nothing, over a
/// file that is not theirs. And exit `1` is `ExitCode::Refused`, whose sentence
/// is *the command refused, and nothing was written* — said about a run that had
/// written fifty-eight files and named none of them.
#[test]
fn a_file_one_agent_cannot_take_does_not_stop_the_others_or_hide_the_writes() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let cursor = home.path().join(".cursor");
    std::fs::create_dir_all(&cursor).expect("cursor's directory");
    std::fs::write(cursor.join("hooks.json"), "no es json {\n").expect("a file it cannot take");

    let status = |arguments: &[&str]| {
        estigia_command()
            .args(arguments)
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .env("APPDATA", home.path().join("AppData").join("Roaming"))
            .env("XDG_CONFIG_HOME", home.path().join(".config"))
            .current_dir(home.path())
            .output()
            .expect("the binary runs")
            .status
            .code()
    };

    let (said, refused, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(!ok, "a file it cannot take was accepted");
    assert!(
        refused.contains("cursor") || refused.contains("Cursor"),
        "the refusal does not name the agent whose file it is: {refused}"
    );
    // The report of what it did, which used to be thrown away with the results.
    assert!(
        said.contains("Claude Code") && said.contains("changed"),
        "the run wrote files and reported none of them:\n{said}{refused}"
    );

    // And the refusal says what happened to the rest of the batch. The
    // outcome's own line is written for a single write — *the write landed;
    // what failed came after it* — and a reader handed one refusal has no way
    // to tell from it that ten other agents were done.
    assert!(
        refused.contains("agent(s) were done"),
        "the refusal says nothing about the agents that were done: {refused}"
    );

    // Every other agent is configured, or one operator's hand-edited file has
    // cost ten agents their gate. Cursor itself stays untouched because setup
    // now prevalidates its late hook render before writing its earlier files.
    let (state, _, _) = run(home.path(), &["status"], "");
    let left = state
        .lines()
        .filter(|line| line.contains("not configured"))
        .collect::<Vec<_>>();
    assert_eq!(
        left.len(),
        1,
        "agents other than Cursor were left unconfigured by its file:\n{state}"
    );
    assert!(
        left[0].contains("cursor"),
        "the one untouched agent was not Cursor:\n{state}"
    );

    // And the code says what happened to the world. `1` is *nothing was
    // written*; this run wrote.
    assert_eq!(
        status(&["setup", "--all", "--allow-source-build"]),
        Some(1),
        "a second run wrote nothing and did not answer with the refused code"
    );
    let fresh = tempfile::tempdir().expect("a second home");
    std::fs::create_dir_all(fresh.path().join("AppData").join("Roaming")).expect("a roaming dir");
    std::fs::create_dir_all(fresh.path().join(".cursor")).expect("cursor's directory");
    std::fs::write(
        fresh.path().join(".cursor").join("hooks.json"),
        "no es json {\n",
    )
    .expect("a file it cannot take");
    let first = estigia_command()
        .args(["setup", "--all", "--allow-source-build"])
        .env("HOME", fresh.path())
        .env("USERPROFILE", fresh.path())
        .env("APPDATA", fresh.path().join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", fresh.path().join(".config"))
        .current_dir(fresh.path())
        .output()
        .expect("the binary runs")
        .status
        .code();
    assert_eq!(
        first,
        Some(2),
        "a run that wrote and then refused answered with the code that means nothing was written"
    );
}

/// Setting one row for a checkout does not answer for the rest of them.
///
/// A repository's file is an **override**: a row that is not in it means *this
/// checkout does not answer for that setting*, and the machine's answer stands.
/// `config set --repo` wrote the whole scope, so every row nobody asked about
/// became an answer — with the **defaults**, because a file that is not there
/// yet is read as `Config::default()`.
///
/// Measured through the binary, one command:
///
/// ```text
/// before   Worktree: C:/trees     Tracker: github acme/web
/// asked    Change size = 250 --repo
/// after    Worktree: unset        Tracker: github
/// ```
///
/// That checkout then pointed at a different tracker than the operator had
/// configured, and made its worktrees somewhere else. Nothing said so.
#[test]
fn setting_one_row_for_a_checkout_leaves_the_others_to_the_machine() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    run(
        home.path(),
        &["config", "set", "Tracker", "github acme/web"],
        "",
    );
    run(
        home.path(),
        &["config", "set", "Worktree location", TREES],
        "",
    );

    let repo = home.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("a checkout");

    // The floor: inside the checkout, the machine's answers are what is read.
    let (before, _, _) = run_in(home.path(), &repo, &["config", "list"], "");
    assert!(
        before.contains("github acme/web") && before.contains(TREES),
        "the checkout does not start from the machine's answers:\n{before}"
    );

    let (_, said, ok) = run_in(
        home.path(),
        &repo,
        &["config", "set", "Change size", "400", "--repo"],
        "",
    );
    assert!(ok, "the row was not set: {said}");

    let (after, _, _) = run_in(home.path(), &repo, &["config", "list"], "");
    assert_eq!(
        listed_value(&after, "Change size"),
        "400",
        "the row that was asked for did not land:\n{after}"
    );
    assert!(
        after.contains("github acme/web"),
        "asking about the change size reset this checkout's tracker:\n{after}"
    );
    assert!(
        after.contains(TREES),
        "asking about the change size reset this checkout's worktree location:\n{after}"
    );

    // The file carries the one row, and a second `--repo` adds to it rather
    // than replacing what the first one said.
    let file = repo.join(".git").join("estigia").join("estigia.local.md");
    let written = std::fs::read_to_string(&file).expect("the checkout's file");
    assert!(
        !written.contains("| Tracker |"),
        "the file answers for a row nobody asked it about:\n{written}"
    );
    run_in(
        home.path(),
        &repo,
        &["config", "set", "Integration", "trunk", "--repo"],
        "",
    );
    let written = std::fs::read_to_string(&file).expect("the checkout's file");
    assert!(
        written.contains("| Change size | 400 |")
            && written.contains("| Integration mode | trunk |"),
        "a second row replaced the first instead of joining it:\n{written}"
    );
}

/// Setting one row for an agent does not answer for the rest of its scope.
///
/// The sibling of the repository's file, and the same rule: an adapter's own
/// file inside a shared skill root is an **override**, so a row it does not
/// carry is one that adapter does not answer for and the shared contract's
/// answer stands. `config set --agent` wrote the whole agent scope, so one
/// question pinned five rows.
///
/// Less costly than the repository's only by luck of where the values come
/// from: this one writes the effective answers rather than the defaults, so
/// nothing is lost at the moment of writing. What is lost is later, quietly,
/// and the operator has no reason to look — measured by moving the shared
/// answer afterwards and watching one adapter follow and the other not.
#[test]
fn setting_one_row_for_an_agent_leaves_the_others_to_the_contract() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    // Two adapters that share the neutral root, which is the only place an
    // adapter has a file of its own.
    run(home.path(), &["setup", "cursor"], "");
    run(home.path(), &["setup", "zcode"], "");

    let (_, said, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Model routing",
            "developer=opus",
            "--agent",
            "cursor",
        ],
        "",
    );
    assert!(ok, "the row was not set: {said}");

    let file = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.cursor.md");
    let written = std::fs::read_to_string(&file).expect("cursor's own file");
    assert!(
        written.contains("| Model routing | developer=opus |"),
        "the row that was asked for did not land:\n{written}"
    );
    assert!(
        !written.contains("| Delivery authorisation |"),
        "the file answers for a row nobody asked it about:\n{written}"
    );

    // The shared answer moves, and both adapters move with it — the one with a
    // file of its own included, because that file says nothing about this row.
    let (_, _, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Delivery authorisation",
            "ask",
            "--agent",
            "zcode",
        ],
        "",
    );
    assert!(ok, "the shared table would not take the row");
    for agent in ["zcode", "cursor"] {
        let (listed, _, _) = run(home.path(), &["config", "list", "--agent", agent], "");
        let row = listed
            .lines()
            .find(|line| line.starts_with("Delivery authorisation"))
            .unwrap_or_default();
        assert!(
            row.contains("ask"),
            "{agent} did not follow the shared answer for a row it never set: {row}"
        );
    }
    // And cursor's own answer is still its own.
    let (listed, _, _) = run(home.path(), &["config", "list", "--agent", "cursor"], "");
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("Model routing") && line.contains("developer=opus")),
        "the row that was set for this agent stopped being set:\n{listed}"
    );
}

/// A long write flag on the command line is a write, whatever the command is.
///
/// The population is drawn at *what is visible on the command line*, and
/// `--write` is as visible as a redirect. It was read only for the handful of
/// commands `IN_PLACE` names, so a formatter rewriting every file in a tree
/// classified as **nothing at all** — measured through the gate:
///
/// ```text
/// sed -i s/a/b/ src/x.rs   watched
/// prettier --write src     NOT watched
/// eslint --fix .           NOT watched
/// ```
///
/// Long forms only, and that is what keeps this a shape rather than a
/// catalogue: `-i` is *ignore case* to `grep` and `-w` is *whole word*, so
/// reading either everywhere would report a search as a write.
#[test]
fn a_long_write_flag_is_a_write_whatever_the_command_is() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    let watched = |command: &str| {
        let (said, _, _) = run_in(
            home.path(),
            home.path(),
            &[
                "gate",
                "Bash",
                "--json",
                "--input",
                &format!("{{\"command\":{command:?}}}"),
            ],
            "",
        );
        // The stable name, not the Rust one. This read `"NotWatched"` — the
        // `Debug` spelling `gate --json` used to print — which is how a field
        // built out of a debug rendering becomes an interface without anybody
        // deciding it should be one.
        !said.contains("not-watched")
    };

    // The floor, both ways: a redirect is seen and a read is not, so "seen" and
    // "not seen" are both answers this gate really gives.
    assert!(
        watched("echo hola > f.txt"),
        "a redirect stopped being a write"
    );
    assert!(!watched("cat f.txt"), "a read is being reported as a write");

    for rewrites in [
        "prettier --write src",
        "eslint --fix .",
        "ruff check --fix .",
        "cargo clippy --fix",
        "somethingnew --in-place x",
    ] {
        assert!(
            watched(rewrites),
            "`{rewrites}` rewrites files and the gate does not see it"
        );
    }

    // And a search is still a search. `-i` and `-w` are the reason this reads
    // long forms only.
    for reads in ["grep -i hola src", "grep -w hola src", "sort -i f.txt"] {
        assert!(
            !watched(reads),
            "`{reads}` is a search and the gate reports it as a write"
        );
    }
}

/// An install record that cannot be read stops the uninstall rather than
/// guessing at it.
///
/// The record is the only thing that can tell a file Estigia created from one it
/// wrote over, so an unreadable one is not an empty one. Writing a fresh record
/// would lose the list; removing on no evidence would take somebody else's file.
///
/// Held here because the honesty contract's entry beside it — *by name, and not
/// by content* — describes what this path does when the record **is** readable,
/// and the two answers have to stay different.
#[test]
fn an_unreadable_install_record_stops_the_uninstall_and_says_what_it_costs() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let record = root.join(".estigia").join("installed.json");

    // The floor: with the record intact, uninstall really does remove things —
    // otherwise "it removed nothing" below is a sentence about a command that
    // never removes anything.
    let (planned, _, _) = run(home.path(), &["uninstall", "--all", "--dry-run"], "");
    assert!(
        planned
            .lines()
            .filter(|line| line.contains("would remove"))
            .count()
            > 5,
        "a dry run over an intact install plans nothing:\n{planned}"
    );

    std::fs::write(&record, "esto no es json {\n").expect("the record is damaged");
    let (_, said, ok) = run(home.path(), &["uninstall", "--all"], "");
    assert!(!ok, "an unreadable record was treated as an empty one");
    assert!(
        said.contains("install-record-unreadable"),
        "the refusal does not name what could not be read: {said}"
    );
    // What deleting it costs, because that is the only way forward and it is
    // not free.
    assert!(
        said.contains("forgets it created the files already there"),
        "the resolution does not say what starting a new record costs: {said}"
    );
    // And nothing was taken on the way to refusing.
    assert!(
        root.join("SKILL.md").is_file(),
        "the contract was removed under a record nobody could read"
    );
}

/// The two checks the gate makes *after* the tracker agrees, actually reached.
///
/// Both `stale_verdict` and `out_of_phase` are tested directly, in the library,
/// against hand-built runs — and both could be **disconnected from the gate**
/// with the whole suite still green. Measured: putting `&& false &&` in front of
/// each call inside `gate` left 733 library tests, 105 crossings and every
/// integration test passing. The rule this product is named for — *a verdict is
/// bound to exact bytes* — was one edit away from deciding nothing, and nothing
/// would have said so.
///
/// They live behind the tracker read, which is why nothing reached them: a
/// boundary never rides the renewal window, so every path here goes out to `gh`.
/// So the test brings a `gh`.
#[test]
fn the_gate_reaches_the_checks_that_run_after_the_tracker_agrees() {
    // This one carried the reason the others did not: *asserting here would
    // measure the machine rather than the gate*. It is a good reason and it was
    // applied to the wrong thing — a machine without the stand-in cannot measure
    // the gate either way, and returning green says it did.
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let head = rig.head.clone();

    let pointer = |state: &str, reviewed: &str| {
        let run = serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": state,
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
            "review_receipt": {
                "epoch": "a".repeat(32),
                "pr": 12,
                "head": reviewed,
                "base": "b".repeat(40),
                "digest": "c".repeat(64),
            },
            "reviewed_head": serde_json::Value::Null,
        });
        std::fs::write(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
            serde_json::to_string(&run).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let merge = |state: &str, reviewed: &str| {
        forget_store(home);
        pointer(state, reviewed);
        let answers = serde_json::to_string(&serde_json::json!([
            pr_view_entry(12, &head),
            issue_view_entry(state),
        ]))
        .expect("the script serialises");
        let (out, err, _) = run_with_tracker(
            home,
            repo,
            bin,
            &answers,
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-abcd1234",
                "--input",
                r#"{"command":"gh pr merge 12"}"#,
            ],
            "",
        );
        format!("{out}{err}")
    };

    // A verdict published against bytes that have since moved.
    let stale = merge("review", &"0".repeat(40));
    assert!(
        stale.contains("verdict-bound-to-other-bytes"),
        "the gate delivered on a review of other bytes: {stale}"
    );

    // The floor, and it is the whole test: without it, a gate that refused
    // every delivery would pass the assertion above.
    let fresh = merge("review", &head);
    assert!(
        fresh.contains("allow"),
        "a delivery reviewed against these exact bytes was refused: {fresh}"
    );

    // And the phase check, reached through the same seam. `in-progress` is a
    // state where no verdict exists, so landing the work is refused even with
    // the head matching.
    let early = merge("in-progress", &head);
    assert!(
        early.contains("out-of-phase"),
        "the gate landed work from a state where no verdict exists: {early}"
    );
}

/// The control-surface question is asked of a compound shell line, not
/// skipped because a repository-shell fragment matched it first.
///
/// `REPOSITORY_SHELL`'s arm used to return as soon as any of its own fragments
/// appeared in the command, before the two arms below it ever asked whether
/// the line also names a control surface. `git stash` alone answers from the
/// renewal window — that is the floor this test needs, and it is asserted
/// below rather than assumed. `git stash; rm .claude/settings.json` used to
/// answer the same cached way, because `REPOSITORY_SHELL` matched `git stash`
/// and returned before `.claude/settings.json` was ever read. It must instead
/// reach the tracker, the same as a bare write to that path does.
#[test]
fn a_compound_line_reaches_the_control_surface_question() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let pointer = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "state": "in-progress",
        "repo_dir": repo,
        "verified_at": now,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&pointer).expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    // `CONTROL_SURFACE`'s `.claude/settings` entry: this is the fragment the
    // brief measured `rm ~/.claude/settings.json` against, and `.claude/settings.json`
    // matches it after folding just as the home-relative spelling does.
    let surface = ".claude/settings.json";

    let ask = |command: &str| {
        let payload = serde_json::to_string(&serde_json::json!({ "command": command }))
            .expect("the payload serialises");
        let (out, err, _) = run_with_tracker(
            home,
            repo,
            bin,
            &issue_answer("in-progress"),
            &[
                "gate",
                "Bash",
                "--json",
                "--run-id",
                "claude-abcd1234",
                "--input",
                &payload,
            ],
            "",
        );
        let value: serde_json::Value = serde_json::from_str(out.trim())
            .unwrap_or_else(|error| panic!("not JSON: {error}: {out}{err}"));
        value["reason"]
            .as_str()
            .unwrap_or_else(|| panic!("no reason reported: {value}"))
            .to_owned()
    };

    // The floor: an ordinary `git stash` really does ride the renewal window.
    // If this stopped being true, every assertion below would pass with
    // nothing left to distinguish.
    let stash_alone = ask("git stash");
    assert!(
        stash_alone.contains("verified inside the renewal window"),
        "an ordinary repository-shell command stopped riding the renewal window: {stash_alone}"
    );

    // A bare write to the control surface reaches the tracker instead — the
    // read a compound line must not be allowed to skip.
    let write_alone = ask(&format!("rm {surface}"));
    assert!(
        !write_alone.contains("verified inside the renewal window"),
        "a bare control-surface write rode the renewal window: {write_alone}"
    );

    for compound in [
        format!("git stash; rm {surface}"),
        format!("git stash && rm {surface}"),
        format!("git stash\nrm {surface}"),
    ] {
        let answered = ask(&compound);
        assert!(
            !answered.contains("verified inside the renewal window"),
            "`{compound}` answered from the renewal window: {answered}"
        );
    }

    // The other disjunct. `CONTROL_SURFACE` is reached through `surface_of`;
    // the repository configuration is reached through
    // `text_names_the_repository_configuration` instead, and it is the path
    // `docs/configuration.md`'s worked examples are built on rather than the
    // one above.
    let configured = ask("git commit -m wip && printf x >> .git/estigia/estigia.local.md");
    assert!(
        !configured.contains("verified inside the renewal window"),
        "a compound line naming the repository configuration rode the renewal window: {configured}"
    );
}

#[test]
fn a_resumed_worktree_is_the_checkout_whose_reviewed_head_is_spent() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let worktrees = tempfile::tempdir().expect("a parent for the reviewed worktree");
    let reviewed_checkout = worktrees.path().join("reviewed");
    let added = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--quiet", "-b", "reviewed"])
        .arg(&reviewed_checkout)
        .output()
        .expect("git creates the reviewed worktree");
    assert!(
        added.status.success(),
        "the reviewed worktree was not created"
    );
    let committed = Command::new("git")
        .arg("-C")
        .arg(&reviewed_checkout)
        .args([
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--allow-empty",
            "--quiet",
            "-m",
            "reviewed",
        ])
        .output()
        .expect("git advances the reviewed worktree");
    assert!(
        committed.status.success(),
        "the reviewed head was not created"
    );
    let reviewed = Command::new("git")
        .arg("-C")
        .arg(&reviewed_checkout)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the reviewed head");
    let reviewed = String::from_utf8_lossy(&reviewed.stdout).trim().to_owned();
    assert_ne!(reviewed, rig.head, "the two checkouts have the same head");

    // `gh pr merge` is judged by the PR head the tracker reports, scripted
    // here as this worktree's real head — never by the invoking checkout's
    // own `HEAD`. `reviewed_head` below varies the receipt (or the legacy
    // `reviewed_head` field), not the checkout.
    let merge = |reviewed_head: &str, receipt_pr: Option<u64>, command_pr: u64, named: bool| {
        let pointer = serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": "review",
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
            "review_receipt": receipt_pr.map(|pr| serde_json::json!({
                "epoch": "a".repeat(32),
                "pr": pr,
                "head": reviewed_head,
                "base": "b".repeat(40),
                "digest": "c".repeat(64),
            })),
            "reviewed_head": receipt_pr.is_none().then_some(reviewed_head),
        });
        std::fs::write(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
            serde_json::to_string(&pointer).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
        let mut arguments = vec!["gate", "Bash"];
        if named {
            arguments.extend(["--run-id", "claude-abcd1234"]);
        }
        let input = format!(r#"{{"command":"gh pr merge {command_pr}"}}"#);
        arguments.extend(["--input", &input]);
        let answers = serde_json::to_string(&serde_json::json!([
            pr_view_entry(command_pr, &reviewed),
            issue_view_entry("review"),
        ]))
        .expect("the script serialises");
        let (out, err, _) =
            run_with_tracker(home, &reviewed_checkout, bin, &answers, &arguments, "");
        format!("{out}{err}")
    };

    for named in [false, true] {
        let fresh = merge(&reviewed, Some(12), 12, named);
        assert!(
            fresh.contains("allow"),
            "a PR head matching the reviewed one was refused: {fresh}"
        );
        if !named {
            let ledger = decisions(home);
            assert!(
                ledger.contains("claude-abcd1234") && ledger.contains("allow"),
                "the sibling holder's decision was not attributed: {ledger}"
            );
        }

        let stale = merge(&rig.head, Some(12), 12, named);
        assert!(
            stale.contains("verdict-bound-to-other-bytes"),
            "delivery on a PR head that moved past the reviewed one was allowed: {stale}"
        );
        assert!(
            stale.contains(&rig.head[..7]) && stale.contains(&reviewed[..7]),
            "the refusal does not name both the reviewed head and the PR's current head: {stale}"
        );
        assert!(
            !stale.contains(&reviewed_checkout.display().to_string()),
            "the refusal named the invoking checkout rather than the PR head: {stale}"
        );

        let wrong_pr = merge(&reviewed, Some(54), 12, named);
        assert!(
            wrong_pr.contains("delivery-pr-mismatch")
                || wrong_pr.contains("complete-review-transaction-not-selected"),
            "an exact HEAD selected another PR's receipt: {wrong_pr}"
        );

        let legacy = merge(&reviewed, None, 12, named);
        assert!(
            legacy.contains("complete-review-transaction"),
            "a legacy reviewed head qualified PR-targeted delivery: {legacy}"
        );
    }

    // The last negative above deliberately left a legacy pointer on disk.
    // Restore the complete receipt before exercising the unrelated-clone case.
    assert!(
        merge(&reviewed, Some(12), 12, true).contains("allow"),
        "the complete receipt was not restored for the unrelated-clone floor"
    );

    let unrelated_root = tempfile::tempdir().expect("a parent for an unrelated clone");
    let unrelated = unrelated_root.path().join("unrelated");
    let cloned = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(repo)
        .arg(&unrelated)
        .output()
        .expect("git creates an unrelated clone");
    assert!(
        cloned.status.success(),
        "the unrelated clone was not created"
    );
    let checked_out = Command::new("git")
        .arg("-C")
        .arg(&unrelated)
        .args(["checkout", "--quiet", &reviewed])
        .output()
        .expect("git checks out the reviewed head in the unrelated clone");
    assert!(
        checked_out.status.success(),
        "the unrelated clone did not reach the reviewed head"
    );
    let (out, err, _) = run_with_tracker(
        home,
        &unrelated,
        bin,
        &issue_answer("review"),
        &[
            "gate",
            "Bash",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"command":"gh pr merge 12"}"#,
        ],
        "",
    );
    let unrelated = format!("{out}{err}");
    assert!(
        unrelated.contains("verdict-bound-to-other-repository"),
        "a named run delivered its verdict from an unrelated clone: {unrelated}"
    );
}

#[test]
fn a_stale_hook_payload_from_every_retired_host_still_gets_its_dialect_s_answer() {
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ndescription: reads\ntools: Read, Grep, Glob\n---\nRead.\n",
    )
    .expect("a sub-agent definition");
    let cwd = home.path().display().to_string().replace('\\', "\\\\");

    let flat = |tool: &str| {
        format!(
            "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
             \"tool_name\":\"{tool}\",\"tool_input\":{{\"file_path\":\"x.rs\"}}}}"
        )
    };
    let table: [(&str, &str, String); 6] = [
        ("gemini-cli", "\"decision\":\"deny\"", flat("write_file")),
        ("continue", "\"permissionDecision\":\"deny\"", flat("Write")),
        (
            "cline",
            "\"review\":true",
            format!(
                "{{\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\"tool_call\":\
                 {{\"name\":\"Write\",\"input\":{{\"file_path\":\"x.rs\"}}}}}}"
            ),
        ),
        ("crush", "", flat("write")),
        (
            "windsurf",
            "",
            format!(
                "{{\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
                 \"agent_action_name\":\"pre_write_code\",\
                 \"tool_info\":{{\"file_path\":\"x.rs\"}}}}"
            ),
        ),
        ("qwen", "\"decision\":\"deny\"", flat("write_file")),
    ];

    let slugs: Vec<&str> = table.iter().map(|(slug, ..)| *slug).collect();
    let retired: Vec<&str> = estigia::setup::RETIRED_AGENTS
        .iter()
        .map(|adapter| adapter.slug)
        .collect();
    assert_eq!(slugs, retired, "a retired host has no payload here");

    for (slug, marker, payload) in &table {
        let dialect = match *slug {
            "gemini-cli" | "qwen" => "gemini-cli",
            "continue" => "claude-code",
            "cline" => "cline",
            _ => "exit-code",
        };
        let (out, stderr, ok) = run(
            home.path(),
            &[
                "hook",
                "pre-tool-use",
                "--agent",
                slug,
                "--dialect",
                dialect,
            ],
            payload,
        );
        assert!(
            !stderr.contains("not a dialect"),
            "{slug}: the dialect was not known: {stderr}"
        );
        if dialect == "exit-code" {
            assert!(!ok, "{slug}: an exit-code host was told to proceed");
            assert!(out.trim().is_empty(), "{slug}: printed a status: {out}");
            assert!(
                stderr.contains("tool-outside-declared-role"),
                "{slug}: the reason reached nobody: {stderr}"
            );
        } else {
            assert!(ok, "{slug}: the hook did not exit zero: {stderr}");
            assert!(out.contains(marker), "{slug}: answered {out}");
            assert!(
                out.contains("tool-outside-declared-role"),
                "{slug}: the reason reached nobody: {out}"
            );
        }
    }
}

fn home_with_an_unreadable_sub_agent_definition() -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    std::fs::create_dir_all(
        home.path()
            .join(".opencode")
            .join("agents")
            .join("reader.md"),
    )
    .expect("something unreadable where the definition should be");
    home
}

fn stand_down_in(home: &std::path::Path) {
    let (_, error, declared) = run(
        home,
        &["stand-down", "--reason", "measuring", "--minutes", "5"],
        "",
    );
    assert!(declared, "the stand-down was refused: {error}");
}

#[test]
fn the_hook_denies_a_sub_agents_stand_down_through_an_unreadable_definition() {
    let home = home_with_an_unreadable_sub_agent_definition();
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let call = |command: &str| {
        let payload = format!(
            "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
             \"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{command:?}}}}}"
        );
        run(home.path(), &["hook", "pre-tool-use"], &payload).0
    };

    let before = call("estigia stand-down --reason x");
    assert!(
        before.contains("stand-down-is-for-a-person"),
        "no stand-down is in force and the deny is not this one: {before}"
    );

    stand_down_in(home.path());

    let ordinary = call("ls");
    assert!(
        ordinary.contains("\"permissionDecision\":\"allow\""),
        "the stand-down no longer lifts the role refusal, so the half below proves nothing: \
         {ordinary}"
    );
    let declared = call("estigia stand-down --reason x");
    assert!(
        declared.contains("\"permissionDecision\":\"deny\"")
            && declared.contains("stand-down-is-for-a-person"),
        "a sub-agent declared a stand-down past its unreadable definition: {declared}"
    );
}

#[test]
fn the_gate_door_denies_a_sub_agents_stand_down_through_an_unreadable_definition() {
    let home = home_with_an_unreadable_sub_agent_definition();
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let ask = |command: &str| {
        let payload =
            format!("{{\"agent_type\":\"reader\",\"command\":{command:?},\"cwd\":\"{cwd}\"}}");
        run_in(
            home.path(),
            home.path(),
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };

    let (said, error, ok) = ask("estigia stand-down --reason x");
    assert!(
        !ok && format!("{said}{error}").contains("stand-down-is-for-a-person"),
        "no stand-down is in force and the deny is not this one: {said}{error}"
    );

    stand_down_in(home.path());

    let (said, error, ok) = ask("ls");
    assert!(
        ok,
        "the stand-down no longer lifts the role refusal, so the half below proves nothing: \
         {said}{error}"
    );
    let (said, error, ok) = ask("estigia stand-down --reason x");
    assert!(
        !ok && format!("{said}{error}").contains("stand-down-is-for-a-person"),
        "a sub-agent declared a stand-down past its unreadable definition: {said}{error}"
    );
}

#[test]
fn a_refused_person_only_call_is_in_the_ledger_on_the_hook_road() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"tool_name\":\"Bash\",\
         \"tool_input\":{{\"command\":\"estigia stand-down --reason x\"}}}}"
    );
    let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload);
    assert!(out.contains("stand-down-is-for-a-person"), "{out}");
    let ledger = decisions(home.path());
    assert!(
        ledger.contains("deny") && ledger.contains("stand-down-is-for-a-person"),
        "the refused call left no ledger entry: {ledger:?}"
    );
}

#[test]
fn a_refused_person_only_call_is_in_the_ledger_on_the_gate_road() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let payload = r#"{"command":"echo x > ~/.estigia/stand-down.json"}"#;
    let (said, error, ok) = run(
        home.path(),
        &[
            "gate",
            "Bash",
            "--run-id",
            "claude-aaaa1111",
            "--input",
            payload,
        ],
        "",
    );
    assert!(
        !ok && format!("{said}{error}").contains("stand-down-is-for-a-person"),
        "{said}{error}"
    );
    let ledger = decisions(home.path());
    assert!(
        ledger.contains("deny") && ledger.contains("stand-down-is-for-a-person"),
        "the refused call left no ledger entry: {ledger:?}"
    );
}

#[test]
fn a_sub_agent_writing_the_record_is_denied_through_an_unreadable_definition() {
    let home = home_with_an_unreadable_sub_agent_definition();
    stand_down_in(home.path());
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let target = home
        .path()
        .join(".estigia")
        .join("stand-down.json")
        .display()
        .to_string()
        .replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
         \"tool_name\":\"Write\",\"tool_input\":{{\"file_path\":\"{target}\",\"content\":\"{{}}\"}}}}"
    );
    let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload);
    assert!(
        out.contains("\"permissionDecision\":\"deny\"")
            && out.contains("stand-down-is-for-a-person"),
        "{out}"
    );
}

#[test]
fn an_agents_lift_is_refused_with_a_way_out_that_lifts() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    stand_down_in(home.path());
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"tool_name\":\"Bash\",\
         \"tool_input\":{{\"command\":\"estigia stand-down --lift --reason cleared\"}}}}"
    );
    let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload);
    assert!(
        out.contains("stand-down-is-for-a-person") && out.contains("--lift --reason"),
        "{out}"
    );
}

const RECOVERY: &str = "estigia recover 7 --reason r";

#[test]
fn the_hook_denies_a_sub_agents_recovery_through_an_unreadable_definition() {
    let home = home_with_an_unreadable_sub_agent_definition();
    stand_down_in(home.path());
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
         \"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{RECOVERY:?}}}}}"
    );
    let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload);
    assert!(
        out.contains("\"permissionDecision\":\"deny\"") && out.contains("recover-is-for-a-person"),
        "{out}"
    );
}

#[test]
fn the_gate_door_denies_a_sub_agents_recovery_through_an_unreadable_definition() {
    let home = home_with_an_unreadable_sub_agent_definition();
    stand_down_in(home.path());
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload =
        format!("{{\"agent_type\":\"reader\",\"command\":{RECOVERY:?},\"cwd\":\"{cwd}\"}}");
    let (said, error, ok) = run_in(
        home.path(),
        home.path(),
        &[
            "gate",
            "Bash",
            "--run-id",
            "claude-aaaa1111",
            "--input",
            &payload,
        ],
        "",
    );
    assert!(
        !ok && format!("{said}{error}").contains("recover-is-for-a-person"),
        "{said}{error}"
    );
}

#[test]
fn gate_reads_an_input_larger_than_the_argument_limit_from_standard_input() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");
    let repo = tempfile::tempdir().expect("a checkout");
    let cwd = repo.path().display().to_string();

    let small = format!("{{\"file_path\":\"src/x.rs\",\"content\":\"x\",\"cwd\":{cwd:?}}}");
    let by_argument = run_in(
        home.path(),
        repo.path(),
        &[
            "gate",
            "Write",
            "--run-id",
            "claude-aaaa1111",
            "--input",
            &small,
        ],
        "",
    );
    let by_stdin = run_in(
        home.path(),
        repo.path(),
        &[
            "gate",
            "Write",
            "--run-id",
            "claude-aaaa1111",
            "--input",
            "-",
        ],
        &small,
    );
    assert_eq!(by_argument, by_stdin, "standard input changed the decision");

    let huge = format!(
        "{{\"file_path\":\"src/x.rs\",\"content\":\"{}\",\"cwd\":{cwd:?}}}",
        "x".repeat(4 * 1024 * 1024)
    );
    let by_huge_stdin = run_in(
        home.path(),
        repo.path(),
        &[
            "gate",
            "Write",
            "--run-id",
            "claude-aaaa1111",
            "--input",
            "-",
        ],
        &huge,
    );
    assert_eq!(
        by_huge_stdin, by_argument,
        "a 4 MiB input on standard input was decided differently from the small one"
    );
}

#[test]
fn gate_refuses_standard_input_over_the_limit_and_standard_input_that_is_not_utf8() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let arguments = [
        "gate",
        "Write",
        "--run-id",
        "claude-aaaa1111",
        "--input",
        "-",
    ];
    let over_the_limit = vec![b' '; 32 * 1024 * 1024 + 1];
    let not_utf8 = b"{\"file_path\":\"\xff\xfe\"}".to_vec();
    for (stdin, reason) in [
        (over_the_limit, "more than 33554432 bytes"),
        (not_utf8, "not UTF-8"),
    ] {
        let (said, error, ok) = run_in_with_bytes(home.path(), home.path(), &arguments, &stdin);
        let shown = format!("{said}{error}");
        assert!(!ok, "{reason}: was allowed: {shown}");
        assert!(
            shown.contains("gate-input-not-json") && shown.contains(reason),
            "{reason}: was refused for another reason: {shown}"
        );
    }
}

#[test]
fn gate_refuses_an_input_on_standard_input_that_is_not_json_or_is_missing() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    for stdin in ["", "not json", "{\"file_path\":"] {
        let (said, error, ok) = run_in(
            home.path(),
            home.path(),
            &[
                "gate",
                "Write",
                "--run-id",
                "claude-aaaa1111",
                "--input",
                "-",
            ],
            stdin,
        );
        assert!(!ok, "{stdin:?} was allowed: {said}{error}");
        assert!(
            format!("{said}{error}").contains("not JSON"),
            "{stdin:?} was refused for another reason: {said}{error}"
        );
    }
}
