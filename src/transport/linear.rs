//! The Linear source.
//!
//! Linear holds an issue's title and description behind a GraphQL API. Under
//! `Workflow: assisted` Estigia reads those two fields and the issue's state
//! type, and refuses a finished issue. It writes one thing, and only where the
//! repository's `State names` row maps a state: the issue's workflow state,
//! through [`MOVE_MUTATION`], read back afterwards. The credential comes from
//! the environment rather than from any file somebody could commit, and every
//! operation this source can send is a constant in this module.
//!
//! `curl` performs the request. The crate's transport has one process boundary
//! ([`super::run`]) and no TLS of its own; rather than grow a second, the read
//! goes through the same boundary, with the endpoint and the credential staged
//! in a file so neither appears in the process argument list. The request body
//! is staged too, and only its path travels on the argument list.

use std::path::PathBuf;

use crate::identity::Identity;

use super::{Failure, How, run};

/// The environment variable the credential is read from.
pub const CREDENTIAL_VAR: &str = "LINEAR_API_KEY";

/// The environment variable a fake endpoint is read from, for tests.
pub const ENDPOINT_VAR: &str = "ESTIGIA_LINEAR_URL";

/// Where Linear's API lives when nothing overrides it.
pub const DEFAULT_ENDPOINT: &str = "https://api.linear.app/graphql";

/// The query that reads an issue to work.
pub const QUERY: &str =
    "query EstigiaIssue($id: String!) { issue(id: $id) { title description state { type } } }";

/// The query that reads an issue's state and the states its team offers.
pub const STATES_QUERY: &str = "query EstigiaStates($id: String!) { issue(id: $id) { id state { \
                                name } team { states { nodes { id name } } } } }";

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
    let body = serde_json::json!({
        "query": QUERY,
        "variables": { "id": identity.to_string() },
    })
    .to_string();
    parse(&post(&body)?, identity)
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
    let body = serde_json::json!({
        "query": MOVE_MUTATION,
        "variables": { "id": id, "stateId": state_id },
    })
    .to_string();
    let answer: serde_json::Value = serde_json::from_str(&post(&body)?).map_err(|error| {
        unreachable_api(&Failure::Read(format!(
            "Linear returned unparseable JSON: {error}"
        )))
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
    let body = serde_json::json!({
        "query": STATES_QUERY,
        "variables": { "id": identity.to_string() },
    })
    .to_string();
    let value: serde_json::Value = serde_json::from_str(&post(&body)?).map_err(|error| {
        unreachable_api(&Failure::Read(format!(
            "Linear returned unparseable JSON: {error}"
        )))
    })?;
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

/// Sends one staged request and answers the body of a 2xx, or the stop that
/// names why there is none.
fn post(body: &str) -> Result<String, Failure> {
    let credential = std::env::var(CREDENTIAL_VAR).unwrap_or_default();
    if credential.trim().is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-credential-missing",
            "detail": format!("{CREDENTIAL_VAR} is unset, so Linear cannot be read"),
            "action": format!(
                "export {CREDENTIAL_VAR} with a read-only Linear API key; nothing was created"
            ),
        })));
    }
    // The endpoint override is a test seam, not a configuration surface: a
    // shipped binary reads the constant and cannot be pointed elsewhere. `test`
    // as well as `debug_assertions`, because the release lane runs the suite in
    // a release profile and the fake endpoint must still be honoured there.
    #[cfg(any(test, debug_assertions))]
    let endpoint = std::env::var(ENDPOINT_VAR).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
    #[cfg(not(any(test, debug_assertions)))]
    let endpoint = DEFAULT_ENDPOINT.to_owned();
    if !safe_in_config(&endpoint) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-credential-malformed",
            "detail": "the Linear endpoint holds a quote, a backslash or a control character, \
                       which the curl configuration cannot carry",
            "action": "nothing was created; this endpoint is not one a shipped binary reads",
        })));
    }
    if !safe_in_config(&credential) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "linear-credential-malformed",
            "detail": format!(
                "the value of {CREDENTIAL_VAR} holds a quote, a backslash or a control \
                 character, which the curl configuration cannot carry"
            ),
            "action": format!("check {CREDENTIAL_VAR}; nothing was created"),
        })));
    }

    let staged = Staged::write(&endpoint, &credential, body).map_err(|error| {
        Failure::Write(format!("the Linear request could not be staged: {error}"))
    })?;
    let arguments = curl_arguments(&staged.config, &staged.body);
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let answer = run(&borrowed, None, How::read()).map_err(|why| unreachable_api(&why));
    drop(staged);
    let answer = answer?;

    let (status, text) = split_status(&answer.stdout);
    match status {
        Some(200..=299) => Ok(text),
        Some(401 | 403) => Err(credential_rejected()),
        Some(_) | None if reports_authentication(&text) => Err(credential_rejected()),
        Some(_) | None => Err(unreachable_api(&Failure::Read(format!(
            "Linear answered {status:?}: {}",
            text.trim()
        )))),
    }
}

