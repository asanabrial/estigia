//! The OpenCode and Pi adapters: a plugin and an extension, because that is
//! what each host reads.
//!
//! OpenCode is one module with two host generations: OpenCode 1 loads the default object's
//! `server()` — and older releases its named function export — while OpenCode 2
//! loads `setup` and its `execute.before` tool hook. Two limits, both known:
//! the V1 `tool.execute.before` is reported not to intercept calls made by
//! subagents, and the plugin shells out to the binary on every matching call.

use std::path::Path;

/// Where OpenCode loads global plugins from, under its config directory.
pub const DIRECTORY: &str = "plugins";

/// The file Estigia writes, and the only one it writes.
pub const FILE: &str = "estigia-workflow-authority.js";

/// The marker that says the file is Estigia's.
pub const MARKER: &str = "estigia:workflow-authority";

/// The plugin source.
///
/// JavaScript rather than TypeScript, and it decides nothing: everything it
/// knows is how to ask the binary and how to throw when the answer is no.
pub fn source(executable: &Path) -> String {
    let binary = binary_literal(executable);
    format!(
        r#"// {MARKER} — managed by `estigia setup opencode`
//
// Refuses a repository write that no live claim authorises. Removed by
// `estigia setup opencode --uninstall`.
//
// One gate, two entrypoints: OpenCode 2 loads the default definition's `setup`
// and its `execute.before` tool hook, while OpenCode 1 loads the default's
// `server` — and the named export stays for releases that read function exports
// instead. Both forward the session id their host hands the hook.
//
// Known limits: on OpenCode 1, `tool.execute.before` is reported not to
// intercept tool calls made by subagents spawned through the task tool, and
// OpenCode 2's hook has not been measured for that hole; every matching call
// shells out to the binary.
const ESTIGIA = {binary};
const RUNTIME = "opencode";

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
// past a gate `doctor` reports as live. `bash` is OpenCode 1's shell tool;
// `shell` is OpenCode 2's.
const GATED = [
  "edit",
  "write",
  "patch",
  "multiedit",
  "notebookedit",
  "update",
  "bash",
  "shell",
];

// The gate is asked and nothing is decided here. The shell is resolved at call
// time — OpenCode 1 hands `$` in the plugin context, OpenCode 2 exposes
// `Bun.$` — and one that is not there goes out unchecked **with a word**: a
// hook that breaks does not deny.
const gate = async (tool, args, cwd, shell, session) => {{
  if (typeof shell !== "function") {{
    console.error(
      `estigia: the gate could not be run (this host supplied no shell), ` +
        `so this ${{tool}} went out unchecked`,
    );
    return;
  }}
  const stdin = Buffer.from(JSON.stringify(args ?? {{}}));
  const identified = typeof session === "string" && session.trim() !== "";

  // `.nothrow()`, so the status is read rather than turned into an exception
  // that says only *something went wrong*. Estigia defines three codes and no
  // more — `0` allow, `1` refused, `2` an outcome it could not read back — and
  // only those three are decisions. Anything else means it did not answer: the
  // binary is gone after a `cargo clean` or an uninstall, a shim resolved to
  // nothing, it panicked.
  //
  // A missing binary once **blocked every write in the session** with a message
  // that was not a refusal and a fix nobody inside the agent could guess. The
  // other three scripts this crate writes settle it the other way and say so in
  // the same words: *a hook that breaks does not deny*.
  let answer;
  try {{
    answer = identified
      ? await shell`${{ESTIGIA}} gate ${{tool}} --input - --session-id ${{session}} --runtime ${{RUNTIME}} < ${{stdin}}`
          .cwd(cwd)
          .quiet()
          .nothrow()
      : await shell`${{ESTIGIA}} gate ${{tool}} --input - < ${{stdin}}`
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
  const status = typeof answer?.exitCode === "number" ? answer.exitCode : undefined;
  if (status === 0) return;
  const said = String(answer?.stderr ?? "").trim();
  if (status !== 1 && status !== 2) {{
    const signal = answer?.signal ?? answer?.signalCode;
    const ended = status === undefined
      ? `no exit code${{signal ? `, signal ${{signal}}` : ""}}`
      : `exit ${{status}}`;
    console.error(
      `estigia: the gate did not answer (${{ended}}${{said ? `: ${{said}}` : ""}}), ` +
        `so this ${{tool}} went out unchecked`,
    );
    return;
  }}

  // Throwing is how this API says no. The text is the refusal itself — its
  // code, what happened to the world, and what to do about it — because this is
  // the only place the agent learns any of that.
  throw new Error(said || `estigia refused this ${{tool}} (exit ${{status}})`);
}};

const before = (tool, args, cwd, shell, session) => {{
  const name = String(tool ?? "").toLowerCase();
  if (!GATED.includes(name)) return;
  return gate(name, args, cwd, shell, session);
}};

const introduced = new Set();

const mailOf = async (shell, cwd, session) => {{
  if (typeof shell !== "function" || typeof session !== "string" || session.trim() === "") return "";
  const ask = async (verb) => {{
    try {{
      const answer = await shell`${{ESTIGIA}} session ${{verb}} --session-id ${{session}} --runtime ${{RUNTIME}}`
        .cwd(cwd)
        .quiet()
        .nothrow();
      if (answer?.exitCode === 0) return String(answer.stdout ?? "").trim();
      console.error(`estigia: session ${{verb}} did not answer (exit ${{answer?.exitCode}})`);
    }} catch (error) {{
      console.error(`estigia: session ${{verb}} could not be run (${{String(error?.message ?? error).trim()}})`);
    }}
    return "";
  }};
  if (!introduced.has(session)) {{
    introduced.add(session);
    await ask("start");
  }}
  return ask("deliver");
}};

const handOver = async (session, sessionID, text) => {{
  try {{
    if (typeof session?.prompt === "function") {{
      await session.prompt({{ sessionID, text, delivery: "steer" }});
      return;
    }}
  }} catch (error) {{
    console.error(`estigia: the host would not steer a message in (${{String(error?.message ?? error).trim()}})`);
  }}
  try {{
    await session.synthetic({{ sessionID, text, description: "Estigia mailbox", resume: false }});
  }} catch (error) {{
    console.error(
      `estigia: a message taken from the mailbox could not be handed to the host ` +
        `(${{String(error?.message ?? error).trim()}}) and is lost: ${{text}}`,
    );
  }}
}};

// OpenCode 1. `directory` and `worktree` arrive in the context, exactly as they
// did for the function export older releases read; the session id arrives in the
// hook's own input, where the host's published types carry it, and is forwarded
// the same way OpenCode 2's event is. A hook input without one, with no run id
// exported around the plugin, keeps the checkout road.
export const EstigiaWorkflowAuthority = async ({{ $, directory, worktree }}) => {{
  const cwd = worktree ?? directory ?? process.cwd();
  return {{
    "tool.execute.before": async (input, output) =>
      before(input?.tool, output?.args, cwd, $, input?.sessionID),
    "tool.execute.after": async (input, output) => {{
      if (output === null || typeof output !== "object") return;
      if (output.output !== undefined && typeof output.output !== "string") return;
      const text = await mailOf($, cwd, input?.sessionID);
      if (text) output.output = `${{output.output ?? ""}}\n\n${{text}}`;
    }},
  }};
}};

// OpenCode 2. `location.directory` is the working tree this instance serves —
// measured inside a git worktree, where it is the worktree and
// `location.project.canonical` is the checkout the worktree came from — and the
// hook event carries the tool, its input, and the session id forwarded below.
export default {{
  id: "estigia-workflow-authority",
  async setup(ctx) {{
    const cwd = ctx?.location?.directory ?? process.cwd();
    await ctx.tool.hook("execute.before", async (event) => {{
      await before(event?.tool, event?.input, cwd, globalThis.Bun?.$, event?.sessionID);
    }});
    try {{
      await ctx.tool.hook("execute.after", async (event) => {{
        if (typeof ctx?.session?.prompt !== "function" && typeof ctx?.session?.synthetic !== "function") return;
        const text = await mailOf(globalThis.Bun?.$, cwd, event?.sessionID);
        if (text) await handOver(ctx?.session, event.sessionID, text);
      }});
    }} catch (error) {{
      console.error(`estigia: this host takes no execute.after hook, so no mail is delivered (${{String(error?.message ?? error).trim()}})`);
    }}
  }},
  // OpenCode 1 reads the default object and hands `server` the same context the
  // named export receives; measured on 1.17.13, 1.18.20 and 1.18.31, none of
  // which invoke the named export while this one exists.
  async server(context) {{
    return EstigiaWorkflowAuthority(context);
  }},
}};
"#
    )
}

/// A path as a JSON string literal, so a Windows path's backslashes survive into
/// the module.
fn binary_literal(executable: &Path) -> String {
    serde_json::to_string(&executable.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "\"estigia\"".to_owned())
}

/// Where Pi loads global extensions from, under its agent directory.
pub const PI_DIRECTORY: &str = "extensions";

/// The extension file Estigia writes for Pi, and the only one it writes.
pub const PI_FILE: &str = "estigia-workflow-authority.ts";

/// The event the Pi extension registers its gate under, and the name
/// `doctor` reads back for it.
pub const PI_GATE_EVENT: &str = "tool_call";

/// The built-in Pi tools the extension wakes the gate for.
pub const PI_GATED: &[&str] = &["edit", "write", "bash"];

/// The Pi extension source.
///
/// Plain JavaScript, which is also valid TypeScript, so Pi's loader runs it with
/// no build step and `node` loads the same bytes. It decides nothing: it asks the
/// binary and returns `{ block: true, reason }` when the answer is no.
pub fn pi_source(executable: &Path) -> String {
    let gated = serde_json::to_string(PI_GATED).unwrap_or_else(|_| "[]".to_owned());
    PI_TEMPLATE
        .replace("__MARKER__", MARKER)
        .replace("__BINARY__", &binary_literal(executable))
        .replace("__GATED__", &gated)
        .replace("__GATE_EVENT__", PI_GATE_EVENT)
}

const PI_TEMPLATE: &str = r#"// __MARKER__ — managed by `estigia setup pi`
//
// Refuses a repository write that no live claim authorises, puts to the person
// a write the binary says needs asking (in the terminal UI only), gives each run the
// context `SessionStart` gives other agents, shows the claim this session
// holds, counts the checkouts that hold uncommitted work in a widget, draws a
// refusal as a card, and offers `/estigia-doctor` and `/estigia-changes`, which
// show the person the report of `estigia doctor` and the listing of
// `estigia changes`. Those last five are display only.
// Removed by `estigia uninstall pi`.
import { execFile } from "node:child_process";
import { closeSync, openSync, readSync } from "node:fs";

const ESTIGIA = __BINARY__;
const RUNTIME = "pi";
const GATED = __GATED__;
const ROOT_SESSION = "ESTIGIA_PI_PARENT_SESSION";
const ROOT_PID = "ESTIGIA_PI_PARENT_PID";
const STATUS_KEY = "estigia-claim";
const GATE_TIMEOUT_MS = 30000;
const GATE_MAX_BUFFER = 8 * 1024 * 1024;
const STATUS_TIMEOUT_MS = 5000;
const SESSION_TIMEOUT_MS = 5000;
const SECTION = "estigia";
const MAIL_SECTION = "estigia-mail";
const WAKE_INTERVAL_MS = 10000;
const CARD = "estigia-refusal";
const TITLE = "Estigia";
const ASKED = 4;
const DOCTOR_COMMAND = "estigia-doctor";
const DOCTOR_CARD = "estigia-doctor";
const CHANGES_COMMAND = "estigia-changes";
const WIDGET_KEY = "estigia-changes";
const CHANGES_TIMEOUT_MS = 20000;
const WIDGET_UNKNOWN = "uncommitted work unknown";
const DOCTOR_TIMEOUT_MS = 60000;

const unchecked = (tool, why, ctx) => {
  const text = `estigia: the gate ${why}, so this ${tool} went out unchecked`;
  console.error(text);
  if (!ctx?.hasUI) return;
  try {
    ctx.ui.notify(text, "error");
  } catch {}
};

const sessionOf = (ctx) => {
  try {
    return ctx?.sessionManager?.getSessionId?.();
  } catch {
    return undefined;
  }
};

const filled = (value) => typeof value === "string" && value.trim() !== "";

const REPLACEMENT_REASONS = new Set(["new", "fork", "resume", "reload"]);
const replaced = new Set();

const parentFileOf = (ctx) => {
  try {
    const named = ctx?.sessionManager?.getHeader?.()?.parentSession;
    return filled(named) ? named : undefined;
  } catch {
    return undefined;
  }
};

const idInName = (file) => {
  const name = String(file).split(/[\\/]/).pop().replace(/\.jsonl$/, "");
  const at = name.indexOf("_");
  return at >= 0 && at < name.length - 1 ? name.slice(at + 1) : undefined;
};

// A session whose header names a parent is that parent's child, in this process or
// another, unless it began as a replacement of the session before it (new, fork,
// resume, reload), which is a root whatever its header says. Only a session without one falls back to the pid: the first process to
// see it exports its id and pid, a child pi inherits both through process.env, and
// a process whose pid is not the exported one is that child. A process never
// names its own session as its parent and never overwrites a root it did not set.
const rootOf = (own, ctx) => {
  if (!filled(own)) return undefined;
  const exported = process.env[ROOT_SESSION];
  const named = replaced.has(own) ? undefined : parentFileOf(ctx);
  if (named !== undefined) {
    if (filled(exported) && exported !== own) return exported;
    const id = header(named) ?? idInName(named);
    return id === own ? undefined : id;
  }
  if (filled(exported) && process.env[ROOT_PID] !== String(process.pid)) {
    return exported === own ? undefined : exported;
  }
  process.env[ROOT_SESSION] = own;
  process.env[ROOT_PID] = String(process.pid);
  return undefined;
};

// Estigia answers 0 allow, 1 refused, 2 an outcome it could not read back, and 4
// a question for a person, which it sends only when this extension passed
// --can-ask. Those are the decisions. A spawn error, a timeout or a missing
// exit code means the binary did not answer, and a hook that breaks does not
// deny. Any other number is an answer this extension cannot read, and it blocks.
const gate = (tool, shown, input, ctx) =>
  new Promise((resolve) => {
    const session = sessionOf(ctx);
    const parent = rootOf(session, ctx);
    const askable = ctx?.mode === "tui" && typeof ctx?.ui?.confirm === "function";
    const args = ["gate", tool, "--input", "-"];
    if (filled(session)) {
      args.push("--session-id", session, "--runtime", RUNTIME);
      if (parent) args.push("--parent-session-id", parent);
      if (askable) args.push("--can-ask");
    }
    const child = execFile(
      ESTIGIA,
      args,
      {
        cwd: ctx?.cwd ?? process.cwd(),
        encoding: "utf8",
        timeout: GATE_TIMEOUT_MS,
        maxBuffer: GATE_MAX_BUFFER,
        windowsHide: true,
      },
      (error, _stdout, stderr) => {
        const said = String(stderr ?? "").trim();
        if (error?.code === "ERR_CHILD_PROCESS_STDIO_MAXBUFFER") {
          return resolve({
            block: true,
            reason: `estigia: the gate's answer was larger than this extension can read (over ${GATE_MAX_BUFFER} bytes), so this ${shown} was blocked. Re-run \`estigia setup pi\` to refresh the extension.`,
          });
        }
        if (error?.killed) {
          unchecked(shown, `did not answer within ${GATE_TIMEOUT_MS / 1000}s`, ctx);
          return resolve(undefined);
        }
        if (error && typeof error.code !== "number") {
          const ended = error.signal
            ? `no exit code, signal ${error.signal}`
            : String(error.message ?? error).trim();
          unchecked(shown, `did not answer (${ended}${said ? `: ${said}` : ""})`, ctx);
          return resolve(undefined);
        }
        const status = error ? error.code : 0;
        if (status === 0) return resolve(undefined);
        if (status === 1 || status === 2) {
          return resolve({ block: true, reason: said || `estigia refused this ${shown} (exit ${status})` });
        }
        if (status === ASKED && askable && filled(session)) {
          return resolve({ ask: said || `estigia asks whether this ${shown} may go ahead` });
        }
        resolve({
          block: true,
          reason: `estigia: the gate answered something this extension cannot read (exit ${status}${said ? `: ${said}` : ""}), so this ${shown} was blocked. Re-run \`estigia setup pi\` to refresh the extension.`,
        });
      },
    );
    child?.stdin?.on("error", () => {});
    child?.stdin?.end(JSON.stringify(input ?? {}));
  });

// A tool is judged by the name the classifier knows, and failing that by the
// shape of its input: a string `command` is a shell call, which is also what
// Pi's `powershell` is, and a path with content or edits is a write.
const gatedAs = (name, input) => {
  if (GATED.includes(name)) return name;
  if (input === null || typeof input !== "object") return undefined;
  if (typeof input.command === "string") return "bash";
  if (typeof input.path === "string" && typeof input.content === "string") return "write";
  if (typeof input.path === "string" && Array.isArray(input.edits)) return "edit";
  return undefined;
};

const confirmed = async (ctx, question) => {
  try {
    return (await ctx.ui.confirm(TITLE, question)) === true;
  } catch {
    return false;
  }
};

const UNKNOWN = "estigia: claim unknown";

const describe = (answer) => {
  if (answer === null || typeof answer !== "object" || !("issue" in answer)) return UNKNOWN;
  if (answer.issue === null || answer.issue === undefined) return undefined;
  const issue = /^\d+$/.test(String(answer.issue)) ? `#${answer.issue}` : String(answer.issue);
  const parts = [issue];
  if (filled(answer.state)) parts.push(answer.state);
  if (answer.live !== true && filled(answer.standing)) parts.push(answer.standing);
  return `estigia: ${parts.join(" · ")}`;
};

// A claim the binary did not report is shown as unknown, never as the last one
// it did.
const claimOf = (subject, cwd) =>
  new Promise((resolve) => {
    execFile(
      ESTIGIA,
      ["goal", "--json", "--session-id", subject, "--runtime", RUNTIME],
      { cwd, encoding: "utf8", timeout: STATUS_TIMEOUT_MS, windowsHide: true },
      (error, stdout) => {
        if (error) return resolve(UNKNOWN);
        try {
          resolve(describe(JSON.parse(stdout)));
        } catch {
          resolve(UNKNOWN);
        }
      },
    );
  });

const header = (file) => {
  if (!filled(file)) return undefined;
  let fd;
  try {
    fd = openSync(file, "r");
    const buffer = Buffer.alloc(4096);
    const read = readSync(fd, buffer, 0, buffer.length, 0);
    const id = JSON.parse(buffer.toString("utf8", 0, read).split("\n", 1)[0])?.id;
    return filled(id) ? id : undefined;
  } catch {
    return undefined;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
};

const failure = (error, stderr) => {
  if (error?.killed) return `no answer within ${SESSION_TIMEOUT_MS / 1000}s`;
  const said = String(stderr ?? "").trim();
  const ended =
    typeof error?.code === "number"
      ? `exit ${error.code}`
      : error?.signal
        ? `no exit code, signal ${error.signal}`
        : String(error?.message ?? error).trim();
  return said ? `${ended}: ${said}` : ended;
};

const askSession = (verb, subject, extra, cwd) =>
  new Promise((resolve) => {
    execFile(
      ESTIGIA,
      ["session", verb, "--session-id", subject, "--runtime", RUNTIME, ...extra],
      { cwd, encoding: "utf8", timeout: SESSION_TIMEOUT_MS, windowsHide: true },
      (error, stdout, stderr) => {
        if (error) return resolve({ why: failure(error, stderr) });
        resolve({ text: String(stdout ?? "").trim() });
      },
    );
  });

const started = new Map();
const ended = new Set();

const sessionContext = async (session, ctx) => {
  if (!filled(session)) return "Estigia: session context unknown (Pi gave this session no id)";
  const begun = started.get(session) ?? {};
  const extra = [];
  if (filled(begun.reason)) extra.push("--reason", begun.reason);
  if (filled(begun.previous)) extra.push("--previous-session-id", begun.previous);
  const answer = await askSession(
    "start",
    rootOf(session, ctx) ?? session,
    extra,
    ctx?.cwd ?? process.cwd(),
  );
  if (answer.text) return answer.text;
  return `Estigia: session context unknown (${answer.why ?? "the binary printed nothing"})`;
};

let latest = 0;
const showClaim = async (ctx) => {
  if (!ctx?.hasUI) return;
  const session = sessionOf(ctx);
  if (!filled(session)) return;
  const ticket = ++latest;
  const text = await claimOf(rootOf(session, ctx) ?? session, ctx.cwd ?? process.cwd());
  if (ticket !== latest) return;
  try {
    ctx.ui.setStatus(STATUS_KEY, text);
  } catch {}
};

const tallied = (answer) => {
  const trees = answer?.worktrees;
  if (!Array.isArray(trees)) return WIDGET_UNKNOWN;
  const holding = trees.filter((tree) => !filled(tree?.unreadable) && Array.isArray(tree?.files) && tree.files.length > 0).length;
  const unreadable = trees.filter((tree) => filled(tree?.unreadable)).length;
  if (holding === 0 && unreadable === 0) return undefined;
  const noun = (count) => `${count} checkout${count === 1 ? "" : "s"}`;
  if (holding === 0) return `${noun(unreadable)} unreadable`;
  return `uncommitted work in ${noun(holding)}${unreadable > 0 ? `, ${unreadable} unreadable` : ""}`;
};

const changesOf = (cwd) =>
  new Promise((resolve) => {
    execFile(
      ESTIGIA,
      ["changes", "--json"],
      { cwd, encoding: "utf8", timeout: CHANGES_TIMEOUT_MS, maxBuffer: GATE_MAX_BUFFER, windowsHide: true },
      (error, stdout) => {
        if (error) return resolve(WIDGET_UNKNOWN);
        try {
          resolve(tallied(JSON.parse(stdout)));
        } catch {
          resolve(WIDGET_UNKNOWN);
        }
      },
    );
  });

let newest = 0;
const showChanges = async (ctx) => {
  if (!ctx?.hasUI || typeof ctx?.ui?.setWidget !== "function") return;
  const ticket = ++newest;
  const text = await changesOf(ctx.cwd ?? process.cwd());
  if (ticket !== newest) return;
  try {
    ctx.ui.setWidget(WIDGET_KEY, text === undefined ? undefined : [`estigia: ${text} \u00b7 /${CHANGES_COMMAND}`]);
  } catch {}
};

const doctorFailure = (error) => {
  if (error?.code === "ERR_CHILD_PROCESS_STDIO_MAXBUFFER") {
    return `its report was larger than ${GATE_MAX_BUFFER} bytes`;
  }
  if (error?.killed) return `no answer within ${DOCTOR_TIMEOUT_MS / 1000}s`;
  if (typeof error?.code === "number") return `exit ${error.code}, nothing printed`;
  if (error?.signal) return `no exit code, signal ${error.signal}`;
  return String(error?.message ?? error).trim();
};

const askReport = (verb, cwd) =>
  new Promise((resolve) => {
    execFile(
      ESTIGIA,
      [verb],
      { cwd, encoding: "utf8", timeout: DOCTOR_TIMEOUT_MS, maxBuffer: GATE_MAX_BUFFER, windowsHide: true },
      (error, stdout, stderr) => {
        const spoken = [String(stdout ?? "").replace(/\n+$/, ""), String(stderr ?? "").trim()]
          .filter((part) => part !== "")
          .join("\n");
        const answered = !error || (typeof error.code === "number" && !error.killed);
        if (answered && spoken !== "") return resolve({ answered: true, text: spoken });
        resolve({ answered: false, text: `estigia ${verb} did not answer (${doctorFailure(error ?? { code: 0 })})` });
      },
    );
  });

const askDoctor = (cwd) => askReport("doctor", cwd);

const showDoctor = (pi, ctx, report) => {
  if (!ctx?.hasUI) {
    (report.answered ? console.log : console.error)(report.text);
    return;
  }
  try {
    if (typeof pi.appendEntry === "function") pi.appendEntry(DOCTOR_CARD, report);
    else ctx.ui.notify(report.text, report.answered ? "info" : "error");
  } catch {}
};

const wrap = (line, width) => {
  if (!(width > 0)) return [line];
  const pieces = [];
  for (let at = 0; at < line.length || at === 0; at += width) pieces.push(line.slice(at, at + width));
  return pieces;
};

// An entry, not a message: a custom message is converted to a user message for
// the model whatever its display flag says, and the model already holds the
// reason as the tool result.
const card = (pi, ctx, tool, reason) => {
  if (!ctx?.hasUI) return;
  try {
    pi.appendEntry?.(CARD, { tool, reason });
  } catch {}
};

const peeked = (text) => {
  try {
    const answer = JSON.parse(text);
    return typeof answer?.messages === "string" && Number.isInteger(answer?.through)
      ? { text: answer.messages, through: answer.through }
      : undefined;
  } catch {
    return undefined;
  }
};

export default function (pi) {
  let waker;
  let handover = Promise.resolve();
  let ticking = false;
  const stopWake = () => {
    if (waker !== undefined) clearInterval(waker);
    waker = undefined;
  };
  const wake = async (own, cwd) => {
    const mail = await askSession("deliver", own, ["--peek", "--json"], cwd);
    if (mail.why) return console.error(`estigia: the mailbox was not read (${mail.why})`);
    const found = peeked(mail.text);
    if (found === undefined) return;
    try {
      pi.sendMessage(
        { customType: MAIL_SECTION, content: found.text, display: true },
        { deliverAs: "followUp", triggerTurn: true },
      );
    } catch (error) {
      return console.error(`estigia: waking the session failed, so the mail stays pending (${String(error?.message ?? error).trim()})`);
    }
    const taken = await askSession("deliver", own, ["--through", String(found.through)], cwd);
    if (taken.why) console.error(`estigia: mail handed to the session was not recorded as delivered (${taken.why})`);
  };
  const startWake = (ctx) => {
    stopWake();
    const own = sessionOf(ctx);
    if (!filled(own) || rootOf(own, ctx) || typeof pi.sendMessage !== "function") return;
    const cwd = ctx?.cwd ?? process.cwd();
    waker = setInterval(() => {
      if (ticking) return;
      ticking = true;
      handover = wake(own, cwd)
        .catch(() => {})
        .finally(() => {
          ticking = false;
        });
    }, WAKE_INTERVAL_MS);
    waker?.unref?.();
  };
  pi.registerEntryRenderer?.(CARD, (entry, _options, theme) => {
    const data = entry?.data ?? {};
    const reason = String(data.reason ?? "");
    const head = `estigia refused this ${data.tool ?? "call"}`;
    return {
      render: (width) => [
        ...wrap(head, width).map((line) => theme?.fg?.("error", line) ?? line),
        ...reason.split("\n").flatMap((line) => wrap(line, width)),
      ],
      invalidate: () => {},
    };
  });
  pi.registerEntryRenderer?.(DOCTOR_CARD, (entry, _options, theme) => {
    const data = entry?.data ?? {};
    return {
      render: (width) =>
        String(data.text ?? "")
          .split("\n")
          .flatMap((line) => wrap(line, width))
          .map((line) => (data.answered === false ? (theme?.fg?.("error", line) ?? line) : line)),
      invalidate: () => {},
    };
  });
  pi.registerCommand?.(DOCTOR_COMMAND, {
    description: "Show the report of `estigia doctor` (shown to you, not sent to the model)",
    handler: async (_args, ctx) => {
      showDoctor(pi, ctx, await askDoctor(ctx?.cwd ?? process.cwd()));
    },
  });
  pi.registerCommand?.(CHANGES_COMMAND, {
    description: "Show the uncommitted work in every checkout (shown to you, not sent to the model)",
    handler: async (_args, ctx) => {
      showDoctor(pi, ctx, await askReport("changes", ctx?.cwd ?? process.cwd()));
    },
  });
  pi.on("session_start", async (event, ctx) => {
    const session = sessionOf(ctx);
    if (filled(session) && REPLACEMENT_REASONS.has(event?.reason)) replaced.add(session);
    rootOf(session, ctx);
    if (filled(session)) {
      started.set(session, { reason: event?.reason, previous: header(event?.previousSessionFile) });
    }
    void showClaim(ctx);
    void showChanges(ctx);
    startWake(ctx);
  });
  pi.on("before_agent_start", async (event, ctx) => {
    const text = await sessionContext(sessionOf(ctx), ctx);
    const options = event?.systemPromptOptions;
    if (options === null || typeof options !== "object") {
      console.error("estigia: this Pi exposes no systemPromptOptions, so the session context was not added");
      return undefined;
    }
    options.sections ??= {};
    options.sections[SECTION] = text;
    const own = sessionOf(ctx);
    if (filled(own) && !rootOf(own, ctx)) {
      await handover;
      const mail = await askSession("deliver", own, [], ctx?.cwd ?? process.cwd());
      if (mail.text) options.sections[MAIL_SECTION] = mail.text;
      else if (mail.why) console.error(`estigia: the mailbox was not read (${mail.why})`);
    }
    return undefined;
  });
  pi.on("agent_start", async () => {
    stopWake();
  });
  pi.on("session_shutdown", async (event, ctx) => {
    stopWake();
    const session = sessionOf(ctx);
    if (!filled(session) || rootOf(session, ctx)) return;
    const key = `${session}:${event?.reason ?? ""}`;
    if (ended.has(key)) return;
    ended.add(key);
    const extra = filled(event?.reason) ? ["--reason", event.reason] : [];
    const answer = await askSession("end", session, extra, ctx?.cwd ?? process.cwd());
    if (answer.why) console.error(`estigia: the session end was not recorded (${answer.why})`);
  });
  pi.on("agent_settled", async (_event, ctx) => {
    void showClaim(ctx);
    void showChanges(ctx);
    startWake(ctx);
  });
  pi.on("tool_result", async (_event, ctx) => {
    void showClaim(ctx);
  });
  pi.on("__GATE_EVENT__", async (event, ctx) => {
    rootOf(sessionOf(ctx), ctx);
    const name = String(event?.toolName ?? "").toLowerCase();
    const as = gatedAs(name, event?.input);
    if (as === undefined) return undefined;
    const answer = await gate(as, name, event?.input, ctx);
    if (answer?.ask === undefined) {
      if (answer) card(pi, ctx, name, answer.reason);
      return answer;
    }
    if (await confirmed(ctx, answer.ask)) return undefined;
    const refusal = { block: true, reason: answer.ask };
    card(pi, ctx, name, refusal.reason);
    return refusal;
  });
}
"#;

/// Whether the plugin file at `path` is Estigia's, somebody else's, or absent.
pub fn is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(MARKER))
}

/// The optional loop plugin, written while the `Loop plugin` setting is on.
pub const LOOP_FILE: &str = "flow-loop.js";

/// The name the loop plugin was written under before it was renamed; setup and
/// uninstall take back an Estigia-owned file still carrying it.
pub const LEGACY_LOOP_FILE: &str = "estigia-loop.js";

/// The marker that says the loop plugin is Estigia's, so a file it did not
/// write is neither replaced nor removed.
pub const LOOP_MARKER: &str = "estigia:loop";

/// Whether the loop plugin file is Estigia's, somebody else's, or absent.
pub fn loop_is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(LOOP_MARKER))
}

