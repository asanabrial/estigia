// estigia:goal-loop — managed by the `Goal loop plugin` setting in Pi's table
//
// `/goal <condition>` keeps a session working until a condition holds, and
// `/loop [interval] <prompt>` repeats a prompt on a timer or after every turn.
// Nothing here consults a tracker, runs a program or reads a file: the only
// judge of a goal is the model, asked through `ctx.modelRegistry.streamSimple`
// with the session's own transcript, and what it answers is a judgement of the
// transcript, not evidence of anything.
//
// An evaluator answer that cannot be read, or that does not come within the
// timeout, stops the goal; it never continues it. Nothing throws out of a hook.
// State lives in this process only: a goal or loop ends when Pi shuts the
// session down, and is not replayed from the session file. Every stop and every
// report is a notification: visible, and no model turn that could re-arm anything.
const MESSAGE_TYPE = "goal-loop";
const DEFAULT_MAX = 25;
const MIN_INTERVAL_MS = 10000;
const MAX_INTERVAL_MS = 2147483647;
const UNITS = { s: 1000, m: 60000, h: 3600000, d: 86400000 };
const JUDGE_TIMEOUT_MS = 120000;
const JUDGE_MAX_TOKENS = 200;
const TRANSCRIPT_LIMIT = 120000;
const RESULT_LIMIT = 2000;
const DELIVERY = { triggerTurn: true, deliverAs: "followUp" };

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
    console.error(`goal-loop: ${text}`);
  }
};

const running = (record) => Boolean(record) && !record.stopped;

