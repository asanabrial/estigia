/// The transport's source: the dispatcher that answers in this process — the
/// flags a tool sends are checked against the code that reads them, not
/// against a published schema alone.
fn transport_source() -> String {
    crate::source_text::code_of(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("transport")
                .join("dispatch.rs"),
        )
        .expect("the dispatcher is part of this crate"),
    )
}

/// Every `.rs` file of the transport, one entry each and in path order, so a
/// body can be found wherever it is without one file's text running into the
/// next.
fn transport_sources() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("transport");
    let mut paths = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs")
                && path.file_name().is_some_and(|name| name != "tests.rs")
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| crate::source_text::code_of(&std::fs::read_to_string(path).unwrap_or_default()))
        .collect()
}

/// The body of `pub fn <name>` inside one file's text, up to the next public
/// function of that same file or its end.
fn function_body_in(source: &str, name: &str) -> Option<String> {
    let padded = format!("\n{source}");
    let opens = format!("\npub fn {name}(");
    let start = padded.find(&opens)?;
    let rest = &padded[start + 1..];
    Some(
        rest.find("\npub fn ")
            .map_or(rest, |at| &rest[..at])
            .to_owned(),
    )
}

/// The body of the function one operation runs, wherever the dispatcher sends it.
///
/// The dispatcher is a table of one-line arms, so *what does this operation
/// do* is never in the arm — it is under the name the arm calls, in
/// `commands.rs`, `claim.rs`, `branch.rs` or `board.rs`.
fn transport_body(operation: &str) -> String {
    let Some(arm) = transport_block(operation) else {
        return String::new();
    };
    let Some(at) = arm.find("super::") else {
        return String::new();
    };
    let called: String = arm[at + "super::".len()..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    let Some(name) = called.rsplit("::").next() else {
        return String::new();
    };
    transport_sources()
        .iter()
        .find_map(|source| function_body_in(source, name))
        .unwrap_or_default()
}

/// One operation's arm of the dispatcher, from its name to the next one.
fn transport_block(operation: &str) -> Option<String> {
    let source = transport_source();
    let opens = format!("\n        \"{operation}\" =>");
    let start = source.find(&opens)?;
    let rest = source[start + 1..].to_owned();
    let end = rest[1..]
        .find("\n        \"")
        .map_or(rest.len(), |at| at + 1);
    Some(rest[..end].to_owned())
}

/// Every flag an arm names, and how it reads each one.
///
/// The dispatcher reads its arguments through one type, so the shape of a
/// flag is the method that asks for it: `need` and `number` refuse without
/// it, `number` parses it as a whole number, `on` takes no value at all.
fn transport_reads(operation: &str) -> Vec<(String, &'static str)> {
    let Some(block) = transport_block(operation) else {
        return Vec::new();
    };
    let mut found: Vec<(String, &'static str)> = Vec::new();
    for (call, how) in [
        ("f.need(\"", "need"),
        ("f.number(\"", "number"),
        ("f.identity(\"", "need"),
        ("f.on(\"", "on"),
        ("f.get(\"", "get"),
        ("f.path(\"", "path"),
    ] {
        let mut cursor = block.as_str();
        while let Some(at) = cursor.find(call) {
            cursor = &cursor[at + call.len()..];
            let Some(close) = cursor.find('"') else { break };
            note(&mut found, &cursor[..close], how);
        }
    }
    // A flag the arm parses itself, such as `list-state`'s `--limit`: no
    // `f.number`, because it defaults rather than refusing, but it is an
    // integer to every caller. Reading only the method left the schema
    // declaring `integer` against a transport this said took a string.
    if block.contains("parse::<u32>") || block.contains("parse::<u64>") {
        let mut cursor = block.as_str();
        while let Some(at) = cursor.find("f.get(\"") {
            cursor = &cursor[at + "f.get(\"".len()..];
            let Some(close) = cursor.find('"') else { break };
            let name = cursor[..close].to_owned();
            if cursor[close..]
                .split("f.get(")
                .next()
                .is_some_and(|window| {
                    window.contains("parse::<u32>") || window.contains("parse::<u64>")
                })
            {
                note(&mut found, &name, "int");
            }
        }
    }
    if block.contains("issue()") {
        note(&mut found, "issue", "number");
    }
    // `assist` reads its issue through `identity()`, which takes a number or a
    // Linear key: a required string, not a number.
    if block.contains("identity()") {
        note(&mut found, "issue", "need");
    }
    if block.contains("run_id()") {
        note(&mut found, "run-id", "need");
    }
    found
}

/// Adds one flag, spelled the way a caller writes it, keeping the firmest read.
///
/// One flag can be asked for twice in an arm, and the two calls can say
/// different things about whether the transport will run without it. The
/// firmer answer is the one that decides: a flag some path demands is a flag
/// the tool has to send.
fn note(found: &mut Vec<(String, &'static str)>, name: &str, how: &'static str) {
    let flag = format!("--{name}");
    let firmness = |how: &str| match how {
        "need" | "number" => 3,
        "int" => 2,
        "on" => 1,
        _ => 0,
    };
    match found.iter_mut().find(|(known, _)| *known == flag) {
        Some(slot) if firmness(how) > firmness(slot.1) => slot.1 = how,
        Some(_) => {}
        None => found.push((flag, how)),
    }
}

use super::*;

use crate::test_env::repository_with_commit;

/// Registers a real worktree of `repo` at `path`, and writes an ownership
/// marker inside it naming `run_id` and `issue` — the same shape
/// `write_ownership` leaves behind, so
/// [`crate::transport::branch::discover_adopted_worktree`] reads exactly
/// what a real `start_branch` would have written.
fn register_adopted_worktree(
    repo: &std::path::Path,
    path: &std::path::Path,
    branch: &str,
    run_id: &str,
    issue: u64,
) {
    let added = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "-b", branch])
        .arg(path)
        .output()
        .is_ok_and(|output| output.status.success());
    assert!(added, "git could not add the worktree this fixture needs");
    let marker = crate::transport::branch::ownership_path(path)
        .expect("git names the admin directory of a worktree it just created");
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).expect("the marker's directory exists");
    }
    std::fs::write(
        &marker,
        serde_json::json!({
            "run_id": run_id,
            "issue": issue,
            "branch": branch,
            "worktree": path.display().to_string(),
            "claimed_at": "2026-01-01T00:00:00Z",
        })
        .to_string(),
    )
    .expect("the ownership marker is written");
}

/// A `GateContext` with nothing installed, over a real `repo_dir` — the shape
/// every discovery test needs and none of them differ on.
fn bare_context(root: &tempfile::TempDir, repo_dir: &std::path::Path) -> GateContext {
    GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: repo_dir.to_path_buf(),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

/// Drives the server the way a client does, and returns what it said.
fn ask(request: Value) -> Option<Value> {
    handle_line(&request.to_string(), Err(&crate::skill::no_skill_root()))
}

fn result(request: Value) -> Value {
    ask(request)
        .expect("a request with an id gets a response")
        .get("result")
        .cloned()
        .expect("a successful call has a result")
}

/// Every method this server answers is one somebody crossed.
///
/// The list is read out of the dispatch rather than written here, so a
/// method added later is crossed by this test or fails it.
#[test]
fn every_method_this_server_implements_is_answered() {
    let source = crate::source_text::code_of(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/harness/mcp/mod.rs"),
        )
        .expect("this module's own source"),
    );
    let dispatch = source
        .split_once("Some(match method {")
        .expect("the dispatch is where it was")
        .1
        .split_once("    })")
        .expect("the dispatch ends")
        .0;
    let methods: Vec<String> = dispatch
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"')?.split_once('"'))
        .map(|(name, _)| name.to_owned())
        .collect();
    // The floor: a dispatch read as empty would pass every assertion below
    // without measuring one of them.
    assert!(
        methods.len() >= 4 && methods.iter().any(|name| name == "ping"),
        "the dispatch was not read: {methods:?}"
    );

    for method in &methods {
        let answer = ask(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": {},
        }))
        .unwrap_or_else(|| panic!("{method} was answered with silence"));
        // Dispatched, not necessarily satisfied: `tools/call` with no name is
        // an honest `invalid params`. What must not come back is *this
        // server does not implement that*, which is what a deleted arm
        // produces.
        assert_ne!(
            answer["error"]["code"],
            super::code::METHOD_NOT_FOUND,
            "{method} is dispatched by name and answered as though it were not: {answer}"
        );
    }

    assert_eq!(
        ask(json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}})).expect("an answer")
            ["result"],
        json!({}),
        "ping does not answer, so a client checking whether the server is there is told it is not"
    );

    let answer = ask(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/list",
        "params": {},
    }))
    .expect("an answer");
    assert_eq!(
        answer["error"]["code"],
        super::code::METHOD_NOT_FOUND,
        "a method this server does not implement was answered as though it did: {answer}"
    );
}

#[test]
fn the_wire_constants_are_the_ones_the_specification_fixes() {
    assert_eq!(
        super::PROTOCOL_VERSION,
        "2025-06-18",
        "the revision this server implements moved without anybody saying so"
    );
    assert_eq!(super::code::PARSE_ERROR, -32700, "JSON-RPC 2.0 parse error");
    assert_eq!(
        super::code::INVALID_REQUEST,
        -32600,
        "JSON-RPC 2.0 invalid request"
    );
    assert_eq!(
        super::code::METHOD_NOT_FOUND,
        -32601,
        "JSON-RPC 2.0 method not found"
    );
    assert_eq!(
        super::code::INVALID_PARAMS,
        -32602,
        "JSON-RPC 2.0 invalid params"
    );

    let codes = [
        super::code::PARSE_ERROR,
        super::code::INVALID_REQUEST,
        super::code::METHOD_NOT_FOUND,
        super::code::INVALID_PARAMS,
    ];
    let distinct: std::collections::BTreeSet<i64> = codes.iter().copied().collect();
    assert_eq!(distinct.len(), codes.len(), "two errors share a number");
}

#[test]
fn a_claim_records_the_key_its_retry_depends_on() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::create_dir_all(&context.repo_dir).expect("the checkout");

    let pointer = context.state_root.join("claude-swearing0a1b2c3d.json");
    let write = |issue: Option<u64>, operation_id: Option<&str>| {
        let mut run = crate::harness::session::Run::new("claude-swearing0a1b2c3d".to_owned());
        run.issue = issue.map(crate::identity::Identity::Number);
        run.repo_dir = Some(context.repo_dir.clone());
        run.operation_id = operation_id.map(ToOwned::to_owned);
        std::fs::write(
            &pointer,
            serde_json::to_string(&run).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let key_now = || {
        let text = std::fs::read_to_string(&pointer).expect("the pointer reads");
        serde_json::from_str::<serde_json::Value>(&text).expect("it is JSON")["operation_id"]
            .clone()
    };
    let swear = |issue: u64| {
        let _ = run_tool(
            "claim",
            &json!({"issue": issue, "run_id": "claude-swearing0a1b2c3d", "horizon": "2026-08-06T10:00Z"}),
            Ok(&context),
        );
    };

    write(None, None);
    swear(12);
    let first = key_now();
    assert!(
        first.as_str().is_some_and(|key| key.len() == 32),
        "a claim recorded no key, so the transport is being sent a call it requires \
         `--operation-id` for: {first}"
    );

    write(Some(12), first.as_str());
    swear(12);
    assert_eq!(
        key_now(),
        first,
        "a retried claim minted a second key, and the transport reads a repeated \
         claim only when the key is the same one"
    );
}

#[test]
fn releasing_another_issue_leaves_the_held_ones_retry_key_alone() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::create_dir_all(&context.repo_dir).expect("the checkout");

    let minted = "a".repeat(32);
    let pointer = context.state_root.join("claude-holder0a1b2c3d.json");
    let write = |release_id: Option<&str>| {
        let mut run = crate::harness::session::Run::new("claude-holder0a1b2c3d".to_owned());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.repo_dir = Some(context.repo_dir.clone());
        run.release_id = release_id.map(ToOwned::to_owned);
        std::fs::write(
            &pointer,
            serde_json::to_string(&run).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let key_now = || {
        let text = std::fs::read_to_string(&pointer).expect("the pointer reads");
        serde_json::from_str::<serde_json::Value>(&text).expect("it is JSON")["release_id"].clone()
    };

    write(Some(&minted));
    let _ = run_tool(
        "release",
        &json!({"issue": 99, "run_id": "claude-holder0a1b2c3d"}),
        Ok(&context),
    );
    assert_eq!(
        key_now(),
        serde_json::Value::String(minted),
        "a release naming another issue overwrote the held issue's retry key"
    );

    // The floor: the held issue still gets one written down, or the dedupe
    // this protects never happens and the assertion above guards an empty
    // field.
    write(None);
    let _ = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-holder0a1b2c3d"}),
        Ok(&context),
    );
    let stored = key_now();
    assert!(
        stored.as_str().is_some_and(|key| key.len() == 32),
        "releasing the held issue no longer records a key to retry under: {stored}"
    );

    let first = stored.as_str().expect("a key").to_owned();
    let _ = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-holder0a1b2c3d"}),
        Ok(&context),
    );
    assert_eq!(
        key_now(),
        serde_json::Value::String(first),
        "a second release of the held issue minted a new key, so the transport cannot see it \
         as a repeat"
    );
}

#[test]
fn an_argument_declared_an_integer_is_refused_when_it_is_not_one() {
    let tool = crate::harness::mcp::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .expect("`claim` is a tool this server exposes");
    let integer = tool
        .arguments
        .iter()
        .find(|argument| argument.json_type == "integer")
        .expect("`claim` takes an issue number");

    let with = |value: serde_json::Value| {
        let mut arguments = serde_json::json!({
            "run_id": "claude-aaaa1111",
            "horizon": "2099-01-01T00:00Z",
        });
        arguments[integer.name] = value;
        super::flags_for(tool, &arguments)
    };

    // The floor: a whole number still goes through, spelled as a whole number.
    let flags = with(serde_json::json!(12)).expect("an issue number was refused");
    assert!(
        flags.iter().any(|flag| flag == "12"),
        "the issue number no longer reaches the transport: {flags:?}"
    );

    for value in [
        serde_json::json!("twelve"),
        serde_json::json!("12"),
        serde_json::json!(12.5),
        serde_json::json!(true),
    ] {
        let refusal = with(value.clone()).expect_err(&format!(
            "{value} passed for an integer and went to the transport"
        ));
        let super::ToolFailure::Malformed(message) = refusal else {
            panic!("{value} was refused as something other than a malformed call");
        };
        assert!(
            message.contains(integer.name),
            "the refusal does not name the argument that is wrong: {message}"
        );
    }
}

#[test]
fn a_record_nothing_can_read_stops_every_tool_that_names_a_run() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::write(
        context.state_root.join("claude-torn0a1b2c3d.json"),
        "{\"run_id\": \"claude-torn0a1b2c3d\", \"issue\":",
    )
    .expect("a torn pointer");

    let mut bound = 0;
    for tool in super::tools::TOOLS {
        if !tool
            .arguments
            .iter()
            .any(|argument| argument.name == "run_id")
        {
            continue;
        }
        bound += 1;
        let mut arguments = serde_json::Map::new();
        for argument in tool.arguments {
            arguments.insert(
                argument.name.to_owned(),
                match argument.json_type {
                    "integer" => json!(12),
                    "boolean" => json!(true),
                    _ => argument
                        .choices
                        .map_or_else(|| json!("x"), |choices| json!(choices[0])),
                },
            );
        }
        arguments.insert("run_id".to_owned(), json!("claude-torn0a1b2c3d"));

        let failure = run_tool(tool.name, &Value::Object(arguments), Ok(&context))
            .expect_err("a run whose record cannot be read is not one to act under");
        let ToolFailure::Refused(refusal) = failure else {
            panic!(
                "{}: refused as malformed rather than by the rule",
                tool.name
            );
        };
        assert_eq!(
            refusal.code, "run-pointer-unreadable",
            "{} was refused for something else first: {refusal}",
            tool.name
        );
    }
    assert!(
        bound >= 7,
        "only {bound} tools were seen to take a run id, so this checked almost nothing"
    );

    let readable = crate::harness::session::Run::new("claude-whole0a1b2c3d".to_owned());
    std::fs::write(
        context.state_root.join("claude-whole0a1b2c3d.json"),
        serde_json::to_string(&readable).expect("the pointer serialises"),
    )
    .expect("a readable pointer");
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "claim",
        &json!({"issue": 12, "run_id": "claude-whole0a1b2c3d", "horizon": "2099-01-01T00:00Z"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-pointer-unreadable",
            "a run whose record reads perfectly was told nothing could read it"
        );
    }
}

#[test]
fn a_record_that_will_not_open_stops_every_tool_that_names_a_run() {
    // The reduced reproduction: the pointer is there in name, and the
    // filesystem refuses to read it — a directory at its path — while the
    // tracker timeline can still name that run as holder. The loader used to
    // answer "this run swore nothing" for that, and a tool call then went to
    // the tracker unchecked.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::create_dir_all(context.state_root.join("claude-shut0a1b2c3d.json"))
        .expect("a directory where the pointer should be");

    let mut bound = 0;
    for tool in super::tools::TOOLS {
        if !tool
            .arguments
            .iter()
            .any(|argument| argument.name == "run_id")
        {
            continue;
        }
        bound += 1;
        let mut arguments = serde_json::Map::new();
        for argument in tool.arguments {
            arguments.insert(
                argument.name.to_owned(),
                match argument.json_type {
                    "integer" => json!(12),
                    "boolean" => json!(true),
                    _ => argument
                        .choices
                        .map_or_else(|| json!("x"), |choices| json!(choices[0])),
                },
            );
        }
        arguments.insert("run_id".to_owned(), json!("claude-shut0a1b2c3d"));

        let failure = run_tool(tool.name, &Value::Object(arguments), Ok(&context))
            .expect_err("a run whose record will not open is not one to act under");
        let ToolFailure::Refused(refusal) = failure else {
            panic!(
                "{}: refused as malformed rather than by the rule",
                tool.name
            );
        };
        assert_eq!(
            refusal.code, "run-pointer-unreadable",
            "{} was refused for something else first: {refusal}",
            tool.name
        );
        assert!(
            refusal.to_string().contains("claude-shut0a1b2c3d.json"),
            "{}: the refusal names no pointer path, so nobody can find the record: {refusal}",
            tool.name
        );
    }
    assert!(
        bound >= 7,
        "only {bound} tools were seen to take a run id, so this checked almost nothing"
    );
}

#[test]
fn a_run_id_that_holds_another_checkout_is_not_one_to_act_under() {
    let root = tempfile::tempdir().expect("a temporary root");
    let here = root.path().join("here");
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir_all(&here).expect("this checkout");
    std::fs::create_dir_all(&elsewhere).expect("the other one");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: here.clone(),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");

    let pointer =
        |run_id: &str, repo: Option<&std::path::Path>, worktree: Option<&std::path::Path>| {
            let mut run = crate::harness::session::Run::new(run_id.to_owned());
            run.issue = Some(crate::identity::Identity::Number(12));
            run.repo_dir = repo.map(std::path::Path::to_path_buf);
            run.worktree = worktree.map(std::path::Path::to_path_buf);
            std::fs::write(
                context.state_root.join(format!("{run_id}.json")),
                serde_json::to_string(&run).expect("the pointer serialises"),
            )
            .expect("the pointer is written");
        };

    pointer("claude-other0a1b2c3d", Some(&elsewhere), None);
    let taking = json!({"issue": 12, "run_id": "claude-other0a1b2c3d"});
    let failure = run_tool("release", &taking, Ok(&context))
        .expect_err("a release under another checkout's run is not this run's to make");
    let ToolFailure::Refused(refusal) = failure else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "run-id-names-another-checkout");
    assert!(
        refusal.to_string().contains("SessionStart"),
        "the refusal does not say which id would have been this run's: {refusal}"
    );

    // The floors, because a rule that refuses everything would pass the
    // assertion above and take the product with it: a run holding this
    // checkout is not refused by this rule, and goes on to fail for its own
    // reasons — there is no tracker here.
    pointer("claude-here0a1b2c3d", Some(&here), None);
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-here0a1b2c3d"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run acting in its own checkout was told it was somebody else's"
        );
    }

    pointer("claude-split0a1b2c3d", Some(&elsewhere), Some(&here));
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-split0a1b2c3d"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run was refused in the worktree its own claim covers"
        );
    }

    pointer("claude-worktree0a1b2c3d", None, Some(&elsewhere));
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-worktree0a1b2c3d"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run whose record never named its checkout was refused the way back"
        );
    }

    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "claim",
        &json!({"issue": 12, "run_id": "claude-fresh0a1b2c3d", "horizon": "2099-01-01T00:00Z"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run with nothing claimed yet was refused for holding somebody else's checkout"
        );
    }

    let deep = here.join("src").join("deep");
    std::fs::create_dir_all(&deep).expect("a subdirectory of this checkout");
    let below = GateContext {
        repo_dir: deep,
        ..context.clone()
    };
    pointer("claude-below0a1b2c3d", Some(&here), None);
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-below0a1b2c3d"}),
        Ok(&below),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a call from inside the claimed checkout was called somebody else's"
        );
    }
}

#[test]
fn a_run_whose_pointer_will_not_parse_cannot_swear_to_another_issue() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::write(
        context.state_root.join("claude-torn50a1b2c3d.json"),
        "{\"run_id\": \"claude-torn50a1b2c3d\", \"issue\":",
    )
    .expect("a half-written pointer");

    let failure = run_tool(
        "claim",
        &json!({"issue": 7, "run_id": "claude-torn50a1b2c3d", "horizon": "2026-12-31T23:00Z"}),
        Ok(&context),
    )
    .expect_err("a run whose pointer will not parse swore to another issue");

    match failure {
        ToolFailure::Refused(refusal) => {
            assert_eq!(refusal.code, "run-pointer-unreadable");
            assert!(matches!(
                refusal.resolution,
                crate::outcome::Resolution::NoCommand { .. }
            ));
        }
        other => panic!("refused for the wrong reason: {other:?}"),
    }
}

#[test]
fn the_handshake_answers_with_this_server_s_identity() {
    let response = result(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": PROTOCOL_VERSION, "capabilities": {}}
    }));
    assert_eq!(response["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(response["serverInfo"]["name"], crate::NAME);
    assert_eq!(
        response["serverInfo"]["version"],
        crate::build_info::VERSION
    );
    assert_eq!(response["capabilities"]["tools"]["listChanged"], false);
}

#[test]
fn an_unknown_protocol_version_gets_this_server_s_own_rather_than_an_echo() {
    let response = result(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "1999-01-01"}
    }));
    assert_eq!(response["protocolVersion"], PROTOCOL_VERSION);

    let bare = result(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
    }));
    assert_eq!(bare["protocolVersion"], PROTOCOL_VERSION);
    assert!(
        bare.get("serverInfo").is_some(),
        "a client that named no version got half an answer: {bare}"
    );
}

#[test]
fn a_notification_is_answered_with_silence() {
    assert!(ask(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
}

#[test]
fn malformed_json_is_a_parse_error_rather_than_a_crash() {
    let response =
        handle_line("{not json", Err(&crate::skill::no_skill_root())).expect("an answer");
    assert_eq!(response["error"]["code"], code::PARSE_ERROR);
}

#[test]
fn an_unimplemented_method_says_so_by_name() {
    let response =
        ask(json!({"jsonrpc": "2.0", "id": 7, "method": "resources/list"})).expect("an answer");
    assert_eq!(response["error"]["code"], code::METHOD_NOT_FOUND);
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|text| text.contains("resources/list"))
    );
}

#[test]
fn every_tool_is_listed_with_a_usable_schema() {
    let listed = result(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let listed = listed["tools"].as_array().expect("a list of tools");
    assert_eq!(listed.len(), TOOLS.len());
    for tool in listed {
        assert!(tool["name"].as_str().is_some_and(|name| !name.is_empty()));
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| text.len() > 20),
            "{} has no usable description",
            tool["name"]
        );
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert!(tool["inputSchema"]["required"].is_array());
    }
    for (name, _) in NOT_EXPOSED {
        assert!(
            !listed.iter().any(|tool| tool["name"] == *name),
            "`{name}` is declared without a tool of its own and must not be advertised: a \
             client that sees it offers a call no tool performs"
        );
    }
}

/// A name declared without a tool of its own is not advertised.
///
/// The declaration exists for the seam test that holds the contract's required
/// operations against the tools; a caller reaches the capability through the
/// tool that absorbed it, so a second schema would offer a call no tool
/// performs.
#[test]
fn every_unexposed_operation_is_not_advertised() {
    let listed = result(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let listed = listed["tools"].as_array().expect("a list of tools");
    for (name, reason) in NOT_EXPOSED {
        assert!(
            !listed.iter().any(|tool| tool["name"] == *name),
            "`{name}` is declared without a tool of its own and is advertised anyway"
        );
        assert!(reason.len() > 20, "`{name}` is declared without saying why");
    }
}

/// Asking for an unexposed operation answers with why no tool
/// exists **and** where the contract carries it — not with the client's
/// `No such tool available`, and not with a reasonless refusal.
#[test]
fn every_unexposed_operation_refuses_with_its_reason_and_the_contract_location() {
    for (name, reason) in NOT_EXPOSED {
        let response = result(json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": name, "arguments": {}}
        }));
        assert_eq!(response["isError"], true, "`{name}` did not refuse");
        let text = response["content"][0]["text"]
            .as_str()
            .expect("a text result");
        assert!(
            text.contains(reason),
            "`{name}` refused without its reason: {text}"
        );
        assert!(
            text.contains("bindings/"),
            "`{name}` refused without naming where the contract carries the operation: {text}"
        );
    }
}

/// A name that exists only to refuse refuses every call, whatever
/// arguments it is handed — no argument reaches a dispatch that could act.
#[test]
fn a_name_that_only_refuses_refuses_whatever_it_is_given() {
    for (name, _) in NOT_EXPOSED {
        for arguments in [
            json!({}),
            json!({"issue": 1}),
            json!({"anything": "at all"}),
        ] {
            let response = result(json!({
                "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                "params": {"name": name, "arguments": arguments}
            }));
            assert_eq!(
                response["isError"], true,
                "`{name}` with {arguments} did not refuse"
            );
        }
    }
}

