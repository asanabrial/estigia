//! The project-board projection.
//!
//! The `status:*` label is the authoritative store, so every method here is
//! **best-effort**: a project that cannot be reached is a quiet skip and never
//! blocks a transition. Which projects are updated is read from the issue
//! itself — its `projectItems` — so there is nothing to configure.
//!
//! `transition` projects **before** it moves the label, because the fragile,
//! easily-skipped half has to run before anything can short-circuit it. That
//! ordering means a failure escaping [`set_status`] would kill the authoritative
//! write, so nothing here propagates one: a project problem degrades to a
//! reported skip.

use super::{Context, Failure};

const ITEMS_QUERY: &str = r#"
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      projectItems(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          project {
            id
            number
            title
            fields(first: 100) {
              nodes {
                ... on ProjectV2SingleSelectField {
                  id
                  name
                  options { id name description }
                }
              }
            }
          }
          fieldValueByName(name: "Status") {
            ... on ProjectV2ItemFieldSingleSelectValue { name }
          }
        }
      }
    }
  }
}
"#;

const SET_MUTATION: &str = r#"
mutation($project: ID!, $item: ID!, $field: ID!, $option: String!) {
  updateProjectV2ItemFieldValue(input: {
    projectId: $project
    itemId: $item
    fieldId: $field
    value: { singleSelectOptionId: $option }
  }) {
    projectV2Item { id }
  }
}
"#;

/// How many pages of an issue's project items are walked before the read is
/// called unanswerable. A bound and not a cap: reaching it is a failed read.
const ITEMS_MAX_PAGES: usize = 100;

/// One column of a project's `Status` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// The option id the mutation addresses.
    pub id: String,
    /// The column's display name.
    pub name: String,
    /// Its description, which boards use to carry the `status:*` label.
    pub description: String,
}

/// What a mutation needs for one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// The project's node id.
    pub project_id: String,
    /// The `Status` field's node id.
    pub field_id: String,
    /// Every column.
    pub columns: Vec<Column>,
}

impl Meta {
    /// Matches a workflow state to a column, by name **or** description.
    ///
    /// Not by description alone. Real boards describe `Analysis`..`Blocked` with
    /// their exact `status:*` labels and then describe `Done` as `closed`,
    /// because that column also tracks the tracker's own closed flag — so a
    /// matcher demanding `status:done` would fail on the one transition that
    /// matters most.
    pub fn column_for(&self, state: &str) -> Option<Column> {
        let wanted = state.to_lowercase();
        self.columns
            .iter()
            .find(|column| {
                let name = column.name.to_lowercase().replace(' ', "-");
                let description = column.description.to_lowercase();
                name == wanted || description == format!("status:{wanted}") || description == wanted
            })
            .cloned()
    }
}

/// One project the issue belongs to, with its card and current column.
#[derive(Debug, Clone)]
struct Item {
    item_id: String,
    project_id: String,
    number: i64,
    title: String,
    meta: Option<Meta>,
    column: Option<String>,
}

/// `set_status` — project the state onto every project the issue belongs to.
///
/// Best-effort by construction: the answer reports per project what happened and
/// **never fails outward**. The transition's read-back is what still catches a
/// genuine defect.
pub fn set_status(context: &Context, issue: u64, state: &str) -> serde_json::Value {
    let items = match projects(context, issue) {
        Ok(items) => items,
        Err(skip) => return serde_json::json!({ "attempted": true, "skipped": skip }),
    };
    if items.is_empty() {
        return serde_json::json!({
            "attempted": true,
            "projects": [],
            "skipped": format!("issue #{issue} is not on any project"),
        });
    }
    let mut projects = Vec::new();
    for item in &items {
        let Some(meta) = &item.meta else {
            projects.push(item_report(
                item,
                "skipped",
                "project has no single-select `Status` field",
            ));
            continue;
        };
        let Some(column) = meta.column_for(state) else {
            projects.push(item_report(
                item,
                "skipped",
                &format!("no column mirrors '{state}'"),
            ));
            continue;
        };
        match mutation(context, meta, &item.item_id, &column.id) {
            Ok(()) => projects.push(item_report(item, "set_to", &column.name)),
            Err(detail) => projects.push(item_report(item, "skipped", &detail)),
        }
    }
    serde_json::json!({ "attempted": true, "projects": projects })
}

