fn write_repository_rows(file: &Path, rows: &str) {
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the directory");
    fs::write(
        file,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n{rows}{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the repository layer is written");
}

fn repository_with_rows(rows: &str) -> (tempfile::TempDir, PathBuf) {
    let repository = tempfile::tempdir().expect("a repository");
    std::fs::create_dir_all(repository.path().join(".git")).expect("a checkout");
    write_repository_rows(&repository_config_path(repository.path()), rows);
    let path = repository.path().to_path_buf();
    (repository, path)
}

fn asking_agent(root: &Path) {
    let mut agent = Config::default();
    crate::config::Setting::Delivery
        .apply(&mut agent, "ask")
        .expect("ask is accepted");
    crate::setup::write_agent_configuration_wholly(
        &agent_override(root, "opencode"),
        "opencode",
        &agent,
    )
    .expect("the agent layer is written");
}

#[test]
fn a_repair_ceiling_row_in_the_local_or_agent_file_is_reported_and_changes_nothing() {
    let rows = "| Setting | Value |\n|---|---|\n| Repair ceiling | 3 |\n";
    for file in [
        crate::config::LOCAL_FILE.to_owned(),
        "estigia.opencode.md".to_owned(),
    ] {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("the contract installs");
        std::fs::write(skill.join(&file), rows).expect("the override is written");

        let (layers, report) =
            config_layers_for_reporting(&skill, Some("opencode"), None).expect("the layers read");
        assert_eq!(layers.effective(), &Config::default(), "{file}");
        assert_eq!(
            report
                .unwrap_or_else(|| panic!("{file}: the retired row was dropped silently"))
                .code,
            "repair-ceiling-retired",
            "{file}"
        );
    }
}

#[test]
fn a_repository_files_repair_ceiling_row_is_reported_and_changes_nothing() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    let (_repository, path) = repository_with_rows("| Repair ceiling | 5 | 3 |\n");

    let layers = config_layers(&skill, Some("opencode"), Some(&path)).expect("the layers read");
    assert_eq!(layers.effective(), &Config::default());
    let (_, report) = config_layers_for_reporting(&skill, Some("opencode"), Some(&path))
        .expect("the layers read");
    assert_eq!(
        report.expect("the retired repository row is reported").code,
        "repair-ceiling-retired"
    );
}

#[test]
fn a_hand_written_auto_does_not_loosen_an_agents_ask() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    asking_agent(&skill);
    let (_repository, path) = repository_with_rows("| Delivery authorisation | auto | auto |\n");

    let layers = config_layers(&skill, Some("opencode"), Some(&path)).expect("the layers read");

    assert!(
        !layers.effective().delivery.is_autonomous(),
        "a repository row loosened the agent's `ask`"
    );
    assert!(
        !layers
            .repository_settings
            .contains(&crate::config::Setting::Delivery),
        "the ignored row was reported as one the repository answers for"
    );
    let (_, report) = config_layers_for_reporting(&skill, Some("opencode"), Some(&path))
        .expect("the layers read");
    let report = report.expect("the row that was ignored is not reported");
    assert_eq!(report.code, crate::config::CANNOT_LOOSEN);
    assert!(
        report.message.contains("Delivery authorisation"),
        "{report}"
    );
}

#[test]
fn a_linked_worktree_reads_the_authorisation_rows_of_its_checkout() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    let (_repository, path) = repository_with_rows("| Transition authorisation | ask | auto |\n");
    let outside = tempfile::tempdir().expect("somewhere for the worktree");
    let linked = path.join(".git").join("worktrees").join("wt");
    let worktree = outside.path().join("wt");
    std::fs::create_dir_all(&linked).expect("the linked worktree's git directory");
    std::fs::create_dir_all(&worktree).expect("the worktree");
    std::fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", linked.display()),
    )
    .expect("the pointer git writes in a linked worktree");
    std::fs::write(linked.join("commondir"), "../..\n").expect("the shared directory");

    let layers = config_layers(&skill, None, Some(&worktree)).expect("the layers read");

    assert!(!layers.effective().transitions.is_autonomous());
}

/// The contract hands the agent a route that resolves in a linked worktree:
/// its `.git` is a **file**, and the binary never enforces the three rows.
#[test]
fn a_linked_worktree_is_handed_a_route_to_its_repository_file_that_resolves() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    let (_repository, path) = repository_with_rows("| Delivery authorisation | ask | auto |\n");
    let outside = tempfile::tempdir().expect("somewhere for the worktree");
    let linked = path.join(".git").join("worktrees").join("wt");
    let worktree = outside.path().join("wt");
    std::fs::create_dir_all(&linked).expect("the linked worktree's git directory");
    std::fs::create_dir_all(&worktree).expect("the worktree");
    std::fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", linked.display()),
    )
    .expect("the pointer git writes in a linked worktree");
    std::fs::write(linked.join("commondir"), "../..\n").expect("the shared directory");

    let routed = super::git_common_dir(&worktree)
        .join("estigia")
        .join(crate::config::LOCAL_FILE);
    assert_eq!(
        routed,
        super::repository_config_path(&worktree),
        "the route the contract names and the file the binary reads are not the same place"
    );
    assert!(
        routed.starts_with(&path),
        "the worktree route left the checkout that holds its answers: {}",
        routed.display()
    );
    let layers = config_layers(&skill, None, Some(&worktree)).expect("the layers read");
    assert!(
        !layers.effective().delivery.is_autonomous(),
        "the file the route names does not hold the ask the route is meant to reach"
    );

    let body = configuration_body(&Config::default());
    assert!(
        body.contains("`estigia config list`, run from the repository"),
        "the contract does not hand the agent a read of the repository's rows:\n{body}"
    );
    assert!(
        !body.contains("$("),
        "the contract routes the agent through a shell substitution the gate cannot place:\n{body}"
    );
    assert!(
        !body.contains(".git/estigia/estigia.local.md"),
        "the contract names a literal path that does not exist in a linked worktree:\n{body}"
    );
}

/// No text the agent obeys names the literal `.git/estigia/` path.
#[test]
fn no_text_the_agent_obeys_names_a_literal_git_estigia_path() {
    let body = configuration_body(&Config::default());
    for (which, text) in [
        ("the rendered block", body.as_str()),
        ("the shipped contract", CONTRACT_CONTENTS),
    ] {
        assert!(
            !text.contains(".git/estigia/"),
            "{which} names a literal `.git/estigia/` path, which is a file in a linked \
                 worktree:\n{text}"
        );
    }
}

/// A worktree answers with the repository it belongs to.
///
/// The layout is built by hand rather than by git, so this measures on
/// every machine and not only where git is installed —
/// `a_worktree_answers_what_its_repository_answers` in `tests/pipe/config.rs`
/// crosses the same thing against a real `git worktree add`.
#[test]
fn a_linked_worktree_keeps_its_answers_with_the_repository() {
    let outside = tempfile::tempdir().expect("somewhere to build a checkout");
    let repo = outside.path().join("main");
    let git_dir = repo.join(".git");
    let worktree = outside.path().join("wt");
    let linked = git_dir.join("worktrees").join("wt");
    std::fs::create_dir_all(&linked).expect("the linked worktree's git directory");
    std::fs::create_dir_all(&worktree).expect("the worktree");
    std::fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", linked.display()),
    )
    .expect("the pointer git writes in a linked worktree");
    // Relative, as git writes it, and relative to the directory holding it.
    std::fs::write(linked.join("commondir"), "../..\n").expect("the pointer to the shared one");

    let from_worktree = super::repository_config_path(&worktree);
    // Where it is, said in a way somebody can follow: joining the pointers
    // as git writes them gives `…/worktrees/wt/../../estigia/…`, which is
    // the answer `config set --repo` prints when it says what it wrote.
    assert!(
        !from_worktree
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir)),
        "the path an operator is shown steps back out of itself: {}",
        from_worktree.display()
    );
    assert_eq!(
        std::fs::canonicalize(from_worktree.parent().expect("a parent")).ok(),
        std::fs::canonicalize(
            super::repository_config_path(&repo)
                .parent()
                .expect("a parent")
        )
        .ok(),
        "a worktree looks for its answers somewhere the checkout does not keep them, so the \
             same repository answers two different ways depending on which directory you are in"
    );

    // And a submodule, whose `.git` is the same kind of file with no
    // `commondir` beside it, answers for itself rather than for its parent.
    let parent = outside.path().join("parent");
    let module = parent.join(".git").join("modules").join("child");
    let child = parent.join("child");
    std::fs::create_dir_all(&module).expect("the submodule's git directory");
    std::fs::create_dir_all(&child).expect("the submodule");
    std::fs::write(
        child.join(".git"),
        format!("gitdir: {}\n", module.display()),
    )
    .expect("the submodule's pointer");
    assert!(
        super::repository_config_path(&child).starts_with(&module),
        "a submodule was made to answer with its parent's rows: {}",
        super::repository_config_path(&child).display()
    );

    // The floor: an ordinary checkout is unmoved, or every file written
    // before this existed would have been left behind.
    std::fs::create_dir_all(&git_dir).expect("an ordinary git directory");
    assert!(
        super::repository_config_path(&repo).starts_with(&git_dir),
        "an ordinary checkout no longer keeps its answers under `.git`"
    );
}

fn linked_worktree_of(checkout: &Path, outside: &Path) -> PathBuf {
    let linked = checkout.join(".git").join("worktrees").join("wt");
    let worktree = outside.join("wt");
    std::fs::create_dir_all(&linked).expect("the linked worktree's git directory");
    std::fs::create_dir_all(&worktree).expect("the worktree");
    std::fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", linked.display()),
    )
    .expect("the pointer git writes in a linked worktree");
    std::fs::write(linked.join("commondir"), "../..\n").expect("the shared directory");
    worktree
}

#[test]
fn a_subdirectory_of_a_checkout_reads_the_checkouts_rows() {
    let (_repository, path) = repository_with_rows("| Integration | trunk | branch |\n");
    let deep = path.join("apps").join("engine");
    std::fs::create_dir_all(&deep).expect("a subdirectory");

    let (found, text) = repository_rows(&deep)
        .expect("the rows read")
        .expect("the subdirectory found no repository file");

    assert_eq!(found, repository_config_path(&path));
    assert!(text.contains("Integration"));
}

#[test]
fn a_subdirectory_of_a_linked_worktree_reads_the_checkouts_rows() {
    let (_repository, path) = repository_with_rows("| Integration | trunk | branch |\n");
    let outside = tempfile::tempdir().expect("somewhere for the worktree");
    let worktree = linked_worktree_of(&path, outside.path());
    let deep = worktree.join("apps").join("engine");
    std::fs::create_dir_all(&deep).expect("a subdirectory of the worktree");

    let (found, _) = repository_rows(&deep)
        .expect("the rows read")
        .expect("the worktree subdirectory found no repository file");

    assert_eq!(found, repository_config_path(&path));
}

#[test]
fn a_directory_outside_every_repository_has_no_repository_layer() {
    let outside = tempfile::tempdir().expect("a directory with no checkout above it");
    let deep = outside.path().join("a").join("b");
    std::fs::create_dir_all(&deep).expect("a nested directory");

    assert!(repository_rows(&deep).expect("the rows read").is_none());
    assert_eq!(
        repository_config_path(&deep),
        deep.join(".git")
            .join("estigia")
            .join(crate::config::LOCAL_FILE)
    );
}

#[test]
fn a_subdirectory_of_a_submodule_reads_the_submodules_rows() {
    let outside = tempfile::tempdir().expect("somewhere to build a checkout");
    let parent = outside.path().join("parent");
    let module = parent.join(".git").join("modules").join("child");
    let child = parent.join("child");
    std::fs::create_dir_all(&module).expect("the submodule's git directory");
    std::fs::create_dir_all(child.join("src")).expect("the submodule");
    std::fs::write(
        child.join(".git"),
        format!("gitdir: {}\n", module.display()),
    )
    .expect("the submodule's pointer");
    write_repository_rows(
        &repository_config_path(&parent),
        "| Integration | trunk | branch |\n",
    );
    write_repository_rows(
        &module.join("estigia").join(crate::config::LOCAL_FILE),
        "| Integration | branch | branch |\n",
    );

    let (found, text) = repository_rows(&child.join("src"))
        .expect("the rows read")
        .expect("the submodule subdirectory found no repository file");

    assert!(found.starts_with(&module), "{}", found.display());
    assert!(
        text.contains("| Integration | branch |"),
        "the parent's rows were read for the submodule:\n{text}"
    );
}

