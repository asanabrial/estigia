use super::*;
use crate::test_env::{with_config_home, with_process_env};

/// The real thing, copied from a published `agents/builder.md`.
const BUILDER: &str = "---\n\
    name: builder\n\
    description: Turns approved implementation plans into production-ready code.\n\
    tools: Read, Write, Edit, Glob, Grep, Bash\n\
    model: inherit\n\
    maxTurns: 100\n\
    ---\n\n\
    # The Builder\n\n\
    Take an approved plan and ship the code.\n";

fn install_canonical_reviewer(home: &std::path::Path) {
    let file = home.join(".claude/agents/review-blind.md");
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the user agents directory");
    // The default rendering, because that is what a default install writes. The
    // grant is derived from the evidence standard, so the raw
    // asset is a template and no installed file ever holds it.
    std::fs::write(
        file,
        crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            crate::config::Evidence::Reading,
            crate::config::Seal::Unsealed,
            RESERVED_REVIEWER,
        ),
    )
    .expect("the canonical reviewer");
}

fn install_canonical_sealed_reviewer(home: &std::path::Path) {
    let file = home.join(".claude/agents/review-blind-sealed.md");
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the user agents directory");
    std::fs::write(
        file,
        crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            crate::config::Evidence::Reading,
            crate::config::Seal::Sealed,
            RESERVED_REVIEWER_SEALED,
        ),
    )
    .expect("the canonical sealed reviewer");
}

fn project_reviewer(project: &std::path::Path, relative: &str) -> std::path::PathBuf {
    let file = project.join(".claude/agents").join(relative);
    std::fs::create_dir_all(file.parent().expect("a parent"))
        .expect("the project agents directory");
    std::fs::write(
        &file,
        "---\nname: review-blind\ntools: Read, Write, Bash\n---\nHostile.\n",
    )
    .expect("a project reviewer");
    file
}

#[test]
fn every_seal_has_a_reserved_name() {
    for seal in crate::config::Seal::all() {
        assert!(
            RESERVED_REVIEWERS
                .iter()
                .any(|&(_, reserved, _)| reserved == seal),
            "{seal:?} has no entry in RESERVED_REVIEWERS"
        );
    }
}

#[test]
fn reserved_reviewer_launch_rejects_exact_and_nested_renamed_project_shadows() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    install_canonical_reviewer(home.path());

    let exact = project_reviewer(project.path(), "review-blind.md");
    let refusal =
        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
            .expect_err("the exact project shadow launched");
    assert_eq!(refusal.code, "reviewer-project-shadow");
    assert!(matches!(refusal.resolution, Resolution::NoCommand { .. }));
    std::fs::remove_file(exact).expect("the exact shadow is removed");

    project_reviewer(project.path(), "nested/renamed.md");
    let refusal =
        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
            .expect_err("the nested renamed shadow launched");
    assert_eq!(refusal.code, "reviewer-project-shadow");
}

#[test]
fn reserved_reviewer_launch_scans_from_the_cwd_through_the_repository_root() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    install_canonical_reviewer(home.path());
    std::fs::create_dir(project.path().join(".git")).expect("a repository marker");
    let nested = project.path().join("src/deep");
    std::fs::create_dir_all(&nested).expect("a nested launch directory");
    project_reviewer(project.path(), "root.md");

    let refusal = authorize_review_blind_launch(&nested, Some(home.path()), RESERVED_REVIEWER)
        .expect_err("changing cwd hid the project reviewer");
    assert_eq!(refusal.code, "reviewer-project-shadow");
}

#[test]
fn valid_yaml_key_spellings_cannot_hide_a_reserved_project_reviewer() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    install_canonical_reviewer(home.path());
    let agents = project.path().join(".claude/agents");
    std::fs::create_dir_all(&agents).expect("the project agents directory");

    for (file, declaration) in [
        ("spaced.md", "name : review-blind"),
        ("quoted.md", "\"name\": review-blind"),
    ] {
        let candidate = agents.join(file);
        std::fs::write(
            &candidate,
            format!("---\n{declaration}\ntools: Read\n---\nHostile.\n"),
        )
        .expect("a valid YAML project reviewer");
        let refusal =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
                .expect_err("a valid YAML spelling hid the project reviewer");
        assert_eq!(refusal.code, "reviewer-project-shadow");
        std::fs::remove_file(candidate).expect("the candidate is removed");
    }
}

#[test]
fn unprovable_or_duplicate_project_reviewer_candidates_fail_closed() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    install_canonical_reviewer(home.path());
    let agents = project.path().join(".claude/agents");
    std::fs::create_dir_all(&agents).expect("the project agents directory");

    std::fs::write(agents.join("unknown.md"), [0xff, 0xfe]).expect("unreadable text");
    let refusal =
        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
            .expect_err("an unreadable candidate was clearance");
    assert_eq!(refusal.code, "reviewer-project-unprovable");
    std::fs::remove_file(agents.join("unknown.md")).expect("the obstacle is removed");

    std::fs::write(
        agents.join("ambiguous.md"),
        "---\nname: *operator_owned\n---\n",
    )
    .expect("an ambiguous name");
    let refusal =
        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
            .expect_err("an ambiguous candidate was clearance");
    assert_eq!(refusal.code, "reviewer-project-unprovable");
    std::fs::remove_file(agents.join("ambiguous.md")).expect("the ambiguity is removed");

    project_reviewer(project.path(), "one.md");
    project_reviewer(project.path(), "nested/two.md");
    let refusal =
        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
            .expect_err("duplicate project candidates were accepted");
    assert_eq!(refusal.code, "reviewer-project-unprovable");
}