/// `read_status` — the column one issue currently sits in, per project.
///
/// The read-back `transition` covers the board with. Each entry says what the
/// project shows, what the state asked for, and whether they agree.
pub fn read_status(context: &Context, issue: u64, state: &str) -> serde_json::Value {
    let items = match projects(context, issue) {
        Ok(items) => items,
        Err(skip) => return serde_json::json!({ "attempted": true, "skipped": skip }),
    };
    if items.is_empty() {
        return serde_json::json!({
            "attempted": true,
            "projects": [],
            "skipped": format!("issue #{issue} is not on any project"),
        });
    }
    let projects = items
        .iter()
        .map(|item| {
            let expected = item
                .meta
                .as_ref()
                .and_then(|meta| meta.column_for(state))
                .map(|column| column.name);
            let agrees = match &expected {
                None => true,
                Some(expected) => item.column.as_deref() == Some(expected.as_str()),
            };
            serde_json::json!({
                "project": item.project_id,
                "number": item.number,
                "title": item.title,
                "column": item.column,
                "expected": expected,
                "agrees": agrees,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "attempted": true, "projects": projects })
}

/// One project's report, shaped so a reader can tell which it was.
fn item_report(item: &Item, key: &str, value: &str) -> serde_json::Value {
    serde_json::json!({
        "project": item.project_id,
        "number": item.number,
        "title": item.title,
        key: value,
    })
}

/// The projects one issue belongs to, following pagination.
///
/// An unreadable identity or listing is an `Err` carrying the reason, which the
/// callers report as a skip rather than raising.
fn projects(context: &Context, issue: u64) -> Result<Vec<Item>, String> {
    let (owner, name) = super::closing::repo_identity(context).map_err(|failure| {
        format!(
            "this repository's identity could not be read: {}",
            failure.detail()
        )
    })?;
    let number = issue.to_string();
    let mut found = Vec::new();
    let mut cursor: Option<String> = None;

    for page in 0..ITEMS_MAX_PAGES {
        let held = cursor.clone().unwrap_or_default();
        let mut variables: Vec<(&str, &str, bool)> = vec![
            ("owner", owner.as_str(), false),
            ("name", name.as_str(), false),
            ("number", number.as_str(), true),
        ];
        if cursor.is_some() {
            variables.push(("cursor", held.as_str(), false));
        }
        let data = graphql(context, ITEMS_QUERY, &variables).map_err(|failure| failure.detail())?;
        let Some(data) = data else {
            return Err("the issue's project listing did not answer".to_owned());
        };
        let issue_node = data
            .get("data")
            .and_then(|data| data.get("repository"))
            .and_then(|repository| repository.get("issue"));
        let Some(issue_node) = issue_node.filter(|value| !value.is_null()) else {
            return Err(format!(
                "issue #{issue} could not be found in this repository"
            ));
        };
        let connection = issue_node.get("projectItems");
        let Some(connection) = connection.filter(|value| value.is_object()) else {
            return Err("the issue's project listing did not answer".to_owned());
        };
        let (nodes, page_info) = super::connection_page(connection, "project listing")
            .map_err(|failure| failure.detail())?;
        for node in nodes {
            found.push(read_item(node));
        }
        if page_info
            .get("hasNextPage")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Ok(found);
        }
        let next = page_info
            .get("endCursor")
            .and_then(serde_json::Value::as_str)
            .filter(|next| !next.is_empty())
            .map(ToOwned::to_owned);
        // A page claiming a successor and naming none would loop forever, and
        // returning a short list **as the list** would report a missing project
        // rather than an unfinished read.
        match next {
            Some(next) => cursor = Some(next),
            None => {
                return Err(
                    "the project listing claimed another page and named no cursor, so the item \
                     list stopped short of the end"
                        .to_owned(),
                );
            }
        }
        if page + 1 == ITEMS_MAX_PAGES {
            return Err("the project listing exceeded its safety bound".to_owned());
        }
    }
    Err("the project listing exceeded its safety bound".to_owned())
}

fn read_item(node: &serde_json::Value) -> Item {
    let project = node.get("project");
    Item {
        item_id: text(node, "id"),
        project_id: project
            .map(|project| text(project, "id"))
            .unwrap_or_default(),
        number: project
            .and_then(|project| project.get("number"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default(),
        title: project
            .and_then(|project| project.get("title"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        meta: project.and_then(read_meta),
        column: node
            .get("fieldValueByName")
            .and_then(|value| value.get("name"))
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn read_meta(project: &serde_json::Value) -> Option<Meta> {
    let status = project
        .get("fields")
        .and_then(|fields| fields.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node.get("name").and_then(serde_json::Value::as_str) == Some("Status"))
        })?;
    Some(Meta {
        project_id: text(project, "id"),
        field_id: text(status, "id"),
        columns: status
            .get("options")
            .and_then(serde_json::Value::as_array)
            .map(|options| options.iter().map(read_column).collect())
            .unwrap_or_default(),
    })
}

fn read_column(value: &serde_json::Value) -> Column {
    Column {
        id: text(value, "id"),
        name: text(value, "name"),
        description: text(value, "description"),
    }
}

fn mutation(context: &Context, meta: &Meta, item_id: &str, option_id: &str) -> Result<(), String> {
    let answer = graphql(
        context,
        SET_MUTATION,
        &[
            ("project", meta.project_id.as_str(), false),
            ("item", item_id, false),
            ("field", meta.field_id.as_str(), false),
            ("option", option_id, false),
        ],
    )
    .map_err(|failure| failure.detail())?;
    match answer {
        Some(_) => Ok(()),
        None => Err("the project mutation did not answer".to_owned()),
    }
}

fn graphql(
    context: &Context,
    query: &str,
    variables: &[(&str, &str, bool)],
) -> Result<Option<serde_json::Value>, Failure> {
    let mut arguments = vec![
        "api".to_owned(),
        "graphql".to_owned(),
        "-f".to_owned(),
        format!("query={query}"),
    ];
    for (key, value, numeric) in variables {
        // `-F` types the value; `-f` keeps it a string. A project number sent
        // as a string is a type error at the API, not here.
        arguments.push(if *numeric { "-F" } else { "-f" }.to_owned());
        arguments.push(format!("{key}={value}"));
    }
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    super::gh_json(&borrowed, Some(&context.repo_dir))
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests;