/// The loop extension Estigia writes for Pi while the `Loop plugin` setting is on.
pub const PI_LOOP_FILE: &str = "estigia-loop.ts";

/// The loop extension source for Pi: it continues a session while `estigia goal`
/// says the run has work, and registers `/flow-loop` and `/flow-status` itself.
///
/// It decides nothing: the only exits are the binary's answer and the completion
/// cap. An answer that did not come stops the loop and says so.
pub fn pi_loop_source(executable: &Path) -> String {
    PI_LOOP_TEMPLATE
        .replace("__MARKER__", LOOP_MARKER)
        .replace(
            "__PREAMBLE__",
            &crate::harness::loop_text::preamble_parts().to_string(),
        )
        .replace(
            "__OBJECTIVE__",
            &crate::harness::loop_text::objective_parts().to_string(),
        )
        .replace("__BINARY__", &binary_literal(executable))
}

const PI_LOOP_TEMPLATE: &str = include_str!("pi_loop.js");

/// The generic goal and loop plugin, written while the `Goal loop plugin`
/// setting is on.
pub const GOAL_LOOP_FILE: &str = "goal-loop.js";

/// The marker that says the goal and loop plugin is Estigia's.
pub const GOAL_LOOP_MARKER: &str = "estigia:goal-loop";

