# Secure binary lifecycle design

## State shape

`~/.estigia/lifecycle/provenance/<sha256>.json` records a digest of bytes observed through an executable
pathname, compiled canonical SemVer, and a SHA-256 over `skill::FILES` plus `AGENT_DEFINITIONS`. Its publisher
derives version/assets internally. The asset hash has a versioned domain, collection count, typed
collection/path/content tags, per-collection counts, and eight-byte lengths.
`~/.estigia/lifecycle/releases/<canonical-semver>.json` records one release observation; build metadata
is rejected. Files are published create-once through a unique same-directory stage opened with
`create_new`, `write_all`, `sync_all`, and a hard link. Existing semantically equal records are
idempotent and different values conflict; unknown fields are rejected. Readers open once, inspect and
read that same handle, reject final Unix symlinks and Windows reparse points/non-regular records, and derive high-water as the
greatest SemVer, so there is no mutable scalar for cooperating older writers to lower.

## Status

The path returned by `current_exe` is reopened and streamed through SHA-256. This is raceable observed
inventory and a record lookup key, not mapped-image or exact executing-byte identity. A matching record
must match the version and asset digest compiled into this process; otherwise status is `unknown`. A
matching installer record and derived high-water produce `current`, `downgrade_blocked`,
`ahead_of_recorded`, or `recorded_no_history`. No record means source/unrecorded. Any evidence read
fault produces `unknown`. Public release status is `unavailable` with `checked: false` because this
slice deliberately has no network client.

## Mutation boundary

CLI dispatch performs preflight after argument-only validation and target selection but before
configuration reads that lead into adapter mutation. Guided setup checked before opening the screen
when this change was written; #118 moved that check to the screen's install, because opening the
screen deploys no adapter asset and the door withheld nothing the flag would not grant one command
later, while costing the operator the rows the screen exists to show. The check is global rather than
per-adapter because setup batches intentionally continue after an adapter failure. Dry-run returns
before lifecycle inspection because it cannot deploy assets.

## Threat and concurrency boundary

The JSON records are local recorded history, not signatures or authenticated receipts. The protected
object is compiled SemVer plus the exact embedded assets setup/sync can deploy: a pathname replacement
with the same values cannot downgrade that payload, while different values fail. They do not
authenticate mapped code, arbitrary code, or same-user state; intermediate path components retain
platform pathname-resolution races. No production installer emits records yet.
The corrected framing and field model is schema 3; schema-2 evidence is unsupported and fails closed.

Preflight is an unlocked snapshot and is not atomic with the later setup/sync batch. Re-reading before
each adapter would not close the race and could turn it into partial deployment, so this slice does
not make that change or claim atomicity. The later installer/upgrade slice must coordinate release
publication with setup/sync mutation.