/// `reclaim` is declared without a tool of its own, and a caller
/// that names it is refused with the reason and pointed at the tool that
/// absorbed it — `claim` with `takeover` — rather than at a dead end.
#[test]
fn reclaim_over_mcp_refuses_with_its_reason_and_names_the_tool_that_absorbed_it() {
    let response = result(json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": {"name": "reclaim", "arguments": {"issue": 1, "run_id": "claude-a"}}
    }));
    assert_eq!(response["isError"], true, "{response}");
    let text = response["content"][0]["text"]
        .as_str()
        .expect("a text result");
    assert!(text.contains("claim"), "{text}");
    assert!(text.contains("takeover"), "{text}");
    assert!(text.contains("bindings/github.md"), "{text}");
}

#[test]
fn a_read_only_tool_says_so_in_its_schema() {
    let read_only = tools::find("verify_claim").expect("verify_claim exists");
    let writing = tools::find("claim").expect("claim exists");
    assert_eq!(read_only.schema()["annotations"]["readOnlyHint"], true);
    assert_eq!(writing.schema()["annotations"]["readOnlyHint"], false);
}

#[test]
fn every_required_operation_is_exposed_or_declared() {
    let required = crate::skill::required_operations();
    for operation in &required {
        let exposed = TOOLS.iter().any(|tool| tool.contract_name == operation);
        let declared = NOT_EXPOSED.iter().any(|(name, _)| name == operation);
        assert!(
            exposed || declared,
            "the contract requires `{operation}` and no tool exposes it, nor does NOT_EXPOSED \
             say why one cannot exist"
        );
        assert!(
            !(exposed && declared),
            "`{operation}` is both exposed and declared unexposed"
        );
    }
    for (operation, reason) in NOT_EXPOSED {
        assert!(
            required.contains(&(*operation).to_owned()),
            "`{operation}` is declared unexposed but the contract never required it"
        );
        assert!(
            reason.len() > 20,
            "`{operation}` is declared unexposed without saying why"
        );
        assert!(
            !TOOLS.iter().any(|tool| tool.name == *operation),
            "`{operation}` is both an acting tool and declared unexposed — a name that only \
             refuses is not an operation this server performs"
        );
    }
}

#[test]
fn every_tool_maps_to_a_transport_operation_the_binding_documents() {
    let binding = crate::skill::FILES
        .iter()
        .find(|file| file.path == "bindings/github.md")
        .expect("the GitHub binding ships");
    for tool in TOOLS
        .iter()
        .filter(|tool| !tools::is_local_store(tool.name))
    {
        assert!(
            binding.contents.contains(tool.operation),
            "{} runs `{}`, which bindings/github.md never names",
            tool.name,
            tool.operation
        );
    }
}

#[test]
fn every_executable_operation_the_binding_documents_is_exposed_or_declared() {
    let transport = transport_source();

    let mut operations: Vec<&str> = transport
        .as_str()
        .match_indices("\n        \"")
        .map(|(at, marker)| {
            let rest = &transport.as_str()[at + marker.len()..];
            &rest[..rest.find('"').unwrap_or(0)]
        })
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        .collect();
    operations.sort_unstable();
    operations.dedup();
    assert!(
        operations.len() >= 15,
        "the transport's subcommands could not be read: {operations:?}"
    );

    for operation in operations {
        let exposed = TOOLS.iter().any(|tool| tool.operation == operation);
        let declared = NOT_RUN.iter().any(|(name, _)| *name == operation);
        assert!(
            exposed || declared,
            "the transport provides `{operation}` and no tool reaches it, nor does NOT_RUN say \
             why one should not. An operation the agent cannot call is one it will reconstruct \
             by hand."
        );
    }
    for (operation, reason) in NOT_RUN {
        assert!(
            !TOOLS.iter().any(|tool| tool.operation == *operation),
            "`{operation}` is both exposed and declared unexposed"
        );
        assert!(
            reason.len() > 20,
            "`{operation}` is declared unexposed without saying why"
        );
    }
}

#[test]
fn every_tool_names_a_row_the_binding_actually_has() {
    let binding = crate::skill::FILES
        .iter()
        .find(|file| file.path == "bindings/github.md")
        .expect("the GitHub binding ships");
    for tool in TOOLS
        .iter()
        .filter(|tool| !tools::is_local_store(tool.name))
    {
        let row = format!("| {}", tool.contract_name);
        let backticked = format!("| `{}`", tool.contract_name);
        assert!(
            binding.contents.contains(&row) || binding.contents.contains(&backticked),
            "{} claims to implement `{}`, which is not a row in the binding's operations table",
            tool.name,
            tool.contract_name
        );
    }
}

const ABSORPTION_MAP_HEADING: &str = "### Operations no tool exposes";

const FIELDS_IN_OPERATION_TABLES: &[&str] = &["last_activity"];

fn spelling(name: &str) -> String {
    name.replace('-', "_")
}

fn is_identifier(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn cli_names(command: &clap::Command, into: &mut Vec<String>) {
    for sub in command.get_subcommands() {
        into.push(spelling(sub.get_name()));
        into.extend(sub.get_all_aliases().map(spelling));
        cli_names(sub, into);
    }
}

fn callable_names() -> Vec<String> {
    let mut names: Vec<String> = TOOLS
        .iter()
        .flat_map(|tool| [tool.name, tool.contract_name, tool.operation])
        .chain(NOT_EXPOSED.iter().map(|(name, _)| *name))
        .chain(NOT_RUN.iter().map(|(name, _)| *name))
        .map(spelling)
        .collect();
    cli_names(
        &<crate::cli::Cli as clap::CommandFactory>::command(),
        &mut names,
    );
    names
}

fn backticked(text: &str) -> Vec<(usize, usize, &str)> {
    let mut found = Vec::new();
    let mut open = None;
    for (at, _) in text.match_indices('`') {
        match open.take() {
            Some(start) => found.push((start, at + 1, &text[start + 1..at])),
            None => open = Some(at),
        }
    }
    found
}

fn operation_candidates(line: &str, in_operation_table: bool) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut seen = |word: &str| {
        if is_identifier(word) {
            found.push(word.to_owned());
        }
    };
    let mut rest = line;
    while let Some(at) = rest.find("SCRIPT ") {
        rest = &rest[at + "SCRIPT ".len()..];
        let end = rest
            .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'))
            .unwrap_or(rest.len());
        seen(&rest[..end]);
    }
    for (start, end, word) in backticked(line) {
        let before = line[..start].trim_end().to_ascii_lowercase();
        let after = &line[end..];
        let called = before.ends_with(" call") || before.ends_with(" calls") || before == "call";
        let with_flag = after.starts_with(" with `--");
        if called || with_flag {
            seen(word);
        }
    }
    if in_operation_table {
        let mut cells = line.strip_prefix('|').unwrap_or("").split('|');
        let first = cells.next().unwrap_or("").trim();
        let agent_performed = cells
            .next()
            .is_some_and(|cell| cell.trim_start().starts_with("performed by the agent"));
        if !agent_performed {
            seen(first);
            for (start, _, word) in backticked(first) {
                let lead = first[..start].trim_end();
                if !(lead.ends_with("with") || lead.ends_with("its")) {
                    seen(word);
                }
            }
        }
    }
    found
}

fn absorption_map(binding: &str) -> Vec<&str> {
    let mut in_fence = false;
    let mut in_map = false;
    let mut lines = Vec::new();
    for line in binding.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence && line.starts_with('#') {
            in_map = line.starts_with(ABSORPTION_MAP_HEADING);
        }
        if in_map && line.starts_with("- ") {
            lines.push(line);
        }
    }
    lines
}

fn retired_names() -> Vec<String> {
    let binding = crate::skill::FILES
        .iter()
        .find(|file| file.path == "bindings/github.md")
        .expect("the GitHub binding ships");
    absorption_map(binding.contents)
        .into_iter()
        .flat_map(|line| {
            let (left, _) = line.split_once('→').expect("a map entry has an arrow");
            backticked(left)
                .into_iter()
                .map(|(_, _, word)| spelling(word))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn unresolved_operations(path: &str, contents: &str, callable: &[String]) -> Vec<String> {
    let retired = retired_names();
    let mut stale = Vec::new();
    let mut in_fence = false;
    let mut in_map = false;
    let mut in_operation_table = false;
    for (index, line) in contents.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && line.starts_with('#') {
            in_map = path == "bindings/github.md" && line.starts_with(ABSORPTION_MAP_HEADING);
        }
        if in_map {
            continue;
        }
        if !in_fence {
            if let Some(row) = line.strip_prefix('|') {
                if let Some(cell) = row.split('|').next() {
                    let header = cell.trim().trim_matches('`');
                    if header.eq_ignore_ascii_case("operation") {
                        in_operation_table = true;
                    }
                }
            } else {
                in_operation_table = false;
            }
        }
        for word in operation_candidates(line, in_operation_table && !in_fence) {
            let word = spelling(&word);
            if callable.contains(&word) || FIELDS_IN_OPERATION_TABLES.contains(&word.as_str()) {
                continue;
            }
            let why = if retired.contains(&word) {
                "was absorbed (see the map in bindings/github.md)"
            } else {
                "is not a tool, an unexposed operation or a subcommand"
            };
            stale.push(format!("{path}:{} names `{word}`, which {why}", index + 1));
        }
    }
    stale
}

#[test]
fn no_shipped_skill_file_names_an_operation_no_tool_performs() {
    let callable = callable_names();
    let mut stale: Vec<String> = Vec::new();
    let mut scanned = 0;
    for file in crate::skill::FILES {
        scanned += file.contents.lines().count();
        stale.extend(unresolved_operations(file.path, file.contents, &callable));
    }
    assert!(
        scanned > 1000,
        "the shipped skill was not read: {scanned} lines"
    );
    assert!(
        stale.is_empty(),
        "these name an operation that no tool performs and NOT_EXPOSED does not declare; \
         name the tool or view that answers it instead:\n{}",
        stale.join("\n")
    );
}

#[test]
fn the_absorption_map_names_only_operations_no_tool_performs_and_answers_with_one_that_exists() {
    let callable = callable_names();
    let binding = crate::skill::FILES
        .iter()
        .find(|file| file.path == "bindings/github.md")
        .expect("the GitHub binding ships");
    let entries = absorption_map(binding.contents);
    assert!(entries.len() > 4, "the absorption map was not read");
    for line in entries {
        let (left, right) = line.split_once('→').expect("a map entry has an arrow");
        for (_, _, word) in backticked(left) {
            let exposed = TOOLS.iter().any(|tool| {
                spelling(tool.name) == spelling(word)
                    || spelling(tool.contract_name) == spelling(word)
            });
            assert!(
                !exposed,
                "`{word}` is in the absorption map and is also a tool"
            );
        }
        let (_, _, answer) = backticked(right)
            .into_iter()
            .next()
            .expect("a map entry names the call that answers it");
        assert!(
            callable.contains(&spelling(answer)),
            "the map answers with `{answer}`, which nothing performs: {line}"
        );
    }
}

#[test]
fn the_retired_name_scan_sees_hyphenated_bare_and_fenced_names() {
    let callable = callable_names();
    for (label, text) in [
        (
            "hyphenated",
            "Run `review-findings` with `--issue` first.\n",
        ),
        (
            "bare",
            "| Operation | Command |\n|---|---|\n| repair_rounds | read-only |\n",
        ),
        ("fenced", "```bash\nSCRIPT review-findings --issue 1\n```\n"),
        (
            "after the map",
            "### Operations no tool exposes\n- `a` → `claim`\n### Next\ncall `spend_summary`\n",
        ),
    ] {
        let hits = unresolved_operations("bindings/github.md", text, &callable);
        assert!(
            !hits.is_empty(),
            "{label}: a retired name went unseen in {text:?}"
        );
    }
    let inside_map = "### Operations no tool exposes\n```\n# x\n```\ncall `spend_summary`\n";
    assert_eq!(
        unresolved_operations("bindings/github.md", inside_map, &callable),
        Vec::<String>::new()
    );
    let clean = "Read `README.md` and the `view` field; pass `--force`.\n| Operation | Command |\n|---|---|\n| `claim` | `SCRIPT claim` |\n| `review_read` with `view` | ok |\n";
    assert_eq!(
        unresolved_operations("references/x.md", clean, &callable),
        Vec::<String>::new()
    );
}

#[test]
fn no_two_tools_answer_to_the_same_name() {
    let mut names = TOOLS.iter().map(|tool| tool.name).collect::<Vec<_>>();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len());
}

/// Publication and receipt-bound review tools bind to the
/// repository host; everything else to the tracker. A single top-of-run_tool
/// check on the tracker is what made Linear refuse `claim` and `publish`
/// alike.
#[test]
fn publication_tools_bind_to_the_repository_host_and_the_rest_to_the_tracker() {
    let host = [
        "publish_review",
        "republish_review",
        "release_ci",
        "draft_pr",
        "judge_workspaces",
    ];
    for tool in TOOLS {
        let expected = if host.contains(&tool.name) {
            super::tools::TransportBind::RepositoryHost
        } else {
            super::tools::TransportBind::Tracker
        };
        assert_eq!(
            tool.binds(),
            expected,
            "{} bound to {:?}, expected {:?}",
            tool.name,
            tool.binds(),
            expected
        );
    }
    assert!(
        host.iter().all(|name| tools::find(name).is_some()),
        "a host-bound name is not a tool this server exposes"
    );
}

#[test]
fn a_tool_that_does_not_exist_is_refused_with_the_ones_that_do() {
    let response = ask(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "merge_everything", "arguments": {}}
    }))
    .expect("an answer");
    let message = response["error"]["message"].as_str().expect("a message");
    assert!(message.contains("merge_everything"));
    for tool in TOOLS {
        assert!(message.contains(tool.name), "{} is not offered", tool.name);
    }
}

#[test]
fn a_missing_required_argument_names_it_and_says_what_it_is_for() {
    let response = ask(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "claim", "arguments": {"issue": 12}}
    }))
    .expect("an answer");
    let message = response["error"]["message"].as_str().expect("a message");
    assert!(message.contains("run_id"), "{message}");
    assert!(message.contains("SessionStart"), "{message}");
}

#[test]
fn an_uninstalled_harness_answers_with_a_refusal_the_agent_can_act_on() {
    let response = result(json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {"name": "verify_claim", "arguments": {
            "issue": 12, "run_id": "claude-abcd1234", "expect_state": "in-progress"
        }}
    }));
    assert_eq!(response["isError"], true);
    let text = response["content"][0]["text"].as_str().expect("text");
    assert!(text.contains("nothing was written"));
    assert!(text.contains("skill-root-unknown"));
}

#[test]
fn a_tool_answer_is_rendered_compact_because_the_model_pays_per_byte() {
    let body = json!({
        "ok": true,
        "entries": [
            {"path": "src/lib.rs", "mode": "100644", "blob": "a".repeat(40)},
            {"path": "src/main.rs", "mode": "100644", "blob": "b".repeat(40)},
        ],
    });
    let text = super::answer_text(Some(body.clone()));
    assert_eq!(
        text.lines().count(),
        1,
        "a tool answer carried indentation the model pays for: {text}"
    );
    let mut expected = body;
    expected["build"] = json!({
        "commit": crate::build_info::commit(),
        "dirty": crate::build_info::dirty(),
    });
    let back: serde_json::Value = serde_json::from_str(&text).expect("the answer is still JSON");
    assert_eq!(
        back, expected,
        "compacting the answer changed what it says, or the build identity was not carried"
    );
}

#[test]
fn an_answer_with_no_body_still_says_it_worked() {
    let text = super::answer_text(None);
    let answered: serde_json::Value = serde_json::from_str(&text).expect("the answer is JSON");
    assert_eq!(answered["ok"], true);
    assert_eq!(
        answered["build"]["commit"],
        json!(crate::build_info::commit()),
        "the None fallback should still carry the build identity: {text}"
    );
    assert_eq!(
        answered["build"]["dirty"],
        crate::build_info::dirty(),
        "the None fallback should still carry the dirty flag: {text}"
    );
}

#[test]
fn an_object_body_gets_the_build_identity_inserted() {
    let body = json!({"issue": 12});
    let text = super::answer_text(Some(body));
    let answered: serde_json::Value = serde_json::from_str(&text).expect("the answer is JSON");
    assert_eq!(answered["issue"], 12);
    assert_eq!(
        answered["build"]["commit"],
        json!(crate::build_info::commit())
    );
    assert_eq!(answered["build"]["dirty"], crate::build_info::dirty());
}

#[test]
fn a_non_object_body_is_left_alone_rather_than_corrupted() {
    let text = super::answer_text(Some(json!("plain string")));
    assert_eq!(
        text, r#""plain string""#,
        "a non-object body must not be turned into an object carrying `build`: {text}"
    );
}

#[test]
fn the_unimplemented_list_names_a_cost_for_every_gap() {
    for (part, cost) in UNIMPLEMENTED {
        assert!(!part.is_empty());
        assert!(
            cost.len() > 30,
            "`{part}` is listed as unimplemented without saying what it costs"
        );
    }
}

#[test]
fn serving_a_stream_answers_every_request_in_order() {
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}).to_string(),
    ]
    .join("\n");

    let mut output = Vec::new();
    serve(
        std::io::Cursor::new(requests),
        &mut output,
        Err(crate::skill::no_skill_root()),
    )
    .expect("the stream is served");

    let answers: Vec<Value> = String::from_utf8(output)
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is one JSON message"))
        .collect();

    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0]["id"], 1);
    assert_eq!(answers[1]["id"], 2);
    assert_eq!(answers[2]["id"], 3);
    assert!(answers[1]["result"]["tools"].is_array());
}

#[test]
fn a_blank_line_is_skipped_rather_than_answered() {
    let mut output = Vec::new();
    serve(
        std::io::Cursor::new("\n\n"),
        &mut output,
        Err(crate::skill::no_skill_root()),
    )
    .expect("the stream is served");
    assert!(output.is_empty());
}

#[test]
fn claim_carries_an_idempotency_key_and_reuses_it_on_retry() {
    let root = tempfile::tempdir().expect("a temporary root");
    let first = session::mint_operation_id("claude-abcd1234", 12);
    assert_eq!(first.len(), 32);
    assert!(
        first
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    );

    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.operation_id = Some(first.clone());
    session::store(root.path(), &run).expect("the pointer writes");
    assert_eq!(
        session::load(root.path(), "claude-abcd1234").operation_id,
        Some(first)
    );
}

#[test]
fn a_run_that_already_holds_an_issue_cannot_swear_to_a_second() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    session::store(&context.state_root, &run).expect("the pointer writes");

    let failure = run_tool(
        "claim",
        &json!({"issue": 34, "run_id": "claude-abcd1234", "horizon": "2026-08-01T18:00Z"}),
        Ok(&context),
    )
    .expect_err("a second oath is refused");
    let ToolFailure::Refused(refusal) = failure else {
        panic!("a well-formed call was reported malformed");
    };
    assert_eq!(refusal.code, "already-holding");
    assert!(refusal.message.contains("#12"));
    assert!(
        refusal.to_string().contains("estigia release"),
        "it must name the way out"
    );
}

#[test]
fn re_claiming_the_same_issue_is_a_renewal_and_is_not_refused() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    session::store(&context.state_root, &run).expect("the pointer writes");

    let failure = run_tool(
        "claim",
        &json!({"issue": 12, "run_id": "claude-abcd1234", "horizon": "2026-08-01T18:00Z"}),
        Ok(&context),
    )
    .expect_err("there is no transport in this sandbox");
    assert!(
        !failure.to_string().contains("already-holding"),
        "{failure}"
    );
}

#[test]
fn holding_nothing_is_not_holding_something_else() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let failure = run_tool(
        "claim",
        &json!({"issue": 34, "run_id": "claude-fresh0a1b2c3d000", "horizon": "2026-08-01T18:00Z"}),
        Ok(&context),
    )
    .expect_err("there is no transport in this sandbox");
    assert!(
        !failure.to_string().contains("already-holding"),
        "{failure}"
    );
}

#[test]
fn a_pointer_is_never_written_under_an_empty_run_id() {
    // `transition` used to reach the pointer code with an empty run id: the
    // issue moved, a pointer file named for nobody recorded it, and the run
    // that made the move went on believing the old state.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let _ = run_tool(
        "transition",
        &json!({"issue": 12, "to": "review"}),
        Ok(&context),
    );
    let stray = std::fs::read_dir(&context.state_root)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(stray, 0, "a pointer was written under an empty run id");
}

#[test]
fn a_boolean_goes_out_as_a_bare_flag_or_not_at_all() {
    let tool = tools::find("target").expect("a declared tool");

    let set =
        super::flags_for(tool, &json!({"base": "main", "manifest": true})).expect("well formed");
    assert!(
        set.contains(&"--manifest".to_owned()),
        "a boolean was not rendered as a bare flag: {set:?}"
    );

    let clear =
        super::flags_for(tool, &json!({"base": "main", "manifest": false})).expect("well formed");
    assert!(
        !clear.contains(&"--manifest".to_owned()),
        "declining a boolean still sent it: {clear:?}"
    );
    let missing = super::flags_for(tool, &json!({"base": "main"})).expect("well formed");
    assert!(
        !missing.contains(&"--manifest".to_owned()),
        "omitting a boolean still sent it: {missing:?}"
    );

    assert!(matches!(
        super::flags_for(tool, &json!({"fix": "yes"})),
        Err(ToolFailure::Malformed(_))
    ));

    let closing = tools::find("check_closing_keywords").expect("a declared tool");
    let sent =
        super::flags_for(closing, &json!({"issue": 12, "base": "main"})).expect("well formed");
    assert!(
        sent.windows(2).any(|pair| pair == ["--base", "main"]),
        "a string argument stopped carrying its value: {sent:?}"
    );
}

/// The `task` tool's `compact` argument reaches the transport as the bare
/// `--compact` flag, and declining it sends nothing, so an optimized client
/// selects the compact answer through the schema rather than a private call.
#[test]
fn the_task_tool_carries_compact_to_the_transport_flag() {
    let tool = tools::find("task").expect("a declared tool");
    let asked = super::flags_for(
        tool,
        &json!({"issue": 5, "run_id": "claude-abcd1234", "id": "T1", "compact": true}),
    )
    .expect("well formed");
    assert!(
        asked.contains(&"--compact".to_owned()),
        "the compact argument never reached the transport: {asked:?}"
    );
    let declined = super::flags_for(
        tool,
        &json!({"issue": 5, "run_id": "claude-abcd1234", "id": "T1", "compact": false}),
    )
    .expect("well formed");
    assert!(
        !declined.contains(&"--compact".to_owned()),
        "declining compact still sent it: {declined:?}"
    );
}

/// The `compact` argument reaches the transport through the real server, not
/// only through `flags_for`: a client that asks for the compact answer gets the
/// changed row alone, and one that does not still gets the whole checklist.
#[test]
fn the_task_tool_answers_compact_through_the_server() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo_dir = root.path().join("repo");
    std::fs::create_dir_all(&repo_dir).expect("a checkout");
    let store_path = root.path().join("state.db");
    let mut context = bare_context(&root, &repo_dir);
    context.tracker = crate::config::Tracker::Github {
        repo: Some(crate::config::RepoRef {
            owner: "fixture".to_owned(),
            name: "repo".to_owned(),
        }),
    };
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);

    let (compact, legacy) = crate::test_env::with_scripted_gh(bin.path(), &held, || {
        crate::transport::store::with_store_at(&store_path, || {
            let call = |compact: bool| {
                super::run_tool(
                    "task",
                    &json!({
                        "issue": 5,
                        "run_id": "claude-abcd1234",
                        "id": "T1",
                        "text": "first",
                        "route": "inline",
                        "compact": compact,
                    }),
                    Ok(&context),
                )
                .expect("the task records")
            };
            (call(true), call(false))
        })
    });

    let parse = |answered: &Value| -> Value {
        serde_json::from_str(
            answered["content"][0]["text"]
                .as_str()
                .expect("a tool result carries text"),
        )
        .expect("the answer is JSON")
    };
    let compact = parse(&compact);
    assert!(
        compact.get("tasks").is_none(),
        "the compact argument never reached the transport: {compact}"
    );
    assert_eq!(compact["task"]["id"], "T1", "{compact}");
    let legacy = parse(&legacy);
    assert_eq!(
        legacy["tasks"].as_array().map(Vec::len),
        Some(1),
        "the legacy answer lost the whole checklist: {legacy}"
    );
}

/// Whether one transport subcommand changes the filesystem, read out of its
/// own body.
///
/// Only the direct calls in `cmd_<operation>`. A helper that writes on its
/// behalf is not seen — which is why the test below carries a floor: a scan
/// that finds nothing at all reads exactly like one that finds nothing wrong.
fn transport_touches_the_disk(operation: &str) -> bool {
    let body = transport_body(operation);
    [
        "fs::write(",
        "create_dir_all(",
        "File::create(",
        "write_all(",
        // `git worktree add` is how the port makes a worktree; reading only
        // the standard library's writers made `start-branch` — the
        // operation that puts a whole checkout on disk — look read-only.
        "\"worktree\"",
        // `changelog_notes` puts its `--out` file down through
        // `replace_atomically`, which names none of the calls above, so the
        // one operation that took a path to write read as read-only.
        "replace_atomically(",
    ]
    .iter()
    .any(|call| body.contains(call))
}

#[test]
fn a_function_body_ends_with_its_own_file_whatever_file_follows_it() {
    let reader = "\npub fn read() {\n    let _ = 1;\n}\n";
    let writer = "fn helper() {\n    std::fs::write(\"x\", \"\");\n}\n";
    let body = function_body_in(reader, "read").expect("the function is in its own file");
    assert!(!body.contains("fs::write"), "{body}");
    assert!(function_body_in(writer, "read").is_none());
}

#[test]
fn a_function_at_the_very_start_of_a_file_is_found() {
    let source = "pub fn read() {
    let _ = 1;
}
pub fn other() {}
";
    let body = function_body_in(source, "read").expect("the first line of a file is searched");
    assert!(body.starts_with("pub fn read("), "{body}");
    assert!(!body.contains("other"), "{body}");
}

