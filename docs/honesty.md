# What this instrument does not measure

An axis list that names what is not covered is the difference between "zero defects found" and
"zero defects found where we looked".

Read this before adding a claim elsewhere in the repository. It records known limits, their current
measurements and the tests that hold them. Unmeasured cases are not clearance.

`tests/honesty.rs` crosses the *countable* claims here against the code: the number of agents, the
number of tools, the number of doctor checks, the settings rows. A count that drifts fails the
suite. Everything else here is prose held by review.

## How this file is organised

[`docs/README.md`](README.md) maps the documentation set. Entries live in the area
files below, each in its original order; the section links under each area are navigation rather than
a subject index.

- [`honesty-testing.md`](honesty-testing.md) — Tests, the suite and the build: what the suite, its fixtures, the CI lane and the build do not establish.
  - [Test evidence, and the run id shape](honesty-testing.md#test-evidence-and-the-run-id-shape)
  - [Comments, instructions and the build](honesty-testing.md#comments-instructions-and-the-build)
  - [Node, CI and the actions that run the suite](honesty-testing.md#node-ci-and-the-actions-that-run-the-suite)
  - [The suite, its fixtures and the retired port](honesty-testing.md#the-suite-its-fixtures-and-the-retired-port)
  - [Test counts and mutation claims in a receipt](honesty-testing.md#test-counts-and-mutation-claims-in-a-receipt)
  - [Citations, and the coordinates that rot](honesty-testing.md#citations-and-the-coordinates-that-rot)
  - [Generated files and reproduction](honesty-testing.md#generated-files-and-reproduction)
  - [The npm installer and the registries](honesty-testing.md#the-npm-installer-and-the-registries)
  - [The release builds; CI tests](honesty-testing.md#the-release-builds-ci-tests)
- [`honesty-review-panels.md`](honesty-review-panels.md) — Review models, targets and panels: what the reviewer routing, the review target and the panel do not establish.
  - [Models, routing and reviewer definitions](honesty-review-panels.md#models-routing-and-reviewer-definitions)
  - [Review targets, panels and the lane that runs them](honesty-review-panels.md#review-targets-panels-and-the-lane-that-runs-them)
- [`honesty-review-findings.md`](honesty-review-findings.md) — Findings, grants and locations: what a recorded finding, a grant and a location do not establish.
  - [Findings, grants and locations](honesty-review-findings.md#findings-grants-and-locations)
- [`honesty-review-rules.md`](honesty-review-rules.md) — Review rounds, roles and corrections: what the roles, the round bounds, the correction budget and the publication lane do not establish.
  - [Roles and the definitions they need](honesty-review-rules.md#roles-and-the-definitions-they-need)
  - [Skips, deletions and the bounds of a round](honesty-review-rules.md#skips-deletions-and-the-bounds-of-a-round)
  - [The correction budget and the publication lane](honesty-review-rules.md#the-correction-budget-and-the-publication-lane)
  - [The review-only claim](honesty-review-rules.md#the-review-only-claim)
  - [Re-running a red lane](honesty-review-rules.md#re-running-a-red-lane)
- [`honesty-gate-surface.md`](honesty-gate-surface.md) — The control surface and the seams: what the control surface records, who else reads it, and the calls no gate sees.
  - [The control surface and what it records](honesty-gate-surface.md#the-control-surface-and-what-it-records)
  - [What the gate writes, and who else reads](honesty-gate-surface.md#what-the-gate-writes-and-who-else-reads)
  - [The seams, and the gates that never see a call](honesty-gate-surface.md#the-seams-and-the-gates-that-never-see-a-call)
  - [The push guard, the MCP server and the state a run keeps](honesty-gate-surface.md#the-push-guard-the-mcp-server-and-the-state-a-run-keeps)
- [`honesty-gate-reach.md`](honesty-gate-reach.md) — The gate's reach across the agents: what the gate reaches in each agent it is installed into, and what it does not.
  - [The gate's reach across the agents](honesty-gate-reach.md#the-gates-reach-across-the-agents)
- [`honesty-gate.md`](honesty-gate.md) — What the gate covers and narrows: what the gate covers, narrows and records, and the refusals it names.
  - [What the gate covers](honesty-gate.md#what-the-gate-covers)
  - [The populations and the prose they cross](honesty-gate.md#the-populations-and-the-prose-they-cross)
  - [Reachability, registration and the rounds around them](honesty-gate.md#reachability-registration-and-the-rounds-around-them)
  - [What the gate narrows, and what it records](honesty-gate.md#what-the-gate-narrows-and-what-it-records)
  - [Refusals, and the way out they name](honesty-gate.md#refusals-and-the-way-out-they-name)
  - [Which hook commands are Estigia's](honesty-gate.md#which-hook-commands-are-estigias)
- [`honesty-gate-boundary.md`](honesty-gate-boundary.md) — The gate's boundary: where the gate's shell and write boundary stops reading.
  - [The gate's boundary](honesty-gate-boundary.md#the-gates-boundary)
- [`honesty-delivery.md`](honesty-delivery.md) — Delivery, identity and ownership: what delivery, the tracker identity and ownership do not establish.
  - [Delivery: the receipt, the base and the remote](honesty-delivery.md#delivery-the-receipt-the-base-and-the-remote)
  - [Identity and the tracker](honesty-delivery.md#identity-and-the-tracker)
  - [Ownership](honesty-delivery.md#ownership)
  - [What `release_ci` says about CI](honesty-delivery.md#what-release_ci-says-about-ci)
  - [What `deliver` does not see](honesty-delivery.md#what-deliver-does-not-see)
  - [The HTTPS repeat](honesty-delivery.md#the-https-repeat)
- [`honesty-setup.md`](honesty-setup.md) — Setup, doctor and the host plugins: what setup, doctor and the generated host integrations do not establish.
  - [The OpenCode plugin, and what it knows about a call](honesty-setup.md#the-opencode-plugin-and-what-it-knows-about-a-call)
  - [Setup, configuration and uninstall](honesty-setup.md#setup-configuration-and-uninstall)
  - [doctor, and what it checks](honesty-setup.md#doctor-and-what-it-checks)
  - [The Pi system section and the prompt cache](honesty-setup.md#the-pi-system-section-and-the-prompt-cache)
- [`honesty-sessions.md`](honesty-sessions.md) — Sessions, mail and the records a run leaves: what the session registry, the mailbox and the local records do not establish.
  - [What `estigia changes` does not do](honesty-sessions.md#what-estigia-changes-does-not-do)
  - [The feed](honesty-sessions.md#the-feed)
  - [Where](honesty-sessions.md#where)
  - [The session registry](honesty-sessions.md#the-session-registry)
  - [The session mailbox](honesty-sessions.md#the-session-mailbox)
  - [The needs-person record](honesty-sessions.md#the-needs-person-record)
  - [The waits audit](honesty-sessions.md#the-waits-audit)
- [`honesty-cost.md`](honesty-cost.md) — Measurements and cost: what a measurement reconstructs and what the token-efficiency programme does not show.
  - [What a measurement reconstructs](honesty-cost.md#what-a-measurement-reconstructs)
  - [The token-efficiency programme](honesty-cost.md#the-token-efficiency-programme)

The last run, **The review tier, the lens and the finding** has its own
document: [`honesty-review-tier.md`](honesty-review-tier.md). It stays last in the order and nothing
follows it. Within it, "the entry above" and "the entry below" reach the neighbouring entry, and a
pointer that leaves it names the document it points at.
