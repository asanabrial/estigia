# Installer lifecycle admission specification

## Acceptance criteria

1. A hidden argument-free candidate command derives executable pathname, observed pathname digest,
   compiled canonical SemVer, typed/count-framed asset digest, and current lifecycle root itself.
2. One operation reads high-water before publication, refuses a lower candidate, hashes the candidate,
   publishes provenance first, and then publishes release.
3. Exact replay is idempotent. Malformed/non-regular state and provenance conflict fail closed; failed
   provenance publication never advances release history.
4. Both official installers retain checksum and conditional attestation checks, extract the archive,
   invoke the extracted candidate, require its native success, and only then replace the destination.
   They do not invoke an installed binary or pass identity arguments.
5. Tests cover process behavior, hidden parser/help behavior, and both installer sequences.
6. The local records remain installer-recorded rather than authenticated. Publication and replacement
   are not atomic, concurrent installers are not serialized, and no remote update or rollback exists.