#[test]
fn a_tool_that_changes_the_filesystem_is_not_advertised_as_read_only() {
    // `writes` becomes `readOnlyHint` in the schema, which is the annotation
    // an MCP client reads to decide whether it may run a tool without
    // asking. `changelog_notes` once said read-only while its `out`
    // argument wrote a file at a path the caller chose.
    let mut writers = 0;
    for tool in TOOLS {
        if !transport_touches_the_disk(tool.operation) {
            continue;
        }
        writers += 1;
        assert!(
            tool.writes,
            "{} runs `{}`, which changes the filesystem, and is advertised \
             `readOnlyHint: true` \u{2014} a client that trusts the hint runs it without asking",
            tool.name, tool.operation
        );
    }
    // The floor: nothing found is not the same as nothing wrong. A scan that
    // reads 0 has checked almost nothing, and the throwaway version of this
    // scan that missed a writing tool read exactly that.
    assert!(
        writers >= 2,
        "only {writers} operation(s) were seen to touch the disk, so this checked almost nothing"
    );
}

#[test]
fn bookkeeping_arguments_never_reach_the_transport() {
    for (tool, argument) in tools::POINTER_ONLY {
        let found = tools::find(tool).expect("a declared tool");
        assert!(
            found.arguments.iter().any(|a| a.name == *argument),
            "{tool} declares `{argument}` as bookkeeping and does not accept it"
        );
        assert!(tools::is_pointer_only(tool, argument));
        assert!(!tools::is_pointer_only(tool, "issue"));
    }
}

#[test]
fn every_tracker_either_has_a_transport_that_ships_or_none_at_all() {
    let tracker = crate::config::Tracker::Github { repo: None };
    if let Some(transport) = tracker.transport() {
        assert!(
            transport == "estigia"
                || crate::skill::FILES
                    .iter()
                    .any(|file| file.path == transport),
            "{tracker:?} names {transport}, which is not installed"
        );
    }
}

#[test]
fn a_transition_cannot_be_made_without_saying_whose_run_it_is() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let error = run_tool(
        "transition",
        &json!({"issue": 12, "to": "review"}),
        Ok(&context),
    )
    .expect_err("a transition without a run id is malformed");
    assert!(error.to_string().contains("run_id"), "{error}");
}

#[test]
fn a_bookkeeping_argument_is_still_required_when_leaving_it_out_would_desynchronise() {
    let transition = tools::find("transition").expect("transition exists");
    let run_id = transition
        .arguments
        .iter()
        .find(|argument| argument.name == "run_id")
        .expect("transition takes a run id");
    assert!(run_id.required);
    assert!(tools::is_pointer_only("transition", "run_id"));
}

#[test]
fn a_refusal_reaches_the_caller_with_the_transport_s_own_taxonomy_intact() {
    let answer = tracker::Answer {
        code: 5,
        body: Some(json!({
            "ok": false,
            "reason": "ambiguous-write",
            "action": "RE-READ the branch, link and refs before retrying"
        })),
    };
    let refusal = tracker::translate(&answer, "publish_review").expect("a refusal");
    assert_eq!(refusal.outcome, crate::outcome::MutationOutcome::Unknown);

    let failure = ToolFailure::Refused(Box::new(refusal));
    let ToolFailure::Refused(carried) = failure else {
        panic!("a refusal was reported malformed");
    };
    assert_eq!(carried.outcome, crate::outcome::MutationOutcome::Unknown);
    assert_eq!(
        carried.replay,
        crate::outcome::Replayability::StatusRequired
    );
    assert!(!carried.outcome.is_clean());
}

#[test]
fn publication_refusals_invalidate_only_when_a_remote_write_cannot_be_excluded() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let old_receipt = crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: "b".repeat(40),
        base: "c".repeat(40),
        digest: "d".repeat(64),
    };

    for name in ["publish_review", "republish_review"] {
        let tool = tools::find(name).expect("the publication tool exists");
        for (case, answer, invalidates) in [
            (
                "committed",
                tracker::Answer {
                    code: 1,
                    body: Some(json!({
                        "ok": false,
                        "reason": "draft-readback-failed",
                        "world": "committed",
                        "action": "re-read the pull request"
                    })),
                },
                true,
            ),
            (
                "ambiguous",
                tracker::Answer {
                    code: 5,
                    body: Some(
                        crate::transport::Failure::Write(
                            "the publication write did not answer".to_owned(),
                        )
                        .envelope(),
                    ),
                },
                true,
            ),
            (
                "pre-write",
                tracker::Answer {
                    code: 1,
                    body: Some(json!({
                        "ok": false,
                        "reason": "unexpected-state",
                        "action": "re-read the issue"
                    })),
                },
                false,
            ),
        ] {
            let run_id = format!("claude-{name}-{case}");
            let mut run = crate::harness::session::Run::new(run_id.clone());
            run.review_transaction = Some(old_receipt.clone());
            run.reviewed_head = Some("e".repeat(40));
            crate::harness::session::store(&context.state_root, &run)
                .expect("the old authority writes");

            let failure = super::refusal_from_answer(tool, &answer, &mut run, &context)
                .expect("the dispatch answer is a refusal");
            assert!(matches!(failure, ToolFailure::Refused(_)));

            let stored = crate::harness::session::load(&context.state_root, &run_id);
            if invalidates {
                assert_eq!(stored.review_transaction, None, "{name} {case}");
                assert_eq!(stored.reviewed_head, None, "{name} {case}");
            } else {
                assert_eq!(
                    stored.review_transaction.as_ref(),
                    Some(&old_receipt),
                    "{name} {case}"
                );
                assert!(stored.reviewed_head.is_some(), "{name} {case}");
            }
        }
    }
}

#[test]
fn a_malformed_call_and_a_refusal_are_not_the_same_failure() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    assert!(matches!(
        run_tool("claim", &json!({"issue": 12}), Ok(&context)),
        Err(ToolFailure::Malformed(_))
    ));
    assert!(matches!(
        run_tool(
            "claim",
            &json!({"issue": 12, "run_id": "claude-a", "horizon": "2026-08-01T18:00Z"}),
            Err(&crate::skill::no_skill_root())
        ),
        Err(ToolFailure::Refused(_))
    ));
}

fn assert_envelope(response: &Value, expected_id: &Value) {
    assert_eq!(
        response["jsonrpc"], "2.0",
        "every response carries the protocol version"
    );
    assert_eq!(response["id"], *expected_id, "a response echoes its id");
    let has_result = response.get("result").is_some();
    let has_error = response.get("error").is_some();
    assert!(
        has_result ^ has_error,
        "a response carries exactly one of result and error: {response}"
    );
    if let Some(error) = response.get("error") {
        assert!(
            error["code"].is_i64(),
            "an error carries a numeric code: {error}"
        );
        assert!(
            error["message"].as_str().is_some_and(|m| !m.is_empty()),
            "an error carries a message: {error}"
        );
    }
    assert!(
        response.as_object().is_some_and(|object| object
            .keys()
            .all(|key| ["jsonrpc", "id", "result", "error"].contains(&key.as_str()))),
        "a response carries no member outside the specification: {response}"
    );
}

#[test]
fn every_response_this_server_can_produce_is_a_valid_envelope() {
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list"}),
        json!({"jsonrpc": "2.0", "id": "a string id", "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
               "params": {"name": "nope", "arguments": {}}}),
        json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call",
               "params": {"name": "claim", "arguments": {}}}),
        json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
               "params": {"name": "verify_claim", "arguments": {
                   "issue": 12, "run_id": "claude-a", "expect_state": "in-progress"}}}),
        json!({"jsonrpc": "2.0", "id": null, "method": "ping"}),
    ];
    for request in requests {
        let response = ask(request.clone())
            .unwrap_or_else(|| panic!("{request} carries an id and must be answered"));
        assert_envelope(&response, &request["id"]);
    }
}

#[test]
fn a_parse_error_answers_with_a_null_id_because_none_could_be_read() {
    let response =
        handle_line("{not json", Err(&crate::skill::no_skill_root())).expect("an answer");
    assert_envelope(&response, &Value::Null);
    assert_eq!(response["error"]["code"], code::PARSE_ERROR);
}

#[test]
fn every_tool_result_is_shaped_the_way_a_client_reads_it() {
    let response = result(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "verify_claim", "arguments": {
            "issue": 12, "run_id": "claude-a", "expect_state": "in-progress"}}
    }));
    let content = response["content"]
        .as_array()
        .expect("a tool result carries an array of content blocks");
    assert!(!content.is_empty());
    for block in content {
        assert_eq!(block["type"], "text");
        assert!(block["text"].as_str().is_some_and(|text| !text.is_empty()));
    }
    assert!(response["isError"].is_boolean());
}

fn transport_flags(operation: &str) -> Option<(Vec<String>, Vec<String>)> {
    transport_block(operation)?;
    let read = transport_reads(operation);
    let accepted = read.iter().map(|(flag, _)| flag.clone()).collect();
    let required = read
        .iter()
        .filter(|(_, how)| *how == "need" || *how == "number")
        .map(|(flag, _)| flag.clone())
        .collect();
    Some((accepted, required))
}

fn transport_valueless(operation: &str) -> Vec<String> {
    transport_reads(operation)
        .into_iter()
        .filter(|(_, how)| *how == "on")
        .map(|(flag, _)| flag)
        .collect()
}
#[test]
fn a_boolean_argument_is_one_the_transport_takes_without_a_value() {
    let mut booleans = 0;
    let mut valueless_seen = 0;
    for tool in TOOLS {
        let valueless = transport_valueless(tool.operation);
        valueless_seen += valueless.len();
        for argument in tool.arguments {
            if tools::is_pointer_only(tool.name, argument.name) {
                continue;
            }
            let flag = argument.as_flag();
            let takes_none = valueless.contains(&flag);
            if argument.json_type == "boolean" {
                booleans += 1;
            }
            assert_eq!(
                argument.json_type == "boolean",
                takes_none,
                "{}: `{flag}` is {} here and {} in the transport",
                tool.name,
                if argument.json_type == "boolean" {
                    "a boolean"
                } else {
                    argument.json_type
                },
                if takes_none {
                    "takes no value"
                } else {
                    "takes one"
                }
            );
        }
    }
    assert!(booleans > 0, "no tool declares a boolean argument");
    assert!(
        valueless_seen > 0,
        "no valueless flag was read out of the transport, so the shapes were never compared"
    );
}

const CONSTRAINED_TO_THE_STATES: &[(&str, &str)] = &[
    ("list-state", "state"),
    ("verify-claim", "expect-state"),
    ("transition", "to"),
    ("transition", "from"),
    ("claim", "transition-to"),
    ("heartbeat", "expect-state"),
    ("start-branch", "expect-state"),
    ("publish-review", "expect-state"),
    ("republish-review", "expect-state"),
];

fn transport_constrained(operation: &str) -> Vec<String> {
    CONSTRAINED_TO_THE_STATES
        .iter()
        .filter(|(named, _)| *named == operation)
        .map(|(_, flag)| format!("--{flag}"))
        .collect()
}

#[test]
fn a_flag_constrained_to_the_workflow_states_publishes_them() {
    let mut published = 0;
    let mut found = 0;
    for tool in crate::harness::mcp::tools::TOOLS {
        let constrained = transport_constrained(tool.operation);
        found += constrained.len();
        for argument in tool.arguments {
            if crate::harness::mcp::tools::is_pointer_only(tool.name, argument.name) {
                continue;
            }
            let flag = argument.as_flag();
            let takes_states = constrained.contains(&flag);
            if takes_states {
                published += 1;
                assert_eq!(
                    argument.choices,
                    Some(crate::config::STATES),
                    "{}: `{flag}` publishes a vocabulary that is not the workflow's",
                    tool.name
                );
            }
            assert!(
                !takes_states || argument.choices.is_some(),
                "{}: `{flag}` publishes no vocabulary here and is held to the workflow \
                 states in the transport",
                tool.name
            );
        }
    }
    let kept = crate::harness::mcp::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .and_then(|tool| {
            tool.arguments
                .iter()
                .find(|argument| argument.name == "state")
        })
        .expect("`claim` takes the state it swears into");
    assert_eq!(
        kept.choices,
        Some(crate::config::STATES),
        "the state `claim` moves the run pointer to publishes no vocabulary"
    );

    assert!(published > 0, "no argument publishes the workflow states");
    assert!(
        found > 0,
        "no `choices=STATES` flag was read out of the transport, so this crossed nothing"
    );
}

fn transport_required(operation: &str) -> Vec<String> {
    transport_reads(operation)
        .into_iter()
        .filter(|(_, how)| *how == "need" || *how == "number")
        .map(|(flag, _)| flag)
        .collect()
}
/// Each `flags.push("--x")` is attributed to the operations named in the
/// nearest preceding `tool.operation` condition — moving a push above its
/// guard would silently re-attribute it.
fn server_supplied() -> Vec<(String, String)> {
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    let mut supplied = Vec::new();
    for (at, _) in source.match_indices("flags.push(\"--") {
        let after = &source[at + "flags.push(\"".len()..];
        let Some(close) = after.find('"') else {
            continue;
        };
        let flag = after[..close].to_owned();
        let Some(guard) = source[..at].rfind("tool.operation") else {
            continue;
        };
        let condition = &source[guard..at];
        let Some(brace) = condition.find('{') else {
            continue;
        };
        for operation in condition[..brace].split('"').skip(1).step_by(2) {
            supplied.push((operation.to_owned(), flag.clone()));
        }
    }
    supplied
}

#[test]
fn a_flag_the_transport_requires_is_one_somebody_sends() {
    let supplied = server_supplied();
    let mut crossed = 0;
    for tool in crate::harness::mcp::tools::TOOLS {
        for flag in transport_required(tool.operation) {
            crossed += 1;
            let asked_of_the_agent = tool.arguments.iter().any(|argument| {
                argument.required
                    && argument.as_flag() == flag
                    && !crate::harness::mcp::tools::is_pointer_only(tool.name, argument.name)
            });
            let sent_by_the_server = supplied
                .iter()
                .any(|(operation, sent)| operation == tool.operation && *sent == flag);
            assert!(
                asked_of_the_agent || sent_by_the_server,
                "{}: the transport will not run without `{flag}`, and neither the schema nor \
                 this server sends it \u{2014} every call of this tool fails argparse",
                tool.name
            );
        }
    }
    assert!(
        crossed > 20,
        "only {crossed} required flags were read out of the transport, so this crossed almost \
         nothing"
    );
    assert!(
        !supplied.is_empty(),
        "no flag was read out of this module's source, so the server's half was never counted"
    );
}

fn transport_integers(operation: &str) -> Vec<String> {
    transport_reads(operation)
        .into_iter()
        .filter(|(_, how)| *how == "number" || *how == "int")
        .map(|(flag, _)| flag)
        .collect()
}
#[test]
fn an_integer_argument_is_one_the_transport_parses_as_a_whole_number() {
    let mut declared = 0;
    let mut found = 0;
    for tool in crate::harness::mcp::tools::TOOLS
        .iter()
        .filter(|tool| !crate::harness::mcp::tools::is_local_store(tool.name))
    {
        let integers = transport_integers(tool.operation);
        found += integers.len();
        for argument in tool.arguments {
            if crate::harness::mcp::tools::is_pointer_only(tool.name, argument.name) {
                continue;
            }
            let flag = argument.as_flag();
            let parsed_as_int = integers.contains(&flag);
            if argument.json_type == "integer" {
                declared += 1;
            }
            assert_eq!(
                argument.json_type == "integer",
                parsed_as_int,
                "{}: `{flag}` is {} here and {} in the transport",
                tool.name,
                argument.json_type,
                if parsed_as_int {
                    "a whole number"
                } else {
                    "not typed"
                }
            );
        }
    }
    assert!(declared > 0, "no tool declares an integer argument");
    assert!(
        found > 0,
        "no `type=int` flag was read out of the transport, so the shapes were never compared"
    );
}

#[test]
fn the_flags_read_out_of_the_transport_are_the_ones_it_declares() {
    let (accepted, required) = transport_flags("claim").expect("claim is a subcommand");
    assert!(
        accepted.contains(&"--operation-id".to_owned()),
        "{accepted:?}"
    );
    assert!(required.contains(&"--horizon".to_owned()), "{required:?}");
    assert!(
        accepted.contains(&"--transition-to".to_owned()),
        "claim no longer accepts the state it moves the issue to in the same call: {accepted:?}"
    );
    assert_eq!(accepted.len(), 10, "{accepted:?}");

    let (_, optional) = transport_flags("transition").expect("transition is a subcommand");
    assert!(
        !optional.contains(&"--from".to_owned()),
        "`--from` is optional and was read as required"
    );
    assert!(transport_flags("no-such-operation").is_none());
}

#[test]
fn every_tool_sends_flags_the_transport_accepts() {
    for tool in TOOLS
        .iter()
        .filter(|tool| !tools::is_local_store(tool.name))
    {
        let Some((accepted, required)) = transport_flags(tool.operation) else {
            panic!(
                "{} runs `{}`, which the transport does not declare",
                tool.name, tool.operation
            );
        };

        let added: &[&str] = match tool.operation {
            "claim" | "reclaim" | "unassign" => &["--operation-id", "--runtime"],
            "handoff-review" => &["--operation-id", "--runtime"],
            "review-finding"
            | "review-refutation"
            | "review-disposition"
            | "review-lens-report"
            | "review-validator-result"
            | "review-computed-verdict"
            | "spend" => &["--operation-id"],
            "draft-pr" => &["--branch", "--worktree"],
            _ => &[],
        };

        let sent: Vec<String> = tool
            .arguments
            .iter()
            .filter(|argument| !tools::is_pointer_only(tool.name, argument.name))
            .map(|argument| argument.as_flag())
            .chain(added.iter().map(|flag| (*flag).to_owned()))
            .collect();

        for flag in &sent {
            assert!(
                accepted.contains(flag),
                "{} sends `{flag}` and `{}` does not accept it — argparse would reject the whole \
                 call, and the agent would read it as a configuration defect",
                tool.name,
                tool.operation
            );
        }
        for flag in &required {
            assert!(
                sent.contains(flag),
                "`{}` requires `{flag}` and {} never sends it — every call fails",
                tool.operation,
                tool.name
            );
            let optional = tool
                .arguments
                .iter()
                .find(|argument| &argument.as_flag() == flag && !argument.required);
            assert!(
                optional.is_none(),
                "`{}` requires `{flag}` and {} declares it optional — an agent that leaves it \
                 out sends no flag at all, and argparse refuses the whole call",
                tool.operation,
                tool.name
            );
        }
    }
}

#[test]
fn everything_declared_unimplemented_is_actually_unimplemented() {
    let methods = [
        ("resources", "resources/list"),
        ("prompts", "prompts/list"),
        ("sampling", "sampling/createMessage"),
    ];
    for (part, method) in methods {
        assert!(
            UNIMPLEMENTED.iter().any(|(name, _)| *name == part),
            "`{part}` is implemented and no longer declared — or was never declared"
        );
        let response = ask(json!({"jsonrpc": "2.0", "id": 1, "method": method}))
            .expect("a request with an id is answered");
        assert_eq!(
            response["error"]["code"],
            code::METHOD_NOT_FOUND,
            "`{method}` answers, and `{part}` is still listed as unimplemented"
        );
    }
}

#[test]
fn the_advertised_tool_count_is_the_acting_tools() {
    let listed = result(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}));
    assert_eq!(listed["tools"].as_array().map(Vec::len), Some(TOOLS.len()));
    let response = result(json!({
        "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {}
    }));
    assert_eq!(response["capabilities"]["tools"]["listChanged"], false);
}

#[test]
fn valid_json_that_is_not_a_request_is_refused_rather_than_ignored() {
    // A batch, and the three scalars that reach the same branch. Every one
    // of them used to be answered with silence, because `Value::get("id")`
    // says `None` for anything that is not an object and the notification
    // rule took that as "no id, say nothing" — the one wrong answer a client
    // waiting on that id can never recover from.
    for line in [
        r#"[{"jsonrpc":"2.0","id":1,"method":"tools/list"}]"#,
        r#""hello""#,
        "42",
        "null",
    ] {
        let response = handle_line(line, Err(&crate::skill::no_skill_root()))
            .unwrap_or_else(|| panic!("{line} was answered with silence"));
        assert_eq!(response["error"]["code"], -32600, "{line}");
        assert_eq!(response["id"], Value::Null, "{line}");
    }

    assert!(
        handle_line(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            Err(&crate::skill::no_skill_root())
        )
        .is_none()
    );
}

#[test]
fn a_two_step_operation_can_name_what_its_first_call_found() {
    for name in ["release", "claim"] {
        let tool = super::tools::TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect("the tool is listed");
        assert!(
            tool.arguments
                .iter()
                .any(|argument| argument.name == "target_operation"),
            "`{name}` is a two-step write with no way to name what step one found"
        );
    }
}

#[test]
fn a_call_that_wrote_nothing_leaves_the_run_pointer_alone() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let release = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "release")
        .expect("the tool is listed");

    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    run.worktree = Some(std::path::PathBuf::from("/w/7"));
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    super::apply_effect(
        release,
        &serde_json::json!({"issue": 7, "run_id": "claude-abcd1234"}),
        Some(&serde_json::json!({"ok": true, "write_performed": false})),
        &mut run,
        &context,
    );
    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(
        after.issue,
        Some(crate::identity::Identity::Number(7)),
        "a call that wrote nothing took the issue"
    );
    assert!(
        after.worktree.is_some(),
        "a call that wrote nothing took the worktree the gate watches"
    );

    super::apply_effect(
        release,
        &serde_json::json!({"issue": 7, "run_id": "claude-abcd1234"}),
        Some(&serde_json::json!({"ok": true, "assignee_kept": false})),
        &mut run,
        &context,
    );
    assert_eq!(
        crate::harness::session::load(&context.state_root, "claude-abcd1234").issue,
        None,
        "the release that happened left the run still holding the issue"
    );
}

#[test]
fn isolating_a_run_records_the_checkout_it_was_adjudicated_from() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let start = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "start_branch")
        .expect("the tool is listed");
    assert_eq!(
        start.effect,
        super::PointerEffect::Isolated,
        "this test is measuring the wrong tool"
    );
    let worktree = root.path().join("trees").join("issue-56");

    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(56));
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    super::apply_effect(
        start,
        &serde_json::json!({"issue": 56, "run_id": "claude-abcd1234"}),
        Some(&serde_json::json!({
            "ok": true,
            "worktree": worktree.display().to_string(),
        })),
        &mut run,
        &context,
    );

    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(
        after.worktree.as_ref(),
        Some(&worktree),
        "the isolated checkout was not recorded at all"
    );
    assert!(
        after
            .covered()
            .any(|covered| crate::paths::covers(covered, &context.repo_dir)),
        "isolation left the run covering {:?} and not the checkout it was adjudicated from ({}), \
         which is the refusal this test exists to prevent",
        after.covered().collect::<Vec<_>>(),
        context.repo_dir.display()
    );

    // The floor, because a rule that overwrites would pass the assertion
    // above and take the guarantee with it: a run whose claim recorded
    // checkout A must not have that replaced by a server standing in B.
    let elsewhere = root.path().join("somebody-elses-checkout");
    let mut theirs = crate::harness::session::Run::new("claude-other0a1b2c3d".to_owned());
    theirs.issue = Some(crate::identity::Identity::Number(56));
    theirs.repo_dir = Some(elsewhere.clone());
    crate::harness::session::store(&context.state_root, &theirs).expect("the pointer writes");
    super::apply_effect(
        start,
        &serde_json::json!({"issue": 56, "run_id": "claude-other0a1b2c3d"}),
        Some(&serde_json::json!({
            "ok": true,
            "worktree": worktree.display().to_string(),
        })),
        &mut theirs,
        &context,
    );
    assert_eq!(
        crate::harness::session::load(&context.state_root, "claude-other0a1b2c3d").repo_dir,
        Some(elsewhere),
        "isolation overwrote the checkout the claim was made in with the server's own"
    );
}

#[test]
fn a_renewal_completes_a_record_the_tracker_has_just_agreed_with() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let renew = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("the tool is listed");
    assert_eq!(
        renew.effect,
        super::PointerEffect::Renew,
        "this test is measuring the wrong tool"
    );

    let worktree = root.path().join("trees").join("issue-56");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.worktree = Some(worktree.clone());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    let arguments = serde_json::json!({
        "issue": 56,
        "run_id": "claude-abcd1234",
        "expect_state": "review",
    });
    super::apply_effect(
        renew,
        &arguments,
        Some(&serde_json::json!({"ok": true, "issue": 56, "state": "review"})),
        &mut run,
        &context,
    );

    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(
        after.issue,
        Some(crate::identity::Identity::Number(56)),
        "the renewal left the run holding nothing, so the gate goes on measuring nothing"
    );
    assert_eq!(
        after.state.as_deref(),
        Some("review"),
        "the state the renewal was measured against was not written down"
    );
    assert_eq!(
        after.worktree.as_ref(),
        Some(&worktree),
        "the renewal took the isolated checkout the gate watches"
    );
    assert!(
        after
            .covered()
            .any(|covered| crate::paths::covers(covered, &context.repo_dir)),
        "the run is still stranded after the one call it is told to make before every write"
    );

    let elsewhere = root.path().join("another-checkout");
    let mut held = crate::harness::session::Run::new("claude-other0a1b2c3d".to_owned());
    held.issue = Some(crate::identity::Identity::Number(12));
    held.state = Some("review".to_owned());
    held.repo_dir = Some(elsewhere.clone());
    crate::harness::session::store(&context.state_root, &held).expect("the pointer writes");
    super::apply_effect(
        renew,
        &arguments,
        Some(&serde_json::json!({"ok": true})),
        &mut held,
        &context,
    );
    let after = crate::harness::session::load(&context.state_root, "claude-other0a1b2c3d");
    assert_eq!(
        (after.issue, after.state.as_deref(), after.repo_dir),
        (
            Some(crate::identity::Identity::Number(12)),
            Some("review"),
            Some(elsewhere)
        ),
        "a renewal overwrote a record that already said what it held"
    );

    {
        let silent = "release_ci";
        let tool = super::tools::TOOLS
            .iter()
            .find(|tool| tool.name == silent)
            .expect("the tool is listed");
        assert_eq!(
            tool.effect,
            super::PointerEffect::Renew,
            "{silent} no longer renews, so this case is measuring nothing"
        );
        assert!(
            !tool
                .arguments
                .iter()
                .any(|argument| matches!(argument.name, "state" | "expect_state")),
            "{silent} now carries a state, and this case exists because it did not"
        );

        let mut quiet = crate::harness::session::Run::new(format!("claude-{silent}"));
        crate::harness::session::store(&context.state_root, &quiet).expect("the pointer writes");
        super::apply_effect(
            tool,
            &serde_json::json!({"issue": 56, "run_id": quiet.run_id}),
            Some(&serde_json::json!({"ok": true})),
            &mut quiet,
            &context,
        );
        let after = crate::harness::session::load(&context.state_root, &quiet.run_id);
        assert_eq!(
            after.state, None,
            "{silent} carries no state and stamped one the tracker never named"
        );
        assert_eq!(
            after.issue,
            Some(crate::identity::Identity::Number(56)),
            "{silent} completed nothing at all"
        );
    }
}