/// The generic goal and loop extension Estigia writes for Pi while the
/// `Goal loop plugin` setting is on.
pub const PI_GOAL_LOOP_FILE: &str = "estigia-goal-loop.ts";

/// Whether the goal and loop plugin file is Estigia's, somebody else's, or absent.
pub fn goal_loop_is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(GOAL_LOOP_MARKER))
}

/// The review relay plugin: what OpenCode loads so the binary can build a sealed
/// reviewer's prompt and admit its answer when no reviewer CLI is installed.
pub const REVIEW_FILE: &str = "estigia-review-relay.js";

/// The marker that says the review relay plugin is Estigia's.
pub const REVIEW_MARKER: &str = "estigia:review-relay";

/// Whether the review relay plugin file is Estigia's, somebody else's, or absent.
pub fn review_relay_is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(REVIEW_MARKER))
}

const RELAY_EXIT_CLASS: &str = r#"const parseCode = (status, stderr) => {
  let said = null;
  try {
    said = JSON.parse(String(stderr ?? "").trim());
  } catch (error) {
    said = null;
  }
  const code = said && typeof said.code === "string" && said.code !== "" ? said.code : null;
  const message = said && typeof said.message === "string" && said.message !== "" ? said.message : null;
  if (status === 0) return { code: null, message };
  if (status === 1 || status === 2) {
    return { code: code ?? "transport-refused", message: message ?? String(stderr ?? "").trim() };
  }
  return { code: "authority_unavailable", message: message ?? String(stderr ?? "").trim() };
};"#;