#[test]
fn a_nested_git_entry_that_is_not_a_checkout_does_not_hide_the_outer_checkout() {
    let broken = [
        "a file that does not say gitdir",
        "an empty file",
        "a gitdir line that names nothing",
        "a dangling symlink",
    ];
    for which in broken {
        let (_repository, path) = repository_with_rows("| Integration | trunk | branch |\n");
        let nested = path.join("vendor").join("lib");
        std::fs::create_dir_all(nested.join("src")).expect("a nested directory");
        let at = nested.join(".git");
        match which {
            "a file that does not say gitdir" => {
                std::fs::write(&at, "not a gitdir pointer").expect("the garbage marker");
            }
            "an empty file" => std::fs::write(&at, "").expect("the empty marker"),
            "a gitdir line that names nothing" => {
                std::fs::write(&at, "gitdir:\n").expect("the empty pointer");
            }
            _ => {
                #[cfg(unix)]
                std::os::unix::fs::symlink(nested.join("gone"), &at).expect("the dangling symlink");
                #[cfg(not(unix))]
                std::fs::write(&at, "").expect("the stand-in marker");
            }
        }

        assert_eq!(
            checkout_root(&nested.join("src")),
            Some(path.clone()),
            "{which} was taken for a checkout"
        );
        let (found, text) = repository_rows(&nested.join("src"))
            .expect("the rows read")
            .unwrap_or_else(|| panic!("{which} hid the outer checkout's repository file"));
        assert_eq!(found, repository_config_path(&path), "{which}");
        assert!(text.contains("Integration"), "{which}");
    }
}

#[test]
fn a_nested_git_entry_that_is_not_a_checkout_leaves_no_checkout_when_none_is_above() {
    let outside = tempfile::tempdir().expect("a directory with no checkout above it");
    let nested = outside.path().join("lib");
    std::fs::create_dir_all(nested.join("src")).expect("a nested directory");
    std::fs::write(nested.join(".git"), "not a gitdir pointer").expect("the garbage marker");

    assert_eq!(checkout_root(&nested.join("src")), None);
    assert!(
        repository_rows(&nested.join("src"))
            .expect("the rows read")
            .is_none()
    );
}

/// Registering one checkout does not forget another that is merely away.
///
/// The list is pruned as it is *read*, so an unreachable checkout is not
/// offered — that part is right, and somebody who picked one would get the
/// contract's answers believing they were that repository's. Rewriting the
/// file from the pruned list is a different act: it drops the checkout
/// permanently, and plugging the drive back in does not bring it back.
///
/// Taking one off the list is `config forget`, which does it by removing
/// that checkout's own file. Nothing else may decide it.
#[test]
fn recording_a_checkout_does_not_forget_one_that_is_only_away() {
    let home = tempfile::tempdir().expect("a temporary home");
    let outside = tempfile::tempdir().expect("somewhere to keep checkouts");
    let make = |name: &str| {
        let dir = outside.path().join(name);
        let file = repository_config_path(&dir);
        fs::create_dir_all(file.parent().expect("that file has a directory"))
            .expect("the checkout is made");
        fs::write(
            &file,
            "| Setting | Value here | Skill default |
",
        )
        .expect("its rows are written");
        dir
    };

    let away = make("away");
    remember_repository(home.path(), &away);
    // Gone from where it was — a drive unplugged, a share down. Not deleted.
    let moved = outside.path().join("away-elsewhere");
    fs::rename(&away, &moved).expect("the checkout is moved out of reach");
    assert!(
        !known_repositories(home.path()).contains(&away),
        "an unreachable checkout is being offered, which is the other failure"
    );

    // Registering something else, which is the act that used to drop it.
    remember_repository(home.path(), &make("here"));

    fs::rename(&moved, &away).expect("the checkout comes back");
    assert!(
        known_repositories(home.path()).contains(&away),
        "a checkout that was only away was forgotten by registering another"
    );
}

use super::*;

#[test]
fn a_skill_that_was_never_installed_is_absent_rather_than_out_of_date() {
    // The distinction `status` prints. Both send an operator to the same
    // command, so this was never a dead end — but a line that says a file is
    // out of date, about a directory with no files in it, is describing a
    // world that is not there.
    let root = tempfile::tempdir().expect("a temporary directory");
    let config = Config::default();
    assert_eq!(
        presence(&root.path().join("issue-flow"), &config),
        Presence::Absent
    );

    install(root.path(), &config, false).expect("the skill installs");
    assert_eq!(presence(root.path(), &config), Presence::Current);

    let first = root.path().join(relative(FILES[0].path));
    std::fs::write(&first, "something older").expect("age one file");
    assert_eq!(presence(root.path(), &config), Presence::Stale);

    // And one file left behind is still not a fresh machine: an operator
    // whose uninstall was interrupted must not read `not configured`.
    for file in &FILES[1..] {
        let _ = std::fs::remove_file(root.path().join(relative(file.path)));
    }
    assert_eq!(presence(root.path(), &config), Presence::Stale);
}
#[test]
fn the_installed_directory_matches_the_declared_name() {
    assert_eq!(
        declared_name(),
        Some(DIRECTORY),
        "the skill directory and the frontmatter name must agree"
    );
}

#[test]
fn the_skill_declares_a_version() {
    assert!(
        version().is_some(),
        "SKILL.md frontmatter must carry a version"
    );
}

#[test]
fn the_contract_still_names_the_operations_it_requires() {
    // The guard on the guard: if `required_operations` parsed nothing, the
    // seam test below would check nothing. The count is deliberately not
    // pinned, so adding an operation the contract requires does not mean
    // retyping a number here.
    let required = super::required_operations();
    assert!(
        !required.is_empty(),
        "the contract's MUST-map line parsed to no operations"
    );
    assert!(required.contains(&"verify_claim".to_owned()));
    assert!(required.contains(&"review_disposition".to_owned()));
    assert!(required.contains(&"reclaim".to_owned()));
}

#[test]
fn every_binding_maps_every_operation_the_contract_requires() {
    // Seam test: the contract against each binding that ships. An operation
    // the contract demands and no binding provides is a run that discovers
    // it mid-delivery.
    let required = super::required_operations();
    let mut gaps = Vec::new();
    // A binding is a direct child of `bindings/`; a companion nested one
    // level deeper (`bindings/github/*.md`) is routed prose for the
    // binding beside it, not a second implementation of the contract, and
    // must not be asked to map anything on its own.
    for binding in FILES.iter().filter(|f| {
        f.path
            .strip_prefix("bindings/")
            .is_some_and(|rest| !rest.contains('/'))
    }) {
        for operation in &required {
            if !binding.contents.contains(&format!("`{operation}`")) {
                gaps.push((binding.path, operation.clone()));
            }
        }
    }

    assert!(
        gaps.is_empty(),
        "a binding does not map an operation the contract requires: {gaps:?}. Map it, or \
             declare it unsupported."
    );
}

#[test]
fn every_file_the_payload_links_to_is_shipped() {
    // Seam test, the direction that bites at runtime: a reference the agent
    // is told to load and that is not installed is a dead end, and nothing
    // else would catch it.
    for file in FILES {
        for link in links_from(file) {
            assert!(
                FILES.iter().any(|shipped| shipped.path == link),
                "{} links to {link}, which is not installed",
                file.path
            );
        }
    }
}

/// One file's links, resolved against its own directory.
///
/// `references/repository-delivery.md` links to `../SKILL.md#hard-rules`;
/// taken literally that is a path nothing ships, which is a false alarm the
/// guard cannot afford — a seam test people learn to ignore is a seam test
/// that has stopped working.
fn links_from(file: &SkillFile) -> Vec<String> {
    let directory = Path::new(file.path).parent().unwrap_or(Path::new(""));
    markdown_link_targets(file.contents)
        .into_iter()
        .map(|target| {
            let target = target.split('#').next().unwrap_or_default();
            normalize_relative(directory, target)
        })
        .filter(|target| target.ends_with(".md"))
        .collect()
}

/// Joins a link to the directory it was written in, collapsing `..`.
fn normalize_relative(directory: &Path, target: &str) -> String {
    let mut segments = directory
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other.to_owned()),
        }
    }
    segments.join("/")
}

#[test]
fn every_file_the_payload_tells_an_agent_to_execute_is_shipped() {
    // The guard that was missing, and the reason it was missing is worth
    // keeping: `every_file_the_payload_links_to_is_shipped` reads markdown
    // links, and `bindings/github.md` does not *link* to its executable —
    // it prints the command line in a fence:
    //
    //     python <skill>/scripts/github.py --repo-dir <repo> <operation>
    //
    // A binding whose every operation runs through a file the installer
    // does not write is a dead end at the first tracker call, and the link
    // guard sails straight past it.
    for file in FILES {
        for named in executables_named(file.contents) {
            // A **retired** path is not a dead end: the prose that names one
            // is explaining what went away and why, which is worth keeping.
            // What must not survive is prose that still tells an agent to
            // run something nobody installs.
            if RETIRED.contains(&named.as_str()) {
                continue;
            }
            assert!(
                FILES.iter().any(|shipped| shipped.path == named),
                "{} tells an agent to run {named}, which is not installed",
                file.path
            );
        }
    }
}

/// Every `<skill>/…`-rooted path the payload hands to an interpreter.
///
/// Deliberately narrow: it matches the `<skill>/` prefix the bindings use to
/// mean "inside the installed directory", which is the only form that can
/// name something the installer is responsible for. A bare `python foo.py`
/// is the operator's own file and none of Estigia's business.
fn executables_named(document: &str) -> Vec<String> {
    let mut named = Vec::new();
    let mut rest = document;
    while let Some(position) = rest.find("<skill>/") {
        rest = &rest[position + "<skill>/".len()..];
        let path = rest
            .split(|c: char| c.is_whitespace() || c == '`' || c == '"')
            .next()
            .unwrap_or_default();
        if !path.is_empty() {
            named.push(path.to_owned());
        }
    }
    named.sort_unstable();
    named.dedup();
    named
}

/// The `(path)` half of every markdown link in a document.
fn markdown_link_targets(document: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = document;
    while let Some(open) = rest.find("](") {
        rest = &rest[open + 2..];
        let Some(close) = rest.find(')') else { break };
        let target = rest[..close].trim();
        if !target.is_empty() && !target.contains("://") {
            targets.push(target.to_owned());
        }
        rest = &rest[close + 1..];
    }
    targets
}

#[test]
fn configuration_block_is_the_only_transformation() {
    let config = Config::default();
    for file in FILES {
        if file.path == CONTRACT {
            continue;
        }
        assert_eq!(
            rendered(file, &config),
            file.contents,
            "{} must be installed verbatim",
            file.path
        );
    }
}

#[test]
fn the_installed_contract_names_the_override_file_estigia_actually_reads() {
    // Seam test: what `installed_config` opens against what the contract
    // tells the agent to open. issue-flow's block named `operator.local.md`;
    // Estigia reads `estigia.local.md` first. A contract that names the old
    // one, or names none at all, sends the agent to the wrong file — and
    // the agent has no other way to learn the mechanism exists.
    let rendered = rendered(&FILES[0], &Config::default());
    assert!(rendered.contains(crate::config::LOCAL_FILE));
    assert!(rendered.contains(crate::config::LEGACY_LOCAL_FILE));
}

#[test]
fn one_row_the_gate_cannot_read_does_not_cost_it_the_rows_beside_it() {
    // `Config::read` applied each setting with `?`, so the first value it did
    // not recognise discarded the whole document — every row beside it
    // included, however well they read.
    //
    // That collateral is what made a declared gap look unavoidable. The
    // honesty contract said an unreadable contract costs the operator's
    // declared boundaries because "the list that made it a boundary is the
    // thing that went missing", and that holds only when the boundary row is
    // itself the bad one. Here it is not: `Irreversible commands` parses,
    // and a mistyped `Renewal window` three lines away threw it away. The
    // gate then classified `terraform apply` as a routine write, which never
    // reaches the phase question — the one place *configuration may only
    // tighten* runs backwards.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install succeeds");
    fs::write(
        skill.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |
|---|---|
             | Irreversible commands | terraform apply |
             | Renewal window | 30 days |
",
    )
    .expect("write the override");
    let here = root.path();

    // The floor: this document really is one the strict read refuses, and
    // the row it refuses over is not the boundary row.
    let strict = installed_config_in(&skill, here);
    let refusal = strict.expect_err("the document parses, so this measures nothing");
    assert!(
        refusal.message.contains("Renewal window"),
        "the refusal is about the wrong row: {}",
        refusal.message
    );

    let (kept, complaint) = installed_config_in_keeping_what_parses(&skill, here);
    assert_eq!(
        kept.boundaries,
        vec!["terraform apply".to_owned()],
        "a row that parsed was thrown away with one that did not"
    );
    // And the complaint is the same one every other caller gets, so nothing
    // has become quieter: `doctor` still names the file and the row.
    assert_eq!(
        complaint.map(|refusal| refusal.message),
        Some(refusal.message),
        "the gate salvaged the rows and stopped saying anything was wrong"
    );

    // The window is *not* salvaged into something wider: `30 days` is refused
    // by its setter, so what is kept is the default, and the gate closes the
    // window itself whenever it had to salvage. Keeping what parses can only
    // narrow — falling back to `Config::default` is the loosest there is.
    assert_eq!(kept.window, crate::harness::RENEWAL_WINDOW);

    // And a document with nothing wrong in it still comes back through the
    // strict path, complaining about nothing.
    fs::write(
        skill.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |
|---|---|
| Irreversible commands | terraform apply |
",
    )
    .expect("write the override");
    let (clean, quiet) = installed_config_in_keeping_what_parses(&skill, here);
    assert!(
        quiet.is_none(),
        "a document that parses was reported as broken"
    );
    assert_eq!(clean.boundaries, vec!["terraform apply".to_owned()]);
}

