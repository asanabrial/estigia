//! The release workflow, cross-checked against the installers.
//!
//! Both installers compose an asset name from a target triple and were written
//! before the workflow existed. Nothing but this file makes the two agree, and a
//! disagreement publishes assets nobody can fetch — a failure that reads as a
//! network problem rather than a naming one, from a script that is working
//! perfectly.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|why| panic!("{relative} ships with the crate: {why}"))
}

/// One of the installers, which live under `scripts/`.
///
/// The directory is named once, here. Every assertion below still reports the
/// bare `install.sh` / `install.ps1` an operator types, because that is the
/// name in the published one-liner and the name a failure has to be findable by.
fn script(name: &str) -> String {
    read(&format!("scripts/{name}"))
}

fn position(text: &str, needle: &str, meaning: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{meaning}: {needle:?}"))
}

/// The six the installers can ask for: every architecture they resolve, on
/// every operating system they resolve.
const TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

/// The triples the workflow's build matrix actually lists.
fn workflow_targets() -> Vec<String> {
    read(".github/workflows/release.yml")
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("- target:")
                .map(|target| target.trim().to_owned())
        })
        .collect()
}

/// The list below is the same list the workflow builds.
///
/// `TARGETS` is written here by hand and every check in this file iterates it,
/// so the two directions it claims to cover are both *from* this list outward. A
/// seventh triple added to the workflow is visited by neither: nothing asserts
/// an installer resolves it, and the release publishes an archive no script
/// knows how to ask for — which is the failure this file's own header describes,
/// arriving through the one door it left open.
///
/// So the hand-written list is crossed against the matrix rather than trusted:
/// same members, both ways.
#[test]
fn the_list_this_file_checks_is_the_list_the_workflow_builds() {
    let mut built = workflow_targets();
    built.sort();
    built.dedup();
    // A parse that found nothing would make every other check here vacuous and
    // this one pass by agreeing with an empty set.
    assert!(
        built.len() >= 4,
        "the matrix parsed to {} target(s), so this crossing has stopped reading it: {built:?}",
        built.len()
    );

    let mut listed: Vec<String> = TARGETS.iter().map(|target| (*target).to_owned()).collect();
    listed.sort();
    assert_eq!(
        built, listed,
        "the workflow builds one set of targets and this file checks another; whichever is only \
         in the matrix is published without an installer that can fetch it, and whichever is only \
         here is asserted about a build nobody runs"
    );
}

#[test]
fn the_workflow_builds_every_target_an_installer_can_ask_for() {
    let workflow = read(".github/workflows/release.yml");
    for target in TARGETS {
        assert!(
            workflow.contains(target),
            "{target} is a target an installer resolves and the workflow never builds; \
             that platform gets a download failure from a working script"
        );
    }
}

#[test]
fn the_installers_resolve_exactly_the_targets_the_workflow_builds() {
    // The other direction, which is the one that rots quietly: a target built
    // and published that no installer ever asks for is dead weight nobody
    // notices, and it hides the fact that the list was edited in one place.
    let shell = script("install.sh");
    let powershell = script("install.ps1");

    for target in TARGETS {
        let Some((architecture, system)) = target.split_once('-') else {
            panic!("{target:?} is not a triple, so nothing below is checking anything");
        };
        if system.contains("windows") {
            assert!(
                powershell.contains("-pc-windows-msvc"),
                "install.ps1 does not compose a windows triple"
            );
            assert!(
                powershell.contains(architecture),
                "install.ps1 never resolves {architecture}, so the workflow builds an \
                 archive it cannot ask for"
            );
        } else {
            assert!(shell.contains(system), "install.sh never resolves {system}");
            assert!(
                shell.contains(architecture),
                "install.sh never resolves {architecture}"
            );
        }
    }
}

