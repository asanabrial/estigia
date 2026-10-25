//! One operation name and its flags, answered **here** rather than by spawning
//! an interpreter. What this module does is what the transport's own `main()`
//! does: read an operation name and a flag list, and call the function that
//! answers it.
//!
//! The flags are the transport's, not a new vocabulary: a flag spelling that
//! drifted from the transport's would be a tool that silently stopped passing
//! something, which is why the names below are the binding's own.
//!
//! **Unknown is refused, never ignored.** An operation this does not know is a
//! caller asking for something that will not happen, and answering `ok` to it —
//! or falling back to the interpreter — is the shape this crate exists to
//! refuse: a result nobody read reported as a result.

use std::path::Path;

use super::claim::Departure;
use super::{Context, Failure};

/// The flags this transport takes with no value at all.
///
/// A closed list, not a guess: every name here is read with [`Flags::on`] and
/// nowhere else — `a_switch_nobody_passed_is_off` scans this file's own source
/// for every `.on("...")` call and checks the two agree. A flag that takes a
/// value which itself starts with `--` (a free-text argument can send one) is
/// exactly what a "next word starting with `--`" guess misreads, so everything
/// not on this list takes the next word as its value unconditionally.
const SWITCHES: &[&str] = &[
    "force",
    "held-by-other",
    "manifest",
    "inline-payload",
    "with-review",
    "start-review",
    "record-verdict",
    "with-text",
    "reproduce",
    "clear",
    "compact",
    "takeover",
    "request-review",
    "identities",
    "include-heading",
    "read-only",
    "no-link",
];

/// The flags of one call, by name.
///
/// Built once so a missing value is answered the same way everywhere. Repeated
/// flags keep the **last**, which is what `argparse` does and therefore what a
/// caller that passes one twice already expects.
struct Flags<'a> {
    named: std::collections::BTreeMap<&'a str, &'a str>,
    present: std::collections::BTreeSet<&'a str>,
}

impl<'a> Flags<'a> {
    fn read(flags: &'a [String]) -> Self {
        let mut named = std::collections::BTreeMap::new();
        let mut present = std::collections::BTreeSet::new();
        let mut at = 0;
        while at < flags.len() {
            let Some(name) = flags[at].strip_prefix("--") else {
                at += 1;
                continue;
            };
            present.insert(name);
            // A known switch never takes the next word as its own value —
            // `--force --json` must not read `force` as `"--json"` and swallow
            // the flag after it. Everything else takes the next word as its
            // value unconditionally, whatever that word looks like, because
            // "starts with `--`" is not a reliable sign of a flag boundary once
            // a value can be free text (`SWITCHES`, above).
            if SWITCHES.contains(&name) {
                at += 1;
                continue;
            }
            match flags.get(at + 1) {
                Some(value) => {
                    named.insert(name, value.as_str());
                    at += 2;
                }
                // The last word in the list names a flag that takes a value,
                // and there is none to read. Left present with no value, so
                // `need` refuses it as `missing-argument` and `get` answers
                // `None` — the same as any other flag nobody supplied a value
                // for, rather than this guessing it was a switch that was
                // never declared one.
                None => at += 1,
            }
        }
        Self { named, present }
    }

    fn get(&self, name: &str) -> Option<&'a str> {
        self.named.get(name).copied()
    }

    fn on(&self, name: &str) -> bool {
        self.present.contains(name)
    }

    /// A flag the operation cannot proceed without.
    ///
    /// `ConfigDefect` and not `Read`: a missing argument is the caller's own
    /// defect, and the transport spells it the same way — exit `2`, the code
    /// that means *the operator's configuration is wrong, or an argument was
    /// malformed*.
    fn need(&self, name: &str, operation: &str) -> Result<&'a str, Failure> {
        let value = self.get(name).ok_or_else(|| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "missing-argument",
                "operation": operation,
                "argument": format!("--{name}"),
            }))
        })?;
        // **There is not the same as says something**, and only the first is
        // being asked: a required argument is one that arrived **and** answered,
        // and none of them has a meaningful empty value.
        if value.trim().is_empty() {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "blank-argument",
                "operation": operation,
                "argument": format!("--{name}"),
                "value": value,
                // Naming the flag in the words, not only in a field beside
                // them — the sentence is what an agent reads.
                "detail": format!("--{name} is there and says nothing"),
            })));
        }
        Ok(value)
    }

    /// A flag that has to be a number.
    fn number(&self, name: &str, operation: &str) -> Result<u64, Failure> {
        let text = self.need(name, operation)?;
        text.trim().parse::<u64>().map_err(|_| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "malformed-argument",
                "operation": operation,
                "argument": format!("--{name}"),
                "value": text,
            }))
        })
    }

    fn path(&self, name: &str) -> Option<&'a Path> {
        self.get(name).map(Path::new)
    }

    /// A flag that is an issue identity: a number, or a tracker key such as
    /// `ENG-123`.
    fn identity(&self, name: &str, operation: &str) -> Result<crate::identity::Identity, Failure> {
        let text = self.need(name, operation)?;
        crate::identity::Identity::parse(text).map_err(|detail| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "malformed-argument",
                "operation": operation,
                "argument": format!("--{name}"),
                "value": text,
                "detail": detail,
            }))
        })
    }
}

/// The refusal for a batch argument that names no slot, or names both forms.
fn capture_argument_defect(operation: &str, argument: &str, detail: &str) -> Failure {
    let mut envelope = serde_json::Map::new();
    envelope.insert("ok".to_owned(), serde_json::json!(false));
    envelope.insert("reason".to_owned(), serde_json::json!("malformed-argument"));
    envelope.insert("operation".to_owned(), serde_json::json!(operation));
    envelope.insert("argument".to_owned(), serde_json::json!(argument));
    envelope.insert("detail".to_owned(), serde_json::json!(detail));
    Failure::ConfigDefect(serde_json::Value::Object(envelope))
}

/// A flag that means something only with the mode that selects it, refused
/// rather than silently dropped when that mode is absent. The caller asked for
/// two different reads at once, and answering one of them is the shape this
/// crate refuses.
fn flag_without_its_mode(operation: &str, flag: &str, detail: &str) -> Failure {
    capture_argument_defect(operation, &format!("--{flag}"), detail)
}

