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
/// every platform. Closing one set of platform failures paid that at least nine
/// times, three platforms each: `ci` on `main` failed nine consecutive times,
/// runs `31753883982` through `31760121174`, from late on 2026-08-13 into the
/// early hours of 2026-08-14, before `31760887100` went green. Nine is a floor
/// rather than the count — the run the issue names as the first of the episode,
/// `31752296629`, is no longer retrievable.
///
/// The floor is a version rather than a word, because "deprecated" is not
/// something a file can be asked. Read from each major's own `action.yml`: `v2`,
/// `v3` and `v4` declare `node12`, `node16` and `node20`; `v1` declares
/// `runs.plugin` and no Node runtime at all; `v5`, `v6` and `v7` are `node24`,
/// and there is no `v8`. So the four rejected are rejected on what they say
/// rather than on a guess, and a future deprecation moves this number — moving
/// it is the point, because the number is where somebody has to look.
///
/// The cache half is read from the block under `Swatinem/rust-cache@` rather
/// than from the file, because the first version of it searched the whole file
/// and three ways of turning the fix off left it green: the option commented out
/// above a `rust-cache` on its defaults, the option parked on another step, and
/// the caching step deleted. Each of those is now red, and so is a workflow with
/// no caching step at all — a floor only holds up something that is there. The
/// block ends where the next step's indentation begins rather than at the first
/// `- `, so a `cache-directories:` sequence written above the option does not
/// end the step early and fail a workflow that is configured correctly.
///
/// What it has been found not to catch is tabulated in `docs/honesty.md`, with
/// no count: this reads a workflow as lines, and nobody has enumerated where
/// that fails. Some of it is meaning a line cannot carry — a commit pin names no
/// version, a step under `if: false` reads exactly like one that runs — and some
/// is syntax this reader does not handle, which a YAML parser would close. The
/// thirteen correct workflows this guard used to refuse are tabulated beside it,
/// and both tables were built the same way: by writing the file a different
/// legal way and running it, rather than by reading this code and reasoning
/// about what it would do.
#[test]
fn no_workflow_checks_out_with_a_deprecated_action_or_discards_a_red_cache() {
    // The version floor applies to every workflow in the directory, not to the
    // two this repository happens to have today: a lane added later that checks
    // out is a lane this floor has to reach.
    let workflows = every_workflow();
    assert!(
        workflows.len() >= 2,
        "the workflow directory listed {workflows:?}, fewer than the two lanes \
         this repository is known to have, so the loop below reads almost nothing"
    );
    for name in &workflows {
        let running = what_runs(&read(&format!(".github/workflows/{name}")));
        let stale: Vec<&String> = running
            .iter()
            .filter(|code| {
                runs_action(code, "actions/checkout")
                    .and_then(checkout_major)
                    .is_some_and(|major| major < 5)
            })
            .collect();
        assert!(
            stale.is_empty(),
            "{name} checks out with an action GitHub already forces onto a newer \
             runtime: {stale:?}"
        );
    }
    // The floor is only a floor where a step is there to have a version, and
    // only these two lanes are known to check out. A scheduled labeller or stale
    // sweep needs no checkout, so requiring one of every file would fail a
    // workflow that is written correctly — which is what widening this loop to
    // the directory did until a review ran that file past it.
    for name in ["ci.yml", "release.yml"] {
        let running = what_runs(&read(&format!(".github/workflows/{name}")));
        assert!(
            running
                .iter()
                .any(|code| runs_action(code, "actions/checkout").is_some()),
            "{name} has no checkout step, so the version check above read nothing"
        );
    }

    assert!(
        what_runs(&read(".github/workflows/ci.yml"))
            .iter()
            .any(|code| runs_action(code, "swatinem/rust-cache").is_some()),
        "ci.yml has no caching step, so the option check below reads nothing"
    );
    // The option only does anything on the step it is written under. Read as a
    // whole file, a `cache-on-failure: true` parked on some other step — or
    // commented out beside a `rust-cache` left on its defaults — answers yes
    // while every red run still discards what it compiled. Every caching step in
    // every workflow is read: every one, because which of several decides the
    // saving behaviour is a question about the action rather than about this
    // file; every workflow, for the same reason the version floor reads them
    // all, since a lane added later can copy a bare `rust-cache` as easily as
    // this one carried it.
    for name in &workflows {
        let running = what_runs(&read(&format!(".github/workflows/{name}")));
        for at in 0..running.len() {
            // The same rule the two floors above use, so a step titled after the
            // action, or a command echoing its name, is not read as a caching
            // step and the step around it is not asked for the option.
            if runs_action(&running[at], "swatinem/rust-cache").is_none() {
                continue;
            }
            // From the line the step opens on, which is not always its `uses:`:
            // a step that names itself first puts `uses:` and `with:` at the
            // same depth, and measuring from there ends the block immediately.
            let opens = step_opening(&running, at);
            let depth = indent_of(&running[opens]);
            // A step ends where the next one starts. Stopping at the first `- `
            // instead would end it at any block sequence inside its own `with:`
            // — `cache-directories:` above the option would fail a workflow that
            // is configured correctly.
            let under_the_cache: String = running[opens + 1..]
                .iter()
                .take_while(|code| code.trim().is_empty() || indent_of(code) > depth)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            // Read as keys and values, for the reason the `uses:` half is:
            // searching the block for the text refused `cache-on-failure:  true`
            // over a second space, and the same eight lines apart.
            let settings: Vec<(String, String)> =
                under_the_cache.lines().filter_map(key_and_value).collect();
            let says = |key: &str, value: &str| {
                settings
                    .iter()
                    .any(|(k, v)| k == key && v.to_lowercase() == value)
            };
            assert!(
                says("cache-on-failure", "true"),
                "a red run throws away everything it compiled, so the next push \
                 starts from cold; the caching step at {name}:{} reads: \
                 {under_the_cache:?}",
                opens + 1
            );
            // `save-if: false` means, in the action's own words, that the cache
            // is only restored — no run saves one, which is the red-run case and
            // every other case besides.
            assert!(
                !says("save-if", "false"),
                "the caching step at {name}:{} saves on no run at all, so \
                 `cache-on-failure` decides nothing",
                opens + 1
            );
        }
    }
    // Only `ci.yml` caches, and that is this repository's judgement rather than
    // the issue's: the issue ruled out caching anything but cargo, which is what
    // `rust-cache` caches, and said nothing about the release lane. This line
    // holds that lane where it is so a cache appears there deliberately rather
    // than by copying — whether one would pay off has not been measured, and
    // measuring it is what should delete this line, not the copying. A step that
    // runs the action, for the reason the version floor reads only those: naming
    // it in a command or a title is talking about it.
    let cached: Vec<String> = what_runs(&read(".github/workflows/release.yml"))
        .into_iter()
        .filter(|code| runs_action(code, "swatinem/rust-cache").is_some())
        .collect();
    assert!(
        cached.is_empty(),
        "the release lane gained a cache this guard does not check: {cached:?}"
    );
}