#[test]
fn the_asset_names_are_the_ones_the_installers_compose() {
    // `estigia-<version>-<triple>` plus `.tar.gz` or `.zip`. Both halves are
    // read out of the scripts rather than written here, so this cannot agree
    // with a spelling the scripts stopped using.
    let shell = script("install.sh");
    let powershell = script("install.ps1");
    let workflow = read(".github/workflows/release.yml");

    assert!(
        shell.contains("estigia-$VERSION-$TARGET"),
        "install.sh no longer composes the name this checks"
    );
    assert!(
        powershell.contains("estigia-$version-$target"),
        "install.ps1 no longer composes the name this checks"
    );

    assert!(shell.contains(".tar.gz"), "install.sh stopped using tar.gz");
    assert!(powershell.contains(".zip"), "install.ps1 stopped using zip");
    assert!(
        workflow.contains("estigia-${version}-${{ matrix.target }}")
            || workflow.contains("estigia-$version-${{ matrix.target }}"),
        "the workflow packages under a name the installers do not compose"
    );
    assert!(
        workflow.contains(".tar.gz") && workflow.contains(".zip"),
        "the workflow does not produce both archive kinds"
    );
}

#[test]
fn a_partial_release_is_refused_rather_than_published() {
    // Four assets out of six is the worst outcome: two platforms get a
    // download failure from an installer that is working perfectly, and
    // nothing in the release says which two.
    let workflow = read(".github/workflows/release.yml");
    assert!(
        workflow.contains("-ne 6"),
        "nothing checks that all six archives are present before publishing"
    );
    assert!(
        workflow.contains("fail-fast: false"),
        "a failing matrix would report one broken platform and hide the rest"
    );
    assert!(
        workflow.contains("if-no-files-found: error"),
        "a build that packaged nothing would upload nothing and pass"
    );
}

#[test]
fn every_archive_ships_the_sum_the_installers_can_check() {
    let workflow = read(".github/workflows/release.yml");
    assert!(
        workflow.contains(".sha256"),
        "the archives ship without sums, leaving verification to whoever downloads them"
    );
    // Both halves of the matrix, because one platform's packaging step is a
    // different script from the other's.
    assert!(
        workflow.contains("shasum -a 256"),
        "no sum on the unix side"
    );
    assert!(
        workflow.contains("Algorithm SHA256"),
        "no sum on the windows side"
    );
}

#[test]
fn what_is_published_is_attested_and_what_is_installed_is_checked() {
    // An attestation nobody verifies is decoration. Both halves are asserted
    // together for that reason: producing provenance and never checking it is
    // the same as not having it, and checking for provenance that is never
    // produced would refuse every install.
    let workflow = read(".github/workflows/release.yml");
    assert!(
        workflow.contains("attest-build-provenance"),
        "the archives are published with nothing saying where they came from"
    );
    // Signed by the workflow run's own short-lived identity, not by a key
    // somebody holds — which is the whole difference between "this workflow
    // built it" and "somebody with the key said so".
    assert!(
        workflow.contains("id-token: write") && workflow.contains("attestations: write"),
        "the attestation step cannot sign without those permissions, and would fail at \
         the moment of a release rather than now"
    );
    // The subject is the archive people fetch, not the binary inside it.
    let Some(attest) = workflow.split("attest-build-provenance").nth(1) else {
        panic!("the attestation step is gone, so nothing below is checking anything");
    };
    assert!(
        attest.contains("dist/*.tar.gz") && attest.contains("dist/*.zip"),
        "the provenance does not cover what is actually downloaded"
    );

    for installer in ["install.sh", "install.ps1"] {
        let script = script(installer);
        assert!(
            script.contains("gh attestation verify"),
            "{installer} installs an archive without ever checking its provenance"
        );
        // A tool that is absent is not a failed check. Requiring the GitHub CLI
        // would make the installer refuse to work on the machines least likely
        // to have it, and a check that stops people installing gets removed
        // rather than fixed.
        assert!(
            script.contains("gh auth status"),
            "{installer} cannot tell a bad signature from a tool that could not ask"
        );
        // But when it IS here and says no, that is a stop.
        assert!(
            script.to_lowercase().contains("refusing to install"),
            "{installer} reports a failed provenance check without refusing"
        );
    }
}

