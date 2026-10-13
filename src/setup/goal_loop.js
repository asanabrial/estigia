// estigia:goal-loop — managed by the `Goal loop plugin` setting in OpenCode's table
//
// `/goal <condition>` keeps a session working until a condition holds, and
// `/loop [interval] <prompt>` repeats a prompt on a timer or after every turn.
// Nothing here consults a tracker, runs a program or reads a file: the only
// judge of a goal is the model, asked through `ctx.session.generate` — text
// generated from the session's own context without mutating its history — and
// what it answers is a judgement of the transcript, not evidence of anything.
//
// An evaluator answer that cannot be read stops the goal; it never continues
// it. Nothing throws out of a hook. Every instance of this plugin sees every
// event on the server, so one acts only on a session whose goal or loop was
// asked in its own location, and an event's own location is never consulted.
// Every stop and every report is admitted with `resume: false`: visible, and no
// model turn that could re-arm anything.
const GOAL = "goal-loop:goal:";
const LOOP = "goal-loop:loop:";
const DEFAULT_MAX = 25;
const JUDGE_EVERY = 3;
const MIN_INTERVAL_MS = 10000;
const MAX_INTERVAL_MS = 2147483647;
const UNITS = { s: 1000, m: 60000, h: 3600000, d: 86400000 };
const RESUBSCRIBE_MS = 1000;

const timers = new Map();
const disposed = new WeakSet();
const deciding = new Set();

const errorText = (error) => String(error?.message ?? error);

const locationOf = (ctx) => ctx?.location?.directory ?? process.cwd();

const running = (record) =>
  record && typeof record === "object" && !(record.stopped && typeof record.stopped === "object");

const say = (ctx, sessionID, text) =>
  ctx.session.synthetic({
    sessionID,
    text,
    description: text.startsWith("Goal") ? text : `Goal loop: ${text}`,
    resume: false,
  });

const enqueue = (ctx, sessionID, text) =>
  ctx.session.prompt({ sessionID, text, delivery: "queue" });

const takeMax = (text) => {
  const found = /(?:^|\s)--max(?:=|\s+)(\S+)/.exec(text);
  const rest = text.replace(/(?:^|\s)--max(?:=|\s+)\S+/, " ").trim();
  if (!found) return { rest, max: null, refused: null };
  const max = Number(found[1]);
  const refused =
    Number.isInteger(max) && max >= 1 ? null : `--max takes a whole number of turns of at least 1, and was given ${found[1]}`;
  return { rest, max, refused };
};

const takeInterval = (text) => {
  const found = /^(\d+)([smhd])(?:\s+|$)/.exec(text);
  if (!found) return { rest: text, interval: null, refused: null };
  const interval = Number(found[1]) * UNITS[found[2]];
  const refused =
    interval < MIN_INTERVAL_MS
      ? `an interval must be at least ${MIN_INTERVAL_MS / 1000} seconds, and ${found[1]}${found[2]} is less`
      : interval > MAX_INTERVAL_MS
        ? `an interval must be at most ${MAX_INTERVAL_MS} milliseconds (about 24 days), and ${found[1]}${found[2]} is more`
        : null;
  return { rest: text.slice(found[0].length).trim(), interval, refused };
};

const judgement = (condition) =>
  "Judge, from this conversation alone, whether the following condition now holds. " +
  "Answer with the single word MET or the two words NOT MET on the first line, " +
  "then one short line giving the reason. The first line is read case-insensitively, ignoring trailing punctuation, and anything else counts as no answer.\n\n" +
  `Condition: ${condition}`;

