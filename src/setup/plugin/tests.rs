use super::*;

#[test]
fn a_hook_script_is_still_a_script_when_the_path_holds_an_apostrophe() {
    let awkward = Path::new("/home/o'brien/estigia");

    let posix = cline_hook(awkward, false);
    assert!(
        posix.contains(r"'\''"),
        "the apostrophe was not escaped for `sh`: {posix}"
    );
    let directory = tempfile::tempdir().expect("a temporary directory");
    let file = directory.path().join("PreToolUse.sh");
    std::fs::write(&file, &posix).expect("the script is written");
    let checked = std::process::Command::new("sh")
        .arg("-n")
        .arg(&file)
        .output()
        .expect("this test requires `sh` on the path to parse the hook script");
    assert!(
        checked.status.success(),
        "the hook is not a shell script: {}\n{posix}",
        String::from_utf8_lossy(&checked.stderr)
    );

    let windows = cline_hook(awkward, true);
    assert!(
        windows.contains("'/home/o''brien/estigia'"),
        "the apostrophe was not doubled for PowerShell: {windows}"
    );
    assert!(
        !windows.contains(r"'\''"),
        "PowerShell was handed a POSIX escape: {windows}"
    );

    // A gate that cannot run says so, in both languages: the script must
    // read the status, must not let a non-zero one reach the agent, and
    // must put the word `estigia` on standard error when it gets one.
    for windows in [false, true] {
        let script = cline_hook(Path::new("/usr/local/bin/estigia"), windows);
        assert!(
            !script.contains("exec "),
            "the status is inherited rather than read, so nothing can say it was not a \
                 decision: {script}"
        );
        assert!(
            script.contains("estigia: the gate did not answer"),
            "a gate that could not run passes the write without a word: {script}"
        );
        assert!(
            script.contains("exit 0") || script.contains("exit 0\n"),
            "a status that is not a decision is handed to the agent: {script}"
        );
    }

    let plain = cline_hook(Path::new("/usr/local/bin/estigia"), false);
    assert!(
        plain.contains("'/usr/local/bin/estigia' hook pre-tool-use"),
        "{plain}"
    );
}
