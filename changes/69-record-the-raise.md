section: The harness

- **A raise is recorded with the publication and the gate reads it.** Issue #69. `publish_review` and `republish_review`
  now take `raise`, `raise_reason`, `focus` and `focus_reason`, refuse them with the reasons `review_tier` already
  names, and record them with the computed tier on the published marker; `review_read` with `view: transaction` answers
  `computed_level`, `raise`, `raise_reason`, `focus` and `focus_reason`. The tier `review_status`, the computed verdict
  and `release_ci` derive takes the larger of the diff's tier and `high` for a recorded raise, so it never lowers and
  the lens counts per tier are unchanged. The skill states the six risk items and the raise-only rule, citing the binary
  as the authority. A raise is a caller's declaration and nothing checks that it applies; `docs/honesty-review-tier.md`
  says what it does not cover.
