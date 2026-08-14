# Installer lifecycle admission proposal

## Problem

The completed [secure binary lifecycle](../secure-binary-lifecycle/proposal.md) slice created strict
local records and setup/sync preflight, but official installers did not publish those records. A
release install therefore left the binary indistinguishable from an unrecorded source build.

## Change

Make the extracted candidate publish candidate-derived lifecycle provenance and release history before
an official installer replaces the destination executable.

## Out of scope

No authenticated same-user state, mandatory attestation, public Release assertion, remote update,
self-upgrade, rollback, installer serialization, or atomic publication-and-copy transaction.
