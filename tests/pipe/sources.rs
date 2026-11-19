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
fn every_file_of_the_suite_is_in_what_the_suite_sources_yield() {
    let sources = suite_sources();
    let paths = suite_paths();
    assert!(
        paths.len() > 10,
        "the suite's directory was not read: {paths:?}"
    );
    for path in paths {
        let text = std::fs::read_to_string(&path).expect("a readable file");
        assert!(
            sources.contains(&text),
            "{} is not in what the suite sources yield",
            path.display()
        );
    }
}