const takeMax = (text) => {
  const found = /(?:^|\s)--max(?:=|\s+|$)(\S*)/.exec(text);
  const rest = text.replace(/(?:^|\s)--max(?:=|\s+|$)\S*/, " ").trim();
  if (!found) return { rest, max: null, refused: null };
  const max = found[1] === "" ? NaN : Number(found[1]);
  const refused =
    Number.isInteger(max) && max >= 1
      ? null
      : `--max takes a whole number of turns of at least 1, and was given ${found[1] === "" ? "nothing" : found[1]}`;
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
  "Judge, from the conversation above alone, whether the following condition now holds. " +
  "Answer with the single word MET or the two words NOT MET on the first line, " +
  "then one short line giving the reason. The first line is read case-insensitively, ignoring trailing punctuation, and anything else counts as no answer.\n\n" +
  `Condition: ${condition}`;

const textOf = (content, limit) => {
  const parts = typeof content === "string" ? [content] : Array.isArray(content) ? content : [];
  const text = parts
    .map((part) => (typeof part === "string" ? part : part?.type === "text" ? part.text : ""))
    .filter((part) => typeof part === "string" && part !== "")
    .join("\n");
  return text.length > limit ? `${text.slice(0, limit)} [truncated]` : text;
};

const transcriptOf = (ctx) => {
  const messages = ctx.sessionManager.buildSessionProjection().messages;
  const lines = [];
  for (const message of Array.isArray(messages) ? messages : []) {
    if (message?.role === "user" || message?.role === "custom") {
      lines.push(`[user]\n${textOf(message.content, TRANSCRIPT_LIMIT)}`);
    } else if (message?.role === "assistant") {
      lines.push(`[assistant]\n${textOf(message.content, TRANSCRIPT_LIMIT)}`);
    } else if (message?.role === "toolResult") {
      lines.push(`[tool result: ${message.toolName ?? "tool"}]\n${textOf(message.content, RESULT_LIMIT)}`);
    }
  }
  const whole = lines.join("\n\n");
  return whole.length > TRANSCRIPT_LIMIT ? `[earlier conversation omitted]\n${whole.slice(-TRANSCRIPT_LIMIT)}` : whole;
};

const readVerdict = (answer) => {
  if (answer?.stopReason === "error" || answer?.stopReason === "aborted") {
    return { read: false, why: `the evaluator failed (${answer.errorMessage ?? answer.stopReason})` };
  }
  const blocks = Array.isArray(answer?.content) ? answer.content : null;
  if (blocks === null) return { read: false, why: "the evaluator returned no text" };
  const text = blocks
    .filter((block) => block?.type === "text" && typeof block.text === "string")
    .map((block) => block.text)
    .join("\n");
  if (text.trim() === "") return { read: false, why: "the evaluator returned no text" };
  const lines = text.trim().split("\n");
  const first = lines[0].trim().replace(/[.!*`]+$/, "").toUpperCase();
  const reason = lines.slice(1).join(" ").trim();
  if (first === "MET") return { read: true, met: true, reason };
  if (first === "NOT MET") return { read: true, met: false, reason };
  return { read: false, why: `the evaluator's first line was neither MET nor NOT MET (${lines[0].slice(0, 60)})` };
};

export default function (pi) {
  const goals = new Map();
  const loops = new Map();
  const timers = new Map();
  const deciding = new Set();
  let counter = 0;

  const disarm = (session) => {
    const held = timers.get(session);
    if (held === undefined) return;
    clearTimeout(held);
    timers.delete(session);
  };

  const closeGoal = (goal, reason) => {
    if (running(goal)) goal.stopped = { reason, at: new Date().toISOString() };
  };

  const closeLoop = (session, loop, reason) => {
    disarm(session);
    if (running(loop)) loop.stopped = { reason, at: new Date().toISOString() };
  };

  const stopGoal = (ctx, goal, reason) => {
    closeGoal(goal, reason);
    console.error(`goal-loop: the goal for this session stopped — ${reason}`);
    say(ctx, `Goal stopped: ${reason}`, "warning");
  };

  const enqueue = (text) =>
    pi.sendMessage({ customType: MESSAGE_TYPE, content: text, display: true }, DELIVERY);

  const ownGoal = (ctx, session, goal) =>
    sessionOf(ctx) === session && goals.get(session) === goal && running(goal);

  const ownLoop = (ctx, session, loop) =>
    sessionOf(ctx) === session && loops.get(session) === loop && running(loop);

  const judge = async (ctx, goal) => {
    const model = ctx.model;
    if (!model) return { read: false, why: "the session has no model to ask" };
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), JUDGE_TIMEOUT_MS);
    try {
      const context = {
        systemPrompt: "You judge, from a conversation transcript, whether a stated condition now holds.",
        messages: [
          {
            role: "user",
            content: `${transcriptOf(ctx)}\n\n${judgement(goal.condition)}`,
            timestamp: Date.now(),
          },
        ],
      };
      const stream = ctx.modelRegistry.streamSimple(model, context, {
        signal: controller.signal,
        maxTokens: JUDGE_MAX_TOKENS,
      });
      return readVerdict(await stream.result());
    } catch (error) {
      return { read: false, why: `the evaluator failed (${controller.signal.aborted ? `no answer within ${JUDGE_TIMEOUT_MS / 1000} seconds` : errorText(error)})` };
    } finally {
      clearTimeout(timer);
    }
  };

  const consider = async (ctx, session, goal) => {
    const verdict = await judge(ctx, goal);
    if (!ownGoal(ctx, session, goal)) return;
    if (!verdict.read) {
      stopGoal(ctx, goal, `the evaluator's answer could not be read (${verdict.why})`);
      return;
    }
    goal.turns += 1;
    const turns = goal.turns;
    goal.judged = { at: new Date().toISOString(), met: verdict.met, reason: verdict.reason, turns };
    if (verdict.met) {
      closeGoal(goal, "the condition was judged met");
      say(ctx, `Goal met after ${turns} turn${turns === 1 ? "" : "s"}: ${verdict.reason || goal.condition}`);
      return;
    }
    if (turns >= goal.max) {
      stopGoal(ctx, goal, `the turn cap (${goal.max}) was reached without the condition being judged met`);
      return;
    }
    enqueue(
      `Goal: ${goal.condition}\nIt was judged not yet met${verdict.reason ? `: ${verdict.reason}` : "."} Keep working toward it.`,
    );
  };

  const repeat = (ctx, session, loop) => {
    if (!ownLoop(ctx, session, loop)) return;
    const runs = loop.runs + 1;
    if (typeof loop.max === "number" && runs > loop.max) {
      closeLoop(session, loop, `the run cap (${loop.max}) was reached`);
      say(ctx, `Loop stopped: the run cap (${loop.max}) was reached.`);
      return;
    }
    loop.runs = runs;
    loop.last = new Date().toISOString();
    loop.next = loop.interval === null ? null : new Date(Date.now() + loop.interval).toISOString();
    enqueue(loop.prompt);
    if (loop.interval !== null) arm(ctx, session, loop);
  };

  const arm = (ctx, session, loop) => {
    disarm(session);
    timers.set(
      session,
      setTimeout(() => {
        timers.delete(session);
        guarded(ctx, session, () => repeat(ctx, session, loop));
      }, loop.interval),
    );
  };

  const guarded = (ctx, session, work) => {
    try {
      const done = work();
      if (done && typeof done.catch === "function") return done.catch((error) => failed(ctx, session, error));
    } catch (error) {
      failed(ctx, session, error);
    }
    return undefined;
  };

  const failed = (ctx, session, error) => {
    const why = errorText(error);
    console.error(`goal-loop: could not continue (${why})`);
    if (typeof session !== "string" || session === "") return;
    const goal = goals.get(session);
    const loop = loops.get(session);
    const own = running(goal) || running(loop);
    if (running(goal)) closeGoal(goal, `the goal failed (${why})`);
    if (running(loop)) closeLoop(session, loop, `the loop failed (${why})`);
    if (own) say(ctx, `Goal and loop stopped: ${why}`, "warning");
  };

  const advance = async (ctx, session, aborted) => {
    const goal = goals.get(session);
    if (running(goal)) {
      if (aborted) {
        stopGoal(ctx, goal, "the turn was aborted");
      } else if (idle(ctx) && !deciding.has(session)) {
        deciding.add(session);
        try {
          await consider(ctx, session, goal);
        } finally {
          deciding.delete(session);
        }
      }
    }
    const loop = loops.get(session);
    if (ownLoop(ctx, session, loop) && loop.interval === null) {
      if (aborted) {
        closeLoop(session, loop, "the turn was aborted");
        say(ctx, "Loop stopped: the turn was aborted.");
      } else if (idle(ctx)) {
        repeat(ctx, session, loop);
      }
    }
  };

  pi.on("agent_settled", (event, ctx) => {
    const session = sessionOf(ctx);
    if (session === undefined || (!running(goals.get(session)) && !running(loops.get(session)))) return undefined;
    return guarded(ctx, session, () => advance(ctx, session, event?.aborted === true));
  });

  pi.on("session_shutdown", () => {
    for (const session of [...timers.keys()]) disarm(session);
    goals.clear();
    loops.clear();
    deciding.clear();
  });

  const reportGoal = (ctx, session) => {
    const goal = goals.get(session);
    if (!goal) {
      say(ctx, "No goal is set for this session. Set one with `/goal <condition>`.");
      return;
    }
    const lines = [`Goal: ${goal.condition}`, `Turns: ${goal.turns} of at most ${goal.max}`];
    lines.push(goal.stopped ? `Stopped at ${goal.stopped.at}: ${goal.stopped.reason}` : "Still running.");
    lines.push(
      goal.judged
        ? `Last judgement at ${goal.judged.at}: ${goal.judged.met ? "met" : "not met"}${goal.judged.reason ? ` — ${goal.judged.reason}` : ""}`
        : "No judgement has been made yet.",
    );
    lines.push("This is a model's judgement of the conversation, not a check that the condition holds.");
    say(ctx, lines.join("\n"));
  };

  const reportLoop = (ctx, session) => {
    const loop = loops.get(session);
    if (!loop) {
      say(ctx, "No loop is set for this session. Start one with `/loop [interval] <prompt>`.");
      return;
    }
    const lines = [`Loop: ${loop.prompt}`];
    lines.push(loop.interval === null ? "Repeats after each turn." : `Repeats every ${loop.interval / 1000} seconds.`);
    lines.push(`Runs: ${loop.runs}${typeof loop.max === "number" ? ` of at most ${loop.max}` : ""}`);
    if (loop.next && running(loop)) lines.push(`Next run at ${loop.next}.`);
    lines.push(loop.stopped ? `Stopped at ${loop.stopped.at}: ${loop.stopped.reason}` : "Still running.");
    say(ctx, lines.join("\n"));
  };

  const refuseChild = (ctx, what) => {
    say(ctx, `${what} did not start: a child session does not run goals or loops.`, "warning");
  };

  const goalCommand = async (text, ctx) => {
    const session = sessionOf(ctx);
    if (session === undefined) return;
    const given = text.trim();
    if (given === "") {
      reportGoal(ctx, session);
      return;
    }
    if (given === "clear") {
      const goal = goals.get(session);
      if (!running(goal)) {
        say(ctx, "No running goal is set for this session.");
        return;
      }
      closeGoal(goal, "the operator cleared the goal");
      say(ctx, "Goal cleared.");
      return;
    }
    if (isChild(ctx)) {
      refuseChild(ctx, "Goal");
      return;
    }
    const { rest, max, refused } = takeMax(given);
    if (refused) {
      say(ctx, `Goal not set: ${refused}.`, "warning");
      return;
    }
    if (rest === "") {
      say(ctx, "Goal not set: a condition is needed, as in `/goal all tests pass`.", "warning");
      return;
    }
    const previous = goals.get(session);
    goals.set(session, {
      id: ++counter,
      condition: rest,
      max: max ?? DEFAULT_MAX,
      turns: 0,
      started: new Date().toISOString(),
    });
    if (running(previous)) say(ctx, `Goal replaced: it was "${previous.condition}".`);
    enqueue(
      `Goal: ${rest}\nWork toward it. After each of your turns the condition is judged from this conversation, ` +
        "and you are prompted again until it holds.",
    );
  };

  const loopCommand = async (text, ctx) => {
    const session = sessionOf(ctx);
    if (session === undefined) return;
    const given = text.trim();
    if (given === "stop") {
      const held = loops.get(session);
      if (!running(held)) {
        say(ctx, "No running loop is set for this session.");
        return;
      }
      closeLoop(session, held, "the operator stopped the loop");
      say(ctx, "Loop stopped.");
      return;
    }
    if (given === "status") {
      reportLoop(ctx, session);
      return;
    }
    if (isChild(ctx)) {
      refuseChild(ctx, "Loop");
      return;
    }
    const { rest: withoutMax, max, refused: maxRefused } = takeMax(given);
    const { rest, interval, refused } = takeInterval(withoutMax);
    const why = maxRefused ?? refused;
    if (why) {
      say(ctx, `Loop not started: ${why}.`, "warning");
      return;
    }
    if (rest === "") {
      say(ctx, "Loop not started: a prompt is needed, as in `/loop 5m check the build`, `/loop status` or `/loop stop`.", "warning");
      return;
    }
    if (running(loops.get(session))) {
      say(ctx, "This session's loop is already running, so nothing was sent. Use `/loop stop` to end it.");
      return;
    }
    const loop = { id: ++counter, prompt: rest, interval, max, runs: 0, started: new Date().toISOString() };
    loops.set(session, loop);
    repeat(ctx, session, loop);
  };

  const command = (name, description, work) =>
    pi.registerCommand(name, {
      description,
      handler: async (args, ctx) => {
        try {
          await work(String(args ?? ""), ctx);
        } catch (error) {
          console.error(`goal-loop: /${name} failed (${errorText(error)})`);
        }
      },
    });

  command("goal", "Keep working until a condition is judged met, or report the current goal", goalCommand);
  command("loop", "Repeat a prompt every interval, or after each turn when none is given", loopCommand);
}