/// Whether a capture refusal is the expected state of a batch rather than a
/// failure that should stop it: a slot a running job already holds, or one
/// whose reviewer has not finished.
fn soft_capture(failure: &Failure, reason: &str) -> bool {
    failure
        .envelope()
        .get("reason")
        .and_then(serde_json::Value::as_str)
        == Some(reason)
}

/// The slots a capture call names: every entry of `--names`, or the one `--name`.
fn capture_names<'a>(f: &Flags<'a>, operation: &str) -> Result<Vec<&'a str>, Failure> {
    match f.get("names") {
        Some(list) => {
            let names: Vec<&str> = list
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .collect();
            if names.is_empty() {
                return Err(capture_argument_defect(
                    operation,
                    "--names",
                    "`--names` names no slot; list at least one",
                ));
            }
            Ok(names)
        }
        None => match f.get("name") {
            Some(name) => Ok(vec![name]),
            None => Err(capture_argument_defect(
                operation,
                "--name",
                "give `--name` for one slot or `--names` for several",
            )),
        },
    }
}

/// One batch slot the call left where it was, carrying the job and deadline the
/// refusal named so the caller need not re-read `review_status` to find them.
fn pending_slot(name: &str, failure: &Failure) -> serde_json::Value {
    let envelope = failure.envelope();
    let field = |key: &str| {
        envelope
            .get(key)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    };
    let mut slot = serde_json::Map::new();
    slot.insert("name".to_owned(), serde_json::json!(name));
    slot.insert("reason_code".to_owned(), field("reason"));
    slot.insert("job".to_owned(), field("job"));
    slot.insert("deadline_at".to_owned(), field("deadline_at"));
    serde_json::Value::Object(slot)
}

/// A merged call's answer: the primary part's own answer, with the later
/// part's answer beside it under its own name. The primary answer is kept
/// whole, so a caller that knew only the old shape still reads it.
fn merged_step(
    mut answer: serde_json::Value,
    key: &str,
    step: serde_json::Value,
) -> serde_json::Value {
    answer[key] = step;
    answer
}

/// Adds the tier and the first `review_status` to a publication's answer when
/// the caller asked for them, and leaves it untouched otherwise.
///
/// Both reads follow a publication and decide nothing: the tier is
/// `base..head` classified, and `review_status` names the one next step with
/// its forecast. A read that fails is reported under
/// `review_reads_unavailable` rather than refusing the call, because the push
/// already landed and a retry would mint a new epoch rather than repeat this
/// one — a refusal here would be the shape this crate refuses, a landed write
/// reported as one that did not happen.
fn with_review_reads(
    context: &Context,
    issue: u64,
    worktree: Option<&Path>,
    with_review: bool,
    (focus, declaration): (Option<&str>, super::tier::Declaration<'_>),
    mut answer: serde_json::Value,
) -> serde_json::Value {
    if !with_review {
        return answer;
    }
    let base = answer["base"].as_str().unwrap_or_default().to_owned();
    let head = answer["head"].as_str().unwrap_or_default().to_owned();
    match super::tier::review_tier_declared(context, &base, &head, worktree, focus, &declaration) {
        Ok(tier) => answer["tier"] = tier,
        Err(failure) => {
            answer["review_reads_unavailable"] = failure.envelope();
            return answer;
        }
    }
    match super::claim::review_status(context, issue) {
        Ok(status) => answer["review_status"] = status,
        Err(failure) => answer["review_reads_unavailable"] = failure.envelope(),
    }
    answer
}

const REVIEW_STATE: &str = "review";

struct ReviewStart<'a> {
    issue: u64,
    run_id: &'a str,
    expect_state: &'a str,
    now: &'a str,
}

fn begin_review(context: &Context, start: &ReviewStart<'_>) -> serde_json::Value {
    if start.expect_state == REVIEW_STATE {
        return serde_json::json!({ "transitioned": true });
    }
    match super::commands::transition(
        context,
        start.issue,
        REVIEW_STATE,
        Some(start.expect_state),
        start.now,
    ) {
        Ok(_) => serde_json::json!({ "transitioned": true }),
        Err(failure) => serde_json::json!({
            "transitioned": false,
            "transition_failure": failure.envelope(),
        }),
    }
}

fn launch_each(
    context: &Context,
    (issue, run_id): (u64, &str),
    names: &[&str],
    batch: bool,
    now: &str,
    launched: &mut Vec<serde_json::Value>,
) -> Result<(), (String, Failure)> {
    for name in names {
        let input = super::claim::CaptureInput {
            issue,
            run_id,
            name,
            wait_seconds: 0,
            now,
        };
        match super::claim::launch_review_input(context, &input) {
            Ok(answer) => launched.push(answer),
            Err(failure) if batch && soft_capture(&failure, "capture-already-running") => {
                launched.push(pending_slot(name, &failure));
            }
            Err(failure) => return Err(((*name).to_owned(), failure)),
        }
    }
    Ok(())
}

fn with_review_start(
    context: &Context,
    start: &ReviewStart<'_>,
    mut report: serde_json::Value,
    mut answer: serde_json::Value,
) -> serde_json::Value {
    let skip = |report: &mut serde_json::Value, cause: &str, detail: serde_json::Value| {
        report["launch_skipped"] = serde_json::json!({ "cause": cause, "detail": detail });
    };
    let next = answer["review_status"]["next"].clone();
    if report["transitioned"] != true {
        skip(&mut report, "transition-failed", serde_json::Value::Null);
    } else if !answer["review_status"].is_object() {
        skip(
            &mut report,
            "review-status-unavailable",
            serde_json::Value::Null,
        );
    } else if next["kind"] != "collect" {
        skip(
            &mut report,
            "next-is-not-collect",
            next["reason_code"].clone(),
        );
    } else {
        let names: Vec<&str> = next["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|input| input["capture"] == "launch_review_input")
            .filter_map(|input| input["name"].as_str())
            .collect();
        let head = answer["head"].as_str().unwrap_or_default();
        let lane = if names.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            super::claim::publication_lane(context, head, answer["pr"].as_u64())
        };
        match lane {
            Err(failure) => skip(
                &mut report,
                "publication-lane-unreadable",
                failure.envelope(),
            ),
            Ok(lane) if lane["state"] == "red" => skip(&mut report, "publication-lane-red", lane),
            Ok(_) => {
                let mut launched = Vec::new();
                let outcome = launch_each(
                    context,
                    (start.issue, start.run_id),
                    &names,
                    true,
                    start.now,
                    &mut launched,
                );
                report["launched"] = serde_json::Value::Array(launched);
                if let Err((name, failure)) = outcome {
                    report["launch_failure"] =
                        serde_json::json!({ "name": name, "failure": failure.envelope() });
                }
            }
        }
    }
    answer["review_start"] = report;
    answer
}