// These tests prove the *default* canonical-reviewer road, and
// `authorize_review_blind_launch` now reads `CLAUDE_CONFIG_DIR` first —
// cleared here the same way `with_home_env` already clears it for `setup`'s
// own tests, so an operator's own relocated shell cannot stand in for `home`.
#[test]
fn canonical_user_reviewer_must_exist_read_and_match() {
    with_process_env("CLAUDE_CONFIG_DIR", "", || {
        let project = tempfile::tempdir().expect("a project");
        let home = tempfile::tempdir().expect("a home");
        let canonical = home.path().join(".claude/agents/review-blind.md");

        let missing =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
                .expect_err("a missing canonical reviewer launched");
        assert_eq!(missing.code, "reviewer-canonical-unavailable");

        std::fs::create_dir_all(&canonical).expect("an unreadable canonical path");
        let unreadable =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
                .expect_err("an unreadable canonical reviewer launched");
        assert_eq!(unreadable.code, "reviewer-canonical-unavailable");
        std::fs::remove_dir(&canonical).expect("the obstacle is removed");

        std::fs::write(
            &canonical,
            "---\nname: review-blind\ntools: Read, Write\n---\n",
        )
        .expect("a changed canonical reviewer");
        let changed =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
                .expect_err("a changed canonical reviewer launched");
        assert_eq!(changed.code, "reviewer-canonical-unavailable");
        assert!(
            changed.to_string().contains("estigia setup --all"),
            "a stale definition refusal must name the command that clears it: {changed}"
        );

        std::fs::write(
            &canonical,
            crate::setup::render_reviewer_agent(
                crate::setup::REVIEWER_TEMPLATE_BEFORE_CONTRACT,
                crate::config::Evidence::Reading,
                crate::config::Seal::Unsealed,
                RESERVED_REVIEWER,
            ),
        )
        .expect("a historical canonical reviewer");
        let historical =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
                .expect_err("a historical canonical reviewer launched");
        assert_eq!(historical.code, "reviewer-canonical-unavailable");
        assert!(
            historical.to_string().contains("estigia setup --all"),
            "a historical definition refusal must name estigia setup --all: {historical}"
        );

        std::fs::write(
            &canonical,
            crate::setup::render_reviewer_agent(
                crate::skill::REVIEW_AGENT.contents,
                crate::config::Evidence::Reading,
                crate::config::Seal::Unsealed,
                RESERVED_REVIEWER,
            ),
        )
        .expect("the canonical reviewer is restored");
        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
            .expect("the canonical reviewer with no collision proceeds");
    });
}

#[test]
fn canonical_user_reviewer_must_be_the_unique_reserved_definition() {
    with_process_env("CLAUDE_CONFIG_DIR", "", || {
        let project = tempfile::tempdir().expect("a project");
        let home = tempfile::tempdir().expect("a home");
        install_canonical_reviewer(home.path());
        let duplicate = home.path().join(".claude/agents/nested/other.md");
        std::fs::create_dir_all(duplicate.parent().expect("a parent"))
            .expect("the nested user agent directory");
        std::fs::write(
            duplicate,
            "---\n\"name\": review-blind\ntools: Read\n---\nHostile.\n",
        )
        .expect("a duplicate user reviewer");

        let refusal =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER)
                .expect_err("a duplicate user reviewer launched");
        assert_eq!(refusal.code, "reviewer-canonical-unavailable");
    });
}

#[test]
fn the_binary_launched_names_are_reserved_and_the_installed_ones_are_the_judges() {
    for name in BINARY_LAUNCHED_REVIEWERS {
        assert!(reserved_reviewer(name).is_some(), "{name} is not reserved");
    }
    for &(name, _, template) in RESERVED_REVIEWERS {
        assert_eq!(
            !binary_launched_reviewer(name),
            template == crate::skill::REVIEW_AGENT.contents,
            "{name}: installed rows are exactly the judge template's"
        );
    }
}

#[test]
fn a_hand_launch_of_a_binary_launched_reviewer_is_refused_without_naming_a_setup_command() {
    with_process_env("CLAUDE_CONFIG_DIR", "", || {
        let project = tempfile::tempdir().expect("a project");
        let home = tempfile::tempdir().expect("a home");
        for name in [
            RESERVED_LENS,
            RESERVED_RISK,
            RESERVED_READABILITY,
            RESERVED_RELIABILITY,
            RESERVED_READBACK,
            RESERVED_REFUTER,
            RESERVED_VALIDATOR,
        ] {
            let refusal = authorize_review_blind_launch(project.path(), Some(home.path()), name)
                .expect_err("a hand launch with no installed file proceeded");
            let shown = refusal.to_string();
            assert!(
                shown.contains("launch_review_input") && !shown.contains("estigia setup"),
                "{name}: {shown}"
            );
            assert!(matches!(refusal.resolution, Resolution::NoCommand { .. }));

            let canonical = home.path().join(format!(".claude/agents/{name}.md"));
            std::fs::create_dir_all(canonical.parent().expect("a parent"))
                .expect("the user agents directory");
            std::fs::write(
                &canonical,
                reserved_reviewer_template(name).expect("a template"),
            )
            .expect("a leftover definition");
            let refusal = authorize_review_blind_launch(project.path(), Some(home.path()), name)
                .expect_err("a leftover definition made a hand launch proceed");
            assert!(!refusal.to_string().contains("estigia setup"), "{name}");
            std::fs::remove_file(canonical).expect("the leftover is removed");
        }
    });
}

#[test]
fn a_project_definition_still_shadows_a_binary_launched_reviewer() {
    with_process_env("CLAUDE_CONFIG_DIR", "", || {
        let project = tempfile::tempdir().expect("a project");
        let home = tempfile::tempdir().expect("a home");
        let agents = project.path().join(".claude/agents");
        std::fs::create_dir_all(&agents).expect("the project agents directory");
        std::fs::write(
            agents.join("review-risk.md"),
            "---\nname: review-risk\ntools: Read, Write, Bash\n---\nHostile.\n",
        )
        .expect("a project shadow");
        let refusal =
            authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_RISK)
                .expect_err("a project shadow of a binary-launched reviewer launched");
        assert_eq!(refusal.code, "reviewer-project-shadow");
    });
}

/// The prelaunch proof covers the sealed name too, at its own canonical path:
/// launching it succeeds when it alone reserves the name, and a duplicate
/// reserving `review-blind-sealed` refuses exactly as one reserving
/// `review-blind` does above.
#[test]
fn canonical_sealed_reviewer_must_be_the_unique_reserved_definition() {
    with_process_env("CLAUDE_CONFIG_DIR", "", || {
        let project = tempfile::tempdir().expect("a project");
        let home = tempfile::tempdir().expect("a home");
        install_canonical_sealed_reviewer(home.path());

        authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER_SEALED)
            .expect("the sealed canonical reviewer with no collision was refused");

        let duplicate = home.path().join(".claude/agents/nested/other.md");
        std::fs::create_dir_all(duplicate.parent().expect("a parent"))
            .expect("the nested user agent directory");
        std::fs::write(
            &duplicate,
            "---\n\"name\": review-blind-sealed\ntools: Read\n---\nHostile.\n",
        )
        .expect("a duplicate sealed user reviewer");

        let refusal = authorize_review_blind_launch(
            project.path(),
            Some(home.path()),
            RESERVED_REVIEWER_SEALED,
        )
        .expect_err("a duplicate sealed user reviewer launched");
        assert_eq!(refusal.code, "reviewer-canonical-unavailable");
    });
}

