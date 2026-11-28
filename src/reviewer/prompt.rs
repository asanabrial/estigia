//! The bytes a binary-launched reviewer is handed.
//!
//! One fixed section carrying the receipt, the criteria, the findings and the
//! diff, then a role's own template with its tool-grant rule removed because
//! this reviewer has no tools, and last the exact shape the role must answer
//! in. The shared sections lead so that concurrent reviewers of one receipt
//! share a prefix a prompt cache can serve.
//! The same input is the same bytes, and a diff that is not the one the
//! receipt published is refused rather than reviewed.

use super::role::Answer;
use super::{Receipt, ReviewerCode, Role};
use crate::outcome::Refusal;
use crate::transport::claim::{
    FINDING_CAUSAL_DISPOSITIONS, FINDING_CLASSES, FINDING_EVIDENCE_CLASSES, FINDING_ORIGINS,
    REFUTATION_OUTCOMES,
};
use crate::transport::ownership::sha256_hex;
use crate::transport::target::REVIEW_PAYLOAD_CEILING;

/// The most bytes a prompt may be: the transport's payload ceiling plus room for
/// the template, the criteria and the findings.
pub const PROMPT_CEILING: usize = REVIEW_PAYLOAD_CEILING + 128 * 1024;

/// One finding a prompt carries: a candidate for the refuter, or a parent
/// finding for the validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingForPrompt {
    /// The finding's identity.
    pub id: String,
    /// One of `FINDING_CLASSES`.
    pub class: String,
    /// What the finding cites.
    pub evidence: String,
    /// What it costs if it holds.
    pub impact: String,
    /// The `path:line` it names, if it named one.
    pub location: Option<String>,
}

/// The diff sections for the files `findings` cite, or `None` when the whole
/// payload must be shown.
///
/// `None` is the safe answer: a finding with no location, a cited path the diff
/// does not carry, or a payload that is not UTF-8 all fall back to the whole
/// diff rather than hiding a file the reviewer was asked about.
pub fn payload_for_candidates(findings: &[FindingForPrompt], payload: &[u8]) -> Option<Vec<u8>> {
    if findings.is_empty() {
        return None;
    }
    let mut paths = Vec::new();
    for finding in findings {
        paths.push(finding.location.as_deref()?.split(':').next()?);
    }
    let text = std::str::from_utf8(payload).ok()?;
    let mut cuts = Vec::new();
    for section in diff_sections(text) {
        if section_path(section).is_some_and(|path| paths.contains(&path)) {
            cuts.push(section);
        }
    }
    if cuts.is_empty() {
        None
    } else {
        Some(cuts.concat().into_bytes())
    }
}

fn diff_sections(text: &str) -> Vec<&str> {
    let mut starts = Vec::new();
    if text.starts_with("diff --git ") {
        starts.push(0);
    }
    for (at, _) in text.match_indices("\ndiff --git ") {
        starts.push(at + 1);
    }
    starts.push(text.len());
    starts
        .windows(2)
        .map(|pair| &text[pair[0]..pair[1]])
        .collect()
}

fn section_path(section: &str) -> Option<&str> {
    let header = section.split_once('\n').map_or(section, |(line, _)| line);
    let (_, after) = header.split_once(" b/")?;
    Some(after)
}

/// Everything one prompt is built from.
#[derive(Debug, Clone, Copy)]
pub struct PromptInput<'a> {
    /// The role being asked.
    pub role: Role,
    /// The review the run belongs to.
    pub receipt: &'a Receipt,
    /// The exact diff the receipt published.
    pub payload: &'a [u8],
    /// The acceptance criteria the change is held to.
    pub criteria: &'a str,
    /// The findings the role adjudicates or checks: a refuter's candidates, or a
    /// lens's parent findings when the publication is a repair. Empty when the
    /// role checks none.
    pub findings: &'a [FindingForPrompt],
}

/// Builds the prompt for `input`.
///
/// Refuses with `reviewer-payload-mismatch` when `input.payload` does not hash
/// to `receipt.payload_digest`, and with `reviewer-input-too-large` past
/// [`PROMPT_CEILING`]. Never truncates.
pub fn build(input: &PromptInput<'_>) -> Result<Vec<u8>, Refusal> {
    build_showing(input, None)
}

/// Builds the prompt for `input`, showing `shown` bytes of the diff in place of
/// the whole payload the receipt published.
///
/// The digest is always checked against `input.payload`, so a caller may cut
/// the diff it displays but never the bytes it vouches for. `None` shows the
/// whole payload.
pub fn build_showing(input: &PromptInput<'_>, shown: Option<&[u8]>) -> Result<Vec<u8>, Refusal> {
    let actual = sha256_hex(input.payload);
    if actual != input.receipt.payload_digest {
        return Err(ReviewerCode::PayloadMismatch.refusal(format!(
            "the diff offered hashes to {actual}, and the receipt published {}",
            input.receipt.payload_digest
        )));
    }

    let mut prompt = b"# Launch\n\n".to_vec();
    prompt.extend_from_slice(receipt_section(input.receipt).as_bytes());
    prompt.extend_from_slice(b"\n## Criteria\n\n");
    prompt.extend_from_slice(
        serde_json::Value::from(input.criteria.trim())
            .to_string()
            .as_bytes(),
    );
    prompt.extend_from_slice(findings_heading(input.role, input.findings).as_bytes());
    prompt.extend_from_slice(findings_section(input.findings).as_bytes());
    prompt.extend_from_slice(
        format!(
            "\n## Diff\n\n<<<DIFF sha256={actual} bytes={}\n",
            input.payload.len()
        )
        .as_bytes(),
    );
    prompt.extend_from_slice(shown.unwrap_or(input.payload));
    prompt.extend_from_slice(
        b"\nDIFF>>>\n\n---\n\n# Instructions\n\nWherever the instructions below say the \
          diff, they mean the diff under `## Diff` above; it and the sections before it are \
          everything you are given. A file shown as a `diff --git` header and one \
          `# estigia-generated` line, with no hunks, is verified by byte-identical regeneration \
          when the verdict is recorded: do not review it by reading, and do not report its \
          missing hunks.\n\n",
    );
    if shown.is_some() {
        prompt.extend_from_slice(
            b"Only the file diffs the candidates cite are shown. The digest and byte count \
              above name the whole published payload, which was verified before this cut; treat \
              these sections as that payload's cited files, not as a different change.\n\n",
        );
    }
    prompt.extend_from_slice(body_of(input.role.template()).as_bytes());
    prompt.extend_from_slice(b"\n\n");
    prompt.extend_from_slice(answer_section(input.role).as_bytes());

    if prompt.len() > PROMPT_CEILING {
        return Err(ReviewerCode::InputTooLarge.refusal(format!(
            "the prompt is {} bytes and the ceiling is {PROMPT_CEILING}",
            prompt.len()
        )));
    }
    Ok(prompt)
}