#[test]
fn a_repository_row_that_did_not_parse_keeps_the_workflow_beside_it() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install succeeds");
    let (_repository, here) =
        repository_with_rows("| Claim workflow | assisted |\n| Renewal window | 30 days |\n");

    let kept = installed_config_in_keeping_what_parses_apart(&skill, &here);
    assert!(
        kept.row.is_some(),
        "the bad row was dropped without a complaint"
    );
    assert!(kept.contract.is_none());
    assert_eq!(
        kept.config.workflow,
        crate::config::Workflow::Assisted,
        "a row that parsed was lost with the one that did not"
    );
    assert_eq!(
        kept.config.window,
        std::time::Duration::from_secs(1),
        "a renewal window nobody can read was read wider than one second"
    );
}

#[test]
fn a_repository_workflow_row_that_does_not_parse_shuts_the_tracker_writers() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install succeeds");
    let (_repository, here) = repository_with_rows("| Claim workflow | maybe |\n");

    let kept = installed_config_in_keeping_what_parses_apart(&skill, &here);
    assert!(kept.row.is_some());
    assert_eq!(
        kept.config.workflow,
        crate::config::Workflow::Assisted,
        "a workflow row nobody can read left the tracker writers open"
    );
}

#[test]
fn an_unreadable_repository_file_keeps_the_layers_underneath_it() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let here = root.path().join("repo");
    std::fs::create_dir_all(&here).expect("the checkout");
    let config = Config {
        boundaries: vec!["terraform apply".to_owned()],
        ..Config::default()
    };
    install(&skill, &config, false).expect("install succeeds");

    let file = repository_config_path(&here);
    fs::create_dir_all(file.parent().expect("a parent")).expect("the rows directory");
    fs::write(&file, [0xff, 0xfe]).expect("an unreadable repository file");

    let (kept, complaint) = installed_config_in_keeping_what_parses(&skill, &here);
    assert_eq!(
        kept.boundaries,
        vec!["terraform apply".to_owned()],
        "a repository file that cannot be read discarded the contract's own rows"
    );
    let complaint = complaint.expect("the unreadable file was not reported");
    assert!(
        complaint.message.contains(&file.display().to_string()),
        "the complaint does not name the file it could not read: {}",
        complaint.message
    );
    assert!(
        complaint.message.contains("UTF-8"),
        "the complaint does not name what the read answered: {}",
        complaint.message
    );
}

#[test]
fn a_bad_contract_row_does_not_hide_an_unreadable_repository_file() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let here = root.path().join("repo");
    std::fs::create_dir_all(&here).expect("the checkout");
    let config = Config {
        boundaries: vec!["terraform apply".to_owned()],
        ..Config::default()
    };
    install(&skill, &config, false).expect("install succeeds");

    let contract = skill.join(CONTRACT);
    let installed = fs::read_to_string(&contract).expect("the contract reads");
    let broken = installed.replace("| Change size | 800 |", "| Change size | banana |");
    assert_ne!(
        broken, installed,
        "no row was corrupted, so this measures nothing"
    );
    fs::write(&contract, broken).expect("the bad contract row is written");

    let file = repository_config_path(&here);
    fs::create_dir_all(file.parent().expect("a parent")).expect("the rows directory");
    fs::write(&file, [0xff, 0xfe]).expect("an unreadable repository file");

    let kept = installed_config_in_keeping_what_parses_apart(&skill, &here);
    assert!(
        kept.contract.is_some(),
        "the bad contract row was not reported"
    );
    assert!(
        kept.file.is_some(),
        "the unreadable repository file was hidden by the contract's own failure"
    );
    assert_eq!(
        kept.config.boundaries,
        vec!["terraform apply".to_owned()],
        "the rows the contract did parse were thrown away with the bad one"
    );
}

#[test]
fn an_override_beside_the_contract_wins_and_the_legacy_name_still_works() {
    for name in [crate::config::LOCAL_FILE, crate::config::LEGACY_LOCAL_FILE] {
        let root = tempfile::tempdir().expect("a temporary root");
        let skill = root.path().join(DIRECTORY);
        install(&skill, &Config::default(), false).expect("install succeeds");
        fs::write(
            skill.join(name),
            "| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Evidence standard | measuring | reading |\n",
        )
        .expect("write the override");

        assert_eq!(
            installed_config(&skill).expect("the config reads").evidence,
            crate::config::Evidence::Measuring,
            "{name} beside the contract did not override it"
        );
    }
}

/// What this crate used to ship is taken off a machine that has it.
///
/// The mirror of the test this replaced, which asserted the transport was
/// **installed** beside the binding that ran it. Nothing runs it now — the
/// operations are answered in process and the binding names Estigia's tools
/// — so what has to be true is the opposite, and it is the harder half: a
/// path dropped from `FILES` is a file nobody removes, left in a directory
/// the contract no longer mentions for anything that goes looking to find
/// and run.
#[test]
fn the_retired_transport_is_taken_off_a_machine_that_has_it() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install succeeds");

    let transport = skill.join("scripts").join("github.py");
    assert!(
        !transport.is_file(),
        "a fresh install still writes {}",
        transport.display()
    );

    // And the case that matters: a machine that already had it. The file is
    // put back **and named in the record**, which is the only thing that
    // makes it this crate's to remove.
    std::fs::create_dir_all(transport.parent().expect("a directory")).expect("the directory");
    std::fs::write(
        &transport,
        "print('the old transport')
",
    )
    .expect("the file");
    record::note_created(&skill, std::iter::once("scripts/github.py")).expect("the record");

    install(&skill, &Config::default(), false).expect("the next install succeeds");
    assert!(
        !transport.is_file(),
        "{} survived an install that retires it",
        transport.display()
    );
}

#[test]
fn the_one_way_doors_this_repository_declared_are_named_where_the_agent_reads() {
    // The contract asks the agent to renew "at every expensive or
    // irreversible boundary" — a judgement. The operator's list is the part
    // it cannot derive: `terraform apply` is a one-way door *here*, and
    // nowhere in the sentence that asks for the judgement.
    //
    // The gate enforces the list either way, so this is not about letting
    // something through. It is about which of the two ways the agent finds
    // out: reading it, or being stopped.
    let mut declared = Config::default();
    crate::config::Setting::Boundaries
        .apply(&mut declared, "terraform apply, npm publish")
        .expect("a list the parser accepts");

    let body = configuration_body(&declared);
    assert!(
        body.contains("terraform apply") && body.contains("npm publish"),
        "the doors this repository declared are not named where the agent reads"
    );
    // On top of, not instead of: the built-in boundaries are not removable,
    // and a sentence that read like a replacement would say the opposite of
    // what the setting is.
    assert!(
        body.contains("on top of"),
        "the declared list reads as though it replaced the contract's own"
    );

    // A repository that declared none says nothing: the general rule is
    // already in the contract, and repeating it is the noise this block has
    // no room for.
    assert!(
        !configuration_body(&Config::default()).contains("one-way doors, on top"),
        "a repository with no declared boundaries is told about them anyway"
    );
}

#[test]
fn a_repository_that_lands_on_trunk_says_so_before_the_refusal_does() {
    // The word `trunk` appears nowhere in the skill — not once, in any file
    // the agent reads. So an agent on a trunk repository took the branch
    // route it was taught, reached the delivering write, and met
    // `unflagged-on-trunk`: told to name a flag it had never been told this
    // repository runs on, at the most expensive moment to learn it.
    //
    // The gate refuses either way. What this changes is whether being
    // refused is how the agent finds out.
    let trunk = Config {
        integration: crate::config::Integration::Trunk,
        ..Config::default()
    };
    let body = configuration_body(&trunk);
    assert!(
        body.contains("lands on **trunk**"),
        "a trunk repository does not tell the agent where work lands"
    );
    // And names the mechanism the refusal will ask for, in the same words.
    assert!(
        body.contains("ESTIGIA_FLAG"),
        "it says work lands early and not what makes that allowed"
    );
    // Without overstating it: naming a flag is a claim, not a proof, and
    // `out_of_phase` says so where it accepts one.
    assert!(
        body.contains("not proof"),
        "it offers the flag as though Estigia could check it"
    );

    // A branch repository is the skill's own assumption and needs no
    // sentence: a block regenerated on every run has no room for prose
    // about the arrangement it already describes.
    assert!(
        !configuration_body(&Config::default()).contains("lands on **trunk**"),
        "a branch repository is told about trunk anyway"
    );
}

#[test]
fn a_named_model_routing_reaches_the_agent_that_has_to_honour_it() {
    // `Model routing` is `Scope::Agent`: an adapter's own file may move the
    // row, and this sentence — rendered once into a shared contract — cannot
    // follow, so it must defer to the row rather than restate its value.
    let mut routed = Config::default();
    crate::config::Setting::Models
        .apply(&mut routed, "developer=opus, reviewer=sonnet")
        .expect("a routing the parser accepts");

    let body = configuration_body(&routed);
    assert!(
        body.contains("Where `Model routing` names a model"),
        "a contract with a routing does not tell the agent to honour it"
    );

    // The value belongs to the row and to nothing else: every adapter reads
    // these bytes, and each one's row may say something else.
    let prose = selected_documents(&routed);
    assert!(
        !prose.contains("developer=opus") && !prose.contains("reviewer=sonnet"),
        "the shared sentence restates a routing a per-adapter row can move:\n{prose}"
    );

    // A host fact has to name its host, because the adapters sharing a root
    // read one file.
    assert!(
        body.contains("Claude Code") && !body.contains("this host"),
        "the generated block points at whichever host is reading:\n{body}"
    );

    // The routed family is named: a role key is already written too.
    assert!(
        body.contains("the roles the routing names"),
        "the sentence hides a definition `setup` already routes:\n{body}"
    );

    // And an unset routing says nothing, because there is nothing to say.
    assert!(
        !configuration_body(&Config::default()).contains("Where `Model routing` names a model"),
        "a contract with no routing explains routing anyway"
    );
}

#[test]
fn decision_mode_auto_reaches_the_agent_and_ask_says_nothing() {
    let mut auto = Config::default();
    crate::config::Setting::DecisionMode
        .apply(&mut auto, "auto")
        .expect("auto parses");
    assert_eq!(
        configuration_body(&auto)
            .matches(crate::skill::DECISION_MODE_AUTO)
            .count(),
        1
    );
    assert!(!configuration_body(&Config::default()).contains(crate::skill::DECISION_MODE_AUTO));
}

#[test]
fn a_by_task_routing_reaches_the_agent_with_its_policy() {
    let mut routed = Config::default();
    crate::config::Setting::Models
        .apply(&mut routed, crate::config::BY_TASK)
        .expect("the third value parses");

    let body = configuration_body(&routed);
    assert!(
        body.contains(crate::skill::BY_TASK_POLICY),
        "a `by task` routing does not carry the policy the agent has to follow:\n{body}"
    );
    assert_eq!(
        body.matches(crate::skill::BY_TASK_POLICY).count(),
        1,
        "the policy is restated rather than routed from one place"
    );
    for part in [
        "the shape of the work decides",
        "cheap and shallow",
        "expensive and deliberate",
        "the launcher",
        "recording the model and the effort",
    ] {
        assert!(
            body.contains(part),
            "the policy never says {part:?}:\n{body}"
        );
    }
    // The by-role paragraph is a different routing and is not emitted.
    assert!(
        !body.contains("Where `Model routing` names a model"),
        "a `by task` routing also explained the per-role routing:\n{body}"
    );
    // And an unset routing says nothing about either.
    assert!(
        !configuration_body(&Config::default()).contains("by task"),
        "a contract with no routing explains `by task` anyway"
    );
}