/// The review relay plugin source.
///
/// JavaScript rather than TypeScript, and it decides nothing: it asks the binary
/// for the prompt a sealed reviewer is handed and hands the reviewer's raw answer
/// back for admission, so the orchestrator never receives that answer. The two
/// v4 defects are refused in the shape of the code: standard output is read to
/// EOF before a failed delivery is treated as the outcome, and an authority that
/// did not answer is never reported as a moved receipt.
pub fn review_relay_source(executable: &Path) -> String {
    let binary = serde_json::to_string(&executable.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "\"estigia\"".to_owned());
    REVIEW_RELAY_TEMPLATE
        .replace("__MARKER__", REVIEW_MARKER)
        .replace("__BINARY__", &binary)
        .replace("__EXIT_CLASS__", RELAY_EXIT_CLASS)
}

const REVIEW_RELAY_TEMPLATE: &str = r#"// __MARKER__ — managed by `estigia setup opencode`
//
// Relays a sealed reviewer the host launches to the binary. In `execute.before`
// the binary materialises the exact prompt and this puts it in the task input;
// in `execute.after` the reviewer's raw output goes to the binary for admission
// and the result the orchestrator reads is replaced, so the reviewer's answer
// never reaches the orchestrator. Removed by
// `estigia setup opencode --uninstall`.
//
// Two refusals are kept apart, which is the whole point: an authority that did
// not answer is `authority_unavailable`, never `capture-result-stale`, and a
// result for a receipt the review has moved past is refused with the stale class
// the binary already carries. A failed delivery of the output to the relay is
// not the outcome either: standard output is read to EOF first, because the
// reason a relay refused is written there.
const ESTIGIA = __BINARY__;
const RUNTIME = "opencode";

const REVIEWERS = [
  "review-lens",
  "review-risk",
  "review-readability",
  "review-reliability",
  "review-readback",
  "review-validator",
  "review-refuter",
];
const SUBAGENT_TOOLS = ["task", "agent", "subagent"];
const MARKER = /estigia:review(?:\s+name=([A-Za-z0-9_-]+))?/;

const errorText = (error) => String(error?.message ?? error);

const coded = (code, message) => {
  const error = new Error(message);
  error.code = code;
  return error;
};

const shellOf = (shell) => (typeof shell === "function" ? shell : globalThis.Bun?.$);

const targetOf = (event) => {
  const input = event?.input;
  if (input && typeof input === "object") {
    const named = [input.subagent_type, input.agent, input.agent_type, input.name].find(
      (value) => typeof value === "string" && REVIEWERS.includes(value),
    );
    if (named) return named;
  }
  const hay = typeof input === "string" ? input : JSON.stringify(input ?? {});
  const found = MARKER.exec(hay);
  return found && found[1] ? found[1] : null;
};

const reviewerEvent = (event) => {
  const tool = String(event?.tool ?? "").toLowerCase();
  if (!SUBAGENT_TOOLS.includes(tool)) return null;
  return targetOf(event);
};

__EXIT_CLASS__

// The relay is asked and nothing is decided here. Standard input is always
// attached with the shell's own `<` redirect, an empty one for `prompt`: Bun's
// shell has no `.stdin()`, and a command left without one inherits the host's
// terminal and blocks on it. Where the input cannot be attached nothing is run.
// A failed delivery is swallowed because the relay may exit before it reads the
// whole answer, and the reason it refused is on standard output, which is read
// to EOF before anything is treated as the outcome.
const ask = async (shell, cwd, action, name, session, stdin) => {
  const run = shellOf(shell);
  if (typeof run !== "function") {
    return {
      code: "authority_unavailable",
      message: "estigia: the review relay could not be run (this host supplied no shell)",
    };
  }
  if (typeof Blob !== "function") {
    return {
      code: "authority_unavailable",
      message:
        "estigia: the review relay was not run (this host cannot attach standard input to it)",
    };
  }
  const input = new Blob([stdin ?? ""]);
  const identified = typeof session === "string" && session.trim() !== "";
  let command;
  try {
    command = identified
      ? run`${ESTIGIA} review ${action} --name ${name} --session-id ${session} --runtime ${RUNTIME} < ${input}`
          .cwd(cwd)
          .quiet()
          .nothrow()
      : run`${ESTIGIA} review ${action} --name ${name} < ${input}`.cwd(cwd).quiet().nothrow();
  } catch (error) {
    return {
      code: "authority_unavailable",
      message: `estigia: the review relay could not be started (${errorText(error)})`,
    };
  }
  let answer;
  try {
    answer = await command;
  } catch (error) {
    answer = command;
  }
  const status = typeof answer?.exitCode === "number" ? answer.exitCode : 3;
  const parsed = parseCode(status, answer?.stderr);
  if (parsed.code) return parsed;
  return { code: null, stdout: String(answer?.stdout ?? "") };
};

const putPrompt = (event, prompt) => {
  if (!event || typeof event !== "object") return;
  if (event.input && typeof event.input === "object") {
    event.input.prompt = prompt;
  } else {
    event.input = prompt;
  }
};

const putResult = (event, note) => {
  if (!event || typeof event !== "object") return;
  if (typeof event.result === "string") {
    event.result = note;
    return;
  }
  if (!event.result || typeof event.result !== "object") event.result = {};
  for (const field of ["result", "text"]) delete event.result[field];
  event.result.output = note;
  event.result.content = [{ type: "text", text: note }];
};

const rawOutput = (event) => {
  const result = event?.result;
  if (typeof result === "string") return result;
  if (result && typeof result === "object") {
    if (Array.isArray(result.content)) {
      const parts = result.content
        .map((part) => (typeof part === "string" ? part : part?.text))
        .filter((text) => typeof text === "string");
      if (parts.length > 0) return parts.join("\n");
    }
    for (const field of ["output", "content", "result", "text"]) {
      if (typeof result[field] === "string") return result[field];
    }
    return JSON.stringify(result);
  }
  return String(result ?? "");
};

const slots = new Map();

const before = async (event, cwd, shell) => {
  const name = reviewerEvent(event);
  if (!name) return;
  const asked = await ask(shell, cwd, "prompt", name, event?.sessionID, null);
  if (asked.code) {
    throw coded(
      asked.code,
      asked.message || `estigia: the reviewer's prompt could not be read (${asked.code})`,
    );
  }
  const prompt = String(asked.stdout ?? "").trim();
  if (prompt === "") {
    throw coded("authority_unavailable", "estigia: the review relay answered no prompt");
  }
  putPrompt(event, prompt);
  if (event?.id !== undefined && event?.id !== null) slots.set(String(event.id), name);
};

const after = async (event, cwd, shell) => {
  const key = event?.id !== undefined && event?.id !== null ? String(event.id) : null;
  const remembered = key === null ? undefined : slots.get(key);
  if (key !== null) slots.delete(key);
  const name = remembered ?? reviewerEvent(event);
  if (!name) return;
  const asked = await ask(shell, cwd, "admit", name, event?.sessionID, rawOutput(event));
  if (asked.code) {
    const refusal = asked.message || `estigia refused the reviewer's answer (${asked.code})`;
    if (event && typeof event === "object") {
      event.status = "error";
      event.error = refusal;
    }
    putResult(event, refusal);
    return;
  }
  putResult(
    event,
    "estigia: the reviewer's answer was admitted to the review ledger and is not shown here.",
  );
};

const relay = async ({ $, directory, worktree }) => {
  const cwd = worktree ?? directory ?? process.cwd();
  return {
    "tool.execute.before": async (input, output) => {
      const event = { tool: input?.tool, sessionID: input?.sessionID, id: input?.callID };
      event.input = output?.args;
      await before(event, cwd, $);
    },
    "tool.execute.after": async (input, output) => {
      const event = {
        tool: input?.tool,
        sessionID: input?.sessionID,
        id: input?.callID,
        input: input?.args,
        result: output?.output,
      };
      await after(event, cwd, $);
      if (event.status === "error") {
        output.output = event.error;
        throw coded("review-admission-refused", event.error);
      }
      if (typeof event.result === "string") output.output = event.result;
    },
  };
};

