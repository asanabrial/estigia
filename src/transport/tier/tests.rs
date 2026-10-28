use super::*;

// ---------------------------------------------------------------------------
// Pure fixtures: no process boundary, so these exercise the classifier's own
// arithmetic directly rather than through a repository.
// ---------------------------------------------------------------------------

fn active_stat(path: &str, additions: u64, deletions: u64) -> DiffStat {
    DiffStat {
        path: path.to_owned(),
        additions,
        deletions,
        binary: false,
        mode_only: false,
        old_mode: "100644".to_owned(),
        new_mode: "100644".to_owned(),
        old_object: "a".repeat(40),
        new_object: "b".repeat(40),
    }
}

fn passive_stat(path: &str, additions: u64) -> DiffStat {
    active_stat(path, additions, 0)
}

/// The classifier's own two computations over one stat set: the reasons, and
/// the level those reasons are cross-checked against in [`review_tier`].
/// Every fixture in this module asserts both, which is what stands in for the
/// cross-check itself — the two can never legitimately disagree, so a fixture
/// this test drives is one way a future desync between them is caught;
/// `the_two_derivations_agree_on_a_spread_of_diff_stat_shapes` is the other,
/// and it does not go through this helper.
fn classify(stats: &[DiffStat]) -> (RiskLevel, RiskLevel) {
    let active_content = BTreeSet::new();
    let reasons = derive_snapshot_risk_reasons(stats, &active_content, Vec::new());
    let (only_passive, touches_configuration) = passive_and_configuration(stats, &active_content);
    let level = classify_risk(stats, &reasons, only_passive, touches_configuration);
    (level, risk_level_from_reasons(&reasons))
}

/// Ports `TestVolumeAloneNeverSelectsATier` (`risk_test.go:561`): the central
/// property of the port, and the one a stray `if additions > N` reintroduces
/// fastest. Breaking `classify_risk` to escalate past a threshold turns this
/// red at the threshold and nowhere else, which is what makes it a test of
/// the rule rather than of one example.
#[test]
fn volume_alone_never_selects_a_tier() {
    for lines in [1, 399, 400, 401, 1_000, 5_000, 50_000] {
        let stats = [
            active_stat("src/view.rs", lines / 2, lines - lines / 2),
            active_stat("src/render.rs", lines, 0),
        ];
        let (level, explained) = classify(&stats);
        assert_eq!(level, RiskLevel::Medium, "{lines} changed lines escalated");
        assert_eq!(explained, level);
    }
}

/// Ports `TestVolumeAloneNeverEscalatesPassiveDocumentation`
/// (`risk_test.go:584`).
#[test]
fn a_large_passive_only_change_is_the_zero_tier() {
    for lines in [1, 400, 401, 9_000] {
        let stats = [passive_stat("docs/guide.md", lines)];
        let (level, explained) = classify(&stats);
        assert_eq!(
            level,
            RiskLevel::Low,
            "{lines} documentation lines escalated"
        );
        assert_eq!(explained, level);
    }
}

/// Ports the "configuration cannot be low" case in
/// `TestClassifyRiskUsesDeterministicFirstMatch` (`risk_test.go:61`): a
/// passive document beside a manifest file is a consolidated review, because
/// the manifest is never itself a passive candidate.
#[test]
fn configuration_forbids_the_zero_tier() {
    let stats = [
        passive_stat("docs/guide.md", 4),
        active_stat("Cargo.toml", 1, 0),
    ];
    let (level, explained) = classify(&stats);
    assert_eq!(level, RiskLevel::Medium);
    assert_eq!(explained, level);
}

/// Ports `TestWebhookPathsEmitSecurityHotPath` (`risk_test.go:122`) and the
/// whole-token half of `TestTierTwoRequiresGenuineEvidence`
/// (`risk_test.go:607`): a token is matched as a **whole** field between the
/// split characters, never as a substring. `authority.rs` carries `auth` as a
/// substring and must not match; splitting on the wrong boundary, or matching
/// with `.contains` instead of an exact token, turns this red.
#[test]
fn hot_path_tokens_match_whole_tokens_only() {
    let cases: &[(&str, &[&str])] = &[
        ("internal/auth/token.rs", &["auth"]),
        ("src/update/apply.rs", &["update"]),
        ("src/security/guard.rs", &["security"]),
        ("backend/webhook_dispatcher.rs", &["security"]),
        ("src/payments/charge.rs", &["payments"]),
        ("src/authority.rs", &[]),
        ("src/authoritative_update_log.rs", &["update"]),
        ("src/view.rs", &[]),
        ("docs/guide.md", &[]),
    ];
    for &(path, want) in cases {
        let mut got = hot_path_risk_signals(path);
        got.sort_unstable();
        let mut want: Vec<&str> = want.to_vec();
        want.sort_unstable();
        assert_eq!(got, want, "{path} did not match {want:?}");
    }
}

/// Ports the signal half of `TestTierTwoRequiresGenuineEvidence`
/// (`risk_test.go:607`) and `TestClassifyRiskIgnoresTheFormerFourHundredLineBoundary`
/// (`risk_test.go:294`): any evidence is `High`, at any size on either side of
/// the removed 400-line boundary.
#[test]
fn any_named_evidence_is_high_at_any_size() {
    for lines in [1, 400, 401] {
        let stats = [active_stat("internal/auth/token.rs", lines, 0)];
        let (level, explained) = classify(&stats);
        assert_eq!(level, RiskLevel::High);
        assert_eq!(explained, level);
    }
}

/// A mode this classifier does not recognise as harmless never reads as
/// passive — fails closed rather than trusting an unfamiliar mode. Symlink
/// (`120000`) is a mode `valid_raw_git_mode` accepts as well-formed but
/// `is_regular_non_executable_git_mode` refuses; removing that refusal is
/// what this pins.
#[test]
fn an_unrecognised_mode_is_never_passive() {
    let stat = DiffStat {
        old_mode: "100644".to_owned(),
        new_mode: "120000".to_owned(),
        ..passive_stat("docs/guide.md", 1)
    };
    assert!(!is_passive_content_candidate_stat(&stat));
    let (level, explained) = classify(&[stat]);
    assert_eq!(level, RiskLevel::Medium);
    assert_eq!(explained, level);
}

#[test]
fn malformed_raw_git_modes_are_refused() {
    for mode in ["10064", "1006440", "10064x", "", "10064 "] {
        assert!(!valid_raw_git_mode(mode), "{mode:?} was accepted");
    }
    assert!(valid_raw_git_mode("100644"));
    assert!(valid_raw_git_mode("120000"));
}

#[test]
fn executable_mode_changed_reads_the_execute_bits_not_the_whole_mode() {
    assert!(executable_mode_changed("100644", "100755"));
    assert!(executable_mode_changed("100755", "100644"));
    assert!(!executable_mode_changed("100644", "100644"));
    // A mode-only entry that keeps its whole octal value differently spelled
    // is not reachable through `git diff --raw`, but the execute bits are
    // what this function reads, not the mode string as a whole: two modes
    // whose non-execute bits differ while the execute bits agree do not flip.
    assert!(!executable_mode_changed("100644", "100664"));
    // Git's raw diff always spells an absent side `000000`, never the empty
    // string, so this never treats a real add/delete as absent — only the
    // literal empty string does, matching `risk.go:714` exactly.
    assert!(executable_mode_changed("000000", "100755"));
    assert!(!executable_mode_changed("000000", "100644"));
    assert!(!executable_mode_changed("", "100755"));
    assert!(!executable_mode_changed("100755", ""));
}

#[test]
fn service_token_tokens_must_be_adjacent_and_extension_agnostic() {
    assert!(is_service_token_review_path("src/service_token.rs"));
    assert!(is_service_token_review_path("src/service-token.py"));
    // Not adjacent: a `/` path boundary between the two tokens is not the
    // `-`/`_` boundary this reads within one segment.
    assert!(!is_service_token_review_path("service/token/store.rs"));
    assert!(!is_service_token_review_path("src/token_service.rs"));
    assert!(!is_service_token_review_path("src/service_only.rs"));
}

#[test]
fn shell_review_path_requires_the_exact_workflow_prefix() {
    assert!(is_shell_review_path("scripts/deploy.sh"));
    assert!(is_shell_review_path(".github/workflows/ci.yml"));
    assert!(is_shell_review_path(".github/workflows/release.yaml"));
    assert!(!is_shell_review_path("workflows/ci.yml"));
    assert!(!is_shell_review_path("config/app.yml"));
    assert!(!is_shell_review_path("docs/guide.md"));
}

#[test]
fn semantic_risk_eligibility_excludes_documentation_fixtures_and_readme() {
    let eligible = |path: &str| is_semantic_risk_eligible(&active_stat(path, 1, 0));
    assert!(eligible("src/service_token.rs"));
    assert!(eligible(".github/workflows/ci.yml"));
    assert!(!eligible("docs/service_token.rs"));
    assert!(!eligible("testdata/service_token.rs"));
    assert!(!eligible("pkg/fixtures/service_token.rs"));
    assert!(!eligible("README_service_token.py"));
    // Neither a semantic source extension nor a configuration path.
    assert!(!eligible("src/service_token.png"));
}

#[test]
fn operational_markdown_is_never_a_passive_extension() {
    for path in [
        "AGENTS.md",
        "CLAUDE.md",
        ".claude/commands/deploy.md",
        "prompts/system.md",
        "runtime/policy.md",
        "openspec/changes/example/proposal.md",
    ] {
        assert!(
            !is_passive_document_extension(path),
            "{path} was nominated passive"
        );
    }
    assert!(is_passive_document_extension("docs/guide.md"));
    assert!(is_passive_document_extension("book/chapter.mdx"));
}

/// Ports `isStaticMDXDocument` (`risk.go:990-1017`), unreachable from any
/// test before this: Judge B stubbed the whole function to `true` and the
/// suite stayed green. Each case below pins one line of the Go directly.
#[test]
fn static_mdx_document_detection_pins_every_disqualifying_construct() {
    // Ordinary prose, no runtime syntax anywhere: static.
    assert!(is_static_mdx_document(
        "Static prose line.\nAnother line.\n"
    ));

    // A bare `import` outside any fence disqualifies.
    assert!(!is_static_mdx_document(
        "import Widget from './widget'\n\nProse.\n"
    ));
    // Same for `export`.
    assert!(!is_static_mdx_document("export const x = 1\n\nProse.\n"));

    // The identical line, fenced, is skipped entirely and stays static.
    assert!(is_static_mdx_document(
        "Prose before.\n```\nimport Widget from './widget'\n```\nProse after.\n"
    ));
    assert!(is_static_mdx_document(
        "Prose before.\n~~~\nexport const x = 1\n~~~\nProse after.\n"
    ));
    // An unterminated fence still holds through the end of the document.
    assert!(is_static_mdx_document(
        "Prose.\n```\nimport Widget from './widget'\nexport const x = 1\n"
    ));

    // A brace or a tag outside a fence disqualifies, even with no keyword.
    assert!(!is_static_mdx_document("Prose with a { brace.\n"));
    assert!(!is_static_mdx_document("Prose with a } brace.\n"));
    assert!(!is_static_mdx_document("Prose with a <Tag /> in it.\n"));

    // The same characters inside a single-backtick code span are stripped
    // before the brace/tag check runs, so they do not disqualify.
    assert!(is_static_mdx_document(
        "Prose with `{ code }` and `<Tag/>` spans.\n"
    ));

    // A leading JavaScript block comment is stripped before the keyword
    // check, so `import` behind one still disqualifies.
    assert!(!is_static_mdx_document(
        "/* license */ import Widget from './widget'\n"
    ));

    // `startsJavaScriptKeyword` matches a whole word: `importance` is not
    // `import` followed by a non-identifier character.
    assert!(is_static_mdx_document("Prose about importance.\n"));
    assert!(is_static_mdx_document("Prose about exported goods.\n"));
}