#[test]
fn the_table_sends_a_shared_agent_to_its_own_override_before_the_rows() {
    let body = configuration_body(&Config::default());
    let table = body.find("| Setting |").expect("the table is rendered");
    let preamble = &body[..table];
    assert!(
        preamble.contains("`estigia.<agent>.md`"),
        "an agent on a shared root reads these rows and is never told its own file overrides them:\n{preamble}"
    );
    // The scope, not merely the file: only the agent-scoped rows move with an adapter.
    assert!(
        preamble.contains("agent-scoped rows") && preamble.contains("`Model routing`"),
        "the preamble names the per-agent file without saying which rows it can move:\n{preamble}"
    );
    assert!(
        preamble.contains("`estigia.local.md` wins"),
        "the preamble names the per-agent file without saying which file wins:\n{preamble}"
    );
}

#[test]
fn a_row_set_to_ask_tells_the_agent_what_asking_obliges() {
    // `selected_documents` states the rule this checks: *a setting the
    // agent cannot act on is a setting that does nothing.* `ask` accepts a
    // duration, and what the duration does — wait, then record the proposed
    // transition as a comment instead of applying it — lived in a doc
    // comment on `Authority::Ask` and nowhere the agent reads. Nothing in
    // Estigia enforced it either, and nothing read the timeout. So
    // `ask 30m` was a value the screen offered, the table recorded, and
    // nobody was obliged by.
    let mut asking = Config::default();
    asking.set_profile(crate::config::Profile::Cautious);
    assert!(
        matches!(asking.delivery, crate::config::Authority::Ask { .. }),
        "`cautious` does not carry an `ask`, which is what makes this the common case"
    );
    let body = configuration_body(&asking);
    assert!(
        body.contains("propose and wait"),
        "a contract with an `ask` row does not say what asking obliges"
    );
    assert!(
        body.contains("instead of applying it"),
        "it says to wait and not what the waiting ends in"
    );

    let body = configuration_body(&Config::default());
    assert!(
        !body.contains("permission to acquire a reviewer"),
        "the contract still describes a review consent that is retired"
    );

    // And a machine that decides on its own is not told about a rule it
    // will never meet: noise in a block regenerated on every run is noise
    // nobody can delete.
    let deciding = Config {
        delivery: crate::config::Authority::Auto,
        transitions: crate::config::Authority::Auto,
        ..Config::default()
    };
    assert!(
        !configuration_body(&deciding).contains("propose and wait"),
        "a contract with no `ask` row explains asking anyway"
    );
}

#[test]
fn the_two_language_rows_are_two_rows_everywhere_they_are_read() {
    // There was one row, `Task body language`, and the only thing in the
    // whole skill that consulted it was one sentence of the analyst
    // template — the summary callout. So the row was
    // named for the body and reached the summary: a setting widened by
    // being renamed rather than by anything reading it. The split is only
    // real if **both** rows reach the agent, so both are crossed here
    // against the two places that carry them.
    let template = include_str!("../../skill/assets/analyst-issue-template.md");
    for setting in [
        crate::config::Setting::Summary,
        crate::config::Setting::Body,
    ] {
        assert!(
            template.contains(setting.label()),
            "`{}` is a row nothing in the template names, so nothing writes prose in it",
            setting.label()
        );
    }

    // Different answers, so a contract that carried one of them twice — or
    // collapsed them back into one — cannot pass.
    let split = Config {
        summary_language: crate::config::Language::parse("Español").expect("a language"),
        body_language: crate::config::Language::parse("Deutsch").expect("a language"),
        ..Config::default()
    };
    let body = configuration_body(&split);
    assert!(
        body.contains("in **Español**") && body.contains("in **Deutsch**"),
        "the contract does not carry both languages: {body}"
    );
    assert!(
        body.contains("the title and every section from `## Problem` down in **Deutsch**"),
        "the title is written outside the template, and the contract does not say which row governs it: {body}"
    );
    assert!(
        body.contains("a pull request's title and body in **Deutsch**"),
        "a pull request is titled outside any template, and the contract does not say which row governs it: {body}"
    );
    assert!(
        body.contains("whatever language the repository"),
        "a repository written in another language overrides the rows unless the contract says it does not: {body}"
    );
    // And the headings are named as the thing that never moves. They are
    // the contract's own vocabulary and the machine marker's neighbours.
    assert!(
        body.contains("Headings are never translated"),
        "nothing stops an agent translating `## Problem` along with the prose"
    );

    // A repository that writes both in English is not told about a rule it
    // will never meet: noise in a block regenerated on every run is noise
    // nobody can delete.
    assert!(
        !configuration_body(&Config::default()).contains("Write the summary sentence"),
        "a contract at the default explains the languages anyway"
    );
    // But moving either one alone is enough to say it, or the row that was
    // moved would be the one that goes unmentioned.
    for moved in [
        Config {
            summary_language: crate::config::Language::parse("Español").expect("a language"),
            ..Config::default()
        },
        Config {
            body_language: crate::config::Language::parse("Español").expect("a language"),
            ..Config::default()
        },
    ] {
        assert!(
            configuration_body(&moved).contains("Write the summary sentence"),
            "one row moved away from the default and the contract said nothing"
        );
    }
}

#[test]
fn the_issue_template_asks_for_what_an_implementer_cannot_decide() {
    let template = include_str!("../../skill/assets/analyst-issue-template.md");
    for asked in [
        "### Decisions taken",
        "### If this does not match",
        "The implementer has this issue and the repository, and nothing else.",
    ] {
        assert!(
            template.contains(asked),
            "the template never asks for {asked:?}"
        );
    }
}

#[test]
fn the_contract_carries_the_resolved_configuration() {
    let rendered = rendered(&FILES[0], &Config::default());
    assert_eq!(FILES[0].path, CONTRACT);
    assert!(rendered.contains(crate::config::BLOCK_BEGIN));
    assert!(!rendered.contains("issue-flow:config:start"));
    assert!(rendered.contains("| Integration mode | branch | branch |"));
    // Everything above the block is untouched contract.
    assert!(rendered.starts_with(&format!("---\nname: {DIRECTORY}")));
}

#[test]
fn uninstalling_over_somebody_else_s_checkout_leaves_the_checkout() {
    // Found by running it: `estigia setup agents` on a machine whose
    // `~/.agents/skills/issue-flow` was a real issue-flow clone, then
    // `--uninstall`. It deleted `SKILL.md`, `bindings/`, `references/`,
    // `assets/` and `scripts/` — the clone. Estigia had created none of
    // them: it had overwritten two and found the rest already correct, and
    // afterwards no two of those cases can be told apart from the disk,
    // because all of them then hold Estigia's text.
    //
    // This is the arrangement Estigia is built for, not a mishap. It
    // installs upstream's skill under upstream's name so the two are one
    // directory.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let theirs = skill.join("references");
    fs::create_dir_all(&theirs).expect("their directory");

    // Two shapes of pre-existing file: one Estigia will overwrite, and one
    // it will find already exactly right. Both were somebody else's.
    let overwritten = skill.join(relative(CONTRACT));
    fs::write(&overwritten, "# their contract\n").expect("their contract");
    let already_right = FILES
        .iter()
        .find(|file| file.path.starts_with("references/"))
        .expect("the skill ships a reference");
    let untouched = skill.join(relative(already_right.path));
    fs::write(&untouched, already_right.contents).expect("their reference");

    let installed = install(&skill, &Config::default(), false).expect("install");
    assert_eq!(
        installed
            .actions
            .iter()
            .filter(|action| action.change == Change::Create)
            .count(),
        // Plus the record itself, which this install creates and names.
        FILES.len() - 2 + 1,
        "the two that were already there were not created"
    );

    let removed = uninstall(&skill, false).expect("uninstall");
    assert!(
        overwritten.is_file(),
        "uninstall deleted a file that was here before Estigia overwrote it"
    );
    assert!(
        untouched.is_file(),
        "uninstall deleted a file Estigia never even changed"
    );
    assert_eq!(
        removed
            .actions
            .iter()
            .filter(|action| action.change == Change::Kept)
            .count(),
        2,
        "and says so, rather than leaving them behind silently"
    );
    // Everything Estigia did create is gone, so a second uninstall has
    // nothing to do and the directory is theirs again.
    for file in FILES {
        let path = skill.join(relative(file.path));
        if path == overwritten || path == untouched {
            continue;
        }
        assert!(!path.exists(), "{} outlived the uninstall", path.display());
    }
    assert!(
        !skill.join(".estigia").exists(),
        "the record outlived what it was recording"
    );
}

#[test]
fn a_missing_record_and_a_file_that_predates_estigia_are_not_the_same_answer() {
    // Both leave the file alone, and only one of them is a fact. "This was
    // here before Estigia" is something a record establishes; "there is no
    // record" is the absence of one. Reported as the first, an operator
    // whose record had been deleted was told their files predated an
    // install that had in fact written every one of them.
    let root = tempfile::tempdir().expect("a temporary root");

    // A record that exists and does not name this file.
    let theirs = root.path().join("theirs");
    fs::create_dir_all(&theirs).expect("their directory");
    fs::write(theirs.join(relative(CONTRACT)), "# theirs\n").expect("their contract");
    install(&theirs, &Config::default(), false).expect("install");
    let over = uninstall(&theirs, false).expect("uninstall");
    assert_eq!(
        over.actions
            .iter()
            .filter(|action| action.change == Change::Kept)
            .count(),
        1,
        "the one file that was here first is the one that was here first"
    );
    assert!(
        !over
            .actions
            .iter()
            .any(|action| action.change == Change::Unrecorded),
        "a record was written, so nothing here is unaccounted for"
    );

    // No record at all.
    let ours = root.path().join("ours");
    install(&ours, &Config::default(), false).expect("install");
    fs::remove_dir_all(ours.join(".estigia")).expect("forget the record");
    let blind = uninstall(&ours, false).expect("uninstall");
    assert_eq!(
        blind
            .actions
            .iter()
            .filter(|action| action.change == Change::Unrecorded)
            .count(),
        FILES.len(),
        "without a record nothing here can be shown to be Estigia's"
    );
    assert!(
        !blind
            .actions
            .iter()
            .any(|action| action.change == Change::Kept),
        "nothing here predates Estigia, and saying so would be an invention"
    );
}

#[test]
fn an_install_with_no_record_of_itself_removes_nothing() {
    // The record is how uninstall knows its own files. Absent one — a
    // directory somebody else's tool filled, or one installed by a build
    // that predates the record — the honest answer is that Estigia cannot
    // show it wrote any of this, and a tool whose subject is authority does
    // not delete on a guess.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");
    fs::remove_dir_all(skill.join(".estigia")).expect("forget the record");

    let removed = uninstall(&skill, false).expect("uninstall");
    assert_eq!(removed.changed_files(), 0, "nothing was Estigia's to take");
    for file in FILES {
        assert!(
            skill.join(relative(file.path)).exists(),
            "{} was removed without a record saying Estigia wrote it",
            file.path
        );
    }
}

#[test]
fn a_row_nobody_can_read_names_the_file_it_is_in() {
    // Three files can carry a table under one skill root, and only one of
    // them is Estigia\'s to fix — it will not edit the operator\'s. Refusing
    // with `Loop plugin is "maybe"` and no path left them opening
    // all three to find which one they had typed it into.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");

    let bad = format!(
        "{}\n| Setting | Value here |\n|---|---|\n| Loop plugin | maybe |\n{}\n",
        CONFIG_FENCE.begin, CONFIG_FENCE.end
    );
    for path in [
        agent_override(&skill, "codex"),
        skill.join(crate::config::LOCAL_FILE),
    ] {
        fs::write(&path, &bad).expect("a file with a row nobody can read");
        let refusal = installed_config_for(&skill, Some("codex"))
            .expect_err("`maybe` is not a loop plugin value");
        assert_eq!(refusal.code, "config-value-unrecognised");
        let message = format!("{refusal}");
        assert!(
            message.contains(&path.display().to_string()),
            "the refusal does not say which file: {message}"
        );
        fs::remove_file(&path).expect("clean up between the two");
    }
}

#[test]
fn a_row_no_setting_resolves_is_named_and_never_its_value() {
    // `estigia.local.md` is never regenerated, so a row outlives the
    // setting it names, and a typo is indistinguishable from a deleted
    // setting: both drop out of every reader in silence. The reader reports
    // the label and nothing else — this file holds permissions and machine
    // paths, and none of it may be published.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");
    let local = skill.join(crate::config::LOCAL_FILE);
    fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n\
             | Evidence standard | measuring |\n\
             | Blind judges | hunter2-the-value |\n",
    )
    .expect("the operator's file");

    assert_eq!(
        unknown_override_rows(&skill),
        vec!["Blind judges".to_owned()],
        "the rows no setting resolves are not what the reader names"
    );
    assert!(
        !format!("{:?}", unknown_override_rows(&skill)).contains("hunter2-the-value"),
        "a value from the operator's file reached the reader's answer"
    );

    // A file that cannot be opened is not this reader's failure to raise:
    // `local_overrides` already refuses that case by name, and a report
    // must not become a second way for a command to fail.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&local, fs::Permissions::from_mode(0o000))
            .expect("the file is made unreadable");
        // A reader running as root reads it anyway; then there is nothing
        // to measure and the assertion would be about the user, not the
        // code.
        if fs::read_to_string(&local).is_err() {
            assert!(
                unknown_override_rows(&skill).is_empty(),
                "an unreadable override file was reported by the label reader"
            );
        }
        fs::set_permissions(&local, fs::Permissions::from_mode(0o600))
            .expect("the file is made readable again");
    }
}

