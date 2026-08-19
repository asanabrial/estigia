use super::*;
use crate::test_env::with_config_home;

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
    // grant is derived from the evidence standard since issue 83, so the raw
    // asset is a template and no installed file ever holds it.
    std::fs::write(
        file,
        crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            crate::config::Evidence::Reading,
        ),
    )
    .expect("the canonical reviewer");
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
fn reserved_reviewer_launch_rejects_exact_and_nested_renamed_project_shadows() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    install_canonical_reviewer(home.path());

    let exact = project_reviewer(project.path(), "review-blind.md");
    let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("the exact project shadow launched");
    assert_eq!(refusal.code, "reviewer-project-shadow");
    assert!(matches!(refusal.resolution, Resolution::NoCommand { .. }));
    std::fs::remove_file(exact).expect("the exact shadow is removed");

    project_reviewer(project.path(), "nested/renamed.md");
    let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
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

    let refusal = authorize_review_blind_launch(&nested, Some(home.path()))
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
        let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
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
    let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("an unreadable candidate was clearance");
    assert_eq!(refusal.code, "reviewer-project-unprovable");
    std::fs::remove_file(agents.join("unknown.md")).expect("the obstacle is removed");

    std::fs::write(
        agents.join("ambiguous.md"),
        "---\nname: *operator_owned\n---\n",
    )
    .expect("an ambiguous name");
    let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("an ambiguous candidate was clearance");
    assert_eq!(refusal.code, "reviewer-project-unprovable");
    std::fs::remove_file(agents.join("ambiguous.md")).expect("the ambiguity is removed");

    project_reviewer(project.path(), "one.md");
    project_reviewer(project.path(), "nested/two.md");
    let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("duplicate project candidates were accepted");
    assert_eq!(refusal.code, "reviewer-project-unprovable");
}

#[test]
fn canonical_user_reviewer_must_exist_read_and_match() {
    let project = tempfile::tempdir().expect("a project");
    let home = tempfile::tempdir().expect("a home");
    let canonical = home.path().join(".claude/agents/review-blind.md");

    let missing = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("a missing canonical reviewer launched");
    assert_eq!(missing.code, "reviewer-canonical-unavailable");

    std::fs::create_dir_all(&canonical).expect("an unreadable canonical path");
    let unreadable = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("an unreadable canonical reviewer launched");
    assert_eq!(unreadable.code, "reviewer-canonical-unavailable");
    std::fs::remove_dir(&canonical).expect("the obstacle is removed");

    std::fs::write(
        &canonical,
        "---\nname: review-blind\ntools: Read, Write\n---\n",
    )
    .expect("a changed canonical reviewer");
    let changed = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("a changed canonical reviewer launched");
    assert_eq!(changed.code, "reviewer-canonical-unavailable");

    std::fs::write(
        &canonical,
        crate::setup::render_reviewer_agent(
            crate::skill::REVIEW_AGENT.contents,
            crate::config::Evidence::Reading,
        ),
    )
    .expect("the canonical reviewer is restored");
    authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect("the canonical reviewer with no collision proceeds");
}

