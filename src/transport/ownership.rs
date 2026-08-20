//! Reading the timeline that decides who owns an issue.
//!
//! The first slice of the ownership reducer: the pieces that are **pure**, so
//! their edge cases are testable without a tracker. Everything above them —
//! `reduce_ownership` and the events it folds — rests on these, and a difference
//! here would be a difference in all five commands that read ownership at once.

/// An instant, as seconds since the epoch.
///
/// A parsed instant rather than the string, and that is the whole point.
/// Timestamps arrive at **two precisions** — GitHub writes
/// `2026-07-25T21:54:35Z`, a declared horizon is written `2026-07-26T00:54Z` —
/// and comparing those as strings is wrong in exactly the case that matters:
/// `00:54:00Z` sorts *before* `00:54Z`, because `0` precedes `Z`. A run that had
/// just spoken would be read as silent, and lose its claim.
pub type Stamp = i64;

/// The value an unparseable stamp orders as.
///
/// A horizon is free text a person may have written in prose, and it must
/// compare as *unknown* rather than as some accidental ordering.
pub const UNKNOWN: Stamp = Stamp::MIN;

/// How long a run may stay silent before its claim is stale.
const SILENCE: i64 = 4 * 60 * 60;

/// Parses an ISO-8601 UTC stamp, or `None` when it is not one.
///
/// The zone marker and the time are **both optional**, because the original
/// hands the string to `datetime.fromisoformat` and reads whatever comes back as
/// UTC. `2026-07-26` is midnight on that day there, and a port that rejected it
/// would call a written horizon unreadable — which orders it first and reads a
/// live claim as expired. Found by the differential; the unit test beside this
/// had asserted the opposite.
pub fn parse_stamp(value: &str) -> Option<Stamp> {
    let text = value.trim();
    let text = text.strip_suffix('Z').unwrap_or(text);
    let (date, time) = text.split_once('T').unwrap_or((text, "00:00"));
    // ISO-8601's **basic** form, which `datetime.fromisoformat` accepts and
    // this refused: `20260726T005435Z` was a live horizon to the transport and
    // unreadable here. That is not a harmless difference — [`stamp_rank`] puts
    // an unreadable stamp *last*, so a run carrying one loses a race on this
    // side and wins it on the other.
    //
    // The two halves are independent there, so `20260726T00:54:35Z` and
    // `2026-07-26T005435Z` both parse and `2026-0726T00:54:35Z` does not.
    // Measured against the transport rather than reasoned about.
    let date = &expand_date(date);
    let time = &expand_time(time);
    // An offset belongs to the time, and only there: a date carries `-` of its
    // own. `+00:00` was the one offset this read, so anything else — and
    // `+02:00` is ordinary ISO-8601 — came back unreadable. That was harmless
    // while unreadable only meant "old"; it stopped being harmless when the
    // same value decided who won a claim. See [`stamp_rank`].
    let (time, offset) = split_offset(time);

    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    // Against the month rather than against 31. `2026-02-29` is not a date —
    // 2026 is not a leap year — and it satisfies `HORIZON_RE`, which is the
    // *shape* check both sides run before this one. So the shape let it through
    // and only this decided: the transport's `datetime` refused it and this
    // read it as an instant, which is a horizon one side accepts and the other
    // calls `invalid-horizon`. `2026-04-31`, `2026-06-31`, `2026-09-31` and
    // `2026-11-31` are the same mistake and the natural ones to make.
    //
    // Found by generating stamps rather than by thinking of one.
    if parts.next().is_some() || !(1..=12).contains(&month) || day < 1 || day > days_in(year, month)
    {
        return None;
    }

    let mut clock = time.split(':');
    let hour: i64 = clock.next()?.parse().ok()?;
    let minute: i64 = clock.next()?.parse().ok()?;
    // The second is optional: that is the two-precision case this exists for.
    let second: i64 = match clock.next() {
        Some(second) => second.split('.').next()?.parse().ok()?,
        None => 0,
    };
    // 59, not 60: `datetime` has no leap second, and a stamp this reads and the
    // transport does not is the divergence the rest of this function is about.
    // Hour 24 is ISO-8601's end of day, and `datetime.fromisoformat` takes it:
    // `2026-07-26T24:00Z` is the next day's midnight there and was unreadable
    // here — so that horizon was a live claim to the transport and, through
    // [`stamp_rank`], the **last** thing to rank on this side. A run carrying
    // one loses a race here and wins it there.
    //
    // Only with a zero minute and second, which is where `datetime` draws the
    // line too: `24:01` is refused on both sides. Measured against it rather
    // than reasoned about. The arithmetic below needs no special case — 24
    // hours past midnight *is* the next day.
    let end_of_day = hour == 24 && minute == 0 && second == 0;
    if clock.next().is_some() || (hour > 23 && !end_of_day) || minute > 59 || second > 59 {
        return None;
    }

    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset)
}

/// The time without its UTC offset, and the offset in seconds.
///
/// `+HH:MM`, `-HH:MM` and the compact `+HHMM`, which is the whole of what
/// ISO-8601 allows after a time. Anything else is left on the string, where the
/// clock parser will refuse it — a shape nobody recognised must not be read as
/// a shape with no offset at all.
fn split_offset(time: &str) -> (&str, Stamp) {
    let Some(at) = time.rfind(['+', '-']) else {
        return (time, 0);
    };
    let (clock, sign_and_offset) = time.split_at(at);
    let sign: Stamp = if sign_and_offset.starts_with('-') {
        -1
    } else {
        1
    };
    let digits = &sign_and_offset[1..];
    let (hours, minutes) = match digits.split_once(':') {
        Some((hours, minutes)) => (hours, minutes),
        None if digits.len() == 4 => digits.split_at(2),
        // Not an offset this build knows. Left where it is so the clock parser
        // refuses the whole stamp rather than silently reading it as UTC.
        None => return (time, 0),
    };
    match (hours.parse::<Stamp>(), minutes.parse::<Stamp>()) {
        (Ok(hours), Ok(minutes)) if hours <= 23 && minutes <= 59 => {
            (clock, sign * (hours * 3_600 + minutes * 60))
        }
        _ => (time, 0),
    }
}

/// The stamp to compare **against a cursor**, with an unreadable one first.
///
/// Ordering first is right here and only here: this answers *is this comment
/// after the one we last read*, and an unreadable one answering "no" leaves the
/// run looking silent, which expires its claim. That fails closed.
///
/// It is the wrong answer for deciding **who won a race** — see [`stamp_rank`],
/// which is what the winner sorts use, and which this used to be.
pub fn stamp_order(value: &str) -> Stamp {
    parse_stamp(value).unwrap_or(UNKNOWN)
}

