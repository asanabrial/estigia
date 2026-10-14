// __MARKER__ — managed by `estigia setup pi`
//
// Runs a delegated worker as a separate `pi --mode rpc` process, draws one row per
// worker above the editor, opens a full view of them, keeps what finished as session
// entries the model is never sent, and stops or steers one on request. It decides nothing: the child
// inherits the environment the gate extension exported, so every write it makes
// is decided by the binary as the parent's run. Removed by `estigia uninstall pi`.
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

const PI_COMMAND = "__PI__";
const TOOL = "__TOOL__";
const COMMAND = "estigia-agents";
const SHORTCUT = "ctrl+alt+a";
const HISTORY = "estigia-agent";
const LOG_LINES = 200;
const LOG_WIDTH = 2000;
const TRANSCRIPT_ROWS = 20;
const ANSWER_KEPT = 4000;
const WIDGET = "estigia-agents";
const ROOT_SESSION = "ESTIGIA_PI_PARENT_SESSION";
const GATE_COMMAND = "estigia-doctor";
const KEEP_FINISHED_MS = 60000;
const KEEP_FINISHED_ROWS = 3;
const SETUP_TIMEOUT_MS = 30000;
const KILL_GRACE_MS = 3000;
const REDRAW_MS = 1000;
const LISTING_WIDTH = 120;
const DIALOGS = ["select", "confirm", "input", "editor"];

const runs = new Map();
const restored = [];
let host;
let counter = 0;
let sessionDir;
let latest;
let ticker;

const filled = (value) => typeof value === "string" && value.trim() !== "";

const agentDirectory = () => process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent");

const definitionOf = (name) => {
  if (!/^[A-Za-z0-9_-]+$/.test(name)) return undefined;
  const file = join(agentDirectory(), "agents", `${name}.md`);
  if (!existsSync(file)) return undefined;
  const text = readFileSync(file, "utf8").replace(/^﻿/, "");
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/.exec(text);
  if (!match) return { prompt: text.trim() };
  const field = (key) => new RegExp(`^${key}:[ \\t]*(.+)$`, "m").exec(match[1])?.[1]?.trim();
  return { model: field("model"), tools: field("tools"), prompt: match[2].trim() };
};

