//! The OpenCode adapter: a plugin, because that is what OpenCode reads.
//!
//! Claude Code takes a line in a settings file. OpenCode takes a **module** —
//! JavaScript or TypeScript, loaded at startup from `plugins/`, exporting a
//! named function that returns a hooks object. Its `tool.execute.before` hook
//! blocks a call by throwing, which is a real deny and the reason this adapter
//! exists at all.
//!
//! Written against the published plugin reference: the location, the export
//! shape, the hook signature and the throw-to-block rule all come from there.
//!
//! # Two limits, both worth knowing before switching it on
//!
//! - `tool.execute.before` is reported not to intercept calls made by
//!   **subagents** spawned through the task tool. A gate with a hole is still a
//!   gate, but a gate whose hole nobody mentions is a lie.
//! - The plugin shells out to the binary on every matching call. It runs in Bun,
//!   the call is one short-lived process, and the gate does no network work
//!   unless it has to ask the tracker — but it is not free, and the tools it
//!   matches on are named rather than `*` for exactly that reason.

use std::path::Path;

/// Where OpenCode loads global plugins from, under its config directory.
pub const DIRECTORY: &str = "plugins";

/// The file Estigia writes, and the only one it writes.
pub const FILE: &str = "estigia-workflow-authority.js";

/// The marker that says the file is Estigia's.
pub const MARKER: &str = "estigia:workflow-authority";

/// The plugin source.
///
/// JavaScript rather than TypeScript: both are loaded, and a `.js` file needs no
/// type imports from a package whose version this crate does not control.
///
/// It decides nothing. Everything it knows is how to ask the binary and how to
/// throw when the answer is no — so a decision that changes needs no reinstall,
/// and the plugin API surface it depends on stays as small as it can be.
pub fn source(executable: &Path) -> String {
    // JSON-encoded so a Windows path's backslashes survive into the module.
    let binary = serde_json::to_string(&executable.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "\"estigia\"".to_owned());
    format!(
        r#"// {MARKER} — managed by `estigia setup opencode`
//
// Refuses a repository write that no live claim authorises. Removed by
// `estigia setup opencode --uninstall`.
//
// Known limit: `tool.execute.before` is reported not to intercept tool calls
// made by subagents spawned through the task tool.
const ESTIGIA = {binary};

// The tools a write can arrive through. Named rather than `*`: waking a process
// for every read would be a cost paid thousands of times to answer "not mine".
//
// Crossed against the classifier's own populations by
// `the_plugin_gates_the_tools_the_classifier_judges` — this list is the fourth
// copy of that rule and the only one in another language, so nothing but a test
// can keep it level with them. Every name here has to be one `harness::classify`
// judges, or the plugin wakes a process to be told "not mine".
//
// `notebookedit` and `update` are named on the same asymmetry the Claude Code
// matcher settles: if this host has no such tool the entry wakes for nothing,
// and if it has one, leaving it out is every write through it going ungated —
// past a gate `doctor` reports as live.
const GATED = ["edit", "write", "patch", "multiedit", "notebookedit", "update", "bash"];

export const EstigiaWorkflowAuthority = async ({{ $, directory, worktree }}) => {{
  return {{
    "tool.execute.before": async (input, output) => {{
      const tool = String(input?.tool ?? "").toLowerCase();
      if (!GATED.includes(tool)) return;

      // The run is found by the checkout, because OpenCode's plugin context
      // carries a project and no session identity to mint a run id from.
      const cwd = worktree ?? directory ?? process.cwd();
      const payload = JSON.stringify(output?.args ?? {{}});

      // `.nothrow()`, so the status is read rather than turned into an
      // exception that says only *something went wrong*. Estigia defines three
      // codes and no more — `0` allow, `1` refused, `2` an outcome it could not
      // read back — and only those three are decisions. Anything else means it
      // did not answer: the binary is gone after a `cargo clean` or an
      // uninstall, a shim resolved to nothing, it panicked.
      //
      // This caught every failure and threw it as a refusal, so a missing binary
      // **blocked every write in the session** with a message that was not a
      // refusal and a fix nobody inside the agent could guess. The other three
      // scripts this crate writes settle it the other way, in the same words:
      // *a hook that breaks does not deny* — rule 3 of `harness::hook`. This one
      // was the fourth, and the only one in another language.
      //
      // And it says so. Letting a write through unchecked is the stance; doing
      // it without a word is not the same stance.
      let answer;
      try {{
        answer = await $`${{ESTIGIA}} gate ${{tool}} --input ${{payload}}`
          .cwd(cwd)
          .quiet()
          .nothrow();
      }} catch (error) {{
        console.error(
          `estigia: the gate could not be run (${{String(error?.message ?? error).trim()}}), ` +
            `so this ${{tool}} went out unchecked`,
        );
        return;
      }}
      const status = answer?.exitCode ?? 0;
      if (status === 0) return;
      const said = String(answer?.stderr ?? "").trim();
      if (status !== 1 && status !== 2) {{
        console.error(
          `estigia: the gate did not answer (exit ${{status}}${{said ? `: ${{said}}` : ""}}), ` +
            `so this ${{tool}} went out unchecked`,
        );
        return;
      }}

      // Throwing is how this API says no. The text is the refusal itself —
      // its code, what happened to the world, and what to do about it — because
      // this is the only place the agent learns any of that.
      throw new Error(said || `estigia refused this ${{tool}} (exit ${{status}})`);
    }},
  }};
}};
"#
    )
}