fn with_recorded_verdict(
    context: &Context,
    (issue, run_id): (u64, &str),
    worktree: Option<&Path>,
    now: &str,
    mut answer: serde_json::Value,
) -> serde_json::Value {
    let status = match super::claim::review_status(context, issue) {
        Ok(status) => status,
        Err(failure) => {
            answer["verdict_unavailable"] = failure.envelope();
            return answer;
        }
    };
    let next = &status["next"];
    if next["operation"] != "record_computed_verdict" {
        answer["review_status"] = status;
        return answer;
    }
    let arguments = &next["arguments"];
    let field = |name: &str| arguments[name].as_str().unwrap_or_default();
    let pr = arguments["pr"].as_u64().unwrap_or_default();
    let (epoch, head, base, digest) = (
        field("epoch"),
        field("head"),
        field("base"),
        field("digest"),
    );
    let receipt = super::claim::ReviewTransaction {
        epoch: epoch.to_owned(),
        pr,
        head: head.to_owned(),
        base: base.to_owned(),
        digest: digest.to_owned(),
    };
    if !receipt.is_complete() {
        answer["verdict_unavailable"] = serde_json::json!({ "cause": "receipt-incomplete" });
        return answer;
    }
    let operation_id = super::claim::review_operation_id(
        "review-computed-verdict",
        &[run_id, epoch, &pr.to_string(), head, base, digest],
    );
    let recorded = super::claim::record_computed_verdict(
        context,
        &super::claim::ComputedVerdictReview {
            issue,
            run_id,
            operation_id: &operation_id,
            epoch,
            pr,
            head,
            base,
            digest,
            worktree,
            never_ran_evidence_file: None,
            now,
        },
    );
    match recorded {
        Ok(verdict) => answer["verdict"] = verdict,
        Err(failure) => answer["verdict_failure"] = failure.envelope(),
    }
    answer
}

/// The close-evidence comment a merged `unassign` carries, posted through
/// [`super::commands::comment`] as a plain note — the `comment` that ends a
/// delivery, in the same call as the release. Escaped and attributed exactly
/// as the separate call is, so the prose is the same either way.
fn post_close_evidence(
    context: &Context,
    issue: u64,
    path: &Path,
    run_id: &str,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    super::commands::comment(context, issue, path, Some(run_id), Some("note"), now)
}

/// One spend record a merged `unassign` carries, parsed from its JSON object.
/// The operation id is derived exactly as the tool server derives a standalone
/// `spend`'s, so a merged record and a separate one naming the same measurement
/// read back as one event.
struct MergedSpend {
    operation_id: String,
    phase: String,
    epoch: String,
    pr: u64,
    head: String,
    base: String,
    digest: String,
    conditions: String,
    counts: [u64; 4],
    note: String,
}

impl MergedSpend {
    fn parse(run_id: &str, value: &serde_json::Value) -> Result<Self, Failure> {
        let missing = |field: &str| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "missing-argument",
                "argument": "--spend",
                "detail": format!("--spend has no `{field}`"),
            }))
        };
        let text = |field: &str| -> Result<String, Failure> {
            value
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .ok_or_else(|| missing(field))
        };
        let count = |field: &str| -> Result<u64, Failure> {
            value
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| missing(field))
        };
        let phase = text("phase")?;
        let epoch = text("epoch")?;
        let pr = count("pr")?;
        let head = text("head")?;
        let base = text("base")?;
        let digest = text("digest")?;
        let conditions = text("conditions")?;
        let counts = [
            count("input")?,
            count("tokens")?,
            count("seconds")?,
            count("contexts")?,
        ];
        let note = value
            .get("note")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let pr_field = pr.to_string();
        let operation_id = super::claim::review_operation_id(
            "spend",
            &[
                run_id,
                &phase,
                &epoch,
                &pr_field,
                &head,
                &base,
                &digest,
                &conditions,
            ],
        );
        Ok(Self {
            operation_id,
            phase,
            epoch,
            pr,
            head,
            base,
            digest,
            conditions,
            counts,
            note,
        })
    }

    fn review<'a>(
        &'a self,
        issue: u64,
        run_id: &'a str,
        now: &'a str,
    ) -> super::claim::SpendReview<'a> {
        let [input, tokens, seconds, contexts] = self.counts;
        super::claim::SpendReview {
            issue,
            run_id,
            operation_id: &self.operation_id,
            phase: &self.phase,
            epoch: &self.epoch,
            pr: self.pr,
            head: &self.head,
            base: &self.base,
            digest: &self.digest,
            conditions: &self.conditions,
            input,
            tokens,
            seconds,
            contexts,
            note: &self.note,
            now,
        }
    }
}

/// `--spend` as a list of records: one JSON object or an array of them, every
/// element parsed before any is recorded.
fn parse_merged_spends(run_id: &str, spend: &str) -> Result<Vec<MergedSpend>, Failure> {
    let malformed = |detail: String| {
        Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "malformed-argument",
            "argument": "--spend",
            "detail": detail,
        }))
    };
    let value: serde_json::Value = serde_json::from_str(spend)
        .map_err(|error| malformed(format!("--spend is a JSON object or an array: {error}")))?;
    let records = match value {
        serde_json::Value::Array(records) if !records.is_empty() => records,
        serde_json::Value::Array(_) => {
            return Err(malformed("--spend is an empty array".to_owned()));
        }
        single => vec![single],
    };
    records
        .iter()
        .map(|record| MergedSpend::parse(run_id, record))
        .collect()
}