const clock = (ms) => {
  const seconds = Math.floor(ms / 1000);
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, "0")}s`;
};

const compact = (count) => (count >= 1000 ? `${(count / 1000).toFixed(1)}k` : String(count));

const fit = (line, width) => (line.length > width ? `${line.slice(0, Math.max(0, width - 1))}…` : line);

const textOf = (content) =>
  (Array.isArray(content) ? content : [])
    .filter((part) => part?.type === "text" && typeof part.text === "string")
    .map((part) => part.text)
    .join("");

const note = (run, line) => {
  run.log.push(String(line).slice(0, LOG_WIDTH));
  if (run.log.length > LOG_LINES) run.log.shift();
};

const rowText = (run, now) => {
  const mark = run.status === "running" ? "●" : run.status === "done" ? "✓" : "✗";
  const elapsed = clock((run.endedAt ?? now) - run.startedAt);
  return `${mark} ${run.label} (${run.agent}) ${run.model ?? "…"} ${compact(run.tokens)} tok $${run.cost.toFixed(4)} ${elapsed}`;
};

const rowsOf = (now) => {
  const finished = [...runs.values()]
    .filter((run) => run.status !== "running" && now - run.endedAt < KEEP_FINISHED_MS)
    .sort((a, b) => b.endedAt - a.endedAt)
    .slice(0, KEEP_FINISHED_ROWS);
  return [...[...runs.values()].filter((run) => run.status === "running"), ...finished];
};

const render = (width) => {
  const now = Date.now();
  const rows = rowsOf(now);
  if (rows.length === 0) return [];
  const lines = ["estigia agents"];
  for (const run of rows) {
    const head = rowText(run, now);
    lines.push(fit(run.activity ? `${head}  ${run.activity.split("\n")[0]}` : head, width));
    if (run.refusal) for (const line of run.refusal.split("\n").slice(1)) lines.push(fit(`    ${line}`, width));
  }
  return lines;
};

const redraw = () => {
  const ui = latest?.ui;
  if (!latest?.hasUI || typeof ui?.setWidget !== "function") return;
  if (rowsOf(Date.now()).length === 0) {
    ui.setWidget(WIDGET, undefined);
    return;
  }
  ui.setWidget(WIDGET, (tui) => {
    redraw.tui = tui;
    return { render, invalidate() {} };
  });
  redraw.tui?.requestRender?.();
};

const tick = () => {
  redraw();
  if (![...runs.values()].some((run) => run.status === "running") && rowsOf(Date.now()).length === 0) {
    clearInterval(ticker);
    ticker = undefined;
  }
};

const remember = (run, text) => {
  const { id, agent, label, status, model, tokens, cost, startedAt, endedAt, task } = run;
  try {
    host.appendEntry(HISTORY, { id, agent, label, status, model, tokens, cost, startedAt, endedAt, task, text: text.slice(0, ANSWER_KEPT) });
  } catch {}
};

const finish = (run, status, resolve, text) => {
  if (run.status !== "running") return;
  run.status = status;
  run.endedAt = Date.now();
  remember(run, text);
  clearTimeout(run.setupTimer);
  try {
    run.child.stdin.end();
  } catch {}
  redraw();
  resolve({ content: [{ type: "text", text }], details: { id: run.id, agent: run.agent, status } });
};

const send = (run, record) => {
  try {
    run.child.stdin.write(`${JSON.stringify(record)}\n`);
  } catch {}
};

const stop = (run) => {
  if (run.status !== "running") return;
  const signal = (name) => {
    try {
      if (process.platform === "win32") run.child.kill(name);
      else process.kill(-run.child.pid, name);
    } catch {}
  };
  signal("SIGTERM");
  setTimeout(() => signal("SIGKILL"), KILL_GRACE_MS).unref();
  run.stopped = true;
};

const onEvent = (run, event, resolve) => {
  if (event.type === "response") {
    if (event.id === "commands") {
      clearTimeout(run.setupTimer);
      const commands = event.data?.commands ?? [];
      if (!event.success || !commands.some((c) => c?.name === GATE_COMMAND && c?.source === "extension")) {
        stop(run);
        finish(run, "failed", resolve, "estigia: the child did not load the gate extension, so no work was sent to it");
        return;
      }
      send(run, { id: "state", type: "get_state" });
      send(run, { id: "prompt", type: "prompt", message: run.task });
    } else if (event.id === "state" && event.success) {
      const model = event.data?.model;
      run.model = model?.id ?? model?.name ?? run.model;
    } else if (event.id === "prompt" && !event.success) {
      stop(run);
      finish(run, "failed", resolve, `estigia: the child refused the task: ${event.error ?? "no reason given"}`);
    }
    return;
  }
  if (event.type === "message_update" && event.usage) {
    run.pending = { tokens: event.usage.totalTokens ?? 0, cost: event.usage.cost?.total ?? 0 };
  } else if (event.type === "message_end") {
    const usage = event.message?.usage;
    run.pending = undefined;
    if (usage) {
      run.settledTokens += usage.totalTokens ?? 0;
      run.settledCost += usage.cost?.total ?? 0;
    }
    if (event.message?.role === "assistant") {
      const said = textOf(event.message.content);
      if (said) {
        run.text = said;
        note(run, `assistant: ${said}`);
      }
    }
  } else if (event.type === "tool_execution_start") {
    run.activity = `${event.toolName} ${JSON.stringify(event.args ?? {})}`;
    note(run, `tool: ${run.activity}`);
    run.refusal = undefined;
  } else if (event.type === "tool_execution_end") {
    const said = textOf(event.result?.content);
    note(run, `${event.isError ? "refused" : "result"}: ${said || event.toolName}`);
    if (event.isError && said) {
      run.refusal = said;
      run.activity = said;
    } else {
      run.activity = event.toolName;
    }
  } else if (event.type === "extension_ui_request" && DIALOGS.includes(event.method)) {
    send(run, { type: "extension_ui_response", id: event.id, cancelled: true });
  } else if (event.type === "agent_settled") {
    finish(run, "done", resolve, run.text || "estigia: the child finished without saying anything");
    return;
  }
  run.tokens = run.settledTokens + (run.pending?.tokens ?? 0);
  run.cost = run.settledCost + (run.pending?.cost ?? 0);
  redraw();
};

const launch = (params, ctx, signal) =>
  new Promise((resolve) => {
    const refuse = (text) => resolve({ content: [{ type: "text", text }], details: { status: "refused" } });
    const agent = String(params?.agent ?? "");
    const task = String(params?.task ?? "");
    if (!filled(task)) return refuse("estigia: a subagent needs a task");
    const definition = definitionOf(agent);
    if (!definition) return refuse(`estigia: no agent definition ${agent || "(none)"} in ${join(agentDirectory(), "agents")}`);
    if (!filled(process.env[ROOT_SESSION])) {
      return refuse(
        `estigia: the gate extension has not exported ${ROOT_SESSION}, so a child could not be linked to this session's claim and was not launched`,
      );
    }
    sessionDir ??= mkdtempSync(join(tmpdir(), "estigia-agents-"));
    const args = ["--mode", "rpc", "--session-dir", sessionDir];
    if (filled(definition.model) && definition.model !== "inherit") args.push("--model", definition.model);
    if (filled(definition.tools)) args.push("--tools", definition.tools);
    if (filled(definition.prompt)) args.push("--append-system-prompt", definition.prompt);
    counter += 1;
    const run = {
      id: `a${counter}`,
      agent,
      task,
      label: filled(params?.label) ? String(params.label) : agent,
      status: "running",
      startedAt: Date.now(),
      tokens: 0,
      cost: 0,
      settledTokens: 0,
      settledCost: 0,
      text: "",
      log: [],
    };
    note(run, `task: ${task}`);
    let child;
    try {
      child = spawn(PI_COMMAND, args, {
        cwd: ctx?.cwd ?? process.cwd(),
        detached: process.platform !== "win32",
        stdio: ["pipe", "pipe", "ignore"],
        windowsHide: true,
      });
    } catch (error) {
      return refuse(`estigia: could not start ${PI_COMMAND}: ${String(error?.message ?? error)}`);
    }
    run.child = child;
    runs.set(run.id, run);
    latest = ctx;
    ticker ??= setInterval(tick, REDRAW_MS);
    ticker.unref?.();
    let buffered = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      buffered += chunk;
      let at;
      while ((at = buffered.indexOf("\n")) >= 0) {
        const line = buffered.slice(0, at).replace(/\r$/, "");
        buffered = buffered.slice(at + 1);
        if (line === "") continue;
        try {
          onEvent(run, JSON.parse(line), resolve);
        } catch {}
      }
    });
    child.stdin.on("error", () => {});
    child.on("error", (error) => finish(run, "failed", resolve, `estigia: could not run ${PI_COMMAND}: ${String(error?.message ?? error)}`));
    child.on("close", (code, name) => {
      const how = run.stopped ? "stopped" : "failed";
      finish(run, how, resolve, run.stopped ? "estigia: the child was stopped" : `estigia: the child exited (${name ?? code}) before it settled`);
    });
    run.setupTimer = setTimeout(() => {
      stop(run);
      finish(run, "failed", resolve, "estigia: the child did not answer within the setup deadline");
    }, SETUP_TIMEOUT_MS);
    run.setupTimer.unref?.();
    signal?.addEventListener?.("abort", () => stop(run), { once: true });
    send(run, { id: "commands", type: "get_commands" });
    redraw();
  });