#[test]
fn go_path_clean_matches_the_source_project_on_the_cases_normalize_checks() {
    assert_eq!(go_path_clean("a/./b"), "a/b");
    assert_eq!(go_path_clean("a//b"), "a/b");
    assert_eq!(go_path_clean("a/b/../c"), "a/c");
    assert_eq!(go_path_clean("../a"), "../a");
    assert_eq!(go_path_clean(".."), "..");
    assert!(normalize_logical_path("../etc/passwd").is_err());
    assert!(normalize_logical_path("a/../../b").is_err());
    assert!(normalize_logical_path("").is_err());
    assert!(normalize_logical_path("a\\b").is_err());
    assert!(normalize_logical_path("/etc/passwd").is_err());
    assert_eq!(
        normalize_logical_path("src/lib.rs").expect("clean"),
        "src/lib.rs"
    );
}

#[test]
fn the_tiers_license_zero_one_and_four_lenses() {
    assert_eq!(RiskLevel::Low.lenses(), 0);
    assert_eq!(RiskLevel::Medium.lenses(), 1);
    assert_eq!(RiskLevel::High.lenses(), 4);
    for focus in [
        ReviewFocus::Risk,
        ReviewFocus::Resilience,
        ReviewFocus::Readability,
        ReviewFocus::Reliability,
    ] {
        for level in [RiskLevel::Low, RiskLevel::Medium, RiskLevel::High] {
            assert_eq!(
                level.lens_roles(focus).len(),
                usize::from(level.lenses()),
                "{level:?} under {focus:?} names a different number of roles than it counts"
            );
        }
    }
}

#[test]
fn high_names_the_four_roles_with_resilience_filled_by_readback() {
    assert_eq!(
        RiskLevel::High.lens_roles(ReviewFocus::Readability),
        [
            "review-risk",
            "review-readback",
            "review-readability",
            "review-reliability"
        ]
    );
    assert!(
        !RiskLevel::High
            .lens_roles(ReviewFocus::default())
            .contains(&"review-lens")
    );
}

#[test]
fn medium_runs_the_focus_lens_and_reads_reliability_when_none_is_named() {
    assert_eq!(ReviewFocus::default(), ReviewFocus::Reliability);
    for (spelling, role) in [
        ("risk", "review-risk"),
        ("resilience", "review-readback"),
        ("readability", "review-readability"),
        ("reliability", "review-reliability"),
    ] {
        let focus = ReviewFocus::parse(Some(spelling)).expect("a named focus");
        assert_eq!(RiskLevel::Medium.lens_roles(focus), [role], "{spelling}");
    }
    assert_eq!(
        RiskLevel::Medium.lens_roles(ReviewFocus::parse(None).expect("the default")),
        ["review-reliability"]
    );
    assert!(RiskLevel::Low.lens_roles(ReviewFocus::Risk).is_empty());
}

#[test]
fn an_unknown_focus_is_a_read_failure_and_not_the_default() {
    for spelling in ["", "security", "Risk", "review-risk"] {
        assert!(
            ReviewFocus::parse(Some(spelling)).is_err(),
            "{spelling:?} was accepted as a focus"
        );
    }
}

#[test]
fn the_review_tier_answer_names_the_lens_roles_each_tier_licenses() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"start\n", false);
    let head = commit_file(at, "docs/guide.md", b"start\nmore prose\n", false);
    let low = review_tier(&test_context(at), &base, &head, None, Some("risk")).expect("an answer");
    assert_eq!(low["level"], "low", "{low}");
    assert_eq!(low["lenses"], 0, "{low}");
    assert_eq!(low["lens_roles"], serde_json::json!([]), "{low}");

    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let head = commit_file(at, "src/lib.rs", b"fn main() { let _ = 1; }\n", false);
    let context = test_context(at);
    let medium = review_tier(&context, &base, &head, None, None).expect("an answer");
    assert_eq!(medium["level"], "medium", "{medium}");
    assert_eq!(medium["lenses"], 1, "{medium}");
    assert_eq!(
        medium["lens_roles"],
        serde_json::json!(["review-reliability"]),
        "{medium}"
    );
    let focused = review_tier(&context, &base, &head, None, Some("risk")).expect("an answer");
    assert_eq!(
        focused["lens_roles"],
        serde_json::json!(["review-risk"]),
        "{focused}"
    );
    assert!(review_tier(&context, &base, &head, None, Some("security")).is_err());

    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/auth.rs", b"fn check() {}\n", false);
    let head = commit_file(at, "src/auth.rs", b"fn check() { verify(); }\n", false);
    let high = review_tier(&test_context(at), &base, &head, None, Some("risk")).expect("an answer");
    assert_eq!(high["level"], "high", "{high}");
    assert_eq!(high["lenses"], 4, "{high}");
    assert_eq!(
        high["lens_roles"],
        serde_json::json!([
            "review-risk",
            "review-readback",
            "review-readability",
            "review-reliability"
        ]),
        "{high}"
    );
}

fn tier_fixtures() -> [(&'static str, tempfile::TempDir, String, String); 3] {
    let low = repo();
    let low_base = commit_file(low.path(), "docs/guide.md", b"start\n", false);
    let low_head = commit_file(low.path(), "docs/guide.md", b"start\nmore prose\n", false);
    let medium = repo();
    let medium_base = commit_file(medium.path(), "src/lib.rs", b"fn main() {}\n", false);
    let medium_head = commit_file(
        medium.path(),
        "src/lib.rs",
        b"fn main() { let _ = 1; }\n",
        false,
    );
    let high = repo();
    let high_base = commit_file(high.path(), "src/auth.rs", b"fn check() {}\n", false);
    let high_head = commit_file(
        high.path(),
        "src/auth.rs",
        b"fn check() { verify(); }\n",
        false,
    );
    [
        ("low", low, low_base, low_head),
        ("medium", medium, medium_base, medium_head),
        ("high", high, high_base, high_head),
    ]
}

fn declared(
    at: &std::path::Path,
    base: &str,
    head: &str,
    declaration: &Declaration<'_>,
) -> Result<serde_json::Value, Failure> {
    review_tier_declared(&test_context(at), base, head, None, None, declaration)
}

fn refusal_of(result: Result<serde_json::Value, Failure>) -> serde_json::Value {
    match result {
        Err(Failure::ConfigDefect(payload)) => payload,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_raise_never_answers_a_tier_below_the_computed_one() {
    let rank = |word: &str| RiskLevel::parse(word).expect("a tier");
    for (computed, root, base, head) in tier_fixtures() {
        for item in RISK_ITEMS {
            let answer = declared(
                root.path(),
                &base,
                &head,
                &Declaration {
                    raise: Some(item),
                    raise_reason: Some("it moves an authorisation boundary"),
                    ..Declaration::default()
                },
            )
            .expect("an answer");
            let level = rank(answer["level"].as_str().expect("a level"));
            assert!(
                level >= rank(computed),
                "{item} lowered {computed}: {answer}"
            );
            assert_eq!(answer["computed_level"], computed, "{answer}");
            assert_eq!(answer["raise"], *item, "{answer}");
            assert_eq!(
                answer["raise_reason"], "it moves an authorisation boundary",
                "{answer}"
            );
            assert_eq!(level, RiskLevel::High, "{item} over {computed}: {answer}");
            assert_eq!(answer["lenses"], 4, "{answer}");
        }
    }
}

#[test]
fn an_answer_without_a_declaration_is_the_computed_one_and_carries_none_of_it() {
    for (computed, root, base, head) in tier_fixtures() {
        let answer =
            declared(root.path(), &base, &head, &Declaration::default()).expect("an answer");
        assert_eq!(answer["level"], computed, "{answer}");
        for key in ["computed_level", "raise", "raise_reason", "focus_reason"] {
            assert!(answer.get(key).is_none(), "{key} on {answer}");
        }
    }
}

#[test]
fn a_raise_that_would_lower_or_leaves_the_list_is_refused_with_its_own_reason() {
    let [_, (_, root, base, head), _] = tier_fixtures();
    for word in ["low", "medium", "none"] {
        let refusal = refusal_of(declared(
            root.path(),
            &base,
            &head,
            &Declaration {
                raise: Some(word),
                raise_reason: Some("why"),
                ..Declaration::default()
            },
        ));
        assert_eq!(refusal["reason"], "risk-raise-lowers", "{word}: {refusal}");
        assert!(refusal["action"].is_string(), "{refusal}");
    }
    let refusal = refusal_of(declared(
        root.path(),
        &base,
        &head,
        &Declaration {
            raise: Some("vibes"),
            raise_reason: Some("why"),
            ..Declaration::default()
        },
    ));
    assert_eq!(refusal["reason"], "invalid-risk-raise", "{refusal}");
    assert_eq!(
        refusal["expected"],
        serde_json::json!(RISK_ITEMS),
        "{refusal}"
    );
}

#[test]
fn a_raise_and_its_reason_travel_together() {
    let [_, (_, root, base, head), _] = tier_fixtures();
    for reason in [None, Some(""), Some("  ")] {
        let refusal = refusal_of(declared(
            root.path(),
            &base,
            &head,
            &Declaration {
                raise: Some("concurrency"),
                raise_reason: reason,
                ..Declaration::default()
            },
        ));
        assert_eq!(refusal["reason"], "risk-raise-reason-missing", "{refusal}");
    }
    let refusal = refusal_of(declared(
        root.path(),
        &base,
        &head,
        &Declaration {
            raise_reason: Some("why"),
            ..Declaration::default()
        },
    ));
    assert_eq!(
        refusal["reason"], "risk-raise-reason-without-raise",
        "{refusal}"
    );
}

#[test]
fn a_focus_reason_is_answered_back_and_needs_a_focus() {
    let [_, (_, root, base, head), _] = tier_fixtures();
    let context = test_context(root.path());
    let answer = review_tier_declared(
        &context,
        &base,
        &head,
        None,
        Some("risk"),
        &Declaration {
            focus_reason: Some("the diff edits a lock order"),
            ..Declaration::default()
        },
    )
    .expect("an answer");
    assert_eq!(
        answer["focus_reason"], "the diff edits a lock order",
        "{answer}"
    );
    assert_eq!(
        answer["lens_roles"],
        serde_json::json!(["review-risk"]),
        "{answer}"
    );
    let refusal = refusal_of(declared(
        root.path(),
        &base,
        &head,
        &Declaration {
            focus_reason: Some("why"),
            ..Declaration::default()
        },
    ));
    assert_eq!(refusal["reason"], "focus-reason-without-focus", "{refusal}");
    let refusal = refusal_of(review_tier_declared(
        &context,
        &base,
        &head,
        None,
        Some("risk"),
        &Declaration {
            focus_reason: Some(" "),
            ..Declaration::default()
        },
    ));
    assert_eq!(refusal["reason"], "blank-focus-reason", "{refusal}");
}

#[test]
fn a_raise_reaches_the_repair_answer_and_keeps_the_whole_panel() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let parent = commit_file(at, "src/lib.rs", b"fn main() { let _ = 1; }\n", false);
    let head = commit_file(at, "src/lib.rs", b"fn main() { let _ = 2; }\n", false);
    let answer = review_tier_for_repair_declared(
        &test_context(at),
        &base,
        &parent,
        &head,
        None,
        Some("warning"),
        None,
        &Declaration {
            raise: Some("migration"),
            raise_reason: Some("it rewrites stored rows"),
            ..Declaration::default()
        },
    )
    .expect("an answer");
    assert_eq!(answer["computed_level"], "medium", "{answer}");
    assert_eq!(answer["level"], "high", "{answer}");
    assert_eq!(answer["lenses"], 4, "{answer}");
    assert!(answer.get("panel").is_none(), "{answer}");
}

#[test]
fn the_review_tier_answer_states_the_forecast_line_for_each_tier() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"start\n", false);
    let head = commit_file(at, "docs/guide.md", b"start\nmore prose\n", false);
    let low = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(
        low["forecast"],
        "low: no lenses, about 0 tokens against a Review budget of 150000 per context",
        "{low}"
    );

    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let head = commit_file(at, "src/lib.rs", b"fn main() { let _ = 1; }\n", false);
    let medium =
        review_tier(&test_context(at), &base, &head, None, Some("risk")).expect("an answer");
    assert_eq!(
        medium["forecast"],
        "medium: 1 lens (review-risk), about 150000 tokens against a Review budget of 150000 per context",
        "{medium}"
    );
    let mut budgeted = test_context(at);
    budgeted.config = vec![(
        "Review budget".to_owned(),
        "100000".to_owned(),
        crate::transport::Source::Written,
    )];
    let medium = review_tier(&budgeted, &base, &head, None, None).expect("an answer");
    assert_eq!(
        medium["forecast"],
        "medium: 1 lens (review-reliability), about 100000 tokens against a Review budget of 100000 per context",
        "{medium}"
    );
    budgeted.config[0].1 = "none".to_owned();
    let medium = review_tier(&budgeted, &base, &head, None, None).expect("an answer");
    assert_eq!(
        medium["forecast"],
        "medium: 1 lens (review-reliability), no Review budget is declared, so no spend is forecast",
        "{medium}"
    );

    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/auth.rs", b"fn check() {}\n", false);
    let head = commit_file(at, "src/auth.rs", b"fn check() { verify(); }\n", false);
    let high = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(
        high["forecast"],
        "high: 4 lenses (review-risk, review-readback, review-readability, review-reliability), about 600000 tokens against a Review budget of 150000 per context",
        "{high}"
    );
}

