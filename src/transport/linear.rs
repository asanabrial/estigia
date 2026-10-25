//! The Linear source.
//!
//! Linear holds an issue's title and description behind a GraphQL API. Under
//! `Workflow: assisted` Estigia reads those two fields and the issue's state
//! type, and refuses a finished issue. It writes one thing, and only where the
//! repository's `State names` row maps a state: the issue's workflow state,
//! through [`MOVE_MUTATION`], read back afterwards. Every operation this source
//! can send is a constant in this module.
//!
//! The `linear` CLI performs the request, through the crate's one process
//! boundary ([`super::run`]), the way `gh` carries every GitHub call. The CLI
//! holds the credential — `linear auth login`, or `LINEAR_API_KEY`, which it
//! reads first — so no credential passes through Estigia, and only the
//! operation and its variables travel on the argument list.

use std::time::Duration;

use crate::identity::Identity;

use super::{Failure, How, run};

/// The program every Linear operation is sent through.
pub const PROGRAM: &str = "linear";

/// The environment variable the CLI reads a credential from ahead of its own.
pub const CREDENTIAL_VAR: &str = "LINEAR_API_KEY";

/// How long one request may take before it is given up.
const DEADLINE: Duration = Duration::from_secs(30);

/// The query that reads an issue to work.
pub const QUERY: &str =
    "query EstigiaIssue($id: String!) { issue(id: $id) { title description state { type } } }";

/// The query that reads an issue's state and the states its team offers.
pub const STATES_QUERY: &str = "query EstigiaStates($id: String!) { issue(id: $id) { id state { \
                                name } team { states { nodes { id name } } } } }";

/// The workflow state `assist` moves an issue into, as `State names` keys it.
pub const STARTED: &str = "in-progress";
/// The workflow state `ready_pr` moves an issue into.
pub const IN_REVIEW: &str = "review";
/// The workflow state `review_stages` moves an issue into on reading the merge.
pub const DELIVERED: &str = "done";

/// The one mutation this source sends: the issue's workflow state.
pub const MOVE_MUTATION: &str = "mutation EstigiaMove($id: String!, $stateId: String!) { \
                                 issueUpdate(id: $id, input: { stateId: $stateId }) { success } }";

/// One issue's title and description, as Linear spells them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The issue's title.
    pub title: String,
    /// The issue's description, in Markdown.
    pub description: String,
}

/// Reads one issue's title and description from Linear.
///
/// Every failure is a stop that names its reason — a missing, malformed or
/// rejected credential, an unknown key, a finished issue, or an unreachable
/// API — and happens before any repository write, because the caller is the
/// assisted path that reads the issue before it creates a checkout.
pub fn read_issue(identity: &Identity) -> Result<Issue, Failure> {
    let variables = serde_json::json!({ "id": identity.to_string() });
    parse(&post(QUERY, &variables, How::read())?, identity)
}

/// Moves one issue to the state its team names `target`, and reads it back.
///
/// Answers `moved: false` when the issue is already there. A state the team
/// does not have, a mutation Linear refuses and a read-back that disagrees are
/// each a stop naming which, and the last is `world: unknown`.
pub fn move_issue(identity: &Identity, target: &str) -> Result<serde_json::Value, Failure> {
    let States {
        id,
        current: from,
        offered: states,
    } = read_states(identity)?;
    if from.eq_ignore_ascii_case(target) {
        return Ok(serde_json::json!({ "ok": true, "moved": false, "state": from }));
    }
    let Some((state_id, _)) = states
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(target))
    else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-state-unknown",
            "detail": format!("issue {identity}'s team has no state named {target:?}"),
            "states": states.iter().map(|(_, name)| name).collect::<Vec<_>>(),
            "action": "set `State names` to names this team has; nothing was moved",
        })));
    };
    let variables = serde_json::json!({ "id": id, "stateId": state_id });
    let answer = post(MOVE_MUTATION, &variables, How::write())
        .and_then(|text| json_of(&text))
        .map_err(|failure| {
            Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "linear-move-unconfirmed",
                "world": "unknown",
                "detail": format!(
                    "moving {identity} to {target:?} was sent and no answer could be read: {}",
                    failure.detail()
                ),
                "action": "read the issue in Linear; do not report it moved or unmoved",
            }))
        })?;
    let accepted = answer
        .get("data")
        .and_then(|data| data.get("issueUpdate"))
        .and_then(|update| update.get("success"))
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    if !accepted {
        let messages: Vec<&str> = graphql_errors(&answer)
            .into_iter()
            .map(error_message)
            .collect();
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-move-refused",
            "world": "unknown",
            "detail": format!(
                "Linear did not accept moving {identity} to {target:?}: {}",
                messages.join("; ")
            ),
            "action": "read the issue in Linear before moving it again; a credential that \
                       can only read cannot move it",
        })));
    }
    let now = read_states(identity)?.current;
    if !now.eq_ignore_ascii_case(target) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-move-not-read-back",
            "world": "unknown",
            "detail": format!(
                "{identity} was moved to {target:?} and reads back as {now:?}"
            ),
            "action": "read the issue in Linear; do not report it moved",
        })));
    }
    Ok(serde_json::json!({ "ok": true, "moved": true, "from": from, "state": now }))
}