const items = () => {
  const all = [...runs.values()];
  const live = all.filter((run) => run.status === "running");
  const done = all.filter((run) => run.status !== "running").sort((a, b) => b.endedAt - a.endedAt);
  return [...live, ...done, ...[...restored].reverse()];
};

const transcriptOf = (item) =>
  (item.log ?? [`task: ${item.task}`, `assistant: ${item.text}`]).flatMap((entry) => entry.split("\n"));

const viewLines = (at, width) => {
  const all = items();
  const lines = ["estigia agents  up/down select  s stop  x stop all  f steer  q close"];
  if (all.length === 0) return [...lines, "no subagents yet"];
  const chosen = Math.min(at, all.length - 1);
  const now = Date.now();
  all.forEach((item, index) => lines.push(fit(`${index === chosen ? ">" : " "} ${item.id} ${rowText(item, now)}`, width)));
  lines.push("");
  for (const line of transcriptOf(all[chosen]).slice(-TRANSCRIPT_ROWS)) lines.push(fit(line, width));
  return lines;
};

const stopConfirmed = async (targets, ctx) => {
  if (ctx?.hasUI && typeof ctx.ui.confirm === "function") {
    if (!(await ctx.ui.confirm("Stop subagents", `Stop ${targets.length} running subagent(s)?`))) return;
  }
  for (const run of targets) stop(run);
};

