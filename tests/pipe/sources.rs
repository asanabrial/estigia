// The whole `pipe` suite as one body of text, for the checks that read its
// own source.

use std::path::PathBuf;

fn suite_paths() -> Vec<PathBuf> {
    let tests = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let directory = tests.join("pipe");
    let mut modules: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()))
        .map(|entry| entry.expect("a readable entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    modules.sort();
    let mut paths = vec![tests.join("pipe.rs")];
    paths.extend(modules);
    paths
}

/// Every line of this suite, read from the directory, for the checks that read
/// its own source.
pub(crate) fn suite_sources() -> String {
    suite_paths()
        .iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_suite_sources_hold_the_root_and_a_module_a_hand_list_would_omit() {
    let sources = suite_sources();
    // Split so that this file does not hold the text it looks for.
    let in_the_root = ["fn estigia", "_runner() -> "].concat();
    let in_a_module = ["fn publish_review_carries", "_the_payload_only_when_asked"].concat();
    assert!(
        sources.contains(&in_the_root),
        "tests/pipe.rs is not in what the suite sources yield"
    );
    assert!(
        sources.contains(&in_a_module),
        "tests/pipe/publication_lane.rs is not in what the suite sources yield"
    );
}