#[test]
fn a_retired_row_is_not_reported_as_an_unknown_override() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");
    let local = skill.join(crate::config::LOCAL_FILE);
    fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n\
             | Delivery route | direct |\n\
             | Review delegation | auto |\n\
             | Merge strategy | squash |\n\
             | Review protocol | receipt-driven |\n\
             | Blind judges | 3 |\n",
    )
    .expect("the operator's file");

    assert_eq!(
        unknown_override_rows(&skill),
        vec!["Blind judges".to_owned()],
        "a retired row is reported by its own reader, not as an unknown one"
    );
}

#[test]
fn one_setting_named_twice_in_one_file_is_countable() {
    // Rows apply in the order they are met, so of two rows for one setting
    // the lower one holds. Measured, not assumed: with the current label
    // written *above* the label it replaced, the dead label is what every
    // command reports — position wins, not canonicity, and nothing said so.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");
    let local = skill.join(crate::config::LOCAL_FILE);
    let alias = crate::config::Setting::Summary.aliases()[1];
    let header = "| Setting | Value |\n|---|---|\n";

    // Named once, however spelled: nothing to report.
    fs::write(&local, format!("{header}| {alias} | A |\n")).expect("their file");
    assert_eq!(duplicated_rows(&skill, "codex"), Vec::new());

    // Named twice, and the file's line order is what decides which holds.
    fs::write(
        &local,
        format!("{header}| Summary language | B |\n| {alias} | A |\n"),
    )
    .expect("their file");
    assert_eq!(
        duplicated_rows(&skill, "codex"),
        vec![(local.display().to_string(), "Summary language")],
        "one setting named twice is not counted"
    );

    // And the value in force is the lower row — which is what makes the
    // silence worth breaking rather than a note about tidiness.
    let held = installed_config_for(&skill, Some("codex")).expect("a configuration");
    assert_eq!(held.summary_language.as_str(), "A");
}

#[test]
fn an_operator_file_the_newer_name_killed_is_countable() {
    // Both names exist so an installation that came from issue-flow keeps
    // working. The moment somebody writes `estigia.local.md`, the older
    // file stops being read — entirely, not row by row — and nothing said
    // so. The contract even promised the opposite: "`operator.local.md` is
    // still read where it exists", which is false in exactly the migration
    // it was written for.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");

    let older = skill.join(crate::config::LEGACY_LOCAL_FILE);
    let newer = skill.join(crate::config::LOCAL_FILE);

    // Neither, and the older alone: nothing to report. An installation that
    // never migrated is working exactly as it should.
    assert_eq!(shadowed_local(&skill), None);
    fs::write(&older, "# theirs\n").expect("their older file");
    assert_eq!(
        shadowed_local(&skill),
        None,
        "the older file alone is the one being read"
    );

    // Both: the older one is dead and this is the only thing that says it.
    fs::write(&newer, "# theirs\n").expect("their newer file");
    assert_eq!(
        shadowed_local(&skill).as_deref(),
        Some(older.display().to_string().as_str()),
        "the file that stopped being read is not named"
    );

    // And `local_override` still answers the other half — which one *is*
    // read — so the two agree about which is which.
    assert_eq!(local_override(&skill), Some(newer));
}

#[test]
fn a_row_an_agent_s_own_file_writes_in_vain_is_countable() {
    // The behaviour is right and it is silent: a per-agent file may narrow
    // what its own agent does and may not restate what the repository is,
    // so those rows are put back from the contract. The operator wrote a
    // value and every command answers a different one.
    //
    // `config set --agent` refuses to write such a row. It arrives the
    // other two ways anyway — typed by hand, or written by a build from
    // before that row belonged to the repository — and reporting an effect
    // that did not happen is the failure this whole tool exists to refuse.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");

    let repository = crate::config::SETTINGS
        .iter()
        .find(|setting| setting.scope() == crate::config::Scope::Everywhere)
        .expect("some setting belongs to the repository");
    let theirs = crate::config::SETTINGS
        .iter()
        .find(|setting| setting.scope() == crate::config::Scope::Agent)
        .expect("some setting differs by agent");
    let machine = crate::config::SETTINGS
        .iter()
        .find(|setting| setting.scope() == crate::config::Scope::Machine)
        .expect("some setting belongs to the machine");

    // A file with one of each: only its own agent row is in force.
    fs::write(
        agent_override(&skill, "opencode"),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | {} | {} | x |\n| {} | {} | x |\n| {} | {} | x |\n{}\n",
            CONFIG_FENCE.begin,
            repository.label(),
            repository.value_of(&Config::default()),
            machine.label(),
            machine.value_of(&Config::default()),
            theirs.label(),
            theirs.value_of(&Config::default()),
            CONFIG_FENCE.end
        ),
    )
    .expect("their file");

    let ignored = overridden_rows(&skill, "opencode");
    assert_eq!(
        ignored,
        vec![repository.label(), machine.label()],
        "the rows nothing reads are exactly the non-agent rows"
    );

    // No file at all, and a file with nothing of the repository's in it,
    // both count nothing — an operator with a plain per-agent file is not
    // told they have a problem.
    assert!(overridden_rows(&skill, "cursor").is_empty());
    fs::write(
        agent_override(&skill, "cursor"),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n| {} | {} | x |\n{}\n",
            CONFIG_FENCE.begin,
            theirs.label(),
            theirs.value_of(&Config::default()),
            CONFIG_FENCE.end
        ),
    )
    .expect("their file");
    assert!(overridden_rows(&skill, "cursor").is_empty());
}

#[test]
fn one_agent_s_file_cannot_restate_what_the_repository_is() {
    // The per-agent file exists so two agents can be told different things
    // about what *they* may do. It is not a second answer to what the
    // repository is: the gate reads those rows without asking whose turn it
    // is, so a tracker set for one adapter had the agent talking to Linear
    // while the gate read GitHub. `config set --agent` refuses to write
    // them, and this is the other half — the file is text, and an operator
    // or an older build can put anything in it.
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("install");

    // A setting with only one answer cannot demonstrate anything, so each
    // side is the first that has a second answer to move to.
    let pick = |scope| {
        crate::config::SETTINGS
            .iter()
            .find_map(|setting| {
                (setting.scope() == scope)
                    .then(|| other_answer(*setting, &Config::default()))
                    .flatten()
                    .map(|answer| (*setting, answer))
            })
            .expect("a setting with more than one answer")
    };
    let (repository, moved_repository) = pick(crate::config::Scope::Everywhere);
    let (mine, moved_mine) = pick(crate::config::Scope::Agent);

    // Both moved, in one file, by hand.
    let mut wanted = Config::default();
    repository
        .apply(&mut wanted, &moved_repository)
        .expect("a value the setting accepts");
    mine.apply(&mut wanted, &moved_mine)
        .expect("a value the setting accepts");
    fs::write(
        agent_override(&skill, "opencode"),
        format!(
            "# theirs\n\n{}\n{}\n{}\n",
            CONFIG_FENCE.begin,
            wanted.render_rows(),
            CONFIG_FENCE.end
        ),
    )
    .expect("their file");

    let read = installed_config_for(&skill, Some("opencode")).expect("the layered read");
    assert_eq!(
        mine.value_of(&read),
        moved_mine,
        "{} differs by agent and the agent\'s own file did not move it",
        mine.label()
    );
    assert_eq!(
        repository.value_of(&read),
        repository.value_of(&Config::default()),
        "{} is a fact about the repository and one agent\'s file changed it",
        repository.label()
    );
}

#[test]
fn config_layers_keep_contract_agent_local_and_repository_snapshots_distinct() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    install(&skill, &Config::default(), false).expect("the contract installs");

    let mut agent = Config::default();
    crate::config::Setting::Delivery
        .apply(&mut agent, "ask")
        .expect("Delivery authorisation is accepted");
    crate::setup::write_agent_configuration_wholly(
        &agent_override(&skill, "opencode"),
        "opencode",
        &agent,
    )
    .expect("the agent layer is written");
    fs::write(
        skill.join(crate::config::LOCAL_FILE),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Model routing | developer=inherit | none |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("the local layer is written");
    let repository_path = repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config {
            corrections_per_lineage: 1,
            ..Config::default()
        },
        crate::config::EVERYWHERE_SETTINGS,
    )
    .expect("the repository layer is written");

    let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
        .expect("all four layers read");

    assert_eq!(
        crate::config::Setting::Delivery.value_of(&layers.contract),
        "auto"
    );
    assert_eq!(
        crate::config::Setting::Delivery
            .value_of(layers.agent.as_ref().expect("the agent snapshot")),
        "ask"
    );
    assert_eq!(
        crate::config::Setting::Models.value_of(layers.local.as_ref().expect("the local snapshot")),
        "developer=inherit"
    );
    assert_eq!(
        crate::config::Setting::CorrectionsPerLineage
            .value_of(layers.repository.as_ref().expect("the repository snapshot")),
        "1"
    );
}

#[test]
fn an_agent_files_repository_row_cannot_pin_a_new_portable_value() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    fs::write(
        agent_override(&skill, "opencode"),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Evidence standard | measuring | reading |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the hand-edited agent file exists");
    let layers = config_layers(&skill, Some("opencode"), None).expect("the layers read");
    let mut portable = Config::default();
    crate::config::Setting::Evidence
        .apply(&mut portable, "reading")
        .expect("the newer contract value is accepted");

    let effective = layers
        .effective_over(&portable)
        .expect("the effective view composes");

    assert_eq!(
        layers.agent_settings,
        Vec::<crate::config::Setting>::new(),
        "a forbidden repository row claimed agent-file ownership"
    );
    assert_eq!(
        crate::config::Setting::Evidence.value_of(&effective),
        "reading",
        "the ignored row reapplied the older contract value"
    );
}

#[test]
fn a_repositorys_model_row_cannot_pin_a_new_portable_value() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    install(&skill, &Config::default(), false).expect("the contract installs");
    let repository_path = repository_config_path(repository.path());
    fs::create_dir_all(
        repository_path
            .parent()
            .expect("the repository file has a parent"),
    )
    .expect("the repository configuration directory exists");
    fs::write(
        repository_path,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Model routing | developer=opus | none |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the hand-edited repository file exists");
    let layers =
        config_layers(&skill, Some("opencode"), Some(repository.path())).expect("the layers read");
    let mut portable = Config::default();
    crate::config::Setting::Models
        .apply(&mut portable, "developer=sonnet")
        .expect("the newer Model routing value is accepted");

    let effective = layers
        .effective_over(&portable)
        .expect("the effective view composes");

    assert_eq!(
        layers.repository_settings,
        Vec::<crate::config::Setting>::new(),
        "a forbidden agent row claimed repository-file ownership"
    );
    assert_eq!(
        crate::config::Setting::Models.value_of(&effective),
        "developer=sonnet",
        "the ignored row reapplied the older contract value"
    );
}

#[test]
fn an_invalid_forbidden_agent_row_is_ignored_before_validation() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    fs::write(
        agent_override(&skill, "opencode"),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Integration | impossible | branch |\n\
                 | Delivery authorisation | ask | auto |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the hand-edited agent file exists");

    let layers = config_layers(&skill, Some("opencode"), None)
        .expect("the forbidden invalid row is outside this document's scope");

    assert_eq!(
        layers.agent_settings,
        vec![crate::config::Setting::Delivery]
    );
    assert_eq!(
        crate::config::Setting::Delivery
            .value_of(layers.agent.as_ref().expect("the agent layer exists")),
        "ask"
    );
}

#[test]
fn an_invalid_permitted_agent_row_still_refuses() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).expect("the contract installs");
    fs::write(
        agent_override(&skill, "opencode"),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Delivery authorisation | sometimes | auto |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the hand-edited agent file exists");

    let refusal = config_layers(&skill, Some("opencode"), None)
        .expect_err("an invalid owned row was ignored");

    assert_eq!(refusal.code, "config-value-unrecognised");
}