fn body_of(template: &str) -> String {
    let trimmed = template.trim_start_matches('\u{feff}').trim_start();
    let body = crate::harness::roles::frontmatter(template)
        .and_then(|frontmatter| trimmed.get("---".len() + frontmatter.len() + "\n---".len()..))
        .unwrap_or(trimmed);
    let body = crate::skill::with_lens_rules(body);
    without_tool_grant_rule(body.trim()).trim().to_owned()
}

fn without_tool_grant_rule(body: &str) -> String {
    let mut kept = Vec::new();
    let mut skipping = false;
    for line in body.lines() {
        if line.starts_with("- Your one tool, `WebSearch`") {
            skipping = true;
            continue;
        }
        if skipping && line.starts_with("  ") {
            continue;
        }
        skipping = false;
        kept.push(line);
    }
    kept.join("\n")
}

fn receipt_section(receipt: &Receipt) -> String {
    format!(
        "## Receipt\n\nepoch: {}\npr: {}\nhead: {}\nbase: {}\ndigest: {}\npayload-digest: {}\n",
        receipt.epoch,
        receipt.pr,
        receipt.head,
        receipt.base,
        receipt.digest,
        receipt.payload_digest
    )
}

fn findings_heading(role: Role, findings: &[FindingForPrompt]) -> &'static str {
    if role.answer() == Answer::Findings && !findings.is_empty() {
        "\n\n## Findings\n\nThe parent publication recorded the findings below; a finding that \
         reassesses one must name its `id` in `parent`.\n\n"
    } else {
        "\n\n## Findings\n\n"
    }
}

fn findings_section(findings: &[FindingForPrompt]) -> String {
    if findings.is_empty() {
        return "none\n".to_owned();
    }
    findings
        .iter()
        .map(|finding| {
            let mut line = serde_json::json!({
                "id": finding.id,
                "class": finding.class,
                "evidence": finding.evidence,
                "impact": finding.impact,
            });
            if let (Some(location), Some(object)) = (&finding.location, line.as_object_mut()) {
                object.insert("location".to_owned(), serde_json::json!(location));
            }
            format!("{line}\n")
        })
        .collect()
}

fn answer_section(role: Role) -> String {
    let shape = match role.answer() {
        Answer::Findings => format!(
            "{{\"findings\": [{{\"id\": \"<short identity built from the path and the \
             behaviour>\", \"class\": \"<{classes}>\", \
             \"evidence\": \"<what you saw>\", \"impact\": \"<what it costs>\", \
             \"location\": \"<path:line or path:start-end, required>\", \
             \"evidence_class\": \"<{evidence}; required for severe>\", \
             \"causal_disposition\": \"<{causal}; required for severe>\", \
             \"origin\": \"<{origin}; only for a severe finding new to a repair>\", \
             \"parent\": \"<id of the finding this reassesses, optional>\"}}]}}\n\nA \
             `severe` finding disposed `introduced` or `worsened` names in `location` only \
             lines of one unbroken run of `+` lines in the diff above \u{2014} not a context \
             line, and not a span from one run to another \u{2014} or the whole answer is \
             discarded. Where a defect reaches over several runs, locate it on the one that \
             carries it.\n\nAn empty `findings` list says the sweep found nothing.",
            classes = FINDING_CLASSES.join("|"),
            evidence = FINDING_EVIDENCE_CLASSES.join("|"),
            causal = FINDING_CAUSAL_DISPOSITIONS.join("|"),
            origin = FINDING_ORIGINS.join("|"),
        ),
        Answer::Outcomes => format!(
            "{{\"outcomes\": [{{\"id\": \"<the candidate's id>\", \"outcome\": \"<{}>\"}}]}}\n\n\
             One entry for every candidate under `## Findings`, and none for anything else.",
            REFUTATION_OUTCOMES.join("|")
        ),
        Answer::Inspection => "{\"inspection\": \"available\", \"original_criteria\": \
             \"<pass|fail>\", \"correction_regression\": \"<pass|fail>\"}\n\nor, when you could \
             not inspect the candidate and so have no verdict:\n\n{\"inspection\": \
             \"unavailable\", \"reason\": \"<why>\"}\n\nNever name a criterion on an unavailable \
             inspection."
            .to_owned(),
    };
    format!(
        "## Answer format\n\nAnswer with exactly one JSON object and nothing else, optionally \
         inside one ```json fence. Any other text around it makes the answer unreadable and it \
         is discarded. This format overrides any instruction above to state, say or explain \
         something: what has a field goes in that field, and everything else is left out.\n\n\
         {shape}\n"
    )
}

#[cfg(test)]
mod tests;
