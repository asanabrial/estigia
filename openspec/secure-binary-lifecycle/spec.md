# Secure binary lifecycle specification

## Acceptance criteria

1. `estigia update` emits text and global `--json` status without changing lifecycle state.
2. Status distinguishes source/unrecorded pathname observations, installer-recorded payload identity and local release relation,
   blocked downgrade, unknown state, and unavailable latest public release without claiming current.
3. Machine state is separate from adapter install records and survives uninstall.
4. Provenance is create-once by raceable observed-path SHA-256 lookup key and requires exact compiled
   package version plus typed/count-framed embedded-asset digest; publishers derive those values;
   releases are create-once by canonical SemVer without build metadata; the
   greatest recorded release is the high-water and cannot be lowered by an older cooperating writer.
5. Missing state is ordinary; malformed, unreadable, non-canonical, or key-mismatched state fails
   closed and is never overwritten. Unknown fields, final symlinks/reparse points and non-regular record files are rejected; metadata and content come from one opened handle.
6. One preflight runs before setup, sync, or guided setup mutates any adapter. Source builds require
   per-command `--allow-source-build`; recorded downgrade and unknown state remain blocked.
7. Source overrides do not create recorded history. Dry-run performs no lifecycle read or write
   because it deploys no assets.
8. Local records are not authenticated against malicious same-account writes. No production installer
   emits them, and preflight is not atomic with adapter mutation; the later installer/upgrade slice
   must coordinate high-water publication with setup/sync.
9. Mapped-code identity, exact executing-byte authentication, arbitrary-code authenticity, binary replacement and mutating upgrade are absent.
