#!/usr/bin/env python3
"""Re-measure Claude Code transcripts the way the baseline measurement did.

python3 scripts/measure-transcripts.py [--projects DIR] [--json]

Reads every top-level `*.jsonl` session transcript under `--projects` (default
`~/.claude-personal/projects`), plus each session's `subagents/*.jsonl` and
`.meta.json` siblings, and reports session count, input tokens by context,
forced-`Stop` turns and per-issue input tokens — per repository and overall.
Repository attribution and detail are in `docs/review-cost.md`; this file is
the mechanism, not the discussion of what it found.

Beside `context_input_tokens` it reports the usage classes separately, from the
record's own counters. The classes are the keys of `CLAUDE_USAGE_KEYS`, this
CLI's mapping, and a class no measured record carried is `null` rather than
`0`, so an unobserved counter is never read as a zero; a record missing any of
them is counted in `usage.records_missing_a_counter`, which makes the class
totals a floor rather than a silent one. A provider whose cached counters are
already inside its input total needs its own mapping, not this one.

`turn_profile` counts, per context, the model turns (distinct assistant message
identities), the model the API answered with, the model the CLI asked for, the
effort, and the tool calls (distinct `tool_use` identities). `usage_by_model`
splits the usage classes by the model that answered, and `sessions` lists each
session with its wall-clock span and the subagents it launched. A field a record
does not carry is counted under `unknown`, never guessed.
"""

import argparse
import collections
import datetime
import glob
import json
import os
import statistics
import sys

CLAUDE_USAGE_KEYS = {
    "input": "input_tokens",
    "cache_read": "cache_read_input_tokens",
    "cache_creation": "cache_creation_input_tokens",
    "output": "output_tokens",
}


def context_tokens(usage):
    return (
        usage.get("input_tokens", 0)
        + usage.get("cache_read_input_tokens", 0)
        + usage.get("cache_creation_input_tokens", 0)
    )


def new_usage_total():
    return {
        "tokens": dict.fromkeys(CLAUDE_USAGE_KEYS),
        "records": 0,
        "records_missing_a_counter": 0,
    }


def add_usage(total, usage):
    total["records"] += 1
    missing = False
    for name, key in CLAUDE_USAGE_KEYS.items():
        value = usage.get(key)
        if value is None:
            missing = True
            continue
        total["tokens"][name] = value + (total["tokens"][name] or 0)
    if missing:
        total["records_missing_a_counter"] += 1
    return total


def merge_usage(target, source):
    for name, value in source["tokens"].items():
        if value is not None:
            target["tokens"][name] = value + (target["tokens"][name] or 0)
    target["records"] += source["records"]
    target["records_missing_a_counter"] += source["records_missing_a_counter"]
    return target


UNKNOWN = "unknown"


def measured_records(records):
    seen = set()
    for record in records:
        message = record.get("message") or {}
        message_id = message.get("id") or record.get("uuid")
        if record.get("type") == "assistant" and "usage" in message and message_id not in seen:
            seen.add(message_id)
            yield record


def measured_usages(records):
    for record in measured_records(records):
        yield record["message"]["usage"]


def assistant_turns(records):
    seen = set()
    for record in records:
        if record.get("type") != "assistant":
            continue
        message = record.get("message") or {}
        message_id = message.get("id") or record.get("uuid")
        if message_id not in seen:
            seen.add(message_id)
            yield record


def tool_call_count(records):
    identities = set()
    anonymous = 0
    for record in records:
        if record.get("type") != "assistant":
            continue
        content = (record.get("message") or {}).get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if block.get("type") != "tool_use":
                continue
            if block.get("id"):
                identities.add(block["id"])
            else:
                anonymous += 1
    return len(identities) + anonymous


def new_turn_profile():
    return {
        "model_turns": 0,
        "served_models": {},
        "requested_models": {},
        "efforts": {},
        "tool_calls": 0,
    }


def tally(counts, key):
    key = key if key else UNKNOWN
    counts[key] = counts.get(key, 0) + 1


def turn_profile(records):
    profile = new_turn_profile()
    for record in assistant_turns(records):
        profile["model_turns"] += 1
        tally(profile["served_models"], (record.get("message") or {}).get("model"))
        tally(profile["requested_models"], record.get("requestedModel"))
        tally(profile["efforts"], record.get("effort"))
    profile["tool_calls"] = tool_call_count(records)
    return profile


def merge_turn_profile(target, source):
    target["model_turns"] += source["model_turns"]
    target["tool_calls"] += source["tool_calls"]
    for field in ("served_models", "requested_models", "efforts"):
        for key, count in source[field].items():
            target[field][key] = target[field].get(key, 0) + count
    return target


def usage_by_model(records):
    totals = {}
    for record in measured_records(records):
        model = record["message"].get("model") or UNKNOWN
        totals[model] = add_usage(totals.get(model, new_usage_total()), record["message"]["usage"])
    return totals


