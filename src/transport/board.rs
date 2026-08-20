//! The project-board mirror.
//!
//! Every method here is **best-effort by design**. The `status:*` label is the
//! authoritative store, so a board that cannot be reached is a quiet skip and
//! never blocks a transition. What is *not* allowed is not trying — a skipped
//! attempt is how a board was once found five states behind.
//!
//! One asymmetry shapes the whole file: `transition` mirrors the board **before**
//! it moves the label, because the fragile, easily-skipped half has to run before
//! anything can short-circuit it. That ordering means a failure escaping
//! [`Board::set_status`] would kill the authoritative write — the exact inverse
//! of this module's promise. So nothing here propagates one: a board problem
//! degrades to a reported skip, and the transition's read-back is what still
//! catches a genuine defect.

use std::path::PathBuf;

use super::{Context, Failure};

const FIELDS_QUERY: &str = r#"
query($login: String!, $number: Int!) {
  OWNER(login: $login) {
    projectV2(number: $number) {
      id
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
  }
}
"#;

const ITEMS_QUERY: &str = r#"
query($login: String!, $number: Int!, $cursor: String) {
  OWNER(login: $login) {
    projectV2(number: $number) {
      items(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          content { ... on Issue { number repository { nameWithOwner } labels(first: 100) { totalCount nodes { name } } } }
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

/// Every project an owner has, a page at a time.
const BOARDS_QUERY: &str = r#"
query($login: String!, $cursor: String) {
  OWNER(login: $login) {
    projectsV2(first: 100, after: $cursor) {
      pageInfo { hasNextPage endCursor }
      nodes { number title closed }
    }
  }
}
"#;

/// How many pages of projects are walked before the read is called unanswerable.
///
/// A bound and not a cap on the answer: reaching it **fails**, because a
/// truncated list offered as the list is a partial read reported as the state.
const BOARDS_MAX_PAGES: usize = 100;

/// How long resolved ids may be believed without asking again.
const CACHE_SECONDS: f64 = 86_400.0;

/// One column of the board's `Status` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// The option id the mutation addresses. Stable across renames, which is why
    /// caching is safe at all.
    pub id: String,
    /// The column's display name.
    pub name: String,
    /// Its description, which real boards use to carry the `status:*` label.
    pub description: String,
}

/// What `meta` resolves once: the ids a mutation needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// The project's node id.
    pub project_id: String,
    /// The `Status` field's node id.
    pub field_id: String,
    /// Every column.
    pub columns: Vec<Column>,
}

/// Which board item, if any, is this repository's issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ItemPick {
    /// The card belongs here.
    Ours { id: String },
    /// The number matched a card from another repository.
    Foreign { belongs_to: String },
    /// No card carries this number.
    Absent,
}

/// The mirror.
#[derive(Debug, Clone)]
pub struct Board {
    /// Whether the mirror is addressable at all.
    pub enabled: bool,
    /// The owner half of the spec, kept even when the number will not parse.
    pub owner: Option<String>,
    /// The project number, when it is a number.
    pub number: Option<i64>,
    /// Why the mirror stood down, when it did.
    pub skip_reason: Option<String>,
    /// Whether resolved ids may come from the cache.
    pub use_cache: bool,
    repo_dir: PathBuf,
    resolved: Option<Meta>,
}

impl Board {
    /// Reads a board spec.
    ///
    /// A spec is a board when it is non-empty, is not the word `none`, and
    /// carries a `/`. A number that will not parse disables the mirror and keeps
    /// the owner — the label is the authoritative store, so an unaddressable
    /// board must never be a failure.
    pub fn parse(spec: &str, context: &Context, use_cache: bool) -> Self {
        let mut board = Self {
            enabled: !spec.is_empty() && !spec.eq_ignore_ascii_case("none") && spec.contains('/'),
            owner: None,
            number: None,
            skip_reason: None,
            use_cache,
            repo_dir: context.repo_dir.clone(),
            resolved: None,
        };
        // Said, not silently dropped. A spec that is not empty and is not
        // `none` is an operator who **asked for a board**, and one without a
        // `/` is that board unaddressable — `config list` goes on reporting it
        // set while the mirror is off. The two shapes an operator reaches for
        // are exactly these: the project's number on its own, and the node id
        // (`PVT_kwDO…`) pasted out of the URL bar.
        //
        // Disabling stays right: the label is the authoritative store, so an
        // unaddressable board must never be a failure. Only the silence was
        // wrong, and `acme/seven` two lines down already had the sentence.
        // Through the one function that holds the rule, so the writer cannot
        // drift from it: `BoardRef::parse` asks the same question before it
        // lets a value into the contract.
        board.skip_reason = board_spec_fault(spec).filter(|_| !board.enabled);
        if board.enabled {
            let (owner, number) = spec.split_once('/').unwrap_or((spec, ""));
            board.owner = Some(owner.to_owned());
            match board_number(number) {
                Some(parsed) => board.number = Some(parsed),
                None => {
                    board.enabled = false;
                    board.skip_reason = Some(format!("unparseable board spec '{spec}'"));
                }
            }
        }
        board
    }
}

/// What is wrong with a board spec, in the reader's own terms.
///
/// The rule lived only here, inside [`Board::parse`], and the **writer** had a
/// looser one of its own: `BoardRef::parse` refused an empty value and one
/// holding `|` — the character that would break the markdown table — and nothing
/// else. Measured on the installed binary:
///
/// ```text
/// estigia config set "Project board" acme/no-numero
/// Project board is now acme/no-numero
/// ```
///
/// Accepted, written into the contract, reported by `config list` as set — and
/// read here as `unparseable board spec 'acme/no-numero'`, which disables the
/// mirror. So `config set`, whose one promise is *"validating it before anything
/// is written"*, wrote a value that turns a feature off, and the only trace is a
/// `skip_reason` inside an operation's answer.
///
/// One rule, one place: the reader keeps it and the writer asks. `None` when the
/// spec addresses a board, or is the operator declining one.
pub fn board_spec_fault(spec: &str) -> Option<String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("none") {
        return None;
    }
    let Some((owner, number)) = spec.split_once('/') else {
        // The two shapes an operator reaches for: the project's number on its
        // own, and the node id pasted out of the URL bar.
        return Some(format!(
            "board spec '{spec}' names no owner: a board is `<owner>/<number>`"
        ));
    };
    if owner.trim().is_empty() {
        return Some(format!(
            "board spec '{spec}' names no owner: a board is `<owner>/<number>`"
        ));
    }
    board_number(number).is_none().then(|| {
        format!("board spec '{spec}' names no project number: a board is `<owner>/<number>`")
    })
}

/// The project number a spec names, read the way the transport reads it.
///
/// `parse::<i64>` and Python's `int` are not the same function, and the
/// difference decides whether the board is mirrored **at all**. Measured:
/// `acme/1_0` turned the mirror on over board 10 on one side and off on the
/// other, and it survives the contract cell's trim, so an ordinary
/// configuration reaches it. One side keeping a column nobody updates, or two
/// sides updating different boards, is not a difference worth having over a
/// numeric literal rule.
///
/// So `int`'s rule, exactly as measured: surrounding whitespace, an optional
/// sign, and single underscores **between** digits — `_10`, `10_`, `1__0` and
/// `+_7` are refused by both.
///
/// One difference is left, named rather than hidden: `int` also reads non-ASCII
/// decimal digits, and `std` has no table to match it without a dependency. A
/// project number written in Arabic-Indic digits would still be mirrored by the
/// transport and skipped here — with a `skip_reason` saying so, which is the
/// half of it that is visible.
fn board_number(text: &str) -> Option<i64> {
    let text = text.trim();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1_i64, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return None;
    }
    let plain: String = digits.chars().filter(|c| *c != '_').collect();
    if !plain.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    plain.parse::<i64>().ok().map(|value| sign * value)
}

