// __MARKER__ — managed by the `Loop plugin` setting in Pi's table
//
// Continues a session while Estigia says the run still holds its issue or another
// issue is claimable by it, waits while the only work left is held or blocked on a
// condition that may still come true, and stops when nothing else can open.
// Written by `estigia setup pi` when that setting is `on`, removed by
// `estigia setup pi --uninstall`.
//
// Completion is not a value the agent writes. The only exits are `estigia goal`'s
// answer and the completion cap (`--max`). An answer that did not come — a timeout,
// a binary that could not be run, no exit code — stops the loop and says so; an
// answer that came but could not be read, or a claim Estigia could not confirm, is
// retried on the wait timer before the loop stops. Nothing here throws out of a hook.
import { spawn } from "node:child_process";

const ESTIGIA = __BINARY__;
const RUNTIME = "pi";
const MESSAGE_TYPE = "estigia-loop";
const ASK_TIMEOUT_MS = 30000;
const DEFAULT_WAIT = 5;
const MIN_WAIT = 1;
const MAX_WAIT = 1440;
const MINUTE_MS = 60000;
const UNCONFIRMED_ATTEMPTS = 3;

const PREAMBLE = __PREAMBLE__;
const OBJECTIVE = __OBJECTIVE__;

const loops = new Map();
const timers = new Map();
const deciding = new Set();
const stopRequested = new Map();

const fill = (text, values) =>
  Object.entries(values).reduce(
    (out, [key, value]) => out.replaceAll("{" + key + "}", String(value)),
    text,
  );

const preamble = (run_id, dischargeable, capped) =>
  fill(PREAMBLE.opening, { run_id }) +
  (dischargeable > 0
    ? fill(PREAMBLE.discharge, { n: dischargeable, s: dischargeable === 1 ? "" : "s" })
    : "") +
  PREAMBLE.ends +
  (capped ? PREAMBLE.cap : "") +
  PREAMBLE.close;

const held = (issue, run_id) => fill(PREAMBLE.held, { issue, run_id });

const objective = (given) =>
  given === ""
    ? OBJECTIVE.default
    : /^\/?flow(?:\s|$)/.test(given)
      ? given
      : OBJECTIVE.prefix + given;

const errorText = (error) => String(error?.message ?? error);

const sessionOf = (ctx) => {
  try {
    const id = ctx?.sessionManager?.getSessionId?.();
    return typeof id === "string" && id.trim() !== "" ? id : undefined;
  } catch {
    return undefined;
  }
};

const isChild = (ctx) => {
  try {
    return Boolean(ctx?.sessionManager?.getHeader?.()?.parentSession);
  } catch {
    return false;
  }
};

const idle = (ctx) => {
  try {
    return ctx.isIdle() === true;
  } catch {
    return false;
  }
};

const say = (ctx, text, level = "info") => {
  try {
    ctx.ui.notify(text, level);
  } catch {
    console.error(`estigia: ${text}`);
  }
};

const running = (goal) => Boolean(goal) && !goal.stopped;

const ask = (session, cwd, withCompleted) =>
  new Promise((resolve) => {
    const args = ["goal", "--json"];
    if (withCompleted) args.push("--completed");
    args.push("--session-id", session, "--runtime", RUNTIME);
    let settled = false;
    let timer;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(value);
    };
    const silent = (why) => finish({ read: false, silent: true, why });
    let child;
    try {
      child = spawn(ESTIGIA, args, {
        cwd,
        shell: false,
        windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"],
      });
    } catch (error) {
      silent(`estigia could not be run (${errorText(error)})`);
      return;
    }
    let out = "";
    child.stdout.on("data", (chunk) => {
      out += chunk;
    });
    child.stderr.on("data", () => {});
    child.on("error", (error) => silent(`estigia could not be run (${errorText(error)})`));
    timer = setTimeout(() => {
      try {
        child.kill();
      } catch {}
      silent(`estigia goal gave no answer within ${ASK_TIMEOUT_MS / 1000} seconds`);
    }, ASK_TIMEOUT_MS);
    child.on("close", (status, signal) => {
      if (typeof status !== "number") {
        silent(`estigia goal ended with no exit code${signal ? `, signal ${signal}` : ""}`);
        return;
      }
      if (status !== 0) {
        finish({ read: false, silent: false, why: `estigia goal exited ${status}` });
        return;
      }
      try {
        finish({ read: true, said: JSON.parse(out.trim()) });
      } catch {
        finish({ read: false, silent: false, why: "estigia goal did not answer JSON" });
      }
    });
  });

