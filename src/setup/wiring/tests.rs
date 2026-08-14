use super::*;

/// The real thing, copied out of `%APPDATA%\gemini\settings.json` on the
/// machine this was written on — including that the command it registers is a
/// **debug build inside a working tree**, which is the whole reason this module
/// exists.
const GEMINI: &str = r#"{
  "mcpServers": {
    "codegraph": { "command": "codegraph" },
    "leteo": { "command": "npx", "args": ["-y", "leteo"] }
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "H:\\REPO\\estigia\\target\\debug\\estigia.exe hook pre-tool-use --dialect gemini-cli",
            "timeout": 10
          }
        ]
      }
    ]
  }
}"#;

#[test]
fn the_registered_command_is_read_back_out_of_whatever_shape_holds_it() {
    let found = wires(GEMINI);
    assert_eq!(
        found.len(),
        1,
        "one Estigia call, and it was not found: {found:?}"
    );
    let wire = &found[0];

    // The path comes back **unescaped**. Read as text, the JSON gives
    // `H:\\REPO\\...`, which is a path that does not exist — a fault reported on
    // a machine where everything works.
    assert_eq!(
        wire.executable,
        std::path::PathBuf::from(r"H:\REPO\estigia\target\debug\estigia.exe")
    );
    assert_eq!(wire.event, Some("pre-tool-use"));
    assert_eq!(wire.named, "pre-tool-use");

    // Somebody else's servers on the same file are not ours.
    assert!(!wire.command.contains("codegraph"));
}

#[test]
fn a_command_naming_an_event_this_build_lacks_is_reported_rather_than_counted() {
    // The failure this catches: a hook entry that is present, so `status` says
    // `gate on`, and that this binary refuses — which for a `PreToolUse` hook is
    // a *non-blocking* error. The tool call goes through ungated with one line
    // in a transcript nobody reads.
    let file = GEMINI.replace("pre-tool-use", "before-tools");
    let found = wires(&file);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].event, None);
    assert_eq!(found[0].named, "before-tools");
    assert!(!found[0].is_live());
    let fault = found[0].fault().expect("it says what is wrong");
    assert!(fault.contains("before-tools"), "{fault}");
    assert!(fault.contains("ungated"), "{fault}");

    // And the host's own spelling is not a fault: `PreToolUse` is a name this
    // build reads.
    let file = GEMINI.replace("hook pre-tool-use", "hook PreToolUse");
    assert_eq!(wires(&file)[0].event, Some("pre-tool-use"));
}

#[test]
fn an_executable_that_is_not_there_is_a_gate_that_is_off() {
    let root = tempfile::tempdir().expect("a temporary root");
    let real = root.path().join("estigia.exe");
    std::fs::write(&real, "").expect("the executable");

    let live = format!(
        "{{\"hooks\":[{{\"command\":{}}}]}}",
        serde_json::to_string(&format!("{} hook pre-tool-use", real.display())).expect("json")
    );
    let found = wires(&live);
    assert_eq!(found.len(), 1);
    assert!(found[0].is_live(), "a real executable was called missing");
    assert_eq!(found[0].fault(), None);

    // Delete it — which is what `cargo clean`, or moving the checkout, does.
    std::fs::remove_file(&real).expect("gone");
    let found = wires(&live);
    assert!(!found[0].is_live());
    let fault = found[0].fault().expect("it says what is wrong");
    assert!(fault.contains("ungated"), "{fault}");
    assert!(fault.contains("estigia"), "{fault}");
}

#[test]
fn a_path_with_a_space_in_it_is_one_path() {
    // Splitting on whitespace alone reports the executable as its own first
    // directory — a fault that is not there, on a machine where everything
    // works.
    let quoted = r#"{"command": "\"C:\\Program Files\\estigia\\estigia.exe\" hook session-start"}"#;
    let found = wires(quoted);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found[0].executable,
        std::path::PathBuf::from(r"C:\Program Files\estigia\estigia.exe")
    );
    assert_eq!(found[0].event, Some("session-start"));
}

#[test]
fn somebody_elses_hook_is_not_read_as_ours() {
    // These files hold other people's servers and other tools' hooks. Reporting
    // one of theirs as a broken gate of ours sends somebody to fix a file that
    // is not wrong.
    for foreign in [
        r#"{"command": "husky hook pre-tool-use"}"#,
        r#"{"command": "npx some-tool hook pre-push"}"#,
        r#"{"command": "/usr/local/bin/other hook session-start"}"#,
        r#"{"command": "estigia doctor"}"#,
        r#"{"command": "estigia"}"#,
    ] {
        assert!(
            wires(foreign).is_empty(),
            "{foreign} was read as an Estigia gate"
        );
    }

    // A copy somebody renamed and put somewhere of their own still counts: they
    // installed this, and the file name is what says so.
    let mine = r#"{"command": "/opt/bin/estigia-0.1.0 hook pre-tool-use"}"#;
    assert_eq!(wires(mine).len(), 1);
}

#[test]
fn a_file_that_is_not_json_is_still_read() {
    // Two dialects register through a program rather than a document. A reader
    // that only understood JSON would report those as having no gate at all,
    // which is a fault on every machine where they work.
    let plugin = "export const hooks = {\n  async before(input) {\n    \
                  await run('/usr/local/bin/estigia hook pre-tool-use --dialect opencode')\n  }\n}";
    let found = wires(plugin);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].event, Some("pre-tool-use"));

    // And a file with nothing of ours in it yields nothing rather than noise.
    assert!(wires("").is_empty());
    assert!(wires("{}").is_empty());
    assert!(wires("not json, no hooks, nothing").is_empty());
}