impl Board {
    /// Where resolved ids are remembered between runs.
    ///
    /// A **private** subdirectory, not the shared temp root. On a host with a
    /// world-writable temp directory another local account could pre-plant this
    /// file and point the mirror's mutation at project and field ids of its
    /// choosing — bounded by what this token can already write, so not a
    /// privilege escalation, but a confused deputy writing to the wrong board.
    ///
    /// Nothing on this path may fail outward. Identity falls back to a shared
    /// name rather than erroring, for the reason the whole module exists: a
    /// failure here would abort a label move.
    fn cache_path(&self) -> PathBuf {
        let identity = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "shared".to_owned());
        let safe: String = identity
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || ".-_".contains(character) {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        let directory = std::env::temp_dir().join(format!("issue-flow-{safe}"));
        if std::fs::create_dir_all(&directory).is_err() {
            return std::env::temp_dir().join(self.cache_name());
        }
        directory.join(self.cache_name())
    }

    fn cache_name(&self) -> String {
        format!(
            "board-{}-{}.json",
            self.owner.as_deref().unwrap_or_default(),
            self.number.unwrap_or_default()
        )
    }

    /// One GraphQL call.
    fn graphql(
        &self,
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
        super::gh_json(&borrowed, Some(&self.repo_dir))
    }

