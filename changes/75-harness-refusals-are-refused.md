section: The harness

- **A refusal the gate itself builds for a rule is classed `refused`.** `issue-required`, `issue-creation-outside-flow` and `unclaimed-commit` were missing from the class table, so they rendered `Class: unavailable`, which says the control surface could not be read and invites a replay that cannot succeed. They are `refused` now. The crossing test now also reads the `const CODE` literals the harness spells, so an unnamed one fails the build. No gate decision and no other code's class changes.