/// `YYYYMMDD` written the long way, and anything else untouched.
fn expand_date(date: &str) -> String {
    if date.len() == 8 && date.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..])
    } else {
        date.to_owned()
    }
}

/// `HHMM` and `HHMMSS` written the long way, fraction and all kept as they came.
fn expand_time(time: &str) -> String {
    let (digits, rest) = time.split_at(
        time.bytes()
            .position(|byte| !byte.is_ascii_digit())
            .unwrap_or(time.len()),
    );
    match digits.len() {
        4 => format!("{}:{}{rest}", &digits[..2], &digits[2..]),
        6 => format!("{}:{}:{}{rest}", &digits[..2], &digits[2..4], &digits[4..]),
        _ => time.to_owned(),
    }
}

/// The stamp to **rank** by, with an unreadable one ordering last.
///
/// The earliest live claim wins, and `live.first()` is the winner. So a stamp
/// nobody can read used to win: [`stamp_order`] puts it first, and the sorts
/// that pick a holder used that.
///
/// One constant was serving two uses that want opposite answers. Against a
/// cursor, unreadable-as-oldest fails closed. In a race, unreadable-as-earliest
/// is a **queue jump**: a run holds a claim it did not prove it got to first,
/// and the run that genuinely did is refused.
///
/// It was reachable rather than theoretical. `2026-08-04T18:00:00+02:00` is
/// ISO-8601, and this parser read no offset at all until it was taught to — so
/// any tracker or proxy answering in local time handed the race to whoever
/// happened to be answered that way.
pub fn stamp_rank(value: &str) -> Stamp {
    parse_stamp(value).unwrap_or(Stamp::MAX)
}

/// When a claim stops being live.
///
/// Two clocks, and the rule is *not* "whichever is later". A horizon that has
/// already passed at the moment of acquisition is taken as written — a run that
/// declares a deadline behind it has declared it, and silence cannot extend
/// what was never granted. Otherwise a run that has spoken since acquiring
/// carries its horizon **or** its activity window, whichever reaches further.
pub fn ownership_deadline(
    horizon: Option<Stamp>,
    acquired_at: Option<Stamp>,
    spoke_at: Option<Stamp>,
) -> Option<Stamp> {
    let activity = spoke_at.map(|at| at + SILENCE);
    if let (Some(horizon), Some(acquired)) = (horizon, acquired_at) {
        if horizon <= acquired {
            return Some(horizon);
        }
        if let Some(spoke) = spoke_at
            && spoke > acquired
            && let Some(activity) = activity
        {
            return Some(horizon.max(activity));
        }
    }
    horizon.or(activity)
}

/// Whether an acquisition marker carries everything its own shape requires.
///
/// A marker with none of the operation attributes is a **legacy** one and passes
/// — the timeline holds markers written before operations existed, and rejecting
/// them would erase ownership somebody genuinely holds. One that carries *any*
/// of them is claiming the newer contract and must satisfy all of it.
pub fn valid_acquisition_marker(mark: &super::markers::Marker) -> bool {
    let scoped = ["op-id", "from-op", "target-op", "evidence-hash"]
        .iter()
        .any(|key| mark.contains_key(*key));
    if !scoped {
        return true;
    }
    let operation = mark.get("op-id").map(String::as_str).unwrap_or_default();
    let horizon = mark.get("horizon").map(String::as_str).unwrap_or_default();
    let kind = mark.get("kind").map(String::as_str).unwrap_or_default();

    is_operation_id(operation)
        && mark.get("runtime").is_some_and(|value| !value.is_empty())
        && is_horizon(horizon)
        && parse_stamp(horizon).is_some()
        && (kind == "claim"
            || (mark.get("from").is_some_and(|value| !value.is_empty())
                && mark.get("from-op").is_some_and(|value| !value.is_empty())))
}

/// Thirty-two lowercase hex characters, exactly.
pub fn is_operation_id(value: &str) -> bool {
    value.len() == 32
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// `YYYY-MM-DDTHH:MM[:SS]Z`, exactly.
///
/// The transport spells it once, as `HORIZON_RE`, and both places that consult
/// it consult that one constant. This crate had two: this, and a byte-identical
/// `is_horizon_shape` in `claim`, with nothing crossing them — one deciding
/// whether an incoming `--horizon` is well formed, the other whether a claim
/// marker on the timeline is. Two answers to one question, which is the shape
/// this crate has already paid for once in `same_directory`. They agreed on the
/// day they were found; nothing was making them.
pub fn is_horizon(value: &str) -> bool {
    let shape = |text: &str, pattern: &str| {
        text.len() == pattern.len()
            && text.chars().zip(pattern.chars()).all(|(c, p)| match p {
                'd' => c.is_ascii_digit(),
                other => c == other,
            })
    };
    shape(value, "dddd-dd-ddTdd:ddZ") || shape(value, "dddd-dd-ddTdd:dd:ddZ")
}

/// How many days that month really has.
///
/// The proleptic Gregorian rule, which is the one `datetime` applies: every
/// fourth year, except centuries, except every fourth century.
fn days_in(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days from 1970-01-01 to this civil date. Howard Hinnant's algorithm.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The run-id a comment's **prose** claims for, when it claims at all.
///
/// Both halves are required, never one: the phrase must **open** the comment and
/// the horizon clause must be present somewhere in it. A mention like "already
/// claimed by @someone months ago" satisfies neither, and must not be able to
/// unseat a real claim.
///
/// Leading markdown decoration is skipped, because a claim written as a bullet
/// or a quote is still a claim: `*`, `_`, `>`, `#` and whitespace.
pub fn claim_prose(body: &str) -> Option<String> {
    let opened =
        body.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '*' | '_' | '>' | '#'));
    const PHRASE: &str = "claimed by";
    if !opened
        .get(..PHRASE.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(PHRASE))
    {
        return None;
    }
    let rest = &opened[PHRASE.len()..];
    let spaces = rest.len() - rest.trim_start().len();
    if spaces == 0 {
        return None;
    }
    let run: String = rest[spaces..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .collect();
    if run.is_empty() || !mentions_horizon(body) {
        return None;
    }
    Some(run)
}

/// Whether the body carries the horizon clause a prose claim needs.
///
/// The transport spells this `expect(?:s|ing)?\s+to\s+report\s+by`. This used to
/// split the body into whitespace-separated words and compare four of them
/// exactly — a different grammar wearing the same name, because any decoration
/// touching a word hid the clause from this side and from no other. `Claimed by
/// <run>. *Expects to report by* <time>` was a claim the transport read and this
/// did not.
///
/// The direction is what makes converging worth more than arguing. This reads
/// *somebody else's* claim, and the module that spells it says so: its fallback
/// exists for a claim whose marker was lost or that another binding wrote. A
/// clause missed here is an issue read as free while another run holds it.
///
/// So this matches where the regex matches, including inside `unexpects` and in
/// front of `byte`, which have no word boundaries to stop them. Being stricter
/// than the reference implementation is still disagreeing with it, and the
/// disagreement is the defect.
fn mentions_horizon(body: &str) -> bool {
    let lowered = body.to_lowercase();
    let mut searched = 0;
    while let Some(at) = lowered[searched..].find("expect") {
        let opened = searched + at + "expect".len();
        searched += at + 1;
        let rest = &lowered[opened..];
        // `(?:s|ing)?`. Neither branch can lose to the empty one: both leave a
        // letter where the `\s+` below demands a space, so there is nothing for
        // the regex's backtracking to find that this misses.
        let rest = rest
            .strip_prefix('s')
            .or_else(|| rest.strip_prefix("ing"))
            .unwrap_or(rest);
        if after_spacing(rest, "to")
            .and_then(|rest| after_spacing(rest, "report"))
            .and_then(|rest| after_spacing(rest, "by"))
            .is_some()
        {
            return true;
        }
    }
    false
}

/// `text` with a run of whitespace and then `word` taken off the front.
///
/// `None` when either is missing: the transport writes `\s+` and not `\s*`.
fn after_spacing<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let trimmed = text.trim_start();
    if trimmed.len() == text.len() {
        return None;
    }
    trimmed.strip_prefix(word)
}