export const EstigiaReviewRelay = relay;

export default {
  id: "estigia-review-relay",
  async setup(ctx) {
    const cwd = ctx?.location?.directory ?? process.cwd();
    if (typeof ctx?.tool?.hook !== "function") return;
    await ctx.tool.hook("execute.before", async (event) => {
      await before(event, cwd, globalThis.Bun?.$);
    });
    await ctx.tool.hook("execute.after", async (event) => {
      await after(event, cwd, globalThis.Bun?.$);
    });
  },
  async server(context) {
    return relay(context);
  },
};
"#;

/// The review relay extension Estigia writes for Pi.
pub const PI_REVIEW_FILE: &str = "estigia-review-relay.ts";

/// The one tool the Pi review relay registers, and the name a Pi agent calls to
/// run a sealed reviewer.
pub const PI_REVIEW_TOOL: &str = "estigia_review_lens";

/// The review relay extension source for Pi.
///
/// Pi has no sub-agent tool to intercept, so this registers one of its own: the
/// tool asks the binary for the prompt and for the model the `reviewer` routing
/// row names, runs one tool-free completion through Pi's own model registry, and
/// hands the raw answer to the binary for admission. It decides nothing, and the
/// answer never reaches the model that called the tool.
pub fn pi_review_relay_source(executable: &Path) -> String {
    let reviewers: Vec<&str> = crate::reviewer::Role::ALL
        .iter()
        .map(|role| role.name())
        .collect();
    let reviewers = serde_json::to_string(&reviewers).unwrap_or_else(|_| "[]".to_owned());
    PI_REVIEW_RELAY_TEMPLATE
        .replace("__MARKER__", REVIEW_MARKER)
        .replace("__BINARY__", &binary_literal(executable))
        .replace("__TOOL__", PI_REVIEW_TOOL)
        .replace("__REVIEWERS__", &reviewers)
        .replace(
            "__DEADLINE__",
            &crate::reviewer::DEFAULT_DEADLINE.as_millis().to_string(),
        )
        .replace("__EXIT_CLASS__", RELAY_EXIT_CLASS)
}

const PI_REVIEW_RELAY_TEMPLATE: &str = r#"// __MARKER__ — managed by `estigia setup pi`
//
// Registers `__TOOL__`, which runs one sealed reviewer in process and hands
// its raw answer to the binary for admission, so the answer never reaches the
// model that called the tool. Pi has no sub-agent tool to intercept, which is
// why this registers one. Removed by `estigia uninstall pi`.
//
// The binary builds the prompt, names the model and admits the answer; nothing
// is decided here. Two refusals are kept apart: an authority that did not
// answer is `authority_unavailable`, never `capture-result-stale`, and a result
// for a receipt the review has moved past carries the stale class the binary
// already has. A failed delivery of the answer is not the outcome either:
// standard output is read to the end first, because the reason a relay refused
// is written there.
import { spawn } from "node:child_process";

const ESTIGIA = __BINARY__;
const RUNTIME = "pi";
const TOOL = "__TOOL__";
const REVIEWERS = __REVIEWERS__;
const DEADLINE_MS = __DEADLINE__;

const errorText = (error) => String(error?.message ?? error);

const refusal = (code, message) => {
  const error = new Error(`${code}: ${message}`);
  error.code = code;
  return error;
};

const sessionOf = (ctx) => {
  try {
    return ctx?.sessionManager?.getSessionId?.();
  } catch {
    return undefined;
  }
};

__EXIT_CLASS__

const run = (args, cwd, stdin) =>
  new Promise((resolve) => {
    const out = [];
    const err = [];
    let settled = false;
    const settle = (answer) => {
      if (settled) return;
      settled = true;
      resolve(answer);
    };
    let child;
    try {
      child = spawn(ESTIGIA, args, { cwd, shell: false, windowsHide: true });
    } catch (error) {
      settle({ status: null, stdout: "", stderr: errorText(error) });
      return;
    }
    child.stdout.on("data", (chunk) => out.push(chunk));
    child.stderr.on("data", (chunk) => err.push(chunk));
    child.stdin.on("error", () => {});
    child.on("error", (error) => settle({ status: null, stdout: "", stderr: errorText(error) }));
    child.on("close", (status) =>
      settle({
        status,
        stdout: Buffer.concat(out).toString("utf8"),
        stderr: Buffer.concat(err).toString("utf8"),
      }),
    );
    child.stdin.end(stdin ?? "");
  });

const ask = async (cwd, action, name, session, stdin) => {
  const args = ["review", action, "--name", name];
  if (typeof session === "string" && session.trim() !== "") {
    args.push("--session-id", session, "--runtime", RUNTIME);
  }
  const ran = await run(args, cwd, stdin);
  const parsed = parseCode(ran.status, ran.stderr);
  if (parsed.code) return parsed;
  return { code: null, stdout: ran.stdout };
};

const fail = (asked, what) => {
  throw refusal(asked.code, asked.message || `estigia could not ${what} (${asked.code})`);
};

const modelOf = (ctx, named) => {
  if (named === "") {
    if (ctx?.model) return ctx.model;
    throw refusal(
      "reviewer-unavailable",
      "the `reviewer` routing row inherits the session model and this session has none",
    );
  }
  const at = named.indexOf("/");
  const found = at > 0 ? ctx?.modelRegistry?.find?.(named.slice(0, at), named.slice(at + 1)) : undefined;
  if (!found) {
    throw refusal(
      "reviewer-unavailable",
      `the \`reviewer\` routing row names ${named}, which this Pi's model registry does not list as provider/id; name a model it lists, or set the row to inherit`,
    );
  }
  return found;
};

const complete = async (ctx, model, prompt, signal) => {
  const registry = ctx?.modelRegistry;
  if (typeof registry?.streamSimple !== "function") {
    throw refusal("reviewer-unavailable", "this Pi exposes no model registry that completes");
  }
  const timeout = AbortSignal.timeout(DEADLINE_MS);
  const bound = signal ? AbortSignal.any([signal, timeout]) : timeout;
  const context = {
    messages: [{ role: "user", content: [{ type: "text", text: prompt }], timestamp: Date.now() }],
  };
  let assistant;
  try {
    assistant = await registry.streamSimple(model, context, { signal: bound }).result();
  } catch (error) {
    if (timeout.aborted) throw refusal("reviewer-deadline", "the reviewer did not finish in time");
    throw refusal("reviewer-error-result", `the reviewer's completion failed (${errorText(error)})`);
  }
  if (timeout.aborted) throw refusal("reviewer-deadline", "the reviewer did not finish in time");
  if (assistant?.stopReason === "error" || assistant?.stopReason === "aborted") {
    throw refusal(
      "reviewer-error-result",
      `the reviewer's completion ended ${assistant.stopReason} (${assistant.errorMessage ?? "no provider message"})`,
    );
  }
  const parts = Array.isArray(assistant?.content) ? assistant.content : [];
  const text = parts
    .filter((part) => part?.type === "text" && typeof part.text === "string")
    .map((part) => part.text)
    .join("");
  if (text.trim() === "") throw refusal("reviewer-no-answer", "the reviewer finished without saying anything");
  return text;
};

const lens = async (params, signal, ctx) => {
  const name = String(params?.name ?? "");
  if (!REVIEWERS.includes(name)) {
    throw refusal("reviewer-unavailable", `${name || "no name"} is not a reviewer this relay runs`);
  }
  const cwd = ctx?.cwd ?? process.cwd();
  const session = sessionOf(ctx);
  const prompted = await ask(cwd, "prompt", name, session, null);
  if (prompted.code) fail(prompted, "read the reviewer's prompt");
  if (prompted.stdout.trim() === "") {
    throw refusal("authority_unavailable", "estigia: the review relay answered no prompt");
  }
  const modelled = await ask(cwd, "model", name, session, null);
  if (modelled.code) fail(modelled, "read the reviewer's model");
  const answer = await complete(ctx, modelOf(ctx, modelled.stdout.trim()), prompted.stdout, signal);
  const admitted = await ask(cwd, "admit", name, session, answer);
  if (admitted.code) fail(admitted, "admit the reviewer's answer");
  return {
    content: [
      {
        type: "text",
        text: "estigia: the reviewer's answer was admitted to the review ledger and is not shown here.",
      },
    ],
    details: { name, admitted: true },
  };
};

export default function (pi) {
  pi.registerTool({
    name: TOOL,
    label: "Estigia review lens",
    description:
      "Runs one sealed Estigia reviewer in process and admits its answer to the review ledger. The answer is not returned. Call it for each input review_status offers.",
    parameters: {
      type: "object",
      properties: {
        name: { type: "string", enum: REVIEWERS, description: "The reviewer slot review_status offers." },
      },
      required: ["name"],
      additionalProperties: false,
    },
    execute: (_id, params, signal, _update, ctx) => lens(params, signal, ctx),
  });
}
"#;

/// The subagent runner extension Estigia writes for Pi on every install.
pub const PI_AGENTS_FILE: &str = "estigia-agents.ts";

/// The marker that says the Pi subagent runner is Estigia's.
pub const AGENTS_MARKER: &str = "estigia:agents";

/// The one tool the Pi subagent runner registers.
pub const PI_AGENTS_TOOL: &str = "estigia_subagent";

/// Whether the Pi subagent runner file is Estigia's, somebody else's, or absent.
pub fn agents_is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(AGENTS_MARKER))
}

/// The subagent runner extension source for Pi.
///
/// It starts each worker as its own `pi --mode rpc` process, draws a card per
/// worker above the editor, and stops one on request. It decides nothing: the
/// child inherits the environment the gate extension exported, and the binary
/// decides every write the child makes.
pub fn pi_agents_source() -> String {
    PI_AGENTS_TEMPLATE
        .replace("__MARKER__", AGENTS_MARKER)
        .replace("__PI__", "pi")
        .replace("__TOOL__", PI_AGENTS_TOOL)
}

const PI_AGENTS_TEMPLATE: &str = include_str!("pi_agents.js");

/// The marker inside one of the loop's slash commands. An HTML comment, so it
/// renders as nothing where the command's prompt is read.
pub const COMMAND_MARKER: &str = "<!-- estigia:loop-command -->";

/// Whether a slash command file is Estigia's, somebody else's, or absent.
pub fn command_is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(COMMAND_MARKER))
}

/// The marker inside one of the Pi prompt templates. An HTML comment, so it
/// renders as nothing where the template's text is read.
pub const PI_PROMPT_MARKER: &str = "<!-- estigia:pi-prompt -->";

/// Whether a Pi prompt template is Estigia's, somebody else's, or absent.
pub fn prompt_is_ours(existing: Option<&str>) -> bool {
    existing.is_some_and(|text| text.contains(PI_PROMPT_MARKER))
}