#[test]
fn an_invalid_forbidden_repository_row_is_ignored_before_validation() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    install(&skill, &Config::default(), false).expect("the contract installs");
    let path = repository_config_path(repository.path());
    fs::create_dir_all(path.parent().expect("the repository file has a parent"))
        .expect("the repository configuration directory exists");
    fs::write(
        path,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Model routing | nonsense | none |\n\
                 | Corrections per lineage | 1 | 3 |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the hand-edited repository file exists");

    let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
        .expect("the forbidden invalid row is outside this document's scope");

    assert_eq!(
        layers.repository_settings,
        vec![crate::config::Setting::CorrectionsPerLineage]
    );
    assert_eq!(
        crate::config::Setting::CorrectionsPerLineage.value_of(
            layers
                .repository
                .as_ref()
                .expect("the repository layer exists")
        ),
        "1"
    );
}

#[test]
fn an_invalid_permitted_repository_row_still_refuses() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    install(&skill, &Config::default(), false).expect("the contract installs");
    let path = repository_config_path(repository.path());
    fs::create_dir_all(path.parent().expect("the repository file has a parent"))
        .expect("the repository configuration directory exists");
    fs::write(
        path,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
                 | Evidence standard | impossible | reading |\n{}\n",
            CONFIG_FENCE.begin, CONFIG_FENCE.end
        ),
    )
    .expect("the hand-edited repository file exists");

    let refusal = config_layers(&skill, Some("opencode"), Some(repository.path()))
        .expect_err("an invalid owned row was ignored");

    assert_eq!(refusal.code, "config-value-unrecognised");
}

#[test]
fn a_repository_row_may_only_tighten_what_the_machine_holds() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    install(&skill, &Config::default(), false).expect("the contract installs");
    let path = repository_config_path(repository.path());

    write_repository_rows(&path, "| Evidence standard | measuring | measuring |\n");
    let ignored = layer_repository(&Config::default(), repository.path())
        .expect("a row that loosens is ignored, not a failure to read the file");
    assert_eq!(
        ignored.evidence,
        crate::config::Evidence::Reading,
        "a repository widened the evidence standard"
    );
    let complaint = repository_complaint(&Config::default(), repository.path())
        .expect("the file reads")
        .expect("the looser row was dropped in silence");
    assert_eq!(complaint.code, crate::config::CANNOT_LOOSEN);
    assert!(
        complaint.message.contains("Evidence standard"),
        "{complaint}"
    );
    assert!(
        complaint.message.contains(&path.display().to_string()),
        "the complaint does not name the file: {complaint}"
    );
    let (tolerant, complaint) = installed_config_in_keeping_what_parses(&skill, repository.path());
    assert!(
        complaint.is_none(),
        "an ignored row was reported as a row the gate could not read"
    );
    assert_eq!(tolerant.evidence, crate::config::Evidence::Reading);

    // A tighter answer is taken, and a repository-scoped row is answered freely.
    write_repository_rows(
        &path,
        "| Corrections per lineage | 1 | 3 |\n| Issue required | on | off |\n",
    );
    let tightened = layer_repository(&Config::default(), repository.path())
        .expect("a repository tightened two rows");
    assert_eq!(tightened.corrections_per_lineage, 1);
    assert_eq!(
        tightened.issue_requirement,
        crate::config::IssueRequirement::On
    );
}

#[test]
fn a_repository_cannot_lengthen_the_renewal_window_or_drop_a_boundary_the_machine_declares() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    let machine = Config {
        boundaries: vec!["npm publish".to_owned()],
        window: std::time::Duration::from_secs(30),
        ..Config::default()
    };
    install(&skill, &machine, false).expect("the contract installs");
    let path = repository_config_path(repository.path());

    write_repository_rows(
        &path,
        "| Renewal window | 2m | 2m |\n| Irreversible commands | none | unset |\n",
    );
    let ignored = layer_repository(&machine, repository.path())
        .expect("the rows are ignored, not a failure to read the file");
    assert_eq!(ignored.window, std::time::Duration::from_secs(30));
    assert_eq!(ignored.boundaries, vec!["npm publish".to_owned()]);
    let complaint = repository_complaint(&machine, repository.path())
        .expect("the file reads")
        .expect("a looser row was dropped in silence");
    assert_eq!(complaint.code, crate::config::CANNOT_LOOSEN);

    write_repository_rows(
        &path,
        "| Renewal window | 15s | 2m |\n| Irreversible commands | npm publish, terraform apply | unset |\n",
    );
    let tightened = layer_repository(&machine, repository.path())
        .expect("a shorter window and one more boundary are tightenings");
    assert_eq!(tightened.window, std::time::Duration::from_secs(15));
    assert_eq!(
        tightened.boundaries,
        vec!["npm publish".to_owned(), "terraform apply".to_owned()]
    );
    assert!(
        repository_complaint(&machine, repository.path())
            .expect("the file reads")
            .is_none(),
        "a tightening was reported as ignored"
    );
}

#[test]
fn config_layers_bind_repository_values_and_ownership_to_one_snapshot() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join(DIRECTORY);
    let repository = tempfile::tempdir().expect("a repository");
    let replacement = tempfile::tempdir().expect("a replacement repository");
    install(&skill, &Config::default(), false).expect("the contract installs");
    let mut first = Config::default();
    crate::config::Setting::CorrectionsPerLineage
        .apply(&mut first, "1")
        .expect("the first correction limit is accepted");
    crate::setup::write_repository_configuration(
        &repository_config_path(repository.path()),
        &first,
        &[crate::config::Setting::CorrectionsPerLineage],
    )
    .expect("the first snapshot exists");
    let mut second = Config::default();
    crate::config::Setting::Tracker
        .apply(&mut second, "github acme/repo")
        .expect("the replacement tracker is accepted");
    let replacement_path = repository_config_path(replacement.path());
    crate::setup::write_repository_configuration(
        &replacement_path,
        &second,
        &[crate::config::Setting::Tracker],
    )
    .expect("the replacement snapshot exists");
    inject_repository_snapshot_replacement(
        fs::read_to_string(replacement_path).expect("the replacement snapshot reads"),
    );

    let layers = config_layers(&skill, Some("opencode"), Some(repository.path()))
        .expect("one repository snapshot supplies the layer");

    assert_eq!(
        layers.repository_settings,
        vec![crate::config::Setting::CorrectionsPerLineage]
    );
    assert_eq!(
        crate::config::Setting::CorrectionsPerLineage
            .value_of(layers.repository.as_ref().expect("repository values")),
        "1"
    );
}

/// Any answer a setting accepts other than the one it currently holds.
///
/// `None` where it offers only one: `Delivery route` accepts `direct` and
/// nothing else, so it can never demonstrate a value moving.
fn other_answer(setting: crate::config::Setting, config: &Config) -> Option<String> {
    let now = setting.value_of(config);
    setting
        .answers()
        .choices
        .iter()
        .find(|choice| **choice != now && setting.apply(&mut config.clone(), choice).is_ok())
        .map(|choice| (*choice).to_owned())
}

#[test]
fn install_then_uninstall_leaves_nothing_behind() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join("skills").join(DIRECTORY);
    let config = Config::default();

    let installed = install(&skill, &config, false).unwrap();
    assert_eq!(installed.changed_files(), FILES.len() + 1, "the record too");
    assert_eq!(presence(&skill, &config), Presence::Current);

    let removed = uninstall(&skill, false).unwrap();
    assert_eq!(removed.changed_files(), FILES.len() + 1, "the record too");
    assert!(
        !skill.exists(),
        "the skill directory came out empty and went"
    );
    assert!(
        root.path().join("skills").exists(),
        "and nothing above it moved"
    );
}

#[test]
fn uninstall_keeps_a_file_estigia_did_not_write() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(DIRECTORY);
    install(&skill, &Config::default(), false).unwrap();
    let mine = skill.join("references").join("my-own-notes.md");
    fs::write(&mine, "mine").unwrap();

    uninstall(&skill, false).unwrap();
    assert!(
        mine.exists(),
        "an operator's own file is not Estigia's to remove"
    );
}

#[test]
fn a_dry_run_writes_nothing_and_counts_everything() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(DIRECTORY);
    let planned = install(&skill, &Config::default(), true).unwrap();
    // Every shipped file, and the install record beside them: a plan that
    // leaves it out undercounts by the one file `uninstall` depends on.
    assert_eq!(planned.changed_files(), FILES.len() + 1);
    assert!(planned.actions.iter().all(|a| a.change == Change::Create));
    assert!(!skill.exists());
}

#[test]
fn a_second_install_reports_no_change() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(DIRECTORY);
    let config = Config::default();
    install(&skill, &config, false).unwrap();
    let again = install(&skill, &config, false).unwrap();
    assert_eq!(again.changed_files(), 0);
}

#[test]
fn the_list_of_repositories_drops_the_ones_that_no_longer_answer() {
    // The list exists so a checkout can be found again from anywhere. Its
    // one rule is that it says what is true **now**: a repository whose
    // file has gone — deleted, or the whole clone thrown away — is not
    // offered, because somebody who picks it gets the contract's answers
    // and believes they are that repository's.
    let home = tempfile::tempdir().expect("a home");
    let live = tempfile::tempdir().expect("a checkout that answers");
    let gone = tempfile::tempdir().expect("a checkout that will not");

    assert!(
        known_repositories(home.path()).is_empty(),
        "a machine with nothing recorded already lists something"
    );

    for repo in [live.path(), gone.path()] {
        let path = repository_config_path(repo);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "| Setting | Value |
|---|---|
",
        )
        .unwrap();
        remember_repository(home.path(), repo);
    }
    assert_eq!(
        known_repositories(home.path()).len(),
        2,
        "both were recorded, so the pruning below measures something"
    );

    // One of them stops answering for itself.
    fs::remove_file(repository_config_path(gone.path())).unwrap();
    let known = known_repositories(home.path());
    assert_eq!(known, vec![live.path().to_path_buf()], "{known:?}");

    // And recording the same checkout twice lists it once: the list is
    // read on every `--repo`, and a line per write would grow forever.
    remember_repository(home.path(), live.path());
    remember_repository(home.path(), live.path());
    assert_eq!(known_repositories(home.path()).len(), 1);
}

#[test]
fn a_repository_with_no_file_of_its_own_reads_exactly_as_before() {
    // The half that protects everybody who already has Estigia installed.
    // A layer that changed their answers would be a migration; this is an
    // addition, and an addition has to be invisible until it is used.
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(DIRECTORY);
    let repo = tempfile::tempdir().unwrap();
    let config = Config {
        evidence: crate::config::Evidence::Measuring,
        ..Config::default()
    };
    install(&skill, &config, false).unwrap();
    assert!(
        !repository_config_path(repo.path()).exists(),
        "the fixture already has a repository file, so this measures nothing"
    );
    assert_eq!(
        installed_config_in(&skill, repo.path()).unwrap(),
        installed_config(&skill).unwrap(),
        "a repository with nothing of its own answered differently"
    );
}

#[test]
fn what_a_repository_says_about_itself_wins_and_says_nothing_about_an_agent() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(DIRECTORY);
    let repo = tempfile::tempdir().unwrap();
    install(&skill, &Config::default(), false).unwrap();
    let installed = installed_config(&skill).unwrap();

    // One row about the repository, and one about an agent, in the same
    // file. The first is this repository's to answer; the second is not.
    let path = repository_config_path(repo.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "| Setting | Value |
|---|---|
| Corrections per lineage | 1 |
             | Model routing | developer=opus |
",
    )
    .unwrap();

    let layered = installed_config_in(&skill, repo.path()).unwrap();
    assert_eq!(
        layered.corrections_per_lineage, 1,
        "the repository did not get to tighten a row the whole machine answers"
    );
    assert_ne!(
        installed.corrections_per_lineage, 1,
        "the installed contract already said 1, so the line above measures nothing"
    );
    assert_ne!(
        crate::config::Setting::Models.value_of(&installed),
        "developer=opus",
        "the contract already says what the file asks for, so the line below would hold whether or not the row was put back"
    );
    assert_eq!(
        crate::config::Setting::Models.value_of(&layered),
        crate::config::Setting::Models.value_of(&installed),
        "a repository file restated a row that is not about the repository"
    );
}

#[test]
fn the_installed_configuration_reads_back() {
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join(DIRECTORY);
    let config = Config {
        evidence: crate::config::Evidence::Measuring,
        ..Config::default()
    };
    install(&skill, &config, false).unwrap();
    assert_eq!(installed_config(&skill).unwrap(), config);
}