/// Ordinary review licenses lenses only, at every tier and on every road that
/// answers a tier: no `judges` key, and a forecast that counts lenses alone.
#[test]
fn the_review_tier_answer_carries_no_judges_at_any_tier() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"start\n", false);
    let body = "Ordinary prose line.\n".repeat(900);
    let head = commit_file(at, "docs/guide.md", body.as_bytes(), false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "low", "{answer}");
    assert!(answer.get("judges").is_none(), "{answer}");

    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
    assert!(answer.get("judges").is_none(), "{answer}");
    assert!(
        !answer["forecast"].as_str().unwrap().contains("judge"),
        "{answer}"
    );

    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/auth.rs", b"fn check() {}\n", false);
    let head = commit_file(at, "src/auth.rs", b"fn check() { verify(); }\n", false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "high", "{answer}");
    assert!(answer.get("judges").is_none(), "{answer}");
}

// ---------------------------------------------------------------------------
// Real repositories: byte-content proof and the public entry point.
// ---------------------------------------------------------------------------

fn git(at: &std::path::Path, args: &[&str]) {
    crate::transport::run(args, Some(at), crate::transport::How::write())
        .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
}

fn oid(at: &std::path::Path) -> String {
    crate::transport::run(
        &["git", "rev-parse", "HEAD"],
        Some(at),
        crate::transport::How::read(),
    )
    .expect("HEAD")
    .stdout
    .trim()
    .to_owned()
}

fn repo() -> tempfile::TempDir {
    static TEMPLATES: crate::test_env::Templates<()> = crate::test_env::Templates::new();
    let (root, ()) = TEMPLATES.copy("seeded", |at| {
        git(at, &["git", "init", "--quiet", "-b", "main", "."]);
        git(
            at,
            &["git", "config", "user.email", "estigia@example.invalid"],
        );
        git(at, &["git", "config", "user.name", "Estigia test"]);
        git(at, &["git", "config", "core.autocrlf", "false"]);
        std::fs::write(at.join(".gitkeep"), "").expect("the seed file");
        git(at, &["git", "add", "."]);
        git(at, &["git", "commit", "--quiet", "-m", "seed"]);
    });
    root
}

/// Writes `path`, stages it, marks it executable when asked, and commits.
/// `content` is written as raw bytes so a caller can seed invalid UTF-8 or a
/// shebang without going through a `&str`.
fn commit_file(at: &std::path::Path, path: &str, content: &[u8], executable: bool) -> String {
    let full = at.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).expect("the parent directory");
    }
    std::fs::write(&full, content).expect("the file");
    git(at, &["git", "add", "--", path]);
    if executable {
        git(at, &["git", "update-index", "--chmod=+x", "--", path]);
    }
    git(at, &["git", "commit", "--quiet", "-m", "change"]);
    oid(at)
}

/// Ports `TestTierIsClassifiedByContentNotExtension`'s "executable Markdown"
/// case (`risk_test.go:665`): the extension nominates, the bytes decide. A
/// `.md` file beginning `#!` is a process boundary regardless of its name.
#[test]
fn a_shebang_withdraws_the_passive_nomination_even_for_markdown() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);
    let head = commit_file(
        at,
        "docs/guide.md",
        b"#!/usr/bin/env bash\necho hello\n",
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// Ports the "executable Markdown" mode case from
/// `TestIsPassiveContentCandidateStat` (`risk_test.go` mode table): an
/// executable bit withdraws the nomination even when the bytes are ordinary
/// prose. Both ends sit at the same executable mode, so this isolates the
/// nomination withdrawal from the mode-flip producer covered separately by
/// `a_mode_flip_is_high_even_inside_an_excluded_segment` below.
#[test]
fn an_executable_bit_withdraws_the_passive_nomination_even_for_markdown() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", true);
    let head = commit_file(at, "docs/guide.md", b"Ordinary prose, edited.\n", true);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// Ports `executableModeChanged` (`risk.go:713-720`): an execute-bit flip is
/// evidence on its own, carrying no eligibility gate — a chmod inside `docs/`,
/// which `is_semantic_risk_eligible` would otherwise exclude, still escalates.
#[test]
fn a_mode_flip_is_high_even_inside_an_excluded_segment() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/notes.txt", b"Ordinary notes.\n", false);
    let head = commit_file(at, "docs/notes.txt", b"Ordinary notes.\n", true);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "high", "{answer}");
    let reasons = answer["reasons"].as_array().expect("reasons");
    assert!(
        reasons
            .iter()
            .any(|reason| reason["code"] == "executable_mode" && reason["signal"] == "permissions"),
        "the reasons do not name the mode flip: {reasons:?}"
    );
}

/// Ports `isShellReviewPath`'s workflow case (`risk.go:706-707`): this
/// repository's own publication lane is a `.yml` file, which the
/// configuration exclusion alone would read as `medium`. The shell-source
/// producer is what keeps it `high`.
#[test]
fn a_change_to_the_publication_lane_workflow_is_high() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, ".github/workflows/ci.yml", b"name: ci\n", false);
    let head = commit_file(
        at,
        ".github/workflows/ci.yml",
        b"name: ci\non: push\n",
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "high", "{answer}");
    let reasons = answer["reasons"].as_array().expect("reasons");
    assert!(
        reasons
            .iter()
            .any(|reason| reason["code"] == "shell_source" && reason["signal"] == "shell_process"),
        "the reasons do not name the workflow as shell source: {reasons:?}"
    );
}

/// A `.yml` outside `.github/workflows/` is ordinary configuration, not shell
/// source — the prefix in `isShellReviewPath` is exact.
#[test]
fn a_yaml_file_outside_the_workflow_prefix_is_not_shell_source() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "config/app.yml", b"a: 1\n", false);
    let head = commit_file(at, "config/app.yml", b"a: 2\n", false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// Ports `isServiceTokenReviewPath` (`risk.go:693-700`): adjacent `service`
/// and `token` path tokens, matched after `strip_semantic_path_extension`
/// strips a `.rs` source extension. `service_token.rs` carries no hot-path
/// token of its own (`auth`, `update`, `security`, `webhook`, `payments`), so
/// a `high` answer here is this producer and nothing else.
#[test]
fn a_service_token_shaped_source_path_is_high() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/service_token.rs", b"fn a() {}\n", false);
    let head = commit_file(
        at,
        "src/service_token.rs",
        b"fn a() { let b = 1; }\n",
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "high", "{answer}");
    let reasons = answer["reasons"].as_array().expect("reasons");
    assert!(
        reasons
            .iter()
            .any(|reason| reason["code"] == "service_token" && reason["signal"] == "auth"),
        "the reasons do not name the service-token path: {reasons:?}"
    );
}