fn credential_rejected() -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "linear-credential-rejected",
        "detail": format!("Linear refused the credential in {CREDENTIAL_VAR}"),
        "action": "check the key is valid and can read this team; nothing was created",
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

fn reports_authentication(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok_and(|value| {
        graphql_errors(&value)
            .into_iter()
            .any(is_authentication_error)
    })
}

/// The staged request files, removed when this is dropped — on success, on an
/// early return and on a panic alike.
struct Staged {
    config: PathBuf,
    body: PathBuf,
}

impl Staged {
    fn write(endpoint: &str, credential: &str, body: &str) -> std::io::Result<Self> {
        // A pid alone is shared by every read in one process, so two concurrent
        // reads would stage into one directory and one would remove the other's
        // files; a counter gives each its own.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = crate::paths::scratch_file(&format!("linear-{}-{sequence}", std::process::id()));
        Self::write_in(&dir, endpoint, credential, body, lock_down)
    }

    fn write_in(
        dir: &std::path::Path,
        endpoint: &str,
        credential: &str,
        body: &str,
        narrow: fn(&std::path::Path) -> std::io::Result<()>,
    ) -> std::io::Result<Self> {
        create_private_dir(dir)?;
        let staged = Self {
            config: dir.join("curl.conf"),
            body: dir.join("body.json"),
        };
        crate::paths::replace_atomically(&staged.body, body)?;
        // The credential is written here rather than passed on the command
        // line: an argument list is readable by any process on the machine, and
        // a key in one would be a credential in a place nobody expected.
        crate::paths::replace_atomically(&staged.config, &config_text(endpoint, credential))?;
        narrow(&staged.body)?;
        narrow(&staged.config)?;
        Ok(staged)
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.config);
        let _ = std::fs::remove_file(&self.body);
        if let Some(parent) = self.config.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

/// `-q` comes first: without it curl reads the operator's own `curlrc`, which
/// could redirect the request or add a header this source never wrote.
fn curl_arguments(config: &std::path::Path, body: &std::path::Path) -> Vec<String> {
    vec![
        "curl".to_owned(),
        "-q".to_owned(),
        "--config".to_owned(),
        config.to_string_lossy().into_owned(),
        "--data-binary".to_owned(),
        body_argument(body),
        "--connect-timeout".to_owned(),
        "10".to_owned(),
        "--max-time".to_owned(),
        "30".to_owned(),
        "-w".to_owned(),
        "\n%{http_code}".to_owned(),
    ]
}

/// The body path travels on the argument list, not in the config: curl's config
/// parser unescapes a backslash, so a Windows path written there cannot open.
fn body_argument(body: &std::path::Path) -> String {
    format!("@{}", body.to_string_lossy())
}

fn config_text(endpoint: &str, credential: &str) -> String {
    format!(
        "url = \"{endpoint}\"\n\
         request = \"POST\"\n\
         header = \"Content-Type: application/json\"\n\
         header = \"Authorization: {credential}\"\n\
         silent\n\
         show-error\n"
    )
}

/// Splits curl's output into the HTTP status and the body before it.
fn split_status(stdout: &str) -> (Option<u16>, String) {
    let trimmed = stdout.trim_end_matches(['\n', '\r']);
    match trimmed.rsplit_once('\n') {
        Some((body, status)) => (status.trim().parse::<u16>().ok(), body.to_owned()),
        None => (trimmed.trim().parse::<u16>().ok(), String::new()),
    }
}

/// Reads the issue out of Linear's answer, or names the key as unknown.
fn parse(text: &str, identity: &Identity) -> Result<Issue, Failure> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
        // A 2xx that is not Linear's JSON is not an answer this source knows:
        // reporting a bare `read-failed` would hide that it was Linear.
        unreachable_api(&Failure::Read(format!(
            "Linear returned unparseable JSON: {error}"
        )))
    })?;
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

/// Creates `dir` with a mode only its owner can enter, so the credential inside
/// is not readable by another local user while the read is in flight. The leaf
/// is created non-recursively, so a directory somebody else pre-created at that
/// path is refused rather than reused. On a platform without
/// Unix permissions, the per-user temporary directory is the protection — the
/// same guarantee `scratch_file` already relies on.
#[cfg(unix)]
fn create_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    if let Some(parent) = dir.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    std::fs::DirBuilder::new().mode(0o700).create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(dir)
}

/// Narrows one staged file to its owner, behind the directory that already
/// excludes everybody else.
#[cfg(unix)]
fn lock_down(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn lock_down(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Whether a value can be written into the curl configuration as a quoted
/// string without closing it.
fn safe_in_config(value: &str) -> bool {
    !value.contains(['"', '\\']) && !value.chars().any(char::is_control)
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
