/// The transport's source: the dispatcher that answers in this process.
///
/// These crossings read `skill/scripts/github.py` and its `argparse` calls,
/// because that file was the transport and the port was unfinished. It said so
/// itself — *"the day the port answers for all of it, these checks compare
/// against the port and this function goes with the file"*. That day came: the
/// binding is deleted, this repository holds one transport in one language, and
/// the flags a tool sends are checked against the code that reads them.
///
/// Not a weaker check for the move. A stronger one: it was comparing the
/// published schema against a parser that no longer ran.
fn transport_source() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("transport")
            .join("dispatch.rs"),
    )
    .expect("the dispatcher is part of this crate")
}

/// Every `.rs` file of the transport, so a body can be found wherever it is.
fn transport_bodies() -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("transport");
    let mut all = String::new();
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
                all.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                all.push('\n');
            }
        }
    }
    all
}

/// The body of the function one operation runs, wherever the dispatcher sends it.
///
/// The dispatcher is a table of one-line arms, so the answer to *what does this
/// operation do* is never in the arm. It is in `commands.rs`, `claim.rs`,
/// `branch.rs` or `board.rs`, under the name the arm calls — and the two checks
/// below are about what the operation **does**, not about how it is reached.
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
    let bodies = transport_bodies();
    let opens = format!("\npub fn {name}(");
    let Some(start) = bodies.find(&opens) else {
        return String::new();
    };
    let rest = &bodies[start + 1..];
    rest.find("\npub fn ")
        .map_or(rest, |at| &rest[..at])
        .to_owned()
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
/// The dispatcher reads its arguments through one type, so the shape of a flag
/// is the method that asks for it: `need` and `number` refuse without it,
/// `number` parses it as a whole number, `on` takes no value at all. Two
/// closures stand in front of the match for the flags almost everything wants —
/// `issue()` and `run_id()` — and an arm that calls one is asking for that flag
/// as surely as one that spells it out.
fn transport_reads(operation: &str) -> Vec<(String, &'static str)> {
    let Some(block) = transport_block(operation) else {
        return Vec::new();
    };
    let mut found: Vec<(String, &'static str)> = Vec::new();
    for (call, how) in [
        ("f.need(\"", "need"),
        ("f.number(\"", "number"),
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
    // A flag the arm parses itself. `list-state` reads `--limit` with
    // `f.get("limit").and_then(|limit| limit.trim().parse::<u32>())` — no
    // `f.number`, because it defaults rather than refusing — and it is an
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
    if block.contains("run_id()") {
        note(&mut found, "run-id", "need");
    }
    found
}

/// Adds one flag, spelled the way a caller writes it, keeping the firmest read.
///
/// One flag can be asked for twice in an arm — `f.get("run-id")` in one branch
/// and `run_id()?` in another — and the two say different things about whether
/// the transport will run without it. The firmer answer is the one that
/// decides: a flag some path demands is a flag the tool has to send.
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
/// Measured by mutation: deleting the `"ping"` arm outright left the whole
/// suite green. `ping` is how a client asks whether the server is still there,
/// and a client that gets *method not found* for it is entitled to hang up —
/// after which the agent has its tools registered, reported on by `status`, and
/// silent. That is the exact failure `initialize`'s own documentation says this
/// server takes care to avoid, in the one method nothing was watching.
///
/// The list is read out of the dispatch rather than written here, so a method
/// added later is crossed by this test or fails it. Reading the source is what
/// keeps the two ends from drifting — the same reason `tests/guards.rs` finds
/// its files by walking the directory instead of listing them.
#[test]
fn every_method_this_server_implements_is_answered() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/harness/mcp/mod.rs"),
    )
    .expect("this module's own source");
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
    // The floor. Four arms answer by name today, and a dispatch this read as
    // empty would pass every assertion below without measuring one of them.
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
        // an honest `invalid params`. What must not come back is *this server
        // does not implement that*, which is what a deleted arm produces.
        assert_ne!(
            answer["error"]["code"],
            super::code::METHOD_NOT_FOUND,
            "{method} is dispatched by name and answered as though it were not: {answer}"
        );
    }

    // And `ping`, whose whole contract is an empty success, answers with one —
    // an arm that dispatched and then failed would satisfy the loop above.
    assert_eq!(
        ask(json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}})).expect("an answer")
            ["result"],
        json!({}),
        "ping does not answer, so a client checking whether the server is there is told it is not"
    );

    // And one that is not implemented still says so, or the loop above would
    // pass against a server that answers everything.
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

/// The numbers on the wire, written out rather than read back.
///
/// Every assertion on these compares the answer against the same constant that
/// produced it — `assert_eq!(response["protocolVersion"], PROTOCOL_VERSION)` —
/// so a typo in the constant is a typo in the expectation too, and the suite
/// stays green. It is the tautology this repository has now found three times
/// (the shell verbs, three of the gate lists, the stand-down cap): a test
/// driven by the constant cannot measure the constant.
///
/// These four numbers and one date are not ours to choose. The error codes are
/// JSON-RPC 2.0's, and a client matches on them; the protocol version is the
/// one revision this server implements, and `initialize`'s own note says what
/// being wrong costs — *the client is entitled to hang up, and then the agent
/// has its tools registered, reported on, and silent*.
///
/// So they are written here by hand. Changing one on purpose means changing it
/// in two places, which is the point: the second place is where somebody says
/// out loud that a wire contract moved.
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

    // And they are distinct, because a client tells them apart by number.
    let codes = [
        super::code::PARSE_ERROR,
        super::code::INVALID_REQUEST,
        super::code::METHOD_NOT_FOUND,
        super::code::INVALID_PARAMS,
    ];
    let distinct: std::collections::BTreeSet<i64> = codes.iter().copied().collect();
    assert_eq!(distinct.len(), codes.len(), "two errors share a number");
}