/// The directory under Pi's agent directory that Pi reads prompt templates from.
pub const PI_PROMPT_DIRECTORY: &str = "prompts";

/// The prompt templates written for Pi: `(file name, source)`.
///
/// Static text naming the MCP operation to call, with no policy of its own.
pub const PI_PROMPTS: &[(&str, &str)] = &[
    ("claim.md", include_str!("pi_prompts/claim.md")),
    (
        "review-status.md",
        include_str!("pi_prompts/review-status.md"),
    ),
];

/// The loop's Claude Code slash commands: `(file name, source)`.
///
/// Claude Code reads a slash command as a markdown prompt, so these carry the
/// marker and instructions and nothing executable; the `Stop` hook is what
/// continues the loop, and `estigia goal` is what stores and reports it.
pub const LOOP_COMMANDS: &[(&str, &str)] = &[
    ("flow-loop.md", FLOW_LOOP_COMMAND),
    ("flow-status.md", FLOW_STATUS_COMMAND),
];

/// The Claude Code slash command that shows `estigia changes`, written on every
/// install: `(file name, source)`.
///
/// A prompt that names the command and asks for its output unchanged; the listing
/// is the binary's and nothing here reads or decides on it.
pub const CHANGES_COMMANDS: &[(&str, &str)] = &[("estigia-changes.md", CHANGES_COMMAND)];

const CHANGES_COMMAND: &str = r#"---
description: Show the uncommitted work in every checkout of this repository
---

<!-- estigia:loop-command -->

Run

    estigia changes

and show its output to me unchanged. It lists, for each checkout of this repository, the files it holds that no commit does, the run that holds it, and `orphan` when no run does. Do not act on the listing; I decide what to do with it.
"#;

const FLOW_LOOP_COMMAND: &str = r#"---
description: Keep working this session's Estigia run until the tracker says it is done
argument-hint: [objective] [--max n]
---

<!-- estigia:loop-command -->

Begin this session's Estigia loop. Run

    estigia goal start "$ARGUMENTS" --run-id <run-id>

substituting the run id Estigia reported at session start for `<run-id>`, and read its answer. Then work the objective above: continue this session's claimed issue, and do not stop while Estigia can still open work — another issue to claim, or a blocked item whose exit condition already holds. The `Stop` hook continues the loop for you; `/flow-loop stop` ends it.
"#;

const FLOW_STATUS_COMMAND: &str = r#"---
description: Report this session's Estigia loop, its last outcome, and why it stopped
---

<!-- estigia:loop-command -->

Run

    estigia goal --run-id <run-id>

substituting the run id Estigia reported at session start for `<run-id>`, and report its output: the objective, the run id, the last outcome and, when the loop has ended, why it stopped.
"#;

/// The goal and loop plugin source: `/goal` and `/loop` for OpenCode 2, holding
/// no path to Estigia and running none of its commands.
pub const GOAL_LOOP_SOURCE: &str = include_str!("goal_loop.js");

/// The generic goal and loop extension source for Pi: `/goal` and `/loop`,
/// holding no path to Estigia and running none of its commands.
pub const PI_GOAL_LOOP_SOURCE: &str = include_str!("pi_goal_loop.js");