    /// Tries a user-owned project, then an organisation-owned one.
    ///
    /// `bindings/github.md` records this as a quiet failure mode: for an
    /// organisation project, `user(login:)` returns null rather than erroring, so
    /// the mirror silently never fires. Trying both removes the silence.
    fn owner_scoped(
        &self,
        query: &str,
        variables: &[(&str, &str, bool)],
    ) -> Option<serde_json::Value> {
        for owner_type in ["user", "organization"] {
            let Ok(Some(data)) = self.graphql(&query.replace("OWNER", owner_type), variables)
            else {
                continue;
            };
            if let Some(project) = data
                .get("data")
                .and_then(|data| data.get(owner_type))
                .and_then(|node| node.get("projectV2"))
                && !project.is_null()
            {
                return Some(project.clone());
            }
        }
        None
    }

    /// Project id, `Status` field id and option ids — resolved once and cached.
    ///
    /// `bindings/github.md` names per-transition discovery as *"the overhead that
    /// tempts a run to skip the mirror"*, so the cache exists to remove the
    /// temptation rather than to save API calls. Ids are stable; a renamed column
    /// is caught by the transition read-back anyway.
    pub fn meta(&mut self) -> Option<Meta> {
        if let Some(resolved) = &self.resolved {
            return Some(resolved.clone());
        }
        let cache = self.cache_path();
        if self.use_cache
            && let Ok(text) = std::fs::read_to_string(&cache)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            && mirror_is_fresh(
                value.get("cached_at").and_then(serde_json::Value::as_f64),
                now_seconds(),
            )
            && let Some(meta) = read_meta(value.get("meta").unwrap_or(&serde_json::Value::Null))
        {
            self.resolved = Some(meta.clone());
            return Some(meta);
        }

        let owner = self.owner.clone().unwrap_or_default();
        let number = self.number.unwrap_or_default().to_string();
        let Some(project) = self.owner_scoped(
            FIELDS_QUERY,
            &[("login", &owner, false), ("number", &number, true)],
        ) else {
            self.skip_reason = Some(
                "project not visible (missing `project` scope, or wrong owner/number)".to_owned(),
            );
            return None;
        };

        let status = project
            .get("fields")
            .and_then(|fields| fields.get("nodes"))
            .and_then(serde_json::Value::as_array)
            .and_then(|nodes| {
                nodes.iter().find(|node| {
                    node.get("name").and_then(serde_json::Value::as_str) == Some("Status")
                })
            });
        let Some(status) = status else {
            self.skip_reason = Some("project has no single-select `Status` field".to_owned());
            return None;
        };

        let meta = Meta {
            project_id: text(&project, "id"),
            field_id: text(status, "id"),
            columns: status
                .get("options")
                .and_then(serde_json::Value::as_array)
                .map(|options| options.iter().map(read_column).collect())
                .unwrap_or_default(),
        };
        // Best-effort, like everything else here: a cache that cannot be written
        // costs one extra query and nothing else.
        //
        // Atomic, because **the Python transport reads this same file** while
        // both exist — same name, same directory, by design. `fs::write`
        // truncates before it fills, and a reader arriving inside that window
        // gets half a JSON document, which it would discard as corrupt and
        // re-query for. The crate's own guard caught this.
        let _ = crate::paths::replace_atomically(
            &cache,
            // A clock that would not answer stamps `null`, and `fresh` reads
            // that back as stale rather than as the epoch. The mirror is still
            // written: it is worth having on the next run, when the clock may
            // be back — it is only never worth *trusting* unread.
            &serde_json::json!({ "cached_at": now_seconds(), "meta": write_meta(&meta) })
                .to_string(),
        );
        self.resolved = Some(meta.clone());
        Some(meta)
    }

