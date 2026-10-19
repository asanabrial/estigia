# The documents, and what each one answers

One document answers one question. This is the map: a reader arrives with a question, leaves with
the document that owns it, and follows that document's own links from there.

| Document | Answers | Read it when |
|---|---|---|
| [`README.md`](../README.md) | What Estigia is, and what it enforces | you are deciding whether to use it |
| [`configuration.md`](configuration.md) | Every setting, its accepted values, and what each one governs | you are changing what a run may do |
| [`what-it-writes.md`](what-it-writes.md) | What setup puts in somebody else's files, and how uninstall takes it back | you are installing it, or removing it |
| [`lifecycle.md`](lifecycle.md) | What `estigia update` reports, what an installer records, and what neither proves | you are reading an install's provenance |
| [`refusal-classes.md`](refusal-classes.md) | The four classes a refusal carries, what each tells the caller to do, and where every code sits | you are reading a refusal, or adding a code |
| [`github-binding.md`](github-binding.md) | The incidents and arguments behind the GitHub binding's rules | you are asking why a binding rule exists |
| [`review-status.md`](review-status.md) | What `review_status` answers for each state of a review, what it reads, and why a stale answer is refused | you are acting on, or changing, the next step of a review |
| [`sessions.md`](sessions.md) | What the session registry and the mailbox record, what `estigia sessions` shows, where each runtime delivers, how a bounded `list_sessions` page and its cursor are bound, and which parts of issue #67 do not exist yet | you are asking which sessions are alive and what each holds |
| [`generated-files.md`](generated-files.md) | How a repository declares a generated file, which commit the declaration is read from, and what reproducing it answers | you are declaring a lockfile, or reading `generated_files` |
| [`review-cost.md`](review-cost.md) | What a blind review costs, measured, and what actually spends it | you are budgeting a review |
| [`honesty.md`](honesty.md) | What this instrument does **not** check, each gap with the measurement that found it | you are about to believe a claim |
| [`honesty-review-tier.md`](honesty-review-tier.md) | What the review tier, the lens and the finding do not establish — the last run of the ledger above | you are reading a tier, a lens or a finding claim |
| [`parity.md`](parity.md) | Which `gentle-ai` revision Estigia measures against, and which layer is in scope | you are asking whether Estigia is at parity |
| [`AGENTS.md`](../AGENTS.md) | How an agent works in this repository, and what it owes the documentation | you are about to change this repository |
| [`CHANGELOG.md`](../CHANGELOG.md) | What changed, released version by released version | you need the history of a behaviour |

Two things a reader might look for are deliberately not in the set. [`skill/`](../skill/SKILL.md) is
the contract this crate **ships** to other people's agents: it is written to be installed and read
elsewhere, and this repository is not its audience. Where the work stands is on the tracker, not in a
file in the checkout.

Where two documents seem to disagree, [`honesty.md`](honesty.md) is the one to read: it says what is
not measured, and a document that contradicts it is a document that will be believed.