/// The loop plugin source: OpenCode 2 continues a session while Estigia says
/// the run still holds its issue, or another issue is claimable by it.
///
/// JavaScript rather than TypeScript, and it decides nothing: the only exits
/// are `estigia goal`'s answer and the completion cap. Completion is never a
/// value the agent writes — there is no done input and no string to produce —
/// and an answer that could not be read, or a claim answer Estigia could not
/// confirm, is retried on the wait timer up to a bounded number of attempts
/// before the loop stops and says the read was retried. V1 has no
/// `setup`/`session.execution.succeeded`, so this module simply does nothing
/// there.
pub fn loop_source(executable: &Path) -> String {
    let binary = binary_literal(executable);
    let preamble_parts = crate::harness::loop_text::preamble_parts();
    let objective_parts = crate::harness::loop_text::objective_parts();
    format!(
        r#"// {LOOP_MARKER} — managed by the `Loop plugin` setting in OpenCode's table
//
// Continues a session while Estigia says the run still holds its issue or
// another issue is claimable by it, waits while the only work left is in progress
// or in review beyond this run's claim, or blocked on a condition that may still come true, and stops
// when nothing else can open. Written by
// `estigia setup opencode` when that setting is `on`, and removed by
// `estigia setup opencode --uninstall`.
//
// Completion is not a value the agent writes: there is no done input and no
// string to produce. The only exits are `estigia goal`'s answer and the
// completion cap (`--max`). An answer that could not be read stops the loop
// rather than continuing it, as does a claim answer Estigia could not confirm —
// an unknown result is not clearance, and claimable work does not make it one.
// Nothing here throws out of a hook. Every instance of this plugin sees every
// event on the server, so one acts only on a session whose goal was
// asked in its own location; an event's own location is never consulted, since
// a turn end may arrive without one or from a worktree. A stop, and `/flow-status`, are
// admitted with `resume: false`: visible, and no model turn to re-arm the loop.
const ESTIGIA = {binary};
const RUNTIME = "opencode";
const KEY = "estigia:loop:";
const WATCH = "estigia:loop-watch:";
const RESUBSCRIBE_MS = 1000;
const DEFAULT_WAIT = 5;
const MIN_WAIT = 1;
const MAX_WAIT = 1440;
const MINUTE_MS = 60000;
const WATCHED = new Set([
  "session.created",
  "session.execution.started",
  "session.execution.succeeded",
  "session.execution.failed",
  "session.execution.interrupted",
]);
const UNCONFIRMED_ATTEMPTS = 3;

const timers = new Map();
const disposed = new WeakSet();
const stopRequested = new Map();
const deciding = new Set();
const sightings = new WeakMap();

const sighted = (ctx) => {{
  let seen = sightings.get(ctx);
  if (seen === undefined) {{
    seen = {{ born: new Map(), children: new Map() }};
    sightings.set(ctx, seen);
  }}
  return seen;
}};

const shell = () => globalThis.Bun?.$;

const PREAMBLE = {preamble_parts};
const OBJECTIVE = {objective_parts};

const fill = (text, values) =>
  Object.entries(values).reduce(
    (out, [key, value]) => out.replaceAll("{{" + key + "}}", String(value)),
    text,
  );

const preamble = (run_id, dischargeable) =>
  fill(PREAMBLE.opening, {{ run_id }}) +
  (dischargeable > 0
    ? fill(PREAMBLE.discharge, {{ n: dischargeable, s: dischargeable === 1 ? "" : "s" }})
    : "") +
  PREAMBLE.ends +
  PREAMBLE.cap +
  PREAMBLE.close;

const held = (issue, run_id) => fill(PREAMBLE.held, {{ issue, run_id }});

const ask = async (sessionID, cwd, withCompleted) => {{
  const run = shell();
  if (typeof run !== "function") {{
    return {{ read: false, why: "this host supplied no shell" }};
  }}
  let answer;
  try {{
    answer = withCompleted
      ? await run`${{ESTIGIA}} goal --json --completed --session-id ${{sessionID}} --runtime ${{RUNTIME}}`
          .cwd(cwd)
          .quiet()
          .nothrow()
      : await run`${{ESTIGIA}} goal --json --session-id ${{sessionID}} --runtime ${{RUNTIME}}`
          .cwd(cwd)
          .quiet()
          .nothrow();
  }} catch (error) {{
    return {{ read: false, why: String(error?.message ?? error) }};
  }}
  const status = typeof answer?.exitCode === "number" ? answer.exitCode : undefined;
  if (status !== 0) {{
    return {{ read: false, why: `estigia goal exited ${{status ?? "without an exit code"}}` }};
  }}
  try {{
    return {{ read: true, said: JSON.parse(String(answer?.stdout ?? "").trim()) }};
  }} catch (error) {{
    return {{ read: false, why: "estigia goal did not answer JSON" }};
  }}
}};

const clearStopAfter = async (sessionID, cwd) => {{
  const run = shell();
  if (typeof run !== "function") return;
  try {{
    await run`${{ESTIGIA}} goal stop-after --clear --session-id ${{sessionID}} --runtime ${{RUNTIME}}`
      .cwd(cwd)
      .quiet()
      .nothrow();
  }} catch (error) {{
    console.error(
      `estigia: the loop could not clear its stop-after request (${{errorText(error)}})`,
    );
  }}
}};

const parse = (text) => {{
  const cap = /(?:^|\s)--max(?:=|\s+)(\d+)/.exec(text);
  const patience = /(?:^|\s)--wait(?:=|\s+)(\S+)/.exec(text);
  const prompt = text
    .replace(/(?:^|\s)--max(?:=|\s+)\d+/, " ")
    .replace(/(?:^|\s)--wait(?:=|\s+)\S+/, " ")
    .trim();
  const minutes = patience ? Number(patience[1]) : DEFAULT_WAIT;
  const refused =
    !Number.isInteger(minutes) || minutes < MIN_WAIT || minutes > MAX_WAIT
      ? `--wait takes whole minutes from ${{MIN_WAIT}} to ${{MAX_WAIT}}, and was given ${{patience[1]}}`
      : null;
  return {{ prompt, max: cap ? Number(cap[1]) : null, wait: minutes, refused }};
}};

const enqueue = (ctx, goal, sessionID, dischargeable, issue = null) =>
  ctx.session.prompt({{
    sessionID,
    text:
      issue === null
        ? `${{preamble(goal.run_id, dischargeable)}}\n\n${{goal.prompt}}`
        : held(issue, goal.run_id),
    delivery: "queue",
  }});

const say = (ctx, sessionID, text, resume = false) =>
  ctx.session.synthetic({{
    sessionID,
    text,
    description: text.startsWith("Estigia") ? text : `Estigia loop: ${{text}}`,
    resume,
  }});

const locationOf = (ctx) => ctx?.location?.directory ?? process.cwd();

const running = (goal) =>
  goal && typeof goal === "object" && !(goal.stopped && typeof goal.stopped === "object");

const errorText = (error) => String(error?.message ?? error);

const disarm = (sessionID) => {{
  const held = timers.get(sessionID);
  if (held === undefined) return;
  clearTimeout(held.timer);
  timers.delete(sessionID);
}};

const disarmAll = (ctx) => {{
  for (const [sessionID, held] of [...timers]) if (held.ctx === ctx) disarm(sessionID);
}};

const arm = (ctx, sessionID, delay) => {{
  if (disposed.has(ctx)) return;
  disarm(sessionID);
  const timer = setTimeout(() => {{
    timers.delete(sessionID);
    void tick(ctx, sessionID);
  }}, delay);
  timers.set(sessionID, {{ ctx, timer }});
}};

const stop = async (ctx, sessionID, why, outcome = `stopped: ${{why}}`, resumable = false) => {{
  disarm(sessionID);
  const goal = await ctx.storage.get(KEY + sessionID);
  if (running(goal)) {{
    const at = new Date().toISOString();
    const {{ waiting, ...rest }} = goal;
    await ctx.storage.set(KEY + sessionID, {{
      ...rest,
      stopped: {{ reason: why, at }},
      last: {{ at, outcome }},
    }});
  }}
  console.error(`estigia: the loop for this session stopped — ${{why}}`);
  await say(ctx, sessionID, `Estigia loop stopped: ${{why}}`, resumable);
}};

const reacted = async (ctx, sessionID, outcome) => {{
  const goal = await ctx.storage.get(KEY + sessionID);
  if (running(goal)) {{
    const {{ waiting, retries, ...rest }} = goal;
    await ctx.storage.set(KEY + sessionID, {{
      ...rest,
      last: {{ at: new Date().toISOString(), outcome }},
    }});
  }}
}};

const describe = (held, pending, unadmitted) =>
  [
    held > 0 ? `${{held}} in progress or in review that this run cannot claim at the last read` : null,
    pending > 0 ? `${{pending}} blocked on a condition that has not come true` : null,
    unadmitted > 0
      ? `${{unadmitted}} open issue(s) the store has never admitted: admit each with an explicit transition`
      : null,
  ]
    .filter(Boolean)
    .join(", ");

const waited = async (ctx, sessionID, on, retries = 0) => {{
  if (disposed.has(ctx)) return;
  const goal = await ctx.storage.get(KEY + sessionID);
  if (!running(goal) || disposed.has(ctx) || stopRequested.has(sessionID)) return;
  const minutes = typeof goal.wait === "number" ? goal.wait : DEFAULT_WAIT;
  const now = Date.now();
  const at = new Date(now).toISOString();
  const since = goal.waiting?.since ?? at;
  const next = new Date(now + minutes * MINUTE_MS).toISOString();
  await ctx.storage.set(KEY + sessionID, {{
    ...goal,
    waiting: {{ since, next, on }},
    retries,
    last: {{ at, outcome: `waiting: ${{on}}` }},
  }});
  if (
    !running(await ctx.storage.get(KEY + sessionID)) ||
    disposed.has(ctx) ||
    stopRequested.has(sessionID)
  ) return;
  arm(ctx, sessionID, minutes * MINUTE_MS);
}};

const observed = async (ctx, outcome) => {{
  try {{
    await ctx.storage.set(WATCH + locationOf(ctx), {{ at: new Date().toISOString(), outcome }});
  }} catch (error) {{
    console.error(`estigia: the loop could not record what it observed (${{errorText(error)}})`);
  }}
}};

const unconfirmed = (read) =>
  !read.read ||
  read.said?.standing === "unread" ||
  read.said?.standing === "unreadable";

const describeUnconfirmed = (read) =>
  read.read
    ? `Estigia could not confirm this run's claim (${{read.said?.standing ?? "no standing"}})`
    : `the answer could not be read (${{read.why}})`;

const retryOrStop = async (ctx, sessionID, read) => {{
  const goal = await ctx.storage.get(KEY + sessionID);
  if (!running(goal)) return;
  const attempts = (typeof goal.retries === "number" ? goal.retries : 0) + 1;
  const why = describeUnconfirmed(read);
  if (attempts >= UNCONFIRMED_ATTEMPTS) {{
    await stop(
      ctx,
      sessionID,
      `${{why}} after ${{attempts - 1}} retries`,
      "stopped-unconfirmed",
      true,
    );
    return;
  }}
  await waited(ctx, sessionID, `${{why}}; retrying`, attempts);
}};

const objective = (given) =>
  given === ""
    ? OBJECTIVE.default
    : /^\/?flow(?:\s|$)/.test(given)
      ? given
      : OBJECTIVE.prefix + given;

const start = async (ctx, invocation) => {{
  const sessionID = invocation?.sessionID;
  const text = String(invocation?.prompt?.text ?? "");
  if (!sessionID) return;
  if (text.trim() === "stop") {{
    const held = await ctx.storage.get(KEY + sessionID);
    if (!running(held)) {{
      await say(ctx, sessionID, "Estigia loop: no running loop is stored for this session.");
      return;
    }}
    await halt(ctx, sessionID, "the operator stopped the loop");
    return;
  }}
  const existing = await ctx.storage.get(KEY + sessionID);
  if (running(existing)) {{
    await say(
      ctx,
      sessionID,
      "Estigia loop: this session's loop is already running, so nothing was enqueued. Use `/flow-loop stop` to end it.",
    );
    return;
  }}
  const cwd = locationOf(ctx);
  const read = await ask(sessionID, cwd);
  if (!read.read) {{
    console.error(
      `estigia: the loop could not read this run's goal (${{read.why}}), so it did not start`,
    );
    await say(ctx, sessionID, `Estigia loop did not start: the goal could not be read (${{read.why}}).`);
    return;
  }}
  const run_id = read.said?.run_id;
  if (typeof run_id !== "string" || run_id === "") {{
    console.error("estigia: the loop was given no run id, so it did not start");
    await say(ctx, sessionID, "Estigia loop did not start: Estigia gave no run id.");
    return;
  }}
  const {{ prompt: given, max, wait, refused }} = parse(text);
  if (refused) {{
    await say(ctx, sessionID, `Estigia loop did not start: ${{refused}}.`);
    return;
  }}
  const prompt = objective(given);
  stopRequested.delete(sessionID);
  if (typeof read.said?.stop_after === "string" && read.said.stop_after !== "") {{
    await clearStopAfter(sessionID, cwd);
  }}
  const goal = {{ prompt, max, wait, run_id, directory: cwd }};
  await ctx.storage.set(KEY + sessionID, goal);
  await enqueue(ctx, goal, sessionID, 0);
}};

const advance = async (ctx, event) => {{
  const sessionID = event?.data?.sessionID;
  if (typeof sessionID !== "string" || sessionID === "") {{
    await observed(ctx, "ignored: a turn end carried no session id");
    return;
  }}
  const goal = await ctx.storage.get(KEY + sessionID);
  if (!running(goal) || goal.directory !== locationOf(ctx)) return;
  await decide(ctx, sessionID);
}};

const parentOf = async (ctx, sessionID) => {{
  const known = sighted(ctx).born.get(sessionID);
  if (known !== undefined) return known;
  try {{
    const parent = (await ctx.session.get({{ sessionID }}))?.data?.parentID;
    return typeof parent === "string" && parent !== "" ? parent : undefined;
  }} catch (error) {{
    return undefined;
  }}
}};

const sight = async (ctx, event) => {{
  try {{
    await sighting(ctx, event);
  }} catch (error) {{
    console.error(`estigia: the loop could not follow a child session (${{errorText(error)}})`);
  }}
}};

const sighting = async (ctx, event) => {{
  const sessionID = event?.data?.sessionID;
  if (typeof sessionID !== "string" || sessionID === "") return;
  const seen = sighted(ctx);
  if (event.type === "session.created") {{
    const parent = event.data.parentID;
    if (typeof parent === "string" && parent !== "") seen.born.set(sessionID, parent);
    return;
  }}
  if (event.type === "session.execution.started") {{
    if (running(await ctx.storage.get(KEY + sessionID))) return;
    const parent = await parentOf(ctx, sessionID);
    if (parent !== undefined) seen.children.set(sessionID, parent);
    return;
  }}
  const parent = seen.children.get(sessionID);
  seen.children.delete(sessionID);
  seen.born.delete(sessionID);
  if (parent === undefined) return;
  if ([...seen.children.values()].includes(parent)) return;
  const goal = await ctx.storage.get(KEY + parent);
  if (!running(goal) || goal.directory !== locationOf(ctx) || !goal.waiting) return;
  await guarded(ctx, parent, () => decide(ctx, parent));
}};

const decide = async (ctx, sessionID, lapsed = false) => {{
  if (deciding.has(sessionID)) return;
  deciding.add(sessionID);
  try {{
    try {{
      await consider(ctx, sessionID, lapsed);
    }} finally {{
      const why = stopRequested.get(sessionID);
      if (why !== undefined) {{
        try {{
          await stop(ctx, sessionID, why);
        }} finally {{
          stopRequested.delete(sessionID);
        }}
      }}
    }}
  }} finally {{
    deciding.delete(sessionID);
  }}
}};

const halt = async (ctx, sessionID, why) => {{
  if (deciding.has(sessionID)) {{
    stopRequested.set(sessionID, why);
    await say(ctx, sessionID, "Estigia loop: stopping as soon as the decision in flight finishes.");
    return;
  }}
  deciding.add(sessionID);
  try {{
    await stop(ctx, sessionID, why);
  }} finally {{
    stopRequested.delete(sessionID);
    deciding.delete(sessionID);
  }}
}};

const consider = async (ctx, sessionID, lapsed) => {{
  const goal = await ctx.storage.get(KEY + sessionID);
  if (!running(goal)) return;
  if (goal.directory !== locationOf(ctx)) return;
  const cwd = typeof goal.directory === "string" ? goal.directory : locationOf(ctx);
  const capped = typeof goal.max === "number";
  const read = await ask(sessionID, cwd, capped);
  if (unconfirmed(read)) {{
    await retryOrStop(ctx, sessionID, read);
    return;
  }}
  if (capped) {{
    const completed = read.said?.completed;
    if (typeof completed !== "number") {{
      await stop(
        ctx,
        sessionID,
        "Estigia did not report how many issues this run has completed",
      );
      return;
    }}
    if (completed >= goal.max) {{
      await stop(ctx, sessionID, `the completion cap (${{goal.max}}) was reached`);
      return;
    }}
  }}
  let dischargeable = 0;
  let issue = null;
  if (read.said?.live === true) {{
    const named = read.said?.issue;
    if (named === null || named === undefined || named === "") {{
      await stop(ctx, sessionID, "Estigia did not report which issue this run holds");
      return;
    }}
    issue = String(named);
  }} else {{
    if (typeof read.said?.stop_after === "string" && read.said.stop_after !== "") {{
      await stop(
        ctx,
        sessionID,
        `the agent asked to stop after the current task (${{read.said.stop_after}})`,
        "stopped-on-request",
      );
      await clearStopAfter(sessionID, cwd);
      return;
    }}
    const standing = read.said?.standing;
    if (standing !== "stopped" && standing !== "nothing-sworn") {{
      await stop(
        ctx,
        sessionID,
        `Estigia could not confirm this run's claim (${{standing ?? "no standing"}})`,
      );
      return;
    }}
    const claimable = read.said?.claimable;
    const discharged = read.said?.dischargeable;
    if (typeof claimable !== "number" || typeof discharged !== "number") {{
      await stop(
        ctx,
        sessionID,
        "Estigia did not report how much work this run's queues hold",
      );
      return;
    }}
    if (claimable + discharged <= 0) {{
      const {{ held, pending, unconditioned, unadmitted }} = read.said;
      const opens =
        (typeof held === "number" ? held : 0) +
        (typeof pending === "number" ? pending : 0) +
        (typeof unadmitted === "number" ? unadmitted : 0);
      if (opens > 0) {{
        await waited(ctx, sessionID, describe(held, pending, unadmitted));
        return;
      }}
      const unread = held === null || pending === null;
      await stop(
        ctx,
        sessionID,
        "Estigia reports no live claim and no claimable or dischargeable work" +
          (unread ? " (what is held or pending could not be read)" : "") +
          (typeof unconditioned === "number" && unconditioned > 0
            ? `; ${{unconditioned}} blocked with no recorded condition, or one that is not well formed, are left for a person`
            : ""),
      );
      return;
    }}
    dischargeable = discharged;
  }}
  if (!running(await ctx.storage.get(KEY + sessionID)) || stopRequested.has(sessionID)) return;
  const awaited = [...sighted(ctx).children]
    .filter(([, parent]) => parent === sessionID)
    .map(([child]) => child);
  if (awaited.length > 0 && !lapsed) {{
    await waited(
      ctx,
      sessionID,
      `${{awaited.length}} child session(s) still running: ${{awaited.join(", ")}}`,
    );
    return;
  }}
  disarm(sessionID);
  await enqueue(ctx, goal, sessionID, dischargeable, issue);
  await reacted(ctx, sessionID, "enqueued");
}};

const guarded = async (ctx, sessionID, work) => {{
  try {{
    await work();
  }} catch (error) {{
    const why = errorText(error);
    console.error(`estigia: the loop could not continue (${{why}})`);
    if (typeof sessionID !== "string" || sessionID === "") {{
      await observed(ctx, `failed: ${{why}}`);
      return;
    }}
    try {{
      await stop(ctx, sessionID, `the loop failed (${{why}})`, `failed: ${{why}}`);
    }} catch (recording) {{
      console.error(`estigia: the loop could not record its failure (${{errorText(recording)}})`);
    }}
  }}
}};

const react = (ctx, event) =>
  guarded(ctx, event?.data?.sessionID, async () => {{
    await sight(ctx, event);
    if (event.type === "session.execution.succeeded") await advance(ctx, event);
  }});

const tick = (ctx, sessionID) =>
  guarded(ctx, sessionID, () => decide(ctx, sessionID, true));

const rearm = async (ctx) => {{
  try {{
    let after;
    do {{
      const page = await ctx.storage.scan({{ prefix: KEY, after }});
      for (const entry of page?.entries ?? []) {{
        const goal = entry?.value;
        if (!running(goal) || !goal.waiting || typeof goal.waiting !== "object") continue;
        if (goal.directory !== locationOf(ctx)) continue;
        const due = Date.parse(goal.waiting.next);
        arm(ctx, String(entry.key).slice(KEY.length), Number.isFinite(due) ? Math.max(0, due - Date.now()) : 0);
      }}
      after = page?.next;
    }} while (after);
  }} catch (error) {{
    await observed(ctx, `failed: could not re-arm waiting loops (${{errorText(error)}})`);
  }}
}};

const pause = (signal) =>
  new Promise((resolve) => {{
    const timer = setTimeout(resolve, RESUBSCRIBE_MS);
    signal.addEventListener("abort", () => {{
      clearTimeout(timer);
      resolve();
    }}, {{ once: true }});
  }});

const report = async (ctx, invocation) => {{
  const sessionID = invocation?.sessionID;
  if (!sessionID) return;
  if (String(invocation?.prompt?.text ?? "").trim() !== "") {{
    await say(
      ctx,
      sessionID,
      "/flow-status takes no argument. To set a completion condition use /goal, and to run the Estigia loop use /flow-loop.",
    );
    return;
  }}
  const goal = await ctx.storage.get(KEY + sessionID);
  const lines = [];
  if (!goal || typeof goal !== "object") {{
    lines.push(
      "Estigia goal: no loop goal is stored for this session — the loop never started here, or it stopped before this record existed.",
    );
  }} else {{
    lines.push(`Estigia goal: ${{goal.prompt}}`);
    lines.push(`Loop run id: ${{goal.run_id}}`);
    if (typeof goal.directory === "string") lines.push(`Asked in: ${{goal.directory}}`);
    if (typeof goal.max === "number") lines.push(`Completion cap: ${{goal.max}}`);
    if (typeof goal.wait === "number") lines.push(`Re-ask interval while waiting: ${{goal.wait}} minutes`);
    if (goal.stopped && typeof goal.stopped === "object") {{
      lines.push(`The loop stopped at ${{goal.stopped.at}} — ${{goal.stopped.reason}}`);
    }} else {{
      lines.push("No stop is recorded for this loop.");
    }}
    if (goal.waiting && typeof goal.waiting === "object" && running(goal)) {{
      lines.push(
        `Waiting since ${{goal.waiting.since}} on ${{goal.waiting.on}}; the next re-ask is at ${{goal.waiting.next}}.`,
      );
    }}
    if (goal.last && typeof goal.last === "object") {{
      lines.push(`Last turn end observed at ${{goal.last.at}}: ${{goal.last.outcome}}`);
    }} else {{
      lines.push("No turn end has been observed for this loop yet.");
    }}
    const watched = await ctx.storage.get(WATCH + locationOf(ctx));
    if (watched && typeof watched === "object") {{
      lines.push(`Last thing this plugin instance observed at ${{watched.at}}: ${{watched.outcome}}`);
    }}
    const read = await ask(
      sessionID,
      typeof goal.directory === "string" ? goal.directory : locationOf(ctx),
    );
    lines.push(
      read.read
        ? `Tracker answer: ${{JSON.stringify(read.said)}}`
        : `Tracker answer could not be read (${{read.why}}).`,
    );
    if (
      read.read &&
      typeof read.said?.stop_after === "string" &&
      read.said.stop_after !== ""
    ) {{
      lines.push(`A stop-after request is pending: ${{read.said.stop_after}}`);
    }}
  }}
  await say(ctx, sessionID, lines.join("\n"));
}};

export default {{
  id: "flow-loop",
  async setup(ctx) {{
    const controller = new AbortController();
    await ctx.command.transform((editor) =>
      editor.add({{
        name: "flow-loop",
        description: "Keep working this session's Estigia run until the tracker says it is done",
        execute: async (invocation) => {{
          try {{
            await start(ctx, invocation);
          }} catch (error) {{
            console.error(`estigia: the loop could not start (${{String(error?.message ?? error)}})`);
          }}
        }},
      }}),
    );
    await ctx.command.transform((editor) =>
      editor.add({{
        name: "flow-status",
        description: "Report this session's Estigia goal, and why its loop stopped",
        execute: async (invocation) => {{
          try {{
            await report(ctx, invocation);
          }} catch (error) {{
            console.error(`estigia: the goal could not be reported (${{String(error?.message ?? error)}})`);
          }}
        }},
      }}),
    );
    const watch = async () => {{
      while (!controller.signal.aborted) {{
        let why = "the event stream ended";
        try {{
          for await (const event of ctx.event.subscribe({{ signal: controller.signal }})) {{
            if (!WATCHED.has(event?.type)) continue;
            await react(ctx, event);
          }}
        }} catch (error) {{
          why = `the event stream failed (${{errorText(error)}})`;
        }}
        if (controller.signal.aborted) return;
        console.error(`estigia: ${{why}}; subscribing again`);
        await observed(ctx, `interrupted: ${{why}}; subscribed again after ${{RESUBSCRIBE_MS}} ms`);
        await pause(controller.signal);
      }}
    }};
    watch();
    await rearm(ctx);
    return () => {{
      controller.abort();
      disposed.add(ctx);
      disarmAll(ctx);
    }};
  }},
}};
"#
    )
}