#[test]
fn a_sealed_path_holding_the_unsealed_rendering_does_not_pass() {
    with_process_env("CLAUDE_CONFIG_DIR", "", || {
        let project = tempfile::tempdir().expect("a project");
        let home = tempfile::tempdir().expect("a home");
        let file = home.path().join(".claude/agents/review-blind-sealed.md");
        std::fs::create_dir_all(file.parent().expect("a parent"))
            .expect("the user agents directory");
        std::fs::write(
            &file,
            crate::setup::render_reviewer_agent(
                crate::skill::REVIEW_AGENT.contents,
                crate::config::Evidence::Measuring,
                crate::config::Seal::Unsealed,
                RESERVED_REVIEWER,
            ),
        )
        .expect("the mislabelled reviewer");

        let refusal = authorize_review_blind_launch(
            project.path(),
            Some(home.path()),
            RESERVED_REVIEWER_SEALED,
        )
        .expect_err("the unsealed rendering passed under the sealed path");
        assert_eq!(refusal.code, "reviewer-canonical-unavailable");
    });
}

/// The tools one definition declares, through the reader the harness runs.
///
/// This used to be `declared_tools`, a second parser nothing outside these
/// tests called — inline commas only, no brackets, no block form. Four tests
/// held it, this module's header pointed at it for the rule, and the gate ran
/// `declared_policy` instead. So the list somebody else wrote was being
/// certified against a stand-in for the thing that reads it, and the block-form
/// case below is one the stand-in answers `None` to.
fn allowlist(definition: &str) -> Option<Vec<String>> {
    match declared_policy(definition) {
        Some(Policy::Allowlist(tools)) => Some(tools),
        _ => None,
    }
}

#[test]
fn the_list_is_read_from_the_definition_somebody_else_wrote() {
    let tools = allowlist(BUILDER).expect("builder declares its tools");
    assert_eq!(tools, ["Read", "Write", "Edit", "Glob", "Grep", "Bash"]);

    // The same six, written the way YAML writes a list. One reader answers both
    // spellings or the tests above are about a dialect rather than about the
    // rule — and this is the case that told the two readers apart.
    let block = BUILDER.replace(
        "tools: Read, Write, Edit, Glob, Grep, Bash\n",
        "tools:\n  - Read\n  - Write\n  - Edit\n  - Glob\n  - Grep\n  - Bash\n",
    );
    assert_ne!(
        block, BUILDER,
        "the fixture did not change, so it poses nothing"
    );
    assert_eq!(
        allowlist(&block).expect("the block form is a list"),
        tools,
        "the same list in the other YAML spelling read differently"
    );

    // And a comment among the entries is not the end of them. Unindented is the
    // case that mattered: it ended the block, the list came out empty, and empty
    // is `None` — no policy, every tool permitted. `permission_rules` had
    // already learned this for the block beside it and nothing carried it here.
    for comment in ["  # what it needs\n", "# what it needs\n"] {
        let annotated = block.replace("tools:\n", &format!("tools:\n{comment}"));
        assert_ne!(annotated, block, "the fixture did not change");
        assert_eq!(
            allowlist(&annotated).unwrap_or_default(),
            tools,
            "a {} comment among the entries dropped the list",
            if comment.starts_with(' ') {
                "indented"
            } else {
                "column-zero"
            }
        );
    }

    // The block still *ends*, or this reads the rest of the frontmatter as
    // tools — a fix that never stopped would be the same defect widening.
    let after = block.replace("model: inherit\n", "model: inherit\n  - Sneaked\n");
    assert!(
        !allowlist(&after)
            .unwrap_or_default()
            .iter()
            .any(|tool| tool == "Sneaked"),
        "the list swallowed an entry under a later key"
    );

    // Other keys are not the list, however much they look like one.
    assert!(may_use(&tools, "Bash"));
    assert!(!may_use(&tools, "inherit"));
    assert!(!may_use(&tools, "builder"));

    // Case-insensitive, because the two ends are written by different people:
    // the host sends the tool name and a human typed the allowlist.
    assert!(may_use(&tools, "bash"));
    assert!(may_use(&tools, "  Edit  "));
    assert!(!may_use(&tools, "WebFetch"));
}

#[test]
fn not_being_told_is_not_an_empty_allowlist() {
    // Three different ways of not knowing, none of which may become "refuse
    // everything" — that would stop an agent behaving perfectly.
    let no_frontmatter = "# The Builder\n\nTake an approved plan.\n";
    let unterminated = "---\nname: builder\ntools: Read, Write\n";
    let no_tools_key = "---\nname: builder\nmodel: inherit\n---\n\n# The Builder\n";
    let empty_list = "---\nname: builder\ntools:\n---\n\n# The Builder\n";
    // The fifth shape, and the one `declared_policy`'s own note calls out by
    // name: *an explicitly empty list — `tools: []` — is a fifth thing, and the
    // only one that is genuinely distinguishable. It is deliberately **not**
    // distinguished.* The decision was written down and the shape it is about
    // was the one this population left out, so nothing held the decision to it:
    // a later reader who "fixes" the parser into a total denial breaks an agent
    // that is behaving perfectly, and no test moves.
    let brackets = "---\nname: builder\ntools: []\n---\n\n# The Builder\n";
    let spaced = "---\nname: builder\ntools: [ ]\n---\n\n# The Builder\n";

    for definition in [
        no_frontmatter,
        unterminated,
        no_tools_key,
        empty_list,
        brackets,
        spaced,
    ] {
        assert_eq!(
            allowlist(definition),
            None,
            "{definition:?} was read as a declared list"
        );
        // And the gate stands aside on every one of them.
        assert!(
            gate(Some("builder"), "Bash", Some(definition)).is_none(),
            "an undeclared list refused a call"
        );
    }
}

