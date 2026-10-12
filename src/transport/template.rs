//! The structural contract an analyst issue body is held to before it is filed.

use super::Failure;

include!(concat!(env!("OUT_DIR"), "/template_headings.rs"));

const ALERT: &str = "> [!note]";

fn headings_outside_fences(body: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = body.lines().collect();
    let mut fence: Option<&str> = None;
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let opener = ["```", "~~~"]
            .into_iter()
            .find(|marker| trimmed.starts_with(marker));
        match (fence, opener) {
            (None, Some(marker)) => fence = Some(marker),
            (Some(open), Some(marker)) if open == marker => fence = None,
            (None, None) if line.starts_with("## ") => found.push((index, line.trim_end())),
            _ => {}
        }
    }
    found
}

fn violation(heading: &str, problem: &str) -> Failure {
    Failure::ConfigDefect(serde_json::json!({
        "ok": false,
        "reason": "body-violates-template",
        "heading": heading,
        "problem": problem,
        "template": "skill/assets/analyst-issue-template.md",
        "action": format!(
            "fill the analyst issue template without changing its fixed first heading, its \
             `> [!NOTE]` alert or its heading order; `{heading}` is {problem}. Nothing was filed."
        ),
    }))
}

/// Refuses a body that does not follow the analyst template, naming the first
/// heading that is missing or out of order.
pub fn check(body: &str) -> Result<(), Failure> {
    let first = TEMPLATE_HEADINGS[0];
    let headings = headings_outside_fences(body);
    let Some(&(first_line, opening)) = headings.first() else {
        return Err(violation(first, "missing"));
    };
    if opening != first {
        return Err(violation(first, "not the first heading"));
    }
    let alert = body
        .lines()
        .skip(first_line + 1)
        .find(|line| !line.trim().is_empty());
    if !alert.is_some_and(|line| line.trim().eq_ignore_ascii_case(ALERT)) {
        return Err(violation(first, "not followed by a `> [!NOTE]` alert"));
    }
    let mut cursor = 1;
    for required in &TEMPLATE_HEADINGS[1..] {
        match headings
            .iter()
            .skip(cursor)
            .position(|(_, line)| line == required)
        {
            Some(offset) => cursor += offset + 1,
            None if headings.iter().any(|(_, line)| line == required) => {
                return Err(violation(required, "out of template order"));
            }
            None => return Err(violation(required, "missing")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