/// A missing skill names a command that actually installs it.
///
/// It named `estigia install`, and this test asserted that spelling while
/// being called *the command that installs it*. It is not one: `install` is
/// an alias for `setup`, and `setup` with no agent refuses — *`setup` needs
/// to know which agent to configure*. So the only instruction on the screen
/// was a dead end, which the ratchet forbids more strongly than saying
/// nothing.
///
/// Both spellings, because the difference is a fact this crate has in hand:
/// asked about one agent it names that agent, and asked about none it names
/// the command that covers all of them.
#[test]
fn a_missing_skill_refuses_with_the_command_that_installs_it() {
    let root = tempfile::tempdir().unwrap();
    let refusal = installed_config(root.path()).unwrap_err();
    assert_eq!(refusal.code, "skill-not-installed");
    assert!(
        refusal.to_string().contains("run: estigia setup --all"),
        "{refusal}"
    );

    let named = installed_config_for(root.path(), Some("claude-code")).unwrap_err();
    assert_eq!(named.code, "skill-not-installed");
    assert!(
        named.to_string().contains("run: estigia setup claude-code"),
        "the agent was named and the answer was not about it: {named}"
    );
}

/// A file Estigia wrote and somebody edited is left where it is.
///
/// The removal's half of the question the record could not answer until it
/// held digests. `uninstall` took a file the operator had edited with the
/// rest, on its own `remove` line like every other and in no way marked as
/// one they had touched — the honesty contract said exactly that, and said
/// what closing it needed.
///
/// Kept rather than removed-and-marked, for the reason the guard's refusal
/// gives about a push hook: an uninstall that carries off somebody's
/// additions is the thing an operator notices about other people's software
/// and does not forgive.
///
/// Three answers, and the last two are what keep the fix from becoming a
/// refusal to uninstall anything: an untouched file still goes, and a record
/// with no digest — every one written before this existed — still goes too.
#[test]
fn a_file_somebody_edited_is_kept_by_the_uninstall() {
    let root = tempfile::tempdir().expect("a skill root");
    install(root.path(), &Config::default(), false).expect("installs");

    let mine = root.path().join(relative("references/runtime-notes.md"));
    let untouched = root
        .path()
        .join(relative("references/repository-delivery.md"));
    let held = std::fs::read_to_string(&mine).expect("the file reads");
    std::fs::write(
        &mine,
        format!(
            "{held}
## my own note
"
        ),
    )
    .expect("the operator edits it");

    uninstall(root.path(), false).expect("uninstalls");
    assert!(mine.is_file(), "a file somebody edited was taken away");
    assert!(
        std::fs::read_to_string(&mine)
            .expect("the file reads")
            .contains("my own note"),
        "the file survived and its addition did not"
    );
    assert!(
        !untouched.is_file(),
        "an untouched file survived, so the uninstall stopped uninstalling"
    );

    // A record that predates digests: every machine that upgraded into this
    // has one, and reading its silence as an edit would leave whole
    // installations behind.
    let old = tempfile::tempdir().expect("a second root");
    install(old.path(), &Config::default(), false).expect("installs");
    let path = old.path().join(relative("references/runtime-notes.md"));
    let held = std::fs::read_to_string(&path).expect("the file reads");
    std::fs::write(
        &path,
        format!(
            "{held}
## edited
"
        ),
    )
    .expect("edited");
    let ledger = old.path().join(".estigia").join("installed.json");
    let text = std::fs::read_to_string(&ledger).expect("the record reads");
    let mut value: serde_json::Value = serde_json::from_str(&text).expect("json");
    value.as_object_mut().expect("an object").remove("digests");
    std::fs::write(&ledger, value.to_string()).expect("a record without digests");

    uninstall(old.path(), false).expect("uninstalls");
    assert!(
        !path.is_file(),
        "a record with no digests left its installation behind"
    );
}

/// An uninstall takes the retired file too, or it leaves the application behind.
///
/// `uninstall_from` walks `FILES`, and a retired path is not in it — so the
/// removal walked straight past `scripts/github.py`. Measured before this
/// was written: after a full `uninstall` on a machine that had it, the file
/// was still there and so was its directory.
///
/// Worse than untidy. What survives is an **executable copy of a transport
/// the contract no longer names**, on a machine whose owner believes Estigia
/// is gone — and an uninstall that leaves part of the tool behind is the
/// thing an operator notices about somebody else's software and does not
/// forgive.
#[test]
fn an_uninstall_takes_the_retired_transport_with_it() {
    let root = tempfile::tempdir().expect("a skill root");
    install(root.path(), &Config::default(), false).expect("installs");

    // A machine that already had it: the file, and the record calling it
    // this crate's own — which is the only thing that makes it removable.
    let retired = root.path().join(relative("scripts/github.py"));
    std::fs::create_dir_all(retired.parent().expect("a directory")).expect("the directory");
    std::fs::write(
        &retired,
        "print('the old transport')
",
    )
    .expect("the file");
    record::note_created(root.path(), std::iter::once("scripts/github.py")).expect("the record");

    uninstall(root.path(), false).expect("uninstalls");
    assert!(
        !retired.is_file(),
        "{} survived an uninstall",
        retired.display()
    );

    // The floor, and the rule that governs every removal here: a file no
    // record claims is somebody else's and stays.
    let theirs = tempfile::tempdir().expect("a second root");
    install(theirs.path(), &Config::default(), false).expect("installs");
    let stranger = theirs.path().join(relative("scripts/github.py"));
    std::fs::create_dir_all(stranger.parent().expect("a directory")).expect("the directory");
    std::fs::write(
        &stranger,
        "print('somebody else')
",
    )
    .expect("the file");
    uninstall(theirs.path(), false).expect("uninstalls");
    assert!(
        stranger.is_file(),
        "a file no record claims was removed by an uninstall"
    );
}

/// The two files this round retired are taken off a machine that has
/// them, the same way the transport above is.
///
/// Round one removed `bindings/linear.md`, `bindings/trello.md` and
/// `references/migration-inventory.md` from `FILES` and added nothing to
/// `RETIRED`. `uninstall_from` walks `FILES` and then `RETIRED`
/// (`retire`, above) — a path in neither is reached by neither, so it
/// stays on every machine that ever had it: invisible to `status`,
/// untouched by `uninstall`. Confirmed red before `RETIRED` named them:
/// each iteration failed the assertion below, the file still present
/// after the call it claims removes it.
///
/// `bindings/linear.md` is no longer here: it is shipped again, so
/// `FILES` reaches it and this test's own premise would be false.
#[test]
fn the_two_paths_this_round_retired_are_taken_off_a_machine_that_has_them() {
    for path in ["bindings/trello.md", "references/migration-inventory.md"] {
        let root = tempfile::tempdir().expect("a skill root");
        install(root.path(), &Config::default(), false).expect("installs");

        let file = root.path().join(relative(path));
        std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory");
        std::fs::write(&file, "as an older build wrote it\n").expect("the file");
        record::note_created(root.path(), std::iter::once(path)).expect("the record");

        uninstall_from(root.path(), false, &mut Pending::new()).expect("uninstalls");
        assert!(!file.is_file(), "{path} survived an uninstall");
    }
}

/// Every path this crate has ever shipped, named once and never removed —
/// the crossing that let three paths fall into neither `FILES` nor
/// `RETIRED` and go unnoticed.
///
/// Grown only by appending: adding a `SkillFile` to `FILES` adds its path
/// here in the same edit, and nothing here is ever deleted. That is a
/// different action from the one this guard exists to catch — removing a
/// `FILES` entry without adding it to `RETIRED` — so forgetting the
/// append is not the same mistake as the one being guarded against.
///
/// A `git log` walk for removed `SkillFile` paths was considered instead
/// of this frozen list, and would have been more accurate — it needs no
/// upkeep of its own. It was not chosen: this crate is published
/// (`Cargo.toml`'s `include` list exists because of it), and `cargo test`
/// against a package extracted from a tarball has no `.git` at all, so
/// that guard would hard-fail outside a checkout rather than measure
/// anything. `a_linked_worktree_keeps_its_answers_with_the_repository`,
/// above, gives up the same accuracy for the same reason: built by hand so
/// it measures on every machine and not only where git is installed.
const EVER_SHIPPED: &[&str] = &[
    CONTRACT,
    "bindings/github.md",
    "bindings/github/board.md",
    "bindings/github/delivery.md",
    "bindings/github/operations.md",
    "bindings/github/operations-guarantees.md",
    "bindings/github/recovery.md",
    "bindings/github/refusals.md",
    "bindings/github/setup.md",
    "references/analyst.md",
    "references/decision-gates.md",
    "references/domain-composition.md",
    "references/repository-delivery.md",
    "policies/blind-judges.md",
    "policies/judgment-day.md",
    "protocols/rdd.md",
    "protocols/sdd.md",
    "references/runtime-notes.md",
    "references/safety-incidents.md",
    "assets/analyst-issue-template.md",
    "scripts/github.py",
    "bindings/linear.md",
    "bindings/trello.md",
    "references/migration-inventory.md",
    "agents/sdd-explore.md",
    "agents/sdd-propose.md",
    "agents/sdd-spec.md",
    "agents/sdd-design.md",
    "agents/sdd-tasks.md",
    "agents/review-blind.md",
    "agents/review-blind-sealed.md",
    "agents/review-lens.md",
    "agents/review-risk.md",
    "agents/review-readability.md",
    "agents/review-reliability.md",
    "agents/review-readback.md",
    "agents/review-refuter.md",
    "agents/review-validator.md",
    "agents/developer.md",
    "agents/analyst.md",
    "agents/implementer.md",
];

/// Whether a definition path is one a current list installs or a retirement
/// list takes away.
///
/// Deliberately not `AGENT_DEFINITIONS`: that is the lifecycle hash's
/// collection, not a road `setup` or `uninstall` walks, so reading it here
/// would answer "reachable" about a definition no walk of it ever reaches.
/// The roads are the install list, the two retirement lists, and the
/// reviewer's own reserved names — which carry the historical-rendering
/// ledger rather than a name in `RETIRED_AGENTS`.
fn definition_is_reached(path: &str) -> bool {
    DELEGATED_AGENTS.iter().any(|(_, file)| file.path == path)
        || RETIRED_PHASE_AGENTS
            .iter()
            .any(|name| format!("agents/{name}") == path)
        || RETIRED_AGENTS
            .iter()
            .any(|name| format!("agents/{name}") == path)
        || crate::harness::roles::RESERVED_REVIEWERS
            .iter()
            .any(|(name, _, _)| format!("agents/{name}.md") == path)
}

/// A path this crate has ever shipped is either still shipped or named on
/// the way out — never simply gone from both.
///
/// Confirmed red against the state this round left: with the three paths
/// above absent from `RETIRED`, this failed naming all three, because
/// `install_into` and `uninstall_from` were reaching neither.
#[test]
fn every_path_this_crate_has_ever_shipped_is_reachable_or_retired() {
    for path in EVER_SHIPPED {
        assert!(
            FILES.iter().any(|file| file.path == *path)
                || RETIRED.contains(path)
                || definition_is_reached(path),
            "{path} was shipped once and is in neither a current nor a retirement list"
        );
    }
}

/// The crossing above only reaches a path that is named here, so one this
/// crate ships now and forgets to append is invisible to it — the direction
/// it leaves open. Both collections: the skill files and the agent
/// definitions.
#[test]
fn every_path_this_crate_ships_now_is_named_as_ever_shipped() {
    for path in FILES
        .iter()
        .map(|file| file.path)
        .chain(AGENT_DEFINITIONS.iter().map(|file| file.path))
    {
        assert!(
            EVER_SHIPPED.contains(&path),
            "{path} ships now and is not in EVER_SHIPPED"
        );
    }
}

/// Line endings are not an edit.
///
/// The digest that keeps somebody's work has to answer the same question the
/// install already answers with `normalize(found) == normalize(desired)`, or
/// it is a second rule about one thing. It was: hashed over raw bytes, a
/// file converted to CRLF read as edited.
///
/// Not exotic on this crate's own platform — `core.autocrlf` and half the
/// editors rewrite line endings without anybody editing anything — and it
/// failed in the expensive direction: the uninstall **kept** files nobody
/// had touched, so a fix against leaving somebody's work behind began
/// leaving the application behind.
///
/// Both halves: a rewrapped file is the same file, and a changed one is not.
#[test]
fn changing_the_line_endings_is_not_editing_the_file() {
    let lines = ["# a note", "", "with two lines", ""];
    let lf = lines.join("\n");
    let crlf = lines.join("\r\n");
    // The fixture first: two spellings, and they have to be two. Written
    // with both joins the same, this compared a string with itself and
    // passed with the normalisation taken out — a test measuring nothing.
    assert_ne!(lf, crlf, "the two spellings are the same string");
    assert_eq!(
        record::digest_of(&lf),
        record::digest_of(&crlf),
        "a file rewrapped to CRLF read as one somebody had edited"
    );
    assert_ne!(
        record::digest_of(&lf),
        record::digest_of(&["# a note", "", "with three lines", ""].join("\n")),
        "the digest stopped telling two different files apart"
    );
}