#[test]
fn a_call_past_the_declared_list_is_refused_and_says_whose_list_it_is() {
    let refusal = gate(Some("builder"), "WebFetch", Some(BUILDER))
        .expect("WebFetch is not on the builder's list");
    assert_eq!(refusal.code, "tool-outside-declared-role");

    let message = format!("{refusal}");
    // Names the role, the tool, and what the role *does* have — a refusal that
    // said only "not allowed" would send somebody to read the file this
    // already read.
    assert!(message.contains("builder"), "{message}");
    assert!(message.contains("WebFetch"), "{message}");
    assert!(message.contains("Read"), "{message}");
    // And the way out is the author's file, not an exception from Estigia: the
    // list is theirs and this only holds it.
    assert!(message.contains("definition"), "{message}");

    // Everything on the list still passes.
    for tool in ["Read", "Write", "Edit", "Glob", "Grep", "Bash"] {
        assert!(
            gate(Some("builder"), tool, Some(BUILDER)).is_none(),
            "{tool} is declared and was refused"
        );
    }
}

/// The reserved reviewer's resolution names an action that clears the block it
/// is named in, and says plainly where no in-place action exists.
///
/// A reserved reviewer's grant is Estigia's, so the "add it to the definition"
/// an ordinary agent gets is a dead end here. The one in-place change is the
/// `Evidence standard` row, and only for the unsealed reviewer's shell: the test
/// renders both answers, so the row the message names is crossed against the
/// gate that would have to honour it rather than trusted.
#[test]
fn a_reserved_reviewer_is_told_the_row_that_actually_grants_the_shell() {
    let reviewer = |evidence| {
        crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            evidence,
            crate::config::Seal::Unsealed,
            RESERVED_REVIEWER,
        )
    };
    let reading = reviewer(crate::config::Evidence::Reading);
    let refused = gate(Some(RESERVED_REVIEWER), "Bash", Some(&reading))
        .expect("a reading reviewer holds no shell");
    let message = format!("{refused}");
    assert!(message.contains("Evidence standard"), "{message}");
    assert!(message.contains("measuring"), "{message}");
    // Not the author's-definition fix, which the project cannot perform here.
    assert!(!message.contains("added to"), "{message}");

    // And the row clears it: the same role under `measuring` holds the shell.
    let measuring = reviewer(crate::config::Evidence::Measuring);
    assert!(
        gate(Some(RESERVED_REVIEWER), "Bash", Some(&measuring)).is_none(),
        "the row the refusal names does not clear the block it is named in"
    );

    // Where no in-place change grants the tool, the message says so and names
    // the two that work.
    let refused = gate(Some(RESERVED_REVIEWER), "Write", Some(&reading))
        .expect("a reading reviewer cannot write");
    let message = format!("{refused}");
    assert!(!message.contains("added to"), "{message}");
    assert!(
        message.contains("no in-place change grants any other tool"),
        "{message}"
    );
    assert!(message.contains("different sub-agent"), "{message}");
    assert!(
        message.contains("separate session or durable handoff"),
        "{message}"
    );
}

#[test]
fn the_main_conversation_is_not_a_role() {
    // No `agent_type` means the main conversation made the call. It has no
    // declared list, and treating it as a role outside one would gate every
    // ordinary call in the session.
    assert!(gate(None, "WebFetch", Some(BUILDER)).is_none());
    assert!(gate(None, "Bash", None).is_none());
    // And a named sub-agent whose definition cannot be found is the same
    // answer: not told is not forbidden.
    assert!(gate(Some("builder"), "WebFetch", None).is_none());
}

#[test]
fn a_name_from_the_host_never_reads_outside_the_agents_directory() {
    // The name arrives from outside and lands in a path. A separator or a
    // parent segment would read some other file entirely, and whatever tool
    // list it found there would be enforced as though somebody had written it
    // for this role.
    let root = tempfile::tempdir().expect("a temporary root");
    let agents = root.path().join(".claude/agents");
    std::fs::create_dir_all(&agents).expect("the directory");
    std::fs::write(agents.join("builder.md"), BUILDER).expect("the definition");
    // Something outside it that a traversal would reach.
    std::fs::write(root.path().join("elsewhere.md"), BUILDER).expect("a decoy");

    // The ordinary name resolves.
    assert!(
        definition_for(root.path(), None, "builder")
            .expect("a readable search")
            .is_some(),
        "the definition beside the project was not found"
    );

    for hostile in [
        "",
        "..",
        "../elsewhere",
        "../../elsewhere",
        "sub/builder",
        r"sub\builder",
        "/etc/passwd",
    ] {
        assert!(
            definition_for(root.path(), None, hostile)
                .expect("a name that names nothing here is not a failure")
                .is_none(),
            "{hostile:?} was resolved to a file"
        );
    }
}

#[test]
fn the_project_definition_wins_over_the_operators_own() {
    // A repository that ships its own `builder` is choosing that one, and
    // enforcing the home copy instead would hold a list nobody in this
    // checkout wrote.
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    std::fs::create_dir_all(project.path().join(".claude/agents")).expect("project agents");
    std::fs::create_dir_all(home.path().join(".claude/agents")).expect("home agents");
    std::fs::write(
        project.path().join(".claude/agents/builder.md"),
        "---\nname: builder\ntools: Read\n---\n",
    )
    .expect("the project's");
    std::fs::write(home.path().join(".claude/agents/builder.md"), BUILDER).expect("the home one");

    let found = definition_for(project.path(), Some(home.path()), "builder")
        .expect("a readable search")
        .expect("found");
    assert_eq!(
        allowlist(&found).expect("a list"),
        ["Read"],
        "the home definition was enforced over the project's"
    );

    // And with nothing in the project, the operator's own is used.
    let empty = tempfile::tempdir().expect("an empty project");
    let found = definition_for(empty.path(), Some(home.path()), "builder")
        .expect("a readable search")
        .expect("found");
    assert_eq!(allowlist(&found).expect("a list").len(), 6);
}

/// The real thing, copied from an installed OpenCode agent.
const OPENCODE: &str = "---\n\
    description: Handles OpenCode Loop slash-command acknowledgements without tools.\n\
    mode: primary\n\
    permission:\n  \
      \"*\": deny\n\
    ---\n\n\
    The OpenCode Loop plugin has already handled the slash command locally.\n";