/// A landed write followed by a refusal: the answer a release gives when its
/// close stopped part-way. A refusal would clear what the pointer recorded
/// about a claim that is still held, so the stop is named in the answer.
fn close_incomplete(
    mut done: serde_json::Map<String, serde_json::Value>,
    step: &str,
    failure: &Failure,
) -> serde_json::Value {
    done.insert("ok".to_owned(), serde_json::json!(false));
    done.insert(
        "close_incomplete".to_owned(),
        serde_json::json!({ "step": step, "failure": failure.envelope() }),
    );
    serde_json::Value::Object(done)
}

/// A release that folds its close into itself, in the order the store forces:
/// every spend record while the issue is still in `review`, the move to `done`,
/// the evidence comment, then the release. Everything refusable without a
/// write is refused first; a stop after a write landed comes back under
/// `close_incomplete` with what did land.
fn merged_release(
    context: &Context,
    departure: &Departure<'_>,
    evidence: Option<&Path>,
    spend: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let (issue, run_id, now) = (departure.issue, departure.run_id, departure.now);
    let spends = match spend {
        Some(spend) => parse_merged_spends(run_id, spend)?,
        None => Vec::new(),
    };
    for record in &spends {
        super::claim::validate_spend(&record.review(issue, run_id, now))?;
    }
    if evidence.is_none() && spends.is_empty() {
        return super::claim::unassign(context, departure);
    }
    let ledger = super::store::Ledger::open(context, issue, now)?;
    for record in &spends {
        super::claim::check_spend_receipt(&ledger, &record.review(issue, run_id, now))?;
    }
    super::claim::check_release_epoch(&ledger, departure)?;

    let mut done = serde_json::Map::new();
    let mut landed = false;
    let stopped = |done: &mut serde_json::Map<_, _>, step: &str, failure: Failure, landed| {
        if landed {
            Ok(close_incomplete(std::mem::take(done), step, &failure))
        } else {
            Err(failure)
        }
    };

    let spend_answer = |mut recorded: Vec<serde_json::Value>| match recorded.len() {
        0 => None,
        1 => Some(recorded.remove(0)),
        _ => Some(serde_json::Value::Array(recorded)),
    };
    let mut recorded = Vec::new();
    for record in &spends {
        match super::claim::record_spend_on(context, &ledger, &record.review(issue, run_id, now)) {
            Ok(answer) => {
                landed = true;
                recorded.push(answer);
            }
            Err(failure) => {
                if let Some(spent) = spend_answer(recorded) {
                    done.insert("spend".to_owned(), spent);
                }
                return stopped(&mut done, "spend", failure, landed);
            }
        }
    }
    if let Some(spent) = spend_answer(recorded) {
        done.insert("spend".to_owned(), spent);
    }

    let state = ledger.state();
    let in_review = match state {
        Ok(state) => state.as_deref() == Some("review"),
        Err(failure) => return stopped(&mut done, "transition", failure, landed),
    };
    if in_review {
        match super::commands::transition_on(context, &ledger, issue, "done", None, now) {
            Ok(moved) => {
                landed = true;
                done.insert("transition".to_owned(), moved);
            }
            Err(failure) => return stopped(&mut done, "transition", failure, landed),
        }
    }
    if let Some(path) = evidence {
        match post_close_evidence(context, issue, path, run_id, now) {
            Ok(posted) => {
                landed = true;
                done.insert("evidence".to_owned(), posted);
            }
            Err(failure) => return stopped(&mut done, "evidence", failure, landed),
        }
    }
    match super::claim::unassign_on(context, &ledger, departure) {
        Ok(serde_json::Value::Object(mut fields)) => {
            fields.extend(done);
            Ok(serde_json::Value::Object(fields))
        }
        Ok(other) => Ok(other),
        Err(failure) => stopped(&mut done, "release", failure, landed),
    }
}

/// Runs one transport operation in this process.
///
/// `now` is passed in rather than read here: the timeline decisions this feeds
/// are compared against stamps the caller already holds, and a function that
/// asks the clock twice can answer two questions about two different moments.
pub fn dispatch(
    context: &Context,
    operation: &str,
    flags: &[String],
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let mode = super::window::state(context);
    let open = matches!(mode, super::window::Mode::Open);
    super::window::with_mode(mode, || {
        super::store::counting(operation, || {
            if open && super::outbox::flush::writes_to_github(operation) {
                super::outbox::flush::before_writing(context, now)?;
            }
            run_operation(context, operation, flags, now)
        })
    })
}

