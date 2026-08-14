# Installer lifecycle admission tasks

1. Add process tests for admission, idempotence, downgrade zero-write, malformed state, provenance
   conflict, parser shape, and hidden help.
2. Add static installer tests for verification, extraction, candidate invocation, native exit checks,
   no identity arguments, and replacement order.
3. Add candidate-owned lifecycle orchestration and hidden CLI dispatch.
4. Update both release installers to admit the extracted candidate before replacement.
5. Update README, changelog, handoff, and structured change documentation.
6. Run focused tests and every repository gate.