/// A run id belonging to somebody else's checkout is not this run's to act under.
///
/// `estigia mcp` takes no arguments and the protocol carries no session, so the
/// `run_id` argument is the whole of a caller's identity — and the caller
/// supplies it. That is this product's first rule inverted: a claim is
/// adjudicated, not asserted, and here the identity the claim is exercised under
/// was asserted and believed.
///
/// The guard next door covers the one name every unidentified session shares,
/// and states the general cost in its own words: *a release under a name no run
/// owns takes an issue away from whoever actually holds it.* Another live run's
/// id does exactly that, and it is discoverable — the ledger carries run ids and
/// so does every claim comment.
///
/// What is checkable is the fact the gate already checks one file over: a claim
/// covers the checkout it was made in and the isolated one the run was given,
/// and anything from elsewhere is not covered by it. The rule was stated there
/// and not carried here, so a *tool call* was measured against a claim that a
/// *write* in the same directory would not have been.
/// A record nothing can read stops every tool, not only the ones that swear.
///
/// The gate denies every write from a run whose pointer is on disk and will not
/// parse, on the directive every agent is handed: an unknown result is not
/// clearance. This refused it too — inside the `Swear` branch, so `release`,
/// `transition` and `heartbeat` walked past it to the tracker. A release under
/// a run whose record cannot be read puts down an issue nothing could say it
/// held.
///
/// That is the same shape the guard above it records having been fixed out of,
/// word for word: *the first cut of this guard sat inside the `Swear` branch,
/// and `release` walked straight past it to the tracker.* The lesson was
/// written down beside the guard that learned it and not carried to its
/// neighbour, which is why this walks **every** tool that takes a `run_id`
/// rather than the ones somebody remembered.
/// Releasing another issue does not take the held one's retry key with it.
///
/// `release_id` is one key, not one per issue. A release naming a number this
/// run does not hold minted a fresh key and **stored** it, writing over the one
/// the held issue's retry depends on — before the transport had been reached at
/// all, so a call that never got past this process still changed what a later
/// one means.
///
/// What that costs is in the comment beside the key: the transport answers a
/// repeated release from the marker already on the issue, *and only if the id is
/// the same one*. So the genuine release, retried, arrives as a stranger and is
/// recorded twice — which is the whole of what the key exists to prevent.
///
/// It is not refused on the pointer, and that is deliberate: the pointer is a
/// note about what was last read and none of it is authority. Whether this run
/// may release that issue is the timeline's answer, not this file's.
#[test]
fn a_claim_records_the_key_its_retry_depends_on() {
    // The arm this measures supplies `--operation-id` and `--runtime`, which
    // `claim` and `reclaim` *require* — argparse rejects the call without them.
    // Nothing exercised it: removing the arm outright left all 607 tests green,
    // so the flags that make the one operation the whole harness rests on work
    // at all were held by code no test read. Its neighbour `unassign` had this
    // coverage; these two did not.
    //
    // Observable without a transport, because the arm mints the key and writes
    // it to the pointer before the call goes anywhere: the run stops at a
    // transport that is not installed, and the key is already down.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::create_dir_all(&context.repo_dir).expect("the checkout");

    let pointer = context.state_root.join("claude-swearing.json");
    let write = |issue: Option<u64>, operation_id: Option<&str>| {
        let mut run = crate::harness::session::Run::new("claude-swearing".to_owned());
        run.issue = issue;
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
            &json!({"issue": issue, "run_id": "claude-swearing", "horizon": "2026-08-06T10:00Z"}),
            Ok(&context),
        );
    };

    // A run that has sworn nothing writes a key down before it asks.
    write(None, None);
    swear(12);
    let first = key_now();
    assert!(
        first.as_str().is_some_and(|key| key.len() == 32),
        "a claim recorded no key, so the transport is being sent a call it requires \
         `--operation-id` for: {first}"
    );

    // And a retry for the same issue reuses it, which is the whole reason the
    // key is stored rather than minted per call: a repeated claim under a new
    // key is the second claim event this prevents.
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
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::create_dir_all(&context.repo_dir).expect("the checkout");

    let minted = "a".repeat(32);
    let pointer = context.state_root.join("claude-holder.json");
    let write = |release_id: Option<&str>| {
        let mut run = crate::harness::session::Run::new("claude-holder".to_owned());
        run.issue = Some(12);
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

    // Another issue: the call goes through to the transport — which is not
    // installed here, and that is the point at which it stops — and the key the
    // held issue depends on is untouched.
    write(Some(&minted));
    let _ = run_tool(
        "release",
        &json!({"issue": 99, "run_id": "claude-holder"}),
        Ok(&context),
    );
    assert_eq!(
        key_now(),
        serde_json::Value::String(minted),
        "a release naming another issue overwrote the held issue's retry key"
    );

    // The floor: the held issue still gets one written down, or the dedupe this
    // protects never happens in the first place and the assertion above guards
    // an empty field.
    write(None);
    let _ = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-holder"}),
        Ok(&context),
    );
    let stored = key_now();
    assert!(
        stored.as_str().is_some_and(|key| key.len() == 32),
        "releasing the held issue no longer records a key to retry under: {stored}"
    );

    // And it is reused rather than reminted, which is what makes a retry a
    // repeat.
    let first = stored.as_str().expect("a key").to_owned();
    let _ = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-holder"}),
        Ok(&context),
    );
    assert_eq!(
        key_now(),
        serde_json::Value::String(first),
        "a second release of the held issue minted a new key, so the transport cannot see it \
         as a repeat"
    );
}

/// An argument declared `integer` is refused when it is not one.
///
/// `flags_for` type-checked `boolean` and nothing else, so `{"issue": "twelve"}`
/// against a schema that says integer was rendered verbatim and handed to the
/// transport. What came back was argparse refusing the whole call — and the
/// note beside the boolean check records what that costs, in the same words,
/// for the same reason: *the agent read the failure as a configuration defect*.
///
/// The schema is the contract this server publishes. A server that publishes
/// one and enforces a looser one has published a description rather than a
/// contract, and the agent that believed it gets its error from two processes
/// away.
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
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    // On disk, under this name, and not parseable: the state `session::load`
    // marks unreadable rather than absent.
    std::fs::write(
        context.state_root.join("claude-torn.json"),
        "{\"run_id\": \"claude-torn\", \"issue\":",
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
                    // A value the argument actually takes. `"x"` is not a
                    // workflow state, and filling one in made these tests pose
                    // a malformed call — which the rule under test never
                    // reaches, so they would have measured the argument check
                    // instead of themselves.
                    _ => argument
                        .choices
                        .map_or_else(|| json!("x"), |choices| json!(choices[0])),
                },
            );
        }
        arguments.insert("run_id".to_owned(), json!("claude-torn"));

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

    // And a record that reads is not refused by this rule. It goes on and fails
    // for its own reasons — there is no tracker here — which is what makes the
    // loop above about the unreadable record rather than about anything else.
    let readable = crate::harness::session::Run::new("claude-whole".to_owned());
    std::fs::write(
        context.state_root.join("claude-whole.json"),
        serde_json::to_string(&readable).expect("the pointer serialises"),
    )
    .expect("a readable pointer");
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "claim",
        &json!({"issue": 12, "run_id": "claude-whole", "horizon": "2099-01-01T00:00Z"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-pointer-unreadable",
            "a run whose record reads perfectly was told nothing could read it"
        );
    }
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
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: here.clone(),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");

    let pointer =
        |run_id: &str, repo: Option<&std::path::Path>, worktree: Option<&std::path::Path>| {
            let mut run = crate::harness::session::Run::new(run_id.to_owned());
            run.issue = Some(12);
            run.repo_dir = repo.map(std::path::Path::to_path_buf);
            run.worktree = worktree.map(std::path::Path::to_path_buf);
            std::fs::write(
                context.state_root.join(format!("{run_id}.json")),
                serde_json::to_string(&run).expect("the pointer serialises"),
            )
            .expect("the pointer is written");
        };

    // Somebody else's run, holding a claim over a checkout this server is not in.
    pointer("claude-elsewhere", Some(&elsewhere), None);
    let taking = json!({"issue": 12, "run_id": "claude-elsewhere"});
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
    // assertion above and take the product with it.
    //
    // A run holding this checkout is not refused by this rule. It goes on and
    // fails for its own reasons — there is no tracker here.
    pointer("claude-here", Some(&here), None);
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-here"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run acting in its own checkout was told it was somebody else's"
        );
    }

    // A run given an isolated checkout is covered in both: the claim was made in
    // the base repository and the delivery is written in the worktree, and a
    // call from either is this run's.
    pointer("claude-split", Some(&elsewhere), Some(&here));
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-split"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run was refused in the worktree its own claim covers"
        );
    }

    // A record holding only a worktree does not say where its claim was sworn,
    // and an incomplete record is an unknown one rather than a narrower one. It
    // is refused nowhere, because refusing it is a gate with no door in it: this
    // is the shape a run is stranded in, and every way out — the renewal, the
    // re-claim, the release — is a tool call from the very checkout the refusal
    // was naming. The call still has to satisfy the tracker, which is the only
    // thing that adjudicates.
    pointer("claude-worktree-only", None, Some(&elsewhere));
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-worktree-only"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run whose record never named its checkout was refused the way back"
        );
    }

    // And a run that has claimed nothing has no checkout to be outside of,
    // which is every first `claim` there has ever been.
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "claim",
        &json!({"issue": 12, "run_id": "claude-fresh", "horizon": "2099-01-01T00:00Z"}),
        Ok(&context),
    ) {
        assert_ne!(
            refusal.code, "run-id-names-another-checkout",
            "a run with nothing claimed yet was refused for holding somebody else's checkout"
        );
    }

    // And a server standing one directory below the claimed checkout is standing
    // inside it. This asked `same_directory` about a question that is *does this
    // claim cover this work*, so a tool call from a subdirectory was measured
    // against a claim a write in that same directory would not have been —
    // which is the sentence three lines above this check, run backwards.
    let deep = here.join("src").join("deep");
    std::fs::create_dir_all(&deep).expect("a subdirectory of this checkout");
    let below = GateContext {
        repo_dir: deep,
        ..context.clone()
    };
    pointer("claude-below", Some(&here), None);
    if let Err(ToolFailure::Refused(refusal)) = run_tool(
        "release",
        &json!({"issue": 12, "run_id": "claude-below"}),
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
    // One issue at a time is enforced by reading what this run already holds.
    // Read as `session::load(..).issue`, an unreadable pointer answered `None`
    // — so the rule did not run at all: the agent could swear to a second
    // issue while what it holds is unknown, and the pointer it then wrote would
    // take the first one's trace with it.
    //
    // The gate already refuses this state for writes. Swearing is where the
    // state is *made*, which makes it the earlier place to refuse.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::write(
        context.state_root.join("claude-torn0.json"),
        "{\"run_id\": \"claude-torn0\", \"issue\":",
    )
    .expect("a half-written pointer");

    let failure = run_tool(
        "claim",
        &json!({"issue": 7, "run_id": "claude-torn0", "horizon": "2026-12-31T23:00Z"}),
        Ok(&context),
    )
    .expect_err("a run whose pointer will not parse swore to another issue");

    match failure {
        ToolFailure::Refused(refusal) => {
            assert_eq!(refusal.code, "run-pointer-unreadable");
            // No command, for the same reason the gate names none: the missing
            // fact is which issue, and only the tracker has it.
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
    assert_eq!(response["capabilities"]["tools"]["listChanged"], false);
}

#[test]
fn an_unknown_protocol_version_gets_this_server_s_own_rather_than_an_echo() {
    // Echoing a version back unchecked tells a client it is talking to
    // something that speaks its revision. This server speaks one.
    //
    // This test and the one above have always asserted the same outcome,
    // because there is only one — while the doc over `initialize` described a
    // negotiation and the code under it was an `if` whose arms were the same
    // string. The behaviour was right and the sentence was not.
    let response = result(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "1999-01-01"}
    }));
    assert_eq!(response["protocolVersion"], PROTOCOL_VERSION);

    // And a request with no version at all is still answered, rather than
    // refused or answered with nothing: `initialize` reads it only to say
    // something about it.
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
    // JSON-RPC: a message with no id gets no response at all. A client that
    // receives one has to decide what to do with it.
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
}

