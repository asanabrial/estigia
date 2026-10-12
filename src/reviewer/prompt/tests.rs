use super::*;
use crate::transport::ownership::sha256_hex;

const DIFF: &str = "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1 +1 @@\n-old\n+new\n";

fn receipt_for(payload: &[u8]) -> Receipt {
    Receipt {
        epoch: "e1".to_owned(),
        pr: 42,
        head: "h".repeat(40),
        base: "b".repeat(40),
        digest: "manifest-digest".to_owned(),
        payload_digest: sha256_hex(payload),
    }
}

fn finding(id: &str) -> FindingForPrompt {
    FindingForPrompt {
        id: id.to_owned(),
        class: "severe".to_owned(),
        evidence: "the evidence".to_owned(),
        impact: "the impact".to_owned(),
        location: Some("x.rs:1".to_owned()),
    }
}

fn prompt_for(role: Role, findings: &[FindingForPrompt]) -> String {
    let receipt = receipt_for(DIFF.as_bytes());
    let input = PromptInput {
        role,
        receipt: &receipt,
        payload: DIFF.as_bytes(),
        criteria: "  the criteria  ",
        findings,
    };
    String::from_utf8(build(&input).expect("a prompt")).expect("utf-8")
}

#[test]
fn the_same_input_is_the_same_bytes() {
    let findings = [finding("f1")];
    assert_eq!(
        prompt_for(Role::Refuter, &findings),
        prompt_for(Role::Refuter, &findings)
    );
}

#[test]
fn the_prompt_carries_the_exact_diff_and_the_receipt() {
    let text = prompt_for(Role::Risk, &[]);
    assert!(text.contains(&format!("\n{DIFF}\nDIFF>>>")), "{text}");
    let receipt = receipt_for(DIFF.as_bytes());
    for line in [
        "epoch: e1".to_owned(),
        "pr: 42".to_owned(),
        format!("head: {}", receipt.head),
        format!("base: {}", receipt.base),
        "digest: manifest-digest".to_owned(),
        format!("payload-digest: {}", receipt.payload_digest),
        format!(
            "<<<DIFF sha256={} bytes={}",
            receipt.payload_digest,
            DIFF.len()
        ),
    ] {
        assert!(text.contains(&line), "missing {line:?} in {text}");
    }
    assert!(text.contains("\n\"the criteria\"\n"));
    assert!(text.contains("## Findings\n\nnone\n"));
}

#[test]
fn findings_are_one_json_object_per_line() {
    let text = prompt_for(Role::Validator, &[finding("f1"), finding("f2")]);
    assert!(text.contains(
        "{\"id\":\"f1\",\"class\":\"severe\",\"evidence\":\"the evidence\",\"impact\":\"the \
         impact\",\"location\":\"x.rs:1\"}\n{\"id\":\"f2\""
    ));
}

#[test]
fn a_diff_that_is_not_the_published_one_is_refused() {
    let receipt = receipt_for(DIFF.as_bytes());
    let input = PromptInput {
        role: Role::Risk,
        receipt: &receipt,
        payload: b"a different diff",
        criteria: "",
        findings: &[],
    };
    let refusal = build(&input).expect_err("a payload the receipt did not publish");
    assert_eq!(refusal.code, "reviewer-payload-mismatch");
}

#[test]
fn a_prompt_past_the_ceiling_is_refused_not_truncated() {
    let payload = vec![b'x'; PROMPT_CEILING];
    let receipt = receipt_for(&payload);
    let input = PromptInput {
        role: Role::Risk,
        receipt: &receipt,
        payload: &payload,
        criteria: "",
        findings: &[],
    };
    let refusal = build(&input).expect_err("over the ceiling");
    assert_eq!(refusal.code, "reviewer-input-too-large");
}

#[test]
fn neither_the_frontmatter_nor_the_tool_grant_rule_reaches_a_reviewer() {
    for role in Role::ALL {
        let text = prompt_for(role, &[]);
        assert!(text.starts_with("You are "), "{role:?}: {text}");
        for leaked in [
            "WebSearch",
            "{{TOOLS}}",
            "tools:",
            "model: inherit",
            "name: review-",
            "its repository",
        ] {
            assert!(!text.contains(leaked), "{role:?} leaks {leaked:?}");
        }
        assert!(text.contains("## Rules"), "{role:?} lost its rules");
    }
}

#[test]
fn each_role_is_asked_with_its_own_template() {
    let openings = [
        (Role::Risk, "review-risk"),
        (Role::Readback, "review-readback"),
        (Role::Readability, "review-readability"),
        (Role::Reliability, "review-reliability"),
        (Role::Lens, "review-lens"),
        (Role::Refuter, "review-refuter"),
        (Role::Validator, "review-validator"),
    ];
    let mut bodies = std::collections::BTreeSet::new();
    for (role, name) in openings {
        assert_eq!(role.name(), name);
        assert_eq!(Role::from_name(name), Some(role));
        let template = crate::harness::roles::reserved_reviewer_row(name)
            .expect("a reserved row")
            .1;
        let text = prompt_for(role, &[]);
        let own_rules = body_of(template);
        assert!(
            text.starts_with(&own_rules),
            "{name} is not its own template"
        );
        assert!(bodies.insert(own_rules), "{name} shares a template");
    }
    assert_eq!(Role::from_name("review-blind"), None);
}

#[test]
fn the_answer_format_names_the_roles_vocabulary() {
    let lens = prompt_for(Role::Risk, &[]);
    assert!(lens.contains("severe|warning|suggestion"));
    assert!(lens.contains("deterministic|inferential|insufficient"));
    assert!(lens.contains("introduced|worsened|pre-existing|base-only|unknown"));
    assert!(prompt_for(Role::Refuter, &[]).contains("corroborated|refuted|inconclusive"));
    assert!(prompt_for(Role::Validator, &[]).contains("\"inspection\": \"unavailable\""));
    assert!(lens.ends_with("says the sweep found nothing.\n"));
}

#[test]
fn the_prompt_says_a_manifest_line_is_verified_by_regeneration_not_by_reading() {
    for role in [Role::Risk, Role::Refuter, Role::Validator] {
        let text = prompt_for(role, &[]);
        assert!(
            text.contains("`# estigia-generated` line")
                && text.contains("byte-identical regeneration")
                && text.contains("do not review it by reading"),
            "{role:?}: {text}"
        );
    }
}

const ANSWER_OVERRIDE: &str = "This format overrides any instruction above to state, say or \
     explain something: what has a field goes in that field, and everything else is left out.";

#[test]
fn the_answer_format_overrides_every_instruction_to_write_prose_for_every_role() {
    for role in Role::ALL {
        let text = prompt_for(role, &[]);
        let (_, answer) = text
            .split_once("## Answer format")
            .expect("an answer section");
        assert!(answer.contains(ANSWER_OVERRIDE), "{role:?}: {answer}");
    }
}

#[test]
fn no_findings_role_is_told_to_state_that_it_found_nothing() {
    for role in Role::ALL.into_iter().filter(|role| role.is_lens()) {
        let text = prompt_for(role, &[]);
        assert!(
            !text.to_lowercase().contains("state plainly"),
            "{role:?} asks for prose its answer format discards"
        );
        assert!(
            text.contains("An empty `findings` list is how you say the sweep found nothing"),
            "{role:?}"
        );
    }
}