/// What a workflow runs, line for line: comments stripped, and the body of every
/// block scalar blanked.
///
/// The indices match the file's, so a line number in a message still points at
/// the line. A `run: |` body is shell or text — a workflow that prints a YAML
/// recipe, or documents the step it replaced, has `- uses: actions/checkout@v4`
/// inside a string, and reading that as a step refuses a correct file. `ci.yml`
/// already writes bodies that shape.
fn what_runs(workflow: &str) -> Vec<String> {
    let mut running: Vec<String> = Vec::new();
    let mut body_under: Option<usize> = None;
    for line in workflow.lines() {
        let code = code_of(line);
        let blank = code.trim().is_empty();
        if let Some(depth) = body_under {
            if blank || indent_of(&code) > depth {
                running.push(String::new());
                continue;
            }
            body_under = None;
        }
        if !blank && opens_a_block(&code) {
            body_under = Some(indent_of(&code));
        }
        running.push(code);
    }
    running
}

/// How far a line is indented, in spaces.
fn indent_of(code: &str) -> usize {
    code.len() - code.trim_start().len()
}

/// Whether a line's value is a block scalar header — the whole value, not its
/// last character.
///
/// Read forwards from the `:` rather than by trimming the tail. Trimming took
/// the chomping indicator off before the indentation one, so `|2-` was a header
/// and `|-2` was not, though YAML accepts either order: half the header space
/// was asserted from the half that had been written down. And a value that
/// merely *ends* in a pipe — `run: cargo test | tee log` — is a command, not a
/// header, which trimming could not tell apart at all.
fn opens_a_block(code: &str) -> bool {
    let Some((_, value)) = code.split_once(':') else {
        return false;
    };
    let value = value.trim();
    let Some(indicators) = value.strip_prefix('|').or_else(|| value.strip_prefix('>')) else {
        return false;
    };
    indicators
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == '+')
}

