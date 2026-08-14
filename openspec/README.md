# openspec

Specification work for changes to this repository, in the layout
[`skill/protocols/sdd.md`](../skill/protocols/sdd.md) defines. This crate ships that protocol to
other people's agents; keeping its own documentation in the same shape is the cheapest way to find
out when the shape is wrong.

## What lives here

```text
openspec/
  config.yaml        the conventions this repository works under
  <change-id>/
    proposal.md      what is being changed, and what is not
    spec.md          the acceptance criteria
    design.md        the shape, and the decisions inside it
    tasks.md         the ordered work
```

One directory per change, named for the issue or the change it belongs to. A change small enough to
need no proposal needs no directory either — this is for work whose shape has to be agreed before it
is written, not a filing requirement.

## What it is not

**Not authority.** The protocol is explicit about this and it applies to this repository too: when
`openspec/` and the issue disagree, *the issue wins*. The tracker is the surface a claim is
adjudicated on and the one another run can read; a file in a checkout is neither. Record a
divergence rather than resolving it silently.

**Not a schema.** `config.yaml` is convention. Nothing parses it into a contract, so a field you do
not recognise is somebody's note — leave it alone.

## How this relates to the rest

Five documents describe this repository, and they answer different questions. Follow the one that
matches what you are asking:

| Where | Answers |
|---|---|
| [`README.md`](../README.md) | What Estigia is, and what it enforces |
| [`docs/honesty.md`](../docs/honesty.md) | What it does **not** check, each gap with the measurement that found it |
| [`docs/`](../docs/) | The reference the README points at: the settings table, what setup writes, the binary lifecycle |
| [`CHANGELOG.md`](../CHANGELOG.md) | What changed, released version by released version |
| [`AGENTS.md`](../AGENTS.md) | How an agent works in this repository, and what it owes the documentation |
| `openspec/` | What a change is *for*, agreed before it is written |

The working log is not among them: it is kept outside the repository, so a clone no longer carries a
row pointing at a file it does not have. `docs/honesty.md` took its place in the count by leaving the
README, where it had grown to 55% of the file — the most important document in the repository,
reachable only by scrolling past everything else.

`docs/honesty.md` is the one to read before adding a claim anywhere else: it
carries what this crate knows it does not check, and a new document that contradicts it is a
document that will be believed.

## Writing here

The prose in this repository states the failure a rule prevents, not only the rule. That is the
house style and it is worth keeping: a spec that says *what* without *what goes wrong otherwise* is
one nobody can weigh when it conflicts with something else.