/// The newest event this run acquired before `position`.
///
/// Binary search over a per-run history that is already in position order, which
/// is what keeps the reducer from being quadratic on a long timeline.
pub fn latest_event_before<T>(
    history: &[T],
    position: usize,
    position_of: impl Fn(&T) -> usize,
) -> Option<&T> {
    let mut low = 0;
    let mut high = history.len();
    while low < high {
        let middle = (low + high) / 2;
        if position_of(&history[middle]) < position {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    (low > 0).then(|| &history[low - 1])
}

/// A SHA-256 hex digest.
///
/// The transport hashes in seven places and three of them are identities the
/// other side compares — an ownership epoch, a forced-reclaim evidence digest, a
/// delivery-target manifest. They have to come out **byte for byte** the same,
/// which is why this is a dependency rather than something approximated.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The epoch an acquisition belongs to.
///
/// An operation id when there is one. Otherwise the comment's own id — and when
/// even that is missing, a digest over its timestamp and body, truncated to the
/// same width. The fallback exists so a timeline written before operations
/// existed still has stable epochs: without one, every read would invent a new
/// identity for the same event and no release could ever match its target.
pub fn ownership_epoch(
    operation_id: Option<&str>,
    comment_id: Option<&str>,
    created_at: &str,
    body: &str,
) -> String {
    if let Some(operation) = operation_id.filter(|value| !value.is_empty()) {
        return operation.to_owned();
    }
    let identity = match comment_id.filter(|value| !value.is_empty()) {
        Some(id) => id.to_owned(),
        None => sha256_hex(
            format!(
                "{created_at}
{body}"
            )
            .as_bytes(),
        )
        .chars()
        .take(32)
        .collect(),
    };
    format!("legacy-{identity}")
}

/// The marker kinds an operation id may reserve, and the attributes each carries.
pub const OPERATION_FIELDS: &[(&str, &[&str])] = &[
    ("claim", &["run-id", "runtime", "horizon"]),
    (
        "reclaim",
        &[
            "run-id",
            "runtime",
            "horizon",
            "from",
            "from-op",
            "forced",
            "evidence-hash",
        ],
    ),
    ("standdown", &["run-id", "target-op"]),
    ("unassign", &["run-id", "runtime", "target-op"]),
    (
        "review-handoff",
        &[
            "run-id",
            "target-op",
            "epoch",
            "pr",
            "head",
            "base",
            "digest",
            "authority",
            "requested-at",
            "deadline",
            "blocker",
            "discharger",
        ],
    ),
    (
        "review-verdict",
        &[
            "run-id", "reviewer", "epoch", "pr", "head", "base", "digest", "outcome",
        ],
    ),
    // The finding's **identity**, and deliberately not its body. `evidence` and
    // `impact` are what the finding says; `id` and `class` with the receipt are
    // what it *is*, and they are what the operation id is minted from. So two
    // recordings of one finding whose wording differs are one operation and the
    // first body stands, while a different `id` or a different `class` is a
    // different finding rather than an edit of this one.
    //
    // Nothing is lost by leaving the body out: `first_operation_markers` already
    // refuses an edited comment, so the words cannot be changed after the fact
    // either way.
    (
        "review-finding",
        &[
            "run-id", "reviewer", "epoch", "pr", "head", "base", "digest", "id", "class",
        ],
    ),
];

/// Whether this kind is one an operation id may reserve.
pub fn is_operation_kind(kind: &str) -> bool {
    OPERATION_FIELDS.iter().any(|(name, _)| *name == kind)
}

/// One comment, as much of it as the reducer reads.
///
/// `Default` is written out below rather than derived: `bool::default()` is
/// `false`, and for `includes_created_edit` **`false` is the trusted value**.
/// A derived default would hand out a comment claiming it had never been
/// edited — the opposite of what a comment nobody has described should say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The tracker's own id, when it gave one.
    pub id: Option<String>,
    /// When the tracker says it was written.
    pub created_at: String,
    /// Its text.
    pub body: String,
    /// Whether **this** identity wrote it. Anything else is untrusted.
    pub viewer_did_author: bool,
    /// Whether it was edited after creation. `false` is the only trusted value:
    /// an edited comment can be made to say anything after the fact.
    pub includes_created_edit: bool,
}

impl Default for Comment {
    /// Both trust flags at their untrusted value, matching `claim::comment_of`:
    /// a comment nobody described is not a comment that vouched for itself.
    fn default() -> Self {
        Self {
            id: None,
            created_at: String::new(),
            body: String::new(),
            viewer_did_author: false,
            includes_created_edit: true,
        }
    }
}

/// Every acquisition the timeline actually stands on.
///
/// `ownership_events` minus the reclaims that never proved themselves, which is
/// what "authoritative" means here: a reclaim whose control check did not pass
/// is on the issue and decides nothing. `holding` already reduces this down to
/// a winner; this is the list before that, and a release needs it because the
/// epoch it ends is usually no longer live by the time it is asked about.
pub fn authoritative_events(comments: &[Comment]) -> Vec<Event> {
    let events = ownership_events(comments);
    let valid = valid_reclaim_positions(&events, comments);
    events
        .into_iter()
        .filter(|event| {
            event.kind != "reclaim" || valid.contains(&(event.position, event.marker_index))
        })
        .collect()
}

/// Where each operation id was first reserved.
///
/// Returns `(position, marker index, the marker, whether the comment is
/// unedited)`. **First wins**: a later correction cannot rewrite an operation's
/// meaning, because the whole point of an operation id is that retrying names
/// the same event rather than a new one.
pub fn first_operation_markers(
    comments: &[Comment],
) -> std::collections::BTreeMap<String, (usize, usize, super::markers::Marker, bool)> {
    let mut first = std::collections::BTreeMap::new();
    for (position, comment) in comments.iter().enumerate() {
        if !comment.viewer_did_author {
            continue;
        }
        for (index, mark) in super::markers::parse(&comment.body).into_iter().enumerate() {
            let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
            let operation = mark.get("op-id").map(String::as_str).unwrap_or_default();
            if is_operation_kind(kind) && is_operation_id(operation) {
                first.entry(operation.to_owned()).or_insert((
                    position,
                    index,
                    mark,
                    !comment.includes_created_edit,
                ));
            }
        }
    }
    first
}

/// The newest trusted liveness marker this run wrote after `after`.
///
/// Mentions and targets do **not** count: a marker naming a run is not that run
/// speaking, and reading it as activity would keep a dead claim alive on the
/// strength of somebody else writing about it.
pub fn last_activity_by(comments: &[Comment], run_id: &str, after: &str) -> String {
    comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .filter(|comment| stamp_order(&comment.created_at) > stamp_order(after))
        .filter(|comment| {
            super::markers::parse(&comment.body).iter().any(|mark| {
                mark.get("run-id").map(String::as_str) == Some(run_id)
                    && mark
                        .get("kind")
                        .is_some_and(|kind| super::markers::ACTIVITY_KINDS.contains(&kind.as_str()))
            })
        })
        .map(|comment| comment.created_at.clone())
        .max()
        .unwrap_or_default()
}

/// The activation boundary for evidence on a forced reclaim.
///
/// The last shipped evidence-free forced event was before this. After it, a
/// stale client cannot mint a new "legacy" event by merely omitting the
/// attribute — which is what an activation boundary is for.
pub const FORCED_EVIDENCE_SINCE: &str = "2026-07-27T19:00:00Z";

/// The heading a forced reclaim writes its evidence under.
///
/// Persisted protocol syntax, not presentation copy: changing it needs a parser
/// migration that keeps already-written forced events valid.
pub const FORCED_EVIDENCE_HEADING: &str = "Forced takeover reason and evidence:";

/// The digest over the evidence a forced reclaim wrote, when it wrote any.
///
/// The evidence is the text **between the heading and the marker**, and it must
/// contain no marker of its own — otherwise a body could quote a marker into its
/// own evidence and change what the digest covers without changing what a reader
/// sees.
pub fn forced_evidence_digest(body: &str, marker_index: Option<usize>) -> Option<String> {
    let marker_at = marker_index.and_then(|index| marker_offsets(body).get(index).copied())?;
    let heading_at = body.find(FORCED_EVIDENCE_HEADING)?;
    let from = heading_at + FORCED_EVIDENCE_HEADING.len();
    if from > marker_at {
        return None;
    }
    let evidence = body.get(from..marker_at)?.trim();
    if evidence.is_empty() || !super::markers::parse(evidence).is_empty() {
        return None;
    }
    Some(sha256_hex(evidence.as_bytes()))
}

/// Where each marker begins in a body, in order.
fn marker_offsets(body: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(next) = body[at..].find("<!--") {
        let start = at + next;
        let after = &body[start + 4..];
        let Some(close) = after.find("-->") else {
            break;
        };
        if super::markers::opens_a_marker(after) {
            found.push(start);
        }
        at = start + 4 + close + 3;
    }
    found
}

/// One acquisition on the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// When the tracker says it was written.
    pub created_at: String,
    /// Its index in the comment list, which breaks ties the clock cannot.
    pub position: usize,
    /// Who acquired.
    pub run_id: String,
    /// The runtime they declared, when they declared one.
    pub runtime: Option<String>,
    /// The horizon they declared.
    pub horizon: Option<String>,
    /// `claim` or `reclaim`.
    pub kind: String,
    /// Who a reclaim displaced.
    pub from: Option<String>,
    /// The operation this belongs to, when it is a modern one.
    pub operation_id: Option<String>,
    /// The epoch a reclaim displaced.
    pub from_operation: Option<String>,
    /// The evidence hash the marker declared.
    pub evidence_hash: Option<String>,
    /// The digest over the evidence actually written under the heading.
    pub evidence_body_hash: Option<String>,
    /// Whether the acquisition declared itself forced.
    pub forced: bool,
    /// Whether this forced event has to carry evidence to count.
    pub evidence_required: bool,
    /// Which marker in the comment this came from.
    pub marker_index: Option<usize>,
    /// The comment it was read from.
    pub comment: Comment,
}