const clearStopAfter = async (session, cwd) => {
  const args = ["goal", "stop-after", "--clear", "--session-id", session, "--runtime", RUNTIME];
  await new Promise((resolve) => {
    try {
      const child = spawn(ESTIGIA, args, { cwd, shell: false, windowsHide: true, stdio: "ignore" });
      const timer = setTimeout(() => {
        try {
          child.kill();
        } catch {}
        resolve();
      }, ASK_TIMEOUT_MS);
      child.on("error", () => {
        clearTimeout(timer);
        resolve();
      });
      child.on("close", () => {
        clearTimeout(timer);
        resolve();
      });
    } catch {
      resolve();
    }
  });
};

const parse = (text) => {
  const cap = /(?:^|\s)--max(?:=|\s+)(\d+)/.exec(text);
  const patience = /(?:^|\s)--wait(?:=|\s+)(\S+)/.exec(text);
  const prompt = text
    .replace(/(?:^|\s)--max(?:=|\s+)\d+/, " ")
    .replace(/(?:^|\s)--wait(?:=|\s+)\S+/, " ")
    .trim();
  const minutes = patience ? Number(patience[1]) : DEFAULT_WAIT;
  const refused =
    !Number.isInteger(minutes) || minutes < MIN_WAIT || minutes > MAX_WAIT
      ? `--wait takes whole minutes from ${MIN_WAIT} to ${MAX_WAIT}, and was given ${patience[1]}`
      : null;
  return { prompt, max: cap ? Number(cap[1]) : null, wait: minutes, refused };
};

const disarm = (session) => {
  const held = timers.get(session);
  if (held === undefined) return;
  clearTimeout(held);
  timers.delete(session);
};

const stop = (ctx, session, why, outcome = `stopped: ${why}`, quiet = false) => {
  disarm(session);
  const goal = loops.get(session);
  if (running(goal)) {
    const at = new Date().toISOString();
    delete goal.waiting;
    goal.stopped = { reason: why, at };
    goal.last = { at, outcome };
  }
  console.error(`estigia: the loop for this session stopped — ${why}`);
  if (!quiet) say(ctx, `Estigia loop stopped: ${why}`, "warning");
};

const reacted = (goal, outcome) => {
  delete goal.waiting;
  delete goal.retries;
  goal.last = { at: new Date().toISOString(), outcome };
};

const same = (ctx, session) => {
  if (sessionOf(ctx) === session) return true;
  stop(ctx, session, "the session changed while the loop was deciding", undefined, true);
  return false;
};

const describe = (held, pending, unadmitted) =>
  [
    held > 0 ? `${held} in progress or in review that this run cannot claim at the last read` : null,
    pending > 0 ? `${pending} blocked on a condition that has not come true` : null,
    unadmitted > 0
      ? `${unadmitted} open issue(s) the store has never admitted: admit each with an explicit transition`
      : null,
  ]
    .filter(Boolean)
    .join(", ");

const arm = (pi, ctx, session, delay) => {
  disarm(session);
  timers.set(
    session,
    setTimeout(() => {
      timers.delete(session);
      void tick(pi, ctx, session);
    }, delay),
  );
};

const waited = (pi, ctx, session, on, retries = 0) => {
  const goal = loops.get(session);
  if (!running(goal) || stopRequested.has(session)) return;
  const minutes = typeof goal.wait === "number" ? goal.wait : DEFAULT_WAIT;
  const now = Date.now();
  const at = new Date(now).toISOString();
  goal.waiting = {
    since: goal.waiting?.since ?? at,
    next: new Date(now + minutes * MINUTE_MS).toISOString(),
    on,
  };
  goal.retries = retries;
  goal.last = { at, outcome: `waiting: ${on}` };
  arm(pi, ctx, session, minutes * MINUTE_MS);
};

const unconfirmed = (read) =>
  !read.read || read.said?.standing === "unread" || read.said?.standing === "unreadable";

const describeUnconfirmed = (read) =>
  read.read
    ? `Estigia could not confirm this run's claim (${read.said?.standing ?? "no standing"})`
    : `the answer could not be read (${read.why})`;

const retryOrStop = (pi, ctx, session, read) => {
  const goal = loops.get(session);
  if (!running(goal)) return;
  const attempts = (typeof goal.retries === "number" ? goal.retries : 0) + 1;
  const why = describeUnconfirmed(read);
  if (attempts >= UNCONFIRMED_ATTEMPTS) {
    stop(ctx, session, `${why} after ${attempts - 1} retries`, "stopped-unconfirmed");
    return;
  }
  waited(pi, ctx, session, `${why}; retrying`, attempts);
};