/// The hook script Cline runs, in the language its launcher speaks.
///
/// Cline discovers hooks by file name rather than a settings entry, so this is
/// a file Estigia owns whole — and it comes in two languages, because the
/// launcher runs it through PowerShell on Windows and the shell everywhere
/// else. It does nothing but hand its input to Estigia and print what comes
/// back; a decision here would be a second place the gate lives.
pub fn cline_hook(executable: &Path, windows: bool) -> String {
    // Quoted through the helpers, not `display()` between two quote
    // characters: an apostrophe in the path closed the quoting early. See
    // `paths::powershell_quoted` and `setup::UNQUOTABLE`.
    let path = if windows {
        crate::paths::powershell_quoted(executable)
    } else {
        crate::paths::shell_quoted(executable)
    };
    // From the one place that decides what a gate is invoked with; this file
    // once carried its own copy and missed the `--agent` round.
    let arguments = super::render::hook_arguments("cline", crate::harness::hook::Dialect::Cline);
    // The status is read and said, not inherited and swallowed: this dialect
    // exits `0` even to refuse, so any other status means Estigia did not
    // answer, and `exec` handed that back looking like an approval. A hook
    // that breaks does not deny — but it says so.
    if windows {
        [
            &format!("# {MARKER} — managed by `estigia setup cline`"),
            "$ErrorActionPreference = 'Continue'",
            // Both encodings, or the payload is mangled: Windows PowerShell
            // defaults `$OutputEncoding` to `us-ascii`, and
            // `[Console]::In.ReadToEnd()` decodes with `[Console]::InputEncoding`,
            // the OEM code page. Measured, not reasoned; see
            // `setup::tests` for the transcript.
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

        let windows = cline_hook(awkward, true);
        assert!(
            windows.contains("'/home/o''brien/estigia'"),
            "the apostrophe was not doubled for PowerShell: {windows}"
        );
        assert!(
            !windows.contains(r"'\''"),
            "PowerShell was handed a POSIX escape: {windows}"
        );

        // A gate that cannot run says so, in both languages: the script must
        // read the status, must not let a non-zero one reach the agent, and
        // must put the word `estigia` on standard error when it gets one.
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

        let plain = cline_hook(Path::new("/usr/local/bin/estigia"), false);
        assert!(
            plain.contains("'/usr/local/bin/estigia' hook pre-tool-use"),
            "{plain}"
        );
    }
}