/// Every acquisition on the timeline, oldest first.
///
/// Parsed **once**, so every command that reads ownership reads the same
/// timeline. Three rules decide what counts, and each one is a way the timeline
/// could otherwise be rewritten by somebody who should not be able to:
///
/// 1. Only comments **this identity wrote and never edited** are read at all. An
///    edited comment can be made to say anything after the fact.
/// 2. A marker carrying an operation id counts only at the position that
///    **reserved** it, and only if that comment is unedited. Otherwise a second
///    copy of one operation would be a second acquisition.
/// 3. A prose claim counts only when the comment carries **no** acquisition
///    marker at all. A body with both is speaking the marker's language, and the
///    marker is the exact half.
pub fn ownership_events(comments: &[Comment]) -> Vec<Event> {
    let first_operations = first_operation_markers(comments);
    let mut events: Vec<Event> = Vec::new();

    for (position, comment) in comments.iter().enumerate() {
        if !comment.viewer_did_author || comment.includes_created_edit {
            continue;
        }
        let parsed = super::markers::parse(&comment.body);
        let is_acquisition = |mark: &super::markers::Marker| {
            matches!(
                mark.get("kind").map(String::as_str),
                Some("claim" | "reclaim")
            )
        };
        let mut selected = parsed.iter().enumerate().find(|(_, mark)| {
            is_acquisition(mark)
                && mark.get("run-id").is_some_and(|run| !run.is_empty())
                && valid_acquisition_marker(mark)
        });

        // An operation counts only where it was reserved, and only unedited.
        if let Some((index, mark)) = selected
            && let Some(operation) = mark.get("op-id").filter(|value| !value.is_empty())
            && let Some((at, marker_at, _, unedited)) = first_operations.get(operation)
            && (!unedited || (position, index) != (*at, *marker_at))
        {
            selected = None;
        }

        let (marker_index, mark) = match selected {
            Some((index, mark)) => (Some(index), Some(mark.clone())),
            // A prose claim, and only when no acquisition marker is present:
            // a body carrying both is speaking the marker's language.
            None if !parsed.iter().any(is_acquisition) => {
                match claim_prose(&comment.body).filter(|_| !comment.includes_created_edit) {
                    Some(run) => {
                        let mut mark = super::markers::Marker::new();
                        mark.insert("kind".to_owned(), "claim".to_owned());
                        mark.insert("run-id".to_owned(), run);
                        (None, Some(mark))
                    }
                    None => (None, None),
                }
            }
            None => (None, None),
        };
        let Some(mark) = mark else { continue };

        let text = |key: &str| mark.get(key).filter(|v| !v.is_empty()).cloned();
        let forced = mark.get("forced").map(String::as_str) == Some("true");
        let created = parse_stamp(&comment.created_at);
        let cutover = parse_stamp(FORCED_EVIDENCE_SINCE);
        events.push(Event {
            created_at: comment.created_at.clone(),
            position,
            run_id: text("run-id").unwrap_or_default(),
            runtime: text("runtime"),
            horizon: text("horizon"),
            kind: text("kind").unwrap_or_else(|| "claim".to_owned()),
            from: text("from"),
            operation_id: text("op-id"),
            from_operation: text("from-op"),
            evidence_hash: text("evidence-hash"),
            evidence_body_hash: forced
                .then(|| forced_evidence_digest(&comment.body, marker_index))
                .flatten(),
            forced,
            // After the activation boundary a forced event must carry evidence.
            // An unreadable timestamp counts as after it: a stale client must not
            // be able to mint a new "legacy" event by writing a date nobody can
            // parse.
            evidence_required: forced
                && (mark.get("evidence").map(String::as_str) == Some("required")
                    || created.is_none()
                    || cutover.is_some_and(|cutover| created.is_some_and(|at| at >= cutover))
                    || comment.includes_created_edit),
            marker_index,
            comment: comment.clone(),
        });
    }

    // The clock first, then the position — because two comments can share a
    // timestamp and the server's order is the only tie-break there is.
    events.sort_by(|left, right| {
        stamp_rank(&left.created_at)
            .cmp(&stamp_rank(&right.created_at))
            .then(left.position.cmp(&right.position))
    });
    events
}

