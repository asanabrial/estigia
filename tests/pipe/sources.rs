// The whole `pipe` suite as one body of text, for the checks that read its
// own source. The root is included by relative path because it cannot include
// itself.

/// Every line of this suite, for the checks that read its own source.
pub(crate) const SUITE_SOURCES: &str = concat!(
    include_str!("../pipe.rs"),
    include_str!("cli_process.rs"),
    include_str!("config.rs"),
    include_str!("gate_hook.rs"),
    include_str!("part_04.rs"),
    include_str!("part_05.rs"),
    include_str!("part_06.rs"),
    include_str!("part_07.rs"),
    include_str!("part_08.rs"),
    include_str!("part_09.rs"),
    include_str!("part_10.rs"),
    include_str!("release.rs"),
    include_str!("status_doctor.rs"),
    include_str!("sync.rs"),
);