const readVerdict = (answer) => {
  const text = typeof answer === "string" ? answer : (answer?.text ?? answer?.data?.text);
  if (typeof text !== "string") return { read: false, why: "the evaluator returned no text" };
  const lines = text.trim().split("\n");
  const first = lines[0].trim().replace(/[.!*`]+$/, "").toUpperCase();
  const reason = lines.slice(1).join(" ").trim();
  if (first === "MET") return { read: true, met: true, reason };
  if (first === "NOT MET") return { read: true, met: false, reason };
  return { read: false, why: `the evaluator's first line was neither MET nor NOT MET (${lines[0].slice(0, 60)})` };
};

const disarm = (sessionID) => {
  const held = timers.get(sessionID);
  if (held === undefined) return;
  clearTimeout(held.timer);
  timers.delete(sessionID);
};

const disarmAll = (ctx) => {
  for (const [sessionID, held] of [...timers]) if (held.ctx === ctx) disarm(sessionID);
};

const closeGoal = async (ctx, sessionID, reason, outcome) => {
  const goal = await ctx.storage.get(GOAL + sessionID);
  if (running(goal)) {
    const at = new Date().toISOString();
    await ctx.storage.set(GOAL + sessionID, { ...goal, stopped: { reason, at }, last: { at, outcome } });
  }
};

const closeLoop = async (ctx, sessionID, reason) => {
  disarm(sessionID);
  const loop = await ctx.storage.get(LOOP + sessionID);
  if (running(loop)) {
    await ctx.storage.set(LOOP + sessionID, { ...loop, stopped: { reason, at: new Date().toISOString() } });
  }
};

const stopGoal = async (ctx, sessionID, reason, outcome = `stopped: ${reason}`) => {
  await closeGoal(ctx, sessionID, reason, outcome);
  console.error(`goal-loop: the goal for this session stopped — ${reason}`);
  await say(ctx, sessionID, `Goal stopped: ${reason}`);
};

const judge = async (ctx, sessionID, goal) => {
  let answer;
  try {
    answer = await ctx.session.generate({ sessionID, prompt: judgement(goal.condition) });
  } catch (error) {
    return { read: false, why: `the evaluator failed (${errorText(error)})` };
  }
  return readVerdict(answer);
};

const consider = async (ctx, sessionID) => {
  const goal = await ctx.storage.get(GOAL + sessionID);
  if (!running(goal) || goal.directory !== locationOf(ctx)) return;
  const counted = goal.turns + 1;
  if (counted % JUDGE_EVERY !== 0 && counted < goal.max) {
    const current = await ctx.storage.get(GOAL + sessionID);
    if (!running(current) || current.id !== goal.id) return;
    await ctx.storage.set(GOAL + sessionID, { ...current, turns: counted });
    await enqueue(ctx, sessionID, `Goal: ${goal.condition}\nKeep working toward it.`);
    return;
  }
  const verdict = await judge(ctx, sessionID, goal);
  const held = await ctx.storage.get(GOAL + sessionID);
  if (!running(held) || held.id !== goal.id) return;
  if (!verdict.read) {
    await stopGoal(ctx, sessionID, `the evaluator's answer could not be read (${verdict.why})`);
    return;
  }
  const turns = held.turns + 1;
  const at = new Date().toISOString();
  const last = { at, met: verdict.met, reason: verdict.reason, turns };
  if (verdict.met) {
    await ctx.storage.set(GOAL + sessionID, { ...held, turns, judged: last });
    await closeGoal(ctx, sessionID, "the condition was judged met", "met");
    await say(ctx, sessionID, `Goal met after ${turns} turn${turns === 1 ? "" : "s"}: ${verdict.reason || goal.condition}`);
    return;
  }
  await ctx.storage.set(GOAL + sessionID, { ...held, turns, judged: last });
  if (turns >= held.max) {
    await stopGoal(ctx, sessionID, `the turn cap (${held.max}) was reached without the condition being judged met`);
    return;
  }
  const current = await ctx.storage.get(GOAL + sessionID);
  if (!running(current) || current.id !== held.id) return;
  await enqueue(
    ctx,
    sessionID,
    `Goal: ${held.condition}\nIt was judged not yet met${verdict.reason ? `: ${verdict.reason}` : "."} Keep working toward it.`,
  );
};

const arm = (ctx, sessionID, delay) => {
  if (disposed.has(ctx)) return;
  disarm(sessionID);
  const timer = setTimeout(() => {
    timers.delete(sessionID);
    void guarded(ctx, sessionID, () => tick(ctx, sessionID));
  }, delay);
  timers.set(sessionID, { ctx, timer });
};

const tick = async (ctx, sessionID) => {
  if (disposed.has(ctx)) return;
  const loop = await ctx.storage.get(LOOP + sessionID);
  if (!running(loop) || loop.interval === null || loop.directory !== locationOf(ctx)) return;
  await repeat(ctx, sessionID, loop);
};

const repeat = async (ctx, sessionID, seen) => {
  const loop = await ctx.storage.get(LOOP + sessionID);
  if (!running(loop) || loop.id !== seen.id) return;
  const runs = loop.runs + 1;
  if (typeof loop.max === "number" && runs > loop.max) {
    await closeLoop(ctx, sessionID, `the run cap (${loop.max}) was reached`);
    await say(ctx, sessionID, `Loop stopped: the run cap (${loop.max}) was reached.`);
    return;
  }
  const next = loop.interval === null ? null : new Date(Date.now() + loop.interval).toISOString();
  await ctx.storage.set(LOOP + sessionID, { ...loop, runs, next, last: new Date().toISOString() });
  await enqueue(ctx, sessionID, loop.prompt);
  if (loop.interval !== null) arm(ctx, sessionID, loop.interval);
};

const guarded = async (ctx, sessionID, work) => {
  try {
    await work();
  } catch (error) {
    const why = errorText(error);
    console.error(`goal-loop: could not continue (${why})`);
    if (typeof sessionID !== "string" || sessionID === "") return;
    try {
      const own = (record) => running(record) && record.directory === locationOf(ctx);
      const goal = own(await ctx.storage.get(GOAL + sessionID));
      const loop = own(await ctx.storage.get(LOOP + sessionID));
      if (goal) await closeGoal(ctx, sessionID, `the goal failed (${why})`, `failed: ${why}`);
      if (loop) await closeLoop(ctx, sessionID, `the loop failed (${why})`);
      if (goal || loop) await say(ctx, sessionID, `Goal and loop stopped: ${why}`);
    } catch (recording) {
      console.error(`goal-loop: could not record the failure (${errorText(recording)})`);
    }
  }
};

const advance = async (ctx, event) => {
  const sessionID = event?.data?.sessionID;
  if (typeof sessionID !== "string" || sessionID === "") return;
  const goal = await ctx.storage.get(GOAL + sessionID);
  if (running(goal) && goal.directory === locationOf(ctx) && !deciding.has(sessionID)) {
    deciding.add(sessionID);
    try {
      await consider(ctx, sessionID);
    } finally {
      deciding.delete(sessionID);
    }
  }
  const loop = await ctx.storage.get(LOOP + sessionID);
  if (running(loop) && loop.interval === null && loop.directory === locationOf(ctx)) {
    await repeat(ctx, sessionID, loop);
  }
};

const react = (ctx, event) => guarded(ctx, event?.data?.sessionID, () => advance(ctx, event));

const startGoal = async (ctx, sessionID, text) => {
  const { rest, max, refused } = takeMax(text);
  if (refused) {
    await say(ctx, sessionID, `Goal not set: ${refused}.`);
    return;
  }
  if (rest === "") {
    await say(ctx, sessionID, "Goal not set: a condition is needed, as in `/goal all tests pass`.");
    return;
  }
  const previous = await ctx.storage.get(GOAL + sessionID);
  await ctx.storage.set(GOAL + sessionID, {
    id: crypto.randomUUID(),
    condition: rest,
    max: max ?? DEFAULT_MAX,
    turns: 0,
    directory: locationOf(ctx),
    started: new Date().toISOString(),
  });
  if (running(previous)) await say(ctx, sessionID, `Goal replaced: it was "${previous.condition}".`);
  await enqueue(
    ctx,
    sessionID,
    `Goal: ${rest}\nWork toward it. Every third turn, and on the last turn the cap allows, the condition is judged from this conversation, ` +
      "and you are prompted again until it holds.",
  );
};

const reportGoal = async (ctx, sessionID) => {
  const goal = await ctx.storage.get(GOAL + sessionID);
  if (!goal || typeof goal !== "object") {
    await say(ctx, sessionID, "No goal is set for this session. Set one with `/goal <condition>`.");
    return;
  }
  const lines = [`Goal: ${goal.condition}`, `Turns: ${goal.turns} of at most ${goal.max}`];
  if (goal.stopped && typeof goal.stopped === "object") {
    lines.push(`Stopped at ${goal.stopped.at}: ${goal.stopped.reason}`);
  } else {
    lines.push("Still running.");
  }
  if (goal.judged && typeof goal.judged === "object") {
    lines.push(
      `Last judgement at ${goal.judged.at}: ${goal.judged.met ? "met" : "not met"}` +
        `${goal.judged.reason ? ` — ${goal.judged.reason}` : ""}`,
    );
  } else {
    lines.push("No judgement has been made yet.");
  }
  lines.push("This is a model's judgement of the conversation, not a check that the condition holds.");
  await say(ctx, sessionID, lines.join("\n"));
};

const goalCommand = async (ctx, invocation) => {
  const sessionID = invocation?.sessionID;
  if (!sessionID) return;
  const text = String(invocation?.prompt?.text ?? "").trim();
  if (text === "") {
    await reportGoal(ctx, sessionID);
    return;
  }
  if (text === "clear") {
    const goal = await ctx.storage.get(GOAL + sessionID);
    if (!running(goal)) {
      await say(ctx, sessionID, "No running goal is set for this session.");
      return;
    }
    await closeGoal(ctx, sessionID, "the operator cleared the goal", "cleared");
    await say(ctx, sessionID, "Goal cleared.");
    return;
  }
  await startGoal(ctx, sessionID, text);
};

const loopCommand = async (ctx, invocation) => {
  const sessionID = invocation?.sessionID;
  const text = String(invocation?.prompt?.text ?? "").trim();
  if (!sessionID) return;
  if (text === "stop") {
    const held = await ctx.storage.get(LOOP + sessionID);
    if (!running(held)) {
      await say(ctx, sessionID, "No running loop is set for this session.");
      return;
    }
    await closeLoop(ctx, sessionID, "the operator stopped the loop");
    await say(ctx, sessionID, "Loop stopped.");
    return;
  }
  const { rest: withoutMax, max, refused: maxRefused } = takeMax(text);
  const { rest, interval, refused } = takeInterval(withoutMax);
  const why = maxRefused ?? refused;
  if (why) {
    await say(ctx, sessionID, `Loop not started: ${why}.`);
    return;
  }
  if (rest === "") {
    await say(ctx, sessionID, "Loop not started: a prompt is needed, as in `/loop 5m check the build`, or `/loop stop`.");
    return;
  }
  const existing = await ctx.storage.get(LOOP + sessionID);
  if (running(existing)) {
    await say(ctx, sessionID, "This session's loop is already running, so nothing was enqueued. Use `/loop stop` to end it.");
    return;
  }
  const loop = { id: crypto.randomUUID(), prompt: rest, interval, max, runs: 0, directory: locationOf(ctx), started: new Date().toISOString() };
  await ctx.storage.set(LOOP + sessionID, loop);
  await repeat(ctx, sessionID, loop);
};

const rearm = async (ctx) => {
  try {
    let after;
    do {
      const page = await ctx.storage.scan({ prefix: LOOP, after });
      for (const entry of page?.entries ?? []) {
        const loop = entry?.value;
        if (!running(loop) || loop.interval === null || loop.directory !== locationOf(ctx)) continue;
        const due = Date.parse(loop.next);
        arm(ctx, String(entry.key).slice(LOOP.length), Number.isFinite(due) ? Math.max(0, due - Date.now()) : 0);
      }
      after = page?.next;
    } while (after);
  } catch (error) {
    console.error(`goal-loop: could not re-arm interval loops (${errorText(error)})`);
  }
};

const pause = (signal) =>
  new Promise((resolve) => {
    const timer = setTimeout(resolve, RESUBSCRIBE_MS);
    signal.addEventListener("abort", () => {
      clearTimeout(timer);
      resolve();
    }, { once: true });
  });

const register = (ctx, name, description, run) =>
  ctx.command.transform((editor) =>
    editor.add({
      name,
      description,
      execute: async (invocation) => {
        try {
          await run(ctx, invocation);
        } catch (error) {
          console.error(`goal-loop: /${name} failed (${errorText(error)})`);
        }
      },
    }),
  );

export default {
  id: "goal-loop",
  async setup(ctx) {
    const controller = new AbortController();
    await register(ctx, "goal", "Keep working until a condition is judged met, or report the current goal", goalCommand);
    await register(ctx, "loop", "Repeat a prompt every interval, or after each turn when none is given", loopCommand);
    const watch = async () => {
      while (!controller.signal.aborted) {
        let why = "the event stream ended";
        try {
          for await (const event of ctx.event.subscribe({ signal: controller.signal })) {
            if (event?.type !== "session.execution.succeeded") continue;
            await react(ctx, event);
          }
        } catch (error) {
          why = `the event stream failed (${errorText(error)})`;
        }
        if (controller.signal.aborted) return;
        console.error(`goal-loop: ${why}; subscribing again`);
        await pause(controller.signal);
      }
    };
    watch();
    await rearm(ctx);
    return () => {
      controller.abort();
      disposed.add(ctx);
      disarmAll(ctx);
    };
  },
};