fn run_operation(
    context: &Context,
    operation: &str,
    flags: &[String],
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let f = Flags::read(flags);
    if operation == "review-finding" && f.get("lens").is_some_and(|lens| !lens.is_empty()) {
        return Err(super::claim::recorded_by_capture_only(operation));
    }
    let issue = || super::outbox::resolve(f.number("issue", operation)?);
    let identity = || match f.identity("issue", operation)? {
        crate::identity::Identity::Number(number) => {
            super::outbox::resolve(number).map(crate::identity::Identity::Number)
        }
        key => Ok(key),
    };
    let run_id = || f.need("run-id", operation);

    match operation {
        "config" => super::commands::config(context),
        "ensure-states" => super::commands::ensure_states(context),
        "list-state" => {
            // `--identities` replaces the queue read rather than narrowing it,
            // so a call naming both asks for two answers and gets one.
            if f.on("identities") && f.get("state").is_some() {
                return Err(capture_argument_defect(
                    operation,
                    "--state",
                    "give --state for one state's queue or --identities, never both",
                ));
            }
            // 200, the transport's own default — a partial read offered as the
            // state is not the state.
            let limit = f.get("limit");
            let limit = limit
                .and_then(|limit| limit.trim().parse::<u32>().ok())
                .unwrap_or(200);
            if f.on("identities") {
                super::commands::list_identities(context, limit)
            } else {
                let state = f.get("state").ok_or_else(|| {
                    Failure::ConfigDefect(serde_json::json!({
                        "ok": false,
                        "reason": "missing-argument",
                        "operation": operation,
                        "argument": "--state",
                        "detail": "give --state for one state's queue, or --identities",
                    }))
                })?;
                if state.trim().is_empty() {
                    return Err(Failure::ConfigDefect(serde_json::json!({
                        "ok": false,
                        "reason": "blank-argument",
                        "operation": operation,
                        "argument": "--state",
                        "value": state,
                        "detail": "--state is there and says nothing",
                    })));
                }
                super::commands::list_state(context, state, run_id()?, limit, now)
            }
        }
        "comment" => super::commands::comment(
            context,
            issue()?,
            Path::new(f.need("body-file", operation)?),
            f.get("run-id"),
            f.get("kind"),
            now,
        ),
        "create" => {
            super::commands::findings_require_an_epoch(f.get("epoch"), f.get("findings"))?;
            super::commands::create(
                context,
                f.need("identity", operation)?,
                f.need("title", operation)?,
                Path::new(f.need("body-file", operation)?),
                // Defaulted, not required: the transport defaults `state` to
                // `ready` and the tool marks it optional.
                f.get("state").unwrap_or("ready"),
                f.get("priority"),
                f.get("domain"),
                f.get("runtime"),
                f.get("run-id"),
                f.get("epoch"),
                f.get("findings"),
                f.get("blocked-until"),
                now,
            )
        }
        "blocked-until" => {
            run_id()?;
            super::commands::blocked_until(
                context,
                issue()?,
                f.get("until-issue")
                    .and_then(|number| number.trim().parse::<u64>().ok()),
                f.get("until-state"),
                f.on("clear"),
                now,
            )
        }
        "task" => super::commands::task(
            context,
            &super::commands::TaskCall {
                issue: issue()?,
                run_id: run_id()?,
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                id: f.need("id", operation)?,
                text: f.get("text"),
                route: f.get("route"),
                status: f.get("status"),
                commit: f.get("commit"),
                evidence: f.get("evidence"),
                compact: f.on("compact"),
                now,
            },
        ),
        "tasks" => super::commands::tasks(context, issue()?),
        "transition" => super::commands::transition(
            context,
            issue()?,
            f.need("to", operation)?,
            f.get("from"),
            now,
        ),
        "changelog-notes" => super::commands::changelog_notes(
            context,
            Path::new(f.need("file", operation)?),
            f.need("version", operation)?,
            f.on("include-heading"),
            f.path("out"),
        ),
        "check-closing-keywords" => super::commands::check_closing_keywords(
            context,
            issue()?,
            f.get("base"),
            f.get("branch"),
        ),
        "review-tier" => {
            if f.get("parent-head").is_none() && f.get("clearing-finding").is_some() {
                return Err(flag_without_its_mode(
                    operation,
                    "clearing-finding",
                    "--clearing-finding is read only with --parent-head, which names the repair \
                     range it qualifies",
                ));
            }
            let declaration = super::tier::Declaration {
                raise: f.get("raise"),
                raise_reason: f.get("raise-reason"),
                focus_reason: f.get("focus-reason"),
            };
            match f.get("parent-head") {
                Some(parent_head) => super::tier::review_tier_for_repair_declared(
                    context,
                    f.need("base", operation)?,
                    parent_head,
                    f.need("head", operation)?,
                    f.path("worktree"),
                    f.get("clearing-finding"),
                    f.get("focus"),
                    &declaration,
                ),
                None => super::tier::review_tier_declared(
                    context,
                    f.need("base", operation)?,
                    f.need("head", operation)?,
                    f.path("worktree"),
                    f.get("focus"),
                    &declaration,
                ),
            }
        }
        "target" => {
            if f.get("recorded-base").is_some() {
                if f.get("native-start").is_some() {
                    return Err(flag_without_its_mode(
                        operation,
                        "native-start",
                        "--native-start compares a reviewer's target, and --recorded-base answers \
                         base movement instead, which has no target to compare",
                    ));
                }
                if f.on("manifest") {
                    return Err(flag_without_its_mode(
                        operation,
                        "manifest",
                        "--manifest adds the manifest array to the target answer, and \
                         --recorded-base answers base movement instead, which carries no manifest",
                    ));
                }
            }
            match f.get("recorded-base") {
                Some(recorded_base) => super::commands::base_movement(
                    context,
                    f.need("base", operation)?,
                    recorded_base,
                    f.path("worktree"),
                ),
                None => super::target::expected_target(
                    context,
                    f.need("base", operation)?,
                    f.path("worktree"),
                    // `--native-start`: a manifest a reviewer was given, and any
                    // disagreement with it fails closed.
                    f.path("native-start"),
                    f.on("manifest"),
                ),
            }
        }
        "verify-claim" => super::claim::verify_claim(
            context,
            issue()?,
            run_id()?,
            f.need("expect-state", operation)?,
            now,
            f.get("allow-closed-by-pr")
                .and_then(|number| number.trim().parse::<u64>().ok()),
        ),
        "heartbeat" => super::claim::heartbeat(
            context,
            issue()?,
            run_id()?,
            f.need("expect-state", operation)?,
            Path::new(f.need("body-file", operation)?),
            f.get("kind").unwrap_or("heartbeat"),
            now,
        ),
        "claim" => {
            let issue = issue()?;
            if !f.on("takeover") {
                if f.on("force") {
                    return Err(flag_without_its_mode(
                        operation,
                        "force",
                        "--force overrides the staleness check only for a takeover, and no \
                         --takeover was given",
                    ));
                }
                if f.get("reason-file").is_some() {
                    return Err(flag_without_its_mode(
                        operation,
                        "reason-file",
                        "--reason-file is the evidence a forced takeover carries, and no \
                         --takeover was given",
                    ));
                }
            }
            if f.on("takeover") {
                super::claim::reclaim(
                    context,
                    &super::claim::Reclaim {
                        issue,
                        run_id: run_id()?,
                        runtime: f.need("runtime", operation)?,
                        horizon: f.need("horizon", operation)?,
                        operation_id: f.need("operation-id", operation)?,
                        target_operation: f.get("target-operation"),
                        force: f.on("force"),
                        reason_file: f.path("reason-file"),
                        now,
                    },
                )
            } else {
                let acquisition = super::claim::Acquisition {
                    issue,
                    run_id: run_id()?,
                    runtime: f.need("runtime", operation)?,
                    horizon: f.need("horizon", operation)?,
                    operation_id: f.need("operation-id", operation)?,
                    now,
                };
                // The `claim` then `transition` pair as one call: the claim is
                // adjudicated first, so its refusal is what a caller meets before
                // anything moves, and the transition's own refusals follow it.
                match f.get("transition-to") {
                    Some(to) => {
                        super::commands::require_state(to)?;
                        super::claim::claim_checks(&acquisition)?;
                        let ledger = super::store::Ledger::open(context, issue, now)?;
                        let answer = super::claim::claim_on(context, &ledger, &acquisition)?;
                        let moved =
                            super::commands::transition_on(context, &ledger, issue, to, None, now)?;
                        Ok(merged_step(answer, "transition", moved))
                    }
                    None => super::claim::claim(context, &acquisition),
                }
            }
        }
        "unassign" => {
            let departure = Departure {
                issue: issue()?,
                run_id: run_id()?,
                runtime: f.need("runtime", operation)?,
                operation_id: f.need("operation-id", operation)?,
                target_operation: f.get("target-operation"),
                held_by_other: f.on("held-by-other"),
                worktree: f.path("worktree"),
                now,
            };
            if departure.target_operation.is_none() {
                return super::claim::unassign(context, &departure);
            }
            merged_release(context, &departure, f.path("evidence-file"), f.get("spend"))
        }
        "handoff" => super::handoff::record(
            context,
            &super::handoff::Handoff {
                issue: issue()?,
                runtime: f.need("to", operation)?,
                run_id: run_id()?,
                now,
            },
        ),
        "handoff-review" => super::claim::handoff_review(
            context,
            &super::claim::HandoffReview {
                issue: issue()?,
                run_id: run_id()?,
                runtime: f.need("runtime", operation)?,
                operation_id: f.need("operation-id", operation)?,
                target_operation: f.need("target-operation", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                blocker: f.need("blocker", operation)?,
                discharger: f.need("discharger", operation)?,
                now,
            },
        ),
        "review-finding" => super::claim::record_review_finding(
            context,
            &super::claim::FindingReview {
                issue: issue()?,
                run_id: run_id()?,
                reviewer: f.need("reviewer", operation)?,
                operation_id: f.need("operation-id", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                id: f.need("id", operation)?,
                class: f.need("class", operation)?,
                evidence: f.need("evidence", operation)?,
                impact: f.need("impact", operation)?,
                // Optional, and empty is the meaningful value rather than a
                // missing one: a first publication has no parent ledger to
                // continue, and most findings are not new blockers against a
                // repair. `need` refuses an empty string for exactly the flags
                // where empty names nothing.
                parent: f.get("parent").unwrap_or_default(),
                origin: f.get("origin").unwrap_or_default(),
                // Read the same way as `parent` and `origin`: absent and empty
                // both reach the transport as undeclared, and only `severe` is
                // refused over the absence — see `record_review_finding`.
                evidence_class: f.get("evidence-class").unwrap_or_default(),
                causal_disposition: f.get("causal-disposition").unwrap_or_default(),
                location: f.get("location").unwrap_or_default(),
                // Empty means this finding is a judge's, the same rule
                // `parent`, `origin`, `evidence-class` and `causal-disposition`
                // already follow above.
                lens: f.get("lens").unwrap_or_default(),
                now,
            },
        ),
        "review-disposition" => super::claim::record_disposition(
            context,
            &super::claim::DispositionReview {
                issue: issue()?,
                run_id: run_id()?,
                operation_id: f.need("operation-id", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                dispositions: f.need("dispositions", operation)?,
                now,
            },
        ),
        "launch-review-input" => {
            let issue = issue()?;
            let run_id = run_id()?;
            if f.get("names").is_some() && f.get("name").is_some() {
                return Err(capture_argument_defect(
                    operation,
                    "--names",
                    "give either `--name` for one slot or `--names` for several, never both",
                ));
            }
            let batch = f.get("names").is_some();
            let names = capture_names(&f, operation)?;
            let mut launched = Vec::with_capacity(names.len());
            launch_each(context, (issue, run_id), &names, batch, now, &mut launched)
                .map_err(|(_, failure)| failure)?;
            if batch {
                Ok(serde_json::json!({
                    "ok": true,
                    "issue": issue,
                    "launched": launched,
                }))
            } else {
                Ok(launched.pop().unwrap_or(serde_json::Value::Null))
            }
        }
        "collect-review-input" => {
            let issue = issue()?;
            let run_id = run_id()?;
            if f.get("names").is_some() && f.get("name").is_some() {
                return Err(capture_argument_defect(
                    operation,
                    "--names",
                    "give either `--name` for one slot or `--names` for several, never both",
                ));
            }
            let wait_seconds = match f.get("wait-seconds") {
                Some(text) => text.trim().parse::<u64>().map_err(|_| {
                    Failure::ConfigDefect(serde_json::json!({
                        "ok": false,
                        "reason": "malformed-argument",
                        "operation": operation,
                        "argument": "--wait-seconds",
                        "value": text,
                    }))
                })?,
                None => 0,
            };
            let batch = f.get("names").is_some();
            let names = capture_names(&f, operation)?;
            let mut collected = Vec::with_capacity(names.len());
            for name in &names {
                let input = super::claim::CaptureInput {
                    issue,
                    run_id,
                    name,
                    wait_seconds,
                    now,
                };
                match super::claim::collect_review_input(context, &input) {
                    Ok(answer) => collected.push(answer),
                    Err(failure) if batch && soft_capture(&failure, "capture-running") => {
                        collected.push(pending_slot(name, &failure));
                    }
                    Err(failure) => return Err(failure),
                }
            }
            let running = collected
                .iter()
                .any(|slot| slot["reason_code"] == "capture-running");
            let mut answer = if batch {
                serde_json::json!({
                    "ok": true,
                    "issue": issue,
                    "collected": collected,
                })
            } else {
                collected.pop().unwrap_or(serde_json::Value::Null)
            };
            if f.on("record-verdict") {
                answer = if running {
                    answer["verdict_skipped"] = serde_json::json!("capture-running");
                    answer
                } else {
                    with_recorded_verdict(context, (issue, run_id), f.path("worktree"), now, answer)
                };
            }
            Ok(answer)
        }
        "review-computed-verdict" => super::claim::record_computed_verdict(
            context,
            &super::claim::ComputedVerdictReview {
                issue: issue()?,
                run_id: run_id()?,
                operation_id: f.need("operation-id", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                worktree: f.path("worktree"),
                never_ran_evidence_file: f.path("never-ran-evidence-file"),
                now,
            },
        ),
        "escalate-review" => super::claim::escalate_review(
            context,
            &super::claim::ReviewEscalation {
                issue: issue()?,
                run_id: run_id()?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                escalated_to: f.need("escalated-to", operation)?,
                reason: f.need("reason", operation)?,
                now,
            },
        ),
        "publish-review" => {
            let issue = issue()?;
            let declaration = super::tier::Declaration {
                raise: f.get("raise"),
                raise_reason: f.get("raise-reason"),
                focus_reason: f.get("focus-reason"),
            };

            let start = ReviewStart {
                issue,
                run_id: run_id()?,
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                now,
            };
            let answer = super::claim::publish_review(
                context,
                &super::claim::Publication {
                    issue,
                    run_id: start.run_id,
                    expect_state: start.expect_state,
                    base: f.need("base", operation)?,
                    branch: f.need("branch", operation)?,
                    title: f.need("pr-title", operation)?,
                    pr_body_file: f.path("pr-body-file"),
                    worktree: f.path("worktree"),
                    // `--inline-payload`: carry the payload's full text in the
                    // answer. Defaults to off, so an ordinary publication's
                    // answer stays inside what the MCP surface can return.
                    inline_payload: f.on("inline-payload"),
                    // `--topology`/`--topology-reason`: both free text, both take a
                    // value, so neither belongs on `SWITCHES` above — a boolean
                    // missing from that list swallows the next word, a value flag
                    // missing from it is simply read the ordinary way.
                    topology: f.get("topology"),
                    topology_reason: f.get("topology-reason"),
                    focus: f.get("focus"),
                    declaration,
                    expected_epoch: f.get("expected-epoch"),
                    now,
                },
            )?;
            let starting = f.on("start-review");
            let report = starting.then(|| begin_review(context, &start));
            let answer = with_review_reads(
                context,
                issue,
                f.path("worktree"),
                f.on("with-review") || starting,
                (f.get("focus"), declaration),
                answer,
            );
            Ok(match report {
                Some(report) => with_review_start(context, &start, report, answer),
                None => answer,
            })
        }
        // The same arguments as `publish-review`, read the same way, because the
        // two differ in the push and in nothing else. Spelled out rather than
        // shared with the arm above: the flag guard reads each arm's own body,
        // so an arm that borrowed its neighbour's would be an operation whose
        // arguments nothing checks.
        "republish-review" => {
            let issue = issue()?;
            let declaration = super::tier::Declaration {
                raise: f.get("raise"),
                raise_reason: f.get("raise-reason"),
                focus_reason: f.get("focus-reason"),
            };

            let start = ReviewStart {
                issue,
                run_id: run_id()?,
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                now,
            };
            let answer = super::claim::republish_review(
                context,
                &super::claim::Publication {
                    issue,
                    run_id: start.run_id,
                    expect_state: start.expect_state,
                    base: f.need("base", operation)?,
                    branch: f.need("branch", operation)?,
                    title: f.need("pr-title", operation)?,
                    pr_body_file: f.path("pr-body-file"),
                    worktree: f.path("worktree"),
                    inline_payload: f.on("inline-payload"),
                    topology: f.get("topology"),
                    topology_reason: f.get("topology-reason"),
                    focus: f.get("focus"),
                    declaration,
                    expected_epoch: f.get("expected-epoch"),
                    now,
                },
            )?;
            let starting = f.on("start-review");
            let report = starting.then(|| begin_review(context, &start));
            let answer = with_review_reads(
                context,
                issue,
                f.path("worktree"),
                f.on("with-review") || starting,
                (f.get("focus"), declaration),
                answer,
            );
            Ok(match report {
                Some(report) => with_review_start(context, &start, report, answer),
                None => answer,
            })
        }
        "spend" => super::claim::record_spend(
            context,
            &super::claim::SpendReview {
                issue: issue()?,
                run_id: run_id()?,
                operation_id: f.need("operation-id", operation)?,
                phase: f.need("phase", operation)?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                conditions: f.need("conditions", operation)?,
                input: f.number("input", operation)?,
                tokens: f.number("tokens", operation)?,
                seconds: f.number("seconds", operation)?,
                contexts: f.number("contexts", operation)?,
                note: f.get("note").unwrap_or_default(),
                now,
            },
        ),
        "review-status" => {
            let issue = issue()?;
            let mut answer = super::claim::review_status(context, issue)?;
            let lane = answer["receipt"]["head"].as_str().map(|head| {
                super::claim::publication_lane(context, head, answer["receipt"]["pr"].as_u64())
            });
            answer["lane"] = match lane {
                Some(Ok(value)) => value,
                Some(Err(failure)) => failure.envelope(),
                None => serde_json::Value::Null,
            };
            answer["corrections"] = match super::claim::repair_rounds(context, issue, now) {
                Ok(value) => value,
                Err(failure) => failure.envelope(),
            };
            Ok(answer)
        }
        "review-read" => {
            let issue = issue()?;
            let view = f.need("view", operation)?;
            let missing = |name: &str| {
                Failure::ConfigDefect(serde_json::json!({
                    "ok": false,
                    "reason": "missing-argument",
                    "operation": operation,
                    "argument": format!("--{name}"),
                    "detail": format!("view `{view}` needs --{name}"),
                }))
            };
            match view {
                "transaction" => {
                    super::claim::review_transaction(context, issue, f.get("epoch"), now)
                }
                "findings" => super::claim::review_findings(
                    context,
                    issue,
                    f.get("epoch"),
                    f.get("id"),
                    f.on("with-text"),
                    now,
                ),
                "verdict" => {
                    let pr = f.get("pr");
                    let pr = pr
                        .and_then(|number| number.trim().parse::<u64>().ok())
                        .ok_or_else(|| missing("pr"))?;
                    super::claim::show_computed_verdict(
                        context,
                        issue,
                        &super::claim::ReviewTransaction {
                            epoch: f.get("epoch").ok_or_else(|| missing("epoch"))?.to_owned(),
                            pr,
                            head: f.get("head").ok_or_else(|| missing("head"))?.to_owned(),
                            base: f.get("base").ok_or_else(|| missing("base"))?.to_owned(),
                            digest: f.get("digest").ok_or_else(|| missing("digest"))?.to_owned(),
                        },
                        f.path("worktree"),
                        f.on("reproduce"),
                        now,
                    )
                }
                "membership" => super::claim::review_payload_membership(
                    context,
                    issue,
                    f.get("epoch"),
                    Path::new(f.get("candidate").ok_or_else(|| missing("candidate"))?),
                    f.path("worktree"),
                    now,
                ),
                "inspect" => super::claim::inspect_candidate(
                    context,
                    issue,
                    &super::claim::InspectRequest {
                        epoch: f.get("epoch").ok_or_else(|| missing("epoch"))?,
                        request: f.get("request").ok_or_else(|| missing("request"))?,
                        path: f.get("path"),
                        side: f.get("side"),
                        worktree: f.path("worktree"),
                    },
                    now,
                ),
                "spend" => super::claim::spend_summary(
                    context,
                    issue,
                    f.get("phase").ok_or_else(|| missing("phase"))?,
                    now,
                ),
                other => Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false,
                    "reason": "malformed-argument",
                    "operation": operation,
                    "argument": "--view",
                    "value": other,
                    "detail": "view must be transaction, findings, verdict, membership, inspect \
                               or spend",
                }))),
            }
        }
        "release-ci" => super::claim::release_ci(
            context,
            &super::claim::CiRelease {
                issue: issue()?,
                run_id: run_id()?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                worktree: f.path("worktree"),
                now,
            },
        ),
        "start-branch" => super::branch::start_branch(
            context,
            &super::branch::Start {
                issue: crate::identity::Identity::Number(issue()?),
                branch: f.need("branch", operation)?,
                base: f.need("base", operation)?,
                run_id: run_id()?,
                worktree_root: f.get("worktree-root"),
                // Derived from the checkout when the caller does not name it:
                // the MCP tool's argument list has no `repo_name` in any
                // spelling, so a required flag refused every call an agent
                // could make.
                repo_name: &match f.get("repo-name") {
                    Some(named) => named.to_owned(),
                    None => super::closing::repo_identity(context)?.1,
                },
                // The transport defaults this to `in-progress` and the tool
                // marks it optional.
                expect_state: f.get("expect-state").unwrap_or("in-progress"),
                link: !f.on("no-link"),
                now,
            },
            // The operator's own `Worktree location`, which the flag overrides
            // and does not replace.
            context.get("worktree location"),
        ),
        "judge-workspaces" => super::judges::judge_workspaces(
            context,
            &super::judges::Panel {
                issue: crate::identity::Identity::Number(issue()?),
                run_id: run_id()?,
                epoch: f.need("epoch", operation)?,
                pr: f.number("pr", operation)?,
                head: f.need("head", operation)?,
                base: f.need("base", operation)?,
                digest: f.need("digest", operation)?,
                count: f.number("count", operation)?,
                worktree_root: f.get("worktree-root"),
            },
            context.get("worktree location"),
        ),
        "assist" => {
            let issue = identity()?;
            let started = super::branch::assist(
                context,
                &super::branch::Start {
                    issue: issue.clone(),
                    branch: f.need("branch", operation)?,
                    base: f.need("base", operation)?,
                    run_id: run_id()?,
                    worktree_root: f.get("worktree-root"),
                    repo_name: &super::closing::repo_identity(context)?.1,
                    expect_state: "in-progress",
                    link: false,
                    now,
                },
                // The operator's own `Worktree location`, which the flag overrides
                // and does not replace.
                context.get("worktree location"),
            )?;
            Ok(super::linear::follow_state(
                &context.layered().state_names,
                Some(&issue),
                super::linear::STARTED,
                started,
            ))
        }
        "flush" => {
            if f.on("read-only") {
                super::outbox::flush::status(context)
            } else {
                let root = crate::harness::session::state_root(None)
                    .map_err(|refusal| Failure::Read(refusal.message))?;
                super::outbox::flush::run(context, &root, now)
            }
        }
        "draft-pr" => super::draft::draft_pull_request(
            context,
            &super::draft::Draft {
                branch: f.need("branch", operation)?,
                worktree: Path::new(f.need("worktree", operation)?),
                title: f.need("title", operation)?,
                body_file: Path::new(f.need("body-file", operation)?),
                corrections: context.layered().corrections_after_ready(),
            },
        ),
        "review-stages" => {
            let layered = context.layered();
            let read = super::stages::review_stages(
                context,
                &super::stages::Stages {
                    branch: f.need("branch", operation)?,
                    stages: &layered.external_reviewers,
                    ready_by: layered.ready_by(),
                    merge_by: layered.merge_by(),
                    request: f.on("request-review"),
                },
            )?;
            if read.get("position").and_then(serde_json::Value::as_str) == Some("merged") {
                let issue = f.get("issue").map(|_| identity()).transpose()?;
                return Ok(super::linear::follow_state(
                    &layered.state_names,
                    issue.as_ref(),
                    super::linear::DELIVERED,
                    read,
                ));
            }
            Ok(read)
        }
        "ready-pr" => {
            let layered = context.layered();
            let ready = super::draft::mark_ready(
                context,
                &super::draft::Ready {
                    branch: f.need("branch", operation)?,
                    worktree: Path::new(f.need("worktree", operation)?),
                    ready_by: layered.ready_by(),
                },
            )?;
            let issue = f.get("issue").map(|_| identity()).transpose()?;
            Ok(super::linear::follow_state(
                &layered.state_names,
                issue.as_ref(),
                super::linear::IN_REVIEW,
                ready,
            ))
        }
        _ => Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "unknown-operation",
            "operation": operation,
        }))),
    }
}

#[cfg(test)]
mod tests;