struct States {
    id: String,
    current: String,
    offered: Vec<(String, String)>,
}

fn read_states(identity: &Identity) -> Result<States, Failure> {
    let variables = serde_json::json!({ "id": identity.to_string() });
    let value = json_of(&post(STATES_QUERY, &variables, How::read())?)?;
    let issue = value
        .get("data")
        .and_then(|data| data.get("issue"))
        .filter(|issue| !issue.is_null());
    let Some(issue) = issue else {
        if graphql_errors(&value)
            .into_iter()
            .any(is_authentication_error)
        {
            return Err(credential_rejected());
        }
        return Err(issue_unknown(identity));
    };
    let text = |value: Option<&serde_json::Value>| {
        value
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let states = issue
        .get("team")
        .and_then(|team| team.get("states"))
        .and_then(|states| states.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .map(|node| (text(node.get("id")), text(node.get("name"))))
                .collect()
        })
        .unwrap_or_default();
    Ok(States {
        id: text(issue.get("id")),
        current: text(issue.get("state").and_then(|state| state.get("name"))),
        offered: states,
    })
}

/// `answer` with the Linear issue moved to the state `State names` maps
/// `state` to, under `linear_state`; unchanged where nothing is mapped or the
/// issue is not a Linear key. A move that fails is reported there, never
/// folded into the answer's own `ok`.
pub fn follow_state(
    names: &[(String, String)],
    identity: Option<&Identity>,
    state: &str,
    mut answer: serde_json::Value,
) -> serde_json::Value {
    let Some(identity @ Identity::Key(_)) = identity else {
        return answer;
    };
    let Some((_, target)) = names.iter().find(|(mapped, _)| mapped == state) else {
        return answer;
    };
    let moved = match move_issue(identity, target) {
        Ok(moved) => moved,
        Err(failure) => failure.envelope(),
    };
    if let Some(object) = answer.as_object_mut() {
        object.insert("linear_state".to_owned(), moved);
    }
    answer
}

/// Sends one operation through the CLI and answers Linear's JSON, or the stop
/// that names why there is none.
fn post(query: &str, variables: &serde_json::Value, how: How) -> Result<String, Failure> {
    request(PROGRAM, query, variables, how)
}

fn request(
    program: &str,
    query: &str,
    variables: &serde_json::Value,
    how: How,
) -> Result<String, Failure> {
    let variables = variables.to_string();
    let how = How {
        check: false,
        timeout: Some(DEADLINE),
        ..how
    };
    let answer = run(
        &[program, "api", "--variables-json", &variables, query],
        None,
        how,
    )
    .map_err(|why| {
        if why.detail().contains("not found on PATH") {
            cli_missing()
        } else {
            unreachable_api(&why)
        }
    })?;
    if answer.status == 0 {
        return Ok(answer.stdout);
    }
    // A failed request prints Linear's `errors` on stdout for some answers and
    // on stderr for others; either is the answer `parse` classifies.
    if let Some(json) = [&answer.stdout, &answer.stderr]
        .into_iter()
        .find_map(|text| json_in(text))
    {
        return Ok(json);
    }
    if answer.stderr.contains("No API key configured") {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-credential-missing",
            "detail": "the linear CLI holds no credential, so Linear cannot be read",
            "action": format!(
                "run `linear auth login`, or export {CREDENTIAL_VAR}; nothing was created"
            ),
        })));
    }
    Err(unreachable_api(&Failure::Read(format!(
        "{program} api exited {}: {}",
        answer.status,
        answer.stderr.trim()
    ))))
}

