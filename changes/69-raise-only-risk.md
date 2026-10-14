section: The harness

- **`review_tier` takes a raise-only risk declaration and a focus reason.** Issue #69. An agent that knows a change carries
  a risk its diff signals cannot show had no way to ask for more review, and a `--focus` carried no reason. `review_tier`
  now accepts `raise`, one of six risk items (`authorization`, `migration`, `input`, `concurrency`,
  `irreversibility`, `contract`), with a required `raise_reason`; it lifts the tier to `high` and never lowers
  it, and the answer keeps the derived tier as `computed_level`. `focus_reason` is answered back beside a `focus`. Six
  refusals are new and listed in `src/harness/refusal_class.rs`: `invalid-risk-raise`, `risk-raise-lowers`,
  `risk-raise-reason-missing`, `risk-raise-reason-without-raise`, `blank-focus-reason` and `focus-reason-without-focus`.
  Without these arguments the answer is unchanged.