#[test]
fn a_plugin_that_names_a_binary_is_wiring_like_any_other() {
    // `wires` reads command lines — `hook` and an event name side by side.
    // OpenCode's plugin is JavaScript and calls `estigia gate <tool> --input
    // <payload>`, so those words are never adjacent and it read as no wiring at
    // all. `doctor` then said, of a file naming a binary:
    //
    //   opencode: gated by its own file rather than a settings entry, so there
    //   is no wiring here to be wrong
    //
    // Measured with both plugin-style gates pointed at a path that is not
    // there: `cline` answered *the gate is registered and would not run*, and
    // `opencode` answered that there was nothing to be wrong.
    let root = tempfile::tempdir().expect("a temporary root");
    let real = root.path().join("estigia.exe");
    std::fs::write(&real, "#!/bin/sh\n").expect("a binary to name");

    let present = crate::setup::plugin::source(&real);
    let wire = plugin_wire(&present).expect("the plugin names a binary");
    assert_eq!(wire.executable, real);
    assert!(wire.fault().is_none(), "{:?}", wire.fault());
    // The path is read back as JSON, the way it was written: trimming quotes
    // would hand back `C:\\Users\\…` on Windows and report a fault that is not
    // there — the mistake `wires` already carries a comment about.
    assert!(
        !wire.executable.display().to_string().contains("\\\\"),
        "the path came back escaped: {}",
        wire.executable.display()
    );

    let gone = crate::setup::plugin::source(&root.path().join("moved-away.exe"));
    let wire = plugin_wire(&gone).expect("it still names one");
    assert!(
        wire.fault().is_some_and(|why| why.contains("not there")),
        "a plugin naming nothing was reported as sound"
    );

    // Somebody else's plugin is not wiring of Estigia's, however it invokes
    // things. The marker is what says whose file it is, here as everywhere.
    assert!(plugin_wire("export const Whatever = async () => {};").is_none());
    assert!(plugin_wire("const ESTIGIA = \"/usr/bin/estigia\";").is_none());

    // And Cline's hook is a command line, so it goes through `wires` and never
    // reaches the fallback: two readers, and the ordinary one stays first.
    let cline = crate::setup::plugin::cline_hook(&real, false);
    assert!(!wires(&cline).is_empty(), "the command-line reader lost it");
}

#[test]
fn the_plugin_reader_is_one_registered_actually_reaches() {
    // The test above measures `plugin_wire`. Taking the fallback out of
    // `registered` left it green — a reader that is written, correct and called
    // by nothing, which is the shape this crate has now found five times. The
    // whole defect was that `registered` returned an empty list for the one
    // agent gated by a plugin.
    //
    // So this goes through the function `doctor` calls, over a real
    // installation, for the two adapters that carry a file of their own.
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        ..crate::setup::SetupOptions::default()
    };
    for slug in ["opencode", "cline"] {
        let adapter = crate::setup::find_agent(slug).expect("a declared agent");
        crate::setup::setup(adapter, &crate::config::Config::default(), &options)
            .expect("setup runs");

        let found = registered(adapter, &options);
        let wires: Vec<&Wire> = found.iter().flat_map(|(_, wires)| wires).collect();
        assert!(
            !wires.is_empty(),
            "{slug} is gated by a file of its own and `registered` found nothing in it"
        );
        // And what it found names this binary, which is what makes the row
        // `ok … running <path>` rather than a sentence about there being
        // nothing to check.
        assert!(
            wires
                .iter()
                .all(|wire| !wire.executable.as_os_str().is_empty()),
            "{slug}: a wire with no executable to check"
        );
    }
}

/// A matcher that wakes for nothing this build gates is named as such.
///
/// `Wire` reads command lines, and a matcher is not one — so the field that
/// decides *which tools* the gate runs for was the one thing `doctor`'s gate
/// row could not see, while its `about` promises *whether the gate this agent
/// registers would actually run*. Measured on the product: a settings file
/// whose matcher had been narrowed by hand to a tool that does not exist
/// reported `ok — 3 live` and would never have fired for a write.
///
/// The quiet cases matter as much: a matcher naming one judged tool is narrow
/// and deliberate, no matcher at all is wide rather than broken, and an entry
/// that is somebody else's is not this check's business.
#[test]
fn a_matcher_that_wakes_for_nothing_this_build_gates_is_reported() {
    const JUDGED: &[&str] = &["edit", "write", "bash", "multiedit"];
    let entry = |matcher: &str, command: &str| {
        format!(
            r#"{{"hooks": {{"PreToolUse": [{{"matcher": "{matcher}",
               "hooks": [{{"type": "command", "command": "{command}"}}]}}]}}}}"#
        )
    };
    let ours = "estigia hook pre-tool-use";

    // Names nothing judged: the gate is there and wakes for no write.
    let narrow = entry("Notepad", ours);
    assert_eq!(
        narrowed(&narrow, JUDGED),
        vec!["Notepad".to_owned()],
        "a matcher that gates nothing was not reported"
    );

    // Names one: narrow on purpose is not broken.
    assert!(narrowed(&entry("Edit", ours), JUDGED).is_empty());
    assert!(narrowed(&entry("Edit|Write|Bash", ours), JUDGED).is_empty());

    // Somebody else's entry, whatever its matcher says.
    assert!(
        narrowed(&entry("Notepad", "their-linter --check"), JUDGED).is_empty(),
        "another tool's matcher was read as Estigia's"
    );

    // No matcher at all is every tool, which is wide rather than wrong.
    let wide = format!(
        r#"{{"hooks": {{"PreToolUse": [{{"hooks": [{{"type": "command", "command": "{ours}"}}]}}]}}}}"#
    );
    assert!(narrowed(&wide, JUDGED).is_empty());

    // And nothing invented out of a file that will not parse.
    assert!(narrowed("{not json", JUDGED).is_empty());
}