#[test]
fn canonical_user_reviewer_must_be_the_unique_reserved_definition() {
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

    let refusal = authorize_review_blind_launch(project.path(), Some(home.path()))
        .expect_err("a duplicate user reviewer launched");
    assert_eq!(refusal.code, "reviewer-canonical-unavailable");
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
    assert!(gate(Some("opencode-loop-local"), "Read", Some(OPENCODE)).is_some());

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

/// The phases this crate ships declare a tool list, and the gate makes it true.
///
/// This is the whole of the claim that Estigia is that thing *and better*.
/// Other harnesses ship these by the handful — `agents/sdd-*.md`, with
/// `tools: Read, Grep, Glob, WebFetch, WebSearch, …` on `sdd-explore` and no
/// `Write` anywhere in it — and then rely on the host to honour them. A
/// published `agents/builder.md` does the same. None of them is at the tool
/// boundary, so none can do anything when a phase writes.
///
/// Estigia is. Claude Code sends `agent_type` on every tool event that fires
/// inside a sub-agent, so the list *these files already declare* is read at the
/// gate and a call outside it is refused. Same files, same names, same host
/// routing — and the declared list is a boundary rather than a request.
///
/// Read out of the shipped payload rather than a fixture: a copy of the
/// frontmatter written here would keep passing after somebody widened the real
/// one, which is the failure this whole module exists to refuse.
#[test]
fn a_shipped_planning_phase_gets_the_grant_its_own_work_needs() {
    for openspec in [false, true] {
        let config = crate::config::Config {
            planning: crate::config::Planning::Sdd {
                openspec,
                lite: false,
            },
            ..crate::config::Config::default()
        };
        a_shipped_planning_phase_cannot_write(&config, openspec);
    }
}

fn a_shipped_planning_phase_cannot_write(config: &crate::config::Config, openspec: bool) {
    let mut checked = 0;
    for file in crate::skill::PHASE_AGENTS {
        let Some(phase) = file
            .path
            .strip_prefix("agents/sdd-")
            .and_then(|rest| rest.strip_suffix(".md"))
        else {
            continue;
        };
        // Rendered by the function that ships, under both answers `Planning`
        // can give. Substituting one literal here crossed the read-only grant
        // and left the two wider ones crossed by nothing — including
        // `Read, Grep, Glob, Write, Edit`, which is what an `openspec` install
        // actually writes, so the rendering most able to do harm was the one
        // this test never saw. Two blind judges measured that on issue 83.
        let definition = crate::setup::render_phase_agent(file.contents, phase, config);
        let declared = declared_policy(&definition).unwrap_or_else(|| {
            panic!("agents/sdd-{phase}.md declares no policy, so the gate would enforce nothing")
        });

        // A phase that files an artifact on disk needs to write it; one that
        // only thinks does not, and `explore` reaches outward instead. This
        // is the same condition `render_phase_agent` decides by, asserted
        // from the other side: the grant has to match the work.
        let writes_artifacts = openspec && matches!(phase, "spec" | "design" | "tasks");
        let artifact = match writes_artifacts {
            true => Verdict::Allow,
            false => Verdict::Deny,
        };
        for tool in ["Write", "Edit"] {
            assert_eq!(
                declared.verdict(tool),
                artifact,
                "sdd-{phase} under openspec={openspec} answers the wrong thing for {tool}, so \
                 either a planning phase writes where it should not or the one that files an \
                 artifact cannot file it"
            );
        }
        // No answer buys a shell, and none buys the authority to hand the
        // work on. `roles.rs` runs inside `PreToolUse`, so this asserts the
        // reachable half rather than the declared one.
        for tool in ["Bash", "Agent", "Task"] {
            assert_eq!(
                declared.verdict(tool),
                Verdict::Deny,
                "sdd-{phase} would be allowed to run {tool}, and a planning phase that runs or \
                 delegates is the thing the declared list exists to stop"
            );
        }
        // `explore` is the one phase that reaches outward, and the pair that
        // lets it is what its grant is *for*. This gate never sees either call
        // — the module note above says so — which makes the declared list the
        // whole of that boundary, and is exactly why narrowing it has to redden
        // something here rather than nowhere.
        let outward = match phase {
            "explore" => Verdict::Allow,
            _ => Verdict::Deny,
        };
        for tool in ["WebFetch", "WebSearch"] {
            assert_eq!(
                declared.verdict(tool),
                outward,
                "sdd-{phase} answers the wrong thing for {tool}, so either a planning phase \
                 reaches outward where it should not or `explore` cannot do the one thing it \
                 is for"
            );
        }
        // A rendering that left `{{TOOLS}}` in place parses as an allowlist
        // naming one tool called `{{TOOLS}}`, which denies everything and
        // reads as a working gate.
        assert!(
            !definition.contains("{{"),
            "sdd-{phase}: a placeholder survived the rendering"
        );
        // And it is not an empty allowlist: a phase that could do nothing at all
        // would satisfy the loop above and be useless.
        assert_eq!(
            declared.verdict("Read"),
            Verdict::Allow,
            "sdd-{phase} cannot read, so its list denies everything and measures nothing"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 5,
        "expected the five planning phases in the payload, found {checked}"
    );
}

#[test]
fn the_shipped_blind_reviewer_gets_the_grant_its_standard_decides() {
    // Both renderings, because the grant is derived from the evidence standard
    // since issue 83 and the raw asset carries a placeholder rather than a tool
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
    // that, and `skill/policies/blind-judges.md` says the directory it is
    // confined to is the only thing answering for it. So a widening past the
    // shell is refused here because the isolation rule is written for a judge
    // with a shell and nothing wider, not because this assertion is what keeps
    // the target safe.
    for evidence in crate::config::Evidence::all() {
        let definition =
            crate::setup::render_reviewer_agent(crate::skill::REVIEW_AGENT.contents, evidence);
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