/// Which attributes of a release marker name the run it releases.
///
/// A release names its target in one of two ways, and the difference is not
/// cosmetic. `run-id` on a stand-down is the run being told to stop; `target` is
/// the run somebody else is releasing. A `reclaim` names only who it displaces,
/// because the run it names as `run-id` is the one *taking over*.
///
/// Reading the wrong attribute would release the wrong run — and the run that
/// kept working would be the one everybody believed had stood down.
pub const RELEASE_ATTRS_BY_KIND: &[(&str, &[&str])] = &[
    ("standdown", &["run-id", "target"]),
    ("release", &["run-id", "target"]),
    ("unassign", &["run-id", "target"]),
    ("reclaim", &["from"]),
];

/// The runs a marker releases, if it releases any.
pub fn released_by(mark: &super::markers::Marker) -> Vec<String> {
    let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
    RELEASE_ATTRS_BY_KIND
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, attributes)| {
            attributes
                .iter()
                .filter_map(|attribute| mark.get(*attribute))
                .filter(|value| !value.is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The last position at which each run was released by a **legacy** marker.
///
/// Legacy only: a marker carrying any operation attribute is scoped to one
/// epoch and is folded elsewhere. A broad legacy release cannot be allowed to
/// end an acquisition that came after it, which is why the position is kept
/// rather than a flag.
pub fn legacy_release_positions(
    comments: &[Comment],
    events: &[Event],
    valid_reclaims: &std::collections::BTreeSet<(usize, Option<usize>)>,
) -> std::collections::BTreeMap<String, usize> {
    // Who each run last acquired as, before any given point. A broad control
    // names a *run*, not an epoch, so this is what decides whether it is still
    // allowed to speak for that run at all.
    let mut history: std::collections::BTreeMap<&str, Vec<&Event>> = Default::default();
    for event in events {
        history.entry(&event.run_id).or_default().push(event);
    }
    let mut latest: std::collections::BTreeMap<String, usize> = Default::default();
    for (position, comment) in comments.iter().enumerate() {
        if !comment.viewer_did_author || comment.includes_created_edit {
            continue;
        }
        for (index, mark) in super::markers::parse(&comment.body).into_iter().enumerate() {
            let scoped = ["op-id", "target-op", "from-op"]
                .iter()
                .any(|key| mark.contains_key(*key));
            if scoped {
                continue;
            }
            // A reclaim releases whoever it displaces — but only if the reclaim
            // itself stands. Counting a **refused** takeover's release was the
            // defect the differential caught: two invalid reclaims naming the
            // holder released it anyway, so the valid third one found nothing
            // left to displace and was refused in turn.
            if mark.get("kind").map(String::as_str) == Some("reclaim")
                && !valid_reclaims.contains(&(position, Some(index)))
            {
                continue;
            }
            for target in released_by(&mark) {
                // A broad control stays compatible only while the run it names
                // is still on a legacy epoch. Past that, an old unscoped
                // stand-down — a delayed retry from an earlier binding is the
                // ordinary way one arrives — would end an acquisition made
                // under an operation id that it cannot possibly have known
                // about, and this dropped a live claim on exactly that. The
                // issue then read as unheld while the run went on working,
                // which is how two runs come to hold one issue.
                let modern = history.get(target.as_str()).is_some_and(|events| {
                    latest_event_before(events, position, |event| event.position)
                        .is_some_and(|event| event.operation_id.is_some())
                });
                if modern {
                    continue;
                }
                let entry = latest.entry(target).or_insert(position);
                *entry = (*entry).max(position);
            }
        }
    }
    latest
}

/// Who holds the issue, and the event that established it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    /// The live winner, if any.
    pub holder: Option<String>,
    /// The event that established it.
    pub event: Option<Event>,
    /// Every live acquisition, oldest first.
    pub live: Vec<Event>,
    /// Every acquisition whose deadline has passed.
    pub stale: Vec<Event>,
}

/// Where each **operation-scoped** release ends the epoch it names.
///
/// The other half of [`legacy_release_positions`], and the half nothing
/// computed. That function skips every marker carrying `op-id`, `target-op` or
/// `from-op` — correctly, because a scoped release is checked rather than
/// believed — and both callers of [`reduce_ownership`] then handed it an empty
/// map for the scoped ones. The parameter was typed, documented and consumed,
/// and always empty.
///
/// What that cost is the whole of `unassign`: this crate writes a release
/// carrying `target-op`, and its own reducer never saw it. A run that released
/// an issue went on reading as its holder — while the transport, given the same
/// two comments, reported no holder at all.
///
/// A control counts only where its operation id was **first** reserved, only
/// from an unedited comment, and only when the epoch it names is really an
/// acquisition it may end. Anything else makes the operation a conflict, which
/// releases nothing: a release nobody can verify must not take an issue away
/// from the run holding it.
pub fn epoch_release_positions(
    comments: &[Comment],
    events: &[Event],
    valid_reclaims: &std::collections::BTreeSet<(usize, Option<usize>)>,
) -> std::collections::BTreeMap<String, usize> {
    let first_operations = first_operation_markers(comments);
    let epoch_of = |event: &Event| {
        ownership_epoch(
            event.operation_id.as_deref(),
            event.comment.id.as_deref(),
            &event.comment.created_at,
            &event.comment.body,
        )
    };
    let by_epoch: std::collections::BTreeMap<String, &Event> = events
        .iter()
        .map(|event| (epoch_of(event), event))
        .collect();
    // Handed in rather than derived here: this used to build the set from
    // "every event that is a reclaim", which is a different question — it
    // called every takeover valid, including the ones that never proved
    // themselves.
    let mut controls: std::collections::BTreeMap<String, (String, usize)> = Default::default();
    let mut conflicts: std::collections::BTreeSet<String> = Default::default();
    let mut companions: std::collections::BTreeSet<String> = Default::default();

    for (position, comment) in comments.iter().enumerate() {
        if !comment.viewer_did_author {
            continue;
        }
        for (marker_index, mark) in super::markers::parse(&comment.body).into_iter().enumerate() {
            let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
            if !RELEASE_ATTRS_BY_KIND.iter().any(|(name, _)| *name == kind)
                || !is_operation_kind(kind)
            {
                continue;
            }
            let Some(operation) = mark.get("op-id").filter(|value| is_operation_id(value)) else {
                continue;
            };
            let target_epoch = if kind == "reclaim" {
                mark.get("from-op")
            } else {
                mark.get("target-op")
            }
            .filter(|value| !value.is_empty());
            let Some((at, marker_at, first_mark, unedited)) = first_operations.get(operation)
            else {
                continue;
            };

            // A stand-down written in the same comment as the claim it belongs
            // to rides along with it, so it does not have to be the marker the
            // operation was reserved at.
            let claim_companion =
                kind == "standdown" && first_mark.get("kind").map(String::as_str) == Some("claim");
            if claim_companion && !companions.insert(operation.clone()) {
                continue;
            }
            let companion = claim_companion
                && *unedited
                && target_epoch.map(String::as_str) == Some(operation.as_str())
                && !comment.includes_created_edit;
            if (position, Some(marker_index)) != (*at, Some(*marker_at)) && !companion {
                continue;
            }
            if controls.contains_key(operation) || conflicts.contains(operation) {
                continue;
            }

            let target = target_epoch.and_then(|epoch| by_epoch.get(epoch.as_str()));
            let refused = !unedited
                || (kind == "reclaim" && !valid_reclaims.contains(&(position, Some(marker_index))))
                || target_epoch.is_none()
                || (matches!(kind, "standdown" | "unassign")
                    && match target {
                        None => true,
                        Some(target) => {
                            Some(&target.run_id) != mark.get("run-id")
                                || (kind == "standdown"
                                    && target_epoch.map(String::as_str) != Some(operation.as_str()))
                                || (kind == "unassign"
                                    && (mark
                                        .get("runtime")
                                        .is_none_or(|runtime| runtime.is_empty())
                                        || target.runtime.as_ref().is_some_and(|recorded| {
                                            Some(recorded) != mark.get("runtime")
                                        })))
                        }
                    });
            if refused {
                conflicts.insert(operation.clone());
            } else if let Some(epoch) = target_epoch {
                controls.insert(operation.clone(), (epoch.clone(), position));
            }
        }
    }

    let mut released: std::collections::BTreeMap<String, usize> = Default::default();
    for (epoch, position) in controls.into_values() {
        let entry = released.entry(epoch).or_insert(position);
        *entry = (*entry).max(position);
    }
    released
}

/// The fold that decides the live winner.
///
/// **Pure and fed**, the same way `classify_base_movement` is: the release
/// positions come in as arguments rather than being scanned here. The states it
/// separates are races no test can reproduce by calling a tracker, so the only
/// way to check it is to hand it a timeline and compare.
///
/// Three rules, and each one is a way ownership could otherwise be resurrected:
///
/// 1. **A reclaim starts a new epoch.** Only acquisitions from the last takeover
///    onward are candidates — losing contenders from before it must not come
///    back when the reclaimed run releases.
/// 2. **Within an epoch, each run's newest acquisition is its proof**, unless an
///    identical repeat straddles a release: then the later one is a genuine
///    re-acquisition rather than a retry of the same event.
/// 3. **The earliest live run wins.** A normal claim race is settled by who got
///    there first, not by who spoke last.
pub fn reduce_ownership(
    events: &[Event],
    legacy_releases: &std::collections::BTreeMap<String, usize>,
    epoch_releases: &std::collections::BTreeMap<String, usize>,
    comments: &[Comment],
    now: &str,
) -> Holding {
    // Everything before the last takeover belongs to a closed epoch.
    let from = events
        .iter()
        .rposition(|event| event.kind == "reclaim")
        .unwrap_or(0);
    let candidates = &events[from..];

    let mut latest: Vec<Event> = Vec::new();
    for event in candidates {
        let previous = latest.iter().position(|held| held.run_id == event.run_id);
        if let Some(index) = previous {
            let earlier = &latest[index];
            let released_between = legacy_releases
                .get(&event.run_id)
                .is_some_and(|at| earlier.position < *at && *at < event.position);
            let same = earlier.operation_id == event.operation_id
                && earlier.kind == event.kind
                && earlier.runtime == event.runtime
                && earlier.horizon == event.horizon
                && earlier.from == event.from
                && earlier.forced == event.forced;
            // An identical repeat is a retry of one event — unless a release
            // sits between them, which makes the later one a re-acquisition.
            if same && (event.operation_id.is_some() || !released_between) {
                continue;
            }
            latest[index] = event.clone();
        } else {
            latest.push(event.clone());
        }
    }

    // An acquisition that a release already ended is not held at all.
    latest.retain(|event| {
        let legacy = legacy_releases.get(&event.run_id).copied();
        let epoch = epoch_releases
            .get(&ownership_epoch(
                event.operation_id.as_deref(),
                event.comment.id.as_deref(),
                &event.comment.created_at,
                &event.comment.body,
            ))
            .copied();
        let ended_at = legacy.into_iter().chain(epoch).max();
        ended_at.is_none_or(|at| event.position > at)
    });

    latest.sort_by(|left, right| {
        stamp_rank(&left.created_at)
            .cmp(&stamp_rank(&right.created_at))
            .then(left.position.cmp(&right.position))
    });

    let moment = parse_stamp(now);
    let (mut live, mut stale) = (Vec::new(), Vec::new());
    for event in latest {
        let spoke = last_activity_by(comments, &event.run_id, &event.created_at);
        let deadline = ownership_deadline(
            event.horizon.as_deref().and_then(parse_stamp),
            parse_stamp(&event.created_at),
            parse_stamp(&spoke).or_else(|| parse_stamp(&event.created_at)),
        );
        let expired = matches!((moment, deadline), (Some(now), Some(deadline)) if deadline < now);
        if expired {
            stale.push(event)
        } else {
            live.push(event)
        }
    }

    // The earliest live run wins: a claim race is settled by who got there
    // first, not by who spoke last.
    let winner = live.first().cloned();
    Holding {
        holder: winner.as_ref().map(|event| event.run_id.clone()),
        event: winner,
        live,
        stale,
    }
}

/// Whether the ownership-relevant comments arrive in timestamp order.
///
/// The tracker usually returns them that way, and when it does, an accepted
/// takeover can drop every earlier candidate: nothing before it can resurrect.
/// When it does **not**, that shortcut is unsafe — timestamp order can move the
/// boundary — so the full prefix is kept instead.
///
/// Only the comments that carry ownership are looked at. A note posted out of
/// order says nothing about whether the claims did.
pub fn comments_are_chronological(comments: &[Comment]) -> bool {
    let relevant: Vec<&Comment> = comments
        .iter()
        .filter(|comment| comment.viewer_did_author)
        .filter(|comment| {
            let marks = super::markers::parse(&comment.body);
            marks.iter().any(|mark| {
                mark.get("kind").is_some_and(|kind| {
                    kind == "claim"
                        || super::markers::RELEASE_KINDS.contains(&kind.as_str())
                        || super::markers::ACTIVITY_KINDS.contains(&kind.as_str())
                })
            }) || claim_prose(&comment.body).is_some()
        })
        .collect();
    relevant
        .windows(2)
        .all(|pair| stamp_rank(&pair[0].created_at) <= stamp_rank(&pair[1].created_at))
}

/// The hash that binds a forced reclaim's evidence to the event it justifies.
///
/// Every field the takeover asserted goes in, so the digest cannot be lifted
/// from one forced reclaim and pasted onto another: change the run, the horizon,
/// or who is displaced, and the binding no longer matches. The evidence alone
/// would be transferable, and transferable evidence is no evidence.
///
/// The field order and the compact JSON separators are the wire format. Both
/// sides have to serialise identically or every forced takeover is rejected.
/// Every field that can be **absent** is `Option`, and that is the whole of what
/// this signature says. The transport builds the same list out of a dictionary,
/// so a marker with no `runtime` puts `null` in the JSON it hashes; these three
/// took `&str` and an absent one arrived as `""`. Measured on the pair, with
/// `runtime`, `horizon` and `from` absent: `bcb711ad…` there and `4e09f435…`
/// here — the same forced reclaim, two digests, so whichever side did not write
/// the marker reads the evidence as unbound and refuses a takeover the other
/// performed.
///
/// The crossing had the shape and could not see it: its corpus posed no absence,
/// and it adapted the cases with `unwrap_or_default()`, which is the difference
/// itself written into the test.
pub fn forced_reclaim_hash(
    operation_id: Option<&str>,
    evidence: &str,
    run_id: &str,
    runtime: Option<&str>,
    horizon: Option<&str>,
    from_run: Option<&str>,
    from_operation: Option<&str>,
) -> String {
    let fields = serde_json::json!([
        "forced-reclaim-v1",
        operation_id,
        true,
        evidence,
        run_id,
        runtime,
        horizon,
        from_run,
        from_operation,
    ]);
    sha256_hex(ensure_ascii(&fields.to_string()).as_bytes())
}

/// JSON with every non-ASCII character written as an escape, as `json.dumps`
/// writes it.
///
/// `ensure_ascii=True` is that function's default and `serde_json` has no such
/// mode, so the same fields hashed to different digests the moment one of them
/// carried a character outside ASCII: `claude-sí` is `claude-s\u00ed` there and
/// eight raw bytes here.
///
/// The digest is what binds a forced takeover to the event it was written for,
/// so two digests is a takeover one side accepts and the other refuses — of an
/// issue another run is holding. It is reachable through the timeline rather
/// than through this crate's own minting: `from-run` and `from-operation` are
/// read out of markers and percent-decoded, so they carry whatever was written.
///
/// Structural JSON is all ASCII, so escaping every non-ASCII character wherever
/// it falls only ever touches the inside of a string. Above the basic plane the
/// escape is a surrogate pair, which is what `json.dumps` emits.
fn ensure_ascii(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for character in json.chars() {
        if character.is_ascii() {
            out.push(character);
            continue;
        }
        let mut units = [0u16; 2];
        for unit in character.encode_utf16(&mut units) {
            out.push_str(&format!("\\u{unit:04x}"));
        }
    }
    out
}

/// Whether a reclaim's **own proof** holds, before anything else is consulted.
///
/// Two refusals, and both are about a takeover claiming more than it shows:
///
/// - a forced reclaim that must carry evidence has to carry a digest, and that
///   digest has to be **bound** to this event. Without the binding, evidence
///   written for one takeover would justify another.
/// - a modern reclaim must name the epoch it displaces. Displacing "whatever is
///   there" is how a takeover lands on a holder nobody meant to remove.
pub fn reclaim_proof_is_valid(event: &Event) -> bool {
    if event.forced && event.evidence_required {
        let Some(digest) = &event.evidence_body_hash else {
            return false;
        };
        let bound = forced_reclaim_hash(
            event.operation_id.as_deref(),
            digest,
            &event.run_id,
            event.runtime.as_deref(),
            event.horizon.as_deref(),
            event.from.as_deref(),
            event.from_operation.as_deref(),
        );
        let must_bind = event.operation_id.is_some() || event.comment.includes_created_edit;
        if must_bind && event.evidence_hash.as_deref() != Some(bound.as_str()) {
            return false;
        }
    }
    // A modern reclaim names what it displaces, or it names nothing at all.
    !(event.operation_id.is_some() && event.from_operation.is_none())
}

/// Which reclaims on this timeline are valid, resolved **bottom-up**.
///
/// This is where the recursion lives, and where it is broken. Validating a
/// takeover needs the ownership *as it stood just before it*, and reducing
/// ownership needs to know which takeovers are valid. Recursing would branch on
/// every deep history; instead the timeline is walked once, forward, carrying
/// the events accepted so far — so each validation reduces a prefix that is
/// already settled.
///
/// A **refused** takeover is not carried forward at all. That is not tidiness:
/// a reclaim releases whoever it displaces, so letting a refused one contribute
/// its release would end the holder anyway — and the differential caught exactly
/// that, with two invalid reclaims clearing the way for nobody and the valid
/// third one then finding nothing to displace.
///
/// When the comments are chronological, an accepted takeover **clears** the
/// earlier candidates: nothing before it can resurrect. When they are not, the
/// full prefix is kept, because timestamp order can move that boundary.
pub fn valid_reclaim_positions(
    events: &[Event],
    comments: &[Comment],
) -> std::collections::BTreeSet<(usize, Option<usize>)> {
    let mut valid: std::collections::BTreeSet<(usize, Option<usize>)> = Default::default();
    let mut prior: Vec<Event> = Vec::new();
    let chronological = comments_are_chronological(comments);

    let mut ordered: Vec<&Event> = events.iter().collect();
    ordered.sort_by_key(|event| (event.position, event.marker_index));

    for event in ordered {
        if event.kind == "reclaim" {
            let mut candidates = prior.clone();
            if !chronological {
                candidates.sort_by(|left, right| {
                    stamp_rank(&left.created_at)
                        .cmp(&stamp_rank(&right.created_at))
                        .then(left.position.cmp(&right.position))
                });
            }
            if !reclaim_is_valid(event, &candidates, comments, &valid) {
                continue;
            }
            valid.insert((event.position, event.marker_index));
            if chronological {
                prior.clear();
            }
        }
        prior.push(event.clone());
    }
    valid
}

/// Whether one takeover holds against the ownership that preceded it.
///
/// Four conditions, and each refuses a takeover claiming more than the timeline
/// supports:
///
/// - its **own proof** holds ([`reclaim_proof_is_valid`]);
/// - it names a target that actually existed at that moment;
/// - a modern takeover may not displace a legacy acquisition in silence;
/// - and it may take from a **live** holder only if it says it is forced. Taking
///   a live holder quietly is the one thing a reclaim must never do.
fn reclaim_is_valid(
    event: &Event,
    prior: &[Event],
    comments: &[Comment],
    valid_so_far: &std::collections::BTreeSet<(usize, Option<usize>)>,
) -> bool {
    if !reclaim_proof_is_valid(event) {
        return false;
    }
    // The prefix, and only the reclaims already accepted within it.
    let upto = &comments[..event.position.min(comments.len())];
    let legacy = legacy_release_positions(upto, prior, valid_so_far);
    let before = reduce_ownership(prior, &legacy, &Default::default(), upto, &event.created_at);

    let epoch_of = |candidate: &Event| {
        ownership_epoch(
            candidate.operation_id.as_deref(),
            candidate.comment.id.as_deref(),
            &candidate.comment.created_at,
            &candidate.comment.body,
        )
    };
    let target = match &event.from_operation {
        Some(wanted) => before
            .event
            .iter()
            .chain(&before.stale)
            .find(|candidate| epoch_of(candidate) == *wanted)
            .cloned(),
        None => before.event.clone().or_else(|| {
            before
                .stale
                .iter()
                .find(|candidate| Some(&candidate.run_id) == event.from.as_ref())
                .cloned()
        }),
    };
    let Some(target) = target else { return false };

    let holder_is_target = before
        .event
        .as_ref()
        .is_some_and(|held| held.position == target.position);
    // Parenthesised deliberately: `&&` binds tighter than `||`, so without the
    // group "no holder" would short-circuit to true and skip every condition
    // below — a takeover accepted without naming anyone or saying it is forced.
    (before.holder.is_none() || holder_is_target)
        && (event.operation_id.is_some() || target.operation_id.is_none())
        && Some(&target.run_id) == event.from.as_ref()
        && (before.holder.is_none() || event.forced)
}

/// Which attribute of a marker addresses the run it **instructs**.
///
/// Not the same table as the releases, and the difference is the whole reason
/// this exists separately. **A self-release is not an instruction.** Every
/// `standdown` this transport writes carries the *author's* own run-id — a claim
/// that lost a race, a deliberate unassign — so reading `run-id` as the
/// addressee would have every run stand itself down on its own message.
///
/// A `reclaim` names the displaced run in `from`. A stand-down aimed at somebody
/// else names them in `target`. And `adjudication`, which this transport never
/// writes, stays addressable every way: it arrives from a person or another
/// runtime, where `run-id` can only be the run being adjudicated against.
pub const INSTRUCTION_ATTRS_BY_KIND: &[(&str, &[&str])] = &[
    ("standdown", &["target"]),
    ("reclaim", &["target", "from"]),
    ("adjudication", &["run-id", "target", "from"]),
];

/// The kinds that can instruct a run to stop.
///
/// Read off the table rather than listed again. There used to be a flat
/// `CONTROL_KINDS` beside a flat `TARGET_ATTRS` in `markers`, which is the rule
/// this table replaced: *any* control kind naming me, by *any* attribute. Under
/// it a run that released an item and later re-claimed it read its **own**
/// release as an order to stop — seen live, and the reason the answer became
/// per-kind. Both constants outlived the rule and stayed public, so the next
/// reader had a plausible, superseded way to ask the question.
pub fn control_kinds() -> impl Iterator<Item = &'static str> {
    INSTRUCTION_ATTRS_BY_KIND.iter().map(|(kind, _)| *kind)
}

/// Whether this marker instructs `run_id` to stop.
pub fn is_control_for(mark: &super::markers::Marker, run_id: &str) -> bool {
    let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
    INSTRUCTION_ATTRS_BY_KIND
        .iter()
        .find(|(name, _)| *name == kind)
        .is_some_and(|(_, attributes)| {
            attributes
                .iter()
                .any(|attribute| mark.get(*attribute).map(String::as_str) == Some(run_id))
        })
}

#[cfg(test)]
mod tests;