/// The eligibility gate `is_semantic_risk_eligible` excludes a `docs/`
/// segment — the same file, moved under `docs/`, no longer escalates on its
/// name, even though `has_adjacent_service_token_tokens` would still match it.
#[test]
fn a_service_token_shaped_path_under_docs_does_not_escalate() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/service_token.rs", b"fn a() {}\n", false);
    let head = commit_file(
        at,
        "docs/service_token.rs",
        b"fn a() { let b = 1; }\n",
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// The eligibility gate's `readme`-prefix exclusion: the same adjacent
/// tokens, in a basename the gate reads as documentation regardless of its
/// extension.
#[test]
fn a_readme_prefixed_basename_does_not_escalate_on_its_name() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "README_service_token.py", b"a = 1\n", false);
    let head = commit_file(at, "README_service_token.py", b"a = 2\n", false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// Fail-closed on content this transport cannot decode as UTF-8: `\xff\xfe` is
/// two invalid subparts, each decoded to `U+FFFD`.
#[test]
fn invalid_utf8_content_fails_closed_rather_than_reading_as_passive() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);
    let head = commit_file(at, "docs/guide.md", b"prose \xff\xfe garbage\n", false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// The length-preserving case: `\xf0\x9f\x98` is the three leading bytes of a
/// four-byte sequence and `A` is not a valid continuation, so those three
/// bytes become one three-byte `U+FFFD`. Decoded length equals raw length, so
/// only a direct `U+FFFD` check in the decoded text catches it.
#[test]
fn a_length_preserving_invalid_subpart_still_fails_closed() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);
    let head = commit_file(at, "docs/guide.md", b"prose \xf0\x9f\x98A more\n", false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// The named divergence from `utf8.Valid`: valid UTF-8 that legitimately
/// **contains** the replacement character reads exactly like lossily-decoded
/// damage, because both arrive as the same `U+FFFD`, and this function cannot
/// tell them apart once they have. `medium`, not `low` — fail-closed, and
/// pinned here so it is a stated property rather than an incidental one.
#[test]
fn a_document_legitimately_containing_the_replacement_character_is_not_passive() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);
    let head = commit_file(
        at,
        "docs/guide.md",
        "prose \u{fffd} on purpose\n".as_bytes(),
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
}

/// The end-to-end low case, at a size well past the retired 400-line
/// boundary, proving the port end to end and not only at the classifier's
/// pure layer.
#[test]
fn a_large_ordinary_documentation_change_is_low_end_to_end() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "docs/guide.md", b"start\n", false);
    let body = "Ordinary prose line.\n".repeat(900);
    let head = commit_file(at, "docs/guide.md", body.as_bytes(), false);
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "low", "{answer}");
    assert!(answer["changed_lines"].as_u64().expect("a count") >= 900);
}

/// A repair round is sized by its own delta, not by the whole
/// change it sits on top of. The whole range answers `medium` — it touches a
/// `.rs` file, which is never passive — while the repair itself only adds
/// passive prose on top of that already-published code change, and
/// [`review_tier_for_repair`] answers `low` for that narrower range, naming
/// it `range: "repair"` with no `whole_change_reason`, while [`review_tier`]
/// still answers `medium` for the whole one. Strictly linear history:
/// `parent_head` is still an ancestor of `head` and the narrow range is
/// non-empty, so this alone cannot fail for either reason the two
/// repair rounds exist —
/// `a_rebased_repairs_own_range_is_not_recoverable_so_it_answers_the_whole_change_instead`
/// and
/// `an_identical_bytes_republish_has_no_repair_range_to_measure_so_it_answers_the_whole_change_instead`
/// below are what prove the guards that matter.
#[test]
fn a_repair_rounds_own_delta_answers_low_while_the_whole_change_answers_medium() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    let repair_head = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);

    let context = test_context(at);
    let whole = review_tier(&context, &base, &repair_head, None, None).expect("an answer");
    assert_eq!(whole["level"], "medium", "{whole}");
    assert!(whole.get("range").is_none(), "{whole}");

    let repair = review_tier_for_repair(
        &context,
        &base,
        &parent_head,
        &repair_head,
        None,
        None,
        None,
    )
    .expect("an answer over the repair's own range");
    assert_eq!(repair["level"], "low", "{repair}");
    assert_eq!(repair["range"], "repair", "{repair}");
    assert!(repair.get("whole_change_reason").is_none(), "{repair}");
    assert!(
        repair.get("panel").is_none(),
        "no clearing_finding was named, so no lens override should appear: {repair}"
    );
}

/// A repair of a `warning` or a `suggestion` after an accepted
/// verdict is reviewed by the delta lens alone, once the narrow range is the
/// honest one — `panel: "lens"`, one lens, in place of `level`'s
/// own allocation (`src/lib.rs` here is never passive, so the whole range and
/// the repair range both carry a medium-or-higher level absent the override, and
/// the fixture proves the override rather than the tier's own answer).
#[test]
fn a_warning_or_suggestion_cleared_narrow_repair_is_reviewed_by_the_lens_alone() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    let repair_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi there\"); }\n",
        false,
    );
    let context = test_context(at);

    for class in ["warning", "suggestion"] {
        let repair = review_tier_for_repair(
            &context,
            &base,
            &parent_head,
            &repair_head,
            None,
            Some(class),
            None,
        )
        .expect("a warning or suggestion clearing_finding is not refused");
        assert_eq!(repair["range"], "repair", "{class}: {repair}");
        assert_eq!(repair["panel"], "lens", "{class}: {repair}");
        assert_eq!(repair["lenses"], 1, "{class}: {repair}");
        assert_eq!(
            repair["lens_roles"],
            serde_json::json!(["review-lens"]),
            "{class}: {repair}"
        );
        assert!(repair.get("judges").is_none(), "{class}: {repair}");
    }

    let severe = review_tier_for_repair(
        &context,
        &base,
        &parent_head,
        &repair_head,
        None,
        Some("severe"),
        None,
    )
    .expect("a severe clearing_finding is not refused");
    assert!(
        severe.get("panel").is_none(),
        "a severe clearing finding keeps the tier's own allocation: {severe}"
    );
    let ordinary = review_tier_for_repair(
        &context,
        &base,
        &parent_head,
        &repair_head,
        None,
        None,
        None,
    )
    .expect("no clearing_finding at all is not refused");
    assert!(severe.get("judges").is_none(), "{severe}");
    assert!(ordinary.get("judges").is_none(), "{ordinary}");
    assert_eq!(
        severe["lenses"], ordinary["lenses"],
        "a severe clearing finding must answer the tier's own lenses, unchanged: {severe}"
    );
}

/// A repair answer states the forecast line over the panel it actually answers: the
/// tier's own for an ordinary repair, and the one delta lens once a `warning` or
/// `suggestion` cleared the gate. Forecasting before the lens override would still
/// price the tier's own lenses, not the one the answer licenses.
#[test]
fn a_repair_answer_forecasts_the_panel_it_licenses_and_the_lens_alone_prices_one_lens() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    let repair_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi there\"); }\n",
        false,
    );
    let repair = |context: &Context, clearing: Option<&str>| {
        review_tier_for_repair(
            context,
            &base,
            &parent_head,
            &repair_head,
            None,
            clearing,
            None,
        )
        .expect("an answer over the repair's own range")
    };
    let context = test_context(at);

    let ordinary = repair(&context, None);
    assert_eq!(
        ordinary["forecast"],
        "medium: 1 lens (review-reliability), about 150000 tokens against a Review budget of 150000 per context",
        "{ordinary}"
    );
    let severe = repair(&context, Some("severe"));
    assert_eq!(severe["forecast"], ordinary["forecast"], "{severe}");

    let lens_only = repair(&context, Some("warning"));
    assert_eq!(lens_only["panel"], "lens", "{lens_only}");
    assert_eq!(
        lens_only["forecast"],
        "medium: 1 lens (review-lens), about 150000 tokens against a Review budget of 150000 per context",
        "{lens_only}"
    );

    let mut unbudgeted = test_context(at);
    unbudgeted.config = vec![(
        "Review budget".to_owned(),
        "none".to_owned(),
        crate::transport::Source::Written,
    )];
    let lens_only = repair(&unbudgeted, Some("suggestion"));
    assert_eq!(
        lens_only["forecast"],
        "medium: 1 lens (review-lens), no Review budget is declared, so no spend is forecast",
        "{lens_only}"
    );
}

/// A `warning`-cleared repair whose own delta is `high` keeps the
/// tier's own panel — the hot-path shape is reviewed by the tier's lenses,
/// not by the delta lens alone.
#[test]
fn a_warning_cleared_high_repair_keeps_the_tier_panel() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/auth.rs", b"fn check() {}\n", false);
    let parent_head = commit_file(at, "src/auth.rs", b"fn check() { verify(); }\n", false);
    let repair_head = commit_file(
        at,
        "src/auth.rs",
        b"fn check() { verify(); audit(); }\n",
        false,
    );
    let context = test_context(at);

    let repair = review_tier_for_repair(
        &context,
        &base,
        &parent_head,
        &repair_head,
        None,
        Some("warning"),
        None,
    )
    .expect("a warning clearing_finding is not refused");
    assert_eq!(repair["level"], "high", "{repair}");
    assert_eq!(repair["range"], "repair", "{repair}");
    assert!(
        repair.get("panel").is_none(),
        "a high repair keeps the tier's lenses even when a warning cleared it: {repair}"
    );
    assert_eq!(repair["lenses"], 4, "{repair}");
}

/// An unknown `clearing_finding` is refused by name, naming the vocabulary it
/// must come from — the same three words [`super::super::claim::FINDING_CLASSES`]
/// already enforces on the finding itself.
#[test]
fn an_unknown_clearing_finding_class_is_refused() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    let repair_head = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);
    let context = test_context(at);

    let failure = review_tier_for_repair(
        &context,
        &base,
        &parent_head,
        &repair_head,
        None,
        Some("critical"),
        None,
    )
    .expect_err("an unrecognised clearing_finding class was accepted");
    let message = failure.detail();
    assert!(
        message.contains("severe") && message.contains("warning") && message.contains("suggestion"),
        "the refusal does not name the allowed values: {message}"
    );
}