#[test]
fn the_tag_and_the_tree_have_to_name_one_version() {
    // Estigia does not choose the version — `publish_version` is declared
    // `(agent, not scripted)`. What it does insist on is that the two places a
    // version appears agree. The archive is named from the tag and the binary
    // reports `Cargo.toml`; nothing else compares them, so a tag one minor
    // ahead ships `estigia-v0.2.0-*.tar.gz` containing something that calls
    // itself 0.1.0 — and every later "which version fixed this" inherits it.
    let workflow = read(".github/workflows/release.yml");
    assert!(
        workflow.contains("cargo metadata") || workflow.contains("cargo pkgid"),
        "nothing reads the version the tree declares, so the tag is taken on trust"
    );
    assert!(
        workflow.contains("GITHUB_REF_NAME#v"),
        "the tag is not stripped of its `v`, so it can never equal a Cargo version"
    );
    // On the build job, not the publish one: catching it after six matrix legs
    // have compiled wastes the run, and catching it after publishing is too
    // late by definition.
    let build = workflow
        .split("  publish:")
        .next()
        .unwrap_or_default()
        .to_owned();
    assert!(
        build.contains("cargo metadata") || build.contains("cargo pkgid"),
        "the check runs after the archives are built, when it is already wasted"
    );
}

#[test]
fn the_installers_reach_for_the_layout_the_workflow_builds() {
    // The names were crossed in both directions and the **inside** of the
    // archive was not. Both installers extract and then reach for
    // `<package>/estigia`, which is only right because the packaging steps
    // happen to `tar -C dist "<package>"` and `Compress-Archive -Path
    // dist/<package>` — a directory entry, not a bare binary.
    //
    // A disagreement there fails *after* the download and *after* the checksum
    // has already said the bytes are the published ones. It reads as a corrupt
    // release rather than as a path that was never right, which is the worst
    // place in the whole sequence to be wrong.
    let workflow = read(".github/workflows/release.yml");
    let shell = script("install.sh");
    let powershell = script("install.ps1");

    // The archive holds a directory named for the package, and the binary is
    // inside it.
    assert!(
        workflow.contains(r#"mkdir -p "dist/${package}""#)
            && workflow.contains(r#"tar -C dist -czf "dist/${package}.tar.gz" "${package}""#),
        "the tar no longer wraps the binary in a directory named for the package"
    );
    assert!(
        workflow.contains(r#"New-Item -ItemType Directory -Force -Path "dist/$package""#)
            && workflow.contains(r#"Compress-Archive -Path "dist/$package""#),
        "the zip no longer wraps the binary in a directory named for the package"
    );

    // And that is exactly where each installer looks.
    assert!(
        shell.contains(r#""$TEMP/$PACKAGE/estigia""#),
        "install.sh does not read the binary out of the package directory"
    );
    assert!(
        powershell.contains(r#""$temp\$package\estigia.exe""#),
        "install.ps1 does not read the binary out of the package directory"
    );

    // The binary the workflow copies in is the one the installer takes out —
    // named, because `estigia` and `estigia.exe` are not interchangeable and
    // the two halves are written in different shells.
    assert!(
        workflow.contains(r#"cp "target/${{ matrix.target }}/release/estigia" "dist/${package}/""#),
        "the unix package no longer contains a file called `estigia`"
    );
    assert!(
        workflow.contains(
            r#"Copy-Item "target/${{ matrix.target }}/release/estigia.exe" "dist/$package/""#
        ),
        "the windows package no longer contains a file called `estigia.exe`"
    );
}

#[test]
fn the_windows_installer_adds_to_a_path_without_damaging_it() {
    // The one script that edits something of the operator's that Estigia does
    // not own. `install.sh` only *tells* them what to add; this one writes their
    // user PATH, and two ways of doing that damage it.
    let script = script("install.ps1");

    // `[Environment]::SetEnvironmentVariable(..., 'User')` writes REG_SZ. A user
    // PATH holding a `%USERPROFILE%` entry — which is how several popular
    // installers write theirs — comes back frozen to whatever it expanded to
    // and stops following the variable.
    assert!(
        !script.contains("SetEnvironmentVariable('Path'"),
        "the installer writes the user PATH through the API that freezes its variables"
    );
    // And reading it back has the same trap one layer down: `Get-ItemProperty`
    // expands as it reads, so keeping the *kind* while writing an expanded
    // *value* leaves an ExpandString with nothing left to expand. Measured on a
    // throwaway key before this was written.
    assert!(
        script.contains("DoNotExpandEnvironmentNames"),
        "the installer reads the user PATH expanded, so writing it back freezes it"
    );
    // A fresh account has no user PATH, and joining onto nothing puts a
    // separator first. An empty entry in PATH means the current directory:
    // every command typed anywhere would look in that folder before anywhere
    // else.
    assert!(
        script.contains("IsNullOrEmpty"),
        "an account with no user PATH would be given one that starts with a separator"
    );
}

#[test]
fn installers_record_with_the_extracted_candidate_before_replacement() {
    let shell = script("install.sh");
    let shell_verify = position(&shell, "checksum mismatch", "shell verifies checksum");
    let shell_extract = position(&shell, "tar -xzf", "shell extracts the archive");
    let shell_record = position(
        &shell,
        "\"$CANDIDATE\" __record-install",
        "shell runs the extracted candidate recorder",
    );
    let shell_replace = position(
        &shell,
        "\"$INSTALL_DIR/estigia\"",
        "shell names the replacement destination",
    );
    assert!(
        shell_verify < shell_extract
            && shell_extract < shell_record
            && shell_record < shell_replace
    );

    let powershell = script("install.ps1");
    let ps_verify = position(
        &powershell,
        "checksum mismatch",
        "PowerShell verifies checksum",
    );
    let ps_extract = position(
        &powershell,
        "Expand-Archive",
        "PowerShell extracts the archive",
    );
    let ps_record = position(
        &powershell,
        "& $candidate '__record-install'",
        "PowerShell runs the extracted candidate recorder",
    );
    let ps_exit = ps_record
        + position(
            &powershell[ps_record..],
            "$LASTEXITCODE",
            "PowerShell checks the candidate native exit",
        );
    let ps_replace = position(
        &powershell,
        "Copy-Item $candidate",
        "PowerShell replaces from the admitted candidate",
    );
    assert!(
        ps_verify < ps_extract
            && ps_extract < ps_record
            && ps_record < ps_exit
            && ps_exit < ps_replace
    );

    for forbidden in ["--version", "--digest", "--asset", "--sha256"] {
        assert!(
            !shell[shell_record..shell_replace].contains(forbidden),
            "install.sh passes candidate identity through {forbidden}"
        );
        assert!(
            !powershell[ps_record..ps_replace].contains(forbidden),
            "install.ps1 passes candidate identity through {forbidden}"
        );
    }
}

#[test]
fn no_script_a_person_pipes_into_a_shell_carries_a_control_character() {
    // These two are fetched over the network and executed unread — `irm … | iex`
    // and `curl … | sh`. A stray control byte in one is invisible in every
    // review it will ever get, and it arrived here twice while the PATH fix
    // above was being written: a `\b` produced by an editing script, sitting
    // inside a comment, displayed as a missing letter.
    //
    // Harmless in a comment and not harmless in general — and *invisible* is the
    // property that matters, in the one file nobody reads before running it.
    for name in ["install.ps1", "install.sh"] {
        let raw = std::fs::read(root().join("scripts").join(name))
            .unwrap_or_else(|why| panic!("{name} ships with the crate: {why}"));
        let stray: Vec<(usize, u8)> = raw
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, byte)| *byte < 9 || (11..=12).contains(byte) || (14..=31).contains(byte))
            .collect();
        assert!(
            stray.is_empty(),
            "{name} carries {} control byte(s) nobody can see: {:?}",
            stray.len(),
            &stray[..stray.len().min(4)]
        );
    }
}

#[test]
fn the_checksum_an_installer_fetches_is_one_the_release_publishes() {
    // The targets were crossed between these three files and the file **names**
    // were not — so the workflow published one sum per archive,
    // `estigia-<version>-<triple>.tar.gz.sha256`, and both installers asked for
    // an aggregate `SHA256SUMS` that nothing has ever produced.
    //
    // Fail-closed, which is right and is why it would have gone out: the first
    // person to run the documented one-liner against the first real release gets
    // *no checksums published for <version>; refusing to install unverified*
    // from an installer that is working exactly as written, about a release that
    // is complete.
    let workflow = read(".github/workflows/release.yml");
    let published: Vec<&str> = ["*.tar.gz", "*.zip", "*.sha256"]
        .into_iter()
        .filter(|pattern| workflow.contains(pattern))
        .collect();
    // The floor: the release step was read, and it publishes archives and sums.
    assert_eq!(
        published.len(),
        3,
        "the workflow no longer publishes what this crossing is about: {published:?}"
    );
    assert!(
        !workflow.contains("SHA256SUMS"),
        "the workflow produces an aggregate listing now; the installers may fetch it"
    );

    for name in ["install.sh", "install.ps1"] {
        // What the script **does**, not whether a word appears in it: the
        // comments here explain the aggregate listing that is not fetched, and
        // an assertion on the whole text is one those comments fail. The same
        // shape as a pin satisfied by commented-out code, facing the other way.
        let script = script(name);
        let doing: String = script
            .lines()
            .map(str::trim_start)
            .filter(|line| !line.starts_with('#') && !line.starts_with("//"))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        assert!(
            !doing.contains("SHA256SUMS"),
            "{name} fetches an aggregate listing the release does not publish"
        );
        assert!(
            doing.contains(".sha256"),
            "{name} does not fetch the per-archive sum the release publishes"
        );
    }
}

#[test]
fn an_installer_looks_for_the_archive_the_workflow_writes() {
    // The last axis of this contract, and the one that would fail **after**
    // verifying: an installer that downloads the right bytes, checks them, and
    // then reaches into the archive for a path that is not in it.
    //
    // Three things have to line up, and none of them was crossed: the name of
    // the archive, the directory inside it, and the file inside that.
    let workflow = read(".github/workflows/release.yml");
    let sh = script("install.sh");
    let ps1 = script("install.ps1");

    // The name. Both build it from the same three parts in the same order, and
    // the version carries its `v` on both sides — the workflow takes the tag
    // whole, and the installers read it off the `releases/latest` redirect.
    assert!(
        workflow.contains(r#"package="estigia-${version}-${{ matrix.target }}""#),
        "the workflow builds the package name some other way now"
    );
    assert!(
        sh.contains(r#"PACKAGE="estigia-$VERSION-$TARGET""#),
        "install.sh no longer builds the name the workflow writes"
    );
    assert!(
        ps1.contains(r#"$package = "estigia-$version-$target""#),
        "install.ps1 no longer builds the name the workflow writes"
    );

    // The layout. `tar -C dist … "${package}"` and `Compress-Archive -Path
    // "dist/$package"` both put the binary one directory down, under the
    // package's own name — which is what the installers reach into.
    assert!(
        workflow.contains(r#"tar -C dist -czf "dist/${package}.tar.gz" "${package}""#),
        "the tar no longer holds the package directory the installer reaches into"
    );
    assert!(
        workflow.contains(r#"Compress-Archive -Path "dist/$package""#),
        "the zip no longer holds the package directory the installer reaches into"
    );
    assert!(
        sh.contains(r#""$TEMP/$PACKAGE/estigia""#),
        "install.sh reaches somewhere else in the archive"
    );
    assert!(
        ps1.contains(r#""$temp\$package\estigia.exe""#),
        "install.ps1 reaches somewhere else in the archive"
    );

    // And the binary is put there by the build. A copy that silently did not
    // happen is an archive with a directory and nothing in it.
    assert!(
        workflow.contains(r#"cp "target/${{ matrix.target }}/release/estigia" "dist/${package}/""#)
            && workflow.contains("Copy-Item \"target/${{ matrix.target }}/release/estigia.exe\""),
        "the build no longer puts the binary where the archive expects it"
    );
}

#[test]
fn no_workflow_needs_only_interpreters_it_sets_up() {
    // The reverse of what stood here. This asked both workflows to install
    // Python, because the differential suite spawned it and a runner without
    // one published **no release at all**, discovered on a tag.
    //
    // The suite does not spawn it any more and neither file may reach for it.
    // The check is kept rather than deleted because the failure it guards is
    // the same one in the other direction: a step that needs a tool the job
    // never sets up fails on a tag, which is the worst moment there is.
    for name in ["ci.yml", "release.yml"] {
        let workflow = read(&format!(".github/workflows/{name}"));
        let reaches: Vec<&str> = workflow
            .lines()
            .filter(|line| {
                let code = line.split('#').next().unwrap_or_default();
                code.contains("python") || code.contains("setup-python")
            })
            .collect();
        assert!(
            reaches.is_empty(),
            "{name} reaches for an interpreter this repository no longer uses: {reaches:?}"
        );
    }

    // And the direction the name now carries, which the old one only implied.
    // The suite gained an interpreter: `the_plugin_hands_the_gate_the_directory
    // _the_call_runs_in` executes the generated OpenCode plugin, which is
    // JavaScript, and it fails rather than skipping when `node` is absent. Both
    // files run `cargo test`, so both must set it up.
    //
    // Hosted runners ship Node today, which is exactly why this is worth
    // asserting: without the step both files pass on evidence that belongs to a
    // runner image rather than to the workflow, and the day it changes the
    // release lane discovers it on a tag. The guard that was kept for Python is
    // the same guard, and it had only ever named one tool.
    // Named for the test, not for the spawn. `Command::new("node")` was the
    // first spelling of this line and it held nothing: the same string already
    // stood in `the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer`,
    // which spawns `node` and **skips** when it is absent. Measured — deleting
    // the behavioural test outright left this guard green, so both workflows
    // would go on installing a runtime for a test that no longer needs one, and
    // the guard would be satisfied by the one test that never did.
    assert!(
        read("src/setup/tests.rs")
            .contains("fn the_plugin_hands_the_gate_the_directory_the_call_runs_in"),
        "the test that requires an interpreter is gone, so the steps below are \
         asking both workflows to install a tool nothing needs"
    );
    for name in ["ci.yml", "release.yml"] {
        let workflow = read(&format!(".github/workflows/{name}"));
        assert!(
            workflow.contains("cargo test"),
            "{name} has no test step, so nothing above was checked against a real workflow"
        );
        assert!(
            workflow.contains("actions/setup-node"),
            "{name} runs a suite that executes JavaScript and never installs a \
             runtime for it, so it passes only while the runner image happens to"
        );
    }

    // The floor: the guard is looking at real files with real steps in them.
    for name in ["ci.yml", "release.yml"] {
        let workflow = read(&format!(".github/workflows/{name}"));
        assert!(
            workflow.contains("cargo"),
            "{name} has no cargo step, so nothing above was checked against a real workflow"
        );
    }
}

/// Every check the contributing rules name is a check CI runs.
///
/// `AGENTS.md` tells the next person what to run before calling a change done,
/// and `ci.yml` is what actually refuses one. They are two spellings of one
/// list, and they had drifted: the rules named `cargo test`, `clippy` and
/// `fmt`, CI ran those three — and `cargo doc` was in neither, while it had been
/// **failing outright** on four public pages linking to private items. A crate
/// that keeps its reasoning in doc comments and cannot build them is one nobody
/// reads, and nothing anywhere said so.
///
/// Read out of both files rather than restated here, because a third copy of a
/// list is what this crate keeps finding disagreeing with itself.
#[test]
fn every_command_the_rules_tell_you_to_run_is_one_ci_runs() {
    let rules = read("AGENTS.md");
    let workflow = read(".github/workflows/ci.yml");
    let Some(at) = rules.find("**Run everything**") else {
        panic!("the contributing rules no longer say what to run");
    };
    // To the end of that list item, which is where the next numbered rule
    // starts. Taking the rest of the file would collect every command the
    // document mentions anywhere.
    let listed = &rules[at..];
    let listed = &listed[..listed.find("\n\n").unwrap_or(listed.len())];

    let mut named: Vec<&str> = Vec::new();
    for piece in listed.split('`') {
        let piece = piece.trim().trim_end_matches(&[',', '.'][..]);
        if let Some(rest) = piece.strip_prefix("cargo ") {
            // The verb only: the rules and CI pass different flags on purpose —
            // CI adds `--all-features` and `--check`, and demanding the exact
            // line would make this a test about spelling.
            named.push(rest.split_whitespace().next().unwrap_or(rest));
        }
    }
    named.sort_unstable();
    named.dedup();
    // The floor: the list was really read. An empty one would agree with
    // everything.
    assert!(
        named.len() >= 4,
        "only {} command(s) were read out of the contributing rules: {named:?}",
        named.len()
    );

    // The `run:` lines only. Searching the whole file was satisfied by the
    // **comment** beside the step that explains why it is there — a mention is
    // not a call, which is a defect this crate has already found once, in a
    // hook that merely printed the words `estigia hook pre-push`. Written into
    // the guard against it on the first attempt, and caught by turning the step
    // off and watching this pass.
    let ran: Vec<&str> = workflow
        .lines()
        .filter_map(|line| line.trim().strip_prefix("run: "))
        .collect();
    for verb in named {
        assert!(
            ran.iter()
                .any(|line| line.starts_with(&format!("cargo {verb}"))),
            "`AGENTS.md` says to run `cargo {verb}` and `ci.yml` never does: {ran:?}"
        );
    }
}

#[test]
fn pull_request_ci_starts_only_when_a_draft_is_released() {
    let workflow = read(".github/workflows/ci.yml");
    let compact: String = workflow.chars().filter(|c| !c.is_whitespace()).collect();

    assert!(
        compact.contains("push:branches:[main]")
            && compact.contains("pull_request:types:[ready_for_review]"),
        "CI is not limited to default-branch pushes and ready-for-review events:\n{workflow}"
    );
    for forbidden in ["opened", "synchronize", "reopened"] {
        assert!(
            !compact.contains(forbidden),
            "pull request CI still starts on {forbidden}:\n{workflow}"
        );
    }
}

#[test]
fn ci_uses_no_privileged_pr_context_or_write_permission() {
    let workflow = read(".github/workflows/ci.yml");
    assert!(!workflow.contains("pull_request_target"));
    assert!(!workflow.contains("write"));
    assert!(workflow.contains("contents: read"));
    assert!(workflow.contains("persist-credentials: false"));
}

/// Neither workflow checks out with an action GitHub is already forcing onto a
/// newer runtime, and a red run keeps what it compiled.
///
/// Both halves were unguarded, which is how both got old. `actions/checkout@v4`
/// targets Node 20 and every run of this repository printed *"Node.js 20 is
/// deprecated … being forced to run on Node.js 24"* — a warning that becomes a
/// workflow that will not start, on GitHub's schedule rather than on ours. And
/// `Swatinem/rust-cache` discards the cache of a failing run by default, so the
/// fix pushed after a red build recompiles the dependency tree from cold on
/// every platform. Six red runs closing one set of platform failures paid that
/// six times, three platforms each.
///
/// The floor is a version rather than a word, because "deprecated" is not
/// something a file can be asked. `v1` through `v4` are the majors that run on
/// Node 20 or older; anything at or above `v5` is on `node24`. A future
/// deprecation moves this number, and moving it is the point — the number is
/// where somebody has to look.
#[test]
fn no_workflow_checks_out_with_a_deprecated_action_or_discards_a_red_cache() {
    for name in ["ci.yml", "release.yml"] {
        let workflow = read(&format!(".github/workflows/{name}"));
        let stale: Vec<&str> = workflow
            .lines()
            .filter(|line| {
                let code = line.split('#').next().unwrap_or_default();
                ["@v1", "@v2", "@v3", "@v4"]
                    .iter()
                    .any(|old| code.contains(&format!("actions/checkout{old}")))
            })
            .collect();
        assert!(
            stale.is_empty(),
            "{name} checks out with an action GitHub already forces onto a newer \
             runtime: {stale:?}"
        );
        // The floor is only a floor if the step is still there to have a version.
        assert!(
            workflow.contains("actions/checkout@"),
            "{name} has no checkout step, so the version check above read nothing"
        );
    }

    // Only `ci.yml` caches — the release lane builds each target once, and the
    // issue that asked for this said not to add caching anywhere else.
    let ci = read(".github/workflows/ci.yml");
    assert!(
        ci.contains("cache-on-failure: true"),
        "a red CI run throws away everything it compiled, so the next push starts \
         from cold on all three platforms"
    );
    assert!(
        !read(".github/workflows/release.yml").contains("rust-cache"),
        "the release lane gained a cache this guard does not check"
    );
}