/// Whether the plugin file at `path` is Estigia's, somebody else's, or absent.
pub fn is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(MARKER))
}

/// The hook script Cline runs, in the language its launcher speaks.
///
/// Cline discovers hooks by **file name** — one executable per event under
/// `~/.cline/hooks/` — rather than by an entry in a settings file. So this is a
/// file Estigia owns whole, which is why it travels the same path as the
/// OpenCode plugin rather than the settings envelopes.
///
/// Two languages, because `HookProcess.getHookLaunchConfig` runs the script
/// through PowerShell on Windows (`-NoProfile -NonInteractive -ExecutionPolicy
/// Bypass -File`) and through the shell everywhere else. One script would be
/// registered and silent on the other half of the platforms.
///
/// The script itself does nothing but hand its input to Estigia and print what
/// comes back. Putting a decision in here would be a second place the gate
/// lives, and the second one is always the stale one.
pub fn cline_hook(executable: &Path, windows: bool) -> String {
    // Quoted through the helpers that know what each language escapes with,
    // rather than by `display()` between two quote characters. It was the
    // second, and a path holding an apostrophe — an ordinary Windows profile —
    // closed the quoting early: `sh -n` answers *unexpected EOF while looking
    // for matching `''* to the file this writes. The gate is not
    // registered-and-wrong there; its script is not shell at all.
    //
    // Nothing refuses an apostrophe, deliberately: `setup::UNQUOTABLE` measured
    // that one survives the **double** quoting the settings hooks use.
    let path = if windows {
        crate::paths::powershell_quoted(executable)
    } else {
        crate::paths::shell_quoted(executable)
    };
    // From the one place that decides what a gate is invoked with. This file
    // had its own copy — `hook pre-tool-use --dialect cline`, and nothing else
    // — so the round that taught the ledger which agent sent an ungated call
    // reached the eight agents registered through a settings file and not the
    // one registered through this script. Measured on a `setup --all`: nine
    // hook commands, and Cline's was the only one with no `--agent` in it.
    let arguments = super::render::hook_arguments("cline", crate::harness::hook::Dialect::Cline);
    // The status is read and said, not inherited and swallowed. This dialect
    // answers in JSON on standard output and exits `0` even to refuse, so any
    // other status means Estigia did not answer at all — and the loudest way to
    // get one is the binary not being there: a `cargo clean`, an uninstall, a
    // moved profile. `exec` handed that back as 127 and printed nothing of
    // Estigia's, so the write went through looking exactly like one the gate had
    // approved.
    //
    // Letting it through is the stance — *a hook that breaks does not deny* —
    // and this crate already draws the line three times in those words: the push
    // guard about a binary it cannot find, the MCP server about a protocol
    // revision it does not speak, and `hook` about a dialect slug it does not
    // know. *Doing it without a word is not the same stance.* These two scripts
    // were where it was not said.
    if windows {
        [
            &format!("# {MARKER} — managed by `estigia setup cline`"),
            "$ErrorActionPreference = 'Continue'",
            // Windows PowerShell encodes a string piped to a native command
            // with `$OutputEncoding`, and its default there is **`us-ascii`**.
            // Measured: `'{"file_path":"src/anio-ñ.rs"}'` arrived at the binary
            // as `src/anio-?.rs`, so every non-ASCII byte of a Cline tool call
            // was destroyed on the way to the gate — in a script this crate
            // writes whole.
            //
            // It is not cosmetic. A checkout whose path carries an accent stops
            // matching the one the claim was made in, so the write reads as
            // another checkout and goes through *outside the oath*: the gate
            // deciding about a path that does not exist.
            //
            // PowerShell 7 already defaults to UTF-8, so this costs nothing
            // there and fixes 5.1, which is what Windows ships.
            //
            // **Both** encodings, and the second was found by measuring rather
            // than by reasoning: setting only the outgoing one left the payload
            // arriving as `src/anio-├▒.rs`, because `[Console]::In.ReadToEnd()`
            // decodes standard input with `[Console]::InputEncoding` — the OEM
            // code page — before anything is piped anywhere. The first fix
            // turned one mangling into another and looked plausible doing it.
            "$OutputEncoding = [System.Text.UTF8Encoding]::new($false)",
            "[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)",
            "$payload = [Console]::In.ReadToEnd()",
            &format!("$payload | & {path} {arguments}"),
            "if ($LASTEXITCODE -ne 0) {",
            concat!(
                "  [Console]::Error.WriteLine(\"estigia: the gate did not answer (exit ",
                "$LASTEXITCODE), so this write went out unchecked\")"
            ),
            "}",
            "exit 0",
            "",
        ]
        .join(
            "
",
        )
    } else {
        [
            "#!/usr/bin/env sh",
            &format!("# {MARKER} — managed by `estigia setup cline`"),
            &format!("{path} {arguments}"),
            "status=$?",
            "if [ \"$status\" -ne 0 ]; then",
            concat!(
                "  echo \"estigia: the gate did not answer (exit $status), so this write went ",
                "out unchecked\" >&2"
            ),
            "fi",
            "exit 0",
            "",
        ]
        .join(
            "
",
        )
    }
}