const steer = async (run, ctx) => {
  if (run?.status !== "running" || typeof ctx?.ui?.input !== "function") return;
  const message = await ctx.ui.input(`Follow-up for ${run.label}`, "");
  if (!filled(message)) return;
  send(run, { type: "steer", message });
  note(run, `you (sent, not confirmed): ${message}`);
};

const openView = (ctx) =>
  ctx.ui.custom(
    (tui, _theme, _keys, done) => {
      let at = 0;
      const timer = setInterval(() => tui.requestRender?.(), REDRAW_MS);
      timer.unref?.();
      const close = () => {
        clearInterval(timer);
        done(undefined);
      };
      const press = async (data) => {
        const all = items();
        const chosen = all[Math.min(at, all.length - 1)];
        if (data === "\u001b[A" || data === "k") at = Math.max(0, at - 1);
        else if (data === "\u001b[B" || data === "j") at = Math.min(Math.max(0, all.length - 1), at + 1);
        else if (data === "s" && chosen) stop(chosen);
        else if (data === "x") await stopConfirmed(all.filter((run) => run.status === "running"), ctx);
        else if (data === "f") await steer(chosen, ctx);
        else if (data === "q" || data === "\u001b") close();
        tui.requestRender?.();
      };
      return { render: (width) => viewLines(at, width), invalidate() {}, handleInput: (data) => press(data) };
    },
    { overlay: true },
  );

const handle = async (args, ctx) => {
  const words = String(args ?? "").trim().split(/\s+/).filter(Boolean);
  const say = (text) => (ctx?.hasUI ? ctx.ui.notify(text, "info") : console.log(text));
  const live = [...runs.values()].filter((run) => run.status === "running");
  if (words[0] !== "stop") {
    if (ctx?.hasUI && typeof ctx.ui.custom === "function") return openView(ctx);
    say(rowsOf(Date.now()).length ? render(LISTING_WIDTH).join("\n") : "estigia: no subagents are running");
    return;
  }
  const targets = words[1] === "all" ? live : live.filter((run) => run.id === words[1]);
  if (targets.length === 0) return say("estigia: nothing to stop; name an id or `all`");
  if (words[1] === "all") return stopConfirmed(targets, ctx);
  for (const run of targets) stop(run);
};

export default function (pi) {
  host = pi;
  pi.on("session_start", (_event, ctx) => {
    restored.length = 0;
    for (const entry of ctx?.sessionManager?.getEntries?.() ?? []) {
      if (entry?.type === "custom" && entry.customType === HISTORY && entry.data) restored.push(entry.data);
    }
  });
  pi.registerShortcut(SHORTCUT, { description: "Open the Estigia subagent view", handler: (ctx) => openView(ctx) });
  pi.registerTool({
    name: TOOL,
    label: "Estigia subagent",
    description:
      "Runs one delegated worker (an agent definition setup wrote, such as developer or analyst) as a separate Pi process and returns its final answer when it settles. Its writes are decided by Estigia as this session's run.",
    parameters: {
      type: "object",
      properties: {
        agent: { type: "string", description: "The agent definition name." },
        task: { type: "string", description: "What the worker is to do." },
        label: { type: "string", description: "A short name shown on its card." },
      },
      required: ["agent", "task"],
      additionalProperties: false,
    },
    execute: (_id, params, signal, _update, ctx) => launch(params, ctx, signal),
  });
  pi.registerCommand(COMMAND, {
    description: "Open the subagent view, or `stop <id|all>`",
    handler: handle,
  });
}