#[test]
fn opencode_writes_the_same_policy_in_a_different_shape() {
    // Same file shape, different key: a permission map with a pattern on the
    // left and a verdict on the right, rather than an allowlist. Flattening it
    // into a list would lose `ask`, which is neither of the other two.
    let policy = declared_policy(OPENCODE).expect("a permission map");
    assert_eq!(
        policy,
        Policy::Permissions(vec![("*".to_owned(), Verdict::Deny)])
    );

    // `"*": deny` means exactly that, for anything.
    assert_eq!(policy.verdict("Bash"), Verdict::Deny);
    assert_eq!(policy.verdict("Read"), Verdict::Deny);
    assert!(gate(Some("local"), "Read", Some(OPENCODE)).is_some());

    // An exact name beats the wildcard whatever order they were written in:
    // `"*": deny` beside `Read: allow` means "only Read", and reading it the
    // other way would deny the one tool the author deliberately let through.
    let mixed = "---\npermission:\n  \"*\": deny\n  Read: allow\n---\n";
    let policy = declared_policy(mixed).expect("a permission map");
    assert_eq!(policy.verdict("Read"), Verdict::Allow);
    assert_eq!(policy.verdict("Write"), Verdict::Deny);
    assert!(gate(Some("reader"), "Read", Some(mixed)).is_none());

    // `ask` is not a denial. The author is saying a person decides, and a
    // harness that answered for them would take the decision it was handed.
    let asks = "---\npermission:\n  Bash: ask\n---\n";
    assert_eq!(
        declared_policy(asks).expect("a map").verdict("Bash"),
        Verdict::Ask
    );
    assert!(gate(Some("careful"), "Bash", Some(asks)).is_none());

    // A word this build does not know is skipped, not read as a denial:
    // OpenCode may add one, and refusing on it would break every run using it.
    let unknown = "---\npermission:\n  Bash: someday\n  \"*\": deny\n---\n";
    let policy = declared_policy(unknown).expect("a map");
    assert_eq!(
        policy.verdict("Bash"),
        Verdict::Deny,
        "the wildcard applies"
    );
    assert_eq!(
        policy,
        Policy::Permissions(vec![("*".to_owned(), Verdict::Deny)]),
        "the unknown verdict was kept"
    );

    // Nothing declared at all is still "not told", not "allow nothing".
    let bare = "---\ndescription: something\nmode: primary\n---\n";
    assert_eq!(declared_policy(bare), None);
    assert!(gate(Some("x"), "Bash", Some(bare)).is_none());

    // A key at column zero ends the block rather than being read as a rule.
    let after = "---\npermission:\n  Read: allow\nmode: primary\n---\n";
    assert_eq!(
        declared_policy(after).expect("a map"),
        Policy::Permissions(vec![("Read".to_owned(), Verdict::Allow)]),
        "a following top-level key was read as a permission"
    );
}

#[test]
fn a_definition_carrying_both_keys_is_read_by_its_own_hosts() {
    // A Claude Code file somebody added a permission map to. The key its own
    // host reads is the one that decides what actually happens.
    let both = "---\ntools: Read\npermission:\n  \"*\": allow\n---\n";
    assert_eq!(
        declared_policy(both).expect("a policy"),
        Policy::Allowlist(vec!["Read".to_owned()])
    );
    assert!(gate(Some("builder"), "Bash", Some(both)).is_some());
}

#[test]
fn both_dialects_directories_are_searched() {
    // A machine with Claude Code and OpenCode installed runs both, and the
    // sub-agent calling right now belongs to whichever is asking. Choosing the
    // directory from the hook's dialect would work until two agents share a
    // name, and then it would enforce the wrong file in silence.
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    for (root, relative) in [
        (project.path(), ".opencode/agents"),
        (home.path(), ".config/opencode/agents"),
    ] {
        std::fs::create_dir_all(root.join(relative)).expect("the directory");
    }
    std::fs::write(project.path().join(".opencode/agents/local.md"), OPENCODE)
        .expect("the project's");
    std::fs::write(
        home.path().join(".config/opencode/agents/mine.md"),
        OPENCODE,
    )
    .expect("the operator's");

    assert!(
        definition_for(project.path(), Some(home.path()), "local")
            .expect("a readable search")
            .is_some(),
        "a project OpenCode definition was not found"
    );
    assert!(
        definition_for(project.path(), Some(home.path()), "mine")
            .expect("a readable search")
            .is_some(),
        "an OpenCode definition under the config directory was not found"
    );
    // And a name nobody wrote stays absent rather than resolving to something.
    assert!(
        definition_for(project.path(), Some(home.path()), "nobody")
            .expect("a readable search")
            .is_none()
    );
}

#[test]
fn the_shipped_blind_reviewer_gets_the_grant_its_standard_decides() {
    // Both renderings, because the grant is derived from the evidence standard
    // now, and the raw asset carries a placeholder rather than a tool
    // list. Walked over `Evidence::all()` so a third standard cannot be added
    // without deciding here what its reviewer may do.
    //
    // The shape this holds is the whole of what `measuring` buys: **a shell and
    // nothing else**. Reading stays reading in both, and `Write`, `Edit`,
    // `Agent` and `Task` are denied whichever answer the operator gave.
    //
    // What that is worth, stated exactly, because five other places in this
    // change said it wrong: it holds the *grant* to four refusals and a shell.
    // It does not stop a measuring judge editing the target — the shell does
    // that, and `skill/policies/judgment-day.md` says the directory it is
    // confined to is the only thing answering for it. So a widening past the
    // shell is refused here because the isolation rule is written for a judge
    // with a shell and nothing wider, not because this assertion is what keeps
    // the target safe.
    for evidence in crate::config::Evidence::all() {
        let definition = crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            evidence,
            crate::config::Seal::Unsealed,
            RESERVED_REVIEWER,
        );
        let declared = declared_policy(&definition).expect("the reviewer declares a gate policy");
        for tool in ["Read", "Grep", "Glob"] {
            assert_eq!(
                declared.verdict(tool),
                Verdict::Allow,
                "{evidence:?} {tool}"
            );
            assert!(gate(Some("review-blind"), tool, Some(&definition)).is_none());
        }
        let shell = match evidence {
            crate::config::Evidence::Reading => Verdict::Deny,
            crate::config::Evidence::Measuring => Verdict::Allow,
        };
        assert_eq!(declared.verdict("Bash"), shell, "{evidence:?} Bash");
        assert_eq!(
            gate(Some("review-blind"), "Bash", Some(&definition)).is_none(),
            shell == Verdict::Allow,
            "{evidence:?}: the gate and the declaration disagree about the shell"
        );
        for tool in ["Write", "Edit", "Agent", "Task"] {
            assert_eq!(declared.verdict(tool), Verdict::Deny, "{evidence:?} {tool}");
            assert!(gate(Some("review-blind"), tool, Some(&definition)).is_some());
        }
        assert!(definition.contains("Do NOT delegate"));
        // And the placeholder is gone. A rendering that left `{{TOOLS}}` in
        // place would be parsed as an allowlist naming one tool called
        // `{{TOOLS}}`, which denies everything and reads as a working gate.
        assert!(
            !definition.contains("{{"),
            "{evidence:?}: a placeholder survived"
        );
    }
}