/// The directory a retired file lived in goes with it.
///
/// `remove_empty_directories` builds its candidates from `FILES` — what this
/// crate ships **now** — and `RETIRED` is by definition not in that list. So
/// `scripts/github.py` was taken and `scripts/` stayed, empty, in a skill
/// directory whose owner had just been told the application was gone.
///
/// The comment above the retirement loop in `uninstall_from` already claims
/// otherwise: *"after `uninstall`, the retired `scripts/github.py` was still
/// there, and so was the directory holding it"*. The file was fixed. The
/// sentence went on naming both.
///
/// Measured on the real binary before this was written: a `sync` over an
/// install carrying the retired transport printed one `remove` line and left
/// `issue-flow/scripts/` behind, and a full `uninstall` left it too — the
/// only empty directory in the tree, and the only leftover the uninstall's
/// own note does not account for.
///
/// Both halves, because the second is what makes the first safe: an empty
/// directory is taken, and a directory with anything at all in it is not.
#[test]
fn the_directory_a_retired_file_lived_in_goes_with_it() {
    for (name, theirs) in [
        ("scripts/gone.py", None),
        ("scripts/kept.py", Some("mine.py")),
    ] {
        let root = tempfile::tempdir().expect("a skill root");
        let config = crate::config::Config::default();
        install(root.path(), &config, false).expect("the skill installs");

        let file = root.path().join(relative(name));
        let directory = file.parent().expect("a directory").to_path_buf();
        std::fs::create_dir_all(&directory).expect("the directory");
        let body = "print('as this crate wrote it')\n";
        std::fs::write(&file, body).expect("the file");
        record::note_written(
            root.path(),
            std::iter::once((name, record::digest_of(body))),
        )
        .expect("the record");
        if let Some(theirs) = theirs {
            std::fs::write(directory.join(theirs), "# mine\n").expect("their file");
        }

        let created: std::collections::BTreeSet<String> =
            std::iter::once(name.to_owned()).collect();
        let taken = retire(root.path(), &[name], &created, &mut Pending::new(), false)
            .expect("the retirement runs");

        // The floor: the retirement did what this is about to judge it on.
        assert_eq!(taken.len(), 1, "{name}: nothing was retired");
        assert_eq!(taken[0].change, Change::Remove, "{name}");
        assert!(!file.is_file(), "{name}: the retired file was kept");

        match theirs {
            None => assert!(
                !directory.exists(),
                "{name}: the directory Estigia made to hold one file is empty and still there"
            ),
            Some(theirs) => {
                assert!(
                    directory.join(theirs).is_file(),
                    "{name}: an operator's file went with the sweep"
                );
                assert!(
                    directory.is_dir(),
                    "{name}: a directory holding somebody's work was removed"
                );
            }
        }
    }
}

/// A machine one release behind is not accused of writing over somebody.
///
/// `OVERWRITE` is the loud word: it says an edit of yours has just been
/// discarded. It is earned by a file whose contents have **moved since
/// Estigia wrote them** — and the same branch decides `Update`, which is
/// bookkeeping and says nothing about anybody's work.
///
/// Both halves of that were unmeasured. Replacing the digest comparison
/// with *the record has no digest at all* left the whole suite green, and
/// under it every stale file on every ordinary upgrade would have been
/// reported as an overwrite of the operator's work — the false alarm this
/// branch's own comment says it exists to avoid, and the sentence the
/// README makes about a machine one release behind.
///
/// Two fixtures, and they differ only in the record: a file Estigia wrote
/// and an older build's content, both stale against this build's copy. The
/// one whose digest still matches what was written is `Update`; the one
/// whose contents moved since is `Overwrite`.
#[test]
fn a_stale_file_nobody_edited_is_an_update_and_not_an_overwrite() {
    for (edited, wanted) in [(false, Change::Update), (true, Change::Overwrite)] {
        let root = tempfile::tempdir().expect("a skill root");
        let config = crate::config::Config::default();
        install(root.path(), &config, false).expect("the skill installs");

        // An older build's copy of a payload file: different from what this
        // build ships, so the install has something to do.
        let path = "references/runtime-notes.md";
        let file = root.path().join(relative(path));
        let older = "# Runtime notes\n\nas an older build wrote them\n";
        std::fs::write(&file, older).expect("the older copy");
        // And the record says so — this is the machine one release behind.
        record::note_written(
            root.path(),
            std::iter::once((path, record::digest_of(older))),
        )
        .expect("the record");

        if edited {
            // Their edit, after the record was written.
            std::fs::write(&file, format!("{older}\nand a line of mine\n")).expect("their edit");
        }

        let plan = install(root.path(), &config, true).expect("the plan");
        let action = plan
            .actions
            .iter()
            .find(|action| action.path.ends_with(relative(path)))
            .unwrap_or_else(|| panic!("the plan does not mention {path}"));
        assert_eq!(
            action.change,
            wanted,
            "a stale file {} was reported as {:?}",
            if edited {
                "somebody edited"
            } else {
                "nobody touched"
            },
            action.change
        );
    }
}
/// A retired file somebody patched is left where it is.
///
/// The **third** removal path, and it read the record by name while the
/// install and the uninstall had both learned to read the digest. Measured:
/// a `scripts/github.py` an operator had patched was taken by the next
/// `sync`, on a plain `remove` line.
///
/// The tension here is real and the answer is deliberate. A retired file is
/// one this crate asked to be gone, and an executable copy of an old
/// transport left beside a contract that no longer names it is the hazard
/// retirement exists to end. It is kept anyway: taking somebody's work is
/// the worse of the two failures, the file is theirs now, and nothing this
/// crate runs goes looking for it.
#[test]
fn a_retired_file_somebody_patched_is_kept() {
    let root = tempfile::tempdir().expect("a skill root");
    let config = crate::config::Config::default();
    install(root.path(), &config, false).expect("the skill installs");

    let path = "scripts/retired-by-a-test.py";
    let file = root.path().join(relative(path));
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory");
    std::fs::write(
        &file,
        "print('as this crate wrote it')
",
    )
    .expect("the file");
    let created: std::collections::BTreeSet<String> = std::iter::once(path.to_owned()).collect();
    record::note_written(
        root.path(),
        std::iter::once((
            path,
            record::digest_of(
                "print('as this crate wrote it')
",
            ),
        )),
    )
    .expect("the record");

    // Untouched: taken, or the fix is a refusal to retire anything.
    let taken = retire(root.path(), &[path], &created, &mut Pending::new(), false)
        .expect("the retirement runs");
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].change, Change::Remove);
    assert!(!file.is_file(), "an untouched retired file was kept");

    // Patched: kept, and named on its own line so nothing is left silently.
    //
    // The directory is remade first: the removal above took it, because the
    // file was the whole of it. That is the behaviour
    // `the_directory_a_retired_file_lived_in_goes_with_it` is for, and this
    // fixture would otherwise fail on the write rather than on the claim.
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory again");
    std::fs::write(
        &file,
        "print('patched by the operator')
",
    )
    .expect("the file");
    let kept = retire(root.path(), &[path], &created, &mut Pending::new(), false)
        .expect("the retirement runs");
    assert_eq!(kept.len(), 1, "a kept file was not named");
    assert_eq!(kept[0].change, Change::Kept);
    assert!(
        std::fs::read_to_string(&file)
            .expect("the file reads")
            .contains("patched by the operator"),
        "a patched retired file was taken with the rest"
    );
}

/// A file this crate used to ship is taken away — and only that file.
///
/// `uninstall_from` walks `FILES`, so a path deleted from that list stops being
/// anybody's to remove: it stays on every machine that ever had it, invisible to
/// `status` and untouched by `uninstall`. For the transport that is not
/// untidiness — a `scripts/github.py` left beside a contract that no longer
/// names it is the second implementation coming back, and two implementations
/// cost exactly what they disagree about.
///
/// Three properties, and the middle one is what keeps this from becoming a
/// cleanup that eats somebody's work: a retired path is removed **only** where
/// the install record says Estigia created it.
#[test]
fn a_retired_file_is_taken_away_and_a_stranger_s_is_not() {
    let root = tempfile::tempdir().expect("a skill root");
    let config = crate::config::Config::default();
    super::install(root.path(), &config, false).expect("the skill installs");

    // A path this crate's record claims, and one it does not, side by side in
    // one root — so the difference between them is the record and nothing else.
    let mine = "scripts/retired-by-a-test.py";
    let theirs = "scripts/somebody-elses.py";
    for path in [mine, theirs] {
        let file = root.path().join(relative(path));
        std::fs::create_dir_all(file.parent().expect("a directory")).expect("the directory");
        std::fs::write(
            &file,
            "print('hello')
",
        )
        .expect("the file");
    }
    let created: std::collections::BTreeSet<String> = std::iter::once(mine.to_owned()).collect();

    let taken = retire(
        root.path(),
        &[mine, theirs],
        &created,
        &mut Pending::new(),
        false,
    )
    .expect("the retirement runs");

    assert_eq!(taken.len(), 1, "one file was retired, not {}", taken.len());
    assert!(
        taken[0].path.ends_with("retired-by-a-test.py"),
        "the wrong file was taken: {:?}",
        taken[0].path
    );
    assert_eq!(taken[0].change, Change::Remove);
    assert!(
        !root.path().join(relative(mine)).is_file(),
        "the retired file is still there"
    );
    assert!(
        root.path().join(relative(theirs)).is_file(),
        "a file no record claims was removed anyway"
    );

    // And the floor on the list: a path nobody retired is left alone, however
    // plainly this crate's own it is.
    //
    // Written back first. Asked against the directory the call above left,
    // the file is already gone and an empty list agrees with a list that
    // retires everything — measured, by making the empty list retire this
    // very path: the assertion below passed.
    std::fs::write(
        root.path().join(relative(mine)),
        "print('hello')
",
    )
    .expect("the file");
    let none = retire(root.path(), &[], &created, &mut Pending::new(), false)
        .expect("an empty retirement runs");
    assert!(none.is_empty(), "a path nobody retired was removed anyway");
}

#[test]
fn an_asset_digest_is_deterministic_and_shaped() {
    let first = super::assets_digest();
    assert_eq!(first, super::assets_digest(), "same sources must agree");
    assert!(
        first.starts_with("sha256:") && first.len() == 7 + 64,
        "unexpected digest shape: {first:?}"
    );
}

#[test]
fn an_asset_digest_moves_with_any_byte_and_no_metadata() {
    let base = [("b.md", "x"), ("a.md", "y")];
    let reference = super::assets_digest_of(&base);
    // One byte of content.
    assert_ne!(
        super::assets_digest_of(&[("b.md", "x"), ("a.md", "z")]),
        reference,
        "a changed byte must move the digest"
    );
    // The path is hashed too, not only the content.
    assert_ne!(
        super::assets_digest_of(&[("c.md", "x"), ("a.md", "y")]),
        reference,
        "a moved path must move the digest"
    );
    // Order is not an input: sorting is what makes two checkouts agree.
    assert_eq!(
        super::assets_digest_of(&[("a.md", "y"), ("b.md", "x")]),
        reference,
        "order must not move the digest"
    );
    // Line endings are normalised, the way installs compare files.
    assert_eq!(
        super::assets_digest_of(&[("b.md", "x"), ("a.md", "y\r\n")]),
        super::assets_digest_of(&[("b.md", "x"), ("a.md", "y\n")]),
        "line endings must not move the digest"
    );
}

#[test]
fn the_editor_reads_only_the_repository_scoped_rows_of_a_checkout() {
    let (_repository, path) =
        repository_with_rows("| Claim workflow | assisted |\n| Change size | 5 |\n");
    let base = crate::config::Config::default();
    let edited = super::repository_snapshot(&path)
        .expect("a readable file")
        .expect("a repository layer")
        .edit_scope_over(&base);
    let size = crate::config::Setting::ChangeSize;
    assert_eq!(edited.workflow, crate::config::Workflow::Assisted);
    assert_eq!(size.value_of(&edited), size.value_of(&base));
}

#[test]
fn the_editor_opens_over_a_repository_row_that_does_not_parse() {
    let (_repository, path) =
        repository_with_rows("| Issue required | sideways |\n| Claim workflow | assisted |\n");
    let edited = super::repository_snapshot(&path)
        .expect("a readable file")
        .expect("a repository layer")
        .edit_scope_over(&crate::config::Config::default());
    assert_eq!(edited.workflow, crate::config::Workflow::Assisted);
}

#[test]
fn no_shipped_agent_definition_names_this_repository() {
    for file in crate::skill::AGENT_DEFINITIONS {
        for needle in ["src/test_env.rs", "CLAUDE.md", "this crate"] {
            assert!(
                !file.contents.contains(needle),
                "{} names this repository's {needle:?}; a definition ships to every repository",
                file.path
            );
        }
    }
}