/// The file name Cline looks for, which carries the event in the name.
pub fn cline_hook_file(windows: bool) -> &'static str {
    if windows {
        "PreToolUse.ps1"
    } else {
        "PreToolUse.sh"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_script_is_still_a_script_when_the_path_holds_an_apostrophe() {
        // Cline's hook is a file Estigia owns whole, and it is **single**
        // quoted — unlike the settings hooks, which are double quoted, and
        // unlike the git hook, which goes through `paths::shell_quoted`.
        //
        // `setup::UNQUOTABLE` refuses `$`, a backtick, `"` and `%`, and
        // deliberately permits an apostrophe: it was measured that
        // `"C:\Users\O'Brien\estigia.exe"` survives double quoting, and
        // refusing it would lock out an ordinary Windows profile for nothing.
        // Here the same path produced `exec '/home/o'brien/estigia' …`, which
        // is not a shell script — `sh -n` answers *unexpected EOF while looking
        // for matching `''*. The gate is not registered-and-wrong; there is
        // nothing for the launcher to run.
        //
        // Checked by handing it to a shell rather than by reading it: what a
        // shell does with the line is the whole question.
        let awkward = Path::new("/home/o'brien/estigia");

        let posix = cline_hook(awkward, false);
        assert!(
            posix.contains(r"'\''"),
            "the apostrophe was not escaped for `sh`: {posix}"
        );
        let directory = tempfile::tempdir().expect("a temporary directory");
        let file = directory.path().join("PreToolUse.sh");
        std::fs::write(&file, &posix).expect("the script is written");
        match std::process::Command::new("sh")
            .arg("-n")
            .arg(&file)
            .output()
        {
            Ok(checked) => assert!(
                checked.status.success(),
                "the hook is not a shell script: {}\n{posix}",
                String::from_utf8_lossy(&checked.stderr)
            ),
            // No POSIX shell here; the assertion above still holds the contract.
            Err(_) => eprintln!("SKIPPED: no shell to parse it with."),
        }

        // PowerShell escapes its own quote by doubling it, and has no `'\''`.
        // One helper each, because a file that parses everywhere except where
        // it runs is the failure this is about.
        let windows = cline_hook(awkward, true);
        assert!(
            windows.contains("'/home/o''brien/estigia'"),
            "the apostrophe was not doubled for PowerShell: {windows}"
        );
        assert!(
            !windows.contains(r"'\''"),
            "PowerShell was handed a POSIX escape: {windows}"
        );

        // A gate that cannot run says so, in both languages.
        //
        // This dialect answers in JSON and exits `0` even to refuse, so any
        // other status means Estigia never answered — and the binary not being
        // there is how that happens: a `cargo clean`, an uninstall, a moved
        // profile. Both scripts handed the write through, which is the stance,
        // and said nothing, which is not: the shell's own `No such file` is not
        // Estigia's voice and names nothing an operator can act on.
        //
        // Held as a property rather than as the text: the script must read the
        // status, must not let a non-zero one reach the agent, and must put the
        // word `estigia` on standard error when it gets one.
        for windows in [false, true] {
            let script = cline_hook(Path::new("/usr/local/bin/estigia"), windows);
            assert!(
                !script.contains("exec "),
                "the status is inherited rather than read, so nothing can say it was not a \
                 decision: {script}"
            );
            assert!(
                script.contains("estigia: the gate did not answer"),
                "a gate that could not run passes the write without a word: {script}"
            );
            assert!(
                script.contains("exit 0") || script.contains("exit 0\n"),
                "a status that is not a decision is handed to the agent: {script}"
            );
        }

        // And an ordinary path is untouched by any of it.
        let plain = cline_hook(Path::new("/usr/local/bin/estigia"), false);
        assert!(
            plain.contains("'/usr/local/bin/estigia' hook pre-tool-use"),
            "{plain}"
        );
    }
}