#[test]
fn a_read_only_tool_says_so_in_its_schema() {
    // An agent must be able to tell a read from a write without reading our
    // source. `verify_claim` is safe to call speculatively; `claim` is not.
    let read_only = tools::find("verify_claim").expect("verify_claim exists");
    let writing = tools::find("claim").expect("claim exists");
    assert_eq!(read_only.schema()["annotations"]["readOnlyHint"], true);
    assert_eq!(writing.schema()["annotations"]["readOnlyHint"], false);
}

#[test]
fn every_required_operation_is_exposed_or_declared() {
    // The seam that matters most for a harness: the contract's MUST-map list
    // against the tools an agent can actually reach. An operation that is
    // neither exposed nor declared is one the agent will look for and not find.
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
    }
}

#[test]
fn every_tool_maps_to_a_transport_operation_the_binding_documents() {
    // The other half: a tool whose operation the binding never names is a tool
    // that will fail at the first call.
    let binding = crate::skill::FILES
        .iter()
        .find(|file| file.path == "bindings/github.md")
        .expect("the GitHub binding ships");
    for tool in TOOLS {
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
    // The direction the first guard did not cover, and the expensive one. The
    // other test asks "does every tool map to something real"; this asks "is
    // every real thing reachable". A transport operation nobody exposes is one
    // the agent has to compose by hand — which is exactly what the binding says
    // not to do: *"Run executable reversible operations instead of
    // reconstructing them."*
    let transport = transport_source();

    // The subcommands the transport's own dispatcher answers to.
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
    // A `contract_name` nobody can find in the binding is a name Estigia
    // invented, and an invented name makes the two seam guards above agree with
    // each other while agreeing with nothing real.
    let binding = crate::skill::FILES
        .iter()
        .find(|file| file.path == "bindings/github.md")
        .expect("the GitHub binding ships");
    for tool in TOOLS {
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

#[test]
fn no_two_tools_answer_to_the_same_name() {
    let mut names = TOOLS.iter().map(|tool| tool.name).collect::<Vec<_>>();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len());
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
    // Not a JSON-RPC error: the agent asked a fair question, and a protocol
    // code is not something it can act on. It gets the resolution instead.
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

    // Three requests, one notification, three answers — in order.
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
    // The transport documents `claim` as "fresh 32 lowercase hex chars; reuse on
    // retry". A fresh key on a retry makes it a second claim event.
    let root = tempfile::tempdir().expect("a temporary root");
    let first = session::mint_operation_id("claude-abcd1234", 12);
    assert_eq!(first.len(), 32);
    assert!(
        first
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    );

    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(12);
    run.operation_id = Some(first.clone());
    session::store(root.path(), &run).expect("the pointer writes");
    assert_eq!(
        session::load(root.path(), "claude-abcd1234").operation_id,
        Some(first)
    );
}

#[test]
fn a_run_that_already_holds_an_issue_cannot_swear_to_a_second() {
    // Incident I02: a five-minute dev loop re-entered selection while already
    // holding an issue and nearly started additional work on every tick. The
    // contract answers it in prose, and prose is what the loop had.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(12);
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
    // The transport dedupes it on the operation id this run already minted, so
    // refusing it here would break the documented retry.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(12);
    session::store(&context.state_root, &run).expect("the pointer writes");

    let failure = run_tool(
        "claim",
        &json!({"issue": 12, "run_id": "claude-abcd1234", "horizon": "2026-08-01T18:00Z"}),
        Ok(&context),
    )
    .expect_err("there is no transport in this sandbox");
    // It still fails — but on the transport's absence, not on holding the issue
    // it is renewing.
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
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let failure = run_tool(
        "claim",
        &json!({"issue": 34, "run_id": "claude-fresh000", "horizon": "2026-08-01T18:00Z"}),
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
    // `transition` does not take `run_id` as a transport flag, so it used to
    // reach the pointer code with an empty one: the issue moved, a pointer file
    // named for nobody recorded it, and the run that made the move went on
    // believing the old state — which is exactly what `verify_claim
    // --expect-state` is measured against afterwards.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
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
    // The other half of `a_boolean_argument_is_one_the_transport_takes_without
    // _a_value`: that one says the two declarations agree, this one says the
    // command line actually built agrees with them.
    let tool = tools::find("audit_board").expect("a declared tool");

    let set = super::flags_for(tool, &json!({"fix": true})).expect("well formed");
    assert_eq!(
        set,
        vec!["--fix".to_owned()],
        "a boolean was rendered with a value the transport's parser rejects"
    );

    // Absent, not `--fix false`: `store_true` has no way to be told no other
    // than not being told at all, and `--fix false` both errors *and* would
    // have switched repair on if it had not.
    let clear = super::flags_for(tool, &json!({"fix": false})).expect("well formed");
    assert!(
        clear.is_empty(),
        "declining a boolean still sent something: {clear:?}"
    );
    let missing = super::flags_for(tool, &json!({})).expect("well formed");
    assert!(missing.is_empty(), "{missing:?}");

    // And a boolean given something that is not one is malformed here, where
    // the agent is told what to fix, rather than an argparse error later.
    assert!(matches!(
        super::flags_for(tool, &json!({"fix": "yes"})),
        Err(ToolFailure::Malformed(_))
    ));

    // A string argument is unaffected: flag and value, as before.
    let closing = tools::find("check_closing_keywords").expect("a declared tool");
    let sent =
        super::flags_for(closing, &json!({"issue": 12, "base": "main"})).expect("well formed");
    assert!(
        sent.windows(2).any(|pair| pair == ["--base", "main"]),
        "a string argument stopped carrying its value: {sent:?}"
    );
}

/// Whether one transport subcommand changes the filesystem, read out of its own
/// body.
///
/// `mkdir` counts. `readOnlyHint` is the claim that a tool "does not modify its
/// environment", and a directory appearing is a modification — `start-branch`
/// makes the parent of a worktree that way, and already says `writes: true`.
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
        // A worktree is a directory this run did not have before, and `git
        // worktree add` is how the port makes one. Reading only the standard
        // library's writers made `start-branch` — the operation that puts a
        // whole checkout on disk — look read-only.
        "\"worktree\"",
        // And the crate's own writer. `changelog_notes` puts its `--out` file
        // down through `replace_atomically`, which is the only correct way to
        // write a file here and names none of the calls above — so the one
        // operation that took a path to write to read as read-only.
        "replace_atomically(",
    ]
    .iter()
    .any(|call| body.contains(call))
}

#[test]
fn a_tool_that_changes_the_filesystem_is_not_advertised_as_read_only() {
    // `writes` is not read by anything in Estigia — it becomes `readOnlyHint`
    // in the schema, which is the annotation an MCP client reads to decide
    // whether it may run a tool **without asking anybody**. And no MCP tool is
    // on the gate's path: these are Estigia's own tools, so `classify_with`
    // never sees them. The client's prompt is the only thing between an agent
    // and the disk, and a wrong hint is what talks the client out of it.
    //
    // `changelog_notes` said read-only and its `out` argument does
    // `Path(out).write_text(...)` at a path the caller chooses. The binding's
    // own table said "read-only" on the same row that documents `--out <f>`.
    //
    // Derived from the transport rather than listed here, so the day an
    // operation starts writing a file the tool that runs it has to say so.
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
    // The floor: nothing found is not the same as nothing wrong. Two operations
    // touch the disk — `changelog-notes` writes the notes, `start-branch` makes
    // a worktree's parent — and this reading 1 is how the first version of this
    // scan and the throwaway one that found the defect were caught disagreeing.
    assert!(
        writers >= 2,
        "only {writers} operation(s) were seen to touch the disk, so this checked almost nothing"
    );
}

#[test]
fn bookkeeping_arguments_never_reach_the_transport() {
    // A flag the transport does not know makes it reject the whole call, so an
    // argument that exists only to move the run pointer must be filtered — and
    // the list of them is data, not a condition buried in the loop.
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
fn a_tracker_with_no_executable_refuses_instead_of_running_the_wrong_one() {
    // `Tracker` accepts `linear` and `trello`, and neither ships an executable.
    // Reaching for `scripts/github.py` because it is the one that exists would
    // issue `gh` calls to a tracker that is not there and answer them as though
    // they were about it — the nearest named state, told with confidence.
    let root = tempfile::tempdir().expect("a temporary root");
    for tracker in [
        crate::config::Tracker::Linear,
        crate::config::Tracker::Trello,
    ] {
        let context = GateContext {
            integration: crate::config::Integration::Branch,
            flag: None,
            stand_down: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: tracker.clone(),
            boundaries: Vec::new(),
        };
        let failure = run_tool(
            "verify_claim",
            &json!({"issue": 12, "run_id": "claude-abcd1234", "expect_state": "in-progress"}),
            Ok(&context),
        )
        .expect_err("a tracker with no executable is refused");
        let ToolFailure::Refused(refusal) = failure else {
            panic!("a well-formed call was reported malformed");
        };
        assert_eq!(refusal.code, "tracker-has-no-transport");
        assert!(refusal.outcome.is_clean(), "nothing was written");
        assert!(
            refusal
                .to_string()
                .contains("estigia config set Tracker github"),
            "{refusal}"
        );
    }
}

#[test]
fn every_tracker_either_has_a_transport_that_ships_or_none_at_all() {
    // The seam under the refusal: a tracker that names an executable Estigia
    // does not install would refuse at the first call instead of the first
    // configuration.
    for tracker in [
        crate::config::Tracker::Github { repo: None },
        crate::config::Tracker::Linear,
        crate::config::Tracker::Trello,
    ] {
        if let Some(transport) = tracker.transport() {
            // A tracker that can be operated names **where** its operations are
            // answered, and that stopped being a path in the payload when the
            // transport was retired: they are answered by this binary. What the
            // check still has to catch is a tracker claiming an implementation
            // that does not exist, so it asks whether anything answers it.
            assert!(
                transport == "estigia"
                    || crate::skill::FILES
                        .iter()
                        .any(|file| file.path == transport),
                "{tracker:?} names {transport}, which is not installed"
            );
        }
    }
}

#[test]
fn a_transition_cannot_be_made_without_saying_whose_run_it_is() {
    // Without `run_id` the pointer cannot follow the move: the run goes on
    // believing the old state, every later `verify_claim --expect-state`
    // measures against a state the issue has left, and every write after the
    // transition is refused with `unexpected-state` — with nothing saying why.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
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
    // `POINTER_ONLY` says an argument is not a transport flag. It says nothing
    // about whether the caller may omit it, and for `transition` omitting it is
    // the trap above.
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
    // The defect this replaced: the CLI rebuilt every tool refusal with a fixed
    // `NotStarted` / `StatusRequired`, so `estigia claim --json` reported that
    // nothing was written for an ambiguous write. The taxonomy was correct one
    // layer down and fabricated one layer up.
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

    // And a `ToolFailure` carries it whole rather than flattening it to text.
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
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let old_receipt = crate::transport::claim::ReviewReceipt {
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
            run.review_receipt = Some(old_receipt.clone());
            run.reviewed_head = Some("e".repeat(40));
            crate::harness::session::store(&context.state_root, &run)
                .expect("the old authority writes");

            let failure = super::refusal_from_answer(tool, &answer, &mut run, &context)
                .expect("the dispatch answer is a refusal");
            assert!(matches!(failure, ToolFailure::Refused(_)));

            let stored = crate::harness::session::load(&context.state_root, &run_id);
            if invalidates {
                assert_eq!(stored.review_receipt, None, "{name} {case}");
                assert_eq!(stored.reviewed_head, None, "{name} {case}");
            } else {
                assert_eq!(
                    stored.review_receipt.as_ref(),
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
    // One is the caller's own defect and the other is the world's answer, and a
    // caller acts on them differently: the first is fixed by changing the call,
    // the second never is.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Linear,
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
            Ok(&context)
        ),
        Err(ToolFailure::Refused(_))
    ));
}

/// Every JSON-RPC invariant a client is entitled to assume.
///
/// The honesty contract said the server *"has never met a real client"*. This
/// is the part of that gap that can be closed without one: a client's parser
/// rejects a malformed envelope before any of this server's content matters,
/// and the rules are few enough to check exhaustively.
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
        // An id of null is legal, and distinct from a notification.
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
    // The one case where the id cannot be echoed: the request never parsed, so
    // there was no id to read. The specification says `null`, and a client that
    // matches responses to requests by id depends on it being there at all.
    let response =
        handle_line("{not json", Err(&crate::skill::no_skill_root())).expect("an answer");
    assert_envelope(&response, &Value::Null);
    assert_eq!(response["error"]["code"], code::PARSE_ERROR);
}

#[test]
fn every_tool_result_is_shaped_the_way_a_client_reads_it() {
    // `content` is an array of typed blocks, and `isError` is a boolean. A
    // client renders the first and branches on the second; getting either shape
    // wrong makes the answer unreadable however correct its text is.
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

/// The flags one transport subcommand accepts, and which of them it demands.
///
/// Read out of the transport's own `argparse` calls rather than from a list
/// beside them. A copy would be the third place this fact lives.
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

/// The flags one transport subcommand takes **without a value**.
///
/// Read out of the same `argparse` calls as [`transport_flags`], for the same
/// reason: a list beside them would be a second place this fact lives, and the
/// two would drift.
fn transport_valueless(operation: &str) -> Vec<String> {
    transport_reads(operation)
        .into_iter()
        .filter(|(_, how)| *how == "on")
        .map(|(flag, _)| flag)
        .collect()
}
#[test]
fn a_boolean_argument_is_one_the_transport_takes_without_a_value() {
    // `every_tool_sends_flags_the_transport_accepts` crosses the flag **names**
    // against that parser. It does not cross their shapes, and that is where
    // the one boolean argument this server has was lost: `audit_board`'s `fix`
    // rendered as `--fix true`, the transport declares it `store_true`, and
    // argparse answered `unrecognized arguments: true` — the whole call
    // rejected, for `false` exactly as for `true`. The parameter could not be
    // used in either of its settings, and the agent read an argparse error as a
    // configuration defect.
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
    // Both floors, because either being zero means this crossed nothing: no
    // boolean declared here, or none found over there.
    assert!(booleans > 0, "no tool declares a boolean argument");
    assert!(
        valueless_seen > 0,
        "no valueless flag was read out of the transport, so the shapes were never compared"
    );
}

/// The flags one operation's parser constrains to a fixed vocabulary.
///
/// The fourth dimension of the axis the crossings below measure: names, then
/// shapes, then whether the transport will run without a flag, and now which
/// words it takes. Six flags carry `choices=STATES` and the schema published
/// none of them, so an agent was told `to` is a string and left to guess
/// between `in-progress`, `in progress` and `doing`.
/// The flags whose value has to be one of the workflow states, by operation.
///
/// Measured off `github.py`'s `choices=STATES` while that file was here, and
/// written down because it is gone. The port does not spell the constraint in
/// one place — it reaches each state through the code that acts on it — so
/// there is nothing to read it back out of, and a list nobody can regenerate
/// has to say so rather than look derived.
///
/// What it still does: a tool that publishes `to` as a free string, after
/// somebody removed the enum, goes red. Six flags carry this and the schema
/// published none of them, so an agent was told `to` is a string and left to
/// guess between `in-progress`, `in progress` and `doing`.
const CONSTRAINED_TO_THE_STATES: &[(&str, &str)] = &[
    ("list-state", "state"),
    ("verify-claim", "expect-state"),
    ("transition", "to"),
    ("transition", "from"),
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

/// A flag the transport constrains to the workflow states publishes them.
///
/// Both ways round, like the shape crossings. A vocabulary the transport
/// enforces and the schema does not publish is one the agent guesses at, and
/// every wrong guess is argparse refusing a whole call from two processes away
/// — on `transition`, which is the tool that moves work through the workflow.
///
/// The other direction matters too: a schema that constrains what the transport
/// does not is a server refusing a value that would have worked.
#[test]
fn a_flag_constrained_to_the_workflow_states_publishes_them() {
    let mut published = 0;
    let mut found = 0;
    for tool in crate::harness::mcp::tools::TOOLS {
        let constrained = transport_constrained(tool.operation);
        found += constrained.len();
        for argument in tool.arguments {
            // Bookkeeping never reaches that parser, so it has no opinion about
            // the word. `claim`'s own is asserted below, on its own terms:
            // being kept here does not make it less of a state.
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
            // One direction only, and the scan is why. A flag constrained to
            // something *other* than the states — `comment --kind`,
            // `create --state` — carries a list literal rather than the word
            // `STATES`, so this scan cannot see it and asserting on it here
            // would be asserting from a subset. It did: both were published as
            // free strings while the transport refused every value but three,
            // and this test passed throughout.
            //
            // The whole crossing walks the real parser object instead of the
            // source text, and lives where a test may run the interpreter:
            // `every_value_the_transport_constrains_is_one_the_schema_publishes`.
            assert!(
                !takes_states || argument.choices.is_some(),
                "{}: `{flag}` publishes no vocabulary here and is held to the workflow \
                 states in the transport",
                tool.name
            );
        }
    }
    // Both floors: no vocabulary published here, or none read out of there.
    // And the one this server keeps for itself. It never reaches that parser,
    // so the crossing above skips it — and it is still the state the run
    // pointer is moved to, so an agent guessing the word gets a pointer that
    // disagrees with the tracker rather than a refusal.
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

/// The flags one operation's parser will not run without.
///
/// The third dimension of the same axis the two crossings below measure. The
/// flag *names* were crossed, then their *shapes* — `boolean` first and
/// `integer` after it, each having drifted before anybody looked. Whether the
/// transport will run at all without one was never asked, and a flag it
/// requires that nothing sends is a tool that fails every time it is called.
fn transport_required(operation: &str) -> Vec<String> {
    transport_reads(operation)
        .into_iter()
        .filter(|(_, how)| *how == "need" || *how == "number")
        .map(|(flag, _)| flag)
        .collect()
}
/// The flags this server fills in itself, and which operations get them.
///
/// Read out of the module's own source rather than restated: a list written
/// here would be a second copy of the rule, and the copy that goes stale is
/// always the one nothing executes. Each `flags.push("--x")` is attributed to
/// the operations named in the nearest condition above it.
fn server_supplied() -> Vec<(String, String)> {
    let source = include_str!("mod.rs");
    let mut supplied = Vec::new();
    for (at, _) in source.match_indices("flags.push(\"--") {
        let after = &source[at + "flags.push(\"".len()..];
        let Some(close) = after.find('"') else {
            continue;
        };
        let flag = after[..close].to_owned();
        // Backwards to the nearest mention of which operation this is under.
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

/// A flag the transport will not run without is one somebody sends.
///
/// Either the agent, because the schema marks it required, or this server,
/// because it mints the value itself — a fresh operation id, the runtime, the
/// checkout this run reserved. Nothing else sends anything, so a required flag
/// in neither set is a tool that fails argparse on every call it ever receives.
///
/// Both halves are derived: the transport's from its own parser, the server's
/// from this module's source. A list restated in this file would be a second
/// copy of the rule, and it is the copy nothing executes that goes stale.
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
    // Both floors: a parser that read nothing, or a source scan that found
    // nothing, would let every tool through this.
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

/// The flags in one operation's parser that take a **whole number**.
///
/// The sibling of [`transport_valueless`], and it exists for the same reason
/// that one does: the flag *names* were crossed against this parser and the
/// **shapes** were not, so `boolean` drifted first and `integer` was next in
/// line. A shape this server publishes and the transport does not hold is a
/// contract the agent believed and nothing enforced.
fn transport_integers(operation: &str) -> Vec<String> {
    transport_reads(operation)
        .into_iter()
        // Both readings: the one that refuses without a number, and the one
        // that parses a number when it is there. They differ in whether the
        // flag is *required*, and not at all in its shape.
        .filter(|(_, how)| *how == "number" || *how == "int")
        .map(|(flag, _)| flag)
        .collect()
}
/// An argument declared an integer here is one the transport parses as one.
///
/// Both ways round, like the boolean crossing above and for the same reason:
/// this server publishes a schema, and a schema the transport does not hold is
/// a promise the agent acts on and nothing keeps. A flag declared `string` here
/// and `type=int` over there is one an agent may send `"twelve"` to, and what
/// comes back is argparse refusing the whole call from two processes away.
#[test]
fn an_integer_argument_is_one_the_transport_parses_as_a_whole_number() {
    let mut declared = 0;
    let mut found = 0;
    for tool in crate::harness::mcp::tools::TOOLS {
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
    // Both floors: either being zero means this compared nothing at all.
    assert!(declared > 0, "no tool declares an integer argument");
    assert!(
        found > 0,
        "no `type=int` flag was read out of the transport, so the shapes were never compared"
    );
}

#[test]
fn the_flags_read_out_of_the_transport_are_the_ones_it_declares() {
    // The guard on the guard: a parser that finds nothing would let every tool
    // below pass without checking anything.
    let (accepted, required) = transport_flags("claim").expect("claim is a subcommand");
    assert!(
        accepted.contains(&"--operation-id".to_owned()),
        "{accepted:?}"
    );
    assert!(required.contains(&"--horizon".to_owned()), "{required:?}");
    assert_eq!(accepted.len(), 5, "{accepted:?}");

    let (_, optional) = transport_flags("transition").expect("transition is a subcommand");
    assert!(
        !optional.contains(&"--from".to_owned()),
        "`--from` is optional and was read as required"
    );
    assert!(transport_flags("no-such-operation").is_none());
}

#[test]
fn every_tool_sends_flags_the_transport_accepts() {
    // The whole path, not its ends. Argument validation is tested, and the
    // response shapes are tested, and between them sits a flag list built by
    // string substitution — where a wrong name is an argparse error the agent
    // reads as a configuration defect, and a missing one is a call that never
    // had a chance.
    //
    // This is the same defect shape as five gates that registered and decided
    // nothing: each end verified, the path between them assumed.
    for tool in TOOLS {
        let Some((accepted, required)) = transport_flags(tool.operation) else {
            panic!(
                "{} runs `{}`, which the transport does not declare",
                tool.name, tool.operation
            );
        };

        // Estigia adds these itself, after the declared arguments.
        let added: &[&str] = match tool.operation {
            "claim" | "reclaim" | "unassign" => &["--operation-id", "--runtime"],
            "handoff-review" => &["--operation-id", "--runtime"],
            "review-finding" | "review-verdict" => &["--operation-id"],
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
            // Present in the list is not enough, and this half was missing. An
            // absent argument that is not `required` is *skipped* when the
            // command line is built — `continue`, no flag — so a flag the
            // transport requires and this tool offers as optional is a call
            // that argparse rejects the moment an agent takes the schema at its
            // word and leaves it out. The loop above only proves the argument
            // exists somewhere in the declaration.
            //
            // Latent when it was written: no tool declares one that way today.
            // A trap set and not yet stepped on is still a trap, and the agent
            // stepping on it reads `the following arguments are required` as a
            // defect in its own call.
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
    // The fourth instance of the same shape: `UNIMPLEMENTED` is a hand-written
    // list of what this server does not do, and nothing checked that it does
    // not do it. An entry that stays after somebody implements the thing turns
    // the honesty contract into the one kind of document worse than none.
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
fn the_declared_tool_count_is_the_number_of_tools() {
    // The server advertises no `listChanged`, which is only honest because the
    // list is a compile-time constant. If it ever stops being one, that entry
    // in UNIMPLEMENTED is the thing that has to change first.
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
    // rule took that as "no id, say nothing".
    //
    // A client that sent an id and gets silence waits forever. Of every
    // wrong answer available, that is the only one it cannot recover from.
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

    // And a real notification is still silent, which is the rule this was
    // borrowing from.
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
    // `release` could only ever make the *discovery* call. The transport
    // answers it `write_performed: false` and says to repeat naming the epoch,
    // and the tool had no argument to name it with — so the release the whole
    // tool exists for could not be reached through this server. `reclaim`, one
    // entry above, has had the argument all along.
    for name in ["release", "reclaim"] {
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
    // The discovery call forgot the pointer: the issue, the worktree the gate
    // watches and the last verification all went, while the tracker still
    // showed the run holding the issue and no release had been written at all.
    // A run whose worktree the gate has forgotten delivers through it ungated.
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let release = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "release")
        .expect("the tool is listed");

    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(7);
    run.worktree = Some(std::path::PathBuf::from("/w/7"));
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    // Step one: read-only, and it says so.
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
        Some(7),
        "a call that wrote nothing took the issue"
    );
    assert!(
        after.worktree.is_some(),
        "a call that wrote nothing took the worktree the gate watches"
    );

    // Step two really releases, and then the pointer goes.
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

/// Isolating a run must not *narrow* what its claim covers.
///
/// `docs/honesty.md` states the invariant: *"The gate covers two directories:
/// the checkout the claim was made in, and the isolated one `start_branch`
/// created."* `Isolated` wrote only the second, which is right whenever the
/// first is already there and wrong in the one shape that is reachable without
/// anybody doing anything strange: a `claim` whose tracker write **landed** and
/// whose readback failed returns `Err` before `apply_effect` ever runs, so the
/// pointer records no `repo_dir` at all.
///
/// From there the dispatch guard's own precondition — `covered().count() > 0` —
/// stands aside, `start_branch` succeeds, and the run goes from covering
/// *everywhere* to covering exactly one directory that is not the one the server
/// is standing in. Every later call is refused `run-id-names-another-checkout`,
/// and there is no way back that does not restart the agent from a path the
/// workflow just created. Measured on this repository during the GitHub outage
/// of 2026-08-17, and twice more by the run that filed the issue.
///
/// So the isolation step records the checkout it was adjudicated from.
/// `start_branch` verifies the claim against the tracker from `context.repo_dir`
/// before it creates anything, which is the same warrant `Swear` records that
/// directory on — this is not coverage manufactured from a client's path.
#[test]
fn isolating_a_run_records_the_checkout_it_was_adjudicated_from() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
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

    // The shape a claim whose readback failed leaves behind: the tracker holds
    // the claim, the pointer holds no checkout.
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(56);
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

    // The floor, because a rule that overwrites would pass the assertion above
    // and take the guarantee with it. A run whose claim recorded checkout A must
    // not have that replaced by a server standing in B — the dispatch guard
    // refuses B for that run, and this is the line that keeps it able to.
    let elsewhere = root.path().join("somebody-elses-checkout");
    let mut theirs = crate::harness::session::Run::new("claude-elsewhere".to_owned());
    theirs.issue = Some(56);
    theirs.repo_dir = Some(elsewhere.clone());
    crate::harness::session::store(&context.state_root, &theirs).expect("the pointer writes");
    super::apply_effect(
        start,
        &serde_json::json!({"issue": 56, "run_id": "claude-elsewhere"}),
        Some(&serde_json::json!({
            "ok": true,
            "worktree": worktree.display().to_string(),
        })),
        &mut theirs,
        &context,
    );
    assert_eq!(
        crate::harness::session::load(&context.state_root, "claude-elsewhere").repo_dir,
        Some(elsewhere),
        "isolation overwrote the checkout the claim was made in with the server's own"
    );
}

/// A renewal repairs the record it was measured against.
///
/// The cure for a run that is *already* stranded, which the entry above only
/// prevents. Recording the claim's checkout at isolation stops new runs reaching
/// the state; it does nothing for the ones in it, and there were several — the
/// issue this was filed under names two, and the run that fixed it made a third.
///
/// A renewal answered `ok` is the tracker saying, at that moment, that this run
/// is the live holder of this issue in this state. That is the same fact `Swear`
/// writes, from the same authority — so the pointer is completed from it. It
/// costs no tracker *write*, which is what makes it reachable during the outage
/// that causes the damage in the first place.
///
/// Filled and never overwritten, and unable to invent anything: a run that does
/// not hold the issue is refused by the tracker long before the pointer is
/// touched.
#[test]
fn a_renewal_completes_a_record_the_tracker_has_just_agreed_with() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
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

    // Stranded: the timeline holds the claim, the record holds a worktree and
    // nothing that says where the claim was sworn.
    let worktree = root.path().join("trees").join("issue-56");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.worktree = Some(worktree.clone());
    crate::harness::session::store(&context.state_root, &run).expect("the pointer writes");

    // `review`, and deliberately not `in-progress`: the defaulted value beside
    // `named_state` is `in-progress`, so a fixture using it cannot tell the state
    // being *read* from the state being *guessed*. Measured — with this at
    // `in-progress`, deleting the `expect_state` read left the whole suite green,
    // which is this repository's own definition of an untested line.
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
        Some(56),
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

    // And it completes rather than replaces. A record that already names an
    // issue, a state and a checkout is describing something this call has no
    // business rewriting — `verify_claim` refuses outright when the tracker
    // disagrees with it, so reaching here is not a licence to overrule it.
    let elsewhere = root.path().join("another-checkout");
    let mut held = crate::harness::session::Run::new("claude-elsewhere".to_owned());
    held.issue = Some(12);
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
    let after = crate::harness::session::load(&context.state_root, "claude-elsewhere");
    assert_eq!(
        (after.issue, after.state.as_deref(), after.repo_dir),
        (Some(12), Some("review"), Some(elsewhere)),
        "a renewal overwrote a record that already said what it held"
    );

    // And a renewal that carries no state writes none. Two of the four renewing
    // tools take a receipt and never a state — the defaulted value beside
    // `named_state` is not something the tracker said on their call, and a
    // pointer stamped with it is announced as fact by `hook::state_clause` and
    // printed by `estigia status` where it printed `unknown`. Fill-never-
    // overwrite makes the guess permanent, so the later `verify_claim` that names
    // the real state cannot take it back.
    for silent in ["release_ci", "record_review_verdict"] {
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
        // The rest of the completion still happens: the fault is the guess, not
        // the repair.
        assert_eq!(after.issue, Some(56), "{silent} completed nothing at all");
    }
}

#[test]
fn every_receipt_effect_recovers_the_complete_receipt_atomically() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let receipt = crate::transport::claim::ReviewReceipt {
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

    for name in [
        "publish_review",
        "republish_review",
        "record_review_verdict",
        "release_ci",
    ] {
        let tool = super::tools::TOOLS
            .iter()
            .find(|tool| tool.name == name)
            .expect("the tool is listed");
        let mut run = crate::harness::session::Run::new(format!("claude-{name}"));
        let arguments = if matches!(name, "record_review_verdict" | "release_ci") {
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
            run.review_receipt.as_ref(),
            Some(&receipt),
            "{name} did not restore all five receipt fields"
        );
        assert_eq!(run.reviewed_head, None);
    }
}

#[test]
fn a_partial_publication_receipt_invalidates_old_local_authority() {
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let old_receipt = crate::transport::claim::ReviewReceipt {
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
        run.review_receipt = Some(old_receipt.clone());
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

        assert_eq!(run.review_receipt, None, "{name} retained an old receipt");
        assert_eq!(run.reviewed_head, None, "{name} retained a legacy head");
    }
}

#[test]
fn every_two_step_operation_has_a_tool_that_can_take_the_second_step() {
    // `every_tool_maps_to_a_transport_operation_the_binding_documents` already
    // crosses the flags — but only the ones argparse calls **required**, and the
    // flag that carries a two-step operation's second call is optional there:
    // it has to be absent on the first. So `release` could send every required
    // flag, pass that check, and still never reach the write it exists for.
    //
    // Read out of the transport rather than listed here, because the population
    // is *which commands answer in two steps* and a list would be a third place
    // to keep in step. It was read out of `github.py`; it is read out of the
    // port, which is the implementation that answers now.
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
    // The other half of the same door, and the worse half. `reclaim` answers
    // its read-only first call `write_performed: false` too, and its effect is
    // `Swear` — so that call used to set the issue, the state and the
    // verification stamp the renewal window rides on. A run that had taken over
    // nothing read as the verified holder of an issue somebody else still held,
    // and its repository writes went through the gate on it.
    let root = tempfile::tempdir().expect("a state root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    };
    let reclaim = super::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "reclaim")
        .expect("the tool is listed");
    assert_eq!(
        reclaim.effect,
        super::tools::PointerEffect::Swear,
        "this test is about what `Swear` on a discovery call would do"
    );

    let fresh = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    crate::harness::session::store(&context.state_root, &fresh).expect("the pointer writes");
    // Reloaded, because that is what the call does before it applies anything:
    // a `Run` held from before the store is a revision behind, and its own
    // write would lose to itself and hide whatever this is measuring.
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
fn no_tool_bound_to_a_run_accepts_the_name_that_names_none() {
    // `<runtime>-unknown` is what a session with no identity is called, and it
    // is the same string for every such session on the machine. The first cut
    // of this guard sat inside the `Swear` branch — so `claim` was refused and
    // `release` walked past it **to the tracker**, measured, failing only
    // because that fixture had no credentials. A release under a name no run
    // owns takes an issue away from whoever actually holds it.
    //
    // Derived from each tool's own arguments rather than from a list here, so a
    // tool that starts taking a `run_id` is covered the day it does.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
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
        // Every other required argument filled, so the refusal below is this
        // rule and not a missing field.
        let mut arguments = serde_json::Map::new();
        for argument in tool.arguments.iter().filter(|a| a.required) {
            arguments.insert(
                argument.name.to_owned(),
                match argument.json_type {
                    "integer" => json!(12),
                    "boolean" => json!(true),
                    // A value the argument actually takes. `"x"` is not a
                    // workflow state, and filling one in made these tests pose
                    // a malformed call — which the rule under test never
                    // reaches, so they would have measured the argument check
                    // instead of themselves.
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

    // And a run id that names a run is not refused **by this rule**. It goes on
    // and fails for its own reasons — there is no tracker here — which is what
    // makes the assertion above about the name rather than about anything else.
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
    // The refusal that stops a run with no identity from acting has to answer
    // **two** situations, and it only answered one.
    //
    // A live session asking for an identity is sent to `status`, correctly. But
    // a pointer already written under that name is an orphan: nothing will ever
    // release it, because this refusal is what stops that — and it declares a
    // checkout, so the guard gates every write in there against an issue nobody
    // holds. `status` prints that run and says *`estigia release --run-id <id>`
    // puts one down*; this answered by sending the operator back to `status`.
    // Two commands pointing at each other, and the ratchet's one rule is that a
    // message may name a way out only when taking it discharges the block.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        flag: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().join("repo"),
        state_root: root.path().join("state"),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
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

    // No pointer: the live session, told where identities come from.
    let asking = refusal(&context);
    assert_eq!(asking.code, "run-id-names-no-run");
    assert!(
        format!("{asking}").contains("status"),
        "a session with no identity was not told where to get one: {asking}"
    );

    // A pointer under that name: the orphan, and the file is the way out.
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
    // And not sent back to the command that sent them here.
    assert!(
        !said.contains("estigia status"),
        "the way out is the command that names this one: {said}"
    );
    // The tracker's side stays the tracker's: this name is not evidence about
    // who holds the issue there.
    assert!(
        said.contains("tracker"),
        "removing the pointer was made to sound like releasing the claim: {said}"
    );

    // Taking it discharges the block, which is the whole rule.
    std::fs::remove_file(&orphan).expect("the operator takes it away");
    assert!(
        !format!("{}", refusal(&context)).contains(&orphan.display().to_string()),
        "the way out was taken and the message still names it"
    );
}

/// A publish names the checkout this run works in, so both ends read one head.
///
/// The gate refuses a delivery whose recorded review head is not this
/// checkout's. `publish-review` reads the head it records from `--worktree`
/// **or, failing that, its own working directory**, and that argument is one
/// the *caller* supplies — so a run with an isolated checkout that published
/// without naming it bound the review to whatever directory the server happened
/// to be in, and every later delivery was refused for a verdict that had never
/// gone stale.
///
/// That is the worst failure a gate has: the agent is stopped and nothing it
/// can do helps, because the mismatch belongs to the harness. Found by auditing
/// the round that added the refusal rather than by it going wrong.
#[test]
fn a_publish_names_the_checkout_the_gate_will_measure_against() {
    let tool = tools::find("publish_review").expect("the tool exists");
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());

    // No isolated checkout: nothing to name, and the transport's own default
    // is the working directory both ends already share.
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

    // Named by the caller: theirs wins. Estigia adding its own on top would be
    // deciding where somebody else's work lives.
    let theirs = vec!["--worktree".to_owned(), "/w/somewhere-else".to_owned()];
    assert!(
        worktree_to_name(tool, &theirs, &run).is_none(),
        "Estigia named a second worktree over the one the caller chose"
    );

    // Release re-derives the same target from the same checkout, and a republish
    // derives it the same way — after a force-push, which is why it is asserted
    // here by name rather than left to whoever adds the next operation. It was
    // left out: refs are shared between git worktrees, so the branch pushed from
    // the base checkout anyway and the readback then disagreed with a head this
    // harness had chosen, answering that somebody else must have pushed.
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

    // Other operations do not derive a review target.
    for other in ["claim", "transition", "heartbeat"] {
        let tool = tools::find(other).expect("the tool exists");
        assert!(
            worktree_to_name(tool, &[], &run).is_none(),
            "{other} was handed a worktree it never asked for"
        );
    }
}

/// One epoch, one shape, wherever it is asked for.
///
/// `reclaim` and `release` both take the holder's operation epoch and both
/// compare it against the same `ownership_epoch` — which answers the holder's
/// 32 hex characters, or `legacy-<id>` for a claim made before there were any.
/// `release` declared it a string and `reclaim` declared it an integer: one
/// rule in two places, disagreeing, which is the shape this crate keeps
/// finding.
///
/// What the disagreement cost is not cosmetic. An agent that believed the
/// published schema sent a number, and a number can never equal that epoch — so
/// the second call of every `reclaim`, the one that actually takes the claim
/// over, answered `target-operation-mismatch` forever.
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

/// A tool that moves the run pointer is a tool that writes.
///
/// Two declarations of one fact sit on every row of that table: what a success
/// does to the pointer, and whether the call writes at all. The second is
/// published in the schema as `readOnlyHint`, which an agent reads to decide
/// whether a call is safe to make speculatively or to retry.
///
/// They cannot disagree in one direction: a pointer that now says this run
/// holds an issue, or has put one down, or has been given a checkout, is a
/// pointer that moved because the tracker did. A row claiming `readOnlyHint`
/// beside such an effect would invite an agent to make that call twice.
///
/// The other direction is open on purpose — `comment` and `create` write and
/// leave the pointer alone.
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
    // The floor: a table where nothing moved the pointer would pass in silence.
    assert!(
        moved >= 5,
        "only {moved} tools move the pointer, so this crossed almost nothing"
    );
}

/// A whole number is not yet an issue number.
///
/// The value is rendered straight into the transport's argv, where a negative
/// one is read as a **flag**. Measured through the running server, before this:
///
/// ```text
/// verify_claim {"issue": -5, …}
///   -> gh issue view failed (1): unknown shorthand flag: '5' in -5
/// ```
///
/// A wrong argument reported as a broken transport is the wrong cause, and the
/// one an agent retries against — the failure the two notes beside the boolean
/// and integer checks already describe, arriving through a bound nobody
/// published.
#[test]
fn an_integer_that_counts_from_one_is_refused_below_it_and_says_so_in_the_schema() {
    for tool in crate::harness::mcp::tools::TOOLS {
        for argument in tool.arguments {
            if argument.json_type != "integer" {
                continue;
            }
            // Every integer this server takes is a count of something that
            // starts at one — an issue number, a page limit. A new one that is
            // genuinely unbounded is a decision, not an omission, and this is
            // where it gets made.
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

            // Published. An agent that cannot see the bound guesses at it, and
            // a rule the schema does not carry makes the schema a description.
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

    // And enforced, through the same path a call takes. `verify_claim`, because
    // that is the tool the measurement above was taken on.
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

    // The floor: an ordinary issue number still reaches the transport, or the
    // refusals below are about a tool nobody can call.
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
/// `flags_for` walks the **declared** arguments and looks each one up in what
/// the caller sent, so a key the caller sent and the tool never heard of was
/// never examined. A misspelled *required* argument is caught by its own
/// absence; a misspelled *optional* one was caught by nothing, and the call ran
/// without the thing the agent asked for and succeeded.
///
/// `list_state` is the measurement because its optional argument exists to
/// prevent exactly this failure. Its own note: an agent asking what is in
/// `ready` on a busy project *read two hundred issues as the whole queue and had
/// no way to ask for more*, and **a partial answer read as the state is the
/// failure this crate is named for**. Written `limt`, the call went out with the
/// default ceiling and answered as though it were the queue.
#[test]
fn an_argument_a_tool_does_not_take_is_refused_rather_than_dropped() {
    let tool = crate::harness::mcp::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "list_state")
        .expect("`list_state` is a tool this server exposes");

    // The floor, both halves: the spelling it does take reaches the transport,
    // and leaving it out is still fine — this refuses a wrong key, not an
    // absent one.
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

    // And it is the caller's own defect rather than the world's answer — the
    // two shapes `ToolFailure` keeps apart, because a client retries one and
    // not the other.
    assert!(
        matches!(refused, super::ToolFailure::Malformed(_)),
        "a wrong argument was reported as the tracker's answer: {refused:?}"
    );

    // Every tool, not just this one: the rule is about the boundary.
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
        // Whatever this tool makes of a well-formed call, it is not a complaint
        // about an unknown key — or the assertion below is measuring nothing.
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

/// A tool whose operation answers in two calls says how to make the second.
///
/// The transport's own words are the source: an operation with a discovery
/// phase answers `write_performed: false` and a `next` of *"repeat <operation>
/// with --target-operation and the same operation ID"*. There are two, and only
/// one of them said so in the description an agent reads before calling.
///
/// It is the same seam the CLI's `release` fell through one round earlier, where
/// the verb threw the discovery answer away and printed `released: <run> no
/// longer holds #<issue>` over a claim that was still held. A caller that is not
/// told there is a second call does not make it, and what it does instead is
/// believe the first one.
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
/// The crossings here run the other way — a flag the transport *requires* that
/// no tool sends is a tool that fails every call — and that direction was the
/// one that had burned this crate. The reverse had not been asked: a flag the
/// binding's own operations table shows in brackets, that the dispatcher reads,
/// and that no tool declares.
///
/// Three of them, and one is not cosmetic. `reclaim` is documented as
/// `[--force --reason-file <f>]` — the forced takeover, the operation whose
/// whole point is that it is deliberate and leaves a written reason — and the
/// tool declares neither, so an agent following the binding cannot make the
/// call the binding describes. `unassign [--held-by-other]` and `verify_claim
/// [--allow-closed-by-pr <pr>]` were the same shape.
///
/// This is `start_branch --repo-name` again: the contract names a call, the one
/// path an agent has cannot make it, and nothing crossed the two.
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
        // `| `name` | SCRIPT <operation> --flag … |` — the operations table.
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
        // The command cell only. The row's third column explains what the
        // operation does, and it names git's own flags — `git merge-tree
        // --write-tree` is in `base_movement`'s prose — so reading the whole
        // line asks the tools to send flags that were never theirs.
        let command = tail[at..].split('|').next().unwrap_or_default();
        for flag in command
            .split_whitespace()
            .filter(|word| word.starts_with("--"))
        {
            let flag = flag.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
            // What the server supplies for the tool, and the bookkeeping
            // arguments that never become flags.
            if ["--runtime", "--operation-id", "--worktree"].contains(&flag) {
                continue;
            }
            if !sends.iter().any(|sent| sent == flag) {
                missing.push(format!("{name} cannot send {flag}"));
            }
        }
    }

    // The floor: the table was read. A parser that matched nothing agrees with
    // a complete set of tools and never fails.
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