    /// Matches a workflow state to a board column, by name **or** description.
    ///
    /// Not by description alone. Real boards describe `Analysis`..`Blocked` with
    /// their exact `status:*` labels and then describe `Done` as `closed`,
    /// because that column also tracks the tracker's own closed flag — so a
    /// matcher demanding `status:done` would fail on the one transition that
    /// matters most.
    pub fn column_for(meta: &Meta, state: &str) -> Option<Column> {
        let wanted = state.to_lowercase();
        meta.columns
            .iter()
            .find(|column| {
                let name = column.name.to_lowercase().replace(' ', "-");
                let description = column.description.to_lowercase();
                name == wanted || description == format!("status:{wanted}") || description == wanted
            })
            .cloned()
    }

    /// Every board item, following pagination.
    ///
    /// `items(first:100)` stops finding issues once a board passes a hundred, and
    /// all three callers need the same walk — writing that loop three times is
    /// three places for the cursor handling to drift apart.
    fn items(&mut self) -> Vec<serde_json::Value> {
        let mut found = Vec::new();
        let owner = self.owner.clone().unwrap_or_default();
        let number = self.number.unwrap_or_default().to_string();
        let mut cursor: Option<String> = None;

        loop {
            let held = cursor.clone().unwrap_or_default();
            let mut variables: Vec<(&str, &str, bool)> =
                vec![("login", &owner, false), ("number", &number, true)];
            if cursor.is_some() {
                variables.push(("cursor", &held, false));
            }
            let Some(project) = self.owner_scoped(ITEMS_QUERY, &variables) else {
                self.skip_reason = Some("project not visible while listing items".to_owned());
                return found;
            };
            let items = project.get("items").cloned().unwrap_or_default();
            if let Some(nodes) = items.get("nodes").and_then(serde_json::Value::as_array) {
                found.extend(nodes.iter().cloned());
            }
            let page = items.get("pageInfo").cloned().unwrap_or_default();
            if page.get("hasNextPage").and_then(serde_json::Value::as_bool) != Some(true) {
                return found;
            }
            // A page that claims a successor and names none would loop forever,
            // and a mirror that hangs is worse than one that stops early.
            cursor = page
                .get("endCursor")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);
            if cursor.is_none() {
                // Said, like its sibling above says it. Stopping early is the
                // right call and returning a short list **as the list** is not:
                // `item_id` then finds nothing and `set_status` reports *"issue
                // #N is not on the board"*, which is a statement about the board
                // made from a listing that never finished. The rule this crate
                // is built on is that an unknown result is not clearance, and
                // this was the code saying otherwise.
                self.skip_reason = Some(
                    "the project listing claimed another page and named no cursor, so the item \
                     list stopped short of the end"
                        .to_owned(),
                );
                return found;
            }
        }
    }

    /// The board item id for one issue.
    pub(crate) fn item_id(&mut self, issue: u64, home: &str) -> ItemPick {
        pick_item(&self.items(), issue, home)
    }

    /// Attempts the mirror. Reports what happened; **never fails outward**.
    ///
    /// Every arm returns a description rather than an error, and that is the
    /// point rather than a lapse. `transition` mirrors before it moves the label,
    /// so a failure escaping here would kill the authoritative write. Every
    /// genuine defect this hides still surfaces: the transition's read-back
    /// compares the board against the label immediately afterwards.
    pub fn set_status(&mut self, issue: u64, state: &str, home: &str) -> serde_json::Value {
        if !self.enabled {
            return serde_json::json!({ "attempted": false, "skipped": "no board configured" });
        }
        let Some(meta) = self.meta() else {
            return serde_json::json!({ "attempted": true, "skipped": self.skip_reason });
        };
        let Some(column) = Self::column_for(&meta, state) else {
            return serde_json::json!({
                "attempted": true,
                "skipped": format!("no board column mirrors '{state}'"),
            });
        };
        let item = match self.item_id(issue, home) {
            ItemPick::Ours { id } => id,
            ItemPick::Foreign { belongs_to } => {
                let board = format!(
                    "{}/{}",
                    self.owner.as_deref().unwrap_or("?"),
                    self.number
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or("?".to_owned())
                );
                return foreign_item_report(issue, &board, &belongs_to, home);
            }
            ItemPick::Absent => {
                // *Not on the board* is a fact about the board, and it may only be
                // said from a listing that reached the end. When the walk stopped
                // short — a page claiming a successor with no cursor, or a project
                // that went invisible mid-walk — what is true is that the issue was
                // not found, which is a different sentence.
                return serde_json::json!({
                    "attempted": true,
                    "skipped": match &self.skip_reason {
                        Some(why) => format!("issue #{issue} was not found on the board, and {why}"),
                        None => format!("issue #{issue} is not on the board"),
                    },
                });
            }
        };
        if let Err(failure) = self.graphql(
            SET_MUTATION,
            &[
                ("project", &meta.project_id, false),
                ("item", &item, false),
                ("field", &meta.field_id, false),
                ("option", &column.id, false),
            ],
        ) {
            return serde_json::json!({
                "attempted": true,
                "skipped": failure.detail(),
            });
        }
        serde_json::json!({ "attempted": true, "set_to": column.name })
    }

    /// The column one issue currently sits in.
    pub fn read_status(&mut self, issue: u64) -> Option<String> {
        if !self.enabled {
            return None;
        }
        self.items().into_iter().find_map(|node| {
            let number = node.get("content")?.get("number")?.as_u64()?;
            (number == issue).then(|| {
                node.get("fieldValueByName")
                    .and_then(|value| value.get("name"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
        })
    }

    /// Every card, for the audit.
    pub fn all_cards(&mut self) -> Vec<serde_json::Value> {
        self.items()
            .into_iter()
            .filter_map(|node| {
                let content = node.get("content")?;
                let number = content.get("number")?.as_u64()?;
                let labels: Vec<String> = content
                    .get("labels")
                    .and_then(|labels| labels.get("nodes"))
                    .and_then(serde_json::Value::as_array)
                    .map(|nodes| nodes.iter().map(|node| text(node, "name")).collect())
                    .unwrap_or_default();
                // A window is not a set, and the workflow state is a label.
                // `totalCount` is what tells the audit it did not finish
                // reading, so it can decline to conclude rather than call a
                // correctly labelled issue drift.
                let counted = content
                    .get("labels")
                    .and_then(|labels| labels.get("totalCount"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                Some(serde_json::json!({
                    "issue": number,
                    "repository": content.get("repository").and_then(|r| r.get("nameWithOwner")).cloned().unwrap_or(serde_json::Value::Null),
                    "column": node
                        .get("fieldValueByName")
                        .and_then(|value| value.get("name"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    "labels_complete": counted <= labels.len() as u64,
                    "labels": labels,
                }))
            })
            .collect()
    }
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The refusal when the card for this number belongs to another repository.
///
/// `belongs_to` names the other repo; without `board` the operator cannot tell
/// which project was asked to move it, and without `detail` the four identities
/// never sit in one sentence.
fn foreign_item_report(issue: u64, board: &str, belongs_to: &str, home: &str) -> serde_json::Value {
    serde_json::json!({
        "attempted": true,
        "ok": false,
        "reason": "board-item-foreign-repository",
        "belongs_to": belongs_to,
        "board": board,
        "detail": format!(
            "issue #{issue} on board {board} belongs to {belongs_to}, not {home}"
        ),
        "action": "estigia config set --repo \"Project board\" \"none\"",
    })
}

/// Pick this repository's card, not whichever card happens to carry the number.
///
/// Matching on number alone is how Estigia moved Investora's #73 when this
/// repository created its own #73. Unknown repository is not clearance.
pub(crate) fn pick_item(nodes: &[serde_json::Value], issue: u64, home: &str) -> ItemPick {
    let mut ours = None;
    let mut foreign = None;
    for node in nodes {
        let Some(content) = node.get("content") else {
            continue;
        };
        let Some(number) = content.get("number").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        if number != issue {
            continue;
        }
        let belongs = content
            .get("repository")
            .and_then(|repository| repository.get("nameWithOwner"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if belongs.is_empty() || belongs == home {
            ours = Some(text(node, "id"));
        } else {
            foreign = Some(if belongs.is_empty() {
                "an unnamed repository".to_owned()
            } else {
                belongs.to_owned()
            });
        }
    }
    match (ours, foreign) {
        (Some(id), _) => ItemPick::Ours { id },
        (None, Some(belongs_to)) => ItemPick::Foreign { belongs_to },
        (None, None) => ItemPick::Absent,
    }
}

fn read_column(value: &serde_json::Value) -> Column {
    Column {
        id: text(value, "id"),
        name: text(value, "name"),
        description: text(value, "description"),
    }
}

fn now_seconds() -> Option<f64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs_f64())
        .ok()
}

/// Whether a mirror stamped at `cached_at` is still worth reading at `now`.
///
/// Both sides optional, and either one missing means **re-query**. This was
/// `now_seconds() - at < CACHE_SECONDS` against a clock that defaulted to zero
/// on failure, and zero minus any real timestamp is a large negative number —
/// so a machine that could not say what time it is held a mirror that never
/// went stale, and kept transitioning issues through column ids the board may
/// have stopped using. The same shape as `Run::within_window`, which already
/// answers "ask" to a clock that will not say.
///
/// A stamp **in the future** was the half that fix left: `now - at` is negative
/// there, and every negative number is under the window, so a mirror written
/// while the clock was a year ahead stayed fresh for a year — the same "never
/// went stale" this function was already rewritten once to stop, reached by a
/// clock that answered rather than one that would not. Measured against a stamp
/// a year out: fresh, on this side and on the transport's.
///
/// The third time this crate has met the shape — `harness::standdown`'s `covers`
/// gained `declared_at <= now` for a stand-down stamped ahead, and it is written
/// the same way here rather than with a tolerance. A grace period is a number
/// nobody can defend, and what it would buy is one extra query on a clock that
/// stepped backwards between the write and the read.
pub fn mirror_is_fresh(cached_at: Option<f64>, now: Option<f64>) -> bool {
    match (cached_at, now) {
        (Some(at), Some(now)) => at <= now && now - at < CACHE_SECONDS,
        _ => false,
    }
}

fn read_meta(value: &serde_json::Value) -> Option<Meta> {
    Some(Meta {
        project_id: value.get("project_id")?.as_str()?.to_owned(),
        field_id: value.get("field_id")?.as_str()?.to_owned(),
        columns: value
            .get("options")?
            .as_array()?
            .iter()
            .map(read_column)
            .collect(),
    })
}

fn write_meta(meta: &Meta) -> serde_json::Value {
    serde_json::json!({
        "project_id": meta.project_id,
        "field_id": meta.field_id,
        "options": meta
            .columns
            .iter()
            .map(|column| serde_json::json!({
                "id": column.id,
                "name": column.name,
                "description": column.description,
            }))
            .collect::<Vec<_>>(),
    })
}

/// Every project an owner has, so a board can be chosen rather than guessed.
///
/// `Project board` is configured as `owner/number`, and the number is the one
/// half of that pair nobody knows without opening a browser. This side could
/// read **one** project by number and had no way to say which numbers exist —
/// the last operation the transport offered and the port did not.
///
/// Paged to exhaustion, and the cap **fails** rather than returning what it has.
/// A truncated list of boards offered as the list of boards is a partial read
/// reported as the state, which is the failure the audit's zero-card rule exists
/// to refuse.
///
/// Neither owner shape answering is a failed read and not an empty list: an
/// owner with no projects answers with an empty connection, and saying "none"
/// about a read that never happened is the same lie in a smaller place.
pub fn list_boards(context: &Context, owner: Option<&str>) -> Result<serde_json::Value, Failure> {
    let owner = match owner {
        Some(named) if !named.is_empty() => named.to_owned(),
        _ => super::closing::owner_of(context)?,
    };
    let board = Board {
        enabled: true,
        owner: Some(owner.clone()),
        number: None,
        skip_reason: None,
        use_cache: false,
        repo_dir: context.repo_dir.clone(),
        resolved: None,
    };

    for owner_type in ["user", "organization"] {
        let query = BOARDS_QUERY.replace("OWNER", owner_type);
        let mut boards: Vec<serde_json::Value> = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for page in 0..BOARDS_MAX_PAGES {
            let held = cursor.clone().unwrap_or_default();
            let mut variables: Vec<(&str, &str, bool)> = vec![("login", &owner, false)];
            if cursor.is_some() {
                variables.push(("cursor", &held, false));
            }
            let Ok(Some(data)) = board.graphql(&query, &variables) else {
                break;
            };
            let connection = data
                .get("data")
                .and_then(|data| data.get(owner_type))
                .and_then(|node| node.get("projectsV2"));
            let Some(connection) = connection.filter(|value| value.is_object()) else {
                break;
            };
            // Present and of the right shape, or the read did not answer. The
            // reason lives with `connection_page` now, because it was the rule
            // and this was one of two places applying it.
            let (nodes, page_info) = super::connection_page(connection, "project listing")?;
            for project in nodes {
                let Some(number) = project.get("number").and_then(serde_json::Value::as_u64) else {
                    continue;
                };
                boards.push(serde_json::json!({
                    "number": number,
                    "title": project.get("title").and_then(serde_json::Value::as_str).unwrap_or_default(),
                    "closed": project.get("closed").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    "board": format!("{owner}/{number}"),
                }));
            }
            if page_info
                .get("hasNextPage")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                return Ok(serde_json::json!({
                    "ok": true,
                    "owner": owner,
                    "owner_type": owner_type,
                    "boards": boards,
                }));
            }
            // A page claiming a successor and naming none, or naming one it has
            // already given, would page forever. Both are a read that did not
            // answer rather than a list that ended.
            let next = page_info
                .get("endCursor")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if next.is_empty() || !seen.insert(next.clone()) {
                return Err(Failure::Read(
                    "project listing did not provide a fresh continuation cursor".to_owned(),
                ));
            }
            cursor = Some(next);
            if page + 1 == BOARDS_MAX_PAGES {
                return Err(Failure::Read(
                    "project listing exceeded its safety bound".to_owned(),
                ));
            }
        }
    }
    Err(Failure::Read(format!(
        "no project boards could be read for {owner}"
    )))
}

#[cfg(test)]
mod tests;