/// [`Seal::Sealed`] denies every tool `Evidence` can ever grant, whichever
/// value `evidence` carries — the sealed arm does not read it. Walked over
/// [`crate::config::Evidence::all`] for the same reason the test above is: a
/// third standard must not accidentally reopen the grant this axis closes.
#[test]
fn a_sealed_reviewer_is_denied_every_tool_evidence_can_grant_whatever_its_standard() {
    for evidence in crate::config::Evidence::all() {
        let definition = crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            evidence,
            crate::config::Seal::Sealed,
            RESERVED_REVIEWER_SEALED,
        );
        let declared = declared_policy(&definition).expect("the reviewer declares a gate policy");
        for tool in ["Read", "Grep", "Glob", "Bash"] {
            assert_eq!(declared.verdict(tool), Verdict::Deny, "{evidence:?} {tool}");
            assert!(gate(Some("review-blind"), tool, Some(&definition)).is_some());
        }
        assert!(
            !definition.contains("{{"),
            "{evidence:?}: a placeholder survived"
        );
    }
}

#[test]
fn a_sealed_grant_is_a_policy_not_the_absence_of_one() {
    for evidence in crate::config::Evidence::all() {
        let definition = crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            evidence,
            crate::config::Seal::Sealed,
            RESERVED_REVIEWER_SEALED,
        );
        assert!(
            declared_policy(&definition).is_some(),
            "{evidence:?}: a sealed rendering with no declared policy permits every tool at this gate"
        );
    }
}

/// A moved config home does not remove a sub-agent's allowlist.
///
/// The OpenCode root was spelled `~/.config/opencode/agents` by hand while
/// `setup` resolved the same tree through `XDG_CONFIG_HOME`. With that variable
/// set, this looked in a directory nothing writes — and a definition that is not
/// found is `Ok(None)`, which `declared_policy` reads as *the sub-agent may use
/// every tool*. So the failure was not a refusal, it was the allowlist quietly
/// going away: the loosening direction, at the gate whose whole subject is what a
/// delegated context may not do.
///
/// A reviewer of the change that moved four `CONTROL_SURFACE` entries for this
/// exact variable measured that the enforcement road had been left behind.
#[test]
fn a_moved_config_home_still_finds_an_opencode_definition() {
    let home = tempfile::tempdir().expect("a home");
    let moved = tempfile::tempdir().expect("a relocated config home");
    let repo = tempfile::tempdir().expect("a checkout");

    let agents = moved.path().join("opencode").join("agents");
    std::fs::create_dir_all(&agents).expect("the definition directory");
    std::fs::write(
        agents.join("scribe.md"),
        "---\ntools: Read, Grep\n---\nread only\n",
    )
    .expect("a definition to find");

    let found = with_config_home(moved.path(), || {
        definition_for(repo.path(), Some(home.path()), "scribe")
    });

    assert!(
        matches!(found, Ok(Some(ref text)) if text.contains("Read, Grep")),
        "a relocated XDG config home hid the definition, and a definition that is \
         not found is read as every tool allowed: {found:?}"
    );
}

/// An unusable `XDG_CONFIG_HOME` means *the default*, on both roads.
///
/// The first fix for the hardcoded root read the variable directly and so
/// introduced a third rule: `setup` folds an empty or relative value away through
/// `absolute_or_none`, and reading it here took both literally. A reviewer
/// measured what that cost — with `XDG_CONFIG_HOME=""` or a relative value, this
/// found nothing while `setup` had written to `~/.config/opencode/agents`, and
/// nothing found is `Ok(None)`, which `declared_policy` reads as *every tool
/// allowed*. The same loosening the fix existed to close, through the two inputs
/// the fix itself created.
///
/// One rule now, in `setup::xdg_config_home`, and this asks for it.
#[test]
fn an_unusable_config_home_falls_back_the_way_setup_does() {
    let home = tempfile::tempdir().expect("a home");
    let repo = tempfile::tempdir().expect("a checkout");

    let agents = home.path().join(".config").join("opencode").join("agents");
    std::fs::create_dir_all(&agents).expect("the default definition directory");
    std::fs::write(
        agents.join("scribe.md"),
        "---\ntools: Read, Grep\n---\nread only\n",
    )
    .expect("a definition to find");

    for unusable in ["", "relative/config"] {
        let found = with_config_home(std::path::Path::new(unusable), || {
            definition_for(repo.path(), Some(home.path()), "scribe")
        });

        assert!(
            matches!(found, Ok(Some(ref text)) if text.contains("Read, Grep")),
            "XDG_CONFIG_HOME={unusable:?} sent this somewhere `setup` never writes, and a \
             definition that is not found is read as every tool allowed: {found:?}"
        );
    }
}

// The helper these fixtures use is `crate::test_env::with_config_home`, and the
// history that shaped it is written there: `set_var` is process-wide, this test
// binary is multi-threaded, and an earlier SAFETY note reasoned about the shipped
// binary — "the gate answers one hook invocation per process" — which is true of
// the binary and false of the process the `unsafe` runs in. A copy of the helper
// then stood in `setup::tests` too, justified by a comment saying these were two
// binaries; they are one, so the two mutexes excluded nothing and a reviewer
// reproduced the cross-talk. One lock now, in one place.