const enqueue = (pi, goal, dischargeable, issue = null) =>
  pi.sendMessage(
    {
      customType: MESSAGE_TYPE,
      content:
        issue === null
          ? `${preamble(goal.run_id, dischargeable, typeof goal.max === "number")}\n\n${goal.prompt}`
          : held(issue, goal.run_id),
      display: true,
    },
    { triggerTurn: true, deliverAs: "followUp" },
  );

const consider = async (pi, ctx, session) => {
  const goal = loops.get(session);
  if (!running(goal)) return;
  const cwd = goal.directory;
  const capped = typeof goal.max === "number";
  const read = await ask(session, cwd, capped);
  if (!same(ctx, session) || !running(loops.get(session))) return;
  if (!read.read && read.silent) {
    stop(ctx, session, `Estigia gave no answer (${read.why})`, "stopped-unanswered");
    return;
  }
  if (unconfirmed(read)) {
    retryOrStop(pi, ctx, session, read);
    return;
  }
  if (capped) {
    const completed = read.said?.completed;
    if (typeof completed !== "number") {
      stop(ctx, session, "Estigia did not report how many issues this run has completed");
      return;
    }
    if (completed >= goal.max) {
      stop(ctx, session, `the completion cap (${goal.max}) was reached`);
      return;
    }
  }
  let dischargeable = 0;
  let issue = null;
  if (read.said?.live === true) {
    const named = read.said?.issue;
    if (named === null || named === undefined || named === "") {
      stop(ctx, session, "Estigia did not report which issue this run holds");
      return;
    }
    issue = String(named);
  } else {
    if (typeof read.said?.stop_after === "string" && read.said.stop_after !== "") {
      stop(
        ctx,
        session,
        `the agent asked to stop after the current task (${read.said.stop_after})`,
        "stopped-on-request",
      );
      await clearStopAfter(session, cwd);
      return;
    }
    const standing = read.said?.standing;
    if (standing !== "stopped" && standing !== "nothing-sworn") {
      stop(ctx, session, `Estigia could not confirm this run's claim (${standing ?? "no standing"})`);
      return;
    }
    const claimable = read.said?.claimable;
    const discharged = read.said?.dischargeable;
    if (typeof claimable !== "number" || typeof discharged !== "number") {
      stop(ctx, session, "Estigia did not report how much work this run's queues hold");
      return;
    }
    if (claimable + discharged <= 0) {
      const { held, pending, unconditioned, unadmitted } = read.said;
      const opens =
        (typeof held === "number" ? held : 0) +
        (typeof pending === "number" ? pending : 0) +
        (typeof unadmitted === "number" ? unadmitted : 0);
      if (opens > 0) {
        waited(pi, ctx, session, describe(held, pending, unadmitted));
        return;
      }
      const unread = held === null || pending === null;
      stop(
        ctx,
        session,
        "Estigia reports no live claim and no claimable or dischargeable work" +
          (unread ? " (what is held or pending could not be read)" : "") +
          (typeof unconditioned === "number" && unconditioned > 0
            ? `; ${unconditioned} blocked with no recorded condition, or one that is not well formed, are left for a person`
            : ""),
      );
      return;
    }
    dischargeable = discharged;
  }
  if (!running(loops.get(session)) || stopRequested.has(session)) return;
  disarm(session);
  enqueue(pi, goal, dischargeable, issue);
  reacted(goal, "enqueued");
};

const decide = async (pi, ctx, session) => {
  if (deciding.has(session)) return;
  deciding.add(session);
  try {
    try {
      await consider(pi, ctx, session);
    } finally {
      const why = stopRequested.get(session);
      if (why !== undefined) {
        stopRequested.delete(session);
        stop(ctx, session, why);
      }
    }
  } finally {
    deciding.delete(session);
  }
};

const guarded = async (ctx, session, work) => {
  try {
    await work();
  } catch (error) {
    const why = errorText(error);
    console.error(`estigia: the loop could not continue (${why})`);
    stop(ctx, session, `the loop failed (${why})`, `failed: ${why}`, true);
  }
};

const tick = (pi, ctx, session) =>
  guarded(ctx, session, async () => {
    if (!same(ctx, session) || !idle(ctx)) return;
    await decide(pi, ctx, session);
  });