def merge_usage_by_model(target, source):
    for model, total in source.items():
        target[model] = merge_usage(target.get(model, new_usage_total()), total)
    return target


def timestamps(records):
    for record in records:
        stamp = record.get("timestamp")
        if not isinstance(stamp, str):
            continue
        try:
            yield datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00"))
        except ValueError:
            continue


def wall_clock_seconds(stamps):
    stamps = list(stamps)
    if not stamps:
        return None
    return (max(stamps) - min(stamps)).total_seconds()


def repo_of(project_dir_name):
    marker = "-repo-"
    at = project_dir_name.find(marker)
    if at == -1:
        return project_dir_name
    return project_dir_name[at + len(marker) :]


def read_jsonl(path):
    records = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return records


def agent_type_of(subagent_path):
    meta_path = subagent_path[:-6] + ".meta.json"
    try:
        with open(meta_path, encoding="utf-8") as handle:
            return json.load(handle).get("agentType") or "unknown"
    except (OSError, json.JSONDecodeError):
        return "unknown"


def issue_counts(records):
    counts = collections.Counter()
    for record in records:
        content = (record.get("message") or {}).get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if block.get("type") != "tool_use":
                continue
            name = block.get("name", "")
            if not name.startswith("mcp__estigia__"):
                continue
            issue = (block.get("input") or {}).get("issue")
            if issue is not None:
                counts[issue] += 1
    return counts


def dominant_issue(counts):
    if not counts:
        return None
    return counts.most_common(1)[0][0]


def stop_forced_turns(records):
    forced = []
    after_stop = False
    for record in records:
        if record.get("type") == "system" and record.get("subtype") == "stop_hook_summary":
            if any("estigia" in error for error in record.get("hookErrors", [])):
                after_stop = True
            continue
        message = record.get("message") or {}
        if record.get("type") == "assistant" and "usage" in message and after_stop:
            forced.append(context_tokens(message["usage"]))
            after_stop = False
    return forced


def session_context_tokens(records):
    return sum(context_tokens(usage) for usage in measured_usages(records))


def session_usage(records):
    total = new_usage_total()
    for usage in measured_usages(records):
        add_usage(total, usage)
    return total


def measure_session(path):
    records = read_jsonl(path)
    counts = issue_counts(records)
    stamps = list(timestamps(records))
    session = {
        "path": path,
        "mtime": os.path.getmtime(path),
        "orchestrator_tokens": session_context_tokens(records),
        "forced_stop_turns": stop_forced_turns(records),
        "issue": dominant_issue(counts),
        "issues": sorted(str(issue) for issue in counts),
        "usage": {"orchestrator": session_usage(records)},
        "subagents": collections.defaultdict(int),
        "turn_profile": {"orchestrator": turn_profile(records)},
        "usage_by_model": usage_by_model(records),
        "subagents_launched": 0,
    }
    directory = path[:-6]
    for subagent_path in sorted(glob.glob(os.path.join(directory, "subagents", "*.jsonl"))):
        kind = agent_type_of(subagent_path)
        transcripts = read_jsonl(subagent_path)
        session["subagents"][kind] += session_context_tokens(transcripts)
        session["usage"][kind] = merge_usage(
            session["usage"].get(kind, new_usage_total()), session_usage(transcripts)
        )
        session["turn_profile"][kind] = merge_turn_profile(
            session["turn_profile"].get(kind, new_turn_profile()), turn_profile(transcripts)
        )
        merge_usage_by_model(session["usage_by_model"], usage_by_model(transcripts))
        session["subagents_launched"] += 1
        stamps.extend(timestamps(transcripts))
    session["wall_clock_seconds"] = wall_clock_seconds(stamps)
    return session


def find_project_sessions(projects_dir):
    by_repo = collections.defaultdict(list)
    if not os.path.isdir(projects_dir):
        return by_repo
    for entry in sorted(os.listdir(projects_dir)):
        project_path = os.path.join(projects_dir, entry)
        if not os.path.isdir(project_path):
            continue
        repo = repo_of(entry)
        for jsonl_path in sorted(glob.glob(os.path.join(project_path, "*.jsonl"))):
            by_repo[repo].append(jsonl_path)
    return by_repo


