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
| `ask` | the gate, when it answers `ask` (a host that cannot ask converts it to a deny first, so only Claude Code reaches this) | the run and the refusal code |

A row is written once per repository and key: a repeat of the same moment writes nothing. A `blocked` issue
with a clause names what discharges it, so it is not a person's to decide and records nothing.

The row holds no message body and no free text from the agent: the reason an escalation was handed over stays
in the receipt it already lives in.

## Reading it

`estigia needs-person [--limit N] [--json]` lists the newest records first. It is read-only, and it does not
create the store it reads.

## What a record is not

It is not a notification: no notifier exists yet, so every row says `not delivered: no notifier`. It is not
an approval and it is not evidence that anyone saw it. Consent stays in the client's own form, and recovery
stays person-only. Recording is best effort and never fails the write it describes. The gaps are in
[`honesty.md`](honesty.md#the-needs-person-record).