/// The JSON object in `text`, which may follow a line of the CLI's own prose.
fn json_in(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let candidate = text[start..].trim_end();
    serde_json::from_str::<serde_json::Value>(candidate)
        .ok()
        .filter(serde_json::Value::is_object)
        .map(|_| candidate.to_owned())
}

fn cli_missing() -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "linear-cli-missing",
        "detail": format!("the `{PROGRAM}` program is not on PATH, so Linear cannot be read"),
        "action": "install the linear CLI (schpet/linear-cli) and sign in with \
                   `linear auth login`; nothing was created",
    }))
}

fn credential_rejected() -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "linear-credential-rejected",
        "detail": format!(
            "Linear refused the credential the linear CLI sent ({CREDENTIAL_VAR} when set, \
             its stored login otherwise)"
        ),
        "action": "sign in again with `linear auth login`, or check the key can read this \
                   team; nothing was created",
    }))
}

fn graphql_errors(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    value
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .map(|errors| errors.iter().collect())
        .unwrap_or_default()
}

fn error_message(error: &serde_json::Value) -> &str {
    error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

fn is_authentication_error(error: &serde_json::Value) -> bool {
    let kind = error
        .get("extensions")
        .and_then(|extensions| extensions.get("type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let message = error_message(error).to_lowercase();
    kind.contains("authentication")
        || message.contains("authentication")
        || message.contains("api key")
}

fn is_not_found_error(error: &serde_json::Value) -> bool {
    let kind = error
        .get("extensions")
        .and_then(|extensions| extensions.get("type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    kind.contains("not found") || error_message(error).to_lowercase().contains("not found")
}

fn issue_unknown(identity: &Identity) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "linear-issue-unknown",
        "detail": format!("Linear knows no issue {identity}"),
        "action": "check the key, or that the credential can see this team; nothing was created",
    }))
}

/// An answer as JSON. One that is not Linear's JSON is not an answer this
/// source knows: reporting a bare `read-failed` would hide that it was Linear.
fn json_of(text: &str) -> Result<serde_json::Value, Failure> {
    serde_json::from_str(text).map_err(|error| {
        unreachable_api(&Failure::Read(format!(
            "Linear returned unparseable JSON: {error}"
        )))
    })
}

/// Reads the issue out of Linear's answer, or names the key as unknown.
fn parse(text: &str, identity: &Identity) -> Result<Issue, Failure> {
    let value = json_of(text)?;
    let data = value.get("data").filter(|data| !data.is_null());
    let errors = graphql_errors(&value);
    if data.is_none() && !errors.is_empty() {
        if errors.iter().any(|error| is_authentication_error(error)) {
            return Err(credential_rejected());
        }
        if errors.iter().any(|error| is_not_found_error(error)) {
            return Err(issue_unknown(identity));
        }
        let messages: Vec<&str> = errors.iter().map(|error| error_message(error)).collect();
        return Err(unreachable_api(&Failure::Read(format!(
            "Linear answered errors: {}",
            messages.join("; ")
        ))));
    }
    let issue = data.and_then(|data| data.get("issue"));
    let Some(issue) = issue.filter(|issue| !issue.is_null()) else {
        return Err(issue_unknown(identity));
    };
    let finished = issue
        .get("state")
        .and_then(|state| state.get("type"))
        .and_then(serde_json::Value::as_str)
        .filter(|kind| matches!(*kind, "completed" | "canceled"));
    if let Some(kind) = finished {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "issue-not-open",
            "detail": format!("issue {identity} is {kind} in Linear, not open"),
            "action": "stop; nothing was created, and the issue is not one to work",
        })));
    }
    Ok(Issue {
        title: issue
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        description: issue
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

fn unreachable_api(why: &Failure) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "linear-unreachable",
        "detail": format!("Linear could not be reached: {}", why.detail()),
        "action": "retry once Linear answers; nothing was created",
    }))
}

#[cfg(test)]
mod tests;