#[test]
fn a_renewal_records_the_checkout_a_resumed_run_adopted() {
    let repo = repository_with_commit();
    let elsewhere = tempfile::tempdir().expect("somewhere for the adopted checkout");
    let adopted = elsewhere.path().join("issue-30");
    register_adopted_worktree(repo.path(), &adopted, "issue-30", "claude-earlier00", 30);

    let root = tempfile::tempdir().expect("a state root");
    let context = bare_context(&root, repo.path());
    let renew = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("the tool is listed");
    assert_eq!(
        renew.effect,
        super::PointerEffect::Renew,
        "this test is measuring the wrong tool"
    );

    let mut run = crate::harness::session::Run::new("claude-resumed0000".to_owned());
    run.issue = Some(crate::identity::Identity::Number(30));
    run.repo_dir = Some(repo.path().to_path_buf());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    super::apply_effect(
        renew,
        &serde_json::json!({
            "issue": 30, "run_id": "claude-resumed0000", "expect_state": "in-progress",
        }),
        Some(&serde_json::json!({"ok": true, "issue": 30, "state": "in-progress"})),
        &mut run,
        &context,
    );

    let after = crate::harness::session::load(&context.state_root, "claude-resumed0000");
    let recorded = after
        .worktree
        .as_deref()
        .and_then(|path| std::fs::canonicalize(path).ok());
    assert_eq!(
        recorded,
        std::fs::canonicalize(&adopted).ok(),
        "a renewal did not discover the checkout this run adopted: {:?}",
        after.worktree
    );

    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        context.tracker.named_repo(),
    );
    let listed = crate::transport::branch::discover_adopted_worktree(
        &transport,
        &crate::identity::Identity::Number(30),
    )
    .expect("the listing reads")
    .expect("the adopted checkout is still registered");
    let expected = crate::transport::branch::canonical_worktree_path(&listed)
        .expect("the listed path resolves");
    assert_eq!(
        after.worktree.as_deref().map(std::path::Path::as_os_str),
        Some(expected.as_os_str()),
        "the recorded worktree is not the exact canonicalised spelling of the path git listed: {:?}",
        after.worktree
    );
}

#[test]
fn a_reclaim_records_the_checkout_it_takes_over() {
    let repo = repository_with_commit();
    let elsewhere = tempfile::tempdir().expect("somewhere for the adopted checkout");
    let adopted = elsewhere.path().join("issue-37");
    register_adopted_worktree(repo.path(), &adopted, "issue-37", "claude-abandoned", 37);

    let root = tempfile::tempdir().expect("a state root");
    let context = bare_context(&root, repo.path());
    let reclaim = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .expect("the tool is listed");
    assert_eq!(
        reclaim.effect,
        super::PointerEffect::Swear,
        "this test is measuring the wrong tool"
    );

    let fresh = crate::harness::session::Run::new("claude-takeover0".to_owned());
    crate::harness::session::store(&context.state_root, &fresh).expect("the pointer writes");
    let mut run = crate::harness::session::load(&context.state_root, "claude-takeover0");

    super::apply_effect(
        reclaim,
        &serde_json::json!({"issue": 37, "run_id": "claude-takeover0"}),
        Some(&serde_json::json!({"ok": true, "target_operation": "a".repeat(32)})),
        &mut run,
        &context,
    );

    let after = crate::harness::session::load(&context.state_root, "claude-takeover0");
    let recorded = after
        .worktree
        .as_deref()
        .and_then(|path| std::fs::canonicalize(path).ok());
    assert_eq!(
        recorded,
        std::fs::canonicalize(&adopted).ok(),
        "a reclaim did not discover the checkout the takeover adopted: {:?}",
        after.worktree
    );

    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        context.tracker.named_repo(),
    );
    let listed = crate::transport::branch::discover_adopted_worktree(
        &transport,
        &crate::identity::Identity::Number(37),
    )
    .expect("the listing reads")
    .expect("the adopted checkout is still registered");
    let expected = crate::transport::branch::canonical_worktree_path(&listed)
        .expect("the listed path resolves");
    assert_eq!(
        after.worktree.as_deref().map(std::path::Path::as_os_str),
        Some(expected.as_os_str()),
        "the recorded worktree is not the exact canonicalised spelling of the path git listed: {:?}",
        after.worktree
    );
}

/// A write inside the adopted checkout is adjudicated against the claim, not
/// waved through as landing nowhere the run swore to.
///
/// Before discovery, `covered()` holds `repo_dir` alone, so a write inside
/// the adopted checkout answers [`crate::harness::Aside::OutsideTheClaim`] —
/// which **permits**, since `Outside` means *"not Estigia's call"*. Calling
/// that a refusal would describe the defect this test exists for as its opposite. Once the renewal records the checkout the same
/// write must be adjudicated against the claim instead — and if that fill
/// is removed, the second half of this test answers exactly like the first.
#[test]
fn a_write_inside_an_adopted_checkout_is_gated_once_a_renewal_records_it() {
    let repo = repository_with_commit();
    let elsewhere = tempfile::tempdir().expect("somewhere for the adopted checkout");
    let adopted = elsewhere.path().join("issue-31");
    register_adopted_worktree(repo.path(), &adopted, "issue-31", "claude-earlier00", 31);

    let root = tempfile::tempdir().expect("a state root");
    // The window is held at zero rather than the ordinary default: a
    // renewal that just answered `ok` also stamps `verified_at`, and a live
    // window would let the *second* `gate` call below answer `Allow` on the
    // renewal alone, without ever reaching the coverage check this test
    // measures.
    let context = GateContext {
        window: std::time::Duration::from_secs(0),
        ..bare_context(&root, repo.path())
    };
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");

    let renew = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("the tool is listed");

    let mut run = crate::harness::session::Run::new("claude-resumed0001".to_owned());
    run.issue = Some(crate::identity::Identity::Number(31));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.path().to_path_buf());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    let target = adopted.join("src").join("main.rs");
    let action = crate::harness::Action::Write {
        target: target.display().to_string(),
    };

    let before = crate::harness::gate(
        &context,
        &mut run.clone(),
        &action,
        crate::harness::Sensitivity::Routine,
    );
    assert!(
        matches!(
            before,
            crate::harness::Decision::Outside(crate::harness::Aside::OutsideTheClaim)
        ),
        "the fixture no longer reproduces the defect this test measures: {before:?}"
    );

    super::apply_effect(
        renew,
        &serde_json::json!({
            "issue": 31, "run_id": "claude-resumed0001", "expect_state": "in-progress",
        }),
        Some(&serde_json::json!({"ok": true, "issue": 31, "state": "in-progress"})),
        &mut run,
        &context,
    );
    assert!(
        run.worktree.is_some(),
        "the renewal did not record the checkout this test's second half depends on"
    );

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-resumed0001",
        31,
        "in-progress",
    )]);
    let after = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        crate::harness::gate(
            &context,
            &mut run,
            &action,
            crate::harness::Sensitivity::Routine,
        )
    });
    assert!(
        !matches!(
            after,
            crate::harness::Decision::Outside(crate::harness::Aside::OutsideTheClaim)
        ),
        "recording the adopted checkout did not stop a write inside it reading as outside the \
         claim: {after:?}"
    );
    assert!(
        matches!(after, crate::harness::Decision::Allow(_)),
        "the write stopped reading as outside the claim, but it was not cleanly adjudicated \
         either: {after:?}"
    );
}

/// Two runs whose worktrees differ are not reported as holding one checkout.
///
/// `the_closest_claim_holds_a_directory_two_runs_both_cover` measures
/// `holders_of`'s own resolution with the worktrees set by hand; this drives
/// the same shape through discovery, so a defect that let two distinct
/// adoptions collapse onto one recorded checkout is caught here too. The
/// base checkout is genuinely ambiguous between the two claims — the same
/// floor that test also asserts — and each run's own adopted checkout is
/// not.
#[test]
fn two_adopted_checkouts_are_not_conflated_into_one_holder() {
    let repo = repository_with_commit();
    let elsewhere = tempfile::tempdir().expect("somewhere for the adopted checkouts");
    let first = elsewhere.path().join("issue-32");
    let second = elsewhere.path().join("issue-33");
    register_adopted_worktree(repo.path(), &first, "issue-32", "claude-firstrun0", 32);
    register_adopted_worktree(repo.path(), &second, "issue-33", "claude-secondrun", 33);

    let root = tempfile::tempdir().expect("a state root");
    let context = bare_context(&root, repo.path());
    let renew = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("the tool is listed");

    let mut run_a = crate::harness::session::Run::new("claude-firstrun0".to_owned());
    run_a.issue = Some(crate::identity::Identity::Number(32));
    run_a.repo_dir = Some(repo.path().to_path_buf());
    crate::harness::session::store(&context.state_root, &run_a).expect("the pointer writes");
    super::apply_effect(
        renew,
        &serde_json::json!({
            "issue": 32, "run_id": "claude-firstrun0", "expect_state": "in-progress",
        }),
        Some(&serde_json::json!({"ok": true, "issue": 32, "state": "in-progress"})),
        &mut run_a,
        &context,
    );

    let mut run_b = crate::harness::session::Run::new("claude-secondrun".to_owned());
    run_b.issue = Some(crate::identity::Identity::Number(33));
    run_b.repo_dir = Some(repo.path().to_path_buf());
    crate::harness::session::store(&context.state_root, &run_b).expect("the pointer writes");
    super::apply_effect(
        renew,
        &serde_json::json!({
            "issue": 33, "run_id": "claude-secondrun", "expect_state": "in-progress",
        }),
        Some(&serde_json::json!({"ok": true, "issue": 33, "state": "in-progress"})),
        &mut run_b,
        &context,
    );

    assert!(
        run_a.worktree.is_some(),
        "the first run's adoption was not recorded"
    );
    assert!(
        run_b.worktree.is_some(),
        "the second run's adoption was not recorded"
    );
    assert_ne!(
        run_a.worktree, run_b.worktree,
        "two distinct adoptions were recorded as one checkout"
    );

    for (path, expected_issue) in [(&first, 32u64), (&second, 33u64)] {
        let holders = crate::harness::guard::holders_of(&context.state_root, path);
        assert_eq!(
            holders.len(),
            1,
            "{} run(s) were found holding {}, not one",
            holders.len(),
            path.display()
        );
        assert_eq!(
            holders[0].issue,
            Some(crate::identity::Identity::Number(expected_issue)),
            "the wrong run was found holding {}",
            path.display()
        );
    }

    // The floor: at the shared base checkout neither claim is closer, so the
    // ambiguity is real and both remain holders there.
    assert_eq!(
        crate::harness::guard::holders_of(&context.state_root, repo.path()).len(),
        2,
        "two runs holding one checkout stopped being an ambiguity"
    );
}

/// The ungated direction: a boundary action from the adopted checkout must
/// stop passing as unsworn once the checkout is recorded.
///
/// This is the shape of the defect: nothing covered the adopted
/// checkout, so a push made from it found zero holders and answered
/// [`crate::harness::Aside::NothingSworn`], which *permits* — standing
/// aside is not a refusal.
/// `two_adopted_checkouts_are_not_conflated_into_one_holder` is the
/// collision direction; this is the silent half.
#[test]
fn a_push_from_the_adopted_checkout_stops_reading_as_unsworn_once_recorded() {
    let repo = repository_with_commit();
    let elsewhere = tempfile::tempdir().expect("somewhere for the adopted checkout");
    let adopted = elsewhere.path().join("issue-34");
    register_adopted_worktree(repo.path(), &adopted, "issue-34", "claude-earlier00", 34);

    let root = tempfile::tempdir().expect("a state root");
    let context = bare_context(&root, &adopted);
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");

    let mut run = crate::harness::session::Run::new("claude-resumed0002".to_owned());
    run.issue = Some(crate::identity::Identity::Number(34));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.path().to_path_buf());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    let before = crate::harness::guard::decide(&context, &adopted, &[]).decision;
    assert!(
        matches!(
            before,
            crate::harness::Decision::Outside(crate::harness::Aside::NothingSworn)
        ),
        "the fixture no longer reproduces the defect this test measures: {before:?}"
    );

    let renew = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("the tool is listed");
    super::apply_effect(
        renew,
        &serde_json::json!({
            "issue": 34, "run_id": "claude-resumed0002", "expect_state": "in-progress",
        }),
        Some(&serde_json::json!({"ok": true, "issue": 34, "state": "in-progress"})),
        &mut run,
        &context,
    );

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-resumed0002",
        34,
        "in-progress",
    )]);
    let after = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        crate::harness::guard::decide(&context, &adopted, &[]).decision
    });
    assert!(
        !matches!(
            after,
            crate::harness::Decision::Outside(crate::harness::Aside::NothingSworn)
        ),
        "recording the adopted checkout did not stop a push from it reading as unsworn: \
         {after:?}"
    );
    assert!(
        matches!(after, crate::harness::Decision::Allow(_)),
        "the push stopped reading as unsworn, but it was not cleanly adjudicated either: \
         {after:?}"
    );
}

#[test]
fn discovery_records_nothing_it_cannot_prove_and_refuses_nothing_on_its_own_account() {
    let renew = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("the tool is listed");

    let repo = repository_with_commit();
    {
        let elsewhere = tempfile::tempdir().expect("somewhere for the checkout");
        let adopted = elsewhere.path().join("issue-41");
        register_adopted_worktree(repo.path(), &adopted, "issue-41", "claude-earlier00", 41);
        let marker = crate::transport::branch::ownership_path(&adopted)
            .expect("git names the admin directory");
        std::fs::write(&marker, "{not json").expect("the marker is corrupted");

        let root = tempfile::tempdir().expect("a state root");
        let context = bare_context(&root, repo.path());
        let mut run = crate::harness::session::Run::new("claude-run00000041".to_owned());
        run.issue = Some(crate::identity::Identity::Number(41));
        run.repo_dir = Some(repo.path().to_path_buf());
        crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

        let recorded = super::apply_effect(
            renew,
            &serde_json::json!({
                "issue": 41, "run_id": run.run_id.clone(), "expect_state": "in-progress",
            }),
            Some(&serde_json::json!({"ok": true, "issue": 41, "state": "in-progress"})),
            &mut run,
            &context,
        );
        assert!(
            recorded,
            "an unreadable ownership marker turned an otherwise-successful renewal into one \
             this run's own pointer failed to record"
        );
        let after = crate::harness::session::load(&context.state_root, &run.run_id);
        assert_eq!(
            after.worktree, None,
            "an unreadable marker was read as though it proved something"
        );
        assert_eq!(
            after.issue,
            Some(crate::identity::Identity::Number(41)),
            "the renewal itself was refused by a marker it never asked to read"
        );
    }

    let repo = repository_with_commit();
    {
        let elsewhere = tempfile::tempdir().expect("somewhere for the checkouts");
        let first = elsewhere.path().join("issue-42-a");
        let second = elsewhere.path().join("issue-42-b");
        register_adopted_worktree(repo.path(), &first, "issue-42-a", "claude-earlier00", 42);
        register_adopted_worktree(repo.path(), &second, "issue-42-b", "claude-earlier01", 42);

        let root = tempfile::tempdir().expect("a state root");
        let context = bare_context(&root, repo.path());
        let mut run = crate::harness::session::Run::new("claude-run00000042".to_owned());
        run.issue = Some(crate::identity::Identity::Number(42));
        run.repo_dir = Some(repo.path().to_path_buf());
        crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

        let recorded = super::apply_effect(
            renew,
            &serde_json::json!({
                "issue": 42, "run_id": run.run_id.clone(), "expect_state": "in-progress",
            }),
            Some(&serde_json::json!({"ok": true, "issue": 42, "state": "in-progress"})),
            &mut run,
            &context,
        );
        assert!(
            recorded,
            "an ambiguous discovery turned an otherwise-successful renewal into a refusal"
        );
        let after = crate::harness::session::load(&context.state_root, &run.run_id);
        assert_eq!(
            after.worktree, None,
            "two checkouts naming one issue were resolved by a guess instead of by neither"
        );
    }

    {
        let not_a_repo = tempfile::tempdir().expect("a directory that is not a repository");
        let root = tempfile::tempdir().expect("a state root");
        let context = bare_context(&root, not_a_repo.path());
        let mut run = crate::harness::session::Run::new("claude-run00000043".to_owned());
        run.issue = Some(crate::identity::Identity::Number(43));
        run.repo_dir = Some(not_a_repo.path().to_path_buf());
        crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

        let recorded = super::apply_effect(
            renew,
            &serde_json::json!({
                "issue": 43, "run_id": run.run_id.clone(), "expect_state": "in-progress",
            }),
            Some(&serde_json::json!({"ok": true, "issue": 43, "state": "in-progress"})),
            &mut run,
            &context,
        );
        assert!(
            recorded,
            "a listing git refused turned an otherwise-successful renewal into one this run's \
             own pointer failed to record"
        );
        let after = crate::harness::session::load(&context.state_root, &run.run_id);
        assert_eq!(
            after.worktree, None,
            "a listing git refused was recorded from anyway"
        );
    }
}

#[test]
fn every_receipt_effect_recovers_the_complete_receipt_atomically() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let receipt = crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: "b".repeat(40),
        base: "c".repeat(40),
        digest: "d".repeat(64),
    };
    let receipt_json = serde_json::json!({
        "epoch": receipt.epoch,
        "pr": receipt.pr,
        "head": receipt.head,
        "base": receipt.base,
        "digest": receipt.digest,
    });

    for name in ["publish_review", "republish_review", "release_ci"] {
        let tool = super::tools::TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect("the tool is listed");
        let mut run = crate::harness::session::Run::new(format!("claude-{name}"));
        let arguments = if name == "release_ci" {
            receipt_json.clone()
        } else {
            serde_json::json!({})
        };
        let body = if matches!(name, "publish_review" | "republish_review") {
            receipt_json.clone()
        } else {
            serde_json::json!({"ok": true})
        };
        super::apply_effect(tool, &arguments, Some(&body), &mut run, &context);
        assert_eq!(
            run.review_transaction.as_ref(),
            Some(&receipt),
            "{name} did not restore all five receipt fields"
        );
        assert_eq!(run.reviewed_head, None);
    }
}

#[test]
fn a_claim_adopts_the_complete_review_transaction_its_answer_carries() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let receipt = crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: "b".repeat(40),
        base: "c".repeat(40),
        digest: "d".repeat(64),
    };
    let tool = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .expect("the tool is listed");
    let complete = serde_json::json!({
        "ok": true,
        "review_transaction": {
            "epoch": receipt.epoch,
            "pr": receipt.pr,
            "head": receipt.head,
            "base": receipt.base,
            "digest": receipt.digest,
        },
    });
    let partial = serde_json::json!({
        "ok": true,
        "review_transaction": {"epoch": receipt.epoch, "pr": receipt.pr},
    });

    let mut run = crate::harness::session::Run::new("claude-claim".to_owned());
    run.reviewed_head = Some("e".repeat(40));
    super::apply_effect(
        tool,
        &serde_json::json!({}),
        Some(&complete),
        &mut run,
        &context,
    );
    assert_eq!(run.review_transaction.as_ref(), Some(&receipt));
    assert_eq!(run.reviewed_head, None);

    for (index, body) in [serde_json::json!({"ok": true}), partial]
        .into_iter()
        .enumerate()
    {
        let mut run = crate::harness::session::Run::new(format!("claude-claim-{index}"));
        super::apply_effect(
            tool,
            &serde_json::json!({}),
            Some(&body),
            &mut run,
            &context,
        );
        assert_eq!(run.review_transaction, None, "{body}");
    }
}

#[test]
fn a_partial_review_transaction_invalidates_old_local_authority() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let old_receipt = crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: "b".repeat(40),
        base: "c".repeat(40),
        digest: "d".repeat(64),
    };

    for name in ["publish_review", "republish_review"] {
        let tool = super::tools::TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect("the tool is listed");
        let mut run = crate::harness::session::Run::new(format!("claude-partial-{name}"));
        run.review_transaction = Some(old_receipt.clone());
        run.reviewed_head = Some("e".repeat(40));
        crate::harness::session::store(&context.state_root, &run)
            .expect("the old authority writes");

        super::apply_effect(
            tool,
            &serde_json::json!({}),
            Some(&serde_json::json!({
                "epoch": "f".repeat(32),
                "pr": 55,
                "head": "1".repeat(40),
                "base": "2".repeat(40)
            })),
            &mut run,
            &context,
        );

        assert_eq!(
            run.review_transaction, None,
            "{name} retained an old receipt"
        );
        assert_eq!(run.reviewed_head, None, "{name} retained a legacy head");
    }
}

#[test]
fn every_two_step_operation_has_a_tool_that_can_take_the_second_step() {
    let two_step: Vec<String> = transport_source()
        .match_indices("\n        \"")
        .map(|(at, _)| {
            let source = transport_source();
            let rest = source[at + 10..].to_owned();
            rest[..rest.find('"').unwrap_or(0)].to_owned()
        })
        .filter(|name| {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                && transport_body(name).contains("\"write_performed\": false")
        })
        .collect();
    assert!(
        two_step.len() >= 2,
        "the transport stopped answering anything in two steps, which is not a \
         thing to quietly agree with: {two_step:?}"
    );

    for operation in &two_step {
        let tool = TOOLS
            .iter()
            .find(|tool| tool.operation == operation)
            .unwrap_or_else(|| panic!("`{operation}` answers in two steps and no tool runs it"));
        assert!(
            tool.arguments
                .iter()
                .any(|argument| argument.name == "target_operation"),
            "`{operation}` answers its first call with `write_performed: false` and asks to be \
             repeated naming what it found — and `{}` has no argument to name it with, so the \
             write it exists for cannot be reached",
            tool.name
        );
    }
}

#[test]
fn discovering_a_takeover_does_not_make_the_run_its_holder() {
    // `reclaim` answers its read-only first call `write_performed: false`
    // too, and its effect is `Swear` — so that call used to set the issue,
    // the state and the verification stamp the renewal window rides on. A
    // run that had taken over nothing read as the verified holder of an
    // issue somebody else still held, and its writes went through the gate.
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let reclaim = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .expect("the tool is listed");
    assert_eq!(
        reclaim.effect,
        super::tools::PointerEffect::Swear,
        "this test is about what `Swear` on a discovery call would do"
    );

    let fresh = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    crate::harness::session::store(&context.state_root, &fresh).expect("the pointer writes");
    let mut run = crate::harness::session::load(&context.state_root, "claude-abcd1234");

    super::apply_effect(
        reclaim,
        &serde_json::json!({"issue": 7, "run_id": "claude-abcd1234"}),
        Some(&serde_json::json!({
            "ok": true,
            "write_performed": false,
            "target_operation": "a".repeat(32)
        })),
        &mut run,
        &context,
    );
    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(
        after.issue, None,
        "a run that took over nothing was recorded as holding the issue"
    );
    assert_eq!(
        after.verified_at, None,
        "a read-only call granted a renewal window"
    );
}

#[test]
fn reclaiming_review_leaves_the_pointer_reading_review() {
    let root = tempfile::tempdir().expect("a state root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the checkout");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: repo,
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let reclaim = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .expect("the tool is listed");
    assert!(
        reclaim
            .arguments
            .iter()
            .any(|argument| argument.name == "state"),
        "reclaim has no state argument, so a takeover cannot name the state it is taking"
    );
    let state = reclaim
        .arguments
        .iter()
        .find(|argument| argument.name == "state")
        .expect("state");
    assert_eq!(
        state.choices,
        Some(crate::config::STATES),
        "reclaim.state is not the workflow vocabulary"
    );

    let fresh = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    crate::harness::session::store(&context.state_root, &fresh).expect("the pointer writes");
    let mut run = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    let mut arguments = serde_json::json!({
        "issue": 35,
        "run_id": "claude-abcd1234",
    });
    if reclaim
        .arguments
        .iter()
        .any(|argument| argument.name == "state")
    {
        arguments["state"] = serde_json::json!("review");
    }
    super::apply_effect(
        reclaim,
        &arguments,
        Some(&serde_json::json!({ "ok": true })),
        &mut run,
        &context,
    );
    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(
        after.state.as_deref(),
        Some("review"),
        "a reclaim of review stamped a state nobody named: {:?}",
        after.state
    );
    assert_eq!(after.issue, Some(crate::identity::Identity::Number(35)));

    let mut again = crate::harness::session::Run::new("claude-efef5678".to_owned());
    crate::harness::session::store(&context.state_root, &again).expect("the pointer writes");
    again = crate::harness::session::load(&context.state_root, "claude-efef5678");
    super::apply_effect(
        reclaim,
        &serde_json::json!({ "issue": 35, "run_id": "claude-efef5678" }),
        Some(&serde_json::json!({ "ok": true })),
        &mut again,
        &context,
    );
    let defaulted = crate::harness::session::load(&context.state_root, "claude-efef5678");
    assert_eq!(
        defaulted.state.as_deref(),
        Some("in-progress"),
        "reclaim without state did not keep claim's documented default"
    );
}