def summarise(sessions):
    context_totals = collections.defaultdict(int)
    usage_totals = {}
    profiles = {}
    by_model = {}
    subagents_launched = 0
    forced_turns = []
    per_issue = collections.defaultdict(int)
    several_issues = 0
    last_modified = None
    for session in sessions:
        context_totals["orchestrator"] += session["orchestrator_tokens"]
        for kind, tokens in session["subagents"].items():
            context_totals[kind] += tokens
        for kind, total in session["usage"].items():
            usage_totals[kind] = merge_usage(usage_totals.get(kind, new_usage_total()), total)
        for kind, profile in session["turn_profile"].items():
            profiles[kind] = merge_turn_profile(profiles.get(kind, new_turn_profile()), profile)
        merge_usage_by_model(by_model, session["usage_by_model"])
        subagents_launched += session["subagents_launched"]
        forced_turns.extend(session["forced_stop_turns"])
        if session["issue"] is not None:
            per_issue[str(session["issue"])] += session["orchestrator_tokens"]
        if len(session["issues"]) > 1:
            several_issues += 1
        if last_modified is None or session["mtime"] > last_modified:
            last_modified = session["mtime"]
    return {
        "session_count": len(sessions),
        "context_input_tokens": dict(context_totals),
        "usage": {
            "tokens": {
                kind: dict(total["tokens"]) for kind, total in sorted(usage_totals.items())
            },
            "records": sum(total["records"] for total in usage_totals.values()),
            "records_missing_a_counter": sum(
                total["records_missing_a_counter"] for total in usage_totals.values()
            ),
        },
        "turn_profile": dict(sorted(profiles.items())),
        "usage_by_model": {
            model: dict(total["tokens"]) for model, total in sorted(by_model.items())
        },
        "subagents_launched": subagents_launched,
        "sessions_with_several_issues": several_issues,
        "forced_stop_turns_count": len(forced_turns),
        "forced_stop_turns_median_context": (
            statistics.median(forced_turns) if forced_turns else None
        ),
        "per_issue_input_tokens": per_issue,
        "last_modified": last_modified,
    }


def session_row(projects_dir, session):
    return {
        "path": os.path.relpath(session["path"], projects_dir),
        "issue": session["issue"],
        "wall_clock_seconds": session["wall_clock_seconds"],
        "subagents_launched": session["subagents_launched"],
        "turn_profile": dict(sorted(session["turn_profile"].items())),
        "usage": {kind: dict(total["tokens"]) for kind, total in sorted(session["usage"].items())},
        "usage_by_model": {
            model: dict(total["tokens"])
            for model, total in sorted(session["usage_by_model"].items())
        },
    }


def render_table(label, summary):
    lines = [f"== {label} =="]
    lines.append(f"sessions: {summary['session_count']}")
    for kind, tokens in sorted(
        summary["context_input_tokens"].items(), key=lambda kv: -kv[1]
    ):
        lines.append(f"  {kind:16} {tokens:>12} input tokens")
    if summary["usage"]["tokens"]:
        lines.append("usage classes (unknown: no measured record carried the counter):")
        for kind, classes in sorted(summary["usage"]["tokens"].items()):
            shown = "  ".join(
                f"{name} {value if value is not None else 'unknown'}"
                for name, value in classes.items()
            )
            lines.append(f"  {kind:16} {shown}")
        if summary["usage"]["records_missing_a_counter"]:
            lines.append(
                "records missing a counter: "
                f"{summary['usage']['records_missing_a_counter']}"
                " (the class totals are a floor)"
            )
    lines.append(
        f"forced Stop turns: {summary['forced_stop_turns_count']}"
        f" (median context {summary['forced_stop_turns_median_context']})"
    )
    if summary["per_issue_input_tokens"]:
        lines.append("per-issue input tokens:")
        for issue, tokens in sorted(summary["per_issue_input_tokens"].items()):
            lines.append(f"  issue {issue}: {tokens} input tokens")
    if summary["sessions_with_several_issues"]:
        lines.append(
            f"sessions naming several issues: {summary['sessions_with_several_issues']}"
            " (each is attributed to its dominant issue)"
        )
    if summary["last_modified"] is not None:
        lines.append(f"most recent session: {summary['last_modified']}")
    return "\n".join(lines)


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--projects",
        default=os.path.expanduser("~/.claude-personal/projects"),
        help="Directory holding Claude Code's per-project transcript folders.",
    )
    parser.add_argument(
        "--json", action="store_true", help="Print JSON only, no table."
    )
    args = parser.parse_args(argv)

    by_repo = find_project_sessions(args.projects)
    per_repo_summary = {}
    all_sessions = []
    for repo, paths in by_repo.items():
        sessions = [measure_session(path) for path in paths]
        all_sessions.extend(sessions)
        per_repo_summary[repo] = summarise(sessions)
    overall = summarise(all_sessions)

    result = {
        "projects": args.projects,
        "by_repo": per_repo_summary,
        "overall": overall,
        "sessions": [session_row(args.projects, session) for session in all_sessions],
    }

    if not args.json:
        for repo, summary in sorted(per_repo_summary.items()):
            print(render_table(repo, summary))
            print()
        print(render_table("overall", overall))
        print()

    print(json.dumps(result, indent=2, sort_keys=True, default=str))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