export default function (pi) {
  pi.on("agent_settled", (event, ctx) => {
    const session = sessionOf(ctx);
    if (session === undefined || !running(loops.get(session))) return undefined;
    return guarded(ctx, session, async () => {
      if (event?.aborted === true) {
        stop(ctx, session, "the turn was aborted");
        return;
      }
      if (!idle(ctx)) return;
      await decide(pi, ctx, session);
    });
  });

  pi.on("session_shutdown", () => {
    for (const session of [...timers.keys()]) disarm(session);
    loops.clear();
    stopRequested.clear();
    deciding.clear();
  });

  const start = async (text, ctx) => {
    const session = sessionOf(ctx);
    if (session === undefined) {
      say(ctx, "Estigia loop did not start: this session has no id.", "warning");
      return;
    }
    if (isChild(ctx)) {
      say(ctx, "Estigia loop did not start: a child session does not run the loop.", "warning");
      return;
    }
    if (text.trim() === "stop") {
      if (!running(loops.get(session))) {
        say(ctx, "Estigia loop: no running loop is stored for this session.");
        return;
      }
      if (deciding.has(session)) {
        stopRequested.set(session, "the operator stopped the loop");
        say(ctx, "Estigia loop: stopping as soon as the decision in flight finishes.");
        return;
      }
      stop(ctx, session, "the operator stopped the loop");
      return;
    }
    if (running(loops.get(session))) {
      say(
        ctx,
        "Estigia loop: this session's loop is already running, so nothing was sent. Use `/flow-loop stop` to end it.",
      );
      return;
    }
    const cwd = ctx.cwd ?? process.cwd();
    const read = await ask(session, cwd);
    if (sessionOf(ctx) !== session) return;
    if (!read.read) {
      say(ctx, `Estigia loop did not start: the goal could not be read (${read.why}).`, "warning");
      return;
    }
    const run_id = read.said?.run_id;
    if (typeof run_id !== "string" || run_id === "") {
      say(ctx, "Estigia loop did not start: Estigia gave no run id.", "warning");
      return;
    }
    const { prompt: given, max, wait, refused } = parse(text);
    if (refused) {
      say(ctx, `Estigia loop did not start: ${refused}.`, "warning");
      return;
    }
    if (typeof read.said?.stop_after === "string" && read.said.stop_after !== "") {
      await clearStopAfter(session, cwd);
      if (sessionOf(ctx) !== session) return;
    }
    const goal = { prompt: objective(given), max, wait, run_id, directory: cwd };
    loops.set(session, goal);
    enqueue(pi, goal, 0);
  };

  const report = async (text, ctx) => {
    const session = sessionOf(ctx);
    if (session === undefined) return;
    if (text.trim() !== "") {
      say(ctx, "/flow-status takes no argument. Use /flow-loop to run the Estigia loop.");
      return;
    }
    const goal = loops.get(session);
    if (!goal) {
      say(ctx, "Estigia goal: no loop goal is stored for this session — the loop never started here, or Pi was restarted since.");
      return;
    }
    const lines = [`Estigia goal: ${goal.prompt}`, `Loop run id: ${goal.run_id}`];
    lines.push(`Asked in: ${goal.directory}`);
    if (typeof goal.max === "number") lines.push(`Completion cap: ${goal.max}`);
    lines.push(`Re-ask interval while waiting: ${goal.wait} minutes`);
    lines.push(
      goal.stopped
        ? `The loop stopped at ${goal.stopped.at} — ${goal.stopped.reason}`
        : "No stop is recorded for this loop.",
    );
    if (goal.waiting && running(goal)) {
      lines.push(`Waiting since ${goal.waiting.since} on ${goal.waiting.on}; the next re-ask is at ${goal.waiting.next}.`);
    }
    lines.push(
      goal.last
        ? `Last turn end observed at ${goal.last.at}: ${goal.last.outcome}`
        : "No turn end has been observed for this loop yet.",
    );
    const read = await ask(session, goal.directory);
    if (sessionOf(ctx) !== session) return;
    lines.push(
      read.read
        ? `Tracker answer: ${JSON.stringify(read.said)}`
        : `Tracker answer could not be read (${read.why}).`,
    );
    say(ctx, lines.join("\n"));
  };

  const command = (name, description, work) =>
    pi.registerCommand(name, {
      description,
      handler: async (args, ctx) => {
        try {
          await work(String(args ?? ""), ctx);
        } catch (error) {
          console.error(`estigia: /${name} failed (${errorText(error)})`);
        }
      },
    });

  command(
    "flow-loop",
    "Keep working this session's Estigia run until the tracker says it is done",
    start,
  );
  command("flow-status", "Report this session's Estigia goal, and why its loop stopped", report);
}