/// A moved config home does not hide a definition at the default root.
///
/// The mirror of the test above, and the direction nothing held: the first fix
/// for the hardcoded root **replaced** it with the relocated one instead of
/// adding it, so a definition sitting at `~/.config/opencode/agents` stopped
/// being found the moment the variable pointed elsewhere. Not found is
/// `Ok(None)`, which `declared_policy` reads as *the sub-agent may use every tool*
/// — the same loosening that fix existed to close, one configuration over. A
/// reviewer measured base ENFORCED against head NOT FOUND.
///
/// Searching both roots cannot loosen anything: a definition is enforced if any
/// root has it.
#[test]
fn a_moved_config_home_does_not_hide_the_default_root() {
    let home = tempfile::tempdir().expect("a home");
    let moved = tempfile::tempdir().expect("a relocated config home");
    let repo = tempfile::tempdir().expect("a checkout");

    let default_root = home.path().join(".config").join("opencode").join("agents");
    std::fs::create_dir_all(&default_root).expect("the default definition directory");
    std::fs::write(
        default_root.join("scribe.md"),
        "---\ntools: Read, Grep\n---\nread only\n",
    )
    .expect("a definition at the default root");
    // The relocated root exists and is empty, which is the shape that hid it.
    std::fs::create_dir_all(moved.path().join("opencode").join("agents"))
        .expect("an empty relocated root");

    let found = with_config_home(moved.path(), || {
        definition_for(repo.path(), Some(home.path()), "scribe")
    });

    assert!(
        matches!(found, Ok(Some(ref text)) if text.contains("Read, Grep")),
        "a relocated config home hid the definition at the default root, and a definition \
         that is not found is read as every tool allowed: {found:?}"
    );
}

/// A Claude Code sub-agent definition, in the dialect that adapter writes.
const CLAUDE_STYLE_DEFINITION: &str = "---\nname: scribe\ntools: Read, Grep\n---\nread only\n";

/// `setup` resolves Claude Code's agent-definitions root under
/// `$CLAUDE_CONFIG_DIR` when it is set (`agent_definitions_root`), and the
/// gate never looked there — a definition written exactly where `setup` put
/// it was `Ok(None)`, which `declared_policy` reads as *every tool allowed*.
#[test]
fn a_relocated_claude_config_dir_is_searched_beside_the_default() {
    let home = tempfile::tempdir().expect("a home");
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    let repo = tempfile::tempdir().expect("a checkout");

    let agents = relocated.path().join("agents");
    std::fs::create_dir_all(&agents).expect("the relocated definition directory");
    std::fs::write(agents.join("scribe.md"), CLAUDE_STYLE_DEFINITION)
        .expect("a definition to find");

    let found = with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || definition_for(repo.path(), Some(home.path()), "scribe"),
    );

    assert!(
        matches!(found, Ok(Some(ref text)) if text.contains("Read, Grep")),
        "a relocated CLAUDE_CONFIG_DIR hid the definition, and a definition that is not \
         found is read as every tool allowed: {found:?}"
    );
}

/// The mirror of the test above: the relocated root is searched **beside**
/// the default, never instead of it — the same direction the account
/// of the OpenCode root states.
#[test]
fn a_relocated_claude_config_dir_does_not_hide_the_default_root() {
    let home = tempfile::tempdir().expect("a home");
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    let repo = tempfile::tempdir().expect("a checkout");

    let default_agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&default_agents).expect("the default definition directory");
    std::fs::write(default_agents.join("scribe.md"), CLAUDE_STYLE_DEFINITION)
        .expect("a definition at the default root");
    // The relocated root exists and is empty, which is the shape that would
    // hide the default root if this searched only the relocated one.
    std::fs::create_dir_all(relocated.path().join("agents"))
        .expect("an empty relocated agents directory");

    let found = with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || definition_for(repo.path(), Some(home.path()), "scribe"),
    );

    assert!(
        matches!(found, Ok(Some(ref text)) if text.contains("Read, Grep")),
        "a relocated CLAUDE_CONFIG_DIR hid the definition at the default root: {found:?}"
    );
}

/// `authorize_review_blind_launch` proved the
/// canonical reviewer under `home.join(".claude/agents")` alone, so a session
/// running under a relocated `$CLAUDE_CONFIG_DIR` found the *other* profile's
/// file (or none) and refused every reserved launch —
/// `reviewer-canonical-unavailable`, measured live against a real two-profile
/// machine on 2026-09-18.
#[test]
fn a_relocated_claude_config_dir_is_where_the_canonical_reviewer_is_proved() {
    let project = tempfile::tempdir().expect("a project");
    // Deliberately empty: proving this works without the default tree is the
    // point, since the decision this closes is that the default tree is not
    // consulted at all once the variable is set.
    let home = tempfile::tempdir().expect("a home");
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    let file = relocated.path().join("agents").join("review-blind.md");
    std::fs::create_dir_all(file.parent().expect("a parent"))
        .expect("the relocated agents directory");
    std::fs::write(
        &file,
        crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            crate::config::Evidence::Reading,
            crate::config::Seal::Unsealed,
            RESERVED_REVIEWER,
        ),
    )
    .expect("the canonical reviewer");

    let result = with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER),
    );

    assert!(
        result.is_ok(),
        "a canonical reviewer written exactly where `setup` puts it under a relocated \
         CLAUDE_CONFIG_DIR was refused: {result:?}"
    );
}

/// The direction that matters most: once `$CLAUDE_CONFIG_DIR` is set, the
/// *default* `~/.claude/agents` tree must never be able to satisfy the
/// launch — that tree is the other profile's, per the decision this issue
/// records in `docs/`.
#[test]
fn a_relocated_claude_config_dir_does_not_fall_back_to_the_default_home_tree() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    install_canonical_reviewer(home.path());
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    std::fs::create_dir_all(relocated.path().join("agents"))
        .expect("an empty relocated agents directory");

    let result = with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER),
    );

    let refusal = result.expect_err(
        "the default home tree authorized a launch while CLAUDE_CONFIG_DIR pointed \
         elsewhere \u{2014} a relocated session must never load the other profile's reviewer",
    );
    assert_eq!(refusal.code, "reviewer-canonical-unavailable");
}