#[test]
fn no_tool_bound_to_a_run_accepts_the_name_that_names_none() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };

    let nameless = format!(
        "{}-{}",
        crate::harness::session::DEFAULT_RUNTIME,
        crate::harness::session::NAMELESS
    );
    let mut bound = 0;
    for tool in TOOLS {
        if !tool.arguments.iter().any(|a| a.name == "run_id") {
            continue;
        }
        bound += 1;
        let mut arguments = serde_json::Map::new();
        for argument in tool.arguments.iter().filter(|a| a.required) {
            arguments.insert(
                argument.name.to_owned(),
                match argument.json_type {
                    "integer" => json!(12),
                    "boolean" => json!(true),
                    _ => argument
                        .choices
                        .map_or_else(|| json!("x"), |choices| json!(choices[0])),
                },
            );
        }
        arguments.insert("run_id".to_owned(), json!(nameless));

        let failure = run_tool(tool.name, &Value::Object(arguments), Ok(&context))
            .expect_err("a run id naming no run is not one to act under");
        let ToolFailure::Refused(refusal) = failure else {
            panic!(
                "{}: refused as malformed rather than by the rule",
                tool.name
            );
        };
        assert_eq!(
            refusal.code, "run-id-names-no-run",
            "{} was refused for something else first: {refusal}",
            tool.name
        );
    }
    assert!(
        bound >= 7,
        "only {bound} tools were seen to take a run id, so this checked almost nothing"
    );

    let ordinary =
        json!({"issue": 12, "run_id": "claude-abcd1234", "horizon": "2099-01-01T00:00Z"});
    if let Err(ToolFailure::Refused(refusal)) = run_tool("claim", &ordinary, Ok(&context)) {
        assert_ne!(
            refusal.code, "run-id-names-no-run",
            "a named run was told its name names nobody"
        );
    }
}

#[test]
fn refusing_a_nameless_run_names_a_way_out_that_is_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let nameless = format!(
        "{}-{}",
        crate::harness::session::DEFAULT_RUNTIME,
        crate::harness::session::NAMELESS
    );
    let arguments = json!({"issue": 7, "run_id": nameless, "horizon": "2099-01-01T00:00Z"});
    let refusal = |context: &GateContext| match run_tool("claim", &arguments, Ok(context)) {
        Err(ToolFailure::Refused(refusal)) => *refusal,
        other => panic!("a name that names no run was accepted: {other:?}"),
    };

    let asking = refusal(&context);
    assert_eq!(asking.code, "run-id-names-no-run");
    assert!(
        format!("{asking}").contains("status"),
        "a session with no identity was not told where to get one: {asking}"
    );

    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    let orphan = crate::harness::session::pointer_path(&context.state_root, &nameless);
    std::fs::write(
        &orphan,
        format!("{{\"run_id\":\"{nameless}\",\"issue\":12}}"),
    )
    .expect("an orphaned pointer");

    let stranded = refusal(&context);
    assert_eq!(stranded.code, "run-id-names-no-run");
    let said = format!("{stranded}");
    assert!(
        said.contains(&orphan.display().to_string()),
        "the one file that can be acted on is not named: {said}"
    );
    assert!(
        !said.contains("estigia status"),
        "the way out is the command that names this one: {said}"
    );
    assert!(
        said.contains("tracker"),
        "removing the pointer was made to sound like releasing the claim: {said}"
    );

    std::fs::remove_file(&orphan).expect("the operator takes it away");
    assert!(
        !format!("{}", refusal(&context)).contains(&orphan.display().to_string()),
        "the way out was taken and the message still names it"
    );
}

/// `runtime_of` returns `""` for a run id whose only `-` leads,
/// like `-abcd1234`. Pushed through unchecked, `run_tool` would send
/// `--runtime ""` and project `dev:`.
#[test]
fn a_run_id_with_no_runtime_is_refused_at_the_mcp_boundary() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let arguments = json!({"issue": 7, "run_id": "-abcd1234", "horizon": "2099-01-01T00:00Z"});
    let failure = run_tool("claim", &arguments, Ok(&context))
        .expect_err("a run id with no runtime is not one to act under");
    let ToolFailure::Refused(refusal) = failure else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "run-id-names-no-run");
    assert!(
        format!("{refusal}").contains("runtime"),
        "the refusal did not say what was actually wrong with the id: {refusal}"
    );
}

/// `runtime_of` falls back to the **whole** id when it
/// holds no `-` at all, so a caller-supplied `run_id` of `myrun` used to reach
/// `src/transport/claim.rs:6640` and project `dev:myrun` — a permanent
/// repository-level label — rather than being refused the way `-abcd1234`
/// already was.
#[test]
fn a_dashless_run_id_is_refused_at_the_mcp_boundary() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let arguments = json!({"issue": 7, "run_id": "myrun", "horizon": "2099-01-01T00:00Z"});
    let failure = run_tool("claim", &arguments, Ok(&context))
        .expect_err("a run id with no `-` is not one to act under");
    let ToolFailure::Refused(refusal) = failure else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "run-id-names-no-run");
    let said = format!("{refusal}");
    assert!(
        said.contains("myrun"),
        "the refusal did not name the id it refused: {refusal}"
    );
    assert!(
        said.contains("no `-`"),
        "the refusal did not say what was actually wrong with the id: {refusal}"
    );
}

/// `runtime_of` splits on the **last** `-`, so any id
/// with at least one dash and a non-empty prefix used to pass — even one
/// whose prefix is not a runtime at all — and that prefix reached
/// `gh label create dev:{runtime} --color bfd4f2 --force`, minting a
/// permanent repository-level label from caller-supplied text.
#[test]
fn a_run_id_with_an_unsafe_runtime_is_refused_at_the_mcp_boundary() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    for unsafe_id in ["-a-b", "a/b-cd", "My Runtime-abcd1234"] {
        let arguments = json!({"issue": 7, "run_id": unsafe_id, "horizon": "2099-01-01T00:00Z"});
        let failure = run_tool("claim", &arguments, Ok(&context))
            .expect_err(&format!("{unsafe_id} was accepted rather than refused"));
        let ToolFailure::Refused(refusal) = failure else {
            panic!("{unsafe_id}: refused as malformed rather than by the rule");
        };
        assert_eq!(
            refusal.code, "run-id-names-no-run",
            "{unsafe_id} was refused for something else first: {refusal}"
        );
    }
}

/// The required arguments of one tool, as the widest cheap filler: a whole
/// number for integers, a string or the first declared choice otherwise.
///
/// The valued argument under test is inserted by its caller, after this.
fn required_arguments(tool: &tools::Tool) -> serde_json::Map<String, Value> {
    let mut arguments = serde_json::Map::new();
    for argument in tool.arguments.iter().filter(|argument| argument.required) {
        arguments.insert(
            argument.name.to_owned(),
            match argument.json_type {
                "integer" => json!(12),
                "boolean" => json!(true),
                _ => argument
                    .choices
                    .map_or_else(|| json!("x"), |choices| json!(choices[0])),
            },
        );
    }
    arguments
}

fn arg_context(root: &tempfile::TempDir) -> GateContext {
    GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

/// The shape of a run id is decided in one place, and this is what
/// reddens when the gate stops asking. Every tool that takes a run id answers
/// the predicate's verdict over a corpus that spans the **class** — the
/// minted shapes, the nameless one, a suffix that carries a dash, and the
/// three spellings closed one at a time — rather than one test per
/// spelling. A disjunct re-added beside the predicate, a check kept in one
/// arm, or a tool that pushes no `--runtime` skipping the check, all make the
/// gate admit a refused shape and this names the tool and the id.
#[test]
fn every_tool_that_takes_a_run_id_refuses_the_shapes_the_predicate_refuses() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = arg_context(&root);

    let unminted = [
        ("codex-01k4-9f2a", session::Unminted::Suffix),
        ("claude-3edc7a52-5e11", session::Unminted::Suffix),
        ("claude-a", session::Unminted::Suffix),
        ("myrun", session::Unminted::NoRuntime),
        ("-abcd1234", session::Unminted::Runtime),
        ("My Runtime-abcd1234", session::Unminted::Runtime),
        ("a/b-cd", session::Unminted::Runtime),
    ];
    let mut bound = 0;
    for tool in tools::TOOLS {
        if !tool
            .arguments
            .iter()
            .any(|argument| argument.name == "run_id")
        {
            continue;
        }
        bound += 1;
        for (id, why) in unminted {
            assert_eq!(
                session::run_id_shape(id),
                session::RunIdShape::Unminted(why),
                "{id}: the predicate no longer refuses this shape, so this test's other half — \
                 the gate agreeing with it — is measuring something else"
            );
            let mut arguments = required_arguments(tool);
            arguments.insert("run_id".to_owned(), json!(id));
            let failure = match run_tool(tool.name, &Value::Object(arguments), Ok(&context)) {
                Err(failure) => failure,
                Ok(answer) => panic!("{} accepted {id}: {answer}", tool.name),
            };
            let ToolFailure::Refused(refusal) = failure else {
                panic!(
                    "{}: {id} was refused as malformed rather than by the rule",
                    tool.name
                );
            };
            assert_eq!(
                refusal.code, "run-id-names-no-run",
                "{} was refused for something else first under {id}: {refusal}",
                tool.name
            );
        }

        // The nameless shape is minted and still names no run: refused by the
        // same code, from the predicate's Nameless answer rather than from a
        // spelling.
        let mut arguments = required_arguments(tool);
        arguments.insert("run_id".to_owned(), json!("claude-unknown"));
        let failure = match run_tool(tool.name, &Value::Object(arguments), Ok(&context)) {
            Err(failure) => failure,
            Ok(answer) => panic!("{} accepted the nameless id: {answer}", tool.name),
        };
        let ToolFailure::Refused(refusal) = failure else {
            panic!(
                "{}: the nameless id was refused as malformed rather than by the rule",
                tool.name
            );
        };
        assert_eq!(
            refusal.code, "run-id-names-no-run",
            "{} was refused for something else first under the nameless id: {refusal}",
            tool.name
        );
    }
    assert!(
        bound >= 18,
        "only {bound} tools were seen to take a run id, so this checked almost nothing"
    );

    // The floor, or a gate that refuses every id would pass everything above.
    let minted = json!({"issue": 12, "run_id": "claude-aaaa1111", "horizon": "2099-01-01T00:00Z"});
    if let Err(ToolFailure::Refused(refusal)) = run_tool("claim", &minted, Ok(&context)) {
        assert_ne!(
            refusal.code, "run-id-names-no-run",
            "a minted id was told it is not one this crate could have minted"
        );
    }
}

/// A `run_id` sent as a JSON number answered `None` to
/// every `Value::as_str` guard at once, so all four refusals were skipped and
/// `flags_for` rendered the number straight into `--run-id`. The read is typed
/// now, and this is per tool because that is how the door was found — a guard
/// defeated in one call is defeated in every call that reads the same string.
#[test]
fn a_run_id_sent_as_a_json_number_is_refused_by_every_tool_that_takes_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = arg_context(&root);
    let mut bound = 0;
    for tool in tools::TOOLS {
        if !tool
            .arguments
            .iter()
            .any(|argument| argument.name == "run_id")
        {
            continue;
        }
        bound += 1;
        for sent in [json!(7), json!(true)] {
            let mut arguments = required_arguments(tool);
            arguments.insert("run_id".to_owned(), sent.clone());
            let failure = match run_tool(tool.name, &Value::Object(arguments), Ok(&context)) {
                Err(failure) => failure,
                Ok(answer) => panic!("{} took {sent} for a run id: {answer}", tool.name),
            };
            let ToolFailure::Malformed(said) = failure else {
                panic!(
                    "{}: {sent} was answered by the world rather than refused",
                    tool.name
                );
            };
            assert!(
                said.contains("`run_id`") && said.contains("string"),
                "{}: the refusal does not say a run id is a string: {said}",
                tool.name
            );
        }
    }
    assert!(
        bound >= 18,
        "only {bound} tools were seen to take a run id, so this checked almost nothing"
    );
}

/// The validated answer carries the runtime, and a dashed one survives whole:
/// the split that reads `claude-code` out of `claude-code-60fabae1` is the
/// predicate's own, so nothing downstream re-splits the id and nothing can
/// disagree with the shape that admitted it.
#[test]
fn a_dashed_runtime_survives_the_validated_id_whole() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = arg_context(&root);
    let tool = tools::find("claim").expect("the tool exists");
    let arguments = json!({"issue": 12, "run_id": "claude-code-60fabae1"});
    let named = super::named_run(tool, &arguments, &context)
        .expect("a minted shape is not refused")
        .expect("a claim names a run");
    assert_eq!(named.id, "claude-code-60fabae1");
    assert_eq!(named.runtime, "claude-code");
}

/// `create`'s `runtime` was a free-form string, and
/// `analyst:<runtime>` is a repository-level label that outlives the issue —
/// so caller text became a permanent label nobody's query finds. It is held
/// to `session::runtime_component`, the same function the runtime derived from
/// a run id goes through, and read from the argument so a number cannot skip
/// it the way door 2 did.
#[test]
fn create_holds_its_declared_runtime_to_the_same_component_as_a_derived_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = arg_context(&root);
    let create = tools::find("create").expect("the tool exists");
    let with_runtime = |runtime: Value| {
        let mut arguments = required_arguments(create);
        arguments.insert("runtime".to_owned(), runtime);
        arguments.insert("run_id".to_owned(), json!("claude-aaaa1111"));
        run_tool("create", &Value::Object(arguments), Ok(&context))
    };

    for runtime in ["My Runtime", "a/b", "codex 01", ""] {
        let failure = with_runtime(json!(runtime))
            .expect_err(&format!("{runtime:?} was taken for a runtime"));
        let ToolFailure::Refused(refusal) = failure else {
            panic!("{runtime:?}: refused as malformed rather than by the rule");
        };
        assert_eq!(
            refusal.code, "runtime-names-no-runtime",
            "{runtime:?} was refused for something else first: {refusal}"
        );
        assert!(
            format!("{refusal}").contains("analyst"),
            "the refusal does not say which label the text would have minted: {refusal}"
        );
    }

    let failure = with_runtime(json!(7)).expect_err("a number went through for a runtime");
    let ToolFailure::Malformed(said) = failure else {
        panic!("a numeric runtime was answered by the world rather than refused");
    };
    assert!(
        said.contains("`runtime`") && said.contains("string"),
        "the refusal does not say a runtime is a string: {said}"
    );

    // The crossing that makes it *the same* predicate: the two doors agree,
    // value for value, because both call `runtime_component`.
    for runtime in ["codex", "claude-code", "My Runtime", "a/b", ""] {
        assert_eq!(
            super::declared_runtime(&json!({"runtime": runtime})).is_err(),
            crate::harness::session::runtime_component(runtime).is_err(),
            "{runtime}: the declared and the derived runtime are read by different rules"
        );
    }
}

#[test]
fn create_refuses_a_repo_that_is_not_owner_name_before_anything_is_read_or_run() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = arg_context(&root);
    let create = tools::find("create").expect("the tool exists");
    let with_repo = |repo: Value| {
        let mut arguments = required_arguments(create);
        arguments.insert("repo".to_owned(), repo);
        arguments.insert("run_id".to_owned(), json!("claude-aaaa1111"));
        run_tool("create", &Value::Object(arguments), Ok(&context))
    };

    for repo in [
        "",
        "o",
        "o/",
        "/b",
        "o/b/c",
        "o b/c",
        "https://github.com/o/b",
        "o/b;true",
    ] {
        let failure = with_repo(json!(repo)).expect_err(&format!("{repo:?} was taken for a repo"));
        let ToolFailure::Refused(refusal) = failure else {
            panic!("{repo:?}: refused as malformed rather than by the rule");
        };
        assert_eq!(
            refusal.code, "repo-ref-malformed",
            "{repo:?} was refused for something else first: {refusal}"
        );
    }

    let ToolFailure::Malformed(said) = with_repo(json!(7)).expect_err("a number named a repo")
    else {
        panic!("a numeric repo was answered by the world rather than refused");
    };
    assert!(
        said.contains("`repo`") && said.contains("owner/name"),
        "the refusal does not say a repo is owner/name: {said}"
    );
}

#[test]
fn only_create_takes_a_repo_and_it_is_optional() {
    for tool in tools::TOOLS {
        let takes = tool
            .arguments
            .iter()
            .find(|argument| argument.name == "repo");
        match (tool.name, takes) {
            ("create", Some(argument)) => {
                assert!(!argument.required, "`create` demands a repo");
                assert!(tools::is_pointer_only("create", "repo"));
            }
            ("create", None) => panic!("`create` does not take a repo"),
            (other, Some(_)) => panic!("`{other}` takes a repo, which only `create` may"),
            _ => {}
        }
    }
}

#[test]
fn a_publish_names_the_checkout_the_gate_will_measure_against() {
    let tool = tools::find("publish_review").expect("the tool exists");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());

    assert!(worktree_to_name(tool, &[], &run).is_none());

    run.worktree = Some(std::path::PathBuf::from("/w/fix-6"));
    assert_eq!(
        worktree_to_name(tool, &[], &run).as_deref(),
        Some(
            std::path::Path::new("/w/fix-6")
                .display()
                .to_string()
                .as_str()
        ),
        "a run with an isolated checkout published without naming it"
    );

    let theirs = vec!["--worktree".to_owned(), "/w/somewhere-else".to_owned()];
    assert!(
        worktree_to_name(tool, &theirs, &run).is_none(),
        "Estigia named a second worktree over the one the caller chose"
    );

    for reader in ["release_ci", "republish_review"] {
        let tool = tools::find(reader).expect("the tool exists");
        assert_eq!(
            worktree_to_name(tool, &[], &run),
            run.worktree.as_ref().map(|path| path.display().to_string()),
            "{reader} derives a review target from a checkout nobody named"
        );
        assert!(
            worktree_to_name(tool, &theirs, &run).is_none(),
            "{reader} was given a second worktree over the one the caller chose"
        );
    }

    for other in ["claim", "transition", "heartbeat"] {
        let tool = tools::find(other).expect("the tool exists");
        assert!(
            worktree_to_name(tool, &[], &run).is_none(),
            "{other} was handed a worktree it never asked for"
        );
    }
}

#[test]
fn the_operation_epoch_has_one_shape_in_every_tool_that_asks_for_it() {
    let mut asked = 0;
    for tool in crate::harness::mcp::tools::TOOLS {
        for argument in tool.arguments {
            if argument.name != "target_operation" {
                continue;
            }
            asked += 1;
            assert_eq!(
                argument.json_type, "string",
                "{} asks for the operation epoch as {}, and it is 32 hex characters or \
                 `legacy-<id>`",
                tool.name, argument.json_type
            );
        }
    }
    assert!(
        asked > 1,
        "only {asked} tool asks for an operation epoch, so this crossed nothing"
    );
}

#[test]
fn a_tool_that_moves_the_run_pointer_is_a_tool_that_writes() {
    use crate::harness::mcp::tools::PointerEffect;
    let mut moved = 0;
    for tool in crate::harness::mcp::tools::TOOLS {
        let moves = !matches!(tool.effect, PointerEffect::None | PointerEffect::Renew);
        if !moves {
            continue;
        }
        moved += 1;
        assert!(
            tool.writes,
            "`{}` moves the run pointer ({:?}) and publishes itself as read-only",
            tool.name, tool.effect
        );
    }
    // The floor: a table where nothing moved the pointer would pass in
    // silence.
    assert!(
        moved >= 5,
        "only {moved} tools move the pointer, so this crossed almost nothing"
    );
}

/// A whole number is not yet an issue number.
///
/// The value is rendered straight into the transport's argv, where a
/// negative one is read as a **flag**. Measured through the running server:
/// `verify_claim {"issue": -5, …}` failed with `gh issue view failed (1):
/// unknown shorthand flag: '5' in -5` — a wrong argument reported as a
/// broken transport, arriving through a bound nobody published.
#[test]
fn an_integer_that_counts_from_one_is_refused_below_it_and_says_so_in_the_schema() {
    for tool in crate::harness::mcp::tools::TOOLS {
        for argument in tool.arguments {
            if argument.json_type != "integer" {
                continue;
            }
            // Every integer this server takes is a count of something that
            // starts at one; a new one that is genuinely unbounded is a
            // decision, not an omission, and this is where it gets made.
            let least = argument.least.unwrap_or_else(|| {
                panic!(
                    "`{}` on `{}` is an integer with no floor, and it goes into argv",
                    argument.name, tool.name
                )
            });
            assert_eq!(
                least, 1,
                "`{}` on `{}` counts from {least}",
                argument.name, tool.name
            );

            let schema = tool.schema();
            assert_eq!(
                schema["inputSchema"]["properties"][argument.name]["minimum"],
                serde_json::json!(least),
                "`{}` on `{}` enforces a floor the schema does not publish",
                argument.name,
                tool.name
            );
        }
    }

    let tool = crate::harness::mcp::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "verify_claim")
        .expect("`verify_claim` is a tool this server exposes");
    let with = |issue: serde_json::Value| {
        let mut arguments = serde_json::json!({
            "run_id": "claude-aaaa1111",
            "expect_state": "in-progress",
        });
        arguments["issue"] = issue;
        super::flags_for(tool, &arguments)
    };

    // The floor: an ordinary issue number still reaches the transport, or
    // the refusals below are about a tool nobody can call.
    let flags = with(serde_json::json!(12)).expect("an issue number was refused");
    assert!(
        flags.iter().any(|flag| flag == "12"),
        "the issue number no longer reaches the transport: {flags:?}"
    );

    for value in [-5, 0] {
        let refused = with(serde_json::json!(value))
            .expect_err(&format!("{value} was accepted as an issue number"));
        let said = format!("{refused:?}");
        assert!(
            said.contains("issue") && said.contains("counts from"),
            "the refusal does not say which argument or what its floor is: {said}"
        );
    }
}

/// An argument this tool does not take is refused, not dropped.
///
/// `flags_for` walks the **declared** arguments and looks each one up in
/// what the caller sent, so a key it never heard of was never examined. A
/// misspelled *required* argument is caught by its own absence; a
/// misspelled *optional* one was caught by nothing, and the call ran
/// without the thing the agent asked for and succeeded — `list_state`'s own
/// incident: an agent asking what is in `ready` on a busy project read two
/// hundred issues as the whole queue with `limit` written `limt`, and had
/// no way to ask for more.
#[test]
fn an_argument_a_tool_does_not_take_is_refused_rather_than_dropped() {
    let tool = crate::harness::mcp::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "list_state")
        .expect("`list_state` is a tool this server exposes");

    // The floor, both halves: the spelling it does take reaches the
    // transport, and leaving it out is still fine — this refuses a wrong
    // key, not an absent one.
    let asked = super::flags_for(
        tool,
        &serde_json::json!({"state": "ready", "run_id": "codex-a", "limit": 500}),
    )
    .expect("the declared spelling was refused");
    assert!(
        asked.iter().any(|flag| flag == "500"),
        "the limit no longer reaches the transport: {asked:?}"
    );
    super::flags_for(
        tool,
        &serde_json::json!({"state": "ready", "run_id": "codex-a"}),
    )
    .expect("an optional argument is still optional");

    let refused = super::flags_for(
        tool,
        &serde_json::json!({"state": "ready", "run_id": "codex-a", "limt": 500}),
    )
    .expect_err("a misspelled optional argument was dropped in silence");
    let said = format!("{refused}");
    assert!(
        said.contains("limt"),
        "the refusal does not name the key that was wrong: {said}"
    );
    assert!(
        said.contains("limit") && said.contains("state"),
        "the refusal does not say what the tool does take, so the next guess is blind: {said}"
    );

    assert!(
        matches!(refused, super::ToolFailure::Malformed(_)),
        "a wrong argument was reported as the tracker's answer: {refused:?}"
    );

    for tool in crate::harness::mcp::tools::TOOLS {
        let mut arguments = serde_json::Map::new();
        for argument in tool.arguments.iter().filter(|argument| argument.required) {
            arguments.insert(
                argument.name.to_owned(),
                match argument.json_type {
                    "integer" => serde_json::json!(1),
                    "boolean" => serde_json::json!(true),
                    _ => serde_json::json!(argument.choices.map_or("x", |choices| choices[0])),
                },
            );
        }
        let whole = serde_json::Value::Object(arguments.clone());
        if let Err(super::ToolFailure::Malformed(said)) = super::flags_for(tool, &whole) {
            assert!(
                !said.contains("does not take"),
                "{}: the required arguments alone read as unknown: {said}",
                tool.name
            );
        }
        arguments.insert("froobaz".to_owned(), serde_json::json!("x"));
        let refused = super::flags_for(tool, &serde_json::Value::Object(arguments))
            .expect_err(&format!("{} took an argument it does not have", tool.name));
        assert!(
            format!("{refused}").contains("froobaz"),
            "{}: the refusal does not name the unknown key",
            tool.name
        );
    }
}

#[test]
fn a_tool_whose_operation_answers_in_two_calls_says_how_to_finish_it() {
    let mut two_step: Vec<String> = Vec::new();
    let mut stack =
        vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/transport")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.extension().is_some_and(|kind| kind == "rs")
                || path.file_name().is_some_and(|name| name == "tests.rs")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let text = crate::source_text::code_of(&text);
            let shipped = text
                .find("#[cfg(test)]")
                .map_or(text.as_str(), |at| &text[..at]);
            let mut rest = shipped;
            while let Some(at) = rest.find("\"next\": \"repeat ") {
                rest = &rest[at + "\"next\": \"repeat ".len()..];
                let Some(end) = rest.find(' ') else { break };
                two_step.push(rest[..end].to_owned());
            }
        }
    }
    two_step.sort();
    two_step.dedup();
    // The floor: a walk that found none would assert nothing, and the whole
    // point is that there is more than one of them.
    assert!(
        two_step.len() >= 2,
        "only {two_step:?} were found to answer in two calls — the walk is broken"
    );

    for operation in &two_step {
        let tool = tools::TOOLS
            .iter()
            .find(|tool| tool.operation == operation)
            .unwrap_or_else(|| panic!("`{operation}` answers in two calls and no tool offers it"));
        assert!(
            tool.description.contains("target_operation"),
            "`{}` answers in two calls and its description never says how to make the second, \
             so a caller that reads it makes one and believes it: {}",
            tool.name,
            tool.description
        );
        assert!(
            tool.arguments
                .iter()
                .any(|argument| argument.name == "target_operation"),
            "`{}` is told to repeat naming the target and takes no such argument",
            tool.name
        );
    }
}

