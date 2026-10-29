# When a run needs a person

Question answered here: what Estigia records when a state needs a person, where, and what that record is not.

## What is recorded

One `needs_person` row in `~/.estigia/state.db`, written in the same place the authority already writes the
state it describes. It carries the kind, the repository, the issue (absent for `ask`), the run, the time, and
a `delivery` text. Nothing is posted to GitHub.

| Kind | Written by | Dedup key |
|---|---|---|
| `escalated` | `escalate_review`, on the first escalation and on a repeat that finds one | the publication epoch |
| `blocked` | `transition --to blocked` from another state, when the store holds no `blocked-until` clause for the issue | the time of the transition |
| `ask` | the gate, when it answers `ask` (a host that cannot ask converts it to a deny first, so only Claude Code reaches this) | the run, the refusal code and a thirty-minute bucket, so the same ask later is a new need |

A row is written once per repository and key: a repeat of the same moment writes nothing. A `blocked` issue
with a clause names what discharges it, so it is not a person's to decide and records nothing.

The row holds no message body and no free text from the agent: the reason an escalation was handed over stays
in the receipt it already lives in.

## Reading it

`estigia needs-person [--limit N] [--json]` lists the newest records first. It is read-only, and it does not
create the store it reads.

## Telling the person

The binary itself hands the OS a notification at the store write that records the need, so it is the same on
every runtime, including those with no gate. It happens only when the `Notifications` row is `os` (default
`off`, see [`configuration.md`](configuration.md)), and only for the write that created the record: a repeat
hands over nothing.

The text carries the kind, the repository, the issue and the run, each reduced to a safe alphabet with
token-shaped runs removed, and says it is not an approval. There is no action button. Spawning never blocks a
decision for more than two seconds and never fails one.

The `delivery` column says what became of it:

| `delivery` | Meaning |
|---|---|
| `handed to the OS` | the notifier exited 0. The OS has the text; nobody is known to have seen it |
| `held: quiet hours` | inside the `quiet` range, so nothing was handed over |
| `held: hourly cap` | `cap` notifications were already handed over in the past hour, counted in the store |
| `not delivered: channel off` | the row is `off` |
| `not delivered: no notifier` | `osascript`, `notify-send` or `powershell` is not on `PATH` here |
| `not delivered: notifier failed` | the notifier exited non-zero, could not start or did not answer in time |

A record that could not be written is reported, not swallowed: the answer of `escalate_review` and of the
transition carries `needs_person_unrecorded`, and a gate `ask` leaves a decision with the verdict
`unrecorded`.

## What a record is not

It is not evidence that anyone saw it: the strongest delivery is `handed to the OS`. It is not an approval, and
consent stays in the client's own form while recovery stays person-only. Recording is best effort and never
fails the write it describes. The gaps are in [`honesty.md`](honesty.md#the-needs-person-record).