/// The panel that rejected the first shape proved a frozen
/// candidate tree is not a defence against base movement — `tree(A)` is the
/// tree *of a commit*, and `git diff tree(A) tree(B)` is `git diff A B`, so
/// the pair that shape prescribed was `parent_head..head` under a different
/// name. This is the case that shape answered wrong: `main` advances with
/// passive prose, the branch is rebased onto it, and the rebased repair head
/// no longer has the old `parent_head` as an ancestor. The base advance is
/// passive on purpose: `parent_head..head` still touches only passive-shaped
/// paths (the base's own prose addition plus the repair's own), so a bug
/// that ignored the ancestry guard and read that range anyway would answer a
/// false `low` rather than merely a different `medium` — the level itself
/// discriminates the guard here, not only the field naming which range was
/// measured. The honest answer reads the whole `base..head` instead,
/// `medium` because it is the one range that still carries the `.rs` edit,
/// `range: "whole-change"`, `whole_change_reason: "history-rewritten"`.
#[test]
fn a_rebased_repairs_own_range_is_not_recoverable_so_it_answers_the_whole_change_instead() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );

    // `main` advances past `base` with passive prose, on a path the branch
    // above never touches, so replaying the branch's own edits onto it is
    // conflict-free.
    git(at, &["git", "checkout", "--quiet", "--detach", &base]);
    let advanced_base = commit_file(at, "docs/other.md", b"Ordinary prose.\n", false);

    // The rebase itself: the branch's two commits recreated on top of the
    // advanced base, as new commit objects — exactly what a real rebase
    // does, and why `parent_head` can never be their ancestor again.
    commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    let rebased_repair_head = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);

    let context = test_context(at);
    assert!(
        !crate::transport::target::is_ancestor(at, &parent_head, &rebased_repair_head)
            .expect("ancestry reads"),
        "the fixture did not rebase away from parent_head; the test proves nothing"
    );

    let repair = review_tier_for_repair(
        &context,
        &advanced_base,
        &parent_head,
        &rebased_repair_head,
        None,
        None,
        None,
    )
    .expect("an answer over the whole change, since the narrow range is not recoverable");
    assert_eq!(repair["level"], "medium", "{repair}");
    assert_eq!(repair["range"], "whole-change", "{repair}");
    assert_eq!(
        repair["whole_change_reason"], "history-rewritten",
        "{repair}"
    );

    // A warning-cleared repair still keeps the tier's own allocation once the
    // narrow range is not recoverable: the delta lens only ever substitutes
    // for a range that was actually measured.
    let warning_cleared = review_tier_for_repair(
        &context,
        &advanced_base,
        &parent_head,
        &rebased_repair_head,
        None,
        Some("warning"),
        None,
    )
    .expect("an answer over the whole change is still not refused with a clearing_finding");
    assert!(
        warning_cleared.get("panel").is_none(),
        "the whole-change fallback must keep the tier's own allocation: {warning_cleared}"
    );
    assert_eq!(
        warning_cleared["whole_change_reason"], "history-rewritten",
        "{warning_cleared}"
    );
}

/// A moved base merged into the branch is read whole, never by the delta lens,
/// whether the caller passes the current base or an older one the head already
/// merged past. Only the current base moved the fork point, so only that one is
/// a base advance; the older base leaves the fork point inside `parent_head`
/// and is a merged range.
#[test]
fn a_merged_base_advance_is_read_whole_and_never_gets_the_delta_lens() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    git(at, &["git", "checkout", "--quiet", "-b", "feature"]);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    git(at, &["git", "checkout", "--quiet", "main"]);
    let advanced_base = commit_file(at, "docs/other.md", b"Ordinary prose.\n", false);
    git(at, &["git", "checkout", "--quiet", "feature"]);
    git(at, &["git", "merge", "--quiet", "--no-edit", "main"]);
    let repair_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi there\"); }\n",
        false,
    );

    let context = test_context(at);
    assert!(
        crate::transport::target::is_ancestor(at, &parent_head, &repair_head)
            .expect("ancestry reads"),
        "the fixture rewrote history; a merge keeps the parent head an ancestor"
    );
    for (clearing, passed_base, reason) in [
        (None, &advanced_base, "base-advanced"),
        (Some("warning"), &advanced_base, "base-advanced"),
        (Some("suggestion"), &advanced_base, "base-advanced"),
        (Some("warning"), &base, "merged-range"),
    ] {
        let answer = review_tier_for_repair(
            &context,
            passed_base,
            &parent_head,
            &repair_head,
            None,
            clearing,
            None,
        )
        .expect("an answer over the whole change");
        assert_eq!(answer["range"], "whole-change", "{answer}");
        assert_eq!(answer["whole_change_reason"], reason, "{answer}");
        assert!(
            answer.get("panel").is_none(),
            "a moved base was granted the delta lens: {answer}"
        );
        assert!(answer.get("judges").is_none(), "{answer}");
    }
}

/// A side branch merged back with `--no-ff` while the base never moved keeps
/// the fork point inside `parent_head`'s history, so the whole change is read
/// for a reason that says what it actually was — a merge in the range, not a
/// base that advanced.
#[test]
fn a_side_branch_merge_with_an_unmoved_base_is_a_merged_range() {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/lib.rs", b"fn main() {}\n", false);
    git(at, &["git", "checkout", "--quiet", "-b", "feature"]);
    let parent_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi\"); }\n",
        false,
    );
    git(at, &["git", "checkout", "--quiet", "-b", "side"]);
    commit_file(at, "docs/side.md", b"Side work.\n", false);
    git(at, &["git", "checkout", "--quiet", "feature"]);
    git(
        at,
        &["git", "merge", "--quiet", "--no-ff", "--no-edit", "side"],
    );
    let repair_head = commit_file(
        at,
        "src/lib.rs",
        b"fn main() { println!(\"hi there\"); }\n",
        false,
    );

    let context = test_context(at);
    assert!(
        crate::transport::target::is_ancestor(at, &base, &parent_head).expect("ancestry reads"),
        "the fixture moved the base; the test proves nothing"
    );
    assert!(
        crate::transport::target::is_ancestor(at, &parent_head, &repair_head)
            .expect("ancestry reads"),
        "the fixture rewrote history; a merge keeps the parent head an ancestor"
    );
    let answer = review_tier_for_repair(
        &context,
        &base,
        &parent_head,
        &repair_head,
        None,
        None,
        None,
    )
    .expect("an answer over the whole change");
    assert_eq!(answer["range"], "whole-change", "{answer}");
    assert_eq!(answer["whole_change_reason"], "merged-range", "{answer}");
    assert!(
        answer.get("panel").is_none(),
        "a merged range was granted the delta lens: {answer}"
    );
}

/// `git merge-base --is-ancestor A A`
/// exits `0`, so an identical-bytes republish — `parent_head == head`, a
/// real input the contract itself names, since a republish invalidates
/// evidence "even when the bytes are identical" — makes the ancestry guard
/// alone insufficient. `parent_head..head` is then empty: no path changed, so
/// `fallback_risk_reason` answers `nothing_measured` and the range carries no
/// evidence about the repair's own edits at all.
/// A range with no changed paths gives no repair-scoped tier to derive,
/// so the honest answer falls back to the whole `base..head`
/// instead: here `high`, from the same `auth` hot-path token the whole
/// range already carried, `range: "whole-change"`,
/// `whole_change_reason: "empty-repair-range"` — never the `low` an empty
/// diff would otherwise manufacture.
#[test]
fn an_identical_bytes_republish_has_no_repair_range_to_measure_so_it_answers_the_whole_change_instead()
 {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, "src/auth.rs", b"fn check() {}\n", false);
    let head = commit_file(at, "src/auth.rs", b"fn check() { verify(); }\n", false);

    let context = test_context(at);
    let whole = review_tier(&context, &base, &head, None, None).expect("an answer");
    assert_eq!(whole["level"], "high", "{whole}");

    let repair = review_tier_for_repair(&context, &base, &head, &head, None, None, None)
        .expect("an answer over the whole change, since the repair range measured nothing");
    assert_eq!(repair["level"], "high", "{repair}");
    assert_eq!(repair["range"], "whole-change", "{repair}");
    assert_eq!(
        repair["whole_change_reason"], "empty-repair-range",
        "{repair}"
    );
}

/// A path that cannot be normalised is a failed read, never a tier: this is
/// the strong form of fail-closed, refusing an answer rather than guessing
/// one. `git diff -z` never emits the bytes that would provoke this in
/// practice, so it is driven directly against [`normalize_logical_path`]
/// rather than through a repository.
#[test]
fn an_unnormalisable_path_refuses_rather_than_answers() {
    assert!(normalize_logical_path("../escape.md").is_err());
}

// ---------------------------------------------------------------------------
// An empty measurement is not a passive one.
// ---------------------------------------------------------------------------

/// Drives `classify_risk` directly over an empty stat set — the shape a
/// caller reaches with `base == head`, or any pair whose diff touches
/// nothing. `only_passive` is vacuously true over an empty slice, which is
/// exactly the trap: this asserts the tier does not fall out of that vacuous
/// truth. Deliberately does not read `risk_level_from_reasons` at all — that
/// cross-check is [`the_two_classifiers_agree_the_empty_stat_set_is_not_the_zero_tier`]'s
/// job, kept separate so this one stays green when only that other function
/// is broken, proving the two are exercised independently.
#[test]
fn classify_risk_over_an_empty_stat_set_never_answers_low() {
    let stats: [DiffStat; 0] = [];
    let reasons: [RiskReason; 0] = [];
    let only_passive = stats.iter().all(is_passive_content_candidate_stat);
    let level = classify_risk(&stats, &reasons, only_passive, false);
    assert_ne!(level, RiskLevel::Low, "an empty measurement answered low");
    assert_eq!(level, RiskLevel::Medium, "{level:?}");
}

/// The agreement `classify_risk` and `risk_level_from_reasons` must keep on
/// the empty case: both take the *same* reasons `derive_snapshot_risk_reasons`
/// answers over an empty stat set, and this asserts they agree without going
/// through [`review_tier_answer`]'s own fail-closed cross-check — an edit
/// that desyncs one of the two from the other must redden this test, not
/// only the harder-to-read internal-error path in
/// [`an_empty_range_answers_medium_naming_that_nothing_was_measured`] below.
#[test]
fn the_two_classifiers_agree_the_empty_stat_set_is_not_the_zero_tier() {
    let active_content = BTreeSet::new();
    let reasons = derive_snapshot_risk_reasons(&[], &active_content, Vec::new());
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert_eq!(reasons[0].code, "nothing_measured", "{reasons:?}");

    let stats: [DiffStat; 0] = [];
    let only_passive = stats.iter().all(is_passive_content_candidate_stat);
    let touches_configuration = false;
    let level = classify_risk(&stats, &reasons, only_passive, touches_configuration);
    let explained = risk_level_from_reasons(&reasons);
    assert_eq!(level, RiskLevel::Medium, "{level:?}");
    assert_eq!(explained, level, "{explained:?} != {level:?}");
}