/// Every optional flag the binding documents is one a tool can send.
///
/// The crossings here run the other way from most of this file — a flag
/// the transport *requires* that no tool sends is a tool that fails every
/// call, and that direction was the one that had burned this crate. This
/// reverse gap once left `reclaim`'s documented `[--force --reason-file
/// <f>]` — the forced takeover, deliberately undeclared — with neither
/// argument on the tool, so an agent following the binding could not make
/// the call it describes; `unassign [--held-by-other]` and `verify_claim
/// [--allow-closed-by-pr <pr>]` were the same shape.
#[test]
fn every_flag_the_binding_documents_is_one_a_tool_can_send() {
    let binding = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("skill")
            .join("bindings")
            .join("github.md"),
    )
    .expect("the binding ships with the crate");

    let mut rows = 0;
    let mut missing: Vec<String> = Vec::new();
    for line in binding.lines().filter(|line| line.starts_with("| `")) {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once('`') else {
            continue;
        };
        let Some(at) = tail.find("SCRIPT ") else {
            continue;
        };
        let Some(tool) = TOOLS.iter().find(|tool| tool.contract_name == name) else {
            continue;
        };
        rows += 1;
        let sends: Vec<String> = tool
            .arguments
            .iter()
            .map(|argument| argument.as_flag())
            .collect();
        let command = tail[at..].split('|').next().unwrap_or_default();
        for flag in command
            .split_whitespace()
            .filter(|word| word.starts_with("--"))
        {
            let flag = flag.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
            if ["--runtime", "--operation-id", "--worktree"].contains(&flag) {
                continue;
            }
            if !sends.iter().any(|sent| sent == flag) {
                missing.push(format!("{name} cannot send {flag}"));
            }
        }
    }

    // The floor: the table was read. A parser that matched nothing agrees
    // with a complete set of tools and never fails.
    assert!(
        rows >= 10,
        "only {rows} operation rows were read out of the binding, so this compared almost nothing"
    );
    assert!(
        missing.is_empty(),
        "the binding documents these calls and the tool an agent has cannot make them: \
         {missing:#?}"
    );
}

/// The reissue's own conjunct, pinned directly rather than through the
/// two-process integration test that reaches it: `claim_key_reused` must be
/// true, not merely a `claim` that failed with `claim-operation-expired`,
/// or the guard is retrying a key that was minted fresh for this very
/// call.
///
/// `tests/pipe/part_09.rs`'s own two-process test cannot pin this: a key freshly
/// minted *this call* cannot already name a dead event on the timeline, so
/// no fixture built from real `gh` answers can drive `answered` to
/// `claim-operation-expired` with `claim_key_reused` false. Only a direct
/// call to the predicate can pose that combination at all.
#[test]
fn a_fresh_key_is_never_reissued_as_though_it_had_lost_a_race() {
    let expired = Err(crate::transport::Failure::Stop(serde_json::json!({
        "ok": false, "reason": "claim-operation-expired",
    })));
    assert!(
        should_reissue_claim("claim", true, &expired),
        "a claim answer with a reused key and claim-operation-expired must reissue"
    );
    assert!(
        !should_reissue_claim("claim", false, &expired),
        "a key minted fresh for this call must never be reissued, whatever the reason \
         the call it named came back with"
    );
    assert!(
        !should_reissue_claim("reclaim", true, &expired),
        "reclaim keeps its own stale-self-requires-claim refusal; it does not reissue"
    );
    let lost_race = Err(crate::transport::Failure::Stop(serde_json::json!({
        "ok": false, "reason": "lost-claim-race", "world": "committed",
    })));
    assert!(
        !should_reissue_claim("claim", true, &lost_race),
        "a genuine lost race must never be retried under a fresh key"
    );
    let ok = Ok(serde_json::json!({ "ok": true }));
    assert!(!should_reissue_claim("claim", true, &ok));
}

#[test]
fn the_start_branch_template_names_the_sibling_it_makes() {
    let start = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "start_branch")
        .expect("the tool is listed");
    let root = start
        .arguments
        .iter()
        .find(|argument| argument.name == "worktree_root")
        .expect("the template argument is listed");
    assert!(
        root.description.contains("sibling") && root.description.contains("<branch>~<run-id>"),
        "the template still reads as a container: {:?}",
        root.description
    );
    // The two clauses a reader acts on: that the suffix is *appended* to the
    // path given (so the directory is a prefix, not a container), and *why*
    // (so two checkouts do not share one directory). Deleting either leaves an
    // operator with a template that reads as a container.
    assert!(
        root.description.contains("appended to the path given"),
        "the template no longer says the suffix is appended to the path: {:?}",
        root.description
    );
    assert!(
        root.description
            .contains("keeping two checkouts from sharing one directory"),
        "the template no longer says why the suffix is appended: {:?}",
        root.description
    );
}

/// The refusal is for an identity whose **every** recording is
/// `severe`. The description said "a severe finding ... naming one is refused"
/// without that qualifier, so a reader could take the one batch that clears
/// `release_ci` for a mixed-class identity to be forbidden.
#[test]
fn the_record_disposition_description_qualifies_the_severe_refusal() {
    let tool = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "record_disposition")
        .expect("the tool is listed");
    assert!(
        tool.description.contains("every recording is `severe`"),
        "the description refuses a severe finding without the qualifier the rest of the \
         contract carries: {}",
        tool.description
    );
    assert!(
        tool.description.contains("non-severe recording"),
        "the description does not say a mixed-class identity is answered for its non-severe \
         recording: {}",
        tool.description
    );
}

std::thread_local! {
    static MCP_AT: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
}

fn mcp_clock() -> Option<u16> {
    MCP_AT.with(|cell| Some(cell.get()))
}

#[test]
fn start_branch_through_the_server_is_not_stopped_by_the_schedule_outside_the_period() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join("skill");
    std::fs::create_dir_all(&skill).expect("a skill root");
    std::fs::write(
        skill.join(crate::skill::CONTRACT),
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Git operation window | enabled 09:00-17:00 | disabled |\n",
    )
    .expect("a contract");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: skill,
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
    };
    std::fs::create_dir_all(&context.state_root).expect("a state directory");
    std::fs::create_dir_all(&context.repo_dir).expect("a checkout");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view",
            "stdout": serde_json::json!({"owner": {"login": "acme"}, "name": "app"}).to_string(),
            "status": 0,
        }),
        crate::test_env::live_holder_issue("claude-abcd1234", 12, "in-progress"),
    ]);
    MCP_AT.with(|cell| cell.set(20 * 60));
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        crate::harness::operation_window::with_now_override(Some(mcp_clock), || {
            super::run_tool(
                "start_branch",
                &json!({
                    "issue": 12,
                    "run_id": "claude-abcd1234",
                    "branch": "fix/12",
                    "base": "main",
                }),
                Ok(&context),
            )
        })
    })
    .expect_err("a start without a worktree location passed through the server");
    match failure {
        super::ToolFailure::Refused(refusal) => assert!(
            !refusal.to_string().contains("git-operation-window-denied"),
            "the server stopped a start on the schedule: {refusal}"
        ),
        super::ToolFailure::Malformed(message) => {
            panic!("the call was malformed rather than denied: {message}")
        }
    }
}

/// `verify_claim`'s `allow_closed_by_pr` reaches the transport rather than
/// stopping at the schema: the successor's renewal passes with the pin the
/// closing pull request carries and refuses with another, where a server that
/// dropped the argument would answer both the same way.
#[test]
fn verify_claim_carries_the_closing_pin_from_the_arguments_to_the_transport() {
    use crate::transport::store::{Repo, Store};

    let root = tempfile::tempdir().expect("a temporary root");
    let repo_dir = root.path().join("repo");
    std::fs::create_dir_all(&repo_dir).expect("a checkout");
    let git = |args: &[&str]| -> String {
        let output = std::process::Command::new("git")
            .current_dir(&repo_dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).expect("utf-8")
    };
    git(&["init", "--quiet", "-b", "main"]);
    git(&["config", "user.email", "estigia@example.invalid"]);
    git(&["config", "user.name", "Estigia test"]);
    std::fs::write(repo_dir.join("one.txt"), "one\n").expect("content");
    git(&["add", "one.txt"]);
    git(&["commit", "--quiet", "-m", "base"]);
    let base = git(&["rev-parse", "HEAD"]).trim().to_owned();
    let head = "b".repeat(40);
    let merge_sha = "e".repeat(40);
    let epoch = "a".repeat(32);
    let digest = "d".repeat(64);
    let op_id = "c".repeat(32);

    let comment = |id: &str, at: &str, body: String| {
        json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let claim = comment(
        "IC_claim",
        "2026-07-28T01:00:00Z",
        format!(
            "Claimed by claude-abcd1234.\n\n{}\n",
            crate::transport::markers::render(
                "claim",
                &[
                    ("run-id", "claude-abcd1234"),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &op_id),
                ],
            )
            .expect("a claim marker"),
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-07-28T01:30:00Z",
        format!(
            "published.\n\n{}\n",
            crate::transport::markers::render(
                "published",
                &[
                    ("run-id", "claude-b"),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            )
            .expect("a published marker"),
        ),
    );

    let repo = Repo {
        host: "github.com".to_owned(),
        owner: "fixture".to_owned(),
        name: "repo".to_owned(),
    };
    let store_path = root.path().join("state.db");
    let mut store = Store::open(&store_path).expect("the scratch store opens");
    store
        .import(
            &repo,
            7,
            &json!({
                "body": "Delivered.",
                "labels": [{ "name": "status:review" }],
                "comments": [claim, publication],
            }),
            "2026-07-28T01:30Z",
        )
        .expect("issue 7 imports");
    drop(store);

    let closing = json!({
        "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
            "nodes": [{ "number": 7, "state": "MERGED", "headRefName": "h", "baseRefName": "b" }],
            "pageInfo": { "hasNextPage": false, "endCursor": null },
        } } } },
    });
    let bin = crate::test_env::scripted_gh();
    let answers = crate::test_env::answers(&[
        json!({
            "matches": "repo view",
            "stdout": json!({ "owner": { "login": "fixture" }, "name": "repo" }).to_string(),
        }),
        json!({ "matches": "api graphql", "stdout": closing.to_string() }),
        json!({
            "matches": "pr view 7 --json mergeCommit",
            "stdout": json!({ "mergeCommit": { "oid": merge_sha } }).to_string(),
        }),
        crate::test_env::issue_view_answer(
            7,
            json!({ "state": "CLOSED", "labels": [], "comments": [] }),
        ),
        json!({
            "matches": "/commits/",
            "stdout": json!({ "parents": [{ "sha": base }, { "sha": head }] }).to_string(),
        }),
    ]);
    let context = bare_context(&root, &repo_dir);
    let call = |pin: u64| {
        super::run_tool(
            "verify_claim",
            &json!({
                "issue": 7,
                "run_id": "claude-abcd1234",
                "expect_state": "review",
                "allow_closed_by_pr": pin,
            }),
            Ok(&context),
        )
    };
    let (granted, refused) = crate::test_env::with_scripted_gh(bin.path(), &answers, || {
        crate::transport::store::with_store_at(&store_path, || (call(7), call(99)))
    });

    let granted = granted.expect("the successor's renewal passed through the server");
    let text = granted["content"][0]["text"]
        .as_str()
        .expect("a tool result carries text");
    assert!(
        text.contains(crate::transport::claim::OWN_DELIVERY_CLOSE_PREFIX),
        "the renewal did not report the auto-close it was renewing past: {text}"
    );

    let super::ToolFailure::Refused(refusal) =
        refused.expect_err("a pin naming another pull request refuses")
    else {
        panic!("the wrong-pin call was refused as something other than a refusal");
    };
    assert_eq!(refusal.code, "issue-not-open", "{refusal:?}");
}

const READ_TOOLS: &[&str] = &[
    "list_sessions",
    "tasks",
    "verify_claim",
    "review_status",
    "review_read",
    "list_state",
    "check_closing_keywords",
    "review_tier",
    "target",
];

const LOCAL_WRITER_TOOLS: &[&str] = &["judge_workspaces", "blocked_until", "task", "send_message"];

#[test]
fn every_tool_is_a_tracker_writer_an_assisted_only_a_local_writer_or_a_read_and_never_two() {
    for tool in TOOLS {
        let classes = [
            (
                "tracker writer",
                tools::TRACKER_WRITERS.contains(&tool.name),
            ),
            ("assisted only", tools::ASSISTED_ONLY.contains(&tool.name)),
            ("local writer", LOCAL_WRITER_TOOLS.contains(&tool.name)),
            ("read", READ_TOOLS.contains(&tool.name)),
        ];
        let held: Vec<&str> = classes
            .iter()
            .filter(|(_, holds)| *holds)
            .map(|(class, _)| *class)
            .collect();
        assert_eq!(
            held.len(),
            1,
            "{} is classed as {held:?}; every tool is exactly one of a tracker writer, an assisted-only tool, a local \
             writer or a read",
            tool.name
        );
    }
    for name in tools::TRACKER_WRITERS
        .iter()
        .chain(tools::ASSISTED_ONLY)
        .chain(LOCAL_WRITER_TOOLS)
        .chain(READ_TOOLS)
    {
        assert!(
            tools::find(name).is_some(),
            "{name} is classified and is not a tool"
        );
    }
}

fn assisted_context(root: &tempfile::TempDir) -> GateContext {
    let mut context = bare_context(root, &root.path().join("repo"));
    context.workflow = Some(crate::config::Workflow::Assisted);
    context
}

fn arguments_naming(tool: &tools::Tool, run_id: &str) -> Value {
    let mut arguments = serde_json::Map::new();
    for argument in tool.arguments.iter().filter(|a| a.required) {
        arguments.insert(
            argument.name.to_owned(),
            match argument.json_type {
                "integer" => json!(12),
                "boolean" => json!(true),
                _ => argument
                    .choices
                    .map_or_else(|| json!("x"), |choices| json!(choices[0])),
            },
        );
    }
    if tool.arguments.iter().any(|a| a.name == "run_id") {
        arguments.insert("run_id".to_owned(), json!(run_id));
    }
    Value::Object(arguments)
}

#[test]
fn an_assisted_workflow_refuses_every_tool_that_writes_the_tracker() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let nameless = format!(
        "{}-{}",
        crate::harness::session::DEFAULT_RUNTIME,
        crate::harness::session::NAMELESS
    );
    for tool in TOOLS {
        let writes = tools::TRACKER_WRITERS.contains(&tool.name);
        let takes_a_run = tool.arguments.iter().any(|a| a.name == "run_id");
        if !writes && !takes_a_run {
            continue;
        }
        let ToolFailure::Refused(refusal) =
            run_tool(tool.name, &arguments_naming(tool, &nameless), Ok(&context))
                .expect_err("neither a nameless run nor an assisted workflow runs a tool")
        else {
            panic!(
                "{}: refused as malformed rather than by the rule",
                tool.name
            );
        };
        let expected = if writes {
            "workflow-assisted-writes-no-tracker"
        } else {
            "run-id-names-no-run"
        };
        assert_eq!(
            refusal.code, expected,
            "{} was refused for something else: {refusal}",
            tool.name
        );
    }
}

#[test]
fn the_assisted_refusal_names_the_row_the_value_and_the_way_out() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let ToolFailure::Refused(refusal) = run_tool("ensure_states", &json!({}), Ok(&context))
        .expect_err("an assisted workflow writes no tracker")
    else {
        panic!("refused as malformed rather than by the rule");
    };
    let said = refusal.to_string();
    for part in [
        "Workflow",
        "assisted",
        "estigia config set Workflow autonomous --repo",
    ] {
        assert!(
            said.contains(part),
            "the refusal does not say {part}: {said}"
        );
    }
}

#[test]
fn an_autonomous_workflow_never_refuses_for_being_assisted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = bare_context(&root, &root.path().join("repo"));
    let nameless = format!(
        "{}-{}",
        crate::harness::session::DEFAULT_RUNTIME,
        crate::harness::session::NAMELESS
    );
    for tool in TOOLS
        .iter()
        .filter(|tool| tool.arguments.iter().any(|a| a.name == "run_id"))
    {
        let ToolFailure::Refused(refusal) =
            run_tool(tool.name, &arguments_naming(tool, &nameless), Ok(&context))
                .expect_err("a nameless run is not one to act under")
        else {
            panic!(
                "{}: refused as malformed rather than by the rule",
                tool.name
            );
        };
        assert_eq!(refusal.code, "run-id-names-no-run", "{}", tool.name);
    }
}

fn assist_arguments(issue: u64) -> Value {
    json!({
        "issue": issue,
        "run_id": "claude-abcd1234",
        "branch": "issue-12",
        "base": "main",
    })
}

#[test]
fn assist_refuses_an_issue_whose_shape_does_not_match_the_tracker() {
    let root = tempfile::tempdir().expect("a temporary root");
    // A key under a GitHub tracker.
    let github = assisted_context(&root);
    let ToolFailure::Refused(refusal) = run_tool(
        "assist",
        &json!({
            "issue": "ENG-123",
            "run_id": "claude-abcd1234",
            "branch": "issue-ENG-123",
            "base": "main",
        }),
        Ok(&github),
    )
    .expect_err("a key under Tracker github") else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "assist-issue-does-not-match-the-tracker");
    assert!(refusal.to_string().contains("ENG-123") || refusal.to_string().contains("Linear key"));

    // A number under a Linear tracker.
    let mut linear = assisted_context(&root);
    linear.tracker = crate::config::Tracker::Linear {
        team: "ENG".to_owned(),
    };
    let ToolFailure::Refused(refusal) = run_tool("assist", &assist_arguments(12), Ok(&linear))
        .expect_err("a number under Tracker linear")
    else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "assist-issue-does-not-match-the-tracker");
}

#[test]
fn an_assisted_only_tool_is_refused_unless_the_row_says_assisted() {
    for (workflow, says) in [
        (
            Some(crate::config::Workflow::Autonomous),
            "work here goes through a claim",
        ),
        (None, "could not be read"),
    ] {
        let root = tempfile::tempdir().expect("a temporary root");
        let mut context = bare_context(&root, &root.path().join("repo"));
        context.workflow = workflow;
        let ToolFailure::Refused(refusal) = run_tool("assist", &assist_arguments(12), Ok(&context))
            .expect_err("assist ran outside an assisted repository")
        else {
            panic!("refused as malformed rather than by the rule");
        };
        assert_eq!(refusal.code, "workflow-not-assisted");
        assert!(refusal.to_string().contains(says), "{refusal}");
    }
}

#[test]
fn assist_from_a_run_holding_another_issue_is_refused_without_naming_release() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(99));
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");
    let ToolFailure::Refused(refusal) = run_tool("assist", &assist_arguments(12), Ok(&context))
        .expect_err("a run holding #99 took #12")
    else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "already-holding");
    assert!(
        !refusal.to_string().contains("estigia release"),
        "the way out is a command assisted refuses: {refusal}"
    );
}

#[test]
fn the_assisted_effect_records_the_local_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let assist = tools::find("assist").expect("the tool is listed");
    let worktree = root.path().join("trees").join("issue-12");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.mark_verified();
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");
    super::apply_effect(
        assist,
        &assist_arguments(12),
        Some(&json!({ "ok": true, "worktree": worktree.display().to_string() })),
        &mut run,
        &context,
    );
    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert!(after.assisted);
    assert_eq!(after.issue, Some(crate::identity::Identity::Number(12)));
    assert_eq!(after.branch.as_deref(), Some("issue-12"));
    assert_eq!(after.worktree.as_ref(), Some(&worktree));
    assert_eq!(after.repo_dir.as_ref(), Some(&context.repo_dir));
    assert_eq!(after.lands_in(), vec![&worktree]);
    assert_eq!(after.verified_at, None);
}

#[test]
fn assist_after_a_verified_stamp_leaves_no_renewal_window_for_a_write() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let assist = tools::find("assist").expect("the tool is listed");
    let worktree = root.path().join("trees").join("issue-12");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.mark_verified();
    assert!(run.within_window(context.window));
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");
    std::fs::create_dir_all(&context.skill_root).expect("the skill root");
    std::fs::write(context.skill_root.join(crate::skill::CONTRACT), "contract")
        .expect("the contract");
    super::apply_effect(
        assist,
        &assist_arguments(12),
        Some(&json!({ "ok": true, "worktree": worktree.display().to_string() })),
        &mut run,
        &context,
    );
    std::fs::create_dir_all(&worktree).expect("the worktree");
    let mut context = context;
    context.repo_dir = worktree.clone();
    let mut after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    let action = crate::harness::Action::Write {
        target: worktree.join("x.rs").display().to_string(),
    };
    let bin = crate::test_env::scripted_gh();
    let decision = crate::test_env::with_scripted_gh(bin.path(), "[]", || {
        crate::harness::gate(
            &context,
            &mut after,
            &action,
            crate::harness::Sensitivity::Routine,
        )
    });
    let crate::harness::Decision::Deny(refusal) = decision else {
        panic!("a routine write rode an earlier stamp: {decision:?}");
    };
    assert_eq!(refusal.code, "assisted-claim-stale");
}

#[test]
fn no_effect_but_the_assisted_decision_leaves_a_stamp_on_an_assisted_pointer() {
    for tool in TOOLS.iter().filter(|tool| {
        !matches!(
            tool.effect,
            PointerEffect::None | PointerEffect::Forget | PointerEffect::Swear
        )
    }) {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = assisted_context(&root);
        let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.assisted = true;
        run.branch = Some("issue-12".to_owned());
        crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");
        super::apply_effect(
            tool,
            &arguments_naming(tool, "claude-abcd1234"),
            Some(&json!({ "ok": true, "worktree": root.path().join("t").display().to_string() })),
            &mut run,
            &context,
        );
        let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
        assert_eq!(
            after.verified_at, None,
            "{} stamped an assisted pointer",
            tool.name
        );
    }
}

#[test]
fn a_later_swear_makes_the_pointer_a_tracker_claim_again() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = bare_context(&root, &root.path().join("repo"));
    let claim = tools::find("claim").expect("the tool is listed");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.assisted = true;
    run.branch = Some("issue-12".to_owned());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");
    super::apply_effect(
        claim,
        &arguments_naming(claim, "claude-abcd1234"),
        Some(&json!({ "ok": true })),
        &mut run,
        &context,
    );
    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert!(!after.assisted && after.branch.is_none(), "{after:?}");
    assert_eq!(after.lands_in().len(), after.covered().count());
}

#[test]
fn a_pointer_from_before_this_build_never_reads_as_assisted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let older = json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "state": "in-progress",
        "repo_dir": root.path(),
        "verified_at": 1,
    });
    let run: crate::harness::session::Run =
        serde_json::from_value(older).expect("an older pointer still loads");
    assert!(!run.assisted && run.branch.is_none());
}

#[test]
fn an_unreadable_contract_does_not_refuse_the_tracker_writers_for_being_assisted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = bare_context(&root, &root.path().join("repo"));
    context.workflow = None;
    let nameless = format!(
        "{}-{}",
        crate::harness::session::DEFAULT_RUNTIME,
        crate::harness::session::NAMELESS
    );
    for tool in TOOLS
        .iter()
        .filter(|tool| tools::TRACKER_WRITERS.contains(&tool.name))
        .filter(|tool| tool.arguments.iter().any(|a| a.name == "run_id"))
    {
        let ToolFailure::Refused(refusal) =
            run_tool(tool.name, &arguments_naming(tool, &nameless), Ok(&context))
                .expect_err("a nameless run is not one to act under")
        else {
            panic!("{}: refused as malformed", tool.name);
        };
        assert_eq!(refusal.code, "run-id-names-no-run", "{}", tool.name);
    }
}

fn draft_pr_arguments(body_file: &std::path::Path) -> Value {
    json!({
        "run_id": "claude-abcd1234",
        "title": "A title",
        "body_file": body_file.display().to_string(),
    })
}

#[test]
fn draft_pr_is_refused_unless_the_row_says_assisted() {
    for workflow in [Some(crate::config::Workflow::Autonomous), None] {
        let root = tempfile::tempdir().expect("a temporary root");
        let mut context = bare_context(&root, &root.path().join("repo"));
        context.workflow = workflow;
        let ToolFailure::Refused(refusal) = run_tool(
            "draft_pr",
            &draft_pr_arguments(&root.path().join("body.md")),
            Ok(&context),
        )
        .expect_err("draft_pr ran outside an assisted repository") else {
            panic!("refused as malformed rather than by the rule");
        };
        assert_eq!(refusal.code, "workflow-not-assisted");
    }
}

#[test]
fn draft_pr_is_refused_for_a_run_with_no_local_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.branch = Some("issue-12".to_owned());
    run.worktree = Some(root.path().join("trees").join("issue-12"));
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");
    let ToolFailure::Refused(refusal) = run_tool(
        "draft_pr",
        &draft_pr_arguments(&root.path().join("body.md")),
        Ok(&context),
    )
    .expect_err("a pointer that is not assisted published a branch") else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "draft-pr-run-not-assisted");
}

