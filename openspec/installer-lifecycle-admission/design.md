# Installer lifecycle admission design

## Candidate-owned identity

`__record-install` is hidden from help and accepts no identity fields. It resolves the process
executable and current lifecycle root, then delegates to `StateRoot::record_installer_install`. Global
JSON remains accepted because it is a printing command, but installers need only its exit status.

## Publication order

The orchestration parses canonical compiled SemVer, reads every release record to derive high-water,
and refuses downgrade before hashing or publishing candidate evidence. It hashes the supplied candidate
pathname, constructs provenance from compiled identity, publishes that immutable record, then publishes
the release record. Equal immutable records make exact replay safe. Existing strict single-handle reads
and create-once publication are defined by the related
[secure lifecycle design](../secure-binary-lifecycle/design.md).

## Installer boundary

Checksum verification and optional GitHub attestation retain their existing meanings. After extraction,
the candidate command is the admission decision; a nonzero native exit stops before destination copy.
This ordering is fail-closed but not transactional: release history may advance before a later copy
failure, and two installer processes can interleave because no lifecycle lock is introduced.
