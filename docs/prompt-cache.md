# What the session context carries, and which of it changes

The Pi extension puts Estigia's session context in a **system-prompt section** (`src/setup/plugin.rs`,
`PI_EXTENSION`'s `before_agent_start`, `options.sections[SECTION]`). A system prompt is the part a
provider can cache across turns, so anything in that section which changes invalidates the cache for
everything after it. This page measures what the section carries and how long each part lives. It does
**not** move anything: the placement change is an experiment, and [`honesty.md`](honesty.md) says what
this measurement cannot establish.

## What it is

`sessionContext` runs `estigia session start --session-id <root> --runtime pi --wake poll`, trimmed,
and puts the whole answer in the section. The answer is `src/harness/hook.rs`'s
`session_start_text`/`session_start_body`, so the section is whatever that body says — the same text a
Claude or OpenCode session receives at its own start, only the delivery differs.

## Measured

Run on 2026-10-22 with this checkout's `estigia` binary built at its own head and a scratch `HOME`, so
the real store was untouched. The command is the extension's own, `--wake poll` included:

```
HOME=$(mktemp -d) estigia session start --session-id probe-a --runtime pi --wake poll
```

1,228 bytes, and 1,228 bytes again for a second probe: the section's *size* is stable while its
*content* is not. The two answers differ on one line — the run id — and the four parts it carries have
four different lifetimes:

| Part | Bytes here | Changes when |
|---|---|---|
| The `## Estigia` heading | 10 | never |
| The "no contract is installed in …" sentence | 141 | the machine's setup changes |
| The run-id line: which issue the run holds, and whether the gate is armed | 126 | a claim, a renewal, a transition, or a session start |
| The uncommitted-work paragraph, naming each checkout and how many files it holds | 945, with five checkouts and their full paths | **any file is edited in any checkout of the repository** |

The four parts are separated by blank lines, which are the remaining six bytes: 10 + 126 + 141 + 945 + 6
= 1,228. A fifth part appears when the build and the checkout disagree — with the binary one commit
behind, the section was 1,797 bytes, the extra 567 saying the gate is not established as current.

The last row is the one worth reading twice. It is an inventory of other checkouts, none of which the
running session is working on, and it changes on the most frequent event there is: **945 of the 1,228
bytes, three quarters of the section**, in a sample where every one of those checkouts belongs to
somebody else. Its size follows the number of checkouts and the length of their paths, so on a machine
with two checkouts it is smaller and on one with ten it is larger.

## What a change would have to answer

Moving the volatile parts out of the system section and delivering them per turn — as a message or a
tool result — would leave the section's bytes identical across turns of one session. That is a
**placement** change, and placement is attention: a fact that used to arrive before the model chose
anything would arrive with the turn instead, and a model that ignored the section would now ignore a
message too. So it is an opt-in experiment, not an edit:

- the stable prefix has to stay byte-identical across turns of one session, measured, not argued;
- the fresh facts have to arrive **before** the decisions they govern — the gate's own refusals are the
  authority and are read at the call, so nothing here is what authorises a write;
- one host's placement is not every host's: Pi exposes `systemPromptOptions.sections`, and the
  Claude/OpenCode paths reach their own hook surfaces. An unsupported host keeps today's placement and
  claims no saving;
- the measurement that would settle it is a cache-hit comparison on one host and model, with the same
  task before and after, and a fallback that restores the current placement.

None of that is done here.

## What this page is not

It is not a saving. It is not a cache measurement: it counts the section's bytes and the two answers
that differ, and a provider's cache is where the cost would be. It is not a compatibility claim for any
host other than the one command above, run on this machine. And it is not a recommendation to remove
the uncommitted-work paragraph — that paragraph is a feature a person asked for, and moving it is the
experiment's question, not this page's answer.