/// The end-to-end case: `review_tier` over `base == head` answers `medium`,
/// one lens, zero changed lines, and a reason naming that nothing was
/// measured — never the `low`, zero-lens answer the vacuous-truth defect
/// produced.
#[test]
fn an_empty_range_answers_medium_naming_that_nothing_was_measured() {
    let root = repo();
    let at = root.path();
    let head = commit_file(at, "docs/guide.md", b"Ordinary prose.\n", false);

    let answer = review_tier(&test_context(at), &head, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
    assert_eq!(answer["lenses"], 1, "{answer}");
    assert_eq!(answer["changed_lines"], 0, "{answer}");
    let reasons = answer["reasons"].as_array().expect("reasons");
    assert!(
        reasons
            .iter()
            .any(|reason| reason["code"] == "nothing_measured"),
        "the reasons do not name the empty range: {reasons:?}"
    );
}

fn test_context(repo_dir: &std::path::Path) -> Context {
    Context {
        skill_dir: repo_dir.to_path_buf(),
        repo_dir: repo_dir.to_path_buf(),
        config: Vec::new(),
        repo: None,
    }
}

/// Ports `CorrectionBudget` (`risk.go:227-232`) and its cap, which the
/// source keeps at `risk.go:20` rather than beside the formula.
#[test]
fn correction_budget_rounds_odd_counts_up_and_caps_at_two_hundred() {
    assert_eq!(correction_budget(0), 0);
    assert_eq!(correction_budget(1), 1);
    assert_eq!(correction_budget(3), 2);
    assert_eq!(correction_budget(4), 2);
    assert_eq!(correction_budget(399), 200);
    assert_eq!(correction_budget(400), 200);
    assert_eq!(correction_budget(1_000_000), 200);
}

/// Ports `CompactCorrectionBudget` (`risk.go:241-250`), the floor the source
/// applies over the legacy formula rather than in place of it.
#[test]
fn floor_two_never_lets_a_positive_original_freeze_a_budget_below_two() {
    assert_eq!(floor_two(0), 0);
    assert_eq!(floor_two(1), 2);
    assert_eq!(floor_two(2), 2);
    assert_eq!(floor_two(3), 2);
    assert_eq!(floor_two(4), 2);
    assert_eq!(floor_two(1_000_000), 200);
}

/// Ports `resolveTree` (`snapshot.go:1266-1272`): the `^{tree}` peel is what
/// collapses a ref or a commit to the object the measurement is taken between.
#[test]
fn tree_of_resolves_a_commit_to_its_tree_and_refuses_an_unresolvable_rev() {
    let root = repo();
    let at = root.path();
    let context = test_context(at);
    let head = commit_file(at, "a.txt", b"same\n", false);
    let tree = tree_of(&context, Some(at), &head).expect("a tree");
    assert!(
        crate::transport::target::is_object_id(&tree),
        "tree_of did not answer a full object id: {tree:?}"
    );
    assert!(tree_of(&context, Some(at), "not-a-real-rev").is_err());
}

/// Ports `CountChangedLines`'s binary and mode-only exclusion arms
/// (`risk.go:217-219`), not its third, generated-golden arm.
#[test]
fn tree_pair_changed_lines_excludes_binary_and_mode_only_changes() {
    let root = repo();
    let at = root.path();
    let context = test_context(at);
    let base = commit_file(at, "text.txt", b"one\ntwo\n", false);
    git(at, &["git", "update-index", "--chmod=+x", "--", ".gitkeep"]);
    git(
        at,
        &[
            "git",
            "-c",
            "user.email=estigia@example.invalid",
            "-c",
            "user.name=Estigia test",
            "commit",
            "--quiet",
            "-m",
            "mode flip",
        ],
    );
    std::fs::write(at.join("text.txt"), b"one\ntwo\nthree\nfour\n").expect("more text");
    git(at, &["git", "add", "text.txt"]);
    std::fs::write(at.join("pic.bin"), [0u8, 159, 146, 150]).expect("binary bytes");
    git(at, &["git", "add", "pic.bin"]);
    git(
        at,
        &[
            "git",
            "-c",
            "user.email=estigia@example.invalid",
            "-c",
            "user.name=Estigia test",
            "commit",
            "--quiet",
            "-m",
            "text and binary",
        ],
    );
    let head = oid(at);

    let base_tree = tree_of(&context, Some(at), &base).expect("the base tree");
    let head_tree = tree_of(&context, Some(at), &head).expect("the head tree");
    assert_eq!(
        tree_pair_changed_lines(&context, Some(at), &base_tree, &head_tree, &[]).expect("a count"),
        2,
        "two added text lines, with the binary file and the mode flip excluded"
    );
    let paths = tree_pair_changed_paths(&context, Some(at), &base_tree, &head_tree)
        .expect("the changed paths");
    assert!(
        paths.contains(&"pic.bin".to_owned()) && paths.contains(&"text.txt".to_owned()),
        "the mode-only and binary paths must still be reported, just excluded from the sum: {paths:?}"
    );
}

/// A rename is two independent paths to `diff_stats` (`--no-renames`), but the
/// integration admits the destination beside the conflicted paths, so this
/// reads it back as the `new` side of `-M`'s rename record.
#[test]
fn tree_pair_rename_destinations_reports_the_new_side_of_a_rename() {
    let root = repo();
    let at = root.path();
    let context = test_context(at);
    // `-M` overrides a repository that turned rename detection off, so this
    // also pins that the flag is what detects the rename.
    git(at, &["git", "config", "diff.renames", "false"]);
    let base = commit_file(at, "old.txt", b"one\ntwo\nthree\nfour\n", false);
    git(at, &["git", "mv", "old.txt", "new.txt"]);
    git(
        at,
        &[
            "git",
            "-c",
            "user.email=estigia@example.invalid",
            "-c",
            "user.name=Estigia test",
            "commit",
            "--quiet",
            "-m",
            "rename",
        ],
    );
    let head = oid(at);
    let base_tree = tree_of(&context, Some(at), &base).expect("the base tree");
    let head_tree = tree_of(&context, Some(at), &head).expect("the head tree");
    assert_eq!(
        tree_pair_rename_destinations(&context, Some(at), &base_tree, &head_tree)
            .expect("the rename destinations"),
        vec!["new.txt".to_owned()]
    );
}

/// A **committed** `.gitattributes` marking every path binary must not reach
/// `DiffIsolation::Isolated`'s count: its fresh `GIT_DIR` is `core.bare =
/// true`, so it has no worktree of its own to read an attributes file from,
/// even though the real worktree this test runs in has one checked out on
/// disk at `at`. Measured directly rather than assumed: the identical
/// `git diff --numstat` over the same two trees answers `-\t-` (binary,
/// silenced) run against the ambient worktree and `2\t0` run against the
/// isolated one. Reddens if `diff_stats` is ever asked for
/// `DiffIsolation::Ambient` here, or if the isolated `GIT_DIR`'s own config
/// stops declaring itself bare.
#[test]
fn a_committed_gitattributes_binary_override_does_not_reach_the_isolated_diff() {
    let root = repo();
    let at = root.path();
    let context = test_context(at);
    std::fs::write(at.join(".gitattributes"), "* -diff\n").expect("the attributes file");
    git(at, &["git", "add", ".gitattributes"]);
    let base = commit_file(at, "text.txt", b"one\ntwo\n", false);
    let head = commit_file(at, "text.txt", b"one\ntwo\nthree\nfour\n", false);

    let base_tree = tree_of(&context, Some(at), &base).expect("the base tree");
    let head_tree = tree_of(&context, Some(at), &head).expect("the head tree");
    let count = crate::test_env::with_environment_read_locked(|| {
        tree_pair_changed_lines(&context, Some(at), &base_tree, &head_tree, &[])
    })
    .expect("a count");
    assert_eq!(
        count, 2,
        "a committed `.gitattributes` marking every path binary silenced the isolated diff"
    );
}

/// The composed path: `DiffIsolation::Isolated` must reach `git` through
/// `run_with_isolated_env`'s replaced environment, not `run_with_env`'s
/// layered one, or a caller's own default global attributes file —
/// `$XDG_CONFIG_HOME/git/attributes`, resolved from the environment rather
/// than from any config value `GIT_CONFIG_GLOBAL` can redirect — silences
/// the count. `essential_env`'s own filter has its own pure test in
/// `transport::tests`; this is the one proving `diff_stats` actually calls
/// it.
///
/// The attribute rule names this test's own committed path by its exact,
/// otherwise-unused basename rather than `* -diff`: every unit test shares
/// one process environment, and a rule matching every path would silence an
/// unrelated, concurrently running test's own ambient `git diff` for as
/// long as `HOME` points here — measured directly, the same two unrelated
/// tests failing in each of three runs of the whole suite with such a rule
/// in place, and three clean runs once it was narrowed to this name.
#[test]
fn a_default_global_attributes_file_does_not_reach_the_isolated_diff() {
    let root = repo();
    let at = root.path();
    let context = test_context(at);
    let base = commit_file(
        at,
        "estigia-default-global-attributes-sentinel.txt",
        b"one\ntwo\n",
        false,
    );
    let head = commit_file(
        at,
        "estigia-default-global-attributes-sentinel.txt",
        b"one\ntwo\nthree\nfour\n",
        false,
    );
    let base_tree = tree_of(&context, Some(at), &base).expect("the base tree");
    let head_tree = tree_of(&context, Some(at), &head).expect("the head tree");

    let profile = tempfile::tempdir().expect("a caller profile directory");
    let config_home = profile.path().join("config");
    std::fs::create_dir_all(config_home.join("git")).expect("the profile's git config directory");
    std::fs::write(
        config_home.join("git").join("attributes"),
        "estigia-default-global-attributes-sentinel.txt -diff\n",
    )
    .expect("the caller's own default global attributes file");
    let home = profile.path().join("home");
    std::fs::create_dir_all(&home).expect("the profile's home directory");

    let count = crate::test_env::with_home_env(&home, &home, &config_home, || {
        tree_pair_changed_lines(&context, Some(at), &base_tree, &head_tree, &[])
    })
    .expect("a count");
    assert_eq!(
        count, 2,
        "the caller's own default global attributes file silenced the isolated diff"
    );
}

// ---------------------------------------------------------------------------
// The fail-closed cross-check between `classify_risk` and
// `risk_level_from_reasons`.
// ---------------------------------------------------------------------------

/// `explained_matches_level` refuses a `(level, reasons)` pair built by hand
/// — never through `derive_snapshot_risk_reasons`, `only_passive` or
/// `touches_configuration` — which is what makes it a pair the real path is
/// not known to ever produce.
///
/// Investigation could not construct a `stats` / `active_content`
/// pair that reaches `review_tier_answer`'s own computation of `level` and
/// `reasons` and makes them disagree — `classify_risk` and
/// `risk_level_from_reasons` read the same `reasons`, and
/// `the_two_derivations_agree_on_a_spread_of_diff_stat_shapes` below pins that
/// agreement over a spread of shapes. This test exists anyway, to prove the
/// refusal itself is correct on the day it becomes reachable, rather than
/// leaving that code path unexercised by anything.
#[test]
fn explained_matches_level_refuses_a_pair_the_real_path_never_produces() {
    let reasons = vec![RiskReason {
        code: REASON_NON_EXECUTABLE_ONLY,
        signal: None,
        path: None,
    }];
    let error = explained_matches_level(RiskLevel::Medium, &reasons)
        .expect_err("a Low-shaped reasons list beside a Medium level must refuse");
    let Failure::Read(message) = error else {
        panic!("the refusal was not a failed read: {error:?}");
    };
    assert_eq!(
        message, "review tier Medium is not explained by its reasons (Low)",
        "{message}"
    );
}

/// The pair `explained_matches_level` must accept: `level` and `reasons`
/// that genuinely agree, which is the ordinary case for every real answer.
#[test]
fn explained_matches_level_accepts_a_pair_that_agrees() {
    let reasons = vec![RiskReason {
        code: REASON_HOT_PATH,
        signal: Some("auth"),
        path: Some("internal/auth/token.rs".to_owned()),
    }];
    explained_matches_level(RiskLevel::High, &reasons).expect("a High level a signal explains");
}

/// A `DiffStat` builder exposing every field the agreement enumeration below
/// varies — `active_stat`/`passive_stat` above fix mode and object ids, which
/// this cannot afford to.
fn shaped_stat(
    path: &str,
    additions: u64,
    deletions: u64,
    binary: bool,
    mode_only: bool,
    old_mode: &str,
    new_mode: &str,
) -> DiffStat {
    DiffStat {
        path: path.to_owned(),
        additions,
        deletions,
        binary,
        mode_only,
        old_mode: old_mode.to_owned(),
        new_mode: new_mode.to_owned(),
        old_object: "a".repeat(40),
        new_object: if mode_only {
            "a".repeat(40)
        } else {
            "b".repeat(40)
        },
    }
}

/// `review_tier_answer`'s own three computations, run directly over a
/// caller-built `stats` / `active_content` pair rather than through a
/// repository: the reasons, `only_passive`, `touches_configuration`, and the
/// two levels those feed — `classify_risk`'s and `risk_level_from_reasons`'s
/// — which must never disagree.
fn agree(stats: &[DiffStat], active_content: &BTreeSet<String>) -> (RiskLevel, RiskLevel) {
    let reasons = derive_snapshot_risk_reasons(stats, active_content, Vec::new());
    let (only_passive, touches_configuration) = passive_and_configuration(stats, active_content);
    let level = classify_risk(stats, &reasons, only_passive, touches_configuration);
    (level, risk_level_from_reasons(&reasons))
}

/// Pins the agreement `explained_matches_level` leans on for every shape
/// the investigation named as worth trying, plus a few its reading
/// of `derive_snapshot_risk_reasons` and `classify_risk` added: a mode-flip on
/// a passive path, a configuration path beside a passive one (in both
/// alphabetical orders), a binary stat beside a configuration path (in both
/// orders), an empty-content stat beside a passive one, a service-token path
/// that is also shell-source, a mode change that does not touch the execute
/// bit, a hot-path token, an executable-mode change on an otherwise ordinary
/// path, and a passive stat the content proof would have contradicted —
/// alone, and on both sides of a configuration path alphabetically, and two
/// ordinary passive documents with nothing else. Whoever edits
/// `classify_risk`, `risk_level_from_reasons` or
/// `derive_snapshot_risk_reasons` so that one of these disagrees turns this
/// red, which is the point: it stands in for the cross-check
/// `review_tier_answer` runs live, over inputs nothing has reached it with
/// yet.
#[test]
fn the_two_derivations_agree_on_a_spread_of_diff_stat_shapes() {
    let cases: Vec<(&str, Vec<DiffStat>, BTreeSet<String>)> = vec![
        (
            "mode-flip on a passive path",
            vec![shaped_stat(
                "docs/guide.md",
                3,
                1,
                false,
                false,
                "100644",
                "100755",
            )],
            BTreeSet::new(),
        ),
        (
            "configuration path beside a passive one, config first",
            vec![
                shaped_stat("aconfig.json", 2, 1, false, false, "100644", "100644"),
                shaped_stat("zdocs/guide.md", 4, 0, false, false, "100644", "100644"),
            ],
            BTreeSet::new(),
        ),
        (
            "configuration path beside a passive one, passive first",
            vec![
                shaped_stat("adocs/guide.md", 4, 0, false, false, "100644", "100644"),
                shaped_stat("zconfig.json", 2, 1, false, false, "100644", "100644"),
            ],
            BTreeSet::new(),
        ),
        (
            "binary stat beside a configuration path, binary first",
            vec![
                shaped_stat("assets/image.bin", 0, 0, true, false, "100644", "100644"),
                shaped_stat("config.json", 2, 1, false, false, "100644", "100644"),
            ],
            BTreeSet::new(),
        ),
        (
            "binary stat beside a configuration path, configuration first",
            vec![
                shaped_stat("aconfig.json", 2, 1, false, false, "100644", "100644"),
                shaped_stat("zzz-binary.bin", 0, 0, true, false, "100644", "100644"),
            ],
            BTreeSet::new(),
        ),
        (
            "empty-content stat beside a passive one",
            vec![
                shaped_stat("docs/guide.md", 2, 0, false, false, "100644", "100644"),
                shaped_stat("src/view.rs", 0, 0, false, false, "100644", "100644"),
            ],
            BTreeSet::new(),
        ),
        (
            "a path that is both service-token and shell-source",
            vec![shaped_stat(
                "service-token.sh",
                3,
                1,
                false,
                false,
                "100644",
                "100644",
            )],
            BTreeSet::new(),
        ),
        (
            "modes differ but not in the executable bit",
            vec![shaped_stat(
                "docs/link.md",
                0,
                0,
                false,
                true,
                "100644",
                "120000",
            )],
            BTreeSet::new(),
        ),
        (
            "a hot-path token",
            vec![shaped_stat(
                "internal/auth/token.rs",
                5,
                2,
                false,
                false,
                "100644",
                "100644",
            )],
            BTreeSet::new(),
        ),
        (
            "an executable-mode change on an ordinary source path",
            vec![shaped_stat(
                "src/view.rs",
                1,
                0,
                false,
                false,
                "100644",
                "100755",
            )],
            BTreeSet::new(),
        ),
        (
            "a passive stat the content proof contradicts, alone",
            vec![shaped_stat(
                "docs/guide.md",
                2,
                0,
                false,
                false,
                "100644",
                "100644",
            )],
            BTreeSet::from(["docs/guide.md".to_owned()]),
        ),
        (
            "a contradicted passive stat sorted after a configuration path",
            vec![
                shaped_stat("aconfig.json", 1, 0, false, false, "100644", "100644"),
                shaped_stat("zdocs/guide.md", 2, 0, false, false, "100644", "100644"),
            ],
            BTreeSet::from(["zdocs/guide.md".to_owned()]),
        ),
        (
            "a contradicted passive stat sorted before a configuration path",
            vec![
                shaped_stat("adocs/guide.md", 2, 0, false, false, "100644", "100644"),
                shaped_stat("zconfig.json", 1, 0, false, false, "100644", "100644"),
            ],
            BTreeSet::from(["adocs/guide.md".to_owned()]),
        ),
        (
            "two ordinary passive documents and nothing else",
            vec![
                shaped_stat("docs/a.md", 3, 0, false, false, "100644", "100644"),
                shaped_stat("docs/b.md", 1, 1, false, false, "100644", "100644"),
            ],
            BTreeSet::new(),
        ),
    ];

    assert_eq!(
        cases.len(),
        14,
        "the enumeration changed size; `CHANGELOG.md`, `docs/honesty.md` and `tests/guards.rs` name this count and have to move with it"
    );

    let mut disagreements = Vec::new();
    for (name, stats, active_content) in &cases {
        let (level, explained) = agree(stats, active_content);
        if level != explained {
            disagreements.push(format!("{name}: level={level:?} explained={explained:?}"));
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} of {} shapes disagreed:\n{}",
        disagreements.len(),
        cases.len(),
        disagreements.join("\n")
    );
}

fn tier_after(path: &str, after: &str) -> serde_json::Value {
    let root = repo();
    let at = root.path();
    let base = commit_file(at, path, b"x\n", false);
    let head = commit_file(at, path, after.as_bytes(), false);
    review_tier(&test_context(at), &base, &head, None, None).expect("an answer")
}

fn names_reason(answer: &serde_json::Value, code: &str, signal: &str) -> bool {
    answer["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .any(|reason| reason["code"] == code && reason["signal"] == signal)
}

#[test]
fn an_added_process_spawn_is_high_with_its_own_reason() {
    let answer = tier_after("src/run.py", "x\nout = subprocess.check_output([\"ls\"])\n");
    assert_eq!(answer["level"], "high", "{answer}");
    assert!(
        names_reason(&answer, "process_boundary", "shell_process"),
        "{answer}"
    );
}

#[test]
fn an_added_interpreter_directive_in_source_is_a_process_boundary() {
    let answer = tier_after("src/tool.py", "#!/usr/bin/env python3\nx = 1\n");
    assert_eq!(answer["level"], "high", "{answer}");
    assert!(
        names_reason(&answer, "process_boundary", "shell_process"),
        "{answer}"
    );
}

#[test]
fn a_dangerous_sink_token_raises_the_tier_for_its_family() {
    // One row per dangerous-sink family. The family is named in both failures,
    // so a red run says which token stopped matching rather than which
    // one-case function happened to be first.
    let cases = [
        (
            "tls_verification",
            "src/client.go",
            "cfg := &tls.Config{InsecureSkipVerify: true}",
        ),
        (
            "unsafe_deserialization",
            "src/load.py",
            "data = pickle.loads(blob)",
        ),
        ("eval", "src/run.js", "const out = eval(input);"),
        (
            "shell_composition",
            "src/exec.py",
            "subprocess.run(command, shell=True)",
        ),
        (
            "weak_credential_hash",
            "src/pw.py",
            "digest = hashlib.md5(password)",
        ),
        (
            "credentialed_cors",
            "src/cors.js",
            "res.setHeader(\"Access-Control-Allow-Origin\", \"*\"); res.setHeader(\"Access-Control-Allow-Credentials\", \"true\");",
        ),
        ("world_writable", "src/perm.go", "os.Chmod(path, 0777)"),
        (
            "unverified_jwt",
            "src/session.go",
            "tok, _, err := parser.ParseUnverified(raw, claims)",
        ),
        ("csrf_opt_out", "src/views.py", "@csrf_exempt"),
        (
            "xxe",
            "src/parse.py",
            "parser = etree.XMLParser(resolve_entities=True)",
        ),
    ];
    for (family, path, line) in cases {
        let answer = tier_after(path, &format!("x\n{line}\n"));
        assert_eq!(answer["level"], "high", "{family}: {answer}");
        assert!(
            names_reason(&answer, "dangerous_sink", family),
            "{family}: no dangerous_sink reason names it: {answer}"
        );
    }
}

#[test]
fn benign_look_alikes_stay_non_high() {
    let root = repo();
    let at = root.path();
    let files = [
        ("src/store.go", "rows, err := db.Exec(query)"),
        ("src/match.js", "const m = /ab+/.exec(text);"),
        ("src/hash.py", "digest = hashlib.sha256(password)"),
        (
            "src/config.py",
            "cfg = yaml.load(raw, Loader=yaml.SafeLoader)",
        ),
        ("src/notes.py", "# pickle.loads(blob)"),
        ("src/lib.rs", "let v = pickle.loads(x);"),
        ("src/calc.js", "const total = evaluate(input);"),
        ("tests/loader.py", "data = pickle.loads(blob)"),
    ];
    let mut base = String::new();
    for (path, _) in files {
        base = commit_file(at, path, b"x\n", false);
    }
    let mut head = base.clone();
    for (path, line) in files {
        head = commit_file(at, path, format!("x\n{line}\n").as_bytes(), false);
    }
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
    assert!(
        answer["reasons"]
            .as_array()
            .expect("reasons")
            .iter()
            .all(|reason| reason.get("signal").is_none()),
        "{answer}"
    );
}

#[test]
fn a_content_scan_past_the_byte_budget_reads_high_with_its_own_reason() {
    let answer = tier_after("src/big.py", &"x = 1\n".repeat(1_500_000));
    assert_eq!(answer["level"], "high", "{answer}");
    assert!(
        names_reason(&answer, "process_scan_limit", "shell_process"),
        "{answer}"
    );
}

#[test]
fn a_passive_candidate_past_the_byte_budget_is_never_low() {
    let answer = tier_after("docs/big.md", &"Ordinary prose line.\n".repeat(450_000));
    assert_eq!(answer["level"], "medium", "{answer}");
}

#[test]
fn a_rust_inner_attribute_is_not_an_interpreter_directive() {
    let root = repo();
    let at = root.path();
    let base = commit_file(
        at,
        "src/lib.rs",
        b"#![allow(dead_code)]\nfn a() {}\n",
        false,
    );
    let head = commit_file(
        at,
        "src/lib.rs",
        b"#![allow(dead_code)]\nfn b() {}\n",
        false,
    );
    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");
    assert_eq!(answer["level"], "medium", "{answer}");
    assert!(
        !names_reason(&answer, "process_boundary", "shell_process"),
        "{answer}"
    );
}

#[test]
fn only_the_first_line_is_an_interpreter_directive() {
    assert!(content_scan::has_shebang_line("#! /bin/sh\nx\n"));
    assert!(content_scan::has_shebang_line("#!/usr/bin/env python3\n"));
    assert!(!content_scan::has_shebang_line("#![deny(missing_docs)]\n"));
    assert!(!content_scan::has_shebang_line("x = 1\n#!/bin/sh\n"));
}

#[test]
fn a_rust_unit_test_module_is_a_test_path() {
    for path in ["src/thing/tests.rs", "src/thing/tier_tests.rs"] {
        let answer = tier_after(path, "x\nlet o = subprocess;\n");
        assert_eq!(answer["level"], "medium", "{path}: {answer}");
    }
}

#[test]
fn a_diff_past_the_byte_budget_with_blobs_under_it_reads_high() {
    let answer = tier_after("src/wide.py", &"\n".repeat(4_300_000));
    assert_eq!(answer["level"], "high", "{answer}");
    assert!(
        names_reason(&answer, "process_scan_limit", "shell_process"),
        "{answer}"
    );
}

#[test]
fn a_header_the_scan_cannot_attribute_reads_high() {
    let answer = tier_after("src/a b/c.py", "x\nx = 1\n");
    assert_eq!(answer["level"], "high", "{answer}");
    assert!(
        names_reason(&answer, "process_scan_limit", "shell_process"),
        "{answer}"
    );
}

const DECLARES_API: &str =
    "[[generated]]\npath = \"docs/api.md\"\ncommand = [\"sh\", \"-c\", \"exit 1\"]\n";

fn generated_tier_world(declared: bool) -> (tempfile::TempDir, String, String) {
    let root = repo();
    let at = root.path();
    let mut seed: Vec<(&str, &[u8])> = vec![
        ("docs/api.md", b"Ordinary prose.\n"),
        ("docs/guide.md", b"Ordinary guide.\n"),
    ];
    if declared {
        seed.push(("estigia.generated.toml", DECLARES_API.as_bytes()));
    }
    let mut base = String::new();
    for (path, content) in seed {
        base = commit_file(at, path, content, false);
    }
    let churn: String = (0..120)
        .map(|row| format!("generated row {row}\n"))
        .collect();
    commit_file(
        at,
        "docs/api.md",
        format!("#!/usr/bin/env bash\n{churn}").as_bytes(),
        false,
    );
    let head = commit_file(at, "docs/guide.md", b"Edited guide.\n", false);
    (root, base, head)
}

#[test]
fn a_declared_generated_path_is_left_out_of_the_tier_and_named_in_the_answer() {
    let (root, base, head) = generated_tier_world(true);

    let answer =
        review_tier(&test_context(root.path()), &base, &head, None, None).expect("an answer");

    assert_eq!(answer["level"], "low", "{answer}");
    assert_eq!(answer["changed_lines"], 2, "{answer}");
    assert_eq!(
        answer["generated_paths"],
        serde_json::json!(["docs/api.md"])
    );
}

#[test]
fn the_same_change_without_a_declaration_is_counted_and_scanned_whole() {
    let (root, base, head) = generated_tier_world(false);

    let answer =
        review_tier(&test_context(root.path()), &base, &head, None, None).expect("an answer");

    assert_eq!(answer["level"], "medium", "{answer}");
    assert!(
        answer["changed_lines"]
            .as_u64()
            .is_some_and(|lines| lines > 100),
        "{answer}"
    );
    assert_eq!(answer["generated_paths"], serde_json::json!([]));
}

#[test]
fn a_repair_tier_leaves_out_what_the_receipt_base_declares() {
    let root = repo();
    let at = root.path();
    commit_file(at, "estigia.generated.toml", DECLARES_API.as_bytes(), false);
    commit_file(at, "docs/api.md", b"Ordinary prose.\n", false);
    let base = commit_file(at, "docs/guide.md", b"Ordinary guide.\n", false);
    let parent_head = commit_file(at, "docs/api.md", b"Regenerated once.\n", false);
    let churn: String = (0..120)
        .map(|row| format!("generated row {row}\n"))
        .collect();
    commit_file(
        at,
        "docs/api.md",
        format!("#!/usr/bin/env bash\n{churn}").as_bytes(),
        false,
    );
    let repair_head = commit_file(at, "docs/guide.md", b"Edited guide.\n", false);

    let answer = review_tier_for_repair(
        &test_context(at),
        &base,
        &parent_head,
        &repair_head,
        None,
        None,
        None,
    )
    .expect("an answer");

    assert_eq!(answer["range"], "repair", "{answer}");
    assert_eq!(answer["level"], "low", "{answer}");
    assert_eq!(answer["changed_lines"], 2, "{answer}");
    assert_eq!(
        answer["generated_paths"],
        serde_json::json!(["docs/api.md"])
    );
}

#[test]
fn tree_pair_changed_lines_leaves_out_the_paths_it_is_told_to() {
    let root = repo();
    let at = root.path();
    let context = test_context(at);
    let base = commit_file(at, "a.txt", b"one\n", false);
    commit_file(at, "lock.txt", b"1\n2\n3\n4\n5\n", false);
    let head = commit_file(at, "a.txt", b"two\n", false);
    let base_tree = tree_of(&context, Some(at), &base).expect("a tree");
    let head_tree = tree_of(&context, Some(at), &head).expect("a tree");

    let whole =
        tree_pair_changed_lines(&context, Some(at), &base_tree, &head_tree, &[]).expect("a count");
    let without = tree_pair_changed_lines(
        &context,
        Some(at),
        &base_tree,
        &head_tree,
        &["lock.txt".to_owned()],
    )
    .expect("a count");

    assert_eq!((whole, without), (7, 2));
}

#[test]
fn a_change_of_nothing_but_generated_files_measures_nothing_and_is_medium() {
    let root = repo();
    let at = root.path();
    commit_file(at, "estigia.generated.toml", DECLARES_API.as_bytes(), false);
    let base = commit_file(at, "docs/api.md", b"Ordinary prose.\n", false);
    let head = commit_file(at, "docs/api.md", b"Regenerated.\n", false);

    let answer = review_tier(&test_context(at), &base, &head, None, None).expect("an answer");

    assert_eq!(answer["level"], "medium", "{answer}");
    assert_eq!(answer["changed_lines"], 0, "{answer}");
    assert_eq!(
        answer["generated_paths"],
        serde_json::json!(["docs/api.md"])
    );
}

#[cfg(unix)]
fn cat_file_calls_to_tier(files: usize) -> usize {
    use std::os::unix::fs::PermissionsExt;

    let root = repo();
    let at = root.path();
    let base = oid(at);
    for index in 0..files {
        std::fs::create_dir_all(at.join("docs")).expect("docs");
        std::fs::create_dir_all(at.join("src")).expect("src");
        std::fs::write(at.join(format!("docs/note{index}.md")), "Ordinary prose.\n")
            .expect("a document");
        std::fs::write(at.join(format!("src/part{index}.rs")), "fn part() {}\n")
            .expect("a source file");
    }
    git(at, &["git", "add", "."]);
    git(at, &["git", "commit", "--quiet", "-m", "many"]);
    let head = oid(at);

    let shims = tempfile::tempdir().expect("a shim directory");
    let log = shims.path().join("calls.log");
    let real = crate::transport::run(
        &["sh", "-c", "command -v git"],
        None,
        crate::transport::How::read(),
    )
    .expect("git is on PATH")
    .stdout
    .trim()
    .to_owned();
    let shim = shims.path().join("git");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nexec '{real}' \"$@\"\n",
            log.display()
        ),
    )
    .expect("a logging git");
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("executable");

    crate::transport::with_process_override(shims.path(), &[], || {
        review_tier(&test_context(at), &base, &head, None, None).expect("an answer")
    });
    std::fs::read_to_string(&log)
        .expect("the shim logged")
        .lines()
        .filter(|line| line.starts_with("cat-file"))
        .count()
}

