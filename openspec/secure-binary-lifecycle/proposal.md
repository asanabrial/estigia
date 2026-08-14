# Secure binary lifecycle proposal

## Problem

Setup and sync deploy this build's embedded assets. Without recorded payload identity or a machine
release floor, an older or unrecorded executable can silently rewrite every configured adapter.

## Change

Add the smallest coherent safety boundary: local read-only update status, create-once machine-wide
recorded provenance and release history, and one fail-closed preflight before setup/sync batch
mutation. This is protection against accidental stale/source deployment and cooperating older
writers, not authentication against a malicious same-user process.

## Out of scope

No mapped-code or exact-executing-byte authentication, authenticated installer record, production record publisher, remote release discovery, HTTP
dependency, binary download, executable replacement, rollback, publication/mutation lock, or mutating
`upgrade` command belongs to this slice.