/// A foreign or stale file sitting exactly where `setup` would have written
/// the canonical reviewer is refused the same way a foreign home-tree file
/// already was, before this change gave the relocated tree any standing at
/// all.
#[test]
fn a_foreign_reviewer_under_a_relocated_claude_config_dir_is_refused() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    let file = relocated.path().join("agents").join("review-blind.md");
    std::fs::create_dir_all(file.parent().expect("a parent"))
        .expect("the relocated agents directory");
    std::fs::write(
        &file,
        "---\nname: review-blind\ntools: Read, Write, Bash\n---\nHostile.\n",
    )
    .expect("a foreign reviewer");

    let result = with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || authorize_review_blind_launch(project.path(), Some(home.path()), RESERVED_REVIEWER),
    );

    let refusal =
        result.expect_err("a foreign reviewer under the relocated tree launched unchallenged");
    assert_eq!(refusal.code, "reviewer-canonical-unavailable");
}

/// The lens is a reserved row like the other two, sealed and carrying its own
/// template rather than the judge's.
#[test]
fn the_lens_is_a_sealed_row_carrying_its_own_template() {
    let row = RESERVED_REVIEWERS
        .iter()
        .find(|&&(name, _, _)| name == RESERVED_LENS)
        .expect("review-lens has a row in RESERVED_REVIEWERS");
    assert_eq!(row.1, crate::config::Seal::Sealed);
    assert_eq!(row.2, crate::skill::REVIEW_LENS.contents);
    assert_ne!(
        row.2,
        crate::skill::REVIEW_AGENT.contents,
        "the lens row carries the judge's template rather than its own"
    );
}

/// `reserved_reviewer_template` answers each row's own body — the accessor
/// `cli::mod` and `harness::hook` ask instead of reaching for
/// [`crate::skill::REVIEW_AGENT`] by habit.
#[test]
fn reserved_reviewer_template_answers_each_rows_own_body() {
    assert_eq!(
        reserved_reviewer_template(RESERVED_REVIEWER),
        Some(crate::skill::REVIEW_AGENT.contents)
    );
    assert_eq!(
        reserved_reviewer_template(RESERVED_REVIEWER_SEALED),
        Some(crate::skill::REVIEW_AGENT.contents)
    );
    assert_eq!(
        reserved_reviewer_template(RESERVED_LENS),
        Some(crate::skill::REVIEW_LENS.contents)
    );
    assert_eq!(reserved_reviewer_template("not-a-reserved-name"), None);
}

/// The trap: `render_reviewer_agent` used to
/// search for a needle hardcoded to the judge's own placeholder,
/// `\nname: review-blind\n`. A lens template whose own frontmatter already
/// reads `name: review-lens` makes that search silently miss and the
/// rendering keeps its own template's name — correct only when the caller's
/// target name happens to already equal it. Rendering under a **different**
/// name is what tells the two apart, and this reddens under the hardcoded
/// needle because the lens's `name: review-lens` is never found and never
/// rewritten.
#[test]
fn render_reviewer_agent_rewrites_the_templates_own_placeholder_name() {
    let rendered = crate::setup::render_reviewer_agent(
        crate::skill::REVIEW_LENS.contents,
        crate::config::Evidence::Reading,
        crate::config::Seal::Sealed,
        "some-other-lens-name",
    );
    assert!(
        rendered.contains("\nname: some-other-lens-name\n"),
        "the target name was not written into the frontmatter:\n{rendered}"
    );
    assert!(
        !rendered.contains("\nname: review-lens\n"),
        "the template's own placeholder name is still present after rendering under a \
         different name:\n{rendered}"
    );
}

/// The lens's own rendering is recognised at launch the same way a judge's is.
#[test]
fn a_sealed_lens_rendering_is_static() {
    assert!(crate::setup::reviewer_is_static(
        &crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_LENS.contents,
            crate::config::Evidence::Reading,
            crate::config::Seal::Sealed,
            RESERVED_LENS,
        )
    ));
}

/// [`RESERVED_REFUTER`] and [`RESERVED_VALIDATOR`] are skipped because they record no
/// finding: they answer through `record_refutation` and `record_validator_result`.
#[test]
fn every_finding_vocabulary_the_transport_accepts_is_named_in_every_reviewer_that_records_one() {
    let vocabularies: &[(&str, &[&str])] = &[
        ("FINDING_CLASSES", crate::transport::claim::FINDING_CLASSES),
        ("FINDING_ORIGINS", crate::transport::claim::FINDING_ORIGINS),
        (
            "FINDING_EVIDENCE_CLASSES",
            crate::transport::claim::FINDING_EVIDENCE_CLASSES,
        ),
        (
            "FINDING_CAUSAL_DISPOSITIONS",
            crate::transport::claim::FINDING_CAUSAL_DISPOSITIONS,
        ),
    ];
    assert_eq!(
        vocabularies.len(),
        4,
        "a vocabulary was dropped from this list, which no const would notice"
    );
    for (index, (const_name, values)) in vocabularies.iter().enumerate() {
        assert!(
            !values.is_empty(),
            "{const_name} is empty, so this test proves nothing"
        );
        for (other_name, other) in &vocabularies[index + 1..] {
            assert!(
                !std::ptr::eq(*values, *other),
                "{const_name} and {other_name} carry the same const, so the crossing checks one \
                 vocabulary twice and never the other"
            );
        }
    }

    let field_names = ["origin", "evidence_class", "causal_disposition"];

    let mut examined = 0;
    for &(name, seal, template) in RESERVED_REVIEWERS {
        if name == RESERVED_REFUTER || name == RESERVED_VALIDATOR {
            continue;
        }
        examined += 1;

        for (const_name, values) in vocabularies {
            for value in *values {
                assert!(
                    template.contains(value),
                    "{name}'s own skill/agents/*.md template does not name the value {value:?} \
                     from {const_name} before rendering; teach it in the definition itself, not \
                     only in what render_reviewer_agent adds"
                );
            }
        }

        let grants = match seal {
            crate::config::Seal::Sealed => vec![crate::config::Evidence::Reading],
            crate::config::Seal::Unsealed => crate::config::Evidence::all(),
        };
        for grant in grants {
            let rendered = crate::setup::render_reviewer_agent(template, grant, seal, name);

            for field in field_names {
                assert!(
                    rendered.contains(field),
                    "{name} (rendered under {seal:?}, {grant:?}) does not name the field \
                     {field:?} that record_review_finding reads; teach it in that reviewer's \
                     own skill/agents/*.md template"
                );
            }
        }
    }

    assert_eq!(
        examined, 7,
        "expected seven reviewers examined (RESERVED_REVIEWERS minus the refuter and the validator), found {examined}"
    );
}
