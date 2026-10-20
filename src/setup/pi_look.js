// estigia:look — managed by `estigia setup pi`
//
// Draws the Estigia header and footer, and puts Pi's own back on
// `/estigia-look off`. Display only: it registers no tool, sends no message,
// appends no entry and decides nothing.
// Removed by `estigia uninstall pi`.
import { existsSync, readFileSync, statSync } from "node:fs";

const COMMAND = "estigia-look";
const CLAIM = "estigia-claim";
const MARK = "estigIA";
const SUBTITLE = "AI harness";
const TAGLINE = [
  ["Light.", "mdLink"],
  ["Local.", "success"],
  ["Agentic.", "error"],
];
const TITLE = "estigia";
const NO_CLAIM = "no claim";
const SEPARATOR = " \u00b7 ";
const drawn = new WeakSet();

const paint = (theme, token, text) =>
  typeof theme?.fg === "function" ? theme.fg(token, text) : text;

const strong = (theme, text) => (typeof theme?.bold === "function" ? theme.bold(text) : text);

const widthOf = (text) => String(text).replace(/\u001b\[[0-9;]*m/g, "").length;

const home = () => process.env.HOME || process.env.USERPROFILE;

const abbreviated = (path) => {
  const text = String(path ?? "");
  const base = home();
  if (!base || !text.startsWith(base)) return text;
  return `~${text.slice(base.length)}`;
};

const gitDirOf = (cwd) => {
  const dot = `${cwd}/.git`;
  if (!existsSync(dot)) return undefined;
  if (statSync(dot).isDirectory()) return dot;
  const named = readFileSync(dot, "utf8").trim();
  const prefix = "gitdir:";
  if (!named.startsWith(prefix)) return undefined;
  const target = named.slice(prefix.length).trim();
  return target.startsWith("/") ? target : `${cwd}/${target}`;
};

const branchAt = (cwd) => {
  try {
    const dir = gitDirOf(cwd);
    if (dir === undefined) return undefined;
    const head = readFileSync(`${dir}/HEAD`, "utf8").trim();
    const prefix = "ref: refs/heads/";
    return head.startsWith(prefix) ? head.slice(prefix.length) : head.slice(0, 7);
  } catch {
    return undefined;
  }
};

const claimOf = (footerData) => {
  const text = footerData?.getExtensionStatuses?.().get(CLAIM);
  if (typeof text !== "string" || text.trim() === "") return undefined;
  return text.replace(/^estigia:\s*/, "").replaceAll(" \u00b7 ", " ");
};

const others = (footerData) =>
  [...(footerData?.getExtensionStatuses?.() ?? [])]
    .filter(([key, text]) => key !== CLAIM && typeof text === "string" && text.trim() !== "")
    .map(([key, text]) => `${key} ${text}`);

const contextOf = (ctx) => {
  try {
    const percent = ctx?.getContextUsage?.()?.percent;
    return typeof percent === "number" ? percent : undefined;
  } catch {
    return undefined;
  }
};

const rule = (theme, width) => paint(theme, "border", "\u2500".repeat(Math.max(0, width)));

const headerComponent = (theme, ctx, branch) => ({
  render: (width) => {
    const lines = [];
    const subtitle = width >= 24 ? paint(theme, "muted", ` ${SUBTITLE}`) : "";
    lines.push(strong(theme, paint(theme, "accent", MARK)) + subtitle);
    if (width >= 40) {
      lines.push(TAGLINE.map(([text, token]) => paint(theme, token, text)).join(" "));
      const where = [];
      const cwd = abbreviated(ctx?.cwd ?? process.cwd());
      if (cwd !== "") where.push(cwd);
      if (typeof branch === "string" && branch !== "") where.push(branch);
      const model = ctx?.model?.id;
      if (width >= 60 && typeof model === "string" && model !== "") where.push(model);
      if (where.length > 0) lines.push(paint(theme, "muted", where.join(SEPARATOR)));
      lines.push(rule(theme, width));
    }
    return lines;
  },
  invalidate: () => {},
});

const footerComponent = (theme, ctx, footerData) => {
  let counted = -1;
  let cost;
  const spend = () => {
    let seen = 0;
    try {
      seen = ctx?.sessionManager?.getEntryCount?.() ?? 0;
      const entries = ctx?.sessionManager?.getEntries?.() ?? [];
      let total = 0;
      let found = false;
      for (const entry of entries) {
        const usage =
          entry?.type === "usage"
            ? entry.usage
            : entry?.type === "message" && entry.message?.usage
              ? entry.message.usage
              : entry?.usage;
        const value = usage?.cost?.total;
        if (typeof value === "number") {
          total += value;
          found = true;
        }
      }
      return found ? total : undefined;
    } catch {
      return undefined;
    } finally {
      counted = seen;
    }
  };
  return {
    render: (width) => {
      const count = ctx?.sessionManager?.getEntryCount?.() ?? 0;
      if (count !== counted) cost = spend();
      const claim = claimOf(footerData);
      const branch = footerData?.getGitBranch?.();
      const model = ctx?.model?.id;
      const thinking = ctx?.thinkingLevel;
      const percent = contextOf(ctx);
      const segments = [
        { text: strong(theme, paint(theme, "accent", TITLE)), drop: 0 },
        {
          text: claim === undefined ? paint(theme, "dim", NO_CLAIM) : paint(theme, "mdHeading", claim),
          drop: 0,
        },
      ];
      if (typeof branch === "string" && branch !== "") {
        segments.push({ text: paint(theme, "muted", branch), drop: 2 });
      }
      if (typeof model === "string" && model !== "") {
        const shown =
          typeof thinking === "string" && thinking !== "off" ? `${model} ${thinking}` : model;
        segments.push({ text: paint(theme, "muted", shown), drop: 3 });
      }
      if (percent !== undefined) {
        const token = percent >= 90 ? "error" : percent >= 70 ? "warning" : "dim";
        segments.push({ text: paint(theme, token, `ctx ${Math.round(percent)}%`), drop: 1 });
      }
      if (cost !== undefined) {
        segments.push({ text: paint(theme, "muted", `$${cost.toFixed(3)}`), drop: 4 });
      }
      const separator = paint(theme, "dim", SEPARATOR);
      const line = (list) => list.map((segment) => segment.text).join(separator);
      while (segments.length > 2 && widthOf(line(segments)) > width) {
        const highest = Math.max(...segments.map((segment) => segment.drop));
        if (highest === 0) break;
        segments.splice(segments.findIndex((segment) => segment.drop === highest), 1);
      }
      const lines = [line(segments)];
      const waiting = others(footerData);
      if (waiting.length > 0) lines.push(paint(theme, "dim", waiting.join(SEPARATOR)));
      return lines;
    },
    invalidate: () => {},
  };
};

const usable = (ctx) =>
  ctx?.mode === "tui" &&
  typeof ctx?.ui?.setHeader === "function" &&
  typeof ctx?.ui?.setFooter === "function";

const draw = (ctx) => {
  if (!usable(ctx)) return false;
  const branch = branchAt(ctx?.cwd ?? process.cwd());
  ctx.ui.setHeader((tui, theme) => headerComponent(theme, ctx, branch));
  ctx.ui.setFooter((tui, theme, footerData) => footerComponent(theme, ctx, footerData));
  drawn.add(ctx);
  return true;
};

const erase = (ctx) => {
  if (typeof ctx?.ui?.setHeader !== "function" || typeof ctx?.ui?.setFooter !== "function") {
    return false;
  }
  ctx.ui.setHeader(undefined);
  ctx.ui.setFooter(undefined);
  drawn.delete(ctx);
  return true;
};

export default function (pi) {
  pi.registerCommand?.(COMMAND, {
    description: "Draw or restore the Estigia header and footer (shown to you, not sent to the model)",
    handler: async (args, ctx) => {
      const answer = String(args ?? "").trim().toLowerCase();
      if (answer === "off") {
        if (erase(ctx)) ctx.ui.notify("estigia: the header and footer are Pi's again", "info");
        return;
      }
      if (answer === "" || answer === "on") {
        if (draw(ctx)) ctx.ui.notify("estigia: the header and footer are drawn", "info");
        return;
      }
      ctx.ui.notify("estigia: /estigia-look takes `on` or `off`", "warning");
    },
  });
  pi.on?.("session_start", async (_event, ctx) => {
    draw(ctx);
  });
}