/// The `@ref` of a step that runs `action`, lowercased — `None` for any line
/// that is not one.
///
/// Three things a plain substring search got wrong, each found by writing a
/// legal workflow and running it. The **key** must be `uses:`, so a step title,
/// an `if:` expression or a `with:` value that quotes the word beside a version
/// is not a step. The value must **start** with the action, so
/// `myorg/tools/actions/checkout@v4` — a different action whose path ends in
/// this one's name — is not this one. And the comparison is case-folded,
/// because `Actions/Checkout@v4` slipped both floors while naming the same
/// action a floor exists to refuse.
fn runs_action(code: &str, action: &str) -> Option<String> {
    let (key, value) = key_and_value(code)?;
    if key != "uses" {
        return None;
    }
    let value = value.to_lowercase();
    value
        .strip_prefix(&format!("{action}@"))
        .map(|reference| reference.to_owned())
}

/// A line's key and value, split at the first `:`, with a step's `- ` removed,
/// the key's own spacing dropped and the value unquoted.
///
/// Reading the key is what tells a step from a sentence about one: `name: this
/// uses: actions/checkout@v4` has the key `name`, and a guard that searches the
/// whole line for `uses:` refuses it. Unquoting matters for the same reason
/// case-folding did: `uses: "actions/checkout@v7"` is ordinary YAML, and a
/// comparison against the raw value read it as no step at all — which both
/// refused a correct workflow and let a quoted `@v4` through a floor. This
/// document has recorded a guard defeated by a leading quote twice before.
fn key_and_value(code: &str) -> Option<(String, String)> {
    let (key, value) = code.split_once(':')?;
    let key = key.trim().trim_start_matches("- ").trim();
    let value = value.trim();
    let value = match value.chars().next() {
        Some(quote @ ('"' | '\'')) if value.len() > 1 && value.ends_with(quote) => {
            &value[1..value.len() - 1]
        }
        _ => value,
    };
    Some((key.to_owned(), value.to_owned()))
}

/// The `actions/checkout` major a `@ref` names, when it names one by tag.
///
/// Parsed rather than matched against a list of old spellings: `@v1` is a prefix
/// of `@v10`, so a list would start refusing the first two-digit major — and
/// surviving to the next deprecation is the whole point of a floor. A commit pin
/// carries no major and answers `None`, which is the hole `docs/honesty.md`
/// records.
fn checkout_major(reference: String) -> Option<u32> {
    reference
        .strip_prefix('v')?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

/// The line a step opens on, walking back from any line inside it.
///
/// A step's first line is the one carrying the `- `, which is not always the
/// `uses:`: `- name: …` then `uses:` then `with:` is the form this repository
/// writes every step it names, and there the `uses:` and the `with:` sit at the
/// same depth. Reading a step's extent from the `uses:` line ends it at the very
/// next line, and the guard above then reports a cache being discarded by a
/// workflow that is configured correctly.
fn step_opening(running: &[String], at: usize) -> usize {
    (0..=at)
        .rev()
        .find(|&n| {
            let opening = running[n].trim_start();
            opening == "-" || opening.starts_with("- ")
        })
        .unwrap_or(at)
}

/// Every workflow file the directory holds, sorted, so that "both workflows"
/// keeps meaning all of them rather than the two that existed when it was
/// written. A directory named `x.yml` is listed by name and is not a workflow.
fn every_workflow() -> Vec<String> {
    let dir = root().join(".github/workflows");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|why| panic!("{} holds the workflows: {why}", dir.display()))
        .map(|entry| entry.unwrap_or_else(|why| panic!("the directory listed it: {why}")))
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".yml") || name.ends_with(".yaml"))
        .collect();
    names.sort();
    names
}

/// Everything on a line before its first `#`, so a guard reads what a workflow
/// runs rather than what it says about itself. Commenting a setting out is how
/// one gets turned off without looking turned off.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}