#[test]
fn draft_pr_publishes_the_branch_and_checkout_the_assisted_pointer_names() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = repository_with_commit();
    let origin = root.path().join("origin.git");
    let git = |at: &std::path::Path, arguments: &[&str]| {
        crate::transport::run(
            &["git"]
                .into_iter()
                .chain(arguments.iter().copied())
                .collect::<Vec<_>>(),
            Some(at),
            crate::transport::How::write(),
        )
        .unwrap_or_else(|failure| panic!("git {arguments:?}: {}", failure.detail()))
        .stdout
        .trim()
        .to_owned()
    };
    std::fs::create_dir_all(&origin).expect("a remote directory");
    git(&origin, &["init", "--quiet", "--bare"]);
    git(repo.path(), &["branch", "-M", "main"]);
    git(
        repo.path(),
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    git(repo.path(), &["push", "--quiet", "origin", "main"]);
    git(repo.path(), &["checkout", "--quiet", "-b", "issue-12"]);
    git(
        repo.path(),
        &["commit", "--allow-empty", "--quiet", "-m", "work"],
    );
    let head = git(repo.path(), &["rev-parse", "HEAD"]);

    let mut context = assisted_context(&root);
    context.repo_dir = repo.path().to_path_buf();
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.assisted = true;
    run.branch = Some("issue-12".to_owned());
    run.worktree = Some(repo.path().to_path_buf());
    run.repo_dir = Some(repo.path().to_path_buf());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    let body = root.path().join("body.md");
    std::fs::write(&body, "Words.").expect("the body file");
    let url = "https://github.com/example/repo/pull/7";
    let viewed = json!({
        "number": 7, "url": url, "headRefOid": head, "headRefName": "issue-12",
        "state": "OPEN", "isDraft": true,
    });
    let answers = crate::test_env::answers(&[
        json!({ "matches": "repo view", "stdout": r#"{"defaultBranchRef":{"name":"main"}}"# }),
        json!({ "matches": "pr list", "stdout": "[]" }),
        json!({ "matches": "pr create", "stdout": url }),
        json!({ "matches": "pr view", "stdout": viewed.to_string() }),
    ]);
    let bin = crate::test_env::scripted_gh();
    let answer = crate::test_env::with_scripted_gh(bin.path(), &answers, || {
        run_tool("draft_pr", &draft_pr_arguments(&body), Ok(&context))
    })
    .unwrap_or_else(|failure| panic!("the draft was not published: {failure:?}"));
    let text = answer.to_string();
    assert!(text.contains(url) && text.contains("issue-12"), "{text}");
    assert_eq!(
        git(repo.path(), &["ls-remote", "--heads", "origin", "issue-12"])
            .split_whitespace()
            .next(),
        Some(head.as_str())
    );
    let after = crate::harness::session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(after.issue, Some(crate::identity::Identity::Number(12)));
    assert_eq!(after.verified_at, None, "the pointer moved");
}

#[test]
fn assist_refuses_an_issue_of_the_wrong_json_kind() {
    let root = tempfile::tempdir().expect("a temporary root");
    let github = assisted_context(&root);
    for issue in [json!(true), json!(1.5)] {
        let ToolFailure::Refused(refusal) = run_tool(
            "assist",
            &json!({
                "issue": issue,
                "run_id": "claude-abcd1234",
                "branch": "issue-12",
                "base": "main",
            }),
            Ok(&github),
        )
        .expect_err("an issue that is not an identity") else {
            panic!("refused as malformed rather than by the rule");
        };
        assert_eq!(refusal.code, "assist-issue-does-not-match-the-tracker");
    }
}

fn transition_pointer_fixture() -> (tempfile::TempDir, GateContext, session::Run) {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    run.state = Some("in-progress".to_owned());
    run.verified_at = Some(1);
    session::store(&context.state_root, &run).expect("the pointer writes");
    (root, context, run)
}

fn apply_transition(issue: u64, run: &mut session::Run, context: &GateContext) -> session::Run {
    let transition = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "transition")
        .expect("the tool is listed");
    super::apply_effect(
        transition,
        &json!({"issue": issue, "to": "ready", "run_id": "claude-abcd1234"}),
        Some(&json!({"ok": true, "issue": issue, "to": "ready"})),
        run,
        context,
    );
    session::load(&context.state_root, "claude-abcd1234")
}

#[test]
fn transitioning_another_issue_leaves_the_held_issues_state_alone() {
    let (_root, context, mut run) = transition_pointer_fixture();
    let after = apply_transition(9, &mut run, &context);
    assert_eq!(after.issue, Some(crate::identity::Identity::Number(7)));
    assert_eq!(
        after.state.as_deref(),
        Some("in-progress"),
        "moving issue #9 rewrote the state the run expects its held issue #7 to be in"
    );
}

#[test]
fn transitioning_another_issue_does_not_renew_the_held_issue() {
    let (_root, context, mut run) = transition_pointer_fixture();
    let after = apply_transition(9, &mut run, &context);
    assert_eq!(
        after.verified_at,
        Some(1),
        "a write to issue #9 restarted the renewal window of held issue #7"
    );
}

#[test]
fn transitioning_the_held_issue_updates_its_state_and_renews_it() {
    let (_root, context, mut run) = transition_pointer_fixture();
    let after = apply_transition(7, &mut run, &context);
    assert_eq!(after.state.as_deref(), Some("ready"));
    assert_ne!(after.verified_at, Some(1));
}

#[test]
fn a_description_never_sends_the_caller_to_a_tool_this_server_does_not_expose() {
    const PREFIXES: [&str; 3] = ["review_", "record_", "spend_"];
    const DATA: [&str; 3] = [
        "review_findings",
        "review_finding",
        "review_reads_unavailable",
    ];
    let exposed: Vec<&str> = TOOLS.iter().map(|tool| tool.name).collect();
    let mut stale = Vec::new();
    for tool in TOOLS {
        let texts = std::iter::once(tool.description)
            .chain(tool.arguments.iter().map(|argument| argument.description));
        for text in texts {
            for word in text.split(|c: char| !(c.is_ascii_lowercase() || c == '_')) {
                if PREFIXES.iter().any(|prefix| word.starts_with(prefix))
                    && !exposed.contains(&word)
                    && !DATA.contains(&word)
                {
                    stale.push(format!("{}: {word}", tool.name));
                }
            }
        }
    }
    assert!(stale.is_empty(), "names no tool exposes: {stale:?}");
}

const TOOLS_LIST_CEILING: usize = 46_996;

#[test]
fn the_tool_menu_stays_under_its_registered_ceiling() {
    let listed = result(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let bytes = serde_json::to_string(&listed).expect("serialisable").len();
    assert!(
        bytes <= TOOLS_LIST_CEILING,
        "the `tools/list` result is {bytes} bytes against a ceiling of {TOOLS_LIST_CEILING}: every \
         session pays it, so trim a description or raise `TOOLS_LIST_CEILING` in a reviewed edit"
    );
}

#[test]
fn no_description_carries_history_or_a_measurement() {
    let listed = result(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let text = serde_json::to_string(&listed).expect("serialisable");
    for phrase in ["moved to", "measured here", "payload bytes were"] {
        assert!(
            !text.contains(phrase),
            "a tool description carries `{phrase}`, which belongs in docs/"
        );
    }
}

fn listed_names(context: Result<&GateContext, &Refusal>) -> Vec<String> {
    let response = handle_line(
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
        context,
    )
    .expect("a request with an id gets a response");
    response["result"]["tools"]
        .as_array()
        .expect("a tool list")
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect()
}

#[test]
fn an_assisted_menu_omits_every_tool_that_writes_the_tracker() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = assisted_context(&root);
    let names = listed_names(Ok(&context));
    for name in tools::TRACKER_WRITERS {
        assert!(
            !names.iter().any(|listed| listed == name),
            "{name} is refused in assisted work and was advertised"
        );
    }
    for name in tools::ASSISTED_ONLY {
        assert!(
            names.iter().any(|listed| listed == name),
            "{name} is the assisted work and was not advertised"
        );
    }
}

#[test]
fn an_autonomous_menu_omits_the_assisted_only_tools_and_keeps_the_rest() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = bare_context(&root, &root.path().join("repo"));
    let names = listed_names(Ok(&context));
    for name in tools::ASSISTED_ONLY {
        assert!(
            !names.iter().any(|listed| listed == name),
            "{name} is refused outside assisted work and was advertised"
        );
    }
    assert_eq!(names.len(), TOOLS.len() - tools::ASSISTED_ONLY.len());
}

#[test]
fn an_unreadable_context_or_an_unknown_workflow_lists_every_tool() {
    let refusal = crate::skill::no_skill_root();
    assert_eq!(listed_names(Err(&refusal)).len(), TOOLS.len());
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = bare_context(&root, &root.path().join("repo"));
    context.workflow = None;
    assert_eq!(listed_names(Ok(&context)).len(), TOOLS.len());
}

#[test]
fn a_tool_the_menu_hides_still_answers_its_typed_refusal_by_name() {
    let root = tempfile::tempdir().expect("a temporary root");
    let assisted = assisted_context(&root);
    assert!(!listed_names(Ok(&assisted)).contains(&"ensure_states".to_owned()));
    let ToolFailure::Refused(refusal) = run_tool("ensure_states", &json!({}), Ok(&assisted))
        .expect_err("an assisted workflow writes no tracker")
    else {
        panic!("refused as malformed rather than by the rule");
    };
    assert_eq!(refusal.code, "workflow-assisted-writes-no-tracker");
}

#[test]
fn a_receipt_run_or_issue_argument_has_one_description_everywhere() {
    let receipt = ["epoch", "pr", "head", "base", "digest"];
    for name in receipt.iter().chain(&["run_id", "issue"]) {
        let mut descriptions: Vec<&str> = TOOLS
            .iter()
            .filter(|tool| {
                *name == "run_id"
                    || *name == "issue"
                    || receipt
                        .iter()
                        .all(|part| tool.arguments.iter().any(|a| a.name == *part && a.required))
            })
            .flat_map(|tool| tool.arguments.iter())
            .filter(|argument| {
                argument.required
                    && argument.name == *name
                    && (*name != "issue" || argument.json_type == "integer")
            })
            .map(|argument| argument.description)
            .collect();
        descriptions.sort_unstable();
        descriptions.dedup();
        assert_eq!(
            descriptions.len(),
            1,
            "`{name}` is described {} ways: {descriptions:?}",
            descriptions.len()
        );
    }
}

#[test]
fn the_review_tools_do_not_list_the_refusals_each_call_already_carries() {
    for name in ["collect_review_input", "launch_review_input"] {
        let tool = tools::find(name).expect("a declared tool");
        for code in ["capture-", "reviewer-"] {
            assert!(
                !tool.description.contains(code),
                "`{name}` lists a refusal code (`{code}…`) that the refusal itself names"
            );
        }
    }
}

#[test]
fn a_finding_cannot_name_a_lens_through_the_tool() {
    let tool = tools::find("record_review_finding").expect("a declared tool");
    assert!(
        !tool
            .arguments
            .iter()
            .any(|argument| argument.name == "lens"),
        "`lens` exists only to be refused and is advertised"
    );
    let ToolFailure::Malformed(message) = super::flags_for(tool, &json!({"lens": "review-risk"}))
        .expect_err("an unadvertised argument is refused")
    else {
        panic!("refused as a refusal rather than as an argument the tool does not take");
    };
    assert!(message.contains("`lens`"), "{message}");
}

#[test]
fn done_and_release_name_a_dirty_claim_checkout_and_other_calls_do_not() {
    let repo = repository_with_commit();
    std::fs::write(repo.path().join("parked.txt"), "x\n").unwrap();
    let done = json!({"issue": 4, "to": "done"});
    for (operation, arguments) in [("transition", &done), ("unassign", &json!({"issue": 4}))] {
        let notice = super::checkout_notice(operation, arguments, Some(repo.path()))
            .unwrap_or_else(|| panic!("{operation} said nothing about a dirty checkout"));
        assert!(
            notice.contains("parked.txt") && notice.contains("estigia changes"),
            "{notice}"
        );
    }
    let review = json!({"issue": 4, "to": "review"});
    assert_eq!(
        super::checkout_notice("transition", &review, Some(repo.path())),
        None
    );
    assert_eq!(
        super::checkout_notice("claim", &done, Some(repo.path())),
        None
    );
    assert_eq!(super::checkout_notice("unassign", &done, None), None);
}

mod send_message {
    use super::*;
    use crate::transport::store::{self, Repo, SessionSeen, Store};

    const SENDER: &str = "claude-send0a1b2c3d";
    const RECIPIENT: &str = "codex-recv0a1b2c3d";
    const HOLDER_NOW: &str = "2026-01-01T00:10Z";

    struct Rig {
        root: tempfile::TempDir,
    }

    fn repo() -> Repo {
        Repo {
            host: "github.com".to_owned(),
            owner: "asanabrial".to_owned(),
            name: "estigia".to_owned(),
        }
    }

    impl Rig {
        fn new() -> Self {
            let rig = Self {
                root: tempfile::tempdir().expect("a temporary root"),
            };
            let store = rig.store();
            for (run_id, runtime) in [(SENDER, "claude"), (RECIPIENT, "codex")] {
                store
                    .session_seen(
                        &SessionSeen {
                            run_id,
                            runtime,
                            session_id: "host-session",
                            repo_dir: None,
                            wake: None,
                        },
                        1_000,
                    )
                    .expect("a session registers");
            }
            rig
        }

        fn context(&self) -> GateContext {
            let mut context = bare_context(&self.root, &self.root.path().join("repo"));
            context.tracker = crate::config::Tracker::Github {
                repo: Some(crate::config::RepoRef::parse("asanabrial/estigia").expect("a repo")),
            };
            context
        }

        fn store(&self) -> Store {
            Store::open(&store::beside(&self.context().state_root)).expect("the store opens")
        }

        fn hold(&self, issue: u64, run_id: &str) {
            let github = json!({
                "body": "Body.",
                "labels": [{ "name": "status:in-progress" }],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!(
                        "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=codex \
                         horizon=2099-01-01T00:00Z op-id={} -->\n",
                        "0123456789abcdef0123456789abcdef"
                    ),
                }],
            });
            self.store()
                .import(&repo(), issue, &github, HOLDER_NOW)
                .expect("a claim imports");
            self.pointer(run_id, issue);
            let runtime = self
                .store()
                .sessions()
                .expect("the registry reads")
                .into_iter()
                .find(|row| row.run_id == run_id)
                .map(|row| row.runtime)
                .unwrap_or_else(|| "claude".to_owned());
            self.seen(
                run_id,
                &runtime,
                "host-session",
                None,
                crate::harness::session::now_seconds().expect("a clock"),
            );
        }

        fn rows_to(&self, run_id: &str) -> u64 {
            self.store()
                .messages_waiting_for()
                .expect("the mailbox reads")
                .get(run_id)
                .copied()
                .unwrap_or_default()
        }

        fn rows_from(&self, run_id: &str) -> u64 {
            self.store()
                .messages_undelivered_from()
                .expect("the mailbox reads")
                .get(run_id)
                .copied()
                .unwrap_or_default()
        }

        fn seen(
            &self,
            run_id: &str,
            runtime: &str,
            session_id: &str,
            repo_dir: Option<&str>,
            now: u64,
        ) {
            self.store()
                .session_seen(
                    &SessionSeen {
                        run_id,
                        runtime,
                        session_id,
                        repo_dir,
                        wake: None,
                    },
                    now,
                )
                .expect("a session registers");
        }

        fn ended(&self, run_id: &str, now: u64) {
            self.store()
                .session_ended(run_id, None, now)
                .expect("a session ends");
        }

        fn pointer(&self, run_id: &str, issue: u64) {
            let mut run = crate::harness::session::Run::new(run_id.to_owned());
            run.issue = Some(crate::identity::Identity::Number(issue));
            crate::harness::session::store(&self.context().state_root, &run).expect("a pointer");
        }

        fn list(&self, arguments: Value) -> Result<Value, ToolFailure> {
            run_tool("list_sessions", &arguments, Ok(&self.context()))
        }

        fn serve(&self, lines: &[Value]) -> Vec<Value> {
            let input = lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            let mut output = Vec::new();
            serve(std::io::Cursor::new(input), &mut output, Ok(self.context()))
                .expect("the stream is served");
            String::from_utf8(output)
                .expect("utf-8")
                .lines()
                .map(|line| serde_json::from_str(line).expect("one JSON message per line"))
                .collect()
        }
    }

    fn initialize(capabilities: Value) -> Value {
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-11-25", "capabilities": capabilities },
        })
    }

    fn asks() -> Value {
        json!({ "elicitation": { "form": {}, "url": {} } })
    }

    fn send(id: u64, to: &str, text: &str) -> Value {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "send_message",
                "arguments": { "run_id": SENDER, "to": to, "text": text } },
        })
    }

    fn answer_to(id: u64, from: &str, to: i64, text: &str) -> Value {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "send_message",
                "arguments": { "run_id": from, "reply_to": to, "text": text } },
        })
    }

    fn send_from(from: &str, id: u64, to: &str, text: &str) -> Value {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "send_message",
                "arguments": { "run_id": from, "to": to, "text": text } },
        })
    }

    fn answer(number: u64, result: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": format!("estigia-elicit-{number}"), "result": result })
    }

    fn choose(number: u64, choice: &str) -> Value {
        answer(
            number,
            json!({ "action": "accept", "content": { "choice": choice } }),
        )
    }

    fn questions(output: &[Value]) -> Vec<&Value> {
        output
            .iter()
            .filter(|message| message["method"] == "elicitation/create")
            .collect()
    }

    fn reply(output: &[Value], id: u64) -> &Value {
        output
            .iter()
            .find(|message| message["id"] == id && message.get("method").is_none())
            .unwrap_or_else(|| panic!("no answer to {id} in {output:?}"))
    }

    fn text_of(reply: &Value) -> String {
        reply["result"]["content"][0]["text"]
            .as_str()
            .expect("a text result")
            .to_owned()
    }

    #[test]
    fn a_client_that_declared_no_elicitation_is_refused_and_nothing_is_recorded() {
        let rig = Rig::new();
        for capabilities in [
            json!({}),
            json!({ "roots": {} }),
            json!({ "elicitation": true }),
        ] {
            let output = rig.serve(&[initialize(capabilities), send(2, RECIPIENT, "hello")]);
            let text = text_of(reply(&output, 2));
            assert!(text.contains("message-needs-a-person"), "{text}");
            assert_eq!(reply(&output, 2)["result"]["isError"], true);
            assert!(questions(&output).is_empty());
        }
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn a_send_the_mailbox_would_refuse_is_refused_before_the_person_is_asked() {
        let rig = Rig::new();
        for (to, text, code) in [
            ("codex-gone0a1b2c3d", "hello", "message-no-live-session"),
            (RECIPIENT, "   ", "message-empty"),
        ] {
            let output = rig.serve(&[initialize(asks()), send(2, to, text)]);
            let answer = text_of(reply(&output, 2));
            assert!(answer.contains(code), "{answer}");
            assert!(questions(&output).is_empty(), "{output:?}");
        }
        assert_eq!(rig.rows_from(SENDER), 0);
    }

    #[test]
    fn a_call_with_no_initialize_is_never_asked_about() {
        let rig = Rig::new();
        let output = rig.serve(&[send(2, RECIPIENT, "hello")]);
        assert!(text_of(reply(&output, 2)).contains("message-needs-a-person"));
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn every_answer_but_an_allow_is_refused_and_nothing_is_recorded() {
        let rig = Rig::new();
        let refusals = [
            choose(1, "deny"),
            answer(1, json!({ "action": "decline" })),
            answer(1, json!({ "action": "cancel" })),
            answer(
                1,
                json!({ "action": "decline", "content": { "choice": "allow_once" } }),
            ),
            answer(
                1,
                json!({ "action": "cancel", "content": { "choice": "allow_session" } }),
            ),
            answer(
                1,
                json!({ "action": "accept", "content": { "choice": "yes please" } }),
            ),
            answer(1, json!({ "action": "accept" })),
            json!({ "jsonrpc": "2.0", "id": "estigia-elicit-1",
                    "error": { "code": -32601, "message": "no" } }),
        ];
        for refusal in refusals {
            let output = rig.serve(&[initialize(asks()), send(2, RECIPIENT, "hello"), refusal]);
            assert_eq!(questions(&output).len(), 1);
            let text = text_of(reply(&output, 2));
            assert!(text.contains("message-not-allowed"), "{text}");
            assert_eq!(rig.rows_to(RECIPIENT), 0);
        }
    }

    #[test]
    fn the_question_names_the_recipient_the_issue_it_holds_and_the_text() {
        let rig = Rig::new();
        rig.hold(12, RECIPIENT);
        let output = rig.serve(&[
            initialize(asks()),
            send(2, "#12", "please rebase"),
            choose(1, "deny"),
        ]);
        let question = questions(&output)[0];
        let message = question["params"]["message"].as_str().expect("a message");
        for part in [RECIPIENT, "#12", "please rebase", SENDER] {
            assert!(message.contains(part), "{part} missing from {message}");
        }
        let choice = &question["params"]["requestedSchema"]["properties"]["choice"];
        assert_eq!(choice["enum"].as_array().map(Vec::len), Some(3));
        assert_eq!(choice["enumNames"].as_array().map(Vec::len), Some(3));
        assert_eq!(
            question["params"]["requestedSchema"]["required"],
            json!(["choice"])
        );
    }

    #[test]
    fn allow_once_records_one_row_from_the_sender_and_the_next_send_asks_again() {
        let rig = Rig::new();
        let output = rig.serve(&[
            initialize(asks()),
            send(2, RECIPIENT, "first"),
            choose(1, "allow_once"),
            send(3, RECIPIENT, "second"),
            choose(2, "deny"),
        ]);
        assert_eq!(questions(&output).len(), 2);
        assert_eq!(reply(&output, 2)["result"]["isError"], false);
        assert_eq!(reply(&output, 3)["result"]["isError"], true);
        assert_eq!(rig.rows_to(RECIPIENT), 1);
        assert_eq!(rig.rows_from(SENDER), 1);
    }

    #[test]
    fn allow_for_this_session_records_the_send_and_spares_the_next_question() {
        let rig = Rig::new();
        let output = rig.serve(&[
            initialize(asks()),
            send(2, RECIPIENT, "first"),
            choose(1, "allow_session"),
            send(3, RECIPIENT, "second"),
        ]);
        assert_eq!(questions(&output).len(), 1);
        assert_eq!(reply(&output, 3)["result"]["isError"], false);
        assert_eq!(rig.rows_to(RECIPIENT), 2);
        assert_eq!(rig.rows_from(SENDER), 2);
    }

    #[test]
    fn a_session_grant_belongs_to_the_run_that_was_given_it() {
        let rig = Rig::new();
        let mut other = send(3, RECIPIENT, "second");
        other["params"]["arguments"]["run_id"] = json!("claude-othr0a1b2c3d");
        let output = rig.serve(&[
            initialize(asks()),
            send(2, RECIPIENT, "first"),
            choose(1, "allow_session"),
            other,
        ]);
        assert_eq!(questions(&output).len(), 2);
        assert_eq!(rig.rows_to(RECIPIENT), 1);
    }

    #[test]
    fn a_session_grant_belongs_to_the_process_that_was_given_it() {
        let rig = Rig::new();
        rig.serve(&[
            initialize(asks()),
            send(2, RECIPIENT, "first"),
            choose(1, "allow_session"),
        ]);
        let output = rig.serve(&[initialize(asks()), send(2, RECIPIENT, "again")]);
        assert_eq!(questions(&output).len(), 1);
        assert_eq!(rig.rows_to(RECIPIENT), 1);
    }

    #[test]
    fn an_issue_number_reaches_the_run_the_store_shows_holding_it() {
        let rig = Rig::new();
        rig.hold(12, RECIPIENT);
        for (id, to) in [(2, "12"), (3, "#12")] {
            let output = rig.serve(&[
                initialize(asks()),
                send(id, to, "hello"),
                choose(1, "allow_once"),
            ]);
            assert_eq!(reply(&output, id)["result"]["isError"], false);
        }
        assert_eq!(rig.rows_to(RECIPIENT), 2);
    }

    #[test]
    fn an_issue_nobody_holds_is_refused_without_asking_and_nothing_is_recorded() {
        let rig = Rig::new();
        let output = rig.serve(&[initialize(asks()), send(2, "#99", "hello")]);
        assert!(text_of(reply(&output, 2)).contains("message-no-live-session"));
        assert!(questions(&output).is_empty());
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn a_stale_pointer_does_not_answer_an_issue_address() {
        let rig = Rig::new();
        let mut pointer = session::Run::new(RECIPIENT.to_owned());
        pointer.issue = Some(crate::identity::Identity::Number(12));
        session::store(&rig.context().state_root, &pointer).expect("the pointer writes");
        let output = rig.serve(&[initialize(asks()), send(2, "12", "hello")]);
        assert!(text_of(reply(&output, 2)).contains("message-no-live-session"));
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn a_line_that_arrives_while_the_person_is_asked_is_served_afterwards() {
        let rig = Rig::new();
        let output = rig.serve(&[
            initialize(asks()),
            send(2, RECIPIENT, "hello"),
            json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}),
            choose(1, "allow_once"),
            json!({"jsonrpc": "2.0", "id": 8, "method": "ping"}),
        ]);
        let order: Vec<String> = output
            .iter()
            .map(|message| {
                message["method"]
                    .as_str()
                    .map_or_else(|| message["id"].to_string(), str::to_owned)
            })
            .collect();
        assert_eq!(order, ["1", "elicitation/create", "2", "7", "8"]);
        assert_eq!(rig.rows_to(RECIPIENT), 1);
    }

    #[test]
    fn an_input_that_closes_while_the_person_is_asked_refuses_and_records_nothing() {
        let rig = Rig::new();
        let output = rig.serve(&[initialize(asks()), send(2, RECIPIENT, "hello")]);
        assert_eq!(questions(&output).len(), 1);
        let text = text_of(reply(&output, 2));
        assert!(text.contains("message-question-failed"), "{text}");
        assert!(!text.contains("message-not-allowed"), "{text}");
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn a_question_nobody_answers_in_time_refuses_and_records_nothing() {
        let rig = Rig::new();
        let (sender, lines) = std::sync::mpsc::channel();
        for line in [initialize(asks()), send(2, RECIPIENT, "hello")] {
            sender.send(Ok(line.to_string())).expect("queued");
        }
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = sender.send(Ok(choose(1, "allow_once").to_string()));
        });
        let mut output = Vec::new();
        super::super::serve_lines(
            &lines,
            &mut output,
            Ok(rig.context()),
            std::time::Duration::from_millis(100),
        )
        .expect("served");
        let output: Vec<Value> = String::from_utf8(output)
            .expect("utf-8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert!(text_of(reply(&output, 2)).contains("message-not-allowed"));
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn the_tool_takes_no_argument_that_skips_the_question() {
        let rig = Rig::new();
        let context = rig.context();
        for extra in ["requested_by", "confirmed", "consent"] {
            let mut arguments = json!({ "run_id": SENDER, "to": RECIPIENT, "text": "hello" });
            arguments[extra] = json!(true);
            let failure = run_tool("send_message", &arguments, Ok(&context))
                .expect_err("an argument that is not in the schema");
            assert!(matches!(failure, ToolFailure::Malformed(_)), "{failure}");
        }
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn the_default_answer_is_the_live_sessions_and_the_registry_is_the_opt_in() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        rig.seen(
            "claude-live0a1b2c3d",
            "claude",
            "host-live",
            Some("/repo/one"),
            now,
        );
        let context = rig.context();

        let around = answered(run_tool("list_sessions", &json!({}), Ok(&context)).expect("a list"));
        assert_eq!(around["live_count"].as_u64(), Some(1), "{around}");
        assert_eq!(around["of"].as_u64(), Some(3), "{around}");
        assert_eq!(around["live"][0]["run_id"], "claude-live0a1b2c3d");
        assert_eq!(around["live"][0]["name"], "one-1", "{around}");
        assert_eq!(around["live"][0]["repo"], "one", "{around}");
        assert!(around.get("sessions").is_none(), "{around}");

        let page = answered(
            run_tool("list_sessions", &json!({"limit": 10}), Ok(&context)).expect("a page"),
        );
        assert_eq!(page["matched"].as_u64(), Some(3), "{page}");
        assert_eq!(
            page["stale_after"],
            crate::cli::sessions::STALE_AFTER_SECONDS
        );
    }

    #[test]
    fn a_short_name_and_the_runtime_vocabulary_address_the_same_live_rows() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        rig.seen(
            "opencode-live0a1b2c3d",
            "opencode",
            "host",
            Some("/repo/estigia"),
            now,
        );
        let context = rig.context();
        for said in [
            "opencode",
            "opencode@estigia",
            "estigia-1",
            "opencode-live0a1b2c3d",
        ] {
            let answer = answered(
                run_tool("list_sessions", &json!({"address": said}), Ok(&context))
                    .expect("an addressed list"),
            );
            assert_eq!(answer["matched"].as_u64(), Some(1), "{said}: {answer}");
            assert_eq!(
                answer["live"][0]["run_id"], "opencode-live0a1b2c3d",
                "{said}: {answer}"
            );
        }
    }

    #[test]
    fn an_address_nobody_live_matches_answers_the_closest_and_sends_nothing() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        rig.seen(
            "opencode-live0a1b2c3d",
            "opencode",
            "host",
            Some("/repo/estigia"),
            now,
        );
        let context = rig.context();
        let answer = answered(
            run_tool(
                "list_sessions",
                &json!({"address": "codex@estigia"}),
                Ok(&context),
            )
            .expect("an addressed list"),
        );
        assert_eq!(answer["matched"].as_u64(), Some(0), "{answer}");
        assert_eq!(answer["live"].as_array().map(Vec::len), Some(0), "{answer}");
        assert_eq!(answer["closest"][0]["runtime"], "opencode", "{answer}");
    }

    #[test]
    fn an_ambiguous_address_answers_every_candidate_and_sends_nothing() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        for run in ["opencode-one0a1b2c3d", "opencode-two0a1b2c3d"] {
            rig.seen(run, "opencode", "host", Some("/repo/estigia"), now);
        }
        let output = rig.serve(&[initialize(asks()), send(2, "opencode@estigia", "hello")]);
        let text = text_of(reply(&output, 2));
        assert!(text.contains("message-recipient-ambiguous"), "{text}");
        for run in ["opencode-one0a1b2c3d", "opencode-two0a1b2c3d"] {
            assert!(text.contains(run), "{run} missing from {text}");
        }
        assert!(questions(&output).is_empty(), "{output:?}");
        assert_eq!(rig.rows_to("opencode-one0a1b2c3d"), 0);
        assert_eq!(rig.rows_to("opencode-two0a1b2c3d"), 0);
    }

    #[test]
    fn one_call_hands_an_issue_to_the_opencode_session_of_a_repository_from_every_runtime() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        let investora = "opencode-invest0a1b2c3";
        let estigia = "opencode-estig0a1b2c3d";
        rig.seen(
            investora,
            "opencode",
            "ses_inv",
            Some("/work/investora"),
            now,
        );
        rig.seen(estigia, "opencode", "ses_est", Some("/work/estigia"), now);

        for (number, from) in [
            (10, SENDER),
            (20, "opencode-sndr0a1b2c3d"),
            (30, "codex-sndr0a1b2c3d"),
            (40, "pi-sndr0a1b2c3d"),
        ] {
            rig.seen(
                from,
                &from[..from.rfind('-').unwrap_or(0)],
                "host",
                None,
                now,
            );

            let first = rig.serve(&[
                initialize(asks()),
                send_from(from, number, "opencode@investora", "flow dev 723 investora"),
                choose(1, "allow_once"),
            ]);
            assert_eq!(
                reply(&first, number)["result"]["isError"],
                false,
                "{from}: {first:?}"
            );
            let said = text_of(reply(&first, number));
            assert!(said.contains(investora), "{from}: {said}");
            assert!(said.contains("queued"), "{from}: {said}");

            let second = rig.serve(&[
                initialize(asks()),
                send_from(from, number + 1, "opencode@estigia", "flow dev 170 estigia"),
                choose(1, "allow_once"),
            ]);
            assert_eq!(
                reply(&second, number + 1)["result"]["isError"],
                false,
                "{from}: {second:?}"
            );
        }

        assert_eq!(rig.rows_to(investora), 4);
        assert_eq!(rig.rows_to(estigia), 4);
        assert_eq!(rig.rows_to(RECIPIENT), 0);
    }

    #[test]
    fn a_reply_answers_the_sender_without_a_run_id() {
        let rig = Rig::new();
        let output = rig.serve(&[
            initialize(asks()),
            send(2, RECIPIENT, "first"),
            choose(1, "allow_once"),
            answer_to(3, RECIPIENT, 1, "answer"),
            choose(2, "allow_once"),
        ]);
        assert_eq!(reply(&output, 3)["result"]["isError"], false);
        assert_eq!(rig.rows_to(RECIPIENT), 1);
        assert_eq!(rig.rows_from(SENDER), 1);
        assert_eq!(rig.rows_to(SENDER), 1);
    }

    #[test]
    fn a_reply_to_a_message_a_person_sent_names_the_reason() {
        let rig = Rig::new();
        let receipt = crate::harness::mailbox::send(
            &rig.context().state_root,
            None,
            RECIPIENT,
            "from a person",
        )
        .expect("a person sends");
        let output = rig.serve(&[
            initialize(asks()),
            answer_to(2, RECIPIENT, receipt.id, "answer"),
        ]);
        assert!(text_of(reply(&output, 2)).contains("message-reply-has-no-sender"));
        assert_eq!(rig.rows_to(RECIPIENT), 1);
    }

    fn answered(result: Value) -> Value {
        serde_json::from_str(result["content"][0]["text"].as_str().expect("text")).expect("json")
    }

    fn refusal_code(failure: ToolFailure) -> String {
        let ToolFailure::Refused(refusal) = failure else {
            panic!("refused as something other than a refusal: {failure}");
        };
        refusal.code.to_owned()
    }

    fn run_ids(page: &Value) -> Vec<String> {
        page["sessions"]
            .as_array()
            .expect("a page of rows")
            .iter()
            .map(|row| row["run_id"].as_str().expect("a run id").to_owned())
            .collect()
    }

    const INSERTED: &str = "pi-new0a1b2c3d";

    fn churn(rig: &Rig) {
        rig.seen("pi-c0000003", "pi", "host-c", None, 9_000_000_000);
        rig.seen("claude-d0000004", "claude", "host-d", None, 1);
        rig.ended("codex-e0000005", 9_000_000_000);
        rig.ended("claude-g0000007", 9_000_000_000);
        rig.seen(
            "claude-g0000007",
            "claude",
            "host-g-moved",
            None,
            9_000_000_000,
        );
        rig.seen("pi-f0000006", "pi", "host-f-moved", None, 5_003);
        rig.seen(INSERTED, "pi", "host-new", Some("/repo/new"), 9_000_000_000);
    }

    #[test]
    fn a_snapshot_is_walked_without_omissions_or_duplicates_while_the_store_moves() {
        let rig = Rig::new();
        for (run_id, runtime, started, seen) in [
            ("claude-a0000001", "claude", 5_000u64, 5_000u64),
            ("codex-b0000002", "codex", 5_000, 5_000),
            ("pi-c0000003", "pi", 5_001, 5_001),
            ("claude-d0000004", "claude", 5_002, 5_002),
            ("codex-e0000005", "codex", 5_003, 5_003),
            ("pi-f0000006", "pi", 5_004, 5_003),
            ("claude-g0000007", "claude", 5_005, 5_005),
        ] {
            rig.seen(run_id, runtime, "host-session", None, started);
            if seen != started {
                rig.seen(run_id, runtime, "host-session", None, seen);
            }
        }

        let first = answered(rig.list(json!({"limit": 2})).expect("a first page"));
        let taken_at = first["taken_at"].as_u64().expect("a snapshot time");
        let mut rows: Vec<(u64, String)> = rig
            .store()
            .sessions()
            .expect("the rows read")
            .into_iter()
            .filter(|row| row.started_at <= taken_at)
            .map(|row| (row.started_at, row.run_id))
            .collect();
        rows.sort();
        let expected: Vec<String> = rows.into_iter().map(|(_, run_id)| run_id).collect();
        assert_eq!(expected.len(), 9, "{expected:?}");

        let mut walked = run_ids(&first);
        let mut cursor = first["next_cursor"].as_str().map(str::to_owned);
        let mut pages = 1;
        while let Some(token) = cursor {
            if pages == 1 {
                churn(&rig);
            }
            let answer = answered(
                rig.list(json!({"limit": 2, "cursor": token}))
                    .expect("a continued page"),
            );
            assert_eq!(answer["taken_at"].as_u64(), Some(taken_at));
            assert_eq!(answer["matched"].as_u64(), Some(expected.len() as u64));
            let page = run_ids(&answer);
            assert_eq!(answer["returned"].as_u64(), Some(page.len() as u64));
            walked.extend(page);
            cursor = answer["next_cursor"].as_str().map(str::to_owned);
            pages += 1;
        }

        assert_eq!(walked, expected, "the traversal is not the snapshot");
        assert!(!walked.contains(&INSERTED.to_owned()));
        assert_eq!(pages, 5, "the page size did not hold: {walked:?}");
    }

    #[test]
    fn a_pointer_only_run_is_absent_from_a_bounded_traversal_and_present_in_the_full_list() {
        let rig = Rig::new();
        rig.seen("claude-a0000001", "claude", "host-a", None, 5_000);
        rig.seen("codex-b0000002", "codex", "host-b", None, 5_001);
        rig.pointer("pi-old0a1b2c3d", 11);
        let first = answered(rig.list(json!({"limit": 1})).expect("a first page"));
        rig.pointer("pi-new0a1b2c3d", 12);
        let mut walked = run_ids(&first);
        let mut cursor = first["next_cursor"].as_str().map(str::to_owned);
        while let Some(token) = cursor {
            let answer = answered(
                rig.list(json!({"limit": 1, "cursor": token}))
                    .expect("a continued page"),
            );
            walked.extend(run_ids(&answer));
            cursor = answer["next_cursor"].as_str().map(str::to_owned);
        }
        assert_eq!(
            walked,
            vec![
                "claude-send0a1b2c3d",
                "codex-recv0a1b2c3d",
                "claude-a0000001",
                "codex-b0000002"
            ],
            "a bounded snapshot is the registry rows, and a pointer-only run is not one of them"
        );

        let full: Vec<String> = crate::cli::sessions::entries(&rig.context().state_root)
            .expect("the registry reads")
            .into_iter()
            .map(|entry| entry.run_id)
            .collect();
        assert!(full.contains(&"pi-old0a1b2c3d".to_owned()), "{full:?}");
        assert!(full.contains(&"pi-new0a1b2c3d".to_owned()), "{full:?}");
    }

    #[test]
    fn a_status_unregistered_selector_is_refused_through_the_tool() {
        let rig = Rig::new();
        rig.pointer("pi-ghost0a1b2c3d", 12);
        let ToolFailure::Refused(refusal) = rig
            .list(json!({"status": "unregistered"}))
            .expect_err("a refusal")
        else {
            panic!("refused as something other than a refusal");
        };
        assert_eq!(refusal.code, "sessions-unregistered-not-bounded");
        assert!(
            refusal.message.contains("pointer-only"),
            "the refusal does not name what is missing: {}",
            refusal.message
        );
        assert!(
            refusal.message.contains("estigia sessions"),
            "the refusal does not say the full list still shows it: {}",
            refusal.message
        );
    }

    #[test]
    fn selectors_answer_only_the_rows_they_name() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        rig.seen(
            "claude-live0a1b2c3d",
            "claude",
            "host-live",
            Some("/repo/one"),
            now,
        );
        rig.seen(
            "pi-stale0a1b2c3d",
            "pi",
            "host-stale",
            Some("/repo/two"),
            now - crate::cli::sessions::STALE_AFTER_SECONDS - 1,
        );
        rig.seen(
            "codex-end0a1b2c3d",
            "codex",
            "host-end",
            Some("/repo/one"),
            now,
        );
        rig.ended("codex-end0a1b2c3d", now);
        rig.pointer("pi-ghost0a1b2c3d", 12);

        let selected = |arguments: Value| -> Vec<String> {
            run_ids(&answered(rig.list(arguments).expect("a page")))
        };

        assert_eq!(
            selected(json!({"status": "live"})),
            vec!["claude-live0a1b2c3d"]
        );
        assert_eq!(
            selected(json!({"status": "stale"})),
            vec![
                "claude-send0a1b2c3d",
                "codex-recv0a1b2c3d",
                "pi-stale0a1b2c3d"
            ]
        );
        assert_eq!(
            selected(json!({"status": "ended"})),
            vec!["codex-end0a1b2c3d"]
        );
        assert_eq!(selected(json!({"runtime": "pi"})), vec!["pi-stale0a1b2c3d"]);
        assert_eq!(
            selected(json!({"run": "codex-end0a1b2c3d"})),
            vec!["codex-end0a1b2c3d"]
        );
        assert_eq!(
            selected(json!({"session_id": "host-stale"})),
            vec!["pi-stale0a1b2c3d"]
        );
        assert_eq!(
            selected(json!({"repo_dir": "/repo/one"})),
            vec!["claude-live0a1b2c3d", "codex-end0a1b2c3d"]
        );
    }

    #[test]
    fn a_malformed_cursor_is_refused_not_restarted() {
        let rig = Rig::new();
        let code = refusal_code(
            rig.list(json!({"cursor": "not-a-cursor"}))
                .expect_err("a refusal"),
        );
        assert_eq!(code, "sessions-cursor-malformed");
    }

    #[test]
    fn an_oversized_cursor_is_refused_before_it_is_read() {
        let rig = Rig::new();
        let oversized = "a".repeat(crate::cli::sessions::MAX_CURSOR_BYTES + 1);
        let code = refusal_code(
            rig.list(json!({"cursor": oversized}))
                .expect_err("a refusal"),
        );
        assert_eq!(code, "sessions-cursor-too-large");
    }

    #[test]
    fn an_edited_cursor_is_refused() {
        let rig = Rig::new();
        rig.seen(
            "claude-second0a1b2c3d",
            "claude",
            "host-second",
            None,
            2_000,
        );
        let page = answered(rig.list(json!({"limit": 1})).expect("a page"));
        let token = page["next_cursor"].as_str().expect("a cursor").to_owned();
        let (payload, digest) = token.split_once('.').expect("a payload and a digest");
        let mut flipped = payload.to_owned();
        let first = flipped.as_bytes()[0];
        flipped.replace_range(0..1, if first == b'a' { "b" } else { "a" });
        let tampered = format!("{flipped}.{digest}");
        let code = refusal_code(
            rig.list(json!({"limit": 1, "cursor": tampered}))
                .expect_err("a refusal"),
        );
        assert_eq!(code, "sessions-cursor-tampered");
    }

    #[test]
    fn a_cursor_from_other_selectors_is_refused() {
        let rig = Rig::new();
        rig.seen(
            "claude-second0a1b2c3d",
            "claude",
            "host-second",
            None,
            2_000,
        );
        let page = answered(
            rig.list(json!({"runtime": "claude", "limit": 1}))
                .expect("a page"),
        );
        let token = page["next_cursor"].as_str().expect("a cursor").to_owned();
        let code = refusal_code(
            rig.list(json!({"runtime": "codex", "limit": 1, "cursor": token}))
                .expect_err("a refusal"),
        );
        assert_eq!(code, "sessions-cursor-mismatch");
    }

    #[test]
    fn an_expired_cursor_is_refused() {
        let rig = Rig::new();
        let context = rig.context();
        let token = crate::cli::sessions::cursor_token(&crate::cli::sessions::Cursor {
            version: crate::cli::sessions::PAGE_VERSION,
            selectors: crate::cli::sessions::Selectors::default(),
            taken_at: 1,
            last_started_at: 0,
            last_run_id: "claude-send0a1b2c3d".to_owned(),
            store: store::store_identity(&store::beside(&context.state_root)),
        });
        let code = refusal_code(rig.list(json!({"cursor": token})).expect_err("a refusal"));
        assert_eq!(code, "sessions-cursor-expired");
    }

    #[test]
    fn a_cursor_from_a_replaced_store_is_refused() {
        let rig = Rig::new();
        let token = crate::cli::sessions::cursor_token(&crate::cli::sessions::Cursor {
            version: crate::cli::sessions::PAGE_VERSION,
            selectors: crate::cli::sessions::Selectors::default(),
            taken_at: 1,
            last_started_at: 0,
            last_run_id: "claude-send0a1b2c3d".to_owned(),
            store: "replaced".to_owned(),
        });
        let code = refusal_code(rig.list(json!({"cursor": token})).expect_err("a refusal"));
        assert_eq!(code, "sessions-cursor-store-replaced");
    }

    #[test]
    fn a_limit_above_the_maximum_is_refused_rather_than_clamped() {
        let rig = Rig::new();
        let code = refusal_code(
            rig.list(json!({"limit": crate::cli::sessions::MAX_LIMIT + 1}))
                .expect_err("a refusal"),
        );
        assert_eq!(code, "sessions-limit-too-large");
    }

    #[test]
    fn a_limit_the_shared_boundary_cannot_read_is_refused_before_the_page() {
        let rig = Rig::new();
        for limit in [json!(0), json!(-1), json!(1.5), json!("5"), json!(null)] {
            let failure = rig
                .list(json!({"limit": limit}))
                .expect_err("a malformed call");
            assert!(
                matches!(failure, ToolFailure::Malformed(_)),
                "{limit} was answered as something other than a malformed call: {failure}"
            );
            let answer = ask(json!({
                "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                "params": { "name": "list_sessions", "arguments": { "limit": limit } },
            }))
            .expect("an answer");
            assert_eq!(
                answer["error"]["code"],
                super::super::code::INVALID_PARAMS,
                "{limit} did not answer the malformed-params code: {answer}"
            );
        }

        let ToolFailure::Malformed(said) = rig
            .list(json!({"limit": null}))
            .expect_err("a malformed call")
        else {
            panic!("a null `limit` was not answered as a malformed call");
        };
        assert!(
            said.contains("whole number"),
            "`docs/sessions.md` says the integer check answers a null, and this is not it: {said}"
        );
    }

    #[test]
    fn a_limit_of_one_is_the_smallest_page_and_still_answers() {
        let rig = Rig::new();
        let page = answered(rig.list(json!({"limit": 1})).expect("a page"));
        assert_eq!(page["limit"].as_u64(), Some(1));
        assert_eq!(run_ids(&page).len(), 1);
    }

    #[test]
    fn the_sessions_limit_is_declared_an_integer_in_the_table_and_the_schema() {
        let tool = crate::harness::mcp::tools::find("list_sessions").expect("a tool");
        let limit = tool
            .arguments
            .iter()
            .find(|argument| argument.name == "limit")
            .expect("`list_sessions` takes a limit");
        assert_eq!(limit.json_type, "integer");
        assert_eq!(
            tool.schema()["inputSchema"]["properties"]["limit"]["type"],
            "integer",
            "the schema does not publish the type the table declares"
        );
    }

    #[test]
    fn a_cursor_payload_that_is_not_hexadecimal_is_refused_whatever_its_bytes() {
        let rig = Rig::new();
        for token in [
            "€a.digest",
            "a€.digest",
            "€€.digest",
            "abc.digest",
            "zz.digest",
        ] {
            let code = refusal_code(rig.list(json!({"cursor": token})).expect_err("a refusal"));
            assert_eq!(code, "sessions-cursor-malformed", "{token}");
        }
    }

    /// A `status` selector is read from each page's own read of the row, so a
    /// traversal filtered by `status` can omit a row whose status changed between
    /// pages, and `matched` is that page's count rather than a count held across
    /// the walk. This pins the behaviour `docs/sessions.md` states; a design that
    /// froze the matched set would move it.
    ///
    /// A change that made the walk complete — the thing this design cannot do
    /// statelessly — would return `codex-b0000002` and redden this.
    #[test]
    fn a_status_filtered_traversal_reads_the_status_afresh_and_can_omit_a_changed_row() {
        let rig = Rig::new();
        let now = crate::harness::session::now_seconds().expect("a clock");
        for (run_id, runtime, started) in [
            ("claude-a0000001", "claude", 5_000u64),
            ("codex-b0000002", "codex", 5_001),
            ("pi-c0000003", "pi", 5_002),
        ] {
            rig.seen(run_id, runtime, "host-session", None, started);
            rig.seen(run_id, runtime, "host-session", None, now);
        }

        let first = answered(
            rig.list(json!({"status": "live", "limit": 1}))
                .expect("a first page"),
        );
        assert_eq!(run_ids(&first), vec!["claude-a0000001"]);
        assert_eq!(first["matched"].as_u64(), Some(3));
        let cursor = first["next_cursor"].as_str().expect("a cursor").to_owned();

        rig.ended("codex-b0000002", now);

        let second = answered(
            rig.list(json!({"status": "live", "limit": 1, "cursor": cursor}))
                .expect("a continued page"),
        );
        assert_eq!(
            run_ids(&second),
            vec!["pi-c0000003"],
            "the row that ended between pages is read afresh and drops out of the walk"
        );
        assert_eq!(
            second["matched"].as_u64(),
            Some(2),
            "`matched` is the page's own count, not one held across the walk"
        );
        assert_eq!(second["next_cursor"], Value::Null);
    }

    #[test]
    fn a_cursor_from_another_version_is_refused() {
        let rig = Rig::new();
        let context = rig.context();
        let token = crate::cli::sessions::cursor_token(&crate::cli::sessions::Cursor {
            version: crate::cli::sessions::PAGE_VERSION + 1,
            selectors: crate::cli::sessions::Selectors::default(),
            taken_at: 1,
            last_started_at: 0,
            last_run_id: "claude-send0a1b2c3d".to_owned(),
            store: store::store_identity(&store::beside(&context.state_root)),
        });
        let code = refusal_code(rig.list(json!({"cursor": token})).expect_err("a refusal"));
        assert_eq!(code, "sessions-cursor-mismatch");
    }

    #[test]
    fn a_clock_that_cannot_be_read_refuses_the_page() {
        let rig = Rig::new();
        let context = rig.context();
        let refusal = crate::cli::sessions::page(&json!({}), &context.state_root, None)
            .expect_err("a refusal");
        assert_eq!(refusal.code, "clock-unreadable");
    }

    #[test]
    fn a_selector_past_the_bound_is_refused_with_the_bound_named() {
        let rig = Rig::new();
        let too_long = "d".repeat(crate::cli::sessions::MAX_SELECTOR_BYTES + 1);
        let ToolFailure::Refused(refusal) = rig
            .list(json!({"repo_dir": too_long}))
            .expect_err("a refusal")
        else {
            panic!("refused as something other than a refusal");
        };
        assert_eq!(refusal.code, "sessions-selector-too-large");
        assert!(
            refusal
                .message
                .contains(&crate::cli::sessions::MAX_SELECTOR_BYTES.to_string()),
            "the refusal does not say what the bound is: {}",
            refusal.message
        );
    }

    #[test]
    fn a_selector_at_the_bound_is_issued_a_cursor_the_tool_accepts() {
        let rig = Rig::new();
        let bound = crate::cli::sessions::MAX_SELECTOR_BYTES;
        let long = |byte: char| byte.to_string().repeat(bound);
        let (runtime, session_id, repo_dir) = (long('u'), long('s'), long('d'));
        let (first, second) = (long('a'), long('b'));
        rig.seen(&first, &runtime, &session_id, Some(&repo_dir), 5_000);
        rig.seen(&second, &runtime, &session_id, Some(&repo_dir), 5_001);

        let mut walked: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut arguments = json!({
                "runtime": runtime.clone(),
                "session_id": session_id.clone(),
                "repo_dir": repo_dir.clone(),
                "limit": 1,
            });
            if let Some(token) = &cursor {
                arguments["cursor"] = json!(token);
            }
            let page = answered(rig.list(arguments).expect("a page"));
            walked.extend(run_ids(&page));
            let Some(token) = page["next_cursor"].as_str().map(str::to_owned) else {
                break;
            };
            assert!(
                token.len() <= crate::cli::sessions::MAX_CURSOR_BYTES,
                "the tool issued a {}-byte cursor, past the {} it accepts",
                token.len(),
                crate::cli::sessions::MAX_CURSOR_BYTES
            );
            cursor = Some(token);
        }
        assert_eq!(
            walked,
            vec![first, second],
            "the walk did not visit the whole snapshot with the cursors it was issued"
        );
    }

    /// `started_at` has second granularity, so `started_at <= taken_at` admits a
    /// session that registers in the same second as `taken_at` but after the
    /// first page was taken. This pins that membership is frozen only to the
    /// second, while the ordering key is what is frozen outright; a design that
    /// stored the snapshot would move it.
    #[test]
    fn a_session_registered_in_the_same_second_as_taken_at_can_join_the_walk() {
        let rig = Rig::new();
        rig.seen("claude-a0000001", "claude", "host-a", None, 5_000);
        rig.seen("codex-b0000002", "codex", "host-b", None, 5_001);

        let first = answered(rig.list(json!({"limit": 1})).expect("a first page"));
        let taken_at = first["taken_at"].as_u64().expect("a snapshot time");
        let cursor = first["next_cursor"].as_str().expect("a cursor").to_owned();

        rig.seen("pi-c0000003", "pi", "host-c", None, taken_at);

        let second = answered(
            rig.list(json!({"limit": 10, "cursor": cursor}))
                .expect("a continued page"),
        );
        assert!(
            run_ids(&second).contains(&"pi-c0000003".to_owned()),
            "a row registered in the second of `taken_at` was not admitted: {:?}",
            run_ids(&second)
        );
    }
}