#[cfg(unix)]
#[test]
fn a_tier_scan_spawns_the_same_cat_file_processes_for_one_blob_or_many() {
    let one = cat_file_calls_to_tier(1);
    assert!(one > 0, "the scan read no blob");
    assert_eq!(cat_file_calls_to_tier(6), one);
}

#[test]
fn a_batch_reads_each_blob_by_its_size_even_when_it_holds_a_header_shaped_line() {
    let root = repo();
    let at = root.path();
    let tricky = b"0123456789abcdef0123456789abcdef01234567 blob 3\nabc\n";
    commit_file(at, "a.txt", tricky, false);
    commit_file(at, "b.txt", b"second\n", false);
    let object = |path: &str| {
        crate::transport::run(
            &["git", "rev-parse", &format!("HEAD:{path}")],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("a blob")
        .stdout
        .trim()
        .to_owned()
    };
    let (first, second) = (object("a.txt"), object("b.txt"));
    let oids = [first.as_str(), second.as_str(), first.as_str()];

    assert_eq!(
        blob_sizes(at, &oids).expect("sizes"),
        vec![tricky.len() as u64, 7, tricky.len() as u64]
    );
    let tricky_text = String::from_utf8(tricky.to_vec()).expect("ascii");
    assert_eq!(
        blob_contents(at, &oids).expect("contents"),
        vec![tricky_text.clone(), "second\n".to_owned(), tricky_text]
    );
}

#[test]
fn a_missing_object_is_a_failed_read_never_an_empty_blob() {
    let root = repo();
    let at = root.path();
    let missing = "0000000000000000000000000000000000000001";
    assert!(matches!(blob_sizes(at, &[missing]), Err(Failure::Read(_))));
    assert!(matches!(
        blob_contents(at, &[missing]),
        Err(Failure::Read(_))
    ));
}
